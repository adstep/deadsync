use crate::act;
use crate::assets::{self, AssetManager};
use crate::engine::display::{self, MonitorSpec};
use crate::engine::gfx::{BackendType, PresentModePolicy};
use crate::engine::space::{is_wide, screen_height, screen_width, widescale};
// Screen navigation is handled in app via the dispatcher
use crate::config::{
    self, BreakdownStyle, DefaultFailType, DisplayMode, FullscreenType, LogLevel,
    MachinePreferredPlayMode, MachinePreferredPlayStyle, NewPackMode, SelectMusicItlWheelMode,
    SelectMusicPatternInfoMode, SelectMusicScoreboxPlacement, SelectMusicWheelStyle, SimpleIni,
    SyncGraphMode, dirs,
};
use crate::engine::audio;
#[cfg(target_os = "windows")]
use crate::engine::input::WindowsPadBackend;
use crate::engine::input::{InputEvent, VirtualAction};
use crate::game::parsing::{noteskin as noteskin_parser, simfile as song_loading};
use crate::game::{course, profile, scores};
use crate::screens::input as screen_input;
use crate::screens::pack_sync as shared_pack_sync;
use crate::screens::select_music;
use crate::screens::{Screen, ScreenAction};
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::assets::i18n::{lookup_key, tr, tr_fmt};
use crate::engine::present::actors;
use crate::engine::present::actors::Actor;
use crate::engine::present::color;
use crate::engine::present::font;
use crate::screens::components::shared::screen_bar::{ScreenBarPosition, ScreenBarTitlePlacement};
use crate::screens::components::shared::{heart_bg, screen_bar};
use null_or_die::{BiasKernel, KernelTarget};

mod types;
mod menus;
mod sound;
mod choice_helpers;
pub(crate) use types::*;
pub(crate) use menus::*;
pub(crate) use sound::*;
pub(crate) use choice_helpers::*;

/* ---------------------------- transitions ---------------------------- */
const TRANSITION_IN_DURATION: f32 = 0.4;
const TRANSITION_OUT_DURATION: f32 = 0.4;
const RELOAD_BAR_H: f32 = 30.0;

/* -------------------------- hold-to-scroll timing ------------------------- */
const NAV_INITIAL_HOLD_DELAY: Duration = Duration::from_millis(300);
const NAV_REPEAT_SCROLL_INTERVAL: Duration = Duration::from_millis(50);

/* ----------------------------- cursor tweening ----------------------------- */
// Simply Love metrics.ini uses 0.1 for both [ScreenOptions] TweenSeconds and CursorTweenSeconds.
// ScreenOptionsService rows inherit OptionRow tween behavior, so keep both aligned at 0.1.
const SL_OPTION_ROW_TWEEN_SECONDS: f32 = 0.1;
const CURSOR_TWEEN_SECONDS: f32 = SL_OPTION_ROW_TWEEN_SECONDS;
const ROW_TWEEN_SECONDS: f32 = SL_OPTION_ROW_TWEEN_SECONDS;
// Spacing between inline items in OptionRows (pixels at current zoom)
const INLINE_SPACING: f32 = 15.75;

// Match Simply Love operator menu ranges (±1000 ms) for these calibrations.
const GLOBAL_OFFSET_MIN_MS: i32 = -1000;
const GLOBAL_OFFSET_MAX_MS: i32 = 1000;
const VISUAL_DELAY_MIN_MS: i32 = -1000;
const VISUAL_DELAY_MAX_MS: i32 = 1000;
const VOLUME_MIN_PERCENT: i32 = 0;
const VOLUME_MAX_PERCENT: i32 = 100;
const INPUT_DEBOUNCE_MIN_MS: i32 = 0;
const INPUT_DEBOUNCE_MAX_MS: i32 = 200;
const NULL_OR_DIE_POSITIVE_MS_MIN_TENTHS: i32 = 1;
const NULL_OR_DIE_POSITIVE_MS_MAX_TENTHS: i32 = 1000;
const NULL_OR_DIE_MAGIC_OFFSET_MIN_TENTHS: i32 = -1000;
const NULL_OR_DIE_MAGIC_OFFSET_MAX_TENTHS: i32 = 1000;

// --- Monitor & Video Mode Data Structures ---


#[inline(always)]
fn format_ms(value: i32) -> String {
    // Positive values omit a '+' and compact to the Simply Love "Nms" style.
    format!("{value}ms")
}

#[inline(always)]
fn format_percent(value: i32) -> String {
    format!("{value}%")
}

#[inline(always)]
fn format_tenths_ms(value_tenths: i32) -> String {
    format!("{:.1} ms", value_tenths as f64 / 10.0)
}

#[inline(always)]
fn adjust_ms_value(value: &mut i32, delta: isize, min: i32, max: i32) -> bool {
    let new_value = (*value + delta as i32).clamp(min, max);
    if new_value == *value {
        false
    } else {
        *value = new_value;
        true
    }
}

#[inline(always)]
fn adjust_tenths_value(value: &mut i32, delta: isize, min: i32, max: i32) -> bool {
    let new_value = (*value + delta as i32).clamp(min, max);
    if new_value == *value {
        false
    } else {
        *value = new_value;
        true
    }
}

#[inline(always)]
fn tenths_from_f64(value: f64) -> i32 {
    let scaled = value * 10.0;
    let nudge = scaled.signum() * scaled.abs().max(1.0) * f64::EPSILON * 16.0;
    (scaled + nudge).round() as i32
}

#[inline(always)]
fn f64_from_tenths(value: i32) -> f64 {
    value as f64 / 10.0
}

// Keyboard input is handled centrally via the virtual dispatcher in app

/// Bars in `screen_bar.rs` use 32.0 px height.
const BAR_H: f32 = 32.0;

/// Screen-space margins (pixels, not scaled)
const LEFT_MARGIN_PX: f32 = 33.0;
const RIGHT_MARGIN_PX: f32 = 25.0;
const FIRST_ROW_TOP_MARGIN_PX: f32 = 18.0;
const BOTTOM_MARGIN_PX: f32 = 0.0;

/// Unscaled spec constants (we’ll uniformly scale).
const VISIBLE_ROWS: usize = 10; // how many rows are shown at once
// Match player_options.rs row height.
const ROW_H: f32 = 33.0;
const ROW_GAP: f32 = 2.5;
const SEP_W: f32 = 2.5; // gap/stripe between rows and description
// Match SL non-wide/wide block sizing used by ScreenPlayerOptions underlay.
const OPTIONS_BLOCK_W_43: f32 = 614.0;
const OPTIONS_BLOCK_W_169: f32 = 792.0;
const DESC_W_43: f32 = 287.0; // ScreenOptionsService overlay.lua: WideScale(287,292)
const DESC_W_169: f32 = 292.0;
// derive description height from visible rows so it never includes a trailing gap
const DESC_H: f32 = (VISIBLE_ROWS as f32) * ROW_H + ((VISIBLE_ROWS - 1) as f32) * ROW_GAP;

#[inline(always)]
fn desc_w_unscaled() -> f32 {
    widescale(DESC_W_43, DESC_W_169)
}

#[inline(always)]
fn list_w_unscaled() -> f32 {
    widescale(
        OPTIONS_BLOCK_W_43 - SEP_W - DESC_W_43,
        OPTIONS_BLOCK_W_169 - SEP_W - DESC_W_169,
    )
}

/// Left margin for row labels (in content-space pixels).
const TEXT_LEFT_PAD: f32 = 40.66;
/// Left margin for the heart icon (in content-space pixels).
const HEART_LEFT_PAD: f32 = 13.0;
/// Label text zoom, matched to the left column titles in `player_options.rs`.
const ITEM_TEXT_ZOOM: f32 = 0.88;
/// Width of the System Options submenu label column (content-space pixels).
const SUB_LABEL_COL_W: f32 = 142.5;
/// Left padding for text inside the System Options submenu label column.
const SUB_LABEL_TEXT_LEFT_PAD: f32 = 11.0;
/// Left padding for inline option values in the System Options submenu (content-space pixels).
const SUB_INLINE_ITEMS_LEFT_PAD: f32 = 13.0;
/// Horizontal offset (content-space pixels) for single-value submenu items
/// (e.g. Language and Exit) within the items column.
const SUB_SINGLE_VALUE_CENTER_OFFSET: f32 = -43.0;

/// Heart sprite zoom for the options list rows.
/// This is a StepMania-style "zoom" factor applied to the native heart.png size.
const HEART_ZOOM: f32 = 0.026;


/// Description pane layout (mirrors Simply Love's `ScreenOptionsService` overlay).
/// Title and bullet list use separate top/side padding so they can be tuned independently.
const DESC_TITLE_TOP_PAD_PX: f32 = 9.75; // padding from box top to title
const DESC_TITLE_SIDE_PAD_PX: f32 = 7.5; // left/right padding for title text
const DESC_BULLET_TOP_PAD_PX: f32 = 23.25; // vertical gap between title and bullet list
const DESC_BULLET_SIDE_PAD_PX: f32 = 7.5; // left/right padding for bullet text
const DESC_BULLET_INDENT_PX: f32 = 10.0; // extra indent for bullet marker + text
const DESC_TITLE_ZOOM: f32 = 1.0; // title text zoom (roughly header-sized)
const DESC_BODY_ZOOM: f32 = 1.0; // body/bullet text zoom (similar to help text)

#[inline(always)]
fn desc_wrap_extra_pad_unscaled() -> f32 {
    // Slightly tighter wrap in 4:3 to avoid edge clipping from font metric/render mismatch.
    widescale(6.0, 0.0)
}

#[inline(always)]
fn submenu_inline_widths_fit(widths: &[f32]) -> bool {
    if widths.is_empty() {
        return false;
    }
    if is_wide() {
        return true;
    }
    let total_w = widths.iter().copied().sum::<f32>()
        + INLINE_SPACING * (widths.len().saturating_sub(1) as f32);
    let item_col_w = (list_w_unscaled() - SUB_LABEL_COL_W).max(0.0);
    let inline_w = (item_col_w - SUB_INLINE_ITEMS_LEFT_PAD).max(0.0);
    total_w <= inline_w
}

const SUBMENU_FADE_DURATION: f32 = 0.2;

fn build_submenu_row_layout(
    state: &State,
    asset_manager: &AssetManager,
    kind: SubmenuKind,
    row_idx: usize,
) -> Option<SubmenuRowLayout> {
    let rows = submenu_rows(kind);
    let row = rows.get(row_idx)?;
    let choice_texts = submenu_display_choice_texts(state, kind, rows, row_idx);
    if choice_texts.is_empty() {
        return None;
    }
    let value_zoom = 0.835_f32;
    let texts: Vec<Arc<str>> = choice_texts
        .iter()
        .map(|text| Arc::<str>::from(text.as_ref()))
        .collect();
    let mut widths: Vec<f32> = Vec::with_capacity(choice_texts.len());
    let mut text_h = 16.0_f32;
    asset_manager.with_fonts(|all_fonts| {
        asset_manager.with_font("miso", |metrics_font| {
            text_h = (metrics_font.height as f32).max(1.0) * value_zoom;
            for text in &texts {
                let mut w =
                    font::measure_line_width_logical(metrics_font, text.as_ref(), all_fonts) as f32;
                if !w.is_finite() || w <= 0.0 {
                    w = 1.0;
                }
                widths.push(w * value_zoom);
            }
        });
    });
    if widths.len() != texts.len() {
        widths.clear();
        widths.extend(
            texts
                .iter()
                .map(|text| (text.chars().count().max(1) as f32) * 8.0 * value_zoom),
        );
    }
    let inline_row = row.inline && submenu_inline_widths_fit(&widths);
    let mut x_positions: Vec<f32> = Vec::new();
    let mut centers: Vec<f32> = Vec::new();
    if inline_row {
        x_positions = Vec::with_capacity(widths.len());
        centers = Vec::with_capacity(widths.len());
        let mut x = 0.0_f32;
        for &draw_w in &widths {
            x_positions.push(x);
            centers.push(draw_w.mul_add(0.5, x));
            x += draw_w + INLINE_SPACING;
        }
    }
    Some(SubmenuRowLayout {
        texts: Arc::from(texts),
        widths: Arc::from(widths),
        x_positions: Arc::from(x_positions),
        centers: Arc::from(centers),
        text_h,
        inline_row,
    })
}

fn submenu_row_layout(
    state: &State,
    asset_manager: &AssetManager,
    kind: SubmenuKind,
    row_idx: usize,
) -> Option<SubmenuRowLayout> {
    let rows = submenu_rows(kind);
    let mut cache = state.submenu_row_layout_cache.borrow_mut();
    if state.submenu_layout_cache_kind.get() != Some(kind) || cache.len() != rows.len() {
        state.submenu_layout_cache_kind.set(Some(kind));
        cache.clear();
        cache.resize(rows.len(), None);
    }
    if let Some(layout) = cache.get(row_idx).cloned().flatten() {
        return Some(layout);
    }
    let layout = build_submenu_row_layout(state, asset_manager, kind, row_idx)?;
    if row_idx < cache.len() {
        cache[row_idx] = Some(layout.clone());
    }
    Some(layout)
}

pub fn clear_submenu_row_layout_cache(state: &State) {
    state.submenu_layout_cache_kind.set(None);
    let mut cache = state.submenu_row_layout_cache.borrow_mut();
    cache.clear();
}

fn sync_submenu_inline_x_from_row(
    state: &mut State,
    asset_manager: &AssetManager,
    kind: SubmenuKind,
    visible_row_idx: usize,
) {
    let Some(row_idx) = submenu_visible_row_to_actual(state, kind, visible_row_idx) else {
        return;
    };
    let Some(layout) = submenu_row_layout(state, asset_manager, kind, row_idx) else {
        return;
    };
    if !layout.inline_row || layout.centers.is_empty() {
        return;
    }
    let choice_idx = submenu_choice_indices(state, kind)
        .get(row_idx)
        .copied()
        .unwrap_or(0)
        .min(layout.centers.len().saturating_sub(1));
    state.sub_inline_x = layout.centers[choice_idx];
}

fn apply_submenu_inline_x_to_row(
    state: &mut State,
    asset_manager: &AssetManager,
    kind: SubmenuKind,
    visible_row_idx: usize,
) {
    let Some(row_idx) = submenu_visible_row_to_actual(state, kind, visible_row_idx) else {
        return;
    };
    let Some(layout) = submenu_row_layout(state, asset_manager, kind, row_idx) else {
        return;
    };
    if !layout.inline_row || layout.centers.is_empty() {
        return;
    }
    let choice_idx = submenu_choice_indices(state, kind)
        .get(row_idx)
        .copied()
        .unwrap_or(0)
        .min(layout.centers.len().saturating_sub(1));
    if let Some(slot) = submenu_cursor_indices_mut(state, kind).get_mut(row_idx) {
        *slot = choice_idx;
    }
    state.sub_inline_x = layout.centers[choice_idx];
}

fn move_submenu_selection_vertical(
    state: &mut State,
    asset_manager: &AssetManager,
    kind: SubmenuKind,
    dir: NavDirection,
) {
    let total = submenu_total_rows(state, kind);
    if total == 0 {
        return;
    }
    let current_row = state.sub_selected.min(total.saturating_sub(1));
    if !state.sub_inline_x.is_finite() {
        sync_submenu_inline_x_from_row(state, asset_manager, kind, current_row);
    }
    state.sub_selected = match dir {
        NavDirection::Up => {
            if current_row == 0 {
                total - 1
            } else {
                current_row - 1
            }
        }
        NavDirection::Down => (current_row + 1) % total,
    };
    apply_submenu_inline_x_to_row(state, asset_manager, kind, state.sub_selected);
}

const SOUND_VOLUME_LEVELS: [u8; 6] = [0, 10, 25, 50, 75, 100];

fn set_choice_by_id(choice_indices: &mut Vec<usize>, rows: &[SubRow], id: SubRowId, idx: usize) {
    if let Some(pos) = rows.iter().position(|r| r.id == id)
        && let Some(slot) = choice_indices.get_mut(pos)
    {
        let max_idx = rows[pos].choices.len().saturating_sub(1);
        *slot = idx.min(max_idx);
    }
}



pub struct State {
    pub selected: usize,
    prev_selected: usize,
    pub active_color_index: i32, // <-- ADDED
    bg: heart_bg::State,
    nav_key_held_direction: Option<NavDirection>,
    nav_key_held_since: Option<Instant>,
    nav_key_last_scrolled_at: Option<Instant>,
    nav_lr_held_direction: Option<isize>,
    nav_lr_held_since: Option<Instant>,
    nav_lr_last_adjusted_at: Option<Instant>,
    view: OptionsView,
    submenu_transition: SubmenuTransition,
    pending_submenu_kind: Option<SubmenuKind>,
    pending_submenu_parent_kind: Option<SubmenuKind>,
    submenu_parent_kind: Option<SubmenuKind>,
    submenu_fade_t: f32,
    content_alpha: f32,
    reload_ui: Option<ReloadUiState>,
    score_import_ui: Option<ScoreImportUiState>,
    pack_sync_overlay: shared_pack_sync::OverlayState,
    score_import_confirm: Option<ScoreImportConfirmState>,
    sync_pack_confirm: Option<SyncPackConfirmState>,
    menu_lr_chord: screen_input::MenuLrChordTracker,
    menu_lr_undo: i8,
    pending_dedicated_menu_buttons: Option<bool>,
    // Submenu state
    sub_selected: usize,
    sub_prev_selected: usize,
    sub_inline_x: f32,
    sub_choice_indices_system: Vec<usize>,
    sub_choice_indices_graphics: Vec<usize>,
    sub_choice_indices_input: Vec<usize>,
    sub_choice_indices_input_backend: Vec<usize>,
    sub_choice_indices_online_scoring: Vec<usize>,
    sub_choice_indices_null_or_die: Vec<usize>,
    sub_choice_indices_null_or_die_options: Vec<usize>,
    sub_choice_indices_sync_packs: Vec<usize>,
    sub_choice_indices_machine: Vec<usize>,
    sub_choice_indices_advanced: Vec<usize>,
    sub_choice_indices_course: Vec<usize>,
    sub_choice_indices_gameplay: Vec<usize>,
    sub_choice_indices_sound: Vec<usize>,
    sub_choice_indices_select_music: Vec<usize>,
    sub_choice_indices_groovestats: Vec<usize>,
    sub_choice_indices_arrowcloud: Vec<usize>,
    sub_choice_indices_score_import: Vec<usize>,
    system_noteskin_choices: Vec<String>,
    sub_cursor_indices_system: Vec<usize>,
    sub_cursor_indices_graphics: Vec<usize>,
    sub_cursor_indices_input: Vec<usize>,
    sub_cursor_indices_input_backend: Vec<usize>,
    sub_cursor_indices_online_scoring: Vec<usize>,
    sub_cursor_indices_null_or_die: Vec<usize>,
    sub_cursor_indices_null_or_die_options: Vec<usize>,
    sub_cursor_indices_sync_packs: Vec<usize>,
    sub_cursor_indices_machine: Vec<usize>,
    sub_cursor_indices_advanced: Vec<usize>,
    sub_cursor_indices_course: Vec<usize>,
    sub_cursor_indices_gameplay: Vec<usize>,
    sub_cursor_indices_sound: Vec<usize>,
    sub_cursor_indices_select_music: Vec<usize>,
    sub_cursor_indices_groovestats: Vec<usize>,
    sub_cursor_indices_arrowcloud: Vec<usize>,
    sub_cursor_indices_score_import: Vec<usize>,
    score_import_profiles: Vec<ScoreImportProfileConfig>,
    score_import_profile_choices: Vec<String>,
    score_import_profile_ids: Vec<Option<String>>,
    score_import_pack_choices: Vec<String>,
    score_import_pack_filters: Vec<Option<String>>,
    sync_pack_choices: Vec<String>,
    sync_pack_filters: Vec<Option<String>>,
    sound_device_options: Vec<SoundDeviceOption>,
    #[cfg(target_os = "linux")]
    linux_backend_choices: Vec<String>,
    master_volume_pct: i32,
    sfx_volume_pct: i32,
    assist_tick_volume_pct: i32,
    music_volume_pct: i32,
    global_offset_ms: i32,
    visual_delay_ms: i32,
    input_debounce_ms: i32,
    null_or_die_fingerprint_tenths: i32,
    null_or_die_window_tenths: i32,
    null_or_die_step_tenths: i32,
    null_or_die_magic_offset_tenths: i32,
    video_renderer_at_load: BackendType,
    display_mode_at_load: DisplayMode,
    display_monitor_at_load: usize,
    display_width_at_load: u32,
    display_height_at_load: u32,
    max_fps_at_load: u16,
    vsync_at_load: bool,
    present_mode_policy_at_load: PresentModePolicy,
    display_mode_choices: Vec<String>,
    software_thread_choices: Vec<u8>,
    software_thread_labels: Vec<String>,
    max_fps_choices: Vec<u16>,
    max_fps_labels: Vec<String>,
    resolution_choices: Vec<(u32, u32)>,
    refresh_rate_choices: Vec<u32>, // New: stored in millihertz
    // Hardware info
    pub monitor_specs: Vec<MonitorSpec>,
    // Cursor ring tween (StopTweening/BeginTweening parity with ITGmania ScreenOptions::TweenCursor).
    cursor_initialized: bool,
    cursor_from_x: f32,
    cursor_from_y: f32,
    cursor_from_w: f32,
    cursor_from_h: f32,
    cursor_to_x: f32,
    cursor_to_y: f32,
    cursor_to_w: f32,
    cursor_to_h: f32,
    cursor_t: f32,
    // Shared row tween state for the active view (main list or submenu list).
    row_tweens: Vec<RowTween>,
    submenu_layout_cache_kind: Cell<Option<SubmenuKind>>,
    submenu_row_layout_cache: RefCell<Vec<Option<SubmenuRowLayout>>>,
    description_layout_cache: RefCell<Option<DescriptionLayout>>,
    graphics_prev_visible_rows: Vec<usize>,
    advanced_prev_visible_rows: Vec<usize>,
    select_music_prev_visible_rows: Vec<usize>,
}

