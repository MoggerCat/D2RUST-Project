# CLAUDE.md — Project instructions for Claude Code

Read this file at the start of every session, then `docs/METHODS.md` (how
work is done). Detailed docs live in `docs/`.
For the current project state, findings and open questions, read
`docs/HANDOFF.md`.
Before any architectural change, read `docs/ARCHITECTURE.md` and
`docs/EARLY_DECISIONS.md`.

## What this project is

A private Rust reimplementation of Diablo II: Lord of Destruction, built as the
development base for an online overhaul mod. The engine is written from
**behavior specifications** (`specs/`), never translated line-by-line from
decompiled code.

- Private development build: not distributed.
- Public release: the mod only. It requires the user's original D2 LoD files
  **and** an online account to play. The file check confirms the player
  *has* the game, not that their copy matches one exact release. It never
  compares installs against fixed hashes and never scans beyond the game
  folder.
- **Current scope: Phases 0–6** (engine through the full local client).
  Online, ownership gate and the mod (Phases 7–9) are deferred; the mod is
  a separate future project. Don't spend effort on them.

## Hard rules (never break these)

1. **No Blizzard files in git.** MPQs, executables, DLLs, extracted assets,
   extracted .txt/.bin tables and saves go in `game/` (gitignored). Never
   commit, embed, or `include_bytes!` them. The pre-commit hook in
   `tools/hooks/` enforces this.
2. **No decompiled code in `crates/`.** Ghidra output lives only in `re/`
   (gitignored). Rust code implements a spec in `specs/`, and every module
   names the spec it implements: `// Spec: specs/items/affix-rolling.md`.
3. **Session separation.** *Implementation* tasks read `specs/`, `docs/`,
   and `crates/` only; do not open `re/` or `../refs/`. *Spec-writing* tasks
   may read `re/` and `../refs/`; their output is prose, tables and behavior descriptions in
   `specs/`, never pasted decompiler output.
4. **License hygiene.** D2MOO (MIT) and Riiablo (Apache-2.0) may be adapted
   with notices in `THIRD_PARTY_NOTICES.md`. OpenDiablo2 / OpenD2 (GPL-3.0):
   read for understanding only, never copy code.
5. **Bevy is for the client only.** `d2-client` is a Bevy app. Game logic
   never lives in Bevy systems, components or resources. `d2-sim`,
   `d2-data`, `d2-formats`, `d2-net` and `d2-server` must not depend on
   Bevy. The client talks to the game only through `d2-client::bridge`.
6. **Determinism in `d2-sim`.** No I/O, no wall-clock time, no global or
   thread RNG, no `HashMap` iteration affecting outcomes, no floating point
   in game logic (integer / fixed-point math as the original uses). All
   randomness flows through the D2-compatible seeded RNG, in the original's
   draw order.
7. **Server-authoritative from day one.** Even single-player runs as a local
   in-process server. The client never decides outcomes (damage, drops,
   item stats, movement validity); it sends intents and renders state.
8. **Mod content is separate from fidelity code.** Original behavior is the
   default. Mod changes go in mod data layers or behind explicit rule
   switches (`Ruleset::Original` vs `Ruleset::Mod`), never as edits to
   fidelity code paths.
9. **Mods ship as patches, not copies.** Mod data is expressed as changes
   applied to the user's own tables at load time. Never write out full
   modified Blizzard tables into the repo or a release.
10. **Fidelity is measured, not guessed — everywhere.** Every behavior taken
    from the original is held to exact match; there is no "close enough" tier.
    "Exact" means equal under a comparison defined in the feature's spec:
    identical bytes for logic, data and RNG; identical pixels for rendering;
    identical decoded samples and trigger ticks for audio; identical tick
    numbers (not wall-clock time) for timing. A feature is "done" only when
    its check passes against the original 1.14d (traces, live data, renders).
    A feature without a check yet is "unverified", never "done": queue the
    check (`docs/HANDOFF.md` §5) instead of judging by eye.

