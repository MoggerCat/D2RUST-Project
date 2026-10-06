# Handoff: real-file providers for level generation — `claude/drlg-data`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Cloud implementation session, 2026-10-06, from `main` at `edad871`. Task
class: integration from clear specs, medium (METHODS M14). Scope of every
claim: this branch, synthetic DS1 / DT1 bytes, no game files (M09).
HANDOFF §2 step 7 "Open: parse DS1 for `Ds1Source` / `SubFiles` …".

## 1. State

**Implemented; the live-file check is queued (§6), so unverified** (M02).
`d2_server::world_data` provides the DRLG data seams from the user's files:

- `Ds1Files: Ds1Source` (lvlprest DS1s), `SubFileMap` / `WorldFiles:
  SubFiles` (lvlsub DS1s), `Dt1Files: TileSource` (lvltypes DT1s + the
  fixed library), all parsed by `d2_formats::{ds1, dt1}` and handed to
  the sim as the existing seam types. Keys are the exact table strings
  the sim asks with.
- `tables::LevelTables::from_fixed(&FixedSet)`: `DrlgData`,
  `PresetData` (lvlprest rows, monpreset rows and act ranges, monstats /
  superuniques counts, the item class of `hdm `, `preset-tables.tsv`),
  `OutdoorData`, `MazeData`, each through its own `from_tables` /
  `from_record`.
- `archive::load(&ArchiveSet)`: `bin::load` → `fixup::apply` →
  `LevelTables` → `WorldFiles::load` (everything loaded up front; a named
  file that is missing or does not parse is an error, nothing falls
  back).

The module adds no DRLG rule: the 1.14d parser rules of `preset.md`
§5.2–§5.3 stay in `d2_sim::drlg::preset::Ds1File::from_input`; the
provider only re-expresses the parsed file (stored width / height =
`d2_formats` value − 1, orientations as stored, act read as i32).

Tests: 8 CI tests on synthetic bytes (`world_data/tests.rs`) and 3
`#[ignore]` game-file tests (`world_data/tests/game.rs`, never run).

## 2. Code map rows (for `docs/HANDOFF.md` §3)

| Path | What | Spec |
|---|---|---|
| `crates/d2-server/src/world_data/mod.rs` | `WorldDataError`; `archive_name` (§12 tile path), `names_file`; `ds1_input` (DS1 → `Ds1Input`), `sub_file` (DS1 → `SubFile`), `tile_info` (DT1 tile → `TileInfo`); `Ds1Files`, `Dt1Files`, `WorldFiles::load` | `preset.md` §5, `outdoor-tilesub.md` §1, `rooms.md` §9.3, `fixups.md` §12 |
| `…/world_data/tables.rs` | `LevelTables::from_fixed`: the four level-type table views | `preset.md` §5.3, §13; `levels.md`; `maze.md` §1 |
| `…/world_data/archive.rs` | `reader`, `fixed_tables`, `load` over `ArchiveSet` | `formats/mpq.md`, `data/loading.md` |
| `…/world_data/tests.rs` | synthetic DS1 / DT1 bytes through the providers | |
| `…/world_data/tests/game.rs` | Act I from the live data through `WorldTypes` (`#[ignore]`) | |

## 3. Signature changes

None in another crate. `d2-server`: `d2-data` moved from dev- to normal
dependency, `d2-formats` added (depcheck OK: no Bevy, no forbidden
pair); `lib.rs` gains `pub mod world_data;`.

## 4. Design notes

