mod backends;

pub mod draw_prep;

#[cfg(not(target_pointer_width = "32"))]
use crate::engine::gfx::backends::vulkan;
use crate::engine::gfx::backends::{opengl, software, wgpu_core};
use glam::Mat4 as Matrix4;
use glow::HasContext;
use image::RgbaImage;
use std::ops::Deref;
use std::{collections::HashMap, error::Error, hash::BuildHasherDefault, str::FromStr, sync::Arc};
use twox_hash::XxHash64;
use winit::window::Window;

// --- Public Data Contract ---
pub type TextureHandle = u64;
pub const INVALID_TEXTURE_HANDLE: TextureHandle = 0;
pub type FastU64Map<V> = HashMap<u64, V, BuildHasherDefault<XxHash64>>;
pub type TextureHandleMap<V> = FastU64Map<V>;
pub type TMeshCacheKey = u64;
pub const INVALID_TMESH_CACHE_KEY: TMeshCacheKey = 0;

#[derive(Clone)]
pub struct RenderList {
    pub clear_color: [f32; 4],
    pub cameras: Vec<Matrix4>,
    pub objects: Vec<RenderObject>,
}
#[derive(Clone)]
pub struct RenderObject {
    pub object_type: ObjectType,
    pub texture_handle: TextureHandle,
    pub transform: Matrix4,
    pub blend: BlendMode,
    pub z: i16,
    pub order: u32,
    pub camera: u8,
}

#[repr(C)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    serde::Serialize,
    serde::Deserialize,
    bytemuck::Pod,
    bytemuck::Zeroable,
)]
pub struct MeshVertex {
    pub pos: [f32; 2],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(
    Clone, Copy, Debug, serde::Serialize, serde::Deserialize, bytemuck::Pod, bytemuck::Zeroable,
)]
pub struct TexturedMeshVertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub tex_matrix_scale: [f32; 2],
    pub color: [f32; 4],
}

