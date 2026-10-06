# Local buddy run 2026-10-06 (LOCAL-RUN batches 1–5)

Branch `claude/local-buddy-2026-10-06` from main `0472619`. Machine:
Windows 11 Pro build 26200, AMD Radeon RX 9070 XT (Vulkan, DiscreteGpu,
driver "AMD proprietary driver", driver_info "26.8.1 (LLPC)"), 64 GB.
`D2_GAME_DIR` = the 1.14d install. Every command as written in
`docs/LOCAL-RUN.md`; no expected value or test was changed. Raw logs stay
on the PC (scratch). Batch 6 not run (no player).

## 0.1 Install check

`cargo run -p hash-manifest -- $D2_GAME_DIR my-install.toml`, then the diff
against `traces/reference-install.toml`: **identical** (19 entries, every
`.mpq`, `.exe`, `.dll`; `cmp` reports no difference). Results below are
comparable.

## Batch 1: tables and formats

| # | Result |
|---|---|
| 1.1 `data-tool tables` | PASS (14 s) |
| 1.2 `data-tool links` | PASS |
| 1.3 `mpq-tool check` | PASS |
| 1.4 `mpq-tool formats` | PASS (61 s) |
| 1.5 `cargo test -p d2-data -p d2-formats -- --ignored` | **FAIL: 53 pass, 5 fail**, all five in `game_sweep` (same as 2.11) |
| 1.6 `patch_game` | PASS |
| 1.7 `patch check …overhaul.d2stack` | PASS: one N01 note, digest `66010ecda7c8df5b7135579888c536fd2a30287fb877848719a31b6c8f97a625` |
| 1.8 `fixups_on_live_set` (C32) | PASS |
| 1.9 `dump-compare` (C33) | PASS on a fresh dump `traces/raw/20261006-201456-tables` (the `-021210` dump is not on this PC): `70 identical, 0 differ only in PENDING rows, 0 differ elsewhere; compared maps: identical` |
| 1.10 `check_stats.py --files` | PASS (359 stats, 84 ops, 42 op targets) |
| 1.11 `path_tables.py` | PASS (exit 0) |
| 1.12 C21 `ds1_layer_limits_and_truncated_trees_groups` | PASS |
| 1.13 C22 | not run (no inspect command given) |

## Batch 2: game-file tests

| # | Result |
|---|---|
| 2.1 C1 `cargo test -p d2-sim -- --ignored` | **FAIL**: 6 pass, 1 fail (`real_grid_belt_and_type_tables`, = 2.2) |
| 2.2 C36 `real_grid_belt_and_type_tables` | **FAIL** (`items/inventory/tests.rs:1540`) |
| 2.3 C38 `expfield_live` | PASS |
| 2.4 C34 `game_world` | **FAIL**: `cubemain_vector_records`, `waypoint_objects`, `vendor_columns_from_live_items` |
| 2.4 C34 `game_drlg_tables` | **FAIL**: 6 pass, 2 fail (`lvlprest_measurements`, `act1_placement_on_live_tables`) |
| 2.5 C35 `game_treasure` | **FAIL**: 15 pass, 1 fail (`sweep_drop_quality_every_item`) |
| 2.5 C35 `game_items` | **FAIL**: 7 pass, 1 fail (`sweep_create_every_item_every_quality`) |
| 2.6 C37 `game_core` | PASS (12) |
| 2.7 C37 `game_world_data` | **FAIL**: 2 pass, 1 fail (`every_lvlprest_ds1_parses`) |
| 2.8 C23 `world_data` | **FAIL**: 2 pass, 1 fail (`outdoor_levels_generate_through_the_dispatcher`) |
| 2.9 C51 `game_inventory_path` | PASS (6). Printed: 659 items, 8 distinct sizes (1×1, 1×2, 1×3, 1×4, 2×1, 2×2, 2×3, 2×4), 0 with a zero size, none larger than 10×4; grids 3×4, 6×4, 6×8, 10×4, 10×10; beltable items 27 (27 of them 1×1) |
| 2.10 C45 `game_monsters` | **FAIL**: `real_levels_rows`, `ai_index_of_every_row`, `type_init_every_class` |
| 2.10 C45 `game_skills` | PASS |
| 2.11 C46.1 `game_sweep` | **FAIL**: 6 pass, 5 fail (see note G1) |
| 2.12 C46.2 `game_assets` | **FAIL**: 4 pass, 1 fail (`cpu_compositor_on_real_frames`) |
| 2.13 `game_wired_host` (twice) | **FAIL** both runs, all 7 classes at `game_wired_host.rs:301`; no `digest` line printed, so the two runs cannot be compared |
| 2.14 `composite` all live COFs | PASS |
| 2.15 `frames` all live frame sets | PASS (685 s, debug) |

