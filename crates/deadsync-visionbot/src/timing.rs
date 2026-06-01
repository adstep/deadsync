//! Sub-frame crossing-time prediction from a short window of centroid samples.
//!
//! As the leading arrow approaches the receptor its centroid moves linearly in
//! time (valid only for **constant scroll** — an MVP constraint). Given a few
//! `(position, qpc)` samples near the receptor we fit a line and solve for the
//! QPC tick at which the centroid reaches the receptor coordinate.
//!
//! We use a **Theil–Sen** estimator (median of pairwise slopes) for robustness
//! against a single bad centroid, and report R² around that line so the caller
//! can gate low-quality fits. With only 3–6 samples per crossing, robustness to
//! one outlier matters more than the efficiency of least squares.

/// One centroid observation: position along the scroll axis at a capture time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub pos: f64,
    pub qpc: i64,
}

/// A successful crossing prediction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Prediction {
    /// Predicted QPC tick at which the centroid reaches the receptor.
    pub crossing_qpc: i64,
    /// Fitted slope in position units per QPC tick.
    pub slope_pos_per_tick: f64,
    /// Coefficient of determination around the fitted line (0.0..=1.0).
    pub r2: f64,
    /// Number of samples used.
    pub n: usize,
    /// Largest absolute residual in position units.
    pub max_residual_pos: f64,
}

/// Acceptance criteria for a prediction.
#[derive(Debug, Clone, Copy)]
pub struct FitGate {
    pub min_samples: usize,
    pub min_r2: f64,
    /// Reject if |residual| exceeds this many position units for any sample.
    pub max_residual_pos: f64,
    /// Expected sign of `slope_pos_per_tick` (the travel sign). `0.0` disables
    /// the sign check.
    pub expected_slope_sign: f64,
    /// Reject if the predicted crossing lies more than this many ticks outside
    /// the sample time span (guards against wild extrapolation).
    pub max_extrapolation_ticks: i64,
}

impl Default for FitGate {
    fn default() -> Self {
        FitGate {
            min_samples: 3,
            min_r2: 0.9,
            max_residual_pos: 12.0,
            expected_slope_sign: 0.0,
            max_extrapolation_ticks: i64::MAX,
        }
    }
}

/// Why a prediction was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateReject {
    TooFewSamples,
    DegenerateTime,
    ZeroSlope,
    WrongSlopeSign,
    LowR2,
    HighResidual,
    Extrapolated,
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = values.len();
    if n == 0 {
        return 0.0;
    }
    if n % 2 == 1 {
        values[n / 2]
    } else {
        0.5 * (values[n / 2 - 1] + values[n / 2])
    }
}

