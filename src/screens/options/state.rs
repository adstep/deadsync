use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavDirection {
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavWrap {
    Wrap,
    Clamp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmenuKind {
    System,
    Graphics,
    Input,
    InputBackend,
    OnlineScoring,
    NullOrDie,
    NullOrDieOptions,
    SyncPacks,
    Machine,
    Advanced,
    Course,
    Gameplay,
    Sound,
    SelectMusic,
    GrooveStats,
    ArrowCloud,
    ScoreImport,
}

impl SubmenuKind {
    pub(super) const ALL: [Self; 17] = [
        Self::System,
        Self::Graphics,
        Self::Input,
        Self::InputBackend,
        Self::OnlineScoring,
        Self::NullOrDie,
        Self::NullOrDieOptions,
        Self::SyncPacks,
        Self::Machine,
        Self::Advanced,
        Self::Course,
        Self::Gameplay,
        Self::Sound,
        Self::SelectMusic,
        Self::GrooveStats,
        Self::ArrowCloud,
        Self::ScoreImport,
    ];
    pub(super) const COUNT: usize = Self::ALL.len();

    #[inline]
    pub(super) const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Debug)]
pub(super) struct SubmenuState {
    pub(super) choice_indices: Vec<usize>,
    pub(super) cursor_indices: Vec<usize>,
}

#[derive(Clone, Debug)]
pub(super) struct SubmenuStates([SubmenuState; SubmenuKind::COUNT]);

impl SubmenuStates {
    pub(super) fn new(init: impl FnMut(usize) -> SubmenuState) -> Self {
        Self(std::array::from_fn(init))
    }

    pub(super) fn iter_mut(&mut self) -> std::slice::IterMut<'_, SubmenuState> {
        self.0.iter_mut()
    }
}

impl std::ops::Index<SubmenuKind> for SubmenuStates {
    type Output = SubmenuState;
    #[inline]
    fn index(&self, kind: SubmenuKind) -> &SubmenuState {
        &self.0[kind.index()]
    }
}

