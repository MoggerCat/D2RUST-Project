# Handoff: property tests of the path wiring on the wired sim — `claude/prop-wired-path`

> Not yet folded into `docs/HANDOFF.md` (§1, §3, §7, §8) and `docs/PLAN.md`; this file is the detailed record until a docs session folds it.

Cloud test session, 2026-10-06. Task class: property tests from specs plus
root-cause fixes, medium effort (METHODS M14). Base:
`claude/tender-meitner-mphas3` at `99b1e72`, merged with its head `6cd6480`
before the gate (§8). Repo only, synthetic tables and DS1s, no game files
(M09). Inputs read: `specs/sim/pathing.md` §1, §8–§10,
`specs/sim/path-placement.md` §3–§12, `specs/monsters/population.md`
§9, §11, `specs/sim/intents-events.md` §2.4, `specs/drlg/levels.md` §10,
`docs/handoff/{wire-path-sim,prop-worldsim,prop-walk}.md`, `crates/`.
Parallel sessions `wire-path-server` (server walk/run handlers) and
`unify-items` (item wiring): their files are not touched.

## 1. State

New file `crates/d2-sim/tests/prop_wired_path.rs`. Host: the worldgen
fixture `d2_sim::bench_fixtures::Fx` (`WorldSim<TestPending>`, act 0's DRLG
through the level-type dispatcher on the recorded Act I seed), ISLE
(level 30, 40 × 18 tiles at (8000, 8000)) generated and all 15 rooms
streamed, `ActionHooks::enable_paths()`, population on
(`TestPending::populate`), charstats walk 6 / run 9 (vector V1). Per case:
one or two players allocated through `View::allocate` at fixed spawn
points, **each with a client** (see counterexample 1), stat 67 = 100,
stamina 0 / 1 / 100 (8.8); 0–400 random static cells (WALL, NOPLAYER,
MISSILE_BARRIER) away from the spawns; monstats2 `SizeX` 0..=3 written
into all three copies the fixture keeps (world tables, population's
`PopTables`, action tables).

Operations (10–40, then 60 quiet ticks): C→S 0x01 / 0x03 to a point
within 50 of the player (`intents-events.md` §2.4 r3), or next to another
unit (±2); 0x02 / 0x04 to a unit within 50 (§2.4 r4), all through
`wiring::path::walk::walk_message`; 1–30 ticks of `d2_sim::tick::tick`; a
monster at the coarse free box around a random point (§8, n 1, mask
0x3C01, as population §6.3 r4 does) then `View::allocate`; a same-act
level warp to ISLE, tile index 0 (`wiring::path::place::level_warp`).

| Property (checked after every operation and every tick) | Spec |
|---|---|
| Each unit's size shape (point / plus / box) lies on sub-tiles free of its move mask in the static grid; a unit that changed cell in a tick has its whole pattern on active, free cells | path-placement §3, §4 r2, r5, §6 r1, §7, §8; population §9.3 |
| The grids equal a reference rebuilt from the static grid and every unit's pattern footprint (§3 table, markers only with a non-zero mask): static bits unchanged; no unit bit without an owner; every owner's bit present, except on cells two footprints touched since (§5.1 clear ANDs the complement, so a shared bit is lost when either owner clears it) | §5.1, §5.2, §6 |
| NO_PATH / PET marker cells of two units never coincide; a unit that moved in a tick holds no other unit's marker in its pattern | §5.1, §6 r1 |
| Per tick, per axis, \|Δ precise\| ≤ 16 · velocity (max of before / after), + 0x8000 when it ends on a cell centre | pathing §9.4 step 2.1, §9.6 r3–r4, §9.7 |
| Room active, contains the unit's cell, equals the room list entry, unit in the room's list, flag 0x2 cleared; a room change in a tick goes to an adjacent room of the old one and keeps the old room as the previous room | pathing §9.3, §9.6 r9, §9.8 |
| Warp: result `Some(true)`, the room is an active room of ISLE holding the player, the plus is free, flags 2 0x10000, event 14 at frame + 50 | path-placement §10 r3–r6, §11; levels §10.5 |
| A walk request returns 0 and moves nobody | pathing §1.1, §1.5 |
| Two runs of the same input: identical digests after every step (frame, game seed, all path records, all grids, unit rooms / modes / flags 2 / seeds / timers, errors); no panic, overflow or `WorldSim::errors()` entry | CLAUDE.md rule 6 |

