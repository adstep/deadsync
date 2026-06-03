//! A small ARGB color type used by config options that carry colors (such as
//! the gameplay background color).

/// An ARGB color. Each channel is a linear value in the range `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub a: f32,
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Color {
    /// Opaque black.
    pub const BLACK: Self = Self {
        a: 1.0,
        r: 0.0,
        g: 0.0,
        b: 0.0,
    };

    /// Build an opaque color (alpha = 1.0) from RGB channels.
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { a: 1.0, r, g, b }
    }

    /// Build a color from the renderer's `[r, g, b, a]` array form.
    pub const fn from_rgba(rgba: [f32; 4]) -> Self {
        Self {
            a: rgba[3],
            r: rgba[0],
            g: rgba[1],
            b: rgba[2],
        }
    }

    /// Channels as an `[r, g, b, a]` array for the renderer's tint/diffuse.
    pub fn to_rgba(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// Parse a hex color string (case-insensitive, trimmed, optional leading
    /// `#`). Accepts both 6-digit `RRGGBB` (opaque) and 8-digit `AARRGGBB`
    /// forms. Returns `None` for malformed input so the caller can fall back to
    /// a default.
    pub fn from_hex(raw: &str) -> Option<Self> {
        let hex = raw.trim().trim_start_matches('#');
        if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let byte = |idx: usize| u8::from_str_radix(&hex[idx..idx + 2], 16).ok();
        let chan = |idx: usize| Some(byte(idx)? as f32 / 255.0);
        match hex.len() {
            6 => Some(Self {
                a: 1.0,
                r: chan(0)?,
                g: chan(2)?,
                b: chan(4)?,
            }),
            8 => Some(Self {
                a: chan(0)?,
                r: chan(2)?,
                g: chan(4)?,
                b: chan(6)?,
            }),
            _ => None,
        }
    }

    /// Format as an uppercase hex string: `#RRGGBB` when fully opaque, otherwise
    /// `#AARRGGBB`. Round-trips with [`Color::from_hex`].
    pub fn to_hex(self) -> String {
        let channel = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        let (r, g, b) = (channel(self.r), channel(self.g), channel(self.b));
        let a = channel(self.a);
        if a == 255 {
            format!("#{r:02X}{g:02X}{b:02X}")
        } else {
            format!("#{a:02X}{r:02X}{g:02X}{b:02X}")
        }
    }
}

// The judgement-display domain axes (`JudgmentWindow`, `JudgmentMode`) and the
// slot count live in `crate::judgment` — they are app-level concepts, not color
// concerns. Re-exported here so `config::color::{JudgmentWindow, JudgmentMode}`
// and the public `config::*` paths keep resolving for existing call sites.
pub use crate::judgment::{JudgmentMode, JudgmentWindow, JUDGMENT_WINDOW_COUNT};

/// A user-configurable judgement palette for one scoring mode: one [`Color`] per
/// [`JudgmentWindow`] slot, stored by canonical window index.
///
/// The Hard-EX score accent is intentionally **not** part of this struct — it is
/// not a timing window. It lives once on [`JudgmentPalettes::hard_ex_score`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JudgmentPalette {
    windows: [Color; JUDGMENT_WINDOW_COUNT],
}

impl JudgmentPalette {
    /// Build a palette from an array of per-slot colors (indexed by
    /// [`JudgmentWindow::index`]).
    pub const fn from_windows(windows: [Color; JUDGMENT_WINDOW_COUNT]) -> Self {
        Self { windows }
    }

    /// The color for one window slot.
    pub fn color(&self, window: JudgmentWindow) -> Color {
        self.windows[window.index()]
    }

    /// Set the color for one window slot.
    pub fn set_color(&mut self, window: JudgmentWindow, color: Color) {
        self.windows[window.index()] = color;
    }
}

/// All three independently-customizable per-mode judgement palettes plus the
/// single global Hard-EX score accent color.
///
/// Replaces the former trio of `Config` fields with one mode-indexed container so
/// adding a future mode is one [`JudgmentMode`] variant + data rather than a new
/// `Config` field and a match arm at every call site.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JudgmentPalettes {
    palettes: [JudgmentPalette; 3],
    /// The Arrow Cloud "H.EX" score accent (`#FF00CC` by default). Not a timing
    /// window: it is a score color also reused as the innermost Hard-EX band.
    pub hard_ex_score: Color,
}

impl JudgmentPalettes {
    /// Build from the three per-mode palettes (ITG, FA+, HEX order) and the
    /// global Hard-EX accent.
    pub const fn from_parts(palettes: [JudgmentPalette; 3], hard_ex_score: Color) -> Self {
        Self {
            palettes,
            hard_ex_score,
        }
    }

    /// The palette for `mode` (returned by value; [`JudgmentPalette`] is `Copy`).
    pub fn palette(&self, mode: JudgmentMode) -> JudgmentPalette {
        self.palettes[mode.index()]
    }

    /// A mutable reference to the palette for `mode`.
    pub fn palette_mut(&mut self, mode: JudgmentMode) -> &mut JudgmentPalette {
        &mut self.palettes[mode.index()]
    }
}
