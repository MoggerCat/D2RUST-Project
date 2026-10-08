# Local run 2026-10-07: first playable with walking

- Head: `origin/claude/specs-staging-7` @ `a0033923` (includes `2a8084a2`) **plus** `claude/pc1-native-skills`
  (`b11b98d`, merged as `2743beb`). Without that fix a new character has an empty client skill list, so
  no left skill and a click sends no walk (see below).
- Build: `cargo build -p d2-client --release` 0.2 min (cached; a full build earlier the same session took
  2.2 min). `target\release\d2-client.exe` = 90.1 MB.
- Data line: `play: game data from D2_GAME_DIR (137 levels, 573 objects, waypoint object class 119; level files: 2043 DS1, 34 lvlsub DS1, 241 DT1)`
- Logs: `play-walk-log.txt` (first run, ~95 s), `play-walk2-log.txt` (debug run: UI / world view).

## What worked / what didn't

| Item | Result |
|---|---|
| Tiles (Rogue Encampment) | drawn, full brightness (preview, no lighting) |
| Player | drawn; walks on left click, holding keeps walking; R run works; camera follows well |
| Walk animation | **only the down-facing direction** animates (every direction of travel shows the same facing) |
| NPCs / waypoint | **not visible**: 4 units in the model, 1 drawn, 3 hidden every frame |
| C / I panels | open and close (UI log `Opened(1)` / `Opened(2)`), drawn; some details missing |
| Outside the town gate | **entirely black**; she can walk / run onto it and is visible while moving, then is hidden under the black when she stops |
| Blood Moor ground / monsters | none (no new ground, model stays at 4 units) |
| Stability | no panic in either run (~95 s and the debug session); old 5 s crash gone |

## Numbers

Task Manager (user): d2-client.exe about 380 MB memory, low CPU (GPU not noted).

## Error lines

No `panicked`, no `error`, no `skip`. Warnings: unit art `DATA\GLOBAL\CHARS\SO\SH\SOSHlit{TN,WL,RN}hth.dcc: in no archive`
(the sorceress shield layer for town-neutral, walk and run is not found). Audio: `load_errors: 0, engine_errors: 0`.

## The walking fix found in this session (`claude/pc1-native-skills`)

`client/msg-skills.md` §2 rule 8 (new, read from 1.14d `0x00647EE0`, called by the client player init
`0x00460BF0` and the server player init `0x005348C0`): a new player gets skill 0 and its `charstats`
Skill 1–10 (base 1, owner −1), with Attack selected in both hands, before any S→C 0x94 / 0x23. The client
0x59 handler now runs it (`bridge::skills::init_player`, `charstats` loaded by
`single_player::client_class_skills`). Also: a new character's join 0x23 carry owner −1 (PROVISIONAL,
`formats/d2s-load.md` §8 r3, REC-02). Still missing (cloud): the server sends no S→C 0x94 and has no
server-side player skill list (`adapters/character.rs` `add_skill_level` "no provider").

## User's words

"Yes she walked although one direction animation but she walked, view followed her pretty good."
"She can run. Only animation is downward animation. Task manager 380mb memory use low cpu."
"When I pressed C and I the panels appeared just fine with some missing details … But it worked. And
outside town is still entirely black and no NPCs."

## Later the same night (branch `claude/local-2026-10-07`)

Merged and play-tested by the user:
- `claude/pc1-native-skills` (b11b98d): native skills at player creation (`client/msg-skills.md` §2 r8) → clicks walk.
- `claude/fix-facing` (1751f4f): the preview draws the local player facing its predicted movement. User: "the directions are amazing".
- `claude/fix-npcs` (cd7719e): one shared level-types handle for act maps and population, the object preset pass
  (PROVISIONAL, REC-94), 0x51 before the first object update → 7 NPCs and 17 objects reach the client (28 units).
- 7690c64: a pending sound-world question (line test `0x00622AA0`, no client collision rooms) is logged once in the
  preview instead of closing the app (it crashed as soon as NPCs existed).

User's notes on the last build: NPCs, stash and waypoint are drawn but cannot be used and do not animate; outside the
town gate is still black. Likely cause (to confirm): the server-side player does not follow the client's predicted
walk, so interact (C→S 0x13) fails the server's distance test and no room switch / 0x07 is sent outside town. Non-local
units have no client animation in the preview (only the local player's frames advance).

Open, for the next session:
- `claude/fix-outdoor` (worker stopped at shutdown; WIP pushed if any): does C→S 0x01 move the server player; client
  rooms / DT1 tiles outside town; the player hidden under black when stopped.
- Server: no S→C 0x94 and no server-side player skill list (`adapters/character.rs` `add_skill_level` "no provider").
- Random object population calls a placeholder (`wiring/worldgen/dispatch.rs:156`).
- Disk: `target\debug` was deleted to free 23 GB; the next debug build starts from scratch.
