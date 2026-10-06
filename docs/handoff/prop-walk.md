# Handoff: property tests of path core and walk — `claude/prop-walk`

> Not yet folded into `docs/HANDOFF.md` (§1, §3, §7, §8) and `docs/PLAN.md`; a docs session folds it, then this file stays as the detailed record.

Cloud test session, 2026-10-06. Task class: property tests from specs plus
root-cause fixes, medium effort (METHODS M14). Base:
`claude/tender-meitner-mphas3` at `729c76e`. Repo only, synthetic grids, no
game files (M09). Inputs read: `specs/sim/pathing.md`,
`specs/sim/path-placement.md` §1–§6, `docs/handoff/impl-walk.md`,
`impl-path-core.md`, `crates/`. Parallel sessions `wire-path-sim` (path
seams, `wiring/`) and `mutants-path` (`mutant_tests.rs`): this session
touches neither; its work is two new test files and this note. No change
to non-test code.

## 1. State

Proptest invariant tests through the public API of `d2_sim::path`. Each
reference model and seam fake is written from the spec rules, not from
the code. Same conventions as `prop-sim-core`: a default case count per
property, `PROPTEST_CASES` overrides it, no failure persistence.

| File | Property | Model (spec) |
|---|---|---|
| `crates/d2-sim/tests/prop_path_core.rs` | `centre_round_trips` | `path-placement.md` §1 r2: sub-tile → 16.16 centre → sub-tile over the u16 range the record holds; fraction 0x8000 |
| | `client_and_distance` | §1 r1 (tile × 5), r3 (both client forms; at a centre a = 32x + 16), r4 (dx² + dy², symmetric) |
| | `cell_lookup_follows_adjacency` | §4 r1 on up to 6 rooms (overlap allowed) with random partial, ordered adjacency arrays; null and inactive rooms give none |
| | `footprints_match_the_model` | §4–§6 state machine on a 3×3 room layout (gaps, a room without a grid, null / inactive room arguments, random initial bits): random footprint add, remove (player / monster / object / other rules, force), try / forced / missile moves and box set / clear. After every operation each grid equals the model grid. Remove, try-move and missile-move results equal the model's. Point, plus, box (1×1, 2×3, 4×1, 5×5), size (−1..4) and pattern (0..6) queries equal the model's before and after |
| `crates/d2-sim/tests/prop_walk.rs` | `compute_returns_free_bounded_deterministic_paths` | `pathing.md` §3 with types 1, 7 (player) and 2 (monster) on random walls in one room. Count 0..77, index 0. Every point is free under the fake's §4 view, except the toward point P of edge case 4 (below). A clear §5.1 r4 ray gives exactly [target] for types 2 and 7. Every segment is an 8-direction run of free cells, except a first non-run segment, which must end at the reference ray's P. Type 2's greedy part is ≤ max distance. The grid is unchanged after the compute (§3 steps 6, 9). Two runs on copies give identical records and worlds |
| | `astar_reaches_iff_bfs_does` | §7 called directly (patterns 0, 1, 2): the output is corners of free 8-direction runs from the start. Reaching the target implies BFS reaches it. When the start's component has ≤ 200 cells (storage cannot fill), A* reaches the target iff BFS does. Start = target gives 0 |
| | `astar_serpentine_reaches_far_end` | §7 r2 storage bound: on serpentine corridors of ≤ 200 cells, A* always reaches the far end (forces large searches, which random grids rarely do) |
| | `greedy_walk_stops_at_max_distance` | §5.1 r5 (velocity 0 → P = start) and §5.2 steps 4–5 in an empty room, target ≥ 20 away: the monster's greedy walk is exactly 14 steps, or 13 when the last iteration turned (its end is not appended) |
| | `movement_stays_free_and_stops` | §1 → §9: C→S 0x01 / 0x03 then player event 0 every tick until stopped (≤ 2000 ticks). Mode (§1.5 step 2: run without stamina → walk, town → 6). Velocity = 6·256·p/100 with p = f + stat 67 (+50 when running, §8.2), floor 25, f = 150·s/(150 + s) (§8.1). Each tick: \|dir_vec\| ≤ 4096 per axis, direction and new direction < 64, velocity ∈ {v, 0}, \|vel_vec\| ≤ 16·velocity, the unit's cell is free. At the stop: cell centre (fraction 0x8000), count = index = 0. Every try move crosses one cell, and is accepted iff its cell is free. Without a refusal the unit stops on the path's last point. The grid then holds exactly one footprint, at the unit |
| | `requests_never_panic` | §1 on arbitrary payloads: any message id, u16 point coordinates (as the transport parses them, `client-messages.tsv`), any u32 type / GUID, random mode, class, cursor item, states 13 / 15 / 42 / 54, used skill, frame, type-1 expire, town, and a target unit of any type. Result 0, `None` for ids outside 0x01–0x04. The outcome agrees with a §1.3 mode check written from the table. `Moving` → mode 2/3/6, `Neutral` → mode 1/5. The seed moves only with state 42 and an interrupt skill (Randomness 2). Up to 40 ticks of event 0 follow, and a stop → mode 1/5 |

