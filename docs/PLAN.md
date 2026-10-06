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
| Level types and monster / NPC / vendor modules (2026-10-06, `impl-drlg-*`, `impl-monster-*`, `impl-npc`, `impl-vendors`, `impl-skilluse-vitals`) | Same rules as the other Phase 3 systems: narrow seams with fakes, tables as typed records or embedded TSVs (`preset-tables.tsv`, `maze-specials.tsv`, `umods.tsv`, `preset-monsters.tsv`, `vendors.tsv`, `functions.tsv` parsed strictly or checked row by row with perturbation tests), narrowest reading with a `TODO(<spec ref>)`, unverified until a trace or live-table check runs | `d2-sim` still has no `d2-formats` dependency (DS1 arrives as `Ds1Input` through `Ds1Source`; HANDOFF §7 I1 for the live-data test home). Original fatals return errors (`PriceFatal`, `PresetError`, `TreasureError`) for the caller or `Ruleset` to decide. Wall-clock time is an input (`now`) to the vendors' store refresh. |
| GPU compositor (2026-10-06, `p6-gpu`, `p6-verify-gpu`) | Compute shader in WGSL, integer math only, no sampler or blending; 16×16 workgroups; output a u32-per-pixel storage buffer (RGBA pass a bit copy of the palette bytes); atlas an R8Uint 2048² texture array; verify compares both the index framebuffer (bytes) and RGBA (pixels), runs each job's GPU half twice and requires equal readbacks; exit code 2 for `GPU NOT WIRED` and `NO ADAPTER` (never a pass) | `wgpu = "29.0.3"` added to `d2-client` with Bevy's own backend features (M10). Cloud check on Mesa llvmpipe only: a software rasterizer says nothing about a real driver's integer and texture paths, so GPU exactness stays unverified until the local run. Case format v1 gained `[[unit]]` without a version bump. |
| Coverage claims (2026-10-06, `coverage-claims`) | A test claims a rule only when its assertions check that rule's outcome, with the narrowest ID that is fully true; no claim for code-vs-TSV consistency checks, M08 perturbation tests or d2rs-only errors; in `conformance` only trace replays are claimed | 10 partial claims were dropped on review. Open policy point: rules that are one unit (prose-only sections, tables, indented sub-lists) can be claimed by a test that checks the main behavior only; the fix is numbered items in those specs. Claims went from 386 to 1,520 on that branch. |

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
      host frame runs end to end. Intent handlers are stubs until their
      system specs exist; S→C 0x15 (resync) has no layout yet.*
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
- [ ] Items: generation, quality rolls, affixes, uniques/sets, runewords. *Implemented, unverified (`d2_sim::items`: 57 tests; item-creation recording R1 and live-table counts queued).*
- [ ] Treasure classes and drops. *Implemented, unverified (`d2_sim::treasure`: 48 tests; live set vectors, memory dump and walk recording queued).*
- [ ] Skills and combat formulas. *Implemented, unverified (`d2_sim::{skills,combat}`: hit, damage, skill levels, mana, learning; `skills::use_` the use pipeline and `combat::vitals` creation, stat points, level-up, experience level factor, 2026-10-06, `docs/handoff/impl-skilluse-vitals.md`; game-file tests queued, HANDOFF §5 C1, C10). Not implemented: the per-skill bodies (seam `SkillFunctions`, `use.md` OQ10) and the kill experience gain.*
- [ ] Level generation (DRLG). *Levels and rooms implemented, unverified (`d2_sim::drlg`); level types implemented 2026-10-06, unverified (`drlg::preset`, `drlg::maze`, `drlg::outdoor`: Act I and tilesub in full, Acts II and IV as specified, Act III partial, Act V siege strip only; notes `impl-drlg-{preset,maze,outdoor}.md`). No level-type dispatcher, no `DrlgError` variants for the three and no DS1 / DT1 providers yet, so no level from the live tables (HANDOFF §2 step 7, §5 Blocked).*
- [ ] Monsters: spawning, AI, missiles. *Missiles (creation, flight, hit, pierce) and the AI core with 17 AI functions implemented, unverified; monster population (`monsters::population`) and init (`monsters::init`; boss mods §14.2 bloodraven only) implemented 2026-10-06, unverified (no spawn RNG trace; HANDOFF §5 A14). Server-do / server-hit bodies and the other 131 AI functions not implemented.*
- [ ] Quests, waypoints, NPCs, vendors, Horadric Cube. *Quest control and Act I, waypoints, cube implemented, unverified (`d2_sim::world`); NPC interaction (`world::npc`) and vendors (`world::vendors`) implemented 2026-10-06, unverified (no hire / resurrect / heal / services recording; HANDOFF §5 A16); Act II–V quests not implemented.*
- [ ] Seam providers and wiring: `UnitSystem` as the sim's `EventDispatch`, providers of every system trait, `d2-server` intent handlers (still stubs). First pass done 2026-10-06, unverified: `d2_sim::wiring::economy` (items, treasure, cube and quest item seams) and `wiring::action` (`ActionSim`: events, combat, missiles, AI, DRLG rooms, waypoints; every seam without a provider is `Pending`); the two are separate modules, providers for path / position, player data, inventory, monster data, objects and the new NPC / vendor / skill-use / vitals / population / init seams remain (HANDOFF §2 steps 1–3, 7b).
**Exit per item:** spec status `conformance-passing`.

