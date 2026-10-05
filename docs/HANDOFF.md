# Handoff (updated 2026-10-06, branch `claude/phase3-specs`)

Start here in a fresh session, after `CLAUDE.md`. This file holds state,
the next steps, the code and command map, and the local run queue. Rules
and facts live in specs (`specs/README.md`); this file points to them
rather than restating them.

## 1. State

| Phase | Status | Proof |
|---|---|---|
| 0 Setup | done | CI green on GitHub (`MoggerCat/MXL-ULTIMATE`) |
| 1 Formats | done | `mpq-tool check`, `mpq-tool formats` |
| 1b First pixels | done | `d2-client verify` (GPU = CPU reference, byte-exact) |
| 2 Data | in progress: core, callbacks, first fix-ups, typed tables, cross-reference validation (`data-tool links`: 0 broken); fix-up specs complete and dump-confirmed | `data-tool tables` (2026-10-05, with callbacks): 73 live tables, 72 byte-identical, 1 explained (`monstats` `NameStr`); 4/4 code buffers identical; `d2-data` game-file tests all pass (including `typed_tables_decode`); `data-tool dump-compare`: 57/70 tables identical to 1.14d memory (rest: unimplemented fix-ups, §2) |
| 3 Simulation | in progress: RNG done; tick, unit-ordering and intents/events specs written and confirmed on recordings (not implemented) | `cargo test -p d2-sim -p conformance`: spec vectors pass; all 256 draws of `traces/sim/rng/*.json` replay exactly. `check_tick.py` / `check_packets.py` on the 2026-10-06 recordings: 0 mismatches |
| 4 Conformance | recording proven feasible | `tools/trace-recorder`: 32,543 recorded RNG draws match the spec exactly |
| 5–6 | not started | |
| 7–9 | deferred (out of current scope) | |

## 2. Next steps (in order)

1. **Implement the fix-ups** (implementation, medium; cloud or local):
   `specs/data/fixups.md` and `specs/data/runtime-maps.md` in
   `d2-data::fixup`: every `PENDING` row, the fix-ups the dump found
   (lvltypes/lvlprest/lvlsub tile paths, objects names and frame counts,
   monumod clamp), the monequip correction (`fixups.md` §9: empty item
   code exempt, rows without a valid monster untouched), the runtime maps
   (`runtime-maps.md`, with the CRT `qsort` of §1), and `AnimData.d2`
   (`formats/animdata.md`) for the monstats speeds. Unit tests from the
   synthetic vectors; `dump-compare` compares the maps too (add their
   d2rs counterparts). Proof: local queue entry (§5).
2. **Patch layers** (implementation, medium): `specs/data/patch-layers.md`
   (in progress on `claude/patch-layers`).
   Phase 2 exit = "patches apply"; the mod itself is deferred.
3. **Phase 3 specs, part 2** (spec writing / RE, local, high): units,
   stats, stat lists and modifiers (`docs/PLAN.md` Phase 3). Built on
   `specs/sim/tick.md` (timer events: what each event type does per unit
   kind is the open part, `tick.md` §5.6–§5.7 and open question 3) and
   `sim/unit-order.md`. Record with `record_tick.py` (it already logs
   every timer run with unit and arguments); extend it per M10 for stats.
4. **Implement the tick core** (implementation, medium; cloud or local,
   after or alongside step 3): `d2-sim::tick` timer queue and step order
   (`tick.md`), `d2-sim::units::lists` (`unit-order.md`), `d2-proto`
   ids and sizes from the two TSVs (`intents-events.md`). Unit tests from
   the specs' synthetic vectors; the recordings become format-1 traces
   (`tick-0001`, `packets-0001`) once a converter and enough of `d2-sim`
   exist.

## 3. Code map

