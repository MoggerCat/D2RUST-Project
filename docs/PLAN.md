# Plan

## Goal

A private, high-fidelity Rust reimplementation of D2 LoD (target 1.14d
behavior) used to build an online overhaul mod. Public release = the mod,
gated by (a) a check that the user has the original D2 LoD files and
(b) an online account.

"99.9%" is defined by the conformance suite: percentage of recorded original
behaviors the engine reproduces exactly.

## Decisions log

| Decision | Choice | Notes |
|---|---|---|
| Target version | 1.14d | Changed from 1.10f on 2026-10-05: 1.14d is the version owned. D2MOO (1.10f) is used as a guide; every behavior is confirmed against 1.14d. |
| RE tooling | Ghidra 12.1.4 on Temurin JDK 21, run headless | Scripts in `tools/ghidra/` (ours, committed); project and exports in `re/` (gitignored). Ghidra 12.1.x doesn't work on JDK 25. |
| Format parsers are strict | Odd files are investigated, not tolerated | A failing file is checked against the level tables. Unused leftovers go in the survey's documented `KNOWN_UNUSED` list (with spec notes). Live data quirks become spec rules (e.g. DS1 truncated groups). |
| Formats exit check | `mpq-tool formats` + visual `mpq-tool render` | Renders go to `game/renders/` (gitignored). Never commit them. |
| GPU correctness | CPU reference renderer + byte-exact GPU comparison (`d2-client verify`) | Headless offscreen render at 1:1. Exactness requires: R8Uint indices + sRGB palette via `textureLoad`, sRGB target, `Msaa::Off`, `Tonemapping::None`, quads on pixel boundaries. A capture is judged only once identical to the previous one (stable render), with a capped number of retries. |
| Map draw rules (1b) | `WALL_BASE` 80, hidden cells skipped, first-match tiles | Wall base and hidden rule decided from evidence (renders, probes); see `specs/render/map-preview.md`. |
| `d2-proto` message tables | Generated from `specs/sim/*-messages.tsv` by `data-tool gen-proto` into `crates/d2-proto/src/generated.rs` (METHODS M17, same pattern as `gen-tables`) | Strict TSV parser (M07); tests: staleness, TSV-vs-code with a perturbation test (M08). Typed structs only for fixed sizes with fixed-field layouts; variable messages (chat, warden, save chunks) keep descriptors only. 2026-10-06. |
| `d2-server` seam adapters | Adapters in `d2-server` (`adapters/`), not in `d2-sim` | `d2-sim` may not depend on `d2-server` (depcheck). Fields the seams read that no written spec puts in `d2-sim` (unit mode, state 54, player data, positions, owners, acts) are staged by the caller (`PlayerFields`, `UnitFacts`), never defaulted; they move into `d2-sim` with their specs. Unwritten intent handlers return 0 and record the intent. A negative C→S chat size stays its own result through the seam (`SizeError::Negative`, `Classified::NegativeSize`), as in `d2-proto`. 2026-10-06. |
| Reference sources | `../refs/<project>/`, outside the repo | Spec sessions only. GPL projects are read only. |
| Language | Rust | Toolchain pinned in `rust-toolchain.toml`. |
| Fidelity scope | Exact match everywhere, no "close enough" tier (`CLAUDE.md` rule 10) | Decided 2026-10-05. No boundary between "must be exact" and "may be approximate" can be drawn reliably, and one slipped area costs more debugging than loosening saves. Each area defines its comparison (bytes, pixels, decoded samples, ticks); unchecked features stay "unverified". |
| Methods collection | `docs/METHODS.md` + `tools/methods.py` | Decided 2026-10-05. General methods live there, each with its project binding (Here) and status (proven with evidence, or trial); `CLAUDE.md` keeps hard rules and project facts. `methods.py export` gives the general form for future projects. |
| Token budget | Cap 24M per 5-hour window, median target 80%, floor 60% (METHODS M15) | Decided 2026-10-06 with the Max 20x plan (was: under 6M). The floor is met with more independent work in parallel, never with padding. |
| Client engine | Bevy, pinned version (0.19.x at time of writing) | Client crate only. Upgrades are separate tasks between milestones. |
| Simulation | Plain Rust, deterministic, integer math, 25 Hz | Never depends on Bevy. |
| Network model | Server-authoritative; single player = local server | Same protocol for both. |
| Method | Clean-room specs → Rust | See `CLEAN_ROOM.md`. |
| Mod data | Patch layers applied to user's tables | Release never contains Blizzard tables. |
| Trace reading | `conformance` crate reads traces (`serde_json`); `d2-sim` stays I/O-free | 2026-10-05, RNG step. `conformance::Trace` checks the top-level fields of `traces/FORMAT.md`; per-behavior replayers (`conformance::rng`) check events. |
| Ownership gate | "Has the game" check + online account | Changed 2026-10-05. Required MPQs must exist, open, and contain the files the engine loads. No exact-hash match against a Blizzard release, since official installs changed over time. Reads only the chosen game folder. No CD-key validation logic. |
| Distribution | Private engine build; public mod only | Public mod contains our engine code, so clean-room records matter. |
| Exact pins (Phase 0) | Rust 1.99.0, Bevy =0.19.1 | Bevy 0.19.1 needs Rust 1.95 or later. Pins live in `rust-toolchain.toml` and the workspace `Cargo.toml`. |
| Dependency rules | Enforced by `tools/depcheck` (CI) | No Bevy outside `d2-client`, and `d2-sim` can't depend on proto/net/server/client. |
| Sim determinism lints | `crates/d2-sim/clippy.toml` + `float_arithmetic = deny` | Bans HashMap/HashSet, Instant/SystemTime and float arithmetic in `d2-sim`. |
| Reference install record | `tools/hash-manifest` → `traces/reference-install.toml` | Dev only, never shipped or used for gating. Records the local `game/` that traces come from, so a changed install is noticed. |
| Data source of truth (Phase 2) | The `.bin` set 1.14d loads; `.txt` compiler verified byte for byte against it | 1.14d never reads excel `.txt` in normal play (a missing `.bin` is fatal). Full policy: `specs/data/loading.md` "d2-data policy". Mods patch `.txt` cells and compile (`patch-layers.md`). |
| Per-table layouts | `specs/data/fields.tsv` + `tables.tsv`, extracted from 1.14d `Game.exe` | 92 field lists, 3,499 fields (`specs/data/schema.md`). Replace prose per-table specs; Rust table code is generated from them. Column meanings are specified by the Phase 3 specs that use them. |
| Archive search order | `specs/data/loading.md` §2: priority descending, ties newest-opened first | `ArchiveSet` (d2-formats) computes it from the open order; second group and video path assumed opened after the startup group (loading.md open question 1). Excel files resolve P → X → D either way. |
| Unspecified table callbacks (Phase 2) | Superseded 2026-10-05: implemented from `specs/data/callbacks.md` | Was: write nothing until specified, with their bytes counted as explained by the cross-check. Now every byte must match. |
| Data cross-check tool | `tools/data-tool` (`data-tool tables`) | Separate from `mpq-tool`: it depends on `d2-data`. Exit status 1 on any unexplained difference. |
| Spec process | One writer per spec, then executable checks | Facts confirmed against 1.14d with provenance; one owner spec per rule. Extra LLM review layers proved costly for little gain (2026-10-05). |
| Tick trace replay (2026-10-06) | `conformance::tick` runs `d2_sim::tick::tick` once per recorded tick; hooks apply each step's recorded inputs, the dispatch compares every run and applies the recorded handler's work; d2-sim's own work (step 6 clears, step 9 deactivation, freeing timers after their run) is produced, not replayed | The recording logs list primitives, d2-sim's API is coarser: each recorded primitive starts a d2-sim operation and the primitives it performs next must follow in the recording ("owed"). Unit allocation is split from `SUNIT_Add` in `d2-sim` (`alloc_unit`, `add_allocated`), as the spec has it, because a missile's init schedules a timer before the missile is listed. |
| Client↔game bridge (2026-10-06) | `specs/client/bridge.md` (d2rs design): the bridge carries the 1.14d message bytes both ways; S→C chunks are split with `d2-proto` and dispatched by id into a plain-Rust `ClientWorld`; ids without an owner spec (`specs/client/bridge-dispatch.tsv`, all `TBD` today) are counted, never interpreted; Bevy entities only mirror the model | Server reached through a narrow `ServerLink` trait in `d2-client` (send / pump / receive / protocol version) until `d2-server` is wired; one bridge frame per Bevy frame in `PreUpdate` = the 1.14d client frame (pump: drain → tick → flush, then receive), input later in the frame, so intents of frame k are drained at k+1. No interpolation or prediction in the bridge (supersedes the ARCHITECTURE line): between-tick views are original client behavior, owned by Phase 6 specs. The link must report `d2_proto::PROTOCOL_VERSION`; the bridge persists nothing. Chunks 1.14d asserts on (message > 0x204 or past the chunk end) stop the frame with an error. |
| Phase 6 client design (2026-10-06, drafts `specs/client/*`) | Draw list (plain Rust) → CPU reference compositor + GPU **compute** compositor with integer math (no fixed-function blending), proven byte-identical per verify case; UI is our own integer-pixel panel framework on the same compositor (not `bevy_ui`); audio exactness = decoded samples + voice log (tick, file, integer params), the mix itself is ours | Two fidelity links: original → CPU reference (captures; images stay in `game/captures/`, hashes in `traces/render/`) and CPU → GPU (`d2-client verify`). Every original rule is an RE owner spec listed in each draft's part (b). Assets: in-memory only, synchronous load on miss (never a dropped draw), deterministic LRU budgets. Controls file `d2controls 1`, strict TOML. |
| Tick core and unwritten specs (2026-10-06) | `d2-sim` owns step order, list iteration and flags; step bodies owned by unwritten specs are `TickHooks` methods (defaults do nothing), timer events go to `EventDispatch` | Lists are index-linked arenas with the original's insert rules, so iteration order is exact without pointers; unit specs plug in without changing the tick. |
| Spec rule coverage (2026-10-06) | Rule IDs read from spec headings and numbered lists (`§5.2 r4`); tests claim them with `// Covers:` comments; `tools/coverage.py` (Python, like `spec_index.py`) reports unit / game-file / trace coverage per spec; only game-file and trace count as verified | `docs/COVERAGE.md`. Comments over a TSV map: one owner per claim, no sync check needed. `--check` in CI fails on dangling claims, never on low coverage. Rows of spec TSVs are not counted yet. |
| Phase 3 system seams (2026-10-06, `docs/handoff/impl-*.md`) | Each system module in `d2-sim` takes the world through narrow traits (`StatHost`, `UnitHooks`, `ItemStats`, `SkillUnits`, `CombatWorld`, `MissileUnits`, `AiUnits`, `DropSink`, `LevelTypes`, `WaypointWorld`, `CubeWorld`, `QuestWorld`, …) with fakes in tests; tables come in as `d2_data` typed records (no `d2-formats` dependency in `d2-sim`: DT1 tiles as `TileInfo`, DS1 grids as `RoomGrids`) | Lets the groups be implemented in parallel and unit-tested without a provider. Cost: overlapping getters across traits, to be merged by the provider session (HANDOFF §3 "Seams"). Where the live-data tests live is open (HANDOFF §7 I1). |
| Machine tables in code (2026-10-06) | A spec TSV that mirrors hard-coded `Game.exe` data is either embedded and parsed strictly (`quests.tsv`, `quest-messages.tsv` via `include_str!`) or copied into a Rust constant that a test compares row by row (`stat-ops`, `unit-handlers`, `property-functions`, `treasure-quality`, `treasure-chest-acts`, `skillcalc`, `misscalc`, `srvdo`, `srvhit`, `ai-functions`, `cube-ops`); every check has a perturbation test | METHODS M05, M08. `waypoints.tsv` is an expectation, never read by code. |
| x87 arithmetic in the sim (2026-10-06) | No floats in `d2-sim` (rule 6): the max-life rescale is emulated with integers at a configurable precision (default 53 bits); treasure NoDrop uses an integer binary64 (`treasure::softfloat`, round to nearest even, checked against host IEEE in tests only) | Open: x87 precision control and out-of-range conversion (`stat-lists.md` OQ1, `treasure.md` OQ5); unreachable on 1.14d data for NoDrop. |
| Unwritten bodies are logged, not guessed (2026-10-06) | A function a spec names without specifying (server-do / server-hit bodies, AI functions, Act II–V quest callbacks, level generators) is a stub that logs `Unhandled::…` and does the neutral thing; narrowest reading wherever a spec is ambiguous, each with a `TODO(<spec ref>)` | Same rule as the `d2-server` handler stubs. Readings are listed in HANDOFF §7. |
| Phase 6 infrastructure (2026-10-06, `docs/handoff/p6-*.md`) | Paths: strict canonical `mpq://` form, anything else is an error naming the path (`CanonicalPath`). Residency: byte-budget `Pool` with deterministic LRU (frame, key), never evicts the current frame, evicts only when an insert needs room. Frames: first-fit shelf packer over 2048² R8 pages with 1-pixel gutters, frame sets go in whole or not at all, a whole page is evicted at once. Scene: `DrawItem` list stable-sorted by `DrawKey`, 32×32 bins, per-pixel `dest = blend(chain(src), dest)`; index 0 is transparent. UI: panels emit `UiDraw` into a sink (not `DrawItem`s); frame mapping is integer scale with bars rounded down; window < 800×600 refused. Audio: Q8 gains, 32.32 phase, nearest = `floor(phase)`, voice log `d2rs-audio-log 1`. Controls: `d2controls 1` strict TOML, `preset` required | d2rs-own design (specs part (a)); the original behavior behind each is an owner spec in part (b) and stays a `TODO(spec: …)` hook. |
| Dependency: `toml_edit` (2026-10-06) | `d2-client` uses `toml_edit 0.25` (`parse` only) for the controls file | METHODS M10: a real TOML parser over a hand-written subset; already in `Cargo.lock` through Bevy; chosen over `toml` for byte spans (line numbers in errors). |
| Wiring (2026-10-06, `docs/handoff/wire-*.md`) | `d2_sim::wiring::{action, economy}`: adapters implement the system seams on the real modules and add no rule; every seam call without a provider is a `Pending` / `…Rest` method whose default is nothing; one `ActionSim` is the `EventDispatch` + `TickHooks`, built on the unit dispatch of `units.md` §5, not on the `MonsterDispatch { MissileDispatch }` chain | Follows `tick.md` §5.6 (conformance-passing), which drops frozen types 0, 1, 2, 6, 7, 9, 10, 11, 13, 14; the chain runs type 10 for a frozen monster (open W1). Missile and AI stores are lent (`Option::take`) per call, a re-entrant call logs `WiringError::Reentrant`. Smallest changes outside `wiring/`: `TickHooks::room_deactivated`, `AiUnits::level_id(game, unit)`, `MissileStore::remove`, re-exports. Item seeds derive twice (allocator and `create_item`), checked equal by `EconomyError::SeedMismatch` (HANDOFF §7 I6). |
| Inventory and item moves (2026-10-06, `impl-inventory`, `impl-moves`) | Two modules from one spec by two sessions: `items::inventory` (§1–§5) on `InvWorld`, `items::moves` (§6–§11) on its own seams `InventoryOps` / `MoveUnits` / `MovePending` with the narrowest default per call; no adapter connects them or calls `moves::handle` from `d2-server`; spec readings that the spec leaves open are `TODO(spec: …)` at the site (HANDOFF §7 IV1–IV8, MV1–MV7) | Unwritten failure results read as "nothing" (result 0); every type test through itemtypes equivalence; the second session's "constants and item record" copies are an open integration item (§7 J11). |
| Path placement (2026-10-06, `impl-path-place`) | `d2_sim::path` implements `path-placement.md` §7–§12 over provider traits for the collision, unit, level and warp-tile views; fatal asserts return `PlaceError` instead of exiting; malformed `ExpField` data are errors, not panics; position history (§10 rule 7) stays out of `d2-sim` (wall-clock) | No implementor of `CollisionView` exists (`impl-path-core` not in this tree); callers still use their seams until HANDOFF §2 step 7j. |
| Frame composition (2026-10-06, `render-composition`) | The index framebuffer persists between frames; the pixel write reads `table[dest][src]` (the PL2 layout) in the CPU reference, the WGSL shader and `pack::emulate`; a frame starts from the previous frame with `clear_rows` / `clear_after` from the frame plan; the palette for presentation is `present_palette(pl2)`; a single-frame case starts from all zeros | Behavior change of `BlendOp::IndexTable` (was `[src][dest]`); `verify` case files keep their meaning because `[[table]]` rows are filled as `table[dest][src] = rule(src, dest)`; the PL2-vs-`.dat` palette and the `L` / `T` order stay open (HANDOFF §7 RN1, RN2). |
| Capture verify case (2026-10-06, `render-capture`) | A separate case kind `scene` and directory (`crates/d2-client/capture-cases`) so a bare `d2-client verify` never needs a recording; a capture whose scene source is not wired reports `SCENE NOT WIRED` (exit 2), never a pass; stability needs no renderer | `--perturb N` corrupts N pixels after the integrity checks; the recorder's own stability line is not trusted over the case's (HANDOFF §7 CP3). |
| Typed S→C builders (2026-10-06, `s2c-builders`, `proto-bits`) | `d2_proto::s2c`: a builder per message whose full layout a spec gives, `UNWRITTEN` bytes written 0 and ignored by `decode`, `parse` refuses ids without a full layout (`Unbuilt`), an audit of all 181 ids; `bits:` layouts in the TSV (`name:width` LSB-first from bit 0 of byte 0, widths 1–32, S→C only) | No `PROTOCOL_VERSION` bump (wire bytes unchanged); `d2-sim` keeps its own builders until HANDOFF §2 step 7k. |
| Property tests, sixth fold (2026-10-06, `prop-sim-core`, `fuzz-server`, `prop-worldsim`) | Reference models are written from the spec rules, not from the code, and perturbed through the code under test (M08); no `Covers:` claim on a property that overlaps unit-tier claims or sits in a `proptest!` macro the coverage tool does not parse; minimized failures become `regress_*` tests; failure persistence off | Seven `d2-sim` bugs and one bridge bug fixed at the root (HANDOFF §1 3w); `AnimError::Endless` is a deliberate d2rs refusal where 1.14d loops forever. |
| Game-file tests are blind-written (2026-10-06, `game-tests-*`) | 67 more `#[ignore]` tests with expected values taken only from specs; no `Covers:` claim until a local pass (an `Intended claim` line or the note's claim table says what to add); a failing test is corrected from the observation or the spec, not the other way | Same rule as HANDOFF §8's lesson of 2026-10-06; none of the 109 such tests has run. |
| Level types and monster / NPC / vendor modules (2026-10-06, `impl-drlg-*`, `impl-monster-*`, `impl-npc`, `impl-vendors`, `impl-skilluse-vitals`) | Same rules as the other Phase 3 systems: narrow seams with fakes, tables as typed records or embedded TSVs (`preset-tables.tsv`, `maze-specials.tsv`, `umods.tsv`, `preset-monsters.tsv`, `vendors.tsv`, `functions.tsv` parsed strictly or checked row by row with perturbation tests), narrowest reading with a `TODO(<spec ref>)`, unverified until a trace or live-table check runs | `d2-sim` still has no `d2-formats` dependency (DS1 arrives as `Ds1Input` through `Ds1Source`; HANDOFF §7 I1 for the live-data test home). Original fatals return errors (`PriceFatal`, `PresetError`, `TreasureError`) for the caller or `Ruleset` to decide. Wall-clock time is an input (`now`) to the vendors' store refresh. |
| GPU compositor (2026-10-06, `p6-gpu`, `p6-verify-gpu`) | Compute shader in WGSL, integer math only, no sampler or blending; 16×16 workgroups; output a u32-per-pixel storage buffer (RGBA pass a bit copy of the palette bytes); atlas an R8Uint 2048² texture array; verify compares both the index framebuffer (bytes) and RGBA (pixels), runs each job's GPU half twice and requires equal readbacks; exit code 2 for `GPU NOT WIRED` and `NO ADAPTER` (never a pass) | `wgpu = "29.0.3"` added to `d2-client` with Bevy's own backend features (M10). Cloud check on Mesa llvmpipe only: a software rasterizer says nothing about a real driver's integer and texture paths, so GPU exactness stays unverified until the local run. Case format v1 gained `[[unit]]` without a version bump. |
| Coverage claims (2026-10-06, `coverage-claims`) | A test claims a rule only when its assertions check that rule's outcome, with the narrowest ID that is fully true; no claim for code-vs-TSV consistency checks, M08 perturbation tests or d2rs-only errors; in `conformance` only trace replays are claimed | 10 partial claims were dropped on review. Open policy point: rules that are one unit (prose-only sections, tables, indented sub-lists) can be claimed by a test that checks the main behavior only; the fix is numbered items in those specs. Claims went from 386 to 1,520 on that branch. |
| Server intent handlers (2026-10-06, `server-items`, `server-world`, `server-skills`) | One module per system under `d2-server::adapters::handlers`, called from `SimGame::handle` before the stub (order: items, world, skills); a handler only finds the player, calls the `d2-sim` module and routes its sends and result code, with no field check, refusal code or message layout of its own. A C→S id with no owner spec stays a stub; an S→C message is queued only where its bytes are known (0x77), the rest (0x15, 0x5A, 0x3F, 0x9D) are recorded, not sent. Item and skill handlers are inert until `SimGame::items` / `SimGame::skills` is set; world systems arrive through `WorldHost<D>` with `SimGame<D, W = NoWorld>` | Each handler session changed no `d2-sim` code. Fatal paths of the modules (`NpcError`, `WaypointError`, `QuestError`, `PriceFatal`) end the game in 1.14d; here they are recorded and the handler returns `ResultCode::Malformed`. Only 0x49 (waypoints) runs on a real provider; NPC / vendor / quest ids wait for providers. Open: HANDOFF §7 J1–J4, SI1–SI4, SK1–SK9. |
| Wiring, second pass (2026-10-06, `wire-worldgen`, `wire-interaction`) | `d2_sim::wiring::{worldgen, interaction}` follow the first pass: adapters on the real modules, no rule of their own, each unprovided call a default-nothing method of `WorldPending` / the `…Rest` traits. `WorldSim` = `ActionSim` + `WorldState`; the act DRLGs share one `LevelTypes` (`WorldTypes`: DrlgType 1 → `Maze`, 2 → `Presets`, 3 → `Outdoor`); `Desk` is the NPC / vendor / quest world over `Economy`, with the vendors' world a `VendorDesk` because `NpcLink` needs the NPC-control block | Smallest changes outside `wiring/` (each needed by a seam): `DrlgError::LevelType(u32)`, `LevelTypes::door_unit` takes the orientation, `Drlg::active_room_seed_mut`, `MonsterInit::{allocate_monster, add_modifier}` take `&mut PopState`, `superunique_init` split in two, `NpcLink::make_hire_list` takes the NPC-control seed (store generation and the hire list draw on the same seed in one open). With the defaults no room population runs, only presets place monsters (the DRLG data population reads are in no spec). Open: HANDOFF §7 WG1–WG9, WI1–WI10. |
| Gap tests (2026-10-06, `gaps-*`) | Unit tests from specs for the rules no claim named, unit tier only; a rule a test cannot observe or the code does not implement stays unclaimed with the reason in the note; a game-file claim on an `#[ignore]` test counts in `coverage.py` at once but is verified only after a local pass (`COVERAGE.md` §3) | Code fixes: tile mode (`units.md` §3.1 r7), unique rarity as 32 bits (`quality.md`). Behavior-neutral: `Seed` `#[repr(C)]`, `check_server_files`. A spec/code deviation is reported to its owner, not fixed across modules (`units.md` §5 r4 vs `TimerQueue::schedule`). Four later sessions (`gaps-drlg-sim-world`, `gaps-combat-items-monsters`, `gaps-data-rng`, `gaps-client-formats`) changed no non-test code; the claim grammar of `tools/coverage.py` now accepts `r0`. Coverage at `4b5b0bf`: 3,259 claims, tested (any tier) 90.9%, verified 8.0% (the `fd37fba` figures were 3,164, 88.6%, 8.0%). |
| Determinism lint (2026-10-06, `determinism-lint`) | CLAUDE.md hard rule 6 is checked mechanically by `cargo run -p depcheck` (CI step): a lexical scan of the non-test sources of `d2-sim` for floats, hash collections, clock, ambient RNG, I/O, environment, global state and `unsafe`; `d2-sim` must not depend on `rand` / `getrandom`; exceptions only through `tools/depcheck/determinism-allow.txt` with a reason, a stale entry is an error | Zero real hits and no allowlist entry at `edad871`. "No `HashMap` iteration affecting outcomes" is enforced as the stricter "no hash collections"; a type alias in another crate or a float from a dependency is not seen. |
| Window app and render node (2026-10-06, `p6-window`, `verify-map`) | The game runs on a **dedicated server thread** behind `ThreadLink` (`Send + Sync` for any link, forwards each `ServerLink` call synchronously: request, then answer), because `SimGame<ActionSim<_>, ActionWorld>` is not `Send` (`DrlgWorld` boxed seams, `Rc<RefCell<WorldTypes>>`); the GPU path is a render-graph system on Bevy's own device writing the presented texture, no readback; the frame store is dense and append-only; the `map` verify case composes in chunks of ≤ 1024² and `--perturb` corrupts indices and RGBA at the same pixels; case-file format stays version 1; `play` is the default subcommand | `d2-sim` unchanged; per-pixel results do not depend on chunking (tested at sides 32, 64, 1024); the black window is by design until the §B specs exist (HANDOFF §2 step 5). |
| Synthetic install and property tests (2026-10-06, `synthetic-data`, `fuzz-data`) | CI tests the load path on a generated install: `d2_formats::mpq::writer` behind feature `test-support`, `crates/test-fixtures` (made-up rows, headers from `fields.tsv`, never a Blizzard value; dev-dependency only); `equiv_matrix` returns a `FixupError` after n² × 128 pops instead of hanging on an `equiv1` cycle | the 1.14d tables use a tiny fraction of the budget, so their matrices are unchanged; FZ1 asks the spec to state it. |
| CI layout and the local gate (2026-10-06, `ci-speed`) | five parallel jobs (`tools`, `clippy-core`, `test-sim`, `test-rest`, `client`), only `client` installs the Bevy libs; nextest plus `cargo test --doc`; `sh tools/gate.sh` runs the same steps locally | nothing dropped from the old `--workspace` run; timings are estimates until the first runs are read. |
| Wiring routes: the lent world (2026-10-06, `wire-routing`) | `WorldSim` owns the world state and lends it into `ActionHooks::monster_world` around timer events and tick hooks; a route finding the slot empty keeps its previous `Pending` answer | `ActionSim` alone behaves exactly as before; message paths from `d2-server` are not lent yet (HANDOFF J9). |
| Client d2rs-own rules (2026-10-06, `client-own-gaps`) | sound pool: canonical-path keys, one decode per path, `Arc<Sound>` as decoded, charged samples × 2 bytes; prefetch: request order without duplicates, `Pool::offer` never evicts or overruns, a taken entry ranks as last used by the previous frame; text: glyph by the record whose `code` equals the unit | the rank and `TextOpts` are open for the user (HANDOFF §2 step 7g, CG1, CG2). |
| Quest world host (2026-10-06, `quest-host`) | `TradeWorld::quests` runs the real `QuestControl` on `EconomyQuests` from the interaction `Desk`'s economy; the Kashya mercenary reward is run as `NpcControl::quest_mercenary` right after the quest call, in `Desk::quest_message`'s order, applied to every quest call | No rule added, `d2-sim` untouched; Act II–V callbacks still `unhandled`. Open: HANDOFF §7 QH1, QH2, EN1, J7. |
| Local server link (2026-10-06, `p5-local-server`) | `d2_client::bridge::local::LocalLink` implements `ServerLink` on `d2_server::host::Host` with the `bridge.md` §3 rule 1 mapping; `d2-client` depends on `d2-server` (depcheck allows client → server) | `PendingSession` stands in for session code until the session spec exists. `BridgeResource` needs `Send + Sync`; the real game's providers are unchecked. `bridge.md` open question 1 is answered by the adapter. |
| One wired host (2026-10-06, `host-merge`) | `SimGame<D, W>` has one host value `world: W` and `WorldHost<D>` is the one host trait (`npc`, `vendors`, `waypoints`, `quests`, `cube`, `skill`, `moves`, `take_sent`, `fault`); `WiredWorld<R, S>` (was `TradeWorld`) holds the economy parts, the cube and the quests over **one** unit store and **one** item store built from `ActionSim`'s own units, stats and hooks; the skill slot is a type parameter, not a `dyn`; `SkillSeams` is deleted and the message path's skill seams are `UseView`'s; game-creation fields have one home in the action wiring (`ActionHooks::game_seed`, `ai_info`, `UnitData::expansion`), the economy builds its `GameFields` from it per call; the player's interaction has one owner on the wired host (the trade rest's `NpcRest`) | No rule changed; the item, skill and e2e tests run on the new constructors, `prop_handle` / `prop_worldsim` ported with every property kept. Open: the drop's item store, the unique bits, the inventory copies, the player-data provider (HANDOFF §7 HM1–HM6, IS4, IS5) |
| Path provider opt-in, one set of seams (2026-10-06, `wire-path-sim`) | `d2_sim::wiring::path` answers every path seam from `d2_sim::path` only when `ActionHooks::enable_paths()` was called (`ActionHooks::paths: Option<Box<PathState>>`, `None` by default); the walk's duplicate seams, tables and types were folded onto the path core (`Walk<'a, C>`, `PathWorld: CollisionRooms`, one `DynamicPath`); the path record lives in `PathState::records` keyed by the unit | Every earlier test and fixture is unchanged with the provider off; the server's walk / run handler, missile flight on the provider (`pathing.md` OQ3), the treasure FreeSpot seam and the teleport footprint room are the open blockers (HANDOFF §2 step 7p) |
| Inventory on the host (2026-10-06, `wire-inventory-sim`, `wire-inventory-server`) | `InvDesk` implements the four seams of `items::inventory` / `items::moves` on the economy with one owner per field; the server maps the 23 ids of `items::moves::HANDLED` through `WorldHost::moves`, direct sends through `MoveRest::take_sent`, and the deferred item messages in `update_pass` **after** `d2_sim::tick::tick` (not inside the client pass of `tick.md` §6 step 5, which the action wiring does not implement) | No rule added; 1.14d walks the old room's adjacency in the tick of a room switch, the pass reads the new one (HANDOFF §7 IS2); the item bit stream of 0x9C / 0x9D is still open (`inventory.md` OQ1) |
| Camera and placement in the app (2026-10-06, `render-camera-placement`, `render-wire`) | Every frame is built through `rules::OriginalView` with the camera computed once from a `ViewFeed`; no player → no camera (`NoCamera`), never an invented origin; one drawn frame per server tick; unreproducible original behaviour (shake `t3 = 0` division, a product over 32 bits, unsupported DC6 `flip`, cut top-down cels) is an error, never a guess | The window stays black until the feed hooks and the recorder gap are filled (HANDOFF §2 step 7q); every number is unverified against 1.14d |
| Property tests: the model can be the wrong side (2026-10-06, `fix-statlist-prop`) | A property failure is first checked against the spec rule: the stat-list failure was a model error and the fix stayed in the model, no assertion loosened; PR CI leaves `PROPTEST_CASES` unset (the variable overrides every property's default); a scheduled deep run is proposed, not added | `ci.yml` unchanged; decision SF1 (HANDOFF §2 step 7s) |
| Asset pool prefetch rank (CG1) | Keep: a prefetched entry ranks as last used by the previous frame | User decision 2026-10-06. The client cannot tell reliably how far ahead of use a prefetch is; ranking it as recently used works locally. d2rs-own (`client-own-gaps` §6). |
| Text clip rect (CG2) | Both: `TextOpts::clip` carries the request's clip rect now; the rules decide what it does once `ui/text.md` exists | User decision 2026-10-06. The field changes no placement until the spec says how the original clips text; if the spec says otherwise the field moves or goes. |
| S→C byte builders stay in `d2-sim` (2026-10-06, `s2c-use`, option A) | **d2-sim keeps no dependency on d2-proto (s2c-use option A): the sim's byte builders are cross-checked against d2_proto::s2c in conformance.** `tools/depcheck` `FORBIDDEN` has kept `("d2-sim", "d2-proto")` since Phase 0 ("the sim does no I/O and knows nothing about transport or the client"); the claim "depcheck allows it" in `s2c-builders` §5 and the task was wrong, and lifting a layering rule is an architecture decision | The one-maker property is `conformance/tests/s2c_builders.rs` (7 tests: sim bytes == `encode()` and `parse` == the same `Message`, edge values plus a fixed sweep) and `d2-sim/tests/s2c_bytes.rs` pins the bytes; two makers in code, tied by checks. Options B (lift the rule for `d2-proto` only, the sim builders become one-line wrappers) and C (a transport-free `s2c` crate) stay open to the user (HANDOFF §7 S2C1). `bridge-dispatch.tsv` stays all `TBD` (S2C2). |
| One item store and one inventory per game (2026-10-06, `unify-items`) | The game's item store is `ActionHooks::items` (lent to an economy for a call); the inventory model (`InvParts` + `InvState`) is the only inventory on the host, and the cube and the vendor's player-inventory calls answer from it (`InvDesk` host API, `InvVendors`); no rule is added | Closes `host-merge` W-5 and `wire-inventory-server` F1 / F2; `DeathDrops::items` and the cube's `Inventory` are deleted (signature changes in HANDOFF §3). Unverified until the item-move recordings R1–R6; the vendors' other inventory calls and the store inventories stay the rest's (HANDOFF §7 UI4–UI6). |
| Treasure drop on the path provider (2026-10-06, `drop-freespot`) | `DropPlacer<H>::place` receives the economy; with `ActionHooks::paths` and `PathState::field` loaded the death drop runs `floor_drop` (`path-placement.md` §9) and gives each item its path part (§2.5) so the next drop avoids it; otherwise the `FreeSpot` seam answers as before | WP2 closed. No host loads `ExpField.D2` yet, so the server keeps `FreeSpot`; DF1 (room passed to `0x0064E810`), DF2 (the path part is run from `placed`, not `create_item`) are readings (HANDOFF §7). |
| Walk / run on the server (2026-10-06, `wire-path-server`, `path-update-pass`) | C→S 0x01–0x04 run through `WorldHost::walk` only when the path provider is on (else the stub, as before); the update pass sends 0x15, 0x0F, 0x10 to the other clients from `TickHooks::send_unit_update`; the sim builds the bytes, the tests state them with `d2-proto`'s typed builders; 0x96 is never sent (no sender spec) | Unverified (no per-tick recording, `pathing.md` OQ1); WS1–WS5 and PU1–PU2 are spec readings that may change every expected walk byte (HANDOFF §7). |
| Answers the property fixes chose (2026-10-06, `prop-fixes`) | `StatLists::expire_lists` stops with `StatListError::EndlessExpiry` at an expired extended list (the state 1.14d spins in); a stale `ListId` acts as a null list; `TimerId` carries a d2rs-own generation next to the original's slot; `in_range` is the exact test abs(dx) ≤ 50 and abs(dy) ≤ 50 | d2rs's answers, not fidelity claims: the spec owner decides whether `stat-lists.md` states them (HANDOFF §7 SL1–SL4). The original's timer record reuse (LIFO free list) is unchanged. |
| Coverage grammar: table-row units (2026-10-06, `spec-unit-numbering`, coordinator decision) | A table directly under a `<!-- rows -->` line gives units `§<s> row<N>` (`§<s> t<K> row<N>` for the K-th marked table); opt-in per table, so no existing unit or claim id changes; edge-case bullet lists in specs are numbered lists | `sim/rng.md` §5.2–§7 are 39 units; the any-tier share dips where sections were claimed whole (camera 100 → 85.7%, cube 93.9 → 82.5%): the dips are real. |
| Mutation testing (2026-10-06, `mutants-*`, M08) | A surviving mutant is (a) killed by a test of the spec's outcome, (b) shown equivalent or unreachable, or (c) a spec question; no code is changed for a mutant, no `Covers:` claim for a test that checks one clause of a rule; `mutants.out` is never committed | 1,342 + 48 + 844 + 1,304 + 2,385 + 1,819 + 1,940 mutants run on `d2-sim` and `d2-proto`; missed 149 → 47, 32 → 9, 135 → 64, 389 → 34, 222 → 48, 358 → 47; no code deviation found. The tick / RNG replay killed none of the sim survivors. |
| Nightly deep property runs (2026-10-06, `ci-nightly-props`) | `.github/workflows/nightly-props.yml` (03:17 UTC and manual) runs `tools/props-deep.sh` groups `sim`, `wire`, `worldsim` at 20,000 / 20,000 / 500 cases with `PROPTEST_MAX_SHRINK_TIME=60000`; PR CI keeps `PROPTEST_CASES` unset | Answers SF1. `sim` measured 670 s; `wire` and `worldsim` are estimates; a counterexample is a real bug, never a flake (HANDOFF §8). |
| Game from a synthetic install (2026-10-06, `fixedset-game`) | `test-fixtures` builds the install, a one-room town and a `SimGame` in CI (`GameData`, `ActCreation::TownOnly`: the DRLG without the act placer, then the town); `ActCreation::Full` is the 1.14d path on real data | The TownOnly DRLG seed skips the placer's draws (a fixture state); `test-fixtures` depends on `d2-sim` and `d2-server`, still test-only. |
| MPQ Huffman decoder (2026-10-06, `mpq-huffman`, BB1) | Output and errors unchanged; the leader lookup is a 256-slot cache used only as a hint (a stale or colliding entry costs time, never changes a result) and tables 1–8 take a 1024-entry fast path until the first escape; the encoder exists under `test-support` | 2–3.7× on tables 0–3 (criterion, cloud); decided on the tables the 1.14d `.wav` files use (HANDOFF §5 C62). `specs/formats/mpq.md` unchanged. |
| Hirelings (2026-10-06, `impl-hirelings`) | `d2_sim::world::hirelings` owns the hireling (pet type 7) list beside `sim/pets.md`'s other types, behind one seam `HirelingWorld`; the interaction desk implements it (`wiring::interaction::hirelings::HireView`) and `NpcRest: HirelingRest` carries the calls no written spec provides; the NPC mercenary calls (`init_mercenary`, `revive_mercenary`, `pet(7, …)`) run the hireling rules on `InteractionState::{hirelings, hireling_tables}`. 0x7A follows the confirmed `server-messages.tsv` layout (pet @5, owner @9), against `pets.md` §8. ExpRatio (§7.2 r3) is a caller-supplied step until specified | Open: `docs/handoff/impl-hirelings.md` §4 (HL1–HL9). |

## Phases

Each phase lists its exit criteria. Check items off as they land.

### Phase 0 — Setup
- [x] Cargo workspace with all crates as empty stubs; dependency rules
      enforced (no Bevy outside `d2-client`, via `cargo run -p depcheck`)
- [x] `rust-toolchain.toml` (1.99.0); Bevy pinned `=0.19.1` in workspace
      `Cargo.toml`
- [x] CI: fmt, clippy, tests without game files, spec indexes, methods
      collection (`.github/workflows/ci.yml`, green on GitHub)
- [x] Pre-commit hook installed (`tools/hooks/install.sh`)
- [x] `game/` populated with 1.14d (`Game.exe` 1.14.3.71); game-file test
      passes. *`D2_GAME_DIR` still needs setting permanently (`setx`).*
- [x] Reference sources in `../refs/`: D2MOO, Riiablo, CE_Database,
      1.14d-notes
- [x] Ghidra project for 1.14d `Game.exe` with community labels in `re/`
      (`re/ghidra/D2_114d.gpr`; scripts and commands in `tools/ghidra/`)
- [x] Reference install record (`cargo run -p hash-manifest` →
      `traces/reference-install.toml`)
- [x] Versioned trace format defined (`traces/FORMAT.md`)
**Exit:** `cargo test` passes in CI; ignored game-file tests pass locally.

### Phase 1 — Formats (`d2-formats`)
- [x] MPQ reader (hash/block tables, compression types used by D2).
      `specs/formats/mpq.md`; all 35,364 blocks in the 1.14d archives decode.
- [x] Palettes + PL2, DC6, DCC, DT1, DS1, COF (specs in `specs/formats/`)
- [x] String tables (.tbl) and font tables (font .tbl, found by the survey)
- [x] Extraction CLI in `tools/` (`mpq-tool`: info, list, extract, check,
      formats, render)
**Exit:** every file in the target MPQs opens and decodes without error.
**Status (2026-10-05): met.** `mpq-tool check` decodes all 35,364 blocks.
`mpq-tool formats` parses every live file of every Phase 1 format (21,717
DCC, 1,657 DC6, 3,605 COF, 2,456 DS1, 254 DT1, 33 string tables, 14 font
tables, 19 palettes, 17 PL2). The only exceptions are 7 documented unused
leftovers (6 version-4 DT1s, 1 junk COF). DT1, DC6 (including flipped
frames) and DCC were also checked visually with `mpq-tool render`.
.txt/.bin tables are Phase 2. .wav decodes at the MPQ level. .bik video is
out of scope for now.

### Phase 1b — First pixels (`d2-client`, Bevy)
- [x] Bevy asset loader backed by `d2-formats` (`mpq://` asset source plus
      DS1/DT1/palette/DC6/DCC loaders, `d2-client::assets`)
- [x] Palette shader (R8Uint index texture + sRGB palette, `textureLoad`,
      exact; `d2-client::render`)
- [x] Draw a DS1 map's tiles and a static DC6/DCC sprite
      (`specs/render/map-preview.md`)
**Exit:** an Act 1 map renders correctly from the user's MPQs. Confirms the
whole pipeline before deeper work.
**Status (2026-10-05): met.** `d2-client view` shows the Rogue Encampment.
`d2-client verify` proves the GPU output byte-identical to the CPU
reference renderer (full town: 32.2M pixels, repeated runs; Lut Gholein:
37.5M pixels; off-center crops). A negative test (`--perturb N`) fails with
exactly N pixels. Known gap: 8 unflagged invisible collision tiles in
`townN1.ds1` draw as blue patches (spec open question 3, needs RE).

### Phase 2 — Data (`d2-data`)
- [x] Core: strict `.txt` reader, embedded schema (`fields.tsv`/
      `tables.tsv`), txt → record compiler (types, linkers, `strkey`,
      `calc`, `param`), `.bin` loader with the `loading.md` checks, and a
      byte-exact cross-check against 1.14d (`data-tool tables`)
- [x] Table-specific callbacks (`callbacks.md`, `d2-data::compile::callbacks`;
      confirmed by `data-tool tables` 2026-10-05: 73/73 runtime tables
      match, 72 byte-identical, 1 explained; 4/4 code buffers identical)
- [x] Post-load fix-ups and runtime maps (`fixups.md`, `runtime-maps.md`,
      `formats/animdata.md`; summary `loading.md` §7.4): specs complete,
      every rule confirmed by the 1.14d post-load dump (`dump_tables.py`,
      2026-10-06). All implemented (2026-10-06, cloud) in `d2-data::fixup`
      (`records`, `maps`, `text`, `qsort`) and `d2-formats::animdata`;
      `fixup::PENDING` empty; unit tests from the synthetic vectors pass.
      Verified 2026-10-06: `data-tool dump-compare` 70/70 tables and every
      map with a d2rs counterpart identical; `fixups_on_live_set` passes
- [x] Typed structs for all tables (generated from the schema:
      `d2-data::tables`, `data-tool gen-tables`; callback and fix-up bytes
      are read from the raw record; every live table decodes,
      `typed_tables_decode` 2026-10-06)
- [x] Cross-reference resolution with validation errors (`d2-data::links`, `data-tool links`; `field-types.md` §6.7; 1.14d live set 2026-10-06: 0 broken, 0 unchecked)
- [x] Mod patch layer format + loader (`patch-layers.md`, `d2-data::patch`,
      `data-tool patch`): G1–G8 pass on the 1.14d set (2026-10-06), and
      `data-tool patch check` on the Example stack gives the one N01 note
**Exit:** all tables load from the user's install; patches apply; broken
references are reported.
**Status (2026-10-06): met.** Every item checked on the 1.14d set:
`data-tool tables`, `data-tool links` (0 broken), `data-tool
dump-compare` (70/70 tables, every map), patch layers G1–G8.
**Status (2026-10-05):** foundation specs written in `specs/data/`
(`loading`, `txt-format`, `field-types`, `calc-expressions`,
`patch-layers`, plus `fields.tsv`/`tables.tsv`/`schema.md`).
Step 1 (d2-data core) done. `cargo run --release -p data-tool -- tables`
on the 1.14d install: the live set (73 record tables, 4 code buffers,
`hitclass`, the sound `.txt` tables) loads and passes every `loading.md`
§4.2/§8/§10.8 check. Compiling every table from its highest-priority
`.txt` in load order: 69 of 73 runtime tables byte-identical; `monstats`,
`monstats2`, `monpreset`, `cubemain` identical in every field footprint,
the rest explained by spec rules (unspecified table callbacks; `monstats`
record 707 `NameStr`); 4 of 4 code buffers byte-identical; `hitclass` and
the 12 shipped by-products byte-identical. Compiler diagnostics equal the
`txt-format.md` §9 counts. Since then (cloud, unconfirmed on game files):
the table callbacks are implemented, so the target is 73/73 with only
`NameStr` explained; the fully stated §7.4 fix-ups are applied by
`d2-data::fixup` to a copy of the loaded set.
2026-10-06: `dump_tables.py` dumps the tables and maps from 1.14d memory
after the load; `fixups.md` and `runtime-maps.md` specify every fix-up
and map, each reproduced byte for byte against that dump. Then (cloud)
all of them implemented in `d2-data::fixup`, with `AnimData.d2` in
`d2-formats::animdata`; `dump-compare` compares every map d2rs builds.

### Phase 3 — Core simulation (`d2-sim`)
- [x] D2 seeded RNG (exact sequence match) — **first**. *`d2-sim::rng` (`Seed`: step, `roll`, `mask`, `mask_range`, `roll_range`, setters, `derive`, `time_value`); unit tests from the spec vectors; `conformance` replays all 256 draws of `traces/sim/rng/*.json` exactly. Spec status `conformance-passing`.*
- [ ] Tick loop, intents in / events out, unit ordering. *Specs written
      (2026-10-06): `specs/sim/tick.md`, `sim/unit-order.md`,
      `sim/intents-events.md` (+ `client-messages.tsv`,
      `server-messages.tsv`), each confirmed on a hand-played 1.14d
      recording (`check_tick.py`: 4,902 ticks, 16,704 timer runs, 197 list
      snapshots, 0 mismatches; `check_packets.py`: rules R1–R7, 0
      failures). Status draft until implemented.* *Tick core implemented
      2026-10-06 (`d2-sim::tick`, `units::lists`, `game`): unit tests from
      every synthetic vector pass. Trace replay done 2026-10-06
      (`cargo test -p conformance --test tick_replay`): `sim-0006..0008`,
      11,105 ticks, 48,316 timer runs and 446 list snapshots equal, 0
      mismatches; `tick.md` and `unit-order.md` are `conformance-passing`.
      `d2-proto` part done:
      ids, names, size rules and layouts of both directions generated from
      the two TSVs (`data-tool gen-proto`), size lookup, classifier, S→C
      split, typed fixed layouts. `d2-server` local transport and host loop (queues, drain, gate, size
      check, point/unit parse, buffers, flush, delivery, tick driver) done
      against seams (34 unit tests). Seams wired 2026-10-06
      (`claude/phase3-wiring`): `d2-server::adapters::ProtoSizes` on
      `d2-proto`, `SimGame` on `d2-sim::game::Game` / `tick`; size vectors
      and a TSV-fake agreement check run on `d2-proto`; one single-player
      host frame runs end to end. Intent handlers: item / cube (0x2A, 0x4F), skill / combat and world ids implemented 2026-10-06, unverified (`handlers::{items, skills, world}`, only 0x49 on a real provider; `docs/handoff/server-*.md`); every other id is a stub until its system spec exists; S→C 0x15 (resync) has no layout yet.* *Eighth fold: C→S 0x01–0x04 (walk / run) run for players on the path provider, and the update pass sends 0x15 / 0x0F / 0x10 (HANDOFF §1 3ag; unverified, no per-tick recording).*
- [ ] Units, stats, stat lists, modifiers. *Implemented 2026-10-06 (`d2_sim::{units,stats}`, `docs/handoff/impl-units-stats.md`), unverified: synthetic vectors, TSV checks and perturbation tests pass; recordings queued (HANDOFF §5 A). Specs written (2026-10-06,
      branch `claude/phase3-units`): `specs/sim/units.md` (+ TSVs; per-kind
      event rules 0 errors on the three recordings, `check_units.py`),
      `sim/stats.md`, `sim/stat-lists.md` (unverified: recording queued,
      HANDOFF §5). Tick traces `traces/sim/tick/sim-0006`–`0008` committed
      (`convert_tick.py`). Status draft until implemented.*
- [ ] *Specs for the items below (2026-10-06), one writer per topic, all
      draft (rules from the 1.14d disassembly; recordings queued, HANDOFF
      §5): items, treasure, skills/combat, monsters/missiles pushed;
      world (quests, waypoints, cube, NPC, vendors), DRLG pushed. Branches
      `claude/phase3-{items,treasure,skills,drlg,monsters,world}`.*
- [ ] Items: generation, quality rolls, affixes, uniques/sets, runewords. *Implemented, unverified (`d2_sim::items`: 57 tests; item-creation recording R1 and live-table counts queued). Gap tests 2026-10-06 (`gaps-items-stats`): `affixes` 44/46, `generation` 70/72, `quality` 45/48, `properties` 36/37 rules claimed at unit tier on that branch; unique rarity read as 32 bits; `treasure.md` 77/85 (`gaps-combat-items-monsters`). Inventory and item moves implemented 2026-10-06, unverified (`d2_sim::items::{inventory, moves}`, `impl-inventory`, `impl-moves`: grids, belt, equip, the 0x16–0x29 / 0x50 / 0x61 / 0x63 handlers, deferred item messages, ground items; `inventory.md` 139/148 rule units; the two modules and the host are not connected, HANDOFF §2 step 7i; item bit stream OQ1 open).*
- [ ] Treasure classes and drops. *Implemented, unverified (`d2_sim::treasure`: 48 tests; live set vectors, memory dump and walk recording queued).*
- [ ] Skills and combat formulas. *Implemented, unverified (`d2_sim::{skills,combat}`: hit, damage, skill levels, mana, learning; `skills::use_` the use pipeline and `combat::vitals` creation, stat points, level-up, experience level factor, 2026-10-06, `docs/handoff/impl-skilluse-vitals.md`; game-file tests queued, HANDOFF §5 C1, C10). Not implemented: the per-skill bodies (seam `SkillFunctions`, `use.md` OQ10) and the kill experience gain beyond `kill_experience` in `wiring::interaction` (§4.2, §4.3's add; nothing calls it). Skill / combat handlers 0x05–0x11, 0x3A, 0x3B, 0x3C and the use / vitals seams wired 2026-10-06, unverified (`server-skills`, `wire-interaction`); gap tests for `combat/damage.md` (115/126) in `gaps-combat-ai`, `combat/hit.md` (46/49) and `skills/levels.md` (43/47) in `gaps-combat-items-monsters`.*
- [ ] Level generation (DRLG). *Levels and rooms implemented, unverified (`d2_sim::drlg`); level types implemented 2026-10-06, unverified (`drlg::preset`, `drlg::maze`, `drlg::outdoor`: Act I and tilesub in full, Acts II and IV as specified, Act III partial, Act V siege strip only; notes `impl-drlg-{preset,maze,outdoor}.md`). Level-type dispatcher, `DrlgError::LevelType` and the maze / outdoor preset providers wired 2026-10-06, unverified (`wiring::worldgen`, `wire-worldgen`); DS1 / DT1 / lvlsub providers from the user's files written 2026-10-06, unverified (`d2_server::world_data`, `drlg-data`; live check HANDOFF §5 C23; the RNG trace compare stays blocked, §5 Blocked); `wiring::worldgen` lends the world state into the action hooks so monster init, umod, free and monster-data routes run (`wire-routing`, unverified). Gap tests `gaps-drlg-world`, `gaps-drlg-sim-world`: `levels` 40/48, `rooms` 80/91, `outdoor` 65/72, `preset` 46/60.*
- [ ] Monsters: spawning, AI, missiles. *Missiles (creation, flight, hit, pierce) and the AI core with 17 AI functions implemented, unverified; monster population (`monsters::population`) and init (`monsters::init`; boss mods §14.2 bloodraven only) implemented 2026-10-06, unverified (no spawn RNG trace; HANDOFF §5 A14). Server-do / server-hit bodies and the other 131 AI functions not implemented. Population and init wired to real rooms and regions 2026-10-06, unverified (`wiring::worldgen`); with the default `WorldPending` no room population runs, only presets place monsters (DRLG data population reads unspecified). Gap tests (`gaps-combat-ai`, `gaps-combat-items-monsters`): `ai.md` 108/119, `missiles.md` 104/117, `population.md` 158/165; message 0x73 not implemented. Spec-body audit 2026-10-06 (`spec-bodies-ai-missiles`): no further AI / server-do / server-hit body is implementable exactly; the missing text per entry is HANDOFF §2 step 12.*
- [ ] Quests, waypoints, NPCs, vendors, Horadric Cube. *Quest control and Act I, waypoints, cube implemented, unverified (`d2_sim::world`); NPC interaction (`world::npc`) and vendors (`world::vendors`) implemented 2026-10-06, unverified (no hire / resurrect / heal / services recording; HANDOFF §5 A16); Act II–V quests not implemented. Hirelings (`world::hirelings`: rows, offer, init / replace, level stats, experience, death, revive, follow, classic act change, restore steps, item swap) implemented 2026-10-06, unverified (`impl-hirelings`; wired into hire / resurrect / quest mercenary on the desk; death, teleport follow, kill share, 0x61 swap and the save restore have no caller yet). NPC / vendor / quest seams wired on real units, stat lists and store 2026-10-06, unverified (`wiring::interaction`); in `d2-server` only 0x49 (waypoints) has a real provider, the NPC, vendor and quest handlers are tested on a seam fake. Quest ids 0x31 / 0x40 / 0x58 run the real `QuestControl` on `TradeWorld` and the Kashya mercenary reward on the NPC control 2026-10-06, unverified (`quest-host`, tested through the host frame on synthetic tables; the single-player e2e runs the NPC, vendor and cube steps, `e2e-next`). Gap tests `gaps-drlg-world`: `quests` 72/81, `waypoints` 47/61, `cube` 46/49. Audit of the unhandled quest callbacks 2026-10-06 (`spec-bodies-quests-skills`): none stated completely; Act I, other-hook and Acts II–V gaps are listed in HANDOFF §2 step 12. Act I remainders 2026-10-06, unverified (`impl-quests-act1-rest`: `quests-act1-rest.md` §1–§8: gibbet operate / event 7, Cairn stone init and Tristram-portal timer, town-Cain marker, Countess chest trap, character progression, Catacombs states kept, Kashya's 0x50 before the refresh, the bucket-order player walk; eight new quest seams have no host provider yet).*
- [ ] Seam providers and wiring: `UnitSystem` as the sim's `EventDispatch`, providers of every system trait, `d2-server` intent handlers (item / cube, skill and world ids done, the rest stubs, HANDOFF §1 3k). Second pass done 2026-10-06, unverified: `wiring::worldgen` (`WorldSim`, level types, population, init) and `wiring::interaction` (NPC, vendors, skill use, vitals); the four wiring modules are still not combined into one host and the seams listed in `wire-worldgen.md` §5 / `wire-interaction.md` §6 stay `Pending`. First pass done 2026-10-06, unverified: `d2_sim::wiring::economy` (items, treasure, cube and quest item seams) and `wiring::action` (`ActionSim`: events, combat, missiles, AI, DRLG rooms, waypoints; every seam without a provider is `Pending`); the two are separate modules, providers for path / position, player data, inventory, objects and the seams named in the notes remain (HANDOFF §2 steps 1–3, 7b). Sixth fold 2026-10-06, unverified: free-point searches, unit placement and warps (`d2_sim::path`, `impl-path-place`: `path-placement.md` §7–§12; the core §1–§6 and `pathing.md` have no code, HANDOFF §2 step 7j); wiring seams closed (`wire-open-seams`: skill timer events 5 / 8 / 9, event 7, removal, tick step 8 quests, Kashya's reward); the combat path (`e2e-combat-path`: anim record routing, reaction / kill, death drops) and the vendor world host (`TradeWorld`, `e2e-vendor-host`); `d2_proto::s2c` typed builders for 16 S→C ids and the `bits:` layout (`s2c-builders`, `proto-bits`). Seventh fold 2026-10-06, unverified: **one host** (`host-merge`: `SimGame` has one `world: W`, `WiredWorld` replaces `TradeWorld`, one unit store and one item store, `SkillSeams` deleted, game-creation fields in the action wiring); the inventory wired (`wire-inventory-sim`: `wiring::inventory::InvDesk`) and served by `d2-server` (`wire-inventory-server`: C→S 0x16–0x29, 0x50, 0x61, 0x63 and the deferred item messages after the tick); the path core, the walk and their opt-in provider (`impl-path-core`, `impl-walk`, `wire-path-sim`: `path-placement.md` §1–§6, `pathing.md` §1–§10, `wiring::path`); open: the server walk / run handler, missile flight on the provider, the treasure FreeSpot seam, the drop's item store, three inventory copies (HANDOFF §2 steps 7o, 7p). **Eighth fold 2026-10-06, unverified:** the server walk / run handler and the update-pass callers (`wire-path-server`, `path-update-pass`), the treasure drop's floor drop on the path provider (`drop-freespot`, when the field is loaded), one item store and one inventory (`unify-items`); still open: missile flight on the provider (WP1), the teleport footprint room (WP3), the host loading `ExpField.D2`, the vendors' other inventory calls (HANDOFF §2 steps 7o, 7p).
**Exit per item:** spec status `conformance-passing`.

### Phase 4 — Conformance (runs alongside Phase 3)
- [ ] Trace recorder for the original game. *Feasibility proven 2026-10-05: `tools/trace-recorder` records every RNG draw of 1.14d (Python debugger); traces in `traces/sim/rng/`. Other event types not yet.*
- [ ] Replay harness. *Per-behavior replayers in `crates/conformance`: RNG draws (`conformance::rng`) and the tick (`conformance::tick`: timer runs and unit lists, perturbation-tested). Harnesses for recordings not yet made (2026-10-06, `conformance-harness`): `units::replay_anim` (`tick-raw-1`), `stats::replay_stats` (`stats-raw-1`), `packets::replay_packets` (`packets-raw-1`, through `d2_server::dispatch`), `rooms::replay_rooms`; fixtures pass the Python checkers, the recording tests are `#[ignore]`d and none has run on 1.14d (HANDOFF §5 A, C39). Frame-capture verify case kind `scene` (`render-capture`, `d2-client verify --cases crates/d2-client/capture-cases`): stability check complete, compare needs a `SceneSource`. Seventh fold: movement and placement replay harnesses (`conformance-path-render`, `conformance::{movement, placement, needs}`, `recordings-needed`), proven on synthetic recordings only; no d2-sim provider behind either seam and no recording holds per-tick path state or placement calls.*
- [ ] Coverage report (the "99.x%" number). *Tool and claim scheme done 2026-10-06 (`docs/COVERAGE.md`, `py tools/coverage.py`, `--check` in CI); claims seeded in d2-sim rng, d2-data, d2-formats. At `edd9925` (`py tools/coverage.py --summary`, sixth fold): 3,542 claims, 3,106 rule units, unit tier 2,612 (84.1%), game-file 189 (6.1%), trace 30 (1.0%), verified 219 (7.1%), any tier 2,660 (85.6%); the share fell because 402 rule units of new specs (`inventory`, `pathing`, `path-placement`, `composition`, `camera`, `capture`, `sprite-placement`) entered the denominator (at `4b5b0bf`: 3,259 claims, 2,704 rule units, unit tier 2,404 (88.9%), game-file 187 (6.9%), trace 30 (1.1%), verified 217 (8.0%), any tier 2,459 (90.9%) (at `fd37fba`: 3,164 claims, unit 2,342, verified 216, any 2,396; before the gap tests: 2,386 claims, verified 188, any 1,803; before `coverage-claims`: 386 claims, verified 146, any 274). The 28 game-tier claims of `gaps-data-formats` and the one of `gaps-client-formats` sit on `#[ignore]` tests that have never run, so 219 is an upper bound until the local run (HANDOFF §5 C17, C20, C38); the 67 game-file tests of the sixth fold (HANDOFF §1 3w) carry no claim until they pass. The unit tier is claims by synthetic tests; only game-file and trace tiers verify. The claim grammar accepts `r0` (`gaps-data-rng`). Open: numbered items in specs whose sections are one unit (so claims can be narrowed; `sim/rng.md` §5.2–§7 first, HANDOFF §7 GD11), and the units no claim names (HANDOFF §2 step 6).* *Eighth fold (`py tools/coverage.py --summary`, merged tree): 4,194 claims, 3,339 rule units, unit tier 2,858 (85.6%), game-file 197 (5.9%), trace 30 (0.9%), verified 227 (6.8%), any tier 2,911 (87.2%); `--check` passes. `spec-unit-numbering` numbered the edge-case lists of 15 specs and added table-row units (`sim/rng.md` §5.2–§7: 39 units, 1 row pair claimed); the mutation-test sessions added claims only where a test checks a whole rule; `ui/text.md` (39), `render/draw-order.md` (23) and `render/unit-composite.md` (41) entered the denominator with no claim until their implementation sessions land. Open: the sections still one unit (`data/field-types.md` §6, `drlg/rooms.md` §9.9, `missiles.md` §r11, `drlg/maze.md` §3), HANDOFF §2 step 7u(e).*

### Phase 5 — Local server + bridge
- [ ] `d2-proto` message types (versioned). *1.14d message tables and typed fixed layouts done (Phase 3, `PROTOCOL_VERSION` 1); the bridge carries these 1.14d messages unchanged and needs no d2rs-own message so far (`specs/client/bridge.md` §1 rule 4).*
- [ ] In-process server running `d2-sim`. *Eighth fold: one item store and one inventory per game (`unify-items`), walk / run handlers (`wire-path-server`) and the treasure drop's free spot on the path provider (`drop-freespot`); the item moves run through the host, the kill's gold can be picked up, sold and cubed in the e2e (HANDOFF §1 3af, 3ag). Host loop runs on the real adapters (one single-player frame end to end); item / cube, skill / combat and world intent handlers exist (only 0x49 on a real provider), every other handler is a stub until its spec exists and the providers are wired. Sixth fold: `TradeWorld` also runs the NPC, vendor and quest ids on the wired host, and `SimGame::tick` now runs the dispatch's `TickHooks`; `items::moves` (the item ids) is not connected to any handler (HANDOFF §2 step 7i). `d2_proto::s2c` builders and parser exist (HANDOFF §1 3t); `fuzz-server` properties cover the transport, dispatcher, handlers and bridge (one bug fixed).*
- [ ] `d2-client::bridge`: S→C messages → client world model → Bevy
      mirror entities, input → intents. *Design `specs/client/bridge.md`
      and skeleton done 2026-10-06 (branch `claude/phase5-bridge`):
      receive split + dispatch by id (all ids unowned until client-model
      specs exist), intent send path, `ClientWorld`, `BridgePlugin`
      mirror; synthetic vectors pass. `ServerLink` adapter over the
      `d2-server` host written 2026-10-06 (`bridge::local::LocalLink`,
      `claude/p5-local-server`): a headless end-to-end test sends C→S 0x49
      and receives the exact S→C bytes into `ClientWorld` on synthetic data;
      unverified against the original. The app now builds
      and runs a single-player game on a server thread (`claude/p6-window`,
      2026-10-06; the game is not `Send`, so it never leaves its thread;
      synthetic DRLG, black window by design). Still open: session code is a
      placeholder (HANDOFF §2 step 4). No tick
      interpolation (decisions log).*
**Exit:** walk around Act 1 town via the local server.

### Phase 6 — Full client
- [ ] Animation (COF/DCC), lighting, blend modes, draw ordering. *Infrastructure only (C3 frames, C4 scene + CPU compositor, C5 GPU compute compositor, C6 verify harness with the GPU half wired, C7 COF composite mechanics; GPU exactness proven on llvmpipe only, real-GPU run queued, HANDOFF §5 C15, C16); in-app render-graph node `compose_node` and frame store done 2026-10-06 (`p6-window`, `verify-map`; llvmpipe only, real-GPU run queued, HANDOFF §5 C24–C31), the `map` verify case ported to the compositor; d2rs-own sound pool, prefetch queue and text layout have code (`client-own-gaps`); original-behavior RE specs not written, so the window stays black. Sixth fold: `render/composition.md` §2–§6 implemented in both compositors (framebuffer, frame cycle, `table[dest][src]` pixel write; unverified, no capture), the `scene` capture verify case (`render-capture`), the world view wired to the frame store, UI text layout, sound pool and the user's levels (`p6-integrate`, `p6-world-view`; HANDOFF §1 row 6, checks C40–C44). Seventh fold: the camera and sprite-placement rules (`d2_client::rules`: `camera.md` §1–§10, `sprite-placement.md` §1–§8) are implemented and wired into the app's world view through a `ViewFeed`, and the capture's `scene` case checks the recorded camera values before any pixel (`render-camera-placement`, `render-wire`; unverified, no capture of a world; the window stays black until the feed hooks and the recorder gap are filled, HANDOFF §2 step 7q); criterion baselines for the sim, formats, proto and compositor (`bench-baselines`). Eighth fold: the specs `ui/text.md`, `render/draw-order.md`, `render/unit-composite.md` (+ `unit-directions.tsv`) and the render follow-ups (`camera`, `capture`, `composition`, `sprite-placement`: RC1–RC6, RW1, RW5, RN1–RN3, CP1–CP4; recorder `frames-raw-2`) landed from the local session, and the implementation sessions `impl-ui-text`, `impl-draw-order`, `impl-unit-composite` and `render-followups-impl` are running (their notes are the ninth fold's input); `mpq::writer` and the Huffman decoder speed-up (`mpq-huffman`).*
- [ ] UI panels, inventory, fonts, audio, controls config. *Cores only (C8 UI, C9 controls file, C10 audio mixer / voice log); no real panel, font, sound table or WAV parser. Eighth fold: `specs/ui/text.md` (+ `text-fonts.tsv`) is written (draft, answers CG2); `impl-ui-text` is implementing it.*
**Exit:** play through all acts locally with correct visuals.

#### Phase 6 work breakdown (2026-10-06, design drafts)

Design drafts (d2rs-own, part (a) binds code, part (b) lists unwritten
owner specs): `specs/client/render-pipeline.md`, `client/assets.md`,
`client/ui.md`, `client/audio.md`. Cloud implements infrastructure now
(no original behavior needed; each proven by synthetic vectors, GPU
halves queued locally); everything that reproduces the original waits
for the local RE spec named.

**Cloud, ready now (pure infrastructure).** Independent unless noted;
none touches `d2-client::bridge` (`claude/phase5-bridge`).

| # | Task | Spec | Proof in cloud | Status (2026-10-06) |
|---|---|---|---|---|
| C1 | Canonical lowercase `mpq://` paths; loaders for `pl2`, `cof`, `tbl` (font vs strings by magic) | `assets.md` §A1–A2 | unit tests §Test vectors | done (`claude/p6-assets`); game-file check queued |
| C2 | Residency cache core: byte budgets, deterministic LRU, never evict the current frame, stall metric (plain Rust) | `assets.md` §A4–A5 | unit tests | done (`claude/p6-assets`); prefetch waits for the bridge hook |
| C3 | `IndexFrame` + per-direction `FrameSet` from DCC/DC6/DT1; deterministic shelf atlas packer (plain Rust) + R8Uint page upload | `render-pipeline.md` §A2, `assets.md` §A3 | unit tests (packing, determinism) | done (`claude/p6-frames`); game-file check queued |
| C4 | `d2-client::scene`: `DrawItem`, `DrawKey` stable sort, shade chain, `BlendOp::{Opaque, IndexTable}`, bins, CPU reference compositor | `render-pipeline.md` §A3–A8 | §Test vectors (CPU, CI) | done (`claude/p6-scene`) |
| C5 | GPU compute compositor matching C4 (after C4) | §A9 | CPU half in CI; GPU byte-exact queued locally | done (`claude/p6-gpu`): `gpu_compositor`, `gpu_compare` example; 12 cases 0 differing on llvmpipe, `--perturb 7` exactly 7; real-GPU run queued (HANDOFF §5 C15) |
| C6 | Verify harness: case files (`version = 1`), runner, per-case `--perturb`; port today's map verify as case `map`; `synthetic` cases (after C4) | §A10 | CPU half in CI; GPU queued | done (`claude/p6-verify`), GPU half wired with C5 and `[[unit]]` COF cases (`claude/p6-verify-gpu`): 10 synthetic cases CPU vs GPU 0 differing on llvmpipe; queued with a real GPU (HANDOFF §5 C16) |
| C7 | COF composite mechanics: slot order → per-component items, path and placement behind `TODO(spec)` hooks | §A7 | synthetic COF vectors | done (`claude/p6-cof`): `composite`; live-COF check queued (HANDOFF §5 C9) |
| C8 | UI core: `Panel`/`UiRoot`, widgets, integer hit tests, event routing, frame-coordinate mapping | `ui.md` §A2, §A4 | §Test vectors | done (`claude/p6-ui`) |
| C9 | Controls file `d2controls 1`: strict parser, writer, presets (`dev` only), clash check, migration hook | `ui.md` §A6 | §Test vectors | done (`claude/p6-controls`); `original` preset waits for `ui/controls.md` |
| C10 | Audio core: trigger queue, tick scheduler, integer mixer, voice log (`d2rs-audio-log 1`), rodio `Decodable` output | `audio.md` §A2–A5 | §Test vectors, golden hashes | done (`claude/p6-audio`); not wired in `app.rs` |

**Waits for local RE specs** (spec session, high effort; order is the
critical path to a first playable scene):

1. `render/sprite-placement.md`, `render/camera.md`, `render/composition.md`
   (+ frame capture in `tools/trace-recorder`, render §B9): unlocks link 1
   (original → CPU reference) at all.
2. `render/unit-composite.md`, `render/draw-order.md`: units in the town. **Written (eighth fold, draft, no claim yet); implementation sessions `impl-unit-composite` and `impl-draw-order` are running.**
3. `render/shading.md`, `render/blend-modes.md`, `render/lighting.md`.
4. `formats/wav.md`, `audio/triggers.md`, `audio/sound-table.md`
   (+ `record_sound.py`), then `audio/environment.md`.
5. `ui/text.md` (**written, eighth fold; `impl-ui-text` running**), `ui/panels.md`, `ui/controls.md`, `ui/inventory.md`,
   `ui/automap.md`.

> **Current scope ends at Phase 6** (decided 2026-10-05). Phases 7–9 are
> deferred and not yet planned in detail. The mod is a separate future
> project. Keep the decisions that keep them possible (local server,
> versioned formats, `Ruleset`), but spend no effort on them now.

### Phase 7 — Online (deferred)
- [ ] Network transport, remote server
- [ ] Accounts (argon2, TLS), character storage (versioned format)
- [ ] Server-side validation of all actions

### Phase 8 — Ownership gate (`d2-verify`) (deferred)
- [ ] "Has the game" check at launch (required MPQs present, valid, contain
      expected files), with clear error messages
- [ ] Server rejects clients that fail verification

### Phase 9 — The mod (deferred; separate project)
- [ ] Mod patch layers and assets
- [ ] Mod systems behind `Ruleset::Mod`
- [ ] Release packaging verified to contain **no Blizzard files**

## Working with Claude Code

- *Spec session:* "Read `re/exports/<area>` and `../refs/D2MOO/source/<area>`,
  then write `specs/<area>/<behavior>.md` from `specs/_TEMPLATE.md`. Confirm
  against 1.14d and note differences from D2MOO. No code."
- *Implementation session (fresh):* "Implement `specs/<area>/<behavior>.md`
  in `crates/d2-sim`. Do not read `re/`. Add tests from the test vectors."
- *Conformance session:* "Add traces from `traces/<area>` and fix failures
  against the spec."
- *Bevy upgrade session (between milestones only):* "Upgrade Bevy to <ver> in
  `d2-client` only, following the official migration guide."

Where each kind runs (cloud vs local) is in `CLAUDE.md` "Where work runs".
Commit after every session. Update this checklist at the end of each one.

## Risks

| Risk | Mitigation |
|---|---|
| Scope | Phase order; conformance coverage as the progress metric |
| Bevy breaking changes | Pinned version; thin bridge; upgrades as separate tasks |
| Non-determinism | Integer math, ordered collections, RNG first, no Bevy in sim |
| Late networking retrofit | Local-server architecture from Phase 5 |
| Version mismatch with D2MOO (1.10f vs our 1.14d) | Target locked; specs record 1.14d confirmation and D2MOO differences; traces from 1.14d decide |
| Clean-room contamination | Session separation; specs as written record |
| GPL contamination | OpenDiablo2/OpenD2 read-only |
| Blizzard files leaking into git/release | Pre-commit hook; release check |
| Piracy bypass | Account system + server-side verification |
| Legal uncertainty | Not legal advice; consider a lawyer before public release |
