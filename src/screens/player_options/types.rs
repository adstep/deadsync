use crate::assets::i18n::{LookupKey, tr, tr_fmt};
use crate::assets::AssetManager;
use crate::game::chart::ChartData;
use crate::game::song::SongData;

use super::*;

pub fn hud_offset_choices() -> Vec<String> {
    (HUD_OFFSET_MIN..=HUD_OFFSET_MAX)
        .map(|v| v.to_string())
        .collect()
}

#[derive(Clone, Copy, Debug)]
pub struct RowWindow {
    pub first_start: i32,
    pub first_end: i32,
    pub second_start: i32,
    pub second_end: i32,
}

#[inline(always)]
pub fn compute_row_window(
    total_rows: usize,
    selected_row: [usize; PLAYER_SLOTS],
    active: [bool; PLAYER_SLOTS],
) -> RowWindow {
    if total_rows == 0 {
        return RowWindow {
            first_start: 0,
            first_end: 0,
            second_start: 0,
            second_end: 0,
        };
    }

    let total_rows_i = total_rows as i32;
    if total_rows <= VISIBLE_ROWS {
        return RowWindow {
            first_start: 0,
            first_end: total_rows_i,
            second_start: total_rows_i,
            second_end: total_rows_i,
        };
    }

    let total = VISIBLE_ROWS as i32;
    let halfsize = total / 2;

    // Mirror ITGmania ScreenOptions::PositionRows() semantics (signed math matters).
    let p1_choice = if active[P1] {
        selected_row[P1] as i32
    } else {
        selected_row[P2] as i32
    };
    let p2_choice = if active[P2] {
        selected_row[P2] as i32
    } else {
        selected_row[P1] as i32
    };
    let p1_choice = p1_choice.clamp(0, total_rows_i - 1);
    let p2_choice = p2_choice.clamp(0, total_rows_i - 1);

    let (mut first_start, mut first_end, mut second_start, mut second_end) =
        if active[P1] && active[P2] {
            let earliest = p1_choice.min(p2_choice);
            let first_start = (earliest - halfsize / 2).max(0);
            let first_end = first_start + halfsize;

            let latest = p1_choice.max(p2_choice);
            let second_start = (latest - halfsize / 2).max(0).max(first_end);
            let second_end = second_start + halfsize;
            (first_start, first_end, second_start, second_end)
        } else {
            let first_start = (p1_choice - halfsize).max(0);
            let first_end = first_start + total;
            (first_start, first_end, first_end, first_end)
        };

    first_end = first_end.min(total_rows_i);
    second_end = second_end.min(total_rows_i);

    loop {
        let sum = (first_end - first_start) + (second_end - second_start);
        if sum >= total_rows_i || sum >= total {
            break;
        }
        if second_start > first_end {
            second_start -= 1;
        } else if first_start > 0 {
            first_start -= 1;
        } else if second_end < total_rows_i {
            second_end += 1;
        } else {
            break;
        }
    }

    RowWindow {
        first_start,
        first_end,
        second_start,
        second_end,
    }
}

#[inline(always)]
pub fn row_layout_params() -> (f32, f32) {
    // Must match the geometry in get_actors(): rows align to the help box.
    let frame_h = ROW_HEIGHT;
    let first_row_center_y = screen_center_y() + ROW_START_OFFSET;
    let help_box_h = 40.0_f32;
    let help_box_bottom_y = screen_height() - 36.0;
    let help_top_y = help_box_bottom_y - help_box_h;
    let n_rows_f = VISIBLE_ROWS as f32;
    let mut row_gap = if n_rows_f > 0.0 {
        (n_rows_f - 0.5).mul_add(-frame_h, help_top_y - first_row_center_y) / n_rows_f
    } else {
        0.0
    };
    if !row_gap.is_finite() || row_gap < 0.0 {
        row_gap = 0.0;
    }
    (first_row_center_y, frame_h + row_gap)
}

/* -------------------------- hold-to-scroll timing ------------------------- */
pub const NAV_INITIAL_HOLD_DELAY: Duration = Duration::from_millis(300);
pub const NAV_REPEAT_SCROLL_INTERVAL: Duration = Duration::from_millis(50);

pub const PLAYER_SLOTS: usize = 2;
pub const P1: usize = 0;
pub const P2: usize = 1;

pub const MATCH_NOTESKIN_LABEL: &str = "MatchNoteSkinLabel";
pub const NO_TAP_EXPLOSION_LABEL: &str = "NoTapExplosionLabel";