impl Default for TexturedMeshVertex {
    #[inline(always)]
    fn default() -> Self {
        Self {
            pos: [0.0, 0.0, 0.0],
            uv: [0.0, 0.0],
            tex_matrix_scale: [1.0, 1.0],
            color: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

#[derive(Clone)]
pub enum TexturedMeshVertices {
    Shared(Arc<[TexturedMeshVertex]>),
    Transient(Arc<Vec<TexturedMeshVertex>>),
}

impl AsRef<[TexturedMeshVertex]> for TexturedMeshVertices {
    #[inline(always)]
    fn as_ref(&self) -> &[TexturedMeshVertex] {
        match self {
            Self::Shared(vertices) => vertices.as_ref(),
            Self::Transient(vertices) => vertices.as_slice(),
        }
    }
}

impl Deref for TexturedMeshVertices {
    type Target = [TexturedMeshVertex];

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshMode {
    Triangles,
}

#[derive(Clone)]
pub enum ObjectType {
    Sprite {
        center: [f32; 4],
        size: [f32; 2],
        rot_sin_cos: [f32; 2],
        tint: [f32; 4],
        uv_scale: [f32; 2],
        uv_offset: [f32; 2],
        local_offset: [f32; 2],
        local_offset_rot_sin_cos: [f32; 2],
        edge_fade: [f32; 4],
    },
    Mesh {
        tint: [f32; 4],
        vertices: Arc<[MeshVertex]>,
        mode: MeshMode,
    },
    #[allow(dead_code)]
    TexturedMesh {
        tint: [f32; 4],
        vertices: TexturedMeshVertices,
        geom_cache_key: TMeshCacheKey,
        mode: MeshMode,
        uv_scale: [f32; 2],
        uv_offset: [f32; 2],
        uv_tex_shift: [f32; 2],
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SamplerFilter {
    Linear,
    Nearest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SamplerWrap {
    Clamp,
    Repeat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SamplerDesc {
    pub filter: SamplerFilter,
    pub wrap: SamplerWrap,
    pub mipmaps: bool,
}

impl Default for SamplerDesc {
    #[inline(always)]
    fn default() -> Self {
        Self {
            filter: SamplerFilter::Linear,
            wrap: SamplerWrap::Clamp,
            mipmaps: false,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendMode {
    Alpha,
    Add,
    #[allow(dead_code)]
    Multiply,
    #[allow(dead_code)]
    Subtract,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentModePolicy {
    Mailbox,
    Immediate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PresentModeTrace {
    #[default]
    Unknown,
    Fifo,
    FifoRelaxed,
    Mailbox,
    Immediate,
}

impl PresentModeTrace {
    #[inline(always)]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Fifo => "fifo",
            Self::FifoRelaxed => "fifo_relaxed",
            Self::Mailbox => "mailbox",
            Self::Immediate => "immediate",
        }
    }
}

impl core::fmt::Display for PresentModeTrace {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ClockDomainTrace {
    #[default]
    Unknown,
    Device,
    Monotonic,
    MonotonicRaw,
    Qpc,
}

impl ClockDomainTrace {
    #[inline(always)]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Device => "device",
            Self::Monotonic => "monotonic",
            Self::MonotonicRaw => "monotonic_raw",
            Self::Qpc => "qpc",
        }
    }
}

impl core::fmt::Display for ClockDomainTrace {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PresentStats {
    pub mode: PresentModeTrace,
    pub display_clock: ClockDomainTrace,
    pub host_clock: ClockDomainTrace,
    pub in_flight_images: u8,
    pub waited_for_image: bool,
    pub applied_back_pressure: bool,
    pub queue_idle_waited: bool,
    pub suboptimal: bool,
    pub submitted_present_id: u32,
    pub completed_present_id: u32,
    pub refresh_ns: u64,
    pub actual_interval_ns: u64,
    pub present_margin_ns: u64,
    pub host_present_ns: u64,
    pub calibration_error_ns: u64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DrawStats {
    pub vertices: u32,
    pub acquire_us: u32,
    pub submit_us: u32,
    pub present_us: u32,
    pub present_stats: PresentStats,
    pub gpu_wait_us: u32,
    pub backend_setup_us: u32,
    pub backend_prepare_us: u32,
    pub backend_record_us: u32,
}

// --- Public API Facade ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendType {
    #[cfg(not(target_pointer_width = "32"))]
    Vulkan,
    #[cfg(not(target_pointer_width = "32"))]
    VulkanWgpu,
    #[cfg(target_os = "macos")]
    Metal,
    OpenGL,
    OpenGLWgpu,
    Software,
    #[cfg(target_os = "windows")]
    DirectX,
}

// A handle to a backend-specific texture resource.
pub enum Texture {
    #[cfg(not(target_pointer_width = "32"))]
    Vulkan(vulkan::Texture),
    #[cfg(not(target_pointer_width = "32"))]
    VulkanWgpu(wgpu_core::Texture),
    #[cfg(target_os = "macos")]
    Metal(wgpu_core::Texture),
    OpenGL(opengl::Texture),
    OpenGLWgpu(wgpu_core::Texture),
    Software(software::Texture),
    #[cfg(target_os = "windows")]
    DirectX(wgpu_core::Texture),
}

// An internal enum to hold the state for the active rendering backend.
enum BackendImpl {
    #[cfg(not(target_pointer_width = "32"))]
    Vulkan(vulkan::State),
    #[cfg(not(target_pointer_width = "32"))]
    VulkanWgpu(wgpu_core::State),
    #[cfg(target_os = "macos")]
    Metal(wgpu_core::State),
    OpenGL(opengl::State),
    OpenGLWgpu(wgpu_core::State),
    Software(software::State),
    #[cfg(target_os = "windows")]
    DirectX(wgpu_core::State),
}

/// A public, opaque wrapper around the active rendering backend.
/// This hides platform-specific variants from the rest of the application.
pub struct Backend(BackendImpl);

/// Replace the RGB of fully-transparent texels that border opaque texels with
/// the average RGB of their opaque 8-neighborhood. This eliminates the gray /
/// colored "halo" that linear filtering produces when sampling across a
/// transparent edge whose RGB does not match the adjacent visible content
/// (a one-pixel transparent border with `(255,255,255,0)` is the canonical
/// example). Returns `Some(modified)` only when at least one texel needed
/// fixing, so the common case (no transparent edges) avoids any allocation.
fn bleed_transparent_rgb(image: &RgbaImage) -> Option<RgbaImage> {
    let (w, h) = image.dimensions();
    if w == 0 || h == 0 {
        return None;
    }
    let raw = image.as_raw();
    debug_assert_eq!(raw.len(), (w as usize) * (h as usize) * 4);

    // Quick scan: bail unless the image has both transparent and opaque texels.
    let mut has_transparent = false;
    let mut has_visible = false;
    for chunk in raw.chunks_exact(4) {
        if chunk[3] == 0 {
            has_transparent = true;
        } else {
            has_visible = true;
        }
        if has_transparent && has_visible {
            break;
        }
    }
    if !(has_transparent && has_visible) {
        return None;
    }

    let mut out = image.clone();
    let stride = (w as usize) * 4;
    let src = raw;
    let mut wrote = false;
    let dst: &mut [u8] = &mut out;
    for y in 0..h {
        for x in 0..w {
            let idx = (y as usize) * stride + (x as usize) * 4;
            if src[idx + 3] != 0 {
                continue;
            }
            let mut r = 0u32;
            let mut g = 0u32;
            let mut b = 0u32;
            let mut n = 0u32;
            let y0 = y.saturating_sub(1);
            let y1 = (y + 1).min(h - 1);
            let x0 = x.saturating_sub(1);
            let x1 = (x + 1).min(w - 1);
            for ny in y0..=y1 {
                for nx in x0..=x1 {
                    if nx == x && ny == y {
                        continue;
                    }
                    let nidx = (ny as usize) * stride + (nx as usize) * 4;
                    if src[nidx + 3] != 0 {
                        r += u32::from(src[nidx]);
                        g += u32::from(src[nidx + 1]);
                        b += u32::from(src[nidx + 2]);
                        n += 1;
                    }
                }
            }
            if n > 0 {
                dst[idx] = (r / n) as u8;
                dst[idx + 1] = (g / n) as u8;
                dst[idx + 2] = (b / n) as u8;
                // alpha intentionally left at 0
                wrote = true;
            }
        }
    }

    wrote.then_some(out)
}

#[cfg(test)]
mod bleed_tests {
    use super::bleed_transparent_rgb;
    use image::{Rgba, RgbaImage};

    #[test]
    fn no_op_when_no_transparent_pixels() {
        let img = RgbaImage::from_pixel(4, 4, Rgba([10, 20, 30, 255]));
        assert!(bleed_transparent_rgb(&img).is_none());
    }

    #[test]
    fn no_op_when_fully_transparent() {
        let img = RgbaImage::from_pixel(4, 4, Rgba([255, 255, 255, 0]));
        assert!(bleed_transparent_rgb(&img).is_none());
    }

    #[test]
    fn bleeds_neighbors_into_transparent_border() {
        let mut img = RgbaImage::from_pixel(3, 3, Rgba([255, 255, 255, 0]));
        img.put_pixel(1, 1, Rgba([200, 100, 50, 255]));
        let bled = bleed_transparent_rgb(&img).expect("should bleed");
        for y in 0..3 {
            for x in 0..3 {
                let p = bled.get_pixel(x, y).0;
                if x == 1 && y == 1 {
                    assert_eq!(p, [200, 100, 50, 255]);
                } else {
                    assert_eq!(p[0..3], [200, 100, 50], "pixel {x},{y}");
                    assert_eq!(p[3], 0, "alpha preserved at {x},{y}");
                }
            }
        }
    }

    #[test]
    fn leaves_isolated_transparent_pixels_alone() {
        let mut img = RgbaImage::from_pixel(5, 1, Rgba([255, 255, 255, 0]));
        img.put_pixel(0, 0, Rgba([10, 20, 30, 255]));
        let bled = bleed_transparent_rgb(&img).expect("should bleed");
        // pixel 1 borders pixel 0 -> bled to its RGB
        assert_eq!(bled.get_pixel(1, 0).0, [10, 20, 30, 0]);
        // pixel 2 has no opaque 8-neighbor -> unchanged
        assert_eq!(bled.get_pixel(2, 0).0, [255, 255, 255, 0]);
    }
}

impl Backend {
    pub fn draw(
        &mut self,
        render_list: &RenderList,
        textures: &TextureHandleMap<Texture>,
        apply_present_back_pressure: bool,
    ) -> Result<DrawStats, Box<dyn Error>> {
        match &mut self.0 {
            #[cfg(not(target_pointer_width = "32"))]
            BackendImpl::Vulkan(state) => {
                vulkan::draw(state, render_list, textures, apply_present_back_pressure)
            }
            #[cfg(not(target_pointer_width = "32"))]
            BackendImpl::VulkanWgpu(state) => {
                wgpu_core::draw(state, render_list, textures, apply_present_back_pressure)
            }
            #[cfg(target_os = "macos")]
            BackendImpl::Metal(state) => {
                wgpu_core::draw(state, render_list, textures, apply_present_back_pressure)
            }
            BackendImpl::OpenGL(state) => {
                opengl::draw(state, render_list, textures, apply_present_back_pressure)
            }
            BackendImpl::OpenGLWgpu(state) => {
                wgpu_core::draw(state, render_list, textures, apply_present_back_pressure)
            }
            BackendImpl::Software(state) => {
                software::draw(state, render_list, textures, apply_present_back_pressure)
            }
            #[cfg(target_os = "windows")]
            BackendImpl::DirectX(state) => {
                wgpu_core::draw(state, render_list, textures, apply_present_back_pressure)
            }
        }
    }

    pub fn request_screenshot(&mut self) {
        match &mut self.0 {
            #[cfg(not(target_pointer_width = "32"))]
            BackendImpl::Vulkan(state) => vulkan::request_screenshot(state),
            #[cfg(not(target_pointer_width = "32"))]
            BackendImpl::VulkanWgpu(state) => wgpu_core::request_screenshot(state),
            #[cfg(target_os = "macos")]
            BackendImpl::Metal(state) => wgpu_core::request_screenshot(state),
            BackendImpl::OpenGL(state) => opengl::request_screenshot(state),
            BackendImpl::OpenGLWgpu(state) => wgpu_core::request_screenshot(state),
            BackendImpl::Software(state) => software::request_screenshot(state),
            #[cfg(target_os = "windows")]
            BackendImpl::DirectX(state) => wgpu_core::request_screenshot(state),
        }
    }

    pub fn capture_frame(&mut self) -> Result<RgbaImage, Box<dyn Error>> {
        match &mut self.0 {
            BackendImpl::OpenGL(state) => opengl::capture_frame(state),
            #[cfg(not(target_pointer_width = "32"))]
            BackendImpl::Vulkan(state) => vulkan::capture_frame(state),
            #[cfg(not(target_pointer_width = "32"))]
            BackendImpl::VulkanWgpu(state) => wgpu_core::capture_frame(state),
            #[cfg(target_os = "macos")]
            BackendImpl::Metal(state) => wgpu_core::capture_frame(state),
            BackendImpl::OpenGLWgpu(state) => wgpu_core::capture_frame(state),
            BackendImpl::Software(_) => Err(std::io::Error::other(
                "Screenshot capture is not implemented for Software renderer yet",
            )
            .into()),
            #[cfg(target_os = "windows")]
            BackendImpl::DirectX(state) => wgpu_core::capture_frame(state),
        }
    }

    pub fn configure_software_threads(&mut self, threads: Option<usize>) {
        if let BackendImpl::Software(state) = &mut self.0 {
            software::set_thread_hint(state, threads);
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        match &mut self.0 {
            #[cfg(not(target_pointer_width = "32"))]
            BackendImpl::Vulkan(state) => vulkan::resize(state, width, height),
            #[cfg(not(target_pointer_width = "32"))]
            BackendImpl::VulkanWgpu(state) => wgpu_core::resize(state, width, height),
            #[cfg(target_os = "macos")]
            BackendImpl::Metal(state) => wgpu_core::resize(state, width, height),
            BackendImpl::OpenGL(state) => opengl::resize(state, width, height),
            BackendImpl::OpenGLWgpu(state) => wgpu_core::resize(state, width, height),
            BackendImpl::Software(state) => software::resize(state, width, height),
            #[cfg(target_os = "windows")]
            BackendImpl::DirectX(state) => wgpu_core::resize(state, width, height),
        }
    }

    pub fn cleanup(&mut self) {
        match &mut self.0 {
            #[cfg(not(target_pointer_width = "32"))]
            BackendImpl::Vulkan(state) => vulkan::cleanup(state),
            #[cfg(not(target_pointer_width = "32"))]
            BackendImpl::VulkanWgpu(state) => wgpu_core::cleanup(state),
            #[cfg(target_os = "macos")]
            BackendImpl::Metal(state) => wgpu_core::cleanup(state),
            BackendImpl::OpenGL(state) => opengl::cleanup(state),
            BackendImpl::OpenGLWgpu(state) => wgpu_core::cleanup(state),
            BackendImpl::Software(state) => software::cleanup(state),
            #[cfg(target_os = "windows")]
            BackendImpl::DirectX(state) => wgpu_core::cleanup(state),
        }
    }

    pub fn create_texture(
        &mut self,
        image: &RgbaImage,
        sampler: SamplerDesc,
    ) -> Result<Texture, Box<dyn Error>> {
        let bled = bleed_transparent_rgb(image);
        let image = bled.as_ref().unwrap_or(image);
        match &mut self.0 {
            #[cfg(not(target_pointer_width = "32"))]
            BackendImpl::Vulkan(state) => {
                let tex = vulkan::create_texture(state, image, sampler)?;
                Ok(Texture::Vulkan(tex))
            }
            #[cfg(not(target_pointer_width = "32"))]
            BackendImpl::VulkanWgpu(state) => {
                let tex = wgpu_core::create_texture(state, image, sampler)?;
                Ok(Texture::VulkanWgpu(tex))
            }
            #[cfg(target_os = "macos")]
            BackendImpl::Metal(state) => {
                let tex = wgpu_core::create_texture(state, image, sampler)?;
                Ok(Texture::Metal(tex))
            }
            BackendImpl::OpenGL(state) => {
                let tex = opengl::create_texture(&state.gl, image, sampler)?;
                Ok(Texture::OpenGL(tex))
            }
            BackendImpl::OpenGLWgpu(state) => {
                let tex = wgpu_core::create_texture(state, image, sampler)?;
                Ok(Texture::OpenGLWgpu(tex))
            }
            BackendImpl::Software(_state) => {
                let tex = software::create_texture(image, sampler)?;
                Ok(Texture::Software(tex))
            }
            #[cfg(target_os = "windows")]
            BackendImpl::DirectX(state) => {
                let tex = wgpu_core::create_texture(state, image, sampler)?;
                Ok(Texture::DirectX(tex))
            }
        }
    }

    pub fn update_texture(
        &mut self,
        texture: &mut Texture,
        image: &RgbaImage,
    ) -> Result<(), Box<dyn Error>> {
        let bled = bleed_transparent_rgb(image);
        let image = bled.as_ref().unwrap_or(image);
        match (&mut self.0, texture) {
            #[cfg(not(target_pointer_width = "32"))]
            (BackendImpl::Vulkan(state), Texture::Vulkan(texture)) => {
                vulkan::update_texture(state, texture, image)
            }
            #[cfg(not(target_pointer_width = "32"))]
            (BackendImpl::VulkanWgpu(state), Texture::VulkanWgpu(texture)) => {
                wgpu_core::update_texture(state, texture, image)
            }
            #[cfg(target_os = "macos")]
            (BackendImpl::Metal(state), Texture::Metal(texture)) => {
                wgpu_core::update_texture(state, texture, image)
            }
            (BackendImpl::OpenGL(state), Texture::OpenGL(texture)) => {
                opengl::update_texture(&state.gl, texture, image)?;
                Ok(())
            }
            (BackendImpl::OpenGLWgpu(state), Texture::OpenGLWgpu(texture)) => {
                wgpu_core::update_texture(state, texture, image)
            }
            (BackendImpl::Software(_state), Texture::Software(texture)) => {
                software::update_texture(texture, image)
            }
            #[cfg(target_os = "windows")]
            (BackendImpl::DirectX(state), Texture::DirectX(texture)) => {
                wgpu_core::update_texture(state, texture, image)
            }
            _ => Err(std::io::Error::other("texture/backend mismatch").into()),
        }
    }

    pub fn retire_textures(&mut self, textures: &mut TextureHandleMap<Texture>) {
        let old_textures = std::mem::take(textures);
        match &mut self.0 {
            #[cfg(not(target_pointer_width = "32"))]
            BackendImpl::Vulkan(state) => {
                let retired = old_textures
                    .into_values()
                    .filter_map(|texture| match texture {
                        Texture::Vulkan(texture) => Some(texture),
                        _ => None,
                    })
                    .collect();
                vulkan::retire_textures(state, retired);
            }
            #[cfg(not(target_pointer_width = "32"))]
            BackendImpl::VulkanWgpu(_) => {
                drop(old_textures);
            }
            #[cfg(target_os = "macos")]
            BackendImpl::Metal(_) => {
                drop(old_textures);
            }
            BackendImpl::OpenGL(state) => {
                // SAFETY: Each texture handle came from this backend and runtime
                // retirement only drops the GL object name; the driver defers
                // actual destruction until it is no longer in use.
                unsafe {
                    for tex in old_textures.values() {
                        if let Texture::OpenGL(opengl::Texture(handle)) = tex {
                            state.gl.delete_texture(*handle);
                        }
                    }
                }
            }
            BackendImpl::OpenGLWgpu(_) => {
                drop(old_textures);
            }
            BackendImpl::Software(_) => {
                drop(old_textures);
            }
            #[cfg(target_os = "windows")]
            BackendImpl::DirectX(_) => {
                drop(old_textures);
            }
        }
    }

    pub fn dispose_textures(&mut self, textures: &mut TextureHandleMap<Texture>) {
        self.wait_for_idle();

        let old_textures = std::mem::take(textures);
        match &mut self.0 {
            #[cfg(not(target_pointer_width = "32"))]
            BackendImpl::Vulkan(_) => {
                // Vulkan textures are cleaned up by their Drop implementation.
                drop(old_textures);
            }
            #[cfg(not(target_pointer_width = "32"))]
            BackendImpl::VulkanWgpu(_) => {
                drop(old_textures);
            }
            #[cfg(target_os = "macos")]
            BackendImpl::Metal(_) => {
                drop(old_textures);
            }
            BackendImpl::OpenGL(state) => {
                // SAFETY: `wait_for_idle()` above guarantees no in-flight GPU work
                // still references these texture handles, and each handle came from
                // this OpenGL backend.
                unsafe {
                    for tex in old_textures.values() {
                        if let Texture::OpenGL(opengl::Texture(handle)) = tex {
                            state.gl.delete_texture(*handle);
                        }
                    }
                }
            }
            BackendImpl::OpenGLWgpu(_) => {
                drop(old_textures);
            }
            BackendImpl::Software(_) => {
                drop(old_textures);
            }
            #[cfg(target_os = "windows")]
            BackendImpl::DirectX(_) => {
                drop(old_textures);
            }
        }
    }

    pub fn wait_for_idle(&mut self) {
        match &mut self.0 {
            #[cfg(not(target_pointer_width = "32"))]
            BackendImpl::Vulkan(state) => {
                let _ = vulkan::flush_pending_uploads(state);
                if let Some(device) = &state.device {
                    // SAFETY: `device` is the live Vulkan logical device for this
                    // backend, and we only wait for idle before tearing down or
                    // reclaiming resources.
                    unsafe {
                        let _ = device.device_wait_idle();
                    }
                }
                vulkan::retire_submitted_uploads(state);
                vulkan::retire_all_textures(state);
            }
            #[cfg(not(target_pointer_width = "32"))]
            BackendImpl::VulkanWgpu(state) => {
                let _ = state.device.poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: None,
                });
            }
            #[cfg(target_os = "macos")]
            BackendImpl::Metal(state) => {
                let _ = state.device.poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: None,
                });
            }
            BackendImpl::OpenGL(_) => {
                // This is a no-op for OpenGL.
            }
            BackendImpl::OpenGLWgpu(state) => {
                let _ = state.device.poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: None,
                });
            }
            BackendImpl::Software(_) => {
                // CPU renderer is synchronous; nothing to wait for.
            }
            #[cfg(target_os = "windows")]
            BackendImpl::DirectX(state) => {
                let _ = state.device.poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: None,
                });
            }
        }
    }
}

/// Creates and initializes a new graphics backend.
pub fn create_backend(
    backend_type: BackendType,
    window: Arc<Window>,
    vsync_enabled: bool,
    present_mode_policy: PresentModePolicy,
    gfx_debug_enabled: bool,
) -> Result<Backend, Box<dyn Error>> {
    let backend_impl = match backend_type {
        #[cfg(not(target_pointer_width = "32"))]
        BackendType::Vulkan => BackendImpl::Vulkan(vulkan::init(
            &window,
            vsync_enabled,
            present_mode_policy,
            gfx_debug_enabled,
        )?),
        #[cfg(not(target_pointer_width = "32"))]
        BackendType::VulkanWgpu => BackendImpl::VulkanWgpu(wgpu_core::init_vulkan(
            window,
            vsync_enabled,
            present_mode_policy,
            gfx_debug_enabled,
        )?),
        #[cfg(target_os = "macos")]
        BackendType::Metal => BackendImpl::Metal(wgpu_core::init_metal(
            window,
            vsync_enabled,
            present_mode_policy,
            gfx_debug_enabled,
        )?),
        BackendType::OpenGL => {
            BackendImpl::OpenGL(opengl::init(window, vsync_enabled, gfx_debug_enabled)?)
        }
        BackendType::OpenGLWgpu => BackendImpl::OpenGLWgpu(wgpu_core::init_opengl(
            window,
            vsync_enabled,
            present_mode_policy,
            gfx_debug_enabled,
        )?),
        BackendType::Software => BackendImpl::Software(software::init(window, vsync_enabled)?),
        #[cfg(target_os = "windows")]
        BackendType::DirectX => BackendImpl::DirectX(wgpu_core::init_dx12(
            window,
            vsync_enabled,
            present_mode_policy,
            gfx_debug_enabled,
        )?),
    };
    Ok(Backend(backend_impl))
}

impl Backend {
    pub fn set_present_config(
        &mut self,
        vsync_enabled: bool,
        present_mode_policy: PresentModePolicy,
    ) {
        match &mut self.0 {
            #[cfg(not(target_pointer_width = "32"))]
            BackendImpl::Vulkan(state) => {
                vulkan::set_present_config(state, vsync_enabled, present_mode_policy)
            }
            #[cfg(not(target_pointer_width = "32"))]
            BackendImpl::VulkanWgpu(state) => {
                wgpu_core::set_present_config(state, vsync_enabled, present_mode_policy)
            }
            #[cfg(target_os = "macos")]
            BackendImpl::Metal(state) => {
                wgpu_core::set_present_config(state, vsync_enabled, present_mode_policy)
            }
            BackendImpl::OpenGL(state) => opengl::set_vsync_enabled(state, vsync_enabled),
            BackendImpl::OpenGLWgpu(state) => {
                wgpu_core::set_present_config(state, vsync_enabled, present_mode_policy)
            }
            BackendImpl::Software(_) => {}
            #[cfg(target_os = "windows")]
            BackendImpl::DirectX(state) => {
                wgpu_core::set_present_config(state, vsync_enabled, present_mode_policy)
            }
        }
    }
}

// -- Boilerplate impls --
impl core::fmt::Display for BackendType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            #[cfg(not(target_pointer_width = "32"))]
            Self::Vulkan => write!(f, "Vulkan"),
            #[cfg(not(target_pointer_width = "32"))]
            Self::VulkanWgpu => write!(f, "Vulkan (wgpu)"),
            #[cfg(target_os = "macos")]
            Self::Metal => write!(f, "Metal (wgpu)"),
            Self::OpenGL => write!(f, "OpenGL"),
            Self::OpenGLWgpu => write!(f, "OpenGL (wgpu)"),
            Self::Software => write!(f, "Software"),
            #[cfg(target_os = "windows")]
            Self::DirectX => write!(f, "DirectX"),
        }
    }
}
impl FromStr for BackendType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            #[cfg(not(target_pointer_width = "32"))]
            "vulkan" => Ok(Self::Vulkan),
            #[cfg(not(target_pointer_width = "32"))]
            "vulkan-wgpu" | "vulkan_wgpu" | "wgpu-vulkan" | "vulkan (wgpu)" => Ok(Self::VulkanWgpu),
            #[cfg(target_os = "macos")]
            "metal" | "metal-wgpu" | "metal_wgpu" | "wgpu-metal" | "metal (wgpu)" => {
                Ok(Self::Metal)
            }
            "opengl" => Ok(Self::OpenGL),
            "opengl-wgpu" | "opengl_wgpu" | "wgpu-opengl" | "opengl (wgpu)" => Ok(Self::OpenGLWgpu),
            "software" | "cpu" => Ok(Self::Software),
            #[cfg(target_os = "windows")]
            "directx" | "dx12" | "directx (wgpu)" => Ok(Self::DirectX),
            _ => Err(format!("'{s}' is not a valid video renderer")),
        }
    }
}

impl PresentModePolicy {
    #[inline(always)]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mailbox => "mailbox",
            Self::Immediate => "immediate",
        }
    }
}

impl core::fmt::Display for PresentModePolicy {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for PresentModePolicy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "mailbox" | "balanced" => Ok(Self::Mailbox),
            "immediate" | "unhinged" => Ok(Self::Immediate),
            other => Err(format!("'{other}' is not a valid present mode policy")),
        }
    }
}
