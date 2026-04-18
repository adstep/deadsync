use crate::assets::i18n::{tr, tr_fmt};
use crate::assets::{self, AssetManager};
use crate::engine::audio;
use crate::engine::space::screen_center_x;
use crate::game::parsing::noteskin::{
    self, NUM_QUANTIZATIONS, NoteAnimPart, Quantization, SpriteSlot,
};
use crate::screens::components::shared::screen_bar::{
    AvatarParams, ScreenBarParams,
};
use crate::screens::ScreenAction;
use std::time::Instant;

mod constants;
mod row_kind;
mod speed_mod;
mod layout;
mod noteskins;
mod pane_state;
mod panes;
mod profile;
mod visibility;
mod inline_nav;
mod choice;
mod input;
mod render;
mod state;

pub(crate) use constants::*;
pub(crate) use row_kind::*;
pub(crate) use speed_mod::*;
pub(crate) use layout::*;
pub(crate) use noteskins::*;
#[allow(unused_imports)]
pub(crate) use pane_state::*;
pub(crate) use panes::*;
pub(crate) use profile::*;
pub(crate) use visibility::*;
pub(crate) use inline_nav::*;
pub(crate) use choice::*;
pub(crate) use input::*;
pub(crate) use render::*;
pub(crate) use state::*;

#[cfg(test)]
mod tests;

/* ---------------------------- transitions ---------------------------- */
pub(crate) const TRANSITION_IN_DURATION: f32 = 0.4;
pub(crate) const TRANSITION_OUT_DURATION: f32 = 0.4;

