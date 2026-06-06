//! Host-side placeholder for the menu render path, used only under
//! `feature = "hot"`.
//!
//! The real renderer lives in [`render.rs`](super) (`get_actors`). Under the
//! `hot` feature that file is **excluded from the engine rlib** (see
//! `menu/mod.rs`) so that editing it recompiles *only* the small
//! `deadsync-screens` cdylib — a subsecond relink — instead of dirtying the
//! whole engine rlib (~a minute). The host therefore has no compiled-in menu
//! renderer; it dispatches through the hot-loaded cdylib instead.
//!
//! This stub supplies the two symbols the engine still references
//! (`get_actors`, `clear_render_cache`) so the host links and runs. It is the
//! fallback shown **before the cdylib has loaded** and **after a quarantined
//! panic / rejected ABI**, where it renders nothing (the screen clear color)
//! until a valid `deadsync_screens` library is swapped in. Cache invalidation
//! is kept identical to the real path so state stays consistent across swaps.

use crate::engine::present::actors::Actor;
use crate::screens::menu::state::{HostContext, State};

/// Mirror of the real renderer's cache reset so screen transitions invalidate
/// the host-owned render caches identically regardless of which renderer is live.
pub fn clear_render_cache(state: &State) {
    *state.info_text_cache.borrow_mut() = None;
    *state.groovestats_text_cache.borrow_mut() = None;
    *state.arrowcloud_text_cache.borrow_mut() = None;
}

/// Placeholder renderer: emits no actors. The hot-loaded cdylib provides the
/// real `get_actors`; this is only reached before the first successful load or
/// while a panicking generation is quarantined.
pub fn get_actors(_state: &State, _ctx: &HostContext, _alpha: f32) -> Vec<Actor> {
    Vec::new()
}
