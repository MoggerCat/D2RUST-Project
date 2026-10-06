# Handoff (updated 2026-10-06, branch `claude/docs-fold`: the Phase 3 system and Phase 6 infrastructure notes folded in)

Start here in a fresh session, after `CLAUDE.md` and `docs/METHODS.md`.
This file holds state, the next steps, the code and command map, and the
local run queue. Rules and facts live in specs (`specs/README.md`); this
file points to them rather than restating them. The detailed per-session
records (seams, public APIs, design choices, per-site questions) stay in
`docs/handoff/*.md`; each of the 13 notes folded here starts with a
pointer line. Newer notes not folded yet: `wire-economy`, `wire-action`,
`p6-gpu`, `p6-verify`, `p6-cof`, `coverage-claims`.

Index (this file is long; read by section, METHODS M11): §1 State ·
§2 Next steps · §3 Code map (Phase 0–2 crates, tick / server, Phase 3
systems, Phase 6 client, tools) · §4 Command map · §5 Local run queue
(Done, A player, B Ghidra / spec edits, C game files, Blocked) · §6
Environment · §7 Where facts live and open questions (carried-over 1–8,
tick T1–T5, tick replay TR1–TR4, parser R1–R2, Phase 3 systems by owner
spec, Phase 6 by owner spec) · §8 Lessons.

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
| 3f DRLG | `d2_sim::drlg` (levels + rooms) implemented, **unverified**; the level-type specs (`preset`, `maze`, `outdoor`) have no code, so no real level can be generated | 48 drlg tests (104 in d2-sim) on fakes |
| 3g Missiles, monster AI | `d2_sim::missiles`, `d2_sim::monsters::ai` implemented, **unverified**; 17 AI functions of 148 have bodies, server-do index 1 only, no server-hit body (stubs log) | 76 new tests (131 in d2-sim); `catalogues_match_tsv`, `ai_table_matches_tsv`, `implemented_matches_catalogue` with perturbation tests |
| 3h World (quests, waypoints, cube) | `d2_sim::world` implemented, **unverified**; Act II–V quest callbacks and several Act I ones are `unhandled` | 78 new tests (134 in d2-sim); TSV checks with perturbation tests |
| 3 not started | monster population / init (`monsters/init.md`), NPC and vendors code (`world/npc.md`, `vendors.md`), object operate/init, vitals (`combat/vitals.md`), skill use functions (`skills/use.md`), level type generators | specs exist as drafts |
| 4 Conformance | recording proven feasible; coverage tool done | `tools/trace-recorder`: 32,543 recorded RNG draws match the spec exactly. `py tools/coverage.py --summary` at this commit: 386 claims over 2,704 rule units; unit 248 (9.2%), game-file 144 (5.3%), trace 2 (0.1%), verified 146 (5.4%), any 274 (10.1%). Claims exist in d2-sim `rng.rs`, d2-formats, d2-data, `check_rng.py`, and the Phase 6 modules; the Phase 3 systems and conformance are being claimed in a parallel session (`coverage-claims`) |
| 5 Local server + bridge | bridge design `specs/client/bridge.md` and skeleton `d2-client::bridge` done; **no `d2-server` adapter yet** (server reached through the `ServerLink` trait; every S→C id unowned, `bridge-dispatch.tsv` all `TBD`) | `cargo test -p d2-client bridge`: every synthetic vector of `bridge.md`, the dispatch TSV check with its perturbation test, a windowless Bevy `App` mirror test |
| 6 Client | design drafts `specs/client/{render-pipeline,assets,ui,audio}.md` (d2rs-own). Cloud tasks implemented (plain-Rust infrastructure, no original behavior): **C1** paths + loaders, **C2** residency cache, **C3** frames + atlas, **C4** scene + CPU compositor, **C8** UI core, **C9** controls file, **C10** audio core. **C5, C6, C7 not implemented** (GPU compositor, verify harness, COF composite; parallel sessions `p6-gpu`, `p6-verify`, `p6-cof`). Nothing original-behavior is reproduced: every §B point is a `TODO(spec: …)` hook | CI unit tests from each spec's vectors (`d2-client` per branch: frames 53 pass + 1 ignored, scene 20 tests, ui 20, controls 16, audio 25); game-file and GPU halves queued (§5) |
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
   Wire the event chain: `UnitSystem` (units) as `EventDispatch`, with
   `MonsterDispatch { MissileDispatch { rest } }` in front (decision
   needed, §7 Integration). Add the tick hook DRLG asks for
   (`TickHooks::room_removed`, §7 tick). Move `d2-server`'s staged
   `PlayerFields` / `UnitFacts` into `d2-sim` as the unit specs land.
2. **Wire the economy and action systems** (parallel sessions
   `wire-economy`, `wire-action` write `docs/handoff/wire-*.md`; not
   folded here).
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
5. **Phase 6 C5, C6, C7** (parallel sessions `p6-gpu`, `p6-verify`,
   `p6-cof`, notes in `docs/handoff/`): GPU compositor over
   `AtlasTextures::pages` + `Bins` + `MapTable::rows()`; verify harness
   (case files, `--perturb`); COF composite mechanics. C7 builds unit
   items with `sub` = slot. Resolve the `FrameId` / atlas-slot seam first
   (§7 Phase 6 integration).
