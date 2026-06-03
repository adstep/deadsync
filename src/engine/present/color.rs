//! Color helpers and the engine's canonical default judgement palette.
use crate::config;

/// Accepts "#rgb", "#rgba", "#rrggbb", "#rrggbbaa" (or without '#').
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

#[macro_export]
macro_rules! rgba {
    ($hex:literal $(,)?) => {
        $crate::engine::present::color::rgba_hex($hex)
    };
}

#[macro_export]
macro_rules! rgba_const {
    ($name:ident, $hex:literal $(,)?) => {
        const $name: [f32; 4] = $crate::engine::present::color::rgba_hex($hex);
    };
    ($vis:vis $name:ident, $hex:literal $(,)?) => {
        $vis const $name: [f32; 4] = $crate::engine::present::color::rgba_hex($hex);
    };
}

/* =========================== THEME PALETTES =========================== */

/// Start at #C1006F in the decorative palette.
pub const DEFAULT_COLOR_INDEX: i32 = 2;

pub const FILE_DIFFICULTY_NAMES: [&str; 5] = ["Beginner", "Easy", "Medium", "Hard", "Challenge"];
pub const DISPLAY_DIFFICULTY_NAMES: [&str; 5] = ["Beginner", "Easy", "Medium", "Hard", "Challenge"];
pub const ZMOD_DISPLAY_DIFFICULTY_NAMES: [&str; 5] =
    ["Beginner", "Easy", "Medium", "Hard", "Expert"];

#[inline(always)]
fn contains_ascii_ci(haystack: &str, needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    let hay = haystack.as_bytes();
    if hay.len() < needle.len() {
        return false;
    }
    let limit = hay.len() - needle.len();
    let mut i = 0;
    while i <= limit {
        let mut j = 0;
        while j < needle.len() {
            if !hay[i + j].eq_ignore_ascii_case(&needle[j]) {
                break;
            }
            j += 1;
        }
        if j == needle.len() {
            return true;
        }
        i += 1;
    }
    false
}

#[inline(always)]
pub fn difficulty_display_name(difficulty_name: &str, zmod_rating_box_text: bool) -> &'static str {
    if difficulty_name.eq_ignore_ascii_case("edit") {
        return "Edit";
    }
    let difficulty_index = FILE_DIFFICULTY_NAMES
        .iter()
        .position(|&name| name.eq_ignore_ascii_case(difficulty_name))
        .unwrap_or(2);
    if zmod_rating_box_text {
        ZMOD_DISPLAY_DIFFICULTY_NAMES[difficulty_index]
    } else {
        DISPLAY_DIFFICULTY_NAMES[difficulty_index]
    }
}

#[inline(always)]
pub fn difficulty_display_name_for_song(
    difficulty_name: &str,
    song_main_title: &str,
    zmod_rating_box_text: bool,
) -> &'static str {
    if !zmod_rating_box_text || !difficulty_name.eq_ignore_ascii_case("challenge") {
        return difficulty_display_name(difficulty_name, zmod_rating_box_text);
    }
    if contains_ascii_ci(song_main_title, b"(NOVICE)") {
        return difficulty_display_name("Beginner", true);
    }
    if contains_ascii_ci(song_main_title, b"(EASY)") {
        return difficulty_display_name("Easy", true);
    }
    if contains_ascii_ci(song_main_title, b"(MEDIUM)") {
        return difficulty_display_name("Medium", true);
    }
    if contains_ascii_ci(song_main_title, b"(HARD)") {
        return difficulty_display_name("Hard", true);
    }
    if contains_ascii_ci(song_main_title, b"(EDIT)") {
        return difficulty_display_name("Edit", true);
    }
    difficulty_display_name(difficulty_name, true)
}

/// Decorative / sprite tint palette (hearts, backgrounds, sprites)
pub const DECORATIVE_RGBA: [[f32; 4]; 12] = [
    rgba_hex("#FF3C23"),
    rgba_hex("#FF003C"),
    rgba_hex("#C1006F"),
    rgba_hex("#8200A1"),
    rgba_hex("#413AD0"),
    rgba_hex("#0073FF"),
    rgba_hex("#00ADC0"),
    rgba_hex("#5CE087"),
    rgba_hex("#AEFA44"),
    rgba_hex("#FFFF00"),
    rgba_hex("#FFBE00"),
    rgba_hex("#FF7D00"),
];

