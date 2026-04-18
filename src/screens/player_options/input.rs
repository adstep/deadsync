use crate::assets::AssetManager;
use crate::engine::audio;
use crate::engine::input::{InputEvent, VirtualAction};
use crate::screens::{Screen, ScreenAction};
use crate::screens::input as screen_input;
use std::time::Instant;

use super::*;

pub fn on_nav_press(state: &mut State, player_idx: usize, dir: NavDirection) {
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    state.scroll_focus_player = idx;
    state.nav_key_held_direction[idx] = Some(dir);
    state.nav_key_held_since[idx] = Some(Instant::now());
    state.nav_key_last_scrolled_at[idx] = Some(Instant::now());
}

pub fn on_nav_release(state: &mut State, player_idx: usize, dir: NavDirection) {
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    if state.nav_key_held_direction[idx] == Some(dir) {
        state.nav_key_held_direction[idx] = None;
        state.nav_key_held_since[idx] = None;
        state.nav_key_last_scrolled_at[idx] = None;
    }
}

#[inline(always)]
pub fn on_start_press(state: &mut State, player_idx: usize) {
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    let now = Instant::now();
    state.start_held_since[idx] = Some(now);
    state.start_last_triggered_at[idx] = Some(now);
}

#[inline(always)]
pub fn clear_start_hold(state: &mut State, player_idx: usize) {
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    state.start_held_since[idx] = None;
    state.start_last_triggered_at[idx] = None;
}

pub fn apply_pane(state: &mut State, pane: OptionsPane) {
    // Pane swap is just a swap. Each pane was built up front in `init()`
    // with its own rows, selection, and tween state, so switching panes
    // preserves cursor and scroll position automatically.
    state.current_pane = pane;
    // Cancel any in-flight Start hold so it doesn't immediately repeat
    // on the new pane.
    state.start_held_since = [None; PLAYER_SLOTS];
    state.start_last_triggered_at = [None; PLAYER_SLOTS];
}

pub fn switch_to_pane(state: &mut State, pane: OptionsPane) {
    if state.current_pane == pane {
        return;
    }
    audio::play_sfx("assets/sounds/start.ogg");

    state.nav_key_held_direction = [None; PLAYER_SLOTS];
    state.nav_key_held_since = [None; PLAYER_SLOTS];
    state.nav_key_last_scrolled_at = [None; PLAYER_SLOTS];
    state.start_held_since = [None; PLAYER_SLOTS];
    state.start_last_triggered_at = [None; PLAYER_SLOTS];

    state.pane_transition = match state.pane_transition {
        PaneTransition::FadingOut { t, .. } => PaneTransition::FadingOut { target: pane, t },
        _ => PaneTransition::FadingOut {
            target: pane,
            t: 0.0,
        },
    };
}

pub fn focus_exit_row(state: &mut State, active: [bool; PLAYER_SLOTS], player_idx: usize) {
    if state.rows().is_empty() {
        return;
    }
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    state.selected_row_mut()[idx] = state.rows().len().saturating_sub(1);
    state.arcade_row_focus_mut()[idx] = row_allows_arcade_next_row(state, state.selected_row()[idx]);
    sync_selected_rows_with_visibility(state, active);
}

#[inline(always)]
pub fn finish_start_without_action(
    state: &mut State,
    active: [bool; PLAYER_SLOTS],
    player_idx: usize,
    should_focus_exit: bool,
) -> Option<ScreenAction> {
    if should_focus_exit {
        focus_exit_row(state, active, player_idx);
    }
    None
}

