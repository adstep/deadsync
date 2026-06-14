//! Pure, unit-testable math for the perceptual latency calibration tool.
//!
//! Everything here operates on plain nanosecond integers / second floats so it
//! can be exercised by `cargo test` without any audio device or render context.
//!
//! Timeline convention: taps and beats are expressed as nanoseconds on a single
//! monotonic timeline (host/QPC nanos when available, otherwise `Instant`
//! deltas). The metronome grid is anchored at `0`, so beat `k` is at
//! `k * period_ns`.

/// Number of beat taps collected per wizard pass.
pub(super) const WIZARD_SAMPLE_COUNT: usize = 16;
/// Leading taps discarded as warm-up before averaging.
pub(super) const WIZARD_WARMUP_TAPS: usize = 2;
/// Metronome period for the wizards (0.5 s -> 120 BPM).
pub(super) const WIZARD_PERIOD_SECONDS: f64 = 0.5;
/// Reaction stimuli collected per modality in the certifier.
pub(super) const CERTIFIER_SAMPLE_COUNT: usize = 6;
/// Standard-deviation gate (seconds). Taps noisier than this are not trusted to
/// auto-fill an offset; the user can still accept manually.
pub(super) const WIZARD_STDDEV_MAX_SECONDS: f32 = 0.040;

const NANOS_PER_SECOND: f64 = 1_000_000_000.0;

/// Convert a duration in seconds to integer nanoseconds (saturating, finite).
#[inline(always)]
pub(super) fn seconds_to_ns(seconds: f64) -> i128 {
    if !seconds.is_finite() {
        return 0;
    }
    (seconds * NANOS_PER_SECOND).round() as i128
}

#[inline(always)]
pub(super) fn ns_to_seconds(ns: i128) -> f64 {
    ns as f64 / NANOS_PER_SECOND
}

/// Signed timing error of a tap relative to the nearest metronome beat.
///
/// Positive = tapped late (after the beat), negative = tapped early. The grid is
/// anchored at 0 with spacing `period_ns`.
#[inline(always)]
pub(super) fn nearest_beat_error_ns(tap_ns: i128, period_ns: i128) -> i128 {
    if period_ns <= 0 {
        return 0;
    }
    let k = div_round_nearest(tap_ns, period_ns);
    tap_ns - k * period_ns
}

/// Round-half-away-from-zero integer division.
#[inline(always)]
fn div_round_nearest(numer: i128, denom: i128) -> i128 {
    if denom == 0 {
        return 0;
    }
    if (numer >= 0) == (denom > 0) {
        (numer + denom / 2) / denom
    } else {
        (numer - denom / 2) / denom
    }
}

/// Mean of a set of nanosecond errors.
#[inline(always)]
pub(super) fn mean_ns(samples: &[i128]) -> i128 {
    if samples.is_empty() {
        return 0;
    }
    let sum: i128 = samples.iter().copied().sum();
    div_round_nearest(sum, samples.len() as i128)
}

/// Population standard deviation of nanosecond errors, in seconds.
#[inline(always)]
pub(super) fn stddev_seconds(samples: &[i128], mean: i128) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let mut acc = 0.0_f64;
    for &s in samples {
        let d = ns_to_seconds(s - mean);
        acc += d * d;
    }
    (acc / samples.len() as f64).sqrt() as f32
}

/// Map a measured mean tap error to the offset value that cancels it.
///
/// The wizards present the stimulus exactly on the engine beat (audio clicks are
/// output-latency compensated; flashes peak on the beat). With anticipatory
/// tapping the mean error equals the player's perceived end-to-end latency for
/// that modality, so the offset that aligns judging/visuals with perception is
/// simply its negation. Result is rounded to whole milliseconds (matching the
/// rest of the offset UI) and clamped.
#[inline(always)]
pub(super) fn suggested_offset_seconds(mean_error_ns: i128, clamp_abs_ms: i32) -> f32 {
    let raw_ms = -ns_to_seconds(mean_error_ns) * 1000.0;
    let rounded = raw_ms.round() as i32;
    rounded.clamp(-clamp_abs_ms, clamp_abs_ms) as f32 / 1000.0
}

/// Whether a sample set is tight enough to be trusted for auto-fill.
#[inline(always)]
pub(super) fn within_stddev_gate(stddev: f32) -> bool {
    stddev.is_finite() && stddev <= WIZARD_STDDEV_MAX_SECONDS
}

/// Slice of collected samples used for averaging (warm-up taps dropped).
#[inline(always)]
pub(super) fn scored_samples(samples: &[i128]) -> &[i128] {
    if samples.len() > WIZARD_WARMUP_TAPS {
        &samples[WIZARD_WARMUP_TAPS..]
    } else {
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PERIOD_NS: i128 = 500_000_000;

    #[test]
    fn nearest_beat_error_picks_closest_beat() {
        // 10 ms after beat 4.
        let t = 4 * PERIOD_NS + 10_000_000;
        assert_eq!(nearest_beat_error_ns(t, PERIOD_NS), 10_000_000);
        // 10 ms before beat 4 -> negative error against beat 4.
        let t = 4 * PERIOD_NS - 10_000_000;
        assert_eq!(nearest_beat_error_ns(t, PERIOD_NS), -10_000_000);
    }

    #[test]
    fn nearest_beat_error_handles_exact_midpoint_and_zero() {
        assert_eq!(nearest_beat_error_ns(0, PERIOD_NS), 0);
        assert_eq!(nearest_beat_error_ns(PERIOD_NS, PERIOD_NS), 0);
        assert_eq!(nearest_beat_error_ns(12_345, 0), 0);
    }

    #[test]
    fn mean_and_stddev_basic() {
        let s = [10_000_000_i128, 20_000_000, 30_000_000];
        assert_eq!(mean_ns(&s), 20_000_000);
        let sd = stddev_seconds(&s, 20_000_000);
        assert!((sd - 0.008_164_966).abs() < 1e-6, "sd was {sd}");
    }

    #[test]
    fn mean_rounds_negative_half_away_from_zero() {
        assert_eq!(mean_ns(&[-1, -2]), -2);
        assert_eq!(mean_ns(&[1, 2]), 2);
    }

    #[test]
    fn suggested_offset_negates_and_rounds_to_ms() {
        // Tapped 23.4 ms late on average -> offset should be -23 ms.
        assert!((suggested_offset_seconds(23_400_000, 1000) - (-0.023)).abs() < 1e-6);
        // Tapped 12 ms early -> +12 ms.
        assert!((suggested_offset_seconds(-12_000_000, 1000) - 0.012).abs() < 1e-6);
    }

    #[test]
    fn suggested_offset_clamps() {
        assert!((suggested_offset_seconds(-5_000_000_000, 1000) - 1.0).abs() < 1e-6);
        assert!((suggested_offset_seconds(5_000_000_000, 1000) - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn stddev_gate_threshold() {
        assert!(within_stddev_gate(0.030));
        assert!(!within_stddev_gate(0.050));
        assert!(!within_stddev_gate(f32::NAN));
    }

    #[test]
    fn scored_samples_drops_warmup() {
        let s = [1_i128, 2, 3, 4, 5];
        assert_eq!(scored_samples(&s), &[3, 4, 5]);
        let short = [1_i128, 2];
        assert!(scored_samples(&short).is_empty());
    }

    #[test]
    fn seconds_ns_roundtrip() {
        assert_eq!(seconds_to_ns(0.5), 500_000_000);
        assert!((ns_to_seconds(500_000_000) - 0.5).abs() < 1e-12);
        assert_eq!(seconds_to_ns(f64::NAN), 0);
    }
}