impl std::ops::IndexMut<SubmenuKind> for SubmenuStates {
    #[inline]
    fn index_mut(&mut self, kind: SubmenuKind) -> &mut SubmenuState {
        &mut self.0[kind.index()]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionsView {
    Main,
    Submenu(SubmenuKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DescriptionCacheKey {
    Main(usize),
    Submenu(SubmenuKind, usize),
}

/// A pre-wrapped block of text in the description pane, ready for rendering.
#[derive(Clone, Debug)]
pub(super) enum RenderedHelpBlock {
    Paragraph { text: Arc<str>, line_count: usize },
    Bullet { text: Arc<str>, line_count: usize },
}

#[derive(Clone, Debug)]
pub(super) struct DescriptionLayout {
    pub(super) key: DescriptionCacheKey,
    pub(super) blocks: Vec<RenderedHelpBlock>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SubmenuTransition {
    None,
    FadeOutToSubmenu,
    FadeInSubmenu,
    FadeOutToMain,
    FadeInMain,
}

pub struct State {
    pub selected: usize,
    pub(super) prev_selected: usize,
    pub active_color_index: i32, // <-- ADDED
    pub(super) bg: visual_style_bg::State,
    pub(super) nav_key_held_direction: Option<NavDirection>,
    pub(super) nav_key_held_since: Option<Instant>,
    pub(super) nav_key_last_scrolled_at: Option<Instant>,
    pub(super) nav_lr_held_direction: Option<isize>,
    pub(super) nav_lr_held_since: Option<Instant>,
    pub(super) nav_lr_last_adjusted_at: Option<Instant>,
    pub(super) view: OptionsView,
    pub(super) submenu_transition: SubmenuTransition,
    pub(super) pending_submenu_kind: Option<SubmenuKind>,
    pub(super) pending_submenu_parent_kind: Option<SubmenuKind>,
    pub(super) submenu_parent_kind: Option<SubmenuKind>,
    pub(super) submenu_fade_t: f32,
    pub(super) content_alpha: f32,
    pub(super) reload_ui: Option<ReloadUiState>,
    pub(super) score_import_ui: Option<ScoreImportUiState>,
    pub(super) pack_sync_overlay: shared_pack_sync::OverlayState,
    pub(super) score_import_confirm: Option<ScoreImportConfirmState>,
    pub(super) sync_pack_confirm: Option<SyncPackConfirmState>,
    pub(super) menu_lr_chord: screen_input::MenuLrChordTracker,
    pub(super) menu_lr_undo: i8,
    pub(super) pending_dedicated_menu_buttons: Option<bool>,
    // Submenu state
    pub(super) sub_selected: usize,
    pub(super) sub_prev_selected: usize,
    pub(super) sub_inline_x: f32,
    pub(super) sub: SubmenuStates,
    pub(super) system_noteskin_choices: Vec<String>,
    pub(super) score_import_profiles: Vec<ScoreImportProfileConfig>,
    pub(super) score_import_profile_choices: Vec<String>,
    pub(super) score_import_profile_ids: Vec<Option<String>>,
    pub(super) score_import_pack_choices: Vec<String>,
    pub(super) score_import_pack_filters: Vec<Option<String>>,
    pub(super) sync_pack_choices: Vec<String>,
    pub(super) sync_pack_filters: Vec<Option<String>>,
    pub(super) sound_device_options: Vec<SoundDeviceOption>,
    #[cfg(target_os = "linux")]
    pub(super) linux_backend_choices: Vec<String>,
    pub(super) master_volume_pct: i32,
    pub(super) sfx_volume_pct: i32,
    pub(super) assist_tick_volume_pct: i32,
    pub(super) music_volume_pct: i32,
    pub(super) global_offset_ms: i32,
    pub(super) visual_delay_ms: i32,
    pub(super) input_debounce_ms: i32,
    pub(super) null_or_die_fingerprint_tenths: i32,
    pub(super) null_or_die_window_tenths: i32,
    pub(super) null_or_die_step_tenths: i32,
    pub(super) null_or_die_magic_offset_tenths: i32,
    pub(super) video_renderer_at_load: BackendType,
    pub(super) display_mode_at_load: DisplayMode,
    pub(super) display_monitor_at_load: usize,
    pub(super) display_width_at_load: u32,
    pub(super) display_height_at_load: u32,
    pub(super) max_fps_at_load: u16,
    pub(super) vsync_at_load: bool,
    pub(super) present_mode_policy_at_load: PresentModePolicy,
    pub(super) high_dpi_at_load: bool,
    pub(super) display_mode_choices: Vec<String>,
    pub(super) software_thread_choices: Vec<u8>,
    pub(super) software_thread_labels: Vec<String>,
    pub(super) max_fps_choices: Vec<u16>,
    pub(super) resolution_choices: Vec<(u32, u32)>,
    pub(super) refresh_rate_choices: Vec<u32>, // New: stored in millihertz
    // Hardware info
    pub monitor_specs: Vec<MonitorSpec>,
    // Cursor ring tween (StopTweening/BeginTweening parity with ITGmania ScreenOptions::TweenCursor).
    pub(super) cursor_initialized: bool,
    pub(super) cursor_from_x: f32,
    pub(super) cursor_from_y: f32,
    pub(super) cursor_from_w: f32,
    pub(super) cursor_from_h: f32,
    pub(super) cursor_to_x: f32,
    pub(super) cursor_to_y: f32,
    pub(super) cursor_to_w: f32,
    pub(super) cursor_to_h: f32,
    pub(super) cursor_t: f32,
    // Shared row tween state for the active view (main list or submenu list).
    pub(super) row_tweens: Vec<RowTween>,
    pub(super) submenu_layout_cache_kind: Cell<Option<SubmenuKind>>,
    pub(super) submenu_row_layout_cache: RefCell<Vec<Option<SubmenuRowLayout>>>,
    pub(super) description_layout_cache: RefCell<Option<DescriptionLayout>>,
    pub(super) graphics_prev_visible_rows: Vec<usize>,
    pub(super) advanced_prev_visible_rows: Vec<usize>,
    pub(super) select_music_prev_visible_rows: Vec<usize>,
    pub(super) i18n_revision: u64,
}

pub fn init() -> State {
    let cfg = config::get();
    let system_noteskin_choices = discover_system_noteskin_choices();
    let software_thread_choices = build_software_thread_choices();
    let software_thread_labels = software_thread_choice_labels(&software_thread_choices);
    let max_fps_choices = build_max_fps_choices();
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
        active_color_index: cfg.simply_love_color,
        bg: visual_style_bg::State::new(),

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
        sub: SubmenuStates::new(|i| {
            let len = submenu_rows(SubmenuKind::ALL[i]).len();
            SubmenuState {
                choice_indices: vec![0; len],
                cursor_indices: vec![0; len],
            }
        }),
        system_noteskin_choices,
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
        high_dpi_at_load: cfg.high_dpi,
        display_mode_choices: build_display_mode_choices(&[]),
        software_thread_choices,
        software_thread_labels,
        max_fps_choices,
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
        i18n_revision: crate::assets::i18n::revision(),
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
        &mut state.sub[SubmenuKind::System].choice_indices,
        SYSTEM_OPTIONS_ROWS,
        RowId::SysGame,
        0,
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::System].choice_indices,
        SYSTEM_OPTIONS_ROWS,
        RowId::SysTheme,
        0,
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::System].choice_indices,
        SYSTEM_OPTIONS_ROWS,
        RowId::SysLanguage,
        language_choice_index(cfg.language_flag),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::System].choice_indices,
        SYSTEM_OPTIONS_ROWS,
        RowId::SysLogLevel,
        cfg.log_level.choice_index(),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::System].choice_indices,
        SYSTEM_OPTIONS_ROWS,
        RowId::SysLogFile,
        usize::from(cfg.log_to_file),
    );
    if let Some(noteskin_row_idx) = SYSTEM_OPTIONS_ROWS
        .iter()
        .position(|row| row.id == RowId::SysDefaultNoteSkin)
        && let Some(slot) = state.sub[SubmenuKind::System].choice_indices.get_mut(noteskin_row_idx)
    {
        *slot = machine_noteskin_idx;
    }