pub fn handle_nav_event(
    state: &mut State,
    asset_manager: &AssetManager,
    active: [bool; PLAYER_SLOTS],
    player_idx: usize,
    dir: NavDirection,
    pressed: bool,
) {
    if !active[player_idx] || state.rows().is_empty() {
        return;
    }
    if pressed {
        sync_selected_rows_with_visibility(state, active);
        match dir {
            NavDirection::Up => {
                move_selection_vertical(state, asset_manager, active, player_idx, NavDirection::Up)
            }
            NavDirection::Down => move_selection_vertical(
                state,
                asset_manager,
                active,
                player_idx,
                NavDirection::Down,
            ),
            NavDirection::Left => {
                if !move_arcade_horizontal_focus(state, asset_manager, player_idx, -1) {
                    apply_choice_delta(state, asset_manager, player_idx, -1);
                    if arcade_row_uses_choice_focus(state, player_idx) {
                        state.arcade_row_focus_mut()[player_idx.min(PLAYER_SLOTS - 1)] = false;
                    }
                }
            }
            NavDirection::Right => {
                if !move_arcade_horizontal_focus(state, asset_manager, player_idx, 1) {
                    apply_choice_delta(state, asset_manager, player_idx, 1);
                    if arcade_row_uses_choice_focus(state, player_idx) {
                        state.arcade_row_focus_mut()[player_idx.min(PLAYER_SLOTS - 1)] = false;
                    }
                }
            }
        }
        on_nav_press(state, player_idx, dir);
    } else {
        on_nav_release(state, player_idx, dir);
    }
}

#[inline(always)]
pub fn clear_nav_hold(state: &mut State, player_idx: usize) {
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    state.nav_key_held_direction[idx] = None;
    state.nav_key_held_since[idx] = None;
    state.nav_key_last_scrolled_at[idx] = None;
}

#[inline(always)]
pub fn player_side_for_idx(player_idx: usize) -> crate::game::profile::PlayerSide {
    if player_idx == P2 {
        crate::game::profile::PlayerSide::P2
    } else {
        crate::game::profile::PlayerSide::P1
    }
}

pub fn handle_arcade_start_press(
    state: &mut State,
    asset_manager: &AssetManager,
    active: [bool; PLAYER_SLOTS],
    player_idx: usize,
    repeated: bool,
) -> Option<ScreenAction> {
    if screen_input::menu_lr_both_held(&state.menu_lr_chord, player_side_for_idx(player_idx)) {
        handle_arcade_prev_event(state, asset_manager, active, player_idx);
        return None;
    }
    if repeated && !state.rows().is_empty() {
        let idx = player_idx.min(PLAYER_SLOTS - 1);
        let row_idx = state.selected_row()[idx].min(state.rows().len().saturating_sub(1));
        if row_idx + 1 == state.rows().len() {
            return None;
        }
    }
    handle_arcade_start_event(state, asset_manager, active, player_idx)
}

pub fn repeat_held_arcade_start(
    state: &mut State,
    asset_manager: &AssetManager,
    active: [bool; PLAYER_SLOTS],
    player_idx: usize,
    now: Instant,
) -> Option<ScreenAction> {
    if !active[player_idx] {
        clear_start_hold(state, player_idx);
        return None;
    }
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    let (Some(held_since), Some(last_triggered_at)) = (
        state.start_held_since[idx],
        state.start_last_triggered_at[idx],
    ) else {
        return None;
    };
    if now.duration_since(held_since) <= NAV_INITIAL_HOLD_DELAY
        || now.duration_since(last_triggered_at) < NAV_REPEAT_SCROLL_INTERVAL
    {
        return None;
    }
    state.start_last_triggered_at[idx] = Some(now);
    handle_arcade_start_press(state, asset_manager, active, player_idx, true)
}

