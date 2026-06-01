//! Geometry primitives shared by calibration and detection.
//!
//! Calibration is stored **resolution-relative** (fractions of the client area)
//! so it survives window resizes, and resolved to integer pixel rectangles
//! against the current frame size at runtime.

use serde::{Deserialize, Serialize};

/// Scroll axis of the notefield.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Axis {
    /// Arrows travel vertically (the common case).
    Vertical,
    /// Arrows travel horizontally.
    Horizontal,
}

/// Travel direction of arrows toward the receptor, in screen-pixel terms.
///
/// Screen `y` increases downward, `x` increases rightward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollDir {
    /// Up-scroll: arrows move toward smaller `y` (receptor above the field).
    Up,
    /// Down-scroll: arrows move toward larger `y`.
    Down,
    /// Arrows move toward smaller `x`.
    Left,
    /// Arrows move toward larger `x`.
    Right,
}

impl ScrollDir {
    pub fn axis(self) -> Axis {
        match self {
            ScrollDir::Up | ScrollDir::Down => Axis::Vertical,
            ScrollDir::Left | ScrollDir::Right => Axis::Horizontal,
        }
    }

    /// Sign of the position coordinate change as an arrow travels toward the
    /// receptor: `-1.0` for Up/Left (decreasing coord), `+1.0` for Down/Right.
    pub fn travel_sign(self) -> f64 {
        match self {
            ScrollDir::Up | ScrollDir::Left => -1.0,
            ScrollDir::Down | ScrollDir::Right => 1.0,
        }
    }
}

/// A point expressed as fractions of the client area (0.0..=1.0).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PointFrac {
    pub x: f32,
    pub y: f32,
}

/// A rectangle expressed as fractions of the client area (0.0..=1.0).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RectFrac {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// An integer pixel point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointPx {
    pub x: i32,
    pub y: i32,
}

/// An integer pixel rectangle, clamped to the frame bounds when resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RectPx {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl PointFrac {
    pub fn to_px(self, width: u32, height: u32) -> PointPx {
        PointPx {
            x: (self.x * width as f32).round() as i32,
            y: (self.y * height as f32).round() as i32,
        }
    }
}

impl RectFrac {
    /// Resolve to a pixel rectangle clamped to `[0, width) x [0, height)`.
    pub fn to_px(self, width: u32, height: u32) -> RectPx {
        let w = width as f32;
        let h = height as f32;
        let x0 = (self.x * w).round().clamp(0.0, w) as i32;
        let y0 = (self.y * h).round().clamp(0.0, h) as i32;
        let x1 = ((self.x + self.w) * w).round().clamp(0.0, w) as i32;
        let y1 = ((self.y + self.h) * h).round().clamp(0.0, h) as i32;
        RectPx {
            x: x0,
            y: y0,
            w: (x1 - x0).max(0),
            h: (y1 - y0).max(0),
        }
    }
}

impl RectPx {
    pub fn right(self) -> i32 {
        self.x + self.w
    }
    pub fn bottom(self) -> i32 {
        self.y + self.h
    }
    pub fn is_empty(self) -> bool {
        self.w <= 0 || self.h <= 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn travel_sign_matches_direction() {
        assert_eq!(ScrollDir::Up.travel_sign(), -1.0);
        assert_eq!(ScrollDir::Down.travel_sign(), 1.0);
        assert_eq!(ScrollDir::Left.travel_sign(), -1.0);
        assert_eq!(ScrollDir::Right.travel_sign(), 1.0);
        assert_eq!(ScrollDir::Up.axis(), Axis::Vertical);
        assert_eq!(ScrollDir::Left.axis(), Axis::Horizontal);
    }

    #[test]
    fn rect_resolves_and_clamps() {
        let r = RectFrac {
            x: 0.5,
            y: 0.5,
            w: 0.25,
            h: 0.25,
        };
        let px = r.to_px(800, 600);
        assert_eq!(px.x, 400);
        assert_eq!(px.y, 300);
        assert_eq!(px.w, 200);
        assert_eq!(px.h, 150);

        let over = RectFrac {
            x: 0.9,
            y: 0.9,
            w: 0.5,
            h: 0.5,
        };
        let pxo = over.to_px(800, 600);
        assert_eq!(pxo.right(), 800);
        assert_eq!(pxo.bottom(), 600);
    }
}
