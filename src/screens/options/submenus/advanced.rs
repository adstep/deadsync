use super::super::*;

const DEFAULT_FAIL_TYPE_BINDING: CycleBinding =
    CycleBinding::Index(|i| config::update_default_fail_type(DefaultFailType::from_choice(i)));
const BANNER_CACHE_BINDING: CycleBinding = CycleBinding::Bool(config::update_banner_cache);
const CDTITLE_CACHE_BINDING: CycleBinding = CycleBinding::Bool(config::update_cdtitle_cache);
const CACHE_SONGS_BINDING: CycleBinding = CycleBinding::Bool(config::update_cache_songs);
const FAST_LOAD_BINDING: CycleBinding = CycleBinding::Bool(config::update_fastload);

fn apply_song_parsing_threads(state: &mut State, new_idx: usize) -> Outcome {
    let threads = software_thread_from_choice(&state.software_thread_choices, new_idx);
    config::update_song_parsing_threads(threads);
    Outcome::changed()
}

const SONG_PARSING_THREADS_BINDING: CustomBinding = CustomBinding {
    apply: apply_song_parsing_threads,
};

pub(in crate::screens::options) const ADVANCED_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: RowId::AdvDefaultFailType,
        label: lookup_key("OptionsAdvanced", "DefaultFailType"),
        choices: &[
            localized_choice("OptionsAdvanced", "FailImmediate"),
            localized_choice("OptionsAdvanced", "FailImmediateContinue"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(DEFAULT_FAIL_TYPE_BINDING),
    },
    SubRow {
        id: RowId::AdvBannerCache,
        label: lookup_key("OptionsAdvanced", "BannerCache"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(BANNER_CACHE_BINDING),
    },
    SubRow {
        id: RowId::AdvCdTitleCache,
        label: lookup_key("OptionsAdvanced", "CDTitleCache"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(CDTITLE_CACHE_BINDING),
    },
    SubRow {
        id: RowId::AdvSongParsingThreads,
        label: lookup_key("OptionsAdvanced", "SongParsingThreads"),
        choices: &[localized_choice("Common", "Auto")],
        inline: false,
        behavior: RowBehavior::Custom(SONG_PARSING_THREADS_BINDING),
    },
    SubRow {
        id: RowId::AdvCacheSongs,
        label: lookup_key("OptionsAdvanced", "CacheSongs"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(CACHE_SONGS_BINDING),
    },
    SubRow {
        id: RowId::AdvFastLoad,
        label: lookup_key("OptionsAdvanced", "FastLoad"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(FAST_LOAD_BINDING),
    },
];

pub(in crate::screens::options) const ADVANCED_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: RowId::AdvDefaultFailType,
        name: lookup_key("OptionsAdvanced", "DefaultFailType"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsAdvancedHelp",
            "DefaultFailTypeHelp",
        ))],
    },
    Item {
        id: RowId::AdvBannerCache,
        name: lookup_key("OptionsAdvanced", "BannerCache"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsAdvancedHelp",
            "BannerCacheHelp",
        ))],
    },
    Item {
        id: RowId::AdvCdTitleCache,
        name: lookup_key("OptionsAdvanced", "CDTitleCache"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsAdvancedHelp",
            "CdTitleCacheHelp",
        ))],
    },
    Item {
        id: RowId::AdvSongParsingThreads,
        name: lookup_key("OptionsAdvanced", "SongParsingThreads"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsAdvancedHelp",
            "SongParsingThreadsHelp",
        ))],
    },
    Item {
        id: RowId::AdvCacheSongs,
        name: lookup_key("OptionsAdvanced", "CacheSongs"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsAdvancedHelp",
            "CacheSongsHelp",
        ))],
    },
    Item {
        id: RowId::AdvFastLoad,
        name: lookup_key("OptionsAdvanced", "FastLoad"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsAdvancedHelp",
            "FastLoadHelp",
        ))],
    },
    Item {
        id: RowId::Exit,
        name: lookup_key("Options", "Exit"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsHelp",
            "ExitSubHelp",
        ))],
    },
];


impl ChoiceEnum for DefaultFailType {
    const ALL: &'static [Self] = &[Self::Immediate, Self::ImmediateContinue];
    const DEFAULT: Self = Self::ImmediateContinue;
}
