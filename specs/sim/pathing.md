# Spec: Simulation — Walk and run: mode request, path finding, per-tick movement

- **Status:** draft: every rule read from the 1.14d `Game.exe` code
  (addresses below; D2MOO 1.10f D2Common `Path.cpp`, `PathMisc.cpp`,
  `Step.cpp`, `AStar.cpp` and D2Game `PlrModes.cpp` used as a map,
  differences noted); `sim/path-tables.tsv` equals the executable
  (`py tools/trace-recorder/path_tables.py`, 582 rows, perturbation
  self-test passes). Recordings confirm the message side only (Test
  vectors R4–R6): no movement message reaches the walking player's own
  client. Per-tick positions are unrecorded (open question 1).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::path::walk` (mode request, path compute,
  path types 1/2/7, step), `d2-sim::path` (records: `sim/path-placement.md`)
- **Related specs:** `sim/path-placement.md` (coordinates, path records,
  collision queries and footprints, placement, warps);
  `sim/intents-events.md` §2.4 rules 3–4 (C→S 0x01–0x04 parsing and
  range checks: owner); `sim/units.md` §4.4–§4.6 (mode set, every-tick
  event 0, player and monster event dispatch); `sim/tick.md` §3 step 4,
  §5 (when event 0 runs), §6 (client pass); `sim/stats.md`,
  `sim/stat-lists.md` (stat reads, the run stat list); `sim/rng.md` §3
  (`roll`); `drlg/rooms.md` §6 (adjacency); `sim/unit-order.md` §5–§6
  (room lists, update queue). Machine table: `sim/path-tables.tsv`.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 52–67 |
| Inputs | 68–77 |
| Outputs / state changes | 78–84 |
| Rules | 85–86 |
|   1. Walk and run requests | 87–234 |
|   2. Path types | 235–274 |
|   3. Path compute (`0x00649970(path, unit, town access)`) | 275–341 |
|   4. Target preparation (flag 0x1000, `0x00648120`) | 342–358 |
|   5. Toward (type 2, `0x00679C80`) | 359–426 |
|   6. Straight (type 7, `0x00679ED0`) | 427–436 |
|   7. A* (type 1, `0x0067B850`) | 437–474 |
|   8. Velocity, direction vector, facing | 475–561 |
|   9. Per-tick movement | 562–745 |
|   10. Messages | 746–808 |
|   11. Missile paths (`0x00649760`) | 809–861 |
|   12. Other path types (1.14d-read 2026-10-08) | 862–1075 |
|   13. Path accessors and the cell line test (1.14d-read 2026-10-08) | 1076–1225 |
| Constants & data dependencies | 1226–1262 |
| Randomness | 1263–1273 |
| Edge cases & original bugs | 1274–1321 |
| Test vectors | 1322–1359 |
| Provenance | 1360–1415 |
| Open questions | 1416–1489 |
<!-- /index -->

## Summary

A walk or run request (C→S 0x01–0x04) that passes the transport checks
asks for player mode 2/6 (walk/town walk) or 3 (run) toward a point or a
unit. If the player's current mode allows it, the target is stored and a
path is computed at once: players use path type 7, a straight-line ray
test, then a greedy 8-direction walk around obstacles (up to 73 steps),
then, for targets within 18 sub-tiles, a bounded A* (200 nodes). A path
is a list of up to 77 corner points. If the path is non-empty the mode is
set and an every-tick event 0 moves the unit in timer step 4 of each
tick: a 16.16 fixed-point step of (velocity × direction / 256) per tick,
crossing cells one at a time with a footprint move per cell; a blocked
cell stops the unit at the centre of its last free cell; reaching the
last point stops it and returns it to neutral. The walking player's own
client receives no per-step message.

## Inputs

| Name | Type | Source |
|---|---|---|
| request | mode 2 or 3; target point (x, y) or unit (type, GUID) | `intents-events.md` §2.4 rules 3–4 |
| unit | player or monster with a dynamic path | `sim/path-placement.md` §2.3 |
| collision grids | room grids | `sim/path-placement.md` §4 |
| stats | 10 stamina, 67 velocitypercent, 96 item_fastermovevelocity, 154 item_staminadrainpct | `sim/stats.md` |
| tables | charstats `WalkVelocity`, `RunVelocity`, `RunDrain`; armor `speed`; monstats `Velocity` | `data/fields.tsv` |

## Outputs / state changes

Path fields (`sim/path-placement.md` §2.3), unit mode and timer events
(`sim/units.md` §4), unit position, room and room lists, footprints,
stamina stat, update-queue entries, S→C 0x0F / 0x10 / 0x15 / 0x0D in the
update pass.

## Rules

### 1. Walk and run requests

#### 1.1 From the message to the mode request

After `intents-events.md` §2.4 accepts the message (point: rule 3; unit:
rule 4), the handler calls:

| Id | Handler | Call |
|---|---|---|
| 0x01 walk to point | `0x005497E0` | `0x005809D0(no skill, mode 2, x, y, re-entry 0)` |
| 0x03 run to point | `0x005498D0` | `0x005809D0(no skill, mode 3, x, y, 0)` |
| 0x02 walk to unit | `0x00549890` | `0x00580A70(no skill, mode 2, type, GUID, 0)` |
| 0x04 run to unit | `0x00549920` | `0x00580A70(no skill, mode 3, type, GUID, 0)` |

Both return 0 to the dispatcher whether or not the mode starts.

#### 1.2 Player mode request (`0x005809D0` point form, `0x00580A70` unit form)

1. Game null → fatal. Unit form: the target unit is looked up by type
   and GUID (`0x00552F60`); none → a log line, nothing else.
2. Mode check `0x0057EDD0` (rule 1.3) fails → nothing.
3. Unless re-entry: check `0x0057EEC0(mode, 1, skill)` (rule 1.4) fails →
   nothing.
4. Unit form, player: player data +0x154 := 0 and +0x150 := 0.
5. Start function of the mode (table `0x006E1740`: point form at
   +8·mode, unit form at +8·mode + 4; a null entry is fatal). If a skill
   was given it becomes the used skill first. Walk/run/town-walk/
   knockback (modes 2, 3, 6, 19): point form `0x0057F1F0` sets the path
   target (`0x00648AD0`: target x, y; target unit := none); unit form
   `0x0057F190` sets the target unit (`0x00648B90`: unit, its type and
   GUID), except that a knockback request on a unit already in mode 19
   does nothing. Both then run §1.5.

d2rs wiring without its path provider (a d2rs host setup, no 1.14d
counterpart) runs no mode request: design choice, no fidelity rule.

#### 1.3 Mode check (`0x0057EDD0`, requested mode m)

1. If the player's inventory has a cursor item (inventory +0x20 ≠ 0)
   and m ≠ 0: allowed only for m = 17.
2. m ∈ {0, 1, 5, 17}: allowed.
3. Otherwise by the current mode (unit +0x10):

| Current mode | Allowed when |
|---|---|
| 0 DT, 4 GH, 9 BL, 17 DD | never |
| 7 A1, 8 A2, 10 SC, 11 TH | frame ≤ E + 5, or m = 4, or m = 9 |
| 12 KK | frame ≤ E + 5 |
| 13 S1 | class ≠ 0 (not Amazon) |
| 15 S3 | class ≠ 5 (not Druid) |
| 18 SQ | the used skill's skills.txt row has `SeqInput` (+0x16) > 0, else frame ≤ E + 5 |
| 1, 2, 3, 5, 6, 14, 16, 19, > 19 | always |

E = the smallest positive expire among the unit's type-1 timers (0 if
none; `0x005415A0`); frame = game +0xA8.

#### 1.4 Interrupt check (`0x0057EEC0`, mode m, flag 1, skill s)

1. Unit has state 54 (`0x36`), or is null, or is in mode 0 or 17 →
   refuse.
2. m = 0 → allow. No used skill (`0x00620250`) or no skills.txt row →
   allow.
3. s = the used skill and the row's `srvdofunc` (+0x2E) is 67 or 76 →
   allow.
4. Row flag `interrupt` clear (byte +7 & mask `0x006CE284`): current
   mode 1 → re-enter neutral (`0x005809D0(no skill, 1, 0, 0, re-entry
   1)`) and allow; else allow only for m ∈ {7, 8, 10, 11, 13, 18} with
   frame ≤ E + 5 (E as in §1.3); refuse otherwise.
5. Flag set: if the unit has state 42 (`0x2A`, concentration): v = stat
   164 of that state's list (`0x00625D00`); **draw `roll(100)` on the
   unit seed** (unit +0x20, `0x0045C390`); r < v → go to rule 6. A
   failed roll (r ≥ v), or no state 42, goes on to the state 15 test:
   state 15 (`0x0F`) → rule 6; else allow.
6. Current mode 1 → re-enter neutral as in rule 4 and allow; else refuse.

#### 1.5 Starting the movement (`0x0057F090(mode)`)

1. Path type reset (`0x00648DC0`): flag 0x8000 → velocity := saved
   velocity (+0x80); player → type 7; other units with flag 0x2000 →
   the previous type (+0x40).
2. Mode: 3 (run) with stamina (stat 10) = 0 → treated as walk. Walk (2),
   town walk (6), or a stamina-less run → 6 if the unit's room is in a
   town (`0x0061AB00`), else 2. 3 with stamina stays 3. 19
   (knockback): path type 8 (`0x00648CF0`), distance budget (+0x90) := 5.
3. Path compute (§3) with town access 0.
4. Used skill := none (`0x00620210`).
5. Point count (+0x28) = 0 → neutral start `0x0057F020` (cancel event
   types 0 and 1, mode 5 in a town room else 1, used skill none) and
   stop.
6. Mode set `0x00553570(mode)` (`sim/units.md` §4.1; for 3 and 19 the
   animation setup attaches the run stat list, §7.2), cancel types 0
   and 1 (`0x00553990`), every-tick event 0 (`0x00553F00`,
   `sim/units.md` §4.4).

A new request while moving recomputes the path from the current
precise position (§3); the fraction is kept.

#### 1.6 Client position resync (C→S 0x5F, `0x0054CD50`)

The message (5 bytes: id, x u16, y u16; `sim/client-messages.tsv`)
carries the position the client believes its player is at. Handler
`0x0054CD50(game, player, msg, len)`:

1. len ≠ 5 → result 3. The player's client (`0x005531C0`: player data
   +0x9C) is required (null → fatal).
2. Ignored (result 0) when a used skill is set (`0x00620250`), the
   player is dead (`0x005541B0`), or d < 5, where d =
   `0x006417F0(player, x, y)` = max(|dx|, |dy|) + ⌊min(|dx|, |dy|)/2⌋
   between the player's sub-tile position and (x, y).
3. **Walk branch**, when the player has state 108 or d < 15 or d > 45:
   if the path has a target unit other than the player (`0x00553540`,
   after its validity check `0x00553490`) → result 0; else the mode
   request of §1.2–§1.5 (`0x005809D0(no skill, m, x, y, 0)`) with m = 3 if
   the player's mode is 3 (run), else 2.
4. **Snap branch**, 15 ≤ d ≤ 45 without state 108:
   1. Reachability test (`0x0054CB10`): (x, y) must have a room by the
      cell lookup from the player's room (`sim/path-placement.md` §4
      rule 1) and the player must have neither state 54 nor 108; else
      not reachable. Save the path's move-test mask (+0x50), path type
      (+0x3C), max path distance (+0x91) and target (unit, else point);
      set mask 0x409, type 15, +0x90 = +0x91 = 77, target := (x, y);
      compute (§3, `0x00649970` with town access 0). Reachable iff the
      compute returns non-zero and the last path point (index count −
      1) equals (x, y). Restore mask, type, the target, and +0x90 =
      +0x91 := the saved +0x91. The computed points and count are
      **not** restored.
   2. Reachable → placement (`0x0054CC40`): `0x00554EA0(room 0, x, y,
      exact 0, alt 1)` (`sim/path-placement.md` §10); failure → step 3.
      Success: record the game frame (game +0xA8) in the client's
      5-slot resync ring (+0x3C0, `0x00539360`: the first slot that is
      0 or more than 2250 frames old; none → nothing recorded). Delay
      := 125 frames, unless the ring is full (`0x005393F0`: all five
      slots non-zero and at most 2250 frames old) and the game type
      byte (game +0x6A) is 0: then r = `lo' % 100` (one inlined step
      of the player's unit seed, unit +0x20, `sim/rng.md` §6) picks
      from table `0x006E1064`
      (r < 50 → 1500, r < 75 → 3000, else 4500 frames). Set state 108;
      attach a stat list (`0x006251F0` flags 2, expire = frame +
      delay; state 108; remove callback `0x0054CC30` clears state 108)
      and schedule event 12 at frame + delay (`0x005417D0`,
      `sim/stat-lists.md` §10.4). Result 0.
   3. Not reachable, or placement failed → S→C 0x15 to the client
      (`0x00548010(player, client, 0)`, §10). Result 0.

