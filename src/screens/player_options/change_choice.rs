use crate::assets::AssetManager;
use crate::engine::audio;

use crate::game::profile::*;
use super::*;

pub fn change_choice_for_player(
    state: &mut State,
    asset_manager: &AssetManager,
    player_idx: usize,
    delta: isize,
) {
    if state.rows.is_empty() {
        return;
    }
    let player_idx = player_idx.min(PLAYER_SLOTS - 1);
    let row_index = state.selected_row[player_idx].min(state.rows.len().saturating_sub(1));
    let id = state.rows[row_index].id;
    if id == RowId::Exit {
        return;
    }
    let is_shared = row_is_shared(id);

    // Shared row: Music Rate
    if id == RowId::MusicRate {
        let row = &mut state.rows[row_index];
        let increment = 0.01f32;
        let min_rate = 0.05f32;
        let max_rate = 3.00f32;
        state.music_rate += delta as f32 * increment;
        state.music_rate = (state.music_rate / increment).round() * increment;
        state.music_rate = state.music_rate.clamp(min_rate, max_rate);
        row.choices[0] = fmt_music_rate(state.music_rate);

        audio::play_sfx("assets/sounds/change_value.ogg");
        crate::game::profile::set_session_music_rate(state.music_rate);
        audio::set_music_rate(state.music_rate);
        return;
    }

    // Per-player row: Speed Mod numeric
    if id == RowId::SpeedMod {
        let speed_mod = {
            let speed_mod = &mut state.speed_mod[player_idx];
            let (upper, increment) = match speed_mod.mod_type.as_str() {
                "X" => (20.0, 0.05),
                "C" | "M" => (2000.0, 5.0),
                _ => (1.0, 0.1),
            };
            speed_mod.value += delta as f32 * increment;
            speed_mod.value = (speed_mod.value / increment).round() * increment;
            speed_mod.value = speed_mod.value.clamp(increment, upper);
            speed_mod.clone()
        };
        sync_profile_scroll_speed(&mut state.player_profiles[player_idx], &speed_mod);
        audio::play_sfx("assets/sounds/change_value.ogg");
        return;
    }

    let play_style = crate::game::profile::get_session_play_style();
    let persisted_idx = session_persisted_player_idx();
    let should_persist =
        play_style == crate::game::profile::PlayStyle::Versus || player_idx == persisted_idx;
    let persist_side = if player_idx == P1 {
        crate::game::profile::PlayerSide::P1
    } else {
        crate::game::profile::PlayerSide::P2
    };

    let row = &mut state.rows[row_index];
    let num_choices = row.choices.len();
    if num_choices == 0 {
        return;
    }
    let mut visibility_changed = false;

    let current_idx = row.selected_choice_index[player_idx] as isize;
    let new_index = ((current_idx + delta + num_choices as isize) % num_choices as isize) as usize;

    if is_shared {
        row.selected_choice_index = [new_index; PLAYER_SLOTS];
    } else {
        row.selected_choice_index[player_idx] = new_index;
    }

    if id == RowId::TypeOfSpeedMod {
        let new_type = match row.selected_choice_index[player_idx] {
            0 => "X",
            1 => "C",
            2 => "M",
            _ => "C",
        };

        let speed_mod = &mut state.speed_mod[player_idx];
        let old_type = speed_mod.mod_type.clone();
        let old_value = speed_mod.value;
        let reference_bpm = reference_bpm_for_song(
            &state.song,
            resolve_p1_chart(&state.song, &state.chart_steps_index),
        );
        let rate = if state.music_rate.is_finite() && state.music_rate > 0.0 {
            state.music_rate
        } else {
            1.0
        };
        let target_bpm: f32 = match old_type.as_str() {
            "C" | "M" => old_value,
            "X" => (reference_bpm * rate * old_value).round(),
            _ => 600.0,
        };
        let new_value = match new_type {
            "X" => {
                let denom = reference_bpm * rate;
                let raw = if denom.is_finite() && denom > 0.0 {
                    target_bpm / denom
                } else {
                    1.0
                };
                let stepped = round_to_step(raw, 0.05);
                stepped.clamp(0.05, 20.0)
            }
            "C" | "M" => {
                let stepped = round_to_step(target_bpm, 5.0);
                stepped.clamp(5.0, 2000.0)
            }
            _ => 600.0,
        };
        speed_mod.mod_type = new_type.to_string();
        speed_mod.value = new_value;
        let speed_mod = speed_mod.clone();
        sync_profile_scroll_speed(&mut state.player_profiles[player_idx], &speed_mod);
    } else if id == RowId::Turn {
        let setting = TURN_OPTION_VARIANTS
            .get(row.selected_choice_index[player_idx])
            .copied()
            .unwrap_or(TurnOption::None);
        state.player_profiles[player_idx].turn_option = setting;
        if should_persist {
            crate::game::profile::update_turn_option_for_side(persist_side, setting);
        }
    } else if id == RowId::Accel || id == RowId::Effect || id == RowId::Appearance {
        // Multi-select rows toggled with Start; Left/Right only moves cursor.
    } else if id == RowId::Attacks {
        let setting = ATTACK_MODE_VARIANTS
            .get(row.selected_choice_index[player_idx])
            .copied()
            .unwrap_or(AttackMode::On);
        state.player_profiles[player_idx].attack_mode = setting;
        if should_persist {
            crate::game::profile::update_attack_mode_for_side(persist_side, setting);
        }
    } else if id == RowId::HideLightType {
        let setting = HIDE_LIGHT_TYPE_VARIANTS
            .get(row.selected_choice_index[player_idx])
            .copied()
            .unwrap_or(HideLightType::NoHideLights);
        state.player_profiles[player_idx].hide_light_type = setting;
        if should_persist {
            crate::game::profile::update_hide_light_type_for_side(persist_side, setting);
        }
    } else if id == RowId::RescoreEarlyHits {
        let enabled = row.selected_choice_index[player_idx] == 1;
        state.player_profiles[player_idx].rescore_early_hits = enabled;
        if should_persist {
            crate::game::profile::update_rescore_early_hits_for_side(persist_side, enabled);
        }
    } else if id == RowId::TimingWindows {
        let setting = TIMING_WINDOWS_VARIANTS
            .get(row.selected_choice_index[player_idx])
            .copied()
            .unwrap_or(TimingWindowsOption::None);
        state.player_profiles[player_idx].timing_windows = setting;
        if should_persist {
            crate::game::profile::update_timing_windows_for_side(persist_side, setting);
        }
    } else if id == RowId::CustomBlueFantasticWindow {
        let enabled = row.selected_choice_index[player_idx] == 1;
        state.player_profiles[player_idx].custom_fantastic_window = enabled;
        if should_persist {
            crate::game::profile::update_custom_fantastic_window_for_side(persist_side, enabled);
        }
        visibility_changed = true;
    } else if id == RowId::CustomBlueFantasticWindowMs {
        if let Some(choice) = row.choices.get(row.selected_choice_index[player_idx])
            && let Ok(raw) = choice.trim_end_matches("ms").parse::<u8>()
        {
            let ms = crate::game::profile::clamp_custom_fantastic_window_ms(raw);
            state.player_profiles[player_idx].custom_fantastic_window_ms = ms;
            if should_persist {
                crate::game::profile::update_custom_fantastic_window_ms_for_side(persist_side, ms);
            }
        }
    } else if id == RowId::MiniIndicator {
        let choice_idx =
            row.selected_choice_index[player_idx].min(row.choices.len().saturating_sub(1));
        let mini_indicator = MINI_INDICATOR_VARIANTS
            .get(choice_idx)
            .copied()
            .unwrap_or(MiniIndicator::None);
        let subtractive_scoring = mini_indicator == MiniIndicator::SubtractiveScoring;
        let pacemaker = mini_indicator == MiniIndicator::Pacemaker;
        state.player_profiles[player_idx].mini_indicator = mini_indicator;
        state.player_profiles[player_idx].subtractive_scoring = subtractive_scoring;
        state.player_profiles[player_idx].pacemaker = pacemaker;

        if should_persist {
            let profile_ref = &state.player_profiles[player_idx];
            crate::game::profile::update_mini_indicator_for_side(persist_side, mini_indicator);
            crate::game::profile::update_gameplay_extras_for_side(
                persist_side,
                profile_ref.column_flash_on_miss,
                subtractive_scoring,
                pacemaker,
                profile_ref.nps_graph_at_top,
            );
        }
        visibility_changed = true;
    } else if id == RowId::IndicatorScoreType {
        let score_type = MINI_INDICATOR_SCORE_TYPE_VARIANTS
            .get(row.selected_choice_index[player_idx])
            .copied()
            .unwrap_or(MiniIndicatorScoreType::Itg);
        state.player_profiles[player_idx].mini_indicator_score_type = score_type;
        if should_persist {
            crate::game::profile::update_mini_indicator_score_type_for_side(
                persist_side,
                score_type,
            );
        }
    } else if id == RowId::DensityGraphBackground {
        let transparent = row.selected_choice_index[player_idx] == 1;
        state.player_profiles[player_idx].transparent_density_graph_bg = transparent;
        if should_persist {
            crate::game::profile::update_transparent_density_graph_bg_for_side(
                persist_side,
                transparent,
            );
        }
    } else if id == RowId::BackgroundFilter {
        let setting = BACKGROUND_FILTER_VARIANTS
            .get(row.selected_choice_index[player_idx])
            .copied()
            .unwrap_or(BackgroundFilter::Darkest);
        state.player_profiles[player_idx].background_filter = setting;
        if should_persist {
            crate::game::profile::update_background_filter_for_side(persist_side, setting);
        }
    } else if id == RowId::Mini {
        if let Some(choice) = row.choices.get(row.selected_choice_index[player_idx]) {
            let trimmed = choice.trim_end_matches('%');
            if let Ok(val) = trimmed.parse::<i32>() {
                state.player_profiles[player_idx].mini_percent = val;
                if should_persist {
                    crate::game::profile::update_mini_percent_for_side(persist_side, val);
                }
            }
        }
    } else if id == RowId::Perspective {
        let setting = PERSPECTIVE_VARIANTS
            .get(row.selected_choice_index[player_idx])
            .copied()
            .unwrap_or(Perspective::Overhead);
        state.player_profiles[player_idx].perspective = setting;
        if should_persist {
            crate::game::profile::update_perspective_for_side(persist_side, setting);
        }
    } else if id == RowId::NoteFieldOffsetX {
        if let Some(choice) = row.choices.get(row.selected_choice_index[player_idx])
            && let Ok(raw) = choice.parse::<i32>()
        {
            state.player_profiles[player_idx].note_field_offset_x = raw;
            if should_persist {
                crate::game::profile::update_notefield_offset_x_for_side(persist_side, raw);
            }
        }
    } else if id == RowId::NoteFieldOffsetY {
        if let Some(choice) = row.choices.get(row.selected_choice_index[player_idx])
            && let Ok(raw) = choice.parse::<i32>()
        {
            state.player_profiles[player_idx].note_field_offset_y = raw;
            if should_persist {
                crate::game::profile::update_notefield_offset_y_for_side(persist_side, raw);
            }
        }
    } else if id == RowId::JudgmentOffsetX {
        if let Some(choice) = row.choices.get(row.selected_choice_index[player_idx])
            && let Ok(raw) = choice.parse::<i32>()
        {
            state.player_profiles[player_idx].judgment_offset_x = raw;
            if should_persist {
                crate::game::profile::update_judgment_offset_x_for_side(persist_side, raw);
            }
        }
    } else if id == RowId::JudgmentOffsetY {
        if let Some(choice) = row.choices.get(row.selected_choice_index[player_idx])
            && let Ok(raw) = choice.parse::<i32>()
        {
            state.player_profiles[player_idx].judgment_offset_y = raw;
            if should_persist {
                crate::game::profile::update_judgment_offset_y_for_side(persist_side, raw);
            }
        }
    } else if id == RowId::ComboOffsetX {
        if let Some(choice) = row.choices.get(row.selected_choice_index[player_idx])
            && let Ok(raw) = choice.parse::<i32>()
        {
            state.player_profiles[player_idx].combo_offset_x = raw;
            if should_persist {
                crate::game::profile::update_combo_offset_x_for_side(persist_side, raw);
            }
        }
    } else if id == RowId::ComboOffsetY {
        if let Some(choice) = row.choices.get(row.selected_choice_index[player_idx])
            && let Ok(raw) = choice.parse::<i32>()
        {
            state.player_profiles[player_idx].combo_offset_y = raw;
            if should_persist {
                crate::game::profile::update_combo_offset_y_for_side(persist_side, raw);
            }
        }
    } else if id == RowId::ErrorBarOffsetX {
        if let Some(choice) = row.choices.get(row.selected_choice_index[player_idx])
            && let Ok(raw) = choice.parse::<i32>()
        {
            state.player_profiles[player_idx].error_bar_offset_x = raw;
            if should_persist {
                crate::game::profile::update_error_bar_offset_x_for_side(persist_side, raw);
            }
        }
    } else if id == RowId::ErrorBarOffsetY {
        if let Some(choice) = row.choices.get(row.selected_choice_index[player_idx])
            && let Ok(raw) = choice.parse::<i32>()
        {
            state.player_profiles[player_idx].error_bar_offset_y = raw;
            if should_persist {
                crate::game::profile::update_error_bar_offset_y_for_side(persist_side, raw);
            }
        }
    } else if id == RowId::VisualDelay {
        if let Some(choice) = row.choices.get(row.selected_choice_index[player_idx])
            && let Ok(raw) = choice.trim_end_matches("ms").parse::<i32>()
        {
            state.player_profiles[player_idx].visual_delay_ms = raw;
            if should_persist {
                crate::game::profile::update_visual_delay_ms_for_side(persist_side, raw);
            }
        }
    } else if id == RowId::GlobalOffsetShift {
        if let Some(choice) = row.choices.get(row.selected_choice_index[player_idx])
            && let Ok(raw) = choice.trim_end_matches("ms").parse::<i32>()
        {
            state.player_profiles[player_idx].global_offset_shift_ms = raw;
            if should_persist {
                crate::game::profile::update_global_offset_shift_ms_for_side(persist_side, raw);
            }
        }
    } else if id == RowId::JudgmentTilt {
        let enabled = row.selected_choice_index[player_idx] == 1;
        state.player_profiles[player_idx].judgment_tilt = enabled;
        if should_persist {
            crate::game::profile::update_judgment_tilt_for_side(persist_side, enabled);
        }
        visibility_changed = true;
    } else if id == RowId::JudgmentTiltIntensity {
        if let Some(choice) = row.choices.get(row.selected_choice_index[player_idx])
            && let Ok(mult) = choice.parse::<f32>()
        {
            let mult = round_to_step(mult, TILT_INTENSITY_STEP)
                .clamp(TILT_INTENSITY_MIN, TILT_INTENSITY_MAX);
            state.player_profiles[player_idx].tilt_multiplier = mult;
            if should_persist {
                crate::game::profile::update_tilt_multiplier_for_side(persist_side, mult);
            }
        }
    } else if id == RowId::JudgmentBehindArrows {
        let enabled = row.selected_choice_index[player_idx] != 0;
        state.player_profiles[player_idx].judgment_back = enabled;
        if should_persist {
            crate::game::profile::update_judgment_back_for_side(persist_side, enabled);
        }
    } else if id == RowId::LifeMeterType {
        let setting = LIFE_METER_TYPE_VARIANTS
            .get(row.selected_choice_index[player_idx])
            .copied()
            .unwrap_or(LifeMeterType::Standard);
        state.player_profiles[player_idx].lifemeter_type = setting;
        if should_persist {
            crate::game::profile::update_lifemeter_type_for_side(persist_side, setting);
        }
    } else if id == RowId::LifeBarOptions {
        // Multi-select row toggled with Start; Left/Right only moves cursor.
    } else if id == RowId::DataVisualizations {
        let setting = DATA_VISUALIZATIONS_VARIANTS
            .get(row.selected_choice_index[player_idx])
            .copied()
            .unwrap_or(DataVisualizations::None);
        state.player_profiles[player_idx].data_visualizations = setting;
        if should_persist {
            crate::game::profile::update_data_visualizations_for_side(persist_side, setting);
        }
        visibility_changed = true;
    } else if id == RowId::TargetScore {
        let setting = TARGET_SCORE_VARIANTS
            .get(row.selected_choice_index[player_idx])
            .copied()
            .unwrap_or(TargetScoreSetting::S);
        state.player_profiles[player_idx].target_score = setting;
        if should_persist {
            crate::game::profile::update_target_score_for_side(persist_side, setting);
        }
    } else if id == RowId::OffsetIndicator {
        let enabled = row.selected_choice_index[player_idx] != 0;
        state.player_profiles[player_idx].error_ms_display = enabled;
        if should_persist {
            crate::game::profile::update_error_ms_display_for_side(persist_side, enabled);
        }
    } else if id == RowId::ErrorBar {
        // Multi-select row toggled with Start; Left/Right only moves cursor.
    } else if id == RowId::ErrorBarTrim {
        let setting = ERROR_BAR_TRIM_VARIANTS
            .get(row.selected_choice_index[player_idx])
            .copied()
            .unwrap_or(ErrorBarTrim::Off);
        state.player_profiles[player_idx].error_bar_trim = setting;
        if should_persist {
            crate::game::profile::update_error_bar_trim_for_side(persist_side, setting);
        }
    } else if id == RowId::MeasureCounter {
        visibility_changed = true;
        let setting = MEASURE_COUNTER_VARIANTS
            .get(row.selected_choice_index[player_idx])
            .copied()
            .unwrap_or(MeasureCounter::None);
        state.player_profiles[player_idx].measure_counter = setting;
        if should_persist {
            crate::game::profile::update_measure_counter_for_side(persist_side, setting);
        }
    } else if id == RowId::MeasureCounterLookahead {
        let lookahead = (row.selected_choice_index[player_idx] as u8).min(4);
        state.player_profiles[player_idx].measure_counter_lookahead = lookahead;
        if should_persist {
            crate::game::profile::update_measure_counter_lookahead_for_side(
                persist_side,
                lookahead,
            );
        }
    } else if id == RowId::MeasureLines {
        let setting = MEASURE_LINES_VARIANTS
            .get(row.selected_choice_index[player_idx])
            .copied()
            .unwrap_or(MeasureLines::Off);
        state.player_profiles[player_idx].measure_lines = setting;
        if should_persist {
            crate::game::profile::update_measure_lines_for_side(persist_side, setting);
        }
    } else if id == RowId::JudgmentFont {
        let setting = assets::judgment_texture_choices()
            .get(row.selected_choice_index[player_idx])
            .map(|choice| crate::game::profile::JudgmentGraphic::new(&choice.key))
            .unwrap_or_default();
        state.player_profiles[player_idx].judgment_graphic = setting;
        if should_persist {
            crate::game::profile::update_judgment_graphic_for_side(
                persist_side,
                state.player_profiles[player_idx].judgment_graphic.clone(),
            );
        }
        visibility_changed = true;
    } else if id == RowId::ComboFont {
        let setting = COMBO_FONT_VARIANTS
            .get(row.selected_choice_index[player_idx])
            .copied()
            .unwrap_or(ComboFont::Wendy);
        state.player_profiles[player_idx].combo_font = setting;
        if should_persist {
            crate::game::profile::update_combo_font_for_side(persist_side, setting);
        }
        visibility_changed = true;
    } else if id == RowId::ComboColors {
        let setting = COMBO_COLORS_VARIANTS
            .get(row.selected_choice_index[player_idx])
            .copied()
            .unwrap_or(ComboColors::Glow);
        state.player_profiles[player_idx].combo_colors = setting;
        if should_persist {
            crate::game::profile::update_combo_colors_for_side(persist_side, setting);
        }
    } else if id == RowId::ComboColorMode {
        let setting = COMBO_MODE_VARIANTS
            .get(row.selected_choice_index[player_idx])
            .copied()
            .unwrap_or(ComboMode::FullCombo);
        state.player_profiles[player_idx].combo_mode = setting;
        if should_persist {
            crate::game::profile::update_combo_mode_for_side(persist_side, setting);
        }
    } else if id == RowId::CarryCombo {
        let enabled = row.selected_choice_index[player_idx] == 1;
        state.player_profiles[player_idx].carry_combo_between_songs = enabled;
        if should_persist {
            crate::game::profile::update_carry_combo_between_songs_for_side(persist_side, enabled);
        }
    } else if id == RowId::HoldJudgment {
        let setting = assets::hold_judgment_texture_choices()
            .get(row.selected_choice_index[player_idx])
            .map(|choice| crate::game::profile::HoldJudgmentGraphic::new(&choice.key))
            .unwrap_or_default();
        state.player_profiles[player_idx].hold_judgment_graphic = setting;
        if should_persist {
            crate::game::profile::update_hold_judgment_graphic_for_side(
                persist_side,
                state.player_profiles[player_idx]
                    .hold_judgment_graphic
                    .clone(),
            );
        }
    } else if id == RowId::NoteSkin {
        let setting_name = row
            .choices
            .get(row.selected_choice_index[player_idx])
            .cloned()
            .unwrap_or_else(|| crate::game::profile::NoteSkin::DEFAULT_NAME.to_string());
        let setting = crate::game::profile::NoteSkin::new(&setting_name);
        state.player_profiles[player_idx].noteskin = setting.clone();
        if should_persist {
            crate::game::profile::update_noteskin_for_side(persist_side, setting.clone());
        }
        sync_noteskin_previews_for_player(state, player_idx);
    } else if id == RowId::MineSkin {
        let match_noteskin = tr("PlayerOptions", MATCH_NOTESKIN_LABEL);
        let selected = row
            .choices
            .get(row.selected_choice_index[player_idx])
            .map(String::as_str)
            .unwrap_or(match_noteskin.as_ref());
        let setting = if selected == match_noteskin.as_ref() {
            None
        } else {
            Some(crate::game::profile::NoteSkin::new(selected))
        };
        state.player_profiles[player_idx]
            .mine_noteskin
            .clone_from(&setting);
        if should_persist {
            crate::game::profile::update_mine_noteskin_for_side(persist_side, setting);
        }
        sync_noteskin_previews_for_player(state, player_idx);
    } else if id == RowId::ReceptorSkin {
        let match_noteskin = tr("PlayerOptions", MATCH_NOTESKIN_LABEL);
        let selected = row
            .choices
            .get(row.selected_choice_index[player_idx])
            .map(String::as_str)
            .unwrap_or(match_noteskin.as_ref());
        let setting = if selected == match_noteskin.as_ref() {
            None
        } else {
            Some(crate::game::profile::NoteSkin::new(selected))
        };
        state.player_profiles[player_idx]
            .receptor_noteskin
            .clone_from(&setting);
        if should_persist {
            crate::game::profile::update_receptor_noteskin_for_side(persist_side, setting);
        }
        sync_noteskin_previews_for_player(state, player_idx);
    } else if id == RowId::TapExplosionSkin {
        let match_noteskin = tr("PlayerOptions", MATCH_NOTESKIN_LABEL);
        let no_tap_explosion = tr("PlayerOptions", NO_TAP_EXPLOSION_LABEL);
        let selected = row
            .choices
            .get(row.selected_choice_index[player_idx])
            .map(String::as_str)
            .unwrap_or(match_noteskin.as_ref());
        let setting = if selected == match_noteskin.as_ref() {
            None
        } else if selected == no_tap_explosion.as_ref() {
            Some(crate::game::profile::NoteSkin::none_choice())
        } else {
            Some(crate::game::profile::NoteSkin::new(selected))
        };
        state.player_profiles[player_idx]
            .tap_explosion_noteskin
            .clone_from(&setting);
        if should_persist {
            crate::game::profile::update_tap_explosion_noteskin_for_side(persist_side, setting);
        }
        sync_noteskin_previews_for_player(state, player_idx);
    } else if id == RowId::Stepchart
        && let Some(diff_indices) = &row.choice_difficulty_indices
        && let Some(&difficulty_idx) = diff_indices.get(row.selected_choice_index[player_idx])
    {
        state.chart_steps_index[player_idx] = difficulty_idx;
        if difficulty_idx < crate::engine::present::color::FILE_DIFFICULTY_NAMES.len() {
            state.chart_difficulty_index[player_idx] = difficulty_idx;
        }
    }

    if visibility_changed {
        sync_selected_rows_with_visibility(state, session_active_players());
    }
    sync_inline_intent_from_row(state, asset_manager, player_idx, row_index);
    audio::play_sfx("assets/sounds/change_value.ogg");
}

pub fn apply_choice_delta(
    state: &mut State,
    asset_manager: &AssetManager,
    player_idx: usize,
    delta: isize,
) {
    if state.rows.is_empty() {
        return;
    }
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    let row_idx = state.selected_row[idx].min(state.rows.len().saturating_sub(1));
    if let Some(row) = state.rows.get(row_idx)
        && row_supports_inline_nav(row)
    {
        if state.current_pane == OptionsPane::Main || row_selects_on_focus_move(row.id) {
            change_choice_for_player(state, asset_manager, idx, delta);
            return;
        }
        if move_inline_focus(state, asset_manager, idx, delta) {
            audio::play_sfx("assets/sounds/change_value.ogg");
        }
        return;
    }
    change_choice_for_player(state, asset_manager, player_idx, delta);
}
