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

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| fury / fist arg | {owner, origin, skill, level, n, class} | `0x005AB370`, `0x005ADD20` |
| default filters | fury 0xA783, fist 0xA683, battle cry 0xA783 when its filter ≠ 0 | as named |
| bone wall summon request | flags 0xD, AI state 0, pet max 0 | `0x005AEDA0` |
| bone wall umod | 15 (`partydead`) | `monumod.txt` row 15 |
| panther green ring | GX / GY, radius 2, 8 entries | `0x006E2618` / `0x006E25F8` |
| missiles.txt | `sHitPar1..2`, `HitSubMissile1`, `SubMissile1`, `HitFlags`, `ResultFlags` | `data/fields.tsv` |
| skills.txt | `aurarangecalc`, `auralencalc` +0x60, `aurafilter` +0x50, `auratargetstate` +0x82, `calc1`, `calc4`, `pettype` +0xBE | `data/fields.tsv` |

Use counts and order: `bodies.md` Constants.

## Randomness

| Body | Re-seed | Draws, in order |
|---|---|---|
| do 12, hit 20, do 13, hit 21, hit 25 | — | none of their own (summon and state helpers: skills / monsters specs) |
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

## Test vectors

| Input | Expected | Source |
|---|---|---|
| lightningfury, n = 3, five hostile units in range | 3 furylightning missiles, aimed at the first three scanned | synthetic, §32 |
| pantherpotgreen hit, `sHitPar1` 1 | 8 rancidgascloud, offsets (0, 2), (2, 2), (2, 0), (2, −2), (0, −2), (−2, −2), (−2, 0), (−2, 2) | live row, §37 |
| bonewallmaker, data +0x2C = 0 | return 2 (removed) | synthetic, §33 |
| fistoftheheavensdelay, struck unit gone | return 0, no bolts | synthetic, §35 |

## Provenance

- 1.14d `Game.exe`: `0x005AECA0`, `0x005AB370`, `0x005AB2A0`,
  `0x005AEDA0`, `0x005AB500`, `0x005ADD20`, `0x005AB630`,
  `0x005ADCD0`, `0x005A89A0` (zeroes its record), `0x005A9BF0`,
  `0x005AB820`, `0x005AB700`; table `0x006E25F8`–`0x006E2637` dumped.
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
