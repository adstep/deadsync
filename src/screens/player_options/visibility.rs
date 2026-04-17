
use super::*;

pub fn session_active_players() -> [bool; PLAYER_SLOTS] {
    let play_style = crate::game::profile::get_session_play_style();
    let side = crate::game::profile::get_session_player_side();
    let joined = [
        crate::game::profile::is_session_side_joined(crate::game::profile::PlayerSide::P1),
        crate::game::profile::is_session_side_joined(crate::game::profile::PlayerSide::P2),
    ];
    let joined_count = usize::from(joined[P1]) + usize::from(joined[P2]);
    match play_style {
        crate::game::profile::PlayStyle::Versus => {
            if joined_count > 0 {
                joined
            } else {
                [true, true]
            }
        }
        crate::game::profile::PlayStyle::Single | crate::game::profile::PlayStyle::Double => {
            if joined_count == 1 {
                joined
            } else {
                match side {
                    crate::game::profile::PlayerSide::P1 => [true, false],
                    crate::game::profile::PlayerSide::P2 => [false, true],
                }
            }
        }
    }
}

#[inline(always)]
pub fn arcade_options_navigation_active() -> bool {
    crate::config::get().arcade_options_navigation
}

#[inline(always)]
pub const fn pane_uses_arcade_next_row(pane: OptionsPane) -> bool {
    !matches!(pane, OptionsPane::Main)
}

#[inline(always)]
pub fn session_persisted_player_idx() -> usize {
    let play_style = crate::game::profile::get_session_play_style();
    let side = crate::game::profile::get_session_player_side();
    match play_style {
        crate::game::profile::PlayStyle::Versus => P1,
        crate::game::profile::PlayStyle::Single | crate::game::profile::PlayStyle::Double => {
            match side {
                crate::game::profile::PlayerSide::P1 => P1,
                crate::game::profile::PlayerSide::P2 => P2,
            }
        }
    }
}

pub const ARCADE_NEXT_ROW_TEXT: &str = "▼";

#[derive(Clone, Copy, Debug)]
pub struct RowVisibility {
    show_measure_counter_children: bool,
    show_judgment_offsets: bool,
    show_judgment_tilt_intensity: bool,
    show_combo_offsets: bool,
    show_error_bar_children: bool,
    show_custom_fantastic_window_ms: bool,
    show_density_graph_background: bool,
    show_combo_rows: bool,
    show_lifebar_rows: bool,
    show_indicator_score_type: bool,
    show_global_offset_shift: bool,
}

#[inline(always)]
pub fn row_visible_with_flags(id: RowId, visibility: RowVisibility) -> bool {
    if id == RowId::MeasureCounterLookahead || id == RowId::MeasureCounterOptions {
        return visibility.show_measure_counter_children;
    }
    if id == RowId::JudgmentOffsetX || id == RowId::JudgmentOffsetY {
        return visibility.show_judgment_offsets;
    }
    if id == RowId::JudgmentTiltIntensity {
        return visibility.show_judgment_tilt_intensity;
    }
    if id == RowId::ComboOffsetX || id == RowId::ComboOffsetY {
        return visibility.show_combo_offsets;
    }
    if id == RowId::ErrorBarTrim
        || id == RowId::ErrorBarOptions
        || id == RowId::ErrorBarOffsetX
        || id == RowId::ErrorBarOffsetY
    {
        return visibility.show_error_bar_children;
    }
    if id == RowId::CustomBlueFantasticWindowMs {
        return visibility.show_custom_fantastic_window_ms;
    }
    if id == RowId::DensityGraphBackground {
        return visibility.show_density_graph_background;
    }
    if id == RowId::ComboColors || id == RowId::ComboColorMode || id == RowId::CarryCombo {
        return visibility.show_combo_rows;
    }
    if id == RowId::LifeMeterType || id == RowId::LifeBarOptions {
        return visibility.show_lifebar_rows;
    }
    if id == RowId::IndicatorScoreType {
        return visibility.show_indicator_score_type;
    }
    if id == RowId::GlobalOffsetShift {
        return visibility.show_global_offset_shift;
    }
    true
}

