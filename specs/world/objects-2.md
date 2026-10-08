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
| Summary | 43–52 |
| Inputs | 53–56 |
| Outputs / state changes | 57–61 |
| Rules | 62–63 |
|   16. Operate functions, part 2 | 64–191 |
|   17. Small init functions | 192–218 |
|   18. Object events 0, 3, 8, 9, 10 | 219–286 |
|   19. Obelisk completion (C→S 0x44, `0x00585240`) | 287–333 |
|   20. Item drop helpers (open question 13) | 334–429 |
|   21. Curable-state removal (`0x00578C20`, open question 15) | 430–442 |
|   22. Object allocation modes (open question 8) | 443–491 |
|   23. Client side of S→C 0x0E and 0x4D (open question 4) | 492–509 |
|   24. Guards and corner cases of part 1 (read 2026-10-07) | 510–551 |
|   25. Portal pair creation (`0x0056D130`, `0x0056CF40`) | 552–643 |
|   26. Shrine state lists and shrine texts (REC-239, read 2026-10-08) | 644–737 |
| Constants & data dependencies | 738–741 |
| Randomness | 742–756 |
| Edge cases & original bugs | 757–781 |
| Test vectors | 782–813 |
| Provenance | 814–856 |
| Open questions | 857–860 |
<!-- /index -->

## Summary

Part 2 of `world/objects.md`, split at § boundaries to keep each file
under 60 KB; section numbers continue from part 1. It owns the generic
operate functions beyond §8–§13, the small init functions, and the
object event handlers 0, 1, 3, 8, 9 and 10; §19–§23 answer open
questions of part 1 (obelisk completion, the item drop helpers, the
well's curable-state removal, object allocation modes, the client side
of 0x0E / 0x4D); §24 settles guards and corner cases of part 1.

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
   of the item's items record (`subtype`, `data/fields.tsv` seq 62):
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
   is missing or lacks `bitfield1` bit 0, re-pick: the first 11
   re-picks with §20.6, later ones with the weapon pick (§20.5, level
   `*level`) (unbounded loop).
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

4. Allocation mode per object call site (type 2, or a type register
   that holds 2; the mode is the sixth stack argument of `0x00555230`,
   stack order x, y, game, room, flag, mode, GUID). Read from the
   pushes before each call (2026-10-07):

   | Mode | Call sites (function) | Creates |
   |---|---|---|
   | 0 | `0x0054E03B` (`0x0054DF80`) | object 562 at an evilhut pack leader (`monsters/population.md` §9 rule 6) |
   | 0 | `0x0054E5AA`, `0x0054E5EA` (`0x0054E490`) | objects 571 / 572 at barricade-door monsters 432 / 433 (`monsters/population.md` OQ3) |
   | 0 | `0x0054F35D` (`0x0054F180`), `0x0054F478` (`0x0054F430`) | presets 581 / 580, 582 (§6) |
   | 0 | `0x00550526`, `0x005506B8`, `0x00550C07` (random, spread, oriented spot) | population (`world/object-population.md` §6) |
   | 0 | `0x00550ED0`, `0x005510F8` (fn 1), `0x005511E8` (flies 103, `0x00551150`), `0x00551408` (fn 7), `0x00551A4B`, `0x00551BA7` (fn 4), `0x00551E56`, `0x00551F9D` (fn 5), `0x005520DD`, `0x005522F0`, `0x0055238E` (themes) | population |
   | 0 | `0x005B6B43` (`0x005B6AD0`), `0x005BD479` (`0x005BD390`) | object 131 (quest and monster code, `world/quests-act3.md`, `world/quests-act4.md`) |
   | 1 | `0x005823B9`, `0x00582408` (`0x00582380`) | fire objects 162 / 160 of traps 5 / 7 (§8.3) |
   | 1 | `0x0056D249` (`0x0056D130`) | town portal 59 (§12) |
   | 1 | `0x00588A13`, `0x00588A3B`, `0x00588A90`, `0x00589313`, `0x0058A55D`, `0x0058A8A2`, `0x0058CB1C`, `0x0058D847`, `0x00594429`, `0x005945C2`, `0x0059B779`, `0x0059B7AB`, `0x0059D969`, `0x005B4648` | quest objects (classes 189, 558, 561, 565, 318, 100, 566; `world/quests*.md`) |
   | 2 | `0x0056D092` (`0x0056CF40`) | portal object of `0x0056CF40` (`monsters/population.md` §1) |
   | argument | `0x0054F131` (`0x0054F0D0`, presets 574–579), `0x00555843` (`0x005557D0`, DS1 presets and the inactive-unit restore, rule 1) | the preset's or the saved mode |

   Every population and monster-pack object is therefore allocated in
   **mode 0**; its init (§3 rule 6) and PreOperate (§3 rule 8) give the
   rest. The other `0x00555230` calls allocate players (type 0),
   monsters (type 1), missiles (type 3) or items (type 4).

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

