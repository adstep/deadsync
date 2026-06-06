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
//! 1. **Same toolchain / same engine rlib.** The reload payload is plain Rust
//!    (`Vec<Actor>` by value, `Arc<str>`, nested enums), passed over
//!    `extern "Rust"`. There is no C ABI here and forcing `#[repr(C)]` on the
//!    payload would be meaningless: both artifacts must be built by the **same
//!    rustc**, for the **same target**, with the **same panic strategy**, and
//!    must link **identical** layouts of every boundary type. [`BUILD_HASH`] +
//!    [`LAYOUT_HASH`] + [`HotHeader::panic_strategy`] encode that contract and a
//!    stale cdylib is rejected at load.
//!
//! 2. **Shared `std` / one allocator.** Host and cdylib are both built with
//!    `-C prefer-dynamic` so they share one `std-*.dll` → one global allocator.
//!    Only then is it sound for the host to drop a `Vec<Actor>` / `Arc<str>`
//!    that the cdylib allocated. Forgetting the flag yields two allocators and
//!    silent heap corruption — see the runtime docs.
//!
//! 3. **Nothing cdylib-owned may escape inside an `Actor`.** No reference into
//!    cdylib memory, no fn pointer into cdylib code, no trait object / closure
//!    whose vtable lives in the cdylib. In particular a `&'static str` baked
//!    from a **string literal in cdylib code** points into cdylib rodata and
//!    dangles after unload. Note: a `&'static str` *const defined in this rlib*
//!    is **not** a safe workaround, because static linking **duplicates** the
//!    rlib's rodata into the cdylib — referencing the const from cdylib code
//!    still bakes a cdylib-owned pointer. Font/texture keys must therefore be
//!    sourced **host-side** (a pointer into the exe's rodata) and handed in via
//!    `HostContext`; see [`font_keys`].
//!
//! Only [`HotHeader`] is read "blind" through a raw exported symbol, so it is
//! the one type that strictly requires `#[repr(C)]`. [`ScreenVTable`] is also
//! `#[repr(C)]` purely to freeze its field order (cheap insurance as it grows);
//! its function pointers stay `extern "Rust"`.

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

/// The reloadable dispatch table. One `extern "Rust"` fn pointer per hot entry
/// point. Render-only for the menu pilot — input / audio / navigation stay
/// host-owned and never run in the cdylib (see the plan's boundary shape).
///
/// `#[repr(C)]` freezes field order; the pointers themselves remain Rust-ABI.
#[repr(C)]
pub struct ScreenVTable {
    /// `menu::render::get_actors` — the pure layout transform that is hot-edited.
    pub menu_get_actors:
        extern "Rust" fn(&menu::State, &menu::HostContext, f32) -> Vec<Actor>,
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

    extern "Rust" fn noop_get_actors(
        _state: &menu::State,
        _ctx: &menu::HostContext,
        _alpha: f32,
    ) -> Vec<Actor> {
        Vec::new()
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
