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
| Summary | 49–64 |
| Inputs | 65–74 |
| Outputs / state changes | 75–81 |
| Rules | 82–83 |
|   1. Walk and run requests | 84–176 |
|   2. Path types | 177–215 |
|   3. Path compute (`0x00649970(path, unit, town access)`) | 216–261 |
|   4. Target preparation (flag 0x1000, `0x00648120`) | 262–278 |
|   5. Toward (type 2, `0x00679C80`) | 279–338 |
|   6. Straight (type 7, `0x00679ED0`) | 339–348 |
|   7. A* (type 1, `0x0067B850`) | 349–386 |
|   8. Velocity, direction vector, facing | 387–453 |
|   9. Per-tick movement | 454–606 |
|   10. Messages | 607–629 |
| Constants & data dependencies | 630–666 |
| Randomness | 667–677 |
| Edge cases & original bugs | 678–708 |
| Test vectors | 709–745 |
| Provenance | 746–781 |
| Open questions | 782–817 |
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
   unit seed** (unit +0x20, `0x0045C390`); r < v → go to rule 6. Else if
   state 15 (`0x0F`) → rule 6. Else allow.
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

### 2. Path types

Type table, index = path type (`sim/path-tables.tsv` tables
`pathtype_flags`, `pathtype_diroff`; functions `0x006EB6D8`):

| Type | D2MOO name | Function | Flags | Used by |
|---|---|---|---|---|
| 0 | IDA* | `0x0067AD00` | 0x21900 | monster AI (owner: AI spec) |
| 1 | A* | `0x0067B850` | 0x1900 | §7 |
| 2 | toward | `0x00679C80` | 0 | monsters' default, §5 |
| 3 | toward (restart) | `0x00679E50` | 0 | |
| 4 | missile | none (missile code) | 0x60000 | `missiles/missiles.md` |
| 5, 6 | circle CW / CCW | `0x00679C80` | 0 (offset +2 / −2) | monster AI |
| 7 | straight | `0x00679ED0` | 0x21900 | players, §6 |
| 8 | knockback (server) | `0x0067A000` | 0x1E600 | player mode 19 |
| 9 | leap | `0x00679F70` | 0x1E800 | skills |
| 10, 14 | charged bolt, blessed hammer | none | 0x60000 | missiles |
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
blocked target (§4), 0x20000 store saved steps (§9.4). Types without a
function (4, 10, 14, 17) are fatal if computed by §3. The types used only
by monster AI and skills (0, 3, 5, 6, 8, 9, 11, 12, 13, 15, 16) are named
here and specified with their callers (open question 3).

### 3. Path compute (`0x00649970(path, unit, town access)`)

1. Path null → result 0. Owner ≠ unit → fatal. Missile flag (0x40000) →
   missile path (`0x00649760`, missiles spec).
2. Collided mask (+0x54) := 0. With a target unit whose position is
   (0, any) or (any, 0) → result 0 (nothing reset).
3. Start = the current sub-tile (x, y); (0, 0) → step 11.
4. Target refresh (`0x006498A0`, result r, "target slack", default 1):
   with a target unit, target (+0x10/+0x12) := its position; (0, 0) →
   step 11 with r = 0. By the target's type: player or monster: if the
   path lead byte (+0x68) ≠ 0, the target point is moved ahead
   (`0x00679190`, uses floating point, open question 4), r = 1; object:
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
`0x00679B30` (monster circling, open question 3).

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
   (and 8–11 for classes < 410). Then p = f + stat 67 (unit total `0x00625480`; a player's
   base is 100 from creation, `combat/vitals.md` §1), at least 25, where f = stat 96
   (item/skill getter `0x00625500`) scaled by `animstat` row 4: f ≠ 0 →
   150·f / (150 + f) (truncated). Velocity := base · p / 100
   (truncated), base = charstats `WalkVelocity` × 256 for players,
   monstats `Velocity` × 256 for monsters (`0x00621360`; the mode does
   not change the base).
3. Setting a different velocity sets +0x38 := 15; max velocity := the
   new value (`0x00648690`).

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
(+0x66) := `dirdiff`[(d − direction) & 63]. Turning the current
direction toward the new one over time is not part of this spec (open
question 5).

### 9. Per-tick movement

#### 9.1 Who calls it

Event 0 every tick (`sim/tick.md` §3 step 4; `sim/units.md` §4.4–§4.6):
players in modes 2, 3, 6, 19 → `0x00580C20`; monsters in moving modes →
the mode's event-0 function, e.g. walk `0x005A8490`: state 13 →
`0x005C9D90`, state 22 → `0x005CE4F0`, step (§9.3), result 2 → the
mode's end (`0x005A8030`).

#### 9.2 Player step (`0x00580C20`)

1. Target check (`0x00553490`): with a target unit, look it up by its
   stored type and GUID; if the unit found differs from the stored
   pointer, or is an item in mode 1 or 2 (equipped, belt), target :=
   none (`0x00648B90(0)`).
2. State 13 (`0x0D`) → `0x005C9D90` (skills spec).
3. Mode 3 (run): run drain (rule 9.9); exhausted → restart the
   movement (§1.5 with mode 3: becomes walk, path recomputed).
