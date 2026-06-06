//! `deadsync-screens` — the reloadable hot unit for the menu.
//!
//! This crate compiles the **real** `screens/menu/render.rs` directly into a
//! `cdylib` via `#[path]`, so editing that file and rebuilding only this crate
//! produces a fresh `menu_get_actors` without recompiling the engine rlib. The
//! engine rlib is statically linked for every other type (`State`,
//! `HostContext`, `Actor`, components…). The cdylib exports exactly one symbol,
//! `deadsync_hot_entry`, returning a pointer to a `static HotHeader` whose
//! vtable the host validates and dispatches through (see `deadsync::hot`).
//!
//! NOTE (boundary safety): the included `render.rs` still produces some
//! `&'static str` font/texture keys (e.g. `font("miso")` and the component
//! builders' `TextureStatic*` keys) that, under static linking, point into
//! *this cdylib's* rodata and would dangle after an unload. That is the
//! documented deferred impurity; the boundary repoints (route keys through
//! host-owned `HostContext`) are required before old cdylibs are ever unloaded.
//! Today the runtime keeps all loaded libraries mapped, so the keys stay valid.

// The real render path, compiled into THIS crate so edits don't touch the rlib.
// Relative to `crates/deadsync-screens/src/`, the repo root is three levels up.
#[path = "../../../src/screens/menu/render.rs"]
mod render;

use deadsync::hot::{
    ABI_VERSION, BUILD_HASH, HotHeader, LAYOUT_HASH, MAGIC, PANIC_STRATEGY, ScreenVTable,
};

/// The dispatch table this cdylib publishes. `render::get_actors` is the freshly
/// compiled copy; its `fn` item coerces to the vtable's `extern "Rust"` pointer.
static VTABLE: ScreenVTable = ScreenVTable {
    menu_get_actors: render::get_actors,
};

/// `HotHeader` holds a raw `*const ()` vtable pointer, so it isn't `Sync`. The
/// header is immutable for the life of the dylib and only ever read, so wrapping
/// it to place it in a `static` is sound.
struct HeaderSync(HotHeader);
// SAFETY: `HEADER` is never mutated after construction; the pointer targets the
// `static VTABLE` in this same library, valid for the library's whole lifetime.
unsafe impl Sync for HeaderSync {}

static HEADER: HeaderSync = HeaderSync(HotHeader {
    magic: MAGIC,
    abi_version: ABI_VERSION,
    panic_strategy: PANIC_STRATEGY,
    _pad: [0; 3],
    size: size_of::<HotHeader>() as u32,
    layout_hash: LAYOUT_HASH,
    build_hash: BUILD_HASH,
    vtable: &VTABLE as *const ScreenVTable as *const (),
});

/// The single exported entry point. The host fetches this symbol, reads the
/// header, validates it against `deadsync::hot::EXPECTED`, and dispatches through
/// the vtable on success.
#[unsafe(no_mangle)]
pub extern "C" fn deadsync_hot_entry() -> *const HotHeader {
    &HEADER.0
}