### 24. Guards and corner cases of part 1 (read 2026-10-07)

Points the implementation of `world/objects.md` §6–§12 read narrowly;
each is settled from the 1.14d function named.

1. **Preset 580's last mode set** (`0x0054F370`, §6) is the ordinary
   mode set `0x00624690(O, 0)` (`sim/units.md` §4). When O is already
   in mode 0 it runs no animation setup and draws nothing, but it still
   queues O for update and sets flag 0x1, so the 0x0E of §14 is sent.
2. **Barrel, operate 5** (`0x005868A0`): the only test is "object
   missing, or its mode is 0"; the "when the object exists" guard of the
   flag clear protects only the null object, which the dispatch (§7.2)
   never passes. So with an object in mode 0 the selectable flag is
   always cleared. The skill start runs only when the operator exists
   and is a player (type 0) and its skill `0x006439B0(P, −1)` exists.
   Order: skill start, mode 1, clear 0x2, free the footprint, trap
   `roll(10000)`, drop `roll(100)`, ENDANIM.
3. **Shrine storm (19)** (`0x00582DA0`, §9.3): the life loss is only
   the base-stat add on stat 6; nothing tests the result or kills. A
   unit can be left with life ≤ 0 and no death runs from the shrine.
   The finder keeps units of type 0 or 1 only; each is also skipped
   when `0x005541B0` reports it dead.
4. **Shrine event 6** (`0x00581620`, §9.1): no object, or an object
   whose hover (+0xA4) is null → nothing, and no reschedule. Otherwise
   hover expiry ≤ the game frame → free the hover (`0x006611A0`),
   +0xA4 := 0, queue, flag 0x100; else event 6 again at the expiry.
5. **Well "used"** (`0x00585720`, §11): set by each heal **write**, not
   by a changed value. A write happens when the stat's total is below
   its maximum (life only with `Parm3` & 2, mana only with `Parm3` & 1,
   stamina always); the written value min(total + ((max · `Parm1`) >>
   8), max) may equal the total (`Parm1` 0 or a small max) and still
   counts. Then each removed poison (state 2) or freeze (state 1) stat
   list, `0x00578C20` ≠ 0 and the pet callback set it.
6. **Locked door with no operator** (§10 mode 6): the key test runs
   with a null unit and is fatal as for chests (§8.1 rule 8); with live
   data no door reaches mode 6 (§22).
7. **Well event 2 mode set** (§11): the refill's mode set is the
   ordinary `0x00624690`, which queues and sets flag 0x1 itself. The
   explicit queue and flag 0x1 after it (`0x00581510`) run on every
   refill that adds a charge, also when the mode rule sets no mode;
   after a mode set they repeat it (no second effect).

### 25. Portal pair creation (`0x0056D130`, `0x0056CF40`)

`0x0056D130(ECX game, EDX owner unit or null, room, x, y, level, out,
class, exact)` (`ret 0x1C`) creates a pair of portal objects: object 1
at (x, y) in the caller's act, object 2 in `level`, linked to each
other. Returns 1 when both exist, else 0. Callers (12): NPC travel in
the same act `0x0054B8F3` (class 59, owner the player, `world/npc.md`
§8.3), the portal shrine effect 17 `0x00582AA8` (`world/objects.md` §9.2), quest
code `0x0058B01D`, `0x0058BD33`, `0x00592DE8`, `0x005933EB`,
`0x00594228` (cow portal), `0x005964D3`, `0x0059B0E7`, `0x0059CBA8`
(Tyrael, `world/quests-act2.md` §8.11), `0x005A99BD`, and the town portal
cast `0x005BE290` at `0x005BE32A` (its use path: `skills/bodies-3.md` §4.4).

