# Handoff (updated 2026-10-06, branch `claude/docs-fold-2`: the remaining implementation, wiring and Phase 6 notes folded in)

Start here in a fresh session, after `CLAUDE.md` and `docs/METHODS.md`.
This file holds state, the next steps, the code and command map, and the
local run queue. Rules and facts live in specs (`specs/README.md`); this
file points to them rather than restating them. The detailed per-session
records (seams, public APIs, design choices, per-site questions) stay in
`docs/handoff/*.md`; each of the 28 notes folded here starts with a
pointer line. No note is waiting to be folded as of this commit.

Index (this file is long; read by section, METHODS M11): §1 State ·
§2 Next steps · §3 Code map (Phase 0–2 crates, tick / server, Phase 3
systems, wiring, Phase 6 client, tools) · §4 Command map · §5 Local run
queue (Done, A player, B Ghidra / spec edits, C game files and GPU,
Blocked) · §6 Environment · §7 Where facts live and open questions
(carried-over 1–8, tick T1–T5, tick replay TR1–TR4, parser R1–R2, Phase 3
systems by owner spec incl. the second set and wiring W1–W16 / WE1–WE9,
Phase 6 by owner spec) · §8 Lessons.

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
| 3b Messages + server host | `d2-proto` tables generated from the two TSVs; `d2-server` transport + host loop wired to `d2-proto` / `d2-sim` through adapters; **every intent handler is a stub** (returns 0, logs); unverified on recordings | `cargo test -p d2-proto`: generated tables equal the TSVs, spec size/classifier/layout vectors; `cargo test -p d2-server`: 43 tests (every synthetic vector of `intents-events.md` §1–§3, `tick.md` §1; one single-player host frame on the real adapters); `check_units.py`: 0 errors on all three recordings (14,034 schedules); `convert_tick.py --check traces/sim/tick/*.json`: 0 errors (CI) |
| 3c Units, stats, stat lists | `d2_sim::{units,stats}` implemented from draft specs, **unverified** (units spec confirmed on recordings for the event rules only; stats / stat lists await `record_stats.py`) | 115 d2-sim tests on the branch: every synthetic vector of `stats.md`, `stat-lists.md`, `units.md` §4.2; TSV checks with perturbation tests (`stat-ops.tsv`, `unit-handlers.tsv`, `unit-events.tsv`) |
| 3d Items, treasure | `d2_sim::{items,treasure}` implemented, **unverified** | items: 57 tests (113 in d2-sim on the branch); treasure: 48 tests (104 in d2-sim); both `property-functions.tsv` / `treasure-*.tsv` checked with perturbation tests |
| 3e Combat, skills | `d2_sim::{combat,skills}` implemented, **unverified** | 128 d2-sim tests + 3 ignored game-file tests (not run); `skillcalc.tsv` / `misscalc.tsv` checked with a perturbation test |
| 3f DRLG | `d2_sim::drlg` (levels + rooms) and the three level types implemented, **unverified**: `drlg::preset` (DrlgType 2), `drlg::maze` (DrlgType 1), `drlg::outdoor` (DrlgType 3: Act I and `outdoor-tilesub.md` in full, Acts II and IV as specified, Act III partial, Act V only the siege strip). No dispatcher composes the three as one `LevelTypes`, `DrlgError` has no variants for them, and the DS1 / DT1 providers are seams, so no level from the live tables is generated yet | 48 drlg tests (104 in d2-sim) on fakes; preset 26 tests (537 in d2-sim on its branch), maze 26 (538), outdoor 41 (553). Recorded vectors reproduced on synthetic inputs with the recorded seeds: Act I placement (start seed 4014346869, rects, DRLG seed after creation), Blood Moor room sub-theme pick, the maze Den of Evil and Spider Cavern vectors |
| 3g Missiles, monster AI, monster population and init | `d2_sim::missiles`, `d2_sim::monsters::ai` implemented, **unverified**; 17 AI functions of 148 have bodies, server-do index 1 only, no server-hit body (stubs log). `d2_sim::monsters::population` (regions, room population, pick, bosses and packs, spawn point, placement search, presets, ambient spawns) and `d2_sim::monsters::init` (creation after placement, type init, stats and skills, level, mods §14.1 full, §14.2 bloodraven only, umods §16–§22, 0xAC init-owned fields) implemented, **unverified**: no spawn RNG trace exists | 76 new tests (131 in d2-sim); `catalogues_match_tsv`, `ai_table_matches_tsv`, `implemented_matches_catalogue` with perturbation tests; population 35 tests (547 in d2-sim on its branch, `SPECIAL_PRESETS` TSV check with perturbation), init 39 + 1 ignored (`real_level_stats`; 551 in d2-sim on its branch, `umods_match_tsv` with perturbation) |
| 3h World (quests, waypoints, cube, NPC, vendors) | `d2_sim::world` (quests, waypoints, cube) implemented, **unverified**; Act II–V quest callbacks and several Act I ones are `unhandled`. `world::npc` (interaction, chat, menu, heal, hire, resurrect, services, act travel) and `world::vendors` (price, store generation, gamble, buy / sell / repair) implemented, **unverified**: no recording of hire, resurrect, heal, identify or services exists | 78 new tests (134 in d2-sim); TSV checks with perturbation tests; npc 33 + 1 ignored (`live_monstats_records`; 544 in d2-sim on its branch), vendors 60 (572; every synthetic vector, the three recorded prices, the recorded Charsi store order of 43 codes, the recorded 0x32 / 0x33 bytes, `vendors.tsv` check with perturbation) |
| 3i Skill use, vitals | `d2_sim::skills::use_` (validators and handlers, use at point / on unit, mode gate, skill start / do, delay, periodic and aura events, 0x3C select) and `d2_sim::combat::vitals` (player creation stats, 0x3A stat points, level-up, experience level factor, `add_experience` as far as creation needs) implemented, **unverified**; the per-skill bodies (`srvst` 64 and `srvdo` 152 filled slots of `table::FUNCS`) are the seam `SkillFunctions`; the kill experience gain is not implemented | 28 + 12 tests (551 pass, 3 ignored in d2-sim on its branch); every vector of `use.md` and `vitals.md`; `FUNCS` checked row by row against `functions.tsv` with perturbation |
| 3j Wiring | `d2_sim::wiring::economy` (items, treasure, cube and quest item seams on real `Economy`, `StatLists`, `UnitRecord`) and `d2_sim::wiring::action` (`ActionSim`: the combined `EventDispatch` + `TickHooks`; combat, missile, AI, DRLG room, waypoint seams) wired, **unverified**: the adapters add no rule of their own; every seam call without a provider is `Pending` / `…Rest` with the narrowest default | economy 17 integration tests (512 pass, 3 ignored in d2-sim on its branch, game seed `0x5EED`), action 19 (514 pass, 3 ignored; `cargo test -p conformance` tick replay unchanged); neither branch's count is a merged count |
| 3 not implemented | object operate / init, the per-skill function bodies (`use.md` OQ10), kill experience, path / position / movement (`units.md` path spec not written; wiring `Pending`), inventory and player data, Act II–V quests, the other 131 AI functions, server-do / server-hit bodies, mercenary spawn / init (spec not written) | specs exist as drafts or are unwritten |
| 4 Conformance | recording proven feasible; coverage tool done | `tools/trace-recorder`: 32,543 recorded RNG draws match the spec exactly. `py tools/coverage.py --summary` at this commit: 2,386 claims over 2,704 rule units (`py tools/coverage.py --summary` at this commit): unit 1,771 (65.5%), game-file 158 (5.8%), trace 30 (1.1%), verified 188 (7.0%), any 1,803 (66.7%) (the `coverage-claims` branch measured 1,520 claims, unit 1,072, verified 180 before the implementation notes landed; the game tier counts only while its latest local run passes, `COVERAGE.md` §3). Verified units come from the sim-0006/0007/0008 replays (tick.md 14, unit-order.md 12, rng.md §3 r2 / r4) and three ignored game-file tests (§5 C1, C8): the unit tier is claims by synthetic tests, not fidelity |
| 5 Local server + bridge | bridge design `specs/client/bridge.md` and skeleton `d2-client::bridge` done; **no `d2-server` adapter yet** (server reached through the `ServerLink` trait; every S→C id unowned, `bridge-dispatch.tsv` all `TBD`) | `cargo test -p d2-client bridge`: every synthetic vector of `bridge.md`, the dispatch TSV check with its perturbation test, a windowless Bevy `App` mirror test |
| 6 Client | design drafts `specs/client/{render-pipeline,assets,ui,audio}.md` (d2rs-own). Cloud tasks implemented (plain-Rust infrastructure, no original behavior): **C1** paths + loaders, **C2** residency cache, **C3** frames + atlas, **C4** scene + CPU compositor, **C5** GPU compute compositor (`gpu_compositor`: WGSL, integer math, no sampler, 16×16 workgroups), **C6** verify harness (`verify`: case files version 1, `--perturb`, `GpuCompositor` seam), **C6×C5** GPU half of the synthetic cases wired (`verify::gpu::Wgpu`, `[[unit]]` COF cases), **C7** COF composite mechanics (`composite`), **C8** UI core, **C9** controls file, **C10** audio core. Nothing original-behavior is reproduced: every §B point is a `TODO(spec: …)` hook or a `ComponentResolver` method. GPU byte-exactness is **unverified** on a real GPU (proven only on Mesa llvmpipe 25.2.8, Vulkan, a software adapter) | CI unit tests from each spec's vectors (`d2-client` per branch: frames 53 pass + 1 ignored, scene 20 tests, ui 20, controls 16, audio 25; `gpu_compositor` 11 + 2 ignored, `composite` 11 + 1 ignored, `verify` 191 lib tests pass, 5 ignored on the `p6-verify-gpu` branch); llvmpipe: all 12 `gpu_compare` cases and all 10 synthetic verify cases 0 differing, `--perturb 7` reports exactly 7 on every half; game-file and real-GPU halves queued (§5) |
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
   and `wiring::action`, §1 3j): combine the two into one host (they are
   separate modules, `wiring/mod.rs` has only `action` and `economy`);
   give the game-creation fields one home (`GameFields` in economy,
   `ActionHooks::game_seed` in action, `GameInfo` in `monsters::population`
   and `monsters::init`; none on `Game`, §7 Integration I7); replace the
   `Pending` / `…Rest` defaults with providers in this order: path and
   position (`units.md` path spec first), player data and inventory,
   monster data (`monsters::init` `InitHost`, `population::PopWorld` /
   `MonsterInit`), items (`ItemStore` for items as `UnitId`s), objects /
   interaction / messages; then `NpcWorld`, `NpcVendors` / `VendorWorld`,
   `UseWorld`, `VitalsUnits`; seed the game seed at game creation
   (W16). Provider order inside a group: the seam lists of §3 and the
   notes `wire-action.md` §4, `wire-economy.md` §5.