    set_choice_by_id(
        &mut state.sub[SubmenuKind::Graphics].choice_indices,
        GRAPHICS_OPTIONS_ROWS,
        RowId::GfxVSync,
        yes_no_choice_index(cfg.vsync),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Graphics].choice_indices,
        GRAPHICS_OPTIONS_ROWS,
        RowId::GfxPresentMode,
        cfg.present_mode_policy.choice_index(),
    );
    sync_max_fps(&mut state, cfg.max_fps);
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Graphics].choice_indices,
        GRAPHICS_OPTIONS_ROWS,
        RowId::GfxShowStats,
        cfg.show_stats_mode.min(3) as usize,
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Graphics].choice_indices,
        GRAPHICS_OPTIONS_ROWS,
        RowId::GfxValidationLayers,
        yes_no_choice_index(cfg.gfx_debug),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Graphics].choice_indices,
        GRAPHICS_OPTIONS_ROWS,
        RowId::GfxHighDpi,
        yes_no_choice_index(cfg.high_dpi),
    );
    if let Some(slot) = get_choice_by_id_mut(
        &mut state.sub[SubmenuKind::Graphics].choice_indices,
        GRAPHICS_OPTIONS_ROWS,
        RowId::GfxSoftwareThreads,
    ) {
        *slot = software_thread_choice_index(
            &state.software_thread_choices,
            cfg.software_renderer_threads,
        );
    }
    #[cfg(target_os = "windows")]
    set_choice_by_id(
        &mut state.sub[SubmenuKind::InputBackend].choice_indices,
        INPUT_BACKEND_OPTIONS_ROWS,
        RowId::InpGamepadBackend,
        windows_backend_choice_index(cfg.windows_gamepad_backend),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::InputBackend].choice_indices,
        INPUT_BACKEND_OPTIONS_ROWS,
        RowId::InpUseFsrs,
        yes_no_choice_index(cfg.use_fsrs),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::InputBackend].choice_indices,
        INPUT_BACKEND_OPTIONS_ROWS,
        RowId::InpMenuNavigation,
        usize::from(cfg.three_key_navigation),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::InputBackend].choice_indices,
        INPUT_BACKEND_OPTIONS_ROWS,
        RowId::InpOptionsNavigation,
        usize::from(cfg.arcade_options_navigation),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::InputBackend].choice_indices,
        INPUT_BACKEND_OPTIONS_ROWS,
        RowId::InpMenuButtons,
        usize::from(cfg.only_dedicated_menu_buttons),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Machine].choice_indices,
        MACHINE_OPTIONS_ROWS,
        RowId::MchSelectProfile,
        usize::from(cfg.machine_show_select_profile),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Machine].choice_indices,
        MACHINE_OPTIONS_ROWS,
        RowId::MchSelectColor,
        usize::from(cfg.machine_show_select_color),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Machine].choice_indices,
        MACHINE_OPTIONS_ROWS,
        RowId::MchSelectStyle,
        usize::from(cfg.machine_show_select_style),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Machine].choice_indices,
        MACHINE_OPTIONS_ROWS,
        RowId::MchPreferredStyle,
        cfg.machine_preferred_style.choice_index(),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Machine].choice_indices,
        MACHINE_OPTIONS_ROWS,
        RowId::MchSelectPlayMode,
        usize::from(cfg.machine_show_select_play_mode),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Machine].choice_indices,
        MACHINE_OPTIONS_ROWS,
        RowId::MchPreferredMode,
        cfg.machine_preferred_play_mode.choice_index(),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Machine].choice_indices,
        MACHINE_OPTIONS_ROWS,
        RowId::MchFont,
        cfg.machine_font.choice_index(),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Machine].choice_indices,
        MACHINE_OPTIONS_ROWS,
        RowId::MchEvalSummary,
        usize::from(cfg.machine_show_eval_summary),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Machine].choice_indices,
        MACHINE_OPTIONS_ROWS,
        RowId::MchNameEntry,
        usize::from(cfg.machine_show_name_entry),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Machine].choice_indices,
        MACHINE_OPTIONS_ROWS,
        RowId::MchGameoverScreen,
        usize::from(cfg.machine_show_gameover),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Machine].choice_indices,
        MACHINE_OPTIONS_ROWS,
        RowId::MchMenuMusic,
        usize::from(cfg.menu_music),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Machine].choice_indices,
        MACHINE_OPTIONS_ROWS,
        RowId::MchVisualStyle,
        cfg.visual_style.choice_index(),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Machine].choice_indices,
        MACHINE_OPTIONS_ROWS,
        RowId::MchReplays,
        usize::from(cfg.machine_enable_replays),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Machine].choice_indices,
        MACHINE_OPTIONS_ROWS,
        RowId::MchPerPlayerGlobalOffsets,
        usize::from(cfg.machine_allow_per_player_global_offsets),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Machine].choice_indices,
        MACHINE_OPTIONS_ROWS,
        RowId::MchKeyboardFeatures,
        usize::from(cfg.keyboard_features),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Machine].choice_indices,
        MACHINE_OPTIONS_ROWS,
        RowId::MchVideoBgs,
        usize::from(cfg.show_video_backgrounds),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Machine].choice_indices,
        MACHINE_OPTIONS_ROWS,
        RowId::MchWriteCurrentScreen,
        usize::from(cfg.write_current_screen),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Advanced].choice_indices,
        ADVANCED_OPTIONS_ROWS,
        RowId::AdvDefaultFailType,
        cfg.default_fail_type.choice_index(),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Advanced].choice_indices,
        ADVANCED_OPTIONS_ROWS,
        RowId::AdvBannerCache,
        usize::from(cfg.banner_cache),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Advanced].choice_indices,
        ADVANCED_OPTIONS_ROWS,
        RowId::AdvCdTitleCache,
        usize::from(cfg.cdtitle_cache),
    );
    if let Some(slot) = get_choice_by_id_mut(
        &mut state.sub[SubmenuKind::Advanced].choice_indices,
        ADVANCED_OPTIONS_ROWS,
        RowId::AdvSongParsingThreads,
    ) {
        *slot =
            software_thread_choice_index(&state.software_thread_choices, cfg.song_parsing_threads);
    }
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Advanced].choice_indices,
        ADVANCED_OPTIONS_ROWS,
        RowId::AdvCacheSongs,
        usize::from(cfg.cachesongs),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Advanced].choice_indices,
        ADVANCED_OPTIONS_ROWS,
        RowId::AdvFastLoad,
        usize::from(cfg.fastload),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::NullOrDieOptions].choice_indices,
        NULL_OR_DIE_OPTIONS_ROWS,
        RowId::NodSyncGraph,
        cfg.null_or_die_sync_graph.choice_index(),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::NullOrDieOptions].choice_indices,
        NULL_OR_DIE_OPTIONS_ROWS,
        RowId::NodSyncConfidence,
        sync_confidence_choice_index(cfg.null_or_die_confidence_percent),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::NullOrDieOptions].choice_indices,
        NULL_OR_DIE_OPTIONS_ROWS,
        RowId::NodPackSyncThreads,
        software_thread_choice_index(
            &state.software_thread_choices,
            cfg.null_or_die_pack_sync_threads,
        ),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::NullOrDieOptions].choice_indices,
        NULL_OR_DIE_OPTIONS_ROWS,
        RowId::NodKernelTarget,
        cfg.null_or_die_kernel_target.choice_index(),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::NullOrDieOptions].choice_indices,
        NULL_OR_DIE_OPTIONS_ROWS,
        RowId::NodKernelType,
        cfg.null_or_die_kernel_type.choice_index(),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::NullOrDieOptions].choice_indices,
        NULL_OR_DIE_OPTIONS_ROWS,
        RowId::NodFullSpectrogram,
        yes_no_choice_index(cfg.null_or_die_full_spectrogram),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Course].choice_indices,
        COURSE_OPTIONS_ROWS,
        RowId::CrsShowRandom,
        yes_no_choice_index(cfg.show_random_courses),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Course].choice_indices,
        COURSE_OPTIONS_ROWS,
        RowId::CrsShowMostPlayed,
        yes_no_choice_index(cfg.show_most_played_courses),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Course].choice_indices,
        COURSE_OPTIONS_ROWS,
        RowId::CrsShowIndividualScores,
        yes_no_choice_index(cfg.show_course_individual_scores),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Course].choice_indices,
        COURSE_OPTIONS_ROWS,
        RowId::CrsAutosubmitIndividual,
        yes_no_choice_index(cfg.autosubmit_course_scores_individually),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Gameplay].choice_indices,
        GAMEPLAY_OPTIONS_ROWS,
        RowId::GpBgBrightness,
        bg_brightness_choice_index(cfg.bg_brightness),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Gameplay].choice_indices,
        GAMEPLAY_OPTIONS_ROWS,
        RowId::GpCenteredP1,
        usize::from(cfg.center_1player_notefield),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Gameplay].choice_indices,
        GAMEPLAY_OPTIONS_ROWS,
        RowId::GpZmodRatingBox,
        usize::from(cfg.zmod_rating_box_text),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Gameplay].choice_indices,
        GAMEPLAY_OPTIONS_ROWS,
        RowId::GpBpmDecimal,
        usize::from(cfg.show_bpm_decimal),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Gameplay].choice_indices,
        GAMEPLAY_OPTIONS_ROWS,
        RowId::GpAutoScreenshot,
        auto_screenshot_cursor_index(cfg.auto_screenshot_eval),
    );

    set_choice_by_id(
        &mut state.sub[SubmenuKind::Sound].choice_indices,
        SOUND_OPTIONS_ROWS,
        RowId::SndMasterVolume,
        master_volume_choice_index(cfg.master_volume),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Sound].choice_indices,
        SOUND_OPTIONS_ROWS,
        RowId::SndSfxVolume,
        master_volume_choice_index(cfg.sfx_volume),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Sound].choice_indices,
        SOUND_OPTIONS_ROWS,
        RowId::SndAssistTickVolume,
        master_volume_choice_index(cfg.assist_tick_volume),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Sound].choice_indices,
        SOUND_OPTIONS_ROWS,
        RowId::SndMusicVolume,
        master_volume_choice_index(cfg.music_volume),
    );
    let sound_device_idx =
        sound_device_choice_index(&state.sound_device_options, cfg.audio_output_device_index);
    set_sound_choice_index(&mut state, RowId::SndDevice, sound_device_idx);
    set_sound_choice_index(
        &mut state,
        RowId::SndOutputMode,
        audio_output_mode_choice_index(cfg.audio_output_mode),
    );
    #[cfg(target_os = "linux")]
    let linux_backend_idx = linux_audio_backend_choice_index(&state, cfg.linux_audio_backend);
    #[cfg(target_os = "linux")]
    set_sound_choice_index(&mut state, RowId::SndLinuxBackend, linux_backend_idx);
    #[cfg(target_os = "linux")]
    set_sound_choice_index(
        &mut state,
        RowId::SndAlsaExclusive,
        alsa_exclusive_choice_index(cfg.audio_output_mode),
    );
    let sound_rate_idx = sample_rate_choice_index(&state, cfg.audio_sample_rate_hz);
    set_sound_choice_index(&mut state, RowId::SndSampleRate, sound_rate_idx);
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Sound].choice_indices,
        SOUND_OPTIONS_ROWS,
        RowId::SndMineSounds,
        usize::from(cfg.mine_hit_sound),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::Sound].choice_indices,
        SOUND_OPTIONS_ROWS,
        RowId::SndRateModPitch,
        usize::from(cfg.rate_mod_preserves_pitch),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::SelectMusic].choice_indices,
        SELECT_MUSIC_OPTIONS_ROWS,
        RowId::SmShowBanners,
        yes_no_choice_index(cfg.show_select_music_banners),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::SelectMusic].choice_indices,
        SELECT_MUSIC_OPTIONS_ROWS,
        RowId::SmShowVideoBanners,
        yes_no_choice_index(cfg.show_select_music_video_banners),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::SelectMusic].choice_indices,
        SELECT_MUSIC_OPTIONS_ROWS,
        RowId::SmShowBreakdown,
        yes_no_choice_index(cfg.show_select_music_breakdown),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::SelectMusic].choice_indices,
        SELECT_MUSIC_OPTIONS_ROWS,
        RowId::SmBreakdownStyle,
        cfg.select_music_breakdown_style.choice_index(),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::SelectMusic].choice_indices,
        SELECT_MUSIC_OPTIONS_ROWS,
        RowId::SmNativeLanguage,
        translated_titles_choice_index(cfg.translated_titles),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::SelectMusic].choice_indices,
        SELECT_MUSIC_OPTIONS_ROWS,
        RowId::SmWheelSpeed,
        music_wheel_scroll_speed_choice_index(cfg.music_wheel_switch_speed),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::SelectMusic].choice_indices,
        SELECT_MUSIC_OPTIONS_ROWS,
        RowId::SmWheelStyle,
        cfg.select_music_wheel_style.choice_index(),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::SelectMusic].choice_indices,
        SELECT_MUSIC_OPTIONS_ROWS,
        RowId::SmCdTitles,
        yes_no_choice_index(cfg.show_select_music_cdtitles),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::SelectMusic].choice_indices,
        SELECT_MUSIC_OPTIONS_ROWS,
        RowId::SmWheelGrades,
        yes_no_choice_index(cfg.show_music_wheel_grades),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::SelectMusic].choice_indices,
        SELECT_MUSIC_OPTIONS_ROWS,
        RowId::SmWheelLamps,
        yes_no_choice_index(cfg.show_music_wheel_lamps),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::SelectMusic].choice_indices,
        SELECT_MUSIC_OPTIONS_ROWS,
        RowId::SmWheelItlRank,
        cfg.select_music_itl_rank_mode.choice_index(),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::SelectMusic].choice_indices,
        SELECT_MUSIC_OPTIONS_ROWS,
        RowId::SmWheelItl,
        cfg.select_music_itl_wheel_mode.choice_index(),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::SelectMusic].choice_indices,
        SELECT_MUSIC_OPTIONS_ROWS,
        RowId::SmNewPackBadge,
        cfg.select_music_new_pack_mode.choice_index(),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::SelectMusic].choice_indices,
        SELECT_MUSIC_OPTIONS_ROWS,
        RowId::SmPatternInfo,
        cfg.select_music_pattern_info_mode.choice_index(),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::SelectMusic].choice_indices,
        SELECT_MUSIC_OPTIONS_ROWS,
        RowId::SmChartInfo,
        select_music_chart_info_cursor_index(
            cfg.select_music_chart_info_peak_nps,
            cfg.select_music_chart_info_matrix_rating,
        ),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::SelectMusic].choice_indices,
        SELECT_MUSIC_OPTIONS_ROWS,
        RowId::SmPreviews,
        yes_no_choice_index(cfg.show_select_music_previews),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::SelectMusic].choice_indices,
        SELECT_MUSIC_OPTIONS_ROWS,
        RowId::SmPreviewMarker,
        yes_no_choice_index(cfg.show_select_music_preview_marker),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::SelectMusic].choice_indices,
        SELECT_MUSIC_OPTIONS_ROWS,
        RowId::SmPreviewLoop,
        usize::from(cfg.select_music_preview_loop),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::SelectMusic].choice_indices,
        SELECT_MUSIC_OPTIONS_ROWS,
        RowId::SmGameplayTimer,
        yes_no_choice_index(cfg.show_select_music_gameplay_timer),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::SelectMusic].choice_indices,
        SELECT_MUSIC_OPTIONS_ROWS,
        RowId::SmShowRivals,
        yes_no_choice_index(cfg.show_select_music_scorebox),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::SelectMusic].choice_indices,
        SELECT_MUSIC_OPTIONS_ROWS,
        RowId::SmScoreboxPlacement,
        cfg.select_music_scorebox_placement.choice_index(),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::SelectMusic].choice_indices,
        SELECT_MUSIC_OPTIONS_ROWS,
        RowId::SmScoreboxCycle,
        scorebox_cycle_cursor_index(
            cfg.select_music_scorebox_cycle_itg,
            cfg.select_music_scorebox_cycle_ex,
            cfg.select_music_scorebox_cycle_hard_ex,
            cfg.select_music_scorebox_cycle_tournaments,
        ),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::GrooveStats].choice_indices,
        GROOVESTATS_OPTIONS_ROWS,
        RowId::GsEnable,
        yes_no_choice_index(cfg.enable_groovestats),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::GrooveStats].choice_indices,
        GROOVESTATS_OPTIONS_ROWS,
        RowId::GsEnableBoogie,
        yes_no_choice_index(cfg.enable_boogiestats),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::GrooveStats].choice_indices,
        GROOVESTATS_OPTIONS_ROWS,
        RowId::GsAutoPopulate,
        yes_no_choice_index(cfg.auto_populate_gs_scores),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::GrooveStats].choice_indices,
        GROOVESTATS_OPTIONS_ROWS,
        RowId::GsAutoDownloadUnlocks,
        yes_no_choice_index(cfg.auto_download_unlocks),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::GrooveStats].choice_indices,
        GROOVESTATS_OPTIONS_ROWS,
        RowId::GsSeparateUnlocks,
        yes_no_choice_index(cfg.separate_unlocks_by_player),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::ArrowCloud].choice_indices,
        ARROWCLOUD_OPTIONS_ROWS,
        RowId::AcEnable,
        yes_no_choice_index(cfg.enable_arrowcloud),
    );
    set_choice_by_id(
        &mut state.sub[SubmenuKind::ArrowCloud].choice_indices,
        ARROWCLOUD_OPTIONS_ROWS,
        RowId::AcSubmitFails,
        yes_no_choice_index(cfg.submit_arrowcloud_fails),
    );
    refresh_score_import_options(&mut state);
    refresh_null_or_die_options(&mut state);
    set_choice_by_id(
        &mut state.sub[SubmenuKind::ScoreImport].choice_indices,
        SCORE_IMPORT_OPTIONS_ROWS,
        RowId::SiOnlyMissing,
        yes_no_choice_index(false),
    );
    sync_submenu_cursor_indices(&mut state);
    state
}