The reference is written from the spec tables, not from the code. Its
"since" rule: a cell touched by one unit that was placed, removed or
changed cell in the step is exact again (that unit wrote it last); a cell
touched by none is exact; otherwise it keeps its state. Cells of a room
streamed in again are rebuilt from the tiles and start tainted.

Runtime (debug): the file ≈ 22 s at the defaults (10 cases; each case
runs twice for the determinism comparison). Hunting runs: release,
`PROPTEST_CASES=400` (before `WalkNear`) and 600 (after): see §5.

## 2. Bugs found and fixed

**PWP1 — population's size test read one sub-tile for every size**
(`wiring/worldgen/population.rs`, `PopWorld::collides` = `0x0064D9B0`,
and `mask_at` = `0x0064CB30`). The adapter carried the old
`TODO(wire-action W5)`: "the footprint a size covers is not specified".
`path-placement.md` §4 rules 1–5 now specify it (size 0, 1 point; 2 plus;
3 box; other 0xFFFF; a cell without a room reads 0x27), and the path core
implements it. Fixed as the missile adapter already does: with the path
provider on, `collides` is `path::collision::size_value(…) ≠ 0` and
`mask_at` is `point_value(…) ≠ 0`; with it off nothing changes (every
existing test keeps its answers). Minimal input: `SizeX` 2, a wall at the
ISLE DS1 monster's (12, 11): the preset monster was placed on (40012,
40010) with its plus over the wall; now §9.3's test refuses the point and
the r = 4 search (§11.3 step 2) places it on a free plus. Regression:
`regress_population_tests_the_size_shape` (fails before the fix).

No other code bug. No panic, overflow, wiring error or determinism break
in any run.

## 3. Counterexamples that are not code bugs (each a fixed regression test)

| # | Counterexample | Reading | Test |
|---|---|---|---|
| 1 | A player without a client loses its room after ~132 ticks: tick step 9 deactivates it (inactivity counts only clients, `rooms.md` §7.2) and the path keeps the stale room | fixture: every game player has a client; the host now adds one | `regress_a_client_keeps_the_players_rooms_active` (M08: without the client the room is lost) |
| 2 | A `SizeX` 1 monster stands with its plus over a wall | specified: pattern of size 1 is the plus (§3) but the placement tests the size shape, one cell (§4 r5, population §9.3); property 1 tests the size shape | `regress_a_size_one_monster_is_placed_by_its_cell_not_its_plus` |
| 3 | A monster whose room tick step 9 deactivated leaves the room list but keeps its path record and footprint | the compress to inactive storage (`0x005433F0`) is not specified (`unit-order.md` OQ 3); `TickHooks::compress_unit` is a no-op. The property skips such monsters (their last footprint is still allowed in a neighbour's grid) | `regress_a_deactivated_room_leaves_its_monster_path_record` |
| 4 | Two players warped to ISLE land on the same point | the teleport stamps from the old room's lookups (path core reading of §6 r4; open point `wire-path-sim.md` §6: which room `0x00650910` passes to `0x0064EFA0`); the first player's spawn room is not adjacent to its old room, so nothing is stamped and the second free search does not see it. The property treats a warped player as unstamped until it next changes cell | `regress_second_warp_lands_on_an_unstamped_player` (a spec answer that stamps from the destination room changes it) |
| 5 | After 132 ticks a warp's check read the static grid before the spawn room was streamed back in (test mistake) | levels §10.5 makes the room active | `regress_a_warp_streams_its_spawn_room_back_in` |
| 6 | A monster placed through the coarse free-box search stands outside the room the search returned: from room A (40000, 40080, 40 × 10) around (40008, 40080) with a wall at (40008, 40078), §8 returns A with (40006, 40078), a cell of the room above | the code follows §8's wording: pass 1's row y = 40079 is outside A's rows, so its cell visit reads the room above's rect; pass 2's row y = 40078 is inside "the rect last read" (the room above's), so rule 2 takes `room` (A) as the row room, and the cell is inside A's columns. **PWQ1** (for the spec owner): §8's edge cases could state that the out room need not hold the point, and §10 / population then put a unit in a room that does not hold its cell until its first move (room recache, pathing §9.6 r9). The property exempts such a monster until it changes cell | `regress_coarse_box_room_need_not_hold_the_point` (M08: without the wall pass 1 returns the room above) |

