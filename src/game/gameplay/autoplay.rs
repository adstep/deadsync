use crate::engine::input::INPUT_SLOT_INVALID;
use crate::game::note::NoteType;

use super::input::{lane_from_column, push_input_edge};
use super::{
    MAX_COLS, SongTimeNs, State, handle_hold_let_go, handle_hold_success, judge_a_lift,
    judge_a_tap, player_note_range, refresh_roll_life_on_step,
};

/// Standard deviation (in seconds) of the autoplay timing-jitter
/// distribution. The bot adds a per-note offset drawn from a normal
/// distribution centred on perfect timing with this σ. 18 ms keeps the
/// vast majority of hits inside the W1 / Fantastic window (`±21.5 ms`)
/// while the tails reach W2 / Excellent and beyond.
const AUTOPLAY_JITTER_STDDEV_S: f64 = 0.018;

/// Per-note probability that we replace the tight Gaussian draw with a
/// wide-tailed "shank" so the run produces the occasional W3 / miss.
/// Without this the σ alone is too narrow to ever push past 102 ms.
const AUTOPLAY_SHANK_PROBABILITY: f64 = 0.012;
const AUTOPLAY_SHANK_STDDEV_S: f64 = 0.075;

/// Deterministic xorshift64* — same input note index always produces
/// the same offset, so replays of the same chart under autoplay stay
/// reproducible. We mix the engine's `autoplay_jitter_seed` into the
/// note index so different sessions can produce different (but each
/// internally consistent) runs.
#[inline(always)]
fn xorshift64(mut state: u64) -> u64 {
    state ^= state >> 12;
    state ^= state << 25;
    state ^= state >> 27;
    state.wrapping_mul(0x2545_F491_4F6C_DD1D)
}

#[inline(always)]
fn unit_random_pair(seed: u64) -> (f64, f64) {
    // Two consecutive xorshift outputs converted to (0, 1] floats.
    let a = xorshift64(seed.wrapping_add(0xA341_316C_C8C8_E537));
    let b = xorshift64(seed.wrapping_add(0xB231_C7B5_8888_AAAA));
    let to_unit = |bits: u64| (bits >> 11) as f64 / ((1u64 << 53) as f64);
    // Clamp away from exactly 0 so ln() is well-defined in Box-Muller.
    let u1 = to_unit(a).max(f64::MIN_POSITIVE);
    let u2 = to_unit(b);
    (u1, u2)
}

/// Box-Muller transform: produces one standard-normal sample from two
/// uniform draws. We discard the second normal and accept the doubled
/// cost — this is per-note autoplay code, not a hot inner loop.
#[inline(always)]
fn normal_sample(seed: u64) -> f64 {
    let (u1, u2) = unit_random_pair(seed);
    let r = (-2.0 * u1.ln()).sqrt();
    let theta = 2.0 * std::f64::consts::PI * u2;
    r * theta.cos()
}

