//! deadsync-visionbot: an external client that watches the DeadSync game window,
//! detects when each scrolling arrow visually aligns with its receptor, and
//! injects a key press at that instant — so press precision and early/late drift
//! across a song can be measured.
//!
//! Precision is measured by the **game's own** audio-clock per-note offsets
//! (exported to JSON by the engine when `DEADSYNC_TIMING_EXPORT=1`). The bot's
//! self-log is a secondary diagnostic. The `timing-analyze` binary reads the
//! engine exports and reports per-lane statistics and drift (slope of offset vs
//! song time).
//!
//! The crate is organized as a platform-independent, unit-tested core plus thin
//! Windows-only capture/injection layers:
//! * [`frame`] — a borrowed view over a captured BGRA frame.
//! * [`geometry`] — axes, points, rectangles, resolution-relative helpers.
//! * [`calibration`] — per-lane receptor/ROI/key model (serde TOML).
//! * [`detect`] — leading-arrow centroid + confidence over an ROI (pure).
//! * [`timing`] — robust local fit → predicted crossing timestamp (pure).
//! * [`scheduler`] — applies the latency lead `L`, gates stale predictions.
//! * [`drift`] — offset-vs-time regression + per-lane stats (pure analyzer core).
//! * [`engine_export`] — deserialize mirror of the engine's timing JSON.
//! * [`selflog`] — bot self-log schema (pure) + writer (I/O).
//!
//! On Windows the [`capture`] and [`inject`] modules provide the WGC capture and
//! SendInput actuation; the `main` binary orchestrates capture→detect→timing→
//! schedule→inject.

pub mod calibration;
pub mod detect;
pub mod drift;
pub mod engine_export;
pub mod frame;
pub mod geometry;
pub mod scheduler;
pub mod selflog;
pub mod timing;

#[cfg(windows)]
pub mod capture;
#[cfg(windows)]
pub mod inject;