/// Simply Love SRPG9 event colors mapped to the normal Select Color hue wheel.
/// The source theme uses `SL.SRPG9.Colors` directly when SRPG9 is active, but
/// DeadSync's Select Color screen is keyed to `DECORATIVE_RGBA`.
pub const SRPG9_RGBA: [[f32; 4]; 12] = [
    rgba_hex("#c32020"), // Red
    rgba_hex("#bf0052"), // Pink
    rgba_hex("#9c0082"), // Purple
    rgba_hex("#5131a4"), // Violet
    rgba_hex("#006ecb"), // Blue
    rgba_hex("#009bcf"), // Light Blue
    rgba_hex("#51c0c8"), // Cyan
    rgba_hex("#36855b"), // Green-Blue
    rgba_hex("#3d6526"), // Green
    rgba_hex("#666000"), // Yellow
    rgba_hex("#954f00"), // Orange
    rgba_hex("#954f00"), // Orange
];

/// Simply Love-ish UI accent palette
pub const SIMPLY_LOVE_RGBA: [[f32; 4]; 12] = [
    rgba_hex("#FF5D47"),
    rgba_hex("#FF577E"),
    rgba_hex("#FF47B3"),
    rgba_hex("#DD57FF"),
    rgba_hex("#8885ff"),
    rgba_hex("#3D94FF"),
    rgba_hex("#00B8CC"),
    rgba_hex("#5CE087"),
    rgba_hex("#AEFA44"),
    rgba_hex("#FFFF00"),
    rgba_hex("#FFBE00"),
    rgba_hex("#FF7D00"),
];

/// Judgment colors (canonical defaults; runtime values come from config).
pub const DEFAULT_JUDGMENT_RGBA: [[f32; 4]; 6] = [
    rgba_hex("#21CCE8"), // Fantastic
    rgba_hex("#E29C18"), // Excellent
    rgba_hex("#66C955"), // Great
    rgba_hex("#B45CFF"), // Decent
    rgba_hex("#C9855E"), // Way Off
    rgba_hex("#FF3030"), // Miss
];

/// Dimmed judgment colors (default reference values).
pub const DEFAULT_JUDGMENT_DIM_RGBA: [[f32; 4]; 6] = [
    rgba_hex("#0C4E59"),
    rgba_hex("#593D09"),
    rgba_hex("#2D5925"),
    rgba_hex("#3F2059"),
    rgba_hex("#593B29"),
    rgba_hex("#591010"),
];

/// Dimmed judgment colors for eval (default reference values).
pub const DEFAULT_JUDGMENT_DIM_EVAL_RGBA: [[f32; 4]; 6] = [
    rgba_hex("#08363E"),
    rgba_hex("#3C2906"),
    rgba_hex("#1B3516"),
    rgba_hex("#301844"),
    rgba_hex("#352319"),
    rgba_hex("#440C0C"),
];

pub const DEFAULT_JUDGMENT_FA_PLUS_WHITE_RGBA: [f32; 4] = rgba_hex("#FFFFFF");
pub const DEFAULT_JUDGMENT_FA_PLUS_WHITE_EVAL_DIM_RGBA: [f32; 4] = rgba_hex("#444444");
pub const DEFAULT_JUDGMENT_FA_PLUS_WHITE_GAMEPLAY_DIM_RGBA: [f32; 4] = rgba_hex("#595959");

// Arrow Cloud "H.EX" score color (default reference value).
pub const DEFAULT_HARD_EX_SCORE_RGBA: [f32; 4] = rgba_hex("#FF00CC");

/// Scale factor that derives gameplay-dimmed judgement colors from their base.
/// Chosen to closely reproduce the historical `DEFAULT_JUDGMENT_DIM_RGBA`.
pub const JUDGMENT_GAMEPLAY_DIM_SCALE: f32 = 0.38;
/// Scale factor that derives evaluation-dimmed judgement colors from their base.
/// Chosen to closely reproduce the historical `DEFAULT_JUDGMENT_DIM_EVAL_RGBA`.
pub const JUDGMENT_EVAL_DIM_SCALE: f32 = 0.26;

pub use crate::judgment::{JudgmentMode, JudgmentWindow};

/// The gameplay judgement window mode for a player: FA+ when the white-Fantastic
/// split is shown, otherwise ITG. (HEX window colors only appear in explicit
/// Hard-EX eval/graph contexts; the H.EX *score number* is handled separately.)
pub fn gameplay_judgment_mode(show_fa_plus_window: bool) -> JudgmentMode {
    if show_fa_plus_window {
        JudgmentMode::FaPlus
    } else {
        JudgmentMode::Itg
    }
}

fn palette_for(mode: JudgmentMode) -> config::JudgmentPalette {
    config::get().judgment.palette(mode)
}

/// Multiply the RGB channels of an `[r, g, b, a]` color by `factor`, preserving
/// alpha. Used to derive dimmed judgement variants from a base color.
pub fn scale_rgb(c: [f32; 4], factor: f32) -> [f32; 4] {
    [c[0] * factor, c[1] * factor, c[2] * factor, c[3]]
}