So a player whose client keeps snapping (five snaps within 2250
frames) is locked to the walk branch for 1500–4500 frames, otherwise
for 125.

### 2. Path types

Type table, index = path type (`sim/path-tables.tsv` tables
`pathtype_flags`, `pathtype_diroff`; functions `0x006EB6D8`):

| Type | D2MOO name | Function | Flags | Used by |
|---|---|---|---|---|
| 0 | IDA* | `0x0067AD00` | 0x21900 | monster AI (owner: AI spec) |
| 1 | A* | `0x0067B850` | 0x1900 | §7 |
| 2 | toward | `0x00679C80` | 0 | monsters' default, §5 |
| 3 | toward (restart) | `0x00679E50` | 0 | |
| 4 | missile | `0x006492F0` (§11.1) | 0x60000 | `missiles/missiles.md` |
| 5, 6 | circle CW / CCW | `0x00679C80` | 0 (offset +2 / −2) | monster AI |
| 7 | straight | `0x00679ED0` | 0x21900 | players, §6 |
| 8 | knockback (server) | `0x0067A000` | 0x1E600 | player mode 19 |
| 9 | leap | `0x00679F70` | 0x1E800 | skills |
| 10, 14 | charged bolt, blessed hammer | `0x0067A240`, `0x0067A140` (§11.2, §11.3) | 0x60000 | missiles |
| 11 | knockback (client) | `0x00679FD0` | 0x1E604 | |
| 12 | back-up turn | `0x00679E60` | 0 (offset −4) | monster AI |
| 13 | toward, finish | `0x00679C80` | 0x100 | monster re-path |
| 15 | wall follow | `0x0067C2D0` | 0x800 | monster re-path |
| 16 | IDA* | `0x0067AD00` | 0 | |
| 17 | none | — | 0 | |

Set type (`0x00648CF0(path, t)`): a player's path never takes type 2
(the call is a fatal assert); if the new flags have 0x2000 and the path
lacks 0x4000, previous type := current type; if they have 0x8000 and the
path lacks 0x10000, saved velocity := velocity; flags := (flags &
~0x7FF00) | table flags; type := t; direction offset (+0x98) := table
offset; a previous type of 11 or 8, or t = 4 with max distance ≥ 78, is a
fatal assert.

Flag meanings used here: 0x100 (no rule in this spec), 0x800 remove the
target unit's footprint while computing (§3 step 6), 0x1000 prepare a
blocked target (§4), 0x20000 store saved steps (§9.4). Types 4, 10 and 14
carry flag 0x40000 and are computed by §11; reaching §3's function table
without that flag (it has no entry for 4, 10, 14, 17) is fatal. The types used only
by monster AI and skills (0, 3, 5, 6, 8, 9, 11, 12, 13, 15, 16) are named
here and specified with their callers (open question 3).

### 3. Path compute (`0x00649970(path, unit, town access)`)

1. Path null → result 0. Owner ≠ unit → fatal. Missile flag (0x40000) →
   missile path (`0x00649760`, §11).
2. Collided mask (+0x54) := 0. With a target unit whose position is
   (0, any) or (any, 0) → result 0 (nothing reset).
3. Start = the current sub-tile (x, y); (0, 0) → step 11.
4. Target refresh (`0x006498A0`, result r, "target slack", default 1):
   with a target unit, target (+0x10/+0x12) := its position; (0, 0) →
   step 11 with r = 0. By the target's type: player or monster: if the
   path lead byte (+0x68) ≠ 0, the target point is moved ahead
   (`0x00679190`, rule below), r = 1; object:
   doors (`0x00621A70`) shift the target 2 sub-tiles away from the
   door's line toward the unit's side (orientation `0x00621AC0` set:
   y ± 2, else x ± 2; − when the unit's coordinate is smaller) and
   r = 2; other objects r = 2; item r = 2; others r = 1.
5. Target (0, 0) → step 11. Start = target → step 11. |tx − sx| > 100 or
   |ty − sy| > 100 → step 11. Start room := path room; target room :=
   cell lookup from it (`sim/path-placement.md` §4 rule 1); either null
   → step 11. Town access 0 and `0x006483A0` refuses (a monster that
   cannot be in town, `sim/path-placement.md` §3, whose target room, or
   the room found from the midpoint when none, is in a town) → step 11.
6. Remove the unit's own footprint (`0x00649560(unit, 0)`); with a
   target unit of non-zero size and path flag 0x800, remove its
   footprint too (remembering whether it was removed).
7. If flag 0x1000 and the target collides (pattern query `0x0064D910`
   with the path's pattern and move mask, from the start room):
   target preparation (§4); result 0 → no path function runs.
8. Index := 0, count := 0, final target (+0x18) := target; run the type's
   function (§2) unless step 7 said not to; count := its result.
9. Put back the target's footprint (if removed), then the unit's
   (`0x00649400`). Count 0 → step 12.
10. Velocity and direction toward point[0] (`0x0064FE40`, §8.2; drops
    leading points equal to the position). If points remain: room-exit
    flag (`0x00647FB0`: flag 0x1 := some point lies outside the path
    room's sub-tile rect, tested in order until the first outside point);
    previous target := target; +0x38 := 0; if flag 0x10 is clear and
    there is no target unit: target := the last point; flag 0x20 := 1;
    result = count. If no point remains → step 11.
11. Index := 0, count := 0.
12. Flag 0x20 := 0; result 0.

**Target lead** (`0x00679190(path)`, and `0x00679250(out, path)`
which writes the led target point to `out` and skips the lead when
+0x68 = 0): with the target unit's direction d (`0x006487F0` on the
target's path, +0x64) and lead L = path +0x68 (u8), a := (8·d −
256·L) & 0x1FF; target x += trunc(L · T[(a + 0x80) & 0x1FF]), target y
+= trunc(L · T[a]) (16-bit wrapping adds; trunc toward zero, x87
control word with RC = 11), T = the 512-entry f32 sine table at
`0x00707800` (`0x0040B330` cosine, `0x0040B350` sine). **In 1.14d L is
always 0**: the only writer of +0x68 is `0x006490C0` (D2MOO
`D2Common_10207`, also writes +0x67), which has no call and no
pointer anywhere in the image (`disasm.py xref`), and the path record
is zeroed at allocation (`sim/path-placement.md` §2.3). So the lead
adds 0 and the floating point never affects an outcome; d2rs
implements "lead = 0" (no table, hard rule 6 holds). Were it needed:
T is not the f32 rounding of sin(2πi/512) (328 of 512 entries differ
by one ulp; sha256 of the 2048 bytes
`22d4615dd85e77c049244da1033035af272eedf66ea63ef3476cb7e397afaf54`),
and trunc(L·T[i]) is the same under 24-, 53- and 64-bit x87 precision
for all L ≤ 255 and all i (exhaustive check), so an integer form from
T's bit patterns would be exact.

Path functions receive a record ("path info", D2MOO `D2PathInfoStrc`):
start, target, start room, target room, slack r (step 4), max distance
(path +0x91), IDA* score (+0x92), type, size, pattern, move mask.

### 4. Target preparation (flag 0x1000, `0x00648120`)

1. Missile path → fatal. Three probes p0 = p1 = p2 := target; directions
   (d0, d1, d2) := `altdir`[o(target → start)] (§5.1 octant).
2. Loop:
   1. p0 = start → result 0.
   2. p0 += step(d0); free (pattern query = 0) → found p0.
   3. p1 += step(d1); free → found p1.
   4. d2 ≠ 255: p2 += step(d2); free → found p2.
   5. d0 := `altdir`[o(p0 → start)] first entry (d1, d2 unchanged); repeat.
3. Found p: target := p; p = start → result 0. Player (unit type 0):
   orthogonal push (rule 4). Result 1.
4. Push (`0x00648050`): (dx, dy) = target − start; if |dx| < 5, |dy| < 5
   and not both 0: ox = `snap9`[40 + dx + 9·dy], oy = `snap9`[40 + dy +
   9·dx]; c = max(|dx|, |dy|); while c < 5: candidate := target + (ox,
   oy); if it collides (pattern query) stop; target := candidate; c += 1.

### 5. Toward (type 2, `0x00679C80`)

#### 5.1 Helpers

1. Octant o(p → q) (`0x00678C10`): dx = qx − px, dy = qy − py; with
   ax = |dx|, ay = |dy|:
   - if ax < 2·ay and 2·ax ≤ ay: dx < 0 → o = 7 + clamp(dy) where
     dy < −1 gives o = 5; else dx := dx & 1 and continue below;
   - if ax ≥ 2·ay: dy := −1 if dy < 0, else dy & 1;
   - then dx := clamp to [−2, 2]; dy < −1 → o = 5·dx + 10; else o =
     5·dx + 12 + min(dy, 2).
   o is a row 0..24 of `testdir` and `altdir`.
   "clamp(dy)" is the same [−2, 2] clamp as dx (`0x00678C79`–
   `0x00678C94`: dy < −1 → −2, dy > 1 → 2), so every branch returns o =
   5·dx' + dy' + 12 with dx', dy' ∈ [−2, 2]; the first branch has dx' =
   −1 unclamped. In that branch ay ≥ 2·ax ≥ 2, so dy' is −2 or 2 and o
   is 5 or 9. The middle case (ay < 2·ax and ax < 2·ay) changes neither
   value before both clamps. Vectors (synthetic): (dx, dy) = (−1, −2) →
   5; (−1, 5) → 9; (1, 2) → 1·5 + 2 + 12 = 19 (dx := 1 & 1); (3, 2) →
   2·5 + 2 + 12 = 24 (middle case); (4, −1) → 2·5 − 1 + 12 = 21.
2. Step of a direction d (`dir8_toward`, `0x006F1798`): 0 (1,0), 1 (1,1),
   2 (0,1), 3 (−1,1), 4 (−1,0), 5 (−1,−1), 6 (0,−1), 7 (1,−1).
3. Path distance (`0x00679380`): ax = |Δx|, ay = |Δy|; both < 8 →
   `dist8_path`[ax + 8·ay], negative → 0, else value + 1; otherwise
   2·max + min.
4. Ray test (`0x00679720`, start s, end e, from the path: room, pattern,
   move mask; result clear, or blocked with e := the last cell before
   the blocking one): Nx = |Δx| + 1, Ny = |Δy| + 1, steps ±1 by sign
   (0 counts as +).
   - Nx > Ny: e.x = s.x → clear. err := Ny; repeat: remember the cell;
     x += sx; test (x, y) → blocked; err += Ny; if err ≥ Nx: y += sy,
     err −= Nx, and if err > 0 test (x, y) → blocked; x = e.x → clear.
   - Nx = Ny: repeat: remember; x = e.x → clear; x += sx, y += sy; test
     → blocked.
   - Nx < Ny: the mirror (y major; err starts at Nx).
   "Test" = pattern query (`0x0064D910`) from the path room. The
   remembered cell is the one at the top of the iteration, so a block
   found after the minor step returns the cell before the major step.
5. Next-position check (`0x00679A60`, point P := the target): velocity
   0 → P := position, "blocked". Velocity and direction toward point
   index 0 (§8.2, which sets the facing); velocity 0 → P := position,
   "blocked". Else ray test from the position to P.

#### 5.2 Algorithm

Index := 0, count := 0. Direction offset ≠ 0 (types 5, 6, 12) →
`0x00679B30` (monster circling, §12.1) and its result is returned.

1. points[0] := target; next-position check. Clear → points = [target].
2. P = the returned point. dist(P, target) ≤ slack → points = [P].
3. cur := start; if P ≠ start: points[n++] := P, cur := P.
4. Greedy walk, at most `max distance` (+0x91: 73 players, 14 monsters)
   iterations, prev := 255, steps := 0, turned := 0:
   1. cur = target → leave the loop.
   2. turned := 0. (t0, t1, t2) := `testdir`[o(cur → target)]. The first
      of t0, t1, t2 whose step from cur is free (pattern query) is d; t2
      = 255 at this point is a fatal assert. None free, or d is the
      reverse of prev ((d − 4) & 7 = prev) → **tail**.
   3. If d ≠ prev: points[n++] := cur unless cur = start; turned := 1.
   4. cur += step(d); steps += 1; prev := d.
5. After the loop: turned = 1 → done. Else **tail**: steps ≠ 0 →
   points[n++] := cur.
6. count := n.

A turn appends the corner it starts from, so a path is the list of
corners; after step 3 the first greedy turn appends P a second time
(edge case 3).

### 6. Straight (type 7, `0x00679ED0`)

1. Index := 0, count := 0. n := Toward (§5).
2. n > 0 with last point L: dist(L, target) ≤ slack and L ≠ start →
   result n.
3. If dx² + dy² ≤ 324 (start to target, |Δ| per axis): A* (§7); a
   non-zero result is returned. A* 0 and n ≠ 0 → Toward again (§5; its
   result is returned).
4. Else result n.

### 7. A* (type 1, `0x0067B850`)

1. Target room check (`0x0067B740`), only with a target unit: some cell
   of target + (−2,−2), (−2,+2), (+2,−2), (+2,+2), (−2,0), (0,−2),
   (+2,0), (0,+2), tested in that order, must be free (pattern query);
   none → 0.
2. Nodes: storage for 200 (the start included); node = point, g (cost
   from start), h, f = g + h, parent, up to 8 children, list links.
   h(p) = 2·max(|Δx|, |Δy|) + min(...) to the target. Cost of a step:
   2 straight, 3 diagonal (same x or same y → 2).
3. Open list sorted by f ascending: a new node is inserted **before** the
   first node with f ≥ its f. Closed list: popped nodes. (Two 128-bucket
   hash tables, `0x006F1998`/`0x006F1B98`, only speed up membership
   tests; they do not change results.)
4. Start node: g 0, h = f = h(start); open it.
5. Loop: pop the first open node c (to closed). Best := c if best is
   none, or h(c) < h(best), or h equal and g(c) > g(best) + 5. h(c) = 0
   → stop. Expand c over neighbours in the order (−1,−1), (−1,+1),
   (+1,−1), (+1,+1), (−1,0), (0,−1), (+1,0), (0,+1); a neighbour that
   collides (pattern query from the start room) is skipped. For a
   neighbour q with g' = g(c) + cost:
   - q open: add q to c's children (first empty slot of 8; none free →
     not added); if g' < g(q): parent := c, g, f updated (the open list
     is **not** re-sorted).
   - q closed: add as child; if g' < g(q): update, then propagate with a
     stack (≤ 200 entries): pop a node, for each child in slot order up
     to the first empty slot: if g(node) + cost < g(child): update the
     child and push it.
   - else: storage full → stop the whole search; else a new node (g',
     h(q), parent c), child of c, inserted into the open list.
   The loop also stops when the open list is empty.
