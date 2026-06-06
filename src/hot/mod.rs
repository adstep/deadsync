//! Hot-reload boundary ABI — type definitions only.
//!
//! This module defines the handshake between the host (the `deadsync` exe, which
//! links the live engine rlib) and a reloadable `deadsync-screens` **cdylib**.
//! It deliberately contains **no loader logic** — polling, shadow copying,
//! `dlopen`, `catch_unwind`, and the keep-alive ring all live in the standalone
//! `deadsync-hot` runtime crate.
//!
//! # Boundary invariants (must hold for soundness)
//!
//! 1. **Same toolchain / same engine rlib layout.** The render *output* now
//!    crosses as an opaque **byte blob** ([`ActorBlob`] → `actor_wire` bytes), not
//!    a `Vec<Actor>`, so the payload no longer depends on `Actor`'s in-memory
//!    layout. But the host still hands the cdylib `&menu::State` and
//!    `&menu::HostContext` **by reference**, read by field layout, so those two
//!    types (and anything reachable by value through them) must still have
//!    **identical layout** in both artifacts. [`BUILD_HASH`] + [`LAYOUT_HASH`] +
//!    [`HotHeader::panic_strategy`] encode that contract and a stale cdylib is
//!    rejected at load.
//!
//! 2. **No shared allocator required (the Option-A payoff).** Nothing that owns
//!    heap crosses the boundary in either direction: the cdylib allocates and
//!    frees *its own* render `Arc`s and *its own* scratch byte buffer; the host
//!    allocates and frees *its own* decoded `Vec<Actor>`. Only POD (`ptr`/`len`/
//!    `u32` status) and read-only `&State`/`&HostContext` borrows cross, and the
//!    cdylib only ever *reads* the host's `Arc<str>` bytes (a pointer deref — no
//!    allocator op). Because of this the menu boundary no longer needs
//!    `-C prefer-dynamic`/one shared `std`, which is what re-enables `lto` and a
//!    release-mode hot loop. (The two artifacts must still be the same rustc; see
//!    invariant 1.)
//!
//! 3. **Nothing cdylib-owned may escape, and the cdylib must never drop/clone a
//!    host `Arc`.** Render output is copied out as bytes (keys, not handles), so
//!    no reference into cdylib memory survives. The inbound `&HostContext`
//!    exposes real `Arc<str>`, but the render path only reads them via `.as_ref()`
//!    and re-owns every string it emits with a cdylib-side `Arc::from(&str)` — it
//!    must never clone or drop a host `Arc` (that would free host heap with the
//!    cdylib allocator once the shared allocator is gone). Font/texture keys are
//!    likewise sourced host-side via `HostContext`; see [`font_keys`].
//!
//! Only [`HotHeader`] is read "blind" through a raw exported symbol, so it is
//! the one type that strictly requires `#[repr(C)]`. [`ScreenVTable`] and
//! [`ActorBlob`] are also `#[repr(C)]`: the vtable to freeze field order, the
//! blob because it crosses an `extern "C"` return by value.

#![allow(dead_code)] // Wired up by the cdylib and the runtime crate.

use crate::engine::present::actors::{Actor, SpriteSource, TextContent};
use crate::screens::menu;
use core::mem::offset_of;
use core::ptr::NonNull;

// The generic header + validation live in the standalone, app-agnostic
// `deadsync-hot` runtime crate. This rlib re-exports the core type as
// `HotHeader` (so the cdylib and tests keep naming `deadsync::hot::HotHeader`)
// and supplies the deadsync-specific [`ScreenVTable`], [`EXPECTED`] descriptor,
// and layout/build hashes the runtime validates against.
pub use deadsync_hot::HotHeaderCore as HotHeader;
pub use deadsync_hot::{Expected, HeaderRejection};