Branch coverage, measured on one default run (temporary counters, not
committed): 134 of 256 walks moved, 2 had a refused move; A* reached the
target in 89 small-component and 21 large-component cases; the request
outcomes covered `Moving`, `Neutral`, `ModeRefused`, `InterruptRefused`
and `NoTargetUnit`.

Hunting runs (debug, overflow checks on): `PROPTEST_CASES=10000` on both
files, and 3000 × 4 after the last changes: all pass. Release:
`PROPTEST_CASES=20000` × 2: all pass.

No coverage claims were added, following `prop-sim-core` (the properties
overlap the existing unit-tier claims of `impl-walk` / `impl-path-core`).

## 2. Bugs

**None found in the code.** No panic, overflow or model mismatch in any
run above. No root-cause fix was needed, so no non-test file changed.

One property had to follow the spec more closely (a test mistake, not a
code bug; recorded because the spec does not spell out the consequence):

- **PW-Q1: edge case 4 lets the toward path start at a colliding cell.**
  Minimal input: room 21×13 at (40, 60), start (52, 65), target (60, 70),
  monster pattern 1, walls such that (55, 67) and row y = 68 are wall.
  Ray §5.1 r4, x-major (Nx 9, Ny 6): after the minor step to y = 67 with
  err = 0, the cell (54, 67) is not tested (edge case 4). The next major
  step is blocked, so the remembered cell P = (54, 67) is returned, and
  its plus meets the wall at (55, 67). §5.2 steps 2–3 then store P as
  points[0] (and edge case 3 may store it twice). A unit walking this
  path is refused at P and stops at the last free cell (edge case 7).
  The code does what the spec says; the property allows exactly this
  point. For the spec owner: edge case 4 could add "so the returned
  point P can collide".

## 3. M08 (each check can fail)

Mutants in the code under test, each restored afterwards; result at the
default case count:

| Mutant | Caught by |
|---|---|
| `footprint.rs`: marker applied for any mask (≠ 1 instead of ≠ 0) | `footprints_match_the_model` |
| `collision.rs`: top strip at full width instead of the inside width | same |
| `footprint.rs`: missile move stamps at new unless 0x4 (not 0x5) | same |
| `collision.rs`: unknown pattern → 0 instead of 0xFFFF | same |
| `footprint.rs`: monster keeps its footprint in mode 17 instead of 12 | same |
| `find.rs`: A* does not skip colliding neighbours | `compute_…`, `astar_reaches_iff_bfs_does` |
| `find.rs`: A* storage 120 instead of 200 | `astar_serpentine_reaches_far_end` |
| `find.rs`: greedy loop max distance + 1 / + 6 / − 1 | `greedy_walk_stops_at_max_distance` |
| `geom.rs`: ray tests after a minor step with err ≥ 0 (edge case 4 removed) | `compute_…` |
| `step.rs`: footprint move accepted unless 0x1000 | `movement_stays_free_and_stops` |
| `velocity.rs`: velocity percent floor 20 | same |
| `request.rs`: mode 13 check on class ≠ 1 | `requests_never_panic` |
| `find.rs`: unit footprint not put back after the compute | `compute_…` |
| `find.rs`: open-list insert before f > (not ≥) | **not caught** (tie order changes which path, not whether it is free or reaches; the W2 unit vector catches it, `impl-walk.md` §5) |