/// Returns the timing offset (seconds) the bot should add to a note's
/// nominal row time. Negative = press early, positive = press late.
/// Mixes a tight inner Gaussian with a rare wider-σ "shank" so the
/// distribution stays centred on perfect-timing-with-some-jitter while
/// still occasionally reaching past the Great window into a miss.
/// Deterministic per `note_index` so replays of the same chart stay
/// reproducible.
#[inline(always)]
fn autoplay_jitter_offset_s(note_index: usize) -> f64 {
    let seed = (note_index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let pick = xorshift64(seed.wrapping_add(0xDEAD_BEEF_CAFE_BABE));
    let pick_unit = (pick >> 11) as f64 / ((1u64 << 53) as f64);
    let sigma = if pick_unit < AUTOPLAY_SHANK_PROBABILITY {
        AUTOPLAY_SHANK_STDDEV_S
    } else {
        AUTOPLAY_JITTER_STDDEV_S
    };
    normal_sample(seed) * sigma
}

#[inline(always)]
pub(super) fn autoplay_blocks_scoring(state: &State) -> bool {
    live_autoplay_enabled(state)
}

#[inline(always)]
pub(super) const fn live_autoplay_enabled_from_flags(
    autoplay_enabled: bool,
    replay_mode: bool,
) -> bool {
    autoplay_enabled && !replay_mode
}

#[inline(always)]
pub(super) fn live_autoplay_enabled(state: &State) -> bool {
    live_autoplay_enabled_from_flags(state.autoplay_enabled, state.replay_mode)
}

#[inline(always)]
fn settle_due_autoplay_active_holds(state: &mut State, cutoff_time_ns: SongTimeNs) {
    for column in 0..state.num_cols {
        let Some(active) = state.active_holds[column].as_ref() else {
            continue;
        };
        if active.end_time_ns > cutoff_time_ns {
            continue;
        }
        let note_index = active.note_index;
        let end_time_ns = active.end_time_ns;
        let hold_succeeded = !active.let_go && active.life > 0.0;
        state.active_holds[column] = None;
        if hold_succeeded {
            handle_hold_success(state, column, note_index);
        } else {
            handle_hold_let_go(state, column, note_index, end_time_ns);
        }
    }
}

pub(super) fn run_autoplay(state: &mut State, now_music_time_ns: SongTimeNs) {
    if !state.autoplay_enabled {
        return;
    }

    for player in 0..state.num_players {
        let (note_start, note_end) = player_note_range(state, player);
        let mut cursor = state.autoplay_cursor[player].max(note_start);
        while cursor < note_end {
            while cursor < note_end && state.notes[cursor].result.is_some() {
                cursor += 1;
            }
            if cursor >= note_end {
                break;
            }

            let row = state.notes[cursor].row_index;
            let mut row_end = cursor + 1;
            while row_end < note_end && state.notes[row_end].row_index == row {
                row_end += 1;
            }
            let row_time_ns = state.note_time_cache_ns[cursor];
            if row_time_ns > now_music_time_ns {
                break;
            }
            // Finalize any already-ended autoplay holds before a new warped
            // row on the same lane can replace the active hold slot.
            settle_due_autoplay_active_holds(state, row_time_ns);
            for idx in cursor..row_end {
                let (result_is_some, is_fake, can_be_judged, note_type, col) = {
                    let note = &state.notes[idx];
                    (
                        note.result.is_some(),
                        note.is_fake,
                        note.can_be_judged,
                        note.note_type,
                        note.column,
                    )
                };
                // ITGmania PC_AUTOPLAY gets W1 from PlayerAI; the mine branch
                // treats that as an avoid, so mines are left for the overdue
                // avoid pass instead of being hit by live autoplay.
                if result_is_some
                    || is_fake
                    || !can_be_judged
                    || matches!(note_type, NoteType::Mine)
                {
                    continue;
                }

                if col >= state.num_cols {
                    continue;
                }

                state.autoplay_used = true;
                // Apply per-note timing jitter so the bot's judgments
                // form a normal distribution centred on perfect
                // timing instead of being a flat stream of W0s.
                // Offset is deterministic per `note_index` so the
                // same chart always produces the same distribution.
                let jitter_s = autoplay_jitter_offset_s(idx);
                let jitter_ns = (jitter_s * 1.0e9) as i64;
                let press_time_ns = row_time_ns.saturating_add(jitter_ns);
                match note_type {
                    NoteType::Lift => {
                        let _ = judge_a_lift(state, col, press_time_ns);
                    }
                    NoteType::Tap | NoteType::Hold | NoteType::Roll => {
                        let _ = judge_a_tap(state, col, press_time_ns);
                    }
                    NoteType::Mine | NoteType::Fake => {}
                }
            }

            cursor = row_end;
        }
        state.autoplay_cursor[player] = cursor;
    }

    let mut roll_cols = [usize::MAX; MAX_COLS];
    let mut roll_count = 0usize;
    for col in 0..state.num_cols {
        if state.active_holds[col]
            .as_ref()
            .is_some_and(|active| matches!(active.note_type, NoteType::Roll) && !active.let_go)
            && roll_count < MAX_COLS
        {
            roll_cols[roll_count] = col;
            roll_count += 1;
        }
    }
    for col in roll_cols.into_iter().take(roll_count) {
        refresh_roll_life_on_step(state, col, state.current_music_time_ns);
    }
}

pub(super) fn run_replay(state: &mut State) {
    if !state.autoplay_enabled || !state.replay_mode {
        return;
    }
    while state.replay_cursor < state.replay_input.len() {
        let edge = state.replay_input[state.replay_cursor];
        if edge.event_music_time_ns > state.current_music_time_ns {
            break;
        }
        state.replay_cursor += 1;
        let col = edge.lane_index as usize;
        if col >= state.num_cols {
            continue;
        }
        let Some(lane) = lane_from_column(col) else {
            continue;
        };
        push_input_edge(
            state,
            edge.source,
            lane,
            INPUT_SLOT_INVALID,
            edge.pressed,
            edge.event_music_time_ns,
            false,
        );
        state.autoplay_used = true;
    }
}
