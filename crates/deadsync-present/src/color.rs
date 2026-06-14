//! Theme palettes and color helpers for the present layer.
//!
//! The canonical [`Color`] type now lives in `deadsync-core` and is re-exported
//! here so existing `crate::color::Color` paths keep working.

pub use deadsync_core::Color;

/// Parse a hex color string into a raw `[f32; 4]` array.
///
/// Thin `const` wrapper over [`deadsync_core::color::rgba_hex`], kept for the
/// `rgba!` / `rgba_const!` macros and array-based call sites. Accepts
/// "#rgb", "#rgba", "#rrggbb", "#rrggbbaa" (or without '#'); panics on invalid
/// input, so use only with trusted literals.
pub const fn rgba_hex(s: &str) -> [f32; 4] {
    deadsync_core::color::rgba_hex(s)
}

#[macro_export]
macro_rules! rgba {
    ($hex:literal $(,)?) => {
        $crate::color::rgba_hex($hex)
    };
}

#[macro_export]
macro_rules! rgba_const {
    ($name:ident, $hex:literal $(,)?) => {
        const $name: [f32; 4] = $crate::color::rgba_hex($hex);
    };
    ($vis:vis $name:ident, $hex:literal $(,)?) => {
        $vis const $name: [f32; 4] = $crate::color::rgba_hex($hex);
    };
}

/* =============================== COLOR TYPE =============================== */

// `Color` is defined in `deadsync-core` and re-exported above. Palettes and
// helpers below build on that canonical type.

