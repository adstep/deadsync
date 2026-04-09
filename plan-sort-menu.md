# Plan: Align Select Music Menu with Simply Love / ITGMania

## Background

The reference implementation is **Simply Love** (the dominant ITG theme), which builds a sort menu overlay on top of ITGMania's engine. ITGMania's engine provides a basic mode menu that replaces the MusicWheel with sort items, but Simply Love replaces this with a richer centered overlay — which is the UI that virtually all ITG players actually use.

Deadsync's select music menu (currently named `sort_menu`) is already architecturally similar to Simply Love's: a centered overlay with a vertical wheel, background dim, and a mix of sort orders and utility actions. This plan focuses on closing the remaining gaps in **options** and **UI presentation**, and includes renaming the module from `sort_menu` to `select_music_menu` to better reflect its scope.

---

## 1. Sort Order Comparison

### Simply Love Sort Orders

Simply Love organizes sorts into expandable categories. The table below compares against Deadsync.

**Category: Sorts** (10 items in Simply Love)

| Simply Love Sort | Deadsync Equivalent | Status |
|---|---|---|
| Group | `SortByGroup` | ✅ Supported |
| Title | `SortByTitle` | ✅ Supported |
| Artist | `SortByArtist` | ✅ Supported |
| Genre | *(none)* | **Missing** |
| BPM | `SortByBpm` | ✅ Supported |
| Length | `SortByLength` | ✅ Supported |
| Meter | `SortByMeter` | ✅ Supported |
| Popularity | `SortByPopularity` | ✅ Supported |
| Recent | `SortByRecent` | ✅ Supported |
| Top Grades | *(none)* | **Missing** |

**Category: Profile** (per-player sorts, shown only if persistent profile loaded)

| Simply Love Sort | Deadsync Equivalent | Status |
|---|---|---|
| Popularity P1 | *(none)* | **Missing** |
| Recent P1 | *(none)* | **Missing** |
| Top Grades P1 | *(none)* | **Missing** |
| Popularity P2 | *(none)* | **Missing** |
| Recent P2 | *(none)* | **Missing** |
| Top Grades P2 | *(none)* | **Missing** |
| Preferred (favorites) | *(none)* | **Missing** |

### Action / Utility Options Comparison

**Top-level items in Simply Love:**

| Simply Love Item | Deadsync Equivalent | Status |
|---|---|---|
| Go Back | *(close menu)* | ✅ Supported (on Sorts sub-page) |
| Switch Profile | Switch Profile | ✅ Supported |
| Leaderboard | Leaderboard | ✅ Supported |
| Song Search | Song Search | ✅ Supported |
| Add Favorite | *(none)* | **Missing** |
| Preferred (favorites sort) | *(none)* | **Missing** |
| Casual Mode | *(none)* | N/A (Deadsync design decision) |

**Category: Advanced** (in Simply Love)

| Simply Love Item | Deadsync Equivalent | Status |
|---|---|---|
| Test Input | Test Input | ✅ Supported |
| Practice Mode | *(none)* | **Missing** |
| Load New Songs | Load New Songs | ✅ Supported |
| View Downloads | View Downloads | ✅ Supported |
| Set Summary | *(none)* | **Missing** |
| Online Lobbies | Online Lobbies | ✅ Supported |

**Category: Styles** (in Simply Love)

| Simply Love Item | Deadsync Equivalent | Status |
|---|---|---|
| Change to Single/Double/Solo | Switch to Single/Double | ✅ Partial (no Solo) |

**Category: Playlists** (in Simply Love)

| Simply Love Item | Deadsync Equivalent | Status |
|---|---|---|
| Machine playlists | *(none)* | **Missing** |
| Personal playlists | *(none)* | **Missing** |

**Deadsync-only items** (not in Simply Love — fine to keep):

- Sync Pack / Sync Song (null-or-die)
- Play Replay

### ITGMania Engine-Only Sort Orders (not in Simply Love)

The ITGMania engine defines additional per-difficulty meter sorts (Easy/Medium/Hard/Challenge × Single/Double) in its mode menu. Simply Love does **not** expose these — it uses a single generic Meter sort, same as Deadsync. No action needed here.

---

## 2. Missing Features — Implementation Plan

### Priority 1: Core missing sorts

1. **Genre sort** — Sort songs alphabetically by genre field. Requires parsing genre from song metadata (`.sm`/`.ssc` `#GENRE` tag). Many ITG songs leave this blank, so behavior with empty genre should group them under a fallback section.

