//! Leading-arrow detection over a per-lane ROI.
//!
//! For the MVP we assume a fixed noteskin, fixed resolution, constant scroll and
//! no hidden/sudden/reverse mods. Within a lane's ROI we build a 1-D luma
//! profile along the scroll axis, then locate the **leading** arrow — the bright
//! blob nearest the receptor along the travel direction — and report its
//! luma-weighted centroid plus a confidence scalar. The timing layer turns a
//! sequence of centroids into a predicted crossing time.

use crate::frame::FrameView;
use crate::geometry::{Axis, RectPx, ScrollDir};

/// A detected leading-arrow blob within an ROI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Detection {
    /// Luma-weighted centroid coordinate along the scroll axis, in frame pixels.
    pub centroid: f64,
    /// Confidence in 0.0..=1.0 derived from peak contrast above threshold.
    pub confidence: f32,
    /// Peak mean-luma of the leading blob.
    pub peak: f32,
    /// Number of scanlines in the leading blob.
    pub blob_len: u32,
    /// Total number of distinct above-threshold blobs in the ROI (for gating
    /// against overlapping/adjacent notes).
    pub blob_count: u32,
}

/// Ordered (coord, mean_luma) profile from the receptor-near end outward.
///
/// Index 0 is the scanline nearest the receptor along the travel direction.
pub fn profile(frame: &FrameView, roi: RectPx, dir: ScrollDir) -> Vec<(i32, f32)> {
    if roi.is_empty() {
        return Vec::new();
    }
    let axis = dir.axis();
    let coords: Vec<i32> = match dir {
        // near-receptor end first, scanning outward (away from receptor)
        ScrollDir::Up => (roi.y..roi.bottom()).collect(),
        ScrollDir::Down => (roi.y..roi.bottom()).rev().collect(),
        ScrollDir::Left => (roi.x..roi.right()).collect(),
        ScrollDir::Right => (roi.x..roi.right()).rev().collect(),
    };
    coords
        .into_iter()
        .map(|c| (c, frame.line_mean_luma(roi, axis, c)))
        .collect()
}

/// Detect the leading arrow in `roi`. Returns `None` when nothing crosses
/// `threshold` (0.0..=255.0 mean luma).
pub fn detect_leading(
    frame: &FrameView,
    roi: RectPx,
    dir: ScrollDir,
    threshold: f32,
) -> Option<Detection> {
    let prof = profile(frame, roi, dir);
    detect_in_profile(&prof, threshold)
}

/// Core detection over a precomputed near→far profile. Separated for testing.
pub fn detect_in_profile(prof: &[(i32, f32)], threshold: f32) -> Option<Detection> {
    if prof.is_empty() {
        return None;
    }

    // Count all above-threshold runs (blobs) for the gate, and capture the first
    // (nearest-receptor) run as the leading blob.
    let mut blob_count = 0u32;
    let mut in_blob = false;
    let mut leading: Option<(usize, usize)> = None; // [start, end) indices
    for (i, &(_, v)) in prof.iter().enumerate() {
        if v >= threshold {
            if !in_blob {
                in_blob = true;
                blob_count += 1;
                if leading.is_none() {
                    leading = Some((i, i + 1));
                }
            } else if let Some(l) = leading.as_mut()
                && l.1 == i
            {
                l.1 = i + 1;
            }
        } else {
            in_blob = false;
        }
    }

    let (start, end) = leading?;
    let run = &prof[start..end];

    // Luma-weighted centroid in pixel coordinates, with the threshold subtracted
    // so the background floor does not bias the centroid.
    let mut wsum = 0.0f64;
    let mut csum = 0.0f64;
    let mut peak = 0.0f32;
    for &(c, v) in run {
        let w = (v - threshold).max(0.0) as f64;
        wsum += w;
        csum += w * c as f64;
        if v > peak {
            peak = v;
        }
    }
    if wsum <= 0.0 {
        return None;
    }
    let centroid = csum / wsum;
    let confidence = ((peak - threshold) / (255.0 - threshold)).clamp(0.0, 1.0);

    Some(Detection {
        centroid,
        confidence,
        peak,
        blob_len: (end - start) as u32,
        blob_count,
    })
}

/// Signed distance from `centroid` to the receptor along the travel direction.
///
/// Positive while the arrow is approaching (has not yet reached the receptor),
/// crossing through zero at the moment of alignment.
#[inline]
pub fn distance_to_receptor(centroid: f64, receptor_coord: f64, dir: ScrollDir) -> f64 {
    // travel_sign is the sign of coord change as the arrow approaches; the
    // approaching arrow is on the +travel_sign side, so distance is the signed
    // gap that shrinks to zero.
    (centroid - receptor_coord) * -dir.travel_sign()
}

/// Axis helper re-exported for callers building ROIs.
pub fn axis_of(dir: ScrollDir) -> Axis {
    dir.axis()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_single_blob_centroid() {
        // near→far profile with one blob over indices coords 10,11,12
        let prof = vec![
            (8, 0.0),
            (9, 10.0),
            (10, 200.0),
            (11, 240.0),
            (12, 200.0),
            (13, 5.0),
        ];
        let d = detect_in_profile(&prof, 100.0).expect("detect");
        assert_eq!(d.blob_count, 1);
        assert_eq!(d.blob_len, 3);
        // weighted centroid pulled toward the 240 peak at 11
        assert!((d.centroid - 11.0).abs() < 0.2, "centroid={}", d.centroid);
        assert!(d.confidence > 0.5);
    }

    #[test]
    fn picks_nearest_receptor_blob_and_counts_all() {
        // two blobs; near end (index 0) first → leading at coords 1..3
        let prof = vec![
            (0, 0.0),
            (1, 200.0),
            (2, 200.0),
            (3, 0.0),
            (4, 0.0),
            (5, 220.0),
            (6, 220.0),
        ];
        let d = detect_in_profile(&prof, 100.0).expect("detect");
        assert_eq!(d.blob_count, 2);
        assert!((d.centroid - 1.5).abs() < 0.01);
    }

    #[test]
    fn none_when_below_threshold() {
        let prof = vec![(0, 10.0), (1, 20.0), (2, 5.0)];
        assert!(detect_in_profile(&prof, 100.0).is_none());
    }

    #[test]
    fn distance_sign_for_up_scroll() {
        // up-scroll: receptor above (smaller y); approaching arrow has larger y.
        let d = distance_to_receptor(300.0, 250.0, ScrollDir::Up);
        assert!(d > 0.0);
        // once centroid reaches receptor, distance ~ 0
        let d0 = distance_to_receptor(250.0, 250.0, ScrollDir::Up);
        assert!(d0.abs() < 1e-9);
    }
}
