//! Byte-buffer codec for the hot-reload render boundary.
//!
//! The hot-reload cdylib builds a `Vec<Actor>` exactly as the in-lib renderer
//! does, then `encode_actors` serializes it into a flat, self-describing byte
//! buffer. The host calls `decode_actors` to rebuild an owned `Vec<Actor>` on
//! its own allocator. No Rust heap value (`Arc`, `Vec`, `Box`, `String`) ever
//! crosses the FFI boundary by value, so the host `.exe` and the cdylib do not
//! need to share one allocator (`-C prefer-dynamic`) — which is what lets the
//! hot loop use `lto`/release profiles.
//!
//! This module is compiled into the engine rlib, which is statically linked into
//! BOTH the host and the cdylib, so the encoder and decoder cannot desync. The
//! buffer additionally carries a `MAGIC`/`VERSION` header (and the boundary's
//! `build_hash`/`layout_hash` gate the dylib as a whole) so decode fails closed.
//!
//! Lossy normalizations performed on decode (all behavior-preserving for the
//! renderer, which keys off string contents, not variant identity):
//!   * `SpriteSource` is encoded key-only; the host GPU `TextureHandle` is
//!     dropped and re-resolved host-side after decode.
//!   * `TextContent::{Static,Owned,Shared}` all decode to `Shared(Arc<str>)`.
//!   * `Background::Texture`/`Text.font` `&'static str` keys are re-interned
//!     host-side into bounded leaked `&'static str` (see `intern_static`).

use super::actors::{
    Actor, Background, SizeSpec, SpriteSource, TextAlign, TextAttribute, TextContent,
};
use super::anim::{EffectClock, EffectMode, EffectState};
use crate::engine::gfx::{BlendMode, MeshVertex, TexturedMeshVertex};
use glam::Mat4 as Matrix4;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// "DSAW" — deadsync actor wire.
pub const MAGIC: u32 = 0x4453_4157;
/// Bump on any breaking change to the byte layout below.
pub const VERSION: u16 = 1;

// Hard decode limits. The encoder never produces anything near these; they exist
// solely so a corrupt/hostile buffer fails closed instead of OOM-ing or
// overflowing the stack.
const MAX_ACTORS: usize = 1_000_000;
const MAX_DEPTH: usize = 64;
const MAX_STR_LEN: usize = 1 << 20;
const MAX_VERTS: usize = 4_000_000;
const MAX_ATTRS: usize = 1_000_000;
/// Bound on distinct interned font/texture keys, so a buggy/hostile dylib cannot
/// leak host memory without limit.
const MAX_INTERNED_KEYS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    UnexpectedEof,
    BadMagic(u32),
    BadVersion(u16),
    BadTag { what: &'static str, tag: u8 },
    BadUtf8,
    TooLarge { what: &'static str, got: usize, max: usize },
    DepthExceeded,
    TrailingBytes(usize),
    InternOverflow,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexpectedEof => write!(f, "unexpected end of actor-wire buffer"),
            Self::BadMagic(m) => write!(f, "bad actor-wire magic 0x{m:08x}"),
            Self::BadVersion(v) => write!(f, "unsupported actor-wire version {v}"),
            Self::BadTag { what, tag } => write!(f, "invalid {what} tag {tag}"),
            Self::BadUtf8 => write!(f, "invalid utf-8 in actor-wire string"),
            Self::TooLarge { what, got, max } => {
                write!(f, "{what} length {got} exceeds limit {max}")
            }
            Self::DepthExceeded => write!(f, "actor nesting exceeds depth limit"),
            Self::TrailingBytes(n) => write!(f, "{n} trailing bytes after decode"),
            Self::InternOverflow => write!(f, "too many distinct interned keys"),
        }
    }
}

impl std::error::Error for DecodeError {}

// ---------------------------------------------------------------------------
// Static-string interner (decode side only; runs in the host).
// ---------------------------------------------------------------------------

fn interner() -> &'static Mutex<HashMap<String, &'static str>> {
    static INTERNER: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
    INTERNER.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Map a decoded key onto a process-`'static` string, leaking at most
/// `MAX_INTERNED_KEYS` distinct values. Font/texture keys are a tiny fixed set
/// in practice, so the bounded leak never grows during steady-state hot reload.
fn intern_static(s: &str) -> Result<&'static str, DecodeError> {
    let mut map = interner().lock().expect("font interner poisoned");
    if let Some(&existing) = map.get(s) {
        return Ok(existing);
    }
    if map.len() >= MAX_INTERNED_KEYS {
        return Err(DecodeError::InternOverflow);
    }
    let leaked: &'static str = Box::leak(s.to_owned().into_boxed_str());
    map.insert(s.to_owned(), leaked);
    Ok(leaked)
}

// ---------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------

struct Writer<'a> {
    out: &'a mut Vec<u8>,
}

impl<'a> Writer<'a> {
    #[inline]
    fn u8(&mut self, v: u8) {
        self.out.push(v);
    }
    #[inline]
    fn bool(&mut self, v: bool) {
        self.out.push(v as u8);
    }
    #[inline]
    fn u16(&mut self, v: u16) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    #[inline]
    fn i16(&mut self, v: i16) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    #[inline]
    fn u32(&mut self, v: u32) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    #[inline]
    fn i32(&mut self, v: i32) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    #[inline]
    fn u64(&mut self, v: u64) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    #[inline]
    fn f32(&mut self, v: f32) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    #[inline]
    fn f32n<const N: usize>(&mut self, v: &[f32; N]) {
        for &x in v {
            self.f32(x);
        }
    }
    #[inline]
    fn len(&mut self, n: usize) {
        self.u32(n as u32);
    }
    fn str(&mut self, s: &str) {
        self.len(s.len());
        self.out.extend_from_slice(s.as_bytes());
    }
    fn opt_f32n<const N: usize>(&mut self, v: &Option<[f32; N]>) {
        match v {
            Some(a) => {
                self.bool(true);
                self.f32n(a);
            }
            None => self.bool(false),
        }
    }
}

