# Coverage batch 7: specs/drlg/*.md and specs/render/*.md (not overlay.md)

Scope: raise `tools/coverage.py` "unit" coverage for the DRLG and render
specs (overlay.md excluded: another session). The triage doc
`docs/handoff/uncovered-rules-sort.md` did not exist in the base commit, so
the rule list was triaged from `tools/coverage.py` and the spec text.

## Rules covered (unit tier, before -> after, `tools/coverage.py`)

| Spec | Before | After | Rules now claimed |
|---|---|---|---|
| drlg/wall-remap.md | 4 / 7 | 7 / 7 | §2 r4, §edge r1, §edge r2 |
| drlg/levels.md | 59 / 80 | 68 / 80 | §3 r7, §6 r2, §10 r6, §10 r7, §11.2 r1, §11.2 r3, §11.3 r2, §11.6 r2, §11.6 r3 |
| drlg/outdoor.md | 78 / 82 | 79 / 82 | §12.2 |
| drlg/rooms.md | 90 / 108 | 92 / 108 | §4.6 r9, §4.6 r11 |
| render/blend-modes.md | 20 / 22 | 22 / 22 | §edge r7, §edge r8 |
| render/draw-order-2.md | 43 / 55 | 51 / 55 | §11.3 text, §11.9 text, §11.9 r1-r3, §15.1 r3-r5 |
| render/draw-order.md | 19 / 25 | 20 / 25 | §5 r4 |
| render/unit-composite.md | 33 / 45 | 36 / 45 | §2 r4, §3 r2, §9 |

(Render tests live in `d2-client::rules`; DRLG tests in `d2-sim::drlg`.)

## Code fixes found by the tests

1. `d2-sim` `Drlg::client_build_timer` (`drlg/rooms.md` §4.6 r9, r11): the
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