/// Sentinel identifying a deadsync hot-reload header. Bump only on a hard format
/// break of [`HotHeader`] itself (not on vtable/state changes — that's
/// [`LAYOUT_HASH`]).
pub const MAGIC: u64 = 0xDEAD_5719_C0DE_0001;

/// Bumped on any intentional change to the [`ScreenVTable`] shape/semantics.
pub const ABI_VERSION: u32 = 1;

/// Panic strategy of this build: `0` = unwind, `1` = abort. Host and cdylib must
/// match or `catch_unwind` across the boundary is unsound. The pilot runs the
/// dev profile (unwind) on both sides.
pub const PANIC_STRATEGY: u8 = if cfg!(panic = "abort") { 1 } else { 0 };

// --- FNV-1a (const) ---------------------------------------------------------

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

const fn fnv1a_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
        i += 1;
    }
    hash
}

const fn fnv1a_u64(hash: u64, value: u64) -> u64 {
    fnv1a_bytes(hash, &value.to_le_bytes())
}

/// Fold a type's `size_of` + `align_of` into the running hash.
const fn mix_layout<T>(hash: u64) -> u64 {
    fnv1a_u64(fnv1a_u64(hash, size_of::<T>() as u64), align_of::<T>() as u64)
}

/// Compile-time hash over the layout of every type that crosses the boundary.
///
/// Covers `size`/`align` of the header, the vtable, the per-screen `State`, the
/// `HostContext`, and the `Actor` payload tree (`Actor` / `SpriteSource` /
/// `TextContent`), plus the `#[repr(C)]` field offsets of [`HotHeader`] that the
/// loader reads blind. This is a **stale-artifact smoke detector**, not a full
/// structural ABI proof: it can miss an equal-size/equal-align field-type swap
/// inside `State`/`HostContext`. That is acceptable for the single-developer,
/// same-checkout pilot; a stronger structural hash is future work before the
/// runtime is trusted for unattended reloads.
pub const LAYOUT_HASH: u64 = {
    let mut h = FNV_OFFSET;
    h = mix_layout::<HotHeader>(h);
    h = mix_layout::<ScreenVTable>(h);
    h = mix_layout::<ActorBlob>(h);
    h = mix_layout::<menu::State>(h);
    h = mix_layout::<menu::HostContext>(h);
    h = mix_layout::<Actor>(h);
    h = mix_layout::<SpriteSource>(h);
    h = mix_layout::<TextContent>(h);
    // Pin the repr(C) field offsets the loader dereferences before it trusts
    // anything else in the header.
    h = fnv1a_u64(h, offset_of!(HotHeader, magic) as u64);
    h = fnv1a_u64(h, offset_of!(HotHeader, size) as u64);
    h = fnv1a_u64(h, offset_of!(HotHeader, layout_hash) as u64);
    h = fnv1a_u64(h, offset_of!(HotHeader, build_hash) as u64);
    h = fnv1a_u64(h, offset_of!(HotHeader, vtable) as u64);
    h
};

/// Compile-time hash over the toolchain identity: full `rustc -vV`, the git
/// short rev, the crate version, target arch/os, and the panic strategy.
///
/// `extern "Rust"` is **not** stable across rustc versions, so a toolchain swap
/// must invalidate a previously-built cdylib even if every layout is unchanged.
/// Both `DEADSYNC_RUSTC_VERSION` and `DEADSYNC_BUILD_HASH` are emitted by
/// `build.rs`.
pub const BUILD_HASH: u64 = {
    let mut h = FNV_OFFSET;
    h = fnv1a_bytes(h, env!("DEADSYNC_RUSTC_VERSION").as_bytes());
    h = fnv1a_bytes(h, env!("DEADSYNC_BUILD_HASH").as_bytes());
    h = fnv1a_bytes(h, env!("CARGO_PKG_VERSION").as_bytes());
    h = fnv1a_bytes(h, std::env::consts::ARCH.as_bytes());
    h = fnv1a_bytes(h, std::env::consts::OS.as_bytes());
    h = fnv1a_bytes(h, &[PANIC_STRATEGY]);
    h
};

