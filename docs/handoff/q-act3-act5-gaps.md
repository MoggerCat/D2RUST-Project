# q-act3-act5-gaps

Links connected (REC-246, all `d2rs-own, unverified`):
- **Quest-item use** (`ass` +1 skill point, `xyz` +20 life, `tr2`): C→S 0x20 reached `inventory-moves.md` §7.11 step 4, but the preview rest answered every quest flag as clear. `WiredWorld::moves` now stages the players' flag records into `PreviewMoveRest` and writes the changes back; used quest items leave the grid (`wiring/inventory/pending.rs`). Test: `crates/test-fixtures/tests/quest_item_use.rs`.
- **Act III / V waypoints**: synthetic chain levels carry their `waypoints.tsv` indexes (`CHAIN_WAYPOINTS`; Harrogath keeps 35 for the Act IV portal); Kurast Docks has a waypoint object. Test: `app_a3_a5_waypoints.rs` (0x13 + 0x49 to Spider Forest, Rigid Highlands).
- **Arreat Summit**: statues 474-476 and the altar are host presets in level 120 (`synthetic_act5.rs`), their object rows use the quest init/operate functions; the exits to 118/128 close (`Pending::set_summit_open`, `warp_quest_gate`) once the altar was operated and the Ancients live. Test: `app_a5_ancients.rs`.
- Test expectation changed: `app_single_player` seed test gets one more step for the new Kurast waypoint allocation.

Left: the `5D` message after a quest-item use (chain unknown), the summit door 564, invisible Ancient 561, the Ancients fight (540-542). The gate is not spec-exact (closed only after the altar).

Local check: `cargo run -p d2-client --release -- play --new sorceress Test`; Act III/V need an act change (Warriv/Tyrael/portal), then click the town waypoint and pick a field; nothing is rejected in the log.