2. **Top Grades sort** — Sort songs by best grade achieved (descending). Requires querying best grade per song from the scores system.

### Priority 2: Per-player sorts (shown only when persistent profile loaded)

3. **Popularity P1 / P2** — Sort by per-player play count instead of global. Requires per-player play count tracking.

4. **Recent P1 / P2** — Sort by per-player last play time. Requires per-player last-played timestamps.

5. **Top Grades P1 / P2** — Sort by per-player best grade.

### Priority 3: Favorites / Playlists

6. **Add Favorite** — Toggle a song as a favorite from the sort menu. Requires a favorites list stored per-profile.

7. **Preferred / Favorites sort** — Sort order that shows only favorited songs. Uses the favorites list from above.

8. **Playlists** — Support machine-wide and per-profile playlist files. Lower priority; evaluate scope.

### Priority 4: Utility actions

9. **Practice Mode** — Simply Love has a practice mode accessible from the sort menu (event mode only). Evaluate if Deadsync needs this.

10. **Set Summary** — Shows evaluation summary for stages played so far. Evaluate if Deadsync needs this.

### Decision Point: Scope

Reasonable MVP for Simply Love parity:
- **Must have:** Genre, Top Grades, Add Favorite, Preferred/Favorites sort
- **Should have:** Per-player Popularity/Recent/Top Grades
- **Nice to have:** Playlists, Practice Mode, Set Summary

---

## 3. Category Design

### Current Architecture

Deadsync uses a **two-page model**: a `Page` enum (`Main`, `Sorts`) and a `State` enum (`Hidden`, `Visible { page, selected_index }`). The `select_music_menu_items()` function in `select_music.rs` builds a flat `Vec<Item>` for each page. Selecting "SORTS..." switches pages; "Go Back" returns.

### Proposed Architecture: Collapsible Categories

Replace the two-page model with a **single flat list** where category headers expand/collapse inline. This matches Simply Love and scales better as more items are added.

#### New Types

```rust
/// Identifies a category that can be expanded/collapsed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    Sorts,
    Profile,   // per-player sorts (only if profile loaded)
    Advanced,
    Styles,
    // Future: Playlists
}

/// A visible entry in the sort menu wheel.
#[derive(Clone, Debug)]
pub enum Entry {
    /// A collapsible category header. Shows folder icon.
    CategoryHeader {
        category: Category,
        label: &'static str,
    },
    /// A regular action item nested under a category (indented).
    CategoryItem(Item),
    /// A standalone action item (not inside any category).
    StandaloneItem(Item),
}
```

#### State Changes

```rust
/// Tracks which categories are currently expanded.
/// Replaces the Page enum.
pub struct CategoryState {
    pub expanded: HashSet<Category>,
}

pub enum State {
    Hidden,
    Visible {
        selected_index: usize,
        categories: CategoryState,
    },
}
```

- No more `Page` enum — the single list dynamically includes/excludes children based on `categories.expanded`.
- `selected_index` indexes into the flattened `Vec<Entry>` built each frame.

#### Item Assembly

`sort_menu_items()` becomes `select_music_menu_entries()` and returns `Vec<Entry>`:

```
Go Back                          ← StandaloneItem (always first)
Switch Profile                   ← StandaloneItem
Song Search                      ← StandaloneItem
Leaderboard                      ← StandaloneItem (if song selected)
Add Favorite                     ← StandaloneItem (if song selected)
▸ Sorts                          ← CategoryHeader(Sorts)
  ├ Group                        ← CategoryItem (only if Sorts expanded)
  ├ Title
  ├ Artist
  ├ Genre
  ├ BPM
  ├ Length
  ├ Level
  ├ Most Popular
  ├ Recently Played
  └ Top Grades
▸ Profile                        ← CategoryHeader(Profile) (only if profile loaded)
  ├ Popularity P1
  ├ Recent P1
  ├ Top Grades P1
  └ Preferred (Favorites)
▸ Advanced                       ← CategoryHeader(Advanced)
  ├ Test Input
  ├ Load New Songs
  ├ Online Lobbies
  ├ View Downloads               (conditional)
  ├ Sync Pack / Sync Song        (conditional)
  ├ Play Replay                  (conditional)
  └ null-or-die                  (conditional)
▸ Styles                         ← CategoryHeader(Styles)
  └ Change to Single/Double      (conditional)
```

#### Interaction