3. **`d2-server` intent handlers per system** (replace the stubs of
   `SimGame::unhandled` with calls into the system modules; give
   `TickHooks` a way to send messages, as the adapter's `tick` has the sink
   and the hooks don't). Each handler follows its spec's message section
   (`sim/intents-events.md` §2, plus `world/*` for 0x49, 0x4F, 0x2A).
4. **Bridge ↔ `d2-server` adapter** (`specs/client/bridge.md` §3 rule 1):
   add `d2-server` to `d2-client`'s dependencies and implement
   `d2_client::bridge::link::ServerLink` for the wired host:
   `send(Game|System, b)` → `Host::send_game` (`None` → `Sent::Filtered`)
   / `Host::send_system` (a classifier refusal is a link error, the bridge
   already refused those, §4 rule 3); `pump()` → `Host::frame()`
   (`ticked`); `receive()` → `Host::receive(LOCAL_CLIENT)` (each returned
   message is one chunk); `protocol_version()` →
   `d2_proto::PROTOCOL_VERSION`; errors boxed into `LinkError::Server`.
   Needed from `d2-server`: a constructor for a wired single-player host
   (real `MessageSizes`, `Intents`/`Tick` adapter, a `SessionHandler`,
   `SystemClock`) that connects client 0, and `Host` being `Send + Sync`
   (the Bevy resource holds `Box<dyn ServerLink + Send + Sync>`). Then an
   end-to-end test: a `Walk` through the bridge is drained and handled.
5. **Phase 6 in-app wiring** (C5, C6, C7 are implemented, §1): a
   render-graph node using `Gpu::from_device(render_device.wgpu_device()
   .clone(), (**render_queue.0).clone())`, copying `AtlasTextures` pages
   into the compositor's array layers (exact texture copy) or owning the
   array; a store that maps `(FrameSetKey, index)` → `scene::FrameId`
   (`composite::ComponentResolver::frame_id` is a hook for lack of one);
   port the `map` verify case to the compositor (`verify/map.rs` moves to
   `GpuCompositor`, the `Material2d` path goes, §A9). `AtlasFrames` and
   `SlotSource` already map `FrameId` → `AtlasSlot` for the verify path.
6. **Coverage** (`coverage-claims` is done, §1; the rest is test and tool
   work). Next test-writing targets are the units no claim names
   (`py tools/coverage.py` lists them per spec) and the specs with the
   lowest unit-tier share in `--summary` (`data/loading.md` 14.9%,
   `formats/mpq.md` 27.1%, `sim/rng.md` 30.8%, `monsters/ai.md` 32.8%,
   `sim/units.md` 39.0%, `items/affixes.md` 39.1%). Two
   policy / tool items from that session and `impl-vendors`: (a) specs
   with prose-only or one-table sections (`damage.md` §8, `cube.md`
   edge-case bullets, `stats.md` §6.3, …) are one unit, so a claim can
   cover more than its test checks; the fix is numbered items in those
   specs (spec work), after which claims can be narrowed; (b)
   `tools/coverage.py` lists rule `§9.2 r0` of `vendors.md` (the list starts
   at `0.`) but rejects the claim `§9.2 r0` as malformed (`r<N>`), so it
   cannot be claimed (the test is `price::rule_zero`): accept `r0` in the
   claim grammar or number from the first item. Rule: `Covers:` comments
   (`docs/COVERAGE.md`).
7. **Level-type dispatcher** (cloud, repo only; the generators exist, §1
   3f): one `LevelTypes` that dispatches by DrlgType (1 → `Maze::init_level`
   / `generate` / `reset_level`, 2 → `drlg::preset`, 3 → `OutdoorTypes`;
   `Maze` is not a `LevelTypes`, `OutdoorTypes` is an adapter that forwards
   everything else to `others`); add `DrlgError` variants for the three
   (`Maze`, `Outdoor`, `Preset`: the outdoor adapter reports non-DRLG errors
   as `UnknownLevel(id)` and keeps the real one in `last_error`); give
   `LevelTypes::door_unit` the orientation (§11 needs it; right door =
   orientation 9); implement `MazePresets` and `OutdoorPresets` on
   `drlg::preset` (`alloc_map`, `build_area`; both must draw `roll(Files)`
   on the level seed at `0x00666F33`); parse DS1 for `Ds1Source` /
   `SubFiles` from `d2_formats::ds1` in the server or world code (stored
   width / height, orientations as stored).
7b. **Providers for the new system modules**: `monsters::population`
   (`PopWorld`, `MonsterInit`), `monsters::init` (`InitHost`; its
   `LifecycleHooks::init_kind` must call `type_init` for monsters; the
   timer type-7 handler calls `handle_event7`, mode change `0x005A7C20`
   calls `dispatch(.., 0 / 1)`, combat `3 / 4`, missile creation `5`; the
   tick room pass calls `population::room_step` per active room in act
   room-list order; game creation calls `Regions::create`), `world::npc`
   (`NpcWorld`, `NpcVendors`; `NpcControl::new` steps the game seed once,
   third among the game-creation seeds, after the object-control seed and
   before `QuestControl::new`), `world::vendors` (`VendorWorld`; embed one
   `VendorRecord` per NPC record), `skills::use_` (`UseWorld`,
   `UseMissiles` through the action wiring, `SkillFunctions`),
   `combat::vitals` (`VitalsUnits`).

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
10. **Phase 6 RE specs**, in the order of `docs/PLAN.md` Phase 6: first
    `render/sprite-placement.md`, `camera.md`, `composition.md` and a
    frame-capture recorder (render §B1, §B2, §B7, §B9); then unit
    composite and draw order; shading, blend modes, lighting; audio;
    UI text, panels, controls, inventory, automap.

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
`p6-verify-gpu`).

