# q-fixture-inventory: moving off invented fixtures

Task `q-fixture-inventory`, branch `claude/q-fixture-inventory`. Documentation only, no code
changed, no game files read (M23, CLAUDE.md rule 1). Inventory:
`docs/handoff/fixture-inventory.tsv` (124 rows; column meanings in section 6).

## 1. What exists (size, measured by grep)

Method: `grep -c '#\[test\]'` per file, `grep` for `GameData::Synthetic`, `test_fixtures::`,
`MemorySource`, `NoStrings`, and "synthetic / made up / d2rs-own / invented" in `crates/`.
Counts of tests per fixture are approximate (`~` in the TSV): a test file is attributed to the
fixtures it imports, not traced call by call. Run the greps again before relying on a number.

| Family | Where | Size | Tests behind it |
|---|---|---|---|
| Made-up install | `crates/test-fixtures` (`synth`, `content`, `install`, `drlg`, `act1..5`, `acts`, `game`, `host`, `cube_item`) | ~5,960 lines | 65 tests in 19 files in the crate, plus 3 `d2-server` files, 2 `d2-native` files, 3 `d2-client` files, 1 bench |
| Client synthetic game | `d2-client/src/app/synthetic_*.rs` (11 files, 1,828 lines) and the synthetic half of `single_player.rs` (3,438 lines, 8 `GameData::Synthetic` blocks) | ~2,500 lines of invention | 209 of 462 client integration tests, 52 files (every class test, every `smoke_*`, travel, quests, town) |
| In-crate hand-built rows | `d2-sim` (4,641 tests, 120 files with the wording), `d2-server` (393 tests, 32 files) | thousands of small row builders | rule-level unit tests; 117 + 24 `#[ignore]` game twins already exist |
| Parser inputs | `d2-formats` robust/dcc tests, `d2-data` synthetic compile | small | keep (section 5) |
| Hand-built recordings | `conformance` replays, `fixtures/placement-players.jsonl` | small | 10 tests; real `traces/raw` counterparts exist |
| REC notes | `docs/HANDOFF.md` section 7: 54 notes name made-up data, 31 more are `d2rs-own` behaviour with no observation | 85 of 140 REC entries | none directly; they list values to re-derive |

Two facts that change the plan:

1. **The play app already has the real path.** `GameData::Live` (`LiveData::load`) builds the
   same `WaypointTables`, `LevelSource`, `GameParts` the synthetic arm builds. The synthetic path
   is a parallel implementation selected by `GameData::select(..)`, whose fallback
   `_ => Ok(GameData::Synthetic)` plays invented data **silently** when no game directory is set.
2. **There is no shared real-data gate yet.** Each crate reads `D2_GAME_DIR` by hand
   (`d2-sim/tests/game_common/mod.rs` reads `extracted/patch_d2/.../excel/*.bin`;
   `d2-server/tests/game_town_run.rs` and the client use MPQs through `ArchiveSet::open_dir`).
   `facts/` does not exist in the tree yet (`ls facts` fails); `traces/` holds `rng`, `tick`,
   eight `.scenario` files and `reference-install.toml` (names, sizes, hashes only).
   Layout of the private data repo is in its own README; I did not read it, so every path below
   that mentions `extracted/` is the one the existing tests use, to be checked against that README.

## 2. Migration plan, in order

Order = most tests and the client play path first; each wave leaves CI green without data.

### Wave 0: the gate (prerequisite, small)

- One helper, used by every crate (suggest `test-fixtures` keeps only this, or a new
  `crates/test-data`): `game_dir() -> Option<PathBuf>` from `$D2_GAME_DIR`, plus
  `require_game()` that **fails** when `D2_REQUIRE_REAL=1` and the directory is missing, and
  skips (with a printed line) otherwise. Replace the four hand-rolled readers.
- The gate summary prints `real-data tests run: N of M` (M23 Check). Today every game test is
  `#[ignore]`, so the gate cannot say how much ran on real data.
- Decide how cloud sessions fetch the private repo into `D2_GAME_DIR` (setup script), and whether
  CI ever gets data (CLAUDE.md: CI keeps only what truly needs none). Needs the owner's call; not
  assumed here.

### Wave 1: the client play path and the largest fixture (most tests)

1. Make `GameData::Live` the only play data: remove the `Synthetic` variant, the `--synthetic`
   flag, and the silent fallback (`single_player.rs:1636`, `main.rs:74,146,408,488`,
   `play.rs:336-386`); no game dir is an error naming `D2_GAME_DIR`. (Rows CS01, CS11, CS13.)
