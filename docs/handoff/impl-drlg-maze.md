# Handoff: maze level generation (`d2_sim::drlg::maze`)

Branch `claude/impl-drlg-maze`, from `claude/bold-ptolemy-jvyvxy` at
a5b323a (cloud, 2026-10-06). Spec: `specs/drlg/maze.md` +
`specs/drlg/maze-specials.tsv`.

## State

**Implemented, unverified.** Every rule of the spec has code; the spec's
two synthetic vectors (Den of Evil, Spider Cavern) reproduce exactly,
with the spec's seed values, on a fake preset seam. 26 new tests in
`crates/d2-sim/src/drlg/maze/tests.rs`; `cargo test -p d2-sim`: 538
passed, 3 ignored. Coverage (`py tools/coverage.py`): `maze.md` 52/53
units claimed at the unit tier (edge case 9, the unreachable table rows,
has no test). Nothing is trace- or game-file-checked
yet: no recording generates a maze level (spec open question 1).

Changes outside the new submodule: one line `pub mod maze;` in
`crates/d2-sim/src/drlg/mod.rs`. No dependency changes.

Gate run before push: `cargo fmt --all -- --check`, `cargo clippy -p
d2-sim --all-targets -- -D warnings`, `cargo test -p d2-sim`, `cargo run
-p depcheck`, `python3 tools/spec_index.py --check`, `python3
tools/methods.py check`, `python3 tools/coverage.py --check`: all clean.

## Code map rows (for HANDOFF §3)

| Path | What | Spec |
|---|---|---|
| `crates/d2-sim/src/drlg/maze/mod.rs` | `MazeRow` / `MazeData` (lvlmaze, lvlprest `Files`, specials; `from_tables`), seam `MazePresets`, `MapId`, `MazeError`, `Rotation`, `MazeLevel`, `Maze` (`init_level`, `generate`, `reset_level`, `free_level`) | `maze.md` §1, §9 |
| `crates/d2-sim/src/drlg/maze/specials.rs` | strict parser of `maze-specials.tsv` (`include_str!`, no copy of the rows in code) | §3.6 |
| `crates/d2-sim/src/drlg/maze/cells.rs` | `Cell`, `MazeLink`, `LinkTarget`; geometry (`adjacent`, `gaps`, `collide`, `direction`), `shape_def` / `shape_override`; `Gen`: allocate, place test, pick, merge, grow / fixed / special fallback / blank, stamp, probe, extreme finders, random cell, bounding box | §2–§3 |
| `crates/d2-sim/src/drlg/maze/layout.rs` | `generate` (§4), ring, grow tree, catacomb start, hub, spiral, lava cross, fill blanks, A3 sewer swaps, per-type special builders, lair, A2 sewers, tombs, temple, Barracks, River of Flame, normalize, theme pass, build + file rotation | §4–§9 |
| `crates/d2-sim/src/drlg/maze/tests.rs` | spec vectors and rules on `FakePresets`; TSV cross-check with perturbation test (M05, M08) | |

## Design

- Cells are real DRLG rooms (`Drlg::alloc_room`, kind `Preset`), so the
  two allocation draws (§3.1), the level list (newest first, room count)
  and room seeds are the merged `drlg` code's. Maze data per cell (def,
  file, lock, links) lives in `Gen::cells` beside them, because
  `drlg::room` has no orth links. Unlinking a listed cell and freeing use
  the `pub(super)` fields/`free_room` of `drlg` (no edits there).
- `Maze` is **not** a `LevelTypes`. The composite provider that
  dispatches by DrlgType (wiring session) calls `Maze::init_level` from
  `LevelTypes::init_level` (DrlgType 1), `Maze::generate` from
  `generate`, `Maze::reset_level` from `reset_level`. Type data is keyed
  by (act, level slot), so one `Maze` serves all five acts.
- **Seam request:** `DrlgError` has no variant to carry `MazeError`; the
  wiring session adds one (e.g. `DrlgError::Maze(MazeError)`) or maps it.
