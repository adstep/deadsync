use std::sync::Arc;

use crate::assets::AssetManager;
use crate::engine::present::font;
use crate::engine::space::{screen_height, screen_width, widescale};

use super::*;

pub fn format_ms(value: i32) -> String {
    // Positive values omit a '+' and compact to the Simply Love "Nms" style.
    format!("{value}ms")
}

#[inline(always)]
pub fn format_percent(value: i32) -> String {
    format!("{value}%")
}

#[inline(always)]
pub fn format_tenths_ms(value_tenths: i32) -> String {
    format!("{:.1} ms", value_tenths as f64 / 10.0)
}

#[inline(always)]
pub fn adjust_ms_value(value: &mut i32, delta: isize, min: i32, max: i32) -> bool {
    let new_value = (*value + delta as i32).clamp(min, max);
    if new_value == *value {
        false
    } else {
        *value = new_value;
        true
    }
}

#[inline(always)]
pub fn adjust_tenths_value(value: &mut i32, delta: isize, min: i32, max: i32) -> bool {
    let new_value = (*value + delta as i32).clamp(min, max);
    if new_value == *value {
        false
    } else {
        *value = new_value;
        true
    }
}

#[inline(always)]
pub fn tenths_from_f64(value: f64) -> i32 {
    let scaled = value * 10.0;
    let nudge = scaled.signum() * scaled.abs().max(1.0) * f64::EPSILON * 16.0;
    (scaled + nudge).round() as i32
}

#[inline(always)]
pub fn f64_from_tenths(value: i32) -> f64 {
    value as f64 / 10.0
}

// Keyboard input is handled centrally via the virtual dispatcher in app

/// Bars in `screen_bar.rs` use 32.0 px height.
pub const BAR_H: f32 = 32.0;

/// Screen-space margins (pixels, not scaled)
pub const LEFT_MARGIN_PX: f32 = 33.0;
pub const RIGHT_MARGIN_PX: f32 = 25.0;
pub const FIRST_ROW_TOP_MARGIN_PX: f32 = 18.0;
pub const BOTTOM_MARGIN_PX: f32 = 0.0;

/// Unscaled spec constants (we’ll uniformly scale).
pub const VISIBLE_ROWS: usize = 10; // how many rows are shown at once
// Match player_options.rs row height.
pub const ROW_H: f32 = 33.0;
pub const ROW_GAP: f32 = 2.5;
pub const SEP_W: f32 = 2.5; // gap/stripe between rows and description
// Match SL non-wide/wide block sizing used by ScreenPlayerOptions underlay.
pub const OPTIONS_BLOCK_W_43: f32 = 614.0;
pub const OPTIONS_BLOCK_W_169: f32 = 792.0;
pub const DESC_W_43: f32 = 287.0; // ScreenOptionsService overlay.lua: WideScale(287,292)
pub const DESC_W_169: f32 = 292.0;
// derive description height from visible rows so it never includes a trailing gap
pub const DESC_H: f32 = (VISIBLE_ROWS as f32) * ROW_H + ((VISIBLE_ROWS - 1) as f32) * ROW_GAP;

#[inline(always)]
pub fn desc_w_unscaled() -> f32 {
    widescale(DESC_W_43, DESC_W_169)
}

#[inline(always)]
pub fn list_w_unscaled() -> f32 {
    widescale(
        OPTIONS_BLOCK_W_43 - SEP_W - DESC_W_43,
        OPTIONS_BLOCK_W_169 - SEP_W - DESC_W_169,
    )
}

/// Left margin for row labels (in content-space pixels).
pub const TEXT_LEFT_PAD: f32 = 40.66;
/// Left margin for the heart icon (in content-space pixels).
pub const HEART_LEFT_PAD: f32 = 13.0;
/// Label text zoom, matched to the left column titles in `player_options.rs`.
pub const ITEM_TEXT_ZOOM: f32 = 0.88;
/// Width of the System Options submenu label column (content-space pixels).
pub const SUB_LABEL_COL_W: f32 = 142.5;
/// Left padding for text inside the System Options submenu label column.
pub const SUB_LABEL_TEXT_LEFT_PAD: f32 = 11.0;
/// Left padding for inline option values in the System Options submenu (content-space pixels).
pub const SUB_INLINE_ITEMS_LEFT_PAD: f32 = 13.0;
/// Horizontal offset (content-space pixels) for single-value submenu items
/// (e.g. Language and Exit) within the items column.
pub const SUB_SINGLE_VALUE_CENTER_OFFSET: f32 = -43.0;

/// Heart sprite zoom for the options list rows.
/// This is a StepMania-style "zoom" factor applied to the native heart.png size.
pub const HEART_ZOOM: f32 = 0.026;