## 4. Not covered (gaps for later sessions)

- Multi-room walks: the walk fake is one room, so room recache (§9.6
  r9), room-change messages (§9.8) and `OUTSIDE_ROOM` are not exercised
  by the properties.
- Missiles (§6 r3 through the walk, §9.4 missile branches), knockback
  (mode 19, type 8), monster re-path (§9.10 with a non-zero budget), and
  target units that move between ticks.
- The footprint model uses complete adjacency, on purpose. The room from
  which each stamped cell is looked up (`footprint.rs`
  `TODO(spec: path-placement.md §5.1)`) is unspecified, and with complete
  adjacency every reading gives the same result. Partial adjacency is
  covered for the lookup itself (`cell_lookup_follows_adjacency`).
- `set_foot_mask`, `make_corpse_footprint`, `teleport` (§5.3, §6 r4) have
  no property.

## 5. Port to the `wire-path-sim` walk API

`claude/tender-meitner-mphas3` at `5413b24` (main `5cc2cfb` +
`render-wire`) reshaped `path::walk` (`docs/handoff/wire-path-sim.md`
§3–§4). The base is merged into this branch, and `prop_walk.rs` is ported
with every property kept; `prop_path_core.rs` needed no change.

- One context `World` implements `CollisionRooms` (one room, a
  `CollisionGrid`), `PathWorld` and `WalkUnits`. The walk code now calls
  the core's `find_room`, `pattern_collides` and `try_move`, so the room
  grid is a real `CollisionGrid`. The reference `query` / `free` /
  `stamp` stay the test's own, written from the spec.
- Records are `record::DynamicPath`; the target goes through
  `put_target` / `final_target()`. `set_path_type`, `PathTables::spec()`
  and `Finder { t, c }` replace the old helpers.
- The fake no longer sees footprint moves (the core's `try_move` does
  them), so `movement_stays_free_and_stops` checks each tick's crossings
  through the path's saved steps (type 7 has flag 0x20000): flag 0x8 is
  set iff the cell changed; each saved step is 8-adjacent to the one
  before and free; the unit ends in the last one. A refusal (collided
  mask ≠ 0) must stop the movement in the same tick (§9.6 r4, edge
  case 7). Without a refusal the unit still has to stop on the last
  point.
- M08 re-run on the new code: the same mutants are caught (footprint
  move accepted unless 0x1000, A* neighbour skip, storage 120, greedy +1,
  ray err ≥ 0, floor 20, mode 13 class, footprint not restored). The
  open-list tie order still survives, as before.

## 6. Gate

`sh tools/gate.sh all` after the port (2026-10-06, on the merged base
`5413b24` with the clippy fix below): **GATE: PASS** (all 13 steps).
The first run after the port failed only workspace clippy
(`unnecessary_mut_passed`: `astar` takes `&Finder`), fixed in the test.

Earlier, before the port, at `eb20a75`:
**GATE: PASS**. spec_index, methods, coverage `--check` / `--selftest`,
trace checkers, pre-commit selftest, fmt, depcheck (+determinism),
workspace clippy, `d2-sim` + conformance tests, the other crates' tests,
`d2-client` tests (322 / 322), doc-tests: all PASS.