2. Replace the invented vitals/monster/hire/skill tables (`synthetic_charstats`,
   `synthetic_vitals`, `synthetic_monstats`, `synthetic_hire_rows`, `synthetic_unit_rows`,
   `synthetic_skill_rows`) by `LiveData` rows. (CS02, CS11.)
3. The 209 `GameData::Synthetic` tests become real-data tests: they build through one rig
   (`app_support`) that calls the gate and then `GameData::select(Some(dir), false)`.
4. In `test-fixtures`: TF01/TF02 (made-up set + install) are replaced by
   `ArchiveSet::open_dir($D2_GAME_DIR)` in the 3 `d2-server` and 2 `d2-native` files, and
   in the 8 `synthetic_load` / 4 `server_tables` tests (most already have a live twin:
   `game_wired_host`, `game_town_run`, `world_data_tables`).
5. CI keeps: parser/writer round trips (TF04), MPQ writer tests, pure-rule units. CI drops: every
   test whose only subject is "the made-up world loads".

### Wave 2: DRLG and the act variants

- `act1..5.rs`, `acts.rs`, `drlg.rs` (TF03, TF05-TF10, ~1,700 lines) exist so `Drlg::create`
  can run on 8-150 invented level rows. On the real `levels` / `lvlprest` / `lvltypes` /
  `lvlsub` / `lvlmaze` tables they are unnecessary: run `ActCreation::Full` for acts 0-4.
  Their asserted offsets/sizes are already spec test vectors from recordings
  (`outdoor.md`), keep those as facts.
- `host.rs` session (TF12) and `game.rs` (TF11): keep the builders (they work on the live set);
  delete `ActCreation::TownOnly`; `Session::new` uses the real waypoint object found in the real
  town preset.
- Item/vendor tables (CS10, TF13): real `armor/weapons/misc/itemtypes/inventory/npc.bin`.

### Wave 3: the invented world (positions, chains, quests)

CS03-CS09, CS12 invent where things stand. The real source is the level stream (preset DS1
units, `lvlwarp`, `levels` Vis0-7, `lvlmaze`) which `GameData::Live` already feeds. After the
switch, the coordinates that tests assert (Akara x, Blood Moor at tile (24,0), warp tile slots,
Den-to-Cave stairs, the Moldy Tome in the Black Marsh) are replaced by values **measured from
the real DS1s by a tool** and committed under `facts/` with format version and command; never
typed in from memory. Quest scripts that relied on host-placed monsters (Blood Raven, Izual,
Duriel) read the real monster presets.

### Wave 4: in-crate unit fixtures (no rush)

