# q-act-travel: act change (waypoint and Warriv)

Stitching session `q-act-travel`, branch `claude/q-act-travel`.

## Links connected

| Link | Before | Now |
|---|---|---|
| Waypoint 0x49 to another act | `WaypointView::warp` → `level_warp` returned `None` → logging `LocalSeams::warp` | `wiring/path/act_change.rs::run` (called from `WaypointView::warp`) |
| Warriv / Meshif / Cain act change | `NpcWorld::act_change` only logged in `rest.rs` | `Desk::act_change` also calls `LifecycleHooks::request_act_change` → `ActionHooks::act_changes`; `WiredWorld::act_changes` (d2-server `hireling_host.rs`, run first in `hireling_calls`, i.e. after every handler and tick) runs the level warp / act change |
| Messages | none | old act's removals, 0x05, 0x03 (seed, town, object seed), placement (0x07, adds, 0x15 / 0x0D from the update pass), then 0x59 + 0x0B (local player back) and 0x04 |
| Client | 0x03 handler existed | palette act and tiles follow from 0x03 (`palette_act`, client DRLG of the act); `in_game` back on through 0x04 |
| Waypoint panel act tabs | tab 0 only | every tab opens (`ui/waypoint_ui.rs`) |

Tests: `crates/d2-client/tests/app_act_travel.rs` (waypoint to Lut Gholein, and the queued NPC act change; the client ends in level 40, act 1, `in_game`, nothing rejected; both fail before the change: the client stays unloaded without a player), `d2-server` `an_npc_act_change_is_queued_for_the_host`.

## PROVISIONAL

REC-122 (`docs/HANDOFF.md` §7).

## Left

- The quest side effects around Warriv's change (act completion, waypoint activation) are the existing quest calls; the `act_change` argument (0 / 5) is not used.
- No Warriv NPC in the synthetic world: the NPC link is tested through the hook queue, not by clicking him.
- Panel tab gating and row names (string table).
- The pets / hireling follow after the act change runs through the existing `HirelingCall::ActChange` queue for the waypoint path only.

## The user's local check (game files)

```
cargo run -p d2-client --release -- play --new sorceress Test
```
Click the town waypoint, click the Act II tab (only rows of known waypoints are active; Lut Gholein is not known on a new character, so use a save that has it, or kill Andariel and talk to Warriv, "Go East"). Expect the screen to switch to Lut Gholein with Act II tiles and palette, the character standing in the town, no `rejected` lines in the log. Record the console lines if the character is missing or the screen stays black.
