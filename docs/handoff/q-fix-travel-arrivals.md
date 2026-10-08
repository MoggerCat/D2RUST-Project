# q-fix-travel-arrivals: q-smoke-travel's open travel breaks B3–B5

Branch `claude/q-fix-travel-arrivals`. Repros: `crates/d2-client/tests/smoke_travel.rs`
(bridge + in-process server + sim, no window, synthetic game). Every
`#[ignore]` of B4 and B5 is gone; B2 (idle rooms lose their units) stays
ignored: q-fix-idle-rooms owns it.

## B5: Meshif not in the client model at Kurast Docks: already fixed

Cause: the synthetic Kurast Docks room had no Act III NPCs. q-smoke-town's
`town_npcs::act3_docks` (REC-278), merged into staging, places them
inside the room. `meshif_sails_back_west` passes unchanged; un-ignored.

## B4: the Kurast Docks town portal did not take the player back: test pick

Not an engine bug. The synthetic Spider Forest room borders the Kurast
Docks room, so after the trip to town both ends of the portal pair are in
the client model (the field portal sits in an active room of level 76 in
sight). `Rig::find` took the first class-59 object, the field portal 38
sub-tiles away; a C→S 0x13 to an object out of reach is answered with
nothing (correct). Acts I and V passed because their field end is not in
sight of the town. New `Rig::portal_here` takes the portal whose client
room is in the player's level. `a_town_portal_in_act_iii_…` un-ignored.

## B3: arrival walk-outs not drawn: fixed (PROVISIONAL, REC-288)

The path: server arrival (`path-placement.md` §12.2 r5–6, `objects.md`
§12 r11, `waypoints.md` §7 r7) → S→C 0x0D code 1 (target) then 0x15
(arrival point) → bridge mode request (`model.md` §8 r4: player code
0x01 = walk to (r0, r1); kept as `last_mode_request`) → walk prediction
(`bridge/predict.rs`) → view. The missing link was the last one: the
prediction only took the walks the client sent.

- `ClientUnit::mode_requests` (`bridge/world.rs`, bumped in
  `bridge/modes.rs`): d2rs bookkeeping so a new request is seen even when
  it repeats the last one.
- `Predict::server_walk`: a new code-0x01 request of the local player is
  held until the player's client room holds the target (the 0x0D comes
  before the 0x15 that moves the player into that level, so a target in
  another level is never stepped toward from the old position), then
  walked like a client walk. A walk the player sends drops a held one.
- `Predict::observe`: a level change the prediction did not walk into
  (another act, or the predicted cell outside the player's room) snaps to
  the model. Found by the new check: Fortress ↔ Harrogath both arrive at
  (23, 23), the model position did not change, and the player stayed
  drawn next to Tyrael / Cain in the old act. Walking across a level
  border (the view's room recache, `app_level_border`) is unchanged.

New smoke checks (each fails before the change): every arrival checks the
drawn player (`PreviewWalk::predict.cell()`) ends at the move's last 0x0D
code-1 target, else at the placement; every warp (all five acts) and every
Town Portal trip checks it ends on the server player. Unit tests in
`predict.rs`: `an_arrival_walk_out_starts_at_the_arrival_point`,
`a_walk_out_ahead_of_its_placement_is_held`,
`a_player_walk_or_another_request_drops_a_held_walk_out`,
`a_level_change_to_the_same_cell_moves_the_prediction`,
`walking_across_a_level_border_is_no_placement`.

PROVISIONAL (REC-288; spec line in `client/model.md` OQ2): that the walk
outlives the 0x15 placement (its teleport zeroes the path point count,
`path-placement.md` §6 r4) and the hold. Consequence: a **waypoint
arrival is drawn at x + 3, y + 3** (the 0x0D's offset, `waypoints.md`
edge case 5) while its server player stays at x, y, inside the
position-check tolerance (`model.md` §6 r4), so nothing corrects it. No
client rule built from the messages alone tells the waypoint 0x0D from
the warp one (same shape); the recording settles which one the original
draws.

## Also in this branch

- The q-smoke-travel merge (Warriv in Act I, REC-280) broke two newer
  staging tests; staging has since merged its own fix and this branch
  takes staging's versions.

## Left

- B2 (q-fix-idle-rooms).
- REC-288 capture: `record_frames.py` over a waypoint arrival, a cave
  warp and a Town Portal with REC-51's hooks (0x00463390 local path x/y
  per frame, the server position): does the client walk to the 0x0D
  target after the 0x15, and does a waypoint arrival step 3 sub-tiles?

## Local check

```
cargo nextest run -p d2-client --test smoke_travel
cargo nextest run -p d2-client --lib -E 'test(predict::)'
```

In `play` (synthetic or live): walk into a cave (Den of Evil) and take a
Town Portal: after arriving, the player walks a few steps out (warp: the
lvlwarp `ExitWalkX/Y`; portal: +5, +5) instead of standing on the
arrival point. Take Tyrael's or Cain's travel: the player appears at the
other town's arrival point, not where it stood in the old act. A
waypoint arrival steps 3 sub-tiles down the screen (provisional,
REC-288).
