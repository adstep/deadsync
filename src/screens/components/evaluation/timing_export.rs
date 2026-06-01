//! Opt-in per-note timing telemetry export.
//!
//! The evaluation screen already computes, for every play, the authoritative
//! per-note timing data the game judged against its audio (music) clock —
//! [`timing_stats::ScatterPoint`] (song time + signed offset), aggregate
//! [`timing_stats::TimingStats`], and per-column/foot
//! [`timing_stats::ArrowTimingStats`]. That data is normally consumed to draw
//! the evaluation graphs and then dropped.
//!
//! When the `DEADSYNC_TIMING_EXPORT` environment variable is truthy, this module
//! serializes that existing data to a JSON file so an external analysis tool can
//! measure press precision and early/late drift across a song. It performs no new
//! timing math and is a no-op when the flag is unset, so normal play is
//! unaffected.
//!
//! Output directory:
//! * `DEADSYNC_TIMING_EXPORT_DIR` if set, else `<data_dir>/save/timing_exports`.

use std::path::PathBuf;

use deadsync_rules::timing as timing_stats;
use serde::{Deserialize, Serialize};

use super::super::super::evaluation::ScoreInfo;

const SCHEMA: &str = "deadsync.timing_export";
const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimingExportSession {
    pub schema: String,
    pub version: u32,
    pub exported_at_unix_ms: u128,
    pub engine_version: String,
    pub run: RunMeta,
    pub summary: Summary,
    pub notes: Vec<NotePoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunMeta {
    pub song_title: String,
    pub chart_hash: String,
    pub chart_type: String,
    pub difficulty: String,
    pub meter: u32,
    pub profile_name: String,
    pub player_side: String,
    pub music_rate: f32,
    pub score_percent: f64,
    pub grade: String,
    /// True when the run was autoplay/replay-driven (a control run that presses
    /// on audio time rather than from vision). Such runs should show ~0 offset
    /// and ~0 drift and validate the measurement chain.
    pub autoplay_or_replay: bool,
    pub graph_first_second: f32,
    pub graph_last_second: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    pub mean_ms: f32,
    pub mean_abs_ms: f32,
    pub stddev_ms: f32,
    pub max_abs_ms: f32,
    pub per_column: Vec<Bucket>,
    pub left_foot: Bucket,
    pub right_foot: Bucket,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bucket {
    pub count: u32,
    pub mean_ms: f32,
    pub mean_abs_ms: f32,
    pub stddev_ms: f32,
    pub max_abs_ms: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotePoint {
    /// Song time of the note in seconds.
    pub t: f32,
    /// Signed audio-clock timing error in milliseconds (negative = early,
    /// positive = late). `None` for missed notes.
    pub offset_ms: Option<f32>,
    /// Arrow Cloud-style direction code: 1..=4 for L/D/U/R, other values for
    /// jumps/chords.
    pub dir: u8,
    pub stream: bool,
    pub left_foot: bool,
    pub miss: bool,
    pub miss_because_held: bool,
}

impl From<&timing_stats::ArrowTimingBucket> for Bucket {
    fn from(b: &timing_stats::ArrowTimingBucket) -> Self {
        Bucket {
            count: b.count,
            mean_ms: b.stats.mean_ms,
            mean_abs_ms: b.stats.mean_abs_ms,
            stddev_ms: b.stats.stddev_ms,
            max_abs_ms: b.stats.max_abs_ms,
        }
    }
}

#[inline]
fn enabled() -> bool {
    std::env::var("DEADSYNC_TIMING_EXPORT")
        .map(|v| {
            let v = v.trim();
            !(v.is_empty() || v == "0" || v.eq_ignore_ascii_case("false"))
        })
        .unwrap_or(false)
}

fn export_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("DEADSYNC_TIMING_EXPORT_DIR") {
        let trimmed = dir.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    crate::config::dirs::app_dirs()
        .data_dir
        .join("save")
        .join("timing_exports")
}

/// Build the serializable session from a finalized [`ScoreInfo`].
pub fn build_session(si: &ScoreInfo, exported_at_unix_ms: u128) -> TimingExportSession {
    let notes = si
        .scatter
        .iter()
        .map(|p| NotePoint {
            t: p.time_sec,
            offset_ms: p.offset_ms,
            dir: p.direction_code,
            stream: p.is_stream,
            left_foot: p.is_left_foot,
            miss: p.offset_ms.is_none(),
            miss_because_held: p.miss_because_held,
        })
        .collect();

    let summary = Summary {
        mean_ms: si.timing.mean_ms,
        mean_abs_ms: si.timing.mean_abs_ms,
        stddev_ms: si.timing.stddev_ms,
        max_abs_ms: si.timing.max_abs_ms,
        per_column: si.arrow_timing.per_column.iter().map(Bucket::from).collect(),
        left_foot: Bucket::from(&si.arrow_timing.left_foot),
        right_foot: Bucket::from(&si.arrow_timing.right_foot),
    };

    TimingExportSession {
        schema: SCHEMA.to_string(),
        version: SCHEMA_VERSION,
        exported_at_unix_ms,
        engine_version: env!("CARGO_PKG_VERSION").to_string(),
        run: RunMeta {
            song_title: si.song.title.clone(),
            chart_hash: si.chart.short_hash.clone(),
            chart_type: si.chart.chart_type.clone(),
            difficulty: si.chart.difficulty.clone(),
            meter: si.chart.meter,
            profile_name: si.profile_name.clone(),
            player_side: format!("{:?}", si.side),
            music_rate: si.music_rate,
            score_percent: si.score_percent,
            grade: format!("{:?}", si.grade),
            autoplay_or_replay: si.disqualified,
            graph_first_second: si.graph_first_second,
            graph_last_second: si.graph_last_second,
        },
        summary,
        notes,
    }
}

/// Export the per-note timing data for a finalized play when the
/// `DEADSYNC_TIMING_EXPORT` flag is set. No-op otherwise.
pub fn maybe_export(si: &ScoreInfo) {
    if !enabled() {
        return;
    }

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);

    let session = build_session(si, now_ms);

    let dir = export_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        log::warn!("timing export: failed to create dir {dir:?}: {e}");
        return;
    }

    let hash = sanitize(&session.run.chart_hash);
    let side = sanitize(&session.run.player_side);
    let file_name = format!("{now_ms}_{hash}_{side}.json");
    let path = dir.join(file_name);

    let json = match serde_json::to_string_pretty(&session) {
        Ok(j) => j,
        Err(e) => {
            log::warn!("timing export: failed to serialize: {e}");
            return;
        }
    };

    match std::fs::write(&path, json) {
        Ok(()) => log::info!(
            "timing export: wrote {} notes to {path:?}",
            session.notes.len()
        ),
        Err(e) => log::warn!("timing export: failed to write {path:?}: {e}"),
    }
}

fn sanitize(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('_');
    if trimmed.is_empty() {
        "x".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> TimingExportSession {
        TimingExportSession {
            schema: SCHEMA.to_string(),
            version: SCHEMA_VERSION,
            exported_at_unix_ms: 1_700_000_000_000,
            engine_version: "0.0.0-test".to_string(),
            run: RunMeta {
                song_title: "Test Song".to_string(),
                chart_hash: "abc123".to_string(),
                chart_type: "dance-single".to_string(),
                difficulty: "Challenge".to_string(),
                meter: 12,
                profile_name: "Player 1".to_string(),
                player_side: "P1".to_string(),
                music_rate: 1.0,
                score_percent: 99.12,
                grade: "AAA".to_string(),
                autoplay_or_replay: false,
                graph_first_second: 0.0,
                graph_last_second: 120.0,
            },
            summary: Summary {
                mean_ms: -1.2,
                mean_abs_ms: 8.4,
                stddev_ms: 11.0,
                max_abs_ms: 90.0,
                per_column: vec![Bucket {
                    count: 3,
                    mean_ms: -1.0,
                    mean_abs_ms: 5.0,
                    stddev_ms: 4.0,
                    max_abs_ms: 12.0,
                }],
                left_foot: Bucket {
                    count: 1,
                    mean_ms: 2.0,
                    mean_abs_ms: 2.0,
                    stddev_ms: 0.0,
                    max_abs_ms: 2.0,
                },
                right_foot: Bucket {
                    count: 1,
                    mean_ms: -3.0,
                    mean_abs_ms: 3.0,
                    stddev_ms: 0.0,
                    max_abs_ms: 3.0,
                },
            },
            notes: vec![
                NotePoint {
                    t: 1.5,
                    offset_ms: Some(3.2),
                    dir: 1,
                    stream: true,
                    left_foot: false,
                    miss: false,
                    miss_because_held: false,
                },
                NotePoint {
                    t: 2.0,
                    offset_ms: None,
                    dir: 2,
                    stream: false,
                    left_foot: true,
                    miss: true,
                    miss_because_held: false,
                },
            ],
        }
    }

    #[test]
    fn session_round_trips_through_json() {
        let original = sample();
        let json = serde_json::to_string_pretty(&original).expect("serialize");
        let decoded: TimingExportSession = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, decoded);
    }

    #[test]
    fn miss_offset_serializes_as_null() {
        let json = serde_json::to_string(&sample()).expect("serialize");
        assert!(json.contains("\"offset_ms\":null"));
    }

    #[test]
    fn sanitize_strips_unsafe_chars() {
        assert_eq!(sanitize("P1"), "P1");
        assert_eq!(sanitize("a/b:c"), "a_b_c");
        assert_eq!(sanitize("///"), "x");
    }
}
