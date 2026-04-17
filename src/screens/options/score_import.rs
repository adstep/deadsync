use crate::config;
use crate::engine::audio;
use crate::game::{course, profile, scores};
use crate::screens::{Screen, ScreenAction};
use crate::screens::pack_sync as shared_pack_sync;
use crate::screens::select_music;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use super::*;

pub fn start_reload_songs_and_courses(state: &mut State) {
    if state.reload_ui.is_some() {
        return;
    }

    // Clear navigation holds so the menu can't "run away" after reload finishes.
    clear_navigation_holds(state);

    let (tx, rx) = std::sync::mpsc::channel::<ReloadMsg>();
    state.reload_ui = Some(ReloadUiState::new(rx));

    std::thread::spawn(move || {
        let _ = tx.send(ReloadMsg::Phase(ReloadPhase::Songs));

        let mut on_song = |done: usize, total: usize, pack: &str, song: &str| {
            let _ = tx.send(ReloadMsg::Song {
                done,
                total,
                pack: pack.to_owned(),
                song: song.to_owned(),
            });
        };
        song_loading::scan_and_load_songs_with_progress_counts(
            &dirs::app_dirs().songs_dir(),
            &mut on_song,
        );

        let _ = tx.send(ReloadMsg::Phase(ReloadPhase::Courses));

        let mut on_course = |done: usize, total: usize, group: &str, course: &str| {
            let _ = tx.send(ReloadMsg::Course {
                done,
                total,
                group: group.to_owned(),
                course: course.to_owned(),
            });
        };
        let dirs = dirs::app_dirs();
        course::scan_and_load_courses_with_progress_counts(
            &dirs.courses_dir(),
            &dirs.songs_dir(),
            &mut on_course,
        );

        let _ = tx.send(ReloadMsg::Done);
    });
}

pub fn begin_score_import(state: &mut State, selection: ScoreImportSelection) {
    if state.score_import_ui.is_some() {
        return;
    }
    clear_navigation_holds(state);
    let mut profile_cfg = profile::Profile::default();
    profile_cfg
        .display_name
        .clone_from(&selection.profile.display_name);
    profile_cfg
        .groovestats_api_key
        .clone_from(&selection.profile.gs_api_key);
    profile_cfg
        .groovestats_username
        .clone_from(&selection.profile.gs_username);
    profile_cfg
        .arrowcloud_api_key
        .clone_from(&selection.profile.ac_api_key);

    let endpoint = selection.endpoint;
    let profile_id = selection.profile.id.clone();
    let profile_name = if selection.profile.display_name.is_empty() {
        selection.profile.id.clone()
    } else {
        selection.profile.display_name.clone()
    };
    let pack_group = selection.pack_group.clone();
    let pack_label = selection.pack_label.clone();
    let only_missing_gs_scores = selection.only_missing_gs_scores;

    log::warn!(
        "{} score import starting for '{}' (pack: {}, only_missing_gs={}). Hard-limited to 3 requests/sec. For many charts this can take more than one hour.",
        endpoint.display_name(),
        profile_name,
        pack_label,
        if only_missing_gs_scores { "yes" } else { "no" }
    );

    let cancel_requested = Arc::new(AtomicBool::new(false));
    let cancel_for_thread = Arc::clone(&cancel_requested);
    let (tx, rx) = std::sync::mpsc::channel::<ScoreImportMsg>();
    state.score_import_ui = Some(ScoreImportUiState::new(
        endpoint,
        profile_name.clone(),
        pack_label,
        cancel_requested,
        rx,
    ));

    std::thread::spawn(move || {
        let result = scores::import_scores_for_profile(
            endpoint,
            profile_id,
            profile_cfg,
            pack_group,
            only_missing_gs_scores,
            |progress| {
                let _ = tx.send(ScoreImportMsg::Progress(progress));
            },
            || cancel_for_thread.load(Ordering::Relaxed),
        );
        let done_msg = result.map_err(|e| e.to_string());
        let _ = tx.send(ScoreImportMsg::Done(done_msg));
    });
}

pub fn begin_score_import_from_confirm(state: &mut State) {
    let Some(confirm) = state.score_import_confirm.take() else {
        return;
    };
    begin_score_import(state, confirm.selection);
}

#[inline(always)]
pub fn sync_pack_preferred_difficulty_index() -> usize {
    let profile_data = profile::get();
    let play_style = profile::get_session_play_style();
    let max_diff_index = color::FILE_DIFFICULTY_NAMES.len().saturating_sub(1);
    if max_diff_index == 0 {
        0
    } else {
        profile_data
            .last_played(play_style)
            .difficulty_index
            .min(max_diff_index)
    }
}

