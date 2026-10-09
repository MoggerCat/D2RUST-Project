# Handoff (updated 2026-10-07, branch `claude/fold-handoff-10` on `claude/specs-staging-6` at `7fa1a02`: the tenth fold (impl-pc1-final, impl-pc1-wiring, impl-pc2-fixes, impl-world-rest, impl-session-flow, impl-path-motion, local2-2026-10-06, impl-server-join-2, impl-monster-death, impl-client-msgs-3, impl-client-drlg-2, impl-d2s-load-hirelings, impl-monster-skill-slots, impl-quests-answers, impl-render-missile-answers, impl-umods-cs-handlers, fix-drlg-answers, e2e-night-flows, fixedset-act1, gaps-night-specs, mutants-night-code, mutants-path, mutants-walk-handlers, pc2-spec-d2s, pc2-spec-hirelings, pc2-spec-quests-act2, pc2-spec-quests-act3, pc2-spec-quests-core; 28 notes in this fold, 165 folded in all) on top of the ninth fold (branch `claude/fold-handoff-night` on `main` at `674996d`: impl-ai-acts2-5, impl-skill-slots-2, impl-missile-bodies-2, impl-quests-act1-rest, impl-quests-act2, impl-quests-act3, impl-quests-act4-5, impl-objects, impl-hirelings, impl-drlg-act3-5, impl-room-population, impl-client-staging, impl-lighting-blend, impl-draw-order-2, impl-ui-panels, impl-audio, impl-d2s, wire-client-staging, wire-world-staging, impl-client-msgs-2, impl-client-drlg, impl-server-join; 22 notes in this fold, 137 folded in all) on top of the eighth fold (`claude/docs-fold-8`: ci-nightly-props, unify-items, mutants-inventory, mutants-combat-skills, mutants-core, s2c-use, spec-unit-numbering, prop-fixes, path-update-pass, prop-walk-gaps, fixedset-game, mpq-huffman, drop-freespot, wire-path-server, game-tests-wired-host, mutants-world, mutants-items-treasure; 17 notes))
- **REC-267** Cube transmute animation and open latch (PROVISIONAL, `d2-client` `ui/cube_ui.rs`, `ui/msg_ui.rs`, `ui/original.rs`; M22; q-cube-transmute, `// d2rs-own, unverified`): (1) `panels.md` §12 r4 gives the start routine `0x0048A540` but not its caller; the preview starts the animation at the transmute button release (the same moment 0x4F 0x18 leaves), stops it on any cube close and on a new open. (2) The 70 ms wall-clock step runs on the client frame tick at 40 ms per frame (as `game_messages`), so a frame is held 2 ticks (80 ms). (3) Draw mode 3 and light 0xFF are not carried by `ImageRequest`: the cel `menu\horadric` frame n is requested at (W/2, H/2−1) and the sink must apply mode 3. (4) S→C 0x77 0x15 now calls `StashCubeInput::cube_opened` (latch clear) through the next cube event. Needed: the animation's caller (a trace of a transmute: tick of the first frame vs the 0x18 and the result items), a render of the frames.

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

The ninth fold (2026-10-07) took the 22 notes the night's sessions landed on
`main` (PRs #35–#38 through `claude/specs-staging`; list in the title line).
Facts that §1–§3 and §7 stated before change in these places: every
`spec'd-here` AI function (93 of 148), skill slot (130) and every non-null
server-do / server-hit body has code, and Acts I–V quests, world objects,
hirelings and the Act III / V DRLG exist (rows 3aj–3am; read the "3 not
implemented" row and step 12 as rewritten); rooms populate from the act DRLG
(F1 / GH1 closed); the in-process server sends the game part of the join and
the bridge owns a client DRLG (rows 3ao, 5, 6; step 4); 53 of 181 S→C ids
have an owner and a handler (read "every S→C id still unowned" as stale); the
original UI and audio run in `play` with game files; the 47 `d2-client` and
ten `d2-sim` reds the night's notes report are gone (workspace green at
`ef00681`). Every open question of the 22 notes is in §7 "Ninth set", split
by answerer: **PC 1** (RE / spec answers, 288) and **PC 2** (recordings and
captures, 38; S9-A1 is PC 1's recording list). The §5 queue numbers C65–C86
were already unique and in order; the fold adds C87–C91 and S9-A1–S9-A7. The
other 34 notes without a pointer line (`impl-ai-act1`, `impl-quests-act1`,
`impl-skill-bodies`, `impl-ui-text`, `impl-draw-order`, `impl-unit-composite`,
`render-followups-impl`, `local-buddy-*`, the 2026-10-06 `mutants-*`, …) are
not in this fold. Coverage at this fold (`python3 tools/coverage.py
--summary` on `674996d`): 8,149 claims over 7,342 rule units, unit 5,846
(79.6%), game-file 270 (3.7%), trace 61 (0.8%), verified 331 (4.5%), any
tier 5,934 (80.8%); the share fell because the rule units grew by about
4,000 since the eighth fold (the specs that landed for these sessions).

The tenth fold (2026-10-07) took the 28 notes that arrived after the ninth
fold's merge (`913d3b0`) through PRs #39–#41 and the three staging merges after
them (list in the title line; `xpc-to-pc1.md` / `xpc-to-pc2.md` are live request
queues, not folded). Facts that §1–§3 and §7 stated before change in these
places: the single-player session is C→S 0x67 → 0x6B → first tick with the
join's messages (rows 3ap; read the row 3ao "Not sent" list, §7 SJ-2–SJ-5 and
MB-8 as closed); monsters walk and run on the path provider and missiles fly
on it (row 3aq; read WP1, "walk / run for monsters" and "missile flight on the
provider" as closed); every `spec'd-here` AI row (148), every referenced skill
slot (213) and the umod callbacks have code, and kill experience, player death,
the unit-event registry and `combat::events` are wired (row 3ar; read WI10 and
"kill experience ... nothing calls it" as closed); the chest drop, the
shrine / well / misc providers, quest inits inside the allocation, the hireling
death caller, the `.d2s` load order and the wall-remap table exist (row 3as;
N-2 / N-3 / N-4, QA-1–QA-6, WW-6, WW-10, GN1, HL1–HL9, DS-1–DS-5 are closed);
I-1 is settled as owner @5 / pet @9 on both sides; the nine ids of CM-1–CM-6
have handlers, `Dispatch::from_spec()` builds again (the 85 `d2-client` reds of
the base are gone) and 167 of 181 S→C ids are owned (row 6, tenth fold). The
five reds that the staging-5 notes report on their bases (`monsters::ai::tests::
specd_here_*` ×2, `scenario-run` ×3) were corrected by `impl-pc1-final`; its
final gate and the gate on the merged head are not recorded in any note, so the
first step of §2 7w is to run `CARGO_INCREMENTAL=0 sh tools/gate.sh`. Every
open question of the 28 notes is in §7 "Tenth set", by answerer: **PC 1**
(`xpc-to-pc1.md` plus the notes' own lists named there) and **PC 2**
(`xpc-to-pc2.md`, recordings R2-39–R2-46). The §5 queue adds C93–C98; the C
numbers were unique and in order up to C92 (added after the ninth fold by
`fix-drlg-answers`). The other 35 notes without a pointer line (`impl-ai-act1`,
`impl-quests-act1`, `impl-skill-bodies`, `impl-ui-text`, `impl-draw-order`,
`impl-unit-composite`, `render-followups-impl`, `local-buddy-*`, the 2026-10-06
`mutants-*`, …) are not in this fold either. Coverage at this fold
(`python3 tools/coverage.py --summary` on `7fa1a02`): 9,412 claims over 9,246
rule units, unit 6,729 (72.8%), game-file 284 (3.1%), trace 61 (0.7%),
verified 345 (3.7%), any tier 6,817 (73.7%); the share fell again because the
rule units grew by about 1,900 (the PC 1 / PC 2 spec batch: `skills/bodies-3.md`
271 and `bodies-4.md` 187 units, `monsters/umod-init-bodies.md` 26, the
`tools/original-hooks*.md` specs, `ai-bodies-6.md` / `-7.md` 354 units, with
no unit test for the first three yet).

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
ninth set by answerer: PC 1 I-1–I-7, AI, SK, MB, QA–QE, OB, WW, HL, DR, RP,
CS, CD, SJ, CM, LB, DW, UP, WC, ST, TR, EN, DS; PC 2 R2-1–R2-38, plus
"Resolved in the ninth fold"; Phase 6 by owner spec) ·
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
| 3z Inventory on the wired host (`wire-inventory-sim`, `wire-inventory-server`) | **Wired, unverified** (`inventory.md` is a draft, no recording R1–R6; the adapters add no rule). `d2_sim::wiring::inventory` (new module, one line in `wiring/mod.rs`; no change to `items::inventory` / `items::moves`): `InvDesk` (borrows `Economy`, the `InvTables`, an `InvState` and a rest `R: InvRest + ?Sized`) implements `InvWorld`, `InventoryOps`, `MoveUnits` and `MovePending`, so `items::moves::handle` and `player_update` run on real state; one owner per field (`UnitRecord`: mode, unit flags, update bits; `ItemStore`: item flags, page, quality; `StatLists`: quantity, gold, durability, sockets; `InvState::items` / `expiry`: command flags, body location, grid x / y, owner GUID, node; `UnitLists`: room membership). Seams without a provider stay `InvRest` / `MovePending` defaults (path and placement, rooms, stat-list routines, requirements and hands, player data, belt / sound / quest hooks, item creation and use, sockets, hirelings, picks and transport; table in `wire-inventory-sim.md` §5): auto pickup of an item with a body location stops at `equip_picked`, 0x26 leaves the item in the belt, no gold pile is made without `gold_request`, ground items without a `free_spot` never drop. **Server:** the 23 ids of `items::moves::HANDLED` (C→S 0x16–0x29, 0x50, 0x61, 0x63; 0x4C stays unowned, `cube.md` §10) run `SimGame::handle` → `handlers::items::moves::handle` → `WorldHost::moves` (new; `WiredWorld::inventory: Option<InvParts>`, parts lent out for the call) → `InvDesk` → `items::moves::handle`; result 0–3 → `ResultCode`, a `MoveFatal` or a queueing failure → `WorldError::Move` and `Malformed`; direct sends (0x63's 0x9D action 5 / 0x9C action 0xE, a stack merge's 0x42, the targeting reset's 0x3F) leave through `MoveRest::take_sent`; the deferred item messages run in `handlers::items::moves::update_pass`, called by `SimGame::tick` after the tick and the host's tick sends (per client, per player in the client room's adjacent rooms: `player_update` → 0x9C / 0x9D / 0x7D, 0x47, 0x48; then `InvDesk::update_done` and +0xC8 bits 0 and 1 cleared; failures to `tick_faults`); `MOVE_IDS` names `inventory-moves.md §7.x` for each id in `ITEM_IDS`. e2e (`e2e_single_player`, `e2e_support::InvFx`): step 5b, pick-up of the kill's gold (0x16), runs the handler and **stops** (asserted: result 1, the gold's record is in `DeathDrops::items`, not the host's store: HM5 / IS4); new steps 16–21 (frames 26–31) on a cap made in the host's store: 0x16 → 0x9C 1, 0x18 → 0x9C 4, 0x19 → 0x9D 5, 0x1A → 0x9D 6, 0x1C → 0x9D 8, 0x17 → ground (nothing sent), each frame's bytes exact, plus 0x47 / 0x48; client 33 frames, 32 ticks | sim: 26 integration tests in `wiring/inventory/tests/` (every handler through `items::moves::handle` with real message bytes, then the update pass; fake only `Rest`); M08: `ground::pickup_position_follows_the_grid_record` and each of 7 adapter routes broken by hand failed its tests. Server: 22 host-frame tests in `handlers/items/moves/tests.rs` (result codes, state and the exact S→C bytes of every id, 0x29's fatal → `Malformed` + fault, 0x61 classic → 3, the stub without inventory parts) and `move_ids_match_client_tsv_and_the_module` with `move_ids_check_reports_perturbations` (M08). `e2e_single_player` 3 pass, `same_seed_same_run` and `other_seed_other_run` pass. Gate PASS (coverage 3,542 then 3,813 claims, 0 errors; the sim note: d2-sim + conformance 1,445 passed; the server note's merge failed once only on the known seed-dependent `stats::prop_tests::stat_lists_match_the_model`, fixed by `fix-statlist-prop`, rerun 1,548 passed). Open: WN1–WN4, IS1–IS6, §2 steps 7o / 7i |
| 3aa Path core, walk and the path wiring (`impl-path-core`, `impl-walk`, `wire-path-sim`) | **Implemented and wired, unverified.** `d2_sim::path::{coords, record, collision, footprint, tables}` (`path-placement.md` §1–§6: sub-tiles, 16.16 centres, `DynamicPath` / `StaticPath` records, the `CollisionRooms` seam, point / plus / box / size / pattern queries, pattern and size footprints, try / forced / missile move, teleport; `PathTables` = all 18 tables of `path-tables.tsv`, embedded, strict parse; 26 tests, 33 of 84 units claimed) and `path::walk` (`pathing.md` §1–§10 for path types 1, 2, 7: request and mode checks, A* / toward / straight, velocity, step, cell walk, set position, reset, room-change messages, the builders of 0x0D, 0x0F, 0x10, 0x15, 0x96; no floating point, the x87 target lead is the seam `WalkUnits::target_lead`; 34 tests, vectors W1–W6, W9, D1–D4, M1, M2, S1, S2 exact, 105 of 125 units claimed; V1–V3 follow the formula, PQ1). No per-tick position trace exists (`pathing.md` OQ1, `path-placement.md` OQ1), so nothing is checked against 1.14d. `wire-path-sim` folded the duplicate seams, tables and types of the walk onto the core (`Walk<'a, C> { t, c }`, `WalkUnits` without `Game` arguments, `PathWorld: CollisionRooms`; list in the note §3) and wired it as `d2_sim::wiring::path::{PathState, PathCtx, rooms, units, walk, place}`, **opt-in per game**: `ActionHooks::paths: Option<Box<PathState>>`, `None` by default, `ActionHooks::enable_paths()` turns it on (off: every path seam keeps its `Pending` answer and all earlier tests are unchanged; on: `Pending`'s path methods are not called). Path records (unit +0x2C) live in `PathState::records`, created by the allocation's path part (§2.5) and freed by the unit removal; walk / run schedules the every-tick event 0 and the unit dispatch calls `UnitHooks::player_movement_step` → `wiring::path::walk::player_step`; the `CollisionRooms` provider is `DrlgWorld` (new `Drlg::active_grid_mut`); `Pending::warp` (same act) → `place::level_warp`, `Pending::set_player_mode_arrival` → `walk_to`, `WorldPending::nearest_free_point` → `place::coarse_free_box`, the missile collision queries (settles wire-action W5 / WG7 when on) and `AiWorld::collides` → the core. `path/mod.rs` now declares the whole module (J12 resolved) | core 26 + walk 34 + `gaps-path-render` 8 tests (`pathing.md` 106 → 114 of 125 units, 91.2%; no code fix: no test showed a deviation); wiring: 7 tests on the action fixture (`allocation_gives_the_unit_its_path_and_footprint`; walk M1 translated: x = 0x1AE000 + k·0x6000, tick 14 lands on 0x1F8000; room-edge crossing; a wall changes the walk; waypoint warp puts the player in the spawn room, the e2e step-6 condition; without the provider the warp stays pending; coarse free box avoids a monster footprint), all with an M08; `cargo test -p conformance` tick replay unchanged. `prop-walk`: 7 properties on the public walk API, **no bug found** (PX1 is a spec wording), 10,000 cases debug, 20,000 release; `prop-path-place`: 10 properties on §7–§12, one overflow fix in `path/warp.rs` (`wrapping_add` / `wrapping_mul`, PF1), `PROPTEST_CASES=2000` in ≈ 70 s. Gates PASS (impl-path-core: d2-sim 1,265; impl-walk: 1,273, coverage 3,403 claims; gaps-path-render: d2-sim + conformance 1,556, rest 567, d2-client 360, coverage 3,869 claims, 0 errors). Not done: three blockers WP1–WP3 and the list of §2 step 7p |
| 3ab Conformance harnesses for movement and placement (`conformance-path-render`) | `crates/conformance`: `movement::{read_movement, replay_movement}` (walk / run from `packets-raw-1`: C→S 0x01–0x04 → `Mover::request`, ticks, S→C 0x0D, 0x0F, 0x10, 0x15, 0x96; the first 0x15 of a (type, GUID) seeds the mover; message bytes and per-tick positions compared; mismatches name the layout field), `placement::{read_player_placements, replay_placement}` (players from 0x15, room = the one 0x07 since the previous 0x15; `Spawn` records for monsters and items), `needs::{NEEDED, report}` and `cargo run -p conformance --bin recordings-needed` (7 entries: units, stats, packets, movement-walk, movement-path-state, placement-players, placement-monsters-items; MISSING / present / BLOCKED). **Proven on synthetic recordings only; no d2-sim provider behind either seam yet** (`NotWired` → `MOVER NOT WIRED` / `PLACEMENT NOT WIRED`, never a pass); no `Covers:` claim (a claim here is the `trace` tier). No recording format holds per-tick path state or placement calls (CR1, CR2) | CI fixtures `movement-walk.jsonl` (first 14 0x96 follow `pathing.md` M1; the test mover is a stand-in) and `placement-players.jsonl` (R1–R3 shaped), both pass `check_packets.py` R1–R7; perturbations: each of 42 0x96 (message and position channels), missing / extra / wrong client, changed target, dropped tick, second 0x15, every spawn × guid / class / x / y, wrong room; `recordings_needed.rs`: every named script / test exists, every ignored `needs traces/raw/` test is listed. Queue: §5 C55–C57, A-seventh |
| 3ac Camera, placement and the app's world view (`render-camera-placement`, `render-wire`) | **Implemented, unverified against 1.14d (no capture).** `d2_client::rules` (`camera.rs`: `FrameSize`, `OpenMode` 0–3, `ViewRect`, tile / unit origins once per frame with shake, `unit_draw`, handed tile positions, culling, `Shake` envelope and `shake_offsets` = two `roll_range(−a, 2a)` draws; `placement.rs`: `Cel` (`dc6` / `dcc`), `RowPlan`, `draw_position`, `place`, `block_pixel`; `view.rs`: `OriginalView<R, S>` implements `ViewRules` over wrapped rules and a `ViewSource`; minimal data change `IndexFrame.anchor: FrameAnchor` (`Top`, `Bottom` = DC6 `flip` 0, `TopDown` = DC6 `flip` 1), `from_dc6` refuses `flip` ∉ {0, 1}, `from_dcc` refuses an odd `variable0`). `render-wire` put it in the app: every frame is built by `world_view::build_frame` (camera once from a `ViewFeed`, then `build` over `OriginalView`); `WorldViewState` gained `feed: Box<dyn ViewFeed>` (`ViewFeed: ViewSource` + `player`, `open_mode`, `shake`, `player_seed`; the app's `NoFeed` states no player, no map tiles, no shake, so without a player there is no camera and the window stays black: `NoCamera` errors on any listed tile or unit); one frame per server tick (draw only when `ClientWorld::server_ticks` advanced; UI input queued to the next drawn frame; `FrameStats.server_tick`); the capture `scene` case has a real source (`verify/capture_case/scene_source.rs`: `WorldScene<W: CaptureWorld>` builds the camera from the record, checks the recorded `view_rect`, `shift_x`, `tile_origin`, `unit_origin` against `Camera::new` before any pixel, then asks the world for units, tiles, UI; today's recordings stop there: `NotRecorded` → `SCENE NOT WIRED`, exit 2, after the camera check ran). Seams stopped, not invented: RW1–RW5, RC6 | `rules::` 16 tests (camera §2 vectors, 800 × 600 origins 600 / 1720 / 600 / 1716, player (400, 292), culling −81 / −80 / 553, shake envelope; placement DC6 / DCC vectors; a CPU golden scene compared as the whole 800 × 600 index framebuffer, M08: a one-pixel camera move changes exactly the 12 expected pixels); `world_view::feed::tests` 3, `verify::capture_case::scene_tests` 5 + 1 ignored GPU, `app_frame_loop` 6 (new: one drawn frame per tick with the feed asked once per tick `[1..5]`, shake on the tick time base); synthetic capture `CPU 0 of 480000`, `--perturb 1 / 7 / 64` → exactly N, a recorded tile origin off by one → FAIL naming the value; llvmpipe `gpu_half_matches_the_synthetic_capture` 0 differing, perturb 7 → 7. Gates PASS (coverage 3,319 claims at the first note). Claims: camera §1–§10 + edge cases, sprite-placement §2–§5, §7, §8 (not §1, §6). Queue: §5 C52–C54, A-seventh |
| 3ad Tests and tooling, seventh fold (`bench-baselines`, `prop-client`, `prop-inventory`, `fix-statlist-prop`, `mutants-server`, `mutants-monsters-missiles`, `game-tests-inventory-path`) | **Benchmarks** (criterion, not in CI; `cargo bench -p d2-sim --bench sim`, `-p d2-formats --bench formats`, `-p d2-proto --bench proto`, `-p d2-client --bench compose`; feature `bench-fixtures` widens the `cfg(test)` of worldgen / stats / skills / items fixtures, behaviour-neutral). First numbers (cloud 4 vCPU Xeon 2.10 GHz, rustc 1.99, quick settings, order of magnitude; the tick budget is 40 ms): sim tick, 24 monsters × 200 ticks 46 µs; populated level setup 954 µs; DRLG act 0 509 µs, + the 40 × 18 preset level 528 µs; `create_item` ≈ 215 ns; treasure `walk` ≈ 330 ns / drop; stat lists (player + 12 item lists) 36.7 µs; MPQ read 256 KiB PKWARE + encrypted 267 MiB/s, 64 KiB plain 15.6 GiB/s; DC6 32 frames 17.8 µs; DCC 8 directions 70 µs; proto classify ≈ 15 ns, split 500 messages 3.0 µs; compositor `compose` 800 × 600 / 400 items 4.75 ms (101 Mpx/s), `compose_binned` 10.7 ms, `to_rgba` 0.73 ms. Hot spots: BB1 (MPQ Huffman about 3 MiB/s, not kept in the bench), the CPU compositor is the reference only, the sim numbers are floor values (idle monsters, no combat, pathing, missiles or clients). **Property tests:** `prop-client` (7 files in `d2-client/tests`: `prop_client_bridge`, `prop_world_view`, `prop_controls`, `prop_ui`, `prop_atlas`, `prop_scene`, `prop_cache`, plus `prop_support`; ≈ 25 s in debug; hunted at 3000–5000 cases, 400 for world view and atlas) found five bugs, each fixed at the root with a `regress_*` test (`ui/geom.rs` `Rect` edges past i32 in i64; `CellGrid::new` refuses a grid past i32 with `TooLarge`; `ScrollList::scroll` saturates; `Presentation::to_frame` saturates the bars; views past the i32 screen range are refused by `compose`, `compose_binned` and `bin` with the new `SceneError::View`; a sixth, `route` past 0x204 bytes, was fuzz-server's fix already on the base); `prop-inventory` (`d2-sim/tests/prop_inventory.rs`: state machine of the inventory model against a reference written from `inventory.md` §1–§5, 256 cases of 1–60 ops; `items/moves/prop_tests.rs`: every `HANDLED` id on arbitrary payloads, 512 cases; 20,000 and 30,000 cases pass) found one bug (`grid::in_bounds` overflowed `x + w` in i32, reachable from a 0x18 payload at x = 0x7FFFFFFF; now i64, the placement is refused, `regress_place_near_i32_max`); `fix-statlist-prop`: the CI failure `stat_lists_match_the_model` ("full of stat 19 layer 0 … left -1, right 0") was a **model** error (`toggle_drift` did not drift the parent P that holds the toggled list in its active chain, `stat-lists.md` §8.6 moves values into U's list only), fixed in the model with `regress_full_after_free_and_toggle`, no code change; 200,000 cases pass. **Mutation testing** (`cargo-mutants` 27.1.0): `d2-server` 862 mutants, 390 caught / 366 missed / 3 timeout / 103 unviable → 415 caught / 341 missed with 25 kills from `mutants_core.rs` and `mutants_adapters.rs` (`dispatch`, `check_size`, `select_skill`, `ClientBuffers`, `Inbox`, `Host::flush` / `disconnect`, `SystemClock`, `unit_target`, skill routing); on the merged base 915 mutants, 457 caught, 343 missed, 3 timeout, 112 unviable (these counts predate the host merge: `skills/seams.rs` is gone and `trade.rs` is `wired.rs`, so re-run); `d2-net` has no mutants (two doc lines); monsters / missiles 2,461 mutants, 322 missed → 50 missed after 121 new tests (115 with `Covers:` claims; missiles 54 → 6, monsters 268 → 44), the 50 are 22 equivalent, 25 unobservable (17 default `InitHost` bodies) and 3 spec gaps (MM1–MM3); no code disagreed with a spec. **Game-file tests, not run** (`game-tests-inventory-path`): `d2-sim/tests/game_inventory_path.rs`, 6 `#[ignore]` tests + 2 CI tests; no `Covers:` claim until a local pass (§5 C51) | `cargo test -p d2-client` at `prop-client`: unit 220 + 5 ignored, every integration file passes; `prop-inventory` and `prop-walk` add no claims (they overlap the unit claims); mutation: the 36 spec-decided survivors of `d2-server` and their fixtures are §2 step 7t; the benches compile in `cargo clippy --workspace --all-targets` |
| 3ae Gap tests for the newly implemented specs (`gaps-new-specs`) | unit-tier tests, no game files, no code fix (every new test passed against the existing code): `path/gap_tests.rs` (`path-placement.md` §7.1 wrappers: max distance 50 reaches ring 49 and not 50, the fallback argument of `0x0064E7B0`, `0x0064E7E0` steps by k and never falls back, `0x0064E810` needs the walk-back to the origin; §9 text: the floor drop is one cell against 0x3E01, each of its six bits blocks, 0x8 does not; §12.1 text: types 10 / 11, the tile relative to R, slot from bits 20–25 only), `items/inventory/gap_tests.rs` (§1.4 r3, §2.4 r5, §4.3 hand table for L = 4, 5, 11, 12, §4.6 r3, edge case 9: all 512 occupancies of a 3 × 3 grid × item sizes up to 3 × 3), `items/moves/tests/gaps.rs` (§7.21: 0x4C is left alone at any length), `d2-server` `handlers/world/tests/gaps.rs` (`quests.md` §6.2: 0x40 frames as one byte, runs `0x00546040`: 0x28 first, 0x52 last) | M08 by hand, each reverted and each failing exactly its new test: `FREE_MAX_DISTANCE` 51, fallback on in `free_point_step`, `ITEM_FLOOR` without 0x2000, the warp letter swapped, `SWAP_OTHER_TO_PAGE` → 0, the §4.6 step 3 skip test dropped, the §2.3 grid-edge weights 0, the §2.4 link check moved after step 6. Any-tier coverage on the merged base: `inventory.md` 145 of 148, `path-placement.md` 80 of 84, `quests.md` 74 of 81, `composition.md` 9 of 16, `capture.md` 6 of 9, total 2,829 of 3,106 (91.1%). Not claimed, with reasons (note §4): `inventory.md` §1.1 (no byte layout), §2.4 optional-inventory argument, §6.1 r4 (not implemented); `path-placement.md` §10 / §12.2 caller lists, §10 r7 (position history kept out of `d2-sim`), edge r1 / r8; `quests.md` §2.3 r4, §9.1, §9.2, §9.4 (partial), §10.2, §10.7 (D2MOO-derived), §11; `composition.md` §1, §3 r1, r3, §7; `capture.md` §1–§3 (the recorder's side); `ai.md` 11 and `missiles.md` 13 unchanged from `gaps-combat-ai`. No new spec question |
| 3af One item store and one inventory per game; the drop's free spot (`unify-items`, `drop-freespot`) | **Wired, unverified** (no rule added: the same `d2-sim` modules run on one store and one model). **Eighth fold.** `ActionHooks::items` (`wiring::economy::ItemStore`) is the game's one item store, lent to an economy for a call and written back; `DeathDrops::items` and `WiredWorld`'s own store are gone, so the kill's gold / item can be picked up (0x16), sold (0x33) and cubed (0x2A) from the item data the drop wrote (closes `host-merge` W-5 = HM5, `wire-inventory-server` F1 = IS4). `WiredWorld::inventory` (`InvParts` + `InvState` + the move rest) is the one inventory: the cube's `Staged::inventories` and `Inventory` are deleted, the cube (`ServerCube`) and the vendor's player-inventory calls (`InvVendors`, `handlers::items::vendor_inv`) answer from the model (F2 = IS5); an item placed by 0x18 is sellable and usable in the cube, and an item bought or transmuted lands where the moves see it. New `d2_sim::wiring::inventory::host` (reads `InvState::{of, items_of, cursor_of, holds, body_items, fillers}`; rules `InvDesk::{place (§2.4), remove (§1.4), free (0x00557FD0), reset_targeting (§5.3), check_stored / check_ground_or_owned (§5.1), send_item_page (§6.4 direct 0x9D)}`). **Free spot** (WP2 of step 7p): `DropPlacer<H>::place(econ, x, y)` receives the economy, so with `ActionHooks::paths` on **and** `PathState::field` loaded the death drop runs `wiring::path::place::floor_drop` (`treasure.md` §7 step 2, `path-placement.md` §9, size 1, fallback true), the dropper's position is `h.path_position`, and each created item gets its path part (`SUNIT_Add`, `path-placement.md` §2.5) from `DropPlacer::placed`, so the next drop avoids it; with the provider off, or on without the field, the `FreeSpot` seam answers as before (the hosts do not load `ExpField.D2` yet). Readings: `owns_item` as `inventory.md` §5.1's owned-item test (R1 = UI1), `remove_stored` queues no message (R2 = UI2), the order of a vendor call's inventory messages (R3 = UI3), the free spot's room (DF1) | d2-sim inventory wiring 31 (5 new in `tests/host.rs`), d2-server 163 (cube tests ported to the model), `e2e_single_player` 3 (36 frames, was 32: kill gold picked up, cap sold at Akara, buy stops at the item copy `0x0055A2A0`, ring picked, put in the cube, transmuted; same seed same run) and `e2e_vendor` 4 pass; `wiring/action/tests/death.rs` 4 new (vectors D1–D3 translated by (+3, 0): floor drop (15, 13), wall column → (14, 13), item bit at the start → (14, 13), a dropped item blocks the next, no field → the seam's answer); M08: with `placed` disabled and the old `h.x.position` read, all 4 fail. Gates: unify-items **PASS** (every step); drop-freespot **PASS** (d2-sim + conformance 2,038, rest 599, d2-client 363) |
| 3ag Walk / run on the server and the update pass (`wire-path-server`, `path-update-pass`) | **Wired, unverified** (no rule added; no per-tick recording, `pathing.md` OQ1). **Eighth fold: C→S 0x01–0x04 run** (`d2_server::adapters::handlers::walk`: `WALK_IDS`, `handle` reads x, y or type + GUID and returns result 0 in every case (§1.1), `run` calls `wiring::path::walk::walk_message` through `ActionSim::with`, `enable_paths`); `WorldHost::walk` (default `None`, implemented by `ActionWorld` and `WiredWorld`), tried in `SimGame::handle` after skills and before the stub; without the provider the ids stay stubs (recorded in `unhandled`), so every earlier host and test is unchanged. **Update pass** (`pathing.md` §10 rules 3 then 2): `tick::client_update` → `TickHooks::send_unit_update` (`0x0053A5D0`) → `ActionSim` (forwarded by `WorldSim` and `QuestTick`) → `wiring::path::walk::update_messages`: S→C 0x15 from flags 2 (0x10000 → flag 1; 0x800 for the other clients → flag 0), then for a player with unit flag 0x1 in modes 2 / 3 / 6 0x0F or 0x10 to every client but its own; sent through `Pending::send`, reaching the clients by `SimGame::tick`'s `take_sent`. 0x0D is the placement's and the waypoint arrival's (`waypoints.md` §7 r7): with the provider on the same-act warp places the player in the spawn room and the client gets 0x07 then 0x0D (e2e step 6 holds on `e2e_walk.rs`; `e2e_single_player.rs` keeps the provider off because its step 4 flies a missile, blocker WP1). 0x96 is the client vitals sync's (`pathing.md` §10 r5, `combat/vitals.md` §5; sent since, q-proto-audit P16); rule 4 (monsters 0x67 / 0x68, `0x00598220`) has no spec owner: `update_messages` sends nothing for non-players. The sim builds the bytes (`path::walk::messages`); every test states them with `d2-proto`'s typed builders (`PlayerMove`, `PlayerToTarget`, `PlayerStop`, `ReassignPlayer`, `MapReveal`). Readings to settle: no 0x15 after a waypoint warp to another level (WS1), flags 2 0x10000 / 0x800 and unit flag 0x1 never cleared (WS2), the dispatcher's range checks read the staged `UnitFacts` (WS3), run velocity = walk velocity (WS4), a same-level two-room waypoint travel sends no 0x0D (WS5), the second client's 0x0F per tick (PU1) | d2-server walk tests 9 → 14 (`walk/tests.rs`: `walk_to_point_moves_the_player_and_tells_the_other_client` = vector M1 translated, x = 0x1AE000 + k · 0x6000, tick 14 at 0x1F8000; run to a point (stamina − 14 · 40, code 0x17); walk / run to a unit (0x10, codes 0 / 0x18); room change A → B; refused requests still 0; stubs without the provider; the waypoint to the town (0x07 then 0x0D); a warp within the level (0x15 next pass); `walk_ids_match_client_messages_tsv` with three perturbations; the five update-pass branches: 0x800 before the mode update, 0x10000 to both clients, town walk, only players, nothing without the provider); `d2-client/tests/e2e_walk.rs` 2 (walk A → B 62 frames, run back 56, walk to the waypoint object, 0x49 to the town: 0x07 + 0x0D exact bytes; same run twice). Gates PASS: wire-path-server (tests in its commit), path-update-pass d2-sim + conformance 1,876, rest 604, d2-client 362 |
| 3ah Tests and tooling, eighth fold (`prop-fixes`, `prop-walk-gaps`, `mutants-core`, `mutants-inventory`, `mutants-world`, `mutants-items-treasure`, `mutants-combat-skills`, `ci-nightly-props`, `game-tests-wired-host`, `spec-unit-numbering`) | **Fixes the property tests deferred** (step 7n, each test written first and seen failing on the base): `StatLists::expire_lists` returns `Result` and stops with `StatListError::EndlessExpiry(list)` at the first expired extended list (the state 1.14d spins in; event 12 passes it up as `UnitError::Stats`; the replay reports it as a mismatch); a stale `ListId` acts as a null list in every public `StatLists` method (the lists stay byte-for-byte unchanged); `TimerId { slot, generation }` (the generation is d2rs's own, so `cancel` of a reused id no longer reaches the new timer); `dispatch::in_range` uses `abs_diff` (exact abs(dx) ≤ 50 and abs(dy) ≤ 50 for any positions); the `remove_active_room` item stays open (Q3 = SL3). **Property tests for the walk gaps**: `prop_walk_rooms.rs` (multi-room walk: room recache §9.6 r9, §9.8 messages, flag 0x1; the messages as set differences; `set_position_recaches_by_spec`), `prop_walk_motion.rs` (knockback keeps the distance budget, `set_path_type_follows_the_table`, `chase_a_moving_target`), `prop_path_footprint_ops.rs` (`set_foot_mask_restamps`, `corpse_footprint_is_a_plus_of_0x8000`, `teleport_follows_the_spec`) on the shared fake `walk_rooms_fake`; 17 M08 mutants caught (one survived the first run until the generator picked a teleport onto its own cell 15 % of the time); `PROPTEST_CASES=30000` release hunt passes; no code bug found; `// Covers:` on 16 rules (`pathing.md` §1.5 r2, §2, §3 r10, §9.2 r1, r3, §9.5 r3, §9.6 r4, r8, r9, §9.7, §9.8, §9.10; `path-placement.md` §5.1, §5.3 r1, r3, §6 r4). **Mutation testing** (`cargo-mutants` 27.1.0, base `4b5b0bf` / `edd9925`; no code was found wrong against its spec and none was changed; every test names its spec rule): `d2-sim` tick / units / stats 1,342 mutants, 149 missed → 47 (rng + game 48, 0 missed; the conformance replay killed **0** of the 149, its traces do not reach stats, regeneration or the stat-list chain); `d2-proto` 844 mutants, 32 missed → 9 (14 of 993 after merging `Packed` fields and `s2c`); `items::inventory` + `items::moves` 1,304 mutants, 135 missed → 64 (33 of the 64 are `MovePending` default bodies); `d2-sim::world` 2,385 mutants, 389 missed → 34 (115 tests); `items` + `treasure` 1,819 mutants, 222 missed → 48 (62 tests; 1,686 caught); combat + skills 1,940 mutants, 358 missed → 47 (105 tests; missed rate 18.8 % → 2.5 % of viable; 5 timeouts). Every survivor is classified equivalent, unreachable or not decided by the spec (listed in the notes); claims only where the assertions check the whole rule (`stats.md` §2 r2, §5 r2, r3, §8 r2, r3, §9 r3; `intents-events.md` §2.4 r10; `callbacks.md` §2 text; the treasure and item rules named in `mutants-items-treasure`; `coverage --check` 3,315 claims at that gate). **Nightly deep property job** (`.github/workflows/nightly-props.yml`, 03:17 UTC + `workflow_dispatch`, input `cases`, three matrix jobs `sim`, `wire`, `worldsim`, 90 min timeout; `tools/props-deep.sh [sim, wire, worldsim or all] [cases]` holds the filters so local == CI; on failure it prints each `minimal failing input` and uploads `target/props-deep/` as `proptest-<group>`): `sim` at 20,000 cases **measured 670 s** (`floor_drop_never_blocked` 630 s, `nearest_with_field_matches_reference` 597 s), `wire` ≈ 10–20 min and `worldsim` ≈ 12 min **estimates**, not run; the first deep run found the stat-list failure (fixed on the model side, `fix-statlist-prop`, plus `prop-fixes` item 1) and `anim_schedule_matches_the_closed_form` aborting with "Too many global rejects" (fixed by the coordinator: `schedule_inputs()` doubles a positive step until the §4.2 loop is under `MAX_LOOP`); after both, `stats::prop_tests` at 20,000, `prop_timer` at 3,000 and `prop_units` at 20,000 pass. **`game-tests-wired-host`**: 7 `#[ignore]` tests in `crates/d2-server/tests/game_wired_host.rs`, one per `charstats` class (`wired_host_amazon` … `_assassin`; §5 C59), each runs twice and compares FNV-1a 64 digests; no claim (`Intended claim: none`), **none has run**. **`spec-unit-numbering`**: the `Edge cases & original bugs` lists of 15 specs became numbered lists (`N. `, text unchanged) and 8 claims were narrowed after reading each test (cube r1–r7, txt-format r1, r3, r5, r8, preset, animdata r1–r4, camera r1 / r2, sprite-placement r1, loading r1, r2, r4); `tools/coverage.py` + `docs/COVERAGE.md` §1 gained **table-row units** (a table directly under a `<!-- rows -->` line gives `§<s> row<N>`, `§<s> t<K> row<N>` for the K-th marked table; opt-in per table; selftest extended): `sim/rng.md` §5.2–§7 are 39 units, only §5.4 row1–2 claimed (`setters_and_derive`); the rest is trace / RE material | prop-fixes: `gate.sh` PASS (13 steps; d2-sim + conformance 1,880 passed / 107 skipped, rest 601 / 75, d2-client 362 / 16); prop-walk-gaps `--no-client` PASS (d2-sim + conformance 1,885, rest 599; 4,143 claims, 0 errors); the five mutation notes' gates PASS (mutants-combat-skills: d2-sim 1,337 passed, 5 ignored; mutants-items-treasure 1,294 passed, 5 ignored; mutants-core 1,273 passed, 5 ignored); `coverage` before → after of `spec-unit-numbering`: rules 3,106 → 3,190, any 2,839 (91.4 %) → 2,896 (90.8 %), verified 219 → 227 (7.1 %): the dips are real (camera 100 → 85.7 %, sprite-placement 77.8 → 58.3 %, cube 93.9 → 82.5 %) |
| 3ai Test installs, MPQ Huffman and the S→C builders, eighth fold (`fixedset-game`, `mpq-huffman`, `s2c-use`) | **Game from a `FixedSet`** (`test-fixtures`, step 7e, repo only): DS1 writer (`test_fixtures::ds1::write`, v1–18, round trips v7–18 × tag types 0 / 1 / 2 and the spec vectors) and DT1 writer (`dt1::write`; iso and RLE blocks; the spec vectors); synthetic DRLG files (`drlg`: a 9 × 9 town with one waypoint object, keep, cave, field, sub, six DT1; archive names `DATA\GLOBAL\TILES\` + table string); a lvlprest row Def 3 for level 2 ("Synth Field", DrlgType 2: the town's `Vis0` links it, so the first tick's room update builds it); `GameData { bins, anim, fixed, level, files }` with `load` / `from_install` and the builders `action_tables`, `world_tables`, `vitals`, `stat_data`, `unit_data`, `waypoints`, `level_types`, `drlg_world`, `world_sim` (the ones `game_wired_host.rs` assembles by hand on the live set; they work for the user's install too), `Seams`, `type Sim = SimGame<WorldSim<Seams>, ActionWorld>`; `test-fixtures` now depends on `d2-sim` and `d2-server` (normal dependencies, still test-only, no Bevy). `crates/test-fixtures/tests/synthetic_game.rs` (CI, no `#[ignore]`): archives → `bin::load` → `fixup::apply` → table views and `world_data` providers → `WorldSim` → a one-room town generated and streamed (81 floor records, the waypoint preset unit, a class-3 player) → `SimGame::join` → `Host::frame` × 51, no fault, no wiring error, a second run equal. **Seam: `ActCreation::TownOnly`** (`Drlg::create` with `NoLevelTypes` and then the town allocated and generated): `ActCreation::Full` (the act placer of `levels.md` §3 step 7 allocates 1.14d ids 1–7, 17, 26, 39 and expects their types, which the 8-row made-up levels table lacks: `UnknownLevel(17)`) works on the user's real data only, and the TownOnly DRLG seed skips the placer's draws (a fixture state, not a 1.14d one) (FG1); no server message reaches the idle client in 50 frames (no join sequence is wired; FG2). **MPQ Huffman** (`mpq-huffman`, BB1 of step 7r; `specs/formats/mpq.md` §9, §11 unchanged): the encoder `huffman::compress` is now `cfg(any(test, feature = "test-support"))` (weight tables 0–8, O(1) symbol lookup, word accumulator), `writer::Method::Huffman { table, pkware }` (mask 0x01, or 0x09 with PKWARE over the Huffman stream, a sector longer than the sector stored raw) and `writer::huffman`, `WriteError::BadHuffmanTable`; **decoder speed-up with unchanged output and errors**: a 256-slot leader cache `weight % 256 → node` used only as a hint (`Tree::leader` accepts an entry only if that node has weight `w` and its predecessor does not) and a static fast path for tables 1–8 (the tree stays borrowed from the template until the first escape; a 1024-entry lookup resolves up to 10 bits). Cause of the BB1 figure: not a rebuild per sector (cloning costs 30 ns for table 0, 1.2 µs for the 515-node tables) but one Increment per tree level per symbol with about four SipHash operations each. Criterion `mpq_huffman` (256 KiB, 512-byte sectors, cloud 4 vCPU Xeon 2.10 GHz, medians, order of magnitude): text table 0 74.7 ms (3.35 MiB/s) → 22.2 ms (11.3 MiB/s), table 0 + PKWARE 72.4 → 25.2 ms, table 1 6.10 → 1.78 ms (140 MiB/s), table 2 3.82 → 1.90, table 3 6.01 → 2.03; deltas table 0 51.3 → 13.8 ms, table 1 7.43 → 2.27, tables 4 and 6 about unchanged (escapes early), table 5 1.26×. Tests (`mpq/huffman_tests.rs`): `huffman_round_trip` (every table × three data shapes against the model), `decoder_matches_the_model` (arbitrary and truncated streams), `archive_huffman_round_trip` (tables, ± PKWARE, sector shifts 0–3, encryption + FIX_KEY, SECTOR_CRC), `huffman_then_adpcm` (masks 0x41 / 0x81), `leader_cache_is_only_a_hint`; M08: ignoring the fast path's `consume` error is caught, a broken leader test never finishes, wrong cache nodes survive by design. **S→C builders** (`s2c-use`, step 7k): **blocked, then decided: `d2-sim` keeps no dependency on `d2-proto`** (option A; `tools/depcheck` `FORBIDDEN` has had `("d2-sim", "d2-proto")` since Phase 0; the task text and `s2c-builders.md` §5 item 3 said it was allowed, §8 lesson). The sim keeps its byte builders; new `world::cube::trade_action` (0x77) used by the three inline `[0x77, …]` sends; `d2-sim/tests/s2c_bytes.rs` (7 tests; pins the bytes of `npc::{transaction, service_result, resurrect_message}`, `waypoints::{menu_message, arrival_message}`, `path::walk::messages::player_stop`, `cube::trade_action`); `conformance/tests/s2c_builders.rs` (7 tests, one per builder: sim bytes == `d2_proto::s2c::{NpcTransaction, OpenUi, Unknown9B, WaypointMenu, PlayerStop, TradeAction}::encode()` and `parse(bytes)` == the same `Message` over every edge combination plus a fixed xorshift sweep; 0x77 over all 256 actions), which settles J13 "one maker per id" as a check; M08: flipping bit 0 of `trade_action`'s byte fails exactly the two 0x77 tests. `bridge-dispatch.tsv` stays all `TBD` (no client spec owns an S→C id; the typed variant for each is in `s2c-use` §3) | `synthetic_game` 2 tests (`providers_hold_the_named_files`, `town_join_and_frames`) and the writers' round trips pass; mpq-huffman: `--no-client` gate PASS (1,876 + 607 tests); s2c-use: every non-client step PASS, the three `d2-client` steps fail in that container only (`wayland-sys` finds no `wayland-client`; `tools/cloud-setup.sh` installs it); `cargo clippy --workspace --exclude d2-client --all-targets -- -D warnings` clean |
| 3aj Bodies: monster AI Acts II–V, skill bodies batch 2 / 3, every missile body (`impl-ai-acts2-5`, `impl-skill-slots-2`, `impl-missile-bodies-2`) | **Implemented, unverified** (no recording of any Act II–V AI, skill body or missile body). **AI:** 56 more functions in `monsters::ai::{bodies2, bodies3, bodies4, bodies5, common}` (93 of 148 indices have bodies; the 55 `unread` stay stubs that log `Unhandled::Function`), seam `AiActs` (part of `AiHost`) answered by `wiring::action::View` for unit flags, max life, state groups, states count, move mask and path stop, every other call a `Pending::ai_*` default; `AiTables` gains `skills` and `missiles`, `skill_modes` is `[u8; 8]`. **Skills:** every `functions.tsv` row of status `spec'd-here` has a body (45 start and 85 do slots: `skills::use_::bodies::{helpers2, helpers3, starts2, dos2, b3_lvl01..b3_lvl30}`), seam `BodyWorld` (+ command seams `BodyEffect`, `PathOp`), `UseView` answers on real data where the wired host has it, the rest goes to `Pending`; player pet lists `player::pets` (`sim/pets.md` §1–§9 over `PetWorld`; no host yet). **Missiles:** every non-null server-do (36) and server-hit (53) body (`missiles::{bodies_ext, bodies_ext2}`), new `MissileBodies` seams defaulted (the wired `View` answers `skill_field` and `Calc4` only), `Unhandled::Fatal { addr, missile }` where 1.14d asserts. Also: the A1Q6 Catacombs entry keeps states 4 and 5; `HratliMagicLvl` pinned by a compile test | `cargo test -p d2-sim --lib monsters::ai` (86 tests in `tests/act2.rs`..`act5.rs`; `specd_here_matches_tsv` + perturbation), `… skills::use_` (`bodies/tests2.rs` 23, `tests3.rs` 29; `function_tables_match_tsv`, `bodies_match_tsv_notes` + perturbations), `… player::pets` (27), `… missiles` (`tests/ext.rs` 64; `bodies_match_catalogue_status`, `bodies_check_catches_perturbations`); unit coverage `ai-bodies-2..5` 108/109, 70/72, 82/84, 142/143; checks C87, §5 S9-A4 |
| 3ak Quests Acts I–V (`impl-quests-act1-rest`, `impl-quests-act2`, `impl-quests-act3`, `impl-quests-act4-5`) | **Implemented, unverified** (no recording of Acts II–V; no trace of a Cain rescue, a Cairn stone 0x50, a Countess kill or a Catacombs entry): `world::quests::{act1 (q4, q5, q6 remainders), act2, act3, act4, act5, late}`: every callback, active / status function, timer, quest object init / operate (as `pub fn`s) and other-system hook the specs describe; `QuestControl::new` inits Acts II–V; `TimerFn::{Act2, Act3, Act4, Act5}`; Kashya's deferred reward order (`wiring::economy::QuestDeferred`) and the bucket-order player walk (`quest_players`). About a hundred new `QuestWorld` seams with default bodies that report `unhandled(0xFF / 0xFE, addr)`; on the wired host `HostQuests` (row 3al) answers object mode, timers, allocation, room level, the interaction owner and identify, the rest stay on the rest with a reason (`wire-world-staging.md` §3 item 6). Quest-object callers from objects, AI, monster creation, cube and item use are wired only for the Act I / II objects the dispatcher routes | `cargo test -p d2-sim --lib world::quests` (Act II 81 tests, Act III 145, `world::quests` 246 on `impl-quests-act4-5`; Act I rest `act1_rest_q4_tests.rs`, `act1_rest_misc_tests.rs`); every Test-vector row a test with `// Covers:`; §5 S9-A1 (12)–(13), S9-A2, S9-A3, C91 |
| 3al Objects, hirelings, game creation and quest-object routing (`impl-objects`, `impl-hirelings`, `wire-world-staging`) | **Implemented and wired, unverified**: `world::objects` (+ `chests`, `shrines`, `misc`: control, init / operate dispatch, presets 574–580, chests, breakables, traps, the 24 shrine codes, doors, wells, torch, portal rules 1–2, events 1, 2, 4, 5, 6, 11, S→C 0x0E / 0x4D) kept in `ActionHooks::objects` and lent per call; C→S 0x13 with unit type 2 → `WorldHost::objects` on `ActionWorld` and `WiredWorld`; `ChestWorld` / `ShrineWorld` / `MiscWorld` are empty on the wired host (no drops, no shrine effects, no keys). `world::hirelings` (rows, offer, init / replace, level stats, experience, death, revive, follow, classic act change, restore steps, item swap) on the interaction desk (`HireView`, `HirelingRest`); no production loader sets `HirelingTables`. `WorldSim::create_game` runs regions → object control → NPC control → quest control in `rng.md` §5.2 order (the app's game does not call it); the server frame's clock is the object host tick; quest inits / operates / event 7 are queued and run on the host's quest control (`wiring::economy::quest_objects`, `HostQuests`); the hireling teleport follow has a caller (death, 0x61 swap, save restore and the act change have none) | `cargo test -p d2-sim --lib world::objects` (84), `wiring::action::tests::objects` (7), `world::hirelings` (53), `wiring::worldgen::tests::creation`, `wiring::economy::quest_objects::tests`; `cargo test -p d2-server` `world/tests/objects.rs` (3 + the host tick), `world/tests/quest_objects.rs` (5); checks C78–C80, C90, §5 S9-A5 |
| 3am DRLG Acts III–V and live room population (`impl-drlg-act3-5`, `impl-room-population`) | **Implemented, unverified**: `drlg::outdoor::{jungle, act3, kurast, act5}` (jungle placer on the DRLG seed, Kurast chain anchored on 78, jungle stamping, Kurast / Travincal, the Act V §11 build; `OutdoorError::Fatal(id)`), the maze link rules of f71ee9b and the preset door flag; every derived vector of `outdoor-act3-act5.md` is reproduced. `drlg::logic` (room +0x64 logical rooms, `levels.md` §11.1–§11.4) and the §11.5 queries on `Level` answer `PopWorld` from the act DRLG, so rooms populate when activated (closes F1 and GH1); `WorldPending` keeps only `nearest_free_point`; `seed-finder` answers champion / unique queries on the live host (its F2, activation order, still holds). Cold Plains 97 vs 98 rooms stays an outdoor finding; `fix-drlg-answers` implemented PC 1's answers (act3-5 Q1–Q13, room-population §3) and added the live grid / substitution diagnostic (C92) | `cargo test -p d2-sim --lib drlg` (367 on that branch: `kurast_tests` 13, `act5_tests` 13, `maze/links_tests` 6, `tests::logic` 13), `wiring::worldgen::tests::population::drlg_population_reads_are_the_act_drlgs`; coverage `outdoor-act3-act5.md` 40/45, `outdoor.md` 78/82; checks C65, C89, §5 S9-A1 (4), S9-A6 |
| 3an Character save `.d2s` (`impl-d2s`) | **Implemented, unverified** (no 1.14d save compared, `d2s.md` OQ3): `d2_formats::d2s` reads and writes byte-identically every file it accepts (the whole header, every section, the loader's codes and 27-entry result table; item entries opaque, sized by `d2_proto::item_bits::save_entry_len`); the item save format's writer `items::bitstream::write_save` and reader `item_bits::decode_save_record`; `tools/d2s-tool` (`new`, `new-stub`, `dump`, `check`, `set`) for local game testing. The load effects (`d2s.md` §9: `d2-server` character storage) have no code | `cargo test -p d2-formats d2s` (27 incl. a perturbation test), `-p d2-sim items::bitstream::save_tests` (6), `-p d2-proto item_bits::save_tests` (7), `-p d2s-tool` (9 + 1 ignored); C66, `docs/LOCAL-RUN.md` 2.18 and 6.7 |
| 3ao Session join from the in-process server (`impl-server-join`) | **Implemented, unverified** (C84–C86): `d2_server::adapters::session::enter_game` queues S→C 0x59 (part A), 0x0B, 0x03 (act, init seed = game +0x7C, town level, game +0x80 = `ObjectState::obj_seed`), then game entry (`wiring::path::place::game_entry`: 0x07 of the spawn room, 0x15); the room switch sends 0x07 for each joined room (`ActionSim::client_level_change`); `d2_proto::s2c` builds 0x0B and 0x59; the app's game joins its player through it, so `play` builds the client DRLG (row 6, ninth fold). **Tenth fold: the "Not sent" list of this row is closed** (0x01 / 0x00 / 0x02, the messages between 0x0B and 0x03, 0x53, 0x04, the switch's add messages and 0x08 are sent; the client reaches `in_game`; row 3ap). Still true: the app's 0x03 carries game +0x80 = 0 (no game-creation sequence in the app); the staged e2e games still skip the join and refuse their 0x07s (§2 7w) | `CARGO_INCREMENTAL=0 cargo test --workspace` at `ef00681`: 5,263 passed, 0 failed, 218 ignored; `test-fixtures` `synthetic_game::town_entry_sends_the_join_sequence`, `d2-client --test app_client_drlg`, `d2-proto s2c::tests::recorded_join_messages` |
| 3ap Session flow, the join's messages and the monster mode message (`impl-session-flow`, `impl-server-join-2`, `impl-monster-death`) | **Implemented, unverified** (C84, C94): **session**: C→S 0x67 → 0x6B → first tick on the host drain (`Intents::session_message`, default false; `d2_server::adapters::session_flow::SessionFlow` set with `SimGame::set_session(SessionFlow::new(arena_flags, loader))`): the 0x67 checks of `0x0052C330`, the client record, S→C 0x01 / 0x00 / 0x02 and client states, 0x6B through the caller's `CharacterLoader` (load result 0x18 for a classic game with class ≥ 5), `SessionFault` for every refusal; `enter_game` in the order of `intents-events.md` §8.2 (0x59, 0xAA, 0x76, 0x0B, [0x5F], 0x7B per hot key, [0x23 × 2], 0x95…, 0x03 + state 2, game entry, state 3), the game entry as `path-placement.md` §11 (0x07, the room switch, placement, 0x15, 0x7E), 0x53 after 0x03 from `world::environment::Environment` (per act, created as index 2), 0x04 with the first tick; `units::messages` (pure builders: 0x01, 0x00, 0x02, 0x04, 0x08–0x0B, 0x23, 0x51, 0x5F, 0x76, 0x7B, 0x7E, 0xAA), `wiring::action::switch::room_switch` (`0x00537B50`: joins with the AI wake-up `0x00573780`, leaves with 0x0A / 0x08, add messages for players, objects and tiles; `client_room_ready` `0x0061A460`), `combat::vitals::sync::join` (the join's forced 0x95 + gold + experience); the app's `single_player` joins through `create_game` / `enter_game` and the client reaches `in_game`. **Monster update** (`intents-events.md` §7.3 r2 steps 1, 2, 4): `monsters::mode_message` (mode table `0x006E1D90`, §7.4 rules 3–6, the nine builders of §7.7) and `View::monster_update` send 0x15 + the room-change messages, the mode message 0x67–0x6D and the overhead message (0x76); 0x69 code 8 is sent at the kill and code 9 at the end of the death animation (MB-8 closed); `room_cleanup` (tick step 6) clears unit flag 0x1 / flag-ex 0x10000 / 0x800 (WS2 / PU1 closed; WS1 unchanged); DT event functions `death_event0` / `death_event1`. **Not sent / not done**: monster add messages 0xAC (server fields past `init.md` §24) and 0x98 / 0x21, 0x73 (layout), 0x9C in the switch, object class 59's 0x82, the loader's 0x94 / 0x22 / 0x21 / 0x5E / 0x28 / 0x29, the stat messages (`0x0053BE40` choice of 0x1D–0x1F), item messages, the update-list reset, the join sequence after 0x04 (0x5B, 0x65, 0x8D, 0x5A) and `0x0055DF00`, §7.3 r2 steps 3 and 5–10, §7.5 steps 1, 2, 4–6, the second 0x53 of frame 2 (`-022633` seq 228), C→S 0x67 name checks and 0x69 / 0x6A / 0x6C / 0x6E / 0x70; a host whose `Pending::monster_death_start` has no body never reaches DT, so it sends no 0x69; the environment record never advances (`0x0061C040`'s arguments); the app still calls `create_game` / `enter_game` directly and uses the recorded arena flags 0x00100004 | `CARGO_INCREMENTAL=0 cargo test --workspace --exclude d2-client` at the notes' heads: 4,400 / 4,607 / 5,514 passed (the failures each note names are the base's); `test-fixtures` `synthetic_game::{session_messages_run_creation_then_the_join, session_refusals_stop_the_sequence, town_entry_sends_the_join_sequence}` and `monster_death` (the client gets `69 01000000 08 0000 0000 00 06` in frame 11 and `… 09 1800 1400 00 00` in frame 34, a second run identical), `d2-server tests::host::system_messages_reach_the_game_first`, `d2-sim` `world::environment::tests`, `wiring::action::unit_update::tests` (6 + 2), `monsters::mode_message::tests` (7), `room_switch_sends_add_and_leave_messages_and_the_join_completes` |
| 3aq Monster walk / run and missile flight on the path provider (`impl-path-motion`) | **Wired, unverified** (no per-tick recording, `pathing.md` OQ1): **WP1 is stale**: a missile built with the provider on gets its type-4 path and flies by `pathing.md` §11 (`path::walk::missile`; the action fixture's arrow moves 0xC00 a frame); `wiring::path::monsters`: `ActionHooks::monster_path_setup` (the path part of `0x005A7C20`: the AI request's target, re-path budget 20, request byte 101 → path type 13 with a type-15 retry on 0 points, byte 100 → type 0), walk / run starts `0x005A7520` / `0x005A7550` set their mode, `UnitHooks::anim_velocity` (`pathing.md` §8.1), walk event 0 `0x005A8490` (state 13 / 22 steps, the step `0x00554CA0`), the mode end `0x005A8030` (also event 1 of modes 3–9 and 14, not gated on the provider), `AiUnits::path_target`, `AiModes::path_blocked`; `wiring::path::missiles`: the `MissileBodies` path seams (`path_target_point`, `target_position`, `set_path_type`, `set_path_distance`, `path_teleport`) and a real charged-bolt path for the `zigzag` init. An AI walk moves a monster sub-tile by sub-tile and thinks again when the path ends; a point walk reproduces vector M1 for `Velocity` 6. **Left**: event 0 of run `0x005A84F0`, knockback `0x005A8630` and `0x005A8670` (MV2: a running monster sets mode 15 and stands still; seam `Pending::monster_run_event0`), the attack / skill / BL / KB / SQ starts leave the mode unchanged (MV1: AI attacks never enter their mode), path types 0, 3, 5, 6, 8, 9, 11, 12, 15, 16 (MV8: the type-15 retry finds nothing), missile init callbacks (jitter, lightning fan / ring, DiabWall) need the missile frames inside `create_missile`, the `0x0064E260` line test (MV11); `crates/d2-client/tests/e2e_single_player.rs` still runs with the provider off and its comments (lines 62, 1574) name WP1 | `cargo nextest run -p d2-sim motion_tests` (11 tests, `wiring/path/motion_tests.rs`: missile built and flown, monster point walk = M1, provider off = no movement, path type 0 / 13 / 15, flag 0x800, attack end, AI walk to a unit stops at (27, 30) after 19 ticks, zigzag 26 points / 25 draws, missile teleport); gates at `1a3acca`: 4,619 run, 4,614 passed (the 5 known reds) |
| 3ar PC 1's last spec batch in code (`impl-pc1-final`, `impl-pc1-wiring`, `impl-umods-cs-handlers`, `impl-monster-skill-slots`, `impl-render-missile-answers`) | **Implemented, unverified**: **AI**: every `spec'd-here` row (all 148) has a body: `monsters/ai/{bodies6,bodies7}.rs` (55 thinks, inits, Uber alternates), seam `AiSummons`, `common::spawn_info` (§13.1); special state 6 think `0x005E7C10` stays a logged stub (no spec). **Skills**: batch 4 (`skills/use_/bodies/{b4_helpers,b4_mon,b4_more}.rs`: the 83 slots of `bodies-3.md` / `bodies-4.md`, new `BodyWorld` methods and `BodyEffect` variants, `PathMissile`, `Pending::body_*` defaults) so every `functions.tsv` `spec'd-here` row is `SpecdHere` (213 slots: 64 start, 149 do; srvdo 53 / 138 / 142 stay `unreferenced`); `dos::curse_unit` goes on with v1 = 0 for `aurastat1` = −1. **Monster init and umods**: boss mods (`create::boss_mods_for`, summoner flags, ancient barbarian equipment seams), `elemental()` clamp, §20.1 per-hcIdx cases, §26 `make_unique` / `warp_eligible`, §27 `reinit`; the 32 umod callbacks (`monsters::init::callbacks`, `find`; dispatcher sites inside `units::modes::monster_set_mode`; mode 4 not wired, `damage.md` §7.1 OQ3), §15.1 purge and §15.2 quest-death bodies; the 15 `unit-events.tsv` rows in `IMPLEMENTED_SITES`. **Combat**: `combat::events` (functions 1–14, 17–31, item cast, Reanimate, iteration `0x005C0C30`) and the unit-event registry (`ActionHooks::unit_events`, `enable_unit_events` runs it on `UseView` over `ActionHooks::handlers`; callers: combat events, missile event 0, DT end event 13, level-up event 12), `combat::vitals::experience` (`ExpRatio` gain, the party quotient float emulated exactly in integers, `lastexp`, gold / experience death penalties, corpse experience), the kill in `damage.md` §7.2 order, hit §6.4 weapon block, damage §5.1 mode-0 attacker; **player death** (`wiring::action::death`: DT start → `death_penalties`, DD start → `corpse_creation`, `corpse_pickup`; seams `stash_cap`, `death_drop_gold`, `create_corpse`, `corpse_owner_guid`, `corpse_loot_allowed`, `corpse_take_back`). **Server C→S** §9 handlers (`adapters/handlers/player.rs`: 0x12, 0x4B, 0x51, 0x53 / 0x54 real; 0x14, 0x3F, 0x41, 0x46 / 0x47, 0x48, 0x4D, 0x3D partly; 0x5F → `handle_resync`; 0x44 / 0x60 entries real, bodies `None`; 0x15, 0x3E, 0x4C, 0x59 stubs), `units/mode_set.rs` (`0x00624690`, duplicates `modes::set_mode`: TODO fold), the 0x77 sender is `world::cube::trade_action`. **Missile / render answers** (`impl-render-missile-answers`): elemental roll, bypass flags 0x100 / 0x200 / 0x400, `unit_find` + `FindFilter`, the 0x53 eclipse setter, wall direction 0 reuse, GDI 45° lines, weather W1–W7 (`snow_color`, `splash_threshold`, `ColorTables::build`), Levels `Pal` column. Message renames (`AddStatPoint`, Buy / Sell / Repair, CreateGame, …) and `tools/conflict_markers.py` (+ `--selftest`) in `tools/gate.sh` and CI. **Not wired / left**: the player DT start from the damage §7.1 player branch (`Pending::reaction`), `Pending::event_layer_split` (0, 0), the rest of `0x00580EC0` / `0x0057FCA0` (character save), corpse creation itself, the no-corpse +0x508 path, stash limit `0x00623460`, the party-share x87 precision control (`stat-lists.md` OQ1), `Pending::level_up_event` on the interaction layer and the hireling `level_events`, `MovePending::corpse_pickup` (items) should call `ActionHooks::corpse_pickup`, multishot inside a missile creation (U2: `Reentrant("missiles")`), C→S 0x67 refusal rules, no unit tests for the batch 4 skill bodies, the `TODO(spec: ai-bodies-6/7…)` readings (pet move case 0, turret second skill check, wolf follow, CycleOfLife entry, QuillMother params 3–4, InvisoPet without owner, ShadowWarrior pettype, Raven level, ShadowMaster scoring), the particle move `0x004732C0` (`draw-order-2.md` OQ3), `WallPass` has no caller | `cargo test -p d2-sim`: `events_tests` (every spec vector), `experience_tests` (with an IEEE cross-check), `tests/act6.rs` / `act7.rs`, `init/tests.rs`, `wiring::interaction::tests::unit_events` (3), `wiring::action::tests::player_death` (2), `missiles::tests::ext`; tests corrected to the specs and listed in each note (`specd_here_check_catches_perturbations`, `boss_mods_bloodraven`, `dodge_avoid_evade_weapon_block`, `apply_melee_hit_steps`, `add_experience_caps_and_levels`, `superunique`, `elemental_umods`, …); `cargo test -p d2-client --no-fail-fast` 884 lib tests passed, 1 failed (`bridge::local_tests::unknown_and_unowned_ids`, another session's) at `impl-render-missile-answers`' head |
| 3as PC 2's code fixes, the world rest, quest, hireling, save and DRLG answers (`impl-pc2-fixes`, `impl-world-rest`, `impl-quests-answers`, `impl-d2s-load-hirelings`, `fix-drlg-answers`) | **Implemented, unverified**: **fixes** (`local2-2026-10-06` §3): warp letter `EXIT_LEFT` 10 → 'l' / 11 → 'r'; `InvWorld::targeting_probe` takes the inventory owner (0x3F only for a player owner; the `InvRest::targeting_probe` seam is gone); `DrlgData::wall_remap` is the embedded `specs/drlg/wall-remap.tsv` (`WallClass::{Table, Keep, Stop}`, `rooms.md` §9.6 step 3, `DrlgError::WallRemapType`; the corner half of R+0x20 is not modelled); S→C 0x22 row (`UpdateItemSkill`, "CODE-TABLE CHANGE" commit, `gen-proto`); unit allocation order (`units.md` §3.1) **not** changed (the per-kind inits' room / level source is unspecified); re-path budget and position history were already in. **World rest**: chest drop `D(Q)` (`wiring/economy/chest_drop.rs`, `ActionHooks::object_drops`; items are real item units placed by the path provider's floor drop, `NoSpot` without it), `ChestWorld` / `ShrineWorld` / `MiscWorld` on the action wiring (wells heal on the real stat lists, `cure_states`), `QuestWorld::object_treasure(object, operator, kind)`, `missile_range`, `world_data::tables::hireling_tables` (nothing calls it yet). **Quests**: roomless-object effects (QA-1 / QA-2), A1Q6 credit fatal without a client (QA-4), gibbet / tree / Cain-portal inits and Wirt's body operate (WW-6; `act1/q4.rs`), `DrlgWorld::refresh_room` (WW-10), `HostQuests` position / room seams from the path provider (N-2), quest inits run inside the allocation (`objects::create` split into `create_init` / `create_rest`, `ActionHooks::quest_host`, `WiredWorld::lend_quests`; N-3, C79), Act II QB-1–QB-20 (orifice, Jerhyn), Act III QC-1–QC-5, `QuestWorld::set_drop_code`, `quests.tsv` row 40 (`act5_intro_matches_its_rows`). **Hirelings and saves**: `ExpRatios` (`0x00613E60` + `0x0057E390`), 0x7A owner @5 / pet @9 on both sides (I-1 closed; `e2e_pet_action` passes), `recompute_max`, stats queued on the merc with damage sums 21 / 22, `HirelingRows::act_of`, `life::on_kill` + `WiredWorld::pet_deaths`, `resurrect` refuses a living hireling (GN1); `d2_formats::d2s` `item_flags_on_load` / `Header::reset_appearance` / `Npcs::set_intro_a`, `d2s-tool new` / `set` per `d2s.md` (appearance 32 × 0xFF, flag 0x2000 cleared), `d2-server::adapters::character` (`load` in the master's order, `normalise_quests`, `gold_limits`, `ActionCharacter`), `session::enter_game_from_save`. **DRLG**: Acts III–V Q1–Q13 and room-population Q1–Q3 of `fix-drlg-answers` (jungle code lookup with `OutdoorError::Crash`, ravine / barricade walks, door flag 0x20, out-of-grid record lookups, `OutdoorLevel::sub_hits`); the Cold Plains 97 vs 98 cause was **not** found in the cloud (the synthetic build gives 98): live check C92. **Not done**: object population (G1), footprints (G2), the key test `0x0055F140` (locked chests / doors never open for a non-assassin), code drop `0x00585970` / `0x00559A30` (G3), trap / spawn / range / inside-room parts of `objects.md` §8.2 (G4), shrine missiles and portal shrine (G5, G6), quest-chest gate `0x00545850` (G7), C→S 0x61 swap, restore creation place and classic act change (G8), Act III inits in the quest-object dispatcher (QC-6), C→S 0x44 → `act2::q6::item_to_object`, `set_room_portal`, `QuestTick` drains after the hooks; every `Unapplied` step of `ActionCharacter` (header client fields, quests on the host rest, waypoints, NPC fields, skills, items from records, corpse, hireling restore, mouse skills, start items), `d2s-tool --quests all` stays refused (DS-4); `ActionHooks::object_drops` keeps its own unique bits apart from `WiredWorld::uniques` and nothing sets it in production | per-note tests (`wiring::action::tests::objects` 13, `world_data::tests::hireling_tables_load_from_the_three_tables`, `mutants_wiring_inventory::targeting_reset_clears_and_queues_0x3f`, `drlg::tests::mutant_tests::{wall_remap_keep_and_stop, bit_7_merges_to_the_new_type}`, `cold_plains_grid_reproduces_the_table`, `hirelings/tests/game.rs` (3, ignored), `synthetic_game::a_full_save_loads_before_the_join_sequence`); workspace gates: 4,606 run at `impl-world-rest` (the 5 known reds), 4,594 passed / 0 failed at `impl-quests-answers` (without `d2-client`) |
| 3at Tests of the night's code (`e2e-night-flows`, `gaps-night-specs`, `fixedset-act1`, `mutants-night-code`, `mutants-path`, `mutants-walk-handlers`) | **Tests only** (no fidelity code changed; test-support edits and one test-only seam each): `test-fixtures` `e2e_night_flows` (6: game creation and the real join, chest, shrine, Act I stone and gibbet, Act II journal, hiring at Greiz, hireling follows a waypoint, kill from live room population), `d2-server` `e2e_night_world` (3), `d2-client` `e2e_pet_action` (I-1; passes now). **Act I on synthetic data**: `test_fixtures::{act1.rs, host.rs}` and `tests/act1_game.rs` (the `outdoor.md` placement vector (the 20-level `levels.md` vector after the merge) with the 35-room town, every chain level generates, join + walk into Blood Moor; two runs give one digest); `d2-server/tests/game_wired_host.rs` is built on `GameData` + `host::Session` (the old live test could never find the town waypoint before the rooms were streamed, and 0x03 is no longer a stub). **Unit-test gaps** (`gaps-night-specs`): unit tier 79.6% → 80.8% over 17 specs; the units left and their reasons are in that note (`ui/panels.md` 20, `formats/d2s.md` 27, `hirelings.md` 13). **Mutation runs** (cargo-mutants 27.1.0 with `--cargo-test-arg=--lib`, `CARGO_INCREMENTAL=1`, `CARGO_PROFILE_DEV_DEBUG=false`; `mutants.out` never committed): `world::objects` 463 of 712 tested, 35 survivors: 20 killed by 9 new tests, 6 equivalent, 9 undecided seam defaults (`chests.rs` `unit_type`, `item_quality`, `trap_monster_id`, `room_units`, `within`, `in_room`); `world::hirelings`, `player::pets`, `missiles::bodies*` (1,381) not run; `d2_sim::path`: 1,389 of 1,828 tested (288 missed), the 50 tests were restored and ported to the current API, `walk/step.rs` 250 mutants: 74 missed → 11 (all equivalent), the other files not measured; `wiring::path` 193 of 340: the 58 targeted kills confirmed, 10 more killed, 5 equivalent left, `walk.rs` (147) not run; d2-server walk handlers 0x01–0x04: 0 missed. No code contradicted a spec | `cargo test -p test-fixtures`, `cargo test -p d2-server --test e2e_night_world`, `--test mutants_walk`; gates: 5,322 passed / 0 failed (`e2e-night-flows`), 5,303 (`gaps-night-specs`), 13 steps PASS (`fixedset-act1` after the merge); `tools/coverage.py --check`: 0 errors |
| 3au Spec answers from PC 2's local session and the local run (`pc2-spec-d2s`, `pc2-spec-hirelings`, `pc2-spec-quests-act2`, `pc2-spec-quests-act3`, `pc2-spec-quests-core`, `local2-2026-10-06`) | **Specs only, no code** (the code side is row 3as): `formats/d2s.md` (DS-1–DS-5, flag 0x2000 and +0x88 → 0xFF, the new split `formats/d2s-load.md` with the new-character start `0x00569F80` and the load effects in order), `world/hirelings.md` + `npc.md` (HL1–HL9, WW-2, OQ6 / GN1; the hire recorded), `world/quests-act2-2.md` (QB-1–QB-20, Jerhyn, orifice, 0x58), `world/quests-act3-2.md` §11 (QC-1–QC-5, QC-7), `world/quests.md` / `quests-act1-rest.md` §9 (QA-1–QA-6, WW-6, WW-10, DS-4; `quests.tsv` row 40 and the Act III rows `specified`, `quest-messages.tsv` +15 rows). The local session (Windows, RX 9070 XT, Ghidra 12.1.4 export of 1.14d with no community labels) merged the spec-answer branches, ran C13 (passes exactly), C20 (passes after the TC 0 `picks` 1 → 0 fix), the fuzz crate (18 targets, ~1.59 G execs, one DT1 RLE crash fixed) and the real GPU on a second adapter (`gpu_compare` 18/18, `verify` 11/11); its live-run findings (`play --frames 1500` panics at `d2-sim/src/drlg/room.rs:144` "live DRLG room" after ~104 ticks; the batch 2 red rows) are queued as C98. Not done there: wave B tests for C10, C11, C12, C14, C18, C19, wave C groups, the wave D specs, wave F | recordings and local results are in §5 Done and `local-buddy-2026-10-06.md`; the 1.14d facts carry their addresses in the specs |
| 3 not implemented | **Tenth fold (the list is current; the ninth fold's list is superseded):** AI: the special-state 6 think `0x005E7C10` and the `unread` rows no spec covers, the `TODO(spec: ai-bodies-6/7…)` readings (row 3ar); skills: no unit test for the batch 4 bodies (`bodies-3.md`, `bodies-4.md` at 0% unit tier), srvdo 53 / 138 / 142 `unreferenced`; providers behind seams: the `Pending::*` defaults the notes name (`event_*`, `stash_cap`, `body_*`, the umod seams `umod_target`, `umod_apply_state`, `quest_death`, `spawn_near`, `ai_use_skill`, `skill_level`, `monster_run_event0`, `missile_init_callback`), the `QuestWorld` calls left on the rest (QC-6: Act III inits and providers, C→S 0x44 `item_to_object`, `set_room_portal`), `PetWorld` on a host, item-use bodies, player data (HM3), sockets and the other `InvRest` seams; world: object population (`objects.md` §15, G1), footprints (G2), the key test `0x0055F140`, code drop (G3), trap / spawn parts (G4), shrine missiles (G5, G6), quest-chest gate (G7); hirelings: the C→S 0x61 swap, the classic act change and the restore's creation place (G8), `HirelingTables` has a loader but no caller; saves: every `Unapplied` step of `ActionCharacter` (the `d2s-load.md` effects), a new character's player record in the app; server messages: 0x73, monster add messages (0xAC server fields past `init.md` §24), 0x9C in the switch and the item bit stream of 0x9C / 0x9D, §7.3 r2 steps 3 and 5–10, the stat builder `0x0053BE40`, the join sequence after 0x04 (0x5B, 0x65, 0x8D, 0x5A), the loader's 0x94 / 0x22 / 0x21 / 0x5E / 0x28 / 0x29, C→S 0x67 name checks and 0x69 / 0x6A / 0x6C / 0x6E / 0x70; path: event 0 of run / knockback / sequence (MV2), mode starts of attack / skill / BL / KB / SQ (MV1), path types 0, 3, 5, 6, 8, 9, 11, 12, 15, 16 (MV8), missile init callbacks; combat: the player DT start from the damage §7.1 branch, the event layer split, the character-save part of `0x00580EC0`, corpse creation, `MovePending::corpse_pickup`; app: `set_session` and the C→S 0x67 / 0x6B send, `object_drops` and its unique bits, the game creation with `WorldSim::create_game`, `Entry::record`; client: passive skills, client stat lists, the 0x53 day-period refresh, eclipse parts, the UI parts of 0x5D / 0x63 / 0x77, monster skills at 0xAC, the NPC text-list walk (row 6, tenth fold) | specs exist as drafts or are unwritten (§7 ninth and tenth sets) |
| 4 Conformance | recording proven feasible; coverage tool done; **after PC 1 round 2 of 2026-10-08 (`real_saves_round_trip` claim unlocked): 13,430 claims over 10,919 rule units: unit 9,674 (88.6%), game-file 419 (3.8%), trace 80 (0.7%), verified 499 (4.6%), any tier 9,782 (89.6%), `--check` 0 errors; before it, after the PC 1 run of 2026-10-08 (`py tools/coverage.py --summary`): 13,398 claims over 10,583 rule units: unit 9,659 (91.3%), game-file 416 (3.9%), trace 80 (0.8%), verified 496 (4.7%), any tier 9,767 (92.3%), `--check` 0 errors (31 game-tier claim lines unlocked: game 384 -> 416, verified 464 -> 496; the rule-unit count also moved with the spec sessions); eighth fold (`py tools/coverage.py --summary`): 4,194 claims over 3,339 rule units: unit 2,858 (85.6%), game-file 197 (5.9%), trace 30 (0.9%), verified 227 (6.8%), any tier 2,911 (87.2%); `--check` passes; the denominator grew with the three new specs (`ui/text.md` 39, `render/draw-order.md` 23, `render/unit-composite.md` 41 units, no claim yet) and with 39 table-row units of `sim/rng.md` §5.2–§7 and the numbered edge-case lists (`spec-unit-numbering`: rules 3,106 → 3,190 on its own base), so the any-tier share fell from 91.4% at the seventh fold; the tool's `verified` count counts game-file claims on `#[ignore]` tests that never ran** | `tools/trace-recorder`: 32,543 recorded RNG draws match the spec exactly. `py tools/coverage.py --summary` at `edd9925` (all branches merged, sixth fold): 3,542 claims over 3,106 rule units: unit 2,612 (84.1%), game-file 189 (6.1%), trace 30 (1.0%), verified 219 (7.1%), any 2,660 (85.6%) — tested (any tier) 85.6%, verified 7.1%; the percentages fell because the denominator grew by 402 rule units of specs written since (`inventory` 148, `pathing` 125, `path-placement` 84, `composition` 16, `camera` 11, `capture` 9, `sprite-placement` 9; `pathing`, `camera` and `sprite-placement` have no claim) while claims grew by 283; for the `4b5b0bf` figure: 3,259 claims over 2,704 units, unit 2,404 (88.9%), game-file 187, trace 30, verified 217, any 2,459 (90.9%); the 28 game-tier claims the `gaps-data-formats` session added and the one of `gaps-client-formats` (`ds1.md` edge cases) sit on `#[ignore]` tests that have never run, and the tool counts them (verified was 188 before the first 28), while that note and `docs/COVERAGE.md` §3 count a game claim as verified only while its latest local run passes: read 219 as an upper bound until §5 C17, C20 and C38 are run (the two game claims added since sit on `path::search::tests::expfield_live`); the 67 game-file tests of the sixth fold carry no claim yet (§1 3w), so none of them is in 189 (history: `--summary` total line on main at `fd37fba` was 3,164 claims, unit 86.6%, any 88.6%, verified 216; the `coverage-claims` branch measured 1,520 claims, unit 1,072, verified 180 before the implementation notes landed; per-branch figures of the gap-test notes, 67.1% → 71.6–73.5% any tier on their own bases and 88.8% → 89.2–89.9% for the second four, are not additive; the 90.9% above is the one merged run). Verified units come from the sim-0006/0007/0008 replays (tick.md 14, unit-order.md 12, rng.md §3 r2 / r4) and three ignored game-file tests (§5 C1, C8): the unit tier is claims by synthetic tests, not fidelity; seventh fold (all branches merged one by one, merged tree at this fold: `py tools/coverage.py --check` 4,038 claims, 0 errors; `--summary` 2,839 of 3,106 rules any tier (91.4%), unit 2,791 (89.9%), verified 219 (7.1%, unchanged: tick, rng and the game-file / trace checks of earlier folds); the notes' own gates reported 3,403, 3,813 and 3,869 claims; `pathing.md` 114 of 125 units any tier, `path-placement.md` 77 of 84, `camera.md` 11 of 11, `sprite-placement.md` 7 of 9, repository 2,827 of 3,106 (91.0%) any tier on `gaps-path-render`'s base; `prop-*` and `game-tests-inventory-path` add no claim; `mutants-monsters-missiles` adds 115 unit-tier claims. Replay harnesses for movement and placement exist but have never run on a 1.14d recording (row 3ab). Still no verified unit from this fold |
| 5 Local server + bridge | **Tenth fold:** the session is C→S 0x67 → 0x6B → first tick through `SessionFlow` (row 3ap); the nine ids of CM-1–CM-6 have handlers (0x21–0x23, 0x2C, 0x53, 0x5D, 0x63, 0x77, 0x94; `impl-client-msgs-3`: 62 owned ids), then `impl-pc1-final` gave every owned S→C id a handler (167 of 181 owned; the rest are out of scope); `Dispatch::from_spec()` builds again, so the 85 `d2-client` reds (`TableError::Mismatch`) are gone. **Ninth fold:** the in-process server sends the session join (row 3ao); 53 of the 181 S→C ids of `bridge-dispatch.tsv` have an owner spec and a handler (0x7A / 0x81 from `impl-client-staging`, the rest from the `client/msg-*` and `model.md` specs), so the line "every S→C id still unowned" below is stale; 0x2C, 0x53, 0x5D, 0x63, 0x77, 0x94 stay `TBD` (`impl-client-msgs-2`, §7 CM-1–CM-6); the bridge owns a client DRLG (row 6, ninth fold). Earlier: bridge design `specs/client/bridge.md` and skeleton `d2-client::bridge` done; `bridge::local::LocalLink` implements `ServerLink` on `d2_server::host::Host` (`claude/p5-local-server`, synthetic data only, unverified; `d2-client` now depends on `d2-server`); every S→C id still unowned (`bridge-dispatch.tsv` all `TBD`); session code is the placeholder `PendingSession`; the app now builds the single-player game on a server thread (row 6) | `cargo test -p d2-client bridge`: every synthetic vector of `bridge.md`, the dispatch TSV check with its perturbation test, a windowless Bevy `App` mirror test; `bridge/local_tests.rs` headless end to end on the real host, `ProtoSizes` and `SimGame<ActionSim, ActionWorld>` (two-act synthetic DRLG, seed 1234, a sorceress at (42, 20) beside a Cold Plains waypoint): C→S 0x49 drained, dispatched (result 0), ticked, flushed, and the exact S→C `0D 00 <guid> 01 2D00 1700 0000` reaches a synthetic 0x0D handler that adds unit (0, guid) to `ClientWorld`; duplicate filter, protocol version check (a wrapper reporting version + 1 refused), unknown / unowned ids; `app_frame_loop` and `app_single_player` (row 6) run the same link through a Bevy `App` for 300 ticks |
| 6 Client | design drafts `specs/client/{render-pipeline,assets,ui,audio}.md` (d2rs-own). Cloud tasks implemented (plain-Rust infrastructure, no original behavior): **C1** paths + loaders, **C2** residency cache, **C3** frames + atlas, **C4** scene + CPU compositor, **C5** GPU compute compositor (`gpu_compositor`: WGSL, integer math, no sampler, 16×16 workgroups), **C6** verify harness (`verify`: case files version 1, `--perturb`, `GpuCompositor` seam), **C6×C5** GPU half of the synthetic cases wired (`verify::gpu::Wgpu`, `[[unit]]` COF cases), **C7** COF composite mechanics (`composite`), **C8** UI core, **C9** controls file, **C10** audio core. Nothing original-behavior is reproduced: every §B point is a `TODO(spec: …)` hook or a `ComponentResolver` method. GPU byte-exactness **proven on a real GPU** (Intel HD Graphics 630, Vulkan: `gpu_compare` 12/12, `d2-client verify` 11/11, perturbations exact, §5 Done 2026-10-06) and on Mesa llvmpipe 25.2.8 | CI unit tests from each spec's vectors (`d2-client` per branch: frames 53 pass + 1 ignored, scene 20 tests, ui 20, controls 16, audio 25; `gpu_compositor` 11 + 2 ignored, `composite` 11 + 1 ignored, `verify` 191 lib tests pass, 5 ignored on the `p6-verify-gpu` branch); llvmpipe: all 12 `gpu_compare` cases and all 10 synthetic verify cases 0 differing, `--perturb 7` reports exactly 7 on every half; game-file and real-GPU halves queued (§5) |
| 6 Client, fifth fold (`p6-window`, `verify-map`, `client-own-gaps`) | **The client is an app on a window** (`cargo run -p d2-client`, subcommand `play`, now the default; `view` stays): `SimGame<ActionSim<LocalSeams>, ActionWorld>` (two-act synthetic DRLG, sorceress beside the Cold Plains waypoint, 0x49 on its real provider) behind `d2_server::host::Host` in `bridge::local::LocalLink`, **on a server thread** (`app::server_thread::ThreadLink`, `Send + Sync` for any link; the game is not `Send`: `DrlgWorld` holds un-`Send` boxed `TileSource` / `LevelTypes` and `wiring::worldgen::SharedTypes` is an `Rc<RefCell<_>>`; `d2-sim` unchanged); per frame `BridgePlugin` pumps the server in `PreUpdate`, the world view packs the frame in `Update`, and the render-graph system `compose_node` runs the compute compositor on Bevy's own device (`Gpu::from_device`) and copies its RGBA rows into the presented texture, no readback. With `D2_GAME_DIR` (and no `--synthetic`) the `levels` / `objects` tables are the user's and the waypoint object is the first `objects` row with operate function 23 and init function 17 (`waypoints.md` §5.1 r1); the DRLG stays synthetic. **The window is black by design:** no S→C id has an owner spec, every §B rule is `Unspecified`, the frame palette is all zeros. **Frame store done** (`frames::FrameStore`: `(FrameSetKey, index)` → dense `scene::FrameId`, a `scene::FrameSource`, builds a C3 atlas; `composite::build_with` takes ids from it) and **the `map` verify case ported to the compositor** (`verify/map.rs`, chunks of ≤ 1024², three byte comparisons, `--perturb N` flips N indices and N RGBA pixels). **d2rs-own gaps now with code** (`client-own-gaps`): `audio::pool::SoundPool` (audio §A1 r1, r3; assets §A5 `sounds`), `assets::prefetch::PrefetchQueue` + `Pool::offer` (assets §A4 r3), `ui::text::layout_text` + `TextRules` (ui §A3); `WavDecoder`, the sound-id → file map and the text rules stay `TODO(spec: …)`. **Bug found and fixed in the staging** by the first window run: joining with `Some(room)` skips the client room switch (`rooms.md` §4.1), so tick step 9 freed the player's room and the next client update panicked (PW1) | `app_frame_loop` (2 tests: 300 ticks, 0x49 → unowned S→C 0x0D, presented image = CPU reference; GPU node output read back = CPU reference byte for byte, skipped without an adapter), `app_single_player` (4 + 1 ignored: `Send + Sync`, failing builder, deterministic build on the thread, live tables); `d2-client` lib 224 pass, 6 ignored. llvmpipe only (Xvfb, `xvfb-run cargo run -p d2-client -- play --frames 1000`, synthetic): 1,000 frames, 419 server ticks (~25/s at ~60 fps), `gpu: true`, node frames 999, exit 0; ignored GPU tests, `gpu_compare` 12/12 and `--perturb 7` exactly 7 re-run after the `device.rs` refactor; `verify::map::tests::gpu_map_matches_cpu_reference` (synthetic 100×70 map, chunk sides 32 / 64 / 1024) 0 differing, `--perturb 7` exactly 7 on CPU binned, GPU indices and GPU RGBA, `gpu_half` still 10/10; real `townN1` and a real GPU not run (§5 C24–C31). Coverage: audio 6 → 8 of 10 units, assets 8 → 9 of 12, ui 9 → 10 of 13 (unit tier); `bridge.md` §8 r1–r3 claimed on the frame loop; `render-pipeline.md` §a10 two claims (the map comparison and the exact `--perturb` count) |
| 6 Client, render composition and capture (`render-composition`, `render-capture`) | **Composition** (`render/composition.md` §2–§6, implemented in both compositors, byte-identical on llvmpipe, **unverified** against 1.14d: no capture): the pixel write is now `dest' = map[base + dest][src]` (**behavior change**: was `[base + src][dest]`; `MapTable::push_table` takes `table[dest][src]`, the PL2 layout, so a PL2 table is pushed unchanged; `verify/mod.rs` fills `[[table]]` rows as `table[dest][src] = rule(src, dest)` so existing case files keep their meaning); `scene::frame::FrameCycle` / `FramePlan` (persistent index framebuffer, `plan(blank_screen)` = `{clear_rows: H − 47 or 0, clear_after}`, `UNCLEARED_ROWS` = 47, `compose` / `commit`, `set_post_clear`); GPU params 32 → 48 bytes (`clear_rows`, `clear_after`) and a storage binding 9 `base`; `scene::present_palette(pl2)` (index i = `pl2[4i..4i+3]`, 0 included). Kept as TODOs: `composition.md` OQ2 (order of `L` and `T`), §7 DirectDraw (display type 3 only), §1, §3 r1 / r3 (`camera.md`, `draw-order.md`). **Capture** (`render/capture.md` §4–§8): case kind `scene` (`verify::capture`, `capture_case`, `capture_tests`; `crates/d2-client/capture-cases/{stability,placement,camera,composition}-0001.toml`, `raw = "latest"`): `check = "stability"` complete (PNG re-hash, `clear_counter > 0` frames all index 0, one `index_sha256` per state group, at least two keys seen at least twice); `check = "compare"` composes the recorded state with the CPU reference and compares indices (count + first mismatch) and the 768 palette bytes, then the GPU half, but **no `SceneSource` is wired**: `main.rs` passes `SceneNotWired`, so a compare case ends `SCENE NOT WIRED` (exit 2, never a pass) after its integrity checks. Implemented, **unverified** (never run on a recording) | composition: `cargo test -p d2-client`: lib 257 pass, 6 ignored on its branch; `gpu_compare` 12 → 18 cases (`frame-blank-screen`, `frame-no-clear`, `frame-post-clear`, `pixel-write`, `blend-ops`, `frame-stress`), 18/18 0 differing and `--perturb 7` 7 / 7 on llvmpipe; M08 by hand on the old read order; `composition.md` 9 of 16 units claimed. Capture: 17 tests on generated captures (`--perturb N` → exactly N on CPU indices, GPU indices and GPU RGBA; stability `--perturb` → N re-hashes); the PNG reader decodes the bytes `record_frames.py` writes for its selftest image; `capture.md` 6 of 9 units claimed |
| 6 Client, integration (`p6-integrate`, `p6-world-view`) | **World view** (`d2_client::world_view`, `p6-world-view`, d2rs-own, no original behavior): per frame bridge → UI (`run_ui`, intents forwarded) → `build` (tiles, units through `composite`, UI items, stable sort by key) → compose (GPU compositor on Bevy's device, else the CPU reference) → an 800×600 image on a sprite at the integer scale. Every original rule is a hook trait (`ViewRules`, `UiRules`; `Unspecified` draws nothing the model does not state and errors on a rule it lacks, never guesses); the hook table with owner specs is in the note. **Integration** (`p6-integrate`, wired, headless tests on synthetic data, live-file and real-GPU halves queued §5 C40–C44, **unverified**): `ViewAssets::frames: frames::FrameStore` (ids = the store's insertion order, `GpuAtlas` packs it incrementally, `ViewError::AtlasAhead`; `FrameTable` / `BoundFrames` removed); `verify` runs every case on one shared `verify::gpu::Wgpu` (`map::run_with`); UI text through `world_view::text_sprites` (`ui::text::layout_text`; hook trait `TextHooks`, `Unspecified` refuses); sound through `app::sound` (`GameAudio`, `PoolBank` → `SoundPool`; placeholders `NoSoundTable`, `NoWavDecoder`, `NoCues`, so the window plays nothing yet, by design); live levels: `GameData::Live(Arc<LiveData>)` = waypoint tables + `LevelTables` + `WorldFiles` + `ArchiveSet` (`GameData::select`: no directory or `--synthetic` → synthetic, a directory that does not load is an error), `WorldTypes` behind `SharedTypes`, acts 0 and 1 created with town level ids 1 and 40, init seed = `--seed` (game +0x7C, source unspecified, §7 PI1), Cold Plains generated and the first rooms of Cold Plains and Lut Gholein streamed. Not wired: the spawn room and position of a joining player, the waypoint object from level presets, every §B rendering rule (the window stays black), `app::Mode::Verify` and `render/` cleanup | world_view 9 tests (205 lib pass on its branch; golden FNV hash `0x03866d4be03b3500` of the CPU frame, perturbation tests, `gpu_packing_emulates_to_the_cpu_image`); p6-integrate: `app_frame_loop.rs` new `frame_loop_uses_the_frame_store_text_layout_and_sound_pool` and `placeholder_hooks_refuse_text_and_sounds`, `app_single_player.rs` `data_selection_falls_back_only_without_game_files`, `world_view/tests.rs` `frame_ids_are_the_frame_stores`; `sh tools/gate.sh` PASS, llvmpipe `adapter: llvmpipe (LLVM 20.1.2, 256 bits)`, `xvfb-run … play --synthetic --frames 600` exit 0 |
| 6 Client, specs landed from the local session (eighth fold; no handoff note) | **New specs, all draft, no claim yet**: `specs/ui/text.md` (+ `text-fonts.tsv`, 39 rule units: 1.14d text rendering: fonts, glyphs, colour codes, wrap, and the answer to CG2), `specs/render/draw-order.md` (23 units, items B6 and B10), `specs/render/unit-composite.md` (41 units, item B4) with `unit-directions.tsv` (449 lines), and a render follow-up pass over `camera.md`, `capture.md`, `composition.md`, `sprite-placement.md` (answers RC1–RC6, RW1, RW5, RN1–RN3, CP1–CP4: roofs use the floor drawer `0x004F68E0`, per-block wall culling (camera §7), exact shake arithmetic with no zero divisor reached (camera §8), composition §5 `T[256d + L[s]]` with P dropped, act load sets the clear counter, the capture's hooks / level / cursor rule / light quality / weather / draw log; **the "light flicker" is the cursor animation**) plus `record_frames.py` 0.2.0 (capture format 2, `frames-raw-2`, `--draws-every` draw log, celfile records, selftest with perturbation checks; `traces/FORMAT.md`). **Implementation sessions are running: `impl-ui-text`, `impl-draw-order`, `impl-unit-composite`, `render-followups-impl`**; their notes are the next fold's input. Until they land the code keeps its `TODO(spec)` hooks and the app's window stays black (§2 step 7q) | spec index check passes (`py tools/spec_index.py --check`); `py tools/coverage.py --summary` at this fold lists `ui/text.md` 0 of 39, `draw-order.md` 0 of 23, `unit-composite.md` 0 of 41 |
| 6 Client, ninth fold: rules (`impl-lighting-blend`, `impl-draw-order-2`, `impl-ui-panels`, `impl-audio`, `impl-client-staging`) | **Implemented, unverified** (no capture compared, no game-file run): `rules::lighting` (light map, records, sources, environment, overrides, per-draw values, `LitRules` through `ViewFeed::light`; `specs/render/{env-periods,wall-light-points}.tsv`), shading §6 colormap shifts, blend §5 shadow positions and §8 GDI lines / rectangles (harness case `gdi-lines-rects`); `rules::draw_order::{sight, weather, background, edges}` and the draw-order answers (group-mode fade, pool overflow, clock arithmetic, flag 0x20000 after the draw, automap reveal), UC2–UC5, UT1–UT5; `ui::{layout, states, panels::*}` (`SetUIState` with gate, open mode and cursor jump; character, inventory, skill tree, stash, cube, waypoint, NPC menus, shop, border); audio `d2_formats::wav`, `d2_data::sounds`, `audio::{sound_table, triggers, environment}`; client model 0x7A / 0x81 (`ClientWorld::pets`, the hireling GUID), the 0x15 level and palette act; the formats sweep and four S→C masks follow PC 1's answers. Strict errors remain for passes 1, 4, 9, edge floors, sight without collision rooms, the eclipse, |dx| = |dy| lines and unit shadows (no draw-order item) | `cargo test -p d2-client --lib rules::` (lighting 25 + 32 + 7 + 3, shading shifts 8, GDI 8), `--lib ui::panels` (86), `--lib audio` (132), `cargo test -p d2-formats wav`; GPU harness `--include-ignored` 0 differing on llvmpipe; coverage `blend-modes` 20/20, `shading` 29/32, `lighting` 81/90, `draw-order-2` 43/44; checks C67–C75, C88 |
| 6 Client, ninth fold: wiring and the client DRLG (`wire-client-staging`, `impl-client-drlg`, `impl-client-msgs-2`) | **Wired, unverified**: with game files `play` runs the original UI (`ui::original::OriginalUi`: `UiStates` the authority, inventory / skill tree / character / border adapters, `dev` hotkeys I / C / T, panel art from the archives, the UI open mode is the camera's) and the original audio (sound table, `D2Wav`, `SoundDriver`: one `run_tick` per server tick, UI sounds the only trigger feed). The bridge owns a client DRLG (`bridge::drlg::ClientDrlg`: built by 0x03, rooms in / out of sight by 0x07 / 0x08, `active_rooms`, the room of a point, unit seeds from the room seed with fatal 0x13C, the player's level, BlankScreen, `Indoors`); the act palette is presented from the first frame (`app::palette`). Pending with named reasons (`world_view::model_feed::PENDING`, `audio::driver::PENDING`, `ui::original::PENDING`): near rooms, map tiles, tile art and blocks (CD-6, CD-7), light, weather, the environment machines (day phase, weather), sound `blocked`, the S→C handlers 0x2C, 0x53, 0x5D, 0x63, 0x77, 0x94 (`impl-client-msgs-2`: no owner spec, no code), character totals (CM-8), the client seed (WC-10). The world itself is not drawn yet (no map tiles) | `cargo test -p d2-client` (lib 822 and every target on `wire-client-staging`; `tests/app_original_ui.rs`, `tests/app_client_drlg.rs`, `bridge::msg::tests_drlg` 5, `app::palette::tests` 2); checks C76, C77, C81–C83, C86, §5 S9-A7 |
| 6 Client, tenth fold: handlers, the output channel and the client DRLG timer (`impl-client-msgs-3`, `impl-client-drlg-2`, `impl-pc1-final`, `impl-pc1-wiring`) | **Wired, unverified**: **handlers** `update_item_oskill` / `update_item_skill` / `set_skill` / `base_skill_levels` (`bridge::skills::SkillList`, `ClientUnit::skills`, `ClientTables::skills`; `play` fills the rows from `skills`), `play_sound`, `darkness` (the act's `Environment` is created by 0x03; `ClientWorld::{environment, eclipse_pending, base, total}`), `quest_status`, `waypoint_menu`, `trade_action`; `bridge::output` (`Output`, `Outputs`, the `bridge.md` §10 r8 table check) with `Bridge::take_outputs` and `world_view::present::deliver_outputs` (UI outputs to `OriginalUi::apply_output`, `ServerSound` to the sound request list: `UiSounds` holds one ordered `Vec<SoundRequest>`); `ui::original::msg_ui` (quest-log latch, waypoint menu state, trade state; stash / cube set their flag and inventory mode, the panels are not installed); a handler for every owned S→C id (`impl-pc1-final`: 167); **0x28 NPC dialog** (user decision, `msg-ui.md` OQ10 option A): the UI layer evaluates the case (`NpcTextList`, `dialog_case`: B0 / B1 / B2 / rest) and hands it back; `Bridge::npc_dialog_branch` applies the model writes and sends C→S 0x31 into the `world::DIALOG_REPLY_SLOT` right after the 0x2F (an unanswered slot is dropped at the next frame); only the one-entry kind-0 text list (A 37351) is answered, `[0x007C0C68]` stays 0 (no writer specified). **Client DRLG**: the client build timer (`Drlg::{build_timer, build_cursor}`, `rooms.md` §4.6; `ClientDrlg::client_update`, `[0x007A0498]` counter, level free on every 13th update), `RoomUnits` (a unit's room is its list membership; `set_room_order` writes the draw's y sort back), `TileInfo { roof_height, height }`, `Drlg::wall_coord`, 0x07 at a point in no room crashes at `0x0061B672`, `MapState` (`world_view/near_rooms.rs`: the near-room feed from the client DRLG; `ModelFeed::with_map` is opt-in, `play` keeps it off), `rules::draw_order` sorts each room's units by y; the server 0x7A encoder writes owner @5 / pet @9 (I-1). **Pending with named reasons**: passive skills (`SkillError::PassiveState` after the list is updated), client stat lists (`total` = `base`), the 0x53 day-period refresh `0x0061C100` and the object refresh `0x004BC5E0`, eclipse parts (`0x0061BDF0` was specified later: `impl-render-missile-answers` applies it), 0x5D screen messages and the quest-log table `0x00723F30`, 0x63 input reset `0x0044DA40`, 0x77 codes 0x00–0x02, 0x05, 0x06, 0x0A, 0x2C events 12 / 16 / 17 / 18 and every follow-up, monster skills at 0xAC (the `monstats` skill columns are not in the client tables), the stash and cube panels; unit facts for the near rooms (flags, `unflatDead`, `DrawUnder`, states 7 / 143 / 146, sight test), tile art and blocks (light), light records, unit flag 0x800000 with C→S 0x4B, item / missile / tile room lists, the app supplies no `objects`, `states`, `skilldesc`, `overlay_count` tables nor `expansion_installed`, so those handler parts are skipped in `play` | `cargo test -p d2-client`: 999 passed, 0 failed at `48c55fd` (base 85 failed); `bridge::output` `variants_match_the_spec_table` + `a_changed_row_is_reported`, `bridge::msg::tests_drlg` (5 new), `bridge::tests::ui_answers_npc_dialog_with_0x31_in_order`, `msg_ui::tests::{npc_text_list, npc_dialog_case, npc_dialog_answer_is_kept_for_the_bridge}`, `rules::draw_order::tests::the_fill_sorts_each_rooms_units_by_y_stably`, `d2-sim drlg::tests::rooms::client_build_timer_*`; checks C93, C96 |
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
   **Ninth fold:** C→S 0x13 with unit type 2 runs the object code
   (`WorldHost::objects`, row 3al) and quest objects run on the host's quest
   control; the hireling ids C→S 0x46 / 0x47 stay `NoOwner` (AI spec); what
   is left of this step is in 7v.
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
   **Ninth fold (`impl-server-join`, row 3ao): the game part of the join is
   done** (`adapters::session::enter_game`: 0x59, 0x0B, 0x03, game entry;
   the room switch's 0x07s), and `play` builds the client DRLG. **Left:**
   0x04 and client state 3 (SJ-4), the messages between 0x0B and 0x03 and
   0x53 (SJ-3), 0x01 / 0x00 / 0x02 (SJ-5), the switch's add messages (SJ-8),
   0x08 (SJ-6); a session spec for the C→S 0x67 / 0x6B path; folding the
   staged e2e games onto `enter_game` (7v (f)).
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
   **Ninth fold:** (e) in part (the `dev` hotkeys I / C / T open the
   original panels, `UiRoot` is in play mode; `wire-client-staging`), the
   audio half of (f) (`SoundDriver` and the sound table in `app::sound`),
   the join-time placement (row 3ao) and the act palette (row 6, ninth
   fold) are done. Still open: (a), (d), the prefetch half of (f), the
   waypoint object from presets in the app (the app's game still runs no
   `WorldSim::create_game`), PI1 / PI2, step 7l.
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
   (`use.md` OQ10). **Ninth fold:** the seed order is code
   (`WorldSim::create_game`, `wire-world-staging`); every `spec'd-here` skill
   slot has a body (row 3aj; the 83 `mapped` slots stay behind
   `SkillFunctions`).
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
   close here. **Ninth fold:** `impl-lighting-blend`, `impl-draw-order-2`,
   `impl-ui-panels`, `impl-audio`, `wire-client-staging` and `impl-client-drlg`
   landed (row 6, ninth fold): the open mode, the level and BlankScreen, the
   act palette and the light / weather / tile-block hooks exist; the feeds
   that fill them wait on the items of 7v (d). The notes of `impl-ui-text`,
   `impl-draw-order`, `impl-unit-composite` and `render-followups-impl` are
   still unfolded (no pointer line). The app's window stays black
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
7v. **Follow-ups of the ninth fold** (cloud, repo only; rows 3aj–3ao and
   the two ninth-fold rows of 6; the spec answers they wait on are §7
   ninth set). **Tenth fold:** (b) the death caller, (e) the monster part of the client pass (0x69 code 8 / 9) and the join messages of step 4 are done (rows 3ap, 3as); (c) is half done (`create_game` / `enter_game` in the app; `set_session` and the C→S 0x67 / 0x6B send are not); read the rest with 7w. (a) **Providers for the night's seams**, owner by owner:
   `Pending::ai_*` (skills: entries, checks, corpse search; path: patterns,
   placement, free points, 64-direction; population / init: spawn, queen
   class, chain byte, class for level; damage: kill; stat lists: curse flag,
   wisp buff, change-class list; quests: `QuestCall`; `impl-ai-acts2-5` OQ19);
   the `BodyWorld` `Pending` defaults (monster creation, skill-move paths,
   entry params / flags, collision and free points, room levels, item reads)
   and `PetWorld` on a host (`player::pets`; `BodyEffect::PetAdd` goes to
   `Pending`); `init_cb::JITTER` / `DAMAGE_PERCENT` in the missile wiring
   (`Pending::missile_init_callback`); the `MissileBodies` seams onto the
   providers that exist (`skills::use_::bodies::helpers` `scan_unit`,
   `aura_fill`, `apply_state`, `accepts`; `population::preset` `0x0054E600`;
   `world::quests` `0x0056D130`; `path::footprint`, `path::place`); the
   object extension seams `ChestWorld` (treasure drop, key test),
   `ShrineWorld`, `MiscWorld` (footprints, trap spawn, stats / states /
   missiles); the `QuestWorld` calls left on the rest (`wire-world-staging`
   §3 item 6) and the callers of the Act II–V object / AI / cube / item-use
   hooks; fold the seams that reach one function under two names
   (`impl-quests-act4-5` §2: `spawn_object` / `place_object`, the three
   `0x0061AED0` names, `spawn_superunique` / `_at_unit`,
   `create_object_at` / `create_missile_at`, `kill_in_place` /
   `remove_monster`). (b) **Hireling callers**: death from the kill path
   (after WW-2), C→S 0x61 (needs `MoveWorld::equip_on_merc` with a player
   and a result, then `HirelingItems` over `InvDesk`, and the item copy
   `0x0055A2A0`), the classic act change, the save restore (after the
   `.d2s` load effects), a production loader for `HirelingTables`. (c) **The
   app's game creation**: call `WorldSim::create_game` (regions, objects,
   NPC, quests) so 0x03 carries game +0x80 and the town's objects exist;
   a missile flight / object population follow later. (d) **Client feeds**
   once CD-2, CD-6, CD-7 are answered: `near_rooms` / `map_tiles` /
   `tile_art` / `tile_blocks` from `ClientDrlg` (the local player's
   adjacency array), then the light feed (`FrameLight`, `LookFeed`), the
   weather frame, the sound `blocked` line test over the client collision
   grids (`draw-order-2.md` §16, a follow-up with no spec gap), the client
   seed as a mutable model field (WC-10); the S→C handlers of CM-1–CM-6 as
   soon as a spec takes each id (register in `bridge::msg::HANDLERS`, hand
   0x63 / 0x77 to `ui::original`, 0x2C / 0x5D to `audio::driver`, drop the
   `PENDING` rows). (e) **Server messages**: the §7.3 client pass
   (0x67–0x6D), then 0x69 code 8 at death and code 9 at the end of the
   death animation (`intents-events.md` §7.4 r7; MB-8); the join messages
   of step 4. (f) **Tests**: fold the staged e2e games (`e2e_full_loop`,
   `e2e_single_player`, `e2e_walk`, `game_wired_host`, `synthetic_game::run`)
   onto `enter_game` (their scenarios then start at the spawn point); the
   real-data `#[ignore]` tests `impl-audio` §2 item 2 asks for (environment:
   50 `soundenviron` rows, songs 4,657–4,684; triggers: every TSV id
   resolves) on `SoundTableData::from_txt`; C90 and C91 need their tests
   written. (g) **Small code items**: UC1 (`composite::check` rejects
   duplicate layer records, §5.1 r5 says first match), the `ui/text.rs`
   vertical window (now detailed in `ui/text.md` §9), mixer modes 1–2, start
   offset and loop start in the core mixer (`impl-audio` §2 item 3),
   `WiredWorld::now` (vendors) still the caller's clock.


7w. **Follow-ups of the tenth fold** (cloud, repo only; rows 3ap–3at and the
   tenth-fold row of 6; the spec answers they wait on are §7 tenth set).
   (a) **Gate on the merged head**: `CARGO_INCREMENTAL=0 sh tools/gate.sh` (no
   note records one after `impl-pc1-final`); fix what it reports, then re-run
   `python3 tools/conflict_markers.py --selftest`. (b) **The app's session**:
   build the app's `SimGame` with `set_session(SessionFlow::new(0x0010_0004,
   loader))` (the loader allocates the player from the 0x67 class / name or
   loads the `.d2s` through `adapters::character`, returning `Entry::new(act,
   name)`), send C→S 0x67 in the first frame and 0x6B after the first flush
   that carried 0x02 (`client/model.md` §7 r3) through `send_system`, then
   0x03 carries the real game +0x80 (`impl-session-flow` "What the app will
   need"); set `ActionHooks::object_drops` and share one unique-bit store with
   `WiredWorld::uniques`; call `hireling_tables` into
   `InteractionState::hireling_tables`; `enable_unit_events` on every host with
   `UseRest`; give `Entry::record` a value so 0x5F / 0x23 are sent. (c)
   **Seams that have a spec now**: `Pending::monster_death_start` (the DT start
   `0x005A6FF0`, without it a production host never sends 0x69); the player DT
   start from `damage.md` §7.1 (who starts it); `MovePending::corpse_pickup` →
   `ActionHooks::corpse_pickup`; `VitalsRest::level_up_event` and the hireling
   `level_events`; the quest-object dispatcher for Act III inits (QC-6: `InitPoint`
   from the init's room, x, y, then the host providers `has_act3`,
   `spawn_monster_in_room`, `room_covering`, `special_monster`, `weapon_code`);
   C→S 0x44 → `act2::q6::item_to_object`; `d2s-tool --quests all` after
   quests OQ14; the `Unapplied` steps of `ActionCharacter`;
   `QuestTick` onto the lent host; fold `units/mode_set.rs` into
   `modes::set_mode`; the missile init callbacks (`Pending::missile_init_callback`:
   jitter `0x005C9290`, lightning fan / ring, DiabWall) with the missile frames
   inside `create_missile`, then the `JitterMissile` / `PathMissile` seams on
   the provider. (d) **Tests**: unit tests for the batch 4 skill bodies
   (`bodies-3.md` / `bodies-4.md` test vectors, 458 rule units at 0%) and for
   `umod-init-bodies.md` (26 units, 0%); the lent quest host on the wired host
   (`d2-server world/tests/quest_objects.rs`, `impl-quests-answers` Left 1);
   turn the provider on in `e2e_single_player.rs` and drop its WP1 comments;
   one path-provider `TestPending` in `e2e_support` for `e2e_full_loop.rs` and
   `e2e_night_world.rs` (N-1); check that the kill test of `e2e_night_flows` (written
   when the transcript was empty, before 0x69 code 8 / 9 were sent) agrees with
   the merged head; the `wiring::path` `walk.rs` mutation run and the
   `world::hirelings`, `player::pets`, `missiles::bodies*`, rest-of-`path/**`
   mutation runs (commands in the notes; `--cargo-test-arg=--lib`,
   `CARGO_INCREMENTAL=1`); the real-data `#[ignore]` tests of step 7v (f).
   (e) **Spec-blocked** (do not guess): everything listed under the owner in §7
   tenth set; the five "Not wired" blocks of row 3ar name the seams.

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
   **Ninth fold:** 53 of 181 ids are owned; the next six are §7 CM-1–CM-8
   (owners for 0x2C, 0x53, 0x5D, 0x63, 0x77, 0x94, a bridge → UI / sound
   output channel, the client's stat-list extension), then CD-1–CD-7.
10. **Phase 6 RE specs**, in the order of `docs/PLAN.md` Phase 6: the first
    four are written as drafts (`render/{sprite-placement,camera,
    composition,capture}.md`, with the recorder `tools/trace-recorder/
    record_frames.py`; render §B1, §B2, §B7, §B9) and wait for their
    recordings (§5 A, `stability-0001` first) and for the answers RN1,
    RN2, CP1–CP4 (§7); then unit composite and draw order; shading, blend
    modes, lighting; audio; UI text, panels, controls, inventory, automap.
    **Ninth fold:** shading, blend modes, lighting, `draw-order-2.md`, audio
    and `ui/panels.md` are implemented (row 6, ninth fold); their answers are
    §7 LB, DW, UP, ST / TR / EN; still unwritten: `ui/controls.md` (key table,
    WC-4), `ui/inventory.md`, `ui/automap.md`, the client build timer (CD-2).
11. **Spec and code questions of the fourth fold** (spec writing / Ghidra,
    §5 B, §7): GD8–GD11 (patch-layers report order and A05, the `.bin`
    automap check, numbering the `rng.md` cross-reference tables), GT1,
    GP1, GB1, QH1 and EN1; plus the earlier GD items the new notes
    extend (`loading.md` §7.3 also omits `setitems.set` → `sets.index`).

12. **Spec text the sixth-fold audits found missing** (**ninth fold: mostly
    done.** On main `ai-functions.tsv` has 93 `spec'd-here` and 55 `unread`
    rows (no `summarized` or `D2MOO-only` left), `srvdo.tsv` 53 and `srvhit.tsv`
    71 rows all `spec'd-here`, Acts II–V quests have specs and code
    (`quests.tsv`'s `spec` column still says `catalogued`, QC-7, QE-7),
    `functions.tsv` has 130 `spec'd-here` slots with bodies and 83 `mapped`
    (64 `srvdo`, 19 `srvst`) without; left: the 55 `unread` AI rows, the 83
    `mapped` skill slots, any Act I callback still reported `unhandled`, the quest
    bodies of §7 QB-2, QB-17–QB-19, QE-1–QE-6. The counts below are the
    sixth fold's and are kept for the record.) (spec writing /
    Ghidra, `spec-bodies-*` §3; every entry needs the bar of `ai-bodies.md`
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
      43 FoulCrowNest, 59 BloodRaven; the Act I rows first, `ai-bodies.md`
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
    - **Quests** (`quests-act1.md` §10, `quests.tsv`): the Act I callbacks still
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

15. **Spec answers and recordings of the ninth fold** (2026-10-07): §7
    "Ninth set". **PC 1** (Ghidra / spec edits, 288 questions by spec file):
    start with the integration items I-1 (0x7A field order: server and client
    disagree on the wire today), I-2, I-3, I-6, then the owners the client
    waits on (CM-1–CM-8, CD-1–CD-7, SJ-2–SJ-8), then the spec-table edits
    (QC-7, QE-7, CS-4–CS-8, OB-1). **PC 2** (recordings / captures, 38
    questions): §5 A S9-A1 (PC 1's list: draw-order OQ7, `weather-0001`,
    levels 74 / 120, Act III entry, a kill with a drop, a hireling game, an
    act change, Might in a party, Kick / Bash, a Druid summon, a Clay Golem,
    a Countess kill, a Cain rescue), S9-A2–S9-A7, and the C entries that need
    the game running (C69, C70, C74, C75, C78–C80, C83–C85).
16. **Spec answers and recordings of the tenth fold** (2026-10-07): §7 "Tenth set". **PC 1**: `xpc-to-pc1.md` (the open lines; PC 2 → PC 1 requests and the notes' spec gaps), then the owner lists of §7 tenth set that it does not repeat (client messages CM3, client DRLG CD2, umod callbacks UC, skill slots MS, DRLG FD). **PC 2**: `xpc-to-pc2.md` (the PC 1 → PC 2 requests and the world-rest gaps G1–G8), then §5 A recordings R2-39–R2-46 and the C entries that need the game running (C92–C98).

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
In the ninth fold (2026-10-07, 22 notes): `impl-ai-acts2-5`,
`impl-skill-slots-2`, `impl-missile-bodies-2` and the four quest sessions
(step 12 in the main), `impl-objects`, `impl-hirelings`, `wire-world-staging`
(steps 2, 3, 7b: objects, hirelings, the creation seed order),
`impl-drlg-act3-5`, `impl-room-population` (step 7: the DRLG population
reads), `impl-d2s`, `impl-server-join` (step 4, the game part of the join),
`impl-client-staging`, `impl-lighting-blend`, `impl-draw-order-2`,
`impl-ui-panels`, `impl-audio`, `wire-client-staging`, `impl-client-drlg`
(steps 5, 7q, 10 in part) and `impl-client-msgs-2` (no code: step 9's next
owners).

Per-session notes live in `docs/handoff/*.md` (detailed record of seams,
public APIs, design choices and per-site open questions). 137 of the 171
notes are folded into this file as of the ninth fold and start with the
pointer line; the 34 others (e.g. `impl-ai-act1`, `impl-quests-act1`,
`impl-skill-bodies`, `impl-ui-text`, `impl-draw-order`,
`impl-unit-composite`, `render-followups-impl`, `local-buddy-*`,
`mutants-*` of 2026-10-06) have no pointer line and are a later fold's
input. Read the note of a module before you change it.

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
| `crates/d2-sim/src/items/moves/{mod,seams,deferred,handlers,ground,layouts}.rs` (+ `moves/tests/`) | `handle` (size check, dispatch by id; `HANDLED` = 23 ids), one function per C→S id 0x16–0x29, 0x50, 0x61, 0x63; `ITEM_ACTIONS` + `dispatch` (`0x005973F0`), `player_update`, `ground_update`, `send_item_page`, `send_to_belt`, `mark`, `category`, `owner_refresh`; ground pickup / drop / expiry / cube spill / gold piles; byte builders 0x9C / 0x9D / 0x7D / 0x47 / 0x48 / 0x42 / 0x3F / 0x19 / 0x1D–0x1F; seams `InventoryOps`, `MoveUnits`, `MovePending`, `MoveWorld` (groups and defaults in `impl-moves.md` §4) | `items/inventory-moves.md` §6–§11, `item-actions.tsv` |
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
| `crates/d2-server/src/adapters/sim.rs` | `SimGame<D = Unspecified, W = NoWorld>`: one host `world: W`; `tick_faults: Vec<(ClientId, WorldError)>`; `SimParts` / `parts()`, `player_of`, `client_of`; `handle` tries items (cube), the moves (`WorldHost::moves`), world, skills, then the stub; `Tick` needs `D: EventDispatch + TickHooks, W: WorldHost<D>`, queues `take_sent` after the tick, then runs `items::moves::update_pass` | `intents-events.md` §2–§4, §3.2; `tick.md` §3; `inventory-moves.md` §6 |
| `crates/d2-server/src/adapters/handlers/world.rs`, `world/{action,wired}.rs` | `WorldHost<D>` (`npc`, `vendors`, `waypoints`, `quests`, `cube`, `skill`, `moves`, `take_sent`, `fault`; defaults `None` / stub), `NoWorld`, `WorldError::Move`; `ActionEvents` (+ `create_game(&GameFields)`), `ActionWorld<S = NoSkills>`, `Outbox`; `WiredWorld<R, S = NoSkills>` (`with_economy`, `Parts` + `cube`, `inventory: Option<InvParts>`, `TradeRest`, `HostWaypoints`, `uniques`) | `world/*.md`, `npc.md`, `vendors.md`, `quests.md`, `cube.md` §1–§2, `waypoints.md` §6, `inventory-moves.md` §6 |
| `crates/d2-server/src/adapters/handlers/items.rs`, `items/cube_world.rs`, `items/moves.rs` (+ `moves/tests.rs`) | `ITEM_IDS`, `handle` (via `WorldHost::cube`), `CubeParts`, `CubeCall`, `CubeHooks`, `Interact`, `Staged`, `Inventory`, `ItemPending`, `ItemError`, `ServerCube<H>`; `MOVE_IDS`, `is_move_id`, `MoveRest` (`InvRest` + `take_sent`), `InvParts`, `MoveCall`, `handle`, `update_pass`; 22 host-frame tests, id-table check + M08 | `cube.md` §1, §2, §8; `inventory-moves.md` §6–§11 |
| `crates/d2-server/src/adapters/handlers/skills/{mod,wired,world}.rs` | `SkillHost<D>` (`handle(Call) -> Option<Handled>`), `NoSkills`, `LearnRest`, `SkillRest`; `WiredSkills` (only `unsent`; `Default`); `World<'v, 'a, X>` over `UseView` (`SkillSeams` and `seams.rs` are gone) | `use.md`, `levels.md` §6.4, `vitals.md` §2 |
| `crates/d2-sim/src/wiring/economy/{game_fields,death}.rs` | `GameFields::from_action(seed, &ai::GameInfo, expansion, uniques)`, `GameFields::ai_info`; the death drop's creation fields come from the action wiring (`ActionHooks::ai_info`, `UnitData::expansion`), `uniques` from `DeathDrops::fields` | `generation.md` Inputs; `treasure.md` §3 |
| `crates/d2-sim/src/wiring/inventory/{mod,inv_world,ops,units,pending}.rs` (+ `tests/{mod,ground,buffer,equip,belt,stack,gold}.rs`) | `InvState`, `InvRest`, `InvDesk` (`new`, `with_inv`, `update_done`), `InvError`; `InvWorld`, `InventoryOps`, `MoveUnits`, `MovePending` for `InvDesk` (`R: InvRest + ?Sized`); 26 tests | `inventory.md` §1–§5, `inventory-moves.md` §6–§11 |
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

**Ninth fold (METHODS M21; each in its note):** new code map entries, one
line each (the notes list every public item and seam).

| Path | What | Spec |
|---|---|---|
| `crates/d2-sim/src/monsters/ai/{bodies2,bodies3,bodies4,bodies5,common}.rs` (+ `tests/act2.rs`..`act5.rs`) | the Act II–V AI bodies, seam `AiActs` in `seams.rs`; `View` answers part of it, `Pending::ai_*` the rest | `monsters/ai-bodies-2.md`..`-5.md`, `ai.md` §3, §7, §10 |
| `crates/d2-sim/src/skills/use_/bodies/{helpers2,helpers3,starts2,dos2,b3_lvl01..b3_lvl30,fake,tests2,tests3}.rs` | batch 2 / 3 skill bodies, `BodyWorld` (+ `BodyEffect`, `PathOp`, `MissileRequest`), `START_BODIES` / `DO_BODIES` | `skills/bodies.md` §6–§8, `bodies-2.md` |
| `crates/d2-sim/src/player/pets.rs` (+ `pets_tests.rs`) | player pet lists over `PetWorld` | `sim/pets.md` §1–§9 |
| `crates/d2-sim/src/missiles/{bodies_ext,bodies_ext2}.rs` (+ `tests/ext.rs`) | every non-null server-do / server-hit body; `MissileBodies` seams in `seams.rs`; `hit::damage_tail`, `create.rs` zigzag callback | `missiles/bodies.md` §1–§30, `bodies-2.md` §31–§62 |
| `crates/d2-sim/src/world/quests/{act2,act3,act4,act5,late}.rs` (+ subdirectories and tests), `act1/{q4,q5,q6}.rs` | Acts II–V quest records, callbacks, objects, timers, hooks; the Act I remainders | `world/quests-act1-rest.md`, `quests-act2.md`, `quests-act3.md`, `quests-act4.md`, `quests-act5.md`, `quests-act5-2.md` |
| `crates/d2-sim/src/world/objects.rs`, `objects/{chests,shrines,misc}.rs` | object control, init / operate dispatch, chests, shrines, doors, wells, torch, portal | `world/objects.md`, `object-functions.tsv` |
| `crates/d2-sim/src/wiring/action/objects.rs` | `ObjectState` (control, tables, host tick, `obj_seed`), `ObjectView`, quest-object queue (`QuestObjectCall`) | `objects.md` §3, §7, §14; `rng.md` §5.2 |
| `crates/d2-sim/src/world/hirelings/{rows,pets,life,level,items}.rs`, `wiring/interaction/hirelings.rs` | hirelings (`HirelingWorld`, `HirelingItems`); `HireView` on the desk, `HirelingRest` | `world/hirelings.md` |
| `crates/d2-sim/src/wiring/worldgen/creation.rs`, `wiring/economy/{quest_objects,quest_host}.rs` | `WorldSim::create_game`; the quest-object dispatcher and `HostQuests` on the wired host | `rng.md` §5.2; `world/quests*.md` |
| `crates/d2-sim/src/drlg/outdoor/{jungle,act3,kurast,act5}.rs`, `drlg/logic.rs` | Acts III / V outdoor builds; logical rooms and the §11.5 population reads | `drlg/outdoor-act3-act5.md`, `outdoor.md` §9, §11; `drlg/levels.md` §11 |
| `crates/d2-formats/src/{d2s,wav}.rs`, `tools/d2s-tool` | `.d2s` reader / writer and the dev tool; WAV parser | `formats/d2s.md`, `formats/wav.md` |
| `crates/d2-server/src/adapters/session.rs` | `enter_game`, `load_act`, `JoinError` | `sim/path-placement.md` §11, §13; `client/model.md` §11 |
| `crates/d2-client/src/rules/lighting/`, `rules/{shading,blend}.rs` additions, `rules/draw_order/{sight,weather,background,edges}.rs` | light map, environment, overrides, per-draw light; colormap shifts; GDI and shadows; weather, backgrounds, edges, sight | `render/lighting.md`, `shading.md`, `blend-modes.md`, `draw-order-2.md` |
| `crates/d2-client/src/ui/{layout,states,original}.rs`, `ui/panels/`, `world_view/panel_art.rs` | panel tables, `SetUIState`, the panels, the original UI in the app | `ui/panels.md` (+ TSVs) |
| `crates/d2-client/src/audio/{sound_table,triggers,environment,calls,driver}`, `crates/d2-data/src/sounds.rs` | sound table, triggers, environment, the sound layer and its driver | `audio/sound-table.md`, `triggers.md`, `environment.md` |
| `crates/d2-client/src/bridge/{drlg.rs,msg/pets.rs}`, `app/palette.rs` | the client DRLG copy, 0x7A / 0x81, the act palette | `client/model.md` §9, §12, §14; `render/composition.md` §4 |

**Tenth fold (METHODS M21; each in its note):** new code map entries, one line
each.

| Path | What | Spec |
|---|---|---|
| `crates/d2-server/src/adapters/session_flow.rs`, `seams.rs` (`Intents::session_message`) | C→S 0x67 / 0x6B on the host drain: `SessionFlow`, `CharacterLoader`, `SessionFault` | `sim/intents-events.md` §2.5, §8.1, §8.2 |
| `crates/d2-sim/src/units/messages.rs`, `world/environment.rs`, `wiring/action/{switch,unit_update,vitals_sync}.rs`, `monsters/mode_message.rs` | the join's pure builders, the per-act environment (0x53), the room switch, the monster update pass and mode message, the join's forced vitals | `sim/intents-events.md` §7.2–§7.9, §8; `tick.md` §6; `render/lighting.md` §9 |
| `crates/d2-sim/src/wiring/path/{monsters,missiles,motion_tests}.rs` | monster walk / run set-up, velocity and mode end; the `MissileBodies` path seams | `sim/pathing.md` §8.1, §9, §11; `monsters/ai.md` §7 |
| `crates/d2-sim/src/monsters/ai/{bodies6,bodies7}.rs` (+ `tests/act6.rs`, `act7.rs`) | the last 55 AI thinks, inits, Uber alternates; seam `AiSummons` | `monsters/ai-bodies-6.md`, `ai-bodies-7.md` |
| `crates/d2-sim/src/skills/use_/bodies/{b4_helpers,b4_mon,b4_more}.rs` | batch 4 skill bodies (83 slots), `PathMissile` | `skills/bodies-3.md`, `bodies-4.md` |
| `crates/d2-sim/src/monsters/init/{callbacks,find}.rs`, `wiring/worldgen/{umod_host,init_units}.rs` | the 32 umod callbacks, unit find, their wired providers | `monsters/umod-callbacks.md`, `umod-init-bodies.md` |
| `crates/d2-sim/src/combat/{events,vitals/experience}.rs`, `wiring/action/death.rs`, `wiring/interaction/skill_use.rs` | `combat::events`, experience, player death, the unit-event registry on `UseView` | `combat/events.md`, `combat/vitals.md` §4 |
| `crates/d2-server/src/adapters/handlers/player.rs`, `crates/d2-sim/src/units/mode_set.rs` | C→S §9 handlers; `0x00624690` | `sim/intents-events.md` §9 |
| `crates/d2-sim/src/wiring/economy/{chest_drop,quest_objects,quest_host}.rs`, `wiring/action/objects.rs`, `world/quests/act1/{q4,q5,q6}.rs` | chest drop, `ChestWorld` / `ShrineWorld` / `MiscWorld` providers, the lent quest host, Act I object routes | `world/objects.md`, `quests-act1-rest.md` §9 |
| `crates/d2-server/src/adapters/character.rs`, `world_data/tables.rs`, `session.rs` (`enter_game_from_save`), `crates/d2-formats/src/d2s/` (load flags) | save load in the master's order, the hireling table loader | `formats/d2s-load.md`, `d2s.md` §8.2 |
| `crates/d2-sim/src/drlg/{room,level,tiles,logic,active}.rs` (wall remap table, build timer, `Drlg::wall_coord`), `drlg/outdoor/{jungle,act3}.rs` | the 1.14d wall remap, the client build timer, Act III–V answers | `drlg/wall-remap.md`, `rooms.md` §4.6, §9.6; `outdoor-act3-act5.md` |
| `crates/d2-client/src/bridge/{output,skills,msg/{skills,sound,lighting,ui}}.rs`, `ui/original/msg_ui.rs`, `audio/driver.rs` | the bridge output channel, the client skill list, the nine handlers, the UI dispatch, `SoundRequest` | `client/bridge.md` §10, `msg-skills.md`, `msg-ui.md`, `audio/triggers.md` |
| `crates/d2-client/src/bridge/{world,update,drlg}.rs`, `world_view/near_rooms.rs`, `rules/draw_order/` | `RoomUnits`, the client build timer, `MapState` near-room feed, per-room y sort | `drlg/rooms.md` §4.6, `sim/unit-order.md` §5, `render/draw-order.md` §3, §9 |
| `crates/test-fixtures/src/{act1,host}.rs`, `tests/{act1_game,e2e_night_flows,monster_death}.rs`, `crates/d2-server/tests/{e2e_night_world,mutants_walk}.rs`, `tools/conflict_markers.py` | Act I synthetic set, shared wired-host session, e2e tests, the conflict-marker check | `outdoor.md` placement vector; METHODS M08 |

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
| `cargo test -p d2-sim --lib monsters::ai`, `--lib skills::use_`, `--lib player::pets`, `--lib missiles`, `--lib world::quests`, `--lib world::objects`, `--lib world::hirelings`, `--lib drlg` (ninth fold) | the Act II–V AI bodies (86 + catalogue check), batch 2 / 3 skill bodies (52 + table checks), pets (27), missile bodies (64 + catalogue check), quests Acts I–V, objects (84), hirelings (53), Acts III / V DRLG and logical rooms, each on synthetic fakes (row 3aj–3am) | repo |
| `cargo test -p d2-sim --lib wiring::worldgen::tests::creation`, `--lib wiring::economy::quest_objects`; `cargo test -p d2-server --lib world::tests` (`objects`, `quest_objects`) | game-creation seed order (perturbed without the object step); quest-object routing table vs `object-functions.tsv`; the host tick, quest objects and the hireling follow through host frames | repo |
| `cargo test -p d2-formats d2s`, `cargo test -p d2s-tool`; `cargo run -p d2s-tool -- new --name N --class C -o OUT` / `dump` / `check` / `set` | `.d2s` vectors, round trip and perturbation; generated saves read back (`D2_SAVE_DIR` + `D2_GAME_DIR`: `--test real_saves -- --ignored`, C66) | repo (`game/`, saves for C66) |
| `cargo test -p test-fixtures --test synthetic_game town_entry_sends_the_join_sequence`; `cargo test -p d2-client --test app_client_drlg` (`-- --ignored`: C81, C86) | the session join's exact bytes on the synthetic town; the app builds the client DRLG from the real join | repo (`game/` for the ignored tests) |
| `cargo test -p d2-client --test app_original_ui` (`-- --ignored`: C76), `--lib ui::panels`, `--lib audio`, `--lib rules::lighting`, `--lib rules::draw_order`, `--lib bridge::msg::tests_drlg` | the original UI and audio in a headless app; panels, sound system, lighting, weather / backgrounds / edges / sight, the client DRLG | repo |
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

Tenth fold additions: `python3 tools/conflict_markers.py [--selftest]` (also a
step of `tools/gate.sh` and the CI `check` job); `cargo nextest run -p d2-sim
motion_tests` (monster walk, missile flight); `cargo test -p test-fixtures --test
act1_game` / `--test e2e_night_flows` / `--test monster_death` (CI, synthetic);
`cargo test -p d2-server --test e2e_night_world` / `--test mutants_walk`;
`cargo test -p d2-client --test e2e_pet_action` (no longer ignored);
`cargo test -p d2-sim --lib wiring::path::mutant_tests`. Mutation runs: `cargo
mutants -p d2-sim --file '<glob>' -j 3 --timeout 60 --cargo-test-arg=--lib` (not
`-- --lib`: that limits only the run, and the build still compiles ~40
integration targets, ~100 s a mutant); with `CARGO_INCREMENTAL=1
CARGO_PROFILE_DEV_DEBUG=false` one lib-test build is ~14 s instead of ~55 s with
`CARGO_INCREMENTAL=0` (about 1.4 GB of incremental data per worker; use the
latter for the gate).

## 5. Local run queue

- **stitch-save** (`docs/handoff/stitch-save.md`): `play --new sorceress Tester`, close; `play --save <Tester.d2s>` shows the same character; second close makes `.bak`. Record any `NOT saved` line.
- **Front end (`claude/pc1-frontend`, `ui/frontend-*.md`)**: screenshots of the 1.14d front end at 800×600 (and 640×480 where named), `game\Game.exe -w`; restore any registry value changed.
  - **Front end: main menu, character select, create (`ui/frontend-menus.md` §F1.4, §F2.4, §F3.2–F3.3).** Start, skip to the main menu, screenshot (expansion layout: buttons at the §F1.4 table rects, "v 1.14d" bottom-left, logo fire animating). Single Player with ≥ 9 saves: screenshot the 8 slots and scroll once. Create New: screenshot the line-up, click the Paladin and record ≥ 50 fps (walk-forward then idle-selected loop, REC-210); type `a-b_c` and `-x` into the name box (REC-209). Look for slot / class positions and frames as in the spec tables; a different value is a finding for `ui/frontend-menus.md`.
  - **Front end: options art check, no game run (`ui/frontend-options.md` §O2 r3, Constants).** `target/release/mpq-tool.exe extract game/d2data.mpq "data\local\UI\ENG\*.dc6" <scratch>`, same for d2exp, plus `data\global\ui\WIDGETS\opt*`, `data\global\ui\CURSOR\pentspin.DC6`, `data\global\ui\MENU\textslid.DC6`. Expect pentspin widths 51, 43, 27, 9, 23, 40, 50, 52; optbar 255+35 × 33; optbarC × 37; textslid 17 frames of 12×13; `exit.dc6` reads "SAVE AND EXIT GAME".
  - **Front end: Esc-menu screenshots, 800×600 (REC-212).** Esc → each sub-menu, one screenshot each; pixel-compare with the d2rs render of `d2-client::ui::front_end::options` once it exists. Look for pentagram positions / frames, slider fill darkness (modes 1/2/0 per style), labels at x 170 (left) and right edge 630.
  - **Front end: Configure Controls screenshot, 800×600 (§O9).** Esc → Options → Configure Controls. Look for headings at x 108 / 298 / 488, y 100; row 0 text at y 122; selection bar 570 wide; Cancel / Default / Accept centred at 193 / 399 / 605. Select Inventory, Enter, then C: Character's second key becomes "None" and Inventory's first key C. Cancel returns to the Options menu with Previous selected.
  - **Front end: loading-screen pixel check (`ui/frontend-loading.md` L-rules).** Manual: `Game.exe -w` at 800×600 and 640×480; Single Player → expansion character with progression ≥ 5 → OK → Normal; screenshot during the load (or break on `0x004F6190` in `0x004565E0`). Look for: `loadingscreen.dc6` frame 0 then frame 1, top-left at (272,173) / (192,113), rest index 0, no text or bar, Loading palette; first world frame all black.
  - **Front end: act-change capture (REC-221, REC-223).** Packet capture plus per-frame screenshots: waypoint Act I → Act II, then a first-time Warriv travel. Look for: 0x05/0x03/…/0x04 order with ticks, loading frame 0 in both, 0x61 `02` before 0x04, what is on screen after the video.
  - **Front end: difficulty enable check (`ui/frontend-loading.md`).** Test saves with status +0x24 = 0x0420, 0x0520, 0x0920, 0x0A20, 0x0400, 0x0804 (fix the checksum); press OK on each. Expect: no box / box Hell off / Hell off / Hell on / box Hell off / box Hell on (the spec's test vectors).
  - **Front end: cinematics enable pixel check (`ui/frontend-credits.md`, REC-228).** `reg add "HKCU\Software\Blizzard Entertainment\Diablo II" /v "Aux Battle.net" /t REG_SZ /d 216.148.246.34 /f`, start `game\Game.exe -w`, open Cinematics, screenshot; repeat with `.178` (0xB2) and `.35` (0x23). Look for 1 / 6 / 7 enabled entries (expansion; classic 1/5/5), blank labels on disabled expansion buttons, the C8 button rects, no logo fire in the expansion menu.
  - **Front end: credits screenshots (REC-225).** Main menu → Credits; screenshot at the click, ~12 s and ~12 min. Look for the first text near y 590 scrolling up, headings colour 1 centred at x 405, left column ending x 400 / right column starting x 410, sky palette; at the end a blank screen with EXIT and no return; Esc leaves.
  - **Front end: startup chain and keys (REC-229).** `reg delete "HKCU\Software\Blizzard Entertainment\Diablo II" /v "Aux Battle.net" /f`, start the game. Expect two Blizzard videos, the D2 intro, then the trademark screen (no static Blizzard-logo picture). During a video Left arrow does not skip, `a` does. On the trademark screen Q does nothing, N goes to the main menu. Restart: no intro; the registry now holds `216.148.246.34`. (Restore the user's registry value afterwards.)

- **q-fix-idle-rooms (REC-287)**: `play` with game files, a new character in the Rogue Encampment: walk out to the Cold Plains and on until the town is well out of sight, stay about 2 minutes, walk back. Look for: every town NPC and the waypoint there **once** (no duplicate from the presets), the NPCs talk, the waypoint opens its menu. Repeat in the Blood Moor after killing a few monsters: corpses / survivors come back, nothing doubled.
- **q-fix-flow-save (REC-291)**: with game files and real saves (the C66 run's, or any game-written `.d2s` with equipped items, a crossbow among them): `D2_GAME_DIR=<install> D2_SAVE_DIR=<save folder> cargo test -p d2s-tool --test real_saves -- --ignored`. Look for: `real_saves_appearance_rebuilds_to_the_files_bytes` passes (the tool's rebuild of +0x88..+0xA7 from the save's items equals the game's bytes; a crossbow save checks REC-291 (5), the weapon class); then `play --save <a game save>` → Save and Exit → the file's appearance bytes unchanged and `d2s-tool check` OK; a 0x5C–0x5E save loaded and re-saved (version 0x60, items readable by the game).
- **q-fixture-migrate (`docs/handoff/q-fixture-migrate.md`)**: `d2-client play --new sorceress Test` in the window: the Rogue Encampment shows its NPCs, stash, fires and its own waypoint once (no build-placed waypoint any more); the first click on the waypoint opens the menu (init 17, mode 2); walking past the fires and out of town never stops the client (no fatal 0x591). Then the real-data gate of the client: `D2_GAME_DIR=… cargo nextest run -p d2-client --run-ignored only -E 'kind(test) & not test(/gpu_/)'`; findings q-fix-real-* of §4 there are expected until fixed, any other failure is new.
- **q-fix-cold-plains (spec seq check, no game run)**: open `traces/raw/20261005-232125-rng.jsonl` and list the level-seed draws of Cold Plains (seed {4014346872, 666}) at sites `0x0067438F` and `0x00666F33` between seq 11600 and 12020. Look for: the preset-30 build-list roll at 12010 (d2rs, and the stride of the 48 / 44 / 29 rolls at 11622 / 11752 / 11881) and the first cell draw at 12011, or the spec's 12009 / 12010 (`outdoor.md` Test vectors "Order of the Blood Moor build" and "Cold Plains grid"; the same note gives the type-1 group roll as seq 8396 there and 8395 in `outdoor-tilesub.md` Test vectors). Correct whichever spec line is off by one; no code depends on it.
- **q-render-compare (REC-295..299)**: with game files and a window: `cargo run -p d2-client -- play --new sorceress Test --seed 1 --dump-draws game/facts-d2rs/smoke --at-tick 40`; look for: exit 0, `draws.tsv`, `frame.tsv`, `sprites.tsv` written; then `cargo run -p d2-client -- facts-compare game/facts-d2rs/smoke game/facts-d2rs/smoke` prints PARTIAL (d2rs's own `?` cells), never an error. Then the first PC 1 scene end to end: `docs/handoff/q-render-compare.md`.

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

Ninth fold (2026-10-07, 22 notes): C65–C86 were queued by the sessions
themselves as they merged (numbers unique and in order; the merges
renumbered `wire-world-staging`'s #78–#80, `impl-client-drlg`'s C81–C83 and
`impl-server-join`'s C84–C86; C75's "C65 files" now reads C72, the eight
`wav.md` files of C72). The fold adds C87–C91 from notes whose checks had
not reached the queue (`impl-ai-acts2-5`, `impl-client-staging`,
`impl-room-population`, `impl-hirelings`, `impl-quests-act2`), A additions
S9-A1–S9-A7 (S9-A1 is PC 1's recording list for PC 2) and one B pointer to
§7 ninth set. Notes with no local check of their own: `impl-skill-slots-2`
(its e2e fixtures ran green in `impl-server-join`'s workspace gate at
`ef00681`),
`impl-client-msgs-2` (no code).

Index: Done · A player · B Ghidra / spec edits · C game files and GPU · Blocked.

- **q-realdata-gate (moves these runs to the cloud)**: once the private repo `MoggerCat/D2RUST-private-repo` has the commit `install: complete` (full 1.14d install plus `extracted/`), the window-free runs of `docs/LOCAL-RUN.md` Batches 1–2 (`data-tool tables` / `links`, `mpq-tool check` / `formats`, every `#[ignore]` game-file test) no longer need the developer PC. A cloud session runs `sh tools/realdata-gate.sh` (fetches and assembles the install into `$HOME/game` by itself; `--no-client` skips the Bevy crate; `--help` lists options). Look for: exit 0 and the summary line `ignored tests: N passed, 0 failed`; exit 2 means the install is not complete yet (nothing ran). Record the summary in Done and remove the matching `C<n>` entries as their rows pass. Still local: GPU and window runs (Batches 3–6), recordings, and the two `D2_TABLES_DUMP` tests. Inventory: `docs/handoff/realdata-tests.tsv`; reading guide: `docs/handoff/q-realdata-gate.md`. **First run done 2026-10-08** (`docs/handoff/q-realdata-run.md`): 213 passed, 11 failed (4 root causes, rows in the build queue); cloud prerequisites there (nextest, the apt line of `tools/cloud-setup.sh`).

### Done (kept for the record)

Done 2026-10-08 (cloud, branch claude/q-realdata-run from 40e9e1e; the
first real-data gate run, `docs/handoff/q-realdata-run.md`). Install:
private repo `install/` at `5174f0cb` assembled (0 mismatches), equal to
`traces/reference-install.toml`. Command: `D2_GAME_DIR=$HOME/game sh
tools/realdata-gate.sh` (release). LOCAL-RUN Batch 1 rows 1.1–1.4, 1.5,
1.6, 1.8, 1.12 and Batch 2 rows 2.1–2.16 ran inside it.
- Tool checks: `data-tool tables` 73 runtime tables, 72 identical, 1
  explained, 0 mismatched, code buffers 4/4; `links` 0 broken; `mpq-tool
  check` all blocks decode; `formats` 0 errors (cof 3,606 / 3,605 +
  `amblxbow.cof`, dc6 1,653, dt1 256 / 250, ds1 2,372). PASS.
- Ignored tests, run 1 (`106f4ad`): 208 passed, 29 failed; run 2
  (`efffaf0`, after the fixes): **213 passed, 11 failed**, 15 skipped (6
  GPU, 2 dump, 5 recording, 1 `D2_SAVE`, 1 repro). Per crate run 2:
  d2-formats 29/29, d2-data 37/37, d2-sim 110/110 (`game_core` 12,
  `game_drlg_tables` 12, `game_inventory_path` 6, `game_items` 8,
  `game_monsters` 19, `game_skills` 16, `game_treasure` 16, `game_world`
  10), d2-server 9/17, conformance 2/2, d2s-tool 1/1 (no saves), scenario-run
  1/1, seed-finder 1/1, d2-client 23/26.
- The 11 failures: Cold Plains 97 vs 98 rooms (`world_data` dispatcher
  test and all 7 `game_wired_host`, `q-fix-cold-plains-rooms`), S→C 0x23
  fatal 0x668 (`q-fix-set-skill-fatal`), `menu\horadric` frame 1 offset
  (`q-fix-panel-horadric-offsets`), and new: Game.exe's controls table
  holds command 1's slot-0 entry first (`q-fix-real-controls-default-order`).
- Fixed in the session: d2s token reference slots (`ktr` 45, real table),
  the item sweep's two spec-stated cases, the wired host's walk back, the
  sweep / lvlprest counts the specs already state. Claims unlocked:
  `dt1.md` §file-header-276-bytes, §tile-header-96-bytes-each-consecutive,
  §block-header-20-bytes-each-at-the-tile-s-block-headers-offset,
  §block-pixels; `ds1.md` §rules; `generation.md` §3 r1.
- GPU tests on llvmpipe (software, not the real-GPU claim): 6/6 pass.
  `play --frames 600` on the install (Xvfb, llvmpipe): data line `137
  levels, 573 objects, waypoint object class 119; 2043 DS1, 34 lvlsub DS1,
  241 DT1`, then the C40 render panic (draw item 653), exit 101: C40
  reproduces in the cloud.
- `py tools/coverage.py --check`: 13,742 claims, 0 errors; `--summary`:
  rules 11,164 (+625 exempt), unit 9,755 (87.4%), game-file 421 (3.8%),
  trace 80 (0.7%), **verified 501 (4.5%)**, any tier 9,863 (88.3%).

Done 2026-10-08 round 2 (local PC 1, branch claude/local-pc1-s8 from 4660bf4;
Lane B worker, Windows 10, `D2_GAME_DIR` = `d2rs\game`; release builds
except the d2-sim lib test, which needs debug: release rustc dies with
STATUS_ACCESS_VIOLATION on `d2-sim` lib tests, with or without sccache).
Section C entries C26, C41, C65, C67, C68, C72, C73, C76, C81, C87, C88 and
C90 removed; the others run keep a `Result 2026-10-08 round 2` line. GPU
**Intel(R) HD Graphics 630, Vulkan, IntegratedGpu, driver "?"**.
- C65 `game_drlg_tables act3_act5`: 2 pass (`act3_act5_table_values`,
  `act3_act5_placement_on_live_tables`). C87 `game_monsters
  ai_index_of_every_row`: pass. C88 `game_sweep cof_every_live_file_parses
  string_tables_every_key_resolves`: both pass (COF 3,605 parse, only
  `amblxbow.cof` fails (72 bytes), 3 padded 42-byte files; tbl 29 copies of
  20 paths in 10 languages, 63,167 used entries, 4,236 resolve to an earlier
  duplicate, 16,786 non-ASCII, 0 raw FF, 130 `C3 BF`). C72 `wav_game` 2 pass
  (`spec_files`, `every_sounds_txt_file`). C73 `sound_table::tests::game::
  live_sound_table` pass. C76 `app_original_ui` 1 pass. C95 / C90
  `hirelings::tests::game` 3 pass (debug).
- C67 (script over `Game.exe`, no run): `specs/render/env-periods.tsv` 18
  rows equal `0x007443F0` / `0x00744438` / `0x00744480` (12-byte entries
  `i32 start, i32 type, u32 0x00BBGGRR`), and `wall-light-points.tsv` 108
  rows equal `0x0072A9E8` / `0x0072ABC8` (entry = 8 bytes `i32 dx, i32 dy` at
  `table + 8 * (6 * direction + point)`, directions 0..9, 60 entries per
  table): 0 differences.
- C68: `RandTransforms.dat` 7,680 bytes, `GreenBlood.dat` 256, `palshift.dat`
  (e.g. `monsters\23\COF`) 2,048: as expected.
- C26 / C41 / LOCAL-RUN 4.3: `app_frame_loop` `frame_loop_runs_on_the_users_levels`
  pass; `app_single_player` 5 tests pass:
  `live_tables_give_a_waypoint_object`, `client_skill_rows_from_the_install`
  (357 skills rows, 29 with a passive state),
  `live_game_creation_installs_the_drops_and_the_hireling_tables`,
  `live_data_generates_the_levels_from_the_users_files` (act 0 level 1: 35
  rooms, rect (944, 832, 56, 40); act 0 level 3: 96 rooms, rect (1016, 920,
  80, 80); act 1 level 40: 49 rooms, rect (1000, 1000, 56, 56)) and
  `a_save_from_the_command_line_joins` (needs `D2_SAVE`; with `werwer.d2s`
  it passes, the load log lists five `Unapplied` steps: header client
  fields, quests, npc fields, item indices, quest entry).
- C81 `the_recorded_join_on_the_install` pass.
- LOCAL-RUN 4.1 (C24) `play --synthetic --frames 1500`: ran to the end,
  `gpu: true`, 628 server ticks, 624 node frames, then exit 101 (see C24).
  4.2 (C40 / C53): FAIL, render panic at once (see C40).
- Q-note local checks with no player (branches merged into staging; each
  note got a "Local check done" line): `q-menu-*` (`front_end` 12,
  `front_end_char_select` 14, `front_end_cinematics` 6, `front_end_controls`
  14, `front_end_create` 10, `front_end_credits` 2, `front_end_difficulty`
  2, `front_end_main_menu` 6), a d2-client lib run of 21 tests (credits,
  game_messages, hud_tips, stash_gold_max, inv_items_socket, weapons),
  `app_necro` 4, `app_server_core` 5, d2-sim lib (debug) 40 tests (equip,
  vitals_sync, socket, unit_update): all pass. The play parts of those notes
  need a player.
- Claims: `tools/d2s-tool/tests/real_saves.rs` `real_saves_round_trip`
  (passed in round 1 and again now: `charactertest.d2s` 1,012 bytes and
  `werwer.d2s` 1,289 bytes, OK) turned from `Claim once` into `// Covers:`
  (`d2s.md` s1 r1, s3 r1, s2.2 r2, s7.1 r5, s8.2 r2). No other `Claim once`
  / `Intended claim` line belongs to a passing test (the rest say "none" or
  sit on tests that fail). `py tools/coverage.py --check`: 13,430 claims, 0
  errors; `--summary`: rules 10,919 (+625 exempt), unit 9,674 (88.6%),
  game-file 419 (3.8%), trace 80 (0.7%), **verified 499 (4.6%)**, any tier
  9,782 (89.6%) (the rule count moved with spec sessions: 10,583 -> 10,919).
- **Findings** (no expected value changed; rows for the coordinator):
  (1) C40: `play` on live data panics in the render system, `draw item 653:
  drawn area Rect { x: 560, y: 296, width: 32, height: 32 } leaves the
  gradient block Rect { x: 560, y: 296, width: 32, height: 15 }`
  (`crates/d2-client/src/scene/mod.rs:141`; 3 of 3 runs; row
  `q-fix-play-gradient-block`). (2) C24: exit 101 after the last frame,
  `play.rs:521` `resource_mut::<WorldViewState>()` after `app.run()` (row
  `q-fix-play-exit-resource`). (3) C86 / C24: S->C 0x23 refused with fatal
  0x668 (row `q-fix-set-skill-fatal`). (4) C71: `menu\horadric` frame 1
  offset (-205, 17), the spec says 0 (row `q-fix-panel-horadric-offsets`).
  (5) C99: `ktr` token position 51, the test and spec say 45 (row
  `q-fix-appearance-token-ktr`). Known and unchanged: C89 / C92 Cold Plains,
  C46 (1) counts. LOCAL-RUN 4.1 expectation "node frames = frames - 1" is
  stale: node frames = server ticks - 3 or - 4.
- Not run: C2-C6, C10-C14, C18-C20 (tests not written), C49, C50, C52, C60,
  C62, C69, C70 (player), C74, C75, C78-C80, C83-C85, C91, C94, C96, C97
  (C59 fails the same way); LOCAL-RUN 2.17, 3.15.

Done 2026-10-08 (local PC 1, branch claude/local-pc1-s8 from 31291c8e;
Lane B worker, Windows 10, `D2_GAME_DIR` = `d2rs\game`; entries C21, C22,
C27-C33, C36-C38, C42-C45, C47, C48, C51, C54, C57, C61 and C66 part 1
removed below; C23, C34, C35, C39, C46, C55, C56, C59 keep a `Result
2026-10-08` line). Builds: `--release` for the d2-sim, d2-server, d2-formats
sweep and every d2-client / GPU row (disk), debug for the d2-data /
d2-formats `--ignored` run, the first d2-sim rows and the conformance rows.
Other workers edited `specs/` in the same tree meanwhile (the
`coverage.py` denominators moved with them).
- Section 0.1: `hash-manifest` of the install vs `traces/reference-install.toml`:
  `git diff --no-index` empty, 19 entries identical.
- Batch 1: `data-tool tables` 73 runtime tables, 72 identical, 1 explained,
  0 mismatched, code buffers 4/4 identical (1.1 pass); `links` 72,175 valid,
  84,277 misses (-1), 0 broken, 0 unchecked fields (1.2); `mpq-tool check`
  all blocks decoded (1.3); `mpq-tool formats` exit 0, 0 errors; the counts
  differ from the LOCAL-RUN 1.4 expectation, which is the pre-fix
  case-sensitive count (section 8 "counted a file twice"): cof 3,606 / 3,605
  parsed + `amblxbow.cof`, dc6 **1,653**, dt1 **256 files / 250 parsed, the 6
  gaps are the `KNOWN_UNUSED` ones**, dcc 21,717, ds1 **2,372**, tbl 29, pl2
  17, wav 2,663 (1.4 pass on the fixed counts). 1.5 `cargo test -p d2-data -p
  d2-formats -- --ignored` (debug; cargo stops at the first failing binary,
  so the four `d2-formats` binaries after `game_sweep` were run separately):
  **64 pass, 2 fail** (the two count assertions of `game_sweep`, findings
  below; every file decodes). 1.6 `patch_game` 5/5 incl. G1-G8; 1.7 `patch
  check` exit 0, one N01 note, digest `66010ecd...97a625` (pass); 1.8
  `fixups_on_live_set` pass; 1.9 `dump-compare traces/raw/20261006-021210-tables`:
  70 identical, 0 differ, `itemtypes_equiv` and `montype_equiv` identical;
  1.10 `check_stats.py --files`: pass (359 stats, 84 ops, 42 op targets);
  1.11 `path_tables.py`: ok, 582 rows equal `Game.exe` (18 tables); 1.12 C21
  `ds1_layer_limits_and_truncated_trees_groups` pass; 1.13 C22 `automap.bin`
  from `d2exp.mpq` (3,286 records of 44 bytes): no LevelName (bytes 0..16) or
  TileName (16..24) that is empty or starts with `0` (a ten-line Python scan
  of the `mpq-tool extract`ed `data\global\excel\automap.bin`).
- Batch 2: 2.1 `d2-sim -- --ignored` (debug): the 11 lib ignored tests pass
  (the five of C1 included), `game_core` 12 pass, then `game_drlg_tables` 11
  pass / 1 fail (cargo stopped there; the other d2-sim game binaries were run
  in 2.5-2.10). 2.2 C36 `real_grid_belt_and_type_tables` pass. 2.3 C38
  `expfield_live` pass. 2.4 C34 `game_world` **10 passed**,
  `game_drlg_tables` **12 tests: 11 passed, 1 failed** (`lvlprest_measurements`:
  rows naming a file beyond `Files`: 80, test and spec say 82). 2.5 C35
  (release) `game_treasure` **16 passed**, `game_items` **7 passed, 1 failed**
  (`sweep_create_every_item_every_quality`: 560 failures = item 556 `ear`
  "ear without a player" 160, 530 `isc` and 519 `ibk` "affix 1 does not fit"
  160 each, 645 `0sc` "affix 3 does not fit" 80; 10 crafted requests end in
  the affixes.md Edge case 3 crash and are counted, not failed). 2.6 C37
  `game_core` **12 passed** (3,558 AnimData records scheduled, 0 with a speed
  past i16; 357 skills, 78,804 values evaluated; 734 monstats rows, 673 with
  `DamageRegen`). 2.7 `d2-server game_world_data` **3 passed**: 39 Act I
  levels generate, 2,043 distinct lvlprest DS1 names, frame 100 with 0
  messages queued. 2.8 C23 `world_data` **2 passed, 1 failed**
  (`outdoor_levels_generate_through_the_dispatcher`: Cold Plains 97 rooms =
  62 preset + 35 outdoor, recorded 98 = 61 + 37; the grid and substitution
  difference lists are non-empty, C92). 2.9 C51 `game_inventory_path` **6
  passed**: 659 items, 8 distinct sizes (1x1 ... 2x4), 0 zero-size, none
  larger than 10x4, grids (3,4) (6,4) (6,8) (10,4) (10,10), 15 items equip at
  body location 8, 27 beltable items (27 of them 1x1); `path_tables.py`
  agrees. 2.10 C45 `game_monsters` **19 passed**, `game_skills` **16
  passed**; evilhut is **monstats data row 529** (0-based; the only row with
  `sparsePopulate` 40; the spec text says 528). 2.11 C46.1 `game_sweep`
  (release, 64 s): **9 passed, 2 failed** (`ds1_every_file_parses`: 2,372
  files, v3 1, v8 6, v12 14, v13 36, v15 13, v16 229, v17 147, v18 1,926, vs
  2,456 / 1,997 expected; `dt1_every_live_file_decodes`: 250 files, 15,873
  tiles, blocks {0x0001 226,996; 0x1001 108,905; 0x2005 15,712}, vs 110,259
  for 0x1001). Also printed: COF 3,605 parse, failed only `amblxbow.cof` (72
  bytes), 3 padded 42-byte files; DC6 1,653 files, 26,317 frames, 140 flipped;
  DCC 21,717 files, 271,176 directions, 3,305,132 frames; AnimData 3,558
  records, 3,529 with a `.cof` (3,500 distinct names, 9 non-matching copies
  skipped); 19 `.dat` palettes, 17 `.pl2`; 14 font tables. 2.12 C46.2
  `game_assets` (release) **5 passed**: 32,502 listed names read back
  identical, **0 refused**; 16 `.pl2`, 3,512 `.cof` (failed only
  `amblxbow.cof`), 36 `.tbl` (14 font, 20 strings); the two text `.tbl` files
  `data\local\FONT\LATIN\DEFAULT.TBL` and `FONTER.TBL` end in
  `Failed(... Format(Truncated { format: "tbl", offset: 8921 / 2303, needed:
  2 }))` (GA1; the test passes). 2.13 C59 `game_wired_host`: **0 passed, 7
  failed** (run twice, identical text): every class arrives in Blood Moor at
  (4790, 5656), frame 1007, then `town waypoint: not reached after 4 legs;
  player at (4798, 5634) in level Some(2)` (`crates/test-fixtures/src/host.rs:529`),
  so no `digest` line is printed. 2.14 `all_live_cofs_give_slot_orders` pass
  (3,511 names + `amblxbow.cof`, 242,300 frames, 0 failures). 2.15
  `all_live_frame_sets_build_and_pack` pass (23,595 files, 6 parse errors,
  288,702 frame sets, 3,345,171 frames, largest 96x960 `worldstone.dt1` tile
  50). 2.16 C61: `wav_game` 2 pass (incl. `every_sounds_txt_file`),
  `mpq_game` 4 pass, `mpq-tool formats` exit 0. 2.17 not run (optional). 2.18
  C66 part 1 `real_saves_round_trip` pass; `d2s-tool check` OK for
  `charactertest.d2s` (1,012 bytes) and `werwer.d2s` (1,289 bytes), read in
  section order, rewritten byte for byte. `charactertest.d2s` raw bytes:
  +0x10..+0x37 `00000000 6368617261637465727465737400 0000 20000000 04101e01
  00000000 a130c46a ffffffff`, +0x88..+0xA7 `ffffffffff19ff4f` then `ff` x24,
  `67 66` ("gf") at 0x2FD, `JM` at 844, `jf` at 1007, `kf` at 1009.
- Batch 3, GPU **Intel(R) HD Graphics 630 (Vulkan, IntegratedGpu, driver not
  reported: `driver ?`)** for every row: 3.1 `gpu_compare` **21 of 21** cases
  `0 differing bytes` (the example has 21 cases now, LOCAL-RUN said 18); 3.2
  `--perturb 7`: every case `7 differing ... 7 perturbed`, `Error: 21 of 21
  cases differ`, exit 1; 3.3 `gpu_compositor::tests::gpu` 2 pass; 3.4
  `verify::tests::gpu_half` pass (10 synthetic cases, 0 differing); 3.5
  `scene_tests::gpu_half` pass (`GPU indices: 0 of 480000 bytes differ`,
  `GPU: 0 of 480000 pixels differ`, 3 frames match); 3.6 `gpu_map` pass (sides
  32 / 64 / 1024, 0 of 7,000 differ); 3.7 `app_frame_loop` 6 pass, 1 ignored
  (`frame_loop_runs_on_the_users_levels`, C41, Batch 4), `adapter:
  Intel(R) HD Graphics 630 (Vulkan, ?)`; 3.8 `gpu_compositor_on_real_frames`
  pass (`GPU indices: 0 of 480000 bytes differ; GPU: 0 of 480000 pixels
  differ`; perturb 7 -> RGBA 7 of 480000); 3.9 `verify --case map`: 2,697 draw
  items, view 7840x4112 at -3200,-192 (as the last run), 40 chunks, `CPU
  binned: 0 of 32,238,080`, `GPU indices: 0`, `GPU: 0 of 32,238,080 pixels`,
  `PASS map (map)`, exit 0 (so no `palette[0]` / VM1 note); 3.10 `--perturb
  7`: 7 / 7 / 7, `FAIL map (map): CPU and GPU halves`, exit 1; 3.11 `verify`:
  `summary: 11 pass, 0 fail, 0 error, 0 GPU not wired, 0 no adapter`, exit 0,
  one `GPU compositor: adapter:` line, nothing wrong with one device; 3.12
  `verify --perturb 7`: `0 pass, 11 fail`, map 7 of 32,238,080, exit 1; 3.13
  `--perturb 5`: 5 on each, exit 1; 3.14 `verify --ds1 ...townN1.ds1`: map
  lines as 3.9, `PASS map (map)`, exit 0. 3.15 (PL2 palette, C49) not run: no
  command exists.
- Batch 5 (debug): 5.1 `tick_replay` 7 pass; 5.2 `recordings-needed`:
  `units-anim` present (4 files), `stats-lists` MISSING, `packets` present
  (2), **`movement-walk` present (2 `*-packets.jsonl` files)**,
  `movement-path-state` BLOCKED, `placement-players` present (2),
  `placement-monsters-items` BLOCKED; 5.3 `packets_replay` FAIL (expected):
  `20261006-015956-packets.jsonl: record 5: s2c: recorded message
  0100040010000100 to client 0 (size 8) not queued by the server`; 5.4
  `placement_replay` FAIL `PLACEMENT NOT WIRED`: `...015956`
  `PlacementReadStats { players: 2, with_room: 0, monster_assigns: 144,
  item_messages: 114 }`, seq 178 frame 1 (4863, 5653) and seq 311452;
  `...022633` `{ players: 4, with_room: 0, monster_assigns: 121,
  item_messages: 15 }`, seq 154 (4673, 4548), seq 9751 frame 133 (4893,
  4993), seq 94552 frame 1292 (4698, 4528), seq 127643; every `room None`; 5.5
  `movement_replay` FAIL `MOVER NOT WIRED`: `...015956` `MoveReadStats {
  requests: 385, ticks: 4239, seeds: 1, sent: {13: 1, 21: 1, 150: 122},
  other_inputs: 109 }`, `...022633` `{ requests: 130, ticks: 4061, seeds: 1,
  sent: {13: 5, 21: 3, 150: 32}, other_inputs: 42 }` (385 = 121 + 47 + 215 +
  2 and 130 as `pathing.md` R4 / R5; no 0x0F / 0x10 in `sent`).
- Claims: 31 `Claim once the first local run passes` / `Intended claim` lines
  turned into `// Covers:` (same ids) for passing tests in `game_assets.rs`
  (5), `game_sweep.rs` (8: dc6, dcc, cof, palettes, font, tbl, both
  animdata), `game_world_data.rs` (1), `game_drlg_tables.rs` (1: `maze.md` s1
  r1; the `s3.3` id was dropped because the spec has no such rule now),
  `game_inventory_path.rs` (4), `game_items.rs` (2), `game_treasure.rs`
  (10); `tbl.md` `live-tables-1-14d` was dropped from its claim (that id is
  exempt). Left as intended claims because the test fails:
  `dt1_every_live_file_decodes`, `ds1_every_file_parses`,
  `sweep_create_every_item_every_quality`. `py tools/coverage.py --check`: 13,398
  claims, 0 errors; `--summary`: rules 10,583 (+625 exempt), unit 9,659
  (91.3%), game-file 416 (3.9%), trace 80 (0.8%), **verified 496 (4.7%)**,
  any tier 9,767 (92.3%) (before this run: game 384, verified 464 of 10,529;
  the denominator moved with the spec sessions in between).
- **Findings** (expected values not changed; owner in brackets):
  (1) `game_sweep` count assertions: the observed 2,372 DS1 (v18 1,926) and
  108,905 0x1001 DT1 blocks agree with the case-insensitive `mpq-tool
  formats` (ds1 2,372, dt1 256 files / 250 parsed); `specs/formats/ds1.md`
  Status (2,456 / 1,997) and `specs/formats/dt1.md` (110,259) say otherwise
  [tests `game_sweep.rs` `ds1_every_file_parses`, `dt1_every_live_file_decodes`;
  also `docs/PLAN.md` line 175 and `specs/formats/native-assets.md` (2,456)].
  (2) `lvlprest_measurements`: 80 vs 82 rows naming a file beyond `Files`
  [`specs/drlg/preset.md` "Files beyond `Files`", `game_drlg_tables.rs:234`].
  (3) `sweep_create_every_item_every_quality` 560 failures on items 519 `ibk`,
  530 `isc`, 556 `ear`, 645 `0sc` [`specs/items/generation.md` s3 /
  `affixes.md`, `game_items.rs:411`, `check_item`]. (4) Cold Plains 97 rooms
  (62 preset + 35 outdoor) vs the recorded 98 (61 + 37), grid and
  substitution lists non-empty [`specs/drlg/outdoor.md`,
  `world_data/tests/game.rs:206`, C92]. (5) `game_wired_host`: all seven
  classes stop at the first walk to the town waypoint
  [`crates/test-fixtures/src/host.rs:529`, wired-host walk]. (6) 5.4:
  `with_room` is 0 on both recordings [`conformance::placement` reader].

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
- `claude/fuzz-parsers` `a704a89`: wave E. Property tests at
  `PROPTEST_CASES=200000` (release): d2-formats lib 173 pass, `prop_more_formats`
  8 pass, no failure. New `fuzz/` cargo-fuzz crate (own workspace; nightly,
  libFuzzer + ASan on MSVC; `fuzz/README.md`: the ASan DLL dir must be on
  PATH; a panic hook writes the input because Windows panics abort without
  a crash file), feature `fuzz` on d2-formats, 18 targets (every
  d2-formats parser and decompressor, `.txt`, `.bin`, patch layer) 1 h each,
  ~1.59 G execs. One crash, fixed: DT1 RLE zero-length run past the block
  (`dt1::tests::regress_rle_empty_run_past_block`, `dt1.md` edge case).
  Thin coverage: `mpq_archive` 2.5 M, `patch_layer` 2.2 M, `txt` 5.5 M.
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
  97 vs 98 (C92 now prints the grid and substitution differences).

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

**Draw order 2 (`impl-draw-order-2`, 2026-10-06;
`docs/handoff/impl-draw-order-2.md`).** (1) `weather-0001`: Rogue
Encampment in rain, record two frames with the player seed per frame and
the three weather pools (`capture.md` §3.4 + pools); compare pass 4 / 9
pixels and count the player-seed draws per frame against the drawn water
floors (`draw-order-2.md` §11.5: one `roll_range(0, 1000)` each). Also
note whether splashes appear while int(intensity) is 0 (question W5).
(2) Walk past the walls of a `Logicals` preset room (Crypt / Mausoleum,
levels 18, 19) with the frame lists recorded: the group-mode fade
(`draw-order.md` §8, OQ6) on pixels. (3) Arcane Sanctuary and Arreat
Summit captures with `[0x00712C4C]` / `[0x00712C50]` recorded
(`draw-order-2.md` §12).

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

**Ninth-fold additions to A** (2026-10-07, the night's 22 notes; PC 2 runs
them; each settles the §7 ninth-set questions named. C74, C78, C80, C83,
C84, C85 also need the game running: they are numbered in C but batch with
these).
- *S9-A1 PC 1's recording list for PC 2* (`record_packets.py` +
  `record_rng.py`, `record_tick.py` where a per-tick list is asked,
  `record_frames.py` for (1)–(3)): (1) capture `order-0003` beside a
  visible river-bank cell, run-2 seed `TownE1` tile (950, 933) (`townN1`
  (44, 32), (51, 32)): `draw-order.md` OQ7; (2) `weather-0001` (Draw order
  2 (1) above: W5); (3) levels 74 / 120, Arcane Sanctuary and Arreat
  Summit with `[0x00712C4C]` / `[0x00712C50]` recorded (Draw order 2 (3),
  pass 1); (4) Act III entry: DRLG-seed draws `0x00677966`…`0x006784D9`,
  levels 76..78 +0x1C..+0x28, +0x1B8, +0x1BC, then builds of 76..78, 111,
  112, 117 (`outdoor-act3-act5.md` OQ1, OQ3); (5) a kill with an item
  drop (the drop position of S8-A3 and the treasure walk); (6) a hireling
  game: hire, level-up, death, resurrect, give / take, a teleport
  (`hirelings.md` OQ9: HL2, HL3, HL7, HL8; C80); (7) an act change (the
  classic hireling act change `hirelings.md` §6 r3–4, the act palette
  switch `model.md` §11 r4 vs `msg-units.md` §3 r4.4); (8) Might in a
  party (aura bodies on party members, `bodies.md` §7–§8); (9) Kick /
  Bash on a monster (srvst 2, srvdo 2: kick damage and hit,
  `bodies-2.md` §2); (10) a Druid summon and (11) a Clay Golem (summon
  class / spawn `bodies.md` §6, golem stats, the pet lists `sim/pets.md`,
  the 0x7A order of §7 ninth set I-1); (12) a Countess kill with a listed
  chest (one monster 326 mode 12 at the death position, one missile 332
  per chest: `quests-act1-rest.md` §4); (13) a Cain rescue (gibbet: mode
  1, slot 4 = `0x2002`, 0x28 at once, 17 frames later class 146 at gibbet
  + (3, 3) and `5d 04 00 06 0000`; also C78).
- *S9-A2 Act I remainders* (`impl-quests-act1-rest` queue 3–4): entering
  Catacombs 1 after Andariel (state 4: no 0x5D, state kept, §8 item 6);
  the save header after Andariel on Normal (progression bits 8–12, §5).
- *S9-A3 Act II–V quest runs* (packets + RNG, one run per act): Act II
  (`quests-act2.md` OQ10: chat-end status, kill timers, Tyrael's portal,
  Meshif); Act III (`quests-act3.md` OQ8: Alkor 564 with a second player
  in Act III and a third elsewhere, a Khalim chest, a council kill's 0x5D
  status byte, two orb hits, Mephisto's timer and Hellgate, Natalya, the
  Dark Wanderer's 7 / 8 minions); Act IV (`quests-act4.md` OQ13: Izual,
  Hellforge, seals and bosses, Diablo in classic with one uncredited
  player (OQ3) and expansion, the classic end frames (OQ2), the Harrogath
  portal); Act V (`quests-act5.md` OQ7: Shenk, the rescue portals, the
  rune, Anya's thaw / scroll / item, the Ancients' experience, Baal, the
  last portal).
- *S9-A4 Monster AI and skill / missile bodies* (`record_tick.py` +
  `record_rng.py`): Act II–V monsters per `ai-bodies-2.md`..`-5.md` OQ1;
  the skill bodies (`bodies.md`, `bodies-2.md` OQ1); the missile list of
  `impl-missile-bodies-2` §5.3 (Cairn Stones portal, Baal's taunt, Royal
  Strike, Plague Javelin, Blade Fury, a lightning trailing javelin
  explosion, Lightning Fury, Bone Wall, Battle Cry, Fist of the Heavens,
  the panther potions).
- *S9-A5 Objects* (`objects.md` OQ1–2, `impl-objects` §3.4): packets + RNG
  of a chest, shrine, door, well and portal operate, and an object
  allocation with `Sync` = 0.
- *S9-A6 Room population* (`impl-room-population` §4 items 3–4): dump the
  DRLG room +0x64 lists of a crypt level (`Logicals` 1) and an outdoor
  level after activation and compare with `drlg::logic` on the same tiles
  (`levels.md` OQ7); the first population of a Blood Moor room through
  `WorldSim` against the spawn RNG recording (`population.md`
  "Recordings", now runnable).
- *S9-A7 Panel captures* (`impl-ui-panels` §5, `wire-client-staging` §5):
  `placement-0001` (inventory frames), `ui-0001`, `ui-0002` (`ui/panels.md`
  §Test vectors); the act palette is presented since `impl-client-drlg`.

### B. Ghidra / spec edits only (no game run, no player)

**Ninth fold (2026-10-07):** every RE / spec question of the night's 22
notes is in §7 "Ninth set", part **PC 1**, grouped by spec file (none is
repeated here). Spec-table edits it names: `quests.tsv` `spec` column for
the Act III rows (QC-7) and row 40 with its 15 `quest-messages.tsv` rows
(QE-7), the 0x7A field names (I-1), `scenario.md` §6 vs
`specs/tools/scenario-masks.tsv` (CS-4–CS-7), `tbl.md` OQ3's stale "33"
(CS-8).

**`impl-draw-order-2` questions** (owner `render/draw-order-2.md`, also
`unit-composite.md`): W1–W7 (weather state, snow spawn draws, snow line
table, colour ramps, the intensity scale vs observed splashes, flash /
particle order, initial values), §12 seed width / star tick / palette /
summit build order, §14 extents and edge-floor keys, §16 missing grid
cell and r1 stop cell; listed in `docs/handoff/impl-draw-order-2.md`
"Questions for the spec owner".

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
    test written, home per §7 I1): `monsters/ai-bodies.md` §9.1 (values from the
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
    **Result 2026-10-08** (PC 1): `act1_placement_matches_the_recorded_vector` and `den_of_evil_matches_the_maze_vector` pass; `outdoor_levels_generate_through_the_dispatcher` fails: Cold Plains 97 rooms (62 preset + 35 outdoor) vs 98 (61 + 37), grid / substitution lists non-empty (C92).
24. Window on a real GPU, synthetic tables (`p6-window` check 1):
    `cargo run -p d2-client --release -- play --synthetic --frames 1500`.
    Expect a black 800×600 view in the window; a log line every 250 frames
    with `server ticks` ≈ 25 per second, `gpu: true`, `node frames` = frames
    − 1 (or − 2), no error; exit 0. Record the adapter line
    (`bevy_render::renderer: AdapterInfo { … }`).
    **Result 2026-10-08 round 2** (PC 1, release): ran to frame 1500 on Intel HD Graphics 630 / Vulkan, `gpu: true`, 628 server ticks, node frames = ticks - 4 (624; not frames - 1), audio load/engine errors 0, then exit **101**: `play.rs:521` `resource_mut::<WorldViewState>()` after `app.run()` panics (resource missing). Also `S->C 0x23 refused: fatal assert 0x668` twice. Findings `q-fix-play-exit-resource`, `q-fix-set-skill-fatal`.
25. Window with the user's tables (`p6-window` check 2):
    `D2_GAME_DIR=<game> cargo run -p d2-client --release -- play --frames 1500`.
    Expect the line `play: game tables from D2_GAME_DIR (<n> levels, <m>
    objects, waypoint object class <k>)`, then as C24. Record n, m, k.
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
    **Result 2026-10-08** (PC 1): `game_world` 10 passed; `game_drlg_tables` is 12 tests now, 11 passed, `lvlprest_measurements` failed (80 rows name a file beyond `Files`, expected 82): finding for `preset.md`. Claims unlocked: all but that test's (it has none).
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
    **Result 2026-10-08** (PC 1, release): `game_treasure` 16 passed, claims unlocked; `game_items` 7 passed, 1 failed: `sweep_create_every_item_every_quality`, 560 failures on items 519 `ibk`, 530 `isc` ("affix 1 does not fit"), 556 `ear` ("ear without a player"), 645 `0sc` ("affix 3 does not fit"); its `generation.md` §3 r1 claim stays unconfirmed.

39. Packets replay baseline (`conformance-harness` §5 item 3): `cargo test -p
    conformance --test packets_replay -- --ignored --nocapture` on the
    existing `traces/raw/*-packets.jsonl`. **Expect FAIL** at the first
    message the original dispatched (the test replays on an empty `SimGame`:
    no session code builds the recorded game, Phase 5): record the `seq` and
    the message id as the baseline. It passes only when a session and a world
    reproduce the recorded game and every S→C sender exists (Blocked).
    **Baseline 2026-10-08** (PC 1): FAIL at `20261006-015956-packets.jsonl` record 5: `s2c: recorded message 0100040010000100 to client 0 (size 8) not queued by the server`.
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
    **Result 2026-10-08 round 2** (PC 1, release): data line `137 levels, 573 objects, waypoint object class 119; level files: 2043 DS1, 34 lvlsub DS1, 241 DT1`, then the render system panics at once (3 of 3 runs, exit 101): `draw item 653: drawn area Rect { x: 560, y: 296, width: 32, height: 32 } leaves the gradient block Rect { x: 560, y: 296, width: 32, height: 15 }` (`scene/mod.rs:141`). No 250-frame line is printed. Finding `q-fix-play-gradient-block`; C53, C77, C82, C93 (play part), C98 wait on it.
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
    **Result 2026-10-07** (local, `claude/local-2026-10-07` @ `59e4c88`,
    base `043d2be`; `docs/local/2026-10-07/RESULTS.md`): (1) **9 passed, 2
    failed**, both count assertions, no file failed to decode.
    `ds1_every_file_parses`: 2,372 files vs 2,456 expected (versions {3: 1,
    8: 6, 12: 14, 13: 36, 15: 13, 16: 229, 17: 147, 18: 1926}): the same 84
    missing as on 2026-10-06, so the enumeration fix (`patch_d2.mpq` names
    without a listfile) still misses DS1s. `dt1_every_live_file_decodes`:
    250 files; block formats {1: 226,996, 4097: 108,905, 8197: 15,712} vs
    4097: 110,259 expected (1,354 fewer RLE blocks; the other two match).
    Possibly the 250-vs-251 file question of `render/camera.md` OQ7 /
    REC-61. Findings for the owners: fix the enumeration or re-derive the
    count from `mpq-tool formats`, never just edit the numbers. (2), (3) not
    run. Same run: `d2-sim --test game_core` **12 passed**.
    **Follow-up (fix-sweep-counts, `6233461`):** the expected 2,456 DS1 and
    110,259 RLE blocks came from the old case-sensitive `mpq-tool`, which
    double-counted; the names now come from one source (`d2_formats::mpq::names`).
    Next local run: `cargo run -p mpq-tool -- formats` on the install, then
    the sweep rerun; update the expected counts only from that measurement
    (`docs/handoff/fix-sweep-counts.md`).
    **Result 2026-10-08** (PC 1): (1) 9 passed, 2 failed (`ds1_every_file_parses` 2,372 files, `dt1_every_live_file_decodes` 0x1001 blocks 108,905; spec numbers 2,456 / 110,259 are the pre-fix case-sensitive counts); (2) 5 passed, 0 refused; (3) pass on Intel HD Graphics 630. Only (1) remains: fix the specs' counts and the two assertions together.
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
    **Result 2026-10-08 round 2** (PC 1): not reached, same panic as C40.
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
    **Baseline 2026-10-08** (PC 1): FAIL `PLACEMENT NOT WIRED`; `...015956` players 2, with_room 0, monster_assigns 144, item_messages 114; `...022633` players 4, with_room 0, monster_assigns 121, item_messages 15; every room `None` (R3's room not read: reader finding).
56. Movement replay on the existing recordings, no player
    (`conformance-path-render` §5 item 2): `cargo test -p conformance --test
    movement_replay recorded_walks_replay_exactly -- --ignored
    --nocapture`. Expect per file `read MoveReadStats { requests, ticks,
    seeds, sent, other_inputs }` with requests = the 0x01–0x04 counts of
    `pathing.md` R4 (121 + 47 + 215 + 2 = 385, all accepted) and R5 (49 + 9 +
    72 = 130), no 0x0F / 0x10 in `sent` (R4), then FAIL `MOVER NOT WIRED`.
    Different request counts are a reader finding (the reader counts every
    `c2s`; R4 / R5 count dispatched ones: equal when nothing was dropped).
    **Baseline 2026-10-08** (PC 1): FAIL `MOVER NOT WIRED`; `...015956` requests 385, ticks 4239, sent {13: 1, 21: 1, 150: 122}, other_inputs 109; `...022633` requests 130, ticks 4061, sent {13: 5, 21: 3, 150: 32}, other_inputs 42.
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
    **Result 2026-10-08** (PC 1): 0 passed, 7 failed, identical in two runs: Blood Moor arrival (4790, 5656) frame 1007, then `town waypoint: not reached after 4 legs; player at (4798, 5634) in level Some(2)` (`test-fixtures/src/host.rs:529`).
60. `GameData::load` on the live set (`fixedset-game` §4; optional, after C59):
    with `D2_GAME_DIR` set, `GameData::load(bins, &archives)` with
    `ActCreation::Full` should reproduce the setup `game_wired_host.rs`
    assembles by hand (same level ids, same seeds, same digests). If it does,
    folding that file onto the constructor is the refactor of step 7e.
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
66. Character saves (`claude/impl-d2s`, note
    `docs/handoff/impl-d2s.md` §4; `docs/LOCAL-RUN.md` 2.18 and 6.7).
    (1, done 2026-10-08) `D2_SAVE_DIR=<Saved Games\Diablo II>` `cargo test --release -p
    d2s-tool --test real_saves -- --ignored --nocapture`: every 1.14d
    `.d2s` parses in `formats/d2s.md` §1 order and rewrites byte for
    byte; record header +0x10..+0x37, +0x88..+0xA7, stats at 0x2FD,
    `jf`/`kf` bytes (d2s OQ3). (2) The three `d2s-tool new` / `new-stub`
    characters of LOCAL-RUN 6.7 load in 1.14d; after the game re-saves
    them, `d2s-tool check` passes and `dump` of ours vs the game's shows
    no difference outside the save time. A difference in an item record
    is an `items/bitstream.md` §5 finding.
69. Environment per tick (`lighting.md` OQ7, `impl-lighting-blend`): record
    env `+0x0C` (and R, G, B `+0x18..+0x1A`) every client update over one
    in-game day and replay `rules::lighting::environment::Environment::update`
    from the same start; expect equal on every tick. Add the light-map
    SHA-256 (`LightMap::digest`, 18,432 bytes at `0x007B0E68`) and `q` to the
    capture key (§12 r3, OQ9).
70. Captures for the GDI and shadow rules (`impl-lighting-blend`): a weather
    line or Arcane Sanctuary star (static camera) compared with
    `rules::blend::gdi_line_pixels` — settles `blend-modes.md` §8 r1's
    |dx| = |dy| major axis (now `BlendError::LineMajorAxisTie`) and whether
    the line reaches (x1, y1); the player's shadow on a flat floor against
    `unit_shadow_position` (blend-modes OQ1).

71. Panel files (`impl-ui-panels` §4): `D2_GAME_DIR=<install> cargo test -p
    d2-client --test game_panels -- --ignored`. Expect both tests to pass:
    every quad set of `panel-layout.tsv` (`art0` rows, the seven
    `skltree_?_back`) has frames f … f + 3 of 256 × 256, 64 × 256, 256 × 176,
    64 × 176 with offsets 0; the frame counts / sizes of `ui/panels.md`
    §Constants (`buysellbtn` 23 from `d2exp`, `buyselltabs` 8 × 79 × 31, …).
    A different count is a finding for the spec (archive order,
    `client/assets.md`), not a reason to change the test.
    **Result 2026-10-08 round 2** (PC 1, release): `panel_quads_have_the_stated_sizes_and_zero_offsets` passes; `panel_files_frame_counts_and_sizes` FAILS at `game_panels.rs:99`: `menu\horadric` frame 1 offset (-205, 17) vs the expected (0, 0) (all earlier files in the list pass; the assert stops at the first, other `horadric` frames may follow). Finding `q-fix-panel-horadric-offsets` (`ui/panels.md` s7 r2 / Constants).
74. Sound request log (player; `triggers.md` / `environment.md` "Checks",
    `record_sound.py` hooks `0x004B9A00`, `0x004B9B50`, `0x004E40A0`,
    `0x004DCAA0`, `0x004DCD40`, `0x004E42E0`, `0x004B99A0`, `0x004CC270`):
    town walk, a fight, an NPC talk, item moves, town → wilderness → cave and
    back, a day change, Blood Raven's death. Expect the d2rs rule functions,
    fed the same inputs, to give the identical `(C, id, unit, delay, flags,
    offset)` sequence, volume sets and roll order; then the voice log
    `(tick, kind, file, vol, pan, looped)` (`client/audio.md` §A5).
75. Decoded samples (`wav.md` OQ 1–2, `client/audio.md` §B1): dump the
    DirectSound buffer after `0x515180` (and a `Stream`=1 sound such as
    `music\act1\crypt.wav`) for the C72 files; expect byte equality with
    our `data`.
77. `play` smoke with the UI and sound layer (`wire-client-staging`):
    `D2_GAME_DIR=<install> cargo run -p d2-client --release -- play
    --frames 500`, pressing I, C, T a few times. Expect: no error from the
    world view or audio frames (a `UI image file`, `sound world: …
    (pending …)` or `SetUIState` error is a finding); the panels toggle per
    the conflict table (I then T closes the inventory); the image is still
    black (the frame palette is `unspecified_palette` until the act
    palette is wired, `play.rs`), so judge by the log only. Then the
    capture cases `ui-0001` / `ui-0002` (`ui/panels.md` §Test vectors)
    once a palette is presented.
    **Result 2026-10-08 round 2** (PC 1): `play --frames 600` panics at once (C40); the key presses need a player anyway.
78. Quest objects on the wired host (`docs/handoff/wire-world-staging.md`
    §4; player, Act I, `record_packets.py` + `record_rng.py`): operate a
    Cairn stone, Cain's gibbet and the Forgotten Tower tome. Expect the
    S→C 0x0E mode messages at the same tick numbers and bytes as
    `world/tests/quest_objects.rs` derives (gibbet: mode 1 at the
    operate, event 7 → mode 3 at operate frame + 17).
79. Quest init order (`wire-world-staging.md` §3 item 1; `rng.md` §5.3
    draws): record the RNG while the town-Cain marker object (class 385,
    init 54) is created after Cain left Tristram. Expect Cain's spawn
    draw right after the marker's allocation; if other objects of the same
    room are allocated between them in 1.14d, the drained init order is
    exact, else the quest control must be lent into the action wiring.
80. Hireling teleport follow (`hirelings.md` §6 r1, `path-placement.md`
    §10 r6; packets): teleport with a living hireling; record where the
    hireling's warp messages fall against the player's room / 0x15
    messages (d2rs runs the follow when the handler returns).
82. Act palette in `play` (`impl-client-drlg`): `D2_GAME_DIR=<install>
    cargo run -p d2-client --release -- play --frames 500`. Expect: no
    error from `present_act_palette`; the frame palette is act 1's
    `pal.pl2` from the first frame (`composition.md` §4: act 0 at game
    start), so the window is no longer all zeros where index 0 differs
    from black. Since `impl-server-join` the in-process server sends the
    session join (0x59, 0x0B, 0x03, 0x07, 0x15, then the room switch's
    0x07s), so the client DRLG is built in this run: expect no 0x07 /
    0xAC / 0x15 refusal in the log (a `fatal assert 0x13C` / `0x168` or a
    `TODO(spec: model.md §9` rejection is a finding: see
    `impl-client-drlg` §3 Q1–Q2).
    **Result 2026-10-08 round 2** (PC 1): `play --frames 600` panics in the render system at once (C40), no palette line to judge.
83. Client room seeds vs the server's (`client/model.md` OQ 9, §12 r5):
    with `tools/trace-recorder`, after a join, read the client unit seed
    (`+0x20`) of the first monster 0xAC adds at a non-zero point and the
    client active room's seed (`+0x6C`) before and after; record the
    S→C stream. Expect the bridge (live `DrlgSource`, the recorded stream)
    to give the same `ClientUnit::seed` and room seed step (one step per
    creation at a point, `init_low(lo')`); a mismatch is either OQ 9
    (client rooms created by other draws, e.g. the §4.6 build timer) or
    object creation not stepping the seed (`impl-client-drlg` §3 Q3).
84. Server join order (`impl-server-join` §3 Q1; `sim/path-placement.md`
    §11, R2; `client/model.md` §11 r3): in `20261006-022633-packets.jsonl`
    frame 1, list every S→C id from 0x59 to 0x15 with its `caller`.
    `path-placement.md` §11 says game entry sends 0x07 (spawn room) and
    0x15 "at once" and the other 0x07s come from the first per-client
    update's room switch, which would give 0x07, 0x15, 0x07 × 9; R2 and
    `model.md` §11 r3 record 0x07 × 10 then 0x15. d2rs sends the §11 text
    order. Expect the callers to settle which: the 0x15's caller
    (`0x005394A0` at once, or the update pass `0x00580860` through flag-ex
    0x10000) and the nine 0x07s' (`0x0053A8E0`). Also list the ids
    between 0x0B and 0x03 and after 0x03 (0x53?) with their callers:
    each is a message d2rs does not send yet (`impl-server-join` §2).
85. Game +0x80 in 0x03 (`client/model.md` §11 r1, `objects.md` §2 r2):
    with `tools/trace-recorder`, at a single-player join read the result
    of `0x00546C60` and game +0x80, and the S→C 0x03 u32@8 (recorded
    0x9FE0D161 in `-022633`). Expect all three equal (d2rs sends the
    object control's `lo'`, `ObjectState::obj_seed`; the app's game has
    no object control and sends 0, a known wiring gap).
86. The session join on game files (`impl-server-join`):
    `D2_GAME_DIR=<install> cargo test -p d2-client --test app_client_drlg
    -- --ignored the_session_join_on_the_install --nocapture`. Expect a
    pass: no message refused, act 0 with town level 1, the local player
    in a level-1 room of the client DRLG at the server's spawn point;
    record the printed 0x07 count and positions (1 from game entry + the
    spawn room's adjacency array) and compare them with the ten 0x07 of
    the recorded join (C84: same count, and the first at the spawn room).
    A spawn search failure (`game entry failed`) is a finding for
    `drlg/levels.md` §10 / `path-placement.md` §11 on the live town.
    **Result 2026-10-08 round 2** (PC 1, release): `the_recorded_join_on_the_install` (C81) passes; `the_session_join_on_the_install` FAILS: `bridge.log().rejected` = three `Rejected { id: 35, error: Fatal(1640) }` (0x23 SetSkill, fatal 0x668 "skill outside the table", `bridge/msg/skills.rs` `set_skill` -> `skills::select`). The test binds only `levels` in `ClientTables` (no skill rows), so this is a test wiring gap or a skill the server sends outside the table; the synthetic `play` run shows the same refusal twice. Finding `q-fix-set-skill-fatal`.
89. Room population on the live files (`impl-room-population` §4 items
    1–2): `D2_GAME_DIR=<install> cargo test -p d2-server world_data --
    --ignored`, then `cargo test -p seed-finder --test game_seed_finder --
    --ignored`. Expect the C23 expectations unchanged and no panic or
    `WorldgenError` in the Den of Evil / Blood Moor rooms (their
    `Logicals` rows now run the grid build; an error names the room);
    seeds 1–8 build twice with the same result; record the
    room-population monsters the views now list.
    **Result 2026-10-08 round 2** (PC 1, release): `world_data` 2 pass, 1 fail = the known Cold Plains test (C23 / C92: grid cells (7, 1), (8, 1), (3, 2), (8, 4), (4, 5), (5, 5), (4, 7) differ); `seed-finder --test game_seed_finder` 1 pass (`den_of_evil_and_blood_moor_build_on_live_tables`).
91. `missiles.txt` row 338 `horadricstaff` (`impl-quests-act2` "Local run
    queue"; **test to write** once a missiles loader is reachable from the
    quest tests): expect Range 440, so the orifice timer period
    (440 − 75) / 20 = 18.
92. Cold Plains grid and border substitutions on the live files
    (`fix-drlg-answers` §2; `outdoor.md` Test vectors "Cold Plains
    grid", OQ 10): `D2_GAME_DIR=<install> cargo test -p d2-server
    world_data::tests::game::outdoor_levels_generate_through_the_dispatcher
    -- --ignored --nocapture`. The test now prints the live grid (grid 0 /
    grid 2 per cell) and the border-substitution hits (type, row, group,
    snapped cell, variant, level-seed lo' after the variant roll) and
    both difference lists, then asserts, in this order: every cell equal
    to the table; hits = (type 1, group 0, (3, 1), v 2, lo'
    1833932632), (2, 1, (6, 6), 1, 3559729267), (3, 8, (3, 6), 0,
    1651351014), (3, 11, (0, 5), 0, 2564466130) (seq 8447, 8644, 10204,
    10499 = level-seed draws 1550, 1747, 3307, 3602 from {4014346872,
    666}; "group" = index in the DS1's group list, read from the spec's
    "group 8", "group 11"); 98 rooms, 61 preset + 37 outdoor. Expect a
    pass. On a fail, record the printed grid and hits: the first wrong
    hit names the substitution (a wrong lo' with the right hit = the
    draws diverged before it; a wrong group / cell / variant with the
    right lo' = the test or replace of `outdoor-tilesub.md` §2.3 on live
    data), a grid difference with all hits right names a non-substitution
    step. The cloud session could not reproduce the third blank cell (the
    synthetic build gives 98, `cold_plains_tests`).

93. Client handlers on the install (`impl-client-msgs-3` §4): `D2_GAME_DIR=<install>
    cargo test -p d2-client --test app_single_player -- --ignored
    client_skill_rows`: expect a pass; the printed row count is the `skills`
    record count (`[0x00744304]` +0xBA0) and the passive count is > 0. Then
    **B seq 227 0x94 bytes** (recording `20261006-022633`): the 8-entry list of
    `update_item_skill_quantities` is synthetic (the spec gives only "ends
    `0300 01`"); replace it with the recorded bytes and expect the same
    result. Then `cargo run -p d2-client --release -- play --frames 600` with
    `D2_GAME_DIR`: expect no handler rejection for 0x2C / 0x53 / 0x5D / 0x63 /
    0x77 / 0x21–0x23 / 0x94 in the log other than the named pending ones.
    **Result 2026-10-08 round 2** (PC 1, release): `client_skill_rows_from_the_install` passes (357 skills rows, 29 with a passive state). The B seq 227 bytes replacement and the `play` part are not done (play panics, C40).
94. Kill on the wired host against the recording (`impl-monster-death` §5;
    **blocked** until a host with a real death start `0x005A6FF0`, the add
    messages and the trace converter (`intents-events.md` §6 r4) exist): a
    trace of a kill compared with `20261006-015956` frames 2724 / 2748 (0x69
    code 8 and code 9, 24 frames apart). Until then the pair is unverified.
95. Hirelings and saves after the load work (`impl-d2s-load-hirelings` Left):
    `D2_GAME_DIR=<install> cargo test -p d2-sim -- --ignored hirelings::tests::game`
    (`world/hirelings/tests/game.rs`: the unit columns of the Test-vector table,
    the recorded hire of Diane (Id 0, L 7, the 14 queued stats), the live
    `ExpRatio` column); then C66 (2) again with the new `d2s-tool new` (items
    without 0x2000, appearance 32 × 0xFF) and `set` (`save::resave`).
    **Result 2026-10-08 round 2** (PC 1, debug): `hirelings::tests::game` 3 pass (`unit_columns_of_the_test_vectors`, `recorded_hire_of_diane_queues_the_recorded_stats`, `live_exp_ratio_column`). Only the C66 (2) part is open. Note: `cargo test --release -p d2-sim --lib` dies with rustc STATUS_ACCESS_VIOLATION (with and without sccache); the debug build works.
96. Client room timer and room lists (`impl-client-drlg-2` §6): C83 now also
    covers the timed builds: memory reads of +0x98, +0x45C, +0x460 per client
    update against `ClientDrlg` (`rooms.md` OQ 15); `unit-order.md` OQ 5: one
    room's list (active room +0x74, unit +0xE8) before and after a drawn frame
    against `RoomUnits` after `set_room_order`.
97. `game_wired_host` after the Act I fixture (`fixedset-act1` §4):
    `D2_GAME_DIR=<install> cargo test -p d2-server --test game_wired_host --
    --ignored --nocapture` (C59 again): expect it to reach Blood Moor; check
    the 0x3C, 0x0D and 0x16 stub assumptions (the skill and item handlers
    return `None` without providers, so those steps should still reach the
    stub).
98. Live `play` and the buddy reds on the merged head (`local2-2026-10-06` §4,
    §3 item 6): `d2-client play --frames 1500` on live data panicked the
    server thread at `d2-sim/src/drlg/room.rs:144` ("live DRLG room") after
    ~104 ticks on `0472619`; rerun, and file the first wrong room if it still
    fails. Rerun the batch 2 failing rows (C36 `real_grid_belt_and_type_tables`;
    C34 `game_world`, `game_drlg_tables`; C35 `sweep_drop_quality_every_item`,
    `sweep_create_every_item_every_quality`; C37 `every_lvlprest_ds1_parses`
    (object ids ≥ 573); C45 `game_monsters`; C46.1 `game_sweep`; C46.2
    `cpu_compositor_on_real_frames`; `game_wired_host`). Confirm the
    `mpq-tool formats` counts after the case fix (DC6 1,653, DT1 256) and
    re-derive `game_sweep`'s expected values (buddy note G1).
    **Result 2026-10-07** (same local run, base `043d2be`): still panics,
    exit 101 about 5 s in (frame 250, server tick 104), seed 1234; data
    line `137 levels, 573 objects, waypoint object class 119; level files:
    2043 DS1, 34 lvlsub DS1, 241 DT1`. Panic `d2-sim/src/drlg/room.rs:152`
    "live DRLG room" via `free_inactive_rooms` → `Drlg::free_level_rooms` →
    `Outdoor::reset_level` → `Drlg::level_rooms`. **Fixed after the run**
    by `play-drlg` (`1c72b9f`, merged in `be60b147`): the outdoor reset
    drops the records of freed rooms (`docs/handoff/play-drlg.md`). Rerun
    `play --frames 3000` on a head that has it: expect no panic.
    **Result 2026-10-08 round 2** (PC 1): `play --frames 600` no longer reaches the DRLG panic; it stops at once on the render panic of C40 (`q-fix-play-gradient-block`). The batch-2 rows were rerun in round 1.
99. Save appearance token table on live data (`formats/d2s-appearance.md`
    §1 r3, Open question 3): `D2_GAME_DIR=<install> cargo test -p
    d2-server --test character_save -- --ignored`
    (`token_positions_on_the_users_install`): expect `hax` 4 … `ktr` 45,
    `cap` 57, `buc` 79, `sst`→`bst` 0x25 with the PROVISIONAL reference
    slots (`ReferenceSlots::provisional_1_14d`). A miss means the
    reconstruction is wrong; then read the 256 × 8 bytes at `0x00744CA8`
    of the 1.14d image into the spec (settles Open question 3).
    **Result 2026-10-08 round 2** (PC 1, release): FAIL at `character_save.rs:358`: `ktr` position left 51, right 45 (assertions before it, `hax` 4 and the earlier tokens, pass). Finding `q-fix-appearance-token-ktr` (`d2s-appearance.md` s1 r3 / OQ3: read the 256 x 8 bytes at `0x00744CA8`).

100. Render audit fixes on the install (`docs/handoff/q-render-audit.md`,
    branch `claude/q-render-audit`): `D2_GAME_DIR=<install> cargo test
    -p d2-client --test game_panels -- --ignored`: expect
    `panel_files_frame_counts_and_sizes` to pass with `menu\horadric`
    frames 0 / 30 2 x 2 at (0, 0), frame 1 92 x 121 at (-205, 17), frame
    15 at (-280, 82) (`ui/panels.md` §12.4; replaces row
    `q-fix-panel-horadric-offsets`). Then `play` in the Rogue Encampment:
    hover an object (box and name on the object, not 8 rows below), hold
    an item over the belt (drawn over it), hover the run button (tip
    centred on its point), walk with a torch-lit floor in view (the light
    pool moves with the player in 1/8-sub-tile steps).

101. World render fixes (`docs/handoff/q-fix-render-world.md`, branch
    `claude/q-fix-render-world`): `D2_GAME_DIR=<install> cargo test -p
    d2-server --test game_world_data -- --ignored
    act_edge_floor_records_resolve_in_the_base_libraries`: expect a tile
    index printed for acts 0–2 (a miss is the original's fatal 0x44C:
    the path or key in `ACT_EDGE_TILE` is wrong). Then `play`: walk from
    the Rogue Encampment into the Blood Moor (a `DrawEdges` level): the
    view keeps moving (no frozen frame) and edge floors fill the map's
    outer strip; cast Fire Bolt next to a wall (the bolt hides behind
    walls in front of it and is additive, not a dark block); drop an item
    behind a wall in a `LOSDraw` level (hidden); an idle object (a
    waypoint, a chest) holds its frame; a waypoint to another act shows
    one black frame after the act load.

Kept entries (unchanged):

**Treasure** (`specs/items/treasure.md`, branch `claude/phase3-treasure`):
Recording (player): `record_rng.py` extended with hooks at `0x0055A6D0`

Phase 6 asset budgets (from `specs/client/assets.md` OQ 1, design
only, no code yet): measure the decoded size (`Σ width × height`) of
every live DCC/DC6/DT1 frame, per file and in total, and of the files a
town and a dungeon scene use; record the numbers in `assets.md` §A5 and
set the default budgets from them.

### Blocked (not runnable yet)

- Native assets full conversion (`specs/formats/native-assets.md` §7.2).
  Runnable now (`native-wire`, `docs/handoff/native-wire.md`): `d2-convert`
  converts excel plus every image and text kind (dc6, dcc, dt1, pal, pl2,
  cof, ds1, tbl, font, animdata, expfield); audio is not converted (OQ3, "no
  sound for now") and `report.txt` lists `.wav` as skipped with that reason.
  C-TABLE step 2 (`native-ctable`, `docs/handoff/native-ctable.md`) runs
  inside `convert` and `verify --deep`: the native excel set is compiled and
  compared with every live `.bin`. Command (local,
  `D2_GAME_DIR` = the 1.14d install):
  `cargo run --release -p d2-convert -- convert --install "$D2_GAME_DIR"
  --out "$TMP/d2native"`, then `cargo run --release -p d2-convert -- verify
  --deep --install "$D2_GAME_DIR" --out "$TMP/d2native"` (`--game` /
  `--native` are accepted as the spec's spellings). Expect: exit 0 both;
  stderr ends `done: N files, 0 failed (…s)` and `verify: all files pass`;
  `report.txt` has `C-TABLE step 2: 73/73 tables identical to the live .bin
  (1 override)` and one `<table>: identical` line per table; `verify` ends
  with `verify: C-TABLE 73/73 tables identical to the live .bin (1 override)`;
  `base/data/global/excel/_bin-overrides.toml` holds exactly one entry,
  `monstats` record 707 `NameStr` bytes `0x0615` (5382), and `files.tsv` has
  its row (kind `excel`, archive `-`, no source SHA; `excel` `converted` is
  the `.txt` count + 1). A different count, a second override or any
  `failed:C-TABLE` row is a finding (the report names table, record, offset,
  column).
  `manifest.toml` `complete = true` and `[counts.<kind>]` with `failed = 0`
  in every kind (the 12 kinds above, no `wav`); per-kind `converted` equal to
  the winning copies `mpq-tool formats` finds (minus audio); `report.txt`
  shows `.wav: N (skipped: audio not converted for now …)`. Any
  `failed:<check>` row is a finding: copy the first difference from
  `report.txt` (likely suspects: DT1 blocks the §2.3 r3 layouts cannot
  hold, tbl whose hash table differs from a rebuild with `[encoding]`,
  DS1/COF fields the sidecar rejects). A rerun of `convert` must be
  near-instant and leave `files.tsv` byte-identical (resume). Record in the
  spec: total and per-kind work time (`report.txt`), native root size
  (`du -sh`), DT1 fallback tiles (report note "blocks layout", OQ 5), `tbl`
  rebuild differences (report note, OQ 6), unnamed blocks per archive and the
  unconverted extensions (both in `report.txt`). Then, once N4 lands,
  `d2-client play` and `verify` with `--source native` vs `--source mpq`:
  identical results.
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
   Answered: never read by 1.14d (`formats/ds1.md` OQ4).
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
`0x58ff90` (`quests-act1.md` §10, not written), so the text list after Akara's
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
  **Answered 2026-10-07** (`monsters/umod-callbacks.md` §2, read from the asm): mode 0 runs before the start function (after the mode damage rewrite `0x005A4F50`) and not at all when the requested mode is 3 (GH); mode 1 runs after the start function and the animation prepare but **before** the cancel of events 0 / 1 and the animation schedule, so a type-7 event it schedules sits **before** the mode's animation events in the timer queue. The reading above (both after the whole mode set) is wrong; RT-R1 now only confirms it.
- RT2 (`action/ai.rs` `monster_level`; `ai.md` §2.4 step 2): the getter for
  the "level" of the teleport heal is not named; code takes stat 12 (`level`)
  unit total (`init.md` §7 rule 4).
- RT3: umod mode 4 (`0x005A43A0`, called from the reaction `0x0057CEE0` at
  two sites) is not placed by `damage.md` §7.1 (call level only, OQ3); stays
  inside `Pending::reaction`. **Answered 2026-10-07** (`monsters/umod-callbacks.md` §2 r5): mode 4 runs on the monster defender, after the GH mode set (get-hit, stunned or `0x0057CB00` allows it) or after the soft-hit path (get-hit refused, or soft hit 0x4000); never on knockback, block or death.
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

**`items/inventory-moves.md` §6–§11** (`impl-moves` §5; M1, M2, M5 and the seamed
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
"clears per-unit flags", `inventory-moves.md` §6.3 r4, without naming them), so a later
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
MI1 [`mutants-inventory` §4] `inventory-moves.md` §7.6 says X "becomes the cursor
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

### PC 2 recording list (spec answers that need a recording or capture)

Folded 2026-10-08 into one list: the former spec-writer lines of this section (the early lines, the ninth-set R2-1..R2-46 and the later PC 2 lines), the `pc2-rec-*.md` lane lists and `docs/handoff/recording-list.md` (now a pointer). Each former line is a `Settles:` item of the entry that captures it; lines already done and folded (pc2rec-d1 / d3 / d4-rng, pc2rec-p1 / p2-packets) are not repeated. **This section is PC 2's testing prompt**: do the entries in order, REC-01 first. PC 1's RE / spec questions (Ghidra answers) are not recordings and live in `xpc-to-pc1.md` and the specs.

#### How to run the list

- Do the entries in order, REC-01 first. Priority 1 = blocks d2rs code or settles a `Pending` / integration conflict (I-n, C-n); priority 2 = unattended (`--auto`), cheapest first; priority 3 = needs manual play. Inside priority 1 the unattended entries come first. REC ids are cited by specs and never renumbered; new entries take the next number.
- Tag **[AUTO]** = no player (`--auto CHAR [--seed N] [--input SCRIPT]`, `tools/trace-recorder/autostart.py`); **[MANUAL]** = a person plays inside the recording window; **[ASSISTED]** = `spawn.py` (debugger-placed monsters, foreground `SendInput`); **[NO]** = not reachable (see the last section).
- Prerequisites: PC 2 (Windows, real GPU), `game/Game.exe` with the reference SHA-256 (`traces/reference-install.toml`; every recorder refuses another), `py` 3.10, branch `claude/pc2-recordings` (autostart, `dumpdrlg`, `check_*.py`). `record_sound.py`, `record_objects.py`, `spawn.py`, `run_scenario.py`, `d2ui.py`, `make_saves.py` live on `origin/claude/local-buddy-q-rec-2026-10-07` / `-q9-rec-2026-10-07`: merge or copy them first (entries say which use them). **NEW HOOK** in an entry = a probe the recorder lacks; add it first (INT3 at the address, log the named fields, check expected bytes as the other hooks do).
- Characters: **expansion only** (a classic one is refused). `TestSor` (`d2s-tool new --waypoints all --quests acts=4`, panel shows tabs I–IV, no Act V) is the default; seed `644409375` (the D1/D3/D4 seed) unless stated; `ScnAma` / `ScnSor` for town-only runs. "Prepared save" = `d2s-tool` edit, copied read-only into `%USERPROFILE%\Saved Games\Diablo II`, never into the repo. `--auto` uses `-nosave`: a run that must Save and Exit is [MANUAL] without `--auto`.
- Input-script fragments (800 × 600; waypoint panel: act tabs y 75, x 115 / 178 / 240 / 300; entries x 200, y 138 + 35·k, k = 0 first entry):
  - `T12` = `wait 3; goto 2 119 300; wait 3; click 178 75; wait 2; click 200 138; waitlevel 40 400; wait 5`
  - `T23` = `goto 2 156,157,237,238,288,323,324,398,402 300; wait 3; click 240 75; wait 2; click 200 138; waitlevel 75 400; wait 5`
  - `T34` = the same `goto`, then `click 300 75; wait 2; click 200 138; waitlevel 103 400; wait 5`
  - Back to Act I from any town: `goto 2 <WPALL> 300; wait 3; click 115 75; click 200 138; waitlevel 1 60; wait 3`
  - Act I wilderness by waypoint (entry y): Cold Plains 173 (level 3), Stony Field 208 (4), Dark Wood 243 (5), Black Marsh 278 (6), Jail 1 348 (29), Catacombs 2 418 (35); each: `goto 2 <WPALL> 300; wait 3; click 115 75; click 200 <y>; waitlevel <id> 400; wait 8`. `<WPALL>` = the class list of the pc2rec-d3 / d4 inputs (header `args` of those raw files).
  - Other script words: `wait S`, `move X Y`, `click X Y`, `rclick X Y`, `hold X Y S`, `key K [S]`, `text STR`, `shot NAME`, `waitlevel ID [S]`, `goto TYPE CLASS[,..] [S [DX DY]]` (type 1 = NPC / monster, 2 = object), `dumpdrlg [LABEL]`, `end`.
- Pairs: one process = one debugger, so an RNG trace and a packets trace are two runs of the same character and `--seed`; the same script then reproduces the same game.
- Where traces go: `traces/raw/pc2rec-<entry>-<kind>.jsonl` (use `--out`; gitignored). Run the matching `check_*.py` (`check_rng`, `check_packets`, `check_tick`, `check_units`, `check_stats`, `check_drlg_acts`) on every raw file; record the result. After every run `tasklist | findstr Game.exe` must be empty. Commit only a normalized trace a spec asks for (`convert_*.py`), never raw files.
- How to report: one line per entry appended to `docs/handoff/xpc-to-pc1.md`: `REC-nn | DONE / PARTIAL / BLOCKED | raw file(s) + sha256 | check result | one-line finding per question id (matches / differs: what) | what was not recorded`. PC 1 reads the line, edits the spec (closes the OQ) and strikes the line with the commit. Already done and folded (not repeated): pc2rec-d1 / d3 / d4-rng, pc2rec-p1 / p2-packets (waypoints OQ1 incl. cross-act travel, drlg levels OQ1 / OQ3, outdoor OQ1 / OQ4 / OQ5 / OQ7, outdoor-tilesub OQ2, maze OQ1, intents-events OQ14 chat line, Act III jungle ids).


#### Priority 0 (high-priority captures: RNG draw order or wire / saved byte layout; run these first)

##### REC-81 [AUTO] population.md §11.4 hcIdx 10 (Radament)
- Priority P0 (RNG draw order).
- Settles: population.md §11.4 hcIdx 10 (Radament).
- Steps: Act II save at the Sewers Level 3 waypoint; `record_rng.py --seconds 120` + `record_tick.py --seconds 120` with a hook on `0x005A49B0` / `0x005B23C0`; walk into Radament's room so he spawns.
- Output: seed (unit vs room) and caller of the `roll(5)` draw, mode argument of each `0x005B23C0` call.
- Compare: boss unit seed, mode 1. Fold: write the result into the cited rule and drop its PROVISIONAL line.

##### REC-82 [AUTO] ai-bodies-7.md §27 step 12 (aitype 1 / 12)
- SETTLED 2026-10-08 from the binary (pc1-s8): no recording needed; drop this entry.
- Priority P0 (RNG draw order).
- Settles: ai-bodies-7.md §27 step 12 (aitype 1 / 12).
- Steps: `d2s-tool` Assassin with Shadow Master and a few aitype-1 / aitype-12 skills; `record_rng.py --seconds 240` + `record_tick.py --seconds 240` while the Shadow Master fights in the Blood Moor; log unit-seed steps with caller inside `0x005EB970` per think.
- Output: draw sequence per skill in list order.
- Compare: no `roll(A2n)` for an aitype-1 skill whose aurastate the unit lacks; aitype 12 non-progressive scored with the aitype 4 rule. Fold: write the result into the cited rule and drop its PROVISIONAL line.

##### REC-62 [MANUAL] render/camera.md §8 (which missiles / skills reach `0x004D2610`, `0x004D3D30`, `0x004D5310
- Priority P0 (RNG draw order: each shake draws 2 values per frame on the client player seed).
- Settles: render/camera.md §8 (which missiles / skills reach `0x004D2610`, `0x004D3D30`, `0x004D5310`, `0x004D6680`, `0x004D6820`, `0x004D7400`, `0x004D8000`, `0x004F0710`, and the computed (A, t1, t2, t3)).
- Steps: NEW HOOK at `0x00476A80` entry logging return address, ECX, EDX, [esp+4], [esp+8], client update count, local player seed before / after; run during the REC-31 skill / missile sitting and the REC-29 Act IV run (Diablo).
- Output: `pc2rec-r62-shake.jsonl`.
- Compare: per site the parameters against the §8 table, the triggering class, 2 draws per frame while a ≠ 0. Fold: write the result into the cited rule and drop its PROVISIONAL line.

##### REC-51 [AUTO] client/model.md OQ1 and OQ2, client/msg-ui.md OQ2 (0x10000 per-caller conditions)
- Priority P0 (RNG draw order in the client animation).
- Settles: client/model.md OQ1 and OQ2, client/msg-ui.md OQ2 (0x10000 per-caller conditions).
- Steps: `record_frames.py --seconds 120` + `record_rng.py --seconds 120` on one character in the Blood Moor: walk, run, fight 3+ monsters until one dies; hooks: entry/exit of 0x004AFF60 (unit type, GUID, mode +0x10, seed +0x20/+0x24); 0x00464810 and 0x004647D0 (caller, unit, mode); 0x00463390 (local player path x/y); per frame log `[0x007A0498]`, the server tick, the local player's client and server positions.
- Output: `pc2rec-r51-frames.jsonl`, `pc2rec-r51-rng.jsonl`.
- Compare: client seed draws per frame against the message-driven draws in client/msg-units.md; every 0x004AFF60 mode change against that frame's S→C messages; local position against server position (any lead = prediction). Fold: write the result into the cited rule and drop its PROVISIONAL line.

##### REC-95 [ASSISTED] sim/intents-events.md §7.4 r3 (0x4C / 0x4D level byte), sim/pathing.md §10 r2 (player skill-mode messages)
- Priority P0 (wire byte layout).
- Settles: `intents-events.md` §7.4 r3 (the 0x4C / 0x4D level byte: base + bonus clamped to a byte, PROVISIONAL); `pathing.md` §10 r2 (which player modes send 0x4C / 0x4D, and whether the attacker's own client receives them; d2rs-own today: every skill mode A1, A2, SC, TH, KK, S1–S4, SQ, own client included).
- Steps: `record_packets.py --seconds 120` on two clients in one game (TCP/IP) in the Blood Moor: client A attacks a Fallen (left click, Attack), casts a point skill on the ground and a unit skill on a monster; a Fallen Shaman casts at A. Log every S→C 0x4C / 0x4D / 0x0C on both clients with the server tick and the caster's skill level.
- Output: `rec96-packets.jsonl`.
- Compare: the level byte against the skill level; the player modes that send a skill message and which clients get it; fold into the cited rules and drop their PROVISIONAL lines. Also note, for `stitch-server-core.md` §2, the hostility (`0x00554200`) and melee-range (`0x00622870`) answers seen (who can attack whom, the reach in sub-tiles).

#### Priority 1

##### REC-01 [AUTO, no run] Analysis of two existing raw files
- Priority 1 (cheapest: no game).
- Settles: `client/model.md` OQ14 + OQ15 first part: from `tp80-packets.jsonl` list the S→C messages between the town-portal use and the C→S 0x4B after tick 2919 (expect the 0x08 dropping the hireling's room, no 0x0A / 0x15 / 0xAC for GUID 1 before it) and the same window at the second teleport (R2-28, C80); `world/objects.md` OQ14: in `obj1-objects.jsonl` the mode argument of the two class-37 allocations with speed 0 (expect 2).
- Steps: `py` over the two files (grep `s2c` records between the two tick markers; filter allocations of class 37).
- Output: a note in the xpc line (message list, mode values).
- Fold: PC 1 closes OQ14 / OQ15a and objects OQ14; a differing message names a model.md rule to fix.

##### REC-02 [AUTO] Join stream and new-character entry (packets + RNG twin)
- Priority 1 (blocks code: the join order is an open integration conflict).
- Settles: I-5 (client room seed: which function steps it: `model.md` §2 r6 vs `msg-units.md`), I-6 (join order: 0x07 × 9 then 0x15, or ×10), SJ-1..3, C83, C84 (frame-1 S→C order and callers from 0x59 to 0x15; R2-8, R2-10), C85 / R2-9 (game +0x80 = `0x00546C60`'s result = 0x03 u32@8), `client/model.md` OQ9 (client side of the join stream; R2-10), `sim/intents-events.md` OQ13 (caller of the first 0x48 in frame 2), `client/bridge.md` OQ6 (a 0x2C then a 0x0A in one chunk: look in this stream), `quests.md` OQ4 / R-PQ-3 (new character: 0x5E, 0x28 type 6, 0x29 each sent twice; an existing character once; code `0x0056A072` then `0x005344EF`), `sim/rng.md` OQ2 (a brand-new character's first game and a save-and-exit reload: log game +0x7C and the S→C 0x03 map seed; the reload half is a 30 s [MANUAL] run without `--auto`).
- Steps (A, packets): `py tools/trace-recorder/record_packets.py --seconds 60 --auto ScnAma --seed 1234 --input "wait 5; end" --out traces/raw/pc2rec-r02a-packets.jsonl`; (B, rng twin): the same with `record_rng.py --seconds 120 ... --out traces/raw/pc2rec-r02b-rng.jsonl`; (C, existing character): A again with `--auto TestSor --seed 644409375` (`pc2rec-r02c-packets.jsonl`). Level-1 fresh save `ScnAma` stands for "new character" (R-PQ-3); if its entry sends each message once, also run a character created in the game's own menu by hand (manual, 30 s). NEW HOOK for C85: log EAX of `0x00546C60` and game +0x80 at the first tick.
- Output: the three files above.
- Compare: S→C order in frame 1 with callers against `path-placement.md` §11 and `model.md` §11 r3; the room-seed steps in B (sites from `rng.md`) against I-5; C against A for the twice-vs-once. Fold: PC 1 fixes the losing spec text; d2rs's join order and client seed step follow.

##### REC-03 [AUTO] Runtime tables not in `dump_tables`
- Priority 1 (blocks the treasure walk code).
- Settles: `items/treasure.md` OQ4 (IT-9): runtime TC array `[0x0096C5EC]` (count × 0x2C bytes plus each TC's entry block) and chest table `[0x0096C5F4]` equal the §1 model built from `treasureclassex.bin`.
- Steps: `py tools/trace-recorder/dump_tables.py --out traces/raw/pc2rec-r03-tables` with the two dumps added (NEW HOOK-free: memory read at the dump point `0x0044B93C`, files `map-tc-runtime.bin`, `map-chest-table.bin`). ~7 s.
- Output: `traces/raw/pc2rec-r03-tables/`.
- Compare: byte for byte with the §1 model (`data-tool`). Fold: treasure.md OQ4 closed; a differing start / flag names the §1 rule.

##### REC-04 [AUTO] Tick run over a waypoint walk with the room / unit probes
- Priority 1 (settles `Pending` of `drlg/rooms.md` OQ13 / OQ15 / OQ23; blocks DRLG client code).
- Settles: `drlg/rooms.md` OQ15 (client build timer (B, T, cursor) per client update over a level load), OQ23 (across a client level free: per client update `[0x007A0498]`, client DRLG +0x98, +0x45C, +0x460 and the byte at (+0x460)+0x44, plus every room free address; whether a freed cursor room is reallocated before the next timed step, §4.6 rule 11), OQ13 + `drlg/wall-remap.md` OQ1 (every built room's link chains from memory; Acts I–IV; Act V see REC-30), `drlg/levels.md` OQ7 + `monsters/population.md` (R2-31: room +0x64 lists of a crypt and an outdoor level), `sim/units.md` OQ8 (leave and re-enter a wilderness area: restore order and GUIDs, `0x00542B40`), `sim/tick.md` OQ2 (`0x005416B0` with EDX = 0 over a long run: any timer without a unit, log the caller), part of `sim/units.md` OQ1 (0.2.0 `anim` + `site` records: waypoint / walk / town only; combat part in REC-09).
- Steps: `py tools/trace-recorder/record_tick.py --seconds 400 --auto TestSor --seed 644409375 --input "<T12>; dumpdrlg act2; <Act I tab: Cold Plains>; wait 8; dumpdrlg coldplains; <Stony Field>; <Cold Plains again>; <Jail 1>; <Catacombs 2>; dumpdrlg cata2; <T-to Act III via T12/T23>; dumpdrlg act3; end" --out traces/raw/pc2rec-r04-tick.jsonl`. NEW HOOKS: the OQ15 / OQ23 fields per client update, room-free addresses, `0x005416B0` entry with EDX = 0 and its caller, a `dumpdrlg rooms` word writing each built room's link chains and +0x64 list.
- Output: `pc2rec-r04-tick.jsonl` (+ its `dumpdrlg` records in the footer notes).
- Compare: `check_tick`, `check_units`, `check_drlg_acts` must pass (0 errors); then the logged fields against rooms.md §4.6 and the wall-remap link chains. Fold: close OQ13 / 15 / 23 / wall-remap OQ1 / levels OQ7 / units OQ8 / tick OQ2 or edit the rule.

##### REC-05 [AUTO] Act changes by waypoint: packets and palette frames
- Priority 1 (I-4 integration conflict: act palette switch).
- Settles: I-4 / R2-11 (act palette switch: `model.md` §11 r4 compares the Levels `Act` byte, `msg-units.md` §3 r4.4 the area byte of `0x0061DB70`), `quests.md` OQ10 / R-PQ-4 for the waypoint-driven act changes (S→C 0x61 bytes against §8.1; Warriv, Meshif, the Hellgate and Tyrael portals are REC-27 / 28 / 29), `client/model.md` act-change message order (the part not settled by waypoints OQ1, already folded), `client/model.md` OQ8 (a waypoint to another act: S→C 0x05, 0x03, 0x53 bytes; static answer 13).
- Steps: (A) `record_packets.py --seconds 120 --auto TestSor --seed 644409375 --input "<T12>; <T23>; <T34>; <back to Act I>; end"`; (B) `record_frames.py --seconds 120 --auto TestSor --seed 644409375 --input "<same>" ` (GDI palette read each frame). NEW HOOK: log the palette-switch call (`0x0061DB70` area byte, Levels `Act` byte) in A.
- Output: `pc2rec-r05a-packets.jsonl`, `pc2rec-r05b-frames.jsonl`.
- Compare: first frame whose palette changes against the act-change message tick; the byte the switch reads. Fold: PC 1 picks the correct reading in `model.md` §11 / `msg-units.md` §3.

##### REC-06 [AUTO] Waypoint object init (town waypoint after travel)
- Priority 1 (cheap, shares REC-05's route).
- Settles: `world/waypoints.md` OQ3 (R-NV-13): does init 17 (`0x00547210`) run for the town waypoint on arrival, with which arrival-list head (game +0x10F0 → +0x1110); the arrival node left in the list (edge case 3).
- Steps: `record_packets.py --seconds 180 --auto TestSor --seed 644409375 --input "<T12 to Cold Plains by waypoint: Act I tab, y 173>; <back to Rogue Encampment: click 200 138>; <Cold Plains>; <Rogue Encampment>; end"`. NEW HOOK: INT3 `0x00547210` entry and exit, log object class, room, the list head.
- Output: `pc2rec-r06-packets.jsonl`.
- Compare: breakpoint hits per arrival; if never hit, the town object came back without init (its 0x51 mode 2). Fold: waypoints.md OQ3.

##### REC-07 [MANUAL] Object operation and room population
- Priority 1 (blocks object code; objects.md OQ1 is the largest open draw order).
- Settles: `world/objects.md` OQ1 (R2-18: draw order of §8–§12 and the 0x0E / 0x4D / 0x60 bytes of §14 for one chest, shrine, door, well and portal; a `Sync` = 0 allocation, OQ2), OQ10 (a fire object class 160–162 a few seconds: no 0x0E 1 → 2 update), R2-17 (Cairn stone / gibbet / tome operate: 0x0E ticks and bytes, C78: the Cairn part is in REC-11), `sim/units.md` OQ4 (delayed events of a chest, door, trap, shrine: object-control seed draws of events 0 and 8 with caller addresses, scheduled frame), `client/model.md` OQ15 second part (one shrine use: 0x0E / 0x4D bytes and the `0x004B9A00` request, id = table +0x10 for the code), `sim/units.md` OQ1 (combat-free part: shrine, well, trade), `world/object-population.md` OQ1 (OP-1: first population of fresh rooms, R2-32 / S9-A6: eight active-room-seed steps per room plus one per passing slot, populate steps, classes from the Blood Moor `ObjGrp`), `client/model.md` OQ9 monster half (a monster's client +0x20 seed after its 0xAC against the server unit's seed, from the Blood Moor monsters of run 3), `monsters/init.md` OQ4 (one population pass with the rng hook and callers = run 2), `sim/units.md` OQ6 trade part (78 rows left `proof = file`: a `site` recording over a trade and an item use; the combat part is REC-09).
- Steps: new Normal character, Rogue Encampment → Blood Moor (west, walk on ~60 s so new rooms populate), operate one chest, shrine, door, well, a town portal (scroll) and a fire object if met. Three runs of the same route and character, `record_objects.py` (q9-rec) for the first: (1) `record_objects.py` (packets + RNG of the operates); (2) `record_rng.py --seconds 120` (population: OP-1); (3) `record_packets.py --seconds 120` (the 0x51 objects of the same route); optionally `record_tick.py` for units OQ4. Write each action with its wall-clock second.
- Output: `pc2rec-r07-objects.jsonl`, `-rng.jsonl`, `-packets.jsonl`.
- Compare: `check_rng` / `check_packets` pass; draw lists per operate against objects.md §8–§12; population draws against object-population.md §3–§7 (no corpse-on-stick from population, §7.7). Fold: close objects OQ1 / 10, units OQ4, population OQ1; mismatches become spec edits.

##### REC-08 [MANUAL] A monster kill with a drop, pick-up and stash
- Priority 1 (blocks the treasure / item creation code).
- Settles: `sim/intents-events.md` OQ11 (tick order 0x9C, 0x69, 0x65 with callers; expect 0x65 from `0x0053FB30` called by `0x0053FC20`; breakpoint: the return address one level up of `0x0053FB30` in a kill tick), R2-33 (the drop position, S8-A3, and the treasure walk S9-A1 (5)), `items/treasure.md` OQ1–OQ3, items R1 (item creation draws: `items/affixes.md` OQ1, `generation.md` OQ2, `properties.md` OQ1, `quality.md` OQ1), `items/inventory.md` OQ1 / OQ2 / OQ12 (R1–R5 item-move packet order: pick up, drop, stash, swap, gold), `items/bitstream.md` OQ2 (pickup and stash of a set, unique, rare, runeword, ear, gold pile, tome and a socket-filled item: the S→C 0x9C / 0x9D of each: a `d2s-tool` save holding these items in the stash makes it deterministic), `combat/vitals.md` OQ8 (every S→C 0x18 / 0x95 / 0x96 / 0x1A–0x1C with tick, over damage, potions, running), `quests.md` packets for 0x50 where it appears.
- Steps: Blood Moor, kill monsters until a drop (use a high-level character so drops are items, a second pass for each of the listed item kinds), pick up, stash, drop. Pairs: `record_rng.py --seconds 300` run and `record_packets.py --seconds 300` run, same character, seed fixed with `--seed` where the game start is automatic; treasure OQ1–3 also under `record_stats.py` if a drop has stats to check.
- Output: `pc2rec-r08-rng.jsonl`, `-packets.jsonl` (+ `-stats.jsonl`).
- Compare: the draw order of the kill tick against `treasure.md` §3–§9; packet order against `inventory.md` §R1–R5; d2rs's item streams byte-for-byte. Fold: close the OQs or edit.

##### REC-09 [MANUAL] Combat core with stats
- Priority 1 (blocks the combat / stat code: spec rules are static readings).
- Settles: `combat/damage.md` OQ1 (hooks `0x0057DBF0` entry / exit EBX record +0x08…+0x4C; `0x0057C6C0` defender stat 6 before / after; attacker and defender seeds), OQ2 (life / mana leech item and a vampire hitting the player: `0x0057C420`, `0x0057A980` / `0x0057AA00` amounts), `combat/hit.md` OQ1 + OQ2 and §6.4 / edge case 8 (shield-wearing player and blocking monsters: `0x0057DB61`, `0x0057DFB0`, `0x0057E04B`; a player with two `passive_weaponblock` entries: `0x0057DCA0` return), `combat/vitals.md` OQ1 (level-up, five stat points and a skill point: 0x3A / 0x3B, hooks `0x00570880`, `0x00570D60`, stats 4–13), §4.7 (a level ≥ 2 character dies in Nightmare / Hell, picks up the corpse: stat 13 before death / after death / on the corpse `0x0057F875` / after pickup, expect + `pct(loss, 75, 100)`; a Token of Absolution: 0x21 per class skill, stats 5 and 4), `sim/stat-lists.md` OQ4 + `sim/stats.md` OQ1 (`record_stats.py` over equips, a level-up, a buff and a curse; items with +max life / mana, IAS / FRW, a socketed item: `check_stats.py` passes with no hidden writes), `client/stat-lists.md` OQ6 (a buff: 0xA8 bytes), `client/msg-skills.md` OQ1 (equipping a charged item), `sim/units.md` OQ9 rest (kills by melee, missile and spell: at `0x005A6FF0` entry the record R bytes +0x00…+0x1C, byte +0x14, caller chain) and `monsters/init.md` OQ13 (a bonefetish1 kill beside the player: death area damage, §4.6 branch 1.4; the server 0xAC of a hireling and of a boss with umods, §24 rules 4 / 6), `render/camera.md` OQ5 (`0x00650840` calls with caller per client update over monster fights; a local player's missile's first client update, two steps expected), `sim/units.md` OQ1 (combat, skills, cooldown part), `sim/intents-events.md` §3.5 (a monster casting at the player from outside its rooms: 0x4D; a pet summon: 0x99 / 0x9A), `sim/units.md` OQ6 combat part (78 rows left `proof = file`: a `site` recording over combat with skills to confirm the scheduled unit kinds of the state-timer, damage and trade sites; trade and item use in REC-07).
- Steps: three sittings of ~240 s on one character with the hooks named: (1) `record_rng.py` (damage, hit, leech, level-up draws and seeds); (2) `record_stats.py --seconds 300` (equips, level-up, buff, curse, leech, charged item); (3) `record_packets.py` plus `record_tick.py` (0x18 / 0x95 / 0x96 / 0x1A–0x1C bytes, units OQ1 / OQ9). Death + corpse in sitting 2 on a Nightmare character. NEW HOOKS: those listed per OQ above.
- Output: `pc2rec-r09-{rng,stats,packets,tick}.jsonl`.
- Compare: `check_stats`, `check_tick`, `check_units`, `check_packets` pass; recompute §3–§5 of damage.md / hit.md from the logged stats: every record and life change must equal. Fold: close the OQs.

##### REC-10 [MANUAL] Hireling session (and resurrect at an NPC)
- Priority 1 (blocks `hirelings.md`; I-1 0x7A order is a live integration conflict).
- Settles: `world/hirelings.md` R2-27 rest / R2-42 / OQ7 / OQ2 / OQ9 (HL-7, HL-8): with `bdMercTwo`: hire (0x9B + 0x7A), level-up (0xA1 / 0xA2 and the second stats batch), death (0x9B name id + cost, 0x7A remove), resurrect at Kashya (0x9B `ffff 00000000`, 0x81, 0x2A code 5; = `npc.md` OQ5 / OQ6 R-NV-1: C→S 0x62, gold 0x1D next frame), give / take an item (two 0x540E60 notices, new GUIDs), the 0x7A order and remove broadcasts on the wire (I-1, SK-2a, HL-3, R2-26), `world/hirelings.md` OQ2 / R2-44 (die, change level, return, resurrect), §8 rule 5 (the hireling dies in the wilderness, the player waits > 11 room passes = 132 frames in town so the room is freed, then resurrects: expect 0x81, the merc's 0xAC at the player, `9b ffff 00000000`, 0x2A code 5, no 0x4B / error), R2-28 / `client/model.md` OQ10 (0x7A / 0x81 in a game with a hireling; teleport with a living hireling: its warp messages vs the player's room / 0x15), R2-29 (an act change with a hireling: classic act change §6 r3–4; waypoint Act I → II and back), `intents-events.md` §3.5 (0x58 codes and 0x4E on the hire list), `sim/units.md` OQ3 (a hireling following: the 20 history entries of player data +0xA8 per frame and a town-portal teleport frame), `world/hirelings.md` OQ1–OQ5, OQ9 rest and §6 rule 6 (why the first teleport had an extra 0x15: read it from the teleport window), `world/npc.md` OQ5.
- Steps (`record_packets.py --seconds 600`, character level ≥ 9 in Act I, ≥ 2,000 gold, a hired Rogue): hire at Kashya; fight in the Blood Moor until the Rogue levels; give / take an item; let the Rogue die; go to town and resurrect; repeat with a level change between death and resurrect; separately die far away and wait > 132 frames in town; a waypoint to Act II and back (Sorceress Teleport once for R2-28). A two-player game (second client sees the owner's hireling level up: `hirelings.md` OQ7 / R2-43; expect 0x9E–0xA0 stats there) is [NO] unattended: one run if a second client over TCP/IP can be joined, else mark BLOCKED.
- Output: `pc2rec-r10-packets.jsonl` (+ `-rng.jsonl` for the resurrect frame).
- Compare: `check_packets`; message order per event against `hirelings.md` §8, §13; the 0x7A order against `pets.md` §8. Fold: close the OQs; pick I-1.

##### REC-11 [MANUAL] Act I quest session
- Priority 1 (blocks the Act I quest code; C78 / C79 are open).
- Settles: `quests.md` OQ2 / R-PQ-1 (Flavie: player-seed draws between C→S 0x2F / 0x31 and S→C 0x27; message lines of the 0x27: chains 25 and 30 share event 0), `quests-act1-rest.md` OQ3 / R2-12, R2-13, R2-17 (a: Tristram, Cairn stone done, operate the gibbet: mode 1, slot 4 = `0x2002`, 0x28 at once, 17 frames later Cain (class 146) at gibbet + (3, 3) and `5d 04 00 06 0000`; b: Forgotten Tower 5: kill the Countess with a chest in her room: one monster 326 mode 12 at the death spot, one missile 332 per listed chest), OQ12 / R-QC-2 / C79 / R2-16 / R2-40 (the town-Cain marker, class 385, after Cain left Tristram: packets + RNG: Cain's spawn draws must fall between the marker's unit-seed step and the next preset unit's), OQ5 / R-PQ-7 / R2-14 (a: after killing Andariel (state 4) leave and re-enter Catacombs 1: no 0x5D on entry, state kept; b: complete A1Q2 with no hireling and talk to Kashya: order 0x28, the hireling's 0x50 and creation messages, then 0x27, 0x29), R2-15 (save header progression bits 8–12 after Andariel; I-3), `quests-status.md` OQ1 / R-PQ-14 (the quest log in several states: Den of Evil started; 3 monsters left; just completed; completed in an earlier game; the screen and the C→S 0x58 after the completion animation: `record_frames.py` + `record_packets.py`), `sim/intents-events.md` §3.5 (talk to an NPC with a quest line: 0x27 count > 1, 0x50, 0x91, 0x89; log caller and bytes), `world/quests-act1-rest.md` OQ2–OQ5 (the open ones not listed above: read each OQ's own capture).
- Steps: a new Normal expansion character; play the listed quest points in order, one packets + RNG + tick run per part (`record_packets.py --seconds 300`, `record_rng.py --seconds 300`, `record_tick.py` for the Cain rescue), `record_frames.py --seconds 120` for the quest log opens. Use a `d2s-tool` save with the quest state pre-set for (a) of OQ3 and OQ5.
- Output: `pc2rec-r11-*.jsonl`.
- Compare: each listed byte / frame / draw against the OQ's expectation. Fold: close the OQs.

##### REC-80 [NO RUN] population.md §6.3, §10.3, §11.5 rule 6
- Priority P1.
- Settles: population.md §6.3, §10.3, §11.5 rule 6; ai-bodies-7.md §27 step 8.
- Steps: (Ghidra read, no game run) `py tools/ghidra/disasm.py fn 0x005A09E0`, `fn 0x005B2570`, `fn 0x0054E600` (+ the preset-id switch of `0x005559A0`), `fn 0x005EB6D0`.
- Output: creation-call mode and room args; any SetBoss / flag test before `0x0058F030` / `0x0058F100`; the default case of the preset-id switch; branch structure of the callback's counters.
- Compare: the PROVISIONAL text in each section. Fold: write the result into the cited rule and drop its PROVISIONAL line.

#### Priority 2 (unattended, cheapest first)

##### REC-12 [AUTO] Short frames run: first-frame order, settings word, panels, light registry
- Settles: `render/lighting.md` OQ6 / RA-L2 (the flushed S→C buffer holding 0x53 precedes the first in-game `EndScene`; any earlier frame has `env` I 128 white, §9.1), `render/blend-modes.md` OQ3 / RA-B3 (settings word `[0x0072DA5C]`, Blended Shadows, logged once per frame), `render/camera.md` OQ6 (client player seed `unit +0x20` at frame start and end over cursor movement), `render/draw-order.md` OQ14 (a capture with a panel open: any UI or cursor draw between the world passes `0x00456EE0` … `0x00477980`), `render/lighting.md` OQ10 (memory read of `[0x0072DA50]` and `[0x0072A348]` at the first in-game draw, with and without a "Light Quality" registry value: two runs; the value is the game's own registry key on this PC, restore it after), `render/lighting.md` OQ5 part (the 0x89 id 0 needs REC-32), `sim/unit-order.md` OQ5 (the client room unit-list order before each draw sort, one town scene: NEW HOOK at the sort, log the list per draw).
- Steps: `record_packets.py --seconds 60 --auto ScnAma --seed 1234 --input "wait 10; end"` + `record_frames.py --seconds 60 --auto ScnAma --seed 1234 --draws-every 1 --input "wait 5; move 400 300; move 600 200; wait 2; key I; wait 3; key ESC; key C; wait 3; key ESC; end"` (NEW HOOK: log `[0x0072DA5C]` once per frame, `[0x0072DA50]` and `[0x0072A348]` at the first draw). Repeat the frames run with the "Light Quality" registry value set and absent.
- Output: `pc2rec-r12a-packets.jsonl`, `pc2rec-r12b-frames.jsonl`, `-r12c-frames-lq.jsonl`.
- Compare: tick of the 0x53 flush vs frame 0; seed per frame; draw log between the world passes with a panel open. Fold: lighting OQ6 / OQ10, blend OQ3, camera OQ6, draw-order OQ14.

##### REC-13 [AUTO] The day cycle in one long run
- Settles: `render/lighting.md` OQ7 / RA-L3 (`sin` / `cos` rounding: recorded `env[0]` = I of §9.3 r4 from the recorded ticks; every θ where it differs by 1; first extend `record_frames.py` to log env +0x08 (ticks) and +0x28 (speed) beside +0x0C), R2-7 (env +0x0C and RGB per client update over a day vs `Environment::update`, C69), `render/shading.md` OQ1 / RA-S1 (floor light gradient: the `FloorTileDraw` 768-byte light grid vs the CPU render at night, on open floor with no other light, then with a camp fire at the screen edge), `render/lighting.md` OQ11 (answered statically: a day-period change with a fire 39 in sight: mode 0 without light in the day, mode 1 with light radius 9 otherwise: confirmation), `lighting.md` OQ9 / RA-L4 (the light records that change the digest: see REC-14).
- Steps: `record_frames.py --seconds 2100 --auto ScnAma --seed 1234 --draws-every 50 --draws-light --input "wait 2090; end"` in the Rogue Encampment (one day = speed × 360 ticks: 46,080 ticks ≈ 1,843 s at speed 128; use the recorded speed). If `lighting.md` §9 says the town differs from the Blood Moor, repeat manually in the Blood Moor at a dead end (RA-L3's own wording).
- Output: `pc2rec-r13-frames.jsonl` (size: check the disk first).
- Compare: `env[0]` vs §9.3 r4 for every frame; light grids at the night frames (frames' `env` colour (125, 144, 243), normal period 5) vs `FloorTileDraw` CPU reference (`d2-client verify`). Fold: lighting OQ7 / OQ11, shading OQ1.

##### REC-14 [AUTO] Frames: shadows, light records, item colormaps (prepared saves)
- Settles: `render/blend-modes.md` OQ1 + OQ2 / RA-B1 + R2-6 (shadow shape and position §5 r2–r4, pixels `A0[256·d + 0]`; ethereal weapon mode-1 table, ethereal shield mode-2 table in the §2 orientation: a frame matching only with row / column swapped decides the other orientation), `lighting.md` OQ9 / RA-L4 (the light records of §6.1 that change the digest between frames: Rogue Encampment, 7 standing positions, 30 s each, `--draws-every 1`, digest of §12 r3 in the key), `render/shading.md` OQ4 / RA-S2 (item colormaps, §6 r4: a unique and a set item with `chrtransform` ≠ 0, a magic item with a `transformcolor` affix, a socketed normal item; dropped on open floor and in the inventory), `render/capture.md` OQ7 (rerun of run 1b, frames 13,486–14,636, with `--draws-every` and the light-map digest).
- Steps: prepared saves `ShadEth` (ethereal weapon + shield equipped) and `ColItems` (the items in the inventory). `record_frames.py --seconds 60 --auto ShadEth --draws-every 1 --input "wait 20; end"` (also with the weapon swapped out: a second save); `--seconds 300 --auto ScnAma --draws-every 1 --input "<7 × (move X Y or click X Y; wait 30)>"` (positions from a first `shot` run); `--auto ColItems --input "key I; wait 3; <click item; click open floor> × 4; wait 5; key I; wait 5; end"` (cell coordinates from a `shot` first). Needs REC-12's settings-word hook.
- Output: `pc2rec-r14{a,b,c}-frames.jsonl`.
- Compare: CPU reference renders of the frames equal the capture. Fold: blend OQ1 / OQ2, lighting OQ9, shading OQ4.

##### REC-15 [AUTO] World clicks and keys: C→S ids
- Settles: `ui/controls.md` OQ2 (controls-0001: C→S id per world click, hold resend interval, Shift / Ctrl / R / Alt effects: recording 015956 showed 0x10 every 13 frames), the 0x58 / 0x4E senders of `intents-events.md` §3.5 that need no shop (hotkeys only).
- Steps: `record_packets.py --seconds 180 --auto ScnSor --seed 1234 --input "wait 3; click 600 300 x5 (wait 1 between); key CTRL 1; click ..; key R; click ..x5; key R; hold 600 300 3; rclick ... x3; hold-right via hold + rclick; key SHIFT 3 with click; key ALT 2; key 96 (Numpad0); key 103 (Numpad7); key 73 (I); key 87 (W); end"` (the Fire Bolt right skill: a prepared `ScnSor` with Fire Bolt on right and a monster near the town edge for steps (5)–(6) is not available: in town clicks on the ground only; the monster steps are in REC-09). If a modifier cannot overlap a mouse click with `PostMessageW`, do those steps by hand.
- Output: `pc2rec-r15-packets.jsonl`.
- Compare: id (0x01 / 0x03 / 0x05–0x11), tick of every send and resend interval; Ctrl → 0x53 / 0x54; Numpad → 0x3F 0x19 / 0x20; wheel → 0x3C. Fold: controls OQ2.

##### REC-16 [AUTO] Key-configuration panel and text checks
- Settles: `ui/controls.md` OQ6 + OQ7 (controls-0002: `A` vs `C` columns for Character Screen; whether Accept rewrote `<name>.key` and `default.key`; row pitch, highlight, scroll step, button positions), `ui/text.md` OQ5 (memory read of `0x007D6268` on a loading screen) and OQ9 (captures `text-0001`, `text-0002`).
- Steps: `record_frames.py --seconds 40 --auto ScnSor --seed 1234 --draws-every 1 --input "wait 3; key ESC; click <Configure Controls>; wait 2; hold <list> 0; scroll bottom; click <Key/Button Two of Inventory Screen>; key K; click <Cancel>; ...; end"` (coordinates from a `shot` run first); `Get-ChildItem` the two `.key` files before / after; repeat at 640 × 480 (the pane's 640 option). NEW HOOK for text OQ5.
- Output: `pc2rec-r16-frames.jsonl`, the `.key` mtimes in the xpc line.
- Fold: controls OQ6 / OQ7, text OQ5 / OQ9.

##### REC-17 [AUTO] Inventory, gold, character, stash, control-panel UI frames (prepared saves)
- Settles: `ui/inventory.md` OQ1 + OQ5 (inv-0001: tints, rectangles `0x004F6340` family, cursor item draw position, hotspot: a wand 1 × 3, helm 2 × 2, armor 2 × 3; hover over empty, one item, two items, outside the grid, the helm slot), §5 (inv-0002: hover anchor and box clamping for a ring top-left, an armor bottom-right, the equipped weapon, a belt potion, a stash item), §9 + OQ5 + OQ7 (inv-0003: gold dialog pixels, 123 typed, deposit 1000 `4F 14 00 00 00 E8 03`, withdraw 500 `4F 13 00 00 00 F4 01`, withdraw 999,999 clamp), `ui/panels.md` §8.11 (character panel of a level-99 character, experience 3,520,485,254, and of level 1), `ui/panels-2.md` OQ4 (stash open at 800 × 600, expansion and classic: the `GoldMax` line font), `ui/panels.md` §12.4 / OQ11 (cube transmute frames at steps 1, 15, 29 and the step after, at 800 × 600 and 640 × 480: prepared cube save), `ui/control-panel.md` OQ1 + OQ6 (a globe refilling after a potion frame by frame; the control panel at 640 × 480 and 800 × 600 with the belt popped and the mini panel in its three layouts), `ui/messages.md` OQ1 (chat lines: whisper, echo, broadcast; a timed box; NPC dialog scrolling with frame times), R2-36 / `ui/panels.md` UP-7–UP-27 (`placement-0001`, `ui-0001`, `ui-0002`: the rectangle and draw-order readings), `client/msg-ui.md` §4–§11 chat part (one chat line and one whisper to the own name: C→S 0x14 / 0x15 and S→C 0x26 forms 1, 2, 6, overhead form 5; breakpoints `0x0049F410` (overhead set: unit, text, language) and `0x004A1600` (record bytes)).
- Steps: `record_frames.py --draws-every 1` and `record_packets.py` (two runs each), `--auto InvUI` (prepared: the items, 5,000 gold, a belt with potions; a level-99 save `Lv99`; a cube save). Clicks and key `I` / `C` / `B`, `text` for chat; coordinates from a `shot`; stash object by `goto 2 <Bank class from objects.txt>`. Two-monster overhead text (`messages.md`) is in REC-09.
- Output: `pc2rec-r17*-frames.jsonl`, `-packets.jsonl`.
- Compare: rectangles / colours / positions against the OQs' spec tables; C→S bytes. Fold: close the UI OQs.

##### REC-18 [AUTO] NPC menus, waypoint panel and talk on arrival
- Settles: `ui/menus.md` OQ1 + OQ4 (Akara's menu, Charsi's menu and Kashya's hire list at 800 × 600: box position, item rows, highlight), `ui/menus.md` §1 / `ui/panels.md` §13 (the waypoint menu with each tab clicked and a row hovered), `world/npc.md` OQ7 / R-NV-4 (talk on arrival: stand 7–8 sub-tiles from Akara, one click, the character runs to her: expect one C→S 0x13, then S→C 0x27, 0x29, 0x28 in one frame when it stops, no second 0x13), `client/model.md` §7 r8 / §17 r6 part (b) and R2-39 NPC part (walk out of town once plainly, once right after starting an NPC talk: log the town flag `[0x007A5260]`, the interact NPC fields and the C→S 0x30), `intents-events.md` §3.5 (open a shop and the hire list: 0x58 codes, 0x4E), `client/msg-ui.md` §4–§11 NPC part (talk to Akara: 0x27 count and kinds; open and close the hire list: 0x4F, 0x4E × n, 0x50 code 2; same breakpoints as REC-17).
- Steps: `record_frames.py --seconds 90 --auto ScnSor --seed 1234 --draws-every 1 --input "wait 3; goto 1 148 40; wait 3; shot akara; key ESC; goto 1 154 40; wait 3; shot charsi; key ESC; goto 1 150 40; wait 3; shot kashya; key ESC; goto 2 119 300; wait 3; click 115 75; shot wp1; click 178 75; shot wp2; click 240 75; shot wp3; click 300 75; shot wp4; move 200 138; wait 1; shot wphover; key ESC; end"` (NPC classes Akara 148, Charsi 154, Kashya 150: check against `monstats`). Twin `record_packets.py` with `--input "wait 3; click <Akara draw point from the shot>; wait 8; key ESC; end"` for R-NV-4 and the leave-town-after-talk variant. NEW HOOK: the town flag and NPC fields.
- Output: `pc2rec-r18{a,b}-{frames,packets}.jsonl`, screenshots in `traces/raw/shots`.
- Fold: menus OQ1 / OQ4 / §1, npc OQ7, model §7 r8.

##### REC-19 [AUTO] Sound: request-site map of every UI control, one-shot lengths, async loads
- Settles: `audio/triggers.md` OQ12 / RA-T1 (UI control → §11 request site: `0x004B9A00` caller for ids 1–6, 15, 16; every site of §11's counts 24, 7, 4, 26, 1, 8, 1, 1 appears or is named unreached), R2-39 (a UI-sound request log clicking each control of `client/ui.md` §B8.1 once: popups, configure controls, stash / cube / trade buttons, NPC menu, text list; an NPC talk → cancel with a memory read of the NPC's unit flags, mode, store items in S: `model.md` §17 r1–r3; a left skill dropping to level 0: r4), `audio/sound-table.md` OQ12 (the tick at which one-shot sounds end: the request start tick and the tick the slot frees, for several known-length sounds, 50 ms thread `0x00516250`), OQ13 (ticks from request to playable of an `Async Only` load), `audio/triggers.md` OQ1 UI part (item pickup / drop / identify, the options sliders).
- Steps: `record_sound.py --seconds 400 --auto ScnSor --seed 1234 --input "<hover + click each mini-panel button, each panel, skill tree point spend, quest log, automap, party, message log, Esc menu, Options sliders; belt; run toggle; NPC talk → cancel; drop an item on an invalid spot / ground; swap; a trade window tab, buy, sell, repair>" -- -w` (sound on: the recorders default to `-w -ns`; pass `-- -w`; Master and Music volume at their defaults). Side log of each action with its wall-clock second (the script's `shot` lines serve). Hook `0x004B9A00` logging id, caller, unit, delay, flags, T, C; add `0x00516250` slot-free.
- Output: `pc2rec-r19-sound.jsonl`.
- Compare: one row per (control, action) → (id, caller); channel-end ticks vs sample length. Fold: triggers OQ12 / OQ1, sound-table OQ12 / OQ13, `model.md` §17.

##### REC-20 [AUTO] Sound and music by waypoint, wav dumps, the day change
- Settles: `formats/wav.md` OQ1 / R2-35 / C75 (dump at `0x00516760` (pointer, size) for the `wav.md` Test-vector files and one ADPCM file per channel count; the first 256 KiB of `music\act1\crypt.wav` from the refill copy `0x004157C0`: byte for byte with our decode), `audio/environment.md` OQ1 + entry 74 rest, partial (`0x004DCAA0`, `0x004DCD40`, `0x004E42E0`: town → Cold Plains → Catacombs 2 by waypoint, the music / ambient request at each level change, a day change in a 2100 s town sit; the Den and Blood Raven's death are REC-33), `audio/sound-table.md` OQ1 / OQ10 (the exact captures written in those open questions: read them first), `audio/triggers.md` OQ1 (waypoint activation sound).
- Steps: `record_sound.py --seconds 2200 --auto TestSor --seed 644409375 --input "wait 2; <Cold Plains by waypoint>; wait 20; <Catacombs 2 by waypoint (T-path via Jail 1, as D3)>; wait 60; <back to Rogue Encampment>; wait 1900; end" -- -w`; hooks as REC-19 plus the three environment addresses and the `0x00516760` / `0x004157C0` dumps.
- Output: `pc2rec-r20-sound.jsonl`, `pc2rec-r20-wav/` (dump files).
- Compare: dumps against `data-tool` decode; the request log against environment §10 r1. Fold: wav OQ1 / OQ2, environment OQ1, sound-table.

##### REC-21 [AUTO] x87 control word under each video mode
- Settles: `items/treasure.md` OQ5 (the control word at `0x0055A935` (treasure float math): 53-bit (CRT default) or 24-bit after the video layer: SetCooperativeLevel flags 0x11 / 0x411 lack FPU-preserve), `sim/stat-lists.md` OQ1 (the x87 control word at the §7.2 max-rescale call during a max-life change: add a prepared save with a +max life item to equip / remove at second 10; a D3D device may set 24-bit), `sim/pathing.md` OQ9 (the FPU control word at `0x0067A140`; or a Blessed Hammer's per-tick positions if the word is not enough).
- Steps: three runs, game args after `--`: `-w -ns` (DirectDraw), `-w -ns -d3d`, and Glide if a wrapper is installed: `record_tick.py --seconds 30 --auto ScnAma --seed 1234 --input "wait 15; end" -- -w -ns -d3d`. NEW HOOK: read the game thread's FPU control word from its thread context at the first tick (the kill needs no drop: the word is a thread property, read it at every tick marker; if the engine reloads it per treasure call, REC-08's drop confirms).
- Output: `pc2rec-r21-{dd,d3d,glide}-tick.jsonl`.
- Fold: treasure.md OQ5 states the precision; d2rs's float helper follows.

##### REC-22 [AUTO] The area-5 hook probes
- Settles: `tools/original-hooks.md` OQ1–OQ4 (walk injection at `0x0044F136`, forced start 0x67, seed override chains run twice, save-dir breakpoint `0x00534410`) and `tools/original-hooks-spawn.md` OQ1 (spawn at `0x0052FD1E` with `record_packets.py`): each OQ names its probe, `tools/original-hooks.md` OQ9 (breakpoint on `0x00552E6C` over a full scenario: log the allocating caller, the unit-seed fallback).
- Steps: run each probe as its OQ words it (`run_scenario.py --probe` covers the seed override); `--auto` where the recorder allows.
- Output: per probe, one line in the xpc report. Fold: close the OQ.

##### REC-35 [AUTO] DRLG extra probes: Trees substitution DS1, river units, room-entry returns
- Priority 2 (unattended; the Cold Plains walk of REC-04 can carry the same hooks).
- Settles: `formats/ds1.md` OQ3 + `drlg/preset.md` OQ2 (after the Trees substitution DS1 loads: its 14 group records, 0x18 bytes each, and the 0x320 bytes of slack after the file buffer), `drlg/outdoor-tilesub.md` OQ4 (the same Trees group records, +0x14 of each; OQ2 of that spec is already folded), `drlg/preset.md` OQ3 (a level whose river / navi units are added at first activation: the §8 adds against the §9 transfers per room), `drlg/rooms.md` OQ7 (the value `0x0066D820` returns for RNG sequence 6822–6835 of the RNG recording's run), `formats/ds1.md` OQ3 second half (a memory read of the slack bytes after the DS1 buffer, `0x00517079`, and which lvlsub group `trees.ds1`'s 14th group picks).
- Steps: `py tools/trace-recorder/record_rng.py --seconds 400 --auto TestSor --seed 644409375 --input "<T12>; <Cold Plains by waypoint>; wait 20; <Stony Field by waypoint>; wait 20; end" --out traces/raw/pc2rec-r35-rng.jsonl` (same character, seed and waypoint list as pc2rec-d3 so the sequence numbers 6822–6835 match; take the header `args` of that raw file). NEW HOOKS: a memory read of the Trees DS1's 14 group records and the 0x320 slack at the point its load finishes (names in `ds1.md` OQ3); INT3 at the §8 add and §9 transfer sites of `preset.md` logging room and unit; entry and return of `0x0066D820` with the RNG sequence number.
- Output: `pc2rec-r35-rng.jsonl` (+ the dump files beside it).
- Compare: `check_rng` passes; the group records against `ds1.md` §3; the add / transfer counts per room against `preset.md` §8 / §9. Fold: close ds1 OQ3, preset OQ2 / OQ3, tilesub OQ4, rooms OQ7.

##### REC-36 [AUTO] Load a character saved in Act III (client +0x1AC writes)
- Priority 2 (unattended once the save exists).
- Settles: `sim/path-placement.md` OQ8 (the writes of client +0x1AC in order, sites `0x0052FB97`, `0x00530E17` and the `0x00532690` path, while a character saved in Act III loads).
- Steps: prepared save `ScnAct3` (`d2s-tool new --waypoints all --quests acts=4`, then a `d2s-tool` edit to put the saved area in Act III's town, Kurast Docks; check `d2s-tool --help` for the field; read-only copy in `%USERPROFILE%\Saved Games\Diablo II`). Fallback if the edit is not possible: take `TestSor` to Act III with `<T12>; <T23>`, Save and Exit by hand (no `--auto`), then reload it. `py tools/trace-recorder/record_tick.py --seconds 60 --auto ScnAct3 --seed 1234 --input "wait 10; end" --out traces/raw/pc2rec-r36-tick.jsonl`. NEW HOOK: INT3 at each of the three sites, log old and new value of client +0x1AC, site, order.
- Output: `pc2rec-r36-tick.jsonl`.
- Compare: the order of the three writes against `path-placement.md` §11 / OQ8. Fold: close OQ8 or edit §11.

##### REC-43 [no game] Re-export Ghidra with labels
- Priority 2 (cheap, unattended; only needs the PC's Ghidra project).
- Settles: R2-46 last clause: re-export Ghidra with labels once `1.14d-notes` is on the PC.
- Steps: as `tools/ghidra/README.md`.
- Output: the export named there; one line in the xpc report.

##### REC-60 [AUTO] ui/text.md §15 r6 (scroll window, caret-inside-window case)
- SETTLED 2026-10-08 from the binary (pc1-s8): no recording needed; drop this entry.
- Priority P2.
- Settles: ui/text.md §15 r6 (scroll window, caret-inside-window case).
- Steps: `record_frames.py --seconds 40 --auto ScnSor --seed 1234 --draws-every 1 --input "wait 3; key ENTER; text <60 chars longer than the chat box>; key HOME; key RIGHT x5; key END; key LEFT x10; key SHIFT+LEFT x5; key BACKSPACE; key DELETE; key RIGHT x20; key ESC; end"` + NEW HOOK at `0x004FE7C0` exit logging E +0x25C, +0x4C, +0x50.
- Output: `pc2rec-r60-frames.jsonl` + hook log.
- Compare: §15 r5 / r6 per key: caret, selection ends, first / last window. Fold: write the result into the cited rule and drop its PROVISIONAL line.

##### REC-61 [NO RUN] render/camera.md OQ1 (roof block count
- Priority P2.
- Settles: render/camera.md OQ1 (roof block count; roof y multiple of 8) and OQ7 (wall-block file / block count; x, y ≡ 0 mod 32).
- Steps: (no game run) `mpq-tool extract d2data.mpq "data\global\tiles\*.dt1"` and the same for d2exp.mpq; keep the files a `LvlTypes` File column names (exclude the 6 unused); over orientation-15 tiles count tiles and blocks and list block y ∉ {0, 8, …, 64}; over orientations ∉ {0, 13, 15} count files and blocks and list blocks with x or y ≢ 0 (mod 32).
- Output: counts + exception lists.
- Compare: 13,432 vs 15,432 roof blocks; 104,780 / 251 vs 104,767 / 250; zero exceptions expected. Fold: write the result into the cited rule and drop its PROVISIONAL line.

##### REC-90 [AUTO] sim/path-placement.md §10 rule 1 (static-path unit passed to `0x00554EA0`)
- Priority P2.
- Settles: sim/path-placement.md §10 rule 1 (static-path unit passed to `0x00554EA0`).
- Steps: `py tools/trace-recorder/record_tick.py --seconds 300 --auto TestSor --seed 644409375` + NEW INT3 hook on `0x00554EA0` entry logging caller return address, unit type (unit +0x00) and path pointer +0x2C over a waypoint walk, a town portal, a Teleport cast and an item / object placement.
- Output: `traces/raw/pc2rec-r90-tick.jsonl`.
- Compare: no entry with unit type 2, 4 or 5 expected; if one shows up, write the 1.14d effect into §10 r1 and drop the d2rs static-set choice. Fold: write the result into the cited rule and drop its PROVISIONAL line.

##### REC-91 [NO RUN] formats/d2s-appearance.md §1 r2, §1 r3, Open question 3 (reference table `0x00744CA8`)
- SETTLED 2026-10-08 from the binary (pc1-s8): no recording needed; drop this entry.
- Priority P2.
- Settles: formats/d2s-appearance.md §1 r2 / Open question 3 (PROVISIONAL: slots 57–124 are `weap` slots, no other slot reserved; `ReferenceSlots::provisional_1_14d`).
- Steps: (no game run) read the 256 × 8 bytes at `0x00744CA8` of the 1.14d image; also local run queue item 99 (`D2_GAME_DIR=<install> cargo test -p d2-server --test character_save -- --ignored`, `token_positions_on_the_users_install`).
- Output: the 256 entries.
- Compare: the reconstruction (weapons 4–56, `cap` 57, `buc` 79, potions 125–134); a miss → write the table into §1 r2. Fold: write the result into the cited rule and drop its PROVISIONAL line.

##### REC-92 [MANUAL] formats/d2s-appearance.md §2 r1, Open question 4 (empty `alternategfx`)
- SETTLED 2026-10-08 from the binary (pc1-s8): no recording needed; drop this entry.
- Priority P2.
- Settles: formats/d2s-appearance.md §2 r1 / Open question 4 (PROVISIONAL: an empty `alternategfx` is compared like any code, matching the first unfilled entry).
- Steps: IT-6 save (`docs/handoff/pc2-rec-pc2-items.md`): a character holding an item whose `code` sits above the first hole (a throwing potion) in a hand; save; read the appearance bytes of the d2s header.
- Output: the save file's appearance block.
- Compare: the token written vs `AppearanceTable::lookup`. Fold: write the result into the cited rule and drop its PROVISIONAL line.

##### REC-93 [MANUAL] audio/triggers-2.md §14 r2 (chest at-once key sound sent twice)
- Priority P2.
- Settles: audio/triggers-2.md §14 r2 (PROVISIONAL: the chest's at-once send leaves +0x6E and flag 0x400 set, so the player's unit update of the same tick sends event 11 again).
- Steps: `py tools/trace-recorder/record_packets.py --seconds 120` while opening a locked chest with a key.
- Output: `traces/raw/pc2rec-r93-packets.jsonl`.
- Compare: count of S→C 0x2C event 11 in the opening tick (2 expected by the provisional reading, 1 otherwise). Fold: write the result into the cited rule and drop its PROVISIONAL line.

##### REC-94 [MANUAL] world/objects.md §7.1 r3 (interact range `0x00623660`)
- Priority P2.
- Settles: world/objects.md §7.1 r3 (PROVISIONAL: a player operator is always in interact range; `LocalSeams::object_in_range` in `crates/d2-client/src/app/single_player.rs`).
- Steps: `py tools/trace-recorder/record_packets.py --seconds 120` + NEW HOOK at `0x00623660` exit logging operator / object positions and the result, while clicking the Rogue Encampment waypoint from 1, 3, 5 and 8 subtiles away (walk interrupted by a second click) and a chest from the same distances.
- Output: `traces/raw/pc2rec-r94-packets.jsonl` + hook log.
- Compare: the distance at which the hook first returns 1 and whether S→C 0x63 follows the C→S 0x13. Fold: write the range test into §7.1 r3 and drop its PROVISIONAL line.

##### REC-117 [MANUAL] drlg/maze.md, drlg/rooms.md §9.5.1 (exit warps of maze levels, q-act1-dungeons)
- Priority P2.
- Settles: which maze/preset room carries each exit warp of Den of Evil, Cave, Tower, Crypt, Mausoleum, Barracks, Jail, Cathedral and Catacombs levels, and the tile position (PROVISIONAL in the synthetic world: `crates/d2-client/src/app/synthetic_maze.rs` puts the way back in the first room at sub-tile (20, 20); live levels depend on the DS1 `warp_unit` tables, which are untranscribed).
- Steps: record S→C 0x09 tile units and 0x07/0x15 on entering each level from the town and back.
- Output: `traces/raw/` packet logs.
- Compare: room and position of every warp tile per level.

##### REC-99 [MANUAL] sim/path-placement.md §12.2 (C→S 0x13 on a warp tile, tile unit creation)
- Priority P1.
- Settles: sim/path-placement.md §12.2 callers (`0x00548C32`: what the 0x13 tile case checks before `0x005550B0`), `client/model.md` §8 rule 7 (the interact sender lists no tile case; `ui/controls.md` §6 r9.2 sends 0x13 (T, g) for a tile), and where 1.14d allocates the tile units from the type-5 presets (PROVISIONAL: every active room's type-5 presets become tile units once, `View::spawn_warp_tiles` in `crates/d2-sim/src/wiring/action/warp_tile.rs`; the 0x13 tile case is `View::warp_tile_message`, no range test).
- Steps: `py tools/trace-recorder/record_packets.py --seconds 120` while clicking the Blood Moor cave entrance (Den of Evil) from 1, 3 and 8 subtiles away, once with the walk interrupted by a second click, plus a hook at `0x00548C32`.
- Output: `traces/raw/pc2rec-r95-packets.jsonl` + hook log.
- Compare: the C→S 0x13 (type, GUID) the client sends for a tile and when (on arrival or at once); the S→C 0x09 tile units of the room at join; the result of the warp (S→C 0x07, 0x15, 0x0D). Fold: write the tile case into §12.2 and drop the PROVISIONAL lines.

##### REC-50 [NO RUN] client/stat-lists.md §6 r6.5 setfunc 15 (+0xC4 bit 0x80000000 reader), client/model.md §14
- Priority P2.
- Settles: client/stat-lists.md §6 r6.5 setfunc 15 (+0xC4 bit 0x80000000 reader), client/model.md §14 r3 / OQ10 (reader of pet record +0x1C).
- Steps: (no game run) Ghidra scan of client code 0x00400000–0x0051FFFF with `py tools/ghidra/disasm.py`: every read of unit +0xC4 followed by a sign test (`js`/`jns`/`test reg,reg`, `shr 0x1f`, `and 0x80000000`) and every read of [record+0x1C] where the record comes from the pet list `[0x007BB5BC]` (walkers 0x00478E40 / 0x00478F20 and callers).
- Output: a list of reading functions and their conditions.
- Compare: none found → the provisionals stand; otherwise write each reader's rule into the spec. Fold: write the result into the cited rule and drop its PROVISIONAL line.

#### Priority 3 (manual / assisted)

##### REC-23 [MANUAL] Vendors: prices, equip-on-buy, refresh, gamble, copy rebuild
- Settles: `world/vendors.md` OQ1 + OQ6 (R-NV-5: magic / rare / unique sell prices and buy-back vs §9.2 rules 2–9; term B uses stat "bonus" `0x00625560`: the same steps under `record_stats.py`), OQ3 (R-NV-6: arrows to the equip slot with a bow, to the inventory with a crossbow; a cap not auto-equipped, §7.1.1), OQ4 (R-NV-7: a new 0x9C action 11 list with new GUIDs and NPC-control seed draws between two opens across a level change), OQ7 (R-NV-8: Elzix gamble: 14 items, first `rin`, second `amu`; §5.1 draws; purchase 0x32 transaction 2; 0x37 identify; re-open), OQ8 (R-NV-9: ethereal socketed weapon and a low-quality throwing weapon sold and bought back: stats 20–24, 67, 68, 159, 160, durability field by field).
- Steps: per R-NV-5 … R-NV-9 as in `pc2-rec-npc-vendors.md` (five sittings of 180–300 s: packets; 5 and 7 and 8 also under `record_stats.py` / `record_rng.py` with the same steps; Act I save with ≥ 50,000 gold, magic / rare / unique items; Act II save level ≥ 20 with ≥ 100,000 gold).
- Output: `pc2rec-r23-*`. Fold: close vendors OQ1 / 3 / 4 / 6 / 7 / 8.

##### REC-24 [MANUAL] NPC services
- Settles: `world/npc.md` OQ6 rest (R-NV-2: Akara heal: C→S 0x13 then 0x2F, 0x1D–0x1F in that frame, sound-10; Cain identify of 3 items: C→S 0x34, one 0x2A code 3, gold −300; with none: 0x2A code 9; R-NV-3: imbue (Charsi), socket (Larzuk), personalize (Anya), respec (Akara): C→S 0x38, S→C 0x58 result 6, the item 0x9C order, socket count = `MaxSock` cap; act travel a–d (resurrect is REC-10)).
- Steps: `pc2-rec-npc-vendors.md` R-NV-2 and R-NV-3 (saves A–D: A: Act I Tools of the Trade reward pending, a socketless Long Sword; B: Act V Siege done, a Crystal Sword; C: Act V Betrayal done, a rare body armour; D: Hell character with the Den reward unused). `record_packets.py --seconds 240` / 600 s per save.
- Output: `pc2rec-r24-*`. Fold: npc OQ6.

##### REC-25 [MANUAL] Cube and the cow portal
- Settles: `world/cube.md` OQ1 / OQ2 / OQ4 (R-NV-10: the transmute frame messages with callers and tick pass: input removal 0x9C / 0x0A, output 0x9C, sound event 4; two game-seed steps per created item; run A V1 three Chipped Amethysts; run B V22 with Wirt's Leg + Tome of Town Portal in the Rogue Encampment (Baal killed in Normal, the Cow King never killed) the portal object's allocation and init-12 draws; run C record 19 `pole,mag,pre=191`), `quests.md` OQ3 / R-PQ-2 (the cow portal: draws between C→S 0x4F 0x18 and the portal's S→C 0x51; object class / mode).
- Steps: `pc2-rec-npc-vendors.md` R-NV-10; `record_packets.py` + `record_rng.py` per run (300 s each, `check_rng`).
- Output: `pc2rec-r25-*`. Fold: cube OQ1 / 2 / 4, quests OQ3.

##### REC-26 [MANUAL] First waypoint activation
- Settles: `world/waypoints.md` OQ2 / R-NV-12 (a new Normal character without the Cold Plains waypoint: the first click lights it (no 0x63; 0x0E / 0x51 mode 1), mode 2 reached after frame + 15 + 1 with no message; the second click opens the menu: S→C 0x63 with the Cold Plains bit).
- Steps: R-NV-12 in `pc2-rec-npc-vendors.md`: `record_packets.py --seconds 180` by hand from the Blood Moor into Cold Plains.
- Output: `pc2rec-r26-packets.jsonl`. Fold: waypoints OQ2.

##### REC-27 [MANUAL] Act II run (and the orifice)
- Settles: `world/quests-act2-2.md` OQ1 / R2-41 / R-PQ-9 (orifice: operate → 0x58 result 0; cancel → C→S 0x44 action 2 → result 1; a wrong cursor item → result 4; the staff → result 5, byte 6 = 1: the 7 bytes of each, byte 6 for results 0, 1, 4; `quests-act2.md` OQ31), `quests-act2.md` OQ10 / OQ2 / OQ3 / R-PQ-8 / R2-19 (Jerhyn, Drognan, Atma; Radament; scroll and cube chests: gate mode 1 / 2 and end-animation frame per chest §1.3; Claw Viper Temple altar `53 05000000 00000000 01` at the start and `53 02000000 00000000 00` at the altar; Horazon's journal; the Summoner; the staff; Duriel; Tyrael's portal; Meshif's completion order), `quests.md` OQ10 for the Warriv / Meshif / Tyrael act changes (REC-05 covered the waypoint ones).
- Steps: `record_packets.py` + `record_rng.py` per sitting (300 s each) on an expansion character entering Act II on Normal with no Act II quest done; a `d2s-tool` save per stage.
- Output: `pc2rec-r27-*`. Fold: close the Act II OQs.

##### REC-28 [MANUAL] Act III run
- Settles: `quests-act3.md` OQ8 / R-PQ-10 / R2-20 (the 19 "Readings taken literally" of `impl-quests-act3`: Lam Esen's tome, the Golden Bird / Alkor potion, Gidbinn, Khalim's parts and flail, the orb's two hits, the council kills' 0x5D status bytes, Mephisto's timer and the Hellgate, Natalya's spawn, the Dark Wanderer's 7 / 8 minions; chest gold and item draws §4; the Alkor 564 broadcast needs a second player: [NO] unattended, BLOCKED unless a TCP/IP host plus two clients is available), `drlg/outdoor-act3-act5.md` OQ1 creation draws already folded (D1), `quests-act3.md` OQ1 (open: read its own capture).
- Steps: `pc2rec-rec-pc2-quests.md` R-PQ-10. Output: `pc2rec-r28-*`.

##### REC-29 [MANUAL] Act IV run
- Settles: `quests-act4.md` OQ13 / R-PQ-11 / R2-21 (Izual and the ghost, the Hellforge drops, the five seals, Diablo's spawn and death in a classic game and an expansion one, Tyrael's portal object 566; the classic game is [NO]: see the last section), `quests-act4.md` OQ2 / QD-1 / R-PQ-12 (classic Diablo kill with frames: whether the 2250 / 2375-frame offsets drift with wall-clock time; idle and loaded machine; the uncredited player's 0x50 bytes 3–14: OQ3).
- Steps: expansion part by hand (`record_packets.py` + `record_rng.py`, 300 s per sitting); classic part BLOCKED (see the last section).
- Output: `pc2rec-r29-*`.

##### REC-30 [MANUAL] Act V run and the Act V levels
- Settles: `quests-act5.md` OQ7 + `quests-act5-2.md` OQ7 / R-PQ-13 / R2-22 (Shenk and the socket reward, rescue portals and the rune draw, Anya's thaw and item draw `0x00558200`, Nihlathak's portal, the Ancients' reset and experience, Baal's gold draws, missile 625, the zoo id 0x50), `drlg/outdoor.md` OQ9 (levels 109–132) and `outdoor-act3-act5.md` OQ3 part 2 + the 111 / 112 / 117 builds (R2-30).
- **Not reachable unattended**: `TestSor` has no Act V tab. Needs a character that has Harrogath: a played-through save, or a `d2s-tool` save with the Act V quest bits and waypoint flags; then `record_rng.py` + `dumpdrlg` for each Act V level.
- Output: `pc2rec-r30-*`.

##### REC-31 [ASSISTED] Skill, monster and missile bodies, umods
- Settles: `skills/bodies-3.md` OQ1–OQ4 (Fetish Shaman / Baal Inferno E param 1, timers, missile frames per do of `0x005CC4E0`; Greater Mummy resurrect: T mode, skill, life after `0x005CCB10`; Sand Leaper jump `0x005CB940`; Sand Maggot egg cast `0x005CAFA0`), `skills/bodies-4.md` OQ1–OQ3 (Royal Strike / Claws of Thunder charge release: path points and seeds `0x005D4870`, `0x005D4150`; Overseer whip `0x005D1F70`; imps on a barricade tower `0x005D1AB0`), `monsters/umod-callbacks.md` OQ1 + OQ2 (a fire-enchanted unique dying beside the player, a suicide minion hit into GH in Act V: timer sets / runs (type 7, site), rng draws with caller, area-damage targets), OQ6 (stat list of a missile fired by a spectral-hit unique: is stat 12 present), OQ7 / §28 (client side of a fire-enchanted unique's death: `0x004CD540` creates, `0x004B9A00` id 2,458, mode / +0x44 per update), `missiles/bodies.md` OQ1 / OQ3 (Plague Javelin hit: cloud positions per tick), R2-24 (Might in a party, Kick / Bash, a Druid summon, a Clay Golem: SK-*), R2-25 (the missile list of `impl-missile-bodies-2` §5.3: MB-1…MB-6), R2-23 (Act II–V monster AI: moved to REC-40), `monsters/ai.md` OQ1 (cold damage: REC-40), `skills/descriptions.md` OQ1 + OQ2 (character panel damage / attack-rating lines for one skill per `descdam` 1–24 and `descatt` 1–5; kick `v-(v+1)`, smite, dual-wield assassin for `descatt` 5; stats 0, 2, 17–25, 48–59, 119, 159, 160; the title string of a skill whose skilldesc `str name` ≠ `str alt`: `record_frames.py` with the panel open), `monsters/init.md` §27 (kill a fetish shaman in Act III: the `0x00574370` call, class 278–282 → 141–145, mode 1, and its rng draws with callers).
- Steps: `spawn.py` (q9-rec branch: places monsters by debugger call, foreground input) with `record_tick.py` / `record_rng.py` / `record_packets.py` / `record_frames.py` per OQ; the hook addresses are the ones in the OQ text. Act V items (GH) need an Act V save (REC-30).
- Output: `pc2rec-r31-*`. Fold: close each OQ.

##### REC-32 [MANUAL] Weather, translucent walls, arcane, Den light
- Settles: `render/draw-order.md` OQ7 / R2-1 (the 8 `townN1` river-bank cells at `TownE1` tile (950, 933): off-screen from every reachable spot per `local-buddy-q9-rec` R2-1: try a screen-edge map position; BLOCKED otherwise), OQ6 / R2-4 (a `Logicals` preset walk, levels 18 and 19: the group-mode fade), `render/draw-order-2.md` OQ1 / OQ9 / R2-2 (Rogue Encampment in rain: per frame the player seed, rain target `[0x007A89E0]`, `k` `0x007C8A38`, `last_s`, the splash pool count; splashes only when `r` < ⌊target × 1000 / 256⌋), R2-3 (Arcane Sanctuary / Arreat Summit with the two seed globals; Act V part REC-30), `render/blend-modes.md` OQ6 / RA-B4 / R2-5 (rain and Arcane-star line endpoints: 45° lines x-major, end one short on the minor axis), OQ2 / RA-B2 (Lut Gholein translucent walls: walk behind a building towards the palace; alpha byte and pixels per frame), `render/lighting.md` OQ5 / RA-L1 (new Amazon in the Den of Evil: S→C `89 00` when the last monster dies, then the Den's ambient; `record_packets.py` + `record_frames.py` in two sittings), `render/unit-composite.md` OQ4 + OQ7 (a monster's first attack after a mode change to a not-yet-loaded mode: is a draw made before the graphics-ready flag; a bone prison, a leaping unit and a missile with creation flag 0x100: motion record gfx +0x30, 0x4C bytes), `render/camera.md` OQ1 (a roof in view), `composition-0001`, `frames-raw-2` (`camera.md` OQ5 / OQ8, `capture.md` OQ5 / OQ8), `sprite-placement.md` OQ5 (a GT / THS1 monster), `sim/pathing.md` OQ1 / OQ2 (a position trace), `world/quests-status.md`-style packets for 0x50.
- Steps: `record_frames.py --draws-every 1` (rain needs weather on: wait for a rain period or re-run; `weather.rain` = 1 in the frames) per OQ; `spawn.py` for the unit-composite probes.
- Output: `pc2rec-r32-*`.

##### REC-33 [MANUAL] Audio in fights and the Den, with sound on
- Settles: `audio/triggers.md` OQ1 (full list: two floor materials, a melee and a caster fight with a death and a block, an NPC greeting and talk, a waypoint, a Leap, a fire-enchanted unique's death), OQ3 (a write watch on unit +0xB0, last-hit class, during a fight: who writes it and when), `audio/environment.md` OQ1 + entry 74 rest (town → Blood Moor → Den of Evil → town, a day change, Blood Raven's death stinger 34, rain on and off), `audio/sound-table.md` OQ1 / OQ10 in full, R2-34 (ST-3, ST-4, ST-9, ST-10 (x87 vs `log2` / `powf`), TR-4, EN-*).
- Steps: `record_sound.py -- -w` by hand, side log with wall-clock seconds; REC-19 / 20's hooks. Output: `pc2rec-r33-sound.jsonl`.

##### REC-34 [MANUAL] Save and item round trips (no `--auto`)
- Settles: `formats/d2s-load.md` OQ3 / `d2s.md` OQ5 / IT-3 (item index stability across Save and Exit: A / B / C copies, the hotkey words +0x38..+0x87), `d2s-appearance.md` OQ1–OQ2 / IT-6 (+0x88..+0xA7 after each of: helm `cap`; `brs` / `plt`; a `transformcolor` magic item; set / unique `chrtransform`; gem-socketed normal; crossbow `lxb`; circlet; Enchant active), R2-45 / `d2s.md` OQ17 (one character saved with weapon, shield, helm, body armour, belt, gloves, boots, and again with a different set), R2-37 / R2-38 (1.14d saves round trip: header +0x10..+0x37, +0x88..+0xA7, stats at 0x2FD, `jf` / `kf`; `d2s-tool` characters load and re-save: quest bits, trailer bit, flag 0x2000: DS-2, DS-4, DS-5), `d2s.md` OQ15 / IT-7 (Iron Golem item: `kf` g = 1, an item record, a skill-90 cast at entry `0x005394A0`), `d2s-load.md` OQ2 / IT-8 (an equipped stale runeword: where the item ends, flags 0x20), `items/properties.md` OQ4 rest / IT-10 (break `0x0062BFBA` during load `0x0056AD78` and save `0x0063087E`; the 6 slots at ebp−0x2C), `items/treasure.md` OQ12 / IT-11 (capture only if seen), `world/quests.md` OQ14 / R-QC-1 / R-PQ-5 / R2-39 (a 1.14d expansion character that completed every Normal quest, saved right after the last one and after one more game: `d2s-tool dump` both quest sections; per slot the set bits; bits 13 / 14 gone in the second; DS-4 per-path bits), `formats/d2s.md` OQ4–OQ14 and OQ16 (the open ones not listed above, and the `d2s` lines of `xpc-to-pc2.md`: read each OQ's own capture; OQ1 and OQ2 are Deferred).
- Steps: by hand, `tools/d2s_check.py` before / after, saves in `%USERPROFILE%\Saved Games\Diablo II`. Output: the saves' hashes and the check results in the xpc line.

##### REC-37 [MANUAL] Animation-rate records: IAS / FCR / FHR / FBR / FRW
- Priority 3 (manual play; prepared saves).
- Settles: `sim/units.md` §4.7 (OQ1, OQ2 answered statically: unit +0x4C, +0x3C and the §4.2 start index per `anim` record for a player with IAS / FCR / FHR / FBR / FRW items in every mode (a dual-wield Assassin or Barbarian, a were-form, a Holy Shield block) and for a monster walking, running, attacking, casting and knocked back).
- Steps: prepared saves (`d2s-tool`) `AnimBar` (dual-wield Barbarian with IAS / FRW items), `AnimAss` (dual-wield Assassin), `AnimDru` (were-form), `AnimPal` (Holy Shield). Blood Moor, one sitting each: `py tools/trace-recorder/record_tick.py --seconds 240 --out traces/raw/pc2rec-r37-<save>-tick.jsonl` (0.2.0 `anim` records); run, attack, get hit, block; write the actions with wall-clock seconds.
- Output: `pc2rec-r37-*-tick.jsonl`.
- Compare: `check_tick`, `check_units` pass; the start index and +0x4C per record against `units.md` §4.2 / §4.7. Fold: close OQ1 / OQ2.

##### REC-38 [MANUAL] Monster init and umods: uniques, ancient barbarians, warping shrine
- Priority 3 (manual play; the Act V parts need REC-30's Act V save).
- Settles: `monsters/umod-init-bodies.md` OQ1 (0xAC assign + stat messages of a lightning, cold or mana-burn unique with minions: expect the §2 values; minions on Normal get no damage stats, cold minions still get coldlength), `monsters/init.md` OQ10 (a unique's client name draws), OQ4 / §4.1, §14.3, §26 (rng hook with caller addresses during one Act V ancient-barbarian spawn: four item creations after the boss mods; and one warping-shrine use: the §17 draws on the chosen monster). The classic-SP 0x67 part of `init.md` OQ1 is [NO]; the population pass of OQ4 is in REC-07.
- Steps: `record_packets.py --seconds 300` + `record_rng.py --seconds 300` on the same character: Normal for the minion stats, then a Nightmare character for the unique kinds; find the uniques by their overhead name in the Cold Plains / Stony Field / Dark Wood area and note the second of each sighting. Ancient barbarians: Arreat Summit (REC-30's save). Warping shrine: any act, reroll games with `--seed` until a room has one (not deterministic: record the seed that worked).
- Output: `pc2rec-r38-{packets,rng}.jsonl`.
- Compare: `check_rng`, `check_packets` pass; 0xAC / stat bytes against `umod-init-bodies.md` §2; draws against `init.md`. Fold: close the OQs.

##### REC-39 [MANUAL] Necromancer pets and the 0x7A / 0x99 messages
- Priority 3 (manual play; one Necromancer save).
- Settles: `sim/pets.md` OQ3 count part (summon a 4th skeleton over max 3: expect three 0x7A action 0 for the removed one, §6 table) and §10 (summon two pet types of one group, then lower a `petmax` skill level so the resync trims; leave the game with a hireling: the free path, 0x7A removes), `client/msg-skills.md` OQ4 / `client/msg-ui.md` OQ7 (a pet summon with a pending skill: does any later message of the same receive change the 0x99 unit: 0x0A, 0x15, 0xA8 / 0xA9 state 118).
- Steps: `d2s-tool` Necromancer `NecPets` (Raise Skeleton and Skeleton Mage so max is 3, Clay Golem not needed, a Rogue hireling). `record_packets.py --seconds 400` and `record_tick.py --seconds 400` over the same actions in the Blood Moor: summon three skeletons then a fourth; summon a skeleton and a mage; respec one point out of Raise Skeleton at Akara; leave the game (Save and Exit) with the hireling alive. Share the sitting with REC-40's `ai-bodies-6.md` logs.
- Output: `pc2rec-r39-{packets,tick}.jsonl`.
- Compare: `check_packets`, `check_tick` pass; 0x7A order against `pets.md` §6 / §8. Fold: close pets OQ3, msg-skills OQ4, msg-ui OQ7.

##### REC-40 [MANUAL] Monster AI think logs (acts, Nightmare, freeze / knockback, NPCs)
- Priority 3 (manual play; the acts II-IV area runs can be `--auto` with a strong prepared save, the Act V areas need REC-30's save).
- Settles: `monsters/ai.md` OQ1 / OQ2 / OQ3 (freeze a monster with cold and knock back a fallen and a sand leaper; play one Nightmare area; log type-2 schedules (site, frame), timer type 12, state 1 on / off, mode changes. Expect a think at freeze apply + len + 1 and one at expiry + `aidel` (`0x0057B170`); knockback end +1 / 15 / gethit (`0x005A8520`); Nightmare mode-end delays = `aidel(N)` (zombie1 14)), `monsters/ai.md` OQ8 / OQ9 / OQ11 (a town walk clicking NPCs: C→S 0x13, 0x59; an Act I fight with a fallen shaman: command 4 next to each NPC think, every client message next to type-2 schedules, mode changes of fallens), `sim/units.md` OQ7 (answered statically: single player has game +0x6A = 3: a Nightmare single-player game, log monster event-2 sets (U10), expect the Nightmare `aidel`, 0 → 15), R2-23 / `monsters/ai-bodies-2.md` OQ1 / OQ2, `ai-bodies-3.md` OQ1 / OQ2, `ai-bodies-4.md` OQ1 / OQ2, `ai-bodies-5.md` OQ1 (one run per act II–V: Far Oasis, Arcane Sanctuary, Spider Forest, Durance of Hate, Chaos Sanctuary with Diablo, Arreat Summit, Worldstone Chamber; vultures, bat demons and frog demons also modes 8–11 / 14 and collision; for Diablo the mode chosen per think with the player's resistances), `monsters/ai-bodies-6.md` OQ1 (a necromancer game with golem, skeletons and a skeleton mage, a hireling following and fighting, an Act II town walk, a MinionSpawner, an EvilHole and a desert turret), `monsters/ai-bodies-7.md` OQ1 (arrow / poison / nova traps, the Act II palace guard before and after the door opens, the Dark Wanderer, druid summons (wolves, bear, ravens, vines), assassin shadows) and OQ2 (Uber Tristram: for Uber Mephisto, Diablo and Baal log the AI control's function (+0x04) and special state (+0x00) after creation and at each think, every type-2 schedule, every mode change with its caller and every `0x005B0E00` call on them; expect no attack or skill mode started by AI code, only gethit / knockback / death).
- Steps: `record_tick.py --seconds 240` + `record_rng.py --seconds 240` per area, same character and seed; per think log the type-2 schedule frame, the unit-seed steps with caller, the mode requests (mode, target, point) and AI params 0–2. Reach acts II–IV by waypoint (`<T12>`, `<T23>`, `<T34>`, then the area's own waypoint entry); Chaos Sanctuary and the Uber areas need quest / key progress: a `d2s-tool` save. A character strong enough for Nightmare areas (prepared save `AiNm`). Uber Tristram: a Hell character with the keys and organs, portal from Harrogath (REC-30's save).
- Output: `pc2rec-r40-<area>-{tick,rng}.jsonl`.
- Compare: `check_tick`, `check_units`, `check_rng` pass; each think delay against the `ai.md` / `ai-bodies-*.md` tables. Fold: close the OQs; the AI-n rows named in them.

##### REC-41 [ASSISTED] Skill use and level hooks
- Priority 3 (assisted: `spawn.py` places monsters; prepared saves per skill).
- Settles: `skills/use.md` OQ3–OQ7 (hook `0x0056FAF0` entry / return and `0x0056F7F0` entry: cast a non-`TargetAlly` skill on a party member, does the do run after start returned 0; `0x0056BFE0` for Teleport at level ≥ 25 with < 1 mana: cast free?; two C→S 0x06 two frames apart: does the second restart A1; `0x005A7670` (arg1, arg2, unit +0x4E) on monsters; arg1 of `0x005539B0` per type-0 timer during Strafe / Zeal), `skills/levels.md` OQ1 (hooks `0x00646460`, `0x00644D50` / `0x00644E40` entry / return with known skill levels, and `0x0056BFE0` mana before / after for a few skills), §7.5 / §7.6 (no OQ, unverified: equip an aura item, e.g. Dragon, and a charged item: log the `0x005BF510` / `0x00647320` calls and the type-9 timers they schedule), `skills/bodies-2.md` OQ1–OQ5, OQ7, OQ8, OQ12 (Jab / Smite monsters: stats 21, 22, 19 and element stats around `mode_damage`; Dragon Talon L6 / L12: kicks, last-kick knockback, E param 1; Find Potion per act and difficulty: codes, seed draws; Leap and monster Leap: E flags, landing frame, knockback, 0xA5; Shock Field: caster seed before / after; Conversion on a higher-level monster: stats 12, 6, 7 during and after; Holy Freeze pulses: state 107, target seed; Whirlwind with one and two weapons: E param 4, hits per do). `skills/bodies.md` OQ1–OQ3 / OQ9 are in REC-31.
- Steps: one prepared save per skill group (`d2s-tool`; skills at the levels named); `record_tick.py` / `record_rng.py` / `record_stats.py` per OQ with the hook addresses above. The party-member cast needs a second player: mark it BLOCKED unless a second client joins.
- Output: `pc2rec-r41-*`.
- Compare: the logged values against each OQ's expectation. Fold: close each OQ.

##### REC-42 [MANUAL] Combat on-hit events (knockback, freeze, slow, skill-on-hit, mana)
- Priority 3 (manual play; prepared saves with the items).
- Settles: `combat/events.md` OQ1 (items with knockback, freeze, slow, skill-on-hit and damage-to-mana; Energy Shield, Bone Armor and Iron Maiden in play: log each event function's entry / return (table `0x007325B0` targets), H's seed before / after, and the record).
- Steps: prepared saves `EvtItems` (one item per property), `EvtSor` (Energy Shield), `EvtNec` (Bone Armor, Iron Maiden); Blood Moor fights, `record_rng.py --seconds 300` + `record_tick.py --seconds 300` with INT3 on the table targets (the hook list is in `combat/events.md`).
- Output: `pc2rec-r42-{rng,tick}.jsonl`.
- Compare: per event the record bytes and the seed before / after against `events.md`. Fold: close OQ1.

##### REC-45 [MANUAL] Baal's portal opening (level 131 to 132)
- Priority 3 (manual play; needs REC-30's Act V save at the Worldstone Chamber).
- Settles: `world/objects-client.md` §26.15 (does the generic step reach mode 2 itself at the end of mode 1: the object speed +0x4C in mode 1 and the frame of the mode-2 change, for Baal's portal opening, level 131 → 132; REC-30 covers the Act V levels but not this object).
- Steps: an Act V save with the Worldstone Chamber reachable (`d2s-tool` quest bits); `record_packets.py --seconds 300` and `record_tick.py --seconds 300` on the same actions: finish the Baal wave quest, wait for the portal to open, read the object's S→C 0x51 mode and the +0x4C speed per client update (NEW HOOK: log the object's +0x4C and mode at each client update).
- Output: `pc2rec-r45-{packets,tick}.jsonl`.
- Compare: the tick of the mode 1 → 2 change against the speed and the frame rule of `objects-client.md` §26. Fold: close §26.15.

##### REC-46 [MANUAL] Classic single-player start (C→S 0x67 of a classic character)
- Priority 3 (manual play; the forced start makes an expansion game and refuses a classic character, so it cannot be `--auto`).
- Settles: `client/model.md` §7 r9 PROVISIONAL (the creation flags u32@0x27 a classic character sends; expected 0x00000004, the builder's default without bit 20), and the classic game type and flags the server stores (`sim/intents-events.md` §2.5, §8.1; `monsters/init.md` OQ1 is answered statically).
- Steps: a new classic character made in the menu (Single Player → classic), start it at Normal; `record_packets.py --seconds 60` plain from the menu through the first frames in town; Save and Exit.
- Output: `pc2rec-r46-packets.jsonl`.
- Compare: the client_out 0x67 (seq 1) bytes @0x11–@0x14, @0x25, @0x27, @0x2B–@0x2D against §7 r9's table; S→C 0x01 u32@2 = u32@0x27 & 0x3179C7 and u8@6 = 0 (`intents-events.md` §8.1). Fold: replace the PROVISIONAL in `client/model.md` §7 r9.

##### REC-47 [MANUAL] Automap in the play preview (draw modes, header, markers)
- Priority 3 (manual play; the preview's automap is d2rs-own, unverified).
- Settles: `ui/automap.md` automap-0001 / automap-0002 (the cel draw modes of §10 r4, the clip rectangle and origin of §9) against the preview, which draws every cel opaque, no header or name text, and only the local player's marker (`docs/handoff/q-automap.md`).
- Steps: New character, Blood Moor, walk 30 s, Tab (full), screenshots; fade options 0–3; options → mini.
- Output: screenshots / `--draws-every 1` trace.
- Compare: cel draws (order, X, Y, mode) and marker lines vs §10 and §11.

##### REC-102 [MANUAL] items/inventory-moves.md §7.17 (potion use from the belt: item-use spec `0x005BF240` unwritten)
- Priority 3 (manual play). PROVISIONAL, `claude/q-belt-ui`: a belt healing potion attaches a `healthpot` (state 100) list with stat 74 for 100 frames; a mana potion a `manapot` (106) list with stat 26; `rvs` / `rvl` restore 35 % / 70 % of both at once. Amounts (hp 45/90/150/270/480, mp 30/60/120/225/450) and the duration are d2rs-own, unverified (`d2-sim/src/wiring/inventory/potion.rs`).
- Steps: new Amazon in town, take damage, press belt key 1–4; `record_packets.py --seconds 60` plain.
- Compare: the S→C 0x95 life/mana bytes per tick (and 0x9D removal) against the PROVISIONAL list; fold the real amounts, duration and message order into a new item-use spec.

##### REC-135 [MANUAL] items/inventory-moves.md §7.17 (stamina potion `vps`: item-use spec `0x005BF240` unwritten)
- Priority 3 (manual play). PROVISIONAL, `claude/q-stamina-run`: a stamina potion attaches a state 136 list with stat 28 (`staminarecoverybonus`) 1000 for 250 frames, the shrine's values (`wiring/inventory/potion.rs`); the client's run prediction walks while the model's stamina (stat 10) is 0 (`bridge/predict.rs`).
- Steps: new character outside town, run until the bar is empty (the run turns to a walk), drink a `vps` from the belt; `record_packets.py --seconds 60` plain.
- Compare: S→C 0x96 / 0x95 stamina per tick against the PROVISIONAL list and the tick the run turns into a walk; fold the real amount and duration into the item-use spec.

##### REC-200 [MANUAL] Front end: Game exit target
- Priority 3 (front-end screens; a capture of the 1.14d menus, no game run needed for most).
- Settles: `ui/frontend-menus.md` PROVISIONAL REC-200.
- Steps and compare: Game exit target. Capture: in single player, Save and Exit from a game. Steps: hook `0x0044B8A0` return and the next screen builder called (`0x0043B080` vs `0x004336C0`). Settles: which screen return value 4 maps to (expected character select).
- Fold: write the result into the cited `ui/frontend-menus.md` rule and drop its PROVISIONAL line.

##### REC-201 [MANUAL] Front end: Control draw order
- Priority 3 (front-end screens; a capture of the 1.14d menus, no game run needed for most).
- Settles: `ui/frontend-menus.md` PROVISIONAL REC-201.
- Steps and compare: Control draw order. Capture: break in the D2Win list draw while the main menu shows; log the control order (types 2, 3, 3, 6…). Settles: creation order = draw order (logo over background, buttons over logo).
- Fold: write the result into the cited `ui/frontend-menus.md` rule and drop its PROVISIONAL line.

##### REC-203 [MANUAL] Front end: Character-select OK condition
- Priority 3 (front-end screens; a capture of the 1.14d menus, no game run needed for most).
- Settles: `ui/frontend-menus.md` PROVISIONAL REC-203.
- Steps and compare: Character-select OK condition. Capture: press OK with a new character and with one that finished Normal; log whether `0x00439780` or `0x00434A00` runs and the session values compared at `0x00439A79`–`0x00439A97`. Settles: when the difficulty popup appears (owned by part 2 if it covers character select).
- Fold: write the result into the cited `ui/frontend-menus.md` rule and drop its PROVISIONAL line.

##### REC-204 [MANUAL] Front end: Mouse wheel on character select
- Priority 3 (front-end screens; a capture of the 1.14d menus, no game run needed for most).
- Settles: `ui/frontend-menus.md` PROVISIONAL REC-204.
- Steps and compare: Mouse wheel on character select. Capture: 1.14d, ≥ 11 characters, wheel up / down over the list and over the scroll bar; log `[0x00779DC8]` and `[0x0070CC00]` per event (hook `0x00439DF0`, `0x00439E90`). Settles: whether the wheel scrolls (`first` changes) and by how much, or nothing.
- Fold: write the result into the cited `ui/frontend-menus.md` rule and drop its PROVISIONAL line.

##### REC-205 [MANUAL] Front end: Dead hardcore figure
- Priority 3 (front-end screens; a capture of the 1.14d menus, no game run needed for most).
- Settles: `ui/frontend-menus.md` PROVISIONAL REC-205.
- Steps and compare: Dead hardcore figure. Capture: a hardcore save with status 0x0C (male and female class); screenshot the slot; hook `0x005066C0` args. Settles: what class' 8 / 9 draws (token, mode, palette) and the draw flags from `0x006CE278`.
- Fold: write the result into the cited `ui/frontend-menus.md` rule and drop its PROVISIONAL line.

##### REC-206 [MANUAL] Front end: Legacy `Save Path` migration
- Priority 3 (front-end screens; a capture of the 1.14d menus, no game run needed for most).
- Settles: `ui/frontend-menus.md` PROVISIONAL REC-206.
- Steps and compare: Legacy `Save Path` migration. Capture: registry with `Save Path` → a folder with saves, no `NewSavePath`; start the game; read `NewSavePath` after; repeat with an empty folder. Hook `0x00406DE0` return. Settles: which folder is kept.
- Fold: write the result into the cited `ui/frontend-menus.md` rule and drop its PROVISIONAL line.

##### REC-207 [MANUAL] Front end: Slot level line text
- Priority 3 (front-end screens; a capture of the 1.14d menus, no game run needed for most).
- Settles: `ui/frontend-menus.md` PROVISIONAL REC-207.
- Steps and compare: Slot level line text. Capture: screenshot one slot per class; or read the string id pushed to the `D2Lang_GetStringByIndex` call after `" %d "` in `0x004380F0` (`disasm.py fn 0x004380F0`). Settles: the exact level / class line text and colour.
- Fold: write the result into the cited `ui/frontend-menus.md` rule and drop its PROVISIONAL line.

##### REC-208 [MANUAL] Front end: class names 10097 / 10098
- Priority 3 (front-end screens; a capture of the 1.14d menus, no game run needed for most).
- Settles: `ui/frontend-menus.md` PROVISIONAL REC-208.
- Steps and compare: class names 10097 / 10098. Capture: extract `data\local\lng\eng\patchstring.tbl` from `Patch_D2.mpq` (by name; the archive has no listfile) or, in the 1.14d game with expansion, hover the Druid and the Assassin on the create screen and screenshot text 197. Settles which id shows which name.
- Fold: write the result into the cited `ui/frontend-menus.md` rule and drop its PROVISIONAL line.

##### REC-209 [MANUAL] Front end: filter argument 2
- Priority 3 (front-end screens; a capture of the 1.14d menus, no game run needed for most).
- Settles: `ui/frontend-menus.md` PROVISIONAL REC-209.
- Steps and compare: filter argument 2. Capture: on the create screen, select a class, type `-` into the empty box, then `a-`; hook `0x00430590` and log its 3 arguments. Settles whether a leading separator is rejected (arg = caret position) or the argument means something else.
- Fold: write the result into the cited `ui/frontend-menus.md` rule and drop its PROVISIONAL line.

##### REC-210 [MANUAL] Front end: animation timing
- Priority 3 (front-end screens; a capture of the 1.14d menus, no game run needed for most).
- Settles: `ui/frontend-menus.md` PROVISIONAL REC-210.
- Steps and compare: animation timing. Capture: record the create screen at ≥ 50 fps; click the Paladin; count frames from click to `panu3` start (expect 79 × 40 ms ≈ 3.2 s) and check that loops skip the last frame. Settles §F3.3 rule 2/4 as implemented.
- Fold: write the result into the cited `ui/frontend-menus.md` rule and drop its PROVISIONAL line.

##### REC-211 [MANUAL] Front end: hardcore availability
- Priority 3 (front-end screens; a capture of the 1.14d menus, no game run needed for most).
- Settles: `ui/frontend-menus.md` PROVISIONAL REC-211.
- Steps and compare: hardcore availability. Capture: fresh install, empty save folder, Single Player → create screen, select any class; check the Hardcore box is shown (and log `[0x00779DA4]` at `0x00435580`). Settles §F3.5 rule 2.
- Fold: write the result into the cited `ui/frontend-menus.md` rule and drop its PROVISIONAL line.

##### REC-212 [MANUAL] Front end: Esc menu tree capture
- Priority 3 (front-end screens; a capture of the 1.14d menus, no game run needed for most).
- Settles: `ui/frontend-options.md` PROVISIONAL REC-212.
- Steps and compare: 1.14d single player at 800×600 (`-w`): open inventory, quest log and automap, press Esc, then Options → Video Options, press Esc there. Expect the whole menu to close and the saved panels to reopen (§O1 r4). Screenshot the Game, Options, Sound, Video and Automap menus and one with a slider row selected; pixel-compare with §O4 r4 (label x/y, both pentagrams — Return to Game selected: (99, 336) and (649, 336) — slider bar and skull, no backdrop). Optional: hook `0x00454850` to confirm the pentagram step after > 50 ms.
- Fold: write the result into the cited `ui/frontend-options.md` rule and drop its PROVISIONAL line.

##### REC-213 [MANUAL] Front end: Options slider float precision
- Priority 3 (front-end screens; a capture of the 1.14d menus, no game run needed for most).
- Settles: `ui/frontend-options.md` PROVISIONAL REC-213.
- Steps and compare: in game, Video Options → Contrast; press Right to the last position (99); read `HKCU\Software\Blizzard Entertainment\Diablo II\Contrast`: 100 means 53-bit precision, 99 single precision. Also break in `0x0047CD00` and read the x87 control word (FNSTCW, bits 8–9).
- Fold: write the result into the cited `ui/frontend-options.md` rule and drop its PROVISIONAL line.

##### REC-220 [MANUAL] Front end: loading-screen art source
- Priority 3 (front-end screens; a capture of the 1.14d menus, no game run needed for most).
- Settles: `ui/frontend-loading.md` PROVISIONAL REC-220.
- Steps and compare: extract `data\global\ui\Loading\loadingscreen.dc6` from `Patch_D2.mpq` by name (no listfile) or hook the cel load `0x004788B0` at `0x004565A0` during a single-player start and log the path and the archive it opened from; compare with the d2data copy (666,014 bytes, 10 frames 256×256). Same bytes or no Patch_D2 entry settles it.
- Fold: write the result into the cited `ui/frontend-loading.md` rule and drop its PROVISIONAL line.

##### REC-221 [MANUAL] Front end: act-change loading end tick
- Priority 3 (front-end screens; a capture of the 1.14d menus, no game run needed for most).
- Settles: `ui/frontend-loading.md` PROVISIONAL REC-221.
- Steps and compare: single player, waypoint Rogue Encampment → Lut Gholein; record S→C packets with tick numbers and a screenshot per client frame. Expected 0x05, 0x03, 0x53, 0x07×n, 0x0D in the 0x49 drain, then 0x04 next tick; presented frames: last world frame, loading frame 0, one black frame, Act II world.
- Fold: write the result into the cited `ui/frontend-loading.md` rule and drop its PROVISIONAL line.

##### REC-222 [MANUAL] Front end: keys during loading
- Priority 3 (front-end screens; a capture of the 1.14d menus, no game run needed for most).
- Settles: `ui/frontend-loading.md` PROVISIONAL REC-222.
- Steps and compare: press Esc, and separately a skill hotkey, between the first loading draw and the first game frame (game start and act change); hook `0x004690B0`, log ui 9 (`[0x007A27E4]`) and whether the server ticks (`0x0052FC20`). If Esc opens the menu or pauses the load, d2rs must dispatch keys; otherwise it ignores them as written.
- Fold: write the result into the cited `ui/frontend-loading.md` rule and drop its PROVISIONAL line.

##### REC-223 [MANUAL] Front end: screen after an act-start video
- Priority 3 (front-end screens; a capture of the 1.14d menus, no game run needed for most).
- Settles: `ui/frontend-loading.md` PROVISIONAL REC-223.
- Steps and compare: first Warriv travel with videos enabled; screenshot each frame from the end of `ACT02START` to the first Act II world frame. Black settles the written rule; the loading frame means a redraw exists (find its caller).
- Fold: write the result into the cited `ui/frontend-loading.md` rule and drop its PROVISIONAL line.

##### REC-225 [MANUAL] Front end: credits scroll rate and end
- Priority 3 (front-end screens; a capture of the 1.14d menus, no game run needed for most).
- Settles: `ui/frontend-credits.md` PROVISIONAL REC-225.
- Steps and compare: 1.14d expansion, main menu → Credits; record at ≥ 50 fps for 13 min, hook `0x004341F0` and `0x004FBF30`, log `[0x0077996C]` and column C's `[+0x58]` / `[+0x9C]` per tick. Compare against C3: first text row at baseline y 590, 19 px per 10 ticks, "The End" gone at draw 17,770, no automatic return to the main menu after.
- Fold: write the result into the cited `ui/frontend-credits.md` rule and drop its PROVISIONAL line.

##### REC-226 [MANUAL] Front end: 640×480 frame
- Priority 3 (front-end screens; a capture of the 1.14d menus, no game run needed for most).
- Settles: `ui/frontend-credits.md` PROVISIONAL REC-226.
- Steps and compare: set the 640×480 video option and restart; screenshot the main menu, Credits and Cinematics. Settles whether the front end stays 800×600 with the C3/C8 positions, or shows what changes.
- Fold: write the result into the cited `ui/frontend-credits.md` rule and drop its PROVISIONAL line.

##### REC-228 [MANUAL] Front end: disabled cinematics buttons
- Priority 3 (front-end screens; a capture of the 1.14d menus, no game run needed for most).
- Settles: `ui/frontend-credits.md` PROVISIONAL REC-228.
- Steps and compare: set `Aux Battle.net` = `216.148.246.34`, open Cinematics with the classic and the expansion front end, screenshot both. Compare the disabled buttons: draw mode 1, blank labels in the expansion menu, the colour of the kept classic label against an enabled one.
- Fold: write the result into the cited `ui/frontend-credits.md` rule and drop its PROVISIONAL line.

##### REC-229 [MANUAL] Front end: startup chain, no blizno screen
- Priority 3 (front-end screens; a capture of the 1.14d menus, no game run needed for most).
- Settles: `ui/frontend-credits.md` PROVISIONAL REC-229 and `ui/front-end.md` §F1.3 rows 1–2.
- Steps and compare: delete `Aux Battle.net`, start 1.14d hooking `0x004F5D90` (log video paths) and `0x0042F430` (log descriptor ids); repeat with the value present. Settles if no descriptor 0/1 is ever built and the videos are the two Bliz logos plus `d2intro` on the first run only.
- Fold: write the result into the cited `ui/frontend-credits.md` rule and drop its PROVISIONAL line.

##### REC-170 [MANUAL] Lighting detail: other light sources in the play preview
- Priority 3 (preview only; `world_view/light_sources.rs`, d2rs-own, unverified).
- Settles: the PROVISIONAL points of `docs/handoff/q-light-radius-detail.md`: monster / object / missile lights rebuilt each frame as plain records (no §6.4 radius walk, flicker, umod 3, `L_c`, cast, overlay or Den of Evil lights).
- Steps and compare: capture a light map (`render/capture.md`) in a town and a cave with a lit monster, an object with `Lit` and a missile; compare cell values with the computed map.
- Fold: replace the preview fills with the client light list of `lighting.md` §6, §8.

#### Deferred (not recorded; outside Phases 0–6)
- IT-1 `formats/d2s.md` OQ1 (legacy loader `0x00534020`, pre-1.09 saves), IT-2 OQ2 (item records of save versions 0x5C–0x5F), `formats/d2s-legacy.md` and `items/bitstream-legacy.md` Pending lines (1.07 / 1.08 / 1.00–1.06 saves): need sample files and are only needed if old saves must load. Record only if the user opens that scope.
##### REC-96 [ASSISTED] Level-up stat messages (S→C 0x1D-0x1F, 0x21) around a level-up
- Priority 2 (the play preview sends them by a diff at the tick's sync).
- Settles: PROVISIONAL `combat/vitals.md` §3 step 7 / `wiring::action::vitals_sync::stat_changes` (the original sends a changed stat from the unit's client update from the changed-stat array, `sim/stat-lists.md` §11; the preview sends level, stat points, skill points, attributes, the maxima and next-level experience against a per-client cache at the end of each tick), and PROVISIONAL `skills/levels.md` §6.4 (the message after a spent skill point: 0x21 skill, base level; what 0x3B sends on the validator's codes 2 / 3, OQ5).
- Steps: a new character at level 1; `record_packets.py --seconds 120` through one kill that levels up (stat points and a skill point appear), then spend one stat point (C→S 0x3A) and one skill point (C→S 0x3B).
- Output: `rec96-packets.jsonl`.
- Compare: which S→C stat ids, in which order and in which tick follow the level-up; the stat ids and values after 0x3A; the 0x21 (and any 0x22 / 0x23) after 0x3B against `d2-server` `skills/world.rs` `add_skill_level`. Fold: replace the two PROVISIONAL points.

- **REC-44** legacy saves, cited by the specs as the capture: `formats/d2s-legacy.md` OQ2 (load one 1.07 / 1.08 save, version 0x57 / 0x59, in 1.14d and compare the unit (stats, skills, items, hireling) with the rules), `items/bitstream-legacy.md` OQ1 (the same 1.07 / 1.08 save: every item against §2–§4, affix ids after the offset of §2 rule 3) and OQ2 (a 1.00–1.06 save, version 0x47, holding a stored, equipped, belt, cursor and socketed item, an ear and a corpse with an equipped item: compare with §6–§8 and edge cases 7–9). No such save exists on this PC; record only if the user opens that scope (steps: place the save in `%USERPROFILE%\Saved Games\Diablo II`, `record_packets.py --seconds 60` plain, then `tools/d2s_check.py`).

#### [NO] Not reachable with the current setup (and why)
- A classic game (REC-46; `monsters/init.md` OQ1 0x67 in classic SP; `client/model.md` §7 r8 / §17 r6 (a) joining a classic game with an expansion character; `quests-act4.md` OQ2 classic end; REC-29's classic part): the forced start always makes an expansion game; a classic character is refused. Needs a manual classic start.
- Two-client games (a second client sees the owner's hireling, REC-10 / `hirelings.md` OQ7 and R1 on a hosted game; the Alkor broadcast of REC-28; the uncredited player of `quests-act4.md` OQ2): needs a second client joined over TCP/IP; not attempted.
- Act V (levels 109–132, REC-30, REC-31 GH, and the Act V parts of REC-38 / REC-40): the waypoint panel of `TestSor` has tabs I–IV only.
- Fights, kills, quest chains, hireling events: the input script has no combat loop, so these are [MANUAL] / [ASSISTED] (REC-07…11, 23…34) and never `--auto`.
- `render/draw-order.md` OQ7 (TownE1 tile (950, 933)): off-screen from every reachable spot (`local-buddy-q9-rec` R2-1): REC-32 notes it.

- REC-97 (PROVISIONAL, M22; player death in the play preview, `q-death`): the S→C form of a player's death is not specified (`sim/pathing.md` §10 r2 lists only the walk rows). Preview: 0x0D PlayerStop with the mode-request code 8 (DT) and 9 (DD), then code 7 (neutral after death) on the respawn, sent to every client; the corpse is a player-type unit allocated at the DD start (mode 17, state 7, owner GUID kept in `DeathState::owners`, announced by 0x59 + 0x0D code 9); its items stay with the player (the inventory model has no corpse grid, `vitals.md` §4.7 r1.7) and the dropped gold is not made into piles (`Pending::death_drop_gold` has no preview provider), so a death loses the gold of §4.6 rule 1 for good. The dispatch gate of a player who has died follows the live unit mode (it is staged at mode 1 otherwise), so C→S 0x41 is accepted in mode 17. Check: a death trace (S→C bytes of a player's death and of the 0x41 respawn).
- REC-98 (PROVISIONAL, M22): no spec gives the screen or text shown while the player is dead. Preview: a Bevy text node "You have died / Press Esc to return to town" while the local player's mode is 0 or 0x11; Esc in mode 0x11 sends C→S 0x41 (`ui/panels.md` §3 r1). Needs the original's wording and layout (RE of the dead-state draw).
- REC-111 (PROVISIONAL, M22; monsters attack the player, `q-monster-ai`): (1) the skill an attack mode uses is not specified (`monsters/ai.md` §7.1's plain attack request sets none): preview, Attack (skill 0) level 1; (2) the attack-family per-frame `0x005A7670` (`skills/use.md` §5.2 OQ6 gives only "tests +0x4E = 1"): preview, the sequence frame's skill part with unit +0x4E := the type-0 timer's code; (3) the start's target clear is not kept for monsters. Needs the monster half of `use.md` OQ6 and the monster composer (RE).
- **REC-177** Town and panel gaps (PROVISIONAL, d2rs-own; M22; q-town-gaps): (1) the shop's single-item repair button (frame 6, third of a repairer's row) toggles a mode; the next left click on one of the player's items (grid page 0 or worn) sends C→S 0x35 for it with no confirm dialog (`ui/shop_ui.rs`, `ui/panels/inv_items_repair.rs`); (2) Gheed and Charsi stand in the synthetic Rogue Encampment at x 36 / 8 (`app/town_npcs.rs`; Elzix and Jamella were already placed by q-a2-town / q-a4); (3) the weapon switch C→S 0x60 has no written body (`0x005616A0`, intents-events.md OQ16): d2-sim `wiring/inventory/swap.rs` trades body locations 4/5 with 11/12 (hand items unlink their stat lists, the swap set's link; requirements are not rechecked; every moved item joins the update list; S→C 0x97 flips the client's set); the W key sends it (`world_view/swap_key.rs`); (4) SUPERSEDED by REC-188 (q-item-bonus-wire; the pseudo states are removed, the client sums the item lists): item bonuses to the panel ride as the stat lists of pseudo states 0xFE / 0xFD (S→C 0xA8 SetState, ≤ 16 stats each, `vitals_sync.rs` `item_state_changes`) instead of base + bonus, so the client's total is base + lists and the existing value-above-base colour is blue; the stats are 0-3, 7, 9, 11, 19, 21-25, 31, 39-46. Supersedes the transport of REC-163 (2). Not done: the damage / attack-rating block of the character panel is not drawn in the preview at all (`char_details` is not fed); swap requirements and durability; the weapon-set tabs; a saved character's linked items. Needed: a trace of the W key with two weapon sets (0x60 answer, 0x9D moves, 0x97).

#### PC 1 s8 additions (2026-10-08, `docs/handoff/pc1-s8.md`; REC-300+)
Spec rules these confirm or settle were written from the 1.14d binary on `claude/local-pc1-s8`; most are confirmations. Recordability: `docs/handoff/pc1-s8.md` Lane C header (some need `q-rec-hooks` first).

##### REC-300 [MANUAL] (R-SWAP-1)
- R-SWAP-1 [MANUAL]: Settles: REC-177 (3) message order and fields. Expansion character, set 1 sword + shield, set 2 a two-hander that fails a requirement, a different left/right skill chosen in each set. Press W twice, then once more with both sets empty. Capture S→C in order: 0x97, the direct 0x23s, 0x9D action 0x17 ×n, 0x47, 0x48, the two queued 0x23; the 0x23 fields and the 0x9D bit streams (body location, flag 0x4000 on the unusable two-hander).

##### REC-301 [MANUAL] (R-SET-1, R-SET-2)
- R-SET-1 [MANUAL]: Settles: REC-188 (stat-lists OQ8). Equip a two-hander (action 0x07) over a worn set shield while a partial set bonus shows; record 0x9C/0x9D and what follows, then read the character panel: does anything refresh the taken-off item's set list?
- R-SET-2 [MANUAL]: Settles: REC-188 (stat-lists §2 r6, set test vectors). Equip 2 then 3 pieces of one set, unequip one; record the 0x9C/0x9D streams (set mask, lists), confirm no 0xA8 for states 165–170 and base-only 0x1D–0x1F; compare panel totals with base + item lists + client-computed set bonuses.

##### REC-302 [MANUAL] (R-WW-1)
- R-WW-1 [MANUAL]: Settles: REC-173, REC-232 (tick timing). Whirlwind with one and with two weapons, and one Leap: per tick the unit position, 0x4C/0x4D, type-0 timer args, E param 4, hit ticks.

##### REC-303 [MANUAL] (R-CLAW-1)
- R-CLAW-1 [MANUAL]: Settles: REC-233. Fists of Fire or Dragon Claw with two claws vs one claw on one monster: type-0 events per attack, unit flag 0x40, damage per hit with different claws.

##### REC-304 [MANUAL] (R-SENTRY-1)
- R-SENTRY-1 [MANUAL]: Settles: REC-233 / REC-176 sentry part. One Lightning Sentry laid near a monster: shots fired (expect 10), think ticks, death tick.

##### REC-305 [AUTO] (R-PAUSE-1)
- R-PAUSE-1 [AUTO]: Verifies `ui/frontend-options.md` §O1 r6 (REC-237). 1.14d single player: log the server frame counter and the client update-clock global `0x007A0490` each loop pass; open the Esc menu 10 s, close it. Expect no server tick while open and no catch-up after.

##### REC-306 [MANUAL] (R-ACT-1)
- R-ACT-1 [MANUAL]: Settles: REC-175, REC-174 (Meshif part), waypoints OQ9. Tyrael's travel and the Harrogath portal (operate 73) to level 109, Meshif to level 75: packets and RNG (level-seed `roll(n)` at `0x0066ACB3`), order 0x05 / 0x03 / 0x53 / 0x07… / 0x15, arrival point, whether a 0x0A for the player's own GUID is sent. Needs saves at those quest stages.

##### REC-307 [MANUAL] (R-TYRPORT-1)
- R-TYRPORT-1 [MANUAL]: Settles: REC-174. Talk to Tyrael (msg 302), walk through the portal: its 0x51, position, the two game-seed steps, arrival point in Lut Gholein (expect near the tile-12 spawn), removal / ENDANIM on use.

##### REC-308 [AUTO] (R-HCFLAG-1)
- R-HCFLAG-1 [AUTO]: Confirms `client/model.md` §7 r9 (REC-46). 0x67 bytes of a classic-hardcore and an expansion-hardcore character joining single player; expect u32@0x27 = 0x804 and 0x100804.

##### REC-309 [AUTO] [MANUAL] (R-MSG-1, R-MSG-2, R-MSG-3, R-MSG-4, R-MSG-5)
- R-MSG-1 [AUTO]: Settles: msg-stats-items OQ1. Recorder dumps the local player's stat list after frame 2 of a join.
- R-MSG-2 [MANUAL]: Settles: msg-units OQ7. Hire a hireling: 0x7A, 0x81, 0xAC.
- R-MSG-3 [MANUAL]: Checks msg-skills §1 r6 (REC-09). Equip / unequip a charged item, use the charges.
- R-MSG-4 [AUTO]: Settles: msg-skills OQ4. Scan existing recordings for a frame where a later message changes the unit, target or state 118 after a 0x99 / 0x9A event.
- R-MSG-5 [MANUAL]: Checks msg-units §8 r10.1. Necromancer skeleton render capture (expect palette map 1).

##### REC-310 [AUTO] (R-EXIT-1)
- R-EXIT-1 [AUTO]: Settles: intents-events OQ2 remainder. Single-player exit: S→C receive order on the client (system 0xB0 vs game 0x05 / 0x06 in the same drain).

##### REC-311 [AUTO] (R-MUPD-1)
- R-MUPD-1 [AUTO]: Settles: intents-events OQ10 remainder. One plain Act I fight: which monster-update senders fire (0x11, 0x57, 0xA7–0xA9).

##### REC-312 [MANUAL] (R-DOOR-1)
- R-DOOR-1 [MANUAL]: Confirms intents-events §9 r4. Hover an open door with a monster in the doorway: C→S 0x3D and the door's next mode (2 → 5).

##### REC-313 [MANUAL] (R-BELT-1, R-BELT-2, R-BELT-3)
- R-BELT-1 [MANUAL]: Settles: control-panel §5 r4, r13. Set `CfgBelt1` to F1, bind `CfgBelt3` in its secondary slot only; screenshot the belt (expect "F1" and "None").
- R-BELT-2 [MANUAL]: Settles: control-panel §5 r8. Hovered belt potion with and without an NPC shop open, screenshot.
- R-BELT-3 [MANUAL]: Settles: control-panel §5 r4–r5. Highlight pixels: hovered usable potion; cursor item over empty, swappable, invalid box.

##### REC-314 [MANUAL] (R-HUD-1)
- R-HUD-1 [MANUAL]: Settles: control-panel §5 r14, REC-238. Hovered run button and experience bar at 800×600, screenshots.

##### REC-315 [AUTO trace + MANUAL screenshot] (R-GOLD-1)
- R-GOLD-1 [AUTO trace + MANUAL screenshot]: Settles: panels-2 §21 r6, r8. Stash gold withdraw and deposit dialogs, C→S 0x4F trace.

##### REC-316 [MANUAL] (R-LOAD-1)
- R-LOAD-1 [MANUAL]: Settles: REC-236 (and `frontend-loading.md` L5–L7 checks). Draw calls of the loading screen across a waypoint change and a game start at 800×600 (repeated 0x03 for the same act, 0x61 videos).

##### REC-317 [AUTO] [MANUAL] (R-LVL-1, R-LVL-2)
- R-LVL-1 [AUTO]: Confirms `drlg/levels.md` §12.3 (REC-230). Enter Act III with `-seed 644409375` (TestSor), dump drlg +0x90 vis/warp of levels 75–83.
- R-LVL-2 [MANUAL]: Confirms §12.4 (REC-230). Act V save: dump vis/warp of 109–112, walk into 112 and 117, list warp tile units (class 71/72).

##### REC-318 [ASSISTED] (R-MODE-1)
- R-MODE-1 [ASSISTED]: Confirms `sim/pathing.md` §10 r2 (REC-95). A dodge/avoid trigger and a normal A1 attack with a second client: S→C 0x4C/0x4D received by the own and the other client.

##### REC-319 [AUTO] (R-ACTBYTE-1)
- R-ACTBYTE-1 [AUTO]: Confirms path-placement §13 r2. Load a character saved in Act III, log the S→C 0x03 act byte (expect 2, town 75).

##### REC-320 [AUTO] (R-MIS-1, R-MIS-2)
- R-MIS-1 [AUTO]: Confirms missiles OQ9. Quill rat A2 on Normal and Hell, missile positions per tick (expect v = 2112 / 3456).
- R-MIS-2 [AUTO]: Confirms missiles OQ11. Hook `0x0059FA30` in a combat recording, log the tick step in progress (expect step 4 or between ticks).

##### REC-321 [MANUAL] (R-SHAKE-1)
- R-SHAKE-1 [MANUAL]: Optional confirmation of camera §8 rows 36, 38 (REC-62). Next to the Hellforge or at Duriel's death: shake start time, `seed_start` / `seed_end`, (dx, dy) per frame.

##### REC-322 [AUTO] (R-NOTICK-1)
- R-NOTICK-1 [AUTO]: Settles camera OQ8 / capture OQ8 against recordings. For runs 1–2 list frames after 0 ticks; each must be paused or inside the `[0x007A04BC]` hold (record it and GetTickCount per frame).

##### REC-323 [AUTO] [MANUAL] (R-SHRINE-1, R-SHRINE-2, R-SHRINE-3, R-SHRINE-4)
- R-SHRINE-1 [AUTO]: Settles objects-2 §26.1–§26.3 (REC-239). Stat-list trace (state, flags, expiry, source, stats) after each state shrine (codes 6–15); same shrine twice, a second shrine, a shrine while cursed (Amplify Damage), expiry of the stamina and skill shrines.
- R-SHRINE-2 [MANUAL]: Settles objects-client §29 r2–r5, OQ4. 800×600 hover screenshots: chest, locked chest, shrine before/after, a door per mode, a waypoint, a town portal.
- R-SHRINE-3 [MANUAL]: Settles objects-2 §26.4. Frame-stepped capture from shrine operation until the bubble disappears.
- R-SHRINE-4 [MANUAL]: Confirms client/model §15 r6. Storm, Exploding and Poison shrines: client missile creates (class, position, target, level, order).

##### REC-324 [AUTO] (R-DRLG-DIR)
- R-DRLG-DIR [AUTO]: Settles outdoor OQ1. After Act I creation read the u32 at `[level+0x14]+0x04` for levels 1 and 27, and 40 after Act II (expect 3 and 1 for the recorded seed).

##### REC-325 [AUTO] (R-WARP-RESTORE)
- R-WARP-RESTORE [AUTO]: Confirms rooms §8 r6. Blood Moor: log the room's type-5 units (GUID, class, x, y) before deactivation and after reactivation, plus the tick step they appear in (expect new GUIDs).

##### REC-326 [MANUAL] (R-TINT-1)
- R-TINT-1 [MANUAL]: Confirms shading §6 r1.1, lighting §8. Monster under Poison then Cold, local player under Poison, GDI `-w`: cel draws' palette argument and the player's light colour (record +0x25..+0x27).

##### REC-327 [MANUAL] (R-WALL-1)
- R-WALL-1 [MANUAL]: Confirms lighting §11 r2, r4 / blend-modes §6. A wall in a preset with Logicals (Jail or Cathedral) next to a light, plus a Lut Gholein roof: per-pixel tile output and the light-map words.

##### REC-328 [AUTO] (R-LIGHTEDGE-1)
- R-LIGHTEDGE-1 [AUTO]: Confirms lighting §4 r2. Light-map flag words at `0x007B0E68` at the edge of the loaded rooms.

##### REC-329 [MANUAL] (R-BUBBLE-1)
- R-BUBBLE-1 [MANUAL]: Confirms messages §5 r1. An NPC overhead bubble at two draw rates; when it disappears, in drawn frames.

##### REC-330 [AUTO, optional] (R-SYS4-1)
- R-SYS4-1 [AUTO, optional]: Scan packet recordings for S→C 0x26 with u8@1 = 4 (none expected).

##### REC-331 [MANUAL] (R-GARG-1)
- R-GARG-1 [MANUAL]: Settles sprite-placement §3 PROVISIONAL, dcc §Frame size limit. Walk a room with a `gargoyletrap` preset (`ACT1\CATACOMB\catNEtheme1.ds1` has 3): record unit and cel draws, or note an abort with 0x58C.

##### REC-332 [MANUAL, second client] [MANUAL] (R-TP-1, R-TP-2, R-TP-3, R-TP-4)
- R-TP-1 [MANUAL]: Confirms objects-2 §27.1, §27.4, §27.5. TP scroll in Blood Moor with an older pair elsewhere; enter O1, return through O2. Expect old pair 0x0A; O1's 0x51/0x60/0x82 + sound 7; 0x7C, 0x22, 0x3F, scroll removal; 0x0D on arrival; 0x0A for both after the return.
- R-TP-2 [MANUAL]: Confirms §27.1 r4, use §1 r5. Scroll in the Rogue Encampment: sound 24, one 0x3F, one 0x7C, scroll kept, no 0x22.
- R-TP-3 [MANUAL]: Confirms use edge 1. Tome in the field: 0x7C, 0x3E (stat 70), 0x7C, 0x22; tome keeps flag 0x4.
- R-TP-4 [MANUAL, second client]: Confirms §27.5 r2. Second player enters the room with the owner's portal: 0x82 fields.

##### REC-333 [MANUAL] (R-CUBE-1, R-CUBE-2)
- R-CUBE-1 [MANUAL]: Confirms cube §11 r2–r3 (REC-244). Items in the cube, panel open, lift the cube, place it elsewhere, Transmute a valid recipe; then drop the cube on the ground. Expect the panel stays open, the transmute works, page-3 items go to the inventory, no 0x4F 0x17 / 0x77.
- R-CUBE-2 [MANUAL]: Confirms panels-2 §20 r3. Hover close and Transmute at 800×600, screenshots.

##### REC-334 [AUTO] [MANUAL] (R-SAVE-1, R-SAVE-2, R-SAVE-3, R-SAVE-4, R-SAVE-5)
- R-SAVE-1 [AUTO]: Confirms d2s §2.1. Waypoint to Act II, save: header +0xA8..+0xAA = `81 00 00`.
- R-SAVE-2 [MANUAL]: Confirms d2s §2.3. Andariel killed in Normal, saved: status bits 8–12 = 1 (5 on Nightmare after Andariel, expansion).
- R-SAVE-3 [MANUAL]: Confirms d2s §2.4 r6. Oskill / charged item skill as right skill, save and reload; variant with the item stashed first.
- R-SAVE-4 [MANUAL]: Confirms d2s §2.5 r3, §8.4, OQ4. Hire after the last load, give items, save; gain experience, save again.
- R-SAVE-5 [MANUAL]: Confirms d2s-load §3, d2s OQ15. Necromancer with an item Iron Golem, saved, rejoin with a packet log: recast at the join.

##### REC-335 [AUTO hook, MANUAL placement] (R-LOS-1)
- R-LOS-1 [AUTO hook, MANUAL placement]: Confirms pathing §13.4. Hook `0x00645950` entry/exit (x, y, GUID, mask, result, unit position) casting a lineofsight-4 skill across a dungeon wall and in the open; also client `0x004C6140`.

##### REC-400 [AUTO] (R-3E-1) — wire byte layout, top of the list (M22)
- R-3E-1 [AUTO]: Settles client/msg-stats-items §5 r1.3 PROVISIONAL (q-fix-proto). Packet log while buying a stack (arrows), stacking keys, repairing a damaged item, recharging a wand and using a tome: every S→C 0x3E's GUID / value / param width bits against the field values (narrowest width?), the set flag, the value (base after the change?), the param. Or a Ghidra read of `0x0053D130`'s width choice.

##### REC-401 [MANUAL, second client] (R-JOIN-1)
- R-JOIN-1 [MANUAL, second client]: Settles intents-events §8.3 PROVISIONAL (q-fix-proto). Packet logs of both clients while a second player joins a game with one player in it: which 0x5B / 0x65 each client gets (the joiner's to everyone? every player's to the joiner?), their order against 0x8D and the 0x5A, and 0x5B's level / party words.

##### REC-402 [AUTO hook] (R-CHAT15-1)
- R-CHAT15-1 [AUTO hook]: Settles intents-events §2.4 r6 PROVISIONAL (q-fix-proto). Hook `0x0054A5D0` exit (EAX) with crafted C→S 0x15: msg of 256 characters, msg filling the message to its last byte (strlen + 4 = size), no NUL; or read its return paths in Ghidra.

##### REC-410 [AUTO hook] (R-OVL11-1)
- R-OVL11-1 [AUTO hook]: Settles intents-events §7.3 r2 step 9 PROVISIONAL (q-fix-proto-rest). Hook `0x005715A0` / `0x0053D850` while a monster gets an overlay from a skill (a Frost Nova hit, a Holy Fire aura pulse): the S→C 0x11 bytes (type, GUID, overlay) against stat 178 of the unit's overlay list read as base and as total (`0x00625A50`), and the overlay count compared (data tables +0xBC0, v = count sent).

##### REC-411 [AUTO] (R-LAND-1)
- R-LAND-1 [AUTO]: Settles intents-events §7.9 r2 (a) PROVISIONAL (q-fix-proto-rest). Packet log of a player's Leap landing and a monster Leap (Sand Leaper): the S→C 0xA5 bytes (the unit type and GUID against the leaping unit, the skill).

##### REC-412 [AUTO hook] (R-HEAL-1)
- R-HEAL-1 [AUTO hook]: Settles intents-events §7.9 r2 (b) PROVISIONAL (q-fix-proto-rest). Hook `0x00571CD0` at the 0xAB case (the test `0x00554200` with its arguments) while a second player and a hostile and a friendly monster regenerate near the client: which receivers get the 0xAB.

##### REC-413 [AUTO] (R-CAST-1)
- R-CAST-1 [AUTO]: Settles intents-events §7.9 r2 (c) PROVISIONAL (q-fix-proto-rest). Packet log of an item-cast event (a weapon with "chance to cast on striking", `events.txt` item cast) at a unit and at a point: the S→C 0x99 / 0x9A bytes (the level byte, w, the 16- or 17-byte choice).

##### REC-414 [AUTO] (R-MISS73-1)
- R-MISS73-1 [AUTO]: Settles missiles R2.4 PROVISIONAL (q-fix-proto-rest). Packet log of joining a room that holds a `ClientSend` missile in flight: the S→C 0x73 bytes against the missile's 16.16 position, first path point, current frame, level and stat 328.

##### REC-415 [PC 1, Ghidra read] (R-CALLERS-1)
- R-CALLERS-1 [PC 1, Ghidra read]: Settles msg-stats-items §1 r4 and msg-skills §9 PROVISIONAL (q-fix-proto-rest). The static callers of the S→C 0x20 sender `0x0053C1D0` and the 0x93 sender `0x0053C6F0` (and, again, `0x0053E1C0` for 0xA6): when they run and with which values; then d2rs sends them from those places.

##### REC-416 [AUTO] (R-92-1)
- R-92-1 [AUTO]: Settles msg-stats-items §5 r5 PROVISIONAL (q-fix-proto-rest). Packet log of a join, a trade and a death with items on the body: when the S→C 0x92 is sent (the bytes) and what follows it (the 0x9D records that re-add the items), and a Ghidra read of `0x004C23E0` / `0x0062E410` for the node order.

##### REC-404 [PC 1, Ghidra read; AUTO hook] (R-SUBFILE-1)
- R-SUBFILE-1: Settles drlg/outdoor-tilesub §2.3 PROVISIONAL (q-fix-cold-plains). The file argument the border substitution's replace callback `0x0066F520` passes to the stamp `0x006743C0`, and that function's path for it (the recording fixes that no build-list roll `0x0067438F` happens for a replacement's stamps; the file is d2rs's guess, 0, build list untouched). Or a hook on `0x006743C0` entry (P, F, border) and the grid-2 file bits at a replacement-stamped multi-file piece (4–7) after the level build.

##### REC-430 [AUTO hook] (R-ANIM-1)
- R-ANIM-1 [AUTO hook]: Settles audio unit-feed REC-430 (q-fix-audio-sounds). Hook the client unit update (footstep call `0x004CAF60` entry) and log U+0x44 / +0x48 / +0x4C, C and the unit's mode for a walking zombie, a running player and a walking Fallen over 60 updates: the frame at the mode set, the frame count F, the speed s (AnimData speed or modified by Velocity / run speed), and the update order of frame advance and footstep.

##### REC-431 [AUTO hook] (R-FLOOR-1)
- R-FLOOR-1 [AUTO hook]: Settles audio unit-feed REC-431 (q-fix-audio-sounds). Footstep request log (`0x004B9A00` ids for `0x004CAF60`) of the player running over a stone floor (Cathedral), sand (Lut Gholein), snow (Harrogath) and wood in one run, with the `soundenviron.Material 1` of each level: the material k each step requested vs the d2rs default (Material 1).

##### REC-432 [AUTO hook] (R-UNITORDER-1)
- R-UNITORDER-1 [AUTO hook]: Settles audio unit-feed REC-432 (q-fix-audio-sounds). Request log of one client update with two monsters idling in the same room: the order of their `Neutral` / footstep requests against their GUIDs and room list order (d2rs: ascending unit key).

##### REC-433 [AUTO hook] (R-HITCLASS-1)
- R-HITCLASS-1 [AUTO hook]: Settles audio unit-feed REC-433 (q-fix-audio-sounds). Swing request log (`0x004CB6A0`, `0x00623C20` result) for a player with a bow, a two-handed sword, a javelin and a weapon swapped to the alternate slot: the weapon hit class per case against the body location 4 item's `weapons.txt` `hit class`.

##### REC-434 [AUTO hook] (R-FIRSTSIGHT-1)
- R-FIRSTSIGHT-1 [AUTO hook]: Settles audio unit-feed REC-434 (q-fix-audio-sounds). Request log over a level entry and over a monster spawning into view: whether `Init` (`0x004CC380`), the on-sounds of the states a created unit already has (`0x004D9B20`), and the drop sound of an item created in mode 5 are requested at unit creation, or only on later changes.

##### REC-435 [PC 1, Ghidra read; AUTO hook] (R-SKILLSND-1)
- R-SKILLSND-1: Settles audio unit-feed REC-435 (q-fix-audio-sounds). Which of the skill start (`0x004C6140`), missile hit (`0x004D2D70`) and `ProgSound` calls run for a unit the client did not start itself; request log casting Fire Bolt, Frozen Orb and a monster Skill1 with the caster in view (the model does not run these functions, so they are not requested).

##### REC-436 [AUTO hook] (R-GREETMODE-1)
- R-GREETMODE-1 [AUTO hook]: Settles audio unit-feed REC-436 (q-fix-audio-sounds; `triggers.md` §10 r1). The `mode` argument `0x004E0590` gets from `0x004B4FD0` and `0x004B66B0` (d2rs: 0, which may pick the inactive line): hook `0x004E0590` entry (N, mode) while talking to Akara twice, to a quest NPC with no text line, and leaving.

## 8. Lessons (problems met, fixes)

| Problem | Fix |
|---|---|
| 2026-10-08: the build passed 7,000+ tests and was reported about 99 % done, yet was not coherent or playable on the real install (rendering seams, rubber-banding, shop). Cloud code was built and tested only against invented "synthetic" fixtures and its own specs; nobody measured the real game for the cloud, and no check covered the seams between systems | M23 (real data and measured facts from the first commit, contract checks at seams), M24 (progress as remaining work); CLAUDE.md rule 1 rewritten: game files in the private data repo, cloud has them; measured facts in public `facts/` |
| 2026-10-09 (q-fixture-migrate): the play game on the real install had towns with no NPCs, no stash and an unusable waypoint, and walking out of town was fatal; 209 client tests were green because they ran on the invented `GameData::Synthetic`, whose build placed every NPC and object by hand, so the three seams (population's level-type handle, the tick's object population, init 17) were never exercised. Made where the seams were wired (single_player build, worldgen dispatch, object init), caught only by the first headless run on the install | no silent fallback to invented data (`GameData::select(None)` is an error); client tests read the install through `app_support::game_data()` and run in the real-data gate; the play smoke runs on the install (M23) |
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
| PC 1's integration merge committed conflict markers in `specs/combat/vitals.md` OQ4 (6a77d80, 2026-10-07): the auto-resolver handled only append lists and section indexes, then committed whatever was left; `spec_index --check` and `coverage --check` do not see markers (caught by a grep after the push; fixed d841c3e) | the fold script (`C:\d2orchold.sh` on PC 1) greps `specs docs crates tools` for `<<<<<<<` / `>>>>>>>` after auto-resolution and aborts before committing; a marker check belongs in CI next to `spec_index --check` (proposal for the cloud; METHODS M07, M21) |
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
| DT1 `decode_rle` built a slice index for a skip-only `(skip, 0)` pair past the 32 × 32 block; the bounds check ran only for `count > 0` (panic; found 2026-10-06 by libFuzzer within minutes, missed by the 200k-case proptests) | the `dt1` fuzz target and the regress test; audit other parsers for a bounds check guarded by `count > 0` (M07, M21) |
| GPU render exactness | R8Uint indices, sRGB palette via `textureLoad`, `Msaa::Off`, `Tonemapping::None`, pixel-aligned quads |
| Two local sessions worked the same PC areas in parallel from different bases (2026-10-08, caught at the staging-6 merge: 8 of 9 PC 2 branches conflicted, "to reconcile" notes) | before writing, a lane diffs its files against every open integration branch (check: the lane start diffs its files against the open integration branches) |
| Cloud sessions overrode their assigned REC ids with "highest + 1" (q-a2-tyrael-door and q-assassin-gaps both took REC-241, already q-save-gaps') and one replaced q-strings-bind's REC-238 lines in files it had merged from staging (2026-10-08, made in the session's final renumber step, caught at the coordinator merge by `git grep` of the ids) | build-loop.md now says: use the assigned id, never renumber it, never touch another session's id lines (check: after each merge, `git grep -n REC-<id>` lists only the branch's files) |

- **REC-237** Esc game menu art, layout and strings (PROVISIONAL, `ui/esc_menu.rs`, M22): `controls.md` §3 row 56 and `panels.md` §3.1 specify only when the menu opens and closes. Needed for the spec: the ui 9 box art file and frames, the entry rectangles, the strings (Options, Save and Exit Game, Return to Game by id), what Options opens, and what `0x00456300(0, 1)` closes on Esc. Capture: screenshot of the Esc menu at 800 × 600 and the draw calls while it is open. The preview draws a dark box with Font16 English text.
- **REC-238** Control-panel tool tip font and globe numbers (PROVISIONAL, `d2-client` `ui/hud_tips.rs`, M22; q-strings-bind): the run, menu, new-stats / new-skills and experience tips resolve their string ids through the bound string tables and draw as centred text in font 1; `control-panel.md` does not name the tip font. The globe numbers (§3 r6) are still not drawn (no text width at panel draw). The stash GoldMax line now draws with the fixed stash limit 2 500 000. Needed for the spec: the tip font of `0x00502280`; capture: screenshot of a hovered run button and experience bar.
- **REC-252** Globe numbers and stamina tip (PROVISIONAL, `d2-client` `ui/hud_tips.rs`, M22; q-hud-globes): the life / mana numbers (§3 r6, `panelhealth` 4165 / `panelmana` 4166, toggles or hover) and the stamina tip (§4 r2, 4164) now draw in font 1 with widths from the bound `FontMeasure` (width 0 until fonts are bound). The text font (1) is the spec's "current font" for the numbers but is not named for the stamina tip; the shrine state 136 is read from the unit's states; state group 24 (blue stamina) is still not in the model. Capture: screenshot of the hovered life globe and stamina bar.
- **REC-236** Front-end game start and loading (PROVISIONAL, `app/front_start.rs`, `app/loading_overlay.rs`, M22): `frontend-loading.md` L5–L7 define the loading draws by message (0x03 / 0x04 / 0x05 / 0x61); the preview derives the same events from the client model's state (`act` change, `in_game` edges), so a repeated 0x03 for the same act load is not seen and 0x61 act videos are not fed. The start act (`start_act`) is not used to place the player (the save load does). The loading art is drawn as a UI node over the world, not by the sprite compositor, and sized 256 × 256 from its placement. Capture: draw calls of the loading screen across a waypoint change and a game start, at 800 × 600.
- **REC-245** Unit state tint, overhead bubbles and the system line source (PROVISIONAL, `d2-client` `world_view/state_tint.rs`, `ui/overhead_ui.rs`; M22; q-unit-fx): (1) `stat-lists.md` §3 r6.1/r6.3 name the state colour call `0x004D97F0` (run when a state with `colorshift` ≠ 0 turns on or off) but no spec gives its body. The preview reads it as the unit palette index (unit `+0x6C`, `shading.md` §6 r1) := `colorshift` while the state is on, so the unit's `P` is remap map `colorshift − 1`; among several tinted states the highest `colorpri` wins, ties the lowest state id. Needed: a capture of a unit under a tinted state (a shrine buff, Frozen, Poison, Amplify Damage) with the draw calls' palette argument, and what `colorpri` orders. (2) Overhead bubbles (`messages.md` §5) tick the counter once per client frame (25 Hz) instead of once per text pass, and place the bubble from the model's cell with the camera of the local player's model cell (no predicted walk). (3) No server code sends a type 4 system line (the line itself is wired and tested from raw bytes): the server-side sources (`You cannot …` refusals) are not specified. Needed: the trigger list of S→C 0x26 type 4 from a recording.
- **REC-251** Act videos and repeated same-act 0x03 in the loading flow (PROVISIONAL, `app/front_start.rs` `LoadFeed`, `ui/front_end/screens/loading.rs`, M22): the model now logs session messages in arrival order (`ClientWorld::session_log`, d2rs-own), so 0x05 / 0x03 / 0x61 / 0x04 reach `LoadingScreen` exactly as sent, a repeated 0x03 of the same act redraws loading, and a table video (0x61 ids 2–5) presents black with no loading redraw (REC-223 rule, unverified). Video playback stays a stub (the id is recorded). Capture: as REC-221 / REC-223.

##### REC-106 [NO RUN] world/npc.md §2–§3 host seams in the play preview (q-quests)
`AppRest` answers `distance` (integer Euclid in sub-tiles), `axis_check` (50 on both axes), `unit_check` (known GUID), `player_busy` (free), `start_allowed` (true), `tristram_cain_busy` (false), `approach` (logged only: the player is not moved) from a position snapshot. `0x00661480` (the 34 list bytes of S→C 0x27) is encoded as the inverse of the client's read. Monster init does not embed the interaction lists: `WiredWorld::interact_classes` registers the `npc && interact` units. After the client's dialog branch (0x2F, 0x31) the preview sends C→S 0x30 at once (no speech, no NPC menu). Needs a spec for `0x00548EF0`, `0x00548F80`, `0x00457490`, `0x00535060`, `0x00641530`, and the real close timing (recorded: 15 frames after 0x31).

##### REC-107 [NO RUN] ui/panels.md ui 0x0F quest log draw `0x004A34F0`, open `0x004A3FE0`
Not specified: the open function, the slot positions `0x00723EA8/EAC`, the cel layout, tab buttons, the 270 px wrap, the replay button. The preview opens on Q with C→S 0x40, draws a 3 × 2 slot grid, icon cels per state, one-line title/text; no just-completed animation and no C→S 0x58 acknowledge.
- **REC-100** Hireling follow and fight (PROVISIONAL, `d2-server` `handlers/world/hireling_drive.rs`, M22): `hirelings-ai.md` §1 points to `ai-bodies-6.md` §7 for the Hireable think, but the live host has no AI target providers (`Pending::good_target_search`, owner link; `stitch-combat.md` §1 row 12) and `AppRest::HirelingRest` has no unit access. The preview runs a per-frame stand-in: hostile monster within 20 sub-tiles, walk to it, swing every 20 frames at mean `mindamage`/`maxdamage`; else walk or run to the owner past 6 sub-tiles. No XP share, drops, get-hit or `hireling.txt` skill pick. Needed for the spec: the target providers' rules (hostility, `0x00552FD0` owner), the swing/skill timing from the AI think. Capture: a merc in the Blood Moor, per-frame mode, position and target.
- **REC-101** Hire list row text (PROVISIONAL, `app/hire_stats.rs`): the act of the list is not sent; the first act whose rolled row owns the name id is used; Life and Def read 0.
- **REC-103** Gold: pile art, drop dialog, server fills (PROVISIONAL, `ui/gold_dialog.rs`, `world_view/ground_items.rs`, `PreviewMoveRest`, M22): (1) `panels-3.md` §28 gives the gold dialog's controls but no art file list or frames for the box, spinner, OK / Cancel, so the preview draws a dark box with Font16 English text (prompt string 4033 by id once strings are bound) and binds Enter / Escape and a raw digit-key map; capture a screenshot of the drop dialog at 800 × 600. (2) `inventory-moves.md` §10.2 leaves the `gld` creation request layout (`0x00559CE0`) unwritten: the play rest creates a plain normal-quality item of the game's format at ilvl 1; capture the request fields of a dropped pile. (3) `0x00641530` (unit distance) and the free-spot search `0x0064E810` are not specified for the play host: it uses the larger sub-tile axis distance of the staged places and takes the drop start point as free; the server's `walk_to_item` is still a seam, so the client walks to a clicked item and asks again (`GroundItems::frame`). (4) The flippy file of a `gld` pile is read from `misc.txt` by code and its direction is the amount class of `unit-composite.md` §9; confirm the `gld` flippy name and direction count against a real install (`data-tool`/render check).
- **REC-117** Town Portal scroll / tome and the portal pair (PROVISIONAL, `d2-sim` `wiring/action/town_portal.rs`, `wiring/inventory/town_portal.rs`, M22): (1) the item-use table entry `0x00741790` for `tsc` / `tbk` (`pSpell` first word of `0x005BF240`) is unwritten; the desk takes the codes `tsc ` / `tbk ` as the use (a scroll is consumed with the 0x9D flag 0x20, a tome loses one stat-70 charge, no S→C stat message for the tome). (2) `0x0056D130`'s body is specified only as far as its draws (`cube.md` §9 item 4): the field portal sits at the free spot (size 3, mask 0x1C09) nearest the player, the town portal at the act's town spawn point (tile index 11, else 0); both class 59, owner = the player, `InteractType` = the other side's level; player data +0x48 is read as the field portal's GUID so `objects.md` §12 rule 12 removes the pair when the player arrives from town (the way in leaves it); a cast in a town creates nothing and still uses the scroll; a new cast removes the previous pair. (3) Removal sends S→C 0x0A to the operator only (the original's `0x0061A270(room, 2, GUID)` room notice is unwritten). (4) The preview has no quest records, so `object_quest_record` answers true; state 102 (`just_portaled`, rule 13) is not set. (5) Client: a right press on a page-0 inventory item sends C→S 0x20 with the player's point (`ui/panels.md` does not say what the original sends). Check: a trace of using a Town Portal scroll (S→C 0x51 / 0x60 / 0x82 of the pair, the 0x9D, the player-stop on entry and the removal after the way back).
- **REC-243** Town Portal gaps (PROVISIONAL, `d2-sim` `wiring/action/town_portal.rs`, `wiring/action/switch.rs`, `wiring/action/objects.rs`, `d2-server` `handlers/world/wired.rs`, M22): (1) an in-town cast: `0x0056D130`'s town rule is unwritten; d2rs opens the pair to the field level of the player's latest field cast (`PortalLinks::last_field_level`; the original's last-level source is unknown), near portal beside the player in town, far portal at that level's spawn point (tile 11); no field cast yet → nothing is made. (2) S→C 0x82: sent after the 0x51 of a class-59 portal from the object's owner GUID, the owner's join name and the pair's GUID (`View::portal_owner_from_links`); `Pending::portal_owner` still wins when a host answers it. (3) State 102 (`objects.md` §12 r13): stat list + event 12 + state on at `f + 75`; the remove callback `0x0056E900` and the event 12 body are unwritten, the list's expiry frees it; the synthetic app game has no states table so only the unit test sees it. (4) Vendor buy: a store item the buy took out of the NPC grid (not permanent) goes to the trading player as S→C 0x9C action 12 (`vendors.md` §7.1 r10); action 4 for the copy is not added here. Check: a trace of a Town Portal cast in town (0x51 / 0x82 of both ends, state 102 on arrival) and of a purchase (0x2A, then 0x9C action 12 / 4).
- **REC-124** Hit reaction `0x0057CEE0` in the preview (PROVISIONAL, `d2-sim` `wiring/action/reaction.rs`, M22): a surviving monster gets get-hit (mode 3) by a direct mode change toward the attacker, not the AI mode-request record; a hit the get-hit test (§6.2) absorbs, or a result with flag 0x4000, only queues the unit and sets unit flag 0x8000 (S→C 0x0C). Knockback / block (§7.1 4.1, 4.2, 4.4, 4.5), umod mode 4, the life-percent soft test (4.8) and the whole player branch's mode requests (§7.1 step 5) are not done; a player only gets the soft marking. Also the skill run-to (`use.md` §3) now uses the unit-form run request (`pathing.md` §1.2). Check: a trace of a monster hit by a player (S→C 0x0C / 0x69 timing, get-hit frame).
- **REC-104** Stash grid and the inventory beside it (PROVISIONAL, `ui/panels/stash_items.rs`, `ui/root.rs`, M22): without `inventory.bin` rows the stash grid is the measured record 12 / 28 corner (`panels.md` §Test vectors: 6 × 8, cell 29, left 74 / 154, top 82 / 142); the classic stash (record 8 / 24, 6 × 4) is not measured and uses the same corner. `panels.md` §11 r2 says the right half shows the inventory, but the gate refuses opening ui 1 while the stash is open, so `UiRoot::sync_states` draws the inventory panel whenever ui 0x19 is open. Check: capture of the stash open at 800 × 600 in a classic and an expansion game (grid corner, inventory half), `ui-0001`.
- **REC-105** Stash contents are not saved (PROVISIONAL, `app/save.rs`, q-stash): a played character's items, the stash included, are not written to the `.d2s` (the sim has no unit→save-record path wired to `write_save`, and `create_items` of a load is unapplied), so items put into the stash in a session are lost at exit and a loaded save's stash is not shown. A loaded save's item bytes pass through unchanged (stitch-save), so nothing is lost for a character that is not played with items. Needs: sim item unit → `StreamItem` → `write_save`, and `create_items` on load (`formats/d2s-load.md` §8.2), then a round-trip test (stash item in, save, load, same page-4 cell).
- **REC-108** Monster drop spot (PROVISIONAL, `app/monster_drop.rs`, q-monster-drops): the play host's free-spot seam has no path walk-back field, so a monster's drop lands on its start spot as is. Settle against 1.14d: kill a Blood Moor monster and record where its drop lands (`record_packets.py`, the 0x9C ground position).
- **REC-109** Cast target, animation names, line of sight (PROVISIONAL, `app/skill_rest.rs`, `app/anim_names.rs`, M22): (1) `use.md` §4 does not say where `0x0057FE90` / `0x0057FEF0` keep the mode start's point or unit; `UseRest::keep_target` keeps it per unit and `target` / `target_position` read it back (the skill's missile needs it). (2) The COF name of the server's animation lookup (`animdata.md` OQ2) is the client art's name (`unit_cof`: token + mode + bare-hand `hth`, upper case); the rate is the AnimData speed as is. (3) `line_clear` stays blocked (no collision on the seam), so skills with `lineofsight` > 0 do not start. Needed: OQ2's composer, the animation-rate spec, the target storage. Capture: a Sorceress casting Fire Bolt at a point: the 0x4D bytes, the missile's first position and the mana before / after.
- **REC-112** Waypoint travel in the preview (PROVISIONAL, `app/single_player.rs`, q-waypoint-travel): the synthetic Stony Field (level 4, waypoint 2) is not built at game creation; the 0x49 travel builds it on arrival through `level_warp` (`waypoints.md` §7 r5, `path-placement.md` §11). Whether 1.14d runs any extra level init on a waypoint arrival is unrecorded. Settle against 1.14d: take a waypoint to a never-visited field level and record the monsters/objects present on arrival.
- **REC-113** Identify effect (PROVISIONAL, `d2-sim` `wiring/inventory/identify.rs`, `pending.rs`, q-identify, M22): `inventory-moves.md` §7.18 step 5 calls the item-use dispatcher `0x005BF240`, whose table (`world/cube.md` OQ7) is unwritten for scrolls. The preview identifies on 0x27 when the used item's code is `isc` or `ibk`, the target is an item and not identified: item flag 0x10 and 0x1 set, owner update queued (0x9D action 0x15), the used scroll removed with the flag-0x20 removal message. Not specified: which `pSpell` index identify has, whether an already identified target refuses, the tome's charge decrement path, whether the server sends 0x9D 0x15 or another message to show the new properties, the Cain NPC case (`AppRest::identify` still logs only). Capture: use a scroll of identify on an unidentified magic item and record the S→C messages (`record_packets.py`).
- **REC-114** Item tool tip text and identify cursor (PROVISIONAL, `ui/item_tip.rs`, `ui/panels/inv_items_tip.rs`, q-identify, M22): no spec gives the item description text (the builder behind `0x0048DD90`) or the right-press that starts the identify cursor (`0x00468830` = 6). The preview builds name (quality colour), "Unidentified", Defense / Durability / Quantity / Required lines (English text, string ids unknown) and one line per property from `itemstatcost` `descfunc` 1–4, anchors the box at the mouse over the synthetic fill file, and sets the identify cursor client-side on a right press of `isc` / `ibk`. Needed for the spec: the description builder (line order, `descfunc` 1–27 shapes, `descpriority` sort, set / unique extras, string ids of the labels, colour per quality), the hover anchor and box pixels (`ui/text.md` §8), the use handler that sets cursor state 6. Capture: item hover boxes of a normal, magic, rare, unique, set and unidentified item (`ui/text.md` capture `text-0002`).
- **REC-242** Item tip shapes, set bonuses, shop tip (PROVISIONAL, `ui/item_tip_desc.rs`, `ui/item_tip_set.rs`, `ui/item_tip.rs`, `ui/shop_ui.rs`, q-item-tips, M22): `descfunc` 5–28 use the community-documented `itemstatcost` shapes in English (per-level lines show the raw value with `descstr2`, not scaled by character level; skill-tab and class names are fixed English; fixed phrases have unknown string ids). A set item's own set lists are green; under its set name the tip lists every partial step (green) and the full-set bonus (orange) whatever the player wears (computed with `d2_sim::items::props::set_bonuses`). A vendor item's tip adds `Price: n` and turns its white lines red when strength / dexterity / level (base stats) fall short. Right press on `tsc` / `tbk` sends C→S 0x20 like the cube. Needed: the description builder `0x0048DD90` / `0x004E60A0` (line shapes, which set steps show, colours, label string ids). Capture: `text-0002` hover boxes of a set item (worn 0/1/2/full), a vendor item the player cannot use, and items with each `descfunc`.
- **REC-115** Loaded-item placement is d2rs-own (PROVISIONAL, `d2-sim` `wiring/inventory/load.rs`, q-save-full; supersedes the item part of REC-105): `formats/d2s.md` §8.2 rule 3 (`0x00531210` / `0x00531520`) has no spec, so a loaded item is set to cursor mode and placed with the start items' calls: equipped at its body location, belt at slot x, otherwise the page its record names at (x, y); a taken or invalid saved cell takes a free position; an item with no place is freed (rule 3). Not done: the runeword refresh (§8.2 rule 5), item indices of hotkeys / mouse skills (`resolve_item_indices`), the corpse, the hireling's and the golem's items, and the stash gold. Needs: a spec for the placement routines, then a recorded save/load of equipped, belt, cube and stash items.
- **REC-110** NPC shop panel and store display (PROVISIONAL, `ui/shop_ui.rs`, `d2-server` `handlers/world/wired.rs` `flush_shown`, M22): `world/vendors.md` §4 step 3 says the client receives one 0x9C action 11 per store item, but no item spec writes the client's shop panel grid. Preview: the 0x9C action 0x0B records are the store; the grid is 10 × 10 cells of 29 px at (`sx` + 15, `H` + `sy` − 400) (the NPC's own `inventory.bin` record is not read); the trade epoch is a counter (`ClientWorld::store_serial`, `ItemRecord::seq`); a left click on a store item buys it without the `menus.md` §4.4 confirm dialog (the right click is the spec's quick buy); a cursor item dropped on the grid sells it (§4.5); repair all is the 18-frame button, single-item repair is not wired. The buy price shown is computed by the server host (`VendorRest::store_price` → `ShopPrices`) because the preview client has no price tables. The shop opens when store items arrive, for the trader nearest to the player. Needed for the spec: the shop grid record and its position, the confirm dialog, the button hit areas, and a recording of the 0x9C action 11 stream. Check: screenshot of the Gheed / Charsi shop at 800 × 600 and the S→C 0x9C bytes of a trade open.
- **REC-116** Client missiles and overlays (PROVISIONAL, `world_view/missiles.rs`, M22): the real client creates its own missiles from the skill messages (`intents-events.md` §7.6 r1) but the client skill start `0x004C6F40` / `0x004C6EB0` and the client missile body (`overlay.md` `0x004CF3C0`) are not specified. Filled: the skill's `cltmissile` starts at the cast request (no delay to the action frame, no skill level), flies straight at `Vel` · 4096 (16.16 subtiles / tick, `missiles.md` R4.1 read with a 12-bit direction), `Range` ticks, ends at the first living monster within one subtile, then plays `explosionmissile`. Art `data\global\missiles\<CelFile>.dcc|.dc6`, overlays `data\global\overlays\<Filename>.dcc|.dc6`; opaque blend, no light, pass 6 after all units; castoverlay once on the caster; each state's `overlay1` looping. Needed: the client skill start, the missile client functions, the overlay draw spec, `Trans` blends. Capture: a Sorceress casting Fire Bolt at a point: the missile's screen position per tick and the frame index.
- **REC-119** Horadric Cube in the play preview (PROVISIONAL, `d2-sim` `wiring/inventory/cube_open.rs`, `items/moves/handlers.rs`, `d2-client` `ui/cube_ui.rs`, `ui/panels/cube_items.rs`, q-cube, M22): the item-use dispatcher `0x005BF240` / table entry 7 `0x005BF0C0` (`world/cube.md` OQ7) is unwritten for the use path, so C→S 0x20 on an item of code `box ` opens the cube directly: stash interaction cleared with S→C 0x77 0x11 if open, interaction (type 4, cube GUID) set if none, S→C 0x77 0x15, the cube is not consumed. The client sends 0x20 on a right press of a stored `box ` with the player's subtile position. The cube grid is `inventory.bin` record 9 (+16 at 800 × 600), with an estimated 3 × 4 fallback (left 116, top 130) when the record is missing; the transmute animation (`panels.md` §12.4), the tool tips and the cube-gone close are not done; the play host's `ItemPending` (`PreviewCubePending`) does nothing (no copies, tempered affixes, repair / recharge, quest hooks, Cow portal). The cube is not in a new character's start items. Needs: the item-use spec, a capture of a cube open (0x77 order, inventory pass `0x0055FA40`), the record 9 / 25 grid corner, and a recorded transmute (`world/cube.md` test vectors).
- **REC-244** Cube gaps in the play preview (PROVISIONAL, `d2-server` `WiredWorld::start_extra`, `d2-client` `app/single_player.rs`, `ui/cube_ui.rs`, q-cube-gaps, M22): charstats gives no Horadric Cube, so the play host names `box ` in `WiredWorld::start_extra` and a new character gets one copy in the inventory after the charstats start items (loc 0, count 1, the same §10.3 creation; extra game-seed draws follow the original's). The cube-gone close (`panels.md` §12 r2, `panels-2.md` §20 r4) runs once per UI pass in `OriginalUi::cube_poll`: ui 0x1A open and no stored `box ` in the model → `SetUIState(0x1A, off)` and 0x4F 0x17 twice. The button tool tips (`strClose` 4144, `strUiMenu2` 3341, positions `cube_tooltips`) draw in font 1 when the string table has the id (font is d2rs-own). Needs: the cube's real source (Act II Horadric quest reward, REC-119), a capture of the tool tip font, a capture of a cube leaving the inventory.
- (REC-119 note) the "not in a new character's start items" and "cube-gone close" gaps are closed by REC-244.
- **REC-120** State-change messages for players (PROVISIONAL, `d2-sim` `wiring/action/state_update.rs`, q-states-auras, M22): `intents-events.md` §3.5 rule 6 / §7.3 rule 2 step 8 give 0xA7 / 0xA8 / 0xA9 per changed state for a monster's update only; the player's update step list is not specified. d2rs sends them for players and monsters in the same per-client pass (`dispatch.rs` `send_unit_update`), skipping a unit new to the client (its 0xAA carries its states). The combat adapter's `set_state` queues the unit (as `0x00639DB0`) so cold / stun / poison toggles are sent. Not done: unit tints for the colour states (needs the state-colour spec), the client-side `0x004D9E60` hooks. Needs: the player update step list, a recording of a Paladin aura on / off (0xA8 bytes).
- **REC-121** Socket filler test and socketed tool tip (PROVISIONAL, `d2-sim` `wiring/inventory/units.rs`, `ui/panels/inv_items_socket.rs`, q-sockets, M22): `inventory-moves.md` §7.19 names the socket-filler test `0x0062BEB0` but no spec writes it. Read as: the item is a gem (20), rune (74) or jewel (58) (`properties.md` §9 names gem and rune; jewel is the third socketable). The same test drives the client's cursor decision (`inventory.md` §6 r5: socketed + identified + flag 0x100 clear + filled < sockets). The tool tip lists, under an item's own lines, each filler's name (orange) and its blue property lines; the original's layout of filled sockets is not specified. The filler's `ListKey::ITEM` list is attached to the target by `StatLists::equip` (`stat-lists.md` §8.4, read for `properties.md` §9 rule 4); the target's stats reaching a wearer are the equip wiring's, not done here. Needed: the `0x0062BEB0` test, the tip layout for filled sockets. Capture: hover a socketed item with a gem and a runeword item.
- **REC-122** Act change in the preview (PROVISIONAL, `d2-sim` `wiring/path/act_change.rs`, `d2-server` `hireling_host.rs` `act_changes`, M22; q-act-travel): `0x0053ACC0` has no owner spec (`impl-world-rest.md` G8). A level warp (waypoint C→S 0x49, or the quest/NPC `act_change` queued through `LifecycleHooks::request_act_change` and run when the call returns) into another act sends, in `waypoints.md` OQ1's order, the old act's removals (client room switch to no room), S→C 0x05, 0x03 (init seed of the act, its town, object seed), then places the player at spawn tile index 0. Not recorded: S→C 0x53 / 0x5D, the `arg` of `0x0054B830` (0 / 5; read as tile index 0), how the original brings the local player back (here: S→C 0x59 + 0x0B after the placement, then 0x04 LoadComplete; the removals sent 0x0A for the player's own unit), and the other steps of `0x0053ACC0`. The waypoint panel opens every act tab (client quest gate unknown, `msg-ui.md` OQ 4). Check: a recording of Warriv's Go East and of a waypoint to another act (S→C order, 0x0B/0x04 presence).
- **REC-123** NPC menu box is d2rs-own (PROVISIONAL, `ui/npc_menu_ui.rs`, q-npc-menu, M22): the menu opens when S→C 0x28 is delivered (the 0x2F / 0x31 of the dialog branch go out as before); `menus.md` §2.2 gives the build (`0x004B4830`) but not the open trigger, the Talk handler's sends (`panels.md` §14.1 open: 0x2F vs 0x30) or the box anchor from the NPC's screen point. The preview draws plain text rows at a fixed anchor with English labels when the string table has no id, Talk shows the text list's kind-0 strings and sends nothing, Hire opens the hire list, Trade sends C→S 0x38 action 1, the cancel row or a press outside sends C→S 0x30 (the automatic chat close of REC-106 is off while the menu is up). Needed: the open trigger, Talk's handler bytes, the anchor and a recorded menu click sequence (0x38 / 0x2F / 0x30 order).
- **REC-125** Summons and pets on the preview host (PROVISIONAL, `d2-sim` `wiring/interaction/summon.rs`, `d2-server` `hireling_drive.rs`, q-summons, M22): the summon bodies (`bodies.md` §6.2, §8.1, §8.2) ran into `Pending::create_monster` / `body_effect`, which the host never answered, so no summoned unit existed. d2rs now allocates the monster itself (on the aimed point; the spawn-spread search is not specified), lists it with `pets.md` §2–§6 over `ActionHooks::pet_lists` and sends 0x7A add / remove; a pet that died or vanished leaves its list in `View::pet_sweep` (the original does it in the kill's `0x005751A0`). The resync `0x00575900` is a no-op, so a list keeps the maximum of its last add. The pets follow and fight by the hirelings' stand-in think (REC-100). `ActionHooks::bodies` is now installed on live data (pettype count and group). Not done: pet owner link for the real AI (`OwnerData`, `NodeInsert`, `AiRefresh` still reach `Pending::body_effect` and do nothing), golem items, raise-skeleton corpse removal, player death clearing the pets, mana upkeep, pet life bars. Needs: a recording of Raise Skeleton / Summon Grizzly past the maximum (0x7A bytes, which units die), `pets.md` OQ3.
- **REC-126** Hardcore and the save at death in the play preview (PROVISIONAL, `d2-client` `app/hardcore.rs`, `app/save.rs`, `app/single_player.rs` `LocalSeams`, q-hardcore, M22): the character's hardcore flag is the save's status bit 0x4 (or `play --new --hardcore`), set as client flag 4 of the local client, so C→S 0x41 drops the client with reason 3 instead of respawning (`handlers::player::resurrect`). The original saves at the DD start (`vitals.md` §4.8 r2, `0x00532400`); the preview saves from the app when the death screen comes up (the DT start; sim penalties applied), writing the dead bit 0x8 for a hardcore character, which the next load refuses (`d2s.md` §2.2 r5). Leaving a dead hardcore character closes the game (no character screen yet). Not done: the corpse pickup click, items and gold piles on the corpse and the gold drop (`death_drop_gold`, `stash_cap` keep their defaults), the ear drop. Needs: a save capture at a death (HIGH PRIORITY, saved bytes) and the original's hardcore death flow (drop reason 3 screen).
- **REC-127** Game messages in play (PROVISIONAL, `d2-client` `ui/game_messages.rs`, q-messages, M22): the screen message list (`messages.md` §2) is fed by S→C 0x26 types 1, 2, 4, 6 (`chat_action`, §3 table) and the 0x5D quest rows 3 / 15 (strings 3708 / 3710, colour 0). The expiry clock counts 40 ms per client frame (`world.frames`; the original reads `GetTickCount`). The 8-bit text is read as Latin-1 (REC-ui-chat-filter); the text backing is the HUD dark fill tiles, not `DrawRectangle(…, 0, 1)`. Not done: overhead bubbles (needs the unit's screen position; the record is kept by `chat_line`), the 0x5A event text lines (`msg-ui.md` §19 r3 gives no formats: `ui/*`), NPC text and dialog panels, the recipe scroll. Needed: the 0x5A line formats; a capture of a chat line against the screen position and the translucent backing.
- **REC-128** Cain's quest on the wired host and Cain's identify (PROVISIONAL, `d2-sim` `wiring/economy/quest_host.rs`, `wiring/inventory/identify.rs`, `d2-server` `world/wired.rs`, `d2-client` `app/rest.rs`, q-a1-cain, M22): the quest world's `spawn_monster` / `spawn_monster_flags` (`0x005B2F20`), `remove_monster` (`0x005A7E60` + `0x005A7C20`), `find_object_near`, `create_object` and `open_portal` (`0x0056D130`) went to the app's rest, which only logged. `HostQuests` now answers them in a DRLG room: a monster unit allocated through the action wiring (NPC classes allied), removed at once with S→C 0x0A to every player (the removal mode's animation is not modelled), a portal object in mode 1 with `interact` = level and the owner's GUID, the object search over the whole act (the spec says the room list). Spawn spread and spawn flags are not applied. The Tristram/town Cain placement is the quest code's (`quests-act1-rest.md` §1.2, §3). Cain's identify (C→S 0x34): the NPC call stages each player's inventory entries on the rest (`NpcRest::stage_inventory`, from `InvDesk::npc_entries`: place from the item's mode and page), `AppRest::identify` records the items, and the host applies them on the inventory model (`InvDesk::identify_unit`: REC-113's flags and 0x9D 0x15 update). Not specified: `0x00578460`'s per-item effect beyond the flag, the 0x9D order against the 0x2A. Needed: a recording of a Cain identify (R-NV-2) and of the Tristram portal / Cain rescue (OQ3, OQ12).
- **REC-129** Quest events in the play host and the synthetic Tower line (PROVISIONAL, `d2-server` `wired/quest_events.rs`, `d2-client` `synthetic_tower.rs`, `single_player.rs` `LocalSeams`, M22; q-a1-tower): the original raises the quest control's monster link (`0x005436B0`), kill parse (`0x00543A30`) and level change (`0x00543B90`) from inside monster init, the kill and the warp; no spec fixes where in the frame. Here the seams queue links and kills (`Pending::monster_quest_chain`, `kill_step(QuestKill)` → `take_quest_events`) and `WiredWorld::after_tick` runs them on the quest control once per tick (links, then the level changes read off the players' room levels, then kills). Quest chains live in `AppRest::chains`. The level-to-level `Quest` link (`0x00545CD0`, `attach_quest_chain`) is not wired. Synthetic only (d2rs-own, unverified): Black Marsh (6) behind a Blood Moor warp pair, the Moldy Tome (objects row 60, operate 6, init 4) in its room, Tower (20) and Cellars 1–5 (21–25) as maze levels with warp pairs 15–28, no population there (the Countess has to be spawned by hand). Needed: a trace of the Countess kill (0x5D/0x27 order, quest-event timing against the death mode) and of the tome read.
- **REC-132** Sisters to the Slaughter in the preview (PROVISIONAL, `d2-server` `handlers/world/wired/quest_events.rs`, `d2-client` `app/rest.rs`, q-a1-andariel, M22): no live caller ran the quest kill parse `0x00543A30` or the level-change event `0x00543B90` (`wiring/action/dispatch.rs` TODO), and the client rest answered no unit chain, level or monster kind. d2rs now queues both in `ActionHooks::quest_events` (the kill at the kill's quest step, the level change when the client's player room has a different level id than the last noted, old = 0 before the first) and the host drains them after the tick on the quest control; they run when the tick returns, not inside the kill / client update. Andariel (class 156) has no spec'd link to chain 6 (`quests.md` §4.6 names only levels 8 and 74 for the monster-creation link; "quest code attaches links directly"), so a kill of class 156 links her to chain 6 first. The preview rest keeps unit chains, levels (last sync) and monster classes in `AppRest`. The synthetic game gets Catacombs 4 (level 37, a flat level, reached by a level warp in the test) and Warriv (class 155) as an interactive NPC class. Not done: the Catacombs 1-4 stairs chain on the live data (real DS1 warps), Andariel's poison missiles and AI wiring on live data (the AI body and function exist, `ai.md` §9.12), the portal timer's visible portals, the quest log row. Needs: a recording of the Andariel kill (S->C quest flags 0x5D / 0x28 order, the portal object) and `quests.md` OQ on the link.
- **REC-133** Act II synthetic set (PROVISIONAL, `test-fixtures` `act2.rs`, M22; q-a2-fields): `Drlg::create(1, ..)` over an Act II-shaped set runs the real placer (A2 / A2C rows) and the §8 generator for levels 41–46. The ids, DrlgTypes, desert LevelType 16, the sizes and the absolute offsets of levels 40 and 46 are the spec's; names, one 8 × 8 floor cell for every lvlprest id 2..=413, `Files` counts, the lvlsub row and the monster columns are made up. Waypoint rooms are checked as room flags only (the outdoor waypoint's object comes from the lvlsub / lvlprest content, not synthesised). Act II population and a player walk (`Session` and `GameData::drlg_world` are act-0 only) are not covered. Needed: a local run of Lut Gholein → Rocky Waste → Dry Hills on the user's files (waypoint object present and operable, monsters spawned).
- **REC-130** Sisters' Burial Grounds in the play preview (PROVISIONAL, `app/synthetic_burial.rs`, q-a1-bloodraven, M22): the synthetic game has a one-room Burial Grounds (level 17) reached from the Blood Moor (the original: Cold Plains), a host-placed Blood Raven (`HOST_MONSTER_PRESET` in the level types' preset list, class 267, fixed sub-tile, quest chain 2, no AI, no minions; with game files population creates her and her boss mods link the chain), Kashya in the synthetic town with made-up `hireling` rows (the synthetic game has no `HirelingTables`, so the reward marks a slot hired but creates no unit), and the quest updater `0x00543E10` run on every 20th frame from the host (`quests.md` §5 says tick step 8 at frame % 20 = 0; where in the frame is the host's choice, REC-129). Needed: a local run on the user's files (Kashya's two talks, the level 17 arrival, Blood Raven's death and the reward) and the monster-flag `0x80000000` guard of the kill parse (`damage.md` §7.2 step 3), which the sim's `kill` does not read.
- **REC-134** Every act in the play-host session (PROVISIONAL, `test-fixtures` `host.rs` / `game.rs` / `act2.rs`, M22; q-act-worlds): `Session::new_in_act(d, setup, act)` and `GameData::{world_sim_act, drlg_world_act}` create and join in any act 0..=4 (`Session::new` and the old entry points are act 0, so no `Setup` literal changes). The Act II town (Lut Gholein) gets Act I's waypoint record (made up; the real town's waypoint position is not synthesised). Population in the desert uses `isSpawn` on the set's monsters (made up). Needed: a local run on the user's files (Lut Gholein → Rocky Waste → Dry Hills).
- **REC-139** Act III dungeons on a synthetic set (PROVISIONAL, `test-fixtures` `act3.rs`, M22; q-a3-dungeons): an Act III-shaped set with Kurast Docks (75) as the preset town and the dungeon levels by their 1.14d ids and level types (Spider 84/85 type 23, Swampy Pit / Flayer Dungeon 86–91 type 24, Sewers 92/93 type 25, Durance of Hate 1/2 type 22) built by the real maze generator; the temples 94–99 and Durance 3 (102) as one-cell preset levels. Made up: names, the 24 × 24 floor cell for every def, `lvlmaze` rows (`Rooms` 6/8/10, `Merge` 500, level rect 200), the vis chains, the town waypoint, no outdoor Act III (`TownOnly` creation: the jungle placer needs block data). Checked against the spec: Spider levels 4 cells (ring(2), no stamps), Sewers 1 18 cells (ring(5) + two stamps), Dungeon levels grow past the ring. Not done: a player walking in (the entrances are DS1 warp units of the outdoor levels; the host has no warp), the Spider Cavern per-def vector (defs are not kept on built rooms), the stamped objects (waypoint, chests). Needed: a local run on the user's files (Kurast → Spider Forest → Spider Cavern, Great Marsh → Swampy Pit, Lower Kurast → sewers, Durance of Hate 1–3).
- **REC-140** Lut Gholein in the play preview (PROVISIONAL, `d2-client` `app/single_player.rs` `ACT2_NPCS`, q-a2-town, M22): the synthetic Act II town room holds Warriv (175), Atma, Drognan, Fara, Greiz, Jerhyn (201), Elzix, Lysander and Meshif (210) in a row (4 sub-tiles apart) and a waypoint object; the live game's NPCs and waypoint come from the town presets (`ds1`), so the positions are d2rs-own. Greiz gets one synthetic `hireling` row (seller 198, names 2000–2002) so his hire list can be made. Not done: the harem / palace / sewer entrances (level warps and their levels are not in the synthetic DRLG), per-NPC menus with real vendor stock (needs game files), Jerhyn's palace position and Meshif's boat. Needs: a game-file run of the Act II town (`play` with a Lut Gholein save) to see the real NPC placement and menus.
- **REC-141** Corpse and death losses in the play preview (PROVISIONAL, `d2-sim` `items/moves/ground.rs` `corpse_fill`, `wiring/inventory/pending.rs` `corpse_taken`, `wiring/action/death.rs`, `d2-server` `world/wired.rs` `corpse_fill`, `d2-client` `world_view/corpse_click.rs`, `app/save_full.rs`, `app/hardcore.rs`, q-corpse, M22): at the corpse creation (`vitals.md` §4.7 rule 1.7) the cursor item goes into the corpse's grid and each body item onto the same location of the corpse; grid and belt items stay on the player; an item the corpse cannot take stays on the player (the original drops it near the player, not done). The death's gold drop (§4.6 rule 1) makes gold piles near the player (`gold_piles`, the preview rest's free spot); the corpse's own gold piles (rule 1.8) are not made (stat 14 is already 0 by then). The client click on a dead player unit (mode 17, not the local player) sends C→S 0x16 type 0 (`inventory-moves.md` §7.1; the task text said 0x13, the spec routes the corpse take-back through PickItem), walks to the corpse and asks again like the ground-item click (the server's walk toward a unit is a seam). The take-back (§12.2) is the sim's; on success `corpse_taken` sends S→C 0x8E (action 0) and 0x0A to the player, frees the corpse unit and its inventory. d2rs-own: S→C 0x0A for each item that leaves the player for the corpse (no spec states the client view of these moves). The character is saved at the DD start as well (`save_at_dd`, in addition to REC-126's DT save), with the corpse's items in the d2s corpse section (`d2s.md` §8.3; x = y = 0 as measured in rule 6, the unknown u32 0 per rule 3; one corpse per owner; the loaded corpse section is not made again at load: `create_corpse` stays unapplied). Not done: the ear drop, the status bit 0x8 on a softcore death (rule 6 note), the hireling corpse, other players' clients told of the corpse removal, the pickup by a party member (`corpse_loot_allowed`). Needed: a save capture of a death with items on the corpse (HIGH PRIORITY, saved bytes) and a trace of the 0x9C/0x9D/0x0A messages at a player's death and corpse pickup.
- **REC-143** Act IV in the play preview (PROVISIONAL, `d2-client` `app/synthetic_act4.rs` and `app/single_player.rs`, q-a4, M22): the synthetic game gets act index 3 with the Pandemonium Fortress (103) holding Tyrael (367), Jamella, Halbu and Cain (246) in a row and a waypoint (levels row waypoint 27), then flat one-room levels Outer Steppes, Plains of Despair, City of the Damned, River of Flame, Chaos Sanctuary (104–108) joined by warp pairs (lvlwarp ids after `synthetic_act2::last_warp()`) in walking order; Izual (256) is a host-placed preset in the Plains and carries chain 22 (`host_monster_created`), so his kill reaches A4Q1 (state 4) through the shared kill/quest event path. `LevelSource.acts` is now a `Vec`. All places are d2rs-own; the original builds the outdoor levels from the act placer and the Fortress from a town preset. Not done: Hephasto / Hellforge (A4Q2 objects), the Chaos Sanctuary's seals, seal bosses and Diablo (A4Q3), Tyrael's talk-driven quest start (0x31 scroll reply), Act III as a real predecessor (the act change is the hooks queue). Also (d2-sim `wiring/path/monsters.rs` `monster_attack_start`): the monster Attack/skill start `0x005A75C0` now clears unit flag 0x40 before the skill start (`units.md` §4.6 rule 7 states no flag step; the player starts and the SQ start clear it). The attack do sets it (`skills/bodies.md`) and the per-frame event runs the do only while it is clear, so a monster whose first attack missed never attacked again (found on game seed 9). Needs: a trace of a monster's second and third attack (does unit +0xC4 bit 0x40 clear between attacks?), and a game-file run of Act IV (`play` with an Act IV save).
- **REC-166** Act IV endgame in the play preview (PROVISIONAL, `d2-sim` `wiring/economy/quest_objects.rs`, `quest_host.rs`, `world/quests/act4/q2.rs`, `d2-server` `wired/quest_events.rs`, `d2-client` `synthetic_act4.rs`, q-a4-endgame, M22): the Act IV quest objects now run (init 48, 55, 56, 59, 78 and operate 49, 52, 54, 55, 56, 73 by `object-functions.tsv` address; init 59, the seal-boss dummy's first events, `quests.md` §9 rule 7, is new in `q2::dummy_init`). `HostQuests` answers the seams the endgame needs (`object_frame_count1`, `unit_dead`, `alignment`, `level_monsters`, `spawn_quest_object`/`place_object` through the object state, `set_room_portal` = `refresh_room`, `client_idle`, `act_change` = the host's act-change queue, `superunique_id` = identity for 36–38, `preset_superunique_spawn` = a plain monster of the `superuniques` row's class, or of class = id when the game has no rows, linked to chain 23). Diablo (243) and Hephasto (409) get their chain 23 / 24 links by class in the kill path, as Mephisto's (REC-142); the real links belong to monster creation. The synthetic Chaos Sanctuary room is 40 x 40 tiles so the seal bosses' offsets (up to 52 sub-tiles) fit; objects 131, 255, 376, 392-396, 566 are synthetic `objects` rows and 243, 409, 36-38 are killable classes. Not done: the soulstone / hammer / gem items (the synthetic game has no `mss `/`hfh `/gem rows: the Hephasto test only sees the hammer drop asked for), `wielded_weapon_code`, the Harrogath waypoint activation (`0x005B4FF0`, reported), Tyrael's portal spawn (message 20000), the classic end-of-game timer, and the Fortress to Harrogath relocation: the host's act change reports the placement `Ok(true)` but the player stays in the Fortress room in the synthetic game (act 3 to act 4; act 0 to act 4 works), so the portal test stops at the queued act change. Needs: a trace of the seal order and Diablo's spawn tick, and a game-file run (`play` with an Act IV save).
- **REC-175** Act IV to V in the play preview (PROVISIONAL, `d2-sim` `wiring/path/act_change.rs`, `wiring/economy/quest_host.rs`, `d2-client` `single_player.rs`; q-a4-harrogath, M22): (1) the cause of REC-166's "player stays in the Fortress room": the act change placed the player with the old path room still set, and the room recache (`0x0064FAD0`) keeps the old room when the cell lies in its rectangle; acts share tile coordinates, so the synthetic Fortress (act 3) and Harrogath (act 4) collide at the spawn point (act 0 to act 4 never overlapped). `act_change::run` now takes the unit out of its old room first (footprint, room list, path room none; the original's `0x0053ACC0` removes the unit from the old room, spec owner missing, `impl-world-rest.md` G8; d2rs-own, unverified). (2) `HostQuests::activate_waypoint` (`0x005B4FF0`) sets the level's waypoint index (from `ActionTables::levels`, filled in the synthetic game) in the host's records of the game difficulty; the portal's report is gone. Not done: the soulstone / hammer / gem items (the synthetic game still has no item tables or inventory model: `has_item` reads `econ.items` through the inventory, so the flow needs `ItemTables` rows for `mss `, `hfh ` and the gems plus `InvTables`, which also turns on the start items and every item path in the preview), `wielded_weapon_code`, Tyrael's portal spawn, the classic end-of-game timer.
- **REC-144** Harrogath in the play preview (PROVISIONAL, `d2-client` `app/single_player.rs` `ACT5_NPCS`, q-a5-town, M22): the synthetic Act V town (level 109, act 4, waypoint 30, `world/waypoints.tsv`; 27 is the Fortress, 35 Halls of Death's Calling (q-fix-prov-waypoint-index)) holds Larzuk, Anya, Malah, Nihlathak, Qual-Kehk and Cain in a row (4 sub-tiles apart) and a waypoint object; live positions come from the town presets, so these are d2rs-own. Qual-Kehk gets three synthetic `hireling` rows (act 5, class 560, names 3000–3002). Not done: Larzuk's socketing end to end, vendor stock, Nihlathak's palace. Needs: a game-file run of Harrogath.
- **REC-157** Act 2 / 3 / 5 mercenaries in the play preview (PROVISIONAL, `d2-client` `app/merc_rows.rs`, `app/rest.rs` `owners`, `app/single_player.rs` `MERC_CLASSES`, q-mercs-acts, M22): the synthetic game had `hireling_tables: None`, so no hire created a unit (`NoHirelingTables`). It now holds one made-up `hireling` row per synthetic hire row (Kashya, Greiz 198, Asheara 252, Qual-Kehk 515; `Id` = row order, stats/growth/skills made up, no aura or skill rows) so the offer the list shows and the init rolls share a row; the three mercenary classes (271, 357, 560) are plain monster rows on the client. The three non-Kashya rows are priced 0: the synthetic game has no gold stat row, so nothing could pay. The preview rest now stores `set_owner` (it only logged, and `owner` answered `None`), which the kill path (`life::on_kill`) needs to mark a merc dead and let Kashya / Greiz / Asheara / Qual-Kehk resurrect it (C→S 0x62). Not done: auras and skills of the real `hireling.txt` rows (live data only; unseen), mercenary equipment, the rest `warp_to` (the merc is not warped on a player teleport), XP share. Needed: a game-file run hiring in each act and checking the merc's skill/aura (Defiance, Prayer, Might...), its equipment and a resurrection price.
- **REC-162** Gamble windows and repair in the play preview (PROVISIONAL, `d2-client` `app/rest.rs`, `ui/shop_ui.rs`, `ui/npc_menu_ui.rs`, `d2-server` `world/wired.rs` `flush_shown`, `d2-sim` `vendors/trade.rs` `repair_all_quote`, q-gamble, M22): `AppRest::place_in_store` and `place_in_gamble` answered "no room", so in `play` no store and no gamble list was ever placed (the e2e rest answered true, which hid it); both now answer true (the preview has no NPC grid model). The shop panel packs the shown items into its 10 x 10 grid itself, row by row in arrival order, instead of reading the server's positions (the rest sets none). The Gamble menu row marks the next shop as a gamble window (`ShopState::gamble`), so a click sends C→S 0x32 with transaction bit 2 (`menus.md` 4.3; before, the flag was fixed false and the server refused the buy). The host publishes the gamble price (`vendors.md` 9.4, transaction 2) for a gamble window's items, and, for a repairer, the repair-all total under item GUID 0 (`ShopPrices`, `REPAIR_ALL_KEY`); the total is drawn above the button bar when a font is bound. Not done: the single-item repair button (frame 6; needs the inventory panel's clicks), the confirm dialog, the original's price text layout, gamble items' identify on buy, and Gheed / Charsi / Elzix / Jamella are not placed in the preview towns (Act 1 has only Akara and Kashya; q-a2-town / q-a4 place the others). Anya (512), Alkor (254), Larzuk (511), Hratli (253) are in the play towns. Needed: a game-file run opening Anya's or Alkor's gamble window, buying one item (price vs the shop's number, the item unidentified) and repairing a damaged equipped item with the repair-all button.
- **REC-131** Imbue on the real inventory (PROVISIONAL, `d2-sim` `wiring/interaction/npc_items.rs`, `d2-server` `wired.rs` `desk_with`, q-a1-malus, M22): the NPC item services (`npc.md` §8.1: cursor item, item facts, remove from cursor, create, place) ran on the rest, which refused all of them. The wired host now lends the inventory model to the NPC desk (`Desk::inv`, `NpcInventory`) for the NPC call. The imbue creates a new item from the input's record with the drop request of §8.1 (item level = base level + 4 if above 5, quality 6, `flags2` 0x20 | ethereal 4 / else 2) on the economy; the input is freed after; the new item is placed in the backpack at a free spot. d2rs-own, unverified: `0x00558270` copies only the record (not affixes, durability, sockets or the personalized name, the rest still holds the name), no free spot leaves the item on the rest's `place_or_drop` (ground drop unwired), a refusal leaves the item on the cursor (`put_back` `0x00563C00` unwritten), `nameable` is never set, `max_sockets` in the facts is 0. Needs: the `0x00558270` field list, a recording of the imbue (0x58, 0x9D order and the new item's mode).
- **REC-136** Act II quest hooks in the play host (PROVISIONAL, `d2-server` `wired/quest_events.rs`, `d2-client` `single_player.rs`, `handlers/world.rs` `PreviewCubePending`, M22; q-a2-quests): Radament's and the Summoner's AI calls (`QuestCall`) and the cube's `hst ` hook are queued as `QuestEvent`s and run on the quest control once per tick after the steps (`0x00599420`, `0x0059C330`, `0x0059E5C0`), not inside the AI / cube call as the original does; tests stage the events at the d2-server level (no Act II play world yet, q-act-worlds). Not verified: kills of Radament / Summoner / Duriel through the live path, the Book of Skill use (`use_book_of_skill`, no item-use caller), Jerhyn / Meshif travel east. Needed: a local run Lut Gholein → Sewers 3 (Radament) and Arcane Sanctuary (Summoner) with the quest log open.

- **REC-138** Act III synthetic set and play walk (PROVISIONAL, `test-fixtures` `act3.rs`, M22; q-a3-fields): `Drlg::create(2, ..)` over an Act III-shaped set runs the real jungle placer and Kurast chain, and the generator builds levels 76–83. The ids, DrlgTypes, LevelTypes 20 / 21 / 22, the §1 sizes, the docks offset, and the jungle pieces' lvlprest rows (32 × 32 blocks, 64 × 32 head and tail, `Files`) are the spec's; the Kurast pieces' sizes (one 8 × 8 cell, causeway 48 × 16, Travincal 16 / 32 × 32), all preset content (floor only), the floor tile for main index 1 and the monster columns are made up. The docks get Act I's waypoint record (made up). No waypoint object exists in the outdoor levels (Act III runs no waypoint placer; their objects come from real preset content, not synthesised), so waypoint travel is untested. Needed: a local run on the user's files (Kurast Docks → Spider Forest → ... → Travincal; waypoint objects in Kurast, monsters, no `rejected` lines).
- **REC-147** Act 2 dungeons in the play preview (PROVISIONAL, `d2-client` `synthetic_act2.rs`, `synthetic_maze.rs`, `single_player.rs`; `d2-sim` `DrlgWorld::staff_tomb`, `HostQuests::true_tomb_level`; M22; q-a2-dungeons): the maze generator, the Act II tomb choice (`levels.md` §3.4) and the tomb/Sanctuary special builders (`maze.md` §5.4, §5.5, §6.2, §6.3) were written and tested; the preview world had no Act 2 level behind Lut Gholein. Synthetic only (d2rs-own, unverified): the sewers (47–49), Halls of the Dead (56, 57, 60), Claw Viper Temple (58, 61), Maggot Lair (62–64), Stony Tomb (55, 59) and the Arcane Sanctuary (74) hang off Lut Gholein, Tal Rasha's tombs (66–72) off a flat Canyon of the Magi stand-in (46) behind Lut Gholein (the original reaches them through the fields, the Palace Cellar and the Canyon); warp ids 29 up, tile places, one first room per maze level carries the ways back and on. The true tomb (DRLG +0x94, `0x0061AEB0`) is now read from the Act II DRLG by `HostQuests::true_tomb_level` (0x3E `trs ` clue → 0x50) and by `LocalSeams::object_staff_tomb` (orifice). Duriel's Lair (73), the boss-tomb entrance, the orifice/staff flow and population in these levels are not wired. Needed: a trace of one Act II game's tomb choice and the clue message.
- **REC-137** Act III town in the play preview (PROVISIONAL, `d2-client` `app/town_npcs.rs`, `single_player.rs`, q-a3-town, M22): the synthetic game can place any list of town NPC classes (`build_with_town`); the Act III set (Ormus, Asheara, Hratli, Alkor, Natalya, Meshif, Cain) and Asheara's `hireling` rows (class, price, level, names) are made up. The preview rest's `approach` only logs, so a walk that ends 7–8 sub-tiles from an NPC (talk distance 6, approach 8, `npc.md` §2) never starts the talk until the player clicks again from closer. Needed: a local run in Kurast Docks on the user's files (every NPC's menu, Hratli / Ormus / Alkor stock, Meshif's sail) and the host walking the player on `approach`.
- **REC-142** Act III quest glue in the play preview (PROVISIONAL, `d2-sim` `wiring/economy/quest_objects.rs`, `d2-server` `wired/quest_events.rs`, q-a3-quests, M22): the Act III object init/operate functions (`quests-act3.md` §1.4: inits 23, 25, 39, 41–45, 49, 50, 52, 53, 60; operates 28, 31, 44, 45, 53, 57–59) are routed to the quest control; an init that needs a point (decoy, altar, wanderer, Hratli) does nothing for a null room. A monster of Mephisto's class (242) is linked to chain 20 by class when it dies (no spec says where chain 20's link is added; as Andariel's REC-132). Not done: the Council members' and Gidbinn's links, the Durance warp call, Natalya/Hratli spawns have no live object to run on in the synthetic world (no Act III levels beyond the town NPCs of REC-137), the red portal to Act IV. Needs: a game-file run of Act III (Kurast Docks, Travincal, Durance of Hate).
- **REC-145** Charsi's imbue dialog in the play preview (PROVISIONAL, `d2-client` `ui/imbue_ui.rs`, `ui/npc_menu_ui.rs`; M22; q-imbue-ui): the server imbue worked (REC-131) but `play` could not send it. Charsi's NPC menu gets an Imbue row (string 4017, `OptionKind::Imbue`, a runtime insert; `panels.md` §14 leaves the imbue insert open and the record in `npc-menus.tsv` is unchanged) that is always offered, and the row opens a plain-text dialog at the geometry of `messages.md` §11 (item area 123–211 × 106–220, imbue button 122–154 × 224–256, close 177–209 × 224–256) inside the NPC menu panel. A click in the item area with a cursor item places its GUID (the NPC's accept check is the server's), the imbue button sends C→S 0x38 `[0][NPC][item]`, the close button C→S 0x30; S→C 0x58 codes 1, 5, 6, 7 close it, 4 leaves the waiting step. No background art, button cels, hover captions, note box or placed-item cel, and the dialog is opened from the menu, not by a server 0x58 code 0. The original state machine (`messages/socket.rs`) is not wired. Needed: a capture of Charsi's menu and of the original dialog's open path.
- **REC-146** Act V synthetic set and play walk (PROVISIONAL, `test-fixtures` `act5.rs`, M22; q-a5-fields): `Drlg::create(4, ..)` over an Act V-shaped set runs the real A5 / A5T placement (siege strip, B1 / B2 / BD linkers; the rects match the `outdoor-act3-act5.md` vector) and the Act V build for levels 110, 111, 112 and 117 (barricade border, ravine, entrances, caves, siege connection, type-12 border substitution, prisons, special presets). The ids, DrlgTypes, LevelTypes 29 / 30 / 31, the §1 sizes and offsets and the stamped lvlprest ids are the spec's; every piece's size (2 × 2 cells, siege strip 2 × 6), all preset content (floor only), the floor tiles, the monster columns, the level 136 preset and the type-12 `lvlsub` file (three 1 × 1 groups that stamp prison pieces 915..917, so step 8 finds its three cells) are made up. Barricade rows are walls in the original but walkable here (no collision tiles), and no waypoint object exists in 111 / 112 (preset content), so waypoint travel, the ice caves (113..116, 118, 119) and Nihlathak's temple / halls (mazes) are untested. Needed: a local run on the user's files (Harrogath → Bloody Foothills → Frigid Highlands → Arreat Plateau; Frozen Tundra via the cave; prisons, barricades, monsters, no `rejected` lines).
- **REC-148** Act V quest hooks in the play host (PROVISIONAL, `d2-server` `wired/quest_events.rs`, `d2-sim` `wiring/economy/quest_objects.rs`, `d2-client` `single_player.rs`, `rest.rs`; M22; q-a5-quests): the Act V AI calls (Shenk `0x00587900`, Nihlathak `0x0058BC40`, the Ancients' `0x0058CF90`, the Baal crab `0x0058E600`) and Anya's temple-portal request (`0x0058BC80`) are queued as `QuestEvent`s and run on the quest control once per tick after the steps, not inside the AI call; the Act V object init/operate functions (62–77, 79; `quests-act5.md` §1.4) now route to `act5::q*`. Shenk is recognised as the unit monster init linked to chain 31 (superunique 42; the live record's superunique index is not read in the preview). `AncientsNotActivatable` queues the disarm and answers false; `AncientsActivatable` / `AncientsPortal` (statue AI) are left unwired (spec §7.9: `0x0058CFE0` has no caller, `0x0058CFB0` needs the quest control). Warp gates `0x0058D090` / `0x0058E640` (`warp_check`) have no caller in the warp path. Operate return values are ignored. Not verified: the flow in a live Act V (no Harrogath/Arreat levels in the synthetic world). Needed: a local run through Harrogath with the quest log open.
- **REC-150** Amazon skills in the play preview (PROVISIONAL, `d2-client` `app/weapons.rs`, `skill_rest.rs`, `d2-sim` `bodies/passive.rs`, `UseRest::skill_weapon` / `skill_item_at`, `d2-server` `SimGame::set_world_sync`, `skills/world.rs` `add_skill_level`, M22; q-amazon; the id the task prompt reserved): (1) the equipped weapon and quiver reach the skill pipeline from a per-tick copy of the inventory model: weapon in use = the right-hand item with a hand class; hand class from the item's type (`bow` 1, `xbow` 7, `jave` / `ajav` 3, other `weap` 2, else 0; the `wclass` column is not in the inventory's table projection); the skills (not combat) see it, because the preview's equipped items do not feed the player's damage stats (`equip_rules` off). (2) The passive-state refresh `0x00646D60` (`client/msg-skills.md` §2 r4) is new in the sim: stat list of the passive state with `passivestat1…5` = `eval(passivecalc_i)` and markers 350 / 351; layered stats (`passiveitype` > 0, the masteries) are set on layer 0; the skill-point spend turns the state on and refreshes. (3) The bow animation: the player's attack plays the bare-hand `hth` COF (the client art has no equipped items), so the action frame is the bare-hand one. (4) Jab damage is not observable in the synthetic camp (the town rule cuts the hit), `line_clear` is still `false` on the seam (skills with `lineofsight` > 0 do not start), `use_state` does not check the weapon type a skill requires (`itypea1`). Needed: a local run with an Amazon on the user's files (below) and a trace of one Magic Arrow / Jab / Valkyrie cast.
- **REC-151** Necromancer casts in the play host (PROVISIONAL, `d2-client` `tests/app_necro.rs`; M22; q-necro): Teeth (`srvdo` 8), Poison Nova (22), Bone Armor (18) and Clay Golem (56) run end to end on the synthetic play host through the existing cast / missile / state / summon wiring, with test-local skill rows (no 1.14d `skills.txt` row is read, calc formulas are `push v; end` stand-ins). No production change was needed. Not covered: curses (`srvdo` 30, needs a hostile monster in range), Corpse Explosion (st 17 / do 55) and Raise Skeleton (st 15 / do 31) (need a corpse target), Revive, Bone Spear / Bone Wall / Bone Prison. Not verified on live data.
- **REC-152** Barbarian skills in the play preview (PROVISIONAL, `d2-client` `app/skill_rest.rs`, `app/single_player.rs`, `d2-sim` `wiring/interaction/skill_use.rs` `path_op`, tests `app_barbarian`, q-barb, M22): the bodies were already in `d2-sim` (srvst 32 / srvdo 2, srvdo 68, srvdo 70); three seams of the preview were missing. (1) `UseRest::line_clear` answered "blocked" (no collision grid reaches the seams), which refused every `lineofsight` skill at its start and emptied Double Swing's next-target scan (`accepts` filter bit 0x200); it now answers "clear" (a wall between the two is not seen). (2) `Pending::frame_event_index` / `set_frame_event_index` (unit +0x38 bits 8+) were defaults (0, no-op); they now read and write the use pipeline's `event_arg`, so Double Swing alternates (`bodies-2.md` §4.8); monster bodies that set the index see it too. (3) A retarget (`PathOp::TargetUnit`) reached only the path provider (q-paladin, REC-153), never the unit's kept mode target that `UseRest::target` reads; `UseView::path_op` now also tells the host (`Pending::body_path_op`, default no-op), and the preview stores it. Not done: Leap (srvst 40 / srvdo 77) and Whirlwind (srvst 38 / srvdo 76) still do nothing in play (probed after the merge of q-paladin's path provider: no mana, no move; the start returns 0 before the path ops); Howl / Taunt / Find Item / Grim Ward bodies and the client side of the warcry state; item-type masteries (`passiveitype`) layered stats; saved Barbarians' passives at load. Warcry numbers (state ids, lengths) are made up in the synthetic rows. Needed: a game-file run with a level-30 Barbarian (Leap, Whirlwind, Battle Orders, Double Swing, Sword Mastery) and a trace of the original's frame event index.
- **REC-248** Client collision for `line_clear` (PROVISIONAL, `d2-sim` `wiring/interaction/skill_rooms.rs` `rooms_line_clear`, `skill_use.rs`; q-client-collision, M22): the play preview's `UseRest::line_clear` answered "clear" (REC-152) because the seam holds no grid. The play host runs the d2-sim path provider (`enable_paths`) on the DRLG rooms' collision grids, so `UseView::line_clear` now runs `path::line::line_test` from the caster's room and path position to the point under the skill's mask (blocked when the unit has no room, rule 1); without the provider the seam answers as before. The same grids already answer `line_blocked`, `box_collides`, `pattern_collides` and the walk paths. Not done: the stop cell and unit sizes (`units_line_blocked` rules 4–5) are not applied here; the original's mask handling for `lineofsight` (use.md §5) is unchecked. Needed: a game-file trace of a LOS-gated skill behind a wall.
- **REC-149** NPC approach in the play preview (PROVISIONAL, `d2-sim` `wiring/interaction`, `d2-server` `world/npc_approach.rs`, `d2-client` `npc_seams.rs`, `single_player.rs` synthetic charstats, q-npc-approach, M22): C→S 0x13 at distance 7–8 now starts the run to the NPC (`PathCtx::approach_unit`, `npc.md` §2 rule 3.1) and queues the interaction; when the player's mode leaves walk / run / town walk at the start of a tick, the 0x13 handling runs again (talk on arrival, rule 3.4). d2rs-own: the arrival is read from the mode, not the `0x00580C20` step result; a new walk request drops the queue; every unit has size 2 in the snapshot distance, which now is the spec's unit distance `0x00641530` (was Euclid); the synthetic game has made-up charstats rows (walk 6, run 9). Needed: a local run on the user's files (click Akara / Kashya from afar, one click talks), and the arrival from the step result.
- **REC-163** Charms and item bonuses in the panel (PROVISIONAL, `d2-sim` `wiring/inventory/item_link.rs` `link_charms`, `pending.rs` `charm_unlink`, `wiring/action/vitals_sync.rs`; M22; q-charms): (1) an active charm on inventory page 0 links its stat list to the player at the inventory pass (§5.7 step 2) and unlinks when it leaves (the bodies of `0x0063D1D0` / `0x0063D2B0` are unwritten, OQ6); (2) SUPERSEDED by REC-188 (the client now builds the item lists; the stat messages carry the base only): the client builds no item stat lists, so the stat messages for attributes (0-3), defense (31) and resists (39-46) carry base + the linked item lists, and the panel shows the total with no bonus colour. Not done: max life/mana/stamina, damage, attack rating, saved-character charms, colours. Needed: a trace of one charm picked up (0x1D-0x1F after the move) and a recording of the character panel with +resist gear.
- **REC-167** Duriel's Lair and the staff hand-in in the play preview (PROVISIONAL, `d2-client` `app/synthetic_act2.rs`, `synthetic_maze.rs`, `single_player.rs`; `d2-sim` `wiring/economy/quest_host.rs`, `wiring/path/place.rs`; `d2-server` `world/wired/quest_events.rs`; M22; q-a2-duriel): (1) C→S 0x44 reaches `q6::item_to_object` through `Pending::staff_in_orifice` → `QuestEvent::InsertItem` (run after the tick, the handler's own result is 0); (2) the warp gate `Pending::warp_quest_gate` (`quests.md` §8.2) answers for level 73 from the quest control's `lair_warp_open`, published once per tick (`set_lair_open`), and only from the staff tomb; (3) the host's `set_room_portal`, `player_busy` and `spawn_quest_object` are REC-166's (q-a4-endgame, merged first), and a game with an empty `missiles` table answers Range 440 for row 338 (spec test vector, lair timer period 18); (4) Duriel (monstats 211) is linked to chain 13 by class on his kill, as Andariel. d2rs-own, unverified: the synthetic Lair (level 73, a maze level with one way back per tomb, one way in from every tomb), the orifice's place in the first room of the staff tomb (object row 152, operate 25, init 21), object row 100. Not done: the lair entrance object (class 100) is created by the timer but the way in is the warp tile; Duriel's AI and fight (the test spawns and kills him); Tyrael, his door and portal, Jerhyn's and Meshif's travel east (needs `q-act-travel` for those NPCs); the staff item itself (the synthetic game has no item tables, the hand-in is a `d2-server` test). Needed: a trace of the hand-in (0x58 results, the lair timer's tick) and of the lair warp refusal.
- **REC-161** Worn items feed the wearer; set bonuses (PROVISIONAL, `d2-sim` `wiring/inventory/item_link.rs`, `InvState::link_item_stats`, on in `preview_inv_parts`; `d2-client` `ui/item_tip.rs`; M22; q-item-uniques): the bodies of the stat link `0x0063D1D0` / unlink `0x0063D2B0` are unwritten (`inventory.md` OQ6), so the preview's equipped items fed nothing. Read as: the item's whole unit stat list is attached to the owner on a body equip (`stat-lists.md` §8.4, reset 1) and detached when it leaves the body; a set-quality item runs the set-item update (`properties.md` §13) and the set bonuses (§11, `props::set_bonuses`, owner list state from the update) after the call (the owner's inventory is lent out during it, so the update is queued like the other equip rules); when a piece leaves, the owner list is freed and the pieces still worn are re-evaluated. The tip of a set item adds its set name (gold) at the foot; the set bonus lines are not drawn. Unique and set fixed stats and rare affixes were already rolled by the sim (`items/quality.md`, `affixes.md`) and need no new link. Not verified on live data; the hand and swap-slot pieces count only for body slots 1–10. Needed: a trace of equipping two pieces of one set (the stat 71 owner lists and the stat totals).
- **REC-230** Level warps in every act (PROVISIONAL, `d2-client` `app/synthetic_chains.rs`, `d2-sim` `wiring/worldgen/dispatch.rs`; M22; q-levels-warps-all): the synthetic world had warp pairs only for part of Act I, Act II and Act IV. Chains of flat one-room levels joined by warp pairs now cover Cold Plains → Dark Wood → Tamoe Highland → Underground Passage 1, Act III (Kurast Docks … Durance of Hate 3) and Act V (Harrogath … Worldstone Keep levels); every chain order, level neighbour and tile place is made up (the original joins these through outdoor placement and cave mouths with several exits per level). Act III (act index 2, town 75) is now created in the synthetic and the live act list (before, an act change into it found no DRLG and left the player in place). A real gap fixed: with the inactive store off, a room that went inactive and was reactivated (waypoint or act-change arrival, a revisit) lost its warp tile units for good; the reactivation now allocates them again from the room's presets (idempotent). Not verified on live data. Needed: a trace of Act III / Act V level entries (the lvlwarp rows of those levels and where the original puts their tiles).
- **REC-231** Front-end host hookups (PROVISIONAL, `d2-client` `app/front_host.rs`, `ui/front_end/{mod,screen}.rs`, screens `credits`, `create`, `controls`; M22; q-fe-host-screens): the host now feeds screens pointer-move, wheel, key-up and the middle button (`FrontInput::{Wheel,KeyUp,Middle}`, `Screen::{pointer,wheel,key_up,middle_down,overlay,sync}`) and draws each screen's `overlay` after the control list. d2rs-own, unverified: (1) text is still a bar (glyph path = q-fe-draw); the host measures text with a fixed 8 px per unit (`provisional_adv`), so the credits columns A/C are placed with that width, and `DrawItem::Text` carries no colour (credits `k`, controls colour); (2) controls: the dark panel and its `boxpieces` border are two new `DrawItem::{Rect,Border}` items (flat box and 1 px outline), the slider is `MENU\textslid`; the middle button is the pseudo key 0x100 pressed and released at once; the Enter-release swallow of `ControlsScreen` is kept and the host's real key-up for that key is ignored once; (3) create: hero controls are art-less click boxes and the heroes, hover texts (197/198 at the descriptor positions, font 1), check marks (`clickbox` frame 1), name text with a blinking `_` caret and the 5303 / 5165 popup text (position and font made up) are overlay items; the 5303 YES / NO buttons (5166 / 5167, positions made up) exist while the warning is up; the duplicate-name popup is text only; hover uses the 88×184 descriptor box (REC-181); the stub `.d2s` is written at the game-load outcome with `D2s::new_stub` (time = wall clock, creation flags hardcore 0x04 / expansion 0x20, class byte = `Class::id`), no free-space check (5149); (4) char select: `register_with(reg, Some(save_dir), handle)`; the `SelectionHandle` is created by the host but not yet read (q-fe-start-flow); (5) credits text: loose file under `D2_GAME_DIR` first, then `data\local\ui\eng\{Expansion}Credits.txt` in the archives (path unverified). Not verified against the original. Needed: a render of the credits, controls and create screens.
- **REC-249** Act I's dungeons in the play preview (PROVISIONAL, `d2-client` `app/synthetic_chains.rs`, `app/single_player.rs`; M22; q-a1-dungeons): the synthetic Act I now holds every dungeon level as a flat one-room level joined by warp pairs in a tree (replaces REC-230's Cold Plains chain): Black Marsh → Dark Wood → Tamoe Highland → Monastery Gate → Outer Cloister → Barracks → Jail 1–3 → Inner Cloister → Cathedral → Catacombs 1–4; branches Dark Wood → Underground Passage 1–2 and Tristram, Tamoe → Pit 1–2, Black Marsh → Hole 1–2, Burial Grounds → Crypt and Mausoleum, Cold Plains → Cave Level 2. Every parent, slot, id and tile place is made up (the original joins these through outdoor placement, DS1 stairs and `lvlwarp` rows; Den/Cave 1 and the Tower cellars stay the maze levels of `q-act1-dungeons` / `q-a1-tower`). Catacombs 4 lost its old room at (0, 24), which overlapped the Burial Grounds. Not verified on live data. Needed: a trace of the Act I level entries (lvlwarp rows and tile places per dungeon).
- **REC-259** Act I's fixture tree follows the level links (PROVISIONAL, `d2-client` `app/synthetic_chains.rs`, `app/single_player.rs`, `app/synthetic_tower.rs`; M22; q-a1-vis-links; amends REC-249): 1.14d's Act I chain Stony Field → Underground Passage 1 → Dark Wood → Black Marsh → Tamoe Highland → Monastery Gate … Catacombs 4; Tristram off the Stony Field, Burial Grounds off Cold Plains, Cave 2 off Cave 1, Hole off the Black Marsh, Pit off Tamoe, Forgotten Tower off the Black Marsh. Blood Moor → Cold Plains → Stony Field are outdoor borders, not tiles (the tests put the player there). The invented Blood Moor tiles to the Black Marsh and the Burial Grounds are gone. The `Vis0–7`/`Warp0–7` values themselves are not in the specs (only the columns), so the parents come from the task row's chain, and slots, ids and tile places stay made up. Needed: the real `Vis`/`Warp` values from `levels.txt` (local `data-tool tables`) to compare slot by slot.
- **REC-261** Act I tree dungeons as maze builds (PROVISIONAL, `d2-client` `app/synthetic_a1_maze.rs`, `app/synthetic_maze.rs`; M22; q-dungeon-builds): Cave Level 2, Underground Passage 1–2, Hole 1–2 and Pit 1–2 are built by the real maze generator over synthetic `lvlmaze` rows (6 cells for the caves and passages, 4 for holes and pits, 24-tile cells, one floor DS1) instead of flat rooms; their tree warp tiles (REC-249 slots and ids) stand in the first maze room (the original takes exits from the cells' DS1 warp units). Row counts, room counts and tile places are made up. Not done: the Den of Evil stays a flat room (its preset build needs the lvlprest/DS1 rows of the live game), Crypt, Mausoleum, Tristram and the cathedral/catacomb levels stay flat. Needed: live-data run of the Act I walk and a trace of the Act I level entries (lvlmaze rows, warp units per cell).
- **REC-174** Act II's close in the play preview (PROVISIONAL, `d2-sim` `wiring/path/act_change.rs`, `wiring/action/town_portal.rs`, `wiring/economy/quest_host.rs`; `d2-client` test `app_a2_tyrael.rs`; M22; q-a2-tyrael): (1) a real gap fixed: an act change left the player's unit record act (+0x18) at the old act, so the monster AI's "same act" target test (`ai.md` §5.2 step 5.1) never found a player who had changed act, and no monster attacked after the first act change; `act_change::run` now sets it. (2) Duriel (AI 44) attacks the player with the existing monster AI (q-monster-ai); the test gives him plain A1/A2 attacks (monstats skills -1), not Charge / Jab. (3) Tyrael's message 302 (`quests-act2.md` §8.11) needs a portal: the host's `create_portal` was a stub returning false; `HostQuests::create_portal` now creates one portal object (class 59, mode 1) at the free spot nearest the player (town portal rules, size 3, mask 0x1C09) with the destination level as its interact byte and the player as owner (`View::create_quest_portal`). d2rs-own, unverified: the portal has no partner object at the destination, and where the player arrives (the quest portal check `portal_destination` is not driven by the test); Jerhyn's 442 and Meshif's 450 and his travel row go through the existing quest and `act_travel` code to Act III's town (level 75, REC-230). Not done: Tyrael's chamber door (object 153) and Tyrael as a placed NPC in the Lair (the test spawns him); the way through the portal is not walked. Needed: a trace of Tyrael's portal (object class, position, destination) and of the Lair's population.
- **REC-234** Tyrael's chamber and portal arrival (PROVISIONAL, `d2-client` `synthetic_act2.rs`, `synthetic_maze.rs`, `single_player.rs`; `d2-sim` `wiring/action/objects.rs`; test `app_a2_tyrael.rs`; M22; q-a2-tyrael-door): (1) Duriel (211), Tyrael (251) and Tyrael's door (object 153, init 38) are placed by the synthetic Lair's presets (host presets, d2rs-own positions), not by the test; Duriel's AI is still started by the test. The door is shut (mode 0) until Duriel dies, then animates (`q6` kill). (2) A real gap fixed: a portal without a partner had no destination (`object_level_spawn` default None, so walking in failed); `View::level_spawn_point` now uses the DRLG spawn location of type 12 in the destination level (first step of `q6::portal_destination`), then rule 10's free spot (not that function's size 3, mask 0xBE11, radius 7, which stays undriven). d2rs-own, unverified. Needed: a trace of Tyrael's portal arrival and the Lair's population.
- **REC-254** Duriel's AI from the world and the partnerless portal's free spot (PROVISIONAL, `d2-sim` `wiring/worldgen/{dispatch,init_units,mod}.rs`, `wiring/action/objects.rs`; `d2-client` `single_player.rs`; test `app_a2_tyrael.rs`; M22; q-a2-duriel-ai): (1) host-placed monsters whose class the host names (`WorldPending::host_monster_ai`, Duriel only) get the creation-time AI setup (control, `ai.md` §3.3 install state 0, first think next frame) when the preset spawns; the test no longer starts it. Other host monsters (Blood Raven, Izual, Tyrael) are unchanged. (2) `View::level_spawn_point` for a partner-less portal now runs the portal check's rule: type-12 spawn, then `free_point_step(size 3, mask 0xBE11, step 7)` (`path-placement.md` §7.1; `quests-act2.md` §8.3 calls the last argument a radius, the placement table calls it a step). The gate (+0x3C = 1) is not asked, so every partner-less portal takes it. d2rs-own, unverified. Needed: a trace of Duriel's first think and of Tyrael's portal arrival.
- **REC-240** Belt labels and tips, stash gold dialogs (PROVISIONAL, `d2-client` `ui/hud_belt.rs`, `ui/gold_dialog.rs`, `ui/stash_ui.rs`; M22; q-belt-stash): (1) belt key labels are the default bindings' names `1`–`4` (spec: the name of the key bound to the belt slot; the HUD holds no controls table, so a rebound key still shows its default); drawn in font 1, colour 4, at (left + 2, bottom − 2) (`control-panel.md` §5 r4). (2) The belt hover tip is the item tool tip (`item_tip`) at the text position, not the `0x0048C060` / `0x004E6410` strings (§5 r8, REC-ui-belt-hover). (3) The highlight rectangles (§5 r4, r5) are in the belt draw list (`HudBelt::draw_list`) but not painted: the play sink has no rectangle primitive. (4) The stash gold button opens the withdraw dialog (kind 4, limit stat 15) and the inventory gold button with the stash open the deposit dialog (kind 3, pre-filled with the carried gold); OK sends C→S 0x4F 0x13 / 0x14 with the amount split (`inventory.md` §11 r4); the dialog text is the preview English. Server amounts and limits are the existing `world/stash` rules. Needed: a trace of the belt with a rebound key and of the stash gold dialog's prompt strings (4049, 4050).
- **REC-239** Shrine states and the object mouse-over label (PROVISIONAL, `d2-sim` `wiring/action/objects.rs` `ShrineWorld::apply_state`, `d2-client` `world_view/object_label.rs`; M22; q-shrines-labels): (1) `ShrineWorld::apply_state` was the default (`None`), so no shrine ever gave its state. It now makes a plain state list on the player (expiry = frame + `Duration in frames`, the row's stat/value, extra list stats through `set_list_stat`), turns the state on and arms the type-12 timer, so the list is freed on the expiry frame (synthetic test `a_shrine_state_carries_its_stats_and_ends_on_its_tick`). Not wired: the helper's refusals (state-table check, curse and monster rules, keep-the-higher-level) and the remove callbacks `0x00583BD0` (skill shrine: skill refresh) and `0x00583A40` (stamina shrine), so those two states end as plain states. (2) The label: no spec states it. The preview draws the `objects.txt` `Name` of the hovered object (string-table entry with that key, else the name) centred 96 px above its feet, chat font, colour 0, from the existing hover pick. Not done: the shrine's overhead text (S→C 0x26 type 5, the decimal string id 3683 + shrine id) is held by the UI overhead store but nothing draws it. Needed: a capture of an object hover (text, font, colour, position) and of a shrine's overhead text, and a trace of each shrine state's duration and stat list.
- **REC-182** Difficulty box encoding and title colour (PROVISIONAL, `d2-client` `ui/front_end/screens/difficulty.rs`; M22; q-menu-difficulty): the box rules (L1/L2) are from the spec; the hand-over from character select is ours: `FlowCtx::difficulties_open` = 1 (start at once), 2 (box, Hell off), 3 (box, Hell on). The title text colour 7 is not drawn (`Control` has no colour field). Not verified on live data. Needed: the REC-203 capture of the box.
- **REC-180** Character select (PROVISIONAL, `d2-client` `ui/front_end/screens/char_select.rs`; M22; q-menu-charselect): (1) a screen's controls are built only on entry, so every visible change (selection, scroll, pop-up) re-enters the screen through `Trigger::GameExit` (resolves to character select) with a `refresh` flag that makes `build` keep the model; a dedicated refresh trigger needs a shared `flow.rs` edit; (2) scan order is sorted file-name order (the file system's order is unspecified); (3) legacy (< 0x5C) saves show level 1 (offset not in `formats/d2s-legacy.md`) and the default component set `0x0070CCC8` is all 0xFF; (4) the level line is "Level N ClassName" with English class names (REC-207), the expansion character title tier uses the character's own 0x20 bit, Matriarch/Patriarch by sex; name colour (red/gold) and the paper doll / selected-character animation are modelled (`slot_lines`, `Entry::paper_doll`) but not drawn (`Control` has no colour field, `DrawItem` no doll); (5) click fires on button-up (REC-168), double click uses the front-end tick clock; (6) mouse wheel: none (REC-204). Needed: a local run with real saves to compare slots and order.
- **REC-168** Front-end shell (PROVISIONAL, `d2-client` `ui/front_end/`; M22; q-menu-shell): (1) a click fires on button-up over the control that took the button-down (the spec gives the pressed art but not the firing edge); (2) label baseline constant k for button heights 33–34 (§F1.1 r5 lists 4/3/2/1 for h ≥ 35, = 32, 21–31, ≤ 20): 3 is used; none of the front end's buttons has those heights; (3) the cinematics progress byte N is kept in memory (`MemProgress`), not yet in a client settings file; (4) screens not built through `0x0043C4F0` (character select / create) leave the palette as it was; (5) the in-game exit goes to character select (frontend-menus §F1.3 last row, REC-200). The front-end host (a Bevy window before the game starts, mapping `DrawItem`s to art) is not written; `play` still opens the game directly. Needed: the screen sessions, then a local run.
- **REC-178** Front-end host (PROVISIONAL, `app/front_host.rs`, `main.rs`; M22; q-menu-host): (1) `play` without `--new` / `--save` / `--frames` opens the front end in its own window (800×600, sprite of a CPU-composed image: DC6 `data\global\ui\<file>.dc6` frame `frame`, bottom-left at the control position, sky `pal.dat`), then the game in a second window (a second Bevy app in the same process; if winit refuses a second event loop on the user's platform this needs a single-app handoff); after the game window closes, back to the main menu; (2) text items are not drawn (a bar marks each: no glyph path from `DrawItem::Text` yet); (3) the game starts with the default character at the menu's difficulty (the select / create screens report no choice yet); (4) cursor scaled to 800×600 from the logical window size, left button only; (5) saves-found scans the save dir for `*.d2s`. Needed: the screen sessions, then a local run.
- **REC-189** Front-end shared draw (PROVISIONAL, `d2-client` `ui/front_end/mod.rs` `draw()`, `ui/front_end/glyphs.rs`, `app/front_host.rs`; M22; q-fe-draw): (1) a pressed enabled button draws frames `tiles…2·tiles−1` (`frontend-menus.md` F1.1 r4), the label baseline +2; (2) the logo fire is an extra `DrawItem::Blend` (mode 3) after each logo half's base, same frame index (F1.5 r2–r3); fire cel names `FrontEnd\FireLeft` / `FireRight` are d2rs-own guesses (the spec lists `D2logoFireLeft`; the base names in `main_menu.rs` are already the short form); the create screen's `FrontEnd\fire` is drawn the same way; (3) the host adds mode 3 as per-channel `min(255, d+s)` on the palette colours (the spec's fit of the PL2 table, not the table itself) and adds the DC6 frame offsets to the position (`sprite-placement.md` §2, not checked against a render); (4) text items become glyph quads through `layout_text`/`OriginalText` (font DC6 + `.tbl`, colour 0, no PL2 map); a button label is centered in its width with baseline `y − (h − text_height)/2 + k` (+2 pressed); glyph cels are drawn from their left edge with no frame offsets. Settled by a local run of `play` against the original's main menu (screenshot compare), then a render check (REC-201).
- **REC-181** Character create screen (PROVISIONAL, `d2-client` `ui/front_end/screens/create.rs`; M22; q-menu-create): (1) screens get no pointer-move, so a click on an idle hero counts as hovered (it walks forward); hover texts and hover animation are in `CreateState` but not fed; (2) the hero hit box is the 88x184 descriptor, not the DC6 frame-0 box (§F3.3 r5; needs real art); (3) the name caret is taken to be at the end, so a `-`/`_` is never the first character (REC-209 settles the filter's second argument); (4) a walking hero blocks a click by its state before the click, and heroes visited earlier in the loop stay switched (§F3.3 r6.2 does not say whether a just-switched hero counts; edge case 2 needs it not to). The host must write the 335-byte stub save from `NewCharacterSink`; the free-space popup 5149 is not built. Needed: a local run with the real art, and a trace of a click on a hero with the pointer still (state at click).
- **REC-184** Configure Controls screen (PROVISIONAL, `d2-client` `ui/front_end/screens/controls.rs`; M22; q-menu-controls): (1) the front end has no wheel, mouse-middle/X, key-up or held-button events, so the adapter captures keys and left clicks only (the model has `wheel`, `key_up` and the mouse key values, tested); the Enter that starts editing has its release swallowed by the adapter; the scroll arrows act per click, not held at 50 ms; (2) button hit boxes use a fixed half width of 60 px (spec: text width / 2 + 10 needs the font measure); the slider track draws 20 frames (spec gives the 12 px step, not the count); (3) cells are hit by the §O9 r4 row rule; (4) the dynamic parts (rows, blink, colours) are exposed as `ConfigureControls::draw_list` because `Control`/`DrawItem` carry no colour and a screen cannot redraw between entries: the host draws that list; (5) persistence is d2rs-own (§O9 r8): Accept writes `controls.toml` over the `dev` preset, mapping the original commands that have an `Action` (0,1,2,4,5,6,7,8,9,12,13,14–26,35–38,42–44,46–53,56); other commands and keys with no portable name (F13+, Caps…) are not saved; right Shift/Ctrl/Alt are not producible; (6) flow: Cancel and Accept both go to the Options menu (the front end has no game to return to). Needed: a local look at the layout, the host, the Options entry arm.
- **REC-155** Druid skills in the play host (PROVISIONAL, `d2-client` `app/skill_rest.rs` `sync_shapes`, `world_view/disguise.rs`, `world_view/unit_assets.rs`; M22; q-druid): (1) `UseRest::shapeshifted` is true for a player that has the `aurastate` of a `srvdofunc` 116 row (Werewolf, Werebear), refreshed before each intent and tick; (2) the shape is drawn by the draw identity substitution of `render/unit-composite.md` §1.1 (`states` `gfxtype` 1: the monster `gfxclass`, player mode → monster mode with the `monstats2` fallback), applied whenever the model has the state (the client model keeps no flag-ex bit 3) and the states tested in ascending id. Not verified: Firestorm / Tornado / Molten Boulder / Raven / Oak Sage in a live game (the synthetic world has no collision, so a server missile dies after one tick there: the Molten Boulder trail is untested); the werewolf art and its animation rate; the revert-by-recast (`state_group` needs `BodyTables`). Needed: a werewolf render capture and a local run as a Druid.
- **REC-165** Render polish in the play preview (PROVISIONAL, d2-client `world_view/unit_shadow`, `preview_light.rs`, `preview.rs` `unit_facts`; M22; q-render-polish): (1) unit shadows (`blend-modes.md` §5 r1-r3): every COF layer with the shadow byte casts the sheared half-height shadow of its cel (a derived `#shadow` frame set per component set), drawn in the shadow pass slot with chain `[Z]` + `A0`. Unit height `oz` is 0 (no motion record), so the shadow sits at the feet; the no-shadow flag and state 146 are not in the model; no COF box pre-test; preview facts mark players, monsters and objects flag-ex 0x80 (sight test = visible). Items and missiles (single-cel shadow `0x00471450`, §5 r4) have none. (2) Hover outline: the preview's cursor pick (`bridge::hover::pick`) is the hover target, drawn with the spec'd hover mode (`blend-modes.md` §3 `h`) only when the preview light is on (with `D2RS_FULLBRIGHT=1` no light, no highlight). (3) Wall transparency and the edge clip were already wired (draw-order §8 fades instantly on the GDI clock; the preview edge clip is on); the town has no `Logicals`, so no wall fades there. Not verified against 1.14d. Needed: a render capture with a unit near a wall in a level with `Logicals` (wall fade) and one of a shadow (pixels of a standing player and monster).
- **REC-158** Worn items in combat (PROVISIONAL, d2-client `app/single_player.rs` `Pending::current_weapon`, `app/weapons.rs`; M22; q-weapon-combat): (1) the stat link itself is REC-161 (`wiring/inventory/item_link.rs`, switch `InvState::link_item_stats`); this entry's d2-sim equip tests run on it; (2) combat's weapon is the right-hand item with a hand class; (3) grip (`0x0063D340`) is 2 for a base item with `2handed`, else 1. Not verified: weapon-swap slots (11/12), `equip_rules` (still off), durability/requirements, dual wield, a live melee hit. Needed: a melee-damage trace with an equipped weapon.
- **REC-154** Sorceress skills in the play preview (PROVISIONAL, `d2-sim` `wiring/interaction/skill_rooms.rs`, `skill_use.rs`; `d2-client` `tests/app_sorc.rs`; M22; q-sorc): the bodies of Charged Bolt (do 17), Frozen Armor (18), Nova (22), Fire Wall (24), Enchant (25), Teleport (27) and Meteor / Blizzard (28) ran on the spec'd seams, but `UseView` answered the room seams from `Pending` defaults (box collision "collides", level `Teleport` none, free point none, placement refused), so Teleport, Fire Wall, Meteor and Blizzard did nothing. With the path provider on (the play host), `UseView` now answers `room_teleport` (the room's level row in `levels.txt`), `box_collides` (the DRLG grid; Size ≤ 1 a point query, `path-placement.md` §4 r3–4), `free_point` and `place_unit` (`0x00554EA0` through the path provider) from the DRLG rooms. d2rs-own, unverified: the path's target point (`bodies-2b.md` §6.2 step 3) is the point kept at the cast's mode start (`UseRest::target_position`) when the path record has none, because a cast at a point does not walk. Test rows are test-local (the synthetic game has no `skills`, `missiles`, `states` or `levels`). Not done: Teleport with `Teleport` = 2 (`line_clear` still blocked on the seam), the Enchant weapon check, the Ice Bolt chill and every damage, duration and count formula (test rows use constants). Needed: a trace of one Teleport and one Fire Wall cast (position after the cast; the three missile positions).
- **REC-153** Paladin skills in the preview (PROVISIONAL, `d2-sim` `wiring/interaction/body_path.rs`, `d2-server` `handlers/skills/world.rs`, q-paladin, M22): the skill bodies' path operations (`PathOp`: velocity, target point / unit, type, step counts, masks, compute) now run on the action wiring's path provider instead of the `Pending::body_path_op` no-op, and `UseView::has_path` reads the provider; a run-mode (3) skill used at a point (Charge) sets the path's target point at the mode start (`use.md` §4 does not say where a skill mode's point target reaches the path). Not done: the other `PathOp`s (Clear14, Face, Reset, TurnToward, SnapCenter, 649070, 648E40), melee skills on a monster (Sacrifice, Smite, Zeal), the spiral Blessed Hammer on the client. Needs: a trace of a Charge (0x0C at a point; the 0x15 / 0x0F walk updates) and of Might on / off. Check: `cargo test -p d2-client --test app_paladin`.
- **REC-156** Assassin skills in the play host (PROVISIONAL, `d2-client` `tests/app_assassin.rs`; M22; q-assassin): Tiger Strike (`srvst` 23 / `srvdo` 34, a progressive charge state on a hit), a finisher after a charge (`srvdo` 35, fault-free only), Sentry (45) and Shadow Warrior (49) pets (S→C 0x7A), and a generic state buff as Burst of Speed (18) run end to end on the synthetic play host; no production change was needed. The first three tests failed on test-row faults, not engine bugs: `pettype` 0 (no pet type, the pet list refuses it), `skills.stat_count` 0 (`stat_ok` refused `aurastat1`), and the missing `BodyTables` stat/state vectors and `0xFFFF` state/overlay sentinels. Rows are test-local (no 1.14d `skills.txt` row is read). Not covered: Dragon Talon / Dragon Flight (`path_op` is the default no-op), Fire Blast and Lightning Sentry missiles, the finisher's damage and charge consumption, the shadow's stats, the sentries' real AI (`NodeInsert`), the claw weapon requirement (`itypea1`, see REC-150 (4)). Not verified on live data.
- **REC-176** Paladin, Necromancer and Assassin skill gaps in the play host (PROVISIONAL, `d2-client` `tests/app_skill_gaps.rs` + `app/weapons.rs` + `app/single_player.rs`, `d2-sim` `items/inventory/tables.rs` + `wiring/interaction/skill_use.rs`, `d2-sim` M22; q-skill-gaps): 12 end-to-end tests with a real outcome each: Smite (needs a shield; none = no hit), Sacrifice (monster and caster both lose life), Zeal, a curse (state on the monster), Corpse Explosion (damage to a monster beside the corpse), Raise Skeleton (a pet from the corpse), Revive (the corpse stands up as a pet), Fire Blast as a generic `srvmissile` skill (a missile flies), Tiger Strike and a finisher (`srvdo` 35) on a claw (damage + charge state) and Tiger Strike refused without a claw. Links connected: (1) Smite's shield seam: `Pending::shield` / `shield_damage` / `has_shield` answered from the weapon copy (`ItemFacts::shield`, `dam`; shield = a left-hand item of type `shld`, damage = the items row `mindam` / `maxdam`, new `InvItemRec` fields); d2rs-own, unverified. (2) The monster mode request of the bodies (`mode_request`, Revive's `0x005A7E60`/`0x005A7C20`) was the `Pending` default 0; without a host answer it now runs the monster mode set, so a revived corpse leaves mode 12 and the pet sweep keeps it. (3) The weapon-type test of `use_state` (`0x00643F80`, `client/stat-lists.md` §2 r8; REC-150 (4)) was never run: a skill with an `itypea1`..`itypea3` row needs a hand item of one of those types that is not `etypea1`/`etypea2`; the spec names only `itypea1` / `etypea1` and the combination over the two hands is a guess (PROVISIONAL); `itypea` 0 / 0xFFFF asks nothing (blank test rows). Test-fixture faults found on the way (not engine bugs): corpse skills need `targetcorpse`, the monster class needs `isatt`, `switchai`, `corpsesel`, `revive` and the client's per-class mode mask (walk), `BodyTables::monlvl` needs a row (else monster life is 0), a test in a town room is cut. Not covered: Lightning Sentry's missiles (the sentry AI needs charges, skill slots and AI params the preview does not give a laid sentry), Dragon Talon / Flight (q-skill-moves), dual claws (no inventory in the preview), Revive's life (0 with the synthetic monlvl). Not verified on live data (rule 10).

- **REC-172** Settings and controls files, Esc-menu Options page (PROVISIONAL, `d2-client` `app/config.rs`, `ui/esc_menu.rs`; M22; q-config): `settings.toml` (`version = 1`, `[video] resolution` 0/1, `[video] window_mode` windowed / borderless / fullscreen) and `controls.toml` (§A6) are kept in the folder above the saves (`Documents/d2rs`, or the parent of `--save-dir` / the `--save` file's folder). A bad file stops `play` at start; a missing `controls.toml` is written from the `dev` preset, because the original's default key table has no `Preset::Original` data in the tree yet. The Esc menu's Options entry opens a d2rs-own page (Resolution, Window Mode, Controls, Previous): the first two write `settings.toml` at once and change the window; Controls only names the file (no Configure Controls screen; edit the file). The window-mode row is ours (the 1.14d menu has none); the resolution row sizes the window, the frame stays 800 x 600 (`frontend-options.md` §O8). The Options page art and layout are not the original's. Needed: the capture of the original Options tree (`frontend-options.md` §O2) and `Preset::Original` from `key-commands.tsv`.
- **REC-183** Loading screen state machine (PROVISIONAL, `d2-client` `ui/front_end/screens/loading.rs`, q-menu-loading, M22): keys ignored while loading (spec REC-222), no loading redraw after an act video (REC-223), 0x04 one tick after a warp (REC-221); not wired to the bridge or drawn yet. Needs: the host wiring and a game-file run of a game start and a waypoint act change.
- **REC-186** Cinematics menu (PROVISIONAL, `d2-client` `ui/front_end/screens/cinematics.rs`; M22; q-menu-cinematics): (1) `FrontCtx` carries no progress store or video hook, so the screen holds shared handles (`register_with`; the default `register` uses in-memory N and the logging stub); (2) the C5 r4 input flushes around a video are the host's: the screen only counts them (`Handles::flushes`); (3) the title text uses font id 7 for Font24 (id not in the spec); (4) the classic menu draws only its panel, not descriptor 8 / logo (the shell's); (5) the in-game writer is `note_video_request(store, id)`, with the video ids of frontend-loading L10; nothing calls it yet (the in-game video request path does not exist). Needed: the host wiring (settings-file store, real flush), a local look at the layout.
- **REC-173** Skill moves in play: Leap, Whirlwind, Dragon Flight (PROVISIONAL, `d2-sim` `wiring/interaction/skill_use.rs`, `skill_rooms.rs`, `skills/list.rs`; `d2-client` `tests/app_barbarian.rs`, `tests/app_assassin.rs`; M22; q-skill-moves): the bodies stopped on host seams that answered with defaults. Now on the path provider / skill list: `UseView::pattern_collides` (`0x0064D910` on the DRLG grid with the unit's path pattern; the default "collides" refused every Leap landing), the used entry's flags word (+0x0C) and params 1 to 4 (`ListEntry::flags` / `params`; the seam defaults dropped them, so Leap's and Whirlwind's phases never advanced), `used_skill_flags`, `path_point_count`, `path_last_point` and `step_path` (`0x00553490` / `0x00554CA0`: the path step, 2 when finished). d2rs-own, unverified: the pattern is the unit's own path pattern (read from the call shape). Test rows are test-local (a Leap range formula, charstats speeds, levels with `Teleport` = 1). Not done: Whirlwind's hits along the way (its do step runs only on arrival under `use.md` §5.2 as written, and the existing unit test fixes that; the real Whirlwind hits while it moves, so the step-2 clause of §5.2 needs a trace to settle), Dragon Talon's kick damage (the body runs, the synthetic game has no kick damage stats), Leap Attack, Leap's landing damage, the client's leap arc. Needs: a trace of one Leap and one Whirlwind (positions and 0x4C / 0x4D per tick; the hit ticks). Check: `cargo test -p d2-client --test app_barbarian --test app_assassin`.
- **REC-179** Main menu (PROVISIONAL, `d2-client` `ui/front_end/screens/main_menu.rs`; M22; q-menu-main): (1) cel file names (`FrontEnd\gameselectscreen[EXP]`, `WideButtonBlank`, `NarrowButtonBlank`, `ShortButtonBlank`, `BlackLeft/Right`) and the classic background name are guessed from the global names in §F1.4; (2) version text font id 1 for Font16; (3) Battle.net, Open Battle.net and Other Multiplayer are always built disabled (no Phase 7+ code); (4) disabled buttons draw the up frames (no third state in the cels); (5) the shared `draw()` never passes `pressed` and has no overlay pass, so pressed art and the additive fire overlay (FireLeft/FireRight, mode 3) are not drawn yet; `main_menu::button_frame` holds the per-state rule. Sounds named only. Needed: host draw edit, then a local run.
- **REC-185** Credits screen (PROVISIONAL, `d2-client` `ui/front_end/screens/credits.rs`; M22; q-menu-credits): the credits text decode (ANSI / UTF-16 LE / BE, CR LF → NUL), the A/B/C column parse (C4 r3), the scroll (2 px per tick from baseline 590, top row 20 + ⌊t/10⌋) and the row layout (C3 r2) follow the spec and are tested on synthetic text. Fills: (1) the default text source reads loose `D2_GAME_DIR/data/local/ui/eng/{Credits,ExpansionCredits}.txt`, not the MPQ, and a missing file gives an empty list (spec C4 r2: fatal); (2) the shared draw list has no per-tick text, so the host draws `CreditsScreen::visible_rows` itself (FontFormal10 = font 3, k colours) — not wired into the `play` window; (3) a stop line (non-letter first line) skips the closing balance (spec silent); (4) no automatic return, ever (C3 r6 holds with the shipped files; short lists would return in the original). Not verified on the shipped files (needs a local run: the row counts 1,231 / 1,797 of C4 r6).
- **REC-250** Blocks-light flags, near-room fills and ambient overrides in the play preview (PROVISIONAL, `d2-client` `world_view/preview_light.rs`; M22; q-lighting-blocks): closes REC-247 (3) and (4). The light map's blocks-light flags (`lighting.md` §4) read the client DRLG collision (`ClientDrlg::drlg.collision_at`, mask 0x22, the grid q-client-collision uses); the player's light (kind 0, §8) takes the shadowed contribution (§7.3); the player room's near list (its adjacency array) fills its rectangles with each room's own level ambient (§3 r3); the room ambient is the scripted override of `world.overrides` (§10) before the level / act ambient (§3.1). Open: (1) client quest byte 1 is not held, read as 0 (the Den of Evil glow never shows); (2) other sources (monsters kind 0, objects kind 2) draw plain, not shadowed / cached; (3) the overrides' counters advance wherever the bridge advances them, not per this preview's frame; (4) the near room's level comes from the active-room record. Needed: a capture of a light next to a wall (light values behind it) and of a room border with different level ambients.
- **REC-255** Cain's identify caption (PROVISIONAL, `d2-client` `ui/npc_menu_ui.rs`; M22; q-identify-cain): the menu counts the unidentified items from the client's item model (header flag 0x10 clear; backpack page 0, cube page 3, worn; stash and belt skipped, `npc.md` §6 step 3) and, per `menus.md` §2.3, drops the Identify row at n = 0 and draws `NPCIdentify2` (4021) + `100 × n` otherwise. The quest-4 bits (Search for Cain) that waive the charge are not in the client model, so the caption always shows the cost and never falls back to 4020 as is; the server (`npc.md` §6 step 4) charges nothing when those bits are set. Needed: a trace of Cain's menu with the quest done.
- **REC-247** Per-block tile light and Levels ambient in the play preview (PROVISIONAL, `d2-client` `world_view/preview_blocks.rs`, `preview_light.rs`; M22; q-lighting-detail): walls, lower walls, floors and roofs get the per-block gradients of `shading.md` §4 from the frame's light map (wall points `lighting.md` §11 r2, floor grid r3, roof r4) through the CPU reference rules; the player level's `Levels.txt` ambient (`Intensity`, `Red`, `Green`, `Blue`) wins over the act environment when it has a colour (§3.1 r2). Open: (1) a roof block takes the floor-grid path with the wall-alpha blend (§11 r4 says "the same grid", `shading.md` §4 names roofs under the wall corner path); (2) a wall's fade state is taken as 0; (3) the blocks-light flags (§4) are not built: the client holds no collision grid, so no light is shadowed; (4) no near-room ambient fills (§3 r3) and no scripted override (§10). Needed: a capture of one wall tile and one floor tile next to a light source (per-pixel light values) to settle (1) and (2); the client's collision source for (3).
- **REC-241** Save gaps (PROVISIONAL, `d2-client` `app/save_gaps.rs`, `d2-sim` `wiring/inventory/load.rs`; M22; q-save-gaps): (1) the weapon-swap pair of mouse skills (header +0x80 / +0x84) and the weapon-switch bit have no sim state (C→S 0x60 is a stub), so they pass through as loaded; (2) a skill granted by an item is not in the sim's skill list, so a mouse skill saved with an item index loads on the native entry of the same skill, and the level test of `0x005701B0` is not run; (3) the town byte takes the player's act from the host snapshot (`unit_act`) and zeroes the other two bytes as `d2s.md` §2.1 says; the progression bits of the status word have no live source and pass through; (4) the hireling's items are saved from the living hireling unit and loaded onto it after the queued restore runs at the join, but only for a header that has a hireling block (a hireling hired since the load has no block, so its list is not written); `hirelings.md` §10 rule 8 (`hireling_items_loaded`) stays unapplied; (5) the Iron Golem's item is saved from a living golem (class 0x123); a loaded golem item is kept as loaded until a golem is summoned, because the re-summon (`d2s.md` Open question 15) is not wired; (6) runeword refresh (`d2s.md` §8.2 rule 5): a top-level item with flag 0x4000000 that matches no runeword row is freed when stored or on the cursor, and an equipped one is taken off and placed like a stored item (where the original leaves it is untraced, `d2s-load.md` §6 / Open question 2); matching items keep the runeword stats of the record. Needed: a recorded save with a hired mercenary carrying items, a summoned Iron Golem with an item, mouse skills on an item-granted skill and a changed act, then compare field by field.
- **REC-265** Save: swap set, switch, golem, hireling block, progression (PROVISIONAL, `d2-sim` `skills/list.rs`, `wiring/interaction/skill_events.rs`, `d2-server` `world/gap_items.rs`, `d2-client` `app/save_gaps.rs`; M22; q-save-swap; continues REC-241): (1) the mouse pair of the set not in hand and the switch bit live in the player's `SkillList` (`swap_left`, `swap_right`, `weapon_switch`); C→S 0x60 trades the pairs after `swap_weapon_sets` succeeds (`WorldHost::weapon_switched`); the original's behavior of the skills on a switch is untraced (`intents-events.md` open question 16); save writes header +0x80 / +0x84 / +0x10, load selects them with the item index rules of `d2s.md` §2.4; (2) item-granted skill entries: a pair entry owned by an item selects the entry of that owner when the list has it, else the native entry (unchanged from REC-241 2); (3) Iron Golem re-summon at the join (`d2s-load.md` §3): with a saved golem item and a skill 90 entry the join runs the body of `bodies-2b.md` §7.12 steps 2–6 at the player's skill level (`golem_summon`), but the saved item is not made as a unit, so the golem comes without its item and the save keeps the loaded item bytes; the join cast's item target and the `0x005394A0` GUID check are not run; (4) the hireling block (`d2s.md` §2.5 rule 1) is written from the first hireling node (dead flag, seed, name − row `NameFirst`, `Id`, experience stat 13 of the unit); the 16 reserved bytes stay as loaded; (5) progression: the client save flags (`AppRest::save_flags`, seeded from the header at the join, raised by `0x00538680` through `QuestRest::set_client_save_flags`) are written into status bits 8–12 and never lowered; (6) hotkeys have no live source (the hotkey intents are not stored by the sim), so they pass through as loaded and `resolve_item_indices` stays unapplied. Needed: a recorded save with a switched weapon set, an Iron Golem with item and a progressed character, then compare field by field.
- **REC-282** Save smoke fills (PROVISIONAL, `d2-client` `app/save_gaps.rs` `seed_new_flags`, `app/save_full.rs` `join_corpses`, `d2-sim` `wiring/action/dying.rs` `load_corpse`; M22; q-smoke-save; `// d2rs-own, unverified`): (1) a new character played above Normal (`play --new ... --difficulty nightmare|hell`, a CLI stand-in the original has no counterpart for) gets the least progression that unlocks that difficulty (`d2s.md` §2.2 rule 5.4: Nightmare 5 / 4, Hell 10 / 8, expansion / classic) in its client save flags, so its save loads on the difficulty it was played on. (2) a save's corpse (`d2s.md` §8.3 rule 4) is made at the join as a player corpse unit linked to the player (`ActionSim::load_corpse`: owner, state 7, mode 17) where the player is then, with no room and not announced to the client; its items go in through the corpse placement (`InvDesk::load_corpse_entry`), so the corpse and its items survive the next save; where the original puts the loaded corpse (and whether the client sees it in town) is not specified. Needed for (1): nothing to record (no original counterpart); revisit when the create screen chooses the difficulty. Needed for (2): a recording of loading a character saved with a corpse (where the corpse unit appears, its S→C messages).
- **REC-233** Assassin gaps in the play host (PROVISIONAL, `d2-sim` `wiring/action/mod.rs` `Sentry` + `wiring/interaction/summon.rs` + `skills/use_/bodies/{effects,dos2}.rs`, `d2-server` `adapters/handlers/world/sentry_drive.rs`, `d2-client` `app/single_player.rs` + `tests/app_assassin_gaps.rs`; M22; q-assassin-gaps): (1) traps: srvdo 45 now reports the laid trap (`BodyEffect::SentryLaid`: owner, skill, level, shots = the laying skill's `calc4` at the level) to `ActionHooks::sentries`; a host think (`sentry_drive.rs`, d2rs-own, unverified) follows `ai-bodies-6.md` §14 without the AI store: owner gone/dead/in town or no shots left → mode 0; a hostile monster within 25 sub-tiles and a 15-frame gap → one shot (shots − 1) with the class's monstats `Skill1` (else the laying skill) in `Sk1mode` (else attack 1), through the existing monster skill start/do; traps no longer follow like pets (the hirelings' think skips them; its target filter is shared, `nearest_hostile`). Not done: the `roll(100) ≥ aip1` draw (aip1 100 for the lightning sentry), per-trap AI params, `Skill1`'s own `calc4`/entry level (the laying skill's are used), the real AI owner link (`OwnerData` still a no-op). (2) Listed pets are on the player side (`sync_seams`: hostile to monsters, the summon's alignment effect `0x005543B0` is not wired); it lets a pet's skill hit. (3) Dual claws: `Pending::has_inventory` / `item_usable` answered from the weapon copy (a player with hands, a copied item is usable) so srvdo 35's two-claw test (`bodies.md` §8.10) runs: with two claws the second frame event strikes again. Weapon type 45 (`weap`) is in the fact's types. The second claw uses the same weapon damage as the first (the preview's damage reads the right-hand weapon; the left claw's own damage is not read). (4) Finisher after a charge (Tiger Strike then Attack): already worked; the test now covers the charge being spent and the extra strike. Test-fixture notes: skill ids 5 and 6 are special in the start (use 3, 4 …); the rig's states had no group bitsets, so PGSV was never flagged (it now takes them from the records when `pgsv` is given); a monster used as a target must be tough (the rig's player one-shots 100 hp). Not covered: trap missiles on live data (the test shoots a melee `Attack` skill), Dragon Talon/Flight (q-skill-moves). Not verified on live data (rule 10).
- **REC-187** Esc options menu (PROVISIONAL, `d2-client` `ui/options_menu.rs`, `ui/esc_menu.rs`, `app/config.rs`; M22; q-menu-options): (1) the DC6 label and value images, `OptBar`/`OptBarC`/`OptSkull` and the spinning `pentspin` are not drawn: labels and values are Font16 English text (value text centred in a 130 px block ending at h + 230, not right-aligned), the slider is a thin bar with a gold knob at the spec x (h − 60 + t), the pentagrams are gold squares at the spec x/y, no spin; (2) menu keys reach the panel as `UiEvent::Char` private-use units (0xF025–0xF028 arrows, 0x0D Enter) from `edge::key_chars`, and the pointer through `CursorMoved`/`Press`/`Release`; hover is applied per event, not per draw; (3) an extra Window Mode row (Windowed / Borderless / Fullscreen) sits after Resolution in the Video menu (d2rs-own, kept from q-config), so Video has 9 rows and its tops are 58 + 45k, not the spec's 8-row table; (4) expansion install assumed (Resolution and Automap Mode rows present); (5) rows of §O8 are stored but have no effect, the four disabled rows (3D Sound, EAX, 3D Bias, Perspective) are skipped; Light Quality, Blended Shadows and the Automap rows are stored here but their consumers are not rewired by this row; (6) Configure Controls sets `controls_requested` (read by `OriginalUi::take_controls_request`) and `flow` has the `OPTIONS` + `ConfigureControls` → `CONTROLS` arm; the in-game host for the CONTROLS screen does not exist, so nothing opens yet; (7) Contrast uses integer math (p 99 ↔ v 100 as REC-213's double-precision reading); (8) Gamma/Contrast snap on `set_settings` and are marked changed. Needed: a local look, the CONTROLS host, REC-212/213 captures.
- **REC-188** Item bonuses on the original wire (PROVISIONAL; M22; q-item-bonus-wire): S→C 0x1D-0x1F carry the base only and the client sums the lists of its equipped items (`client/stat-lists.md` §2). `d2-client` `bridge/item_lists.rs` decodes each 0x9C / 0x9D stream (`d2_proto::item_bits::decode`, list value = (raw - Save Add) << ValShift) into `ItemData::props`; `ClientWorld::total` adds the lists attached to the unit. Server: the pseudo states 0xFE / 0xFD (`vitals_sync.rs` `item_state_changes`) are removed. PROVISIONAL (stat-lists.md OQ 2, set items): every list the stream carries counts (base-magic, active set lists, runeword); attach = body location not 11 / 12, item flag 0x4000 clear, plus charms on inventory page 0. Not done: socketed gem / rune lists (§2 r2.2), the level-change re-attach (no net effect on a total), set-bonus lists per set mask. Needed: a recording of 0x9C equip next to the character panel with +stat gear.
- **REC-246** Act III / V gaps in the preview (PROVISIONAL, q-act3-act5-gaps, M22, `// d2rs-own, unverified`): (a) the item-move rest answered every quest flag as clear, so `ass` / `xyz` / `tr2` never worked (`inventory-moves.md` §7.11 step 4); the play host now stages the players' flag records for the call and writes the changes back (`WiredWorld::moves`), but `0x005458E0` (the `5D chain 02 00 0000` the use sends) is not sent: the item handler's seam passes no chain, and no spec says which chain the call takes from the item. Needed: the chain per code (`ass` 8? `xyz` 15?, `tr2` 37?). (b) The Act III and Act V waypoint indexes of the synthetic chains follow `waypoints.tsv`; the Kurast Docks waypoint object stands at a made-up place. (c) The Arreat Summit exits close only after the altar was operated (the preview has no fight to open them; spec: closed while the Ancients live in a not-intro record). The Arreat Summit's quest objects (statues 474–476, altar 546) are host presets at made-up places; the summit door 564, the invisible Ancient 561 and the fight itself (Ancients 540–542) are not placed. The summit's exits (`0x0058D090`) are the chain's two neighbours 118 / 128. Capture: the summit's preset object list and warp tiles in 1.14d.
- **REC-232** Skill gaps: Leap landing, Leap Attack, Dragon Talon / Flight kicks, Whirlwind hits (PROVISIONAL, tests only: `d2-client` `tests/app_barbarian.rs`, `tests/app_barbarian/rig.rs`; M22; q-skill-leap-talon; REC-232 was this task's provisional id): no sim change was needed. (1) Dragon Talon's and Dragon Flight's kicks hurt a monster in the synthetic game once the rows carry the `Kick` flag (physical part `(dex + str − 20) / 4..3`, `levels.md` §3.3) and the player has strength and dexterity; the kicks only land in a field room (`damage.md` §3.1: `apply` ignores a town defender), so the rig tests leave town first. (2) Leap's landing (`bodies-2.md` §4.7) is a knockback with a zeroed damage record, so it deals no damage by spec; the test pins the landing on the aimed point and the monster's life unchanged. The existing `leap_spends_mana_and_moves_the_barbarian` passed after one sub-tile only: with the 8-frame synthetic animation the flight never finished (the do restarts the animation from frame 10, beyond its end). d2rs-own, unverified: a 24-frame test animation `BASCHTH` with a frame event on every frame from 3 (the leaps' `anim` = 10). (3) Leap Attack (`bodies-2b.md` §6.11, §6.12, a monster skill in the real game) works for a player on that animation: the leap carries it past the monster and the strike (§2.19) hurts it; the row id 100 is made up. (4) Whirlwind's hits while moving stay as `use.md` §5.2 reads (the do runs on arrival only; `attack_frame_events` pins it), although the srvdo 76 body (`bodies-2b.md` §8.11 step 4) hits while `flags & 1`. Needs a trace: one Whirlwind with a monster in the path, per tick the player's position, the frame-event ticks (type-0 timers) and the monster's life, with and without a second melee weapon; it settles whether the do runs on every frame event while moving. Check: `cargo nextest run -p d2-client --test app_barbarian`.
- **REC-286** Visibility predicate inputs in play (PROVISIONAL, `d2-client` `world_view/visibility.rs`, `app/visibility.rs`; M22; q-fix-visibility; `client/model.md` §13 r6): the play app gives the bridge `world_view::visibility::ViewVisibility` (rules 1–5 over the unit art and the last drawn frame's camera, `WorldFrame::camera`). Not settled by any spec: (1) before the first drawn frame (no camera yet) the unit origin and `shiftX` read as 0 (the globals `0x007A520C` / `0x007A5208` / `0x007A5214` taken as zero-initialised); (2) the cel direction is the view's preview facing (`UnitArt::dir64`, REC-51), as the model holds no client path record; the cel frame is `+0x44 >> 8` as §13 r3 says, which can differ from the preview's drawn frame when the model's frame is 0; (3) a unit with no resident COF (synthetic data, a COF that failed to load), no TR layer or no loaded TR file reads as not visible, so §6 rule 7 corrects (C→S 0x5F for the local player) instead of the message being refused; (4) the cel's other context fields and the `0x006001F0` load arguments stay `model.md` OQ7. Needed: a replay of recording A's 0x68 / 0x6B / 0x6C checks with a live camera (OQ7), and the initial values of the three camera globals. Check: `cargo nextest run -p d2-client --test app_play_visibility` and `-E 'test(visibility)'`.
- **REC-279** Combat smoke fills (PROVISIONAL; M22; q-smoke-combat; `// d2rs-own, unverified`): (1) **respawn placement** (`d2-client` `bridge/modes.rs` code 7): `0x00480EF0(U, r0, r1, was-dead)` places a unit that was dead at (r0, r1), as 0x15's place (`msg-units.md` §3 r4, no free-point fallback); the respawn's 0x15 finds the player still dead (§3 r4.3), so the 0x0D code 7 that follows is the only message with the town point. Needed: the body of `0x00480EF0` (`model.md` OQ1) or a 1.14d trace of a respawn (the client position after the 0x0D). (2) **Good-unit target search** (`d2-client` `app/single_player.rs` `LocalSeams::nearest_foe`, `good_target_search`, `secondary_target`): the nearest monster of the other side that is not dying or dead, by the no-size distance (`ai.md` §6), closer than 35; no line test, no alternative targets (the scan 5 / 6 callbacks `0x005DCA70`, `0x005DCBD0` and `0x005DD510` are not written). Needed: those callbacks' bodies, or a trace of a hireling picking among two monsters. (3) **Corpse 0x74 on re-add** (`d2-sim` `wiring/action/switch.rs` `corpse_assign`): `intents-events.md` §7.2 part B's `0x0053DA40` for a dead unit with state 7 is sent as flag 1 with the corpse's GUID in both fields, so the client's 0x74 (`msg-units.md` §7 r7) puts the corpse in mode 0 instead of leaving 0x59's mode 5; the owner as P would kill a living local player on its client. Needed: a 1.14d capture of a player's own corpse coming back into view (the room join's 0x59 … 0x74 bytes). Fixtures of `tests/smoke_combat.rs` are made up (skills, `monstats` rows 0 and 271, AnimData, the `hp1` row, the mercenary's to-hit / damage). Check: `cargo nextest run -p d2-client --test smoke_combat`.
- **REC-275** Leap arc and Whirlwind spin in the play preview (PROVISIONAL; M22; q-move-anims; REC-275 was this task's provisional id): `world_view/skill_motion.rs` gives the local player the spec's motion record (`unit-composite.md` §8: creator `0x004C8726`, timed arc `0x004DA5B0`, update `0x004DA350`) for Leap / Leap Attack (`srvdofunc` 77 / 78), fed to the draw through `ViewSource::unit_offset`; Whirlwind (76) shows the skill's `anim` mode with frames looping from frame 3 (`bodies-2b.md` §8.11 step 4.1). Not in any spec, so d2rs-own, unverified: (1) the start signal. The server sends the client nothing for a skill move (position and mode stay as before), so the start is the local player's skill mode request (code 0x15, record 0 = skill, 2/3 = point; code 0x16 reading record 2/3 as unit type / GUID is a guess). A second request identical to the last one does not restart the arc. (2) The callers `0x004C8D58`, `0x004C902F` of `0x004C8670` are not traced. (3) Path speed `s`: the class run velocity; `d` = `0x006417F0` of the unit cell to the point. (4) `n := (d<<16)/(s<<12), −1 when > 1` is read as one less when above 1. (5) Whirlwind spin lasts the same `n` ticks (no arrival signal). Needs a capture of a leap and a whirl: per client tick the player's ox / oy / oz and mode / frame. Check: `cargo nextest run -p d2-client --test app_move_anims`.
- **REC-235** Quest items in the synthetic game, the Act IV Hellforge flow (PROVISIONAL, `d2-sim` `wiring/economy/{quest_host,quest_reward,quest_objects,chest_drop}.rs`, `wiring/inventory/potion.rs`, `d2-server` `handlers/world{,/wired}.rs`, `d2-client` `app/synthetic_items.rs`, `single_player.rs`; q-a4-quest-items, M22): the synthetic game now has item tables (`hfh `, `mss `, 21 gems / skulls, `r01`..`r25`), `InvTables`, drop tables and an `itemstatcost` stat table (all `d2rs-own, unverified`), so Cain's scroll 679 hands over the soulstone, the Hellforge takes it, Hephasto drops the hammer, three wielded hits smash the forge and the gems drop (`crates/d2-client/tests/app_a4_endgame.rs`). Fills with no spec owner: (1) `has_item` / `delete_item` / `wielded_weapon_code` of a quest call read and remove the items through the lent inventory model (`QuestInventory::{items_of, cursor_of, weapon_in_use, delete}`; the quest loan of the object operate now carries the model, `QuestLoan::inv`); the delete is the potion removal (message 0x20, unlink, free) for every item mode (`quests.md` §9.2's equipped branch is not separate). (2) the weapon in use (`quests-act3-2.md` §11.5) falls back to the first `weap` item of body locations 5, 4 while inventory +0x1C is unset: nothing writes it (`weapon_in_use_update` is an open seam). (3) quest drops (`drop_item_at`) take the start spot as is (`StartSpot`, as REC-108's monster drops) instead of `NoSpot`, are identified, and the placed item gets its spot in the static path (`HostQuests::drop_item_at`; the treasure walk's `placed` does it only with the walk-back field), which the preview's staged item place reads when the item data has none. (4) the play host adds the player's inventory before the start items need charstats. (5) the Hellforge's synthetic object row has `Mode2` 1 and frame counts 22 so its end-animation event sets mode 2. Not done: ground items are not yet announced to the client (q-monster-drops owns the update pass; the e2e test reads the server's store); chest drops still use `NoSpot`; the walk lend (`lend_quests` for `walk`) does not drain the inventory model's queued sends.
- **REC-256** Configure Controls over the game (PROVISIONAL, `d2-client` `ui/controls_host.rs`, `ui/esc_menu.rs`, `world_view/present.rs`; M22; q-controls-ingame): (1) the front end's `ConfigureControls` model (§O9) is hosted inside the Esc menu panel while ui 9 is open; the list, headers, key names and Cancel / Default / Accept draw in Font13 (13) at the §O9 r2 positions; (2) the border and scroll art are plain rectangles (white 1 px outline, 12 px squares for the arrows and thumb, no track), text for strings the table lookup lacks is English (`Function`, `Key / Button One`, command names from the d2rs action names, `Cancel`, `Default`, `Accept`); (3) clicks act on press (cell, buttons, arrows), the wheel scrolls; keys reach the screen as raw virtual keys from `present.rs` `ui_input` and the game's bindings get none while it is open (Esc cancels); the message clock is 40 ms per frame tick; (4) Cancel and Accept return to the Options menu with Previous Menu selected; Accept writes `controls.toml` (`save_table`) and replaces `WorldViewUi::bindings` with `table_to_bindings`; (5) the table opens from `controls.toml` (or the defaults) with the expansion flag false, since the play host does not pass the game type to this call; (6) mouse-button assignment (Mouse 3–5, wheel) is not captured. (7) the spec (§O9 r1) closes ui 9 on open and opens ui 11; here ui 9 stays open and the screen is drawn by its panel (the queue row asked for it), so the remembered-state rule is not modelled. Needed: a local look, the expansion flag, the DC6 art.
- **REC-264** UI rectangle primitive and belt key labels (PROVISIONAL, `d2-client` `ui/hud_belt.rs`, `ui/hud.rs`, `world_view/present.rs`; M22; q-ui-rects): (1) the rectangle `0x0046EFD0` draws as opaque tiles of the synthetic `d2rs\hudfill` file (frames 5–8 = red, green, blue, yellow, the nearest palette colour of `BeltColor::rgb`), reusing `esc_menu::push_fill`; the original's mode 0 (`k` = 2, table blend `T[256·d + color]`, `render/blend-modes.md` §8 r2) is not applied, so a belt box is opaque behind its item; (2) only the belt's `BeltDraw::Box` is painted (the Controls screen's rectangles already use `push_fill`); (3) belt key labels are the first key of the bindings' `BeltSlot1`–`4` actions (`Key::name`), an unbound slot has no label, the default `1`–`4` until bindings are set (`present.rs` calls `set_belt_keys` each frame); the controls table's `vk_name` and the cut to width 28 (§5 r4) are not used; the string ids 4049 / 4050 (`panels-2.md`: bank gold deposit / withdraw) are not belt key names and are not bound. Needed: a local look, the blend, the table's key names.
- **REC-253** Equipment rules on in play (PROVISIONAL, `d2-server` `adapters/handlers/items/moves.rs` `InvParts::new`, `d2-sim` `wiring/inventory/{inv_world,equip_rules,mod}.rs`, `items/inventory/bookkeeping.rs`; M22; q-equip-rules): `InvState::equip_rules` and `link_item_stats` are on for every `InvParts`. The §5.7 inventory pass step 4 ("switch on until a sweep changes nothing") never ended because the "linked" test (`item_active_on`, a rest seam answering false) never saw the stat link; with `link_item_stats` on it now asks the stat list (attached to the wearer), and the unlink goes through `unlink_item_stats`. d2rs-own, unverified: step 4 is capped at one sweep per body location (a world whose link never sticks cannot hang the server). A call that runs the rules directly (a body remove) now flushes the queued owner refresh at its end (`InvDesk::flush_equip`), so it no longer leaks into the next message's update pass. Still defaults in the preview rest: the skill list, the mouse-slot skills and `saved_mouse_skill` (weapon bookkeeping selects nothing). Needed: the skill list / mouse slots seams of play. Capture: equip a weapon with a throw skill selected and compare the mouse skills.
- **REC-258** Front-end text widths (PROVISIONAL; M22; q-fe-text-widths): resolves item (1) of REC-231's width part. The host now measures overlay text with the font table's advances (`ui::front_end::glyphs::text_width`, `ui/text.md` §6 max width) through `FrontArt::text_width`; `provisional_adv` is deleted. d2rs-own, unverified: with no art loaded (no game files) every width is 0; a font that fails to load measures 0. Needed: a render of the credits to compare column placement with the original.
- **REC-257** Esc menu art (PROVISIONAL, `d2-client` `ui/esc_art.rs`; M22; q-options-art; supersedes REC-187 point 1): labels, value images, `OptBar`/`OptBarC`, `OptSkull` and `pentspin` are drawn as DC6 through the panel-art path (`data\local\ui\eng\` via the `*local\` file prefix, widgets under `data\global\ui\`), at the §O4 positions with the spec's widths (frames tile at 256 px); (1) the disabled look (draw mode 1) and the dark slider rectangles of §O4 r2 are not drawn (no mode or darken field in `ImageRequest`); (2) the `pentspin` frame is `(tick / 2) % 8` (one step per 50 ms at 25 Hz), not the original's wall-clock counter; (3) Window Mode has no art and stays Font16 text; (4) `textslid` belongs to the key-config screen, not this menu; (5) missing archive files draw empty (`optional_file`). Needed: a local look and a pixel check (queue in §5).
- **REC-274** Duriel's Charge and Jab in the synthetic Lair (PROVISIONAL, `d2-client` `app/skill_rest.rs`, `app/single_player.rs`, `tests/app_a2_charge_jab.rs`; M22; q-a2-charge-jab): (1) a monster has no skill list, so the `Pending` entry calls (`entry_flags`, `entry_param`, set forms) were no-ops for it and Charge (`skills/bodies-2.md` §5.3) never kept its moving flag, target or hit mode; they now live per unit in the skill store (flags share the used-skill flags), so the monster frame event steps the path (`use.md` §5.2). (2) The synthetic rows are test fixtures: Charge `srvst 31 / srvdo 67`, Jab `srvst 5 / srvdo 7` as Duriel's `Skill1` / `Skill2`, `Sk1mode` S1, `Sk2mode` S2, `aip3 = aip5 = 100`, `Run` 9, the Charge animation with frame code 1 on every frame. Real rows and Duriel's real `aip5` (0 in live data, so he never charges) come from the user's tables. d2rs-own, unverified (rule 10).
- **REC-266** Preview skill seams: throw skill and item-granted skills (PROVISIONAL, `d2-server` `adapters/handlers/items/moves/preview_skills.rs`, `adapters/handlers/world.rs` / `world/wired.rs`, `d2-sim` `skills/list.rs` `SkillList::remove`, `wiring/inventory/{mod,equip_rules}.rs`; M22; q-preview-skill-seams; `// d2rs-own, unverified`): (a) the preview rest answered the §5.8 skill seams (mouse skills, `select_skill`, `has_skill_owned`, `throw_skill_row`, saved mouse skills, skill quantity) with defaults. They now read the player's list in `ActionHooks::skill_lists` (lent to the rest for one move call and returned; never a second copy); a selection sends S->C 0x23. (b) Weapon in use (inventory +0x1C) had no setter in the play host, so the bookkeeping saw no weapon: ~~`InvState::weapon_hand_fallback` (on in `preview_inv_parts`) reads the right-hand item when +0x1C holds none.~~ Retired 2026-10-09 (q-fix-weapon-in-use): the body link / unlink of `world/quests-act3-2.md` §11.5 r1–r2 now write +0x1C (`d2-sim` `items/inventory/weapon.rs`). (c) Stat 97 / 107 callback (`skills/levels.md` §7.1) is run once after each item-move call, not at each stat change: a skill with a positive item total gets a native base-0 entry and the client its 0x21; a base-0 entry whose total fell to 0 is removed (hands back to Attack, no message). Not done: `use_state` stays "usable" (no no-quantity / no-level restore), the 0x22 quantity send, the stat callback on load / join of already-worn items (first move call syncs them), stat 98/151/204 handlers. Needed: a recording of equipping a throwing potion and an `item_singleskill` item.
- **REC-273** Model unit facts for the draw order (PROVISIONAL, `d2-client` `world_view/unit_facts.rs`, `model_feed.rs`, `play.rs`; M22; q-view-unit-facts): `ViewFeed::unit_facts` is answered from the model once `set_unit_fact_tables` has given the table columns (`unflatDead` through `MonStatsEx`, `DrawUnder`, leveldefs `LOSDraw`, loaded from the user's tables): flags +0xC4 (the bits a model rule writes: 0x2, 0x4, 0x200, 0x800000), flag-ex +0xC8, states 7 / 143 / 146, `unflatDead`, `DrawUnder`. In the preview these fill over the d2rs-own facts (flag-ex 0x80 and the sight answer stay the preview's). d2rs-own, unverified: flag bits no model rule writes (flat 0x100000, missile 0x10000) read 0; outside the preview, in a `LOSDraw` level, `sight_hidden` stays `None` (the §15 line test needs the client DRLG's collision grid) so the frame refuses; a monster class with no `monstats2` row or an object class past the rows refuses. Needed: the collision grid as `CollisionRooms` over the client DRLG, and a render of a corpse and a DrawUnder object to compare the order.
- **REC-269** Character panel extras (PROVISIONAL, `d2-client` `ui/char_feed.rs`, `app/hud.rs`; M22; q-char-panel-full, `// d2rs-own, unverified`): (1) the class line is the string-table text of the `charstats` `class` key (the loader's wide name, `fixups.md` §4), Font16 colour 0 in [sx + 193, sx + 310]; (2) the resist / defense colours read the unit's state ids against the `states` flags (rfblue/red, rcblue/red, rlblue/red, rpblue/red, armblue/red); a red state wins; the defense `holyshield` test uses state 101 alone, without the shield (item type 51) test; (3) next level is the `experience` row of the base level (the HUD's table, no second copy) through `next_level_value`; (4) the damage / attack-rating block draws for the left and right skill from the skilldesc `str name`, `descdam`, `descatt` of the skill: only `descdam` 1, 7, 18, 19, 20 print a value (min / max damage stats 21 / 22 plus stats 18 / 17 percent, no skill modifiers, `SrcDam`, elements or dual wield) and only `descatt` 1, 2 (stat 19 plus stat 119 percent, colour 0); other entries draw the name only; the skill state test `0x004D9FC0` is not applied; the to-hit popups are not drawn. Needed: a trace of the panel with a weapon and each skill; `skills/descriptions.md` §3 entries in `ui::skill_desc`.
- **REC-272** Preview weather: water floor, passes 4 and 9 (PROVISIONAL, `d2-client` `world_view/weather_view.rs`, `rules/draw_order/{sky,source}.rs`, `world_view/{model_feed,feed}.rs`; M22; q-weather-passes; `// d2rs-own, unverified`): (1) the weather state (`Weather`, `FloorContext`) lives in the play preview's feed; the local player's weather seed is `Seed::init_low(guid)` (the client seed is read-only in the model, `camera.md` OQ6), the update count is the server tick, the camera delta is the change of the camera unit origin between frames, the day period is 1 (`0x0061C100` has no model source), the video mode is 1 and the frame rate (`[0x007BB390]`) is 25, so the flash always draws; a level change calls `0x004726F0(0)`, act load runs on the first level and each act change; (2) pass 4 cels are DC6 `UncompOverlays\Rain1-4` / `bubble1-4` frame sets, draw mode 3 (additive), light -1, keyed pass 4 / pool / slot; pass 9 lines are drawn one 1x1 item per pixel (`blend-modes.md` §8 r1, the pixel list of `gdi_line_pixels`), the flash is a rectangle frame of the clipped size (§8 r2, mode 5); (3) the floors keep their water bit in the preview (`dry_floors` is off when weather is on) so the floor pass spawns splashes / bubbles; (4) `LevelRow` carries `Rain` / `Mud`; (5) lightning is never started in play: its only caller is `draw-order-2.md` OQ4, so pass 9 draws a flash only in tests (`start_lightning`); thunder sound is not played (sound deferred) and its two position rolls are not drawn. Needed: a weather capture (`capture.md` §3.4) of a rain level to settle the move counter (OQ3), the day period source, the draw-order OQ4 caller and the frame-rate value.
- **REC-268** Pending-list audit, Shift stat spend and cursor warp (PROVISIONAL, `d2-client` `ui/original.rs`, `ui/edge.rs`, `ui/frame.rs`, `world_view/present.rs`; M22; q-pending-audit, `// d2rs-own, unverified`): (1) Shift on a stat add button spends all points in chunks of 32 (`panels.md` §8 r5, `panels-2.md` §17 r2: the spec's count is the full stat, not 5); the flag is the host's per-frame `OriginalUi::set_shift`, not a UI event. The skill tree has no spec'd Shift spend: one point per click. (2) The cursor jump (§4.3, `UiEffect::CursorX`) warps the window cursor to the top-left physical pixel of the target frame pixel at the same y (`Presentation::from_frame`); the original's `0x00468770(x', y)` sets the cursor in frame space, the window mapping is ours. Needed: a live run opening the inventory or waypoint menu with the mouse far from the panel, to see the cursor land where the original's does.
- **REC-273** Model unit facts for the draw order (PROVISIONAL, `d2-client` `world_view/unit_facts.rs`, `model_feed.rs`, `play.rs`; M22; q-view-unit-facts): `ViewFeed::unit_facts` is answered from the model once `set_unit_fact_tables` has given the table columns (`unflatDead` through `MonStatsEx`, `DrawUnder`, leveldefs `LOSDraw`, loaded from the user's tables): flags +0xC4 (the bits a model rule writes: 0x2, 0x4, 0x200, 0x800000), flag-ex +0xC8, states 7 / 143 / 146, `unflatDead`, `DrawUnder`. In the preview these fill over the d2rs-own facts (flag-ex 0x80 and the sight answer stay the preview's). d2rs-own, unverified: flag bits no model rule writes (flat 0x100000, missile 0x10000) read 0; the sight test (§15) runs over the client DRLG's collision grids (`ClientRooms`, `LOSDraw` from leveldefs; unit sizes from `path-placement.md` §3 tables) from the model's cells (no walk prediction, an unplaced unit at (0, 0)); where it cannot be placed (no DRLG, no local room, no size row) `sight_hidden` stays `None` and a `LOSDraw` level refuses the frame; a monster class with no `monstats2` row or an object class past the rows refuses. Needed: a render of a corpse and a DrawUnder object to compare the order.
- **REC-277** The shop opens on the NPC menu's Trade / Gamble choice (PROVISIONAL, `d2-client` `ui/npc_menu_ui.rs` `NpcMenuState::shop_request`, `ui/shop_ui.rs` `shop_poll`; M22; q-play-smoke F1): the choice sends C→S 0x38 (`ui/panels-2.md` §14 r1) and the next shop poll opens ui 0x0C for that NPC, so closing it sends C→S 0x30 even when no store item (S→C 0x9C action 11) arrives. Before, the shop opened only on arriving store items, so an empty store left the player in the NPC interaction (the next 0x13 did nothing). d2rs-own, unverified: whether the 1.14d Trade handler `0x004B42B0` sets the shop UI state itself or waits for a server message is not written. Needed: the handler's `SetUIState` calls (`panels-2.md` §14 r1 table) and a trace of a Trade with an empty store. (b), q-play-smoke: The play host's item-move rest answers the room delete record `0x0061A270` (`items/inventory-moves.md` §8, the pickup's room step) with S→C 0x0A (type 4, GUID) to the staged players at once (`d2-server` `handlers/world.rs` `PreviewMoveRest::room_delete_notice`); when 1.14d sends a room's delete list (room +0x18, `skills/bodies.md` §5) is not written. Without it a picked-up gold pile, freed on the server, stayed on the client's ground. Needed: the room pass that sends the delete list, from a trace of a gold pickup. (c), q-play-smoke: The position check follows the server while the play preview predicts the local player's walk (`d2-client` `bridge/check.rs` `Checked::Followed`, `bridge/world.rs` `ClientWorld::local_walk` / `predicted`, `world_view/walk_room.rs`; `client/model.md` §6 r8, OQ2): rule 8 for the local player sends no C→S 0x5F; the server's point stands (rule 3 stores it and `Predict::observe` snaps the prediction to it). Rules 3–7 are unchanged (the own position is the model's, so `app_play_visibility` still asks the predicate). Before, rule 8 sent 0x5F with the last placement (or, on q-smoke-town / pc1's local-cell variant, the straight-line predicted cell past an obstacle), and the server's resync (`sim/pathing.md` §1.6) walked or snapped the player there: the rubber band on real data. d2rs-own, unverified: 1.14d's own position is its own path (same path code, same rooms), so it parts from the server's only when the two truly disagree; the preview's guess does not see collision and is never better than the server's point. A placement since the prediction was recorded (the model position moved) restores rule 8 as specified. Open: the drawn player can still stand past a wall after the server stopped (no 0x96 follows a stop, `combat/vitals.md` §5.3 step 3.4); settled by a client path step over the client rooms (REC-51, `model.md` OQ2). Check: `cargo nextest run -p d2-client --test play_smoke` (`Run::check` asserts no 0x5F), `--test app_play_visibility`, `--test app_level_border`, `-E 'test(position_check_vectors)'`. (d), q-play-smoke: The walk prediction steps the local player's own path over the client DRLG (`d2-client` `bridge/client_path.rs` `ClientPath`, `bridge/predict.rs` `Predict::path_step`; `client/model.md` §3 r3, OQ2): a new walk (or a snap to a server point) places a player path at the predicted cell (`path-placement.md` §2.4) and runs the player mode request (`pathing.md` §1.2, walk 2 / run 3); each server tick runs the player event 0 step (§9.2); the path's position is the drawn position and its stop ends the walk. Without a client DRLG or an active room at the cell, the straight line stays. d2rs-own, unverified: that the 1.14d client step `0x00463390` runs these functions with these inputs (velocity percent 100 plus the run bonus, the model's stamina, no state, no used skill, no run drain), and that client units leave no footprints on the client grids (the player's footprint goes on a private copy; other units' footprints are absent, so a path the server bends round a monster is drawn straight). Needed: REC-51's `record_frames.py` over a run into a wall and round an obstacle (the client path x / y per frame against the server's). Check: `cargo nextest run -p d2-client --test play_smoke` (`a_blocked_run_is_drawn_where_the_server_stops`; every run leg asserts the drawn player stands within 2 sub-tiles of the server's).
- **REC-278** Synthetic town smoke data and the town fixes it found (PROVISIONAL, `d2-client`, `d2-sim`, `d2-server`; M22; q-smoke-town, `// d2rs-own, unverified` unless named): (1) the synthetic Kurast Docks NPCs (`app/town_npcs.rs` `act3_docks`) stand in a row at (3 + 4·i, 12) of that room; needed: the Kurast Docks preset positions (a live `play` run in Act III, the NPC units' S→C 0xAC). (2) The synthetic stores (`app/synthetic_vendors.rs`, `app/synthetic_items.rs`): a cap and a buckler with made-up costs, defense and durability, every trader's column, one price row per trader (sell 1024, buy 512, repair 128, max buy 5000). (3) The synthetic vitals (`app/single_player.rs` `synthetic_charstats`, `synthetic_vitals`): sorceress-like start attributes, life, stamina and per-level gains, skill slots −1, experience 500·L²; the synthetic `itemstatcost` marks stats 0–15 `Saved` (the 1.14d set). The user's tables replace (2) and (3). (4) `InvDesk::take_cursor` (`d2-sim` `wiring/inventory/host.rs`): a vendor sale of the cursor item sends S→C 0x42 [0][player GUID] before the free; needed: a recorded sale from the cursor. (5) The shop close also closes the inventory (`ui/shop_ui.rs`, `set_ui(INVENTORY, 1)`); needed: a recorded shop close. (6) The NPC menu's Resurrect insert reads `merc_state` from S→C 0x9B; until one arrives the model holds 0xFFFF (no mercenary), not 0; needed: the client's initial `[0x00725494]`. (7) The start items' update-list pass and reset (`d2-server` `WiredWorld::start_items`, `intents-events.md` §8.2 rules 3.5 / 3.10) run at the start items' end, not at the join itself; needed: a recorded new-character join (its 0x9C before the 0x23 pair). (8) Stat 15 `goldbank` joins the vitals sync's watched stats (`wiring/action/vitals_sync.rs`, within REC-96's stand-in for the changed-stat array flush). (2026-10-09, q-fix-flow-server: the act change's player re-add (0x59, part B, 0x0B, the vitals cache reset, `wiring/path/act_change.rs`) is gone: the act change follows `waypoints.md` §11, the player stays the local player and 0x04 comes from the client pass.)
- **REC-273** Model unit facts for the draw order (PROVISIONAL, `d2-client` `world_view/unit_facts.rs`, `model_feed.rs`, `play.rs`; M22; q-view-unit-facts): `ViewFeed::unit_facts` is answered from the model once `set_unit_fact_tables` has given the table columns (`unflatDead` through `MonStatsEx`, `DrawUnder`, leveldefs `LOSDraw`, loaded from the user's tables): flags +0xC4 (the bits a model rule writes: 0x2, 0x4, 0x200, 0x800000), flag-ex +0xC8, states 7 / 143 / 146, `unflatDead`, `DrawUnder`. In the preview these fill over the d2rs-own facts (flag-ex 0x80 and the sight answer stay the preview's). d2rs-own, unverified: flag bits no model rule writes (flat 0x100000, missile 0x10000) read 0; outside the preview, in a `LOSDraw` level, `sight_hidden` stays `None` (the §15 line test needs the client DRLG's collision grid) so the frame refuses; a monster class with no `monstats2` row or an object class past the rows refuses. Needed: the collision grid as `CollisionRooms` over the client DRLG, and a render of a corpse and a DrawUnder object to compare the order.
- **REC-269** Character panel extras (PROVISIONAL, `d2-client` `ui/char_feed.rs`, `app/hud.rs`; M22; q-char-panel-full, `// d2rs-own, unverified`): (1) the class line is the string-table text of the `charstats` `class` key (the loader's wide name, `fixups.md` §4), Font16 colour 0 in [sx + 193, sx + 310]; (2) the resist / defense colours read the unit's state ids against the `states` flags (rfblue/red, rcblue/red, rlblue/red, rpblue/red, armblue/red); a red state wins; the defense `holyshield` test uses state 101 alone, without the shield (item type 51) test; (3) next level is the `experience` row of the base level (the HUD's table, no second copy) through `next_level_value`; (4) the damage / attack-rating block draws for the left and right skill from the skilldesc `str name`, `descdam`, `descatt` of the skill: only `descdam` 1, 7, 18, 19, 20 print a value (min / max damage stats 21 / 22 plus stats 18 / 17 percent, no skill modifiers, `SrcDam`, elements or dual wield) and only `descatt` 1, 2 (stat 19 plus stat 119 percent, colour 0); other entries draw the name only; the skill state test `0x004D9FC0` is not applied; the to-hit popups are not drawn. Needed: a trace of the panel with a weapon and each skill; `skills/descriptions.md` §3 entries in `ui::skill_desc`.
- **REC-272** Preview weather: water floor, passes 4 and 9 (PROVISIONAL, `d2-client` `world_view/weather_view.rs`, `rules/draw_order/{sky,source}.rs`, `world_view/{model_feed,feed}.rs`; M22; q-weather-passes; `// d2rs-own, unverified`): (1) the weather state (`Weather`, `FloorContext`) lives in the play preview's feed; the local player's weather seed is `Seed::init_low(guid)` (the client seed is read-only in the model, `camera.md` OQ6), the update count is the server tick, the camera delta is the change of the camera unit origin between frames, the day period is 1 (`0x0061C100` has no model source), the video mode is 1 and the frame rate (`[0x007BB390]`) is 25, so the flash always draws; a level change calls `0x004726F0(0)`, act load runs on the first level and each act change; (2) pass 4 cels are DC6 `UncompOverlays\Rain1-4` / `bubble1-4` frame sets, draw mode 3 (additive), light -1, keyed pass 4 / pool / slot; pass 9 lines are drawn one 1x1 item per pixel (`blend-modes.md` §8 r1, the pixel list of `gdi_line_pixels`), the flash is a rectangle frame of the clipped size (§8 r2, mode 5); (3) the floors keep their water bit in the preview (`dry_floors` is off when weather is on) so the floor pass spawns splashes / bubbles; (4) `LevelRow` carries `Rain` / `Mud`; (5) lightning is never started in play: its only caller is `draw-order-2.md` OQ4, so pass 9 draws a flash only in tests (`start_lightning`); thunder sound is not played (sound deferred) and its two position rolls are not drawn. Needed: a weather capture (`capture.md` §3.4) of a rain level to settle the move counter (OQ3), the day period source, the draw-order OQ4 caller and the frame-rate value.

- **REC-280** Warriv in the synthetic Act I town (PROVISIONAL, `d2-client` `app/town_npcs.rs` `ACT1`; M22; q-smoke-travel; `// d2rs-own, unverified`): the synthetic Rogue Encampment had Warriv's class rows but no Warriv unit, so the Act I → II NPC travel (`npc.md` §8.3) could not be taken in the preview. He now stands 3 sub-tiles from the town room's origin, beside Charsi. With game files the town presets place him. Needed: nothing for fidelity (synthetic world only); the live town's preset position replaces it.
- **REC-288** Arrival walk-outs in the walk prediction (PROVISIONAL, `d2-client` `bridge/predict.rs` `Predict::server_walk`, `Predict::observe`, `bridge/world.rs` `ClientUnit::mode_requests`; M22; q-fix-travel-arrivals; `client/model.md` OQ2): an S→C 0x0D code 1 for the local player (player mode request code 0x01, walk to (r0, r1), `model.md` §8 r4) sets the preview's predicted walk, held until the player's client room holds the target, so the warp (`path-placement.md` §12.2 r5–6), portal (`objects.md` §12 r11) and waypoint (`waypoints.md` §7 r7) walk-outs are drawn from the arrival point of the 0x15 that follows; a walk the player sends drops a held one. Not 1.14d facts: that the walk outlives the 0x15 placement (its teleport zeroes the path point count, `path-placement.md` §6 r4) and the hold. Consequence: a waypoint arrival is drawn at x + 3, y + 3 (the 0x0D's offset, `waypoints.md` edge case 5) while the server player stays at x, y (inside the position-check tolerance, so nothing corrects it). Also: a level change the prediction did not walk into (another act, or the predicted cell outside the player's room) now snaps it to the model (an act change landing on the old act's arrival cell left the player drawn where it last walked). Needed: `record_frames.py` over a waypoint arrival, a cave warp and a Town Portal (REC-51's hooks: 0x00463390 local path x/y per frame, the server position): does the client walk to the 0x0D target after the 0x15, and does a waypoint arrival step 3 sub-tiles?
- **REC-430..REC-436** Unit sounds in play (PROVISIONAL, `d2-client` `audio/unit_feed.rs`, `audio/driver.rs`, `ui/msg_ui.rs`; M22; q-fix-audio-sounds; `triggers.md` §4–§6, §8–§10, `triggers-2.md` §18, §21): the pass turns the client model into the calls of `UnitSnd.cpp`. (REC-430) a player or monster has no animation frame in the model; f starts at 0 at each mode change and adds the AnimData speed per client update, F = frames × 256 (settled by R-ANIM-1). (REC-431) the floor material k is the `soundenviron` `Material 1` default; no tile flags (R-FLOOR-1). (REC-432) units run in ascending key order (R-UNITORDER-1). (REC-433) a player's weapon hit class is the right-hand body item's `hit class` (R-HITCLASS-1). (REC-434) a unit first seen plays `Init`, its states' on-sounds and the drop sound of a dropping item (R-FIRSTSIGHT-1). (REC-435) skill start, missile hit and `ProgSound` are not requested (R-SKILLSND-1). (REC-436) the interaction greeting uses mode 0 (R-GREETMODE-1). Also d2rs-own: the local player's mode is the preview's drawn walk / run mode (REC-51), and `NPC Speech` changes only after the first seen value (OQ 7).
- **REC-440** Client object animation set-up (PROVISIONAL, `d2-client` `bridge/objects/mod.rs` `anim_setup`, `bridge/msg/units.rs` `assign_object`, `bridge/modes.rs` `object`; M22; q-cloud-game, q-fix-render-real-object-anim; `world/objects-client.md` §25 r8): S→C 0x51 runs the `world/objects.md` §4 set-up in its mode byte on the object's client seed (speed `roll(d >> 3)` + d − (d >> 4), `Sync` ≠ 0 → d), and the 0x0E code 3 mode change runs it again in the new mode, every time; the generic step adds the unit's speed. Measured, not traced: the `a1-town-arrival-ama` recording (Wine, `frames.jsonl` f 3–113) fits N2 125, RB 124 on the first seed draw and three torches 191, 188, 199 on the second, the torches' frame counting from one tick later (their init's mode set sends a 0x0E); every object frame of ticks 13, 73, 113 then matches. Open: which call inside `0x004BC720` / `0x004BCF60` runs the set-up, and whether `reinit` / `set_mode` of the client functions (§26) also draw (d2rs: no draw, speed back to `FrameDelta`). Settles with a hook of `0x00624390` on client units (Ghidra read of `0x004BC720`, `0x004BCF60`).
- **REC-404** Border substitution stamp file (PROVISIONAL, `d2-sim` `drlg/outdoor/tilesub.rs` `SUB_STAMP_FILE`; M22; q-fix-cold-plains; `drlg/outdoor-tilesub.md` §2.3): a replacement's stamp makes no build-list roll (recorded, Blood Moor and Cold Plains); its file is 0 and the build list is not touched (only multi-file pieces 4–7 would show it). Settled by R-SUBFILE-1.
- **REC-410** Overlay id of the 0x11 in the monster update (PROVISIONAL, `d2-sim` `stats/lists.rs` `overlay_to_send`, `wiring/action/unit_update.rs`; M22; q-fix-proto-rest; `sim/intents-events.md` §7.3 r2 step 9): stat 178 of the overlay list read as its base value; sent when 0 ≤ v ≤ the overlay count. Settled by R-OVL11-1.
- **REC-411** 0xA5 names the unit the record sits on (PROVISIONAL, `d2-sim` `wiring/action/event_records.rs` `EventRecord::Landing`, `wiring/interaction/skill_use.rs`; M22; q-fix-proto-rest; `sim/intents-events.md` §7.9 r2). Settled by R-LAND-1.
- **REC-412** 0xAB send gate (PROVISIONAL, `d2-sim` `wiring/action/event_records.rs` `send_event_records`, `wiring/action/units.rs` `send_life_fraction`; M22; q-fix-proto-rest; `sim/intents-events.md` §7.9 r2): only the revived-unit test `0x00451F30`; the hostility test `0x00554200(unit)` is not applied. Settled by R-HEAL-1.
- **REC-413** 0x99 / 0x9A record bytes (PROVISIONAL, `d2-sim` `wiring/interaction/skill_use.rs` `queue_item_cast`, `wiring/action/event_records.rs`; M22; q-fix-proto-rest; `sim/intents-events.md` §7.9 r2): level byte = the cast level cut to a byte, w = the aim flag. Settled by R-CAST-1.
- **REC-414** 0x73 field sources (PROVISIONAL, `d2-sim` `units/messages.rs` `client_missile`, `wiring/action/switch.rs` `missile_add`; M22; q-fix-proto-rest; `missiles/missiles.md` R2.4): the 16.16 path position, `points[0]` as the first point, the current frame, level cut to a byte, stat 328; sent for a `ClientSend` row, an existing owner and a non-zero path velocity. Settled by R-MISS73-1.
- **REC-415** Senders of 0x20 and 0x93 (PROVISIONAL, `d2-sim` `units/messages.rs` `stat_update`, `skill_bonus`; M22; q-fix-proto-rest; `client/msg-stats-items.md` §1 r4, `client/msg-skills.md` §9): the builders exist, no d2rs code calls them (no spec names a caller). Settled by R-CALLERS-1.
- **REC-416** Client 0x92 on a derived inventory (PROVISIONAL, `d2-client` `bridge/msg/items.rs` `remove_items_display`, `bridge/item_lists.rs`; M22; q-fix-proto-rest; `client/msg-stats-items.md` §5 r5): nodes = the unit's items by their last record, effect = `ItemData::unlinked` until the next record. Settled by R-92-1.
- **REC-405** Per-client inventory refresh without its item steps (PROVISIONAL, `d2-sim` `wiring/action/dispatch.rs` `client_update_messages`; M22; q-fix-flow-server; `// d2rs-own, unverified`): the per-client update (`tick.md` §6 rule 5) sends the player's changed-stat array (`stat-lists.md` §11 rule 2) and, when flag-ex bit 21 is set (the join's item messages, `intents-events.md` §8.2 rule 3.5), the inventory refresh's S→C 0x48 (`inventory.md` §5.7 step 8) at the spec's place; the refresh's item and skill steps (§5.7 steps 1–7) are not run there, since the inventory model is the host's. Needed: nothing to record; run the pass through the host's lent inventory model when the tick hooks can reach it.
- **REC-406** The join sequence's 0x8D (PROVISIONAL, `d2-sim` `wiring/action/dispatch.rs` `join_sequence`; M22; q-fix-flow-server; `// d2rs-own, unverified`): between the join's 0x65 and 0x5A, `0x0055B620`'s S→C 0x8D AssignPlayerToParty (GUID, party word), as the recorded frame-2 order lists it (`intents-events.md` §8.3). Not specified: `0x0055B620`'s arguments: d2rs sends the 0x5B's party, 0xFFFF (no party); `client/msg-units.md` §8 r11 says no 0x8D was recorded, while §8.3 lists one. The multi-client part is REC-401. Needed: the recorded frame-2 bytes of `-022633` seq 220–224 compared with d2rs's (`app_single_player` session test).
- **REC-402** C→S 0x15 chat string check refusal code (PROVISIONAL, `d2-server` `dispatch.rs` `parse`; M22; q-fix-proto; `sim/intents-events.md` §2.4 r6): 2. Settled by R-CHAT15-1.
- **REC-401** Join sequence with more than one client (PROVISIONAL, `d2-sim` `wiring/action/dispatch.rs` `join_sequence`; M22; q-fix-proto; `sim/intents-events.md` §8.3): each client in state 4 gets the joiner's 0x5B and 0x65, then the join 0x5A. Settled by R-JOIN-1.
- **REC-400** S→C 0x3E UpdateItemStats sender widths (PROVISIONAL, `d2-sim` `units/messages.rs` `update_item_stat`, `wiring/inventory/pending.rs`, `wiring/interaction/{vendor_world,skill_use}.rs`; M22; q-fix-proto; `client/msg-stats-items.md` §5 r1.3): narrowest width per sized field, set flag 1, the item's layer-0 base value, param 0. Settled by R-3E-1.
- **REC-295..REC-299** Rendering facts export (PROVISIONAL, `d2-client` `facts/export.rs`; M22; q-render-compare; `specs/tools/facts-render.md` §5 r3, r5, r7, OQ 1, 4): d2rs wrapper names, tile X / Y as the handed position, cel mode / light / palette not exported (`?`), tick alignment, 1.14d cel sizes. Each one's settling capture: `docs/handoff/q-render-compare.md` §Provisional points.
- **REC-260** Chest drops in play (PROVISIONAL, `d2-sim` `wiring/action/objects.rs`, `wiring/economy/quest_host.rs`, `d2-server` `adapters/handlers/items/moves.rs` `update_pass`, `d2-client` `app/synthetic_items.rs`; q-chest-drops, M22): (1) the object view's chest drop, drop helpers and the quest `object_treasure` use `StartSpot` (the start spot as is, `treasure.md` §7 step 2 without the collision search; as REC-108) instead of `NoSpot`, so an opened chest creates its items. (2) Ground items were never announced: the item update pass ran only the players' update lists (the ground unit update of §6.3 part 1 is not run by the tick wiring). The pass now sends 0x9C (`announce_item`) once per client for each ground-mode item unit in the rooms adjacent to the client's room, tracked in `SimGame::announced_ground`, on only where `SimGame::announce_ground` is set (the play host; the unit hosts keep their pass) (`// d2rs-own, unverified`; the original uses unit flag 0x10, which the tick's room clean-up clears first). Items put on the ground by a player's own drop are marked into the update list as before and are also announced once by this path if their unit was never seen (a duplicate add the client upserts). (3) the synthetic game's chest table entry (`chest: [Some(1); 45]`) names a class picking the hammer row once. Settle against 1.14d: open a Blood Moor chest, record the 0x9C position and action (`record_packets.py`).
- **REC-263** Shrine state ends and refusals (PROVISIONAL, `d2-sim` `wiring/action/objects.rs` `ShrineWorld::apply_state`, `wiring/action/units.rs` `lists_expired`; M22; q-shrine-overhead): (1) an expired shrine state kept its state bit on forever (nothing ran the list's remove callback); the stat lists now queue the callback of a freed list and `UnitHooks::lists_expired` runs it after the type-12 walk: state off for the default callback, and for the stamina shrine's `0x00583A40` also stamina clamped to its maximum (the list set stamina to 2v). The bodies of `0x00583BD0` (skill shrine: skill refresh) and `0x00583A40` are not in any spec: the skill one is the default (levels read the stat, so there is nothing to refresh), the stamina clamp is d2rs-own, unverified. (2) The timed-state helper's step 1 (state outside the table → none) and step 4 (a list of the same state is refreshed: new expiry + timer, stats kept; the skill and level the shrine passes are not stated, taken as equal) are wired for shrines; the curse and monster rules cannot apply to a player's shrine state. (3) The shrine overhead text (0x26 type 5, decimal string id 3683 + shrine id) already drew through q-unit-fx's bubbles; a test now pins its frames (8 · length + 125). Needed: a trace of a shrine state's end (stats, stamina) and of the skill shrine's end.
- **REC-270** Skill tree panel in play (PROVISIONAL, `d2-client` `ui/skill_tree_ui.rs`, `ui/original.rs`, `app/hud.rs`; M22; q-skill-tree, `// d2rs-own, unverified`): (1) icon file prefix `CC` = `Am`/`So`/`Ne`/`Pa`/`Ba`/`Dr`/`As` per `panels.md` §10.3 (now specified): `spells\<cc>skillicon`, registered in `UiFiles` at `OriginalUi::new`; (2) the entries join `skills`/`skilldesc` rows with the local player's stats (level 12, str 0, energy 1, dex 2, vit 3, free points 5) and `SkillList`; the shown level is native base + native `level_bonus` + the base of item-owned entries (the full `skill_level`/`bonus_level` formulas of `skills/levels.md` are not run in the client); (3) learnability follows `panels-3.md` §25 r2 but the point cost is always 1 (`skpoints` formula not evaluated); a `reqskill` of 0xFFFF is "none"; (4) flag mask `[0x006CE270]` = 4 (the `InGame` bit). Not done: hover description, tab tool tips, free-points box, no-points message. Needed: a capture of a skill tree with learned and grey icons (remap `k` per state, spec OQ4).
- **REC-276** Trap missiles in the play host (PROVISIONAL, `d2-sim` `wiring/interaction/skill_use.rs` `UseView::target_position`, `d2-client` `tests/app_assassin_gaps.rs`; M22; q-trap-missiles; continues REC-233): (1) a monster's skill had no target position when its do function made a missile (the host seam answers only for the player's skill target), so `skill_missile` returned none and a laid trap never fired its missile; `UseView::target_position` now falls back to the unit's path target (the target unit's position, else the path's target point, both coordinates non-zero; read-only, no stale-target clearing, `pathing.md` §13.2 r2–3). (2) Missile damage setup (`0x0059F900`, `missiles.md` §R2.3 step 23) has no implementation in the play host (`Pending::missile_damage_setup` is a no-op), so any missile hits for 0; the test sets the missile's stats 21 / 22 itself. (3) Synthetic monsters have no unit-collision footprint in the grid, so a missile never reaches its hit test (collision word 0, `missiles.md` §R4 step 7); the test stamps bit 0x100 on the target's sub-tile. Test rows are made up (`// d2rs-own, unverified`): the shot skill is row 4 with `srvmissile` 0 and a faster missile row (vel 16, range 200). Not done: per-trap AI params, the real `skills.txt` / `missiles.txt` rows of Fire Blast / Lightning Sentry / Wake of Fire (synthetic game only), missile damage setup, unit footprints for spawned monsters. Needed: a recording of a Lightning Sentry (shots, gap, missile ticks, damage) to settle REC-233's range / gap / shots.
- **REC-271** Inventory item tints (PROVISIONAL, `d2-client` `ui/panels/inv_items_tint.rs` `ItemsUi::draw_tints`, `ui/hud.rs` frame 9; M22; q-equip-backgrounds; `ui/inventory.md` §2, §3 r2–r3, §6 r4): drawn before the item graphics for the page-0 grid items (hovered green / refused red / unidentified / usable blue) and the equipped items (hovered, refused, unidentified, else none). Refused = `ItemTips::can_use` fails (strength, dexterity, level; REC-242: requirement stat modifiers not applied) or item flag 0x4. d2rs-own, unverified: painted as OPAQUE `d2rs\hudfill` tiles (frames 5-8 + tint index, new frame 9 for tint 4) through `esc_menu::push_fill`, not the spec's translucent A2 blend (§2 r3; the UI sprite path has no blend). Not read: shooter / quiver, `0x004C2240`, `0x0062A4E0`, the quest-item term, cursor state 8, the placement tint with a cursor item (nothing is hovered then); the empty-slot pictures (`panels.md` §9.4) are not drawn. Check queued: a capture of the tinted pixels once the blend exists.
- **REC-271** Equipped-item tints (PROVISIONAL, `d2-client` `ui/panels/inv_items.rs` `equip_tint`, `ui/draw.rs` `UiDraw::Tint`; M22; q-equip-backgrounds; `ui/inventory.md` §6 r4, §2): an equipped item whose strength / dexterity / level requirement fails (`ItemTips::can_use`, REC-242: requirement stat modifiers not applied), whose flag 0x4 is set, gets tint 0 (refused, red); unidentified gets tint 4; otherwise none. Not read: the shooter / quiver term, `0x004C2240`, `0x0062A4E0`, the hover tint and the two-handed ghost. The scene has no fill primitive, so `ui_items` skips `UiDraw::Tint` (nothing painted yet); the grid-item tints (§3) and the empty-slot pictures (`panels.md` §9.4) are not drawn. Check queued: none (needs a fill primitive and a capture of the A2 blend, `inventory.md` §2 r3).
- **REC-281** Item smoke test fills (PROVISIONAL, M22; q-smoke-items; every point `d2rs-own, unverified`; builds on REC-260's ground announcement): (1) the item update pass forgets an announced ground item once it has no room (picked up, freed: `update_pass`, `SimGame::announced_ground`), so a drop from the cursor is announced again (0x9C action 2 while the drop's unit flag 0x1000 is set, else 0); 1.14d's per-unit update sends §6.3 part 2 for every changed tick instead (settle with a recorded drop / pick-up / drop session). (2) A treasure drop's spot becomes the item's static path also without the walk-back field (`death.rs` `Spots::placed`), and the inventory model's new item data of a ground item takes its path position (`InvDesk::sync_in`, `LifecycleHooks::path_xy`). When the desk puts an item on the ground (`add_to_room`), its static path follows (`LifecycleHooks::ground_item_placed`; the footprint is not moved). (3) Removal records: a freed ground item (gold picked up) is recorded with its path room on `ActionHooks::removed_items`; the per-client update's removals (`tick.md` §6 rule 5, `0x0053A770`) send S→C 0x0A type 4 for records in the client room's adjacent rooms; tick step 7 frees them. Other unit types are not recorded (the record-making path is not specified). (3b) Far pick-up: the desk records `walk_to_item` (`InvState::item_walks`); the wired host runs the player to the item (`PathCtx::approach_unit`) and, when the player's mode leaves walk / run / town walk at the start of a later tick, runs the 0x16 handling again with the same cursor flag (`world/item_approach.rs`, as REC-149's NPC approach); a new walk request drops it. (3c) A treasure drop of low / normal quality without affixes is identified (`ItemDrops::create`; `generation.md` §1.4 says only "set by callers"). (3d) The desk's `in_town` reads the unit side's levels (`LifecycleHooks::town_room`) for the stash page. (3e) The weapon switch gives each moved item command flag 0x200000 (0x9D action 0x17) and refreshes the owner (extends REC-177). (3f) The identify effect (REC-113) runs the inventory pass, so an identified charm's list links. (3g) The client's item list carries the armor's defense (stat 31) besides the property lists; durability, quantity and sockets are left out (`stat-lists.md` OQ2). (4) Synthetic data (`app/synthetic_items.rs` `smoke`): items `ssd ` (2 sockets), `axe ` (str 32), `cap `, `hp1 ` (type 76, beltable, useable), `isc `, `tsc `, `cm1 ` (one charm suffix: properties row 0, function 1, +3 strength), `box `, `gld ` (compact rows for gold, potion, scrolls; hp1 autobelt; `helm` ⊂ `armo` in the equivalence); cube parts with no recipe; the chest table (every act / difficulty / tier, REC-260's) names "Act 1 Chest A" (negative picks: axe, potion, both scrolls, charm, a gem); the smoke monster (class 5, killable, TC "smoke": sword, cap, gold, with a client unit row).
- **REC-287** Inactive room store in play (PROVISIONAL, `d2-sim` `wiring/action/inactive.rs`, `d2-client` `app/single_player.rs`, `tests/smoke_travel.rs`; M22; q-fix-idle-rooms; fixes q-smoke-travel B2): the play host turns the store on (`ActionHooks::enable_inactive_store`); tick step 9 stores the units of a freed room (`units.md` §3.3) and the room's next build restores them (§3.4 rule 4, the room is rebuilt populated, `rooms.md` §5, so only the restore runs). The §3.3 facts now come from the tables (leveldefs `SaveMonsters` and `objects` `Restore` / `RestoreVirgins` of the object state, monstats2 `restore` through `MonStatsEx`, monster data for the record's type flags, level, name seed, umods, superunique index), the host seam only where the table is absent (the synthetic game has no monstats2: seam default 1). The restore re-creates the units in d2-sim (the `Pending::restore_monster` / `restore_other` seams are gone): a monster is allocated again with its stored GUID in mode 1 / 12 (type init from the lent world), an "other" record is a new unit with unit flags 0x3000000, a kept unit (flag-ex 0x100) is placed again with flag 0x10; objects get byte +4, +0x78, +0xB8 back, a shrine its event 5 and shrine row. d2rs-own, unverified: (1) the restored monster's allied flag is the record's alignment-2 bit (the restore's alignment work, `population.md` §13 r4 `0x00547ED0`, and node-8 insert, `ai.md` §5, are not run); (2) unique / minion records get the saved umods, type flags, superunique index and name seed copied in, but the umod run `0x005A2120` is not repeated, and the stored life (+0x48) is not put back (the spec names no rule for it); (3) a kept pet's owner re-link `0x0058F350` is not run; (4) the well branch: one charge per `Parm0` frames since the store, capped at 2·`Parm2`, then one event 2 at frame + `Parm0` + 1 while below the cap (the spec names the re-schedules without a time); (5) ground items: no item writer is wired (`Pending::item_record` default), so an item is left as §3.3 says ("not freed here") and unlinked by the freed room, as before; (6) the synthetic world: every unit the build places gets unit flags 0x3000000 (they stand in for the live towns' preset units, `population.md` §11.1), and the synthetic waypoint, chest (`RestoreVirgins` 1), stash and tome rows have `Restore` 1. Needed: a live `play` run (§5 queue: leave the Rogue Encampment, wait for its rooms to free, come back: every NPC once, the waypoint once), and a recording of a revisited level's monster life and a well's charges after a long absence.
- **REC-291** Server saves on the flow's path (PROVISIONAL, `d2-server` `adapters/storage.rs`, `adapters/sim.rs` `Tick::tick` / `SimGame::save_characters`, `adapters/session_flow.rs` `leave`; `d2-client` `app/save.rs` `FileStore`, `app/play.rs` `leave_game`; M22; q-fix-flow-save; `flows/save-exit.md` §2 r2, §3 r1): (1) the 8192-frame save is raised at tick step 5 (`d2_sim` `Game::character_save_due`, `tick.md` §6 r3) but written after the sim's steps 6–11 of that tick, because the writer reads the whole game (the app's rest, quests, hirelings), not only `Game`; the file can hold a step-6..11 change of that frame that 1.14d's step-5 save would not. (2) The window close (and `--exit-after`) leaves through the same C→S 0x69 after the app stops (`play::leave_game`), so the server's leave writes the file; the 1.14d `WM_CLOSE` path is §O3's, its exact chain is `flows/save-exit.md` OQ1. (3) The save's inventory flag bit 1 clear (`items/inventory-moves.md` §6.1 r4, "save pending") is not done (no reader of the bit). d2rs-own, unverified. (5) `d2s-tool resave` and the play save's cross-check build the appearance inputs from a save alone (`d2-server` `character::save::equipment_of_save`): the weapon class `0x0064F380` is taken as the `wclass` index of the weapon in use (0 without one); it matters only for a crossbow (`d2s-appearance.md` §4 r4). d2rs-own, unverified; settled by the real-save check of §5 (q-fix-flow-save). (4) A new character's map seed is the fixed `DEFAULT_SEED` (1234), not 1.14d's fresh `time_value` draw (`sim/rng.md` §5.2), so dev runs and draw dumps stay reproducible (`single_player::game_seed`); a save's seed is restored when its town byte has 0x80 and no `--seed` is given (`d2s.md` §2.2 r8). d2rs-own, unverified. Needed: a recording across a frame % 8192 = 0 tick (the `.d2s` save time and contents against the game state at step 5), the 0x69 send on the Esc-menu path (OQ1), and a save → reload in 1.14d comparing level layouts (the DRLG's use of the restored +0xAB, `d2s.md` OQ9). Check: `cargo nextest run -p test-fixtures --test synthetic_game -E 'test(save)'`, `-p d2-client --test play_smoke -E 'test(the_live_run)'`, `-p d2-sim -E 'test(character_save)'`.
