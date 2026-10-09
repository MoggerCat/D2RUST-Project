# Handoff: q-tool-autoplay (wrapped 2026-10-09)

Branch `claude/q-tool-autoplay`. The autoplay bot (objective, fight and
stuck logic) was dropped by the user's decision; what stays:

- `d2-client autoplay-host` (`crates/d2-client/src/app/autoplay_host.rs`,
  spec `specs/tools/autoplay.md`): the `play` client headless on
  `MinimalPlugins`, mouse and key events on stdin into the UI queue,
  `state` / `map` reads only.
- `tools/autoplay/replay.py` and `tools/autoplay/routes/act1..5-probe.route`:
  fixed real-input smoke routes with checks (README there).
- One fix in `ui/waypoint_ui.rs`: `OriginalUi::waypoint_rows` read tab 0
  whatever tab the menu opened on; it reads the open's tab
  (`app_play_npc` still passes).

## Findings (real input only; open unless noted)

| Id | What | Repro | Routed to |
|---|---|---|---|
| attack-freeze | After the first attack click (C→S 0x06, damage dealt) the client model keeps the local player in mode 7 while the server is back at 1; `bridge::click::can_act` refuses modes 7–12, so every later click sends nothing. Nothing in the client ends the local player's attack mode (`play_smoke` sends raw 0x06 and reads the server mode, so it misses this). | fresh Amazon seed 1234: walk into Blood Moor, left-click a zombie (frame ~684) | skills-2 session_012CW5dsXr7CfrvY7guLFsV6 |
| esc-npc | Esc closes an NPC talk without C→S 0x30; afterwards every other NPC's interact (0x13 sent) gets no 0x2F / menu. The menu's cancel rows send 0x30 and work. The same Esc also opens the Esc menu. | Act I: talk to Akara, Esc, click Kashya | client UI session_012dLEFStoaXPQozKq9iGx4Y |
| esc-questlog | With the quest log open (ui 17, e.g. after Atma's talk), Esc closes it and opens the Esc menu (9) in the same press; Esc on that menu reopens the log, so Esc never clears the screen (the Q key closes the log). | Act II save, talk to Atma, Esc twice | client UI session_012dLEFStoaXPQozKq9iGx4Y |
| town-objects-gone | After the Cold Plains → Rogue Encampment waypoint trip the server holds no level-1 objects (17 at game start incl. waypoint 119 at 4899,4209; 0 after the return, player at 4898,4208; the town NPCs are back), so the town waypoint / stash can't be used again. | fresh Amazon seed 1234: walk to the Cold Plains waypoint, take it to town | progression session_01UWEn2ZtdtLRHPZRdytF7Gc |
| monster-motion-crash | Client panic: `d2_sim::path::walk::geom::direction_vector` index out of bounds (negative index, len 128) from `d2_client::bridge::motion::MonsterMotion::frame`, frame 3265 in Cold Plains walking toward Stony Field (seed 1234, reproduced twice). | fresh Amazon seed 1234, Cold Plains → Stony Field | client track session_01BQSDGiWHKAbHXoWk7HThvM |
| npc-no-menu | Act III natalya (297), Act IV halbu (257) and jamella (405), all monstats `interact` 1: the click sends C→S 0x13 but no 0x2F / NPC menu ever comes. Other town NPCs of Acts I–V talk. Lead from session_01YUEYT5 (stopped): its branch 8cbd57d8 changed the NPC scan-2 gate (quest active test, `wiring/action/ai.rs` nearest_player); probably unrelated to the 0x13 → 0x2F path, rule it out first. | `save --class sor --level 30 --waypoints all --quests acts=N-1 --act N-1`, click the NPC | open (quests/NPC session stopped) |
| nihlathak-no-interact | Act V nihlathak (515, `interact` 1): clicks on him only walk, no 0x13 is ever sent. | Act V save, click Nihlathak | open |

Not findings: townsfolk and guards (act2male/female 195/196, act2guard1
203, act3male/female 294/296, Act V 567–569) never interact; their
monstats `interact` is empty (injured barbarians 568/569 have 1: unverified
whether 1.14d lets them talk).

## How it was run

Headless host, 1.14d install from the private data repo, `D2_GAME_DIR`
set. Each act's probe took 1–3 min wall. The routes in
`tools/autoplay/routes/` are those runs' recorded input, so each finding
above that sits on a route can be re-checked by replaying the route and
clicking on from its end.