#[inline(always)]
pub fn conditional_row_parent(id: RowId) -> Option<RowId> {
    if id == RowId::MeasureCounterLookahead || id == RowId::MeasureCounterOptions {
        return Some(RowId::MeasureCounter);
    }
    if id == RowId::JudgmentOffsetX || id == RowId::JudgmentOffsetY {
        return Some(RowId::JudgmentFont);
    }
    if id == RowId::JudgmentTiltIntensity {
        return Some(RowId::JudgmentTilt);
    }
    if id == RowId::ComboOffsetX || id == RowId::ComboOffsetY {
        return Some(RowId::ComboFont);
    }
    if id == RowId::ErrorBarTrim
        || id == RowId::ErrorBarOptions
        || id == RowId::ErrorBarOffsetX
        || id == RowId::ErrorBarOffsetY
    {
        return Some(RowId::ErrorBar);
    }
    if id == RowId::CustomBlueFantasticWindowMs {
        return Some(RowId::CustomBlueFantasticWindow);
    }
    if id == RowId::DensityGraphBackground {
        return Some(RowId::DataVisualizations);
    }
    if id == RowId::ComboColors
        || id == RowId::ComboColorMode
        || id == RowId::CarryCombo
        || id == RowId::LifeMeterType
        || id == RowId::LifeBarOptions
    {
        return Some(RowId::Hide);
    }
    if id == RowId::IndicatorScoreType {
        return Some(RowId::MiniIndicator);
    }
    None
}

pub fn measure_counter_children_visible(rows: &[Row], active: [bool; PLAYER_SLOTS]) -> bool {
    let Some(row) = rows.iter().find(|r| r.id == RowId::MeasureCounter) else {
        return true;
    };
    let max_choice = row.choices.len().saturating_sub(1);
    let mut any_active = false;
    for player_idx in active_player_indices(active) {
        any_active = true;
        let choice_idx = row.selected_choice_index[player_idx].min(max_choice);
        if choice_idx != 0 {
            return true;
        }
    }
    !any_active
}

pub fn judgment_offsets_visible(rows: &[Row], active: [bool; PLAYER_SLOTS]) -> bool {
    let Some(row) = rows.iter().find(|r| r.id == RowId::JudgmentFont) else {
        return true;
    };
    let max_choice = row.choices.len().saturating_sub(1);
    let mut any_active = false;
    for player_idx in active_player_indices(active) {
        any_active = true;
        let choice_idx = row.selected_choice_index[player_idx].min(max_choice);
        // "None" is always the last choice for font/texture rows.
        if choice_idx != max_choice {
            return true;
        }
    }
    !any_active
}

#[inline(always)]
pub fn judgment_tilt_intensity_visible(rows: &[Row], active: [bool; PLAYER_SLOTS]) -> bool {
    let Some(row) = rows.iter().find(|r| r.id == RowId::JudgmentTilt) else {
        return true;
    };
    let max_choice = row.choices.len().saturating_sub(1);
    let mut any_active = false;
    for player_idx in active_player_indices(active) {
        any_active = true;
        let choice_idx = row.selected_choice_index[player_idx].min(max_choice);
        if choice_idx != 0 {
            return true;
        }
    }
    !any_active
}

pub fn combo_offsets_visible(rows: &[Row], active: [bool; PLAYER_SLOTS]) -> bool {
    let Some(row) = rows.iter().find(|r| r.id == RowId::ComboFont) else {
        return true;
    };
    let max_choice = row.choices.len().saturating_sub(1);
    let mut any_active = false;
    for player_idx in active_player_indices(active) {
        any_active = true;
        let choice_idx = row.selected_choice_index[player_idx].min(max_choice);
        // "None" is always the last choice for font/texture rows.
        if choice_idx != max_choice {
            return true;
        }
    }
    !any_active
}

