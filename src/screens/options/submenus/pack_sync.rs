use super::super::*;

pub(in crate::screens::options) const SYNC_PACK_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: RowId::SpPack,
        label: lookup_key("OptionsSyncPack", "SyncPackPack"),
        choices: &[localized_choice("OptionsSyncPack", "AllPacks")],
        inline: false,
        behavior: RowBehavior::Exit,
    },
    SubRow {
        id: RowId::SpStart,
        label: lookup_key("OptionsSyncPack", "SyncPackStart"),
        choices: &[localized_choice("Common", "Start")],
        inline: false,
        behavior: RowBehavior::Exit,
    },
];

pub(in crate::screens::options) const SYNC_PACK_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: RowId::SpPack,
        name: lookup_key("OptionsSyncPack", "SyncPackPack"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSyncPackHelp",
            "SyncPackPackHelp",
        ))],
    },
    Item {
        id: RowId::SpStart,
        name: lookup_key("OptionsSyncPack", "SyncPackStart"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSyncPackHelp",
            "SyncPackStartHelp",
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
