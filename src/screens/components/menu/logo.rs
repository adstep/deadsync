use crate::act;
use crate::assets;
use crate::engine::present::actors::Actor;
use crate::engine::space::screen_center_x;

/// Parameters to tweak the layout easily.
#[derive(Clone, Copy, Debug)]
pub struct LogoParams {
    pub target_h: f32,
    pub top_margin: f32,
    /// Positive values move the banner *up* inside the logo.
    pub banner_y_offset_inside: f32,
}

impl Default for LogoParams {
    fn default() -> Self {
        Self {
            target_h: 238.0,
            top_margin: 102.0,
            banner_y_offset_inside: 0.0,
        }
    }
}

/// Animated arrow row config — mirrors the `ds-arrow-pulse` CSS in
/// `assets/telemetry/versus.html`.
const ARROW_COUNT: usize = 8;
const ARROW_PULSE_PERIOD: f32 = 2.0;
const ARROW_PULSE_STAGGER: f32 = 0.125;
const ARROW_PULSE_MIN: f32 = 1.0;
const ARROW_PULSE_MAX: f32 = 1.25;
/// Fraction of the logo's height used by the arrow row (matches the
/// reference HTML's ~17% allocation between the DEAD/SYNC strips).
const ARROW_ROW_H_FRAC: f32 = 0.18;
/// Fraction of the logo's height for the DEAD / SYNC text strips.
const TEXT_STRIP_H_FRAC: f32 = 0.40;

const ARROW_TEXTURES: [&str; ARROW_COUNT] = [
    "init_arrow_1.png",
    "init_arrow_2.png",
    "init_arrow_3.png",
    "init_arrow_4.png",
    "init_arrow_5.png",
    "init_arrow_6.png",
    "init_arrow_7.png",
    "init_arrow_8.png",
];

/// Build the “banner inside logo” stack with the actor DSL.
/// Returns a `Vec<Actor>` to be included in a screen's actor list.
pub fn build_logo(params: LogoParams) -> Vec<Actor> {
    // Layout width is anchored to the DEAD strip's aspect ratio so the
    // composite (DEAD + arrows + SYNC) keeps the same footprint as the
    // old single `logo.png` (≈ 2.95:1 at target_h=238).
    let dead_dims =
        assets::texture_dims("menu_dead.png").unwrap_or(assets::TexMeta { w: 827, h: 280 });
    let dead_h = params.target_h * TEXT_STRIP_H_FRAC;
    let dead_aspect = if dead_dims.h > 0 {
        dead_dims.w as f32 / dead_dims.h as f32
    } else {
        827.0 / 280.0
    };
    let logo_w = dead_h * dead_aspect;
    let logo_h = params.target_h;

    let center_x = screen_center_x();
    let logo_top_y = params.top_margin;
    let dance_center_y = 0.5f32.mul_add(logo_h, logo_top_y) - params.banner_y_offset_inside;

    // Arrow row geometry.
    let arrow_dims =
        assets::texture_dims("init_arrow_1.png").unwrap_or(assets::TexMeta { w: 77, h: 90 });
    let arrow_aspect = if arrow_dims.h > 0 {
        arrow_dims.w as f32 / arrow_dims.h as f32
    } else {
        77.0 / 90.0
    };
    let arrow_h = logo_h * ARROW_ROW_H_FRAC;
    let arrow_w = arrow_h * arrow_aspect;
    // Spread the 8 arrows across roughly 90% of the logo width.
    let arrow_span = logo_w * 0.90;
    let arrow_spacing = if ARROW_COUNT > 1 {
        arrow_span / (ARROW_COUNT as f32 - 1.0)
    } else {
        0.0
    };
    let arrow_row_y = logo_top_y + logo_h * (TEXT_STRIP_H_FRAC + ARROW_ROW_H_FRAC * 0.5);
    let first_arrow_x = center_x - arrow_span * 0.5;

    // DEAD text strip — sits at the top of the logo box.
    let dead_top_y = logo_top_y;
    // SYNC text strip — sits at the bottom of the logo box.
    let sync_bot_y = logo_top_y + logo_h;

    let mut out: Vec<Actor> = Vec::with_capacity(3 + ARROW_COUNT);

    // Background dance banner (unchanged).
    out.push(act!(sprite("dance.png"):
        align(0.5, 0.5):
        xy(center_x, dance_center_y):
        zoomtowidth(logo_w)
    ));

    // DEAD strip.
    out.push(act!(sprite("menu_dead.png"):
        align(0.5, 0.0):
        xy(center_x, dead_top_y):
        zoomtowidth(logo_w)
    ));

    // Animated colored arrows — `pulse` is a built-in looping effect
    // mode on the engine clock, so the animation continues for as long
    // as the actor exists. Per-arrow `effectoffset` staggers the wave
    // so the row looks like a travelling ripple.
    for i in 0..ARROW_COUNT {
        let x = first_arrow_x + arrow_spacing * (i as f32);
        let offset = ARROW_PULSE_STAGGER * (i as f32);
        out.push(act!(sprite(ARROW_TEXTURES[i]):
            align(0.5, 0.5):
            xy(x, arrow_row_y):
            zoomtoheight(arrow_h):
            pulse():
            effectperiod(ARROW_PULSE_PERIOD):
            effectoffset(offset):
            effectmagnitude(ARROW_PULSE_MIN, ARROW_PULSE_MAX, 0.0)
        ));
        // Silence dead_code warning on arrow_w (kept for future use /
        // potential horizontal layout debugging).
        let _ = arrow_w;
    }

    // SYNC strip.
    out.push(act!(sprite("menu_sync.png"):
        align(0.5, 1.0):
        xy(center_x, sync_bot_y):
        zoomtowidth(logo_w)
    ));

    out
}

/// Convenience: build with default params.
pub fn build_logo_default() -> Vec<Actor> {
    build_logo(LogoParams::default())
}
