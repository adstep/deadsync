//! Gameplay CPU / allocation benchmark.
//!
//! Drives the real `game::gameplay::update` loop over an entire chart with
//! **autoplay** enabled, using a deterministic injected clock
//! (`gameplay::update_simulated_clock`) so the run is reproducible and needs no
//! audio device. Reports steady-state CPU time and heap-allocation behaviour
//! per frame — the headline question being "does gameplay allocate during play?".
//!
//! Usage:
//!   cargo run --profile local --bin gameplay_bench
//!   cargo run --profile local --bin gameplay_bench -- <song-or-dir> [<song-or-dir> ...]
//!   cargo run --profile local --bin gameplay_bench -- --chart 3 <song.ssc>
//!
//! With no song arguments a dense built-in synthetic chart is used so the bench
//! always has something to measure.

use deadsync::game::gameplay;
use deadsync::test_support::gameplay_sim_bench::{self, SimTarget};
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc::new();

/// Frames run before measurement to settle process-global lazy state (noteskin
/// caches, thread-locals) so they don't pollute the per-frame allocation count.
const WARMUP_FRAMES: usize = 600;
/// Extra song time simulated past the final note, so end-of-song handling runs.
const TAIL_SECONDS: f64 = 2.0;
/// Hard cap on simulated frames per run (safety against pathological charts).
const FRAME_CAP: usize = 4_000_000;
/// Frame rates the loop is benchmarked at.
const FRAME_RATES_HZ: [f64; 2] = [240.0, 60.0];

// ---------------------------------------------------------------------------
// Counting allocator (mirrors the proven harness in tests/engine_perf/bench.rs).
// ---------------------------------------------------------------------------

struct CountingAlloc {
    alloc_calls: AtomicU64,
    dealloc_calls: AtomicU64,
    realloc_calls: AtomicU64,
    alloc_bytes: AtomicU64,
    free_bytes: AtomicU64,
    live_bytes: AtomicU64,
    peak_live_bytes: AtomicU64,
    measure_peak_live_bytes: AtomicU64,
}

#[derive(Clone, Copy)]
struct AllocSnapshot {
    alloc_calls: u64,
    dealloc_calls: u64,
    realloc_calls: u64,
    alloc_bytes: u64,
    free_bytes: u64,
    live_bytes: u64,
    measure_peak_live_bytes: u64,
}

#[derive(Clone, Copy)]
struct AllocDelta {
    alloc_calls: u64,
    dealloc_calls: u64,
    realloc_calls: u64,
    alloc_bytes: u64,
    free_bytes: u64,
    live_bytes: u64,
    peak_live_delta: u64,
}

impl CountingAlloc {
    const fn new() -> Self {
        Self {
            alloc_calls: AtomicU64::new(0),
            dealloc_calls: AtomicU64::new(0),
            realloc_calls: AtomicU64::new(0),
            alloc_bytes: AtomicU64::new(0),
            free_bytes: AtomicU64::new(0),
            live_bytes: AtomicU64::new(0),
            peak_live_bytes: AtomicU64::new(0),
            measure_peak_live_bytes: AtomicU64::new(0),
        }
    }

    fn begin_measurement(&self) -> AllocSnapshot {
        let live = self.live_bytes.load(Ordering::Relaxed);
        self.measure_peak_live_bytes.store(live, Ordering::Relaxed);
        self.snapshot()
    }

    fn snapshot(&self) -> AllocSnapshot {
        AllocSnapshot {
            alloc_calls: self.alloc_calls.load(Ordering::Relaxed),
            dealloc_calls: self.dealloc_calls.load(Ordering::Relaxed),
            realloc_calls: self.realloc_calls.load(Ordering::Relaxed),
            alloc_bytes: self.alloc_bytes.load(Ordering::Relaxed),
            free_bytes: self.free_bytes.load(Ordering::Relaxed),
            live_bytes: self.live_bytes.load(Ordering::Relaxed),
            measure_peak_live_bytes: self.measure_peak_live_bytes.load(Ordering::Relaxed),
        }
    }

    fn add_live(&self, size: usize) {
        let live = self.live_bytes.fetch_add(size as u64, Ordering::Relaxed) + size as u64;
        update_peak(&self.peak_live_bytes, live);
        update_peak(&self.measure_peak_live_bytes, live);
    }

    fn sub_live(&self, size: usize) {
        let _ = self.live_bytes.fetch_sub(size as u64, Ordering::Relaxed);
    }
}