// ---------------------------------------------------------------------------
// Reader
// ---------------------------------------------------------------------------

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    #[inline]
    fn take(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        let end = self.pos.checked_add(n).ok_or(DecodeError::UnexpectedEof)?;
        let slice = self.buf.get(self.pos..end).ok_or(DecodeError::UnexpectedEof)?;
        self.pos = end;
        Ok(slice)
    }
    #[inline]
    fn arr<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        let s = self.take(N)?;
        let mut a = [0u8; N];
        a.copy_from_slice(s);
        Ok(a)
    }
    #[inline]
    fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }
    #[inline]
    fn bool(&mut self) -> Result<bool, DecodeError> {
        Ok(self.u8()? != 0)
    }
    #[inline]
    fn u16(&mut self) -> Result<u16, DecodeError> {
        Ok(u16::from_le_bytes(self.arr()?))
    }
    #[inline]
    fn i16(&mut self) -> Result<i16, DecodeError> {
        Ok(i16::from_le_bytes(self.arr()?))
    }
    #[inline]
    fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(self.arr()?))
    }
    #[inline]
    fn i32(&mut self) -> Result<i32, DecodeError> {
        Ok(i32::from_le_bytes(self.arr()?))
    }
    #[inline]
    fn u64(&mut self) -> Result<u64, DecodeError> {
        Ok(u64::from_le_bytes(self.arr()?))
    }
    #[inline]
    fn f32(&mut self) -> Result<f32, DecodeError> {
        Ok(f32::from_le_bytes(self.arr()?))
    }
    #[inline]
    fn f32n<const N: usize>(&mut self) -> Result<[f32; N], DecodeError> {
        let mut a = [0f32; N];
        for x in a.iter_mut() {
            *x = self.f32()?;
        }
        Ok(a)
    }
    fn count(&mut self, what: &'static str, max: usize) -> Result<usize, DecodeError> {
        let n = self.u32()? as usize;
        if n > max {
            return Err(DecodeError::TooLarge { what, got: n, max });
        }
        Ok(n)
    }
    fn str(&mut self) -> Result<&'a str, DecodeError> {
        let n = self.count("string", MAX_STR_LEN)?;
        let bytes = self.take(n)?;
        std::str::from_utf8(bytes).map_err(|_| DecodeError::BadUtf8)
    }
    fn opt_f32n<const N: usize>(&mut self) -> Result<Option<[f32; N]>, DecodeError> {
        if self.bool()? { Ok(Some(self.f32n()?)) } else { Ok(None) }
    }
}

// ---------------------------------------------------------------------------
// Small enum tag tables
// ---------------------------------------------------------------------------

#[inline]
fn blend_tag(b: BlendMode) -> u8 {
    match b {
        BlendMode::Alpha => 0,
        BlendMode::Add => 1,
        BlendMode::Multiply => 2,
        BlendMode::Subtract => 3,
    }
}
#[inline]
fn blend_from(tag: u8) -> Result<BlendMode, DecodeError> {
    match tag {
        0 => Ok(BlendMode::Alpha),
        1 => Ok(BlendMode::Add),
        2 => Ok(BlendMode::Multiply),
        3 => Ok(BlendMode::Subtract),
        t => Err(DecodeError::BadTag { what: "blend", tag: t }),
    }
}

#[inline]
fn align_tag(a: TextAlign) -> u8 {
    match a {
        TextAlign::Left => 0,
        TextAlign::Center => 1,
        TextAlign::Right => 2,
    }
}
#[inline]
fn align_from(tag: u8) -> Result<TextAlign, DecodeError> {
    match tag {
        0 => Ok(TextAlign::Left),
        1 => Ok(TextAlign::Center),
        2 => Ok(TextAlign::Right),
        t => Err(DecodeError::BadTag { what: "text-align", tag: t }),
    }
}

#[inline]
fn clock_tag(c: EffectClock) -> u8 {
    match c {
        EffectClock::Time => 0,
        EffectClock::Beat => 1,
    }
}
#[inline]
fn clock_from(tag: u8) -> Result<EffectClock, DecodeError> {
    match tag {
        0 => Ok(EffectClock::Time),
        1 => Ok(EffectClock::Beat),
        t => Err(DecodeError::BadTag { what: "effect-clock", tag: t }),
    }
}

#[inline]
fn mode_tag(m: EffectMode) -> u8 {
    match m {
        EffectMode::None => 0,
        EffectMode::DiffuseRamp => 1,
        EffectMode::DiffuseShift => 2,
        EffectMode::GlowShift => 3,
        EffectMode::Pulse => 4,
        EffectMode::Bob => 5,
        EffectMode::Bounce => 6,
        EffectMode::Wag => 7,
        EffectMode::Spin => 8,
    }
}
#[inline]
fn mode_from(tag: u8) -> Result<EffectMode, DecodeError> {
    Ok(match tag {
        0 => EffectMode::None,
        1 => EffectMode::DiffuseRamp,
        2 => EffectMode::DiffuseShift,
        3 => EffectMode::GlowShift,
        4 => EffectMode::Pulse,
        5 => EffectMode::Bob,
        6 => EffectMode::Bounce,
        7 => EffectMode::Wag,
        8 => EffectMode::Spin,
        t => return Err(DecodeError::BadTag { what: "effect-mode", tag: t }),
    })
}

