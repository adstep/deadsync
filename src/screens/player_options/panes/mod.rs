use crate::assets::i18n::tr;
use crate::game::profile::{
    AttackMode, BackgroundFilter, ComboColors, ComboFont, ComboMode, DataVisualizations,
    ErrorBarTrim, HideLightType, LifeMeterType, MeasureCounter, MeasureLines, MiniIndicator,
    MiniIndicatorScoreType, Perspective, TargetScoreSetting, TimingWindowsOption, TurnOption,
};
use crate::game::song::SongData;
use crate::screens::Screen;

use super::*;

pub fn hud_offset_choices() -> Vec<String> {
    (HUD_OFFSET_MIN..=HUD_OFFSET_MAX)
        .map(|v| v.to_string())
        .collect()
}

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

pub fn choose_different_screen_label(return_screen: Screen) -> String {
    match return_screen {
        Screen::SelectCourse => tr("PlayerOptions", "ChooseDifferentCourse").to_string(),
        _ => tr("PlayerOptions", "ChooseDifferentSong").to_string(),
    }
}

pub fn what_comes_next_choices(pane: OptionsPane, return_screen: Screen) -> Vec<String> {
    let choose_different = choose_different_screen_label(return_screen);
    match pane {
        OptionsPane::Main => vec![
            tr("PlayerOptions", "WhatComesNextGameplay").to_string(),
            choose_different,
            tr("PlayerOptions", "WhatComesNextAdvancedModifiers").to_string(),
            tr("PlayerOptions", "WhatComesNextUncommonModifiers").to_string(),
        ],
        OptionsPane::Advanced => vec![
            tr("PlayerOptions", "WhatComesNextGameplay").to_string(),
            choose_different,
            tr("PlayerOptions", "WhatComesNextMainModifiers").to_string(),
            tr("PlayerOptions", "WhatComesNextUncommonModifiers").to_string(),
        ],
        OptionsPane::Uncommon => vec![
            tr("PlayerOptions", "WhatComesNextGameplay").to_string(),
            choose_different,
            tr("PlayerOptions", "WhatComesNextMainModifiers").to_string(),
            tr("PlayerOptions", "WhatComesNextAdvancedModifiers").to_string(),
        ],
    }
}


mod main;
mod advanced;
mod uncommon;

pub use main::build_main_rows;
pub use advanced::build_advanced_rows;
pub use uncommon::build_uncommon_rows;

pub fn build_rows(
    song: &SongData,
    speed_mod: &SpeedMod,
    chart_steps_index: [usize; PLAYER_SLOTS],
    preferred_difficulty_index: [usize; PLAYER_SLOTS],
    session_music_rate: f32,
    pane: OptionsPane,
    noteskin_names: &[String],
    return_screen: Screen,
    fixed_stepchart: Option<&FixedStepchart>,
) -> Vec<Row> {
    match pane {
        OptionsPane::Main => build_main_rows(
            song,
            speed_mod,
            chart_steps_index,
            preferred_difficulty_index,
            session_music_rate,
            noteskin_names,
            return_screen,
            fixed_stepchart,
        ),
        OptionsPane::Advanced => build_advanced_rows(return_screen),
        OptionsPane::Uncommon => build_uncommon_rows(return_screen),
    }
}