/// Fit a line and predict the crossing tick where `pos == target_pos`.
pub fn predict_crossing(
    samples: &[Sample],
    target_pos: f64,
    gate: &FitGate,
) -> Result<Prediction, GateReject> {
    let n = samples.len();
    if n < gate.min_samples.max(2) {
        return Err(GateReject::TooFewSamples);
    }

    // Work in ticks relative to the first sample to keep magnitudes small.
    let t0 = samples[0].qpc;
    let ts: Vec<f64> = samples.iter().map(|s| (s.qpc - t0) as f64).collect();
    let ps: Vec<f64> = samples.iter().map(|s| s.pos).collect();

    if ts.iter().all(|&t| (t - ts[0]).abs() < f64::EPSILON) {
        return Err(GateReject::DegenerateTime);
    }

    // Theil–Sen slope: median of pairwise slopes.
    let mut slopes = Vec::with_capacity(n * (n - 1) / 2);
    for i in 0..n {
        for j in (i + 1)..n {
            let dt = ts[j] - ts[i];
            if dt.abs() > f64::EPSILON {
                slopes.push((ps[j] - ps[i]) / dt);
            }
        }
    }
    if slopes.is_empty() {
        return Err(GateReject::DegenerateTime);
    }
    let slope = median(&mut slopes);
    if slope.abs() < f64::EPSILON {
        return Err(GateReject::ZeroSlope);
    }
    if gate.expected_slope_sign != 0.0 && slope.signum() != gate.expected_slope_sign.signum() {
        return Err(GateReject::WrongSlopeSign);
    }

    // Theil–Sen intercept: median of (pos_i - slope * t_i).
    let mut intercepts: Vec<f64> = (0..n).map(|i| ps[i] - slope * ts[i]).collect();
    let intercept = median(&mut intercepts);

    // Residuals and R² around the fitted line.
    let mean_p = ps.iter().sum::<f64>() / n as f64;
    let mut ss_res = 0.0;
    let mut ss_tot = 0.0;
    let mut max_res = 0.0f64;
    for i in 0..n {
        let pred = intercept + slope * ts[i];
        let r = ps[i] - pred;
        ss_res += r * r;
        ss_tot += (ps[i] - mean_p) * (ps[i] - mean_p);
        max_res = max_res.max(r.abs());
    }
    let r2 = if ss_tot > 0.0 {
        (1.0 - ss_res / ss_tot).clamp(0.0, 1.0)
    } else {
        0.0
    };
    if r2 < gate.min_r2 {
        return Err(GateReject::LowR2);
    }
    if max_res > gate.max_residual_pos {
        return Err(GateReject::HighResidual);
    }

    // Solve target_pos = intercept + slope * t  ->  t (relative), then to QPC.
    let t_cross = (target_pos - intercept) / slope;
    let crossing_qpc = t0 + t_cross.round() as i64;

    if gate.max_extrapolation_ticks != i64::MAX {
        let tmin = *ts
            .iter()
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(&0.0);
        let tmax = *ts
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(&0.0);
        let m = gate.max_extrapolation_ticks as f64;
        if t_cross < tmin - m || t_cross > tmax + m {
            return Err(GateReject::Extrapolated);
        }
    }

    Ok(Prediction {
        crossing_qpc,
        slope_pos_per_tick: slope,
        r2,
        n,
        max_residual_pos: max_res,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovers_known_crossing() {
        // pos = 1000 - 0.5 * t (ticks). Crosses target 250 at t = 1500.
        let samples: Vec<Sample> = (0..5)
            .map(|k| {
                let t = k as i64 * 200;
                Sample {
                    pos: 1000.0 - 0.5 * t as f64,
                    qpc: 10_000 + t,
                }
            })
            .collect();
        let gate = FitGate {
            expected_slope_sign: -1.0,
            ..Default::default()
        };
        let p = predict_crossing(&samples, 250.0, &gate).expect("predict");
        assert_eq!(p.crossing_qpc, 10_000 + 1500);
        assert!((p.slope_pos_per_tick + 0.5).abs() < 1e-9);
        assert!(p.r2 > 0.999);
    }

    #[test]
    fn robust_to_single_outlier() {
        let mut samples: Vec<Sample> = (0..6)
            .map(|k| {
                let t = k as i64 * 100;
                Sample {
                    pos: 800.0 - 1.0 * t as f64,
                    qpc: t,
                }
            })
            .collect();
        // corrupt one centroid badly
        samples[3].pos += 60.0;
        let gate = FitGate {
            min_r2: 0.5,
            max_residual_pos: 100.0,
            expected_slope_sign: -1.0,
            ..Default::default()
        };
        let p = predict_crossing(&samples, 0.0, &gate).expect("predict");
        // true crossing at t = 800; Theil-Sen should stay close despite outlier
        assert!((p.crossing_qpc - 800).abs() <= 5, "qpc={}", p.crossing_qpc);
    }

    #[test]
    fn rejects_wrong_sign() {
        let samples: Vec<Sample> = (0..4)
            .map(|k| Sample {
                pos: 100.0 + k as f64,
                qpc: k as i64,
            })
            .collect();
        let gate = FitGate {
            expected_slope_sign: -1.0,
            ..Default::default()
        };
        assert_eq!(
            predict_crossing(&samples, 0.0, &gate),
            Err(GateReject::WrongSlopeSign)
        );
    }

    #[test]
    fn rejects_too_few() {
        let samples = vec![Sample { pos: 1.0, qpc: 0 }];
        assert_eq!(
            predict_crossing(&samples, 0.0, &FitGate::default()),
            Err(GateReject::TooFewSamples)
        );
    }
}
