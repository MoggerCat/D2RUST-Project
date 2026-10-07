# Local buddy PC status

Updated 2026-10-07 16:08 (PC 2, "buddy PC"); FINAL for this run: the orchestrator loop is stopped (user wrap-up before a spec-writing phase). No game, locks or worktrees left running.

## Ninth fold (specs-staging 913d3b0)

- C87 PASS (`ai_index_of_every_row`); C88 PASS (cof: only d2char `amblxbow.cof` fails, ambl1hs / ambl1ht / amblhth parse; tbl 29 tables / 20 paths / 10 languages, 63,167 used entries, 16,786 non-ASCII, 0 raw FF, 130 C3 BF); C89 `world_data` 2/3 (Cold Plains 97 = 62 + 35, unchanged), seed-finder PASS, population monsters per seed 1-8: Den of Evil 66, 75, 62, 79, 78, 100, 96, 94 (27 rooms), Blood Moor 82, 94, 93, 102, 109, 85, 86, 101 (79-84 rooms).
- C90 PASS: new `#[ignore]` test `crates/d2-sim/src/world/hirelings/tests/game.rs`: all 8 `hirelings.md` Test-vector rows match on every offer and unit column (reads "row N" as the bracket whose Level is N; MaxLvl 99 constant); now `// Covers: hirelings.md §1.2 r2, §2, §4`.
- C91 PASS: new `#[ignore]` test `staff_hand_in_period_from_the_live_missiles_range` (`world/quests/act2/tests_q6.rs`): row 338 Range 440 -> period 18.
- S9-A5 / `objects.md` OQ2 answered (obj1, bdMercTwo town -> Blood Moor; check_rng OK 3,103 draws, 28 chain_unexplained; check_packets OK): 26 of 32 object allocations had Sync = 0; no animation setup runs before the init function at allocation; the only animation draw is roll(FrameDelta >> 3) at 0x624563 from a mode set inside the init (class 37: init 8 sets mode 2, roll(25) = 23, speed 211); classes 35, 36, 39 allocated in mode 0 with init leaving the mode get no draw. Two class-37 allocations show speed 0 at return (not examined).
- S9-A6 partly: 29 first populations of Blood Moor rooms with RNG (28 monsters) and 29 dumps of DRLG room +0x64 lists (one-record and grid-built rooms) recorded; the comparison with `drlg::logic` / `WorldSim` is for the cloud; Den of Evil crypt dump not started. New tool `tools/trace-recorder/record_objects.py` 0.1.0.
- R2 status (per item in `docs/handoff/local-buddy-q9-rec.md`): answered from earlier recordings R2-7, -9, -10, -26, -28; partly R2-2, -5, -18, -24, -27, -31, -32, -33, -34, -37; blocked by story progression the Act II-V, Andariel, Cain and Countess items; NOT STARTED R2-4, -6, -23, -25, -35, -36, -38, R2-8 (data in join1-packets.jsonl, not analysed), S9-A1 (6) hireling extras, the five object operates, S9-A4, S9-A7.

| Branch | SHA | Result |
|---|---|---|
| claude/local-buddy-q9-rec-2026-10-07 | 5cfb040 | ninth-fold PC 2 recordings (partial, stopped at wrap-up) |
| claude/local-buddy-q9-a-2026-10-07 | 38c8759 | C87-C91 (C90 / C91 tests written) |

(Earlier sections below are from 12:10 and still current.) Overwritten at every 2-hour summary.
Install check: `hash-manifest` identical to `traces/reference-install.toml`.
Current batch: docs/HANDOFF.md §5 entries 59-86 on main 674996d (notes `docs/handoff/local-buddy-q-*.md` on the q-* branches).

## Findings