## 4. M08 (the properties can fail)

Mutants in the code under test, each restored afterwards; the property
at 10 cases:

| Mutant | Caught by |
|---|---|
| `path/footprint.rs` `try_move`: test result `& !0x1000` (NO_PATH ignored) | marker property ("moved over …'s marker"); survived until the `WalkNear` operation was added |
| `wiring/path/walk.rs`: room-list insert skipped on a room change | room property (list room `None`) |
| `path/walk/step.rs`: `>> 5` for `>> 6` in the velocity vector (double speed) | step bound |
| `path/footprint.rs` `try_move`: the old footprint not cleared | grid reference (bit without an owner) |

Branch coverage of the generator, one default-count run (temporary
counters, not committed): units changed cell in ~650 ticks, 14 room
changes, every warp placed (`Some(true)`), every monster placement found
a box, walk outcomes `Moving(1..34)` and `Neutral`.

## 5. Hunts

- Release, 400 cases (first op mix): one failure, counterexample 5 (test
  side).
- Release, 600 cases (with `WalkNear`): one failure, counterexample 6
  (§8 quirk, PWQ1).
- Release, 600 cases after the exemption: clean (71 s).
- Release, 2,500 cases: clean (283 s).

Nightly: `tools/props-deep.sh` runs every d2-sim `prop_*` binary at 20,000
debug cases in its `sim` group; this file takes ~2 s per debug case, so it
is moved to a group of its own, `wiredpath` (300 cases, ≈ 11 min), and
`.github/workflows/nightly-props.yml` gains that matrix entry.

## 6. Not covered, questions

1. Monster movement: AI path steps stay `Pending` (`wire-path-sim.md` §5),
   so monsters never move here; the step and room properties run on
   players only in practice.
2. Missiles (`pathing.md` OQ3: no missile path function), knockback,
   items and objects with footprints, corpse footprints (§5.3 r3),
   removals other than room deactivation, warps across acts (stay
   `Pending::warp`) and warp tiles (§12, no lvlwarp record in the
   fixture).
3. The two spec points of §3 rows 3 and 4 decide whether the reference
   can drop its exclusions: the stamp room of the teleport (RE:
   `0x00650910` → `0x0064EFA0`) and the compress / restore of units in a
   deactivated room (room-lifecycle spec).
4. `wire-action` W5 / `wire-worldgen` WG7 are settled for population with
   the provider on (PWP1); without the provider the one-cell read stays
   (no spec reason to keep it; left for the provider-off hosts' owner).

## 7. Changes outside the new files

- `crates/d2-sim/src/wiring/worldgen/population.rs`: PWP1 (two adapter
  methods, provider-gated). No signature change.
- `tools/props-deep.sh`, `.github/workflows/nightly-props.yml`: the
  `wiredpath` group (§5); the `sim` group's filter excludes
  `prop_wired_path`.

## 8. Gate

`sh tools/gate.sh all` on this branch after the merge of `6cd6480`
(2026-10-06, `cargo nextest` installed): **GATE: PASS** — spec_index,
methods, coverage `--check` / `--selftest`, trace checkers, pre-commit
selftest, fmt, depcheck (+determinism), workspace clippy, d2-sim +
conformance (2,042 passed, 107 skipped), the other crates (599 passed),
d2-client (363 passed), doc-tests.
