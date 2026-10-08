# play-fix2: findings of the first playable run

> Fix session, 2026-10-07, branch `claude/play-fix2` (base `5eda08d9`).
> Input: `docs/local/2026-10-07/PLAYABLE.md` and its logs
> `play-walk-log.txt`, `play-walk2-log.txt`. Read only `specs/`, `docs/`,
> `crates/`, `tools/`. Synthetic fixtures only; nothing here is verified
> against 1.14d (rule 10). Preview fills stay `// d2rs-own, unverified`.

## 1. Outside the town gate is black; no Blood Moor; model stays at 4 units

**Cause 1, server (the main one).** Every C→S walk / run was refused
before it reached the path code. The dispatcher's point and unit parse
(`sim/intents-events.md` §2.4 r3–r4) reads `UnitFacts` (act, position)
that only tests stage (`SimGame::set_unit`). The play app staged none,
so `point_state` was `None` and 0x01–0x04 returned `Invalid`. The server
player never moved. That meant no room switch, no 0x07 for the Blood
Moor, and no unit adds past the rooms of the join. Only the client's walk
prediction moved the player on screen. A headless app test reproduced
it: the server player stayed at (103, 23) while the client walked.

Fix: `Intents::refresh_targets` (`d2-server/src/seams.rs`) runs before
the point / unit parse (`dispatch.rs`). `SimGame` fills the facts of the
client's player and of a unit target from `WorldHost::live_facts`: the
action wiring's room act and path position (`world/action.rs`, delegated
by `WiredWorld`). This happens only for units the caller did not stage,
so staged test facts are unchanged.

**Cause 2, server.** Units created in a room the client already holds
were never sent. Examples: town NPCs and objects populated after the
join, and monsters of rooms populated after they came in sight. Rule
§7.1 r2.1 (a unit with flag 0x10, "not yet announced", gets its add
messages in the first client pass) was done only inside the monster
update. `ActionSim::send_unit_update` now runs it for every type. A
missile gets nothing at all, and a monster keeps its own path. Test
fixtures that stage units straight into a client's rooms now clear flag
0x10, as the room clean-up does for units the clients already know.

**Cause 3, client.** The walk prediction moved only the drawn position.
The model kept the local player in the room of its last placement. So
the near rooms (`draw-order.md` §9: the adjacency array of the player's
room) never followed her. A server 0x08 for that room would also leave
her in no room, and then there is no map at all. This is the
"hidden under the black when she stops" case. Now `world_view/walk_room.rs`
recaches the local player's room at the predicted sub-tile each frame
(`ClientWorld::recache_local_room`: `model.md` §12 r2, the room recache
of `unit-order.md` §5 r6). This is a D2 preview fill, PROVISIONAL REC-51.

**Draw order vs missing tiles:** draw order was not the cause. With no
near room under her the map was empty, and with no room at all the unit
order was skipped.

Tests:
- `tests/app_level_border.rs` `walking_east_out_of_the_town_the_map_follows_into_the_blood_moor`
  is new. The app's synthetic set now has a Blood Moor room east of the
  town (vis 1 ↔ 2, a border). The test walks across it and checks that
  the server player arrives and the client's player room is level 2.
  Before the fix the walk never moved the server player.
- `bridge/msg/tests_drlg.rs` `the_predicted_walk_recaches_the_local_players_room`.
- `d2-sim` `update_pass_announces_a_new_object_first`.

## 2. NPCs / waypoint not visible (4 units, 1 drawn, 3 hidden)

`units_hidden` counts every model unit whose `unit_pose` is `None`. That
includes units the draw order does not file: not in a near room's list,
or off the frame's draw grid (`OriginalView::unit_pose`, `UnitSlot::NotDrawn`).

The 3 hidden units were in the model but outside the player's near rooms
or off screen. The live waypoint stands at a fixed offset in the town's
*first* room (`single_player.rs` build), which need not be near the start
point. The NPCs were missing from the model entirely, because of causes 1
and 2 above (no streaming, no announce).

No hiding predicate was wrong, so nothing was changed there. The
progress log now prints a second line every 250 frames with the model and
predicted cell, the local room, the active rooms by level and the units by
type. It also warns once for each refused S→C message (these were silent
before) and counts dropped messages. The next run shows which units the
model holds.

## 3. Only the down-facing walk animation

`UnitRules::dir64` was 0 for every unit.
- **Local player:** `Predict` now keeps a facing. It is the
  `direction_vector` (`pathing.md` §8.3) from the predicted position to
  the walk target, and it is kept when she stops. `walk.rs` hands it to
  `UnitArt::pose_dir`.
