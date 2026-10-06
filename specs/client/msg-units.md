# Spec: Client — Unit messages (add, remove, place, move, vitals)

- **Status:** draft: handlers read in the 1.14d `Game.exe` (addresses
  below); field values checked against the two single-player recordings
  `traces/raw/20261006-015956-packets.jsonl` and `-022633-packets.jsonl`;
  unverified: no executable check runs it yet.
- **Target version:** 1.14d
- **Crate/module:** `d2-client::bridge` handlers for the ids below, over
  the model of `client/model.md`.
- **Related specs:** `client/model.md` (unit table §2, local player §3,
  queue §4, update pass §5, position check §6, mode requests §8, bit
  reader §10); `sim/server-messages.tsv` (sizes, server layouts);
  `monsters/init.md` §24 (what the server puts in 0xAC);
  `sim/pathing.md` §10 (when the server sends 0x0F, 0x10, 0x15);
  `sim/path-placement.md` (paths, teleport); `sim/stat-lists.md` §5
  (stat set / add).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 42–54 |
| Inputs | 55–61 |
| Outputs / state changes | 62–66 |
| Rules | 67–68 |
|   1. Unit add | 69–224 |
|   2. 0x0A RemoveUnit (`0x0045CC10`) | 225–234 |
|   3. 0x15 ReassignPlayer (`0x0045D160`) | 235–271 |
|   4. Queued movement and action messages | 272–306 |
|   5. Local player vitals: 0x18, 0x95, 0x96 | 307–328 |
| Constants & data dependencies | 329–340 |
| Randomness | 341–346 |
| Edge cases & original bugs | 347–361 |
| Test vectors | 362–395 |
| Provenance | 396–421 |
| Open questions | 422–440 |
<!-- /index -->

Owned ids: 0x0A, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x15, 0x18, 0x4C, 0x4D,
0x51, 0x59, 0x67, 0x68, 0x69, 0x6A, 0x6B, 0x6C, 0x6D, 0x6E, 0x6F, 0x70,
0x71, 0x72, 0x95, 0x96, 0xAC.

## Summary

These messages create, remove and place client units and feed them mode
requests and position checks. Creation (0x59 players, 0xAC monsters,
0x51 objects), removal (0x0A), re-placement (0x15) and the local
player's vitals with a position check (0x18, 0x95, 0x96) act when
received. The movement and action messages (0x0C–0x10, 0x4C, 0x4D,
0x67–0x72) have unit handlers: they are queued on the unit and act in
the update pass (`client/model.md` §4–§5), each as an optional position
check followed by one mode request. In single player the server sends
no 0x0F / 0x10 for the local player (`sim/pathing.md` §10 rule 2); the
local player gets 0x15, 0x0D, 0x18, 0x95, 0x96.

## Inputs

| Name | Type | Source |
|---|---|---|
| message | id + bytes | `client/model.md` §4 |
| tables | `monstats` (row count, `MonStatsEx` link), `monstats2` (component choice counts at +0x15 + i, §1.2 rule 7; path byte +0x0E), `itemstatcost` (send bits, send param bits, signed flag) | `data/` (live tables) |

## Outputs / state changes

Client units added, removed, placed; their stats, seed, kind data; mode
requests (`client/model.md` §8); C→S 0x5F from the position check.

## Rules

### 1. Unit add

#### 1.1 0x59 AssignPlayer (`0x0045E4C0` → `0x00466200`)

1. Layout: GUID u32@1, class u8@5, name 16 bytes @6 (zero-padded),
   x u16@0x16, y u16@0x18.
2. Allocate a type-0 unit; common fields and seed by
   `client/model.md` §2 rule 6 (at (0, 0): no room, seed {1, 666}).
3. Player init (`0x00460BF0(unit, room, x, y)`): dynamic path at (x, y)
   (`0x00649D00`, `sim/path-placement.md` §2.4); an inventory if none
   (`0x0063ABD0`); a skill list (`0x006438B0`, +0xA8); stats 68, 67, 69
   := 100 (set, layer 0); `[0x007A5260]` := `0x0061AB00(room)`; mode :=
   5 (`0x00624690`); unit flags := (flags & ~0x20) | 0xE; light and
   overlay set-up (`0x00474160`, Phase 6); then, unless the unit is
   already the local player, one seed step (`client/model.md`
   Randomness rule 2).
