use super::super::*;

fn apply_score_import_endpoint(state: &mut State, _new_idx: usize) -> Outcome {
    refresh_score_import_profile_options(state);
    Outcome::changed()
}

const SCORE_IMPORT_ENDPOINT_BINDING: CustomBinding = CustomBinding {
    apply: apply_score_import_endpoint,
};

pub(in crate::screens::options) const SCORE_IMPORT_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: SubRowId::ScoreImportEndpoint,
        label: lookup_key("OptionsScoreImport", "ScoreImportEndpoint"),
        choices: &[
            literal_choice("GrooveStats"),
            literal_choice("BoogieStats"),
            literal_choice("ArrowCloud"),
        ],
        inline: true,
        behavior: RowBehavior::Custom(SCORE_IMPORT_ENDPOINT_BINDING),
    },
    SubRow {
        id: SubRowId::ScoreImportProfile,
        label: lookup_key("OptionsScoreImport", "ScoreImportProfile"),
        choices: &[localized_choice("OptionsScoreImport", "NoEligibleProfiles")],
        inline: false,
        behavior: RowBehavior::Legacy,
    },
    SubRow {
        id: SubRowId::ScoreImportPack,
        label: lookup_key("OptionsScoreImport", "ScoreImportPack"),
        choices: &[localized_choice("OptionsScoreImport", "AllPacks")],
        inline: false,
        behavior: RowBehavior::Legacy,
    },
    SubRow {
        id: SubRowId::ScoreImportOnlyMissing,
        label: lookup_key("OptionsScoreImport", "ScoreImportOnlyMissing"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
        behavior: RowBehavior::Legacy,
    },
    SubRow {
        id: SubRowId::ScoreImportStart,
        label: lookup_key("OptionsScoreImport", "ScoreImportStart"),
        choices: &[localized_choice("Common", "Start")],
        inline: false,
        behavior: RowBehavior::Legacy,
    },
];

pub(in crate::screens::options) const SCORE_IMPORT_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: ItemId::SiEndpoint,
        name: lookup_key("OptionsScoreImport", "ScoreImportEndpoint"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsScoreImportHelp",
            "ScoreImportEndpointHelp",
        ))],
    },
    Item {
        id: ItemId::SiProfile,
        name: lookup_key("OptionsScoreImport", "ScoreImportProfile"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsScoreImportHelp",
            "ScoreImportProfileHelp",
        ))],
    },
    Item {
        id: ItemId::SiPack,
        name: lookup_key("OptionsScoreImport", "ScoreImportPack"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsScoreImportHelp",
            "ScoreImportPackHelp",
        ))],
    },
    Item {
        id: ItemId::SiOnlyMissing,
        name: lookup_key("OptionsScoreImport", "ScoreImportOnlyMissing"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsScoreImportHelp",
            "ScoreImportOnlyMissingHelp",
        ))],
    },
    Item {
        id: ItemId::SiStart,
        name: lookup_key("OptionsScoreImport", "ScoreImportStart"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsScoreImportHelp",
            "ScoreImportStartHelp",
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