impl AllocSnapshot {
    fn diff(self, start: Self) -> AllocDelta {
        AllocDelta {
            alloc_calls: self.alloc_calls.saturating_sub(start.alloc_calls),
            dealloc_calls: self.dealloc_calls.saturating_sub(start.dealloc_calls),
            realloc_calls: self.realloc_calls.saturating_sub(start.realloc_calls),
            alloc_bytes: self.alloc_bytes.saturating_sub(start.alloc_bytes),
            free_bytes: self.free_bytes.saturating_sub(start.free_bytes),
            live_bytes: self.live_bytes.saturating_sub(start.live_bytes),
            peak_live_delta: self
                .measure_peak_live_bytes
                .saturating_sub(start.measure_peak_live_bytes),
        }
    }
}

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            self.alloc_calls.fetch_add(1, Ordering::Relaxed);
            self.alloc_bytes
                .fetch_add(layout.size() as u64, Ordering::Relaxed);
            self.add_live(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        self.dealloc_calls.fetch_add(1, Ordering::Relaxed);
        self.free_bytes
            .fetch_add(layout.size() as u64, Ordering::Relaxed);
        self.sub_live(layout.size());
    }

    unsafe fn realloc(&self, ptr: *mut u8, old: Layout, new_size: usize) -> *mut u8 {
        let out = unsafe { System.realloc(ptr, old, new_size) };
        if !out.is_null() {
            self.realloc_calls.fetch_add(1, Ordering::Relaxed);
            if new_size >= old.size() {
                let delta = new_size - old.size();
                self.alloc_bytes.fetch_add(delta as u64, Ordering::Relaxed);
                self.add_live(delta);
            } else {
                let delta = old.size() - new_size;
                self.free_bytes.fetch_add(delta as u64, Ordering::Relaxed);
                self.sub_live(delta);
            }
        }
        out
    }
}

fn update_peak(slot: &AtomicU64, value: u64) {
    let mut observed = slot.load(Ordering::Relaxed);
    while value > observed {
        match slot.compare_exchange_weak(observed, value, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(actual) => observed = actual,
        }
    }
}

// ---------------------------------------------------------------------------
// CLI + target specs
// ---------------------------------------------------------------------------

/// A buildable benchmark target. Kept as a spec (rather than a built `State`) so
/// each run can construct a fresh `State` after warm-up.
enum TargetSpec {
    Synthetic,
    Song {
        path: PathBuf,
        chart_ix: Option<usize>,
    },
}

impl TargetSpec {
    fn build(&self) -> Result<SimTarget, String> {
        match self {
            TargetSpec::Synthetic => Ok(gameplay_sim_bench::synthetic_target()),
            TargetSpec::Song { path, chart_ix } => gameplay_sim_bench::song_target(path, *chart_ix),
        }
    }
}

struct Cli {
    specs: Vec<TargetSpec>,
}

fn parse_cli() -> Result<Cli, String> {
    let mut chart_ix: Option<usize> = None;
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--chart" => {
                let val = args
                    .next()
                    .ok_or_else(|| "--chart requires a value".to_string())?;
                chart_ix = Some(
                    val.parse::<usize>()
                        .map_err(|_| format!("invalid --chart value: {val}"))?,
                );
            }
            other if other.starts_with("--chart=") => {
                let val = &other["--chart=".len()..];
                chart_ix = Some(
                    val.parse::<usize>()
                        .map_err(|_| format!("invalid --chart value: {val}"))?,
                );
            }
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            other if other.starts_with("--") => {
                return Err(format!("unknown flag: {other}"));
            }
            other => paths.push(PathBuf::from(other)),
        }
    }

    let specs = if paths.is_empty() {
        vec![TargetSpec::Synthetic]
    } else {
        paths
            .into_iter()
            .map(|path| TargetSpec::Song { path, chart_ix })
            .collect()
    };
    Ok(Cli { specs })
}

fn print_usage() {
    println!("gameplay CPU / allocation benchmark");
    println!("usage: gameplay_bench [--chart <ix>] [<song-or-dir> ...]");
    println!("  no song args  -> dense synthetic chart");
    println!("  <song-or-dir> -> .ssc/.sm file, or a directory containing one");
    println!("  --chart <ix>  -> chart index to play (default: hardest by meter)");
}

// ---------------------------------------------------------------------------
// Measurement
// ---------------------------------------------------------------------------

struct RunResult {
    frames: usize,
    elapsed: Duration,
    worst_frame: Duration,
    alloc: AllocDelta,
    checksum: u64,
}

