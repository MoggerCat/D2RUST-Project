# Handoff: the server's single-player join — `claude/impl-server-join`

> Not yet folded into `docs/HANDOFF.md` §1–§4 (only the §5 queue entries
> C84–C86 and the text of C82 are edited there, plus `docs/PLAN.md`); the
> coordinator folds it.

Cloud implementation session, 2026-10-07, task class: implementation
from specs, medium. Base: `claude/specs-staging` at `5f90364` (merge of
`impl-client-drlg`). Repo only: no `game/`, no `re/`, no recordings (M09:
every claim below holds on this branch, on synthetic data). Task:
`docs/HANDOFF.md` §2 step 4, the next unlock of
`docs/handoff/impl-client-drlg.md` §6 step 1. Read:
`specs/sim/path-placement.md` §2.5, §10, §11 (incl. "Recipients"), §13,
Test vectors R1–R3; `specs/client/model.md` §3, §5, §7, §9, §11, §12,
Test vectors, Provenance; `specs/client/msg-units.md` §1.1;
`specs/sim/intents-events.md` §1, §2.5, §3.2, §4, §7.2, OQ 2;
`specs/sim/tick.md` §6; `specs/drlg/rooms.md` §4.1–§4.2;
`specs/drlg/levels.md` §2, §10; `specs/world/objects.md` §2;
`specs/sim/rng.md` §5.2; `specs/sim/server-messages.tsv`.

## 1. What changed

| Path | What | Spec |
|---|---|---|
| `d2-server` `adapters/session.rs` (new) | `enter_game(sim, client, &Entry { act, name })`: for a joined client whose player has no room, queues to that player's client (through `Pending::send`, so the host flushes it with the next tick, ahead of the tick's messages) S→C 0x59 (GUID, class, name, (0, 0)), 0x0B (type 0, GUID), 0x03 (act, the act DRLG's init seed = game +0x7C, the act's town level id, game +0x80), then game entry (`wiring::path::place::game_entry`: 0x07 spawn room, put in world, 0x15 flag 1). `load_act(act, map_seed, obj_seed)`. `JoinError` (not joined, no player, already placed, no act DRLG, path provider off, game entry's fatal assert) | `path-placement.md` §11, §13 r1–r3; `model.md` §11 r1, r3; `intents-events.md` §4 r3, §7.2 (part A) |
| `d2-sim` `wiring/path/place.rs` | `game_entry(ctx, player, act)`: the wiring of `path::place::game_entry` (fatal asserts logged as `WiringError::Place`) | `path-placement.md` §11, §13 |
| `d2-sim` `drlg/active.rs`, `wiring/action/rooms.rs`, `wiring/action/dispatch.rs` | `Drlg::client_changes_room` returns the rooms the client joined (new adjacency order); `DrlgWorld::client_changes_room` returns their (tile x, tile y, level id); `ActionSim::client_level_change` sends S→C 0x07 for each to the client's player | `path-placement.md` §11 "Recipients"; `rooms.md` §4.1 |
| `d2-sim` `wiring/action/objects.rs` | `ObjectState::obj_seed`: `dwObjSeed`, the `lo'` result of `0x00546C60` (game +0x80) | `objects.md` §2 r2; `rng.md` §5.2 |
| `d2-proto` `s2c` | built types `GameHandshake` (0x0B, 6 bytes) and `AssignPlayer` (0x59, 26 bytes), parsed; audit rows and `docs/handoff/s2c-builders.md` updated | `model.md` §3 r1; `msg-units.md` §1.1 r1; `intents-events.md` §7.2 |
| `d2-client` `app/single_player.rs` | the app's game: path provider on; synthetic level 1 (Rogue Encampment, one 8 × 8 room at tile (16, 0), so act 0's levels do not overlap); rooms streamed in the town, Cold Plains, Lut Gholein; the waypoint object in the **town's** room (was Cold Plains); the player allocated without a room at (0, 0), joined in state 4, then `enter_game` (act 0, name "Sorceress"); `LocalGame::player_guid`, `Started::player_guid`; `ACT1_TOWN`, `PLAYER_NAME`; `PLAYER_X` removed | `path-placement.md` §13 |
| `tests` | see below | — |

