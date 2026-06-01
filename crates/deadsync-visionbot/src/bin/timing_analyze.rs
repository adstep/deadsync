//! `timing-analyze`: read engine timing-export JSON files and report press
//! precision and early/late drift.
//!
//! Usage:
//!   timing-analyze <file-or-dir> [more files/dirs ...]
//!
//! For each play and in aggregate it prints per-lane and overall offset
//! statistics (mean/median/stddev/min/max, early vs late counts) and the drift
//! slope (ms per minute of song time) from a linear regression of the engine's
//! audio-clock offset against song time. Misses are reported separately.
//!
//! The exported offsets are the engine's **authoritative** audio-clock timing
//! error per note. A nonzero drift slope means the on-screen arrows drifted
//! relative to the audio the engine judged against. Note the engine renders from
//! a smoothed display clock (≤20 ms lag / ≤6 ms lead vs audio); that smoothing
//! is the physical cause of any fixed visual/audio skew, but does not affect
//! these audio-clock offsets.

use std::path::{Path, PathBuf};

use deadsync_visionbot::drift::{DriftFit, Histogram, OffsetSample, OffsetStats, linear_regression};
use deadsync_visionbot::engine_export::{self, TimingExportSession};

fn collect_files(args: &[String]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for a in args {
        let p = PathBuf::from(a);
        if p.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&p) {
                for e in rd.flatten() {
                    let ep = e.path();
                    if ep.extension().and_then(|s| s.to_str()) == Some("json") {
                        out.push(ep);
                    }
                }
            }
        } else if p.is_file() {
            out.push(p);
        } else {
            eprintln!("warning: path not found: {}", p.display());
        }
    }
    out.sort();
    out
}

fn non_miss_samples(s: &TimingExportSession) -> Vec<OffsetSample> {
    s.notes
        .iter()
        .filter_map(|n| {
            n.offset_ms.map(|o| OffsetSample {
                t_sec: n.t as f64,
                offset_ms: o as f64,
            })
        })
        .collect()
}

fn fmt_stats(label: &str, stats: &OffsetStats) {
    println!(
        "  {label:<10} n={:<5} mean={:+7.2} median={:+7.2} sd={:6.2} min={:+7.2} max={:+7.2} early={} late={}",
        stats.count,
        stats.mean_ms,
        stats.median_ms,
        stats.stddev_ms,
        stats.min_ms,
        stats.max_ms,
        stats.early_count,
        stats.late_count,
    );
}

fn fmt_drift(fit: &DriftFit) {
    let dir = if fit.slope_ms_per_min.abs() < 1e-6 {
        "flat"
    } else if fit.slope_ms_per_min > 0.0 {
        "drifting LATE"
    } else {
        "drifting EARLY"
    };
    println!(
        "  drift: {:+.3} ms/min (slope {:+.5} ms/s, R²={:.3}, n={}) -> {dir}",
        fit.slope_ms_per_min, fit.slope_ms_per_s, fit.r2, fit.n
    );
}

fn dir_name(code: u8) -> &'static str {
    match code {
        1 => "Left",
        2 => "Down",
        3 => "Up",
        4 => "Right",
        _ => "Other",
    }
}

fn analyze_one(path: &Path, session: &TimingExportSession) {
    println!(
        "== {} | {} [{}] {} meter {} | rate {:.2} | {} | {} | grade {} {}",
        path.file_name().and_then(|s| s.to_str()).unwrap_or("?"),
        session.run.song_title,
        session.run.chart_type,
        session.run.difficulty,
        session.run.meter,
        session.run.music_rate,
        session.run.player_side,
        session.run.profile_name,
        session.run.grade,
        if session.run.autoplay_or_replay {
            "(AUTOPLAY/REPLAY control)"
        } else {
            ""
        },
    );

    let total = session.notes.len();
    let misses = session.notes.iter().filter(|n| n.miss).count();
    println!("  notes={total} misses={misses}");

    let samples = non_miss_samples(session);
    let offsets: Vec<f64> = samples.iter().map(|s| s.offset_ms).collect();
    if let Some(stats) = OffsetStats::compute(&offsets) {
        fmt_stats("overall", &stats);
    }
    if let Some(fit) = linear_regression(&samples) {
        fmt_drift(&fit);
    } else {
        println!("  drift: (insufficient data)");
    }

    // Per-lane (direction code 1..=4).
    for code in 1u8..=4 {
        let lane: Vec<f64> = session
            .notes
            .iter()
            .filter(|n| n.dir == code)
            .filter_map(|n| n.offset_ms.map(|o| o as f64))
            .collect();
        if let Some(stats) = OffsetStats::compute(&lane) {
            fmt_stats(dir_name(code), &stats);
        }
    }

    // Compact histogram of overall offsets.
    if !offsets.is_empty() {
        let h = Histogram::build(&offsets, 50.0, 5.0);
        print_histogram(&h);
    }
    println!();
}

fn print_histogram(h: &Histogram) {
    let max = h.bins.iter().copied().max().unwrap_or(0);
    if max == 0 {
        return;
    }
    println!("  histogram (5ms bins, -50..+50ms):");
    for (i, &count) in h.bins.iter().enumerate() {
        if count == 0 {
            continue;
        }
        let edge = h.min_edge_ms + i as f64 * h.bin_ms;
        let bar = "#".repeat(((count as f64 / max as f64) * 40.0).round() as usize);
        println!("    {edge:+6.0} | {bar} {count}");
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: timing-analyze <file-or-dir> [more ...]");
        std::process::exit(2);
    }

    let files = collect_files(&args);
    if files.is_empty() {
        eprintln!("no .json export files found");
        std::process::exit(1);
    }

    let mut all_samples: Vec<OffsetSample> = Vec::new();
    let mut all_offsets: Vec<f64> = Vec::new();
    let mut loaded = 0usize;

    for f in &files {
        match engine_export::load(f) {
            Ok(session) => {
                analyze_one(f, &session);
                let s = non_miss_samples(&session);
                all_offsets.extend(s.iter().map(|x| x.offset_ms));
                all_samples.extend(s);
                loaded += 1;
            }
            Err(e) => eprintln!("error reading {}: {e}", f.display()),
        }
    }

    if loaded > 1 {
        println!("==== AGGREGATE over {loaded} plays ====");
        if let Some(stats) = OffsetStats::compute(&all_offsets) {
            fmt_stats("all", &stats);
        }
        if let Some(fit) = linear_regression(&all_samples) {
            fmt_drift(&fit);
        }
    }
}
