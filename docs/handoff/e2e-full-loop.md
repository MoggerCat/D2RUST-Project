# Handoff: the full single-player loop with the path provider on — `claude/e2e-full-loop`

> Not yet folded into `docs/HANDOFF.md` / `docs/PLAN.md` (neither is edited here); the coordinator folds it (§1 state, §3 code map, §7 questions).

Cloud implementation session, 2026-10-06, task class: integration from
clear specs, medium (METHODS M14). Base: `claude/tender-meitner-mphas3`
at `6cd6480` (unify-items, wire-path-server, s2c-use merged). Repo only,
synthetic tables, fixed seeds, no game files (M09). No `d2-sim` /
`d2-server` file changed: one new test file. Parallel session
`path-update-pass` (update-pass callers for walking) owns no file here.

## 1. State

**Wired, unverified** (M02): no rule added. New
`crates/d2-client/tests/e2e_full_loop.rs` (3 tests, all pass) runs the
loop over the bridge on `SimGame<WorldSim<_>, WiredWorld<Rest,
WiredSkills>>` (the `e2e_single_player.rs` fixture: synthetic act 0,
ISLE / GATE preset levels, the DS1 monster, Akara, the game's one item
store and inventory model) with **`enable_paths` called before any
allocation**: every position, step, placement and warp is
`d2_sim::wiring::path`'s; the fixture's `Pending` has no path answers
left. 119 bridge frames, one tick each.