/// Description pane layout (mirrors Simply Love's `ScreenOptionsService` overlay).
/// Title and bullet list use separate top/side padding so they can be tuned independently.
pub const DESC_TITLE_TOP_PAD_PX: f32 = 9.75; // padding from box top to title
pub const DESC_TITLE_SIDE_PAD_PX: f32 = 7.5; // left/right padding for title text
pub const DESC_BULLET_TOP_PAD_PX: f32 = 23.25; // vertical gap between title and bullet list
pub const DESC_BULLET_SIDE_PAD_PX: f32 = 7.5; // left/right padding for bullet text
pub const DESC_BULLET_INDENT_PX: f32 = 10.0; // extra indent for bullet marker + text
pub const DESC_TITLE_ZOOM: f32 = 1.0; // title text zoom (roughly header-sized)
pub const DESC_BODY_ZOOM: f32 = 1.0; // body/bullet text zoom (similar to help text)

#[inline(always)]
pub fn desc_wrap_extra_pad_unscaled() -> f32 {
    // Slightly tighter wrap in 4:3 to avoid edge clipping from font metric/render mismatch.
    widescale(6.0, 0.0)
}

#[inline(always)]
pub fn submenu_inline_widths_fit(widths: &[f32]) -> bool {
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

pub const SUBMENU_FADE_DURATION: f32 = 0.2;

pub fn build_submenu_row_layout(
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

pub fn submenu_row_layout(
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

pub fn sync_submenu_inline_x_from_row(
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

pub fn apply_submenu_inline_x_to_row(
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

pub fn move_submenu_selection_vertical(
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

pub const SOUND_VOLUME_LEVELS: [u8; 6] = [0, 10, 25, 50, 75, 100];

pub fn set_choice_by_id(choice_indices: &mut Vec<usize>, rows: &[SubRow], id: SubRowId, idx: usize) {
    if let Some(pos) = rows.iter().position(|r| r.id == id)
        && let Some(slot) = choice_indices.get_mut(pos)
    {
        let max_idx = rows[pos].choices.len().saturating_sub(1);
        *slot = idx.min(max_idx);
    }
}

pub fn scaled_block_origin_with_margins() -> (f32, f32, f32) {
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
pub fn scroll_offset(selected: usize, total_rows: usize) -> usize {
    let anchor_row: usize = 4; // keep cursor near middle (5th visible row)
    let max_offset = total_rows.saturating_sub(VISIBLE_ROWS);
    if total_rows <= VISIBLE_ROWS {
        0
    } else {
        selected.saturating_sub(anchor_row).min(max_offset)
    }
}

#[inline(always)]
pub fn row_dest_for_index(
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

pub fn init_row_tweens(total_rows: usize, selected: usize, s: f32, list_y: f32) -> Vec<RowTween> {
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

pub fn update_row_tweens(
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

pub fn update_graphics_row_tweens(state: &mut State, s: f32, list_y: f32, dt: f32) {
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

pub const fn advanced_parent_row(actual_idx: usize) -> Option<usize> {
    let _ = actual_idx;
    None
}

pub fn update_advanced_row_tweens(state: &mut State, s: f32, list_y: f32, dt: f32) {
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

pub const fn select_music_parent_row(actual_idx: usize) -> Option<usize> {
    match actual_idx {
        SELECT_MUSIC_SHOW_VIDEO_BANNERS_ROW_INDEX => Some(SELECT_MUSIC_SHOW_BANNERS_ROW_INDEX),
        SELECT_MUSIC_BREAKDOWN_STYLE_ROW_INDEX => Some(SELECT_MUSIC_SHOW_BREAKDOWN_ROW_INDEX),
        SELECT_MUSIC_PREVIEW_LOOP_ROW_INDEX => Some(SELECT_MUSIC_MUSIC_PREVIEWS_ROW_INDEX),
        SELECT_MUSIC_SCOREBOX_PLACEMENT_ROW_INDEX => Some(SELECT_MUSIC_SHOW_SCOREBOX_ROW_INDEX),
        SELECT_MUSIC_SCOREBOX_CYCLE_ROW_INDEX => Some(SELECT_MUSIC_SHOW_SCOREBOX_ROW_INDEX),
        _ => None,
    }
}

pub fn update_select_music_row_tweens(state: &mut State, s: f32, list_y: f32, dt: f32) {
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
pub fn measure_text_box(asset_manager: &AssetManager, text: &str, zoom: f32) -> (f32, f32) {
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
pub fn ring_size_for_text(draw_w: f32, text_h: f32) -> (f32, f32) {
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
pub fn row_mid_y_for_cursor(
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
pub fn wrap_miso_text(
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

pub fn build_description_layout(
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

pub fn description_layout(
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

pub fn submenu_cursor_dest(
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