4. Gfx (`0x004DC510`), stat list (`0x00623F50`), add (`client/model.md`
   §2 rule 4), name := the 16 bytes (`0x006220F0`).
5. Model: `class`, `position` = (x, y) or none at (0, 0), `mode` 5,
   stats {67: 100, 68: 100, 69: 100}, `name`.

#### 1.2 0xAC AssignMonster (`0x0045F190`)

1. Layout: GUID u32@1, class u16@5, x u16@7, y u16@9, life u8@0xB,
   size u8@0xC (the message size), bit stream @0xD of size − 0xD bytes
   (`client/model.md` §10), fields in order:

   | Field | Bits | Condition |
   |---|---|---|
   | mode | 4 | always |
   | has components | 1 | always |
   | component i (i = 0..15) | `monstats2` choice count c of component i: c < 3 → 1 bit; else bit length of (c − 1) (0 when c = 1) | has components, and the class has a `monstats` / `monstats2` row (else the 16 reads are skipped) |
   | has type flags | 1 | always |
   | champion 4, unique 8, superunique 2, minion 0x10, ghostly 0x40 | 1 each, in this order | has type flags |
   | hcIdx | 16 (signed read, masked to 16 bits) | flags & 2 (superunique), else 0 |
   | umods | 8 each until a 0 byte | has type flags (written into a zeroed 9-byte buffer; the loop has no bound) |
   | name seed | 16 | has type flags, else 0 |
   | has value | 1 | has type flags |
   | value | 32 | has value, else −1 |

   The server side of the same fields: `monsters/init.md` §24.
2. Create (`0x00466360`): if GUID = the local player's hireling GUID
   (`0x00478F20(local player, 7)`) and that monster is in set S, it is
   re-initialised instead of created (`0x0046EC10`, gfx, mode := the
   4-bit mode). Otherwise the class must be a `monstats` row with a
   `monstats2` row, else nothing is created and the message has no
   effect. Allocate type 1; common fields (`client/model.md` §2 rule 6;
   a monster's seed is `init_low(+0x28)` also when there is no room:
   {0, 666}); type data: name seed, type flags, hcIdx (when superunique),
   the umods (9 bytes), the components (16 bytes), the value;
   `0x004AE8D0(unit, room, x, y, class, mode)` (stats from the tables,
   path, a mode set through `0x00624690` whose argument is not traced;
   open question 2); path byte from `monstats2`
   +0x0E (`0x00649070`); gfx; add.
3. After creation (none → stop): `0x00470B70(unit)`. Unless the unit is
   a hireling class (`0x0063EE90`) whose GUID is the local player's
   hireling GUID: stat 7 := 0x8000, stat 6 := life << 8 (set). A
   hireling class with mode 1: flag +0xC8 bit 0x40000 cleared. Stat 328
   (0x148) := (x + y) & 0xFFFF.
4. Then from the same stream: 1 bit; set → 31 bits v, `0x00621CC0(unit,
   0, v)`: the source-unit link (`skills/bodies.md` §6.20, `0x00621C30`)
   with owner type 0 (player) and GUID v: unit `+0x94` := 0, `+0x98` :=
   v, state 98 `sourceunit` with stats 353 := 0, 354 := v when the unit
   has a stat holder, and flag-ex (`+0xC8`) |= 0x400 (the linked-unit
   bit read by `0x004639D0`, `render/unit-composite.md` §1.1). 1 bit; set → a stat list: 9-bit stat id s
   until s ≥ 0x1FF, s < 0, s ≥ `itemstatcost` row count, or the row's
   send bits are 0 (each ends the list); param := `send param bits`
   bits (0 when 0); value := `send bits` bits, signed when send bits <
   32 and the row's signed flag (row +4 & `[0x006CE26C]`); the first
   stat gets the unit a stat list with flag 0x40 (an existing one is
   reused, `0x006257D0`; else `0x006251F0` + attach `0x00626E10`); set
   (s, value, param) on that list (`0x00627150`).
