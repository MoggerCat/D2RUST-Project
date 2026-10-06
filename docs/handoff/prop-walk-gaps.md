# Handoff: property tests for the prop-walk gaps — `claude/prop-walk-gaps`

Cloud test session, 2026-10-06. Task: `docs/HANDOFF.md` §2 step 7t(b),
the gaps listed in `docs/handoff/prop-walk.md` §4 except missiles (not
part of this task). Task class: property tests from the specs, medium
(METHODS M14). Base: `claude/tender-meitner-mphas3` at `01dff69`. Repo
only, synthetic worlds, no game files (M09). Inputs: `specs/sim/pathing.md`
§1.5, §2, §3, §8.1, §9; `specs/sim/path-placement.md` §3–§6;
`docs/handoff/prop-walk.md`; `crates/`. Only new test files; no change to
non-test code (no property found a code bug). The parallel session
`prop-wired-path` owns the wired-path property files; none of them is
touched.

## 1. State

Four new files under `crates/d2-sim/tests/`, every model and fake written
from the specs, public API only, same conventions as `prop_walk.rs` (a
default case count per property, `PROPTEST_CASES` overrides it, no
failure persistence).

- `walk_rooms_fake/mod.rs`: the shared multi-room fake (`CollisionRooms`,
  `PathWorld`, `WalkUnits`, `PathMotion` on one `World`). Rooms tile a
  grid of `rx` × `ry` rooms, a slot may be a hole; adjacency arrays are
  the 8-neighbour rooms or every room, rotated so their order varies.
  With either, any cell within two cells of a room's cell is found from
  that room, so the reference views (one lookup over the union) equal
  every reading of the unstated lookup room of `path-placement.md` §5.1
  and §6 rule 4 (the two `footprint.rs` TODOs). Seams record their calls
  (`Ev`: room list remove / insert, update queue, unit flag, AI room
  memo, add / removal messages). `PathMotion` goes through
  `Walk::set_position` / `Walk::reset`, as the wiring does. Shared
  models: §9.8 messages as set differences, §9.6 rules 8–9 set position,
  §9.5 unit distance.

| File | Property | Model (spec) |
|---|---|---|
| `prop_walk_rooms.rs` | `multi_room_walk_tracks_rooms_and_messages` | 2–6 rooms, random walls, a player (0x01 / 0x03 request) or a monster (type 2 compute) walking anywhere. §3 step 10: flag 0x1 iff a live point is outside the path room. Every tick: the path's room holds the unit's cell (§9.6 r9), the cell is free; room-list removes and inserts keep the unit in exactly that room, one queue per insert; on a room change prev room = old room, the messages equal the §9.8 model (own client skipped, random client arrays and owners) and a monster's AI memo is cleared once; no message otherwise; flag 0x2 clear after the step. End: cell centre, on the last point unless refused or exhausted, grids = initial + one footprint at the unit |
| | `room_change_messages_merge` | §9.8 directly: random sorted client arrays, client players, prev / current room (or none), flag 0x2 set or not, player or monster |
| | `set_position_recaches_by_spec` | §9.6 r8–r9: 1–5 rooms with random rects (overlap, gaps, inactive), random partial adjacency arrays with unknown ids, random room / prev room / hint / Q (any fraction) / flags / count, missile or not; record and seam calls equal the reference |
| `prop_walk_motion.rs` | `knockback_keeps_the_distance_budget` | Mode 19 request (§1.2, §1.5 step 2): type 8, previous type 7, saved velocity = the old velocity, budget 5, velocity 0x1000 (§8.1 r1), type flags from the table; knockback function = the seam (open question 3), a straight line of 1–7 cells. Walked against a control copy whose type is 7: identical cells every tick, type 8 keeps budget 5, type 7 loses one per tick that crossed or tried to cross a cell (§9.6 r4), down to 0. No stamina drain (§9.2 step 3) |
| | `set_path_type_follows_the_table` | §2 type set on random records (flags, type, previous type, velocities, max distance) and 1–11 random (player, type 0..19) sets: fields equal the model; fatal asserts (no row, player type 2, previous 8 / 11, type 4 with max distance ≥ 78) agree |
| | `chase_a_moving_target` | Monster (type 2) or player (0x02 request, type 7) chasing a player / monster target of size 0–3 that steps, jumps, vanishes or is replaced by another unit with its GUID; stop distance 0–3, budget 0 or 1–3. Per tick: stale target dropped (§9.2 r1); unit distance ≤ stop → stopped, no re-path; target moved > 5 from the previous target → budget 0: stopped, nothing queued, type unchanged; else unit flag 1 and one queue, budget = (b − index) then the crossing decrement, type 13 (monster, moving) / 13 or 15 (stopped) / 7 (player), previous target = the target unless §4 prepared it; no move → no re-path, budget per §9.6 r4 |
| `prop_path_footprint_ops.rs` | `set_foot_mask_restamps` | §5.3 r1 (with §5.1): 3 × 2 rooms with a possible hole, random cell bits, any pattern 0–6 / size −1..4, old / new masks (0 included), path room = the cell's, another or none, missile (size) or not |
| | `corpse_footprint_is_a_plus_of_0x8000` | §5.3 r3: forced remove with the old pattern and mask, pattern 5, mask 0x8000 through r1 |
| | `teleport_follows_the_spec` | §6 r4: fatal assert (non-zero point, no room) leaves everything unchanged; missile: (0, 0) clears (size), else flag 0x8 iff the cell changes, collided mask = size query at the new point before the move (§4 r3 centre rule, r4 box strips), size move, saved step; others: clear or forced move; flag 0x1 iff the room differs; set position with the destination room as hint and the reset (§9.7); `teleport_and_clear` clears the count |