// ---------------------------------------------------------------------------
// Leaf encoders / decoders
// ---------------------------------------------------------------------------

fn write_size(w: &mut Writer, s: SizeSpec) {
    match s {
        SizeSpec::Px(v) => {
            w.u8(0);
            w.f32(v);
        }
        SizeSpec::Fill => w.u8(1),
    }
}
fn read_size(r: &mut Reader) -> Result<SizeSpec, DecodeError> {
    match r.u8()? {
        0 => Ok(SizeSpec::Px(r.f32()?)),
        1 => Ok(SizeSpec::Fill),
        t => Err(DecodeError::BadTag { what: "size-spec", tag: t }),
    }
}

fn write_size2(w: &mut Writer, s: &[SizeSpec; 2]) {
    write_size(w, s[0]);
    write_size(w, s[1]);
}
fn read_size2(r: &mut Reader) -> Result<[SizeSpec; 2], DecodeError> {
    Ok([read_size(r)?, read_size(r)?])
}

fn write_effect(w: &mut Writer, e: &EffectState) {
    w.u8(clock_tag(e.clock));
    w.u8(mode_tag(e.mode));
    w.f32n(&e.color1);
    w.f32n(&e.color2);
    w.f32(e.period);
    w.f32(e.offset);
    w.f32n(&e.timing);
    w.f32n(&e.magnitude);
}
fn read_effect(r: &mut Reader) -> Result<EffectState, DecodeError> {
    Ok(EffectState {
        clock: clock_from(r.u8()?)?,
        mode: mode_from(r.u8()?)?,
        color1: r.f32n()?,
        color2: r.f32n()?,
        period: r.f32()?,
        offset: r.f32()?,
        timing: r.f32n()?,
        magnitude: r.f32n()?,
    })
}

/// `SpriteSource` is encoded KEY-ONLY: the host GPU `TextureHandle`/generation
/// is dropped and re-resolved host-side after decode. Decodes to `Solid` or the
/// owned `Texture(Arc<str>)` key variant.
fn write_source(w: &mut Writer, s: &SpriteSource) {
    match s.texture_key() {
        None => w.u8(0),
        Some(key) => {
            w.u8(1);
            w.str(key);
        }
    }
}
fn read_source(r: &mut Reader) -> Result<SpriteSource, DecodeError> {
    match r.u8()? {
        0 => Ok(SpriteSource::Solid),
        1 => Ok(SpriteSource::Texture(Arc::from(r.str()?))),
        t => Err(DecodeError::BadTag { what: "sprite-source", tag: t }),
    }
}

fn write_text_content(w: &mut Writer, c: &TextContent) {
    w.str(c.as_str());
}
fn read_text_content(r: &mut Reader) -> Result<TextContent, DecodeError> {
    Ok(TextContent::Shared(Arc::from(r.str()?)))
}

fn write_attribute(w: &mut Writer, a: &TextAttribute) {
    w.u64(a.start as u64);
    w.u64(a.length as u64);
    w.f32n(&a.color);
    match &a.vertex_colors {
        Some(vc) => {
            w.bool(true);
            for c in vc {
                w.f32n(c);
            }
        }
        None => w.bool(false),
    }
    w.opt_f32n(&a.glow);
}
fn read_attribute(r: &mut Reader) -> Result<TextAttribute, DecodeError> {
    let start = r.u64()? as usize;
    let length = r.u64()? as usize;
    let color = r.f32n()?;
    let vertex_colors = if r.bool()? {
        Some([r.f32n()?, r.f32n()?, r.f32n()?, r.f32n()?])
    } else {
        None
    };
    let glow = r.opt_f32n()?;
    Ok(TextAttribute { start, length, color, vertex_colors, glow })
}

fn write_mat4(w: &mut Writer, m: &Matrix4) {
    w.f32n(&m.to_cols_array());
}
fn read_mat4(r: &mut Reader) -> Result<Matrix4, DecodeError> {
    Ok(Matrix4::from_cols_array(&r.f32n::<16>()?))
}

fn write_mesh_vertices(w: &mut Writer, verts: &[MeshVertex]) {
    w.len(verts.len());
    for v in verts {
        w.f32n(&v.pos);
        w.f32n(&v.color);
    }
}
fn read_mesh_vertices(r: &mut Reader) -> Result<Arc<[MeshVertex]>, DecodeError> {
    let n = r.count("mesh-vertices", MAX_VERTS)?;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        v.push(MeshVertex { pos: r.f32n()?, color: r.f32n()? });
    }
    Ok(Arc::from(v))
}

fn write_tmesh_vertices(w: &mut Writer, verts: &[TexturedMeshVertex]) {
    w.len(verts.len());
    for v in verts {
        w.f32n(&v.pos);
        w.f32n(&v.uv);
        w.f32n(&v.color);
        w.f32n(&v.tex_matrix_scale);
    }
}
fn read_tmesh_vertices(r: &mut Reader) -> Result<Arc<[TexturedMeshVertex]>, DecodeError> {
    let n = r.count("tmesh-vertices", MAX_VERTS)?;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        v.push(TexturedMeshVertex {
            pos: r.f32n()?,
            uv: r.f32n()?,
            color: r.f32n()?,
            tex_matrix_scale: r.f32n()?,
        });
    }
    Ok(Arc::from(v))
}

