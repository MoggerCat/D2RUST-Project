# cov-world-quests (batch 7)

Combined note. Part A = world/*, part B = drlg/render. Both were done by subagents and merged onto specs-staging-7 bf602bf.

# Coverage batch 7: world quests / hirelings / vendors / waypoints / npc

Base bf602bf. Unit-claim "any" counts via `tools/coverage.py` (uncovered before -> after, quests-status and vendors-2 §10 excluded from the work).

| Spec | uncovered before | after |
|---|---|---|
| hirelings-2 | 6 | 5 |
| hirelings-ai | 2 | 1 |
| hirelings | 23 | 13 |
| npc | 3 | 2 |
| quests-act1-rest | 11 | 7 |
| quests-act1 | 2 | 0 |
| quests-act2-2 | 13 | 7 |
| quests-act2 | 9 | 4 |
| quests-act3-2 | 2 | 1 |
| quests-act3 | 2 | 1 |
| quests-act4 | 5 | 3 |
| quests-act5-2 | 5 | 3 |
| quests-act5 | 5 | 3 |
| quests-helpers | 10 | 2 |
| quests | 24 | 18 |
| vendors-2 | 24 | 8 |
| vendors | 6 | 3 |
| waypoints | 14 | 9 |

(Before = list at the task start; some rules were also covered meanwhile by the rebase onto bf602bf's exemptions, which remove non-behavior rules from the count.)

## Code fixes found by tests
- waypoints `travel`: level 0 or the object's own level now only closes (spec §7 r2, edge r6); the old test asserted a warp. Test corrected.
- Item decoder (`items::bitstream::read`): compact records get ilvl 1, quality 2, seed 0, suffix slot 0 (tsc 0 / isc 1); full-record ilvl < 1 reads as 1; unique index >= uniques count is -1 (vendors-2 §7.3.1 r4, r5).
- `item_from_record` (copy / load): rebuilds weapon base speed and damage (quality 1 3/4 with floors, ethereal 3/2), armor block and speed, stat 17/18 raise-to-column, stat 57 sets poison_count (vendors-2 §7.3.1 r1-r3). Previously copies had no base damage.
- New `QuestControl::not_intro_test` (`0x005444B0`, quests §2.3 r4); exemption of §2.3 r4 removed.
- vendors repair handler result: upstream bf602bf already resolved it (routine result), my change dropped in the rebase.

## Left, with reason
- Narration / provenance / pointers / dead code (no behavior): hirelings §6 r8, §10 r9, §11 r8, §14, hirelings-2 §12 style items, npc §10 (dead code), act files' edge-case "no caller" notes (act3 r17, act4 r18), waypoints §9, §10, edge text, §8 r1.
- Conflict with existing claimed rule: waypoints edge r7 (says busy operate does not set the bit; §5.2 r4 and its test say it does).
- npc edge r13 (personalize with failed duplicate): spec itself flags the d2rs policy as "to reconcile"; the existing test asserts the one-0x58 policy.
- Need wiring not present (bucket C): quests §9.1/§9.2 reward item creation / delete (seams unwired), §9.6 object init 46 / 59 and operate 43 (no spawn/fit-A/placement seams), act1-rest §9 r12 refresh-room (needs DRLG fixture), act1-rest §8 r1 (+0x38 not kept), hirelings §6 r3 / §10 r8 / edge r8 (no callers: act change, save restore, hire-list slot marking), hirelings §8 r5/r6, edge r10/r13, vendors §1 r6 (no-record paths unreachable by types), vendors-2 §7.3.1 r6 text, act2-2 §1 r11/r14/r15/r16, §2 text, act2 §5.9 r1/r3/l2 r1 (speed fields not modelled), §6.10, act5 §1.4 table, act5 r7/r11, act5-2 r10/r12/r14, act4 r19/r21, act1-rest §4.1/§9 r3/r6, helpers §8 r1/edge r2.
- vendors-2 §10: other session.

## Notes
- Claims on exempt rules removed (§8 text, §5.3 text, §3 r3); `tools/coverage.py --check` passes.
- Pre-existing failures at this base, unrelated: `world::objects::tests::{routes_match_function_table,route_check_catches_perturbations}`.

---

# Coverage batch 7: specs/drlg/*.md and specs/render/*.md (not overlay.md)

Scope: raise `tools/coverage.py` "unit" coverage for the DRLG and render
specs (overlay.md excluded: another session). The triage doc
`docs/handoff/uncovered-rules-sort.md` did not exist in the base commit, so
the rule list was triaged from `tools/coverage.py` and the spec text.

## Rules covered (unit tier, before -> after, `tools/coverage.py`)

| Spec (rules, unit-tier covered, base bf602bf) | Before | After | Rules now claimed |
|---|---|---|---|
| drlg/wall-remap.md (6) | 4 | 6 | §edge r1, §edge r2 (§2 r4 is exempt: claim dropped) |
| drlg/levels.md (74) | 59 | 68 | §3 r7, §6 r2, §10 r6, §10 r7, §11.2 r1, §11.2 r3, §11.3 r2, §11.6 r2, §11.6 r3 |
| drlg/outdoor.md (82) | 78 | 79 | §12.2 |
| drlg/rooms.md (102) | 92 | 93 | §4.6 r11 |
| render/blend-modes.md (22) | 20 | 22 | §edge r7, §edge r8 |
| render/draw-order-2.md (53) | 43 | 50 | §11.3 text, §11.9 r1-r3, §15.1 r3-r5 (§11.9 text is exempt) |
| render/draw-order.md (24) | 19 | 20 | §5 r4 |
| render/unit-composite.md (43) | 33 | 36 | §2 r4, §3 r2, §9 |

(Render tests live in `d2-client::rules`; DRLG tests in `d2-sim::drlg`.)

## Code fixes found by the tests

1. (Superseded at rebase: bf602bf already fixed this with `BuildCursor`; my fix was dropped, only the r11 test remains.) `d2-sim` `Drlg::client_build_timer` (`drlg/rooms.md` §4.6 r9, r11): the
   cursor C "none" and the status-2 list's head node were one state
   (`None`), so a walk that ended on the head restarted at the first room
   next time instead of keeping the head (rule 6 reads the head's status 2).
   Added `Drlg::build_cursor_on_head`; rule 6 keeps a head cursor, rule 11
   clears C when its room is freed (`free_room`). The existing test
   `client_build_timer_builds_status_2_rooms_one_per_run_out` asserted the
   old end state (`C = first room` after a no-build walk); changed to the
   rule 9 state (C = head), which the spec states.
2. `d2-sim` `Drlg::spawn_room` (`drlg/levels.md` §10 r6): on the `Position`
   != 0 path a waypoint room with no waypoint object returned the room
   centre; the spec leaves (x, y) at (-1, -1) and the centre default runs
   only on the `Position` = 0 path. Fixed.
3. `d2-client` `rules::draw_order::sight` (`render/draw-order-2.md` §15.1
   r3, r5): sizes were `u32` (a monster's `SizeX` is signed, negative values
   are kept, only values above 2 are capped), and the end-pulling took the
   "+" branch when the two coordinates were equal (spec: "ax < bx" is the
   only "+" case). `SightUnit::size` is now `i32`; pulling matches the spec.
4. `d2-client` `rules::draw_order::weather` (`render/draw-order-2.md` §11.9):
   the particle move `0x004732C0` was an "Open question 3" error; it is
   now implemented from the spec (new `Weather::move_counter`, the `F`
   counter). PROVISIONAL: the move runs only when particles are live
   (§11.2 r3), so `F` counts those calls; §11.9 r3 says "even with an empty
   pool". Marked `// PROVISIONAL (render/draw-order-2.md §11.2 r3 vs §11.9
   r3; REC-??)`; no REC number exists in the repo yet, so the HANDOFF §7
   recording list needs a row (weather capture with rain / snow state per
   frame, `capture.md` §3.4). The test
   `moving_live_particles_is_open_question_3` was replaced by
   `live_particles_move_before_the_rain_cycle`.

## Rules left, with reason

Narration / structure (no behavior to test): drlg/levels §1 (record layout),
§3 text, §10 text, §11 text, §11.2 text, §11.3 text, §edge r3, r4, r7 (r4's
"loop runs out" half cannot occur; an existing test checks the first half
without a claim, left as the earlier author left it), §11.6 r1, r4
(population order consequences, owned by `monsters/population.md`);
drlg/maze §edge r9; drlg/outdoor §3 text, §7 text, §7.5 text;
drlg/outdoor-act3-act5 §rules text, §5 text, §4 r3 (dependent on act 3
placement end to end), §edge r3, r6; drlg/outdoor-tilesub §2.1 (callback
table of the original), §edge r4 (reads past a table);
drlg/preset §1 (record layout), §2 r3 (load filter, `data/loading.md`),
§3.1 r4 (level size code runs in the `Drlg` allocation, not in the preset
unit), §3.2 r4 (automap, client only), §5.2 r4, r7, r9, r11 (byte-level DS1
walk: the sim's `Ds1Input` is the decoded file, those rules belong to
`d2-formats`), §6 text, §12 (pops at run time, presentation), §13 (column
index), §edge r3 (unchecked reads);
drlg/rooms §1 (layout), §2 r5 (type data allocators are not modelled),
§4.6 text, r3 (statistics: "not modelled" by the spec), r10 (level-free
counter is a client game global with no code), §5 r8 (client act callback,
`render/lighting.md` §6.4), §6 r4 (consumer order), §8 r4 (needs the unit
flag/path update of `0x0061A840`: `UnitLists::free_room` does not own unit
records, wiring needed; the existing TODO stays), §8 r5 (per-type exits of
tick step 9), §9.2 text and §9.2 l2 r1/r2 (tile free, no counterpart),
§9.8, §9.9 (recorded vectors: need the trace), §edge r3, r4 (unreachable /
address-sorted client arrays);
render/capture §1, §2, composition §1 text/r1/r2/§7, map-preview §edge
(text, r2): reference-renderer configuration, recorder hooks and
simplification notes, not behavior of d2rs code;
render/draw-order §3 text, §5 text, §5 r2 (perspective test not in the
reference renderer), §7 (spread over several tests, no single check),
§edge (list of earlier rules);
render/draw-order-2 §15.1 text, r1, r2, r6 (null-unit fatal, path-kind
position read and the generalised mask belong to the unit side; the
collision-line seam provider for arbitrary masks is not small);
render/lighting §1 r3 (`LightMap::build` has no per-frame caller yet), §2
r4 (q is an input of `build`; computed by the caller), §12 r4, r5
(recorded observations);
render/sprite-placement §6 (index 0 = transparent is the frame decoders'),
§edge r2 (dead branch);
render/unit-composite §1 text, §3 text, r1 (direction sources are
`ClientUnit` path fields not in the rules module), §5 text, r3, r4
(overlays: owned by `render/overlay.md`, another session), §8 text, §10
(mapping table), §edge (list of earlier rules).
