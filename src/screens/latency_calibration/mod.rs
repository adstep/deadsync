//! Perceptual ("end-to-end") latency calibration screen.
//!
//! Mirrors osu!'s Audio/Visual Offset Wizard + Latency Certifier, but writes
//! into DeadSync's existing offsets:
//!   * audio / judgment offset -> `config.global_offset_seconds`
//!   * visual offset           -> `config.visual_delay_seconds`
//!
//! The tool runs a short phase machine:
//!   `Intro -> AudioWizard -> VisualWizard -> Certifier -> Results`.
//! Each wizard plays a steady metronome (audio click or visual flash) on the
//! audio master-clock timeline and collects anticipatory taps; the mean tap
//! error maps to the offset that cancels it. The certifier is a report-only
//! reaction test that isolates audio-vs-visual hardware skew.

mod measure;
mod render;

use crate::config;
use crate::screens::components::shared::visual_style_bg;
use crate::screens::components::shared::transitions;
use crate::screens::{Screen, ScreenAction};
use deadsync_audio_stream as audio;
use deadsync_input::{InputEvent, VirtualAction};
use deadsync_present::actors::Actor;
use deadsync_present::color;
use std::time::Instant;

const TRANSITION_IN_DURATION: f32 = 0.4;
const TRANSITION_OUT_DURATION: f32 = 0.4;

const CLICK_PATH: &str = "assets/sounds/assist_tick.ogg";

/// Offset clamp (matches the Sound/Graphics submenu bounds: +/-1000 ms).
const OFFSET_CLAMP_MS: i32 = 1000;

/// Lead-in before the first beat so the player can settle after the fade.
const PREP_NS: i128 = 1_000_000_000;
/// Half-width of the visual flash envelope around a beat.
const FLASH_HALF_WIDTH_NS: i128 = 90_000_000;

/// Certifier inter-stimulus interval bounds.
const CERT_MIN_GAP_NS: i128 = 900_000_000;
const CERT_MAX_GAP_NS: i128 = 1_900_000_000;
/// Reaction times outside this window are discarded (false start / missed).
const CERT_MIN_REACTION_NS: i128 = 80_000_000;
const CERT_MAX_REACTION_NS: i128 = 700_000_000;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Phase {
    Intro,
    AudioWizard,
    VisualWizard,
    Certifier,
    Results,
}

/// A single metronome measurement pass (audio clicks or visual flashes).
///
/// Everything runs on a single `Instant` timeline anchored at the run start, so
/// it works even with no song playing (the music stream clock is pinned at zero
/// outside gameplay). Audio clicks are emitted immediately as each beat arrives
/// via the assist-tick SFX lane; the player taps to the perceived stimulus and
/// the mean error vs the grid is the perceptual offset.
struct WizardRun {
    epoch_instant: Instant,
    period_ns: i128,
    /// Next metronome beat index to emit.
    next_emit_k: i64,
    /// Raw nearest-beat errors, in timeline nanoseconds.
    samples: Vec<i128>,
    audio: bool,
}

/// Outcome of a finished wizard pass.
#[derive(Clone, Copy)]
struct WizardResult {
    suggested_offset_seconds: f32,
    mean_error_ns: i128,
    stddev_seconds: f32,
    gate_ok: bool,
    scored: usize,
}

/// One reaction stimulus awaiting a tap.
#[derive(Clone, Copy)]
struct Stimulus {
    at_ns: i128,
    audio: bool,
}

/// Reaction-test pass for the certifier.
struct CertRun {
    epoch_instant: Instant,
    rng: u64,
    pending: Option<Stimulus>,
    next_stimulus_at_ns: i128,
    next_is_audio: bool,
    audio_samples: Vec<i128>,
    visual_samples: Vec<i128>,
}