4. Step (§9.3), result s.
5. Host-only position history (`sim/path-placement.md` §10 rule 7):
   when `GetTickCount` > last + 25 ms and the position is more than
   √45 sub-tiles from the previous record. Not simulated (open
   question 4 there).
6. s = 2 (stopped): player data +0x150 ≠ 0 would start a queued action
   (`0x00548B00` NPC talk, or a skill on a unit through `0x00580A70`);
   no 1.14d server code stores a non-zero value there (every server
   write stores 0: `0x00580A70`, `0x00580C20`), so in 1.14d s = 2 always
   means **neutral start** `0x0057F020`. Result s; `sim/units.md` §4.5
   then runs the ENDANIM handler, which starts neutral again (edge case
   6).

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
   4. Position := Q (rule 9.6 "set position").
   5. Not a missile and index < count: velocity and direction toward the
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

Unit distance (`0x00641530`): Δ per axis; both < 8 and both sizes < 4:
d = `dist8_unit`[Δx + 8Δy] (negative → 0); minus 1 if either size is 3
(not below 0); result d + 1 if either size < 2, else d. Otherwise each
axis Δ − (size1/2 + size2/2) (not below 0), then 2·max + min.

#### 9.6 One step (`0x00650660`) and set position (`0x0064FB90`)

1. Collided mask := 0; "monster re-path" := the unit is a monster and
   flag 0x10 is set.
2. Velocity vector 0 → count := index := 0; Q := the current cell
   centre; done.
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

No path → 0. Unless flag 0x10: player or monster for which `0x00649120`
(monster: distance budget) is 0 → 0; else queue for update, unit flags
|= 1, distance budget −= index. Types 2, 13, 15: finish → type 13; else
type 2 and target := final target; compute (§3); non-zero → result;
else type 15 and compute again. Other types: compute (§3).

### 10. Messages

1. Walk/run requests reply nothing; the path and mode changes queue the
   unit for update (mode set, `sim/units.md` §4.1).
2. Update pass (`sim/tick.md` §6 step 5, per unit `0x0053A500`): for a
   player whose mode changed (flag 0x1), the mode's update function
   (table `0x007319E8`, 3 dwords per mode: function, code to point, code
   to unit) runs per client: modes 2, 3, 6 → `0x00548180`: nothing for
   the client whose player it is; other clients get S→C 0x10 (target
   unit set: code, target type, target GUID, x, y) or 0x0F (code, target
   x, y, 0, x, y). Codes: walk 1 / unit 0, run 0x17 / 0x18 (walk and
   town walk share the row).
3. Same pass, before it: a player with flags 2 bit 0x10000, or bit 0x800
   when the client's player is not this unit → S→C 0x15 (`0x00548010`:
   type, GUID, x, y, flag 1 for 0x10000 else 0). Waypoint arrival and
   game entry show it (`sim/path-placement.md` R1–R3).
4. Monsters: the monster update `0x00598220` (owner: the unit-update
   spec) sends 0x67/0x68 for moving monsters.
5. S→C 0x95 / 0x96 / 0x18 (life, mana, stamina, position) come from the
   player status routine `0x00548760` in the client pass, not from
   movement (owner: open question 6).
6. Layouts of 0x0D, 0x0F, 0x10, 0x15, 0x96 are in `sim/server-messages.tsv`.

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
3. Target lead (§3 step 4) uses x87 floating point (no draw; open
   question 4).

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
   tested.
5. Cell walk: after 10 crossed cells in one tick the rest of the step is
   not collision-tested (needs a velocity over 10 sub-tiles per tick).
6. Arrival: the step that reaches the last point also ends movement
   (index = count) in the same tick; the player event then starts
   neutral twice (§9.2 step 6 and the ENDANIM handler).
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
  `0x0053B4B0`, `0x0053C3F0`, `0x0053C320`, `0x0053C230`.
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
4. Target lead (`0x00679190`, `0x00679250` with path +0x68 ≠ 0): x87
   sine/cosine (`0x0040B330`, `0x0040B350`) on (8·direction − 256·lead)
   & 0x1FF; who sets +0x68 (monster AI?) and the exact rounding. Settle:
   Ghidra on `0x00679190`, `0x0040B330`; d2rs needs a bit-exact
   replacement (hard rule 6).
5. Server-side facing turn (D2MOO `D2Common_10193`, turning +0x64 toward
   +0x65 by +0x66): which server code calls it and whether facing feeds
   any message or outcome. Settle: xref the 1.14d equivalent.
6. Owner of the player status messages 0x95 / 0x96 / 0x18
   (`0x00548760`: life/mana/stamina change thresholds, potion-heal
   prediction states 100/106, position resend after 4 quiet calls when
   moved ≥ 2): vitals spec or a client-update spec.
7. Monster movement messages 0x67 / 0x68 and the unit-update pass
   (`0x00598220`, `0x00571600`, `0x00571F90`): the unit-update spec.
8. `0x00649120` (monster re-path budget) and `0x00649140`: read only in
   §9.10; confirm with the AI spec.

Answered handoff questions (`docs/HANDOFF.md` §7):

- PQ1: stat 67 is read as the unit total (`0x00625480` in `0x00623F50`);
  a player's base is 100, so V1–V3 now state stat 67 = 100 / 150 / 150
  and V3's result is 2565; V4 adds the floor of 25 (§8.1).