Per-session notes live in `docs/handoff/*.md` (detailed record of seams,
public APIs, design choices and per-site open questions). The ones for the
28 sessions above are folded into this file as of this commit; read the
note of a module before you change it. Every note in `docs/handoff/`
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
| `crates/d2-server/src/adapters/{sizes,sim}.rs` | the seams on the real crates: `ProtoSizes` (`d2-proto` size lookup), `SimGame` (`Intents` + `Tick` on `d2_sim::game::Game`; staged `PlayerFields` / `UnitFacts`; stub handlers) | `sim/intents-events.md` §2–§4, `sim/tick.md` §3, `sim/unit-order.md` §7 |
| `crates/d2-server/src/transport.rs` | client duplicate filter, classifier, three server queues, drain (truncating copy) | `sim/intents-events.md` §2.1 |
| `crates/d2-server/src/dispatch.rs` | game message entry, dispatcher, gate, stubs, exact size, point/unit parse, chat check, 0x3C/0x51 decode | `sim/intents-events.md` §2.2–2.4 |
| `crates/d2-server/src/buffers.rs` | per-client 0x200-byte buffers, local delivery split, receive lists | `sim/intents-events.md` §3 |
| `crates/d2-server/src/host.rs` | tick driver, `Host::frame` (drain → tick → flush), flush, injectable `Clock` | `sim/tick.md` §1, §8; `sim/intents-events.md` §1 |
| `crates/d2-server/src/tests/` | vectors; `fakes.rs`: TSV-driven size fake, fake game; `adapters.rs`: the adapters, fake agreement, end-to-end host frame | |
| `crates/d2-client/src/bridge/` | client↔game boundary: `link` (`ServerLink` trait, `SendQueue`, `Pumped`), `intent` (encode + classifier routing), `receive` (split, dispatch, `ReceiveLog`), `dispatch` (TSV table, `HANDLERS`, `check`), `world` (`ClientWorld`, `UnitKey`, `addressed_unit`), `mirror` (Bevy `BridgePlugin`, `BridgeResource`, `UnitView`), `mod.rs` (`Bridge`: `send`, `send_bytes`, `frame`); tests in `tests.rs` | `client/bridge.md`, `client/bridge-dispatch.tsv` |
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
| `crates/d2-client/src/gpu_compositor/{mod,pack,device,harness,tests}.rs`, `compositor.wgsl`, `examples/gpu_compare.rs` | `SHADER`, `WORKGROUP` = 16, `GpuError`; `pack` (validate + serialize, re-bins with `scene::bin`), `Packed`, `GpuItem`, `Params`, `SlotSource`, `emulate` (the shader on packed bytes, CPU), `AtlasFrames`; WGSL `compose` + `to_rgba`; `Gpu` (`headless`, `from_device`, `compose`, `compose_rgba`; limits checked before dispatch); 12 synthetic cases, `compare`, `diff`, `perturb`, `Report`; CLI `gpu_compare [--case] [--perturb N]` | `client/render-pipeline.md` §A9, §A10 |
| `crates/d2-client/src/verify/{mod,case,map,gpu,tests}.rs`, `render-cases/*.toml` | runner over versioned case files: `GpuCompositor` trait, `GpuJob`, `GpuOutcome::{Image{indices,rgba}, NotWired, NoAdapter, Error}`, `build`, `run_cpu`, `compare`, `compare_indices`, `perturb`, `perturb_indices`, `Status`, `Summary` (exit codes 0 / 1 / 2); case format v1 strict parser (`[[frame]]`, `[[map]]`, `[[table]]`, `[[item]]`, `[[unit]]`, `[[expect]]`); `map.rs` = today's map verify; `gpu.rs` `Wgpu` (lazy headless adapter, two dispatches must agree); cases: `map` + 10 `synth-*` | `client/render-pipeline.md` §A10, `render/map-preview.md` |
| `crates/d2-client/src/composite/{mod,tests}.rs` | `Slot`, `slot_order` (§A7 step 2, strict COF checks), `ComponentRequest`, `ComponentFrame`, `ComponentResolver` (hooks: `frame`, `frame_id`, `place`, `shade`, `blend`), `UnitParams`, `ComponentDraw`, `build`, `CompositeError` | `client/render-pipeline.md` §A7, `formats/cof.md` |
| `d2-net`, `d2-verify` | stubs | |
| `tools/mpq-tool` | info, list, extract, check, formats, render | |
| `tools/data-tool` | `tables`: the Phase 2 cross-check; `links`: broken links in the live set; `gen-tables`: regenerate typed structs; `gen-proto`: regenerate `d2-proto`'s message tables from the TSVs; `dump-compare`: fix-ups vs a 1.14d memory dump; `patch check/render/diff`: mod stacks | `data/field-types.md` §6.7, `data/fixups.md`, `data/runtime-maps.md`, `data/patch-layers.md` §10 |
| `tools/trace-recorder` | Python debugger recording RNG draws from `Game.exe` (Windows); `dump_tables.py`: the excel tables and runtime maps in 1.14d memory after the load; `record_tick.py` + `check_tick.py`: server tick, timer events, unit lists; `record_packets.py` + `check_packets.py`: client↔server messages | `sim/rng.md`, `traces/FORMAT.md`, `data/runtime-maps.md`, `sim/tick.md`, `sim/unit-order.md`, `sim/intents-events.md` |
| `tools/trace-recorder/convert_tick.py` | tick recording → format-1 trace `traces/sim/tick/sim-NNNN.json`; `--check` replays traces through the `check_tick.py` model (CI) | `sim/tick.md` Test vectors |
| `tools/trace-recorder/{check_units,record_stats,check_stats}.py` | per-kind event rules U1–U11 on tick recordings; stat-list recorder (`stats-raw-1`) and checker | `sim/units.md`, `sim/stat-lists.md` |
| `tools/depcheck` | dependency rules (no Bevy outside `d2-client`) | |
| `tools/methods.py` | methods collection `docs/METHODS.md`: `check` (CI), `list`, `new`, `export` | `docs/METHODS.md` |
| `tools/spec_index.py` | section indexes in specs (`--check` in CI) | `specs/README.md` |
| `tools/coverage.py` | spec rule coverage: rule IDs from specs, `Covers:` claims from tests and checks, unit / game-file / trace table, uncovered list; `--check` (CI), `--selftest` | `docs/COVERAGE.md` |
| `tools/cloud-setup.sh` | cloud session setup (Linux libs, pinned Rust) | |
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
`d2-client` (§7 Phase 6).
`d2-sim` has no `d2-formats` dependency: tables arrive as `d2_data`
typed records, DT1 tiles as `TileInfo`, DS1 grids as `RoomGrids`.

