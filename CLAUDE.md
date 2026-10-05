# CLAUDE.md — Project instructions for Claude Code

Read this file at the start of every session. Detailed docs live in `docs/`.
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

- **Quality comes from evidence, not from more reviewers.** Prove things
  with executable checks against the real 1.14d data (e.g. `data-tool
  tables`, `mpq-tool formats`, `d2-client verify`, traces). Don't stack
  agent review/revise/critic layers; one focused writer per spec or module,
  then the check. Keep agent prompts tight, with exact file pointers.
- **Specs are the project's memory.** Hold the bar in `specs/README.md`:
  1.14d-confirmed facts with provenance, one owner spec per rule, dense
  plain prose, machine-readable data where it beats prose.
- **Read by section.** Specs over 12 KB start with a section index
  (`<!-- index -->`, with line ranges). Read the index, then only the
  sections you need; agent prompts name them ("loading.md §6 and §8").
  After editing a spec, run `py tools/spec_index.py` (CI checks it).
- **Start from the map.** `docs/HANDOFF.md` has the code map and the
  command that proves each thing. Use it instead of exploring the repo.
- **Turn facts into checks.** When a spec holds a list or table code
  consumes, make it a TSV plus a mechanical check (like `fields.tsv` +
  `data-tool tables`). A failing check beats an agent rereading prose.
- **Say where a claim holds.** A fact passed between sessions, agents or
  documents names its scope: the branch or commit, the game version, the
  archive or table it was checked on. "Exists" means "exists on `main`"
  unless stated otherwise. A claim without its scope is treated as
  unverified by the receiver.
- **Proven methods before new ones.** For a problem, first use a project
  convention, an existing tool in `tools/`, or an established method
  (debugger traces, Ghidra exports, byte-exact cross-checks, strict
  parsers, TSV + check). Invent only when none fits; then say so in the
  commit and the spec, and give the new piece its own check before
  anything builds on it.
- **Match the agent to the task.**

  | Task | Model | Effort |
  |---|---|---|
  | RE / spec writing, debugging an exactness mismatch, architecture | Opus | high |
  | Implementation from a clear spec, extraction scripts, tools | Opus or Sonnet | medium |
  | Doc cleanup, formatting, boilerplate, renames, simple fixes | Sonnet or Haiku | low |

  Parallel agents buy speed, not savings: split only genuinely
  independent work, and give each its own files.
- **Token budget** (user's plan, 2026-10-05): ~15–20M tokens per 5-hour
  window, ~400M per week. Measured costs: a focused agent run is
  ~250–500k tokens; a long main thread costs more with every turn. Plan a
  5-hour window at **under 6M** (user's cap): roughly 8–12 agent runs plus
  the main thread. Spend it on the critical path first; run independent
  agents in parallel for speed.
  Say the planned spend before launching a batch. Raise effort to high
  only for: RE/spec writing, an exactness mismatch the first fix didn't
  solve, architecture decisions, and a spec a whole phase will build on.
- **Short sessions.** One step per session: start from `docs/HANDOFF.md`
  and the latest commit, end by updating HANDOFF/PLAN, committing and
  pushing. Start a fresh session rather than continuing a long one.
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

A cloud session that needs a game-file check adds it to the "Local run
queue" in `docs/HANDOFF.md` (exact command and what to look for) and
leaves its work on a branch. A local session runs the queue, records the
results there, and commits.
