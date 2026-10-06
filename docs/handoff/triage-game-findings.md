# Triage of the 1.14d game-file failures (2026-10-06)

Branch `claude/triage-game-findings`, from `b1cc0b6` (main + local2 /
local-buddy merges). Input: the identical failures from two PCs
(`local-buddy-2026-10-06.md`, `local-buddy-a-sim.md`,
`local-buddy-a-server.md`, HANDOFF §5 Done 2026-10-06). This is a cloud
session with no game files: **no `#[ignore]` test was run here**. Every
fix below is backed by a spec rule or by a value recorded on the live
install. No expected value was changed just to match, and no test was
skipped or loosened. Where the spec is silent or doubtful, the expected
value stays and the test now prints what the next local run needs.

Classes: **a** = code bug (fixed, with a synthetic regression); **b** = test
bug (corrected from a spec rule or a recorded live value); **c** = spec gap
or spec wrong (a question for the spec writer, below).

## Counts

| Class | Tests |
|---|---|
| a (code fixed) | 2: `act1_placement_on_live_tables` (DRLG neighbour entries), the `mpq-tool formats` name set behind the `game_sweep` counts |
| b (test fixed) | 12 (wired host counted once): `real_grid_belt_and_type_tables`, `sweep_drop_quality_every_item`, `waypoint_objects`, `cubemain_vector_records`, `vendor_columns_from_live_items`, `lvlprest_measurements`, `every_lvlprest_ds1_parses`, `game_wired_host` (7 cases, one cause), `type_init_every_class`, `dc6_every_file_decodes`, `dt1_every_live_file_decodes` (file count), `cpu_compositor_on_real_frames` |
| b, value still unknown | 2: `ds1_every_file_parses`, `string_tables_every_key_resolves` (part c) |
| c (spec question) | 4: `ai_index_of_every_row`, `real_levels_rows`, `cof_every_live_file_parses`, `sweep_create_every_item_every_quality` (mixed b + c) |
| a, location unknown | 1: `outdoor_levels_generate_through_the_dispatcher` (diagnostic added) |
| unresolved, outside the list | `d2-client play` panic at `drlg/room.rs:144` (needs a backtrace) |

## Results per test