/// The configured color for a single judgement window slot in `mode`, as a
/// renderer `[r, g, b, a]` array. This is the typed replacement for raw `[6]`
/// indexing into [`judgment_rgba`].
pub fn judgment_window_rgba(mode: JudgmentMode, window: JudgmentWindow) -> [f32; 4] {
    palette_for(mode).color(window).to_rgba()
}

/// Gameplay-dimmed color for a single judgement window slot in `mode`.
pub fn judgment_window_gameplay_dim_rgba(mode: JudgmentMode, window: JudgmentWindow) -> [f32; 4] {
    scale_rgb(judgment_window_rgba(mode, window), JUDGMENT_GAMEPLAY_DIM_SCALE)
}

/// Evaluation-dimmed color for a single judgement window slot in `mode`.
pub fn judgment_window_eval_dim_rgba(mode: JudgmentMode, window: JudgmentWindow) -> [f32; 4] {
    scale_rgb(judgment_window_rgba(mode, window), JUDGMENT_EVAL_DIM_SCALE)
}

/// The judgement windows that make up the **6-row ITG judgement display**, in
/// render order: a single combined Fantastic row (colored with `W0`), then
/// Excellent..Way Off, then Miss. This deliberately omits the white `W1`
/// sub-band, which only appears in the 7-row FA+ split display
/// ([`JudgmentWindow::ALL`]).
pub const ITG_DISPLAY_WINDOWS: [JudgmentWindow; 6] = [
    JudgmentWindow::W0,
    JudgmentWindow::W2,
    JudgmentWindow::W3,
    JudgmentWindow::W4,
    JudgmentWindow::W5,
    JudgmentWindow::Miss,
];

/// The six base judgement colors for `mode`'s ITG display, read from the live
/// config in [`ITG_DISPLAY_WINDOWS`] order (`[W0(blue), W2, W3, W4, W5, Miss]` —
/// omitting the white `W1` sub-band returned by [`judgment_white_fantastic_rgba`]).
///
/// Prefer [`judgment_window_rgba`] for a single named window; this array view is
/// retained for the judgement-counter render loops that iterate the 6 ITG rows.
pub fn judgment_rgba(mode: JudgmentMode) -> [[f32; 4]; 6] {
    let p = palette_for(mode);
    ITG_DISPLAY_WINDOWS.map(|w| p.color(w).to_rgba())
}

/// Gameplay-dimmed judgement window colors for `mode`, derived from the live
/// config by scaling.
pub fn judgment_dim_rgba(mode: JudgmentMode) -> [[f32; 4]; 6] {
    let base = judgment_rgba(mode);
    core::array::from_fn(|i| scale_rgb(base[i], JUDGMENT_GAMEPLAY_DIM_SCALE))
}

/// Evaluation-dimmed judgement window colors for `mode`, derived from the live
/// config by scaling.
pub fn judgment_dim_eval_rgba(mode: JudgmentMode) -> [[f32; 4]; 6] {
    let base = judgment_rgba(mode);
    core::array::from_fn(|i| scale_rgb(base[i], JUDGMENT_EVAL_DIM_SCALE))
}

/// The white W1 outer-Fantastic color for `mode`, read from the live config.
pub fn judgment_white_fantastic_rgba(mode: JudgmentMode) -> [f32; 4] {
    palette_for(mode).color(JudgmentWindow::W1).to_rgba()
}

/// Gameplay-dimmed white W1 outer-Fantastic color for `mode`.
pub fn judgment_white_fantastic_gameplay_dim_rgba(mode: JudgmentMode) -> [f32; 4] {
    scale_rgb(
        judgment_white_fantastic_rgba(mode),
        JUDGMENT_GAMEPLAY_DIM_SCALE,
    )
}

/// Evaluation-dimmed white W1 outer-Fantastic color for `mode`.
pub fn judgment_white_fantastic_eval_dim_rgba(mode: JudgmentMode) -> [f32; 4] {
    scale_rgb(judgment_white_fantastic_rgba(mode), JUDGMENT_EVAL_DIM_SCALE)
}

/// The Arrow Cloud "H.EX" score color, a single global accent (not a window).
pub fn hard_ex_score_rgba() -> [f32; 4] {
    config::get().judgment.hard_ex_score.to_rgba()
}

pub const EDIT_DIFFICULTY_RGBA: [f32; 4] = rgba_hex("#B4B7BA");