| Step | C→S | Result | S→C (asserted exact) | Stops at |
|---|---|---|---|---|
| join, tick 1 | — | — | none | — rooms activated (4), the DS1 monster created on its path at (40012, 40010) |
| walk toward the monster | 0x01 (40016, 40014) | 0 | none (20 frames) | — mode 2 → 1, monotone per-tick steps, ends on the target centre |
| attack | 0x0C | 0 | none | **the missile's path build** (`missiles.md` §R2.3 step 16 → `pathing.md` §3): `Walk(Fatal("path type without a function"))`; the missile leaves without a hit; no kill, no drop, no experience |
| (drop) pick-up | — | — | — | **not reached** (the kill): a ground cap made by the economy wiring stands in |
| run to the cap | 0x03 (40030, 40026) | 0 | none (50 frames) | — mode 3, stamina drained, ends on the cap's sub-tile |
| pick to cursor | 0x16 cursor 1 | 0 | 0x9C 1, 0x47, 0x48 | — (distance: staged seam, §4 F4) |
| place in grid | 0x18 (8, 0) | 0 | 0x9C 4, 0x47, 0x48 | — |
| run to Akara | 0x04 type 1 | 0 | none (24 frames) | — ends **on** Akara's sub-tile (§4 F2) |
| talk | 0x13 | 0 | 0x27, 0x29, 0x28 | — |
| trade | 0x38 action 1 | 0 | none | — store generated |
| sell the cap | 0x33 | 0 | 0x2A kind 3 code 1, GUID, gold | — removed, freed, price received |
| buy the store cap | 0x32 | 1 | 0x2A code 9, GUID −1 | **item copy `0x0055A2A0`** (as `e2e_single_player`) |
| walk to the waypoint | 0x02 type 2 | 0 | none (8 frames) | — ends on the object's sub-tile (objects have no footprint) |
| waypoint travel | 0x49 → GATE | 0 | 0x07 (placement room's tile origin, level 31), 0x0D (pos + 3) | — placed by the path code in GATE's spawn room; next tick nothing (no 0x15, `wire-path-server.md` §4 F1) |

`same_seed_same_run` compares the whole transcript (frames, per-walk
precise positions, seeds, monsters' path positions, logs, store, the
player's end position / room / mode, stats). `other_seed_other_run`
(M08): another game seed changes the game seed, the monster seeds and
the store; the walks and every result code are equal.

## 2. Code map rows (for `docs/HANDOFF.md` §3)

| Path | What | Spec |
|---|---|---|
| `crates/d2-client/tests/e2e_full_loop.rs` | the full loop above on the wired host, path provider on | `pathing.md` §1.1, §9, §10; `path-placement.md` §2.4, §2.5, §10, §11; `use.md` §1; `missiles.md` §R2.3; `inventory-moves.md` §7–§8; `npc.md` §2; `vendors.md` §3–§9; `waypoints.md` §6–§8 |

## 3. Fixes and signature changes

None outside the new test file. Fixture deltas against
`e2e_single_player.rs` (fixture data, not behaviour): the combat
monstats are `e2e_support::monstats()` with row 0 the DS1 monster (the
path shape of Akara reads her monstats row, `path-placement.md` §3; with
one row she got no path and sat at (0, 0)); the sorceress' charstats
walk / run columns (6, 9, drain 20) and stats 67 = 100, 10 = 0x6400, as
`e2e_walk.rs`; no cube.

## 4. Findings (blockers first)

- **F1 (blocks the kill, so the drop and its pick-up)** — `TODO(spec)`
  in the test. With the path provider a missile gets a dynamic path of
  type 4 (`path-placement.md` §2.4: "missile: masks 0, type 4"). Its
  creation builds the path (`missiles.md` §R2.3 step 16 →
  `0x00649970`); `pathing.md` §3 step 1 branches to the missile path
  `0x00649760` only on path flag 0x40000, which no written rule sets,
  so §3 runs type 4's function: none, fatal (§2 table). Two spec
  gaps: (a) where 0x40000 is set for missiles (allocation §2.4, or §R2.3
  step 15); (b) the missile path function `0x00649760` itself (pathing
  OQ3, "missile flight on the path provider", `HANDOFF.md` row 3).
  Settle in Ghidra: `0x00649D00` for missiles and `0x00649760`.
- **F2** — `TODO(spec)` in the test. A walk / run to a unit (0x02 /
  0x04) ends on the target's own sub-tile: the stop distance (path
  +0x93, `pathing.md` §9.5 rule 3) has no written setter (0), and the
  player's move mask 0x1C09 (§2.4) does not hold the monster footprint
  bit 0x100. Settle: who writes +0x93 for the 0x02 / 0x04 mode request
  (`pathing.md` §1.2) and the interaction approach.
- **F3 (code, not provable here)**: `wiring::economy::death::
  monster_death_drop` reads the monster's position from `h.x.position`
  (`Pending`) instead of `h.path_position`; with the path provider on it
  would read (0, 0) and drop at (2, 3). Not fixed: the e2e cannot reach
  the drop (F1), so no test proves it. Fix (one line) with a d2-sim test
  of the drop with paths on, or when F1 is settled.
- **F4** — `TODO(spec/wiring)` in the test. Distances on the item and
  NPC paths are still staged seams, not the path positions: the pick-up
  (`InvRest::distance`, `inventory-moves.md` §8.1 rule 4) and the talk range
  (`TradeRest` distance, `npc.md` §2), both `0x00641530` (unit distance,
  written in `pathing.md` §9.5). The walks above put the player on the
  cap and on Akara, but the handlers do not read it. Wiring task: answer
  these seams from `ActionHooks::path_position` / the §9.5 formula when
  the provider is on.
- **F5**: an item made on the ground by `Economy::create_item` (mode 3)
  has no path (`path_has` false): the item units are made without a
  position (`item_units.rs`), and `path-placement.md` §2.5 gives a
  floor item its static path only when added at a position. The
  inventory model keeps the ground position in its item data (§2.2),
  so the pick-up works; a dropped item's path would come from the floor
  drop (§9) once the drop runs with paths.
- **F6**: the dispatcher's range checks still read `SimGame`'s staged
  `UnitFacts` (`wire-path-server.md` §4 finding 3): the test re-stages
  them from the path positions before each request.

## 5. Questions

- Q1 (F1): the missile path flag 0x40000's setter and `0x00649760`.
- Q2 (F2): the stop distance of a walk / run to a unit.

## 6. Local checks to queue

None new runnable now. When F1 is settled and the sorceress kill
recording (`e2e-combat-path.md` §6 item 2) exists, replay it through
this test with paths on and compare the missile's per-tick path
positions, the hit frame and the drop spot (expected equal).

## 7. Gate

`sh tools/gate.sh`: **GATE: PASS** (spec_index, methods, coverage 4,146 claims 0 errors + selftest, trace checkers, hook selftest, fmt, depcheck + determinism, clippy workspace, tests: d2-sim + conformance 2,034 passed, rest 599, d2-client 366 (e2e_full_loop 3), doc-tests).