/// Final certifier report (report-only; never auto-applied).
#[derive(Clone, Copy, Default)]
struct CertResult {
    audio_mean_ns: i128,
    audio_count: usize,
    visual_mean_ns: i128,
    visual_count: usize,
}

pub struct State {
    pub active_color_index: i32,
    bg: visual_style_bg::State,
    phase: Phase,
    /// Pre-seed hint from output telemetry (ns), shown on the intro.
    estimated_output_delay_ns: u64,
    wizard: Option<WizardRun>,
    cert: Option<CertRun>,
    audio_result: Option<WizardResult>,
    visual_result: Option<WizardResult>,
    cert_result: Option<CertResult>,
}

pub fn init() -> State {
    State {
        active_color_index: color::DEFAULT_COLOR_INDEX,
        bg: visual_style_bg::State::new(),
        phase: Phase::Intro,
        estimated_output_delay_ns: 0,
        wizard: None,
        cert: None,
        audio_result: None,
        visual_result: None,
        cert_result: None,
    }
}

pub fn on_enter(state: &mut State) {
    state.phase = Phase::Intro;
    state.wizard = None;
    state.cert = None;
    state.audio_result = None;
    state.visual_result = None;
    state.cert_result = None;
    state.estimated_output_delay_ns = audio::get_output_timing_snapshot().estimated_output_delay_ns;
    audio::preload_sfx(CLICK_PATH);
}

pub fn update(state: &mut State, _dt: f32) -> Option<ScreenAction> {
    match state.phase {
        Phase::AudioWizard | Phase::VisualWizard => update_wizard(state),
        Phase::Certifier => update_certifier(state),
        _ => {}
    }
    None
}

pub fn handle_input(state: &mut State, ev: &InputEvent) -> ScreenAction {
    if !ev.pressed {
        return ScreenAction::None;
    }
    if matches!(ev.action, VirtualAction::p1_back | VirtualAction::p2_back) {
        return ScreenAction::Navigate(Screen::Options);
    }
    let is_start = matches!(ev.action, VirtualAction::p1_start | VirtualAction::p2_start);
    let is_tap = is_pad_tap(ev.action);

    match state.phase {
        Phase::Intro => {
            if is_start {
                start_wizard(state, true);
                state.phase = Phase::AudioWizard;
            }
        }
        Phase::AudioWizard | Phase::VisualWizard => {
            if is_tap {
                record_wizard_tap(state, ev);
            }
        }
        Phase::Certifier => {
            if is_tap {
                record_cert_tap(state, ev);
            }
        }
        Phase::Results => {
            if is_start {
                commit_results(state);
                return ScreenAction::Navigate(Screen::Options);
            }
        }
    }
    ScreenAction::None
}

pub fn in_transition() -> (Vec<Actor>, f32) {
    transitions::fade_in_black(TRANSITION_IN_DURATION, 1100)
}

pub fn out_transition() -> (Vec<Actor>, f32) {
    transitions::fade_out_black(TRANSITION_OUT_DURATION, 1200)
}

pub fn push_actors(actors: &mut Vec<Actor>, state: &State, alpha_mul: f32) {
    render::push_actors(actors, state, alpha_mul);
}

pub fn get_actors(state: &State, alpha_mul: f32) -> Vec<Actor> {
    let mut actors = Vec::with_capacity(32);
    render::push_actors(&mut actors, state, alpha_mul);
    actors
}

// ---------------------------------------------------------------------------
// Wizard logic
// ---------------------------------------------------------------------------

#[inline(always)]
fn is_pad_tap(action: VirtualAction) -> bool {
    matches!(
        action,
        VirtualAction::p1_left
            | VirtualAction::p1_right
            | VirtualAction::p1_up
            | VirtualAction::p1_down
            | VirtualAction::p2_left
            | VirtualAction::p2_right
            | VirtualAction::p2_up
            | VirtualAction::p2_down
    )
}