use crate::game::profile::{
    AttackMode, BackgroundFilter, ComboColors, ComboFont, ComboMode, DataVisualizations,
    ErrorBarTrim, HideLightType, LifeMeterType, MeasureCounter, MeasureLines, MiniIndicator,
    MiniIndicatorScoreType, Perspective, TargetScoreSetting, TimingWindowsOption, TurnOption,
};

/// MiniIndicator variants in row-choice order (index ↔ enum).
pub const MINI_INDICATOR_VARIANTS: [MiniIndicator; 7] = [
    MiniIndicator::None,
    MiniIndicator::SubtractiveScoring,
    MiniIndicator::PredictiveScoring,
    MiniIndicator::PaceScoring,
    MiniIndicator::RivalScoring,
    MiniIndicator::Pacemaker,
    MiniIndicator::StreamProg,
];

pub const TURN_OPTION_VARIANTS: [TurnOption; 9] = [
    TurnOption::None,
    TurnOption::Mirror,
    TurnOption::Left,
    TurnOption::Right,
    TurnOption::LRMirror,
    TurnOption::UDMirror,
    TurnOption::Shuffle,
    TurnOption::Blender,
    TurnOption::Random,
];

pub const BACKGROUND_FILTER_VARIANTS: [BackgroundFilter; 4] = [
    BackgroundFilter::Off,
    BackgroundFilter::Dark,
    BackgroundFilter::Darker,
    BackgroundFilter::Darkest,
];

pub const PERSPECTIVE_VARIANTS: [Perspective; 5] = [
    Perspective::Overhead,
    Perspective::Hallway,
    Perspective::Distant,
    Perspective::Incoming,
    Perspective::Space,
];

pub const COMBO_FONT_VARIANTS: [ComboFont; 8] = [
    ComboFont::Wendy,
    ComboFont::ArialRounded,
    ComboFont::Asap,
    ComboFont::BebasNeue,
    ComboFont::SourceCode,
    ComboFont::Work,
    ComboFont::WendyCursed,
    ComboFont::None,
];

pub const COMBO_COLORS_VARIANTS: [ComboColors; 5] = [
    ComboColors::Glow,
    ComboColors::Solid,
    ComboColors::Rainbow,
    ComboColors::RainbowScroll,
    ComboColors::None,
];

pub const COMBO_MODE_VARIANTS: [ComboMode; 2] = [ComboMode::FullCombo, ComboMode::CurrentCombo];

pub const DATA_VISUALIZATIONS_VARIANTS: [DataVisualizations; 3] = [
    DataVisualizations::None,
    DataVisualizations::TargetScoreGraph,
    DataVisualizations::StepStatistics,
];

pub const TARGET_SCORE_VARIANTS: [TargetScoreSetting; 14] = [
    TargetScoreSetting::CMinus,
    TargetScoreSetting::C,
    TargetScoreSetting::CPlus,
    TargetScoreSetting::BMinus,
    TargetScoreSetting::B,
    TargetScoreSetting::BPlus,
    TargetScoreSetting::AMinus,
    TargetScoreSetting::A,
    TargetScoreSetting::APlus,
    TargetScoreSetting::SMinus,
    TargetScoreSetting::S,
    TargetScoreSetting::SPlus,
    TargetScoreSetting::MachineBest,
    TargetScoreSetting::PersonalBest,
];

pub const LIFE_METER_TYPE_VARIANTS: [LifeMeterType; 3] = [
    LifeMeterType::Standard,
    LifeMeterType::Surround,
    LifeMeterType::Vertical,
];

pub const ERROR_BAR_TRIM_VARIANTS: [ErrorBarTrim; 4] = [
    ErrorBarTrim::Off,
    ErrorBarTrim::Fantastic,
    ErrorBarTrim::Excellent,
    ErrorBarTrim::Great,
];

pub const MEASURE_COUNTER_VARIANTS: [MeasureCounter; 6] = [
    MeasureCounter::None,
    MeasureCounter::Eighth,
    MeasureCounter::Twelfth,
    MeasureCounter::Sixteenth,
    MeasureCounter::TwentyFourth,
    MeasureCounter::ThirtySecond,
];

pub const MEASURE_LINES_VARIANTS: [MeasureLines; 4] = [
    MeasureLines::Off,
    MeasureLines::Measure,
    MeasureLines::Quarter,
    MeasureLines::Eighth,
];