- **Selecting a CategoryHeader** toggles `expanded` for that category. The list rebuilds, inserting or removing children. Cursor stays on the header.
- **Selecting a CategoryItem or StandaloneItem** triggers its `Action` as before.
- **Navigation** wraps around the flattened list (same wheel behavior).
- **Re-entry memory**: when the menu is opened, restore the previously expanded categories and selected index.

#### Rendering Changes to `build_overlay()`

Each `Entry` variant renders differently:

| Entry Type | Visual |
|---|---|
| `CategoryHeader` | Folder icon (▸ collapsed / ▾ expanded) + bold label, left-aligned. No `top_label`. |
| `CategoryItem` | Indented left-aligned text: `top_label` (small) + `bottom_label` (large). Same as current items but shifted right ~30px. |
| `StandaloneItem` | Same as `CategoryItem` but not indented. |

Focus indicator: row background color change (darker → lighter on focus), matching Simply Love's `(0.2, 0.2, 0.2)` → `(0.35, 0.35, 0.35)`.

#### Top-Level vs Category Assignment

Items are assigned to categories based on their purpose:

- **Standalone** (ungrouped, always visible): Go Back, Switch Profile, Song Search, Leaderboard, Add Favorite. These are the most common actions and should be immediately accessible.
- **Sorts**: All `SortBy*` actions. This is the largest category.
- **Profile**: Per-player sort variants and favorites. Only shown when a persistent profile is loaded.
- **Advanced**: Utility actions that are used less frequently (Test Input, Load New Songs, Online Lobbies, View Downloads, Sync, Replay).
- **Styles**: Style switching (Single/Double). Only shown when applicable.

#### Default Expansion

On first open, **Sorts** is expanded by default (most common use case). Other categories start collapsed. Expansion state persists across opens within the same session.

---

## 4. UI Comparison

### Simply Love Sort Menu UI
- **Centered overlay** with 80% opacity black background dim
- **210×204px** menu box with white 1px border, black fill
- **"Options" header** above the menu box (black text on white quad)
- **9 visible wheel slots**, focus on slot 5 (center)
- **36px row height** per item
- **Collapsible categories** — items like "Sorts", "Profile", "Advanced", "Styles" expand/collapse with a folder icon
- **Left-aligned text** — top label (Common Normal, 1.15 zoom) + bottom label (Common Bold, 0.8 zoom)
- **Colors:** focused items are white bg `(0.35, 0.35, 0.35)`, unfocused are darker `(0.2, 0.2, 0.2)`. "Go Back" is red.
- **Folder icon** (`folder-solid.png`) shown for category headers
- **"Press SELECT To Cancel"** hint text below the menu
- **Smooth transitions:** 0.1s accelerate/decelerate for focus changes, staggered fade-in for text

