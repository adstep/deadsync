# Options Module — Refactor Plan

## Completed Work
- Module split: 10,947-line monolith → 30 focused submodules (mod.rs is 67 lines)
- Draft PR #283 open on pnn64/deadsync (branch: `adstep/main/options-module-split`)
- Build passes, 901/906 tests pass (5 pre-existing song_lua failures)

## Architecture Refactor — Progress Tracker

| Step | ID | Title | Status | Description |
|------|----|-------|--------|-------------|
| 1 | R2 | SubmenuState map | ✅ done | Replace 34 per-submenu Vec fields with `[SubmenuState; 17]` indexed by SubmenuKind. Eliminate 17-arm dispatch matches. |
| 2 | R3 | Eliminate hardcoded row indices | ✅ done | Remove 72+ ROW_INDEX constants. Compute visibility from SubRowId instead of positional index. |
| 3 | R4 | Typed choice enums | ✅ done | Replace 60+ converter function pairs with ChoiceEnum trait for type-safe choice-to-enum mapping. |
| 4 | R1 | Introduce RowBehavior | pending | Port RowBehavior enum with typed bindings. Replaces 1329-line input.rs dispatch with ~30-line dispatcher. |
| 5 | R7 | Outcome pattern | pending | Return Outcome struct from dispatch. Centralize side effects (config, audio, cache) in caller. |
| 6 | R6 | Decompose rendering | pending | Break get_actors (1230 lines) into focused helpers. get_actors becomes thin orchestrator. |
| 7 | R8 | Centralize submenu registration | pending | Create SubmenuDef struct. Adding a submenu becomes one definition instead of 8+ edits. |
| 8 | R5 | Unify ItemId and SubRowId | pending | Merge two parallel 126+ variant enums into single RowId. Cross-cutting, highest risk. |

## Completed Step Notes

### R2: SubmenuState map
- `SubmenuStates` newtype wraps `[SubmenuState; 17]` with `Index<SubmenuKind>` / `IndexMut<SubmenuKind>`
- Access: `state.sub[SubmenuKind::Graphics].choice_indices` (no `.index()` needed)
- `sync_submenu_cursor_indices` uses `clone_from` loop
- Init uses `SubmenuStates::new(|i| ...)` with `from_fn`

### R3: Eliminate row indices
- Added `row_position`, `get_choice_by_id`, `get_choice_by_id_mut` helpers to `row.rs`
- Visibility filters match on `row.id` instead of positional index
- `select_music_parent_row` maps child→parent via `SubRowId` instead of index constants

### R4: Typed choice enums
- Added `ChoiceEnum` trait to `row.rs` with `ALL`, `DEFAULT`, `choice_index()`, `from_choice()`
- 17 enum types implement it (e.g., `LogLevel`, `PresentModePolicy`, `MachineFont`, etc.)
- Call sites: `value.choice_index()` / `Type::from_choice(idx)`
- Skipped: lossy mappings (LanguageFlag), dynamic (software_thread), arithmetic (volume), bitmask

## Next Step: R1 — Introduce RowBehavior
This is the highest-impact change. Port `player_options`' `RowBehavior` enum pattern so each
row carries typed behavior (Cycle/Numeric/Custom/Exit) instead of relying on the giant
`apply_submenu_choice_delta` match in `input.rs` (1,329 lines, 124 SubRowId dispatch sites).

## Branch Info
- Code branch: `adstep/main/options-submenu-state` (on `origin` = adstep/deadsync)
- Based on: `adstep/main/options-module-split` (module split PR #283 on pnn64/deadsync)
- Upstream: `pnn64/deadsync` (remote `upstream`)

## Detailed Recommendations
See `docs/refactor/options-refactor-recommendations.md` for full architectural analysis.
