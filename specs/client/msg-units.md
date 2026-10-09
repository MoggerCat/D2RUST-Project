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
| Summary | 47–59 |
| Inputs | 60–66 |
| Outputs / state changes | 67–74 |
| Rules | 75–76 |
|   1. Unit add | 77–260 |
|   2. 0x0A RemoveUnit (`0x0045CC10`) | 261–270 |
|   3. 0x15 ReassignPlayer (`0x0045D160`) | 271–313 |
|   4. Queued movement and action messages | 314–401 |
|   5. Local player vitals: 0x18, 0x95, 0x96 | 402–432 |
|   6. Unit states: 0xA7, 0xA8, 0xA9, 0xAA | 433–463 |
|   7. Other unit messages (general handlers, act at receive) | 464–599 |
|   8. Player roster (0x5B, 0x5C, 0x65, 0x75, 0x82, 0x8E; life from 0x0D, 0xAB) | 600–750 |
| Constants & data dependencies | 751–762 |
| Randomness | 763–770 |
| Edge cases & original bugs | 771–793 |
| Test vectors | 794–844 |
| Provenance | 845–896 |
| Open questions | 897–942 |
<!-- /index -->

Owned ids: 0x0A, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x15, 0x18, 0x4C, 0x4D,
0x51, 0x59, 0x67, 0x68, 0x69, 0x6A, 0x6B, 0x6C, 0x6D, 0x6E, 0x6F, 0x70,
0x71, 0x72, 0x95, 0x96, 0xA7, 0xA8, 0xA9, 0xAA, 0xAC; §7–§8: 0x09,
0x11, 0x57, 0x5B, 0x5C, 0x5F, 0x60, 0x65, 0x73, 0x74, 0x7E, 0x82, 0x8E,
0x98, 0xA4, 0xAB.

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
The player roster (§8). Outputs (`client/bridge.md` §10): `UnitOverlay`
(0x11), `UmodFx` (0x57), `ClientMissile` (0x73), `CommonCof` (0x7E),
`MonsterPreload` (0xA4), `RosterChanged` (0x5B, 0x5C, 0x65).

## Rules

### 1. Unit add

#### 1.1 0x59 AssignPlayer (`0x0045E4C0` → `0x00466200`)

1. Layout: GUID u32@1, class u8@5, name 16 bytes @6 (zero-padded),
   x u16@0x16, y u16@0x18.
2. Allocate a type-0 unit; common fields and seed by
   `client/model.md` §2 rule 6 (at (0, 0): no room, seed {1, 666}).
3. Player init (`0x00460BF0(unit, room, x, y)`): dynamic path at (x, y)
   (`0x00649D00`, `sim/path-placement.md` §2.4); an inventory if none
   (`0x0063ABD0`); a skill list (`0x006438B0`, +0xA8) and at once its
   native skills (`0x00647EE0`, call `0x00460C31`; `client/msg-skills.md`
   §2 rule 8); stats 68, 67, 69
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
   1. **Hireling re-init in detail** (2026-10-08; answers
      `docs/handoff/impl-client-staging.md` Q1). The test is the 0xAC
      GUID = `0x00478F20(local player, 7)` and a monster (1, GUID) in S
      (`0x00463940`). Then, in order: `0x0046EC10(U)` frees U's
      graphics record (+0x54: its component chain +0x28 and its overlay
      chain +0x2C, each overlay's light removed, `0x004743D0`; none →
      fatal 0x814), `0x004DC510(U)` rebuilds the graphics (both Phase 6
      render), **U's mode := the 4-bit mode** (`0x00624690`); the
      existing U is returned. Nothing else of the message (class,
      position, type data, components, seed) is written. The caller
      `0x0045F190` then runs rules 3 and 4 on U as for a new monster.
      **Hireling classes** (`0x0063EE90`, monsters only, class →
      kind): 271 → 1, 338 → 2, 359 → 3, 560 and 561 → 4, others 0.
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
   kind's init (player `0x00460BF0`, missile `0x004CD0A0`, item
   `0x004C1910`, tile: unit flags |= 0x22, static path; jump table
   `0x004661A0` read from the file). Add; an object
   then gets `0x004BC8D0`.
