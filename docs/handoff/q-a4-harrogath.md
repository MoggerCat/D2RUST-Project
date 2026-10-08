# q-a4-harrogath: Act IV to Act V in the play preview

Branch `claude/q-a4-harrogath`. Nothing is verified against 1.14d (rule 10). Open point: REC-231 (`docs/HANDOFF.md` §7).

## Links connected

| Link | Where |
|---|---|
| Fortress to Harrogath left the player in the Fortress room. Cause: the act change placed the unit while its path still named the old room; the room recache keeps the old room when the cell is inside its rectangle, and acts share tile coordinates (the spawn point (23, 23) is in both rooms). Fix: the act change takes the unit out of its old room first (footprint, room list, path room) | `d2-sim` `wiring/path/act_change.rs` (`leave_room`) |
| The portal's `0x005B4FF0` was reported unhandled | `wiring/economy/quest_host.rs` `activate_waypoint`: the level's waypoint index set in the host's record for the game difficulty; `single_player.rs` fills `ActionTables::levels` from the synthetic levels rows |

Test: `crates/d2-client/tests/app_a4_endgame.rs`, `the_portal_to_harrogath_refuses_then_asks_for_the_act_change` now runs on to Harrogath (level 109) and checks waypoint 35 is lit. It failed before the change (level stayed 103). Changed expectation: the old assertion that the log names `0x5b4ff0` is now its opposite, because the call is answered (spec `quests-act4.md` §5.9 states the activation).

## PROVISIONAL (REC-231)

The old-room removal before the placement is `d2rs-own, unverified` (the original's `0x0053ACC0` step has no owner spec).

## What is left

- Soulstone / Hellforge hammer / gem items in the synthetic game: not done. `has_item` reads the real item store through the inventory model, so the flow needs synthetic `ItemTables` rows (`mss `, `hfh `, gems) and `InvTables` (`inventory: Some(..)` in `GameParts::synthetic`), which also turns on start items and every other item path in the preview; that wants its own session and a check of the whole d2-client suite. Until then the Hellforge with a soulstone, the three hammer hits and the gem drops run only in the d2-sim unit tests (`q3_tests`).
- `wielded_weapon_code`, Tyrael's portal spawn (message 20000), the classic end-of-game timer.

## The user's local check (game files)

```
cargo nextest run -p d2-client --test app_a4_endgame
D2_GAME_DIR=<install> cargo run -p d2-client --release -- play --new sorceress Test
```
With an Act IV save that has killed Diablo: click Tyrael's portal; expect Harrogath (the town, the player on its start point), the Harrogath waypoint in the waypoint panel, and no `unhandled 254/255` line naming `0x5B4FF0`.
