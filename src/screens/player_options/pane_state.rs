//! Per-pane mutable UI state.
//!
//! `State` stores `panes: [PaneState; 3]` so each pane keeps its own row
//! vector, selection cursor, scroll position, and tween state across pane
//! swaps.

use super::{PLAYER_SLOTS, Row, RowTween};

/// Per-player cursor tween (StopTweening/BeginTweening parity with
/// ITGmania `ScreenOptions::TweenCursor`). Lives on the pane so each pane
/// keeps its own cursor position across pane swaps.
#[derive(Clone, Copy, Debug)]
pub struct CursorTween {
    pub initialized: bool,
    pub from_x: f32,
    pub from_y: f32,
    pub from_w: f32,
    pub from_h: f32,
    pub to_x: f32,
    pub to_y: f32,
    pub to_w: f32,
    pub to_h: f32,
    pub t: f32,
}

impl CursorTween {
    pub const fn new() -> Self {
        Self {
            initialized: false,
            from_x: 0.0,
            from_y: 0.0,
            from_w: 0.0,
            from_h: 0.0,
            to_x: 0.0,
            to_y: 0.0,
            to_w: 0.0,
            to_h: 0.0,
            t: 1.0,
        }
    }
}

impl Default for CursorTween {
    fn default() -> Self {
        Self::new()
    }
}

/// Mutable UI state scoped to a single options pane (Main / Advanced / Uncommon).
///
/// `State` stores one of these per pane so each pane keeps its own
/// selection, scroll position, and tween state across pane swaps.
#[allow(dead_code)]
pub struct PaneState {
    pub rows: Vec<Row>,
    pub selected_row: [usize; PLAYER_SLOTS],
    pub prev_selected_row: [usize; PLAYER_SLOTS],
    pub inline_choice_x: [f32; PLAYER_SLOTS],
    pub arcade_row_focus: [bool; PLAYER_SLOTS],
    pub row_tweens: Vec<RowTween>,
    pub cursor: [CursorTween; PLAYER_SLOTS],
}

#[allow(dead_code)]
impl PaneState {
    pub fn empty() -> Self {
        Self {
            rows: Vec::new(),
            selected_row: [0; PLAYER_SLOTS],
            prev_selected_row: [0; PLAYER_SLOTS],
            inline_choice_x: [f32::NAN; PLAYER_SLOTS],
            arcade_row_focus: [true; PLAYER_SLOTS],
            row_tweens: Vec::new(),
            cursor: [CursorTween::new(); PLAYER_SLOTS],
        }
    }
}