pub fn begin_pack_sync(state: &mut State, selection: SyncPackSelection) {
    if !matches!(
        state.pack_sync_overlay,
        shared_pack_sync::OverlayState::Hidden
    ) {
        return;
    }

    clear_navigation_holds(state);

    let target_chart_type = profile::get_session_play_style().chart_type();
    let preferred_difficulty_index = sync_pack_preferred_difficulty_index();
    let pack_group = selection.pack_group.as_deref();
    let song_cache = crate::game::song::get_song_cache();
    let mut targets = Vec::new();

    for pack in song_cache.iter() {
        if pack_group.is_some() && Some(pack.group_name.as_str()) != pack_group {
            continue;
        }
        for song in &pack.songs {
            let Some(steps_index) = select_music::best_steps_index(
                song.as_ref(),
                target_chart_type,
                preferred_difficulty_index,
            ) else {
                continue;
            };
            let Some(chart_ix) = select_music::selected_chart_ix_for_sync(
                song.as_ref(),
                target_chart_type,
                steps_index,
            ) else {
                continue;
            };
            let Some(chart) = song.charts.get(chart_ix) else {
                continue;
            };
            targets.push(shared_pack_sync::TargetSpec {
                simfile_path: song.simfile_path.clone(),
                song_title: song.display_full_title(false),
                chart_label: shared_pack_sync::chart_label(chart),
                chart_ix,
            });
        }
    }
    drop(song_cache);

    if !shared_pack_sync::begin(
        &mut state.pack_sync_overlay,
        selection.pack_label.clone(),
        targets,
    ) {
        log::warn!(
            "Failed to start pack sync for {:?}: no matching charts were found.",
            selection.pack_group
        );
    }
}

pub fn begin_pack_sync_from_confirm(state: &mut State) {
    let Some(confirm) = state.sync_pack_confirm.take() else {
        return;
    };
    begin_pack_sync(state, confirm.selection);
}

pub fn poll_reload_ui(reload: &mut ReloadUiState) {
    while let Ok(msg) = reload.rx.try_recv() {
        match msg {
            ReloadMsg::Phase(phase) => {
                reload.phase = phase;
                reload.line2.clear();
                reload.line3.clear();
            }
            ReloadMsg::Song {
                done,
                total,
                pack,
                song,
            } => {
                reload.phase = ReloadPhase::Songs;
                reload.songs_done = done;
                reload.songs_total = total;
                reload.line2 = pack;
                reload.line3 = song;
            }
            ReloadMsg::Course {
                done,
                total,
                group,
                course,
            } => {
                reload.phase = ReloadPhase::Courses;
                reload.courses_done = done;
                reload.courses_total = total;
                reload.line2 = group;
                reload.line3 = course;
            }
            ReloadMsg::Done => {
                reload.done = true;
            }
        }
    }
}