6. **Coverage claims** (`coverage-claims` session): claim
   `crates/conformance`, `d2-server`, `d2-sim` `tick`/`units` and the new
   system modules, `d2-proto`, `d2-client` `verify`, and the checkers
   `check_tick.py` / `check_packets.py`; then fill gaps with tests
   (`intents-events.md`, `tick.md`, `unit-order.md`, `calc-expressions.md`,
   `loading.md`). Rule: `Covers:` comments (`docs/COVERAGE.md`).
7. **Level type generators** (preset, maze, outdoor): need their specs
   first (local, high); until then no real level exists for DRLG tests.

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
notes in `docs/handoff/impl-*.md`); Phase 6 C1–C4, C8–C10
(`docs/handoff/p6-*.md`).

Per-session notes live in `docs/handoff/*.md` (detailed record of seams,
public APIs, design choices and per-site open questions). The ones for the
13 sessions above are folded into this file as of this commit; read the
note of a module before you change it. Newer notes (`wire-*`, `p6-gpu`,
`p6-verify`, `p6-cof`, `coverage-claims`) are not folded yet.

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
| `cargo run --release -p d2-client -- verify` | GPU render byte-identical to CPU reference | `game/`, GPU |
| `py tools/trace-recorder/record_rng.py --seconds N` then `check_rng.py` | RNG spec matches the real game | `game/`, Windows |
| `py tools/trace-recorder/record_tick.py --seconds 200` (play by hand) then `check_tick.py <file>` | tick steps, timer queue and unit lists match `sim/tick.md`, `sim/unit-order.md` | `game/`, Windows, a player |
| `py tools/trace-recorder/record_packets.py --seconds 180` (play by hand) then `check_packets.py <file>` | message transport rules R1–R7 of `sim/intents-events.md` | `game/`, Windows, a player |
| `convert_tick.py --check traces/sim/tick/*.json` | committed tick traces replay through the spec model, round trip exact | repo |
| `check_units.py <tick file>`, `check_stats.py <stats file>` | unit event rules / stat-list rules on a recording | a recording |
| `check_tick.py --selftest`, `check_packets.py --selftest`, `convert_tick.py --selftest`, `check_units.py --selftest`, `check_stats.py --selftest`, `tools/ghidra/disasm.py selftest` | the checkers catch perturbations (M08) | repo (disasm: `game/`) |
| `py tools/trace-recorder/dump_tables.py` (~7 s) then `cargo run --release -p data-tool -- dump-compare traces/raw/<time>-tables` | d2rs fix-ups and maps equal 1.14d memory after the load | `game/`, Windows |
| `cargo test -p d2-sim` (per module: `items`, `treasure`, `drlg`, `world`, `missiles`, `monsters`, `stats`, `units`, `skills`, `combat`) | every synthetic vector of the system specs; each machine-table check (`stat-ops.tsv`, `unit-handlers.tsv`, `property-functions.tsv`, `treasure-*.tsv`, `skillcalc.tsv`, `misscalc.tsv`, `srvdo.tsv`, `srvhit.tsv`, `ai-functions.tsv`, `cube-ops.tsv`, `waypoints.tsv` expectations, `quests.tsv`) has a perturbation test (M05, M08). Proves the implementation matches the draft spec, not 1.14d | repo |
| `cargo test -p d2-sim -- --ignored` | 3 combat game-file tests: `codes_match_game_tables`, `real_skill_vectors`, `real_table_constants` (§5 group C) | `game/extracted/patch_d2/data/global/excel/` |
| `cargo test -p d2-client` (modules `assets`, `frames`, `scene`, `ui`, `controls`, `audio`, `bridge`) | Phase 6 infrastructure vectors (design specs, CI) | repo |
| `D2_GAME_DIR=<game> cargo test -p d2-client --lib frames::tests::all_live_frame_sets_build_and_pack -- --ignored --nocapture` | every live DCC / DC6 / DT1 builds frame sets that pack and check clean in a 64-page atlas; prints largest frame (≤ 2046) | `game/` |
| `cargo test -p d2-client --test assets_game -- --ignored` | **does not exist yet** (queued §5 C5): canonical paths and new loaders on the live archives | `game/` |

## 5. Local run queue

Cloud sessions add checks here (command + what to look for); a local
session runs them, records the result, and removes the entry. Merged
2026-10-06 from the 13 notes in `docs/handoff/` and the earlier queue,
deduplicated and grouped by what the run needs: **A** the player at the
game (recordings), **B** Ghidra / spec edits only (no game run), **C**
game files only (no player). Order inside a group is the order to run.
"Expect" lines are the pass condition; a different result is a finding
for the owner spec, not a reason to change the numbers.

