//! Reusable, pure marquee/ping-pong scrolling math for long text fields.
//!
//! UI text fields that must fit a fixed-width window (e.g. the SelectMusic
//! "step artist box") can scroll overflowing text inside a clipped window
//! (ping-pong: pause at the start, scroll to reveal the end, pause, scroll
//! back) while leaving text that already fits untouched. This module provides
//! the pure math that drives that behavior.
//!
//! ## Units
//! All widths are **rendered/screen-space** (already multiplied by the text
//! zoom). Mixing pre-zoom `maxwidth` values with post-zoom geometry is a common
//! source of bugs, so callers must pass `box_w` and the measured widths in the
//! same post-zoom space (see [`rendered_width`]).
//!
//! ## Clip coordinate space (for the renderer wiring, not this module)
//! The renderer's text clip rect is expressed in the actor's **parent** space and
//! is applied independently of the text's animated offset. To make the visible
//! window track the text correctly regardless of the parent origin, anchor the
//! clip rect to the text's *un-scrolled* x and animate the offset separately.

#[inline]
#[must_use]
pub fn quantize_up_even(v: i32) -> i32 {
    if v <= 0 {
        0
    } else if (v & 1) != 0 {
        v + 1
    } else {
        v
    }
}

/// Convert an integer logical line width (as returned by
/// `font::measure_line_width_logical`) into a rendered/screen-space width,
/// matching the renderer's quantize-then-scale order.
#[inline]
#[must_use]
pub fn rendered_width(logical_w: i32, zoom: f32) -> f32 {
    quantize_up_even(logical_w) as f32 * zoom
}

/// Configuration for a ping-pong scrolling text field. All distances are in
/// rendered/screen-space pixels; all durations are in seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollConfig {
    /// Rendered width of the visible window (e.g. `maxwidth * zoom`).
    pub box_w: f32,
    /// Scroll speed in rendered px/sec for the moving portion of a ping-pong.
    pub speed_px_s: f32,
    /// Dwell at each end of a ping-pong pass (start and the revealed end).
    pub pause_s: f32,
    /// Dwell for a value that already fits the window (the rotation period).
    pub fit_dwell_s: f32,
    /// Hysteresis: only scroll if `rendered_w > box_w + overflow_tol`.
    pub overflow_tol: f32,
}

/// The per-frame draw state for the active value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollState {
    /// Horizontal offset (rendered px) to add to the text origin. `0.0` means the
    /// start of the text is at the window's left edge; negative reveals later
    /// characters. Always within `[-(rendered_w - box_w), 0]`.
    pub offset_x: f32,
    /// Whether the value overflows and is therefore being clipped + scrolled. When
    /// `false`, the caller should render exactly as before (e.g. keep `maxwidth`).
    pub clipped: bool,
}

#[inline]
fn sanitize_elapsed(elapsed: f32) -> f32 {
    if elapsed.is_finite() {
        elapsed.max(0.0)
    } else {
        0.0
    }
}

/// Whether a value of `rendered_w` overflows the window and should scroll.
#[inline]
#[must_use]
pub fn overflows(rendered_w: f32, cfg: &ScrollConfig) -> bool {
    cfg.box_w > 0.0
        && cfg.speed_px_s > 0.0
        && cfg.overflow_tol.is_finite()
        && rendered_w.is_finite()
        && rendered_w > cfg.box_w + cfg.overflow_tol
}

/// Total time the active value is held: `fit_dwell_s` if it fits, else a full
/// ping-pong (`pause + travel + pause + travel`). Always finite and > 0.
#[inline]
fn value_dwell(rendered_w: f32, cfg: &ScrollConfig) -> f32 {
    if overflows(rendered_w, cfg) {
        let travel = ((rendered_w - cfg.box_w) / cfg.speed_px_s).max(0.0);
        let pause = cfg.pause_s.max(0.0);
        let dwell = pause + travel + pause + travel;
        if dwell.is_finite() && dwell > 0.0 {
            dwell
        } else {
            cfg.fit_dwell_s.max(0.0)
        }
    } else {
        cfg.fit_dwell_s.max(0.0)
    }
}