## 4. Command map (what proves what)

| Command | Proves | Needs |
|---|---|---|
| `cargo test -p <crate>` | unit tests from spec vectors | repo |
| `cargo test -p conformance --test tick_replay` | `d2-sim::tick` reproduces every timer run and list snapshot of `traces/sim/tick/*.json`; perturbation tests (`a_*_is_reported_exactly`) report exactly the changed record (M08) | repo |
| `cargo clippy -p <crate> --all-targets -- -D warnings`, `cargo fmt --all` | lint/format gate | repo |
| `cargo run -p depcheck` | crate dependency rules | repo |
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
| `cargo run -p d2-client --example gpu_compare [-- --case NAME] [-- --perturb N]` | 12 synthetic cases: compute compositor equals `scene::compose` / `to_rgba`; `--perturb N` reports exactly N (M08) | GPU (no game files) |
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
| `cargo test -p d2-sim wiring` | the 17 economy and 19 action integration tests (real modules together, synthetic tables / DRLG level, fixed seeds); re-run `cargo test -p conformance --test tick_replay` after any change to `ActionSim`'s tick hooks (only steps 9 / 10 and the client room change are routed; the replay's own hooks do not use them) | repo |
| `cargo test -p d2-client` (modules `assets`, `frames`, `scene`, `ui`, `controls`, `audio`, `bridge`, `gpu_compositor`, `composite`, `verify`) | Phase 6 infrastructure vectors (design specs, CI) | repo |
| `D2_GAME_DIR=<game> cargo test -p d2-client --lib frames::tests::all_live_frame_sets_build_and_pack -- --ignored --nocapture` | every live DCC / DC6 / DT1 builds frame sets that pack and check clean in a 64-page atlas; prints largest frame (≤ 2046) | `game/` |
| `cargo test -p d2-client --test assets_game -- --ignored` | **does not exist yet** (queued §5 C5): canonical paths and new loaders on the live archives | `game/` |

