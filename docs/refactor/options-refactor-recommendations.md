# Options Module — Architecture Refactor Recommendations

## Problem Statement

The `options` module was recently split from a 10,947-line monolith into
submodules (Phase 1 of the refactor plan). The code structure is now
manageable, but the **architecture** still suffers from patterns that make
it hard to extend, test, and maintain:

- 126-variant `SubRowId` enum with no associated behavior
- Choice logic scattered across a 1,329-line `input.rs` with 124 `SubRowId::` dispatch sites
- 34 separate `Vec<usize>` fields for per-submenu choice/cursor state
- 72+ hardcoded `ROW_INDEX` constants for visibility logic
- Parallel `ItemId` / `SubRowId` taxonomies with no structural link
- Rendering is one big pipeline with no per-row decomposition

`player_options` has already solved these problems. The recommendations
below port its proven patterns to `options`.

---

## Recommendation 1: Introduce `RowBehavior` for Submenu Rows

### Current state
Every row is a static `SubRow { id, label, choices, inline }`. All
behavior lives in `apply_submenu_choice_delta` as a giant match/if-chain
on `SubRowId` × `SubmenuKind` — 124 `SubRowId::` references, deeply nested.

### Target state (from `player_options`)
Each row carries a `RowBehavior` enum:

```rust
enum RowBehavior {
    Cycle(CycleBinding),       // enum-backed: choice index ↔ config value
    Numeric(NumericBinding),   // clamped i32 value (volume, offset, etc.)
    Bitmask(BitmaskBinding),   // multi-select bitfield
    Custom(CustomBinding),     // escape hatch for complex rows
    Exit,                      // navigation-only
}
```

Each binding owns its own `apply` + `persist` logic. The dispatcher
becomes ~30 lines:

```rust
fn dispatch_delta(row: &mut Row, delta: isize) -> Outcome {
    match &row.behavior {
        Cycle(b)   => cycle_and_apply(b, delta),
        Numeric(b) => clamp_and_apply(b, delta),
        Custom(b)  => (b.apply)(state, delta),
        _          => Outcome::unchanged(),
    }
}
```

### Impact
- `input.rs` shrinks from 1,329 lines to ~300
- Adding a new row = adding a binding, not editing a dispatcher
- Each submenu file owns its row definitions AND behavior

---

## Recommendation 2: Replace Per-Submenu Vec Fields with a SubmenuState Map

### Current state
`State` has 34 fields like:
```rust
sub_choice_indices_system: Vec<usize>,
sub_choice_indices_graphics: Vec<usize>,
sub_cursor_indices_system: Vec<usize>,
sub_cursor_indices_graphics: Vec<usize>,
// ... × 17 submenus
```

Accessed via `submenu_choice_indices(state, kind)` which is a 17-arm match.

### Target state
```rust
struct SubmenuState {
    choice_indices: Vec<usize>,
    cursor_indices: Vec<usize>,
    selected: usize,
    // row_tweens, scroll_offset, etc. could live here too
}

// In State:
submenu_states: EnumMap<SubmenuKind, SubmenuState>,
// or: [SubmenuState; SubmenuKind::COUNT]
```

### Impact
- Eliminates 34 fields and all the 17-arm dispatch matches
- `SubmenuKind` indexes directly into the array
- Adding a new submenu = adding a variant, not 4+ new fields + match arms

---

## Recommendation 3: Eliminate Hardcoded Row Index Constants

### Current state
Visibility logic relies on constants like:
```rust
const GRAPHICS_SOFTWARE_THREADS_ROW_INDEX: usize = 5;
const SELECT_MUSIC_SCOREBOX_CYCLE_ROW_INDEX: usize = 8;
```
72+ such constants across `visibility.rs` and `submenus/graphics.rs`.
If rows are reordered, these silently break.

### Target state
Visibility should be computed from `SubRowId`, not positional index:

```rust
fn row_visible(state: &State, kind: SubmenuKind, id: SubRowId) -> bool {
    match id {
        SubRowId::SoftwareThreads => selected_renderer_supports_threads(state),
        SubRowId::MaxFpsValue => max_fps_enabled(state),
        SubRowId::HighDpi => graphics_show_high_dpi(state),
        _ => true,
    }
}
```

Or better yet, attach visibility predicates to the row definition itself:

```rust
SubRow {
    id: SubRowId::SoftwareThreads,
    visible_when: Some(|state| selected_renderer_supports_threads(state)),
    ..
}
```

### Impact
- Row reordering becomes safe
- Visibility logic is co-located with row definitions
- No more fragile index arithmetic

---

## Recommendation 4: Use Typed Enums for Choice Values

### Current state
Many rows use `usize` choice indices mapped to values via converter
function pairs:
```rust
const fn log_level_choice_index(level: LogLevel) -> usize { ... }
const fn log_level_from_choice(idx: usize) -> LogLevel { ... }
```

There are **60+ such converter pairs** across the submenu files. The
`usize` intermediate loses type safety.

### Target state (from `player_options`)
Use the `index_binding!` macro pattern:

```rust
index_binding!(LOG_LEVEL_VARIANTS, LogLevel::Info, log_level, persist_fn, false)
```

Where `LOG_LEVEL_VARIANTS: &[LogLevel]` is the canonical choice-index
↔ enum mapping, and the macro generates the binding with no manual
converter functions.

### Impact
- Eliminates 60+ boilerplate converter function pairs
- Choice index ↔ value mapping is guaranteed correct by construction
- Config writes go through a single typed path

