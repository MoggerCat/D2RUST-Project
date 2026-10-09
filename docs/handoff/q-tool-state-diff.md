# q-tool-state-diff — hand-back (2026-10-09)

Branch `claude/q-tool-state-diff`. Tooling session: one command per 1.14d
check, first divergence per channel; the playthrough harness; the suite.
Recipe for area sessions: `docs/handoff/diff-driven.md`; PC 1:
`docs/handoff/pc1-data.md` "How to check a behaviour in one command".

## What exists

| Piece | Command | Spec |
|---|---|---|
| One check, every channel, both sides | `python3 tools/scenario-diff/scenario_diff.py traces/checks/<name>.check` (`--channels`, `--orig-only`, `--d2rs-only`, `--reuse`, `--dry-run`) | `specs/tools/scenario-diff.md` |
| All checks, match % per area, playability per act | `python3 tools/scenario-diff/suite.py [--area A] [--md F]` (parallel Wine prefixes, 1.14d recordings reused) | `scenario-diff.md` §3.4 |
| state | `record_state.py` / `d2-client state-dump` / `state_diff.py` | `specs/tools/state-snapshot.md` |
| rng | `record_rng.py --frames` (x86 emulation, 11 s) / `state-dump --rng` (feature `rng-trace`) / `rng_diff.py` | `specs/tools/rng-trace.md` |
| packets | `record_packets.py` / `state-dump --packets` / `packets_diff.py` | `specs/tools/packets-trace.md` |
| draws | `record_frames.py` + `facts_render.py` / `play --dump-draws` / `facts-compare` | `scenario-diff.md` §3 r7 |
| Input, both sides | `input frame F; click X Y; clickunit T C; key K; ...` (headless hover pick, belt keys) | `scenario-diff.md` §2 r4, §3 r8 |
| C→S injection, both sides | `at <frame> send <Name> f=v... | hex ...` (1.14d original-hooks §1 r4) | `scenario-diff.md` §3 r12 |
| Pokes, every recorder and `play` | `at <frame> poke ...` (q-tool-poke's `poke.py` / `d2_sim::poke`); new `hop` (below) | `specs/tools/poke.md` |
| Playthrough (d2rs only) | `python3 tools/playthrough/playthrough.py --all --json F` | `specs/tools/playthrough.md` |

## Playthrough table (2026-10-09, `--all`, 59737caf)

| Act | Reached | In a row | First blocker |
|---|---|---|---|
| I | 12/17 | 3 | kill-zombie: a killed zombie returns to mode 1 with hp 0 (q-diff-combat-a1's death clean-up fix 069d6bcf should settle it once merged) |
| II | 14/15 | 5 | radament-killed: Radament never found by the sweep |
| III | 13/14 | 4 | flayer-jungle-altar: the altar object (cl 251) never seen |
| IV | 9/12 | 4 | izual-killed: Izual never in a death mode |
| IV blockers | 4/4 | 4 | — |
| V | 11/13 | 4 | frozen-anya: Anya (cl 527) never seen |

## Open items for the next session

1. **Switch sweep milestones to `goto preset`.** Frozen Anya: the hops
   work (1,349 ok) but level 114 is a maze and the sweep stays in x
   10000–10319 / y 6500–6819. q-tool-checkpoints' `goto <f> preset
   <level> [<type>:]<class>` (staging, `poke.md` §6) walks to a preset;
   act5 `frozen-anya`, act3 `flayer-jungle-altar`, act2
   `radament-killed` should use it instead of `sweep`. Not done here
   (the act files' targets are the area sessions' call).
2. **Headless right click walks instead of casting** in every act (C→S
   0x01 not 0x0C; the playthrough kills with missile pokes) — skills-2's
   row.
3. **d2rs findings filed:** `q-fix-m-act3-entry`, `-act4-entry`,
   `-act5-entry`, `-baal-chamber`, `q-fix-belt-load-potions`
   (`docs/handoff/build-queue.tsv`); the Akara buy (frame 24: d2rs "no
   room") with the items session.
4. **Not compared yet:** C→S 0x67's trailing bytes (1.14d buffer
   leftovers; a mask needs a spec statement); `@wp` references on 1.14d
   (gap); the hover pick is d2rs-own (a box per unit, 1.14d hit-tests
   sprites; 1.14d also needs the cursor one frame before a unit click).
5. **depcheck:** `crates/d2-sim/src/debug/rng_trace.rs`'s `thread_local!`
   is behind the off-by-default `rng-trace` feature; depcheck reads
   source text and only knows `cfg(test)`, so the file has an entry in
   `tools/depcheck/determinism-allow.txt` with the reason. A feature-aware
   depcheck could drop the entry.

## New in this last round

- poke `hop <ref> <x> <y>` (both sides, `poke.md` §1): ≤ 16 sub-tiles
  per axis from the current position to the first free spot of a fixed
  candidate ring (as q-play-act5's walk); 1.14d and d2rs move
  identically. Playthrough sweeps walk their targets with it.
- `play --send`, `record_rng --poke/--send/--input`: every channel takes
  pokes, sends and the shared input on both sides.
