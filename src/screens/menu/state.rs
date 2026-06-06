//! Host-owned state and boundary types for the title menu screen.
//!
//! `State` lives here (host-owned, survives a hot reload) together with the
//! small Copy status-key enums and the render-time cache. `HostContext` is the
//! boundary value the host resolves each frame and hands to the pure
//! `render::get_actors`.
//!
//! NOTE: `HostContext` is an in-process facade resolved by the host. It uses
//! ergonomic Rust types (`Arc<str>`, `Option<String>`, `&'static str`, Rust
//! `fn` pointers, a `&[(&str,&str)]` argument) and crosses the cdylib boundary
//! over `extern "Rust"` alongside the rest of the render payload, so both
//! artifacts must be built by the same toolchain with identical type layouts
//! (enforced by the `LAYOUT_HASH`/`BUILD_HASH` handshake).

use deadsync::screens::components::shared::visual_style_bg;
use deadsync::screens::input as screen_input;
use deadsync_online::arrowcloud::ConnectionError as ArrowCloudError;
use deadsync_online::groovestats::ConnectionError as GrooveStatsError;
use std::cell::{Cell, RefCell};
use std::sync::Arc;

/// Resolved status text + extra lines, cached on `State` keyed by a Copy status key.
#[derive(Clone)]
pub struct StatusTextCache<K, const N: usize> {
    pub key: K,
    pub main: Arc<str>,
    pub lines: [Option<Arc<str>>; N],
    pub line_count: usize,
}

/// Fully captures every input that affects the GrooveStats/BoogieStats status text.
/// Derived host-side from the network globals; the render path only formats from it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GrooveStatusKey {
    Pending {
        boogie: bool,
    },
    Error {
        boogie: bool,
        kind: GrooveStatsError,
    },
    Connected {
        boogie: bool,
        disabled_mask: u8,
    },
}

/// Fully captures every input that affects the ArrowCloud status text.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ArrowCloudStatusKey {
    Pending,
    Connected,
    Error(ArrowCloudError),
}

pub struct State {
    pub selected_index: usize,
    pub active_color_index: i32,
    pub rainbow_mode: bool,
    pub started_by_p2: bool,
    // The following fields are read/written by the (hot) render path, so they
    // must be `pub` for the future `deadsync-screens` cdylib to reach them.
    #[doc(hidden)]
    pub bg: visual_style_bg::State,
    #[doc(hidden)]
    pub i18n_revision: Cell<u64>,
    #[doc(hidden)]
    pub info_text_cache: RefCell<Option<(Option<String>, Arc<str>)>>,
    #[doc(hidden)]
    pub groovestats_text_cache: RefCell<Option<StatusTextCache<GrooveStatusKey, 3>>>,
    #[doc(hidden)]
    pub arrowcloud_text_cache: RefCell<Option<StatusTextCache<ArrowCloudStatusKey, 1>>>,
    // Input-path only (stays host-owned, never touched by the hot render unit).
    pub(crate) menu_lr_chord: screen_input::MenuLrChordTracker,
    pub(crate) menu_lr_undo: [i8; 2],
}

/// Everything the pure render path needs that would otherwise be a process-global
/// read. The host resolves this each frame (see `super::build_host_context`).
///
/// Value/callback hybrid: cheap scalars are passed by value; i18n is passed as a
/// callback so the ~25 `tr` sites stay editable in the render code.
pub struct HostContext {
    /// `i18n::tr` — resolve a localized string.
    pub tr: fn(&str, &str) -> Arc<str>,
    /// `i18n::tr_fmt` — resolve a localized format string with `{name}` args.
    pub tr_fmt: fn(&str, &str, &[(&str, &str)]) -> Arc<str>,
    /// `i18n::revision()` snapshot — used to invalidate the render caches.
    pub i18n_revision: u64,
    pub version: Arc<str>,
    pub banner_tag: Option<String>,
    pub song_count: usize,
    pub pack_count: usize,
    pub course_count: usize,
    pub groove_key: GrooveStatusKey,
    pub arrowcloud_key: ArrowCloudStatusKey,
    pub screen_center_x: f32,
    /// Shared UI background elapsed clock (`visual_style_bg::elapsed_seconds()`),
    /// resolved host-side so the hot render path animates off the host's ticked
    /// clock instead of the statically-linked cdylib's own frozen copy of the
    /// `GLOBAL_ELAPSED_BITS` process-global.
    pub bg_elapsed_s: f32,
    /// Lib-owned font key for the menu list (`current_machine_font_key`).
    pub menu_font: &'static str,
}
