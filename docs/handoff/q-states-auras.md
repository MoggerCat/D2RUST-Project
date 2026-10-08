# q-states-auras

## Links connected
- Missing link: the server sent 0xAA at unit add only; a later state toggle sent nothing, so the client (0xA7/0xA8/0xA9 handlers in `bridge/msg/states.rs`, overlays in `world_view/missiles.rs`) never saw auras, curses or cold/poison.
- `d2-sim` `units/messages.rs`: `state_ref` (0xA7/0xA9), `set_state` (0xA8); entry writer shared with 0xAA.
- `d2-sim` `wiring/action/state_update.rs`: `View::state_change_messages` (spec `intents-events.md` §3.5 r6), called from `dispatch.rs` `send_unit_update`.
- `wiring/action/combat.rs` `set_state` now queues the unit (like `0x00639DB0`).
- Expiry: `expire_lists` already toggles the state off; the changed bit then gives 0xA9.

## PROVISIONAL
REC-120 (players get the messages too).

## Left
Unit tints for colour states; client-side state hooks; a recorded aura trace.

## Local check
`cargo nextest run -p d2-sim unit_update` then `play`: cast Might / a curse / Frost Nova; the state overlay should appear on the unit and vanish when it expires.

Local check done 2026-10-08 (PC 1 round 2, branch claude/local-pc1-s8): `cargo test -p d2-sim --lib unit_update` passes (40 tests in one filtered run with equip, vitals_sync, socket); the in-game overlay check needs a player.