fn write_background(w: &mut Writer, bg: &Option<Background>) {
    match bg {
        None => w.u8(0),
        Some(Background::Color(c)) => {
            w.u8(1);
            w.f32n(c);
        }
        Some(Background::Texture(key)) => {
            w.u8(2);
            w.str(key);
        }
    }
}
fn read_background(r: &mut Reader) -> Result<Option<Background>, DecodeError> {
    match r.u8()? {
        0 => Ok(None),
        1 => Ok(Some(Background::Color(r.f32n()?))),
        2 => Ok(Some(Background::Texture(intern_static(r.str()?)?))),
        t => Err(DecodeError::BadTag { what: "background", tag: t }),
    }
}

fn write_opt_blend(w: &mut Writer, b: &Option<BlendMode>) {
    match b {
        Some(b) => {
            w.bool(true);
            w.u8(blend_tag(*b));
        }
        None => w.bool(false),
    }
}
fn read_opt_blend(r: &mut Reader) -> Result<Option<BlendMode>, DecodeError> {
    if r.bool()? { Ok(Some(blend_from(r.u8()?)?)) } else { Ok(None) }
}

// ---------------------------------------------------------------------------
// Actor variant tags
// ---------------------------------------------------------------------------

const T_SPRITE: u8 = 0;
const T_TEXT: u8 = 1;
const T_MESH: u8 = 2;
const T_TEXTURED_MESH: u8 = 3;
const T_FRAME: u8 = 4;
const T_SHARED_FRAME: u8 = 5;
const T_CAMERA: u8 = 6;
const T_CAMERA_PUSH: u8 = 7;
const T_CAMERA_POP: u8 = 8;
const T_SHADOW: u8 = 9;

fn write_children(w: &mut Writer, children: &[Actor]) {
    w.len(children.len());
    for c in children {
        write_actor(w, c);
    }
}
fn read_children(r: &mut Reader, depth: usize) -> Result<Vec<Actor>, DecodeError> {
    let n = r.count("children", MAX_ACTORS)?;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        v.push(read_actor(r, depth + 1)?);
    }
    Ok(v)
}

fn write_actor(w: &mut Writer, a: &Actor) {
    match a {
        Actor::Sprite {
            align,
            offset,
            world_z,
            size,
            source,
            tint,
            glow,
            z,
            cell,
            grid,
            uv_rect,
            visible,
            flip_x,
            flip_y,
            cropleft,
            cropright,
            croptop,
            cropbottom,
            fadeleft,
            faderight,
            fadetop,
            fadebottom,
            blend,
            mask_source,
            mask_dest,
            rot_x_deg,
            rot_y_deg,
            rot_z_deg,
            local_offset,
            local_offset_rot_sin_cos,
            texcoordvelocity,
            animate,
            state_delay,
            scale,
            shadow_len,
            shadow_color,
            effect,
        } => {
            w.u8(T_SPRITE);
            w.f32n(align);
            w.f32n(offset);
            w.f32(*world_z);
            write_size2(w, size);
            write_source(w, source);
            w.f32n(tint);
            w.f32n(glow);
            w.i16(*z);
            write_opt_uu(w, cell);
            write_opt_uu(w, grid);
            w.opt_f32n(uv_rect);
            w.bool(*visible);
            w.bool(*flip_x);
            w.bool(*flip_y);
            w.f32(*cropleft);
            w.f32(*cropright);
            w.f32(*croptop);
            w.f32(*cropbottom);
            w.f32(*fadeleft);
            w.f32(*faderight);
            w.f32(*fadetop);
            w.f32(*fadebottom);
            w.u8(blend_tag(*blend));
            w.bool(*mask_source);
            w.bool(*mask_dest);
            w.f32(*rot_x_deg);
            w.f32(*rot_y_deg);
            w.f32(*rot_z_deg);
            w.f32n(local_offset);
            w.f32n(local_offset_rot_sin_cos);
            w.opt_f32n(texcoordvelocity);
            w.bool(*animate);
            w.f32(*state_delay);
            w.f32n(scale);
            w.f32n(shadow_len);
            w.f32n(shadow_color);
            write_effect(w, effect);
        }
        Actor::Text {
            align,
            offset,
            local_transform,
            color,
            stroke_color,
            glow,
            font,
            content,
            attributes,
            align_text,
            z,
            scale,
            fit_width,
            fit_height,
            line_spacing,
            wrap_width_pixels,
            max_width,
            max_height,
            max_w_pre_zoom,
            max_h_pre_zoom,
            jitter,
            distortion,
            clip,
            mask_dest,
            blend,
            shadow_len,
            shadow_color,
            effect,
        } => {
            w.u8(T_TEXT);
            w.f32n(align);
            w.f32n(offset);
            write_mat4(w, local_transform);
            w.f32n(color);
            w.opt_f32n(stroke_color);
            w.f32n(glow);
            w.str(font);
            write_text_content(w, content);
            w.len(attributes.len());
            for at in attributes {
                write_attribute(w, at);
            }
            w.u8(align_tag(*align_text));
            w.i16(*z);
            w.f32n(scale);
            write_opt_f32(w, fit_width);
            write_opt_f32(w, fit_height);
            write_opt_i32(w, line_spacing);
            write_opt_i32(w, wrap_width_pixels);
            write_opt_f32(w, max_width);
            write_opt_f32(w, max_height);
            w.bool(*max_w_pre_zoom);
            w.bool(*max_h_pre_zoom);
            w.bool(*jitter);
            w.f32(*distortion);
            w.opt_f32n(clip);
            w.bool(*mask_dest);
            w.u8(blend_tag(*blend));
            w.f32n(shadow_len);
            w.f32n(shadow_color);
            write_effect(w, effect);
        }
        Actor::Mesh { align, offset, size, vertices, visible, blend, z } => {
            w.u8(T_MESH);
            w.f32n(align);
            w.f32n(offset);
            write_size2(w, size);
            write_mesh_vertices(w, vertices);
            w.bool(*visible);
            w.u8(blend_tag(*blend));
            w.i16(*z);
        }
        Actor::TexturedMesh {
            align,
            offset,
            world_z,
            size,
            local_transform,
            texture,
            tint,
            glow,
            vertices,
            geom_cache_key,
            uv_scale,
            uv_offset,
            uv_tex_shift,
            depth_test,
            visible,
            blend,
            z,
        } => {
            w.u8(T_TEXTURED_MESH);
            w.f32n(align);
            w.f32n(offset);
            w.f32(*world_z);
            write_size2(w, size);
            write_mat4(w, local_transform);
            w.str(texture);
            w.f32n(tint);
            w.f32n(glow);
            write_tmesh_vertices(w, vertices);
            w.u64(*geom_cache_key);
            w.f32n(uv_scale);
            w.f32n(uv_offset);
            w.f32n(uv_tex_shift);
            w.bool(*depth_test);
            w.bool(*visible);
            w.u8(blend_tag(*blend));
            w.i16(*z);
        }
        Actor::Frame { align, offset, size, children, background, z } => {
            w.u8(T_FRAME);
            w.f32n(align);
            w.f32n(offset);
            write_size2(w, size);
            write_children(w, children);
            write_background(w, background);
            w.i16(*z);
        }
        Actor::SharedFrame { align, offset, size, children, background, z, tint, blend } => {
            w.u8(T_SHARED_FRAME);
            w.f32n(align);
            w.f32n(offset);
            write_size2(w, size);
            write_children(w, children);
            write_background(w, background);
            w.i16(*z);
            w.f32n(tint);
            write_opt_blend(w, blend);
        }
        Actor::Camera { view_proj, children } => {
            w.u8(T_CAMERA);
            write_mat4(w, view_proj);
            write_children(w, children);
        }
        Actor::CameraPush { view_proj } => {
            w.u8(T_CAMERA_PUSH);
            write_mat4(w, view_proj);
        }
        Actor::CameraPop => {
            w.u8(T_CAMERA_POP);
        }
        Actor::Shadow { len, color, child } => {
            w.u8(T_SHADOW);
            w.f32n(len);
            w.f32n(color);
            write_actor(w, child);
        }
    }
}

