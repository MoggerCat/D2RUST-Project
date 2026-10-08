# q-a1-bloodraven: Act I quest 2, Sisters' Burial Grounds, in the play preview

Stitching session, branch `claude/q-a1-bloodraven`. Nothing here is verified against 1.14d (rule 10). Open points: REC-134 (`docs/HANDOFF.md` §7). Sound not wired.

## The path, and the links that were missing

| # | Link | Before | Now |
|---|---|---|---|
| 1 | Kashya in the town (class 150, `npc` + `interact`) | none; the synthetic town held Akara only | `app/single_player.rs`: monstats row, client unit row, allocation at `KASHYA_X`; her `hireling` rows (`synthetic_hire_rows`), without which her NPC start stopped in `send_hire_list` |
| 2 | Talk → message 24 (intro), then 81 (quest) → 0x28 / 0x5D | server tests only | real host, unchanged code; test `kashya_gives_the_sisters_burial_grounds` |
| 3 | Level change → quest event 3 (state 3 on level 17), kill → kill parse, monster chain link | q-a1-tower's quest event queue (`wired/quest_events.rs`, `LocalSeams::{monster_quest_chain, kill_step}`, `AppRest::chains`) already carries them; reused as is. Blood Raven's link is queued by `LocalSeams::host_monster_created` (below); with game files her boss mods call `monster_quest_chain` (`init.md` §14.3) |
| 4 | The quest updater `0x00543E10` (A1Q2's timer, period 15: the "quest complete" 0x5D) | not driven in the live host | `quest_events.rs` runs `QuestControl::update` on every 20th frame (`quests.md` §5) |
| 5 | Kill → quest chain → state 4 | tests only | test `blood_raven_dies_and_kashya_pays` |
| 6 | The reward (message 92 → state 5, status 13, 0x28, mercenary slot hired) | server tests only | test `kashyas_reward_runs_after_blood_raven` |
| 7 | A level to put Blood Raven in | none | synthetic Burial Grounds (level 17, one flat room), warp pair from the Blood Moor slot 3 (warp ids 29 / 30), host-placed Blood Raven (below) |

d2-sim changes: `WorldPending::host_monster_created`, `View::spawn_host_monsters` (`HOST_MONSTER_PRESET`, next to the warp tiles). d2-server: the updater line in `quest_events.rs`. d2-client: `app/synthetic_burial.rs`, the level / Kashya edits in `single_player.rs`.

## Tests
`cargo nextest run -p d2-client --test app_play_bloodraven` (3): the quest starts from Kashya (the client's quest log row 2 shows status 1); the player enters level 17 (state 3), Blood Raven stands there with chain 2 and reaches the client, her death (the sim's `kill`, the player as killer) takes the quest to state 4, 17 updater ticks later the status is 3 (completed now); Kashya's message 92 pays (state 5, status 13, one mercenary slot hired). `app_play_quests.rs` finds Akara by class now that a second NPC stands in the town.

## PROVISIONAL (REC-134, all `// d2rs-own, unverified`)
- The Burial Grounds is reached from the Blood Moor and is one flat room (the original: Cold Plains, a drawn outdoor level); Blood Raven is placed by the host from the level types' preset list (`HOST_MONSTER_PRESET`, class 267, fixed sub-tile) on the synthetic game only. With game files the population creates her (preset 5) and her boss mods link chain 2 through link 5.
- The synthetic Blood Raven has no AI and no minions (the synthetic `monstats` rows hold no AI data); with game files both come from the monster init and the AI.
- Kashya's town position, and her `hireling` rows (Rogue Scout, one per difficulty, names 100–104).
- The synthetic game has no `HirelingTables`: the reward marks a mercenary hired but creates no unit (`NoHirelingTables` in the interaction errors). With game files the existing hireling path (q-hire-follow) applies.
- `players_near` (the J3 near test) answers every player; single player only.

## What is left
- The kill parse skips the `0x80000000` unit-flag guard of `damage.md` §7.2 step 3 (the sim's `kill` does not read it).
- Expectations changed with the new scene, not weakened: `app_single_player` (one more seed step), `app_frame_loop` (joined 26, handled 37, queued 3), `app_play_quests` (Akara found by class).
- Walking from the Blood Moor to Kashya in the synthetic world was not exercised (a walk west panicked in `path/walk/geom.rs:197`, index underflow: not this task's code; the reward test starts in the town instead).
- With game files nothing here was run (no `game/` in the cloud).

## The user's local check (game files)
```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
git fetch origin claude/q-a1-bloodraven; git checkout claude/q-a1-bloodraven
cargo test -p d2-client --test app_play_bloodraven
cargo run -p d2-client --release -- play --new amazon Test
```
In `play`: finish the Den of Evil (Akara's reward), then talk to Kashya twice (introduction, then the quest; press Q: the Burial Grounds row shows started). Go to the Burial Grounds and kill Blood Raven (on the way the quest changes to state 3, on her death to state 4; about 6 seconds later the log marks it done). Talk to Kashya again: the reward text, and a mercenary offer. Note in `docs/HANDOFF.md` §7 REC-134 what differs (a Blood Raven without the quest link: the console line `QuestKill` absent; Kashya without a menu: `NoHirelingRow` in the log).
