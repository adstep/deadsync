use crate::assets::i18n::LookupKey;
use crate::engine::present::actors::Actor;
use crate::game::parsing::noteskin::Noteskin;
use crate::game::song::SongData;
use crate::screens::Screen;
use crate::screens::components::shared::heart_bg;
use crate::screens::input as screen_input;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use super::*;
use super::row_kind::RowKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavDirection {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionsPane {
    Main,
    Advanced,
    Uncommon,
}

impl OptionsPane {
    pub const ALL: [OptionsPane; 3] =
        [OptionsPane::Main, OptionsPane::Advanced, OptionsPane::Uncommon];

    #[inline(always)]
    pub fn to_index(self) -> usize {
        match self {
            OptionsPane::Main => 0,
            OptionsPane::Advanced => 1,
            OptionsPane::Uncommon => 2,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum PaneTransition {
    None,
    FadingOut { target: OptionsPane, t: f32 },
    FadingIn { t: f32 },
}

impl PaneTransition {
    #[inline(always)]
    pub fn alpha(self) -> f32 {
        match self {
            Self::None => 1.0,
            Self::FadingOut { t, .. } => (1.0 - t).clamp(0.0, 1.0),
            Self::FadingIn { t } => t.clamp(0.0, 1.0),
        }
    }

    #[inline(always)]
    pub fn is_active(self) -> bool {
        !matches!(self, Self::None)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowId {
    TypeOfSpeedMod,
    SpeedMod,
    Mini,
    Perspective,
    NoteSkin,
    MineSkin,
    ReceptorSkin,
    TapExplosionSkin,
    JudgmentFont,
    JudgmentOffsetX,
    JudgmentOffsetY,
    ComboFont,
    ComboOffsetX,
    ComboOffsetY,
    HoldJudgment,
    BackgroundFilter,
    NoteFieldOffsetX,
    NoteFieldOffsetY,
    VisualDelay,
    GlobalOffsetShift,
    MusicRate,
    Stepchart,
    WhatComesNext,
    Exit,
    // Advanced pane
    Turn,
    Scroll,
    Hide,
    LifeMeterType,
    LifeBarOptions,
    DataVisualizations,
    DensityGraphBackground,
    TargetScore,
    ActionOnMissedTarget,
    MiniIndicator,
    IndicatorScoreType,
    GameplayExtras,
    ComboColors,
    ComboColorMode,
    CarryCombo,
    JudgmentTilt,
    JudgmentTiltIntensity,
    JudgmentBehindArrows,
    OffsetIndicator,
    ErrorBar,
    ErrorBarTrim,
    ErrorBarOptions,
    ErrorBarOffsetX,
    ErrorBarOffsetY,
    MeasureCounter,
    MeasureCounterLookahead,
    MeasureCounterOptions,
    MeasureLines,
    RescoreEarlyHits,
    EarlyDecentWayOffOptions,
    ResultsExtras,
    TimingWindows,
    FAPlusOptions,
    CustomBlueFantasticWindow,
    CustomBlueFantasticWindowMs,
    // Uncommon pane
    Insert,
    Remove,
    Holds,
    Accel,
    Effect,
    Appearance,
    Attacks,
    HideLightType,
    GameplayExtrasMore,
}

pub struct Row {
    pub id: RowId,
    pub name: LookupKey,
    pub choices: Vec<String>,
    pub selected_choice_index: [usize; PLAYER_SLOTS],
    pub help: Vec<String>,
    pub choice_difficulty_indices: Option<Vec<usize>>,
    /// Per-row behaviour. Every builder must specify the appropriate
    /// `RowKind` variant; the dispatcher in `choice.rs` keys off it to
    /// route L/R and Start.
    pub kind: RowKind,
}

#[derive(Clone, Debug)]
pub struct FixedStepchart {
    pub label: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpeedModType {
    X,
    C,
    M,
}

impl SpeedModType {
    /// Letter form used by `ScrollSpeedSetting`'s `Display` impl.
    #[inline]
    pub fn as_letter(self) -> &'static str {
        match self {
            Self::X => "X",
            Self::C => "C",
            Self::M => "M",
        }
    }

    /// Index into the `TypeOfSpeedMod` row's choices array.
    /// Order must stay in sync with the row definition in `panes/main.rs`
    /// (currently `[X, C, M]`).
    #[inline]
    pub fn type_choice_index(self) -> usize {
        match self {
            Self::X => 0,
            Self::C => 1,
            Self::M => 2,
        }
    }

    /// Inverse of [`Self::type_choice_index`]. Unknown indices fall back to
    /// `C`, matching the previous default behavior.
    #[inline]
    pub fn from_type_choice_index(idx: usize) -> Self {
        match idx {
            0 => Self::X,
            2 => Self::M,
            _ => Self::C,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SpeedMod {
    pub mod_type: SpeedModType,
    pub value: f32,
}

impl SpeedMod {
    /// Canonical display form of a speed mod value, e.g. `1.50x`, `C600`, `M450`.
    /// Used by the player-options rows and helper renderers.
    pub fn display(&self) -> String {
        match self.mod_type {
            SpeedModType::X => format!("{:.2}x", self.value),
            SpeedModType::C => format!("C{}", self.value as i32),
            SpeedModType::M => format!("M{}", self.value as i32),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RowTween {
    pub from_y: f32,
    pub to_y: f32,
    pub from_a: f32,
    pub to_a: f32,
    pub t: f32,
}

impl RowTween {
    #[inline(always)]
    pub fn y(&self) -> f32 {
        (self.to_y - self.from_y).mul_add(self.t, self.from_y)
    }

    #[inline(always)]
    pub fn a(&self) -> f32 {
        (self.to_a - self.from_a).mul_add(self.t, self.from_a)
    }
}

pub struct State {
    pub song: Arc<SongData>,
    pub return_screen: Screen,
    pub fixed_stepchart: Option<FixedStepchart>,
    pub chart_steps_index: [usize; PLAYER_SLOTS],
    pub chart_difficulty_index: [usize; PLAYER_SLOTS],
    /// Per-pane mutable UI state (rows, selection, inline focus, row tweens).
    /// Indexed by `OptionsPane::to_index`.
    pub panes: [PaneState; 3],
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
    pub(super) bg: heart_bg::State,
    pub nav_key_held_direction: [Option<NavDirection>; PLAYER_SLOTS],
    pub nav_key_held_since: [Option<Instant>; PLAYER_SLOTS],
    pub nav_key_last_scrolled_at: [Option<Instant>; PLAYER_SLOTS],
    pub start_held_since: [Option<Instant>; PLAYER_SLOTS],
    pub start_last_triggered_at: [Option<Instant>; PLAYER_SLOTS],

    pub(super) allow_per_player_global_offsets: bool,
    pub player_profiles: [crate::game::profile::Profile; PLAYER_SLOTS],
    pub(super) noteskin_cache: HashMap<String, Arc<Noteskin>>,
    pub(super) noteskin: [Option<Arc<Noteskin>>; PLAYER_SLOTS],
    pub(super) mine_noteskin: [Option<Arc<Noteskin>>; PLAYER_SLOTS],
    pub(super) receptor_noteskin: [Option<Arc<Noteskin>>; PLAYER_SLOTS],
    pub(super) tap_explosion_noteskin: [Option<Arc<Noteskin>>; PLAYER_SLOTS],
    pub(super) preview_time: f32,
    pub(super) preview_beat: f32,
    pub(super) help_anim_time: [f32; PLAYER_SLOTS],
    // Combo preview state (for Combo Font row)
    pub(super) combo_preview_count: u32,
    pub(super) combo_preview_elapsed: f32,
    pub(super) pane_transition: PaneTransition,
    pub(super) menu_lr_chord: screen_input::MenuLrChordTracker,
}

impl State {
    /// Returns the per-pane state for the currently active pane.
    #[inline]
    pub fn current_pane_data(&self) -> &PaneState {
        &self.panes[self.current_pane.to_index()]
    }

    /// Returns the per-pane state for the currently active pane (mutable).
    #[inline]
    pub fn current_pane_data_mut(&mut self) -> &mut PaneState {
        &mut self.panes[self.current_pane.to_index()]
    }

    /// Returns the per-pane state for the given pane.
    #[inline]
    #[allow(dead_code)]
    pub fn pane_data(&self, pane: OptionsPane) -> &PaneState {
        &self.panes[pane.to_index()]
    }

    /// Returns the per-pane state for the given pane (mutable).
    #[inline]
    #[allow(dead_code)]
    pub fn pane_data_mut(&mut self, pane: OptionsPane) -> &mut PaneState {
        &mut self.panes[pane.to_index()]
    }

    // --- Forwarding accessors for the current pane ---

    #[inline]
    pub fn rows(&self) -> &Vec<Row> {
        &self.current_pane_data().rows
    }
    #[inline]
    pub fn rows_mut(&mut self) -> &mut Vec<Row> {
        &mut self.current_pane_data_mut().rows
    }
    #[inline]
    pub fn selected_row(&self) -> &[usize; PLAYER_SLOTS] {
        &self.current_pane_data().selected_row
    }
    #[inline]
    pub fn selected_row_mut(&mut self) -> &mut [usize; PLAYER_SLOTS] {
        &mut self.current_pane_data_mut().selected_row
    }
    #[inline]
    pub fn prev_selected_row(&self) -> &[usize; PLAYER_SLOTS] {
        &self.current_pane_data().prev_selected_row
    }
    #[inline]
    pub fn prev_selected_row_mut(&mut self) -> &mut [usize; PLAYER_SLOTS] {
        &mut self.current_pane_data_mut().prev_selected_row
    }
    #[inline]
    pub fn inline_choice_x(&self) -> &[f32; PLAYER_SLOTS] {
        &self.current_pane_data().inline_choice_x
    }
    #[inline]
    pub fn inline_choice_x_mut(&mut self) -> &mut [f32; PLAYER_SLOTS] {
        &mut self.current_pane_data_mut().inline_choice_x
    }
    #[inline]
    pub fn arcade_row_focus(&self) -> &[bool; PLAYER_SLOTS] {
        &self.current_pane_data().arcade_row_focus
    }
    #[inline]
    pub fn arcade_row_focus_mut(&mut self) -> &mut [bool; PLAYER_SLOTS] {
        &mut self.current_pane_data_mut().arcade_row_focus
    }
    #[inline]
    pub fn row_tweens(&self) -> &Vec<RowTween> {
        &self.current_pane_data().row_tweens
    }
    #[inline]
    pub fn row_tweens_mut(&mut self) -> &mut Vec<RowTween> {
        &mut self.current_pane_data_mut().row_tweens
    }
    #[inline]
    pub fn cursor(&self) -> &[CursorTween; PLAYER_SLOTS] {
        &self.current_pane_data().cursor
    }
    #[inline]
    pub fn cursor_mut(&mut self) -> &mut [CursorTween; PLAYER_SLOTS] {
        &mut self.current_pane_data_mut().cursor
    }
}

#[allow(clippy::too_many_arguments)]
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

    let speed_mod_p1 = SpeedMod::from(p1_profile.scroll_speed);
    let speed_mod_p2 = SpeedMod::from(p2_profile.scroll_speed);
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
    let player_profiles = [p1_profile.clone(), p2_profile.clone()];

    // Build all three panes up front so each keeps its own row vector,
    // selection cursor, and tween state across pane swaps.
    let mut pane_rows: [Vec<Row>; 3] = std::array::from_fn(|i| {
        build_rows(
            &song,
            &speed_mod_p1,
            chart_steps_index,
            preferred_difficulty_index,
            session_music_rate,
            OptionsPane::ALL[i],
            &noteskin_names,
            return_screen,
            fixed_stepchart.as_ref(),
        )
    });
    // Apply profile defaults to every pane so each row's selected_choice_index
    // is initialized from the profile. The mask values are profile-derived
    // (independent of which rows are present), so we keep the values from
    // whichever pane contains each mask-owning row.
    let mut masks_p1: Option<(u8, u8, u8, u8, u8, u8, u16, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8)> = None;
    let mut masks_p2: Option<(u8, u8, u8, u8, u8, u8, u16, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8)> = None;
    for rows in pane_rows.iter_mut() {
        let m1 = apply_profile_defaults(rows, &player_profiles[P1], P1);
        let m2 = apply_profile_defaults(rows, &player_profiles[P2], P2);
        if masks_p1.is_none() {
            masks_p1 = Some(m1);
            masks_p2 = Some(m2);
        }
    }
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
    ) = masks_p1.expect("at least one pane built");
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
    ) = masks_p2.expect("at least one pane built");

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
    let panes: [PaneState; 3] = std::array::from_fn(|i| {
        let rows = std::mem::take(&mut pane_rows[i]);
        let row_tweens = init_row_tweens(
            &rows,
            [0; PLAYER_SLOTS],
            active,
            [hide_active_mask_p1, hide_active_mask_p2],
            [error_bar_active_mask_p1, error_bar_active_mask_p2],
            allow_per_player_global_offsets,
        );
        PaneState {
            rows,
            selected_row: [0; PLAYER_SLOTS],
            prev_selected_row: [0; PLAYER_SLOTS],
            inline_choice_x: [f32::NAN; PLAYER_SLOTS],
            arcade_row_focus: [true; PLAYER_SLOTS],
            row_tweens,
            cursor: [CursorTween::new(); PLAYER_SLOTS],
        }
    });
    State {
        song,
        return_screen,
        fixed_stepchart,
        chart_steps_index,
        chart_difficulty_index,
        panes,
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
        allow_per_player_global_offsets,
        player_profiles,
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
        pane_transition: PaneTransition::None,
        menu_lr_chord: screen_input::MenuLrChordTracker::default(),
    }
}

pub fn in_transition() -> (Vec<Actor>, f32) {
    let actor = crate::act!(quad:
        align(0.0, 0.0): xy(0.0, 0.0):
        zoomto(crate::engine::space::screen_width(), crate::engine::space::screen_height()):
        diffuse(0.0, 0.0, 0.0, 1.0):
        z(1100):
        linear(TRANSITION_IN_DURATION): alpha(0.0):
        linear(0.0): visible(false)
    );
    (vec![actor], TRANSITION_IN_DURATION)
}

pub fn out_transition() -> (Vec<Actor>, f32) {
    let actor = crate::act!(quad:
        align(0.0, 0.0): xy(0.0, 0.0):
        zoomto(crate::engine::space::screen_width(), crate::engine::space::screen_height()):
        diffuse(0.0, 0.0, 0.0, 0.0):
        z(1200):
        linear(TRANSITION_OUT_DURATION): alpha(1.0)
    );
    (vec![actor], TRANSITION_OUT_DURATION)
}