5. Model: `class`, `position` (x, y), `mode` (the 4-bit mode, rule 6),
   stats {6:
   life << 8, 7: 0x8000, 328: (x + y) & 0xFFFF} plus the table stats of
   rule 6, kind data {name seed, flags, hcIdx, umods, components, value,
   v31}, the 0x40 stat list.
6. Monster set-up `0x004AE8D0(unit, room, x, y, class, mode)` (`d` =
   difficulty `[0x007A060C]`), in order:
   1. A new stat list (`0x00626D40`); base stats (`0x00627260`): 12
      `level` := `monstats` Level[`d`] (`+0xAA + 2d`); 68 `attackrate`
      := 100; 67 `velocitypercent` := 75; 69 `other_animrate` := 100;
      36, 37, 39, 41, 43, 45 (damage, magic, fire, light, cold, poison
      resist) := `ResDm`, `ResMa`, `ResFi`, `ResLi`, `ResCo`, `ResPo`
      [`d`] (`+0x144`, `+0x14A`, `+0x150`, `+0x156`, `+0x15C`, `+0x162`
      + 2d); classic scaling `0x0063EEF0(unit, expansion, d)`
      (`monsters/init.md` §13); 7 `maxhp` := 0x6400; 6 `hitpoints` :=
      0x6400 (rule 3 later overwrites 6 and 7).
   2. Path: `0x00649D00(…, room, x, y, unit, 1)` places the dynamic path;
      velocity := `monstats` `Velocity` (`+0x32`) << 8 (`0x00648690`).
   3. Inventory (`0x0063ABD0`) when the unit has none and `monstats`
      `interact` is set or the class is 291 `irongolem`, 357
      `valkyrie`, 417 `deathsentry` or 418 `shadowwarrior`.
   4. Mode := the `mode` argument (`0x00624690`: the 4-bit mode of rule
      1's stream). When `0x0063EA40(unit)` holds and `monstats2` `deadCol`
      (byte `+0x06` bit 3) is clear: path settings `0x00649560(unit, 1)`,
      `0x00649190(unit, 5)`, `0x00648C30(path, 0x8000)`.
   5. Current frame `+0x44` := rnd(`+0x48`) from the unit seed `+0x20`
      (`0x0045C3E0`, `sim/rng.md` range rule).
   6. Unit flags (`+0xC4`): 0x2 := `monstats2` `isSel` (byte `+4`
      bit 3); 0x20 := not `shadow` (byte `+5` bit 6); 0x8 set; 0x4 :=
      `isAtt` (byte `+5` bit 1).
   7. `0x004AE0A0(unit, 0)`, `0x004AE4F0(unit, class)`, light
      (`0x004AE210`, `render/lighting.md` §8), `+0xA8` :=
      `0x006438B0(0)`, umod hooks (`0x004AD020`, `render/lighting.md` §8
      r2).
   8. Skills: for `i` = 0…7, `monstats` Skill`i` (`+0x170 + 2i`) ≥ 0
      with level byte (`+0x198 + i`) > 0: add the skill at level byte +
      the act level bonus (`0x00611D30(d)` `+0x10`) (`0x00647280`), then
      `0x00644340(0x006439F0(unit, skill), mode byte +0x180 + i)`. Then
      `0x0063EBC0(unit, bonus)`.
   9. Unless `monstats` byte `+0x0D` bit 0 (`npc`): initial direction
      byte `b` := 0, or `low 6 bits` of one RNG step of the unit seed
      when `0x0046C140(unit, 2)`; base class 96 (`mummy1` family) → 7;
      base 301 (`vilechild` family) → `0x0046C570(unit, 0, 0x004AE7B0)`;
      class 351 → 0x18, 353 → 0x28, 352, 357, 344 → 0; then
      `0x006488A0(path, b)`.
7. **Table inputs of rule 1** (read at `0x0045F1F1`–`0x0045F293`): the
   class must satisfy 0 ≤ class < `monstats` row count (`[0x00744304]`
   +0xA80; rows 0x1A8 bytes at +0xA78); that row's `MonStatsEx` link
   (s16 at row +0x18, the `monstats2` row index) must satisfy 0 ≤ link
   < `monstats2` row count (+0xA98; rows 0x134 bytes at +0xA90). Then
   for i = 0…15 the choice count c is the u8 at `monstats2` row
   +0x15 + i, i.e. byte 21 + i written by the `HDv`, `TRv`, `LGv`,
   `Rav`, `Lav`, `RHv`, `LHv`, `SHv`, `S1v`…`S8v` columns
   (`data/callbacks.md` §5; `data/fields.tsv` `monstats2` fields 10–25,
   `cb(monstats2.composit)`). Bits read for component i: c < 3 → 1;
   else bit length of c − 1 (`bsr` + 1; c = 1 is covered by c < 3).
   Either check failing skips the 16 reads (rule 1) and the creation
   (rule 2). d2rs: the loader fills `ModelInputs::tables` from
   `d2-data`'s `monstats` rows (`MonStatsEx`) and `monstats2` count
   bytes; no other column is needed by 0xAC's stream.
8. The hireling GUID of rule 2 and rule 3: `client/model.md` §14 rule 4.

#### 1.3 0x51 AssignObject (`0x0045CBD0` → `0x00466300`)

1. Layout: type u8@1, GUID u32@2, class u16@6, x u16@8, y u16@0xA, mode
   u8@0xC, interact u8@0xD.
2. Create with the type of byte 1 (`0x00465FD0`): common fields
   (`client/model.md` §2 rule 6). Type 2: object init `0x004BC720(unit,
   room, x, y, class, mode)` (mode := the mode byte; static path at
   (x, y)). Type 1 is a fatal assert 0x202. Types 0, 3, 4, 5 take their
   kind's init (player `0x00460BF0`, missile `0x004C1910`, item
   `0x004CD0A0`, tile: unit flags |= 0x22, static path). Add; an object
   then gets `0x004BC8D0`.