## 5. Local run queue

Cloud sessions add checks here (command + what to look for); a local
session runs them, records the result, and removes the entry. Merged
2026-10-06 from the 13 notes in `docs/handoff/`, then from the 15 notes
of the second fold (`wire-*`, `p6-gpu`, `p6-verify`, `p6-cof`,
`p6-verify-gpu`, `coverage-claims`, `impl-skilluse-vitals`,
`impl-monster-*`, `impl-drlg-{preset,maze,outdoor}`, `impl-vendors`,
`impl-npc`) and the earlier queue, deduplicated and grouped by what the run
needs: **A** the player at the game (recordings), **B** Ghidra / spec edits
only (no game run), **C** game files only (no player; includes the checks
that need a real GPU). Order inside a group is the order to run.
"Expect" lines are the pass condition; a different result is a finding
for the owner spec, not a reason to change the numbers.

Index: Done · A player · B Ghidra / spec edits · C game files and GPU · Blocked.

### Done (kept for the record)

Done 2026-10-06 (local): fix-ups (`dump-compare
traces/raw/20261006-021210-tables`: 70/70 tables, every map with a d2rs
counterpart identical, nothing pending; `fixups_on_live_set` ok) and
patch layers (`patch_game` 5/5 incl. G1–G8; `data-tool patch check
game/patch-example/overhaul.d2stack`: exit 0, one N01 note, data digest
`66010ecda7c8df5b7135579888c536fd2a30287fb877848719a31b6c8f97a625`).

