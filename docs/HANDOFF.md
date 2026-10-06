# Handoff (updated 2026-10-06, branch `claude/docs-fold-8` on `claude/tender-meitner-mphas3` at `62fcef6`: the eighth fold (ci-nightly-props, unify-items, mutants-inventory, mutants-combat-skills, mutants-core, s2c-use, spec-unit-numbering, prop-fixes, path-update-pass, prop-walk-gaps, fixedset-game, mpq-huffman, drop-freespot, wire-path-server, game-tests-wired-host, mutants-world, mutants-items-treasure; 17 notes in this fold, 115 folded in all) on top of `claude/docs-fold-7` (the seventh fold: host-merge, host-merge-port, impl-path-core, impl-walk, wire-path-sim, wire-inventory-sim, wire-inventory-server, render-camera-placement, render-wire, conformance-path-render, bench-baselines, prop-client, prop-inventory, prop-path-place, prop-walk, fix-statlist-prop, mutants-server, mutants-monsters-missiles, game-tests-inventory-path, gaps-path-render, gaps-new-specs; 21 notes))

Start here in a fresh session, after `CLAUDE.md` and `docs/METHODS.md`.
This file holds state, the next steps, the code and command map, and the
local run queue. Rules and facts live in specs (`specs/README.md`); this
file points to them rather than restating them. The detailed per-session
records (seams, public APIs, design choices, per-site questions) stay in
`docs/handoff/*.md`; each of the 115 notes (all of them) starts with a
pointer line. The seventh fold took the 21 notes that arrived after the
sixth: the 20 without a pointer line (one of them, `gaps-new-specs`, landed
on the base while this fold was written) plus `bench-baselines`, which carried
a pointer line but had no content here (its numbers are in §1 row 3ad).
Four of the notes (`host-merge`, `wire-path-sim`, `wire-inventory-server`,
`render-wire`) change facts that §1–§3 and §7 stated before: where an older
row says `TradeWorld`, `SimGame::items` / `skills`, `SkillSeams`, "path core
has no code" or J1 / J4 / J12 as open, read the seventh-fold rows (3y–3ae)
and §7 "Resolved in the seventh fold". New notes arrive as the next fold's
input: a note without a pointer line is not folded yet.

The eighth fold took the 17 notes that arrived after the seventh and have no
pointer line (`ci-nightly-props`, `unify-items`, `mutants-inventory`,
`mutants-combat-skills`, `mutants-core`, `s2c-use`, `spec-unit-numbering`,
`prop-fixes`, `path-update-pass`, `prop-walk-gaps`, `fixedset-game`,
`mpq-huffman`, `drop-freespot`, `wire-path-server`, `game-tests-wired-host`,
`mutants-world`, `mutants-items-treasure`; the `local-run-guide` task is
`docs/LOCAL-RUN.md`, which is not a note). Facts that §1–§3 and §7 stated
before change in four places: C→S 0x01–0x04 run on the server (row 3ag; read
"walk / run are stubs" in rows 3k and "3 not implemented" as stale), the host
has one item store and one inventory (row 3af; read `host-merge` W-5 and
`wire-inventory-server` F1 / F2 as closed), the treasure drop uses the path
provider's floor drop when the provider and its field are on (WP2 closed), and
`d2-sim` keeps no dependency on `d2-proto` (decision, `docs/PLAN.md`; step 7k).
The specs `ui/text.md`, `render/draw-order.md`, `render/unit-composite.md`
(with `unit-directions.tsv`) and the render follow-ups landed from the local
session; the implementation sessions `impl-ui-text`, `impl-draw-order`,
`impl-unit-composite` and `render-followups-impl` are running (row 6, "specs
landed"). Coverage at this fold (`py tools/coverage.py --summary`): 4,194
claims over 3,339 rule units, unit 2,858 (85.6%), game-file 197 (5.9%), trace
30 (0.9%), verified 227 (6.8%), any tier 2,911 (87.2%).

Index (this file is long; read by section, METHODS M11): §1 State ·
§2 Next steps · §3 Code map (Phase 0–2 crates, tick / server, Phase 3
systems, wiring, Phase 6 client, tools) · §4 Command map · §5 Local run
queue (Done, A player, B Ghidra / spec edits, C game files and GPU,
Blocked) · §6 Environment · §7 Where facts live and open questions
(carried-over 1–8, tick T1–T5, tick replay TR1–TR4, parser R1–R2, Phase 3
systems by owner spec incl. the second set, wiring W1–W16 / WE1–WE9 and
the third set: server handlers SI1–SI4 / SK1–SK9, worldgen WG1–WG9,
interaction WI1–WI10, gap-test questions GI / GC / GD / GW, integration
J1–J6; fourth set: J7–J8, QH1–QH2, EN1, GD8–GD11, GT1, GP1, GB1; fifth set:
J9–J10, RT1–RT6, PW1–PW2, VM1–VM2, DQ1, FZ1–FZ2, CG1–CG2; sixth set:
IV1–IV8, MV1–MV7, PP1–PP7, SC1–SC4, PB1, CH1–CH4, PS1–PS4, PK1–PK3,
FS1–FS4, GA1–GA5, GS1–GS6, GM1–GM6, RN1–RN3, CP1–CP4, PI1–PI3,
WV1–WV3, EC1–EC4, EV1–EV4, WO1–WO3, J11–J13; seventh set: HM1–HM7,
PC1–PC5, PF1, PQ1–PQ9, GR1, PX1, WP1–WP5, GX1–GX4, CR1–CR3, RC1–RC6,
RW1–RW5, WN1–WN4, IS1–IS6, PN1, KP1–KP2, MS1, MM1–MM3, BB1, SF1, plus
"Resolved in the seventh fold"; eighth set: UI1–UI7, DF1–DF3, WS1–WS5,
PU1–PU2, SL1–SL4, PG1–PG2, FG1–FG2, GH1–GH7, MI1–MI3, MW1–MW3, MT1–MT2,
MK1–MK2, MC1, MH1, S2C1–S2C2, SU1, NP1, plus "Resolved in the eighth fold";
Phase 6 by owner spec) ·
§8 Lessons.

## 1. State

Facts per phase. "Unverified" = implemented from a draft spec whose
queued check (§5) has not run (METHODS M02); the unit tests only prove
the spec's synthetic vectors. Test counts below are per branch, as each
note reported them; no session has recorded a `cargo test` run on the
merged tree (the coordinator's gate runs it).

| Phase | Status | Proof |
|---|---|---|
| 0 Setup | done | CI green on GitHub (`MoggerCat/D2RUST-Project`) |
| 1 Formats | done | `mpq-tool check`, `mpq-tool formats` |
| 1b First pixels | done | `d2-client verify` (GPU = CPU reference, byte-exact) |
| 2 Data | done | `data-tool tables` (2026-10-05, with callbacks): 73 live tables, 72 byte-identical, 1 explained (`monstats` `NameStr`); 4/4 code buffers identical; `data-tool links`: 0 broken; `data-tool dump-compare traces/raw/20261006-021210-tables`: 70/70 tables and every map identical to 1.14d memory; `d2-data` game-file tests all pass (incl. `fixups_on_live_set`, `typed_tables_decode`, patch G1–G8) |
| 3a Tick core, unit order | **conformance-passing** (`sim/tick.md`, `sim/unit-order.md`): `d2-sim::tick`, `units::lists`, `game` | `cargo test -p conformance --test tick_replay`: `traces/sim/tick/sim-0006..0008` (11,105 ticks, 65,754 inputs), all 48,316 timer runs in order and all 446 list snapshots equal, 0 mismatches; 4 perturbation tests report exactly the changed record. RNG: all 256 draws of `traces/sim/rng/*.json` replay exactly (`sim/rng.md` `conformance-passing`). `check_tick.py` / `check_packets.py` on the 2026-10-06 recordings: 0 mismatches |
| 3b Messages + server host | `d2-proto` tables generated from the two TSVs; `d2-server` transport + host loop wired to `d2-proto` / `d2-sim` through adapters; intent handlers are stubs (return 0, log) except the item / cube, skill / combat and world ids of row 3k; unverified on recordings | `cargo test -p d2-proto`: generated tables equal the TSVs, spec size/classifier/layout vectors; `cargo test -p d2-server`: 43 tests (every synthetic vector of `intents-events.md` §1–§3, `tick.md` §1; one single-player host frame on the real adapters); `check_units.py`: 0 errors on all three recordings (14,034 schedules); `convert_tick.py --check traces/sim/tick/*.json`: 0 errors (CI) |
| 3c Units, stats, stat lists | `d2_sim::{units,stats}` implemented from draft specs, **unverified** (units spec confirmed on recordings for the event rules only; stats / stat lists await `record_stats.py`) | 115 d2-sim tests on the branch: every synthetic vector of `stats.md`, `stat-lists.md`, `units.md` §4.2; TSV checks with perturbation tests (`stat-ops.tsv`, `unit-handlers.tsv`, `unit-events.tsv`) |
| 3d Items, treasure | `d2_sim::{items,treasure}` implemented, **unverified** | items: 57 tests (113 in d2-sim on the branch); treasure: 48 tests (104 in d2-sim); both `property-functions.tsv` / `treasure-*.tsv` checked with perturbation tests |
| 3e Combat, skills | `d2_sim::{combat,skills}` implemented, **unverified** | 128 d2-sim tests + 3 ignored game-file tests (not run); `skillcalc.tsv` / `misscalc.tsv` checked with a perturbation test |
| 3f DRLG | `d2_sim::drlg` (levels + rooms) and the three level types implemented, **unverified**: `drlg::preset` (DrlgType 2), `drlg::maze` (DrlgType 1), `drlg::outdoor` (DrlgType 3: Act I and `outdoor-tilesub.md` in full, Acts II and IV as specified, Act III partial, Act V only the siege strip). The dispatcher is `wiring::worldgen::WorldTypes` (3l) and the DS1 / DT1 providers from the user's files are `d2_server::world_data` (3o); no level from the live tables has been generated yet (§5 C23) | 48 drlg tests (104 in d2-sim) on fakes; preset 26 tests (537 in d2-sim on its branch), maze 26 (538), outdoor 41 (553). Recorded vectors reproduced on synthetic inputs with the recorded seeds: Act I placement (start seed 4014346869, rects, DRLG seed after creation), Blood Moor room sub-theme pick, the maze Den of Evil and Spider Cavern vectors |
| 3g Missiles, monster AI, monster population and init | `d2_sim::missiles`, `d2_sim::monsters::ai` implemented, **unverified**; 17 AI functions of 148 have bodies, server-do index 1 only, no server-hit body (stubs log). `d2_sim::monsters::population` (regions, room population, pick, bosses and packs, spawn point, placement search, presets, ambient spawns) and `d2_sim::monsters::init` (creation after placement, type init, stats and skills, level, mods §14.1 full, §14.2 bloodraven only, umods §16–§22, 0xAC init-owned fields) implemented, **unverified**: no spawn RNG trace exists | 76 new tests (131 in d2-sim); `catalogues_match_tsv`, `ai_table_matches_tsv`, `implemented_matches_catalogue` with perturbation tests; population 35 tests (547 in d2-sim on its branch, `SPECIAL_PRESETS` TSV check with perturbation), init 39 + 1 ignored (`real_level_stats`; 551 in d2-sim on its branch, `umods_match_tsv` with perturbation) |
| 3h World (quests, waypoints, cube, NPC, vendors) | `d2_sim::world` (quests, waypoints, cube) implemented, **unverified**; Act II–V quest callbacks and several Act I ones are `unhandled`. `world::npc` (interaction, chat, menu, heal, hire, resurrect, services, act travel) and `world::vendors` (price, store generation, gamble, buy / sell / repair) implemented, **unverified**: no recording of hire, resurrect, heal, identify or services exists | 78 new tests (134 in d2-sim); TSV checks with perturbation tests; npc 33 + 1 ignored (`live_monstats_records`; 544 in d2-sim on its branch), vendors 60 (572; every synthetic vector, the three recorded prices, the recorded Charsi store order of 43 codes, the recorded 0x32 / 0x33 bytes, `vendors.tsv` check with perturbation) |
| 3i Skill use, vitals | `d2_sim::skills::use_` (validators and handlers, use at point / on unit, mode gate, skill start / do, delay, periodic and aura events, 0x3C select) and `d2_sim::combat::vitals` (player creation stats, 0x3A stat points, level-up, experience level factor, `add_experience` as far as creation needs) implemented, **unverified**; the per-skill bodies (`srvst` 64 and `srvdo` 152 filled slots of `table::FUNCS`) are the seam `SkillFunctions`; the kill experience gain is not implemented | 28 + 12 tests (551 pass, 3 ignored in d2-sim on its branch); every vector of `use.md` and `vitals.md`; `FUNCS` checked row by row against `functions.tsv` with perturbation |
| 3j Wiring | `d2_sim::wiring::economy` (items, treasure, cube and quest item seams on real `Economy`, `StatLists`, `UnitRecord`) and `d2_sim::wiring::action` (`ActionSim`: the combined `EventDispatch` + `TickHooks`; combat, missile, AI, DRLG room, waypoint seams) wired, **unverified**: the adapters add no rule of their own; every seam call without a provider is `Pending` / `…Rest` with the narrowest default | economy 17 integration tests (512 pass, 3 ignored in d2-sim on its branch, game seed `0x5EED`), action 19 (514 pass, 3 ignored; `cargo test -p conformance` tick replay unchanged); neither branch's count is a merged count |
| 3k Server intent handlers | **Eighth fold: walk / run 0x01–0x04 now run on the path provider (row 3ag) and the item handlers' stores are one (row 3af); read "walk / run … still stubs" below as stale.** **Seventh fold: the host is one value (`world: W`, row 3y): read `TradeWorld` as `WiredWorld`, `SimGame::items` / `skills` as the host's `cube` / `skill`; the item moves 0x16–0x29, 0x50, 0x61, 0x63 now run (row 3z); walk / run 0x01–0x04 are still stubs.** `d2_server::adapters::handlers::{items, skills, world}`, called from `SimGame::handle` in the order items, world, skills, then the stub; **implemented, unverified** (every owner spec is a draft; no recording replayed through them). Items: C→S 0x2A and 0x4F (the cube's buttons) run `world::cube` through `wiring::economy::EconomyCube`; the other item ids (0x16–0x29, 0x50, 0x61, 0x63) have an owner spec and code since the sixth fold (`items::moves`, 3r) but no adapter calls it, so they stay stubs (0x4C: `world/cube.md` §10). Skills / combat: 0x05–0x11 (skill at point / on unit, hold forms, 0x0B), 0x3A, 0x3B, 0x3C run `skills::use_`, `skills::levels`, `combat::vitals` on `wiring::action::ActionSim`; 0x12, 0x41, 0x51 and walk / run 0x01–0x04 stay stubs. World: 0x13 (NPC), 0x2F, 0x30, 0x31, 0x32–0x38, 0x40, 0x58, 0x62 and 0x49 reach `d2_sim::world`, but on `ActionWorld` **only 0x49 runs on a real provider** (waypoints); NPC / vendor / quest ids stay stubs on `ActionWorld` and `NoWorld` (tested on a seam fake with the real `NpcControl`, trade functions and `QuestControl`). **On `TradeWorld<R>` (`handlers::world::trade`, `e2e-vendor-host`) the quest ids run too** (`quest-host`, 2026-10-06, unverified): `WorldHost::quests` runs the real `QuestControl` on `wiring::economy::EconomyQuests` built from the interaction `Desk`'s economy and rest; C→S 0x31, 0x40, 0x58 leave their stubs there; the Kashya mercenary reward (`0x00579180`) is collected during the quest call and run right after it as `NpcControl::quest_mercenary` before the result goes back, so `QuestRest::mercenary_reward` is not reached on that host; no Act II–V callback is raised, the `unhandled` calls the tests reach are Act I gaps (§2 step 7c); 0x3E, 0x3F, 0x44, 0x46, 0x47, 0x4D, 0x59 are stubs. The item and skill handlers are inert until `SimGame::items` / `SimGame::skills` is set (default `None`). S→C: only 0x77 (cube) has known bytes in the handlers; 0x15 and 0x5A are recorded, not sent; typed builders for 16 more ids exist in `d2_proto::s2c` and byte builders for 0x19, 0x1D–0x1F, 0x3F, 0x42, 0x47, 0x48, 0x7D, 0x9C, 0x9D in `items::moves::layouts` (3r, 3t), none called from a handler | per branch (each from `claude/bold-ptolemy-jvyvxy` at `1470723`, none merged-count): d2-server 53 (items, 10 new), 54 (skills, 11 new), 68 (world, 25 new); d2-sim 828 pass, 5 ignored on each. M08: ground range 11 fails `item_to_cube_ground_item`; cutting the sent 0x77 to one byte fails both byte tests. `ids_match_client_tsv` (skills) and `world_ids_are_sim_handlers` (world) check the id tables against `client-messages.tsv` with perturbation; `ITEM_IDS` has a test. On the wired sim a mode start fails after the mode set (no AnimData record routed), so no skill do runs from a message (§7 SK1). Quest host: `cargo test -p d2-server --lib trade_quests` 7 pass on `SimGame<ActionSim, TradeWorld>` through the host frame (Akara 0x31 message 64 → S→C 0x27 + 0x29, record diff = slot 1 bit 2 only, chain 1 state 2, chat end `5d 01 00 01 0000`; Kashya message 92 → `50 0200 <name> 00×10`; 0x40 → `28 06 …` + `52`; 0x58 sets the log bit; same seed twice identical). M08: one flag bit changes exactly one byte of 0x28 (byte 13, xor 0x04); removing `mercenaries` from `TradeWorld::quests` fails the Kashya test. Staged answers, not behavior: 0x27 bytes 6–39 zero, mercenary spawn fails, unit act 0 |
| 3l Wiring, second set | `d2_sim::wiring::worldgen` (`WorldSim` = `ActionSim` + `WorldState`: act DRLGs through one `LevelTypes` dispatching to Maze / Presets / Outdoor; population and init on real rooms and regions) and `wiring::interaction` (`Desk`: NPC, vendor and quest seams on real units, stat lists and store; `UseView`: skill use; `VitalsView`: vitals) wired, **unverified**: the adapters add no rule of their own. With the default `WorldPending` **no room population (`population.md` §3) runs, only presets place monsters**: the DRLG data population reads (coordinate lists `0x0061AD50` / `0x0061AD30` / `0x0061B130`, populated level `0x0061A1F0`, populated-room count `0x0061ABF0`, warp points `0x0061AC10`, level spawn of kind 11, nearest free point `0x0064E840`) are in no DRLG spec. Not routed at that point (routed since in 3n, except combat umod mode 4, the AI state, timer type 14 and the kill → `kill_experience`): monster event 7 and the umod callbacks, `init_kind` for monsters allocated by action code, skill-use tick events 5, 8, 9, 14. `wiring::interaction` makes one module change (`NpcLink::make_hire_list` takes the NPC-control seed, §3 Seams) | worldgen 12 integration tests (d2-sim 840 pass, 5 ignored; `tick_replay` 7 pass unchanged; `cargo check -p d2-server` builds); fixture act 0 with init seed 644409375 (`dwStartSeed` 4014346869), game seed {1234, 666}; the recorded Act I placement vector through the dispatcher (DRLG seed {1406222081, 1674353446}, allocation order 4, 3, 2, 1, 17, 39, 26, 7, 6, 27, 5), the Den of Evil maze vector, preset room streaming, population and init on the real rooms, two identical runs of 10 ticks. interaction 11 tests (d2-sim 839 pass, 5 ignored; `tick_replay` 7, `tick_traces` 2, `rng_traces` 2): real gold paid, store generation with the game seed stepped exactly twice, hire list on the continued NPC seed, a skill missile hitting a real monster, cooldown list and its type-12 timers, a kill granting a level-up. Neither count is merged |
| 3m Gap tests | eight sessions of unit tests from the specs, no game files, unit tier only (the first four: `gaps-items-stats`: 140 tests, 7 specs; `gaps-combat-ai`: ai, missiles, damage; `gaps-data-formats`: data, formats, rng; `gaps-drlg-world`: drlg, world, tick, unit-order, intents-events; the second four, 2026-10-06: `gaps-drlg-sim-world` (`outdoor.md` §2.4 linkers, `unit-order.md` §1 r4 / r5: 65/72 and 38/38), `gaps-combat-items-monsters` (24 tests: `treasure.md` 62 → 77 of 85, `hit.md` 41 → 46 of 49, `levels.md` 40 → 43 of 47, `population.md` 156 → 158 of 165), `gaps-data-rng` (14 `d2-data` tests: `callbacks.md` 41/41, `runtime-maps.md` 25/25, `fixups.md` 34/36, `loading.md` 33/47; `tools/coverage.py` accepts the claim `r0`, which enabled `vendors.md §9.2 r0` and `affixes.md §5 r0`; `sim/rng.md` gets no claim, its 7 open units are cross-reference tables, §7 GD11), `gaps-client-formats` (18 units: `bridge.md` 22 → 33 of 39, `map-preview.md` 11 → 13, `ui.md` 8 → 9, `assets.md` 7 → 8, `cof.md`, `dcc.md`, `ds1.md`; one is a game-tier claim on an `#[ignore]` test); the data-formats session also wrote game-tier claims on `#[ignore]` tests that have never run (§5 C17). Code fixes: `units.md §3.1 r7` (`units/lifecycle.rs`: allocation set `mode := arg` for tiles too; a tile keeps mode 0); `quality.md` edge-case r4 (`items/tables.rs`: unique rarity read as 32 bits at `+0x30`, `UniqueRec::from_record(r, raw)`; no change on 1.14d data). Behavior-neutral: `rng::Seed` is `#[repr(C)]` (`rng.md` §1 r1); `d2-data::bin::check_server_files` split out of `load()` (`loading.md` §3.3). **Deviation found, not fixed**: `units.md §5 r4` (an event scheduled through `0x005416B0` with expire −1 becomes every-tick and loses its callback) vs `TimerQueue::schedule` (`tick/timer.rs`), which passes the callback to `schedule_every_tick`; `tick.md` §5.2 does not mention dropping it (owner: tick module, after reconciling the two specs, §7 GI). The second four found no code deviation; they found spec / code or spec / TSV disagreements that stay unfixed (§7 GD8–GD11, GT1, GP1, GB1) and no non-test code changed except three `#[cfg(test)] mod` lines and `pub(super)` on test fixtures | M08 by hand: items 13 + 7 breaks and stats 12 each failed a new test (uncaught mutants unobservable, except the MulDiv 32-bit product, which got `muldiv(-0x20_0000, 0x1_0000, 3) == 0`); ai, missiles, damage breaks each failed the matching test. Gates `cargo test -p d2-data -p d2-formats -p d2-sim` pass on the data-formats branch; `coverage.py --check` 0 errors. Second four: each claimed test failed when its rule was broken by hand (treasure 15 breaks, hit / levels 10, population 2, linkers 2; M08); `cargo test -p d2-sim` 1,230 pass, 5 ignored on `gaps-combat-items-monsters`, 1,208 on `gaps-drlg-sim-world` (per branch); the `r0` selftest fails on the old grammar |
| 3n Wiring, routes (`wire-routing`) | the open event and callback paths of 3j / 3l routed through the world state, **unverified** (the routes add no rule of their own): `WorldSim` **lends** its `WorldState` into `ActionHooks::monster_world` around every timer event and forwarded tick hook (`WorldSim::lend`); with the world lent, `init_kind` runs the monster type init (`init.md` §5) for monsters allocated by action code, event 7 (`monster_umod`) runs `init::handle_event7`, a monster mode change runs the umod dispatcher in modes 0 then 1 (`0x005A7C20`), the combat hit hook mode 3, missile creation mode 5, `free_kind` calls `WorldState::forget` (monster data, minion / owner maps), and `monster_flag` (`0x005A0180`), `is_unique` / `is_champion` and the monster level (stat 12) read the monster data. Without a lent world every route answers its previous `Pending` (`ActionSim` alone, d2-server tests and the conformance replay behave as before). **Not routed (no spec):** umod mode 4 (`0x005A43A0`), AI state (`dwAiState`), `is_boss` / superunique / minion owner getters, timer type 14 and the delay list's remove callback; skill-use timer types 5, 8, 9, 12 were already routed (`wire-open-seams`, `e2e-combat-path`). `WorldHooks` (the delegating unit hooks of `wire-open-seams`) is removed | 7 routing tests (`wiring/worldgen/tests/routing.rs`), each with its no-world twin (M08; by hand, cutting one route fails only its test); d2-sim 1,213 pass, 5 ignored; workspace 1,873 pass, 0 fail, 61 ignored on the branch; `tick_replay` 7 pass unchanged |
| 3o DRLG data from the user's files (`drlg-data`) | `d2_server::world_data`: `Ds1Files: Ds1Source`, `SubFileMap` / `WorldFiles: SubFiles`, `Dt1Files: TileSource`, `LevelTables::from_fixed(&FixedSet)` (the four level-type table views), `archive::load(&ArchiveSet)` (`bin::load` → `fixup::apply` → tables → every named DS1 / DT1, up front; a missing or unparsable named file is an error); no DRLG rule added (the §5.2–§5.3 parser rules stay in `Ds1File::from_input`); DS1 v < 7 refused (`OldOrientations`); implemented, **unverified** (live check queued, §5 C23) | 8 CI tests on synthetic DS1 / DT1 bytes (d2-server 107 pass, 3 ignored); 3 `#[ignore]` game-file tests (`world_data/tests/game.rs`: Act I placement vector, Den of Evil maze vector, outdoor generation through the dispatcher), never run. Still seams: `LevelTypes::warp_unit` `0x0066E1C0`, the type-2 tile free `0x00666610`, the DRLG population reads (§5 B), `DrlgData::wall_remap` `None` and `doors` empty |
| 3p Test infrastructure (`synthetic-data`, `fuzz-data`, `ci-speed`) | **Synthetic install:** `d2_formats::mpq::writer` (`MpqWriter`, behind feature `test-support`) writes format-0 archives `Archive` reads (encrypted tables, PKWARE DCL under COMPRESS / IMPLODE, FIX_KEY, SINGLE_UNIT, SECTOR_CRC, listfile); `crates/test-fixtures` generates a made-up table set (headers from `fields.tsv`, rows written from scratch, no Blizzard value), `.tbl`, `AnimData.d2`, and builds a whole install (`install::build`: MPQs → `compile_all` → `.bin` back into `patch_d2.mpq` → `bin::load`); the load path MPQ → `.txt` / `.bin` → typed tables → server tables now runs in CI. Not done: a running game from these tables (no `SimGame` / `ActionSim` constructor from a `FixedSet`; no DS1 / DT1 writer). **Property tests (fuzz-data):** `d2-data/tests/prop_data.rs` (10), `d2-formats/tests/prop_more_formats.rs` (8), `d2-sim/tests/prop_tables.rs` (3); **one bug found and fixed:** `itemtypes` / `montype` `equiv1` link cycles made `fixup::apply` / `maps::equiv_matrix` hang (budget n² × 128 pops, then a `FixupError`; the 1.14d tables are unchanged). **CI and gate (ci-speed):** five parallel jobs and `sh tools/gate.sh` (§4) | `cargo test -p test-fixtures` (3 unit + `server_tables` 4 + `synthetic_load` 8; M08 perturbations: one cell → exactly one byte, a short `inventory` → `LoadError::Check`, a missing hcIdx → load error), `cargo test -p d2-formats` 164 pass, 1 ignored (writer: 11 round-trip tests + 2 property tests); `compare_sets` 73/73 identical on the synthetic set; hunting runs `PROPTEST_CASES` 4,000 / 20,000 / 3,000 passed after the fix; gate summary of `ci-speed` all PASS (estimates only for CI wall time) |
| 3q Game-file tests, not run (`game-tests-drlg-world`, `game-tests-items-treasure`) | 42 `#[ignore]` tests reading `D2_GAME_DIR`: `d2-sim/tests/game_world.rs` (10: waypoints, quests, cube, npc, vendors), `game_drlg_tables.rs` (8: leveldefs, lvlprest, lvlmaze, lvlsub, Act I placement, preset file choice), `game_treasure.rs` (16), `game_items.rs` (8) with the shared loader `items_treasure_live/mod.rs`. **Every expected value is blind-written** (spec numbers, not observed; the group C memory dump confirms only the treasure counts of §5 Done), so no test carries a `// Covers:` claim yet: the planned claims are in the notes' tables (13 rule units for items / treasure; `waypoints.md` §1, §5, §7; `npc.md` §1.1; `vendors.md` §9.3; `lvlprest` / `lvlmaze` rows; `levels.md` §3–§4; `preset.md` §3.1 r3) and are added after the first passing local run | compile, clippy clean, ignored in CI; no run (§5 C34, C35). Gate on the branches: coverage 3,259 claims after the merge, 0 errors |
| 3r Inventory and item moves (`impl-inventory`, `impl-moves`) | **Seventh fold: wired to the host and served by `d2-server` (row 3z).** `d2_sim::items::inventory` (`inventory.md` §1–§5: grids and the `Inventory` item list, page / body / belt placement and compaction, equip checks and requirements, auto-equip location, the six item checks, busy / trading / targeting reset / move gate) and `d2_sim::items::moves` (§6–§11: the 0x16–0x29, 0x50, 0x61, 0x63 handlers, `ITEM_ACTIONS` dispatch table, per-client update pass, ground pickup / drop, gold, byte builders for the S→C 0x19, 0x1D–0x1F, 0x3F, 0x42, 0x47, 0x48, 0x7D, 0x9C, 0x9D) implemented, **unverified**: the spec is a draft, no recording R1–R6 exists. **The two modules are not connected**: `moves` reaches the inventory through its own seam `InventoryOps` (no provider), `inventory` through `InvWorld`; no adapter on `d2-server` calls `items::moves::handle` (the item ids stay stubs on the host, row 3k). Open spec points: `inventory.md` OQ1 (item bit stream of 0x9C / 0x9D), OQ6, OQ9–OQ17, OQ19, §7 IV1–IV8, MV1–MV7 | inventory 33 tests (32 + 1 ignored `real_grid_belt_and_type_tables`, §5 C36; 1,269 pass in d2-sim after the base merge); moves 59 tests (1,291 pass, 5 ignored on its branch): vectors G1–G3, X1, edge cases, every handler's validation order and result codes, `item_actions_match_tsv`, `layouts_match_server_tsv`, `handled_ids_match_client_tsv` with perturbation tests; `inventory.md` 139 of 148 rule units claimed (unit tier; the 9 unclaimed units are named in §1 of the two notes) |
| 3s Path placement (`impl-path-place`) | **Seventh fold: the path core and the walk now exist and the callers are wired (row 3aa); `path/mod.rs` declares the whole module.** `d2_sim::path::{search, place, warp, place_seams}` (`sim/path-placement.md` §7–§12): `nearest_free_point` (`0x0064DEA0`) and its wrappers `free_point` / `free_point_step` / `free_point_field`, the `ExpField` walk-back (`0x0066A670`, from `ExpField.D2`), `coarse_free_box` (`0x0064E840`), `floor_drop`, `place_unit` (`0x00554EA0`), `level_spawn_point`, `game_entry`, `level_warp_place`, `warp_tile_preset` (`0x0066E1C0`), `warp_player` (`0x005550B0`) implemented, **unverified**: no position trace exists (spec OQ1). The collision model (`CollisionView`, `path-placement.md` §1–§6, `impl-path-core`) and the walk (`sim/pathing.md`, `impl-walk`) are **not in this tree**: `path/mod.rs` declares only these four modules, the provider traits `CollisionView` / `PlaceHost` / `LevelView` / `WarpTileView` have no implementor, and `pathing.md` (125 units) has no code and no claim | 25 tests + 1 ignored (`expfield_live`, §5 C38) in `path::`: vectors P1, P1b, P2, P2b, P3, P4, P5, P5b, D1–D3 exact, F1–F3 on a synthetic field; `field_tables_match_tsv` (`path-tables.tsv`) with a perturbation; `path-placement.md` 43 of 84 rule units claimed |
| 3t Server → client builders (`s2c-builders`, `proto-bits`) | `d2_proto::s2c`: one typed builder per S→C message whose full layout a spec gives (`PlayerStop` 0x0D, `QuestInfo` 0x28, `GameQuestInfo` 0x29, `NpcTransaction` 0x2A, `MercForHire` 0x4E, `QuestSpecial` 0x50 quest form, `QuestLogInfo` 0x52, `OpenUi` 0x58, `QuestItemState` 0x5D, `WaypointMenu` 0x63, `TradeAction` 0x77, `UniqueEvent` 0x89, `NpcWantsInteract` 0x8A, `NpcGossipAct` 0x91, `Unknown9B`, `WardenRequest`), the trait `ServerMsg` (`CONSTS` checked by `decode`, `UNWRITTEN` bytes written 0 and ignored), a client-side `parse(&[u8]) -> Message` (`ParseError::{WrongSize, Incomplete, Invalid, WrongId, Const, Unbuilt}`), and `AUDIT` / `audit(id)` for every id: 15 built, 30 generated (TSV layout), 20 partial, 78 unspecified, 38 never (181 ids). `FieldType::Packed { bit, width }` and the TSV `bits:` layout (`packed_get` / `packed_put`, widths 1–32, S→C only): 0x96 `WalkVerify` is a typed struct, and 0x0D / 0x0F / 0x10 / 0x15 got typed structs from layouts already in the TSV (`generated.rs` regenerated twice; no `PROTOCOL_VERSION` bump). Implemented, **unverified**: the vectors are the recorded messages the specs quote, not a recording replay. `d2-sim` keeps its own byte builders (`world::npc::{transaction, service_result, resurrect_message}`, waypoints 0x63, cube 0x77, the 0x0D of the waypoint travel); the bridge's `bridge-dispatch.tsv` is still all `TBD` | `cargo test -p d2-proto`: 22 + 8 pass on the s2c branch (11 new `s2c` tests: `sizes_match_tsv`, `layouts_cover_every_byte`, `audit_matches_tsv_parser_and_types`, `note_table_matches_audit`, recorded 0x63 / 0x0D / 0x07 / 0x2A / 0x28 / 0x5D / 0x8A / 0x9B / 0x77 bytes, single-byte-flip perturbations, round trips); `bits_layout_0x96_bit_ranges`, `bits_layout_strict_errors`, `bits_0x96_round_trip` (bytes `96 BC 5A 1A 89 77 DF 3F 40`); no coverage claims |
| 3u Conformance harnesses for coming recordings (`conformance-harness`) | `crates/conformance`: `units::replay_anim` (`tick-raw-1`, `record_tick.py` 0.2.0 `anim` records through `units::anim::schedule`), `stats::replay_stats` (`stats-raw-1` through `StatLists` and `stat_ops`), `packets::replay_packets` (`packets-raw-1` through a `PacketServer`; `DispatchServer` on `d2_server::dispatch` + `Tick`), `rooms::replay_rooms` (format-1 tick traces; the `RoomModel` has no d2-sim provider), over `raw::RawRecording` (header `format` checked) and `raw::Mismatch { source, at, field, detail }` (first difference, numbered as the Python checker numbers it). Fixtures hand-built from the checkers' own `synthetic()` recordings; **no harness has run on a 1.14d recording**, so no `Covers:` claim (a `conformance` claim is trace tier) | 36 pass, 3 ignored (`units_replay`, `stats_replay`, `packets_replay` recording tests: §5 A "Additions to the units / stats / packets recordings" and C39); the rooms test runs in CI on the committed traces (446 snapshots, 5,808 arrays read, rooms = replayed active set; arrays not compared); each fixture passes its Python checker |
| 3v Spec-body audits (`spec-bodies-ai-missiles`, `spec-bodies-quests-skills`) | audit of every AI (148), server-do (53), server-hit (71) row and of every quest callback reported `unhandled` and every `SkillFunctions` slot: **no new body was implementable exactly** (the spec text gives full rules only for the 17 AI functions and server-do 1 + helper `0x005A9820` already implemented). One body added: `srvst` 18 Attract `0x005C3260` (returns 1; `skills::use_::bodies::start`, `START_BODIES` = [18], `DO_BODIES` empty; the cores call a body first, the `SkillFunctions` seam only on `None`). Everything else stays a logged stub; the per-entry "missing spec text" lists are the notes' §3 (§2 step 11) | `missiles::tests_bodies` (`SRV_DO_IMPLEMENTED` / `SRV_HIT_IMPLEMENTED` = the non-null `spec'd-here` TSV rows, perturbation reported exactly; claim `missiles.md` §R9.2); `skills::use_::tests::bodies` 4 tests (`bodies_match_tsv_notes`, `bodies_check_reports_perturbations`, two Attract tests; claims `use.md` §8, §5.3 r6); d2-sim 1,234 / 1,236 pass on the two branches |
| 3w Property and game-file tests, sixth fold (`fuzz-server`, `prop-sim-core`, `prop-worldsim`, `game-tests-*`) | **Property tests:** `fuzz-server`: `d2-proto/tests/prop_messages.rs`, `d2-server/tests/{prop_transport,prop_handle}.rs`, `d2-client/tests/prop_bridge.rs` (sizes, classifier, split, typed decode / encode; dispatch code against a model of `intents-events.md` §2.3 / §2.4 built from the TSV columns; every game id 0x01–0x66 on an action, item and trade host; bridge routing); one bug fixed (`Bridge::send_bytes` of a system message over 0x204 bytes: `IntentError::TooLarge`). `prop-sim-core`: `d2-sim/tests/prop_{rng,timer,lists,units}.rs` and `stats/prop_tests.rs` against reference models written from the specs; **seven bugs fixed** (§7 PS1–PS4 are the spec observations): `UnitLists::hash_bucket(_, ≥ 128)` panic, `free_room` left units linked, `anim::schedule` overflow and endless loop (`AnimError::Endless`), `lifecycle::allocate` left a trace on failure, `StatLists::detach` panic on a parked child of a freed list, `fraction_changed` overflow. `prop-worldsim`: `d2-client/tests/prop_worldsim.rs` (`SimGame<WorldSim<TestPending>, TradeWorld<Rest>>`, 40–80 operations then 100 quiet ticks; no panic, bounds, the dispatcher's rejections leave the digest unchanged, same seed → identical bytes and digest, other seed → other digest): no bug. **Game-file tests, none run:** `game-tests-sim-core` 15 (`d2-sim/tests/game_core.rs` 12, `d2-server/tests/game_world_data.rs` 3), `game-tests-monsters-skills` 35 (`game_monsters.rs` 19, `game_skills.rs` 16 + shared `game_common/mod.rs`), `game-tests-client-assets` 17 (`d2-formats/tests/game_sweep.rs` 11, `d2-client/tests/game_assets.rs` 6): 67 `#[ignore]` tests, every expected value from a spec, none observed; no `Covers:` claim yet (each carries an `Intended claim` line or, for monsters / skills, the claim table of the note's §4); with the 42 of row 3q there are 109 | `cargo test -p d2-sim -p conformance`: 1,264 pass, 0 failed, 6 ignored on the prop-sim-core branch; `prop_worldsim` ≈ 12 s at the defaults, 150-case hunt (215 s) clean; fuzz-server hunts at 20,000 / 5,000 / 2,000–6,000 / 3,000 cases clean; the game-file tests compile and pass clippy only |
| 3x End-to-end hosts and open seams, folded late (`wire-open-seams`, `e2e-single-player`, `e2e-combat-path`, `e2e-vendor-host`) | **Wiring seams closed** (`wire-open-seams`, `wiring` only, unverified, adapters add no rule): timer events 5 / 8 / 9 → `Pending::skill_event` → `interaction::skill_events::route` → `use_::{active_state_event, periodic_event, item_aura_event}`; monster event 7 → `init::handle_event7` (later `WorldHooks` was removed in the fifth fold, 3n); removal frees the world state (`WorldState::forget`); tick step 8 quests (`economy::QuestTick` wraps any `TickHooks + UnitSide`, `update_quests`); Kashya's mercenary reward via `Desk::quest_message` (0x31); regeneration through `tick::tick`. **Single-player e2e** (`d2-client/tests/e2e_single_player.rs`, 3 tests): `Bridge` → `LocalLink` → `Host` → `SimGame<WorldSim<_>, ActionWorld>` + `WiredSkills`, act 0 on the recorded Act I seed 644409375, game seed 1234; steps 1–3 (creation, small preset level, population and join) run, step 4 (cast → missile → hit → kill → 100 experience) and 5a (drop of the kill) run since `e2e-combat-path`, 5b (pick-up) is a stub, 5c (buy / sell) runs on `TradeWorld` (`e2e_vendor.rs`, 4 tests) up to the stubs listed in §2 step 7c, 6 (waypoint travel) stops at the same-act placement (no S→C 0x0D). Fixes outside the test: `SimGame::tick` now passes the dispatch as the `TickHooks` (`Tick` needs `D: TickHooks`: room activation, removal, level free and the room pass now run through the host), and `handlers::world::ActionEvents` (`ActionSim` and `WorldSim`) so waypoint and skill handlers work on a game with population. **Combat path** (`e2e-combat-path`): `UnitHooks::{anim_record, anim_rate, frame_bonus, player_action_frame, monster_mode_function}`, `wiring::action::reaction` (`reaction`, `kill`), `economy::death` (`monster_death_drop`: gate, TC, walk, gold by `ItemDrops`), `ActionHooks::{anim_data, vitals, mode_target}`, `d2-sim` → `d2-formats` dependency. **Vendor host** (`e2e-vendor-host`): `d2_server::adapters::handlers::world::TradeWorld<R>` (`npc` on `Desk`, `vendors` on `VendorDesk`, `waypoints` delegated to `ActionWorld`, `quests` added by `quest-host`; one unit world, `TradeRest`, `SimGame::with_world`); buy / sell stop at `VendorRest::copy_item` (`0x0055A2A0`, no items spec) and the inventory seams. All **wired, unverified** (M02) | wire-open-seams: 12 new integration tests (1,843 pass, 61 ignored workspace-wide on its branch; conformance `tick_replay` 7 pass unchanged); e2e: `same_seed_same_run` / `other_seed_other_run` over the transcript; `wiring/action/tests/death.rs` 7 tests; `e2e_vendor.rs` 4 tests (one gold more changes only byte 11 of the 0x2A); no S→C bytes on the cast / kill / drop path (no written spec ties a message to it) |
| 3y One wired host (`host-merge`, `host-merge-port`) | **Wired, unverified** (no rule added: the same `d2-sim` modules run, now on one unit world behind one host value). `SimGame<D, W>` has one host field `world: W` (the fields `items` and `skills` are gone); `WorldHost<D>` is the one host trait (`npc`, `vendors`, `waypoints`, `quests`, `cube`, `skill`, `take_sent`, `fault`); hosts `NoWorld`, `ActionWorld<S = NoSkills>` (waypoints, skill slot) and `WiredWorld<R, S = NoSkills>` (renamed from `TradeWorld`, `world/wired.rs`: economy parts, `CubeParts`, quests, NPC control, vendor tables, `HostWaypoints`). One unit store: `ItemWorld`, `ItemHooks`, `ItemView` are deleted; the cube runs on `WiredWorld::with_economy`, built from `ActionSim`'s own units, stats, data and hooks and the host's one `ItemStore`. One provider of the skill-use seams: `skills::seams` (`SkillSeams`) is deleted, `skills::world::World` wraps `wiring::interaction::UseView` (`SkillRest` = `Pending` + `UseRest` + `LearnRest`; 0x3A runs on the action wiring's `View` as `VitalsUnits`). Game-creation fields have one home in the action wiring (`ActionHooks::game_seed`, `ai_info`, `UnitData::expansion`; `ActionEvents::create_game(&GameFields)`, `GameFields::from_action`; item format +0x78 follows the expansion). The player's interaction (+0x64 / +0x68 / +0x6C) is owned by the trade rest's `NpcRest::{interact_unit, set_interact, reset_interact}` (cube through `items::Interact`, waypoints through `HostWaypoints`). `SimGame::tick` drains `WorldHost::take_sent` after `d2_sim::tick::tick` and queues each message to its client (failures in `SimGame::tick_faults`). Closes §7 J1, J4, I7 / W16 on the server host, EV1 / EV2, `e2e-vendor-host` §4 findings 1–3, `e2e-next` §5 findings 1 and 3 (2 in part). Ports on the merged host with every property kept: `prop_handle.rs` (5 pass in ≈ 0.7 s; hunts `PROPTEST_CASES=3000` all and 6000 on the trade host: no failure; faults now also read `tick_faults`, the action wiring's errors and the cube's; quest ids 0x31 / 0x40 / 0x58 join the owned ids and `hosts_reach_the_handlers` checks them; the action host's player level is 1, so 0x3B now reaches the spend), `prop_worldsim.rs` (4 pass in ≈ 11 s; `PROPTEST_CASES=200`: 4 pass in 287 s), `mutants_adapters.rs` (`SkillSlot`, a minimal `WorldHost`). Found by the port, a fixture and not the host: 0x49 to waypoint index 0 recorded `WiringError::Drlg(NoSpawnRoom)` because the synthetic DRLG had rooms only on Cold Plains (now one room on each waypoint level, acts 0 and 1) | `d2-server` lib 106 pass (count unchanged: the item tests now run on `SimGame<ActionSim, WiredWorld>`, the skill tests on `SimGame<ActionSim<Book>, ActionWorld<WiredSkills>>` with the units in a real field room); `e2e_single_player` 3 and `e2e_vendor` 4 pass on the new constructors; workspace 1,936 passed, 0 failed, 62 ignored; `sh tools/gate.sh` GATE PASS after each port. Other fixes: stale docs of `world/action.rs` and `SimGame::handle`'s `pierce_idx` TODO. Open: HM1–HM7 (§7), §2 step 7o |
| 3z Inventory on the wired host (`wire-inventory-sim`, `wire-inventory-server`) | **Wired, unverified** (`inventory.md` is a draft, no recording R1–R6; the adapters add no rule). `d2_sim::wiring::inventory` (new module, one line in `wiring/mod.rs`; no change to `items::inventory` / `items::moves`): `InvDesk` (borrows `Economy`, the `InvTables`, an `InvState` and a rest `R: InvRest + ?Sized`) implements `InvWorld`, `InventoryOps`, `MoveUnits` and `MovePending`, so `items::moves::handle` and `player_update` run on real state; one owner per field (`UnitRecord`: mode, unit flags, update bits; `ItemStore`: item flags, page, quality; `StatLists`: quantity, gold, durability, sockets; `InvState::items` / `expiry`: command flags, body location, grid x / y, owner GUID, node; `UnitLists`: room membership). Seams without a provider stay `InvRest` / `MovePending` defaults (path and placement, rooms, stat-list routines, requirements and hands, player data, belt / sound / quest hooks, item creation and use, sockets, hirelings, picks and transport; table in `wire-inventory-sim.md` §5): auto pickup of an item with a body location stops at `equip_picked`, 0x26 leaves the item in the belt, no gold pile is made without `gold_request`, ground items without a `free_spot` never drop. **Server:** the 23 ids of `items::moves::HANDLED` (C→S 0x16–0x29, 0x50, 0x61, 0x63; 0x4C stays unowned, `cube.md` §10) run `SimGame::handle` → `handlers::items::moves::handle` → `WorldHost::moves` (new; `WiredWorld::inventory: Option<InvParts>`, parts lent out for the call) → `InvDesk` → `items::moves::handle`; result 0–3 → `ResultCode`, a `MoveFatal` or a queueing failure → `WorldError::Move` and `Malformed`; direct sends (0x63's 0x9D action 5 / 0x9C action 0xE, a stack merge's 0x42, the targeting reset's 0x3F) leave through `MoveRest::take_sent`; the deferred item messages run in `handlers::items::moves::update_pass`, called by `SimGame::tick` after the tick and the host's tick sends (per client, per player in the client room's adjacent rooms: `player_update` → 0x9C / 0x9D / 0x7D, 0x47, 0x48; then `InvDesk::update_done` and +0xC8 bits 0 and 1 cleared; failures to `tick_faults`); `MOVE_IDS` names `inventory.md §7.x` for each id in `ITEM_IDS`. e2e (`e2e_single_player`, `e2e_support::InvFx`): step 5b, pick-up of the kill's gold (0x16), runs the handler and **stops** (asserted: result 1, the gold's record is in `DeathDrops::items`, not the host's store: HM5 / IS4); new steps 16–21 (frames 26–31) on a cap made in the host's store: 0x16 → 0x9C 1, 0x18 → 0x9C 4, 0x19 → 0x9D 5, 0x1A → 0x9D 6, 0x1C → 0x9D 8, 0x17 → ground (nothing sent), each frame's bytes exact, plus 0x47 / 0x48; client 33 frames, 32 ticks | sim: 26 integration tests in `wiring/inventory/tests/` (every handler through `items::moves::handle` with real message bytes, then the update pass; fake only `Rest`); M08: `ground::pickup_position_follows_the_grid_record` and each of 7 adapter routes broken by hand failed its tests. Server: 22 host-frame tests in `handlers/items/moves/tests.rs` (result codes, state and the exact S→C bytes of every id, 0x29's fatal → `Malformed` + fault, 0x61 classic → 3, the stub without inventory parts) and `move_ids_match_client_tsv_and_the_module` with `move_ids_check_reports_perturbations` (M08). `e2e_single_player` 3 pass, `same_seed_same_run` and `other_seed_other_run` pass. Gate PASS (coverage 3,542 then 3,813 claims, 0 errors; the sim note: d2-sim + conformance 1,445 passed; the server note's merge failed once only on the known seed-dependent `stats::prop_tests::stat_lists_match_the_model`, fixed by `fix-statlist-prop`, rerun 1,548 passed). Open: WN1–WN4, IS1–IS6, §2 steps 7o / 7i |
| 3aa Path core, walk and the path wiring (`impl-path-core`, `impl-walk`, `wire-path-sim`) | **Implemented and wired, unverified.** `d2_sim::path::{coords, record, collision, footprint, tables}` (`path-placement.md` §1–§6: sub-tiles, 16.16 centres, `DynamicPath` / `StaticPath` records, the `CollisionRooms` seam, point / plus / box / size / pattern queries, pattern and size footprints, try / forced / missile move, teleport; `PathTables` = all 18 tables of `path-tables.tsv`, embedded, strict parse; 26 tests, 33 of 84 units claimed) and `path::walk` (`pathing.md` §1–§10 for path types 1, 2, 7: request and mode checks, A* / toward / straight, velocity, step, cell walk, set position, reset, room-change messages, the builders of 0x0D, 0x0F, 0x10, 0x15, 0x96; no floating point, the x87 target lead is the seam `WalkUnits::target_lead`; 34 tests, vectors W1–W6, W9, D1–D4, M1, M2, S1, S2 exact, 105 of 125 units claimed; V1–V3 follow the formula, PQ1). No per-tick position trace exists (`pathing.md` OQ1, `path-placement.md` OQ1), so nothing is checked against 1.14d. `wire-path-sim` folded the duplicate seams, tables and types of the walk onto the core (`Walk<'a, C> { t, c }`, `WalkUnits` without `Game` arguments, `PathWorld: CollisionRooms`; list in the note §3) and wired it as `d2_sim::wiring::path::{PathState, PathCtx, rooms, units, walk, place}`, **opt-in per game**: `ActionHooks::paths: Option<Box<PathState>>`, `None` by default, `ActionHooks::enable_paths()` turns it on (off: every path seam keeps its `Pending` answer and all earlier tests are unchanged; on: `Pending`'s path methods are not called). Path records (unit +0x2C) live in `PathState::records`, created by the allocation's path part (§2.5) and freed by the unit removal; walk / run schedules the every-tick event 0 and the unit dispatch calls `UnitHooks::player_movement_step` → `wiring::path::walk::player_step`; the `CollisionRooms` provider is `DrlgWorld` (new `Drlg::active_grid_mut`); `Pending::warp` (same act) → `place::level_warp`, `Pending::set_player_mode_arrival` → `walk_to`, `WorldPending::nearest_free_point` → `place::coarse_free_box`, the missile collision queries (settles wire-action W5 / WG7 when on) and `AiWorld::collides` → the core. `path/mod.rs` now declares the whole module (J12 resolved) | core 26 + walk 34 + `gaps-path-render` 8 tests (`pathing.md` 106 → 114 of 125 units, 91.2%; no code fix: no test showed a deviation); wiring: 7 tests on the action fixture (`allocation_gives_the_unit_its_path_and_footprint`; walk M1 translated: x = 0x1AE000 + k·0x6000, tick 14 lands on 0x1F8000; room-edge crossing; a wall changes the walk; waypoint warp puts the player in the spawn room, the e2e step-6 condition; without the provider the warp stays pending; coarse free box avoids a monster footprint), all with an M08; `cargo test -p conformance` tick replay unchanged. `prop-walk`: 7 properties on the public walk API, **no bug found** (PX1 is a spec wording), 10,000 cases debug, 20,000 release; `prop-path-place`: 10 properties on §7–§12, one overflow fix in `path/warp.rs` (`wrapping_add` / `wrapping_mul`, PF1), `PROPTEST_CASES=2000` in ≈ 70 s. Gates PASS (impl-path-core: d2-sim 1,265; impl-walk: 1,273, coverage 3,403 claims; gaps-path-render: d2-sim + conformance 1,556, rest 567, d2-client 360, coverage 3,869 claims, 0 errors). Not done: three blockers WP1–WP3 and the list of §2 step 7p |
| 3ab Conformance harnesses for movement and placement (`conformance-path-render`) | `crates/conformance`: `movement::{read_movement, replay_movement}` (walk / run from `packets-raw-1`: C→S 0x01–0x04 → `Mover::request`, ticks, S→C 0x0D, 0x0F, 0x10, 0x15, 0x96; the first 0x15 of a (type, GUID) seeds the mover; message bytes and per-tick positions compared; mismatches name the layout field), `placement::{read_player_placements, replay_placement}` (players from 0x15, room = the one 0x07 since the previous 0x15; `Spawn` records for monsters and items), `needs::{NEEDED, report}` and `cargo run -p conformance --bin recordings-needed` (7 entries: units, stats, packets, movement-walk, movement-path-state, placement-players, placement-monsters-items; MISSING / present / BLOCKED). **Proven on synthetic recordings only; no d2-sim provider behind either seam yet** (`NotWired` → `MOVER NOT WIRED` / `PLACEMENT NOT WIRED`, never a pass); no `Covers:` claim (a claim here is the `trace` tier). No recording format holds per-tick path state or placement calls (CR1, CR2) | CI fixtures `movement-walk.jsonl` (first 14 0x96 follow `pathing.md` M1; the test mover is a stand-in) and `placement-players.jsonl` (R1–R3 shaped), both pass `check_packets.py` R1–R7; perturbations: each of 42 0x96 (message and position channels), missing / extra / wrong client, changed target, dropped tick, second 0x15, every spawn × guid / class / x / y, wrong room; `recordings_needed.rs`: every named script / test exists, every ignored `needs traces/raw/` test is listed. Queue: §5 C55–C57, A-seventh |
| 3ac Camera, placement and the app's world view (`render-camera-placement`, `render-wire`) | **Implemented, unverified against 1.14d (no capture).** `d2_client::rules` (`camera.rs`: `FrameSize`, `OpenMode` 0–3, `ViewRect`, tile / unit origins once per frame with shake, `unit_draw`, handed tile positions, culling, `Shake` envelope and `shake_offsets` = two `roll_range(−a, 2a)` draws; `placement.rs`: `Cel` (`dc6` / `dcc`), `RowPlan`, `draw_position`, `place`, `block_pixel`; `view.rs`: `OriginalView<R, S>` implements `ViewRules` over wrapped rules and a `ViewSource`; minimal data change `IndexFrame.anchor: FrameAnchor` (`Top`, `Bottom` = DC6 `flip` 0, `TopDown` = DC6 `flip` 1), `from_dc6` refuses `flip` ∉ {0, 1}, `from_dcc` refuses an odd `variable0`). `render-wire` put it in the app: every frame is built by `world_view::build_frame` (camera once from a `ViewFeed`, then `build` over `OriginalView`); `WorldViewState` gained `feed: Box<dyn ViewFeed>` (`ViewFeed: ViewSource` + `player`, `open_mode`, `shake`, `player_seed`; the app's `NoFeed` states no player, no map tiles, no shake, so without a player there is no camera and the window stays black: `NoCamera` errors on any listed tile or unit); one frame per server tick (draw only when `ClientWorld::server_ticks` advanced; UI input queued to the next drawn frame; `FrameStats.server_tick`); the capture `scene` case has a real source (`verify/capture_case/scene_source.rs`: `WorldScene<W: CaptureWorld>` builds the camera from the record, checks the recorded `view_rect`, `shift_x`, `tile_origin`, `unit_origin` against `Camera::new` before any pixel, then asks the world for units, tiles, UI; today's recordings stop there: `NotRecorded` → `SCENE NOT WIRED`, exit 2, after the camera check ran). Seams stopped, not invented: RW1–RW5, RC6 | `rules::` 16 tests (camera §2 vectors, 800 × 600 origins 600 / 1720 / 600 / 1716, player (400, 292), culling −81 / −80 / 553, shake envelope; placement DC6 / DCC vectors; a CPU golden scene compared as the whole 800 × 600 index framebuffer, M08: a one-pixel camera move changes exactly the 12 expected pixels); `world_view::feed::tests` 3, `verify::capture_case::scene_tests` 5 + 1 ignored GPU, `app_frame_loop` 6 (new: one drawn frame per tick with the feed asked once per tick `[1..5]`, shake on the tick time base); synthetic capture `CPU 0 of 480000`, `--perturb 1 / 7 / 64` → exactly N, a recorded tile origin off by one → FAIL naming the value; llvmpipe `gpu_half_matches_the_synthetic_capture` 0 differing, perturb 7 → 7. Gates PASS (coverage 3,319 claims at the first note). Claims: camera §1–§10 + edge cases, sprite-placement §2–§5, §7, §8 (not §1, §6). Queue: §5 C52–C54, A-seventh |
| 3ad Tests and tooling, seventh fold (`bench-baselines`, `prop-client`, `prop-inventory`, `fix-statlist-prop`, `mutants-server`, `mutants-monsters-missiles`, `game-tests-inventory-path`) | **Benchmarks** (criterion, not in CI; `cargo bench -p d2-sim --bench sim`, `-p d2-formats --bench formats`, `-p d2-proto --bench proto`, `-p d2-client --bench compose`; feature `bench-fixtures` widens the `cfg(test)` of worldgen / stats / skills / items fixtures, behaviour-neutral). First numbers (cloud 4 vCPU Xeon 2.10 GHz, rustc 1.99, quick settings, order of magnitude; the tick budget is 40 ms): sim tick, 24 monsters × 200 ticks 46 µs; populated level setup 954 µs; DRLG act 0 509 µs, + the 40 × 18 preset level 528 µs; `create_item` ≈ 215 ns; treasure `walk` ≈ 330 ns / drop; stat lists (player + 12 item lists) 36.7 µs; MPQ read 256 KiB PKWARE + encrypted 267 MiB/s, 64 KiB plain 15.6 GiB/s; DC6 32 frames 17.8 µs; DCC 8 directions 70 µs; proto classify ≈ 15 ns, split 500 messages 3.0 µs; compositor `compose` 800 × 600 / 400 items 4.75 ms (101 Mpx/s), `compose_binned` 10.7 ms, `to_rgba` 0.73 ms. Hot spots: BB1 (MPQ Huffman about 3 MiB/s, not kept in the bench), the CPU compositor is the reference only, the sim numbers are floor values (idle monsters, no combat, pathing, missiles or clients). **Property tests:** `prop-client` (7 files in `d2-client/tests`: `prop_client_bridge`, `prop_world_view`, `prop_controls`, `prop_ui`, `prop_atlas`, `prop_scene`, `prop_cache`, plus `prop_support`; ≈ 25 s in debug; hunted at 3000–5000 cases, 400 for world view and atlas) found five bugs, each fixed at the root with a `regress_*` test (`ui/geom.rs` `Rect` edges past i32 in i64; `CellGrid::new` refuses a grid past i32 with `TooLarge`; `ScrollList::scroll` saturates; `Presentation::to_frame` saturates the bars; views past the i32 screen range are refused by `compose`, `compose_binned` and `bin` with the new `SceneError::View`; a sixth, `route` past 0x204 bytes, was fuzz-server's fix already on the base); `prop-inventory` (`d2-sim/tests/prop_inventory.rs`: state machine of the inventory model against a reference written from `inventory.md` §1–§5, 256 cases of 1–60 ops; `items/moves/prop_tests.rs`: every `HANDLED` id on arbitrary payloads, 512 cases; 20,000 and 30,000 cases pass) found one bug (`grid::in_bounds` overflowed `x + w` in i32, reachable from a 0x18 payload at x = 0x7FFFFFFF; now i64, the placement is refused, `regress_place_near_i32_max`); `fix-statlist-prop`: the CI failure `stat_lists_match_the_model` ("full of stat 19 layer 0 … left -1, right 0") was a **model** error (`toggle_drift` did not drift the parent P that holds the toggled list in its active chain, `stat-lists.md` §8.6 moves values into U's list only), fixed in the model with `regress_full_after_free_and_toggle`, no code change; 200,000 cases pass. **Mutation testing** (`cargo-mutants` 27.1.0): `d2-server` 862 mutants, 390 caught / 366 missed / 3 timeout / 103 unviable → 415 caught / 341 missed with 25 kills from `mutants_core.rs` and `mutants_adapters.rs` (`dispatch`, `check_size`, `select_skill`, `ClientBuffers`, `Inbox`, `Host::flush` / `disconnect`, `SystemClock`, `unit_target`, skill routing); on the merged base 915 mutants, 457 caught, 343 missed, 3 timeout, 112 unviable (these counts predate the host merge: `skills/seams.rs` is gone and `trade.rs` is `wired.rs`, so re-run); `d2-net` has no mutants (two doc lines); monsters / missiles 2,461 mutants, 322 missed → 50 missed after 121 new tests (115 with `Covers:` claims; missiles 54 → 6, monsters 268 → 44), the 50 are 22 equivalent, 25 unobservable (17 default `InitHost` bodies) and 3 spec gaps (MM1–MM3); no code disagreed with a spec. **Game-file tests, not run** (`game-tests-inventory-path`): `d2-sim/tests/game_inventory_path.rs`, 6 `#[ignore]` tests + 2 CI tests; no `Covers:` claim until a local pass (§5 C51) | `cargo test -p d2-client` at `prop-client`: unit 220 + 5 ignored, every integration file passes; `prop-inventory` and `prop-walk` add no claims (they overlap the unit claims); mutation: the 36 spec-decided survivors of `d2-server` and their fixtures are §2 step 7t; the benches compile in `cargo clippy --workspace --all-targets` |
| 3ae Gap tests for the newly implemented specs (`gaps-new-specs`) | unit-tier tests, no game files, no code fix (every new test passed against the existing code): `path/gap_tests.rs` (`path-placement.md` §7.1 wrappers: max distance 50 reaches ring 49 and not 50, the fallback argument of `0x0064E7B0`, `0x0064E7E0` steps by k and never falls back, `0x0064E810` needs the walk-back to the origin; §9 text: the floor drop is one cell against 0x3E01, each of its six bits blocks, 0x8 does not; §12.1 text: types 10 / 11, the tile relative to R, slot from bits 20–25 only), `items/inventory/gap_tests.rs` (§1.4 r3, §2.4 r5, §4.3 hand table for L = 4, 5, 11, 12, §4.6 r3, edge case 9: all 512 occupancies of a 3 × 3 grid × item sizes up to 3 × 3), `items/moves/tests/gaps.rs` (§7.21: 0x4C is left alone at any length), `d2-server` `handlers/world/tests/gaps.rs` (`quests.md` §6.2: 0x40 frames as one byte, runs `0x00546040`: 0x28 first, 0x52 last) | M08 by hand, each reverted and each failing exactly its new test: `FREE_MAX_DISTANCE` 51, fallback on in `free_point_step`, `ITEM_FLOOR` without 0x2000, the warp letter swapped, `SWAP_OTHER_TO_PAGE` → 0, the §4.6 step 3 skip test dropped, the §2.3 grid-edge weights 0, the §2.4 link check moved after step 6. Any-tier coverage on the merged base: `inventory.md` 145 of 148, `path-placement.md` 80 of 84, `quests.md` 74 of 81, `composition.md` 9 of 16, `capture.md` 6 of 9, total 2,829 of 3,106 (91.1%). Not claimed, with reasons (note §4): `inventory.md` §1.1 (no byte layout), §2.4 optional-inventory argument, §6.1 r4 (not implemented); `path-placement.md` §10 / §12.2 caller lists, §10 r7 (position history kept out of `d2-sim`), edge r1 / r8; `quests.md` §2.3 r4, §9.1, §9.2, §9.4 (partial), §10.2, §10.7 (D2MOO-derived), §11; `composition.md` §1, §3 r1, r3, §7; `capture.md` §1–§3 (the recorder's side); `ai.md` 11 and `missiles.md` 13 unchanged from `gaps-combat-ai`. No new spec question |
| 3af One item store and one inventory per game; the drop's free spot (`unify-items`, `drop-freespot`) | **Wired, unverified** (no rule added: the same `d2-sim` modules run on one store and one model). **Eighth fold.** `ActionHooks::items` (`wiring::economy::ItemStore`) is the game's one item store, lent to an economy for a call and written back; `DeathDrops::items` and `WiredWorld`'s own store are gone, so the kill's gold / item can be picked up (0x16), sold (0x33) and cubed (0x2A) from the item data the drop wrote (closes `host-merge` W-5 = HM5, `wire-inventory-server` F1 = IS4). `WiredWorld::inventory` (`InvParts` + `InvState` + the move rest) is the one inventory: the cube's `Staged::inventories` and `Inventory` are deleted, the cube (`ServerCube`) and the vendor's player-inventory calls (`InvVendors`, `handlers::items::vendor_inv`) answer from the model (F2 = IS5); an item placed by 0x18 is sellable and usable in the cube, and an item bought or transmuted lands where the moves see it. New `d2_sim::wiring::inventory::host` (reads `InvState::{of, items_of, cursor_of, holds, body_items, fillers}`; rules `InvDesk::{place (§2.4), remove (§1.4), free (0x00557FD0), reset_targeting (§5.3), check_stored / check_ground_or_owned (§5.1), send_item_page (§6.4 direct 0x9D)}`). **Free spot** (WP2 of step 7p): `DropPlacer<H>::place(econ, x, y)` receives the economy, so with `ActionHooks::paths` on **and** `PathState::field` loaded the death drop runs `wiring::path::place::floor_drop` (`treasure.md` §7 step 2, `path-placement.md` §9, size 1, fallback true), the dropper's position is `h.path_position`, and each created item gets its path part (`SUNIT_Add`, `path-placement.md` §2.5) from `DropPlacer::placed`, so the next drop avoids it; with the provider off, or on without the field, the `FreeSpot` seam answers as before (the hosts do not load `ExpField.D2` yet). Readings: `owns_item` as `inventory.md` §5.1's owned-item test (R1 = UI1), `remove_stored` queues no message (R2 = UI2), the order of a vendor call's inventory messages (R3 = UI3), the free spot's room (DF1) | d2-sim inventory wiring 31 (5 new in `tests/host.rs`), d2-server 163 (cube tests ported to the model), `e2e_single_player` 3 (36 frames, was 32: kill gold picked up, cap sold at Akara, buy stops at the item copy `0x0055A2A0`, ring picked, put in the cube, transmuted; same seed same run) and `e2e_vendor` 4 pass; `wiring/action/tests/death.rs` 4 new (vectors D1–D3 translated by (+3, 0): floor drop (15, 13), wall column → (14, 13), item bit at the start → (14, 13), a dropped item blocks the next, no field → the seam's answer); M08: with `placed` disabled and the old `h.x.position` read, all 4 fail. Gates: unify-items **PASS** (every step); drop-freespot **PASS** (d2-sim + conformance 2,038, rest 599, d2-client 363) |
| 3ag Walk / run on the server and the update pass (`wire-path-server`, `path-update-pass`) | **Wired, unverified** (no rule added; no per-tick recording, `pathing.md` OQ1). **Eighth fold: C→S 0x01–0x04 run** (`d2_server::adapters::handlers::walk`: `WALK_IDS`, `handle` reads x, y or type + GUID and returns result 0 in every case (§1.1), `run` calls `wiring::path::walk::walk_message` through `ActionSim::with`, `enable_paths`); `WorldHost::walk` (default `None`, implemented by `ActionWorld` and `WiredWorld`), tried in `SimGame::handle` after skills and before the stub; without the provider the ids stay stubs (recorded in `unhandled`), so every earlier host and test is unchanged. **Update pass** (`pathing.md` §10 rules 3 then 2): `tick::client_update` → `TickHooks::send_unit_update` (`0x0053A5D0`) → `ActionSim` (forwarded by `WorldSim` and `QuestTick`) → `wiring::path::walk::update_messages`: S→C 0x15 from flags 2 (0x10000 → flag 1; 0x800 for the other clients → flag 0), then for a player with unit flag 0x1 in modes 2 / 3 / 6 0x0F or 0x10 to every client but its own; sent through `Pending::send`, reaching the clients by `SimGame::tick`'s `take_sent`. 0x0D is the placement's and the waypoint arrival's (`waypoints.md` §7 r7): with the provider on the same-act warp places the player in the spawn room and the client gets 0x07 then 0x0D (e2e step 6 holds on `e2e_walk.rs`; `e2e_single_player.rs` keeps the provider off because its step 4 flies a missile, blocker WP1). 0x96 has no sender spec (`pathing.md` §10 r5, OQ6): never sent; rule 4 (monsters 0x67 / 0x68, `0x00598220`) has no spec owner: `update_messages` sends nothing for non-players. The sim builds the bytes (`path::walk::messages`); every test states them with `d2-proto`'s typed builders (`PlayerMove`, `PlayerToTarget`, `PlayerStop`, `ReassignPlayer`, `MapReveal`). Readings to settle: no 0x15 after a waypoint warp to another level (WS1), flags 2 0x10000 / 0x800 and unit flag 0x1 never cleared (WS2), the dispatcher's range checks read the staged `UnitFacts` (WS3), run velocity = walk velocity (WS4), a same-level two-room waypoint travel sends no 0x0D (WS5), the second client's 0x0F per tick (PU1) | d2-server walk tests 9 → 14 (`walk/tests.rs`: `walk_to_point_moves_the_player_and_tells_the_other_client` = vector M1 translated, x = 0x1AE000 + k · 0x6000, tick 14 at 0x1F8000; run to a point (stamina − 14 · 40, code 0x17); walk / run to a unit (0x10, codes 0 / 0x18); room change A → B; refused requests still 0; stubs without the provider; the waypoint to the town (0x07 then 0x0D); a warp within the level (0x15 next pass); `walk_ids_match_client_messages_tsv` with three perturbations; the five update-pass branches: 0x800 before the mode update, 0x10000 to both clients, town walk, only players, nothing without the provider); `d2-client/tests/e2e_walk.rs` 2 (walk A → B 62 frames, run back 56, walk to the waypoint object, 0x49 to the town: 0x07 + 0x0D exact bytes; same run twice). Gates PASS: wire-path-server (tests in its commit), path-update-pass d2-sim + conformance 1,876, rest 604, d2-client 362 |
| 3ah Tests and tooling, eighth fold (`prop-fixes`, `prop-walk-gaps`, `mutants-core`, `mutants-inventory`, `mutants-world`, `mutants-items-treasure`, `mutants-combat-skills`, `ci-nightly-props`, `game-tests-wired-host`, `spec-unit-numbering`) | **Fixes the property tests deferred** (step 7n, each test written first and seen failing on the base): `StatLists::expire_lists` returns `Result` and stops with `StatListError::EndlessExpiry(list)` at the first expired extended list (the state 1.14d spins in; event 12 passes it up as `UnitError::Stats`; the replay reports it as a mismatch); a stale `ListId` acts as a null list in every public `StatLists` method (the lists stay byte-for-byte unchanged); `TimerId { slot, generation }` (the generation is d2rs's own, so `cancel` of a reused id no longer reaches the new timer); `dispatch::in_range` uses `abs_diff` (exact abs(dx) ≤ 50 and abs(dy) ≤ 50 for any positions); the `remove_active_room` item stays open (Q3 = SL3). **Property tests for the walk gaps**: `prop_walk_rooms.rs` (multi-room walk: room recache §9.6 r9, §9.8 messages, flag 0x1; the messages as set differences; `set_position_recaches_by_spec`), `prop_walk_motion.rs` (knockback keeps the distance budget, `set_path_type_follows_the_table`, `chase_a_moving_target`), `prop_path_footprint_ops.rs` (`set_foot_mask_restamps`, `corpse_footprint_is_a_plus_of_0x8000`, `teleport_follows_the_spec`) on the shared fake `walk_rooms_fake`; 17 M08 mutants caught (one survived the first run until the generator picked a teleport onto its own cell 15 % of the time); `PROPTEST_CASES=30000` release hunt passes; no code bug found; `// Covers:` on 16 rules (`pathing.md` §1.5 r2, §2, §3 r10, §9.2 r1, r3, §9.5 r3, §9.6 r4, r8, r9, §9.7, §9.8, §9.10; `path-placement.md` §5.1, §5.3 r1, r3, §6 r4). **Mutation testing** (`cargo-mutants` 27.1.0, base `4b5b0bf` / `edd9925`; no code was found wrong against its spec and none was changed; every test names its spec rule): `d2-sim` tick / units / stats 1,342 mutants, 149 missed → 47 (rng + game 48, 0 missed; the conformance replay killed **0** of the 149, its traces do not reach stats, regeneration or the stat-list chain); `d2-proto` 844 mutants, 32 missed → 9 (14 of 993 after merging `Packed` fields and `s2c`); `items::inventory` + `items::moves` 1,304 mutants, 135 missed → 64 (33 of the 64 are `MovePending` default bodies); `d2-sim::world` 2,385 mutants, 389 missed → 34 (115 tests); `items` + `treasure` 1,819 mutants, 222 missed → 48 (62 tests; 1,686 caught); combat + skills 1,940 mutants, 358 missed → 47 (105 tests; missed rate 18.8 % → 2.5 % of viable; 5 timeouts). Every survivor is classified equivalent, unreachable or not decided by the spec (listed in the notes); claims only where the assertions check the whole rule (`stats.md` §2 r2, §5 r2, r3, §8 r2, r3, §9 r3; `intents-events.md` §2.4 r10; `callbacks.md` §2 text; the treasure and item rules named in `mutants-items-treasure`; `coverage --check` 3,315 claims at that gate). **Nightly deep property job** (`.github/workflows/nightly-props.yml`, 03:17 UTC + `workflow_dispatch`, input `cases`, three matrix jobs `sim`, `wire`, `worldsim`, 90 min timeout; `tools/props-deep.sh [sim, wire, worldsim or all] [cases]` holds the filters so local == CI; on failure it prints each `minimal failing input` and uploads `target/props-deep/` as `proptest-<group>`): `sim` at 20,000 cases **measured 670 s** (`floor_drop_never_blocked` 630 s, `nearest_with_field_matches_reference` 597 s), `wire` ≈ 10–20 min and `worldsim` ≈ 12 min **estimates**, not run; the first deep run found the stat-list failure (fixed on the model side, `fix-statlist-prop`, plus `prop-fixes` item 1) and `anim_schedule_matches_the_closed_form` aborting with "Too many global rejects" (fixed by the coordinator: `schedule_inputs()` doubles a positive step until the §4.2 loop is under `MAX_LOOP`); after both, `stats::prop_tests` at 20,000, `prop_timer` at 3,000 and `prop_units` at 20,000 pass. **`game-tests-wired-host`**: 7 `#[ignore]` tests in `crates/d2-server/tests/game_wired_host.rs`, one per `charstats` class (`wired_host_amazon` … `_assassin`; §5 C59), each runs twice and compares FNV-1a 64 digests; no claim (`Intended claim: none`), **none has run**. **`spec-unit-numbering`**: the `Edge cases & original bugs` lists of 15 specs became numbered lists (`N. `, text unchanged) and 8 claims were narrowed after reading each test (cube r1–r7, txt-format r1, r3, r5, r8, preset, animdata r1–r4, camera r1 / r2, sprite-placement r1, loading r1, r2, r4); `tools/coverage.py` + `docs/COVERAGE.md` §1 gained **table-row units** (a table directly under a `<!-- rows -->` line gives `§<s> row<N>`, `§<s> t<K> row<N>` for the K-th marked table; opt-in per table; selftest extended): `sim/rng.md` §5.2–§7 are 39 units, only §5.4 row1–2 claimed (`setters_and_derive`); the rest is trace / RE material | prop-fixes: `gate.sh` PASS (13 steps; d2-sim + conformance 1,880 passed / 107 skipped, rest 601 / 75, d2-client 362 / 16); prop-walk-gaps `--no-client` PASS (d2-sim + conformance 1,885, rest 599; 4,143 claims, 0 errors); the five mutation notes' gates PASS (mutants-combat-skills: d2-sim 1,337 passed, 5 ignored; mutants-items-treasure 1,294 passed, 5 ignored; mutants-core 1,273 passed, 5 ignored); `coverage` before → after of `spec-unit-numbering`: rules 3,106 → 3,190, any 2,839 (91.4 %) → 2,896 (90.8 %), verified 219 → 227 (7.1 %): the dips are real (camera 100 → 85.7 %, sprite-placement 77.8 → 58.3 %, cube 93.9 → 82.5 %) |
| 3ai Test installs, MPQ Huffman and the S→C builders, eighth fold (`fixedset-game`, `mpq-huffman`, `s2c-use`) | **Game from a `FixedSet`** (`test-fixtures`, step 7e, repo only): DS1 writer (`test_fixtures::ds1::write`, v1–18, round trips v7–18 × tag types 0 / 1 / 2 and the spec vectors) and DT1 writer (`dt1::write`; iso and RLE blocks; the spec vectors); synthetic DRLG files (`drlg`: a 9 × 9 town with one waypoint object, keep, cave, field, sub, six DT1; archive names `DATA\GLOBAL\TILES\` + table string); a lvlprest row Def 3 for level 2 ("Synth Field", DrlgType 2: the town's `Vis0` links it, so the first tick's room update builds it); `GameData { bins, anim, fixed, level, files }` with `load` / `from_install` and the builders `action_tables`, `world_tables`, `vitals`, `stat_data`, `unit_data`, `waypoints`, `level_types`, `drlg_world`, `world_sim` (the ones `game_wired_host.rs` assembles by hand on the live set; they work for the user's install too), `Seams`, `type Sim = SimGame<WorldSim<Seams>, ActionWorld>`; `test-fixtures` now depends on `d2-sim` and `d2-server` (normal dependencies, still test-only, no Bevy). `crates/test-fixtures/tests/synthetic_game.rs` (CI, no `#[ignore]`): archives → `bin::load` → `fixup::apply` → table views and `world_data` providers → `WorldSim` → a one-room town generated and streamed (81 floor records, the waypoint preset unit, a class-3 player) → `SimGame::join` → `Host::frame` × 51, no fault, no wiring error, a second run equal. **Seam: `ActCreation::TownOnly`** (`Drlg::create` with `NoLevelTypes` and then the town allocated and generated): `ActCreation::Full` (the act placer of `levels.md` §3 step 7 allocates 1.14d ids 1–7, 17, 26, 39 and expects their types, which the 8-row made-up levels table lacks: `UnknownLevel(17)`) works on the user's real data only, and the TownOnly DRLG seed skips the placer's draws (a fixture state, not a 1.14d one) (FG1); no server message reaches the idle client in 50 frames (no join sequence is wired; FG2). **MPQ Huffman** (`mpq-huffman`, BB1 of step 7r; `specs/formats/mpq.md` §9, §11 unchanged): the encoder `huffman::compress` is now `cfg(any(test, feature = "test-support"))` (weight tables 0–8, O(1) symbol lookup, word accumulator), `writer::Method::Huffman { table, pkware }` (mask 0x01, or 0x09 with PKWARE over the Huffman stream, a sector longer than the sector stored raw) and `writer::huffman`, `WriteError::BadHuffmanTable`; **decoder speed-up with unchanged output and errors**: a 256-slot leader cache `weight % 256 → node` used only as a hint (`Tree::leader` accepts an entry only if that node has weight `w` and its predecessor does not) and a static fast path for tables 1–8 (the tree stays borrowed from the template until the first escape; a 1024-entry lookup resolves up to 10 bits). Cause of the BB1 figure: not a rebuild per sector (cloning costs 30 ns for table 0, 1.2 µs for the 515-node tables) but one Increment per tree level per symbol with about four SipHash operations each. Criterion `mpq_huffman` (256 KiB, 512-byte sectors, cloud 4 vCPU Xeon 2.10 GHz, medians, order of magnitude): text table 0 74.7 ms (3.35 MiB/s) → 22.2 ms (11.3 MiB/s), table 0 + PKWARE 72.4 → 25.2 ms, table 1 6.10 → 1.78 ms (140 MiB/s), table 2 3.82 → 1.90, table 3 6.01 → 2.03; deltas table 0 51.3 → 13.8 ms, table 1 7.43 → 2.27, tables 4 and 6 about unchanged (escapes early), table 5 1.26×. Tests (`mpq/huffman_tests.rs`): `huffman_round_trip` (every table × three data shapes against the model), `decoder_matches_the_model` (arbitrary and truncated streams), `archive_huffman_round_trip` (tables, ± PKWARE, sector shifts 0–3, encryption + FIX_KEY, SECTOR_CRC), `huffman_then_adpcm` (masks 0x41 / 0x81), `leader_cache_is_only_a_hint`; M08: ignoring the fast path's `consume` error is caught, a broken leader test never finishes, wrong cache nodes survive by design. **S→C builders** (`s2c-use`, step 7k): **blocked, then decided: `d2-sim` keeps no dependency on `d2-proto`** (option A; `tools/depcheck` `FORBIDDEN` has had `("d2-sim", "d2-proto")` since Phase 0; the task text and `s2c-builders.md` §5 item 3 said it was allowed, §8 lesson). The sim keeps its byte builders; new `world::cube::trade_action` (0x77) used by the three inline `[0x77, …]` sends; `d2-sim/tests/s2c_bytes.rs` (7 tests; pins the bytes of `npc::{transaction, service_result, resurrect_message}`, `waypoints::{menu_message, arrival_message}`, `path::walk::messages::player_stop`, `cube::trade_action`); `conformance/tests/s2c_builders.rs` (7 tests, one per builder: sim bytes == `d2_proto::s2c::{NpcTransaction, OpenUi, Unknown9B, WaypointMenu, PlayerStop, TradeAction}::encode()` and `parse(bytes)` == the same `Message` over every edge combination plus a fixed xorshift sweep; 0x77 over all 256 actions), which settles J13 "one maker per id" as a check; M08: flipping bit 0 of `trade_action`'s byte fails exactly the two 0x77 tests. `bridge-dispatch.tsv` stays all `TBD` (no client spec owns an S→C id; the typed variant for each is in `s2c-use` §3) | `synthetic_game` 2 tests (`providers_hold_the_named_files`, `town_join_and_frames`) and the writers' round trips pass; mpq-huffman: `--no-client` gate PASS (1,876 + 607 tests); s2c-use: every non-client step PASS, the three `d2-client` steps fail in that container only (`wayland-sys` finds no `wayland-client`; `tools/cloud-setup.sh` installs it); `cargo clippy --workspace --exclude d2-client --all-targets -- -D warnings` clean |
| 3 not implemented | object operate / init, the per-skill function bodies (`use.md` OQ10), kill experience (`kill_experience` in `wiring::interaction` applies `vitals.md` §4.2 and §4.3's add only; nothing calls it, the kill has no provider, §7 WI10), walk / run in `d2-server` for monsters and without the provider (C→S 0x01–0x04 run for players on the path provider since the eighth fold, 3ag; they stay stubs when `ActionHooks::paths` is off), missile flight on the path provider (`pathing.md` OQ3), item-use bodies, hirelings, player data, sockets and the other `InvRest` seams (the item moves are wired and served, 3z), the item bit stream of 0x9C / 0x9D, message 0x73 (`missiles.md` R2.4, needs a protocol seam outside `missiles/`), Act II–V quests, the other 131 AI functions, server-do / server-hit bodies, mercenary spawn / init (spec not written) | specs exist as drafts or are unwritten |
| 4 Conformance | recording proven feasible; coverage tool done; **eighth fold (`py tools/coverage.py --summary`): 4,194 claims over 3,339 rule units: unit 2,858 (85.6%), game-file 197 (5.9%), trace 30 (0.9%), verified 227 (6.8%), any tier 2,911 (87.2%); `--check` passes; the denominator grew with the three new specs (`ui/text.md` 39, `render/draw-order.md` 23, `render/unit-composite.md` 41 units, no claim yet) and with 39 table-row units of `sim/rng.md` §5.2–§7 and the numbered edge-case lists (`spec-unit-numbering`: rules 3,106 → 3,190 on its own base), so the any-tier share fell from 91.4% at the seventh fold; the tool's `verified` count counts game-file claims on `#[ignore]` tests that never ran** | `tools/trace-recorder`: 32,543 recorded RNG draws match the spec exactly. `py tools/coverage.py --summary` at `edd9925` (all branches merged, sixth fold): 3,542 claims over 3,106 rule units: unit 2,612 (84.1%), game-file 189 (6.1%), trace 30 (1.0%), verified 219 (7.1%), any 2,660 (85.6%) — tested (any tier) 85.6%, verified 7.1%; the percentages fell because the denominator grew by 402 rule units of specs written since (`inventory` 148, `pathing` 125, `path-placement` 84, `composition` 16, `camera` 11, `capture` 9, `sprite-placement` 9; `pathing`, `camera` and `sprite-placement` have no claim) while claims grew by 283; for the `4b5b0bf` figure: 3,259 claims over 2,704 units, unit 2,404 (88.9%), game-file 187, trace 30, verified 217, any 2,459 (90.9%); the 28 game-tier claims the `gaps-data-formats` session added and the one of `gaps-client-formats` (`ds1.md` edge cases) sit on `#[ignore]` tests that have never run, and the tool counts them (verified was 188 before the first 28), while that note and `docs/COVERAGE.md` §3 count a game claim as verified only while its latest local run passes: read 219 as an upper bound until §5 C17, C20 and C38 are run (the two game claims added since sit on `path::search::tests::expfield_live`); the 67 game-file tests of the sixth fold carry no claim yet (§1 3w), so none of them is in 189 (history: `--summary` total line on main at `fd37fba` was 3,164 claims, unit 86.6%, any 88.6%, verified 216; the `coverage-claims` branch measured 1,520 claims, unit 1,072, verified 180 before the implementation notes landed; per-branch figures of the gap-test notes, 67.1% → 71.6–73.5% any tier on their own bases and 88.8% → 89.2–89.9% for the second four, are not additive; the 90.9% above is the one merged run). Verified units come from the sim-0006/0007/0008 replays (tick.md 14, unit-order.md 12, rng.md §3 r2 / r4) and three ignored game-file tests (§5 C1, C8): the unit tier is claims by synthetic tests, not fidelity; seventh fold (all branches merged one by one, merged tree at this fold: `py tools/coverage.py --check` 4,038 claims, 0 errors; `--summary` 2,839 of 3,106 rules any tier (91.4%), unit 2,791 (89.9%), verified 219 (7.1%, unchanged: tick, rng and the game-file / trace checks of earlier folds); the notes' own gates reported 3,403, 3,813 and 3,869 claims; `pathing.md` 114 of 125 units any tier, `path-placement.md` 77 of 84, `camera.md` 11 of 11, `sprite-placement.md` 7 of 9, repository 2,827 of 3,106 (91.0%) any tier on `gaps-path-render`'s base; `prop-*` and `game-tests-inventory-path` add no claim; `mutants-monsters-missiles` adds 115 unit-tier claims. Replay harnesses for movement and placement exist but have never run on a 1.14d recording (row 3ab). Still no verified unit from this fold |
| 5 Local server + bridge | bridge design `specs/client/bridge.md` and skeleton `d2-client::bridge` done; `bridge::local::LocalLink` implements `ServerLink` on `d2_server::host::Host` (`claude/p5-local-server`, synthetic data only, unverified; `d2-client` now depends on `d2-server`); every S→C id still unowned (`bridge-dispatch.tsv` all `TBD`); session code is the placeholder `PendingSession`; the app now builds the single-player game on a server thread (row 6) | `cargo test -p d2-client bridge`: every synthetic vector of `bridge.md`, the dispatch TSV check with its perturbation test, a windowless Bevy `App` mirror test; `bridge/local_tests.rs` headless end to end on the real host, `ProtoSizes` and `SimGame<ActionSim, ActionWorld>` (two-act synthetic DRLG, seed 1234, a sorceress at (42, 20) beside a Cold Plains waypoint): C→S 0x49 drained, dispatched (result 0), ticked, flushed, and the exact S→C `0D 00 <guid> 01 2D00 1700 0000` reaches a synthetic 0x0D handler that adds unit (0, guid) to `ClientWorld`; duplicate filter, protocol version check (a wrapper reporting version + 1 refused), unknown / unowned ids; `app_frame_loop` and `app_single_player` (row 6) run the same link through a Bevy `App` for 300 ticks |
| 6 Client | design drafts `specs/client/{render-pipeline,assets,ui,audio}.md` (d2rs-own). Cloud tasks implemented (plain-Rust infrastructure, no original behavior): **C1** paths + loaders, **C2** residency cache, **C3** frames + atlas, **C4** scene + CPU compositor, **C5** GPU compute compositor (`gpu_compositor`: WGSL, integer math, no sampler, 16×16 workgroups), **C6** verify harness (`verify`: case files version 1, `--perturb`, `GpuCompositor` seam), **C6×C5** GPU half of the synthetic cases wired (`verify::gpu::Wgpu`, `[[unit]]` COF cases), **C7** COF composite mechanics (`composite`), **C8** UI core, **C9** controls file, **C10** audio core. Nothing original-behavior is reproduced: every §B point is a `TODO(spec: …)` hook or a `ComponentResolver` method. GPU byte-exactness **proven on a real GPU** (Intel HD Graphics 630, Vulkan: `gpu_compare` 12/12, `d2-client verify` 11/11, perturbations exact, §5 Done 2026-10-06) and on Mesa llvmpipe 25.2.8 | CI unit tests from each spec's vectors (`d2-client` per branch: frames 53 pass + 1 ignored, scene 20 tests, ui 20, controls 16, audio 25; `gpu_compositor` 11 + 2 ignored, `composite` 11 + 1 ignored, `verify` 191 lib tests pass, 5 ignored on the `p6-verify-gpu` branch); llvmpipe: all 12 `gpu_compare` cases and all 10 synthetic verify cases 0 differing, `--perturb 7` reports exactly 7 on every half; game-file and real-GPU halves queued (§5) |
| 6 Client, fifth fold (`p6-window`, `verify-map`, `client-own-gaps`) | **The client is an app on a window** (`cargo run -p d2-client`, subcommand `play`, now the default; `view` stays): `SimGame<ActionSim<LocalSeams>, ActionWorld>` (two-act synthetic DRLG, sorceress beside the Cold Plains waypoint, 0x49 on its real provider) behind `d2_server::host::Host` in `bridge::local::LocalLink`, **on a server thread** (`app::server_thread::ThreadLink`, `Send + Sync` for any link; the game is not `Send`: `DrlgWorld` holds un-`Send` boxed `TileSource` / `LevelTypes` and `wiring::worldgen::SharedTypes` is an `Rc<RefCell<_>>`; `d2-sim` unchanged); per frame `BridgePlugin` pumps the server in `PreUpdate`, the world view packs the frame in `Update`, and the render-graph system `compose_node` runs the compute compositor on Bevy's own device (`Gpu::from_device`) and copies its RGBA rows into the presented texture, no readback. With `D2_GAME_DIR` (and no `--synthetic`) the `levels` / `objects` tables are the user's and the waypoint object is the first `objects` row with operate function 23 and init function 17 (`waypoints.md` §5.1 r1); the DRLG stays synthetic. **The window is black by design:** no S→C id has an owner spec, every §B rule is `Unspecified`, the frame palette is all zeros. **Frame store done** (`frames::FrameStore`: `(FrameSetKey, index)` → dense `scene::FrameId`, a `scene::FrameSource`, builds a C3 atlas; `composite::build_with` takes ids from it) and **the `map` verify case ported to the compositor** (`verify/map.rs`, chunks of ≤ 1024², three byte comparisons, `--perturb N` flips N indices and N RGBA pixels). **d2rs-own gaps now with code** (`client-own-gaps`): `audio::pool::SoundPool` (audio §A1 r1, r3; assets §A5 `sounds`), `assets::prefetch::PrefetchQueue` + `Pool::offer` (assets §A4 r3), `ui::text::layout_text` + `TextRules` (ui §A3); `WavDecoder`, the sound-id → file map and the text rules stay `TODO(spec: …)`. **Bug found and fixed in the staging** by the first window run: joining with `Some(room)` skips the client room switch (`rooms.md` §4.1), so tick step 9 freed the player's room and the next client update panicked (PW1) | `app_frame_loop` (2 tests: 300 ticks, 0x49 → unowned S→C 0x0D, presented image = CPU reference; GPU node output read back = CPU reference byte for byte, skipped without an adapter), `app_single_player` (4 + 1 ignored: `Send + Sync`, failing builder, deterministic build on the thread, live tables); `d2-client` lib 224 pass, 6 ignored. llvmpipe only (Xvfb, `xvfb-run cargo run -p d2-client -- play --frames 1000`, synthetic): 1,000 frames, 419 server ticks (~25/s at ~60 fps), `gpu: true`, node frames 999, exit 0; ignored GPU tests, `gpu_compare` 12/12 and `--perturb 7` exactly 7 re-run after the `device.rs` refactor; `verify::map::tests::gpu_map_matches_cpu_reference` (synthetic 100×70 map, chunk sides 32 / 64 / 1024) 0 differing, `--perturb 7` exactly 7 on CPU binned, GPU indices and GPU RGBA, `gpu_half` still 10/10; real `townN1` and a real GPU not run (§5 C24–C31). Coverage: audio 6 → 8 of 10 units, assets 8 → 9 of 12, ui 9 → 10 of 13 (unit tier); `bridge.md` §8 r1–r3 claimed on the frame loop; `render-pipeline.md` §a10 two claims (the map comparison and the exact `--perturb` count) |
| 6 Client, render composition and capture (`render-composition`, `render-capture`) | **Composition** (`render/composition.md` §2–§6, implemented in both compositors, byte-identical on llvmpipe, **unverified** against 1.14d: no capture): the pixel write is now `dest' = map[base + dest][src]` (**behavior change**: was `[base + src][dest]`; `MapTable::push_table` takes `table[dest][src]`, the PL2 layout, so a PL2 table is pushed unchanged; `verify/mod.rs` fills `[[table]]` rows as `table[dest][src] = rule(src, dest)` so existing case files keep their meaning); `scene::frame::FrameCycle` / `FramePlan` (persistent index framebuffer, `plan(blank_screen)` = `{clear_rows: H − 47 or 0, clear_after}`, `UNCLEARED_ROWS` = 47, `compose` / `commit`, `set_post_clear`); GPU params 32 → 48 bytes (`clear_rows`, `clear_after`) and a storage binding 9 `base`; `scene::present_palette(pl2)` (index i = `pl2[4i..4i+3]`, 0 included). Kept as TODOs: `composition.md` OQ2 (order of `L` and `T`), §7 DirectDraw (display type 3 only), §1, §3 r1 / r3 (`camera.md`, `draw-order.md`). **Capture** (`render/capture.md` §4–§8): case kind `scene` (`verify::capture`, `capture_case`, `capture_tests`; `crates/d2-client/capture-cases/{stability,placement,camera,composition}-0001.toml`, `raw = "latest"`): `check = "stability"` complete (PNG re-hash, `clear_counter > 0` frames all index 0, one `index_sha256` per state group, at least two keys seen at least twice); `check = "compare"` composes the recorded state with the CPU reference and compares indices (count + first mismatch) and the 768 palette bytes, then the GPU half, but **no `SceneSource` is wired**: `main.rs` passes `SceneNotWired`, so a compare case ends `SCENE NOT WIRED` (exit 2, never a pass) after its integrity checks. Implemented, **unverified** (never run on a recording) | composition: `cargo test -p d2-client`: lib 257 pass, 6 ignored on its branch; `gpu_compare` 12 → 18 cases (`frame-blank-screen`, `frame-no-clear`, `frame-post-clear`, `pixel-write`, `blend-ops`, `frame-stress`), 18/18 0 differing and `--perturb 7` 7 / 7 on llvmpipe; M08 by hand on the old read order; `composition.md` 9 of 16 units claimed. Capture: 17 tests on generated captures (`--perturb N` → exactly N on CPU indices, GPU indices and GPU RGBA; stability `--perturb` → N re-hashes); the PNG reader decodes the bytes `record_frames.py` writes for its selftest image; `capture.md` 6 of 9 units claimed |
| 6 Client, integration (`p6-integrate`, `p6-world-view`) | **World view** (`d2_client::world_view`, `p6-world-view`, d2rs-own, no original behavior): per frame bridge → UI (`run_ui`, intents forwarded) → `build` (tiles, units through `composite`, UI items, stable sort by key) → compose (GPU compositor on Bevy's device, else the CPU reference) → an 800×600 image on a sprite at the integer scale. Every original rule is a hook trait (`ViewRules`, `UiRules`; `Unspecified` draws nothing the model does not state and errors on a rule it lacks, never guesses); the hook table with owner specs is in the note. **Integration** (`p6-integrate`, wired, headless tests on synthetic data, live-file and real-GPU halves queued §5 C40–C44, **unverified**): `ViewAssets::frames: frames::FrameStore` (ids = the store's insertion order, `GpuAtlas` packs it incrementally, `ViewError::AtlasAhead`; `FrameTable` / `BoundFrames` removed); `verify` runs every case on one shared `verify::gpu::Wgpu` (`map::run_with`); UI text through `world_view::text_sprites` (`ui::text::layout_text`; hook trait `TextHooks`, `Unspecified` refuses); sound through `app::sound` (`GameAudio`, `PoolBank` → `SoundPool`; placeholders `NoSoundTable`, `NoWavDecoder`, `NoCues`, so the window plays nothing yet, by design); live levels: `GameData::Live(Arc<LiveData>)` = waypoint tables + `LevelTables` + `WorldFiles` + `ArchiveSet` (`GameData::select`: no directory or `--synthetic` → synthetic, a directory that does not load is an error), `WorldTypes` behind `SharedTypes`, acts 0 and 1 created with town level ids 1 and 40, init seed = `--seed` (game +0x7C, source unspecified, §7 PI1), Cold Plains generated and the first rooms of Cold Plains and Lut Gholein streamed. Not wired: the spawn room and position of a joining player, the waypoint object from level presets, every §B rendering rule (the window stays black), `app::Mode::Verify` and `render/` cleanup | world_view 9 tests (205 lib pass on its branch; golden FNV hash `0x03866d4be03b3500` of the CPU frame, perturbation tests, `gpu_packing_emulates_to_the_cpu_image`); p6-integrate: `app_frame_loop.rs` new `frame_loop_uses_the_frame_store_text_layout_and_sound_pool` and `placeholder_hooks_refuse_text_and_sounds`, `app_single_player.rs` `data_selection_falls_back_only_without_game_files`, `world_view/tests.rs` `frame_ids_are_the_frame_stores`; `sh tools/gate.sh` PASS, llvmpipe `adapter: llvmpipe (LLVM 20.1.2, 256 bits)`, `xvfb-run … play --synthetic --frames 600` exit 0 |
| 6 Client, specs landed from the local session (eighth fold; no handoff note) | **New specs, all draft, no claim yet**: `specs/ui/text.md` (+ `text-fonts.tsv`, 39 rule units: 1.14d text rendering: fonts, glyphs, colour codes, wrap, and the answer to CG2), `specs/render/draw-order.md` (23 units, items B6 and B10), `specs/render/unit-composite.md` (41 units, item B4) with `unit-directions.tsv` (449 lines), and a render follow-up pass over `camera.md`, `capture.md`, `composition.md`, `sprite-placement.md` (answers RC1–RC6, RW1, RW5, RN1–RN3, CP1–CP4: roofs use the floor drawer `0x004F68E0`, per-block wall culling (camera §7), exact shake arithmetic with no zero divisor reached (camera §8), composition §5 `T[256d + L[s]]` with P dropped, act load sets the clear counter, the capture's hooks / level / cursor rule / light quality / weather / draw log; **the "light flicker" is the cursor animation**) plus `record_frames.py` 0.2.0 (capture format 2, `frames-raw-2`, `--draws-every` draw log, celfile records, selftest with perturbation checks; `traces/FORMAT.md`). **Implementation sessions are running: `impl-ui-text`, `impl-draw-order`, `impl-unit-composite`, `render-followups-impl`**; their notes are the next fold's input. Until they land the code keeps its `TODO(spec)` hooks and the app's window stays black (§2 step 7q) | spec index check passes (`py tools/spec_index.py --check`); `py tools/coverage.py --summary` at this fold lists `ui/text.md` 0 of 39, `draw-order.md` 0 of 23, `unit-composite.md` 0 of 41 |
| 7–9 | deferred (out of current scope) | |

## 2. Next steps (in order)

Cloud (repo only):

1. **Seam providers and wiring in `d2-sim`.** Each Phase 3 group defined
   its own trait seams (`StatHost`, `units::UnitHooks`, `ItemStats` /
   `ItemGame`, `SkillUnits` / `CombatWorld`, `MissileUnits` /
   `MissilePath` / `MissileRooms` / `MissileCombat`, `AiUnits` / `AiModes`
   / `AiWorld`, `AiTargets`, `AiSkills`, `DropSink` / `GameFacts` /
   `Dropper` / `Recipient`, `LevelTypes` / `ActRooms` / `TileSource`,
   `WaypointWorld` / `CubeWorld` / `QuestWorld`); the provider of each is
   listed in the notes. Implement the providers in the order units →
   stats lists → items → combat → missiles/AI → drlg → world, and merge
   overlapping traits where two groups ask the same question (§3 "Seams").
   The event chain is wired in `wiring::action::ActionSim` (one
   `UnitSystem`-based `EventDispatch`, not the `MonsterDispatch {
   MissileDispatch { rest } }` chain; the two disagree on frozen type 10,
   §7 Integration I3 / W1). The tick hook DRLG asked for exists as
   `TickHooks::room_deactivated(game, act, room)` (step 9, default
   nothing). Move `d2-server`'s staged `PlayerFields` / `UnitFacts` into
   `d2-sim` as the unit specs land.
2. **Wiring, second pass** (the first pass is `d2_sim::wiring::economy`
   and `wiring::action`, §1 3j): combine them into one host (four separate modules now:
   `wiring/mod.rs` has `action`, `economy`, `interaction`, `worldgen`;
   `WorldSim` composes only `ActionSim` + `WorldState`, `interaction::Desk`
   borrows `Economy`, `QuestControl` and its own state, and `d2-server`'s
   `ItemWorld` owns a second `Units` / `StatLists` pair, §7 J1);
   give the game-creation fields one home (`GameFields` in economy,
   `ActionHooks::game_seed` in action, `GameInfo` in `monsters::population`
   and `monsters::init`; none on `Game`, §7 Integration I7); replace the
   `Pending` / `…Rest` defaults with providers in this order: path and position (`sim/path-placement.md` §7–§12 done, 3s; the core §1–§6 and `pathing.md` are step 7j), player data and inventory (`items::inventory` / `moves` done, not connected, step 7i),
   monster data (`monsters::init` `InitHost`, `population::PopWorld` /
   `MonsterInit`), items (`ItemStore` for items as `UnitId`s), objects /
   interaction / messages (the monster-data providers `PopWorld`,
   `MonsterInit`, `InitHost` exist in `wiring::worldgen`; `NpcWorld`,
   `NpcVendors`, `VendorWorld`, `UseWorld`, `UseMissiles`, `VitalsUnits` in
   `wiring::interaction`, each with the seams its note leaves open:
   `wire-worldgen.md` §5, `wire-interaction.md` §6, §3l); seed the game
   seed at game creation (W16). Provider order inside a group: the seam lists of §3 and the
   notes `wire-action.md` §4, `wire-economy.md` §5.
   **Seventh fold:** the combination step is done for the server host
   (`host-merge`, §1 row 3y: one host value, one unit store, one item
   store for the cube and the vendors, one home for the game-creation
   fields, one provider of the skill-use seams; §7 J1, J4, I7, W16
   resolved there). What is left of it is step 7o. The path / position
   provider exists (§1 row 3aa, opt-in per game, `enable_paths()`); the
   inventory provider exists (row 3z). Still `Pending`: objects, player
   data (HM3), item use, sockets, hirelings, stat-list routines,
   `InvRest` seams (`wire-inventory-sim.md` §5).
3. **`d2-server` intent handlers, remaining** (item / cube 0x2A, 0x4F,
   skill / combat and world ids are done, §1 3k). Open: providers so the
   NPC / vendor / quest ids leave their stubs (`WorldHost::npc` /
   `vendors` / `quests` on a wired host, written like
   `ActionWorld::waypoints`; the handlers need no change). **Quests done
   on `TradeWorld`** (`quest-host`, §1 3k; `ActionWorld` still stubs
   them, and the doc of `world/action.rs` lines 5–6 still says "the
   quests' ids stay stubs on both": it should read "on this host;
   `TradeWorld` adds them", fix it the next time that file is touched); an AnimData
   record routed in the wired sim (`UnitHooks::anim_record`,
   `wire-action.md` §4), else a mode start schedules no timers and no
   skill do runs from a message (§7 SK1); routing of `player_action_frame`
   (`0x00580460`) and timer types 5, 8, 9, 12, 14 to
   `use_::{attack_frame_event, active_state_event, periodic_event,
   item_aura_event}` and the cooldown expiry (where it lives is not
   decided, §7 J3); S→C layouts for 0x15, 0x5A, 0x3F, 0x9D in
   `server-messages.tsv`; `SimGame::tick` runs the default `Steps` hooks,
   not `ActionSim`'s `TickHooks` (§7 J2); give `TickHooks` a way to send
   messages. The ids with no owner spec stay stubs (§1 3k). Stale text:
   `SimGame::handle`'s doc still carries the `pierce_idx` TODO that
   `use_::handle_message` now does (`server-skills.md` §5 item 9); fix it
   the next time `adapters/sim.rs` is touched.
   **Seventh fold:** the item moves 0x16–0x29, 0x50, 0x61, 0x63 run on the
   wired host (§1 row 3z) and the quest ids run on `WiredWorld` (renamed
   from `TradeWorld`; `ActionWorld` still stubs them). Walk / run
   0x01–0x04 are the remaining stubs of the movement ids: turn the
   provider on at game creation (`ActionHooks::enable_paths()` before any
   unit allocation), give players a path (`View::allocate`, or
   `View::path_place` after the join puts the player in a room), call
   `wiring::path::walk::walk_message(v, game, player, id, a, b)` after
   `intents-events.md` §2.4 (returns `(0, Some(Outcome))`); step 7p.
4. **Bridge ↔ `d2-server` adapter, remaining** (the adapter is done as
   `d2_client::bridge::local::LocalLink` over `Host`, §1 row 5: `send` →
   `send_game` / `send_system`, `pump` → `frame`, `receive` →
   `receive(LOCAL_CLIENT)`; `bridge.md` open question 1 can be closed, §3
   rule 1 matches the code). Open: a constructor for a wired single-player
   game in the app (none is built; `BridgeResource` holds `Box<dyn
   ServerLink + Send + Sync>`, and whether a given `SimGame<D, W>` is `Send
   + Sync` depends on its providers, e.g. `DrlgWorld`'s boxed
   `TileSource` / `LevelTypes`: unchecked); the session code (game create /
   join / leave, a Phase 5 session spec: `PendingSession` records system
   messages 0x67..=0x70 and answers nothing, join is `SimGame::join`
   before the link is made).
   **Update (`p6-window`, done):** the app builds the game
   (`app::single_player::{start, build, LocalGame}`, `LocalSeams`) and runs
   it on a server thread (`app::server_thread::ThreadLink`, `Send + Sync`
   for any link); the `Send + Sync` question is answered: `SimGame<ActionSim<_>,
   ActionWorld>` is **not** `Send` (`DrlgWorld`'s boxed `TileSource` /
   `LevelTypes`, `SharedTypes` as `Rc<RefCell<_>>`), so the game never leaves
   its thread. Open: a `Send` game would need `d2-sim` changes (not
   needed now); the session spec; `bridge/local_tests.rs` still joins with
   `Some(rooms[0])` (runs ≤ 2 ticks; the app joins with `None`, PW1).
5. **Phase 6 in-app wiring: done** in the fifth fold (§1 row 6):
   render-graph node `compose_node` on Bevy's own device (owns the atlas
   array, re-uploads on a version change), `FrameStore`, the `map` case on
   `GpuCompositor` (`verify::map::run_with(…, &mut dyn GpuCompositor)`).
   Open (cloud, repo only): (a) delete the `Material2d` path: `render/`
   (`PaletteMaterial`, `palette.wgsl`), `PaletteRenderPlugin`,
   `app::Mode::Verify` / `VerifyConfig` / `compare_rgba` (nothing calls
   `Mode::Verify` now; `app.rs` still uses `PaletteMaterial` for `view`);
   (b) switch the world view to the store: `world_view::FrameTable` can
   become a `FrameStore` (or implement `FrameIds`), `UnitResolver` can drop
   `frame_id` and call `composite::build_with`; (c) `main.rs` should pass
   the runner's shared `Wgpu` to `verify::map::run_with` so one process
   opens one device (today the `map` case opens a second headless one);
   (d) atlas eviction (C2) is not wired (a full atlas is an error); (e) no
   input leaves the client (C9 controls are not wired to intents; no
   `UiRoot` in play mode); (f) prefetch wiring: the bridge has no "new
   active room / new unit type" report and no Bevy task runs the queued
   loads and `Pool::offer`; `SoundPool` and `AudioPlayer` are not wired in
   `app.rs`; (g) `world_view::ui_bind::UiRules::ui_text` can call
   `layout_text` once `ui/text.md` exists. Seams reached, not invented:
   every §B rendering rule (`Unspecified`), no S→C owner spec, the DRLG is
   synthetic even with `D2_GAME_DIR` (needs C23 and the providers in the
   app), the session spec. The two `p6-window` / `verify-map`
   questions are PW2 and VM1 (§7). **Sixth fold (`p6-integrate`):** (b),
   (c) and (g) are done (frame store in the world view, `verify` on one
   shared `Wgpu`, UI text through `text_sprites`), the sound pool is wired
   (`app::sound`, placeholder hooks) and the DRLG of `play` is live with
   `D2_GAME_DIR` (§1 row 6, integration; checks C40–C44). Still open: (a),
   (d), (e), the prefetch half of (f), the join-time placement (spawn room
   and position, `levels.md` §10, now `path::place::level_spawn_point` +
   `place_unit` in step 7j), the waypoint object from level presets (the app
   runs `ActionSim` + `ActionWorld`, not `WorldSim`), PI1 / PI2 (§7), and
   the frame cycle / `SceneSource` of step 7l.
6. **Coverage** (`coverage-claims` and eight gap-test sessions are done,
   §1; the rest is test and tool work). Next test-writing targets are the
   units no claim names (`py tools/coverage.py` lists them per spec) and
   the specs with the lowest any-tier share in `--summary` at `4b5b0bf`
   (`client/audio.md` 60.0%, `client/assets.md` and `data/schema.md`
   66.7%, `client/ui.md` 69.2%, `data/loading.md` 70.2%,
   `data/patch-layers.md` and `formats/animdata.md` 71.4%,
   `sim/intents-events.md` 72.6%, `sim/rng.md` 73.1%, `world/waypoints.md`
   77.0%, `sim/tick.md` 77.6%, `drlg/preset.md` 78.3%); most of what is
   left there is blocked on a spec (no `ui/text.md`, `formats/wav.md`,
   sound pool, prefetch; numbered rows in `rng.md` §5.2–§7) or a game run;
   the rules each gap session left, with reasons, are in `gaps-*.md`. Two
   policy / tool items from that session and `impl-vendors` (the same
   tool limit as (b) hits `affixes.md` §5, whose line ~152 wraps to start
   with `0.`, so `affixes.md §5 r0` cannot be claimed either,
   `gaps-items-stats.md`): (a) specs
   with prose-only or one-table sections (`damage.md` §8, `cube.md`
   edge-case bullets, `stats.md` §6.3, …) are one unit, so a claim can
   cover more than its test checks; the fix is numbered items in those
   specs (spec work), after which claims can be narrowed; (b)
   `tools/coverage.py` listed rule `§9.2 r0` of `vendors.md` (the list starts
   at `0.`) but rejected the claim as malformed: **done** in `gaps-data-rng`
   (`RULE_REF` accepts `r0`, `r00` / `r01` stay malformed, rule IDs
   unchanged, `price::rule_zero` claims `vendors.md §9.2 r0` and
   `rare_name_pick` claims `affixes.md §5 r0`; the "charges not all full"
   branch of `repairable` is not asserted separately, drop the claim if
   that counts). Rule: `Covers:` comments (`docs/COVERAGE.md`).
   Fifth fold: the 42 game-file tests of `game-tests-*` carry no claim until
   they pass locally (§5 C34, C35: the planned claims are in the notes; a
   claim added to a blind-written test is the §8 lesson of 2026-10-06);
   `client-own-gaps` raised audio / assets / ui to 8 / 9 / 10 units (still
   uncovered: audio §a1 text needs `formats/wav.md`, assets §a4 r1 is the
   render stage's list, assets §a6-writes, ui §a1, ui edge cases, bridge §1
   r2, §3 r4, §5 r1, §7 r4, §9 r2, §9 r3); `synthetic-data`: a narrower
   claim set for `mpq.md` §6–§10 needs numbered items in `mpq.md` (spec
   work); the `fuzz-data` properties claim nothing (robustness, not one
   rule each). Sixth fold: the 67 game-file tests of `game-tests-sim-core`,
   `-monsters-skills`, `-client-assets` (§5 C37, C45, C46) carry no claim
   until they pass; the claim table of `game-tests-monsters-skills` §4 (39
   tests, 277 game-tier units measured before the group C merge) and the
   `Intended claim` lines of the other two are the plan; `prop-sim-core` and
   `fuzz-server` add no claim (the properties overlap the unit-tier claims;
   `coverage-claims.md` §1 asks for a claim only where the assertions check
   the whole rule); `conformance-harness` adds none until a recording
   passes. `inventory.md` is at 139 of 148 units (the nine are in the two
   notes), `path-placement.md` 43 of 84 (§1–§6 belong to `impl-path-core`),
   `pathing.md`, `camera.md` and `sprite-placement.md` have no code
   (0 units); `composition.md` 9 of 16, `capture.md` 6 of 9. Specs lowest
   in `--summary` at `edd9925` by any-tier share (rule units ≥ 9): `pathing`
   0%, `camera` 0%, `sprite-placement` 0%, `composition` 56.2%,
   `path-placement` 51.2%, `capture` 66.7%.
7. **Level-type dispatcher: done** as `wiring::worldgen::levels::WorldTypes`
   (§1 3l, unverified): `DrlgError::LevelType(u32)`,
   `LevelTypes::door_unit(…, orientation)`, `MazePresets` and
   `OutdoorPresets` on the act's `Presets`, `Drlg::active_room_seed_mut`.
   **DS1 / DT1 providers: done** (`drlg-data`, §1 3o,
   `d2_server::world_data`, unverified until §5 C23). Open: use them in
   the app and the wired host (the app's DRLG is synthetic; no constructor
   builds a `SimGame` / `ActionSim` from a `FixedSet`, step 7e), then
   `wire-worldgen.md` §7 check 1 (the RNG trace compare of the preset draws,
   town and Blood Moor builds; the C23 tests compare outcomes, not the draw
   sequence); if a DS1 below v7 is ever needed `d2_formats::ds1` needs a
   raw-orientation option (its owner's change); the DRLG data population reads (coordinate lists, populated
   level, warp points, level spawn of kind 11, nearest free point: spec
   work, §5 B); `LevelTypes::warp_unit` (`0x0066E1C0`) and the type-2 tile
   free `0x00666610` (unspecified, nothing done); `DrlgData::wall_remap`
   stays `None` and `DrlgData::doors` empty (no spec transcribes them into
   a table view); outdoor level generation through the dispatcher is
   exercised only by the ignored test of C23 (§7 WG9); the live-table check
   is §5 C23 (the §5 Blocked entry keeps the RNG-trace part).
7b. **Providers for the new system modules: done** in `wiring::worldgen`
   (`population`, `init`) and `wiring::interaction` (`npc`, `vendors`,
   `skills::use_` seams, `combat::vitals`), unverified. **Routes: done**
   in the fifth fold (`wire-routing`, §1 3n) for `init_kind`, event 7, the
   umod modes 0, 1, 3, 5, `free_kind`, `monster_flag`, the monster level,
   and timer types 5, 8, 9, 12 (already routed). **Still open after
   `wire-routing`:** umod mode 4 (`0x005A43A0`, in the reaction
   `0x0057CEE0`; not placed by `damage.md` §7.1, RT3), AI state (RT4),
   `is_boss` / superunique / minion-owner getters (RT5), timer type 14 and
   `0x0056E900` (RT6); `d2-server` runs message handlers on `ActionSim`
   without the world lent (fix in `d2-server`'s file: run the handler
   inside `WorldSim::lend`, e.g. `ActionEvents::with_action(&mut self, f)`
   instead of returning the reference; J9); `wiring::economy::death.rs`
   reads `h.x.monster_flag` / `h.x.superunique` directly and should call
   `ActionHooks::monster_flag`; `QuestTick`'s `UnitSide for WorldSim` hands
   out the action hooks unlent (quest code that allocates or removes a
   monster would miss the world state; J10); J4 (below, `wire-routing` §6)
   proposes `UseView` as the one skill-use provider. The list below is what
   was open before the routes; read it with that update. To route
   (`wire-worldgen.md` §5, `wire-interaction.md` §6; the owners are
   `wiring::action` files those sessions did not touch): `ActionHooks`'s
   `init_kind` → `init::type_init` for monsters allocated by action code
   (needs `WorldState` reachable from `ActionHooks`); monster event 7
   (`UnitHooks::monster_umod`) → `init::handle_event7` and the umod
   callbacks of `init.md` §22 (mode change `0x005A7C20` → `dispatch(..,
   0 / 1)`, combat `3 / 4`, missile creation `5`); `MonsterStore::remove`
   and the minion / owner maps on unit removal (`ActionHooks::free_kind`);
   the action `Pending` monster-data queries (`monster_flag` `0x005A0180`,
   monster level, AI state), which `WorldState::monsters` can now answer;
   skill-use tick events 5, 8, 9, 14 through `UseView` (needs `X:
   UseRest`; proposed in `wire-interaction.md` §6, a different owner is
   proposed in `server-skills.md` §5 item 2, §7 J3); the kill →
   `kill_experience` (`Pending::reaction` has no provider; `vitals.md`
   OQ2; the e2e kill grants its level-up through the sim's own call,
   `e2e-next` frame 15); `QuestRest::mercenary_reward` →
   `NpcControl::quest_mercenary` (**done on `TradeWorld`**, `quest-host`:
   `EconomyQuests::mercenaries` collects, the host runs it after the call;
   `Desk::quest_message` keeps its own copy of the loop, §7 QH1); the first think
   of a created monster (§7 WG8); `NpcControl::new` steps the game seed
   once, third among the game-creation seeds, after the object-control seed
   and before `QuestControl::new`; `SkillFunctions` bodies
   (`use.md` OQ10).
7c. **One game, one unit world and one item store** (`e2e-next` §5, §7 J7 /
   J8, not done): the single-player e2e now runs on `SimGame<WorldSim<_>,
   TradeWorld<Rest>>` with the cube on `ItemWorld`, and the run shows the
   cost of the split: the cube's box, ring and amulet are not in
   `ActionSim`'s unit records and their creation steps `ItemWorld`'s game
   seed, not `ActionHooks::game_seed`; three `ItemStore` / `ItemTables`
   pairs exist (`DeathDrops::items`, `TradeWorld::items`,
   `ItemWorld::items`), so the dropped gold cannot be sold or cubed; the
   player's interaction (+0x64 / +0x68) has three owners (`Rest::interact`,
   `TestPending::interact`, `ItemWorld::staged.interactions`). Fix: one
   `ItemStore` + `ItemTables` per game, `ServerCube` and
   `handlers::items::handle` generic over the economy's hooks and sourcing
   the economy from the wired host as `TradeWorld::with_economy` does, with
   the cube parts (`CubeData`, `Staged`, `creation`, `ItemPending`) moved
   to the wired world host (`trade.rs`). The e2e steps that still stop at a
   seam: pick-up 0x16 (inventory / item-use spec), the item copy
   `0x0055A2A0` (buy, sell of a non-permanent item), the waypoint's
   same-act placement `0x00554EA0` (path spec), inventory placement /
   removal, session flow. Same-run tests: `e2e-next` asserts 26 frames and
   26 ticks (`cargo test -p d2-client --test e2e_single_player`, 3 tests).
   **Sixth fold:** `prop_worldsim` runs the same host (`SimGame<WorldSim<
   TestPending>, TradeWorld<Rest>>`) for hundreds of ticks and copies the
   ~1,000-line fixture of `e2e_single_player.rs` (PK3: move it into
   `e2e_support`); `e2e-combat-path` added the combat path to `ActionHooks`
   (anim record routing, `reaction` / `kill`, `monster_death_drop`) on this
   host. Two owners of the player's interact unit remain (EV1), duplicate
   game fields (EV2); `QuestCall` on `TradeWorld` is done (`quest-host`).
   Still stubs on the path: pick-up 0x16 (now `items::moves::pick_item`,
   unconnected, step 7i), the item copy `0x0055A2A0` (EV3), the same-act
   placement (now `path::place`, step 7j), the 0x27 text list `0x00661480`.
   **Seventh fold:** the split is closed on the server host (`host-merge`):
   one `ItemStore` + `ItemTables` for the cube and the vendors, one unit
   store, one game seed, one owner of the player's interaction on
   `WiredWorld`. Left of it: the drop's store (`DeathDrops::items`, inside
   `Pending`) is still separate, so the kill's gold cannot be picked up,
   sold or cubed (HM5 = IS4, now visible on the wire: 0x16 returns 1);
   the unique bits have two copies (HM4); the inventory has three copies
   (IS5). Step 7o.
7d. **Determinism lint: done** (`determinism-lint`, 2026-10-06):
   CLAUDE.md hard rule 6 is machine-checked by `cargo run -p depcheck`
   (§4); zero real hits on `d2-sim` at `edad871`, no allowlist entry. Open
   only as limits: the scan is lexical (a `HashMap` behind a type alias in
   another crate, a float from a dependency's API, `BTreeMap` iteration
   with a non-total key are not seen), and "no `HashMap` iteration
   affecting outcomes" is enforced as the stricter "no hash collections".
   `tests/` of `d2-sim` are not scanned.
7e. **Game from a `FixedSet`** (`synthetic-data` §6; **partly done, eighth
   fold** (`fixedset-game`): the DS1 / DT1 writers, the synthetic town and
   `test_fixtures::game::GameData` exist and the e2e reaches `SimGame::join`
   and 50 `Host::frame`s in CI, with `ActCreation::TownOnly`. **Left:**
   `ActCreation::Full` on synthetic data (a made-up levels table shaped like
   Act I, ids 1–39 with the chain's types and outdoor data, FG1); a message
   to the joined client (a join sequence, FG2); outdoor (lvlsub) and maze
   paths on synthetic data; folding `d2-server/tests/game_wired_host.rs` onto
   `GameData::load` (a refactor for that file's owner; §5 C60); the app's
   use of `world_data` with the user's files (step 7).) Original text: a
   constructor that builds `ActionTables`, `WorldTables` and the providers
   from a `FixedSet` would let the e2e test go on to `SimGame::join` and
   one `Host::frame` on synthetic data (and the app use `world_data` with
   the user's files, step 7); it also needs a DS1 / DT1 writer in
   `crates/test-fixtures` for a one-room preset town (both formats have
   specs). The `lvlprest` / `lvlsub` / `lvltypes` rows of the synthetic
   set name `.ds1` / `.dt1` paths that no archive holds.
7f. **CI and the gate: done** (`ci-speed`, §4): five parallel jobs and
   `sh tools/gate.sh` as the one local command. Open: the workflow itself
   was not run (YAML parses; the first run on GitHub is the check), the
   nextest paths are untested locally, and the timings in §6 are estimates
   (workflow logs were not reachable): read the first runs and replace
   them with measured numbers.
7g. **Decisions for the user: answered, done** (`client-own-gaps` §6; either choice keeps
   every image unchanged, the first only changes stalls): **CG1** should a
   prefetched pool entry rank below every used entry (always evicted first)
   instead of "last used by the previous frame"? **CG2** should
   `ui::text::TextOpts` carry the clip rect now (it is on `TextRequest`) or
   wait for `ui/text.md` to say what the original's text call takes? Until
   answered the code keeps the current reading (previous-frame rank; no
   fields in `TextOpts`). **Answered 2026-10-06** (`docs/PLAN.md`
   decisions log): CG1 keep the previous-frame rank; CG2 both,
   `TextOpts::clip` now carries the request's clip rect (unused until
   `ui/text.md` §B3 says what the original takes).
7h. **Spec questions of the fifth fold** (spec writing / Ghidra, §7 fifth
   set): RT1–RT6 (where the umod mode 0 / 1 calls sit in `0x005A7C20`,
   the monster-level getter, umod mode 4, `dwAiState`, the boss /
   superunique / owner getters, type 14), PW1 (can a room that still holds
   units be freed, `rooms.md` §7.2 / §8), VM1 (index 0 black or mapped,
   shown by §5 C28), FZ1 (`runtime-maps.md` §2 equivalence cycle), FZ2
   (`patch-layers.md` §9 D06 vs A12), DQ1 (`rooms.md` §9.3 case of the
   fixed library).

7i. **Connect inventory and item moves** (`impl-inventory`, `impl-moves`,
   §1 row 3r; cloud, repo only). `items::moves` needs three providers:
   `InventoryOps` over `items::inventory` (the signatures are in
   `impl-inventory.md` §3: `place_in_page`, `find_free_position`,
   `equip_from_cursor`, the belt functions, the six checks; the note
   `impl-moves.md` §4 lists every method, the inventory side owns
   `InvWorld`, so the adapter maps one trait onto the other and
   `InvItem` onto the wiring's per-item record), `MoveUnits` over the units
   and `ItemStore` of the one unit world (step 7c), and `MovePending` (its
   table groups the seams by owner: path / placement is now answered by
   `path::place` / `search` (step 7j), stat lists by `StatLists`,
   item creation by `Economy`, the rest stays `Pending`). Then a
   `d2-server` handler module for C→S 0x16–0x29, 0x50, 0x61, 0x63 (not 0x4C:
   `world/cube.md` §10) calling `items::moves::handle` and flushing
   `player_update`'s messages through the S→C builders of `d2_proto::s2c`
   and `moves::layouts`; the item bit stream of 0x9C / 0x9D (`inventory.md`
   OQ1) is the blocker for a byte-exact S→C (spec work, local). Open
   readings go with it: IV1–IV8, MV1–MV7 (§7). Unwired seams carry
   `TODO(spec: …)` at their sites; `moves` leaves OQ9, 10–17, 19 as seams.
   **Seventh fold:** the server adapter exists (`wire-inventory-server`,
   §1 row 3z): the 23 ids run through `WorldHost::moves` and the update
   pass runs after the tick. Open: IS1–IS6, WN1–WN4, the item bit stream
   of 0x9C / 0x9D (OQ1: the headers go out with the empty stream), the
   `InvRest` seams of `wire-inventory-sim.md` §5, and step 7o.
7j. **Path core, walking and the placement callers** (`impl-path-place`,
   §1 row 3s; cloud, repo only). Write `impl-path-core`
   (`path-placement.md` §1–§6: path records, collision, footprints, the
   `CollisionView` provider, units §2 `has_path` / `unit_size` /
   `unit_room`) and `impl-walk` (`sim/pathing.md`, 125 units, no code, no
   claim; `path/mod.rs` must be unioned with `place`, `place_seams`,
   `search`, `warp`); then switch the callers named in `impl-path-place.md`
   §2: the treasure drop seam `DropSink::place` (`treasure/walk.rs`,
   `wiring/economy/death.rs`, `treasure_items.rs`) → `floor_drop(…, size 1,
   fallback true)`, `LevelTypes::warp_unit` (`drlg/seams.rs`) →
   `warp_tile_preset`, the waypoint same-act placement
   (`world/waypoints.md` §7, step 7c) → `level_spawn_point` + `place_unit`
   (the e2e step 6 then sends the 0x0D), the population "nearest free
   point" (§1 3l) → `coarse_free_box`, the client join in `play`
   (`levels.md` §10). The three game-file / recording checks are §5 C38 and
   the R1–R3 replay (Blocked). PP1–PP7 (§7) decide the readings.
   **Seventh fold: done in `d2-sim` except the server walk handler.**
   `impl-path-core` (§1–§6), `impl-walk` (`pathing.md`) and `wire-path-sim`
   (the provider, §1 row 3aa) landed; the callers named above are switched
   when the provider is on: `Pending::warp` → `place::level_warp`, the
   waypoint arrival → `walk_to`, the population free point →
   `coarse_free_box`, the missile collision queries. Not switched: the
   treasure drop `DropSink::place` (WP3, below), `LevelTypes::warp_unit`
   and the client join in `play`. Step 7p.
7k. **Use the S→C builders** (`s2c-builders` §5; cloud). **Decision, eighth
   fold: the rule is kept** (`s2c-use`, option A; `docs/PLAN.md` decisions
   log): `tools/depcheck` `FORBIDDEN` has `("d2-sim", "d2-proto")` and the
   task text "depcheck allows it" was wrong. `d2-sim` keeps its byte
   builders; the one-maker property is a conformance test
   (`crates/conformance/tests/s2c_builders.rs`, 7 tests) and the bytes are
   pinned in `d2-sim/tests/s2c_bytes.rs` (row 3ai). **Done:** the 0x77 maker
   `cube::trade_action`; the check that the sim's builders and `d2_proto::s2c`
   cannot drift (J13). **Left:** `bridge-dispatch.tsv` owners stay `TBD` until
   a client spec owns an S→C id (S2C2; the typed variants are listed in
   `s2c-use` §3); the spec side below. Options B (lift the rule for
   `d2-proto`) and C (split `s2c` into a transport-free crate) stay open to
   the user, S2C1. Original text: `d2-sim` →
   `d2-proto` as a dependency (depcheck allows it) to replace the four byte
   builders (`world::npc::{transaction, service_result, resurrect_message}`,
   waypoints 0x63, cube 0x77, the 0x0D of the travel) with `d2_proto::s2c`;
   name `bridge-dispatch.tsv`'s owner rows with `s2c::Message` variants
   (the table is still all `TBD`); the bridge handlers can match on
   `Message`. Spec side: state in `intents-events.md` §6 that the only
   masked bytes of the exact comparison are 0x2A bytes 3–6 and 0x58 byte 6
   (SC1), and the `partial` rows' layouts (step 12).
7l. **Frame cycle and scene source in the client** (`render-composition`
   "Seams for others", `render-capture` open questions; cloud). The world
   view keeps a `FrameCycle`, composes with `plan =
   cycle.plan(level.blank_screen)` (CPU: `cycle.compose`; GPU:
   `pack(…)?.with_frame(cycle.pixels(), plan)`, read back, `cycle.commit`)
   and builds the palette with `present_palette`; `BlankScreen` from the
   player's `Levels.txt` row (all 137 live rows are 1); `scene::compose`
   still starts from index 0 (a `TODO(spec: render/composition.md)`), so a
   capture `SceneSource` must draw the whole frame or hand the previous
   image; the source needs a `ClientWorld` of the recorded state (CP1: the
   recording grows a level / room / unit snapshot, or the source replays
   the recorded game's seed through the local server). Anyone pushing PL2
   blend tables pushes them unchanged (`[dest][src]`); the `d2-formats` doc
   of `Pl2::alpha_blend` still says `[level][source]` (RN3, a doc edit in
   `d2-formats`, separate task).
7m. **Test follow-ups** (cloud, repo only): after a local pass add the
   claims of §5 C37 / C45 / C46 (claim tables in the notes); fold
   `prop_worldsim`'s fixture copy and `prop_handle`'s `TradeWorld` rests
   into one shared test-support module (PK3, FS2); a follow-up on the
   handlers' early refusals "returns before any effect", spec by spec
   (PK1); `prop-worldsim` bound the S→C comparison to the trade path (the
   action, skill, waypoint and tick paths send nothing, EC1 / EC4);
   `d2-net` has no code, so no `prop_*` there until Phase 7 (FS1); make
   `convert_tick.py` skip `anim` records so a `record_tick.py` 0.2.0
   recording converts to a `sim/tick` trace (CH2, tools owner).
7n. **Fixes the property tests deferred: done, eighth fold** (`prop-fixes`:
   `expire_lists` → `Result` with `EndlessExpiry`, stale `ListId` = null list,
   `TimerId` generation, `in_range` with `abs_diff`; row 3ah). **Left:** T1
   `bucket_slot` (left as is), PW1 `remove_active_room` (SL3: `rooms.md`
   §8.2 does not say which unit fields `0x0061A840` sets), SL1 / SL2 (the
   stale-handle and endless-expiry answers are d2rs's own, for the spec
   owner), the other 50-subtile copies in `skills/use_` and
   `world/waypoints.rs` still subtract `i32` (SL4). Original text: T1 `bucket_slot` still panics past 2^31 − 1 (documented,
   unreachable), `stat-lists.md` edge case 4 (an expired extended list
   panics), stale handles panic (`StatLists`) or hit the new timer
   (`TimerQueue::cancel` of a reused `TimerId`; no generation), the
   `ActRooms::remove_active_room` path still frees rooms that hold units
   (`TODO(rooms.md §8.2)`, PW1), `dispatch::in_range` subtracts `i32`
   positions without wrapping (positions are server-staged, FS3).

7o. **Items and inventory: one store, one inventory, one owner** (`host-merge`
   §4 W-1…W-6 = HM1–HM6, `wire-inventory-server` F1, F2, R1–R3 = IS4, IS5,
   IS1–IS3; cloud, repo only). **Eighth fold: F1 and F2 (IS4, IS5; HM5) are
   done** (`unify-items`, row 3af: one item store in `ActionHooks::items`, one
   inventory in `WiredWorld::inventory`; the vendor and cube answer from the
   model). What `unify-items` left: the vendors' remaining inventory calls
   (`take_from_cursor`, `unequip` by item, `can_belt` / `put_in_belt` with the
   vendors' arguments, `equip_ammo`, `find_tome`, `add_to_tome`,
   `find_partial_stack`, `lower_book_skill`: UI4), the vendor / gamble store
   inventories (`place_in_store`, `add_trade_inventory`,
   `new_store_inventory`: they need the store's grid record, UI5), the cube's
   inventory pass `0x0055FA40` and the item routines no spec writes
   (`duplicate`, `tempered_affix`, repair, recharge, quest hooks: UI6), and
   `ItemSpawn` carrying a position so `Economy::create_item` can run the
   §2.5 path part itself (DF2). Remaining from the earlier list: HM1 `UseView::start_mode` should run the mode start
   itself (`modes::set_mode` → `UseRest::clear_target` → `modes::animate` →
   clear flag 0x40, `use.md` §4) so the server's copy
   `skills::world::World::start_mode` goes; HM2 `NpcWorld::item_format` /
   `VendorWorld::item_format` should answer from the economy's fields
   (`ItemGame::item_format(econ.fields)`, game +0x78; the fixtures answer
   1, the fields give 2); HM3 a player-data provider in `d2-sim` owning
   +0x64 / +0x68 / +0x6C so `Pending::{set_interact, reset_interact,
   interact_guid, busy}` and `NpcRest`'s calls are one seam (then
   `HostWaypoints` goes); HM4 a game-level home for the unique bits
   (+0x1B24) reachable from both the death drop and the host's economy; HM6
   the init write of difficulty 2 (`worldgen::init_units` `set_difficulty`)
   also updates `UnitData::difficulty`. IS1 which +0xC8 bits the room
   clean-up `0x00553220` clears (read as bits 0 and 1); IS2 move
   `update_pass` into the tick hooks' client pass once `TickHooks` gains
   `send_unit_update` / `unit_update` bodies (today it runs after the tick,
   and reads the client's room after the tick's room switch); IS3 the
   ground items' unit update (§6.3) builds nothing on real units (WN1).
   Player creation must call `InvState::add_inventory(player,
   UnitKind::Player { class }, guid)` (`0x0063ABD0`; nothing in the server
   does yet).
7p. **Path: server handler, update-pass callers, three blockers**
   (`wire-path-sim` §5–§7, `impl-walk`, `impl-path-core`; cloud). **Eighth
   fold: (1) done** (`wire-path-server`, `path-update-pass`, row 3ag): the
   server walk / run handler runs C→S 0x01–0x04 on the path provider, the
   callers of `mode_update` / `reassign_flag` are in the update pass
   (`ActionSim::send_unit_update` → `update_messages`; the line "the update
   pass has no caller of `mode_update` yet" at the end of this step is
   stale), and e2e step 6 holds with the provider on (`e2e_walk.rs`).
   **WP2 done** (`drop-freespot`, row 3af; the hosts still do not load
   `PathState::field`, so the server keeps `FreeSpot` until a host loads
   `ExpField.D2` and enables paths). **Left:** WP1 (missile flight with the
   provider on), WP3 / WP4, the readings WS1–WS5 and PU1–PU2 (§7 eighth set:
   no 0x15 after a cross-level waypoint warp, flags never cleared, the second
   client's 0x0F per tick), rule 4 (monsters' 0x67 / 0x68), and the host
   loading the field. Original text: (1) The
   server walk / run handler (step 4 text above) and the callers of
   `walk::messages::{mode_update, reassign_flag}` in the server's update
   pass (`pathing.md` §10.2, §10.3); then d2-client e2e step 6 holds with
   the provider on. Blockers (each has a `TODO` at its site): **WP1**
   missile flight with the provider on (`pathing.md` OQ3:
   `WalkUnits::other_path_function` gives 0 points, so a built missile has
   no points and its first step expires it; hosts with missiles keep the
   provider off or fake via `Pending`); **WP2** the treasure drop's
   FreeSpot (`0x0064E810`): the provider exists
   (`wiring::path::place::floor_drop`, field `PathState::field` loaded by
   the host from `ExpField.D2`) but `FreeSpot` / `DropPlacer::place` run
   while `ItemDrops` holds the hooks, so the seam cannot reach the rooms:
   needs a seam change in `wiring/economy` (pass the rooms into
   `DropPlacer::place`; owner economy / inventory wiring), and
   `economy/death.rs` still reads `h.x.position` (→ `h.path_position`);
   **WP3** the teleport footprint room (`path-placement.md` §6 r4: the core
   clears and stamps from the path's room before the move, so a warp to a
   room not adjacent to it, e.g. waypoint travel, stamps nothing at the
   destination; spec question WP4). Also unspecified and left `Pending`:
   `0x006417F0` (target distance), `0x0054DC40` (teleport spot), AI path
   steps / blocked (flag 0x800 vs `0x00648EB0`) / stop / walk in radius,
   `units.md` §3.1 step 8 corpse settings, objects (no `objects.txt` in
   `ActionTables`: static path, no footprint), the removal's footprint
   clear conditions (`0x00649F50`, cleared unconditionally), the run stat
   list (`attach_run_stats`, `0x00620E80`), torso speed, door orientation,
   target lead (OQ4), re-path budget (OQ8), unit add / removal messages,
   AI room memo, life percent of 0x0D (`0x00621F20`), player data
   +0x148 / +0x14C, pets. Also: monster mode functions own §9.1 and call
   `Walk::step` / `movement` / `repath` / `target_check`; the update pass
   (`tick.md` §6 step 5) has no caller of `mode_update` yet (stale since the
   eighth fold: the callers exist for players, see the start of this step).
7q. **Render wiring: feed hooks and the capture's seams** (`render-wire`,
   `render-camera-placement`; cloud + local). **Eighth fold:** the specs that
   own most of these hooks landed (`ui/text.md`, `render/draw-order.md`,
   `render/unit-composite.md` + `unit-directions.tsv`, and the render
   follow-ups that answer RC1–RC6, RW1, RW5, RN1–RN3, CP1–CP4; row 6 "specs
   landed") and the implementation sessions `impl-ui-text`, `impl-draw-order`,
   `impl-unit-composite` and `render-followups-impl` are running: do not start
   the hooks they own; read their notes in the next fold and delete what they
   close here. The app's window stays black
   until the owners fill the `ViewFeed` hooks (RW2): local player position
   (S→C owner of positions), open mode (`ui/panels.md`), shake start
   (effect specs), player seed (camera OQ6), unit positions and offsets
   (`unit-composite.md`, camera OQ3), map tiles (`draw-order.md`, DRLG →
   client); `ViewSource` methods are `TODO(spec: …)` hooks that refuse
   until then. The capture's recorder gap (RW1, `scene_source::RECORDER_GAP`,
   capture.md §3 + `record_frames.py`): per frame, (1) act and level of the
   player (palette `pal.pl2`, `composition.md` §4; level tile files;
   BlankScreen), (2) the map tiles drawn (cell, list floor / wall / roof +
   DT1 roof height, DT1 file, orientation, main, sub index), (3) the client
   units drawn (type, GUID, position, the offsets of `0x004DA0B0 / D0 /
   F0`, mode, COF direction and frame, component tokens), (4) the UI drawn;
   draw order, shading and blend stay `TODO(spec)` after that. RW3 the
   frame-cycle base: `capture_case::compare_one` composes from index 0;
   once a world is recorded the CPU half must use `scene::compose_frame`
   with `FramePlan` and the previous captured draw as base
   (`SceneJob::previous`), and `verify::GpuJob` needs `base` + `plan`
   (`verify/mod.rs`, `verify/gpu.rs`). RW4 pause (draws every loop pass in
   the original, camera §9; d2rs draws only on ticks): the gate needs a
   "paused" input when pause is wired. RW5 recorded `player.client` (path
   +8 / +0xC) is read, not checked. Spec session: RC1–RC6.
7r. **Performance follow-ups** (`bench-baselines` §"Hot spots"; cloud):
   **BB1 done, eighth fold** (`mpq-huffman`, row 3ai: encoder, writer
   `Method::Huffman`, bench group `mpq_huffman`, decoder 2–3.7× faster on
   tables 0–3; the cause was the per-level SipHash map, not a rebuild per
   sector; two local checks, §5 C61 and C62; next steps if they pay: a compact
   node layout with u32 links, rebuilding the fast table after an escape, once
   C62 says which tables the `.wav` files use). Original text: BB1 MPQ Huffman sectors ran at about 3 MiB/s (78 ms for 256 KiB, ≈ 150 µs
   per 512-byte sector, ~80× slower than PKWARE; likely the adaptive tree
   rebuilt per sector) — measured once on an earlier branch, not kept: a
   Huffman case returns when `mpq::writer` has a Huffman encoder; then
   investigate; a loaded tick bench (the sim numbers are floor values):
   move the combat / missile fixtures of `e2e-combat-path` (in
   `d2-client/tests`) behind `bench-fixtures` and add a "fight" tick case;
   a real DCC / DC6 bench on live-shaped files (the synthetic DCC is
   shallow); the CPU compositor (4.75 ms / frame) is the reference only, so
   no change; no optimisation was made.
7s. **Nightly proptest job: done, eighth fold** (`ci-nightly-props`, row
   3ah: `.github/workflows/nightly-props.yml`, `tools/props-deep.sh`; `ci.yml`
   changed by a comment only; SF1 is answered by the job existing). Open: the
   `wire` and `worldsim` groups were not run at full counts (estimates only),
   and the job's first runs on the GitHub runners (1.5–2× slower than the
   measured 670 s) are the first real timing. Original proposal (`fix-statlist-prop` §5, `ci.yml` was not changed): do not set one fixed
   `PROPTEST_CASES` in PR CI (the variable overrides every property's own
   default in the crate; `ci.yml` keeps it unset), but add a scheduled
   (nightly) job running `PROPTEST_CASES=20000` on the state-machine
   properties alone (the stat-list property takes ≈ 60 s per run in a debug
   build) so this class of failure is found before a PR does. Decision for
   the user (SF1).
7t. **Test follow-ups, seventh fold** (cloud, repo only). (a) After the
   host merge, re-run `cargo mutants -p d2-server --timeout 60` and record
   the counts in §1 3ad (the 915-mutant baseline predates the merge); kill
   the 36 spec-decided survivors of `mutants-server` §(b)3 in the new
   layout: `skills/world.rs` (`has_player_data`, `last_point_frame`,
   `set_last_point_frame`, `in_own_inventory`, `same_act`, `endanim_expire`,
   `start_mode` flag clear, `is_alive`), `cube_world.rs`
   (`interacting_with_stash`, `trading`, `put_item_check` guard),
   `world.rs` `record_of` / `client_of` / `take_sent`, `world/wired.rs`
   `vendors` / `waypoints` / `fault`, `world/action.rs` `fault`; a second
   client and a second vendor record in the trade fixture kill `client_of`
   / `record_of`, a two-act staged fixture with a skill message on a unit
   kills `same_act` / `in_own_inventory`; `ds1_input` `version < 7` → `<=`
   (`world_data/mod.rs:104`, `drlg/preset.md` §5: a synthetic DS1 of
   version 7 kills it; MS1); the 259 forwarding-glue survivors need
   end-to-end scenarios and the 46 `seams.rs` survivors are gone with
   `SkillSeams`. (b) `prop-walk` gaps (§4 of the note): multi-room walks
   (room recache §9.6 r9, §9.8 messages, `OUTSIDE_ROOM`), missiles (§6 r3,
   §9.4), knockback (mode 19, type 8), monster re-path with a budget,
   moving target units, and `set_foot_mask`, `make_corpse_footprint`,
   `teleport` (no property). (c) `prop-client` §"Not changed": KP1, KP2.
   (d) After a local pass of C51, turn each `Claim once…` line of
   `game_inventory_path.rs` into a `// Covers:` claim (`inventory.md` §1.3,
   §1.2, §3 r1, §3 r4) and re-run `py tools/coverage.py --check`;
   `prop-inventory` and `prop-walk` may get claims from a docs session.
   **Eighth fold:** (b) is done except missiles, type 11 and monster circling
   (`prop-walk-gaps`, row 3ah: multi-room walks, knockback, chase,
   `set_foot_mask`, `make_corpse_footprint`, `teleport`; 16 rules claimed; PG1,
   PG2 in §7); (a) is still open (the eighth fold's mutation runs were on
   `d2-sim` and `d2-proto`, not `d2-server`). (e) `mutants-monsters-missiles` left 25 unobservable survivors, 17 of them
   the `InitHost` default bodies (`init/seams.rs`): a provider that
   answers them makes them observable. (f) Test code in `d2-client`'s
   `prop_worldsim` / `prop_handle` copies the e2e fixture (PK3, step 7m).
7u. **Follow-ups of the eighth fold** (cloud, repo only). (a) **`game_wired_host.rs`
   step 4 is stale since `wire-path-server`**: the file was written before
   C→S 0x01–0x04 had a handler and asserts that each 0x03 leg reaches the
   **stub** (`assert_stub`, `unhandled` = (client, 0x03, 5)), then runs the same
   request on `walk_message` itself. The live host enables the path provider
   (`enable_paths`), so the request now runs through `walk::run`, is not
   recorded in `unhandled`, and the second call moves the player twice. Update
   the leg (send the 0x03 through the host only, assert result 0 and the
   movement, drop the direct `walk_message`) **before** the local run of C59;
   steps 5 and 6 still reach stubs (no `SkillRest`, no inventory parts on the
   live host: GH3, GH4). (b) **`mutants-inventory`**: record `room_at`'s
   arguments in the shared `Fake` (like `spot_calls`) and assert (x + 2, y + 3)
   for the four `ground.rs:245` survivors of §9.1 step 2. (c) **`mutants-world`**:
   a game-tier test for `VendorTables::from_fixed` and its `From` projections
   (§5 C63); `cube.md` §7 (what a 19th filler does, MW1) and `npc.md` §7.2
   (the `first` argument of `send_hire_list` has no described effect, MW2) are
   spec work (step 14). (d) **Mutation runs not done yet**: `d2-server` again
   after the host merge (7t(a): the 915-mutant baseline predates it), `d2-data`,
   `d2-formats`, the `d2-client` rules; a mutation run takes about 2 h per 1,900
   mutants with `-j 3` on 4 cores: start it detached (`setsid nohup …`) and use
   `--iterate` (§8). (e) **Coverage grammar**: sections that are still one large
   unit (`data/field-types.md` §6, `drlg/rooms.md` §9.9, `missiles.md` §r11…,
   `drlg/maze.md` §3 text) take `<!-- rows -->` markers like `sim/rng.md`
   §5.2–§7 did; the `§edge-cases-original-bugs` claims of palette, cof, dcc,
   ds1, dt1, tbl, font-tbl, calc, audio, render-pipeline, capture, schema and
   assets were not narrowed (SU1); the rows of `rng.md` §5.2–§7 (38 of 39
   unclaimed) are trace material: a conformance check per row, or A recordings.
   (f) After the first red or green nightly (`ci-nightly-props`): record the
   real timings of `wire` and `worldsim` in §1 row 3ah and fix whatever the
   deep run reports as a counterexample (a regression test first, `fix-statlist-prop`).
   (g) `drop-freespot` DF1 / DF2 and `unify-items` UI1–UI7 are spec or wiring
   readings: the cloud work is in step 7o.


Local (spec / recording / game files; the coordinator batches them, §5):

8. **Phase 3 spec confirmations and recordings**: §5 groups A (needs the
   player), B (Ghidra / spec edits only), C (game files only). Units and
   stats recordings and the topic recordings first: they unblock
   "unverified" → "done" for 3c–3h.
9. **Client-model specs** (spec writing, high): what each S→C id means
   for the client; each sets its path as `owner` in
   `specs/client/bridge-dispatch.tsv` and registers handlers in
   `bridge::dispatch::HANDLERS` (the test `dispatch_table_matches_spec`
   enforces both). `bridge.md` open questions 2–5 go to them.
10. **Phase 6 RE specs**, in the order of `docs/PLAN.md` Phase 6: the first
    four are written as drafts (`render/{sprite-placement,camera,
    composition,capture}.md`, with the recorder `tools/trace-recorder/
    record_frames.py`; render §B1, §B2, §B7, §B9) and wait for their
    recordings (§5 A, `stability-0001` first) and for the answers RN1,
    RN2, CP1–CP4 (§7); then unit composite and draw order; shading, blend
    modes, lighting; audio; UI text, panels, controls, inventory, automap.
11. **Spec and code questions of the fourth fold** (spec writing / Ghidra,
    §5 B, §7): GD8–GD11 (patch-layers report order and A05, the `.bin`
    automap check, numbering the `rng.md` cross-reference tables), GT1,
    GP1, GB1, QH1 and EN1; plus the earlier GD items the new notes
    extend (`loading.md` §7.3 also omits `setitems.set` → `sets.index`).

12. **Spec text the sixth-fold audits found missing** (spec writing /
    Ghidra, `spec-bodies-*` §3; every entry needs the bar of `ai.md`
    §9.3–§9.13 / `missiles.md` §R4: the 1.14d body as a numbered step list
    with every branch, every draw in order and on which seed, every
    constant and column / skill-calc read, the seam calls with their
    arguments and the return value; then the row's `status` becomes
    `spec'd-here` and `implemented_matches_catalogue` /
    `bodies_match_catalogue_status` fail until the body lands). Counts:
    - **AI** (`ai-functions.tsv`, 148 rows; 17 `spec'd-here`): 1
      `summarized` (32 Npc `0x005E7130`: missing `0x005E6800`, the class
      cases 0xC9, 0xFE, 0xFF, 0x109, 0x200, `0x005E68F0`, `0x005E6AE0`,
      `0x005E7080` and the source of idle 10, `ai.md` OQ8), 11
      `D2MOO-only` (4 Bighead, 5 BloodHawk, 8 SandRaider, 10 CorruptRogue,
      11 Baboon, 15 SandMaggot, 20 Scarab, 33 HellMeteor, 37 SkeletonBow,
      43 FoulCrowNest, 59 BloodRaven; the Act I rows first, `ai.md`
      §9.14), 119 `unread` (list with addresses in the note), plus the
      special-state thinks of `ai.md` §3.2 states 2–17
      (`0x005B14E0`, `0x005E5870`, `0x005E52D0`, `0x005E7AC0`,
      `0x005E7C10`, `0x005E4CF0`, `0x005E7DC0`, `0x005E7F80`,
      `0x005E8020`, `0x005E8140`, `0x005E8340`, `0x005E5C50`,
      `0x005E2610`, `0x005E1D30`, `0x005E2D80`), their init functions
      (`0x005E5730`, `0x005E80E0`, `0x005E2CD0`) and every AI init /
      alternate function.
    - **Server-do** (`srvdo.tsv`, 53 rows): 7 `summarized` (8
      MonBlizzCenter, 10 BlizzardCenter, 17 CairnStones, 25
      EruptionCenter, 28 Volcano, 34 BaalTauntControl, 35
      RoyalStrikeChaosIce; what each lacks is in the note §3.4), 28
      `D2MOO-only` (§3.5). **Server-hit** (`srvhit.tsv`, 71 rows): 1
      `summarized` (58 BaalTauntLightningControl: the draws after the seed
      re-init, creation arguments, return value) and 52 `D2MOO-only`
      (§3.7). Priority (`missiles.md` OQ8): server-do 2, 3, 5, 7; server-hit
      1, 4, 12, 13.
    - **Quests** (`quests.md` §10, `quests.tsv`): the Act I callbacks still
      `unhandled`, per chain (1: events 0 / 10, active, seq, the ev 8
      helpers `0x00590190`, `0x00590080`, `0x005900E0`; 2: `0x00590B10`,
      `0x00590C10`, `0x00591080`, `0x00590EC0`, `0x00590FA0`; 3, 4, 5, 6,
      25 / 30, 37: the table of `spec-bodies-quests-skills.md` §3.1, with
      the 1.14d area levels, bits and states each lacks), the other hooks
      (§3.2: `0x00588C50`, `0x005BCFD0`, `0x0059D6A0`, `0x005991B0`,
      `0x0059C3B0`, `0x005467E0`, the per-class `0x005449E0` functions),
      and Acts II–V (§3.3): 31 records catalogued only (`quests.md` §11,
      OQ8; chains 7–13, 26, 27, 14–20, 28, 21–24, 29, 31–36, 38, 39) and
      chain 40 (Act V intro, init `0x0058EA50` not disassembled, OQ6).
    - **Skill functions** (`functions.tsv`, `use.md` OQ10): every filled
      `srvst` slot but 18 (63 of 64: whole body; many have no Ghidra
      function at the entry, `use.md` OQ1) and every filled `srvdo` slot
      (§4 of the note, with the partial knowledge per slot: `srvdo` 65
      BasicAura has §7's duration, stat sources, range calc, filter column,
      mana formula but not the `aurafilter` bits, target iteration order,
      stat-list fields or the failed-mana behaviour; 67 / 76 only the
      re-request rule; 116 only the free-while-shapeshifted mana rule;
      145–147 only the dispatch). Priority (`use.md` OQ10): `srvdo` 1, 2,
      65, 30, 18, then the `srvmissile` path.
13. **Spec confirmations of the sixth fold** (spec writing / Ghidra /
    recordings; each answer is in §7 under its ID): the partial S→C rows
    of `s2c-builders.md` §5 item 1 (0x15 sender `0x0053BC10`, 0x51
    `0x0053BD10`, 0x5A `use.md` OQ9, the 0x27 34-byte text list
    `0x00661480`, 0x50 mercenary form, 0xAC bit positions `init.md` §24,
    the item bit stream of 0x9C / 0x9D `inventory.md` OQ1), SC1–SC4,
    PB1; inventory (`inventory.md` OQ1, 6, 9–17, 19; IV1–IV8, MV1–MV7);
    path placement (`path-placement.md` OQ1 position trace, OQ3, OQ5;
    PP1–PP7, `pathing.md`); render (`composition.md` OQ1 palette source,
    OQ2 `L` / `T` order, §7; `capture.md` OQ1–OQ2; RN1–RN3, CP1–CP4);
    the harness gaps CH1–CH4 (a rooms full-check recorder extension,
    `convert_tick.py` and `anim` records); the `tick.md`, `stat-lists.md`
    wording points PS1–PS4; WO1 (`npc.md` §8.2 vs `vitals.md` §2.1 on
    `0x00570C80`), WO2 (`0x0058F870` body); EC1–EC4 (`damage.md` OQ3, OQ7,
    `vitals.md` OQ2).

14. **Spec confirmations of the seventh fold** (spec writing / Ghidra /
    recordings; each answer is in §7 under its ID): the per-tick position
    recording (`pathing.md` OQ1, `path-placement.md` OQ1: hook `0x00650840`
    entry and exit, per call frame the unit type, GUID, precise x / y
    16.16, point index, count, points, velocity, path flags; placement
    calls `0x0064DEA0`, `0x00554EA0`, `0x00555DA0` and monster creation
    with room and point; or the 0xAC / 0x9C position bytes in
    `server-messages.tsv`; CR1, CR2); `pathing.md` PQ1–PQ9, GR1, PX1,
    OQ3 (missile path function), OQ6 (status routine `0x00548760`, owner of
    the 0x96); `path-placement.md` PC1–PC5, WP4, WP5, PF1 and the map seeds
    of recordings R1 / R2 (GX4); `camera.md` / `sprite-placement.md`
    RC1–RC6; `capture.md` RW1; `inventory.md` WN1–WN3, IS1, IS2, PN1,
    GX1–GX3, OQ1 (the item bit stream); host wanted changes HM1–HM7 are
    code, not spec. Reconcile first: RC1 (roof drawer), WN1 (unit flag
    0x10 vs §6.3), PQ1 (V1–V3 rows), GX1 (record 29).

Done (kept for the record): step "tick trace replay" (`claude/phase3-tick-replay`,
2026-10-06: `conformance::tick::replay`, d2-sim `UnitLists::alloc_unit` /
`add_allocated`, `Game::alloc_unit`, findings TR1–TR4 in §7); step "wire the
`d2-server` seams" (`claude/phase3-wiring`, 2026-10-06: `ProtoSizes`,
`SimGame<D: EventDispatch>`, `PlayerFields` / `UnitFacts` staging,
`SizeError::Negative` / `Classified::NegativeSize`, no change outside
`d2-server`); Phase 3 system implementations (one session per spec group,
notes in `docs/handoff/impl-*.md`, including `impl-skilluse-vitals`,
`impl-monster-init`, `impl-monster-population`, `impl-drlg-{preset,maze,
outdoor}`, `impl-vendors`, `impl-npc`); wiring (`wire-economy`,
`wire-action`: the first pass of step 1); `coverage-claims` (step 6); Phase 6
C1–C10 (`docs/handoff/p6-*.md`, including `p6-gpu`, `p6-verify`, `p6-cof`,
`p6-verify-gpu`); the `d2-server` handler sessions (`server-items`,
`server-world`, `server-skills`: step 3 for the ids that have an owner spec),
the second wiring pass (`wire-worldgen`, `wire-interaction`: steps 2, 7, 7b),
`p5-local-server` (the adapter of step 4), the four gap-test sessions
(`gaps-items-stats`, `gaps-combat-ai`, `gaps-data-formats`,
`gaps-drlg-world`: step 6) and, in the fourth fold, `quest-host` (step 3,
quests; step 7b, mercenary reward), `determinism-lint` (step 7d), the
four later gap-test sessions (`gaps-drlg-sim-world`,
`gaps-combat-items-monsters`, `gaps-data-rng` incl. the `r0` claim,
`gaps-client-formats`: step 6) and `e2e-next` (the single-player e2e on
`TradeWorld`, step 7c opened); in the fifth fold `p6-window` (steps 4, 5:
the app on a window), `verify-map` (step 5: frame store, `map` case),
`wire-routing` (step 7b routes), `drlg-data` (step 7 providers),
`synthetic-data` (7e opened), `ci-speed` (7f), `fuzz-data` (the
`equiv_matrix` hang), `client-own-gaps` (step 6: three d2rs-own rules with
code; 7g) and the two `game-tests-*` sessions (step 6, §5 C34, C35); in the sixth
fold `impl-inventory` + `impl-moves` (7i opened), `impl-path-place` (7j
opened), `s2c-builders` + `proto-bits` (7k opened), `conformance-harness`,
`render-composition` + `render-capture` (7l opened), `p6-integrate` (step 5
(b), (c), (g)), `p6-world-view`, `prop-sim-core` + `fuzz-server` +
`prop-worldsim` (7m, 7n), `game-tests-{sim-core,monsters-skills,
client-assets}` (step 6, §5 C37, C45, C46), `spec-bodies-*` (step 12) and the
four older notes `wire-open-seams`, `e2e-single-player`, `e2e-combat-path`,
`e2e-vendor-host` (steps 2, 3, 7b, 7c).
In the seventh fold `host-merge` + `host-merge-port` (steps 2, 3, 7c on
the server host), `wire-inventory-sim` + `wire-inventory-server` (7i
adapter and handlers), `impl-path-core` + `impl-walk` + `wire-path-sim`
(7j: core, walk, provider; the server handler is 7p), `render-camera-placement`
+ `render-wire` (7l in part: camera, placement, the feed and the scene
source; 7q), `conformance-path-render` (harnesses; recordings §5),
`bench-baselines` (7r opened), `prop-client`, `prop-inventory`,
`prop-path-place`, `prop-walk`, `fix-statlist-prop` (7s opened),
`mutants-server`, `mutants-monsters-missiles`, `gaps-path-render` and
`gaps-new-specs` (step 6) and `game-tests-inventory-path` (§5 C51).

Per-session notes live in `docs/handoff/*.md` (detailed record of seams,
public APIs, design choices and per-site open questions). The ones for the
97 sessions above are folded into this file as of this commit (all notes);
read the note of a module before you change it. Every note in `docs/handoff/`
starts with the pointer line.

## 3. Code map

| Path | What | Spec |
|---|---|---|
| `crates/d2-formats/src/mpq/` | MPQ archive (`mod.rs`), hash/block tables (`tables.rs`), crypto, PKWARE explode, Huffman, ADPCM, `ArchiveSet` (`set.rs`) | `formats/mpq.md`, `mpq-tables.md`, `data/loading.md` §2 |
| `crates/d2-formats/src/{palette,dc6,dcc,dt1,ds1,cof,tbl,font,animdata}.rs` | file formats | `specs/formats/*.md` |
| `crates/d2-data/src/txt.rs` | strict `.txt` reader, column binding | `data/txt-format.md` |
| `crates/d2-data/src/schema.rs` | embedded `fields.tsv` / `tables.tsv` | `data/schema.md` |
| `crates/d2-data/src/compile.rs` (+ `compile/tests.rs`) | cell → bytes, linkers, `strkey`/`calc`/`param` | `data/field-types.md` |
| `crates/d2-data/src/compile/callbacks.rs` (+ `callbacks/tests.rs`) | cube, skill-mode, composit, place callbacks; `@uniques`/`@sets` | `data/callbacks.md` |
| `crates/d2-data/src/codegen.rs`, `tables/` | typed record structs: generator, generated `tables/generated.rs` (don't edit; `data-tool gen-tables`), `Record`, `decode_all` | `data/schema.md` |
| `crates/d2-data/src/links.rs` (+ `links/tests.rs`) | cross-reference validation: linker sizes, broken links by table/row/column | `data/field-types.md` §6.7 |
| `crates/d2-data/src/fixup.rs` (+ `fixup/{records,maps,text,qsort,tests}.rs`) | post-load fix-ups (`records`), runtime maps (`maps`), wide text and tile paths (`text`), the CRT `qsort`; on a copy of the loaded set; `apply(&BinSet, &AnimData)` | `data/fixups.md`, `data/runtime-maps.md` (summary `loading.md` §7.4) |
| `crates/d2-data/src/calc.rs` (+ `calc/tests.rs`) | formula compiler, code buffers | `data/calc-expressions.md` |
| `crates/d2-data/src/compile_set.rs` | all tables in load order | `data/loading.md` §6–7 |
| `crates/d2-data/src/bin.rs` | `.bin` container, live-file resolution, load checks | `data/loading.md` §3–4, §8 |
| `crates/d2-data/src/crosscheck.rs` | txt → bin byte comparison | `data/loading.md` §11 |
| `crates/d2-data/src/patch.rs` (+ `patch/{syntax,apply,diff,check,tests}.rs`) | mod patch layers: parse, apply, render/digests, diff, patched compile (branch `claude/patch-layers`) | `data/patch-layers.md` |
| `crates/d2-formats/src/robust.rs`, `crates/d2-data/src/robust.rs` (copy) | robustness test harness: `bounded` (deadline + panic capture), `mutated` / `bytes` strategies; properties in `d2-formats/src/{robust_tests.rs,mpq/robust_tests.rs}`, per-parser `robust` test modules, `d2-data/src/{robust_tests.rs,calc/robust_tests.rs,patch/robust_tests.rs}` | METHODS M07 |
| `crates/d2-data/src/strings.rs` | string tables, `strkey` | `field-types.md` §7 |
| `crates/d2-client/src/map/` | DS1+DT1 map assembly, CPU reference renderer | `render/map-preview.md` |
| `crates/d2-client/src/{app,assets,render}` | Bevy app, `mpq://` assets, palette shader | `render/map-preview.md` |
| `crates/d2-sim/src/rng.rs` | seeded RNG: `Seed`, draw helpers, `derive`, `time_value` | `sim/rng.md` |
| `crates/d2-sim/src/tick/{mod,timer,events}.rs` (+ `tick/tests.rs`) | `tick()` step order and passes, `TickHooks` / `EventDispatch` hooks; timer queue (`TimerQueue`: schedule, every-tick, cancel helpers, cursor run); event types and §5.6 dispatch data | `sim/tick.md` |
| `crates/d2-sim/src/units/lists.rs` (+ `lists/tests.rs`, `lists/alloc.rs`) | GUID counters, hash lists, room/act/update-queue/client lists (`UnitLists`); unit allocation apart from `SUNIT_Add` (`alloc.rs`) | `sim/unit-order.md` |
| `crates/d2-sim/src/game.rs` | `Game`: frame counter, lists, timers; spawn/remove unit, schedule, hash iteration helpers | `sim/tick.md` §2, §5; `sim/unit-order.md` §2.5, §3 |
| `crates/conformance/src/{trace,rng}.rs` (+ `tests/rng_traces.rs`) | trace loading and top-level checks; RNG trace replay | `traces/FORMAT.md`, `sim/rng.md` |
| `crates/conformance/src/tick.rs` (+ `tests/tick_replay.rs`, `tests/tick_traces.rs`) | tick trace replay through `d2_sim::tick::tick`: hooks per step, run comparison, list snapshots, owed list primitives; perturbation tests | `sim/tick.md`, `sim/unit-order.md` |
| `crates/d2-proto/src/{schema,transport,wire}.rs` | message descriptors and size rules; size lookup per direction, C→S classifier, S→C buffer split; LE reads/writes, `FixedMessage` | `sim/intents-events.md` §2–3 |
| `crates/d2-proto/src/{tsv,codegen}.rs`, `generated.rs` | strict TSV parser + TSV-vs-code check (`tsv::check`); generator; generated `CLIENT_MESSAGES` / `SERVER_MESSAGES` and typed `client::*` / `server::*` (don't edit; `data-tool gen-proto`) | `sim/intents-events.md` §5 |
| `crates/d2-server/src/seams.rs` | the traits `d2-server` needs from `d2-proto` / `d2-sim` / session code (§2 step 4) | `sim/intents-events.md` |
| `crates/d2-server/src/adapters/{sizes,sim}.rs` | the seams on the real crates: `ProtoSizes` (`d2-proto` size lookup), `SimGame<D = Unspecified, W = NoWorld>` (`Intents` + `Tick` on `d2_sim::game::Game`; staged `PlayerFields` / `UnitFacts`; fields `items: Option<ItemWorld>`, `world: W`, `skills: Option<Box<dyn SkillHost<D> + Send + Sync>>`; `handle` tries items, world, skills, then the stub) | `sim/intents-events.md` §2–§4, `sim/tick.md` §3, `sim/unit-order.md` §7 |
| `crates/d2-server/src/adapters/handlers/mod.rs` | handler modules `items`, `skills`, `world`, one `pub mod` line each | `sim/intents-events.md` |
| `crates/d2-server/src/adapters/handlers/items.rs`, `items/{cube_world,tests}.rs` | `ITEM_IDS` (checked by a test), `handle` (0x2A, 0x4F), `ItemWorld` (units, stat lists, unit data, hooks, `GameFields`, `ItemTables`, `ItemStore`, `CubeData`, `Staged`, `ItemPending`), `ItemView`, `ItemError`; `ServerCube` (a `CubeWorld` forwarding item, stat and creation calls to `EconomyCube`, the rest from `Staged`, the `cube.md` §2 checks and `ItemPending`; `InfoRest` answers `player_info`); 10 host-frame tests | `world/cube.md` §1, §2, §8 |
| `crates/d2-server/src/adapters/handlers/skills/{mod,wired,world,seams,tests}.rs` | `IDS`, `SkillHost<D>` (object-safe), `Call`, `Staged`, `Handled`, routing `handle`; `WiredSkills<S>` (`SkillHost<ActionSim<X>>`: `run`, `add_skill_point`); `World` (`SkillUnits`, `ManaUnits`, `SkillFunctions`, `UseMissiles`, `UseWorld`, `LearnUnits`, `VitalsUnits` on the game, the wired unit system and staged facts); `SkillSeams` (every part with no provider, defaults = nothing); 11 tests through `Host::frame` | `skills/use.md` §1, §7; `skills/levels.md` §6.4; `combat/vitals.md` §2; `intents-events.md` §2.4 |
| `crates/d2-server/src/adapters/handlers/world.rs`, `world/{action,tests/}.rs` | `WORLD_IDS` (checked against `client-messages.tsv`), `system`; seam `WorldHost<D>` with the visitor traits `NpcCall`, `VendorCall`, `WaypointCall`, `QuestCall`; `NoWorld`; `WorldError` / `WorldFault`; `handle` (player lookup, module call, send routing, result mapping); `ActionWorld` (`WorldHost<ActionSim<X>>`: waypoints), seam `Outbox`; 25 tests (`waypoints`, `npc`, `vendors`, `quests` on the seam fake `fake.rs`, `ids`) | `world/{npc,vendors,waypoints,quests}.md`, `intents-events.md` §2.4, §3.2 |
| `crates/d2-server/src/adapters/handlers/world/trade.rs`, `world/tests/trade_quests.rs` | `TradeWorld<R>` (`WorldHost`; `e2e-vendor-host`: `npc` on `Desk`, `vendors` on `VendorDesk`, `waypoints` delegated to `ActionWorld`, `TradeRest`, `Parts`, `TradeWorld::with_economy`; `SimGame::with_world`): `quests` (real `QuestControl` on `EconomyQuests`, the mercenary routing); `trade_quests`: 7 tests on `SimGame<ActionSim, TradeWorld>` through the host frame, staged `Rest` | `world/quests.md` §7.3, §6.2, §1.7; `world/npc.md` §7.5 |
| `crates/d2-server/src/transport.rs` | client duplicate filter, classifier, three server queues, drain (truncating copy) | `sim/intents-events.md` §2.1 |
| `crates/d2-server/src/dispatch.rs` | game message entry, dispatcher, gate, stubs, exact size, point/unit parse, chat check, 0x3C/0x51 decode | `sim/intents-events.md` §2.2–2.4 |
| `crates/d2-server/src/buffers.rs` | per-client 0x200-byte buffers, local delivery split, receive lists | `sim/intents-events.md` §3 |
| `crates/d2-server/src/host.rs` | tick driver, `Host::frame` (drain → tick → flush), flush, injectable `Clock` | `sim/tick.md` §1, §8; `sim/intents-events.md` §1 |
| `crates/d2-server/src/tests/` | vectors; `fakes.rs`: TSV-driven size fake, fake game; `adapters.rs`: the adapters, fake agreement, end-to-end host frame | |
| `crates/d2-client/src/bridge/` | client↔game boundary: `link` (`ServerLink` trait, `SendQueue`, `Pumped`), `intent` (encode + classifier routing), `receive` (split, dispatch, `ReceiveLog`), `dispatch` (TSV table, `HANDLERS`, `check`), `world` (`ClientWorld`, `UnitKey`, `addressed_unit`), `mirror` (Bevy `BridgePlugin`, `BridgeResource`, `UnitView`), `mod.rs` (`Bridge`: `send`, `send_bytes`, `frame`); `local` (`LocalLink<G, S, H, C>`: `ServerLink` on `d2_server::host::Host`, `SinglePlayer<G>` = `ProtoSizes` + `SystemClock`, `LocalLink::single_player`, `last_frame()`, `PendingSession` placeholder `SessionHandler`); tests in `tests.rs`, `local_tests.rs` | `client/bridge.md`, `client/bridge-dispatch.tsv` |
| `crates/d2-sim/src/stats/{mod,ops,lists,states}.rs` (+ `stats/tests.rs`) | stat ids, keys, `StatTable` (from the fixed-up itemstatcost), `muldiv`, `by_time`, op table (`OP_ROWS`, checked against the TSV); `StatLists` arena (readers, evaluation, writes, propagate/recompute, value-change callback, attach/detach/free, equip, expiry, mod array; seam `StatHost`); `StateTable` and state queries | `sim/stats.md`, `sim/stat-lists.md`, `sim/stat-ops.tsv` |
| `crates/d2-sim/src/units/{record,lifecycle,anim,modes,dispatch,hooks}.rs` (+ `units/tests.rs`) | `UnitRecord` / `Units`; `allocate` (`0x00555230`) / `remove` (`0x00555600`); §4.2 animation schedule and every-tick movement; set mode, prepare animation, player and monster modes, join, missile setup; `HANDLERS` (checked against the TSV), `UnitSystem` (an `EventDispatch`), regeneration; `Sim`, `UnitData`, seam `UnitHooks` | `sim/units.md`, `sim/unit-handlers.tsv`, `sim/unit-events.tsv`, `sim/stat-lists.md` §10 |
| `crates/d2-sim/src/items/{mod,tables,create,quality,affixes,props}.rs` (+ `items/tests/`) | `create_item` pipeline (`0x00558D90`), `ItemTables::from_fixed`, ratio row / quality roll, affix rollers, property functions (`FUNCS` mirrors the TSV), runewords, set bonuses, craft lists; seams `ItemStats`, `ItemGame` | `items/generation.md`, `quality.md`, `affixes.md`, `properties.md`, `property-functions.tsv` |
| `crates/d2-sim/src/treasure/{mod,runtime,walk,softfloat,quality,drop}.rs` (+ `treasure/tests.rs`) | TC runtime form from typed records, the TC walk, NoDrop (integer binary64), drop quality ladder, monster / chest drops; seams `DropSink`, `GameFacts`, `Dropper`, `Recipient`, `MonsterRank` | `items/treasure.md`, `treasure-quality.tsv`, `treasure-chest-acts.tsv` |
| `crates/d2-sim/src/skills/{mod,levels,special,calc}.rs` (+ `skills/tests.rs`, `fake.rs`) | skill level / bonus, specials (`SKILL_SPECIALS`, `MISS_SPECIALS` checked against the TSVs), damage and mana formulas, to-hit, learning (0x3B validator), runtime formula evaluator; seams `SkillUnits`, `ManaUnits`, `LearnUnits`, `KickItems` | `skills/levels.md`, `skillcalc.tsv`, `misscalc.tsv`, `data/calc-expressions.md` §3 |
| `crates/d2-sim/src/combat/{mod,hit,damage}.rs` (+ `combat/tests.rs`) | `pct`, hit chance / block / dodge, damage record, resistances, leech, stun / freeze / poison, crushing blow, open wounds; seam `CombatWorld` | `combat/hit.md`, `combat/damage.md` |
| `crates/d2-sim/src/drlg/{mod,data,seams,level,room,active,tiles,collision}.rs` (+ `drlg/tests/`) | act DRLG, levels, warps, rooms and rooms-near, active rooms and adjacency, tile library and choice, collision grids; seams `LevelTypes`, `ActRooms` (impl for `UnitLists`), `TileSource` | `drlg/levels.md`, `drlg/rooms.md` |
| `crates/d2-sim/src/missiles/{mod,create,flight,hit,catalogue,seams}.rs` | missile creation, init, class handler, default flight, hit handler, pierce, server-do / server-hit catalogues (checked against the TSVs, stubs log), `MissileDispatch` (an `EventDispatch`); seams `MissileUnits`, `MissilePath`, `MissileRooms`, `MissileCombat`, `MissileHooks` | `missiles/missiles.md`, `srvdo.tsv`, `srvhit.tsv` |
| `crates/d2-sim/src/monsters/ai/{mod,target,tactics,functions,table,seams}.rs` | AI think scheduling, dispatch and prechecks, control tables, target search, distances and commands, 17 AI functions; `AI_TABLE` (checked against the TSV), `MonsterDispatch` (an `EventDispatch`); seams `AiUnits`, `AiModes`, `AiWorld`, `AiTargets`, `AiSkills` | `monsters/ai.md`, `ai-functions.tsv` |
| `crates/d2-sim/src/world/{mod,waypoints,cube,quests}.rs`, `world/quests/{tables,act1}.rs` | waypoint records, menu and travel; cube recipes and transmute; quest control, flags, chains, Act I callbacks; embedded `quests.tsv` / `quest-messages.tsv`; seams `WaypointWorld`, `CubeWorld`, `QuestWorld` | `world/waypoints.md`, `cube.md`, `quests.md` (+ TSVs) |
| `crates/d2-client/src/assets.rs`, `assets/{path,tbl,size,cache}.rs` | canonical `mpq://` paths (`CanonicalPath`, `read_asset`), `FileSource`, `MemorySource`; loaders `pl2`, `cof`, `tbl` (`TblAsset::{Font,Strings}`); `ByteSize`; residency `Pool<K,V>` (byte budget, deterministic LRU, `resolve`, stall metric) | `client/assets.md` §A1–A2, §A4–A5 |
| `crates/d2-client/src/frames/{mod,atlas,upload}.rs` | `IndexFrame`, `FrameSet::{from_dcc,from_dc6,from_dt1}`, `FrameSetKey`; shelf-packed `Atlas` (2048² R8 pages, 1-pixel gutters, `check`); `AtlasTextures`, `upload_dirty` (Bevy edge) | `client/render-pipeline.md` §A2, `assets.md` §A3 |
| `crates/d2-client/src/scene/{mod,item,order,bins,cpu}.rs` | `DrawItem`, `ShadeChain`, `BlendOp::{Opaque,IndexTable}`, `MapTable`, `DrawKey` stable sort, 32×32 bins, CPU reference `compose` / `compose_binned` / `compose_rgba` | `client/render-pipeline.md` §A3–A9 |
| `crates/d2-client/src/ui/{geom,frame,edge,draw,panel,root,widget}.rs` | UI core: `Panel` / `UiRoot`, widgets, integer hit tests, frame-coordinate mapping `Presentation` / `to_frame`, `UiDraw` sink, intents forwarded through `Bridge::send_bytes` | `client/ui.md` §A2, §A4 |
| `crates/d2-client/src/controls/{mod,names}.rs` | `d2controls 1` strict TOML parser (`toml_edit`), writer, `dev` preset, clash check, `migrate`; closed `Key` / `Action` / `Context` lists | `client/ui.md` §A6 |
| `crates/d2-client/src/audio/{mod,mixer,log,output}.rs` | trigger queue, tick scheduler, integer mixer (Q8 gains, 32.32 phase), voice log `d2rs-audio-log 1` (writer, strict parser, `compare_logs`), rodio `MixerStream` output edge | `client/audio.md` §A2–A5 |
| `crates/d2-sim/src/skills/use_/{mod,table}.rs` (+ `use_/tests.rs`) | skill use pipeline: seams `UseWorld` (extends `UseMissiles` + `SkillFunctions` + `ManaUnits`), `UseMissiles`, `SkillFunctions`; §1 validators and handlers (`validate_point`, `validate_unit`, `handle_message`, `handle_hold`, `MsgResult`); §2 `dual_wield`, `use_at_point`; §3 `range`, `use_on_unit`; §4 `can_change_mode`, `interrupt_gate`, `set_mode_with_skill`; §5 `skill_mode`, `frame_events`, `start`, `mana_check`, `do_skill`, `do_core`; §6 `set_delay`, `cooldown_blocks`; §7 `schedule_periodic`, `periodic_event` (type 8), `item_aura_event` (9), `active_state_event` (5), `select_skill` (0x3C); `table::FUNCS` (64 srvst + 152 srvdo filled slots, checked against the TSV by `check_tsv`) | `skills/use.md`, `functions.tsv` |
| `crates/d2-sim/src/combat/vitals/mod.rs` (+ `vitals/tests.rs`) | seam `VitalsUnits`; `VitalsTables` (`charstats`, `experience`; `max_level`, `threshold`, `level_from_exp`); `init_player_stats`, `set_experience_for_target_level`, `handle_add_stat_point` (0x3A), `spend`, `gain_energy`, `gain_vitality`, `reset_stats`, `level_up`, `level_factor`, `add_experience` | `combat/vitals.md` |
| `crates/d2-sim/src/monsters/init/{mod,calc,create,umods,message,seams}.rs` (+ tests) | `MonsterData`, `MonsterStore`, `CreateRequest`, `GameInfo`, `InitTables` / `Ctx`; `pct`, `player_bonus`, `monster_level`, `stats_by_level`, `classic_scaling`; `create`, `type_init`, `stats_and_skills`, `components`, `monprop`, `monequip`, `normal_mods`, `boss_mods` (bloodraven only), `assign_umod`; `UMODS` (from `umods.tsv`), `AURAS`, umod choice / init / minions / superunique / restore, dispatcher (`dispatch`, `handle_event7`; five bodies, other callbacks log `Unhandled::Callback`); `unique_name`, 0xAC init-owned fields (`assign_mode`, `component_bits`, `write_boss_section`, `BitWriter`); seam `InitHost` | `monsters/init.md`, `umods.tsv` |
| `crates/d2-sim/src/monsters/population/{mod,data,region,room,spawn,placement,preset,seams}.rs` (+ tests) | `PopState`, `room_step` / `populate_once`; `PopTables` (+ `with_bins`); `Regions::create`, region list and variants, §13 counters; `populate_room`, `pick`, `boss_or_pack`, `ambient`; `random_boss`, `boss_spawn`, minions, `pack`, `party`, tentacles; `place` (`0x005B2A00`), `place_near`, `spawn_point`, `ring_search`; `place_presets`, `preset_spawn`, `SPECIAL_PRESETS` (checked against the TSV), `spawn_mode_xy`; seams `PopWorld`, `MonsterInit` (= `PopHost`). Spec names `monsters::placement`, the code `population::placement` | `monsters/population.md`, `preset-monsters.tsv` |
| `crates/d2-sim/src/drlg/preset/{mod,data,ds1,map,room}.rs` (+ `preset/tests.rs`) | `Presets`, `PresetCtx`, `PresetUnit`, `PresetMap`, `PresetRoom`; `PresetData`, `PresetTables` (embedded `preset-tables.tsv`, strict parse, door lookup); `Ds1Input` / `Ds1Source` seam, `Ds1File`, `Ds1Cache` (process-wide); DrlgType 2 `init_level` / `generate` / `reset_level`, `alloc_map`, `build_area`, `add_preset_units`; `room_grids`, `door_unit`, `tombstones` | `drlg/preset.md`, `preset-tables.tsv` |
| `crates/d2-sim/src/drlg/maze/{mod,specials,cells,layout}.rs` (+ `maze/tests.rs`) | `MazeData` (lvlmaze, lvlprest `Files`, specials), seam `MazePresets`, `Maze` (`init_level`, `generate`, `reset_level`, `free_level`; not a `LevelTypes`); strict parser of `maze-specials.tsv` (`include_str!`); cells, links, geometry, `Gen`; `generate` (§4) with ring, grow tree, hub, spiral, special builders, lair, tombs, temple, normalize, theme pass, build + file rotation | `drlg/maze.md`, `maze-specials.tsv` |
| `crates/d2-sim/src/drlg/outdoor/{mod,grid,place,vertex,wild,acts,rooms,tilesub}.rs` (+ `outdoor/tests.rs`) | `Outdoor`, `OutdoorData`, seams `OutdoorPresets`, `SubFiles`, `OutdoorTypes` (a `LevelTypes` adapter); grid, build list, placers; link tables, driver, linkers, Place A/B/C, Act I flags, adjacency warps; vertex polygon, borders; Act I wild (cliffs, river, bridge, cottages, path search); Acts II–V (`acts.rs`: II and IV as specified, III partial, V siege strip); generation `0x00675360`, cells → rooms; lvlsub rows, border and room substitution, sub-theme pick | `drlg/outdoor.md`, `outdoor-tilesub.md` |
| `crates/d2-sim/src/world/vendors.rs`, `vendors/{price,store,gamble,trade}.rs` (+ `vendors/tests/`) | per-NPC switches (`column_of`, `GAMBLERS`, `FLAGGED`, `HIRE_CLASSES`, `REPAIRERS`), `VendorTables::from_fixed`, `GlobalLists`, `VendorRecord`, seams `NpcLink`, `VendorWorld`, `Transaction` (0x2A); `cost` (`0x0062EFB0`), `gamble_price`; `generate` (`0x00576980`), `make_store_item`, `open`, `refresh_act`, `level_changed`, `client_left`; gamble list `make_list`, `drop_list`, `identify_gamble` (0x37); `buy` (0x32), `sell` (0x33), `repair` (0x35), `BuyMsg` / `SellMsg` / `RepairMsg` | `world/vendors.md`, `vendors.tsv` |
| `crates/d2-sim/src/world/npc.rs`, `npc/{hire,services}.rs` (+ `npc/tests.rs`) | role lists, `vendors.tsv` embedded parser, `NpcRecord`, `NpcControl` (`new` = `0x00536070`), `InteractionList`, seams `NpcWorld`, `NpcVendors`; handlers `interact` (0x13), `chat_open` (0x2F), `chat_close` (0x30), `menu_action` (0x38), `identify` (0x34), heal; builders `transaction` (0x2A), `service_result` (0x58), `resurrect_message` (0x9B); hire list, `hire` (0x36), `resurrect` (0x62), `quest_mercenary`, `hire_init`; imbue, socket, personalize, respec, act travel | `world/npc.md`, `vendors.tsv` |
| `crates/d2-sim/src/wiring/mod.rs`, `wiring/action/{mod,dispatch,units,combat,missiles,ai,rooms,waypoints,pending}.rs` (+ `action/tests/`) | `ActionTables`, `DrlgWorld`, `ActionHooks`, `View`, `WiringError`; `ActionSim` (the one `EventDispatch` + `TickHooks`: steps 9, 10, client room change); `UnitHooks` + `LifecycleHooks` + `StatHost` for `ActionHooks`; `CombatView` (`CombatWorld` + `SkillUnits`); `MissileWorld`; `AiHost`; DRLG lookups and room step bodies; `WaypointView`; `Pending` (every seam call without a provider, narrowest default; `NoPending`) | `tick.md` §3, §5.6, `units.md`, `hit.md`, `damage.md`, `missiles.md` §R2–§R6, `ai.md`, `rooms.md`, `waypoints.md` |
| `crates/d2-sim/src/wiring/economy/{mod,game_fields,item_stats,item_units,treasure_items,cube_items,quest_items}.rs` (+ `economy/tests/`) | `EconomyError`; `GameFields` (game seed, +0x6D, +0x70, +0x6A, +0x74, +0x1B24; `ItemGame`, `treasure_facts`); `UnitStats` / `StatCtx` (`ItemStats` on `StatLists`, `find_list`); `Economy` (`create_item`, `free_item`, `with_item`, `ItemStore`, `ItemSpawn`); `ItemDrops` (`DropSink`, seam `DropPlacer`); `EconomyCube` + `CubeRest`; `EconomyQuests` + `QuestRest` | `items/generation.md`, `properties.md`, `treasure.md`, `cube.md`, `quests.md`, `units.md` §3 |
| `crates/d2-sim/src/wiring/worldgen/{mod,levels,maze_presets,outdoor_presets,population,population_init,init_units,dispatch}.rs` (+ `worldgen/tests/`) | `WorldgenError`, `WorldPending` (seams without a provider), `WorldTables`, `WorldState` (regions, monster data, minions, superunique tails), `WorldHost`; `WorldTypes` (the act DRLGs' `LevelTypes`, dispatching by `DrlgType` to `Maze` / `Presets` / `Outdoor`; `AllocView`, `SharedTypes` = `Rc<RefCell<WorldTypes>>`); `MazeToPreset`, `OutdoorToPreset`; `PopWorld`, `MonsterInit`, `InitHost` on `WorldHost`; `WorldSim` (`ActionSim` + `WorldState`: `EventDispatch` and every `TickHooks` method to the action systems; `populate_monsters` → `room::populate_room`) | `levels.md` §3.7, §4.3, §5.2, §9.4; `maze.md` §7, §9; `preset.md` §4, §6; `outdoor.md` §12.1; `population.md` §3, §6, §8, §9, §11; `init.md` §4–§6, §10, §14, §16–§20; `tick.md` §3, §4 |
| `crates/d2-sim/src/wiring/interaction/{mod,npc_world,npc_vendors,vendor_world,skill_use,vitals}.rs` (+ `interaction/tests/`) | `InteractionState` (one `VendorRecord` per NPC record, the NPCs' `InteractionList`s, errors), `Desk` (borrows `Economy`, `QuestControl`, `VendorTables`, the state, the rest, host ms), `PlayerQuestsRef`, `InteractionError`; `NpcWorld` for `Desk` (`NpcRest`); `NpcVendors` for `Desk`, `VendorDesk` with `NpcLink`, entry points `buy` (0x32), `sell` (0x33), `repair` (0x35), `identify_gamble` (0x37), `level_changed`, `client_left`; `VendorWorld` for `VendorDesk` (`VendorRest`); `UseView` (wraps `CombatView`: `SkillUnits`, `ManaUnits`, `SkillFunctions`, `UseMissiles`, `UseWorld`; `ActionSim::skill_use`; `UseRest`); `VitalsView` (`VitalsUnits`, `kill_experience`, `VitalsRest`) | `npc.md` §1–§8; `vendors.md` §3–§9; `use.md` §1–§7; `missiles.md` §R2; `vitals.md` §1–§4 |
| `crates/d2-client/tests/e2e_single_player.rs`, `tests/e2e_support/mod.rs`, `tests/e2e_vendor.rs` | the single-player e2e (3 tests; 26 bridge frames: creation, join, cast, kill, level-up, 0x3A, pick-up stub, Akara talk / chat / trade / buy / sell, cube put-in and transmute on `ItemWorld`, waypoint) and its shared fixtures (`Rest`, `item_tables`, `vendor_tables`, `monstats`, `equiv`, `tx`, `blank`, moved unchanged out of `e2e_vendor.rs`, 4 tests) | `e2e-next.md`, the three e2e notes |
| `crates/d2-sim/src/drlg/outdoor/linker_tests.rs`, `units/gap_tests.rs`, `treasure/gap_tests.rs`, `combat/hit_gap_tests.rs`, `skills/levels_gap_tests.rs`, `monsters/population/gap_tests.rs` | fourth-fold gap tests (test-only; the `#[cfg(test)] mod` lines sit in `place.rs`, `treasure/mod.rs`, `combat/mod.rs`, `skills/mod.rs`, `monsters/population/mod.rs`) | `drlg/outdoor.md` §2.4, `sim/unit-order.md` §1, `items/treasure.md`, `combat/hit.md`, `skills/levels.md`, `monsters/population.md` |
| `crates/d2-data/src/{gaps_loading_tests,gaps_patch_fixup_tests}.rs`, `crates/d2-client/src/{bridge/tests,ui/tests}.rs` + tests in `assets/cache.rs`, `map/layout.rs`, `d2-formats/src/{cof,dcc,ds1}.rs` | gap tests for `data/{loading,fixups,runtime-maps,callbacks}.md` (synthetic `patch_d2.mpq`) and for bridge §1 / §2 / §5–§8, ui, assets, map-preview, cof, dcc, ds1; the game half is `d2-formats/tests/formats_game.rs::ds1_layer_limits_and_truncated_trees_groups` (`#[ignore]`, §5 C20) | `data/*`, `client/*`, `formats/*`, `render/map-preview.md` |
| `crates/d2-client/src/gpu_compositor/{mod,pack,device,harness,tests}.rs`, `compositor.wgsl`, `examples/gpu_compare.rs` | `SHADER`, `WORKGROUP` = 16, `GpuError`; `pack` (validate + serialize, re-bins with `scene::bin`), `Packed`, `GpuItem`, `Params`, `SlotSource`, `emulate` (the shader on packed bytes, CPU), `AtlasFrames`; WGSL `compose` + `to_rgba`; `Gpu` (`headless`, `from_device`, `compose`, `compose_rgba`; limits checked before dispatch); 12 synthetic cases, `compare`, `diff`, `perturb`, `Report`; CLI `gpu_compare [--case] [--perturb N]` | `client/render-pipeline.md` §A9, §A10 |
| `crates/d2-client/src/verify/{mod,case,map,gpu,tests}.rs`, `render-cases/*.toml` | runner over versioned case files: `GpuCompositor` trait, `GpuJob`, `GpuOutcome::{Image{indices,rgba}, NotWired, NoAdapter, Error}`, `build`, `run_cpu`, `compare`, `compare_indices`, `perturb`, `perturb_indices`, `Status`, `Summary` (exit codes 0 / 1 / 2); case format v1 strict parser (`[[frame]]`, `[[map]]`, `[[table]]`, `[[item]]`, `[[unit]]`, `[[expect]]`); `map.rs` = today's map verify; `gpu.rs` `Wgpu` (lazy headless adapter, two dispatches must agree); cases: `map` + 10 `synth-*` | `client/render-pipeline.md` §A10, `render/map-preview.md` |
| `crates/d2-client/src/composite/{mod,tests}.rs` | `Slot`, `slot_order` (§A7 step 2, strict COF checks), `ComponentRequest`, `ComponentFrame`, `ComponentResolver` (hooks: `frame`, `frame_id`, `place`, `shade`, `blend`), `UnitParams`, `ComponentDraw`, `build`, `CompositeError` | `client/render-pipeline.md` §A7, `formats/cof.md` |
| `d2-net`, `d2-verify` | stubs | |
| `tools/mpq-tool` | info, list, extract, check, formats, render | |
| `tools/data-tool` | `tables`: the Phase 2 cross-check; `links`: broken links in the live set; `gen-tables`: regenerate typed structs; `gen-proto`: regenerate `d2-proto`'s message tables from the TSVs; `dump-compare`: fix-ups vs a 1.14d memory dump; `patch check/render/diff`: mod stacks | `data/field-types.md` §6.7, `data/fixups.md`, `data/runtime-maps.md`, `data/patch-layers.md` §10 |
| `tools/trace-recorder` | Python debugger recording RNG draws from `Game.exe` (Windows); `dump_tables.py`: the excel tables and runtime maps in 1.14d memory after the load; `record_tick.py` + `check_tick.py`: server tick, timer events, unit lists; `record_packets.py` + `check_packets.py`: client↔server messages | `sim/rng.md`, `traces/FORMAT.md`, `data/runtime-maps.md`, `sim/tick.md`, `sim/unit-order.md`, `sim/intents-events.md` |
| `tools/trace-recorder/convert_tick.py` | tick recording → format-1 trace `traces/sim/tick/sim-NNNN.json`; `--check` replays traces through the `check_tick.py` model (CI) | `sim/tick.md` Test vectors |
| `tools/trace-recorder/{check_units,record_stats,check_stats}.py` | per-kind event rules U1–U11 on tick recordings; stat-list recorder (`stats-raw-1`) and checker | `sim/units.md`, `sim/stat-lists.md` |
| `tools/depcheck` (+ `src/determinism.rs`, `determinism-allow.txt`) | dependency rules (no Bevy outside `d2-client`; `d2-sim` must not depend on `rand` / `getrandom`) and the determinism lint over the non-test `.rs` of `crates/d2-sim/src` (rules `float`, `hash-collection`, `clock`, `ambient-rng`, `io`, `env`, `global-state`, `unsafe`; allowlist lines `file<TAB>rule<TAB>reason`, a missing reason or a stale entry is an error; `#[cfg(test)]` items and files skipped) | CLAUDE.md hard rule 6 |
| `tools/methods.py` | methods collection `docs/METHODS.md`: `check` (CI), `list`, `new`, `export` | `docs/METHODS.md` |
| `tools/spec_index.py` | section indexes in specs (`--check` in CI) | `specs/README.md` |
| `tools/coverage.py` | spec rule coverage (claim grammar accepts `r0`): rule IDs from specs, `Covers:` claims from tests and checks, unit / game-file / trace table, uncovered list; `--check` (CI), `--selftest` | `docs/COVERAGE.md` |
| `crates/d2-sim/src/wiring/action/monsters.rs` | `MonsterWorld<X>` (type init, umod dispatcher, forget, monster data, `into_any`), `umod_mode`, `ActionHooks::{with_monster_world, monster_data, run_umods, monster_flag}`; `ActionHooks::monster_world` is the lent slot (`monster_world_out`) | `monsters/init.md` §5, §22; `sim/units.md` §3.1, §3.2, §4.6 |
| `crates/d2-sim/src/wiring/action/{units,combat,missiles,ai}.rs` | routes: `init_kind` (monster type init), `monster_umod` (event 7), `free_kind` → world forget, `View::monster_set_mode` → umod modes 0, 1, `CombatView::monster_hit_hook` → mode 3, `View::unique_mod_missile` → mode 5, `monster_flag` / `is_unique` / `is_champion` / `monster_level` from monster data | `init.md` §7, §22; `damage.md` §5.2; `missiles.md` rule 28; `ai.md` §2.4 |
| `crates/d2-sim/src/wiring/worldgen/{monster_world,events,dispatch}.rs` | `MonsterWorld` for `WorldState`; `WorldState::forget`, `WorldSim::{lend, with, remove_unit}`; `WorldSim` runs timer events and forwarded tick hooks with the world lent (`lent!`); `WorldHooks` removed | `init.md` §5, §22; `units.md` §3.2; `tick.md` §3–§5 |
| `crates/d2-sim/src/wiring/worldgen/tests/routing.rs` | 7 route tests (declared from `worldgen/mod.rs` with `#[path]`) | |
| `crates/d2-server/src/world_data/{mod,tables,archive}.rs` | `WorldDataError`, `archive_name`, `ds1_input`, `sub_file`, `tile_info`; `Ds1Files`, `Dt1Files`, `SubFileMap` / `WorldFiles::load`; `LevelTables::from_fixed`; `archive::load` over `ArchiveSet` | `drlg/preset.md` §5, §13; `outdoor-tilesub.md` §1; `rooms.md` §9.3; `fixups.md` §12; `maze.md` §1; `data/loading.md` |
| `crates/d2-server/src/world_data/tests.rs`, `tests/game.rs` | 8 synthetic DS1 / DT1 tests; 3 ignored game tests (C23) | |
| `crates/d2-formats/src/mpq/writer.rs`, `writer_tests.rs` | `MpqWriter` (`new`, `sector_size_shift`, `hash_table_count`, `with_listfile`, `add`, `add_file`, `add_deleted`, `to_bytes`, `write`), `FileOptions`, `Method`, `Pkware`, `WriteError`, `implode`; behind feature `test-support` (`mpq::crypto::encrypt` is compiled under it too) | `formats/mpq.md` §1, §4–§10 |
| `crates/test-fixtures/src/{synth,content,install,tbl,animdata}.rs`, `tests/{synthetic_load,server_tables}.rs` | the synthetic install: `TxtFile`, `TableSet`, `StringSet`, `synthetic()`, made-up rows (`content.rs`), `install::build` → `Install { dir, archives, compiled, loaded }`, `.tbl` and `AnimData.d2` writers; 12 end-to-end tests. Use from `[dev-dependencies]` only | `data/schema.md`, `txt-format.md`, `loading.md`, `formats/{tbl,animdata}.md` |
| `crates/d2-data/tests/prop_data.rs` (+ `prop_common/mod.rs`), `crates/d2-formats/tests/prop_more_formats.rs`, `crates/d2-sim/tests/prop_tables.rs` | property tests (10 + 8 + 3); `d2-sim` gained `[dev-dependencies] proptest`; `fixup/maps.rs` `equiv` carries a step budget | `data/runtime-maps.md` §2, `patch-layers.md` §9 |
| `crates/d2-client/src/app/{server_thread,single_player,play}.rs` | `ThreadLink<L>`, `ThreadStopped`; `Sim`, `Link<C>`, `LocalSeams`, `WaypointTables::{synthetic, live}`, `GameData`, `build`, `LocalGame`, `start`, `BuildError`, constants `COLD_PLAINS`, `ACT2_TOWN`, `DEFAULT_SEED`; `add_game`, `unspecified_palette`, `PlayConfig`, `run`. `main.rs`: `play` subcommand (default; `--seed`, `--frames`, `--synthetic`) | `client/bridge.md` §3, §7, §8; `world/waypoints.md` §5.1, §6; `render-pipeline.md` §A1, §A9 |
| `crates/d2-client/src/world_view/{node,present}.rs` | `ComposeJob`, `NodeRuns`, `add_node`, render-graph system `compose_node`; `WorldViewGpu { atlas, pages, sets, seq }` packs and inserts a `ComposeJob` (GPU path); `GpuAtlas::sets()` | `render-pipeline.md` §A1, §A9 |
| `crates/d2-client/tests/{app_frame_loop,app_single_player}.rs` | headless frame loop and GPU node test; thread / build tests | |
| `crates/d2-client/src/gpu_compositor/device.rs` | `Gpu::atlas_texture`, `Gpu::encode_rgba` (shared private `encode` with `run`; behavior unchanged) | `render-pipeline.md` §A9 |
| `crates/d2-client/src/frames/store.rs`, `store/tests.rs` | `FrameStore` (`insert`, `id`, `frame`, `owner`, `contains`, `len`, `frames`, `atlas`), `StoreError::{Duplicate, NotResident, Index, Full}`; `scene::FrameSource` impl | `render-pipeline.md` §A2, §A7 step 3; `assets.md` §A4 |
| `crates/d2-client/src/composite/mod.rs`, `store_tests.rs` | `FrameIds`, `build_with`; `ComponentResolver::frame_id` has a refusing default | `render-pipeline.md` §A7 |
| `crates/d2-client/src/verify/map.rs`, `map/tests.rs`; `verify/mod.rs` | `map` case through `GpuCompositor`: `MapScene`, `compare_layout`, `chunks`, `chunk`, `MapRun`, `GpuResult`, `run`, `run_with`, `CHUNK` = 1024; `[[unit]]` fixture on the store (`case_frames_key`, `build_with`) | `render/map-preview.md`; `render-pipeline.md` §A9, §A10 |
| `crates/d2-client/src/audio/pool.rs` | `SoundPool` (load, peek, begin_frame, decodes, used, drain_events), `WavDecoder` hook, `SoundPoolError`, `sound_bytes`, `POOL_NAME` | `client/audio.md` §A1 r1, r3; `assets.md` §A5 |
| `crates/d2-client/src/assets/{prefetch,cache}.rs` | `PrefetchQueue<K>`; `Pool::offer`, `Offer { Taken, Resident, NoRoom }` | `assets.md` §A4 r3, §A5 |
| `crates/d2-client/src/ui/text.rs` | `layout_text`, `TextRules`, `NoTextRules`, `GlyphLookup`, `GlyphPlacement`, `GlyphDraw`, `TextOpts` (no fields), `TextError` | `ui.md` §A3 |
| `crates/d2-sim/tests/{game_world,game_drlg_tables,game_treasure,game_items}.rs`, `items_treasure_live/mod.rs` | 42 ignored game-file tests (§1 3q); shared loader `bin::load` → `fixup::apply` once per binary | `world/*`, `drlg/*`, `items/*` |
| `.github/workflows/ci.yml`, `tools/gate.sh`, `tools/hooks/selftest.sh` | five parallel CI jobs (§4); the local gate; pre-commit pattern selftest | |
| `tools/cloud-setup.sh` | cloud session setup (Linux libs, pinned Rust) | |
| `crates/d2-sim/src/items/inventory/{mod,tables,grid,belt,equip,checks}.rs` (+ `inventory/tests.rs`) | `Inventory` (item list, grids of `UnitId`, cursor, weapon GUID, update list; `link` / `unlink`, `take_updates`), `InvItem`, `Grid`, seam `InvWorld`; `InvTables::from_fixed`; `place_in_page` (§2.4 steps 1–9), `find_free_position`, `search`, `weight`; belt (`belt_type`, `free_belt_slot`, `auto_belt_gate`, `place_in_belt_slot`, `compact_belt`); `equip_check`, `requirements_met`, `equip_from_cursor` → `EquipOutcome`, `auto_equip_location`; `cursor_item_check` … `item_move_gate`; signatures in `impl-inventory.md` §3, seams in §4 | `items/inventory.md` §1–§5 |
| `crates/d2-sim/src/items/moves/{mod,seams,deferred,handlers,ground,layouts}.rs` (+ `moves/tests/`) | `handle` (size check, dispatch by id; `HANDLED` = 23 ids), one function per C→S id 0x16–0x29, 0x50, 0x61, 0x63; `ITEM_ACTIONS` + `dispatch` (`0x005973F0`), `player_update`, `ground_update`, `send_item_page`, `send_to_belt`, `mark`, `category`, `owner_refresh`; ground pickup / drop / expiry / cube spill / gold piles; byte builders 0x9C / 0x9D / 0x7D / 0x47 / 0x48 / 0x42 / 0x3F / 0x19 / 0x1D–0x1F; seams `InventoryOps`, `MoveUnits`, `MovePending`, `MoveWorld` (groups and defaults in `impl-moves.md` §4) | `items/inventory.md` §6–§11, `item-actions.tsv` |
| `crates/d2-sim/src/path/{search,place,warp,place_seams}.rs` (seventh fold: `path/mod.rs` declares the whole module, rows below) | `nearest_free_point`, `free_point`, `free_point_step`, `free_point_field`, `coarse_free_box`, `ExpField` (`from_bytes`, `walk_back`, `FIELD_DX` / `FIELD_DY`), `floor_drop`, `place_unit`, `level_spawn_point`, `game_entry`, `level_warp_place`, `warp_tile_preset`, `warp_player` → `WarpOutcome`; traits `CollisionView`, `PlaceHost`, `LevelView`, `WarpTileView`; `PlaceError` (`NoPath`, `NoAct`, `SpawnNotFree`, `NoLvlWarp`) | `sim/path-placement.md` §7–§12, `sim/path-tables.tsv` |
| `crates/d2-proto/src/s2c/{mod,field,messages,parse,audit,tests}.rs`; `schema.rs`, `tsv.rs`, `codegen.rs` (`bits:`) | `ServerMsg`, the 16 built types (§1 3t), `parse` / `Message` / `ParseError`, `AUDIT` / `audit` / `Status`; `FieldType::Packed`, `packed_get` / `packed_put`, `PACKED_MAX_WIDTH`; `generated.rs` regenerated (`data-tool gen-proto`) | `sim/intents-events.md` §3, `server-messages.tsv` |
| `crates/conformance/src/{raw,units,stats,packets,rooms}.rs`, `tests/{units,stats,packets,rooms}_replay.rs`, `tests/fixtures/` | replay harnesses (§1 3u): `RawRecording`, `replay_anim`, `replay_stats`, `PacketServer` + `replay_packets` + `DispatchServer`, `RoomModel` + `replay_rooms`; `d2-server` is a dependency (depcheck OK) | `sim/units.md` §4.2, `sim/stat-lists.md`, `sim/intents-events.md`, `drlg/rooms.md` §6 |
| `crates/d2-sim/src/skills/use_/bodies.rs` (+ `use_/tests/bodies.rs`), `missiles/tests_bodies.rs` | `START_BODIES` = [18], `DO_BODIES` = []; `bodies::start(index)` / `do_(index)` → `Option<result>` called by `start_core`, `do_core`, `active_state_event` before the `SkillFunctions` seam; the catalogue checks | `skills/use.md` §8, §5.3 step 6.6; `missiles/missiles.md` §R9.2 |
| `crates/d2-sim/tests/prop_{rng,timer,lists,units}.rs`, `crates/d2-sim/src/stats/prop_tests.rs` | state-machine and invariant properties with reference models written from the specs (§1 3w) | `sim/rng.md`, `tick.md`, `unit-order.md`, `units.md`, `stats.md`, `stat-lists.md` |
| `crates/d2-proto/tests/prop_messages.rs`, `crates/d2-server/tests/{prop_transport,prop_handle}.rs`, `crates/d2-client/tests/{prop_bridge,prop_worldsim}.rs` | robustness and determinism properties on the transport, dispatcher, handlers, bridge and the wired game over time (§1 3w); `prop_worldsim` reuses `e2e_support` | `sim/intents-events.md` §2–§3, `client/bridge.md` §4 |
| `crates/d2-sim/tests/{game_core,game_monsters,game_skills}.rs`, `tests/game_common/mod.rs`, `crates/d2-server/tests/game_world_data.rs`, `crates/d2-formats/tests/game_sweep.rs`, `crates/d2-client/tests/game_assets.rs` | the 67 `#[ignore]` game-file tests of the sixth fold (12 + 19 + 16 + 3 + 11 + 6); `game_common` loads `D2_GAME_DIR/extracted/patch_d2/data/global/excel/<name>.bin` | specs per test in §5 C37, C45, C46 |
| `crates/d2-client/src/scene/frame.rs`; `scene/{item,cpu}.rs`, `gpu_compositor/{pack,compositor.wgsl}` | `FrameCycle`, `FramePlan`, `UNCLEARED_ROWS`, `present_palette`, `PL2_PALETTE_BYTES`; `scene::compose_frame` / `compose_binned_frame`; `BlendOp::IndexTable` reads `table[dest][src]`; GPU params 48 bytes, binding 9 `base`, `Packed::with_frame` | `render/composition.md` §2–§6 |
| `crates/d2-client/src/verify/{capture,capture_case,capture_tests}.rs`, `capture-cases/*.toml` | `frames-raw-1` reader, PNG reader, `Image::check`, `StateKey`, `stability`, `repeated_ticks`; case kind `scene` (`SceneSource` / `SceneJob` / `SceneOutcome`, `SceneNotWired`, `resolve`, `run_capture`); `Status::SceneNotWired` (exit 2); deps `png 0.18`, `sha2`, `serde_json` | `render/capture.md` §3–§8, `traces/FORMAT.md` |
| `crates/d2-client/src/world_view/{mod,ui_bind,present}.rs` (+ `tests.rs`) | `ViewAssets { cofs, fonts, frames: FrameStore, maps, palette }`, `UnitPose`, `TileDraw`, `ViewRules` / `UiRules` / `TextHooks` (hooks, `Unspecified`), `WorldFrame`, `build`, `compose_cpu`, `GpuAtlas` (`ensure(&FrameStore)`, `pack`, `compose`, `frames`, `slots`), `ViewError::{Frame, AtlasAhead, FontMissing, Text, Unresolved}`; `UiQueue`, `UiFrame`, `run_ui`, `text_sprites`; `WorldViewPlugin`, `WorldViewState`, `WorldViewUi` (non-send), `present_scale` | `client/render-pipeline.md` §A1, §A6–§A9, `ui.md` §A2–§A4 |
| `crates/d2-client/src/app/sound.rs`; `app/single_player.rs` (`LiveData`, `GameData::select`, `LevelSource`) | `GameAudio`, `PoolBank`, `AudioParts`, `AudioStats`, `add_audio`, `add_output`, placeholders `SoundTable` / `NoSoundTable`, `NoWavDecoder`, `NoCues`; `GameData::{Synthetic, Live(Arc<LiveData>)}` | `client/audio.md` §A1, §A3; `drlg/levels.md` §2–§3 |
| `crates/d2-sim/src/wiring/action/{units,reaction,pending,mod}.rs`, `wiring/action/tests/death.rs`, `wiring/economy/death.rs` | `UnitHooks::{anim_record, anim_rate, frame_bonus, player_action_frame, monster_mode_function}` on `ActionHooks`, `anim_record()`; `reaction` (state-54 rule, monster will-die → `kill`), `VitalsUnits` on `View`; `Pending::{anim_name, anim_rate, frame_bonus, kill_step, superunique, minion_owner, party_size, quest_tc_open, stats_refresh, level_up_notify, level_up_event, monster_death_start, action_frame}`; `ActionHooks::{anim_data, vitals, mode_target}`, `WiringError::AnimData`; `DropTables`, `DeathDrops`, `FreeSpot`, `monster_death_drop` | `sim/units.md` §4, `combat/damage.md` §7, `combat/vitals.md` §4, `items/treasure.md` §3, §7 |
| `crates/d2-sim/src/wiring/interaction/{skill_events,quest_npc}.rs`, `wiring/worldgen/events.rs`, `wiring/economy/quest_tick.rs` (+ their `tests/`) | `skill_events::route` (events 5 / 8 / 9 on `UseView`), `Pending::skill_event` / `SkillEvent`; `Desk::quest_message` (0x31 + deferred mercenary reward; `EconomyQuests::mercenaries`); `WorldState::forget`, `WorldSim::remove_unit`; `QuestTick`, `UnitSide` (tick step 8) | `skills/use.md` §7, `sim/stat-lists.md` §10.2–§10.3, `world/quests.md` §5, §7.3, `world/npc.md` §7.5 |
| `crates/d2-server/src/adapters/{sim,handlers/world,handlers/skills/wired}.rs`; `world/trade.rs` (seventh fold: now `world/wired.rs`, `WiredWorld`; see the host rows below); `crates/d2-client/tests/e2e_vendor.rs` | `SimGame::tick` passes the dispatch as `TickHooks`; `SimGame::with_world`; `handlers::world::ActionEvents` (`ActionSim`, `WorldSim`), `TradeWorld`, `TradeRest`, `Parts` | `sim/tick.md` §3, `world/npc.md`, `world/vendors.md` |
| `crates/d2-server/src/adapters/sim.rs` | `SimGame<D = Unspecified, W = NoWorld>`: one host `world: W`; `tick_faults: Vec<(ClientId, WorldError)>`; `SimParts` / `parts()`, `player_of`, `client_of`; `handle` tries items (cube), the moves (`WorldHost::moves`), world, skills, then the stub; `Tick` needs `D: EventDispatch + TickHooks, W: WorldHost<D>`, queues `take_sent` after the tick, then runs `items::moves::update_pass` | `intents-events.md` §2–§4, §3.2; `tick.md` §3; `inventory.md` §6 |
| `crates/d2-server/src/adapters/handlers/world.rs`, `world/{action,wired}.rs` | `WorldHost<D>` (`npc`, `vendors`, `waypoints`, `quests`, `cube`, `skill`, `moves`, `take_sent`, `fault`; defaults `None` / stub), `NoWorld`, `WorldError::Move`; `ActionEvents` (+ `create_game(&GameFields)`), `ActionWorld<S = NoSkills>`, `Outbox`; `WiredWorld<R, S = NoSkills>` (`with_economy`, `Parts` + `cube`, `inventory: Option<InvParts>`, `TradeRest`, `HostWaypoints`, `uniques`) | `world/*.md`, `npc.md`, `vendors.md`, `quests.md`, `cube.md` §1–§2, `waypoints.md` §6, `inventory.md` §6 |
| `crates/d2-server/src/adapters/handlers/items.rs`, `items/cube_world.rs`, `items/moves.rs` (+ `moves/tests.rs`) | `ITEM_IDS`, `handle` (via `WorldHost::cube`), `CubeParts`, `CubeCall`, `CubeHooks`, `Interact`, `Staged`, `Inventory`, `ItemPending`, `ItemError`, `ServerCube<H>`; `MOVE_IDS`, `is_move_id`, `MoveRest` (`InvRest` + `take_sent`), `InvParts`, `MoveCall`, `handle`, `update_pass`; 22 host-frame tests, id-table check + M08 | `cube.md` §1, §2, §8; `inventory.md` §6–§11 |
| `crates/d2-server/src/adapters/handlers/skills/{mod,wired,world}.rs` | `SkillHost<D>` (`handle(Call) -> Option<Handled>`), `NoSkills`, `LearnRest`, `SkillRest`; `WiredSkills` (only `unsent`; `Default`); `World<'v, 'a, X>` over `UseView` (`SkillSeams` and `seams.rs` are gone) | `use.md`, `levels.md` §6.4, `vitals.md` §2 |
| `crates/d2-sim/src/wiring/economy/{game_fields,death}.rs` | `GameFields::from_action(seed, &ai::GameInfo, expansion, uniques)`, `GameFields::ai_info`; the death drop's creation fields come from the action wiring (`ActionHooks::ai_info`, `UnitData::expansion`), `uniques` from `DeathDrops::fields` | `generation.md` Inputs; `treasure.md` §3 |
| `crates/d2-sim/src/wiring/inventory/{mod,inv_world,ops,units,pending}.rs` (+ `tests/{mod,ground,buffer,equip,belt,stack,gold}.rs`) | `InvState`, `InvRest`, `InvDesk` (`new`, `with_inv`, `update_done`), `InvError`; `InvWorld`, `InventoryOps`, `MoveUnits`, `MovePending` for `InvDesk` (`R: InvRest + ?Sized`); 26 tests | `inventory.md` §1–§11 |
| `crates/d2-sim/src/path/{mod,coords,record,collision,footprint,tables}.rs` (+ `tests.rs`) | `PathError`, sub-tile / 16.16 / client coordinates, `PathKind`, `StaticPath`, `DynamicPath` (+ `set_path_type`, `put_target`, `final_target`), `UnitShape`, `alloc_dynamic_path`, `CollisionRooms`, `find_room`, `point_value` / `plus_value` / `box_value` / `size_value` / `pattern_value` / `pattern_collides` / `box_apply`, `stamp_*` / `clear_*`, `Footprint`, `add_footprint`, `remove_footprint`, `set_foot_mask`, `set_pattern`, `try_move`, `forced_move`, `missile_move`, `teleport(_and_clear)` (`C: CollisionRooms + PathMotion`), `PathTables` (18 tables, `spec()` / `from_tsv`), `path::Point` | `sim/path-placement.md` §1–§6; `path-tables.tsv` |
| `crates/d2-sim/src/path/walk/{request,find,velocity,step,geom,messages,seams}.rs` (+ `tests/{mod,messages,gaps,fake}.rs`) | `Walk<'a, C> { t, c }`, `request::handle_message` / `request` / `Outcome` / `mode_check` / `interrupt_check` / `start_movement` / `neutral_start`, `find::{compute, toward, straight, astar}`, `velocity::{mode_velocity, set_velocity, run_velocity_bonus, aim}`, `Walk::{player_event0, step, movement, repath, set_position, reset, room_change_messages, run_drain, target_check}`, `messages::{walk_verify 0x0D/0x0F/0x10/0x15/0x96 builders, mode_update, reassign_flag}`, seams `PathWorld: CollisionRooms`, `WalkUnits` (`target_lead`, `other_path_function`, `repath_budget`, …), `PathInfo`, `WalkError`; 34 + 8 tests | `sim/pathing.md` §1–§10 |
| `crates/d2-sim/src/wiring/path/{mod,rooms,units,walk,place}.rs` (+ `tests.rs`) | `PathState` (`records`, `field`), `PathCtx` (one context = game + `View`: `PathWorld`, `WalkUnits`, `PathMotion`), `DrlgWorld` (`CollisionRooms`), `walk::{player_step, build_path, unit_step, walk_message}`, `place::{Shared, Rooms, level_warp, coarse_free_box, floor_drop}`; `ActionHooks::{paths, enable_paths}`, `WiringError::{Path, Walk, Place}`; `Drlg::active_grid_mut`; 7 tests on the action fixture | `path-placement.md` §2.5, §5, §8, §10–§11; `pathing.md`; `waypoints.md` §7 |
| `crates/d2-sim/tests/{prop_path_core,prop_walk,prop_path_place,prop_inventory}.rs`, `crates/d2-sim/src/items/moves/prop_tests.rs` | property tests with reference models written from the specs (§1 3aa, 3ad) | `path-placement.md`, `pathing.md`, `inventory.md` |
| `crates/d2-sim/benches/sim.rs`, `crates/d2-formats/benches/formats.rs`, `crates/d2-proto/benches/proto.rs`, `crates/d2-client/benches/compose.rs`; `d2_sim::bench_fixtures` (feature `bench-fixtures`) | criterion baselines (§1 3ad); not in CI | |
| `crates/d2-server/tests/{mutants_core,mutants_adapters,prop_handle}.rs`, `crates/d2-sim/src/{missiles,monsters}/**/mutant_tests*.rs` | mutation kills (25 in the server, 121 in monsters / missiles) and the host's property test (§1 3y, 3ad) | `intents-events.md`, `ai.md`, `missiles.md`, `population.md`, `init.md` |
| `crates/d2-client/src/rules/{mod,camera,placement,view,tests}.rs`; `frames/mod.rs` (`FrameAnchor`) | `FrameSize`, `OpenMode`, `ViewRect`, `Camera::new`, `unit_draw`, `Shake`, `shake_offsets`; `Cel`, `RowPlan`, `place`, `block_pixel`; `OriginalView<R, S>`, `MapTile`, `BlockRect`, `ViewSource`; 16 tests | `render/camera.md` §1–§10; `sprite-placement.md` §1–§8 |
| `crates/d2-client/src/world_view/{feed,present}.rs` (+ `feed/tests.rs`); `app/play.rs` | `ViewFeed`, `NoFeed`, `RunningShake`, `frame_shake`, `frame_camera`, `build_frame`, `NoCamera`; `WorldViewState::new(assets, rules, feed)`, `FrameStats.server_tick`, one drawn frame per tick | `camera.md` §3, §8–§10; `render-pipeline.md` §A1 |
| `crates/d2-client/src/verify/capture_case/{scene_source,scene_tests}.rs`; `capture_case.rs`; `main.rs` | `WorldScene<W: CaptureWorld>`, `RecordedScene`, `WorldAnswer`, `NotRecorded`, `RECORDER_GAP`, `recorded_camera`, `camera_differences`, `SceneOutcome::{Seam, Differs}`, `SceneSource::take_notes` | `capture.md` §3, §4, §6 |
| `crates/d2-client/tests/{prop_support/mod,prop_client_bridge,prop_world_view,prop_controls,prop_ui,prop_atlas,prop_scene,prop_cache}.rs` | `config` (`PROPTEST_CASES`), `bounded` (20 s deadline); the seven property files of `prop-client` | `client/*.md` |
| `crates/conformance/src/{movement,placement,needs}.rs`, `src/bin/recordings-needed.rs`, `tests/{movement_replay,placement_replay,recordings_needed}.rs`, `tests/fixtures/{movement-walk,placement-players}.jsonl` | `MovementRecording`, `Mover`, `replay_movement`; `Spawn`, `PlacementModel`, `replay_placement`; `NEEDED` (7), `report`; `d2-proto` dependency (no Bevy) | `pathing.md` §10; `path-placement.md` §10–§11 |
| `crates/d2-sim/src/path/gap_tests.rs`, `items/inventory/gap_tests.rs`, `items/moves/tests/gaps.rs`, `crates/d2-server/src/adapters/handlers/world/tests/gaps.rs` | the gap tests of `gaps-new-specs` (§1 3ae); the inventory fake gains `no_free_page0` | `path-placement.md` §7–§12, `inventory.md`, `quests.md` §6.2 |
| `crates/d2-sim/tests/game_inventory_path.rs` | 6 `#[ignore]` game-file tests + 2 CI tests (§5 C51) | `inventory.md` §1–§3; `path-tables.tsv` |
| `tools/ghidra/` | Ghidra scripts (label import, export); `disasm.py`: disassembly, xrefs, whole-binary dump (the decompile drops register arguments) | |

**Seams** (the traits above; the notes list every method and its expected
provider). Overlap the provider sessions must reconcile: unit getters
(stat reads, states, flags, hostility, owner, position) are asked for by
`StatHost`, `UnitHooks`, `SkillUnits`, `CombatWorld`, `MissileUnits`,
`AiUnits`, `QuestWorld` and `Dropper` alike; item queries by `SkillUnits`,
`CubeWorld`, `QuestWorld`, `DropSink`; room / collision queries by
`MissileRooms`, `AiWorld`, `DropSink::place`, `WaypointWorld`. Seams are
traits with fakes in tests, so a provider may implement several.
Seams of the second set of modules (providers in the notes): `InitHost`
(monster init), `PopWorld` / `MonsterInit` (population), `Ds1Source`
(preset; DS1 as plain `Ds1Input`), `MazePresets`, `OutdoorPresets` /
`SubFiles`, `NpcLink` / `VendorWorld` (vendors), `NpcWorld` / `NpcVendors`
(NPC), `UseWorld` / `UseMissiles` / `SkillFunctions`, `VitalsUnits`, and
the wiring's own open ends (`wiring::action::Pending`, `CubeRest`,
`QuestRest`, `DropPlacer`; reasons in `wire-action.md` §4,
`wire-economy.md` §5). **Public signatures changed outside `wiring/`**
(METHODS M21): `tick::TickHooks::room_deactivated(game, act, room)` (new,
default nothing, step 9 after `lists.deactivate_room`; the trace replay is
unchanged); `monsters::ai::AiUnits::level_id(&self, game, unit)` (was
`(unit)`; two call sites and the test fake updated);
`missiles::MissileStore::remove(m)` (new, public); `missiles` re-exports
`MissileParams` and `result_flag`. New dependency: `wgpu = "29.0.3"` in
`d2-client` (§7 Phase 6). Third set (METHODS M21; each is in its note):
`drlg::DrlgError::LevelType(u32)` (new); `drlg::LevelTypes::door_unit(…,
orientation)` (new argument; `tiles.rs` call sites, `OutdoorTypes` and the
DRLG test fake updated); `drlg::Drlg::active_room_seed_mut` (new);
`monsters::population::MonsterInit::{allocate_monster, add_modifier}` take
`state: &mut PopState` (three call sites pass `cx.state`);
`monsters::init::superunique_init` split into `superunique_mods` (§20 steps
1–2) and `superunique_finish` (steps 4–5), `superunique_init` composes them;
`world::vendors::NpcLink::make_hire_list(class, seed: &mut Seed)` (was
`(class)`; `store::open` passes `c.seed`); `rng::Seed` `#[repr(C)]`;
`d2_data::bin::check_server_files(set, table)` split out of `load()`;
`SimGame` gained the type parameter `W` and the fields of §3 above;
`d2-client` now depends on `d2-server`, with dev-dependencies `d2-sim`,
`d2-data`; `d2-server` has dev-dependency `d2-data`. The `d2-server`
handler sessions changed no `d2-sim` code (no accessor was needed).
`d2-sim` has no `d2-formats` dependency: tables arrive as `d2_data`
typed records, DT1 tiles as `TileInfo`, DS1 grids as `RoomGrids`.
**Fifth fold (METHODS M21; each in its note):** `wiring::action::ActionHooks`
gained the public field `monster_world` (and a private one; a struct literal
in another branch would break, all use `ActionHooks::new`);
`wiring::worldgen::events::WorldHooks` removed; new `MonsterWorld`,
`umod_mode`, `ActionHooks::{with_monster_world, monster_data, run_umods,
monster_flag}`, `WorldSim::{lend, with}`; `WorldSim::world` is an empty
placeholder while a hook runs (a host reading it from inside a hook sees an
empty state). `gpu_compositor::device::Gpu` gained `atlas_texture` and
`encode_rgba`; `world_view::present::WorldViewGpu` lost its `gpu` field
(the compositor lives in the render world); `GpuAtlas::sets()` new;
`ComponentResolver::frame_id` has a default body; new `composite::{FrameIds,
build_with}`, `frames::{FrameStore, StoreError}`, `verify::case_frames_key`,
`verify::map::run_with`; `cargo run -p d2-client` without a subcommand runs
`play`, not `view`. Dependencies: `d2-client` moved `d2-sim` and `d2-data`
from dev-dependencies to dependencies; `d2-server` moved `d2-data` to a
normal dependency and added `d2-formats` (depcheck OK); `d2-formats` gained
feature `test-support`; `d2-sim` gained dev-dependency `proptest`; the
workspace gained member `crates/test-fixtures`. Behavior: `d2_data::fixup::
maps::equiv_matrix` (so `fixup::apply`) can now return `Err` for an
`equiv1` link cycle that never reaches the asked column (before, it did not
return). New public items only in `d2_client::{audio::pool, assets::prefetch,
ui::text}` and `assets::cache::{Pool::offer, Offer}`.

**Sixth fold (METHODS M21; each in its note):** `UnitLists::hash_bucket(_, b ≥ 128)`
returns empty (was a panic); `UnitLists::free_room` unlinks the units still in
the room first; `units::anim::AnimError::Endless { speed, frame_count }` (new
variant; `schedule` uses 32-bit wrapping for the start index);
`lifecycle::allocate` refuses an unknown room before any draw and restores
the game seed and GUID counter when `SUNIT_Add` refuses; `StatLists::detach`
and `attach` use `try_ext_mut` / `try_l` so a parked child of a freed list is
detachable; `stats::fraction_changed` wraps. `d2_client::bridge::intent::
IntentError::TooLarge(usize)` (new variant; no exhaustive match exists).
`scene::BlendOp::IndexTable` reads `table[dest][src]` (**behavior change**;
`MapTable::push_table(table[dest][src])`; GPU params 32 → 48 bytes, binding 9
`base`; `verify` `[[table]]` rows are filled to keep their meaning);
`world_view::ViewAssets::sets` → `frames`, `GpuAtlas::ensure(&FrameStore)`,
`GpuAtlas::sets()` → `frames()`, `ViewError::{SetMissing, FrameIndex}` →
`Frame(StoreError)`, `WorldFrame::frames` / `FrameTable` / `BoundFrames` /
`FrameSlots` removed, `GameData::Live(WaypointTables)` →
`GameData::Live(Arc<LiveData>)`. `Tick` (server) needs `D: TickHooks`;
`ActionWorld: WorldHost<D>` and `WiredSkills<S>: SkillHost<D>` for `D:
ActionEvents`; `ActionHooks` gained `anim_data`, `vitals`, `mode_target` and
`Pending` the methods of the combat-path row (all defaults = the previous
behavior); `d2-sim` → `d2-formats` dependency; `Pending::skill_event` (new,
default nothing; a seam value that implements `UseRest` must override it);
`EconomyQuests` has the public field `mercenaries` (use `EconomyQuests::new`);
`WorldSim::run_event` runs the unit dispatch itself. New dependencies: `png
0.18`, `sha2`, `serde_json` in `d2-client`, `proptest` dev-dependencies in
`d2-sim`, `d2-proto`, `d2-server`, `d2-client`. `d2-proto` `generated.rs`
regenerated twice (`bits:` layouts; the inventory `layout` cells). Known
mismatch: the `Pl2::alpha_blend` doc in `d2-formats` says `[level][source]`
(RN3).

**Seventh fold (METHODS M21; each in its note):** `SimGame`: fields `items` and
`skills` removed, `tick_faults`, `parts`, `SimParts`, `player_of`, `client_of`
added; `impl Tick for SimGame<D, W>` needs `D: EventDispatch + TickHooks, W:
WorldHost<D>` (was `D: TickHooks`, any `W`). `handlers::world::TradeWorld<R>`
→ `WiredWorld<R, S = NoSkills>` (module `world::trade` → `world::wired`; field
`fields: GameFields` removed, `uniques` and `cube: Option<CubeParts>` added,
`new(action, tables, quests, npc, vendor_tables, rest, now)`);
`ActionWorld<S = NoSkills>`; `WorldHost::{cube, skill, moves}` (defaults),
`WorldError::Move(MoveFatal)`; `handlers::items`: `ItemWorld`, `ItemView`,
`ItemHooks`, `Interaction`, `Staged::interactions` removed, `handle<D, W:
WorldHost<D>>`, `CubeParts`, `CubeCall`, `CubeHooks`, `Interact` added;
`handlers::skills`: module `seams` removed, `SkillHost::handle(Call) ->
Option<Handled>`, `NoSkills`, `LearnRest`, `SkillRest`, `WiredSkills` not
generic (`Default`), `wired::run` / `add_skill_point` take `World<'_, '_, X:
SkillRest>`. `d2-sim` public additions: `GameFields::from_action` /
`ai_info`, `economy::death::monster_death_drop` reads its fields from the
action wiring, `Drlg::active_grid_mut`, `ActionHooks::{paths, enable_paths}`,
`WiringError::{Path, Walk, Place}`, `wiring::action::tests` is `pub(crate)`,
`InvDesk` and its six impls take `R: InvRest + ?Sized`. `path::walk` was
reshaped (`Walk<'a, C> { t, c }`, `handle_message(t, c, player, id, a, b)`,
`request(t, c, …)`, `compute(t, c, path, unit, town)`, `WalkUnits` methods
lose every `Game` argument and gain `frame()`, `PathWorld: CollisionRooms`
with nine methods, `PathInfo.{path_type, pattern}` u32, `WalkError` not
`Copy`); removed duplicates `walk::seams::{Point, WalkPath}`,
`place_seams::{SubPoint, RoomRect, TileRect}`, `walk/tables.rs`,
`walk::geom::centre`, the walk's `flag`, `path_type` modules; `path::footprint::
{teleport, teleport_and_clear}(c, path, is_missile, room, x, y)` with `C:
CollisionRooms + PathMotion`; `CollisionView::room_rect` /
`WarpTileView::tile_rect` return `drlg::TileRect`. `d2-client`: `IndexFrame`
has `anchor: FrameAnchor` (a struct literal elsewhere needs `anchor:
FrameAnchor::Top`), `FrameError::{Dc6Flip, DccVariable0}`,
`WorldViewState::new(assets, rules, feed)`, `FrameStats.server_tick`,
`SceneOutcome::{Seam, Differs}`, `SceneError::View` and `Rect::check_view`,
`ui::geom::Rect::{right, bottom}` return i64, `CellGrid::new` refuses a grid
past i32. `grid::in_bounds` sums in i64; `path/warp.rs` uses `wrapping_add` /
`wrapping_mul`. New feature `bench-fixtures` in `d2-sim` (self
dev-dependency), criterion as a workspace dev-dependency
(`default-features = false`); `conformance` depends on `d2-proto`.

**Eighth fold (METHODS M21; each in its note):** `d2-sim`: `ActionHooks::items:
wiring::economy::ItemStore` added; `DeathDrops::items` **removed**;
`wiring::economy::DropPlacer` → `DropPlacer<H>` (`place(&mut self, econ: &mut
Economy<'_, H>, x, y)`, new provided `placed(econ, item, spot)`);
`wiring::inventory::host` (new module: `InvState::{of, items_of, cursor_of,
holds, body_items, fillers}`, `InvDesk::{place, remove, free, reset_targeting,
check_stored, check_ground_or_owned, send_item_page}`);
`wiring::path::walk::update_messages` and `ActionSim`'s `TickHooks::
send_unit_update` body; `world::cube::trade_action`; `StatLists::expire_lists`
returns `Result<(), StatListError>` (`EndlessExpiry(list)`), `TimerId {
slot, generation }` with `TimerId::slot()`. `d2-server`: `handlers::items`:
`Inventory` **removed**, `Staged` loses `inventories` and `targeting_resets`,
`ItemPending` loses `place`, `remove_cube_item`, `socketed`, `ItemError::Move(
MoveFatal)`, `CubeCall::call` takes `inv: Option<&mut InvParts>` (fourth
argument, before `interact`), re-exports `InvParts` and `InvVendors`;
`InvParts::desk(&mut Economy)`; `WiredWorld`: field `items` **removed**, private
`inv_sent` added, `Parts` gains `inventory: Option<&'p mut InvParts>`;
`handlers::walk` (new: `WALK_IDS`, `Form`, `form`, `WalkCall`, `WalkResult`,
`enable_paths`, `run`, `handle`), `WorldHost::walk` (provided, default `None`),
`dispatch::in_range` via `abs_diff`. `d2-formats`: `huffman::compress` under
`cfg(any(test, feature = "test-support"))`, `writer::Method::Huffman { table,
pkware }`, `writer::huffman`, `WriteError::BadHuffmanTable`. `test-fixtures`
depends on `d2-sim` and `d2-server` (normal dependencies); new modules `ds1`,
`dt1`, `drlg`, `game` (`GameData`, `ActCreation`, `Seams`, `Sim`);
`content.rs` lost the `Tiles\` prefix of its table strings and gained a lvlprest
Def 3. `tools/coverage.py`: table-row units under `<!-- rows -->`.

New code map rows: `crates/d2-sim/src/wiring/inventory/host.rs`,
`wiring/inventory/tests/host.rs` (5 tests), `crates/d2-server/src/adapters/
handlers/items/vendor_inv.rs` (`InvVendors`), `handlers/walk.rs` +
`walk/tests.rs`, `crates/d2-client/tests/e2e_walk.rs`,
`crates/d2-sim/tests/{s2c_bytes,walk_rooms_fake/mod,prop_walk_rooms,
prop_walk_motion,prop_path_footprint_ops}.rs`, `crates/conformance/tests/
s2c_builders.rs`, `crates/d2-formats/src/mpq/huffman_tests.rs`,
`crates/test-fixtures/{src/{ds1,dt1,drlg,game}.rs,tests/synthetic_game.rs}`,
`crates/d2-server/tests/game_wired_host.rs` (7 ignored), `.github/workflows/
nightly-props.yml`, `tools/props-deep.sh`, and the `mutant_tests.rs` files
(`stats/`, `units/`, `d2-proto` ×3, `items/inventory/`, `items/moves/`,
`items/tests/mutants.rs`, `treasure/`, `world/` ×8, `combat/`, `skills/`).

## 4. Command map (what proves what)

| Command | Proves | Needs |
|---|---|---|
| `sh tools/gate.sh` (`--no-client` skips the Bevy crate) | **the one gate command**: the CI steps in CI order with a PASS / FAIL + seconds summary (every step runs, exit 1 if any fails): spec_index, methods, coverage `--check` / `--selftest`, trace checkers, pre-commit hook selftest, fmt, depcheck (+ determinism), clippy, tests (d2-sim + conformance, the rest, d2-client), doc-tests; uses nextest when installed, else `cargo test`; run `sh tools/cloud-setup.sh` first in a cloud session | repo |
| `cargo test -p <crate>` | unit tests from spec vectors | repo |
| `cargo test -p conformance --test tick_replay` | `d2-sim::tick` reproduces every timer run and list snapshot of `traces/sim/tick/*.json`; perturbation tests (`a_*_is_reported_exactly`) report exactly the changed record (M08) | repo |
| `cargo clippy -p <crate> --all-targets -- -D warnings`, `cargo fmt --all` | lint/format gate | repo |
| `cargo run -p depcheck` | crate dependency rules **and the determinism lint** (CLAUDE.md hard rule 6): every non-test `.rs` under `crates/d2-sim/src` is scanned and `file:line: determinism rule `<rule>`` is reported for floats, `HashMap` / `HashSet`, `std::time` / `Instant`, `rand` / `thread_rng`, `std::fs` / `std::io`, `std::env`, `thread_local!` / `static mut`, `unsafe`; exit non-zero on any hit (already a CI step). Lexical, so it does not see aliases in other crates or dependency floats | repo |
| `cargo test -p depcheck` | 5 selftests (M08): each of 22 banned constructs caught with its line; comments, strings, raw / byte strings, chars, lifetimes not flagged; `#[cfg(test)]` items and external test-module files skipped; allowlist reason mandatory, stale entries reported | repo |
| `cargo run -p data-tool -- gen-proto` then `cargo test -p d2-proto` | `d2-proto` tables regenerated from `specs/sim/*-messages.tsv`; tests `generated_file_is_current`, `tables_match_tsv` (perturbation: `check_reports_exactly_a_changed_row`) | repo |
| `py tools/spec_index.py --check` | spec indexes current | repo |
| `py tools/coverage.py --check` / `--selftest` | every `Covers:` claim names an existing spec rule (perturbation: a renamed claim is reported at its file and line) | repo |
| `py tools/coverage.py [--summary]` | per-spec coverage of spec rules by unit, game-file and trace checks; verified = game-file or trace (`docs/COVERAGE.md` §3) | repo |
| `PROPTEST_CASES=20000 cargo test -p d2-formats -p d2-data robust` | parsers return Ok/Err on malformed input: no panic, hang or huge allocation (default case counts run in `cargo test`, <1 s) | repo |
| `cargo test -p d2-data -p d2-formats -- --ignored` | game-file tests | `game/` |
| `cargo run --release -p data-tool -- tables` | every live table and code buffer reproduced from `.txt` | `game/` |
| `cargo run --release -p data-tool -- links` | no broken link in the live `.bin` set | `game/` |
| `cargo test -p d2-data --test patch_game -- --ignored` | patch layers on the live tables (G1–G8) | `game/` |
| `cargo run --release -p mpq-tool -- check` / `formats` | every archive block / every format file decodes | `game/` |
| `cargo run --release -p d2-client -- verify [--case NAME]… [--cases DIR] [--perturb N]` | every case of `crates/d2-client/render-cases/`: the `map` case GPU render byte-identical to the CPU reference (app path), each synthetic case CPU binned vs CPU reference and the compute compositor's index framebuffer (bytes) and RGBA (pixels) vs the reference; exit 0 all PASS, 1 any FAIL / ERROR, 2 incomplete (`GPU NOT WIRED` / `NO ADAPTER`, never a pass); any map flag (`--ds1`, `--wall-base`, `--view`, `--out`) runs the single-map verify as before; `--perturb N` above the pixel count is an error | `game/` (`map`), GPU |
| `cargo run -p d2-client --example gpu_compare [-- --case NAME] [-- --perturb N]` | 18 synthetic cases (12 + `frame-blank-screen`, `frame-no-clear`, `frame-post-clear`, `pixel-write`, `blend-ops`, `frame-stress` of `render-composition`): compute compositor equals `scene::compose` / `to_rgba`; `--perturb N` reports exactly N (M08) | GPU (no game files) |
| `cargo test -p d2-client --lib gpu_compositor::tests::gpu -- --ignored --nocapture --test-threads 1` | `gpu_matches_cpu`, `gpu_perturb_reports_exactly_n` | GPU |
| `cargo test -p d2-client --lib verify::tests::gpu_half -- --ignored --nocapture` | `gpu_half_matches_cpu_on_every_synthetic_case` (a software adapter such as lavapipe will do; no adapter fails it) | GPU |
| `D2_GAME_DIR=<game> cargo test -p d2-client --lib composite::tests::all_live_cofs_give_slot_orders -- --ignored --nocapture` | every live COF gives a slot order for every direction and frame | `game/` |
| `py tools/trace-recorder/record_rng.py --seconds N` then `check_rng.py` | RNG spec matches the real game | `game/`, Windows |
| `py tools/trace-recorder/record_tick.py --seconds 200` (play by hand) then `check_tick.py <file>` | tick steps, timer queue and unit lists match `sim/tick.md`, `sim/unit-order.md` | `game/`, Windows, a player |
| `py tools/trace-recorder/record_packets.py --seconds 180` (play by hand) then `check_packets.py <file>` | message transport rules R1–R7 of `sim/intents-events.md` | `game/`, Windows, a player |
| `convert_tick.py --check traces/sim/tick/*.json` | committed tick traces replay through the spec model, round trip exact | repo |
| `check_units.py <tick file>`, `check_stats.py <stats file>` | unit event rules / stat-list rules on a recording | a recording |
| `check_tick.py --selftest`, `check_packets.py --selftest`, `convert_tick.py --selftest`, `check_units.py --selftest`, `check_stats.py --selftest`, `tools/ghidra/disasm.py selftest` | the checkers catch perturbations (M08) | repo (disasm: `game/`) |
| `py tools/trace-recorder/dump_tables.py` (~7 s) then `cargo run --release -p data-tool -- dump-compare traces/raw/<time>-tables` | d2rs fix-ups and maps equal 1.14d memory after the load | `game/`, Windows |
| `cargo test -p d2-sim` (per module: `items`, `treasure`, `drlg`, `world`, `missiles`, `monsters`, `stats`, `units`, `skills`, `combat`) | every synthetic vector of the system specs; each machine-table check (`stat-ops.tsv`, `unit-handlers.tsv`, `property-functions.tsv`, `treasure-*.tsv`, `skillcalc.tsv`, `misscalc.tsv`, `srvdo.tsv`, `srvhit.tsv`, `ai-functions.tsv`, `cube-ops.tsv`, `waypoints.tsv` expectations, `quests.tsv`) has a perturbation test (M05, M08). Proves the implementation matches the draft spec, not 1.14d | repo |
| `cargo test -p d2-sim -- --ignored` | 5 game-file tests (`.bin` tables read through `skills::tests_game`): combat `codes_match_game_tables`, `real_skill_vectors`, `real_table_constants`; monster init `real_level_stats`; NPC `live_monstats_records` (§5 group C) | `game/extracted/patch_d2/data/global/excel/` |
| `cargo test -p d2-sim wiring` | the 17 economy, 19 action, 12 worldgen and 11 interaction integration tests (real modules together, synthetic tables / DRLG level, fixed seeds; per-branch counts, no merged run recorded); re-run `cargo test -p conformance --test tick_replay` after any change to `ActionSim`'s tick hooks (only steps 9 / 10 and the client room change are routed; the replay's own hooks do not use them) | repo |
| `cargo test -p d2-server` (modules `adapters::handlers::{items, skills, world}`, `tests::gaps`) | handler vectors through real host frames (send → drain → tick → flush → receive) with `ProtoSizes`: 0x2A / 0x4F bytes (0x77), skill / stat-point / select-skill results, world ids (0x49 wired, NPC / vendor / quest on a seam fake); `ids_match_client_tsv`, `world_ids_are_sim_handlers`, `ITEM_IDS` with perturbation; the gate of the three handler notes also runs `cargo test -p d2-sim` and `cargo clippy -p d2-server -p d2-sim --all-targets -- -D warnings` | repo |
| `cargo test -p d2-server --lib trade_quests` | 7 quest-host tests through the host frame on `TradeWorld` (0x31 / 0x40 / 0x58, Kashya reward, one-flag-bit and routing perturbations, same seed twice) | repo |
| `cargo test -p d2-client --test e2e_single_player` (also `--test e2e_vendor`) | the single-player e2e on `TradeWorld` + `ItemWorld`: 26 frames, `same_seed_same_run`, `other_seed_other_run` (wire identical up to the trade, result codes equal); stops at the seams of §2 step 7c | repo |
| `cargo test -p d2-client bridge` | also `bridge/local_tests.rs`: `LocalLink` end to end on the real host (C→S 0x49 → exact S→C `0D …` → `ClientWorld`) | repo |
| `D2_GAME_DIR=<install> cargo test -p d2-data --test gaps_fields_game -- --ignored`, `… calc::tests::game -- --ignored`, `… --test game_data -- --ignored <names>`, `D2_GAME_DIR=<install> cargo test -p d2-formats --test formats_game -- --ignored` | the gap-test game-file tests (28 new game-tier claims, never run; names and expectations in §5 C17) | `game/` |
| `py tools/trace-recorder/check_stats.py --files <GAME_DIR>` | the specs' stat facts on the 1.14d tables (`stats.md` §2 r2, §5 C2) | `game/` |
| `cargo test -p d2-client` (modules `assets`, `frames`, `scene`, `ui`, `controls`, `audio`, `bridge`, `gpu_compositor`, `composite`, `verify`) | Phase 6 infrastructure vectors (design specs, CI) | repo |
| `D2_GAME_DIR=<game> cargo test -p d2-client --lib frames::tests::all_live_frame_sets_build_and_pack -- --ignored --nocapture` | every live DCC / DC6 / DT1 builds frame sets that pack and check clean in a 64-page atlas; prints largest frame (≤ 2046) | `game/` |
| `cargo test -p d2-sim --test prop_tables`, `cargo test -p d2-data --test prop_data`, `cargo test -p d2-formats --test prop_more_formats` (`PROPTEST_CASES=N` hunts harder; fuzz-data used 3,000 / 4,000 / 20,000) | table readers, compile / bin / fix-up / patch-layer paths and the extra formats return Ok / Err on random input (no panic, hang or huge allocation); the txt → bin → typed round trip; `regress_equiv_walk_cycle` | repo |
| `cargo test -p test-fixtures`, `cargo test -p d2-formats` (feature `test-support` for `cargo clippy -p d2-formats --features test-support`) | the synthetic install end to end (MPQ writer round trips, compile → load → `compare_sets` 73/73, server tables) with M08 perturbations | repo |
| `cargo test -p d2-server world_data` | 8 synthetic DS1 / DT1 provider tests (`--ignored`: C23) | repo |
| `cargo test -p d2-sim wiring::worldgen::tests::routing` | the 7 route tests of `wire-routing` | repo |
| `cargo test -p d2-client --test app_frame_loop`, `--test app_single_player` | headless frame loop (300 ticks) and the GPU node byte check (skipped without an adapter); `Send + Sync`, deterministic build on the server thread | repo (GPU optional) |
| `cargo run -p d2-client -- play [--synthetic] [--seed N] [--frames N]` (needs a display; `xvfb-run` in the cloud) | the app on a window: server ticks ~25/s, `gpu: true`, node frames = frames − 1, exit 0 (§5 C24–C27) | GPU, display |
| `D2_GAME_DIR=<install> cargo test -p d2-sim --test game_world --test game_drlg_tables --test game_treasure --test game_items -- --ignored` | the 42 blind-written game-file tests of `game-tests-*` (§5 C34, C35) | `game/` |
| `D2_GAME_DIR=<install> cargo test --release -p d2-client --test game_assets -- --ignored --nocapture --test-threads 1 --skip gpu_compositor_on_real_frames` | 5 tests: canonical paths read back, the 8 loaders, every `.pl2` / `.cof` / `.tbl`, frame sets and residency on a real DCC, the CPU compositor on real frames (§5 C46; the GPU test of the file is C46 item 3) | `game/` |
| `cargo test -p d2-sim --lib items::inventory`, `cargo test -p d2-sim --lib items::moves` | inventory 32 tests + 1 ignored (T1–T9, B1–B5, E1–E4); moves 59 tests: G1–G3, X1, handler validation order and result codes, `item_actions_match_tsv`, `layouts_match_server_tsv`, `handled_ids_match_client_tsv` (each with a one-cell perturbation test) | repo |
| `cargo test -p d2-sim --lib path::` | 25 tests (P1–P5b, D1–D3, F1–F3 on a synthetic field, `field_tables_match_tsv` against `path-tables.tsv`); `path::search::tests::expfield_live` is ignored (§5 C38) | repo |
| `cargo test -p d2-proto` (modules `s2c`, `tsv`, `codegen`) | the 11 `s2c` tests (§1 3t) and the `bits:` tests (`bits_layout_0x96_bit_ranges`, `bits_layout_strict_errors`, `bits_0x96_round_trip`, `packed_struct_fields`); after any `server-messages.tsv` edit run `cargo run -p data-tool -- gen-proto` first (`generated_file_is_current` fails otherwise: a spec session's TSV edit does not run the generator) | repo |
| `cargo test -p conformance` | 36 pass, 3 ignored: units / stats / packets fixtures against their Python checkers (perturbation tests), the rooms test on the committed traces; `-- --ignored` per recording (§5 C39, A) | repo |
| `cargo test -p d2-sim --test prop_rng --test prop_timer --test prop_lists --test prop_units`, `cargo test -p d2-sim --lib stats::prop_tests` | the `prop-sim-core` properties (`PROPTEST_CASES` overrides the defaults; 1,264 pass in d2-sim + conformance on its branch) | repo |
| `cargo test -p d2-proto --test prop_messages`, `cargo test -p d2-server --test prop_transport --test prop_handle`, `cargo test -p d2-client --test prop_bridge` | `fuzz-server`: no panic or overflow on client bytes, the dispatcher's codes against a spec model, `route_every_id_and_length`; defaults ≈ 4 s in a debug build, hunts `PROPTEST_CASES=20000` / `5000` / `2000` / `3000` | repo |
| `cargo test -p d2-client --test prop_worldsim` (hunt: `PROPTEST_CASES=150`, 215 s) | the wired game over 40–80 operations + 100 quiet ticks: determinism per seed, bounds, no fatal path | repo |
| `cargo test -p d2-sim --lib missiles::tests_bodies`, `cargo test -p d2-sim --lib skills::use_::tests::bodies` | the implemented server-do / server-hit bodies equal the `spec'd-here` TSV rows; `srvst` 18 and the note-vs-body check, each with a perturbation | repo |
| `cargo test -p d2-client --lib scene::tests`, `cargo test -p d2-client --lib capture_tests` | frame cycle, pixel write, palette; 17 capture tests (stability and compare on generated captures, `--perturb`) | repo |
| `py tools/trace-recorder/record_frames.py --selftest`, then `cargo run --release -p d2-client -- verify --cases crates/d2-client/capture-cases --case <id> [--perturb N]` | the frame capture recorder selftest (`selftest ok`); a recorded capture becomes pass / fail: `stability-0001` PASS or an exact finding, the three compare cases `SCENE NOT WIRED` (exit 2) until a `SceneSource` exists (§5 A frame captures) | `game/`, Windows, a player (selftest: repo) |
| `D2_GAME_DIR=<install> cargo test --release -p d2-sim --test game_core`, `… -p d2-server --test game_world_data`, `… -p d2-sim --test game_monsters --test game_skills`, `… -p d2-formats --test game_sweep` (all `-- --ignored --nocapture`) | the 67 blind-written game-file tests of the sixth fold (§5 C37, C45, C46) | `game/` |
| `cargo test -p d2-sim --lib path::` (core 26 + walk 34 + gaps 8), `cargo test -p d2-sim --lib wiring::path`, `cargo test -p d2-sim --lib wiring::inventory` | the rules of `path-placement.md` §1–§6, `pathing.md` §1–§10, the path provider on the action fixture (7: allocation, walk M1 translated, room-edge crossing, wall, waypoint warp lands in the spawn room, provider off, coarse free box) and the inventory wiring (26, every handler through `items::moves::handle`); synthetic, unit tier (supersedes the 25-test `path::` row above) | repo |
| `cargo test -p d2-sim --test prop_path_core --test prop_walk --test prop_path_place --test prop_inventory`, `cargo test -p d2-sim --lib items::moves::prop_tests` (hunt: `PROPTEST_CASES=2000`, ≈ 70 s for `prop_path_place`; `prop-walk` used 10,000 debug / 20,000 release, `prop-inventory` 20,000 and 30,000) | the property models of `prop-walk`, `prop-path-place`, `prop-inventory`: no panic, reference-model equality, M08 mutants caught (the open-list tie order survives by design) | repo |
| `cargo test -p d2-server --lib items::moves`, `cargo test -p d2-server --test prop_handle --test mutants_core --test mutants_adapters` | the 22 host-frame tests of the item moves and the id-table check, the host property test (5 pass, ≈ 0.7 s; `PROPTEST_CASES=3000` for a hunt), the 25 mutation kills | repo |
| `cargo test -p d2-client --lib rules::`, `cargo test -p d2-client --lib world_view::feed`, `cargo test -p d2-client --lib verify::capture_case::scene_tests`, `cargo test -p d2-client --test app_frame_loop` | camera / placement vectors and the CPU golden scene (16), the feed (3), the synthetic capture end to end (5 + 1 ignored GPU), the frame loop (6, one drawn frame per tick) | repo (GPU optional) |
| `cargo test -p d2-client --test prop_client_bridge --test prop_world_view --test prop_controls --test prop_ui --test prop_atlas --test prop_scene --test prop_cache` (hunt: 3000–5000 cases, 400 for world view and atlas) | the `prop-client` properties and the `regress_*` tests of its five root fixes | repo |
| `cargo test -p conformance --test movement_replay --test placement_replay --test recordings_needed`; `cargo run -p conformance --bin recordings-needed` (`D2_TRACES_RAW=<dir>` for another folder) | the synthetic movement / placement fixtures and their perturbations; the list of recordings still needed with MISSING / present / BLOCKED per entry | repo (`traces/raw/` for the last) |
| `cargo bench -p d2-sim --bench sim`, `cargo bench -p d2-formats --bench formats`, `cargo bench -p d2-proto --bench proto`, `cargo bench -p d2-client --bench compose` (append `-- --warm-up-time 1 --measurement-time 2` for a quick pass; `compose` builds Bevy) | criterion baselines (§1 3ad); not in CI; `cargo bench --no-run` and `cargo clippy --workspace --all-targets` compile all four | repo |
| `cargo mutants -p d2-server --timeout 60`; `cargo mutants -p d2-sim --file 'crates/d2-sim/src/missiles/**' --file 'crates/d2-sim/src/monsters/**' --timeout 10 -j 3 -- --lib` (re-test only the survivors: `--iterate --timeout 20`) | mutation counts of §1 3ad (METHODS M08); `mutants.out/` is not committed | repo, `cargo-mutants` 27.1.0 |
| `D2_GAME_DIR=<install> cargo test -p d2-sim --test game_inventory_path -- --ignored --nocapture`; `py tools/trace-recorder/path_tables.py` | the 6 game-file tests of the inventory grids, belts and path tables (§5 C51); `path_tables.py` must agree (exit 0) with `live_path_tables_equal_game_exe` | `game/` |

CI job layout (`.github/workflows/ci.yml`, `ci-speed`): five independent
jobs in parallel, each with its own `Swatinem/rust-cache` `shared-key`;
`concurrency` cancels a superseded run of the same ref; `CARGO_INCREMENTAL=0`.
The union of the clippy and test jobs is the old `--workspace` run.

| Job | Runs | Bevy libs |
|---|---|---|
| `tools` | `spec_index --check`, `methods check`, `coverage --check` + `--selftest`, every trace-checker selftest + `convert_tick --check` + `check_rooms`, `tools/hooks/selftest.sh` (pre-commit patterns in a scratch repo + every tracked file), `cargo fmt --check`, `cargo run -p depcheck` | no |
| `clippy-core` | `clippy --workspace --exclude d2-client --all-targets -D warnings` | no |
| `test-sim` | nextest `-p d2-sim -p conformance` (includes `tick_replay`), then `cargo test --doc` | no |
| `test-rest` | nextest of everything but d2-sim / conformance / d2-client (includes `d2-proto` `generated_file_is_current`), then `cargo test --doc` | no |
| `client` | apt libs, `clippy -p d2-client --all-targets`, nextest `-p d2-client`, doc-tests | yes |

nextest has the selection of `cargo test` (`#[ignore]` stays skipped;
`--no-tests=pass` for crates without tests; doc-tests run via `cargo test
--doc`). Proptest: CI leaves `PROPTEST_CASES` unset (the tests read it and
*override* their in-source defaults); only `PROPTEST_MAX_SHRINK_TIME=10000`
is set. The pre-commit hook sees only staged files, so
`tools/hooks/selftest.sh` tests its patterns on synthetic files and scans
`git ls-files`. Items needing `game/`, a GPU or Windows stay out of CI (the
"Needs" column above). The cloud llvmpipe checks need `apt-get install
mesa-vulkan-drivers libvulkan1` (and `xvfb`, `libxkbcommon-x11-0` for a
window run), not part of `tools/cloud-setup.sh`; without them the GPU tests
print `skipped: no GPU adapter` and pass.

Eighth fold additions: `sh tools/props-deep.sh [sim|wire|worldsim|all] [cases]`
(the nightly job's filters; logs in `target/props-deep/<group>.log`, failing
inputs printed and uploaded as artifact `proptest-<group>`); `cargo mutants -p
d2-sim --file '<dir>/**' -j 3 --timeout 60 -- --lib` for a mutation run (then
`--iterate` on the same output directory; `mutants.out` is never committed);
`cargo bench -p d2-formats --bench formats -- mpq_huffman` (the Huffman group);
`cargo test -p conformance --test s2c_builders` and `cargo test -p d2-sim --test
s2c_bytes` (the sim's byte builders against `d2_proto::s2c`); `cargo test -p
d2-server --test game_wired_host -- --ignored` (C59, game files);
`cargo test -p test-fixtures --test synthetic_game` (the game from a synthetic
install, CI).

## 5. Local run queue

Ordered, copy-pasteable guide to running this queue: `docs/LOCAL-RUN.md`.

Cloud sessions add checks here (command + what to look for); a local
session runs them, records the result, and removes the entry. Merged
2026-10-06 from the 13 notes in `docs/handoff/`, then from the 15 notes
of the second fold (`wire-*`, `p6-gpu`, `p6-verify`, `p6-cof`,
`p6-verify-gpu`, `coverage-claims`, `impl-skilluse-vitals`,
`impl-monster-*`, `impl-drlg-{preset,maze,outdoor}`, `impl-vendors`,
`impl-npc`), then from the 10 notes of the third fold (`server-items`,
`server-world`, `server-skills`, `wire-worldgen`, `wire-interaction`,
`gaps-items-stats`, `gaps-combat-ai`, `gaps-data-formats`,
`gaps-drlg-world`; `p5-local-server` needs no check), then from the 7
notes of the fourth fold (`quest-host`, `e2e-next`, `gaps-data-rng`,
`gaps-client-formats` add checks; `determinism-lint`, `gaps-drlg-sim-world`,
`gaps-combat-items-monsters` queue none) and the earlier queue, deduplicated and grouped by what the run
needs (the fifth fold, from the 10 notes `p6-window`, `verify-map`,
`wire-routing`, `drlg-data`, `fuzz-data`, `game-tests-drlg-world`,
`game-tests-items-treasure` add checks; `client-own-gaps`, `synthetic-data`,
`ci-speed` queue none; the sixth fold, from the 22 notes `conformance-harness`,
`render-capture`, `render-composition`, `impl-inventory`, `impl-path-place`,
`p6-integrate`, `game-tests-{sim-core,monsters-skills,client-assets}`,
`e2e-combat-path`, `wire-open-seams`, `e2e-vendor-host` add checks;
`fuzz-server`, `prop-sim-core`, `prop-worldsim`, `impl-moves`,
`s2c-builders`, `proto-bits`, `spec-bodies-*`, `p6-world-view`,
`e2e-single-player` queue none of their own): **A** the player at the game (recordings), **B** Ghidra / spec edits
only (no game run), **C** game files only (no player; includes the checks
that need a real GPU). Order inside a group is the order to run.
"Expect" lines are the pass condition; a different result is a finding
for the owner spec, not a reason to change the numbers.

Queue size at `docs/handoff` third fold (entries as numbered or bulleted below): A player 24 (4 lead entries + items 1–20; the third fold added (19), (20) and extended (9), (13), (16)); B Ghidra / spec edits 27 items in the two bullets of the third fold (13 Ghidra reads, 14 spec edits) beside the earlier ones; C game files and GPU 19 numbered (the third fold added C17–C19 and extended C2, C5) plus 3 kept entries; Blocked 8 (2 added, the level-types entry extended). Nothing in the third fold needs a new player recording beyond (19) and the cube / world recordings already queued.

Fourth fold: A player none new; B Ghidra / spec edits 1 bullet (the
`gaps-data-rng` and `gaps-client-formats` spec questions); C 2 new numbered
(C20 the `ds1` game test, C21 the `automap.bin` check) and C17's command
list gains nothing (the `formats_game` run already includes C20's test);
Blocked 2 added (the quest-host and e2e-next replays).

Fifth fold: A player 1 recording (RT-R1 below, `record_tick.py` with timer
hooks); B none new (the questions RT1–RT6, PW1, VM1, FZ1, FZ2, DQ1 are
spec work, §7); C 13 new numbered (C23 world_data live check, C24–C27 the
window and the GPU node, C28–C31 the `map` case, C32–C33 the fuzz-data
re-checks, C34–C35 the game-file test runs), plus the interpretation rule
below for C3–C6, C14, C19; Blocked 2 added (the RT replays), 2 changed
(the `map` GPU case moved to C28, the post-routing `tick_replay` re-run is
now a plain repo check).

C3, C4, C5, C6, C14 and C19 below now have test code in the two
`game-tests-*` notes (C34, C35): run those commands instead of writing the
tests; the entries keep their expected values. The notes' interpretation
points (what to look at first when a test fails) are in C34 / C35. After a
pass, turn each `// Claim once the first local run passes` line into
`// Covers:` (same ids), rerun `py tools/coverage.py --check`, and record
the result (`docs/COVERAGE.md` §3). A failing test is a finding for the
owner spec, not a reason to change the numbers.

Sixth fold (22 notes; the five older ones add checks too): A player 1 new
group (frame captures, `capture.md` §8) and 3 additions to existing entries
(the units, stats and packets replays of `conformance-harness`; the
position trace of `path-placement.md` OQ1; the `inventory.md` R1–R6
recordings); B none new (spec work is §2 steps 12–13 and §7 sixth set); C 15
new numbered (C36 `impl-inventory` D1–D3, C37 `game-tests-sim-core`, C38
`expfield_live`, C39 packets replay baseline, C40–C44 `p6-integrate`, C45
`game-tests-monsters-skills`, C46 `game-tests-client-assets`, C47–C49
`render-composition`, C50 AnimData through `anim_record`); Blocked 7
bullets added. The game-file tests of C37, C45, C46 are the code for the
still-unwritten C2, C7, C10, C11: run them instead of writing tests, and
turn each `Intended claim` line (or the claim table of `game-tests-
monsters-skills` §4) into `// Covers:` only for a test that passes.

Seventh fold (2026-10-06, 21 notes): A gets five
additions (S7-A1 walk recording, S7-A2 camera check, S7-A3 path state and
placement calls, S7-A4 after the recorder gap, S7-A5 inventory recordings);
B none new (spec work is §2 steps 12–14 and §7 seventh set); C seven new
numbered (C51 `game-tests-inventory-path`, C52 DC6 / DCC / DT1 counts, C53
window with real files, C54 real GPU, C55 and C56 the movement and placement
replays on the existing recordings, C57 the recordings list) and C58
(optional benches); Blocked one bullet. Notes with no local check:
`host-merge` (its replays, `e2e-vendor-host` §8, `quest-host` §7,
`server-items` §6, now run through one host), `host-merge-port`,
`impl-path-core`, `impl-walk`, `wire-path-sim`, `wire-inventory-sim`,
`wire-inventory-server` (R1–R6 replays only, S7-A5), `prop-*`,
`fix-statlist-prop`, `mutants-*`, `gaps-path-render`, `gaps-new-specs`,
`bench-baselines` (C58 optional).

Eighth fold (2026-10-06, 17 notes): A gets four additions (S8-A1 two clients
in the walk recording, S8-A2 dropped-item pick-up and the vendor replay,
S8-A3 drop position, S8-A4 stat-change traces); B none new; C six new
numbered (C59 `game-tests-wired-host`, after step 7u(a); C60 `GameData::load`
on the live set; C61 and C62 the Huffman decoder on game files and the
`.wav` weight tables; C63 `VendorTables::from_fixed`; C64 mutation survivors
that only game files decide); Blocked: the walk replay (C56) no longer waits
on a server handler. Notes with no local check: `ci-nightly-props` (the
nightly runs on GitHub), `unify-items`, `path-update-pass`, `prop-fixes`,
`prop-walk-gaps`, `mutants-core`, `mutants-combat-skills`, `s2c-use`,
`spec-unit-numbering`, `fixedset-game` (C60 optional), `drop-freespot` (S8-A3),
`wire-path-server` (S8-A1).

Index: Done · A player · B Ghidra / spec edits · C game files and GPU · Blocked.

### Done (kept for the record)

Done 2026-10-06 (local PC 2, coordinator #2, main `63a706b`, Windows 11
Pro 26200; `hash-manifest` of the install = `traces/reference-install.toml`
byte for byte, 19 entries): `cargo build --workspace --release` OK;
`cargo run -p depcheck` OK; `cargo test --workspace -- --ignored`: 66 pass,
5 fail, the same five `game_sweep` counts as the entry below (stops at the
first failing crate, so not a full count). **Second real GPU: AMD Radeon
RX 9070 XT (Vulkan, DiscreteGpu, driver "AMD proprietary driver 26.8.1
(LLPC)")**: `d2-client verify` (release) `summary: 11 pass, 0 fail, 0
error, 0 GPU not wired, 0 no adapter`, map `townN1.ds1` 2,697 draw items,
view 7840×4112 at −3200,−192, CPU binned / GPU indices / GPU RGBA all 0 of
32,238,080 differ; `--perturb 7`: all 11 cases FAIL with exactly 7 on each
of the three comparisons, `0 pass, 11 fail`, exit 1. GPU byte-exactness
now holds on two real adapters (Intel iGPU and AMD discrete). Fresh table
dump `traces/raw/20261006-201456-tables` (73 tables, 30 maps, not
committed). The full LOCAL-RUN batches 1–5 on main `0472619` run as the
buddy session (`docs/handoff/local-buddy-2026-10-06.md`, branch
`claude/local-buddy-2026-10-06` `6e406d4`): install identical; batch 3
(real GPU) all pass incl. `gpu_compare` 18/18; batch 2 has 9 failing rows
(verbatim there); **`d2-client play --frames 1500` on live data panics at
`d2-sim/src/drlg/room.rs:144` ("live DRLG room") after ~104 ticks**.

Done 2026-10-06 (local PC 2, coordinator #2; Ghidra 12.1.4 export of the
1.14d `Game.exe`, 13,048 functions, no community labels: `1.14d-notes`
absent on this PC). Branches to merge (each verified with `git ls-remote`;
all from main `0472619`; spec edits pass `spec_index --check` and
`coverage --check`):
- `claude/spec-answers-tick-messages` `9b432d3`: `tick.md` §1.2 wrap
  (signed, only `now` masked), §4 r5 off-tick population (`population.md`
  OQ1), §5.2 r2 owns expire −1, §5.6 freeze gate; S→C 0x2A, 0x50, 0x58,
  0x5D, 0x63, 0x91, 0xAC header layouts; C→S 0x3A, 0x4C; classifier
  table corrected (0x71–0xFE → 3); masked bytes in `intents-events.md` §6
  r3; `rng.md` §5.3 / §7; `calc-expressions.md` §3.5. Regenerated
  `d2-proto` (fmt, `cargo check -p d2-client --tests`, `cargo test -p
  d2-proto` pass). Open: 0x28 byte 6 on the NPC chat path; 0x28 needs a
  TSV grammar for a fixed-size tail.
- `claude/spec-answers-inventory` `cce8b7e`: `inventory.md` OQ3–OQ11,
  OQ14–OQ17, OQ19, IV1–IV8, MV1, WN1–WN3, PN1, GX1, IS1 (part); `cube.md`
  OQ8. Not done: S→C 0x22 TSV cell (`type:u8@1 unit:u32@3 skill:u16@7
  quantity:u8@9 body_state:u8@11`, then `gen-proto`).
- `claude/spec-answers-path` `b4eb5f6`: `path-placement.md` OQ2–OQ6 (new
  `pathing.md` §1.6 C→S 0x5F resync), `pathing.md` OQ4, OQ5, OQ8; W5, W6
  (§4 rule 6), `0x0064E840` link.
- `claude/spec-answers-render` `6b8dc11`: `composition.md` OQ1 / VM1
  (entry 0 black in all 17 PL2 / `.dat`), OQ3; `sprite-placement.md`
  OQ1–OQ5; `camera.md` OQ1, OQ2, OQ5–OQ7; `capture.md` OQ3, OQ5;
  `render-pipeline.md` §A9 wording. `draw-order.md` OQ9 is answered in
  `camera.md` §7 (for its owner).
- `claude/spec-answers-units-stats` `9395fa4`: GI3, GI5, GI7; `units.md`
  §3.1, new §6.6 (unit events), §6.5 replenish; `stat-lists.md` §8.8,
  §10.4 (endless loop confirmed), OQ2; WE1, WE4, WE6–WE9, W13, W14,
  `properties.md` G1; `use.md` §1–§7, OQ1, OQ2 (identity), new §5.5 (WI
  I8); `vitals.md` OQ2 (§4.3 rewritten), OQ3; WI I3 (`npc.md` §8.2).
  Expect a small conflict with the tick branch in `vitals.md` (adjacent
  OQ lines + index block).
- `claude/spec-answers-drlg` `91b4a34`: new `levels.md` §11 (DRLG data
  population reads: coordinate lists, populated level / room count, warp
  points, kind-11 spawn), WG1, WG6; `rooms.md` §9.2, §9.5, §9.6 (new
  `wall-remap.tsv` + `.md`), §9.7, OQ10, new §2.1; `levels.md` §10.2, edge
  r3; `preset.md` §6 steps 9–10, §11; `maze.md` §2.4, §3.7, §5.5, §7.1,
  §9; `outdoor.md` §6 r4, §7.5.3 / OQ6 (new `outdoor-path-floor.tsv`),
  §8.3, OQ8, OQ9. Open: OQ7 jungle placer. `rooms.md` (~64 KB) and
  `outdoor.md` (~61 KB) are past the split guideline.
- `claude/spec-formats-wav` `40002d8`: new `formats/wav.md` (all 4,992
  RIFF blocks are PCM tag 1; no in-file ADPCM; no resampling on load).
- `claude/spec-audio-sound-table` `3167367`: new `audio/sound-table.md`
  (closes `client/audio.md` §B3, part of §B8). Design note: 1.14d draws
  sound variants from the local player's client unit seed, not a separate
  audio seed (`audio.md` §A2 plans one).
- `claude/live-c13-ds1-survey` `e8f7e1b`: C13 `preset_ds1_survey` +
  `preset_ds1_scan_vectors` pass, every stated number exact (C13 done).
- `claude/live-c20-treasure-dump` `6209859`: C20 pass after fixing TC 0
  picks 1 → 0 (code + `treasure.md` §1.1–§1.2); 1,012 other TCs, 4,167
  entries, 45 chest slots identical (C20 done).
- C2: no new code needed; `game_core::itemstatcost_ops_as_stated` and
  `itemstatcost_columns_as_stated` pass on live data (perturbation of
  record 214 byte 0x53 caught). C2's `StatData` part is covered.

Code fixes these spec answers require (implementation sessions; each is an
escape in §8): `items/inventory` targeting seam passes the player, not the
item (`inventory/mod.rs:604`, `checks.rs:117`); `path::walk` monster re-path
budget is path +0x94, monsters only; the position history belongs in
`d2-sim` (AI reads it, `path-placement.md` §10 r7); `path/warp.rs`
`EXIT_LEFT` is 10, not 11 (test near line 203 expects the old mapping);
`mpq-tool formats` name set must ignore case (then re-derive the
`game_sweep` expected counts, buddy note G1); `render/map-preview.md`
roofs 80 rows too low (needs a capture under a roof); `units` allocation
order and the `SUNIT_Add` result (`units.md` §3.1).

Done 2026-10-06 (local, main `63a706b`, `D2_GAME_DIR` = the reference
install): `cargo run -p depcheck`: OK (8 crates, d2-sim determinism lint
clean). `cargo test --workspace --no-fail-fast -- --ignored`: 112 pass,
**13 fail** (findings for the owners; expected values not changed, rerun
on the current main before fixing):
- `d2-formats/tests/game_sweep.rs` (5): `dc6_every_file_decodes` 1,653 vs
  1,657, `dt1_every_live_file_decodes` 250 vs 254, `ds1_every_file_parses`
  2,372 vs 2,456, `string_tables_every_key_resolves` 29 vs 33 tables: the
  sweep enumerates `(listfile)` names only and `patch_d2.mpq` has none; the
  expected counts are `mpq-tool formats`' (listfile ∪ its known-names list
  in `tools/mpq-tool/src/formats.rs`). Fix the enumeration, not the
  numbers. `cof_every_live_file_parses`: asks for
  `data\global\charsm\cofmblxbw.cof` (not found; the junk file is
  `amblxbow.cof`).
- `d2-sim/tests/game_drlg_tables.rs`: `act1_placement_on_live_tables`
  gives `[5, 27, 6, 7, 26, 39, 17, 1, 2, 3, 4]` (the recorded allocation
  order 4, 3, 2, 1, 17, 39, 26, 7, 6, 27, 5 reversed); expected has
  16, 15, …, 8 in front. `lvlprest_measurements`: 1,079 vs 82.
- `d2-sim/tests/game_world.rs`: `waypoint_objects` FrameCnt1 of object 119
  = 3,840 vs 15 (= 15 × 256: an 8.8 value read raw);
  `cubemain_vector_records` 129 vs 130; `vendor_columns_from_live_items`
  panics on column `weapons.HratliMin`.
- `d2-sim/tests/game_items.rs` `sweep_create_every_item_every_quality`:
  570 failures, first "item 39 dgr q 8 ilvl 1: crafted affix 0 with a
  filled slot (read at 0x5C)" (possible code / spec finding, `quality.md`
  crafted).
- `d2-sim/tests/game_treasure.rs` `sweep_drop_quality_every_item`: item
  520, L 0, M −100: "magic gate".
- `d2-server` `world_data::tests::game::outdoor_levels_generate_through_the_dispatcher`:
  97 vs 98.

Done 2026-10-06 (local, captures with the player): render captures with
`record_frames.py` (branch `claude/spec-render-placement`), 800×600 GDI:
run 1b Den of Evil (14,823 frames, 15,711 ticks; still segment 1,151
frames, 140 distinct images: cursor + light flicker, §7 "first 1.14d frame
captures") and run 2 Rogue Encampment (1,111 frames: walk, run, stop, Town
Portal). Run 1 (automap open) kept as a record. `stability-0001` therefore
**does not pass as specified**; the comparison rule needs the §7 changes
before placement / camera / composition cases can be judged.

Done 2026-10-06 (local, branch `claude/local-2026-10-06` from `edad871`;
group C on game files and the real GPU; entries C1, C8, C9, C15, C16,
C17, parser robustness and treasure memory dump removed below):
- `data-tool tables`: 73 runtime tables, 72 identical, 1 explained,
  code buffers 4/4 identical (unchanged; the behavior-neutral changes of
  `gaps-data-formats` moved nothing).
- `cargo test -p d2-data -p d2-formats -- --ignored`: 50 pass after 5
  blind-written assertions were corrected from the observations (§8):
  `calc::tests::game::text_compile_reproduces_code_files` (record-major
  order, as `bin::formula_fields`); `game_data::compile_only_lookup_tables`
  (hitclass has a live source; monmode / plrmode `.txt` also in X and D,
  16 / 20 records; skills P 357 / X 319 / D 221; sounds 4,699 / 4,698 /
  3,587; monstats 734 / 575 / 410); `loading_edge_cases` (`properties.code`
  is a name linker); `formats_game::animdata_edge_cases` (spec corrected:
  the `.cof` matches the first copy for `64A1HTH`, `64NUHTH`, `MINUHTH`,
  the second for the other 6, `animdata.md` Duplicates);
  `dc6_zero_size_frames` (spec corrected: 0 zero-size frames in the 1,651
  DC6 listed by X and D, `dc6.md`; `patch_d2.mpq` has no `(listfile)`, the
  6 patch DC6 `mpq-tool formats` finds are not in that count).
- `mpq-tool check`: all blocks decoded; `mpq-tool formats`: 0 errors
  (cof 3,606 files, 3,605 parsed + `amblxbow.cof`; dc6 1,657).
- C1 `cargo test -p d2-sim -- --ignored`: 5 pass (`codes_match_game_tables`,
  `real_skill_vectors`, `real_table_constants`, `real_level_stats`,
  `live_monstats_records`); an ```` ```ignore ```` doc snippet that
  `--ignored` compiled (`wiring/interaction/skill_events.rs`) is now
  ```` ```text ````.
- C2 (part) `check_stats.py --files game`: pass (359 stats, 84 ops, 42
  op targets). The `StatData` live-row test is still unwritten (C2 kept).
- C8 frames: pass; 23,595 files (6 parse errors), 288,702 frame sets,
  3,345,171 frames, largest 96×960 (`expansionaallair\worldstone.dt1`
  tile 50) ≤ 2046.
- C9 COFs: pass; 3,511 distinct names + `amblxbow.cof` (the test
  deduplicates names across archives; `cof.md`'s 3,605 counts files),
  242,300 frames, 0 failures: `composite` OQ1 needs no rule.
- C15 real GPU **Intel(R) HD Graphics 630, Vulkan, IntegratedGpu** (driver
  string not reported by wgpu): `gpu_compare` 12/12 `0 differing bytes`,
  `--perturb 7` 12 × `7 differing … 7 perturbed`, `Error: 12 of 12 cases
  differ`, exit 1; `gpu_compositor::tests::gpu` 2 pass;
  `verify::tests::gpu_half` pass.
- C16 `d2-client verify` (release): 11 cases; map `townN1.ds1` 2,697
  draw items, view 7840×4112 at −3200,−192, PASS; synthetic 10 × 0 of P
  differ, PASS; `summary: 11 pass, 0 fail …`, exit 0. `--perturb 7`: every
  case 7 (CPU binned, GPU indices, GPU), map 7 of 32,238,080 (stable
  render on the third capture), `0 pass, 11 fail`, exit 1. `--case map
  --perturb 5`: 5 pixels, exit 1. No panic or hang with two wgpu
  instances in one process. GPU byte-exactness is now proven on one real
  adapter.
- Treasure memory dump (`dump_tables.py` now also writes `tc_records`,
  `tc_entries`, `tc_chest`; new `check_treasure.py`):
  `traces/raw/20261006-115547-tables` (raw dump, not committed): 1,013
  TCs, 4,167 entries (763 in the 160 automatic TCs), 2,742 TC / 660
  `treasureclassex` item (81 `mul`) / 2 unique / 0 set entries, TC 430
  (group 12, picks 1, nodrop 100, ids 523 / 218 / 203 / 370, starts 0 /
  21 / 37 / 58, totals 60 / 60), chest table 45 found, `Act 1 Chest A` =
  TC 385: 0 failures; `--perturb 430` reports exactly TC 430 (M08). Not
  yet done: a byte compare of the whole dump with d2rs's built
  `TreasureClasses` (new C20).

Done 2026-10-06 (local): fix-ups (`dump-compare
traces/raw/20261006-021210-tables`: 70/70 tables, every map with a d2rs
counterpart identical, nothing pending; `fixups_on_live_set` ok) and
patch layers (`patch_game` 5/5 incl. G1–G8; `data-tool patch check
game/patch-example/overhaul.d2stack`: exit 0, one N01 note, data digest
`66010ecda7c8df5b7135579888c536fd2a30287fb877848719a31b6c8f97a625`).

Done 2026-10-06 (local, `main` at `bc739e4`): parser robustness. `cargo
test -p d2-data -p d2-formats -- --ignored` 29/29 pass; `data-tool tables`
73 runtime tables 72 identical, 1 explained, 0 mismatched, code buffers
4/4; `mpq-tool check` all blocks decoded; `mpq-tool formats` 0 errors in
every kind (cof 3605/3606, dt1 254/260: the 7 gaps are the `KNOWN_UNUSED`
files, failing as expected). No file reaches the new whole-file limits
(DC6/DCC ≤ 64M pixels, DT1 blocks ≤ len/20, TBL bytes ≤ len).

Phase 3 recordings: done 2026-10-06 (first skipped at the user's
request, then recorded when the user asked). Combat with missiles
`traces/raw/20261006-022304-tick.jsonl` (4,632 ticks, 24,894 timer runs
incl. 1,580 missile, 186 snapshots) and melee `...-021854-tick.jsonl`:
`check_tick.py` 0 errors. Messages `...-022633-packets.jsonl`:
`check_packets.py` OK, perturbation reported, new client ids 0x0C,
0x3A, 0x3B. Still open: a hosted (multi-client) game.

### A. Needs the player (record_*.py, Windows, `game/`)

**RT-R1 (fifth fold, `wire-routing` §7 check 2):** a recording of a unique
monster with a mode-1 umod dying (e.g. fire enchanted, umod 9), group A:
`record_tick.py` with the timer hooks. Look for the type-7 timer's frame
(death frame + 4) and its position among the death animation's timers in
the queue; it settles RT1 (where in `0x005A7C20` the umod mode 0 / 1 calls
sit).

**Frame captures (sixth fold, `render-capture` §"Local run queue";
`render/capture.md` §8).** `raw = "latest"` reads the newest
`traces/raw/*-frames.jsonl`, so run each verify right after its recording
(or set `raw` in the case file; with `--out` elsewhere give `images =`).
1. `py tools/trace-recorder/record_frames.py --selftest` → `selftest ok`.
2. **stability-0001**: `py tools/trace-recorder/record_frames.py --seconds
   90`, Single Player, any character, Den of Evil cleared, a dead end away
   from doors, stand still 30 s, no panels. Then `cargo run --release -p
   d2-client -- verify --cases crates/d2-client/capture-cases --case
   stability-0001`. Expect `re-hash: 0 of N frames differ`, `stability: G
   state groups, K seen at least twice, 0 with differing frames` with
   K ≥ 2, `PASS stability-0001 (scene)`, exit 0; with `--perturb 1`:
   `re-hash: 1 of N frames differ`, FAIL, exit 1. A differing group or K < 2
   is a `capture.md` finding (OQ1: the state key misses an input), not a
   reason to loosen the check; trust this verdict over the recorder's own
   `0 with differing frames` line (CP3). Record every `flag: … ticks carry
   more than one frame` line (OQ2).
3. **placement-0001**, **camera-0001**, **composition-0001** (`capture.md`
   §8 rows 2–4): record each as the table says, then `… --case <id>`.
   Expect today: no ERROR (every PNG matches its record), `N frames
   selected`, `frames: 0 match, 0 differ or fail, N scene not wired`,
   `SCENE NOT WIRED <id> (scene)`, exit 2. Once a `SceneSource` is wired
   (§2 step 7l): PASS, and `--perturb 5` → every frame `CPU: 5 of 480000
   bytes differ`, FAIL, exit 1. `composition-0001` (a translucent sprite on
   a known floor, after `stability-0001`) is also the capture that
   confirms `render/composition.md` §5 (§5 C47–C49 hold the rest).

**Additions to the units / stats / packets recordings** (sixth fold,
`conformance-harness` §5): after item 1 below, `cargo test -p conformance
--test units_replay -- --ignored --nocapture`: pass, printed `schedules` >
0 (a failure names the record index; cross-check with `check_units.py` U4
"anim exact" at the same index); after item 2,
`cargo test -p conformance --test stats_replay -- --ignored --nocapture`:
pass with `operations`, `callbacks`, `snapshots` > 0 (a mismatch names the
record as `check_stats.py` numbers it; `seed …` = d2-sim does not rebuild a
dumped tree, `stat-lists.md` §6 / §11; `isc[s].…` = a load-time column,
`fixups.md` §2; `callback.u` = the unit argument, §7). Both are `trace`
tier: add `// Covers:` claims to the ignored tests only after they pass.
`convert_tick.py` rejects `record_tick.py` 0.2.0 recordings (it raises
"unknown record kind" for `anim`; the units harness reads the raw file and
is unaffected): converting the item-1 recording to a `sim/tick` trace
fails until the converter skips `anim` (CH2, step 7m).

**Other recordings the sixth-fold specs ask for** (recorder extensions per
M10 first):
- A position trace (`path-placement.md` OQ1; R1–R3 of the spec: game entry,
  waypoint travel and warp arrival with their 0x15 / 0x0D, `impl-path-place`
  §5); no recorder exists. Without it `path::` stays unverified apart from
  `ExpField.D2` (C38).
- Item moves R1–R6 (`inventory.md` Test vectors; grid positions R2, belt
  slots / compaction R4) and the item bit stream of 0x9C / 0x9D
  (`inventory.md` OQ1).
- A sorceress casting at a monster that dies (`e2e-combat-path` §6 check 2;
  `record_packets.py` + timer hooks): event-0 / event-1 frames after 0x0C,
  the missile's creation and hit frames, the kill's mode change, the
  experience delta, the drop's item GUID and gold; replay through the e2e
  with recording-backed seams settles EC1–EC3.
- An aura recording (`wire-open-seams` §7 check 1; `use.md` §7): a paladin's
  aura right skill; type-8 timer frames ≡ 1 mod `perdelay` and the do calls
  per frame. Quest recordings (check 2): the Den of Evil kill → status 5
  after the updater, Kashya message 92 → 0x50 / mercenary spawn; settles WO2.

Batch with the recorder extensions per M10 first; the exact hooks are in
each spec's open questions. One session can cover several.

**Phase 3 units/stats recordings** (local, needs the user at the game;
one session, batched with the topic writers' requests):
1. Units: `py tools/trace-recorder/record_tick.py --seconds 240` (record_tick
   0.2.0) on Normal: melee and skill combat, a shrine, a well, a vendor or
   trade, a cooldown skill. Then `check_units.py` and `check_tick.py` on
   the file: 0 errors each, "U4 anim exact" and "U11 site rule" counts
   above 0. Repeat on Nightmare: AI delays use the Normal `aidel` column
   unless game +0x6A / +0x74 is set (`units.md` OQ7). One recording near
   trap objects (with `record_rng.py`) for the seed of object event 0/8
   delays.
2. Stats: `py tools/trace-recorder/record_stats.py --seconds 240`: equip and
   unequip a +vitality/+life item and a weapon, drink a health and a mana
   potion, spend a stat point, get poisoned or use a timed buff, fight,
   walk and run. `check_stats.py <file>`: 0 errors, callbacks, expiry
   frees and regen checks all above 0; `--perturb-snap 5` and
   `--perturb-cb 0` must fail at the changed record. The max-life
   rescale sets must match too (`stat-lists.md` §7.2).

   entry (TC index = (ptr − [0x96C5EC]) / 0x2C, quality, level, mode),
   `0x0055AEE7` (item id ECX, quality, index, flags) and `0x0055A9B9`
   (nodrop, total, n, result EAX, FPU control word); kill monsters incl.
   champions and uniques with `players 8`, open chests. Draws in
   `0x0055A6D0`–`0x0055AF80` and from `0x00558640` must match
   `treasure.md` §Randomness in order and count.

**Phase 3 topic recordings** (player; one session, batched; recorder
extensions per M10 first; exact hooks in each spec's open questions):
items R1 (item creation: seed sets `0x00552DF0`/`0x00552E90`, every draw
on the item's unit and item seed until `0x00558D90` returns, dump of the
result); skills (hit/block `0x0057DB61`, `0x0057E04B`; damage
`0x0057DBF0`, `0x0057C6C0`; leech `0x0057C420`; mana `0x0056BFE0`;
start/do on an ally `0x0056FAF0`, `0x0056F7F0`; Strafe/Zeal frame codes);
monsters (new Normal game, Blood Moor and Cold Plains incl. a champion
pack, seed-step hooks listed in `population.md`/`init.md`, monster x/y
and type flags; Nightmare melee, freeze, knockback, Fallen Shaman
resurrect; missile exits `0x005AE1F0`, `0x005ADF10`); world (Flavie chat
RNG + packets; game start mode at `0x00546270`; act transitions via
Warriv/Meshif, 0x61; Inifuss scroll 0x50; waypoint to another act and an
undiscovered waypoint twice; cube transmutes of records 23, 2, 19, 104;
hire, resurrect, Akara heal, Cain identify; repair one and all; gamble
open, buy, 0x37; leave town and trade with Charsi again); vitals (hook `0x00570880` entry/exit and
`0x00570D60`, stats 4–13 before/after, one level-up and stat spending);
DRLG (full RNG hook: Den of Evil, Cave 1, Acts 2–4 with drlg +0x94,
+0x484, +0x474; entry returned by `0x0066D820`; `record_tick` extended
with each active room's DRLG rect, near list, status/counts, client
count and counter, `rooms.md` Test vectors).

Next RNG capture when convenient (local, needs the user at the game):
start `py tools/trace-recorder/record_rng.py --seconds 120`, enter a
single-player game, kill a few monsters and pick up a drop. Recording
slows the game a lot (all 846 inline RNG sites are hooked).


**A-new. Replays and recordings from the implementation notes** (these
use existing recordings unless said; no player needed for the existing
ones, but they need `traces/raw/`, so local):

- *Existing recordings, no player.* (1) Missile lifetimes
  (`claude/impl-monsters`): drive `missiles::MissileDispatch` from
  `traces/sim/tick/` and `traces/raw/20261006-022304-tick.jsonl` with a
  path fake; compare per-GUID run counts with `missiles.md` Test vectors
  (69/69). Expect: full lives = `Range` runs; early removals need
  `missiles.md` OQ1 (exit-path recording). Also through
  `wiring::action::ActionSim` with a recording-backed `Pending` path
  (positions, crossed sub-tiles): per-GUID runs and the RNG draws on the
  owner / missile / monster seeds must match (`wire-action.md` §7.1). (2) Think intervals:
  per-class type-2 delay histogram of `ai.md` Test vectors (zombie1 25
  ×502 …) with an `AiHost` fake fed the recorded positions; needs the
  target-node lists (`ai.md` OQ7). (3) Adjacency order (`rooms.md`
  queued checks C1–C4): replay `ract` / `rdeact` from the tick recordings
  against `drlg::Drlg::client_changes_room` / `remove_active_room` once
  the world session drives them. (4) U4 / U11 compare of `units::anim`
  schedules and the dispatch, after the `record_tick.py` 0.2.0 recording
  of A1 exists.
- *Needs the player (new recordings).* (5) Replay the `record_stats.py`
  recording through `StatLists` in `crates/conformance` (base / full / mod
  arrays, chains, flags, callbacks with nested writes, expiry, regen);
  also settles `stat-lists.md` OQ1. (6) Item-creation recording R1 (in the
  topic list above): every unit-seed and item-seed draw between
  allocation and the return of `0x00558D90`, plus a dump of the finished
  item (quality, file index, affix slots, flags, stat-list entries);
  replaying it through `items::create_item` with the recorded game seed,
  request and tables is the acceptance check for that module, and through
  `wiring::economy::Economy::create_item` (real allocator + real lists),
  which also settles WE2 and WE3 against the recorded list layout and
  unit flags (`wire-economy.md` §7.1). (7) End-to-end
  RNG draw-order trace once `MissileUnits` / `AiUnits` exist: missile
  damage rolls on the missile seed, to-hit on the owner seed, AI draws on
  the monster seed, against `rng` traces with positions. (8) Treasure
  walk replay: recorded walk inputs (TC, `Q`, `L`, `F`, seed) through
  `treasure::walk` with a recording `DropSink`; compare draw order / count,
  creation calls (id, quality, index, flags) and NoDrop `N` at
  `0x0055A9B9`; then through `walk` with `wiring::economy::ItemDrops`,
  comparing the created units' stat lists (`wire-economy.md` §7.2). (9) Replay the recorded packets of waypoints OQ1–3, cube
  OQ1–4, quests OQ2, OQ4, OQ5, OQ10 against `d2_sim::world` once its seams
  have providers; they decide the `TODO`s of §7 world. Cube recordings
  also through `EconomyCube` once `CubeRest` has providers
  (`wire-economy.md` §7.3), and through `Host` with an `ItemWorld` from the
  live tables (`server-items.md` §6.1: a cube session with a put-in via
  0x2A, the transmute of V1 record 23 and close; compare each handler
  result and the 0x77 bytes, and once cube OQ1 / OQ2 are settled the
  placement, removal and sound messages). (10) Combat checks
  the specs request (`hit.md` OQ1–2, `damage.md` OQ1–2, `levels.md` OQ1):
  replay recorded entry / exit states through `hit_test`,
  `block_or_dodge`, `start_combat`, `apply`, `special` with a
  recording-backed `CombatWorld`. (11) A hosted (multi-client) game
  recording (still open from the Phase 3 recordings). (12) Audio, after
  `audio/triggers.md` and `sound-table.md` exist (blocked, see below):
  record the original's sound calls twice on a static scene (stability),
  then compare with `audio::log::compare_logs`. (13) Skill use and vitals
  (hooks already named in the topic list: `0x0056FAF0`, `0x0056F7F0`,
  `0x0056BFE0`, Strafe / Zeal frame codes for `use.md` OQ3–5, OQ7;
  `0x00570880`, `0x00570D60` for `vitals.md` OQ1): replay through
  `skills::use_::start` / `do_core` and `combat::vitals::level_up` /
  `spend` with a recording-backed fake. Expect equal stats and the same
  draw count (one `roll(100)` per interruptible request with state 42,
  none otherwise); also through `ActionSim::skill_use` (a missile skill's
  do) and `VitalsView` (a level-up, a stat spend), expect equal stats, the
  same draw count on the caster seed and the same type-12 timer frames
  (`wire-interaction.md` §8.2). (14) Monster population and init: one new Normal game
  with the RNG hook at the call sites listed in `population.md` /
  `init.md`, the first population of a Blood Moor room: room-seed and
  game-seed draws (call site, `lo'`) replayed through `populate_room`
  with a `PopWorld` fed the recorded rooms and collision; expect the
  identical draw sequence and units (`population.md` OQ2, `init.md` OQ4
  Randomness order). (15) Maze levels: with `record_rng.py`, enter Den of
  Evil (8) and Cave Level 1 (9); with the preset provider wired,
  `Maze::generate` on the live tables must reproduce every level-seed and
  room-seed draw at the maze sites `0x00670C70`–`0x00673FE0` and the
  preset draws after them, in order: 0 mismatches (`maze.md` OQ1; the same
  recording as the DRLG entry of the topic list). (16) NPC, vendor and
  hire services: hire, resurrect, Akara heal, Cain identify, repair, a
  magic-item price and a gamble open / purchase (`npc.md` OQ6,
  `vendors.md` OQ6, OQ7); they decide N1–N8 and V2–V5 of §7. Replays of the same recordings:
  through `Desk` / `VendorDesk` with recording-backed rests (0x13 / 0x2F /
  0x38 / 0x32 / 0x33; compare message bytes with 0x2A bytes 3–6 masked, the
  store item list and its durabilities (the store recording also decides
  the §7 vendors observation: compare the 0x9C item durability), gold
  after each transaction, and the NPC-control seed state, store draws then
  hire-list draws in one open; `wire-interaction.md` §8.1), and through
  `Host` with the world handlers once the NPC / vendor / quest providers
  exist (`server-world.md` §7: S→C bytes, 0x2A bytes 3–6 masked, order per
  frame).
- *Existing recordings, no player, only code to write.* (17) 0xAC assign
  decode: `write_boss_section` / `component_bits` against
  `traces/raw/20261006-015956-packets.jsonl` and `-022633-packets.jsonl`
  (`init.md` "Recorded checks" table); expect equal bytes.
  (18) Group sizes and creation order of `20261006-015554`, `-021854`,
  `-022304` (`population.md` Test vectors "Recordings"), once
  `MonsterInit` and the DRLG providers exist (also listed under Blocked).
  (19) (needs the player) Skill and combat handlers (`server-skills.md` §6.1; same recording
  as (13)): with `record_packets.py` and a stat hook, cast a right skill
  at a point, on a monster, with no mana, and spend stat and skill points.
  Expect per message the same result code (the debugger return of the
  handler), `pierce_idx` (stat 328) +1 per skill message (failed uses too;
  not for 0x0B or a refusal before the handler), the 0x5A bytes after the
  no-mana cast (fills `use.md` OQ9) and the vitals deltas of the spec's
  vectors; replay through `Host::frame` with a recording-backed
  `SkillSeams`. After §7 SK1 is wired: a recording's type-0 / type-1
  timers after a cast message (`use.md` vector "request at frame 886 →
  type 0 at 892, type 1 at 900") through the same path. (20) Trace
  definitions of `intents-events.md` §6 r1–r5 (`gaps-drlg-world.md`:
  needs traces; no command is named in the note, the owner spec session
  names it).

**Seventh-fold additions to A** (from `conformance-path-render`,
`render-wire`, `render-camera-placement`, `wire-inventory-server`,
`impl-path-core`, `impl-walk`, `wire-path-sim`):

- *S7-A1 new walk recording, entry `movement-walk`* (player, Windows;
  `conformance-path-render` §5 item 3): `py
  tools/trace-recorder/record_packets.py --seconds 120`, then `py
  tools/trace-recorder/check_packets.py traces/raw/<time>-packets.jsonl`
  (expect `OK`). Play a new single-player character in the Rogue Encampment
  only: about 10 single clicks on open ground (walk, 0x01), the same with run
  on (0x03), one click on an NPC walking and one running (0x02, 0x04), hold
  the button 3 s once (R6), one click into a wall or tent; no waypoint, warp,
  skill, item or NPC dialog. Then run C56. Expect until wired: read counts
  with `other_inputs` small, FAIL `MOVER NOT WIRED`; after wiring: PASS with
  messages = positions = the 0x96 count.
- *S7-A2 camera check on a frame capture* (player, Windows; `render-wire`
  check 2): record each case of `capture.md` §8 with `py
  tools/trace-recorder/record_frames.py --seconds 30` (camera-0001: Rogue
  Encampment, walk 5 s, run 5 s, stand; placement-0001: stand, open and close
  the inventory, stand), then right after each `cargo run --release -p
  d2-client -- verify --cases crates/d2-client/capture-cases --case
  camera-0001` (then `placement-0001`, `composition-0001`). Expect: no ERROR;
  on the first frame `camera: recorded view rect, shiftX, tile origin [..]
  and unit origin [..] equal camera.md §1, §3`, then `scene: seam: the
  recording holds the camera only …`; `frames: 0 match, 0 differ or fail, N
  scene not wired`; `SCENE NOT WIRED <id> (scene)`; exit 2. Inventory frames
  of placement-0001 must pass the camera check with view rect `[-200, 0, 600,
  560]`, shift −200 (mode 1). **Any FAIL** with lines `tile origin
  (camera.md §3): recorded …, rule …` (or view rect, shiftX, unit origin) is
  a `camera.md` finding: record the lines, the draw and the frame's `shake`
  and `open_mode`; do not loosen the check. A `camera: … no player record`
  ERROR means the recorder took a frame without the player (record the draw).
  `--perturb N` cannot fail these frames (no pixel is compared at the seam).
  Also the CPU reference from the recorded positions through
  `rules::Camera` and `rules::placement` must equal the captured index frame
  for `camera-0001` and `placement-0001` (`render-camera-placement` "Local
  checks" A): needs the `ViewSource` and the scene cases (RW1).
- *S7-A3 path state and placement calls* (recorder extensions; both BLOCKED
  in `recordings-needed`): entry `movement-path-state` (hook `0x00650840`
  entry and exit: per call frame unit type, GUID, precise x / y 16.16, point
  index, count, points, velocity, path flags; walking, running, clicks into
  walls in town and in a wilderness area; the record format is the spec
  session's to define) and `placement-monsters-items` (`0x0064DEA0` entry /
  exit with room, start, size, mask, result; `0x00554EA0` arguments and
  result; monster creation with room and point; `0x00555DA0` for drops; each
  with frame, unit type, GUID, class and the room's level and tile rect; or
  the 0xAC / 0x9C position bytes in `server-messages.tsv`). When they exist:
  replay allocations and footprint moves and compare the room grids cell by
  cell (`impl-path-core`), and the per-tick positions (`impl-walk`: the
  settle step of `pathing.md` OQ1).
- *S7-A4 after the recorder gap closes* (`render-wire` check 4; capture.md §3
  change + recorder + the frame-cycle base seam RW3): the same `verify`
  commands as S7-A2 → `PASS`, and `--perturb 5` → every frame `CPU: 5 of
  480000 bytes differ`, FAIL, exit 1.
- *S7-A5 inventory recordings R1–R6* (`inventory.md` Test vectors; also the
  sixth fold's additions): replay their C→S messages through `Host` on the
  wired host (`cargo test -p d2-server --lib items::moves` shape, a
  recording-backed `MoveRest`) and compare result codes and the S→C bytes
  per frame (0x9C / 0x9D bit streams once OQ1 is written). R1 / R3 settle
  WN1–WN3, IS1 and MV1.
- *S8-A1 two clients in one walk recording* (`wire-path-server` §5,
  `path-update-pass` §5; extends S7-A1): the per-tick recording of
  `pathing.md` OQ1 should include **two clients in one game**. Replay its
  walks through `d2-server`'s `walk/tests.rs` shape (same start, target and
  collision) and compare the per-tick precise positions and the S→C bytes
  the other client receives. It settles WS1 (R3's next-tick 0x15 after a
  waypoint warp), WS2 (which flags `0x00553220` clears) and PU1 (the second
  client gets 0x0F once per walk request, not once per tick; whether 0x15
  with flag 0 reaches the other client after a 0x800 event).
- *S8-A2 dropped-item pick-up and the vendor replay* (`unify-items` §5; extends
  S7-A5): when R1–R6 exist, replay a pick-up of a **dropped** item (the W-5
  path, 0x16 on the kill's item) through `Host` + `WiredWorld` and compare the
  result code and the S→C bytes; the vendor replay (`e2e-vendor-host` §8) now
  runs with the inventory model, so its §7.2 rule 9 removal and §7.1 rule 9.7
  placement are compared too.
- *S8-A3 drop position* (`drop-freespot` §5; after C38): a monster kill with a
  recorded item position (R1–R3 style) through the wired host with the path
  provider on and the real field loaded (`PathState::field` from `ExpField.D2`,
  which no host loads yet): compare the item's (x, y) with the recording.
- *S8-A4 stat-change traces* (`mutants-core` "Next"): `record_stats.py` (§5 A
  Stats entry) would let the conformance replay reach the stat code, which
  killed **0** of the 149 `d2-sim` mutant survivors today.

### B. Ghidra / spec edits only (no game run, no player)

**From the 2026-10-06 captures:** the cursor
draw call and its animation counter (owner `render/capture.md` /
`ui/controls.md`); the weather (rain) particle RNG (owner
`render/draw-order.md` or a weather spec).

For the next local spec session. Apply each fix on the owning spec with
evidence.

Spec question for the next local spec session (from the `d2-server`
implementation, 2026-10-06): `tick.md` §1.2 takes `now & 0x7FFFFFFF`
and a signed `now − last`; read literally, across the 31-bit wrap of
`timeGetTime` (~24.8 days of uptime) the difference is negative and no
tick runs until `now` catches up with `last`. Confirm in `0x0052FC20`'s
disassembly (`tools/ghidra/disasm.py`) whether `last` is masked too and
whether the compare is signed; record it as an edge case in `tick.md`.
`d2-server` reproduces the literal reading (test
`driver_first_use_and_mask`).

**Cross-spec fixes reported by writers** (apply on the owning spec with
evidence): `rng.md` §5.3 (item seed re-init after a failed quality
routine; four missile seed sites are missile init callbacks), §7 (item
base stats and low-quality durability use the item's unit seed; monster
population draws mostly from the active room seed, only density/sparse
rolls from the game seed); `calc-expressions.md` §3.5 (`skill(s,c)`
without an entry returns the level-0 special value; OQ5/OQ8 answered by
`skills/levels.md`); `server-messages.tsv` (0x63, 0x28, 0x5D, 0x50, 0x91
per `world/quests.md`; 0x77 is sent in single player; 0x2A per
`world/npc.md` §9, bytes 3–6 never written: mask them; 0xAC per `monsters/init.md` §24); `tick.md` §4
(`0x0052D0F0`), §5.6 (thinks dropped by the freeze gate are never
rescheduled); `units.md` (pointers from the skills and monsters specs);
`combat/vitals.md`: merge the vitals helper's findings
(`re/exports/requests/skills/vitals-findings.md`, local only: regeneration
details to reconcile with `stat-lists.md` §10.1, experience on kill,
monster vitals at spawn to `monsters/init.md`);
`client-messages.tsv` 0x4C (body-part transmogrify, not the cube; the
cube transmute is 0x4F button 0x18, `world/cube.md`), 0x3A (byte +1 stat id ≤ 15, byte +2 count − 1 ≤ 99,
not `stat:u16@1`, `combat/vitals.md`); `rng.md` §7 DRLG rows (level seed
for `0x0066F690`/`0x0066F990`, room-seed sites `0x0066D820`, `0x00670170`,
`0x006706D7`, maze draws `0x006711A0`–`0x00673EC9`, `drlg/rooms.md`);
`unit-order.md` §9/OQ2 → `drlg/rooms.md` §6, OQ3 → units/monsters;
`tick.md` OQ4 → `drlg/rooms.md` §7; `ds1.md` OQ1/OQ4 closed (bytes never
read), OQ3 (truncated `trees.ds1` group: keep 0). Carried-over question 1
below: the server tile build does not hide those tiles; the cause is in
the client draw path.

**New from the notes:**

- `properties.md` G1 (OQ3): owner vs item list in §4.2.
- `rooms.md` §9.6: transcribe the wall-remap table `0x006EF578` (values;
   today `DrlgData::wall_remap` is an input and a linked wall over a
   normal wall errors `MissingWallRemap`); merge case analysis (does
   "keep" keep R's or the new type; R's type outside 1..7; door edges).
- `rooms.md` §9.5 "Warp tiles": how a cell finds the room's warp entry
   for its destination (LitVersion records: 4 floor choices / 1 wall
   choice draws); `rooms.md` OQ10 door tables (transcribe; objects 91–92
   `roll(3)` on the room seed); §9.2 the keep argument of `0x0066F1A0`
   from `0x0066B4C0`; §9.6 link list order; §9.7 animation frame flags;
   `levels.md` §10.2 spawn-tile class rule.
- Level-type specs `drlg/preset.md`, `maze.md`, `outdoor.md` are not
   written (unblock real levels; §5 C).
- `server-messages.tsv`: S→C 0x15 (ReassignPlayer, 11 bytes) layout from
   the sender `0x0053BC10` (so the point parser's resync §2.4 rule 3 can
   be built); a negative C→S chat size and S→C buffer split past the
   buffer end (§7 questions 6–7): read the classifier `0x0052B100`.
- Spec wording to fix (no Ghidra needed): `damage.md` Test vectors
   `pct(0x200000, 50, 0x30000)` = 34 vs §0's rule giving 533 (the code
   follows §0); `quality.md` §8.1 vector "idx 4097 … unique fails (unless
   `nolimit`)" vs §8 step 6 (accept test before the marking); `treasure.md`
   header "Crate/module" (`d2-data::treasure` / `d2-sim::items::treasure`
   vs code `d2_sim::treasure`); `tick.md` "Test vectors, Trace sim/tick"
   section (cited by `convert_tick.py`, missing: TR4); `unit-order.md` §3.1
   missile example (TR1).
- Parser limits and edge cases to confirm on 1.14d: PKWARE explode input
   ending inside the end marker's extra length bits (R1 below); the
   DC6 / DCC / DT1 / TBL whole-file limits of the parser-robustness entry.
- `stat-lists.md` §10.1 vs `units.md` §6.1 (stamina and mana gating on
   `0x00580610`): decide from the disassembly; `units.md` §3.1: allocation
   without `SUNIT_Add`; `stat-lists.md` §8.8 the stay-on-death rule
   `0x0063A4A0`; §10.4 expired extended list (an endless loop in 1.14d?).
   The remaining §7 questions are spec-session items in the same way;
   each lists its owner there.


**From the second set of notes (2026-10-06):**

- *Wiring open questions* (§7 W1–W16, WE1–WE9): the owner specs decide
  them; disassembly needed for WE1 `0x006257D0` (list by state and flags,
  `stat-lists.md` §9.3), WE6 `0x005C1BC0` (`affixes.md` §9 and `cube.md`
  §7.3 disagree), WE7 `0x00660240` (expansion argument, `properties.md`
  §12), WE8 `0x00558110` (which items `has_item` searches), WE9
  `0x00559CE0` / `0x00558200` (reward creation and level default), W2 / W3
  (`missiles.md` §R6.1–§R6.2), W5 `0x0064D9B0` / `0x0064EBA0` (size
  footprint), W6 `0x00641CB0` (unit search order), W13 `0x00543B90` /
  `0x00537340` (client level change), W14 `0x00619E50` vs `0x0066B2B0`.
  Helpers no items spec writes (economy and vendors notes list the same):
  `0x0055A2A0` duplicate, `0x00558C50` drop runeword stats, `0x0055F900`
  repair, `0x0055FE80` recharge; item events `0x005C0BE0`, `0x0056E740`,
  `0x005C0B50` (`stat-lists.md` §7.2 rule 1); the replenish event 3 body
  (`units.md` §6.5, WE4).
- *Skill use and vitals*: `use.md` OQ1, OQ2 and the §4 items of
  `impl-skilluse-vitals` that need the disassembly (§1 rule 2 result codes,
  §2 order of the `use_state` parts, §5.3, §5.4, §7: items 1, 3, 7, 8, 9);
  `vitals.md` OQ2 (kill gain: `ExpRatio` stat 85, hireling, party share),
  OQ3 (operand roles of the `pct` branch in §4.2); merge with the vitals
  helper findings above.
- *Monster init and population*: `init.md` OQ6 (boss mods other than
  bloodraven), OQ7 (umods 17, 18, 23, 25 constants; umod 26 body), OQ8
  (callbacks other than the five bodies of §22), §24 (how an all-zero
  component field is signalled; header and field order around the
  init-owned fields); `population.md` OQ1 (`0x0052D0F0`), OQ2 (draw
  recording), OQ5, OQ6, §6.3 (boss creation mode), §6.3 r4 / §11.4 r6
  (`0x0058F030` arguments, hcIdx 60), §11.4 r6 hcIdx 10 (Radament seed and
  mode), §11.5 (seed of the event-7 `roll(50)`; ids absent from the TSV).
- *Level types*: `outdoor.md` §9.1 / OQ 7 (Act III jungle attach points and
  per-level ids), §9.3 / OQ 8 (Kurast, Travincal), §11 / OQ 9 (Act V
  beyond the siege strip), §7.5.3 / OQ 6 (Act I path floor), OQ 1 of
  `maze.md` (needs the recording, §5 A15); `maze.md` §5.5, §2.4, §3.7,
  §7.1, §9 step 4; `preset.md` §6 step 10 (`0x0066B970` list position),
  step 9, §11 (door flag when `roll(3)` is 0); the `LevelTypes::door_unit`
  signature (orientation).
- *NPC and vendors*: `npc.md` OQ1, OQ2 (`start_allowed`, `npc_ai_param`),
  OQ4, OQ5, OQ6; `server-messages.tsv` 0x27 (`partial`: the
  `0x00661480` text-list encoding is not specified); 0x2A bytes 3–6 and
  0x58 byte 6 are never written (mask them); `vendors.md` OQ1–OQ7
  (OQ5: fatal paths surface as `PriceFatal`, a `Ruleset` decision).
- *Phase 6*: `render-pipeline.md` §A9 wording: "one workgroup per bin"
  could read "per-bin lists, workgroup size free" (the compositor uses
  16×16 workgroups because 1,024 invocations exceed WebGPU's default 256).
- *Spec vs test findings of `coverage-claims`*: `waypoints.md` §7 r2 /
  edge case 6 (picking the waypoint's own level should stop after the
  interact reset; the test `close_and_validation` expects `warp 1 13`, so
  no claim); `cube.md` §6.4 vs §7.1 r4 (`v11_item_level_stored_back`
  passes without the store-back; a vector with non-zero ilvl and plvl
  would separate them); `rooms.md` edge case 4 / OQ 4 (`client_arrays_sorted`
  checks a placeholder sort by client id); `tick.md` §5.2 r3 (one recorded
  `timer_set` asks for a past frame, so no claim); `unit-order.md` §3.2
  (the replay applies the recorded cancels, not the free routine's own).


**From the fourth fold (2026-10-06; spec questions, each also in §7 by
owner spec):**

- *Spec edits, no Ghidra* (`gaps-data-rng.md` §5–§6, `gaps-client-formats.md`
  §7, `gaps-combat-items-monsters.md` §6): `loading.md` §7.3 lists
  `chartemplate` and `setitems` under "all others — none" although
  `chartemplate.class` links `playerclass.code` and `setitems.set` links
  `sets.index` (extends the `loading.md` §7.3 item of GD1–GD7; both targets load earlier); `patch-layers.md`
  §8 "B (table)" order and §11's list of allowed order-dependent failures
  (add A05, or give the row-label check after an index-selector rename a
  rule; GD8, GD9); `sim/rng.md` §5.2–§5.4 and §7: number the rows as items
  so the system tests can claim them one by one, and move §6 (it restates
  the owner specs) out of the rules sections (GD11); `bridge.md` §6 r2
  ("Today every row is `TBD`") moves to Status, or the test pinning it
  changes with the first owner spec (GB1); `treasure.md` §1.2 whether a TC
  row can name itself (GT1); `population.md` §1 r2 whether `0x0052D0F0`
  includes the §1 r1 ambient spawns (GP1, ties to `population.md` OQ1).
- *Ghidra / memory*: none new. `treasure.md` §1.5 counts and `population.md`
  §1 r3 fill order wait for a spec session to state their comparison.

**From the third set of notes (2026-10-06):**

- *Ghidra reads, handlers and wiring*: SI1 `0x00549350` (failure value of
  the cube's range-10 test; d2rs returns 1); SI2 whether `0x0055DF10` →
  `0x00557FD0` reaches `0x00555600` and does nothing else the sim sees;
  SI3 the argument of `0x0044BE50` (targeting reset) and the S→C 0x3F
  layout (also `server-messages.tsv`); WG6 which DT1 header field
  `0x00604BC0` reads; WI I3 which of `0x00570360` / `0x00570C80` resets
  stats (`npc.md` §8.2 and `vitals.md` §2.1 disagree); WI I5 the
  allocation flags of `0x00559CE0` for store items; WI I6 the mode set the
  vendor code calls; WI I8 the record fill of `0x0056ECB0` / `0x0056EE90`
  (`use.md` §5.4 step 7); WG5 the order of the hcIdx extra spawns against
  the aura re-run and quest records (`population.md` §11.4 step 6,
  `init.md` §20 steps 4–5); WG8 which creation step starts a monster's mode
  and first think; the DRLG data population reads (coordinate lists
  `0x0061AD50` / `0x0061AD30` / `0x0061B130`, `0x0061A1F0`, `0x0061ABF0`,
  `0x0061AC10`, level spawn of kind 11, `0x0064E840`; they belong in a
  DRLG or rooms spec, §1 3l), `0x0066E1C0` and `0x00666610`; `units.md §5
  r4` (`0x005416B0` with expire −1 loses its callback) against `tick.md`
  §5.2. The bytes of cube placement, removal and sound messages (`cube.md`
  OQ1 / OQ2, SI4) need the V1 / V22 recordings of group A.
- *Spec edits, no Ghidra*: `use.md` §1 rule 2 should point to
  `intents-events.md` §2.4 rule 4 (codes 2 and 1, `server-skills.md` §5
  item 3); `stat-lists.md` §6.4 text and edge case 2 against rule 5
  (§7 GI); `affixes.md` §5 line ~152 rewrapped so no line starts with `0.`;
  `intents-events.md` §4 r4 prose (0xAF, 0xB0, 0xB2–0xB4 against the TSV's
  0xB1 `none`); `outdoor.md` §6 r4 order (double, then grow) and §8.3 rows
  (file F and in-row order of 377 / 378 / 380 / 381); `dc6.md`, `dt1.md`,
  `animdata.md` edge-case sections: split the 1.14d data claims from the
  behavior so unit tests can claim the behavior; `mpq.md` §11 two gaps
  (input ending between codes without a 0x100 code; increment when `lead`
  is the root or `n`'s parent); `calc-expressions.md` §4.5 (the 8-bit push
  test `len(out) < 1024` allows 1025 bytes) and policy r4 (a
  whitespace-only cell also reports Fail); `loading.md` §7.3 (omits
  `chartemplate`) and policy r5 (`d2exp.mpq` required, not checked up
  front); `txt-format.md` edge cases ("monstats 255 slots" without
  splitting columns from missing fields); `levels.md` edge r3 / §5.4 (a
  10th warp-room centre slot). Each is also in §7 by owner spec.


**Sixth fold (2026-10-06):** no Ghidra read and no spec edit is new beyond
the lists of §2 steps 12–13 (missing body text for AI, server-do, server-hit,
quests and skill functions; the partial S→C rows; `inventory.md`,
`path-placement.md`, `pathing.md` and the render specs' open questions) and
the questions of §7's sixth set. Spec wording to fix without Ghidra:
`tick.md` §5.5 consequence 1 / edge case 4 (PS1), `stat-lists.md` OQ5 (PS2),
`intents-events.md` §6 (masked bytes, SC1), `bridge.md` §4 r3 (FS4),
`vitals.md` "level 99" (GS1), `population.md` evilhut row (GM1),
`quests.md` / `npc.md` respec addresses (WO1), `Pl2::alpha_blend` doc (RN3).

### C. Game files and GPU only (no player, no Ghidra)

Live-table checks for the new modules need a **home**: `d2-sim` has no
`d2-formats` dependency and the notes disagree on where (§7 Integration).
Until decided, run them as ignored tests in `crates/conformance` (or add
the dev-dependency) and record results here.

2. Stats on the live set: build `StatData` from `d2_data::fixup::apply`
   and assert the real-data rows of `stats.md` Test vectors: entries(7),
   deps(12) = 214…250, the A53 set on 214 / 215 / 218 / 219. Also `py tools/trace-recorder/check_stats.py
   --files <GAME_DIR>` (the note `gaps-items-stats.md` names it for `stats.md`
   §2 r2, which stats have ValShift 8; arguments as `check_stats.py --help`
   gives them): expect the stats the spec lists; a different result is a
   finding for the spec.
3. Items on the live set: `ItemTables::from_fixed` counts: items = weapons
   + armor + misc; magic = 747 + 669 + 36 rows; rare suffix count; 8
   qualityitems; skill lists 30 per class.
4. Treasure on the live set: build `TreasureClasses` (typed records of
   treasureclassex, itemtypes, weapons, armor, misc, uniqueitems,
   setitems; `equiv_matrix(itemtypes, ItemTypes)`). Expect `treasure.md`
   "Real 1.14d" vectors: 1,013 TCs, 160 automatic, 2,742 TC / 660 item /
   2 unique / 0 set entries, 0 notes, 81 `mul`; `Act 1 H2H A` = TC 430
   (group 12, picks 1, nodrop 100, entries 523 / 218 / 203 / 370 with
   starts 0 / 21 / 37 / 58, totals 60 / 60); `ROP (N)`; `Act 1 Champ A`
   picks −2; `get(430, 40)` = 445, `get(430, 0)` = 430, `get(430, 85)` =
   471, `get(0, 40)` = none; chest table all 45 found, `Act 1 Chest A` =
   385; chest tiers (normal act 0: `lo` 1, `hi` 12, `s` 4; Hell act 4:
   `lo` 0, `hi` 83, `s` 28); TC count = `d2_data::bin::tc_count`.
5. Waypoints: `WaypointMap::new(&levels)` from the loaded `levels` table;
   `rows()` equal `specs/world/waypoints.tsv` (all 39 rows);
   `WaypointData::new(..).waypoint_classes()` = the 16 classes of
   `waypoints.md` §5 rule 1. `gaps-drlg-world.md` also lists `waypoints.md`
   §3 r3 as needing game files (no test written; claim withheld).
6. Cube: `cube::recipes(cubemain)` on the 1.14d set: 151 records, enabled
   146, ladder 21, version 100 on 96, op 28 on records 0, 1, 2, 148, 149,
   output kinds 0xFC 78 / 0xFF 48 / 0xFE 21 / 0xFD 1 / portals 1 + 1 + 1,
   133 mods all chance 0; then V1–V23 with a real items table.
7. Assets (`claude/p6-assets`): add `crates/d2-client/tests/assets_game.rs`
   (`#[ignore]`, reads `D2_GAME_DIR`) and run `cargo test -p d2-client
   --test assets_game -- --ignored`. **The test code exists since the sixth
   fold as `crates/d2-client/tests/game_assets.rs`: run C46 instead.**
   Expect 0 refused names (every
   listfile name of the 1.14d archives canonicalizes and reads back with
   identical bytes); every `.pl2`, `.cof`, `.tbl` parses; every
   `data\local\font\**` `.tbl` → `Font`, every `data\local\lng\**` `.tbl`
   → `Strings`. A refused non-ASCII name means the strict check needs a
   rule (`assets.md` OQ2).
10. Skill use and vitals on the live set (test not written; needs a
    `D2_GAME_DIR` loader for `charstats` / `experience`, see I1): build
    `VitalsTables` and `SkillTables` from the live `.bin` set and rerun the
    vitals vectors (Sorceress / Barbarian created and level 1 → 10,
    `threshold(0, 1)` = 500, `level_from_exp(0, 499 / 500)` = 1 / 2) and
    the `use.md` mana vectors (Fire Bolt 640, Fire Ball L10 2,432, Teleport
    6,144 / 1,280 / 0 / −1,280, Multiple Shot L10 3,328). Expect the
    numbers in the specs' Test vectors.
11. Monster population on the live set (test not written): an `#[ignore]`
    test reading `D2_GAME_DIR` that builds `PopTables` from the live tables
    (`PopTables::from_records(..).with_bins(..)`); command `D2_GAME_DIR=…
    cargo test -p d2-sim population -- --ignored`. Expect every "Real" row
    of `population.md` Test vectors equal (Blood Moor / Cold Plains / Den
    of Evil rows, Rarity, groups, parties, superuniques 0–9, placespawn
    and sparsePopulate rows).
12. Maze data on the live set (test not written): `MazeData::from_tables`
    on live `lvlmaze.bin` / `lvlprest.bin`. Expect 81 lvlmaze records;
    every `levels.txt` DrlgType 1 level has a record; every def `shape_def`
    can return for each maze level type and mask 1..15 (non-zero), every
    `maze-specials.tsv` special def and the fixed defs of §4–§7 (167,
    288–290, 333, 336, 444–447, 480, 735–738, 836, 852–856, 1038–1041,
    1074–1077) exist in lvlprest with `Files` ≥ 1.
13. DS1 survey through the preset parser rules (needs a crate with both
    `d2-formats` and `d2-sim`, e.g. an ignored test in `conformance`; test
    to write): `cargo test -p conformance -- --ignored preset_ds1_survey`.
    Convert every DS1 named by lvlprest (2,043 files, d2data + d2exp) to
    `Ds1Input` and run `Ds1File::from_input` with the live `PresetData`.
    Expect no error; monster records 2,267 and object records 14,105 before
    conversion; one record with flags ≠ 0 (value 1); kept class ids
    580 / 581 / 582 = 46 / 135 / 24 records; every DS1 size equals its row's
    `SizeX` / `SizeY` (1,054 rows) and every DrlgType 2 level's size. Also
    `TownN1.ds1` at origin (X, Y) → tile info (X+26, Y+7, 0), (X+28, Y+7,
    10), (X+30, Y+14, 11); `Act2/Town/LutN.ds1` (Def 301) → pops (8, 8,
    group 1, (24, 3, 7×7)), (13, 13, 2, (10, 27, 4×5)), (12, 13, 2, (15, 33,
    3×3)) and 5 tile-info entries; `MetroTemple2.ds1` → style 8 rect (2, 2,
    5×10), style 9 rect (0, 0, 12×7), both group 1.
14. Vendor column lists on the live set (test home needed: `data-tool` or
    `crates/conformance`, I1): `GlobalLists::build(&VendorTables::
    from_fixed(&fixed))`. Expect Charsi's column 2 to hold exactly the
    rows with `Charsi*` values (`aqv`, `cqv` permanent; `axe` Min 1 Max 1
    MagicMin 1 MagicMax 1 MagicLvl 1); `npc` rows equal `vendors.md` §9.3;
    `difficultylevels` odds 10000 / 100 / 50 / 90 / 33.
18. Combat / AI / missile gap rules on live data (`gaps-combat-ai.md`; no
    test written, home per §7 I1): `monsters/ai.md` §9.1 (values from the
    live `monstats.txt`), `missiles/missiles.md` R1 r1 (684 records × 420
    bytes from `missiles.bin`) and R10 (the recorded row values; r1–r4 are
    tested on synthetic rows). Expect the spec's numbers.
19. `world/quests.md` §7.1 (`gaps-drlg-world.md`; no test written): every
    NPC id of the quest tables is a valid `monstats` row. Expect all
    found; the unit test covers lookup and table shape only.
20. Treasure dump vs d2rs (test to write, local: reads `traces/raw/
    <time>-tables`, which is not committed): build `TreasureClasses` from the
    live `.bin` set and compare every TC record and entry byte for byte
    with `map-tc_records.bin` / `map-tc_entries.bin` (`tc_chest` as record
    indices via `manifest.json` `tc_base`). Expect identical.
21. `d2-formats` DS1 game claim (`gaps-client-formats.md` §5; claim on
    `ds1.md` edge cases, the last unclaimed unit of `ds1.md`):
    `D2_GAME_DIR=<install> cargo test -p d2-formats --test formats_game
    -- --ignored ds1_layer_limits_and_truncated_trees_groups`. Expect a
    pass: `data\global\tiles\ACT1\OUTDOORS\trees.ds1` (from `d2exp.mpq`,
    else `d2data.mpq`) reads version 12, tag_type 1, 14 groups,
    `groups_truncated`, the 14th group's y / width / height 0, and its
    `files` list holds `C:\D2\DATA\GLOBAL\TILES\ACT1\TOWN\trees.tg1`
    (ASCII case ignored). The synthetic half (4 walls, 2 floors, 1 shadow
    layer) already passes in CI. On failure fix the expected value from the
    file or remove the claim in the same session (`COVERAGE.md` §3).
22. `automap.bin` names (`gaps-data-rng.md` §7, deviation D5 / §7 GD10):
    `cargo run -p data-tool -- tables`, then read the 1.14d `automap.bin`
    for any record whose LevelName or TileName is empty or starts with
    `0`. Expect none (the stricter `bin.rs` `post_load_check`, which treats
    an empty name as known and has no `0` case, is then harmless); any such
    record shows which of the spec (`0…` gives 0 without a compare, empty
    fails) and the `.bin` check is right.

23. Level data from the user's files (`drlg-data` §6; test code written,
    never run): `D2_GAME_DIR=<install> cargo test -p d2-server world_data
    -- --ignored`. Expect 3 pass. `act1_placement_matches_the_recorded_vector`:
    live tables and files load without error; act 0 created with init seed
    644409375 and the town (level 1) generated: `dwStartSeed` 4014346869,
    DRLG seed {1406222081, 1674353446}, allocation order 4, 3, 2, 1, 17,
    39, 26, 7, 6, 27, 5 (recorded); rects Stony Field (1000, 1000, 80, 80),
    Cold Plains (920, 984, 80, 80), Blood Moor (904, 1064, 56, 96), Rogue
    Encampment (960, 1112, 56, 40), Burial Grounds (880, 968, 40, 48) and
    origins Moo Moo Farm (5000, 1148), Gate (3000, 1000), Tamoe (3000,
    1018), Black Marsh (2920, 1002), Dark Wood (2904, 1082) (these rects are
    `outdoor.md`'s "derived from the rules, not yet recorded": a mismatch
    there is a question for the spec as much as for the code).
    `den_of_evil_matches_the_maze_vector`: Den of Evil (8) maps head first
    def 57, 86, 96 at (+24, 0), (+24, +24), (0, 0) from the level origin,
    every room a preset room. `outdoor_levels_generate_through_the_dispatcher`
    (WG9): Blood Moor 81 rooms (33 outdoor), rect (904, 1064, 56, 96); Cold
    Plains 98 rooms (37 outdoor); streaming Blood Moor's first room loads
    its DT1 library without error. A failure names the first wrong value; a
    load error names the archive path. If it passes, `wire-worldgen.md` §7
    check 1 (the RNG trace compare, §5 Blocked) is the next step. Loads
    2,043 DS1s up front.
24. Window on a real GPU, synthetic tables (`p6-window` check 1):
    `cargo run -p d2-client --release -- play --synthetic --frames 1500`.
    Expect a black 800×600 view in the window; a log line every 250 frames
    with `server ticks` ≈ 25 per second, `gpu: true`, `node frames` = frames
    − 1 (or − 2), no error; exit 0. Record the adapter line
    (`bevy_render::renderer: AdapterInfo { … }`).
25. Window with the user's tables (`p6-window` check 2):
    `D2_GAME_DIR=<game> cargo run -p d2-client --release -- play --frames 1500`.
    Expect the line `play: game tables from D2_GAME_DIR (<n> levels, <m>
    objects, waypoint object class <k>)`, then as C24. Record n, m, k.
26. `D2_GAME_DIR=<game> cargo test -p d2-client --test app_single_player --
    --ignored`. Expect `live_tables_give_a_waypoint_object` passes (the
    waypoint object is the first `objects` row with operate function 23 and
    init function 17).
27. GPU node byte check on the real GPU: `cargo test -p d2-client --test
    app_frame_loop -- --nocapture`. Expect `adapter: <name> (<backend>,
    <driver>)` (not llvmpipe) and both tests `ok` (0 differing pixels).
28. The `map` case on the compute compositor (`verify-map` check 1; real
    GPU, `D2_GAME_DIR` set): `cargo run --release -p d2-client -- verify
    --case map`. Expect `verify data\global\tiles\ACT1\TOWN\townN1.ds1: <K>
    draw items, view <W>x<H> at <L>,<T>` with the same K, W×H, L,T as the
    last recorded Phase 1b run; `images in game/renders/verify-townN1`;
    `PASS: GPU render matches the CPU reference exactly`; `<C> chunks of at
    most 1024x1024`, `CPU binned: 0 of <W·H> bytes differ`, `GPU indices: 0 of
    <W·H> bytes differ`, `GPU: 0 of <W·H> pixels differ`; `PASS map (map)`;
    exit 0. If only the `GPU:` (RGBA) line is non-zero and `GPU indices` is
    0, record `palette[0]` of the act palette: that is VM1 (index 0,
    `render/composition.md` §B2), not a compositor bug.
29. `cargo run --release -p d2-client -- verify --case map --perturb 7`.
    Expect `CPU binned: 7 of …`, `GPU indices: 7 of …`, `GPU: 7 of …`,
    `FAIL map (map): CPU and GPU halves`, exit 1. Any other count fails the
    check (M08).
30. `cargo run --release -p d2-client -- verify` (all 11 cases). Expect as
    the `p6-verify-gpu` entry with the map lines of C28: `summary: 11 pass,
    0 fail, 0 error, 0 GPU not wired, 0 no adapter`, exit 0. Record whether
    two headless devices in one process (the runner's and the `map` case's)
    cause any problem.
31. `cargo test -p d2-client --lib verify::map::tests::gpu_map -- --ignored
    --nocapture`. Expect first line `adapter: <real GPU name> (<backend>, …)`,
    test passes. Record adapter name, backend and driver with each of
    C28–C31; until then the `map` case's GPU half is proven on llvmpipe only.
32. `D2_GAME_DIR=<install> cargo test -p d2-data --test game_data -- --ignored
    fixups_on_live_set` (`fuzz-data`): must still pass (the live `itemtypes` /
    `montype` walks stay far under the new n² × 128 budget, so
    `fixup::apply` returns `Ok`).
33. `cargo run --release -p data-tool -- dump-compare
    traces/raw/20261006-021210-tables`: the `itemtypes_equiv` and
    `montype_equiv` maps still identical to 1.14d memory (expected
    unchanged: 70/70 tables and every map identical).
34. World and DRLG table tests (`game-tests-drlg-world` §3; never run, every
    expected value from a spec): `D2_GAME_DIR=<install> cargo test -p d2-sim
    --test game_world -- --ignored` (expect 10 passed) and `… --test
    game_drlg_tables -- --ignored` (expect 8 passed, 0 failed). If one
    fails (the message names the record, level or class):
    `waypoint_map_matches_tsv` a diff between `rows()` and `waypoints.tsv`
    means the TSV or the `levels` `Waypoint` / `Act` reading is wrong;
    `cubemain_live_facts`: the mod count assumes an empty `mod` link reads
    −1 (`field-types.md` link32), check that first if only 133 fails;
    `every_level_drlg_type_dispatches`: a level with DrlgType ∉ {1, 2, 3}
    besides 0 is a finding for `levels.md` §4.4, not a test bug;
    `act1_placement_on_live_tables`: a `types.errors` entry or a `create`
    error means allocation or placement reached for a DS1 or `lvlsub` file
    (the test supplies none), the recorded list and seeds are the claims and
    the rects are derived (a failure there alone points at `outdoor.md` or
    the placer); `recorded_stores_fit_live_columns`: an "absent but yields
    ≥ 1" message means an eligible column entry is missing from the
    recording, check the item level filter (`level` ≤ 6) and the column
    first. Tests: world 10 (waypoints 3, quests 1, cube 2, npc 1, vendors 3),
    drlg 8 (leveldefs 2, lvlprest 2, lvlmaze 1, lvlsub 1, Act I placement 1,
    preset file choice 1). Claims to add after a pass: `waypoints.md` §1 r1,
    §1 r2, §1 r4, §7 r4 (`waypoint_map_matches_tsv`), §5 r1
    (`waypoint_objects`); `npc.md` §1.1 r4, r5; `vendors.md` §9.3;
    `preset.md` §2 r2 (`lvlprest_def_is_the_row_number`); `maze.md` §1 r3;
    `levels.md` §3 r2, §3 r3, §4 r1 and `preset.md` §3.1 r3
    (`act1_placement_on_live_tables`). Then record in §5 Done and §1.
35. Item and treasure tests (`game-tests-items-treasure` §2; these are the
    code for C3 and C4; C20, the byte compare with the dump, is not done
    here): `D2_GAME_DIR=<install> cargo test -p d2-sim --test game_treasure
    -- --ignored` (expect 16 passed) and `… --test game_items -- --ignored`
    (expect 8 passed); add `--release` if the sweeps are slow (~10^5
    creations, ~5·10^4 walks). By name: `live_tc_counts_and_kinds
    live_automatic_tcs live_act1_h2h_a live_rop_n live_act1_champ_a
    live_tcx_slots_5_6_zero live_chest_table live_get_by_level
    live_chest_tiers live_nodrop_pairs live_ratio_rows_are_version_1
    sweep_drop_quality_every_item sweep_tc_structure
    sweep_every_tc_resolves_and_picks sweep_walk_every_tc
    sweep_monster_tcs_resolve` and `live_affix_parts live_qualityitems_count
    live_unique_rarity_32_bits live_type_numbers
    sweep_create_every_item_every_quality sweep_forced_every_unique
    sweep_preferred_every_set_item sweep_every_affix_pick`. Interpretation
    points if one fails: 1,013 is read as the array count including TC 0
    (`treasure.md` OQ4), 660 item entries as the `treasureclassex` TCs only
    (the automatic TCs hold 763 more), 184 / 300 / 240 over the same TCs;
    `live_nodrop_pairs` excludes pairs with a total of 0 (they never reach
    §5.4 step 5): if the counts differ by one, check that scope first. Already
    observed in the memory dump (§5 Done): the TC counts, TC 430, 45 chests,
    `Act 1 Chest A` = 385; **unconfirmed**: 85 / 44 zero totals, 184 / 300 /
    240, the `mul` values, `ROP (N)`, `Act 1 Champ A`, the `get` vectors and
    chest tier area levels, the 23 / 15 NoDrop pairs, Version 1 ratio rows,
    slot 5 / 6 bytes, affix part sizes 747 / 669 / 36, 8 qualityitems,
    `uniqueitems` bytes +0x32, the 24 itemtypes codes, every sweep invariant.
    The sweeps print the first 20 failures with item / TC, quality, level,
    difficulty and mode: each is a spec question unless the d2rs code
    contradicts its spec. After a pass add 13 `Covers:` claims
    (`treasure.md` §1.3 text, §1.4, §1.5 text, §1.5 r4, §1.6, §2, §4 r2,
    §4 r4, §5.4 r5, §6 r2; `affixes.md` §1 r1; `quality.md`
    §edge-cases-original-bugs r4; `generation.md` §3 r1; measured with the
    claims written: game 187 → 200 units, verified 217 → 230, any 2,459 →
    2,460). The sweeps and table-fact tests with a wider rule get none.

36. Inventory tables (`impl-inventory` §6, D1–D3): `D2_GAME_DIR=<game>
    cargo test -p d2-sim --lib real_grid_belt_and_type_tables -- --ignored`.
    Expect a pass: `inventory.bin` 32 records with the §1.3 sizes for 0–15,
    `belts.bin` numboxes `12, 8, 4, 16, 8, 12, 16, 12, 8, 4, 16, 8, 12, 16`,
    and the D3 itemtypes codes (space-padded, `bow `, `axe `, `h2h `). The
    values are the spec's measurements, never run: no claim until it passes
    (`COVERAGE.md` §3).
37. Stats, units, vitals, skill levels, world data and a wired game
    (`game-tests-sim-core` §4; 15 tests, never run, every value from a
    spec): `D2_GAME_DIR=<install> cargo test --release -p d2-sim --test
    game_core -- --ignored --nocapture` (expect 12 passed) and `… -p
    d2-server --test game_world_data -- --ignored --nocapture --test-threads
    1` (expect 3 passed, 0 failed). Look at the printed lines: AnimData
    records scheduled / skipped, monstats rows with `DamageRegen`, the
    evaluated skill-value count, the DS1 version / type / id histograms, the
    Act I room counts, frame and message count after 100 ticks. A non-empty
    `WorldSim::errors()` in `wired_game_on_live_tables_runs_100_ticks` names
    the failing adapter: a wiring finding before a table finding.
    Interpretation points (GS1–GS6, §7): `threshold(c, 99)` is row 100 (a
    panic past the table means the spec's "level 99" is row 99);
    2,043 lvlprest files and the patch MPQ (a count mismatch with every
    file parsing points at `preset.md` OQ5, not the parser; "1,054 rows" is
    read as SizeX and SizeY both ≠ 0, try "either" if only that fails); op 1
    stats 162, 163 each with an entry in entries(11); the fCallback list
    asserts the count, the first six and the last; animation speed is the
    AnimData speed (§4.3 unwritten); the player's stats go through
    `VitalsView` with `NoHost`. After a pass turn each `Intended claim`
    line into `// Covers:`: `fixups.md` §2 r3 (`op_tables_rebuild_from_the_
    columns`), `vitals.md` §4.1 / §1 / §2 / §3 (`charstats_and_experience_
    as_stated`, `player_creation_every_class`, `level_up_and_stat_point_
    vectors`), `stat-lists.md` §7.2 r2 (`monster_damage_regen_every_class`),
    `units.md` §4.2 (`anim_schedule_every_record`), `levels.md` §2, §4, §5
    (`special_value_vectors`), `levels.md` §3 r2, r3, §4 r1, §5
    (`every_act1_level_generates`); the data-fact tests get none; then
    `py tools/coverage.py --check` and record in §5 Done and §1.
38. `path::search::tests::expfield_live` (`impl-path-place` §5): `D2_GAME_DIR=…
    cargo test -p d2-sim --lib path::search::tests::expfield_live --
    --ignored`. Expect a pass: `ExpField.D2` is 65,546 bytes, header (0x010A,
    256, 256), F1 bytes `3 4 5 / 2 8 6 / 1 0 7`, the F2 / F3 walks as the
    spec lists. Written without the file; its `Covers:` (`path-placement.md`
    §7.3 r1, r2, game tier) counts in `coverage.py` already and is verified
    only after this passes.
39. Packets replay baseline (`conformance-harness` §5 item 3): `cargo test -p
    conformance --test packets_replay -- --ignored --nocapture` on the
    existing `traces/raw/*-packets.jsonl`. **Expect FAIL** at the first
    message the original dispatched (the test replays on an empty `SimGame`:
    no session code builds the recorded game, Phase 5): record the `seq` and
    the message id as the baseline. It passes only when a session and a world
    reproduce the recorded game and every S→C sender exists (Blocked).
40. The window with the user's files (`p6-integrate` check 1): `D2_GAME_DIR=
    <game> cargo run -p d2-client --release -- play --frames 1500`. Expect
    `play: game data from D2_GAME_DIR (<n> levels, <m> objects, waypoint
    object class <k>; level files: <a> DS1, <b> lvlsub DS1, <c> DT1)` (record
    n, m, k, a, b, c; drlg-data measured 2,043 lvlprest DS1s), then `single
    player: seed 1234, …`, a black 800×600 window, log lines every 250
    frames with ≈ 25 server ticks per second, `gpu: true`, node frames =
    frames − 1 or − 2, `audio Some(AudioStats { … load_errors: 0,
    engine_errors: 0 })`, no error, exit 0. A DRLG error names the level:
    record it (a live-data finding for the level types, like drlg-data DL1).
    Replaces C25 (the line text changed).
41. `D2_GAME_DIR=<game> cargo test -p d2-client --test app_single_player
    --test app_frame_loop -- --ignored --nocapture` (`p6-integrate` check 2).
    Expect `live_tables_give_a_waypoint_object`,
    `live_data_generates_the_levels_from_the_users_files` (prints rooms and
    rect of act 0 levels 1 and 3 and act 1 level 40; record them; Cold Plains
    would be rect (920, 984, 80, 80) only with the recorded init seed, which
    the app's `--seed` is not, PI1) and `frame_loop_runs_on_the_users_levels`
    (101 frames, 100 ticks) to pass. Includes C26.
42. `verify` on real cases on one device (`p6-integrate` check 3): `cargo run
    --release -p d2-client -- verify`. Expect one `GPU compositor: adapter:
    <real GPU>` line before the first case, the map lines of C28 and
    `summary: 11 pass, 0 fail, 0 error, 0 GPU not wired, 0 no adapter`, exit
    0; with `--perturb 7` every case FAILs with exactly 7 on each comparison,
    exit 1. Supersedes the two-device note of C30.
43. Single-map form (`p6-integrate` check 4): `cargo run --release -p
    d2-client -- verify --ds1 'data\global\tiles\ACT1\TOWN\townN1.ds1'`. Expect
    the map lines of C28 (now with the report lines printed) and exit 0.
44. GPU node on a real GPU, store-backed assets (`p6-integrate` check 5):
    `cargo test -p d2-client --test app_frame_loop -- --nocapture`. Expect
    `adapter: <real GPU>` and all non-ignored tests `ok` (0 differing
    pixels). Same as C27 on the new assets.
45. Monsters, missiles, skills, vitals (`game-tests-monsters-skills` §3; 35
    tests, never run): `D2_GAME_DIR=<install> cargo test --release -p d2-sim
    --test game_monsters -- --ignored` (19 passed) and `… --test game_skills
    -- --ignored` (16 passed, 0 failed; `--release` because the sweeps run
    about 300,000 stats inits and 1.5 M formula evaluations). A failure
    prints the failing row: fix the side the spec says is wrong (code, test
    or spec). Interpretation points (GM1–GM6, §7): the evilhut row (528 or
    529: the test checks "exactly one row, value 40"; record the row in
    `population.md`), `block` read as all three difficulties, `MonDen`
    checked on Normal only, "SrcDam 63" read as 63 rows with SrcDam ≠ 0 and
    "skpoints 0" as no row with a `skpoints` formula, `periodic 2` as rows
    {57, 277}, `mapped` = referenced by some live row, a non-zero
    `pSrvDmgFunc` names a filled slot 1–14. Then add the claims of the table
    in `game-tests-monsters-skills` §4 per passing test (they raised the
    game tier from 187 to 277 units when measured; verified follows only
    after the run), rerun `py tools/coverage.py --check`, record in §5 Done.
46. Formats sweeps and the client asset path (`game-tests-client-assets` §4;
    17 tests, never run), release builds because the DCC sweep decodes about
    22k files: (1) `D2_GAME_DIR=<install> cargo test --release -p d2-formats
    --test game_sweep -- --ignored --nocapture`: expect 11 passes, each
    sweep printing its counts (a failure that names a count while every file
    decodes is GA3; a failure of `animdata_matches_every_cof` on `with_cof`
    is GA4); (2) `cargo test --release -p d2-client --test game_assets --
    --ignored --nocapture --test-threads 1 --skip
    gpu_compositor_on_real_frames`: expect 5 passes, printing the
    listed-name count (0 refused) and the outcome of the two text `.tbl`
    files (GA1); (3) on a machine with a GPU, `… --test game_assets --
    --ignored --nocapture gpu_compositor_on_real_frames`: expect `adapter:
    <name>`, `GPU indices: 0 of 480000 bytes differ; GPU: 0 of 480000 pixels
    differ`, a pass (the RGBA count after the perturbation is only printed:
    the real palette can map two indices to one color). Counts per archive
    as `mpq-tool formats` counts (COF 3,606 files). If DC6 counts 1,651
    instead of 1,657 the six `patch_d2.mpq` names come from elsewhere: record
    the tool's method in the spec and fix `files_where`. After a pass turn
    each `Intended claim` line into `// Covers:` (16 game-tier units if all
    pass: `assets.md` 1 → 5, `render-pipeline.md` 3 → 4, `animdata.md` 1 → 8,
    `cof.md` 3 → 4, `dcc.md` 12 → 13, `palette.md` 2 → 3, `tbl.md` 3 → 4);
    a failing test is corrected from the observation or its claim dropped.
    Overlap with `formats_game.rs` is intended (the new tests add the
    all-archive counts and the §Test vectors rows).
47. Composition on a real GPU (`render-composition` C-rc1): `cargo run -p
    d2-client --example gpu_compare` → 18/18 `0 differing bytes`; `-- --perturb
    7` → every case FAIL with exactly 7 / 7, exit 1; `cargo test -p d2-client
    --lib gpu_compositor::tests::gpu -- --ignored --nocapture --test-threads
    1` → 2 pass. Record adapter name, backend and driver (llvmpipe only so
    far).
48. Composition in `verify` (C-rc2): `cargo run --release -p d2-client --
    verify` → the `map` case and the 10 synthetic cases PASS (the map case has
    no blend table, so the orientation change cannot move it); `--perturb 5`
    → 5. Run together with C42.
49. PL2 palette (`composition.md` OQ1; C-rc3): for each act, the first 1,024
    bytes of `pal.pl2` through `scene::present_palette` against the `.dat`
    palette, and entry 0 = (0, 0, 0). Decides whether the app may keep
    reading `.dat` (`map::cpu::to_rgba` paints index 0 black); also answers
    VM1 together with C28.
50. AnimData through `anim_record` (`e2e-combat-path` §6 check 1; test to
    write, local, `D2_GAME_DIR`): `SKA11HS`-style vectors through
    `ActionHooks::anim_record` with a fixed COF name (event bytes → event-0
    frames, `units.md` §4.2), then the e2e's lookups against real names once
    the composer is specified (`animdata.md` OQ2).

51. Inventory grids, belts and path tables on the live files
    (`game-tests-inventory-path` §4; `crates/d2-sim/tests/game_inventory_path.rs`):
    `D2_GAME_DIR=<install> cargo test -p d2-sim --test game_inventory_path --
    --ignored --nocapture` (the install with the MPQs and `Game.exe`).
    Expect **6 passed, 0 failed** (`live_inventory_records_both_resolutions`,
    `live_page_grids_every_owner`, `live_belt_capacities`,
    `live_potion_groups_similar`, `sweep_every_item_size_places`,
    `live_path_tables_equal_game_exe`); without `--ignored` the 2 CI tests
    pass. Record the printed lines: belt-item count, item sizes, zero-size
    items, items larger than 10 × 4, beltable and 1 × 1 beltable counts.
    Cross-check: `py tools/trace-recorder/path_tables.py` exits 0 and agrees
    with `live_path_tables_equal_game_exe`. Expected values are spec table
    facts (`inventory.md` §1.3 measured table, §3.1, D1 / D2), **none has run**.
    Then turn each `Claim once the first local run passes` line of a passing
    test into `// Covers:` (`inventory.md` §1.3, §1.2, §3 r1, §3 r4; the sweep
    and the path-table test get none) and re-run `py tools/coverage.py
    --check`. A failing test: fix the test from the observation, or turn the
    spec fact into an open question. Look first at GX1–GX3 (§7): record 29 =
    255 × 255 although the spec calls it a copy of record 13 (0 × 0); pages 5
    and 0xFF read as "other (0)" = the class record; items with body
    location 8 that are not belts (read through `body_location_allowed`).
52. DC6 / DCC / DT1 counts for placement (`render-camera-placement` "Local
    checks", C; the `mpq-tool formats` extension is code to write first):
    extend `mpq-tool formats` with (a) DC6 frames with `flip` ∉ {0, 1}
    (expect 0; else RC2 needs a decoder fix), (b) DCC frames with odd
    `variable0` (expect 0, `sprite-placement.md` OQ2), (c) zero bytes inside
    DC6 copy runs, DT1 RLE runs and DT1 iso diamonds (OQ1; any non-zero count
    means `IndexFrame` needs an opaque zero), run it, then
    `D2_GAME_DIR=<install> cargo test -p d2-client -- --ignored` to confirm
    `from_dc6` / `from_dcc` still accept every live file the ignored tests
    use.
53. The window with real files, no player needed (`render-wire` check 1):
    `D2_GAME_DIR=<game> cargo run -p d2-client --release -- play --frames
    1500`. Expect as `p6-integrate` check 1 (black window: `NoFeed` states no
    player), the log lines `last view Some(FrameStats { bridge_frame: <f>,
    server_tick: <t>, items: 0, … })` with `t` equal to that line's server
    ticks (one drawn frame per tick), no error, exit 0. Record the node frame
    count (now at most the tick count). Run with C40.
54. Composition and capture on a real GPU (`render-wire` check 3):
    `cargo test -p d2-client --lib
    verify::capture_case::scene_tests::gpu_half -- --ignored --nocapture` →
    `adapter: <real GPU>`, pass (llvmpipe: `GPU indices: 0`, `GPU: 0 of
    480000 pixels differ`, perturb 7 → 7 on all three); `cargo test -p
    d2-client --test app_frame_loop -- --nocapture` → 6 pass, 0 differing
    pixels in the GPU node. Record adapter name, backend, driver. Run with
    C47.
55. Placement replay on the existing recordings, no player
    (`conformance-path-render` §5 item 1): `cargo test -p conformance --test
    placement_replay recorded_player_placements_replay_exactly -- --ignored
    --nocapture`. Expect today the printed player placements — `20261006-015956`:
    the frame-1 0x15, guid 1 at (4863, 5653) (R1); `-022633`: (4673, 4548),
    room unnamed (ten 0x07 before it, R2), and the waypoint 0x15 at (4893,
    4993) with room level 3 tile (976, 992) (R3) — then FAIL `PLACEMENT NOT
    WIRED`. Record the printed counts (players, with_room, monster_assigns,
    item_messages) as the baseline; different R1–R3 numbers are a reader
    finding.
56. Movement replay on the existing recordings, no player
    (`conformance-path-render` §5 item 2): `cargo test -p conformance --test
    movement_replay recorded_walks_replay_exactly -- --ignored
    --nocapture`. Expect per file `read MoveReadStats { requests, ticks,
    seeds, sent, other_inputs }` with requests = the 0x01–0x04 counts of
    `pathing.md` R4 (121 + 47 + 215 + 2 = 385, all accepted) and R5 (49 + 9 +
    72 = 130), no 0x0F / 0x10 in `sent` (R4), then FAIL `MOVER NOT WIRED`.
    Different request counts are a reader finding (the reader counts every
    `c2s`; R4 / R5 count dispatched ones: equal when nothing was dropped).
57. The recordings list (`conformance-path-render` §5): `cargo run -p
    conformance --bin recordings-needed` (`D2_TRACES_RAW=<dir>` for another
    folder). Expect the 7 entries with MISSING / present / BLOCKED:
    `movement-walk` MISSING, `movement-path-state` and
    `placement-monsters-items` BLOCKED (recorder extensions, A-seventh),
    `placement-players`, `packets`, `units`, `stats` present or missing per
    the folder. Record the output as the baseline.
58. Benches on the developer PC (optional; the note asks for no local
    run): the four `cargo bench` commands of §4, compare with the cloud
    baseline of §1 3ad (a real machine should not be slower by an order of
    magnitude; MPQ Huffman is benched since the eighth fold: `-- mpq_huffman`, 22.2 ms for 256 KiB of text under table 0 in the cloud, row 3ai).
59. The seven live wired-host tests (`game-tests-wired-host` §4; **after step
    7u(a)** has made step 4 use the walk handler, else step 4's `assert_stub`
    fails on the first leg): `D2_GAME_DIR=<install> cargo test --release -p
    d2-server --test game_wired_host -- --ignored --nocapture --test-threads 1`.
    Expect **7 passed, 0 failed** (`wired_host_amazon`, `_sorceress`,
    `_necromancer`, `_paladin`, `_barbarian`, `_druid`, `_assassin`). Record per
    class the Blood Moor arrival position and frame, the kill / pick-up notes
    (GH1: population probably places presets only, so `kill: no monster within
    range`), and the printed `class N: digest <hex>`; run the command a second
    time and compare the seven digests (determinism across processes). A failing
    assertion names its step: read it against GH1–GH7 (§7 eighth set) before
    changing a test. No `Covers:` claim: integration only.
60. `GameData::load` on the live set (`fixedset-game` §4; optional, after C59):
    with `D2_GAME_DIR` set, `GameData::load(bins, &archives)` with
    `ActCreation::Full` should reproduce the setup `game_wired_host.rs`
    assembles by hand (same level ids, same seeds, same digests). If it does,
    folding that file onto the constructor is the refactor of step 7e.
61. Huffman decoder on game files after the speed-up (`mpq-huffman` queue 1):
    `D2_GAME_DIR=<install> cargo test -p d2-formats -- --ignored` and
    `cargo run --release -p mpq-tool -- formats`. Expect everything that passed
    before still passes; in particular all **5,008 Huffman + ADPCM `.wav`
    files** decode to their exact RIFF size (`mpq.md` Observations). The decoder
    is held to the reference model by property tests, but no game-file sector
    has run through the new code.
62. Which Huffman weight tables the `.wav` sectors use (`mpq-huffman` queue 2;
    decides whether further decoder work pays, step 7r): count byte 0 of the
    Huffman stage over the 0x41 / 0x81 sectors (no tool prints this today: add
    a `--huffman-tables` count to `mpq-tool formats`, or a one-off test) and how
    many of those sectors contain an escape. Record the table histogram in
    `mpq.md` Observations.
63. `VendorTables::from_fixed` on the live set (`mutants-world` §4; test home
    needed: `d2-sim` `world/vendors` `#[ignore]` test or `data-tool`): load the
    live tables and compare a few rows with `vendors.md` §9.3 / Constants (a
    Charsi `npc.txt` row's multipliers, `difficultylevels` gamble odds, an
    `itemtypes` row). This is what kills the six `vendors.rs` survivors
    (`From` impls, `typed`, `cost_mod`, `from_fixed`); likewise
    `ItemTables::from_fixed` / `LevelTables::from_fixed` / `InvTables::from_fixed`
    survivors of `mutants-items-treasure`, `mutants-inventory` and
    `mutants-server` (`ds1_input` `version < 7`, MS1) are decided only by game
    files: the existing `#[ignore]` tests of C34–C36 read the real tables, run
    them first.
64. Mutation survivors that only game files decide (`mutants-items-treasure`,
    `mutants-inventory`): `items/tables.rs` `typed`, `from_fixed` (1.14d table
    set), `inventory/tables.rs:84, 111, 118` (`real_grid_belt_and_type_tables`,
    C36): after C35 / C36 pass, re-run `cargo mutants -p d2-sim --file
    'crates/d2-sim/src/items/**' -- --lib -- items:: --iterate` with
    `D2_GAME_DIR` set and `-- --include-ignored` for those files; expect the
    survivors named in the two notes to be caught. Optional, about an hour.

65. Lighting tables against `Game.exe` (`impl-lighting-blend`; Ghidra or a
    hex read of the 1.14d `Game.exe`, no run): `specs/render/env-periods.tsv`
    = the 18 × 12-byte entries at `0x007443F0`, `0x00744438`, `0x00744480`
    (start degree, type, color `0x00BBGGRR`), and
    `specs/render/wall-light-points.tsv` = `0x0072A9E8` / `0x0072ABC8` (9
    directions × 6 points × 2 tables). Expect identical rows; the repo
    tests only hold the TSVs to the spec text.
66. Monster colormap file sizes (`impl-lighting-blend`, `shading.md` §6):
    `mpq-tool extract` `Data\Global\Monsters\RandTransforms.dat` (expect
    7,680 bytes = 30 headerless maps), `GreenBlood.dat` (expect 256) and one
    class's `palshift.dat` (expect 2,048). Another size means
    `rules::shading::{rand_transforms_map, blood_map}` read the wrong bytes.
67. Environment per tick (`lighting.md` OQ7, `impl-lighting-blend`): record
    env `+0x0C` (and R, G, B `+0x18..+0x1A`) every client update over one
    in-game day and replay `rules::lighting::environment::Environment::update`
    from the same start; expect equal on every tick. Add the light-map
    SHA-256 (`LightMap::digest`, 18,432 bytes at `0x007B0E68`) and `q` to the
    capture key (§12 r3, OQ9).
68. Captures for the GDI and shadow rules (`impl-lighting-blend`): a weather
    line or Arcane Sanctuary star (static camera) compared with
    `rules::blend::gdi_line_pixels` — settles `blend-modes.md` §8 r1's
    |dx| = |dy| major axis (now `BlendError::LineMajorAxisTie`) and whether
    the line reaches (x1, y1); the player's shadow on a flat floor against
    `unit_shadow_position` (blend-modes OQ1).

Kept entries (unchanged):

**Treasure** (`specs/items/treasure.md`, branch `claude/phase3-treasure`):
Recording (player): `record_rng.py` extended with hooks at `0x0055A6D0`

Phase 6 asset budgets (from `specs/client/assets.md` OQ 1, design
only, no code yet): measure the decoded size (`Σ width × height`) of
every live DCC/DC6/DT1 frame, per file and in total, and of the files a
town and a dungeon scene use; record the numbers in `assets.md` §A5 and
set the default budgets from them.

### Blocked (not runnable yet)

- Phase 6 §B1 decoded samples per live `.wav` (needs `formats/wav.md`).
- Phase 6 GPU: the `map` case through the compute compositor is ported and
  queued as C28–C31 (it will show the index-0 question VM1); the `sprite` /
  `unit` GPU cases ran in the 11-case verify (C30) with the store in place.
- Eighth fold: the server walk handler exists (3ag), so C56 no longer waits
  on it; what is missing is a `movement::Mover` over the recorded game (the
  recording's map seed, GX4, and `path::walk` behind it) and the per-tick
  two-client recording (S8-A1).
- Seventh fold: the movement and placement replays (C55, C56) fail `NOT
  WIRED` until a d2-sim provider is behind `movement::Mover` (the recorded
  game plus `path::walk` behind the server's handlers and tick, after step 7p)
  and `placement::PlacementModel` (the recorded game's DRLG seeds plus
  `path` §10–§12, `monsters::population`, §9 / `treasure.md` §7); the
  recordings `movement-path-state` and `placement-monsters-items` need
  recorder extensions (S7-A3); the capture's pixel compare needs the recorder
  gap (RW1) and the frame-cycle base (RW3); the live spawn-point test of
  `path-placement.md` §11 needs each recording's map seed (GX4) and a live
  `CollisionView` / `LevelView` provider; live belt vectors B1–B5 need an
  `InvWorld` implementation (30 methods); the item bit stream of 0x9C / 0x9D
  (`inventory.md` OQ1) blocks a byte-exact S→C for the inventory.
- Smoke on a machine with audio, after `output::register` and
  `AudioPlayer` are wired in `app.rs`: `d2-client view` plays a scripted
  `Cue` set; audible, no underrun errors (not an exactness check).
- Original-defaults check of the controls file: needs
  `specs/ui/controls.md` (§B4): identical action list vs the 1.14d key
  config.
- Level types on the live tables (`drlg/levels.md` Test vectors,
  `rooms.md` §9.9, `outdoor.md` Test vectors, `preset.md`): the generators
  exist, `wiring::worldgen::WorldTypes` is the dispatcher and
  `d2_server::world_data` provides the DS1 / DT1 / lvlsub data from the
  user's files (C23 runs the Act I placement, Den of Evil and the Blood
  Moor / Cold Plains outcome checks; **what stays blocked is the RNG
  trace compare below**, which needs the draw sequence, not only the
  outcomes), as ignored tests
  reading `D2_GAME_DIR` (`wire-worldgen.md` §7.1: a crate holding both
  `d2-formats` and `d2-sim`, e.g. an ignored `conformance` test, builds
  `WorldTypes` from `DrlgData`, `PresetData`, `OutdoorData`, `MazeData`,
  DS1s from `d2_formats::ds1` and the lvlsub DS1s, creates act 0 with init
  seed 644409375 and compares `20261005-232125-rng.jsonl`: preset draws at
  `0x0066749F` seq 2433–2453, town generation seq 2455–2559, Blood Moor seq
  2561–6331). Act 1 level list and town tiles: with `OutdoorTypes`
  (the outdoor placer), `Drlg::create(0, 644409375, 0, 1, false, ...)` on the live
  tables must give the list 16, 15, …, 4 (head first) with seeds
  `init_low(4014346869 + id)`, the DRLG seed after creation and the 47
  town room seeds of `20261005-232125-rng.jsonl` seq 2425–2591; an ignored
  test (`D2_GAME_DIR`) building `TownW1.ds1` rooms through the preset type
  with the real Act 1 DT1s in server order 15, 7, 14, 8, 9, 21, 16, 22, 23
  and client order 34 → 0 must equal the `rooms.md` §9.9 table and the
  first 12 draws of seq 6822–6833. Also with the live leveldefs /
  lvlprest / lvlsub (outdoor): the derived rects of `outdoor.md` Test
  vectors ("derived" list) and OQ 1, DRLG seed {1406222081, 1674353446};
  Blood Moor / Cold Plains build draws: generate levels 2 and 3 and
  compare the (site, seed-after) sequence with `20261005-232125-rng.jsonl`
  seq 2561–6331 (Blood Moor) and 6896–12010 (Cold Plains), per-site counts
  of the `outdoor.md` Test vectors table (e.g. Blood Moor build list 15,
  shuffles 100 / 100, jitter 3 / 11 / 11, shrines 1 / 50 / 50, preset
  cells 48, rooms 81, sub-theme 198); Blood Moor room substitution
  (`outdoor-tilesub.md` Test vectors: reset seed {2795816810, 666} → 10
  group rolls, trials 1, 14, 7, 20, 20, 20, 5, 20 and 3, 5; 115 (x, y)
  pairs, seq 32974–33381); preset on the recording: Act 1 with start seed
  4014346869 gives `roll(1)` → 0 at `0x0066749F` for levels 26, 13–16
  (seq 2433–2453), `roll(3)` lo' 2260552554 → 0 for level 27; level 1
  generation: 35 level-seed steps at `0x0066B42E` (seq 2455–2559), first
  lo' 2928842600, no filter draws, rooms row-major (7 × 5); outdoor maps of
  levels 2 and 3: 48 / 61 `roll(Files)` steps per build at `0x00666F33`.
- Replays on providers that do not exist yet (existing recording
  `traces/raw/20261006-015956-packets.jsonl`, plus live tables): NPC talk /
  trade at frames 746, 747, 898, 1958 through the `world::npc` handlers,
  comparing message bytes (0x2A bytes 3–6 masked) and order (`npc.md` OQ1,
  OQ4, OQ5; N1–N8); vendors `generate` for Charsi (frame 899) and Akara
  (frame 1779) must reproduce the recorded 0x9C action 11 code lists in
  order with the recorded NPC-control seed (needs the economy
  `create_item` provider and the live tables); the three recorded
  transactions (buy 56, sell 500, buy 40) through `buy` / `sell` with real
  providers; group sizes and creation order of
  `20261006-015554` / `-021854` / `-022304` through `population` once
  `MonsterInit` and the DRLG providers exist (§5 A18).
- Replay of `traces/raw/20261006-015956-packets.jsonl` frames 1729–1751
  through `Host` + `TradeWorld` (`quest-host.md` §7; needs a packet-recording
  replay through the server host, `e2e-vendor-host.md` §8): expect S→C 0x27
  and 0x29 in frame 1729 and `5d 01 00 01 0000` at frame 1744, comparing
  bytes with 0x27 bytes 6–39 masked until `0x00661480` is specified.
- Replays through the single-player e2e (`e2e-next.md` §7; need the trade
  recording replay, `wire-interaction.md` §8 item 1, and the sorceress
  kill recording, `e2e-combat-path.md` §6 item 2): `cargo test -p d2-client
  --test e2e_single_player` with recording-backed seams; compare S→C bytes
  per frame (0x2A bytes 3–6 masked) and the level-up stats after the kill
  (expect equal; the 100-experience threshold in the e2e is synthetic, the
  real `experience.txt` needs 500).
- A recording of a first room population through `WorldSim` (population
  note check 2, `wire-worldgen.md` §7.2): blocked until the DRLG coordinate
  lists are specified; with the default `WorldPending` no room population
  runs.
- Replay of the spawn RNG recording (`init.md` OQ4: one population pass, rng
  hook with caller addresses; `wire-routing.md` §7 check 1) through
  `WorldSim` with recording-backed `WorldPending`: a monster created by
  action code through the lent world must take the same unit-seed draws as
  population's creation; expect the per-unit draw sequences equal
  (`cargo test -p conformance` once the replay exists).
- `cargo test -p conformance --test tick_replay` after any change to
  `WorldSim` / `ActionSim` routing (repo only; expect 7 pass, unchanged,
  11,105 ticks and 0 mismatches; the replay uses its own hooks;
  `wire-interaction.md` §8.3, `wire-routing.md` §7 check 3). Skill-use tick
  events 5, 8, 9, 12 are routed now (`wire-open-seams`, `wire-routing`);
  type 14 has no body (RT6).
- Replays through `items::moves` and the inventory (`inventory.md` R1–R6;
  grid positions R2, belt slots / compaction R4): need the recordings (§5 A)
  and the connection of step 7i (a provider for `InventoryOps`, `MoveUnits`,
  `MovePending`, then an adapter in `d2-server`).
- Position replays (`path-placement.md` R1–R3: game entry, waypoint travel,
  warp arrival against the recorded 0x15 / 0x0D): need the position trace
  (§5 A), a DRLG of the recorded game that can be regenerated (C23) and the
  path core (step 7j).
- `conformance` replays on 1.14d recordings: `packets_replay` passes only
  with session + world that reproduce the recorded game and every S→C sender
  (C39 records the baseline); `rooms_replay` needs the `rooms.md` Test
  vectors "Full check" recorder extension (per active room: DRLG room, rect,
  level, rooms-near array) and a `RoomModel` that builds a `Drlg` level from
  the recorded rects and returns `Drlg::adjacent_rooms` (CH1).
- Capture compare cases (`placement-0001`, `camera-0001`,
  `composition-0001`): need a `SceneSource` (step 7l, CP1).
- Trade recording through `Host` + `TradeWorld` (`wire-interaction.md` §8
  item 1, `e2e-vendor-host.md` §8): mask 0x2A bytes 3–6 (the unwritten bytes,
  SC1); the cast-and-kill recording (§5 A) through recording-backed seams
  (settles EC1–EC3).
- The aura and quest recordings through `QuestTick` / `Desk::quest_message`
  (A, `wire-open-seams` §7 checks 1–2): compare message bytes and order.

## 6. Environment

- Developer PC: Windows 10, Git, rustup (default and pinned 1.99.0),
  VS 2022 Build Tools, Temurin JDK 21, Ghidra 12.1.4 at
  `%LOCALAPPDATA%\Programs\ghidra_12.1.4_PUBLIC`, Python 3.10 (`py`),
  GitHub CLI (`gh`, logged in as MoggerCat). Disk is tight: build only the
  crates you need.
- `game/`: 1.14d install (`Game.exe` 1.14.3.71), gitignored. Extracted
  tables in `game/extracted/{patch_d2,d2exp,d2data}/` (scratch).
- `re/`: Ghidra project `re/ghidra/D2_114d.gpr` and exports
  (`re/exports/functions.tsv`, `re/exports/funcs/`), gitignored. How to
  regenerate: `tools/ghidra/README.md`.
- `../refs/`: D2MOO (MIT), Riiablo (Apache-2.0), CE_Database,
  1.14d-notes. Spec sessions only.
- Git: `main` on `git@github.com:MoggerCat/D2RUST-Project.git` (renamed from `MXL-ULTIMATE` on 2026-10-06; GitHub redirects the old URL; update a clone with `git remote set-url origin git@github.com:MoggerCat/D2RUST-Project.git`). CI runs on
  pushes to `main` and on pull requests. A cold CI run took ~40 min
  (Bevy), cached ~3 min, as one serial job; since `ci-speed` there are five
  parallel jobs (§4) and the wall time is the slowest job: the `client` job
  when cold (Bevy compile; measured locally cold: clippy 266 s, d2-client
  test 693 s, mostly compiling), a ~1–2 min incremental build when cached
  (**estimates**: workflow logs were not reachable; replace them with the
  first measured runs).

## 7. Where facts live

Simulation core: `specs/sim/tick.md` (tick rate, steps, timer queue; the
sim/tick trace format), `sim/unit-order.md` (GUIDs, unit/room/client
lists), `sim/intents-events.md` (+ `client-messages.tsv`,
`server-messages.tsv`). Units: `sim/units.md` (kinds, lifecycle, modes,
what each timer event does per kind; `unit-events.tsv`,
`unit-handlers.tsv`). Stats: `sim/stats.md` (ids, values, ops;
`stat-ops.tsv`), `sim/stat-lists.md` (lists, modifiers, states, regen).
Treasure classes and
drops: `specs/items/treasure.md` (+ `treasure-quality.tsv`,
`treasure-chest-acts.tsv`). Items:
`specs/items/generation.md`, `quality.md`, `affixes.md`, `properties.md`
(+ `property-functions.tsv`). Combat and skills:
`specs/combat/hit.md`, `damage.md`, `specs/skills/levels.md` (+
`skillcalc.tsv`, `misscalc.tsv`), `use.md` (+ `functions.tsv`;
`combat/vitals.md`: creation, stat points,
level-up, experience table). Monsters and
missiles: `specs/monsters/population.md` (+ `preset-monsters.tsv`),
`init.md` (+ `umods.tsv`), `ai.md` (+ `ai-functions.tsv`),
`specs/missiles/missiles.md` (+ `srvdo.tsv`, `srvhit.tsv`). World: `specs/world/quests.md` (+
`quests.tsv`, `quest-messages.tsv`), `waypoints.md` (+ `waypoints.tsv`), `cube.md` (+ `cube-ops.tsv`), `npc.md`,
`vendors.md` (+ `vendors.tsv`; store generation reproduces the two
recorded stores item for item). Level generation:
`specs/drlg/levels.md`, `rooms.md` (rooms-near order, adjacency arrays,
the deactivation counter = `tick.md` OQ4), `preset.md` (+
`preset-tables.tsv`), `maze.md` (+ `maze-specials.tsv`), `outdoor.md`,
`outdoor-tilesub.md`. Items, inventory and moves: `items/inventory.md`
(+ `item-actions.tsv`). Paths: `sim/path-placement.md` (+ `path-tables.tsv`),
`sim/pathing.md`. Frame composition and captures: `render/{sprite-placement,
camera,composition,capture}.md` (`traces/FORMAT.md` §Render captures). All
draft: rules from the 1.14d disassembly, RNG draw order not yet checked on a trace. Format facts: `specs/formats/*`. Map rendering: `specs/render/map-preview.md`.
Data loading and tables: `specs/data/*` (start at `loading.md`). RNG:
`specs/sim/rng.md`. Client↔game boundary: `specs/client/bridge.md`. Phase 6 client design (d2rs-own drafts, original
behavior listed as unwritten owner specs in each part (b)):
`specs/client/{render-pipeline,assets,ui,audio}.md`. Each spec's "Open questions" holds its unknowns.
Carried-over open questions not yet in a spec's list:

1. 8 unflagged invisible collision tiles in `townN1.ds1` draw as blue
   patches (`map-preview.md` OQ3; needs RE of the client tile draw path).
2. DS1 v12/13 trailing bytes (possibly an early NPC-path section).
3. DC6/DCC vertical placement (one-row disagreement between sources). Owner: `render/sprite-placement.md` (`specs/client/render-pipeline.md` §B1).
4. Meaning of the PL2 rendering tables (Phase 6). Owner: `render/shading.md` (`render-pipeline.md` §B3).
5. `client-messages.tsv` repeats the field name `unk` in one layout
   (0x67 at 0x2B/0x2C, 0x68 at 8/0x14); the generator names them
   `unk_43`, `unk_44`, `unk_8`, `unk_20` (offset suffix). A spec session
   may give them distinct names (`intents-events.md` §5); the parser
   could then reject repeats.
6. The C→S chat size rule (§2.1 rule 5) can come out negative (signed
   byte, e.g. `15 01 00 'hi' 00 'bob' 00 80` → −117). What the
   classifier `0x0052B100` does with a negative size is not in the spec;
   `d2-proto` returns `Size::Negative` / `Classified::NegativeSize`
   instead of guessing; `d2-server` carries it through the seam
   (`SizeError::Negative`, `Classified::NegativeSize`: not queued).
7. S→C buffer split (§3.3) when a message's size runs past the buffer's
   end: not in the spec; `split_server_buffer` returns an error (buffers
   hold whole messages, §3.2 rule 2).
8. S→C 0x15 (ReassignPlayer, 11 bytes) has no `layout` in
   `server-messages.tsv`, so the point parser's resync (§2.4 rule 3)
   cannot be built; `SimGame::queue_resync` logs the request instead
   (from `claude/phase3-wiring`). A spec session should read the sender
   `0x0053BC10` and fill the layout.

From the tick-core implementation (`claude/phase3-tick`; each has a
`TODO` in code naming it):

- T1. Negative timer buckets (frame past 2^31 − 1, `tick.md` §2.2): in
  1.14d they address neighbouring queue fields (§5.1 layout); d2rs
  panics. Unreachable (≈ 994 days); model only if ever needed.
- T2. Step 6 update-queue walk (`tick.md` §3 step 6): `unit-order.md`
  §10 does not say whether the next link is read before or after
  `0x00553220`; d2rs saves it before. Confirm in the disassembly.
- T3. `tick.md` §5.2 rule 4 (monster AI-think with state 54): left to
  the caller (`timer::needs_uninterruptable_check`) until monster states
  exist; the monster spec should say whether `0x005544B0` schedules or
  cancels timers (order relative to the new timer's allocation).
- T4. Unit-less timers (`tick.md` OQ 2) and tile timers: rejected
  (`TimerError::NoTimerClass`); `TimerOwner` always has a unit.
- T5. Step 9 `0x0061A910` and `compress_unit`: d2rs only unlinks the
  room from the act list; whether compression unlinks units and what
  else room removal frees belong to the DRLG / room-lifecycle spec.

From the tick trace replay (`claude/phase3-tick-replay`):

- TR1. d2-sim deviated from `unit-order.md` §1.4/§3.1: it could not
  hold a unit allocated but not yet added, and the missile init
  (`0x0059F8A0`, `tick.md` §5.3) schedules its every-tick event in
  between (`sim-0006` tick 3576, `sim-0008` tick 677: `timer_set` before
  the missile's `room_add`). Fixed with `alloc_unit` / `add_allocated`.
  The spec rules already say this; a spec session may add the missile
  case to §3.1 as an example.
- TR2. Recorder/converter gaps (not bugs; the replay works around them,
  documented in `conformance::tick::replay`): unit allocation is not
  recorded (the replay allocates a unit at its first reference, a
  `timer_set`); client joins, client states and the client's player link
  are not recorded (the replay sets up the single-player client in game
  without a player, so the §6.5 per-client player queue insert is
  replayed from the recording, not produced by d2-sim); the snapshots'
  `adj` arrays are not compared (DRLG data, no input drives them).
  Recording client joins and the client→player link in `record_tick.py`
  would let the replay produce the client pass. Recorder semantics the
  replay relies on, confirmed on all three traces: `room_remove` is
  logged for every call (also for a unit without a room, then no
  `queue_remove` follows), `queue_remove` for every call from the room
  unlink of a unit in a room (queued or not), `queue_add` only for an
  actual insert.
- TR3. T5 above, partly answered by the recordings: step 9 compression
  removes each unit of the room (room unlink, queue unlink, hash unlink,
  `unit-order.md` §3.2) in room-list order before the room is unlinked;
  the replay feeds the removals from the recording (`compress_unit` hook)
  and d2-sim's own `deactivate_room` matches. What else compression and
  `0x0061A910` do stays with the DRLG spec.
- TR4. `convert_tick.py` and `tests/tick_traces.rs` cite `tick.md`
  "Test vectors, Trace sim/tick" for the event kinds and fields; that
  section does not exist. A spec session should add it (kinds and
  fields: `convert_tick.py` docstring and `convert()`).

From parser robustness (`claude/parser-robustness`):

- R1. PKWARE explode: input ending inside the end marker's extra length
   bits. `mpq.md` §10 reads as "still the end marker"; `explode.rs`
   returns an error. Same outcome for a lone PKWARE sector, differs for
   PKWARE→Huffman chains. Confirm on 1.14d (from `claude/parser-robustness`).
- R2. Patch layers P09: does a `table` line that fails P08 count as the
   section's first `table`? Today it does (the layer is an error either
   way; only the finding list differs). `patch-layers.md` §3.


From the Phase 3 system implementations (2026-10-06, notes
`docs/handoff/impl-*.md`), grouped by owner spec. Each has a `TODO` in
code at its site (the tag is the spec § / OQ named in the item; code
tags are of the form `TODO(stat-lists.md §7.2)`). Every reading below is
the narrowest one taken, none is confirmed on 1.14d. A spec session
decides each and removes the TODO.

**Integration (two notes or parts disagree, not decided here):**

- I1. Home of live-data tests for `d2-sim` modules: the units / stats
  note asks for a `d2-formats` dev-dependency of `d2-sim` ("dependency
  request") or the check in `crates/conformance`; items says
  `crates/conformance` or `d2-data/tests`; treasure says
  `crates/conformance` or a `d2-sim` dev-dependency; waypoints says
  `data-tool` or `crates/conformance`; DRLG says `d2-sim` does not need
  `d2-formats`; combat already has three `#[ignore]` tests in `d2-sim`
  reading extracted files from `game/extracted/…`. Second set: monster
  init (`real_level_stats`) and NPC (`live_monstats_records`) followed
  combat (ignored tests in `d2-sim` through `skills::tests_game`); preset
  says `d2-sim` needs no `d2-formats` (DS1 arrives as `Ds1Input`) and its
  DS1 survey needs a crate with both, e.g. an ignored test in
  `conformance`; maze and population say an ignored test reading
  `D2_GAME_DIR`; vendors says `data-tool` or `crates/conformance`; skill
  use / vitals need a loader for `charstats` / `experience`. Decide once.
- I2. Ratio row duplicated: `treasure::quality::ratio_row` and
  `items::quality::ratio_row` (version-limit argument) both implement
  `treasure.md` §6 step 3; the items note says merge when both land (the
  economy wiring confirms both still run their own tests).
- I3. Event dispatch order: the units note has `UnitSystem` implement
  `EventDispatch`; the monsters note builds `MonsterDispatch {
  MissileDispatch { rest } }` and says everything else passes to `next`.
  Which wraps which is not decided. `wiring::action` implements neither:
  it routes timer events through the unit dispatch of `units.md` §5
  (`UnitSystem` over `ActionHooks`), because `MonsterDispatch` runs type 10
  for a frozen monster while `tick.md` §5.6 (conformance-passing) drops
  types 0, 1, 2, 6, 7, 9, 10, 11, 13, 14; the AI test
  `monsters::ai::tests::freeze_drops_thinks_and_type_10_resets` expects the
  chain's behavior (W1; the AI module's owner revisits the test).
- I4. Treasure module path: `treasure.md` header names `d2-data::treasure`
  (runtime form) and `d2-sim::items::treasure` (drops); the code is
  `d2_sim::treasure`. Update the header or move §1 into `d2-data`.
- I5. Test counts of `cargo test -p d2-sim` differ per branch (104–134 for
  the first notes; 512–572 for the second: economy 512, action 514, preset
  537, maze 538, population 547, npc 544, init and skill use 551, outdoor
  553, vendors 572); none is a merged count.
- I6. Item seed derivation runs twice per item: the allocator
  (`units.md` §3.1 step 4) and `create_item` (`generation.md` §2.1) both
  derive the unit and item seeds from the game seed. `Economy::create_item`
  runs the allocator on a copy of the game seed and checks both end in the
  same state (`EconomyError::SeedMismatch`), so the game seed steps exactly
  twice per item; `impl-items` suggests moving the derivation into the
  allocator (a module change), the economy wiring left it.
- I7. Game-creation fields (game seed, difficulty, expansion, +0x6D,
  +0x70, +0x6A, +0x74, +0x1B24, unique bits) have no field on `Game`: they
  live in `wiring::economy::GameFields`, the game seed also in
  `wiring::action::ActionHooks::game_seed` (W16: hosts must seed it at game
  creation), and as `GameInfo` in both `monsters::population` and
  `monsters::init`.
- I8. Monster boss minions: `init.md` runs xfer / owner data / minion list /
  flag 0x10 in `boss_minions_and_init` after each `spawn_boss_minion`;
  `population.md` §6.5 step 4 also lists them, so the population provider
  must not repeat them. The spec names `monsters::placement` for §9, the
  code is `monsters::population::placement`.
- I9. Level-type composition: `Maze` is not a `LevelTypes` (the dispatcher
  calls `init_level` / `generate` / `reset_level`), `OutdoorTypes` is an
  adapter that forwards other types to `others`, `drlg::preset` exposes
  methods for a "future dispatcher"; `DrlgError` has no variant for
  `MazeError`, `OutdoorError` or `PresetError` (each note asks for one;
  outdoor reports `UnknownLevel(id)` and keeps the real error in
  `last_error`); `LevelTypes::door_unit` carries no orientation (§11 needs
  it); `RoomGrids` has no `Logicals` field; `DrlgData::doors` is superseded
  by `PresetTables::doors`; `PresetRoom::link` carries the link bit that
  belongs to the outdoor record.
- I10. The tick hook for room removal: `impl-drlg` asked for
  `room_removed(game, act, room)`; `wiring::action` added
  `TickHooks::room_deactivated(game, act, room)` after
  `lists.deactivate_room(room)` in step 9 (the rest of `0x0061A910` stays
  the DRLG's, T5).
- I11. NPC / vendor record ownership: `impl-vendors` says `world::npc`
  should embed one `VendorRecord` per NPC record (built with
  `VendorRecord::new(class, act, trader, &GlobalLists)` at game creation);
  `impl-npc` defines `NpcRecord` / `NpcControl` and embeds an
  `InteractionList` in `interact` monsters' data, without a
  `VendorRecord`. Not reconciled. Both agree that the NPC-control seed stays
  with `NpcControl` and that the vendors draw from it.
- I12. Verify harness decisions open to the coordinator: exit code 2 now
  covers both `GPU NOT WIRED` and `NO ADAPTER` (never a pass); the
  stable-capture rule is applied as "two dispatches agree" (one dispatch
  may be judged enough, there is no frame pacing to wait out); `[[unit]]`
  extends case format version 1 instead of bumping it (a reader without it
  rejects the key); `GpuOutcome::Image` carries `{ indices, rgba }`
  (`p6-verify` had RGBA only).

**`sim/stat-lists.md`, `sim/stats.md`, `sim/units.md`** (notes: units-stats):

- OQ1 (`TODO(stat-lists.md open question 1)`) x87 precision of the max
  rescale: emulated with integers at `StatData::rescale_precision` (default
  53 bits); low 32 bits of the 64-bit conversion for out-of-range values.
- §7.2: the monster stat-74 write is read as its own step after the
  rescale. §6.4: an op stat ≥ n other than 0xFFFF ends the loop (as the
  fix-up does). §6.2: "r = 0 → skip" applied to charstat contributions too.
- **§10.1 vs `units.md` §6.1** (spec disagreement): `units.md` gates
  stamina and mana on `0x00580610`; `stat-lists.md` runs all three steps;
  d2rs follows `stat-lists.md`; the player fraction update is read as
  inside the r ≠ 0 branch. §10.1 step 6: unit +0xB0 := 0 not modelled.
- §10.3: "invalid skill" read as ≥ skills count. §10.4 / OQ5: an expired
  extended list panics in d2rs (an endless loop in 1.14d). §8.8: the
  stay-on-death rule `0x0063A4A0` is unwritten (seam, default false).
- `units.md` §3.1: allocation without `SUNIT_Add` is `UnitError::NotAdded`;
  d2rs adds before the per-kind init; item start seed taken as the derived
  `lo'`. §4.1: frame count set from AnimData in both prepare branches;
  without a path the speed is unchanged. §4.2: speed 0 → variants do not
  write +0x44; an event byte ≥ 144 or negative speed is an error. §4.5: a
  failed `0x0057EEC0(4, 1, 0)` at the end of a knockback does nothing.
  §6.1: event 6 without a hover does nothing.
- `stats.md` real-data vectors: not run on a game set (§5 C2, I1).

**`items/generation.md`, `quality.md`, `affixes.md`, `properties.md`** (items):

- OQ-G1 (gold, §8 step 1): a request quantity > 0 is the override (+0x54),
  not +0x34. OQ-G2 (forced §9 step 2): flag copies apply to every forced
  item.
- OQ-P1 (functions 18, 19): "set the stat" = §4.2 with set = 1. OQ-P2
  (function 14): cap < 1 returns 0 without setting the flag or stat.
  OQ-P3 (runeword match §10.1): a row matches only with exactly n runes.
  OQ-P4 (set bonuses §11): list flags not in the spec; the caller passes
  them in `ListKey`.
- OQ-A1 (rare §7 / crafted §8 / automagic §11): the roller's "force"
  argument read as true.
- OQ-Q1: the `quality.md` §8.1 vector "idx 4097 … unique fails (unless
  `nolimit`)" contradicts §8 step 6 (accept test runs before the marking):
  implemented per the rule (test `unique_index_4097`). OQ-Q2: itemratio
  divisor 0 → `Fatal::DivideByZero` (none in 1.14d data).
- Format-0 branches (`generation.md` OQ1, `affixes.md` OQ2) not
  implemented (run the format ≥ 1 rules); quality-5 socket filler
  `0x00663CC0` (`properties.md` OQ2) is a no-op; the "stat" vs "base stat"
  readings (totals for stat 72 in `D`, stats 21–24 / 31 / 159 / 160 in low
  quality, 194 in §7.3, 252 / 253, 254, 152; base for 73 and ethereal);
  low-quality tier > 3 → 4 literal.

**`items/treasure.md`** (treasure):

- OQ-atol: CRT `atol` beyond i32 saturates. OQ-quest-magic: §6 step 2
  `magic` and `quest` implemented as a conjunction. OQ5 NoDrop range:
  binary64 round-to-nearest-even; out-of-range → `NoDropRange`.
  §5.6 with `get` = none: picks 1, parent's mods. OQ-gold-equiv: "type 4
  or equivalent" uses the item test of `0x00629A90`. Ratio row
  match: `Class Specific` / `Uber` compared with 0 / 1 (a stored 2 matches
  neither); "type ≠ 38" reads the item's own `type`. Itemratio divisor 0 →
  `ZeroDivisor`. §3.1 order flag → collision → `bonewall` fatal (fatality
  unconfirmed). Chest act: caller input, act > 4 → `Act`. treasureclassex
  +0x30 / +0x32 built as 0 (not in `fields.tsv`). §1.5 step 2 quote cut.
  Expansion search over zero entries selects none. OQ7 "living" counts come
  from the caller. Original fatals (0xF3A, 0xF44, 0xFEA, no ratio row,
  bonewall, > 65,534 TCs) are `TreasureError` values. OQ 8 `DropSink::place`
  (free-spot search `0x0064E810`): no spec.

**`combat/hit.md`, `combat/damage.md`, `skills/levels.md`** (combat):

- `damage.md` Test vectors `pct(0x200000, 50, 0x30000)` = 34 vs §0's rule
  giving 533: code follows §0 (spec disagreement with itself).
- `levels.md` OQ2 level cap (`LEVEL_CAP_114D` = 99, caller-supplied); OQ4
  `roll_elemental` placement `0x0056C8E0` unspecified; §3.1 the elemental
  mastery getter and whether a gated minimum still evaluates
  `EDmgSymPerCalc`; OQ5 / OQ8 0x3B result codes, blood-mana payment
  (`ManaUnits::pay_life`). `calc-expressions.md` §3.5: skills `rand` with
  context but no unit (no draw), missile `rand` returns 0 (OQ1).
- `hit.md` §6.4 weapon block with no entry (0). `damage.md` §3.1 step 6
  bypass stats 103 / 104 / 106 getter; step 13 "high nibble free"
  (`hit_class & 0xF0 = 0`); §3.2 `item_normaldamage`; §4.5 leech rows take
  the difficulty penalty for non-monster defenders on a literal reading
  (monster drain on a Hell player ×2); §5.1 step 6 thorns literal; §5.3
  missing attacker (leech stops after the shift); §5.7 a new freeze list
  also switches state 1 on; §8 crushing blow player-count term for
  non-hireling monsters only; §9 "defender with an inventory" = every
  player; §7 reaction / kill is a seam only (OQ3).

**`drlg/levels.md`, `drlg/rooms.md`** (DRLG): wall remap values (`rooms.md`
§9.6, `TODO(rooms.md §9.6)`); merge case analysis; warp tiles (§9.5);
door tables (OQ10); keep argument of `0x0066F1A0` (§9.2); link list order;
animation frame flags (§9.7: copy base flags + 0x8); all-zero rarities
(§9.4: first entry, no draw); empty lvltypes `File` names skipped (§9.3);
spawn-tile class rule (`levels.md` §10.2; `Position` ≠ 0 with index 13 and
no waypoint room → `NoWaypointRoom`); unbounded overflows kept with TODOs
(near candidates > 30 `rooms.md` OQ5, warp-room centres > 9 `levels.md`
edge case 3, populated memory §9.4); carried from the spec: set handlers 0,
2 and unset handler 2 follow D2MOO (OQ1), `0x0066C0B0` treated as passing
(OQ2), client arrays sorted by client slot (OQ4), collision build assumes
every listed room has its grid (OQ11). Not implemented (no server
outcome): client-copy build timer (§4.6), animation phase copy
`0x0066D750` (§9.7), tile screen coordinates, act tile-library cache
(`levels.md` §3.5), unit flags in a removed room (§8.2). Tick request:
`TickHooks` has no hook after the room unlink; DRLG asks for e.g.
`room_removed(game, act, room)` (tick OQ T5, TR3).

**`missiles/missiles.md`** (monsters note): negative pierce P compared
signed; collide types ≥ 9 = no callback, mask 0; §R6.2 mastery stats read
from the missile; `phys += phys × pct / 100` 32-bit wrapping; §R6.1 armor
stat 120 as `add_target_ac` (sign to confirm); damage without an owner
(`apply_damage` with `owner: None`; skills spec decides); result-flag bit
values are d2rs-local; hit handler with no record returns 1; §R2.3 step 8
D2MOO order (spec OQ6), step 11 anim frame not stored; stubbed server-do
functions keep the missile forever (a sim using them accumulates missiles).

**`monsters/ai.md`**: forced target (§5.1 / §5.2 step 3) melee test only (OQ6);
slot-9 reported distance; `0x005DE9D0` collision only; `SplEndGeneric`
inline thinks do not set neutral; target 0 passes `Point(0, 0)`; Navi
clamp; velocity "method" (13 only for CorruptLancer step 1 and
`0x005DED40`, else 0); FallenShaman step 5 / 6 fall-through; velocity assert
logged and ignored (fatal in 1.14d); a think without AI control logs;
command list linear (`SetCurrentAiCommand` wrap not implemented).

**`world/waypoints.md`**: W1 §6.2 step 2 with no room compares as "no act".
**`world/cube.md`**: C1 §6.4 `exc` / `eli` as if-else-if; C2 §7.3 `mod` /
`useitem` output with no captured item makes nothing; C3 tempered rolls
skipped when the duplicate failed; C4 more than 18 fillers dropped; C5
type pick with 0 items returns 0 without a draw; OQ 1, 2, 4–8 untouched.
**`world/quests.md`**: Q1 §2.3 active / state bytes (only A1Q1 known; row
40 filter OQ6 taken as 42); Q2 §10.1 sequence functions raise 0 → 1 only,
chains 8, 18, 22, 31 `unhandled`; Q3 chat end sends status 1 only after
the start message; Q4 event 3 bits 3 / 4, A1Q2 area level 17 is D2MOO's;
Q5 OQ7 per-quest iterates `unhandled`, `set_status_all` sends 0x5D to every
player; Q6 §6.3 false status function → record's status byte; Q7 OQ10 0x61
byte and intro-flag act from D2MOO, Tyrael's 0x5D before 0x61 `unhandled`;
Q8 §8.4 refusal sound also on failure; Q9 §10.3 respec "active byte" =
chain 30; Q10 §10.5 / §10.7 / §10.8 unspecified items `unhandled` (A1Q2
timer 15, A1Q5, A1Q6 timer 1, Malus refusal sound, event-0 text, Act II–V);
Q11 OQ5 stone-order 0x50 layout (order computed, message `unhandled`); Q12
OQ4 entry mode single player uses (caller passes).

**Second set (notes `impl-skilluse-vitals`, `impl-monster-*`, `impl-drlg-*`,
`impl-vendors`, `impl-npc`, `wire-*`; same rule: narrowest reading, each
has a `TODO(<spec ref>)` at its site, none confirmed on 1.14d), by owner
spec:**

**`skills/use.md`** (tags `TODO(use.md §1 rule 1)`, `§1 rule 2`, `§2`,
`§2 step 3`, `§4`, `§4 step 4`, `§5.1`, `§5.2`, `§5.3 step 1`, `§5.3 step
6.4`, `OQ7`, `OQ9`, …): §1 rule 2 result codes for a bad unit type (≥ 6) and
a failed distance test (`MsgResult::Unspecified`); §1 rule 1 no resync when
the unit has no player data; §2 the order of the `use_state` parts and the
entry lookup (D2MOO `SKILLS_GetSkillById`) are behind the seam; §4
`can_change_mode` past KB says no, state 42 without a stat list reads stat
164 as 0; §5.1 skill mode for unit types other than player / monster uses
`anim`; §5.2 / OQ7 one running index shared by frame codes 1, 2, 4; §5.3
step 1 "stop" returns 0 without the neutral reset, step 2 corpse rules for
monster targets only, step 6.4 no target position fails the line check;
§5.4 step 2 "level > 0" read as the skill level with bonuses, step 7
"valid missile" = `srvmissile ≠ 0xFFFF` and a `missiles` row, step 5 an
`ItemEffect` index past the table is an empty slot; §7 type 8 with arg1 0 or
< −1, type 9 reading of stat 151 (caller passes skill and level), type 5 do
arguments (charge 1, item 0, aim 0). Monster mode starts (`0x005A75C0`,
`0x005A7670`) are the monsters branch's (OQ6).
**`combat/vitals.md`** (`TODO(vitals.md §1)`, `§2`, `OQ2`, `OQ3`): §1 table
write order, `threshold(class, target)` = experience of level `target + 1`
under §4.1 (literal); §2 `gain_energy` "if `n > 0`" covers the current mana
only; OQ2 §4.3 only as far as creation needs (cap, level-up, event 12),
`lastexp` (stat 29) and the kill gain not written; OQ3 operand roles of the
`pct` branch of §4.2.

**`monsters/init.md`** (`TODO(spec: monsters/init.md open question 6)`,
`open question 7`, `§24`): §14.2 boss mods only bloodraven (OQ6); umods
17, 18, 23, 25 read "as fire", mana drain ×256 on the added value, umod 26
body unread (monteleport level 1 mode 4, AI flag 0x20) (OQ7); §24 how an
all-zero component field is signalled (`components_field` returns `None`),
header and field order around the init-owned fields not built; readings:
`monster_playercount` = the §9 n, superunique step 2 difficulty picks only
inside the "fewer than 5 umods" branch, mode-1 callbacks read the mode after
the change, umod 41 re-schedules only when alive, umods 38 / 39 add the same
delta to maxhp and hitpoints, +0x5C bit 2 from create flag 0x08; callbacks
other than §22's five bodies log `Unhandled::Callback` (OQ8); first AI setup
draws (OQ2) and allocator draws (OQ3) are the providers'.
**`monsters/population.md`** (`TODO(spec: population.md §6.3)`, `§6.3 r4`,
`§11.4 r6`, `§11.5`, `§10.3 r1`, `§3.1`, `§4`, `open question 1`, `5`, `6`):
§6.3 boss creation mode taken as 1; §6.3 r4 with a GUID the fallback places
in the room `0x0064E840` returns; §6.5 r4 / §11.4 r6 hcIdx 60 `0x0058F030`
arguments (dedicated seam methods); §11.4 r6 hcIdx 10 (Radament) seed of
`roll(5)` and mode (boss unit seed, mode 1, `place_near` r 4 flags 0x40);
§11.5 r4 seed of the event-7 `roll(50)` (seam `schedule_monumod`) and ids
absent from the TSV (0, 1, 6, 7, 9, 12–16, 19–21, ≥ 33) spawn nothing; §10.3
r1 same calls without the SetBoss condition; §9.2 / OQ5 n = 1 tile record
tests no tile; §4 / §3.1 null region means no pick / no population; §2.4 r1
`(1 << (T & 31)) − v` compared as i32; §13.2 / OQ6 old alignment 4 excluded as
written; §3.2 density compared as i64; difficulty index clamped to 2 for the
levels columns; OQ1 `0x0052D0F0` exposed as `populate_once` (without the
ambient call).

**`drlg/preset.md`** (`TODO(preset.md §6 step 10)`, `§6 step 9`, `§6 step 7`,
`§5.2 step 6`, `§5.3`, `§11`, `§3.2 step 4`): `0x0066B970` list position read
as head insert (from `rooms.md` §9.9); which step writes the level preset
info's map field (set in `generate` after the direction sync); door flag 0x20
when `roll(3)` gives 0 or the position is outside the room (reported by
`DoorOutcome`, caller decides); out-of-grid waypoint cells skipped; negative
DS1 act is `PresetError::NegativeAct`; v < 7 orientation beyond 41, item
ids ≥ 1, door unit types other than 1 / 2 are errors (none in lvlprest data);
pop and tile-info capacity unbounded; link-grid layout (indexed like the §6
cell grid) is the contract with outdoor; spec OQ3 `add_preset_units` before
`room_grids` (`Ds1NotLoaded` otherwise); not implemented: §3.2 step 4 client
automap callbacks, §12 run-time pops, type-2 tile free `0x00666610`, DS1 tag
layer and groups.
**`drlg/maze.md`** (`TODO(spec: maze.md §5.5)`, `§2.4`, `§3.7, rooms.md`,
`§6`, `§7.1, rooms.md`, `§9 step 4`): §5.5 spiral file timing read as after
each branch (later merge re-pick sets file −1 again); §2.4 link list order
kept in creation order; §7.1 cross-level link target taken as the level;
§9 step 4 which built room gets the links and what a link to an
already-built cell resolves to (delegated to `MazePresets::build_map`); §6
levels a per-type table does not name draw r and stamp nothing (lair levels
other than 62–64, Act 2 sewer levels other than 47–49 and 65); §3.7 probe
link removal; `reset_level` (keep = 1) keeps the lvlmaze record and clears
the rotation list, `free_level` drops both; infinite loops kept (grow tree
loops while the room count is below target even if no cell can grow).
**`drlg/outdoor.md`, `outdoor-tilesub.md`** (`TODO(outdoor.md §1.4)`, `§3
step 2`, `§11, OQ 9`, `§12.2`, `edge case 2`, `outdoor-tilesub.md §2.3`,
`§4.1`, `§4.4`, …): not built, no draws: Act III jungle attach points and
per-level jungle ids (§9.1, OQ 7: Act III creation draws fewer times than
1.14d), Kurast and Travincal (§9.3, OQ 8), Act V beyond the siege strip
(§11, OQ 9), Act I path floor (§7.5.3, OQ 6); neighbour list order head
insertion (§1.4); outdoor room list order head (§12.2); polygon merge pair
(last, head) not merged (§3 step 2); blank corners column pass read as the
transposed row pass (§6 step 5); grid path root counted toward the 900
nodes (§7.5.1); desert cliff rows order ascending (§8.3); zero pieces
(straight border piece 0, desert pair (0, 0), style-map P ≤ 0) not stamped,
border flag of the §6 stamps clear; `RandomDS1` scans every path cell (§5.4);
small / odd sizes read as nothing / 0; reset (`0x006754C0`) keeps flags and
neighbour entries; unallocated levels skipped in adjacency / neighbour
passes, more than 6 path starts kept; room grid edges |= 0x4 on all four
sides; Act III jungle blocks kept in tiles; the outdoor tile fill has no
shadow pass (`OutdoorRoom::shadows`, `roof_count`, `units` kept for
`preset.md`).

**`world/vendors.md`** (tags `TODO(specs/world/vendors.md §…)`): V1 §7.2
rule 7 mask at `0x006CE270` unwritten (`unique_nosell_mask` = 0); V2 §9.2
rule 4 an empty affix slot adds no delta; V3 §9.2 (A) / (B) a layer without
a `skills` row is skipped / reads (0, 0); V4 §9.2 (B) encode 4 read as the
two packed fields of `stats::by_time`; V5 §9.2 rule 6 "cost/2" = the
socketed item record's `cost` / 2; V6 §9.4 an item without a normal-code
record prices 0, uber / ultra test literal "≠ 0 and ≠ `0   `" (the store
upgrade uses "≠ 4 spaces"); V7 §3.1 an upgrade code missing from the code
map keeps the base code, a null creation counts as a failed try; V8 a
permanent-list code missing from the code map is skipped without a draw;
V9 §5.1 no gamble index → no list, an index past the list ends it, `rin` /
`amu` missing keep the drawn id, in an expansion game a missing record is
passed to creation; V10 §7.1 rule 2 and every 0x2A whose GUID the spec does
not write: GUID −1; V11 §7.2 rule 8 quantity := max stack on every restored
copy; V12 §8.1 handler results only for rule 4's first refusal (3); V13
"flag 1" / "flag 2" are the item-flag values 0x1 / 0x2; V14 NPC inventory
order for §4 rule 3 is the creation order in `VendorRecord::store`; spec
OQ1–7 untouched (OQ5 surfaces as `PriceFatal`). Tool: the coverage `r0`
issue (§2 step 6).
**`world/npc.md`** (tags `TODO(npc §…)`): N1 §2 start rule 1 failures other
than `0x00457490` and "already in the list" return 0; N2 an NPC without an
interaction list cannot start; N3 §5 step 5 a pet's "life to max" counts as
a change only when below max, removed state lists count as changes; N4 §7.3
steps 1, 3 a missing record answers code 9 before the level cap, a seller
without a Normal row or hire list answers 9; N5 §7.3 step 5 "player level"
is stat 12 uncapped; N6 §7.5 no row for the difficulty, or no slot offered
and not hired: nothing; N7 §8.1 Socket a failed duplicate refuses before
the removal, a failed removal after a good duplicate refuses and leaves the
duplicate; N8 §8.3 order act completion → act change → waypoint (from
`quests.md` §8.1), meshif1 / tyrael2 act change argument 0; edge case 6
(personalize, failed duplicate) stops after the refusal (OQ4); 0x2A bytes
3–6 and 0x58 byte 6 written as 0; `0x00576770`'s `first` argument accepted
and unused; 0x2F / 0x30 use `unit_by_guid`, 0x13 / 0x34 / 0x36 / 0x38 / 0x62
use `monster_by_guid`; §10 (dead code) not implemented on purpose.

**Wiring** (`wiring::action`, tags `TODO(ai.md §1.2)`, `TODO(missiles.md
§R6.1)`, …; `wiring::economy`, tags `TODO(cube.md §7.3 vs affixes.md §9)`,
`TODO(properties.md §4.2, stat-lists.md §4.1 / §8.1)`, …):
- W1 `MonsterDispatch` runs type 10 for frozen monsters, `tick.md` §5.6
  drops it; the combined dispatcher follows `tick.md` (I3).
- W2 `missiles.md` §R6.1 `avoid` / `block` arguments of the block / dodge
  call and hit flags from missile data flags 1, 2 not stated, neither
  applied (no block / dodge draw on missile hits: a draw-order difference
  to 1.14d until settled). W3 §R6.2 where the 103 / 104 / 106 bypass flags
  go in the damage record (no field in `damage.md` §1), not carried; the
  "rolled" hit flag (0x20) is not set by the missile roll, so event 6
  `domissiledamage` never fires on missile hits. W4 missile crit → result
  bit 0x2000 read as the record's crit.
- W5 `0x0064D9B0` / `0x0064EBA0` footprint: the single sub-tile is read /
  cleared for every size. W6 `0x00641CB0` search order: rooms in adjacency
  order, units in room-list order, filtered by position. W7 `0x00648EB0`
  with a moving path: the cached word is the path's (pending), without one
  the grid at the current position.
- W8 state lists (`justhit`, `create_state_list`): allocation flags and
  attach `reset` not stated (flags 0, reset 1), `justhit` list owner = the
  hit unit. W9 AI `change_mode` "failed" = the mode set returning an error
  (`0x005A7C20` falls into the neutral start itself). W10 AI life %
  `100 · life / max` truncating. W11 `knockback_to_gethit` last-hit class
  160 not stored. W12 `calc-expressions.md` §3.5 `stat(s, mode)`: mode 1
  base, other modes total (stats OQ5).
- W13 client level change `0x00543B90` / `0x00537340` not specified: a change
  between acts is not handled. W14 `0x00619E50` vs `0x0066B2B0`: the DRLG
  search (which streams) is used. W15 missiles do not count toward a room's
  allied count (`unit-order.md` §5.2). W16 the game seed of `rng.md` §5.3
  lives in `ActionHooks::game_seed`.
- WE1 `find_list` ("by state and flags `0x006257D0`", `stat-lists.md` §9.3)
  read as the by-flags query (parked chain when 0x2000 is asked, else the
  active chain): first list whose state equals the key's and whose flags
  hold every asked bit. WE2 `list_for` ("created if missing",
  `properties.md` §4.2): the key's flags, expire 0, the unit list's owner
  type and GUID, state := key state, attach with reset 1. WE3 `units.md`
  §3.1 step 8 "flags bit 1" = 0x1 (drop and cube requests pass init flags 1).
  WE4 replenish event 3 arguments: scheduled with (0, 0). WE5 drop request
  source unit (`treasure.md` §7 step 4) left none (affects `generation.md`
  §6.1 class skill mods). WE6 `0x005C1BC0`: `affixes.md` §9 (two §5 rare-name
  picks, no arguments beyond the item) vs `cube.md` §7.3 (`0x005C1BC0(item,
  prefix)` once per side) disagree: not wired. WE7 `0x00660240` expansion
  argument not passed. WE8 which of the player's items `has_item` searches:
  the rest's inventory list. WE9 reward creation `0x00559CE0` and level
  default `0x00558200` left as a seam. Reader choices: unit total for
  monster `level` and `monster_playercount`, MF 80, GF 79, the request
  unit's level, the quest `stat`; base for player `level` (treasure §7 step
  3) and gold stat 14. A missing unit record: `player_class` reads 0xFF in
  the cube and 0 in the quests; `item_seed` / `unit_seed` panic.


**Third set (notes `server-items`, `server-world`, `server-skills`,
`wire-worldgen`, `wire-interaction`, `gaps-*`; same rule: narrowest reading,
none confirmed on 1.14d; the handler, worldgen and interaction items have a
`TODO` at their site, the gap-test questions are in the notes), by owner
spec.** Where a note says a question is the same as one above, it is not
repeated.

**Integration (notes or specs disagree, not decided here):**

- J1. Two unit stores and three host fields. `ItemWorld` (item handlers)
  owns its own `Units` / `StatLists` while `ActionSim` owns another pair; a
  wired single-player host must merge them (`server-items.md` §4 point 5,
  a `TODO` on `ItemWorld`). `SimGame` got `items`, `world: W` and `skills`
  from three parallel branches; the world note says to fold them into one
  host trait if they also add a type parameter or bound (`server-world.md`
  §6; §8 lessons).
- J2. `SimGame::tick` runs the default `Steps` hooks rather than
  `ActionSim`'s `TickHooks` (rooms): unchanged, "worth a look" when the
  wired host is assembled (`server-world.md` §6).
- J3. Where the skill-use events are routed. `server-skills.md` §5 items 1
  and 2: `ActionHooks` routes neither `player_action_frame` (`0x00580460`)
  nor timer types 5, 8, 9, 12 to `use_::{attack_frame_event,
  active_state_event, periodic_event, item_aura_event}` / the cooldown
  expiry (0x3C schedules the type-8 timer, its handler does not run), and
  says the routing likely belongs in `d2-server` or a `UseWorld` provider
  in `d2-sim`; `wire-interaction.md` §6 proposes routing events 5, 8, 9, 14
  in the action wiring's `impl UnitHooks for ActionHooks` through
  `UseView` (needs `X: UseRest`).
- J4. Two providers of the skill-use seams: `d2-server`'s `skills::World` +
  `SkillSeams` (`server-skills.md` §4) and `d2-sim`'s
  `wiring::interaction::UseView` over `CombatView` both implement
  `SkillUnits`, `ManaUnits`, `SkillFunctions`, `UseMissiles`, `UseWorld`
  and `VitalsUnits`; neither note mentions the other.
- J5. `npc.md` §8.2 says `0x00570360` resets stats and `0x00570C80` skills;
  `vitals.md` §2.1 says `0x00570C80` is the stat reset. `reset_stats` /
  `reset_skills` are not wired until a spec session settles it
  (`wire-interaction.md` I3, WI3).
- J6. Mode starts and the first think: the wired action sim routes no
  AnimData record, so `units::modes::animate` fails with
  `AnimError::NoRecord` after the mode set (`server-skills.md` §5 item 1),
  and nothing in init or AI schedules a created monster's first think
  (`wire-worldgen.md` WG8); both notes leave "which step starts the mode"
  to a spec.

**Fourth set (notes `quest-host`, `e2e-next`, `gaps-*`; same rule), by
owner spec.**

**Integration (from `e2e-next.md` §5, now visible in one run; not
decided here):**

- J7. Two unit worlds and three item stores in one game. The cube runs on
  `ItemWorld` (own `Units`, `StatLists`, `GameFields` with a second game
  seed, `ItemStore`) while the action sim owns the units, and
  `DeathDrops::items`, `TradeWorld::items` and `ItemWorld::items` each hold
  a store with its own `ItemTables`; an item made by one is unknown to the
  others (the dropped gold cannot be sold or cubed). Extends J1. Fix in
  §2 step 7c.
- J8. The player's interaction (+0x64 / +0x68) has three owners after one
  e2e run: `Rest::interact` (Akara), `TestPending::interact` (waypoint),
  `ItemWorld::staged.interactions` (cube); extends `e2e-vendor-host.md`
  §4.1.

**`world/quests.md`, `world/npc.md`** (quest host; `TODO` at the sites):
QH1 `Desk::quest_message` (`d2-sim`) and `TradeWorld::quests` each carry
the mercenary-reward loop (the host takes a generic `QuestCall` and cannot
call `Desk::quest_message`): should it become a generic `Desk::quests(|ctl,
w| …)`? Left as is, `d2-sim` untouched. QH2 `unhandled 37 0x58f870` (chain
37, Act I intro, event 11) fires on every 0x31: `wire-open-seams.md` has the
open question of its order against the mercenary reward; the other
`unhandled` calls the tests reach are the Act I event-0 callbacks
`0x58f8f0`, `0x595e20`, `0x594c50`, `0x592580`, `0x5916a0`, `0x590b10`,
`0x58ff90` (`quests.md` §10, not written), so the text list after Akara's
message 64 comes out empty. The 0x40 result 0 stays a reading ("the spec
names no result"). EN1 (`e2e-next.md` §6): where in game creation the
NPC-control and quest seeds are drawn relative to the regions and the
object-control seed (the e2e puts them after `create_regions`, before the
player's items; only `NpcControl::new` → `QuestControl::new` is written,
§2 step 7b); settles the monster seeds of the e2e run against a recording.

**Gap-test questions, fourth set** (the four later `gaps-*` notes; GD1–GD7
above are not repeated):
- `data/*`, `sim/rng.md` (GD8–GD11): GD8 `patch-layers.md` §8 report order
  "B (table)": `patch.rs` `Finding::sort_key` class 2 sorts by code before
  table (B03 `aaa` and B01 `zzz` come out B01 `zzz`, B03 `aaa`); a one-line
  code fix if "(table)" means table first (owner: `patch` module). GD9
  `patch-layers.md` §11 order independence: the valid layer `set #2 ax2 lvl
  5 -> 6` / `set #2 ax2 code ax2 -> ax3` gives A05 when reversed (the row
  label differs after the rename), which §4 implies but §11's list of
  allowed failures does not name. GD10 `loading.md` §8 automap names vs
  `bin.rs` `post_load_check` (empty or `0…` names; check queued, §5 C21).
  GD11 `sim/rng.md` §5.2–§5.5, §6, §7 are one unit each (cross-reference
  tables), so no test can claim them honestly; proposal: number the rows.
  Two more deviations of the same note extend GD items instead of adding
  new ones (GD1–GD7 above): `loading.md` §7.3 also omits `setitems` → `sets`, and
  policy r5 (`d2exp.mpq` required) is not enforced by `bin::load`, which
  sets `lod = set.has_archive("d2exp.mpq")` and loads a classic set
  without error, skipping `expansionstring.tbl` and the hireling check
  (`bin.rs:244`).
- `items/treasure.md` (GT1): §1.2 a lookup "sees only TCs created before":
  can a row name its own TC? The record exists before its items are parsed
  but the code registers the name after them, so a self-reference misses;
  untested.
- `monsters/population.md` (GP1): §1 r2 `0x0052D0F0` "runs the same
  sequence for one room": does that include the §1 r1 ambient spawns?
  `populate_once` (`monsters/population/mod.rs` ~146) leaves ambient out on
  purpose (impl note 14, `impl-monster-population.md`); ties into `population.md` OQ1.
- `client/bridge.md` (GB1): §6 r2 "Today every row is `TBD`" is pinned by
  `every_row_is_tbd_and_no_handler_is_registered`; the session that takes
  the first id must change the spec and the test together; should the
  sentence move to Status.
- Not questions, for the record: `outdoor.md` §2.4 B2 offset index outside
  0..3 stays the `TODO(outdoor.md §2.4)` in `place.rs`; `tick.md` §5.2 r4
  has no caller of `0x005544B0`; `intents-events.md` §3.2 r3 / r4 (empty-game
  timeout, failed-send drop) are not implemented in `Host::flush`.

**`world/cube.md`** (server items; `TODO` at the sites):
SI1 §2 step 1: the failed range-10 test is "non-zero"; d2rs returns 1, the
"out of range" code of `intents-events.md` §2.3 (Ghidra `0x00549350`). SI2
§8 step 1: the spec frees through `0x0055DF10` → `0x00557FD0`; d2rs frees
with `0x00555600` (`units.md` §3.2) after `ItemPending::remove_cube_item`.
SI3 §2 step 3.1: the argument of `0x0044BE50` is not named and S→C 0x3F has
no layout; flag 0x4 is cleared over the inventory list (whether the cursor
item counts is not written), the 0x3F is only recorded. SI4 (`cube.md` OQ1,
OQ2): the bytes of placement, removal and sound messages need the V1 / V22
recordings; sound events (unit +0x6E / +0x70) are recorded, not sent. The
handlers send only 0x77 (2 bytes, byte-exact in the tests) to the acting
client; a send to another player is an `ItemError`. No default behaviour is
invented for `ItemPending` (inventory pass `0x0055FA40`, placement
`0x00560200`, the 0x9D and removal part of `0x00564F30`, socketed list,
`duplicate`, `tempered_affix` (WE6), `drop_runeword_stats`, `repair`,
`recharge`, quest item hooks, the Cow portal): without a provider 0x2A and
0x4F stay stubs.

**`sim/intents-events.md`, `skills/use.md`, `skills/levels.md`,
`world/*`** (server skills and world; `TODO` at the sites): SK3 `use.md` §1
rule 2 leaves the codes of a bad type and a failed distance open;
`intents-events.md` §2.4 rule 4 gives 2 and 1, `wired::run` maps
`MsgResult::Unspecified` with that rule (never reached: the dispatcher
refuses first); suggested spec edit: point `use.md` §1 rule 2 there. SK4
`levels.md` §6.4 step 5 ("the handler then calls …") is read as running
after the spend whether or not a level was added, result 0 (OQ5). SK5
`use.md` §4: where the mode starts store the point / unit target is not
stated (`start_mode` keeps none). SK6 the reenter-1 mode set goes through
`units::modes::player_start`, which asks the request-check hook that
reenter 1 skips in 1.14d; the wiring's hook accepts, so the result is the
same today. SK7 cooldown-list attach `reset` not stated (`stat-lists.md`
§8.1): 1, as the action wiring's state lists (the interaction note reads
the same, WI9). SK8 `same_act` for a pair the message did not stage falls
back to the unit records' act (unit +0x18); `within_reach` uses staged
positions, else `SkillSeams::position`. SK9 stale doc of `SimGame::handle`
(§2 step 3). World handlers (`server-world.md` §5): 0x40's handler result
is 0 (the spec names no other); a message whose NPC is missing runs the
trade function on an empty scratch record and a class without a record
reads as an empty record (`TODO(vendors.md design point 1)`); fatal paths
(`NpcError`, `WaypointError`, `QuestError`, `PriceFatal`, sink errors)
end the game in 1.14d, here they are recorded with `WorldHost::fault` and
the handler returns `ResultCode::Malformed` (1.14d ignores the code,
§2.2 rule 5); 0x3E stays a stub: its item checks (exists, the player's,
same act) have no result codes and no `QuestWorld` item seam
(`quests::read_clue` exists for after them).

**`drlg/maze.md`, `preset.md`, `outdoor.md`, `population.md`, `init.md`**
(worldgen; `TODO` at the sites): WG1 `maze.md` §9 step 3 / `preset.md` §6:
the room flags F of the maze's build call are not stated, 0 used. WG2
`maze.md` §9 step 4 (OQ4): DRLG rooms have no orth links, the cells'
init-flag links are not carried (no draw depends on them). WG3 `outdoor.md`
§12.1: the single-room argument of the outdoor preset-cell build is not
stated, multi-room used. WG4 `preset.md` §11: door record flag 0x20
(`DoorOutcome::Placed`) is the tile code's, the seam returns nothing, so it
is not set. WG5 `population.md` §11.4 step 6 vs `init.md` §20 steps 4–5:
the order of the hcIdx extra spawns and the aura re-run / quest records is
not stated, the spawns run first. WG6 `population.md` §9.2: which DT1
header field `0x00604BC0` reads is not stated; no floor record counts as
water, so frog demons find no water point (the `roll(n)` on the room seed
is still drawn). WG7 `0x0064D9B0` footprint by size: one sub-tile read (the
same as W5 above). WG8 see J6. WG9 outdoor generation through the
dispatcher is not exercised (needs lvlsub rows and substitution DS1s;
streaming a chain room generates its outdoor neighbours, `NoSubRows(0)`
without them), so the population tests use a preset level (id 30) placed
apart from the chain. Also left to a spec (no `TODO`): the DRLG data
population reads of §1 3l and `LevelTypes::warp_unit` (`0x0066E1C0`), the
type-2 tile free `0x00666610`.

**`world/npc.md`, `world/vendors.md`, `skills/use.md`, `combat/vitals.md`**
(interaction; the note's I1–I10; `TODO` at the sites): WI1 `npc.md` §3:
"the unit with GUID" names no type, read as the monster hash (a GUID of
another type answers 1 instead of 3 if the original searches all types).
WI2 `npc.md` §2 rule 2: the think's arguments are not written, 0, 0. WI3
see J5. WI4 `npc.md` §8.1: the service passes only the item; the player of
`0x005761C0(item, player)` is read as none. WI5 `vendors.md` §3.1 rule 2:
the allocation flags of `0x00559CE0` are not written, 1 (as drops and the
cube), so the store item is in the unit lists the 0x32 lookup reads. WI6
`vendors.md` §7: the mode set the vendor code calls is not named, unit
+0x10 is written. WI7 `vendors.md` §9.2 Inputs: stats 70, 152, 72, 73,
252–254 read as unit totals, 31 as base; entries of 107 / 204 from the
full array of an extended list, else the base array. WI8 `use.md` §5.4
step 7: the skill missile helpers' record fill (flags, target, `aim`
position) is not specified; owner, origin, class, skill and level are set,
the rest comes from `UseRest::skill_missile_fill`. WI9 see SK7. WI10
`vitals.md` OQ2, `damage.md` OQ7: only §4.2 (the defender's base stat 13
and both levels as unit totals) and §4.3's add are applied; `ExpRatio`,
stat 85, hireling cap, pet credit and party share are not. Observation for
the vendors spec owner (the module's reading): `vendors.md` §3.1 rule 4
repairs a new store item before rule 5 identifies it, and the repair
(`0x005761C0`, §8.2) starts with "repairable" (§9.2 rule 0), which
requires flag 0x10, so a store item keeps its created durability; if
1.14d store items are always at full durability, the repair's identified
test or the step order needs a check (the store recording, §5 A16).

**Gap-test questions** (the four `gaps-*` notes; each names the code's
current reading):
- `items/generation.md` (GI1, GI2): §9 r2, does "Format 0 only" cover only
  the forced socket count or also the flag copies after "Then" (code copies
  flags for every forced request, `TODO(items OQ-G2)` in `create.rs`
  `forced()`); §4 r5, does the §5.3 quest-difficulty step run only when
  "quest" and a request are given, or always (tested only with a request).
- `sim/stat-lists.md` (GI3, GI4, GI8): §6.4 "Consequences" text and edge
  case 2 say per-level stats never reach a player's full array, but rule 5
  does `set-full(d, recompute(L, d))` (after setting 216 = 8 at level 20 the
  full array holds `(216, 8)`; code follows rule 5); edge case 5, "collect
  at most 16 A53 keys" vs "16-slot buffer, no bound check" (code takes the
  first 16); §10.3 is skill 0 valid for event 9 (code treats it as valid,
  TODO; the test avoids it). §9.3 "by state and flags `0x006257D0`" has no
  rule (WE1 above); §edge-cases-original-bugs r4 (an expired extended list
  loops forever in 1.14d) is a deliberate panic in d2rs.
- `sim/units.md` (GI5–GI7, GI9, deviation): §6.1 vs `stat-lists.md` §10.1,
  stamina and mana regen gated on `0x00580610` or all three steps
  unconditional (code follows stat-lists, TODO in `dispatch.rs`
  `player_regen`; the same disagreement as §7 units above); §4.5 what event
  1 in mode KB does when `0x0057EEC0(4, 1, 0)` fails (code: nothing); §2
  "Dead" lists players and monsters, `stat-lists.md` §10.1 adds "any other
  unit type" (code follows stat-lists); §3.1 r3 the quest chain field +0x74
  is not modelled; **§5 r4 against `tick.md` §5.2** (the deviation of §1
  3m: the spec drops the callback of an expire −1 event, `TimerQueue::
  schedule` keeps it; the two specs need reconciling before the tick
  module's fix, which would pass `None`).
- `missiles/missiles.md` (GC1, GC2): R6.1 r3 "armor −= missile stat 120
  (`item_damagetargetac`, clamped at ≥ 0 after adding)": "−=" and "after
  adding" disagree on the sign, the code passes `+stat120` to
  `add_target_ac` (the same as W2 / W3 above); edge case 5, a roomless
  missile "is removed by R4 step 5 on its first collision-active run":
  R4 step 5 (no room → 2) comes before step 7 (Activate), so does
  "collision-active" mean `CollideType` ≠ 0 or run k ≥ `Activate` (tested
  only with `Activate` 0). Also open at `TODO(spec gap)` sites:
  `damage.md` §4.5 step 3 (do leech rows take the difficulty resist
  penalty), §3.1 step 6 (the getter for stats 103, 104, 106), §5.1 step 6
  (the "attacker is an object…" condition read literally); `ai.md` Navi's
  param 1 clamp (tested only 5 → 4), the distance reported for an
  alternative target from slot 9, combat and flags for a forced target
  (OQ6), a mode or move request toward target 0 (passed through).
  Stubs by design, not gaps: AI Npc (32) and the D2MOO-only Act I AIs 4,
  5, 10, 37, 43, 59; missile server-do 2, 3, 5–37 and server-hit 1–59 log
  `Unhandled`; message 0x73 (R2.4) is a real gap (§1 3 not implemented).
- `data/*`, `formats/*`, `sim/rng.md` (GD1–GD7): `mpq.md` §11 has two gaps
  (the implementation returns an error in both): input ending exactly
  between codes with no 0x100 code and the output not full, and the
  increment when `lead` is the root or `n`'s parent; the edge-case
  sections of `dc6.md`, `dt1.md`, `animdata.md` mix behavior with claims
  about 1.14d data; `calc-expressions.md` §4.5 an 8-bit push needs
  `len(out) < 1024`, which allows 1025 bytes (the code follows the spec;
  confirm on 1.14d); `calc-expressions.md` policy r4 says Fail is "a
  non-empty cell gives 0xFFFFFFFF", a whitespace-only cell also reports
  Fail; `loading.md` §7.3 omits `chartemplate` (`class` links to
  `playerclass.code`) and mixes formula-derived links (misscalc, skillcalc)
  with field links, so it cannot be checked exactly against `fields.tsv`;
  `loading.md` policy r5 says `d2exp.mpq` is required but `bin::load` has no
  explicit check and fails later on missing X files (should a classic
  install be rejected up front?); `txt-format.md` §edge-cases "monstats 255
  slots" without splitting columns from missing fields (monstats left out
  of the slot-count check).
- `drlg/*`, `world/*`, `sim/intents-events.md`, `outdoor.md` (GW1–GW7):
  `waypoints.md` §7 r2 / edge case 6 (the same as §5 B "Spec vs test
  findings": the code warps, the test `close_and_validation` asserts it);
  `levels.md` edge r3 / §5.4 (with 9 warp-room centre slots and no bound
  check, which bytes does a 10th centre write, x into y[0], y[9] into the
  count at +0x228; code appends, TODO); `rooms.md` §9.2 text
  (`0x0066F1A0`: which `keep` does the only caller `0x0066B4C0` pass, code
  assumes 0; the same as §5 B); `intents-events.md` §4 r4 lists S→C
  0xAF–0xB4 as out of scope but `server-messages.tsv` marks 0xB1 `none`
  (test follows the TSV); `outdoor.md` §6 r4 ("each doubled unless its
  vertex is a preset link" plus "grows by 2 in magnitude (a, c)": tests
  follow the code, double then grow); `outdoor.md` §8.3 rows omit the file
  (F) and in-row order of the 377 / 378 / 380 / 381 wall and path pieces;
  `quests.md` §7.1 "every NPC id is a valid monstats row" needs a game-file
  check (§5 C19).

**Fifth set (notes `p6-window`, `verify-map`, `wire-routing`, `drlg-data`,
`fuzz-data`, `client-own-gaps`; same rule: narrowest reading, none confirmed
on 1.14d; a `TODO` at each site unless stated), by owner spec.** `synthetic-data`,
`ci-speed` and the two `game-tests-*` notes add no question (their
interpretation points are in §5 C34, C35).

**Integration (not decided here):**

- J9. `d2-server` message paths reach `ActionSim` directly
  (`ActionEvents for WorldSim` returns `&mut self.action`), so a skill
  message handled there runs without the world lent and its monster routes
  answer `Pending` (today a cast only starts a mode; the missile is created
  by the action frame inside a timer event, which is lent). Proposed fix in
  `d2-server`'s file: run the handler inside `WorldSim::lend`
  (`ActionEvents::with_action(&mut self, f)`). Extends J3 (`wire-routing.md` §4).
- J10. `wiring::economy::death.rs` reads `h.x.monster_flag` /
  `h.x.superunique` directly and should call `ActionHooks::monster_flag` to see
  the lent world's flags; `QuestTick`'s `UnitSide for WorldSim` hands out
  the action hooks without lending (quest code that allocates or removes a
  monster would miss the world state). Settles with J1 / J7.
- J4 (existing; `wire-routing.md` §6 proposal, left alone): keep `UseView`
  as the one skill-use provider (it is what the timer events use, J3);
  `WiredSkills::run` builds it through `ActionSim::skill_use` (inside
  `WorldSim::lend` for a world host); the parts only the server has (the
  staged positions, player data +0x168, `same_act` / `within_reach` of SK8)
  move into the seam value's `UseRest` / `Pending` methods, so `SkillSeams`
  becomes that value and `d2-server`'s `handlers::skills::World` and its
  duplicate impls go. One owner: the `d2-server` skills session, after
  `WorldSim::lend` (this fold).

**`monsters/init.md`, `sim/units.md`, `combat/damage.md`, `monsters/ai.md`,
`skills/use.md`** (`wire-routing`; each has a `TODO` at its site):
- RT1 (`action/units.rs` `View::monster_set_mode`; `init.md` §22, `units.md`
  §4.6): where in `0x005A7C20` the two dispatcher calls (`0x005A4350` mode 0,
  `0x005A4360` mode 1) sit is not stated. The mode-1 callbacks read the new
  mode, so they follow the start function; both run after the whole mode set
  (start, animation, schedule), mode 0 first, so a type-7 event they
  schedule follows the mode's animation events in the timer queue (decides
  the run order of events on the same frame). Settled by §5 A RT-R1.
- RT2 (`action/ai.rs` `monster_level`; `ai.md` §2.4 step 2): the getter for
  the "level" of the teleport heal is not named; code takes stat 12 (`level`)
  unit total (`init.md` §7 rule 4).
- RT3: umod mode 4 (`0x005A43A0`, called from the reaction `0x0057CEE0` at
  two sites) is not placed by `damage.md` §7.1 (call level only, OQ3); stays
  inside `Pending::reaction`.
- RT4: monster data `dwAiState` (+0x54) has no writer in any spec (`ai.md`
  OQ5) and `MonsterData` has no field for it; `Pending::ai_state` stays.
- RT5: `is_boss` `0x0063E9F0` (its test is not specified), superunique
  `0x005A03A0` (reads "hcIdx ≠ −1", but `init.md` does not state the value
  of +0x26 for a non-superunique; `MonsterData` defaults it to 0, a real
  row) and minion owner `0x0058F0D0` (reads monster data +0x2C, `ai.md`
  §3.1, while `WorldState::owners` is written by `0x005DD330`: one field or
  two?) are not routed.
- RT6: timer type 14 / `0x00554570` and the delay list's remove callback
  `0x0056E900` (`use.md` §6: type 14 is never scheduled in 1.14d, neither
  body is specified).
- WG8 / J6 (first think of a created monster, which step starts a mode)
  stays open: `type_init` sets no mode, so an action-allocated monster
  thinks only once something changes its mode or schedules its think.

**`world/rooms.md`, `sim/tick.md`, `client/bridge.md`** (`p6-window`):
- PW1 (`rooms.md` §7.2 / §8, tick step 9): should removing a room that still
  holds units be possible? `free_room` documents "its units must be gone",
  but step 9 freed the player's room (clientless in the DRLG's view, a join
  with `Some(room)` skips the client room switch, `rooms.md` §4.1) and the
  next client update panicked on the dangling room (`units/lists.rs` `r()`,
  "linked room exists"). If 1.14d cannot reach this, the sim should report an
  error instead of panicking; if it can, the units' fate belongs in the spec.
  Latent: `bridge/local_tests.rs` joins with `Some(rooms[0])` (≤ 2 ticks).
- PW2 (`render-pipeline.md` §A9): presentation stays as in `p6-world-view`
  (sprite on layer 31, own camera, integer scale) with the node writing the
  texture before the camera driver; is a dedicated present pass (no sprite)
  wanted?

**`render/composition.md`, `client/render-pipeline.md`** (`verify-map`):
- VM1 (§B2; same as the Phase 6 entry below): the map reference paints index 0
  opaque black (`map-preview.md` §Palette shading) while the compositor maps
  it through the palette; if an act palette's entry 0 is not black the
  `map` case FAILs on the RGBA comparison with exactly the background pixel
  count while indices agree (test `index_zero_question_is_visible`). The
  local run (§5 C28) shows which.
- VM2: `GpuJob` / `Wgpu` cap the atlas at 4 pages per job
  (`verify::gpu::MAX_PAGES`); a chunk whose tiles need more fails loudly
  (`GpuError`); 1024² chunks should stay far below 16 M texels; if a map hits
  it, lower `CHUNK`. Decisions logged in PLAN: case-file format stays
  version 1; chunks of ≤ 1024² (per-pixel results do not depend on
  chunking); `--perturb` corrupts indices and RGBA at the same pixels so
  the count is exactly N on all three comparisons; frame ids dense in
  insertion order, store append-only.

**`drlg/rooms.md`** (`drlg-data`): DQ1 §9.3 names the fixed library with
`Tiles` (mixed case) and says the provider resolves paths case-insensitively;
the archive lookup is, so `archive_name` keeps the case, but the DT1 map is
keyed by the exact string, so a request that differs only in case from a
loaded key is not found (none does today).

**`data/runtime-maps.md`, `data/patch-layers.md`** (`fuzz-data`):
- FZ1 (`runtime-maps.md` §2 / Edge cases): by §2 as written a cycle of
  `equiv1` links that does not contain column j never ends (each pop pushes
  one entry, so the "> 124 entries" test never fires); 1.14d would hang at
  load on such a table. d2rs reports a load error after n² × 128 pops. Add it
  to the "Out of range in 1.14d … d2rs reports a load error" list (with the
  budget) or state another rule.
- FZ2 (`patch-layers.md` §9 diff round trip): D06 checks key uniqueness only
  within the edited table, but A12 checks the key's scope, which can span
  tables (`items.code`: weapons, armor, misc); an edited table whose new or
  renamed key exists in another table of the scope gives a diff that A12
  rejects, so "applying the output after B gives rows X exactly" does not
  hold for that X. Should D06 check the scope (needs the other tables) or
  the round-trip claim be limited? The property asserts the round trip only
  for stacks that applied without an error.

**`client/audio.md`, `assets.md`, `ui.md`** (`client-own-gaps`; **two design
decisions for the user**, §2 step 7g): CG1 should a prefetched pool entry
rank below every used entry (always evicted first) instead of "last used by
the previous frame"? Either keeps the image unchanged; it only changes
stalls. CG2 should `TextOpts` carry the clip rect now (it is on
`TextRequest`) or wait for `ui/text.md`? Not decisions, for the record: the
glyph lookup is by the record whose `code` equals the unit; two records with
one code is `AmbiguousGlyph` (which one the original uses is not
specified); `WavDecoder` has no implementor (`TODO(spec: formats/wav.md
§B1)`), a `SoundBank` needs the sound id → file map (`audio/sound-table.md`
§B3), `TextRules` has only `NoTextRules` (`ui/text.md` §B3), prefetch wiring
and the Bevy task belong to `app.rs`.

**Sixth set (notes `impl-inventory`, `impl-moves`, `impl-path-place`,
`s2c-builders`, `proto-bits`, `conformance-harness`, `prop-sim-core`,
`prop-worldsim`, `fuzz-server`, `game-tests-*`, `render-*`, `p6-integrate`,
`p6-world-view`, `e2e-*`, `wire-open-seams`; same rule: narrowest reading,
none confirmed on 1.14d; a `TODO` at each site unless stated), by owner
spec.** `spec-bodies-*` add no question (their missing text is §2 step 12).

**Integration (not decided here):**

- J11. `items::inventory` and `items::moves` were written in parallel from
  one spec: both define the constant modules `mode`, `page`, `cmd`, `iflag`,
  `ty`, `stat` and an item-data model (`InvItem` vs the `MoveUnits` item
  getters), and two seam traits (`InvWorld`, `InventoryOps` + `MoveUnits`).
  Step 7i's adapter has to map one onto the other; decide which module owns
  the constants and the item record, and drop the copy.
- J12. `path/mod.rs` declares only the four modules of `impl-path-place`;
  `impl-path-core` (§1–§6, the `CollisionView` provider) and `impl-walk`
  (`pathing.md`) were parallel sessions whose code is not in this tree: the
  coordinator unions `path/mod.rs` when they land (`impl-path-place.md`
  header). Until then `place_seams::{CollisionView, PlaceHost, LevelView,
  WarpTileView}` have no implementor.
- J13. S→C bytes now have four makers: `d2_proto::generated` (TSV layouts),
  `d2_proto::s2c::messages`, `items::moves::layouts` and `d2-sim`'s own
  builders (`world::npc::{transaction, service_result, resurrect_message}`,
  waypoints 0x63, cube 0x77, the 0x0D of the travel). 0x0D `PlayerStop` is a
  type in both `generated` (fields `a`, `b`, `life_pct`) and `s2c::messages`
  (the recorded widths); the TSV layout now carries the same widths. Pick
  one maker per id (step 7k).

**`items/inventory.md`** (`impl-inventory` §5; `TODO(spec: …)` at the
sites): IV1 `0x0063C180` (set cursor): does it link the item into the item
list? Only the field is written (`Inventory::set_cursor`); §1.4 rule 1
implies the cursor item is linked but not counted. IV2 §2.4 step 5: the
result when the link check fails (code returns 0). IV3 §2.4 step 3: where
the page-2 trade hook runs ("afterwards"; code: right after a successful
placement). IV4 §3.5: a similar column with no empty slot below `numboxes`:
try the next column (code) or go to the autobelt rule? B1 does not decide
it. IV5 §3.8: compaction sets *item* flags 0x400 and 0x1 (as written; 0x400
is also the PutInBelt *command* flag of `item-actions.tsv` row 11: which
field?); items that do not move are not flagged (code). IV6 §4.7 step 1
"type ≠ 38": primary type (code) or the equivalence test? Step 2 "an
equipped hand weapon": which hand (code: right, then left). IV7 §1.3: the
grid record of an item-owned (socket) inventory and of other owner types
(code: none). IV8 §4.4 step 6 for an object owner (code: no). Not modelled:
placement into an inventory other than the owner's (the optional inventory
of `0x00560200`; the socket case goes through `InvWorld::link_check`).

**`items/inventory.md` §6–§11** (`impl-moves` §5; M1, M2, M5 and the seamed
OQs carry `TODO` at their sites, the rest are readings recorded in the
note): MV1 §6.2 a row whose flags (and condition) match but whose `to`
excludes the client ends the walk (nothing sent, later rows skipped); a
failed condition means "row does not match" (settles with R1 / R3 on two
clients or `0x005973F0`). MV2 §11 the dispatcher's flag argument to the bit
stream (0 used) and the owner fields and flag argument of a filler's 0x9D
action 0x13 (`filler_owner` seam). MV3 §6.2 rows 18–19: the 0x7D `state` =
item flags & the row's flag; §9.2: "quest items → 0 (never)" is the stored
expiry 0, not frame + 0. MV4 unwritten failure results read as "nothing"
(result 0, out 0): §7.7 `0x0063E490` without an item; §7.8 E not in mode 1,
failed placement of N, E's unlink; §7.10 target not in mode 0, the "link"
step; §7.16 the link; §7.17 no hireling; §7.19 the mode / filler / socket
checks; §8.1 r5 a failed `0x00562E00`, r7 the link; §9.3 the unlink; §10.2 a
failed pile creation (stops); §7.23 a failed copy. MV5 every "of type T" test
uses itemtypes equivalence (the spec states it only for 0x26 / 0x61
potions); §7.12 max stack from dst, both quantities announced dst first;
§7.16 both items join the update list (needed for "both send 0x9C action
0x10"). MV6 §8.2 a refused pickup ends the routine (no pickup sound); §10.1
the rest pile's sound is part of the `rest_pile` seam; 0x26 is 13 bytes
(`client-messages.tsv` `partial`), bytes 9–12 unread. MV7 open in the spec
and seamed: OQ1 (item bit stream), 6, 9 (§6.1 r4 not implemented), 10–17,
19; `moves` leaves the `MovePending` groups of its note §4 (rooms, stat
lists, quests, item use OQ14, sockets, hirelings, transport) as no-ops.

**`sim/path-placement.md`** (`impl-path-place` §4): PP1 §7.3 rule 3 / edge
case 4 "the room the search passed": read as the candidate cell's room (the
search's current hint; the vectors use one room); settle on `0x0064DEA0`'s
call to `0x0066A670`. PP2 §8 rule 3 "n + 2 < 2 → the cell's grid value": the
unmasked value (§4 rule 2) of the cell room; settle on `0x0064CEB0`. PP3 §8
"inside the rect's rows / columns": half-open (as `levels.md` §8 rule 2).
PP4 §11 game entry and level warp: the `size` argument `0x005394A0` /
`0x0053AEC0` pass to `0x0061B060`, the `flag` game entry passes to
`0x00554850`, and what either does when no spawn room is found (callers pass
the size; no spawn → `Ok(false)`). PP5 recipients of the 0x07 / 0x15 of game
entry and the 0x0D of warp arrival (§10 rule 6 names the player's client
only for its 0x07; R2 shows ten 0x07 at game entry, §11 names one: the other
nine come from code the spec does not cover). PP6 §12.2 rule 5: the
walk-out target uses the point of rule 2, not the point `0x00554EA0` may have
moved to (same unless the placement search moves it). PP7 §12.2 rule 3
source / destination levels come from the `warp_destination` seam
(`0x006195A0`, spec OQ3) and `quest_gate` (`0x00545B80`, OQ5); §10 rule 7
(position history) is wall-clock and kept out of `d2-sim`.

**`sim/intents-events.md`, `sim/server-messages.tsv`** (`s2c-builders` §5,
`proto-bits`): SC1 §6 says nothing is ignored by default; the unwritten bytes
(0x2A bytes 3–6, 0x58 byte 6: stack contents in the 1.14d builder) must be
named as the only masked bytes of the exact comparison (`s2c::ServerMsg::
UNWRITTEN`; `world::npc::{transaction, service_result}` already write 0).
SC2 layouts the sim needs first, all `partial` in `s2c::AUDIT`: 0x15
(sender `0x0053BC10`, §7 q8), 0x51 (`0x0053BD10`), 0x5A (`use.md` OQ9),
0x27's 34-byte text list (`0x00661480`), 0x50's mercenary form, 0xAC's bit
positions (`init.md` §24), the item bit stream of 0x9C / 0x9D
(`inventory.md` OQ1); each moves to `built` with a type and a vector from a
recording. SC3 the bridge's `bridge-dispatch.tsv` owner rows are still all
`TBD`; they can now name an `s2c::Message` variant. SC4 see J13 (two
`PlayerStop` types; field names `a` / `b` / `life_pct` of the TSV vs the
recorded-widths names). PB1 (`proto-bits`): whether 1.14d's 0x96 builder
clears bit 71 is not in the spec; the typed encode writes 0 (as for every
unlisted bit); no `PROTOCOL_VERSION` bump (the wire bytes are unchanged).

**`sim/units.md`, `sim/stats.md`, `drlg/rooms.md`, `sim/tick.md`** (the
harness note and `prop-sim-core`): CH1 rooms have no d2-sim provider: `Drlg`'s
arrays come from the rooms-near order, i.e. tile rects the tick recordings
lack; next step is the `rooms.md` Test vectors "Full check" recorder
extension (per active room: DRLG room, rect, level, rooms-near array; format
by its spec session) and a `RoomModel` building a `Drlg` level from it. CH2
`convert_tick.py` raises "unknown record kind" for `anim` (handled: header,
game, footer, tick, step, set, cancel, ex, hin, hout, rin, rout, qin, qout,
qclear, ract, rdeact, snap). CH3 stats replay limits (documented in
`stats.rs`): the regeneration entry points are not replayed (`player_regen` /
`monster_regen` need a whole `Sim`; their writes are applied as recorded),
host work inside callbacks is applied after the operation, monstats
`DamageRegen` and itemstatcost `itemevent1` are not in the recording
(`item_event` is a no-op). CH4 the packets replay through `d2-server` fails
on today's recordings by construction (C39). PS1 `tick.md` §5.5 consequence
1 and edge case 4 say an every-tick event scheduled during the queue run
never runs in the same tick; by §5.5 r3 (each list's cursor starts at the head
when the run reaches that list) that holds only for lists already started, so
an every-tick event of a later class (e.g. a monster mode change scheduled
from a missile hit) runs in the same tick; the code and the `sim/tick`
replays follow r3 (wording fix in `tick.md`). PS2 `stat-lists.md` OQ5 ("is an
extended list ever given NEWLENGTH?") has an answer: §8.1 r4 sets NEWLENGTH
on the unit's list R, which is extended, whenever a TEMPONLY list is
attached; if R is an item's list worn by a unit, §10.4 expiry on the wearer
meets R (expire 0 ≤ frame): endless in 1.14d, the deliberate d2rs panic of
edge case 4; reachable through the public API. PS3 §6.1 r1 with §8.1 / §8.2:
attaching to or detaching from a unit whose own list is parked moves no
values (spec behaviour; the model parks only by states ≠ 0). PS4 §8.4 /
§8.6: equip sets the list's unit even when attach refused it (no unit list,
or a cycle), and a later dynamic toggle then propagates into the unit's list
without a counted child (same for a parked list); spec text as written.

**`sim/intents-events.md`, `client/bridge.md`, `d2-net`** (`prop-worldsim`,
`fuzz-server`): PK1 a handler's early refusals are not compared: several
specs order state changes before a refusal (`cube.md` §2 step 3.1 records a
targeting reset before 0x2A returns 3) and a per-handler list of "returns
before any effect" paths is written nowhere; property 4 covers only the
dispatcher's own rejections. PK2 not on the property host: the cube's item
world, the quest ids 0x31 / 0x40 / 0x58 (stubs on `prop_handle`'s hosts),
and anything past act 0's ISLE level (waypoint travel stops at the unwritten
same-act placement, so the player never changes room and rooms beyond the
first activation are never streamed or freed); S→C output on that host
comes only from the trade handlers (0x27 / 0x28 / 0x29 on talk, 0x2A on
buy / sell), so its byte comparison is non-trivial only there. PK3 the
fixture (tables, DS1 / DT1 sources, `TestPending`, ~1,000 lines) duplicates
`e2e_single_player.rs`, and `prop_handle.rs` copies `e2e_vendor.rs`'s
`TradeWorld` rests; fixture logs (`TestPending::log`, `Rest`'s log,
`DeathDrops::placed`, `SimGame`'s staged `UnitFacts` of removed units) grow
with the run and are not bounded. FS1 `d2-net` has no code (`lib.rs` is a
doc comment): no `prop_*` there until Phase 7 (framing, length prefix); the
server queues take any number of messages per frame (the original's linked
lists are unbounded too): a remote client could grow them without limit,
network-mode policy (Phase 7, out of scope). FS2 a shared test-support
module for the fixtures of PK3. FS3 `dispatch::in_range` subtracts `i32`
positions without wrapping; positions are server-staged `UnitFacts`, never
client bytes (targets are u16), so not changed. FS4 `bridge.md` §4 r3 covers
the 0x204 case only through "never sends a message the transport would
assert on": should it name the case and its error (`IntentError::TooLarge`)
next to the 0x200 game-sender case? Facts the properties learned (for the
next ones): an S→C size rule can read past the size it gives (0x16 `u16@1`
min 13 and 0xAC `u8@12` min 13 size a message shorter than the 13 bytes the
rule reads, so a split message re-evaluated alone is `Incomplete`; evaluate
it on the rest of the buffer); the host's first frame starts the tick
driver without a tick (`tick.md` §1 r2).

**Game-file test readings** (to settle on the first run; the notes'
§3 / §2): GA1 `assets.md` does not say what `TblAsset` does with a plain-text
`.tbl` (`DEFAULT.TBL`, `FONTER.TBL`, named in `tbl.md`; `cof.md` names
`amblxbow.cof` as junk); `assets.md` §A2 needs the rule (the test prints the
outcome and excludes the three files). GA2 the C7 entry named the file
`assets_game.rs`; it is `game_assets.rs` (C7 and §4 now point there). GA3
how `mpq-tool formats` enumerates names in `patch_d2.mpq` (no `(listfile)`)
is not written in `specs/` or `docs/`; the tests take them from the union of
the other archives' listfiles (6 DC6 → 1,657 files; 1,651 if elsewhere). GA4
"3,529 of 3,558 names have a `.cof`" could count records or distinct names
(3,529 = 3,558 − 29 duplicates); the test asserts records and prints the
distinct count; if the run gives 3,558 / 3,529, fix the test and the spec
wording. GA5 for SKA11HS, AMA1BOW, 10A1HTH and VMS1HTH only the event bytes
the spec lists are asserted (not that the others are 0). GS1 `vitals.md`
"level 99 → 3,837,739,017" read as experience row 100 (`threshold(c, 99)`);
a panic past the table means the spec means row 99. GS2 `preset.md` measured
the d2data / d2exp copies (OQ5: the patch MPQ has no listfile) while the
provider reads through the archive set (patch overrides apply); "1,054 rows"
is rows with SizeX and SizeY ≠ 0. GS3 `stats.md` §6.3 "(162, 163 → maxstamina)"
under op 1: stats 162 and 163 have op 1 with an entry in entries(11). GS4 the
fCallback list is elided in the spec ("7, 9, 11, 78, 81, 83, …, 204"): count,
first six and last asserted. GS5 §4.3 (animation rate) has no spec: the sweep
uses the AnimData speed as s. GS6 `Unprovided` keeps every `Pending` /
`WorldPending` default and `NamedIds` stays default in the wired 100-tick run;
the player's stats go through `VitalsView` with `NoHost` (the stat host the
game would use is not wired to vitals); a non-empty `errors()` is a wiring
finding. GM1 `population.md` Test vectors "sparsePopulate rows | 528 evilhut
= 40 only" vs `ai-functions.tsv` pairing `529 evilhut`: the test checks
"exactly one row, value 40" (record the row in `population.md` after the
run). GM2 `init.md` "Real 1.14d values" gives one `block` per class after
three difficulty sets: read as all three. GM3 `population.md` Real: "MonDen
600" / "1056" / "MonDen 0" have no "×3": only Normal is checked; `rangedspawn`
"set only on" levels 110–119, 123–131, 135 is checked as exactly those;
`WarpDist` 2025 on every level with `Act` 0. GM4 `levels.md` "SrcDam 63" is
63 rows with SrcDam ≠ 0, "skpoints 0" no row with a `skpoints` formula;
`use.md` "periodic 2 (Thunder Storm, Blade Shield)" is rows {57, 277}. GM5
`functions.tsv` status `mapped` = "referenced by some live row" (inverse of
`unreferenced`). GM6 `missiles.md` §R9 does not say a live row never names a
null server-damage slot; the test requires a non-zero `pSrvDmgFunc` to name a
filled slot 1–14 (§R9.1). Not covered by `game-tests-monsters-skills`:
`combat/hit.md` / `damage.md` vectors (need a `CombatWorld` provider), the
synergy vectors of `levels.md` (need a unit with skill levels) and Kick, the
population recordings (no positions or RNG).

**`render/composition.md`, `render/capture.md`** (`render-composition`,
`render-capture`): RN1 `composition.md` OQ2: the order of `L` (shade chain)
and `T` (translucency table) when both are present; the compositor applies
the whole chain, then `T` (`BlendOp` doc); nothing builds such an item yet
(`blend-modes.md` unwritten). RN2 OQ1 / OQ4: the presentation palette is the
`pal.pl2` first 1,024 bytes (R, G, B, index 0 included) vs the act `.dat`
(§5 C49; `map::cpu::to_rgba` paints 0 black, VM1); `0x0044E100` writes 1 to
the post-clear counter (`FrameCycle::set_post_clear`); §7 DirectDraw
(display type 3) is outside the GDI reference. RN3 `formats/palette.md`
`Pl2::alpha_blend` doc says `[level][source]`, the spec says
`[dest][src]` (d2-formats doc edit, separate task). CP1 who provides the
capture `SceneSource`: the world view needs a `ClientWorld` (level, tiles,
units) for the recorded frame, which the capture does not hold (only player
and camera state); either the recording grows a level / room / unit snapshot
(`capture.md` §3 change) or the source replays the recorded game's seed
through the local server. CP2 initial framebuffer: `scene::compose` takes no
previous frame (`SceneJob::previous` hands it to the source; the original
keeps unwritten pixels, `composition.md` §3, §6). CP3 recorder findings
(`record_frames.py` not changed): `FrameRecorder.stability()` counts every
group, singletons included, and never applies §7's "at least two keys seen
at least twice", so its `0 with differing frames` can show while §7 does not
hold; the raw header names no image directory (the case derives
`game/captures/<stamp>` from the file name); frames before the first server
tick carry `"f": null` (read as "no tick", ignored by `repeated_ticks`). CP4
exit code 2 for `SCENE NOT WIRED`, as for `GPU NOT WIRED`.

**`client/render-pipeline.md`, `client/assets.md`, `drlg/levels.md`**
(`p6-integrate` §Questions, `p6-world-view`): PI1 the live acts take the
app's `--seed` as the DRLG init seed (game +0x7C): which spec owns game
creation's seeds (where +0x7C comes from)? Until then a live run is
reproducible but not the original's seed for a given game. PI2 `GpuAtlas`
packs every resident frame, not only the frame's: fine while residency is
small; with C2 wired, follow the store's eviction (rebuild) or pack per
frame? PI3 the app runs `ActionSim` + `ActionWorld`, not `WorldSim`, so the
waypoint object is allocated by the app and not read from a level's presets,
and the join-time placement (`levels.md` §10 spawn room, then a position) is
not wired (now `path::place::level_spawn_point` + `place_unit`, step 7j).
WV1 `UiRules::ui_pass` gives every UI item one key, so emission order is draw
order (the C8 design); if `ui/panels.md` needs UI items interleaved with
world passes (a cursor pass) the key layout stays and only the hook changes.
WV2 tiles are one hook returning complete `TileDraw`s (placement, key, shade
and blend all come from the draw-order / DRLG specs); split it if those
specs answer them separately. WV3 presentation bars: Bevy centers the scaled
sprite in logical units, so an odd remainder can land half a pixel off from
`Presentation`'s rounded-down bars (outside verify; the cursor mapping uses
`Presentation`, so a one-pixel pointer / image mismatch is possible until
the present step places the image in physical pixels, i.e. in a render-graph
node; C8 open question 2). The hook table of `p6-world-view.md` (tiles,
unit pose / params, component frame, place, shade, blend, UI image / text /
pass, palette, unhandled UI events, panels) names each owner spec; the
window stays black until they exist.

**`combat/damage.md`, `combat/vitals.md`, `sim/units.md`** (`e2e-combat-path`
§5; `TODO` at the sites): EC1 reaction order (`reaction.rs`):
`Pending::reaction` runs before the state-54 rule and the kill; in 1.14d the
state-54 test sits between the hit class and the defender branches, and the
monster kill between the "knockback without KB → get-hit" adjustment and the
knockback / block / get-hit / soft-hit changes; same result while the seam's
branches are empty for a state-54 defender; settled by `damage.md` OQ3 (read
`0x0057CEE0` branch by branch). EC2 experience in the kill: where `0x0057CCB0`
gives experience is not stated (`damage.md` OQ7, `vitals.md` OQ2); given last,
to the attacker, players only, without pet credit / party share / `ExpRatio`
/ stat 85 (no draws, so only the order of stat writes against the other kill
steps can differ). EC3 the drop inside the death start (`death.rs`): the
gate's position in `0x005A6FF0` is unknown; the collision word outside every
room grid is read as 0; the free-spot search gets the room the start-offset
search found, else the monster's room. EC4 the drop's item creation runs on
`ActionHooks::game_seed` (copied into `GameFields::seed` and written back):
one game seed for units and items (`rng.md` §5.3, W16). Seams the path stops
at, with the fixture's answer (none invented): the COF-name composer
`0x0064F5B0` (`animdata.md` OQ2: the weapon class needs equipment), the rate
`0x00623F50` and bonus `0x00623B10` (`units.md` §4.3 unwritten), srvdo bodies
(`use.md` OQ10), `skill_missile_fill` (§5.4 step 7), `missile_damage_setup`
`0x0059F900`, path / target flags / collision bit, `Pending::reaction` and
`kill_step` bodies, `monster_death_start` `0x005A6FF0`, `FreeSpot`
`0x0064E810`, monster rank / minion owner / party / quest TC.

**`world/vendors.md`, `world/npc.md`, `items/inventory.md`**
(`e2e-vendor-host`): EV1 two owners of the player's interact unit (+0x64 /
+0x68): `Pending::{set_interact, reset_interact, interact_guid}` (waypoints)
and `NpcRest::{interact_unit, set_interact, reset_interact}` (NPCs) hold the
same player field, separate on `TradeWorld`; a single player-data provider
should own it. EV2 `GameFields::{difficulty, game_type, ladder}` (economy)
and `ActionHooks::ai_info` (AI) both hold game +0x6D / +0x6A / +0x74; only
the seed is unified. EV3 stubs reached and their owners: `VendorRest::
copy_item` (`0x0055A2A0`, no items spec: every purchase and the store copy of
a sale), `place_in_store`, `add_trade_inventory`, `refresh_npc_inventory`,
`new_store_inventory` (store grid, S→C 0x9C action 11), `owns_item`,
`remove_stored`, `place_in_backpack`, `can_belt`, the cursor (inventory spec,
now `items::inventory`, unconnected), `gold_cap`, `stash_cap`, `drop_gold`
(player spec), `encode_text_list` (0x27 bytes 6–39, `0x00661480`),
`distance`, `axis_check`, `unit_check`, `clear_path` (unit / path spec),
quest callbacks `unhandled` (chains 1–6, 37 on Akara's talk). EV4
`NpcRest::item_format` holds game +0x78, a game field, not a rest call.

**`skills/use.md`, `world/npc.md`, `world/quests.md`, `sim/stat-lists.md`**
(`wire-open-seams` §5–§6; `TODO` at the sites): WO1 spec conflict (recorded
as I3 by `wire-interaction`, not decided): Akara's respec (`npc.md` §8.2,
lines 400–405) says slot 41 bit 1 → "reset stats (`0x00570360`) and skills
(`0x00570C80`)", while `vitals.md` §2.1 names `0x00570C80` the **stat** reset
(players only: strength, energy, dexterity, vitality; `d` = charstats start −
base, `statpts −= d`, stat `+= d` with refresh, `gain_energy` /
`gain_vitality`), implemented as `combat::vitals::reset_stats`;
`0x00570360` appears only in `npc.md`; both stay seams
(`NpcRest::reset_stats` / `reset_skills`, `TODO(npc.md §8.2 vs vitals.md
§2.1)` at `npc_world.rs`); check both addresses in the 1.14d binary and
correct one spec. WO2 mercenary reward order: the reward is deferred to right
after the 0x31 list dispatch; chain 37 (Act I intro) is visited after chain 2
and has an event-11 function `0x0058F870` without a body (`unhandled 37
0x58f870`); in 1.14d it runs after the reward, here before; same order only
if `0x0058F870` does nothing observable for message 92. WO3 event 5 level
argument: `stat-lists.md` §10.2 passes `f(game, unit, skill, a2)`; the do
function takes a level, read as a2. Seams left open by that note: event 9's
second call `0x0056CE70` (no body; nothing reschedules a type-9 event), event
14 / `0x00554570` (`cooldown_end`, never scheduled in 1.14d, `use.md` §6),
event 10 `ai_reset` `0x00573120`, umod 41's `InitHost::run_ai_tick`
`0x00573780`, `TickHooks` environment / presets (non-population) / messages /
items / compress units, the `NpcRest`, `VendorRest`, `UseRest`, `VitalsRest`,
`CubeRest`, `QuestRest` members of `wire-interaction.md` §6 and
`wire-economy.md` §5, the kill → `kill_experience`, one combined host (§2
step 2).

**Wired-game observations** (`e2e-single-player` §2–§4): travelling to a
chain level (Monastery Gate, 26) generates its outdoor neighbours, which fail
on the fixture's empty `lvlsub` rows (`Outdoor(NoSubRows(0))`,
`Drlg(LevelType(7))`): a fixture limit, not a seam fault; the e2e travels to
level 31 (a preset level outside the Act I chain). The client's duplicate
filter drops identical bytes within 200 ms (`bridge.md` §4 rule 5; five idle
frames before the repeated buy in `e2e_vendor`). Replace `SimGame::join`
before the link with the 0x67..0x70 flow through `PendingSession` once the
session spec exists.

From the first 1.14d frame captures (2026-10-06, local, `record_frames.py`
of branch `claude/spec-render-placement`; raw files gitignored:
`traces/raw/20261006-140102-frames-run1b.jsonl` +
`game/captures/20261006-140102/`, `traces/raw/20261006-141725-frames-run2.jsonl`
+ `game/captures/20261006-141726/`, kept in that branch's worktree), by owner:

- **`render/capture.md`** (stability, §5 A): the capture is stable (scene,
  UI and palette identical frame to frame), but no 1.14d scene is fully
  static: (1) the **mouse cursor** is drawn into the framebuffer and
  animates (black ↔ index 172 pixels where it rests); log its position and
  frame per capture, or mask it; (2) see lighting below; (3) run 1 (with the
  automap open, mana regenerating, Quest Log button flashing) showed those
  UI animations too. Stability must be defined as "equal outside the
  listed animated sources", or each source must be modelled. Frame pacing
  under the debugger: of 1,111 town frames, 625 follow 1 tick, 466 follow 2
  ticks, 19 follow 0 ticks (frames dropped, never duplicated): every frame
  carries its tick, so per-tick comparisons stay possible.
- **`render/lighting.md`** (not written; §B8): **corrected** by
  `claude/spec-render-followups` `09c2797`: on run 1b's second still
  segment (f 14,800–15,600) 772 of 785 same-key pairs differ only inside a
  33×30 box = the cursor's idle `orotate` image (64-step loop, one step per
  draw, idle state draws from the player's client seed); no RNG or clock
  read was found in the light map, so there is no evidence of a light
  flicker. Unexplained: the first still segment (f 12,543–13,694) differs
  outside one box (±1 dark-ramp steps around the player and the left
  floor); the next `stability-0001` with recorder `frames-raw-2` settles it.
  Partial lighting rules: `capture.md` OQ5.
- **`render/composition.md`, `blend-modes.md`**: `composition-0001` exists
  (run 2: Town Portal open beside the player, 800×600 GDI, video type 1):
  the input for settling the blend-table orientation (row = destination per
  `composition.md` vs `palette.md` / `render-pipeline.md` §A5) and the
  `pal.pl2` vs `pal.dat` palette question. Rain and torch flames animate in
  town: town comparisons must mask them or model the weather RNG.
- `placement-0001` (inventory open / close) and `camera-0001` (walk, run,
  stop: modes 6 / 2 / 3 / 1) are in the same two runs; no comparison has
  run yet (needs the d2rs side of each case).

From the Phase 6 infrastructure (notes `docs/handoff/p6-*.md`), by owner:

- **Integration (disagreement between notes):** the scene note's
  `DrawItem.frame` is a `FrameId` (atlas slot lookup left to C3 / C5); the
  frames note says C4 composes `FrameRef` = `AtlasSlot` + `IndexFrame`;
  both name a `FrameSource` trait with different methods
  (`scene::FrameSource::frame(FrameId)`, `frames::FrameSource` with
  `frame_set` / `part_count`). `FrameSetKey` holds a `String` (frames) while
  C1 has `CanonicalPath`: replace when merged. `ui::Rect` and
  `scene::Rect` are separate types; the UI emits `UiDraw` through a sink,
  not `DrawItem`s (scene implements the sink). C5 must place the image
  with `ui::Presentation`.
- `render/composition.md` (§B2): scene `to_rgba` maps index 0 through the
  palette, `map::cpu::to_rgba` paints 0 black; the `map` verify case will
  show it. `TODO(spec: render/composition.md)`.
- `render/shading.md` (§B3): mapped shade result 0 drawn as index 0; one
  palette per frame. `render/blend-modes.md` (§B5), `draw-order.md` (§B6),
  `sprite-placement.md` (§B1; `flip_x = true` is an error; `IndexFrame`
  offsets never interpreted), `unit-composite.md`: neutral hooks only.
- `render-pipeline.md` §A2 / §A9: frames wider or higher than 2046 (answered
  by §5 C8); a window smaller than 800×600 is refused (`TooSmall`; §A9 has
  no scale below 1); odd bar remainder rounds the left / top bar down
  (extra pixel right / bottom; C5 must agree); the §A4 clamp is a no-op by
  construction (positions past the image are `Outside`).
- `ui.md` §A2 / `ui/panels.md`: `UiResponse` has no "close me / open panel
  X" answer; keyboard focus model (chat) unknown; hooks: `NoPanelRules`,
  panel background vs world clicks, `ImageRef` frames and offsets, text
  style and caret (`ui/text.md` §B3), `CellGrid` (`ui/inventory.md` §B5),
  wheel rows, `PointerButton` (`ui/controls.md` §B4), cursor drawing.
- `ui.md` §A6 / `ui/controls.md` (§B4): the `Action` list and contexts are
  a placeholder; wheel / modifier semantics; `<config_dir>` per platform
  and missing-file policy undecided; `preset` is required (our choice);
  a newer file version is rejected.
- `assets.md`: budgets are guesses until measured (OQ1); non-ASCII archive
  names refused (OQ2); prefetch (§A4 step 3) waits for the bridge's
  "newly active room / unit type" report; `ClientConfig` (§A6) not
  written; DCC per-direction decode (`Dcc::parse` decodes the whole file);
  §B1 which archive wins and §B2 locale font directory belong to their
  owner specs.
- `audio.md`: tick domains (UI triggers carry the client tick, sim triggers
  the sim tick: one presented-tick counter assumed); whether a natural
  one-shot end appears in the voice log (§B7 trace decides; today not);
  per-voice rate and device rate (spec OQ2, OQ3); the "one voice, full
  volume, centered, constant 1000" vector waits for §B3 tables; variant RNG
  not implemented (no `d2-client → d2-sim` dependency); `CueSource`,
  `SoundBank`, `GainCurve`, `VoicePolicy` seams await `audio/triggers.md`,
  `environment.md`, `sound-table.md`, `formats/wav.md`.
- **`render-pipeline.md` §A9 / §A10 (GPU compositor, second set)**: 16×16
  workgroups, not one 32×32 workgroup per bin (each invocation still walks
  only its bin's list); output is a storage buffer of one u32 per pixel
  (r8uint is not a core WebGPU storage format), readback keeps the low
  byte; the RGBA pass is a bit copy of the palette bytes, not a write to an
  sRGB target (presentation to the window is outside the verify boundary
  and not written); `pack` stores each item's drawable area precomputed;
  the atlas is one R8Uint 2048² texture array uploaded per compose call (4
  MiB per page; C3's `AtlasTextures` holds separate `Image`s, an in-app node
  copies them into layers or owns the array); limits are checked before
  dispatch (`GpuError::Limit`). Spec OQ2 (60 fps on a full town scene) is
  not measured (the `stress` case, 400 items at 800×600, is a correctness
  case). The shader implements C4's neutral behaviors exactly (mapped
  result 0 draws index 0, clear value 0, one palette, `flip_x` rejected), so
  a §B answer changes `scene` and the WGSL together.
- **`render/unit-composite.md` (§B4), `sprite-placement.md` (§B1),
  `shading.md`, `blend-modes.md` (§B5), `draw-order.md` (§B6), `camera.md`**
  (hooks of `composite::ComponentResolver`, tags `TODO(spec: …)`): the
  component file path / armor class variant / mode / weapon class and the
  frame inside it, whether an empty variant is drawn (today every slot is),
  unit direction → COF direction and the frame source (animdata vs COF
  rate), offsets → screen top-left, light level and per-component colormaps,
  COF translucency override → blend op, `UnitParams::{pass, major, minor}`
  and `clip`; `flip_x` always `false`; the COF shadow / selectable fields are
  not read (available through `ComponentRequest::layer`). Open: do live COFs
  list a draw-order component without a layer record, or two records for one
  component (refused here; §5 C9 answers). Residency: `frame_id` is a hook
  because no store maps `(FrameSetKey, index)` → `scene::FrameId`; the
  GPU path uses `SlotSource` (`FrameId(n)` = element n of `[AtlasSlot]`) and
  `AtlasFrames`, the in-game path has neither (the `FrameId` / `FrameRef`
  disagreement above stays undecided).
- **`render-pipeline.md` §A10 (verify)**: synthetic images are not written
  to disk (the report has count and first mismatch; a `gpu.png` / `cpu.png`
  for debugging a mismatch may be wanted); the `map` case sorts first and
  starts its own Bevy app, several wgpu instances in one process may not
  work (§5 C16 records it); `--perturb` flips the index top bit of the
  reference, so under the synthetic palette (red = index) exactly N bytes
  and N pixels differ.
- **Dependencies added** (M10): `toml_edit 0.25` (`parse` only) in
  `d2-client` for the controls file (already in `Cargo.lock` via Bevy);
  `wgpu = "29.0.3"` in `d2-client` with Bevy's own backend features (`wgsl,
  dx12, metal, vulkan, naga-ir`), the exact wgpu Bevy 0.19.1 already builds
  (the only `Cargo.lock` change is the dependency line; `wgpu::naga`
  validates the shader in CI). The cloud llvmpipe check needs `apt-get
  install mesa-vulkan-drivers libvulkan1`, not part of
  `tools/cloud-setup.sh`.

**Seventh set (notes `host-merge`, `host-merge-port`, `impl-path-core`,
`impl-walk`, `wire-path-sim`, `wire-inventory-sim`, `wire-inventory-server`,
`render-camera-placement`, `render-wire`, `conformance-path-render`,
`prop-*`, `mutants-*`, `bench-baselines`, `game-tests-inventory-path`,
`gaps-path-render`, `gaps-new-specs` (no new question), `fix-statlist-prop`; same rule: each question names the
code's reading).** IDs are new; the notes' own labels are in brackets. Note:
`wire-inventory-sim` calls its three questions WV1–WV3, which the sixth set
already uses for the client world view; they are WN1–WN3 here.

**Resolved in the seventh fold:** J1 (one unit store and one host value:
`host-merge`, §1 row 3y) and J4 (one provider of the skill-use seams:
`SkillSeams` deleted); I7 and W16 on the server host (game-creation fields
and the seed have one home in the action wiring: `ActionHooks::game_seed`,
`ai_info`, `UnitData::expansion`; the economy's `GameFields` is built from
them per call, `WiredWorld` no longer holds one); EV2 (same; only the unique
bits stay split, HM4); EV1 on `WiredWorld` (one owner, the trade rest's
`NpcRest`; `HostWaypoints` and `items::Interact` ask it; the two-seams
remainder is HM3); J12 (`path/mod.rs` declares the whole module; the core, the
walk and the placement now meet through `CollisionRooms`, `PathWorld`,
`WalkUnits`, `PathMotion`; the duplicate seams, tables and types of the walk
were folded onto the core, `wire-path-sim` §3); the unwritten provider of
the `CollisionView` seam (`DrlgWorld`); wire-action W5 / WG7 (missile
collision queries, when the provider is on); `server-skills.md` §5 item 9
(`pierce_idx` TODO of `SimGame::handle`: `use_::handle_message` does it);
`quest-host.md` §5 (the stale `world/action.rs` header); `server-world.md`
§6 "fold into one host trait"; `e2e-vendor-host.md` §4 findings 1–3 and
`e2e-next.md` §5 findings 1 and 3 (2 in part: HM5); §2 step 3 "tick
messages" (`SimGame::tick` queues `take_sent`). J2 was already done
on the base; no other J item is touched by this fold.

**Host (`host-merge` §4–§5; `TODO` at the sites):**

- HM1 [W-1]. `UseView::start_mode` hands the mode start to
  `UseRest::start_mode`, which has no access to the unit records; it should
  run `modes::set_mode` → `UseRest::clear_target` → `modes::animate` → clear
  flag 0x40 itself (`use.md` §4); then the server's copy
  `skills::world::World::start_mode` goes. Until then the timer path's start
  (a fixture's `UseRest::start_mode`) and the message path's differ. Files:
  `wiring/{action,interaction}`.
- HM2 [W-2]. `NpcWorld::item_format` / `VendorWorld::item_format` answer from
  `NpcRest::item_format`; they should answer from the economy's fields
  (`ItemGame::item_format(econ.fields)`, game +0x78). The fixtures answer 1,
  the fields give 2 (classic): a real difference once a store item's version
  ≥ 100 matters (`vendors.md` §3 rule on `+0x78 < 100`; EV4).
- HM3 [W-3]. The player's interaction field has two seams
  (`Pending::{set_interact, reset_interact, interact_guid, busy}` and
  `NpcRest`'s calls); a player-data provider in `d2-sim` should own
  +0x64 / +0x68 / +0x6C and both wirings read it; then `HostWaypoints` goes
  (`ActionWorld` alone keeps `Pending`'s interaction calls).
- HM4 [W-4]. The unique bits (+0x1B24) still have two copies when a game has
  both drops and the host's economy: `DeathDrops::fields.uniques` (in the
  action wiring's `Pending`) and `WiredWorld::uniques`; a unique dropped by a
  monster is unknown to the cube and the store. Fix: a game-level home
  reachable from both (e.g. on `ActionHooks`).
- HM5 [W-5] = IS4. The drop's item store (`DeathDrops::items`, inside
  `Pending`) is separate from the host's (`e2e-next.md` §5 finding 2): the
  dropped gold cannot be picked up (0x16 → result 1), sold or cubed. **Being
  fixed by the unify-items session (also IS5).**
- HM6 [W-6]. The init write of difficulty 2 (`worldgen::init_units`
  `set_difficulty`, `init.md` §9) updates `ai_info`, `init_info`, `pop_info`
  but not `UnitData::difficulty`, which the waypoint view reads on
  `ActionWorld` (`WiredWorld` reads `ai_info`).
- HM7 [finding 4, `quality.md` §8]. `ActionHooks::ai_info.game_type_ex` holds
  game +0x74 as `u32`, `GameFields::ladder` as a flag; `from_action` reads
  `≠ 0`. If any reader needs the full +0x74 word, the economy's field becomes
  `u32`. Readings recorded without a question: the `set_mode` of the skill
  pipeline is now `units::modes::set_mode` (`0x00553570`; the old server copy
  used `modes::player_start`, which also asks the request-check hook, which
  accepts); the skill handlers read the room kind from the DRLG (`CombatWorld::
  room`; a unit outside every room makes `start_core` return 0, `use.md`
  §5.3).

**`sim/path-placement.md`** (`impl-path-core` "Open points", `prop-path-place`,
`wire-path-sim` §6, `game-tests-inventory-path` §3; `TODO(spec: …)` at the
sites): PC1 §4 r4 box strips: the right strip has full height and the top
strip the inside width (D2MOO's reading; spec OQ2); also used for the object
box set / clear, whose clipping the spec does not state separately. PC2 §5.1:
which room each stamped cell is looked up from is not stated (the code uses
the centre's room, else the given room, as §4 rule 3); patterns outside 0..5
stamp nothing. PC3 §2.4 r4: a missile path gets "type 4" stored directly;
whether the type table's flags (0x60000, incl. 0x40000 missile path) are
applied is not stated. PC4 §6 r4: teleport clears and queries with the path's
room (as §6 r1) and "flags 0x8 := moved" is read as "the cell changed". PC5
§5.2: an object's `HasCollision` for modes above 7 reads as 0. PF1 §12.1 r3
(`5·lx + OffsetX/Y`) and §12.2 r5 (`point + ExitWalkX/Y`): the code wraps in
32 bits (`wrapping_add` / `wrapping_mul`, as the original's integer
arithmetic); the spec does not say what happens past i32 (1.14d data never
gets there). WP4 [wire-path-sim §6] the teleport footprint room: the core
clears and stamps from the path's room before the move, so a warp to a room
not adjacent to it (waypoint travel) stamps nothing at the destination; the
waypoint test records it; settle on Ghidra `0x00650910` → `0x0064EFA0`'s room
argument. WP5 [wire-path-sim §6] the second spawn search in `waypoints.md` §7
r7 draws again on levels with `Position` 0 (room roll); with several rooms the
arrival test may compare against a different room than the placement used (the
test uses a one-room level; wire-action W14 concerns the same search). WP1–WP3
are the three blockers of §2 step 7p (missile flight, FreeSpot seam, teleport
footprint room). GX4 [game-tests-inventory-path §3] §11's real spawn points
are R1 (4863, 5653) and R2 (4673, 4548) from `20261006-015956` /
`-022633`; the spec states no map seed: R1's tile (972, 1130) lies in the
Rogue Encampment rect `world_data/tests/game.rs` derives for map seed
644409375 (960, 1112, 56 × 40), R2's tile (934, 909) does not, so the
recordings used different seeds: state each recording's seed (`.d2s` 0xAB) in
the Test vectors. CR1 [conformance-path-render §4] no recording format holds
per-tick path state (`pathing.md` OQ1; the recorder hook list is §2 step 14)
and CR2 none holds placement calls (`path-placement.md` OQ1; or the 0xAC /
0x9C position bytes: `server-messages.tsv` has no 0xAC layout, 0x9C's `data@8`
is not broken down). CR3 `pathing.md` OQ6: 0x96 is sent by the status routine
`0x00548760`, whose owner spec is open (the movement harness can start with
positions only: `MoveOptions { messages, positions }`).

**`sim/pathing.md`** (`impl-walk` §4, `gaps-path-render`, `prop-walk` §2):
PQ1 V1–V3 vs §8.1: the formula p = f + stat 67 (floor 25) gives V1 only when
stat 67 holds its creation value 100 (`combat/vitals.md` §1 table; the vector
says 0); V2 then needs 150 (100 + run list 50); V3's printed p = 67 / 1029
drops the base, the formula gives p = 17 + 150 = 167 → 2565. The tests follow
the formula with base 100; the V rows need rewording. PQ2 §8.1: a mode
without the velocity modifier has no rule (`mode_velocity` returns `None`,
the velocity is left as it is). PQ3 §8.1 r3 read as "+0x38 := 15 only when the
value differs; velocity and max velocity always set". PQ4 §9.5: where the rule
says "→ re-path", the arrival result is the re-path's (non-zero passes). PQ5
§9.2 step 2: state 13 is a branch (the seam runs, the event returns "moving");
whether the step continues after `0x005C9D90` is not stated. PQ6 §1.4 r5 read
as "state 42 → roll (r < v → rule 6, else allow); else state 15 → rule 6";
whether a failed roll falls through to the state 15 test is not stated. PQ7
§9.10: the town-access argument of the re-path's compute is not stated (0).
PQ8 §9.6 r8: the step passes no room hint to set position. PQ9 edge case 5 is
unreachable through §9.4: (m · direction) is a 32-bit product that wraps above
8 sub-tiles per tick on an axis (the test drives `cell_walk` directly). GR1
[gaps-path-render, spec question 1] §9.6 r2 vs §9.4 r2.2: movement goes to
reset when the velocity vector is (0, 0), so one step's own "velocity vector
0" branch (`0x00650660`) has no caller in this spec: is it called from
elsewhere (missiles, knockback) or dead in 1.14d? PX1 [prop-walk PW-Q1] edge
case 4 lets the toward path start at a colliding cell: ray §5.1 r4 x-major
skips the cell after the minor step with err = 0 and the next major step is
blocked, so the remembered P can collide with a wall (minimal input in the
note); §5.2 steps 2–3 store P as `points[0]` (edge case 3 may store it twice),
and a unit walking it is refused at P (edge case 7). The code does what the
spec says; edge case 4 could add "so the returned point P can collide".
(Implementation limits recorded in the note, not questions: a cell-walk loop
bound of 2^20 and an A* propagation stack over 200 are `WalkError::Fatal`.)

**`render/camera.md`, `render/sprite-placement.md`** (`render-camera-placement`
"Findings"): RC1 roof drawer: camera.md §6 hands roofs to `DrawGroundTile`
(floor drawer, −80 and panel shift) while sprite-placement.md §7 lists the roof
drawer with the wall drawer `0x005131B0`; implemented per camera.md (owner of
tile positions); reconcile. RC2 DC6 `flip` other than 0 / 1: the original
tests bit 0 only (placement §1), `d2-formats` `dc6.rs` decodes any non-zero
`flip` top-down; `from_dc6` refuses such frames; the count of C52 decides.
RC3 top-down cel starting at `H − 1`: the original writes its following rows
past the surface, d2rs clips them (unreproducible by definition; noted under
placement Edge cases). RC4 shake arithmetic: `t3 = 0` at `t = t1 + t2` divides
by zero in the original (§8 table) and a product `A × t` over 32 bits is not
specified; both are `ShakeError`, never a guessed value. RC5 wall culling as
one clip: per-block culling becomes a clip of the assembled DT1 image, a
culled block overlapping kept ones is an error (never with 32-pixel blocks on
a 32 grid; else the tile image needs per-block draw items). RC6 unit culling
(camera OQ2) is not implemented: units are not culled beyond the frame clip.

**`render/camera.md`, `render/capture.md`, `render/composition.md`**
(`render-wire` "Seams (stopped, not invented)"): RW1 the recorder gap: the
recording holds the camera only; the per-frame act / level, map tiles, client
units and UI the capture needs are listed in §2 step 7q (owner: `capture.md`
§3 by a spec session + `record_frames.py`). RW2 app feed hooks: local player
position, open mode, shake start, player seed (camera OQ6), unit positions and
offsets, map tiles; until then the window is black. RW3 the compare composes
from index 0 (`scene::compose`) while the original keeps the bottom 47 rows of
the previous frame (`composition.md` §3): `compose_frame` + `FramePlan` +
`SceneJob::previous`, and `GpuJob` needs `base` + `plan`. RW4 pause: the
original draws every loop pass while a single-player game is paused (camera
§9), d2rs only on ticks. RW5 recorded `player.client` (path +8 / +0xC) is read
but not checked: no spec ties it to camera §2.

**`items/inventory.md`, wired** (`wire-inventory-sim` §6, `wire-inventory-server`
§5; each pinned by a test): WN1 [WV1] §6.3 vs `units.md` §2 / §3.1 step 5: the
allocator sets unit flag 0x10 ("seed set") on every unit and §6.3 sends a
ground message only for items without it, so on real units `items::moves::
ground_update` never builds 0x9C action 2 / 3; either §6.3 reads another
field or bit or the units table's 0x10 is wrong (pinned by
`ground::assert_ground_update`; settle: Ghidra `0x0055BF30` / `0x0055BED0` or
R1). WN2 [WV2] §7.6: X "becomes the cursor item", then N goes to the location
"as §4.6 step 5", whose text includes "cursor := none"; read literally (as
`swap_2handed` does) X ends in mode 4, unlinked and not the cursor item: an
original quirk (like §7.24's) or the order misread? (R3 or `0x00563D20`.) WN3
[WV3] §7.7: "empty location → 0" comes before §4.3, so §4.3 result 4 and
`0x0063E490`'s other-hand branch are unreachable from 0x1C: a client taking a
two-handed weapon off by the left hand does nothing (confirm with R3). WN4
readings without a rule (doc comment / `TODO(spec: …)` at each site):
`0x00557FD0` free = the unit removal `0x00555600`; `0x0063CB00` = §2.3 on
page 0; `0x0063BE30` = nothing after the §1.4 unlink; "room added" of
`0x00558AA0` = the room list insert; §6.1 rule 4 = `InvDesk::update_done`
(command flags := 0, update list freed; OQ9); a unit without an inventory is
checked against an empty one (§5). IS1 [R1] which +0xC8 bits the room
clean-up `0x00553220` clears is not written; read as the two the owner
refresh sets (§6.1 rule 1: bit 0, bit 1 for players); without a clear the
pass would resend 0x47 / 0x48 every tick. IS2 [R2] the update pass runs after
`d2_sim::tick::tick`, not inside the client pass (`tick.md` §6 step 5), because
the action wiring implements neither the per-client unit update nor
`0x00553220`; known difference: the client's room is read after the tick's
room switch (`0x00537B50`), in the tick of a switch 1.14d walks the old room's
adjacency; move it into a `TickHooks` provider holding `InvParts` once
`send_unit_update` / `unit_update` bodies exist. IS3 [R3] the ground items'
unit update (§6.3) is not run (builds nothing on real units, WN1). IS4 [F1]
= HM5. IS5 [F2] three inventory copies remain for the player: the vendor
rest's staged set (`Rest::inventory`, `owns_item`), the cube's
`Staged::inventories` (list + cursor) and the move code's `InvState`; an item
placed by 0x18 is unknown to the vendor's and the cube's checks and vice
versa; fix: the vendor and cube seams read `InvState` (`InvDesk`): a wiring
task on `world/wired.rs` and `items.rs` (`ItemPending::place` /
`remove_cube_item` → §2.4 / §1.4 on the player's `Inventory`); **being fixed
by the unify-items session**. IS6 [F3] the e2e sorceress fixture had dexterity
0 and §4.2 refuses a stat below 1 (the equip returned 0 with nothing
equipped); the fixture sets a synthetic dexterity 25: the creation stats are
the character spec's. PN1 [prop-inventory §4] §2.1 / §2.2: how `0x0063AFD0`'s
bound test treats x or y near 2^31 (signed or unsigned compare, 32-bit wrap):
d2rs sums in i64 and refuses (0x18 with x = 0x7FFFFFFF → 3); if the original
wraps and places, such a payload would differ. Out of the spec's domain, not
generated: a player level above 214,748 (`level × 10000`, §7.22 gold limit
wraps) and a unit position near `i32::MAX` (`x + 2` in §9.1's start point).
GX1 [game-tests-inventory-path §4] `inventory.md` §1.3: record 29 reads
255 × 255 while the spec calls it a copy of record 13 (0 × 0): the test reads
"16–31 copies of 0–15" as record r = record r − 16 except 29. GX2 §1.3 "other
(0)" is read as every page other than 1–4, so pages 5 and 0xFF get the class
record. GX3 §3.1 rule 1 is read through `body_location_allowed` (itemtypes
`bodyloc1` / `bodyloc2`), so an item type with `bodyloc` 8 that is not a belt
would also be checked.

**Tests and tools** (`prop-client`, `mutants-server`, `mutants-monsters-missiles`,
`bench-baselines`, `fix-statlist-prop`): KP1 `assets::cache::Pool::insert`
adds `bytes` with `+=`: a caller passing byte counts summing past `u64::MAX`
panics in debug (not reachable: sizes of resident memory; the property draws
below 2^40). KP2 `Pool::begin_frame` does not evict (module doc: eviction
happens when an insert needs room), while `assets.md` §A5 says the budget is
exceeded "and kept until the frame ends": if §A5 means eviction at frame end,
`begin_frame` should call it (a spec decision; the property checks the
module's reading). MS1 `ds1_input` `version < 7` → `<=` (`world_data/mod.rs:104`)
survives: version 7 must be accepted; decided by `drlg/preset.md` §5, a
synthetic DS1 of version 7 kills it (candidate for the world-data owner); also
`LevelTables::from_fixed` `delete -` (the `-1` default of `hdm_item`) and
`records` → `Ok(vec![])` need game files (C51-style `#[ignore]` tests). MM1
`monsters/ai/functions.rs:453` Wraith (§9.11) with no target T: the code does
nothing, the spec only says "walk in radius of T". MM2 `ai/functions.rs:583`
Navi (§9.10 r2) "clamp param 1 at 0 and count it down" with a negative param
1: the code gives 0, the mutant −1 (param 1 is only ever set to 60, so
unreachable today). MM3 `population/preset.rs:614` §11.6 r3 when the class's
`BaseId` row is invalid (e.g. −1): the code returns the input class, the spec
does not say. BB1 MPQ Huffman sectors about 3 MiB/s (§2 step 7r). SF1 whether
to run a scheduled `PROPTEST_CASES=20000` job (§2 step 7s); the stat-list
failure itself was a model error: `stat-lists.md` §8.6 moves values into U's
list only, so when the toggled list I sits in the active chain of another list
P (its unit set by a refused attach, §8.4), the flag flip changes how I counts
in P's `stats.md` §6.1 sum while P's full array is untouched (PS3 is the
neighbouring wording point).

**Eighth set (notes `unify-items`, `drop-freespot`, `wire-path-server`,
`path-update-pass`, `prop-fixes`, `prop-walk-gaps`, `fixedset-game`,
`game-tests-wired-host`, `mutants-*`, `mpq-huffman`, `s2c-use`,
`spec-unit-numbering`, `ci-nightly-props`; same rule: each question names the
code's reading).** IDs are new; the notes' own labels are in brackets.

**Resolved in the eighth fold:** HM5 / W-5 and IS4 / F1 (one item store:
`ActionHooks::items`, row 3af); IS5 / F2 (one inventory per unit: the cube's
`Inventory` deleted, the vendor's player-inventory calls on the model);
WP2 (the treasure drop's free spot: `DropPlacer<H>::place(econ, x, y)`; the
field is still not loaded by any host); J13 "one maker per id" (as a check,
not as a single maker: `conformance/tests/s2c_builders.rs`, option A);
SF1 (the nightly job exists); BB1 (MPQ Huffman speed; C61 / C62 verify it);
the walk / run handler of step 7p (1) and the stale line "the update pass has
no caller of `mode_update`" (`path-update-pass` §1: the callers were already
there); `stat-lists.md` edge case 4 (an expired extended list panicked: now
`StatListError::EndlessExpiry`), the stale `ListId` panics, the reused
`TimerId`, `dispatch::in_range` (FS3; the other 50-subtile copies remain, SL4);
`prop_units::anim_schedule_matches_the_closed_form` "Too many global rejects"
(a generator issue, fixed by the coordinator); the doc fix asks of
`path-update-pass` §3 (`world/action.rs` header and `SimGame::handle` doc were
already right on the base).

**Wiring and hosts** (`unify-items`, `drop-freespot`): UI1 [R1] `vendor_inv`
`owns_item` (`0x00557FF0`, body unwritten) is read as `inventory.md` §5.1's
owned-item test (`0x00549220`: the player's item list or its cursor item)
without the lookup the caller did; `in_inventory` (`vendors.md` §8.1 rule 4)
reads the same. UI2 [R2] `remove_stored`: §7.2 rule 9's "item cell := page,
item update message" names no routine or layout, so no message is queued; the
removal is the §1.4 unlink plus the free (`0x0055DF10` → `0x00557FD0`, as
`cube.md` §8 step 1 reads it). UI3 [R3] the order of a vendor call's
inventory messages (a targeting reset's 0x3F, a placement's) and its own 0x2A
is not written; the inventory ones follow. UI4 [F1] the vendors' remaining
inventory calls have no rule to wire (step 7o). UI5 [F2] the store and gamble
inventories (`place_in_store`, `add_trade_inventory`, `new_store_inventory`)
are still the rest's; `InvState::add_inventory` takes any unit kind, the
store's own grid record (`inventory.md` §1.3, `vendors.md` §3.1 step 3) is the
missing piece. UI6 [F3] the cube has no owner for the inventory pass
`0x0055FA40` (`cube.md` OQ 8), and `duplicate`, `tempered_affix`, repair,
recharge and quest hooks have no item spec: `ItemPending` keeps exactly those.
UI7 [F4] the gold's inventory size (1 × 1) is synthetic: `inventory.bin` has no
gold row to measure. DF1 [`drop-freespot` DF1] the room passed to `0x0064E810`
is read as the room the start-offset search found, else the monster's room;
`path-placement.md` §9 says treasure passes "the same arguments" as
`0x00555DA0` (the caller's room): the readings differ only when a ring cell
lies outside the start room and its neighbours but inside the monster room's.
DF2 `0x00558D90` → `0x00555230` allocates the item and its path part
(`SUNIT_Add`, §2.5) is not run by `Economy::create_item` (`ItemSpawn` carries
no position); the death drop runs it from `placed`, the cube output, vendor
and quest floor drops still give a ground item no path. DF3 with the provider
on and no field the item gets no path either (the old behaviour as a whole).

**Walk on the server** (`wire-path-server`, `path-update-pass`): WS1 [F1] no
0x15 after a waypoint warp to another level (recording R3 of
`path-placement.md` has it the next tick): the placement queues the player in
the destination room's update queue, tick 1's per-client update walks the
queues of the *client's* room adjacency (still the old room) before the room
switch (`tick.md` §6.5 order), and step 6 clears the destination queue.
Settle (spec / Ghidra): whether the placement (`0x00554EA0` rule 6) or the level
change switches the client's room before the update walk, or `0x0053A5D0` also
covers the client's own player outside the adjacency. WS2 [F2] flags 2
0x10000 / 0x800 and unit flag 0x1 are never cleared (step 6's `0x00553220`
"clears per-unit flags", `inventory.md` §6.3 r4, without naming them), so a later
queueing sends 0x15 again and, in a walk mode, 0x0F again (`TODO(spec)` at
`update_messages`): Ghidra `0x00553220`. WS3 [F3] the dispatcher's range checks
(`point_state`, `unit_target`) read `SimGame`'s staged `UnitFacts`, not the path
record; with the provider on they go stale after a walk (the e2e re-stages
them): answer from `ActionHooks::path_position` (needs `&D` in
`Intents::point_state`). WS4 [F4] run velocity equals walk velocity
(`attach_run_stats`, `pathing.md` §8.2, `wire-path-sim` §5). WS5 [F5] a same-level
waypoint travel with two rooms sends no 0x0D: the arrival's spawn search
returns another room than the placement used (`wire-path-sim` §6). PU1
[`path-update-pass` §4.1] rule 2 depends on the client order: unit flag 0x1 is
never cleared (WS2) and each per-client update queues its own player at the
end (`tick.md` §6.5), so a client updated *after* the walking player's client
sees the player queued on every tick and gets 0x0F every tick while the mode
stays 2 / 3 / 6; the tests pass only because client 1 runs before client 0;
settle with `0x00553220`; the answer may change the expected per-tick bytes of
every walk test (S8-A1). PU2 rule 4 (monsters 0x67 / 0x68 through `0x00598220`)
has no spec owner; `update_messages` sends nothing for non-players.

**Property fixes and walk properties** (`prop-fixes`, `prop-walk-gaps`): SL1
[Q1] `stat-lists.md` §1 / §8: the spec gives the null-list rule for readers and
base writes only; d2rs makes attach, detach, free, the toggles and the field
accessors no-ops on a stale handle (`TODO(spec: stat-lists.md §1)` on
`ListId`): state a rule or an error? An unverified behaviour choice, not a
fidelity claim. SL2 [Q2] `stat-lists.md` edge case 4 / open question 5:
`StatListError::EndlessExpiry` is d2rs's answer to the endless loop; the spec
should name it beside the edge case (the original's state is the one the error
leaves). SL3 [Q3] `rooms.md` §8.2, PW1: which unit fields `0x0061A840` sets
(flag 0x800000 at +0xC4?, flag-ex 0x20 at +0xC8?), what "its path updated"
does, which units are "client units"; then `UnitLists::free_room` applies it.
SL4 [Q4] `intents-events.md` §2.4 r3: `in_range` is now the exact difference;
the original subtracts in 32 bits, so at |dx| ≥ 2^31 its wrapped result could
differ (unreachable: positions are server-staged, FS3); the other copies in
`skills/use_` and `world/waypoints.rs` still subtract `i32`. PG1 [prop-walk-gaps
PG1] spec reading, not a bug: the arrival check's "index ≥ count" branches
(`pathing.md` §9.5 r2 re-path with finish 0, r3's final re-path) are
unreachable from §9.4: the step that makes index = count also resets the path
(count := 0) in the same tick, and a blocked step sets index := count then
resets too; they run only on a path whose index other code sets (monster AI):
a line in §9.5 when the AI spec arrives. PG2 `pathing.md` §6 r4 lists the
missile's collided-mask query before the footprint move (code and model follow
it); the order is observable when the old and new footprints overlap and the
move mask meets the footprint mask; confirm `0x00650910`.

**Test installs and live-host tests** (`fixedset-game`, `game-tests-wired-host`):
FG1 `ActCreation::Full` needs a synthetic levels table shaped like Act I (ids
1–39 with the chain's types and outdoor data); under `TownOnly` the DRLG seed
skips the act placer's draws, a fixture state. FG2 no server message reaches
the idle joined client in 50 frames (no join sequence is wired); the test
records the count (0) and compares runs. GH1 [F1] population "on" places
presets only (`population.md` §3 reads the coordinate lists through
`WorldPending`, which no spec provides), so the Blood Moor likely has no
monster in reach and step 5 prints `kill: no monster within range`. GH2 [F2]
no walk handler existed when the test was written: **stale now** (§2 step
7u(a)). GH3 [F3] no skill-use provider on a live host: `ActionWorld<WiredSkills>`
needs `X: SkillRest` (`Pending + UseRest + LearnRest`), implemented only by the
synthetic fixture, so the live host uses `NoSkills` and 0x3C / 0x0D are stubs.
GH4 [F4] join placement: no spec places a joining player (`levels.md` §10
spawn room is not wired at join); the test stands the player at the town
waypoint ± 5 sub-tiles; a start on a blocked cell shows as a walk that never
moves. GH5 [F5] the town waypoint is allocated from the town's preset list
(`create_object` is a default `WorldPending` that creates nothing); if no town
preset has `OperateFn` 23 the fixture panics (then check `act_presets(0)`).
GH6 [F6] "Blood Moor exit" = the shared edge of the two generated level rects
(expected west edge x = 960 per `outdoor.md`, not asserted); the path search
may stall at the palisade (activation of the neighbour level's rooms,
`rooms.md` §5 rule 3, or the greedy search limit, `pathing.md` §5.2). GH7 [F7]
the expected 0x0D x, y are read after the 0x49 frame's tick; §7 rule 7 sets
mode 2 at the player's own position, so the tick should not move it; if the
bytes differ by a step the arrival walk moved the player.

**Mutation testing** (`mutants-*`): no code was found wrong against its spec.
MI1 [`mutants-inventory` §4] `inventory.md` §7.6 says X "becomes the cursor
item" and then N goes in "as §4.6 step 5", whose step list includes "Cursor :=
none": read literally (and so coded) the player ends 0x1B with no cursor item
while X is in mode 4; check against `0x00563D20` or a recording. MI2 the
`0x1E` gate (`handlers.rs:507`): a refused gate and the `swap_1h_with_2h` seam
both end in result 0 (`0x00561220`'s body is unwritten, OQ 14). MI3 the 33
`MovePending` default bodies are the narrowest readings of unwritten owner
specs (`impl-moves` §4). MW1 [`mutants-world` §3] `cube.md` §7 gives the fillers
list "max 18" but not what a 19th does (the original's stack array would
overrun; the code keeps 18, `fillers.len() < 18` → `<=` survives): state it or
say it cannot happen in 1.14d data. MW2 `npc.md` §7.2: the `first` argument of
`send_hire_list` has no described effect (six survivors): confirm it is unused
in 1.14d or describe it. MW3 `VendorTables::from_fixed` has no game-tier test
(C63). MT1 [`mutants-items-treasure`] `properties.md` §5 r6 for a cap < 1
(`TODO OQ-P2` in `items/props.rs`): "return 0" or "set stat 194 := 0"; and
`treasure/walk.rs` maps a TC index ≥ the count to `NoTc` (tested via §2's
"none"). MT2 `generation.md` §3 r9: no path through `create_item` sets flag
0x1000000 before step 9, so `personalize` is unobservable. MK1 [`mutants-combat-
skills` §4] `damage.md` §3.1 step 6 says "a hireling of any type", the code asks
the hireling test (`0x0063EE90`) only for monsters: a difference only if it can
be true for another unit type. MK2 `hit.md` §3.3: the first `def < 0` step never
changes the outcome (for values without 32-bit overflow; a wrap at `i32::MIN`
was not searched): a line in the edge cases if confirmed. MC1 [`mutants-core`] the
conformance replay killed none of the 149 sim survivors (stats, regeneration
and the stat-list chain have no trace: S8-A4); `StatTable::from_fixed`'s deps
terminator stops at the first id ≥ n where the spec says "until 0xFFFF" (the
same on every fixed-up table).

**Formats, builders, coverage** (`mpq-huffman`, `s2c-use`, `spec-unit-numbering`,
`ci-nightly-props`): MH1 which weight tables the 1.14d `.wav` sectors use and how
many contain an escape decide whether more decoder work pays (C62). S2C1
`d2-sim` → `d2-proto`: option A (keep the rule) is taken as a decision (PLAN);
options B (lift the rule for `d2-proto`: pure, only `thiserror`, but it also
holds `transport`, which the rule kept out) and C (move `s2c` +
`generated::server` into a transport-free crate allowed for `d2-sim`) remain
the user's to choose. S2C2 `bridge-dispatch.tsv` stays all `TBD`: no client
spec takes an S→C id (`bridge.md` §6 rule 2); writing `Message` variant names in
the `owner` column would break its rules 1 and 5; a fourth column is a spec
change to `bridge.md` §6 rule 1 plus `dispatch::parse`. SU1 the sections still
one large unit and the unnarrowed `§edge-cases-original-bugs` claims (step 7u
(e)). NP1 the `wire` and `worldsim` deep groups were not run at full counts;
GitHub runners may be 1.5–2× slower than the measured 670 s.

## 8. Lessons (problems met, fixes)

| Problem | Fix |
|---|---|
| Ghidra 12.1 crashes on JDK 25 | use JDK 21 (`JAVA_HOME_OVERRIDE` in Ghidra's `launch.properties`) |
| Cargo can't extend workspace lints | `d2-sim` repeats them in its own `[lints]` |
| Running `cargo` outside the repo reinstalled `stable` (1.3 GB) | default toolchain set to 1.99.0 |
| `.txt` tables in the MPQs aren't what 1.14d uses | `.bin` is truth; the txt compiler is proven against it (`loading.md`) |
| 1.14d inlines its RNG step (846 sites) | recorder hooks every site, not one function (`sim/rng.md`) |
| A 24-agent review workflow cost ~6M tokens for little gain | one writer per spec + executable checks (`CLAUDE.md` working rules) |
| Two parallel cloud branches (fix-ups, patch layers) compiled alone but not together: `fixup::apply` gained an `AnimData` argument while `patch_game.rs` was written against the old one (caught by the coordinator's merge gate, 2026-10-06) | the coordinator merges parallel branches together and runs the full gate before opening the PR; a session changing a public signature names it in its handoff (METHODS M21 escape log) |
| The Ghidra decompile drops register arguments (fastcall ECX/EDX, custom conventions) | read register use from the disassembly: `tools/ghidra/disasm.py` |
| Recording a game needs a player: the game never enters a game by itself | ask the user to play during the recording (~3 min) or queue it |
| A local session reported a push as done; the push had been rejected (local and remote branch names differ) and the command's last output line hid the error (2026-10-06, caught by the coordinator reading the remote) | push with an explicit remote branch (`git push origin HEAD:claude/<name>`) and verify by reading the remote (`git status` not ahead, or `git ls-remote`), never by the command's output (METHODS M09, M21) |
| A coordinator removed a writer's worktree right after the writer reported, while the writer's own helper agent was still running in it; the helper lost its shell and its findings arrived only as a message (2026-10-06) | remove a worktree only when `git worktree list` shows it unlocked and no notification of that writer's helpers is pending; findings saved to `re/exports/requests/skills/vitals-findings.md` for a follow-up (METHODS M21) |
| Property tests on the strict parsers (2026-10-06) found 16 bugs that valid files never hit: process aborts from `Vec::with_capacity` on untrusted sizes (MPQ explode/huffman/adpcm and `read_block`, animdata bucket count), debug-build overflow panics (ds1 and dcc size products, animdata `hash`, huffman weights, `.bin` size check, dc6 `frame` / cof `component_at` indices), quadratic or huge work from shared offsets (dt1 block headers, tbl strings, dc6/dcc frame boxes, tbl probes up to `max_tries`), a wrapped DCC i32 corner, and a `patch::apply_stack` `expect` reachable through the public API after a failed `table` line | every size, count and offset from a file is checked or bounded by the input length before it drives an allocation, a product or a loop; `cargo test` runs the properties (M07) |
| A cloud session committed Python bytecode (`tools/trace-recorder/__pycache__/*.pyc`, coverage branch, 2026-10-06); caught by the coordinator reading the merge diff | `.gitignore` covers `__pycache__/` and `*.pyc`; the coordinator lists non-source files in every merge diff before the gate (METHODS M21) |
| The coordinator's union merge of `pub mod` conflicts interleaved two branches' module doc comments (`d2-client/src/lib.rs`, `d2-sim/src/wiring/mod.rs`; 2026-10-06), once leaving a `//!` after an item; caught reading the merged file before the gate | after any union resolve, read the whole resolved file, not only the `mod` lines; inner docs (`//!`) must precede every item (METHODS M21) |
| A coverage claim on a test that checks only part of a rule overstates the unit tier (10 claims dropped on review, 2026-10-06; rules that are one unit make this easy to repeat) | claim a rule only when the assertions check its outcome, with the narrowest ID that is fully true; consistency checks against a TSV and M08 perturbation tests get no claim (`docs/handoff/coverage-claims.md` §1) |
| A software Vulkan adapter (Mesa llvmpipe) passes every GPU case, which says nothing about a real driver's integer and texture paths (2026-10-06) | the GPU half of every Phase 6 check stays "unverified" until the local run on a real adapter records its name, backend and driver (§5 C15, C16; METHODS M02) |
| Five game-file assertions written without game files (`gaps-data-formats`, 2026-10-06) and two spec facts stated without a measurement (`animdata.md` "second copy in all 9", `dc6.md` "zero-size frames occur") failed on the first local run of C17 | a blind-written game assertion is marked "expected value unconfirmed" in its handoff and is not claimed (`COVERAGE.md` §3) until its first local run; a spec fact names its measurement or is an open question (`specs/README.md` bar 1; METHODS M21) |
| Both spec branches of 2026-10-06 would have failed a merge gate: `spec-inventory` edited `server-messages.tsv` without `gen-proto`; `spec-path-placement` repeated two rule ids (`coverage.py --check`) and added a `bits:` layout syntax the d2-proto parser rejects (caught by the cloud coordinator) | `specs/README.md` Process: the pre-push checks and the TSV-grammar rule; the local coordinator runs them on every writer branch before reporting it (fixed on `claude/spec-path-placement` `3fe068b`: 0x96 layout moved to `pathing.md` prose; METHODS M21) |
| Three parallel d2-server handler branches each added `[dev-dependencies] d2-data` and a field/generic to `SimGame`; git merged the two `Cargo.toml` sections silently into a duplicate key and the `SimGame` generics conflicted (2026-10-06, caught by the coordinator's build before the gate) | parallel sessions that extend a shared struct get one named owner per field in their prompts; the coordinator builds the touched crate after each merge, not only after the last (METHODS M21) |
| `wire-interaction` added a seed parameter to the `NpcLink::make_hire_list` seam while `server-world` (in parallel) wrote a test fake against the old signature; each branch was green alone (2026-10-06, caught by the coordinator's workspace clippy on the combined branch) | sessions that change a seam signature name it in their notes under "signature changes"; the coordinator greps other branches for implementors before merging, and always runs clippy/tests on the whole workspace after combining (METHODS M21) |
| A spec branch (`claude/spec-inventory`, 2026-10-06) changed `specs/sim/server-messages.tsv` layouts without regenerating `crates/d2-proto/src/generated.rs`; three d2-proto tests failed after the merge (caught by the coordinator's full gate, not by `cargo check`) | after merging any branch that touches `specs/sim/*-messages.tsv`, run `cargo run -p data-tool -- gen-proto` and commit the result in the merge; spec sessions that edit the TSVs regenerate in the same commit (METHODS M21) |
| `test-fixtures` tests built the synthetic install into one fixed `CARGO_TARGET_TMPDIR` directory behind a `OnceLock`; CI's nextest runs each test in its own process, so processes raced on the same archives (`failed to fill whole buffer`, PR #21). The local gate passed because `cargo nextest` was not installed and `tools/gate.sh` fell back to `cargo test` (one process) | test scratch paths include `std::process::id()`; coordinators install `cargo-nextest` so `tools/gate.sh` runs the same runner as CI (METHODS M21) |
| `ci-speed` split CI's single `check` job into five; main's branch protection requires a status named `check`, so PR #21 was green on every job yet could not merge (2026-10-06) | an aggregate `check` job (`needs:` every job, fails unless all succeed) keeps the required name; a CI change that renames or removes jobs checks the branch-protection required checks first (METHODS M21) |
| The coordinator's merge script committed each merge before its `cargo check`; on a failed check the follow-up `git merge --abort` was a no-op, so a branch that did not build stayed in the history and was pushed (`prop-path-place` against `wire-path-sim`'s seam cleanup, 2026-10-06; reset and force-pushed within minutes, no PR open) | merge with `--no-commit --no-ff`, build, and commit only when `cargo check --workspace --all-targets` passes, else `git merge --abort` (METHODS M21) |
| A property test passed locally twice and failed only on CI's random seed (`stat_lists_match_the_model`, PR #24); a green run merged the PR with the bug still live | a property's counterexample is a real bug, never a flake: reproduce it as a fixed regression test and fix the wrong side per the spec (`fix-statlist-prop`) |
| PR #27 (a fix branch opened from an older main) was green on every job, `check` included, yet GitHub refused the merge ("Required status check `check` is expected"): main's branch protection requires the PR branch to be up to date with main (2026-10-06) | before opening a PR from any branch, merge the latest `origin/main` into it (a merge commit, never a rebase) and let CI run on that head; the coordinator's branch always is, a session's fix branch usually is not (METHODS M21) |
| `prop_transport::inbox_any` failed on one random seed in the coordinator's gate (2026-10-06): `Inbox::deliver` returned `BadSize(537)` for an id 0x94 split whose size rule exceeded 0x204, which is the original's assert (`intents-events.md` §3.3 r2); the property's model allowed only `BadId` | the model was the wrong side again (as `fix-statlist-prop`): before writing a property's error arms, list every error the spec's rules can raise on that input range; a seed-only counterexample becomes a deterministic regression test (`delivery_asserts_on_an_oversized_split`) |
| The `s2c-use` task and `s2c-builders` §5 said depcheck allows `d2-sim` → `d2-proto`; `tools/depcheck` `FORBIDDEN` has forbidden it since Phase 0 (found by the session, 2026-10-06): an unscoped claim about a rule crossed two handoffs unchecked | a task that depends on a layering rule quotes the line of `tools/depcheck/src/main.rs` it relies on; lifting a rule is an architecture decision (`CLAUDE.md`), recorded in `docs/PLAN.md`, never made by an implementation session (METHODS M09) |
| `game_wired_host.rs` (`game-tests-wired-host`) was written on a base without the walk handler and asserts the stub for 0x03; `wire-path-server` merged next and made the assertion false before the test ever ran (2026-10-06, found while folding) | a blind-written game-file test names the handler state it assumes; the coordinator greps the `#[ignore]` tests for stub assertions when a handler lands (§2 step 7u(a)) |
| A full `cargo mutants` run takes about 2 h per 1,900 mutants with `-j 3` on a 4-core container, a background command is stopped after 2 h, `pgrep -f` / `pkill -f` match their own shell, and a `cargo install` beside the toolchain's first install corrupted the 1.99.0 toolchain (rustup lost `rust-std`, 2026-10-06, `mutants-combat-skills` §6) | start the run detached (`setsid nohup … &`) and use `--iterate` for the survivors; match processes by name (`pgrep -x cargo-mutants`); let `tools/cloud-setup.sh` or the first `cargo` call finish before a second cargo |
| The first deep nightly run (`ci-nightly-props`, 20,000 cases) found two counterexamples PR CI never reached: the stat-list model failure and `anim_schedule_matches_the_closed_form` aborting with "Too many global rejects" (a `prop_assume!` rejecting too often at high case counts, 2026-10-06) | a `prop_assume!` is a rejection budget: constrain the strategy so it holds at 20,000 cases (`schedule_inputs()`), and run `tools/props-deep.sh` after adding a property |
| The coordinator merged four local spec branches that change code-mirrored tables (`ai-functions.tsv`, `functions.tsv` / `srvdo.tsv` / `srvhit.tsv`, `bridge-dispatch.tsv`) onto its integration branch before their implementations (2026-10-06): every session based on it saw ~55 red tests (bridge `Dispatch::from_spec` Mismatch, AI / skill catalogue checks) that were not its own | a spec branch marked `CODE-TABLE CHANGE` waits on `claude/specs-staging` and is merged into the integration branch together with (or after) its implementation branch; spec-only branches merge at once |
| TC 0 `picks` built as 1 in d2rs, 0 in 1.14d memory (made in `treasure/runtime.rs`, spec silent; caught 2026-10-06 by C20's byte compare with the memory dump; `check_treasure.py` checks only TC 430) | whole-table byte compares against `dump_tables.py` dumps for every table d2rs builds (C20 test `game_treasure_dump.rs`) (M21) |
| Spec text wrong and code built on it, found by disassembly in the 2026-10-06 spec-answer batch: `npc.md` §8.2 stat / skill reset addresses swapped; `pathing.md` §9.10 re-path budget field (+0x94 monsters, not +0x90); position history "never read" (AI reads it); `path-placement.md` §12.1 exit 10 / 11 direction swapped; `inventory.md` targeting probe gets the player, not the item; `0x0055C110` named "charm re-link" from its callers (it handles scroll / tome charges) | name a function only after reading its body; when a spec says "nothing reads X", attach the xref that proves it; test vectors that use both directions / both owner kinds (M21) |
| `mpq-tool formats` counted a file twice when two listfiles spell it in different case (DC6 1,657 vs 1,653, DT1 260 vs 256), and those counts became `game_sweep`'s expected values and a HANDOFF explanation (caught 2026-10-06 by a spec session's own count) | case-insensitive name sets wherever MPQ names are collected; a count copied into a test names the tool and the method that produced it (M21) |
| A spec's prose count drifted from the tool (`mpq.md` / `audio.md`: "5,008" `.wav`, `mpq-tool check` 4,992) | counts in spec prose name the command that measures them (README bar 1) |
| Writers on one PC shared one scratchpad directory; one writer's helper script was overwritten by another's mid-run (2026-10-06) | give each writer its own scratch subfolder in the prompt (M21) |
| GPU render exactness | R8Uint indices, sRGB palette via `textureLoad`, `Msaa::Off`, `Tonemapping::None`, pixel-aligned quads |

