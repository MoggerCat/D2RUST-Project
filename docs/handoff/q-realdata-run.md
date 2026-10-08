# q-realdata-run: the first real-data run (M23)

Session `q-realdata-run`, branch `claude/q-realdata-run` (from staging
`40e9e1e`), cloud, 2026-10-08. The first run of every game-file test on
the real 1.14d install, then every failure turned into a fix or a queue
row. Methods: M23 (real data before green), M24 (remaining work).

## Data

- Private repo `MoggerCat/D2RUST-private-repo` at `5174f0cb`: `install/`
  assembled with `tools/assemble.py` into `$HOME/game` (23 files, 0
  mismatches). `hash-manifest` of it equals `traces/reference-install.toml`
  (19 entries, no diff): results are comparable with the specs.
- The repo's `extracted/` is laid out per archive
  (`extracted/<archive>/<archive path>`, original case; `Patch_D2.mpq` 114
  of 210 entries, `d2sfx.mpq` 2,199 of 2,365), not the `extracted/patch_d2/`
  view the table tests read (`$D2_GAME_DIR/extracted/patch_d2/data/global/
  excel/<lowercase>.bin`). The gate no longer checks it out (3 GB); the new
  `data-tool excel-dir <out> [game_dir]` writes the live excel set from the
  install (each file from the highest-priority archive, lowercase names):
  184 of 189 candidate names (129 from `patch_d2.mpq`, 55 from `d2exp.mpq`).
- Private README's layout table updated to the per-archive layout
  (private `9710830`).

## Command

```sh
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_RELEASE_DEBUG=0 \
CARGO_INCREMENTAL=0 D2_GAME_DIR=$HOME/game sh tools/realdata-gate.sh
```

Gate fixes from its first real run (`tools/realdata-gate.sh`,
`tools/realdata_inventory.py`):
- the excel view (above);
- the nextest skip filter used `test(=name)`, which matches the full path,
  so lib tests named in the skip list ran (5 GPU tests failed for want of
  an adapter); it now matches the last path segment;
- new inventory `needs` classes: `recording` (reads `traces/raw/`,
  gitignored local recordings: skipped unless `traces/raw/*.jsonl` exists),
  `save` (reads `D2_SAVE`: skipped while unset), `repro` (ignored as a
  known-bug repro, not for game files: always skipped).
- Cloud prerequisites the gate does not install: `cargo install
  cargo-nextest --locked`; for `d2-client`, `tools/cloud-setup.sh`'s apt
  line (`pkg-config libasound2-dev libudev-dev libwayland-dev
  libxkbcommon-dev`; the first run's `d2-client` step failed to build
  without them); for the llvmpipe runs, `mesa-vulkan-drivers`.

## Totals

| Run | Commit | Passed | Failed | Skipped: GPU | Skipped: other |
|---|---|---|---|---|---|
| 1 (first) | `106f4ad` | 208 | 29 | 0 (filter bug: 5 ran and failed) | 2 dump |
| 2 (after fixes) | `efffaf0` | **213** | **11** | 6 | 9: 2 dump, 5 recording, 1 save, 1 repro |

239 ignored tests in all (run 2: 213 + 11 + 15). Of run 1's 29 failures,
12 were not game-file findings (5 recording replays, 5 GPU, 1 `D2_SAVE`,
1 repro); the other 17 had 8 root causes, 4 fixed here.
Tool checks (both runs): `data-tool tables` PASS (73 runtime tables, 72
identical, 1 explained, 0 mismatched; code buffers 4/4), `data-tool links`
PASS (72,175 valid, 0 broken), `mpq-tool check` PASS (every block
decodes), `mpq-tool formats` PASS (0 errors; cof 3,606 / 3,605 parsed +
`amblxbow.cof`; dc6 1,653; dcc 21,717; dt1 256 / 250; ds1 2,372, 1,926 at
v18).

