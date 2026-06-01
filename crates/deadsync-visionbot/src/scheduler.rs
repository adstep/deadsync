//! Press scheduling: turn a predicted crossing time into an emit decision.
//!
//! The vision→detect→predict→inject pipeline has real latency, so we subtract a
//! manually-tuned constant **lead `L`** from the predicted crossing tick to pick
//! the moment to call `SendInput`. `L` is calibrated at bring-up by nudging it
//! until the engine-exported mean offset is ~0; crucially, **drift** (the slope
//! of offset vs song time) is independent of a constant `L`.
//!
//! This module is pure: it converts milliseconds to QPC ticks and decides
//! whether to emit now, wait, or skip a too-late prediction. Keeping latency
//! policy out of the vision math makes both independently testable.

/// Milliseconds to QPC ticks for a given counter frequency (ticks/second).
#[inline]
pub fn ms_to_ticks(ms: f64, qpc_freq: i64) -> i64 {
    (ms * qpc_freq as f64 / 1000.0).round() as i64
}

/// QPC ticks to milliseconds.
#[inline]
pub fn ticks_to_ms(ticks: i64, qpc_freq: i64) -> f64 {
    if qpc_freq == 0 {
        return 0.0;
    }
    ticks as f64 * 1000.0 / qpc_freq as f64
}

/// Scheduling decision for one predicted crossing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Wait until `at_qpc`, then call `SendInput` (keydown).
    Emit { at_qpc: i64 },
    /// The emit moment has already passed by more than the tolerance — skip.
    SkipLate { late_ticks: i64 },
}

/// Compute when to emit a keydown for a crossing at `crossing_qpc`.
///
/// * `lead_ms` — constant latency lead `L` subtracted from the crossing.
/// * `max_late_ms` — if the computed emit time is already this far in the past,
///   skip rather than fire an obviously-late press.
pub fn schedule(
    crossing_qpc: i64,
    now_qpc: i64,
    qpc_freq: i64,
    lead_ms: f64,
    max_late_ms: f64,
) -> Decision {
    let lead = ms_to_ticks(lead_ms, qpc_freq);
    let emit_at = crossing_qpc - lead;
    let late = now_qpc - emit_at;
    let max_late = ms_to_ticks(max_late_ms, qpc_freq);
    if late > max_late {
        Decision::SkipLate { late_ticks: late }
    } else {
        Decision::Emit { at_qpc: emit_at }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FREQ: i64 = 10_000_000; // 10 MHz, like WGC SystemRelativeTime (100ns)

    #[test]
    fn ms_tick_roundtrip() {
        assert_eq!(ms_to_ticks(1.0, FREQ), 10_000);
        assert!((ticks_to_ms(10_000, FREQ) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn emits_with_lead_subtracted() {
        let crossing = 1_000_000i64;
        let d = schedule(crossing, 0, FREQ, 5.0, 5.0);
        // lead 5ms = 50_000 ticks
        assert_eq!(d, Decision::Emit { at_qpc: 950_000 });
    }

    #[test]
    fn skips_when_too_late() {
        // crossing already passed; now is well beyond emit+tolerance
        let crossing = 100_000i64;
        let now = 1_000_000i64;
        let d = schedule(crossing, now, FREQ, 0.0, 5.0);
        match d {
            Decision::SkipLate { late_ticks } => assert!(late_ticks > 0),
            _ => panic!("expected skip"),
        }
    }
}
