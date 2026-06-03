//! Core judgement-display domain types shared across the app.
//!
//! These are app-level concepts — *which* judging style is active and *which*
//! judgement slot a row represents — independent of how they are colored, laid
//! out, or scored. They live here (rather than under `config::color`) so that
//! config, the engine, and the screens can all depend on them without coupling
//! the domain vocabulary to color concerns.
//!
//! Relationship to the scoring crate: [`JudgmentWindow`] deliberately mirrors —
//! but is *separate from* — `deadsync_rules::judgment::TimingWindow` (which has
//! `W0`..`W5` and no `Miss`) and `deadsync_rules::judgment::JudgeGrade` (which
//! collapses `W0`/`W1` into a single `Fantastic`). [`JudgmentWindow`] is the
//! finer-grained *display* axis: it keeps the `W0`(blue)/`W1`(white) Fantastic
//! split and includes `Miss`, giving one slot per configurable judgement color.

/// The number of canonical judgement-window display slots (`W0`..`W5` + `Miss`).
pub const JUDGMENT_WINDOW_COUNT: usize = 7;

/// A judgement-display *slot*, in canonical tightest→loosest order matching the
/// `WindowCounts` / GrooveStats / ITL wire convention (`W0`..`W5` + `Miss`).
///
/// This is the display axis that names which row/palette entry a judgement is
/// shown with. It deliberately mirrors — but is *separate from* —
/// `deadsync_rules::judgment::TimingWindow` (which has `W0`..`W5` and no
/// `Miss`), so display/color concerns never depend on the scoring/timing enum.
/// The two are related by [`JudgmentWindow::index`] order.
///
/// Default colors (single source of truth is the engine `DEFAULT_*` consts):
///
/// | Slot   | Default   | Simply Love-ish meaning                |
/// |--------|-----------|----------------------------------------|
/// | `W0`   | `#21CCE8` | Fantastic (blue) — tightest inner band |
/// | `W1`   | `#FFFFFF` | Fantastic (white) — outer sub-band     |
/// | `W2`   | `#E29C18` | Excellent                              |
/// | `W3`   | `#66C955` | Great                                  |
/// | `W4`   | `#B45CFF` | Decent                                 |
/// | `W5`   | `#C9855E` | Way Off                                |
/// | `Miss` | `#FF3030` | Miss                                   |
///
/// Note the canonical numbering: `W0` is the *blue*, tightest Fantastic band and
/// `W1` is the *white* outer sub-band — matching `WindowCounts.w0`/`w1` and
/// `ItlJudgments` (`w0 = fantastic_plus`, `w1 = fantastic`). Plain ITG does not
/// split Fantastic, so it draws a single combined Fantastic row using the `W0`
/// color and never shows `W1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JudgmentWindow {
    W0,
    W1,
    W2,
    W3,
    W4,
    W5,
    Miss,
}

impl JudgmentWindow {
    /// All slots in canonical tightest→loosest order.
    pub const ALL: [JudgmentWindow; JUDGMENT_WINDOW_COUNT] = [
        Self::W0,
        Self::W1,
        Self::W2,
        Self::W3,
        Self::W4,
        Self::W5,
        Self::Miss,
    ];

    /// Storage index of this slot (`0..JUDGMENT_WINDOW_COUNT`).
    pub const fn index(self) -> usize {
        match self {
            Self::W0 => 0,
            Self::W1 => 1,
            Self::W2 => 2,
            Self::W3 => 3,
            Self::W4 => 4,
            Self::W5 => 5,
            Self::Miss => 6,
        }
    }

    /// The INI key suffix for this slot (e.g. `"W0"`, `"Miss"`).
    pub const fn ini_suffix(self) -> &'static str {
        match self {
            Self::W0 => "W0",
            Self::W1 => "W1",
            Self::W2 => "W2",
            Self::W3 => "W3",
            Self::W4 => "W4",
            Self::W5 => "W5",
            Self::Miss => "Miss",
        }
    }
}

/// Which per-mode judgement style a given visual context should use.
///
/// The mode is selected by *what is being drawn* (which eval pane / graph /
/// gameplay window split), not by raw profile flags, so e.g. viewing the
/// Standard eval pane always renders ITG even when the profile has Hard-EX
/// scoring enabled. The set of rows each mode displays is described by
/// `engine::present::color`'s row API rather than by the stored window set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JudgmentMode {
    Itg,
    FaPlus,
    Hex,
}

impl JudgmentMode {
    /// All modes.
    pub const ALL: [JudgmentMode; 3] = [Self::Itg, Self::FaPlus, Self::Hex];

    /// Storage index of this mode (`0..ALL.len()`). A total, explicit mapping —
    /// never relies on enum discriminants.
    pub const fn index(self) -> usize {
        match self {
            Self::Itg => 0,
            Self::FaPlus => 1,
            Self::Hex => 2,
        }
    }

    /// The INI key infix identifying this mode (e.g. `"Itg"`, `"FaPlus"`).
    pub const fn ini_infix(self) -> &'static str {
        match self {
            Self::Itg => "Itg",
            Self::FaPlus => "FaPlus",
            Self::Hex => "Hex",
        }
    }
}
