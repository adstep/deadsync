use super::super::*;

const SELECT_PROFILE_BINDING: CycleBinding = CycleBinding::Bool(config::update_machine_show_select_profile);
const SELECT_COLOR_BINDING: CycleBinding = CycleBinding::Bool(config::update_machine_show_select_color);
const SELECT_STYLE_BINDING: CycleBinding = CycleBinding::Bool(config::update_machine_show_select_style);
const PREFERRED_STYLE_BINDING: CycleBinding = CycleBinding::Index(|i| config::update_machine_preferred_style(MachinePreferredPlayStyle::from_choice(i)));
const SELECT_PLAY_MODE_BINDING: CycleBinding = CycleBinding::Bool(config::update_machine_show_select_play_mode);
const PREFERRED_MODE_BINDING: CycleBinding = CycleBinding::Index(|i| config::update_machine_preferred_play_mode(MachinePreferredPlayMode::from_choice(i)));
const FONT_BINDING: CycleBinding = CycleBinding::Index(|i| config::update_machine_font(MachineFont::from_choice(i)));
const EVAL_SUMMARY_BINDING: CycleBinding = CycleBinding::Bool(config::update_machine_show_eval_summary);
const NAME_ENTRY_BINDING: CycleBinding = CycleBinding::Bool(config::update_machine_show_name_entry);
const GAMEOVER_SCREEN_BINDING: CycleBinding = CycleBinding::Bool(config::update_machine_show_gameover);
const WRITE_CURRENT_SCREEN_BINDING: CycleBinding = CycleBinding::Bool(config::update_write_current_screen);
const MENU_MUSIC_BINDING: CycleBinding = CycleBinding::Bool(config::update_menu_music);
const VISUAL_STYLE_BINDING: CycleBinding = CycleBinding::Index(|i| config::update_visual_style(VisualStyle::from_choice(i)));
const REPLAYS_BINDING: CycleBinding = CycleBinding::Bool(config::update_machine_enable_replays);
const PER_PLAYER_GLOBAL_OFFSETS_BINDING: CycleBinding = CycleBinding::Bool(config::update_machine_allow_per_player_global_offsets);
const KEYBOARD_FEATURES_BINDING: CycleBinding = CycleBinding::Bool(config::update_keyboard_features);
const VIDEO_BGS_BINDING: CycleBinding = CycleBinding::Bool(config::update_show_video_backgrounds);