3. Object data +4 := interact. If `0x00621B00(unit)` → `0x004BD6B0`.
4. Model: `class`, `position` (x, y), `mode` (mode byte), kind data
   {interact}.
5. **Types 0, 3, 4, 5 are never sent.** The only 1.14d builder of 0x51,
   `0x0053BD10`, has one caller (`0x00572067` in the add messages,
   `sim/intents-events.md` §7.2), which passes type 2; all 206 recorded
   0x51 (both recordings) have type 2. d2rs: a 0x51 with type 0, 3, 4
   or 5 is a handler error (refused and recorded, `client/bridge.md`
   §2.4), like type 1; the kind inits of rule 2 are not modelled. This
   is not an exact-match risk: no 1.14d server input reaches them.

### 2. 0x0A RemoveUnit (`0x0045CC10`)

1. Layout: type u8@1, GUID u32@2.
2. Type 1 with GUID = the local player's hireling GUID
   (`0x00478F20(local player, 7)`) → nothing. Else remove (`client/model.md`
   §2 rule 5).
3. The hireling GUID here and in §1.2 rule 2: `client/model.md` §14
   rule 4 (pet list from 0x7A / 0x81; −1 when there is none, so with no
   hireling 0x0A always removes and 0xAC always creates).

### 3. 0x15 ReassignPlayer (`0x0045D160`)

1. Layout: type u8@1, GUID u32@2, x u16@6, y u16@8, flag u8@0xA (1 when
   the server set it for unit flags 2 bit 0x10000, `sim/pathing.md` §10
   rule 3).
2. Unit not in S → nothing. Else place (`0x004654C0`, rule 4). The unit
   must then have a room (fatal 0x538); act := the room's act. A player
   gets `0x00649CA0(unit)` and move-test mask 0x1C09 (`0x00648CE0`); a
   monster for which `0x0063EA40` is 0 gets footprint mask 0x100
   (`0x00648C30`, `sim/path-placement.md` §5.3 rule 1).
3. Local player: `0x0061AB30(room)` ≠ 0 → fatal 0x81E; then
   `0x0044DB40(3000)` (a wall-clock hold, not modelled).
