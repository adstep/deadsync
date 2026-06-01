//! Per-lane calibration: where each receptor is, the ROI to watch, the key to
//! press, and detection parameters. Stored as resolution-relative TOML so it
//! survives window resizes; resolved to pixel rectangles at runtime against the
//! current frame size.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::geometry::{PointFrac, RectFrac, RectPx, ScrollDir};

/// Full calibration document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Calibration {
    /// Stable identifier recorded into the self-log.
    pub id: String,
    /// Window title to capture.
    #[serde(default = "default_window_title")]
    pub window_title: String,
    /// Reference resolution the fractions were authored at (informational).
    pub reference_width: u32,
    pub reference_height: u32,
    /// Scroll direction of the notefield.
    pub scroll: ScrollDir,
    /// Detection parameters shared by all lanes.
    pub detect: DetectParams,
    /// One entry per playable lane.
    pub lanes: Vec<LaneCalibration>,
}

fn default_window_title() -> String {
    "DeadSync".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectParams {
    /// Mean-luma threshold (0..=255) above which a scanline counts as arrow.
    pub luma_threshold: f32,
    /// Minimum detector confidence (0..=1) required to feed the timing fit.
    pub min_confidence: f32,
    /// Minimum R² required to accept a crossing prediction.
    pub min_fit_r2: f64,
    /// Number of recent centroid samples retained per lane for the fit.
    pub fit_window: usize,
    /// Latency lead `L` in ms subtracted from the predicted crossing.
    pub lead_ms: f64,
    /// Keyup delay in ms after keydown.
    pub keyup_ms: f64,
    /// Skip a press if its emit moment is already this many ms in the past.
    pub max_late_ms: f64,
}

impl Default for DetectParams {
    fn default() -> Self {
        DetectParams {
            luma_threshold: 120.0,
            min_confidence: 0.35,
            min_fit_r2: 0.9,
            fit_window: 6,
            lead_ms: 0.0,
            keyup_ms: 16.0,
            max_late_ms: 8.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LaneCalibration {
    /// Arrow Cloud-style direction code (1=L,2=D,3=U,4=R) for joining to exports.
    pub direction_code: u8,
    /// Virtual-key name to inject (e.g. "Left", "Down", "Up", "Right").
    pub key: String,
    /// Receptor center as a fraction of the client area.
    pub receptor: PointFrac,
    /// ROI strip (fractions) covering the approach to the receptor.
    pub roi: RectFrac,
}

impl LaneCalibration {
    /// Resolve the ROI rectangle in pixels for a given frame size.
    pub fn roi_px(&self, width: u32, height: u32) -> RectPx {
        self.roi.to_px(width, height)
    }

    /// Receptor coordinate along the scroll axis, in pixels.
    pub fn receptor_coord(&self, width: u32, height: u32, scroll: ScrollDir) -> f64 {
        let p = self.receptor.to_px(width, height);
        match scroll.axis() {
            crate::geometry::Axis::Vertical => p.y as f64,
            crate::geometry::Axis::Horizontal => p.x as f64,
        }
    }
}

impl Calibration {
    pub fn load(path: &Path) -> std::io::Result<Calibration> {
        let text = std::fs::read_to_string(path)?;
        toml::from_str(&text)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, text)
    }

    /// A starting template for a 4-panel, up-scroll, single chart with receptors
    /// near the top of the field. Values are rough placeholders to be refined in
    /// the interactive `calibrate`/`validate` modes against a real frame.
    pub fn default_dance_single_up() -> Calibration {
        // Four lanes spread across the left half (P1), receptors high on screen.
        let receptor_y = 0.12_f32;
        let roi_h = 0.18_f32; // strip below the receptor (arrows approach from below)
        let lane_xs = [0.18_f32, 0.27, 0.36, 0.45];
        let roi_w = 0.07_f32;
        let dir_codes = [1u8, 2, 3, 4];
        let keys = ["Left", "Down", "Up", "Right"];
        let lanes = (0..4)
            .map(|i| LaneCalibration {
                direction_code: dir_codes[i],
                key: keys[i].to_string(),
                receptor: PointFrac {
                    x: lane_xs[i],
                    y: receptor_y,
                },
                roi: RectFrac {
                    x: lane_xs[i] - roi_w / 2.0,
                    y: receptor_y,
                    w: roi_w,
                    h: roi_h,
                },
            })
            .collect();
        Calibration {
            id: "dance-single-up-default".to_string(),
            window_title: default_window_title(),
            reference_width: 1920,
            reference_height: 1080,
            scroll: ScrollDir::Up,
            detect: DetectParams::default(),
            lanes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_template_round_trips_through_toml() {
        let cal = Calibration::default_dance_single_up();
        let text = toml::to_string_pretty(&cal).expect("serialize");
        let back: Calibration = toml::from_str(&text).expect("deserialize");
        assert_eq!(cal, back);
        assert_eq!(back.lanes.len(), 4);
        assert_eq!(back.scroll, ScrollDir::Up);
    }

    #[test]
    fn resolves_roi_and_receptor() {
        let cal = Calibration::default_dance_single_up();
        let lane = &cal.lanes[0];
        let roi = lane.roi_px(1920, 1080);
        assert!(roi.w > 0 && roi.h > 0);
        let rc = lane.receptor_coord(1920, 1080, ScrollDir::Up);
        // up-scroll axis is vertical → receptor coord is a y in pixels
        assert!((rc - (0.12 * 1080.0)).abs() < 2.0);
    }
}
