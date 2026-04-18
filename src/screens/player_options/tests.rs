#[cfg(test)]
mod tests {
    use super::super::{
        ActionRow, HUD_OFFSET_MAX, HUD_OFFSET_MIN, HUD_OFFSET_ZERO_INDEX, NAV_INITIAL_HOLD_DELAY,
        NAV_REPEAT_SCROLL_INTERVAL, P1, Row, RowId, RowKind, SpeedMod, SpeedModType, TestAction,
        dispatch_for_test, handle_arcade_start_event, hud_offset_choices, is_row_visible,
        repeat_held_arcade_start, row_visibility, session_active_players,
        sync_profile_scroll_speed,
    };
    use crate::assets::AssetManager;
    use crate::assets::i18n::{LookupKey, lookup_key};
    use crate::game::profile::{self, PlayStyle, PlayerSide, Profile};
    use crate::game::scroll::ScrollSpeedSetting;
    use crate::screens::Screen;
    use crate::test_support::{compose_scenarios, notefield_bench};
    use std::time::{Duration, Instant};

    fn ensure_i18n() {
        use std::sync::Once;
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            crate::assets::i18n::init("en");
        });
    }

    fn test_row(
        id: RowId,
        name: LookupKey,
        choices: &[&str],
        selected_choice_index: [usize; 2],
    ) -> Row {
        Row {
            id,
            name,
            choices: choices.iter().map(ToString::to_string).collect(),
            selected_choice_index,
            help: Vec::new(),
            choice_difficulty_indices: None,
            // Tests don't exercise the dispatcher; pick a dispatcher-inert
            // variant so we don't have to plumb a real binding here.
            kind: RowKind::Action(ActionRow::Exit),
        }
    }

    #[test]
    fn sync_profile_scroll_speed_matches_speed_mod() {
        let mut profile = Profile::default();

        sync_profile_scroll_speed(
            &mut profile,
            &SpeedMod {
                mod_type: SpeedModType::X,
                value: 1.5,
            },
        );
        assert_eq!(profile.scroll_speed, ScrollSpeedSetting::XMod(1.5));

        sync_profile_scroll_speed(
            &mut profile,
            &SpeedMod {
                mod_type: SpeedModType::M,
                value: 750.0,
            },
        );
        assert_eq!(profile.scroll_speed, ScrollSpeedSetting::MMod(750.0));

        sync_profile_scroll_speed(
            &mut profile,
            &SpeedMod {
                mod_type: SpeedModType::C,
                value: 600.0,
            },
        );
        assert_eq!(profile.scroll_speed, ScrollSpeedSetting::CMod(600.0));
    }

    #[test]
    fn error_bar_offsets_hide_with_empty_error_bar_mask() {
        ensure_i18n();
        let rows = vec![
            test_row(
                RowId::ErrorBar,
                lookup_key("PlayerOptions", "ErrorBar"),
                &["Colorful"],
                [0, 0],
            ),
            test_row(
                RowId::ErrorBarOffsetX,
                lookup_key("PlayerOptions", "ErrorBarOffsetX"),
                &["0"],
                [0, 0],
            ),
        ];
        let visibility = row_visibility(&rows, [true, false], [0, 0], [0, 0], false);
        assert!(!is_row_visible(&rows, 1, visibility));

        let visibility = row_visibility(&rows, [true, false], [0, 0], [1, 0], false);
        assert!(is_row_visible(&rows, 1, visibility));
    }

    #[test]
    fn judgment_offsets_hide_when_judgment_font_is_none() {
        ensure_i18n();
        let rows = vec![
            test_row(
                RowId::JudgmentFont,
                lookup_key("PlayerOptions", "JudgmentFont"),
                &["Love", "None"],
                [1, 0],
            ),
            test_row(
                RowId::JudgmentOffsetX,
                lookup_key("PlayerOptions", "JudgmentOffsetX"),
                &["0"],
                [0, 0],
            ),
        ];
        let visibility = row_visibility(&rows, [true, false], [0, 0], [0, 0], false);
        assert!(!is_row_visible(&rows, 1, visibility));

        let rows = vec![
            test_row(
                RowId::JudgmentFont,
                lookup_key("PlayerOptions", "JudgmentFont"),
                &["Love", "None"],
                [0, 0],
            ),
            test_row(
                RowId::JudgmentOffsetX,
                lookup_key("PlayerOptions", "JudgmentOffsetX"),
                &["0"],
                [0, 0],
            ),
        ];
        let visibility = row_visibility(&rows, [true, false], [0, 0], [0, 0], false);
        assert!(is_row_visible(&rows, 1, visibility));
    }

    #[test]
    fn combo_offsets_hide_when_all_active_players_use_none_font() {
        ensure_i18n();
        let rows = vec![
            test_row(
                RowId::ComboFont,
                lookup_key("PlayerOptions", "ComboFont"),
                &["Wendy", "None"],
                [1, 1],
            ),
            test_row(
                RowId::ComboOffsetX,
                lookup_key("PlayerOptions", "ComboOffsetX"),
                &["0"],
                [0, 0],
            ),
        ];
        let visibility = row_visibility(&rows, [true, true], [0, 0], [0, 0], false);
        assert!(!is_row_visible(&rows, 1, visibility));

        let rows = vec![
            test_row(
                RowId::ComboFont,
                lookup_key("PlayerOptions", "ComboFont"),
                &["Wendy", "None"],
                [1, 0],
            ),
            test_row(
                RowId::ComboOffsetX,
                lookup_key("PlayerOptions", "ComboOffsetX"),
                &["0"],
                [0, 0],
            ),
        ];
        let visibility = row_visibility(&rows, [true, true], [0, 0], [0, 0], false);
        assert!(is_row_visible(&rows, 1, visibility));
    }

    #[test]
    fn hud_offset_choices_cover_full_range() {
        let choices = hud_offset_choices();
        assert_eq!(choices.first().map(String::as_str), Some("-250"));
        assert_eq!(
            choices.get(HUD_OFFSET_ZERO_INDEX).map(String::as_str),
            Some("0")
        );
        assert_eq!(choices.last().map(String::as_str), Some("250"));
        assert_eq!(choices.len() as i32, HUD_OFFSET_MAX - HUD_OFFSET_MIN + 1);
    }

    #[test]
    fn held_arcade_start_keeps_advancing_rows() {
        ensure_i18n();
        let base = notefield_bench::fixture();
        let song = base.state().song.clone();

        profile::set_session_play_style(PlayStyle::Single);
        profile::set_session_player_side(PlayerSide::P1);
        profile::set_session_joined(true, false);

        let mut asset_manager = AssetManager::new();
        for (name, font) in compose_scenarios::bench_fonts() {
            asset_manager.register_font(name, font);
        }

        let mut state = super::super::init(song, [0; 2], [0; 2], 1, Screen::SelectMusic, None);
        let active = session_active_players();
        let first_row = state.selected_row()[P1];
        assert!(handle_arcade_start_event(&mut state, &asset_manager, active, P1).is_none());
        let second_row = state.selected_row()[P1];
        assert!(second_row > first_row);

        let now = Instant::now();
        state.start_held_since[P1] = Some(now - NAV_INITIAL_HOLD_DELAY - Duration::from_millis(1));
        state.start_last_triggered_at[P1] =
            Some(now - NAV_REPEAT_SCROLL_INTERVAL - Duration::from_millis(1));

        assert!(repeat_held_arcade_start(&mut state, &asset_manager, active, P1, now).is_none());
        assert!(state.selected_row()[P1] > second_row);
    }

    #[test]
    fn held_arcade_start_stops_at_exit_row() {
        ensure_i18n();
        let base = notefield_bench::fixture();
        let song = base.state().song.clone();

        profile::set_session_play_style(PlayStyle::Single);
        profile::set_session_player_side(PlayerSide::P1);
        profile::set_session_joined(true, false);

        let mut asset_manager = AssetManager::new();
        for (name, font) in compose_scenarios::bench_fonts() {
            asset_manager.register_font(name, font);
        }

        let mut state = super::super::init(song, [0; 2], [0; 2], 1, Screen::SelectMusic, None);
        let active = session_active_players();
        let last_row = state.rows().len().saturating_sub(1);
        state.selected_row_mut()[P1] = last_row;
        state.prev_selected_row_mut()[P1] = last_row;

        let now = Instant::now();
        state.start_held_since[P1] = Some(now - NAV_INITIAL_HOLD_DELAY - Duration::from_millis(1));
        state.start_last_triggered_at[P1] =
            Some(now - NAV_REPEAT_SCROLL_INTERVAL - Duration::from_millis(1));

        assert!(repeat_held_arcade_start(&mut state, &asset_manager, active, P1, now).is_none());
        assert_eq!(state.selected_row()[P1], last_row);
    }

    #[test]
    fn dispatch_for_test_drives_numeric_row_through_dispatcher() {
        ensure_i18n();
        let base = notefield_bench::fixture();
        let song = base.state().song.clone();

        profile::set_session_play_style(PlayStyle::Single);
        profile::set_session_player_side(PlayerSide::P1);
        profile::set_session_joined(true, false);

        let mut state = super::super::init(song, [0; 2], [0; 2], 1, Screen::SelectMusic, None);

        // JudgmentOffsetX is a NumericRow on the Main pane backed by the
        // hud-offset choices array (centered at HUD_OFFSET_ZERO_INDEX).
        let initial_x = state.player_profiles[P1].judgment_offset_x;

        let outcome = dispatch_for_test(&mut state, P1, RowId::JudgmentOffsetX, TestAction::Delta(1));
        assert!(outcome.persisted, "numeric delta should report persisted");
        assert!(
            !outcome.changed_visibility,
            "numeric delta should not affect visibility"
        );
        assert_eq!(state.player_profiles[P1].judgment_offset_x, initial_x + 1);

        let outcome = dispatch_for_test(&mut state, P1, RowId::JudgmentOffsetX, TestAction::Delta(-1));
        assert!(outcome.persisted);
        assert_eq!(state.player_profiles[P1].judgment_offset_x, initial_x);
    }

    /// Builds a fresh `State` with a Single/P1 session — the standard
    /// fixture for `dispatch_for_test` exercises.
    fn fresh_state() -> super::super::State {
        ensure_i18n();
        let base = notefield_bench::fixture();
        let song = base.state().song.clone();
        profile::set_session_play_style(PlayStyle::Single);
        profile::set_session_player_side(PlayerSide::P1);
        profile::set_session_joined(true, false);
        super::super::init(song, [0; 2], [0; 2], 1, Screen::SelectMusic, None)
    }

    #[test]
    fn dispatch_for_test_drives_cycle_index_row_through_dispatcher() {
        let mut state = fresh_state();

        // Perspective is RowKind::Cycle(CycleBinding::Index(&PERSPECTIVE)).
        // PERSPECTIVE_VARIANTS = [Overhead, Hallway, Distant, Incoming, Space].
        use crate::game::profile::Perspective;
        state.player_profiles[P1].perspective = Perspective::Overhead;

        let outcome = dispatch_for_test(&mut state, P1, RowId::Perspective, TestAction::Delta(1));
        assert!(outcome.persisted, "cycle delta should persist");
        assert!(!outcome.changed_visibility);
        assert_eq!(state.player_profiles[P1].perspective, Perspective::Hallway);

        let outcome = dispatch_for_test(&mut state, P1, RowId::Perspective, TestAction::Delta(-1));
        assert!(outcome.persisted);
        assert_eq!(state.player_profiles[P1].perspective, Perspective::Overhead);
    }

    #[test]
    fn dispatch_for_test_drives_cycle_bool_row_through_dispatcher() {
        let mut state = fresh_state();

        // DensityGraphBackground is on the Advanced pane and is
        // RowKind::Cycle(CycleBinding::Bool(&DENSITY_GRAPH_BACKGROUND)).
        // The helper switches `current_pane` automatically.
        let initial = state.player_profiles[P1].transparent_density_graph_bg;

        let outcome = dispatch_for_test(
            &mut state,
            P1,
            RowId::DensityGraphBackground,
            TestAction::Delta(1),
        );
        assert!(outcome.persisted, "bool cycle delta should persist");
        assert_eq!(
            state.player_profiles[P1].transparent_density_graph_bg,
            !initial
        );

        let outcome = dispatch_for_test(
            &mut state,
            P1,
            RowId::DensityGraphBackground,
            TestAction::Delta(-1),
        );
        assert!(outcome.persisted);
        assert_eq!(
            state.player_profiles[P1].transparent_density_graph_bg,
            initial
        );
    }

    #[test]
    fn dispatch_for_test_bitmask_delta_only_moves_focus() {
        let mut state = fresh_state();

        // Scroll is RowKind::Bitmask(&SCROLL). Delta moves the focus
        // cursor along the row's choices but must NOT mutate the profile —
        // mask flips happen on Toggle (Start key), not Delta (L/R).
        use crate::game::profile::ScrollOption;
        let before = state.player_profiles[P1].scroll_option;

        let outcome = dispatch_for_test(&mut state, P1, RowId::Scroll, TestAction::Delta(1));
        assert!(
            outcome.persisted,
            "bitmask delta reports persisted to drive the focus-move SFX"
        );
        assert_eq!(
            state.player_profiles[P1].scroll_option, before,
            "delta on a bitmask row must not flip any bit"
        );
        // Cursor should now be on the second choice.
        let row_idx = state
            .rows()
            .iter()
            .position(|r| r.id == RowId::Scroll)
            .unwrap();
        assert_eq!(state.rows()[row_idx].selected_choice_index[P1], 1);

        // Sanity: ScrollOption is still its default after just moving the
        // cursor (no toggles fired).
        assert_eq!(
            state.player_profiles[P1].scroll_option,
            ScrollOption::Normal
        );
    }

    #[test]
    fn dispatch_for_test_bitmask_toggle_flips_focused_bit() {
        let mut state = fresh_state();

        // Scroll bit 0 is Reverse. Fresh profile defaults to no scroll
        // options. selected_choice_index defaults to 0 → Reverse focused.
        use crate::game::profile::ScrollOption;
        assert_eq!(
            state.player_profiles[P1].scroll_option,
            ScrollOption::Normal
        );
        assert!(!state.player_profiles[P1].reverse_scroll);

        let outcome = dispatch_for_test(&mut state, P1, RowId::Scroll, TestAction::Toggle);
        assert!(outcome.persisted, "bitmask toggle should persist");

        assert!(
            state.player_profiles[P1]
                .scroll_option
                .contains(ScrollOption::Reverse),
            "Reverse bit should be set after toggling bit 0"
        );
        assert!(state.player_profiles[P1].reverse_scroll);

        // Toggling again clears it.
        let outcome = dispatch_for_test(&mut state, P1, RowId::Scroll, TestAction::Toggle);
        assert!(outcome.persisted);
        assert!(
            !state.player_profiles[P1]
                .scroll_option
                .contains(ScrollOption::Reverse)
        );
        assert!(!state.player_profiles[P1].reverse_scroll);
    }

    #[test]
    fn dispatch_for_test_action_exit_is_inert() {
        let mut state = fresh_state();

        // ActionRow::Exit's behavior is wired into the input layer (Start
        // press leaves the screen). The choice dispatcher itself reports
        // Outcome::NONE for both Delta and Toggle.
        let outcome = dispatch_for_test(&mut state, P1, RowId::Exit, TestAction::Delta(1));
        assert!(!outcome.persisted);
        assert!(!outcome.changed_visibility);

        let outcome = dispatch_for_test(&mut state, P1, RowId::Exit, TestAction::Toggle);
        assert!(!outcome.persisted);
        assert!(!outcome.changed_visibility);
    }

    #[test]
    fn dispatch_for_test_action_what_comes_next_advances_choice_index() {
        let mut state = fresh_state();

        // ActionRow::WhatComesNext uses the same cursor-advancing path as
        // a bitmask: Delta(1) bumps selected_choice_index without writing
        // any Profile field.
        let row_idx = state
            .rows()
            .iter()
            .position(|r| r.id == RowId::WhatComesNext)
            .unwrap();
        let before = state.rows()[row_idx].selected_choice_index[P1];
        let n_choices = state.rows()[row_idx].choices.len();
        assert!(n_choices >= 2, "WhatComesNext should expose >=2 choices");

        let outcome = dispatch_for_test(
            &mut state,
            P1,
            RowId::WhatComesNext,
            TestAction::Delta(1),
        );
        assert!(outcome.persisted);
        assert_eq!(
            state.rows()[row_idx].selected_choice_index[P1],
            (before + 1) % n_choices
        );
    }

    #[test]
    fn dispatch_for_test_drives_noteskin_cycle_through_dispatcher() {
        let mut state = fresh_state();

        // NoteSkin is RowKind::Cycle(CycleBinding::NoteSkin(&NOTE_SKIN)).
        // The dispatcher snapshots the chosen choice string and feeds it
        // into NOTE_SKIN.apply, which mutates `Profile.noteskin`.
        let row_idx = state
            .rows()
            .iter()
            .position(|r| r.id == RowId::NoteSkin)
            .unwrap();
        let n_choices = state.rows()[row_idx].choices.len();
        assert!(n_choices >= 1, "NoteSkin row should always have a choice");
        let before_index = state.rows()[row_idx].selected_choice_index[P1];
        let expected_index = (before_index + 1) % n_choices;

        let outcome = dispatch_for_test(&mut state, P1, RowId::NoteSkin, TestAction::Delta(1));
        assert!(outcome.persisted, "NoteSkin cycle delta should persist");

        let new_index = state.rows()[row_idx].selected_choice_index[P1];
        assert_eq!(new_index, expected_index);
        let expected_name = state.rows()[row_idx].choices[new_index].clone();
        assert_eq!(
            state.player_profiles[P1].noteskin,
            crate::game::profile::NoteSkin::new(&expected_name),
        );
    }

    #[test]
    fn dispatch_for_test_drives_custom_mini_row_through_dispatcher() {
        let mut state = fresh_state();

        // Mini is RowKind::Custom(&MINI). Its choices are "-100%".."150%".
        // Delta(+1) cycles the choice and parses it into mini_percent.
        let row_idx = state
            .rows()
            .iter()
            .position(|r| r.id == RowId::Mini)
            .unwrap();
        let before_index = state.rows()[row_idx].selected_choice_index[P1];
        let n_choices = state.rows()[row_idx].choices.len();
        let next_index = (before_index + 1) % n_choices;
        let expected: i32 = state.rows()[row_idx].choices[next_index]
            .trim_end_matches('%')
            .parse()
            .expect("Mini choices are integer percentages");

        let outcome = dispatch_for_test(&mut state, P1, RowId::Mini, TestAction::Delta(1));
        assert!(outcome.persisted, "custom Mini delta should persist");
        assert!(!outcome.changed_visibility);
        assert_eq!(state.player_profiles[P1].mini_percent, expected);
    }

    #[test]
    fn dispatch_for_test_custom_mini_indicator_reports_visibility_change() {
        let mut state = fresh_state();

        // MiniIndicator is RowKind::Custom(&MINI_INDICATOR) on the
        // Advanced pane. Its apply returns persisted_with_visibility(),
        // so the helper should also re-run the visibility sync without
        // panicking. The dispatcher switches `current_pane` for us.
        use crate::game::profile::MiniIndicator;
        assert_eq!(
            state.player_profiles[P1].mini_indicator,
            MiniIndicator::None,
        );

        let outcome = dispatch_for_test(
            &mut state,
            P1,
            RowId::MiniIndicator,
            TestAction::Delta(1),
        );
        assert!(outcome.persisted);
        assert!(
            outcome.changed_visibility,
            "MiniIndicator delta should report a visibility change"
        );
        // Delta(+1) from index 0 (None) advances to SubtractiveScoring.
        assert_eq!(
            state.player_profiles[P1].mini_indicator,
            MiniIndicator::SubtractiveScoring,
        );
        assert!(state.player_profiles[P1].subtractive_scoring);
    }

    #[test]
    fn dispatch_for_test_bitmask_hide_toggle_flips_focused_bit() {
        let mut state = fresh_state();

        // Hide bit 0 is hide_targets. Fresh profile defaults all hide
        // flags to false; selected_choice_index defaults to 0 → that bit
        // is focused.
        assert!(!state.player_profiles[P1].hide_targets);

        let outcome = dispatch_for_test(&mut state, P1, RowId::Hide, TestAction::Toggle);
        assert!(outcome.persisted, "hide toggle should persist");
        assert!(state.player_profiles[P1].hide_targets);

        let outcome = dispatch_for_test(&mut state, P1, RowId::Hide, TestAction::Toggle);
        assert!(outcome.persisted);
        assert!(!state.player_profiles[P1].hide_targets);
    }
}
