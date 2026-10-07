# Handoff: the server's single-player join, part 2 — `claude/impl-server-join-2`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of the tenth fold (`claude/fold-handoff-10`); this file stays as the detailed record. Its open questions are in HANDOFF §7 "Tenth set" (PC 1 / PC 2).

Cloud implementation session, 2026-10-07, task class: implementation
from specs, medium. Base: `claude/specs-staging-2` at `ddbfe0b`. Repo
only (M09: every claim holds on this branch, synthetic data). Task: the
PC 1 answers to `impl-server-join` §3 (`sim/intents-events.md` §7.8,
§7.9, §8; `sim/path-placement.md` §11; `sim/tick.md` §6 rule 6).
Stopped early on a coordinator budget cut; §3 lists what is left.

## 1. What changed

| Path | What | Spec |
|---|---|---|
| `d2-sim` `units/messages.rs` (new) | pure builders: 0x01, 0x00, 0x02, 0x04, 0x08, 0x09, 0x0A, 0x0B/0x76 (`unit_ref`), 0x23, 0x51, 0x5F, 0x7B, 0x7E (zeros), 0xAA (states bit stream: 218-byte stop, 0xF4 writer cap, clamps) | `intents-events.md` §7.2, §7.8, §7.9 r1, §8, edge 10 |
| `d2-sim` `wiring/action/switch.rs` (new) | `View::room_switch` (`0x00537B50`): joins (0x07, AI wake-up `0x00573780` when the room's only client, add messages of every unit but the player), leaves (0x0A per non-missile unit, 0x08, player update after the old room); `add_messages` (player 0x59 + 0xAA + 0x76; object 0x51 + portal seam; tile 0x09); `client_room_ready` (`0x0061A460`); `SessionState::names`; `assign_player` | §7.2, §7.8, §7.9; `tick.md` §6 r6 |
| `d2-sim` `path/place.rs`, `place_seams.rs`, `wiring/path/place.rs` | game entry: 0x07 spawn room, **room switch** (`CollisionView::client_room_switch`), placement, 0x15, **0x7E** (`PlaceMessage::GameEntryDone`) — all before any tick | `path-placement.md` §11 |
| `d2-sim` `drlg/active.rs`, `wiring/action/rooms.rs` | `Drlg::client_switches_room` (joined + left), `Drlg::room_ready`; `DrlgWorld::client_switches_room` → `RoomSwitch` (0x07/0x08 fields, client counts) | `rooms.md` §4.1; `tick.md` §6 r6 |
| `d2-sim` `wiring/action/dispatch.rs` | `client_level_change` → `room_switch`; `client_room_ready`; `send_load_complete` (0x04) | `tick.md` §6 r4–r6 |
| `d2-sim` `combat/vitals/sync.rs`, `wiring/action/vitals_sync.rs` | `sync::join` / `join_run`: the join's forced 0x95 + gold + exp | §8.2 r3.9 |
| `d2-sim` `skills/use_/bodies/mod.rs` | `BodyStat` send bits / param bits / signed, `BodyTables::state_nosend` (0xAA's inputs; kept out of `StatInfo`, whose gap test pins them unread) | §7.9 r1 |
| `d2-sim` `units/lists.rs` | `client_state::LOADING` (1), `ACT_LOADED` (2) | §8.1 r4, §8.2 r4 |
| `d2-server` `adapters/session.rs` | `create_game` (0x01, 0x00 + state 1, 0x02; `GameSetup`); `enter_game` per §8.2: 0x59, 0xAA, 0x76, 0x0B, [0x5F], 0x7B per valid hot key, [0x23 × 2], 0x95…, 0x03 + state 2, game entry, state 3. `Entry::new`, `HotKey`, `PlayerRecord` (0x5F / 0x23 only when the save's record is given) | §8.1, §8.2 |
| `d2-client` `app/single_player.rs` | client record in state 0, `create_game` (`GAME_SETUP`), `enter_game(Entry::new)`; the first tick sends 0x04: **the client reaches `in_game`** | §8, `tick.md` §6 r6 |

Tests updated to the spec'd order (stricter, none loosened):
`test-fixtures` `town_entry_sends_the_join_sequence` (full prefix 0x01 …
0x7E, 0x04 once inside the first flush, state 4);
`d2-sim` `room_switch_sends_add_and_leave_messages_and_the_join_completes`
(new), `game_entry_and_level_warp` (switch before placement, 0x7E),
`prop_path_place` (switch recorded, 0x7E), `units::messages` tests,
e2e `tick_runs…` (AI think 203: the join's wake-up);
`d2-server` `waypoint_to_the_town…` (0x07, 0x0A, 0x08, 0x15, 0x08);
`d2-client` `app_client_drlg` (`in_game` true), `app_frame_loop` (16
handled, unowned {0x76, 0x7E, 0xAA}, sight with 0x08, in game, 0x0D
drained), `e2e_full_loop` / `e2e_single_player` / `e2e_walk` (join 0x51,
the warp's leave side, AI think 203 pending, handled/rejected counts with
the 0x08 fatals 0x59E). The 0x15 "finding 1" of `wire-path-server.md` §4
now comes from the switch's player update (§7.8 r3.4).

## 2. Gate (head in the commit)

- `CARGO_INCREMENTAL=0 cargo test --workspace --exclude d2-client`:
  4,400 passed, 0 failed, 191 ignored.
- `d2-client` on this branch alone: the same 85 reds as the base (the
  dispatch-table `Mismatch`, fixed by `impl-client-msgs-3`). On a scratch
  merge with `origin/claude/impl-client-msgs-3` (`48c55fd`, not merged
  here): all d2-client tests pass (0 failed). Merge order: either; the
  only shared file, `app/single_player.rs`, auto-merges.
- clippy `-D warnings`, fmt: clean. `coverage.py --check`: 8,353 claims,
  0 errors. `spec_index.py --check`: ok.

## 3. Left (named in code; nothing guessed)

- Monster add messages (0xAC server-side fields past `monsters/init.md`
  §24; part B's mode message is impl-monster-death's), missile 0x73,
  item 0x9C in the switch (item world not reachable from the action
  wiring, as §7.1), object class 59's 0x82, player part B for other
  players, corpse 0x74 / `0x00534F80`, leave-side `0x005738D0`, rule 4's
  portal-flag record update (`0x0061AE30`).
- Join: the loader's 0x94, 0x22, 0x21, 0x23, 0x5E, 0x28, 0x29; the stat
  messages (0x0053BE40's 0x1D/0x1E/0x1F choice unspecified); item
  messages (r3.5) and the update-list reset (r3.10); 0x53 (`0x0061C330`);
  followers; the join sequence after 0x04 (0x5B, 0x65, 0x8D, 0x5A) and
  the inventory refresh `0x0055DF00`.
- A new character's player record (0x5F value, skill hands) and the save
  loader: `Entry::record` is `None` in the app, so 0x5F / 0x23 are not
  sent there (the client would reject 0x23 without the loader's 0x94).
- Real C→S 0x67 / 0x6B handling in a `SessionHandler` (the app still
  calls `create_game` / `enter_game` directly; same order).
- Spec questions: the overhead 0x26 text of a set hover (§7.9 r3); the
  app's arena flags (recorded 0x00100004 used).
- Local queue: C84 can now compare the whole frame-1/frame-2 order.

## 4. Merge of `claude/specs-staging-2` (coordinator task)

Merged (no rebase) with monster-death, client-msgs-3, d2s-load and the
rest. Kept both sides: `units::{messages, mode_set}`,
`wiring::action::{switch, unit_update}`, both `WiringError` variants;
`enter_game_from_save` now builds `Entry::new(act, name)` and is followed
by the full session sequence. Tests updated to the join's order:
`e2e_full_loop` (both sides' log counts: 11 handled, 0x0D + 2 × 0x69
dropped), `synthetic_game` (the save tests see 0x01, 0x00, 0x02, 0x59,
0xAA, 0x76, 0x0B, 0x95, 0x03), `e2e_night_flows` (0xAA, 0x76, the
switch's 0x07, 0x7E, 0x04), `e2e_night_world` (the join's AI think at
203 pending), `wiring::path::mutant_tests` (0x7E).
Gate: `CARGO_INCREMENTAL=0 cargo test --workspace`: 5,514 passed, 11
failed (only the allowed ones: skills::* 6, quests tables_parse_and_check,
3 player::tests::wired, bridge::local_tests::unknown_and_unowned_ids),
223 ignored; clippy, fmt clean; coverage 8,703 claims, 0 errors;
spec_index ok.
