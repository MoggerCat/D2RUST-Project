# Handoff: DRLG outdoor levels (`d2_sim::drlg::outdoor`)

Branch `claude/impl-drlg-outdoor`, from `claude/bold-ptolemy-jvyvxy` at
a5b323a (cloud, 2026-10-06). Specs: `specs/drlg/outdoor.md`,
`specs/drlg/outdoor-tilesub.md`. Implementation session (M14: medium).

## State

**Implemented, unverified** (Act I and the tilesub spec in full; Acts
II and IV as specified; Acts III and V only where the spec gives rules).
41 unit tests in `crates/d2-sim/src/drlg/outdoor/tests.rs`;
`cargo test -p d2-sim`: 553 pass (3 ignored). Recorded vectors reproduced as unit
tests:

- **Act I placement** (`outdoor.md` Test vectors): `Drlg::create(0,
  644409375, …)` with synthetic leveldefs sizes/offsets gives start seed
  4014346869, copy-1 draws seq 2419–2424, allocation order 4, 3, 2, 1, 17,
  39, 26, 7, 6, 27, 5, the derived rects of Stony Field, Cold Plains,
  Blood Moor, Rogue Encampment (direction 3), Burial Grounds and the
  monastery chain (Dark Wood retried 3 → 0), Outer Cloister direction 1,
  the DRLG seed after creation {1406222081, 1674353446}, Act I flags 0
  for levels 2 and 3. Passed first time.
- **Blood Moor room sub-theme pick** (`outdoor-tilesub.md`): room seed
  {223305360, 666}, init 2795816810, lo' values of seq 6646–6651, mask
  0b010100, DT1 mask bits.

Changes outside the new module: one line `pub mod outdoor;` in
`crates/d2-sim/src/drlg/mod.rs`. No dependency changes.

Gate before push: `cargo fmt --all -- --check`, `cargo clippy -p d2-sim
--all-targets -- -D warnings`, `cargo test -p d2-sim`, `cargo run -p
depcheck`, `python3 tools/spec_index.py --check`, `python3
tools/methods.py check`, `python3 tools/coverage.py --check`: all clean.
Coverage (unit tier): `outdoor.md` 47/72 units, `outdoor-tilesub.md`
28/30. Uncovered: the Act II–V sections (§2.1–§2.2, §2.4, §2.6 Act
II/IV/V paths, §8–§11), §7.4 special presets, §12.2 substitution order,
`outdoor-tilesub.md` §2.1 callers and edge case 4.

## Code map rows (for HANDOFF §3)

| Path | What | Spec |
|---|---|---|
| `crates/d2-sim/src/drlg/outdoor/mod.rs` | `Outdoor` state (info per level, outdoor room data, preset directions), `OutdoorData` (leveldefs Sub*, lvlprest sizes/Files, lvlsub rows), seams `OutdoorPresets`, `SubFiles` (+ `SubFile`, `SubFileMap`), `OutdoorError`, `OutdoorTypes` adapter to `LevelTypes` | `outdoor.md` §1, §3 |
| `…/outdoor/grid.rs` | `Grid`, grid-2 bits, `Gen` context, build list + stamp, fit test, shuffle, placers (S, R, FarAway, waypoint, shrines) | §1.2, §5 |
| `…/outdoor/place.rs` | link tables, driver, linkers, Place A/B/C, checks, Act I flags, adjacency warps, neighbour entries, jungle blocks, Kurast chain | §2, §9.1, §9.2 |
| `…/outdoor/vertex.rs` | vertex polygon, cell merge, link vis flag, link flags, border tables N/P/Q, borders, blank corners | §4, §5.5, §6 |
| `…/outdoor/wild.rs` | Act I: cliff marking, river/caves, transitions, river + bridge, cottages, special presets, path starts/joins, grid path search, jitter | §7 |
| `…/outdoor/acts.rs` | Act II sequences, variants, cliffs, tomb row; Act III (partial); Act IV mesas/sanctum; Act V siege strip (rest TODO) | §8–§11 |
| `…/outdoor/rooms.rs` | generation `0x00675360`, cells → rooms, outdoor room creation and grids | §3, §12 |
| `…/outdoor/tilesub.rs` | lvlsub rows/files, border substitution (wild and barricade callbacks, Act V style map), sub-theme pick, room substitution (CheckAll, scattered), tests, apply | `outdoor-tilesub.md` |

## Seams (traits) and wiring

| Seam | Methods | Provider |
|---|---|---|
| `OutdoorPresets` | `build_preset_cell(drlg, data, level, def, x, y, file, room_flags)`: `preset.md` §4 map alloc (draws `roll(Files)` on the level seed, site `0x00666F33`), set file, §6 build area | `drlg::preset` (parallel session) |
| `SubFiles` | `sub_file(name) -> Option<&SubFile>`: parsed lvlsub DS1 (groups with variant count, floor layer 0, wall + tile-type layers, shadow, units) | world / d2-server holding parsed DS1s |

