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
mod score_import;
mod layout;
mod render;
mod input;
mod choice_helpers;
pub(crate) use types::*;
pub(crate) use menus::*;
pub(crate) use sound::*;
pub(crate) use choice_helpers::*;
pub(crate) use score_import::*;
pub(crate) use layout::*;
pub(crate) use render::*;
pub(crate) use input::*;

/* ---------------------------- transitions ---------------------------- */
pub(crate) const TRANSITION_IN_DURATION: f32 = 0.4;
pub(crate) const TRANSITION_OUT_DURATION: f32 = 0.4;
pub(crate) const RELOAD_BAR_H: f32 = 30.0;

/* -------------------------- hold-to-scroll timing ------------------------- */
pub(crate) const NAV_INITIAL_HOLD_DELAY: Duration = Duration::from_millis(300);
pub(crate) const NAV_REPEAT_SCROLL_INTERVAL: Duration = Duration::from_millis(50);

/* ----------------------------- cursor tweening ----------------------------- */
// Simply Love metrics.ini uses 0.1 for both [ScreenOptions] TweenSeconds and CursorTweenSeconds.
// ScreenOptionsService rows inherit OptionRow tween behavior, so keep both aligned at 0.1.
pub(crate) const SL_OPTION_ROW_TWEEN_SECONDS: f32 = 0.1;
pub(crate) const CURSOR_TWEEN_SECONDS: f32 = SL_OPTION_ROW_TWEEN_SECONDS;
pub(crate) const ROW_TWEEN_SECONDS: f32 = SL_OPTION_ROW_TWEEN_SECONDS;
// Spacing between inline items in OptionRows (pixels at current zoom)
pub(crate) const INLINE_SPACING: f32 = 15.75;

// Match Simply Love operator menu ranges (±1000 ms) for these calibrations.
pub(crate) const GLOBAL_OFFSET_MIN_MS: i32 = -1000;
pub(crate) const GLOBAL_OFFSET_MAX_MS: i32 = 1000;
pub(crate) const VISUAL_DELAY_MIN_MS: i32 = -1000;
pub(crate) const VISUAL_DELAY_MAX_MS: i32 = 1000;
pub(crate) const VOLUME_MIN_PERCENT: i32 = 0;
pub(crate) const VOLUME_MAX_PERCENT: i32 = 100;
pub(crate) const INPUT_DEBOUNCE_MIN_MS: i32 = 0;
pub(crate) const INPUT_DEBOUNCE_MAX_MS: i32 = 200;
pub(crate) const NULL_OR_DIE_POSITIVE_MS_MIN_TENTHS: i32 = 1;
pub(crate) const NULL_OR_DIE_POSITIVE_MS_MAX_TENTHS: i32 = 1000;
pub(crate) const NULL_OR_DIE_MAGIC_OFFSET_MIN_TENTHS: i32 = -1000;
pub(crate) const NULL_OR_DIE_MAGIC_OFFSET_MAX_TENTHS: i32 = 1000;

// --- Monitor & Video Mode Data Structures ---





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


/* --------------------------------- layout -------------------------------- */

/// content rect = full screen minus top & bottom bars.
/// We fit the (rows + separator + description) block inside that content rect,
/// honoring LEFT, RIGHT and TOP margins in *screen pixels*.
/// Returns (scale, `origin_x`, `origin_y`).

/* -------------------------------- drawing -------------------------------- */


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
