# Spec: World — Objects part 2 (operate functions part 2, small inits, object events)

- **Status:** draft: every rule below was read from the 1.14d `Game.exe`
  disassembly (addresses inline) and the live 1.14d `objects.txt`; D2MOO
  1.10f `ObjMode.cpp` gave names only. No recording of these objects
  exists. Not implemented yet.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::world::objects`
- **Related specs:** `world/objects.md` (part 1: §1–§15, whose notation,
  operate record, ENDANIM, `D(Q)`, trap arm and seeds are used here
  unchanged; its Constants, Randomness, Edge cases, Test vectors and
  Provenance lists also cover this part), `sim/units.md` §6.4 (event
  types), `sim/path-placement.md` §7, §10, §12.2, `items/inventory.md`
  §5.5, `world/quests-act3.md` §7.7, `missiles/missiles.md` §R9.5.
  Machine table: `world/object-functions.tsv`.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 40–49 |
| Inputs | 50–53 |
| Outputs / state changes | 54–58 |
| Rules | 59–60 |
|   16. Operate functions, part 2 | 61–188 |
|   17. Small init functions | 189–215 |
|   18. Object events 0, 3, 8, 9, 10 | 216–283 |
|   19. Obelisk completion (C→S 0x44, `0x00585240`) | 284–331 |
|   20. Item drop helpers (open question 13) | 332–427 |
|   21. Curable-state removal (`0x00578C20`, open question 15) | 428–440 |
|   22. Object allocation modes (open question 8) | 441–465 |
|   23. Client side of S→C 0x0E and 0x4D (open question 4) | 466–483 |
| Constants & data dependencies | 484–487 |
| Randomness | 488–502 |
| Edge cases & original bugs | 503–527 |
| Test vectors | 528–541 |
| Provenance | 542–562 |
| Open questions | 563–566 |
<!-- /index -->

## Summary

Part 2 of `world/objects.md`, split at § boundaries to keep each file
under 60 KB; section numbers continue from part 1. It owns the generic
operate functions beyond §8–§13, the small init functions, and the
object event handlers 0, 1, 3, 8, 9 and 10; §19–§23 answer open
questions of part 1 (obelisk completion, the item drop helpers, the
well's curable-state removal, object allocation modes, the client side
of 0x0E / 0x4D).

## Inputs

As `world/objects.md` Inputs.

## Outputs / state changes

As `world/objects.md`: object modes, flags and data, timers, drops,
player warps and placement, client messages.

## Rules

### 16. Operate functions, part 2

Operate record as §7.2 (game, object O, operator P, control, class id);
"none" = no object unit; ENDANIM as §8.2; f = game frame. Return values
are listed for fidelity only (§7.2). Live rows are `objects.txt` `Id`s.

#### 16.1 Torch tiki, operate 13 (`0x005843A0`)

Mode 0 (or none) → mode 1; mode 1 → mode 0; other modes nothing.
Returns 0. Live: 37.

#### 16.2 Trap door, operate 16 (`0x00581EB0`)

1. Mode 0 (or none) → mode 2. Return 1.
2. Mode 2: the first unit of type 5 (warp tile) in O's room unit list
   (head +0x74, next +0xE8); none → fatal. Warp P through it
   (`0x005550B0`, `sim/path-placement.md` §12.2). Return 1.
3. Other modes: return 1. (O's room, position and size are read first
   and not used.) Live: 74.

#### 16.3 Obelisk, operate 17 (`0x00582610`)

I := the unit P interacts with (`0x00554D00`: P +0x6C ≠ 0 → the unit
with GUID P +0x64, else none).

1. Mode 0 (or none): P busy (`0x00535060` = 1) → nothing. P has no item
   of type 20 (gem) in its item list (`0x005825B0`: inventory walk with
   the item-type test `0x00629BB0`) → sound 19 on P with target P
   (`0x00553380`: sound id at unit +0x6E, target at +0x70, queue, flag
   0x400). Else: P's interact := (type 2, O's GUID) (`0x00554120`: only
   when P +0x6C = 0; then +0x64 GUID, +0x68 type, +0x6C := 1), mode 3,
   S→C 0x58 (7 bytes, builder `0x0053D8D0`) to P's client: 0x58, O's
   GUID @1 (u32), 0 @5 (u8), byte @6 never written (stack contents).
2. Mode 3 and I = O: clear P's interact (`0x00554190`: +0x64 := −1,
   +0x68 := 6, +0x6C := 0), mode 1.
3. Return 1 in every case. Live: 80, 90 (90 has `InitFn` 1, so it also
   holds a shrine record). What follows mode 3 (C→S 0x44, the gem
   insert): §19.

#### 16.4 Secret door, operate 18 (`0x00583FF0`)

Mode 0 (or none): mode 1, clear flag 0x2, ENDANIM, free the footprint
(`0x00623830`). Return 1. Live: 129.

#### 16.5 Armor stand, operate 19 (`0x00584160`); weapon rack, operate 20 (`0x005841D0`)

Mode 0 (or none): drop at O's position in O's room, armor
`0x005594C0(game, room, &pos, −1, 0, 0)` or weapon `0x00559630(…)`
(§20.1, §20.2); then mode 2, clear flag 0x2. Return 1. The drop's draws
come before the mode change's §4 draw. Live: stands 104, 105, 550, 551;
racks 106, 107, 548, 549.

#### 16.6 Bookshelf, operate 26 (`0x00584060`)

Mode 0 (or none):

1. Mode 2, clear flag 0x2.
2. r := C step, lo' mod 20 (C = control seed). r < 13: C step, lo' & 1 =
   0 → code `tsc `, 1 → `isc `. r ≥ 13: C step, lo' & 1 = 0 → `tbk `, 1 →
   `ibk `. Drop item code (+0xB8) := it.
3. Drop it at O (`0x00559A30`, quality argument 2; §20.4).

Return 1. Live: 179, 180 (`InitFn` 2: their trap byte is never armed
here).

#### 16.7 Teleport pad, operate 27 (`0x00581BF0`)

1. O's room R0 none → return 0.
2. Partner T: in R0's unit list the first object (type 2) ≠ O with O's
   class; else, in R0's adjacency array order (`drlg/rooms.md` §6, R0
   itself skipped), the first such object of each room; RT := the room
   it was found in. None → return 0.
3. Point := T's position; free point from RT (`0x0064E7B0`, size 3, mask
   0x1C09, no fallback; `sim/path-placement.md` §7); none → return 0.
4. Place P (`0x00554EA0(game, P, RT, x, y, exact 0, alt 0)`,
   `sim/path-placement.md` §10); failure → return 0.
5. S→C 0x07 (`0x0053BC50`) to P's client: RT's tile x and y (room rect
   +0x10, +0x14) and RT's level; queue P for update (`0x0064C040`); P
   flags 2 (+0xC8) |= 0x10000. Return 0.

O's mode is not tested. Live: 192, 304, 305, 306.

#### 16.8 Slime door, operate 29 (`0x005821A0`)

Mode ≠ 0 → return 1. Else mode 1, free the footprint, flag 0x2 :=
`Selectable[new mode]`, and ENDANIM only when `Mode2` (+0x141) ≠ 0.
Return 1. Live: 229, 230.

#### 16.9 Exploding chest, operate 30 (`0x00581CD0`)

Mode 0 (or none): trap damage `0x005DFA00(game, O, P, 0)`, then `(game,
O, P, 1)` (combat spec); mode 1; ENDANIM. Returns 0. Live: 250
(exploding cow), 454.

#### 16.10 Bank, operate 32 (`0x00564CD0`)

Only for class 267. P's room and O's room both in a town level
(`0x0061AB00`): P's interact := (2, O's GUID) (`0x00554120`); S→C 0x77
(`0x0053CAB0`) with action 0x10 to P's client; scroll/tome recount
(`0x0055FA40`, `items/inventory.md` §5.5). Returns 0. §7.2 rule 2 lets
P operate it with an item on the cursor.

#### 16.11 Stairs, operate 47 (`0x00581F60`) and 50 (`0x00582180`)

- **47:** mode 0 (or none) → mode 1, ENDANIM. Mode 2 → the first warp
  tile (type 5) in O's room, else in the adjacency rooms (O's room
  skipped) in order; found → warp P (`0x005550B0`); none → nothing.
  Return 1. Live: 194, 195.
- **50:** O present and its mode's low 16 bits = 2 → run 47 (the warp);
  else return 0, so mode 0 never opens. Live: 386 (its mode comes from
  init 53, `world/quests-act3.md` §7.7).

#### 16.12 Jungle stash, operate 51 (`0x00583F10`)

Mode 0 (or none): mode 1, clear flag 0x2, ENDANIM, event 10 at f +
`Parm1` + 1, free the footprint when `HasCollision1` (+0x121) = 0, trap
arm with `InteractType` & 0x7F (§8.3), timer-argument owner GUID := P's
GUID (none: −1; `0x00552AF0`). Return 1. The drop is event 10 (§18.5).
Live: 185–188 (`InitFn` 2, so trapped by §5.2; `Parm1` 11, 34, 13, 33).

#### 16.13 Harrogath gate, operate 61 (`0x00582080`)

t := `GetTickCount`; t < +0xD4 + 500 → return 1 (host clock, as §10).
Mode 0 → free the footprint, mode 1, +0xD4 := t, ENDANIM. Mode 2 → stamp
the footprint at O's room and position (`0x00620A70`), mode 0, +0xD4 :=
t. Other modes nothing. Return 1. Unlike doors there is no occupancy
test before closing. Live: 449, 508.

### 17. Small init functions

Init record as §3 rule 6 (game, O, room, control, record, x, y). Mode
sets here run before the unit is added to the world (§3).

| Init | Address | Live rows | Rule |
|---|---|---|---|
| 8 torch | `0x005500C0` | 29, 37, 38, 102, 117 | mode 2 |
| 10 | `0x0054F860` | none | room level (`0x0061A1B0`) = 1 → event 8 at f + 60; else mode 2 and event 0 at f + 25 |
| 13 invisible object | `0x00594020` | 61 | quest chain 4 record exists (`0x00543640(game, 4)`) → link O to it (`0x005436B0(…, O, 4)`, `world/quests.md` §4.6); else mode 2 unless O is already in mode 2 |
| 14 brazier | `0x005500D0` | 101 | mode 1 |
| 22 fire | `0x0054FB40` | 160–162, 245, 345–347 | `Mode2` (+0x141) ≠ 0 and mode 0 (or none) and `Mode0` (+0x13F) = 0 → mode 2; then event 0 at f + 25 |
| 24 spike floor trap | `0x0054FB90` | 196, 261 | event 3 at f + 25 |
| 26 | `0x005500E0` | 259, 373 | mode 1 |
| 27 goo pile | `0x0054FC50` | 266 | C step: lo' mod 1000 ≤ 332 → `InteractType` := 3 (poison trap, §8.3), else 0 |
| 28 gold placeholder | `0x0054F8C0` | 269 | below |
| 34 hell brazier fire | `0x005500F0` | 358, 359 | C step: lo' & 1 = 1 → mode 1 |
| 37 | `0x0059DA50` | none | `world/quests-act2.md` §8.8 |
| 58 fissure | `0x0054FDB0` | 399 | C step; event 8 at f + 25 + lo' mod 250 |

**Gold placeholder** (init 28): mode ≠ 0 → nothing. Else mode 2; n :=
(C step, lo' mod 9) + 1; L := (x, y). n times: dx := C step & 3, dy := C
step & 3; room lookup from the init room at (L.x + dx, L.y + dy)
(`0x00463740`); found room = the init room → P := (x + dx, y + dy), L :=
P, and if the point query at P with mask 0x3F11 is free
(`0x0064D800(room, P, 1, 1)`): gold drop at P (`0x00559300`, §20.3).

### 18. Object events 0, 3, 8, 9, 10

Handlers (game, O) as `sim/units.md` §6.4. **Burn** (`0x00581680`): for
each player (type 0) in O's room unit list not in mode 17 with distance
(`0x006416D0`, `missiles/missiles.md` §R9.5) ≤ `Parm0` + 1: trap damage
`0x005DFA00(game, O, player, 1)`. Only O's own room is scanned.

#### 18.1 Event 0, fire (`0x00581700`)

1. O in mode 1 → its mode field := 2, written directly (no mode set: no
   animation, draw or update).
2. Burn.
3. C step; event 0 at f + 15 + lo' mod 35.

#### 18.2 Event 3, spike floor trap (`0x005818B0`)

State s := `InteractType`; "on it" = a player not in mode 17 at
distance ≤ 0.

- s = 0: for each player on it, sound 13 on O (no target; one per
  player). Any → s := 1, event 3 at f + 25. None → event 3 at f + 15.
- s = 1: each player on it takes trap damage `(game, O, player, 0)`.
  Any → s := 2 and mode 1; none → s := 0. Event 3 at f + (`FrameCnt1` >>
  8) + 1.
- s = 2: O's mode at entry ≠ 0 → mode 0, s := 0. Event 3 at f + 15.
- other s: event 3 at f + 15.

No draws.

#### 18.3 Event 8 (`0x00581250`)

- **Class 399 (fissure):** mode (low 16 bits) not 0 or 2 → nothing, and
  no reschedule. Else C step; event 8 at f + 25 + lo' mod 250; mode 1;
  ENDANIM.
- **Other classes** (init 10 only; no live row): p := Act I period of
  day (`0x0061C100(game act 0, none)`; `sim/stats.md` §8 note). p = 0 →
  mode 1 or 2 → mode 0; event 8 at f + 1000. p in 1..3 → mode 0 → mode 1
  and ENDANIM; event 8 at f + 1000. Other p → event 8 at f + 600.

#### 18.4 Event 9, trapped souls and burning bodies (`0x00585CE0`)

Schedulers: init 51 (`0x005501F0`): mode 1 or 2 → event 9 at f + 35.
Operate 48 (`0x005869F0`): mode ≠ 0 → return 0; r := C step mod 100; r <
90 → `D(0)` (§8, operator P), an item → mode 5; r ≥ 90 → mode 1,
ENDANIM, event 9 at f + 20. Returns 0. Live: 380, 381.

Handler:

- mode 1 → event 9 at f + 10.
- mode 2 → `InteractType` += 1 (u8). Below 2 → burn, event 9 at f + 25.
  Else `D(0)` with no operator; an item → mode 3 and event 9 at f +
  (`FrameCnt3` (+0xE4) >> 8) + 1; no item → nothing more.
- mode 3 → mode 4.
- other modes: nothing.

#### 18.5 Event 10, jungle stash drop (`0x00586850`)

`D(0)` with operator := the player (`0x00552F60(game, type 0, GUID)`)
whose GUID is O's timer-argument owner (`0x00552B10`); gone → none.

#### 18.6 Event 1, ENDANIM (`0x00581490`)

O present, its mode (low 16 bits) = 1 and `Mode2` (+0x141) ≠ 0 → the
mode field := 2, written directly (no mode set: no animation setup, no
draw, no queue, no changed flag); then, inside the same branch,
`HasCollision2` (+0x122) = 0 → free the footprint (`0x00623830`). Any
other case: nothing. (`sim/units.md` §6.4 type 1 lists the handler.)

### 19. Obelisk completion (C→S 0x44, `0x00585240`)

What follows §16.3's mode 3 (open question 9). The client answers the
0x58 result-0 dialog with C→S 0x44 (17 bytes: player GUID, object GUID,
item GUID, action); the entry checks and the action-2 (cancel) and
orifice branches are `world/quests-act2-2.md` §3.2. The handler
(`0x005852E0`) tests neither the object's class nor its mode: every
object other than class 152 takes this branch.

1. Action 3, object class ≠ 152: the item comes off P's cursor
   (`0x0055EEA0(game, P, item)`, items spec); failure → S→C 0x58 result
   4, stop. Then P's interact is cleared (`0x00554190`, §16.3 rule 2).
2. b := **power-up** (`0x00585240(game, s)`, P in EDI), s := byte +0x122
   of the item's items record (`subtype`, `data/fields.tsv` seq 62;
   the record of class −1 when there is no item):
   1. s ≥ the table count (dword `0x00732FAC` = 21; signed compare) →
      b := 0, no draw.
   2. r := `roll(100)` on **P's unit seed** (unit +0x20; one step,
      lo' mod 100).
   3. r ≥ entry s chance (unsigned) → b := 0.
   4. Else the entry's function is called (ECX game, EDX P, value) (a
      bad code pointer is fatal 0x10F5); b := its result (always 1).
3. S→C 0x58: 0x58, the object GUID (the GUID the client sent), result
   5 @5, b @6 (`0x0053D8D0`).
4. b = 1 → mode 1 (`0x00624690`), then ENDANIM at f + (`FrameCnt1` >> 8)
   + 1 (`0x005417D0`, type 1). b = 0 → mode 0. The item is gone in both
   cases. Return 1.

Power-up table `0x00732EB0` (21 entries of {function, chance, value}):

| s | Function | Effect on P (value v) | Chance |
|---|---|---|---|
| 0, 1, 2 | `0x00585120` | m := max mana (stat 9 total, `0x00625D60`) + v·256; set-stat 8 := m and 9 := m (`0x00627260`, layer 0) | 100, 100, 100 |
| 3, 4, 5 | `0x00585160` | base add stat 1 (energy) + v (`0x006272B0`) | 5, 10, 15 |
| 6, 7, 8 | `0x00585180` | base add stat 2 (dexterity) + v | 5, 10, 15 |
| 9, 10, 11 | `0x005851A0` | base add stat 3 (vitality) + v | 5, 10, 15 |
| 12, 13, 14 | `0x005851C0` | base add stat 0 (strength) + v | 5, 10, 15 |
| 15, 16, 17 | `0x005851E0` | m := max life (stat 7 total, `0x00625D10`) + v·256; set-stat 6 := m and 7 := m | 100, 100, 100 |
| 18, 19, 20 | `0x00585220` | base add stat 5 (new skills) + v | 3, 6, 10 |

v = 1 for every entry except 2 and 17 (v = 2). Live 1.14d: no
`weapons`, `armor` or `misc` row sets `subtype` (+0x122 is 0 in all
659 records of the `patch_d2` `.bin` files), so every accepted item
uses entry 0: one `roll(100)` (always < 100), max mana and mana := max
mana + 256 (one point), b = 1. §16.3's gem test (item type 20) only
gates the dialog; this branch accepts any item that comes off the
cursor.

### 20. Item drop helpers (open question 13)

Four helpers create a ground item for objects (and quests). Common
parts: the item-creation pipeline `0x00558D90(game, request, 0)`
(`items/generation.md` §3) with a zeroed 0x84-byte request (layout
there); spawn mode 3, init flags 1, format := game +0x78, x, y and room
from the placement; quality 0 (rolled) unless stated. "Area level" =
`0x0061DCA0(level of the room, difficulty game +0x6D, expansion game
+0x70)`. "Floor search" = `sim/path-placement.md` §9 (start (x + 2,
y + 3) when a room exists there, else (x, y); `0x0064E810(room, &start,
&out, 1, 0x3E01, 0x801, 1)`); §20.1 and §20.4 call `0x00555DA0`, §20.2
and §20.3 run the same steps inline. The request's room is the room the
search returns.

#### 20.1 Armor, `0x005594C0(game, room, &pos, type, act flag, unit)`

1. L := area level of the room's level; L > 1 → L − 1.
2. S := the room's seed (room +0x6C; no room → none).
3. id := armor pick (§20.5) (game, S, L, type, act flag); id < 0 →
   return none.
4. Floor search from pos (`0x00555DA0(room, &pos, &out, 1, 1)`); none
   → return none.
5. Request: unit := `unit` (+0x00), +0x04 := S, game, ilvl L, item id,
   flags2 := 0x40 (superior fallback). Create; return the item.

Operate 19 passes type −1, act flag 0, unit none (§16.5).

#### 20.2 Weapon, `0x00559630(game, room, &pos, type, act flag, unit)`

As §20.1 with the weapon pick (§20.5), tried up to 6 times: a pick is
kept at once when id ≥ 0 and its items record has `bitfield1` bit 1
(+0xDC & 2, `0x00629CC0`; no record → fatal 0x104A). After 6 failing
tries the 6th pick is still used when ≥ 0; < 0 → none. Then the floor
search and the request as §20.1 (flags2 0x40). Live: 200 of the 306
weapon rows have bit 1.

#### 20.3 Gold, `0x00559300(game, room, &pos)`

L as §20.1; S := room seed. Gold id: cached at `0x0088C6F8`, looked up
by code `gld ` (`0x00633640`) while the cache is 0. Floor search; none
→ none. Request: unit none, +0x04 := S, ilvl L, gold id, flags2 0.
Created item → its flag 0x2000 cleared (`0x006280D0(item, 0x2000, 0)`).
Returns the item. The amount is the pipeline's gold rule
(`items/treasure.md` §8, ilvl L).

#### 20.4 By source unit, `0x00559A30(game, U, quality, &level, &request out, type, act flag)`

1. Game none → fatal 0x9C7.
2. Level: `world/quests-act3-2.md` §11.3 (monster: total stat 12;
   player: base stat 12; other: area level of U's room; ≤ 1 → 1);
   written to `*level`.
3. U +0xB8 (drop code) ≠ 0 → id := `0x00633680(code)`; −1 → fatal
   0x9EA. No draw.
4. Code 0 → id := random class (§20.6) with S := U's unit seed (+0x20)
   and m := (U is a monster). If quality = 4: while id's items record
   is missing or lacks `bitfield1` bit 0, re-pick: the first 10
   re-picks with §20.6, later ones with the weapon pick (§20.5)
   (unbounded loop).
5. Position: U's position (`0x00620870`) and room (`0x00620BB0`); floor
   search `0x00555DA0(room, &pos, &out, 1, 1)`; none → return none
   (request out untouched).
6. Request: unit := U, +0x04 := 0, ilvl := `*level`, id, quality :=
   `quality`, flags2 0. Create. Request out ≠ none → the 0x84 request
   bytes (after creation) are copied to it. Return the item.

#### 20.5 Class picks (`0x00555E70` armor, `0x00555FB0` weapon, `0x005560F0` misc)

Arguments: game, seed S, level L (< 1 → 1), type t (−1 any), act flag
a; misc also m. The range is the armor, weapon or misc part of the
items table (combined index = part base + row). No part → −1.

1. For each row in order, it is a candidate when (filter `0x00555E00`):
   `spawnable` (+0x133) > 0, `quest` (+0x12A) = 0, `level` (+0xFD) ≤ L;
   if a = 0: n := `rarity` (+0xFC) − A(L) > 0 → `roll(n)` on S must be 0
   (n ≤ 0: no draw, passes); t = −1 or `type` (+0x11E) = t; classic game
   (game +0x70 = 0): `version` (+0xF6) < 100; fewer than 1023
   candidates so far. Misc only: when m = 0, rows of item type 40
   (`body`) are skipped before the filter (no draw).
2. k := candidates; k > 0 → j := `roll(k)` on S; return candidate j's
   index. k = 0 → no draw and the function returns the uninitialised
   first slot of its stack buffer (edge case 3).

A(L) = `0x006427F0(L)`, the item level used as a **level id**: the act
whose first level id it does not reach (thresholds 1, 40, 75, 103, 109,
1024 at `0x006EB2F0`): L < 40 → 0, < 75 → 1, < 103 → 2, < 109 → 3, <
1024 → 4, else 0.

#### 20.6 Random class (`0x00556240`, L in EDI)

L > 65 → fatal 0x180. The gold id is cached at `0x008846EC` (flag
`0x008846F0` bit 0; code `gld ` via `0x00633680`; −1 → fatal 0x17C, 0
→ fatal 0x17F). g := 65 − L; r := `roll(100)` on S. r < g → gold; r <
g + ⌊L/2⌋ + 5 → armor pick; r < 80 → weapon pick; else misc pick (with
m). So gold (65 − L) %, armor (⌊L/2⌋ + 5) %, weapon (⌈L/2⌉ + 10) %,
misc 20 %.

### 21. Curable-state removal (`0x00578C20`, open question 15)

For each state s = 0 … count − 1 (data +0xC4; 185 live), in order: P
has state s (`0x00639DF0`), s has the `curable` flag (bitset 12,
`0x0063A460`; `data/runtime-maps.md` §4) and P has a stat list of state
s (`0x006256B0`) → detach and free it (`0x006277E0`, `0x00626CD0`,
`sim/stat-lists.md` §8.2, §8.3). Returns 1 if any list was removed,
else 0. No draws. Live curable states: 9 amplifydamage, 19 weaken, 23
dimvision, 27 taunt, 55 ironmaiden, 56 terror, 57 attract, 58 lifetap,
59 confuse, 60 decrepify, 61 lowerresist, 113 defense_curse, 114
blood_mana. The well (`world/objects.md` §11) runs it after the poison
and freeze removals.

### 22. Object allocation modes (open question 8)

No 1.14d path gives a door mode 6, so §10's locked branch (sound 22,
key test) is unreachable with live data:

1. Every call of unit allocation `0x00555230` that can create an
   object passes mode 0, 1 or 2 as a constant, except `0x005557D0`
   (stack mode argument), whose callers are the preset path
   (`0x00555910`, mode = preset unit +0x00; the 574–579 handler
   `0x0054F0D0` forwards it) and the inactive-unit restore
   (`0x00542B40`, the saved mode, record +0x10, of a unit that existed
   before).
2. Preset units of objects carry mode 0: DS1 object records
   (`drlg/preset.md` §5.3, `0x00665B20`), the generated objects of
   `0x00666170`, `0x006661A0` and `0x00666220`, the door presets
   (`0x0066D9E0`, `drlg/preset.md` §11); `0x0066FA10` copies an existing
   unit's mode.
3. The only constant mode-6 sets (`0x00624690` callers) are `0x005627FC`
   (an item: socketing) and `0x00593947` (operate 9, the monolith,
   `world/quests.md`); no instruction stores the constant 6 into a
   unit's mode field.

Doors therefore start in mode 0 (or 2 by §3 rule 8) and §10 only moves
them among 0, 2, 4 and 5.

### 23. Client side of S→C 0x0E and 0x4D (open question 4)

1. 0x0E byte @6 is the client's mode-request code (`client/msg-units.md`
   §4, handler `0x0045CD10`: code u8@6, record u8@7, u32@8); the object
   machine `0x004BD6D0` runs code 3 as the object mode change
   (`0x004BCF60`, then `0x004BD650` when `0x00621B00(U)`); any other
   code is fatal 0x39C there. So 3 is a fixed protocol constant.
2. 0x4D is code 0x15 (`0x0045DF60`: record u32@6, −1, u16@11, u16@13,
   u8@10, 0, 0); `0x004BD5C0` reads only record[0] (the operator GUID,
   as a player unit, `0x00463990`) and takes the shrine code from the
   client's own object (shrine data byte 0 via `0x00621B70`, else
   `objects` +0x16F), then calls the client shrine function (table
   `0x006DA8C4`, 20-byte entries) and `0x004BD550`. Bytes @10, @11, @13
   are copied but never read; @15 is not copied.
3. The server must still send §14's bytes exactly (zeros included) for
   byte-equal traces. The client rows belong to `client/model.md` §8
   rule 5 (cross-file request in `docs/handoff/pc2-spec-objects.md`).

## Constants & data dependencies

Listed in `world/objects.md` (Constants & data dependencies).

## Randomness

`world/objects.md` Randomness items 9–13. Added here:

1. Obelisk completion (§19): one `roll(100)` on P's seed when s < 21,
   then the mode change's draw (U).
2. Drop helpers (§20): armor, weapon and gold draw on the **room seed**
   (room +0x6C): the pick's act rolls (one per row with n > 0, in row
   order, when the act flag is 0) and `roll(k)`; the weapon helper
   repeats the pick up to 6 times. `0x00559A30` draws on U's unit seed:
   `roll(100)` (§20.6), then the chosen pick's draws, repeated by the
   quality-4 loop; with a drop code it draws nothing itself. The
   pipeline's own draws follow (`items/generation.md`).
3. Curable-state removal (§21): none.

## Edge cases & original bugs

`world/objects.md` edge cases 13–21. Added here:

1. **ENDANIM writes mode 2 directly** (§18.6): clients get no mode
   update from it, and the footprint is freed only on that branch.
2. **C→S 0x44 accepts any object** (§19): a player can send the gem
   insert for any object GUID (other than the orifice) and any item
   on the cursor; the item is consumed and the power-up rolled even
   when the object is not an obelisk in mode 3.
3. **Empty pick returns stack garbage** (§20.5): with no candidate the
   picks return an uninitialised dword; a caller treats a negative
   value as none, other values reach item creation unchecked. d2rs:
   return −1 (none) and record it as a divergence. With t = −1 live
   data never gets there: armor, weapons and misc each have a row of
   `level` ≤ 1 and `rarity` 1, which passes at every L ≥ 1 (n ≤ 1, so
   `roll(n)` is 0).
4. **Quality-4 loop can spin forever** (§20.4) when no reachable class
   has `bitfield1` bit 0.
5. **The pick's act uses the item level as a level id** (§20.5):
   item level 40 counts as Act II, 109 and above as Act V.
6. **The weapon rack keeps a 6th failing pick** (§20.2).
7. **Locked doors never occur** (§22): §10's mode-6 branch and sound 22
   are dead with live data.

## Test vectors

`world/objects.md` Test vectors (rows for §16–§18).

| Input | Expected | Source |
|---|---|---|
| event 1, O mode 1, `Mode2` 1, `HasCollision2` 0 | mode field 2, footprint freed, no flag 0x1 | §18.6 |
| event 1, O mode 2 | nothing | §18.6 |
| obelisk 0x44 action 3, item `subtype` 4, P seed {1, 666} | lo' = 1791398751, r = 51 ≥ 10 → b 0: 0x58 result 5, @6 0; object mode 0; item consumed | §19 |
| same with `subtype` 0 (every live item) | r = 51 < 100: max mana and mana := max mana + 256; @6 1; mode 1, ENDANIM | §19 |
| random class, S = {1, 666} (r = 51), L = 10 / 20 / 40 | gold (51 < 55) / armor (45 ≤ 51 < 60) / weapon (50 ≤ 51 < 80) | §20.6 |
| A(L), L = 39, 40, 108, 109, 1024 | 0, 1, 3, 4, 0 | §20.5 |
| well heal, P with states 9 and 60 | both lists removed, result 1 | §21 |

## Provenance

The §16–§18 bullet of `world/objects.md` Provenance; §18.6 from
`0x00581490` (1.14d disassembly).

- §19: `0x005852E0` (0x5853A9–0x585460), `0x00585240`, table
  `0x00732EB0` / count `0x00732FAC` (read from `Game.exe`), callbacks
  `0x00585120`–`0x00585220`; live `subtype` bytes from the `patch_d2`
  `weapons/armor/misc.bin` (record 0x1A8, +0x122). D2MOO
  `OBJMODE_MainObeliskHandler` gave names only.
- §20: `0x005594C0`, `0x00559630`, `0x00559300`, `0x00559A30`,
  `0x00555E00`, `0x00555E70`, `0x00555FB0`, `0x005560F0`, `0x00556240`,
  `0x00629CC0`, `0x006427F0` (thresholds `0x006EB2F0`).
- §21: `0x00578C20`, `0x0063A460` (bitset 12 = `curable`, cross-checked
  with bitset 2 `hide` at +0xD4 and 33 `udead` at +0x150); live
  `states.txt` `curable` column.
- §22: every `0x00555230` call site (pushed mode argument), the
  `0x0066BF30` callers, the `0x00624690` callers with a constant 6, and
  an `all.asm` search for stores of 6 at +0x10.
- §23: `0x004BD6D0`, `0x004BD5C0` (client); `client/msg-units.md` §4.

## Open questions

Kept in `world/objects.md`.
