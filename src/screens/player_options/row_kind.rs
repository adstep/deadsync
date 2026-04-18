//! Per-row behavior dispatch.
//!
//! Each `Row` carries a `RowKind` that tells the dispatcher in `choice.rs`
//! how to handle L/R (`change_choice_for_player`) and Start
//! (`dispatch_kind_toggle`). Variants:
//!
//! - `Numeric` — i32 cycles parsed from the row's choice strings
//! - `Cycle` — Bool / Index / NoteSkin (3 sub-flavours)
//! - `Bitmask` — multi-select rows whose Start toggles the focused bit
//! - `Action` — Exit / WhatComesNext (Start triggers, L/R is mostly inert)
//! - `Custom` — escape hatch for rows whose behaviour doesn't fit the others

use crate::game::profile::{PlayerSide, Profile};

use super::State;

/// Result of a row's reaction to a key press.
///
/// Kept tiny so every dispatcher arm can return one without ceremony. The
/// shared dispatcher in `choice.rs` reads it to decide whether to play the
/// change-value SFX and whether to re-run visibility sync.
#[derive(Clone, Copy, Debug, Default)]
pub struct Outcome {
    pub persisted: bool,
    pub changed_visibility: bool,
}

impl Outcome {
    pub const NONE: Self = Self {
        persisted: false,
        changed_visibility: false,
    };

    #[inline(always)]
    pub const fn persisted() -> Self {
        Self {
            persisted: true,
            changed_visibility: false,
        }
    }

    #[inline(always)]
    pub const fn persisted_with_visibility() -> Self {
        Self {
            persisted: true,
            changed_visibility: true,
        }
    }
}

/// Numeric rows whose `Row::choices` already encode every legal value as
/// a string. Used for the i32 offset rows, `VisualDelay`, and
/// `GlobalOffsetShift`. Rows with bespoke step/parse contracts
/// (`SpeedMod`, `MusicRate`, `JudgmentTiltIntensity`,
/// `CustomBlueFantasticWindowMs`) live under `RowKind::Custom` instead.
pub struct NumericRow {
    pub binding: &'static NumericBinding,
}

/// Static behaviour for a numeric row whose `Row::choices` already encode
/// every legal value as a string. The dispatcher advances
/// `selected_choice_index`, parses the new string via `parse`, then writes
/// the parsed `i32` to the player's `Profile` and (when persisting)
/// forwards it to `persist_for_side`.
pub struct NumericBinding {
    /// Convert a choice string (e.g. `"-37ms"`) into the persisted value.
    pub parse: fn(&str) -> Option<i32>,
    /// Mirror the parsed value into the in-memory player Profile.
    pub apply: fn(&mut Profile, i32),
    /// Forward the parsed value to the side-specific persist helper.
    pub persist_for_side: fn(PlayerSide, i32),
}

/// Cycle rows: pick one of N choices, wraps. Writes one Profile field.
/// Three sub-flavours: plain two-state booleans, variant-table enums, and
/// noteskin pickers.
pub struct CycleRow {
    pub binding: CycleBinding,
}