// --- Boundary types ---------------------------------------------------------

/// [`ActorBlob::status`] value: render succeeded; `ptr`/`len` describe a valid
/// `actor_wire` byte buffer owned by the cdylib's thread-local scratch.
pub const RENDER_OK: u32 = 0;
/// [`ActorBlob::status`] value: the cdylib caught a panic inside the render call
/// (via its internal `catch_unwind`); `ptr` is null and `len` is 0. The host
/// quarantines the generation and falls back to the in-lib renderer.
pub const RENDER_PANIC: u32 = 1;

/// POD result of a hot render call, returned by value over the `extern "C"`
/// boundary. Carries no heap ownership: `ptr`/`len` borrow the cdylib's
/// thread-local encode scratch, valid only until the next hot call on that
/// thread. The host must copy (decode) the bytes synchronously before issuing
/// any further hot call and must never store `ptr`.
///
/// `status` is a raw `u32` (not an `enum`) on purpose: a corrupt/buggy cdylib
/// returning an out-of-range value is then a plain integer the host can reject,
/// not an invalid enum discriminant (which would be UB to even form).
#[repr(C)]
pub struct ActorBlob {
    pub status: u32,
    pub ptr: *const u8,
    pub len: usize,
}

/// Upper bound the host enforces on `ActorBlob::len` before forming a slice with
/// `slice::from_raw_parts`. The codec has its own internal element caps; this is
/// the coarse byte guard that keeps an absurd/corrupt `len` from being UB at the
/// slice boundary itself. 64 MiB is far above any realistic menu frame.
pub const MAX_BLOB_BYTES: usize = 64 << 20;

/// The reloadable dispatch table — one hot entry point per screen. A second hot
/// screen adds one field here (and one `mix_layout` over its `State`/
/// `HostContext` in [`LAYOUT_HASH`]); it does **not** need its own cdylib or its
/// own reloader. Render-only for now — input / audio / navigation stay
/// host-owned and never run in the cdylib.
///
/// `#[repr(C)]` freezes field order. The entry returns an [`ActorBlob`] (a POD
/// byte-buffer descriptor) over `extern "C"`, so no Rust heap value crosses by
/// value — see the module-level invariants.
#[repr(C)]
pub struct ScreenVTable {
    /// Renders the menu and encodes the actors into the cdylib's scratch buffer,
    /// returning a borrowed [`ActorBlob`]. The cdylib catches its own panics and
    /// reports them via [`RENDER_PANIC`]; a panic never crosses this boundary.
    pub menu_get_actors:
        extern "C" fn(&menu::State, &menu::HostContext, f32) -> ActorBlob,
}

/// What this host build expects of any cdylib it loads. Built from this rlib's
/// own consts; a cdylib compiled against a different engine state will disagree
/// on `layout_hash` / `build_hash` and be rejected by the runtime.
pub const EXPECTED: Expected = Expected {
    magic: MAGIC,
    abi_version: ABI_VERSION,
    size: size_of::<HotHeader>() as u32,
    layout_hash: LAYOUT_HASH,
    build_hash: BUILD_HASH,
    panic_strategy: PANIC_STRATEGY,
};

/// Reinterpret a validated opaque vtable pointer as a [`ScreenVTable`].
///
/// # Safety
/// `ptr` must come from a [`HotHeader`] that passed
/// [`HotHeaderCore::verify`](deadsync_hot::HotHeaderCore::verify) against
/// [`EXPECTED`] (so it points to a `ScreenVTable` of the agreed layout) and the
/// owning library must still be loaded.
pub unsafe fn screen_vtable<'a>(ptr: NonNull<()>) -> &'a ScreenVTable {
    unsafe { &*(ptr.as_ptr() as *const ScreenVTable) }
}

