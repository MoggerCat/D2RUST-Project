# local-buddy: queue recordings (task `q-rec`)

Branch `claude/local-buddy-q-rec-2026-10-07` (from `origin/main` 674996d
+ `origin/claude/local-buddy-recordings-2026-10-07`). Local PC, the user's
own 1.14d install (reference `Game.exe` hash checked by the tools on every
run), one game at a time under `C:\d2slots\game.lock`. Raw files live only
in `C:\Users\zffit\Desktop\D2test\traces-raw-buddy\` (never committed).
Queue numbers are `docs/HANDOFF.md` §5 "Local run queue".

## Summary

| # | Item | Status | Raw file (local only) | Check | Key numbers |
|---|---|---|---|---|---|
| 85 | game +0x80 in 0x03 | **done: all three equal** | `join1.jsonl`, `join1-packets.jsonl` (5,995 events) | `check_packets.py` **OK** (no 0x3E) | `0x00546C60` result = game +0x80 = 0x03 u32@8 = **0xF74A29B4** |
| 83 | client room seeds | **recorded** (d2rs bridge replay is the cloud's) | same files | as 85 | 32 client monster creations, all at non-zero points; 32 / 32: the room's seed stepped exactly once and unit +0x28 = the new `lo'`; fresh client rooms hold `{x, 666}` |
| 80 | hireling teleport follow | **done** (town portal, both ways) | `tp80-packets.jsonl` (16,358 events; side file of `tp80-spawn.jsonl`, no spawn made) | `check_packets.py` **OK** (no 0x3E) | per teleport: the merc's 0x0A in the input phase of the C→S 0x13, its 0xAC at the destination in the next tick before the player's 0x15; a merc 0x15 one frame later only on the first teleport |

## 85: game +0x80 in 0x03

`record_join.py` (new, committed; subclass of `spawn.py` with no spawn),
`-w -ns -nosave -name bdAma -ama`, 60 s. The object control was built
from game creation (caller `0x00530DD1`, ECX = the game `0x467007C`)
before the first tick and returned EAX = 0xF74A29B4 (4,148,832,692). Game
+0x80 read at the first tick (frame 1) and when S→C 0x03 was queued
(frame 1, caller `0x0053B3BC`): 0xF74A29B4 both times. The 0x03 bytes
`03 00 7389055a 0100 b4294af7`: act 0, u32@2 = 0x5A058973, u16@6 = 1 (town),
u32@8 = **0xF74A29B4**. All three equal, as `client/model.md` §11 r1
expects (d2rs: `ObjectState::obj_seed`; the app's game sends 0, the known
wiring gap).

## 83: client room seeds versus the server's

Same run. `0x00466360` (client monster create, S→C 0xAC) was entered 32
times in the first two server frames of the join (all creations of the run;
standing in town): GUIDs 1–7 (classes 147, 150, 152 × 3, 154, 155: the
town NPCs) then class 149 units (GUIDs 2–5, 47–52, 94–102, 129–134; GUID
numbers 2–5 reused), every one at a non-zero point.

- **One step per creation at a point (rule 6 / §12 r5): confirmed, 32 / 32.**
  Of the 35 client active rooms exactly one changed its seed per creation,
  the room of the unit's path; after = one D2 RNG step of before, and the
  unit's init seed (+0x28) = the new `lo'`.
- A client room's seed before its first creation is `{x, 666}` (an
  `init_low` value), e.g. the first unit GUID 6 class 154 at (5434, 4697):
  room `0x464D100` rect (5400, 4680, 40, 40), seed `[184151889, 666]` →
  `[2878913519, 76808347]`, unit init seed 2878913519, unit seed (stepped
  further after creation) `[1192825221, 1200772860]`.
- The server unit of the same GUID: seed `[2775221797, 194707121]`, init
  seed 1443133650; its room (same rect) seed `[3816720766, 1313571139]`.
  For the five rects where both are known, the server room's seed is not
  reached from the client room's first `{x, 666}` within 5,000 steps.
  This is an observation for OQ 9 (the server room may be seeded or stepped
  by other draws), not a conclusion: the d2rs bridge replay (live
  `DrlgSource` + the recorded stream in `join1-packets.jsonl`) is what the
  entry asks to compare.
- To replay: `join1.jsonl` `cl_monster` records hold, per creation, the
  unit (GUID, class, point, seed, init seed, room), all 35 rooms' rect and
  seed before and after, and the server unit; the S→C stream (0x03 seed
  0x5A058973, the 0x07s, the 0xACs) is in `join1-packets.jsonl`.

## 80: hireling teleport follow

`bdMercTwo` (level 8 Barbarian, hireling Diane alive) with `spawn.py
--packets --status --trigger <never made> -- -w -ns -nosave -name
bdMercTwo -bar` (no spawn: the trigger file was never created, so the run
is a plain packets recording; `-nosave`, the save is unchanged). The 0x81
of the reload (frame 2) gives the hireling GUID **1** (type 1, class 271).
Walked from the Rogue Encampment (level 1, tiles (832, 1128) 56 × 40) east
into Blood Moor, read the Scroll of Town Portal the save already had
(screenshot tooltips), entered the portal (C→S 0x13 type 2 GUID 0x1B,
frame 2918), then went back through the town side (0x13 GUID 0x1C, frame
3359). The hireling arrived next to the player both times (screenshots
`out-q-rec\a07.png`, `a08.png`).

| | Blood Moor → town | town → Blood Moor |
|---|---|---|
| C→S 0x13 (portal) | frame 2918, input | frame 3359, input |
| merc removed: S→C 0x0A `0a0101000000` (caller `0x53BDC2`) | 2918, input phase, right after the 0x13 (after one 0x07) | 3359, input phase, right after the 0x13 |
| room messages | tick 2919: 0x07s / 0x51s / 0xACs of the town rooms, then the 0x08s and 0x0As of the old rooms | tick 3360: the 0x0As of the town units (no 0x07 in that tick) |
| merc added: S→C 0xAC GUID 1 class 0x10F (caller `0x53E81E`) | tick 2919 at (4333, 5743), **before** the player's 0x15, after the town NPC 0xACs | tick 3360 at (4471, 5721), **before** the player's 0x15 |
| player S→C 0x15 type 0 GUID 1 (caller `0x53BC44`) | tick 2919 to (4333, 5743), after the room hides | tick 3360 to (4471, 5721) |
| merc S→C 0x15 type 1 GUID 1 (caller `0x53BC44`) | tick **2920** to (4333, 5743) (the same point) | none |

So the follow's effect reaches the client as the hireling's 0xAC at the
player's destination point inside the tick that moves the player, queued
before the player's own 0x15; on the first teleport a 0x15 of the
hireling to that same point followed one frame later (not on the second).
The 0x0A that removes the hireling is queued in the input phase of the
interact, before the tick. For d2rs ("the follow runs when the handler
returns", `hirelings.md` §6 r1, `path-placement.md` §10 r6): compare
these positions in the message order. Not settled here: why only the
first teleport had the extra hireling 0x15 (its placement point equalled
the player's both times).