pub fn init() -> State {
    let cfg = config::get();
    let system_noteskin_choices = discover_system_noteskin_choices();
    let software_thread_choices = build_software_thread_choices();
    let software_thread_labels = software_thread_choice_labels(&software_thread_choices);
    let max_fps_choices = build_max_fps_choices();
    let max_fps_labels = max_fps_choice_labels(&max_fps_choices);
    let sound_device_options = build_sound_device_options();
    #[cfg(target_os = "linux")]
    let linux_backend_choices = build_linux_backend_choices();
    let machine_noteskin = profile::machine_default_noteskin();
    let machine_noteskin_idx = system_noteskin_choices
        .iter()
        .position(|name| name.eq_ignore_ascii_case(machine_noteskin.as_str()))
        .unwrap_or(0);
    let mut state = State {
        selected: 0,
        prev_selected: 0,
        active_color_index: color::DEFAULT_COLOR_INDEX, // <-- ADDED
        bg: heart_bg::State::new(),

        nav_key_held_direction: None,
        nav_key_held_since: None,
        nav_key_last_scrolled_at: None,
        nav_lr_held_direction: None,
        nav_lr_held_since: None,
        nav_lr_last_adjusted_at: None,
        submenu_transition: SubmenuTransition::None,
        pending_submenu_kind: None,
        pending_submenu_parent_kind: None,
        submenu_parent_kind: None,
        submenu_fade_t: 0.0,
        content_alpha: 1.0,
        reload_ui: None,
        score_import_ui: None,
        pack_sync_overlay: shared_pack_sync::OverlayState::Hidden,
        score_import_confirm: None,
        sync_pack_confirm: None,
        menu_lr_chord: screen_input::MenuLrChordTracker::default(),
        menu_lr_undo: 0,
        pending_dedicated_menu_buttons: None,
        view: OptionsView::Main,
        sub_selected: 0,
        sub_prev_selected: 0,
        sub_inline_x: f32::NAN,
        sub_choice_indices_system: vec![0; SYSTEM_OPTIONS_ROWS.len()],
        sub_choice_indices_graphics: vec![0; GRAPHICS_OPTIONS_ROWS.len()],
        sub_choice_indices_input: vec![0; INPUT_OPTIONS_ROWS.len()],
        sub_choice_indices_input_backend: vec![0; INPUT_BACKEND_OPTIONS_ROWS.len()],
        sub_choice_indices_online_scoring: vec![0; ONLINE_SCORING_OPTIONS_ROWS.len()],
        sub_choice_indices_null_or_die: vec![0; NULL_OR_DIE_MENU_ROWS.len()],
        sub_choice_indices_null_or_die_options: vec![0; NULL_OR_DIE_OPTIONS_ROWS.len()],
        sub_choice_indices_sync_packs: vec![0; SYNC_PACK_OPTIONS_ROWS.len()],
        sub_choice_indices_machine: vec![0; MACHINE_OPTIONS_ROWS.len()],
        sub_choice_indices_advanced: vec![0; ADVANCED_OPTIONS_ROWS.len()],
        sub_choice_indices_course: vec![0; COURSE_OPTIONS_ROWS.len()],
        sub_choice_indices_gameplay: vec![0; GAMEPLAY_OPTIONS_ROWS.len()],
        sub_choice_indices_sound: vec![0; SOUND_OPTIONS_ROWS.len()],
        sub_choice_indices_select_music: vec![0; SELECT_MUSIC_OPTIONS_ROWS.len()],
        sub_choice_indices_groovestats: vec![0; GROOVESTATS_OPTIONS_ROWS.len()],
        sub_choice_indices_arrowcloud: vec![0; ARROWCLOUD_OPTIONS_ROWS.len()],
        sub_choice_indices_score_import: vec![0; SCORE_IMPORT_OPTIONS_ROWS.len()],
        system_noteskin_choices,
        sub_cursor_indices_system: vec![0; SYSTEM_OPTIONS_ROWS.len()],
        sub_cursor_indices_graphics: vec![0; GRAPHICS_OPTIONS_ROWS.len()],
        sub_cursor_indices_input: vec![0; INPUT_OPTIONS_ROWS.len()],
        sub_cursor_indices_input_backend: vec![0; INPUT_BACKEND_OPTIONS_ROWS.len()],
        sub_cursor_indices_online_scoring: vec![0; ONLINE_SCORING_OPTIONS_ROWS.len()],
        sub_cursor_indices_null_or_die: vec![0; NULL_OR_DIE_MENU_ROWS.len()],
        sub_cursor_indices_null_or_die_options: vec![0; NULL_OR_DIE_OPTIONS_ROWS.len()],
        sub_cursor_indices_sync_packs: vec![0; SYNC_PACK_OPTIONS_ROWS.len()],
        sub_cursor_indices_machine: vec![0; MACHINE_OPTIONS_ROWS.len()],
        sub_cursor_indices_advanced: vec![0; ADVANCED_OPTIONS_ROWS.len()],
        sub_cursor_indices_course: vec![0; COURSE_OPTIONS_ROWS.len()],
        sub_cursor_indices_gameplay: vec![0; GAMEPLAY_OPTIONS_ROWS.len()],
        sub_cursor_indices_sound: vec![0; SOUND_OPTIONS_ROWS.len()],
        sub_cursor_indices_select_music: vec![0; SELECT_MUSIC_OPTIONS_ROWS.len()],
        sub_cursor_indices_groovestats: vec![0; GROOVESTATS_OPTIONS_ROWS.len()],
        sub_cursor_indices_arrowcloud: vec![0; ARROWCLOUD_OPTIONS_ROWS.len()],
        sub_cursor_indices_score_import: vec![0; SCORE_IMPORT_OPTIONS_ROWS.len()],
        score_import_profiles: Vec::new(),
        score_import_profile_choices: vec![
            tr("OptionsScoreImport", "NoEligibleProfiles").to_string(),
        ],
        score_import_profile_ids: vec![None],
        score_import_pack_choices: vec![tr("OptionsScoreImport", "AllPacks").to_string()],
        score_import_pack_filters: vec![None],
        sync_pack_choices: vec![tr("OptionsSyncPack", "AllPacks").to_string()],
        sync_pack_filters: vec![None],
        sound_device_options,
        #[cfg(target_os = "linux")]
        linux_backend_choices,
        master_volume_pct: i32::from(cfg.master_volume.clamp(0, 100)),
        sfx_volume_pct: i32::from(cfg.sfx_volume.clamp(0, 100)),
        assist_tick_volume_pct: i32::from(cfg.assist_tick_volume.clamp(0, 100)),
        music_volume_pct: i32::from(cfg.music_volume.clamp(0, 100)),
        global_offset_ms: {
            let ms = (cfg.global_offset_seconds * 1000.0).round() as i32;
            ms.clamp(GLOBAL_OFFSET_MIN_MS, GLOBAL_OFFSET_MAX_MS)
        },
        visual_delay_ms: {
            let ms = (cfg.visual_delay_seconds * 1000.0).round() as i32;
            ms.clamp(VISUAL_DELAY_MIN_MS, VISUAL_DELAY_MAX_MS)
        },
        input_debounce_ms: {
            let ms = (cfg.input_debounce_seconds * 1000.0).round() as i32;
            ms.clamp(INPUT_DEBOUNCE_MIN_MS, INPUT_DEBOUNCE_MAX_MS)
        },
        null_or_die_fingerprint_tenths: tenths_from_f64(cfg.null_or_die_fingerprint_ms).clamp(
            NULL_OR_DIE_POSITIVE_MS_MIN_TENTHS,
            NULL_OR_DIE_POSITIVE_MS_MAX_TENTHS,
        ),
        null_or_die_window_tenths: tenths_from_f64(cfg.null_or_die_window_ms).clamp(
            NULL_OR_DIE_POSITIVE_MS_MIN_TENTHS,
            NULL_OR_DIE_POSITIVE_MS_MAX_TENTHS,
        ),
        null_or_die_step_tenths: tenths_from_f64(cfg.null_or_die_step_ms).clamp(
            NULL_OR_DIE_POSITIVE_MS_MIN_TENTHS,
            NULL_OR_DIE_POSITIVE_MS_MAX_TENTHS,
        ),
        null_or_die_magic_offset_tenths: tenths_from_f64(cfg.null_or_die_magic_offset_ms).clamp(
            NULL_OR_DIE_MAGIC_OFFSET_MIN_TENTHS,
            NULL_OR_DIE_MAGIC_OFFSET_MAX_TENTHS,
        ),
        video_renderer_at_load: cfg.video_renderer,
        display_mode_at_load: cfg.display_mode(),
        display_monitor_at_load: cfg.display_monitor,
        display_width_at_load: cfg.display_width,
        display_height_at_load: cfg.display_height,
        max_fps_at_load: cfg.max_fps,
        vsync_at_load: cfg.vsync,
        present_mode_policy_at_load: cfg.present_mode_policy,
        display_mode_choices: build_display_mode_choices(&[]),
        software_thread_choices,
        software_thread_labels,
        max_fps_choices,
        max_fps_labels,
        resolution_choices: Vec::new(),
        refresh_rate_choices: Vec::new(),
        monitor_specs: Vec::new(),
        cursor_initialized: false,
        cursor_from_x: 0.0,
        cursor_from_y: 0.0,
        cursor_from_w: 0.0,
        cursor_from_h: 0.0,
        cursor_to_x: 0.0,
        cursor_to_y: 0.0,
        cursor_to_w: 0.0,
        cursor_to_h: 0.0,
        cursor_t: 1.0,
        row_tweens: Vec::new(),
        submenu_layout_cache_kind: Cell::new(None),
        submenu_row_layout_cache: RefCell::new(Vec::new()),
        description_layout_cache: RefCell::new(None),
        graphics_prev_visible_rows: Vec::new(),
        advanced_prev_visible_rows: Vec::new(),
        select_music_prev_visible_rows: Vec::new(),
    };

    sync_video_renderer(&mut state, cfg.video_renderer);
    sync_display_mode(
        &mut state,
        cfg.display_mode(),
        cfg.fullscreen_type,
        cfg.display_monitor,
        1,
    );
    sync_display_resolution(&mut state, cfg.display_width, cfg.display_height);

    set_choice_by_id(
        &mut state.sub_choice_indices_system,
        SYSTEM_OPTIONS_ROWS,
        SubRowId::Game,
        0,
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_system,
        SYSTEM_OPTIONS_ROWS,
        SubRowId::Theme,
        0,
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_system,
        SYSTEM_OPTIONS_ROWS,
        SubRowId::Language,
        language_choice_index(cfg.language_flag),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_system,
        SYSTEM_OPTIONS_ROWS,
        SubRowId::LogLevel,
        log_level_choice_index(cfg.log_level),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_system,
        SYSTEM_OPTIONS_ROWS,
        SubRowId::LogFile,
        usize::from(cfg.log_to_file),
    );
    if let Some(noteskin_row_idx) = SYSTEM_OPTIONS_ROWS
        .iter()
        .position(|row| row.id == SubRowId::DefaultNoteSkin)
        && let Some(slot) = state.sub_choice_indices_system.get_mut(noteskin_row_idx)
    {
        *slot = machine_noteskin_idx;
    }

    set_choice_by_id(
        &mut state.sub_choice_indices_graphics,
        GRAPHICS_OPTIONS_ROWS,
        SubRowId::VSync,
        yes_no_choice_index(cfg.vsync),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_graphics,
        GRAPHICS_OPTIONS_ROWS,
        SubRowId::PresentMode,
        present_mode_choice_index(cfg.present_mode_policy),
    );
    sync_max_fps(&mut state, cfg.max_fps);
    set_choice_by_id(
        &mut state.sub_choice_indices_graphics,
        GRAPHICS_OPTIONS_ROWS,
        SubRowId::ShowStats,
        cfg.show_stats_mode.min(3) as usize,
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_graphics,
        GRAPHICS_OPTIONS_ROWS,
        SubRowId::ValidationLayers,
        yes_no_choice_index(cfg.gfx_debug),
    );
    if let Some(slot) = state
        .sub_choice_indices_graphics
        .get_mut(SOFTWARE_THREADS_ROW_INDEX)
    {
        *slot = software_thread_choice_index(
            &state.software_thread_choices,
            cfg.software_renderer_threads,
        );
    }
    #[cfg(target_os = "windows")]
    set_choice_by_id(
        &mut state.sub_choice_indices_input_backend,
        INPUT_BACKEND_OPTIONS_ROWS,
        SubRowId::GamepadBackend,
        windows_backend_choice_index(cfg.windows_gamepad_backend),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_input_backend,
        INPUT_BACKEND_OPTIONS_ROWS,
        SubRowId::MenuNavigation,
        usize::from(cfg.three_key_navigation),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_input_backend,
        INPUT_BACKEND_OPTIONS_ROWS,
        SubRowId::OptionsNavigation,
        usize::from(cfg.arcade_options_navigation),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_input_backend,
        INPUT_BACKEND_OPTIONS_ROWS,
        SubRowId::MenuButtons,
        usize::from(cfg.only_dedicated_menu_buttons),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_machine,
        MACHINE_OPTIONS_ROWS,
        SubRowId::SelectProfile,
        usize::from(cfg.machine_show_select_profile),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_machine,
        MACHINE_OPTIONS_ROWS,
        SubRowId::SelectColor,
        usize::from(cfg.machine_show_select_color),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_machine,
        MACHINE_OPTIONS_ROWS,
        SubRowId::SelectStyle,
        usize::from(cfg.machine_show_select_style),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_machine,
        MACHINE_OPTIONS_ROWS,
        SubRowId::PreferredStyle,
        machine_preferred_style_choice_index(cfg.machine_preferred_style),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_machine,
        MACHINE_OPTIONS_ROWS,
        SubRowId::SelectPlayMode,
        usize::from(cfg.machine_show_select_play_mode),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_machine,
        MACHINE_OPTIONS_ROWS,
        SubRowId::PreferredMode,
        machine_preferred_mode_choice_index(cfg.machine_preferred_play_mode),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_machine,
        MACHINE_OPTIONS_ROWS,
        SubRowId::EvalSummary,
        usize::from(cfg.machine_show_eval_summary),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_machine,
        MACHINE_OPTIONS_ROWS,
        SubRowId::NameEntry,
        usize::from(cfg.machine_show_name_entry),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_machine,
        MACHINE_OPTIONS_ROWS,
        SubRowId::GameoverScreen,
        usize::from(cfg.machine_show_gameover),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_machine,
        MACHINE_OPTIONS_ROWS,
        SubRowId::MenuMusic,
        usize::from(cfg.menu_music),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_machine,
        MACHINE_OPTIONS_ROWS,
        SubRowId::Replays,
        usize::from(cfg.machine_enable_replays),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_machine,
        MACHINE_OPTIONS_ROWS,
        SubRowId::PerPlayerGlobalOffsets,
        usize::from(cfg.machine_allow_per_player_global_offsets),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_machine,
        MACHINE_OPTIONS_ROWS,
        SubRowId::KeyboardFeatures,
        usize::from(cfg.keyboard_features),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_machine,
        MACHINE_OPTIONS_ROWS,
        SubRowId::VideoBgs,
        usize::from(cfg.show_video_backgrounds),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_machine,
        MACHINE_OPTIONS_ROWS,
        SubRowId::WriteCurrentScreen,
        usize::from(cfg.write_current_screen),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_advanced,
        ADVANCED_OPTIONS_ROWS,
        SubRowId::DefaultFailType,
        default_fail_type_choice_index(cfg.default_fail_type),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_advanced,
        ADVANCED_OPTIONS_ROWS,
        SubRowId::BannerCache,
        usize::from(cfg.banner_cache),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_advanced,
        ADVANCED_OPTIONS_ROWS,
        SubRowId::CdTitleCache,
        usize::from(cfg.cdtitle_cache),
    );
    if let Some(slot) = state
        .sub_choice_indices_advanced
        .get_mut(ADVANCED_SONG_PARSING_THREADS_ROW_INDEX)
    {
        *slot =
            software_thread_choice_index(&state.software_thread_choices, cfg.song_parsing_threads);
    }
    set_choice_by_id(
        &mut state.sub_choice_indices_advanced,
        ADVANCED_OPTIONS_ROWS,
        SubRowId::CacheSongs,
        usize::from(cfg.cachesongs),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_advanced,
        ADVANCED_OPTIONS_ROWS,
        SubRowId::FastLoad,
        usize::from(cfg.fastload),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_null_or_die_options,
        NULL_OR_DIE_OPTIONS_ROWS,
        SubRowId::SyncGraph,
        sync_graph_mode_choice_index(cfg.null_or_die_sync_graph),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_null_or_die_options,
        NULL_OR_DIE_OPTIONS_ROWS,
        SubRowId::SyncConfidence,
        sync_confidence_choice_index(cfg.null_or_die_confidence_percent),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_null_or_die_options,
        NULL_OR_DIE_OPTIONS_ROWS,
        SubRowId::PackSyncThreads,
        software_thread_choice_index(
            &state.software_thread_choices,
            cfg.null_or_die_pack_sync_threads,
        ),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_null_or_die_options,
        NULL_OR_DIE_OPTIONS_ROWS,
        SubRowId::KernelTarget,
        null_or_die_kernel_target_choice_index(cfg.null_or_die_kernel_target),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_null_or_die_options,
        NULL_OR_DIE_OPTIONS_ROWS,
        SubRowId::KernelType,
        null_or_die_kernel_type_choice_index(cfg.null_or_die_kernel_type),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_null_or_die_options,
        NULL_OR_DIE_OPTIONS_ROWS,
        SubRowId::FullSpectrogram,
        yes_no_choice_index(cfg.null_or_die_full_spectrogram),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_course,
        COURSE_OPTIONS_ROWS,
        SubRowId::ShowRandomCourses,
        yes_no_choice_index(cfg.show_random_courses),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_course,
        COURSE_OPTIONS_ROWS,
        SubRowId::ShowMostPlayed,
        yes_no_choice_index(cfg.show_most_played_courses),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_course,
        COURSE_OPTIONS_ROWS,
        SubRowId::ShowIndividualScores,
        yes_no_choice_index(cfg.show_course_individual_scores),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_course,
        COURSE_OPTIONS_ROWS,
        SubRowId::AutosubmitIndividual,
        yes_no_choice_index(cfg.autosubmit_course_scores_individually),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_gameplay,
        GAMEPLAY_OPTIONS_ROWS,
        SubRowId::BgBrightness,
        bg_brightness_choice_index(cfg.bg_brightness),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_gameplay,
        GAMEPLAY_OPTIONS_ROWS,
        SubRowId::CenteredP1Notefield,
        usize::from(cfg.center_1player_notefield),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_gameplay,
        GAMEPLAY_OPTIONS_ROWS,
        SubRowId::ZmodRatingBox,
        usize::from(cfg.zmod_rating_box_text),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_gameplay,
        GAMEPLAY_OPTIONS_ROWS,
        SubRowId::BpmDecimal,
        usize::from(cfg.show_bpm_decimal),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_gameplay,
        GAMEPLAY_OPTIONS_ROWS,
        SubRowId::AutoScreenshot,
        auto_screenshot_cursor_index(cfg.auto_screenshot_eval),
    );

    set_choice_by_id(
        &mut state.sub_choice_indices_sound,
        SOUND_OPTIONS_ROWS,
        SubRowId::MasterVolume,
        master_volume_choice_index(cfg.master_volume),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_sound,
        SOUND_OPTIONS_ROWS,
        SubRowId::SfxVolume,
        master_volume_choice_index(cfg.sfx_volume),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_sound,
        SOUND_OPTIONS_ROWS,
        SubRowId::AssistTickVolume,
        master_volume_choice_index(cfg.assist_tick_volume),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_sound,
        SOUND_OPTIONS_ROWS,
        SubRowId::MusicVolume,
        master_volume_choice_index(cfg.music_volume),
    );
    let sound_device_idx =
        sound_device_choice_index(&state.sound_device_options, cfg.audio_output_device_index);
    set_sound_choice_index(&mut state, SubRowId::SoundDevice, sound_device_idx);
    set_sound_choice_index(
        &mut state,
        SubRowId::AudioOutputMode,
        audio_output_mode_choice_index(cfg.audio_output_mode),
    );
    #[cfg(target_os = "linux")]
    let linux_backend_idx = linux_audio_backend_choice_index(&state, cfg.linux_audio_backend);
    #[cfg(target_os = "linux")]
    set_sound_choice_index(&mut state, SubRowId::LinuxAudioBackend, linux_backend_idx);
    #[cfg(target_os = "linux")]
    set_sound_choice_index(
        &mut state,
        SubRowId::AlsaExclusive,
        alsa_exclusive_choice_index(cfg.audio_output_mode),
    );
    let sound_rate_idx = sample_rate_choice_index(&state, cfg.audio_sample_rate_hz);
    set_sound_choice_index(&mut state, SubRowId::AudioSampleRate, sound_rate_idx);
    set_choice_by_id(
        &mut state.sub_choice_indices_sound,
        SOUND_OPTIONS_ROWS,
        SubRowId::MineSounds,
        usize::from(cfg.mine_hit_sound),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_sound,
        SOUND_OPTIONS_ROWS,
        SubRowId::RateModPreservesPitch,
        usize::from(cfg.rate_mod_preserves_pitch),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_select_music,
        SELECT_MUSIC_OPTIONS_ROWS,
        SubRowId::ShowBanners,
        yes_no_choice_index(cfg.show_select_music_banners),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_select_music,
        SELECT_MUSIC_OPTIONS_ROWS,
        SubRowId::ShowVideoBanners,
        yes_no_choice_index(cfg.show_select_music_video_banners),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_select_music,
        SELECT_MUSIC_OPTIONS_ROWS,
        SubRowId::ShowBreakdown,
        yes_no_choice_index(cfg.show_select_music_breakdown),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_select_music,
        SELECT_MUSIC_OPTIONS_ROWS,
        SubRowId::BreakdownStyle,
        breakdown_style_choice_index(cfg.select_music_breakdown_style),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_select_music,
        SELECT_MUSIC_OPTIONS_ROWS,
        SubRowId::ShowNativeLanguage,
        translated_titles_choice_index(cfg.translated_titles),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_select_music,
        SELECT_MUSIC_OPTIONS_ROWS,
        SubRowId::MusicWheelSpeed,
        music_wheel_scroll_speed_choice_index(cfg.music_wheel_switch_speed),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_select_music,
        SELECT_MUSIC_OPTIONS_ROWS,
        SubRowId::MusicWheelStyle,
        select_music_wheel_style_choice_index(cfg.select_music_wheel_style),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_select_music,
        SELECT_MUSIC_OPTIONS_ROWS,
        SubRowId::ShowCdTitles,
        yes_no_choice_index(cfg.show_select_music_cdtitles),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_select_music,
        SELECT_MUSIC_OPTIONS_ROWS,
        SubRowId::ShowWheelGrades,
        yes_no_choice_index(cfg.show_music_wheel_grades),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_select_music,
        SELECT_MUSIC_OPTIONS_ROWS,
        SubRowId::ShowWheelLamps,
        yes_no_choice_index(cfg.show_music_wheel_lamps),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_select_music,
        SELECT_MUSIC_OPTIONS_ROWS,
        SubRowId::ItlWheelData,
        select_music_itl_wheel_choice_index(cfg.select_music_itl_wheel_mode),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_select_music,
        SELECT_MUSIC_OPTIONS_ROWS,
        SubRowId::NewPackBadge,
        new_pack_mode_choice_index(cfg.select_music_new_pack_mode),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_select_music,
        SELECT_MUSIC_OPTIONS_ROWS,
        SubRowId::ShowPatternInfo,
        select_music_pattern_info_choice_index(cfg.select_music_pattern_info_mode),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_select_music,
        SELECT_MUSIC_OPTIONS_ROWS,
        SubRowId::ChartInfo,
        select_music_chart_info_cursor_index(
            cfg.select_music_chart_info_peak_nps,
            cfg.select_music_chart_info_matrix_rating,
        ),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_select_music,
        SELECT_MUSIC_OPTIONS_ROWS,
        SubRowId::MusicPreviews,
        yes_no_choice_index(cfg.show_select_music_previews),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_select_music,
        SELECT_MUSIC_OPTIONS_ROWS,
        SubRowId::PreviewMarker,
        yes_no_choice_index(cfg.show_select_music_preview_marker),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_select_music,
        SELECT_MUSIC_OPTIONS_ROWS,
        SubRowId::LoopMusic,
        usize::from(cfg.select_music_preview_loop),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_select_music,
        SELECT_MUSIC_OPTIONS_ROWS,
        SubRowId::ShowGameplayTimer,
        yes_no_choice_index(cfg.show_select_music_gameplay_timer),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_select_music,
        SELECT_MUSIC_OPTIONS_ROWS,
        SubRowId::ShowGsBox,
        yes_no_choice_index(cfg.show_select_music_scorebox),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_select_music,
        SELECT_MUSIC_OPTIONS_ROWS,
        SubRowId::GsBoxPlacement,
        select_music_scorebox_placement_choice_index(cfg.select_music_scorebox_placement),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_select_music,
        SELECT_MUSIC_OPTIONS_ROWS,
        SubRowId::GsBoxLeaderboards,
        scorebox_cycle_cursor_index(
            cfg.select_music_scorebox_cycle_itg,
            cfg.select_music_scorebox_cycle_ex,
            cfg.select_music_scorebox_cycle_hard_ex,
            cfg.select_music_scorebox_cycle_tournaments,
        ),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_groovestats,
        GROOVESTATS_OPTIONS_ROWS,
        SubRowId::EnableGrooveStats,
        yes_no_choice_index(cfg.enable_groovestats),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_groovestats,
        GROOVESTATS_OPTIONS_ROWS,
        SubRowId::EnableBoogieStats,
        yes_no_choice_index(cfg.enable_boogiestats),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_groovestats,
        GROOVESTATS_OPTIONS_ROWS,
        SubRowId::GsSubmitFails,
        yes_no_choice_index(cfg.submit_groovestats_fails),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_groovestats,
        GROOVESTATS_OPTIONS_ROWS,
        SubRowId::AutoPopulateScores,
        yes_no_choice_index(cfg.auto_populate_gs_scores),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_groovestats,
        GROOVESTATS_OPTIONS_ROWS,
        SubRowId::AutoDownloadUnlocks,
        yes_no_choice_index(cfg.auto_download_unlocks),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_groovestats,
        GROOVESTATS_OPTIONS_ROWS,
        SubRowId::SeparateUnlocksByPlayer,
        yes_no_choice_index(cfg.separate_unlocks_by_player),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_arrowcloud,
        ARROWCLOUD_OPTIONS_ROWS,
        SubRowId::EnableArrowCloud,
        yes_no_choice_index(cfg.enable_arrowcloud),
    );
    set_choice_by_id(
        &mut state.sub_choice_indices_arrowcloud,
        ARROWCLOUD_OPTIONS_ROWS,
        SubRowId::ArrowCloudSubmitFails,
        yes_no_choice_index(cfg.submit_arrowcloud_fails),
    );
    refresh_score_import_options(&mut state);
    refresh_null_or_die_options(&mut state);
    set_choice_by_id(
        &mut state.sub_choice_indices_score_import,
        SCORE_IMPORT_OPTIONS_ROWS,
        SubRowId::ScoreImportOnlyMissing,
        yes_no_choice_index(false),
    );
    sync_submenu_cursor_indices(&mut state);
    state
}

pub fn open_input_submenu(state: &mut State) {
    state.view = OptionsView::Submenu(SubmenuKind::Input);
    state.pending_submenu_kind = None;
    state.pending_submenu_parent_kind = None;
    state.submenu_parent_kind = None;
    state.submenu_transition = SubmenuTransition::None;
    state.submenu_fade_t = 0.0;
    state.content_alpha = 1.0;
    state.sub_selected = 0;
    state.sub_prev_selected = 0;
    state.sub_inline_x = f32::NAN;
    sync_submenu_cursor_indices(state);
    state.cursor_initialized = false;
    state.cursor_t = 1.0;
    state.row_tweens.clear();
    state.graphics_prev_visible_rows.clear();
    state.advanced_prev_visible_rows.clear();
    state.select_music_prev_visible_rows.clear();
    clear_navigation_holds(state);
    clear_render_cache(state);
}

