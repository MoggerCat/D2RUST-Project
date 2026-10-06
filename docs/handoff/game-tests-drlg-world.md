# Handoff: game-file tests for the world and DRLG tables (branch `claude/game-tests-drlg-world`, 2026-10-06)

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Scope: `#[ignore]` game-file tests (`D2_GAME_DIR`) for `specs/world/*.md`
and the table-level parts of `specs/drlg/*.md`, in two new integration
test files of `d2-sim`. Cloud session (repo only), medium effort; base
`claude/tender-meitner-mphas3` at `4b5b0bf`. Read `specs/`, `docs/`,
`crates/` only. No code, spec, `HANDOFF.md` or `PLAN.md` change; the DS1 /
DT1 file providers are the `drlg-data` session's (`d2-server`), so nothing
here reads a DS1.

None of these tests has run: there are no game files in the cloud. They
compile, pass clippy and are ignored in CI. **Every expected value is
unconfirmed** (blind-written, `HANDOFF.md` §8 lesson of 2026-10-06): the
values come from the specs, not from an observation of the live files, so
the tests carry **no `// Covers:` claim yet**. The "Claim" columns below
are the claims to add after the first local run passes (§3); a test that
fails is corrected from the observation (or the spec fact turned into an
open question) before any claim is added.

The merged base (`claude/tender-meitner-mphas3` at `5edceb8`, group C
results) confirms one value used here: `monstats` 734 records in the
patched set (HANDOFF §5 Done). Nothing else in group C covers these
tables.

## 1. Files

| File | Tests |
|---|---|
| `crates/d2-sim/tests/game_world.rs` | 10: waypoints (3), quests (1), cube (2), npc (1), vendors (3) |
| `crates/d2-sim/tests/game_drlg_tables.rs` | 8: leveldefs (2), lvlprest (2), lvlmaze (1), lvlsub (1), Act I placement (1), preset file choice (1) |

Loading: like `crates/d2-data/tests/game_data.rs` / `patch_game.rs`:
`ArchiveSet::open_dir(D2_GAME_DIR)` → `bin::load` → `fixup::apply` (the
fixed-up set the server builds), then `decode_all` per table. Each file has
its own copy of the 10-line loader (no shared test module). `d2-sim`'s
`clippy.toml` bans `std::fs::read`; the MPQ path needs none.

## 2. Tests, expected values and claims

Every expected value comes from a spec (stated 1.14d fact or measurement,
recorded vector, or a derived vector the spec lists); none was invented.
Intended claims follow `coverage-claims.md` §1: only where the assertions
check a rule's whole outcome. Tests of data facts in Constants / Test vectors
sections, or of a TSV against the tables, carry no claim.

### `game_world.rs`