4. **Place** (`0x004654C0(unit, x, y, flag)`):
   1. Local player: `0x00466FE0` (clears two input globals).
   2. room' := room of (x, y) (`client/model.md` §2 rule 7). No path →
      fatal 0x166; (x, y) ≠ (0, 0) and no room' → fatal 0x168.
   3. Dead unit (`client/model.md` §6 rule 2) → stop.
   4. Local player: with an old room: if `0x00620250(unit)` and
      `0x00644140` of it have the bit of `[0x006CE280]` → flag := 0;
      else a non-zero flag runs `0x004DBFE0` (a player) or is a fatal
      assert 0x17D (other kinds). With room': client room change
      `0x0061ACD0(room', x, y, room' ≠ old)`, town flag to
      `0x004F5190`, and when the area byte (+2 of `0x0061DB70`) of the
      two levels differs, `0x004FB480` and `0x00600C00(area)` (Phase 6:
      automap, music).
   5. Teleport to (x, y) (`0x00650BE0` → `sim/path-placement.md` §6
      rule 4); if that returns 0: nearest free point (`0x0064E7B0`,
      unit size, mask 0x1C09, fallback 1) and forced placement
      (`0x00650C20`), whose failure is fatal 0x1A9.
   6. `0x00459140`; local player: `0x00472C20(flag)`; `0x00463B80`.
5. Model: `position` := (x, y) (or the free point of rule 4.5).
6. For the local player this placement is how the client learns its
   level: the room of rule 4.2 and its level (`client/model.md` §11
   rule 3); at a join 0x15 is the first message that gives the player
   a room. Room of a point, the fatal asserts 0x168 / 0x538 / 0x1A9 and
   the free-point fallback in d2rs: `client/model.md` §12.

### 4. Queued movement and action messages

1. Each row is a unit handler (queued, `client/model.md` §4): optional
   position check `check(U, x, y, 0, 0, 0)` (`client/model.md` §6), then
   the mode request (code, record) (`client/model.md` §8). "–" = 1.14d
   leaves the entry unset (model: 0). Offsets are message offsets.

   | Id | Handler | Check (x, y) | Code | record[0..6] |
   |---|---|---|---|---|
   | 0x0C | `0x0045CC70` | – | u8@6 | u8@7, u8@8, 0, 0, 0, 0, 0 |
   | 0x0D | `0x0045CCC0` | – | u8@6 | u16@7, u16@9, u8@0xB, –, –, –, – |
   | 0x0E | `0x0045CD10` | – | u8@6 | u8@7, u32@8, –, –, –, –, – |
   | 0x0F | `0x0045CD40` | u16@0xC, u16@0xE | u8@6 | u16@7, u16@9, u8@0xB, –, –, –, – |
   | 0x10 | `0x0045CD90` | u16@0xC, u16@0xE | u8@6 | u8@7, u32@8, –, –, –, –, – |
   | 0x4C | `0x0045DF00` | – | 0x16 | u16@6, −1, u8@9, u32@0xA, u8@8, 0, 0 |
   | 0x4D | `0x0045DF60` | – | 0x15 | u32@6, −1, u16@0xB, u16@0xD, u8@0xA, 0, 0 |
   | 0x67 | `0x0045CDE0` | – | u8@5 | u16@6, u16@8, u8@0xA, u8@0xC, i16@0xD, u8@0xF, u8@0xB |
   | 0x68 | `0x0045CE30` | u16@6, u16@8 | u8@5 | u8@0xA, u32@0xB, u8@0xF, u8@0x11, i16@0x12, u8@0x14, u8@0x10 |
   | 0x69 | `0x0045CEA0` | – | u8@5 | u16@6, u16@8, u8@0xA, 2, 0, 4, u8@0xB |
   | 0x6A | `0x0045CEF0` | – | u8@5 | u8@6, u32@7, u8@0xB, 2, 0, 4, – |
   | 0x6B | `0x0045CF40` | u16@0xC, u16@0xE | u8@5 | u16@6, u16@8, u8@0xA, 2, 0, 4, u8@0xB |
   | 0x6C | `0x0045CFB0` | u16@0xC, u16@0xE | u8@5 | u8@6, u32@7, u8@0xB, 2, 0, 4, – |
   | 0x6D | `0x0045D010` | u16@5, u16@7 | 7 | u16@5, u16@7, u8@9, 2, 0, 4, 0 |

   Unsigned fields are zero-extended, `i16` sign-extended. 0x4C / 0x4D
   check that the unit is not null (always true for a drained message).
