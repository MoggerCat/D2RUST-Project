# fix-scenario-wp: scenario travel() gets a real waypoint destination

Test: `scenario-run::scenarios server_messages_are_captured_at_their_tick`.
Cause: `waypoints.md` §7 r2 / edge case 6 (4eb119b0): travel to level 0 or
to the waypoint object's own level only closes the menu. The scenario
travelled town -> town, so no warp and no 0x0D reply.

Fix (fixtures only; d2-sim travel code untouched):
- `test-fixtures/src/content.rs` `world()`: level 4 (Synth Keep, act 0, preset
  level, Def 1) gets levels `Waypoint` index 2 (was 255).
- `test-fixtures/src/drlg.rs`: new `keep()` preset = the one-room preset plus
  the waypoint object (same record as the town's); level 4's DS1 uses it.
- `tools/scenario-run/tests/scenarios.rs` `travel()`: `level=3` -> `level=4`
  (was `level=1`). Same assertions (S2C 0x0D at tick 50, client 0).
- No golden/trace files changed: the checked-in scenario file is untouched
  and the test builds its trace at run time.

Checks: see the head commit message / coordinator's workspace gate. Per the
coordinator's new deadline only `scenario-run` tests and clippy on touched
crates were required of this session; the workspace gate is the
coordinator's.

## Status: NOT green (one test still fails)

Also changed: `tools/scenario-run/src/lib.rs` generates the other act 0
levels the character holds a waypoint of (only levels with a waypoint
index) and `travel()` also rewrites `char waypoint 3` -> `4`.

Result: `server_messages_are_captured_at_their_tick` still fails. Tick 50
now carries 0x07 and 0x15 (the warp happens; `level_warp_place` returns
Ok(true)), but no 0x0D. In `waypoints.rs` `travel`, `world.player().room`
is RoomId(0) (the town room) right after the warp, while `spawn_room(4, 0)`
is RoomId(1), so the §7 r7 arrival condition fails. Likely the player's
room entry in the unit list is only updated by the next tick's room switch
in this host (the d2-server tests' fake/wired hosts update it at once).
Not investigated further (deadline). Options: have the scenario host
update the player's room on placement, or assert the 0x15 reveal at tick 50
(that would be a weaker check, so not done). Other tests' suites
(test-fixtures, d2-server, d2-client) were NOT re-run after the level 4
waypoint/preset change: the coordinator's workspace gate must.