/// How a `CycleRow` writes its currently selected index back to the
/// persisted player profile.
pub enum CycleBinding {
    /// Two-state cycle: `selected_choice_index != 0` ⇒ `true`.
    Bool(&'static BoolBinding),
    /// Variant-table cycle: the selected index maps to a fixed enum variant
    /// via a static lookup table; the binding handles the mapping internally.
    Index(&'static IndexBinding),
    /// NoteSkin-family cycle (NoteSkin / MineSkin / ReceptorSkin /
    /// TapExplosionSkin). Each binding owns the per-row string-to-value
    /// translation (because Option<NoteSkin> + "MatchNoteSkin" /
    /// "NoTapExplosion" sentinels vary) and runs
    /// `sync_noteskin_previews_for_player` after applying.
    NoteSkin(&'static NoteSkinBinding),
}

/// Static behaviour for a two-state bool cycle row.
pub struct BoolBinding {
    pub apply: fn(&mut Profile, bool),
    pub persist_for_side: fn(PlayerSide, bool),
    /// True when toggling this row affects which other rows are visible
    /// (e.g. `JudgmentTilt` reveals `JudgmentTiltIntensity`). When true the
    /// dispatcher re-runs `sync_selected_rows_with_visibility` after the
    /// change.
    pub affects_visibility: bool,
}

/// Static behaviour for a variant-table cycle row. The dispatcher passes the
/// new `selected_choice_index` to `apply` / `persist_for_side`; the binding
/// looks the variant up in its private table and writes it.
pub struct IndexBinding {
    pub apply: fn(&mut Profile, usize),
    pub persist_for_side: fn(PlayerSide, usize),
    pub affects_visibility: bool,
}

/// Static behaviour for a NoteSkin-family cycle row. The dispatcher resolves
/// the new choice string and hands it to `apply`, which mutates the profile,
/// optionally calls the side persist helper, and refreshes preview state.
/// Encapsulating the whole side-effecting body keeps the dispatcher
/// agnostic to the per-row Option<NoteSkin>/sentinel-string differences.
pub struct NoteSkinBinding {
    pub apply: fn(state: &mut State, player_idx: usize, choice: &str, should_persist: bool, side: PlayerSide),
}

/// Bitmask rows: toggle one of N flags. L/R moves focus, Start toggles
/// the focused bit. Some toggles (e.g. `Hide`) also re-run visibility
/// sync; others (e.g. `LifeBarOptions`) write a derived bitmask to
/// multiple Profile fields. The binding always routes through a single
/// fn-pointer; the helper bodies stay in `choice.rs`.
pub struct BitmaskRow {
    pub binding: &'static BitmaskBinding,
}

/// Static behaviour for a bitmask row: one entry point, `toggle`, takes
/// the player index and is responsible for flipping the focused bit on the
/// row's State-owned mask, normalising it, mirroring it to the player
/// Profile, persisting via the side helper, and playing the change SFX.
/// Encapsulating the whole side-effecting body in a single fn keeps the
/// dispatcher agnostic to per-row mask widths (u8 vs u16) and to the
/// individual normaliser/persister helpers each row owns.
pub struct BitmaskBinding {
    pub toggle: fn(&mut State, usize),
}

/// Action rows: Exit / WhatComesNext. Behaviour fires on Start, not L/R.
#[derive(Clone, Copy, Debug)]
pub enum ActionRow {
    Exit,
    WhatComesNext,
}

/// Escape hatch for rows whose behaviour doesn't fit any of the structured
/// variants above (e.g. `MusicRate` writes a global float, `MiniIndicator`
/// updates 4 profile fields + extras, `TypeOfSpeedMod` does BPM-aware
/// conversion). Each binding owns the row's full L/R handler. The
/// dispatcher still wraps the result in the standard tail (sync inline
/// intent + SFX + optional visibility resync).
pub struct CustomBinding {
    pub apply: fn(&mut State, usize, usize, isize) -> Outcome,
}

/// What kind of row this is, and any state owned by the row's behaviour.
pub enum RowKind {
    Numeric(NumericRow),
    Cycle(CycleRow),
    Bitmask(BitmaskRow),
    Action(ActionRow),
    Custom(&'static CustomBinding),
}

/* -------------------------------- Bindings ----------------------------- */

/// Parse a plain `i32` choice string.
fn parse_i32(s: &str) -> Option<i32> {
    s.parse::<i32>().ok()
}

/// Parse a choice string with a trailing `"ms"` suffix.
fn parse_i32_ms(s: &str) -> Option<i32> {
    s.trim_end_matches("ms").parse::<i32>().ok()
}

pub static JUDGMENT_OFFSET_X: NumericBinding = NumericBinding {
    parse: parse_i32,
    apply: |p, v| p.judgment_offset_x = v,
    persist_for_side: crate::game::profile::update_judgment_offset_x_for_side,
};
pub static JUDGMENT_OFFSET_Y: NumericBinding = NumericBinding {
    parse: parse_i32,
    apply: |p, v| p.judgment_offset_y = v,
    persist_for_side: crate::game::profile::update_judgment_offset_y_for_side,
};
pub static COMBO_OFFSET_X: NumericBinding = NumericBinding {
    parse: parse_i32,
    apply: |p, v| p.combo_offset_x = v,
    persist_for_side: crate::game::profile::update_combo_offset_x_for_side,
};
pub static COMBO_OFFSET_Y: NumericBinding = NumericBinding {
    parse: parse_i32,
    apply: |p, v| p.combo_offset_y = v,
    persist_for_side: crate::game::profile::update_combo_offset_y_for_side,
};
pub static ERROR_BAR_OFFSET_X: NumericBinding = NumericBinding {
    parse: parse_i32,
    apply: |p, v| p.error_bar_offset_x = v,
    persist_for_side: crate::game::profile::update_error_bar_offset_x_for_side,
};
pub static ERROR_BAR_OFFSET_Y: NumericBinding = NumericBinding {
    parse: parse_i32,
    apply: |p, v| p.error_bar_offset_y = v,
    persist_for_side: crate::game::profile::update_error_bar_offset_y_for_side,
};
pub static NOTEFIELD_OFFSET_X: NumericBinding = NumericBinding {
    parse: parse_i32,
    apply: |p, v| p.note_field_offset_x = v,
    persist_for_side: crate::game::profile::update_notefield_offset_x_for_side,
};
pub static NOTEFIELD_OFFSET_Y: NumericBinding = NumericBinding {
    parse: parse_i32,
    apply: |p, v| p.note_field_offset_y = v,
    persist_for_side: crate::game::profile::update_notefield_offset_y_for_side,
};
pub static VISUAL_DELAY: NumericBinding = NumericBinding {
    parse: parse_i32_ms,
    apply: |p, v| p.visual_delay_ms = v,
    persist_for_side: crate::game::profile::update_visual_delay_ms_for_side,
};
pub static GLOBAL_OFFSET_SHIFT: NumericBinding = NumericBinding {
    parse: parse_i32_ms,
    apply: |p, v| p.global_offset_shift_ms = v,
    persist_for_side: crate::game::profile::update_global_offset_shift_ms_for_side,
};

/* -------------------------- Two-state bool bindings -------------------- */

pub static JUDGMENT_TILT: BoolBinding = BoolBinding {
    apply: |p, v| p.judgment_tilt = v,
    persist_for_side: crate::game::profile::update_judgment_tilt_for_side,
    affects_visibility: true,
};
pub static JUDGMENT_BEHIND_ARROWS: BoolBinding = BoolBinding {
    apply: |p, v| p.judgment_back = v,
    persist_for_side: crate::game::profile::update_judgment_back_for_side,
    affects_visibility: false,
};
pub static OFFSET_INDICATOR: BoolBinding = BoolBinding {
    apply: |p, v| p.error_ms_display = v,
    persist_for_side: crate::game::profile::update_error_ms_display_for_side,
    affects_visibility: false,
};
pub static RESCORE_EARLY_HITS: BoolBinding = BoolBinding {
    apply: |p, v| p.rescore_early_hits = v,
    persist_for_side: crate::game::profile::update_rescore_early_hits_for_side,
    affects_visibility: false,
};
pub static CUSTOM_BLUE_FANTASTIC_WINDOW: BoolBinding = BoolBinding {
    apply: |p, v| p.custom_fantastic_window = v,
    persist_for_side: crate::game::profile::update_custom_fantastic_window_for_side,
    affects_visibility: true,
};
pub static CARRY_COMBO: BoolBinding = BoolBinding {
    apply: |p, v| p.carry_combo_between_songs = v,
    persist_for_side: crate::game::profile::update_carry_combo_between_songs_for_side,
    affects_visibility: false,
};
pub static DENSITY_GRAPH_BACKGROUND: BoolBinding = BoolBinding {
    apply: |p, v| p.transparent_density_graph_bg = v,
    persist_for_side: crate::game::profile::update_transparent_density_graph_bg_for_side,
    affects_visibility: false,
};

/* -------------------------- Variant-table bindings --------------------- */

/// Build an `IndexBinding` for a row whose choices map 1:1 to a static
/// `[Enum; N]` variant table, write a single Profile field, and persist via
/// a single side helper. Cuts the per-binding boilerplate down to its data.
macro_rules! index_binding {
    ($name:ident, $table:expr, $default:expr, $field:ident, $persist:expr, $vis:expr) => {
        pub static $name: IndexBinding = IndexBinding {
            apply: |p, i| p.$field = $table.get(i).copied().unwrap_or($default),
            persist_for_side: |s, i| {
                $persist(s, $table.get(i).copied().unwrap_or($default))
            },
            affects_visibility: $vis,
        };
    };
}

use super::panes;
use crate::game::profile as gp;

index_binding!(
    TURN,
    panes::TURN_OPTION_VARIANTS,
    gp::TurnOption::None,
    turn_option,
    gp::update_turn_option_for_side,
    false
);
index_binding!(
    ATTACKS,
    panes::ATTACK_MODE_VARIANTS,
    gp::AttackMode::On,
    attack_mode,
    gp::update_attack_mode_for_side,
    false
);
index_binding!(
    HIDE_LIGHT_TYPE,
    panes::HIDE_LIGHT_TYPE_VARIANTS,
    gp::HideLightType::NoHideLights,
    hide_light_type,
    gp::update_hide_light_type_for_side,
    false
);
index_binding!(
    TIMING_WINDOWS,
    panes::TIMING_WINDOWS_VARIANTS,
    gp::TimingWindowsOption::None,
    timing_windows,
    gp::update_timing_windows_for_side,
    false
);
index_binding!(
    INDICATOR_SCORE_TYPE,
    panes::MINI_INDICATOR_SCORE_TYPE_VARIANTS,
    gp::MiniIndicatorScoreType::Itg,
    mini_indicator_score_type,
    gp::update_mini_indicator_score_type_for_side,
    false
);
index_binding!(
    BACKGROUND_FILTER,
    panes::BACKGROUND_FILTER_VARIANTS,
    gp::BackgroundFilter::Darkest,
    background_filter,
    gp::update_background_filter_for_side,
    false
);
index_binding!(
    PERSPECTIVE,
    panes::PERSPECTIVE_VARIANTS,
    gp::Perspective::Overhead,
    perspective,
    gp::update_perspective_for_side,
    false
);
index_binding!(
    LIFE_METER_TYPE,
    panes::LIFE_METER_TYPE_VARIANTS,
    gp::LifeMeterType::Standard,
    lifemeter_type,
    gp::update_lifemeter_type_for_side,
    false
);
index_binding!(
    DATA_VISUALIZATIONS,
    panes::DATA_VISUALIZATIONS_VARIANTS,
    gp::DataVisualizations::None,
    data_visualizations,
    gp::update_data_visualizations_for_side,
    true
);
index_binding!(
    TARGET_SCORE,
    panes::TARGET_SCORE_VARIANTS,
    gp::TargetScoreSetting::S,
    target_score,
    gp::update_target_score_for_side,
    false
);
index_binding!(
    ERROR_BAR_TRIM,
    panes::ERROR_BAR_TRIM_VARIANTS,
    gp::ErrorBarTrim::Off,
    error_bar_trim,
    gp::update_error_bar_trim_for_side,
    false
);
index_binding!(
    MEASURE_COUNTER,
    panes::MEASURE_COUNTER_VARIANTS,
    gp::MeasureCounter::None,
    measure_counter,
    gp::update_measure_counter_for_side,
    true
);
index_binding!(
    MEASURE_LINES,
    panes::MEASURE_LINES_VARIANTS,
    gp::MeasureLines::Off,
    measure_lines,
    gp::update_measure_lines_for_side,
    false
);
index_binding!(
    COMBO_FONT,
    panes::COMBO_FONT_VARIANTS,
    gp::ComboFont::Wendy,
    combo_font,
    gp::update_combo_font_for_side,
    true
);
index_binding!(
    COMBO_COLORS,
    panes::COMBO_COLORS_VARIANTS,
    gp::ComboColors::Glow,
    combo_colors,
    gp::update_combo_colors_for_side,
    false
);
index_binding!(
    COMBO_COLOR_MODE,
    panes::COMBO_MODE_VARIANTS,
    gp::ComboMode::FullCombo,
    combo_mode,
    gp::update_combo_mode_for_side,
    false
);

/* ---------------------------- NoteSkin bindings ----------------------- */

use super::noteskins::sync_noteskin_previews_for_player;
use crate::assets::i18n::tr;
use super::constants::{MATCH_NOTESKIN_LABEL, NO_TAP_EXPLOSION_LABEL};

pub static NOTE_SKIN: NoteSkinBinding = NoteSkinBinding {
    apply: |state, player_idx, choice, should_persist, side| {
        let name = if choice.is_empty() {
            gp::NoteSkin::DEFAULT_NAME.to_string()
        } else {
            choice.to_string()
        };
        let setting = gp::NoteSkin::new(&name);
        state.player_profiles[player_idx].noteskin = setting.clone();
        if should_persist {
            gp::update_noteskin_for_side(side, setting);
        }
        sync_noteskin_previews_for_player(state, player_idx);
    },
};

pub static MINE_SKIN: NoteSkinBinding = NoteSkinBinding {
    apply: |state, player_idx, choice, should_persist, side| {
        let match_label = tr("PlayerOptions", MATCH_NOTESKIN_LABEL);
        let setting = if choice == match_label.as_ref() {
            None
        } else {
            Some(gp::NoteSkin::new(choice))
        };
        state.player_profiles[player_idx]
            .mine_noteskin
            .clone_from(&setting);
        if should_persist {
            gp::update_mine_noteskin_for_side(side, setting);
        }
        sync_noteskin_previews_for_player(state, player_idx);
    },
};

pub static RECEPTOR_SKIN: NoteSkinBinding = NoteSkinBinding {
    apply: |state, player_idx, choice, should_persist, side| {
        let match_label = tr("PlayerOptions", MATCH_NOTESKIN_LABEL);
        let setting = if choice == match_label.as_ref() {
            None
        } else {
            Some(gp::NoteSkin::new(choice))
        };
        state.player_profiles[player_idx]
            .receptor_noteskin
            .clone_from(&setting);
        if should_persist {
            gp::update_receptor_noteskin_for_side(side, setting);
        }
        sync_noteskin_previews_for_player(state, player_idx);
    },
};

pub static TAP_EXPLOSION_SKIN: NoteSkinBinding = NoteSkinBinding {
    apply: |state, player_idx, choice, should_persist, side| {
        let match_label = tr("PlayerOptions", MATCH_NOTESKIN_LABEL);
        let no_tap_label = tr("PlayerOptions", NO_TAP_EXPLOSION_LABEL);
        let setting = if choice == match_label.as_ref() {
            None
        } else if choice == no_tap_label.as_ref() {
            Some(gp::NoteSkin::none_choice())
        } else {
            Some(gp::NoteSkin::new(choice))
        };
        state.player_profiles[player_idx]
            .tap_explosion_noteskin
            .clone_from(&setting);
        if should_persist {
            gp::update_tap_explosion_noteskin_for_side(side, setting);
        }
        sync_noteskin_previews_for_player(state, player_idx);
    },
};

/* ---------------------------- Bitmask bindings ------------------------ */

use super::choice as ch;

pub static INSERT: BitmaskBinding = BitmaskBinding { toggle: ch::toggle_insert_row };
pub static REMOVE: BitmaskBinding = BitmaskBinding { toggle: ch::toggle_remove_row };
pub static HOLDS: BitmaskBinding = BitmaskBinding { toggle: ch::toggle_holds_row };
pub static ACCEL: BitmaskBinding = BitmaskBinding { toggle: ch::toggle_accel_effects_row };
pub static EFFECT: BitmaskBinding = BitmaskBinding { toggle: ch::toggle_visual_effects_row };
pub static APPEARANCE: BitmaskBinding = BitmaskBinding { toggle: ch::toggle_appearance_effects_row };

// Complex bitmask rows. Some of the toggle helpers also run
// `sync_selected_rows_with_visibility` (Hide) or write a derived bitmask to
// multiple Profile fields (LifeBarOptions, GameplayExtras, ResultsExtras,
// ErrorBar*, MeasureCounterOptions, FAPlusOptions, EarlyDecentWayOff). The
// binding still routes through a single fn-pointer; the helper bodies stay
// in choice.rs unchanged.
pub static SCROLL: BitmaskBinding = BitmaskBinding { toggle: ch::toggle_scroll_row };
pub static HIDE: BitmaskBinding = BitmaskBinding { toggle: ch::toggle_hide_row };
pub static LIFE_BAR_OPTIONS: BitmaskBinding = BitmaskBinding { toggle: ch::toggle_life_bar_options_row };
pub static GAMEPLAY_EXTRAS: BitmaskBinding = BitmaskBinding { toggle: ch::toggle_gameplay_extras_row };
pub static RESULTS_EXTRAS: BitmaskBinding = BitmaskBinding { toggle: ch::toggle_results_extras_row };
pub static ERROR_BAR: BitmaskBinding = BitmaskBinding { toggle: ch::toggle_error_bar_row };
pub static ERROR_BAR_OPTIONS: BitmaskBinding = BitmaskBinding { toggle: ch::toggle_error_bar_options_row };
pub static MEASURE_COUNTER_OPTIONS: BitmaskBinding = BitmaskBinding { toggle: ch::toggle_measure_counter_options_row };
pub static FA_PLUS_OPTIONS: BitmaskBinding = BitmaskBinding { toggle: ch::toggle_fa_plus_row };
pub static EARLY_DW_OPTIONS: BitmaskBinding = BitmaskBinding { toggle: ch::toggle_early_dw_row };

/* ---------------------------- Custom bindings ------------------------- */

use super::constants::{TILT_INTENSITY_MAX, TILT_INTENSITY_MIN, TILT_INTENSITY_STEP};
use super::panes::MINI_INDICATOR_VARIANTS;
use super::speed_mod::{
    fmt_music_rate, reference_bpm_for_song, resolve_p1_chart, sync_profile_scroll_speed,
};
use super::{P1, PLAYER_SLOTS, SpeedModType, session_persisted_player_idx};
use crate::assets;
use crate::engine::audio;
use gp::MiniIndicator;

/// Returns (should_persist, persist_side) for the given player index.
fn persist_ctx(player_idx: usize) -> (bool, PlayerSide) {
    let play_style = gp::get_session_play_style();
    let persisted_idx = session_persisted_player_idx();
    let should_persist =
        play_style == gp::PlayStyle::Versus || player_idx == persisted_idx;
    let side = if player_idx == P1 {
        PlayerSide::P1
    } else {
        PlayerSide::P2
    };
    (should_persist, side)
}

/// Advance `selected_choice_index[player_idx]` by `delta`, wrapping. Returns
/// the new index, or `None` if the row has no choices.
fn cycle_choice_index(state: &mut State, player_idx: usize, row_index: usize, delta: isize) -> Option<usize> {
    let row = &mut state.rows_mut()[row_index];
    let n = row.choices.len();
    if n == 0 {
        return None;
    }
    let cur = row.selected_choice_index[player_idx] as isize;
    let new_index = (cur + delta).rem_euclid(n as isize) as usize;
    row.selected_choice_index[player_idx] = new_index;
    Some(new_index)
}

fn round_to_step(value: f32, step: f32) -> f32 {
    (value / step).round() * step
}

pub static MUSIC_RATE: CustomBinding = CustomBinding {
    apply: |state, _player_idx, row_index, delta| {
        let increment = 0.01f32;
        state.music_rate += delta as f32 * increment;
        state.music_rate = (state.music_rate / increment).round() * increment;
        state.music_rate = state.music_rate.clamp(0.05, 3.00);
        let formatted = fmt_music_rate(state.music_rate);
        state.rows_mut()[row_index].choices[0] = formatted;
        gp::set_session_music_rate(state.music_rate);
        audio::set_music_rate(state.music_rate);
        // Mirror the index across both players to honour the shared-row
        // contract even though MusicRate's value lives on State.
        for slot in 0..PLAYER_SLOTS {
            state.rows_mut()[row_index].selected_choice_index[slot] = 0;
        }
        Outcome::persisted()
    },
};

pub static SPEED_MOD: CustomBinding = CustomBinding {
    apply: |state, player_idx, _row_index, delta| {
        let speed_mod = {
            let speed_mod = &mut state.speed_mod[player_idx];
            let (upper, increment) = match speed_mod.mod_type {
                SpeedModType::X => (20.0, 0.05),
                SpeedModType::C | SpeedModType::M => (2000.0, 5.0),
            };
            speed_mod.value += delta as f32 * increment;
            speed_mod.value = (speed_mod.value / increment).round() * increment;
            speed_mod.value = speed_mod.value.clamp(increment, upper);
            speed_mod.clone()
        };
        sync_profile_scroll_speed(&mut state.player_profiles[player_idx], &speed_mod);
        Outcome::persisted()
    },
};

pub static TYPE_OF_SPEED_MOD: CustomBinding = CustomBinding {
    apply: |state, player_idx, row_index, delta| {
        let Some(new_index) = cycle_choice_index(state, player_idx, row_index, delta) else {
            return Outcome::NONE;
        };
        let new_type = SpeedModType::from_type_choice_index(new_index);
        let speed_mod = &mut state.speed_mod[player_idx];
        let old_type = speed_mod.mod_type;
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
        let target_bpm: f32 = match old_type {
            SpeedModType::C | SpeedModType::M => old_value,
            SpeedModType::X => (reference_bpm * rate * old_value).round(),
        };
        let new_value = match new_type {
            SpeedModType::X => {
                let denom = reference_bpm * rate;
                let raw = if denom.is_finite() && denom > 0.0 {
                    target_bpm / denom
                } else {
                    1.0
                };
                round_to_step(raw, 0.05).clamp(0.05, 20.0)
            }
            SpeedModType::C | SpeedModType::M => round_to_step(target_bpm, 5.0).clamp(5.0, 2000.0),
        };
        speed_mod.mod_type = new_type;
        speed_mod.value = new_value;
        let speed_mod = speed_mod.clone();
        sync_profile_scroll_speed(&mut state.player_profiles[player_idx], &speed_mod);
        Outcome::persisted()
    },
};

pub static CUSTOM_BLUE_FANTASTIC_WINDOW_MS: CustomBinding = CustomBinding {
    apply: |state, player_idx, row_index, delta| {
        let Some(new_index) = cycle_choice_index(state, player_idx, row_index, delta) else {
            return Outcome::NONE;
        };
        let row = &state.rows()[row_index];
        let Some(choice) = row.choices.get(new_index).cloned() else {
            return Outcome::NONE;
        };
        let Ok(raw) = choice.trim_end_matches("ms").parse::<u8>() else {
            return Outcome::persisted();
        };
        let ms = gp::clamp_custom_fantastic_window_ms(raw);
        state.player_profiles[player_idx].custom_fantastic_window_ms = ms;
        let (should_persist, side) = persist_ctx(player_idx);
        if should_persist {
            gp::update_custom_fantastic_window_ms_for_side(side, ms);
        }
        Outcome::persisted()
    },
};

pub static MINI_INDICATOR: CustomBinding = CustomBinding {
    apply: |state, player_idx, row_index, delta| {
        let Some(new_index) = cycle_choice_index(state, player_idx, row_index, delta) else {
            return Outcome::NONE;
        };
        let mini_indicator = MINI_INDICATOR_VARIANTS
            .get(new_index)
            .copied()
            .unwrap_or(MiniIndicator::None);
        let subtractive_scoring = mini_indicator == MiniIndicator::SubtractiveScoring;
        let pacemaker = mini_indicator == MiniIndicator::Pacemaker;
        state.player_profiles[player_idx].mini_indicator = mini_indicator;
        state.player_profiles[player_idx].subtractive_scoring = subtractive_scoring;
        state.player_profiles[player_idx].pacemaker = pacemaker;
        let (should_persist, side) = persist_ctx(player_idx);
        if should_persist {
            let profile_ref = &state.player_profiles[player_idx];
            gp::update_mini_indicator_for_side(side, mini_indicator);
            gp::update_gameplay_extras_for_side(
                side,
                profile_ref.column_flash_on_miss,
                subtractive_scoring,
                pacemaker,
                profile_ref.nps_graph_at_top,
            );
        }
        Outcome::persisted_with_visibility()
    },
};

pub static MINI: CustomBinding = CustomBinding {
    apply: |state, player_idx, row_index, delta| {
        let Some(new_index) = cycle_choice_index(state, player_idx, row_index, delta) else {
            return Outcome::NONE;
        };
        let row = &state.rows()[row_index];
        let Some(choice) = row.choices.get(new_index).cloned() else {
            return Outcome::NONE;
        };
        let Ok(val) = choice.trim_end_matches('%').parse::<i32>() else {
            return Outcome::persisted();
        };
        state.player_profiles[player_idx].mini_percent = val;
        let (should_persist, side) = persist_ctx(player_idx);
        if should_persist {
            gp::update_mini_percent_for_side(side, val);
        }
        Outcome::persisted()
    },
};

pub static JUDGMENT_TILT_INTENSITY: CustomBinding = CustomBinding {
    apply: |state, player_idx, row_index, delta| {
        let Some(new_index) = cycle_choice_index(state, player_idx, row_index, delta) else {
            return Outcome::NONE;
        };
        let row = &state.rows()[row_index];
        let Some(choice) = row.choices.get(new_index).cloned() else {
            return Outcome::NONE;
        };
        let Ok(mult) = choice.parse::<f32>() else {
            return Outcome::persisted();
        };
        let mult = round_to_step(mult, TILT_INTENSITY_STEP)
            .clamp(TILT_INTENSITY_MIN, TILT_INTENSITY_MAX);
        state.player_profiles[player_idx].tilt_multiplier = mult;
        let (should_persist, side) = persist_ctx(player_idx);
        if should_persist {
            gp::update_tilt_multiplier_for_side(side, mult);
        }
        Outcome::persisted()
    },
};

pub static MEASURE_COUNTER_LOOKAHEAD: CustomBinding = CustomBinding {
    apply: |state, player_idx, row_index, delta| {
        let Some(new_index) = cycle_choice_index(state, player_idx, row_index, delta) else {
            return Outcome::NONE;
        };
        let lookahead = (new_index as u8).min(4);
        state.player_profiles[player_idx].measure_counter_lookahead = lookahead;
        let (should_persist, side) = persist_ctx(player_idx);
        if should_persist {
            gp::update_measure_counter_lookahead_for_side(side, lookahead);
        }
        Outcome::persisted()
    },
};

pub static JUDGMENT_FONT: CustomBinding = CustomBinding {
    apply: |state, player_idx, row_index, delta| {
        let Some(new_index) = cycle_choice_index(state, player_idx, row_index, delta) else {
            return Outcome::NONE;
        };
        let setting = assets::judgment_texture_choices()
            .get(new_index)
            .map(|choice| gp::JudgmentGraphic::new(&choice.key))
            .unwrap_or_default();
        state.player_profiles[player_idx].judgment_graphic = setting;
        let (should_persist, side) = persist_ctx(player_idx);
        if should_persist {
            gp::update_judgment_graphic_for_side(
                side,
                state.player_profiles[player_idx].judgment_graphic.clone(),
            );
        }
        Outcome::persisted_with_visibility()
    },
};

pub static HOLD_JUDGMENT: CustomBinding = CustomBinding {
    apply: |state, player_idx, row_index, delta| {
        let Some(new_index) = cycle_choice_index(state, player_idx, row_index, delta) else {
            return Outcome::NONE;
        };
        let setting = assets::hold_judgment_texture_choices()
            .get(new_index)
            .map(|choice| gp::HoldJudgmentGraphic::new(&choice.key))
            .unwrap_or_default();
        state.player_profiles[player_idx].hold_judgment_graphic = setting;
        let (should_persist, side) = persist_ctx(player_idx);
        if should_persist {
            gp::update_hold_judgment_graphic_for_side(
                side,
                state.player_profiles[player_idx].hold_judgment_graphic.clone(),
            );
        }
        Outcome::persisted()
    },
};

pub static STEPCHART: CustomBinding = CustomBinding {
    apply: |state, player_idx, row_index, delta| {
        let Some(new_index) = cycle_choice_index(state, player_idx, row_index, delta) else {
            return Outcome::NONE;
        };
        let row = &state.rows()[row_index];
        let Some(diff_indices) = &row.choice_difficulty_indices else {
            return Outcome::persisted();
        };
        let Some(&difficulty_idx) = diff_indices.get(new_index) else {
            return Outcome::persisted();
        };
        state.chart_steps_index[player_idx] = difficulty_idx;
        if difficulty_idx < crate::engine::present::color::FILE_DIFFICULTY_NAMES.len() {
            state.chart_difficulty_index[player_idx] = difficulty_idx;
        }
        Outcome::persisted()
    },
};

/// `ActionOnMissedTarget` has no profile-side effect today; treat it like a
/// pure cursor-advance row so the change-value SFX still plays.
pub static ACTION_ON_MISSED_TARGET: CustomBinding = CustomBinding {
    apply: |state, player_idx, row_index, delta| {
        if cycle_choice_index(state, player_idx, row_index, delta).is_none() {
            return Outcome::NONE;
        }
        Outcome::persisted()
    },
};