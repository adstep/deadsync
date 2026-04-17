use crate::assets::i18n::{lookup_key, tr};
use crate::game::song::SongData;
use crate::screens::Screen;

use super::*;

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

pub fn build_main_rows(
    song: &SongData,
    speed_mod: &SpeedMod,
    chart_steps_index: [usize; PLAYER_SLOTS],
    preferred_difficulty_index: [usize; PLAYER_SLOTS],
    session_music_rate: f32,
    noteskin_names: &[String],
    return_screen: Screen,
    fixed_stepchart: Option<&FixedStepchart>,
) -> Vec<Row> {
    let speed_mod_value_str = match speed_mod.mod_type.as_str() {
        "X" => format!("{:.2}x", speed_mod.value),
        "C" => format!("C{}", speed_mod.value as i32),
        "M" => format!("M{}", speed_mod.value as i32),
        _ => String::new(),
    };
    let (stepchart_choices, stepchart_choice_indices, initial_stepchart_choice_index) =
        if let Some(fixed) = fixed_stepchart {
            let fixed_steps_idx = chart_steps_index[session_persisted_player_idx()];
            (
                vec![fixed.label.clone()],
                vec![fixed_steps_idx],
                [0; PLAYER_SLOTS],
            )
        } else {
            // Build Stepchart choices from the song's charts for the current play style, ordered
            // Beginner..Challenge, then Edit charts.
            let target_chart_type = crate::game::profile::get_session_play_style().chart_type();
            let mut stepchart_choices: Vec<String> = Vec::with_capacity(5);
            let mut stepchart_choice_indices: Vec<usize> = Vec::with_capacity(5);
            for (i, file_name) in crate::engine::present::color::FILE_DIFFICULTY_NAMES
                .iter()
                .enumerate()
            {
                if let Some(chart) = song.charts.iter().find(|c| {
                    c.chart_type.eq_ignore_ascii_case(target_chart_type)
                        && c.difficulty.eq_ignore_ascii_case(file_name)
                }) {
                    let display_name = difficulty_display_name(i);
                    stepchart_choices.push(format!("{} {}", display_name, chart.meter));
                    stepchart_choice_indices.push(i);
                }
            }
            for (i, chart) in
                crate::screens::select_music::edit_charts_sorted(song, target_chart_type)
                    .into_iter()
                    .enumerate()
            {
                let desc = chart.description.trim();
                if desc.is_empty() {
                    stepchart_choices.push(
                        tr_fmt(
                            "PlayerOptions",
                            "EditChartMeter",
                            &[("meter", &chart.meter.to_string())],
                        )
                        .to_string(),
                    );
                } else {
                    stepchart_choices.push(
                        tr_fmt(
                            "PlayerOptions",
                            "EditChartDescMeter",
                            &[("desc", desc), ("meter", &chart.meter.to_string())],
                        )
                        .to_string(),
                    );
                }
                stepchart_choice_indices
                    .push(crate::engine::present::color::FILE_DIFFICULTY_NAMES.len() + i);
            }
            // Fallback if none found (defensive; SelectMusic filters songs by play style).
            if stepchart_choices.is_empty() {
                stepchart_choices.push(tr("PlayerOptions", "CurrentStepchartLabel").to_string());
                let base_pref = preferred_difficulty_index[session_persisted_player_idx()].min(
                    crate::engine::present::color::FILE_DIFFICULTY_NAMES
                        .len()
                        .saturating_sub(1),
                );
                stepchart_choice_indices.push(base_pref);
            }
            let initial_stepchart_choice_index: [usize; PLAYER_SLOTS] =
                std::array::from_fn(|player_idx| {
                    let steps_idx = chart_steps_index[player_idx];
                    let pref_idx = preferred_difficulty_index[player_idx].min(
                        crate::engine::present::color::FILE_DIFFICULTY_NAMES
                            .len()
                            .saturating_sub(1),
                    );
                    stepchart_choice_indices
                        .iter()
                        .position(|&idx| idx == steps_idx)
                        .or_else(|| {
                            stepchart_choice_indices
                                .iter()
                                .position(|&idx| idx == pref_idx)
                        })
                        .unwrap_or(0)
                });
            (
                stepchart_choices,
                stepchart_choice_indices,
                initial_stepchart_choice_index,
            )
        };
    vec![
        Row {
            id: RowId::TypeOfSpeedMod,
            name: lookup_key("PlayerOptions", "TypeOfSpeedMod"),
            choices: vec![
                tr("PlayerOptions", "SpeedModTypeX").to_string(),
                tr("PlayerOptions", "SpeedModTypeC").to_string(),
                tr("PlayerOptions", "SpeedModTypeM").to_string(),
            ],
            selected_choice_index: [match speed_mod.mod_type.as_str() {
                "X" => 0,
                "C" => 1,
                "M" => 2,
                _ => 1, // Default to C
            }; PLAYER_SLOTS],
            help: tr("PlayerOptionsHelp", "TypeOfSpeedModHelp")
                .split("\\n")
                .map(|s| s.to_string())
                .collect(),
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::SpeedMod,
            name: lookup_key("PlayerOptions", "SpeedMod"),
            choices: vec![speed_mod_value_str], // Display only the current value
            selected_choice_index: [0; PLAYER_SLOTS],
            help: tr("PlayerOptionsHelp", "SpeedModHelp")
                .split("\\n")
                .map(|s| s.to_string())
                .collect(),
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::Mini,
            name: lookup_key("PlayerOptions", "Mini"),
            choices: (-100..=150).map(|v| format!("{v}%")).collect(),
            selected_choice_index: [0; PLAYER_SLOTS],
            help: tr("PlayerOptionsHelp", "MiniHelp")
                .split("\\n")
                .map(|s| s.to_string())
                .collect(),
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::Perspective,
            name: lookup_key("PlayerOptions", "Perspective"),
            choices: vec![
                tr("PlayerOptions", "PerspectiveOverhead").to_string(),
                tr("PlayerOptions", "PerspectiveHallway").to_string(),
                tr("PlayerOptions", "PerspectiveDistant").to_string(),
                tr("PlayerOptions", "PerspectiveIncoming").to_string(),
                tr("PlayerOptions", "PerspectiveSpace").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: tr("PlayerOptionsHelp", "PerspectiveHelp")
                .split("\\n")
                .map(|s| s.to_string())
                .collect(),
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::NoteSkin,
            name: lookup_key("PlayerOptions", "NoteSkin"),
            choices: if noteskin_names.is_empty() {
                vec![crate::game::profile::NoteSkin::DEFAULT_NAME.to_string()]
            } else {
                noteskin_names.to_vec()
            },
            selected_choice_index: [0; PLAYER_SLOTS],
            help: tr("PlayerOptionsHelp", "NoteSkinHelp")
                .split("\\n")
                .map(|s| s.to_string())
                .collect(),
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::MineSkin,
            name: lookup_key("PlayerOptions", "MineSkin"),
            choices: build_noteskin_override_choices(noteskin_names),
            selected_choice_index: [0; PLAYER_SLOTS],
            help: tr("PlayerOptionsHelp", "MineSkinHelp")
                .split("\\n")
                .map(|s| s.to_string())
                .collect(),
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::ReceptorSkin,
            name: lookup_key("PlayerOptions", "ReceptorSkin"),
            choices: build_noteskin_override_choices(noteskin_names),
            selected_choice_index: [0; PLAYER_SLOTS],
            help: tr("PlayerOptionsHelp", "ReceptorSkinHelp")
                .split("\\n")
                .map(|s| s.to_string())
                .collect(),
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::TapExplosionSkin,
            name: lookup_key("PlayerOptions", "TapExplosionSkin"),
            choices: build_tap_explosion_noteskin_choices(noteskin_names),
            selected_choice_index: [0; PLAYER_SLOTS],
            help: tr("PlayerOptionsHelp", "TapExplosionSkinHelp")
                .split("\\n")
                .map(|s| s.to_string())
                .collect(),
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::JudgmentFont,
            name: lookup_key("PlayerOptions", "JudgmentFont"),
            choices: assets::judgment_texture_choices()
                .iter()
                .map(|choice| choice.label.clone())
                .collect(),
            selected_choice_index: [0; PLAYER_SLOTS],
            help: tr("PlayerOptionsHelp", "JudgmentFontHelp")
                .split("\\n")
                .map(|s| s.to_string())
                .collect(),
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::JudgmentOffsetX,
            name: lookup_key("PlayerOptions", "JudgmentOffsetX"),
            choices: hud_offset_choices(),
            selected_choice_index: [HUD_OFFSET_ZERO_INDEX; PLAYER_SLOTS],
            help: tr("PlayerOptionsHelp", "JudgmentOffsetXHelp")
                .split("\\n")
                .map(|s| s.to_string())
                .collect(),
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::JudgmentOffsetY,
            name: lookup_key("PlayerOptions", "JudgmentOffsetY"),
            choices: hud_offset_choices(),
            selected_choice_index: [HUD_OFFSET_ZERO_INDEX; PLAYER_SLOTS],
            help: tr("PlayerOptionsHelp", "JudgmentOffsetYHelp")
                .split("\\n")
                .map(|s| s.to_string())
                .collect(),
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::ComboFont,
            name: lookup_key("PlayerOptions", "ComboFont"),
            choices: vec![
                tr("PlayerOptions", "ComboFontWendy").to_string(),
                tr("PlayerOptions", "ComboFontArialRounded").to_string(),
                tr("PlayerOptions", "ComboFontAsap").to_string(),
                tr("PlayerOptions", "ComboFontBebasNeue").to_string(),
                tr("PlayerOptions", "ComboFontSourceCode").to_string(),
                tr("PlayerOptions", "ComboFontWork").to_string(),
                tr("PlayerOptions", "ComboFontWendyCursed").to_string(),
                tr("PlayerOptions", "ComboFontNone").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: tr("PlayerOptionsHelp", "ComboFontHelp")
                .split("\\n")
                .map(|s| s.to_string())
                .collect(),
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::ComboOffsetX,
            name: lookup_key("PlayerOptions", "ComboOffsetX"),
            choices: hud_offset_choices(),
            selected_choice_index: [HUD_OFFSET_ZERO_INDEX; PLAYER_SLOTS],
            help: tr("PlayerOptionsHelp", "ComboOffsetXHelp")
                .split("\\n")
                .map(|s| s.to_string())
                .collect(),
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::ComboOffsetY,
            name: lookup_key("PlayerOptions", "ComboOffsetY"),
            choices: hud_offset_choices(),
            selected_choice_index: [HUD_OFFSET_ZERO_INDEX; PLAYER_SLOTS],
            help: tr("PlayerOptionsHelp", "ComboOffsetYHelp")
                .split("\\n")
                .map(|s| s.to_string())
                .collect(),
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::HoldJudgment,
            name: lookup_key("PlayerOptions", "HoldJudgment"),
            choices: assets::hold_judgment_texture_choices()
                .iter()
                .map(|choice| choice.label.clone())
                .collect(),
            selected_choice_index: [0; PLAYER_SLOTS],
            help: tr("PlayerOptionsHelp", "HoldJudgmentHelp")
                .split("\\n")
                .map(|s| s.to_string())
                .collect(),
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::BackgroundFilter,
            name: lookup_key("PlayerOptions", "BackgroundFilter"),
            choices: vec![
                tr("PlayerOptions", "BackgroundFilterOff").to_string(),
                tr("PlayerOptions", "BackgroundFilterDark").to_string(),
                tr("PlayerOptions", "BackgroundFilterDarker").to_string(),
                tr("PlayerOptions", "BackgroundFilterDarkest").to_string(),
            ],
            selected_choice_index: [3; PLAYER_SLOTS],
            help: tr("PlayerOptionsHelp", "BackgroundFilterHelp")
                .split("\\n")
                .map(|s| s.to_string())
                .collect(),
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::NoteFieldOffsetX,
            name: lookup_key("PlayerOptions", "NoteFieldOffsetX"),
            choices: (0..=50).map(|v| v.to_string()).collect(),
            selected_choice_index: [0; PLAYER_SLOTS],
            help: tr("PlayerOptionsHelp", "NoteFieldOffsetXHelp")
                .split("\\n")
                .map(|s| s.to_string())
                .collect(),
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::NoteFieldOffsetY,
            name: lookup_key("PlayerOptions", "NoteFieldOffsetY"),
            choices: (-50..=50).map(|v| v.to_string()).collect(),
            selected_choice_index: [0; PLAYER_SLOTS],
            help: tr("PlayerOptionsHelp", "NoteFieldOffsetYHelp")
                .split("\\n")
                .map(|s| s.to_string())
                .collect(),
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::VisualDelay,
            name: lookup_key("PlayerOptions", "VisualDelay"),
            choices: (-100..=100).map(|v| format!("{v}ms")).collect(),
            selected_choice_index: [100; PLAYER_SLOTS],
            help: tr("PlayerOptionsHelp", "VisualDelayHelp")
                .split("\\n")
                .map(|s| s.to_string())
                .collect(),
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::GlobalOffsetShift,
            name: lookup_key("PlayerOptions", "GlobalOffsetShift"),
            choices: (-100..=100).map(|v| format!("{v}ms")).collect(),
            selected_choice_index: [100; PLAYER_SLOTS],
            help: tr("PlayerOptionsHelp", "GlobalOffsetShiftHelp")
                .split("\\n")
                .map(|s| s.to_string())
                .collect(),
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::MusicRate,
            name: lookup_key("PlayerOptions", "MusicRate"),
            choices: vec![fmt_music_rate(session_music_rate.clamp(0.5, 3.0))],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: tr("PlayerOptionsHelp", "MusicRateHelp")
                .split("\\n")
                .map(|s| s.to_string())
                .collect(),
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::Stepchart,
            name: lookup_key("PlayerOptions", "Stepchart"),
            choices: stepchart_choices,
            selected_choice_index: initial_stepchart_choice_index,
            help: tr("PlayerOptionsHelp", "StepchartHelp")
                .split("\\n")
                .map(|s| s.to_string())
                .collect(),
            choice_difficulty_indices: Some(stepchart_choice_indices),
        },
        Row {
            id: RowId::WhatComesNext,
            name: lookup_key("PlayerOptions", "WhatComesNext"),
            choices: what_comes_next_choices(OptionsPane::Main, return_screen),
            selected_choice_index: [0; PLAYER_SLOTS],
            help: tr("PlayerOptionsHelp", "WhatComesNextHelp")
                .split("\\n")
                .map(|s| s.to_string())
                .collect(),
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::Exit,
            name: lookup_key("Common", "Exit"),
            choices: vec![tr("Common", "Exit").to_string()],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![String::new()],
            choice_difficulty_indices: None,
        },
    ]
}

pub fn build_advanced_rows(return_screen: Screen) -> Vec<Row> {
    let mut gameplay_extras_choices = vec![
        tr("PlayerOptions", "GameplayExtrasFlashColumnForMiss").to_string(),
        tr("PlayerOptions", "GameplayExtrasDensityGraphAtTop").to_string(),
        tr("PlayerOptions", "GameplayExtrasColumnCues").to_string(),
    ];
    if crate::game::scores::is_gs_get_scores_service_allowed() {
        gameplay_extras_choices
            .push(tr("PlayerOptions", "GameplayExtrasDisplayScorebox").to_string());
    }

    vec![
        Row {
            id: RowId::Turn,
            name: lookup_key("PlayerOptions", "Turn"),
            choices: vec![
                tr("PlayerOptions", "TurnNone").to_string(),
                tr("PlayerOptions", "TurnMirror").to_string(),
                tr("PlayerOptions", "TurnLeft").to_string(),
                tr("PlayerOptions", "TurnRight").to_string(),
                tr("PlayerOptions", "TurnLRMirror").to_string(),
                tr("PlayerOptions", "TurnUDMirror").to_string(),
                tr("PlayerOptions", "TurnShuffle").to_string(),
                tr("PlayerOptions", "TurnBlender").to_string(),
                tr("PlayerOptions", "TurnRandom").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "TurnHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::Scroll,
            name: lookup_key("PlayerOptions", "Scroll"),
            choices: vec![
                tr("PlayerOptions", "ScrollReverse").to_string(),
                tr("PlayerOptions", "ScrollSplit").to_string(),
                tr("PlayerOptions", "ScrollAlternate").to_string(),
                tr("PlayerOptions", "ScrollCross").to_string(),
                tr("PlayerOptions", "ScrollCentered").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "ScrollHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::Hide,
            name: lookup_key("PlayerOptions", "Hide"),
            choices: vec![
                tr("PlayerOptions", "HideTargets").to_string(),
                tr("PlayerOptions", "HideBackground").to_string(),
                tr("PlayerOptions", "HideCombo").to_string(),
                tr("PlayerOptions", "HideLife").to_string(),
                tr("PlayerOptions", "HideScore").to_string(),
                tr("PlayerOptions", "HideDanger").to_string(),
                tr("PlayerOptions", "HideComboExplosions").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "HideHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::LifeMeterType,
            name: lookup_key("PlayerOptions", "LifeMeterType"),
            choices: vec![
                tr("PlayerOptions", "LifeMeterTypeStandard").to_string(),
                tr("PlayerOptions", "LifeMeterTypeSurround").to_string(),
                tr("PlayerOptions", "LifeMeterTypeVertical").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "LifeMeterTypeHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::LifeBarOptions,
            name: lookup_key("PlayerOptions", "LifeBarOptions"),
            choices: vec![
                tr("PlayerOptions", "LifeBarOptionsRainbowMax").to_string(),
                tr("PlayerOptions", "LifeBarOptionsResponsiveColors").to_string(),
                tr("PlayerOptions", "LifeBarOptionsShowLifePercentage").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "LifeBarOptionsHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::DataVisualizations,
            name: lookup_key("PlayerOptions", "DataVisualizations"),
            choices: vec![
                tr("PlayerOptions", "DataVisualizationsNone").to_string(),
                tr("PlayerOptions", "DataVisualizationsTargetScoreGraph").to_string(),
                tr("PlayerOptions", "DataVisualizationsStepStatistics").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "DataVisualizationsHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::DensityGraphBackground,
            name: lookup_key("PlayerOptions", "DensityGraphBackground"),
            choices: vec![
                tr("PlayerOptions", "DensityGraphBackgroundSolid").to_string(),
                tr("PlayerOptions", "DensityGraphBackgroundTransparent").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "DensityGraphBackgroundHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::TargetScore,
            name: lookup_key("PlayerOptions", "TargetScore"),
            choices: vec![
                tr("PlayerOptions", "TargetScoreCMinus").to_string(),
                tr("PlayerOptions", "TargetScoreC").to_string(),
                tr("PlayerOptions", "TargetScoreCPlus").to_string(),
                tr("PlayerOptions", "TargetScoreBMinus").to_string(),
                tr("PlayerOptions", "TargetScoreB").to_string(),
                tr("PlayerOptions", "TargetScoreBPlus").to_string(),
                tr("PlayerOptions", "TargetScoreAMinus").to_string(),
                tr("PlayerOptions", "TargetScoreA").to_string(),
                tr("PlayerOptions", "TargetScoreAPlus").to_string(),
                tr("PlayerOptions", "TargetScoreSMinus").to_string(),
                tr("PlayerOptions", "TargetScoreS").to_string(),
                tr("PlayerOptions", "TargetScoreSPlus").to_string(),
                tr("PlayerOptions", "TargetScoreMachineBest").to_string(),
                tr("PlayerOptions", "TargetScorePersonalBest").to_string(),
            ],
            selected_choice_index: [10; PLAYER_SLOTS], // S by default
            help: vec![tr("PlayerOptionsHelp", "TargetScoreHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::ActionOnMissedTarget,
            name: lookup_key("PlayerOptions", "TargetScoreMissPolicy"),
            choices: vec![
                tr("PlayerOptions", "TargetScoreMissPolicyNothing").to_string(),
                tr("PlayerOptions", "TargetScoreMissPolicyFail").to_string(),
                tr("PlayerOptions", "TargetScoreMissPolicyRestartSong").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "TargetScoreMissPolicyHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::MiniIndicator,
            name: lookup_key("PlayerOptions", "MiniIndicator"),
            choices: vec![
                tr("PlayerOptions", "MiniIndicatorNone").to_string(),
                tr("PlayerOptions", "MiniIndicatorSubtractiveScoring").to_string(),
                tr("PlayerOptions", "MiniIndicatorPredictiveScoring").to_string(),
                tr("PlayerOptions", "MiniIndicatorPaceScoring").to_string(),
                tr("PlayerOptions", "MiniIndicatorRivalScoring").to_string(),
                tr("PlayerOptions", "MiniIndicatorPacemaker").to_string(),
                tr("PlayerOptions", "MiniIndicatorStreamProg").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "MiniIndicatorHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::IndicatorScoreType,
            name: lookup_key("PlayerOptions", "IndicatorScoreType"),
            choices: vec![
                tr("PlayerOptions", "IndicatorScoreTypeITG").to_string(),
                tr("PlayerOptions", "IndicatorScoreTypeEX").to_string(),
                tr("PlayerOptions", "IndicatorScoreTypeHEX").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "IndicatorScoreTypeHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::GameplayExtras,
            name: lookup_key("PlayerOptions", "GameplayExtras"),
            choices: gameplay_extras_choices,
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "GameplayExtrasHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::ComboColors,
            name: lookup_key("PlayerOptions", "ComboColors"),
            choices: vec![
                tr("PlayerOptions", "ComboColorsGlow").to_string(),
                tr("PlayerOptions", "ComboColorsSolid").to_string(),
                tr("PlayerOptions", "ComboColorsRainbow").to_string(),
                tr("PlayerOptions", "ComboColorsRainbowScroll").to_string(),
                tr("PlayerOptions", "ComboColorsNone").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "ComboColorsHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::ComboColorMode,
            name: lookup_key("PlayerOptions", "ComboColorMode"),
            choices: vec![
                tr("PlayerOptions", "ComboColorModeFullCombo").to_string(),
                tr("PlayerOptions", "ComboColorModeCurrentCombo").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "ComboColorModeHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::CarryCombo,
            name: lookup_key("PlayerOptions", "CarryCombo"),
            choices: vec![
                tr("PlayerOptions", "CarryComboNo").to_string(),
                tr("PlayerOptions", "CarryComboYes").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "CarryComboHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::JudgmentTilt,
            name: lookup_key("PlayerOptions", "JudgmentTilt"),
            choices: vec![
                tr("PlayerOptions", "JudgmentTiltNo").to_string(),
                tr("PlayerOptions", "JudgmentTiltYes").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "JudgmentTiltHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::JudgmentTiltIntensity,
            name: lookup_key("PlayerOptions", "JudgmentTiltIntensity"),
            choices: tilt_intensity_choices(),
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "JudgmentTiltIntensityHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::JudgmentBehindArrows,
            name: lookup_key("PlayerOptions", "JudgmentBehindArrows"),
            choices: vec![
                tr("PlayerOptions", "JudgmentBehindArrowsOff").to_string(),
                tr("PlayerOptions", "JudgmentBehindArrowsOn").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "JudgmentBehindArrowsHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::OffsetIndicator,
            name: lookup_key("PlayerOptions", "OffsetIndicator"),
            choices: vec![
                tr("PlayerOptions", "OffsetIndicatorOff").to_string(),
                tr("PlayerOptions", "OffsetIndicatorOn").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "OffsetIndicatorHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::ErrorBar,
            name: lookup_key("PlayerOptions", "ErrorBar"),
            choices: vec![
                tr("PlayerOptions", "ErrorBarColorful").to_string(),
                tr("PlayerOptions", "ErrorBarMonochrome").to_string(),
                tr("PlayerOptions", "ErrorBarText").to_string(),
                tr("PlayerOptions", "ErrorBarHighlight").to_string(),
                tr("PlayerOptions", "ErrorBarAverage").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "ErrorBarHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::ErrorBarTrim,
            name: lookup_key("PlayerOptions", "ErrorBarTrim"),
            choices: vec![
                tr("PlayerOptions", "ErrorBarTrimOff").to_string(),
                tr("PlayerOptions", "ErrorBarTrimFantastic").to_string(),
                tr("PlayerOptions", "ErrorBarTrimExcellent").to_string(),
                tr("PlayerOptions", "ErrorBarTrimGreat").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "ErrorBarTrimHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::ErrorBarOptions,
            name: lookup_key("PlayerOptions", "ErrorBarOptions"),
            choices: vec![
                tr("PlayerOptions", "ErrorBarOptionsMoveUp").to_string(),
                tr("PlayerOptions", "ErrorBarOptionsMultiTick").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "ErrorBarOptionsHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::ErrorBarOffsetX,
            name: lookup_key("PlayerOptions", "ErrorBarOffsetX"),
            choices: hud_offset_choices(),
            selected_choice_index: [HUD_OFFSET_ZERO_INDEX; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "ErrorBarOffsetXHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::ErrorBarOffsetY,
            name: lookup_key("PlayerOptions", "ErrorBarOffsetY"),
            choices: hud_offset_choices(),
            selected_choice_index: [HUD_OFFSET_ZERO_INDEX; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "ErrorBarOffsetYHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::MeasureCounter,
            name: lookup_key("PlayerOptions", "MeasureCounter"),
            choices: vec![
                tr("PlayerOptions", "MeasureCounterNone").to_string(),
                tr("PlayerOptions", "MeasureCounter8th").to_string(),
                tr("PlayerOptions", "MeasureCounter12th").to_string(),
                tr("PlayerOptions", "MeasureCounter16th").to_string(),
                tr("PlayerOptions", "MeasureCounter24th").to_string(),
                tr("PlayerOptions", "MeasureCounter32nd").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "MeasureCounterHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::MeasureCounterLookahead,
            name: lookup_key("PlayerOptions", "MeasureCounterLookahead"),
            choices: vec![
                tr("PlayerOptions", "MeasureCounterLookahead0").to_string(),
                tr("PlayerOptions", "MeasureCounterLookahead1").to_string(),
                tr("PlayerOptions", "MeasureCounterLookahead2").to_string(),
                tr("PlayerOptions", "MeasureCounterLookahead3").to_string(),
                tr("PlayerOptions", "MeasureCounterLookahead4").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "MeasureCounterLookaheadHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::MeasureCounterOptions,
            name: lookup_key("PlayerOptions", "MeasureCounterOptions"),
            choices: vec![
                tr("PlayerOptions", "MeasureCounterOptionsMoveLeft").to_string(),
                tr("PlayerOptions", "MeasureCounterOptionsMoveUp").to_string(),
                tr("PlayerOptions", "MeasureCounterOptionsVerticalLookahead").to_string(),
                tr("PlayerOptions", "MeasureCounterOptionsBrokenRunTotal").to_string(),
                tr("PlayerOptions", "MeasureCounterOptionsRunTimer").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "MeasureCounterOptionsHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::MeasureLines,
            name: lookup_key("PlayerOptions", "MeasureLines"),
            choices: vec![
                tr("PlayerOptions", "MeasureLinesOff").to_string(),
                tr("PlayerOptions", "MeasureLinesMeasure").to_string(),
                tr("PlayerOptions", "MeasureLinesQuarter").to_string(),
                tr("PlayerOptions", "MeasureLinesEighth").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "MeasureLinesHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::RescoreEarlyHits,
            name: lookup_key("PlayerOptions", "RescoreEarlyHits"),
            choices: vec![
                tr("PlayerOptions", "RescoreEarlyHitsNo").to_string(),
                tr("PlayerOptions", "RescoreEarlyHitsYes").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "RescoreEarlyHitsHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::EarlyDecentWayOffOptions,
            name: lookup_key("PlayerOptions", "EarlyDecentWayOffOptions"),
            choices: vec![
                tr("PlayerOptions", "EarlyDecentWayOffOptionsHideJudgments").to_string(),
                tr(
                    "PlayerOptions",
                    "EarlyDecentWayOffOptionsHideNoteFieldFlash",
                )
                .to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "EarlyDecentWayOffOptionsHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::ResultsExtras,
            name: lookup_key("PlayerOptions", "ResultsExtras"),
            choices: vec![tr("PlayerOptions", "ResultsExtrasTrackEarlyJudgments").to_string()],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "ResultsExtrasHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::TimingWindows,
            name: lookup_key("PlayerOptions", "TimingWindows"),
            choices: vec![
                tr("PlayerOptions", "TimingWindowsNone").to_string(),
                tr("PlayerOptions", "TimingWindowsWayOffs").to_string(),
                tr("PlayerOptions", "TimingWindowsDecentsAndWayOffs").to_string(),
                tr("PlayerOptions", "TimingWindowsFantasticsAndExcellents").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "TimingWindowsHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::FAPlusOptions,
            name: lookup_key("PlayerOptions", "FAPlusOptions"),
            choices: vec![
                tr("PlayerOptions", "FAPlusOptionsDisplayFAPlusWindow").to_string(),
                tr("PlayerOptions", "FAPlusOptionsDisplayEXScore").to_string(),
                tr("PlayerOptions", "FAPlusOptionsDisplayHEXScore").to_string(),
                tr("PlayerOptions", "FAPlusOptionsDisplayFAPlusPane").to_string(),
                tr("PlayerOptions", "FAPlusOptions10msBlueWindow").to_string(),
                tr("PlayerOptions", "FAPlusOptions1510msSplit").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "FAPlusOptionsHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::CustomBlueFantasticWindow,
            name: lookup_key("PlayerOptions", "CustomBlueFantasticWindow"),
            choices: vec![
                tr("PlayerOptions", "CustomBlueFantasticWindowNo").to_string(),
                tr("PlayerOptions", "CustomBlueFantasticWindowYes").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "CustomBlueFantasticWindowHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::CustomBlueFantasticWindowMs,
            name: lookup_key("PlayerOptions", "CustomBlueFantasticWindowMs"),
            choices: custom_fantastic_window_choices(),
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "CustomBlueFantasticWindowMsHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::WhatComesNext,
            name: lookup_key("PlayerOptions", "WhatComesNext"),
            choices: what_comes_next_choices(OptionsPane::Advanced, return_screen),
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "WhatComesNextAdvancedHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::Exit,
            name: lookup_key("Common", "Exit"),
            choices: vec![tr("Common", "Exit").to_string()],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![String::new()],
            choice_difficulty_indices: None,
        },
    ]
}

pub fn build_uncommon_rows(return_screen: Screen) -> Vec<Row> {
    let rows = vec![
        Row {
            id: RowId::Insert,
            name: lookup_key("PlayerOptions", "Insert"),
            choices: vec![
                tr("PlayerOptions", "InsertWide").to_string(),
                tr("PlayerOptions", "InsertBig").to_string(),
                tr("PlayerOptions", "InsertQuick").to_string(),
                tr("PlayerOptions", "InsertBMRize").to_string(),
                tr("PlayerOptions", "InsertSkippy").to_string(),
                tr("PlayerOptions", "InsertEcho").to_string(),
                tr("PlayerOptions", "InsertStomp").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "InsertHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::Remove,
            name: lookup_key("PlayerOptions", "Remove"),
            choices: vec![
                tr("PlayerOptions", "RemoveLittle").to_string(),
                tr("PlayerOptions", "RemoveNoMines").to_string(),
                tr("PlayerOptions", "RemoveNoHolds").to_string(),
                tr("PlayerOptions", "RemoveNoJumps").to_string(),
                tr("PlayerOptions", "RemoveNoHands").to_string(),
                tr("PlayerOptions", "RemoveNoQuads").to_string(),
                tr("PlayerOptions", "RemoveNoLifts").to_string(),
                tr("PlayerOptions", "RemoveNoFakes").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "RemoveHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::Holds,
            name: lookup_key("PlayerOptions", "Holds"),
            choices: vec![
                tr("PlayerOptions", "HoldsPlanted").to_string(),
                tr("PlayerOptions", "HoldsFloored").to_string(),
                tr("PlayerOptions", "HoldsTwister").to_string(),
                tr("PlayerOptions", "HoldsNoRolls").to_string(),
                tr("PlayerOptions", "HoldsToRolls").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "HoldsHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::Accel,
            name: lookup_key("PlayerOptions", "Accel"),
            choices: vec![
                tr("PlayerOptions", "AccelBoost").to_string(),
                tr("PlayerOptions", "AccelBrake").to_string(),
                tr("PlayerOptions", "AccelWave").to_string(),
                tr("PlayerOptions", "AccelExpand").to_string(),
                tr("PlayerOptions", "AccelBoomerang").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "AccelHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::Effect,
            name: lookup_key("PlayerOptions", "Effect"),
            choices: vec![
                tr("PlayerOptions", "EffectDrunk").to_string(),
                tr("PlayerOptions", "EffectDizzy").to_string(),
                tr("PlayerOptions", "EffectConfusion").to_string(),
                tr("PlayerOptions", "EffectBig").to_string(),
                tr("PlayerOptions", "EffectFlip").to_string(),
                tr("PlayerOptions", "EffectInvert").to_string(),
                tr("PlayerOptions", "EffectTornado").to_string(),
                tr("PlayerOptions", "EffectTipsy").to_string(),
                tr("PlayerOptions", "EffectBumpy").to_string(),
                tr("PlayerOptions", "EffectBeat").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "EffectHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::Appearance,
            name: lookup_key("PlayerOptions", "Appearance"),
            choices: vec![
                tr("PlayerOptions", "AppearanceHidden").to_string(),
                tr("PlayerOptions", "AppearanceSudden").to_string(),
                tr("PlayerOptions", "AppearanceStealth").to_string(),
                tr("PlayerOptions", "AppearanceBlink").to_string(),
                tr("PlayerOptions", "AppearanceRVanish").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "AppearanceHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::Attacks,
            name: lookup_key("PlayerOptions", "Attacks"),
            choices: vec![
                tr("PlayerOptions", "AttacksOn").to_string(),
                tr("PlayerOptions", "AttacksRandomAttacks").to_string(),
                tr("PlayerOptions", "AttacksOff").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "AttacksHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::HideLightType,
            name: lookup_key("PlayerOptions", "HideLightType"),
            choices: vec![
                tr("PlayerOptions", "HideLightTypeNoHideLights").to_string(),
                tr("PlayerOptions", "HideLightTypeHideAllLights").to_string(),
                tr("PlayerOptions", "HideLightTypeHideMarqueeLights").to_string(),
                tr("PlayerOptions", "HideLightTypeHideBassLights").to_string(),
            ],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![tr("PlayerOptionsHelp", "HideLightTypeHelp").to_string()],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::WhatComesNext,
            name: lookup_key("PlayerOptions", "WhatComesNext"),
            choices: what_comes_next_choices(OptionsPane::Uncommon, return_screen),
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![
                tr("PlayerOptionsHelp", "WhatComesNextHelp1").to_string(),
                tr("PlayerOptionsHelp", "WhatComesNextHelp2").to_string(),
            ],
            choice_difficulty_indices: None,
        },
        Row {
            id: RowId::Exit,
            name: lookup_key("Common", "Exit"),
            choices: vec![tr("Common", "Exit").to_string()],
            selected_choice_index: [0; PLAYER_SLOTS],
            help: vec![String::new()],
            choice_difficulty_indices: None,
        },
    ];
    rows
}

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