---

## Recommendation 5: Unify ItemId and SubRowId

### Current state
Two parallel enums with overlapping semantics:
- `ItemId` (top-level menu + submenu help entries): used for help text lookup
- `SubRowId` (submenu rows): used for behavior dispatch

Many submenu items exist in both enums with slightly different names
(e.g., `ItemId::AdvDefaultFailType` ↔ `SubRowId::DefaultFailType`).

### Target state
Either:
- **Option A:** Merge into a single `RowId` enum (like `player_options`)
  that covers both top-level items and submenu rows
- **Option B:** Keep them separate but derive `ItemId` from `SubRowId`
  automatically, using a trait or associated type

### Impact
- Eliminates the dual-taxonomy confusion
- Help text lookup can use the same ID as behavior dispatch
- Fewer enums to maintain when adding new rows

---

## Recommendation 6: Decompose Rendering into Per-Section Helpers

### Current state
`render.rs` is 1,230 lines. `get_actors` assembles the full screen
as one large function. Submenu row rendering, overlays, transitions,
cursor, and description blocks are interleaved.

### Target state
Break rendering into focused helpers:
```rust
fn render_submenu_rows(state, actors, ...)       // row list + choices
fn render_cursor(state, actors, ...)             // cursor ring
fn render_description(state, actors, ...)        // help text panel
fn render_confirm_overlay(state, actors, ...)    // yes/no dialogs
fn render_reload_overlay(state, actors, ...)     // reload progress
fn render_score_import_overlay(state, actors, ...) // import progress
```

`get_actors` becomes a thin orchestrator:
```rust
pub fn get_actors(state, assets) -> Vec<Actor> {
    let mut actors = render_background(state);
    render_submenu_rows(state, &mut actors, ...);
    render_cursor(state, &mut actors, ...);
    render_description(state, &mut actors, ...);
    if state.confirm.is_some() {
        render_confirm_overlay(state, &mut actors, ...);
    }
    actors
}
```

### Impact
- Each render helper is independently testable
- Easier to modify one section without reading 1,200 lines
- Matches `player_options` pattern where rendering has per-row helpers

---

## Recommendation 7: Move Side-Effect Logic Out of Input Dispatch

### Current state
`apply_submenu_choice_delta` directly calls:
- `config::update_*` (persists to disk)
- `audio::play_sfx` (plays sounds)
- `clear_render_cache` (invalidates caches)
- Display mode rebuilders

This mixes input → state → side-effect in one function.

### Target state
Input returns an `Outcome` (like `player_options`):
```rust
struct Outcome {
    changed: bool,
    visibility_changed: bool,
    needs_persist: bool,
}
```

Side effects are handled by the caller based on the outcome:
```rust
let outcome = dispatch_delta(state, row, delta);
if outcome.changed {
    audio::play_sfx("change_value.ogg");
    clear_render_cache(state);
}
if outcome.visibility_changed {
    sync_visibility(state, kind);
}
```

### Impact
- Input logic becomes pure state transformation
- Side effects are centralized and predictable
- Easier to test row behavior without mocking audio/config

---

## Recommendation 8: Centralize Submenu Registration

### Current state
Adding a new submenu requires edits in **8+ locations**:
1. Add `SubmenuKind` variant
2. Add `ROWS` + `ITEMS` constants in a new submenu file
3. Add arms in `submenu_rows()`, `submenu_items()`, `submenu_title()`
4. Add `sub_choice_indices_*` + `sub_cursor_indices_*` fields to `State`
5. Add arms in `submenu_choice_indices()` + `submenu_cursor_indices()`
6. Add initialization in `init()`
7. Add visibility logic in `submenu_visible_row_indices()`
8. Add layout logic if needed

### Target state
A submenu is fully defined by a single registration:
```rust
struct SubmenuDef {
    kind: SubmenuKind,
    title: LookupKey,
    rows: &'static [SubRow],   // or fn() -> Vec<Row>
    items: &'static [Item],
    visible: fn(&State, SubRowId) -> bool,
}

const SUBMENUS: &[SubmenuDef] = &[
    SubmenuDef { kind: SubmenuKind::System, ... },
    SubmenuDef { kind: SubmenuKind::Graphics, ... },
    // ...
];
```

All dispatch functions (`submenu_rows`, `submenu_items`, etc.) iterate
or index into `SUBMENUS` instead of manual match arms.

### Impact
- Adding a submenu = one new `SubmenuDef` + one `SubmenuKind` variant
- No scattered match arms to forget

---

## Suggested Phasing

| Phase | Recommendations | Risk | Effort |
|-------|----------------|------|--------|
| A | R2 (SubmenuState map) | Low | Small — mechanical field consolidation |
| B | R3 (eliminate row indices) | Low | Medium — rewrite visibility to use SubRowId |
| C | R4 (typed enums / index_binding!) | Low | Medium — replace converter pairs with macro |
| D | R1 (RowBehavior) | Medium | Large — restructure row definitions + input |
| E | R7 (Outcome pattern) | Medium | Medium — extract side effects from dispatch |
| F | R6 (render decomposition) | Low | Medium — break up get_actors |
| G | R8 (submenu registration) | Low | Medium — centralize definitions |
| H | R5 (unify ItemId/SubRowId) | High | Large — cross-cutting enum merge |

Phases A–C are safe, incremental, and unblock D. Phase D is the
highest-impact change. Phases F–H are independent and can be done in
any order after D.