Phase 3 recordings: done 2026-10-06 (first skipped at the user's
request, then recorded when the user asked). Combat with missiles
`traces/raw/20261006-022304-tick.jsonl` (4,632 ticks, 24,894 timer runs
incl. 1,580 missile, 186 snapshots) and melee `...-021854-tick.jsonl`:
`check_tick.py` 0 errors. Messages `...-022633-packets.jsonl`:
`check_packets.py` OK, perturbation reported, new client ids 0x0C,
0x3A, 0x3B. Still open: a hosted (multi-client) game.

### A. Needs the player (record_*.py, Windows, `game/`)

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
  (`wire-economy.md` §7.3). (10) Combat checks
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
  none otherwise). (14) Monster population and init: one new Normal game
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
  `vendors.md` OQ6, OQ7); they decide N1–N8 and V2–V5 of §7.
- *Existing recordings, no player, only code to write.* (17) 0xAC assign
  decode: `write_boss_section` / `component_bits` against
  `traces/raw/20261006-015956-packets.jsonl` and `-022633-packets.jsonl`
  (`init.md` "Recorded checks" table); expect equal bytes.
  (18) Group sizes and creation order of `20261006-015554`, `-021854`,
  `-022304` (`population.md` Test vectors "Recordings"), once
  `MonsterInit` and the DRLG providers exist (also listed under Blocked).

### B. Ghidra / spec edits only (no game run, no player)

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


### C. Game files and GPU only (no player, no Ghidra)

Live-table checks for the new modules need a **home**: `d2-sim` has no
`d2-formats` dependency and the notes disagree on where (§7 Integration).
Until decided, run them as ignored tests in `crates/conformance` (or add
the dev-dependency) and record results here.

1. `cargo test -p d2-sim -- --ignored` (5 tests: 3 combat,
   `real_level_stats`, `live_monstats_records`): extract the
   tables first (`mpq-tool extract` to
   `game/extracted/patch_d2/data/global/excel/`; the tests read `.bin`
   files of `patch_d2` through `skills::tests_game`). Expect 5 passes:
   `codes_match_game_tables` (skillcalc / misscalc `code` columns equal
   `SKILLCALC_CODES` / `MISSCALC_CODES`; filters an `Expansion`
   separator), `real_skill_vectors` (Fire Bolt 36, Fire Ball 47, Frozen
   Orb 64, Teleport 54, `levels.md`), `real_table_constants` (`charstats`
   ToHitFactor / BlockFactor, `difficultylevels`), `real_level_stats`
   (`init.md` "Real 1.14d values": level, min / max HP, AC and XP of six
   Act 1 classes at Normal, Nightmare Blood Moor, Hell Blood Moor and Hell
   Cold Plains; the L-flag 0 `zombie1` row), `live_monstats_records` (47
   records from live `monstats`, each of the 43 `vendors.tsv` entries
   attached with its act / trader / byte 6, a Normal expansion `hireling`
   row for each seller; also check that the `hireling` name ids at +0x114 /
   +0x116 are present in the loaded table (fix-up) before `make_hire_list`
   runs on live data). A missing-file failure means the table lives in
   `d2exp` / `d2data`: adjust the path, not the numbers.
