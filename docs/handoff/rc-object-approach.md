# rc-object-approach: hand-back (branch `claude/rc-object-approach`)

## Checks (gen-obj-*, `--ignore q` as rc-object-operate; state is PARTIAL by construction, items and rng are the hard channels)
- Before (rc-object-operate hand-back): gen-obj-267, -385, -194 DIVERGED (state), rng MATCH. First divergences measured here
  on the merged tree: 267 frame 20 (player mode), 385 frame 20 (player), 194 frame 69 (NPC 1:7 class 155 mode).
- After: all three state 80/80 (385: 47/47, the 1.14d recording ends at tick 47) equal on every compared field, rng MATCH, items
  PARTIAL (none compared). Ledger: 3 rows in `ledger/rc-object-approach.tsv` (they override the DIVERGED rows of `q-run-objects`).
- Regression sample (the 20 rc-object-operate rows + 195 + 3): all 22 state 80/80 equal, rng MATCH.

## Root causes (two, both read from 1.14d)
1. 267 / 385: `poke operate @2:267` / `@2:385` resolve to the town stash (GUID 17) and the urn GUID 2, not the object the poke
   created; 1.14d `0x00548B00` case 2 walks the player (`0x00548A50`, run mode 3 to the object) when `0x00623660` says out of range
   or the line test `0x00622B50(P, O, 0x804)` is blocked, and operates on arrival. d2rs used the d2rs-own preview range
   (`object_preview_range`) and operated at once.
2. 194 / 385: the walk-in-radius point of town NPCs (`0x005DE4E0`, used by `npc_interaction_think` and 7 more AI call sites) used
   a "round to nearest" geometry marked PROVISIONAL (REC-501); `specs/monsters/ai.md` §7.2 already carries the 1.14d algorithm
   (proportional Manhattan split, `while kx+ky<k` fix-up, full-size distance, retreat sign). The stair/urn checks only exposed it
   (Warriv's 2nd walk after the player stands still). No footprint problem (the hand-back's suspicion was wrong).

## Changed
- `d2-sim` `View::object_reach` (new, `wiring/action/objects.rs`): unit distance > 50 -> 1, `in_reach_box` (`0x00623660`, spec
  `world/objects.md` §7.3), `units_line_blocked(.., 0x804)`; walk -> new `ObjectCase::Walk(object)`. Used when the path provider
  exists; `object_preview_range` / `object_approach` stay for hosts without it.
- `path_shape` now returns `UnitShape::Object` (objects.txt `SizeX`/`SizeY`): object unit size 0 -> `SizeX` for the unit
  distance, line test and arrival (1.14d `0x00620510`).
- `unit_distance` (`path/walk/geom.rs`): a negative `dist8_unit` entry returns 0 at once, no `+1` for size < 2 (spec §9.5; the
  stash run stops at Δ=(2,0), as recorded).
- `test-fixtures` `e2e_night_flows`: `Fx::object` stands the player beside the object (the real reach test would walk it).
- `d2-server` walk test `walk_and_run_to_a_unit_send_0x10`: the run to a unit now stops at distance 0 (cell 29), not on the object.
- `d2-server`: `WorldHost::object_walk`, `object_approach.rs` (run to the object via `approach_unit`, queued; the 0x13 object
  case repeats when the run ends; a new walk request drops it), cloned from the NPC / item approaches.
- `radius_point` = `0x005DE4E0` as in the spec: another session landed the same function on specs-staging-7 at the same time;
  the merge took that version (identical algorithm), verified by the three checks after the merge.
- Spec: `world/objects.md` §7.3 range test and the recorded stash walk.

## Open
- PROVISIONAL (REC-1930): the arrival of the object walk is read from the player's mode leaving walk/run/town walk at the end
  of tick step 4 (as the NPC/item approaches), not from the step result 2 of `0x00580C20`. gen-obj-267 is frame-exact with it.
- The walk flag (u32 @9 = -2) of the 0x13 message is not carried to the repeated case (poke / client send 0). Size S.
- The line test with the object on the player: moot here (the poke-created object is never the operate target).
