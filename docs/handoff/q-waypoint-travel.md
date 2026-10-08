# q-waypoint-travel (rows q-waypoint-travel and q-wp-warp)

Finding: the waypoint warp is already connected in staging. The path provider is present in the play host, so `WaypointView::warp` (`d2-sim/src/wiring/action/waypoints.rs`) runs `path::place::level_warp` (same act) and only an act change reaches the logging `LocalSeams::warp`. No sim change was needed.

## What was added
- Test `crates/d2-client/tests/app_waypoint_warp.rs`: operate the town waypoint (C→S 0x13), send C→S 0x49 [GUID][level]; the server player ends in the target level and the client model follows (no rejected message). Two cases: Cold Plains (built at creation) and Stony Field (not built; asserted unbuilt before, built on arrival).
- Synthetic Stony Field (`STONY_FIELD`, waypoint index 2) in `single_player.rs`. The waypoint panel now lists it as an unknown row, so `app_play_npc.rs` expects `(4, false, false)` as a third row (the panel spec lists unknown rows).

## PROVISIONAL
REC-112 (`docs/HANDOFF.md` §7).

## Left
- Cross-act travel (act change) still goes to `LocalSeams::warp` (log only).
- Field waypoint activation is covered by the sim tests; no synthetic field waypoint object was placed in play (the test sets the bit directly).

## Local check
`cargo run -p d2-client --release -- play --new sorceress Test`, click the town waypoint, choose Cold Plains: the screen switches to that level, no `rejected` line. Tab switching and row names are still missing (stitch-npc2 item 4).
