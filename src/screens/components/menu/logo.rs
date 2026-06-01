use std::sync::OnceLock;
use std::time::Instant;

use crate::act;
use crate::assets;
use crate::engine::present::actors::Actor;
use crate::engine::space::screen_center_x;

/// Process-global wall clock for the looping arrow pulse — mirrors the
/// per-frame recompute pattern used by `src/screens/init.rs`.
fn elapsed_seconds() -> f32 {
    static START: OnceLock<Instant> = OnceLock::new();
    let start = START.get_or_init(Instant::now);
    start.elapsed().as_secs_f32()
}

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
/// `assets/telemetry/versus.html`:
///   @keyframes ds-arrow-pulse {
///     0%, 70%, 100% { transform: scale(1); }
///     10%           { transform: scale(1.25); }
///     30%           { transform: scale(1); }
///   }
const ARROW_COUNT: usize = 8;
const ARROW_PULSE_PERIOD: f32 = 2.0;
const ARROW_PULSE_STAGGER: f32 = ARROW_PULSE_PERIOD / ARROW_COUNT as f32;
const ARROW_PULSE_MIN: f32 = 1.0;
const ARROW_PULSE_MAX: f32 = 1.25;
const ARROW_PULSE_UP_DUR: f32 = ARROW_PULSE_PERIOD * 0.10;
const ARROW_PULSE_DOWN_DUR: f32 = ARROW_PULSE_PERIOD * 0.20;
/// Additive white glow alpha at peak (mirrors the CSS
/// `drop-shadow + brightness` burst).
const ARROW_PULSE_GLOW_ALPHA: f32 = 0.50;

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

/// Build the "banner inside logo" stack with the actor DSL.
///
/// Layout:
///   1. `logo.png` — DEAD/SYNC wordmark frame (white-on-transparent).
///   2. 8 pre-colored arrow sprites pulsing with the
///      `ds-arrow-pulse` timing from `assets/telemetry/versus.html`.
pub fn build_logo(params: LogoParams) -> Vec<Actor> {
    let logo_dims = assets::texture_dims("logo.png").unwrap_or(assets::TexMeta { w: 1, h: 1 });
    let logo_aspect = if logo_dims.h > 0 {
        logo_dims.w as f32 / logo_dims.h as f32
    } else {
        1.0
    };

    let logo_h = params.target_h;
    let logo_w = logo_h * logo_aspect;
    let center_x = screen_center_x();
    let logo_top_y = params.top_margin;
    let dance_center_y = 0.5f32.mul_add(logo_h, logo_top_y) - params.banner_y_offset_inside;

    // Dance row height matches dance.png's native aspect inside `logo_w`.
    let dance_dims =
        assets::texture_dims("dance.png").unwrap_or(assets::TexMeta { w: 1360, h: 164 });
    let dance_aspect = if dance_dims.h > 0 {
        dance_dims.w as f32 / dance_dims.h as f32
    } else {
        1360.0 / 164.0
    };
    let arrow_band_h = logo_w / dance_aspect;
    let arrow_spacing = logo_w / ARROW_COUNT as f32;
    let first_arrow_center_x = center_x - logo_w * 0.5 + arrow_spacing * 0.5;

    let mut out: Vec<Actor> = Vec::with_capacity(1 + ARROW_COUNT);

    // DEAD/SYNC wordmark frame.
    out.push(act!(sprite("logo.png"):
        align(0.5, 0.0):
        xy(center_x, logo_top_y):
        zoomtoheight(logo_h)
    ));

    // Animated colored arrows in the gap between DEAD and SYNC.
    let now = elapsed_seconds();
    for i in 0..ARROW_COUNT {
        let x = first_arrow_center_x + arrow_spacing * (i as f32);
        let stagger = ARROW_PULSE_STAGGER * (i as f32);
        let phase = (now - stagger).rem_euclid(ARROW_PULSE_PERIOD);
        let scale = arrow_pulse_scale(phase);
        let glow_a = arrow_pulse_glow(phase);
        let target_h = arrow_band_h * scale;
        out.push(act!(sprite(ARROW_TEXTURES[i]):
            align(0.5, 0.5):
            xy(x, dance_center_y):
            zoomtoheight(target_h):
            z(50):
            glow(1.0, 1.0, 1.0, glow_a)
        ));
    }

    out
}

/// Convenience: build with default params.
pub fn build_logo_default() -> Vec<Actor> {
    build_logo(LogoParams::default())
}

#[inline]
fn arrow_pulse_scale(t: f32) -> f32 {
    if t < ARROW_PULSE_UP_DUR {
        let p = t / ARROW_PULSE_UP_DUR;
        ARROW_PULSE_MIN + (ARROW_PULSE_MAX - ARROW_PULSE_MIN) * p
    } else if t < ARROW_PULSE_UP_DUR + ARROW_PULSE_DOWN_DUR {
        let p = (t - ARROW_PULSE_UP_DUR) / ARROW_PULSE_DOWN_DUR;
        ARROW_PULSE_MAX - (ARROW_PULSE_MAX - ARROW_PULSE_MIN) * p
    } else {
        ARROW_PULSE_MIN
    }
}

#[inline]
fn arrow_pulse_glow(t: f32) -> f32 {
    if t < ARROW_PULSE_UP_DUR {
        (t / ARROW_PULSE_UP_DUR) * ARROW_PULSE_GLOW_ALPHA
    } else if t < ARROW_PULSE_UP_DUR + ARROW_PULSE_DOWN_DUR {
        let p = (t - ARROW_PULSE_UP_DUR) / ARROW_PULSE_DOWN_DUR;
        (1.0 - p) * ARROW_PULSE_GLOW_ALPHA
    } else {
        0.0
    }
}