1. Room null → fatal 0xE42 (`0x0056D147`).
2. `out` ≠ null → *out := null.
3. Town refusal: owner ≠ null and the room is in a town (`0x0061AB00`)
   and not (level ∈ {39, 133, 134, 135, 136} and class = 60) → sound
   event 24 on the owner (`0x00553380(owner, 24, owner)`, `notintown`,
   `audio/triggers-2.md`), return 0.
4. Act of `level` ≠ act of the room's level → fatal 0xE5C (`0x0056D1E1`):
   a portal never leads to another act.
5. Spot: (x, y) as given when `exact` ≠ 0; else the field search
   `0x0064E810(room, &(x, y), origin (x, y), size 3, mask 0x3E01, field
   mask 0xC01, no fallback)` (`sim/path-placement.md` §7.1, §7.2), none
   → return 0. Then the cell lookup `0x00463740` of (x, y) **from the
   argument room** (the room the search found is only tested); none →
   return 0. Call that room R1.
6. Object 1 := `0x00555230` (type 2, `class`, x, y, R1, flags 1, mode 1,
   GUID 0; `0x0056D249`). *out := object 1. Mode set `0x00624690(object
   1, 1)` (already mode 1: update mark only). Its init (§3) runs inside
   the allocation: class 59 init 11 sets its destination (data +0x04) to
   the town of R1's act; class 60 init 12 to its level map
   (`world/objects.md` §5.5).
7. Object 2 := `0x0056CF40(game, object 1, level, R1's level)` (rules
   9–15); none → return 0 (object 1 is already freed then).
8. Object 2's portal flags (data +0x05) |= 3 (again, after rule 14);
   the "has portal" flag of R1's DRLG room and of object 2's room
   (`0x0061AED0(room, 0)`, `drlg/rooms.md` §8 rule 1: the rooms are not
   freed while it is set). Return 1.

`0x0056CF40(ECX game, object 1, level, source level)` (`ret 0xC`):

9. S := `0x0061B060(act of level's record, level, tile index 11, &sx,
   &sy, size 3)` (`sim/path-placement.md` §11); populate S
   (`0x0052D0F0`) **before** the null test (`monsters/population.md`
   §1 rule 2).
10. S null → remove object 1 (`0x0061A270(object 1's room, 2, its
    GUID)`, free `0x00555600`), return 0.
11. Point p := (sx, sy). Arrival hook `0x00545830(game, &p, source
    level, &R)` (`world/quests.md` §8.3): only source level 73 runs
    `0x0059DFD0`: when chain 13's record has +0x3C = 1 (Tyrael's flag,
    `world/quests-act2.md` §8.11), T := `0x0061B060(act II record (game
    +0xC0), 40, tile index 12, &tx, &ty, 3)`; T found → populate T, then
    free point `0x0064E7E0(T, &p, 3, mask 0xBE11, step 7)` from p (the
    tile-11 point); found room F → p := (tx, ty) (the raw tile-12 spawn
    point; the free point found is dropped), R := F, hook result 1.
    Otherwise result 0 and p, R unchanged.
12. Hook result 1 → R := cell lookup of p from R (`0x00463740`); else R
    := S. R null → R := S, p := (sx, sy).
13. Free point `0x0064E7E0(R, &p, 3, mask 0xBE11, step 5)`; found → R :=
    its room. Not found → R := a second `0x0061B060(level, 11, size 3)`
    (it may draw again), or S when that returns null; p stays as given
    to the failed search (§7.2 rule 4). (The fatal 0xE08 after it is
    unreachable: S ≠ null.)
14. Object 2 := `0x00555230` (type 2, class = object 1's class, p, R,
    flags 1, mode 2, GUID 0; `0x0056D092`); null → remove and free
    object 1 as rule 10, return 0. Object 2's destination (data +0x04)
    := the level of object 1's room (> 255 → fatal 0xE2B); portal flags
    |= 3; mode set `0x00624690(object 2, 2)` (update mark only).