HANDOFF §5 entries (main 674996d):
- 59 FAIL: `game_wired_host` 0/7, all at `game_wired_host.rs:642` (stale step-4 stub assert, game-tests-wired-host notes it); no digests.
- 60 PARTIAL: `GameData::load` BinSet / AnimData / LevelTables / WorldFiles equal the hand-built set; digests not run (needs 59).
- 61 FAIL (counts only): `game_sweep` ds1 2,372 vs 2,456 (the fixed mpq-tool confirms 2,372) and dt1 block 0x1001 108,905 vs 110,259; no file errors. 4,975 0x41/0x81 `.wav` blocks decode to exact size (the entry says 5,008; 33 fewer).
- 62: every `.wav` Huffman sector (0x41 / 0x81) uses weight table 8: 350,543 sectors, 313,273 (89.4%) with an escape.
- 63, 65 (2/2), 67 (env periods 18/18, wall-light points 108/108 equal Game.exe), 68 (7,680 / 256 / 2,048), 72 (`wav_game` 2/2), 73 (`live_sound_table`), 76, 81, 86 (10 rooms in sight = the ten 0x07 of the recorded join) PASS.
- 66 PASS: all 13 real saves round-trip byte for byte; `d2s-tool new` TestAma / TestSor / TestStub load and re-save in 1.14d, `check` passes. Game differences: it clears item flag 0x2000 on `hp1` / `lsd` (0x00A02010 -> 0x00A00010, 0x00802010 -> 0x00800010; items/bitstream.md §5 finding); it rewrites +0x88 components to 0xFF; the stub grows 335 -> 953 bytes.
- 71 FAIL: `game_panels` `menu\horadric` frame 1 offset (-205, 17), expected (0, 0); later skltree counts not reached.
- 77, 82 PARTIAL: `play` exits 101 at `drlg/room.rs:152:44` ("live DRLG room", `free_inactive_rooms`) after about 106 ticks, before frame 500; I / C / T toggles OK, no UI / sound / palette / 0x07 refusals; capture cases ui-0001 / 0002 not run.
- 69 DONE: 41,892 client updates, 28 S->C 0x53, one in-game day = 35,844 updates, all 6 periods; new `check_env.py` replays lighting.md §9.3-§9.4 with 0 mismatches; truncated ticks/speed would give 22,132 mismatches, so θ is fractional (answers lighting OQ7 for act 1). d2rs `Environment::update` replay: for the cloud (no harness yet). Light-map key in record_frames 0.2.1 not yet run live.
- 70 PARTIAL (from the pass-1 rain recording): 24,427 weather lines match `gdi_line_pixels` on 99.26% of pixels; the end point is not drawn when the rule ends one step short (27 of 15,824 show it). No |dx| = |dy| > 0 line seen: the tie case (`LineMajorAxisTie`) stays open; shadow comparison not done.
- 74 PARTIAL: new `record_sound.py` (hooks of the entry, must run without `-ns`): 4,106 requests over town walk, Warriv talk, item moves, a fight, town -> Blood Moor -> town and a day change (ambience bed 70 -> 71 at C 23,041); cave and Blood Raven not reached.
- 80 DONE: town portal both ways with bdMercTwo's hireling (GUID 1): its 0x0A is queued in the input phase of C->S 0x13; its 0xAC at the destination comes the next tick, before the player's 0x15; on the first teleport only, an extra hireling 0x15 to the same point one frame later.
- 83 RECORDED: 32 client monster adds, all at non-zero points, each stepped its room's seed exactly once (unit +0x28 = new lo'); a new client room seed is {x, 666}; server room seeds not reachable from the client's first seed within 5,000 steps (observation for client/model.md OQ9).
- 85 DONE: `0x00546C60` result, game +0x80 and S->C 0x03 u32@8 all equal (0xF74A29B4).
- 64 NOT RUN (cargo-mutants not installed); 75 NOT RUN (needs a DirectSound dump); 78 BLOCKED (gibbet needs Tristram, Cairn stones need Scroll of Inifuss progress, the tome is in the Black Marsh); 79 BLOCKED (no save has rescued Cain); 84 BLOCKED (recording `20261006-022633-packets.jsonl` is not on this PC).
- Re-runs on main: d2-server unchanged; `game_monsters::ai_index_of_every_row` now PASS; d2-sim lib ignored 8/8 PASS (debug, no sccache).
- Toolchain finding: rustc 1.99.0 crashes with 0xc0000005 building the d2-sim lib test unit in `--release` (reproducible); debug builds work.

Earlier today (still current): triage re-run (tri-* branches): NEW `game_items::sweep_create_every_item_every_quality` 560 failures "affix 1 does not fit" (item 519 `ibk`); `lvlprest_measurements` 80 vs 82; Cold Plains 97 = 62 preset + 35 outdoor (recorded 61 + 37); WarpDist != 2025: [15: 3800; 20, 21, 23, 25: 100]; seed-finder Den of Evil 27 rooms on every seed (seed-finder.md expected variation).

Recordings (claude/local-buddy-recordings-2026-10-07 19d6be1): kill with drop, Bash, Might (solo), Druid Raven, weather-0001 with rain, Clay Golem (kf empty: not saved), hireling hire from Kashya (no 0x7A on a hire). FINDING F1: S->C 0x3E queued with size 34 while its length byte says 7 (sender 0x53D130) makes check_packets fail whenever 0x3E is queued.

For PC 1 (`specs/tools/original-hooks.md`): ready F0 = 2; refused walk target gives result 1; save path from the registry; seed probe first-draw check impossible (inline step 0x52C2C6); Hell on a save that has not reached it stalls after tick 1.