| Path | What | Spec |
|---|---|---|
| `crates/d2-formats/src/mpq/` | MPQ archive (`mod.rs`), hash/block tables (`tables.rs`), crypto, PKWARE explode, Huffman, ADPCM, `ArchiveSet` (`set.rs`) | `formats/mpq.md`, `mpq-tables.md`, `data/loading.md` §2 |
| `crates/d2-formats/src/{palette,dc6,dcc,dt1,ds1,cof,tbl,font}.rs` | file formats | `specs/formats/*.md` |
| `crates/d2-data/src/txt.rs` | strict `.txt` reader, column binding | `data/txt-format.md` |
| `crates/d2-data/src/schema.rs` | embedded `fields.tsv` / `tables.tsv` | `data/schema.md` |
| `crates/d2-data/src/compile.rs` (+ `compile/tests.rs`) | cell → bytes, linkers, `strkey`/`calc`/`param` | `data/field-types.md` |
| `crates/d2-data/src/compile/callbacks.rs` (+ `callbacks/tests.rs`) | cube, skill-mode, composit, place callbacks; `@uniques`/`@sets` | `data/callbacks.md` |
| `crates/d2-data/src/codegen.rs`, `tables/` | typed record structs: generator, generated `tables/generated.rs` (don't edit; `data-tool gen-tables`), `Record`, `decode_all` | `data/schema.md` |
| `crates/d2-data/src/links.rs` (+ `links/tests.rs`) | cross-reference validation: linker sizes, broken links by table/row/column | `data/field-types.md` §6.7 |
| `crates/d2-data/src/fixup.rs` | post-load fix-ups and runtime maps (on a copy of the loaded set) | `data/fixups.md`, `data/runtime-maps.md` (summary `loading.md` §7.4) |
| `crates/d2-data/src/calc.rs` (+ `calc/tests.rs`) | formula compiler, code buffers | `data/calc-expressions.md` |
| `crates/d2-data/src/compile_set.rs` | all tables in load order | `data/loading.md` §6–7 |
| `crates/d2-data/src/bin.rs` | `.bin` container, live-file resolution, load checks | `data/loading.md` §3–4, §8 |
| `crates/d2-data/src/crosscheck.rs` | txt → bin byte comparison | `data/loading.md` §11 |
| `crates/d2-data/src/strings.rs` | string tables, `strkey` | `field-types.md` §7 |
| `crates/d2-client/src/map/` | DS1+DT1 map assembly, CPU reference renderer | `render/map-preview.md` |
| `crates/d2-client/src/{app,assets,render}` | Bevy app, `mpq://` assets, palette shader | `render/map-preview.md` |
| `crates/d2-sim/src/rng.rs` | seeded RNG: `Seed`, draw helpers, `derive`, `time_value` | `sim/rng.md` |
| `crates/conformance/src/{trace,rng}.rs` (+ `tests/rng_traces.rs`) | trace loading and top-level checks; RNG trace replay | `traces/FORMAT.md`, `sim/rng.md` |
| `d2-proto`, `d2-net`, `d2-server`, `d2-verify` | stubs | |
| `tools/mpq-tool` | info, list, extract, check, formats, render | |
| `tools/data-tool` | `tables`: the Phase 2 cross-check; `links`: broken links in the live set; `gen-tables`: regenerate typed structs; `dump-compare`: fix-ups vs a 1.14d memory dump | `data/field-types.md` §6.7, `data/fixups.md`, `data/runtime-maps.md` |
| `tools/trace-recorder` | Python debugger recording RNG draws from `Game.exe` (Windows); `dump_tables.py`: the excel tables and runtime maps in 1.14d memory after the load; `record_tick.py` + `check_tick.py`: server tick, timer events, unit lists; `record_packets.py` + `check_packets.py`: client↔server messages | `sim/rng.md`, `traces/FORMAT.md`, `data/runtime-maps.md`, `sim/tick.md`, `sim/unit-order.md`, `sim/intents-events.md` |
| `tools/depcheck` | dependency rules (no Bevy outside `d2-client`) | |
| `tools/methods.py` | methods collection `docs/METHODS.md`: `check` (CI), `list`, `new`, `export` | `docs/METHODS.md` |
| `tools/spec_index.py` | section indexes in specs (`--check` in CI) | `specs/README.md` |
| `tools/cloud-setup.sh` | cloud session setup (Linux libs, pinned Rust) | |
| `tools/ghidra/` | Ghidra scripts (label import, export); `disasm.py`: disassembly, xrefs, whole-binary dump (the decompile drops register arguments) | |

## 4. Command map (what proves what)

| Command | Proves | Needs |
|---|---|---|
| `cargo test -p <crate>` | unit tests from spec vectors | repo |
| `cargo clippy -p <crate> --all-targets -- -D warnings`, `cargo fmt --all` | lint/format gate | repo |
| `cargo run -p depcheck` | crate dependency rules | repo |
| `py tools/spec_index.py --check` | spec indexes current | repo |
| `cargo test -p d2-data -p d2-formats -- --ignored` | game-file tests | `game/` |
| `cargo run --release -p data-tool -- tables` | every live table and code buffer reproduced from `.txt` | `game/` |
| `cargo run --release -p data-tool -- links` | no broken link in the live `.bin` set | `game/` |
| `cargo run --release -p mpq-tool -- check` / `formats` | every archive block / every format file decodes | `game/` |
| `cargo run --release -p d2-client -- verify` | GPU render byte-identical to CPU reference | `game/`, GPU |
| `py tools/trace-recorder/record_rng.py --seconds N` then `check_rng.py` | RNG spec matches the real game | `game/`, Windows |
| `py tools/trace-recorder/record_tick.py --seconds 200` (play by hand) then `check_tick.py <file>` | tick steps, timer queue and unit lists match `sim/tick.md`, `sim/unit-order.md` | `game/`, Windows, a player |
| `py tools/trace-recorder/record_packets.py --seconds 180` (play by hand) then `check_packets.py <file>` | message transport rules R1–R7 of `sim/intents-events.md` | `game/`, Windows, a player |
| `check_tick.py --selftest`, `check_packets.py --selftest`, `tools/ghidra/disasm.py selftest` | the checkers catch perturbations (M08) | repo (disasm: `game/`) |
| `py tools/trace-recorder/dump_tables.py` (~7 s) then `cargo run --release -p data-tool -- dump-compare traces/raw/<time>-tables` | d2rs fix-ups and maps equal 1.14d memory after the load | `game/`, Windows |

Tools read `D2_GAME_DIR` (= `<repo>/game`). If a shell doesn't have it:
`export D2_GAME_DIR="$PWD/game"`.

## 5. Local run queue

Cloud sessions add game-file checks here (command + what to look for);
a local session runs them, records the result, and removes the entry.

Fix-ups (after step 1 of §2 lands): `py tools/trace-recorder/dump_tables.py`
(or reuse `traces/raw/20261006-004246-tables`), then `cargo run --release
-p data-tool -- dump-compare traces/raw/<time>-tables`. Expected: all 70
kept tables identical (today 57), `PENDING` empty, every map with a d2rs
counterpart identical (pointer maps compared as indices). Before step 1
the result is the 2026-10-06 baseline: 57 identical, 13 differ (monequip
8 bytes is a d2rs bug).

Phase 3 recordings (local, need the user at the game, ~3 min each;
the game does not enter a game by itself):
- Combat with missiles: `py tools/trace-recorder/record_tick.py
  --seconds 200`, a caster or bow character fighting outside town;
  then `check_tick.py <file>`. Expected: 0 errors and many class-2
  (missile) runs (the 2026-10-06 run had 4). Settles `tick.md` open
  question 1.
- More message ids: `py tools/trace-recorder/record_packets.py
  --seconds 180`: waypoint, stat and skill points, stash, cube, party
  menu if any, exit to menu; then `check_packets.py <file>` (expect
  `OK`) and `--perturb <seq>` (expect "reported").

Next RNG capture when convenient (local, needs the user at the game):
start `py tools/trace-recorder/record_rng.py --seconds 120`, enter a
single-player game, kill a few monsters and pick up a drop. Recording
slows the game a lot (all 846 inline RNG sites are hooked).

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
- Git: `main` on `git@github.com:MoggerCat/MXL-ULTIMATE.git`. CI runs on
  pushes to `main` and on pull requests. A cold CI run takes ~40 min
  (Bevy), cached ~3 min.

## 7. Where facts live

Simulation core: `specs/sim/tick.md` (tick rate, steps, timer events),
`sim/unit-order.md` (GUIDs, unit/room/client lists), `sim/intents-events.md`
(+ `client-messages.tsv`, `server-messages.tsv`). Format facts: `specs/formats/*`. Map rendering: `specs/render/map-preview.md`.
Data loading and tables: `specs/data/*` (start at `loading.md`). RNG:
`specs/sim/rng.md`. Each spec's "Open questions" holds its unknowns.
Carried-over open questions not yet in a spec's list:

1. 8 unflagged invisible collision tiles in `townN1.ds1` draw as blue
   patches (`map-preview.md` OQ3; needs RE of the client tile draw path).
2. DS1 v12/13 trailing bytes (possibly an early NPC-path section).
3. DC6/DCC vertical placement (one-row disagreement between sources).
4. Meaning of the PL2 rendering tables (Phase 6).

## 8. Lessons (problems met, fixes)

| Problem | Fix |
|---|---|
| Ghidra 12.1 crashes on JDK 25 | use JDK 21 (`JAVA_HOME_OVERRIDE` in Ghidra's `launch.properties`) |
| Cargo can't extend workspace lints | `d2-sim` repeats them in its own `[lints]` |
| Running `cargo` outside the repo reinstalled `stable` (1.3 GB) | default toolchain set to 1.99.0 |
| `.txt` tables in the MPQs aren't what 1.14d uses | `.bin` is truth; the txt compiler is proven against it (`loading.md`) |
| 1.14d inlines its RNG step (846 sites) | recorder hooks every site, not one function (`sim/rng.md`) |
| A 24-agent review workflow cost ~6M tokens for little gain | one writer per spec + executable checks (`CLAUDE.md` working rules) |
| The Ghidra decompile drops register arguments (fastcall ECX/EDX, custom conventions) | read register use from the disassembly: `tools/ghidra/disasm.py` |
| Recording a game needs a player: the game never enters a game by itself | ask the user to play during the recording (~3 min) or queue it |
| GPU render exactness | R8Uint indices, sRGB palette via `textureLoad`, `Msaa::Off`, `Tonemapping::None`, pixel-aligned quads |