2. Stats on the live set: build `StatData` from `d2_data::fixup::apply`
   and assert the real-data rows of `stats.md` Test vectors: entries(7),
   deps(12) = 214…250, the A53 set on 214 / 215 / 218 / 219.
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
   `waypoints.md` §5 rule 1.
6. Cube: `cube::recipes(cubemain)` on the 1.14d set: 151 records, enabled
   146, ladder 21, version 100 on 96, op 28 on records 0, 1, 2, 148, 149,
   output kinds 0xFC 78 / 0xFF 48 / 0xFE 21 / 0xFD 1 / portals 1 + 1 + 1,
   133 mods all chance 0; then V1–V23 with a real items table.
7. Assets (`claude/p6-assets`): add `crates/d2-client/tests/assets_game.rs`
   (`#[ignore]`, reads `D2_GAME_DIR`) and run `cargo test -p d2-client
   --test assets_game -- --ignored`. Expect 0 refused names (every
   listfile name of the 1.14d archives canonicalizes and reads back with
   identical bytes); every `.pl2`, `.cof`, `.tbl` parses; every
   `data\local\font\**` `.tbl` → `Font`, every `data\local\lng\**` `.tbl`
   → `Strings`. A refused non-ASCII name means the strict check needs a
   rule (`assets.md` OQ2).
8. Frames: `D2_GAME_DIR=<game> cargo test -p d2-client --lib
   frames::tests::all_live_frame_sets_build_and_pack -- --ignored
   --nocapture`. Expect pass; the printed line gives parse errors (the 7
   known leftovers of `mpq-tool formats`, 6 of them DT1), frame-set and
   frame counts, largest frame ≤ 2046 (above that `render-pipeline.md`
   §A2 needs an oversized-frame rule).

9. COF slot orders: `D2_GAME_DIR=<game> cargo test -p d2-client --lib
   composite::tests::all_live_cofs_give_slot_orders -- --ignored
   --nocapture`. Expect pass; the printed line gives the COF count (3,604
   plus the junk `amblxbow.cof` as the one parse error, `cof.md` §Edge
   cases), the frame count and failures 0 (a failure lists file,
   direction, frame and error). Also answers `composite` OQ1: if a live COF
   lists a draw-order component without a layer record, or two records for
   one component (the module refuses both), `render/unit-composite.md`
   §B4 must state the rule.
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
15. Real GPU, no game files (record adapter name, backend and driver; the
    cloud result is Mesa llvmpipe only):
    - `cargo run -p d2-client --example gpu_compare`. Expect an `adapter:`
      line naming the real GPU, then 12 lines `PASS case …: 0 differing
      bytes (indices), 0 differing pixels (rgba)` (`vector-opaque`,
      `vector-chain1`, `vector-chain2`, `vector-key-order`,
      `vector-equal-keys`, `vector-index-table`, `vector-clip`,
      `vector-four-bins`, `empty`, `two-pages`, `offset-view`, `stress`),
      last line `all 12 cases: 0 differing bytes`, exit 0.
    - `cargo run -p d2-client --example gpu_compare -- --perturb 7`.
      Expect (M08) 12 lines `FAIL case …: 7 differing bytes (indices), 7
      differing pixels (rgba), 7 perturbed; first at (0,0): …`, then
      `Error: 12 of 12 cases differ`, exit 1. Any count other than 7 is a
      failure of the check.
    - `cargo test -p d2-client --lib gpu_compositor::tests::gpu --
      --ignored --nocapture --test-threads 1`. Expect `gpu_matches_cpu` and
      `gpu_perturb_reports_exactly_n` pass.
    - `cargo test -p d2-client --lib verify::tests::gpu_half -- --ignored
      --nocapture`. Expect the first line `adapter: <real GPU>`, pass.