6. Result from best (none → 0): walk the parent chain from best to the
   start (the start is not output); a node is output when its step from
   its parent differs from the previously output step (first output
   always); at 78 outputs the walk stops. Points = the outputs in
   start-to-best order; count 1..77 → copied to the path, result count;
   0 or 78 → result 0.

### 8. Velocity, direction vector, facing

#### 8.1 Velocity (path +0x7C)

Set by the animation-rate routine `0x00623F50` (called by every mode
set and animation start of players and monsters with a path; the
animation-speed half of it belongs to the future animation-rate spec,
`sim/units.md` §4.3):

1. Knockback (player mode 19, monster mode 13): velocity := 0x1000.
2. Modes with the velocity modifier (`0x006214A0`): monsters whose
   monstats `npc` bit is set: modes 2 and 15 only. Otherwise the mode's
   row of `velmod_player`, `velmod_monster` (class < 410) or
   `velmod_monster_x` (class ≥ 410): column b ≠ 0, or column a ≠ 0 and
   the used skill's flags (`0x006446A0`, skill +0x0C) have bit 0x1 and
   not 0x1000. Column b: player modes 2, 3, 6; monster modes 2, 15
   (and 8–11 for classes < 410). "Skill +0x0C" is not a `skills.txt`
   column: `0x006446A0` returns word +0x0C of the used skill **entry**
   (`0x00620250`), the runtime E-flags word written by the skill start
   and bodies (`0x00644660`; `skills/use.md` §5.2 step 2: 1 = moving
   skill, 2 = move ended); no used skill → 0. Then p = f + stat 67 (unit total `0x00625480`; a player's
   base is 100 from creation, `combat/vitals.md` §1), at least 25, where f = stat 96
   (item/skill getter `0x00625500`) scaled by `animstat` row 4: f ≠ 0 →
   150·f / (150 + f) (truncated). Velocity := base · p / 100
   (truncated), base = charstats `WalkVelocity` × 256 for players,
   monstats `Velocity` × 256 for monsters (`0x00621360`; the mode does
   not change the base).
3. Velocity write (`0x00648690`): +0x38 := 15 only when the new value
   differs from the current velocity; velocity and max velocity (+0x84)
   := the new value always.
4. Any other mode (no velocity modifier, not knockback): the routine sets
   only the animation rate; the velocity keeps its previous value.

#### 8.2 Run

Starting mode 3 or 19 attaches a temporary stat list (`0x00620E80`,
flag 4, freed at the next mode set) with stat 67 = 100 · `RunVelocity`
/ `WalkVelocity` − 100 (truncated; 1.14d live: 100·9/6 − 100 = 50); the
list is skipped when `WalkVelocity` is 0. Rule 8.1.2 then reads it.

#### 8.3 Direction vector (`0x0064FC60`)

From precise start (sx, sy) to precise point (tx, ty) (`tan` table:
128 rows {x, y, angle}, x² + y² ≈ 4096²):

1. Lx = |tx − sx|, Ly = |ty − sy| (unsigned differences). Lx ≤ Ly:
   i = 127·Lx / Ly (0 when Ly = 0), (vx, vy) := (row x, row y), a := row
   angle. Lx > Ly: i = 127·Ly / Lx, (vx, vy) := (row y, row x), a := (−1
   − angle) & 15. Division signed, truncated.
2. ty < sy: vy := −vy, a := (−1 − a) & 31.
3. tx < sx: vx := −vx, direction := (a + 8) & 63. Else direction :=
   ((−1 − a) & 63) + 8) & 63.
4. Path flag 0x200: direction := (direction − 32) & 63 (`0x0064FED5`).

#### 8.4 Velocity and direction toward the next point (`0x0064FE40`)

1. While point[index] (cell centre) equals the position: if index ≥
   count − 1: direction and velocity vectors := 0, velocity := 0, index
   := count, return; else index += 1.
2. Direction vector (rule 8.3) to point[index]; velocity vector =
   (direction vector · velocity) >> 8 (arithmetic).
3. "Arrives next step" = the point's cell equals the cell of position +
   velocity vector on both axes. Facing update (`0x006485F0`) when the
   caller asks always (all callers in this spec do).

#### 8.5 Facing (`0x006485F0(path, d)`)

d &= 63. Unit type 2 or 4, or a missile without path flag 0x40:
direction := d. A missile with flag 0x40: nothing. Others (players,
monsters): if d ≠ new direction (+0x65): new direction := d, turn step
(+0x66) := `dirdiff`[(d − direction) & 63]. `0x00648820(path, d)` is
the same with d ≥ 64 → fatal and a missing owner read as type 6.

Turning the current direction toward the new one (`0x00648640`, D2MOO
`D2Common_10193`: direction := (direction + turn step) & 63, snapped to
the new direction once it is passed) is called only from the client's
unit update (`0x00480810` → `0x00463390` players, `0x004B13A0`
monsters). **The server never turns**: of the path functions that
write +0x64 (`0x006485F0`, `0x00648820`, `0x00648640`, `0x006488A0`),
the only one that changes a player's or monster's direction on the
server is the snap `0x006488A0(path, d)` (direction := new direction
:= d & 63; server callers `0x005A65E3`, `0x005C5E30`, `0x005F95DD`).
Direct writes outside these functions were not searched.
Server readers of +0x64 (`0x006487F0`): the target lead (`0x006791CF`,
`0x006792F8`, §3; its lead is always 0), `0x0057CDB2`, `0x00597FE7`,
`0x00597FFB`, `0x00620139` (owners: their callers' specs).

### 9. Per-tick movement

#### 9.1 Who calls it

Event 0 every tick (`sim/tick.md` §3 step 4; `sim/units.md` §4.4–§4.6):
players in modes 2, 3, 6, 19 → `0x00580C20`; monsters in moving modes →
the mode's event-0 function, e.g. walk `0x005A8490`: state 13 →
`0x005C9D90`, state 22 → `0x005CE4F0`, step (§9.3), result 2 → the
mode's end (`0x005A8030`). The two state calls never gate the step
(results untested); run, knockback and sequence event 0: `sim/units.md`
§4.6 rule 13.

#### 9.2 Player step (`0x00580C20`)

1. Target check (`0x00553490`): with a target unit, look it up by its
   stored type and GUID; if the unit found differs from the stored
   pointer, or is an item in mode 1 or 2 (equipped, belt), target :=
   none (`0x00648B90(0)`).
2. State 13 (`0x0D`) → `0x005C9D90` (skills spec); its result is
   ignored and the step goes on with step 3.
3. Mode 3 (run): run drain (rule 9.9); exhausted → restart the
   movement (§1.5 with mode 2: walk, or town walk in a town; path
   recomputed).
4. Step (§9.3), result s.
5. Position history (`sim/path-placement.md` §10 rule 7, the owner):
   when `GetTickCount` > last + 25 ms and the squared distance to the
   previous record is > 45. Simulated: monster AI reads it.
6. s = 2 (stopped), a player with player data and +0x150 ≠ 0 (a queued
   interaction: `0x00460780`, from `0x00641F20`, stores +0x150 := 1,
   +0x154 := −1, or −2 when its caller passes flag ≠ 0, +0x158 := unit
   type, +0x15C := GUID; `world/npc.md` §2 C→S 0x13): +0x154 < 0 →
   **neutral start** `0x0057F020`, then `0x00548B00`(type +0x158, GUID
   +0x15C, +0x154 = −2, game) (NPC / unit interaction); +0x154 ≥ 0 is a
   skill id: `0x006439F0` finds the player's skill → `0x00580A70`(skill,
   its mode `0x00643860`, type, GUID, 0) and result 1; not found →
   neutral start. Then +0x150 := 0 (`0x00580E88`). Asm
   `0x00580D94`–`0x00580E88`. +0x150 = 0: neutral start. Result s;
   `sim/units.md` §4.5 then runs the ENDANIM handler, which starts
   neutral again (edge case 6).

#### 9.3 Step (`0x00554CA0(game, unit)`)

Game null → fatal. Target check (rule 9.2 step 1, again). Movement
(`0x00650840(unit, 0x400)`, rule 9.4) → moving m. Room changed (path
flag 0x2) → room-change messages (rule 9.8). Result 1 if m, else 2.