pub fn error_bar_children_visible(
    active: [bool; PLAYER_SLOTS],
    error_bar_active_mask: [u8; PLAYER_SLOTS],
) -> bool {
    let mut any_active = false;
    for player_idx in active_player_indices(active) {
        any_active = true;
        if crate::game::profile::normalize_error_bar_mask(error_bar_active_mask[player_idx]) != 0 {
            return true;
        }
    }
    !any_active
}

pub fn custom_fantastic_window_ms_visible(rows: &[Row], active: [bool; PLAYER_SLOTS]) -> bool {
    let Some(row) = rows
        .iter()
        .find(|r| r.id == RowId::CustomBlueFantasticWindow)
    else {
        return true;
    };
    let max_choice = row.choices.len().saturating_sub(1);
    let mut any_active = false;
    for player_idx in active_player_indices(active) {
        any_active = true;
        let choice_idx = row.selected_choice_index[player_idx].min(max_choice);
        if choice_idx != 0 {
            return true;
        }
    }
    !any_active
}

pub fn density_graph_background_visible(rows: &[Row], active: [bool; PLAYER_SLOTS]) -> bool {
    let Some(row) = rows.iter().find(|r| r.id == RowId::DataVisualizations) else {
        return true;
    };
    let max_choice = row.choices.len().saturating_sub(1);
    let mut any_active = false;
    for player_idx in active_player_indices(active) {
        any_active = true;
        let choice_idx = row.selected_choice_index[player_idx].min(max_choice);
        if choice_idx == 2 {
            return true;
        }
    }
    !any_active
}

pub fn combo_rows_visible(active: [bool; PLAYER_SLOTS], hide_active_mask: [u8; PLAYER_SLOTS]) -> bool {
    let mut any_active = false;
    for player_idx in active_player_indices(active) {
        any_active = true;
        let hide_combo = (hide_active_mask[player_idx] & (1u8 << 2)) != 0;
        if !hide_combo {
            return true;
        }
    }
    !any_active
}

pub fn lifebar_rows_visible(
    active: [bool; PLAYER_SLOTS],
    hide_active_mask: [u8; PLAYER_SLOTS],
) -> bool {
    let mut any_active = false;
    for player_idx in active_player_indices(active) {
        any_active = true;
        let hide_lifebar = (hide_active_mask[player_idx] & (1u8 << 3)) != 0;
        if !hide_lifebar {
            return true;
        }
    }
    !any_active
}

pub fn indicator_score_type_visible(rows: &[Row], active: [bool; PLAYER_SLOTS]) -> bool {
    let Some(row) = rows.iter().find(|r| r.id == RowId::MiniIndicator) else {
        return true;
    };
    let max_choice = row.choices.len().saturating_sub(1);
    let mut any_active = false;
    for player_idx in active_player_indices(active) {
        any_active = true;
        let choice_idx = row.selected_choice_index[player_idx].min(max_choice);
        // Visible for Subtractive(1), Predictive(2), Pace(3)
        if (1..=3).contains(&choice_idx) {
            return true;
        }
    }
    !any_active
}

#[inline(always)]
pub fn row_visibility(
    rows: &[Row],
    active: [bool; PLAYER_SLOTS],
    hide_active_mask: [u8; PLAYER_SLOTS],
    error_bar_active_mask: [u8; PLAYER_SLOTS],
    allow_per_player_global_offsets: bool,
) -> RowVisibility {
    RowVisibility {
        show_measure_counter_children: measure_counter_children_visible(rows, active),
        show_judgment_offsets: judgment_offsets_visible(rows, active),
        show_judgment_tilt_intensity: judgment_tilt_intensity_visible(rows, active),
        show_combo_offsets: combo_offsets_visible(rows, active),
        show_error_bar_children: error_bar_children_visible(active, error_bar_active_mask),
        show_custom_fantastic_window_ms: custom_fantastic_window_ms_visible(rows, active),
        show_density_graph_background: density_graph_background_visible(rows, active),
        show_combo_rows: combo_rows_visible(active, hide_active_mask),
        show_lifebar_rows: lifebar_rows_visible(active, hide_active_mask),
        show_indicator_score_type: indicator_score_type_visible(rows, active),
        show_global_offset_shift: allow_per_player_global_offsets,
    }
}

