use std::time::Duration;

/* ---------------------------- player slots ---------------------------- */
pub const PLAYER_SLOTS: usize = 2;
pub const P1: usize = 0;
pub const P2: usize = 1;

/* ------------------------------ row layout ----------------------------- */
// Match Simply Love / ScreenOptions defaults.
pub(crate) const VISIBLE_ROWS: usize = 10;
pub(crate) const ROW_START_OFFSET: f32 = -164.0;
pub(crate) const ROW_HEIGHT: f32 = 33.0;
pub(crate) const TITLE_BG_WIDTH: f32 = 127.0;

/* ---------------------------- cursor tweening -------------------------- */
// Simply Love metrics.ini uses 0.1 for both [ScreenOptions] TweenSeconds and CursorTweenSeconds.
// Player Options row/cursor motion should keep this exact parity timing.
pub(crate) const SL_OPTION_ROW_TWEEN_SECONDS: f32 = 0.1;
pub(crate) const CURSOR_TWEEN_SECONDS: f32 = SL_OPTION_ROW_TWEEN_SECONDS;
pub(crate) const ROW_TWEEN_SECONDS: f32 = SL_OPTION_ROW_TWEEN_SECONDS;
// Simply Love [ScreenOptions] uses RowOnCommand/RowOffCommand with linear,0.2.
pub(crate) const PANE_FADE_SECONDS: f32 = 0.2;

/* --------------------------- preview / spacing ------------------------- */
pub(crate) const TAP_EXPLOSION_PREVIEW_SPEED: f32 = 0.7;
// Spacing between inline items in OptionRows (pixels at current zoom)
pub(crate) const INLINE_SPACING: f32 = 15.75;

/* --------------------------- tilt intensity --------------------------- */
pub(crate) const TILT_INTENSITY_MIN: f32 = 0.05;
pub(crate) const TILT_INTENSITY_MAX: f32 = 10.00;
pub(crate) const TILT_INTENSITY_STEP: f32 = 0.05;

/* ----------------------------- HUD offset ----------------------------- */
pub(crate) const HUD_OFFSET_MIN: i32 = crate::game::profile::HUD_OFFSET_MIN;
pub(crate) const HUD_OFFSET_MAX: i32 = crate::game::profile::HUD_OFFSET_MAX;
pub(crate) const HUD_OFFSET_ZERO_INDEX: usize = (-HUD_OFFSET_MIN) as usize;

/* -------------------------- hold-to-scroll timing --------------------- */
pub const NAV_INITIAL_HOLD_DELAY: Duration = Duration::from_millis(300);
pub const NAV_REPEAT_SCROLL_INTERVAL: Duration = Duration::from_millis(50);

/* ----------------------------- choice labels -------------------------- */
pub const MATCH_NOTESKIN_LABEL: &str = "MatchNoteSkinLabel";
pub const NO_TAP_EXPLOSION_LABEL: &str = "NoTapExplosionLabel";
pub const ARCADE_NEXT_ROW_TEXT: &str = "▼";