/// Returns the Simply Love color for a given difficulty, based on an active theme color index.
#[inline(always)]
pub fn difficulty_rgba(difficulty_name: &str, active_color_index: i32) -> [f32; 4] {
    if difficulty_name.eq_ignore_ascii_case("edit") {
        return EDIT_DIFFICULTY_RGBA;
    }
    let difficulty_index = FILE_DIFFICULTY_NAMES
        .iter()
        .position(|&name| name.eq_ignore_ascii_case(difficulty_name))
        .unwrap_or(2); // Default to Medium if not found

    let color_index = active_color_index - (4 - difficulty_index) as i32;
    simply_love_rgba(color_index)
}

#[inline(always)]
const fn wrap(n: usize, i: i32) -> usize {
    (i.rem_euclid(n as i32)) as usize
}

#[inline(always)]
pub fn decorative_rgba(idx: i32) -> [f32; 4] {
    DECORATIVE_RGBA[wrap(DECORATIVE_RGBA.len(), idx)]
}

#[inline(always)]
pub fn srpg9_rgba(idx: i32) -> [f32; 4] {
    SRPG9_RGBA[wrap(SRPG9_RGBA.len(), idx)]
}

#[inline(always)]
pub fn simply_love_rgba(idx: i32) -> [f32; 4] {
    SIMPLY_LOVE_RGBA[wrap(SIMPLY_LOVE_RGBA.len(), idx)]
}

/// Simply Love `LightenColor(c)` parity: multiplies RGB by 1.25, keeps alpha.
#[inline(always)]
pub fn lighten_rgba(c: [f32; 4]) -> [f32; 4] {
    [c[0] * 1.25, c[1] * 1.25, c[2] * 1.25, c[3]]
}

/// Menu selected color rule: “current `SIMPLY_LOVE` minus 2”
#[inline(always)]
pub fn menu_selected_rgba(active_idx: i32) -> [f32; 4] {
    simply_love_rgba(active_idx - 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srpg9_order_tracks_decorative_wheel() {
        assert_eq!(srpg9_rgba(0), rgba_hex("#c32020"));
        assert_eq!(srpg9_rgba(7), rgba_hex("#36855b"));
        assert_eq!(srpg9_rgba(8), rgba_hex("#3d6526"));
        assert_eq!(srpg9_rgba(9), rgba_hex("#666000"));
        assert_eq!(srpg9_rgba(11), rgba_hex("#954f00"));
    }

    /// The `[6]` ITG-display array view is exactly the per-window colors in
    /// [`ITG_DISPLAY_WINDOWS`] order. This locks the index→window mapping so a
    /// future refactor cannot silently make index 1 mean `W1` (white) instead of
    /// `W2` (Excellent).
    #[test]
    fn judgment_rgba_matches_itg_display_window_order() {
        for mode in JudgmentMode::ALL {
            let arr = judgment_rgba(mode);
            for (i, window) in ITG_DISPLAY_WINDOWS.iter().copied().enumerate() {
                assert_eq!(
                    arr[i],
                    judgment_window_rgba(mode, window),
                    "judgment_rgba[{i}] should be {window:?} for {mode:?}"
                );
            }
        }
        // Index 1 is Excellent (W2), NOT the white W1 sub-band.
        assert_eq!(ITG_DISPLAY_WINDOWS[1], JudgmentWindow::W2);
    }

    /// The 7-row FA+ split display is `JudgmentWindow::ALL` in order, with the
    /// white outer-Fantastic accessor equal to the `W1` slot. This is the exact
    /// layout the gameplay/eval counters build, so it must stay pixel-identical.
    #[test]
    fn fa_plus_split_layout_is_all_windows_with_white_at_w1() {
        for mode in JudgmentMode::ALL {
            assert_eq!(JudgmentWindow::ALL[0], JudgmentWindow::W0);
            assert_eq!(JudgmentWindow::ALL[1], JudgmentWindow::W1);
            assert_eq!(
                judgment_window_rgba(mode, JudgmentWindow::W1),
                judgment_white_fantastic_rgba(mode),
                "W1 must be the white outer-Fantastic color for {mode:?}"
            );
        }
    }

    /// With the default config, the themed accessors reproduce the historical
    /// hardcoded `DEFAULT_*` judgement colors exactly (pure-refactor guarantee).
    #[test]
    fn default_config_reproduces_historical_judgment_colors() {
        for mode in JudgmentMode::ALL {
            assert_eq!(judgment_rgba(mode), DEFAULT_JUDGMENT_RGBA);
            assert_eq!(
                judgment_white_fantastic_rgba(mode),
                DEFAULT_JUDGMENT_FA_PLUS_WHITE_RGBA
            );
        }
        assert_eq!(hard_ex_score_rgba(), DEFAULT_HARD_EX_SCORE_RGBA);
    }
}