| Test | Class | Evidence | Fix or question | Local re-run |
|---|---|---|---|---|
| d2-sim `items::inventory::tests::real_grid_belt_and_type_tables` (`tests.rs:1540`) | b | The test opened a fixed path, `game/extracted/patch_d2/.../inventory.bin`. `specs/data/tables.tsv:88-89` gives `inventory` and `belts` as `d2exp`, and `loading.md` §2 gives the lookup order. `d2_data::bin::excel_lookup_order` gives the same. | `game_table` now reads through `ArchiveSet::open_dir(D2_GAME_DIR)` + `d2_data::bin::read_excel` (the loader the passing tests use). Expected values unchanged. | `cargo test -p d2-sim --lib real_grid_belt_and_type_tables -- --ignored` |
| `game_treasure::sweep_drop_quality_every_item` (:454, item 520, M −100) | b | `treasure.md` §6 step 5: M ≤ −100 skips to step 7, past the itemtypes `magic` gate (ladder step 4, `treasure-quality.tsv`). The test asserted "magic ⇒ q ≥ 4" and "M ≤ −100 ⇒ q ≤ 3" together, which a `magic` type can never satisfy. The code (`treasure/quality.rs:266`) follows the spec. | The magic-gate assertion applies only when `mf > -100`; the spec is cited in the test. See Q3. | `cargo test -p d2-sim --release --test game_treasure -- --ignored sweep_drop_quality_every_item` |
| `game_items::sweep_create_every_item_every_quality` (:391, 570 failures, "crafted affix 0 with a filled slot (read at 0x5C)") | b + c | The error is `Fatal::NullAffixGroup` (`items/affixes.rs:365`). It is the crash that `affixes.md` §8 step 3.2 / Edge case 3 states, and the unit test `crafted_null_group` covers it. So the sweep's "every creation succeeds" contradicts the spec. But the spec is doubtful (Q1, Q2): the code writes each pick into the item, so the §3 group test already excludes taken groups, and the crash is reachable on almost every low-ilvl craft. | The test accepts exactly `Fatal(NullAffixGroup)` for quality 8 (crafted). It counts those per (expansion, difficulty, ilvl) and prints them; every other error still fails. Only 5 of the 570 lines were recorded (all of this kind). | `cargo test -p d2-sim --release --test game_items -- --ignored sweep_create_every_item_every_quality --nocapture`; read the "crafted requests ending in affixes.md Edge case 3" line |
| `game_world::waypoint_objects` (:146, FrameCnt1 3840 vs 15) | b | `fixups.md` §13 r2: `FrameCnt0`–`7` := value << 8 in the fixed-up set. `waypoints.md` Inputs (line 75) says "already × 256", and §5.1 r2 reads `>> 8`. 3840 = 15 << 8. | Now asserts `framecnt1 == frames << 8` (frames 15, or 20 for 494/496/511/539). | `cargo test -p d2-sim --test game_world -- --ignored` (expect 10 pass) |
| `game_world::cubemain_vector_records` (:345, 129 vs 130) | b | Record 64 input `fhl,mag,upg`: `fhl` is an item code, not an item type. `callbacks.md` §3 input step 4.3 gives USEANY 0x01, plus `upg` 0x80, so 0x81 = 129 (live). | Asserts `USEANY \| UPG` and `item == item_index("fhl ")`. | same |
| `game_world::vendor_columns_from_live_items` (:427, `weapons.HratliMin`) | b | `fields.tsv:1036-1040` (and armor / misc) spells the columns `HraltiMin/Max/MagicMin/MagicMax/MagicLvl` (Blizzard's typo, offset 332). The test built the name from the NPC name. `world/vendors.rs` uses offsets and is not affected. | `raw_column` maps Hratli to `Hralti`. | same |
| `game_drlg_tables::lvlprest_measurements` (:229, 1,079 vs 82) | b | `fixups.md` §12: strings of 0 or 1 characters are `0` placeholders, not file names (`world_data::names_file` uses `len > 1` too). 1,079 = every row with a spare slot. `preset.md` §13: 82. | Counts `len > 1`; the 82 stays. | `cargo test -p d2-sim --test game_drlg_tables -- --ignored` |
| `game_drlg_tables::act1_placement_on_live_tables` (:329) | **a** | `levels.md` Test vectors (seq 2425–2452, recorded) allocate 8, 9, …, 16 after A1M row 5 and before the town. The only step there is the neighbour entries over 1..17 (`outdoor.md` §2.7), and `levels.md` §4.2 says every lookup by id allocates. `Outdoor::neighbours` skipped unallocated ids (a TODO). | **Fixed** `drlg/outdoor/place.rs`: `neighbours` get-or-allocates each id before the type test. Synthetic regressions now require the full 20-level list: `outdoor/tests.rs` `act1_placement_matches_recording`, `gaps_tests.rs` `act_wide_placement_per_act`, `wiring/worldgen/tests/outdoor.rs`; `mutant_tests.rs` filters types 1 and 2 (`levels.md` §4.4). The d2-server tests `act1_placement_matches_the_recorded_vector` and `every_act1_level_generates` asserted the placer-only prefix and now assert the full list. See Q4. | same; plus `cargo test -p d2-server act1_placement -- --ignored` and `cargo test -p d2-server --test game_world_data -- --ignored` |
| d2-server `world_data::tests::game::outdoor_levels_generate_through_the_dispatcher` (`game.rs:179`, Cold Plains 97 vs 98) | a (location unknown) | 98 is recorded (`outdoor.md` Test vectors, `20261005-232125-rng.jsonl`): 61 preset cells (`0x00666F33`) + 37 outdoor rooms (222 room-seed draws = 37 × 6). The test checks only the total. | `eprintln!` of total / preset / outdoor before the assert; expected value unchanged. The Act I allocation fix may move this; not proven. See Q6. | `cargo test -p d2-server outdoor_levels_generate -- --ignored --nocapture`; read `Cold Plains rooms: … preset, … outdoor (recorded 98 = 61 + 37)` |
| d2-server `game_world_data::every_lvlprest_ds1_parses` (:167, object ids ≥ 573 empty) | b | `preset.md` §5.3: for v ≥ 6, a stored object id < 150 becomes `objpreset[act][id]`. 580–582 are class ids after that remap (`preset-tables.tsv`), and §5 keeps them as class ids. The test counted the raw stored id (always < 150). | Counts unit type 2 with class ≥ 573 on the parser output (`Ds1File::from_input(f, &preset)`). The expected map is unchanged. | `cargo test -p d2-server --test game_world_data -- --ignored every_lvlprest_ds1_parses --nocapture` |
| d2-server `game_wired_host` all 7 (`:301`, "no town preset object with operate function 23") | b | `town_waypoint` searched room unit lists right after `Drlg::create`. `preset.md` §9 (`0x00666710`): units move from the map list to a room only when that room's tiles are built. §7: kept units sit on the map list in level sub-tiles. No town room was streamed yet. | `town_waypoint` walks `presets.level_maps(town)` → `map.units` and picks the room whose sub-tile rect holds the unit (§9 shift). A new `assert_transferred` checks, after `stream_room`, that the unit is on that room's list. | `cargo test -p d2-server --test game_wired_host -- --ignored --nocapture` (compare the 7 `digest` lines over two runs) |
| `game_monsters::type_init_every_class` (:508, 115456 vs 57728) | b | `monsters/init.md` §13: in a classic game with d > 0 and `Align` ≠ 1, maxhp alone is pct(·, 1, 2); "Hitpoints (stat 6) are not scaled (original quirk)". Edge cases 11 says the same. Hirelings have d = 0 (§6 step 4). The test expected hitpoints == maxhp always. | Expects MAXHP = pct(HITPOINTS, 1, 2) in that case, else equal. | `cargo test --release -p d2-sim --test game_monsters -- --ignored --nocapture` |
| `game_monsters::ai_index_of_every_row` (:948, row 528 drehyaiced 129 vs 31) | c | `monsters/ai-functions.tsv` `monstats_rows` disagrees with the other specs by +1 from some row in 144..453: drehyaiced TSV 528 / `ai.md` l.1375 527; minion1 454 / `population.md` l.668 453; deathmauler1 530 / 529; ancientstatue1-3 538-540 / `npc.md` l.100 537-539; evilhut TSV 529 / `population.md` 528. Live row 528 has AI 129 (GenericSpawner = evilhut). Per-index counts pass. | No expected value changed. The test now prints every bad count and row pair before failing. See Q7. | same; read the `row N name: live AI x, tsv AI y` lines |
| `game_monsters::real_levels_rows` (:1037, level 15 3800 vs 2025) | c | `population.md` §8 l.468 and Real table l.1031: "Act 1 WarpDist 2025", with no rows named. Live: levels 1–14 are 2025, level 15 (Hole Level 2) is 3800. Same reader and field for every row (`fields.tsv:2760`). | Expected value kept. The test prints every Act 0 row ≠ 2025, then fails. See Q8. | same; read `act 0 levels with WarpDist != 2025: [...]` |
| `mpq-tool formats` (source of the `game_sweep` counts) | **a** | `formats/mpq.md` §3 `normalize`: names differing only in case or `/` vs `\` are one file. `tools/mpq-tool/src/formats.rs` kept a case-sensitive `BTreeSet`, so it counted such files twice (buddy note G1, spec session 6b8dc11). | **Fixed:** `name_key` / `name_set`; test `formats::tests::name_set_is_case_and_separator_insensitive`. | `cargo run --release -p mpq-tool -- formats "$D2_GAME_DIR"`; record the dc6, dt1 (with block formats), ds1 (with v18) and tbl rows |
| `game_sweep::dc6_every_file_decodes` (1,653 vs 1,657) | b | Live 1,653 / 26,317 frames / termination EE 0x4A9, CD 0x190, 00 0x3C on two PCs; the spec session's case-insensitive count is 1,653. | Expects 1,653, 26,317, EE 1,193, CD 400, 00 60. `dc6.md` Status still has the old numbers (Q11). | `cargo test --release -p d2-formats --test game_sweep -- --ignored --nocapture` |
| `game_sweep::dt1_every_live_file_decodes` (250 vs 254) | b (+ unknown) | The spec session measured 256 files − 6 version-4 = 250 live, which equals the sweep on both PCs. The block-format counts (226,996 / 110,259 / 15,712) also came from the double-counting tool; the live line was cut off in the note. | Live count 250. Block counts not changed: the test will probably fail next on them. Record the whole `dt1:` line. | same |
| `game_sweep::ds1_every_file_parses` (2,372 vs 2,456) | b, value unknown | 2,456 came from the double-counting tool. For `.ds1` the sweep and the fixed tool use the same names, so 2,372 (1,926 at v18) is expected, but nothing independent confirms it yet. | Not changed (comment says why). Change it after the fixed `mpq-tool formats` prints 2,372 / 1,926. | `mpq-tool formats` as above |
| `game_sweep::string_tables_every_key_resolves` (29 vs 33, 10 vs 11 languages) | b + c | 33 came from the double-counting tool, and the sweep also left out the tool's `EXTRA_NAMES` (3 eng `.tbl`). No source explains an 11th language. | The sweep adds `EXTRA_NAMES` and maps `/` to `\`; 33 / 11 kept. See Q10. | same |
| `game_sweep::cof_every_live_file_parses` (`amblxbw.cof` not found) | c | The name is from `formats/cof.md:83-86` ("the real file is `amblxbw.cof`"); no 1.14d archive holds it. Everything else in the test passed (3,605 parse, the only bad file is `amblxbow.cof`, 72 bytes). | Assertion kept. The test prints every listed `...\am\cof\ambl*` name and the archives that hold it. See Q9. | same, filter `cof_`, copy the `ambl*` lines |
| d2-client `game_assets::cpu_compositor_on_real_frames` (:652, 7,683 bytes differ) | b | The test's own reference `evaluate` read the blend table as row = source. `render-pipeline.md` §A5 `IndexTable`, `composition.md` §5 (`T[256 × d + P[s]]`) and `blend-modes.md` §2 all give row = destination, which is what `scene` `BlendOp::apply` does. The GPU test agrees with `compose`. | `evaluate` uses `map[base + dest][i']`; compositor code unchanged. | `cargo test --release -p d2-client --test game_assets -- --ignored --nocapture --test-threads 1 cpu_compositor_on_real_frames` |
| `d2-client play --frames 1500` panic `drlg/room.rs:144` "live DRLG room" (~104 ticks) | unresolved | A freed `DrlgRoomId` was looked up. Rooms are freed via `free_inactive_levels` (`levels.md` §9.2–9.4) after the 10-frame countdown. Suspect: `near` arrays or links of rooms in an outdoor-adjacent (non-warp) level that §9.3 does not clear. Unconfirmed; the Act I allocation fix may also change it. | No change. See Q5. | `RUST_BACKTRACE=1 cargo run -p d2-client --release -- play --frames 1500`; record the frames above `room.rs:144`, the freed room's level id and the caller |

## Questions for PC 1 spec writer

1. **`items/affixes.md` §8, crafted `0x005C21D0`: where do the picks go
   during the loop?** Does each accepted affix go into the item's
   prefix/suffix slots right away, where the next §3 call's group test
   (`0x005C1500`, §4.2) sees it? Or does it go into local arrays that are
   copied to the item after the loop? d2rs writes to the item
   (`affixes.rs:331-395`), so the 252-try "taken" check is redundant, and
   Edge case 3 (the crash) fires on most low-ilvl crafts. Live: about 570
   crafted requests in the creation sweep end in that crash, all of the
   5 recorded ones at ilvl 1 (dgr, sst, leg, ob1).
2. **`affixes.md` Edge case 3: is the crash real?** After the roller
   returns 0 in `0x005C21D0`, is there a `test eax,eax` (or similar) before
   the group lookup via `0x00633EE0`? If there is a guard, give the exact
   control flow for a = 0 with a same-kind slot filled. A live check would
   settle it: craft at a low resulting ilvl.
3. (Low priority) **`items/treasure.md` §6 step 5 (`0x00558640`):** does
   M ≤ −100 really jump past the itemtypes `magic` gate (ladder step 4)? If
   so, an amulet or ring can come out as quality 2 or 3. Does `quality.md`
   §4 force it up at creation?
4. **`drlg/outdoor.md` §2.7, neighbour entries `0x00677680`:** does the
   loop over a..b call get-or-allocate (`0x00642BB0`) on every id before
   the `DrlgType == 3` test? The recorded Act I list requires it for 8..16,
   and the code now does it (please confirm and reword "for each outdoor
   level in a..b"). Is the neighbour id (vis[j]) also looked up with
   get-or-allocate? The code still uses find without allocation there.
   And: adjacency warps `0x006775C0` (Act III 75..83, Act V 109..112), do
   they allocate unallocated ids? The code skips them (TODO). Also add a
   cross-reference in `outdoor.md` Test vectors: its allocation list ends at
   5, while `levels.md` continues with 8..16.
5. **`drlg/levels.md` §9.3–§9.4 (free test `0x00643060`, free
   `0x00642010`):** when a level's rooms are freed, does 1.14d clear the
   `near` arrays of rooms in outdoor-adjacent levels (vis slot, warp −1)?
   Or does the free test refuse while such a neighbour room is active?
   This bears on the `play` panic at `drlg/room.rs:144`.
6. **`drlg/outdoor.md` §7 / §12.1, Cold Plains:** live build 97 rooms,
   recorded 98 (61 preset cells + 37 outdoor). After the next local run
   prints which side is short: if preset, which `0x00666F33` cell is
   missing (the recording has the cell order from seq 12010 on)? If
   outdoor, which cell has 0x100 without 0x200? A per-cell (x, y, grid 0,
   grid 2) dump at `0x006750F0` from the recording would settle it.
7. **`monsters/ai-functions.tsv`, column `monstats_rows`:** the row
   numbers look shifted by +1 starting somewhere in rows 144..453. Live
   1.14d `monstats.bin` row 528 has `AI` 129 (GenericSpawner); the TSV
   pairs 129 with "529 evilhut", while `population.md` (l.434/440/1038)
   says evilhut = 528. The other conflicts are drehyaiced (TSV 528,
   `ai.md` l.1375 527), minion1 (454, `population.md` l.668 453),
   deathmauler1 (530, 529) and ancientstatue1-3 (538-540, `npc.md` l.100
   537-539). Please renumber every pair against `monstats.txt` row order
   and say where the shift starts. The next run of `ai_index_of_every_row`
   prints every mismatching pair. This also closes the evilhut 528/529
   question.
8. **`monsters/population.md` §8 (l.464-468) and Real table l.1031 ("Act 1
   WarpDist 2025"):** live `levels.bin` has 2025 for levels 1–14 and 3800
   for level 15 (Hole Level 2, Act 0). Which rows does the statement cover?
   Please give the Act 0 `WarpDist` column, or the rows that are 2025 and
   the exceptions. Also confirm that §8 uses the room's own level's
   `WarpDist` (+0x0C).
9. **`formats/cof.md` Edge cases (l.83-86):** no 1.14d archive holds
   `data\global\chars\am\cof\amblxbw.cof` (two PCs). Which is the real
   Amazon block/bow COF (by pattern perhaps `amblbow.cof`; unverified)?
   Is `amblxbow.cof` the only `ambl*x*` leftover? The next sweep prints
   every `ambl*` name and its archives.
10. **`formats/tbl.md` Status (33 tables, 11 languages):** both came from
    the case-sensitive `mpq-tool formats`. The live sweep finds 29 tables
    in 10 `data\local\lng\<lang>` folders (chi, deu, eng, esp, fra, ita,
    jpn, kor, pol, por), without the tool's 3 `EXTRA_NAMES`. What is the
    11th language and where is it? Re-derive the count with the fixed tool.
11. **Recount with the fixed `mpq-tool formats`** and update the Status
    lines: `dc6.md` (says 1,657 / 29,117 frames / EE 1,195 / 00 62; live
    1,653 / 26,317 / 1,193 / 60), `dt1.md` (254 live and the three
    block-format counts; live 250), `ds1.md` (2,456 / 1,997 at v18; live
    2,372 / 1,926). The `game_sweep` scope now equals the tool's (listfile
    union + `EXTRA_NAMES`), which answers GA3.
12. (Not a failure; noticed while reading) **`render/composition.md` §5
    l.182:** with both `T` and `L`, `P` is not applied, but `scene/item.rs`
    applies the whole chain (TODO pointing to OQ2). Is OQ2 closed, so that
    the compositor must drop `P` there?
13. (Clarity only, no Ghidra) `world/vendors.md` §1 r1: say that vendor
    index 6's item-table columns are spelled `Hralti…`. `world/waypoints.md`
    §5 r1: say that 15 / 20 are `objects.txt` values and the loaded record
    holds them × 256 (`fixups.md` §13).

## Changed files

- Code: `crates/d2-sim/src/drlg/outdoor/place.rs` (neighbour entries
  allocate); `tools/mpq-tool/src/formats.rs` (case-insensitive name set,
  plus test).
- Synthetic tests updated for the allocation fix:
  `drlg/outdoor/tests.rs`, `gaps_tests.rs`, `mutant_tests.rs`,
  `wiring/worldgen/tests/outdoor.rs`.
- Game-file tests: `d2-sim` `items/inventory/tests.rs`, `tests/game_items.rs`,
  `game_treasure.rs`, `game_world.rs`, `game_drlg_tables.rs`,
  `game_monsters.rs`; `d2-server` `src/world_data/tests/game.rs`,
  `tests/game_world_data.rs`, `tests/game_wired_host.rs`; `d2-formats`
  `tests/game_sweep.rs`; `d2-client` `tests/game_assets.rs`.
- No `// Covers:` line added: none of the corrected game tests has passed on
  the live data yet. After a passing local run, add the intended claims
  from the `game-tests-*` notes. Suggested CI test (not added): treasure,
  a `magic` itemtype at M = −100 returns quality ≤ 3 (`treasure.md` §6 r5).

## Local re-run (one batch)

```sh
cargo run --release -p mpq-tool -- formats "$D2_GAME_DIR"
cargo test -p d2-sim -- --ignored
cargo test --release -p d2-sim --test game_world --test game_drlg_tables --test game_treasure --test game_items --test game_monsters -- --ignored --nocapture
cargo test --release -p d2-server -- --ignored --nocapture
cargo test --release -p d2-formats --test game_sweep -- --ignored --nocapture
cargo test --release -p d2-client --test game_assets -- --ignored --nocapture --test-threads 1
RUST_BACKTRACE=1 cargo run -p d2-client --release -- play --frames 1500
```

Expected: everything in the b/a rows passes. The c rows, Cold Plains and
`ds1` / `string_tables` / the DT1 block counts stay red and print the
values the questions above need.
