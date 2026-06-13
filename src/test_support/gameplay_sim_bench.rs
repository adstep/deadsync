//! Builders for the gameplay CPU/allocation benchmark (`tests/gameplay/bench.rs`).
//!
//! These live in `test_support` (always compiled, crate-internal visibility) so
//! the standalone benchmark binary can construct a fully-initialised gameplay
//! `State` — driving the real `gameplay::update` loop with autoplay — without
//! needing access to crate-private types or helpers.

use crate::game::gameplay;
use crate::game::parsing::simfile;
use crate::game::profile;
use crate::test_support::notefield_bench;
use deadsync_chart::{ChartData, GameplayChartData};
use deadsync_core::input::MAX_PLAYERS;
use deadsync_profile as profile_data;
use deadsync_rules::scroll::ScrollSpeedSetting;
use std::path::Path;
use std::sync::Arc;

/// A ready-to-run gameplay simulation: a fully initialised `State` (autoplay on)
/// plus the metadata the benchmark needs to drive and report on it.
pub struct SimTarget {
    /// Human-readable name for the benchmark report (song title or "synthetic").
    pub name: String,
    /// Chart difficulty label (e.g. "Challenge").
    pub chart_label: String,
    /// Gameplay state, ready to be stepped via `gameplay::update_simulated_clock`.
    pub state: gameplay::State,
    /// Absolute song time (seconds) of the final note's end; the benchmark steps
    /// the clock until just past this point.
    pub notes_end_seconds: f32,
    /// Total note objects in the active chart, for throughput reporting.
    pub note_count: usize,
}

const SCROLL_SPEED: ScrollSpeedSetting = ScrollSpeedSetting::CMod(620.0);

fn init_session() {
    profile::set_session_play_style(profile_data::PlayStyle::Single);
    profile::set_session_player_side(profile_data::PlayerSide::P1);
    profile::set_session_joined(true, false);
}

fn default_profiles() -> [profile_data::Profile; MAX_PLAYERS] {
    let mut profiles = [
        profile_data::Profile::default(),
        profile_data::Profile::default(),
    ];
    for p in &mut profiles {
        p.noteskin = profile_data::NoteSkin::new(profile_data::NoteSkin::CEL_NAME);
        p.scroll_speed = SCROLL_SPEED;
    }
    profiles
}

fn finish(name: String, chart_label: String, mut state: gameplay::State) -> SimTarget {
    state.autoplay_enabled = true;
    let note_count = state.notes.len();
    let notes_end_seconds = (state.notes_end_time_ns.max(0) as f64 / 1_000_000_000.0) as f32;
    SimTarget {
        name,
        chart_label,
        state,
        notes_end_seconds,
        note_count,
    }
}

/// Build a target from a dense, in-memory synthetic chart (no disk access).
///
/// Reuses the notefield benchmark fixture, which already assembles a 96-beat
/// stream peppered with holds, rolls and mines — enough to exercise the judging,
/// hold and stats paths.
pub fn synthetic_target() -> SimTarget {
    init_session();
    let (state, _profile) = notefield_bench::fixture().into_parts();
    finish(
        "synthetic (dense stream w/ holds, rolls, mines)".to_string(),
        "Challenge".to_string(),
        state,
    )
}

/// Build a target from a real simfile on disk.
///
/// `path` may point directly at a `.ssc`/`.sm` file or at a song directory.
/// `chart_ix` selects the chart by index; when `None`, the highest-meter chart is
/// used. Returns an error if parsing fails or the song has no playable charts.
pub fn song_target(path: &Path, chart_ix: Option<usize>) -> Result<SimTarget, String> {
    init_session();
    let song = simfile::load_song_for_bench(path)?;
    if song.charts.is_empty() {
        return Err(format!("Song '{}' has no charts", song.title));
    }
    let chart_ix = match chart_ix {
        Some(ix) if ix < song.charts.len() => ix,
        Some(ix) => {
            return Err(format!(
                "Chart index {ix} out of range (song has {} charts)",
                song.charts.len()
            ));
        }
        None => hardest_chart_ix(&song.charts),
    };

    let gameplay_chart: GameplayChartData = simfile::load_gameplay_charts(&song, &[chart_ix], 0.0)?
        .into_iter()
        .next()
        .ok_or_else(|| format!("Failed to materialize chart {chart_ix}"))?;
    let gameplay_chart = Arc::new(gameplay_chart);
    let chart = Arc::new(song.charts[chart_ix].clone());

    let charts: [Arc<ChartData>; MAX_PLAYERS] = [chart.clone(), chart];
    let gameplay_charts: [Arc<GameplayChartData>; MAX_PLAYERS] =
        [gameplay_chart.clone(), gameplay_chart];

    let chart_label = song.charts[chart_ix].difficulty.clone();
    let name = format!("{} ({})", song.title, song.charts[chart_ix].difficulty);

    let state = gameplay::init(
        song,
        charts,
        gameplay_charts,
        0,
        1.0,
        [SCROLL_SPEED, SCROLL_SPEED],
        default_profiles(),
        None,
        None,
        None,
        Arc::from("BENCH"),
        None,
        None,
        None,
        None,
        None,
        [0; MAX_PLAYERS],
    );

    Ok(finish(name, chart_label, state))
}

fn hardest_chart_ix(charts: &[ChartData]) -> usize {
    charts
        .iter()
        .enumerate()
        .max_by_key(|(_, chart)| chart.meter)
        .map(|(ix, _)| ix)
        .unwrap_or(0)
}
