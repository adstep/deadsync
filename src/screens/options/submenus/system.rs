use super::super::*;

const GAME_BINDING: CycleBinding =
    CycleBinding::Index(|_| config::update_game_flag(config::GameFlag::Dance));
const THEME_BINDING: CycleBinding =
    CycleBinding::Index(|_| config::update_theme_flag(config::ThemeFlag::SimplyLove));
const LOG_LEVEL_BINDING: CycleBinding =
    CycleBinding::Index(|i| config::update_log_level(LogLevel::from_choice(i)));
const LOG_FILE_BINDING: CycleBinding = CycleBinding::Bool(config::update_log_to_file);

fn apply_language(_state: &mut State, new_idx: usize) -> Outcome {
    let flag = language_flag_from_choice(new_idx);
    config::update_language_flag(flag);
    assets::i18n::set_locale(&assets::i18n::resolve_locale(flag));
    Outcome::changed()
}

fn apply_default_noteskin(state: &mut State, new_idx: usize) -> Outcome {
    if let Some(skin_name) = state.system_noteskin_choices.get(new_idx).cloned() {
        profile::update_machine_default_noteskin(profile::NoteSkin::new(&skin_name));
    }
    Outcome::changed()
}

const LANGUAGE_BINDING: CustomBinding = CustomBinding {
    apply: apply_language,
};
const DEFAULT_NOTESKIN_BINDING: CustomBinding = CustomBinding {
    apply: apply_default_noteskin,
};

pub(in crate::screens::options) const SYSTEM_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: SubRowId::Game,
        label: lookup_key("OptionsSystem", "Game"),
        choices: &[localized_choice("OptionsSystem", "DanceGame")],
        inline: false,
        behavior: RowBehavior::Cycle(GAME_BINDING),
    },
    SubRow {
        id: SubRowId::Theme,
        label: lookup_key("OptionsSystem", "Theme"),
        choices: &[localized_choice("OptionsSystem", "SimplyLoveTheme")],
        inline: false,
        behavior: RowBehavior::Cycle(THEME_BINDING),
    },
    SubRow {
        id: SubRowId::Language,
        label: lookup_key("OptionsSystem", "Language"),
        choices: LANGUAGE_CHOICES,
        inline: false,
        behavior: RowBehavior::Custom(LANGUAGE_BINDING),
    },
    SubRow {
        id: SubRowId::LogLevel,
        label: lookup_key("OptionsSystem", "LogLevel"),
        choices: &[
            localized_choice("OptionsSystem", "LogLevelError"),
            localized_choice("OptionsSystem", "LogLevelWarn"),
            localized_choice("OptionsSystem", "LogLevelInfo"),
            localized_choice("OptionsSystem", "LogLevelDebug"),
            localized_choice("OptionsSystem", "LogLevelTrace"),
        ],
        inline: false,
        behavior: RowBehavior::Cycle(LOG_LEVEL_BINDING),
    },
    SubRow {
        id: SubRowId::LogFile,
        label: lookup_key("OptionsSystem", "LogFile"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: false,
        behavior: RowBehavior::Cycle(LOG_FILE_BINDING),
    },
    SubRow {
        id: SubRowId::DefaultNoteSkin,
        label: lookup_key("OptionsSystem", "DefaultNoteSkin"),
        choices: &[literal_choice(profile::NoteSkin::DEFAULT_NAME)],
        inline: false,
        behavior: RowBehavior::Custom(DEFAULT_NOTESKIN_BINDING),
    },
];

pub(in crate::screens::options) const SYSTEM_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: ItemId::SysGame,
        name: lookup_key("OptionsSystem", "Game"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSystemHelp",
            "GameHelp",
        ))],
    },
    Item {
        id: ItemId::SysTheme,
        name: lookup_key("OptionsSystem", "Theme"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSystemHelp",
            "ThemeHelp",
        ))],
    },
    Item {
        id: ItemId::SysLanguage,
        name: lookup_key("OptionsSystem", "Language"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSystemHelp",
            "LanguageHelp",
        ))],
    },
    Item {
        id: ItemId::SysLogLevel,
        name: lookup_key("OptionsSystem", "LogLevel"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSystemHelp",
            "LogLevelHelp",
        ))],
    },
    Item {
        id: ItemId::SysLogFile,
        name: lookup_key("OptionsSystem", "LogFile"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSystemHelp",
            "LogFileHelp",
        ))],
    },
    Item {
        id: ItemId::SysDefaultNoteSkin,
        name: lookup_key("OptionsSystem", "DefaultNoteSkin"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSystemHelp",
            "DefaultNoteSkinHelp",
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

pub(in crate::screens::options) fn discover_system_noteskin_choices() -> Vec<String> {
    let mut names = noteskin_parser::discover_itg_skins("dance");
    if names.is_empty() {
        names.push(profile::NoteSkin::DEFAULT_NAME.to_string());
    }
    names
}

pub(in crate::screens::options) const fn translated_titles_choice_index(
    translated_titles: bool,
) -> usize {
    if translated_titles { 0 } else { 1 }
}

pub(in crate::screens::options) const fn translated_titles_from_choice(idx: usize) -> bool {
    idx == 0
}

pub(in crate::screens::options) const fn language_choice_index(
    flag: config::LanguageFlag,
) -> usize {
    match flag {
        config::LanguageFlag::Auto | config::LanguageFlag::English => 0,
        config::LanguageFlag::German => 1,
        config::LanguageFlag::Spanish => 2,
        config::LanguageFlag::French => 3,
        config::LanguageFlag::Italian => 4,
        config::LanguageFlag::Japanese => 5,
        config::LanguageFlag::Polish => 6,
        config::LanguageFlag::PortugueseBrazil => 7,
        config::LanguageFlag::Russian => 8,
        config::LanguageFlag::Swedish => 9,
        config::LanguageFlag::Pseudo => 10,
    }
}

pub(in crate::screens::options) const fn language_flag_from_choice(
    idx: usize,
) -> config::LanguageFlag {
    match idx {
        1 => config::LanguageFlag::German,
        2 => config::LanguageFlag::Spanish,
        3 => config::LanguageFlag::French,
        4 => config::LanguageFlag::Italian,
        5 => config::LanguageFlag::Japanese,
        6 => config::LanguageFlag::Polish,
        7 => config::LanguageFlag::PortugueseBrazil,
        8 => config::LanguageFlag::Russian,
        9 => config::LanguageFlag::Swedish,
        10 => config::LanguageFlag::Pseudo,
        _ => config::LanguageFlag::English,
    }
}