2. 0x0D also: if type u8@1 = 0, `0x0047A690(GUID u32@2, u8@0xC)`: the
   party roster entry of that GUID (if any) gets life percent := u8@0xC
   (roster +0x14; the roster belongs to the owner of 0x5B, open
   question 4).
3. 0x6D also, between the check and the request: stat 328 (0x148) :=
   base(328) + 1 (`0x006253B0`, `0x00627260`).
4. 0x6E, 0x6F, 0x70, 0x71, 0x72: no effect (`client/model.md` §4 rule
   4).

### 5. Local player vitals: 0x18, 0x95, 0x96

1. General handlers (act at receive); they decode the message with the
   bit reader (`client/model.md` §10) from bit 0 of byte 0 and need a
   local player (none → nothing).

   | Id | Handler | Fields (bits, in order) |
   |---|---|---|
   | 0x18 | `0x0045D9B0` | id 8, life 15, mana 15, stamina 15, a 7, b 7, x 16, y 16, dx 8, dy 8 |
   | 0x95 | `0x0045DB20` | id 8, life 15, mana 15, stamina 15, x 16, y 16, dx 8, dy 8 |
   | 0x96 | `0x0045DC50` | id 8, stamina 15, x 16, y 16, dx 8, dy 8 |

2. Stats (set, layer 0): stat 6 := life << 8, stat 8 := mana << 8,
   stat 10 := stamina << 8 (0x18 and 0x95; 0x96: stat 10 only); 0x18
   also stat 74 := a, stat 26 := b.
3. Then `check(local player, x, y, 0, tx, ty)` (`client/model.md` §6)
   with tx := (x + sdx) & 0xFFFF, ty := (y + sdy) & 0xFFFF, where sdx =
   dx − 0x100 if dx > 0x80 else dx (so 0x80 is +128), the same for dy.
4. 0x18 and 0x95 only: if life ≠ 0 and the local player's mode is 0x11
   (dead): `0x00480E70` and `0x004647D0` (leave the dead mode; Phase 6
   player modes, `client/model.md` open question 1).

## Constants & data dependencies

| Item | Value | Use |
|---|---|---|
| player creation stats | 67, 68, 69 := 100 | §1.1 rule 3 |
| player creation mode | 5 | §1.1 rule 3 |
| monster life stats | 6 := life << 8, 7 := 0x8000 | §1.2 rule 3 |
| position stat | 328 = (x + y) & 0xFFFF; 0x6D adds 1 | §1.2, §4 rule 3 |
| reassign masks | move-test 0x1C09 (players), footprint 0x100 (monsters) | §3 rule 2 |
| stat-list end | 9-bit id 0x1FF | §1.2 rule 4 |
| `itemstatcost` columns | send bits (+8), send param bits (+9), signed flag (+4 & `[0x006CE26C]`) | §1.2 rule 4 |

## Randomness

Seeds only (`client/model.md` Randomness): 0x59 and 0xAC creation at a
point step the room seed once; 0x59 steps the new player's seed once
unless it is already the local player. No other draw.

## Edge cases & original bugs

- 0x0D, 0x0E, 0x0F, 0x10, 0x6A, 0x6C pass records with unset entries
  (stack contents in 1.14d).
- 0xAC's umod loop is unbounded; more than 8 umods before the 0 byte
  would overrun the 9-byte buffer (the server's limit: `monsters/init.md`
  §24).
- 0xAC for an unknown class creates nothing; its queued follow-ups
  (0x6D etc.) are dropped (no unit).
- 0x0A never removes the local player's hireling (it stays when it
  leaves the player's rooms).
- The sign rule of §5 rule 3 maps dx = 0x80 to +128, not −128.
- 0x96 for a dead local player: stamina is set, the check does nothing
  (`client/model.md` §6 rule 2).