fn read_actor(r: &mut Reader, depth: usize) -> Result<Actor, DecodeError> {
    if depth > MAX_DEPTH {
        return Err(DecodeError::DepthExceeded);
    }
    let tag = r.u8()?;
    Ok(match tag {
        T_SPRITE => Actor::Sprite {
            align: r.f32n()?,
            offset: r.f32n()?,
            world_z: r.f32()?,
            size: read_size2(r)?,
            source: read_source(r)?,
            tint: r.f32n()?,
            glow: r.f32n()?,
            z: r.i16()?,
            cell: read_opt_uu(r)?,
            grid: read_opt_uu(r)?,
            uv_rect: r.opt_f32n()?,
            visible: r.bool()?,
            flip_x: r.bool()?,
            flip_y: r.bool()?,
            cropleft: r.f32()?,
            cropright: r.f32()?,
            croptop: r.f32()?,
            cropbottom: r.f32()?,
            fadeleft: r.f32()?,
            faderight: r.f32()?,
            fadetop: r.f32()?,
            fadebottom: r.f32()?,
            blend: blend_from(r.u8()?)?,
            mask_source: r.bool()?,
            mask_dest: r.bool()?,
            rot_x_deg: r.f32()?,
            rot_y_deg: r.f32()?,
            rot_z_deg: r.f32()?,
            local_offset: r.f32n()?,
            local_offset_rot_sin_cos: r.f32n()?,
            texcoordvelocity: r.opt_f32n()?,
            animate: r.bool()?,
            state_delay: r.f32()?,
            scale: r.f32n()?,
            shadow_len: r.f32n()?,
            shadow_color: r.f32n()?,
            effect: read_effect(r)?,
        },
        T_TEXT => {
            let align = r.f32n()?;
            let offset = r.f32n()?;
            let local_transform = read_mat4(r)?;
            let color = r.f32n()?;
            let stroke_color = r.opt_f32n()?;
            let glow = r.f32n()?;
            let font = intern_static(r.str()?)?;
            let content = read_text_content(r)?;
            let n_attr = r.count("text-attributes", MAX_ATTRS)?;
            let mut attributes = Vec::with_capacity(n_attr);
            for _ in 0..n_attr {
                attributes.push(read_attribute(r)?);
            }
            Actor::Text {
                align,
                offset,
                local_transform,
                color,
                stroke_color,
                glow,
                font,
                content,
                attributes,
                align_text: align_from(r.u8()?)?,
                z: r.i16()?,
                scale: r.f32n()?,
                fit_width: read_opt_f32(r)?,
                fit_height: read_opt_f32(r)?,
                line_spacing: read_opt_i32(r)?,
                wrap_width_pixels: read_opt_i32(r)?,
                max_width: read_opt_f32(r)?,
                max_height: read_opt_f32(r)?,
                max_w_pre_zoom: r.bool()?,
                max_h_pre_zoom: r.bool()?,
                jitter: r.bool()?,
                distortion: r.f32()?,
                clip: r.opt_f32n()?,
                mask_dest: r.bool()?,
                blend: blend_from(r.u8()?)?,
                shadow_len: r.f32n()?,
                shadow_color: r.f32n()?,
                effect: read_effect(r)?,
            }
        }
        T_MESH => Actor::Mesh {
            align: r.f32n()?,
            offset: r.f32n()?,
            size: read_size2(r)?,
            vertices: read_mesh_vertices(r)?,
            visible: r.bool()?,
            blend: blend_from(r.u8()?)?,
            z: r.i16()?,
        },
        T_TEXTURED_MESH => Actor::TexturedMesh {
            align: r.f32n()?,
            offset: r.f32n()?,
            world_z: r.f32()?,
            size: read_size2(r)?,
            local_transform: read_mat4(r)?,
            texture: Arc::from(r.str()?),
            tint: r.f32n()?,
            glow: r.f32n()?,
            vertices: read_tmesh_vertices(r)?,
            geom_cache_key: r.u64()?,
            uv_scale: r.f32n()?,
            uv_offset: r.f32n()?,
            uv_tex_shift: r.f32n()?,
            depth_test: r.bool()?,
            visible: r.bool()?,
            blend: blend_from(r.u8()?)?,
            z: r.i16()?,
        },
        T_FRAME => Actor::Frame {
            align: r.f32n()?,
            offset: r.f32n()?,
            size: read_size2(r)?,
            children: read_children(r, depth)?,
            background: read_background(r)?,
            z: r.i16()?,
        },
        T_SHARED_FRAME => Actor::SharedFrame {
            align: r.f32n()?,
            offset: r.f32n()?,
            size: read_size2(r)?,
            children: Arc::from(read_children(r, depth)?),
            background: read_background(r)?,
            z: r.i16()?,
            tint: r.f32n()?,
            blend: read_opt_blend(r)?,
        },
        T_CAMERA => Actor::Camera {
            view_proj: read_mat4(r)?,
            children: read_children(r, depth)?,
        },
        T_CAMERA_PUSH => Actor::CameraPush { view_proj: read_mat4(r)? },
        T_CAMERA_POP => Actor::CameraPop,
        T_SHADOW => Actor::Shadow {
            len: r.f32n()?,
            color: r.f32n()?,
            child: Box::new(read_actor(r, depth + 1)?),
        },
        t => return Err(DecodeError::BadTag { what: "actor", tag: t }),
    })
}

