use crate::act;
use crate::assets::i18n::{tr, tr_fmt};
use crate::assets::{self, AssetManager};
use crate::engine::audio;
use crate::engine::present::actors::Actor;
use crate::engine::space::{
    screen_center_x, screen_center_y, screen_height, screen_width,
};
use crate::game::parsing::noteskin::{
    self, NUM_QUANTIZATIONS, NoteAnimPart, Noteskin, Quantization, SpriteSlot,
};
use crate::game::song::SongData;
use crate::screens::components::shared::heart_bg;
use crate::screens::components::shared::screen_bar::{
    AvatarParams, ScreenBarParams,
};
use crate::screens::input as screen_input;
use crate::screens::{Screen, ScreenAction};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

mod types;
mod noteskins;
mod rows;
mod profile;
mod visibility;
mod inline_nav;
mod change_choice;
mod toggle;
mod input;
mod render;

pub(crate) use types::*;
pub(crate) use noteskins::*;
pub(crate) use rows::*;
pub(crate) use profile::*;
pub(crate) use visibility::*;
pub(crate) use inline_nav::*;
pub(crate) use change_choice::*;
pub(crate) use toggle::*;
pub(crate) use input::*;
pub(crate) use render::*;

#[cfg(test)]
mod tests;

/* ---------------------------- transitions ---------------------------- */
pub(crate) const TRANSITION_IN_DURATION: f32 = 0.4;
pub(crate) const TRANSITION_OUT_DURATION: f32 = 0.4;

/* ----------------------------- cursor tweening ----------------------------- */
// Simply Love metrics.ini uses 0.1 for both [ScreenOptions] TweenSeconds and CursorTweenSeconds.
// Player Options row/cursor motion should keep this exact parity timing.
pub(crate) const SL_OPTION_ROW_TWEEN_SECONDS: f32 = 0.1;
pub(crate) const CURSOR_TWEEN_SECONDS: f32 = SL_OPTION_ROW_TWEEN_SECONDS;
pub(crate) const ROW_TWEEN_SECONDS: f32 = SL_OPTION_ROW_TWEEN_SECONDS;
// Simply Love [ScreenOptions] uses RowOnCommand/RowOffCommand with linear,0.2.
pub(crate) const PANE_FADE_SECONDS: f32 = 0.2;
pub(crate) const TAP_EXPLOSION_PREVIEW_SPEED: f32 = 0.7;
// Spacing between inline items in OptionRows (pixels at current zoom)
pub(crate) const INLINE_SPACING: f32 = 15.75;
pub(crate) const TILT_INTENSITY_MIN: f32 = 0.05;
pub(crate) const TILT_INTENSITY_MAX: f32 = 10.00;
pub(crate) const TILT_INTENSITY_STEP: f32 = 0.05;
pub(crate) const HUD_OFFSET_MIN: i32 = crate::game::profile::HUD_OFFSET_MIN;
pub(crate) const HUD_OFFSET_MAX: i32 = crate::game::profile::HUD_OFFSET_MAX;
pub(crate) const HUD_OFFSET_ZERO_INDEX: usize = (-HUD_OFFSET_MIN) as usize;

// Match Simply Love / ScreenOptions defaults.
pub(crate) const VISIBLE_ROWS: usize = 10;
pub(crate) const ROW_START_OFFSET: f32 = -164.0;
pub(crate) const ROW_HEIGHT: f32 = 33.0;
pub(crate) const TITLE_BG_WIDTH: f32 = 127.0;

