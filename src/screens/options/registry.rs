use super::*;

/// Static metadata for a single submenu. Replaces the four parallel
/// `match kind { ... }` blocks that previously mapped `SubmenuKind` to
/// rows / items / title / launcher-ness.
///
/// To add a new submenu: append a variant to `SubmenuKind`, extend
/// `SubmenuKind::ALL`, and add one row to `SUBMENU_DEFS`. The
/// compile-time assertion below ensures the table stays aligned with the
/// enum's discriminant order.
pub(super) struct SubmenuDef {
    pub(super) kind: SubmenuKind,
    pub(super) rows: &'static [SubRow],
    pub(super) items: &'static [Item],
    pub(super) title: LookupKey,
    pub(super) is_launcher: bool,
}

pub(super) const SUBMENU_DEFS: [SubmenuDef; SubmenuKind::COUNT] = [
    SubmenuDef {
        kind: SubmenuKind::System,
        rows: SYSTEM_OPTIONS_ROWS,
        items: SYSTEM_OPTIONS_ITEMS,
        title: lookup_key("Options", "SystemOptions"),
        is_launcher: false,
    },
    SubmenuDef {
        kind: SubmenuKind::Graphics,
        rows: GRAPHICS_OPTIONS_ROWS,
        items: GRAPHICS_OPTIONS_ITEMS,
        title: lookup_key("Options", "GraphicsOptions"),
        is_launcher: false,
    },
    SubmenuDef {
        kind: SubmenuKind::Input,
        rows: INPUT_OPTIONS_ROWS,
        items: INPUT_OPTIONS_ITEMS,
        title: lookup_key("Options", "InputOptions"),
        is_launcher: true,
    },
    SubmenuDef {
        kind: SubmenuKind::InputBackend,
        rows: INPUT_BACKEND_OPTIONS_ROWS,
        items: INPUT_BACKEND_OPTIONS_ITEMS,
        title: lookup_key("Options", "InputOptions"),
        is_launcher: false,
    },
    SubmenuDef {
        kind: SubmenuKind::OnlineScoring,
        rows: ONLINE_SCORING_OPTIONS_ROWS,
        items: ONLINE_SCORING_OPTIONS_ITEMS,
        title: lookup_key("Options", "OnlineScoreServices"),
        is_launcher: true,
    },
    SubmenuDef {
        kind: SubmenuKind::NullOrDie,
        rows: NULL_OR_DIE_MENU_ROWS,
        items: NULL_OR_DIE_MENU_ITEMS,
        title: lookup_key("Options", "NullOrDieOptions"),
        is_launcher: true,
    },
    SubmenuDef {
        kind: SubmenuKind::NullOrDieOptions,
        rows: NULL_OR_DIE_OPTIONS_ROWS,
        items: NULL_OR_DIE_OPTIONS_ITEMS,
        title: lookup_key("Options", "NullOrDieOptions"),
        is_launcher: false,
    },
    SubmenuDef {
        kind: SubmenuKind::SyncPacks,
        rows: SYNC_PACK_OPTIONS_ROWS,
        items: SYNC_PACK_OPTIONS_ITEMS,
        title: lookup_key("Options", "SyncPacks"),
        is_launcher: false,
    },
    SubmenuDef {
        kind: SubmenuKind::Machine,
        rows: MACHINE_OPTIONS_ROWS,
        items: MACHINE_OPTIONS_ITEMS,
        title: lookup_key("Options", "MachineOptions"),
        is_launcher: false,
    },
    SubmenuDef {
        kind: SubmenuKind::Advanced,
        rows: ADVANCED_OPTIONS_ROWS,
        items: ADVANCED_OPTIONS_ITEMS,
        title: lookup_key("Options", "AdvancedOptions"),
        is_launcher: false,
    },
    SubmenuDef {
        kind: SubmenuKind::Course,
        rows: COURSE_OPTIONS_ROWS,
        items: COURSE_OPTIONS_ITEMS,
        title: lookup_key("Options", "CourseOptions"),
        is_launcher: false,
    },
    SubmenuDef {
        kind: SubmenuKind::Gameplay,
        rows: GAMEPLAY_OPTIONS_ROWS,
        items: GAMEPLAY_OPTIONS_ITEMS,
        title: lookup_key("Options", "GameplayOptions"),
        is_launcher: false,
    },
    SubmenuDef {
        kind: SubmenuKind::Sound,
        rows: SOUND_OPTIONS_ROWS,
        items: SOUND_OPTIONS_ITEMS,
        title: lookup_key("Options", "SoundOptions"),
        is_launcher: false,
    },
    SubmenuDef {
        kind: SubmenuKind::SelectMusic,
        rows: SELECT_MUSIC_OPTIONS_ROWS,
        items: SELECT_MUSIC_OPTIONS_ITEMS,
        title: lookup_key("Options", "SelectMusicOptions"),
        is_launcher: false,
    },
    SubmenuDef {
        kind: SubmenuKind::GrooveStats,
        rows: GROOVESTATS_OPTIONS_ROWS,
        items: GROOVESTATS_OPTIONS_ITEMS,
        title: lookup_key("Options", "GrooveStatsOptions"),
        is_launcher: false,
    },
    SubmenuDef {
        kind: SubmenuKind::ArrowCloud,
        rows: ARROWCLOUD_OPTIONS_ROWS,
        items: ARROWCLOUD_OPTIONS_ITEMS,
        title: lookup_key("Options", "ArrowCloudOptions"),
        is_launcher: false,
    },
    SubmenuDef {
        kind: SubmenuKind::ScoreImport,
        rows: SCORE_IMPORT_OPTIONS_ROWS,
        items: SCORE_IMPORT_OPTIONS_ITEMS,
        title: lookup_key("Options", "ScoreImport"),
        is_launcher: false,
    },
];

// Compile-time guarantee that SUBMENU_DEFS is in the same order as
// SubmenuKind discriminants. If this fails, the array order drifted from
// the enum and the lookups below would return wrong metadata.
const _: () = {
    let mut i = 0;
    while i < SubmenuKind::COUNT {
        assert!(SUBMENU_DEFS[i].kind.index() == i);
        assert!(SubmenuKind::ALL[i].index() == i);
        i += 1;
    }
};

#[inline]
pub(super) const fn submenu_def(kind: SubmenuKind) -> &'static SubmenuDef {
    &SUBMENU_DEFS[kind.index()]
}

#[inline]
pub(super) const fn submenu_rows(kind: SubmenuKind) -> &'static [SubRow] {
    submenu_def(kind).rows
}

#[inline]
pub(super) const fn submenu_items(kind: SubmenuKind) -> &'static [Item] {
    submenu_def(kind).items
}

#[inline]
pub(super) const fn submenu_title(kind: SubmenuKind) -> LookupKey {
    submenu_def(kind).title
}

#[inline]
pub(super) const fn is_launcher_submenu(kind: SubmenuKind) -> bool {
    submenu_def(kind).is_launcher
}