#### 9.4 Movement (`0x00650840(unit, base)`)

1. No path → 0. Flags &= ~0x8. Missile → collided mask := 0.
2. Move only if all hold: flag 0x20; count > 0; velocity ≠ 0; missile,
   or the arrival check (rule 9.5) passes; index < count. Then:
   1. Velocity vector (`0x006502D0`): base ≤ 0 → 0x400. Acceleration
      (+0x88) ≠ 0: counter += 1; when it exceeds 4: velocity +=
      acceleration, clamped to [0, max velocity] (reaching the max
      clears the acceleration), counter := 0. m = (base · velocity) >> 6;
      velocity vector = (m · direction vector) >> 12 per axis
      (arithmetic). 
   2. Vector (0, 0) → rule 3.
   3. One step (rule 9.6) → new precise position Q.
   4. Position := Q (rule 9.6 "set position", room hint none).
   5. Path type ≠ 4 and index < count: velocity and direction toward the
      next point (§8.4).
   6. index < count → result 1.
3. Otherwise: reset (rule 9.7); result 0.

#### 9.5 Arrival check (`0x006503F0`)

1. Types 5, 6 with index ≥ count → passes.
2. No target unit: index ≥ count and position ≠ final target → re-path
   (rule 9.10, finish 0); else passes.
3. Target unit: unit distance (`0x00641530`) ≤ stop distance (+0x93) →
   fails (the unit stops). Target refresh point (`0x00679250`: target
   position, plus the lead of §3 step 4). Target a player or monster
   whose refreshed point is more than 5 from the previous target (+0x14)
   on either axis → re-path (finish 1). Otherwise: index < count, or
   position = final target → passes; else re-path (finish 1).
   Stop distance writers (1.14d, every server write of path +0x93):
   the allocation `0x00649D00` sets 0; `0x00649070(path, n)` sets n − 1
   for n in 1…19, else 0, and is called only by monster AI mode
   requests and skill functions (`monsters/ai.md` §7.1; e.g.
   `0x005DE490`, `0x005DEAD0`, `0x005CB940`, `0x005C07A0`,
   `0x005DA020`). No player walk / run (C→S) path sets it, so a
   player's walk or run to a unit keeps 0: it stops only at unit
   distance 0 or by the other tests of this rule.
4. Wherever a rule says "re-path", the check's result is the re-path's
   result (§9.10): non-zero passes, 0 fails. The movement then tests
   index < count against the new path.

Unit distance (`0x00641530`): Δ per axis; both < 8 and both sizes < 4:
d = `dist8_unit`[Δx + 8Δy]; a negative entry returns 0 at once, with no
size adjustment (`0x00641634`); else d minus 1 if either size is 3
(not below 0); result d + 1 if either size < 2, else d. Otherwise each
axis Δ − (size1/2 + size2/2) (not below 0), then 2·max + min.

#### 9.6 One step (`0x00650660`) and set position (`0x0064FB90`)

1. Collided mask := 0; "monster re-path" := the unit is a monster and
   flag 0x10 is set.
2. Velocity vector 0 → count := index := 0; Q := the current cell
   centre; done. (Dead in 1.14d: the only caller, §9.4 rule 2.2, resets
   before calling with a (0, 0) vector.)
3. Δ := velocity vector. Not a missile: R := point[index] centre −
   position; if |R.x| ≤ M and |R.y| ≤ M with M = max(|Δx|, |Δy|): Δ := R
   and "reaches the point".
4. If position + Δ is in another cell: distance budget (+0x90) −= 1 when
   > 0 and the type is not 8 or 11; cell walk (rule 9.6.5); blocked →
   Q := the centre of the last free cell, and: "monster re-path" → re-path
   (rule 9.10, finish = a target unit is set; its new points drive the
   next steps); else index := count (the movement ends); done. The
   caller ignores this function's own result.
5. Cell walk (`0x00650150`): saved-step count := 0. (sx, sy) := Δ halved
   (arithmetic shift, both together) until |sx| ≤ 0x10000 and |sy| ≤
   0x10000 (`0x00678F00`; x first, then y). c := position. Until c's cell
   equals the cell of position + Δ: n := c + s; if n is in another cell
   than c: footprint move from c's cell to n's cell (rule 9.6.6);
   refused → blocked (Q := centre of c's cell); else with flag 0x20000
   saved step[k] := n's cell; k += 1; k ≥ 10 → stop the walk (the
   remaining cells are not tested). c := n. End: flag 0x8 if k > 0; Q :=
   position + Δ.
6. Footprint move (`0x0064FF90`): path flag 0x4 → forced move
   (`sim/path-placement.md` §6 rule 2; players and monsters only, else
   fatal), accepted. Missile → missile move (§6 rule 3) with the move
   mask; collided mask |= result; refused iff the mask has 0x1 or 0x4.
   Others → try move (§6 rule 1) with the move mask (0x3401 replaced by
   0x3C01); collided mask |= result; refused iff the mask ≠ 0.
7. Reaching the point (rule 3) and not blocked: index += 1.
8. Set position (`0x0064FB90(Q, hint)`): missile path whose new cell has
   no room → count := 0, position unchanged. Else precise := Q, client
   coordinates (`sim/path-placement.md` §1); if flag 0x1: room recache
   (rule 9.6.9).
9. Room recache (`0x0064FAD0(hint)`): the room contains the cell →
   nothing. Else new := cell lookup from the room, else from the hint;
   none and missile → count := 0, return. Previous room := room; the
   unit leaves the old room's list (`0x0064C370`); flag 0x2; room :=
   new; when new ≠ null: room list insert (`0x0064C350`) and queue for
   update (`0x0064C040`). (A non-missile can end with no room.)

#### 9.7 Reset (`0x006507B0`)

Position := its cell centre (set position, rule 9.6.8); flag 0x20 := 0;
count := index := 0; velocity vector := (0, 0).

#### 9.8 Room-change messages (`0x00554670(game, unit, 0)`)

Only when path flag 0x2 is set. Old room := previous room (path +0x20),
new := room. A monster's AI room memo (monster data +0x50) := 0. Walk
both rooms' client arrays (sorted by client address, `drlg/rooms.md`
§7) as a merge: a client only in the old array, whose player is not the
unit, gets the unit's removal (`0x00571600`: S→C 0x0A, type and GUID);
only in the new array (not the unit) gets the unit's add messages
(`0x00571F90`, the unit-update spec); in both: nothing. (The previous
room is found by `0x005545C0`: path +0x20 when it is still a room of the
unit's act, else none.) Then flag 0x2 := 0 (`0x00648B40`). In single
player the only client is the player's own: a moving player sends
nothing here, a monster crossing into or out of the player's rooms gets
add/remove messages.

#### 9.9 Run drain (`0x0057F240`)

1. Room in a town → no drain, keep running.
2. d = 2 · charstats `RunDrain`; with a torso item (body location 3):
   d := d · (armor `speed` / 10 + 1) (truncated); s = stat 154 (item
   getter): d := d − trunc(s · d / 100); d < 1 → 1.
3. Stamina (stat 10, 8.8 fixed) −= d (base stat add `0x006272B0`).
   Still > 0 → keep running; else stamina := 0, exhausted.

#### 9.10 Re-path (`0x00650350(unit, finish)`)

No path → 0. Unless flag 0x10: a monster (type 1) whose re-path
budget (path +0x94, u8, read by `0x00649120`) is 0 → 0 (players skip
this test); else queue for update, unit flags (+0xC4) |= 1, and the
budget += −(current point index +0x24), clamped to 0..255
(`0x00649140`). The only setter is `0x006490E0` (value > 255 → fatal),
called with 20 by the monster movement start `0x005A7C20` (after the
target is set; that function belongs to `monsters/ai.md`), so a monster
re-paths until it has advanced 20 points in total since that start.
Which AI requests reach it, and when: `monsters/ai.md` §7.5.
Types 2, 13, 15: finish → type 13; else
type 2 and target := final target; compute (§3); non-zero → result;
else type 15 and compute again. Other types: compute (§3).
The types are written to path +0x3C directly (not through set type,
§2: flags and direction offset unchanged). Every compute here passes
town access 0.

### 10. Messages

1. Walk/run requests reply nothing; the path and mode changes queue the
   unit for update (mode set, `sim/units.md` §4.1).
2. Update pass (`sim/tick.md` §6 step 5, per unit `0x0053A500`): for a
   player whose mode changed (flag 0x1), the mode's update function
   (table `0x007319E8`, 20 rows of 3 dwords: function, code c1, code
   c2; read by `0x005484B0`, row 0 for a null unit; a null function
   does nothing) runs per client (ECX game, EDX unit; stack client,
   mode). Every row of 1.14d (dump of `0x007319E8`–`0x00731AD7`):

   | Modes | Function | c1, c2 | Own client | Message to each receiver |
   |---|---|---|---|---|
   | WL 2, TW 6 | `0x00548180` | 1, 0 | nothing | 0x10 or 0x0F, below |
   | RN 3 | `0x00548180` | 0x17, 0x18 | nothing | same |
   | NU 1, TN 5 | `0x00548400` | 7, 7 | nothing | 0x0D: a = c1, b = 0 |
   | GH 4 (c 6), BL 9 (c 0x12), DD 17 (c 9) | `0x00548350` | c, c | sent | 0x0D: a = c1, b = unit byte +0xB0 |
   | DT 0 | `0x0054DA10` | 8, 8 | sent, then the stat message | 0x0D as `0x00548350` (a = 8) |
   | KB 19 | `0x005482A0` | 0x14, 0x14 | sent | 0x0F: code c1, byte @11 = unit byte +0xB0 |
   | A1 7, A2 8, SC 10, TH 11, KK 12, S1–S4 13–16, SQ 18 | `0x00548090` | 0x15, 0x16 | only with E-flags bit 0x4 | skill message, below |

   "Nothing" = the row returns for the client whose player
   (`0x00537860(client, 0)`) the unit is. x, y = the unit's position
   (`0x006488C0` / `0x00648900`; players have a dynamic path,
   `sim/path-placement.md` §2); target x, y = the path target
   (`0x00648A00` / `0x00648A10`, path +0x10 / +0x12); life % =
   `0x00621F20(unit)`. Each builder takes DL = message id, then type 0
   and the unit GUID (+0x0C).
   - `0x00548180`: T := the path's target unit (`0x00553540`). T
     non-null and `0x005387F0(T, client)` ≠ 0 (client in T's room's
     client array, `drlg/rooms.md`) → S→C 0x10
     (`0x0053B520`: code c2, T type, T GUID, x, y). Else → S→C 0x0F
     (`0x0053B570`: code c1, target x, target y, 0, x, y).
   - `0x00548400`, `0x00548350` → S→C 0x0D (`0x0053B4B0`: a, x, y, b,
     life %).
   - `0x0054DA10`: `0x00548350`; then, only when the unit is the
     client's player, the stat message of stat 175 `goldlost` (value =
     unit total `0x00625480(unit, 175, 0)`, sender `0x00548520` with the
     argument order of `sim/stat-lists.md` §11 rule 4).
   - `0x005482A0` → S→C 0x0F (c1, target x, target y, unit byte +0xB0,
     x, y).
   - `0x00548090`: E := the used skill (`0x00620250`); none → nothing.
     E-flags (`0x006446A0`, E +0x0C) without bit 0x4 and the unit is
     the client's player → nothing. Bit 0x4 has one setter, the dodge /
     avoid reaction (`combat/damage.md` §7.1 rule 5.1, `0x0057D1CF`), so
     in 1.14d the own client gets this message only for a dodge / avoid
     (mode S1). Skill id := `0x00643CE0(E)`; b := the level with bonuses
     `0x006442A0(unit, E, 1)` (byte). T := the path's target unit
     (`0x00553540`); T non-null and `0x005387F0(T, client)` ≠ 0 →
     `0x0053D530` (T type, T GUID, skill, w 0, b, flag 0); else
     `0x0053D4D0` (target x, target y, skill, w 0, b, flag 0). Builders:
     `sim/intents-events.md` §3.5 rule 5.
