# q-wp-warp

No code change: the row was already done in staging.

- Same-act waypoint warp: `q-waypoint-travel` (test `crates/d2-client/tests/app_waypoint_warp.rs`, REC-112).
- Cross-act waypoint warp and the client level load: `q-act-travel` (test `app_act_travel.rs`, REC-122).
- Warp tiles / level load path: `q-warps` (test `app_level_warp.rs`, REC-99).

`LocalSeams::warp` (single_player.rs) is now only a fallback log. No PROVISIONAL note was added, so REC-169 is unused.

Local check: `cargo run -p d2-client --release -- play --new sorceress Test`, click the town waypoint, pick Cold Plains (same act) or a row on another act tab; the screen switches level with no `rejected` line.