`d2-sim` and `d2-server` unit tests with a few hand-built rows are rule arithmetic and stay (M23:
invented fixtures fill what real data cannot reach). Add, per seam module, one real-data
twin (wiring/*, treasure, drlg, vendors) where none exists; the module list is in TSV rows SM01,
SM02 and `docs/COVERAGE.md`. `d2-native` (TFU02) moves to a small real subset.

### Wave 5: traces, renders and `d2rs-own` behaviour

CS14 (render verify) and CF01 (hand-built recordings) are replaced by captured frames and
`traces/raw`. The 31 `d2rs-own` notes (N-REC rows, e.g. REC-237 Esc menu art, REC-267 cube
animation) describe behaviour nobody observed: they join the recording queue (HANDOFF section 5),
they are not fixture work and stay "unverified".

## 3. How each test changes

| Class | Today | After | Needs new expectations? |
|---|---|---|---|
| A. Rule arithmetic on hand-built rows (most of `d2-sim`, `d2-server` units) | CI | unchanged; add real twin per seam | no |
| B. Seam and flow tests on the invented world (209 client tests, 65 fixture tests) | CI, synthetic | `#[ignore]`-style real-data tests that read `$D2_GAME_DIR` through the gate; skipped (printed) where no data | **yes**, for every number from the invented world |
| C. Tests asserting invented table values (counts, ids, 81 floor records, 8 level rows, class 3, Akara x, skill 0 only) | CI | rewritten against facts | yes: facts or traces |
| D. Parser/writer round trips and robustness inputs | CI | unchanged | no |
| E. Hand-built recordings | CI | real `traces/raw` / `traces/sim` | yes (replay against the real recording) |

Expectations that must come from real data, produced by a tool, never guessed:

- creation stats of each class, experience thresholds, walk speeds (charstats/experience bins);
- positions of NPCs, waypoints, chests, stash, warp tiles, quest objects in each town and level
  (DS1 units; `lvlwarp.bin`);
- level counts/rects/offsets after `ActCreation::Full` (partly already spec vectors);
- town waypoint room, floor record counts, room stream sizes;
- vendor stock and prices (trace of `vendor-buy-sell.scenario`);
- item base stats for the Hellforge/cube items, drop results for fixed seeds (treasure dump
  compare exists: `game_treasure_dump.rs`);
- every frame-timing assertion in the smoke tests: tick numbers from `traces/sim/tick`.

## 4. Code that exists only because the fixture lacked real data

Marked here (no code edit in this task) with what replaces it:

| Path | Why it exists | Replaced by |
|---|---|---|
| `ActCreation::TownOnly` (`test-fixtures/src/game.rs:75,294`; 7 uses) | the 8-row made-up `levels` lacks ids 17/26/39, so the Act I placer fails with `UnknownLevel(17)`; the seed then skips the placer's draws, a state 1.14d never has (FG1) | `ActCreation::Full` on real tables; variant deleted |
| `GameData::Synthetic` and `select` fallback (`single_player.rs:1615,1636`) | tests/play without data | `GameData::Live` only; missing dir is an error |
| 8 `matches!(data, GameData::Synthetic)` blocks (`single_player.rs:2656,2762,2867,2885,2921,2957,2993,3036`) | place stash, chest, NPCs, waypoints by hand because the flat rooms have no town preset | preset unit stream of the real town DS1 (Live path) |
| `SYNTHETIC_PRESET_FLAGS` 0x3000000 (`:640,:3036`, REC-287) | imitates the unit flags `0x005557D0` gives preset units, so the inactive store keeps them | flags set by the real preset creation path |
| `synthetic_level_warps`, `synthetic_chains` slots/tile_xy, `SyntheticTypes` | flat one-room levels joined by invented warp pairs | `levels` Vis0-7 + `lvlwarp.bin` + DS1 warp units; the REC-261 "first room carries the exit" hack disappears |
| `warp_quest_gate` branches for `synthetic_act2::DURIELS_LAIR` / `synthetic_act5::SUMMIT` (`:800-810`) | the invented chain has both exits | real exits of those levels |
| Host-placed monsters: Blood Raven (`:1092`), Izual (`:1095`), `monster_quest_chain` | no monster presets in flat rooms | the real monster preset spawn |
| `client_*` None/`Vec::new()` arms in `play.rs:336-386`, `single_player.rs:1984` | the synthetic game has no archives, objects rows, hire rows, save tables, strings | always `Some(..)` from `LiveData` |
| `NoStrings` in the play app | no string tables loaded in play (queue item `q-strings`); unit tests keep it | `string.tbl` + expansion + patch tables (loading order in `formats/loading.md`) |
| `synthetic_skill_rows` (`tests/app_support/mod.rs`) | the invented skills table has only row 0 | the real `skills.bin` rows |
| `Seams` in `test-fixtures/src/game.rs` | answers transport bookkeeping only | the real server seams, as `game_wired_host.rs` assembles them |
| `ACT2_NPCS` / `town_npcs::ACT5` dx offsets | invented x offsets inside a flat room | preset NPC positions |

Not fixture-only (keep): `NoLevelTypes` (`d2-sim/src/drlg/seams.rs:121`), a unit-test seam.

## 5. Keep list (truly needs no data)

Format writers (`tbl`, `animdata`, `ds1`, `dt1`, MPQ writer), round-trip tests, parser robustness
inputs (`mpq/robust_tests.rs`, fuzz regressions, `dcc` synthetic frame), property tests, rule
arithmetic units. A fixture stays only if it is built from the real data's measured shape
(M23) or covers a branch no real file reaches; its doc comment says which.

## 6. TSV columns

`id` (TF = test-fixtures, TFT = its tests, TFU = other crates' users of it, CS = client synthetic,
SM/DF/CF = other crates, N-REC-n = REC note), `layer`, `file`, `kind`, `what_it_invents`,
`used_by`, `tests` (approximate), `real_source`, `replacement` (real-data / facts / trace / keep),
`wave` (0-5, `-` = keep), `fixture_only_path`. REC rows carry the title from `docs/HANDOFF.md`;
"note-fixture" = the note says made-up/synthetic data, "note-own-design" = `d2rs-own` behaviour.

## 7. Open points for the owner

1. Do cloud runs always have `D2_GAME_DIR`, and should CI (public) never? This sets whether
   class B tests are `#[ignore]` or skip-with-message.
2. Where does `facts/` get created, and by which tool (`data-tool facts ...`)? It does not exist.
3. The private repo layout was not read here; the `extracted/patch_d2/...` path is what the
   current tests use.