| Test | Checks | Source | Claim (after the run) |
|---|---|---|---|
| `waypoint_map_matches_tsv` | `WaypointMap::new(levels).rows()` = `waypoints.tsv`; 39 indexes each on one level; act ranges; index 10 = 48, 11 = 42; none for a missing record / index 255; towns 1, 40, 75, 103, 109 → 0, 9, 18, 27, 30; code-13 waypoint levels {1, 40, 46, 74, 75, 103, 109}; 133–136 no index | waypoints §1, §7 r4, Constants | §1 r1, §1 r2, §1 r4, §7 r4 |
| `waypoint_objects` | OperateFn 23 + InitFn 17 classes = the 16 listed; Mode0–2 = 1; FrameCnt1 15 / 20; FrameDelta1 200; Sync | waypoints §5 r1 | §5 r1 |
| `waypoint_position_levels_use_code_13` | waypoint levels with leveldefs `Position` ≠ 0 get tile code 13 | waypoints §7 r6 (measured) | none (r6 is mostly the spawn search) |
| `quest_tables_against_live_data` | `levels` `Quest` only rows 8 → 1, 74 → 11; monstats 734, superuniques 66 rows; hcIdx values of §4.4 r4 present; quest TSVs parse (41 rows); every `quest-messages.tsv` NPC < monstats count | quests §4.6, §4.4 r4, §7.1; preset §5.3 counts | none (partial units) |
| `cubemain_live_facts` | every one of the 151 records decoded by `cube::recipes`; enabled / ladder / version / op counts and record lists; min diff, class, param, value; `numinputs` = Σ max(qty, 1); slots b, c empty; slot-a kinds, flags, 133 mods (chance 0), level modes; every op is a `cube-ops.tsv` row whose scope equals `op_info` | cube Test vectors "Live-data facts", §5, `cube-ops.tsv` | none (data facts) |
| `cubemain_vector_records` | the record side of V1, V3, V5–V11, V17–V23 (inputs, outputs, level fields, ladder, enabled, op 28, Cow portal) | cube Test vectors | none |
| `npc_records_from_live_monstats` | 47 `interact` rows → 47 records in row order; 43 `vendors.tsv` entries attach (act, trader, byte 6); 527, 537–539 unlisted with act 0, trader 0 | npc §1.1 r4–r5, §1.2 | §1.1 r4, §1.1 r5 |
| `vendor_columns_from_live_items` | `Column::build` = the column read straight from item records through the `<Vendor>Min…MagicLvl`, `spawnable`, `PermStoreItem`, `code` offsets of `fields.tsv` (and Min offset = 326 + i) for all 17 columns; Charsi `aqv`, `cqv` permanent, `axe` (1, 1, 1, 1, 1); column 11 never built | vendors §1 r1–r2, r4; game-file test of Test vectors | none (code vs `fields.tsv`) |
| `recorded_stores_fit_live_columns` | item level 6; Charsi (frame 899) and Akara (frame 1779) recorded stores fit the live columns: entries in list order, counts inside [Min, Max] / [MagicMin, MagicMax], absent entries can yield 0, permanent lists exactly `aqv cqv` / `vps … mp1`; `ktr` version ≥ 100 | vendors §3, Test vectors | none (consistency with the recording, not a replay) |
| `npc_txt_multipliers` | §9.3 table: sell / buy / rep, quest A slot (Gheed's multipliers), quest B (35: 512 / 1024 / 1024, or none), max buy (Gheed all three, others Normal); `difficultylevels` gamble odds | vendors §9.3, Constants | §9.3 |

### `game_drlg_tables.rs`

| Test | Checks | Source | Claim (after the run) |
|---|---|---|---|
| `every_level_drlg_type_dispatches` | level 0 DrlgType 0; every other level 1, 2 or 3; DrlgType 1 has an lvlmaze row; DrlgType 2 claimed by exactly one lvlprest row and the set equals the 35 levels; every warp slot ≠ −1 has an lvlwarp row (`'b'`) | levels §4 r3–r4, §7 r4; maze §1 r1; preset §3.1 r1, Constants | none (invariants from fatal paths) |
| `leveldefs_position_levels` | `Position` ≠ 0 exactly on the 18 levels listed | levels §10 r2 | none (partial) |
| `lvlprest_def_is_the_row_number` | 1,091 rows; `def(i).def == i`; `def(1091)` none | preset §2 r2 | §2 r2 |
| `lvlprest_measurements` | `Files` histogram, `Scan`, `Pops`, `PopPad`, `Animate` Defs, `AutoMap` Defs → levels, `Populate`; 82 rows naming files beyond `Files`; per DrlgType 2 level `Files`, size 0 × 0, `Scan` (level 26 = Def 165, Scan 0, Pops 0) | preset Constants, §3.2 r4, §13 | none |
| `lvlmaze_rows_as_stated` | 81 rows; rows whose level is not DrlgType 1 = levels 0, 13–16, 25, 37, 90, 91, 93, 132, the non-zero ones DrlgType 2 | maze §1 r3 | §1 r3 |
| `lvlsub_rows_as_stated` | Types 0..12 in file order, first rows 0, 1, 2, 3, 4, 6, 10, 16, 17, 21, 28, 31, 33 (also = `FixedSet::lvlsub_types`); BordType of types 0–3, 12; GridSize 2 for type 12; Blood Moor / Cold Plains sub defs; type-6 `Prob0` 30, 50, 90, 0, 50, 20; rows 12 and 14 `Max0` / `Trials0` | tilesub Constants, Test vectors | none |
| `act1_placement_on_live_tables` | `Drlg::create(0, 644409375, …)` through `WorldTypes` (real maze / preset / outdoor level types, no DS1, town id 0 so the town is not generated): no level-type error; `dwStartSeed` 4014346869; DRLG seed after {1406222081, 1674353446}; level list 16 … 4 (head first); every level seed `init_low(start + id)`, one step more for 13–16, 26, 27; level 27 lo' 2260552554; directions 0 for 13–16, 26, 1 for 27; the derived rects and town direction 3 | levels Test vectors (recorded), outdoor Test vectors (recorded seeds; rects derived), preset Test vectors (recorded) | levels §3 r2, §3 r3, §4 r1; preset §3.1 r3 |
| `preset_file_choice_derived_vectors` | levels 90, 124, 94 allocated with the same start seed: lo' values, directions 1, 3, 2, file names `…/DungRm2A.ds1`, `NihlW.ds1`, `Temple2.ds1` (case and separator folded) | preset Test vectors (derived) | none (derived) |

The rects asserted in `act1_placement_on_live_tables` are the spec's
derived values (no recording yet); a failure there alone points at
`outdoor.md` or the placer, not at the recorded part (the claims).

## 3. Local run queue (fold into `HANDOFF.md` §5 C)

On the developer PC with the 1.14d install:

```
D2_GAME_DIR=<install> cargo test -p d2-sim --test game_world -- --ignored
D2_GAME_DIR=<install> cargo test -p d2-sim --test game_drlg_tables -- --ignored
```

Expected: `game_world` 10 passed, `game_drlg_tables` 8 passed, 0 failed.
Look for, if one fails (the assertion message names the record, level or
class):

- `waypoint_map_matches_tsv`: a diff between `rows()` and
  `waypoints.tsv` means the TSV or `levels` `Waypoint` / `Act` reading is
  wrong.
- `cubemain_live_facts`: the mod count assumes an empty `mod` link reads
  −1 (`field-types.md` link32); if 133 fails while the rest passes, check
  that first.
- `every_level_drlg_type_dispatches`: a level with DrlgType ∉ {1, 2, 3}
  besides 0 is a finding for `levels.md` §4.4 (not a test bug).
- `act1_placement_on_live_tables`: a `types.errors` entry or a
  `create` error means allocation or placement reached for a DS1 or a
  `lvlsub` file (the test supplies none); the recorded list and seeds are
  the claims.
- `recorded_stores_fit_live_columns`: an "absent but yields ≥ 1" message
  means an eligible column entry is missing from the recording — check the
  item level filter (`level` ≤ 6) and the column first.

Record the result in `HANDOFF.md` §5 Done and §1 (Phase 3 world / DRLG
rows). Then add the `// Covers:` lines of §2 (above the `#[test]` of each
passing test, same spec IDs) and rerun `py tools/coverage.py --check`.

## 4. Not done (and why)

- Store generation, transmutes, quest dispatch and level generation are not
  replayed on live tables: they need the item creation path, a world fake
  and (for levels) DS1 files; the e2e tests and the `drlg-data` session
  cover those seams. The recorded stores are checked as consistency with
  the columns, not as a replay (the recording's game seed is not in a
  spec).
- No check for `lvlwarp` row names (`levels.md` §7.4 vector names the
  row): `Lvlwarp` has no decoded name field.
- Monstats names of the quest NPC classes (`quests.md` §10.2) are not
  checked: the typed `Monstats.id` is the string-table id, not the text.