#[inline(always)]
pub fn reload_progress(reload: &ReloadUiState) -> (usize, usize, f32) {
    let done = reload.songs_done.saturating_add(reload.courses_done);
    let mut total = reload.songs_total.saturating_add(reload.courses_total);
    if total < done {
        total = done;
    }
    let mut progress = if total > 0 {
        (done as f32 / total as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    if !reload.done && total > 0 && progress >= 1.0 {
        progress = 0.999;
    }
    (done, total, progress)
}

pub fn reload_detail_lines(reload: &ReloadUiState) -> (String, String) {
    (reload.line2.clone(), reload.line3.clone())
}

pub fn build_reload_overlay_actors(reload: &ReloadUiState, active_color_index: i32) -> Vec<Actor> {
    let (done, total, progress) = reload_progress(reload);
    let elapsed = reload.started_at.elapsed().as_secs_f32().max(0.0);
    let count_text = if total == 0 {
        String::new()
    } else {
        crate::screens::progress_count_text(done, total)
    };
    let show_speed_row = total > 0;
    let speed_text = if elapsed > 0.0 && show_speed_row {
        tr_fmt(
            "SelectMusic",
            "LoadingSpeed",
            &[("speed", &format!("{:.1}", done as f32 / elapsed))],
        )
        .to_string()
    } else if show_speed_row {
        tr_fmt("SelectMusic", "LoadingSpeed", &[("speed", "0.0")]).to_string()
    } else {
        String::new()
    };
    let (line2, line3) = reload_detail_lines(reload);
    let fill = color::decorative_rgba(active_color_index);

    let bar_w = widescale(360.0, 520.0);
    let bar_h = RELOAD_BAR_H;
    let bar_cx = screen_width() * 0.5;
    let bar_cy = screen_height() * 0.5 + 34.0;
    let fill_w = (bar_w - 4.0) * progress.clamp(0.0, 1.0);

    let mut out: Vec<Actor> = Vec::with_capacity(7);
    out.push(act!(quad:
        align(0.0, 0.0):
        xy(0.0, 0.0):
        zoomto(screen_width(), screen_height()):
        diffuse(0.0, 0.0, 0.0, 0.65):
        z(300)
    ));
    let phase_label = match reload.phase {
        ReloadPhase::Songs => tr("Init", "LoadingSongsText"),
        ReloadPhase::Courses => tr("Init", "LoadingCoursesText"),
    };
    out.push(act!(text:
        font("miso"):
        settext(if total == 0 { tr("Init", "InitializingText") } else { phase_label }):
        align(0.5, 0.5):
        xy(screen_width() * 0.5, bar_cy - 98.0):
        zoom(1.05):
        horizalign(center):
        z(301)
    ));
    if !line2.is_empty() {
        out.push(act!(text:
            font("miso"):
            settext(line2):
            align(0.5, 0.5):
            xy(screen_width() * 0.5, bar_cy - 74.0):
            zoom(0.95):
            maxwidth(screen_width() * 0.9):
            horizalign(center):
            z(301)
        ));
    }
    if !line3.is_empty() {
        out.push(act!(text:
            font("miso"):
            settext(line3):
            align(0.5, 0.5):
            xy(screen_width() * 0.5, bar_cy - 50.0):
            zoom(0.95):
            maxwidth(screen_width() * 0.9):
            horizalign(center):
            z(301)
        ));
    }

    let mut bar_children = Vec::with_capacity(4);
    bar_children.push(act!(quad:
        align(0.5, 0.5):
        xy(bar_w / 2.0, bar_h / 2.0):
        zoomto(bar_w, bar_h):
        diffuse(1.0, 1.0, 1.0, 1.0):
        z(0)
    ));
    bar_children.push(act!(quad:
        align(0.5, 0.5):
        xy(bar_w / 2.0, bar_h / 2.0):
        zoomto(bar_w - 4.0, bar_h - 4.0):
        diffuse(0.0, 0.0, 0.0, 1.0):
        z(1)
    ));
    if fill_w > 0.0 {
        bar_children.push(act!(quad:
            align(0.0, 0.5):
            xy(2.0, bar_h / 2.0):
            zoomto(fill_w, bar_h - 4.0):
            diffuse(fill[0], fill[1], fill[2], 1.0):
            z(2)
        ));
    }
    bar_children.push(act!(text:
        font("miso"):
        settext(count_text):
        align(0.5, 0.5):
        xy(bar_w / 2.0, bar_h / 2.0):
        zoom(0.9):
        horizalign(center):
        z(3)
    ));
    out.push(Actor::Frame {
        align: [0.5, 0.5],
        offset: [bar_cx, bar_cy],
        size: [actors::SizeSpec::Px(bar_w), actors::SizeSpec::Px(bar_h)],
        background: None,
        z: 301,
        children: bar_children,
    });

    if show_speed_row {
        out.push(act!(text:
            font("miso"):
            settext(speed_text):
            align(0.5, 0.5):
            xy(screen_width() * 0.5, bar_cy + 36.0):
            zoom(0.9):
            horizalign(center):
            z(301)
        ));
    }
    out
}

pub fn poll_score_import_ui(score_import: &mut ScoreImportUiState) {
    while let Ok(msg) = score_import.rx.try_recv() {
        match msg {
            ScoreImportMsg::Progress(progress) => {
                score_import.total_charts = progress.total_charts;
                score_import.processed_charts = progress.processed_charts;
                score_import.imported_scores = progress.imported_scores;
                score_import.missing_scores = progress.missing_scores;
                score_import.failed_requests = progress.failed_requests;
                score_import.detail_line = progress.detail;
            }
            ScoreImportMsg::Done(result) => {
                score_import.done = true;
                score_import.done_since = Some(Instant::now());
                score_import.done_message = match result {
                    Ok(summary) => {
                        if summary.canceled {
                            format!(
                                "Canceled: requested={}, imported={}, missing={}, failed={} (elapsed {:.1}s)",
                                summary.requested_charts,
                                summary.imported_scores,
                                summary.missing_scores,
                                summary.failed_requests,
                                summary.elapsed_seconds
                            )
                        } else {
                            format!(
                                "Complete: requested={}, imported={}, missing={}, failed={}, rate={} req/s (elapsed {:.1}s)",
                                summary.requested_charts,
                                summary.imported_scores,
                                summary.missing_scores,
                                summary.failed_requests,
                                summary.rate_limit_per_second,
                                summary.elapsed_seconds
                            )
                        }
                    }
                    Err(e) => tr_fmt(
                        "OptionsScoreImport",
                        "ImportFailed",
                        &[("error", &e.to_string())],
                    )
                    .to_string(),
                };
            }
        }
    }
}
