# local-buddy: recordings of the original 1.14d (task `rec`)

Branch `claude/local-buddy-recordings-2026-10-07` (from
`origin/claude/specs-staging` + `spec-spawn-hooks-buddy`,
`local-buddy-saves-2026-10-07`, `local-buddy-scenarios`; the only merge
conflict was one README table row each side added: both kept). Local PC,
user's own 1.14d install (`Game.exe` reference hash, checked by the tools
on every run). One game at a time under `C:\d2slots\game.lock`. Raw files
live only in `C:\Users\zffit\Desktop\D2test\traces-raw-buddy\` (never
committed); no normalized trace was committed because no spec asks for one
of these recordings in a fixed place.

## Tool changes (committed)

- `spawn.py` 0.2.0: `--level`, `--trigger`, `--status`, `--packets`
  (README "spawn.py"). Reason: a kill needs a field level (towns forbid
  attacks), spawns must wait until the player has walked out, several
  spawns per game are needed to level a character, and the kill and drop
  are visible only in the messages.
- `d2ui.py`: a one-action CLI to drive the window from a shell.

Driving: the game window takes `PostMessage` mouse / keys; target clicks
are computed from the `--status` snapshot (screen offset of a monster =
600 + 24 (dx − dy), 445 + 12 (dx + dy) in the 1200 × 900 client at 150 %
scaling, subtile deltas). The status snapshot also gave the act's level
rects (town of `bdBar`: level 1 at tiles (1200, 960) 56 × 40, Blood Moor
(1160, 1000) 96 × 56: the exit is on the town's +y side).

## Queue

| # | Item | Status | Raw file (local only) | Check | Key numbers |
|---|---|---|---|---|---|
| 1 | monster kill with item drop | **done** | `kill1-spawn.jsonl` (spawn-raw-1), `kill1-spawn-packets.jsonl` (packets-raw-1, 40,284 events) | `check_rng.py`: OK (46 inline draws, 0 unresolved, 7 `chain_unexplained`, 10 draw sites); `check_packets.py`: **6 failures**, all `s2c id 0x3e size 34 != rule 7 (R5)` (finding F1) | see "Item 1" |
| 7 | Kick / Bash on a monster | **Bash done; Kick blocked** | same files | as item 1 | see "Item 7" |
| 12 | weather-0001 (Rogue Encampment in rain) | **done (recorded; it rained)** | `wx1-frames.jsonl` (frames-raw-2 + `weather_pools`), PNGs `captures\20261007-084753\` (1,851) | `record_frames.py --selftest` ok; recorder verdict `stability: 0 state keys seen twice or more … NOT ENOUGH` (expected: rain, cursor and the player seed change every frame); no `weather-0001` case file exists to verify against | see "Item 12" |
| 13 | draw-order OQ7 (`TownE1` tile (950, 933) of run 2) | **blocked (cells not reachable on screen)** | `oq7-frames.jsonl`, PNGs `captures\20261007-085541\` (856; draws every 20th) | recorder verdict `NOT ENOUGH` (walking) | see "Item 13" |

## Item 1: kill with drop

`bdBar` (Barbarian, level 1 at start), `spawn.py --level 2 --trigger …
--status … --packets -- -w -ns -name bdBar -bar` (save kept: the
character reached level 2 and Save And Exit wrote `bdBar.d2s`, 987 bytes).
Walked out of the Rogue Encampment through the south gate into Blood Moor
(level 2) and killed with left clicks (skill 0, C→S 0x06 on the monster):

| Monster | Spawned (frame) | Killed (0x69 state 8 / 9, frame) | Drop (S→C 0x9C action 0, frame: GUID code) |
|---|---|---|---|
| 2 natural quill rats (class 63, GUIDs 12, 13) | — | 4251 / 4265; 4351 / 4365 | none |
| champion zombie (class 5, GUID 14, umod 16; 1 unit, 8 draws in the call) | 4686 | 5250 / 5274 | 5250: 9 `tkf`, 10 `hp1`, 11 `hp2` |
| champion fallen pack (class 19, GUIDs 15–17; 3 units, 23 draws) | 6261 | last 7338 | 6479: 12 `gld`, 13 `hp1`, 14 `mp2` |
| champion fallen pack (GUIDs 18–21; 4 units, 34 draws) | 6899 | last 7562 | 7338: 15 `ssd`, 16 `hp1`, 17 `mp2` |

The champion zombie's last hit was C→S 0x06 at frame 5243; its three
drops, the 0x69 state-8 message and the death are all in frame 5250, the
state-9 (dead) message in frame 5274. Items 11 and 10 were picked up (C→S
0x16 at frames 5615, 5938; then 0x9C action 14 / 4). Item codes were
read from the 0x9C bit stream (code at bit 141 for action 0). Per-spawn
seeds (game seed before / after the call, room seed) are in the
`spawn` records; the first: game `[329514667, 1578085098]` →
`[2160383105, 137438053]`.

## Item 7: Kick / Bash

- **Bash: done.** After the kills the Barbarian was level 2 (xp 699); the
  skill point went to Bash (C→S 0x3B `7e00`, frame 7878), Bash was set as
  the right skill (C→S 0x3C `7e000000ffffffff`, frame 8421) and used on
  two spawned normal zombies (class 5): C→S 0x0D (right skill on unit) on
  GUID 22 at frame 8813, killed at frame 8820 (one Bash; drop GUID 18
  `aqv`), and on GUID 23 at frame 9106 (state 8 at frame 9113).
- **Kick: blocked.** Kick (skill 1) is innate for every class
  (`charstats` Skill 2) but has no `leftskill` / `rightskill` flag, and
  the right-skill menu of the Barbarian shows no Kick icon (screenshot
  `out-rec\k24.png`): it cannot be selected through the UI. Recording it
  would need an injected C→S 0x3C select of skill 1 (`run_scenario.py`
  can inject, but cannot spawn yet) — a message no unmodified client
  sends, so it was not done.

## Town variants of the saves (spawn.py `--status`, `-nosave`)

Level 1 / level 2 rects (tiles) give the exit side: `bdAma`, `bdSor`,
`bdDru` town (1064, 928) 56 × 40, Blood Moor (1120, 920) 56 × 96 (east
exit; bridge seen: **`TownE1`**, start (5473, 4708)); `bdBar` (1200, 960),
Blood Moor (1160, 1000) (south gate); `bdNec` (768, 1032) / (824, 1024)
(east); `bdPal` (1024, 1208) / (968, 1160) (west); `bdAss`, `bdDead`
(1104, 1080) / (1064, 1120) (south); `bdMerc`, `ScnSor`, `ScnAma`
(864, 912) / (920, 904) (east). (`-name` loads the save's own map.)

## Item 12: weather-0001

`bdAma` (`TownE1`), `record_frames.py --seconds 420 --every 5
--draws-every 40 --weather` (new option, draw-order-2.md §11.1): 1,851
captured frames (every 5th in-game frame, server frames 2–9342), 232 with
the full draw list. **It rained**: rain flag on in every frame; target /
particle count rose to 255 (particles live in frames 6–4279), then the
rain cycle ended (target 0, particles 0 from about frame 4279); rain
phases 0–3 seen; lightning never on. Splashes were live in 159 frames
(frames 2980–3792, walking along the river bank and the bridge); bubbles
never. **W5 answered by observation:** every one of those 159 frames had
int(intensity) = 0 (intensity is at most 0.99609375 = 255/256), so
splashes do appear while int(intensity) is 0. Player seed per frame
(`seed_start`, `seed_end`): every one of the 1,851 transitions is reached
by stepping the D2 RNG (0 unresolved); steps per frame mostly 12 (426
frames; e.g. dry town), 0 (109), 25, 21, 4, and ~100–131 during full rain.
Example pair (frames 6 and 9, `frame-0000002.png`, `frame-0000003.png`):
seeds `[2268541034, 812154327]` → `[2669174633, 946191154]` and
`[658306235, 1655926658]` → `[1583635139, 1518167455]`. Not done: the
pass 4 / 9 pixel compare (no scene source is wired, `render-capture.md`)
and the water-floor count per frame (the draw records carry no floor
material).

## Item 13: draw-order OQ7

`bdAma`'s town is `TownE1` (bridge east of the start, Blood Moor east) at
origin (1064, 928), so the three cells local (48, 36…38) are level tiles
(1112, 964…966) (run 2's (952, 932…934) moved by the origin).
`record_frames.py --seconds 400 --every 10 --draws-every 20`, walked to
both sides: inside the town the south palisade (local y 28) stops the
player at subtile y 4780 (closest point tile (1111, 956)); the cells are
then ~10 tiles down-screen (dx + dy ≈ 10, the 800 × 600 view shows about
± 6.5); from the east bank in Blood Moor (tile (1122.4, 962.6), via the
bridge and south) they are ~10 tiles left (dx − dy ≈ −12, the view shows
± 5). The strip between the palisade and the river that holds the cells
was not reachable. Blocked: needs another seed / variant whose cells are
on a walkable spot (the survey counts such cells in every Act 1 town
variant; the other variants' cell positions are not in the specs), or a
camera-only capture.

## Findings

- **F1 (S→C 0x3E size).** Every `UpdateItemStats` (0x3E) message the
  server queued (`0x0053B280`, caller `0x53D200` in the sender
  `0x0053D130`) has queue size 34 while its length byte (u8@1, the rule
  in `specs/sim/server-messages.tsv`) is 7, e.g. frame 5085
  `3e071022b100…` (34 bytes, 27 trailing zeros). Six times in the run
  (frames 5085, 5116, 7091 …, while fighting). Either the spec's size
  rule describes the message as the client reads it and the queue size is
  a fixed 34-byte buffer, or the rule is wrong; owner
  `specs/sim/intents-events.md` / `server-messages.tsv`. Not loosened in
  the checker.
- Rain in the Rogue Encampment at game start for `bdBar` in both runs
  (screenshots `k01`, `k11`), stopped within a few minutes (Blood Moor
  screenshot `k19` dry, `k17` raining).
- `check_rng.py` reports 7 `chain_unexplained` draws on an OK verdict
  (five spawn calls in one game; the spawn-only runs of
  `local-buddy-spawn.md` had none listed): worth a look by the RNG owner.