fn submenu_choice_indices(state: &State, kind: SubmenuKind) -> &[usize] {
    match kind {
        SubmenuKind::System => &state.sub_choice_indices_system,
        SubmenuKind::Graphics => &state.sub_choice_indices_graphics,
        SubmenuKind::Input => &state.sub_choice_indices_input,
        SubmenuKind::InputBackend => &state.sub_choice_indices_input_backend,
        SubmenuKind::OnlineScoring => &state.sub_choice_indices_online_scoring,
        SubmenuKind::NullOrDie => &state.sub_choice_indices_null_or_die,
        SubmenuKind::NullOrDieOptions => &state.sub_choice_indices_null_or_die_options,
        SubmenuKind::SyncPacks => &state.sub_choice_indices_sync_packs,
        SubmenuKind::Machine => &state.sub_choice_indices_machine,
        SubmenuKind::Advanced => &state.sub_choice_indices_advanced,
        SubmenuKind::Course => &state.sub_choice_indices_course,
        SubmenuKind::Gameplay => &state.sub_choice_indices_gameplay,
        SubmenuKind::Sound => &state.sub_choice_indices_sound,
        SubmenuKind::SelectMusic => &state.sub_choice_indices_select_music,
        SubmenuKind::GrooveStats => &state.sub_choice_indices_groovestats,
        SubmenuKind::ArrowCloud => &state.sub_choice_indices_arrowcloud,
        SubmenuKind::ScoreImport => &state.sub_choice_indices_score_import,
    }
}

const fn submenu_choice_indices_mut(state: &mut State, kind: SubmenuKind) -> &mut Vec<usize> {
    match kind {
        SubmenuKind::System => &mut state.sub_choice_indices_system,
        SubmenuKind::Graphics => &mut state.sub_choice_indices_graphics,
        SubmenuKind::Input => &mut state.sub_choice_indices_input,
        SubmenuKind::InputBackend => &mut state.sub_choice_indices_input_backend,
        SubmenuKind::OnlineScoring => &mut state.sub_choice_indices_online_scoring,
        SubmenuKind::NullOrDie => &mut state.sub_choice_indices_null_or_die,
        SubmenuKind::NullOrDieOptions => &mut state.sub_choice_indices_null_or_die_options,
        SubmenuKind::SyncPacks => &mut state.sub_choice_indices_sync_packs,
        SubmenuKind::Machine => &mut state.sub_choice_indices_machine,
        SubmenuKind::Advanced => &mut state.sub_choice_indices_advanced,
        SubmenuKind::Course => &mut state.sub_choice_indices_course,
        SubmenuKind::Gameplay => &mut state.sub_choice_indices_gameplay,
        SubmenuKind::Sound => &mut state.sub_choice_indices_sound,
        SubmenuKind::SelectMusic => &mut state.sub_choice_indices_select_music,
        SubmenuKind::GrooveStats => &mut state.sub_choice_indices_groovestats,
        SubmenuKind::ArrowCloud => &mut state.sub_choice_indices_arrowcloud,
        SubmenuKind::ScoreImport => &mut state.sub_choice_indices_score_import,
    }
}

fn submenu_cursor_indices(state: &State, kind: SubmenuKind) -> &[usize] {
    match kind {
        SubmenuKind::System => &state.sub_cursor_indices_system,
        SubmenuKind::Graphics => &state.sub_cursor_indices_graphics,
        SubmenuKind::Input => &state.sub_cursor_indices_input,
        SubmenuKind::InputBackend => &state.sub_cursor_indices_input_backend,
        SubmenuKind::OnlineScoring => &state.sub_cursor_indices_online_scoring,
        SubmenuKind::NullOrDie => &state.sub_cursor_indices_null_or_die,
        SubmenuKind::NullOrDieOptions => &state.sub_cursor_indices_null_or_die_options,
        SubmenuKind::SyncPacks => &state.sub_cursor_indices_sync_packs,
        SubmenuKind::Machine => &state.sub_cursor_indices_machine,
        SubmenuKind::Advanced => &state.sub_cursor_indices_advanced,
        SubmenuKind::Course => &state.sub_cursor_indices_course,
        SubmenuKind::Gameplay => &state.sub_cursor_indices_gameplay,
        SubmenuKind::Sound => &state.sub_cursor_indices_sound,
        SubmenuKind::SelectMusic => &state.sub_cursor_indices_select_music,
        SubmenuKind::GrooveStats => &state.sub_cursor_indices_groovestats,
        SubmenuKind::ArrowCloud => &state.sub_cursor_indices_arrowcloud,
        SubmenuKind::ScoreImport => &state.sub_cursor_indices_score_import,
    }
}

const fn submenu_cursor_indices_mut(state: &mut State, kind: SubmenuKind) -> &mut Vec<usize> {
    match kind {
        SubmenuKind::System => &mut state.sub_cursor_indices_system,
        SubmenuKind::Graphics => &mut state.sub_cursor_indices_graphics,
        SubmenuKind::Input => &mut state.sub_cursor_indices_input,
        SubmenuKind::InputBackend => &mut state.sub_cursor_indices_input_backend,
        SubmenuKind::OnlineScoring => &mut state.sub_cursor_indices_online_scoring,
        SubmenuKind::NullOrDie => &mut state.sub_cursor_indices_null_or_die,
        SubmenuKind::NullOrDieOptions => &mut state.sub_cursor_indices_null_or_die_options,
        SubmenuKind::SyncPacks => &mut state.sub_cursor_indices_sync_packs,
        SubmenuKind::Machine => &mut state.sub_cursor_indices_machine,
        SubmenuKind::Advanced => &mut state.sub_cursor_indices_advanced,
        SubmenuKind::Course => &mut state.sub_cursor_indices_course,
        SubmenuKind::Gameplay => &mut state.sub_cursor_indices_gameplay,
        SubmenuKind::Sound => &mut state.sub_cursor_indices_sound,
        SubmenuKind::SelectMusic => &mut state.sub_cursor_indices_select_music,
        SubmenuKind::GrooveStats => &mut state.sub_cursor_indices_groovestats,
        SubmenuKind::ArrowCloud => &mut state.sub_cursor_indices_arrowcloud,
        SubmenuKind::ScoreImport => &mut state.sub_cursor_indices_score_import,
    }
}

fn sync_submenu_cursor_indices(state: &mut State) {
    state.sub_cursor_indices_system = state.sub_choice_indices_system.clone();
    state.sub_cursor_indices_graphics = state.sub_choice_indices_graphics.clone();
    state.sub_cursor_indices_input = state.sub_choice_indices_input.clone();
    state.sub_cursor_indices_input_backend = state.sub_choice_indices_input_backend.clone();
    state.sub_cursor_indices_online_scoring = state.sub_choice_indices_online_scoring.clone();
    state.sub_cursor_indices_null_or_die = state.sub_choice_indices_null_or_die.clone();
    state.sub_cursor_indices_null_or_die_options =
        state.sub_choice_indices_null_or_die_options.clone();
    state.sub_cursor_indices_sync_packs = state.sub_choice_indices_sync_packs.clone();
    state.sub_cursor_indices_machine = state.sub_choice_indices_machine.clone();
    state.sub_cursor_indices_advanced = state.sub_choice_indices_advanced.clone();
    state.sub_cursor_indices_course = state.sub_choice_indices_course.clone();
    state.sub_cursor_indices_gameplay = state.sub_choice_indices_gameplay.clone();
    state.sub_cursor_indices_sound = state.sub_choice_indices_sound.clone();
    state.sub_cursor_indices_select_music = state.sub_choice_indices_select_music.clone();
    state.sub_cursor_indices_groovestats = state.sub_choice_indices_groovestats.clone();
    state.sub_cursor_indices_arrowcloud = state.sub_choice_indices_arrowcloud.clone();
    state.sub_cursor_indices_score_import = state.sub_choice_indices_score_import.clone();
}

pub fn sync_video_renderer(state: &mut State, renderer: BackendType) {
    state.video_renderer_at_load = renderer;
    if let Some(slot) = state
        .sub_choice_indices_graphics
        .get_mut(VIDEO_RENDERER_ROW_INDEX)
    {
        *slot = backend_to_renderer_choice_index(renderer);
    }
    sync_submenu_cursor_indices(state);
    clear_render_cache(state);
}

pub fn sync_display_mode(
    state: &mut State,
    mode: DisplayMode,
    fullscreen_type: FullscreenType,
    monitor: usize,
    monitor_count: usize,
) {
    state.display_mode_at_load = mode;
    state.display_monitor_at_load = monitor;
    set_display_mode_row_selection(state, monitor_count, mode, monitor);
    let target_type = match mode {
        DisplayMode::Fullscreen(ft) => ft,
        DisplayMode::Windowed => fullscreen_type,
    };
    if let Some(slot) = state
        .sub_choice_indices_graphics
        .get_mut(FULLSCREEN_TYPE_ROW_INDEX)
    {
        *slot = fullscreen_type_to_choice_index(target_type);
    }
    sync_submenu_cursor_indices(state);
    clear_render_cache(state);
}

pub fn sync_display_resolution(state: &mut State, width: u32, height: u32) {
    sync_display_aspect_ratio(state, width, height);
    rebuild_resolution_choices(state, width, height);
    state.display_width_at_load = width;
    state.display_height_at_load = height;
    sync_submenu_cursor_indices(state);
    clear_render_cache(state);
}

pub fn sync_show_stats_mode(state: &mut State, mode: u8) {
    set_choice_by_id(
        &mut state.sub_choice_indices_graphics,
        GRAPHICS_OPTIONS_ROWS,
        SubRowId::ShowStats,
        mode.min(3) as usize,
    );
    sync_submenu_cursor_indices(state);
    clear_render_cache(state);
}

pub fn sync_translated_titles(state: &mut State, enabled: bool) {
    set_choice_by_id(
        &mut state.sub_choice_indices_select_music,
        SELECT_MUSIC_OPTIONS_ROWS,
        SubRowId::ShowNativeLanguage,
        translated_titles_choice_index(enabled),
    );
    sync_submenu_cursor_indices(state);
    clear_render_cache(state);
}

pub fn sync_max_fps(state: &mut State, max_fps: u16) {
    let had_explicit_cap = state.max_fps_at_load != 0;
    state.max_fps_at_load = max_fps;
    set_max_fps_enabled_choice(state, max_fps != 0);
    if max_fps != 0 || !had_explicit_cap {
        seed_max_fps_value_choice(state, max_fps);
    }
    sync_submenu_cursor_indices(state);
    clear_render_cache(state);
}

pub fn sync_vsync(state: &mut State, enabled: bool) {
    state.vsync_at_load = enabled;
    if let Some(slot) = state.sub_choice_indices_graphics.get_mut(VSYNC_ROW_INDEX) {
        *slot = yes_no_choice_index(enabled);
    }
    sync_submenu_cursor_indices(state);
    clear_render_cache(state);
}

pub fn sync_present_mode_policy(state: &mut State, mode: PresentModePolicy) {
    state.present_mode_policy_at_load = mode;
    if let Some(slot) = state
        .sub_choice_indices_graphics
        .get_mut(PRESENT_MODE_ROW_INDEX)
    {
        *slot = present_mode_choice_index(mode);
    }
    sync_submenu_cursor_indices(state);
    clear_render_cache(state);
}

pub fn in_transition() -> (Vec<Actor>, f32) {
    let actor = act!(quad:
        align(0.0, 0.0): xy(0.0, 0.0):
        zoomto(screen_width(), screen_height()):
        diffuse(0.0, 0.0, 0.0, 1.0):
        z(1100):
        linear(TRANSITION_IN_DURATION): alpha(0.0):
        linear(0.0): visible(false)
    );
    (vec![actor], TRANSITION_IN_DURATION)
}

pub fn out_transition() -> (Vec<Actor>, f32) {
    let actor = act!(quad:
        align(0.0, 0.0): xy(0.0, 0.0):
        zoomto(screen_width(), screen_height()):
        diffuse(0.0, 0.0, 0.0, 0.0):
        z(1200):
        linear(TRANSITION_OUT_DURATION): alpha(1.0)
    );
    (vec![actor], TRANSITION_OUT_DURATION)
}

/* --------------------------------- input --------------------------------- */

// Keyboard input is handled centrally via the virtual dispatcher in app

fn clear_navigation_holds(state: &mut State) {
    state.nav_key_held_direction = None;
    state.nav_key_held_since = None;
    state.nav_key_last_scrolled_at = None;
    state.nav_lr_held_direction = None;
    state.nav_lr_held_since = None;
    state.nav_lr_last_adjusted_at = None;
}

fn start_reload_songs_and_courses(state: &mut State) {
    if state.reload_ui.is_some() {
        return;
    }

    // Clear navigation holds so the menu can't "run away" after reload finishes.
    clear_navigation_holds(state);

    let (tx, rx) = std::sync::mpsc::channel::<ReloadMsg>();
    state.reload_ui = Some(ReloadUiState::new(rx));

    std::thread::spawn(move || {
        let _ = tx.send(ReloadMsg::Phase(ReloadPhase::Songs));

        let mut on_song = |done: usize, total: usize, pack: &str, song: &str| {
            let _ = tx.send(ReloadMsg::Song {
                done,
                total,
                pack: pack.to_owned(),
                song: song.to_owned(),
            });
        };
        song_loading::scan_and_load_songs_with_progress_counts(
            &dirs::app_dirs().songs_dir(),
            &mut on_song,
        );

        let _ = tx.send(ReloadMsg::Phase(ReloadPhase::Courses));

        let mut on_course = |done: usize, total: usize, group: &str, course: &str| {
            let _ = tx.send(ReloadMsg::Course {
                done,
                total,
                group: group.to_owned(),
                course: course.to_owned(),
            });
        };
        let dirs = dirs::app_dirs();
        course::scan_and_load_courses_with_progress_counts(
            &dirs.courses_dir(),
            &dirs.songs_dir(),
            &mut on_course,
        );

        let _ = tx.send(ReloadMsg::Done);
    });
}

fn begin_score_import(state: &mut State, selection: ScoreImportSelection) {
    if state.score_import_ui.is_some() {
        return;
    }
    clear_navigation_holds(state);
    let mut profile_cfg = profile::Profile::default();
    profile_cfg
        .display_name
        .clone_from(&selection.profile.display_name);
    profile_cfg
        .groovestats_api_key
        .clone_from(&selection.profile.gs_api_key);
    profile_cfg
        .groovestats_username
        .clone_from(&selection.profile.gs_username);
    profile_cfg
        .arrowcloud_api_key
        .clone_from(&selection.profile.ac_api_key);

    let endpoint = selection.endpoint;
    let profile_id = selection.profile.id.clone();
    let profile_name = if selection.profile.display_name.is_empty() {
        selection.profile.id.clone()
    } else {
        selection.profile.display_name.clone()
    };
    let pack_group = selection.pack_group.clone();
    let pack_label = selection.pack_label.clone();
    let only_missing_gs_scores = selection.only_missing_gs_scores;

    log::warn!(
        "{} score import starting for '{}' (pack: {}, only_missing_gs={}). Hard-limited to 3 requests/sec. For many charts this can take more than one hour.",
        endpoint.display_name(),
        profile_name,
        pack_label,
        if only_missing_gs_scores { "yes" } else { "no" }
    );

    let cancel_requested = Arc::new(AtomicBool::new(false));
    let cancel_for_thread = Arc::clone(&cancel_requested);
    let (tx, rx) = std::sync::mpsc::channel::<ScoreImportMsg>();
    state.score_import_ui = Some(ScoreImportUiState::new(
        endpoint,
        profile_name.clone(),
        pack_label,
        cancel_requested,
        rx,
    ));

    std::thread::spawn(move || {
        let result = scores::import_scores_for_profile(
            endpoint,
            profile_id,
            profile_cfg,
            pack_group,
            only_missing_gs_scores,
            |progress| {
                let _ = tx.send(ScoreImportMsg::Progress(progress));
            },
            || cancel_for_thread.load(Ordering::Relaxed),
        );
        let done_msg = result.map_err(|e| e.to_string());
        let _ = tx.send(ScoreImportMsg::Done(done_msg));
    });
}

fn begin_score_import_from_confirm(state: &mut State) {
    let Some(confirm) = state.score_import_confirm.take() else {
        return;
    };
    begin_score_import(state, confirm.selection);
}

#[inline(always)]
fn sync_pack_preferred_difficulty_index() -> usize {
    let profile_data = profile::get();
    let play_style = profile::get_session_play_style();
    let max_diff_index = color::FILE_DIFFICULTY_NAMES.len().saturating_sub(1);
    if max_diff_index == 0 {
        0
    } else {
        profile_data
            .last_played(play_style)
            .difficulty_index
            .min(max_diff_index)
    }
}

fn begin_pack_sync(state: &mut State, selection: SyncPackSelection) {
    if !matches!(
        state.pack_sync_overlay,
        shared_pack_sync::OverlayState::Hidden
    ) {
        return;
    }

    clear_navigation_holds(state);

    let target_chart_type = profile::get_session_play_style().chart_type();
    let preferred_difficulty_index = sync_pack_preferred_difficulty_index();
    let pack_group = selection.pack_group.as_deref();
    let song_cache = crate::game::song::get_song_cache();
    let mut targets = Vec::new();

    for pack in song_cache.iter() {
        if pack_group.is_some() && Some(pack.group_name.as_str()) != pack_group {
            continue;
        }
        for song in &pack.songs {
            let Some(steps_index) = select_music::best_steps_index(
                song.as_ref(),
                target_chart_type,
                preferred_difficulty_index,
            ) else {
                continue;
            };
            let Some(chart_ix) = select_music::selected_chart_ix_for_sync(
                song.as_ref(),
                target_chart_type,
                steps_index,
            ) else {
                continue;
            };
            let Some(chart) = song.charts.get(chart_ix) else {
                continue;
            };
            targets.push(shared_pack_sync::TargetSpec {
                simfile_path: song.simfile_path.clone(),
                song_title: song.display_full_title(false),
                chart_label: shared_pack_sync::chart_label(chart),
                chart_ix,
            });
        }
    }
    drop(song_cache);

    if !shared_pack_sync::begin(
        &mut state.pack_sync_overlay,
        selection.pack_label.clone(),
        targets,
    ) {
        log::warn!(
            "Failed to start pack sync for {:?}: no matching charts were found.",
            selection.pack_group
        );
    }
}

fn begin_pack_sync_from_confirm(state: &mut State) {
    let Some(confirm) = state.sync_pack_confirm.take() else {
        return;
    };
    begin_pack_sync(state, confirm.selection);
}

fn poll_reload_ui(reload: &mut ReloadUiState) {
    while let Ok(msg) = reload.rx.try_recv() {
        match msg {
            ReloadMsg::Phase(phase) => {
                reload.phase = phase;
                reload.line2.clear();
                reload.line3.clear();
            }
            ReloadMsg::Song {
                done,
                total,
                pack,
                song,
            } => {
                reload.phase = ReloadPhase::Songs;
                reload.songs_done = done;
                reload.songs_total = total;
                reload.line2 = pack;
                reload.line3 = song;
            }
            ReloadMsg::Course {
                done,
                total,
                group,
                course,
            } => {
                reload.phase = ReloadPhase::Courses;
                reload.courses_done = done;
                reload.courses_total = total;
                reload.line2 = group;
                reload.line3 = course;
            }
            ReloadMsg::Done => {
                reload.done = true;
            }
        }
    }
}

