use super::*;

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

pub(super) fn on_lr_press(state: &mut State, delta: isize) {
    let now = Instant::now();
    state.nav_lr_held_direction = Some(delta);
    state.nav_lr_held_since = Some(now);
    state.nav_lr_last_adjusted_at = Some(now);
}

pub(super) fn on_lr_release(state: &mut State, delta: isize) {
    if state.nav_lr_held_direction == Some(delta) {
        state.nav_lr_held_direction = None;
        state.nav_lr_held_since = None;
        state.nav_lr_last_adjusted_at = None;
    }
}

pub(super) fn apply_submenu_choice_delta(
    state: &mut State,
    asset_manager: &AssetManager,
    delta: isize,
    wrap: NavWrap,
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
        match row.behavior {
            RowBehavior::Exit => None,
            RowBehavior::Numeric(b) => apply_numeric_behavior(state, &b, delta),
            RowBehavior::Cycle(b) => {
                let new_idx =
                    advance_choice_index(state, asset_manager, kind, rows, row_index, delta, wrap)?;
                apply_cycle_binding(&b, new_idx);
                clear_render_cache(state);
                None
            }
            RowBehavior::Custom(b) => {
                let new_idx =
                    advance_choice_index(state, asset_manager, kind, rows, row_index, delta, wrap)?;
                let outcome = (b.apply)(state, new_idx);
                if outcome.changed {
                    clear_render_cache(state);
                }
                outcome.action
            }
        }
    } else {
        None
    }
}

pub(super) fn cancel_current_view(state: &mut State) -> ScreenAction {
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

pub(super) fn undo_three_key_selection(state: &mut State, asset_manager: &AssetManager) {
    match state.menu_lr_undo {
        1 => match state.view {
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
                    NavWrap::Wrap,
                );
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
                move_submenu_selection_vertical(
                    state,
                    asset_manager,
                    kind,
                    NavDirection::Up,
                    NavWrap::Wrap,
                );
            }
        },
        _ => {}
    }
}

pub(super) fn activate_current_selection(
    state: &mut State,
    asset_manager: &AssetManager,
) -> ScreenAction {
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
                && let Some(action) =
                    apply_submenu_choice_delta(state, asset_manager, 1, NavWrap::Wrap)
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
                            NavWrap::Wrap,
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
                            NavWrap::Wrap,
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
                            NavWrap::Wrap,
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
                            NavWrap::Wrap,
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
                if let Some(action) =
                    apply_submenu_choice_delta(state, asset_manager, -1, NavWrap::Wrap)
                {
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
                if let Some(action) =
                    apply_submenu_choice_delta(state, asset_manager, 1, NavWrap::Wrap)
                {
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

// ============================== RowBehavior dispatch helpers ============================

/// Advance the cycling choice index for a row by `delta`, wrapping or
/// clamping per `wrap`. Writes the new index back to the per-submenu
/// `choice_indices` and `cursor_indices` arrays, updates the inline cursor
/// x-position and plays the change-value SFX. Returns the new index, or
/// `None` if the row had no choices, was out of range, or didn't actually
/// move (clamped at boundary).
fn advance_choice_index(
    state: &mut State,
    asset_manager: &AssetManager,
    kind: SubmenuKind,
    rows: &[SubRow],
    row_index: usize,
    delta: isize,
    wrap: NavWrap,
) -> Option<usize> {
    let num_choices = row_choices(state, kind, rows, row_index).len();
    if num_choices == 0 {
        return None;
    }
    if row_index >= submenu_choice_indices(state, kind).len()
        || row_index >= submenu_cursor_indices(state, kind).len()
    {
        return None;
    }
    let choice_index =
        submenu_cursor_indices(state, kind)[row_index].min(num_choices.saturating_sub(1));
    let cur = choice_index as isize;
    let n = num_choices as isize;
    let raw = cur + delta;
    let mut new_index = match wrap {
        NavWrap::Wrap => raw.rem_euclid(n) as usize,
        NavWrap::Clamp => raw.clamp(0, n - 1) as usize,
    };
    if new_index >= num_choices {
        new_index = num_choices.saturating_sub(1);
    }
    if new_index == choice_index {
        return None;
    }
    submenu_choice_indices_mut(state, kind)[row_index] = new_index;
    submenu_cursor_indices_mut(state, kind)[row_index] = new_index;
    if let Some(layout) = submenu_row_layout(state, asset_manager, kind, row_index)
        && layout.inline_row
        && let Some(&x) = layout.centers.get(new_index)
    {
        state.sub_inline_x = x;
    }
    audio::play_sfx("assets/sounds/change_value.ogg");
    Some(new_index)
}

/// Apply a `RowBehavior::Numeric` slider press: clamp the backing field by
/// `delta`, persist via the binding, and on actual change play SFX +
/// invalidate the render cache. Always returns `None` (numeric rows never
/// emit `ScreenAction`).
fn apply_numeric_behavior(
    state: &mut State,
    binding: &NumericBinding,
    delta: isize,
) -> Option<ScreenAction> {
    let value = (binding.get_mut)(state);
    let changed = match binding.step {
        NumericStep::Ms => adjust_ms_value(value, delta, binding.min, binding.max),
        NumericStep::Tenths => adjust_tenths_value(value, delta, binding.min, binding.max),
    };
    if changed {
        let new_value = *(binding.get_mut)(state);
        (binding.persist)(new_value);
        audio::play_sfx("assets/sounds/change_value.ogg");
        clear_render_cache(state);
    }
    None
}

/// Apply a `RowBehavior::Cycle` change by dispatching to the binding's
/// persist fn. The choice index has already been advanced by
/// `advance_choice_index`; this just writes through to config.
fn apply_cycle_binding(binding: &CycleBinding, new_idx: usize) {
    match binding {
        CycleBinding::Bool(f) => f(new_idx == 1),
        CycleBinding::Index(f) => f(new_idx),
    }
}