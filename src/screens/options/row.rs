use super::*;

pub struct SubRow {
    pub id: RowId,
    pub label: LookupKey,
    pub choices: &'static [Choice],
    pub inline: bool, // whether to lay out choices inline (vs single centered value)
    /// What this row does on L/R (and, for Exit, on Start). During the
    /// migration, untouched rows use `RowBehavior::Legacy` and fall through
    /// to the per-`SubmenuKind` match in `apply_submenu_choice_delta`. Each
    /// row is migrated to a typed binding incrementally; once all rows are
    /// migrated the `Legacy` variant is removed.
    pub behavior: RowBehavior,
}

// ============================== RowBehavior ============================

/// Result of a row's reaction to an L/R press. Mirrors the shape of the
/// `player_options` `Outcome` so the two dispatchers can later be unified.
/// For now this only carries the bare minimum to drive the existing SFX +
/// render-cache invalidation path.
#[derive(Clone, Debug, Default)]
pub struct Outcome {
    /// Row reacted to the press (value changed). Drives change-value SFX
    /// and render-cache invalidation in the dispatcher.
    pub changed: bool,
    /// Optional screen action to forward up the stack (e.g. `ShowStats`
    /// returns `UpdateShowOverlay`).
    pub action: Option<crate::screens::ScreenAction>,
}

impl Outcome {
    pub const NONE: Self = Self {
        changed: false,
        action: None,
    };

    #[inline(always)]
    pub const fn changed() -> Self {
        Self {
            changed: true,
            action: None,
        }
    }

    #[inline(always)]
    pub const fn changed_with_action(action: crate::screens::ScreenAction) -> Self {
        Self {
            changed: true,
            action: Some(action),
        }
    }
}

/// Numeric/slider row binding. Used by ms- and tenths-of-ms sliders that
/// adjust a `State` field by `delta` within `[min, max]` and persist via
/// `config::update_*`. The dispatcher handles the `adjust_*_value` call,
/// SFX and render-cache invalidation; the binding only provides the
/// per-row plumbing.
#[derive(Clone, Copy, Debug)]
pub struct NumericBinding {
    /// Mutable accessor for the backing `State` field (e.g. `master_volume_pct`).
    pub get_mut: fn(&mut State) -> &mut i32,
    pub min: i32,
    pub max: i32,
    /// Adjust step semantics: ms (1) or tenths-of-ms (1 = 0.1 ms).
    pub step: NumericStep,
    /// Persist the new value to the global config + any cascading writes.
    pub persist: fn(i32),
}

#[derive(Clone, Copy, Debug)]
pub enum NumericStep {
    Ms,
    Tenths,
}

/// Cycle row binding — covers the vast majority of options rows whose only
/// effect on change is `config::update_X(value_from_choice(new_idx))`.
#[derive(Clone, Copy, Debug)]
pub enum CycleBinding {
    /// Yes/No row: `config::update_X(new_idx == 1)`.
    Bool(fn(bool)),
    /// Indexed enum row: `config::update_X(Enum::from_choice(new_idx))`.
    /// Type erasure happens at the call site; the fn closes over the
    /// concrete `from_choice` invocation.
    Index(fn(usize)),
}

/// Cycle binding for rows whose value is persisted only on submenu
/// apply/exit (e.g. `VSync`, `FullscreenType`, bitmask rows toggled via
/// Start). The cursor still advances and the render cache is cleared,
/// matching the previous fall-through behaviour.
fn cycle_noop(_idx: usize) {}
pub const DEFERRED_APPLY_CYCLE: CycleBinding = CycleBinding::Index(cycle_noop);

/// Custom row binding for cascading effects that can't be expressed as a
/// pure config write (e.g. `SoundDevice` rebuilds the sample-rate row;
/// `DisplayResolution` rebuilds refresh-rate choices). The `apply` fn
/// receives the full state and the new choice index and returns an
/// `Outcome` describing what the dispatcher should do next.
#[derive(Clone, Copy, Debug)]
pub struct CustomBinding {
    pub apply: fn(&mut State, new_idx: usize) -> Outcome,
}

