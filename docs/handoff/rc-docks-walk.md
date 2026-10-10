# rc-docks-walk (REC-2045)

Task: the server refuses a walk north of y 5082 from (5139, 5087) in Kurast Docks, so `app_support::approach`
never reaches Cain.

## Cause: not d2-sim
- The walk legs came from `test_fixtures::host::route_near`, a BFS over single free cells. Its leg target
  (5141, 5075) lies next to the wall (cells x <= 5140 carry 0x7/0x400). The player's move pattern is the plus
  (pattern 1), so the target collides; target preparation (`pathing.md` §3 step 7) returns 0, no path. Nothing
  was "refused": the order had no walkable target.
- The direct order to Cain (23 sub-tiles away, d² > 324) runs Toward only (§6); it stops at the wall. A* has the
  200-node cap, read in 1.14d `0x0067B850` (`FUN_0067b850`: 200 test, best-node rule +5 on g): equal to
  `path/walk/find.rs::astar`. Both behaviours are the original's.
- Walkable column on the Docks wall's east side starts at x = 5142.

## Changed
- `crates/test-fixtures/src/host.rs`: `search` routes first with plus clearance (cell and 4 neighbours free,
  as the player's pattern), and without it only when that reaches nothing.
- `crates/d2-client/tests/app_play_act3.rs`: new real-data test `the_walk_from_the_docks_arrival_reaches_cain`
  (walks from the arrival to Cain with `approach`, no `goto preset`).
- `specs/sim/pathing.md` Edge cases 2a: the behaviour and the address.

## Checks
- app_play_act3 (real data): before 6 pass / 2 fail (Golden Bird, Blade: host calls with no provider, not this
  task); after: 7 pass + the new test passes, same 2 fail. `test-fixtures` 95 pass.
- No 1.14d scenario compares this walk: ledger row state NO-CHECK (queue: a recorded Act III walk, trace
  `walk-town-ama`-style, for Docks).

## Open
- Golden Bird `sound 0 72`, Blade host calls: owners in `q-fix-rd-act3.md` Open 1 (S each).