fn run_at_rate(spec: &TargetSpec, notes_end_seconds: f32, dt_hz: f64) -> Result<RunResult, String> {
    let dt = (1.0 / dt_hz) as f32;
    let dt_ns = (1_000_000_000.0 / dt_hz) as i64;
    let end_ns = ((notes_end_seconds as f64 + TAIL_SECONDS) * 1_000_000_000.0) as i64;

    // Warm-up on a throwaway state to settle global caches; counts discarded.
    {
        let mut warm = spec.build()?;
        let mut frame: i64 = 0;
        while (frame as usize) < WARMUP_FRAMES {
            let music_ns = frame.saturating_mul(dt_ns);
            black_box(gameplay::update_simulated_clock(
                &mut warm.state,
                dt,
                music_ns,
                1.0,
            ));
            frame += 1;
        }
    }

    // Fresh state for the measured pass.
    let mut target = spec.build()?;
    let mut worst_frame = Duration::ZERO;
    let start_snapshot = ALLOC.begin_measurement();
    let start = Instant::now();

    let mut checksum = 0u64;
    let mut frame: i64 = 0;
    loop {
        let music_ns = frame.saturating_mul(dt_ns);
        let frame_start = Instant::now();
        let action = gameplay::update_simulated_clock(&mut target.state, dt, music_ns, 1.0);
        let frame_elapsed = frame_start.elapsed();
        if frame_elapsed > worst_frame {
            worst_frame = frame_elapsed;
        }
        black_box(&action);
        checksum = checksum.wrapping_add(target.state.players[0].combo as u64);
        checksum ^=
            (target.state.players[0].life.to_bits() as u64).rotate_left((frame & 63) as u32);
        frame += 1;
        if music_ns >= end_ns || frame as usize >= FRAME_CAP {
            break;
        }
    }

    let elapsed = start.elapsed();
    let alloc = ALLOC.snapshot().diff(start_snapshot);

    Ok(RunResult {
        frames: frame as usize,
        elapsed,
        worst_frame,
        alloc,
        checksum,
    })
}

fn print_run(dt_hz: f64, result: &RunResult) {
    let frames = result.frames.max(1) as f64;
    let total_ms = result.elapsed.as_secs_f64() * 1000.0;
    let us_per_frame = result.elapsed.as_secs_f64() * 1_000_000.0 / frames;
    let worst_us = result.worst_frame.as_secs_f64() * 1_000_000.0;
    let allocs_per_frame = result.alloc.alloc_calls as f64 / frames;
    let deallocs_per_frame = result.alloc.dealloc_calls as f64 / frames;
    let bytes_per_frame = result.alloc.alloc_bytes as f64 / frames;
    let net_live = result.alloc.live_bytes as i64 - result.alloc.free_bytes as i64;

    println!(
        "  {:>4.0} Hz  frames {:>7}  {:>8.2} ms total  {:>7.3} us/frame avg  {:>8.3} us worst",
        dt_hz, result.frames, total_ms, us_per_frame, worst_us
    );
    println!(
        "           alloc/frame {:>7.3}  dealloc/frame {:>7.3}  bytes/frame {:>9.1}  reallocs {:>6}  net live {:>+11}  peak +{:<10}  checksum {:#018x}",
        allocs_per_frame,
        deallocs_per_frame,
        bytes_per_frame,
        result.alloc.realloc_calls,
        net_live,
        result.alloc.peak_live_delta,
        result.checksum
    );
}

fn main() {
    deadsync_platform::host_time::init();
    deadsync::assets::i18n::init("en");

    let cli = match parse_cli() {
        Ok(cli) => cli,
        Err(err) => {
            eprintln!("error: {err}");
            print_usage();
            std::process::exit(2);
        }
    };

    println!("gameplay CPU / allocation benchmark");
    println!(
        "autoplay over full chart, deterministic clock; warm-up {WARMUP_FRAMES} frames per run\n"
    );

    let mut any_failed = false;
    for spec in &cli.specs {
        // Build once up front to validate and read chart metadata.
        let info = match spec.build() {
            Ok(target) => target,
            Err(err) => {
                eprintln!("skipping target: {err}");
                any_failed = true;
                continue;
            }
        };
        println!(
            "{}  [chart: {}, notes: {}, length: {:.1}s]",
            info.name, info.chart_label, info.note_count, info.notes_end_seconds
        );
        let notes_end_seconds = info.notes_end_seconds;
        drop(info);

        for &dt_hz in &FRAME_RATES_HZ {
            match run_at_rate(spec, notes_end_seconds, dt_hz) {
                Ok(result) => print_run(dt_hz, &result),
                Err(err) => {
                    eprintln!("  run failed at {dt_hz} Hz: {err}");
                    any_failed = true;
                }
            }
        }
        println!();
    }

    if any_failed {
        std::process::exit(1);
    }
}
