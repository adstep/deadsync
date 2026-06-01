//! A borrowed view over a captured frame's pixel buffer.
//!
//! Frames captured via Windows.Graphics.Capture are `B8G8R8A8` (byte order
//! **B, G, R, A**). The view carries a `stride` (bytes per row) because GPU
//! staging textures are commonly padded wider than `width * 4`.

use crate::geometry::{Axis, RectPx};

/// Pixel format of a [`FrameView`]. WGC delivers BGRA8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// 8 bits per channel, byte order B, G, R, A.
    Bgra8,
    /// 8 bits per channel, byte order R, G, B, A. Used by synthetic test frames.
    Rgba8,
}

/// A non-owning view over a single frame.
#[derive(Debug, Clone, Copy)]
pub struct FrameView<'a> {
    pub data: &'a [u8],
    pub width: u32,
    pub height: u32,
    /// Bytes per row (>= width * 4).
    pub stride: usize,
    pub format: PixelFormat,
    /// Capture timestamp in QPC ticks (`SystemRelativeTime` for WGC frames).
    pub timestamp_qpc: i64,
}

impl<'a> FrameView<'a> {
    pub fn new(
        data: &'a [u8],
        width: u32,
        height: u32,
        stride: usize,
        format: PixelFormat,
        timestamp_qpc: i64,
    ) -> Self {
        FrameView {
            data,
            width,
            height,
            stride,
            format,
            timestamp_qpc,
        }
    }

    /// Luma (perceptual gray, 0.0..=255.0) of the pixel at `(x, y)`.
    ///
    /// Returns `0.0` for out-of-bounds coordinates. Uses Rec. 601 weights.
    #[inline]
    pub fn luma(&self, x: i32, y: i32) -> f32 {
        if x < 0 || y < 0 || x as u32 >= self.width || y as u32 >= self.height {
            return 0.0;
        }
        let off = y as usize * self.stride + x as usize * 4;
        if off + 3 >= self.data.len() {
            return 0.0;
        }
        let (r, g, b) = match self.format {
            PixelFormat::Bgra8 => (
                self.data[off + 2] as f32,
                self.data[off + 1] as f32,
                self.data[off] as f32,
            ),
            PixelFormat::Rgba8 => (
                self.data[off] as f32,
                self.data[off + 1] as f32,
                self.data[off + 2] as f32,
            ),
        };
        0.299 * r + 0.587 * g + 0.114 * b
    }

    /// Mean luma of one scanline of `roi` perpendicular to the scroll `axis`.
    ///
    /// For a vertical axis this is the mean luma of row `coord` across the ROI's
    /// columns; for horizontal it is the mean of column `coord` across the rows.
    pub fn line_mean_luma(&self, roi: RectPx, axis: Axis, coord: i32) -> f32 {
        match axis {
            Axis::Vertical => {
                if roi.w <= 0 {
                    return 0.0;
                }
                let mut sum = 0.0;
                for x in roi.x..roi.right() {
                    sum += self.luma(x, coord);
                }
                sum / roi.w as f32
            }
            Axis::Horizontal => {
                if roi.h <= 0 {
                    return 0.0;
                }
                let mut sum = 0.0;
                for y in roi.y..roi.bottom() {
                    sum += self.luma(coord, y);
                }
                sum / roi.h as f32
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid_rgba(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
        let mut v = vec![0u8; (width * height * 4) as usize];
        for px in v.chunks_exact_mut(4) {
            px.copy_from_slice(&rgba);
        }
        v
    }

    #[test]
    fn luma_white_is_255() {
        let data = solid_rgba(4, 4, [255, 255, 255, 255]);
        let f = FrameView::new(&data, 4, 4, 16, PixelFormat::Rgba8, 0);
        assert!((f.luma(1, 1) - 255.0).abs() < 0.01);
        // out of bounds
        assert_eq!(f.luma(-1, 0), 0.0);
        assert_eq!(f.luma(4, 0), 0.0);
    }

    #[test]
    fn bgra_channel_order() {
        // byte order B,G,R,A: pure red pixel = [0,0,255,255]
        let data = vec![0u8, 0, 255, 255];
        let f = FrameView::new(&data, 1, 1, 4, PixelFormat::Bgra8, 0);
        assert!((f.luma(0, 0) - 0.299 * 255.0).abs() < 0.5);
    }
}
