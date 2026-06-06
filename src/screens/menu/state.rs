//! Host-owned state and boundary types for the title menu screen.
//!
//! `State` lives here (host-owned, survives a hot reload) together with the
//! small Copy status-key enums and the render-time cache. `HostContext` is the
//! boundary value the host resolves each frame and hands to the pure
//! `render::get_actors`.
//!
//! NOTE: `HostContext` is an in-process facade resolved by the host. Every
//! string it carries is **already resolved and cached host-side** before the
//! boundary is crossed — the render path only reads them as `&str` and builds
//! its own owned actors. It uses ergonomic Rust types (`Arc<str>`,
//! `&'static str`) and crosses the cdylib boundary over `extern "Rust"`
//! alongside the rest of the render payload, so both artifacts must be built by
//! the same toolchain with identical type layouts (enforced by the
//! `LAYOUT_HASH`/`BUILD_HASH` handshake).

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
/// read, **fully pre-resolved host-side** (see `super::build_host_context`).
///
/// Every string here is resolved and cached on the host before the boundary is
/// crossed; the render path only ever *reads* them as `&str` and builds its own
/// owned actors. Nothing here transfers ownership of host heap into the render
/// unit, and the render unit must never clone or drop one of these `Arc`s — it
/// reads `.as_ref()` and re-owns. That ownership purity is what lets the
/// boundary eventually run without a shared allocator.
pub struct HostContext {
    /// Pre-resolved menu info line (version + optional update tag + the
    /// song/pack/course summary), cached on `State`.
    pub info_text: Arc<str>,
    /// Pre-resolved menu option labels (Gameplay / Options / Exit).
    pub menu_labels: [Arc<str>; 3],
    /// Pre-resolved footer title (`Common/EventMode`).
    pub footer_title: Arc<str>,
    /// Pre-resolved footer side text (`Common/PressStart`), shown left and right.
    pub footer_side: Arc<str>,
    /// Pre-resolved GrooveStats/BoogieStats status block, cached on `State`.
    pub gs: StatusTextCache<GrooveStatusKey, 3>,
    /// Pre-resolved ArrowCloud status block, cached on `State`.
    pub ac: StatusTextCache<ArrowCloudStatusKey, 1>,
    pub screen_center_x: f32,
    /// Shared UI background elapsed clock (`visual_style_bg::elapsed_seconds()`),
    /// resolved host-side so the hot render path animates off the host's ticked
    /// clock instead of the statically-linked cdylib's own frozen copy of the
    /// `GLOBAL_ELAPSED_BITS` process-global.
    pub bg_elapsed_s: f32,
    /// Lib-owned font key for the menu list (`current_machine_font_key`).
    pub menu_font: &'static str,
}