## Repository layout

```
CLAUDE.md
docs/                 plan, architecture, early decisions, survey, files, clean room
specs/                behavior specifications (source of truth for code)
crates/
  d2-formats/         MPQ, DC6, DCC, DT1, DS1, COF, palettes, string tables
  d2-data/            typed .txt/.bin tables + mod patch layers
  d2-sim/             deterministic game logic (no I/O, no rendering, no Bevy)
  d2-proto/           client<->server message types (shared, versioned)
  d2-net/             transport
  d2-server/          authoritative server, accounts, character storage
  d2-client/          Bevy app: rendering, input, UI, audio, bridge
  d2-verify/          "has the game" check on the user's install (ownership gate)
  conformance/        trace-based tests against the original game
tools/                trace recorder, extractors, git hooks, dev utilities
traces/               recorded original-game behavior (our own observations)
game/                 user's original D2 install (gitignored)
re/                   Ghidra projects and exports (gitignored)
../refs/              reference sources (D2MOO, Riiablo, ...), outside the repo
```

`../refs/` is spec-writing material, like `re/`: implementation sessions
don't open it.

## Pinned versions

- Rust: stable, edition 2021 (record exact toolchain in `rust-toolchain.toml`).
- Bevy: **pinned to one version** in the workspace `Cargo.toml` (0.19.x at
  time of writing). Do not upgrade Bevy as part of a feature task. Upgrades
  are their own task, done between milestones, and touch only `d2-client`.
- D2 target: **1.14d** behavior (retail LoD, single merged `Game.exe`).
  D2MOO documents 1.10f: treat it as a strong hint, and confirm every
  behavior against the 1.14d binary and traces before it goes in a spec.

## Conventions

- `cargo fmt` + `cargo clippy -- -D warnings` must pass.
- `thiserror` in libraries, `anyhow` in binaries/tools.
- Binary parsing: explicit little-endian reads; no `unsafe` transmutes of
  file data.
- Tests needing game files are `#[ignore]` and read `D2_GAME_DIR`; CI runs
  without game files.
- Follow D2MOO / community names for structs and fields.
- Every persisted format (saves, protocol, traces, caches) has a version
  number from the first commit.

## How to work a task

1. Find or write the spec in `specs/` (how: `specs/README.md`; template:
   `specs/_TEMPLATE.md`).
2. Implement against the spec in the right crate.
3. Add unit tests from the spec's test vectors, then conformance tests.
4. Update the spec status and the checklist in `docs/PLAN.md`.
5. If a decision was made, add it to the decisions log in `docs/PLAN.md`.
6. Leave the workspace building and tests passing; summarize what changed.

## Working rules

Every method in `docs/METHODS.md` applies (21 entries; each one's **Here**
line is its binding in this project, with the model/effort table and the
token budget). Read it at the start of a session, after this file. New
methods go there (`py tools/methods.py new "Title"`), not here. Project-only
rule:

- **Disk is limited on the developer PC.** Build only the crates you need;
  build `d2-client` (Bevy) only for client work.

## Where work runs

Cloud sessions have the repo only: no `game/`, no `re/`, no `../refs/`.

| Work | Where | Needs |
|---|---|---|
| Implementation from `specs/`, unit tests, refactors, CI fixes | cloud (or local) | repo |
| Spec writing / RE | local | `re/`, `../refs/`, `game/` |
| Game-file checks: `data-tool tables`, `mpq-tool formats`, ignored tests, `d2-client verify`, renders | local | `game/` |
| Recording traces from the original game | local (Windows) | `game/`, `tools/trace-recorder` |

The ordered local run guide is `docs/LOCAL-RUN.md`.

A cloud session that needs a game-file check adds it to the "Local run
queue" in `docs/HANDOFF.md` (exact command and what to look for) and
leaves its work on a branch. A local session runs the queue, records the
results there, and commits.
