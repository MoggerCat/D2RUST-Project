# Handoff: game from a `FixedSet` — `claude/fixedset-game`

Cloud implementation session, 2026-10-06, HANDOFF §2 step 7e
(`synthetic-data` §6). Repo only, no game files. Medium effort.

## 1. State

The synthetic install now goes all the way to a running server game in
CI: archives → `bin::load` → `fixup::apply` → table views and
`d2_server::world_data` providers → `WorldSim` → one-room town generated
and streamed → waypoint and player allocated → `SimGame::join` →
`Host::frame` × 51. No sim behaviour changed.

1. **DS1 writer** (`test_fixtures::ds1::write`, spec `formats/ds1.md`):
   the inverse of `d2_formats::ds1::Ds1::parse` for versions 1–18 (every
   version condition of §Rules steps 1–12). Takes a `Ds1` value; width /
   height stored − 1; orientations written as given (for v < 7 they are
   the stored, pre-lookup values); groups written whole; for v ≥ 14 a
   path count is always written. `blank(w, h, act)`: an empty v18 preset.
   Round trips: every version 7–18 × tag types 0/1/2, all layers and
   records filled; v1–6 with stored orientation 7 → 0x05 (spec vector);
   the minimal v18 layout (spec vector: wall, orientation, floor, shadow,
   no tag); trailing bytes.
2. **DT1 writer** (`test_fixtures::dt1::write`, spec `formats/dt1.md`):
   header, consecutive tile headers at 276, per tile block headers then
   data, data offsets relative to the tile's block headers. Iso blocks
   from the 32×15 decoded pixels (SKIP / RUN rows), other formats RLE
   `(skip, count)` runs of non-zero pixels, `(0, 0)` per row, rows after
   the last pixel not written. Round trips (iso, 0x1001, 0x2005, a
   0-block tile, an empty file); the iso and RLE spec vectors; header
   fields. `tile(o, main, sub, rarity)`, `file(tiles)` helpers.
3. **Synthetic DRLG files** (`test_fixtures::drlg`, written into
   `d2data.mpq` by `install` stage 1 from the new `Synthetic::files`):
   - Town (`Synth\Town.ds1`, lvlprest Def 0, level 1): 9 × 9 v18 cells
     (stored size 8 = lvlprest `SizeX` / `SizeY`, now the constant
     `content::PRESET_SIZE`), every floor cell 0x2 (`rooms.md` §9.4 floor
     bit, key (0, 0)), no walls, one object: type 2, id 150 at sub-tile
     (22, 22). Id 150 gives class `150 − 150 = 0` (`preset.md` §5.2), the
     synthetic `objects` row 0 (Waypoint, operate function 23).
   - Keep, cave and (new) field: the same grid without the object.
     Sub (`lvlsub`): 3 × 3, tag type 1, one 2 × 2 group, 1 variant.
   - DT1: each floor file one (0, 0, 0) tile, the wall file one (1, 0, 0),
     the fixed library (`rooms.md` §9.3) Blank.dt1 (0, 30, 0/1),
     InvisWal.dt1 empty, Warp.dt1 (10, 0, 0). Rarity 1, sub-tile flags 0.
   - Archive names: `DATA\GLOBAL\TILES\` + table string (`fixups.md` §12;
     the fixed-up lvlprest / lvltypes strings carry that prefix, which
     `WorldFiles` keys on). The fixed library under its own names.
4. **Content changes** (`content.rs`): the table strings lost their
   `Tiles\` prefix (`Synth\Town.ds1`, `Synth1\Floor.dt1`, …) and come from
   the constants `FLOOR_DT1`, `WALL_DT1`, `PRESET_DS1`, `SUB_DS1`,
   `PRESET_SIZE`. **New lvlprest row** Def 3 for level 2 ("Synth Field",
   DrlgType 2): the town's `Vis0` links level 2, so the first tick's room
   update builds it, and a preset level without a def is
   `NoPresetForLevel(2)`. No other row changed; `server_tables` and
   `synthetic_load` pass unchanged.
5. **Game constructor** (`test_fixtures::game`): `GameData { bins, anim,
   fixed, level: LevelTables, files: WorldFiles }`, `GameData::load(bins,
   &ArchiveSet)` / `from_install(&Install)`; builders `action_tables`,
   `world_tables`, `vitals`, `stat_data`, `unit_data` (expansion),
   `waypoints`, `level_types`, `drlg_world(…, ActCreation, init, town)`,
   `world_sim(ActCreation, init, town, game_seed, x)` (hooks with
   AnimData, vitals, paths enabled; `create_regions`). `Seams` (the
   interaction-owner / transport bookkeeping of the live host test) and
   `type Sim = SimGame<WorldSim<Seams>, ActionWorld>`. The builders are
   the ones `d2-server/tests/game_wired_host.rs` assembles by hand on the
   live set; they work for the user's install too (`GameData::load`).
   `test-fixtures` now depends on `d2-sim` and `d2-server` (normal deps;
   the crate is still test-only and pulls no Bevy).
6. **E2E** (`crates/test-fixtures/tests/synthetic_game.rs`, CI, no
   `#[ignore]`):
   - `providers_hold_the_named_files`: 4 lvlprest DS1, 1 lvlsub, 6 DT1
     (2 floors + wall + 3 fixed), keyed by the fixed-up archive names; the
     town's stored size and waypoint record; `world_data::archive_name`
     agrees with the fixture's.
   - `town_join_and_frames`: the town is one room of 8 × 8 tiles; 81
     floor records, all from the (0, 0, 0) floor tile, no walls or
     shadows; one preset unit, the waypoint (objects row with operate
     function 23); the object and a class-3 player allocated (creation
     life `(vit + hpadd) << 8`, `vitals.md` §1); `SimGame::join`,
     `Host::connect`, a first frame, then 50 frames each with one tick,
     no tick fault, no world fault, no wiring error; the player inside
     the town room; a second run equal (determinism).