// Keyboard input is handled centrally via the virtual dispatcher in app
pub fn update(state: &mut State, dt: f32, asset_manager: &AssetManager) -> Option<ScreenAction> {
    // Keep options-screen noteskin previews on a stable clock.
    // ITG/SL preview actors are not driven by selected chart BPM, so tying this to song BPM
    // makes beat-based skins (e.g. cel) appear too fast/slow depending on the selected chart.
    const PREVIEW_BPM: f32 = 120.0;
    state.preview_time += dt;
    state.preview_beat += dt * (PREVIEW_BPM / 60.0);
    let active = session_active_players();
    let now = Instant::now();
    let arcade_style = crate::config::get().arcade_options_navigation;
    let mut pending_action: Option<ScreenAction> = None;
    sync_selected_rows_with_visibility(state, active);

    // Hold-to-scroll per player.
    for player_idx in active_player_indices(active) {
        let (Some(direction), Some(held_since), Some(last_scrolled_at)) = (
            state.nav_key_held_direction[player_idx],
            state.nav_key_held_since[player_idx],
            state.nav_key_last_scrolled_at[player_idx],
        ) else {
            continue;
        };
        if now.duration_since(held_since) <= NAV_INITIAL_HOLD_DELAY
            || now.duration_since(last_scrolled_at) < NAV_REPEAT_SCROLL_INTERVAL
        {
            continue;
        }

        if state.rows().is_empty() {
            continue;
        }
        match direction {
            NavDirection::Up => {
                move_selection_vertical(state, asset_manager, active, player_idx, NavDirection::Up);
            }
            NavDirection::Down => {
                move_selection_vertical(
                    state,
                    asset_manager,
                    active,
                    player_idx,
                    NavDirection::Down,
                );
            }
            NavDirection::Left => {
                if !move_arcade_horizontal_focus(state, asset_manager, player_idx, -1) {
                    apply_choice_delta(state, asset_manager, player_idx, -1);
                }
            }
            NavDirection::Right => {
                if !move_arcade_horizontal_focus(state, asset_manager, player_idx, 1) {
                    apply_choice_delta(state, asset_manager, player_idx, 1);
                }
            }
        }
        state.nav_key_last_scrolled_at[player_idx] = Some(now);
    }

    if arcade_style {
        for player_idx in active_player_indices(active) {
            let action = repeat_held_arcade_start(state, asset_manager, active, player_idx, now);
            if pending_action.is_none() {
                pending_action = action;
            }
        }
    }

    match state.pane_transition {
        PaneTransition::None => {}
        PaneTransition::FadingOut { target, t } => {
            if PANE_FADE_SECONDS <= 0.0 {
                apply_pane(state, target);
                state.pane_transition = PaneTransition::None;
            } else {
                let next_t = (t + dt / PANE_FADE_SECONDS).min(1.0);
                if next_t >= 1.0 {
                    apply_pane(state, target);
                    state.pane_transition = PaneTransition::FadingIn { t: 0.0 };
                } else {
                    state.pane_transition = PaneTransition::FadingOut { target, t: next_t };
                }
            }
        }
        PaneTransition::FadingIn { t } => {
            if PANE_FADE_SECONDS <= 0.0 {
                state.pane_transition = PaneTransition::None;
            } else {
                let next_t = (t + dt / PANE_FADE_SECONDS).min(1.0);
                if next_t >= 1.0 {
                    state.pane_transition = PaneTransition::None;
                } else {
                    state.pane_transition = PaneTransition::FadingIn { t: next_t };
                }
            }
        }
    }

    // Advance help reveal timers.
    for player_idx in active_player_indices(active) {
        state.help_anim_time[player_idx] += dt;
    }

    // If either player is on the Combo Font row, tick the preview combo once per second.
    let mut combo_row_active = false;
    for player_idx in active_player_indices(active) {
        if let Some(row) = state.rows().get(state.selected_row()[player_idx])
            && row.id == RowId::ComboFont
        {
            combo_row_active = true;
            break;
        }
    }
    if combo_row_active {
        state.combo_preview_elapsed += dt;
        if state.combo_preview_elapsed >= 1.0 {
            state.combo_preview_elapsed -= 1.0;
            state.combo_preview_count = state.combo_preview_count.saturating_add(1);
        }
    } else {
        state.combo_preview_elapsed = 0.0;
    }

    // Row frame tweening: mimic ScreenOptions::PositionRows() + OptionRow::SetDestination()
    // so rows slide smoothly as the visible window scrolls.
    let total_rows = state.rows().len();
    let (first_row_center_y, row_step) = row_layout_params();
    if total_rows == 0 {
        state.row_tweens_mut().clear();
    } else if state.row_tweens().len() != total_rows {
        let sel = *state.selected_row();
        let hide = state.hide_active_mask;
        let eb = state.error_bar_active_mask;
        let allow = state.allow_per_player_global_offsets;
        let new_tweens = init_row_tweens(state.rows(), sel, active, hide, eb, allow);
        *state.row_tweens_mut() = new_tweens;
    } else {
        let visibility = row_visibility(
            state.rows(),
            active,
            state.hide_active_mask,
            state.error_bar_active_mask,
            state.allow_per_player_global_offsets,
        );
        let visible_rows = count_visible_rows(state.rows(), visibility);
        if visible_rows == 0 {
            let y = first_row_center_y - row_step * 0.5;
            for tw in state.row_tweens_mut() {
                let cur_y = tw.y();
                let cur_a = tw.a();
                if (y - tw.to_y).abs() > 0.01 || tw.to_a != 0.0 {
                    tw.from_y = cur_y;
                    tw.from_a = cur_a;
                    tw.to_y = y;
                    tw.to_a = 0.0;
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
        } else {
            let selected_visible = std::array::from_fn(|player_idx| {
                let row_idx = state.selected_row()[player_idx].min(total_rows.saturating_sub(1));
                row_to_visible_index(state.rows(), row_idx, visibility).unwrap_or(0)
            });
            let w = compute_row_window(visible_rows, selected_visible, active);
            let mid_pos = (VISIBLE_ROWS as f32) * 0.5 - 0.5;
            let bottom_pos = (VISIBLE_ROWS as f32) - 0.5;
            let measure_counter_anchor_visible_idx =
                parent_anchor_visible_index(state.rows(), RowId::MeasureCounter, visibility);
            let judgment_tilt_anchor_visible_idx =
                parent_anchor_visible_index(state.rows(), RowId::JudgmentTilt, visibility);
            let error_bar_anchor_visible_idx =
                parent_anchor_visible_index(state.rows(), RowId::ErrorBar, visibility);
            let hide_anchor_visible_idx =
                parent_anchor_visible_index(state.rows(), RowId::Hide, visibility);
            let mut visible_idx = 0i32;
            for i in 0..total_rows {
                let visible = is_row_visible(state.rows(), i, visibility);
                let (f_pos, hidden) = if visible {
                    let ii = visible_idx;
                    visible_idx += 1;
                    f_pos_for_visible_idx(ii, w, mid_pos, bottom_pos)
                } else {
                    let anchor =
                        state.rows()
                            .get(i)
                            .and_then(|row| match conditional_row_parent(row.id) {
                                Some(RowId::MeasureCounter) => measure_counter_anchor_visible_idx,
                                Some(RowId::JudgmentTilt) => judgment_tilt_anchor_visible_idx,
                                Some(RowId::ErrorBar) => error_bar_anchor_visible_idx,
                                Some(RowId::Hide) => hide_anchor_visible_idx,
                                _ => None,
                            });
                    if let Some(anchor_idx) = anchor {
                        let (anchor_f_pos, _) =
                            f_pos_for_visible_idx(anchor_idx, w, mid_pos, bottom_pos);
                        (anchor_f_pos, true)
                    } else {
                        (-0.5, true)
                    }
                };

                let dest_y = first_row_center_y + row_step * f_pos;
                let dest_a = if hidden { 0.0 } else { 1.0 };

                let tw = &mut state.row_tweens_mut()[i];
                let cur_y = tw.y();
                let cur_a = tw.a();
                if (dest_y - tw.to_y).abs() > 0.01 || dest_a != tw.to_a {
                    tw.from_y = cur_y;
                    tw.from_a = cur_a;
                    tw.to_y = dest_y;
                    tw.to_a = dest_a;
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
    }

    // Reset help reveal and play SFX when a player changes rows.
    for player_idx in active_player_indices(active) {
        if state.selected_row()[player_idx] == state.prev_selected_row()[player_idx] {
            continue;
        }
        match state.nav_key_held_direction[player_idx] {
            Some(NavDirection::Up) => audio::play_sfx("assets/sounds/prev_row.ogg"),
            Some(NavDirection::Down) => audio::play_sfx("assets/sounds/next_row.ogg"),
            _ => audio::play_sfx("assets/sounds/next_row.ogg"),
        }

        state.help_anim_time[player_idx] = 0.0;
        state.prev_selected_row_mut()[player_idx] = state.selected_row()[player_idx];
    }

    // Retarget cursor tween destinations to match current selection and row destinations.
    for player_idx in active_player_indices(active) {
        let Some((to_x, to_y, to_w, to_h)) =
            cursor_dest_for_player(state, asset_manager, player_idx)
        else {
            continue;
        };

        let cur = state.cursor()[player_idx];
        if !cur.initialized {
            let c = &mut state.cursor_mut()[player_idx];
            c.initialized = true;
            c.from_x = to_x;
            c.from_y = to_y;
            c.from_w = to_w;
            c.from_h = to_h;
            c.to_x = to_x;
            c.to_y = to_y;
            c.to_w = to_w;
            c.to_h = to_h;
            c.t = 1.0;
        } else {
            let dx = (to_x - cur.to_x).abs();
            let dy = (to_y - cur.to_y).abs();
            let dw = (to_w - cur.to_w).abs();
            let dh = (to_h - cur.to_h).abs();
            if dx > 0.01 || dy > 0.01 || dw > 0.01 || dh > 0.01 {
                let t = cur.t.clamp(0.0, 1.0);
                let cur_x = (cur.to_x - cur.from_x).mul_add(t, cur.from_x);
                let cur_y = (cur.to_y - cur.from_y).mul_add(t, cur.from_y);
                let cur_w = (cur.to_w - cur.from_w).mul_add(t, cur.from_w);
                let cur_h = (cur.to_h - cur.from_h).mul_add(t, cur.from_h);

                let c = &mut state.cursor_mut()[player_idx];
                c.from_x = cur_x;
                c.from_y = cur_y;
                c.from_w = cur_w;
                c.from_h = cur_h;
                c.to_x = to_x;
                c.to_y = to_y;
                c.to_w = to_w;
                c.to_h = to_h;
                c.t = 0.0;
            }
        }
    }

    // Advance cursor tween.
    for player_idx in [P1, P2] {
        let c = &mut state.cursor_mut()[player_idx];
        if c.t < 1.0 {
            if CURSOR_TWEEN_SECONDS > 0.0 {
                c.t = (c.t + dt / CURSOR_TWEEN_SECONDS).min(1.0);
            } else {
                c.t = 1.0;
            }
        }
    }

    pending_action
}

// Helpers for hold-to-scroll controlled by the app dispatcher