16. Verify harness on the developer PC (`D2_GAME_DIR` set, GPU):
    `cargo run --release -p d2-client -- verify`. Expect `verify: 11 cases
    from …/render-cases`; the `map` case lines as in the last recorded
    `d2-client verify` (same K draw items, W×H, L,T; `PASS: GPU render
    matches the CPU reference exactly`); then `GPU compositor: adapter:
    <real GPU name> (<backend>, DiscreteGpu|IntegratedGpu, …)`; then for
    each of `synth-bins-span`, `synth-clip`, `synth-cof-frames`,
    `synth-cof-units`, `synth-index-table`, `synth-opaque`,
    `synth-order-keys`, `synth-order-stable`, `synth-shade-chain`,
    `synth-shade-one`: `CPU binned: 0 of P pixels differ`, `GPU indices: 0
    of P bytes differ`, `GPU: 0 of P pixels differ`, `PASS`; last line
    `summary: 11 pass, 0 fail, 0 error, 0 GPU not wired, 0 no adapter`, exit
    0. Then `verify --perturb 7`: every synthetic case `CPU binned: 7 of …`,
    `GPU indices: 7 of … bytes differ`, `GPU: 7 of … pixels differ`, `FAIL …
    CPU and GPU halves`; the map case FAILs with 7 (its app log); exit 1;
    any count other than 7 fails the check (M08). Then `verify --case map
    --perturb 5`: map FAILs (app log reports 5 pixels differ), exit 1. If
    the process panics or hangs after the map case's Bevy app (two wgpu
    instances in one process), record it; workaround: run `verify --case
    map` and the synthetic cases separately.

Kept entries (unchanged):

Parser robustness (branch `claude/parser-robustness`, 2026-10-06): parser
code changed in `d2-formats` (mpq decoders, animdata, dc6, dcc, dt1, ds1,
tbl, cof) and `d2-data` (`bin.rs`, `patch/apply.rs`). Run
`cargo test -p d2-data -p d2-formats -- --ignored`,
`cargo run --release -p data-tool -- tables` and
`cargo run --release -p mpq-tool -- check` / `formats`: expect the same
results as before (every block and format file decodes, 72/73 tables
identical). New whole-file limits that are implementation limits, not
observed 1.14d behavior (each an Open question in its spec): DC6 frames
and DCC direction boxes ≤ 64M pixels per file, DT1 block counts over all
tiles ≤ file length / 20, TBL key+value bytes ≤ file length. Any
`formats` failure naming one of these limits means 1.14d files exceed or
share data, and the limit must be redesigned (not raised by guess).

**Treasure** (`specs/items/treasure.md`, branch `claude/phase3-treasure`):
Memory dump (no player needed; extend `dump_tables.py` per M10): the TC
   array `[0x96C5EC]` (count `[0x96C5F0]`, 0x2C bytes each, entry list at
   +0x28 with count × 0x1C) and the chest table `0x96C5F4` (45 dwords);
   compare with `treasure.md` §1.
2. Recording (player): `record_rng.py` extended with hooks at `0x0055A6D0`

Phase 6 asset budgets (from `specs/client/assets.md` OQ 1, design
only, no code yet): measure the decoded size (`Σ width × height`) of
every live DCC/DC6/DT1 frame, per file and in total, and of the files a
town and a dungeon scene use; record the numbers in `assets.md` §A5 and
set the default budgets from them.

### Blocked (not runnable yet)

- Phase 6 §B1 decoded samples per live `.wav` (needs `formats/wav.md`).
- Phase 6 GPU: the `map` case through the compute compositor (the port of
  `verify/map.rs`, §2 step 5) will show the index-0 question (§7 Phase 6);
  the `sprite` / `unit` GPU cases wait for the same store and port.
- Smoke on a machine with audio, after `output::register` and
  `AudioPlayer` are wired in `app.rs`: `d2-client view` plays a scripted
  `Cue` set; audible, no underrun errors (not an exactness check).
- Original-defaults check of the controls file: needs
  `specs/ui/controls.md` (§B4): identical action list vs the 1.14d key
  config.
- Level types on the live tables (`drlg/levels.md` Test vectors,
  `rooms.md` §9.9, `outdoor.md` Test vectors, `preset.md`): the generators
  exist; blocked on the level-type dispatcher (§2 step 7), `DrlgError`
  variants and the DS1 / DT1 providers, and then run as ignored tests reading
  `D2_GAME_DIR`. Act 1 level list and town tiles: with `OutdoorTypes`
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
  pushes to `main` and on pull requests. A cold CI run takes ~40 min
  (Bevy), cached ~3 min.

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
`outdoor-tilesub.md`. All draft: rules
from the 1.14d disassembly, RNG draw order not yet checked on a trace. Format facts: `specs/formats/*`. Map rendering: `specs/render/map-preview.md`.
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
| Three parallel d2-server handler branches each added `[dev-dependencies] d2-data` and a field/generic to `SimGame`; git merged the two `Cargo.toml` sections silently into a duplicate key and the `SimGame` generics conflicted (2026-10-06, caught by the coordinator's build before the gate) | parallel sessions that extend a shared struct get one named owner per field in their prompts; the coordinator builds the touched crate after each merge, not only after the last (METHODS M21) |
| GPU render exactness | R8Uint indices, sRGB palette via `textureLoad`, `Msaa::Off`, `Tonemapping::None`, pixel-aligned quads |

