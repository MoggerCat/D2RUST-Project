# Handoff (updated 2026-10-05)

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
| 2 Data | in progress: core, callbacks, first fix-ups | `data-tool tables` (2026-10-05, with callbacks): 73 live tables, 72 byte-identical, 1 explained (`monstats` `NameStr`); 4/4 code buffers identical; `d2-data` game-file tests all pass |
| 3 Simulation | in progress: RNG done | `cargo test -p d2-sim -p conformance`: spec vectors pass; all 256 draws of `traces/sim/rng/*.json` replay exactly |
| 4 Conformance | recording proven feasible | `tools/trace-recorder`: 32,543 recorded RNG draws match the spec exactly |
| 5–6 | not started | |
| 7–9 | deferred (out of current scope) | |

## 2. Next steps (in order)

1. **Run the local queue** (§5): confirms the callbacks and fix-ups on
   the game files.
2. **Fix-up specs** (spec writing / RE, local, high): the §7.4 rows
   `loading.md` OQ13 leaves open (equivalence matrix, op-stat tables and
   flags, per-class skill lists, charstats strings, set attachment, gems,
   gamble, monstats class chain, levels, automap form) and the
   `AnimData.d2` format (OQ11); ideally a post-load memory dump to check
   them (OQ15). Then implement them in `d2-data::fixup` (`PENDING` lists
   them).
3. **Typed tables** (implementation, medium): generate Rust structs from
   `specs/data/fields.tsv` + `tables.tsv` (a generator tool; generated
   code committed; a test that regenerating gives the same output).
4. **Cross-reference validation** (implementation, medium): broken links
   reported with table, row, column (`field-types.md` §6).
5. **Patch layers** (implementation, medium): `specs/data/patch-layers.md`.
   Phase 2 exit = "patches apply"; the mod itself is deferred.

## 3. Code map

| Path | What | Spec |
|---|---|---|
| `crates/d2-formats/src/mpq/` | MPQ archive (`mod.rs`), hash/block tables (`tables.rs`), crypto, PKWARE explode, Huffman, ADPCM, `ArchiveSet` (`set.rs`) | `formats/mpq.md`, `mpq-tables.md`, `data/loading.md` §2 |
| `crates/d2-formats/src/{palette,dc6,dcc,dt1,ds1,cof,tbl,font}.rs` | file formats | `specs/formats/*.md` |
| `crates/d2-data/src/txt.rs` | strict `.txt` reader, column binding | `data/txt-format.md` |
| `crates/d2-data/src/schema.rs` | embedded `fields.tsv` / `tables.tsv` | `data/schema.md` |
| `crates/d2-data/src/compile.rs` (+ `compile/tests.rs`) | cell → bytes, linkers, `strkey`/`calc`/`param` | `data/field-types.md` |
| `crates/d2-data/src/compile/callbacks.rs` (+ `callbacks/tests.rs`) | cube, skill-mode, composit, place callbacks; `@uniques`/`@sets` | `data/callbacks.md` |
| `crates/d2-data/src/fixup.rs` | post-load fix-ups and runtime maps (on a copy of the loaded set) | `data/loading.md` §7.4 |
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
| `tools/data-tool` | `tables`: the Phase 2 cross-check | |
| `tools/trace-recorder` | Python debugger recording RNG draws from `Game.exe` (Windows) | `sim/rng.md`, `traces/FORMAT.md` |
| `tools/depcheck` | dependency rules (no Bevy outside `d2-client`) | |
| `tools/spec_index.py` | section indexes in specs (`--check` in CI) | `specs/README.md` |
| `tools/cloud-setup.sh` | cloud session setup (Linux libs, pinned Rust) | |
| `tools/ghidra/` | Ghidra scripts (label import, export) | |

## 4. Command map (what proves what)

| Command | Proves | Needs |
|---|---|---|
| `cargo test -p <crate>` | unit tests from spec vectors | repo |
| `cargo clippy -p <crate> --all-targets -- -D warnings`, `cargo fmt --all` | lint/format gate | repo |
| `cargo run -p depcheck` | crate dependency rules | repo |
| `py tools/spec_index.py --check` | spec indexes current | repo |
| `cargo test -p d2-data -p d2-formats -- --ignored` | game-file tests | `game/` |
| `cargo run --release -p data-tool -- tables` | every live table and code buffer reproduced from `.txt` | `game/` |
| `cargo run --release -p mpq-tool -- check` / `formats` | every archive block / every format file decodes | `game/` |
| `cargo run --release -p d2-client -- verify` | GPU render byte-identical to CPU reference | `game/`, GPU |
| `py tools/trace-recorder/record_rng.py --seconds N` then `check_rng.py` | RNG spec matches the real game | `game/`, Windows |

Tools read `D2_GAME_DIR` (= `<repo>/game`). If a shell doesn't have it:
`export D2_GAME_DIR="$PWD/game"`.

## 5. Local run queue

Cloud sessions add game-file checks here (command + what to look for);
a local session runs them, records the result, and removes the entry.

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

Format facts: `specs/formats/*`. Map rendering: `specs/render/map-preview.md`.
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
| GPU render exactness | R8Uint indices, sRGB palette via `textureLoad`, `Msaa::Off`, `Tonemapping::None`, pixel-aligned quads |