3. Object data +4 := interact. If `0x00621B00(unit)` → `0x004BD6B0`:
   object data +8 := the shrines record of index interact (`0x006414B0`:
   table `[0x0096D468]`, 0xB8-byte rows, count `[0x0096D46C]`; out of
   range → fatal 0x15F / 0x160, handler error), then `0x004BD650`: the
   record's code byte +0 (≥ `[0x0072779C]` → fatal 0x37B) selects an
   entry of the shrine table `0x006DA8C0` (0x14-byte entries) whose
   function +0, when non-null, is called (ECX unit, EDX record): the
   on-mode function of `client/model.md` §15 rules 2–3 (`0x004BD4A0`,
   codes 6–15). Model: kind data {interact, shrine record index}. The
   call is one `ShrineFx` output {on-mode, code, object key, no player,
   the code's two overlay ids} (`client/bridge.md` §10), emitted at
   receive (0x51 is a general handler), only when the code's +0x00
   function is set.
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
   monster for which `0x0063EA40` is 0 (`0x0063EA40(U)`: U is a monster
   in mode 0 or 12, i.e. dying or dead) gets footprint mask 0x100
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
      `0x004F5190`, and when the Levels `Pal` byte of the two levels
      differs (record +0x02 of `0x0061DB70`, 544-byte `levels` record:
      `data/fields.tsv` `levels` `Pal` offset 2; `Act` is offset 3 and
      is not read here), `0x004FB480(Pal of the new level)` (CL, loaded
      at `0x00465622`) and `0x00600C00(Pal)` (Phase 6: automap, music).
      `Pal` ≠ `Act` in 7 of 137 levels.txt rows (125–127 Pal 3, 133 Pal
      0, 134 Pal 1, 135 Pal 3, 136 Pal 0; all Act 4): walking from
      Harrogath (109, Pal 4) into level 133 loads act 1's palette.
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
   (roster +0x14, §8 rule 6).
3. 0x6D also, between the check and the request: stat 328 (0x148) :=
   base(328) + 1 (`0x006253B0`, `0x00627260`).
4. 0x6E, 0x6F, 0x70, 0x71, 0x72: no effect (`client/model.md` §4 rule
   4).
5. Objects (type 2; `client/model.md` §15): 0x0E's code u8@6 is the
   client mode-request code 3 (object mode change; the server writes 3
   for objects, `world/objects.md` §14 rule 1). 0x4D's code 0x15 handler
   `0x004BD5C0` reads only record[0] (u32@6: in the shrine form the
   operator's GUID, looked up as a player); the shrine code comes from
   the client's own object (shrine record `Code`, else objects.txt
   `ShrineFunction`), not from the message. record[2..4] (u16@0xB,
   u16@0xD, u8@0xA) are never read, and u16@0xF is not copied into the
   record; for objects these bytes carry no client effect (`Code` @10
   and the zeros @11, @13, @15 of `world/objects.md` §14).
6. **Dead flag** (unit flag +0xC4 bit 0x10000; 2026-10-08 read of the
   nine callers of the setter `0x00464810`; answers `client/msg-ui.md`
   OQ 2 for this bit). The dead test `0x00464820(U)` is: flag 0x10000
   set, or player mode 0 / 0x11, or monster mode 0 / 0xC
   (`client/model.md` §6 r2). Writers on players and monsters (the
   other six set it on client-made missiles, below):
   1. Player mode request code 0x08 (mode := 0) and code 0x09 (mode :=
      0x11) end in `0x00461010(U)` (`0x004615B2`, `0x004615DA`):
      `0x004C1E30`, path stop `0x00649F70(path, 0)`, **flag 0x10000
      set**, flag 0x4 cleared, flag 0x2 cleared unless U has state 7
      (`0x00639DF0(U, 7)`), `0x0046ECE0(U)`, `0x0046F020(U, 0)`, stat 6
      (life) := 0 (`0x00627260(U, 6, 0, 0)`).
   2. Monster mode request code 0x09 (`0x004AFF60`, byte table
      `0x004B0DF8`[9] → `0x004B09E5`): path stop unless the `monstats2`
      row field `deadCol` (`client/model.md` §19) is set; stat 6 := 0; when U is
      the local player's target (`0x004648F0`), the target is cleared
      (`0x00620C10`); then monstats id 284 (`0x11C`, via `0x00463860`)
      in mode 0xE → its skill restarted (`0x00620210`), no flag; any
      other → mode := 0xC (`0x00480E70`) and **flag 0x10000 set**
      (`0x004B0A5B`). Monster code 0x08 (`0x004B053E`) sets no flag
      (its dead test is the mode).
   3. The frame callback `0x004E2630(U)` (referenced from a data table,
      no direct caller): at animation frame 30 (unit +0x44 >> 8 = 0x1E)
      mode := 0xC and **flag 0x10000 set** (`0x004E2662`); frame 6 and
      22 do other work (`0x006212C0`).
   4. Missiles: the unit returned by the client missile creates
      `0x004CDB40` (`0x004B0AB0`, monster id 435 in code 0x09),
      `0x004CDBA0` (`0x004CF3AA`), `0x004CD540` (`0x004D16A9`,
      `0x004D5B72`); the missile at its animation end (`0x006217C0`)
      in the table functions `0x004D33E0` / `0x004D3D00`; and
      `0x004F0590`'s unit when its fourth argument ≠ 0 (`0x004F0633`,
      callers `0x004F06B6`, `0x004F10FE`).
   Clear: only `0x004647D0(U)` (flag 0x10000 cleared; then flag 0x4
   set unless U is a monster whose monstats2 flag 9 (`0x004638A0`) is
   clear). Its callers: the player machine `0x00461250` (code 0x07,
   `client/model.md` §8 r4), `0x00478BB0` (0x81 type 7, `client/
   model.md` §14 r3), the level hook `0x0045D4B0`, `0x0045D9B0`,
   `0x0045DB20` (the 0x18 / 0x95 revive of §5 r4 is among these),
   `0x00453910`, `0x00463390`, `0x004AFF60`. So a player
   or monster carries the flag from the request that made it dead
   until one of these clears it; a mode change in between (e.g. a
   walk request to a dead unit) leaves the dead test true. d2rs: the
   flag is a field of the client unit, written exactly at 6.1–6.3 and
   the clears; the dead test reads flag or mode.

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
   The server sends dx = (X − path target x) & 0xFF (`combat/vitals.md`
   §5.2, `0x00548760` at `0x0054882C`–`0x00548844`: X byte minus path
   +0x10 byte), so (tx, ty) = 2·(x, y) − target: the path target
   **reflected through the server point**, not the target. The client
   adds, it does not subtract (`0x0045DC50` at `0x0045DCA7` / `0x0045DCAC`;
   `0x0045DB20` the same); both reads 1.14d-confirmed. Effect in
   `client/model.md` §6 r5: a local player lagging behind the server
   point (on the side away from the target) is accepted, one ahead of it
   toward the target is corrected.
4. 0x18 and 0x95 only: if life ≠ 0 and the local player's mode is 0x11
   (dead): `0x00480E70` and `0x004647D0` (leave the dead mode; Phase 6
   player modes, `client/model.md` open question 1).