3. Same pass, before it: a player with flags 2 bit 0x10000, or bit 0x800
   when the client's player is not this unit → S→C 0x15 (`0x00548010`:
   type, GUID, x, y, flag 1 for 0x10000 else 0). Waypoint arrival and
   game entry show it (`sim/path-placement.md` R1–R3).
4. Monsters: the monster update `0x00598220` (owner: the unit-update
   spec) sends 0x67/0x68 for moving monsters.
5. S→C 0x95 / 0x96 / 0x18 (life, mana, stamina, position) come from the
   client vitals sync `0x00548760` at each flush, not from movement
   (owner: `combat/vitals.md` §5).
6. Layouts of 0x0D, 0x0F, 0x10, 0x15, 0x96 are in `sim/server-messages.tsv`.

### 11. Missile paths (`0x00649760`)

§3 step 1 hands a path with flag 0x40000 to `0x00649760`, by path type
(+0x3C); any other type is a fatal assert. A non-zero result sets path
flag 0x20; type 4 with result 0 clears it. The result is the count.

#### 11.1 Type 4, straight missile (`0x006492F0`)

1. Index := 0. With a target unit: target (+0x10 / +0x12) := its
   sub-tile position (objects, items, tiles: static position; others:
   the path's cell).
2. |target x − cell x| ≥ 100, or the same for y, or target x = 0, or
   target y = 0 → result 0.
3. points[0] := target; count := 1; +0x38 := 0; velocity and direction
   toward point 0 (§8.4; a target equal to the position sets velocity 0).
4. Room-exit flag: the path room exists and the target lies outside its
   sub-tile rect (x in [room x, room x + w), y likewise, half-open) →
   flag 0x1. Result count (1). Read again (2026-10-08, `0x006493BD`–
   `0x006493EC`): no path room also sets 0x1 (the null test branches to
   the `or`), and a target inside the room leaves +0x34 unchanged: the
   flag is only ever set here, never cleared.

#### 11.2 Type 10, charged bolt (`0x0067A240`)

1. n := max distance (+0x91) >> 1. d0 := `testdir`[o(cell → target)]
   first entry (§5.1 rule 1, `0x00678D70`).
2. Offset table (32 entries): (−1, 0, +1) repeated 10 times, then −1,
   +1.
3. c := the current cell. n times: one RNG step on the **unit seed**
   (unit +0x20, `sim/rng.md` §2; form `lo' & 31`, §6), d := (table[lo'
   & 31] + d0) & 7; points[k] := c; c += 2 · step(d) (`dir8_toward`).
4. points[n] := c; count := n + 1 (points[0] is the start cell, dropped
   by §8.4 rule 1). n draws per compute.

#### 11.3 Type 14, blessed hammer (`0x0067A140`)

1. Start S = the precise position (16.16); previous := S; count := 0;
   k := 0.
2. Repeat until count = 77: k += 1; r = 9600 · k (0x2580 per step,
   converted to float32); a = 16 · k; x = S.x + trunc(cos(a) · r), y =
   S.y + trunc(sin(a) · r), with sin(a) = float32 table `0x00707800`
   [a & 0x1FF] (512 entries, sin(2πi/512); `0x0040B350`) and cos(a) =
   table[(a + 128) & 0x1FF] (`0x0040B330`); the product is x87 and the
   conversion truncates toward zero. If (x >> 16, y >> 16) differs from
   the previous point's cell: points[count] := that cell, count += 1,
   previous := (x, y).
3. Count := 77. No draw.

The product of two float32 values is exact in 53-bit precision, so d2rs
can compute it with integers (float32 = mantissa · 2^exponent); whether
the x87 precision control is 53-bit (compiler default) or 24-bit while
the game runs is open question 9.

### 12. Other path types (1.14d-read 2026-10-08)

Same calling form as §5 (path info I: start I+0x00, target I+0x04, max
distance I+0x18, path I+0x30; points at path +0x9C, index +0x24, count
+0x28). Results are the count.

#### 12.1 Circling walk (`0x00679B30`; types 5, 6, 12)

Called by Toward (§5.2) when the direction offset (+0x98) k ≠ 0, after
index and count := 0. It is §5.2 step 4 with these differences: n
starts at the current count (+0x28); each of the `testdir` triple
(`0x00678D70`) gets `(t + k) & 7` before the first-free pick
(`0x006793E0`); the corner append of step 4.3 has no "unless cur =
start" (the first step always appends the start); no clear-ray or slack
shortcut (steps 1–3 of §5.2 are not run). End as §5.2 step 5; count :=
n.

#### 12.2 Type 3 (`0x00679E50`)

Index := 0, then Toward (§5): the same as type 2.

#### 12.3 Type 12, back-up turn (`0x00679E60`)

Index := 0; k := −4; n := circling walk (§12.1, appending after the
current count); n = 0 → result 0 (k left at −4). Else count := n; k :=
−2, r := circling walk (appending after n); r = n (nothing added) → k :=
+2, r := circling walk. k := −4 again; result r.

#### 12.4 Type 9, leap (`0x00679F70`)

Index := 0; points[0] := target; next-position check (§5.1 rule 5) with
P := target. P = start → result 0. Else points[0] := P, count := 1,
result 1.

#### 12.5 Type 8, knockback server (`0x0067A000`)

k := (path +0x90 >> 1) + 1 (distance budget). With a target unit
(+0x58; position: objects, items, tiles their static position, others
their path cell, `0x006488C0` / `0x00648900`): dx := start x − its x,
dy := start y − its y; m := max(|dx|, |dy|); m ≠ 0 → dx := k·dx / m,
dy := k·dy / m (signed, truncating); P := start + (dx, dy) (16-bit
adds). No target unit: P := target. Index is not reset. points[0] := P;
ray test (§5.1 rule 4) from start to P (P := the last free cell);
target := P (`0x00648AD0`, which also clears the target unit). P =
start → result 0; else points[0] := P, count := 1, result 1.

#### 12.6 Type 11, knockback client (`0x00679FD0`)

Client only. Index := 0; points[0] := target; next-position check
(§5.1 rule 5) on points[0]; result 1 (count not written).

#### 12.7 Types 0 and 16, IDA* (`0x0067AD00` → `0x0067AAA0`)

Info fields used: start I+0x00, target I+0x04, start room I+0x08,
target room I+0x0C, slack byte I+0x14, score byte I+0x1C (path +0x92),
type I+0x20, pattern I+0x28, move mask I+0x2C, path I+0x30. Distances
are in cost units: orthogonal step 2, diagonal 3; heuristic h(p) = min
+ 2·max of (|target x − p.x|, |target y − p.y|) (`0x0067ABB2`,
`0x0067A8AD`). Directions use the `dir8_toward` numbering (§5.1 rule 2;
step table `0x006F1958`); pref(p) = the first entry of `testdir`[o(p →
target)] (`0x00678D10`, table `0x006F1518`).

1. **Wrapper** (`0x0067AD00`): I+0x14 := 2 × I+0x14 (u8, in place).
   Bounds B (x, y, w, h) := the start room's sub-tile box (`0x00619730`).
   With a target room ≠ the start room (box x2, y2, w2, h2), per axis:
   x2 < x → left := x2, width := x + w − x2; else left := x, width := x2
   + w2 − x (the target room's right edge); y likewise. An axis whose
   width (height) equals the start room's after this gets left −= 10,
   width += 20 (top −= 10, height += 20); with no target room, or the
   same one, both axes get it. Then the search.
2. **Setup** (`0x0067AAA0`): w·h > 50,000 → fatal assert. Grid: (w + 6)
   × (h + 6) dwords, zeroed once (not per iteration); cell (x, y) is at
   (y − B.y + 3)·(w + 6) + (x − B.x + 3); value 0 unvisited, 1 blocked,
   else the best g seen. Search box: x in [B.x, B.x + w], y in [B.y, B.y
   + h] (inclusive). Type 16: random mode, seed = the path owner's unit
   seed (unit +0x20).
3. **Node** (0x1C bytes; pool of at most 900, `0x384`): f (u16), h
   (u16), g (u16), tries (i16), position, order pointer (into a row of
   the order table `0x006F17D8`, 8 rows × 8 i32, one entry per try),
   dir, parent, child (one slot, reused for every try). Root: f = h =
   h(start), g = 0, tries = −3 (so the root tries 8 directions, every
   other node 5), position = start, order = row 0 entry 0, dir =
   pref(start), no parent or child; pool count := 1.
4. **Thresholds**: type 0: T := h(start); type 16: T := h(start) +
   h(start)/2 (signed, truncating). Tmax := max(T, I+0x1C). Repeat:
   reset the root (rule 3, pool count 1), run the depth-first pass with
   T; found → stop; not found and the pool count is 900 → fail; T += 5;
   continue while T < Tmax, else fail (`0x0067AC90`–`0x0067ACD4`).
5. **Depth-first pass** (`0x0067A740`, N := root, at most 10,000 turns):
   1. N's position = target → found N.
   2. Turn count + 1 > 10,000 → not found.
   3. N's position outside the search box → found N (`0x0067A78F`–
      `0x0067A7C0`: leaving the box counts as success).
   4. c := position + step(N.dir). Grid cell of c = 0 → pattern query at
      c (`0x0064D910`, start room I+0x08, I+0x28, I+0x2C); blocked → cell
      := 1, next try (rule 6).
   5. gc := N.g + (2 if c.x = x or c.y = y, else 3) (u16). Cell ≠ 0 and
      cell < gc (unsigned; a blocked cell is always below) → next try.
      Cell := gc. hc := h(c); fc := hc + gc (u16); fc > T (signed) →
      next try.
   6. Child: N has no child slot → pool count 900 → not found; else the
      next pool node, zeroed, becomes N's child with parent N. Child :=
      {f fc, h hc, g gc, tries 0, position c, order := row (pref(c) −
      N.dir) & 7 entry 0, dir := (N.dir + that entry) & 7}, in random
      mode + R[s]: one step of the seed (lo := lo × 0x6AC690C5 + hi,
      64-bit), s = new lo & 0x1F, R = `0x006F18D8` (−2, −1, 0, 1, 2
      six times, then −1, 1).
   7. Child h < I+0x14 (the doubled slack) → found the child. Else N :=
      the child.
6. **Next try** (`0x0067A690`): tries < 4 → next order entry, dir :=
   (dir + entry (+ R[s] in random mode)) & 7 (`0x0067A630`); tries += 1;
   tries ≠ 5 → continue with N. tries = 5 → backtrack: N = root → not
   found; else N := parent, next order entry, dir := (dir + entry (+
   R[s])) & 7, tries += 1; repeat while that also reaches 5. The parent
   advance has no "tries < 4" guard: a parent at tries 4 still advances
   its order pointer (and in random mode draws), then reaches 5 and
   backtracks again. A reused child (rule 5.6) keeps its own child slot
   and parent; only f, h, g, tries, order, dir and position are
   rewritten. Confirmed 2026-10-08 (impl-pc1-s5).
7. **Output** (`0x0067A9F0`): walk from the found node up to (not
   including) the root; a node is recorded when its step from its parent
   differs from the step of the last recorded node (the found node
   always is), at most 78. Points in root → found order go to path
   +0x9C. Fewer than 2 or more than 77 → 0; the search returns the count
   (≥ 78 → 0).

Order table rows (increments, entry 0 first; `0x006F17D8`, dumped):
0: 0 1 6 3 4 5 2 7; 1: 0 1 6 3 1 3 6 1; 2: 0 1 1 1 1 1 2 7; 3: 1 1 1 1
1 3 6 1; 4: 6 4 3 6 1 3 2 7; 5: 7 7 7 7 7 5 2 7; 6: 0 7 7 7 7 5 2 7;
7: 0 7 2 5 7 5 2 7.

Test vector (synthetic, type 0, slack 0, score 70, every cell free,
one room): start (10, 10), target (13, 11). h = 1 + 2·3 = 7 = T; root
dir pref = 0. (11, 10): g 2, h 5, f 7 → child, dir 0. (12, 10): g 4, h
3, f 7 → child (row 1, dir 0). (13, 10): g 6, h 2, f 8 > 7 → next try,
dir 0 + 1 = 1. (13, 11): g 7, h 0, f 7 → child at the target → found.
Output (12, 10), (13, 11); count 2. A straight run (target (13, 10))
records only the found node, so the search returns 0.

#### 12.8 Type 15, wall follow (`0x0067C2D0`)

Uses I+0x00 start S, I+0x04 target, I+0x08 start room (every query is
looked up from it), I+0x28 pattern, I+0x2C move mask, path I+0x30
(whose owner must be a player or monster, else fatal assert). Cell
buffer: 88 entries.

1. L := path +0x91 (max distance); with a target unit (+0x58) and L <
   40 → L := 40.
2. **Line** (`0x0067B9F0`): the cells from S (excluded) to the target
   (included); none when they are equal. Major axis x when |dx| ≥ |dy|,
   else y; length = |major delta|; length > L − 1 → no cells. Each step
   moves one on the major axis; err starts at 0; err += |minor delta|;
   err ≥ |major delta| → err −= |major delta| and one step on the minor
   axis. Line direction D: y-major → 0 for −y, 2 for +y; x-major → 1
   for +x, 3 for −x. N := the cell count; N ≤ 2 → result 0.
3. **Walk**: P := S, i := 0; while i < N: buf[i] blocked (pattern
   query ≠ 0) → repair (rule 4) with P, i, N, L − i, D; failed → result
   = compression (rule 6) of the first i cells. Else (free, or repaired)
   P := buf[i] (after a repair: the cell at the new i, untested), i +=
   1. Result = compression of the N cells; N = 0 → 0.
4. **Repair** (`0x0067BDF0`): followers A and B start at P, each with
   up to 201 points, a direction, a mark and a done flag. d0 := dir(P →
   buf[i]) (table `0x006F1E18` on 3·dy + dx); directions here: 0 (0,
   −1), 1 (1, −1), 2 (1, 0), 3 (1, 1), 4 (0, 1), 5 (−1, 1), 6 (−1, 0), 7
   (−1, −1) (step table `0x006F1D98`). A.dir := d0 + 1, B.dir := d0 + 7
   (indices up to 15; every table repeats after 8). Turn tables
   (dumped): A after a free step T1 = `0x006F1F00` (0, 1 → 6; 2, 3 → 0;
   4, 5 → 2; 6, 7 → 4), after a blocked try T2 = `0x006F1EC0` (0, 1 →
   2; 2, 3 → 4; 4, 5 → 6; 6, 7 → 0); B: T1 = `0x006F1E80` (as A's T2),
   T2 = `0x006F1E40` (0 → 6; 1, 2 → 0; 3, 4 → 2; 5, 6 → 4; 7 → 6).
   Turns alternate, A first; a follower whose mark is non-zero after its
   turn moves again instead of passing the turn. A turn of a not-done
   follower F (other follower O):
   1. Step (`0x0067BBF0`): up to four tries: candidate := F.pos +
      step(F.dir); free → stop; else F.dir := T2[F.dir]. Four blocked →
      F done, end of the turn.
   2. F has points and D ≤ 3: progress k from P along D (0: P.y −
      cand.y; 1: cand.x − P.x; 2: cand.y − P.y; 3: P.x − cand.x); k > 0,
      j := i + k − 1 < N and buf[j] = the candidate → rejoin: F.count <
      L − i (else fatal assert); append the candidate; buf[i..j] :=
      F's points (`0x0067BD80`), N += F.count − (j − i + 1), i := i +
      F.count; the repair succeeds.
   3. O has more than 1 point: F.mark ≠ 0 → the candidate =
      O.points[mark − 2] → the repair fails (buf cut at i, N := i); else
      F.mark := 0. Then the candidate = O.pos → F.mark := O.count.
   4. Append the candidate, F.pos := it, F.dir := T1[F.dir]; F.count ≥
      (L − i) − N − 1 → F done.
   The turns go on while neither follower is done.
5. **No rejoin**: L − i > 80 → fail (buf cut at i, N := i). Else with
   squared distances to the target dA, dB of A's and B's last points and
   dS of S: dB > dA → A if dS ≥ dA, else fail; dB ≤ dA → B if dS ≥ dB,
   else fail (this fail leaves buf and N unchanged). The chosen points
   are copied to buf at i; i := N := i + their count; success. A
   follower with no points has no last point: the dword read in its
   place is the follower's own done flag, so its "last point" is (1, 0)
   when that follower is done, else (0, 0) (not its position). Corrected
   2026-10-08 (impl-pc1-s5 read "its position"; 1.14d `0x0067C0E2`–
   `0x0067C11C`: point count − 1 indexed from the points array, the
   done flag being the dword just before it).
6. **Compression** (`0x0067C1E0`, n cells → path +0x9C): n = 1 → that
   cell; n = 0 → 0. Else p := buf[0] − S, run := 0; for k = 0 … n − 2
   with d := buf[k + 1] − buf[k]: d = p → run += 1; else run ≤ 0 and
   both components differ from p → run := 1, d.x := −2 (a diagonal jog:
   buf[k] dropped, the next change forced); else emit buf[k], run := 0;
   p := d. Then emit buf[n − 1]. Result = emitted count.

Test vector (synthetic, one-cell pattern, L 14, only (12, 10) blocked):
S (10, 10), target (15, 10): line (11, 10) … (15, 10), D 1, N 5.
(12, 10) blocked at i = 1, P (11, 10), d0 = 2. A (12, 11); B (12, 9);
A: (12, 10) blocked → dir 2 → (13, 11); B (13, 9); A (13, 10) = buf[2]
→ rejoin: buf = (11, 10), (12, 11), (13, 11), (13, 10), (14, 10), (15,
10), i = 4; (14, 10) taken untested, (15, 10) free. Compression: (11,
10), (12, 11), (13, 10), (15, 10); count 4.

### 13. Path accessors and the cell line test (1.14d-read 2026-10-08)

Gaps MV7, MV9–MV11 of `docs/handoff/impl-path-motion.md`.

#### 13.1 Small setters

1. **Step counts** `0x00648E70(path, n)`: null path → nothing. Only the
   low byte of n is used, unsigned: b ≥ 77 → 77; distance budget (+0x90)
   and max path distance (+0x91) := b. So a negative n gives its low
   byte: −1 → 255 → 77, −256 → 0, −200 → 56. (`0x00648E40(path, n)`
   writes +0x90 only, no cap.)
2. **Stop distance** `0x00649070(path, n)` (the AI's "path step count",
   `monsters/ai.md` §7.1–§7.2): §9.5 rule 3 (+0x93 := n − 1 for n in
   1…19, else 0).
3. **Stop the path** `0x00648730(path)`: flags (+0x34) &= ~0x20, point
   count (+0x28) := 0. Nothing else (target, type, budgets, re-path
   budget kept). Callers: NPC interaction `0x00548B95`
   (`monsters/ai.md` §1.2), the client's 0x28 branch
   (`client/msg-ui.md` §16), AI bodies.

#### 13.2 Path target unit `0x00553540(game, unit)` and target position `0x0056D2C0`

1. `0x00553540`: target check `0x00553490` (§9.2 step 1: a target unit
   whose stored type and GUID no longer resolve to the stored pointer,
   or an item in mode 1 or 2, is cleared to none by `0x00648B90(0)`),
   then the path's target unit (+0x58); the unit itself counts as none.
2. `0x0056D2C0(game, unit, &x, &y)`: T := rule 1. T present → (x, y) :=
   T's position (types 2, 4, 5: static path +0x0C / +0x10; else the
   dynamic path x / y `0x006488C0` / `0x00648900`, 0 without a path).
   T none → (x, y) := the path's target point (+0x10, +0x12;
   `0x00648A00` / `0x00648A10`). Result 1 when both are non-zero, else 0.