pub(in crate::screens::options) const MACHINE_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: SubRowId::VisualStyle,
        label: lookup_key("OptionsMachine", "VisualStyle"),
        choices: VISUAL_STYLE_CHOICES,
        inline: true,
        behavior: RowBehavior::Cycle(VISUAL_STYLE_BINDING),
    },
    SubRow {
        id: SubRowId::SelectProfile,
        label: lookup_key("OptionsMachine", "SelectProfile"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(SELECT_PROFILE_BINDING),
    },
    SubRow {
        id: SubRowId::SelectColor,
        label: lookup_key("OptionsMachine", "SelectColor"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(SELECT_COLOR_BINDING),
    },
    SubRow {
        id: SubRowId::SelectStyle,
        label: lookup_key("OptionsMachine", "SelectStyle"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(SELECT_STYLE_BINDING),
    },
    SubRow {
        id: SubRowId::PreferredStyle,
        label: lookup_key("OptionsMachine", "PreferredStyle"),
        choices: &[
            localized_choice("OptionsMachine", "PreferredStyleSingle"),
            localized_choice("OptionsMachine", "PreferredStyleVersus"),
            localized_choice("OptionsMachine", "PreferredStyleDouble"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(PREFERRED_STYLE_BINDING),
    },
    SubRow {
        id: SubRowId::SelectPlayMode,
        label: lookup_key("OptionsMachine", "SelectPlayMode"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(SELECT_PLAY_MODE_BINDING),
    },
    SubRow {
        id: SubRowId::PreferredMode,
        label: lookup_key("OptionsMachine", "PreferredMode"),
        choices: &[
            localized_choice("OptionsMachine", "PreferredModeRegular"),
            localized_choice("OptionsMachine", "PreferredModeMarathon"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(PREFERRED_MODE_BINDING),
    },
    SubRow {
        id: SubRowId::Font,
        label: lookup_key("OptionsMachine", "MachineFont"),
        choices: &[
            localized_choice("OptionsMachine", "MachineFontCommon"),
            localized_choice("OptionsMachine", "MachineFontMega"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(FONT_BINDING),
    },
    SubRow {
        id: SubRowId::EvalSummary,
        label: lookup_key("OptionsMachine", "EvalSummary"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(EVAL_SUMMARY_BINDING),
    },
    SubRow {
        id: SubRowId::NameEntry,
        label: lookup_key("OptionsMachine", "NameEntry"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(NAME_ENTRY_BINDING),
    },
    SubRow {
        id: SubRowId::GameoverScreen,
        label: lookup_key("OptionsMachine", "GameoverScreen"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(GAMEOVER_SCREEN_BINDING),
    },
    SubRow {
        id: SubRowId::WriteCurrentScreen,
        label: lookup_key("OptionsMachine", "WriteCurrentScreen"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(WRITE_CURRENT_SCREEN_BINDING),
    },
    SubRow {
        id: SubRowId::MenuMusic,
        label: lookup_key("OptionsMachine", "MenuMusic"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(MENU_MUSIC_BINDING),
    },
    SubRow {
        id: SubRowId::Replays,
        label: lookup_key("OptionsMachine", "Replays"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(REPLAYS_BINDING),
    },
    SubRow {
        id: SubRowId::PerPlayerGlobalOffsets,
        label: lookup_key("OptionsMachine", "PerPlayerGlobalOffsets"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(PER_PLAYER_GLOBAL_OFFSETS_BINDING),
    },
    SubRow {
        id: SubRowId::KeyboardFeatures,
        label: lookup_key("OptionsMachine", "KeyboardFeatures"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(KEYBOARD_FEATURES_BINDING),
    },
    SubRow {
        id: SubRowId::VideoBgs,
        label: lookup_key("OptionsMachine", "VideoBGs"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(VIDEO_BGS_BINDING),
    },
];

pub(in crate::screens::options) const MACHINE_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: ItemId::MchVisualStyle,
        name: lookup_key("OptionsMachine", "VisualStyle"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "VisualStyleHelp",
        ))],
    },
    Item {
        id: ItemId::MchSelectProfile,
        name: lookup_key("OptionsMachine", "SelectProfile"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "SelectProfileHelp",
        ))],
    },
    Item {
        id: ItemId::MchSelectColor,
        name: lookup_key("OptionsMachine", "SelectColor"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "SelectColorHelp",
        ))],
    },
    Item {
        id: ItemId::MchSelectStyle,
        name: lookup_key("OptionsMachine", "SelectStyle"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "SelectStyleHelp",
        ))],
    },
    Item {
        id: ItemId::MchPreferredStyle,
        name: lookup_key("OptionsMachine", "PreferredStyle"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "PreferredStyleHelp",
        ))],
    },
    Item {
        id: ItemId::MchSelectPlayMode,
        name: lookup_key("OptionsMachine", "SelectPlayMode"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "SelectPlayModeHelp",
        ))],
    },
    Item {
        id: ItemId::MchPreferredMode,
        name: lookup_key("OptionsMachine", "PreferredMode"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "PreferredModeHelp",
        ))],
    },
    Item {
        id: ItemId::MchFont,
        name: lookup_key("OptionsMachine", "MachineFont"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "MachineFontHelp",
        ))],
    },
    Item {
        id: ItemId::MchEvalSummary,
        name: lookup_key("OptionsMachine", "EvalSummary"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "EvalSummaryHelp",
        ))],
    },
    Item {
        id: ItemId::MchNameEntry,
        name: lookup_key("OptionsMachine", "NameEntry"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "NameEntryHelp",
        ))],
    },
    Item {
        id: ItemId::MchGameoverScreen,
        name: lookup_key("OptionsMachine", "GameoverScreen"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "GameoverScreenHelp",
        ))],
    },
    Item {
        id: ItemId::MchWriteCurrentScreen,
        name: lookup_key("OptionsMachine", "WriteCurrentScreen"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "WriteCurrentScreenHelp",
        ))],
    },
    Item {
        id: ItemId::MchMenuMusic,
        name: lookup_key("OptionsMachine", "MenuMusic"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "MenuMusicHelp",
        ))],
    },
    Item {
        id: ItemId::MchReplays,
        name: lookup_key("OptionsMachine", "Replays"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "ReplaysHelp",
        ))],
    },
    Item {
        id: ItemId::MchPerPlayerGlobalOffsets,
        name: lookup_key("OptionsMachine", "PerPlayerGlobalOffsets"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "PerPlayerGlobalOffsetsHelp",
        ))],
    },
    Item {
        id: ItemId::MchKeyboardFeatures,
        name: lookup_key("OptionsMachine", "KeyboardFeatures"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "KeyboardFeaturesHelp",
        ))],
    },
    Item {
        id: ItemId::MchVideoBgs,
        name: lookup_key("OptionsMachine", "VideoBGs"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "VideoBgsHelp",
        ))],
    },
    Item {
        id: ItemId::Exit,
        name: lookup_key("Options", "Exit"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsHelp",
            "ExitSubHelp",
        ))],
    },
];



impl ChoiceEnum for MachinePreferredPlayStyle {
    const ALL: &'static [Self] = &[Self::Single, Self::Versus, Self::Double];
    const DEFAULT: Self = Self::Single;
}

impl ChoiceEnum for MachinePreferredPlayMode {
    const ALL: &'static [Self] = &[Self::Regular, Self::Marathon];
    const DEFAULT: Self = Self::Regular;
}

impl ChoiceEnum for MachineFont {
    const ALL: &'static [Self] = &[Self::Common, Self::Mega];
    const DEFAULT: Self = Self::Common;
}

impl ChoiceEnum for VisualStyle {
    const ALL: &'static [Self] = &[
        Self::Hearts,
        Self::Arrows,
        Self::Bears,
        Self::Ducks,
        Self::Cats,
        Self::Spooky,
        Self::Gay,
        Self::Stars,
        Self::Thonk,
        Self::Technique,
        Self::Srpg9,
    ];
    const DEFAULT: Self = Self::Hearts;
}

pub(in crate::screens::options) const VISUAL_STYLE_CHOICES: &[Choice] = &[
    literal_choice("❤"),
    literal_choice("↖"),
    literal_choice("🐻"),
    literal_choice("🦆"),
    literal_choice("😺"),
    literal_choice("🎃"),
    literal_choice("🌈"),
    literal_choice("⭐"),
    literal_choice("🤔"),
    literal_choice("🌀"),
    literal_choice("💪"),
];

impl ChoiceEnum for LogLevel {
    const ALL: &'static [Self] = &[Self::Error, Self::Warn, Self::Info, Self::Debug, Self::Trace];
    const DEFAULT: Self = Self::Trace;
}