### 6. Unit states: 0xA7, 0xA8, 0xA9, 0xAA

1. Dispatch owner of 0xA7, 0xA8, 0xA9 and 0xAA (general handlers, act
   at receive). 0xA7 DelayedState, 0xA8 SetState, 0xA9 EndState: the
   rules of `client/stat-lists.md` §3 rules 1–4 (layouts, state on /
   off, state stats, hooks); this section adds only 0xAA. Server side:
   `sim/intents-events.md` §3.5 rule 6 (0xA7–0xA9), §7.9 rule 1 (0xAA).
2. **0xAA** (`0x0045EFA0`; size u8@6): unit (type u8@1, GUID u32@2) not
   in S → nothing. Bit reader (`client/model.md` §10) over bytes 7 …
   size − 1. Loop:
   1. s := 8 bits; s ≥ 255 → end (the server's closing 0xFF).
   2. State on (`0x004D9B20(unit, s)`, `client/stat-lists.md` §3 rule
      3); list := none.
   3. 1 bit = 1 → stats: id := 9 bits; id = 0x1FF ends the stats. An id
      outside the itemstatcost table, or with `Send Bits` (+0x08) = 0,
      **ends the whole message** (no hooks for s, later states lost).
      Else param := the next `Send Param Bits` (+0x09) bits read signed,
      kept as u16, when that is > 0, else 0; value := `Send Bits` bits,
      read signed when `Send Bits` < 32 and the row's flags (+0x04) have
      bit `[0x006CE26C]`, else unsigned; list := the state stat call
      (`0x004D9D70(unit, list, s, id, value, param)`, §3 rule 2 there:
      the first stat makes the list, later ones reuse it).
   4. Hooks `0x004D9E60(unit, s)` (state on hooks, §3 rule 1 there);
      next s.
3. The reader's overflow flag is never tested; past the end it returns
   0 bits (`client/model.md` §10 rule 2), so a stream without its
   closing 0xFF would read state 0 over and over. 1.14d servers always
   close it (`sim/intents-events.md` §7.9 rule 1 step 4). d2rs: when a
   state read overflows, the handler stops and returns an error
   (`client/bridge.md` §6 rule 4); the states applied before stay.

### 7. Other unit messages (general handlers, act at receive)

1. **0x09** AssignLevelWarp (`0x0045CB90` → `0x004661C0`, 11 bytes;
   server `0x0053BCD0`, called from the add messages at `0x005721E2`
   with the unit's type, GUID, class byte, x, y): type u8@1, GUID
   u32@2, class u8@6, x u16@7, y u16@9. Create with the type of byte 1
   (`0x00465FD0`, as 0x51 in §1.3 rule 2: common fields, the room seed
   step at (x, y) ≠ (0, 0); type 5 (tile): unit flags |= 0x22, static
   path at (x, y)); add (`client/model.md` §2 rule 4). If the created
   unit is type 1, `0x00470B70` runs too (no 1.14d sender passes type
   1). Model: a unit with `class`, `position`.
2. **0x11** (`0x0045D0F0` → `0x00464E50`, 8 bytes): type u8@1, GUID
   u32@2, overlay u16@6. Unit absent, or overlay ≥ the overlay count
   (data tables +0xBC0) → nothing. Else one `UnitOverlay` output {unit
   key, overlay, mode 2, sound}: the effect layer adds the overlay
   (`0x00470390(U, overlay, 2, 0, …)`, `render/unit-composite.md`),
   then overlay 151 (0x97) → sound 396 (`impact_steal_life`), 152
   (0x98) → sound 397 (`impact_steal_mana`) on U (`0x004B9A00`,
   `audio/triggers.md`). Model state: none.
3. **0x57** NpcEnchants (`0x0045E400`, 14 bytes): GUID u32@1, u8@5,
   name u16@6, umods u8@8, u8@9, u8@0xA (low byte of u16@0xA), u16@0xC.
   u8@5 ≠ 1, or no monster (1, GUID) → nothing. Else, on the monster
   data (+0x14; only for a type-1 unit with data): +0x14 (u16) := name
   (`0x004AC780`); the umod bytes +0x1C, +0x1D, +0x1E := u8@8, u8@9,
   u8@0xA (`0x004AC740`); u16@0xC ≠ 0 → flags +0x16 |= 4; then
   flags +0x16 |= 8 (`0x004AC810`). Model: those monster-data fields
   (`ClientUnit` kind data, the fields 0xAC writes, §1.2). Then one
   `UmodFx` output {unit key, the nine umod bytes +0x1C…+0x24, flags
   +0x16 bit 3}: `0x004AD020` runs, when umod byte 0 ≠ 0, the client
   umod function of table `0x00724D78` for the four fixed ids at
   `0x006DA4C8` and then for each of the nine umod bytes (argument:
   flags bit 3); Phase 6 effects.
4. **0x5F** PortalFlags (`0x0045E5D0`, 5 bytes; server `0x0053B400`):
   u32@1. No local player → fatal 0xD15 (bridge: handler error). Else
   the local player's player data +0x2C := u32@1 (`0x006221E0`: fatal
   0xFBF for a null unit, 0xFC0 for a non-player). Model: local player
   `pdata_2c`. Recorded `5f 01000000` at both joins (`-015956` seq 136,
   `-022633` seq 114). Meaning and only client reader (2026-10-08, open
   question 8): the portal flags, ORed by the local player's room-change
   step (`client/model.md` §17 r6 step 2); no client code consumes them.
5. **0x60** TownPortalState (`0x0045E610` → `0x004BDF30`, 7 bytes):
   portal flags u8@1, destination level u8@2, object GUID u32@3
   (`sim/intents-events.md` §3.5, `world/objects.md` §14). No local
   player → fatal 0xD21. Object (2, GUID) absent → nothing; its class
   not 59 or 60 → fatal 0x560. Else object data +0x04 := level; portal
   flags (object data +0x05, `0x006222C0` / `0x00622300`) |= u8@1 & 3
   (bits 1 and 0 are only ever set here, never cleared). Model: the
   object's kind data {interact (+0x04), portal flags (+0x05)}.
6. **0x73** (`0x0045E6D0`, 32 bytes): creates a **client-only missile**
   (set C, not `ClientWorld`, `client/model.md` §2 rule 2). Bytes 1–4
   are not read. One `ClientMissile` output with the captured local
   player key and fields: class u16@5, u32@7, u32@0xB, u32@0xF, u32@0x13,
   u16@0x17, source type u8@0x19, source GUID u32@0x1A, u8@0x1E, u8@0x1F.
   The effect layer builds the 0x5C-byte create record (`0x004CD540`;
   owner = the local player, class, +0x14 := u32@7, +0x18 := u32@0xB,
   +0x30 := u8@0x1E; flags 0x40000001, or 0x40000021 with +0x1C :=
   u32@0xF and +0x20 := u32@0x13 when both are ≠ 0), then on the new
   missile: source link (`0x00621CC0(m, u8@0x19, u32@0x1A)`), stat 328
   := u8@0x1F when ≠ 0, `0x0064A330(m, u16@0x17)`, and +0x44 :=
   `0x0045C3E0(seed +0x20, +0x48 >> 8)` << 8 (a draw on the missile's
   own seed). Owner of the missile body: `missiles/client.md`.
7. **0x74** PlayerCorpseAssign (`0x0045E7B0` → `0x00462F60`, 10 bytes):
   u8@1, player GUID u32@2, corpse GUID u32@6 (both type 0). u8@1 = 0,
   or player P absent → nothing.
   1. P not dead (`0x00464820`, `msg-skills.md` §7 r4.3) → mode := 0
      (`0x00480E70(P, 0)`).
   2. P is the local player: corpse K (0, u32@6) absent → stop. Else K's
      path is reset (`0x00648CE0(path, 0)`, `0x00649190(K, 5)`) and K
      is placed in P's room at P's position (`0x00650BE0(path, K,
      room, x, y)` with x, y from `0x0045ADF0` / `0x0045AE20`; failure
      fatal 0xABA), then K's path direction := P's (`0x006487F0` →
      `0x006488A0`).
   3. For every node of P's inventory (+0x60) in list order with kind 3
      (body): the item is taken off P (`0x0063D2B0`), the body-location
      fix-up `0x00462EE0`, P's gfx for that location refreshed
      (`0x0046F280`), the item removed from the inventory again
      (`0x0063AD90`: none → fatal 0xAC8, another item → fatal 0xAC9),
      `0x004C0EC0(item, P, 0, 0)`, body slot cleared (`0x0063BE30`),
      `0x00470610(P, 0)`, and the item unit removed from S
      (`client/model.md` §2 rule 5).
   Model: P's mode, K's position and direction, P's inventory and the
   removed item units.
8. **0x7E** (`0x0045E970`, 5 bytes): the message bytes are not read.
   Act index A of the local player's room level (`0x00463DD0` →
   `0x00620BB0` → `0x0061A1B0` → `0x006427F0`, `render/lighting.md`
   §9.2 r1) → one `CommonCof` output {A}: the effect layer loads
   `DATA\GLOBAL\` + the act's common COF list name (table `0x006D6C30`,
   `0x0046F310` → `0x0046DDF0`; Phase 6 assets). Recorded `7e 000000ff`
   (`-015956` seq 179) and `7e 00000041` (`-022633` seq 155).
9. **0x98** (`0x0045DE50` → `0x004B1240`, 7 bytes): GUID u32@1, u16@5.
   Monster (1, GUID) with monster data → data +0x40 := u16@5, or −1
   when u16@5 = 0xFFFF. Model: monster kind data `mdata_40`. No client
   reader (2026-10-08, open question 9): no instruction in the client
   code reads monster data +0x40 (a scan of every `[r + 0x40]` read
   whose base was loaded from a unit's +0x14 finds only this writer;
   the shared accessors `0x00554040` / `0x00554070` are called from
   server code alone, `0x0056D840` / `0x0056D8D0`), so the field has no
   observable client effect. The server sends 0x98 only for class 528
   (`sim/intents-events.md` §7.2 monster row), u16@5 = its AI control
   +0x3C (`0x0058F710`, the spawner pick of `monsters/ai-bodies-2.md`
   §13.1).
10. **0xA4** BaalWave (`0x0045D760`, 3 bytes): class u16@1. Class ≥
    the monstats count → nothing. Else one `MonsterPreload` output
    {class}: `0x0046F870(class, 0)` loads, for each of the 15 modes of
    table `0x00712AC4` (12, 0, 1, 2, 15, 4, 3, 5, 7, 6, 13, 8, 9, 10,
    11; count `[0x00712B00]` = 15) whose bit is set in the class's
    monstats2 row (+0xF0 bit field), that mode's graphics
    (`0x0046F650`; Phase 6 assets). Model state: none.
11. **0xAB** NpcHeal (`0x0045F120`, 7 bytes; server `0x0053C150`, the
    0xAB pending records of `sim/intents-events.md` §7.9 r2): type
    u8@1, GUID u32@2, life u8@6 (fraction of 128). Unit absent, the
    local player, or unit flag 0x200 set → nothing. A monster → stat 6
    (life) := u8@6 << 8 (`0x00627260`, layer 0). Any other type →
    roster life percent (§8 rule 6) := (u8@6 × 100) / 128, truncated
    toward zero. Recorded `ab 01 13000000 30` (`-015956` seq 219061).
12. **0x16** UnitPositions (`0x0045D2E0`, general handler; size u16@1,
    min 13; 2026-10-08 read; answers open question 6): count n := u8@3;
    n = 0 → fatal 0x855. Entries of 9 bytes from @4, i = 0 … n − 1:
    type u8@+0, GUID u32@+1, x u16@+5, y u16@+7; unit (type, GUID) in
    S → `check(U, x, y, 0, 0, 0)` (`0x004804E0`, `client/model.md` §6;
    the same call as §4 r1's check); absent → skipped. Nothing else.
    The size word is not compared with 4 + 9n. No 1.14d function
    produces 0x16, so d2rs registers the no-op of `client/bridge.md` §6
    r6 (owner of the d2rs handling); this rule records what the 1.14d
    handler would do.
13. **0x17** (no layout; `sim/server-messages.tsv` row 0x17): the
    general handler `0x0045C900` is a bare return; the unit handler
    `0x0045D260(U, message)` does nothing for the local player, is
    fatal 0x841 for no unit, and otherwise reads two 16-bit message
    words (@8, @0xA) as **addresses** and dereferences them
    (`0x0045D298`–`0x0045D2A7`) before a mode request `0x00480C10(u8@7,
    U, {u8@6 >> 1, &those words}, 1)`: an access violation in 1.14d for
    any value below 0x10000. 0x17 has size 0 in the size table, so the
    split never dispatches it (`client/bridge.md` §6 r6, owner of the
    d2rs no-op); this rule records the dead handler only.

### 8. Player roster (0x5B, 0x5C, 0x65, 0x75, 0x82, 0x8E; life from 0x0D, 0xAB)

1. The client keeps two lists of 0xD8-byte player records, linked at
   +0x80: **active** `[0x007BB5C0]` and **inactive** `[0x007BB5C4]`.
   Record fields:

   | Offset | Field | Writer |
   |---|---|---|
   | +0x00 | name (NUL-terminated, 16 bytes) | 0x5B |
   | +0x10 | player GUID | 0x5B, 0x8E (new record) |
   | +0x14 | life percent | 0x0D (§4 r2), 0xAB |
   | +0x18 | kill count | 0x65 |
   | +0x1C | class (u32) | 0x5B u8@7 |
   | +0x20, +0x22 | u16@0x18, u16@0x1A of 0x5B | 0x5B |
   | +0x30 | u16@0x1E of 0x5B | 0x5B |
   | +0x34 | a UI handle (`0x004DC270`; freed `0x004DC2B0`) | 0x5B |
   | +0x38 | corpse list: 8-byte nodes {corpse GUID, next} | 0x8E |
   | +0x3C, +0x40 | portal GUIDs | 0x82 |
   | +0x44 | u16@0x20 of 0x5B | 0x5B |
   | +0x46, +0x4A | the two strings of 0x5B (see rule 2) | 0x5B |
   | +0x66 | the two names formatted (`0x006D4E34`) when string 1 is not empty | 0x5B |

2. **Lookup** `0x004792E0(GUID)`: GUID = −1 → none; else the first
   active record with GUID +0x10 = GUID, or holding it in its corpse
   list. Inactive records are never found by it.
3. **0x5B** PlayerJoined (`0x0045E4E0` → `0x0047A6F0`; size u16@1,
   min 34): GUID u32@3, class u8@7, name @8 (16 bytes), u16@0x18,
   u16@0x1A, u16@0x1C, u16@0x1E, u16@0x20, string 1 @0x22, string 2
   after string 1's NUL. GUID −1 → fatal 0xB3. A record found by GUID
   or by name (`0x00479360`, active list `[0x007BB5C0]`, next +0x80:
   the first record whose GUID +0x10 equals the GUID, or whose name at
   +0x00 equals the message name byte for byte up to and including the
   NUL (case-sensitive, no length bound); 2026-10-08 read) is updated in place; else an
   inactive record with that GUID is moved out of the inactive list,
   else a new record is allocated (first 0x84 bytes zeroed); +0x34 :=
   a new handle; fields filled (`0x004793C0`): name, GUID, class,
   +0x20, +0x22, +0x30, +0x44 as in rule 1; string 1 copied to +0x46,
   then string 2 copied to +0x4A (so string 1 keeps only its first 4
   bytes when longer than 3 characters); u16@0x1C is not stored. A new
   or moved record is prepended to the active list. Then the party UI
   refresh `0x00479AB0`, `0x0049A640` (output `RosterChanged`).
   Recorded `5b 2400 01000000 04 "charactertest" 0100 ffff 0000 0000
   0000 00 00` (`-015956` seq 267): GUID 1, class 4, +0x20 = 1, +0x22 =
   0xFFFF.
   What the 1.14d sender `0x0053C940` puts in the words and strings
   (static read 2026-10-08): u16@0x18 = the player's level (stat 12,
   `0x00625480(P, 12, 0)`), u16@0x1A = its party id (`0x00554630`;
   0xFFFF = no party), u16@0x1C = u16@0x1E = 0 always, u16@0x20 = the
   server client's u16 +0x45E, string 1 = the client's 4 bytes at +0x460
   (so at most 3 characters), string 2 = the client's text at +0x464.
   Only the legacy save header loader `0x00532690` writes those client
   fields (`formats/d2s-legacy.md` §2 r5); for any other character they
   are 0 and empty, as recorded. So the roster's +0x20 is the level and
   +0x22 the party id (the same slots 0x75 writes, rule 10).
4. **0x5C** PlayerLeft (`0x0045E530` → `0x0047A8B0`, 5 bytes;
   `sim/server-messages.tsv` `guid:u32@1`, sender `0x0053CA90`): GUID
   u32@1 (−1 → fatal 0x121). Its own walk of the active list (not rule
   2: the corpse lists are not searched): the first record with GUID
   +0x10 = GUID is unlinked, its handle +0x34 freed (`0x004DC2B0`), its
   corpse nodes and then the record freed. Then `RosterChanged`, found
   or not (`0x00479AB0`, `0x0049A640`). 0x5C reads no count and writes
   no other record (re-read 2026-10-08 to separate it from rule 5).
5. **0x65** PlayerKillCount (`0x0045E6B0` → `0x00479B10`, 7 bytes;
   `sim/server-messages.tsv` `guid:u32@1 count:u16@5`, sender
   `0x0053D9C0`): GUID u32@1, count u16@5. Record found (rule 2) → +0x18 := count,
   sign-extended from 16 bits; `RosterChanged`. Not found → nothing.
   Recorded at join `65 01000000 0000` and after kills 1, 2, 3, …
   (`-015956` seq 222604 ff.).
6. **Life percent** `0x0047A690(GUID, v)`: record found → +0x14 := v.
7. **0x82** PortalOwnership (`0x0045E9D0`, 29 bytes; server: owner
   GUID u32@1, owner name @5–@20, u32@21, u32@25, `sim/intents-events.md`
   §6 r6): object (2, u32@21) present → its object data +0x28 := the
   name (bounded copy, 16 bytes, `0x004BDFE0`). u32@25 = −1 → stop
   (the roster is not touched). Else object (2, u32@25) present → the
   same name copy; then the owner's record (rule 2) +0x3C := u32@21,
   +0x40 := u32@25. Model: objects' kind data `owner_name`; roster.
8. **0x8E** CorpseAssign (`0x0045EAA0`, 10 bytes): u8@1, player GUID
   u32@2, corpse GUID u32@6. u8@1 ≠ 0 (`0x0047A3D0`): record found
   (rule 2), else a new 0x84-byte zeroed record with GUID +0x10 is
   prepended to the **inactive** list (allocation failure fatal
   0x687); the corpse GUID is prepended to its corpse list unless
   present (node allocation failure fatal 0x698). u8@1 = 0
   (`0x0047A490`): the record found → the first node with that corpse
   GUID is unlinked and freed. No UI refresh.
9. d2rs: `roster: Vec<RosterRecord>` (active, in list order, newest
   first) and `roster_inactive` in `ClientWorld`, written only by the
   messages above; the handle +0x34 and the formatted string +0x66 are
   UI fields and not modelled. `RosterChanged` {} tells the UI layer to
   rebuild the party view from the roster captured in the output
   (`client/bridge.md` §10 r3: the payload carries the active records).
   0x75 (rule 10) also writes the roster.
10. **0x75** PlayerPartyInfo (`0x0045E7E0` → `0x0047A850`, 13 bytes;
    2026-10-08 read): GUID u32@1, party u16@5, level u16@7, u16@9,
    u16@11 (sender `0x0053DA90`: party `0x00554630`, level stat 12;
    u16@9 = the flags +0x04 and u16@11 = the word +0x0C of the
    receiving player's relation entry for that GUID (`0x0055B350`,
    `0x0055B3F0`; list `combat/hit.md` §7 r4), 0 without an entry;
    for the receiver itself u16@9 = 0 and u16@11 = 1). Record r found
    (rule 2), else nothing. If the GUID is in
    r's corpse list (r was found as a corpse holder), nothing. Else r
    +0x22 := party, +0x20 := level, +0x30 := u16@11 (zero-extended);
    u16@9 is not stored (the same slots as 0x5B's u16@0x1A, u16@0x18,
    u16@0x1E, rule 3). Then the pet pass `0x00478FA0`, rule 10.1;
    then `RosterChanged` (`0x00479AB0`, `0x0049A640`).
    1. **Pet pass** `0x00478FA0` (2026-10-08 read; answers open
       question 11). A := the local player (`0x00463DD0` =
       `[0x007A6A70]`, read once before the walk; may be none). For
       each pet record p of the list `[0x007BB5BC]` (`client/model.md`
       §14, link +0x30) in list order with type +0x04 = 4 (`pettype`
       row 4, `skeleton`; type 5 `skeletonmage` is not passed) whose
       monster U = (1, p +0x08) is in S (`0x00463990`): h :=
       `0x00478E70(A, U)` and t := 1 when h = 0, else 0
       (`0x00478FE3`–`0x00478FE9`); U's graphics record (unit +0x54)
       +0x34 := t (`0x00463E20` → `0x0046F1C0`) and +0x38 := t
       (`0x00463E80` → `0x0046F220`, the shift-index writer of
       `render/shading.md` §6 r6.3: t is the `palshift` map index, 1 =
       the friendly skeleton colours; render state, not in the model).
       `0x00478E70(A, U)` (fastcall):
       1. g := U's GUID (+0x0C); q := the **first** pet record of the
          list with pet GUID +0x08 = g, any type (it can differ from p
          when two records share the GUID).
       2. A none → h = 1 (U is never none here).
       3. q none: A and U both monsters (type 1) and
          `0x00650D70(A, U)` ≠ 0 → 0, else 1 (unreachable from the pass:
          q exists).
       4. q's owner +0x0C = A's GUID → h = 0.
       5. Else h := `0x0047A070(A's GUID, owner)`: a := the roster
          record of A's GUID, b := that of the owner (both by §8 r2,
          `0x004792E0`); either none → 1; a +0x22 = b +0x22 and ≠
          0xFFFF (same party) → 0; else `0x004DC440(A's GUID, owner,
          8)` (`render/shading.md` OQ 7 answer: the node for the owner
          in A's roster relation list, flags & 8; no node → 0) ≠ 0 → 1,
          else 0.
       So t = 1 for a skeleton of the local player, of a party member,
       or of a listed player without relation bit 8 (D2MOO: hostile)
       toward them; t = 0 with no local player, an owner without a
       roster record (or the local player without one), or bit 8 set.
       Single player: every type-4 record is the local player's → t = 1
       for each present skeleton.
    0x75 in single player: recorded once in `-022633` (sender `0x0053DA90`): `75
    01000000 ffff 0200 0000 0100` (GUID 1, party 0xFFFF, level 2,
    u16@11 = 1).
11. **Out of scope (Phases 0–6): multiplayer only**
    (`sim/intents-events.md` §4 rule 4; none occurs in either
    recording): 0x7F AllyPartyInfo (`0x0045E990`), 0x8B
    PlayerRelationship (`0x0045EA60`), 0x8C RelationshipUpdate
    (`0x0045EA70`), 0x8D AssignPlayerToParty (`0x0045EA90`), 0x90
    PartyAutomapInfo (`0x0045E9C0`; its automap use is `ui/automap.md`
    §12, a two-player case). The d2rs handler of each is the no-op of
    `client/bridge.md` §6 rule 7.

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

Seeds only (`client/model.md` Randomness): 0x59, 0xAC and 0x51 creation
at a point ≠ (0, 0) step the room seed once (`client/model.md` §2 rule
6; for 0x51 through `0x00466300` → `0x00465FD0`, so every recorded 0x51,
all at non-zero points, steps it); 0x59 steps the new player's seed once
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
- 0x5B copies string 2 over string 1 from byte 4 on (§8 rule 3).
- 0x8E may create a record in the inactive list, which the lookup of §8
  rule 2 never finds; a later 0x5B with that GUID moves it to the
  active list.
- 0x82 with u32@25 = −1 leaves the roster unchanged (§8 rule 7).
- 0x60 only ORs the portal flags; 0x60 never clears a bit.
- 0x11 / 0x73 / 0xA4 / 0x7E have no model effect; their outputs are
  the whole message.
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
| `aa 00 01000000 0c 69 59 f9 ff 1f`, player (0, 1) in S | state 105 on; its list: stat 172 := 2 (param 0); hooks for 105; then 8 bits 0xFF end | B 103, §6 rule 2 |
| `aa 01 08000000 0c 69 59 f9 ff 1f` | the same on monster (1, 8) | A 184 |
| 0xAA with state 5, list bit 1, first stat id with `Send Bits` 0 | state 5 on, no stat, no hooks; rest ignored | synthetic, §6 rule 2.3 |
| 0xAA whose stream lacks the closing 0xFF | states before applied; handler error | synthetic, §6 rule 3 |
| 0x15 for a dead monster | position unchanged | synthetic, §3 rule 4.3 |
| 0x0A type 1, GUID = local player's hireling | nothing | synthetic |
| 0x96 with dx 0x80 | tx = x + 128 | synthetic |
| 0xAC component bits for counts 7, 3, 3, 3, 3, 10, 0, 5, 12, 12, 0 × 6 (`skeleton1`, `data/callbacks.md` §5 example) | 3, 2, 2, 2, 2, 4, 1, 3, 4, 4, 1 × 6 = 33 bits | §1.2 rule 7 |
| 0xAC with class ≥ `monstats` row count, or a row whose `MonStatsEx` is −1 | no component reads, no unit | synthetic, §1.2 rule 7 |
| 0x51 with type 4 | handler error | synthetic, §1.3 rule 5 |
| 0x5F `5f 01000000`, local player present | local player data +0x2C = 1 | `-022633` seq 114, §7 r4 |
| 0x60 `60 03 01 09000000`, object (2, 9) class 59, flags 0 | object +0x04 = 1, flags 3 | synthetic, §7 r5 |
| 0x60 for object class 2 | handler error (fatal 0x560) | synthetic, §7 r5 |
| 0x11 type 1, overlay 0x97 | output `UnitOverlay` {(1, GUID), 151, mode 2, sound 396}; model unchanged | synthetic, §7 r2 |
| 0xAB `ab 01 13000000 30`, monster (1, 0x13) | stat 6 = 0x3000 | `-015956` seq 219061, §7 r11 |
| 0xAB type 0, GUID 1, life 0x40, roster has GUID 1 | roster +0x14 = 50 | synthetic, §7 r11, §8 r6 |
| 0x5B seq 267 bytes then 0x65 `65 01000000 0100` | roster [{name "charactertest", GUID 1, class 4, +0x20 1, +0x22 0xFFFF, kills 1}]; two `RosterChanged` | `-015956` seq 267, 222604 |
| 0x65 count 0x8001 | kills = −32767 | synthetic, §8 r5 |
| 0x8E add (player 1, corpse 7), then 0x65 for GUID 7 | 0x65 updates the record of player 1 (corpse-list match) | synthetic, §8 r2, r8 |
| 0x5C GUID 1 | record unlinked and freed; `RosterChanged` | synthetic, §8 r4 |
| 0x7E `7e 000000ff`, local player in level 1 | `CommonCof` {act 0} | `-015956` seq 179, §7 r8 |
| 0x0A type 1, GUID 0x21, pet list empty | (1, 0x21) removed (hireling GUID −1) | synthetic, §2 rule 3 |
| local player placed from a room of level 109 (Pal 4, Act 4) into a room of level 133 (Pal 0, Act 4) | palette switch to act 1 (`act1\pal.pl2`) | §3 rule 4.4; patch_d2 `levels.txt` |
| local player placed from level 1 (Pal 0) into level 2 (Pal 0) | no palette switch | §3 rule 4.4 |

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
Area 4 session (2026-10-07): §7–§8 read from the handlers `0x0045CB90`,
`0x0045D0F0`, `0x0045E400`, `0x0045E4E0`, `0x0045E530`, `0x0045E5D0`,
`0x0045E610`, `0x0045E6B0`, `0x0045E6D0`, `0x0045E7B0`, `0x0045E970`,
`0x0045E9D0`, `0x0045EAA0`, `0x0045DE50`, `0x0045D760`, `0x0045F120`
and their callees named above (register arguments from the
disassembly); table `0x00712AC4` and its count read from the image;
recorded bytes from both recordings.
States session (2026-10-07, spec-senders-area-2): 0xAA `0x0045EFA0`
read in full (`0x0045EFC3`–`0x0045F104`: the 8-bit state loop, the
list bit at `0x0045F003`, 0x1FF → hooks at `0x0045F030`, the table and
send-bits exits at `0x0045F038`–`0x0045F067` jumping to the return); 0xA7–0xA9 left to
`client/stat-lists.md` §3; the recorded 0xAA bytes decode as in the
table (269 records, both recordings).
Act-switch session (2026-10-07): `0x004654C0` at `0x00465603`–`0x00465634`
(byte +2 of both `0x0061DB70` records compared, CL = new +2 into
`0x004FB480`, whose palette act is CL + 1); `levels` offsets from
`data/fields.tsv`; Pal / Act counted in patch_d2 `levels.txt` (137 rows).

Gap pass (2026-10-08, PC 1 lane D): §8 r10.1 from `0x00478FA0`,
`0x00478E70`, `0x0047A070`, `0x004DC440`, `0x00463E20` → `0x0046F1C0`,
`0x00463E80` → `0x0046F220`, `pettype.txt` rows; §4 r6 from
`0x00464810` (nine call sites), `0x00464820`, `0x004647D0`,
`0x00461010`, the `0x004AFF60` byte table `0x004B0DF8` / pointer table
`0x004B0DC0` read from the image, `0x004E2630`, `0x006217C0`; §7
r12–r13 from `0x0045D2E0`, `0x0045D260`, `0x0045C900`
(`tools/ghidra/disasm.py fn` / `xref`).

## Open questions

1. Meaning of each mode-request code per unit kind: Phase 6 unit-modes
   spec (`client/model.md` open question 1).
2. ~~Monster creation's table stats and placement~~: answered in §1.2
   rule 6. Open inside it: `0x004AE0A0`, `0x004AE4F0`, `0x0046C140`,
   `0x0046C570` (client monster AI / animation set-up, Phase 6).
3. ~~`0x00621CC0(unit, 0, v)`~~: answered in §1.2 rule 4 (source-unit
   link to player GUID v).
4. ~~The party roster~~: answered in §8 (0x0D's life percent is §8
   rule 6). Open: the meaning of the 0x5B words u16@0x18 … u16@0x20 and
   the two strings (D2MOO: level, party id, account names; nothing
   reads them in the single-player recordings).
   *Answered* (static, 2026-10-08): §8 rule 3 (sender `0x0053C940`:
   level, party id, 0, 0, then legacy-save client fields; 0x75's
   words §8 rule 10).
5. *Answered (2026-10-08)*: §3 rule 2 (`0x0063EA40`: dying or dead
   monster) and §1.3 rule 3 (`0x004BD6B0`: shrine record set and the
   shrine function called). Original question: `0x0063EA40` (0x15 rule 2) and `0x004BD6B0` (0x51 rule 3): what
   they test. *Answered* for `0x00621B00`: an object whose objects.txt
   `SubClass` has bit 0 (shrine), `client/model.md` §15 rule 1.
6. *Answered (2026-10-08)*: §7 r12 (0x16) and §7 r13 (0x17; never
   sent, its unit handler faults). Original question: 0x16 UnitPositions (`0x0045D2E0`, also a position check) and 0x17:
   not seen in the single-player recordings; left TBD.
7. A recording with a hireling (0x7A / 0x81, 0xAC of the hireling)
   confirms §1.2 rules 2–3 and §2 rule 2 with a real pet list
   (`client/model.md` open question 10).
8. *Answered (2026-10-08)*: §7 r4 (portal flags; the only client
   reader is the room-change step, `client/model.md` §17 r6 step 2).
   Original question: who reads the local player's player data +0x2C
   (0x5F, §7 r4; the server's meaning: sender `0x0053B400`), so the
   field can be named.
9. *Answered (2026-10-08)*: §7 r9 (no client reader; the server's
   value is the class-528 AI control +0x3C). Original question: monster
   data +0x40 (0x98, §7 r9): its reader and meaning.
10. The client missile body of 0x73 (`0x004CD540`, `0x0064A330`,
    `0x0045C3E0`: `missiles/client.md`) and the umod client functions of 0x57 (table
    `0x00724D78`): Phase 6 effects spec.
11. *Answered (2026-10-08)*: §8 r10.1 (type 4 = `skeleton`; the
    owner / party / relation test; the pair writes graphics +0x34 and
    +0x38, the shift index of `render/shading.md` §6 r6.3). Original
    question: §8 rule 10's pet pass `0x00478FA0`: which pets are type 4, what
    `0x00478E70(local player, U)` tests, and which render spec owns the
    palette-level pair `0x00463E20` / `0x00463E80` it sets. Settle by
    reading `0x00478E70` and the pet-type writers of 0x7A / 0x81.