/// What kind of row this is, plus any state owned by the row's behaviour.
#[derive(Clone, Copy, Debug)]
pub enum RowBehavior {
    Cycle(CycleBinding),
    Numeric(NumericBinding),
    Custom(CustomBinding),
    /// Terminal "Exit"/launcher row — no L/R effect (Start handles it).
    Exit,
}

/// Choice values — some are localizable, some are format-specific literals.
#[derive(Clone, Copy)]
pub enum Choice {
    /// Translatable text (e.g., "Windowed", "On", "Off").
    Localized(LookupKey),
    /// Format-specific literal that should never be translated (e.g., "16:9", "1920x1080").
    Literal(&'static str),
}

impl Choice {
    pub fn get(&self) -> Arc<str> {
        match self {
            Choice::Localized(lkey) => lkey.get(),
            Choice::Literal(s) => Arc::from(*s),
        }
    }

    pub fn as_str_static(&self) -> Option<&'static str> {
        match self {
            Choice::Literal(s) => Some(s),
            Choice::Localized(_) => None,
        }
    }
}

/// Shorthand for `Choice::Localized(lookup_key(section, key))` in const arrays.
#[allow(non_snake_case)]
pub(super) const fn localized_choice(section: &'static str, key: &'static str) -> Choice {
    Choice::Localized(lookup_key(section, key))
}

/// Shorthand for `Choice::Literal(s)` in const arrays.
pub(super) const fn literal_choice(s: &'static str) -> Choice {
    Choice::Literal(s)
}

pub(super) fn set_choice_by_id(choice_indices: &mut Vec<usize>, rows: &[SubRow], id: RowId, idx: usize) {
    if let Some(pos) = rows.iter().position(|r| r.id == id)
        && let Some(slot) = choice_indices.get_mut(pos)
    {
        let max_idx = rows[pos].choices.len().saturating_sub(1);
        *slot = idx.min(max_idx);
    }
}

/// Find the positional index of a row by its `RowId`.
pub(super) fn row_position(rows: &[SubRow], id: RowId) -> Option<usize> {
    rows.iter().position(|r| r.id == id)
}

/// Read the current choice index for a row identified by `RowId`.
pub(super) fn get_choice_by_id(choices: &[usize], rows: &[SubRow], id: RowId) -> Option<usize> {
    row_position(rows, id).and_then(|pos| choices.get(pos).copied())
}

/// Get a mutable reference to the choice index for a row identified by `RowId`.
pub(super) fn get_choice_by_id_mut<'a>(
    choices: &'a mut [usize],
    rows: &[SubRow],
    id: RowId,
) -> Option<&'a mut usize> {
    row_position(rows, id).and_then(move |pos| choices.get_mut(pos))
}

pub(super) const fn yes_no_choice_index(enabled: bool) -> usize {
    if enabled { 1 } else { 0 }
}

pub(super) const fn yes_no_from_choice(idx: usize) -> bool {
    idx == 1
}

/// Trait for enums that map 1:1 to choice indices in option rows.
/// Provides bidirectional conversion between enum values and choice indices.
pub(super) trait ChoiceEnum: Copy + PartialEq + 'static {
    /// All variants in choice-index order.
    const ALL: &'static [Self];
    /// Fallback for out-of-range indices.
    const DEFAULT: Self;

    /// Returns the choice index for this value.
    fn choice_index(self) -> usize {
        Self::ALL.iter().position(|v| *v == self).unwrap_or(0)
    }

    /// Returns the enum value for a choice index, or `DEFAULT` if out of range.
    fn from_choice(idx: usize) -> Self {
        Self::ALL.get(idx).copied().unwrap_or(Self::DEFAULT)
    }
}