fn start_wizard(state: &mut State, audio_mode: bool) {
    let period_ns = measure::seconds_to_ns(measure::WIZARD_PERIOD_SECONDS);
    // First beat at least PREP_NS in the future, aligned to the grid origin
    // (the run start = timeline zero).
    let first_k = (PREP_NS / period_ns) + 1;
    state.wizard = Some(WizardRun {
        epoch_instant: Instant::now(),
        period_ns,
        next_emit_k: first_k as i64,
        samples: Vec::with_capacity(measure::WIZARD_SAMPLE_COUNT),
        audio: audio_mode,
    });
}

fn update_wizard(state: &mut State) {
    let Some(run) = state.wizard.as_mut() else {
        return;
    };
    if run.audio {
        emit_due_audio_beats(run);
    }
    if run.samples.len() >= measure::WIZARD_SAMPLE_COUNT {
        let result = finish_wizard(run);
        let was_audio = run.audio;
        state.wizard = None;
        if was_audio {
            state.audio_result = Some(result);
            start_wizard(state, false);
            state.phase = Phase::VisualWizard;
        } else {
            state.visual_result = Some(result);
            start_certifier(state);
            state.phase = Phase::Certifier;
        }
    }
}

/// Emit any metronome clicks whose beat time has arrived. Clicks play
/// immediately on the assist-tick lane; the player hears them
/// `output_delay` later, and that delay is exactly what the mean tap error
/// captures.
fn emit_due_audio_beats(run: &mut WizardRun) {
    let now = now_ns(run.epoch_instant);
    loop {
        let beat_ns = run.next_emit_k as i128 * run.period_ns;
        if beat_ns > now {
            break;
        }
        audio::play_preloaded_assist_tick(CLICK_PATH);
        run.next_emit_k += 1;
    }
}

fn record_wizard_tap(state: &mut State, ev: &InputEvent) {
    let Some(run) = state.wizard.as_mut() else {
        return;
    };
    if run.samples.len() >= measure::WIZARD_SAMPLE_COUNT {
        return;
    }
    let tap = tap_ns(run.epoch_instant, ev);
    let err = measure::nearest_beat_error_ns(tap, run.period_ns);
    if err.abs() * 2 <= run.period_ns {
        run.samples.push(err);
    }
}

fn finish_wizard(run: &WizardRun) -> WizardResult {
    let scored = measure::scored_samples(&run.samples);
    let mean = measure::mean_ns(scored);
    let stddev = measure::stddev_seconds(scored, mean);
    WizardResult {
        suggested_offset_seconds: measure::suggested_offset_seconds(mean, OFFSET_CLAMP_MS),
        mean_error_ns: mean,
        stddev_seconds: stddev,
        gate_ok: !scored.is_empty() && measure::within_stddev_gate(stddev),
        scored: scored.len(),
    }
}

// ---------------------------------------------------------------------------
// Certifier logic
// ---------------------------------------------------------------------------

fn start_certifier(state: &mut State) {
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15)
        | 1;
    state.cert = Some(CertRun {
        epoch_instant: Instant::now(),
        rng: seed,
        pending: None,
        next_stimulus_at_ns: PREP_NS,
        next_is_audio: true,
        audio_samples: Vec::with_capacity(measure::CERTIFIER_SAMPLE_COUNT),
        visual_samples: Vec::with_capacity(measure::CERTIFIER_SAMPLE_COUNT),
    });
}