#[inline(always)]
pub fn is_row_visible(rows: &[Row], row_idx: usize, visibility: RowVisibility) -> bool {
    rows.get(row_idx)
        .is_some_and(|row| row_visible_with_flags(row.id, visibility))
}

pub fn count_visible_rows(rows: &[Row], visibility: RowVisibility) -> usize {
    rows.iter()
        .filter(|row| row_visible_with_flags(row.id, visibility))
        .count()
}

pub fn row_to_visible_index(rows: &[Row], row_idx: usize, visibility: RowVisibility) -> Option<usize> {
    if row_idx >= rows.len() {
        return None;
    }
    if !is_row_visible(rows, row_idx, visibility) {
        return None;
    }
    let mut pos = 0usize;
    for i in 0..row_idx {
        if is_row_visible(rows, i, visibility) {
            pos += 1;
        }
    }
    Some(pos)
}

pub fn fallback_visible_row(rows: &[Row], row_idx: usize, visibility: RowVisibility) -> Option<usize> {
    if rows.is_empty() {
        return None;
    }
    let start = row_idx.min(rows.len().saturating_sub(1));
    for i in start..rows.len() {
        if is_row_visible(rows, i, visibility) {
            return Some(i);
        }
    }
    (0..start)
        .rev()
        .find(|&i| is_row_visible(rows, i, visibility))
}

pub fn next_visible_row(
    rows: &[Row],
    current_row: usize,
    dir: NavDirection,
    visibility: RowVisibility,
) -> Option<usize> {
    if rows.is_empty() {
        return None;
    }
    let len = rows.len();
    let mut idx = current_row.min(len.saturating_sub(1));
    if !is_row_visible(rows, idx, visibility) {
        idx = fallback_visible_row(rows, idx, visibility)?;
    }
    for _ in 0..len {
        idx = match dir {
            NavDirection::Up => (idx + len - 1) % len,
            NavDirection::Down => (idx + 1) % len,
            NavDirection::Left | NavDirection::Right => return Some(idx),
        };
        if is_row_visible(rows, idx, visibility) {
            return Some(idx);
        }
    }
    None
}

pub fn parent_anchor_visible_index(
    rows: &[Row],
    parent_id: RowId,
    visibility: RowVisibility,
) -> Option<i32> {
    rows.iter()
        .position(|row| row.id == parent_id)
        .and_then(|idx| row_to_visible_index(rows, idx, visibility))
        .map(|idx| idx as i32)
}

#[inline(always)]
pub fn f_pos_for_visible_idx(
    visible_idx: i32,
    window: RowWindow,
    mid_pos: f32,
    bottom_pos: f32,
) -> (f32, bool) {
    let hidden_above = visible_idx < window.first_start;
    let hidden_mid = visible_idx >= window.first_end && visible_idx < window.second_start;
    let hidden_below = visible_idx >= window.second_end;
    if hidden_above {
        return (-0.5, true);
    }
    if hidden_mid {
        return (mid_pos, true);
    }
    if hidden_below {
        return (bottom_pos, true);
    }

    let shown_pos = if visible_idx < window.first_end {
        visible_idx - window.first_start
    } else {
        (window.first_end - window.first_start) + (visible_idx - window.second_start)
    };
    (shown_pos as f32, false)
}

pub fn sync_selected_rows_with_visibility(state: &mut State, active: [bool; PLAYER_SLOTS]) {
    if state.rows.is_empty() {
        state.selected_row = [0; PLAYER_SLOTS];
        state.prev_selected_row = [0; PLAYER_SLOTS];
        return;
    }
    let visibility = row_visibility(
        &state.rows,
        active,
        state.hide_active_mask,
        state.error_bar_active_mask,
        state.allow_per_player_global_offsets,
    );
    for player_idx in [P1, P2] {
        let idx = state.selected_row[player_idx].min(state.rows.len().saturating_sub(1));
        if is_row_visible(&state.rows, idx, visibility) {
            state.selected_row[player_idx] = idx;
            continue;
        }
        if let Some(fallback) = fallback_visible_row(&state.rows, idx, visibility) {
            state.selected_row[player_idx] = fallback;
            if active[player_idx] {
                state.prev_selected_row[player_idx] = fallback;
            }
        }
    }
}