**G1 (`game_sweep` counts).** Same five failures as the main-PC run of
63a706b (HANDOFF §5 Done): DC6 1,653 vs 1,657, DT1 250 vs 254, DS1 2,372 vs
2,456, string tables 29 vs 33, `amblxbw.cof` not found. Spec session
`claude/spec-answers-render` (6b8dc11) measured that `mpq-tool formats`
counts a file twice when two listfiles spell its name in different case
(its name set is case-sensitive): real DC6 1,653 and DT1 256. So the
expected values, taken from `mpq-tool formats`, are inflated, and the
earlier explanation ("`patch_d2.mpq` has no listfile") is incomplete. Fix
`mpq-tool`'s name set (case-insensitive) first, then derive the expected
counts again; not changed here.

## Batch 3: real GPU

Every command reports `adapter: AMD Radeon RX 9070 XT (Vulkan, DiscreteGpu,
driver AMD proprietary driver)`.

| # | Result |
|---|---|
| 3.1 C47 `gpu_compare` | PASS: 18 of 18 cases `0 differing bytes (indices), 0 differing pixels (rgba)`, `all 18 cases: 0 differing bytes` |
| 3.2 `gpu_compare --perturb 7` | PASS: every case `7 differing bytes (indices), 7 differing pixels (rgba), 7 perturbed`, `Error: 18 of 18 cases differ`, exit 1 |
| 3.3 `gpu_compositor::tests::gpu` | PASS |
| 3.4 `verify::tests::gpu_half` | PASS |
| 3.5 C54 `capture_case::scene_tests::gpu_half` | PASS |
| 3.6 C31 `verify::map::tests::gpu_map` | PASS (real adapter) |
| 3.7 `app_frame_loop` | PASS |
| 3.8 C46.3 `gpu_compositor_on_real_frames` | PASS |
| 3.9 C28 `verify --case map` | PASS: 2,697 draw items, view 7840×4112 at −3200,−192 (same as the last Done run), all three comparisons 0 of 32,238,080 |
| 3.10 C29 `--case map --perturb 7` | PASS (7 on each, exit 1) |
| 3.11 C30/C42/C48 `verify` | PASS: `summary: 11 pass, 0 fail, 0 error, 0 GPU not wired, 0 no adapter` |
| 3.12 C42 `verify --perturb 7` | PASS: `summary: 0 pass, 11 fail …`, exit 1 |
| 3.13 C16 `--case map --perturb 5` | PASS (exit 1) |
| 3.14 C43 `verify --ds1 …townN1.ds1` | PASS |
| 3.15 C49 PL2 check | not run (no command); answered by measurement in `claude/spec-answers-render`: entry 0 = (0,0,0) in all 17 `pal.pl2` and `.dat` (VM1 / `composition.md` OQ1) |

## Batch 4: client window

| # | Result |
|---|---|
| 4.1 C24 `play --synthetic --frames 1500` | PASS: ≈ 25 server ticks per second (frame 250: 105 ticks … frame 1250: 522), exit 0 |
| 4.2 C40/C53 `play --frames 1500` | **FAIL**: `play: game data from D2_GAME_DIR (137 levels, 573 objects, waypoint object class 119; level files: 2043 DS1, 34 lvlsub DS1, 241 DT1)`; frame 250 at 104 server ticks, then the server thread panics `crates\d2-sim\src\drlg\room.rs:144:44: live DRLG room` and the client stops (`server thread stopped: no answer from the server thread`), exit 101 |
| 4.3 C41 `app_single_player` + `app_frame_loop` | PASS |

## Batch 5: replays (baselines)

| # | Result |
|---|---|
| 5.1 `tick_replay` | PASS (7) |
| 5.2 C57 `recordings-needed` | units-anim, stats-lists, packets, movement-walk, placement-players MISSING; movement-path-state, placement-monsters-items BLOCKED (`traces/raw` here holds only the fresh table dump) |
| 5.3 C39 `packets_replay` | FAIL in 0.00 s: input recordings absent on this PC, not a baseline |
| 5.4 C55 `placement_replay` | FAIL in 0.00 s: same |
| 5.5 C56 `movement_replay` | FAIL in 0.00 s: same |

