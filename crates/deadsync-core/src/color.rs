//! The canonical color type shared across the whole project.
//!
//! [`Color`] is a linear RGBA value stored as four `f32` channels in the range
//! `0.0..=1.0`. It is `#[repr(transparent)]` over `[f32; 4]` and converts to and
//! from that array for free, so existing array-based call sites keep working
//! while new code can take and pass a `Color` directly.
//!
//! Two hex conventions live side by side because the project mixes them:
//!
//! * [`Color::hex`] — a `const`, panicking parser for trusted literals using the
//!   web/renderer convention (`#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`).
//! * [`Color::from_argb_hex`] / [`Color::to_argb_hex`] — a fallible parser and
//!   formatter using the StepMania ARGB convention (`RRGGBB` opaque or
//!   `AARRGGBB`), used by config options that round-trip colors to disk.

/// A linear RGBA color, stored as four `f32` channels in the range `0.0..=1.0`.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color(pub [f32; 4]);

impl Color {
    /// Fully transparent black.
    pub const TRANSPARENT: Self = Self([0.0, 0.0, 0.0, 0.0]);
    /// Opaque black.
    pub const BLACK: Self = Self([0.0, 0.0, 0.0, 1.0]);
    /// Opaque white.
    pub const WHITE: Self = Self([1.0, 1.0, 1.0, 1.0]);

    /// Build a color from explicit channels.
    #[inline(always)]
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self([r, g, b, a])
    }

    /// Build an opaque color (alpha = 1.0) from RGB channels.
    #[inline(always)]
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self([r, g, b, 1.0])
    }

    /// Parse a hex color string at compile time using the web/renderer
    /// convention. Accepts `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa` (with or
    /// without the leading `#`). Panics on invalid input; use only with trusted
    /// literals.
    #[inline(always)]
    pub const fn hex(s: &str) -> Self {
        Self(rgba_hex(s))
    }

    /// Wrap a raw `[r, g, b, a]` array.
    #[inline(always)]
    pub const fn from_array(rgba: [f32; 4]) -> Self {
        Self(rgba)
    }

    /// Channels as an `[r, g, b, a]` array for the renderer's tint/diffuse.
    #[inline(always)]
    pub const fn to_array(self) -> [f32; 4] {
        self.0
    }

    /// Channels as an `[r, g, b, a]` array. Alias for [`Color::to_array`] kept
    /// for call sites that historically used the `to_rgba` name.
    #[inline(always)]
    pub const fn to_rgba(self) -> [f32; 4] {
        self.0
    }

    #[inline(always)]
    pub const fn r(self) -> f32 {
        self.0[0]
    }

    #[inline(always)]
    pub const fn g(self) -> f32 {
        self.0[1]
    }

    #[inline(always)]
    pub const fn b(self) -> f32 {
        self.0[2]
    }

    #[inline(always)]
    pub const fn a(self) -> f32 {
        self.0[3]
    }

    /// Return a copy with the alpha channel replaced, keeping RGB.
    #[inline(always)]
    pub const fn with_alpha(self, a: f32) -> Self {
        Self([self.0[0], self.0[1], self.0[2], a])
    }

    /// Parse a hex color string in the web RGBA convention (case-insensitive,
    /// trimmed, optional leading `#`). Accepts both 6-digit `RRGGBB` (opaque)
    /// and 8-digit `RRGGBBAA` forms. Returns `None` for malformed input so the
    /// caller can fall back to a default.
    ///
    /// This is the fallible counterpart to the `const`, panicking [`Color::hex`]
    /// and shares its channel order (unlike the ARGB [`Color::from_argb_hex`]).
    pub fn from_rgba_hex(raw: &str) -> Option<Self> {
        let hex = raw.trim().trim_start_matches('#');
        if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let byte = |idx: usize| u8::from_str_radix(&hex[idx..idx + 2], 16).ok();
        let chan = |idx: usize| Some(byte(idx)? as f32 / 255.0);
        match hex.len() {
            6 => Some(Self([chan(0)?, chan(2)?, chan(4)?, 1.0])),
            8 => Some(Self([chan(0)?, chan(2)?, chan(4)?, chan(6)?])),
            _ => None,
        }
    }

    /// Parse a hex color string using the StepMania ARGB convention
    /// (case-insensitive, trimmed, optional leading `#`). Accepts both 6-digit
    /// `RRGGBB` (opaque) and 8-digit `AARRGGBB` forms. Returns `None` for
    /// malformed input so the caller can fall back to a default.
    pub fn from_argb_hex(raw: &str) -> Option<Self> {
        let hex = raw.trim().trim_start_matches('#');
        if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let byte = |idx: usize| u8::from_str_radix(&hex[idx..idx + 2], 16).ok();
        let chan = |idx: usize| Some(byte(idx)? as f32 / 255.0);
        match hex.len() {
            6 => Some(Self([chan(0)?, chan(2)?, chan(4)?, 1.0])),
            8 => Some(Self([chan(2)?, chan(4)?, chan(6)?, chan(0)?])),
            _ => None,
        }
    }

    /// Format as an uppercase hex string using the StepMania ARGB convention:
    /// `#RRGGBB` when fully opaque, otherwise `#AARRGGBB`. Round-trips with
    /// [`Color::from_argb_hex`].
    pub fn to_argb_hex(self) -> String {
        let channel = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        let (r, g, b) = (channel(self.0[0]), channel(self.0[1]), channel(self.0[2]));
        let a = channel(self.0[3]);
        if a == 255 {
            format!("#{r:02X}{g:02X}{b:02X}")
        } else {
            format!("#{a:02X}{r:02X}{g:02X}{b:02X}")
        }
    }
}

