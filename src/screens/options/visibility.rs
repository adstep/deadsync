use super::*;

/// Returns `true` when the given submenu row should be treated as disabled
/// (non-interactive and visually dimmed). Add new cases here for any row
/// that should be conditionally locked based on runtime state.
pub(super) fn is_submenu_row_disabled(kind: SubmenuKind, id: SubRowId) -> bool {
    match (kind, id) {
        (SubmenuKind::InputBackend, SubRowId::MenuButtons) => {
            !crate::engine::input::any_player_has_dedicated_menu_buttons_for_mode(
                config::get().three_key_navigation,
            )
        }
        _ => false,
    }
}

pub(super) fn submenu_visible_row_indices(state: &State, kind: SubmenuKind, rows: &[SubRow]) -> Vec<usize> {
    match kind {
        SubmenuKind::Graphics => {
            let show_sw = graphics_show_software_threads(state);
            let show_present_mode = graphics_show_present_mode(state);
            let show_max_fps = graphics_show_max_fps(state);
            let show_max_fps_value = graphics_show_max_fps_value(state);
            let show_high_dpi = graphics_show_high_dpi(state);
            rows.iter()
                .enumerate()
                .filter_map(|(idx, row)| {
                    if row.id == SubRowId::SoftwareRendererThreads && !show_sw {
                        None
                    } else if row.id == SubRowId::PresentMode && !show_present_mode {
                        None
                    } else if row.id == SubRowId::MaxFps && !show_max_fps {
                        None
                    } else if row.id == SubRowId::MaxFpsValue && !show_max_fps_value {
                        None
                    } else if row.id == SubRowId::HighDpi && !show_high_dpi {
                        None
                    } else {
                        Some(idx)
                    }
                })
                .collect()
        }
        SubmenuKind::Advanced => rows.iter().enumerate().map(|(idx, _)| idx).collect(),
        SubmenuKind::SelectMusic => {
            let show_banners = get_choice_by_id(
                &state.sub[SubmenuKind::SelectMusic].choice_indices,
                SELECT_MUSIC_OPTIONS_ROWS,
                SubRowId::ShowBanners,
            ).unwrap_or_else(|| yes_no_choice_index(true));
            let show_banners = yes_no_from_choice(show_banners);
            let show_breakdown = get_choice_by_id(
                &state.sub[SubmenuKind::SelectMusic].choice_indices,
                SELECT_MUSIC_OPTIONS_ROWS,
                SubRowId::ShowBreakdown,
            ).unwrap_or_else(|| yes_no_choice_index(true));
            let show_breakdown = yes_no_from_choice(show_breakdown);
            let show_previews = get_choice_by_id(
                &state.sub[SubmenuKind::SelectMusic].choice_indices,
                SELECT_MUSIC_OPTIONS_ROWS,
                SubRowId::MusicPreviews,
            ).unwrap_or_else(|| yes_no_choice_index(true));
            let show_previews = yes_no_from_choice(show_previews);
            let show_scorebox = get_choice_by_id(
                &state.sub[SubmenuKind::SelectMusic].choice_indices,
                SELECT_MUSIC_OPTIONS_ROWS,
                SubRowId::ShowGsBox,
            ).unwrap_or_else(|| yes_no_choice_index(true));
            let show_scorebox = yes_no_from_choice(show_scorebox);
            rows.iter()
                .enumerate()
                .filter_map(|(idx, row)| {
                    if row.id == SubRowId::ShowVideoBanners && !show_banners {
                        None
                    } else if row.id == SubRowId::BreakdownStyle && !show_breakdown {
                        None
                    } else if row.id == SubRowId::LoopMusic && !show_previews {
                        None
                    } else if row.id == SubRowId::GsBoxPlacement && !show_scorebox {
                        None
                    } else if row.id == SubRowId::GsBoxLeaderboards && !show_scorebox {
                        None
                    } else {
                        Some(idx)
                    }
                })
                .collect()
        }
        SubmenuKind::Machine => {
            let show_preferred_style = get_choice_by_id(
                &state.sub[SubmenuKind::Machine].choice_indices,
                MACHINE_OPTIONS_ROWS,
                SubRowId::SelectStyle,
            ).unwrap_or(1)
                == 0;
            let show_preferred_mode = get_choice_by_id(
                &state.sub[SubmenuKind::Machine].choice_indices,
                MACHINE_OPTIONS_ROWS,
                SubRowId::SelectPlayMode,
            ).unwrap_or(1)
                == 0;
            rows.iter()
                .enumerate()
                .filter_map(|(idx, row)| {
                    if row.id == SubRowId::PreferredStyle && !show_preferred_style {
                        None
                    } else if row.id == SubRowId::PreferredMode && !show_preferred_mode {
                        None
                    } else {
                        Some(idx)
                    }
                })
                .collect()
        }
        #[cfg(target_os = "linux")]
        SubmenuKind::Sound => rows
            .iter()
            .enumerate()
            .filter_map(|(idx, row)| {
                if row.id == SubRowId::AlsaExclusive && !sound_show_alsa_exclusive(state) {
                    None
                } else {
                    Some(idx)
                }
            })
            .collect(),
        _ => (0..rows.len()).collect(),
    }
}

pub(super) fn submenu_total_rows(state: &State, kind: SubmenuKind) -> usize {
    let rows = submenu_rows(kind);
    submenu_visible_row_indices(state, kind, rows).len() + 1
}

pub(super) fn submenu_visible_row_to_actual(
    state: &State,
    kind: SubmenuKind,
    visible_row_idx: usize,
) -> Option<usize> {
    let rows = submenu_rows(kind);
    let visible_rows = submenu_visible_row_indices(state, kind, rows);
    visible_rows.get(visible_row_idx).copied()
}

#[cfg(target_os = "windows")]
pub(super) const fn windows_backend_choice_index(backend: WindowsPadBackend) -> usize {
    match backend {
        WindowsPadBackend::Auto | WindowsPadBackend::RawInput => 0,
        WindowsPadBackend::Wgi => 1,
    }
}

#[cfg(target_os = "windows")]
pub(super) const fn windows_backend_from_choice(idx: usize) -> WindowsPadBackend {
    match idx {
        0 => WindowsPadBackend::RawInput,
        _ => WindowsPadBackend::Wgi,
    }
}

pub(super) fn submenu_choice_indices(state: &State, kind: SubmenuKind) -> &[usize] {
    &state.sub[kind].choice_indices
}

pub(super) fn submenu_choice_indices_mut(state: &mut State, kind: SubmenuKind) -> &mut Vec<usize> {
    &mut state.sub[kind].choice_indices
}

pub(super) fn submenu_cursor_indices(state: &State, kind: SubmenuKind) -> &[usize] {
    &state.sub[kind].cursor_indices
}

pub(super) fn submenu_cursor_indices_mut(state: &mut State, kind: SubmenuKind) -> &mut Vec<usize> {
    &mut state.sub[kind].cursor_indices
}

pub(super) fn sync_submenu_cursor_indices(state: &mut State) {
    for s in state.sub.iter_mut() {
        s.cursor_indices.clone_from(&s.choice_indices);
    }
}