15. Link (`0x00553590(object 1, object 2)`): each object's data +0x20 /
    +0x24 := the other's position x / y, and +0x18 / +0x1C := the tile
    x / y of the other's room (`0x00619730` words 4 and 5; skipped when
    the other has no room). Partner (`0x00621CE0`, both ways): unit
    +0x94 := the other's type, +0x98 := the other's GUID, flag-ex
    (+0xC8) |= 0x400; a unit with a stat list (+0x5C ≠ 0) also gets
    state 98 and stats 353 / 354 holding the type and GUID. Return
    object 2.

Neither function sets the owner GUID (timer argument, `world/objects.md`
§1, read by §12 rule 4): it stays −1 unless the caller writes it (the
town portal cast `0x005BE290` does after the call). Draws: `world/cube.md`
open question 4 (one game-seed step per object, plus the level build
and population of rules 9, 11 and 13).

Tyrael's portal (`0x0059CBA8`): owner the player, the player's room
and position, level 40, class 59, exact 0, out null. Object 1 stands at
the free spot nearest the player in Duriel's Lair (73), destination 40
(init 11: town of act II); object 2 stands in Lut Gholein at the free
point (mask 0xBE11, step 5) nearest its tile-12 spawn point (rule 11,
because +0x3C is 1 during the call), destination 73. Travel through it
is §12 of `world/objects.md` with partner L = object 2.

### 26. Shrine state lists and shrine texts (REC-239, read 2026-10-08)

The timed-state shrines (`world/objects.md` §9.2 codes 6–15) go through
the state helper `apply_state` `0x0056E970` (`skills/bodies.md` §2.7,
owner of its rules). This section gives the request the shrines pass and
what the helper's rules then mean for them.

#### 26.1 The request

`0x00582800(ECX source, EDX target, duration, value, stat, state,
callback)` fills the helper's request: source := the shrine object,
target := P, **skill := 0, level := 0**, duration := the shrine's
`Duration in frames` (+0x0C), stat, value, state, and callback (0 → the
default `0x0056E900`).

| Code | Caller | stat, value | state | callback | after the call |
|---|---|---|---|---|---|
| 6, 8–11, 13, 15 | `0x00583B30` | table stat, V(stat, `Arg0`) (table stat −1 → value 0; not live) | table | 0 | – |
| 7 | `0x005839B0` | 25, `Arg1` (+0x08) | table (129) | 0 | stat 19 := V(19, `Arg0`) on the returned list |
| 12 | `0x00583BF0` (builds the request itself) | 11, 0 | table (134) | `0x00583BD0` | `0x0056DE40(P)`, whether or not a list was made |
| 14 | `0x00583A70` | 162, v = V(162, `Arg0`) | table (136) | `0x00583A40` | stamina was set to its maximum **before** the call; on the returned list stat 10 := 2v, stat 28 := 1000 |

Table = `0x006E1850` (`world/objects.md` §9.2). A code ≥ `[0x00732EAC]`
is fatal in `0x00583B30` / `0x005839B0` / `0x00583A70` (0x9F9 / 0x9C1 /
0x9E7). §9.1 rule 4 already rules that out. A set on a returned none
does nothing (`0x006270B0` returns 0 on a null list). A 0 value on a
missing stat writes nothing, so code 12's list holds no stat.

#### 26.2 The helper's rules applied to a shrine (live 1.14d data)

1. **State check:** 128–137 are valid; never refuses.
2. **Monster rules:** the target is the operating player; never apply.
3. **Curse path.** All ten shrine states (128 `shrine_armor` … 137
   `shrine_experience`) have `states.txt` `curse` = 1, so:
   1. r := P's `curse_resistance` (stat 109). Its only live source is
      the Assassin's Fade (skill 267, `aurastat5`, calc `dm34`). r ≥ 100
      → no list; 0 < r < 100 → duration −= pct(duration, r, 100)
      (`combat/damage.md`).
   2. P has state 57 (`attract`) → no list.
   3. Old := P's **first stat list with flag 0x20**: any curse list, so
      another shrine's state or a monster's curse (e.g. Amplify Damage).
      The new list's flags start at 0x20.