3. So a stale target (its GUID gone or reused) is cleared **and**
   replaced by the stored target point. `0x00648B90` never writes
   +0x10 / +0x12, so that point is whatever `0x00648AD0` (or the path
   code) last wrote: for an AI request aimed at a unit, the point of an
   earlier request, (0, 0) for a never-written path.

#### 13.3 Cell line test `0x0064E260(room R, &from, &to, mask)`

Cells are sub-tiles; "R's rect" = room +0x4C x, +0x50 y, +0x54 width,
+0x58 height; the cell value is R's collision grid word (`0x0061A010`,
`drlg/rooms.md` §10.3). Result 0 = clear (to unchanged); 1 = blocked,
and `to` := the blocking cell (the first cell that fails).

1. R null → 1, to := from. From not in R's rect → R := the cell lookup
   from R (`sim/path-placement.md` §4 rule 1); none, or from still
   outside → 1, to := from.
2. dx = to.x − from.x, dy = to.y − from.y, sx, sy their signs (+1 for
   0), adx = |dx|, ady = |dy|.
3. Cells visited, in order, each tested `value & mask` ≠ 0 → blocked
   there:
   - adx = ady = 0: the one cell from.
   - adx = 0 (or ady = 0): from, stepping one axis by sy (sx) up to and
     including `to`.
   - ady > adx (y major): cell k = (from.x + sx·⌊k·adx / ady⌋, from.y +
     sy·k), k = 0 … ady (error e starts 0, e += adx per step, x steps
     when e ≥ ady, then e −= ady).
   - adx ≥ ady (x major, ties included): cell k = (from.x + sx·k,
     from.y + sy·⌊k·ady / adx⌋), k = 0 … adx.
   All cells clear → 0.
4. Room crossing: when the next cell leaves R's rect on either axis
   (and the walk is not finished), R := the cell lookup for that cell
   from R; none, or the cell outside the found room's rect → 1, to :=
   that cell (it is not tested). Then the walk goes on in the new room
   with the same error term.

Callers named in other specs: the point-to-unit line `0x00645950`
(§13.4, the skill `lineofsight` test), the line test `0x00645910`
(`skills/bodies-2.md`, `monsters/ai.md` §7.4), the missile line test
(`skills/bodies.md` §2.11), event flag 0x200 (`skills/bodies.md`). No
draws.

Test vectors (synthetic; one room rect (0, 0, 20, 20), mask 1, cell
(3, 1) has bit 1, all others 0):

| from → to | Cells | Result, to |
|---|---|---|
| (0, 0) → (6, 2) | (0,0) (1,0) (2,0) (3,1) | 1, (3, 1) |
| (0, 0) → (2, 6) | (0,0) (0,1) (0,2) (1,3) (1,4) (1,5) (2,6) | 0 |
| (3, 1) → (3, 1) | (3,1) | 1, (3, 1) |

The walk is not symmetric: from → to and to → from round ⌊k·minor /
major⌋ from opposite ends, so they can visit different cells (vector
L4 below). No null-grid guard: every room the walk enters was found by
the cell lookup, and every active room has a grid (`drlg/rooms.md`
§10.2).

#### 13.4 Point-to-unit line `0x00645950` and the coordinate wrapper `0x00645910` (1.14d-read 2026-10-08)