- **Other units:** they face their walk target (mode requests
  0x01 / 0x17, 0x00 / 0x18) or their last position change, and keep that
  facing when standing.
- **Direction counts:** these follow `unit-composite.md` §3 r3 (players
  8, the local player 16, objects 1). Monsters still use the COF's count.

All of this is d2rs-own, unverified (REC-51). Tests: `predict.rs`
`facing_follows_the_direction_vector`,
`the_walk_sets_the_facing_and_it_stays_when_standing`; `unit_rules/tests.rs`
`units_face_their_walk_target_and_keep_the_facing`.

## 4. No S→C 0x94; no server-side skill list

Nothing stored a server player's skills. `Pending::skill_list` had no
provider.

- **New list:** `d2-sim` `skills::list::SkillList` implements
  `msg-skills.md` §2 r1–r4 and r8. Each player has one, in
  `ActionHooks::skill_lists`.
- **Character load:** it runs rule 8 first (skill 0 plus `charstats`
  Skill 1–10, Attack in both hands). A save's skill bytes set the class
  entries (`add_skill_level`).
- **0x94:** `enter_game` sends it after part B for a full save (reader
  `0x0056A7B7`).
- **New characters:** the stub load (`0x00569F80`, §8.2 r3.1) reads no
  skills section, so it sends no 0x94. The client's own rule 8 init
  (`bridge::skills::init_player`) gives its native skills.
- **Unverified:** the 0x94 entry order and levels are d2rs-own.

Tests: `synthetic_game.rs` (full save: `0x94` between 0x76 and 0x0B,
exact bytes; stub: server list, no 0x94), and `list.rs` unit tests.

## 5. `SOSHlit{TN,WL,RN}hth.dcc: in no archive`

`unit-composite.md` §5.1 r3 says that with no shield the SH code is `lit`
and the original still makes the request. §6 r4 says a file that does not
load leaves the slot empty, silently. So the request is spec behavior, and
only our warning was wrong. A component file that is in no archive is now
remembered as an empty slot with no log line. Unreadable files and missing
COFs still warn.

Test: `missing_files_are_skipped_with_one_log_line` (1 line now, not 3),
`an_unreadable_component_file_is_logged`.

## What is left

- **Monsters:** their facing comes only from position changes. The spawn
  direction (msg-units §1.2 r9) and the turn of `pathing.md` §8.5 are not
  in the model.
- **Combat and skill use:** they still read the empty `Pending::skill_list`
  seam, not the new lists. Item-granted skills have no provider.
- **Unit facts for items:** `live_facts` answers players, monsters and
  objects only. An item target (owner) still needs staging.
- **Waypoint position:** the live waypoint is placed at a fixed offset in
  the town's first room, not at its DS1 preset.
- **No live check yet:** nothing above is checked on the user's files.
  REC-51 (own-walk motion) still replaces the prediction.

## Local rerun (Windows, PowerShell, 1.14d files)

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
$env:RUST_LOG = "warn,d2_client=info"
git fetch origin claude/play-fix2; git checkout claude/play-fix2
cargo run -p d2-client --release -- play --new sorceress Test
```

What you should now see:

- **Walking:** left-click a few tiles away. She walks and **faces the
  way she walks** (all 16 directions), and keeps that facing when she
  stops. R toggles run once stamina is in.
- **Town:** NPCs (Akara, Kashya, Charsi, Gheed, Warriv…) and objects
  appear as you walk near them. Each `frame N:` pair of log lines should
  show `units by type` growing past `{0: 1, 2: …}`, with type 1
  (monsters / NPCs) present.
- **Outside the gate:** walk out. The Blood Moor ground draws as you go
  (`active rooms by level` gains level 2, `local room` shows level 2), and
  Blood Moor monsters appear (more type 1 units). She stays drawn when
  she stops.
- **Log:** no `SOSHlit…hth.dcc` warning. Copy any `S→C 0x.. refused: …`
  warning into `docs/HANDOFF.md`. One is expected to be rare; many mean
  a client handler disagrees with the server.
- `--save X.d2s`: the join now carries a 0x94 (the client's skill list
  from the save).

If the outside is still black, copy two consecutive `frame N:` lines
(the second line has the local cell, predicted cell, local room and
active rooms) and any `refused` warnings into `docs/HANDOFF.md`.

## Gate on the pushed head

`cargo fmt --all`; `cargo clippy -p d2-client -p d2-server -p d2-sim
--all-targets -- -D warnings` clean; `cargo nextest run -p d2-client -p
d2-server -p d2-sim`: 6474 run, all pass after the last expectation fix;
`python3 tools/coverage.py --check` 0 errors. `cargo nextest run -p
test-fixtures`: 61 run, all pass.