Per crate, run 2: d2-formats 29/29, d2-data 37/37, d2-sim 110/110
(`game_core` 12, `game_drlg_tables` 12, `game_inventory_path` 6,
`game_items` 8, `game_monsters` 19, `game_skills` 16, `game_treasure` 16,
`game_world` 10, lib 11), d2-server 9/17, conformance 2/2 (`live_preset_ds1`),
d2s-tool 1/1 (`real_saves`: no saves in the cloud, passes on none), scenario-run
1/1, seed-finder 1/1, d2-client 23/26.

Skipped GPU, run separately on **llvmpipe** (Mesa 25.2.8, Vulkan, CPU;
a software adapter, so not the "real GPU" claims of LOCAL-RUN Batch 3):
all 6 pass (`gpu_matches_cpu` 21 cases 0 differing; perturb 7 → exactly 7;
`gpu_compositor_on_real_frames` `GPU indices: 0 of 480000`, `GPU: 0 of
480000 pixels`).

Coverage (`py tools/coverage.py --summary`, after the unlocks): 13,742
claims; rules 11,164 (+625 exempt); unit 9,755 (87.4%); game-file 421
(3.8%); trace 80 (0.7%); **verified 501 (4.5%)**; any tier 9,863 (88.3%).
(PC 1 round 2: game-file 419, verified 499, on 10,919 rules.)

## Findings, most visible first

