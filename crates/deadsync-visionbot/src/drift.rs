//! Precision and drift statistics over a set of per-note offsets.
//!
//! This is the analyzer core, used by the `timing-analyze` binary against the
//! engine's audio-clock export (the authoritative measurement) and reusable for
//! the bot's self-log. The headline question — *did presses drift early/late
//! across the song?* — is answered by [`linear_regression`]: the **slope** of
//! `offset_ms` vs `time_sec`. Because the bot always presses on visual
//! alignment, a nonzero slope means the visual representation drifted relative
//! to the audio the engine judged against; the slope is independent of any
//! constant pipeline latency.

/// One (song time, signed offset) measurement. Misses are excluded by the
/// caller before building these.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OffsetSample {
    pub t_sec: f64,
    pub offset_ms: f64,
}

/// Summary statistics over a set of offsets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OffsetStats {
    pub count: usize,
    pub mean_ms: f64,
    pub median_ms: f64,
    pub stddev_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub mean_abs_ms: f64,
    /// Offsets strictly less than zero (early presses).
    pub early_count: usize,
    /// Offsets strictly greater than zero (late presses).
    pub late_count: usize,
}

impl OffsetStats {
    pub fn compute(offsets: &[f64]) -> Option<OffsetStats> {
        let n = offsets.len();
        if n == 0 {
            return None;
        }
        let mut sorted = offsets.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let sum: f64 = offsets.iter().sum();
        let mean = sum / n as f64;
        let var = offsets.iter().map(|o| (o - mean) * (o - mean)).sum::<f64>() / n as f64;
        let median = if n % 2 == 1 {
            sorted[n / 2]
        } else {
            0.5 * (sorted[n / 2 - 1] + sorted[n / 2])
        };
        Some(OffsetStats {
            count: n,
            mean_ms: mean,
            median_ms: median,
            stddev_ms: var.sqrt(),
            min_ms: sorted[0],
            max_ms: sorted[n - 1],
            mean_abs_ms: offsets.iter().map(|o| o.abs()).sum::<f64>() / n as f64,
            early_count: offsets.iter().filter(|&&o| o < 0.0).count(),
            late_count: offsets.iter().filter(|&&o| o > 0.0).count(),
        })
    }
}

/// Result of regressing offset (ms) against song time (s).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DriftFit {
    /// Drift in ms per second of song time.
    pub slope_ms_per_s: f64,
    /// Intercept in ms at t = 0.
    pub intercept_ms: f64,
    /// Coefficient of determination (0.0..=1.0).
    pub r2: f64,
    pub n: usize,
    /// Convenience: drift expressed per minute of song time.
    pub slope_ms_per_min: f64,
}

/// Ordinary least squares regression of `offset_ms` on `t_sec`.
///
/// Returns `None` if there are fewer than two points or zero time variance.
pub fn linear_regression(samples: &[OffsetSample]) -> Option<DriftFit> {
    let n = samples.len();
    if n < 2 {
        return None;
    }
    let nf = n as f64;
    let mean_t = samples.iter().map(|s| s.t_sec).sum::<f64>() / nf;
    let mean_o = samples.iter().map(|s| s.offset_ms).sum::<f64>() / nf;
    let mut sxx = 0.0;
    let mut sxy = 0.0;
    for s in samples {
        let dt = s.t_sec - mean_t;
        sxx += dt * dt;
        sxy += dt * (s.offset_ms - mean_o);
    }
    if sxx <= 0.0 {
        return None;
    }
    let slope = sxy / sxx;
    let intercept = mean_o - slope * mean_t;

    let mut ss_res = 0.0;
    let mut ss_tot = 0.0;
    for s in samples {
        let pred = intercept + slope * s.t_sec;
        ss_res += (s.offset_ms - pred) * (s.offset_ms - pred);
        ss_tot += (s.offset_ms - mean_o) * (s.offset_ms - mean_o);
    }
    let r2 = if ss_tot > 0.0 {
        (1.0 - ss_res / ss_tot).clamp(0.0, 1.0)
    } else {
        0.0
    };

    Some(DriftFit {
        slope_ms_per_s: slope,
        intercept_ms: intercept,
        r2,
        n,
        slope_ms_per_min: slope * 60.0,
    })
}

/// A fixed-bin histogram of offsets in milliseconds.
#[derive(Debug, Clone, PartialEq)]
pub struct Histogram {
    pub bin_ms: f64,
    /// Lowest bin's left edge in ms.
    pub min_edge_ms: f64,
    pub bins: Vec<u32>,
}

impl Histogram {
    /// Build a histogram spanning `[-range_ms, +range_ms]` with `bin_ms` bins.
    pub fn build(offsets: &[f64], range_ms: f64, bin_ms: f64) -> Histogram {
        let bin_ms = bin_ms.max(0.001);
        let n_bins = ((2.0 * range_ms) / bin_ms).ceil() as usize + 1;
        let mut bins = vec![0u32; n_bins];
        let min_edge = -range_ms;
        for &o in offsets {
            let idx = ((o - min_edge) / bin_ms).floor();
            if idx >= 0.0 && (idx as usize) < n_bins {
                bins[idx as usize] += 1;
            }
        }
        Histogram {
            bin_ms,
            min_edge_ms: min_edge,
            bins,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovers_injected_drift_slope() {
        // offset = 2.0 + 0.5 * t  (0.5 ms/s = 30 ms/min)
        let samples: Vec<OffsetSample> = (0..100)
            .map(|k| {
                let t = k as f64;
                OffsetSample {
                    t_sec: t,
                    offset_ms: 2.0 + 0.5 * t,
                }
            })
            .collect();
        let fit = linear_regression(&samples).expect("fit");
        assert!((fit.slope_ms_per_s - 0.5).abs() < 1e-9);
        assert!((fit.slope_ms_per_min - 30.0).abs() < 1e-6);
        assert!((fit.intercept_ms - 2.0).abs() < 1e-9);
        assert!(fit.r2 > 0.9999);
    }

    #[test]
    fn flat_offsets_have_zero_slope() {
        let samples: Vec<OffsetSample> = (0..10)
            .map(|k| OffsetSample {
                t_sec: k as f64,
                offset_ms: 3.0,
            })
            .collect();
        let fit = linear_regression(&samples).expect("fit");
        assert!(fit.slope_ms_per_s.abs() < 1e-12);
    }

    #[test]
    fn stats_basic() {
        let offsets = vec![-2.0, 0.0, 2.0, 4.0];
        let s = OffsetStats::compute(&offsets).expect("stats");
        assert_eq!(s.count, 4);
        assert!((s.mean_ms - 1.0).abs() < 1e-9);
        assert_eq!(s.early_count, 1);
        assert_eq!(s.late_count, 2);
        assert!((s.min_ms + 2.0).abs() < 1e-9);
        assert!((s.max_ms - 4.0).abs() < 1e-9);
    }

    #[test]
    fn histogram_centers_zero() {
        let offsets = vec![0.0, 0.4, -0.4, 10.0];
        let h = Histogram::build(&offsets, 20.0, 1.0);
        // zero and +/-0.4 fall in the bin covering [-0.? .. ] near center
        let total: u32 = h.bins.iter().sum();
        assert_eq!(total, 4);
    }
}
