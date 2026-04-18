use crate::assets::AssetManager;
use crate::engine::space::widescale;

use super::*;

pub fn inline_choice_centers(
    choices: &[String],
    asset_manager: &AssetManager,
    left_x: f32,
) -> Vec<f32> {
    if choices.is_empty() {
        return Vec::new();
    }
    let mut centers: Vec<f32> = Vec::with_capacity(choices.len());
    let mut x = left_x;
    let zoom = 0.835_f32;
    for text in choices {
        let (draw_w, _) = measure_option_text(asset_manager, text, zoom);
        centers.push(draw_w.mul_add(0.5, x));
        x += draw_w + INLINE_SPACING;
    }
    centers
}

pub fn focused_inline_choice_index(
    state: &State,
    asset_manager: &AssetManager,
    player_idx: usize,
    row_idx: usize,
) -> Option<usize> {
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    let row = state.rows().get(row_idx)?;
    if !row_supports_inline_nav(row) {
        return None;
    }
    let centers = inline_choice_centers(
        &row.choices,
        asset_manager,
        inline_choice_left_x_for_row(state, row_idx),
    );
    if centers.is_empty() {
        return None;
    }
    let mut focus_idx = row.selected_choice_index[idx].min(centers.len().saturating_sub(1));
    let anchor_x = state.inline_choice_x()[idx];
    if anchor_x.is_finite() {
        let mut best_dist = f32::INFINITY;
        for (i, &center_x) in centers.iter().enumerate() {
            let dist = (center_x - anchor_x).abs();
            if dist < best_dist {
                best_dist = dist;
                focus_idx = i;
            }
        }
    }
    Some(focus_idx)
}

pub fn move_inline_focus(
    state: &mut State,
    asset_manager: &AssetManager,
    player_idx: usize,
    delta: isize,
) -> bool {
    if state.rows().is_empty() || delta == 0 {
        return false;
    }
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    let row_idx = state.selected_row()[idx].min(state.rows().len().saturating_sub(1));
    let Some(row) = state.rows().get(row_idx) else {
        return false;
    };
    if !row_supports_inline_nav(row) {
        return false;
    }
    let centers = inline_choice_centers(
        &row.choices,
        asset_manager,
        inline_choice_left_x_for_row(state, row_idx),
    );
    if centers.is_empty() {
        return false;
    }
    if row_allows_arcade_next_row(state, row_idx) {
        if state.arcade_row_focus()[idx] {
            if delta <= 0 {
                return false;
            }
            state.arcade_row_focus_mut()[idx] = false;
            state.inline_choice_x_mut()[idx] = centers[0];
            return true;
        }
        let Some(current_idx) = focused_inline_choice_index(state, asset_manager, idx, row_idx)
        else {
            return false;
        };
        if delta < 0 {
            if current_idx == 0 {
                state.arcade_row_focus_mut()[idx] = true;
                state.inline_choice_x_mut()[idx] = f32::NAN;
                return true;
            }
            state.inline_choice_x_mut()[idx] = centers[current_idx - 1];
            return true;
        }
        if current_idx + 1 >= centers.len() {
            return false;
        }
        state.inline_choice_x_mut()[idx] = centers[current_idx + 1];
        return true;
    }
    let Some(current_idx) = focused_inline_choice_index(state, asset_manager, idx, row_idx) else {
        return false;
    };
    let n = centers.len() as isize;
    let next_idx = ((current_idx as isize + delta).rem_euclid(n)) as usize;
    state.inline_choice_x_mut()[idx] = centers[next_idx];
    true
}

pub fn commit_inline_focus_selection(
    state: &mut State,
    asset_manager: &AssetManager,
    player_idx: usize,
    row_idx: usize,
) -> bool {
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    let (supports_inline, row_id) = match state.rows().get(row_idx) {
        Some(row) => (row_supports_inline_nav(row), row.id),
        None => return false,
    };
    if !supports_inline {
        return false;
    }
    let Some(focus_idx) = focused_inline_choice_index(state, asset_manager, idx, row_idx) else {
        return false;
    };
    let is_shared = row_is_shared(row_id);
    if let Some(row) = state.rows_mut().get_mut(row_idx) {
        if is_shared {
            let changed = row.selected_choice_index.iter().any(|&v| v != focus_idx);
            row.selected_choice_index = [focus_idx; PLAYER_SLOTS];
            return changed;
        }
        let changed = row.selected_choice_index[idx] != focus_idx;
        row.selected_choice_index[idx] = focus_idx;
        return changed;
    }
    false
}