#[inline(always)]
fn reload_progress(reload: &ReloadUiState) -> (usize, usize, f32) {
    let done = reload.songs_done.saturating_add(reload.courses_done);
    let mut total = reload.songs_total.saturating_add(reload.courses_total);
    if total < done {
        total = done;
    }
    let mut progress = if total > 0 {
        (done as f32 / total as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    if !reload.done && total > 0 && progress >= 1.0 {
        progress = 0.999;
    }
    (done, total, progress)
}

fn reload_detail_lines(reload: &ReloadUiState) -> (String, String) {
    (reload.line2.clone(), reload.line3.clone())
}

fn build_reload_overlay_actors(reload: &ReloadUiState, active_color_index: i32) -> Vec<Actor> {
    let (done, total, progress) = reload_progress(reload);
    let elapsed = reload.started_at.elapsed().as_secs_f32().max(0.0);
    let count_text = if total == 0 {
        String::new()
    } else {
        crate::screens::progress_count_text(done, total)
    };
    let show_speed_row = total > 0;
    let speed_text = if elapsed > 0.0 && show_speed_row {
        tr_fmt(
            "SelectMusic",
            "LoadingSpeed",
            &[("speed", &format!("{:.1}", done as f32 / elapsed))],
        )
        .to_string()
    } else if show_speed_row {
        tr_fmt("SelectMusic", "LoadingSpeed", &[("speed", "0.0")]).to_string()
    } else {
        String::new()
    };
    let (line2, line3) = reload_detail_lines(reload);
    let fill = color::decorative_rgba(active_color_index);

    let bar_w = widescale(360.0, 520.0);
    let bar_h = RELOAD_BAR_H;
    let bar_cx = screen_width() * 0.5;
    let bar_cy = screen_height() * 0.5 + 34.0;
    let fill_w = (bar_w - 4.0) * progress.clamp(0.0, 1.0);

    let mut out: Vec<Actor> = Vec::with_capacity(7);
    out.push(act!(quad:
        align(0.0, 0.0):
        xy(0.0, 0.0):
        zoomto(screen_width(), screen_height()):
        diffuse(0.0, 0.0, 0.0, 0.65):
        z(300)
    ));
    let phase_label = match reload.phase {
        ReloadPhase::Songs => tr("Init", "LoadingSongsText"),
        ReloadPhase::Courses => tr("Init", "LoadingCoursesText"),
    };
    out.push(act!(text:
        font("miso"):
        settext(if total == 0 { tr("Init", "InitializingText") } else { phase_label }):
        align(0.5, 0.5):
        xy(screen_width() * 0.5, bar_cy - 98.0):
        zoom(1.05):
        horizalign(center):
        z(301)
    ));
    if !line2.is_empty() {
        out.push(act!(text:
            font("miso"):
            settext(line2):
            align(0.5, 0.5):
            xy(screen_width() * 0.5, bar_cy - 74.0):
            zoom(0.95):
            maxwidth(screen_width() * 0.9):
            horizalign(center):
            z(301)
        ));
    }
    if !line3.is_empty() {
        out.push(act!(text:
            font("miso"):
            settext(line3):
            align(0.5, 0.5):
            xy(screen_width() * 0.5, bar_cy - 50.0):
            zoom(0.95):
            maxwidth(screen_width() * 0.9):
            horizalign(center):
            z(301)
        ));
    }

    let mut bar_children = Vec::with_capacity(4);
    bar_children.push(act!(quad:
        align(0.5, 0.5):
        xy(bar_w / 2.0, bar_h / 2.0):
        zoomto(bar_w, bar_h):
        diffuse(1.0, 1.0, 1.0, 1.0):
        z(0)
    ));
    bar_children.push(act!(quad:
        align(0.5, 0.5):
        xy(bar_w / 2.0, bar_h / 2.0):
        zoomto(bar_w - 4.0, bar_h - 4.0):
        diffuse(0.0, 0.0, 0.0, 1.0):
        z(1)
    ));
    if fill_w > 0.0 {
        bar_children.push(act!(quad:
            align(0.0, 0.5):
            xy(2.0, bar_h / 2.0):
            zoomto(fill_w, bar_h - 4.0):
            diffuse(fill[0], fill[1], fill[2], 1.0):
            z(2)
        ));
    }
    bar_children.push(act!(text:
        font("miso"):
        settext(count_text):
        align(0.5, 0.5):
        xy(bar_w / 2.0, bar_h / 2.0):
        zoom(0.9):
        horizalign(center):
        z(3)
    ));
    out.push(Actor::Frame {
        align: [0.5, 0.5],
        offset: [bar_cx, bar_cy],
        size: [actors::SizeSpec::Px(bar_w), actors::SizeSpec::Px(bar_h)],
        background: None,
        z: 301,
        children: bar_children,
    });

    if show_speed_row {
        out.push(act!(text:
            font("miso"):
            settext(speed_text):
            align(0.5, 0.5):
            xy(screen_width() * 0.5, bar_cy + 36.0):
            zoom(0.9):
            horizalign(center):
            z(301)
        ));
    }
    out
}

fn poll_score_import_ui(score_import: &mut ScoreImportUiState) {
    while let Ok(msg) = score_import.rx.try_recv() {
        match msg {
            ScoreImportMsg::Progress(progress) => {
                score_import.total_charts = progress.total_charts;
                score_import.processed_charts = progress.processed_charts;
                score_import.imported_scores = progress.imported_scores;
                score_import.missing_scores = progress.missing_scores;
                score_import.failed_requests = progress.failed_requests;
                score_import.detail_line = progress.detail;
            }
            ScoreImportMsg::Done(result) => {
                score_import.done = true;
                score_import.done_since = Some(Instant::now());
                score_import.done_message = match result {
                    Ok(summary) => {
                        if summary.canceled {
                            format!(
                                "Canceled: requested={}, imported={}, missing={}, failed={} (elapsed {:.1}s)",
                                summary.requested_charts,
                                summary.imported_scores,
                                summary.missing_scores,
                                summary.failed_requests,
                                summary.elapsed_seconds
                            )
                        } else {
                            format!(
                                "Complete: requested={}, imported={}, missing={}, failed={}, rate={} req/s (elapsed {:.1}s)",
                                summary.requested_charts,
                                summary.imported_scores,
                                summary.missing_scores,
                                summary.failed_requests,
                                summary.rate_limit_per_second,
                                summary.elapsed_seconds
                            )
                        }
                    }
                    Err(e) => tr_fmt(
                        "OptionsScoreImport",
                        "ImportFailed",
                        &[("error", &e.to_string())],
                    )
                    .to_string(),
                };
            }
        }
    }
}

pub fn update(state: &mut State, dt: f32, asset_manager: &AssetManager) -> Option<ScreenAction> {
    if state.reload_ui.is_some() {
        let done = {
            let reload = state.reload_ui.as_mut().unwrap();
            poll_reload_ui(reload);
            reload.done
        };
        if done {
            state.reload_ui = None;
            refresh_score_import_pack_options(state);
        }
        return None;
    }
    if let Some(score_import) = state.score_import_ui.as_mut() {
        poll_score_import_ui(score_import);
        if score_import.done
            && score_import
                .done_since
                .is_some_and(|at| at.elapsed().as_secs_f32() >= SCORE_IMPORT_DONE_OVERLAY_SECONDS)
        {
            state.score_import_ui = None;
        }
        return None;
    }
    if shared_pack_sync::poll(&mut state.pack_sync_overlay) {
        return None;
    }

    let mut pending_action: Option<ScreenAction> = None;
    // ------------------------- local submenu fade ------------------------- //
    match state.submenu_transition {
        SubmenuTransition::None => {
            state.content_alpha = 1.0;
        }
        SubmenuTransition::FadeOutToSubmenu => {
            let step = if SUBMENU_FADE_DURATION > 0.0 {
                dt / SUBMENU_FADE_DURATION
            } else {
                1.0
            };
            state.submenu_fade_t = (state.submenu_fade_t + step).min(1.0);
            state.content_alpha = 1.0 - state.submenu_fade_t;
            if state.submenu_fade_t >= 1.0 {
                // Apply deferred settings before leaving the submenu.
                if matches!(state.view, OptionsView::Submenu(SubmenuKind::InputBackend))
                    && let Some(enabled) = state.pending_dedicated_menu_buttons.take()
                {
                    config::update_only_dedicated_menu_buttons(enabled);
                }
                // Switch view to the target submenu, then fade it in.
                let target_kind = state.pending_submenu_kind.unwrap_or(SubmenuKind::System);
                state.view = OptionsView::Submenu(target_kind);
                state.pending_submenu_kind = None;
                state.submenu_parent_kind = state.pending_submenu_parent_kind.take();
                state.sub_selected = 0;
                state.sub_prev_selected = 0;
                state.sub_inline_x = f32::NAN;
                sync_submenu_cursor_indices(state);
                state.cursor_initialized = false;
                state.cursor_t = 1.0;
                state.row_tweens.clear();
                state.graphics_prev_visible_rows.clear();
                state.advanced_prev_visible_rows.clear();
                state.select_music_prev_visible_rows.clear();
                state.nav_key_held_direction = None;
                state.nav_key_held_since = None;
                state.nav_key_last_scrolled_at = None;
                state.nav_lr_held_direction = None;
                state.nav_lr_held_since = None;
                state.nav_lr_last_adjusted_at = None;
                state.submenu_transition = SubmenuTransition::FadeInSubmenu;
                state.submenu_fade_t = 0.0;
                state.content_alpha = 0.0;
            }
        }
        SubmenuTransition::FadeInSubmenu => {
            let step = if SUBMENU_FADE_DURATION > 0.0 {
                dt / SUBMENU_FADE_DURATION
            } else {
                1.0
            };
            state.submenu_fade_t = (state.submenu_fade_t + step).min(1.0);
            state.content_alpha = state.submenu_fade_t;
            if state.submenu_fade_t >= 1.0 {
                state.submenu_transition = SubmenuTransition::None;
                state.submenu_fade_t = 0.0;
                state.content_alpha = 1.0;
            }
        }
        SubmenuTransition::FadeOutToMain => {
            let leaving_graphics =
                matches!(state.view, OptionsView::Submenu(SubmenuKind::Graphics));
            let (
                desired_renderer,
                desired_display_mode,
                desired_resolution,
                desired_monitor,
                desired_vsync,
                desired_present_mode_policy,
                desired_max_fps,
            ) = if leaving_graphics {
                let vsync = state
                    .sub_choice_indices_graphics
                    .get(VSYNC_ROW_INDEX)
                    .copied()
                    .is_none_or(yes_no_from_choice);
                (
                    Some(selected_video_renderer(state)),
                    Some(selected_display_mode(state)),
                    Some(selected_resolution(state)),
                    Some(selected_display_monitor(state)),
                    Some(vsync),
                    Some(selected_present_mode_policy(state)),
                    Some(selected_max_fps(state)),
                )
            } else {
                (None, None, None, None, None, None, None)
            };
            let step = if SUBMENU_FADE_DURATION > 0.0 {
                dt / SUBMENU_FADE_DURATION
            } else {
                1.0
            };
            state.submenu_fade_t = (state.submenu_fade_t + step).min(1.0);
            state.content_alpha = 1.0 - state.submenu_fade_t;
            if state.submenu_fade_t >= 1.0 {
                // Return to the main options list and fade it in.
                state.view = OptionsView::Main;
                state.pending_submenu_kind = None;
                state.pending_submenu_parent_kind = None;
                state.submenu_parent_kind = None;
                state.cursor_initialized = false;
                state.cursor_t = 1.0;
                state.row_tweens.clear();
                state.graphics_prev_visible_rows.clear();
                state.advanced_prev_visible_rows.clear();
                state.select_music_prev_visible_rows.clear();
                state.nav_key_held_direction = None;
                state.nav_key_held_since = None;
                state.nav_key_last_scrolled_at = None;
                state.nav_lr_held_direction = None;
                state.nav_lr_held_since = None;
                state.nav_lr_last_adjusted_at = None;
                state.submenu_transition = SubmenuTransition::FadeInMain;
                state.submenu_fade_t = 0.0;
                state.content_alpha = 0.0;

                let mut renderer_change: Option<BackendType> = None;
                let mut display_mode_change: Option<DisplayMode> = None;
                let mut resolution_change: Option<(u32, u32)> = None;
                let mut monitor_change: Option<usize> = None;
                let mut vsync_change: Option<bool> = None;
                let mut present_mode_policy_change: Option<PresentModePolicy> = None;
                let mut max_fps_change: Option<u16> = None;

                if let Some(renderer) = desired_renderer
                    && renderer != state.video_renderer_at_load
                {
                    renderer_change = Some(renderer);
                }
                if let Some(display_mode) = desired_display_mode
                    && display_mode != state.display_mode_at_load
                {
                    display_mode_change = Some(display_mode);
                }
                if let Some(monitor) = desired_monitor
                    && monitor != state.display_monitor_at_load
                {
                    monitor_change = Some(monitor);
                }
                if let Some((w, h)) = desired_resolution
                    && (w != state.display_width_at_load || h != state.display_height_at_load)
                {
                    resolution_change = Some((w, h));
                }
                if let Some(vsync) = desired_vsync
                    && vsync != state.vsync_at_load
                {
                    vsync_change = Some(vsync);
                }
                if let Some(policy) = desired_present_mode_policy
                    && policy != state.present_mode_policy_at_load
                {
                    present_mode_policy_change = Some(policy);
                }
                if let Some(max_fps) = desired_max_fps
                    && max_fps != state.max_fps_at_load
                {
                    max_fps_change = Some(max_fps);
                }

                if renderer_change.is_some()
                    || display_mode_change.is_some()
                    || monitor_change.is_some()
                    || resolution_change.is_some()
                    || vsync_change.is_some()
                    || present_mode_policy_change.is_some()
                    || max_fps_change.is_some()
                {
                    pending_action = Some(ScreenAction::ChangeGraphics {
                        renderer: renderer_change,
                        display_mode: display_mode_change,
                        monitor: monitor_change,
                        resolution: resolution_change,
                        vsync: vsync_change,
                        present_mode_policy: present_mode_policy_change,
                        max_fps: max_fps_change,
                    });
                }
            }
        }
        SubmenuTransition::FadeInMain => {
            let step = if SUBMENU_FADE_DURATION > 0.0 {
                dt / SUBMENU_FADE_DURATION
            } else {
                1.0
            };
            state.submenu_fade_t = (state.submenu_fade_t + step).min(1.0);
            state.content_alpha = state.submenu_fade_t;
            if state.submenu_fade_t >= 1.0 {
                state.submenu_transition = SubmenuTransition::None;
                state.submenu_fade_t = 0.0;
                state.content_alpha = 1.0;
            }
        }
    }

    // While fading, freeze hold-to-scroll to avoid odd jumps.
    if !matches!(state.submenu_transition, SubmenuTransition::None) {
        return pending_action;
    }

    if let (Some(direction), Some(held_since), Some(last_scrolled_at)) = (
        state.nav_key_held_direction,
        state.nav_key_held_since,
        state.nav_key_last_scrolled_at,
    ) {
        let now = Instant::now();
        if now.duration_since(held_since) > NAV_INITIAL_HOLD_DELAY
            && now.duration_since(last_scrolled_at) >= NAV_REPEAT_SCROLL_INTERVAL
        {
            match state.view {
                OptionsView::Main => {
                    let total = ITEMS.len();
                    if total > 0 {
                        match direction {
                            NavDirection::Up => {
                                state.selected = if state.selected == 0 {
                                    total - 1
                                } else {
                                    state.selected - 1
                                };
                            }
                            NavDirection::Down => {
                                state.selected = (state.selected + 1) % total;
                            }
                        }
                        state.nav_key_last_scrolled_at = Some(now);
                    }
                }
                OptionsView::Submenu(kind) => {
                    move_submenu_selection_vertical(state, asset_manager, kind, direction);
                    state.nav_key_last_scrolled_at = Some(now);
                }
            }
        }
    }

    if let (Some(delta_lr), Some(held_since), Some(last_adjusted)) = (
        state.nav_lr_held_direction,
        state.nav_lr_held_since,
        state.nav_lr_last_adjusted_at,
    ) {
        let now = Instant::now();
        if now.duration_since(held_since) > NAV_INITIAL_HOLD_DELAY
            && now.duration_since(last_adjusted) >= NAV_REPEAT_SCROLL_INTERVAL
            && matches!(state.view, OptionsView::Submenu(_))
        {
            if pending_action.is_none() {
                pending_action = apply_submenu_choice_delta(state, asset_manager, delta_lr);
            } else {
                apply_submenu_choice_delta(state, asset_manager, delta_lr);
            }
            state.nav_lr_last_adjusted_at = Some(now);
        }
    }

    match state.view {
        OptionsView::Main => {
            if state.selected != state.prev_selected {
                audio::play_sfx("assets/sounds/change.ogg");
                state.prev_selected = state.selected;
            }
        }
        OptionsView::Submenu(_) => {
            if state.sub_selected != state.sub_prev_selected {
                audio::play_sfx("assets/sounds/change.ogg");
                state.sub_prev_selected = state.sub_selected;
            }
        }
    }

    let (s, list_x, list_y) = scaled_block_origin_with_margins();
    match state.view {
        OptionsView::Main => {
            update_row_tweens(
                &mut state.row_tweens,
                ITEMS.len(),
                state.selected,
                s,
                list_y,
                dt,
            );
            state.cursor_initialized = false;
            state.graphics_prev_visible_rows.clear();
            state.advanced_prev_visible_rows.clear();
            state.select_music_prev_visible_rows.clear();
        }
        OptionsView::Submenu(kind) => {
            if matches!(kind, SubmenuKind::Graphics) {
                update_graphics_row_tweens(state, s, list_y, dt);
                state.advanced_prev_visible_rows.clear();
                state.select_music_prev_visible_rows.clear();
            } else if matches!(kind, SubmenuKind::Advanced) {
                update_advanced_row_tweens(state, s, list_y, dt);
                state.graphics_prev_visible_rows.clear();
                state.select_music_prev_visible_rows.clear();
            } else if matches!(kind, SubmenuKind::SelectMusic) {
                update_select_music_row_tweens(state, s, list_y, dt);
                state.graphics_prev_visible_rows.clear();
                state.advanced_prev_visible_rows.clear();
            } else {
                let total_rows = submenu_total_rows(state, kind);
                update_row_tweens(
                    &mut state.row_tweens,
                    total_rows,
                    state.sub_selected,
                    s,
                    list_y,
                    dt,
                );
                state.graphics_prev_visible_rows.clear();
                state.advanced_prev_visible_rows.clear();
                state.select_music_prev_visible_rows.clear();
            }
            let list_w = list_w_unscaled() * s;
            if let Some((to_x, to_y, to_w, to_h)) =
                submenu_cursor_dest(state, asset_manager, kind, s, list_x, list_y, list_w)
            {
                let needs_cursor_init = !state.cursor_initialized;
                if needs_cursor_init {
                    state.cursor_initialized = true;
                    state.cursor_from_x = to_x;
                    state.cursor_from_y = to_y;
                    state.cursor_from_w = to_w;
                    state.cursor_from_h = to_h;
                    state.cursor_to_x = to_x;
                    state.cursor_to_y = to_y;
                    state.cursor_to_w = to_w;
                    state.cursor_to_h = to_h;
                    state.cursor_t = 1.0;
                } else {
                    let dx = (to_x - state.cursor_to_x).abs();
                    let dy = (to_y - state.cursor_to_y).abs();
                    let dw = (to_w - state.cursor_to_w).abs();
                    let dh = (to_h - state.cursor_to_h).abs();
                    if dx > 0.01 || dy > 0.01 || dw > 0.01 || dh > 0.01 {
                        let t = state.cursor_t.clamp(0.0, 1.0);
                        let cur_x = (state.cursor_to_x - state.cursor_from_x)
                            .mul_add(t, state.cursor_from_x);
                        let cur_y = (state.cursor_to_y - state.cursor_from_y)
                            .mul_add(t, state.cursor_from_y);
                        let cur_w = (state.cursor_to_w - state.cursor_from_w)
                            .mul_add(t, state.cursor_from_w);
                        let cur_h = (state.cursor_to_h - state.cursor_from_h)
                            .mul_add(t, state.cursor_from_h);
                        state.cursor_from_x = cur_x;
                        state.cursor_from_y = cur_y;
                        state.cursor_from_w = cur_w;
                        state.cursor_from_h = cur_h;
                        state.cursor_to_x = to_x;
                        state.cursor_to_y = to_y;
                        state.cursor_to_w = to_w;
                        state.cursor_to_h = to_h;
                        state.cursor_t = 0.0;
                    }
                }
            } else {
                state.cursor_initialized = false;
            }
        }
    }

    if state.cursor_t < 1.0 {
        if CURSOR_TWEEN_SECONDS > 0.0 {
            state.cursor_t = (state.cursor_t + dt / CURSOR_TWEEN_SECONDS).min(1.0);
        } else {
            state.cursor_t = 1.0;
        }
    }

    pending_action
}

// Small helpers to let the app dispatcher manage hold-to-scroll without exposing fields
pub fn on_nav_press(state: &mut State, dir: NavDirection) {
    state.nav_key_held_direction = Some(dir);
    state.nav_key_held_since = Some(Instant::now());
    state.nav_key_last_scrolled_at = Some(Instant::now());
}

pub fn on_nav_release(state: &mut State, dir: NavDirection) {
    if state.nav_key_held_direction == Some(dir) {
        state.nav_key_held_direction = None;
        state.nav_key_held_since = None;
        state.nav_key_last_scrolled_at = None;
    }
}

fn on_lr_press(state: &mut State, delta: isize) {
    let now = Instant::now();
    state.nav_lr_held_direction = Some(delta);
    state.nav_lr_held_since = Some(now);
    state.nav_lr_last_adjusted_at = Some(now);
}

fn on_lr_release(state: &mut State, delta: isize) {
    if state.nav_lr_held_direction == Some(delta) {
        state.nav_lr_held_direction = None;
        state.nav_lr_held_since = None;
        state.nav_lr_last_adjusted_at = None;
    }
}

fn apply_submenu_choice_delta(
    state: &mut State,
    asset_manager: &AssetManager,
    delta: isize,
) -> Option<ScreenAction> {
    if !matches!(state.submenu_transition, SubmenuTransition::None) {
        return None;
    }
    let kind = match state.view {
        OptionsView::Submenu(k) => k,
        _ => return None,
    };
    let rows = submenu_rows(kind);
    if rows.is_empty() {
        return None;
    }
    let Some(row_index) = submenu_visible_row_to_actual(state, kind, state.sub_selected) else {
        // Exit row – no choices to change.
        return None;
    };

    if let Some(row) = rows.get(row_index) {
        // Block cycling disabled rows (e.g. dedicated menu buttons when unmapped).
        if is_submenu_row_disabled(kind, row.id) {
            return None;
        }
        if matches!(kind, SubmenuKind::Sound) {
            match row.id {
                SubRowId::MasterVolume => {
                    if adjust_ms_value(
                        &mut state.master_volume_pct,
                        delta,
                        VOLUME_MIN_PERCENT,
                        VOLUME_MAX_PERCENT,
                    ) {
                        config::update_master_volume(state.master_volume_pct as u8);
                        audio::play_sfx("assets/sounds/change_value.ogg");
                        clear_render_cache(state);
                    }
                    return None;
                }
                SubRowId::SfxVolume => {
                    if adjust_ms_value(
                        &mut state.sfx_volume_pct,
                        delta,
                        VOLUME_MIN_PERCENT,
                        VOLUME_MAX_PERCENT,
                    ) {
                        config::update_sfx_volume(state.sfx_volume_pct as u8);
                        audio::play_sfx("assets/sounds/change_value.ogg");
                        clear_render_cache(state);
                    }
                    return None;
                }
                SubRowId::AssistTickVolume => {
                    if adjust_ms_value(
                        &mut state.assist_tick_volume_pct,
                        delta,
                        VOLUME_MIN_PERCENT,
                        VOLUME_MAX_PERCENT,
                    ) {
                        config::update_assist_tick_volume(state.assist_tick_volume_pct as u8);
                        audio::play_sfx("assets/sounds/change_value.ogg");
                        clear_render_cache(state);
                    }
                    return None;
                }
                SubRowId::MusicVolume => {
                    if adjust_ms_value(
                        &mut state.music_volume_pct,
                        delta,
                        VOLUME_MIN_PERCENT,
                        VOLUME_MAX_PERCENT,
                    ) {
                        config::update_music_volume(state.music_volume_pct as u8);
                        audio::play_sfx("assets/sounds/change_value.ogg");
                        clear_render_cache(state);
                    }
                    return None;
                }
                _ => {}
            }
        }
        if matches!(kind, SubmenuKind::Sound) && row.id == SubRowId::GlobalOffset {
            if adjust_ms_value(
                &mut state.global_offset_ms,
                delta,
                GLOBAL_OFFSET_MIN_MS,
                GLOBAL_OFFSET_MAX_MS,
            ) {
                config::update_global_offset(state.global_offset_ms as f32 / 1000.0);
                audio::play_sfx("assets/sounds/change_value.ogg");
                clear_render_cache(state);
            }
            return None;
        }
        if matches!(kind, SubmenuKind::Graphics) && row.id == SubRowId::VisualDelay {
            if adjust_ms_value(
                &mut state.visual_delay_ms,
                delta,
                VISUAL_DELAY_MIN_MS,
                VISUAL_DELAY_MAX_MS,
            ) {
                config::update_visual_delay_seconds(state.visual_delay_ms as f32 / 1000.0);
                audio::play_sfx("assets/sounds/change_value.ogg");
                clear_render_cache(state);
            }
            return None;
        }
        if matches!(kind, SubmenuKind::InputBackend) && row.id == SubRowId::Debounce {
            if adjust_ms_value(
                &mut state.input_debounce_ms,
                delta,
                INPUT_DEBOUNCE_MIN_MS,
                INPUT_DEBOUNCE_MAX_MS,
            ) {
                config::update_input_debounce_seconds(state.input_debounce_ms as f32 / 1000.0);
                audio::play_sfx("assets/sounds/change_value.ogg");
                clear_render_cache(state);
            }
            return None;
        }
        if matches!(kind, SubmenuKind::NullOrDieOptions) {
            match row.id {
                SubRowId::Fingerprint => {
                    if adjust_tenths_value(
                        &mut state.null_or_die_fingerprint_tenths,
                        delta,
                        NULL_OR_DIE_POSITIVE_MS_MIN_TENTHS,
                        NULL_OR_DIE_POSITIVE_MS_MAX_TENTHS,
                    ) {
                        config::update_null_or_die_fingerprint_ms(f64_from_tenths(
                            state.null_or_die_fingerprint_tenths,
                        ));
                        audio::play_sfx("assets/sounds/change_value.ogg");
                        clear_render_cache(state);
                    }
                    return None;
                }
                SubRowId::Window => {
                    if adjust_tenths_value(
                        &mut state.null_or_die_window_tenths,
                        delta,
                        NULL_OR_DIE_POSITIVE_MS_MIN_TENTHS,
                        NULL_OR_DIE_POSITIVE_MS_MAX_TENTHS,
                    ) {
                        config::update_null_or_die_window_ms(f64_from_tenths(
                            state.null_or_die_window_tenths,
                        ));
                        audio::play_sfx("assets/sounds/change_value.ogg");
                        clear_render_cache(state);
                    }
                    return None;
                }
                SubRowId::Step => {
                    if adjust_tenths_value(
                        &mut state.null_or_die_step_tenths,
                        delta,
                        NULL_OR_DIE_POSITIVE_MS_MIN_TENTHS,
                        NULL_OR_DIE_POSITIVE_MS_MAX_TENTHS,
                    ) {
                        config::update_null_or_die_step_ms(f64_from_tenths(
                            state.null_or_die_step_tenths,
                        ));
                        audio::play_sfx("assets/sounds/change_value.ogg");
                        clear_render_cache(state);
                    }
                    return None;
                }
                SubRowId::MagicOffset => {
                    if adjust_tenths_value(
                        &mut state.null_or_die_magic_offset_tenths,
                        delta,
                        NULL_OR_DIE_MAGIC_OFFSET_MIN_TENTHS,
                        NULL_OR_DIE_MAGIC_OFFSET_MAX_TENTHS,
                    ) {
                        config::update_null_or_die_magic_offset_ms(f64_from_tenths(
                            state.null_or_die_magic_offset_tenths,
                        ));
                        audio::play_sfx("assets/sounds/change_value.ogg");
                        clear_render_cache(state);
                    }
                    return None;
                }
                _ => {}
            }
        }
    }

    let choices = row_choices(state, kind, rows, row_index);
    let num_choices = choices.len();
    if num_choices == 0 {
        return None;
    }
    let mut action: Option<ScreenAction> = None;
    if row_index >= submenu_choice_indices(state, kind).len()
        || row_index >= submenu_cursor_indices(state, kind).len()
    {
        return None;
    }
    let choice_index =
        submenu_cursor_indices(state, kind)[row_index].min(num_choices.saturating_sub(1));
    let cur = choice_index as isize;
    let n = num_choices as isize;
    let mut new_index = ((cur + delta).rem_euclid(n)) as usize;
    if new_index >= num_choices {
        new_index = num_choices.saturating_sub(1);
    }
    if new_index == choice_index {
        return None;
    }
    let selected_choice = choices
        .get(new_index)
        .map(|choice| choice.as_ref().to_string());
    drop(choices);

    submenu_choice_indices_mut(state, kind)[row_index] = new_index;
    submenu_cursor_indices_mut(state, kind)[row_index] = new_index;
    if let Some(layout) = submenu_row_layout(state, asset_manager, kind, row_index)
        && layout.inline_row
        && let Some(&x) = layout.centers.get(new_index)
    {
        state.sub_inline_x = x;
    }
    audio::play_sfx("assets/sounds/change_value.ogg");

    if matches!(kind, SubmenuKind::System) {
        let row = &rows[row_index];
        match row.id {
            SubRowId::Game => config::update_game_flag(config::GameFlag::Dance),
            SubRowId::Theme => config::update_theme_flag(config::ThemeFlag::SimplyLove),
            SubRowId::Language => {
                let flag = language_flag_from_choice(new_index);
                config::update_language_flag(flag);
                assets::i18n::set_locale(&assets::i18n::resolve_locale(flag));
            }
            SubRowId::LogLevel => config::update_log_level(log_level_from_choice(new_index)),
            SubRowId::LogFile => config::update_log_to_file(new_index == 1),
            SubRowId::DefaultNoteSkin => {
                if let Some(skin_name) = selected_choice.as_deref() {
                    profile::update_machine_default_noteskin(profile::NoteSkin::new(skin_name));
                }
            }
            _ => {}
        }
    } else if matches!(kind, SubmenuKind::Graphics) {
        let row = &rows[row_index];
        if row.id == SubRowId::DisplayAspectRatio {
            let (cur_w, cur_h) = selected_resolution(state);
            rebuild_resolution_choices(state, cur_w, cur_h);
        }
        if row.id == SubRowId::DisplayResolution {
            rebuild_refresh_rate_choices(state);
        }
        if row.id == SubRowId::DisplayMode {
            let (cur_w, cur_h) = selected_resolution(state);
            rebuild_resolution_choices(state, cur_w, cur_h);
        }
        if row.id == SubRowId::RefreshRate && state.max_fps_at_load == 0 && !max_fps_enabled(state)
        {
            seed_max_fps_value_choice(state, 0);
        }
        if row.id == SubRowId::MaxFps && yes_no_from_choice(new_index) && state.max_fps_at_load == 0
        {
            seed_max_fps_value_choice(state, 0);
        }
        if row.id == SubRowId::ShowStats {
            let mode = new_index.min(3) as u8;
            action = Some(ScreenAction::UpdateShowOverlay(mode));
        }
        if row.id == SubRowId::ValidationLayers {
            config::update_gfx_debug(yes_no_from_choice(new_index));
        }
        if row.id == SubRowId::SoftwareRendererThreads {
            let threads = software_thread_from_choice(&state.software_thread_choices, new_index);
            config::update_software_renderer_threads(threads);
        }
    } else if matches!(kind, SubmenuKind::InputBackend) {
        let row = &rows[row_index];
        if row.id == SubRowId::GamepadBackend {
            #[cfg(target_os = "windows")]
            {
                config::update_windows_gamepad_backend(windows_backend_from_choice(new_index));
            }
        }
        if row.id == SubRowId::MenuNavigation {
            config::update_three_key_navigation(new_index == 1);
        }
        if row.id == SubRowId::OptionsNavigation {
            config::update_arcade_options_navigation(new_index == 1);
        }
        if row.id == SubRowId::MenuButtons {
            state.pending_dedicated_menu_buttons = Some(new_index == 1);
        }
    } else if matches!(kind, SubmenuKind::Machine) {
        let row = &rows[row_index];
        let enabled = new_index == 1;
        match row.id {
            SubRowId::SelectProfile => config::update_machine_show_select_profile(enabled),
            SubRowId::SelectColor => config::update_machine_show_select_color(enabled),
            SubRowId::SelectStyle => config::update_machine_show_select_style(enabled),
            SubRowId::PreferredStyle => config::update_machine_preferred_style(
                machine_preferred_style_from_choice(new_index),
            ),
            SubRowId::SelectPlayMode => config::update_machine_show_select_play_mode(enabled),
            SubRowId::PreferredMode => config::update_machine_preferred_play_mode(
                machine_preferred_mode_from_choice(new_index),
            ),
            SubRowId::EvalSummary => config::update_machine_show_eval_summary(enabled),
            SubRowId::NameEntry => config::update_machine_show_name_entry(enabled),
            SubRowId::GameoverScreen => config::update_machine_show_gameover(enabled),
            SubRowId::MenuMusic => config::update_menu_music(enabled),
            SubRowId::Replays => config::update_machine_enable_replays(enabled),
            SubRowId::PerPlayerGlobalOffsets => {
                config::update_machine_allow_per_player_global_offsets(enabled)
            }
            SubRowId::KeyboardFeatures => config::update_keyboard_features(enabled),
            SubRowId::VideoBgs => config::update_show_video_backgrounds(enabled),
            SubRowId::WriteCurrentScreen => config::update_write_current_screen(enabled),
            _ => {}
        }
    } else if matches!(kind, SubmenuKind::Advanced) {
        let row = &rows[row_index];
        if row.id == SubRowId::DefaultFailType {
            config::update_default_fail_type(default_fail_type_from_choice(new_index));
        } else if row.id == SubRowId::BannerCache {
            config::update_banner_cache(new_index == 1);
        } else if row.id == SubRowId::CdTitleCache {
            config::update_cdtitle_cache(new_index == 1);
        } else if row.id == SubRowId::SongParsingThreads {
            let threads = software_thread_from_choice(&state.software_thread_choices, new_index);
            config::update_song_parsing_threads(threads);
        } else if row.id == SubRowId::CacheSongs {
            config::update_cache_songs(new_index == 1);
        } else if row.id == SubRowId::FastLoad {
            config::update_fastload(new_index == 1);
        }
    } else if matches!(kind, SubmenuKind::NullOrDieOptions) {
        let row = &rows[row_index];
        if row.id == SubRowId::SyncGraph {
            config::update_null_or_die_sync_graph(sync_graph_mode_from_choice(new_index));
        } else if row.id == SubRowId::SyncConfidence {
            config::update_null_or_die_confidence_percent(sync_confidence_from_choice(new_index));
        } else if row.id == SubRowId::PackSyncThreads {
            let threads = software_thread_from_choice(&state.software_thread_choices, new_index);
            config::update_null_or_die_pack_sync_threads(threads);
        } else if row.id == SubRowId::KernelTarget {
            config::update_null_or_die_kernel_target(null_or_die_kernel_target_from_choice(
                new_index,
            ));
        } else if row.id == SubRowId::KernelType {
            config::update_null_or_die_kernel_type(null_or_die_kernel_type_from_choice(new_index));
        } else if row.id == SubRowId::FullSpectrogram {
            config::update_null_or_die_full_spectrogram(yes_no_from_choice(new_index));
        }
    } else if matches!(kind, SubmenuKind::Course) {
        let row = &rows[row_index];
        let enabled = yes_no_from_choice(new_index);
        match row.id {
            SubRowId::ShowRandomCourses => config::update_show_random_courses(enabled),
            SubRowId::ShowMostPlayed => config::update_show_most_played_courses(enabled),
            SubRowId::ShowIndividualScores => config::update_show_course_individual_scores(enabled),
            SubRowId::AutosubmitIndividual => {
                config::update_autosubmit_course_scores_individually(enabled)
            }
            _ => {}
        }
    } else if matches!(kind, SubmenuKind::Gameplay) {
        let row = &rows[row_index];
        if row.id == SubRowId::BgBrightness {
            config::update_bg_brightness(bg_brightness_from_choice(new_index));
        } else if row.id == SubRowId::CenteredP1Notefield {
            config::update_center_1player_notefield(new_index == 1);
        } else if row.id == SubRowId::ZmodRatingBox {
            config::update_zmod_rating_box_text(new_index == 1);
        } else if row.id == SubRowId::BpmDecimal {
            config::update_show_bpm_decimal(new_index == 1);
        }
    } else if matches!(kind, SubmenuKind::Sound) {
        let row = &rows[row_index];
        match row.id {
            SubRowId::MasterVolume => {
                let vol = master_volume_from_choice(new_index);
                config::update_master_volume(vol);
            }
            SubRowId::SfxVolume => {
                let vol = master_volume_from_choice(new_index);
                config::update_sfx_volume(vol);
            }
            SubRowId::AssistTickVolume => {
                let vol = master_volume_from_choice(new_index);
                config::update_assist_tick_volume(vol);
            }
            SubRowId::MusicVolume => {
                let vol = master_volume_from_choice(new_index);
                config::update_music_volume(vol);
            }
            SubRowId::SoundDevice => {
                let device = sound_device_from_choice(state, new_index);
                config::update_audio_output_device(device);
                let current_rate = config::get().audio_sample_rate_hz;
                let rate_choice = sample_rate_choice_index(state, current_rate);
                if current_rate.is_some() && rate_choice == 0 {
                    config::update_audio_sample_rate(None);
                }
                set_sound_choice_index(state, SubRowId::AudioSampleRate, rate_choice);
            }
            SubRowId::AudioOutputMode => {
                config::update_audio_output_mode(audio_output_mode_from_choice(new_index));
                #[cfg(target_os = "linux")]
                set_sound_choice_index(state, SubRowId::AlsaExclusive, 0);
            }
            #[cfg(target_os = "linux")]
            SubRowId::LinuxAudioBackend => {
                let backend = linux_audio_backend_from_choice(state, new_index);
                config::update_linux_audio_backend(backend);
                if matches!(backend, config::LinuxAudioBackend::Alsa) {
                    set_sound_choice_index(
                        state,
                        SubRowId::AlsaExclusive,
                        alsa_exclusive_choice_index(config::get().audio_output_mode),
                    );
                } else {
                    if matches!(
                        config::get().audio_output_mode,
                        config::AudioOutputMode::Exclusive
                    ) {
                        config::update_audio_output_mode(selected_audio_output_mode(state));
                    }
                    set_sound_choice_index(state, SubRowId::AlsaExclusive, 0);
                }
            }
            #[cfg(target_os = "linux")]
            SubRowId::AlsaExclusive => {
                let mode = if new_index == 1 {
                    config::AudioOutputMode::Exclusive
                } else {
                    selected_audio_output_mode(state)
                };
                config::update_audio_output_mode(mode);
            }
            SubRowId::AudioSampleRate => {
                let rate = sample_rate_from_choice(state, new_index);
                config::update_audio_sample_rate(rate);
            }
            SubRowId::MineSounds => {
                config::update_mine_hit_sound(new_index == 1);
            }
            SubRowId::RateModPreservesPitch => {
                config::update_rate_mod_preserves_pitch(new_index == 1);
            }
            _ => {}
        }
    } else if matches!(kind, SubmenuKind::SelectMusic) {
        let row = &rows[row_index];
        if row.id == SubRowId::ShowBanners {
            config::update_show_select_music_banners(yes_no_from_choice(new_index));
        } else if row.id == SubRowId::ShowVideoBanners {
            config::update_show_select_music_video_banners(yes_no_from_choice(new_index));
        } else if row.id == SubRowId::ShowBreakdown {
            config::update_show_select_music_breakdown(yes_no_from_choice(new_index));
        } else if row.id == SubRowId::BreakdownStyle {
            config::update_select_music_breakdown_style(breakdown_style_from_choice(new_index));
        } else if row.id == SubRowId::ShowNativeLanguage {
            config::update_translated_titles(translated_titles_from_choice(new_index));
        } else if row.id == SubRowId::MusicWheelSpeed {
            config::update_music_wheel_switch_speed(music_wheel_scroll_speed_from_choice(
                new_index,
            ));
        } else if row.id == SubRowId::MusicWheelStyle {
            config::update_select_music_wheel_style(select_music_wheel_style_from_choice(
                new_index,
            ));
        } else if row.id == SubRowId::ShowCdTitles {
            config::update_show_select_music_cdtitles(yes_no_from_choice(new_index));
        } else if row.id == SubRowId::ShowWheelGrades {
            config::update_show_music_wheel_grades(yes_no_from_choice(new_index));
        } else if row.id == SubRowId::ShowWheelLamps {
            config::update_show_music_wheel_lamps(yes_no_from_choice(new_index));
        } else if row.id == SubRowId::ItlWheelData {
            config::update_select_music_itl_wheel_mode(select_music_itl_wheel_from_choice(
                new_index,
            ));
        } else if row.id == SubRowId::NewPackBadge {
            config::update_select_music_new_pack_mode(new_pack_mode_from_choice(new_index));
        } else if row.id == SubRowId::ShowPatternInfo {
            config::update_select_music_pattern_info_mode(select_music_pattern_info_from_choice(
                new_index,
            ));
        } else if row.id == SubRowId::MusicPreviews {
            config::update_show_select_music_previews(yes_no_from_choice(new_index));
        } else if row.id == SubRowId::PreviewMarker {
            config::update_show_select_music_preview_marker(yes_no_from_choice(new_index));
        } else if row.id == SubRowId::LoopMusic {
            config::update_select_music_preview_loop(new_index == 1);
        } else if row.id == SubRowId::ShowGameplayTimer {
            config::update_show_select_music_gameplay_timer(yes_no_from_choice(new_index));
        } else if row.id == SubRowId::ShowGsBox {
            config::update_show_select_music_scorebox(yes_no_from_choice(new_index));
        } else if row.id == SubRowId::GsBoxPlacement {
            config::update_select_music_scorebox_placement(
                select_music_scorebox_placement_from_choice(new_index),
            );
        }
    } else if matches!(kind, SubmenuKind::GrooveStats) {
        let row = &rows[row_index];
        if row.id == SubRowId::EnableGrooveStats {
            let enabled = yes_no_from_choice(new_index);
            config::update_enable_groovestats(enabled);
            // Re-run connectivity logic so toggling this option applies immediately.
            crate::game::online::init();
        } else if row.id == SubRowId::EnableBoogieStats {
            config::update_enable_boogiestats(yes_no_from_choice(new_index));
            crate::game::online::init();
        } else if row.id == SubRowId::GsSubmitFails {
            config::update_submit_groovestats_fails(yes_no_from_choice(new_index));
        } else if row.id == SubRowId::AutoPopulateScores {
            config::update_auto_populate_gs_scores(yes_no_from_choice(new_index));
        } else if row.id == SubRowId::AutoDownloadUnlocks {
            config::update_auto_download_unlocks(yes_no_from_choice(new_index));
        } else if row.id == SubRowId::SeparateUnlocksByPlayer {
            config::update_separate_unlocks_by_player(yes_no_from_choice(new_index));
        }
    } else if matches!(kind, SubmenuKind::ArrowCloud) {
        let row = &rows[row_index];
        if row.id == SubRowId::EnableArrowCloud {
            config::update_enable_arrowcloud(yes_no_from_choice(new_index));
            crate::game::online::init();
        } else if row.id == SubRowId::ArrowCloudSubmitFails {
            config::update_submit_arrowcloud_fails(yes_no_from_choice(new_index));
        }
    } else if matches!(kind, SubmenuKind::ScoreImport) {
        let row = &rows[row_index];
        if row.id == SubRowId::ScoreImportEndpoint {
            refresh_score_import_profile_options(state);
        }
    }
    clear_render_cache(state);
    action
}

fn cancel_current_view(state: &mut State) -> ScreenAction {
    match state.view {
        OptionsView::Main => ScreenAction::Navigate(Screen::Menu),
        OptionsView::Submenu(_) => {
            if let Some(parent_kind) = state.submenu_parent_kind {
                state.pending_submenu_kind = Some(parent_kind);
                state.pending_submenu_parent_kind = None;
                state.submenu_transition = SubmenuTransition::FadeOutToSubmenu;
            } else {
                state.submenu_transition = SubmenuTransition::FadeOutToMain;
            }
            state.submenu_fade_t = 0.0;
            ScreenAction::None
        }
    }
}

fn undo_three_key_selection(state: &mut State, asset_manager: &AssetManager) {
    match state.menu_lr_undo {
        1 => match state.view {
            OptionsView::Main => {
                let total = ITEMS.len();
                if total > 0 {
                    state.selected = (state.selected + 1) % total;
                }
            }
            OptionsView::Submenu(kind) => {
                move_submenu_selection_vertical(state, asset_manager, kind, NavDirection::Down);
            }
        },
        -1 => match state.view {
            OptionsView::Main => {
                let total = ITEMS.len();
                if total > 0 {
                    state.selected = if state.selected == 0 {
                        total - 1
                    } else {
                        state.selected - 1
                    };
                }
            }
            OptionsView::Submenu(kind) => {
                move_submenu_selection_vertical(state, asset_manager, kind, NavDirection::Up);
            }
        },
        _ => {}
    }
}

fn activate_current_selection(state: &mut State, asset_manager: &AssetManager) -> ScreenAction {
    match state.view {
        OptionsView::Main => {
            let total = ITEMS.len();
            if total == 0 {
                return ScreenAction::None;
            }
            let sel = state.selected.min(total - 1);
            let item = &ITEMS[sel];
            state.pending_submenu_parent_kind = None;

            match item.id {
                ItemId::SystemOptions => {
                    audio::play_sfx("assets/sounds/start.ogg");
                    state.pending_submenu_kind = Some(SubmenuKind::System);
                    state.submenu_transition = SubmenuTransition::FadeOutToSubmenu;
                    state.submenu_fade_t = 0.0;
                }
                ItemId::GraphicsOptions => {
                    audio::play_sfx("assets/sounds/start.ogg");
                    state.pending_submenu_kind = Some(SubmenuKind::Graphics);
                    state.submenu_transition = SubmenuTransition::FadeOutToSubmenu;
                    state.submenu_fade_t = 0.0;
                }
                ItemId::InputOptions => {
                    audio::play_sfx("assets/sounds/start.ogg");
                    state.pending_submenu_kind = Some(SubmenuKind::Input);
                    state.submenu_transition = SubmenuTransition::FadeOutToSubmenu;
                    state.submenu_fade_t = 0.0;
                }
                ItemId::MachineOptions => {
                    audio::play_sfx("assets/sounds/start.ogg");
                    state.pending_submenu_kind = Some(SubmenuKind::Machine);
                    state.submenu_transition = SubmenuTransition::FadeOutToSubmenu;
                    state.submenu_fade_t = 0.0;
                }
                ItemId::AdvancedOptions => {
                    audio::play_sfx("assets/sounds/start.ogg");
                    state.pending_submenu_kind = Some(SubmenuKind::Advanced);
                    state.submenu_transition = SubmenuTransition::FadeOutToSubmenu;
                    state.submenu_fade_t = 0.0;
                }
                ItemId::CourseOptions => {
                    audio::play_sfx("assets/sounds/start.ogg");
                    state.pending_submenu_kind = Some(SubmenuKind::Course);
                    state.submenu_transition = SubmenuTransition::FadeOutToSubmenu;
                    state.submenu_fade_t = 0.0;
                }
                ItemId::GameplayOptions => {
                    audio::play_sfx("assets/sounds/start.ogg");
                    state.pending_submenu_kind = Some(SubmenuKind::Gameplay);
                    state.submenu_transition = SubmenuTransition::FadeOutToSubmenu;
                    state.submenu_fade_t = 0.0;
                }
                ItemId::SoundOptions => {
                    audio::play_sfx("assets/sounds/start.ogg");
                    state.pending_submenu_kind = Some(SubmenuKind::Sound);
                    state.submenu_transition = SubmenuTransition::FadeOutToSubmenu;
                    state.submenu_fade_t = 0.0;
                }
                ItemId::SelectMusicOptions => {
                    audio::play_sfx("assets/sounds/start.ogg");
                    state.pending_submenu_kind = Some(SubmenuKind::SelectMusic);
                    state.submenu_transition = SubmenuTransition::FadeOutToSubmenu;
                    state.submenu_fade_t = 0.0;
                }
                ItemId::OnlineScoreServices => {
                    audio::play_sfx("assets/sounds/start.ogg");
                    state.pending_submenu_kind = Some(SubmenuKind::OnlineScoring);
                    state.submenu_transition = SubmenuTransition::FadeOutToSubmenu;
                    state.submenu_fade_t = 0.0;
                }
                ItemId::NullOrDieOptions => {
                    audio::play_sfx("assets/sounds/start.ogg");
                    refresh_null_or_die_options(state);
                    state.pending_submenu_kind = Some(SubmenuKind::NullOrDie);
                    state.submenu_transition = SubmenuTransition::FadeOutToSubmenu;
                    state.submenu_fade_t = 0.0;
                }
                ItemId::ManageLocalProfiles => {
                    audio::play_sfx("assets/sounds/start.ogg");
                    return ScreenAction::Navigate(Screen::ManageLocalProfiles);
                }
                ItemId::ReloadSongsCourses => {
                    audio::play_sfx("assets/sounds/start.ogg");
                    start_reload_songs_and_courses(state);
                }
                ItemId::Credits => {
                    audio::play_sfx("assets/sounds/start.ogg");
                    return ScreenAction::NavigateNoFade(Screen::Credits);
                }
                ItemId::Exit => {
                    audio::play_sfx("assets/sounds/start.ogg");
                    return ScreenAction::Navigate(Screen::Menu);
                }
                _ => {}
            }
            ScreenAction::None
        }
        OptionsView::Submenu(kind) => {
            let total = submenu_total_rows(state, kind);
            if total == 0 {
                return ScreenAction::None;
            }
            let selected_row = state.sub_selected.min(total.saturating_sub(1));
            if matches!(kind, SubmenuKind::SelectMusic)
                && let Some(row_idx) = submenu_visible_row_to_actual(state, kind, selected_row)
            {
                let rows = submenu_rows(kind);
                let row_id = rows.get(row_idx).map(|row| row.id);
                if row_id == Some(SubRowId::GsBoxLeaderboards) {
                    let choice_idx = submenu_cursor_indices(state, kind)
                        .get(row_idx)
                        .copied()
                        .unwrap_or(0)
                        .min(SELECT_MUSIC_SCOREBOX_CYCLE_NUM_CHOICES.saturating_sub(1));
                    toggle_select_music_scorebox_cycle_option(state, choice_idx);
                    return ScreenAction::None;
                } else if row_id == Some(SubRowId::ChartInfo) {
                    let choice_idx = submenu_cursor_indices(state, kind)
                        .get(row_idx)
                        .copied()
                        .unwrap_or(0)
                        .min(SELECT_MUSIC_CHART_INFO_NUM_CHOICES.saturating_sub(1));
                    toggle_select_music_chart_info_option(state, choice_idx);
                    return ScreenAction::None;
                }
            }
            if matches!(kind, SubmenuKind::Gameplay)
                && let Some(row_idx) = submenu_visible_row_to_actual(state, kind, selected_row)
            {
                let rows = submenu_rows(kind);
                if rows.get(row_idx).map(|row| row.id) == Some(SubRowId::AutoScreenshot) {
                    let choice_idx = submenu_cursor_indices(state, kind)
                        .get(row_idx)
                        .copied()
                        .unwrap_or(0)
                        .min(config::AUTO_SS_NUM_FLAGS.saturating_sub(1));
                    toggle_auto_screenshot_option(state, choice_idx);
                    return ScreenAction::None;
                }
            }
            if selected_row == total - 1 {
                audio::play_sfx("assets/sounds/start.ogg");
                if let Some(parent_kind) = state.submenu_parent_kind {
                    state.pending_submenu_kind = Some(parent_kind);
                    state.pending_submenu_parent_kind = None;
                    state.submenu_transition = SubmenuTransition::FadeOutToSubmenu;
                } else {
                    state.submenu_transition = SubmenuTransition::FadeOutToMain;
                }
                state.submenu_fade_t = 0.0;
                return ScreenAction::None;
            }
            if matches!(kind, SubmenuKind::Input) {
                let rows = submenu_rows(kind);
                let Some(row_idx) = submenu_visible_row_to_actual(state, kind, selected_row) else {
                    return ScreenAction::None;
                };
                if let Some(row) = rows.get(row_idx) {
                    match row.id {
                        SubRowId::ConfigureMappings => {
                            audio::play_sfx("assets/sounds/start.ogg");
                            return ScreenAction::Navigate(Screen::Mappings);
                        }
                        SubRowId::TestInput => {
                            audio::play_sfx("assets/sounds/start.ogg");
                            return ScreenAction::Navigate(Screen::Input);
                        }
                        SubRowId::InputOptions => {
                            audio::play_sfx("assets/sounds/start.ogg");
                            state.pending_submenu_kind = Some(SubmenuKind::InputBackend);
                            state.pending_submenu_parent_kind = Some(SubmenuKind::Input);
                            state.submenu_transition = SubmenuTransition::FadeOutToSubmenu;
                            state.submenu_fade_t = 0.0;
                            return ScreenAction::None;
                        }
                        _ => {}
                    }
                }
            } else if matches!(kind, SubmenuKind::OnlineScoring) {
                let rows = submenu_rows(kind);
                let Some(row_idx) = submenu_visible_row_to_actual(state, kind, selected_row) else {
                    return ScreenAction::None;
                };
                if let Some(row) = rows.get(row_idx) {
                    match row.id {
                        SubRowId::GsBsOptions => {
                            audio::play_sfx("assets/sounds/start.ogg");
                            state.pending_submenu_kind = Some(SubmenuKind::GrooveStats);
                            state.pending_submenu_parent_kind = Some(SubmenuKind::OnlineScoring);
                            state.submenu_transition = SubmenuTransition::FadeOutToSubmenu;
                            state.submenu_fade_t = 0.0;
                            return ScreenAction::None;
                        }
                        SubRowId::ArrowCloudOptions => {
                            audio::play_sfx("assets/sounds/start.ogg");
                            state.pending_submenu_kind = Some(SubmenuKind::ArrowCloud);
                            state.pending_submenu_parent_kind = Some(SubmenuKind::OnlineScoring);
                            state.submenu_transition = SubmenuTransition::FadeOutToSubmenu;
                            state.submenu_fade_t = 0.0;
                            return ScreenAction::None;
                        }
                        SubRowId::ScoreImport => {
                            audio::play_sfx("assets/sounds/start.ogg");
                            refresh_score_import_options(state);
                            state.pending_submenu_kind = Some(SubmenuKind::ScoreImport);
                            state.pending_submenu_parent_kind = Some(SubmenuKind::OnlineScoring);
                            state.submenu_transition = SubmenuTransition::FadeOutToSubmenu;
                            state.submenu_fade_t = 0.0;
                            return ScreenAction::None;
                        }
                        _ => {}
                    }
                }
            } else if matches!(kind, SubmenuKind::NullOrDie) {
                let rows = submenu_rows(kind);
                let Some(row_idx) = submenu_visible_row_to_actual(state, kind, selected_row) else {
                    return ScreenAction::None;
                };
                if let Some(row) = rows.get(row_idx) {
                    match row.id {
                        SubRowId::NullOrDieOptions => {
                            audio::play_sfx("assets/sounds/start.ogg");
                            state.pending_submenu_kind = Some(SubmenuKind::NullOrDieOptions);
                            state.pending_submenu_parent_kind = Some(SubmenuKind::NullOrDie);
                            state.submenu_transition = SubmenuTransition::FadeOutToSubmenu;
                            state.submenu_fade_t = 0.0;
                            return ScreenAction::None;
                        }
                        SubRowId::SyncPacks => {
                            audio::play_sfx("assets/sounds/start.ogg");
                            refresh_sync_pack_options(state);
                            state.pending_submenu_kind = Some(SubmenuKind::SyncPacks);
                            state.pending_submenu_parent_kind = Some(SubmenuKind::NullOrDie);
                            state.submenu_transition = SubmenuTransition::FadeOutToSubmenu;
                            state.submenu_fade_t = 0.0;
                            return ScreenAction::None;
                        }
                        _ => {}
                    }
                }
            } else if matches!(kind, SubmenuKind::ScoreImport) {
                let rows = submenu_rows(kind);
                let Some(row_idx) = submenu_visible_row_to_actual(state, kind, selected_row) else {
                    return ScreenAction::None;
                };
                if let Some(row) = rows.get(row_idx)
                    && row.id == SubRowId::ScoreImportStart
                {
                    audio::play_sfx("assets/sounds/start.ogg");
                    if let Some(selection) = selected_score_import_selection(state) {
                        if selection.pack_group.is_none() {
                            clear_navigation_holds(state);
                            state.score_import_confirm = Some(ScoreImportConfirmState {
                                selection,
                                active_choice: 1,
                            });
                        } else {
                            begin_score_import(state, selection);
                        }
                    } else {
                        log::warn!(
                            "Score import start requested, but no eligible profile is selected."
                        );
                    }
                    return ScreenAction::None;
                }
            } else if matches!(kind, SubmenuKind::SyncPacks) {
                let rows = submenu_rows(kind);
                let Some(row_idx) = submenu_visible_row_to_actual(state, kind, selected_row) else {
                    return ScreenAction::None;
                };
                if let Some(row) = rows.get(row_idx)
                    && row.id == SubRowId::SyncPackStart
                {
                    audio::play_sfx("assets/sounds/start.ogg");
                    let selection = selected_sync_pack_selection(state);
                    if selection.pack_group.is_none() {
                        clear_navigation_holds(state);
                        state.sync_pack_confirm = Some(SyncPackConfirmState {
                            selection,
                            active_choice: 1,
                        });
                    } else {
                        begin_pack_sync(state, selection);
                    }
                    return ScreenAction::None;
                }
            }
            if screen_input::dedicated_three_key_nav_enabled()
                && let Some(action) = apply_submenu_choice_delta(state, asset_manager, 1)
            {
                return action;
            }
            ScreenAction::None
        }
    }
}

pub fn handle_input(
    state: &mut State,
    asset_manager: &AssetManager,
    ev: &InputEvent,
) -> ScreenAction {
    if state.reload_ui.is_some() {
        return ScreenAction::None;
    }
    let three_key_action = screen_input::three_key_menu_action(&mut state.menu_lr_chord, ev);
    if screen_input::dedicated_three_key_nav_enabled() {
        match ev.action {
            VirtualAction::p1_left
            | VirtualAction::p1_menu_left
            | VirtualAction::p2_left
            | VirtualAction::p2_menu_left
                if !ev.pressed =>
            {
                state.menu_lr_undo = 0;
                on_nav_release(state, NavDirection::Up);
                return ScreenAction::None;
            }
            VirtualAction::p1_right
            | VirtualAction::p1_menu_right
            | VirtualAction::p2_right
            | VirtualAction::p2_menu_right
                if !ev.pressed =>
            {
                state.menu_lr_undo = 0;
                on_nav_release(state, NavDirection::Down);
                return ScreenAction::None;
            }
            _ => {}
        }
    }
    if let Some(score_import) = state.score_import_ui.as_ref() {
        let cancel_requested = matches!(
            three_key_action,
            Some((_, screen_input::ThreeKeyMenuAction::Cancel))
        ) || (ev.pressed
            && matches!(ev.action, VirtualAction::p1_back | VirtualAction::p2_back));
        if cancel_requested {
            score_import.cancel_requested.store(true, Ordering::Relaxed);
            clear_navigation_holds(state);
            state.score_import_ui = None;
            audio::play_sfx("assets/sounds/change.ogg");
            log::warn!("Score import cancel requested by user.");
        }
        return ScreenAction::None;
    }
    if !matches!(
        state.pack_sync_overlay,
        shared_pack_sync::OverlayState::Hidden
    ) {
        return shared_pack_sync::handle_input(&mut state.pack_sync_overlay, ev);
    }
    if let Some(confirm) = state.score_import_confirm.as_mut() {
        if let Some((_, nav)) = three_key_action {
            match nav {
                screen_input::ThreeKeyMenuAction::Prev => {
                    if confirm.active_choice > 0 {
                        confirm.active_choice -= 1;
                        audio::play_sfx("assets/sounds/change.ogg");
                    }
                }
                screen_input::ThreeKeyMenuAction::Next => {
                    if confirm.active_choice < 1 {
                        confirm.active_choice += 1;
                        audio::play_sfx("assets/sounds/change.ogg");
                    }
                }
                screen_input::ThreeKeyMenuAction::Confirm => {
                    let should_start = confirm.active_choice == 0;
                    audio::play_sfx("assets/sounds/start.ogg");
                    if should_start {
                        clear_navigation_holds(state);
                        begin_score_import_from_confirm(state);
                    } else {
                        clear_navigation_holds(state);
                        state.score_import_confirm = None;
                    }
                }
                screen_input::ThreeKeyMenuAction::Cancel => {
                    clear_navigation_holds(state);
                    state.score_import_confirm = None;
                    audio::play_sfx("assets/sounds/change.ogg");
                }
            }
            return ScreenAction::None;
        }
        if !ev.pressed {
            return ScreenAction::None;
        }
        match ev.action {
            VirtualAction::p1_left
            | VirtualAction::p1_menu_left
            | VirtualAction::p2_left
            | VirtualAction::p2_menu_left => {
                if confirm.active_choice > 0 {
                    confirm.active_choice -= 1;
                    audio::play_sfx("assets/sounds/change.ogg");
                }
            }
            VirtualAction::p1_right
            | VirtualAction::p1_menu_right
            | VirtualAction::p2_right
            | VirtualAction::p2_menu_right => {
                if confirm.active_choice < 1 {
                    confirm.active_choice += 1;
                    audio::play_sfx("assets/sounds/change.ogg");
                }
            }
            VirtualAction::p1_start
            | VirtualAction::p1_select
            | VirtualAction::p2_start
            | VirtualAction::p2_select => {
                let should_start = confirm.active_choice == 0;
                audio::play_sfx("assets/sounds/start.ogg");
                if should_start {
                    clear_navigation_holds(state);
                    begin_score_import_from_confirm(state);
                } else {
                    clear_navigation_holds(state);
                    state.score_import_confirm = None;
                }
            }
            VirtualAction::p1_back | VirtualAction::p2_back => {
                clear_navigation_holds(state);
                state.score_import_confirm = None;
                audio::play_sfx("assets/sounds/change.ogg");
            }
            _ => {}
        }
        return ScreenAction::None;
    }
    if let Some(confirm) = state.sync_pack_confirm.as_mut() {
        if let Some((_, nav)) = three_key_action {
            match nav {
                screen_input::ThreeKeyMenuAction::Prev => {
                    if confirm.active_choice > 0 {
                        confirm.active_choice -= 1;
                        audio::play_sfx("assets/sounds/change.ogg");
                    }
                }
                screen_input::ThreeKeyMenuAction::Next => {
                    if confirm.active_choice < 1 {
                        confirm.active_choice += 1;
                        audio::play_sfx("assets/sounds/change.ogg");
                    }
                }
                screen_input::ThreeKeyMenuAction::Confirm => {
                    let should_start = confirm.active_choice == 0;
                    audio::play_sfx("assets/sounds/start.ogg");
                    clear_navigation_holds(state);
                    if should_start {
                        begin_pack_sync_from_confirm(state);
                    } else {
                        state.sync_pack_confirm = None;
                    }
                }
                screen_input::ThreeKeyMenuAction::Cancel => {
                    clear_navigation_holds(state);
                    state.sync_pack_confirm = None;
                    audio::play_sfx("assets/sounds/change.ogg");
                }
            }
            return ScreenAction::None;
        }
        if !ev.pressed {
            return ScreenAction::None;
        }
        match ev.action {
            VirtualAction::p1_left
            | VirtualAction::p1_menu_left
            | VirtualAction::p2_left
            | VirtualAction::p2_menu_left => {
                if confirm.active_choice > 0 {
                    confirm.active_choice -= 1;
                    audio::play_sfx("assets/sounds/change.ogg");
                }
            }
            VirtualAction::p1_right
            | VirtualAction::p1_menu_right
            | VirtualAction::p2_right
            | VirtualAction::p2_menu_right => {
                if confirm.active_choice < 1 {
                    confirm.active_choice += 1;
                    audio::play_sfx("assets/sounds/change.ogg");
                }
            }
            VirtualAction::p1_start
            | VirtualAction::p1_select
            | VirtualAction::p2_start
            | VirtualAction::p2_select => {
                let should_start = confirm.active_choice == 0;
                audio::play_sfx("assets/sounds/start.ogg");
                clear_navigation_holds(state);
                if should_start {
                    begin_pack_sync_from_confirm(state);
                } else {
                    state.sync_pack_confirm = None;
                }
            }
            VirtualAction::p1_back | VirtualAction::p2_back => {
                clear_navigation_holds(state);
                state.sync_pack_confirm = None;
                audio::play_sfx("assets/sounds/change.ogg");
            }
            _ => {}
        }
        return ScreenAction::None;
    }
    // Ignore new navigation while a local submenu fade is in progress.
    if !matches!(state.submenu_transition, SubmenuTransition::None) {
        return ScreenAction::None;
    }
    if let Some((_, nav)) = three_key_action {
        return match nav {
            screen_input::ThreeKeyMenuAction::Prev => {
                match state.view {
                    OptionsView::Main => {
                        let total = ITEMS.len();
                        if total > 0 {
                            state.selected = if state.selected == 0 {
                                total - 1
                            } else {
                                state.selected - 1
                            };
                        }
                    }
                    OptionsView::Submenu(kind) => {
                        move_submenu_selection_vertical(
                            state,
                            asset_manager,
                            kind,
                            NavDirection::Up,
                        );
                    }
                }
                on_nav_press(state, NavDirection::Up);
                state.menu_lr_undo = 1;
                ScreenAction::None
            }
            screen_input::ThreeKeyMenuAction::Next => {
                match state.view {
                    OptionsView::Main => {
                        let total = ITEMS.len();
                        if total > 0 {
                            state.selected = (state.selected + 1) % total;
                        }
                    }
                    OptionsView::Submenu(kind) => {
                        move_submenu_selection_vertical(
                            state,
                            asset_manager,
                            kind,
                            NavDirection::Down,
                        );
                    }
                }
                on_nav_press(state, NavDirection::Down);
                state.menu_lr_undo = -1;
                ScreenAction::None
            }
            screen_input::ThreeKeyMenuAction::Confirm => {
                state.menu_lr_undo = 0;
                clear_navigation_holds(state);
                activate_current_selection(state, asset_manager)
            }
            screen_input::ThreeKeyMenuAction::Cancel => {
                undo_three_key_selection(state, asset_manager);
                state.menu_lr_undo = 0;
                clear_navigation_holds(state);
                cancel_current_view(state)
            }
        };
    }

    match ev.action {
        VirtualAction::p1_back | VirtualAction::p2_back if ev.pressed => {
            return cancel_current_view(state);
        }
        VirtualAction::p1_up
        | VirtualAction::p1_menu_up
        | VirtualAction::p2_up
        | VirtualAction::p2_menu_up => {
            if ev.pressed {
                match state.view {
                    OptionsView::Main => {
                        let total = ITEMS.len();
                        if total > 0 {
                            state.selected = if state.selected == 0 {
                                total - 1
                            } else {
                                state.selected - 1
                            };
                        }
                    }
                    OptionsView::Submenu(kind) => {
                        move_submenu_selection_vertical(
                            state,
                            asset_manager,
                            kind,
                            NavDirection::Up,
                        );
                    }
                }
                on_nav_press(state, NavDirection::Up);
            } else {
                on_nav_release(state, NavDirection::Up);
            }
        }
        VirtualAction::p1_down
        | VirtualAction::p1_menu_down
        | VirtualAction::p2_down
        | VirtualAction::p2_menu_down => {
            if ev.pressed {
                match state.view {
                    OptionsView::Main => {
                        let total = ITEMS.len();
                        if total > 0 {
                            state.selected = (state.selected + 1) % total;
                        }
                    }
                    OptionsView::Submenu(kind) => {
                        move_submenu_selection_vertical(
                            state,
                            asset_manager,
                            kind,
                            NavDirection::Down,
                        );
                    }
                }
                on_nav_press(state, NavDirection::Down);
            } else {
                on_nav_release(state, NavDirection::Down);
            }
        }
        VirtualAction::p1_left
        | VirtualAction::p1_menu_left
        | VirtualAction::p2_left
        | VirtualAction::p2_menu_left => {
            if ev.pressed {
                if let Some(action) = apply_submenu_choice_delta(state, asset_manager, -1) {
                    on_lr_press(state, -1);
                    return action;
                }
                on_lr_press(state, -1);
            } else {
                on_lr_release(state, -1);
            }
        }
        VirtualAction::p1_right
        | VirtualAction::p1_menu_right
        | VirtualAction::p2_right
        | VirtualAction::p2_menu_right => {
            if ev.pressed {
                if let Some(action) = apply_submenu_choice_delta(state, asset_manager, 1) {
                    on_lr_press(state, 1);
                    return action;
                }
                on_lr_press(state, 1);
            } else {
                on_lr_release(state, 1);
            }
        }
        VirtualAction::p1_start | VirtualAction::p2_start if ev.pressed => {
            return activate_current_selection(state, asset_manager);
        }
        _ => {}
    }
    ScreenAction::None
}

/* --------------------------------- layout -------------------------------- */

/// content rect = full screen minus top & bottom bars.
/// We fit the (rows + separator + description) block inside that content rect,
/// honoring LEFT, RIGHT and TOP margins in *screen pixels*.
/// Returns (scale, `origin_x`, `origin_y`).
fn scaled_block_origin_with_margins() -> (f32, f32, f32) {
    let total_w = list_w_unscaled() + SEP_W + desc_w_unscaled();
    let total_h = DESC_H;

    let sw = screen_width();
    let sh = screen_height();

    // content area (between bars)
    let content_top = BAR_H;
    let content_bottom = sh - BAR_H;
    let content_h = (content_bottom - content_top).max(0.0);

    // available width between fixed left/right gutters
    let avail_w = (sw - LEFT_MARGIN_PX - RIGHT_MARGIN_PX).max(0.0);
    // available height after the fixed top margin (inside content area),
    // and before an adjustable bottom margin.
    let avail_h = (content_h - FIRST_ROW_TOP_MARGIN_PX - BOTTOM_MARGIN_PX).max(0.0);

    // candidate scales
    let s_w = if total_w > 0.0 {
        avail_w / total_w
    } else {
        1.0
    };
    let s_h = if total_h > 0.0 {
        avail_h / total_h
    } else {
        1.0
    };
    let s = s_w.min(s_h).max(0.0);

    // X origin:
    // Right-align inside [LEFT..(sw-RIGHT)] so the description box ends exactly
    // RIGHT_MARGIN_PX from the screen edge.
    let ox = LEFT_MARGIN_PX + total_w.mul_add(-s, avail_w).max(0.0);

    // Y origin is fixed under the top bar by the requested margin.
    let oy = content_top + FIRST_ROW_TOP_MARGIN_PX;

    (s, ox, oy)
}

#[inline(always)]
fn scroll_offset(selected: usize, total_rows: usize) -> usize {
    let anchor_row: usize = 4; // keep cursor near middle (5th visible row)
    let max_offset = total_rows.saturating_sub(VISIBLE_ROWS);
    if total_rows <= VISIBLE_ROWS {
        0
    } else {
        selected.saturating_sub(anchor_row).min(max_offset)
    }
}

#[inline(always)]
fn row_dest_for_index(
    total_rows: usize,
    selected: usize,
    row_idx: usize,
    s: f32,
    list_y: f32,
) -> (f32, f32) {
    if total_rows == 0 {
        return (list_y, 0.0);
    }
    let offset = scroll_offset(selected.min(total_rows - 1), total_rows);
    let row_step = (ROW_H + ROW_GAP) * s;
    let first_row_mid_y = (0.5 * ROW_H).mul_add(s, list_y);
    let top_hidden_mid_y = first_row_mid_y - 0.5 * row_step;
    let bottom_hidden_mid_y = ((VISIBLE_ROWS as f32) - 0.5).mul_add(row_step, first_row_mid_y);
    if row_idx < offset {
        (top_hidden_mid_y, 0.0)
    } else if row_idx >= offset + VISIBLE_ROWS {
        (bottom_hidden_mid_y, 0.0)
    } else {
        let vis = row_idx - offset;
        ((vis as f32).mul_add(row_step, first_row_mid_y), 1.0)
    }
}

fn init_row_tweens(total_rows: usize, selected: usize, s: f32, list_y: f32) -> Vec<RowTween> {
    let mut out: Vec<RowTween> = Vec::with_capacity(total_rows);
    for row_idx in 0..total_rows {
        let (y, a) = row_dest_for_index(total_rows, selected, row_idx, s, list_y);
        out.push(RowTween {
            from_y: y,
            to_y: y,
            from_a: a,
            to_a: a,
            t: 1.0,
        });
    }
    out
}

fn update_row_tweens(
    row_tweens: &mut Vec<RowTween>,
    total_rows: usize,
    selected: usize,
    s: f32,
    list_y: f32,
    dt: f32,
) {
    if total_rows == 0 {
        row_tweens.clear();
        return;
    }
    if row_tweens.len() != total_rows {
        *row_tweens = init_row_tweens(total_rows, selected, s, list_y);
        return;
    }
    for (row_idx, tw) in row_tweens.iter_mut().enumerate().take(total_rows) {
        let (to_y, to_a) = row_dest_for_index(total_rows, selected, row_idx, s, list_y);
        let cur_y = tw.y();
        let cur_a = tw.a();
        if (to_y - tw.to_y).abs() > 0.01 || (to_a - tw.to_a).abs() > 0.001 {
            tw.from_y = cur_y;
            tw.to_y = to_y;
            tw.from_a = cur_a;
            tw.to_a = to_a;
            tw.t = 0.0;
        }
        if tw.t < 1.0 {
            if ROW_TWEEN_SECONDS > 0.0 {
                tw.t = (tw.t + dt / ROW_TWEEN_SECONDS).min(1.0);
            } else {
                tw.t = 1.0;
            }
        }
    }
}

fn update_graphics_row_tweens(state: &mut State, s: f32, list_y: f32, dt: f32) {
    let rows = submenu_rows(SubmenuKind::Graphics);
    let visible_rows = submenu_visible_row_indices(state, SubmenuKind::Graphics, rows);
    let total_rows = visible_rows.len() + 1;
    if total_rows == 0 {
        state.row_tweens.clear();
        state.graphics_prev_visible_rows.clear();
        return;
    }

    let selected = state.sub_selected.min(total_rows.saturating_sub(1));
    let visibility_changed = state.graphics_prev_visible_rows != visible_rows;
    if state.row_tweens.is_empty() {
        state.row_tweens = init_row_tweens(total_rows, selected, s, list_y);
    } else if state.row_tweens.len() != total_rows || visibility_changed {
        let old_tweens = std::mem::take(&mut state.row_tweens);
        let old_visible_rows = state.graphics_prev_visible_rows.clone();
        let old_total_rows = old_visible_rows.len() + 1;

        let parent_from = old_visible_rows
            .iter()
            .position(|&idx| idx == VIDEO_RENDERER_ROW_INDEX)
            .and_then(|old_idx| old_tweens.get(old_idx))
            .map(|tw| (tw.y(), tw.a()))
            .unwrap_or_else(|| {
                row_dest_for_index(total_rows, selected, VIDEO_RENDERER_ROW_INDEX, s, list_y)
            });
        let old_exit_from = old_tweens
            .get(old_total_rows.saturating_sub(1))
            .map(|tw| (tw.y(), tw.a()));

        let mut mapped: Vec<RowTween> = Vec::with_capacity(total_rows);
        for (new_idx, actual_idx) in visible_rows.iter().copied().enumerate() {
            let (to_y, to_a) = row_dest_for_index(total_rows, selected, new_idx, s, list_y);
            let (from_y, from_a) = old_visible_rows
                .iter()
                .position(|&old_actual| old_actual == actual_idx)
                .and_then(|old_idx| old_tweens.get(old_idx).map(|tw| (tw.y(), tw.a())))
                .or({
                    if actual_idx == SOFTWARE_THREADS_ROW_INDEX {
                        Some((parent_from.0, 0.0))
                    } else {
                        None
                    }
                })
                .unwrap_or((to_y, to_a));
            let t = if (to_y - from_y).abs() <= 0.01 && (to_a - from_a).abs() <= 0.001 {
                1.0
            } else {
                0.0
            };
            mapped.push(RowTween {
                from_y,
                to_y,
                from_a,
                to_a,
                t,
            });
        }

        let exit_idx = total_rows.saturating_sub(1);
        let (to_y, to_a) = row_dest_for_index(total_rows, selected, exit_idx, s, list_y);
        let (from_y, from_a) = old_exit_from.unwrap_or((to_y, to_a));
        let t = if (to_y - from_y).abs() <= 0.01 && (to_a - from_a).abs() <= 0.001 {
            1.0
        } else {
            0.0
        };
        mapped.push(RowTween {
            from_y,
            to_y,
            from_a,
            to_a,
            t,
        });
        state.row_tweens = mapped;
    }

    state.graphics_prev_visible_rows = visible_rows;
    update_row_tweens(&mut state.row_tweens, total_rows, selected, s, list_y, dt);
}

const fn advanced_parent_row(actual_idx: usize) -> Option<usize> {
    let _ = actual_idx;
    None
}

fn update_advanced_row_tweens(state: &mut State, s: f32, list_y: f32, dt: f32) {
    let rows = submenu_rows(SubmenuKind::Advanced);
    let visible_rows = submenu_visible_row_indices(state, SubmenuKind::Advanced, rows);
    let total_rows = visible_rows.len() + 1;
    if total_rows == 0 {
        state.row_tweens.clear();
        state.advanced_prev_visible_rows.clear();
        return;
    }

    let selected = state.sub_selected.min(total_rows.saturating_sub(1));
    let visibility_changed = state.advanced_prev_visible_rows != visible_rows;
    if state.row_tweens.is_empty() {
        state.row_tweens = init_row_tweens(total_rows, selected, s, list_y);
    } else if state.row_tweens.len() != total_rows || visibility_changed {
        let old_tweens = std::mem::take(&mut state.row_tweens);
        let old_visible_rows = state.advanced_prev_visible_rows.clone();
        let old_total_rows = old_visible_rows.len() + 1;
        let old_exit_from = old_tweens
            .get(old_total_rows.saturating_sub(1))
            .map(|tw| (tw.y(), tw.a()));

        let mut mapped: Vec<RowTween> = Vec::with_capacity(total_rows);
        for (new_idx, actual_idx) in visible_rows.iter().copied().enumerate() {
            let (to_y, to_a) = row_dest_for_index(total_rows, selected, new_idx, s, list_y);
            let parent_from = advanced_parent_row(actual_idx).and_then(|parent_actual_idx| {
                old_visible_rows
                    .iter()
                    .position(|&idx| idx == parent_actual_idx)
                    .and_then(|old_idx| old_tweens.get(old_idx))
                    .map(|tw| (tw.y(), 0.0))
            });
            let (from_y, from_a) = old_visible_rows
                .iter()
                .position(|&old_actual| old_actual == actual_idx)
                .and_then(|old_idx| old_tweens.get(old_idx).map(|tw| (tw.y(), tw.a())))
                .or(parent_from)
                .unwrap_or((to_y, to_a));
            let t = if (to_y - from_y).abs() <= 0.01 && (to_a - from_a).abs() <= 0.001 {
                1.0
            } else {
                0.0
            };
            mapped.push(RowTween {
                from_y,
                to_y,
                from_a,
                to_a,
                t,
            });
        }

        let exit_idx = total_rows.saturating_sub(1);
        let (to_y, to_a) = row_dest_for_index(total_rows, selected, exit_idx, s, list_y);
        let (from_y, from_a) = old_exit_from.unwrap_or((to_y, to_a));
        let t = if (to_y - from_y).abs() <= 0.01 && (to_a - from_a).abs() <= 0.001 {
            1.0
        } else {
            0.0
        };
        mapped.push(RowTween {
            from_y,
            to_y,
            from_a,
            to_a,
            t,
        });
        state.row_tweens = mapped;
    }

    state.advanced_prev_visible_rows = visible_rows;
    update_row_tweens(&mut state.row_tweens, total_rows, selected, s, list_y, dt);
}

const fn select_music_parent_row(actual_idx: usize) -> Option<usize> {
    match actual_idx {
        SELECT_MUSIC_SHOW_VIDEO_BANNERS_ROW_INDEX => Some(SELECT_MUSIC_SHOW_BANNERS_ROW_INDEX),
        SELECT_MUSIC_BREAKDOWN_STYLE_ROW_INDEX => Some(SELECT_MUSIC_SHOW_BREAKDOWN_ROW_INDEX),
        SELECT_MUSIC_PREVIEW_LOOP_ROW_INDEX => Some(SELECT_MUSIC_MUSIC_PREVIEWS_ROW_INDEX),
        SELECT_MUSIC_SCOREBOX_PLACEMENT_ROW_INDEX => Some(SELECT_MUSIC_SHOW_SCOREBOX_ROW_INDEX),
        SELECT_MUSIC_SCOREBOX_CYCLE_ROW_INDEX => Some(SELECT_MUSIC_SHOW_SCOREBOX_ROW_INDEX),
        _ => None,
    }
}

fn update_select_music_row_tweens(state: &mut State, s: f32, list_y: f32, dt: f32) {
    let rows = submenu_rows(SubmenuKind::SelectMusic);
    let visible_rows = submenu_visible_row_indices(state, SubmenuKind::SelectMusic, rows);
    let total_rows = visible_rows.len() + 1;
    if total_rows == 0 {
        state.row_tweens.clear();
        state.select_music_prev_visible_rows.clear();
        return;
    }

    let selected = state.sub_selected.min(total_rows.saturating_sub(1));
    let visibility_changed = state.select_music_prev_visible_rows != visible_rows;
    if state.row_tweens.is_empty() {
        state.row_tweens = init_row_tweens(total_rows, selected, s, list_y);
    } else if state.row_tweens.len() != total_rows || visibility_changed {
        let old_tweens = std::mem::take(&mut state.row_tweens);
        let old_visible_rows = state.select_music_prev_visible_rows.clone();
        let old_total_rows = old_visible_rows.len() + 1;
        let old_exit_from = old_tweens
            .get(old_total_rows.saturating_sub(1))
            .map(|tw| (tw.y(), tw.a()));

        let mut mapped: Vec<RowTween> = Vec::with_capacity(total_rows);
        for (new_idx, actual_idx) in visible_rows.iter().copied().enumerate() {
            let (to_y, to_a) = row_dest_for_index(total_rows, selected, new_idx, s, list_y);
            let parent_from = select_music_parent_row(actual_idx).and_then(|parent_actual_idx| {
                old_visible_rows
                    .iter()
                    .position(|&idx| idx == parent_actual_idx)
                    .and_then(|old_idx| old_tweens.get(old_idx))
                    .map(|tw| (tw.y(), 0.0))
            });
            let (from_y, from_a) = old_visible_rows
                .iter()
                .position(|&old_actual| old_actual == actual_idx)
                .and_then(|old_idx| old_tweens.get(old_idx).map(|tw| (tw.y(), tw.a())))
                .or(parent_from)
                .unwrap_or((to_y, to_a));
            let t = if (to_y - from_y).abs() <= 0.01 && (to_a - from_a).abs() <= 0.001 {
                1.0
            } else {
                0.0
            };
            mapped.push(RowTween {
                from_y,
                to_y,
                from_a,
                to_a,
                t,
            });
        }

        let exit_idx = total_rows.saturating_sub(1);
        let (to_y, to_a) = row_dest_for_index(total_rows, selected, exit_idx, s, list_y);
        let (from_y, from_a) = old_exit_from.unwrap_or((to_y, to_a));
        let t = if (to_y - from_y).abs() <= 0.01 && (to_a - from_a).abs() <= 0.001 {
            1.0
        } else {
            0.0
        };
        mapped.push(RowTween {
            from_y,
            to_y,
            from_a,
            to_a,
            t,
        });
        state.row_tweens = mapped;
    }

    state.select_music_prev_visible_rows = visible_rows;
    update_row_tweens(&mut state.row_tweens, total_rows, selected, s, list_y, dt);
}

#[inline(always)]
fn measure_text_box(asset_manager: &AssetManager, text: &str, zoom: f32) -> (f32, f32) {
    let mut out_w = 1.0_f32;
    let mut out_h = 16.0_f32;
    asset_manager.with_fonts(|all_fonts| {
        asset_manager.with_font("miso", |metrics_font| {
            out_h = (metrics_font.height as f32).max(1.0) * zoom;
            let mut w = font::measure_line_width_logical(metrics_font, text, all_fonts) as f32;
            if !w.is_finite() || w <= 0.0 {
                w = 1.0;
            }
            out_w = w * zoom;
        });
    });
    (out_w, out_h)
}

#[inline(always)]
fn ring_size_for_text(draw_w: f32, text_h: f32) -> (f32, f32) {
    let pad_y = widescale(6.0, 8.0);
    let min_pad_x = widescale(2.0, 3.0);
    let max_pad_x = widescale(22.0, 28.0);
    let width_ref = widescale(180.0, 220.0);
    let border_w = widescale(2.0, 2.5);
    let mut size_t = draw_w / width_ref;
    if !size_t.is_finite() {
        size_t = 0.0;
    }
    size_t = size_t.clamp(0.0, 1.0);
    let mut pad_x = (max_pad_x - min_pad_x).mul_add(size_t, min_pad_x);
    let max_pad_by_spacing = (INLINE_SPACING - border_w).max(min_pad_x);
    if pad_x > max_pad_by_spacing {
        pad_x = max_pad_by_spacing;
    }
    (draw_w + pad_x * 2.0, text_h + pad_y * 2.0)
}

#[inline(always)]
fn row_mid_y_for_cursor(
    state: &State,
    row_idx: usize,
    total_rows: usize,
    selected: usize,
    s: f32,
    list_y: f32,
) -> f32 {
    state
        .row_tweens
        .get(row_idx)
        .map(|tw| tw.to_y)
        .unwrap_or_else(|| row_dest_for_index(total_rows, selected, row_idx, s, list_y).0)
}

#[inline(always)]
fn wrap_miso_text(
    asset_manager: &AssetManager,
    raw_text: &str,
    max_width_px: f32,
    zoom: f32,
) -> String {
    asset_manager
        .with_fonts(|all_fonts| {
            asset_manager.with_font("miso", |miso_font| {
                let mut out = String::new();
                let mut is_first_output_line = true;

                for segment in raw_text.split('\n') {
                    let trimmed = segment.trim_end();
                    if trimmed.is_empty() {
                        if !is_first_output_line {
                            out.push('\n');
                        }
                        continue;
                    }

                    let mut current_line = String::new();
                    for word in trimmed.split_whitespace() {
                        let candidate = if current_line.is_empty() {
                            word.to_owned()
                        } else {
                            let mut tmp = current_line.clone();
                            tmp.push(' ');
                            tmp.push_str(word);
                            tmp
                        };

                        let logical_w =
                            font::measure_line_width_logical(miso_font, &candidate, all_fonts)
                                as f32;
                        if !current_line.is_empty() && logical_w * zoom > max_width_px {
                            if !is_first_output_line {
                                out.push('\n');
                            }
                            out.push_str(&current_line);
                            is_first_output_line = false;
                            current_line.clear();
                            current_line.push_str(word);
                        } else {
                            current_line = candidate;
                        }
                    }

                    if !current_line.is_empty() {
                        if !is_first_output_line {
                            out.push('\n');
                        }
                        out.push_str(&current_line);
                        is_first_output_line = false;
                    }
                }

                if out.is_empty() {
                    raw_text.to_string()
                } else {
                    out
                }
            })
        })
        .unwrap_or_else(|| raw_text.to_string())
}

fn build_description_layout(
    asset_manager: &AssetManager,
    key: DescriptionCacheKey,
    item: &Item,
    s: f32,
) -> DescriptionLayout {
    let title_side_pad = DESC_TITLE_SIDE_PAD_PX * s;
    let wrap_extra_pad = desc_wrap_extra_pad_unscaled() * s;
    let title_max_width_px =
        desc_w_unscaled().mul_add(s, -((2.0 * title_side_pad) + wrap_extra_pad));
    let bullet_side_pad = DESC_BULLET_SIDE_PAD_PX * s;
    let bullet_max_width_px = desc_w_unscaled().mul_add(
        s,
        -((2.0 * bullet_side_pad) + (DESC_BULLET_INDENT_PX * s) + wrap_extra_pad),
    );

    let mut blocks = Vec::new();

    if item.help.is_empty() {
        // No help entries — show the item name as a paragraph fallback.
        let wrapped = wrap_miso_text(
            asset_manager,
            &item.name.get(),
            title_max_width_px,
            DESC_TITLE_ZOOM * s,
        );
        blocks.push(RenderedHelpBlock::Paragraph {
            line_count: wrapped.lines().count().max(1),
            text: Arc::from(wrapped),
        });
    } else {
        for entry in item.help {
            match entry {
                HelpEntry::Paragraph(lkey) => {
                    let raw = lkey.get();
                    let wrapped = wrap_miso_text(
                        asset_manager,
                        &raw,
                        title_max_width_px,
                        DESC_TITLE_ZOOM * s,
                    );
                    blocks.push(RenderedHelpBlock::Paragraph {
                        line_count: wrapped.lines().count().max(1),
                        text: Arc::from(wrapped),
                    });
                }
                HelpEntry::Bullet(lkey) => {
                    let resolved = lkey.get();
                    let trimmed = resolved.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let mut entry_str = String::with_capacity(trimmed.len() + 2);
                    entry_str.push('\u{2022}');
                    entry_str.push(' ');
                    entry_str.push_str(trimmed);
                    let wrapped = wrap_miso_text(
                        asset_manager,
                        &entry_str,
                        bullet_max_width_px,
                        DESC_BODY_ZOOM * s,
                    );
                    blocks.push(RenderedHelpBlock::Bullet {
                        line_count: wrapped.lines().count().max(1),
                        text: Arc::from(wrapped),
                    });
                }
            }
        }
    }

    DescriptionLayout { key, blocks }
}

fn description_layout(
    state: &State,
    asset_manager: &AssetManager,
    key: DescriptionCacheKey,
    item: &Item,
    s: f32,
) -> DescriptionLayout {
    if let Some(layout) = state.description_layout_cache.borrow().as_ref()
        && layout.key == key
    {
        return layout.clone();
    }
    let layout = build_description_layout(asset_manager, key, item, s);
    *state.description_layout_cache.borrow_mut() = Some(layout.clone());
    layout
}

pub fn clear_description_layout_cache(state: &State) {
    *state.description_layout_cache.borrow_mut() = None;
}

pub fn clear_render_cache(state: &State) {
    clear_submenu_row_layout_cache(state);
    clear_description_layout_cache(state);
}

fn submenu_cursor_dest(
    state: &State,
    asset_manager: &AssetManager,
    kind: SubmenuKind,
    s: f32,
    list_x: f32,
    list_y: f32,
    list_w: f32,
) -> Option<(f32, f32, f32, f32)> {
    if is_launcher_submenu(kind) {
        return None;
    }
    let rows = submenu_rows(kind);
    let total_rows = submenu_total_rows(state, kind);
    if total_rows == 0 {
        return None;
    }
    let selected_row = state.sub_selected.min(total_rows - 1);
    let row_mid_y = row_mid_y_for_cursor(state, selected_row, total_rows, selected_row, s, list_y);
    let value_zoom = 0.835_f32;
    let label_bg_w = SUB_LABEL_COL_W * s;
    let item_col_left = list_x + label_bg_w;
    let item_col_w = list_w - label_bg_w;
    let single_center_x =
        item_col_w.mul_add(0.5, item_col_left) + SUB_SINGLE_VALUE_CENTER_OFFSET * s;

    if selected_row == total_rows - 1 {
        let (draw_w, text_h) = measure_text_box(asset_manager, "Exit", value_zoom);
        let (ring_w, ring_h) = ring_size_for_text(draw_w, text_h);
        return Some((single_center_x, row_mid_y, ring_w, ring_h));
    }
    let row_idx = submenu_visible_row_to_actual(state, kind, selected_row)?;
    let row = &rows[row_idx];
    let layout = submenu_row_layout(state, asset_manager, kind, row_idx)?;
    if layout.texts.is_empty() {
        return None;
    }
    let selected_choice = submenu_cursor_indices(state, kind)
        .get(row_idx)
        .copied()
        .unwrap_or(0)
        .min(layout.texts.len().saturating_sub(1));

    let draw_w = layout.widths[selected_choice];
    let center_x = if row.inline && layout.inline_row {
        let choice_inner_left = SUB_INLINE_ITEMS_LEFT_PAD.mul_add(s, list_x + label_bg_w);
        choice_inner_left + layout.centers[selected_choice]
    } else {
        single_center_x
    };
    let (ring_w, ring_h) = ring_size_for_text(draw_w, layout.text_h);
    Some((center_x, row_mid_y, ring_w, ring_h))
}

/* -------------------------------- drawing -------------------------------- */

fn apply_alpha_to_actor(actor: &mut Actor, alpha: f32) {
    match actor {
        Actor::Sprite { tint, .. } => tint[3] *= alpha,
        Actor::Text { color, .. } => color[3] *= alpha,
        Actor::Mesh { vertices, .. } => {
            let mut out: Vec<crate::engine::gfx::MeshVertex> = Vec::with_capacity(vertices.len());
            for v in vertices.iter() {
                let mut c = v.color;
                c[3] *= alpha;
                out.push(crate::engine::gfx::MeshVertex {
                    pos: v.pos,
                    color: c,
                });
            }
            *vertices = std::sync::Arc::from(out);
        }
        Actor::TexturedMesh { tint, .. } => tint[3] *= alpha,
        Actor::Frame {
            background,
            children,
            ..
        } => {
            if let Some(actors::Background::Color(c)) = background {
                c[3] *= alpha;
            }
            for child in children {
                apply_alpha_to_actor(child, alpha);
            }
        }
        Actor::Camera { children, .. } => {
            for child in children {
                apply_alpha_to_actor(child, alpha);
            }
        }
        Actor::Shadow { color, child, .. } => {
            // Apply alpha to the shadow tint and recurse to the child.
            color[3] *= alpha;
            apply_alpha_to_actor(child, alpha);
        }
    }
}

fn build_yes_no_confirm_overlay(
    prompt_text: &str,
    active_choice: u8,
    active_color_index: i32,
) -> Vec<Actor> {
    let w = screen_width();
    let h = screen_height();
    let cx = w * 0.5;
    let cy = h * 0.5;
    let answer_y = cy + 118.0;
    let yes_x = cx - 100.0;
    let no_x = cx + 100.0;
    let cursor_x = [yes_x, no_x][active_choice.min(1) as usize];
    let cursor_color = color::simply_love_rgba(active_color_index);

    vec![
        act!(quad:
            align(0.0, 0.0):
            xy(0.0, 0.0):
            zoomto(w, h):
            diffuse(0.0, 0.0, 0.0, 0.9):
            z(700)
        ),
        act!(quad:
            align(0.5, 0.5):
            xy(cursor_x, answer_y):
            setsize(145.0, 40.0):
            diffuse(cursor_color[0], cursor_color[1], cursor_color[2], 1.0):
            z(701)
        ),
        act!(text:
            align(0.5, 0.5):
            xy(cx, cy - 65.0):
            font("miso"):
            zoom(0.95):
            maxwidth(w - 90.0):
            settext(prompt_text):
            diffuse(1.0, 1.0, 1.0, 1.0):
            z(702):
            horizalign(center)
        ),
        act!(text:
            align(0.5, 0.5):
            xy(yes_x, answer_y):
            font("wendy"):
            zoom(0.72):
            settext(tr("Common", "Yes")):
            diffuse(1.0, 1.0, 1.0, 1.0):
            z(702):
            horizalign(center)
        ),
        act!(text:
            align(0.5, 0.5):
            xy(no_x, answer_y):
            font("wendy"):
            zoom(0.72):
            settext(tr("Common", "No")):
            diffuse(1.0, 1.0, 1.0, 1.0):
            z(702):
            horizalign(center)
        ),
    ]
}

pub fn get_actors(
    state: &State,
    asset_manager: &AssetManager,
    alpha_multiplier: f32,
) -> Vec<Actor> {
    let mut actors: Vec<Actor> = Vec::with_capacity(320);
    let is_fading_submenu = !matches!(state.submenu_transition, SubmenuTransition::None);

    /* -------------------------- HEART BACKGROUND -------------------------- */
    actors.extend(state.bg.build(heart_bg::Params {
        active_color_index: state.active_color_index, // <-- CHANGED
        backdrop_rgba: [0.0, 0.0, 0.0, 1.0],
        // Keep hearts always visible for actor-only fades (Options/Menu/Mappings);
        // local submenu fades are handled via content_alpha on UI actors only.
        alpha_mul: 1.0,
    }));

    if alpha_multiplier <= 0.0 {
        return actors;
    }

    if let Some(reload) = &state.reload_ui {
        let mut ui_actors = build_reload_overlay_actors(reload, state.active_color_index);
        for actor in &mut ui_actors {
            apply_alpha_to_actor(actor, alpha_multiplier);
        }
        actors.extend(ui_actors);
        return actors;
    }
    if let Some(score_import) = &state.score_import_ui {
        let header = if score_import.done {
            "Score import complete"
        } else {
            "Importing scores..."
        };
        let total = score_import.total_charts.max(score_import.processed_charts);
        let progress_line = format!(
            "Endpoint: {}   Profile: {}\nPack: {}\nProgress: {}/{} (found={}, missing={}, failed={})",
            score_import.endpoint.display_name(),
            score_import.profile_name,
            score_import.pack_label,
            score_import.processed_charts,
            total,
            score_import.imported_scores,
            score_import.missing_scores,
            score_import.failed_requests
        );
        let detail_line = if score_import.done {
            score_import.done_message.as_str()
        } else {
            score_import.detail_line.as_str()
        };
        let text = format!("{header}\n{progress_line}\n{detail_line}");

        let mut ui_actors: Vec<Actor> = Vec::with_capacity(2);
        ui_actors.push(act!(quad:
            align(0.0, 0.0):
            xy(0.0, 0.0):
            zoomto(screen_width(), screen_height()):
            diffuse(0.0, 0.0, 0.0, 0.7):
            z(300)
        ));
        ui_actors.push(act!(text:
            align(0.5, 0.5):
            xy(screen_width() * 0.5, screen_height() * 0.5):
            zoom(0.95):
            diffuse(1.0, 1.0, 1.0, 1.0):
            font("miso"):
            settext(text):
            horizalign(center):
            z(301)
        ));
        for actor in &mut ui_actors {
            apply_alpha_to_actor(actor, alpha_multiplier);
        }
        actors.extend(ui_actors);
        return actors;
    }
    if let Some(mut ui_actors) =
        shared_pack_sync::build_overlay(&state.pack_sync_overlay, state.active_color_index)
    {
        for actor in &mut ui_actors {
            apply_alpha_to_actor(actor, alpha_multiplier);
        }
        actors.extend(ui_actors);
        return actors;
    }

    let mut ui_actors = Vec::new();

    /* ------------------------------ TOP BAR ------------------------------- */
    const FG: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    let title_text = match state.view {
        OptionsView::Main => "OPTIONS",
        OptionsView::Submenu(kind) => submenu_title(kind),
    };
    ui_actors.push(screen_bar::build(screen_bar::ScreenBarParams {
        title: title_text,
        title_placement: ScreenBarTitlePlacement::Left,
        position: ScreenBarPosition::Top,
        transparent: false,
        left_text: None,
        center_text: None,
        right_text: None,
        left_avatar: None,
        right_avatar: None,
        fg_color: FG,
    }));

    /* --------------------------- MAIN CONTENT UI -------------------------- */

    // --- global colors ---
    let col_active_bg = color::rgba_hex("#333333"); // active bg for normal rows

    // inactive bg = #071016 @ 0.8 alpha
    let base_inactive = color::rgba_hex("#071016");
    let col_inactive_bg: [f32; 4] = [base_inactive[0], base_inactive[1], base_inactive[2], 0.8];

    let col_white = [1.0, 1.0, 1.0, 1.0];
    let col_black = [0.0, 0.0, 0.0, 1.0];

    // Simply Love brand color (now uses the active theme color).
    let col_brand_bg = color::simply_love_rgba(state.active_color_index); // <-- CHANGED

    // --- scale & origin honoring fixed screen-space margins ---
    let (s, list_x, list_y) = scaled_block_origin_with_margins();

    // Geometry (scaled)
    let list_w = list_w_unscaled() * s;
    let sep_w = SEP_W * s;
    let desc_w = desc_w_unscaled() * s;
    let desc_h = DESC_H * s;

    // Separator immediately to the RIGHT of the rows, aligned to the FIRST row top
    ui_actors.push(act!(quad:
        align(0.0, 0.0):
        xy(list_x + list_w, list_y):
        zoomto(sep_w, desc_h):
        diffuse(col_active_bg[0], col_active_bg[1], col_active_bg[2], col_active_bg[3]) // #333333
    ));

    // Description box (RIGHT of separator), aligned to the first row top
    let desc_x = list_x + list_w + sep_w;
    ui_actors.push(act!(quad:
        align(0.0, 0.0):
        xy(desc_x, list_y):
        zoomto(desc_w, desc_h):
        diffuse(col_active_bg[0], col_active_bg[1], col_active_bg[2], col_active_bg[3]) // #333333
    ));

    // -------------------------- Rows + Description -------------------------
    let selected_item: Option<(DescriptionCacheKey, &Item)>;
    let cursor_now = || -> Option<(f32, f32, f32, f32)> {
        if !state.cursor_initialized {
            return None;
        }
        let t = state.cursor_t.clamp(0.0, 1.0);
        let x = (state.cursor_to_x - state.cursor_from_x).mul_add(t, state.cursor_from_x);
        let y = (state.cursor_to_y - state.cursor_from_y).mul_add(t, state.cursor_from_y);
        let w = (state.cursor_to_w - state.cursor_from_w).mul_add(t, state.cursor_from_w);
        let h = (state.cursor_to_h - state.cursor_from_h).mul_add(t, state.cursor_from_h);
        Some((x, y, w, h))
    };

    match state.view {
        OptionsView::Main => {
            // Active text color (for normal rows) – Simply Love uses row index + global color index.
            let col_active_text =
                color::simply_love_rgba(state.active_color_index + state.selected as i32);

            let total_items = ITEMS.len();
            let row_h = ROW_H * s;
            for (item_idx, _) in ITEMS.iter().enumerate() {
                let (row_mid_y, row_alpha) = state
                    .row_tweens
                    .get(item_idx)
                    .map(|tw| (tw.y(), tw.a()))
                    .unwrap_or_else(|| {
                        row_dest_for_index(total_items, state.selected, item_idx, s, list_y)
                    });
                let row_alpha = row_alpha.clamp(0.0, 1.0);
                if row_alpha <= 0.001 {
                    continue;
                }
                let row_y = row_mid_y - 0.5 * row_h;
                let is_active = item_idx == state.selected;
                let is_exit = item_idx == total_items - 1;
                let row_w = if is_exit || !is_active {
                    list_w - sep_w
                } else {
                    list_w
                };
                let bg = if is_active {
                    if is_exit { col_brand_bg } else { col_active_bg }
                } else {
                    col_inactive_bg
                };

                ui_actors.push(act!(quad:
                    align(0.0, 0.0):
                    xy(list_x, row_y):
                    zoomto(row_w, row_h):
                    diffuse(bg[0], bg[1], bg[2], bg[3] * row_alpha)
                ));

                let heart_x = HEART_LEFT_PAD.mul_add(s, list_x);
                let text_x_base = TEXT_LEFT_PAD.mul_add(s, list_x);
                if !is_exit {
                    let mut heart_tint = if is_active {
                        col_active_text
                    } else {
                        col_white
                    };
                    heart_tint[3] *= row_alpha;
                    ui_actors.push(act!(sprite("heart.png"):
                        align(0.0, 0.5):
                        xy(heart_x, row_mid_y):
                        zoom(HEART_ZOOM):
                        diffuse(heart_tint[0], heart_tint[1], heart_tint[2], heart_tint[3])
                    ));
                }

                let text_x = if is_exit { heart_x } else { text_x_base };
                let label = ITEMS[item_idx].name.get();
                let mut color_t = if is_exit {
                    if is_active { col_black } else { col_white }
                } else if is_active {
                    col_active_text
                } else {
                    col_white
                };
                color_t[3] *= row_alpha;
                ui_actors.push(act!(text:
                    align(0.0, 0.5):
                    xy(text_x, row_mid_y):
                    zoom(ITEM_TEXT_ZOOM):
                    diffuse(color_t[0], color_t[1], color_t[2], color_t[3]):
                    font("miso"):
                    settext(&label):
                    horizalign(left)
                ));
            }

            let sel = state.selected.min(ITEMS.len() - 1);
            selected_item = Some((DescriptionCacheKey::Main(sel), &ITEMS[sel]));
        }
        OptionsView::Submenu(kind) => {
            let rows = submenu_rows(kind);
            let choice_indices = submenu_choice_indices(state, kind);
            let items = submenu_items(kind);
            let visible_rows = submenu_visible_row_indices(state, kind, rows);
            if is_launcher_submenu(kind) {
                let col_active_text =
                    color::simply_love_rgba(state.active_color_index + state.sub_selected as i32);
                let total_rows = rows.len() + 1;
                let row_h = ROW_H * s;
                for row_idx in 0..total_rows {
                    let (row_mid_y, row_alpha) = state
                        .row_tweens
                        .get(row_idx)
                        .map(|tw| (tw.y(), tw.a()))
                        .unwrap_or_else(|| {
                            row_dest_for_index(total_rows, state.sub_selected, row_idx, s, list_y)
                        });
                    let row_alpha = row_alpha.clamp(0.0, 1.0);
                    if row_alpha <= 0.001 {
                        continue;
                    }
                    let row_y = row_mid_y - 0.5 * row_h;
                    let is_active = row_idx == state.sub_selected;
                    let is_exit = row_idx == total_rows - 1;
                    let row_w = if is_exit || !is_active {
                        list_w - sep_w
                    } else {
                        list_w
                    };
                    let bg = if is_active {
                        if is_exit { col_brand_bg } else { col_active_bg }
                    } else {
                        col_inactive_bg
                    };

                    ui_actors.push(act!(quad:
                        align(0.0, 0.0):
                        xy(list_x, row_y):
                        zoomto(row_w, row_h):
                        diffuse(bg[0], bg[1], bg[2], bg[3] * row_alpha)
                    ));

                    let heart_x = HEART_LEFT_PAD.mul_add(s, list_x);
                    let text_x_base = TEXT_LEFT_PAD.mul_add(s, list_x);
                    if !is_exit {
                        let mut heart_tint = if is_active {
                            col_active_text
                        } else {
                            col_white
                        };
                        heart_tint[3] *= row_alpha;
                        ui_actors.push(act!(sprite("heart.png"):
                            align(0.0, 0.5):
                            xy(heart_x, row_mid_y):
                            zoom(HEART_ZOOM):
                            diffuse(heart_tint[0], heart_tint[1], heart_tint[2], heart_tint[3])
                        ));
                    }

                    let text_x = if is_exit { heart_x } else { text_x_base };
                    let label = if row_idx < rows.len() {
                        rows[row_idx].label.get()
                    } else {
                        Arc::from("Exit")
                    };
                    let mut text_color = if is_exit {
                        if is_active { col_black } else { col_white }
                    } else if is_active {
                        col_active_text
                    } else {
                        col_white
                    };
                    text_color[3] *= row_alpha;
                    ui_actors.push(act!(text:
                        align(0.0, 0.5):
                        xy(text_x, row_mid_y):
                        zoom(ITEM_TEXT_ZOOM):
                        diffuse(text_color[0], text_color[1], text_color[2], text_color[3]):
                        font("miso"):
                        settext(&label):
                        horizalign(left)
                    ));

                    if row_idx < rows.len() {
                        let row = &rows[row_idx];
                        if row.inline {
                            let choices = row_choices(state, kind, rows, row_idx);
                            if !choices.is_empty() {
                                let choice_idx = choice_indices
                                    .get(row_idx)
                                    .copied()
                                    .unwrap_or(0)
                                    .min(choices.len().saturating_sub(1));
                                let mut value_color = if is_active {
                                    col_active_text
                                } else {
                                    col_white
                                };
                                value_color[3] *= row_alpha;
                                let value_x = list_w.mul_add(1.0, list_x - TEXT_LEFT_PAD * s);
                                ui_actors.push(act!(text:
                                    align(1.0, 0.5):
                                    xy(value_x, row_mid_y):
                                    zoom(ITEM_TEXT_ZOOM):
                                    diffuse(value_color[0], value_color[1], value_color[2], value_color[3]):
                                    font("miso"):
                                    settext(choices[choice_idx].as_ref()):
                                    horizalign(right)
                                ));
                            }
                        }
                    }
                }

                let sel = state.sub_selected.min(total_rows.saturating_sub(1));
                let (item_idx, item) = if sel < rows.len() {
                    (sel, &items[sel])
                } else {
                    let idx = items.len().saturating_sub(1);
                    (idx, &items[idx])
                };
                selected_item = Some((DescriptionCacheKey::Submenu(kind, item_idx), item));
            } else {
                // Active text color for submenu rows.
                let col_active_text = color::simply_love_rgba(state.active_color_index);
                // Inactive option text color should be #808080 (alpha 1.0), match player options.
                let sl_gray = color::rgba_hex("#808080");

                let total_rows = visible_rows.len() + 1; // + Exit row

                let label_bg_w = SUB_LABEL_COL_W * s;
                let label_text_x = SUB_LABEL_TEXT_LEFT_PAD.mul_add(s, list_x);
                // Keep submenu header labels bounded to the left label column.
                let label_text_max_w = (label_bg_w - SUB_LABEL_TEXT_LEFT_PAD * s - 5.0).max(0.0);

                // Helper to compute the cursor center X for a given submenu row index.
                let calc_row_center_x = |row_idx: usize| -> f32 {
                    if row_idx >= total_rows {
                        return list_w.mul_add(0.5, list_x);
                    }
                    if row_idx == total_rows - 1 {
                        // Exit row: center within the items column (row width minus label column),
                        // matching how single-value rows like Music Rate are centered in player_options.rs.
                        let item_col_left = list_x + label_bg_w;
                        let item_col_w = list_w - label_bg_w;
                        return item_col_w.mul_add(0.5, item_col_left)
                            + SUB_SINGLE_VALUE_CENTER_OFFSET * s;
                    }
                    let Some(actual_row_idx) = visible_rows.get(row_idx).copied() else {
                        return list_w.mul_add(0.5, list_x);
                    };
                    let row = &rows[actual_row_idx];
                    let item_col_left = list_x + label_bg_w;
                    let item_col_w = list_w - label_bg_w;
                    let single_center_x =
                        item_col_w.mul_add(0.5, item_col_left) + SUB_SINGLE_VALUE_CENTER_OFFSET * s;
                    // Non-inline rows behave as single-value rows: keep the cursor centered
                    // on the center of the available items column (row width minus label column).
                    if !row.inline {
                        return single_center_x;
                    }
                    let Some(layout) =
                        submenu_row_layout(state, asset_manager, kind, actual_row_idx)
                    else {
                        return list_w.mul_add(0.5, list_x);
                    };
                    if !layout.inline_row || layout.centers.is_empty() {
                        return single_center_x;
                    }
                    let sel_idx = choice_indices
                        .get(actual_row_idx)
                        .copied()
                        .unwrap_or(0)
                        .min(layout.centers.len().saturating_sub(1));
                    SUB_INLINE_ITEMS_LEFT_PAD.mul_add(s, list_x + label_bg_w)
                        + layout.centers[sel_idx]
                };

                let row_h = ROW_H * s;
                for row_idx in 0..total_rows {
                    let (row_mid_y, row_alpha) = state
                        .row_tweens
                        .get(row_idx)
                        .map(|tw| (tw.y(), tw.a()))
                        .unwrap_or_else(|| {
                            row_dest_for_index(total_rows, state.sub_selected, row_idx, s, list_y)
                        });
                    let row_alpha = row_alpha.clamp(0.0, 1.0);
                    if row_alpha <= 0.001 {
                        continue;
                    }
                    let row_y = row_mid_y - 0.5 * row_h;

                    let is_active = row_idx == state.sub_selected;
                    let is_exit = row_idx == total_rows - 1;

                    let row_w = if is_exit {
                        list_w - sep_w
                    } else if is_active {
                        list_w
                    } else {
                        list_w - sep_w
                    };

                    let bg = if is_active {
                        col_active_bg
                    } else {
                        col_inactive_bg
                    };

                    ui_actors.push(act!(quad:
                        align(0.0, 0.0):
                        xy(list_x, row_y):
                        zoomto(row_w, row_h):
                        diffuse(bg[0], bg[1], bg[2], bg[3] * row_alpha)
                    ));
                    let show_option_row = !is_exit;

                    if show_option_row {
                        let Some(actual_row_idx) = visible_rows.get(row_idx).copied() else {
                            continue;
                        };
                        // Left label background column (matches player options style).
                        ui_actors.push(act!(quad:
                            align(0.0, 0.0):
                            xy(list_x, row_y):
                            zoomto(label_bg_w, row_h):
                            diffuse(0.0, 0.0, 0.0, 0.25 * row_alpha)
                        ));

                        let row = &rows[actual_row_idx];
                        let label = row.label.get();
                        let is_disabled = is_submenu_row_disabled(kind, row.id);
                        #[cfg(target_os = "linux")]
                        let child_label_indent = if matches!(kind, SubmenuKind::Sound)
                            && sound_parent_row(actual_row_idx).is_some()
                        {
                            12.0 * s
                        } else {
                            0.0
                        };
                        #[cfg(not(target_os = "linux"))]
                        let child_label_indent = 0.0;
                        let label_text_x = label_text_x + child_label_indent;
                        let label_text_max_w = (label_text_max_w - child_label_indent).max(0.0);
                        let title_color = if is_active {
                            let mut c = col_active_text;
                            c[3] = 1.0;
                            c
                        } else {
                            col_white
                        };
                        let mut title_color = title_color;
                        title_color[3] *= row_alpha;

                        ui_actors.push(act!(text:
                            align(0.0, 0.5):
                            xy(label_text_x, row_mid_y):
                            zoom(ITEM_TEXT_ZOOM):
                            diffuse(title_color[0], title_color[1], title_color[2], title_color[3]):
                            font("miso"):
                            settext(&label):
                            maxwidth(label_text_max_w):
                            horizalign(left)
                        ));

                        // Inline Off/On options in the items column (or a single centered value if inline == false).
                        if let Some(layout) =
                            submenu_row_layout(state, asset_manager, kind, actual_row_idx)
                            && !layout.texts.is_empty()
                        {
                            let value_zoom = 0.835_f32;
                            let selected_choice = choice_indices
                                .get(actual_row_idx)
                                .copied()
                                .unwrap_or(0)
                                .min(layout.texts.len().saturating_sub(1));
                            let is_chart_info_row = matches!(kind, SubmenuKind::SelectMusic)
                                && row.id == SubRowId::ChartInfo;
                            let is_scorebox_cycle_row = matches!(kind, SubmenuKind::SelectMusic)
                                && row.id == SubRowId::GsBoxLeaderboards;
                            let is_auto_screenshot_row = matches!(kind, SubmenuKind::Gameplay)
                                && row.id == SubRowId::AutoScreenshot;
                            let is_multi_toggle_row = is_chart_info_row
                                || is_scorebox_cycle_row
                                || is_auto_screenshot_row;
                            let chart_info_enabled_mask = if is_chart_info_row {
                                select_music_chart_info_enabled_mask()
                            } else {
                                0
                            };
                            let scorebox_enabled_mask = if is_scorebox_cycle_row {
                                select_music_scorebox_cycle_enabled_mask()
                            } else {
                                0
                            };
                            let auto_screenshot_mask = if is_auto_screenshot_row {
                                auto_screenshot_enabled_mask()
                            } else {
                                0
                            };
                            let mut selected_left_x: Option<f32> = None;
                            let choice_inner_left =
                                SUB_INLINE_ITEMS_LEFT_PAD.mul_add(s, list_x + label_bg_w);

                            if layout.inline_row {
                                for (idx, choice) in layout.texts.iter().enumerate() {
                                    let x = choice_inner_left
                                        + layout.x_positions.get(idx).copied().unwrap_or_default();
                                    let is_choice_selected = idx == selected_choice;
                                    if is_choice_selected {
                                        selected_left_x = Some(x);
                                    }
                                    let is_choice_enabled = if is_chart_info_row {
                                        (chart_info_enabled_mask
                                            & select_music_chart_info_bit_from_choice(idx))
                                            != 0
                                    } else if is_scorebox_cycle_row {
                                        (scorebox_enabled_mask
                                            & scorebox_cycle_bit_from_choice(idx))
                                            != 0
                                    } else if is_auto_screenshot_row {
                                        (auto_screenshot_mask
                                            & auto_screenshot_bit_from_choice(idx))
                                            != 0
                                    } else {
                                        false
                                    };
                                    let mut choice_color = if is_disabled && !is_choice_selected {
                                        sl_gray
                                    } else if is_multi_toggle_row {
                                        if is_choice_enabled {
                                            col_white
                                        } else {
                                            sl_gray
                                        }
                                    } else if is_active {
                                        col_white
                                    } else {
                                        sl_gray
                                    };
                                    choice_color[3] *= row_alpha;
                                    ui_actors.push(act!(text:
                                        align(0.0, 0.5):
                                        xy(x, row_mid_y):
                                        zoom(value_zoom):
                                        diffuse(choice_color[0], choice_color[1], choice_color[2], choice_color[3]):
                                        font("miso"):
                                        settext(choice):
                                        horizalign(left)
                                    ));
                                }
                            } else {
                                let mut choice_color = if is_active { col_white } else { sl_gray };
                                choice_color[3] *= row_alpha;
                                let choice_center_x = calc_row_center_x(row_idx);
                                let draw_w =
                                    layout.widths.get(selected_choice).copied().unwrap_or(40.0);
                                selected_left_x = Some(choice_center_x - draw_w * 0.5);
                                let choice_text = layout
                                    .texts
                                    .get(selected_choice)
                                    .cloned()
                                    .unwrap_or_else(|| Arc::<str>::from("??"));
                                ui_actors.push(act!(text:
                                    align(0.5, 0.5):
                                    xy(choice_center_x, row_mid_y):
                                    zoom(value_zoom):
                                    diffuse(choice_color[0], choice_color[1], choice_color[2], choice_color[3]):
                                    font("miso"):
                                    settext(choice_text):
                                    horizalign(center)
                                ));
                            }

                            // For normal rows, underline the selected option.
                            // For multi-toggle rows, underline each enabled option.
                            if layout.inline_row && is_multi_toggle_row {
                                let line_thickness = widescale(2.0, 2.5).round().max(1.0);
                                let offset = widescale(3.0, 4.0);
                                let underline_y = row_mid_y + layout.text_h * 0.5 + offset;
                                let mut line_color =
                                    color::decorative_rgba(state.active_color_index);
                                line_color[3] *= row_alpha;
                                for idx in 0..layout.texts.len() {
                                    let enabled = if is_chart_info_row {
                                        let bit = select_music_chart_info_bit_from_choice(idx);
                                        bit != 0 && (chart_info_enabled_mask & bit) != 0
                                    } else if is_scorebox_cycle_row {
                                        let bit = scorebox_cycle_bit_from_choice(idx);
                                        bit != 0 && (scorebox_enabled_mask & bit) != 0
                                    } else {
                                        let bit = auto_screenshot_bit_from_choice(idx);
                                        bit != 0 && (auto_screenshot_mask & bit) != 0
                                    };
                                    if !enabled {
                                        continue;
                                    }
                                    let underline_left_x = choice_inner_left
                                        + layout.x_positions.get(idx).copied().unwrap_or_default();
                                    let underline_w =
                                        layout.widths.get(idx).copied().unwrap_or(40.0).ceil();
                                    ui_actors.push(act!(quad:
                                        align(0.0, 0.5):
                                        xy(underline_left_x, underline_y):
                                        zoomto(underline_w, line_thickness):
                                        diffuse(line_color[0], line_color[1], line_color[2], line_color[3]):
                                        z(101)
                                    ));
                                }
                            } else if let Some(sel_left_x) = selected_left_x {
                                let line_thickness = widescale(2.0, 2.5).round().max(1.0);
                                let underline_w = layout
                                    .widths
                                    .get(selected_choice)
                                    .copied()
                                    .unwrap_or(40.0)
                                    .ceil();
                                let offset = widescale(3.0, 4.0);
                                let underline_y = row_mid_y + layout.text_h * 0.5 + offset;
                                let mut line_color =
                                    color::decorative_rgba(state.active_color_index);
                                line_color[3] *= row_alpha;
                                ui_actors.push(act!(quad:
                                    align(0.0, 0.5):
                                    xy(sel_left_x, underline_y):
                                    zoomto(underline_w, line_thickness):
                                    diffuse(line_color[0], line_color[1], line_color[2], line_color[3]):
                                    z(101)
                                ));
                            }

                            // Encircling cursor ring around the active option when this row is active.
                            // During submenu fades, hide the ring to avoid exposing its construction.
                            if is_active
                                && !is_fading_submenu
                                && let Some((center_x, center_y, ring_w, ring_h)) = cursor_now()
                            {
                                let border_w = widescale(2.0, 2.5);
                                let left = center_x - ring_w * 0.5;
                                let right = center_x + ring_w * 0.5;
                                let top = center_y - ring_h * 0.5;
                                let bottom = center_y + ring_h * 0.5;
                                let mut ring_color =
                                    color::decorative_rgba(state.active_color_index);
                                ring_color[3] *= row_alpha;
                                ui_actors.push(act!(quad:
                                align(0.5, 0.5):
                                xy(center_x, top + border_w * 0.5):
                                zoomto(ring_w, border_w):
                                diffuse(ring_color[0], ring_color[1], ring_color[2], ring_color[3]):
                                z(101)
                            ));
                                ui_actors.push(act!(quad:
                                align(0.5, 0.5):
                                xy(center_x, bottom - border_w * 0.5):
                                zoomto(ring_w, border_w):
                                diffuse(ring_color[0], ring_color[1], ring_color[2], ring_color[3]):
                                z(101)
                            ));
                                ui_actors.push(act!(quad:
                                align(0.5, 0.5):
                                xy(left + border_w * 0.5, center_y):
                                zoomto(border_w, ring_h):
                                diffuse(ring_color[0], ring_color[1], ring_color[2], ring_color[3]):
                                z(101)
                            ));
                                ui_actors.push(act!(quad:
                                align(0.5, 0.5):
                                xy(right - border_w * 0.5, center_y):
                                zoomto(border_w, ring_h):
                                diffuse(ring_color[0], ring_color[1], ring_color[2], ring_color[3]):
                                z(101)
                            ));
                            }
                        }
                    } else {
                        // Exit row: centered "Exit" text in the items column.
                        let exit_label = tr("Common", "Exit");
                        let label = &*exit_label;
                        let value_zoom = 0.835_f32;
                        let mut choice_color = if is_active { col_white } else { sl_gray };
                        choice_color[3] *= row_alpha;
                        let center_x = calc_row_center_x(row_idx);
                        let center_y = row_mid_y;

                        ui_actors.push(act!(text:
                        align(0.5, 0.5):
                        xy(center_x, center_y):
                        zoom(value_zoom):
                        diffuse(choice_color[0], choice_color[1], choice_color[2], choice_color[3]):
                        font("miso"):
                        settext(label):
                        horizalign(center)
                    ));

                        // Draw the selection cursor ring for the Exit row when active.
                        // During submenu fades, hide the ring to avoid exposing its construction.
                        if is_active
                            && !is_fading_submenu
                            && let Some((ring_x, ring_y, ring_w, ring_h)) = cursor_now()
                        {
                            let border_w = widescale(2.0, 2.5);
                            let left = ring_x - ring_w * 0.5;
                            let right = ring_x + ring_w * 0.5;
                            let top = ring_y - ring_h * 0.5;
                            let bottom = ring_y + ring_h * 0.5;
                            let mut ring_color = color::decorative_rgba(state.active_color_index);
                            ring_color[3] *= row_alpha;

                            ui_actors.push(act!(quad:
                                align(0.5, 0.5):
                                xy((left + right) * 0.5, top + border_w * 0.5):
                                zoomto(ring_w, border_w):
                                diffuse(ring_color[0], ring_color[1], ring_color[2], ring_color[3]):
                                z(101)
                            ));
                            ui_actors.push(act!(quad:
                                align(0.5, 0.5):
                                xy((left + right) * 0.5, bottom - border_w * 0.5):
                                zoomto(ring_w, border_w):
                                diffuse(ring_color[0], ring_color[1], ring_color[2], ring_color[3]):
                                z(101)
                            ));
                            ui_actors.push(act!(quad:
                                align(0.5, 0.5):
                                xy(left + border_w * 0.5, (top + bottom) * 0.5):
                                zoomto(border_w, ring_h):
                                diffuse(ring_color[0], ring_color[1], ring_color[2], ring_color[3]):
                                z(101)
                            ));
                            ui_actors.push(act!(quad:
                                align(0.5, 0.5):
                                xy(right - border_w * 0.5, (top + bottom) * 0.5):
                                zoomto(border_w, ring_h):
                                diffuse(ring_color[0], ring_color[1], ring_color[2], ring_color[3]):
                                z(101)
                            ));
                        }
                    }
                }

                // Description items for the submenu
                let total_rows = visible_rows.len() + 1;
                let sel = state.sub_selected.min(total_rows.saturating_sub(1));
                let (item_idx, item) = if sel < visible_rows.len() {
                    let actual_row_idx = visible_rows[sel];
                    (actual_row_idx, &items[actual_row_idx])
                } else {
                    let idx = items.len().saturating_sub(1);
                    (idx, &items[idx])
                };
                selected_item = Some((DescriptionCacheKey::Submenu(kind, item_idx), item));
            }
        }
    }

    // ------------------- Description content (selected) -------------------
    if let Some((desc_key, item)) = selected_item {
        // Match Simply Love's description box feel:
        // - explicit top/side padding for title and bullets so they can be tuned
        // - text zoom similar to other help text (player options, etc.)
        let mut cursor_y = DESC_TITLE_TOP_PAD_PX.mul_add(s, list_y);
        let desc_layout = description_layout(state, asset_manager, desc_key, item, s);
        let title_side_pad = DESC_TITLE_SIDE_PAD_PX * s;
        let title_step_px = 20.0 * s;
        let body_step_px = 18.0 * s;
        let bullet_side_pad = DESC_BULLET_SIDE_PAD_PX * s;

        for block in &desc_layout.blocks {
            match block {
                RenderedHelpBlock::Paragraph { text, line_count } => {
                    ui_actors.push(act!(text:
                        align(0.0, 0.0):
                        xy(desc_x + title_side_pad, cursor_y):
                        zoom(DESC_TITLE_ZOOM):
                        diffuse(1.0, 1.0, 1.0, 1.0):
                        font("miso"): settext(text):
                        horizalign(left)
                    ));
                    cursor_y += title_step_px * *line_count as f32 + DESC_BULLET_TOP_PAD_PX * s;
                }
                RenderedHelpBlock::Bullet { text, line_count } => {
                    let bullet_x = DESC_BULLET_INDENT_PX.mul_add(s, desc_x + bullet_side_pad);
                    ui_actors.push(act!(text:
                        align(0.0, 0.0):
                        xy(bullet_x, cursor_y):
                        zoom(DESC_BODY_ZOOM):
                        diffuse(1.0, 1.0, 1.0, 1.0):
                        font("miso"): settext(text):
                        horizalign(left)
                    ));
                    cursor_y += body_step_px * *line_count as f32;
                }
            }
        }
    }
    if let Some(confirm) = &state.score_import_confirm {
        let prompt_text = format!(
            "Import ALL packs for {} / {}?\nOnly missing GS scores: {}.\nRate limit is hard-capped at 3 requests per second.\nFor many charts this can take more than one hour.\nSpamming APIs can be problematic.\n\nStart now?",
            confirm.selection.endpoint.display_name(),
            if confirm.selection.profile.display_name.is_empty() {
                confirm.selection.profile.id.as_str()
            } else {
                confirm.selection.profile.display_name.as_str()
            },
            if confirm.selection.only_missing_gs_scores {
                "Yes"
            } else {
                "No"
            }
        );
        ui_actors.extend(build_yes_no_confirm_overlay(
            &prompt_text,
            confirm.active_choice,
            state.active_color_index,
        ));
    }
    if let Some(confirm) = &state.sync_pack_confirm {
        let prompt_text = format!(
            "Sync {}?\nThis will analyze every matching simfile here in Options.\nYou can review offsets and confidence before saving.\n\nStart now?",
            if confirm.selection.pack_group.is_none() {
                "ALL files"
            } else {
                confirm.selection.pack_label.as_str()
            }
        );
        ui_actors.extend(build_yes_no_confirm_overlay(
            &prompt_text,
            confirm.active_choice,
            state.active_color_index,
        ));
    }

    let combined_alpha = alpha_multiplier * state.content_alpha;
    for actor in &mut ui_actors {
        apply_alpha_to_actor(actor, combined_alpha);
    }
    actors.extend(ui_actors);

    actors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::AssetManager;
    use crate::engine::input::{InputEvent, InputSource, VirtualAction};
    use std::time::Instant;

    fn press(
        state: &mut State,
        asset_manager: &AssetManager,
        action: VirtualAction,
    ) -> ScreenAction {
        let now = Instant::now();
        handle_input(
            state,
            asset_manager,
            &InputEvent {
                action,
                pressed: true,
                source: InputSource::Keyboard,
                timestamp: now,
                timestamp_host_nanos: 0,
                stored_at: now,
                emitted_at: now,
            },
        )
    }

    #[test]
    fn inferred_aspect_choice_maps_1024x768_to_4_3() {
        let idx = inferred_aspect_choice(1024, 768);
        assert_eq!(
            DISPLAY_ASPECT_RATIO_CHOICES[idx].as_str_static(),
            Some("4:3")
        );
    }

    #[test]
    fn sync_display_resolution_selects_loaded_4_3_mode() {
        let mut state = init();
        sync_display_resolution(&mut state, 1024, 768);

        assert_eq!(selected_aspect_label(&state), "4:3");
        assert_eq!(selected_resolution(&state), (1024, 768));
        assert!(state.resolution_choices.contains(&(1024, 768)));
    }

    #[test]
    fn p2_can_navigate_and_change_system_options() {
        let asset_manager = AssetManager::new();
        let mut state = init();

        assert_eq!(state.selected, 0);
        press(&mut state, &asset_manager, VirtualAction::p2_start);
        update(&mut state, 1.0, &asset_manager);
        update(&mut state, 1.0, &asset_manager);
        assert!(matches!(
            state.view,
            OptionsView::Submenu(SubmenuKind::System)
        ));

        press(&mut state, &asset_manager, VirtualAction::p2_down);
        press(&mut state, &asset_manager, VirtualAction::p2_down);
        press(&mut state, &asset_manager, VirtualAction::p2_down);
        assert_eq!(state.sub_selected, 3);

        let before = state.sub_cursor_indices_system[3];
        press(&mut state, &asset_manager, VirtualAction::p2_right);
        assert_eq!(state.sub_cursor_indices_system[3], before + 1);
    }
}