Wiring for the world session: one `Outdoor` per act DRLG, kept beside
the `Drlg`. Either use `OutdoorTypes { outdoor, od, subs, presets,
others, last_error }` as the `LevelTypes` (it handles DrlgType 3 levels
and the outdoor rooms, forwards everything else to `others`), or call
`Outdoor::create_act_levels / init_level / generate / reset_level /
room_grids / free_room_tiles` from a combined dispatcher. The act placer
allocates levels through the `types` it is given; outdoor inits are lazy
(`Outdoor::info_mut`), so pass a provider whose outdoor init is a no-op
(the adapter does). `Outdoor::preset_direction` holds the preset
direction of levels 1, 27, 40 for `drlg::preset` (preset info +0x04).

**Seam requests:** (1) `DrlgError` has no outdoor variants; the adapter
reports non-DRLG outdoor errors as `DrlgError::UnknownLevel(id)` and
keeps the real one in `last_error` — add `DrlgError::Outdoor`. (2) The
outdoor tile fill (`rooms.md` §9.5) has no shadow pass: shadow tiles and
roof growth from room substitution are kept in `OutdoorRoom::shadows` /
`roof_count`, and substitution preset units in `OutdoorRoom::units`
(`preset.md` owns unit records).

## Open questions (each has a `TODO` in code)

1. **Not specified, not built** (no draws): Act III jungle attach points
   and per-level jungle ids (§9.1, OQ 7: Act III creation draws fewer
   times than 1.14d), jungle stamping beyond its `roll(14)`, Kurast and
   Travincal (§9.3, OQ 8); Act V everything except the siege strip
   (§11, OQ 9); Act I path floor (§7.5.3, OQ 6).
2. **Neighbour list order** (§1.4, `0x0066B790`): head insertion used.
3. **Room list order** of outdoor rooms (§12.2 "add to the level"): head.
4. **Polygon merge** (§3 step 2): pair (last, head) not merged.
5. **Blank corners** (§6 step 5): column pass read as the transposed row
   pass.
6. **Grid path** (§7.5.1): root counted toward the 900 nodes; "reaching
   the root fails the round" read as the root's own tries reaching 3.
7. **Desert cliff rows** (§8.3): order of the three wall/path entries in
   rows 0–2 and 5–7 not given; ascending used (decides which first stamp
   draws its build-list roll first).
8. **Zero pieces**: straight border piece 0 (cliff style on W/S edges),
   desert pair (0, 0), style-map P ≤ 0: not stamped (lvlprest row 0 has
   Files 0). Border flag of the §6 stamps: clear.
9. **RandomDS1** (§5.4): scans every path cell before falling back.
10. **Small/odd sizes**: shuffle and FarAway with W or H ≤ 0, GridSize 0,
    B2 index outside 0..3, river U/L for P outside 0/4..15, link-probe
    neighbour without a vis slot: read as "nothing"/0.
11. **Reset** (`0x006754C0`): keeps flags and neighbour entries, so a
    regeneration starts with the flags (0x20/0x40) of the last one.
12. Unallocated levels in the adjacency/neighbour passes: skipped. More
    than 6 path starts: kept (original has 6 slots).
13. Room grid edges |= 0x4 on all four sides; Act III jungle blocks kept
    in tiles ("32-tile blocks").

## Checks to queue (HANDOFF §5, local)

Need `drlg::preset` merged and the world providers (DS1 parser for
`SubFiles`); then, as `#[ignore]` tests reading `D2_GAME_DIR` with the
live leveldefs/lvlprest/lvlsub:

1. **Act I placement on live tables**: `Drlg::create(0, 644409375, 0,
   1, false, …)` with `OutdoorTypes`; expect the rects of
   `outdoor.md` Test vectors ("derived" list) and OQ 1, DRLG seed
   {1406222081, 1674353446}.
2. **Blood Moor / Cold Plains build draws**: generate levels 2 and 3 and
   compare the (site, seed-after) sequence with
   `20261005-232125-rng.jsonl` seq 2561–6331 (Blood Moor) and 6896–12010
   (Cold Plains); per-site counts of the table in `outdoor.md` Test
   vectors (e.g. Blood Moor build list 15, shuffles 100/100, jitter
   3/11/11, shrines 1/50/50, preset cells 48, rooms 81, sub-theme 198).
3. **Blood Moor room substitution** (`outdoor-tilesub.md` Test vectors):
   reset seed {2795816810, 666} → 10 group rolls, trials 1, 14, 7, 20,
   20, 20, 5, 20 and 3, 5; 115 (x, y) pairs (seq 32974–33381).

No checker added here; M08 perturbation not applicable (unit tests
against spec vectors).