pub struct State {
    pub song: Arc<SongData>,
    pub return_screen: Screen,
    pub fixed_stepchart: Option<FixedStepchart>,
    pub chart_steps_index: [usize; PLAYER_SLOTS],
    pub chart_difficulty_index: [usize; PLAYER_SLOTS],
    pub rows: Vec<Row>,
    pub selected_row: [usize; PLAYER_SLOTS],
    pub prev_selected_row: [usize; PLAYER_SLOTS],
    // For Scroll row: bitmask of which options are enabled.
    // 0 => Normal scroll (no special modifier).
    pub scroll_active_mask: [u8; PLAYER_SLOTS],
    // For Hide row: bitmask of which options are enabled.
    // bit0 = Targets, bit1 = Background, bit2 = Combo, bit3 = Life,
    // bit4 = Score, bit5 = Danger, bit6 = Combo Explosions.
    pub hide_active_mask: [u8; PLAYER_SLOTS],
    // For FA+ Options row: bitmask of which options are enabled.
    // bit0 = Display FA+ Window, bit1 = Display EX Score, bit2 = Display H.EX Score,
    // bit3 = Display FA+ Pane, bit4 = 10ms Blue Window, bit5 = 15/10ms Split.
    pub fa_plus_active_mask: [u8; PLAYER_SLOTS],
    // For Early Decent/Way Off Options row: bitmask of which options are enabled.
    // bit0 = Hide Judgments, bit1 = Hide NoteField Flash.
    pub early_dw_active_mask: [u8; PLAYER_SLOTS],
    // For Gameplay Extras row: bitmask of which options are enabled.
    // bit0 = Flash Column for Miss, bit1 = Density Graph at Top,
    // bit2 = Column Cues, bit3 = Display Scorebox.
    pub gameplay_extras_active_mask: [u8; PLAYER_SLOTS],
    // For Gameplay Extras (More) row: bitmask of which options are enabled.
    // bit0 = Column Cues, bit1 = Display Scorebox.
    pub gameplay_extras_more_active_mask: [u8; PLAYER_SLOTS],
    // For Results Extras row: bitmask of which options are enabled.
    // bit0 = Track Early Judgments.
    pub results_extras_active_mask: [u8; PLAYER_SLOTS],
    // For Life Bar Options row: bitmask of which options are enabled.
    // bit0 = Rainbow Max, bit1 = Responsive Colors, bit2 = Show Life Percentage.
    pub life_bar_options_active_mask: [u8; PLAYER_SLOTS],
    // For Error Bar row: bitmask of which options are enabled.
    // bit0 = Colorful, bit1 = Monochrome, bit2 = Text, bit3 = Highlight, bit4 = Average.
    pub error_bar_active_mask: [u8; PLAYER_SLOTS],
    // For Error Bar Options row: bitmask of which options are enabled.
    // bit0 = Move Up, bit1 = Multi-Tick (Simply Love semantics).
    pub error_bar_options_active_mask: [u8; PLAYER_SLOTS],
    // For Measure Counter Options row: bitmask of which options are enabled.
    // bit0 = Move Left, bit1 = Move Up, bit2 = Vertical Lookahead,
    // bit3 = Broken Run Total, bit4 = Run Timer.
    pub measure_counter_options_active_mask: [u8; PLAYER_SLOTS],
    // For Insert row: bitmask of enabled chart insert transforms.
    // bit0 = Wide, bit1 = Big, bit2 = Quick, bit3 = BMRize,
    // bit4 = Skippy, bit5 = Echo, bit6 = Stomp.
    pub insert_active_mask: [u8; PLAYER_SLOTS],
    // For Remove row: bitmask of enabled chart removal transforms.
    // bit0 = Little, bit1 = No Mines, bit2 = No Holds, bit3 = No Jumps,
    // bit4 = No Hands, bit5 = No Quads, bit6 = No Lifts, bit7 = No Fakes.
    pub remove_active_mask: [u8; PLAYER_SLOTS],
    // For Holds row: bitmask of enabled hold transforms.
    // bit0 = Planted, bit1 = Floored, bit2 = Twister,
    // bit3 = No Rolls, bit4 = Holds To Rolls.
    pub holds_active_mask: [u8; PLAYER_SLOTS],
    // For Accel Effects row: bitmask of enabled acceleration transforms.
    // bit0 = Boost, bit1 = Brake, bit2 = Wave, bit3 = Expand, bit4 = Boomerang.
    pub accel_effects_active_mask: [u8; PLAYER_SLOTS],
    // For Visual Effects row: bitmask of enabled visual transforms.
    // bit0 = Drunk, bit1 = Dizzy, bit2 = Confusion, bit3 = Big,
    // bit4 = Flip, bit5 = Invert, bit6 = Tornado, bit7 = Tipsy,
    // bit8 = Bumpy, bit9 = Beat.
    pub visual_effects_active_mask: [u16; PLAYER_SLOTS],
    // For Appearance Effects row: bitmask of enabled appearance transforms.
    // bit0 = Hidden, bit1 = Sudden, bit2 = Stealth, bit3 = Blink, bit4 = R.Vanish.
    pub appearance_effects_active_mask: [u8; PLAYER_SLOTS],
    pub active_color_index: i32,
    pub speed_mod: [SpeedMod; PLAYER_SLOTS],
    pub music_rate: f32,
    pub current_pane: OptionsPane,
    pub scroll_focus_player: usize,
    bg: heart_bg::State,
    pub nav_key_held_direction: [Option<NavDirection>; PLAYER_SLOTS],
    pub nav_key_held_since: [Option<Instant>; PLAYER_SLOTS],
    pub nav_key_last_scrolled_at: [Option<Instant>; PLAYER_SLOTS],
    pub start_held_since: [Option<Instant>; PLAYER_SLOTS],
    pub start_last_triggered_at: [Option<Instant>; PLAYER_SLOTS],
    inline_choice_x: [f32; PLAYER_SLOTS],
    arcade_row_focus: [bool; PLAYER_SLOTS],
    allow_per_player_global_offsets: bool,
    pub player_profiles: [crate::game::profile::Profile; PLAYER_SLOTS],
    noteskin_names: Vec<String>,
    noteskin_cache: HashMap<String, Arc<Noteskin>>,
    noteskin: [Option<Arc<Noteskin>>; PLAYER_SLOTS],
    mine_noteskin: [Option<Arc<Noteskin>>; PLAYER_SLOTS],
    receptor_noteskin: [Option<Arc<Noteskin>>; PLAYER_SLOTS],
    tap_explosion_noteskin: [Option<Arc<Noteskin>>; PLAYER_SLOTS],
    preview_time: f32,
    preview_beat: f32,
    help_anim_time: [f32; PLAYER_SLOTS],
    // Combo preview state (for Combo Font row)
    combo_preview_count: u32,
    combo_preview_elapsed: f32,
    // Cursor ring tween (StopTweening/BeginTweening parity with ITGmania ScreenOptions::TweenCursor).
    cursor_initialized: [bool; PLAYER_SLOTS],
    cursor_from_x: [f32; PLAYER_SLOTS],
    cursor_from_y: [f32; PLAYER_SLOTS],
    cursor_from_w: [f32; PLAYER_SLOTS],
    cursor_from_h: [f32; PLAYER_SLOTS],
    cursor_to_x: [f32; PLAYER_SLOTS],
    cursor_to_y: [f32; PLAYER_SLOTS],
    cursor_to_w: [f32; PLAYER_SLOTS],
    cursor_to_h: [f32; PLAYER_SLOTS],
    cursor_t: [f32; PLAYER_SLOTS],
    row_tweens: Vec<RowTween>,
    pane_transition: PaneTransition,
    menu_lr_chord: screen_input::MenuLrChordTracker,
}