/// Convert a typed `[Color; N]` palette into its raw `[[f32; 4]; N]` form at
/// compile time, so the `*_RGBA` aliases can be derived from the typed
/// source-of-truth palettes below.
const fn colors_to_rgba<const N: usize>(colors: [Color; N]) -> [[f32; 4]; N] {
    let mut out = [[0.0; 4]; N];
    let mut i = 0;
    while i < N {
        out[i] = colors[i].0;
        i += 1;
    }
    out
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
pub const DECORATIVE: [Color; 12] = [
    Color::hex("#FF3C23"),
    Color::hex("#FF003C"),
    Color::hex("#C1006F"),
    Color::hex("#8200A1"),
    Color::hex("#413AD0"),
    Color::hex("#0073FF"),
    Color::hex("#00ADC0"),
    Color::hex("#5CE087"),
    Color::hex("#AEFA44"),
    Color::hex("#FFFF00"),
    Color::hex("#FFBE00"),
    Color::hex("#FF7D00"),
];
/// Raw `[f32; 4]` view of [`DECORATIVE`] for array-based call sites.
pub const DECORATIVE_RGBA: [[f32; 4]; 12] = colors_to_rgba(DECORATIVE);

/// Simply Love SRPG9 event colors mapped to the normal Select Color hue wheel.
/// The source theme uses `SL.SRPG9.Colors` directly when SRPG9 is active, but
/// DeadSync's Select Color screen is keyed to [`DECORATIVE`].
pub const SRPG9: [Color; 12] = [
    Color::hex("#c32020"), // Red
    Color::hex("#bf0052"), // Pink
    Color::hex("#9c0082"), // Purple
    Color::hex("#5131a4"), // Violet
    Color::hex("#006ecb"), // Blue
    Color::hex("#009bcf"), // Light Blue
    Color::hex("#51c0c8"), // Cyan
    Color::hex("#36855b"), // Green-Blue
    Color::hex("#3d6526"), // Green
    Color::hex("#666000"), // Yellow
    Color::hex("#954f00"), // Orange
    Color::hex("#954f00"), // Orange
];
/// Raw `[f32; 4]` view of [`SRPG9`] for array-based call sites.
pub const SRPG9_RGBA: [[f32; 4]; 12] = colors_to_rgba(SRPG9);

/// Simply Love-ish UI accent palette
pub const SIMPLY_LOVE: [Color; 12] = [
    Color::hex("#FF5D47"),
    Color::hex("#FF577E"),
    Color::hex("#FF47B3"),
    Color::hex("#DD57FF"),
    Color::hex("#8885ff"),
    Color::hex("#3D94FF"),
    Color::hex("#00B8CC"),
    Color::hex("#5CE087"),
    Color::hex("#AEFA44"),
    Color::hex("#FFFF00"),
    Color::hex("#FFBE00"),
    Color::hex("#FF7D00"),
];
/// Raw `[f32; 4]` view of [`SIMPLY_LOVE`] for array-based call sites.
pub const SIMPLY_LOVE_RGBA: [[f32; 4]; 12] = colors_to_rgba(SIMPLY_LOVE);

/// Judgment colors
pub const JUDGMENT: [Color; 6] = [
    Color::hex("#21CCE8"), // Fantastic
    Color::hex("#E29C18"), // Excellent
    Color::hex("#66C955"), // Great
    Color::hex("#B45CFF"), // Decent
    Color::hex("#C9855E"), // Way Off
    Color::hex("#FF3030"), // Miss
];
/// Raw `[f32; 4]` view of [`JUDGMENT`] for array-based call sites.
pub const JUDGMENT_RGBA: [[f32; 4]; 6] = colors_to_rgba(JUDGMENT);

/// Dimmed judgment colors
pub const JUDGMENT_DIM: [Color; 6] = [
    Color::hex("#0C4E59"),
    Color::hex("#593D09"),
    Color::hex("#2D5925"),
    Color::hex("#3F2059"),
    Color::hex("#593B29"),
    Color::hex("#591010"),
];
/// Raw `[f32; 4]` view of [`JUDGMENT_DIM`] for array-based call sites.
pub const JUDGMENT_DIM_RGBA: [[f32; 4]; 6] = colors_to_rgba(JUDGMENT_DIM);

/// Dimmed judgment colors for eval
pub const JUDGMENT_DIM_EVAL: [Color; 6] = [
    Color::hex("#08363E"),
    Color::hex("#3C2906"),
    Color::hex("#1B3516"),
    Color::hex("#301844"),
    Color::hex("#352319"),
    Color::hex("#440C0C"),
];
/// Raw `[f32; 4]` view of [`JUDGMENT_DIM_EVAL`] for array-based call sites.
pub const JUDGMENT_DIM_EVAL_RGBA: [[f32; 4]; 6] = colors_to_rgba(JUDGMENT_DIM_EVAL);

pub const JUDGMENT_FA_PLUS_WHITE: Color = Color::hex("#FFFFFF");
pub const JUDGMENT_FA_PLUS_WHITE_RGBA: [f32; 4] = JUDGMENT_FA_PLUS_WHITE.to_array();
pub const JUDGMENT_FA_PLUS_WHITE_EVAL_DIM: Color = Color::hex("#444444");
pub const JUDGMENT_FA_PLUS_WHITE_EVAL_DIM_RGBA: [f32; 4] =
    JUDGMENT_FA_PLUS_WHITE_EVAL_DIM.to_array();
pub const JUDGMENT_FA_PLUS_WHITE_GAMEPLAY_DIM: Color = Color::hex("#595959");
pub const JUDGMENT_FA_PLUS_WHITE_GAMEPLAY_DIM_RGBA: [f32; 4] =
    JUDGMENT_FA_PLUS_WHITE_GAMEPLAY_DIM.to_array();

// Arrow Cloud "H.EX" score color.
pub const HARD_EX_SCORE: Color = Color::hex("#FF00CC");
pub const HARD_EX_SCORE_RGBA: [f32; 4] = HARD_EX_SCORE.to_array();

pub const EDIT_DIFFICULTY: Color = Color::hex("#B4B7BA");
pub const EDIT_DIFFICULTY_RGBA: [f32; 4] = EDIT_DIFFICULTY.to_array();

/// Returns the Simply Love color for a given difficulty, based on an active theme color index.
#[inline(always)]
pub fn difficulty_color(difficulty_name: &str, active_color_index: i32) -> Color {
    if difficulty_name.eq_ignore_ascii_case("edit") {
        return EDIT_DIFFICULTY;
    }
    let difficulty_index = FILE_DIFFICULTY_NAMES
        .iter()
        .position(|&name| name.eq_ignore_ascii_case(difficulty_name))
        .unwrap_or(2); // Default to Medium if not found

    let color_index = active_color_index - (4 - difficulty_index) as i32;
    simply_love_color(color_index)
}

/// `[f32; 4]` view of [`difficulty_color`].
#[inline(always)]
pub fn difficulty_rgba(difficulty_name: &str, active_color_index: i32) -> [f32; 4] {
    difficulty_color(difficulty_name, active_color_index).0
}

#[inline(always)]
const fn wrap(n: usize, i: i32) -> usize {
    (i.rem_euclid(n as i32)) as usize
}

#[inline(always)]
pub fn decorative_color(idx: i32) -> Color {
    DECORATIVE[wrap(DECORATIVE.len(), idx)]
}

#[inline(always)]
pub fn decorative_rgba(idx: i32) -> [f32; 4] {
    decorative_color(idx).0
}

#[inline(always)]
pub fn srpg9_color(idx: i32) -> Color {
    SRPG9[wrap(SRPG9.len(), idx)]
}

#[inline(always)]
pub fn srpg9_rgba(idx: i32) -> [f32; 4] {
    srpg9_color(idx).0
}

#[inline(always)]
pub fn simply_love_color(idx: i32) -> Color {
    SIMPLY_LOVE[wrap(SIMPLY_LOVE.len(), idx)]
}

#[inline(always)]
pub fn simply_love_rgba(idx: i32) -> [f32; 4] {
    simply_love_color(idx).0
}

/// Simply Love `LightenColor(c)` parity: multiplies RGB by 1.25, keeps alpha.
#[inline(always)]
pub fn lighten_color(c: Color) -> Color {
    Color([c.0[0] * 1.25, c.0[1] * 1.25, c.0[2] * 1.25, c.0[3]])
}

/// `[f32; 4]` view of [`lighten_color`].
#[inline(always)]
pub fn lighten_rgba(c: [f32; 4]) -> [f32; 4] {
    lighten_color(Color(c)).0
}

/// Menu selected color rule: “current `SIMPLY_LOVE` minus 2”
#[inline(always)]
pub fn menu_selected_color(active_idx: i32) -> Color {
    simply_love_color(active_idx - 2)
}

/// `[f32; 4]` view of [`menu_selected_color`].
#[inline(always)]
pub fn menu_selected_rgba(active_idx: i32) -> [f32; 4] {
    menu_selected_color(active_idx).0
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

    #[test]
    fn color_round_trips_with_array() {
        let arr = [0.1, 0.2, 0.3, 0.4];
        let c = Color::from(arr);
        assert_eq!(c.to_array(), arr);
        assert_eq!(<[f32; 4]>::from(c), arr);
        assert_eq!(c, Color::new(0.1, 0.2, 0.3, 0.4));
    }

    #[test]
    fn color_hex_matches_rgba_hex() {
        assert_eq!(Color::hex("#21CCE8").to_array(), rgba_hex("#21CCE8"));
        assert_eq!(Color::rgb(1.0, 1.0, 1.0), Color::WHITE);
        assert_eq!(Color::WHITE.with_alpha(0.0), Color::new(1.0, 1.0, 1.0, 0.0));
    }

    #[test]
    fn derived_rgba_arrays_match_typed_palettes() {
        // The `*_RGBA` arrays are derived from the typed `[Color; N]` palettes;
        // verify the derivation stays in lock-step.
        assert_eq!(JUDGMENT_RGBA[0], JUDGMENT[0].to_array());
        assert_eq!(DECORATIVE_RGBA[2], DECORATIVE[2].to_array());
        assert_eq!(SIMPLY_LOVE_RGBA[5], SIMPLY_LOVE[5].to_array());
        assert_eq!(HARD_EX_SCORE_RGBA, HARD_EX_SCORE.to_array());
    }

    #[test]
    fn typed_and_array_accessors_agree() {
        assert_eq!(decorative_color(3).to_array(), decorative_rgba(3));
        assert_eq!(simply_love_color(-2).to_array(), menu_selected_rgba(0));
    }
}