1. **`0x00645950(x, y, unit U, mask)`** → 1 clear, 0 blocked (cdecl,
   4 args). It calls §13.3 as `0x0064E260(R, &from, &to, mask)` with
   R := U's room (`0x00620BB0`: types 2, 4, 5 the static path's room,
   else the dynamic path's room `0x00648A80`, none without a path),
   **from := (x, y), the point**, and **to := U's position** (types 2,
   4, 5 static path +0x0C / +0x10, else `0x006488C0` / `0x00648900`,
   (0, 0) without a path). So:
   - the walk runs from the point **toward the unit**; both end cells
     are tested (the point's cell first, U's own cell last);
   - the point's room is looked up from U's room (§13.3 rule 1: U's room
     itself, else U's room's adjacency array, `sim/path-placement.md`
     §4 rule 1). A point outside every room of that array is blocked
     at once, even when a chain of rooms would reach it; U without a
     room → blocked (rule 1);
   - cells in no loaded room met during the walk block (rule 4); no
     unit size, footprint or stop cell is applied (unlike the unit
     line `0x00622AA0`, `render/draw-order-2.md` §15.1); U's own
     footprint bits are in the last cell and count when the mask has
     them (of item 3's masks only 0x180, PLAYER | MONSTER, holds a bit a
     player or monster stamps for itself, so `lineofsight` 3 blocks on
     the caster's own cell when the caster is stamped there; 4, 0x1C09,
     0x804, 0x805 do not).
2. **`0x00645910(x1, y1, x2, y2, room, mask)`** → 1 clear: §13.3 with
   from := (x1, y1), to := (x2, y2) and the given room. Callers:
   `0x004C82A7`, `0x005D9BE7`, `0x005FD5E9`–`0x005FD798` (four),
   `0x00645EC2`; bodies in the specs named in §13.3.
3. **Callers of `0x00645950`** (all four in `Game.exe`):

   | Call site | Who | Point (x, y) | U | Mask |
   |---|---|---|---|---|
   | `0x0056F74B` in `0x0056F640` | server skill start, `skills/use.md` §5.3 step 6.4 | target position `0x0056D2C0` (§13.2; none → test skipped) | caster | `lineofsight` 1–5 → 4, 0x1C09, 0x180, 0x804, 0x805 (jump table `0x0056F7DC` = `0x0056F720`, `…727`, `…72E`, `…735`, `…73C` in that order) |
   | `0x004C61DB` in `0x004C6140` | client skill start (`client/msg-skills.md` §2 rule 7) | client target position `0x004C52E0` (below) | caster | same values (table `0x004C664C` = `0x004C61B0` … `0x004C61CC` in order); failure → the client start returns 0 |
   | `0x0057E1B1` in `0x0057E090` | area damage (`monsters/umod-callbacks.md` §3.2) | the area centre | victim | 0x805 |
   | `0x005C86F4` in `0x005C8520` | Armageddon state function (`skills/bodies-4.md` §4.9) | the random point | the unit | 0x805 |

   Value 0 skips the test; a value > 5 fails the start on both sides.
4. **Client target position `0x004C52E0(unit, &x, &y)`**: the path's
   target unit (`0x00648BF0`) when set → its position; else the path's
   target point (`0x00648A00` / `0x00648A10`). Either coordinate 0 →
   (x, y) := the unit's position plus an offset by facing octant
   (`0x004C51E0`: octant = facing (`0x00620100`) >> 3; dx = [0, −1, −2,
   −1, 0, 1, 2, 1], dy = [2, 1, 0, −1, −2, −1, 0, 1], as read). It
   always returns 1, so the client never skips the test, unlike the
   server (§13.2 rule 2 returns 0 for a zero coordinate).
5. **Which grid.** Each side tests its own DRLG's active-room grids:
   the server's, and on the client the client DRLG copy's, built by the
   same code at each client active-room creation (`drlg/rooms.md`
   §10.2). Tile bits are equal on both when the rooms are; unit bits
   (DOOR 0x800 in mask 0x804, MONSTER, PLAYER) are those each side's
   units stamped (`sim/path-placement.md` §5), and a cell in a room the
   client has not built blocks there and not on the server.

Test vector L4 (synthetic; one room rect (0, 0, 20, 20), U in it at
(0, 0), cell (5, 2) = 4, all others 0, mask 4 (`lineofsight` 1)):

| Call | Cells | Result |
|---|---|---|
| `0x00645950(6, 2, U, 4)` | (6,2) (5,2) | 0 (blocked at (5, 2)) |
| §13.3 from (0, 0) to (6, 2), mask 4 | (0,0) (1,0) (2,0) (3,1) (4,1) (5,1) (6,2) | clear |
| `0x00645950(30, 2, U, 4)`, no room holds (30, 2) | — | 0 (rule 1, no cell tested) |

## Constants & data dependencies

| Constant | Value | Where |
|---|---|---|
| request range | 50 sub-tiles per axis | `intents-events.md` §2.4 |
| path range | 100 sub-tiles per axis | `0x00649970` |
| player max distance / IDA* score | 73 / 70 | `0x00649D00` |
| monster max distance | 14 | `0x00649D00` |
| A* radius | dx² + dy² ≤ 324 | `0x00679ED0` |
| A* nodes / children / propagation stack | 200 / 8 / 200 | `0x0067B850` |
| path points | 78 slots, ≤ 77 from A* | path +0x9C |
| saved steps per tick | 10 | `0x00650150` |
| step base | 0x400 (100 %) | `0x00554CA0` |
| default velocity | 0x800 | `0x00649D00` |
| knockback velocity | 0x1000 | `0x00623F50` |
| velocity percent floor | 25 | `0x00623F50` |
| run drain factor | 2 × `RunDrain` | `0x0057F240` |

`sim/path-tables.tsv` (generated and checked by
`tools/trace-recorder/path_tables.py`; columns `table index a b c d e
va`, one row per table entry, `va` = address of the entry): `pathtype_flags`,
`pathtype_diroff` (a, 18 rows); `pattern_of_size` (a, 4); `dir8_toward`,
`dir8_target` (a = dx, b = dy, 8; identical); `testdir` (a, b, c; 25);
`altdir` (a, b, c, 255 = none; 25); `dist8_path`, `dist8_unit` (a, 64,
index ax + 8·ay; identical); `snap9` (a, 81, index (dx+4) + 9·(dy+4));
`tan` (a = x, b = y, c = angle; 128); `dirdiff` (a, 64); `field_dx`,
`field_dy` (a, 9); `velmod_player` (20 rows), `velmod_monster`,
`velmod_monster_x` (16 rows each; a = by passive skill, b = velocity
modifier, c–e unused here); `animstat` (a has-base, b base, c stat; 5).
582 rows. Code embeds the TSV (`include_str!`);
`py tools/trace-recorder/path_tables.py` checks every row against the
reference `Game.exe` (exit 0; `--selftest` perturbs each row).

Data columns: charstats `WalkVelocity` (1.14d: 6), `RunVelocity` (9),
`RunDrain` (20; Assassin 15); monstats `Velocity`; armor `speed`;
skills `srvdofunc`, `SeqInput`, `interrupt`.

## Randomness

1. Path finding, movement and placement do not draw.
2. Mode request: `roll(100)` on the player's unit seed in §1.4 rule 5,
   only when the used skill's row has `interrupt` and the player has
   state 42 (concentration). A walk request has no skill argument, so
   the draw depends on the player's *used* skill, which mode starts
   clear (§1.5 step 4) but skill code sets.
3. Target lead (§3 step 4) uses x87 floating point, but its lead byte
   is never set in 1.14d, so it adds 0 (§3, after the steps).

## Edge cases & original bugs

Reproduced by default.

1. A* does not re-sort an open node whose cost dropped (§7 rule 5).
2. A* propagation stack has no bound check (200 entries); a deep
   improvement chain could overflow it in 1.14d (not seen; treat > 200
   as a fatal error and log it).
3. Toward appends the first corner twice when the ray stopped at P ≠
   start and the first greedy step turns (vector W9); the duplicate is
   dropped when the unit reaches it (§8.4 rule 1).
4. Ray test: after a minor-axis step with err = 0 the new cell is not
   tested; when the next major step is blocked that untested cell is
   returned, so Toward's P (§5.2 step 2), stored as `points[0]`, can be
   a colliding cell; a unit walking it is refused at P (edge case 7).
5. Cell walk: after 10 crossed cells in one tick the rest of the step is
   not collision-tested. A straight step cannot cross 10: every vector
   component is a 32-bit product >> 12, so |Δ| < 2^19 (8 sub-tiles) per
   axis, at most 3 halvings, at most 9 sub-steps. The cut is reached only
   through the overshoot of edge case 12.
6. Arrival: the step that reaches the last point also ends movement
   (index = count) in the same tick; the player event then starts
   neutral twice (§9.2 step 6 and the ENDANIM handler) when no
   interaction is queued.
7. A blocked cell ends the path (index := count) and the unit stays at
   the last free cell's centre; there is no retry for players.
8. Request while moving: the path starts from the current cell; the
   fraction of the precise position is kept, so the first step can
   cross a cell earlier or later than from a centre.
9. Running with no stamina left is converted to walk only at request
   time (§1.5) or by the exhaustion restart (§9.2 step 3), which
   recomputes the path from the current position.
10. Path compute with a target at distance > 100 on one axis leaves the
    old points cleared and the unit neutral (§3 step 11).
11. Room recache can leave a non-missile unit without a room when no
    room contains its new cell (§9.6.9); 1.14d continues.
12. Cell-walk overshoot (§9.6 rule 5): s = Δ halved by arithmetic shifts,
    so 2^n · s misses Δ by up to 2^n − 1 units (short for positive,
    past for negative components), and the walk stops only on an exact
    cell match on both axes. When one axis still needs a sub-step while
    the other axis's extra sub-step leaves its final cell, the walk runs
    on past the target cell, moving the footprint each crossing, until
    the 10th crossing (Q := position + Δ, the footprint stays where the
    walk ended) or a refused move (Q := centre of the last free cell,
    beyond the target). Needs a component above 0x10000 (n ≥ 1), a
    component not divisible by 2^n, and a target position within 2^n − 1
    units of a cell edge (vector W10).

## Test vectors

Synthetic (rules above; model `model.py`/`model2.py` in the session
scratchpad; CI unit tests): one room [0, W) × [0, H), cells 0 except the
listed WALL (0x1) cells; player pattern 1, move mask 0x1C09, slack 1,
max distance 73.