// Option<(u32, u32)> helpers (sprite cell/grid).
fn write_opt_uu(w: &mut Writer, v: &Option<(u32, u32)>) {
    match v {
        Some((a, b)) => {
            w.bool(true);
            w.u32(*a);
            w.u32(*b);
        }
        None => w.bool(false),
    }
}
fn read_opt_uu(r: &mut Reader) -> Result<Option<(u32, u32)>, DecodeError> {
    if r.bool()? { Ok(Some((r.u32()?, r.u32()?))) } else { Ok(None) }
}

fn write_opt_f32(w: &mut Writer, v: &Option<f32>) {
    match v {
        Some(x) => {
            w.bool(true);
            w.f32(*x);
        }
        None => w.bool(false),
    }
}
fn read_opt_f32(r: &mut Reader) -> Result<Option<f32>, DecodeError> {
    if r.bool()? { Ok(Some(r.f32()?)) } else { Ok(None) }
}

fn write_opt_i32(w: &mut Writer, v: &Option<i32>) {
    match v {
        Some(x) => {
            w.bool(true);
            w.i32(*x);
        }
        None => w.bool(false),
    }
}
fn read_opt_i32(r: &mut Reader) -> Result<Option<i32>, DecodeError> {
    if r.bool()? { Ok(Some(r.i32()?)) } else { Ok(None) }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Serialize `actors` into `out` (cleared first). Infallible: the encoder never
/// rejects well-formed in-memory actors. The buffer begins with `MAGIC` +
/// `VERSION` so the decoder can fail closed on a stale/foreign producer.
pub fn encode_actors(actors: &[Actor], out: &mut Vec<u8>) {
    out.clear();
    let mut w = Writer { out };
    w.u32(MAGIC);
    w.u16(VERSION);
    w.len(actors.len());
    for a in actors {
        write_actor(&mut w, a);
    }
}

/// Decode a buffer produced by [`encode_actors`] into a fresh, host-owned
/// `Vec<Actor>`. Every length is bounds-checked and the input must be fully
/// consumed, so a corrupt buffer yields `Err` rather than UB.
///
/// Decoded `SpriteSource`/`TexturedMesh` carry texture KEYS only; the caller
/// must re-resolve GPU handles host-side before rendering (mirrors the in-lib
/// renderer's key→handle resolution).
pub fn decode_actors(buf: &[u8]) -> Result<Vec<Actor>, DecodeError> {
    let mut r = Reader { buf, pos: 0 };
    let magic = r.u32()?;
    if magic != MAGIC {
        return Err(DecodeError::BadMagic(magic));
    }
    let version = r.u16()?;
    if version != VERSION {
        return Err(DecodeError::BadVersion(version));
    }
    let n = r.count("actors", MAX_ACTORS)?;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(read_actor(&mut r, 0)?);
    }
    if r.pos != buf.len() {
        return Err(DecodeError::TrailingBytes(buf.len() - r.pos));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(actors: &[Actor]) -> Vec<Actor> {
        let mut buf = Vec::new();
        encode_actors(actors, &mut buf);
        decode_actors(&buf).expect("decode should succeed")
    }

    fn sample_effect() -> EffectState {
        EffectState {
            clock: EffectClock::Beat,
            mode: EffectMode::Pulse,
            color1: [0.1, 0.2, 0.3, 0.4],
            color2: [0.5, 0.6, 0.7, 0.8],
            period: 2.5,
            offset: 0.25,
            timing: [0.1, 0.2, 0.3, 0.4, 0.5],
            magnitude: [1.0, 2.0, 3.0],
        }
    }

    fn sample_sprite() -> Actor {
        Actor::Sprite {
            align: [0.5, 0.5],
            offset: [10.0, 20.0],
            world_z: 1.0,
            size: [SizeSpec::Px(64.0), SizeSpec::Fill],
            source: SpriteSource::TextureStatic("logo"),
            tint: [1.0, 0.9, 0.8, 0.7],
            glow: [0.0, 0.0, 0.0, 0.0],
            z: -3,
            cell: Some((1, 2)),
            grid: None,
            uv_rect: Some([0.0, 0.0, 1.0, 1.0]),
            visible: true,
            flip_x: false,
            flip_y: true,
            cropleft: 0.1,
            cropright: 0.2,
            croptop: 0.3,
            cropbottom: 0.4,
            fadeleft: 0.5,
            faderight: 0.6,
            fadetop: 0.7,
            fadebottom: 0.8,
            blend: BlendMode::Add,
            mask_source: true,
            mask_dest: false,
            rot_x_deg: 12.0,
            rot_y_deg: 34.0,
            rot_z_deg: 56.0,
            local_offset: [3.0, 4.0],
            local_offset_rot_sin_cos: [0.0, 1.0],
            texcoordvelocity: Some([0.01, 0.02]),
            animate: true,
            state_delay: 0.016,
            scale: [2.0, 2.0],
            shadow_len: [1.0, 1.0],
            shadow_color: [0.0, 0.0, 0.0, 0.5],
            effect: sample_effect(),
        }
    }

    fn sample_text() -> Actor {
        Actor::Text {
            align: [0.5, 0.0],
            offset: [1.0, 2.0],
            local_transform: Matrix4::from_scale(glam::Vec3::new(1.0, 2.0, 3.0)),
            color: [1.0, 1.0, 1.0, 1.0],
            stroke_color: Some([0.0, 0.0, 0.0, 1.0]),
            glow: [0.1, 0.1, 0.1, 0.1],
            font: "miso",
            content: TextContent::Owned("Hello".to_string()),
            attributes: vec![TextAttribute {
                start: 0,
                length: 5,
                color: [1.0, 0.0, 0.0, 1.0],
                vertex_colors: Some([[1.0, 0.0, 0.0, 1.0]; 4]),
                glow: Some([0.0, 1.0, 0.0, 1.0]),
            }],
            align_text: TextAlign::Center,
            z: 7,
            scale: [1.0, 1.0],
            fit_width: Some(100.0),
            fit_height: None,
            line_spacing: Some(-2),
            wrap_width_pixels: Some(320),
            max_width: Some(200.0),
            max_height: None,
            max_w_pre_zoom: true,
            max_h_pre_zoom: false,
            jitter: true,
            distortion: 0.5,
            clip: Some([0.0, 0.0, 10.0, 10.0]),
            mask_dest: false,
            blend: BlendMode::Alpha,
            shadow_len: [2.0, 2.0],
            shadow_color: [0.0, 0.0, 0.0, 0.25],
            effect: EffectState::default(),
        }
    }

    fn assert_same(a: &Actor, b: &Actor) {
        // Compare via re-encoding: structural equality without deriving PartialEq
        // on the whole Actor tree (which carries f32/handles).
        let mut ba = Vec::new();
        let mut bb = Vec::new();
        encode_actors(std::slice::from_ref(a), &mut ba);
        encode_actors(std::slice::from_ref(b), &mut bb);
        assert_eq!(ba, bb, "actors differ after roundtrip");
    }

    #[test]
    fn roundtrip_sprite() {
        let actors = vec![sample_sprite()];
        let out = roundtrip(&actors);
        assert_eq!(out.len(), 1);
        assert_same(&actors[0], &out[0]);
    }

    #[test]
    fn roundtrip_text() {
        let actors = vec![sample_text()];
        let out = roundtrip(&actors);
        assert_eq!(out.len(), 1);
        assert_same(&actors[0], &out[0]);
    }

    #[test]
    fn roundtrip_mesh() {
        let actors = vec![Actor::Mesh {
            align: [0.0, 0.0],
            offset: [0.0, 0.0],
            size: [SizeSpec::Fill, SizeSpec::Fill],
            vertices: Arc::from(vec![
                MeshVertex { pos: [0.0, 0.0], color: [1.0, 0.0, 0.0, 1.0] },
                MeshVertex { pos: [1.0, 1.0], color: [0.0, 1.0, 0.0, 1.0] },
            ]),
            visible: true,
            blend: BlendMode::Multiply,
            z: 0,
        }];
        let out = roundtrip(&actors);
        assert_same(&actors[0], &out[0]);
    }

    #[test]
    fn roundtrip_textured_mesh() {
        let actors = vec![Actor::TexturedMesh {
            align: [0.0, 0.0],
            offset: [0.0, 0.0],
            world_z: 0.0,
            size: [SizeSpec::Px(1.0), SizeSpec::Px(1.0)],
            local_transform: Matrix4::IDENTITY,
            texture: Arc::from("atlas"),
            tint: [1.0, 1.0, 1.0, 1.0],
            glow: [0.0, 0.0, 0.0, 0.0],
            vertices: Arc::from(vec![TexturedMeshVertex {
                pos: [0.0, 0.0, 0.0],
                uv: [0.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
                tex_matrix_scale: [1.0, 1.0],
            }]),
            geom_cache_key: 0xDEAD_BEEF,
            uv_scale: [1.0, 1.0],
            uv_offset: [0.0, 0.0],
            uv_tex_shift: [0.0, 0.0],
            depth_test: true,
            visible: true,
            blend: BlendMode::Subtract,
            z: 1,
        }];
        let out = roundtrip(&actors);
        assert_same(&actors[0], &out[0]);
    }

    #[test]
    fn roundtrip_nested_frames() {
        let actors = vec![
            Actor::Frame {
                align: [0.0, 0.0],
                offset: [0.0, 0.0],
                size: [SizeSpec::Fill, SizeSpec::Fill],
                children: vec![sample_sprite(), sample_text()],
                background: Some(Background::Color([0.0, 0.0, 0.0, 1.0])),
                z: 0,
            },
            Actor::SharedFrame {
                align: [0.0, 0.0],
                offset: [0.0, 0.0],
                size: [SizeSpec::Fill, SizeSpec::Fill],
                children: Arc::from(vec![sample_sprite()]),
                background: None,
                z: 1,
                tint: [1.0, 1.0, 1.0, 1.0],
                blend: Some(BlendMode::Add),
            },
            Actor::Camera {
                view_proj: Matrix4::IDENTITY,
                children: vec![Actor::CameraPush { view_proj: Matrix4::IDENTITY }, Actor::CameraPop],
            },
            Actor::Shadow {
                len: [1.0, 1.0],
                color: [0.0, 0.0, 0.0, 0.5],
                child: Box::new(sample_sprite()),
            },
        ];
        let out = roundtrip(&actors);
        assert_eq!(out.len(), actors.len());
        for (a, b) in actors.iter().zip(out.iter()) {
            assert_same(a, b);
        }
    }

    #[test]
    fn source_normalizes_to_key_only() {
        let mut sprite = sample_sprite();
        if let Actor::Sprite { source, .. } = &mut sprite {
            *source = SpriteSource::Solid;
        }
        let out = roundtrip(std::slice::from_ref(&sprite));
        match &out[0] {
            Actor::Sprite { source, .. } => {
                assert!(matches!(source, SpriteSource::Solid));
            }
            _ => panic!("expected sprite"),
        }
    }

    #[test]
    fn static_source_normalizes_to_owned_key() {
        // TextureStatic(&'static) must decode to the owned Texture(Arc<str>) key.
        let sprite = sample_sprite();
        let out = roundtrip(std::slice::from_ref(&sprite));
        match &out[0] {
            Actor::Sprite { source, .. } => {
                assert!(matches!(source, SpriteSource::Texture(_)));
                assert_eq!(source.texture_key(), Some("logo"));
            }
            _ => panic!("expected sprite"),
        }
    }

    #[test]
    fn text_content_normalizes_to_shared() {
        let actors = vec![sample_text()];
        let out = roundtrip(&actors);
        match &out[0] {
            Actor::Text { content, .. } => {
                assert!(matches!(content, TextContent::Shared(_)));
                assert_eq!(content.as_str(), "Hello");
            }
            _ => panic!("expected text"),
        }
    }

    // --- negative / hardening tests ---

    #[test]
    fn empty_buffer_errors() {
        assert!(matches!(decode_actors(&[]), Err(DecodeError::UnexpectedEof)));
    }

    #[test]
    fn bad_magic_errors() {
        let mut buf = Vec::new();
        encode_actors(&[sample_sprite()], &mut buf);
        buf[0] ^= 0xFF;
        assert!(matches!(decode_actors(&buf), Err(DecodeError::BadMagic(_))));
    }

    #[test]
    fn bad_version_errors() {
        let mut buf = Vec::new();
        encode_actors(&[], &mut buf);
        // version is the 2 bytes right after the 4 magic bytes
        buf[4] = 0xFF;
        buf[5] = 0xFF;
        assert!(matches!(decode_actors(&buf), Err(DecodeError::BadVersion(_))));
    }

    #[test]
    fn truncation_errors_at_every_prefix() {
        let mut buf = Vec::new();
        encode_actors(&[sample_sprite(), sample_text()], &mut buf);
        for cut in 0..buf.len() {
            assert!(
                decode_actors(&buf[..cut]).is_err(),
                "prefix of len {cut} should not decode"
            );
        }
    }

    #[test]
    fn trailing_bytes_error() {
        let mut buf = Vec::new();
        encode_actors(&[sample_sprite()], &mut buf);
        buf.push(0);
        assert!(matches!(decode_actors(&buf), Err(DecodeError::TrailingBytes(1))));
    }

    #[test]
    fn bad_actor_tag_errors() {
        let mut buf = Vec::new();
        encode_actors(&[], &mut buf);
        // append a bogus actor count of 1 then an invalid tag
        // rebuild: magic + version + count(1) + tag(255)
        let mut hand = Vec::new();
        hand.extend_from_slice(&MAGIC.to_le_bytes());
        hand.extend_from_slice(&VERSION.to_le_bytes());
        hand.extend_from_slice(&1u32.to_le_bytes());
        hand.push(255);
        assert!(matches!(
            decode_actors(&hand),
            Err(DecodeError::BadTag { what: "actor", .. })
        ));
    }

    #[test]
    fn oversized_count_rejected() {
        let mut hand = Vec::new();
        hand.extend_from_slice(&MAGIC.to_le_bytes());
        hand.extend_from_slice(&VERSION.to_le_bytes());
        hand.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            decode_actors(&hand),
            Err(DecodeError::TooLarge { what: "actors", .. })
        ));
    }

    #[test]
    fn empty_actor_list_roundtrips() {
        let out = roundtrip(&[]);
        assert!(out.is_empty());
    }
}
