# Handoff: property tests of the wired game over time — `claude/prop-worldsim`

> To be folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` by a docs session; this file stays as the detailed record.

Cloud test session, 2026-10-06, task class: property tests from clear
specs, medium (METHODS M14). Base: `claude/tender-meitner-mphas3` at
`14f8185`. Repo only (no `game/`, `re/`, `../refs/`). Scope of every
claim: this branch, the synthetic fixture below, `cargo test` in a debug
build (overflow checks on). Fills the gap `fuzz-server.md` §5 item 2 left
(the `WorldSim` tick path was not fuzzed).

## 1. State

New file `crates/d2-client/tests/prop_worldsim.rs` (d2-client, not
d2-server: the wired fixture and its NPC / vendor rests live in
`d2-client/tests`, and `e2e_support` is reused as a module; the messages
still go straight to `d2_server::dispatch::dispatch` and
`SimGame::tick`, no bridge).

Host: `SimGame<WorldSim<TestPending>, TradeWorld<Rest>>`, built as
`e2e_single_player.rs` steps 1–3 build it: act 0's DRLG through the
level-type dispatcher on the recorded Act I seed, the ISLE preset level
generated and one room streamed, the regions, NPC control and quests on
the game seed, a sorceress (wired skills, 4000 mana, 5000 gold, both
waypoints known), a waypoint object, Akara beside the player, the
player's buckler and cap (economy wiring), `WiredSkills`. Left out: the
bridge and the cube's item world (a second unit world,
`e2e-next.md` finding 1). Fixture staging between steps (seam answers,
as the e2e stages them, not behaviour): every unit's `UnitFacts` from the
fixture positions; each new monster's target flags and collision bit
(`stage_combat`); each new missile's damage (10 points, the unwritten
`0x0059F900`).

Runs: 40–80 ops, each an optional C→S message then 0–7 ticks, then 100
quiet ticks (≥ 100, typically 250–400 ticks per run). Messages: the e2e's
valid intents (right skill at a monster or point 0x0C, on a unit 0x0D,
stat point 0x3A, waypoint travel 0x49 to ISLE / GATE, talk 0x13, chat
0x2F, trade 0x38, buy 0x32 of a store item, sell 0x33 of a player item)
and generated ones (`prop_handle.rs`'s builder: any id 0x01–0x66, 2/3 on
the handled ids, fields from the game's current GUIDs, coordinates near
the player and edge values, sizes exact / one short / one long).

| Test | Property |
|---|---|
| `same_seed_same_messages_same_game` (proptest, 8 cases) | (1) no panic, overflow or fatal path (`WorldSim::errors()`, the world handlers' faults, the interaction errors) after every message and tick; bounds: ≤ 128 units, timers ≤ 8 per unit, ≤ 64 rooms in act 0's list, active ≤ listed, transport outbox ≤ 64; (4) every dispatcher rejection (§2.3 r3 closed gate → 0; §2.4 r1 wrong size → 3, r3 unit type ≥ 6 → 2, r4 point > 50 from the staged player → 1) has the expected code and leaves the digest unchanged; (2) a second run of the same seed and ops is identical: per message and per tick, the result code, the S→C buffers byte for byte, the digest |
| `other_seed_other_digest` (proptest, 8 cases) | (3) two different game seeds, same ops (0–10, + 5 ticks): the digest sequences differ |
| `fixture_reaches_the_wired_paths` | the first tick creates the DS1 monster; the e2e's cast is accepted, the missile kills it within 40 ticks; the digest moves |
| `runs_reach_the_handlers` | spot check of the generator (4 deterministic runs, ≥ 400 ticks): 0x0C, 0x0D, 0x13, 0x2F, 0x38, 0x3A, 0x49 accepted; the dispatcher's 1, 2, 3 all seen; missiles in the digest; S→C 0x27 and 0x2A sent |

Digest (test helper, public state only, readable lines): frame and timer
bucket; game seed; drop-state seed; act 0's active rooms; per unit (type,
id order) GUID, room, class, mode, flags, act, unit / init / item seeds,
animation record and frame fields, fixture position, base and full stat
entries, timers (event, args, expire); the trade world's item store,
vendor records, interaction lists, NPC control and quest control
(`Debug`).

Runtime (debug): the file ≈ 12 s at the defaults. Hunt: `PROPTEST_CASES=150`
(all four tests, 215 s): no failure.

## 2. Bugs found

None. No panic, overflow, fatal path, bound exceeded, rejected intent
with an effect, or determinism break in any run (defaults and the
150-case hunt). Nothing in `d2-sim` / `d2-server` was changed.

Observations (not bugs, for the next session):
- The S→C output on this host comes only from the trade handlers (0x27,
  0x28, 0x29 on talk; 0x2A on buy / sell); the action, skill, waypoint
  and tick paths send nothing (`e2e-next.md` §1: their senders belong to
  unwritten specs). Property 2's byte comparison is therefore non-trivial
  only on the trade path.
- Fixture logs grow with the run (`TestPending::log`, `Rest`'s log,
  `DeathDrops::placed`, `SimGame`'s staged `UnitFacts` of removed units);
  they are test seams, not game state, and not bounded here.

## 3. Changes outside the new files

None (no fix was needed; no signature change).

## 4. Not covered, questions

1. Property 4 covers the dispatcher's own rejections only. A handler's
   early refusals are not compared: several specs order state changes
   before a refusal (`cube.md` §2 step 3.1, as `fuzz-server.md` notes),
   and a per-handler list of "returns before any effect" paths is not
   written in one place. A follow-up could take them spec by spec.
2. The cube's item world and the quest ids (0x31, 0x40, 0x58) are not on
   this host.
3. Only act 0 and the ISLE level are streamed; waypoint travel stops at
   the unwritten same-act placement, so the player never changes room
   and rooms beyond the first activation are not streamed or freed.
4. The fixture copy (tables, DS1 / DT1 sources, `TestPending`) duplicates
   `e2e_single_player.rs` (~1,000 lines). Moving it into `e2e_support`
   would serve both; not done here (the e2e file is outside this task).

## 5. Gate

`sh tools/gate.sh` on this branch, 2026-10-06: all PASS (spec_index,
methods, coverage check / selftest, trace checkers, hook selftest, fmt,
depcheck, clippy workspace, tests d2-sim + conformance, rest, d2-client,
doc-tests). `GATE: PASS`.
