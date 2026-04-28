use super::super::*;

fn persist_enable_groovestats(enabled: bool) {
    config::update_enable_groovestats(enabled);
    crate::game::online::init();
}

fn persist_enable_boogiestats(enabled: bool) {
    config::update_enable_boogiestats(enabled);
    crate::game::online::init();
}

fn persist_enable_arrowcloud(enabled: bool) {
    config::update_enable_arrowcloud(enabled);
    crate::game::online::init();
}

const ENABLE_GROOVESTATS_BINDING: CycleBinding = CycleBinding::Bool(persist_enable_groovestats);
const ENABLE_BOOGIESTATS_BINDING: CycleBinding = CycleBinding::Bool(persist_enable_boogiestats);
const AUTO_POPULATE_SCORES_BINDING: CycleBinding =
    CycleBinding::Bool(config::update_auto_populate_gs_scores);
const AUTO_DOWNLOAD_UNLOCKS_BINDING: CycleBinding =
    CycleBinding::Bool(config::update_auto_download_unlocks);
const SEPARATE_UNLOCKS_BINDING: CycleBinding =
    CycleBinding::Bool(config::update_separate_unlocks_by_player);
const ENABLE_ARROWCLOUD_BINDING: CycleBinding = CycleBinding::Bool(persist_enable_arrowcloud);
const ARROWCLOUD_SUBMIT_FAILS_BINDING: CycleBinding =
    CycleBinding::Bool(config::update_submit_arrowcloud_fails);

pub(in crate::screens::options) const GROOVESTATS_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: SubRowId::EnableGrooveStats,
        label: lookup_key("OptionsGrooveStats", "EnableGrooveStats"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(ENABLE_GROOVESTATS_BINDING),
    },
    SubRow {
        id: SubRowId::EnableBoogieStats,
        label: lookup_key("OptionsGrooveStats", "EnableBoogieStats"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(ENABLE_BOOGIESTATS_BINDING),
    },
    SubRow {
        id: SubRowId::AutoPopulateScores,
        label: lookup_key("OptionsGrooveStats", "AutoPopulateScores"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(AUTO_POPULATE_SCORES_BINDING),
    },
    SubRow {
        id: SubRowId::AutoDownloadUnlocks,
        label: lookup_key("OptionsGrooveStats", "AutoDownloadUnlocks"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(AUTO_DOWNLOAD_UNLOCKS_BINDING),
    },
    SubRow {
        id: SubRowId::SeparateUnlocksByPlayer,
        label: lookup_key("OptionsGrooveStats", "SeparateUnlocksByPlayer"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(SEPARATE_UNLOCKS_BINDING),
    },
];

pub(in crate::screens::options) const ARROWCLOUD_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: SubRowId::EnableArrowCloud,
        label: lookup_key("OptionsGrooveStats", "EnableArrowCloud"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(ENABLE_ARROWCLOUD_BINDING),
    },
    SubRow {
        id: SubRowId::ArrowCloudSubmitFails,
        label: lookup_key("OptionsGrooveStats", "ArrowCloudSubmitFails"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(ARROWCLOUD_SUBMIT_FAILS_BINDING),
    },
];

pub(in crate::screens::options) const ONLINE_SCORING_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: SubRowId::GsBsOptions,
        label: lookup_key("OptionsOnlineScoring", "GsBsOptions"),
        choices: &[],
        inline: false,
        behavior: RowBehavior::Exit,
    },
    SubRow {
        id: SubRowId::ArrowCloudOptions,
        label: lookup_key("OptionsOnlineScoring", "ArrowCloudOptions"),
        choices: &[],
        inline: false,
        behavior: RowBehavior::Exit,
    },
    SubRow {
        id: SubRowId::ScoreImport,
        label: lookup_key("OptionsOnlineScoring", "ScoreImport"),
        choices: &[],
        inline: false,
        behavior: RowBehavior::Exit,
    },
];

pub(in crate::screens::options) const GROOVESTATS_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: ItemId::GsEnable,
        name: lookup_key("OptionsGrooveStats", "EnableGrooveStats"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGrooveStatsHelp",
            "EnableGrooveStatsHelp",
        ))],
    },
    Item {
        id: ItemId::GsEnableBoogie,
        name: lookup_key("OptionsGrooveStats", "EnableBoogieStats"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGrooveStatsHelp",
            "EnableBoogieStatsHelp",
        ))],
    },
    Item {
        id: ItemId::GsAutoPopulate,
        name: lookup_key("OptionsGrooveStats", "AutoPopulateScores"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGrooveStatsHelp",
            "AutoPopulateScoresHelp",
        ))],
    },
    Item {
        id: ItemId::GsAutoDownloadUnlocks,
        name: lookup_key("OptionsGrooveStats", "AutoDownloadUnlocks"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGrooveStatsHelp",
            "AutoDownloadUnlocksHelp",
        ))],
    },
    Item {
        id: ItemId::GsSeparateUnlocks,
        name: lookup_key("OptionsGrooveStats", "SeparateUnlocksByPlayer"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGrooveStatsHelp",
            "SeparateUnlocksByPlayerHelp",
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

pub(in crate::screens::options) const ARROWCLOUD_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: ItemId::AcEnable,
        name: lookup_key("OptionsGrooveStats", "EnableArrowCloud"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGrooveStatsHelp",
            "EnableArrowCloudHelp",
        ))],
    },
    Item {
        id: ItemId::AcSubmitFails,
        name: lookup_key("OptionsGrooveStats", "ArrowCloudSubmitFails"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGrooveStatsHelp",
            "ArrowCloudSubmitFailsHelp",
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

pub(in crate::screens::options) const ONLINE_SCORING_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: ItemId::OsGsBsOptions,
        name: lookup_key("OptionsOnlineScoring", "GsBsOptions"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsOnlineScoringHelp",
            "GsBsOptionsHelp",
        ))],
    },
    Item {
        id: ItemId::OsArrowCloudOptions,
        name: lookup_key("OptionsOnlineScoring", "ArrowCloudOptions"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsOnlineScoringHelp",
            "ArrowCloudOptionsHelp",
        ))],
    },
    Item {
        id: ItemId::OsScoreImport,
        name: lookup_key("OptionsOnlineScoring", "ScoreImport"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsOnlineScoringHelp",
            "ScoreImportHelp",
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
