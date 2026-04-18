use crate::assets::AssetManager;
use crate::engine::audio;

use super::*;

pub fn change_choice_for_player(
    state: &mut State,
    asset_manager: &AssetManager,
    player_idx: usize,
    delta: isize,
) {
    if state.rows().is_empty() {
        return;
    }
    let player_idx = player_idx.min(PLAYER_SLOTS - 1);
    let row_index = state.selected_row()[player_idx].min(state.rows().len().saturating_sub(1));

    let outcome = dispatch_kind_delta(state, player_idx, row_index, delta);
    if outcome.changed_visibility {
        sync_selected_rows_with_visibility(state, session_active_players());
    }
    if outcome.persisted {
        sync_inline_intent_from_row(state, asset_manager, player_idx, row_index);
        audio::play_sfx("assets/sounds/change_value.ogg");
    }
}

pub(super) fn apply_choice_delta(
    state: &mut State,
    asset_manager: &AssetManager,
    player_idx: usize,
    delta: isize,
) {
    if state.rows().is_empty() {
        return;
    }
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    let row_idx = state.selected_row()[idx].min(state.rows().len().saturating_sub(1));
    if let Some(row) = state.rows().get(row_idx)
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

pub(super) fn toggle_scroll_row(state: &mut State, player_idx: usize) {
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    let row_index = state.selected_row()[idx];
    if let Some(row) = state.rows().get(row_index) {
        if row.id != RowId::Scroll {
            return;
        }
    } else {
        return;
    }

    let choice_index = state.rows()[row_index].selected_choice_index[idx];
    let bit = if choice_index < 8 {
        1u8 << (choice_index as u8)
    } else {
        0
    };
    if bit == 0 {
        return;
    }

    // Toggle this bit in the local mask.
    if (state.scroll_active_mask[idx] & bit) != 0 {
        state.scroll_active_mask[idx] &= !bit;
    } else {
        state.scroll_active_mask[idx] |= bit;
    }

    // Rebuild the ScrollOption bitmask from the active choices.
    use crate::game::profile::ScrollOption;
    let mut setting = ScrollOption::Normal;
    if state.scroll_active_mask[idx] != 0 {
        if (state.scroll_active_mask[idx] & (1u8 << 0)) != 0 {
            setting = setting.union(ScrollOption::Reverse);
        }
        if (state.scroll_active_mask[idx] & (1u8 << 1)) != 0 {
            setting = setting.union(ScrollOption::Split);
        }
        if (state.scroll_active_mask[idx] & (1u8 << 2)) != 0 {
            setting = setting.union(ScrollOption::Alternate);
        }
        if (state.scroll_active_mask[idx] & (1u8 << 3)) != 0 {
            setting = setting.union(ScrollOption::Cross);
        }
        if (state.scroll_active_mask[idx] & (1u8 << 4)) != 0 {
            setting = setting.union(ScrollOption::Centered);
        }
    }
    state.player_profiles[idx].scroll_option = setting;
    state.player_profiles[idx].reverse_scroll = setting.contains(ScrollOption::Reverse);
    let play_style = crate::game::profile::get_session_play_style();
    let should_persist = play_style == crate::game::profile::PlayStyle::Versus
        || idx == session_persisted_player_idx();
    if should_persist {
        let side = if idx == P1 {
            crate::game::profile::PlayerSide::P1
        } else {
            crate::game::profile::PlayerSide::P2
        };
        crate::game::profile::update_scroll_option_for_side(side, setting);
    }
    audio::play_sfx("assets/sounds/change_value.ogg");
}

pub(super) fn toggle_hide_row(state: &mut State, player_idx: usize) {
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    let row_index = state.selected_row()[idx];
    if let Some(row) = state.rows().get(row_index) {
        if row.id != RowId::Hide {
            return;
        }
    } else {
        return;
    }

    let choice_index = state.rows()[row_index].selected_choice_index[idx];
    let bit = if choice_index < 8 {
        1u8 << (choice_index as u8)
    } else {
        0
    };
    if bit == 0 {
        return;
    }

    if (state.hide_active_mask[idx] & bit) != 0 {
        state.hide_active_mask[idx] &= !bit;
    } else {
        state.hide_active_mask[idx] |= bit;
    }

    let hide_targets = (state.hide_active_mask[idx] & (1u8 << 0)) != 0;
    let hide_song_bg = (state.hide_active_mask[idx] & (1u8 << 1)) != 0;
    let hide_combo = (state.hide_active_mask[idx] & (1u8 << 2)) != 0;
    let hide_lifebar = (state.hide_active_mask[idx] & (1u8 << 3)) != 0;
    let hide_score = (state.hide_active_mask[idx] & (1u8 << 4)) != 0;
    let hide_danger = (state.hide_active_mask[idx] & (1u8 << 5)) != 0;
    let hide_combo_explosions = (state.hide_active_mask[idx] & (1u8 << 6)) != 0;

    state.player_profiles[idx].hide_targets = hide_targets;
    state.player_profiles[idx].hide_song_bg = hide_song_bg;
    state.player_profiles[idx].hide_combo = hide_combo;
    state.player_profiles[idx].hide_lifebar = hide_lifebar;
    state.player_profiles[idx].hide_score = hide_score;
    state.player_profiles[idx].hide_danger = hide_danger;
    state.player_profiles[idx].hide_combo_explosions = hide_combo_explosions;

    let play_style = crate::game::profile::get_session_play_style();
    let should_persist = play_style == crate::game::profile::PlayStyle::Versus
        || idx == session_persisted_player_idx();
    if should_persist {
        let side = if idx == P1 {
            crate::game::profile::PlayerSide::P1
        } else {
            crate::game::profile::PlayerSide::P2
        };
        crate::game::profile::update_hide_options_for_side(
            side,
            hide_targets,
            hide_song_bg,
            hide_combo,
            hide_lifebar,
            hide_score,
            hide_danger,
            hide_combo_explosions,
        );
    }

    sync_selected_rows_with_visibility(state, session_active_players());
    audio::play_sfx("assets/sounds/change_value.ogg");
}

pub(super) fn toggle_insert_row(state: &mut State, player_idx: usize) {
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    let row_index = state.selected_row()[idx];
    if let Some(row) = state.rows().get(row_index) {
        if row.id != RowId::Insert {
            return;
        }
    } else {
        return;
    }

    let choice_index = state.rows()[row_index].selected_choice_index[idx];
    let bit = if choice_index < 7 {
        1u8 << (choice_index as u8)
    } else {
        0
    };
    if bit == 0 {
        return;
    }

    if (state.insert_active_mask[idx] & bit) != 0 {
        state.insert_active_mask[idx] &= !bit;
    } else {
        state.insert_active_mask[idx] |= bit;
    }
    state.insert_active_mask[idx] =
        crate::game::profile::normalize_insert_mask(state.insert_active_mask[idx]);
    let mask = state.insert_active_mask[idx];
    state.player_profiles[idx].insert_active_mask = mask;

    let play_style = crate::game::profile::get_session_play_style();
    let should_persist = play_style == crate::game::profile::PlayStyle::Versus
        || idx == session_persisted_player_idx();
    if should_persist {
        let side = if idx == P1 {
            crate::game::profile::PlayerSide::P1
        } else {
            crate::game::profile::PlayerSide::P2
        };
        crate::game::profile::update_insert_mask_for_side(side, mask);
    }

    audio::play_sfx("assets/sounds/change_value.ogg");
}

pub(super) fn toggle_remove_row(state: &mut State, player_idx: usize) {
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    let row_index = state.selected_row()[idx];
    if let Some(row) = state.rows().get(row_index) {
        if row.id != RowId::Remove {
            return;
        }
    } else {
        return;
    }

    let choice_index = state.rows()[row_index].selected_choice_index[idx];
    let bit = if choice_index < 8 {
        1u8 << (choice_index as u8)
    } else {
        0
    };
    if bit == 0 {
        return;
    }

    if (state.remove_active_mask[idx] & bit) != 0 {
        state.remove_active_mask[idx] &= !bit;
    } else {
        state.remove_active_mask[idx] |= bit;
    }
    state.remove_active_mask[idx] =
        crate::game::profile::normalize_remove_mask(state.remove_active_mask[idx]);
    let mask = state.remove_active_mask[idx];
    state.player_profiles[idx].remove_active_mask = mask;

    let play_style = crate::game::profile::get_session_play_style();
    let should_persist = play_style == crate::game::profile::PlayStyle::Versus
        || idx == session_persisted_player_idx();
    if should_persist {
        let side = if idx == P1 {
            crate::game::profile::PlayerSide::P1
        } else {
            crate::game::profile::PlayerSide::P2
        };
        crate::game::profile::update_remove_mask_for_side(side, mask);
    }

    audio::play_sfx("assets/sounds/change_value.ogg");
}

pub(super) fn toggle_holds_row(state: &mut State, player_idx: usize) {
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    let row_index = state.selected_row()[idx];
    if let Some(row) = state.rows().get(row_index) {
        if row.id != RowId::Holds {
            return;
        }
    } else {
        return;
    }

    let choice_index = state.rows()[row_index].selected_choice_index[idx];
    let bit = if choice_index < state.rows()[row_index].choices.len().min(u8::BITS as usize) {
        1u8 << (choice_index as u8)
    } else {
        0
    };
    if bit == 0 {
        return;
    }

    if (state.holds_active_mask[idx] & bit) != 0 {
        state.holds_active_mask[idx] &= !bit;
    } else {
        state.holds_active_mask[idx] |= bit;
    }
    state.holds_active_mask[idx] =
        crate::game::profile::normalize_holds_mask(state.holds_active_mask[idx]);
    let mask = state.holds_active_mask[idx];
    state.player_profiles[idx].holds_active_mask = mask;

    let play_style = crate::game::profile::get_session_play_style();
    let should_persist = play_style == crate::game::profile::PlayStyle::Versus
        || idx == session_persisted_player_idx();
    if should_persist {
        let side = if idx == P1 {
            crate::game::profile::PlayerSide::P1
        } else {
            crate::game::profile::PlayerSide::P2
        };
        crate::game::profile::update_holds_mask_for_side(side, mask);
    }

    audio::play_sfx("assets/sounds/change_value.ogg");
}

pub(super) fn toggle_accel_effects_row(state: &mut State, player_idx: usize) {
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    let row_index = state.selected_row()[idx];
    if let Some(row) = state.rows().get(row_index) {
        if row.id != RowId::Accel {
            return;
        }
    } else {
        return;
    }

    let choice_index = state.rows()[row_index].selected_choice_index[idx];
    let bit = if choice_index < state.rows()[row_index].choices.len().min(u8::BITS as usize) {
        1u8 << (choice_index as u8)
    } else {
        0
    };
    if bit == 0 {
        return;
    }

    if (state.accel_effects_active_mask[idx] & bit) != 0 {
        state.accel_effects_active_mask[idx] &= !bit;
    } else {
        state.accel_effects_active_mask[idx] |= bit;
    }
    state.accel_effects_active_mask[idx] =
        crate::game::profile::normalize_accel_effects_mask(state.accel_effects_active_mask[idx]);
    let mask = state.accel_effects_active_mask[idx];
    state.player_profiles[idx].accel_effects_active_mask = mask;

    let play_style = crate::game::profile::get_session_play_style();
    let should_persist = play_style == crate::game::profile::PlayStyle::Versus
        || idx == session_persisted_player_idx();
    if should_persist {
        let side = if idx == P1 {
            crate::game::profile::PlayerSide::P1
        } else {
            crate::game::profile::PlayerSide::P2
        };
        crate::game::profile::update_accel_effects_mask_for_side(side, mask);
    }

    audio::play_sfx("assets/sounds/change_value.ogg");
}

pub(super) fn toggle_visual_effects_row(state: &mut State, player_idx: usize) {
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    let row_index = state.selected_row()[idx];
    if let Some(row) = state.rows().get(row_index) {
        if row.id != RowId::Effect {
            return;
        }
    } else {
        return;
    }

    let choice_index = state.rows()[row_index].selected_choice_index[idx];
    let bit = if choice_index < 10 {
        1u16 << (choice_index as u16)
    } else {
        0
    };
    if bit == 0 {
        return;
    }

    if (state.visual_effects_active_mask[idx] & bit) != 0 {
        state.visual_effects_active_mask[idx] &= !bit;
    } else {
        state.visual_effects_active_mask[idx] |= bit;
    }
    state.visual_effects_active_mask[idx] =
        crate::game::profile::normalize_visual_effects_mask(state.visual_effects_active_mask[idx]);
    let mask = state.visual_effects_active_mask[idx];
    state.player_profiles[idx].visual_effects_active_mask = mask;

    let play_style = crate::game::profile::get_session_play_style();
    let should_persist = play_style == crate::game::profile::PlayStyle::Versus
        || idx == session_persisted_player_idx();
    if should_persist {
        let side = if idx == P1 {
            crate::game::profile::PlayerSide::P1
        } else {
            crate::game::profile::PlayerSide::P2
        };
        crate::game::profile::update_visual_effects_mask_for_side(side, mask);
    }

    audio::play_sfx("assets/sounds/change_value.ogg");
}

pub(super) fn toggle_appearance_effects_row(state: &mut State, player_idx: usize) {
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    let row_index = state.selected_row()[idx];
    if let Some(row) = state.rows().get(row_index) {
        if row.id != RowId::Appearance {
            return;
        }
    } else {
        return;
    }

    let choice_index = state.rows()[row_index].selected_choice_index[idx];
    let bit = if choice_index < state.rows()[row_index].choices.len().min(u8::BITS as usize) {
        1u8 << (choice_index as u8)
    } else {
        0
    };
    if bit == 0 {
        return;
    }

    if (state.appearance_effects_active_mask[idx] & bit) != 0 {
        state.appearance_effects_active_mask[idx] &= !bit;
    } else {
        state.appearance_effects_active_mask[idx] |= bit;
    }
    state.appearance_effects_active_mask[idx] =
        crate::game::profile::normalize_appearance_effects_mask(
            state.appearance_effects_active_mask[idx],
        );
    let mask = state.appearance_effects_active_mask[idx];
    state.player_profiles[idx].appearance_effects_active_mask = mask;

    let play_style = crate::game::profile::get_session_play_style();
    let should_persist = play_style == crate::game::profile::PlayStyle::Versus
        || idx == session_persisted_player_idx();
    if should_persist {
        let side = if idx == P1 {
            crate::game::profile::PlayerSide::P1
        } else {
            crate::game::profile::PlayerSide::P2
        };
        crate::game::profile::update_appearance_effects_mask_for_side(side, mask);
    }

    audio::play_sfx("assets/sounds/change_value.ogg");
}

pub(super) fn toggle_life_bar_options_row(state: &mut State, player_idx: usize) {
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    let row_index = state.selected_row()[idx];
    if let Some(row) = state.rows().get(row_index) {
        if row.id != RowId::LifeBarOptions {
            return;
        }
    } else {
        return;
    }

    let choice_index = state.rows()[row_index].selected_choice_index[idx];
    let bit = if choice_index < 3 {
        1u8 << (choice_index as u8)
    } else {
        0
    };
    if bit == 0 {
        return;
    }

    if (state.life_bar_options_active_mask[idx] & bit) != 0 {
        state.life_bar_options_active_mask[idx] &= !bit;
    } else {
        state.life_bar_options_active_mask[idx] |= bit;
    }

    let rainbow_max = (state.life_bar_options_active_mask[idx] & (1u8 << 0)) != 0;
    let responsive_colors = (state.life_bar_options_active_mask[idx] & (1u8 << 1)) != 0;
    let show_life_percent = (state.life_bar_options_active_mask[idx] & (1u8 << 2)) != 0;
    state.player_profiles[idx].rainbow_max = rainbow_max;
    state.player_profiles[idx].responsive_colors = responsive_colors;
    state.player_profiles[idx].show_life_percent = show_life_percent;

    let play_style = crate::game::profile::get_session_play_style();
    let should_persist = play_style == crate::game::profile::PlayStyle::Versus
        || idx == session_persisted_player_idx();
    if should_persist {
        let side = if idx == P1 {
            crate::game::profile::PlayerSide::P1
        } else {
            crate::game::profile::PlayerSide::P2
        };
        crate::game::profile::update_rainbow_max_for_side(side, rainbow_max);
        crate::game::profile::update_responsive_colors_for_side(side, responsive_colors);
        crate::game::profile::update_show_life_percent_for_side(side, show_life_percent);
    }

    audio::play_sfx("assets/sounds/change_value.ogg");
}

pub(super) fn toggle_fa_plus_row(state: &mut State, player_idx: usize) {
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    let row_index = state.selected_row()[idx];
    if let Some(row) = state.rows().get(row_index) {
        if row.id != RowId::FAPlusOptions {
            return;
        }
    } else {
        return;
    }

    let choice_index = state.rows()[row_index].selected_choice_index[idx];
    let bit = if choice_index < state.rows()[row_index].choices.len().min(u8::BITS as usize) {
        1u8 << (choice_index as u8)
    } else {
        0
    };
    if bit == 0 {
        return;
    }

    // Toggle this bit in the local mask.
    if (state.fa_plus_active_mask[idx] & bit) != 0 {
        state.fa_plus_active_mask[idx] &= !bit;
    } else {
        state.fa_plus_active_mask[idx] |= bit;
    }

    let window_enabled = (state.fa_plus_active_mask[idx] & (1u8 << 0)) != 0;
    let ex_enabled = (state.fa_plus_active_mask[idx] & (1u8 << 1)) != 0;
    let hard_ex_enabled = (state.fa_plus_active_mask[idx] & (1u8 << 2)) != 0;
    let pane_enabled = (state.fa_plus_active_mask[idx] & (1u8 << 3)) != 0;
    let ten_ms_enabled = (state.fa_plus_active_mask[idx] & (1u8 << 4)) != 0;
    let split_15_10ms_enabled = (state.fa_plus_active_mask[idx] & (1u8 << 5)) != 0;
    state.player_profiles[idx].show_fa_plus_window = window_enabled;
    state.player_profiles[idx].show_ex_score = ex_enabled;
    state.player_profiles[idx].show_hard_ex_score = hard_ex_enabled;
    state.player_profiles[idx].show_fa_plus_pane = pane_enabled;
    state.player_profiles[idx].fa_plus_10ms_blue_window = ten_ms_enabled;
    state.player_profiles[idx].split_15_10ms = split_15_10ms_enabled;
    let play_style = crate::game::profile::get_session_play_style();
    let should_persist = play_style == crate::game::profile::PlayStyle::Versus
        || idx == session_persisted_player_idx();
    if should_persist {
        let side = if idx == P1 {
            crate::game::profile::PlayerSide::P1
        } else {
            crate::game::profile::PlayerSide::P2
        };
        crate::game::profile::update_show_fa_plus_window_for_side(side, window_enabled);
        crate::game::profile::update_show_ex_score_for_side(side, ex_enabled);
        crate::game::profile::update_show_hard_ex_score_for_side(side, hard_ex_enabled);
        crate::game::profile::update_show_fa_plus_pane_for_side(side, pane_enabled);
        crate::game::profile::update_fa_plus_10ms_blue_window_for_side(side, ten_ms_enabled);
        crate::game::profile::update_split_15_10ms_for_side(side, split_15_10ms_enabled);
    }

    audio::play_sfx("assets/sounds/change_value.ogg");
}

pub(super) fn toggle_results_extras_row(state: &mut State, player_idx: usize) {
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    let row_index = state.selected_row()[idx];
    if let Some(row) = state.rows().get(row_index) {
        if row.id != RowId::ResultsExtras {
            return;
        }
    } else {
        return;
    }

    let choice_index = state.rows()[row_index].selected_choice_index[idx];
    let bit = if choice_index < 1 {
        1u8 << (choice_index as u8)
    } else {
        0
    };
    if bit == 0 {
        return;
    }

    if (state.results_extras_active_mask[idx] & bit) != 0 {
        state.results_extras_active_mask[idx] &= !bit;
    } else {
        state.results_extras_active_mask[idx] |= bit;
    }

    let track_early_judgments = (state.results_extras_active_mask[idx] & (1u8 << 0)) != 0;
    state.player_profiles[idx].track_early_judgments = track_early_judgments;

    let play_style = crate::game::profile::get_session_play_style();
    let should_persist = play_style == crate::game::profile::PlayStyle::Versus
        || idx == session_persisted_player_idx();
    if should_persist {
        let side = if idx == P1 {
            crate::game::profile::PlayerSide::P1
        } else {
            crate::game::profile::PlayerSide::P2
        };
        crate::game::profile::update_track_early_judgments_for_side(side, track_early_judgments);
    }

    audio::play_sfx("assets/sounds/change_value.ogg");
}

pub(super) fn toggle_error_bar_row(state: &mut State, player_idx: usize) {
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    let row_index = state.selected_row()[idx];
    if let Some(row) = state.rows().get(row_index) {
        if row.id != RowId::ErrorBar {
            return;
        }
    } else {
        return;
    }

    let choice_index = state.rows()[row_index].selected_choice_index[idx];
    let bit = if choice_index < 5 {
        1u8 << (choice_index as u8)
    } else {
        0
    };
    if bit == 0 {
        return;
    }

    if (state.error_bar_active_mask[idx] & bit) != 0 {
        state.error_bar_active_mask[idx] &= !bit;
    } else {
        state.error_bar_active_mask[idx] |= bit;
    }
    state.error_bar_active_mask[idx] =
        crate::game::profile::normalize_error_bar_mask(state.error_bar_active_mask[idx]);
    let mask = state.error_bar_active_mask[idx];
    state.player_profiles[idx].error_bar_active_mask = mask;
    state.player_profiles[idx].error_bar = crate::game::profile::error_bar_style_from_mask(mask);
    state.player_profiles[idx].error_bar_text =
        crate::game::profile::error_bar_text_from_mask(mask);

    let play_style = crate::game::profile::get_session_play_style();
    let should_persist = play_style == crate::game::profile::PlayStyle::Versus
        || idx == session_persisted_player_idx();
    if should_persist {
        let side = if idx == P1 {
            crate::game::profile::PlayerSide::P1
        } else {
            crate::game::profile::PlayerSide::P2
        };
        crate::game::profile::update_error_bar_mask_for_side(side, mask);
    }

    sync_selected_rows_with_visibility(state, session_active_players());
    audio::play_sfx("assets/sounds/change_value.ogg");
}

pub(super) fn toggle_error_bar_options_row(state: &mut State, player_idx: usize) {
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    let row_index = state.selected_row()[idx];
    if let Some(row) = state.rows().get(row_index) {
        if row.id != RowId::ErrorBarOptions {
            return;
        }
    } else {
        return;
    }

    let choice_index = state.rows()[row_index].selected_choice_index[idx];
    let bit = if choice_index < 2 {
        1u8 << (choice_index as u8)
    } else {
        0
    };
    if bit == 0 {
        return;
    }

    if (state.error_bar_options_active_mask[idx] & bit) != 0 {
        state.error_bar_options_active_mask[idx] &= !bit;
    } else {
        state.error_bar_options_active_mask[idx] |= bit;
    }

    let up = (state.error_bar_options_active_mask[idx] & (1u8 << 0)) != 0;
    let multi_tick = (state.error_bar_options_active_mask[idx] & (1u8 << 1)) != 0;
    state.player_profiles[idx].error_bar_up = up;
    state.player_profiles[idx].error_bar_multi_tick = multi_tick;

    let play_style = crate::game::profile::get_session_play_style();
    let should_persist = play_style == crate::game::profile::PlayStyle::Versus
        || idx == session_persisted_player_idx();
    if should_persist {
        let side = if idx == P1 {
            crate::game::profile::PlayerSide::P1
        } else {
            crate::game::profile::PlayerSide::P2
        };
        crate::game::profile::update_error_bar_options_for_side(side, up, multi_tick);
    }

    audio::play_sfx("assets/sounds/change_value.ogg");
}

pub(super) fn toggle_measure_counter_options_row(state: &mut State, player_idx: usize) {
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    let row_index = state.selected_row()[idx];
    if let Some(row) = state.rows().get(row_index) {
        if row.id != RowId::MeasureCounterOptions {
            return;
        }
    } else {
        return;
    }

    let choice_index = state.rows()[row_index].selected_choice_index[idx];
    let bit = if choice_index < 5 {
        1u8 << (choice_index as u8)
    } else {
        0
    };
    if bit == 0 {
        return;
    }

    if (state.measure_counter_options_active_mask[idx] & bit) != 0 {
        state.measure_counter_options_active_mask[idx] &= !bit;
    } else {
        state.measure_counter_options_active_mask[idx] |= bit;
    }

    let left = (state.measure_counter_options_active_mask[idx] & (1u8 << 0)) != 0;
    let up = (state.measure_counter_options_active_mask[idx] & (1u8 << 1)) != 0;
    let vert = (state.measure_counter_options_active_mask[idx] & (1u8 << 2)) != 0;
    let broken_run = (state.measure_counter_options_active_mask[idx] & (1u8 << 3)) != 0;
    let run_timer = (state.measure_counter_options_active_mask[idx] & (1u8 << 4)) != 0;

    state.player_profiles[idx].measure_counter_left = left;
    state.player_profiles[idx].measure_counter_up = up;
    state.player_profiles[idx].measure_counter_vert = vert;
    state.player_profiles[idx].broken_run = broken_run;
    state.player_profiles[idx].run_timer = run_timer;

    let play_style = crate::game::profile::get_session_play_style();
    let should_persist = play_style == crate::game::profile::PlayStyle::Versus
        || idx == session_persisted_player_idx();
    if should_persist {
        let side = if idx == P1 {
            crate::game::profile::PlayerSide::P1
        } else {
            crate::game::profile::PlayerSide::P2
        };
        crate::game::profile::update_measure_counter_options_for_side(
            side, left, up, vert, broken_run, run_timer,
        );
    }

    audio::play_sfx("assets/sounds/change_value.ogg");
}

pub(super) fn toggle_early_dw_row(state: &mut State, player_idx: usize) {
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    let row_index = state.selected_row()[idx];
    if let Some(row) = state.rows().get(row_index) {
        if row.id != RowId::EarlyDecentWayOffOptions {
            return;
        }
    } else {
        return;
    }

    let choice_index = state.rows()[row_index].selected_choice_index[idx];
    let bit = if choice_index < 2 {
        1u8 << (choice_index as u8)
    } else {
        0
    };
    if bit == 0 {
        return;
    }

    if (state.early_dw_active_mask[idx] & bit) != 0 {
        state.early_dw_active_mask[idx] &= !bit;
    } else {
        state.early_dw_active_mask[idx] |= bit;
    }

    let hide_judgments = (state.early_dw_active_mask[idx] & (1u8 << 0)) != 0;
    let hide_flash = (state.early_dw_active_mask[idx] & (1u8 << 1)) != 0;
    state.player_profiles[idx].hide_early_dw_judgments = hide_judgments;
    state.player_profiles[idx].hide_early_dw_flash = hide_flash;

    let play_style = crate::game::profile::get_session_play_style();
    let should_persist = play_style == crate::game::profile::PlayStyle::Versus
        || idx == session_persisted_player_idx();
    if should_persist {
        let side = if idx == P1 {
            crate::game::profile::PlayerSide::P1
        } else {
            crate::game::profile::PlayerSide::P2
        };
        crate::game::profile::update_early_dw_options_for_side(side, hide_judgments, hide_flash);
    }

    audio::play_sfx("assets/sounds/change_value.ogg");
}

pub(super) fn toggle_gameplay_extras_row(state: &mut State, player_idx: usize) {
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    let row_index = state.selected_row()[idx];
    if let Some(row) = state.rows().get(row_index) {
        if row.id != RowId::GameplayExtras {
            return;
        }
    } else {
        return;
    }

    let row = &state.rows()[row_index];
    let choice_index = row.selected_choice_index[idx];
    let ge_flash = tr("PlayerOptions", "GameplayExtrasFlashColumnForMiss");
    let ge_density = tr("PlayerOptions", "GameplayExtrasDensityGraphAtTop");
    let ge_column_cues = tr("PlayerOptions", "GameplayExtrasColumnCues");
    let ge_scorebox = tr("PlayerOptions", "GameplayExtrasDisplayScorebox");
    let bit = row
        .choices
        .get(choice_index)
        .map(|choice| {
            let choice_str = choice.as_str();
            if choice_str == ge_flash.as_ref() {
                1u8 << 0
            } else if choice_str == ge_density.as_ref() {
                1u8 << 1
            } else if choice_str == ge_column_cues.as_ref() {
                1u8 << 2
            } else if choice_str == ge_scorebox.as_ref() {
                1u8 << 3
            } else {
                0
            }
        })
        .unwrap_or(0);
    if bit == 0 {
        return;
    }

    if (state.gameplay_extras_active_mask[idx] & bit) != 0 {
        state.gameplay_extras_active_mask[idx] &= !bit;
    } else {
        state.gameplay_extras_active_mask[idx] |= bit;
    }

    let column_flash_on_miss = (state.gameplay_extras_active_mask[idx] & (1u8 << 0)) != 0;
    let nps_graph_at_top = (state.gameplay_extras_active_mask[idx] & (1u8 << 1)) != 0;
    let column_cues = (state.gameplay_extras_active_mask[idx] & (1u8 << 2)) != 0;
    let display_scorebox = (state.gameplay_extras_active_mask[idx] & (1u8 << 3)) != 0;
    let subtractive_scoring = state.player_profiles[idx].subtractive_scoring;
    let pacemaker = state.player_profiles[idx].pacemaker;

    state.player_profiles[idx].column_flash_on_miss = column_flash_on_miss;
    state.player_profiles[idx].nps_graph_at_top = nps_graph_at_top;
    state.player_profiles[idx].column_cues = column_cues;
    state.player_profiles[idx].display_scorebox = display_scorebox;
    state.gameplay_extras_more_active_mask[idx] =
        (column_cues as u8) | ((display_scorebox as u8) << 1);

    let play_style = crate::game::profile::get_session_play_style();
    let should_persist = play_style == crate::game::profile::PlayStyle::Versus
        || idx == session_persisted_player_idx();
    if should_persist {
        let side = if idx == P1 {
            crate::game::profile::PlayerSide::P1
        } else {
            crate::game::profile::PlayerSide::P2
        };
        crate::game::profile::update_gameplay_extras_for_side(
            side,
            column_flash_on_miss,
            subtractive_scoring,
            pacemaker,
            nps_graph_at_top,
        );
        crate::game::profile::update_column_cues_for_side(side, column_cues);
        crate::game::profile::update_display_scorebox_for_side(side, display_scorebox);
    }

    audio::play_sfx("assets/sounds/change_value.ogg");
}

pub(super) fn toggle_gameplay_extras_more_row(state: &mut State, player_idx: usize) {
    let idx = player_idx.min(PLAYER_SLOTS - 1);
    let row_index = state.selected_row()[idx];
    if let Some(row) = state.rows().get(row_index) {
        if row.id != RowId::GameplayExtrasMore {
            return;
        }
    } else {
        return;
    }

    let choice_index = state.rows()[row_index].selected_choice_index[idx];
    let bit = match choice_index {
        0 => 1u8 << 0, // Column Cues
        1 => 1u8 << 1, // Display Scorebox
        _ => return,
    };

    if (state.gameplay_extras_more_active_mask[idx] & bit) != 0 {
        state.gameplay_extras_more_active_mask[idx] &= !bit;
    } else {
        state.gameplay_extras_more_active_mask[idx] |= bit;
    }

    let column_cues = (state.gameplay_extras_more_active_mask[idx] & (1u8 << 0)) != 0;
    let display_scorebox = (state.gameplay_extras_more_active_mask[idx] & (1u8 << 1)) != 0;
    state.player_profiles[idx].column_cues = column_cues;
    state.player_profiles[idx].display_scorebox = display_scorebox;

    let play_style = crate::game::profile::get_session_play_style();
    let should_persist = play_style == crate::game::profile::PlayStyle::Versus
        || idx == session_persisted_player_idx();
    if should_persist {
        let side = if idx == P1 {
            crate::game::profile::PlayerSide::P1
        } else {
            crate::game::profile::PlayerSide::P2
        };
        crate::game::profile::update_column_cues_for_side(side, column_cues);
        crate::game::profile::update_display_scorebox_for_side(side, display_scorebox);
    }

    audio::play_sfx("assets/sounds/change_value.ogg");
}

/* ------------------------------------------------------------------ */
/* RowKind dispatch                                                   */
/* ------------------------------------------------------------------ */

/// Dispatch a left/right (or rate-key) `delta` to the row's `RowKind`.
///
/// Each row family routes through one arm of the match below; the arm
/// returns an `Outcome` describing whether the change persisted and/or
/// affected row visibility.
fn dispatch_kind_delta(
    state: &mut State,
    player_idx: usize,
    row_index: usize,
    delta: isize,
) -> Outcome {
    // Snapshot dispatch info first so we can drop the borrow on `state`
    // before mutating selected_choice_index / player_profiles.
    enum Action {
        Numeric(&'static NumericBinding),
        Cycle(CycleDispatch),
        BitmaskCursor,
        WhatComesNextCursor,
        Custom(&'static CustomBinding),
        None,
    }
    let action = match &state.rows()[row_index].kind {
        RowKind::Numeric(n) => Action::Numeric(n.binding),
        RowKind::Cycle(c) => match &c.binding {
            CycleBinding::Bool(b) => Action::Cycle(CycleDispatch::Bool(*b)),
            CycleBinding::Index(b) => Action::Cycle(CycleDispatch::Index(*b)),
            CycleBinding::NoteSkin(b) => Action::Cycle(CycleDispatch::NoteSkin(*b)),
        },
        RowKind::Bitmask(_) => Action::BitmaskCursor,
        RowKind::Action(ActionRow::WhatComesNext) => Action::WhatComesNextCursor,
        RowKind::Action(ActionRow::Exit) => Action::None,
        RowKind::Custom(b) => Action::Custom(*b),
    };
    match action {
        Action::Numeric(binding) => apply_numeric_delta(state, player_idx, row_index, delta, binding),
        Action::Cycle(d) => apply_cycle_delta(state, player_idx, row_index, delta, d),
        Action::BitmaskCursor | Action::WhatComesNextCursor => {
            advance_bitmask_cursor(state, player_idx, row_index, delta)
        }
        Action::Custom(b) => (b.apply)(state, player_idx, row_index, delta),
        Action::None => Outcome::NONE,
    }
}

enum CycleDispatch {
    Bool(&'static BoolBinding),
    Index(&'static IndexBinding),
    NoteSkin(&'static NoteSkinBinding),
}

fn apply_cycle_delta(
    state: &mut State,
    player_idx: usize,
    row_index: usize,
    delta: isize,
    dispatch: CycleDispatch,
) -> Outcome {
    let row = &mut state.rows_mut()[row_index];
    let n = row.choices.len();
    if n == 0 {
        return Outcome::NONE;
    }
    let cur = row.selected_choice_index[player_idx] as isize;
    let new_index = (cur + delta).rem_euclid(n as isize) as usize;
    row.selected_choice_index[player_idx] = new_index;

    let play_style = crate::game::profile::get_session_play_style();
    let persisted_idx = session_persisted_player_idx();
    let should_persist =
        play_style == crate::game::profile::PlayStyle::Versus || player_idx == persisted_idx;
    let side = if player_idx == P1 {
        crate::game::profile::PlayerSide::P1
    } else {
        crate::game::profile::PlayerSide::P2
    };

    match dispatch {
        CycleDispatch::Bool(b) => {
            let value = new_index != 0;
            (b.apply)(&mut state.player_profiles[player_idx], value);
            if should_persist {
                (b.persist_for_side)(side, value);
            }
            if b.affects_visibility {
                Outcome::persisted_with_visibility()
            } else {
                Outcome::persisted()
            }
        }
        CycleDispatch::Index(b) => {
            (b.apply)(&mut state.player_profiles[player_idx], new_index);
            if should_persist {
                (b.persist_for_side)(side, new_index);
            }
            if b.affects_visibility {
                Outcome::persisted_with_visibility()
            } else {
                Outcome::persisted()
            }
        }
        CycleDispatch::NoteSkin(b) => {
            // Snapshot the choice string before re-borrowing state for `apply`.
            let choice = state.rows()[row_index]
                .choices
                .get(new_index)
                .cloned()
                .unwrap_or_default();
            (b.apply)(state, player_idx, &choice, should_persist, side);
            Outcome::persisted()
        }
    }
}

fn apply_numeric_delta(
    state: &mut State,
    player_idx: usize,
    row_index: usize,
    delta: isize,
    binding: &NumericBinding,
) -> Outcome {
    let row = &mut state.rows_mut()[row_index];
    let n = row.choices.len();
    if n == 0 {
        return Outcome::NONE;
    }
    let cur = row.selected_choice_index[player_idx] as isize;
    let new_index = (cur + delta).rem_euclid(n as isize) as usize;
    row.selected_choice_index[player_idx] = new_index;
    let Some(choice) = row.choices.get(new_index).cloned() else {
        return Outcome::NONE;
    };
    let Some(value) = (binding.parse)(&choice) else {
        return Outcome::NONE;
    };

    (binding.apply)(&mut state.player_profiles[player_idx], value);

    let play_style = crate::game::profile::get_session_play_style();
    let persisted_idx = session_persisted_player_idx();
    let should_persist =
        play_style == crate::game::profile::PlayStyle::Versus || player_idx == persisted_idx;
    if should_persist {
        let side = if player_idx == P1 {
            crate::game::profile::PlayerSide::P1
        } else {
            crate::game::profile::PlayerSide::P2
        };
        (binding.persist_for_side)(side, value);
    }
    Outcome::persisted()
}

/// Dispatch a Start key press to the row's `RowKind`.
///
/// On `Bitmask` rows this toggles the focused bit; on `Action(Exit)` /
/// `Action(WhatComesNext)` it triggers the row's behaviour. Cycle/Numeric
/// rows ignore Start. Returns `Outcome::NONE` until rows migrate.
pub(super) fn dispatch_kind_toggle(state: &mut State, player_idx: usize, row_index: usize) -> Outcome {
    let binding = match &state.rows()[row_index].kind {
        RowKind::Bitmask(b) => b.binding,
        RowKind::Numeric(_) | RowKind::Cycle(_) | RowKind::Action(_) | RowKind::Custom(_) => {
            return Outcome::NONE;
        }
    };
    (binding.toggle)(state, player_idx);
    Outcome::persisted()
}

/// Move the focus cursor on a bitmask row in response to L/R: advance
/// `selected_choice_index` (wrapped via `rem_euclid` so the index stays
/// in `[0, n)`), then let the caller play the change-value SFX. The
/// actual mask flip is bound to Start.
pub(super) fn advance_bitmask_cursor(
    state: &mut State,
    player_idx: usize,
    row_index: usize,
    delta: isize,
) -> Outcome {
    let row = &mut state.rows_mut()[row_index];
    let n = row.choices.len();
    if n == 0 {
        return Outcome::NONE;
    }
    let cur = row.selected_choice_index[player_idx] as isize;
    let new_index = (cur + delta).rem_euclid(n as isize) as usize;
    row.selected_choice_index[player_idx] = new_index;
    Outcome::persisted()
}

/// Test-only action to drive a single row through the production
/// dispatcher without touching the input stack.
#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(crate) enum TestAction {
    /// L/R press. Numeric/Cycle rows update their value; Bitmask rows move
    /// the focus cursor; Action(WhatComesNext) advances the choice index;
    /// Action(Exit) is a no-op.
    Delta(isize),
    /// Start press. Bitmask rows toggle the focused bit; other kinds are
    /// no-ops at this dispatcher (Action::Exit/WhatComesNext run elsewhere
    /// in the input stack).
    Toggle,
}

/// Drive a single row through the production `RowKind` dispatcher.
///
/// Searches every pane for a row whose `id == row_id`. If found, switches
/// `state.current_pane` to that pane, sets the player's selection cursor
/// onto it, then dispatches `action`. Visibility is re-synced when the row
/// reports a visibility-affecting change, mirroring `change_choice_for_player`.
/// The audio SFX call is skipped (uninitialized audio is already a no-op
/// in test builds, but we skip it explicitly for clarity).
///
/// Returns the [`Outcome`] reported by the row, or `Outcome::NONE` if the
/// row isn't present in any pane.
#[cfg(test)]
pub(crate) fn dispatch_for_test(
    state: &mut State,
    player_idx: usize,
    row_id: RowId,
    action: TestAction,
) -> Outcome {
    let player_idx = player_idx.min(PLAYER_SLOTS - 1);
    let Some((pane, row_index)) = OptionsPane::ALL.into_iter().find_map(|pane| {
        state
            .pane_data(pane)
            .rows
            .iter()
            .position(|r| r.id == row_id)
            .map(|i| (pane, i))
    }) else {
        return Outcome::NONE;
    };
    state.current_pane = pane;
    state.selected_row_mut()[player_idx] = row_index;

    let outcome = match action {
        TestAction::Delta(delta) => dispatch_kind_delta(state, player_idx, row_index, delta),
        TestAction::Toggle => dispatch_kind_toggle(state, player_idx, row_index),
    };

    if outcome.changed_visibility {
        sync_selected_rows_with_visibility(state, session_active_players());
    }
    outcome
}