| Id | Grid | Request | Expected points / target |
|---|---|---|---|
| W1 | 40×40 empty | (10,10) → (20,15) | [(20,15)] (ray clear) |
| W2 | 40×40, walls x = 15, y 5..15 | (10,10) → (20,10) | A*: [(13,13), (13,15), (15,17), (17,15), (17,13), (20,10)] |
| W3 | 60×60, walls x = 30, y 0..49 | (20,20) → (40,20) | [(28,20)] (d² = 400 > 324: no A*) |
| W4 | as W2 | (10,10) → (15,10) (target blocked) | target prepared to (13,10), path [(13,10)] |
| W5 | as W2 | (13,10) → (15,10) | preparation reaches the start: count 0, neutral |
| W6 | 40×40, walls y = 20, x 10..20 | (14,25) → (15,20) | target (15,22), path [(15,22)] |
| W9 | 80×80, walls x = 15, y 9..11 | (10,10) → (34,22) | [(14,12), (14,12), (15,13), (17,13), (18,14), (19,14), (20,15), (21,15), (22,16), (23,16), (24,17), (25,17), (26,18), (27,18), (28,19), (29,19), (30,20), (31,20), (32,21), (33,21)] |
| W10 | open | cell walk (§9.6 rule 5, no collision) from (0x6468D3, 0x6558D3) with Δ = (0x1972D, 0x1972D) | s = (0xCB96, 0xCB96); crossings (101,102), (102,103), …, (110,111): cut at 10; Q = (0x660000, 0x66F000) in cell (102, 102) |
| D1 | `tan` | direction (0,0) → (10,0) sub-tiles | vector (4096, 0), direction 56 |
| D2 | `tan` | (0,0) → (0,−10) | (0, −4096), 40 |
| D3 | `tan` | (0,0) → (5,5) | (2896, 2896), 0 |
| D4 | `tan` | (0,0) → (3,1) | (3888, 1286), 59 |
| M1 | open | walk (velocity 0x600 = 6·256, 100 %) from (100,100) to (105,100) | precise x per tick 0x64E000 + k·0x6000 (k = 1..13), tick 14 = 0x698000 (arrives, stops in tick 14) |
| M2 | open | run (0x900) from (100,100) to (103,101) | (x, y) per tick: (0x6508B0, 0x64AD36), (0x659160, 0x64DA6C), (0x661A10, 0x6507A2), (0x66A2C0, 0x6534D8), (0x672B1F, 0x656301), arrives (0x678000, 0x658000) at tick 6 |
| V1 | stats | walk: `WalkVelocity` 6, stat 67 = 100 (creation value, `combat/vitals.md` §1), stat 96 = 0 | p = 100; velocity 0x600 |
| V2 | stats | run: stat 67 = 150 (100 + the run list's 50, §8.2), stat 96 = 0 | p = 150; 0x600 · 150 / 100 = 0x900 |
| V3 | stats | run as V2, stat 96 = 20 | f = 150·20/170 = 17; p = 17 + 150 = 167; 0x600·167/100 = 2565 |
| V4 | stats | walk, stat 67 = 10, stat 96 = 0 | p = 10 → floor 25; 0x600·25/100 = 384 |
| S1 | stamina | Amazon, `RunDrain` 20, no armor, stat 154 = 0 | −40 per tick (stamina 8.8: −0.15625) |
| S2 | stamina | `RunDrain` 20, armor `speed` 10, stat 154 = 25 | d = 40·2 = 80; 80 − 20 = 60 |

Real (recordings; message side):

| Id | Recording | Observation |
|---|---|---|
| R4 | `20261006-015956-packets.jsonl` | 121 × 0x01, 215 × 0x03, 47 × 0x02, 2 × 0x04 accepted (result 0); 0 × S→C 0x0F / 0x10 (single player: §10 rule 2) |
| R5 | `20261006-022633-packets.jsonl` | 49 × 0x01, 72 × 0x03, 9 × 0x02, all result 0; no 0x15 resync (§1.1, `intents-events.md` §2.4 rule 3) |
| R6 | both | requests repeat every 7 frames while the button is held (e.g. 0x01 at frames 24, 31, 38, 45, 52), each recomputing the path (§1.5) |

## Provenance

- 1.14d functions read (decompile exports, `tools/ghidra/disasm.py`):
  handlers `0x005497E0`, `0x00549890`, `0x005498D0`, `0x00549920`;
  requests `0x005809D0`, `0x00580A70`, `0x0057EDD0` (jump tables
  `0x0057EE70`, `0x0057EE78`, `0x0057EE8C`, `0x0057EEA8`), `0x0057ED70`,
  `0x0057ED90`, `0x0057EEC0`, `0x0057F1F0`, `0x0057F190`, `0x0057F090`,
  `0x0057F020`, `0x00648DC0`, `0x00648CF0`; compute `0x00649970`,
  `0x006498A0`, `0x006483A0`, `0x00648120`, `0x00648050`, `0x00647FB0`;
  Toward `0x00679C80`, `0x00678C10`, `0x00678D70`, `0x00678DF0`,
  `0x006793E0`, `0x00679380`, `0x00679A60`, `0x00679720`; Straight
  `0x00679ED0`; A* `0x0067B850`, `0x0067B740`, `0x0067B440`,
  `0x0067B200`, `0x0067AED0`, `0x0067AF40`, `0x0067AFE0`, `0x0067B690`;
  velocity `0x00623F50`, `0x00624390`, `0x00620E80`, `0x00621360`,
  `0x00621740`, `0x006214A0`, `0x00648690`; vectors `0x0064FC60`,
  `0x0064FE40`, `0x006485F0`; step `0x00580C20`, `0x00553490`,
  `0x0057F240`, `0x00554CA0`, `0x00650840`, `0x006502D0`, `0x006503F0`,
  `0x00650660`, `0x00650090`, `0x00650150`, `0x00678F00`, `0x0064FF90`,
  `0x0064FB90`, `0x0064FAD0`, `0x006507B0`, `0x00650350`, `0x00641530`,
  `0x00554670`; monster walk `0x005A8490`; messages `0x0053A500`,
  `0x00580860`, `0x005484B0` (table `0x007319E8`), `0x00548180`,
  `0x00548010`, builders `0x0053B570`, `0x0053B520`, `0x0053BC10`,
  `0x0053B4B0`, `0x0053C3F0`, `0x0053C320`, `0x0053C230`; missile paths `0x00649760`,
  `0x006492F0`, `0x0067A240`, `0x0067A140`, `0x0040B330`, `0x0040B350`
  (table `0x00707800`); room change `0x005545C0`, `0x00571600`.
- §13 (2026-10-08): `disasm.py fn` on `0x00648E70`, `0x00648E40`,
  `0x00649070`, `0x00648730`, `0x00553540`, `0x00553490`, `0x00648B90`,
  `0x00648AD0`; decompile of `0x0056D2C0` and `0x0064E260` (branch
  structure checked against its error-term updates); test vectors hand
  computed from §13.3.
- §13.4 (2026-10-08, REC-248): decompile of `0x00645950`, `0x00645910`,
  `0x00620BB0`, `0x004C52E0`, `0x004C51E0`, `0x00620100`; `all.asm` call
  sites `0x0056F6EB`–`0x0056F74B` and `0x004C617D`–`0x004C61DB` (push
  order: x, y, unit, mask); jump tables `0x0056F7DC` and `0x004C664C`
  read from `Game.exe` (`re/scripts/rd.py`); every caller of
  `0x00645950` / `0x00645910` / `0x0064E260` listed from `all.asm`.
  Vector L4 hand computed from §13.3.
- D2MOO 1.10f as a map (`D2Common_10142`, `PATH_Toward_6FDAA9F0`,
  `PATH_RayTrace`, `PATH_Straight_Compute`, `PATH_AStar_*`,
  `PATH_PreparePathTargetForPathUpdate`,
  `PATH_FindValidTargetCoordsByMovingOrthogonally`, `D2Common_10226`,
  `sub_6FDACEC0`, `PATH_GetDirectionVector`, `sub_6FC7F600`,
  `sub_6FC81890`, `D2GAME_PLRMODES_First_6FC7F340`,
  `UNITS_UpdateRunWalkAnimRateAndVelocity`). Differences in 1.14d: the
  path compute requires a target room (1.13c change); the 0x96 status
  message lives in `0x00548760`; the cell walk is a separate function
  with the same rules.
- Live tables: charstats rows (patch_d2), `sim/path-tables.tsv` from the
  executable.
- §10 rule 2 table (2026-10-08): dump of `0x007319E8` (20 × 12 bytes)
  from `Game.exe`; reader `0x005484B0`; row functions `0x00548090`,
  `0x00548180`, `0x005482A0`, `0x00548350`, `0x00548400`, `0x0054DA10`
  read in `all.asm`; builders `0x0053B4B0`, `0x0053B520`, `0x0053B570`
  (DL = id); the only E-flags bit-0x4 writer in the server range is
  `0x0057D1CF` (every push / or before `0x00644660` scanned).

## Open questions

1. No per-tick position recording. Settle: a recorder hooking
   `0x00650840` entry/exit (unit GUID, frame, precise x/y, index, count,
   points, velocity, flags) while walking, running and clicking walls in
   town and a wilderness area; check M1/M2-type steps, §5–§7 paths and
   §9.6 blocking on live data.
2. Synthetic vectors W*, M*, D*, V*, S* come from the rules, not from the
   game; confirm a sample with recording 1 (same start, target and
   collision).
3. Path types of monster AI and skills (0, 3, 5, 6, 8, 9, 11, 12, 13,
   15, 16) and `0x00679B30` (direction offset): specify with the AI and
   skill specs (Ghidra on `0x0067AD00`, `0x0067A000`, `0x0067C2D0`).
   The missile types 4, 10, 14 are answered in §11. Answered (2026-10-08)
   in §12 for every type and `0x00679B30` (IDA* §12.7 and wall follow
   §12.8 completed 2026-10-08).
4. *Answered:* target lead (`0x00679190`, `0x00679250`): table sine,
   truncation, and +0x68 has no reachable writer, so the lead is 0
   (§3, after the steps).
5. *Answered:* the facing turn `0x00648640` runs only in the client
   (§8.5); the server direction changes by snap only. What each server
   reader of +0x64 does with it is its owner's (§8.5 list).
6. Answered: the status messages 0x95 / 0x96 / 0x18 (`0x00548760`) are
   owned by `combat/vitals.md` §5 (§10 rule 5).
7. ~~Monster movement messages 0x67 / 0x68 and the unit-update pass~~:
   answered in `sim/intents-events.md` §7 (`0x00598220` §7.3, mode
   messages §7.4, add messages `0x00571F90` §7.2).
8. *Answered:* `0x00649120` / `0x00649140` read and adjust the monster
   re-path budget at path +0x94 (not the distance budget +0x90); set to
   20 by `0x005A7C20` (§9.10).
9. PROVISIONAL: §11.3 runs in 53-bit x87 precision (because `Game.exe` sets 53-bit once at start-up and never changes it); settled by REC-21. x87 precision control during §11.3 (53-bit or 24-bit: a 24-bit mode
   rounds cos · r to float32 before the truncation). Settle: a recording
   of a Blessed Hammer missile's per-tick positions, or a debugger read
   of the FPU control word in `0x0067A140`.
   *Partly answered* (static): `Game.exe` sets 53-bit precision once,
   in the C runtime start-up (`__setdefaultprecision` `0x0068E70A`:
   `_controlfp_s(·, 0x10000, 0x30000)`, called from `0x00682FC4` and
   `0x006ADD40`), and never changes it afterwards: `__set_controlfp` has
   no caller, and every game `fldcw` (40 sites, e.g. `0x0067A1A9`) loads
   the saved word OR 0xC00 (rounding = truncate) and then restores it,
   leaving precision alone. So §11.3 runs in 53-bit unless code outside
   `Game.exe` (a display driver DLL) changes the control word; the
   debugger read at `0x0067A140` still confirms it for the GDI mode.
10. *Answered (2026-10-08)* (`mutants-path` §3, spec reading): §5.1
    rule 1's clamp(dy) is [−2, 2] (dy < −1 → −2, dy > 1 → 2;
    `0x00678C79`–`0x00678C94`); the code's reading is right (§5.1 rule
    1, text after the rule, with vectors).

Answered handoff questions (`docs/HANDOFF.md` §7):

- PQ1: stat 67 is read as the unit total (`0x00625480` in `0x00623F50`);
  a player's base is 100, so V1–V3 now state stat 67 = 100 / 150 / 150
  and V3's result is 2565; V4 adds the floor of 25 (§8.1).
- PQ2: a mode without the velocity modifier leaves the velocity
  unchanged (§8.1 rule 4). PQ3: §8.1 rule 3 as read (`0x00648690`).
- PQ4: the arrival result is the re-path's (§9.5 rule 4). PQ5: the
  step continues after `0x005C9D90` (§9.2 step 2; exhaustion restarts
  with mode 2). PQ6: a failed concentration roll falls through to the
  state 15 test (§1.4 rule 5). PQ7: town access 0, only monsters check
  the budget, types written directly (§9.10). PQ8: no room hint (§9.4
  rule 2.4, `0x0064FB90(Q, 0)`).
- PQ9: edge case 5 cannot happen through §9.4 (32-bit product bounds Δ
  below 8 sub-tiles); the cut is reachable only by the overshoot, edge
  case 12, vector W10.
- GR1: §9.6 rule 2 is dead in 1.14d (`0x00650660` has one caller, which
  tests the vector first). PX1: edge case 4 now says P can collide.
- `world/npc.md` OQ7 (§9.2 rule 6 vs the queued 0x13 interaction):
  §9.2 rule 6 corrected (`0x00460780` writes +0x150 := 1; the arrival
  branch runs `0x00548B00` or `0x00580A70`). §9.5 unit distance: a
  negative `dist8_unit` entry returns 0 directly.
- OQ3 (missile part), OQ6: §11, `combat/vitals.md` §5. The §9.8
  add / removal addresses were swapped (fixed: removal `0x00571600`
  sends 0x0A).