For review by the coordinator:
- claude/local-buddy-scenarios added `scenario-run export` (tools/scenario-run, Rust) so run_scenario.py reads `.scenario` files.
- claude/spec-audio-buddy: two exe-derived numeric constant TSVs (`audio/object-sounds.tsv`, `audio/npc-speech.tsv`; ids only).
- Act V intro message rows specified in `quests-act5-2.md` §9 but not in quest-messages.tsv (test asserts 779 rows; OQ8).
- `quests-act4.md`: Terror's End timers use GetTickCount; spec uses game time pending a recording (OQ2).
- `world/objects.md`: S->C 0x58 builder missing from server-messages.tsv (OQ12).

## Branches (newest first)

| Branch | SHA | Result |
|---|---|---|
| claude/local-buddy-q-rec-2026-10-07 | f43b65d | entries 69, 70, 74, 80, 83, 85 (above); tools record_join.py, record_sound.py, check_env.py, spawn.py --status objects, record_frames 0.2.1 |
| claude/local-buddy-q-data-2026-10-07 | bab6780 | entries 61-63, 67, 68, 72, 73 (above) |
| claude/local-buddy-q-simlib-2026-10-07 | 9da2c67 | d2-sim lib ignored 8/8; rustc release crash |
| claude/local-buddy-q-server-2026-10-07 | 51d4a27 | entries 59, 60, 65 + re-runs |
| claude/local-buddy-q-saves-2026-10-07 | 521424b | entry 66 PASS |
| claude/local-buddy-q-client-2026-10-07 | 7ac9f8f | entries 71, 76, 77, 81, 82, 86 |
| claude/spec-d2s-buddy | f254d8f | d2s OQ4 (merc) answered on bdMercTwo; Clay Golem writes kf count 0; OQ15 (Iron Golem) open |
| claude/local-buddy-recordings-2026-10-07 | 19d6be1 | recordings pass 1 + 2 |
| claude/local-buddy-stg-formats-2026-10-07 | 2ddf2af | staging 06db38f: game_sweep 9/11, C65 2/2 |
| claude/local-buddy-stg-server-2026-10-07 | cea36e4 | staging 06db38f: world_data 4/5, seed-finder PASS |
| claude/local-buddy-tri-sim-2026-10-07 | c96be5d | triage re-run d2-sim |
| claude/local-buddy-tri-server-2026-10-07 | a5d232a | triage re-run d2-server |
| claude/local-buddy-tri-formats-2026-10-07 | 81531cb | triage re-run mpq-tool formats + game_sweep |
| claude/local-buddy-tri-client-2026-10-07 | 2252a3f | triage re-run game_assets + play |
| claude/local-buddy-lb1-2026-10-07 | b0c1b6a | impl-bitstream-vitals LB1 PASS |
| claude/local-buddy-scenarios | d088adc | run_scenario.py probes 6/6; starters diverge at tick 0 |
| claude/local-buddy-saves-2026-10-07 | ab591fd | make_saves.py, d2ui.py, test saves |
| claude/spec-spawn-hooks-buddy | 5cbbed8 | original-hooks-spawn.md + spawn.py |
| claude/spec-objects-buddy | 9ec4578 | objects.md + object-population.md |
| claude/spec-quests-act5-buddy | 37dd2c6 | quests-act5.md + quests-act5-2.md |
| claude/spec-quests-act4-buddy | e66978f | quests-act4.md |
| claude/spec-quests-act3-buddy | 684d9da | quests-act3.md (level 104 fix) |
| claude/spec-audio-buddy | c3952b9 | audio/triggers.md + environment.md |
| claude/spec-ui-panels-buddy | 4e90b09 | ui/panels.md |
| claude/spec-hirelings-buddy | 3737f86 | world/hirelings.md |
| claude/spec-quests-act2-buddy | 94e83c6 | world/quests-act2.md |
| claude/local-buddy-a-*-2026-10-07 | bcbf89a / 906b19b / 8dde1e5 / effbe83 | morning delta checks (superseded by the tri / q runs) |

## Blocked

- Story-progression recordings: Countess, Cain rescue / gibbet (78, 79), act change, Act III, levels 74 / 120, Cairn stones and the Forgotten Tower tome (78), the cave for 74.
- Might in a party (second player); Kick (no left/right-skill flag); draw-order OQ7 (cells off-screen from every reachable position); Iron Golem save (d2s OQ15); ScnSorNinety; champion-pack (needs a Hell save).
- Recordings only on the developer PC: 84, impl-bitstream-vitals LB2.
- 64 needs cargo-mutants; 75 needs a DirectSound dump tool; LB3 needs a replay test.
- Community 1.14d label file (`../refs/1.14d-notes`) not on this PC.

## Next

Nothing runnable is queued. The loop re-checks main, specs-staging and HANDOFF §5 for new entries and answers spec questions about this PC's specs in place.