Model errors found and fixed in the tests (no code change): a player's
re-path prepares a blocked target (§4, type 7 has 0x1000), so its
previous target is the prepared point; a plus / point query whose centre
has no room gives 0x27 alone (§4 r3), not 0x27 ORed with its neighbours;
a box query must follow the strip clipping of §4 r4 next to a hole.

Branch coverage (temporary counters, one default run, not committed):
multi-room 125 of 256 walks moved (46 walk, 37 run, 42 monster), 63
room changes; chase: every arrival branch reached (stop distance,
moved with and without budget, re-path moving and stopped); knockback
cases both reach the end and are refused by walls.

## 2. M08

Each mutant applied to the code alone, the new test file run, the code
restored (`/tmp` script, not committed):

| Mutant (code) | Caught by |
|---|---|
| room recache: no update queue after the insert | `multi_room_walk_…`, `set_position_recaches_by_spec` |
| §9.8: add messages also to the unit's own client | `multi_room_walk_…`, `room_change_messages_merge` |
| §9.8: monster AI room memo not cleared | same two |
| room recache: the hint is ignored | `set_position_recaches_by_spec` |
| §3 step 10: room-exit flag never set | `multi_room_walk_…` |
| §9.6 r4: knockback (type 8) budget decremented | `knockback_keeps_the_distance_budget` |
| §9.10: unit flag 1 not set | `chase_a_moving_target` |
| §9.10: budget −= index dropped | same |
| §9.5 r3: moved threshold ≥ 5 instead of > 5 | same |
| §9.5 r3: stop distance < instead of ≤ | same |
| §9.2 r1: stale target kept | same |
| §9.10: finish re-path sets type 2 instead of 13 | same |
| §5.3 r1: missile mask change by pattern instead of size | `set_foot_mask_restamps` |
| §5.3 r3: corpse pattern 1 instead of 5 | `corpse_footprint_is_a_plus_of_0x8000` |
| §6 r4: flag 0x1 not set on a room change | `teleport_follows_the_spec` |
| §6 r4: flag 0x8 always set | same (survived the first run: a teleport onto its own cell was too rare; the generator now picks it 15 % of the time) |
| §6 r4: missile query after the move | same |

## 3. Hunt

`PROPTEST_CASES=30000 cargo test --release -p d2-sim --test prop_walk_rooms
--test prop_walk_motion --test prop_path_footprint_ops`: all pass (≈ 1.5 s
per file). Default counts in debug: all three files under a second.

## 4. Claims

`// Covers:` lines on the properties, only for the rules each one checks:
`pathing.md` §1.5 r2, §2, §3 r10, §9.2 r1, §9.2 r3, §9.5 r3, §9.6 r4,
§9.6 r8, §9.6 r9, §9.7, §9.8, §9.10; `path-placement.md` §5.1, §5.3 r1,
§5.3 r3, §6 r4.

## 5. Open questions / notes for the coordinator

- PG1 (spec reading, not a bug): with the walk as specified, the
  arrival check's "index ≥ count" branches (§9.5 r2 re-path with finish
  0, r3's final re-path) are unreachable from §9.4: the step that makes
  index = count also resets the path (count := 0) in the same tick, and
  a blocked step sets index := count then resets too. They can only run
  on a path whose index is set by other code (monster AI). Worth a line
  in `pathing.md` §9.5 when the AI spec arrives.
- PG2: §6 r4 lists the missile's collided-mask query before the
  footprint move; the code and the model follow that order. The order is
  observable when the old and new footprints overlap and the move mask
  meets the footprint mask; confirm when the spec session looks at
  `0x00650910` again.
- Not covered here: missiles through the walk (§6 r3, §9.4 missile
  branches; not in this task), type 11, monster circling (types 5, 6),
  the flag-0x10 monster re-path on a blocked cell (`one_step`) is covered
  only by the existing unit tests.
- No local game-file check is needed (synthetic worlds only).

## 6. Gate

`sh tools/gate.sh --no-client` on `d53c4fb`'s tree: **GATE: PASS** (11
steps; d2-sim + conformance 1,885 tests, the rest 599; coverage 4,143
claims, 0 errors). The full `sh tools/gate.sh` fails only its three
`d2-client` steps, which cannot build here: `wayland-sys`'s build script
finds no `wayland-client` through pkg-config in this container (an
environment gap, no client file is touched). The coordinator's full gate
with the client should pass unchanged.