Index: Done · A player · B Ghidra / spec edits · C game files · Blocked.

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
  `missiles.md` OQ1 (exit-path recording). (2) Think intervals:
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
  request and tables is the acceptance check for that module. (7) End-to-end
  RNG draw-order trace once `MissileUnits` / `AiUnits` exist: missile
  damage rolls on the missile seed, to-hit on the owner seed, AI draws on
  the monster seed, against `rng` traces with positions. (8) Treasure
  walk replay: recorded walk inputs (TC, `Q`, `L`, `F`, seed) through
  `treasure::walk` with a recording `DropSink`; compare draw order / count,
  creation calls (id, quality, index, flags) and NoDrop `N` at
  `0x0055A9B9`. (9) Replay the recorded packets of waypoints OQ1–3, cube
  OQ1–4, quests OQ2, OQ4, OQ5, OQ10 against `d2_sim::world` once its seams
  have providers; they decide the `TODO`s of §7 world. (10) Combat checks
  the specs request (`hit.md` OQ1–2, `damage.md` OQ1–2, `levels.md` OQ1):
  replay recorded entry / exit states through `hit_test`,
  `block_or_dodge`, `start_combat`, `apply`, `special` with a
  recording-backed `CombatWorld`. (11) A hosted (multi-client) game
  recording (still open from the Phase 3 recordings). (12) Audio, after
  `audio/triggers.md` and `sound-table.md` exist (blocked, see below):
  record the original's sound calls twice on a static scene (stability),
  then compare with `audio::log::compare_logs`.

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


### C. Game files only (no player, no Ghidra)

Live-table checks for the new modules need a **home**: `d2-sim` has no
`d2-formats` dependency and the notes disagree on where (§7 Integration).
Until decided, run them as ignored tests in `crates/conformance` (or add
the dev-dependency) and record results here.

1. `cargo test -p d2-sim -- --ignored` (3 tests, combat): extract the
   tables first (`mpq-tool extract` to
   `game/extracted/patch_d2/data/global/excel/`). Expect 3 passes:
   `codes_match_game_tables` (skillcalc / misscalc `code` columns equal
   `SKILLCALC_CODES` / `MISSCALC_CODES`; filters an `Expansion`
   separator), `real_skill_vectors` (Fire Bolt 36, Fire Ball 47, Frozen
   Orb 64, Teleport 54, `levels.md`), `real_table_constants` (`charstats`
   ToHitFactor / BlockFactor, `difficultylevels`). A missing-file failure
   means the table lives in `d2exp` / `d2data`: adjust the path, not the
   numbers.
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
- Phase 6 GPU: pages sampled through `textureLoad` equal the CPU bytes
  (verify cases once C5 and C6 exist); the `map` case port will show the
  index-0 question (§7 Phase 6).
- Smoke on a machine with audio, after `output::register` and
  `AudioPlayer` are wired in `app.rs`: `d2-client view` plays a scripted
  `Cue` set; audible, no underrun errors (not an exactness check).
- Original-defaults check of the controls file: needs
  `specs/ui/controls.md` (§B4): identical action list vs the 1.14d key
  config.
- Act 1 level list and town tiles (`drlg/levels.md` Test vectors,
  `rooms.md` §9.9): need the level type implementations. With the outdoor
  placer, `Drlg::create(0, 644409375, 0, 1, false, ...)` on the live
  tables must give the list 16, 15, …, 4 (head first) with seeds
  `init_low(4014346869 + id)`, the DRLG seed after creation and the 47
  town room seeds of `20261005-232125-rng.jsonl` seq 2425–2591; an ignored
  test (`D2_GAME_DIR`) building `TownW1.ds1` rooms through the preset type
  with the real Act 1 DT1s in server order 15, 7, 14, 8, 9, 21, 16, 22, 23
  and client order 34 → 0 must equal the `rooms.md` §9.9 table and the
  first 12 draws of seq 6822–6833.

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
  reading extracted files from `game/extracted/…`. Decide once.
- I2. Ratio row duplicated: `treasure::quality::ratio_row` and
  `items::quality::ratio_row` (version-limit argument) both implement
  `treasure.md` §6 step 3; the items note says merge when both land.
- I3. Event dispatch order: the units note has `UnitSystem` implement
  `EventDispatch`; the monsters note builds `MonsterDispatch {
  MissileDispatch { rest } }` and says everything else passes to `next`.
  Which wraps which is not decided.
- I4. Treasure module path: `treasure.md` header names `d2-data::treasure`
  (runtime form) and `d2-sim::items::treasure` (drops); the code is
  `d2_sim::treasure`. Update the header or move §1 into `d2-data`.
- I5. Test counts of `cargo test -p d2-sim` differ per branch (104–134);
  none is a merged count.

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
- **Dependency added** (M10): `toml_edit 0.25` (`parse` only) in
  `d2-client` for the controls file (already in `Cargo.lock` via Bevy).

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
| GPU render exactness | R8Uint indices, sRGB palette via `textureLoad`, `Msaa::Off`, `Tonemapping::None`, pixel-aligned quads |