fn update_certifier(state: &mut State) {
    let Some(cert) = state.cert.as_mut() else {
        return;
    };
    if cert_done(cert) {
        let result = CertResult {
            audio_mean_ns: measure::mean_ns(&cert.audio_samples),
            audio_count: cert.audio_samples.len(),
            visual_mean_ns: measure::mean_ns(&cert.visual_samples),
            visual_count: cert.visual_samples.len(),
        };
        state.cert = None;
        state.cert_result = Some(result);
        state.phase = Phase::Results;
        return;
    }

    let now = now_ns(cert.epoch_instant);
    // Fire the next stimulus the moment its time arrives. Audio plays
    // immediately on the assist-tick lane; the visual flash is rendered from
    // `pending`. Onset time = `now`, so reaction = tap - onset.
    if cert.pending.is_none() && now >= cert.next_stimulus_at_ns {
        let audio_stim = cert.next_is_audio;
        cert.pending = Some(Stimulus { at_ns: now, audio: audio_stim });
        if audio_stim {
            audio::play_preloaded_assist_tick(CLICK_PATH);
        }
    }
}

fn record_cert_tap(state: &mut State, ev: &InputEvent) {
    let Some(cert) = state.cert.as_mut() else {
        return;
    };
    let Some(stim) = cert.pending else {
        return;
    };
    let tap = tap_ns(cert.epoch_instant, ev);
    let reaction = tap - stim.at_ns;
    if reaction >= CERT_MIN_REACTION_NS && reaction <= CERT_MAX_REACTION_NS {
        if stim.audio {
            cert.audio_samples.push(reaction);
        } else {
            cert.visual_samples.push(reaction);
        }
    }
    // Whether hit, false-started, or missed-window, advance to the next stimulus.
    cert.pending = None;
    let gap = CERT_MIN_GAP_NS + (next_rand(&mut cert.rng) % (CERT_MAX_GAP_NS - CERT_MIN_GAP_NS) as u64) as i128;
    cert.next_stimulus_at_ns = tap.max(stim.at_ns) + gap;
    cert.next_is_audio = !stim.audio;
}

#[inline(always)]
fn cert_done(cert: &CertRun) -> bool {
    cert.audio_samples.len() >= measure::CERTIFIER_SAMPLE_COUNT
        && cert.visual_samples.len() >= measure::CERTIFIER_SAMPLE_COUNT
}

// ---------------------------------------------------------------------------
// Commit
// ---------------------------------------------------------------------------

fn commit_results(state: &State) {
    if let Some(r) = state.audio_result {
        if r.gate_ok {
            config::update_global_offset(r.suggested_offset_seconds);
        }
    }
    if let Some(r) = state.visual_result {
        if r.gate_ok {
            config::update_visual_delay_seconds(r.suggested_offset_seconds);
        }
    }
}

// ---------------------------------------------------------------------------
// Timebase helpers
// ---------------------------------------------------------------------------

/// Current time on the run timeline (zero == the snapshot moment at run start).
#[inline(always)]
fn now_ns(epoch_instant: Instant) -> i128 {
    Instant::now().saturating_duration_since(epoch_instant).as_nanos() as i128
}

/// Convert an input event to run-timeline nanoseconds via its `Instant` stamp.
#[inline(always)]
fn tap_ns(epoch_instant: Instant, ev: &InputEvent) -> i128 {
    ev.timestamp
        .saturating_duration_since(epoch_instant)
        .as_nanos() as i128
}

