//! Rendering for the latency calibration screen. Pure view layer: reads the
//! `State` accessors defined in `mod.rs` and emits `Actor`s per phase.

use crate::act;
use crate::assets::i18n::tr;
use crate::assets::{FontRole, current_machine_font_key_for_text};
use crate::screens::components::shared::visual_style_bg;
use deadsync_present::actors::Actor;
use deadsync_present::space;
use deadsync_present::space::{screen_center_x, screen_height, screen_width};

use super::{Phase, State};

const SECTION: &str = "ScreenCalibrateLatency";

pub(super) fn push_actors(actors: &mut Vec<Actor>, state: &State, alpha_mul: f32) {
    actors.reserve(40);
    let screen_w = screen_width();
    let screen_h = screen_height();

    state.bg.push(
        actors,
        visual_style_bg::Params {
            active_color_index: state.active_color_index,
            backdrop_rgba: [0.0, 0.0, 0.0, 1.0],
            alpha_mul,
        },
    );

    // Phase-specific full-screen flash (drawn behind text).
    push_flash(actors, state, screen_w, screen_h, alpha_mul);

    // Title.
    let title = tr(SECTION, "HeaderText");
    let title_font = current_machine_font_key_for_text(FontRole::Header, &title);
    let title_scale = if space::is_wide() { 0.6 } else { 0.5 };
    actors.push(act!(text:
        font(title_font):
        settext(title):
        align(0.5, 0.5):
        xy(screen_center_x(), 28.0):
        zoom(title_scale):
        maxwidth(screen_w * 0.8):
        horizalign(center):
        diffuse(1.0, 1.0, 1.0, 0.96 * alpha_mul):
        z(85)
    ));

    match state.phase() {
        Phase::Intro => push_intro(actors, state, screen_w, screen_h, alpha_mul),
        Phase::AudioWizard => push_audio_wizard(actors, state, screen_w, screen_h, alpha_mul),
        Phase::VisualWizard => push_visual_wizard(actors, state, screen_w, screen_h, alpha_mul),
        Phase::Certifier => push_certifier(actors, state, screen_w, screen_h, alpha_mul),
        Phase::Results => push_results(actors, state, screen_w, screen_h, alpha_mul),
    }

    // Footer help.
    let footer_key = match state.phase() {
        Phase::Intro => "FooterIntro",
        Phase::Results => "FooterResults",
        _ => "FooterTap",
    };
    actors.push(act!(text:
        font("miso"):
        settext(tr(SECTION, footer_key)):
        align(0.5, 0.5):
        xy(screen_center_x(), screen_h - 22.0):
        zoom(0.74):
        maxwidth(screen_w * 0.92):
        horizalign(center):
        diffuse(1.0, 1.0, 1.0, 0.74 * alpha_mul):
        z(90)
    ));
}

fn push_flash(
    actors: &mut Vec<Actor>,
    state: &State,
    screen_w: f32,
    screen_h: f32,
    alpha_mul: f32,
) {
    let intensity = match state.phase() {
        Phase::VisualWizard => state.flash_intensity(),
        Phase::Certifier => state.cert_stimulus().0,
        _ => 0.0,
    };
    if intensity <= 0.0 {
        return;
    }
    actors.push(act!(quad:
        align(0.5, 0.5):
        xy(screen_center_x(), screen_h * 0.42):
        zoomto(screen_w * 0.5, screen_h * 0.28):
        diffuse(1.0, 1.0, 1.0, intensity * 0.9 * alpha_mul):
        z(70)
    ));
}

fn push_lines(
    actors: &mut Vec<Actor>,
    lines: &[(String, f32)],
    screen_w: f32,
    base_y: f32,
    spacing: f32,
    alpha_mul: f32,
) {
    for (idx, (text, alpha)) in lines.iter().enumerate() {
        actors.push(act!(text:
            font("miso"):
            settext(text.clone()):
            align(0.5, 0.5):
            xy(screen_width() * 0.5, base_y + idx as f32 * spacing):
            zoom(0.85):
            maxwidth(screen_w * 0.86):
            horizalign(center):
            diffuse(1.0, 1.0, 1.0, alpha * alpha_mul):
            strokecolor(0.0, 0.0, 0.0, 0.6 * alpha_mul):
            shadowlength(1.0):
            z(86)
        ));
    }
}

