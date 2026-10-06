# Spec: Missiles — server-do and server-hit bodies (part 3)

- **Status:** draft: every body here read from the 1.14d `Game.exe`
  disassembly (addresses per section, register and stack arguments
  checked with `tools/ghidra/disasm.py fn`). No recording covers them
  (Open question 1).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::missiles` (server-do / server-hit bodies)
- **Related specs:** `missiles/bodies.md` (part 2: conventions, the use-count table that
  orders this work, §1–§30), `missiles/missiles.md` (parameter record §R2.1, creation
  §R2.3, default flight §R4, hit handler §R5, damage §R6, catalogues
  §R9); `missiles/srvdo.tsv`, `missiles/srvhit.tsv`; `skills/bodies.md`
  (`eval`, `scan_unit` §2.12, `accepts` §2.11, `apply_state` §2.7,
  `aura_fill` §2.6, summons §6.1–§6.5, missile at a point §6.13);
  `monsters/init.md`, `monsters/ai.md` (umods, owner data, minion lists).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 45–58 |
| Inputs | 59–66 |
| Outputs / state changes | 67–72 |
| Rules | 73–74 |
|   31. Server-do 12 Diablo wall maker `0x005AECA0` | 75–85 |
|   32. Server-hit 20 Lightning Fury `0x005AB370` | 86–111 |
|   33. Server-do 13 Bone Wall maker `0x005AEDA0` | 112–138 |
|   34. Server-hit 21 Battle Cry `0x005AB500` | 139–157 |
|   35. Server-hit 22 Fist of the Heavens delay `0x005ADD20` | 158–185 |
|   36. Server-hit 24 panther pot orange `0x005A9BF0` | 186–197 |
|   37. Server-hit 25 panther pot green `0x005AB820` | 198–221 |
|   38. Server-hit 28 Grim Ward scare `0x005ABA10` | 222–240 |
|   39. Server-do 15 Frozen Orb `0x005AF030`, server-hit 29 `0x005ABB00` | 241–279 |
|   40. Server-do 16 Frozen Orb nova `0x005AF170` | 280–296 |
|   41. Server-hit 31 fire head `0x005ABD70` | 297–312 |
|   42. Server-hit 32 Cairn Stones `0x005ABE50` | 313–323 |
|   43. Server-do 18 tower chest spawner `0x005AF300`, server-hit 33 `0x005ABEB0` | 324–358 |
| Constants & data dependencies | 359–375 |
| Randomness | 376–387 |
| Edge cases & original bugs | 388–403 |
| Test vectors | 404–416 |
| Provenance | 417–432 |
| Open questions | 433–440 |
<!-- /index -->

## Summary

Continuation of `bodies.md` past its size limit, in the same order (most
live `missiles.txt` rows first, ties by the lowest row) and with the
same conventions (`bodies.md` Summary): "flight" = default flight
(`missiles.md` §R4, `0x005AE1F0`), its result returned; position = the
missile's path x / y; owner = `0x00552FD0`; skill / level = missile data
+0x0A / +0x0C (`0x0064A280` / `0x0064A210`); frames left `0x0064A380`,
elapsed `0x0064A3B0`; data +0x28 / +0x2C read `0x0064A730` /
`0x0064A780`, written `0x0064A710` / `0x0064A760`; "new-step flag" =
path +0x34 bit 3 (`0x006505C0`, 0 without a path); "zeroed record" = a
`missiles.md` §R2.1 parameter record cleared to 0 and passed to the
creator `0x0059FA30`. Section numbers continue from `bodies.md` §30.

## Inputs

| Name | Type | Source |
|---|---|---|
| missile unit | type 3 | ECX game, EDX missile (server-do); + stack unit (server-hit) |
| missiles.txt record | 0x1A4 bytes | data tables +0xB64, count +0xB6C |
| skills record | 0x23C bytes | data tables +0xB98, count +0xBA0; columns by `data/fields.tsv` |

## Outputs / state changes

Created missiles and monsters, states and stat lists on hit units,
damage through the area and damage-stage helpers, missile data +0x28 /
+0x2C, draws on the missile seed where named.

## Rules

### 31. Server-do 12 Diablo wall maker `0x005AECA0`

Row: diabwallmaker (179); `SubMissile1` diabwall.

1. Missile none, no record or `SubMissile1` < 0 → return 2.
2. Owner none → return 2 (D2MOO 1.10f creates without an owner).
3. New-step flag: zeroed record, flags 0x21 (position given, target
   absolute); owner (fetched again); start = target = the missile's
   position; class `SubMissile1`; skill, level. Create.
4. Return flight.

### 32. Server-hit 20 Lightning Fury `0x005AB370`

Row: lightningfury (206); `sHitPar1` 0, `sHitPar2` 0, `HitSubMissile1`
furylightning.

1. Missile none, no record or `HitSubMissile1` < 0 → return 1. k =
   skill, L = level; k invalid → return 1. O = owner; none → return 1.
2. (x, y) = position. r = `sHitPar1`; ≤ 0 → max(`eval(O,
   k.aurarangecalc, k, L)`, 1). n = `sHitPar2`; ≤ 0 → max(`eval(O,
   k.calc1, k, L)`, 1) (r first).
3. f = k.`aurafilter` (+0x50), 0 → 0xA783.
4. `scan_unit(game, O, x, y, r, f, fury_cb, arg, noaura 1)`
   (`0x0056B7E0`, `skills/bodies.md` §2.12); arg = {O, missile, k, L,
   n, `HitSubMissile1`}.
5. Return 3.

`fury_cb` = `0x005AB2A0` (ECX scan context, EDX unit U): context count
≥ n → 0 (not counted). Else zeroed record, flags 0x20 (target
absolute): owner O, origin the missile, skill k, level L, class
`HitSubMissile1`, target = U's position; create; return 1 (counted).
So at most n bolts, to the first n accepted units in scan order (rooms
in adjacency order, units in room-list order).

Live: Lightning Fury `aurarangecalc` par3, `calc1` ln12, `aurafilter`
42371.

### 33. Server-do 13 Bone Wall maker `0x005AEDA0`

Row: bonewallmaker (207). Data +0x28 = GUID of an anchor monster, +0x2C
= pieces left (both set by the skill at creation, skills spec).

1. O = owner; none → return 2. Data +0x2C = 0 → return 2.
2. New-step flag clear → return flight.
3. O again; none → return 2. k = skill, L = level; R = k's record
   (`0x0045C4B0`); none → return 2.
4. c = `summon_class(O, k, L, &mode)` (`0x0056E620`,
   `skills/bodies.md` §6.1); c < 0 → return 0 (no flight this frame).
5. pt = R.`pettype` (+0xBE, byte); ≥ pettype count (data tables +0xBF0)
   → 0.
6. M = the monster with GUID data +0x28 (`0x00552F60(game, 1, GUID)`);
   none → return flight.
7. P = `summon_spawn` (`0x0056D940`, `skills/bodies.md` §6.2) with
   flags 0xD (position given, no second try, keep flag 0x80000000
   clear), owner O, class c, AI special state 0, mode `mode`, position
   = the missile's, pet type pt, pet max 0. None → return flight.
8. P's owner data := (M's GUID, 1, 0, 0) (`0x0058F030`); P joins M's
   minion list (`0x0058F100(game, M, P)`); umod 15 (`partydead`,
   `0x005A4850(game, P, 15, 0)`, `monsters/init.md`); `skill_stats(game,
   O, P, k, L, 0)` (`0x005C4470`, `skills/bodies.md` §6.5); P's stored
   owner := O (`0x00621CE0`); `0x005B1990(game, P, 0, 9)` (Open
   question 2).
9. Data +0x2C −= 1. Return flight.

### 34. Server-hit 21 Battle Cry `0x005AB500`

Row: battlecry (219).

1. O = owner; none or no unit → return 1.
2. k = skill, L = level; k invalid → return 1. s = k.`auratargetstate`
   (i16); s < 0 or s **>** states count → return 1 (s = count passes).
3. f = 0xA783 when k.`aurafilter` ≠ 0, else 0 (inverted fallback;
   Battle Cry's filter is 98304, so 0xA783 is used).
4. `accepts(O, unit, f)` (`0x0056B3E0`, `skills/bodies.md` §2.11) = 0 →
   return 1.
5. Lst = `apply_state` (`0x0056E970`, `skills/bodies.md` §2.7) with
   source O, target the unit, skill k, level L, duration `eval(O,
   k.auralencalc, k, L)`, stat field 0 and value 0 (not −1: list stat 0
   := 0), state s, callback 0 (default).
6. Lst → `aura_fill(unit, Lst, R, k, L)` (`0x005C6CC0`; formulas on the
   hit unit).
7. Return 0.

### 35. Server-hit 22 Fist of the Heavens delay `0x005ADD20`

Row: fistoftheheavensdelay (233); `sHitPar1` 0, `sHitPar2` 0,
`HitSubMissile1` fistoftheheavensbolt. Data +0x28 / +0x2C = (type, GUID)
of the struck unit (set by the skill).

1. Missile none, no record or `HitSubMissile1` < 0 → return 1. O =
   owner; none → return 1. k invalid → return 1.
2. T = the unit (data +0x28 type, +0x2C GUID) (`0x00552F60`); none →
   return 0.
3. (x, y) = position. r = `sHitPar1`, ≤ 0 → max(`eval(O,
   k.aurarangecalc)`, 1); n = `sHitPar2`, ≤ 0 → max(`eval(O, k.calc4)`,
   1).
4. Damage record: `0x005A89A0(missile, T, record)` (zeroes it, then
   `missiles.md` §R6.2 rolls on the missile seed); then the damage tail
   `0x005ADCD0(game, missile, record, T)` (`missiles.md` §R6.1 step 3:
   result flags, damage execution, armor −= stat 120 for a non-hireling
   monster).
5. f = k.`aurafilter`, 0 → 0xA683. `scan_unit(game, O, x, y, r, f,
   fist_cb, arg, noaura 0)`; arg as §32.
6. Return 1.

`fist_cb` = `0x005AB630`: n > 0 and count ≥ n → 0; else as `fury_cb`
(one `HitSubMissile1` aimed at the unit, return 1).

Live: Fist of the Heavens `aurarangecalc` 20, `calc4` ln12, `aurafilter`
42375.

### 36. Server-hit 24 panther pot orange `0x005A9BF0`

Row: pantherpotorange (238).

1. Missile none or no record → return 1. O = owner; none → return 1.
2. r = `sHitPar1`; ≤ 0: k invalid → return 1; r = max(`eval(O, k.calc1,
   k, L)`, 1).
3. (x, y) = position. Zeroed 0x70 record; `0x005A89A0(missile, unit,
   record)`; hit flags |= `HitFlags`, result flags |= `ResultFlags`.
4. `area_damage(game, O, x, y, r, record, 0)` (`missiles.md` §R9.6).
5. Return 1.

### 37. Server-hit 25 panther pot green `0x005AB820`

Row: pantherpotgreen (239); `sHitPar1` 1, `HitSubMissile1`
rancidgascloud.

1. Missile none, no record or `HitSubMissile1` < 0 → return 1. O =
   owner; none → return 1.
2. G = the unit, or the missile when none.
3. `ring8(game, O, G, HitSubMissile1, k, L, max(sHitPar1, 1))`. Return 3.

`ring8(game, owner, origin, class, skill, level, s)` = `0x005AB700`
(ECX game, EDX owner, five stack arguments):

1. No record for class → return 0.
2. Zeroed record, flags 0x1F (position given, target relative,
   velocity given, add loops): owner, origin, class; start = the
   origin's position; skill, level; loops = level − 1 (−1 at level 0);
   velocity field = the created row's `Param1` << 7 (as `bodies.md` §6).
3. For i = 0, s, 2s, … while i < 8: target offset (GX[i], GY[i]);
   create. Return 1.

GX (`0x006E2618`) = 0, 2, 2, 2, 0, −2, −2, −2; GY (`0x006E25F8`) = 2, 2,
0, −2, −2, −2, 0, 2.

### 38. Server-hit 28 Grim Ward scare `0x005ABA10`

Row: grimwardscare (259). Data +0x28 = GUID of the grim ward missile
(set at creation, skills spec).

1. k = skill, L = level; k invalid → return 1. Owner none → return 1.
2. No unit, or the unit is not a monster → return 1.
3. W = the missile with GUID data +0x28 (`0x00552F60(game, 3, GUID)`);
   none → return 1.
4. d = `Param1` + (L − 1) × `Param2` of k (`0x004E6CA0`; 0 when L ≤ 0).
5. Squared distance W → unit (`0x006492A0(W.x, W.y, U.x, U.y)`) ≥ d²
   → return 1.
6. Terror install `0x005DDD00(game, W, unit, k, Param5, Param6)` (the
   ward missile is the source; `Param5` +0x158 and `Param6` +0x15C read
   as they are, `0x004E6C70`, `0x004F4110`; `monsters/ai.md`). Return 1.

Live: Grim Ward `Param1` 3, `Param2` 1 (radius 3 + (L − 1)), `Param5`
10, `Param6` 60.

### 39. Server-do 15 Frozen Orb `0x005AF030`, server-hit 29 `0x005ABB00`

Row: frozenorb (260); `Param1` 1, `Param2` 19, `SubMissile1`
frozenorbbolt, `sHitPar1` 4, `HitSubMissile1` frozenorbnova.

C64[i] = trunc(30 × cos(2πi / 64)) and S64[i] = C64[(i − 16) mod 64],
i = 0…63: 30, 29, 29, 28, 27, 26, 24, 23, 21, 19, 16, 14, 11, 8, 5, 2,
0, … (dumped twice: server-do 15 reads C64 at `0x006E2B78`, S64 at
`0x006E2A78`; server-hit 29 C64 at `0x006E2738`, S64 at `0x006E2638`).

Server-do 15:

1. Missile none, no record or `SubMissile1` < 0 → return 2. O = owner;
   none → return 2.
2. n = max(`Param1`, 1). Elapsed mod n > 0 → return flight.
3. Zeroed record, flags 2 (target relative; start = the origin's
   position): owner O, origin the missile, skill, level, class
   `SubMissile1`.
4. i = |data +0x28 mod 64| (signed remainder, then absolute value).
   Offset (C64[i], S64[i]). Data +0x28 := (i + `Param2`) mod 64
   (signed). Create (after the store).
5. Return flight.

Server-hit 29:

1. Missile none, no record or `HitSubMissile1` < 0 → return 1.
2. Frames left ≠ 0 → return 2 (unit contacts: damage, keep flying; an
   expiry with frames left, e.g. movement stopped, removes it without a
   nova).
3. O = owner; none → return 1.
4. Zeroed record as server-do 15 step 3 with class `HitSubMissile1`. s =
   max(`sHitPar1`, 1). For i = 0, s, 2s, … while i < 64: offset (C64[i],
   S64[i]); create; created → its data +0x28 := C64[i], +0x2C :=
   S64[i].
5. Return 3.

Live: a bolt every frame, each 19 steps (≈ 107°) further round; at
expiry 16 nova pieces (`sHitPar1` 4).

### 40. Server-do 16 Frozen Orb nova `0x005AF170`

Row: frozenorbnova (262); `Param1` 6, `Param2` 2. Data +0x28 / +0x2C =
offset (a, b) from server-hit 29.

1. Missile none or no record → return 2.
2. s = max(`Param2`, 1); e = elapsed. e < `Param1` (signed) and e mod
   s = 0:
   1. a' = (a − b) / 2, b' = (a + b) / 2 (signed, truncating toward 0).
   2. Path target point := (x + a', y + b') (`0x00648AD0`); rebuild
      (`0x00649970(path, missile, 0)`).
   3. Data +0x28 := a', data +0x2C := b'.
3. Return flight.

Each turn rotates the aim 45° and scales its length by 1/√2: the
pieces curl. No draws.

### 41. Server-hit 31 fire head `0x005ABD70`

Row: firehead (277; `CollideKill` 1).

1. Missile none or no record → return 1. O = owner; none, or no unit →
   return 1.
2. Zeroed 0x70 record; v = `elem_roll(game, missile, unit, record)`
   (`missiles.md` §R9.6; `EType` fire, missile seed); the record is not
   used further.
3. O life (stat 6) := min(life + max(v, 0), max life) (`0x00625480`,
   `0x00625D10`, `0x00627260`).
4. Return 3 when `CollideKill` ≠ 0, else 2.

The damage stage then rolls the hit's damage again (`missiles.md`
§R6.2): the heal and the damage are two different rolls.

### 42. Server-hit 32 Cairn Stones `0x005ABE50`

Row: cairnstones (288); `Param4` 38.

1. Missile none, no record, unit given, or data +0x28 ≠ 0 → return 0.
2. Open the portal: `0x005A9930(game, missile, Param4)` (`bodies.md` §1
   step 3; it sets data +0x28 := 1). Return 0.

So the portal also opens when the stones expire before server-do 17
opened it.

### 43. Server-do 18 tower chest spawner `0x005AF300`, server-hit 33 `0x005ABEB0`

Row: towerchestspawner (332); `Param1` 150, `Param2` 2, `Param3` 5,
`Range` 400. Data +0x28 = chest object GUID, +0x2C = 0 at creation
(`world/quests-act1-rest.md` §4 step 5).

Server-do 18:

1. Missile none or no record → return 2. f = frames left.
2. f = 1: C = the object with GUID data +0x28 (`0x00552F60(game, 2,
   GUID)`); C → sound event 0x5C on C (`0x00553380(C, 0x5C, 0)`).
3. f = `Range` (i16) − `Param1`: C as in step 2; C → operate context
   {game, C, operator none, C + 0x20, C's class} and the chest drop
   `0x00585E00` (`items/treasure.md` §4, Q = 4). Then data +0x2C := 1
   (with or without C).
4. Data +0x2C ≠ 0 and f mod max(4 × `Param2`, 1) = 0 (signed):
   1. (px, py) = the missile's coordinates (`0x00620870`). r = `Param3`;
      px += `roll(2r + 1)` − r, then py += `roll(2r + 1)` − r
      (`0x0045C3E0`, missile seed).
   2. Floor drop spot `0x00555DA0(missile room, (px, py), &out, size 1,
      fallback 1)` (`items/treasure.md` §7 step 2); none → skip.
   3. Gold: item request (0x84 bytes, zeroed) {unit the missile, game,
      item level = area level of out's level (`0x0061DCA0(0x0061A1B0(
      room), difficulty game +0x6D, expansion game +0x70)`), item index
      of code `gld ` (`0x00633680`), spawn type 3, out position and
      room, init flags 1, item format game +0x78, quality 2}; create
      (`0x00558D90(game, request, 0)`, `items/generation.md` §3).
5. Return flight.

Server-hit 33: no unit and the missile has a room → refresh the room
(`0x0061AED0(room, 1)`). Return 0.

Live: the chest pops open with 250 frames left, then a gold pile within
±5 sub-tiles every 8 frames.

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| fury / fist arg | {owner, origin, skill, level, n, class} | `0x005AB370`, `0x005ADD20` |
| default filters | fury 0xA783, fist 0xA683, battle cry 0xA783 when its filter ≠ 0 | as named |
| bone wall summon request | flags 0xD, AI state 0, pet max 0 | `0x005AEDA0` |
| bone wall umod | 15 (`partydead`) | `monumod.txt` row 15 |
| panther green ring | GX / GY, radius 2, 8 entries | `0x006E2618` / `0x006E25F8` |
| frozen orb circle | C64 / S64, radius 30, 64 steps | `0x006E2B78` / `0x006E2A78` (do 15), `0x006E2738` / `0x006E2638` (hit 29) |
| tower chest | sound 0x5C at f = 1; chest drop Q 4 at f = `Range` − `Param1`; gold every 4 × `Param2` frames within ±`Param3` | `0x005AF300` |
| grim ward scare | radius `Param1` + (L − 1) × `Param2`; terror `Param5`, `Param6` | `0x005ABA10` |
| missiles.txt | `sHitPar1..2`, `HitSubMissile1`, `SubMissile1`, `HitFlags`, `ResultFlags` | `data/fields.tsv` |
| skills.txt | `aurarangecalc`, `auralencalc` +0x60, `aurafilter` +0x50, `auratargetstate` +0x82, `calc1`, `calc4`, `pettype` +0xBE | `data/fields.tsv` |

Use counts and order: `bodies.md` Constants.

## Randomness

| Body | Re-seed | Draws, in order |
|---|---|---|
| do 12, hit 20, do 13, hit 21, hit 25 | — | none of their own (summon and state helpers: skills / monsters specs) |
| hit 28, do 15, hit 29, do 16, hit 32, hit 33 | — | none of their own |
| hit 31 | — | `elem_roll` (missile seed) |
| do 18 | — | `roll(2r + 1)` × 2 on the missile seed (x first) per gold pile; then the item pipeline's own draws |
| hit 22, hit 24 | — | `missiles.md` §R6.2 rolls on the missile seed; then the damage tail (hit 22) or `area_damage`'s per-unit draws |

Created missiles and monsters draw on their own seeds.

## Edge cases & original bugs

1. Server-hit 21 accepts a state equal to the states count (`>` test)
   and uses 0xA783 only when the skill has a filter (inverted).
2. Server-hit 21's state request carries stat 0 with value 0 instead
   of "no stat" (−1).
3. Server-do 13 returns 0 without flying when the summon class is
   invalid.
4. `ring8` sets flag 8 with loops = level − 1, so a level-0 missile
   passes −1 loops.
5. Server-hit 31 heals the owner with one roll and the damage stage
   rolls the hit again.
6. Server-hit 29 returns 2 (no nova) for any expiry with frames left.
7. Server-do 18 sets data +0x2C := 1 even when the chest is gone, so
   gold still drops.

## Test vectors

| Input | Expected | Source |
|---|---|---|
| lightningfury, n = 3, five hostile units in range | 3 furylightning missiles, aimed at the first three scanned | synthetic, §32 |
| pantherpotgreen hit, `sHitPar1` 1 | 8 rancidgascloud, offsets (0, 2), (2, 2), (2, 0), (2, −2), (0, −2), (−2, −2), (−2, 0), (−2, 2) | live row, §37 |
| bonewallmaker, data +0x2C = 0 | return 2 (removed) | synthetic, §33 |
| fistoftheheavensdelay, struck unit gone | return 0, no bolts | synthetic, §35 |
| frozenorb, data +0x28 = 60, elapsed 5 | bolt at offset (C64[60], S64[60]) = (27, −11); data +0x28 := 15 | live row, §39 |
| frozenorb, data +0x28 = −70 | i = |−70 mod 64| = 6 | synthetic, §39 |
| frozenorbnova (a, b) = (30, 0), elapsed 0, 2, 4 | (15, 15), (0, 15), (−7, 7) | synthetic, §40 |
| towerchestspawner, frames left 250 | chest drop; data +0x2C := 1; 250 mod 8 ≠ 0, no gold | live row, §43 |

## Provenance

- 1.14d `Game.exe`: `0x005AECA0`, `0x005AB370`, `0x005AB2A0`,
  `0x005AEDA0`, `0x005AB500`, `0x005ADD20`, `0x005AB630`,
  `0x005ADCD0`, `0x005A89A0` (zeroes its record), `0x005A9BF0`,
  `0x005AB820`, `0x005AB700`, `0x005ABA10`, `0x004E6CA0`,
  `0x004E6C70`, `0x004F4110`, `0x005AF030`, `0x005ABB00`, `0x005AF170`,
  `0x005ABD70`, `0x005ABE50`, `0x005AF300`, `0x005ABEB0`; tables
  `0x006E25F8`–`0x006E2637`, `0x006E2638`–`0x006E2837`,
  `0x006E2A78`–`0x006E2C77` dumped.
  Table entries checked against `0x0073C768` / `0x0073C840`.
- Live `patch_d2` missiles.txt rows per section; skills.txt Lightning
  Fury, Fist of the Heavens, Battle Cry, Bone Wall; monumod.txt row 15.
- D2MOO 1.10f `MissMode.cpp` bodies of the same names: hints; 1.14d
  differs in server-do 12 (owner required).

## Open questions

1. No recording covers these bodies: record Lightning Fury, Bone Wall,
   Battle Cry, Fist of the Heavens and the panther potions.
2. `0x005B1990(game, P, 0, 9)` on a bone-wall piece: the meaning of
   mode 9 (`monsters/population.md` Open question 7 asks the same for
   mode 8).
