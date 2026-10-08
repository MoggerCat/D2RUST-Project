# Local run guide (Windows PC with the original 1.14d install)

One ordered, copy-pasteable list of everything the cloud work has queued
that needs game files (`game/`), a real GPU, or a recording. It is a
reading aid over `docs/HANDOFF.md` §4 (command map) and §5 (local run
queue, entries `C<n>` / `A`) plus the `docs/handoff/*.md` notes; **those
stay the source of truth**. Every command and expected value below comes
from them; `NOT FOUND` marks anything the notes do not give. A result
that differs from "Expect" is a finding for the owner spec, not a reason
to change the numbers (HANDOFF §5 header; M01, hard rule 10).

Shell: PowerShell on Windows 10, repo root as the working directory.
`py` is Python 3.10. Disk is tight (CLAUDE.md): build only what a batch
names; Batches 4 and 5 are the only ones that build Bevy (`d2-client`).

Contents: [0 Setup](#0-setup-once) · [1 Tables and formats](#batch-1-tables-and-formats-fast) ·
[2 Game-file tests](#batch-2-game-file-tests---ignored) · [3 Real GPU](#batch-3-real-gpu-verify-cases) ·
[4 Client window](#batch-4-the-client-window) · [5 Replays on existing recordings](#batch-5-replays-on-existing-recordings-no-player) ·
[6 New recordings](#batch-6-new-recordings-player-at-the-game) · [7 Not runnable yet](#7-not-runnable-yet-and-no-test-code-yet) ·
[8 What to send back](#8-what-to-send-back)

## 0. Setup once

```powershell
git fetch origin; git checkout -b claude/local-$(Get-Date -Format yyyy-MM-dd) origin/<branch you are asked to run>
$env:D2_GAME_DIR = "C:\path\to\D2RUST-Project\game"    # the 1.14d install (MPQs + Game.exe), NOT a copy of the repo
cargo install cargo-nextest --locked                   # optional: tools/gate.sh uses it when present, else cargo test
```

`D2_GAME_DIR` is read by every `#[ignore]` game-file test, `data-tool`,
`d2-client play` and `hash-manifest`. Set it in each new shell.

### 0.1 Verify the install against the reference

```powershell
cargo run -p hash-manifest -- $env:D2_GAME_DIR my-install.toml
git diff --no-index --stat traces/reference-install.toml my-install.toml    # or: fc traces\reference-install.toml my-install.toml
```

`traces/reference-install.toml` lists 19 files (names, sizes, SHA-256;
`.mpq`, `.exe`, `.dll`). `Game.exe` must be 1.14.3.71 (HANDOFF §6); the
reference `game.exe` is 3,618,792 bytes, sha256
`631066c1…adaaf`.

| Diff shows | Meaning |
|---|---|
| nothing | Identical install. Go on. |
| only `.exe` / `.dll` entries differ (launcher / helper files) | Harmless for game-file results. **Exception:** `record_*.py` and `dump_tables.py` refuse a `Game.exe` whose SHA-256 is not the reference one (`trace-recorder/README.md`): if `game.exe` differs, Batch 6 cannot run. |
| **any `.mpq` entry differs** (size or hash) | Game-file results are **not comparable** to the specs and notes. Stop; do not record results as passes. Report the diff. |

`my-install.toml` is a scratch file; do not commit it (it is not
`traces/reference-install.toml`). The "only exe / dll may differ" rule
and the tool's behavior are from the task brief and
`tools/hash-manifest/src/main.rs`; no HANDOFF line states the mismatch
policy (`NOT FOUND` there).

### 0.2 Record keeping (applies to every batch)

For each batch record in `docs/HANDOFF.md` §5 **Done** a block headed
`Done <date> (local, branch claude/local-<date> from <short commit>)`
with: the command, the printed counts / lines the batch asks for, pass /
fail, and for a GPU batch the adapter name, backend and driver
(`AdapterInfo` line). Remove the matching `C<n>` entry from the queue
(HANDOFF §5 header: "a local session runs them, records the result, and
removes the entry"). Then, for each passing test, unlock its claims:
turn `// Claim once the first local run passes` / `// Intended claim`
lines into `// Covers:` (same ids), run `py tools/coverage.py --check`
(and `--summary`), and write the new game / verified unit counts in §1
(`docs/COVERAGE.md` §3). A failing test: correct the expected value from
the observation **and** the spec, or drop the claim in the same session.

Files that hold claim lines (found with grep): `crates/d2-sim/tests/
game_core.rs`, `game_treasure.rs`, `game_items.rs`, `game_inventory_path.rs`,
`crates/d2-formats/tests/game_sweep.rs`, `crates/d2-server/tests/
game_wired_host.rs`, `game_world_data.rs`, `crates/d2-client/tests/
game_assets.rs`.

---

## Batch 1: tables and formats (fast)

Run in this order. Release builds of the three tools; no Bevy.

| # | Command | Expect | Record |
|---|---|---|---|
| 1.1 | `cargo run --release -p data-tool -- tables` | 73 runtime tables, 72 identical, 1 explained, 0 mismatched; code buffers 4/4 identical (last Done run, unchanged by later changes) | §5 Done |
| 1.2 | `cargo run --release -p data-tool -- links` | no broken link in the live `.bin` set | §5 Done |
| 1.3 | `cargo run --release -p mpq-tool -- check` | every archive block decodes | §5 Done |
| 1.4 | `cargo run --release -p mpq-tool -- formats` | 0 errors in every kind; cof 3,606 files / 3,605 parsed + `amblxbow.cof`; dc6 1,653; dt1 256 files / 250 parsed (the 6 gaps are `KNOWN_UNUSED`); ds1 2,372 (measured 2026-10-08, case-insensitive names; the older 1,657 / 254 / 260 counts were case-sensitive) | §5 Done |
| 1.5 | `cargo test -p d2-data -p d2-formats -- --ignored` | last Done run: 50 pass (29 before). Measured 2026-10-08: **64 pass, 2 fail** (`game_sweep` `ds1_every_file_parses` and `dt1_every_live_file_decodes`, count assertions; cargo stops after `game_sweep`, so run `--test mpq_game --test prop_more_formats --test tests_c2fmt --test wav_game` separately, or use `--no-fail-fast`; debug `game_sweep` takes ~11 min, release ~1 min). Any failure names the assertion. | §5 Done |
| 1.6 | `cargo test -p d2-data --test patch_game -- --ignored` | 5/5 pass incl. G1–G8 (included in 1.5; run alone if 1.5 fails) | §5 Done |
| 1.7 | `cargo run --release -p data-tool -- patch check game/patch-example/overhaul.d2stack` | exit 0, one N01 note, data digest `66010ecda7c8df5b7135579888c536fd2a30287fb877848719a31b6c8f97a625` (needs `game/patch-example/`) | §5 Done |
| 1.8 | `cargo test -p d2-data --test game_data -- --ignored fixups_on_live_set` (C32) | pass (`fixup::apply` returns `Ok` under the new n² × 128 budget) | §5 Done |
| 1.9 | `cargo run --release -p data-tool -- dump-compare traces/raw/20261006-021210-tables` (C33) | 70/70 tables, `itemtypes_equiv` and `montype_equiv` identical to 1.14d memory. Needs that raw dump (gitignored, not in git): if it is gone, record a fresh one first (`py tools/trace-recorder/dump_tables.py`, ~7 s, then `dump-compare traces/raw/<time>-tables`). | §5 Done |
| 1.10 | `py tools/trace-recorder/check_stats.py --files $env:D2_GAME_DIR` (C2 part) | pass: 359 stats, 84 ops, 42 op targets (last Done run used `game`) | §5 Done |
| 1.11 | `py tools/trace-recorder/path_tables.py` | exit 0 (cross-check for 2.9) | with 2.9 |
| 1.12 | C21: `cargo test -p d2-formats --test formats_game -- --ignored ds1_layer_limits_and_truncated_trees_groups` | pass: `trees.ds1` version 12, tag_type 1, 14 groups, `groups_truncated`, 14th group y / width / height 0, `files` holds `C:\D2\DATA\GLOBAL\TILES\ACT1\TOWN\trees.tg1` (case ignored). On failure fix the expected value from the file or drop the claim. | §5 Done |
| 1.13 | C22: `cargo run -p data-tool -- tables`, then inspect `automap.bin` for a record whose LevelName / TileName is empty or starts with `0` | expect none. No tool command exists; measured 2026-10-08 with `target
elease\mpq-tool extract game\d2exp.mpq 'data\global\excelutomap.bin' <scratch>` and a Python scan of the 44-byte records after the 4-byte count (LevelName bytes 0..16, TileName 16..24): 3,286 records, none empty or starting with `0`. | §5 Done |

Claims unlocked: C21 → the last unclaimed unit of `ds1.md` (edge cases).
1.1–1.10 are regression runs; they unlock nothing new.

## Batch 2: game-file tests (`--ignored`)

All need `$env:D2_GAME_DIR`. Use `--release` where shown (the sweeps are
heavy). Counts are the expected `passed`; "never run" means nothing has
passed yet. Run in this order (cheapest and most basic first).

| # | Command (HANDOFF entry) | Expect | Claims unlocked after pass (add `// Covers:`) |
|---|---|---|---|
| 2.1 | `cargo test -p d2-sim -- --ignored` (C1) | 5 pass: `codes_match_game_tables`, `real_skill_vectors`, `real_table_constants`, `real_level_stats`, `live_monstats_records` (passed in the last Done run) | none new |
| 2.2 | `cargo test -p d2-sim --lib real_grid_belt_and_type_tables -- --ignored` (C36) | pass: `inventory.bin` 32 records with the §1.3 sizes for 0–15; `belts.bin` numboxes `12, 8, 4, 16, 8, 12, 16, 12, 8, 4, 16, 8, 12, 16`; itemtypes codes `bow `, `axe `, `h2h ` space-padded. Never run. | `inventory.md` D1–D3 claims (no claim until pass) |
| 2.3 | `cargo test -p d2-sim --lib path::search::tests::expfield_live -- --ignored` (C38) | pass: `ExpField.D2` 65,546 bytes, header (0x010A, 256, 256), F1 bytes `3 4 5 / 2 8 6 / 1 0 7`, F2 / F3 walks as the spec lists | `path-placement.md` §7.3 r1, r2 (already counted at game tier; verified only after this) |
| 2.4 | `cargo test -p d2-sim --test game_world -- --ignored` then `… --test game_drlg_tables -- --ignored` (C34) | `game_world` **10 passed**; `game_drlg_tables` **12 tests, 12 passed** (2026-10-08: 11 pass, `lvlprest_measurements` 80 vs 82 open as `q-fix-lvlprest-beyond-files`). Failure hints are in HANDOFF C34 (waypoint TSV diff, `cubemain_live_facts` mod count assumes empty `mod` link = −1, DrlgType ∉ {1,2,3}, `act1_placement_on_live_tables` rects are derived). | `waypoints.md` §1 r1, §1 r2, §1 r4, §7 r4 (`waypoint_map_matches_tsv`), §5 r1 (`waypoint_objects`); `npc.md` §1.1 r4, r5; `vendors.md` §9.3; `preset.md` §2 r2 (`lvlprest_def_is_the_row_number`); `maze.md` §1 r3; `levels.md` §3 r2, §3 r3, §4 r1 and `preset.md` §3.1 r3 (`act1_placement_on_live_tables`). This also runs the code for C5, C6, C12-part, C14, C19. |
| 2.5 | `cargo test -p d2-sim --test game_treasure -- --ignored` then `… --test game_items -- --ignored` (C35; add `--release` if slow; code for C3, C4) | `game_treasure` **16 passed**; `game_items` **8 passed**. Test names and interpretation points (1,013 includes TC 0; `live_nodrop_pairs` excludes total-0 pairs) are in HANDOFF C35. | 13 claims: `treasure.md` §1.3 text, §1.4, §1.5 text, §1.5 r4, §1.6, §2, §4 r2, §4 r4, §5.4 r5, §6 r2; `affixes.md` §1 r1; `quality.md` §edge-cases-original-bugs r4; `generation.md` §3 r1 (measured: game 187 → 200 units, verified 217 → 230). Sweeps and table-fact tests get none. |
| 2.6 | `cargo test --release -p d2-sim --test game_core -- --ignored --nocapture` (C37) | **12 passed**. Look at the printed AnimData scheduled / skipped, `DamageRegen` rows, skill-value count, DS1 histograms, Act I room counts. Interpretation points GS1–GS6 in HANDOFF C37. | `fixups.md` §2 r3; `vitals.md` §4.1 / §1 / §2 / §3; `stat-lists.md` §7.2 r2; `units.md` §4.2; `levels.md` §2, §4, §5, §3 r2, r3, §4 r1 (see HANDOFF C37 for test names) |
| 2.7 | `cargo test --release -p d2-server --test game_world_data -- --ignored --nocapture --test-threads 1` (C37) | **3 passed, 0 failed**; a non-empty `WorldSim::errors()` in `wired_game_on_live_tables_runs_100_ticks` names the failing adapter (wiring finding first). Print: frame / message count after 100 ticks. | as 2.6 where the test names say so |
| 2.8 | `cargo test -p d2-server world_data -- --ignored` (C23) | **3 pass**: `act1_placement_matches_the_recorded_vector` (seed 644409375 → `dwStartSeed` 4014346869, DRLG seed {1406222081, 1674353446}, allocation order 4, 3, 2, 1, 17, 39, 26, 7, 6, 27, 5, rects and origins listed in HANDOFF C23), `den_of_evil_matches_the_maze_vector` (defs 57, 86, 96 at (+24, 0), (+24, +24), (0, 0)), `outdoor_levels_generate_through_the_dispatcher` (Blood Moor 81 rooms / 33 outdoor, Cold Plains 98 / 37). Loads 2,043 DS1s up front. | none stated; unblocks `wire-worldgen.md` §7 check 1 (RNG trace compare, §7 below) |
| 2.9 | `cargo test -p d2-sim --test game_inventory_path -- --ignored --nocapture` (C51) | **6 passed, 0 failed**: `live_inventory_records_both_resolutions`, `live_page_grids_every_owner`, `live_belt_capacities`, `live_potion_groups_similar`, `sweep_every_item_size_places`, `live_path_tables_equal_game_exe`. Record the printed belt-item count, item sizes, zero-size items, items larger than 10 × 4, beltable and 1 × 1 beltable counts. Look first at GX1–GX3 (record 29 = 255 × 255, pages 5 / 0xFF, body location 8 non-belts). With 1.11 agreeing (exit 0). | `inventory.md` §1.3, §1.2, §3 r1, §3 r4 (sweep and path-table test get none) |
| 2.10 | `cargo test --release -p d2-sim --test game_monsters -- --ignored` then `… --test game_skills -- --ignored` (C45) | `game_monsters` **19 passed**; `game_skills` **16 passed, 0 failed**. Record the evilhut row (528 or 529) in `population.md`; interpretation points GM1–GM6 in HANDOFF C45. | the claim table of `docs/handoff/game-tests-monsters-skills.md` §4 per passing test (raised game tier 187 → 277 units when measured) |
| 2.11 | `cargo test --release -p d2-formats --test game_sweep -- --ignored --nocapture` (C46.1) | **11 passed**, each sweep printing its counts (COF 3,606 files as `mpq-tool formats` counts). A failure naming a count while every file decodes is GA3; `animdata_matches_every_cof` on `with_cof` is GA4. DC6 1,651 instead of 1,657 means the six `patch_d2.mpq` names come from elsewhere: record the tool's method in the spec and fix `files_where`. | 16 game-tier units if all pass: `assets.md` 1 → 5, `render-pipeline.md` 3 → 4, `animdata.md` 1 → 8, `cof.md` 3 → 4, `dcc.md` 12 → 13, `palette.md` 2 → 3, `tbl.md` 3 → 4 |
| 2.12 | `cargo test --release -p d2-client --test game_assets -- --ignored --nocapture --test-threads 1 --skip gpu_compositor_on_real_frames` (C46.2; builds Bevy) | **5 passed**, printing the listed-name count (**0 refused**) and the outcome of the two text `.tbl` files (GA1). A refused non-ASCII name needs a rule (`assets.md` OQ2). | with 2.11 (same table) |
| 2.13 | `cargo test --release -p d2-server --test game_wired_host -- --ignored --nocapture --test-threads 1` (C59, note `game-tests-wired-host` §4; **run after HANDOFF §2 step 7u(a)** has updated step 4: the file asserts the stub for C→S 0x03 and the walk handler now exists) | **7 passed, 0 failed** (`wired_host_amazon`, `_sorceress`, `_necromancer`, `_paladin`, `_barbarian`, `_druid`, `_assassin`). Record per class: Blood Moor arrival position and frame, kill / pick-up notes (F1), `digest <hex>`. **Run it twice and compare the seven digest lines: they must be equal.** A failing assertion names its step; read it against F1–F7 first. | none (integration only) |
| 2.14 | `cargo test -p d2-client --lib composite::tests::all_live_cofs_give_slot_orders -- --ignored --nocapture` | passed in the last Done run (C9: 3,511 distinct names + `amblxbow.cof`, 242,300 frames, 0 failures); regression only | none new |
| 2.15 | `cargo test -p d2-client --lib frames::tests::all_live_frame_sets_build_and_pack -- --ignored --nocapture` | passed in the last Done run (C8: 23,595 files, 6 parse errors, 288,702 frame sets, 3,345,171 frames, largest 96×960 ≤ 2046); regression only | none new |
| 2.16 | `cargo test -p d2-formats -- --ignored` then `cargo run --release -p mpq-tool -- formats` (C61, eighth fold) | everything that passed before still passes; all **5,008 Huffman + ADPCM `.wav` files** decode to their exact RIFF size (`mpq.md` Observations): the Huffman decoder was sped up (`mpq-huffman`) and no game-file sector has run through the new code |
| 2.17 | `GameData::load` with `ActCreation::Full` on the live set (C60, optional, after 2.13) and the `VendorTables::from_fixed` rows (C63; test home needed, `mutants-world` §4) | C60: the same level ids, seeds and digests as `game_wired_host`; C63: a Charsi `npc.txt` row's multipliers, `difficultylevels` gamble odds and an `itemtypes` row equal `vendors.md` §9.3 / Constants |
| 2.18 | `cargo build --release -p d2s-tool`; `$env:D2_SAVE_DIR = "$env:USERPROFILE\Saved Games\Diablo II"`; `cargo test --release -p d2s-tool --test real_saves -- --ignored --nocapture`; for one save also `target\release\d2s-tool check "<save dir>\<Char>.d2s" --game-dir $env:D2_GAME_DIR` and `… dump …` (C66, `docs/handoff/impl-d2s.md` §4) | `real_saves_round_trip` passes: every `.d2s` parses in `d2s.md` §1 order to the file end and rewrites **byte for byte**; `check` prints OK. A failure names the first differing offset or the internal code: an item entry that does not size is an item-bitstream save-format finding (trailer, unit +0x28, children). Record the first save's header +0x10..+0x37, +0x88..+0xA7, the stats bytes at 0x2FD and `jf`/`kf` bytes (d2s OQ3). | `d2s.md` real-save check (§3, §2.2 r2, §1 r1); add the claim on `real_saves_round_trip` after the pass |

Order note: 2.12, 2.14 and 2.15 build `d2-client` (Bevy); do them once, after
the cheaper rows, in one `d2-client` target directory.

## Batch 3: real GPU (verify cases)

Needs a real adapter (the last Done run was Intel HD Graphics 630, Vulkan;
llvmpipe results elsewhere do not count for the "real GPU" claims). For
**every** command below record the adapter line (`adapter: <name>
(<backend>, <driver>)`). Builds Bevy.

| # | Command (HANDOFF entry) | Expect |
|---|---|---|
| 3.1 | `cargo run -p d2-client --example gpu_compare` (C47) | 21 of 21 cases `0 differing bytes` (21 cases since 2026-10-08) (it was 12/12 in the last Done run; 18 = 12 + `frame-blank-screen`, `frame-no-clear`, `frame-post-clear`, `pixel-write`, `blend-ops`, `frame-stress`) |
| 3.2 | `cargo run -p d2-client --example gpu_compare -- --perturb 7` | every case `7 differing … 7 perturbed`, `Error: 18 of 18 cases differ` (derived from the 12-case Done wording), exit 1 |
| 3.3 | `cargo test -p d2-client --lib gpu_compositor::tests::gpu -- --ignored --nocapture --test-threads 1` | 2 pass (`gpu_matches_cpu`, `gpu_perturb_reports_exactly_n`) |
| 3.4 | `cargo test -p d2-client --lib verify::tests::gpu_half -- --ignored --nocapture` | pass (`gpu_half_matches_cpu_on_every_synthetic_case`) |
| 3.5 | `cargo test -p d2-client --lib verify::capture_case::scene_tests::gpu_half -- --ignored --nocapture` (C54) | `adapter: <real GPU>`, pass; (llvmpipe: `GPU indices: 0`, `GPU: 0 of 480000 pixels differ`, perturb 7 → 7 on all three) |
| 3.6 | `cargo test -p d2-client --lib verify::map::tests::gpu_map -- --ignored --nocapture` (C31) | first line `adapter: <real GPU name> (<backend>, …)`, pass (until now the `map` GPU half is proven on llvmpipe only) |
| 3.7 | `cargo test -p d2-client --test app_frame_loop -- --nocapture` (C27 / C44 / C54) | `adapter: <name> (<backend>, <driver>)` (not llvmpipe), all non-ignored tests `ok`: 6 pass, 0 differing pixels in the GPU node |
| 3.8 | `cargo test --release -p d2-client --test game_assets -- --ignored --nocapture --test-threads 1 gpu_compositor_on_real_frames` (C46.3, needs `D2_GAME_DIR`) | `adapter: <name>`, `GPU indices: 0 of 480000 bytes differ; GPU: 0 of 480000 pixels differ`, pass (the RGBA count after the perturbation is only printed) |
| 3.9 | `cargo run --release -p d2-client -- verify --case map` (C28; needs `D2_GAME_DIR`) | `verify data\global\tiles\ACT1\TOWN\townN1.ds1: <K> draw items, view <W>x<H> at <L>,<T>` with the same K, W×H, L,T as the last recorded run (last Done: 2,697 draw items, view 7840×4112 at −3200,−192); `images in game/renders/verify-townN1`; `PASS: GPU render matches the CPU reference exactly`; `<C> chunks of at most 1024x1024`; `CPU binned: 0 of <W·H> bytes differ`; `GPU indices: 0 of <W·H> bytes differ`; `GPU: 0 of <W·H> pixels differ`; `PASS map (map)`; exit 0. If only the `GPU:` (RGBA) line is non-zero and `GPU indices` is 0, record `palette[0]` of the act palette: that is VM1 (`render/composition.md` §B2), not a compositor bug. |
| 3.10 | `cargo run --release -p d2-client -- verify --case map --perturb 7` (C29) | `CPU binned: 7 of …`, `GPU indices: 7 of …`, `GPU: 7 of …`, `FAIL map (map): CPU and GPU halves`, exit 1. Any other count fails the check. |
| 3.11 | `cargo run --release -p d2-client -- verify` (C30 / C42 / C48) | one `GPU compositor: adapter: <real GPU>` line before the first case; map lines as 3.9; `summary: 11 pass, 0 fail, 0 error, 0 GPU not wired, 0 no adapter`; exit 0. Record whether anything goes wrong with two headless devices (C30 note, superseded by C42's one-device wording). |
| 3.12 | `cargo run --release -p d2-client -- verify --perturb 7` (C42) | every case FAILs with exactly 7 on each comparison, exit 1 (Done: map 7 of 32,238,080, `0 pass, 11 fail`) |
| 3.13 | `cargo run --release -p d2-client -- verify --case map --perturb 5` (C16, regression) | 5 pixels, exit 1 |
| 3.14 | `cargo run --release -p d2-client -- verify --ds1 'data\global\tiles\ACT1\TOWN\townN1.ds1'` (C43) | the map lines of 3.9 (with the report lines printed), exit 0 |
| 3.15 | PL2 palette check (C49): first 1,024 bytes of each act's `pal.pl2` through `scene::present_palette` against the `.dat` palette; entry 0 = (0, 0, 0) | **no command or test found** (`composition.md` OQ1; answers VM1 together with 3.9) |

Claims unlocked: the GPU batch carries no `Covers:` list in the notes; it
moves "real GPU" status from llvmpipe-only to proven (record adapter),
and settles VM1 / `composition.md` OQ1. A mismatch is a compositor or
spec finding.

## Batch 4: the client window

Needs a real GPU and a display. Builds Bevy. Record the adapter line
(`bevy_render::renderer: AdapterInfo { … }`), exit code, and the lines
named below.

| # | Command (HANDOFF entry) | Expect |
|---|---|---|
| 4.1 | `cargo run -p d2-client --release -- play --synthetic --frames 1500` (C24) | black 800×600 view; a log line every 250 frames with `server ticks` ≈ 25 per second, `gpu: true`, `node frames` = frames − 1 (or − 2); no error; exit 0. **Measured 2026-10-08 round 2 (Intel HD Graphics 630, Vulkan):** runs to frame 1500, 628 server ticks, node frames = ticks - 4 (624), `gpu: true`, audio errors 0, two `S→C 0x23 refused: fatal assert 0x668` lines, then exit **101** (`play.rs:521` resource missing after `app.run()`) |
| 4.2 | `cargo run -p d2-client --release -- play --frames 1500` (C40 and C53; replaces C25) | `play: game data from D2_GAME_DIR (<n> levels, <m> objects, waypoint object class <k>; level files: <a> DS1, <b> lvlsub DS1, <c> DT1)` (record n, m, k, a, b, c; drlg-data measured 2,043 lvlprest DS1s); `single player: seed 1234, …`; black 800×600 window (`NoFeed`: no player); log lines every 250 frames with ≈ 25 server ticks per second, `gpu: true`, node frames ≤ the tick count (frames − 1 or − 2), `last view Some(FrameStats { bridge_frame: <f>, server_tick: <t>, items: 0, … })` with `t` equal to that line's server ticks, `audio Some(AudioStats { … load_errors: 0, engine_errors: 0 })`; no error; exit 0. A DRLG error names the level: record it. **Measured 2026-10-08 round 2:** data line `137 levels, 573 objects, waypoint object class 119; level files: 2043 DS1, 34 lvlsub DS1, 241 DT1`, then FAIL at once (exit 101): render panic `draw item 653: drawn area Rect { x: 560, y: 296, width: 32, height: 32 } leaves the gradient block Rect { x: 560, y: 296, width: 32, height: 15 }` |
| 4.3 | `cargo test -p d2-client --test app_single_player --test app_frame_loop -- --ignored --nocapture` (C41, includes C26) | pass: `live_tables_give_a_waypoint_object` (first `objects` row with operate function 23 and init function 17), `live_data_generates_the_levels_from_the_users_files` (print rooms and rect of act 0 levels 1 and 3 and act 1 level 40; Cold Plains is rect (920, 984, 80, 80) only with the recorded init seed, which `--seed` is not, PI1), `frame_loop_runs_on_the_users_levels` (101 frames, 100 ticks); **measured 2026-10-08 round 2: pass**, act 0 level 1: 35 rooms, rect (944, 832, 56, 40); level 3: 96 rooms, (1016, 920, 80, 80); act 1 level 40: 49 rooms, (1000, 1000, 56, 56); `a_save_from_the_command_line_joins` needs `D2_SAVE=<save>.d2s` |

Audio-by-ear smoke (`d2-client view` plays a scripted `Cue` set) is
Blocked until `output::register` / `AudioPlayer` are wired (§7).

## Batch 5: replays on existing recordings (no player)

These use the committed traces and `traces/raw/*` that only the developer
PC holds. Repo-only build (conformance); no Bevy.

| # | Command (HANDOFF entry) | Expect |
|---|---|---|
| 5.1 | `cargo test -p conformance --test tick_replay` | 7 pass, 11,105 ticks, 0 mismatches (re-run after any `WorldSim` / `ActionSim` routing change; also runs in CI) |
| 5.2 | `cargo run -p conformance --bin recordings-needed` (C57; `D2_TRACES_RAW=<dir>` for another folder) | the 7 entries MISSING / present / BLOCKED: `movement-walk` present (2 files, 2026-10-08); `movement-path-state` and `placement-monsters-items` BLOCKED; `placement-players`, `packets`, `units`, `stats` present or missing per the folder. Record the output as the baseline. |
| 5.3 | `cargo test -p conformance --test packets_replay -- --ignored --nocapture` (C39) | **Expect FAIL** at the first message the original dispatched (replay on an empty `SimGame`). Record the `seq` and the message id as the baseline. |
| 5.4 | `cargo test -p conformance --test placement_replay recorded_player_placements_replay_exactly -- --ignored --nocapture` (C55) | prints the placements, then FAIL `PLACEMENT NOT WIRED`. Printed: `20261006-015956` frame-1 0x15, guid 1 at (4863, 5653) (R1); `-022633` (4673, 4548) room unnamed, ten 0x07 before it (R2), and the waypoint 0x15 at (4893, 4993) room level 3 tile (976, 992) (R3). Record players, with_room, monster_assigns, item_messages as the baseline. Different R1–R3 numbers are a reader finding. |
| 5.5 | `cargo test -p conformance --test movement_replay recorded_walks_replay_exactly -- --ignored --nocapture` (C56) | per file `read MoveReadStats { requests, ticks, seeds, sent, other_inputs }`: requests = the 0x01–0x04 counts of `pathing.md` R4 (121 + 47 + 215 + 2 = 385, all accepted) and R5 (49 + 9 + 72 = 130); no 0x0F / 0x10 in `sent`; then FAIL `MOVER NOT WIRED`. |
| 5.6 | Missile lifetimes, think intervals, adjacency, 0xAC decode, group sizes (A-new (1)–(4), (17), (18)) | **code to write first**; expectations in HANDOFF §5 A-new (e.g. missile run counts 69/69 per `missiles.md` Test vectors). Not runnable. |

Claims unlocked: none (all of 5.3–5.5 are baselines that fail by design
until the movers / placement / session are wired).

## Batch 6: new recordings (player at the game)

Windows, `game/Game.exe` must be the reference hash (§0.1). The recorder
terminates the game when it ends (time limit, Ctrl+C, exception). It never
writes to `game/` (except `game/captures/` for frames). Raw output lands
in `traces/raw/` (gitignored). Batch the recordings in one sitting; per
M10 a recorder extension comes **before** a recording that needs hooks it
lacks. Record on the PC; commit only normalized traces
(`traces/sim/...`) that a spec asks for.

### 6.1 Frame captures (`render/capture.md` §8; `crates/d2-client/capture-cases/`)

```powershell
py tools/trace-recorder/record_frames.py --selftest                     # expect: selftest ok
py tools/trace-recorder/record_frames.py --seconds 90                   # stability-0001
cargo run --release -p d2-client -- verify --cases crates/d2-client/capture-cases --case stability-0001
cargo run --release -p d2-client -- verify --cases crates/d2-client/capture-cases --case stability-0001 --perturb 1
```

Play for stability-0001: Single Player, any character, Den of Evil
cleared, a dead end away from doors, stand still 30 s, no panels. Run the
verify **right after** each recording (`raw = "latest"` reads the newest
`traces/raw/*-frames.jsonl`).

- Expect `re-hash: 0 of N frames differ`; `stability: G state groups, K seen at least twice, 0 with differing frames` with K ≥ 2; `PASS stability-0001 (scene)`; exit 0. `--perturb 1`: `re-hash: 1 of N frames differ`, FAIL, exit 1.
- A differing group or K < 2 is a `capture.md` finding (OQ1), not a reason to loosen the check. Trust this verdict over the recorder's own `0 with differing frames` line (CP3). Record every `flag: … ticks carry more than one frame` line (OQ2).

Then **placement-0001, camera-0001, composition-0001** (S7-A2). Record
each with `py tools/trace-recorder/record_frames.py --seconds 30`:
camera-0001: Rogue Encampment, walk 5 s, run 5 s, stand; placement-0001:
stand, open and close the inventory, stand; composition-0001: a
translucent sprite on a known floor (after stability-0001; §8 row 4 of
`capture.md` has the exact script). Each right after:

```powershell
cargo run --release -p d2-client -- verify --cases crates/d2-client/capture-cases --case camera-0001
cargo run --release -p d2-client -- verify --cases crates/d2-client/capture-cases --case placement-0001
cargo run --release -p d2-client -- verify --cases crates/d2-client/capture-cases --case composition-0001
```

Expect today: no ERROR; on the first frame `camera: recorded view rect,
shiftX, tile origin […] and unit origin […] equal camera.md §1, §3`, then
`scene: seam: the recording holds the camera only …`; `N frames selected`;
`frames: 0 match, 0 differ or fail, N scene not wired`; `SCENE NOT WIRED
<id> (scene)`; exit 2. Inventory frames of placement-0001 must pass the
camera check with view rect `[-200, 0, 600, 560]`, shift −200 (mode 1).
**Any FAIL** with `tile origin (camera.md §3): recorded …, rule …` (or view
rect, shiftX, unit origin) is a `camera.md` finding: record the lines, the
draw, and the frame's `shake` / `open_mode`; do not loosen. A `camera: …
no player record` ERROR means the recorder took a frame without the
player. `--perturb N` cannot fail these frames (no pixel is compared at
the seam). After a `SceneSource` is wired (§7): PASS and `--perturb 5` →
every frame `CPU: 5 of 480000 bytes differ`, FAIL, exit 1.
**Claims:** none named; stability-0001 is the first capture verdict.

### 6.2 Walk recording `movement-walk` (S7-A1), then C56

```powershell
py tools/trace-recorder/record_packets.py --seconds 120
py tools/trace-recorder/check_packets.py traces/raw/<time>-packets.jsonl      # expect OK
cargo test -p conformance --test movement_replay recorded_walks_replay_exactly -- --ignored --nocapture
```

Play a **new** single-player character in the Rogue Encampment only:
about 10 single clicks on open ground (walk, 0x01), the same with run on
(0x03), one click on an NPC walking and one running (0x02, 0x04), hold the
button 3 s once (R6), one click into a wall or tent; no waypoint, warp,
skill, item or NPC dialog. Expect until wired: read counts with
`other_inputs` small, FAIL `MOVER NOT WIRED`; after wiring: PASS with
messages = positions = the 0x96 count. Then re-run 5.2 (`movement-walk`
should read present).

### 6.3 Units recording (A "Phase 3 units/stats" item 1, plus `conformance-harness` §5)

```powershell
py tools/trace-recorder/record_tick.py --seconds 240
py tools/trace-recorder/check_units.py traces/raw/<time>-tick.jsonl
py tools/trace-recorder/check_tick.py  traces/raw/<time>-tick.jsonl
cargo test -p conformance --test units_replay -- --ignored --nocapture
```

Normal difficulty: melee and skill combat, a shrine, a well, a vendor or
trade, a cooldown skill. Expect `check_units` and `check_tick` 0 errors
each with "U4 anim exact" and "U11 site rule" counts above 0; the replay
passes with printed `schedules` > 0 (a failure names the record index;
cross-check with `check_units.py` U4 at the same index). Repeat on
Nightmare (AI delays use the Normal `aidel` column unless game +0x6A /
+0x74 is set, `units.md` OQ7). One recording near trap objects with
`record_rng.py` for the seed of object event 0 / 8 delays.
`convert_tick.py` rejects `record_tick.py` 0.2.0 recordings (`anim`
record, CH2): do not convert this file; the units harness reads the raw
file. **Claims:** the replay is `trace` tier; add `// Covers:` only after
it passes.

### 6.4 Stats recording

```powershell
py tools/trace-recorder/record_stats.py --seconds 240
py tools/trace-recorder/check_stats.py traces/raw/<time>-stats.jsonl
py tools/trace-recorder/check_stats.py traces/raw/<time>-stats.jsonl --perturb-snap 5     # must fail at the changed record
py tools/trace-recorder/check_stats.py traces/raw/<time>-stats.jsonl --perturb-cb 0       # must fail at the changed record
cargo test -p conformance --test stats_replay -- --ignored --nocapture
```

Equip and unequip a +vitality / +life item and a weapon, drink a health
and a mana potion, spend a stat point, get poisoned or use a timed buff,
fight, walk and run. Expect 0 errors, callbacks / expiry frees / regen
checks all above 0; the max-life rescale sets match (`stat-lists.md`
§7.2); the replay passes with `operations`, `callbacks`, `snapshots` > 0.
A mismatch names the record as `check_stats.py` numbers it: `seed …` =
d2-sim does not rebuild a dumped tree (`stat-lists.md` §6 / §11);
`isc[s].…` = a load-time column (`fixups.md` §2); `callback.u` = the unit
argument (§7). `trace` tier claims after pass.

### 6.5 RT-R1 timer recording (fifth fold, `wire-routing` §7 check 2)

`record_tick.py` with the timer hooks, a unique monster with a mode-1
umod dying (e.g. fire enchanted, umod 9), group A. Look for the type-7
timer's frame (death frame + 4) and its position among the death
animation's timers in the queue; it settles RT1. No `check_*` command
named: **NOT FOUND** (hand analysis).

### 6.7 Generated characters load in the game (`d2s-tool`, C66 part 2)

```powershell
cargo build --release -p d2s-tool
$t = "target\release\d2s-tool"
& $t new --game-dir $env:D2_GAME_DIR --name TestAma --class ama -o TestAma.d2s
& $t new --game-dir $env:D2_GAME_DIR --name TestSor --class sor --level 30 --expansion --difficulty-unlocked hell --waypoints all --quests acts=4 --all-skills 1 --gold 100000 --item hp1 --item lsd@0,0 -o TestSor.d2s
& $t new-stub --name TestStub --class nec -o TestStub.d2s
```

Copy the three files into the save folder (back the folder up first;
never commit saves). Expect: all three appear at character select with
the right class and level, each enters a single-player game, the items
sit where `dump` says, the waypoints and acts are open. Then exit (the
game re-saves) and run `& $t check <file> --game-dir $env:D2_GAME_DIR` on
each re-saved file and `dump` ours vs the game's: differences in the
stats at 0x2FD, item records (trailer bit, flags such as 0x2000),
+0x88..+0xA7 or the quest words are findings for `formats/d2s.md` /
`items/bitstream.md` (record them in HANDOFF C66).

### 6.6 Other recordings (each needs the recorder extensions of M10 first)

| Recording | Command / script | Expect | Source |
|---|---|---|---|
| RNG capture | `py tools/trace-recorder/record_rng.py --seconds 120`, enter a single-player game, kill a few monsters, pick up a drop; then `py tools/trace-recorder/check_rng.py traces/raw/<file>.jsonl` | game very slow (846 inline sites hooked); `check_rng` no mismatch | HANDOFF §5 A "Next RNG capture" |
| Item moves R1–R6 and the 0x9C / 0x9D bit stream | `record_packets.py`; script per `inventory.md` Test vectors | replay through `Host` (`cargo test -p d2-server --lib items::moves` shape); R1 / R3 settle WN1–WN3, IS1, MV1 | S7-A5, A (sixth fold) |
| Position trace (`path-placement.md` OQ1, R1–R3) | **no recorder exists** | — | `impl-path-place` §5 |
| `movement-path-state` / `placement-monsters-items` | recorder extensions (hooks `0x00650840`; `0x0064DEA0`, `0x00554EA0`, `0x00555DA0`) — BLOCKED in 5.2 | replay allocations and footprints; compare room grids cell by cell | S7-A3 |
| Sorceress casts at a monster that dies | `record_packets.py` + timer hooks | event-0 / event-1 frames after 0x0C, missile creation and hit frames, mode change, experience delta, drop's item GUID and gold; settles EC1–EC3 | `e2e-combat-path` §6 check 2 |
| Aura (paladin) and quests (Den of Evil kill → status 5; Kashya message 92 → 0x50 / mercenary) | `record_packets.py` + timers | type-8 timer frames ≡ 1 mod `perdelay`; settles WO2 | `wire-open-seams` §7 |
| Treasure walk (`record_rng.py` hooks at `0x0055A6D0`, `0x0055AEE7`, `0x0055A9B9`; kill champions / uniques with `players 8`, open chests) | extension first | draws in `0x0055A6D0`–`0x0055AF80` and from `0x00558640` match `treasure.md` §Randomness in order and count | HANDOFF §5 A kept entry |
| Topic recordings: items R1, skills, monsters, world, vitals, DRLG (full RNG hook: Den of Evil, Cave 1, Acts 2–4), NPC / vendor / hire services, hosted multi-client game, audio | hooks and scripts are in HANDOFF §5 A "Phase 3 topic recordings" and A-new (5)–(16), (19) | per entry there | HANDOFF §5 A |
| New game with the population RNG hook (Blood Moor room) | `record_rng.py` with the call sites of `population.md` / `init.md` | identical draw sequence and units through `populate_room` | A-new (14) |
| `dump_tables.py` raw dump (also for 1.9) | `py tools/trace-recorder/dump_tables.py` (~7 s), then `cargo run --release -p data-tool -- dump-compare traces/raw/<time>-tables` | every map with a d2rs counterpart identical, nothing pending | §4 command map |

## 7. Not runnable yet, and no test code yet

Do not spend local time on these; they are listed so a result is not
expected from them.

**No test code yet** (a cloud session writes it from the entry, then it
goes in a batch above): C2 (`StatData` live rows), C7 (the C46 tests are
the code), C10 (skill use / vitals on live tables; partly run by 2.10),
C11 (monster population; `game_monsters` covers part), C12 (maze data),
C13 (`preset_ds1_survey` in `conformance`), C14 / C19 (vendor lists,
quest NPC ids; code in 2.4 per HANDOFF), C18 (combat / AI / missile gap
rules), C20 (treasure dump vs d2rs, reads `traces/raw/20261006-115547-tables`),
C50 (`anim_record`), C62 (`mpq-tool formats --huffman-tables` count of the `.wav` weight tables), C63 (`VendorTables::from_fixed` live rows), C64 (mutation re-run with game files), C52 (`mpq-tool formats` extension for DC6 flip,
DCC odd `variable0`, zero bytes in runs; then `D2_GAME_DIR=<install>
cargo test -p d2-client -- --ignored`).

**Blocked on code or specs** (HANDOFF §5 Blocked): the RNG trace compare of
`wire-worldgen.md` §7.1; wav decoded samples (needs `formats/wav.md`);
`MOVER` / `PLACEMENT` / `SceneSource` wiring for 5.4, 5.5 and the capture
compare; NPC / vendor / quest / trade replays through `Host`; the e2e
replays; audio smoke; the controls original-defaults check
(`specs/ui/controls.md` §B4).

**Optional (C58):** the four benches of HANDOFF §4 (`cargo bench -p d2-sim
--bench sim`, `-p d2-formats --bench formats`, `-p d2-proto --bench
proto`, `-p d2-client --bench compose`); compare with the cloud baselines
of HANDOFF §1 row 3ad (a real machine should not be slower by an order of
magnitude). The note asks for no local run.

## 8. What to send back

Do this on a branch named `claude/local-<date>` (e.g. `claude/local-2026-10-07`):

- [ ] §0.1 diff output (empty, or exactly the differing entries).
- [ ] Batch 1: 1.1–1.13 pass counts (table counts, `mpq-tool` kinds, the new `--ignored` total for 1.5, 1.12 result, 1.13 command used).
- [ ] Batch 2: pass / fail and counts for 2.1–2.15; the printed lines asked for in 2.9 and 2.13; the two `digest` runs of 2.13 side by side; every failure message verbatim.
- [ ] Batch 3: adapter name, backend and driver once per command; all 3.1–3.14 results; the `palette[0]` / VM1 note if 3.9 shows it; 3.15 marked not run.
- [ ] Batch 4: n, m, k, a, b, c from 4.2; node frame count; the rooms / rect lines of 4.3.
- [ ] Batch 5: the baselines of 5.2, 5.3 (`seq`, id), 5.4 (counts), 5.5 (counts).
- [ ] Batch 6: for each recording the file name, `check_*` result, replay result; any `flag:` or FAIL lines.
- [ ] For every passing test, the `// Covers:` edits, `py tools/coverage.py --check` clean, and the new coverage figures.
- [ ] Edit `docs/HANDOFF.md` §5: move finished entries to **Done** (date, branch, commit hash), remove their `C<n>` entries, record findings under §7 / §8.
- [ ] `git commit`, then `git push -u origin claude/local-<date>`.
