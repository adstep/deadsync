//! Deserialization mirror of the engine's timing-export JSON.
//!
//! The engine writes one JSON file per play when `DEADSYNC_TIMING_EXPORT=1`
//! (see `src/screens/components/evaluation/timing_export.rs` in the main crate).
//! **The JSON is the contract** between the two — this crate intentionally does
//! not share a Rust type with the engine, so the game's screen layer stays
//! decoupled from the bot. Field names here must match the engine's
//! `#[derive(Serialize)]` structs.

use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct TimingExportSession {
    pub schema: String,
    pub version: u32,
    pub exported_at_unix_ms: u128,
    pub engine_version: String,
    pub run: RunMeta,
    pub summary: Summary,
    pub notes: Vec<NotePoint>,
}

#[derive(Debug, Clone, Deserialize)]
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
    pub autoplay_or_replay: bool,
    pub graph_first_second: f32,
    pub graph_last_second: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Summary {
    pub mean_ms: f32,
    pub mean_abs_ms: f32,
    pub stddev_ms: f32,
    pub max_abs_ms: f32,
    pub per_column: Vec<Bucket>,
    pub left_foot: Bucket,
    pub right_foot: Bucket,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Bucket {
    pub count: u32,
    pub mean_ms: f32,
    pub mean_abs_ms: f32,
    pub stddev_ms: f32,
    pub max_abs_ms: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NotePoint {
    pub t: f32,
    pub offset_ms: Option<f32>,
    pub dir: u8,
    pub stream: bool,
    pub left_foot: bool,
    pub miss: bool,
    pub miss_because_held: bool,
}

/// Load and parse one export file.
pub fn load(path: &Path) -> std::io::Result<TimingExportSession> {
    let text = std::fs::read_to_string(path)?;
    serde_json::from_str(&text).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
    {
      "schema": "deadsync.timing_export",
      "version": 1,
      "exported_at_unix_ms": 1700000000000,
      "engine_version": "0.4.420",
      "run": {
        "song_title": "S", "chart_hash": "h", "chart_type": "dance-single",
        "difficulty": "Hard", "meter": 9, "profile_name": "P", "player_side": "P1",
        "music_rate": 1.0, "score_percent": 98.5, "grade": "AA",
        "autoplay_or_replay": false, "graph_first_second": 0.0, "graph_last_second": 60.0
      },
      "summary": {
        "mean_ms": 1.0, "mean_abs_ms": 5.0, "stddev_ms": 6.0, "max_abs_ms": 40.0,
        "per_column": [{"count":2,"mean_ms":1.0,"mean_abs_ms":3.0,"stddev_ms":2.0,"max_abs_ms":6.0}],
        "left_foot": {"count":1,"mean_ms":0.0,"mean_abs_ms":0.0,"stddev_ms":0.0,"max_abs_ms":0.0},
        "right_foot": {"count":1,"mean_ms":0.0,"mean_abs_ms":0.0,"stddev_ms":0.0,"max_abs_ms":0.0}
      },
      "notes": [
        {"t":1.0,"offset_ms":2.0,"dir":1,"stream":false,"left_foot":true,"miss":false,"miss_because_held":false},
        {"t":2.0,"offset_ms":null,"dir":2,"stream":false,"left_foot":false,"miss":true,"miss_because_held":false}
      ]
    }"#;

    #[test]
    fn parses_sample() {
        let s: TimingExportSession = serde_json::from_str(SAMPLE).expect("parse");
        assert_eq!(s.version, 1);
        assert_eq!(s.notes.len(), 2);
        assert_eq!(s.notes[0].offset_ms, Some(2.0));
        assert_eq!(s.notes[1].offset_ms, None);
        assert!(s.notes[1].miss);
    }
}
