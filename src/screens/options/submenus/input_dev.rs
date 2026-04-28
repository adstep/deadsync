use super::super::*;

const DEBOUNCE_BINDING: NumericBinding = NumericBinding {
    get_mut: |s: &mut State| &mut s.input_debounce_ms,
    min: INPUT_DEBOUNCE_MIN_MS,
    max: INPUT_DEBOUNCE_MAX_MS,
    step: NumericStep::Ms,
    persist: |v| config::update_input_debounce_seconds(v as f32 / 1000.0),
};

fn persist_gamepad_backend(idx: usize) {
    #[cfg(target_os = "windows")]
    {
        config::update_windows_gamepad_backend(windows_backend_from_choice(idx));
    }
    #[cfg(not(target_os = "windows"))]
    let _ = idx;
}

const GAMEPAD_BACKEND_BINDING: CycleBinding = CycleBinding::Index(persist_gamepad_backend);
const USE_FSRS_BINDING: CycleBinding = CycleBinding::Bool(config::update_use_fsrs);
const MENU_NAVIGATION_BINDING: CycleBinding = CycleBinding::Bool(config::update_three_key_navigation);
const OPTIONS_NAVIGATION_BINDING: CycleBinding =
    CycleBinding::Bool(config::update_arcade_options_navigation);

pub(in crate::screens::options) const INPUT_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: SubRowId::ConfigureMappings,
        label: lookup_key("OptionsInput", "ConfigureMappings"),
        choices: &[localized_choice("Common", "Open")],
        inline: false,
        behavior: RowBehavior::Legacy,
    },
    SubRow {
        id: SubRowId::TestInput,
        label: lookup_key("OptionsInput", "TestInput"),
        choices: &[localized_choice("Common", "Open")],
        inline: false,
        behavior: RowBehavior::Legacy,
    },
    SubRow {
        id: SubRowId::InputOptions,
        label: lookup_key("OptionsInput", "InputOptions"),
        choices: &[localized_choice("Common", "Open")],
        inline: false,
        behavior: RowBehavior::Legacy,
    },
];

pub(in crate::screens::options) const INPUT_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: ItemId::InpConfigureMappings,
        name: lookup_key("OptionsInput", "ConfigureMappings"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsInputHelp",
            "ConfigureMappingsHelp",
        ))],
    },
    Item {
        id: ItemId::InpTestInput,
        name: lookup_key("OptionsInput", "TestInput"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsInputHelp",
            "TestInputHelp",
        ))],
    },
    Item {
        id: ItemId::InpInputOptions,
        name: lookup_key("OptionsInput", "InputOptions"),
        help: &[
            HelpEntry::Paragraph(lookup_key("OptionsInputHelp", "InputOptionsHelp")),
            HelpEntry::Bullet(lookup_key("OptionsInput", "GamepadBackend")),
            HelpEntry::Bullet(lookup_key("OptionsInput", "UseFSRs")),
            HelpEntry::Bullet(lookup_key("OptionsInput", "MenuNavigation")),
            HelpEntry::Bullet(lookup_key("OptionsInput", "OptionsNavigation")),
            HelpEntry::Bullet(lookup_key("OptionsInput", "MenuButtons")),
            HelpEntry::Bullet(lookup_key("OptionsInput", "Debounce")),
        ],
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

pub(in crate::screens::options) const INPUT_BACKEND_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: SubRowId::GamepadBackend,
        label: lookup_key("OptionsInput", "GamepadBackend"),
        choices: INPUT_BACKEND_CHOICES,
        inline: INPUT_BACKEND_INLINE,
        behavior: RowBehavior::Cycle(GAMEPAD_BACKEND_BINDING),
    },
    SubRow {
        id: SubRowId::UseFsrs,
        label: lookup_key("OptionsInput", "UseFSRs"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(USE_FSRS_BINDING),
    },
    SubRow {
        id: SubRowId::MenuNavigation,
        label: lookup_key("OptionsInput", "MenuNavigation"),
        choices: &[
            localized_choice("OptionsInput", "MenuNavigationFiveKey"),
            localized_choice("OptionsInput", "MenuNavigationThreeKey"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(MENU_NAVIGATION_BINDING),
    },
    SubRow {
        id: SubRowId::OptionsNavigation,
        label: lookup_key("OptionsInput", "OptionsNavigation"),
        choices: &[
            localized_choice("OptionsInput", "OptionsNavigationStepMania"),
            localized_choice("OptionsInput", "OptionsNavigationArcade"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(OPTIONS_NAVIGATION_BINDING),
    },
    SubRow {
        id: SubRowId::MenuButtons,
        label: lookup_key("OptionsInput", "MenuButtons"),
        choices: &[
            localized_choice("OptionsInput", "DedicatedMenuButtonsGameplay"),
            localized_choice("OptionsInput", "DedicatedMenuButtonsOnly"),
        ],
        inline: true,
        behavior: RowBehavior::Legacy,
    },
    SubRow {
        id: SubRowId::Debounce,
        label: lookup_key("OptionsInput", "Debounce"),
        choices: &[literal_choice("20ms")],
        inline: true,
        behavior: RowBehavior::Numeric(DEBOUNCE_BINDING),
    },
];

pub(in crate::screens::options) const INPUT_BACKEND_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: ItemId::InpGamepadBackend,
        name: lookup_key("OptionsInput", "GamepadBackend"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsInputHelp",
            "GamepadBackendHelp",
        ))],
    },
    Item {
        id: ItemId::InpUseFsrs,
        name: lookup_key("OptionsInput", "UseFSRs"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsInputHelp",
            "UseFSRsHelp",
        ))],
    },
    Item {
        id: ItemId::InpMenuButtons,
        name: lookup_key("OptionsInput", "MenuButtons"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsInputHelp",
            "MenuButtonsHelp",
        ))],
    },
    Item {
        id: ItemId::InpOptionsNavigation,
        name: lookup_key("OptionsInput", "OptionsNavigation"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsInputHelp",
            "OptionsNavigationHelp",
        ))],
    },
    Item {
        id: ItemId::InpMenuNavigation,
        name: lookup_key("OptionsInput", "MenuNavigation"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsInputHelp",
            "MenuNavigationHelp",
        ))],
    },
    Item {
        id: ItemId::InpDebounce,
        name: lookup_key("OptionsInput", "Debounce"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsInputHelp",
            "DebounceHelp",
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