/// Pick which value is active and how far into its dwell we are.
///
/// Returns `Some((active_idx, local_phase_s))` where `local_phase_s` is the time
/// elapsed within the active value's dwell window (feed it to [`scroll_state`]).
/// Returns `None` only when `widths` is empty.
#[must_use]
pub fn select_active(widths: &[f32], cfg: &ScrollConfig, elapsed: f32) -> Option<(usize, f32)> {
    let n = widths.len();
    if n == 0 {
        return None;
    }
    let t = sanitize_elapsed(elapsed);

    let total: f32 = widths.iter().map(|&w| value_dwell(w, cfg)).sum();
    if !total.is_finite() || total <= 0.0 {
        return Some((0, 0.0));
    }

    let mut local = t % total;
    for (i, &w) in widths.iter().enumerate() {
        let d = value_dwell(w, cfg);
        if local < d {
            return Some((i, local));
        }
        local -= d;
    }
    // Floating-point guard: land on the last value at the end of its dwell.
    Some((n - 1, value_dwell(widths[n - 1], cfg)))
}

/// Compute the draw state for the active value given its rendered width and how
/// far into its dwell window we are (`local_phase` from [`select_active`]).
///
/// Fitting values return `{ offset_x: 0.0, clipped: false }` (render unchanged).
/// Overflowing values ping-pong: hold at the start, scroll left to reveal the
/// end, hold, then scroll back. `offset_x` always stays within
/// `[-(rendered_w - box_w), 0]`.
#[must_use]
pub fn scroll_state(rendered_w: f32, cfg: &ScrollConfig, local_phase: f32) -> ScrollState {
    if !overflows(rendered_w, cfg) {
        return ScrollState {
            offset_x: 0.0,
            clipped: false,
        };
    }

    let span = rendered_w - cfg.box_w; // > overflow_tol > 0
    let travel = (span / cfg.speed_px_s).max(0.0);
    let pause = cfg.pause_s.max(0.0);
    let p = sanitize_elapsed(local_phase);

    let t0 = pause; // hold at start
    let t1 = t0 + travel; // scroll out: 0 -> -span
    let t2 = t1 + pause; // hold at end
    let t3 = t2 + travel; // scroll back: -span -> 0

    let offset = if p <= t0 {
        0.0
    } else if p <= t1 {
        let f = if travel > 0.0 { (p - t0) / travel } else { 1.0 };
        -span * f
    } else if p <= t2 {
        -span
    } else if p <= t3 {
        let f = if travel > 0.0 { (p - t2) / travel } else { 1.0 };
        -span * (1.0 - f)
    } else {
        0.0
    };

    // Clamp against floating-point drift so we never overscroll the window.
    let offset = offset.clamp(-span, 0.0);
    ScrollState {
        offset_x: offset,
        clipped: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> ScrollConfig {
        ScrollConfig {
            box_w: 100.0,
            speed_px_s: 50.0,
            pause_s: 0.5,
            fit_dwell_s: 2.0,
            overflow_tol: 1.0,
        }
    }

    #[test]
    fn quantize_up_even_matches_renderer() {
        assert_eq!(quantize_up_even(-5), 0);
        assert_eq!(quantize_up_even(0), 0);
        assert_eq!(quantize_up_even(1), 2);
        assert_eq!(quantize_up_even(2), 2);
        assert_eq!(quantize_up_even(3), 4);
        assert_eq!(quantize_up_even(99), 100);
        assert_eq!(quantize_up_even(100), 100);
    }

    #[test]
    fn rendered_width_quantizes_then_scales() {
        // 99 -> 100 (quantized) * 0.8 = 80.0
        assert!((rendered_width(99, 0.8) - 80.0).abs() < 1e-6);
        // 100 -> 100 * 0.8 = 80.0
        assert!((rendered_width(100, 0.8) - 80.0).abs() < 1e-6);
    }

    #[test]
    fn overflow_uses_hysteresis() {
        let c = cfg(); // box_w 100, tol 1
        assert!(!overflows(100.0, &c));
        assert!(!overflows(101.0, &c)); // exactly box_w + tol does not scroll
        assert!(overflows(101.5, &c));
        // Degenerate configs never scroll.
        let bad = ScrollConfig {
            box_w: 0.0,
            ..c
        };
        assert!(!overflows(1000.0, &bad));
        let bad = ScrollConfig {
            speed_px_s: 0.0,
            ..c
        };
        assert!(!overflows(1000.0, &bad));
        assert!(!overflows(f32::NAN, &c));
        assert!(!overflows(f32::INFINITY, &c));
    }

    #[test]
    fn select_active_empty_is_none() {
        assert_eq!(select_active(&[], &cfg(), 1.0), None);
    }

    #[test]
    fn select_active_all_fit_matches_classic_rotation() {
        // All widths fit -> each dwell is fit_dwell_s (2.0), so the index must
        // equal the classic floor(t / 2) % n rotation, byte-for-byte.
        let c = cfg();
        let widths = [50.0, 60.0, 70.0];
        let n = widths.len();
        for k in 0..240 {
            let t = k as f32 * 0.05; // 0..12s
            let (idx, phase) = select_active(&widths, &c, t).unwrap();
            let expected = ((t / c.fit_dwell_s).floor().max(0.0) as usize) % n;
            assert_eq!(idx, expected, "t={t}");
            // Fitting values never scroll.
            let ss = scroll_state(widths[idx], &c, phase);
            assert_eq!(ss, ScrollState { offset_x: 0.0, clipped: false });
        }
    }

    #[test]
    fn select_active_single_value_holds_index_zero() {
        let c = cfg();
        // Overflowing single value: always index 0, phase advances and wraps.
        let widths = [200.0];
        let (i0, _) = select_active(&widths, &c, 0.0).unwrap();
        let (i1, _) = select_active(&widths, &c, 5.0).unwrap();
        assert_eq!((i0, i1), (0, 0));
    }

    #[test]
    fn overflowing_value_pauses_rotation_for_full_pingpong() {
        let c = cfg();
        // Middle value overflows: span = 200-100 = 100, travel = 100/50 = 2s.
        // dwell = 0.5 + 2 + 0.5 + 2 = 5s. Fitting neighbors dwell 2s each.
        let widths = [50.0, 200.0, 60.0];
        let d_over = 5.0_f32;

        // Timeline: [0,2) idx0, [2,7) idx1 (the long scroll), [7,9) idx2.
        assert_eq!(select_active(&widths, &c, 0.5).unwrap().0, 0);
        for &t in &[2.0_f32, 3.5, 5.0, 6.9] {
            assert_eq!(select_active(&widths, &c, t).unwrap().0, 1, "t={t}");
        }
        assert_eq!(select_active(&widths, &c, 7.5).unwrap().0, 2);

        // Local phase within the overflowing value is t - 2.0.
        let (idx, phase) = select_active(&widths, &c, 4.0).unwrap();
        assert_eq!(idx, 1);
        assert!((phase - 2.0).abs() < 1e-5);
        let _ = d_over;
    }

    #[test]
    fn scroll_state_pingpong_endpoints_and_midpoints() {
        let c = cfg();
        let w = 200.0; // span 100, travel 2s, pause 0.5s
        let span = 100.0_f32;

        // Hold at start.
        assert_eq!(scroll_state(w, &c, 0.0).offset_x, 0.0);
        assert_eq!(scroll_state(w, &c, 0.5).offset_x, 0.0);
        // Mid scroll-out (phase 1.5 => 1.0s into travel => halfway).
        let mid = scroll_state(w, &c, 1.5);
        assert!(mid.clipped);
        assert!((mid.offset_x - (-span * 0.5)).abs() < 1e-4, "{}", mid.offset_x);
        // Fully revealed end (phase 2.5 = pause+travel).
        assert!((scroll_state(w, &c, 2.5).offset_x - (-span)).abs() < 1e-4);
        // Hold at end.
        assert!((scroll_state(w, &c, 3.0).offset_x - (-span)).abs() < 1e-4);
        // Mid scroll-back (phase 4.0 => 1.0s into return => halfway back).
        assert!((scroll_state(w, &c, 4.0).offset_x - (-span * 0.5)).abs() < 1e-4);
        // Back to start (phase 5.0 = full dwell: pause+travel+pause+travel).
        assert!((scroll_state(w, &c, 5.0).offset_x - 0.0).abs() < 1e-4);
    }

    #[test]
    fn scroll_state_offset_stays_in_bounds() {
        let c = cfg();
        let w = 200.0;
        let span = 100.0_f32;
        for k in 0..200 {
            let p = k as f32 * 0.05; // 0..10s, beyond one dwell
            let off = scroll_state(w, &c, p).offset_x;
            assert!(off <= 0.0 + 1e-6 && off >= -span - 1e-6, "p={p} off={off}");
        }
    }

    #[test]
    fn scroll_state_fitting_value_is_unclipped() {
        let c = cfg();
        let ss = scroll_state(80.0, &c, 1.234);
        assert_eq!(ss, ScrollState { offset_x: 0.0, clipped: false });
    }
}