/// Font keys consumed by hot render paths.
///
/// See boundary invariant #3 in the module docs: render code must **not** name
/// these consts directly (that bakes a cdylib-owned pointer under static
/// linking). They are the single authoritative source the **host** reads to
/// populate `HostContext`, so the `&'static str` handed to the cdylib points
/// into the exe's rodata and never dangles.
pub mod font_keys {
    /// Default body font used by menu status/info text.
    pub const MISO: &str = "miso";
}

#[cfg(test)]
mod tests {
    use super::*;

    extern "C" fn noop_get_actors(
        _state: &menu::State,
        _ctx: &menu::HostContext,
        _alpha: f32,
    ) -> ActorBlob {
        ActorBlob {
            status: RENDER_OK,
            ptr: core::ptr::null(),
            len: 0,
        }
    }

    static TEST_VTABLE: ScreenVTable = ScreenVTable {
        menu_get_actors: noop_get_actors,
    };

    fn well_formed_header() -> HotHeader {
        HotHeader {
            magic: MAGIC,
            abi_version: ABI_VERSION,
            panic_strategy: PANIC_STRATEGY,
            _pad: [0; 3],
            size: size_of::<HotHeader>() as u32,
            layout_hash: LAYOUT_HASH,
            build_hash: BUILD_HASH,
            vtable: &TEST_VTABLE as *const ScreenVTable as *const (),
        }
    }

    #[test]
    fn expected_matches_header_size() {
        assert_eq!(EXPECTED.size as usize, size_of::<HotHeader>());
    }

    #[test]
    fn hashes_are_seeded() {
        // A zero hash would mean the const folding silently produced nothing.
        assert_ne!(LAYOUT_HASH, 0);
        assert_ne!(BUILD_HASH, 0);
        assert_ne!(LAYOUT_HASH, BUILD_HASH);
    }

    #[test]
    fn dev_profile_is_unwind() {
        // The pilot requires both sides on unwind; assert this build qualifies.
        assert_eq!(PANIC_STRATEGY, 0);
    }

    #[test]
    fn well_formed_header_verifies_and_dispatches() {
        let header = well_formed_header();
        assert_eq!(header.verify(&EXPECTED), Ok(()));
        // The opaque pointer round-trips back to the exact vtable.
        let vt = unsafe { screen_vtable(header.vtable_ptr().unwrap()) };
        assert!(std::ptr::eq(vt, &TEST_VTABLE));
    }

    #[test]
    fn each_mismatch_is_reported() {
        assert!(matches!(
            HotHeader { magic: MAGIC ^ 1, ..well_formed_header() }.verify(&EXPECTED),
            Err(HeaderRejection::Magic { .. })
        ));
        assert!(matches!(
            HotHeader { abi_version: ABI_VERSION + 1, ..well_formed_header() }.verify(&EXPECTED),
            Err(HeaderRejection::AbiVersion { .. })
        ));
        assert!(matches!(
            HotHeader { size: 0, ..well_formed_header() }.verify(&EXPECTED),
            Err(HeaderRejection::Size { .. })
        ));
        assert!(matches!(
            HotHeader { panic_strategy: PANIC_STRATEGY ^ 1, ..well_formed_header() }.verify(&EXPECTED),
            Err(HeaderRejection::PanicStrategy { .. })
        ));
        assert!(matches!(
            HotHeader { layout_hash: LAYOUT_HASH ^ 1, ..well_formed_header() }.verify(&EXPECTED),
            Err(HeaderRejection::LayoutHash { .. })
        ));
        assert!(matches!(
            HotHeader { build_hash: BUILD_HASH ^ 1, ..well_formed_header() }.verify(&EXPECTED),
            Err(HeaderRejection::BuildHash { .. })
        ));
        assert!(matches!(
            HotHeader { vtable: std::ptr::null(), ..well_formed_header() }.verify(&EXPECTED),
            Err(HeaderRejection::NullVtable)
        ));
    }
}
