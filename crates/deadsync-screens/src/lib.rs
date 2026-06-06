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
//! NOTE (boundary safety): the render output now crosses as **`actor_wire`
//! bytes**, not a `Vec<Actor>`. The included `render.rs` still mints some
//! `&'static str` font/texture keys (e.g. `font("miso")` and the component
//! builders' `TextureStatic*` keys) that point into *this cdylib's* rodata, but
//! they are serialized as owned strings during encode and the host decodes them
//! into host-owned `Arc<str>`, so no cdylib-rodata pointer escapes inside a live
//! `Actor`. Such a key only exists within a single render call.
//!
//! SAFETY INVARIANT (thread-local scratch): [`SCRATCH`] is a cdylib-owned
//! `Vec<u8>` whose destructor and allocator live in *this* module. The runtime
//! must therefore **never unload a hot generation** (today it keeps every loaded
//! library mapped for the reloader's lifetime) — running this module's TLS
//! destructor after unload would be UB. The returned [`ActorBlob`] borrows this
//! buffer; the host decodes synchronously before the next hot call on the thread.

// The real render path, compiled into THIS crate so edits don't touch the rlib.
// Relative to `crates/deadsync-screens/src/`, the repo root is three levels up.
#[path = "../../../src/screens/menu/render.rs"]
mod render;

use deadsync::engine::present::actor_wire;
use deadsync::hot::{
    ABI_VERSION, ActorBlob, BUILD_HASH, HotHeader, LAYOUT_HASH, MAGIC, PANIC_STRATEGY, RENDER_OK,
    RENDER_PANIC, ScreenVTable,
};
use deadsync::screens::menu::state::{HostContext, State};
use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};

thread_local! {
    /// Cdylib-owned encode scratch reused across frames. Cleared and refilled on
    /// every render; the returned `ActorBlob` borrows it until the next call.
    static SCRATCH: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// `extern "C"` hot entry: render the menu, encode the actors into the
/// thread-local scratch, and return a borrowed [`ActorBlob`]. Panics from the
/// (hot-edited) render path are caught here and reported as [`RENDER_PANIC`]; no
/// panic crosses the `extern "C"` boundary (which would otherwise abort).
extern "C" fn menu_get_actors_blob(state: &State, ctx: &HostContext, alpha: f32) -> ActorBlob {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let actors = render::get_actors(state, ctx, alpha);
        SCRATCH.with(|scratch| {
            let mut buf = scratch.borrow_mut();
            buf.clear();
            actor_wire::encode_actors(&actors, &mut buf);
            (buf.as_ptr(), buf.len())
        })
        // `actors` (and its cdylib-owned `Arc`s) drop here, freed by this
        // module's own allocator — nothing heap-owned escapes to the host.
    }));
    match result {
        Ok((ptr, len)) => ActorBlob {
            status: RENDER_OK,
            ptr,
            len,
        },
        Err(_) => ActorBlob {
            status: RENDER_PANIC,
            ptr: std::ptr::null(),
            len: 0,
        },
    }
}

/// The dispatch table this cdylib publishes.
static VTABLE: ScreenVTable = ScreenVTable {
    menu_get_actors: menu_get_actors_blob,
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