Claims: no `// Covers:` line was unlocked (every test whose notes carry an
intended claim either failed or was already claimed).

## Failure messages (verbatim)

#### 1.5
```text
thread 'cof_every_live_file_parses' (26132) panicked at crates\d2-formats\tests\game_sweep.rs:87:39:
data\global\chars\am\cof\amblxbw.cof: file not found: data\global\chars\am\cof\amblxbw.cof

---- string_tables_every_key_resolves stdout ----
string tables: 29 (10 languages {"chi", "deu", "eng", "esp", "fra", "ita", "jpn", "kor", "pol", "por"}), 63167 keys, 4236 resolve to an earlier duplicate

thread 'string_tables_every_key_resolves' (22964) panicked at crates\d2-formats\tests\game_sweep.rs:413:5:
assertion `left == right` failed
  left: 29
 right: 33

---- ds1_every_file_parses stdout ----
ds1: 2372 files, versions {3: 1, 8: 6, 12: 14, 13: 36, 15: 13, 16: 229, 17: 147, 18: 1926}
thread 'ds1_every_file_parses' (15960) panicked at crates\d2-formats\tests\game_sweep.rs:255:5:
assertion `left == right` failed
  left: 2372
 right: 2456

---- dc6_every_file_decodes stdout ----
dc6: 1653 files, 26317 frames, 140 flipped, termination {[00, 00, 00, 00]: 3C, [CD, CD, CD, CD]: 190, [EE, EE, EE, EE]: 4A9}
thread 'dc6_every_file_decodes' (12932) panicked at crates\d2-formats\tests\game_sweep.rs:142:5:
assertion `left == right` failed
  left: 1653
 right: 1657

---- dt1_every_live_file_decodes stdout ----
dt1: 250 live files, 15873 tiles, version-4 ["d2data.mpq:data\\global\\tiles\\act1\\barracks\\barracks.dt1", "d2data.mpq:data\\global\\tiles\\act1\\barracks\\gargtrap.dt1", "d2data.mpq:data\\global\\tiles\\act1\\catacomb\\catacombs.dt1", "d2data.mpq:data\\global\\tiles\\act1\\cathedrl\\cathedrl.dt1"
thread 'dt1_every_live_file_decodes' (29028) panicked at crates\d2-formats\tests\game_sweep.rs:227:5:
assertion `left == right` failed
  left: 250
 right: 254


failures:
```
#### 2.1
```text
thread 'items::inventory::tests::real_grid_belt_and_type_tables' (32596) panicked at crates\d2-sim\src\items\inventory\tests.rs:1540:57:
C:\Users\zffit\Desktop\D2test\D2RUST-Project\game/extracted/patch_d2/data/global/excel/inventory.bin: The system cannot find the file specified. (os error 2)


failures:
    items::inventory::tests::real_grid_belt_and_type_tables
```
#### 2.4a
```text
thread 'cubemain_vector_records' (18912) panicked at crates\d2-sim\tests\game_world.rs:345:5:
assertion `left == right` failed
  left: 129
 right: 130

---- waypoint_objects stdout ----
thread 'waypoint_objects' (15108) panicked at crates\d2-sim\tests\game_world.rs:146:9:
assertion `left == right` failed: FrameCnt1 of 119
  left: 3840
 right: 15

---- vendor_columns_from_live_items stdout ----

thread 'vendor_columns_from_live_items' (31760) panicked at crates\d2-sim\tests\game_world.rs:427:36:
weapons.HratliMin


failures:
    cubemain_vector_records
    vendor_columns_from_live_items
```
#### 2.4b
```text
thread 'lvlprest_measurements' (26944) panicked at crates\d2-sim\tests\game_drlg_tables.rs:229:5:
assertion `left == right` failed
  left: 1079
 right: 82

---- act1_placement_on_live_tables stdout ----
thread 'act1_placement_on_live_tables' (30240) panicked at crates\d2-sim\tests\game_drlg_tables.rs:329:5:
assertion `left == right` failed
  left: [5, 27, 6, 7, 26, 39, 17, 1, 2, 3, 4]
 right: [16, 15, 14, 13, 12, 11, 10, 9, 8, 5, 27, 6, 7, 26, 39, 17, 1, 2, 3, 4]


failures:
```
#### 2.5a
```text
thread 'sweep_drop_quality_every_item' (9244) panicked at crates\d2-sim\tests\game_treasure.rs:454:25:
item 520 L 0 M -100 mods [0, 0, 0, 0, 0, 0]: magic gate


failures:
    sweep_drop_quality_every_item
```
#### 2.5b
```text
thread 'sweep_create_every_item_every_quality' (22204) panicked at crates\d2-sim\tests\game_items.rs:391:5:
570 failures, first: [
    "item 39 dgr  exp false d 0 q 8 ilvl 1: crafted affix 0 with a filled slot (read at 0x5C)",
    "item 63 sst  exp false d 0 q 8 ilvl 1: crafted affix 0 with a filled slot (read at 0x5C)",
    "item 63 sst  exp false d 2 q 8 ilvl 1: crafted affix 0 with a filled slot (read at 0x5C)",
    "item 88 leg  exp false d 0 q 8 ilvl 1: crafted affix 0 with a filled slot (read at 0x5C)",
    "item 276 ob1  exp true d 0 q 8 ilvl 1: crafted affix 0 with a filled slot (read at 0x5C)",
```
#### 2.7
```text
thread 'every_lvlprest_ds1_parses' (29772) panicked at crates\d2-server\tests\game_world_data.rs:167:5:
assertion `left == right` failed: object ids ≥ 573
  left: {}
 right: {580: 46, 581: 135, 582: 24}
FAILED
test wired_game_on_live_tables_runs_100_ticks ... frame 100; messages queued: 0
```
#### 2.8
```text
thread 'world_data::tests::game::outdoor_levels_generate_through_the_dispatcher' (24044) panicked at crates\d2-server\src\world_data\tests\game.rs:179:5:
assertion `left == right` failed
  left: 97
 right: 98


```
#### 2.10a
```text
thread 'real_levels_rows' (26316) panicked at crates\d2-sim\tests\game_monsters.rs:1037:13:
assertion `left == right` failed: level 15
  left: 3800
 right: 2025

---- ai_index_of_every_row stdout ----
thread 'ai_index_of_every_row' (12264) panicked at crates\d2-sim\tests\game_monsters.rs:948:13:
assertion `left == right` failed: row 528 drehyaiced
  left: 129
 right: 31

---- type_init_every_class stdout ----

thread 'type_init_every_class' (31776) panicked at crates\d2-sim\tests\game_monsters.rs:508:17:
assertion `left == right` failed: class 0 d 1 expansion false
  left: 115456
 right: 57728


failures:
```
#### 2.11
```text
thread 'cof_every_live_file_parses' (27036) panicked at crates\d2-formats\tests\game_sweep.rs:87:39:
data\global\chars\am\cof\amblxbw.cof: file not found: data\global\chars\am\cof\amblxbw.cof
test animdata_matches_every_cof ... ok
test cof_every_live_file_parses ... FAILED
string tables: 29 (10 languages {"chi", "deu", "eng", "esp", "fra", "ita", "jpn", "kor", "pol", "por"}), 63167 keys, 4236 resolve to an earlier duplicate

thread 'string_tables_every_key_resolves' (32520) panicked at crates\d2-formats\tests\game_sweep.rs:413:5:
assertion `left == right` failed
  left: 29
 right: 33
test string_tables_every_key_resolves ... FAILED
palettes: 19 .dat (19 pal.dat), 17 .pl2, text colors {12: 1, 13: 16}
test palettes_every_file_parses ... ok
thread 'ds1_every_file_parses' (29072) panicked at crates\d2-formats\tests\game_sweep.rs:255:5:
assertion `left == right` failed
  left: 2372
 right: 2456
test ds1_every_file_parses ... FAILED
dc6: 1653 files, 26317 frames, 140 flipped, termination {[00, 00, 00, 00]: 3C, [CD, CD, CD, CD]: 190, [EE, EE, EE, EE]: 4A9}

thread 'dc6_every_file_decodes' (10868) panicked at crates\d2-formats\tests\game_sweep.rs:142:5:
assertion `left == right` failed
  left: 1653
 right: 1657
test dc6_every_file_decodes ... FAILED
dt1: 250 live files, 15873 tiles, version-4 ["d2data.mpq:data\\global\\tiles\\act1\\barracks\\barracks.dt1", "d2data.mpq:data\\global\\tiles\\act1\\barracks\\gargtrap.dt1", "d2data.mpq:data\\global\\tiles\\act1\\catacomb\\catacombs.dt1", "d2data.mpq:data\\global\\tiles\\act1\\cathedrl\\cathedrl.dt1"

thread 'dt1_every_live_file_decodes' (30896) panicked at crates\d2-formats\tests\game_sweep.rs:227:5:
assertion `left == right` failed
  left: 250
 right: 254
test dt1_every_live_file_decodes ... FAILED
dcc: 21717 files, 271176 directions, 3305132 frames
test dcc_every_file_decodes ... ok
```
#### 2.12
```text
thread 'cpu_compositor_on_real_frames' (22544) panicked at crates\d2-client\tests\game_assets.rs:652:5:
assertion `left == right` failed: compose vs direct: 7683 of 480000 bytes differ, first at (310, 80): expected 15, got 173
  left: 7683
 right: 0
FAILED
test each_loader_loads_a_real_file ... loading data\global\tiles\ACT1\BARRACKS\barE.ds1, data\global\tiles\ACT1\BARRACKS\barset.dt1, data\global\items\fkpskp.DC6, data\global\CHARS\AI\HD\AIHDBHMA11HS.dcc, data\global\palette\ACT1\Pal.PL2, data\global\CHARS\AI\COF\AIA11HS.COF, data\global\palette\act
```
#### 2.13a
```text
thread 'wired_host_amazon' (29328) panicked at crates\d2-server\tests\game_wired_host.rs:301:5:
no town preset object with operate function 23
FAILED
test wired_host_assassin ... 
thread 'wired_host_assassin' (23320) panicked at crates\d2-server\tests\game_wired_host.rs:301:5:
no town preset object with operate function 23
FAILED
test wired_host_barbarian ... 
thread 'wired_host_barbarian' (23476) panicked at crates\d2-server\tests\game_wired_host.rs:301:5:
no town preset object with operate function 23
FAILED
test wired_host_druid ... 
thread 'wired_host_druid' (24044) panicked at crates\d2-server\tests\game_wired_host.rs:301:5:
no town preset object with operate function 23
FAILED
test wired_host_necromancer ... 
thread 'wired_host_necromancer' (31812) panicked at crates\d2-server\tests\game_wired_host.rs:301:5:
no town preset object with operate function 23
FAILED
test wired_host_paladin ... 
thread 'wired_host_paladin' (29292) panicked at crates\d2-server\tests\game_wired_host.rs:301:5:
no town preset object with operate function 23
FAILED
test wired_host_sorceress ... 
thread 'wired_host_sorceress' (27952) panicked at crates\d2-server\tests\game_wired_host.rs:301:5:
no town preset object with operate function 23
FAILED

failures:

failures:
```
#### 4.2
```text
thread 'd2-server' (19196) panicked at crates\d2-sim\src\drlg\room.rs:144:44:
live DRLG room

thread 'Compute Task Pool (20)' (13324) panicked at C:\Users\zffit\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\bevy_ecs-0.19.1\src\error\handler.rs:130:1:
Encountered an error in system `<Enable the debug feature to see the name>`: server: server thread stopped: no answer from the server thread

Encountered a panic in system `<Enable the debug feature to see the name>`!
Encountered a panic in system `<Enable the debug feature to see the name>`!
error: process didn't exit successfully: `target\release\d2-client.exe play --frames 1500` (exit code: 101)
```
#### 5.3
```text
thread 'recorded_messages_replay_exactly' (29500) panicked at crates\conformance\tests\packets_replay.rs:253:5:
no traces/raw/*-packets.jsonl
test recorded_messages_replay_exactly ... FAILED

failures:

```
#### 5.4
```text
thread 'recorded_player_placements_replay_exactly' (3880) panicked at crates\conformance\tests\placement_replay.rs:252:5:
no traces/raw/*-packets.jsonl
test recorded_player_placements_replay_exactly ... FAILED

failures:

```
#### 5.5
```text
thread 'recorded_walks_replay_exactly' (31568) panicked at crates\conformance\tests\movement_replay.rs:366:5:
no traces/raw/*-packets.jsonl
test recorded_walks_replay_exactly ... FAILED

failures:

```