## 2. The seam: act creation (open)

`Drlg::create` over the level types runs the act placer (`levels.md` §3
step 7): for act 0 the A1W / A1M chains and the neighbours of 1..17
(`drlg/outdoor/place.rs`), which allocate 1.14d level ids 1–7, 17, 26,
39 and expect their 1.14d types. The 8-row made-up levels table has none
of that (`UnknownLevel(17)`). So the fixture has
`ActCreation::TownOnly`: `Drlg::create` with `NoLevelTypes` (steps 1–6,
the placer a no-op, no town) and then the town allocated and generated
over the real level types. The DRLG seed then skips the placer's draws:
a fixture state, not a 1.14d one. `ActCreation::Full` is the 1.14d path
for real data. Making `Full` work on synthetic data means a synthetic
levels table shaped like Act I (ids 1–39 with the chain's types and
outdoor data): its own task, if wanted.

## 3. Not done / limits

- No message from the server reaches the idle joined client in 50
  frames (no join sequence is wired at this stage); the test records
  the count (0) and compares runs, it asserts nothing about it.
- The outdoor (lvlsub) and maze paths do not run on synthetic data (only
  the town and its preset neighbour are generated); `Sub.ds1` and the
  cave / keep files only have to load and parse.
- No `Covers:` claim: the writers' round trips are test support; the
  e2e checks invariants on made-up data, not 1.14d.

## 4. Local run queue

None needed: everything runs on synthetic data in CI. Optional: once a
local session has the live set, `GameData::load(bins, &archives)` with
`ActCreation::Full` should reproduce the setup of
`d2-server/tests/game_wired_host.rs`; folding that test onto the
constructor is a refactor for whoever owns that file.

## 5. Files

| Path | What |
|---|---|
| `crates/test-fixtures/src/ds1.rs` | DS1 writer + round trips |
| `crates/test-fixtures/src/dt1.rs` | DT1 writer + round trips |
| `crates/test-fixtures/src/drlg.rs` | the synthetic DS1 / DT1 files |
| `crates/test-fixtures/src/game.rs` | `GameData`, `ActCreation`, `Seams`, `Sim` |
| `crates/test-fixtures/src/{content,synth,install,lib}.rs` | path constants, field def, `Synthetic::files`, packing |
| `crates/test-fixtures/tests/synthetic_game.rs` | the e2e |
| `crates/test-fixtures/Cargo.toml` | deps `d2-sim`, `d2-server` |