/// xorshift64 PRNG (no external crates).
#[inline(always)]
fn next_rand(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

// ---------------------------------------------------------------------------
// Render accessors (read by `render.rs`)
// ---------------------------------------------------------------------------

impl State {
    pub(super) fn phase(&self) -> Phase {
        self.phase
    }

    pub(super) fn estimated_output_delay_ms(&self) -> f32 {
        self.estimated_output_delay_ns as f32 / 1_000_000.0
    }

    /// Number of taps collected so far in the active wizard, and the target.
    pub(super) fn wizard_progress(&self) -> (usize, usize) {
        match &self.wizard {
            Some(run) => (run.samples.len(), measure::WIZARD_SAMPLE_COUNT),
            None => (0, measure::WIZARD_SAMPLE_COUNT),
        }
    }

    /// Visual flash intensity (0..1) for the active visual wizard, else 0.
    pub(super) fn flash_intensity(&self) -> f32 {
        match &self.wizard {
            Some(run) if !run.audio => beat_flash(run.epoch_instant, run.period_ns),
            _ => 0.0,
        }
    }

    /// Audio-wizard beat pulse (0..1) for an on-screen metronome indicator.
    pub(super) fn audio_pulse(&self) -> f32 {
        match &self.wizard {
            Some(run) if run.audio => beat_flash(run.epoch_instant, run.period_ns),
            _ => 0.0,
        }
    }

    /// Certifier: intensity of an active visual stimulus, and whether the
    /// pending stimulus is the audio modality.
    pub(super) fn cert_stimulus(&self) -> (f32, bool) {
        match &self.cert {
            Some(cert) => match cert.pending {
                Some(stim) if !stim.audio => {
                    let now = now_ns(cert.epoch_instant);
                    let on = now >= stim.at_ns && now - stim.at_ns <= CERT_MAX_REACTION_NS;
                    (if on { 1.0 } else { 0.0 }, false)
                }
                Some(_) => (0.0, true),
                None => (0.0, false),
            },
            None => (0.0, false),
        }
    }

    pub(super) fn cert_progress(&self) -> (usize, usize, usize) {
        match &self.cert {
            Some(cert) => (
                cert.audio_samples.len(),
                cert.visual_samples.len(),
                measure::CERTIFIER_SAMPLE_COUNT,
            ),
            None => (0, 0, measure::CERTIFIER_SAMPLE_COUNT),
        }
    }

    pub(super) fn results_lines(&self) -> ResultsView {
        ResultsView {
            audio: self.audio_result.map(|r| OffsetLine {
                suggested_ms: r.suggested_offset_seconds * 1000.0,
                mean_ms: measure::ns_to_seconds(r.mean_error_ns) as f32 * 1000.0,
                stddev_ms: r.stddev_seconds * 1000.0,
                gate_ok: r.gate_ok,
                scored: r.scored,
            }),
            visual: self.visual_result.map(|r| OffsetLine {
                suggested_ms: r.suggested_offset_seconds * 1000.0,
                mean_ms: measure::ns_to_seconds(r.mean_error_ns) as f32 * 1000.0,
                stddev_ms: r.stddev_seconds * 1000.0,
                gate_ok: r.gate_ok,
                scored: r.scored,
            }),
            cert: self.cert_result.map(|c| CertLine {
                audio_ms: measure::ns_to_seconds(c.audio_mean_ns) as f32 * 1000.0,
                audio_count: c.audio_count,
                visual_ms: measure::ns_to_seconds(c.visual_mean_ns) as f32 * 1000.0,
                visual_count: c.visual_count,
                skew_ms: measure::ns_to_seconds(c.audio_mean_ns - c.visual_mean_ns) as f32 * 1000.0,
            }),
        }
    }
}

pub(super) struct OffsetLine {
    pub suggested_ms: f32,
    pub mean_ms: f32,
    pub stddev_ms: f32,
    pub gate_ok: bool,
    pub scored: usize,
}

pub(super) struct CertLine {
    pub audio_ms: f32,
    pub audio_count: usize,
    pub visual_ms: f32,
    pub visual_count: usize,
    pub skew_ms: f32,
}

pub(super) struct ResultsView {
    pub audio: Option<OffsetLine>,
    pub visual: Option<OffsetLine>,
    pub cert: Option<CertLine>,
}

/// Triangular flash envelope peaking on each beat of the grid.
#[inline(always)]
fn beat_flash(epoch_instant: Instant, period_ns: i128) -> f32 {
    if period_ns <= 0 {
        return 0.0;
    }
    let now = now_ns(epoch_instant);
    let d = measure::nearest_beat_error_ns(now, period_ns).abs();
    if d >= FLASH_HALF_WIDTH_NS {
        0.0
    } else {
        1.0 - d as f32 / FLASH_HALF_WIDTH_NS as f32
    }
}