// Format music rate like Simply Love wants:

#[inline(always)]



pub fn init(
    song: Arc<SongData>,
    chart_steps_index: [usize; PLAYER_SLOTS],
    preferred_difficulty_index: [usize; PLAYER_SLOTS],
    active_color_index: i32,
    return_screen: Screen,
    fixed_stepchart: Option<FixedStepchart>,
) -> State {
    let session_music_rate = crate::game::profile::get_session_music_rate();
    let allow_per_player_global_offsets =
        crate::config::get().machine_allow_per_player_global_offsets;
    let p1_profile = crate::game::profile::get_for_side(crate::game::profile::PlayerSide::P1);
    let p2_profile = crate::game::profile::get_for_side(crate::game::profile::PlayerSide::P2);

    let speed_mod_p1 = match p1_profile.scroll_speed {
        crate::game::scroll::ScrollSpeedSetting::CMod(bpm) => SpeedMod {
            mod_type: "C".to_string(),
            value: bpm,
        },
        crate::game::scroll::ScrollSpeedSetting::XMod(mult) => SpeedMod {
            mod_type: "X".to_string(),
            value: mult,
        },
        crate::game::scroll::ScrollSpeedSetting::MMod(bpm) => SpeedMod {
            mod_type: "M".to_string(),
            value: bpm,
        },
    };
    let speed_mod_p2 = match p2_profile.scroll_speed {
        crate::game::scroll::ScrollSpeedSetting::CMod(bpm) => SpeedMod {
            mod_type: "C".to_string(),
            value: bpm,
        },
        crate::game::scroll::ScrollSpeedSetting::XMod(mult) => SpeedMod {
            mod_type: "X".to_string(),
            value: mult,
        },
        crate::game::scroll::ScrollSpeedSetting::MMod(bpm) => SpeedMod {
            mod_type: "M".to_string(),
            value: bpm,
        },
    };
    let chart_difficulty_index: [usize; PLAYER_SLOTS] = std::array::from_fn(|player_idx| {
        let steps_idx = chart_steps_index[player_idx];
        let mut diff_idx = preferred_difficulty_index[player_idx].min(
            crate::engine::present::color::FILE_DIFFICULTY_NAMES
                .len()
                .saturating_sub(1),
        );
        if steps_idx < crate::engine::present::color::FILE_DIFFICULTY_NAMES.len() {
            diff_idx = steps_idx;
        }
        diff_idx
    });

    let noteskin_names = discover_noteskin_names();
    let mut rows = build_rows(
        &song,
        &speed_mod_p1,
        chart_steps_index,
        preferred_difficulty_index,
        session_music_rate,
        OptionsPane::Main,
        &noteskin_names,
        return_screen,
        fixed_stepchart.as_ref(),
    );
    let player_profiles = [p1_profile.clone(), p2_profile.clone()];
    let (
        scroll_active_mask_p1,
        hide_active_mask_p1,
        insert_active_mask_p1,
        remove_active_mask_p1,
        holds_active_mask_p1,
        accel_effects_active_mask_p1,
        visual_effects_active_mask_p1,
        appearance_effects_active_mask_p1,
        fa_plus_active_mask_p1,
        early_dw_active_mask_p1,
        gameplay_extras_active_mask_p1,
        gameplay_extras_more_active_mask_p1,
        results_extras_active_mask_p1,
        life_bar_options_active_mask_p1,
        error_bar_active_mask_p1,
        error_bar_options_active_mask_p1,
        measure_counter_options_active_mask_p1,
    ) = apply_profile_defaults(&mut rows, &player_profiles[P1], P1);
    let (
        scroll_active_mask_p2,
        hide_active_mask_p2,
        insert_active_mask_p2,
        remove_active_mask_p2,
        holds_active_mask_p2,
        accel_effects_active_mask_p2,
        visual_effects_active_mask_p2,
        appearance_effects_active_mask_p2,
        fa_plus_active_mask_p2,
        early_dw_active_mask_p2,
        gameplay_extras_active_mask_p2,
        gameplay_extras_more_active_mask_p2,
        results_extras_active_mask_p2,
        life_bar_options_active_mask_p2,
        error_bar_active_mask_p2,
        error_bar_options_active_mask_p2,
        measure_counter_options_active_mask_p2,
    ) = apply_profile_defaults(&mut rows, &player_profiles[P2], P2);

    let cols_per_player = noteskin_cols_per_player(crate::game::profile::get_session_play_style());
    let mut initial_noteskin_names = vec![crate::game::profile::NoteSkin::DEFAULT_NAME.to_string()];
    for profile in &player_profiles {
        push_noteskin_name_once(&mut initial_noteskin_names, &profile.noteskin);
        if let Some(skin) = profile.mine_noteskin.as_ref() {
            push_noteskin_name_once(&mut initial_noteskin_names, skin);
        }
        if let Some(skin) = profile.receptor_noteskin.as_ref() {
            push_noteskin_name_once(&mut initial_noteskin_names, skin);
        }
        if let Some(skin) = profile.tap_explosion_noteskin.as_ref() {
            push_noteskin_name_once(&mut initial_noteskin_names, skin);
        }
    }
    let mut noteskin_cache = build_noteskin_cache(cols_per_player, &initial_noteskin_names);
    let noteskin_previews: [Option<Arc<Noteskin>>; PLAYER_SLOTS] = std::array::from_fn(|i| {
        cached_or_load_noteskin(
            &mut noteskin_cache,
            &player_profiles[i].noteskin,
            cols_per_player,
        )
    });
    let mine_noteskin_previews: [Option<Arc<Noteskin>>; PLAYER_SLOTS] = std::array::from_fn(|i| {
        resolved_noteskin_override_preview(
            &mut noteskin_cache,
            &player_profiles[i].noteskin,
            player_profiles[i].mine_noteskin.as_ref(),
            cols_per_player,
        )
    });
    let receptor_noteskin_previews: [Option<Arc<Noteskin>>; PLAYER_SLOTS] =
        std::array::from_fn(|i| {
            resolved_noteskin_override_preview(
                &mut noteskin_cache,
                &player_profiles[i].noteskin,
                player_profiles[i].receptor_noteskin.as_ref(),
                cols_per_player,
            )
        });
    let tap_explosion_noteskin_previews: [Option<Arc<Noteskin>>; PLAYER_SLOTS] =
        std::array::from_fn(|i| {
            resolved_tap_explosion_preview(
                &mut noteskin_cache,
                &player_profiles[i].noteskin,
                player_profiles[i].tap_explosion_noteskin.as_ref(),
                cols_per_player,
            )
        });
    let active = session_active_players();
    let row_tweens = init_row_tweens(
        &rows,
        [0; PLAYER_SLOTS],
        active,
        [hide_active_mask_p1, hide_active_mask_p2],
        [error_bar_active_mask_p1, error_bar_active_mask_p2],
        allow_per_player_global_offsets,
    );
    State {
        song,
        return_screen,
        fixed_stepchart,
        chart_steps_index,
        chart_difficulty_index,
        rows,
        selected_row: [0; PLAYER_SLOTS],
        prev_selected_row: [0; PLAYER_SLOTS],
        scroll_active_mask: [scroll_active_mask_p1, scroll_active_mask_p2],
        hide_active_mask: [hide_active_mask_p1, hide_active_mask_p2],
        insert_active_mask: [insert_active_mask_p1, insert_active_mask_p2],
        remove_active_mask: [remove_active_mask_p1, remove_active_mask_p2],
        holds_active_mask: [holds_active_mask_p1, holds_active_mask_p2],
        accel_effects_active_mask: [accel_effects_active_mask_p1, accel_effects_active_mask_p2],
        visual_effects_active_mask: [visual_effects_active_mask_p1, visual_effects_active_mask_p2],
        appearance_effects_active_mask: [
            appearance_effects_active_mask_p1,
            appearance_effects_active_mask_p2,
        ],
        fa_plus_active_mask: [fa_plus_active_mask_p1, fa_plus_active_mask_p2],
        early_dw_active_mask: [early_dw_active_mask_p1, early_dw_active_mask_p2],
        gameplay_extras_active_mask: [
            gameplay_extras_active_mask_p1,
            gameplay_extras_active_mask_p2,
        ],
        gameplay_extras_more_active_mask: [
            gameplay_extras_more_active_mask_p1,
            gameplay_extras_more_active_mask_p2,
        ],
        results_extras_active_mask: [results_extras_active_mask_p1, results_extras_active_mask_p2],
        life_bar_options_active_mask: [
            life_bar_options_active_mask_p1,
            life_bar_options_active_mask_p2,
        ],
        error_bar_active_mask: [error_bar_active_mask_p1, error_bar_active_mask_p2],
        error_bar_options_active_mask: [
            error_bar_options_active_mask_p1,
            error_bar_options_active_mask_p2,
        ],
        measure_counter_options_active_mask: [
            measure_counter_options_active_mask_p1,
            measure_counter_options_active_mask_p2,
        ],
        active_color_index,
        speed_mod: [speed_mod_p1, speed_mod_p2],
        music_rate: session_music_rate,
        current_pane: OptionsPane::Main,
        scroll_focus_player: P1,
        bg: heart_bg::State::new(),
        nav_key_held_direction: [None; PLAYER_SLOTS],
        nav_key_held_since: [None; PLAYER_SLOTS],
        nav_key_last_scrolled_at: [None; PLAYER_SLOTS],
        start_held_since: [None; PLAYER_SLOTS],
        start_last_triggered_at: [None; PLAYER_SLOTS],
        inline_choice_x: [f32::NAN; PLAYER_SLOTS],
        arcade_row_focus: [true; PLAYER_SLOTS],
        allow_per_player_global_offsets,
        player_profiles,
        noteskin_names,
        noteskin_cache,
        noteskin: noteskin_previews,
        mine_noteskin: mine_noteskin_previews,
        receptor_noteskin: receptor_noteskin_previews,
        tap_explosion_noteskin: tap_explosion_noteskin_previews,
        preview_time: 0.0,
        preview_beat: 0.0,
        help_anim_time: [0.0; PLAYER_SLOTS],
        combo_preview_count: 0,
        combo_preview_elapsed: 0.0,
        cursor_initialized: [false; PLAYER_SLOTS],
        cursor_from_x: [0.0; PLAYER_SLOTS],
        cursor_from_y: [0.0; PLAYER_SLOTS],
        cursor_from_w: [0.0; PLAYER_SLOTS],
        cursor_from_h: [0.0; PLAYER_SLOTS],
        cursor_to_x: [0.0; PLAYER_SLOTS],
        cursor_to_y: [0.0; PLAYER_SLOTS],
        cursor_to_w: [0.0; PLAYER_SLOTS],
        cursor_to_h: [0.0; PLAYER_SLOTS],
        cursor_t: [1.0; PLAYER_SLOTS],
        row_tweens,
        pane_transition: PaneTransition::None,
        menu_lr_chord: screen_input::MenuLrChordTracker::default(),
    }
}