pub fn move_arcade_horizontal_focus(
    state: &mut State,
    asset_manager: &AssetManager,
    player_idx: usize,
    delta: isize,
) -> bool {
    if delta == 0 || state.rows().is_empty() {
        return false;
    }
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    let row_idx = state.selected_row()[idx].min(state.rows().len().saturating_sub(1));
    let Some(row) = state.rows().get(row_idx) else {
        return false;
    };
    let row_supports_inline = row_supports_inline_nav(row);
    let num_choices = row.choices.len();
    let current_choice = row
        .selected_choice_index
        .get(idx)
        .copied()
        .unwrap_or(0)
        .min(num_choices.saturating_sub(1));
    if !row_allows_arcade_next_row(state, row_idx) {
        return false;
    }
    if row_supports_inline {
        apply_choice_delta(state, asset_manager, idx, delta);
        return true;
    }
    if num_choices <= 1 {
        return false;
    }
    if state.arcade_row_focus()[idx] {
        if delta < 0 {
            return false;
        }
        state.arcade_row_focus_mut()[idx] = false;
        if current_choice == 0 {
            audio::play_sfx("assets/sounds/change_value.ogg");
        } else {
            change_choice_for_player(state, asset_manager, idx, -(current_choice as isize));
        }
        return true;
    }
    if delta < 0 {
        if current_choice == 0 {
            state.arcade_row_focus_mut()[idx] = true;
            audio::play_sfx("assets/sounds/change_value.ogg");
            return true;
        }
        change_choice_for_player(state, asset_manager, idx, -1);
        return true;
    }
    if current_choice + 1 >= num_choices {
        return false;
    }
    change_choice_for_player(state, asset_manager, idx, 1);
    true
}

pub fn handle_arcade_prev_event(
    state: &mut State,
    asset_manager: &AssetManager,
    active: [bool; PLAYER_SLOTS],
    player_idx: usize,
) {
    if !active[player_idx] || state.rows().is_empty() {
        return;
    }
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    let prev_row = state.selected_row()[idx];
    clear_nav_hold(state, player_idx);
    move_selection_vertical(state, asset_manager, active, player_idx, NavDirection::Up);
    if state.selected_row()[idx] != prev_row {
        audio::play_sfx("assets/sounds/prev_row.ogg");
        state.help_anim_time[idx] = 0.0;
        state.prev_selected_row_mut()[idx] = state.selected_row()[idx];
    }
}

pub fn handle_arcade_start_event(
    state: &mut State,
    asset_manager: &AssetManager,
    active: [bool; PLAYER_SLOTS],
    player_idx: usize,
) -> Option<ScreenAction> {
    if !active[player_idx] {
        return None;
    }
    sync_selected_rows_with_visibility(state, active);
    let num_rows = state.rows().len();
    if num_rows == 0 {
        return None;
    }
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    let row_index = state.selected_row()[idx].min(num_rows.saturating_sub(1));
    if row_index + 1 == num_rows {
        state.arcade_row_focus_mut()[idx] = row_allows_arcade_next_row(state, row_index);
        return handle_start_event(state, asset_manager, active, idx);
    }
    if arcade_row_uses_choice_focus(state, idx) && !state.arcade_row_focus()[idx] {
        let action = handle_start_event(state, asset_manager, active, idx);
        state.arcade_row_focus_mut()[idx] = row_allows_arcade_next_row(state, row_index);
        return action;
    }
    move_selection_vertical(state, asset_manager, active, idx, NavDirection::Down);
    state.arcade_row_focus_mut()[idx] = row_allows_arcade_next_row(state, state.selected_row()[idx]);
    None
}

