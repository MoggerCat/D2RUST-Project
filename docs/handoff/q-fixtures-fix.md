# q-fixtures-fix: two test-fixtures regressions

Both failures were stale expectations, not code bugs. Only tests changed.

1. `e2e_night_flows::using_a_shrine`: q-doors (`ShrineWorld::create_hover`) gave
   the hover seam a provider. The test still said "no hover is created".
   `specs/world/objects.md` §9.1 r3 says the hover is created and event 6 is
   queued at frame + 300; `intents-events.md` §7.9 r3 / §7.1 gives the 0x26
   form 5 message on creation and 0x76 on expiry. The test now asserts the
   timers `[(5, f+1201), (6, f+300)]`, the 0x26 bytes (text `"%d"`(3683+id)),
   and the 0x76 at frame f+300. No assertion weakened.
2. `synthetic_game::a_full_save_loads_before_the_join_sequence`: q-save-full
   applies waypoints (`ActionCharacter::set_waypoints`, `world/waypoints.md`
   §3), so "waypoints" is no longer an unapplied step. The expected list drops
   it; the other five steps and their order are unchanged.

Gate: fmt, clippy -D warnings (test-fixtures), nextest d2-sim/d2-server/d2-client/test-fixtures (6673 passed), `coverage.py --check` (0 errors).
No PROVISIONAL points; nothing local to check.