| # | Test(s) | Observed vs expected | Cause | Outcome |
|---|---|---|---|---|
| 1 | `d2-server` `game_wired_host` (all 7 classes), `world_data::tests::game::outdoor_levels_generate_through_the_dispatcher` | Cold Plains 97 rooms (62 preset + 35 outdoor) vs the recorded 98 (61 + 37); 22 grid cells differ; border substitution 1 matches the recording (type 1, group 0, (3, 1), variant 2, lo' 1833932632), substitution 2 does not (type 2 group 1 at (6, 6): variant 0, lo' 1917574118 vs recorded variant 1, lo' 3559729267 at seq 8644) | code: the outdoor build's draw stream parts between recorded seq 8447 and 8644 (`drlg/outdoor.md` §6–§7) | open, `q-fix-cold-plains-rooms` (row extended). The wired host passes every earlier step now (finding 6) |
| 2 | `d2-client` `app_client_drlg` `the_session_join_on_the_install` | three `Rejected { id: 35, error: Fatal(1640) }`: S→C 0x23 SetSkill refused, fatal assert 0x668 (skill outside the client's table) | code (client skill table at join) | open, `q-fix-set-skill-fatal` (known from PC 1 round 2; reproduced) |
| 3 | `d2-client` `controls::original::tests::defaults_equal_game_exe_table` | `Game.exe` 0x312220 (sha256 `a711045f…cfdd`, the spec's own hash) holds (cmd 1, `B`, slot 0) then (cmd 1, `I`, slot 1); the table built from `key-commands.tsv` (entry 2p = slot 1) gives the reverse. All other 112 entries equal | spec: `ui/controls.md` §3.4 / §B4 r1 states a uniform slot order the binary breaks for command 1; its "Measured 2026-10-07: equal" cannot hold | **new**, `q-fix-real-controls-default-order` (spec work first) |
| 4 | `d2-client` `game_panels` `panel_files_frame_counts_and_sizes` | `menu\horadric` frame 1 offset (−205, 17), the spec says (0, 0) | spec (`ui/panels.md` §7 r2) | open, `q-fix-panel-horadric-offsets` (known; reproduced) |
| 5 | `d2-server` `character_save` `token_positions_on_the_users_install` | `ktr` 51 vs 45 | code: `ReferenceSlots::provisional_1_14d` was the reconstruction `d2s-appearance.md` OQ3 withdrew | **fixed** (`ed7a7aa`): `ReferenceSlots::v1_14d` from §Constants; passes (`ktr` 45, second `ktr` at 243 now asserted) |
| 6 | `game_wired_host` (7) | "town waypoint: not reached after 4 legs" in the Blood Moor / just inside the gate | test assumption: a straight leg back to the start stops behind the palisade; players path greedily (`sim/pathing.md` §5–§6) | **fixed** (`32a38dd`): the walk back retraces the way out (`Session::trail`); now stops at finding 1 |
| 7 | `d2-sim` `game_items` `sweep_create_every_item_every_quality` | 560 failures: ears 160 ("ear without a player"), `ibk` / `isc` 160 each and `0sc` 80 ("affix does not fit") | test assumption, both spec-stated: `generation.md` §9 step 5, `affixes.md` §1 r4 | **fixed** (`d26073f`); passes (10 crafted requests end in `affixes.md` edge case 3, as recorded); claim `generation.md` §3 r1 unlocked |
| 8 | `d2-formats` `game_sweep` `ds1_every_file_parses`, `dt1_every_live_file_decodes`; `d2-sim` `game_drlg_tables` `lvlprest_measurements` | 2,372 / 1,926 vs 2,456 / 1,997; 0x1001 108,905 vs 110,259; 80 vs 82 | stale test values: the specs already say the measured numbers (pc1-s8) | **fixed** (`ff44fdc`), expected values changed because the real data and the spec agree; claims unlocked (`dt1.md` 4 sections, `ds1.md` §rules) |
| 9 | `smoke_town` `a_click_on_an_npc_while_standing_on_the_stash_talks_to_the_npc` (synthetic repro, ran in run 1 because the gate runs every ignored test) | no C→S 0x13 after the walk to Kashya | code (synthetic; q-play-smoke expected it to pass with the client path) | **new**, `q-fix-real-stash-repro`; the gate now skips it as `repro` |

Not findings: the 5 conformance replays need `traces/raw/` recordings
(not in the cloud or the private repo); `a_save_from_the_command_line_joins`
needs `D2_SAVE`.

## Play smoke on the real install

PLAY_SMOKE_PLACEHOLDER

`d2-client` `play_smoke` has no real-install path: its live tests run on
the five-act fixture install (`docs/handoff/q-play-smoke.md`, "the same
run on real 1.14d tables" is an open item).

## Remaining real-data work (M24)

What is left, by size, with how sure each estimate is:

1. **Gate-runnable, open (4 root causes, 11 tests).** Cold Plains draw
   divergence (blocks 8 tests): a DRLG exactness bug located to ~200
   recorded draws; 1–3 sessions, low confidence (the row says "high effort
   if the first fix does not settle it"). SetSkill fatal at join: 1
   session, medium confidence. Horadric offsets and the controls order:
   spec reads on PC 1 plus a small test / table change each, high
   confidence. With these the gate is 224 / 224.
2. **Never run on real data in the cloud.** Recording replays (5 tests:
   packets, placement, movement, stats, tick) need PC 2 recordings in
   `traces/raw/`; the 2 memory-dump tests need `D2_TABLES_DUMP`; `D2_SAVE`
   tests need a save. These are the trace tier, which today covers 80 of
   11,164 rules (0.7%).
3. **The client on real data, by window.** `play` on the live install
   (LOCAL-RUN 4.2) last failed with a render panic (gradient block, row
   `q-fix-play-gradient-block`); see the play smoke section for the cloud
   result. Real-GPU and window batches (LOCAL-RUN 3–4) stay local.
4. **Systems with no real-data check at all.** 87.4% of rules are covered
   only by synthetic unit tests; 4.5% are verified against 1.14d. The gate
   now runs in the cloud in ~15 minutes (release, warm), so every merged
   feature can add a game-file test (M23); the trace tier needs recordings
   and is the bulk of the remaining verification. Size: not estimable as a
   count of sessions from here; the known divergences above are the only
   measured ones, and every real-data run so far found new seams (this one:
   3 new, 4 confirmed), so expect more per system first run on real data.

Uncertainty: items 1 and 2 are counted; items 3 and 4 are open-ended.

## Cleanup

`$HOME/game` (assembled install), the excel view and the private clone are
deleted at the end of the session (disk allowance).
