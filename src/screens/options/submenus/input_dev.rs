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

fn apply_menu_buttons(state: &mut State, new_idx: usize) -> Outcome {
    state.pending_dedicated_menu_buttons = Some(new_idx == 1);
    Outcome::changed()
}

const MENU_BUTTONS_BINDING: CustomBinding = CustomBinding {
    apply: apply_menu_buttons,
};

pub(in crate::screens::options) const INPUT_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: RowId::InpConfigureMappings,
        label: lookup_key("OptionsInput", "ConfigureMappings"),
        choices: &[localized_choice("Common", "Open")],
        inline: false,
        behavior: RowBehavior::Exit,
    },
    SubRow {
        id: RowId::InpTestInput,
        label: lookup_key("OptionsInput", "TestInput"),
        choices: &[localized_choice("Common", "Open")],
        inline: false,
        behavior: RowBehavior::Exit,
    },
    SubRow {
        id: RowId::InpInputOptions,
        label: lookup_key("OptionsInput", "InputOptions"),
        choices: &[localized_choice("Common", "Open")],
        inline: false,
        behavior: RowBehavior::Exit,
    },
];

pub(in crate::screens::options) const INPUT_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: RowId::InpConfigureMappings,
        name: lookup_key("OptionsInput", "ConfigureMappings"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsInputHelp",
            "ConfigureMappingsHelp",
        ))],
    },
    Item {
        id: RowId::InpTestInput,
        name: lookup_key("OptionsInput", "TestInput"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsInputHelp",
            "TestInputHelp",
        ))],
    },
    Item {
        id: RowId::InpInputOptions,
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
        id: RowId::Exit,
        name: lookup_key("Options", "Exit"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsHelp",
            "ExitSubHelp",
        ))],
    },
];

pub(in crate::screens::options) const INPUT_BACKEND_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: RowId::InpGamepadBackend,
        label: lookup_key("OptionsInput", "GamepadBackend"),
        choices: INPUT_BACKEND_CHOICES,
        inline: INPUT_BACKEND_INLINE,
        behavior: RowBehavior::Cycle(GAMEPAD_BACKEND_BINDING),
    },
    SubRow {
        id: RowId::InpUseFsrs,
        label: lookup_key("OptionsInput", "UseFSRs"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(USE_FSRS_BINDING),
    },
    SubRow {
        id: RowId::InpMenuNavigation,
        label: lookup_key("OptionsInput", "MenuNavigation"),
        choices: &[
            localized_choice("OptionsInput", "MenuNavigationFiveKey"),
            localized_choice("OptionsInput", "MenuNavigationThreeKey"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(MENU_NAVIGATION_BINDING),
    },
    SubRow {
        id: RowId::InpOptionsNavigation,
        label: lookup_key("OptionsInput", "OptionsNavigation"),
        choices: &[
            localized_choice("OptionsInput", "OptionsNavigationStepMania"),
            localized_choice("OptionsInput", "OptionsNavigationArcade"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(OPTIONS_NAVIGATION_BINDING),
    },
    SubRow {
        id: RowId::InpMenuButtons,
        label: lookup_key("OptionsInput", "MenuButtons"),
        choices: &[
            localized_choice("OptionsInput", "DedicatedMenuButtonsGameplay"),
            localized_choice("OptionsInput", "DedicatedMenuButtonsOnly"),
        ],
        inline: true,
        behavior: RowBehavior::Custom(MENU_BUTTONS_BINDING),
    },
    SubRow {
        id: RowId::InpDebounce,
        label: lookup_key("OptionsInput", "Debounce"),
        choices: &[literal_choice("20ms")],
        inline: true,
        behavior: RowBehavior::Numeric(DEBOUNCE_BINDING),
    },
];

pub(in crate::screens::options) const INPUT_BACKEND_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: RowId::InpGamepadBackend,
        name: lookup_key("OptionsInput", "GamepadBackend"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsInputHelp",
            "GamepadBackendHelp",
        ))],
    },
    Item {
        id: RowId::InpUseFsrs,
        name: lookup_key("OptionsInput", "UseFSRs"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsInputHelp",
            "UseFSRsHelp",
        ))],
    },
    Item {
        id: RowId::InpMenuNavigation,
        name: lookup_key("OptionsInput", "MenuNavigation"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsInputHelp",
            "MenuNavigationHelp",
        ))],
    },
    Item {
        id: RowId::InpOptionsNavigation,
        name: lookup_key("OptionsInput", "OptionsNavigation"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsInputHelp",
            "OptionsNavigationHelp",
        ))],
    },
    Item {
        id: RowId::InpMenuButtons,
        name: lookup_key("OptionsInput", "MenuButtons"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsInputHelp",
            "MenuButtonsHelp",
        ))],
    },
    Item {
        id: RowId::InpDebounce,
        name: lookup_key("OptionsInput", "Debounce"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsInputHelp",
            "DebounceHelp",
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