fn push_intro(
    actors: &mut Vec<Actor>,
    state: &State,
    screen_w: f32,
    screen_h: f32,
    alpha_mul: f32,
) {
    let seed_ms = state.estimated_output_delay_ms();
    let lines = vec![
        (tr(SECTION, "IntroBody1").to_string(), 0.95),
        (tr(SECTION, "IntroBody2").to_string(), 0.9),
        (
            format!("{}: {:.1} ms", tr(SECTION, "IntroSeed"), seed_ms),
            0.8,
        ),
    ];
    push_lines(actors, &lines, screen_w, screen_h * 0.42, 40.0, alpha_mul);
}

fn push_audio_wizard(
    actors: &mut Vec<Actor>,
    state: &State,
    screen_w: f32,
    screen_h: f32,
    alpha_mul: f32,
) {
    // On-beat pulse indicator (a growing dot).
    let pulse = state.audio_pulse();
    let size = 30.0 + pulse * 80.0;
    actors.push(act!(quad:
        align(0.5, 0.5):
        xy(screen_center_x(), screen_h * 0.40):
        zoomto(size, size):
        diffuse(1.0, 1.0, 1.0, (0.35 + 0.6 * pulse) * alpha_mul):
        z(72)
    ));
    let (got, want) = state.wizard_progress();
    let lines = vec![
        (tr(SECTION, "AudioBody").to_string(), 0.95),
        (format!("{} {got}/{want}", tr(SECTION, "TapsLabel")), 0.85),
    ];
    push_lines(actors, &lines, screen_w, screen_h * 0.62, 40.0, alpha_mul);
}

fn push_visual_wizard(
    actors: &mut Vec<Actor>,
    state: &State,
    screen_w: f32,
    screen_h: f32,
    alpha_mul: f32,
) {
    let (got, want) = state.wizard_progress();
    let lines = vec![
        (tr(SECTION, "VisualBody").to_string(), 0.95),
        (format!("{} {got}/{want}", tr(SECTION, "TapsLabel")), 0.85),
    ];
    push_lines(actors, &lines, screen_w, screen_h * 0.62, 40.0, alpha_mul);
}

fn push_certifier(
    actors: &mut Vec<Actor>,
    state: &State,
    screen_w: f32,
    screen_h: f32,
    alpha_mul: f32,
) {
    let (audio_done, visual_done, want) = state.cert_progress();
    let lines = vec![
        (tr(SECTION, "CertBody").to_string(), 0.95),
        (
            format!(
                "{}: {audio_done}/{want}   {}: {visual_done}/{want}",
                tr(SECTION, "CertAudio"),
                tr(SECTION, "CertVisual")
            ),
            0.85,
        ),
    ];
    push_lines(actors, &lines, screen_w, screen_h * 0.62, 40.0, alpha_mul);
}

fn push_results(
    actors: &mut Vec<Actor>,
    state: &State,
    screen_w: f32,
    screen_h: f32,
    alpha_mul: f32,
) {
    let view = state.results_lines();
    let mut lines: Vec<(String, f32)> = Vec::with_capacity(6);

    if let Some(a) = view.audio {
        let status = tr(SECTION, if a.gate_ok { "Apply" } else { "Skip" });
        lines.push((
            format!(
                "{}: {:+.0} ms  (err {:+.1} ms, sd {:.1} ms, n={}) [{status}]",
                tr(SECTION, "AudioOffset"),
                a.suggested_ms,
                a.mean_ms,
                a.stddev_ms,
                a.scored
            ),
            if a.gate_ok { 1.0 } else { 0.6 },
        ));
    }
    if let Some(v) = view.visual {
        let status = tr(SECTION, if v.gate_ok { "Apply" } else { "Skip" });
        lines.push((
            format!(
                "{}: {:+.0} ms  (err {:+.1} ms, sd {:.1} ms, n={}) [{status}]",
                tr(SECTION, "VisualOffset"),
                v.suggested_ms,
                v.mean_ms,
                v.stddev_ms,
                v.scored
            ),
            if v.gate_ok { 1.0 } else { 0.6 },
        ));
    }
    if let Some(c) = view.cert {
        lines.push((
            format!(
                "{}: {:.0} ms (n={})   {}: {:.0} ms (n={})",
                tr(SECTION, "CertAudio"),
                c.audio_ms,
                c.audio_count,
                tr(SECTION, "CertVisual"),
                c.visual_ms,
                c.visual_count
            ),
            0.85,
        ));
        lines.push((
            format!("{}: {:+.0} ms", tr(SECTION, "CertSkew"), c.skew_ms),
            0.85,
        ));
    }

    push_lines(actors, &lines, screen_w, screen_h * 0.42, 42.0, alpha_mul);
}