- Special tables are parsed at run time from the spec TSV (single
  source, M05). `specials_tsv_parses` cross-checks every row against the
  code's shape tables (row r = N/E/S/W finds base + 8/2/4/1, fallback
  dirs 3/0/1/2, temple and lava rows by their own shapes) and checks
  every table the code stamps exists with the rows it uses;
  `specials_check_catches_perturbations` proves the check and the strict
  parser report exactly a changed cell (M08).

## Seam `MazePresets` (provider: `drlg::preset`, parallel session)

| Method | Spec | Notes |
|---|---|---|
| `level(drlg, data, id)` | §7 (`0x00642BB0`) | get-or-allocate through the act's level types (27, 108) |
| `preset_direction(drlg, level)` | §7.1 (preset data +4) | Outer Cloister's direction q |
| `alloc_map(drlg, data, level, def, rect) -> MapId` | §9 step 1 (`0x00666ED0`) | must draw `roll(Files)` on the level seed (the default file) |
| `set_map_file(map, file)` | §9 step 2 | called only when the cell has an explicit file or rotates |
| `build_map(drlg, data, level, map, small, links)` | §9 steps 3–4 (`0x00667ED0`) | `links` = the cell's init-flag links in list order; the cell is freed right after |

## Open questions (items 1–6 have a `TODO` in code)

1. **Spiral file timing** (§5.5, `layout.rs` `spiral`): "Afterwards every
   kept cell … gets file (r + b) mod 4" is read as after each branch. A
   later branch's merge re-pick sets file −1 again, so "after all four
   branches" would differ whenever Arcane's `Merge` links across
   branches.
2. **Link list order** (§2.4, `cells.rs` `Cell::links`): prepend or
   append not stated; links are kept in creation order. Only reaches the
   preset builder (§9 step 4).
3. **Cross-level link target** (§7.1, `LinkTarget::Level`): taken as the
   level; its box (if ever place-tested) is the level rect. Never read on
   1.14d paths (the cells holding such links are locked).
4. **Built-room linking** (§9 step 4, `MazePresets::build_map`): which
   built room gets the links when one DS1 builds several rooms, and what
   a link to an already-built (freed) cell resolves to, belong to
   `drlg/preset.md`; delegated to the provider.
5. **Unlisted levels of a type** (§6, `layout.rs`): a maze level the
   per-type table does not name draws r and stamps nothing (no such level
   in 1.14d data). Same for lair levels other than 62–64 and Act 2 sewer
   levels other than 47–49, 65 (the latter stated by the spec).
6. **Probe link removal** (§3.7): "net: P unchanged" is implemented by
   removing P's link to the freed probe cell; rooms.md does not describe
   what `0x0066C100` does to the neighbour's link list.
7. **Rows of `lvlmaze` for regeneration**: `reset_level` (keep = 1)
   keeps the lvlmaze record and clears the rotation list (the DRLG frees
   the build list +0x1CC in that path); `free_level` drops both. No
   caller frees a level without keeping it yet.
8. Infinite loops kept as in the original: grow tree loops while the
   room count is below target even if no cell can grow.

## Checks to queue (HANDOFF §5, local)

1. **Trace (spec OQ 1):** record entering Den of Evil (8) and Cave
   Level 1 (9) with the RNG recorder; with the preset provider wired,
   `Maze::generate` on the live tables must reproduce every level-seed
   and room-seed draw at the maze sites (0x670C70–0x673FE0) and the
   preset draws after them, in order. Expected: 0 mismatches.
2. **Game files (no test written yet):** `MazeData::from_tables` on the
   live `lvlmaze.bin` / `lvlprest.bin`: 81 lvlmaze records; every
   `levels.txt` DrlgType 1 level has a record; every def `shape_def` can
   return for each maze level type and mask 1..15 (non-zero), every
   `maze-specials.tsv` special def, and the fixed defs of §4–§7 (167,
   288–290, 333, 336, 444–447, 480, 735–738, 836, 852–856, 1038–1041,
   1074–1077) exist in lvlprest with `Files` ≥ 1. Add as an `#[ignore]`
   test reading `D2_GAME_DIR` once the preset session's table loading
   lands.