pub fn handle_start_event(
    state: &mut State,
    asset_manager: &AssetManager,
    active: [bool; PLAYER_SLOTS],
    player_idx: usize,
) -> Option<ScreenAction> {
    if !active[player_idx] {
        return None;
    }
    sync_selected_rows_with_visibility(state, active);
    let num_rows = state.rows().len();
    if num_rows == 0 {
        return None;
    }
    let row_index = state.selected_row()[player_idx].min(num_rows.saturating_sub(1));
    let should_focus_exit = state.current_pane == OptionsPane::Main && row_index + 1 < num_rows;
    let row = state.rows().get(row_index)?;
    let id = row.id;
    let row_supports_inline = row_supports_inline_nav(row);
    if row_supports_inline {
        let changed = commit_inline_focus_selection(state, asset_manager, player_idx, row_index);
        if changed && !row_toggles_with_start(id) {
            change_choice_for_player(state, asset_manager, player_idx, 0);
            return finish_start_without_action(state, active, player_idx, should_focus_exit);
        }
    }
    // Bitmask rows route through the RowKind dispatcher.
    if matches!(state.rows()[row_index].kind, RowKind::Bitmask(_)) {
        let _ = dispatch_kind_toggle(state, player_idx, row_index);
        return finish_start_without_action(state, active, player_idx, should_focus_exit);
    }
    if id == RowId::GameplayExtrasMore {
        toggle_gameplay_extras_more_row(state, player_idx);
        return finish_start_without_action(state, active, player_idx, should_focus_exit);
    }
    if row_index == num_rows.saturating_sub(1)
        && let Some(what_comes_next_row) = state.rows().get(num_rows.saturating_sub(2))
        && what_comes_next_row.id == RowId::WhatComesNext
    {
        let choice_idx = what_comes_next_row.selected_choice_index[player_idx];
        if let Some(choice) = what_comes_next_row.choices.get(choice_idx) {
            let gameplay = tr("PlayerOptions", "WhatComesNextGameplay");
            let advanced = tr("PlayerOptions", "WhatComesNextAdvancedModifiers");
            let uncommon = tr("PlayerOptions", "WhatComesNextUncommonModifiers");
            let main_mods = tr("PlayerOptions", "WhatComesNextMainModifiers");
            let choose_different = choose_different_screen_label(state.return_screen);
            let choice_str = choice.as_str();
            if choice_str == gameplay.as_ref() {
                audio::play_sfx("assets/sounds/start.ogg");
                return Some(ScreenAction::Navigate(Screen::Gameplay));
            } else if choice_str == choose_different {
                audio::play_sfx("assets/sounds/start.ogg");
                return Some(ScreenAction::Navigate(state.return_screen));
            } else if choice_str == advanced.as_ref() {
                switch_to_pane(state, OptionsPane::Advanced);
            } else if choice_str == uncommon.as_ref() {
                switch_to_pane(state, OptionsPane::Uncommon);
            } else if choice_str == main_mods.as_ref() {
                switch_to_pane(state, OptionsPane::Main);
            }
        }
    }
    finish_start_without_action(state, active, player_idx, should_focus_exit)
}