impl From<[f32; 4]> for Color {
    #[inline(always)]
    fn from(rgba: [f32; 4]) -> Self {
        Self(rgba)
    }
}

impl From<Color> for [f32; 4] {
    #[inline(always)]
    fn from(c: Color) -> Self {
        c.0
    }
}

/// Parse a hex color string into a raw `[r, g, b, a]` array using the
/// web/renderer convention.
///
/// Accepts `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa` (or without `#`).
/// Panics on invalid input; use only with trusted literals.
/// Evaluated at COMPILE TIME if assigned to a const/static.
pub const fn rgba_hex(s: &str) -> [f32; 4] {
    let bytes = s.as_bytes();

    // Handle optional '#' by offsetting start index
    let (bytes, len) = if !bytes.is_empty() && bytes[0] == b'#' {
        let (_, rem) = bytes.split_at(1);
        (rem, s.len() - 1)
    } else {
        (bytes, s.len())
    };

    // Const-safe hex char to u8
    const fn val(b: u8) -> u8 {
        match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => 10 + (b - b'a'),
            b'A'..=b'F' => 10 + (b - b'A'),
            _ => panic!("invalid hex digit in color string"),
        }
    }

    // Combine two hex digits into a byte
    const fn byte2(h: u8, l: u8) -> u8 {
        (val(h) << 4) | val(l)
    }

    // Expand 4-bit color to 8-bit (e.g. F -> FF)
    const fn rep(n: u8) -> u8 {
        (val(n) << 4) | val(n)
    }

    let (r, g, b, a) = match len {
        3 => (rep(bytes[0]), rep(bytes[1]), rep(bytes[2]), 0xFF),
        4 => (rep(bytes[0]), rep(bytes[1]), rep(bytes[2]), rep(bytes[3])),
        6 => (
            byte2(bytes[0], bytes[1]),
            byte2(bytes[2], bytes[3]),
            byte2(bytes[4], bytes[5]),
            0xFF,
        ),
        8 => (
            byte2(bytes[0], bytes[1]),
            byte2(bytes[2], bytes[3]),
            byte2(bytes[4], bytes[5]),
            byte2(bytes[6], bytes[7]),
        ),
        _ => panic!("color hex string must be 3, 4, 6, or 8 digits"),
    };

    [
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        a as f32 / 255.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_with_array() {
        let arr = [0.1, 0.2, 0.3, 0.4];
        let c = Color::from(arr);
        assert_eq!(c.to_array(), arr);
        assert_eq!(c.to_rgba(), arr);
        assert_eq!(<[f32; 4]>::from(c), arr);
        assert_eq!(c, Color::new(0.1, 0.2, 0.3, 0.4));
    }

    #[test]
    fn hex_parses_web_convention() {
        assert_eq!(Color::hex("#FFFFFF"), Color::WHITE);
        assert_eq!(Color::hex("#000000"), Color::BLACK);
        assert_eq!(Color::rgb(1.0, 1.0, 1.0), Color::WHITE);
        assert_eq!(Color::WHITE.with_alpha(0.0), Color::new(1.0, 1.0, 1.0, 0.0));
    }

    #[test]
    fn argb_hex_accepts_hash_and_bare_forms() {
        assert_eq!(Color::from_argb_hex("#000000"), Some(Color::BLACK));
        assert_eq!(
            Color::from_argb_hex("FFFFFF"),
            Some(Color::rgb(1.0, 1.0, 1.0))
        );
        let gray = Color::from_argb_hex("#0C0C0C").unwrap();
        let expected = 12.0 / 255.0;
        for ch in [gray.r(), gray.g(), gray.b()] {
            assert!((ch - expected).abs() < f32::EPSILON);
        }
        assert_eq!(gray.a(), 1.0);
    }

    #[test]
    fn rgba_hex_parses_web_order_and_alpha() {
        assert_eq!(Color::from_rgba_hex("#000000"), Some(Color::BLACK));
        assert_eq!(
            Color::from_rgba_hex("FFFFFF"),
            Some(Color::rgb(1.0, 1.0, 1.0))
        );
        let c = Color::from_rgba_hex("#01FE7F80").unwrap();
        assert!((c.r() - 1.0 / 255.0).abs() < f32::EPSILON);
        assert!((c.g() - 254.0 / 255.0).abs() < f32::EPSILON);
        assert!((c.b() - 127.0 / 255.0).abs() < f32::EPSILON);
        assert!((c.a() - 128.0 / 255.0).abs() < f32::EPSILON);
        assert_eq!(Color::from_rgba_hex("#FFF"), None);
        assert_eq!(Color::from_rgba_hex("#GGGGGG"), None);
    }

    #[test]
    fn argb_hex_parses_argb_order() {
        let c = Color::from_argb_hex("#8001FE7F").unwrap();
        assert!((c.a() - 128.0 / 255.0).abs() < f32::EPSILON);
        assert!((c.r() - 1.0 / 255.0).abs() < f32::EPSILON);
        assert!((c.g() - 254.0 / 255.0).abs() < f32::EPSILON);
        assert!((c.b() - 127.0 / 255.0).abs() < f32::EPSILON);
    }

    #[test]
    fn argb_hex_is_case_insensitive_and_trims() {
        assert_eq!(
            Color::from_argb_hex("  #0c0c0c  "),
            Color::from_argb_hex("#0C0C0C")
        );
        assert_eq!(
            Color::from_argb_hex("  80ffffff  "),
            Color::from_argb_hex("#80FFFFFF")
        );
    }

    #[test]
    fn argb_hex_rejects_malformed() {
        assert_eq!(Color::from_argb_hex(""), None);
        assert_eq!(Color::from_argb_hex("#FFF"), None);
        assert_eq!(Color::from_argb_hex("#GGGGGG"), None);
        assert_eq!(Color::from_argb_hex("#1234567"), None);
        assert_eq!(Color::from_argb_hex("#123456789"), None);
    }

    #[test]
    fn argb_hex_round_trips() {
        assert_eq!(
            Color::from_argb_hex("#0C0C0C").unwrap().to_argb_hex(),
            "#0C0C0C"
        );
        assert_eq!(
            Color::from_argb_hex("#8001FE7F").unwrap().to_argb_hex(),
            "#8001FE7F"
        );
    }
}