pub fn sync_inline_intent_from_row(
    state: &mut State,
    asset_manager: &AssetManager,
    player_idx: usize,
    row_idx: usize,
) {
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    if row_allows_arcade_next_row(state, row_idx) && state.arcade_row_focus()[idx] {
        state.inline_choice_x_mut()[idx] = f32::NAN;
        return;
    }
    let Some(row) = state.rows().get(row_idx) else {
        return;
    };
    if !row_supports_inline_nav(row) {
        return;
    }
    let centers = inline_choice_centers(
        &row.choices,
        asset_manager,
        inline_choice_left_x_for_row(state, row_idx),
    );
    if centers.is_empty() {
        return;
    }
    let sel = row.selected_choice_index[idx].min(centers.len().saturating_sub(1));
    state.inline_choice_x_mut()[idx] = centers[sel];
}

pub fn apply_inline_intent_to_row(
    state: &mut State,
    asset_manager: &AssetManager,
    player_idx: usize,
    row_idx: usize,
) {
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    if row_allows_arcade_next_row(state, row_idx) && state.arcade_row_focus()[idx] {
        state.inline_choice_x_mut()[idx] = f32::NAN;
        return;
    }
    let Some(row) = state.rows().get(row_idx) else {
        return;
    };
    if !row_supports_inline_nav(row) {
        return;
    }
    let centers = inline_choice_centers(
        &row.choices,
        asset_manager,
        inline_choice_left_x_for_row(state, row_idx),
    );
    if centers.is_empty() {
        return;
    }
    let sel = row.selected_choice_index[idx].min(centers.len().saturating_sub(1));
    if state.current_pane == OptionsPane::Main {
        state.inline_choice_x_mut()[idx] = centers[sel];
        return;
    }
    if !state.inline_choice_x()[idx].is_finite() {
        state.inline_choice_x_mut()[idx] = centers[sel];
    }
}

pub fn move_selection_vertical(
    state: &mut State,
    asset_manager: &AssetManager,
    active: [bool; PLAYER_SLOTS],
    player_idx: usize,
    dir: NavDirection,
) {
    if !matches!(dir, NavDirection::Up | NavDirection::Down) || state.rows().is_empty() {
        return;
    }
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    sync_selected_rows_with_visibility(state, active);
    let visibility = row_visibility(
        state.rows(),
        active,
        state.hide_active_mask,
        state.error_bar_active_mask,
        state.allow_per_player_global_offsets,
    );
    let current_row = state.selected_row()[idx].min(state.rows().len().saturating_sub(1));
    if !state.inline_choice_x()[idx].is_finite() {
        if let Some((anchor_x, _, _, _)) = cursor_dest_for_player(state, asset_manager, idx) {
            state.inline_choice_x_mut()[idx] = anchor_x;
        } else {
            sync_inline_intent_from_row(state, asset_manager, idx, current_row);
        }
    }
    if let Some(next_row) = next_visible_row(state.rows(), current_row, dir, visibility) {
        state.selected_row_mut()[idx] = next_row;
        state.arcade_row_focus_mut()[idx] = row_allows_arcade_next_row(state, next_row);
        apply_inline_intent_to_row(state, asset_manager, idx, next_row);
    }
}

#[inline(always)]
pub fn measure_option_text(asset_manager: &AssetManager, text: &str, zoom: f32) -> (f32, f32) {
    let mut out_w = 40.0_f32;
    let mut out_h = 16.0_f32;
    asset_manager.with_fonts(|all_fonts| {
        asset_manager.with_font("miso", |metrics_font| {
            out_h = (metrics_font.height as f32).max(1.0) * zoom;
            let mut w = crate::engine::present::font::measure_line_width_logical(
                metrics_font,
                text,
                all_fonts,
            ) as f32;
            if !w.is_finite() || w <= 0.0 {
                w = 1.0;
            }
            out_w = w * zoom;
        });
    });
    (out_w, out_h)
}