pub fn handle_input(
    state: &mut State,
    asset_manager: &AssetManager,
    ev: &InputEvent,
) -> ScreenAction {
    let active = session_active_players();
    let dedicated_three_key = screen_input::dedicated_three_key_nav_enabled();
    let arcade_style = crate::config::get().arcade_options_navigation;
    if arcade_options_navigation_active() || dedicated_three_key {
        screen_input::track_menu_lr_chord(&mut state.menu_lr_chord, ev);
    }
    let three_key_action = (!dedicated_three_key)
        .then(|| screen_input::three_key_menu_action(&mut state.menu_lr_chord, ev))
        .flatten();
    if state.pane_transition.is_active() {
        if let Some((side, screen_input::ThreeKeyMenuAction::Cancel)) = three_key_action {
            let player_idx = screen_input::player_side_ix(side);
            if active[player_idx] {
                return ScreenAction::Navigate(state.return_screen);
            }
        }
        return match ev.action {
            VirtualAction::p1_back if ev.pressed && active[P1] => {
                ScreenAction::Navigate(state.return_screen)
            }
            VirtualAction::p2_back if ev.pressed && active[P2] => {
                ScreenAction::Navigate(state.return_screen)
            }
            _ => ScreenAction::None,
        };
    }
    if let Some((side, nav)) = three_key_action {
        let player_idx = screen_input::player_side_ix(side);
        if !active[player_idx] {
            return ScreenAction::None;
        }
        return match nav {
            screen_input::ThreeKeyMenuAction::Prev => {
                handle_nav_event(
                    state,
                    asset_manager,
                    active,
                    player_idx,
                    NavDirection::Up,
                    true,
                );
                ScreenAction::None
            }
            screen_input::ThreeKeyMenuAction::Next => {
                handle_nav_event(
                    state,
                    asset_manager,
                    active,
                    player_idx,
                    NavDirection::Down,
                    true,
                );
                ScreenAction::None
            }
            screen_input::ThreeKeyMenuAction::Confirm => {
                clear_nav_hold(state, player_idx);
                if let Some(action) = handle_start_event(state, asset_manager, active, player_idx) {
                    return action;
                }
                ScreenAction::None
            }
            screen_input::ThreeKeyMenuAction::Cancel => {
                clear_nav_hold(state, player_idx);
                ScreenAction::Navigate(state.return_screen)
            }
        };
    }
    match ev.action {
        VirtualAction::p1_back if ev.pressed && active[P1] => {
            return ScreenAction::Navigate(state.return_screen);
        }
        VirtualAction::p2_back if ev.pressed && active[P2] => {
            return ScreenAction::Navigate(state.return_screen);
        }
        VirtualAction::p1_up | VirtualAction::p1_menu_up => {
            handle_nav_event(
                state,
                asset_manager,
                active,
                P1,
                NavDirection::Up,
                ev.pressed,
            );
        }
        VirtualAction::p1_down | VirtualAction::p1_menu_down => {
            handle_nav_event(
                state,
                asset_manager,
                active,
                P1,
                NavDirection::Down,
                ev.pressed,
            );
        }
        VirtualAction::p1_left | VirtualAction::p1_menu_left => {
            handle_nav_event(
                state,
                asset_manager,
                active,
                P1,
                NavDirection::Left,
                ev.pressed,
            );
        }
        VirtualAction::p1_right | VirtualAction::p1_menu_right => {
            handle_nav_event(
                state,
                asset_manager,
                active,
                P1,
                NavDirection::Right,
                ev.pressed,
            );
        }
        VirtualAction::p1_start => {
            if !ev.pressed {
                clear_start_hold(state, P1);
                return ScreenAction::None;
            }
            if arcade_style {
                on_start_press(state, P1);
                if let Some(action) =
                    handle_arcade_start_press(state, asset_manager, active, P1, false)
                {
                    return action;
                }
                return ScreenAction::None;
            }
            if let Some(action) = handle_start_event(state, asset_manager, active, P1) {
                return action;
            }
        }
        VirtualAction::p1_select if ev.pressed && arcade_style => {
            handle_arcade_prev_event(state, asset_manager, active, P1);
            return ScreenAction::None;
        }
        VirtualAction::p2_up | VirtualAction::p2_menu_up => {
            handle_nav_event(
                state,
                asset_manager,
                active,
                P2,
                NavDirection::Up,
                ev.pressed,
            );
        }
        VirtualAction::p2_down | VirtualAction::p2_menu_down => {
            handle_nav_event(
                state,
                asset_manager,
                active,
                P2,
                NavDirection::Down,
                ev.pressed,
            );
        }
        VirtualAction::p2_left | VirtualAction::p2_menu_left => {
            handle_nav_event(
                state,
                asset_manager,
                active,
                P2,
                NavDirection::Left,
                ev.pressed,
            );
        }
        VirtualAction::p2_right | VirtualAction::p2_menu_right => {
            handle_nav_event(
                state,
                asset_manager,
                active,
                P2,
                NavDirection::Right,
                ev.pressed,
            );
        }
        VirtualAction::p2_start => {
            if !ev.pressed {
                clear_start_hold(state, P2);
                return ScreenAction::None;
            }
            if arcade_style {
                on_start_press(state, P2);
                if let Some(action) =
                    handle_arcade_start_press(state, asset_manager, active, P2, false)
                {
                    return action;
                }
                return ScreenAction::None;
            }
            if let Some(action) = handle_start_event(state, asset_manager, active, P2) {
                return action;
            }
        }
        VirtualAction::p2_select if ev.pressed && arcade_style => {
            handle_arcade_prev_event(state, asset_manager, active, P2);
            return ScreenAction::None;
        }
        _ => {}
    }
    ScreenAction::None
}