4. **Old list.**
   1. Same state (skill 0 and level 0 always match a shrine's own list):
      **refresh only**. Expiry := F + duration (after rule 3.1), timer 12
      re-armed, the old list returned. Its stats are **not** rewritten:
      the V just computed is dropped. Only the caller's sets after the
      call (code 7 stat 19; code 14 stats 10 and 28) write the old list.
   2. Another state: the "request level < old level → none" refusal
      needs the same state and skill, so it never fires for a shrine.
      The old list is detached and freed, and its remove callback runs
      (§26.3 for 134 / 136; curses: `skills/bodies.md` §2.8).
   So a player holds at most one shrine state or curse at a time. A
   shrine replaces the current curse or shrine state. A later curse
   replaces the shrine state by the same rule (on the curse's request).
5. **New list:** P is queued for update; state on. Every state shrine
   has duration ≠ 0 (2,400–4,800), so e = F + duration, timer 12 at e,
   flags 0x22 (none of 128–137 has `exp` or `aura`). Source := (type 2,
   the shrine's GUID), state, skill 0, level 0; stat ≠ −1 → list stat :=
   value; attached; remove callback := the request's, else `0x0056E900`.
   Expiry and the type-12 timer: `sim/stat-lists.md` §10.4.

#### 26.3 Remove callbacks

Detach calls them (ECX unit, EDX state, stack list; `sim/stat-lists.md`
§8.2 step 6) on expiry, on replacement (§26.2 r4.2) and on any other
removal.

- **`0x00583BD0`** (state 134, skill shrine): the default callback
  `0x0056E900` (`skills/bodies.md` §2.8) with the same arguments, then
  the skill refresh `0x0056DE40(unit)` (`skills/use.md` §7), which drops
  the +2 of state 134 (`skills/levels.md`).
- **`0x00583A40`** (state 136, stamina shrine): the default callback with
  the same arguments, then stamina (stat 10) := the unit's maximum
  stamina (`0x00625DB0`) through the unit set `0x00627260(unit, 10, max,
  layer 0)`. This runs whether the unit is alive or not.

#### 26.4 Shrine texts on the client

- **Overhead message** (server: `world/objects.md` §9.1 rule 3 and event
  6). The text is the decimal string of 3683 + shrine id: 4 characters
  for every id 0–22 ("3683" … "3705"), so the record lives 8 · 4 + 125
  = 157 frames. It is sent as S→C 0x26 type 5 (unit type 2, the object's
  GUID, empty name; `sim/intents-events.md` §7.9 r3). Event 6 at +300
  frees the server record and sends S→C 0x76.
- **Client.** `client/msg-ui.md` §4 r4 stores an overhead record (d =
  8 · 4 + 125 = 157 overhead draws). `ui/messages.md` §5 draws it as an
  object: `atol` of the text → string 3683 + id (`ShrMsg0`–`ShrMsg22`,
  e.g. 3684 "You feel refreshed.", 3689 "Your skin hardens."), in a
  font-13 bubble above the object (py − 10). S→C 0x76 frees the record
  if it is still there (`client/msg-ui.md` §21).
- **Mouse-over label:** the shrine's name, string 10809 + shrine id
  (`world/objects-client.md` §29 r4.3).

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
| well, `Parm1` 0, `Parm3` 3, P life 10 of max 20 (8.8), mana and stamina full | life written 10 (unchanged), used: charges 2 → 1, mode 1 | §24 rule 5 |
| preset 580, init leaves mode 0 | no draw, O queued, flag 0x1 (0x0E sent) | §24 rule 1 |
| population / evilhut / barricade object | allocated in mode 0 | §22 rule 4 |
| `0x0056D130`, owner P in a town room, level 1, class 59 | sound 24 on P, result 0, no object | §25 rule 3 |
| same, level 39, class 60 (cow portal from Rogue Encampment) | town test passed; pair created | §25 rule 3 |
| destination level in another act | fatal 0xE5C | §25 rule 4 |
| pair created | object 1 mode 1, object 2 mode 2, both class `class`; object 2 +0x04 = object 1's level; each +0x94/+0x98 = the other's type/GUID; each +0x20/+0x24 = the other's position; object 2 portal flags & 3 = 3; owner GUID −1 on both | §25 rules 6, 14, 15 |
| Tyrael (msg 302) from level 73, chain 13 +0x3C = 1 | object 2 in level 40 at the free point (mask 0xBE11, step 5) nearest the tile-12 spawn point | §25 rule 11 |
| spawn search of rule 9 returns none | object 1 removed and freed; result 0 | §25 rule 10 |
| armor shrine (code 6, `Arg0` 100, duration 2,400) at frame F, P without a curse list | one list: state 128, flags 0x22, source (2, shrine GUID), skill 0, level 0, stat 171 = 100, expiry and timer 12 at F + 2,400 | §26.1, §26.2 r5 |
| the same armor shrine type again at F + 100 | no new list; expiry and timer F + 2,500; stat 171 still the first value | §26.2 r4.1 |
| resist fire shrine (code 8) at F + 200 while the armor list is on | armor list freed (state 128 off, default callback); new list state 131, stat 39 = 75, expiry F + 3,800 | §26.2 r4.2 |
| armor shrine while P has a monster curse list (flag 0x20) | the curse list is freed (its callback runs); shrine list made | §26.2 r3.3, r4.2 |
| resist cold shrine (3,600), P's `curse_resistance` 50 | expiry F + 3,600 − pct(3,600, 50, 100) = F + 1,800 | §26.2 r3.1 |
| any shrine state, P's `curse_resistance` 100 | no list; code 14 still refilled stamina, code 12 still refreshed skills | §26.1, §26.2 r3.1 |
| stamina shrine list expires | default callback, then stamina := max stamina | §26.3 |
| skill shrine list replaced by a resist shrine | default callback, then skill refresh `0x0056DE40` | §26.3 |
| shrine id 1 operated | S→C 0x26 type 5 text "3684"; client bubble "You feel refreshed." for 157 overhead draws; S→C 0x76 at server frame + 300 | §26.4 |

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
- §22 rule 4: the pushes before all 53 `0x00555230` calls in `all.asm`
  (script outside the repo), with `disasm.py at` for the sites whose
  mode or type is a register (`0x00550ED0`, `0x00551408`, `0x00551A4B`,
  `0x00551BA7`, `0x0056D092`, `0x00588A3B`, `0x0059D969`).
- §24: `0x0054F370`, `0x005868A0`, `0x00582DA0`, `0x00581620`,
  `0x00585720`, `0x00581510`, `0x00624690`.
- §25 (read 2026-10-08): `0x0056D130` (`0x0056D130`–`0x0056D2BC`),
  `0x0056CF40` (`0x0056CF40`–`0x0056D12B`), `0x00545830`, `0x0059DFD0`,
  `0x00553590`, `0x00619730`, `0x00621CE0`/`0x00621C30`, `0x0061AED0`/
  `0x0061BAC0`, `0x00552AF0`/`0x00552B10` (owner set / get); the 12 call
  sites of `0x0056D130` from `all.asm`; Tyrael's pushes at
  `0x0059CB7A`–`0x0059CBA8`; the town portal cast's owner writes at
  `0x005BE366`, `0x005BE380`. D2MOO 1.10f `D2GAME_CreatePortalObject_6FD13DF0`
  gave the name only.
- §26 (read 2026-10-08, `all.asm`): `0x00582800` (request offsets),
  `0x00583B30`, `0x005839B0`, `0x00583A70`, `0x00583BF0`, `0x00583BD0`,
  `0x00583A40`, `0x006270B0` (null list → 0); the helper's rules from
  `skills/bodies.md` §2.7 (`0x0056E970`). Live `patch_d2` `states.txt`
  rows 128–137 (`curse` 1; `exp`, `aura` empty), `shrines.txt` (`Arg0`,
  `Arg1`, durations), `skills.txt` row 267 (`aurastat5`
  `curse_resistance`; no other row or property sets stat 109). Strings:
  English `string.tbl` 3683–3705 (`ShrMsg0`–`ShrMsg22`).

## Open questions

Kept in `world/objects.md`.