#[inline(always)]
pub fn inline_choice_left_x() -> f32 {
    widescale(162.0, 176.0)
}

#[inline(always)]
pub fn arcade_inline_choice_shift_x() -> f32 {
    widescale(6.0, 8.0)
}

#[inline(always)]
pub fn arcade_next_row_gap_x() -> f32 {
    widescale(5.0, 6.0)
}

#[inline(always)]
pub fn inline_choice_left_x_for_row(state: &State, row_idx: usize) -> f32 {
    inline_choice_left_x()
        + if row_allows_arcade_next_row(state, row_idx) {
            arcade_inline_choice_shift_x()
        } else {
            0.0
        }
}

#[inline(always)]
pub fn arcade_next_row_visible(state: &State, row_idx: usize) -> bool {
    row_allows_arcade_next_row(state, row_idx)
}

#[inline(always)]
pub fn arcade_row_focuses_next_row(state: &State, player_idx: usize, row_idx: usize) -> bool {
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    row_allows_arcade_next_row(state, row_idx)
        && state.arcade_row_focus()[idx]
        && state.selected_row()[idx] == row_idx
}

pub fn arcade_next_row_layout(
    state: &State,
    row_idx: usize,
    asset_manager: &AssetManager,
    zoom: f32,
) -> (f32, f32, f32) {
    let (draw_w, draw_h) = measure_option_text(asset_manager, ARCADE_NEXT_ROW_TEXT, zoom);
    let left_x = inline_choice_left_x_for_row(state, row_idx) - draw_w - arcade_next_row_gap_x();
    (left_x, draw_w, draw_h)
}