- **Path keys.** The sim asks `Ds1Source` / `SubFiles` / `TileSource`
  with the table strings (lvlprest `File1`–`File6`, lvlsub `File`,
  lvltypes `File n`) or `FIXED_LIBRARY`. With tables from `FixedSet`
  these already carry `DATA\GLOBAL\TILES\` (`fixups.md` §12); raw tables
  do not. `archive_name` uses a string that already starts with the
  prefix (ASCII case-insensitive) as is, else applies §12 (`/` → `\`,
  prefix when longer than one character). Strings of 0–1 characters
  (the `0` placeholders) name no file and are not loaded; if the sim ever
  asks for one it gets `None` and reports its own error.
- **All named files, not only the ones a level picks.** Every lvlprest
  `File1`–`File6` string, not just `File1..Files`: the maze and outdoor
  code set the picked file themselves, and the cost (2,043 DS1s
  measured, `preset.md` §5.3) is a one-time load.
- **lvlsub files** go through `Ds1File::from_input` (the lvlsub loader
  is the preset DS1 loader, `outdoor-tilesub.md` §1 r3), so `method` is
  the tag type after `preset.md` §5.2 step 3, the floor grid is absent
  for v < 4, tile types are the orientations after step 6, and the units
  are the file's preset-unit list (head first). Group variants = the
  group's last value (tilesub OQ1).
- **DS1 v < 7 is refused** (`WorldDataError::OldOrientations`):
  `d2_formats::ds1` already maps their orientations with the 25-entry
  table and the sim's 42-entry remap needs the stored values. No
  lvlprest DS1 is below v12 (`preset.md` §5.3); lvlsub files carry
  groups, which need v ≥ 12. If a v < 7 file is ever needed, `d2_formats`
  needs a raw-orientation option (its owner's change).

## 5. Seams reached (unchanged, still unspecified)

- `LevelTypes::warp_unit` (`0x0066E1C0`), the type-2 tile free
  `0x00666610`: not specified, nothing done.
- The DRLG population reads (coordinate lists `0x0061AD50` /
  `0x0061AD30` / `0x0061B130`, populated level, warp points, level spawn
  of kind 11, nearest free point): spec work (HANDOFF §5 B), untouched.
- `DrlgData::wall_remap` stays `None` and `DrlgData::doors` empty as
  `DrlgData::from_tables` builds them (no spec transcribes them into a
  table view yet; the door table reaches the preset code through
  `PresetTables`).

## 6. Local checks to queue (HANDOFF §5)

DL1. With `D2_GAME_DIR` set:

```
cargo test -p d2-server world_data -- --ignored
```

Expected: 3 pass.

- `act1_placement_matches_the_recorded_vector`: the live tables and
  files load without error; act 0 created with init seed 644409375 and
  the town (level 1) generated: `dwStartSeed` 4014346869, DRLG seed
  {1406222081, 1674353446}, allocation order 4, 3, 2, 1, 17, 39, 26, 7,
  6, 27, 5 (recorded); rects Stony Field (1000, 1000, 80, 80), Cold
  Plains (920, 984, 80, 80), Blood Moor (904, 1064, 56, 96), Rogue
  Encampment (960, 1112, 56, 40), Burial Grounds (880, 968, 40, 48) and
  origins Moo Moo Farm (5000, 1148), Gate (3000, 1000), Tamoe (3000,
  1018), Black Marsh (2920, 1002), Dark Wood (2904, 1082) (these rects
  are `outdoor.md`'s "derived from the rules, not yet recorded": a
  mismatch there is a question for the spec as much as for the code).
- `den_of_evil_matches_the_maze_vector`: Den of Evil (8) maps head first
  def 57, 86, 96 at (+24, 0), (+24, +24), (0, 0) from the level origin,
  every room a preset room (`maze.md` vector).
- `outdoor_levels_generate_through_the_dispatcher` (WG9): Blood Moor 81
  rooms (33 outdoor), rect (904, 1064, 56, 96); Cold Plains 98 rooms (37
  outdoor) (`outdoor.md` per-level room allocations); streaming Blood
  Moor's first room loads its DT1 library without error.

A failure names the first wrong value; a load error names the archive
path. If it passes, `wire-worldgen.md` §7 check 1 (the RNG trace compare
of the preset draws, town and Blood Moor builds) is the next step: these
tests compare outcomes, not the draw sequence.

## 7. Questions

- DQ1. `rooms.md` §9.3 names the fixed library with `Tiles` (mixed
  case) and says the provider resolves paths case-insensitively; the
  archive lookup is case-insensitive, so `archive_name` keeps the case.
  The DT1 map is keyed by the exact string, so a request that differs
  only in case from a loaded key is not found (none does today).

## 8. Gate results

All pass on this branch: `cargo fmt --all -- --check`; `cargo clippy
--workspace --all-targets -- -D warnings` (after `sh
tools/cloud-setup.sh`); `cargo test --workspace` (exit 0; d2-server 107
pass, 3 ignored, of them 8 new pass and 3 new ignored); `cargo run -p
depcheck` (8 crates OK); `python3 tools/spec_index.py --check`; `python3
tools/methods.py check` (21 OK); `python3 tools/coverage.py --check`
(3,201 claims, 0 errors) and `--selftest`.

Non-source files in the diff: `Cargo.lock` (the two new d2-server
dependency edges), this note.