pub const TIMING_WINDOWS_VARIANTS: [TimingWindowsOption; 4] = [
    TimingWindowsOption::None,
    TimingWindowsOption::WayOffs,
    TimingWindowsOption::DecentsAndWayOffs,
    TimingWindowsOption::FantasticsAndExcellents,
];

pub const MINI_INDICATOR_SCORE_TYPE_VARIANTS: [MiniIndicatorScoreType; 3] = [
    MiniIndicatorScoreType::Itg,
    MiniIndicatorScoreType::Ex,
    MiniIndicatorScoreType::HardEx,
];

pub const ATTACK_MODE_VARIANTS: [AttackMode; 3] = [AttackMode::On, AttackMode::Random, AttackMode::Off];

pub const HIDE_LIGHT_TYPE_VARIANTS: [HideLightType; 4] = [
    HideLightType::NoHideLights,
    HideLightType::HideAllLights,
    HideLightType::HideMarqueeLights,
    HideLightType::HideBassLights,
];

#[inline(always)]
pub fn active_player_indices(active: [bool; PLAYER_SLOTS]) -> impl Iterator<Item = usize> {
    [P1, P2]
        .into_iter()
        .filter(move |&player_idx| active[player_idx])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavDirection {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionsPane {
    Main,
    Advanced,
    Uncommon,
}

#[derive(Clone, Copy, Debug)]
pub enum PaneTransition {
    None,
    FadingOut { target: OptionsPane, t: f32 },
    FadingIn { t: f32 },
}

impl PaneTransition {
    #[inline(always)]
    pub fn alpha(self) -> f32 {
        match self {
            Self::None => 1.0,
            Self::FadingOut { t, .. } => (1.0 - t).clamp(0.0, 1.0),
            Self::FadingIn { t } => t.clamp(0.0, 1.0),
        }
    }

    #[inline(always)]
    pub fn is_active(self) -> bool {
        !matches!(self, Self::None)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowId {
    TypeOfSpeedMod,
    SpeedMod,
    Mini,
    Perspective,
    NoteSkin,
    MineSkin,
    ReceptorSkin,
    TapExplosionSkin,
    JudgmentFont,
    JudgmentOffsetX,
    JudgmentOffsetY,
    ComboFont,
    ComboOffsetX,
    ComboOffsetY,
    HoldJudgment,
    BackgroundFilter,
    NoteFieldOffsetX,
    NoteFieldOffsetY,
    VisualDelay,
    GlobalOffsetShift,
    MusicRate,
    Stepchart,
    WhatComesNext,
    Exit,
    // Advanced pane
    Turn,
    Scroll,
    Hide,
    LifeMeterType,
    LifeBarOptions,
    DataVisualizations,
    DensityGraphBackground,
    TargetScore,
    ActionOnMissedTarget,
    MiniIndicator,
    IndicatorScoreType,
    GameplayExtras,
    ComboColors,
    ComboColorMode,
    CarryCombo,
    JudgmentTilt,
    JudgmentTiltIntensity,
    JudgmentBehindArrows,
    OffsetIndicator,
    ErrorBar,
    ErrorBarTrim,
    ErrorBarOptions,
    ErrorBarOffsetX,
    ErrorBarOffsetY,
    MeasureCounter,
    MeasureCounterLookahead,
    MeasureCounterOptions,
    MeasureLines,
    RescoreEarlyHits,
    EarlyDecentWayOffOptions,
    ResultsExtras,
    TimingWindows,
    FAPlusOptions,
    CustomBlueFantasticWindow,
    CustomBlueFantasticWindowMs,
    // Uncommon pane
    Insert,
    Remove,
    Holds,
    Accel,
    Effect,
    Appearance,
    Attacks,
    HideLightType,
    GameplayExtrasMore,
}

pub struct Row {
    pub id: RowId,
    pub name: LookupKey,
    pub choices: Vec<String>,
    pub selected_choice_index: [usize; PLAYER_SLOTS],
    pub help: Vec<String>,
    pub choice_difficulty_indices: Option<Vec<usize>>,
}

#[derive(Clone, Debug)]
pub struct FixedStepchart {
    pub label: String,
}

#[derive(Clone, Debug)]
pub struct SpeedMod {
    pub mod_type: String, // "X", "C", "M"
    pub value: f32,
}

#[inline(always)]
pub fn scroll_speed_for_mod(speed_mod: &SpeedMod) -> crate::game::scroll::ScrollSpeedSetting {
    match speed_mod.mod_type.as_str() {
        "C" => crate::game::scroll::ScrollSpeedSetting::CMod(speed_mod.value),
        "X" => crate::game::scroll::ScrollSpeedSetting::XMod(speed_mod.value),
        "M" => crate::game::scroll::ScrollSpeedSetting::MMod(speed_mod.value),
        _ => crate::game::scroll::ScrollSpeedSetting::default(),
    }
}

#[inline(always)]
pub fn sync_profile_scroll_speed(profile: &mut crate::game::profile::Profile, speed_mod: &SpeedMod) {
    profile.scroll_speed = scroll_speed_for_mod(speed_mod);
}

#[derive(Clone, Copy, Debug)]
pub struct RowTween {
    pub from_y: f32,
    pub to_y: f32,
    pub from_a: f32,
    pub to_a: f32,
    pub t: f32,
}

impl RowTween {
    #[inline(always)]
    pub fn y(&self) -> f32 {
        (self.to_y - self.from_y).mul_add(self.t, self.from_y)
    }

    #[inline(always)]
    pub fn a(&self) -> f32 {
        (self.to_a - self.from_a).mul_add(self.t, self.from_a)
    }
}


pub fn fmt_music_rate(rate: f32) -> String {
    let scaled = (rate * 100.0).round() as i32;
    let int_part = scaled / 100;
    let frac2 = (scaled % 100).abs();
    if frac2 == 0 {
        format!("{int_part}")
    } else if frac2 % 10 == 0 {
        format!("{}.{}", int_part, frac2 / 10)
    } else {
        format!("{int_part}.{frac2:02}")
    }
}

#[inline(always)]
pub fn fmt_tilt_intensity(value: f32) -> String {
    format!("{value:.2}")
}

pub fn tilt_intensity_choices() -> Vec<String> {
    let count =
        ((TILT_INTENSITY_MAX - TILT_INTENSITY_MIN) / TILT_INTENSITY_STEP).round() as usize + 1;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        out.push(fmt_tilt_intensity(
            TILT_INTENSITY_MIN + i as f32 * TILT_INTENSITY_STEP,
        ));
    }
    out
}

pub fn custom_fantastic_window_choices() -> Vec<String> {
    let lo = crate::game::profile::CUSTOM_FANTASTIC_WINDOW_MIN_MS;
    let hi = crate::game::profile::CUSTOM_FANTASTIC_WINDOW_MAX_MS;
    let mut out = Vec::with_capacity((hi - lo + 1) as usize);
    for ms in lo..=hi {
        out.push(format!("{ms}ms"));
    }
    out
}

pub fn resolve_p1_chart<'a>(
    song: &'a SongData,
    chart_steps_index: &[usize; PLAYER_SLOTS],
) -> Option<&'a ChartData> {
    let target_chart_type = crate::game::profile::get_session_play_style().chart_type();
    crate::screens::select_music::chart_for_steps_index(
        song,
        target_chart_type,
        chart_steps_index[0],
    )
}

// Prefer #DISPLAYBPM for reference BPM (use max of range or single value); fallback to song.max_bpm, then 120.
pub fn reference_bpm_for_song(song: &SongData, chart: Option<&ChartData>) -> f32 {
    let bpm = song
        .chart_display_bpm_range(chart)
        .map(|(_, hi)| hi as f32)
        .unwrap_or(song.max_bpm as f32);
    if bpm.is_finite() && bpm > 0.0 {
        bpm
    } else {
        120.0
    }
}

/// Translate a difficulty index (0=Beginner..4=Challenge) to a localized display name.
pub fn difficulty_display_name(index: usize) -> String {
    let key = match index {
        0 => "BeginnerDifficulty",
        1 => "EasyDifficulty",
        2 => "MediumDifficulty",
        3 => "HardDifficulty",
        4 => "ChallengeDifficulty",
        _ => "EditDifficulty",
    };
    tr("SelectCourse", key).to_string()
}

pub fn music_rate_display_name(state: &State) -> String {
    let p1_chart = resolve_p1_chart(&state.song, &state.chart_steps_index);
    let is_random = p1_chart.is_some_and(|c| {
        matches!(
            c.display_bpm,
            Some(crate::game::chart::ChartDisplayBpm::Random)
        )
    });
    let bpm_str = if is_random {
        "???".to_string()
    } else {
        let reference_bpm = reference_bpm_for_song(&state.song, p1_chart);
        let effective_bpm = f64::from(reference_bpm) * f64::from(state.music_rate);
        if (effective_bpm - effective_bpm.round()).abs() < 0.05 {
            format!("{}", effective_bpm.round() as i32)
        } else {
            format!("{effective_bpm:.1}")
        }
    };
    tr_fmt("PlayerOptions", "MusicRate", &[("bpm", &bpm_str)]).replace("\\n", "\n")
}

#[inline(always)]
pub fn display_bpm_pair_for_options(
    song: &SongData,
    chart: Option<&ChartData>,
    music_rate: f32,
) -> Option<(f32, f32)> {
    let rate = if music_rate.is_finite() && music_rate > 0.0 {
        music_rate
    } else {
        1.0
    };
    let (mut lo, mut hi) = song
        .chart_display_bpm_range(chart)
        .map_or((120.0_f32, 120.0_f32), |(a, b)| (a as f32, b as f32));
    if !lo.is_finite() || !hi.is_finite() || lo <= 0.0 || hi <= 0.0 {
        lo = 120.0;
        hi = 120.0;
    }
    Some((lo * rate, hi * rate))
}

#[inline(always)]
pub fn speed_mod_bpm_pair(
    song: &SongData,
    chart: Option<&ChartData>,
    speed_mod: &SpeedMod,
    music_rate: f32,
) -> Option<(f32, f32)> {
    let (mut lo, mut hi) = display_bpm_pair_for_options(song, chart, music_rate)?;
    match speed_mod.mod_type.as_str() {
        "X" => {
            lo *= speed_mod.value;
            hi *= speed_mod.value;
        }
        "M" => {
            if hi.abs() <= f32::EPSILON {
                return None;
            }
            lo *= speed_mod.value / hi;
            hi = speed_mod.value;
        }
        "C" => {
            lo = speed_mod.value;
            hi = speed_mod.value;
        }
        _ => {}
    }
    if lo.is_finite() && hi.is_finite() {
        Some((lo, hi))
    } else {
        None
    }
}

#[inline(always)]
pub fn format_speed_bpm_pair(lo: f32, hi: f32) -> String {
    let lo_i = lo.round() as i32;
    let hi_i = hi.round() as i32;
    if lo_i == hi_i {
        lo_i.to_string()
    } else {
        format!("{lo_i}-{hi_i}")
    }
}

#[inline(always)]
pub fn perspective_speed_mult(perspective: crate::game::profile::Perspective) -> f32 {
    match perspective {
        crate::game::profile::Perspective::Overhead => 1.0,
        crate::game::profile::Perspective::Hallway => 0.75,
        crate::game::profile::Perspective::Distant => 33.0 / 39.0,
        crate::game::profile::Perspective::Incoming => 33.0 / 43.0,
        crate::game::profile::Perspective::Space => 0.825,
    }
}

#[inline(always)]
pub fn speed_mod_helper_scroll_text(
    song: &SongData,
    chart: Option<&ChartData>,
    speed_mod: &SpeedMod,
    music_rate: f32,
) -> String {
    speed_mod_bpm_pair(song, chart, speed_mod, music_rate)
        .map_or_else(String::new, |(lo, hi)| format_speed_bpm_pair(lo, hi))
}

#[inline(always)]
pub fn speed_mod_helper_scaled_text(
    song: &SongData,
    chart: Option<&ChartData>,
    speed_mod: &SpeedMod,
    music_rate: f32,
    profile: &crate::game::profile::Profile,
) -> String {
    let Some((mut lo, mut hi)) = speed_mod_bpm_pair(song, chart, speed_mod, music_rate) else {
        return String::new();
    };
    let mini = profile.mini_percent.clamp(-100, 150) as f32;
    let scale = ((200.0 - mini) / 200.0) * perspective_speed_mult(profile.perspective);
    lo *= scale;
    hi *= scale;
    format_speed_bpm_pair(lo, hi)
}

#[inline(always)]
pub fn measure_wendy_text_width(asset_manager: &AssetManager, text: &str) -> f32 {
    let mut out_w = 1.0_f32;
    asset_manager.with_fonts(|all_fonts| {
        asset_manager.with_font("wendy", |metrics_font| {
            let w = crate::engine::present::font::measure_line_width_logical(
                metrics_font,
                text,
                all_fonts,
            ) as f32;
            if w.is_finite() && w > 0.0 {
                out_w = w;
            }
        });
    });
    out_w
}

#[inline(always)]
pub fn round_to_step(x: f32, step: f32) -> f32 {
    if !x.is_finite() || !step.is_finite() || step <= 0.0 {
        return x;
    }
    (x / step).round() * step
}