### Deadsync Sort Menu UI
- **Centered overlay** with background dim — ✅ matches
- **"OPTIONS" header** above the wheel — ✅ matches
- **7 visible wheel slots** (vs Simply Love's 9)
- **Two-page structure** (Main → Sorts sub-page) instead of collapsible categories
- **Center-aligned text** with focus-based scaling (vs Simply Love's left-aligned, fixed-size)
- **Two-line labels:** top_label (small) + bottom_label (large) — similar to Simply Love
- **Color-coded:** blue for "SORTS...", red for "Go Back" — partially matches
- **Hint text** below — ✅ matches

### Key Differences

| Aspect | Simply Love | Deadsync | Action Needed |
|---|---|---|---|
| Menu size | 210×204px, 9 slots | 7 slots, larger items | **Consider adding 2 more visible slots** |
| Organization | Collapsible categories with folder icons | Two-page structure (Main / Sorts) | **Adopt collapsible categories** — see Section 3 design |
| Text alignment | Left-aligned | Center-aligned | **Align to left** for consistency |
| Item sizing | Fixed size, color change on focus | Scale-based focus (zoom in/out) | Keep Deadsync's approach or align — **design decision** |
| Row background | Subtle gray quads per row | No per-row background | **Add row background quads** for visual clarity |
| Category indicators | Folder icon for expandable categories | Blue color for "SORTS..." | **Add folder icons** if adopting collapsible categories |
| Cancel hint | "Press SELECT To Cancel" | Hint text (content TBD) | **Match Simply Love's cancel text** |
| Re-entry memory | Remembers last selection | Unknown | **Verify** and implement if missing |

---

## 5. Action Items

### Pre-work: Split sort_menu.rs into submodules and rename to select_music_menu

`sort_menu.rs` is 2,267 lines with 5 self-contained subsystems. Rename to `select_music_menu` and split into a module directory with the old main menu logic isolated in `classic.rs` so the new `categories.rs` can be developed alongside it:

```
select_music_menu/
  mod.rs          — shared types (Action, Item, constants, utility functions),
                    unified State enum, re-exports, runtime flag routing
  classic.rs      — old main menu: Page enum, VisibleState, build_overlay(),
                    ITEMS_SORTS, select_music_menu_items() logic
  categories.rs   — new main menu: Category, Entry, CategoryState, VisibleState,
                    build_overlay(), select_music_menu_entries() logic
  song_search.rs  — SongSearchState, text entry, filtering, results rendering
  leaderboard.rs  — LeaderboardOverlayState, network fetch, dual-pane rendering
  downloads.rs    — DownloadsOverlayState, scroll, progress bar rendering
  replay.rs       — ReplayOverlayState, score list, replay launching
```

The 4 overlay modules are shared — they're triggered by `Action` variants regardless of which main menu is active. `mod.rs` owns the unified `State` that delegates to either `classic` or `categories`:

```rust
pub enum State {
    Hidden,
    Classic(classic::VisibleState),
    Categories(categories::VisibleState),
}
```

`select_music.rs` branches once at the open point and once at input/render, delegating entirely to the appropriate module:

```rust
match &state.select_music_menu {
    State::Hidden => {},
    State::Classic(s) => classic::handle_input(s, ...),
    State::Categories(s) => categories::handle_input(s, ...),
}
```

- [ ] **Rename `sort_menu` to `select_music_menu`** — Rename module, update all imports and field references in `select_music.rs`.
- [ ] **Split into submodules** — Extract overlays into their own files, move old main menu into `classic.rs`, create empty `categories.rs`.

### Sort Order Changes

- [ ] **Add Genre sort order** — Add `SortByGenre` action, `WheelSortMode::Genre`, and sorting logic. Parse `#GENRE` from song files.
- [ ] **Add Top Grades sort order** — Add `SortByTopGrades` action and sorting logic. Sort by best grade (descending).
- [ ] **Add per-player Popularity sorts** — `SortByPopularityP1` / `SortByPopularityP2`. Requires per-player play count data.
- [ ] **Add per-player Recent sorts** — `SortByRecentP1` / `SortByRecentP2`. Requires per-player last-played data.
- [ ] **Add per-player Top Grades sorts** — `SortByTopGradesP1` / `SortByTopGradesP2`.
- [ ] **Add Favorites toggle** — "Add Favorite" action to mark/unmark songs. Store per-profile.
- [ ] **Add Preferred/Favorites sort** — Sort mode that filters to favorited songs only.
- [ ] **Evaluate Playlists** — Machine and personal playlists. Define file format and loading.
- [ ] **Evaluate Practice Mode** — Decide if needed.
- [ ] **Evaluate Set Summary** — Decide if needed (shows mid-session evaluation summary).

### UI Changes

- [ ] **Implement collapsible categories** — Replace `Page` enum and two-page model with `Category`/`Entry` types and inline expand/collapse (see Section 3 design).
- [ ] **Left-align text** — Switch from center-aligned to left-aligned labels. Indent `CategoryItem` entries ~30px.
- [ ] **Add row background quads** — Render subtle gray backgrounds per row with focus color transitions.
- [ ] **Increase visible slots to 9** — Match Simply Love's 9 visible wheel items (currently 7).
- [ ] **Add folder icons** — Render ▸/▾ icons on `CategoryHeader` entries.
- [ ] **Verify re-entry memory** — Ensure the sort menu remembers the last selected item on re-open.
- [ ] **Match cancel hint text** — "Press SELECT To Cancel" or equivalent.

### Data/Parsing Changes

- [ ] **Ensure `#GENRE` tag is parsed** from `.sm`/`.ssc` files and stored on the song model, needed for Genre sort.
- [ ] **Ensure per-player play counts and last-played timestamps** are available in the profile/scores system, needed for per-player sorts.
- [ ] **Ensure best grade per song** is queryable from the scores system, needed for Top Grades sort.
- [ ] **Add favorites storage** — Per-profile list of favorited songs, needed for Add Favorite and Preferred sort.

---

## 6. Implementation Plan

Ordered steps. Each step should compile and not break existing behavior before moving to the next.

### Phase 1: Refactor (no user-visible changes)

**Step 1: Rename and split sort_menu.rs into submodules**

- Rename `sort_menu.rs` → `select_music_menu/mod.rs` (create directory)
- Update imports in `select_music.rs`: `sort_menu` → `select_music_menu`
- Update State field: `sort_menu: sort_menu::State` → `select_music_menu: select_music_menu::State`
- Update all references (`sort_menu_prev_selected_index` → `select_music_menu_prev_selected_index`, etc.)
- Extract `song_search.rs` (lines 403–913): move `SongSearchCandidate`, `SongSearchResultsState`, `SongSearchTextEntryState`, `SongSearchState`, `SongSearchFilter`, and all `song_search_*` / `build_song_search_*` / `parse_song_search_*` functions
- Extract `leaderboard.rs` (lines 913–1516): move `LeaderboardSideState`, `LeaderboardOverlayStateData`, `LeaderboardOverlayState`, `LeaderboardInputOutcome`, and all leaderboard functions
- Extract `downloads.rs` (lines 1516–1777): move `DownloadsOverlayStateData`, `DownloadsOverlayState`, `DownloadsInputOutcome`, and all downloads functions
- Extract `replay.rs` (lines 1777–2104): move `ReplayOverlayStateData`, `ReplayOverlayState`, `ReplayInputOutcome`, `ReplayStartPayload`, and all replay functions
- Extract `classic.rs`: move `Page` enum, `ITEMS_SORTS`, `RenderParams`, `build_overlay()`, `scroll_dir`, `set_text_clip_rect` — everything specific to the old main menu
- Keep in `mod.rs`: constants, `Action`, `Item`, all `ITEM_*` consts, unified `State` enum (`Hidden | Classic(classic::VisibleState)`), re-exports
- Update `select_music.rs`: replace `sort_menu::Page` / `sort_menu::State` references to work with the new `State::Classic(..)` variant
- Re-export moved types from `mod.rs` so external callers need minimal path changes
- Verify: `cargo check` passes, no behavior changes

**Step 2: Create `categories.rs` with new types**

- Create `select_music_menu/categories.rs`
- Add: `Category` enum, `Entry` enum (`CategoryHeader`, `CategoryItem`, `StandaloneItem`), `CategoryState` struct, `VisibleState` struct
- Add `State::Categories(categories::VisibleState)` variant to the unified `State` enum in `mod.rs`
- Stub out `pub fn build_overlay()` and `pub fn handle_input()` — can return empty/no-op for now
- Verify: compiles, `Categories` variant is unused but present

**Step 3: Add `sort_menu_entries()` builder function**

- In `categories.rs`, add `pub fn build_entries()` that builds `Vec<Entry>` with categories from the shared `ITEM_*` consts
- Map the existing items into the category structure from Section 3
- Write a `#[test]` that calls `build_entries()` with mock state and asserts expected entry count with various expansion states

### Phase 2: Category UI (behind runtime flag)

The new UI is gated by a runtime config setting `use_category_sort_menu: bool` (default `false`). The old UI lives in `classic.rs`, the new UI in `categories.rs` — no code interleaving. Both coexist until the flag is flipped and `classic.rs` is deleted in Phase 5.

**Step 4: Add runtime config flag and wire up routing**

- Add `pub use_category_select_music_menu: bool` to the `Config` struct in `src/config/mod.rs`, default `false`
- In `select_music.rs`, when opening the menu, check the flag to decide which variant to use:
  ```rust
  if config::get().use_category_select_music_menu {
      state.select_music_menu = select_music_menu::State::Categories(categories::open());
  } else {
      state.select_music_menu = select_music_menu::State::Classic(classic::open());
  }
  ```
- In input handling, match on the `State` variant and delegate:
  ```rust
  match &mut state.select_music_menu {
      State::Hidden => {},
      State::Classic(s) => classic::handle_input(s, ...),
      State::Categories(s) => categories::handle_input(s, ...),
  }
  ```
- Same pattern for rendering — delegate `build_overlay()` to the appropriate module
- Verify: with flag `false`, old behavior is identical. With flag `true`, new menu opens (stubbed/empty is fine).

**Step 5: Implement `categories::build_overlay()`**

- Render `Entry` variants:
  - `CategoryHeader`: folder icon (▸/▾ text glyph), bold label, left-aligned, no top_label
  - `CategoryItem`: indented ~30px, left-aligned, top_label + bottom_label
  - `StandaloneItem`: same as `CategoryItem` but no indent
- Add row background quads: unfocused `rgb(0.2, 0.2, 0.2)`, focused `rgb(0.35, 0.35, 0.35)`, smooth 0.1s transition
- Use 9 visible wheel slots (`WHEEL_SLOTS = 9`, `WHEEL_FOCUS_SLOT = 4`)
- None of this touches `classic.rs`

**Step 6: Implement `categories::handle_input()`**

- Up/Down: navigate the flattened `Vec<Entry>` wheel
- Select on `CategoryHeader`: toggle `expanded` for that category, rebuild entry list, cursor stays on header
- Select on `CategoryItem` / `StandaloneItem`: return the `Action` for `select_music.rs` to handle (same as classic path)
- Back/Select: close menu
- Persist `CategoryState` and `selected_index` for re-entry memory

**Step 7: Polish category UI**

- Tune row spacing, font sizes, indent for left-aligned layout
- Ensure cancel hint text shows "Press SELECT To Cancel"
- Test expand/collapse animation, wrapping behavior
- Verify all existing actions (sorts, overlays, style switch, etc.) work through the new path

### Phase 3: New sort orders

**Step 8: Add Genre sort**

- Add `genre: Option<String>` field to `SongData`
- Parse `#GENRE` tag in SM/SSC parsers (search for where `#ARTIST` is parsed and add `#GENRE` nearby)
- Add `WheelSortMode::Genre`, `Action::SortByGenre`
- Add `ITEM_SORT_BY_GENRE` const, insert into Sorts category
- Implement sorting: alphabetical by genre, empty-genre songs grouped under "Unknown Genre" or similar
- Verify: genre sort works with songs that have/lack genre tags

**Step 9: Add Top Grades sort**

- Add `WheelSortMode::TopGrades`, `Action::SortByTopGrades`
- Add `ITEM_SORT_BY_TOP_GRADES` const, insert into Sorts category
- Implement sorting: use `scores::get_cached_score_for_side()` to get best grade per chart, sort descending (best grades first), unplayed songs at bottom
- Verify: sort works, unplayed songs handled gracefully

**Step 10: Add per-player sort variants**

- Add `Action::SortByPopularityP1/P2`, `Action::SortByRecentP1/P2`, `Action::SortByTopGradesP1/P2`
- Add corresponding `WheelSortMode` variants
- Add item consts, insert into Profile category (only visible when profile loaded)
- Implement: filter existing popularity/recent/grades logic by player side
- Depends on: per-player play count data being available in the scores system — may need to extend `played_chart_counts_for_machine()` with a per-profile variant
- Verify: Profile category shows/hides based on loaded profiles

### Phase 4: Favorites

**Step 11: Add favorites storage**

- Define storage format: list of chart hashes or song directory paths, stored per-profile
- Add to profile data model: `favorites: HashSet<String>`
- Load/save favorites when profile loads/saves
- Add helper: `profile::is_favorite(song)`, `profile::toggle_favorite(song)`

**Step 12: Add Favorite toggle action**

- Add `Action::ToggleFavorite` and `ITEM_TOGGLE_FAVORITE`
- Insert as `StandaloneItem` (visible when song selected)
- On select: call `profile::toggle_favorite()`, update label to "Remove Favorite" / "Add Favorite" dynamically
- Play a sound on toggle

**Step 13: Add Favorites/Preferred sort**

- Add `WheelSortMode::Favorites`, `Action::SortByFavorites`
- Add item const, insert into Profile category
- Implement: filter music wheel entries to only favorited songs, sort by title within
- Handle empty favorites list gracefully (show message or skip)

### Phase 5: Cutover

**Step 14: Flip the default**

- Set `use_category_select_music_menu` default to `true`
- Test thoroughly — all sorts, overlays, edge cases
- Ship with both paths available so users can fall back if needed

**Step 15: Remove old path**

- Delete `classic.rs`
- Remove `State::Classic` variant
- Remove `ITEM_CATEGORY_SORTS` and `Action::OpenSorts`
- Remove `ITEMS_SORTS` array
- Remove `Action::BackToMain` and `ITEM_GO_BACK`
- Remove old `select_music_menu_items()` in `select_music.rs`
- Remove all `State::Classic` match arms in `select_music.rs`
- Rename `State::Categories` → `State::Visible`
- Remove the `use_category_select_music_menu` config flag
- Verify: `cargo check`, test with 0/1/2 profiles, no songs, many songs, various resolutions
