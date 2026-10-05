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
| Reference sources | `../refs/<project>/`, outside the repo | Spec sessions only. GPL projects are read only. |
| Language | Rust | Toolchain pinned in `rust-toolchain.toml`. |
| Client engine | Bevy, pinned version (0.19.x at time of writing) | Client crate only. Upgrades are separate tasks between milestones. |
| Simulation | Plain Rust, deterministic, integer math, 25 Hz | Never depends on Bevy. |
| Network model | Server-authoritative; single player = local server | Same protocol for both. |
| Method | Clean-room specs → Rust | See `CLEAN_ROOM.md`. |
| Mod data | Patch layers applied to user's tables | Release never contains Blizzard tables. |
| Ownership gate | "Has the game" check + online account | Changed 2026-10-05. Required MPQs must exist, open, and contain the files the engine loads. No exact-hash match against a Blizzard release, since official installs changed over time. Reads only the chosen game folder. No CD-key validation logic. |
| Distribution | Private engine build; public mod only | Public mod contains our engine code, so clean-room records matter. |
| Exact pins (Phase 0) | Rust 1.99.0, Bevy =0.19.1 | Bevy 0.19.1 needs Rust 1.95 or later. Pins live in `rust-toolchain.toml` and the workspace `Cargo.toml`. |
| Dependency rules | Enforced by `tools/depcheck` (CI) | No Bevy outside `d2-client`, and `d2-sim` can't depend on proto/net/server/client. |
| Sim determinism lints | `crates/d2-sim/clippy.toml` + `float_arithmetic = deny` | Bans HashMap/HashSet, Instant/SystemTime and float arithmetic in `d2-sim`. |
| Reference install record | `tools/hash-manifest` → `traces/reference-install.toml` | Dev only, never shipped or used for gating. Records the local `game/` that traces come from, so a changed install is noticed. |
| Data source of truth (Phase 2) | The `.bin` set 1.14d loads; `.txt` compiler verified byte for byte against it | 1.14d never reads excel `.txt` in normal play (a missing `.bin` is fatal). Full policy: `specs/data/loading.md` "d2-data policy". Mods patch `.txt` cells and compile (`patch-layers.md`). |
| Per-table layouts | `specs/data/fields.tsv` + `tables.tsv`, extracted from 1.14d `Game.exe` | 92 field lists, 3,499 fields (`specs/data/schema.md`). Replace prose per-table specs; Rust table code is generated from them. Column meanings are specified by the Phase 3 specs that use them. |
| Archive search order | `specs/data/loading.md` §2: priority descending, ties newest-opened first | `ArchiveSet` (d2-formats) computes it from the open order; second group and video path assumed opened after the startup group (loading.md open question 1). Excel files resolve P → X → D either way. |
| Unspecified table callbacks (Phase 2) | Write nothing until their specs exist | `monstats` `Sk*mode`, `monstats2` composit, `monpreset` `Place`, `cubemain` inputs/outputs. The cross-check counts their bytes as explained (`field-types.md` §8.3, §10 step 5); every byte inside a field footprint must still match. |
| Data cross-check tool | `tools/data-tool` (`data-tool tables`) | Separate from `mpq-tool`: it depends on `d2-data`. Exit status 1 on any unexplained difference. |
| Spec process | One writer per spec, then executable checks | Facts confirmed against 1.14d with provenance; one owner spec per rule. Extra LLM review layers proved costly for little gain (2026-10-05). |

## Phases

Each phase lists its exit criteria. Check items off as they land.

### Phase 0 — Setup
- [x] Cargo workspace with all crates as empty stubs; dependency rules
      enforced (no Bevy outside `d2-client`, via `cargo run -p depcheck`)
- [x] `rust-toolchain.toml` (1.99.0); Bevy pinned `=0.19.1` in workspace
      `Cargo.toml`
- [ ] CI: fmt, clippy, tests without game files. *`.github/workflows/ci.yml`
      written, and the same steps pass locally on Windows. Not yet run on
      GitHub (no remote).*
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
- [ ] Post-load fix-ups and runtime maps (`loading.md` §7.4): fully stated
      rows done in `d2-data::fixup`; the rest wait for `loading.md` OQ13
- [ ] Typed structs for all tables (generated from the schema)
- [ ] Cross-reference resolution with validation errors
- [ ] Mod patch layer format + loader
**Exit:** all tables load from the user's install; patches apply; broken
references are reported.
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

### Phase 3 — Core simulation (`d2-sim`)
- [ ] D2 seeded RNG (exact sequence match) — **first**. *Spec `specs/sim/rng.md` done and matched by 32,543 recorded draws; Rust code not started.*
- [ ] Tick loop, intents in / events out, unit ordering
- [ ] Units, stats, stat lists, modifiers
- [ ] Items: generation, quality rolls, affixes, uniques/sets, runewords
- [ ] Treasure classes and drops
- [ ] Skills and combat formulas
- [ ] Level generation (DRLG)
- [ ] Monsters: spawning, AI, missiles
- [ ] Quests, waypoints, NPCs, vendors, Horadric Cube
**Exit per item:** spec status `conformance-passing`.

### Phase 4 — Conformance (runs alongside Phase 3)
- [ ] Trace recorder for the original game. *Feasibility proven 2026-10-05: `tools/trace-recorder` records every RNG draw of 1.14d (Python debugger); traces in `traces/sim/rng/`. Other event types not yet.*
- [ ] Replay harness
- [ ] Coverage report (the "99.x%" number)

### Phase 5 — Local server + bridge
- [ ] `d2-proto` message types (versioned)
- [ ] In-process server running `d2-sim`
- [ ] `d2-client::bridge`: snapshots → Bevy entities, input → intents,
      tick interpolation
**Exit:** walk around Act 1 town via the local server.

### Phase 6 — Full client
- [ ] Animation (COF/DCC), lighting, blend modes, draw ordering
- [ ] UI panels, inventory, fonts, audio, controls config
**Exit:** play through all acts locally with correct visuals.

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