## Test vectors

From `traces/raw/20261006-022633-packets.jsonl` ("B") and
`-015956-packets.jsonl` ("A"), seq numbers; synthetic where marked.

| Input | Expected | Source |
|---|---|---|
| `59 01 00 00 00 01 77 65 72 77 65 72 00 …` (x = y = 0) | unit (0, 1), class 1, name "werwer", position none, mode 5, stats {67, 68, 69: 100}, seed {0x6AC6935F, 0} (no local player yet) | B 102 |
| `15 00 01 00 00 00 41 12 c4 11 01` | (0, 1) position (0x1241, 0x11C4), flag 1 | B 154 |
| `ac 06 00 00 00 9a 00 1a 12 b9 11 80 0e 01` | unit (1, 6), class 154, position (0x121A, 0x11B9), mode 1, no components, no flags; stats {6: 0x8000, 7: 0x8000, 328: 0x23D3} | B 157 |
| `6d 06 00 00 00 1a 12 b9 11 80` (queued on (1, 6)) | check at (0x121A, 0x11B9): within T; stat 328 += 1 → 0x23D4; request code 7, record (0x121A, 0x11B9, 0x80, 2, 0, 4, 0) | B 159 |
| `51 02 0d 00 00 00 25 00 14 12 c0 11 02 00` | unit (2, 13), class 37, position (0x1214, 0x11C0), mode 2, interact 0 | B 160 |
| `0e 02 0d 00 00 00 03 00 02 00 00 00` | (2, 13) queued; request code 3, record (0, 2, 0, …) | B 161 |
| `0a 02 0c 00 00 00` | (2, 12) removed | A 7228 |
| `0a 01 19 00 00 00` | (1, 0x19) removed | A 215606 |
| `67 13 00 00 00 01 95 12 55 15 01 00 0d 4b 00 05` | (1, 0x13): request code 1, record (0x1295, 0x1555, 1, 0x0D, 0x004B, 5, 0) | A 213893 |
| `69 13 00 00 00 06 94 12 55 15 2f 06` | request code 6, record (0x1294, 0x1555, 0x2F, 2, 0, 4, 6) | A 218973 |
| `6c 13 00 00 00 10 00 01 00 00 00 00 94 12 55 15` | check (0x1294, 0x1555); request code 0x10, record (0, 1, 0, 2, 0, 4, 0) | A 221956 |
| `0c 01 13 00 00 00 13 06 0f` | request code 0x13, record (6, 0x0F, 0, 0, 0, 0, 0) | A 220507 |
| `0d 00 01 00 00 00 13 e4 12 8e 13 11 57` | local player: request code 0x13, record (0x12E4, 0x138E, 0x11, …); roster life 0x57 | B 25465 |
| `96 4a 80 8e 89 c0 09 00 00` | stamina 0x4A, x 0x131D, y 0x1381, dx 0, dy 0: stat 10 := 0x4A00; check (0x131D, 0x1381, 0, tx 0x131D, ty 0x1381) | B 11145 |
| `96 48 00 8c 89 c3 89 01 7e` | stat 10 := 0x4800; check (0x1318, 0x1387) with target (0x131B, 0x1383) (dy 0xFC → −4) | B 15933 |
| `95 24 00 0c 80 12 00 00 00 00 00 00 00` | stats 6 := 0x2400, 8 := 0x1800, 10 := 0x4A00; x = 0: no check | B 140 |
| `95 1c 80 0a 80 12 a0 5b 22 72 c2 bf 00` | life 0x1C, mana 0x15, stamina 0x4A, check (0x12DD, 0x1391) target (0x12DB, 0x1396) | B 32694 |
| `18 24 80 0c 80 12 80 0c e8 98 08 9c 00 00 00` | stats 6 := 0x2400, 8 := 0x1900, 10 := 0x4A00, 74 := 0x64, 26 := 0; check (0x131D, 0x1381) | B 12323 |
| 0x6D for (1, 7) not in S | dropped at receive | synthetic |
| 0x15 for a dead monster | position unchanged | synthetic, §3 rule 4.3 |
| 0x0A type 1, GUID = local player's hireling | nothing | synthetic |
| 0x96 with dx 0x80 | tx = x + 128 | synthetic |
| 0xAC component bits for counts 7, 3, 3, 3, 3, 10, 0, 5, 12, 12, 0 × 6 (`skeleton1`, `data/callbacks.md` §5 example) | 3, 2, 2, 2, 2, 4, 1, 3, 4, 4, 1 × 6 = 33 bits | §1.2 rule 7 |
| 0xAC with class ≥ `monstats` row count, or a row whose `MonStatsEx` is −1 | no component reads, no unit | synthetic, §1.2 rule 7 |
| 0x51 with type 4 | handler error | synthetic, §1.3 rule 5 |
| 0x0A type 1, GUID 0x21, pet list empty | (1, 0x21) removed (hireling GUID −1) | synthetic, §2 rule 3 |

