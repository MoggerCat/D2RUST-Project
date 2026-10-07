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
| Summary | 35–41 |
| Inputs | 42–45 |
| Outputs / state changes | 46–50 |
| Rules | 51–52 |
|   16. Operate functions, part 2 | 53–179 |
|   17. Small init functions | 180–206 |
|   18. Object events 0, 3, 8, 9, 10 | 207–274 |
| Constants & data dependencies | 275–278 |
| Randomness | 279–282 |
| Edge cases & original bugs | 283–289 |
| Test vectors | 290–298 |
| Provenance | 299–303 |
| Open questions | 304–307 |
<!-- /index -->

## Summary

Part 2 of `world/objects.md`, split at § boundaries to keep each file
under 60 KB; section numbers continue from part 1. It owns the generic
operate functions beyond §8–§13, the small init functions, and the
object event handlers 0, 1, 3, 8, 9 and 10.

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
   holds a shrine record).

#### 16.4 Secret door, operate 18 (`0x00583FF0`)

Mode 0 (or none): mode 1, clear flag 0x2, ENDANIM, free the footprint
(`0x00623830`). Return 1. Live: 129.

#### 16.5 Armor stand, operate 19 (`0x00584160`); weapon rack, operate 20 (`0x005841D0`)

Mode 0 (or none): drop at O's position in O's room, armor
`0x005594C0(game, room, &pos, −1, 0, 0)` or weapon `0x00559630(…)`
(items spec); then mode 2, clear flag 0x2. Return 1. The drop's draws
come before the mode change's §4 draw. Live: stands 104, 105, 550, 551;
racks 106, 107, 548, 549.

#### 16.6 Bookshelf, operate 26 (`0x00584060`)

Mode 0 (or none):

1. Mode 2, clear flag 0x2.
2. r := C step, lo' mod 20 (C = control seed). r < 13: C step, lo' & 1 =
   0 → code `tsc `, 1 → `isc `. r ≥ 13: C step, lo' & 1 = 0 → `tbk `, 1 →
   `ibk `. Drop item code (+0xB8) := it.
3. Drop it at O (`0x00559A30`, quality argument 2; items spec).

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
(`0x0064D800(room, P, 1, 1)`): gold drop at P (`0x00559300`, items spec).

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

## Constants & data dependencies

Listed in `world/objects.md` (Constants & data dependencies).

## Randomness

`world/objects.md` Randomness items 9–13.

## Edge cases & original bugs

`world/objects.md` edge cases 13–21. Added here:

1. **ENDANIM writes mode 2 directly** (§18.6): clients get no mode
   update from it, and the footprint is freed only on that branch.

## Test vectors

`world/objects.md` Test vectors (rows for §16–§18).

| Input | Expected | Source |
|---|---|---|
| event 1, O mode 1, `Mode2` 1, `HasCollision2` 0 | mode field 2, footprint freed, no flag 0x1 | §18.6 |
| event 1, O mode 2 | nothing | §18.6 |

## Provenance

The §16–§18 bullet of `world/objects.md` Provenance; §18.6 from
`0x00581490` (1.14d disassembly).

## Open questions

Kept in `world/objects.md` (9–13).