#[inline(always)]
pub fn row_is_shared(id: RowId) -> bool {
    id == RowId::Exit || id == RowId::WhatComesNext || id == RowId::MusicRate
}

#[inline(always)]
pub fn row_shows_all_choices_inline(id: RowId) -> bool {
    id == RowId::Perspective
        || id == RowId::BackgroundFilter
        || id == RowId::Stepchart
        || id == RowId::WhatComesNext
        || id == RowId::ActionOnMissedTarget
        || id == RowId::ErrorBar
        || id == RowId::ErrorBarTrim
        || id == RowId::ErrorBarOptions
        || id == RowId::OffsetIndicator
        || id == RowId::JudgmentBehindArrows
        || id == RowId::MeasureCounter
        || id == RowId::MeasureCounterLookahead
        || id == RowId::MeasureCounterOptions
        || id == RowId::MeasureLines
        || id == RowId::TimingWindows
        || id == RowId::JudgmentTilt
        || id == RowId::MiniIndicator
        || id == RowId::IndicatorScoreType
        || id == RowId::Turn
        || id == RowId::Scroll
        || id == RowId::Hide
        || id == RowId::LifeMeterType
        || id == RowId::LifeBarOptions
        || id == RowId::DataVisualizations
        || id == RowId::DensityGraphBackground
        || id == RowId::ComboColors
        || id == RowId::ComboColorMode
        || id == RowId::CarryCombo
        || id == RowId::GameplayExtras
        || id == RowId::GameplayExtrasMore
        || id == RowId::ResultsExtras
        || id == RowId::RescoreEarlyHits
        || id == RowId::CustomBlueFantasticWindow
        || id == RowId::EarlyDecentWayOffOptions
        || id == RowId::FAPlusOptions
        || id == RowId::Insert
        || id == RowId::Remove
        || id == RowId::Holds
        || id == RowId::Accel
        || id == RowId::Effect
        || id == RowId::Appearance
        || id == RowId::Attacks
        || id == RowId::HideLightType
}

#[inline(always)]
pub fn row_supports_inline_nav(row: &Row) -> bool {
    !row.choices.is_empty() && row_shows_all_choices_inline(row.id)
}

#[inline(always)]
pub fn row_toggles_with_start(id: RowId) -> bool {
    id == RowId::Scroll
        || id == RowId::Hide
        || id == RowId::Insert
        || id == RowId::Remove
        || id == RowId::Holds
        || id == RowId::Accel
        || id == RowId::Effect
        || id == RowId::Appearance
        || id == RowId::LifeBarOptions
        || id == RowId::GameplayExtras
        || id == RowId::GameplayExtrasMore
        || id == RowId::ResultsExtras
        || id == RowId::ErrorBar
        || id == RowId::ErrorBarOptions
        || id == RowId::MeasureCounterOptions
        || id == RowId::FAPlusOptions
        || id == RowId::EarlyDecentWayOffOptions
}

#[inline(always)]
pub fn row_selects_on_focus_move(id: RowId) -> bool {
    id == RowId::Stepchart
}

#[inline(always)]
pub fn row_allows_arcade_next_row(state: &State, row_idx: usize) -> bool {
    arcade_options_navigation_active()
        && pane_uses_arcade_next_row(state.current_pane)
        && state
            .rows
            .get(row_idx)
            .is_some_and(|row| row.id != RowId::Exit && row_supports_inline_nav(row))
}

#[inline(always)]
pub fn arcade_row_uses_choice_focus(state: &State, player_idx: usize) -> bool {
    if !arcade_options_navigation_active() || !pane_uses_arcade_next_row(state.current_pane) {
        return false;
    }
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    let row_idx = state.selected_row[idx].min(state.rows.len().saturating_sub(1));
    state.rows.get(row_idx).is_some_and(row_supports_inline_nav)
}
