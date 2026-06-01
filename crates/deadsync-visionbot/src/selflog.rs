//! Bot self-log: a *secondary* record of what the bot intended and did.
//!
//! This is **not** the ground truth for precision — that is the engine's
//! audio-clock export. The self-log lets us confirm the bot actually pressed
//! when it intended (separating engine visual/audio skew from bot actuation
//! latency) and diagnose the pipeline. The schema is pure; the [`Writer`] is the
//! only I/O.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Header describing the capture session, written once.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionHeader {
    pub bot_version: String,
    pub started_unix_ms: u128,
    pub calibration_id: String,
    pub window_title: String,
    pub frame_width: u32,
    pub frame_height: u32,
    pub qpc_freq: i64,
    pub lead_ms: f64,
    pub keyup_ms: f64,
}

/// One press attempt.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PressRecord {
    pub lane: u8,
    /// Predicted crossing time (QPC) from the timing fit.
    pub predicted_crossing_qpc: i64,
    /// Scheduled emit time (= predicted − lead).
    pub scheduled_emit_qpc: i64,
    /// QPC just before the `SendInput` keydown call.
    pub sendinput_call_qpc: i64,
    /// QPC just after the `SendInput` keydown call returns.
    pub sendinput_return_qpc: i64,
    /// QPC of the keyup call.
    pub keyup_qpc: i64,
    /// Whether DeadSync was the foreground window at emit time.
    pub foreground: bool,
    /// Timing-fit confidence (R²) for this crossing.
    pub fit_r2: f64,
    /// Number of frames used in the fit.
    pub fit_frames: u32,
    /// `emit − predicted_crossing` in QPC ticks (negative = led the crossing).
    pub scheduler_error_ticks: i64,
}

/// Streaming writer that emits a JSONL event log and a flat CSV of presses.
pub struct Writer {
    jsonl: BufWriter<File>,
    csv: BufWriter<File>,
    wrote_csv_header: bool,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Event<'a> {
    Header(&'a SessionHeader),
    Press(&'a PressRecord),
}

impl Writer {
    /// Open `<dir>/<stem>.jsonl` and `<dir>/<stem>.csv`, writing the header.
    pub fn create(dir: &Path, stem: &str, header: &SessionHeader) -> std::io::Result<Writer> {
        std::fs::create_dir_all(dir)?;
        let jsonl = BufWriter::new(File::create(dir.join(format!("{stem}.jsonl")))?);
        let csv = BufWriter::new(File::create(dir.join(format!("{stem}.csv")))?);
        let mut w = Writer {
            jsonl,
            csv,
            wrote_csv_header: false,
        };
        w.write_jsonl(&Event::Header(header))?;
        Ok(w)
    }

    fn write_jsonl(&mut self, ev: &Event) -> std::io::Result<()> {
        let line = serde_json::to_string(ev)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        self.jsonl.write_all(line.as_bytes())?;
        self.jsonl.write_all(b"\n")
    }

    /// Append one press to both the JSONL and CSV outputs.
    pub fn write_press(&mut self, p: &PressRecord) -> std::io::Result<()> {
        self.write_jsonl(&Event::Press(p))?;
        if !self.wrote_csv_header {
            writeln!(
                self.csv,
                "lane,predicted_crossing_qpc,scheduled_emit_qpc,sendinput_call_qpc,sendinput_return_qpc,keyup_qpc,foreground,fit_r2,fit_frames,scheduler_error_ticks"
            )?;
            self.wrote_csv_header = true;
        }
        writeln!(
            self.csv,
            "{},{},{},{},{},{},{},{:.5},{},{}",
            p.lane,
            p.predicted_crossing_qpc,
            p.scheduled_emit_qpc,
            p.sendinput_call_qpc,
            p.sendinput_return_qpc,
            p.keyup_qpc,
            p.foreground as u8,
            p.fit_r2,
            p.fit_frames,
            p.scheduler_error_ticks
        )
    }

    pub fn flush(&mut self) -> std::io::Result<()> {
        self.jsonl.flush()?;
        self.csv.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_and_press_round_trip_jsonl() {
        let h = SessionHeader {
            bot_version: "0.1.0".into(),
            started_unix_ms: 1,
            calibration_id: "cal".into(),
            window_title: "DeadSync".into(),
            frame_width: 1920,
            frame_height: 1080,
            qpc_freq: 10_000_000,
            lead_ms: 5.0,
            keyup_ms: 16.0,
        };
        let ev = Event::Header(&h);
        let s = serde_json::to_string(&ev).unwrap();
        assert!(s.contains("\"kind\":\"header\""));

        let p = PressRecord {
            lane: 2,
            predicted_crossing_qpc: 1000,
            scheduled_emit_qpc: 950,
            sendinput_call_qpc: 951,
            sendinput_return_qpc: 953,
            keyup_qpc: 1100,
            foreground: true,
            fit_r2: 0.98,
            fit_frames: 4,
            scheduler_error_ticks: -50,
        };
        let s2 = serde_json::to_string(&Event::Press(&p)).unwrap();
        assert!(s2.contains("\"kind\":\"press\""));
    }

    #[test]
    fn writer_emits_files() {
        let dir = std::env::temp_dir().join(format!("dsvb_selflog_{}", std::process::id()));
        let h = SessionHeader {
            bot_version: "0.1.0".into(),
            started_unix_ms: 1,
            calibration_id: "cal".into(),
            window_title: "DeadSync".into(),
            frame_width: 800,
            frame_height: 600,
            qpc_freq: 10_000_000,
            lead_ms: 5.0,
            keyup_ms: 16.0,
        };
        let mut w = Writer::create(&dir, "session", &h).expect("create");
        w.write_press(&PressRecord {
            lane: 0,
            predicted_crossing_qpc: 1,
            scheduled_emit_qpc: 1,
            sendinput_call_qpc: 1,
            sendinput_return_qpc: 1,
            keyup_qpc: 1,
            foreground: true,
            fit_r2: 1.0,
            fit_frames: 3,
            scheduler_error_ticks: 0,
        })
        .expect("write");
        w.flush().expect("flush");

        let csv = std::fs::read_to_string(dir.join("session.csv")).expect("read csv");
        assert!(csv.lines().count() >= 2);
        let jsonl = std::fs::read_to_string(dir.join("session.jsonl")).expect("read jsonl");
        assert!(jsonl.lines().count() >= 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