pub fn in_transition() -> (Vec<Actor>, f32) {
    let actor = act!(quad:
        align(0.0, 0.0): xy(0.0, 0.0):
        zoomto(screen_width(), screen_height()):
        diffuse(0.0, 0.0, 0.0, 1.0):
        z(1100):
        linear(TRANSITION_IN_DURATION): alpha(0.0):
        linear(0.0): visible(false)
    );
    (vec![actor], TRANSITION_IN_DURATION)
}

pub fn out_transition() -> (Vec<Actor>, f32) {
    let actor = act!(quad:
        align(0.0, 0.0): xy(0.0, 0.0):
        zoomto(screen_width(), screen_height()):
        diffuse(0.0, 0.0, 0.0, 0.0):
        z(1200):
        linear(TRANSITION_OUT_DURATION): alpha(1.0)
    );
    (vec![actor], TRANSITION_OUT_DURATION)
}

#[inline(always)]



// Keyboard input is handled centrally via the virtual dispatcher in app
pub fn update(state: &mut State, dt: f32, asset_manager: &AssetManager) -> Option<ScreenAction> {
    // Keep options-screen noteskin previews on a stable clock.
    // ITG/SL preview actors are not driven by selected chart BPM, so tying this to song BPM
    // makes beat-based skins (e.g. cel) appear too fast/slow depending on the selected chart.
    const PREVIEW_BPM: f32 = 120.0;
    state.preview_time += dt;
    state.preview_beat += dt * (PREVIEW_BPM / 60.0);
    let active = session_active_players();
    let now = Instant::now();
    let arcade_style = crate::config::get().arcade_options_navigation;
    let mut pending_action: Option<ScreenAction> = None;
    sync_selected_rows_with_visibility(state, active);

    // Hold-to-scroll per player.
    for player_idx in active_player_indices(active) {
        let (Some(direction), Some(held_since), Some(last_scrolled_at)) = (
            state.nav_key_held_direction[player_idx],
            state.nav_key_held_since[player_idx],
            state.nav_key_last_scrolled_at[player_idx],
        ) else {
            continue;
        };
        if now.duration_since(held_since) <= NAV_INITIAL_HOLD_DELAY
            || now.duration_since(last_scrolled_at) < NAV_REPEAT_SCROLL_INTERVAL
        {
            continue;
        }

        if state.rows.is_empty() {
            continue;
        }
        match direction {
            NavDirection::Up => {
                move_selection_vertical(state, asset_manager, active, player_idx, NavDirection::Up);
            }
            NavDirection::Down => {
                move_selection_vertical(
                    state,
                    asset_manager,
                    active,
                    player_idx,
                    NavDirection::Down,
                );
            }
            NavDirection::Left => {
                if !move_arcade_horizontal_focus(state, asset_manager, player_idx, -1) {
                    apply_choice_delta(state, asset_manager, player_idx, -1);
                }
            }
            NavDirection::Right => {
                if !move_arcade_horizontal_focus(state, asset_manager, player_idx, 1) {
                    apply_choice_delta(state, asset_manager, player_idx, 1);
                }
            }
        }
        state.nav_key_last_scrolled_at[player_idx] = Some(now);
    }

    if arcade_style {
        for player_idx in active_player_indices(active) {
            let action = repeat_held_arcade_start(state, asset_manager, active, player_idx, now);
            if pending_action.is_none() {
                pending_action = action;
            }
        }
    }

    match state.pane_transition {
        PaneTransition::None => {}
        PaneTransition::FadingOut { target, t } => {
            if PANE_FADE_SECONDS <= 0.0 {
                apply_pane(state, target);
                state.pane_transition = PaneTransition::None;
            } else {
                let next_t = (t + dt / PANE_FADE_SECONDS).min(1.0);
                if next_t >= 1.0 {
                    apply_pane(state, target);
                    state.pane_transition = PaneTransition::FadingIn { t: 0.0 };
                } else {
                    state.pane_transition = PaneTransition::FadingOut { target, t: next_t };
                }
            }
        }
        PaneTransition::FadingIn { t } => {
            if PANE_FADE_SECONDS <= 0.0 {
                state.pane_transition = PaneTransition::None;
            } else {
                let next_t = (t + dt / PANE_FADE_SECONDS).min(1.0);
                if next_t >= 1.0 {
                    state.pane_transition = PaneTransition::None;
                } else {
                    state.pane_transition = PaneTransition::FadingIn { t: next_t };
                }
            }
        }
    }

    // Advance help reveal timers.
    for player_idx in active_player_indices(active) {
        state.help_anim_time[player_idx] += dt;
    }

    // If either player is on the Combo Font row, tick the preview combo once per second.
    let mut combo_row_active = false;
    for player_idx in active_player_indices(active) {
        if let Some(row) = state.rows.get(state.selected_row[player_idx])
            && row.id == RowId::ComboFont
        {
            combo_row_active = true;
            break;
        }
    }
    if combo_row_active {
        state.combo_preview_elapsed += dt;
        if state.combo_preview_elapsed >= 1.0 {
            state.combo_preview_elapsed -= 1.0;
            state.combo_preview_count = state.combo_preview_count.saturating_add(1);
        }
    } else {
        state.combo_preview_elapsed = 0.0;
    }

    // Row frame tweening: mimic ScreenOptions::PositionRows() + OptionRow::SetDestination()
    // so rows slide smoothly as the visible window scrolls.
    let total_rows = state.rows.len();
    let (first_row_center_y, row_step) = row_layout_params();
    if total_rows == 0 {
        state.row_tweens.clear();
    } else if state.row_tweens.len() != total_rows {
        state.row_tweens = init_row_tweens(
            &state.rows,
            state.selected_row,
            active,
            state.hide_active_mask,
            state.error_bar_active_mask,
            state.allow_per_player_global_offsets,
        );
    } else {
        let visibility = row_visibility(
            &state.rows,
            active,
            state.hide_active_mask,
            state.error_bar_active_mask,
            state.allow_per_player_global_offsets,
        );
        let visible_rows = count_visible_rows(&state.rows, visibility);
        if visible_rows == 0 {
            let y = first_row_center_y - row_step * 0.5;
            for tw in &mut state.row_tweens {
                let cur_y = tw.y();
                let cur_a = tw.a();
                if (y - tw.to_y).abs() > 0.01 || tw.to_a != 0.0 {
                    tw.from_y = cur_y;
                    tw.from_a = cur_a;
                    tw.to_y = y;
                    tw.to_a = 0.0;
                    tw.t = 0.0;
                }
                if tw.t < 1.0 {
                    if ROW_TWEEN_SECONDS > 0.0 {
                        tw.t = (tw.t + dt / ROW_TWEEN_SECONDS).min(1.0);
                    } else {
                        tw.t = 1.0;
                    }
                }
            }
        } else {
            let selected_visible = std::array::from_fn(|player_idx| {
                let row_idx = state.selected_row[player_idx].min(total_rows.saturating_sub(1));
                row_to_visible_index(&state.rows, row_idx, visibility).unwrap_or(0)
            });
            let w = compute_row_window(visible_rows, selected_visible, active);
            let mid_pos = (VISIBLE_ROWS as f32) * 0.5 - 0.5;
            let bottom_pos = (VISIBLE_ROWS as f32) - 0.5;
            let measure_counter_anchor_visible_idx =
                parent_anchor_visible_index(&state.rows, RowId::MeasureCounter, visibility);
            let judgment_tilt_anchor_visible_idx =
                parent_anchor_visible_index(&state.rows, RowId::JudgmentTilt, visibility);
            let error_bar_anchor_visible_idx =
                parent_anchor_visible_index(&state.rows, RowId::ErrorBar, visibility);
            let hide_anchor_visible_idx =
                parent_anchor_visible_index(&state.rows, RowId::Hide, visibility);
            let mut visible_idx = 0i32;
            for i in 0..total_rows {
                let visible = is_row_visible(&state.rows, i, visibility);
                let (f_pos, hidden) = if visible {
                    let ii = visible_idx;
                    visible_idx += 1;
                    f_pos_for_visible_idx(ii, w, mid_pos, bottom_pos)
                } else {
                    let anchor =
                        state
                            .rows
                            .get(i)
                            .and_then(|row| match conditional_row_parent(row.id) {
                                Some(RowId::MeasureCounter) => measure_counter_anchor_visible_idx,
                                Some(RowId::JudgmentTilt) => judgment_tilt_anchor_visible_idx,
                                Some(RowId::ErrorBar) => error_bar_anchor_visible_idx,
                                Some(RowId::Hide) => hide_anchor_visible_idx,
                                _ => None,
                            });
                    if let Some(anchor_idx) = anchor {
                        let (anchor_f_pos, _) =
                            f_pos_for_visible_idx(anchor_idx, w, mid_pos, bottom_pos);
                        (anchor_f_pos, true)
                    } else {
                        (-0.5, true)
                    }
                };

                let dest_y = first_row_center_y + row_step * f_pos;
                let dest_a = if hidden { 0.0 } else { 1.0 };

                let tw = &mut state.row_tweens[i];
                let cur_y = tw.y();
                let cur_a = tw.a();
                if (dest_y - tw.to_y).abs() > 0.01 || dest_a != tw.to_a {
                    tw.from_y = cur_y;
                    tw.from_a = cur_a;
                    tw.to_y = dest_y;
                    tw.to_a = dest_a;
                    tw.t = 0.0;
                }
                if tw.t < 1.0 {
                    if ROW_TWEEN_SECONDS > 0.0 {
                        tw.t = (tw.t + dt / ROW_TWEEN_SECONDS).min(1.0);
                    } else {
                        tw.t = 1.0;
                    }
                }
            }
        }
    }

    // Reset help reveal and play SFX when a player changes rows.
    for player_idx in active_player_indices(active) {
        if state.selected_row[player_idx] == state.prev_selected_row[player_idx] {
            continue;
        }
        match state.nav_key_held_direction[player_idx] {
            Some(NavDirection::Up) => audio::play_sfx("assets/sounds/prev_row.ogg"),
            Some(NavDirection::Down) => audio::play_sfx("assets/sounds/next_row.ogg"),
            _ => audio::play_sfx("assets/sounds/next_row.ogg"),
        }

        state.help_anim_time[player_idx] = 0.0;
        state.prev_selected_row[player_idx] = state.selected_row[player_idx];
    }

    // Retarget cursor tween destinations to match current selection and row destinations.
    for player_idx in active_player_indices(active) {
        let Some((to_x, to_y, to_w, to_h)) =
            cursor_dest_for_player(state, asset_manager, player_idx)
        else {
            continue;
        };

        let needs_cursor_init = !state.cursor_initialized[player_idx];
        if needs_cursor_init {
            state.cursor_initialized[player_idx] = true;
            state.cursor_from_x[player_idx] = to_x;
            state.cursor_from_y[player_idx] = to_y;
            state.cursor_from_w[player_idx] = to_w;
            state.cursor_from_h[player_idx] = to_h;
            state.cursor_to_x[player_idx] = to_x;
            state.cursor_to_y[player_idx] = to_y;
            state.cursor_to_w[player_idx] = to_w;
            state.cursor_to_h[player_idx] = to_h;
            state.cursor_t[player_idx] = 1.0;
        } else {
            let dx = (to_x - state.cursor_to_x[player_idx]).abs();
            let dy = (to_y - state.cursor_to_y[player_idx]).abs();
            let dw = (to_w - state.cursor_to_w[player_idx]).abs();
            let dh = (to_h - state.cursor_to_h[player_idx]).abs();
            if dx > 0.01 || dy > 0.01 || dw > 0.01 || dh > 0.01 {
                let t = state.cursor_t[player_idx].clamp(0.0, 1.0);
                let cur_x = (state.cursor_to_x[player_idx] - state.cursor_from_x[player_idx])
                    .mul_add(t, state.cursor_from_x[player_idx]);
                let cur_y = (state.cursor_to_y[player_idx] - state.cursor_from_y[player_idx])
                    .mul_add(t, state.cursor_from_y[player_idx]);
                let cur_w = (state.cursor_to_w[player_idx] - state.cursor_from_w[player_idx])
                    .mul_add(t, state.cursor_from_w[player_idx]);
                let cur_h = (state.cursor_to_h[player_idx] - state.cursor_from_h[player_idx])
                    .mul_add(t, state.cursor_from_h[player_idx]);

                state.cursor_from_x[player_idx] = cur_x;
                state.cursor_from_y[player_idx] = cur_y;
                state.cursor_from_w[player_idx] = cur_w;
                state.cursor_from_h[player_idx] = cur_h;
                state.cursor_to_x[player_idx] = to_x;
                state.cursor_to_y[player_idx] = to_y;
                state.cursor_to_w[player_idx] = to_w;
                state.cursor_to_h[player_idx] = to_h;
                state.cursor_t[player_idx] = 0.0;
            }
        }
    }

    // Advance cursor tween.
    for player_idx in [P1, P2] {
        if state.cursor_t[player_idx] < 1.0 {
            if CURSOR_TWEEN_SECONDS > 0.0 {
                state.cursor_t[player_idx] =
                    (state.cursor_t[player_idx] + dt / CURSOR_TWEEN_SECONDS).min(1.0);
            } else {
                state.cursor_t[player_idx] = 1.0;
            }
        }
    }

    pending_action
}

// Helpers for hold-to-scroll controlled by the app dispatcher