## Provenance

1.14d `Game.exe` handlers (receive table `0x007114D0`): 0x0A
`0x0045CC10`; 0x0C `0x0045CC70`; 0x0D `0x0045CCC0`; 0x0E `0x0045CD10`;
0x0F `0x0045CD40`; 0x10 `0x0045CD90`; 0x15 `0x0045D160`; 0x18
`0x0045D9B0`; 0x4C `0x0045DF00`; 0x4D `0x0045DF60`; 0x51 `0x0045CBD0`;
0x59 `0x0045E4C0`; 0x67–0x6D `0x0045CDE0`–`0x0045D010`; 0x6E–0x72
`0x0045D0A0`–`0x0045D0E0`; 0x95 `0x0045DB20`; 0x96 `0x0045DC50`; 0xAC
`0x0045F190`. Helpers: `0x00466200`, `0x00460BF0`, `0x00466300`,
`0x00465FD0`, `0x004BC720`, `0x00466360`, `0x004654C0`, `0x00650BE0`,
`0x00650C20`, `0x0047A690`, `0x00478F20`. Register arguments read from
the disassembly (`tools/ghidra/disasm.py`); the decompiler drops them.
Field values decoded from the recordings with the §5 and §1.2 bit
layouts (every decoded position lies next to the player's recorded
positions; 0x96 stamina tracks walking). The server-side meaning of the
`code` bytes belongs to the sim specs (`sim/pathing.md` §10 rule 2, the
monster update spec).
Ghidra backlog (2026-10-06): monster set-up `0x004AE8D0` (field offsets
from `specs/data/fields.tsv`, stat names from `itemstatcost`); source
link `0x00621CC0` → `0x00621C30`.
Join-update session (2026-10-06): 0xAC table reads `0x0045F1F1`–
`0x0045F293` (row sizes 0x1A8 / 0x134 from the `imul`s); 0x51 builder
`0x0053BD10` and its single caller `0x00572067` (type pushed as 2),
type bytes counted in both recordings (206 × 2). The code bytes'
server meaning: `sim/intents-events.md` §7.4.

## Open questions

1. Meaning of each mode-request code per unit kind: Phase 6 unit-modes
   spec (`client/model.md` open question 1).
2. ~~Monster creation's table stats and placement~~: answered in §1.2
   rule 6. Open inside it: `0x004AE0A0`, `0x004AE4F0`, `0x0046C140`,
   `0x0046C570` (client monster AI / animation set-up, Phase 6).
3. ~~`0x00621CC0(unit, 0, v)`~~: answered in §1.2 rule 4 (source-unit
   link to player GUID v).
4. The party roster (0x5B, `0x0047A6F0`) that 0x0D's life percent
   updates: owner spec of 0x5B.
5. `0x0063EA40` (0x15 rule 2) and `0x00621B00` / `0x004BD6B0` (0x51
   rule 3): what they test.
6. 0x16 UnitPositions (`0x0045D2E0`, also a position check) and 0x17:
   not seen in the single-player recordings; left TBD.
7. A recording with a hireling (0x7A / 0x81, 0xAC of the hireling)
   confirms §1.2 rules 2–3 and §2 rule 2 with a real pet list
   (`client/model.md` open question 10).