### Phase 4 — Conformance (runs alongside Phase 3)
- [ ] Trace recorder for the original game. *Feasibility proven 2026-10-05: `tools/trace-recorder` records every RNG draw of 1.14d (Python debugger); traces in `traces/sim/rng/`. Other event types not yet.*
- [ ] Replay harness. *Per-behavior replayers in `crates/conformance`: RNG draws (`conformance::rng`) and the tick (`conformance::tick`: timer runs and unit lists, perturbation-tested).*
- [ ] Coverage report (the "99.x%" number). *Tool and claim scheme done 2026-10-06 (`docs/COVERAGE.md`, `py tools/coverage.py`, `--check` in CI); claims seeded in d2-sim rng, d2-data, d2-formats. At this commit: 2,386 claims, 2,704 rule units, unit tier 1,771 (65.5%), game-file 158 (5.8%), trace 30 (1.1%), verified 188 (7.0%), any tier 1,803 (66.7%) (before `coverage-claims`: 386 claims, verified 146, any 274). The unit tier is claims by synthetic tests; only game-file and trace tiers verify. Open: numbered items in specs whose sections are one unit (so claims can be narrowed), the claim grammar for `r0`, and the units no claim names (HANDOFF §2 step 6).*

### Phase 5 — Local server + bridge
- [ ] `d2-proto` message types (versioned). *1.14d message tables and typed fixed layouts done (Phase 3, `PROTOCOL_VERSION` 1); the bridge carries these 1.14d messages unchanged and needs no d2rs-own message so far (`specs/client/bridge.md` §1 rule 4).*
- [ ] In-process server running `d2-sim`. *Host loop runs on the real adapters (one single-player frame end to end); intent handlers are stubs until the system providers are wired.*
- [ ] `d2-client::bridge`: S→C messages → client world model → Bevy
      mirror entities, input → intents. *Design `specs/client/bridge.md`
      and skeleton done 2026-10-06 (branch `claude/phase5-bridge`):
      receive split + dispatch by id (all ids unowned until client-model
      specs exist), intent send path, `ClientWorld`, `BridgePlugin`
      mirror; synthetic vectors pass. `ServerLink` adapter over the
      `d2-server` host not written yet (the `d2-server` seams are wired, host
      constructor and `Send + Sync` still needed, HANDOFF §2 step 4). No tick
      interpolation (decisions log).*
**Exit:** walk around Act 1 town via the local server.

### Phase 6 — Full client
- [ ] Animation (COF/DCC), lighting, blend modes, draw ordering. *Infrastructure only (C3 frames, C4 scene + CPU compositor, C5 GPU compute compositor, C6 verify harness with the GPU half wired, C7 COF composite mechanics; GPU exactness proven on llvmpipe only, real-GPU run queued, HANDOFF §5 C15, C16); no in-app render-graph node; original-behavior RE specs not written.*
- [ ] UI panels, inventory, fonts, audio, controls config. *Cores only (C8 UI, C9 controls file, C10 audio mixer / voice log); no real panel, font, sound table or WAV parser.*
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
2. `render/unit-composite.md`, `render/draw-order.md`: units in the town.
3. `render/shading.md`, `render/blend-modes.md`, `render/lighting.md`.
4. `formats/wav.md`, `audio/triggers.md`, `audio/sound-table.md`
   (+ `record_sound.py`), then `audio/environment.md`.
5. `ui/text.md`, `ui/panels.md`, `ui/controls.md`, `ui/inventory.md`,
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