pub fn cursor_dest_for_player(
    state: &State,
    asset_manager: &AssetManager,
    player_idx: usize,
) -> Option<(f32, f32, f32, f32)> {
    if state.rows().is_empty() {
        return None;
    }
    let player_idx = player_idx.min(PLAYER_SLOTS - 1);
    let visibility = row_visibility(
        state.rows(),
        session_active_players(),
        state.hide_active_mask,
        state.error_bar_active_mask,
        state.allow_per_player_global_offsets,
    );
    let mut row_idx = state.selected_row()[player_idx].min(state.rows().len().saturating_sub(1));
    if !is_row_visible(state.rows(), row_idx, visibility) {
        row_idx = fallback_visible_row(state.rows(), row_idx, visibility)?;
    }
    let row = state.rows().get(row_idx)?;

    let y = state.row_tweens()
        .get(row_idx)
        .map(|tw| tw.to_y)
        .unwrap_or_else(|| {
            // Fallback (no windowing) if row tweens aren't initialized yet.
            let (y0, step) = row_layout_params();
            (row_idx as f32).mul_add(step, y0)
        });

    let value_zoom = 0.835_f32;
    let border_w = widescale(2.0, 2.5);
    let pad_y = widescale(6.0, 8.0);
    let min_pad_x = widescale(2.0, 3.0);
    let max_pad_x = widescale(22.0, 28.0);
    let width_ref = widescale(180.0, 220.0);

    let speed_mod_x = screen_center_x() + widescale(-77.0, -100.0);

    // Shared geometry for Music Rate centering (must match get_actors()).
    let help_box_w = widescale(614.0, 792.0);
    let help_box_x = widescale(13.0, 30.666);
    let row_left = help_box_x;
    let row_width = help_box_w;
    let item_col_left = row_left + TITLE_BG_WIDTH;
    let item_col_w = row_width - TITLE_BG_WIDTH;
    let music_rate_center_x = item_col_left + item_col_w * 0.5;

    if row.id == RowId::Exit {
        // Exit row is shared (OptionRowExit); its cursor is centered on Speed Mod helper X.
        let choice_text = row
            .choices
            .get(row.selected_choice_index[P1])
            .or_else(|| row.choices.first())?;
        let (draw_w, draw_h) = measure_option_text(asset_manager, choice_text, value_zoom);
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
        let ring_w = draw_w + pad_x * 2.0;
        let ring_h = draw_h + pad_y * 2.0;
        return Some((speed_mod_x, y, ring_w, ring_h));
    }

    if row_shows_all_choices_inline(row.id) {
        if row.choices.is_empty() {
            return None;
        }
        let spacing = INLINE_SPACING;
        let choice_inner_left = inline_choice_left_x_for_row(state, row_idx);
        let mut widths: Vec<f32> = Vec::with_capacity(row.choices.len());
        let mut text_h: f32 = 16.0;
        asset_manager.with_fonts(|all_fonts| {
            asset_manager.with_font("miso", |metrics_font| {
                text_h = (metrics_font.height as f32).max(1.0) * value_zoom;
                for text in &row.choices {
                    let mut w = crate::engine::present::font::measure_line_width_logical(
                        metrics_font,
                        text,
                        all_fonts,
                    ) as f32;
                    if !w.is_finite() || w <= 0.0 {
                        w = 1.0;
                    }
                    widths.push(w * value_zoom);
                }
            });
        });
        if widths.is_empty() {
            return None;
        }
        if arcade_row_focuses_next_row(state, player_idx, row_idx) {
            let (left_x, draw_w, draw_h) =
                arcade_next_row_layout(state, row_idx, asset_manager, value_zoom);
            let mut size_t = draw_w / width_ref;
            if !size_t.is_finite() {
                size_t = 0.0;
            }
            size_t = size_t.clamp(0.0, 1.0);
            let mut pad_x = (max_pad_x - min_pad_x).mul_add(size_t, min_pad_x);
            let max_pad_by_spacing = (spacing - border_w).max(min_pad_x);
            if pad_x > max_pad_by_spacing {
                pad_x = max_pad_by_spacing;
            }
            let ring_w = draw_w + pad_x * 2.0;
            let ring_h = draw_h + pad_y * 2.0;
            return Some((draw_w.mul_add(0.5, left_x), y, ring_w, ring_h));
        }

        let focus_idx = focused_inline_choice_index(state, asset_manager, player_idx, row_idx)
            .unwrap_or_else(|| row.selected_choice_index[player_idx])
            .min(widths.len().saturating_sub(1));
        let mut left_x = choice_inner_left;
        for w in widths.iter().take(focus_idx) {
            left_x += *w + spacing;
        }
        let draw_w = widths[focus_idx];
        let center_x = draw_w.mul_add(0.5, left_x);

        let mut size_t = draw_w / width_ref;
        if !size_t.is_finite() {
            size_t = 0.0;
        }
        size_t = size_t.clamp(0.0, 1.0);
        let mut pad_x = (max_pad_x - min_pad_x).mul_add(size_t, min_pad_x);
        let max_pad_by_spacing = (spacing - border_w).max(min_pad_x);
        if pad_x > max_pad_by_spacing {
            pad_x = max_pad_by_spacing;
        }
        let ring_w = draw_w + pad_x * 2.0;
        let ring_h = text_h + pad_y * 2.0;
        return Some((center_x, y, ring_w, ring_h));
    }

    // Single value rows (ShowOneInRow).
    let mut center_x = speed_mod_x;
    if row.id == RowId::MusicRate {
        center_x = music_rate_center_x;
    } else if player_idx == P2 {
        center_x = screen_center_x().mul_add(2.0, -center_x);
    }

    let display_text = if arcade_row_focuses_next_row(state, player_idx, row_idx) {
        ARCADE_NEXT_ROW_TEXT.to_string()
    } else if row.id == RowId::SpeedMod {
        state.speed_mod[player_idx].display()
    } else if row.id == RowId::TypeOfSpeedMod {
        let idx = state.speed_mod[player_idx].mod_type.type_choice_index();
        row.choices.get(idx).cloned().unwrap_or_default()
    } else {
        let idx = row.selected_choice_index[player_idx].min(row.choices.len().saturating_sub(1));
        row.choices.get(idx).cloned().unwrap_or_default()
    };

    let (draw_w, draw_h) = measure_option_text(asset_manager, &display_text, value_zoom);
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
    let ring_w = draw_w + pad_x * 2.0;
    let ring_h = draw_h + pad_y * 2.0;
    Some((center_x, y, ring_w, ring_h))
}