So `play` now receives, with its first tick: 0x59, 0x0B, 0x03, 0x07
(spawn room), 0x15, then the room switch's 0x07s, and builds the client
DRLG (`impl-client-drlg`); the local player is known, placed, in a
level-1 room.

Tests added (each with its `// Covers:`): `d2-proto`
`s2c::tests::recorded_join_messages` (0x0B seq 113 bytes; 0x59 offsets);
`d2-sim`
`wiring::action::tests::rooms::room_switch_reveals_each_joined_room_in_adjacency_order`
(three rooms: the first switch reveals A's array, A → C only C);
`test-fixtures` `synthetic_game::town_entry_sends_the_join_sequence`
(the synthetic town from archives: exact bytes of 0x59, 0x0B, 0x03 with
the object control's seed, 0x07, 0x15 at the spawn search's point, the
switch's 0x07; a second `enter_game` is `Placed`; two runs equal);
`d2-client` `app_client_drlg::the_join_builds_the_client_drlg_in_the_app`
now runs the **real join** of the app's game instead of a scripted link
(act, client DRLG, local player, its position = the server's path
position, level 1, rooms in sight, BlankScreen; `in_game` false, see
§2), and `the_session_join_on_the_install` (`#[ignore]`, C86). The
recorded-join vector test stays scripted (it checks recorded bytes).

Tests updated to the spec'd behaviour (stricter, not weaker): the room
switch now sends 0x07s, so `e2e_full_loop` (frame 2: the four 0x07 of
the Isle's adjacency array; after the waypoint warp: 0x07, 0x0D, then the
six 0x07 of the gate room's array; 11 rejections, fatal 0x58A, as this
staged game sends no 0x03), `e2e_single_player` (frame 2's four 0x07;
four rejections), `e2e_walk` (the town's 0x07 after the warp; two
rejections), `d2-server` `walk::tests::waypoint_to_the_town_…` (the
next tick's 0x07 for C), `app_frame_loop::frame_loop_ticks_…` (the join
and the waypoint travel from the town: 0x59, 0x0B, 0x03, 0x07, 0x15,
0x07 (Cold Plains), 0x0D (queued on the local player), 0x07; the local
player hidden by the placeholder rules; the UI open mode set as in
`app_client_drlg`). No test was removed or loosened.

## 2. Still pending (named in code; nothing guessed)

- **S→C 0x04 and client state 3** (`tick.md` §6 r4): a joining client is
  in state 3 until its room is ready (`0x0061A460`, unspecified), then
  gets 0x04. The app joins the client in state 4 and sends no 0x04, so
  the client never sets `in_game`, and the client update pass (which
  drains unit queues, `model.md` §5 r1) never runs. TODO in
  `single_player::build`.
- **The messages between 0x0B and 0x03** (the "…" of `model.md` §11 r3;
  the rest of `0x00530190` before the act creation) and **0x53** after
  0x03 (`model.md` §11 r1; layout partial): not sent (module docs of
  `adapters/session.rs`; C84 lists them from the recording).
- **Session messages of game creation** 0x01, 0x00, 0x02 (recorded order
  0x01, 0x00, 0x02; sender and trigger `intents-events.md` OQ 2; 0x01's
  u32@2 is "unk"): not sent. The app builds the game directly (no C→S
  0x67 / 0x6B path); `enter_game` is the game part of the 0x6B path.
- **Add messages in the room switch** (`0x0053A8E0` → `0x00571F90` for
  every unit of a joined room but the player): part B is unspecified
  (`intents-events.md` §7.2), so none is sent; TODO in
  `ActionSim::client_level_change`. The client therefore never hears of
  the waypoint object or monsters.
- **S→C 0x08**: no spec names its server sender (`0x0053A9B0` on the
  leave side states no message); not sent.
- **Game +0x80** in the app: the app's game has no object control (no
  game-creation sequence, `rng.md` §5.2 TODO), so its 0x03 carries 0.
  `test-fixtures`' world sim has one and sends its `obj_seed`.
- **The staged e2e games** (`e2e_full_loop`, `e2e_single_player`,
  `e2e_walk`, `game_wired_host`, `synthetic_game::run`) still stage the
  player at fixed points without the session join, so their clients
  refuse the 0x07s (no 0x03). Folding them onto `enter_game` changes
  their scenarios (the player starts at the spawn point): a follow-up.

## 3. Questions for the spec owners

1. **0x15 vs the switch's 0x07s** (`path-placement.md` §11): the text
   says game entry sends 0x07 and 0x15 "at once" and the other 0x07s come
   from the first per-client update's room switch, i.e. 0x07, 0x15, 0x07
   × 9; R2 (same section) and `model.md` §11 r3 / Provenance record 0x07
   × 10 then 0x15. d2rs follows the rule text (the join is queued before
   the tick; the switch runs in the tick). Which sends the 0x15, and
   when? (C84.) The client outcome is the same either way.
2. **The spawn room's own 0x07 twice**: by §11 the switch sends 0x07 for
   every room of the new array, which includes the spawn room, after game
   entry's 0x07 for it; d2rs sends both (R2's "one from this section,
   nine from the switch" agrees if the array has nine rooms). Confirm.
3. **The rest of the 0x6B path** (`0x00530190`) between 0x0B and 0x03,
   and where 0x59 / 0x0B are sent from (the recording shows them before
   0x03). d2rs sends 0x59 (part A only) and 0x0B first, as recorded.
4. **0x04 / room ready** (`0x0061A460`) and the client state at join:
   needed for the client to be in game.
5. **0x01 / 0x00 / 0x02 at game creation**: sender, trigger, 0x01's
   u32@2 (recorded 0x00100004).
6. **0x08's server sender**, if any (room leaving the adjacency array).
7. **0x59 name** (player data): d2rs takes it from `Entry::name`; the
   save's name field is `formats/d2s.md`'s, not wired here.

Answers (spec session `claude/spec-server-join`, 2026-10-07):
Q1, Q2 → `sim/path-placement.md` §11 (game entry runs the room switch
itself: 0x07, 0x07 × array, 0x15, 0x7E, before the first tick) and
`sim/intents-events.md` §7.8; Q3 → `intents-events.md` §8.2; Q4 →
`sim/tick.md` §6 rule 6 and `intents-events.md` §8.3; Q5 →
`intents-events.md` §8.1 (0x01 u32@2 = the arena flags); Q6 →
`intents-events.md` §7.8 rule 3 (`0x0053BC90`, room leave); room-switch
add messages → §7.8 rule 2, §7.9; monster messages 0x67–0x6D → §7.7.
Q7 unchanged.

## 4. Gate (this branch, head in the commit)

- `CARGO_INCREMENTAL=0 cargo test --workspace --no-fail-fast`: 5,263
  passed, 0 failed, 218 ignored (every target). The d2-client build
  needs `tools/cloud-setup.sh`'s packages (`libwayland-dev` etc.).
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
  `cargo fmt --all --check`: clean. `cargo run -p depcheck`: OK.
- `python3 tools/coverage.py --check`: 8,149 claims, 0 errors.
  `python3 tools/spec_index.py --check`: ok. `tools/methods.py check`: ok.

## 5. Local run queue (added to `docs/HANDOFF.md` §5)

- C84 the recorded join's frame-1 order and callers (Q1–Q3).
- C85 game +0x80 = `0x00546C60`'s result = 0x03 u32@8.
- C86 the session join on game files
  (`app_client_drlg -- --ignored the_session_join_on_the_install`).
- C82's text updated: `play` now builds the client DRLG.

## 6. Next steps

1. Spec answers for Q1, Q3, Q4 (C84 settles most from the recording);
   then the remaining join messages and 0x04 (the client in game, its
   update pass running).
2. The add messages of the room switch once `intents-events.md` §7.2
   part B is specified (the client then sees the town's objects).
3. Fold the staged e2e games onto `enter_game` (one session).
4. A game-creation sequence for the app (`rng.md` §5.2: monster regions,
   object / NPC / quest controls), which also gives 0x03 its game +0x80.
