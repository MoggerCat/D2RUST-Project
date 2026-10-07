# Spec: Missiles — server-do and server-hit bodies (part 3)

- **Status:** draft: every body here read from the 1.14d `Game.exe`
  disassembly (addresses per section, register and stack arguments
  checked with `tools/ghidra/disasm.py fn`). No recording covers them
  (Open question 1); implemented 2026-10-06 in
  `d2-sim::missiles::bodies_ext2`, unverified.
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
| Summary | 65–78 |
| Inputs | 79–86 |
| Outputs / state changes | 87–92 |
| Rules | 93–94 |
|   31. Server-do 12 Diablo wall maker `0x005AECA0` | 95–105 |
|   32. Server-hit 20 Lightning Fury `0x005AB370` | 106–131 |
|   33. Server-do 13 Bone Wall maker `0x005AEDA0` | 132–158 |
|   34. Server-hit 21 Battle Cry `0x005AB500` | 159–177 |
|   35. Server-hit 22 Fist of the Heavens delay `0x005ADD20` | 178–205 |
|   36. Server-hit 24 panther pot orange `0x005A9BF0` | 206–217 |
|   37. Server-hit 25 panther pot green `0x005AB820` | 218–241 |
|   38. Server-hit 28 Grim Ward scare `0x005ABA10` | 242–260 |
|   39. Server-do 15 Frozen Orb `0x005AF030`, server-hit 29 `0x005ABB00` | 261–299 |
|   40. Server-do 16 Frozen Orb nova `0x005AF170` | 300–316 |
|   41. Server-hit 31 fire head `0x005ABD70` | 317–333 |
|   42. Server-hit 32 Cairn Stones `0x005ABE50` | 334–344 |
|   43. Server-do 18 tower chest spawner `0x005AF300`, server-hit 33 `0x005ABEB0` | 345–381 |
|   44. Server-do 19 Radament death `0x005B0940` | 382–455 |
|   45. Server-hit 35 orb mist `0x005ABEE0` | 456–470 |
|   46. Server-do 20 blade creeper `0x005AF540`, server-do 21 Distraction `0x005AF590`, server-hit 37 `0x005AC020` | 471–498 |
|   47. Server-hit 39 imp spawn monsters `0x005AC1D0` | 499–525 |
|   48. Server-hit 40 catapult spike ball `0x005AC250` | 526–554 |
|   49. Server-hit 43 Healing Vortex `0x005AC350` | 555–569 |
|   50. Server-hit 47 Molten Boulder `0x005AC550` | 570–591 |
|   51. Server-hit 48 Molten Boulder emerge `0x005AC6D0` | 592–600 |
|   52. Server-hit 50 plague vines trail `0x005AC800` | 601–611 |
|   53. Server-do 27 Tornado `0x005AFA30` | 612–630 |
|   54. Server-hit 51 volcano debris `0x005AC870` | 631–641 |
|   55. Server-do 29 recycler delay `0x005AFD70`, server-do 33 vine recycler delay `0x005AFEC0` | 642–658 |
|   56. Server-do 30 rabies plague `0x005B0010`, server-hit 53 rabies contagion `0x005ACA50` | 659–694 |
|   57. Server-do 32 Tiger Fury `0x005B03E0` | 695–707 |
|   58. Server-hit 54 Baal spawn monsters `0x005ACAF0` | 708–719 |
|   59. Server-hit 55 Baal inferno `0x005ACB60` | 720–730 |
|   60. Server-do 36 `0x005B0A40`, server-hit 57 `0x005AD970` Baal FX control | 731–746 |
|   61. Server-hit 59 Baal taunt poison control `0x005ACF20` | 747–758 |
|   62. Unused bodies: server-do 37 `0x005B0AA0`, server-hit 5 `0x005ABC40`, 6 `0x005AA1C0`, 11 `0x005B0870`, 23 `0x005ACFC0` | 759–785 |
| Constants & data dependencies | 786–811 |
| Randomness | 812–831 |
| Edge cases & original bugs | 832–858 |
| Test vectors | 859–878 |
| Provenance | 879–910 |
| Open questions | 911–944 |
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
   (`missiles.md` §R9.6; `EType` fire, missile seed): v is the rolled
   fire amount (8.8), not the element type; the record is not used
   further.
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
      fallback 1)` (`items/treasure.md` §7 step 2); none → skip. With
      no room every cell lookup of the search (`0x00463740`) gives
      none, so the spot is none: no gold.
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

### 44. Server-do 19 Radament death `0x005B0940`

Row: radamentdeath (347); `Range` 400.

1. f = frames left. 2 ≤ f ≤ 24: `corpse_effect(game, 0x3002, x, y,
   missile, skill 124, level 1, cb 0x005AD8F0)`. f = 1: the same with cb
   `0x005AD910`. (No record check.)
2. Return flight (tail jump).

`corpse_effect(game, flags, x, y, unit, skill, level, cb)` = `0x0056DDE0`
(ECX game, EDX flags; D2MOO `sub_6FD14BD0`) builds a request {flags,
unit, skill, x, y, radius 0, level, 0, cb} and runs `0x0056DCC0`:

1. Request or cb null → fatal assertion.
2. r = radius, or (0) skill `Param1` + (level − 1) × `Param2`
   (`0x004E6CA0`). Redemption (124): `Param1` 16 → r = 16.
3. Room = the room containing (x, y), searched from the unit's room
   (`0x00463740`); none → nothing.
4. Unit find around (x, y) with radius r and flags `flags` over that
   room's neighbourhood: the unit find below, with the default filter
   (no callback) and filter record {flags, unit, x, y, r}.
5. For each found unit U in found order: `cb(ECX game, EDX unit, U,
   level, 0)`.

Both callbacks call `0x005D0C40(game, missile, U, 124, level, last)`
(D2MOO `SKILLS_ApplyRedemptionEffect`; skills spec, not yet written,
Open question 4) with last = 0 for `0x005AD8F0` and 1 for `0x005AD910`.

**Unit find** (`0x0065A950` init, `0x0065AC70` collect, `0x0065AA40`
default filter, `0x0065AA00` free; also used by `monsters/ai-bodies-3.md`
with its own callback). Finder: room R, centre (x, y), radius r, flags F,
callback cb (or none), filter argument A; a result array of capacity 15
that grows by 15 whenever it fills (no limit).

1. R none → 0 found.
2. Rooms: if the square x ± r, y ± r lies strictly inside R's subtile
   rectangle (active room +0x4C x, y, w, h: x − r > rx, y − r > ry,
   x + r < rx + w, y + r < ry + h; `0x0065A6B0`) → R alone; else R's
   adjacency array (active room +0x00, count +0x24; `drlg/rooms.md`
   §1), in its order.
3. Per room: F has 0x2000 and the room is in a town (its level is 1,
   40, 75, 103 or 109; `0x0061AB00` → `0x006426A0`) → skip. Then the
   overlap test `0x0065A710`, which rejects only when x + r < rx and
   x − r > rx + w, or y + r < ry and y − r > ry + h: for r ≥ 0 it never
   rejects (original bug; distance is left to the filter).
4. Per unit of the room, from the room's first unit (+0x74) along the
   next-in-room link (unit +0xE8): accepted by cb(ECX unit, EDX A) when
   cb is set, else by the default filter → appended. **Found order** =
   room order, then each room's unit-list order.

Default filter (A = the filter record: +0x00 flags F, +0x08 unit S,
+0x0C x, +0x10 y, +0x14 r, +0x18 limit, +0x1C accepted count, +0x20
coordinate list, +0x24 extra test):

1. A none → reject. F has 0x40 and limit ≤ accepted → reject.
2. Position of U (objects, items, tiles: static path; others: dynamic
   path); (ux − x)² + (uy − y)² > r² → reject (distance ≤ r passes).
3. By U's type: player (0) needs F & 1; without F & 0x1000 mode 17
   (dead) or 0 (death) rejects, with it only mode 17 passes; U = S
   rejects. Monster (1) needs F & 2; without 0x1000 mode 12 (dead) or 0
   (death) rejects, with it only mode 12 passes; F & 4 → U must be
   undead (`0x0063E990`). Object (2) needs F & 0x10. Missile (3) needs
   F & 8, a missiles.txt record, and its `Explosion` flag (record +0x04
   bit 1) clear. Item (4) needs F & 0x20. Tile (5) and others reject.
4. F & 0x80 → unit flag 0x4 (+0xC4) required; F & 0x400 → unit flag
   0x8 required. F & 0x100 → U's room in a town rejects. F & 0x200 and
   U has a room → for each point of the coordinate list (`0x0066A5D0`)
   the collision at it (`0x0064CB30(room, px, py, mask 4)`) equal to 4
   rejects. F & 0x800 → the extra test (ECX U, EDX A) non-zero rejects.
5. Accept: accepted count += 1.

Corpse effect (flags 0x3002): monsters in mode 12 only, outside towns,
within r of (x, y), in found order.

### 45. Server-hit 35 orb mist `0x005ABEE0`

Row: orbmist (368). Data +0x28 = GUID of an object (set by its
creator).

1. Unit given → return 0.
2. B = the object with GUID data +0x28 (`0x00552F60(game, 2, GUID)`);
   none → return 0.
3. B's room → refresh it (`0x0061AED0(room, 1)`).
4. B's mode (unit +0x10) = 0: mode := 1 (`0x00624690(B, 1)`); B's
   objects record (`0x00640E90(class)`) → timer type 1 (end of
   animation) on B at F + (`FrameCnt1` (+0xDC) >> 8) (`0x005417D0(game,
   B, 1, ·, 0, 0)`).
5. Return 1.

### 46. Server-do 20 blade creeper `0x005AF540`, server-do 21 Distraction `0x005AF590`, server-hit 37 `0x005AC020`

Rows: blade creeper (392, server-do 20, server-hit 37), distraction
(393, server-do 21; `SubMissile1` distraction fog).

Server-do 20:

1. O = owner. O none or dead (`0x005541B0` ≠ 0) → hit handler
   `0x005ADF10(game, missile, no unit, a4 = 1)` (`missiles.md` §R5;
   result ignored); return 2.
2. `0x005A99E0` (EBX missile): O and the missile's path exist →
   teleport the path to O's position in O's room (`0x00650BE0(path,
   missile, O room, O.x, O.y)`, `sim/path-placement.md` §6).
3. Frames left := 10 (`0x0064A330`).
4. Flight, result ignored. Return 1.

The missile sits on its owner and never expires while the owner lives;
flight still runs its collision on the owner's spot every frame.

Server-do 21:

1. Missile none or no record → return 2.
2. `SubMissile1` ≠ 0 and the new-step flag: `0x0056EDE0(game, owner,
   skill, level, SubMissile1, x, y)` (`skills/bodies.md` §6.13).
3. Continue as server-do 20 (tail jump).

Server-hit 37: unit given → 2 (damage, keep), else 0.

### 47. Server-hit 39 imp spawn monsters `0x005AC1D0`

Row: imp spawn monsters (409).

1. `spawn_for_level(game, missile room, x, y)` = `0x005B3570`. Return 1
   whatever it did.

`spawn_for_level` (ECX game, EDX room, stack x, y):

1. L = levels record of the room's level (`0x0061DB70(0x0061A1B0(room))`).
   n = `mon` count (+0x33); n = 0 → fatal assertion. No room: the level
   id reads as 0, which has no levels record, so the +0x33 read goes
   through null (fatal). Unreachable for a live missile: creation fails
   without a room (`missiles.md` §R2.3 step 3) and the missile's room is
   always set.
2. i = `roll(n)` on the **room seed** (the active room's seed, room
   +0x6C, `drlg/rooms.md` §1 active-room layout; `0x0045C3E0`).
3. For up to n steps: i := i + 1, 0 when it reaches n; c = mon[i]
   (+0x36 + 2i, i16); c's monstats record (an invalid c reads through a
   null record: fatal) has `isSpawn` (flags +0x0C bit 0) → stop.
4. c = −1 → return 0: a dead check, since c = −1 already failed in
   step 3 (fatal); never reached. All n entries without `isSpawn` →
   fatal assertion.
5. Create request (`monsters/init.md` §2) {game, room, coord list 0,
   class c, mode 1, GUID 0, x, y, spread 5, flags 0} → `0x005B2A00`;
   return 1 when created, else 0.

### 48. Server-hit 40 catapult spike ball `0x005AC250`

Row: catapult spike ball (411); `sHitPar1` 0, `sHitPar2` 0,
`HitSubMissile1` catapult spike in air.

1. Missile none, no record or `HitSubMissile1` < 0 → return 1. O =
   owner; none → return 1.
2. L = level. n = `sHitPar1` + (L − 1) × `sHitPar2`; n ≤ 0: k's record
   (`0x0045C4B0`) none → return 1; n = max(`eval(O, k.calc4, k, L)`, 1).
3. `scatter_at_target(game, missile, O, HitSubMissile1, n, n / 4, k, L)`
   (`0x005D5BF0`; n / 4 signed, truncating; owner fetched again). Return
   1.

`scatter_at_target(game, U, owner, class, n, r, skill, level)` =
`0x005D5BF0` (ECX game, EDX U; also used by a skill function at
`0x005D5DF4`):

1. (tx, ty) = U's target position (`0x0056D2C0`, `skills/bodies.md`
   §2.4: the target unit's position, else the path target point); fails
   → return.
2. **Re-seed U's seed**: `init_low(tx)` (`0x00650E40`).
3. Zeroed record, flags 0x420 (target absolute, frames from distance):
   owner, origin U, class, skill, level.
4. n ≤ 1 or r < 2: target (tx, ty); create once.
5. Else n times: px = tx − r + `roll(2r)`, then py = ty − r + `roll(2r)`
   (`0x0045C3E0`, U's seed); squared distance from U's position to (px,
   py) (`0x006492A0`) ≥ 4 → target (px, py), create; else nothing for
   this one.

### 49. Server-hit 43 Healing Vortex `0x005AC350`

Row: healing vortex (425).

1. Missile none, no record or no unit → return 1. k = skill, L =
   level; k invalid → return 1.
2. O = owner (may be none). lo = `phys_min(O, k, L, 1)`, hi =
   `phys_max(O, k, L, 1)` (`0x00647BC0`, `0x00647D00`,
   `skills/levels.md` §3.3; lo first).
3. v = lo + `roll(hi − lo)` (`0x0045C3E0`, missile seed).
4. life = unit stat 6, max = max life (`0x00625D10`). life ≠ max and
   `ProgOverlay` > 0 → overlay on the unit (`0x00621E40`).
5. Unit life := min(life + v, max).
6. Return 1 when `CollideKill` ≠ 0, else 0.

### 50. Server-hit 47 Molten Boulder `0x005AC550`

Row: moltenboulder (452); `sHitPar1` 0, `sHitPar2` 1, `HitSubMissile1`
moltenboulderfirepath.

1. Missile none or no record → return 1. O = owner; none → return 1.
2. Unit given: not a monster → return 2; a monster whose class lacks
   the monstats2 `large` flag (`0x004638A0(class, 11)`) → return 2. (No
   unit, i.e. the expiry, goes on; D2MOO 1.10f returns 2 there.)
3. (x, y) = position; c = 1. Zeroed 0x70 record; `0x005A89A0(missile,
   unit, record)`.
4. r = `sHitPar1`; ≤ 0: k invalid → return 1; r = max(`eval(O,
   k.aurarangecalc, k, L)`, 1).
5. Flags as `bodies.md` §8 step 4; `area_damage(game, O, x, y, r,
   record, 0)` = 0 → c = 3 (never).
6. `scatter(game, missile, HitSubMissile1, r, max(sHitPar2, 1))`
   (`bodies.md` §9: 18 points, range field r).
7. Return c.

So the boulder rolls through small monsters (damage, keep) and bursts
on large ones and at its end.

### 51. Server-hit 48 Molten Boulder emerge `0x005AC6D0`

Row: moltenboulderemerge (453); `HitSubMissile1` moltenboulder.

1. Missile none, no record or `HitSubMissile1` < 0 → return 1.
2. Zeroed record, flags 0x21: owner (may be none), start = the
   missile's position, skill, level, class `HitSubMissile1`, target =
   the path target point (path +0x10, +0x12). Create. Return 1.

### 52. Server-hit 50 plague vines trail `0x005AC800`

Row: plague vines trail (475); `sHitPar1` 15.

1. Missile none or no record → return 1.
2. Unit given and elapsed < total frames (`0x0064A300`) − `sHitPar1`
   (signed) → return 2.
3. Else return 0.

The trail damages only while more than `sHitPar1` frames are left.

### 53. Server-do 27 Tornado `0x005AFA30`

Row: tornado (478); `Param1` 0, `Param2` 0.

1. Missile none or no record → return 2. k = skill; k invalid → return
   2.
2. O = owner (may be none), L = level, e = elapsed.
3. n = `Param1`; ≤ 0 → max(`eval(O, k.calc4, k, L)`, 1).
4. e mod n = 0 (signed):
   1. Damage record: `0x005A89A0(missile, no unit, record)` (zeroes it;
      §R6.2 rolls on the missile seed).
   2. r = `Param2`; ≤ 0 → max(`eval(O, k.aurarangecalc, k, L)`, 1).
   3. Hit flags |= `HitFlags`, result flags |= `ResultFlags`.
   4. `area_damage(game, missile, 0, 0, r, record, k.aurafilter)`
      (`missiles.md` §R9.6): the **missile** is the source (the per-unit
      callback turns a missile attacker into its owner), x = y = 0 makes
      `scan_unit` use the missile's position, filter 0 → 0x8583.
5. Return flight.

### 54. Server-hit 51 volcano debris `0x005AC870`

Row: volcano debris 2 (481); `HitSubMissile1` volcano small fire.

1. Missile none, no record or `HitSubMissile1` < 0 → return 1.
2. Zeroed record, flags 0 (start at the origin, target = start): owner
   (may be none), origin the missile, skill, level; class
   `HitSubMissile1`; create. `HitSubMissile2` ≥ 0 → the same record with
   that class; create. `HitSubMissile3` ≥ 0 → likewise.
3. Return 1.

### 55. Server-do 29 recycler delay `0x005AFD70`, server-do 33 vine recycler delay `0x005AFEC0`

Rows: recycler delay (498, server-do 29), vine recycler delay (540,
server-do 33); `Param1` 45, `Range` 47.

1. Missile none or no record → return 2. k invalid → return 2.
2. O = owner; none or dead → return flight.
3. Elapsed = `Param1` (signed equality):
   1. Server-do 29: v = O life (stat 6) >> 8, m = max life (`0x00625D10`)
      >> 8. Server-do 33: v = O mana (stat 8) >> 8, m = max mana
      (`0x00625D60`) >> 8 (arithmetic shifts).
   2. v < m: p = `eval(O, k.calc1, k, L)`; v' = min(v + `pct(m, p,
      100)`, m) (`0x00483360`); the stat := v' << 8. `ProgOverlay` in 1 …
      overlay count − 1 (data tables +0xBC0) → overlay on O
      (`0x00621E40(O, ProgOverlay, 0)`).
4. Return flight.

### 56. Server-do 30 rabies plague `0x005B0010`, server-hit 53 rabies contagion `0x005ACA50`

Rows: rabiesplague (515, server-do 30; `Param1` 4, `Param2` 7,
`SubMissile1` rabiescontagion), rabiescontagion (516, server-hit 53).
Plague data +0x28 / +0x2C = (type, GUID) of the infected unit U (set by
the skill).

Server-do 30:

1. Missile none, no record or `SubMissile1` < 0 → return 2. k invalid →
   return 2.
2. U = the unit (data +0x28 type, +0x2C GUID) (`0x00552F60`). O = owner.
3. U none, O none, O dead, or frames left < 0 → hit handler
   `0x005ADF10(game, missile, no unit, a4 = 1)`; return 2.
4. Follow the owner (`0x005A99E0`, §46 step 2).
5. Elapsed mod max(`Param1`, 1) = 0 (signed):
   1. Zeroed record, flags 2 (target relative; start = the origin's
      position): class `SubMissile1`, **owner U**, origin the missile,
      skill k, level L.
   2. r = `Param2`; dx = `roll(2r + 1)` − r, then dy = `roll(2r + 1)` −
      r, on **U's** seed (U + 0x20, `0x0045C390`). Create → N.
   3. s = k.`auratargetstate` (i16) > 0: Lst = O's list of s
      (`0x006256B0`); N and Lst → N data +0x28 := Lst's expiry frame
      (list +0x18, `0x00626090`).
6. Flight, result ignored. Return 1.

Server-hit 53:

1. O = owner; none, or no unit → return 1.
2. k = skill, L = level; t = data +0x28 − F (frames the source state
   has left).
3. t < 10 → return 1. t > `elem_len(O, k, L, 1)` (`0x00644F20`,
   `skills/levels.md` §3.2) → return 1.
4. `0x005C7DB0(game, O, unit, t, k, L)` (D2MOO `sub_6FCFEDD0`, the
   rabies poison; skills spec, Open question 5). Return 2.

### 57. Server-do 32 Tiger Fury `0x005B03E0`

Row: tigerfury (520); `Param1` 5, `SubMissile1` tigerfurytrail.

1. Missile none, no record or `SubMissile1` < 0 → return 2.
2. New-step flag: zeroed record, flags 1 (position given), owner,
   origin the missile, skill, level, class `SubMissile1`. **The position
   is never written**: start (0, 0), so the creation fails at its room
   lookup (`missiles.md` §R2.3 step 3) and no trail appears (Open
   question 6).
3. Return server-do 7 (`0x005AE780`, `missiles.md` §R9.5 item 4: homing
   on `Param1`).

### 58. Server-hit 54 Baal spawn monsters `0x005ACAF0`

Row: baal spawn monsters (550).

1. Unit given → return 1.
2. O = owner; none → return 1. E = O's skill entry for the missile's
   skill (`0x006439F0`); none → return 1.
3. `0x0054E600(game, missile room, class = E param 1 (+0x18,
   `0x006444A0`), x, y, mode 1)` (`monsters/population.md` §11.2) at the
   missile's position.
4. Return 1.

### 59. Server-hit 55 Baal inferno `0x005ACB60`

Row: baal inferno (554); `sHitPar1` 50.

1. No unit or the unit is not a player → return 2.
2. Missile none, no record or `sHitPar1` ≤ 0 → return 1.
3. m = the unit's mana (stat 8); m ≤ 0 → return 1.
4. p = `sHitPar1` clamped to 1…100; loss = min(max(`pct(m, p, 100)`,
   1), m) (`0x00483360`); mana := m − loss.
5. Return 2.

### 60. Server-do 36 `0x005B0A40`, server-hit 57 `0x005AD970` Baal FX control

Row: baalfx control (625); `Range` 650.

`tyrael(game, missile)`: data +0x28 := 1; R = the missile's room; the
quest test `0x005444B0(game, 36)` (the game's quest list, game +0x10F4:
no entry with id 36 → true; else its byte +9 = 1) → `0x0058E920(game,
R, missile)` (D2MOO `ACT5Q6_SpawnTyrael`; quests spec, Open question 7);
then, whatever the test gave, refresh R (`0x0061AED0(R, 1)`): the
refresh is outside the test branch in both callers.

Server-do 36: data +0x28 = 0 and frames left ≤ 100 → `tyrael`. Return
flight (tail jump).

Server-hit 57: no unit and data +0x28 = 0 → `tyrael`. Return 0.

### 61. Server-hit 59 Baal taunt poison control `0x005ACF20`

Row: baal taunt poison control (655); `sHitPar1` 2, `HitSubMissile1`
baal taunt poison.

1. Missile none, no record or `HitSubMissile1` < 0 → return 1. O =
   owner; none → return 1.
2. `ring(game, O, missile, HitSubMissile1, k, L, a = max(sHitPar1, 1),
   b = 2, loops 0)` (`bodies.md` §6). Return 1.

Live: 8 pieces at the even ring directions, then 8 at 1, 3, …, 15.

### 62. Unused bodies: server-do 37 `0x005B0AA0`, server-hit 5 `0x005ABC40`, 6 `0x005AA1C0`, 11 `0x005B0870`, 23 `0x005ACFC0`

No live row uses them; read for completeness.

Server-do 37: missile none or no record → 2. For j = 1, 2, 3:
`SubMissile_j` > 0 (i16) and elapsed = `Param_j` → `0x0056EDE0(game,
owner, skill, level, SubMissile_j, x, y)`. Return flight.

Server-hit 5: as server-hit 29 (§39) without the frames-left test, with
the circle C64 at `0x006E2938` and S64 at `0x006E2838` (same values).
Return 3; 1 on a missing record, `HitSubMissile1` < 0 or no owner.

Server-hit 6: missile none or no record → 1. c = `sHitPar1` in 0 …
monstats count − 1: mode = `sHitPar2` when in 0…15, else 1;
`0x0054E600(game, missile room, c, x, y, mode)`. Return 1.

Server-hit 11: missile none or no record → 1. Zeroed record, flags 0:
owner, origin the missile, skill, level. For i = 1…4:
`HitSubMissile_i` > 0 → create; created and `sHitPar1` > 0 → hit
handler `0x005ADF10(game, new, unit, a4 = 1)`. Return 1 (server-hit 4
returns 3, `missiles.md` §R9.6 item 2).

Server-hit 23: missile none or no record → 1. k > 0, L > 0 and an
owner: unit given → the missile's path target unit := it
(`0x00620C10`); skill server-do `sHitPar1` with the missile as the
caster (`0x0056D810`, §15 step 4). Return 1.

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
| radament corpse effect | flags 0x3002, skill 124 (Redemption), level 1, radius `Param1` 16 | `0x005B0940`, `0x0056DCC0` |
| creeper / distraction hold | frames left := 10 every run | `0x005AF540` |
| imp spawn request | mode 1, spread 5, flags 0 | `0x005B3570` |
| spike scatter | n targets within ±n/4, kept when d² ≥ 4 | `0x005D5BF0` |
| molten boulder burst | monstats2 `large` (flag 11) or expiry | `0x005AC550` |
| recycler heal / mana | at elapsed `Param1`: + pct(max, `calc1`) | `0x005AFD70`, `0x005AFEC0` |
| rabies | contagion every `Param1` frames within ±`Param2` (U's seed); contagion applies while 10 ≤ t ≤ `elem_len` | `0x005B0010`, `0x005ACA50` |
| baal inferno mana burn | clamp(`sHitPar1`, 1, 100) % of mana, at least 1 | `0x005ACB60` |
| baal fx | Tyrael at frames left ≤ 100 (or expiry), quest id 36 | `0x005B0A40`, `0x005AD970` |
| missiles.txt | `sHitPar1..2`, `HitSubMissile1`, `SubMissile1`, `HitFlags`, `ResultFlags` | `data/fields.tsv` |
| skills.txt | `aurarangecalc`, `auralencalc` +0x60, `aurafilter` +0x50, `auratargetstate` +0x82, `calc1`, `calc4`, `pettype` +0xBE | `data/fields.tsv` |

Use counts and order: `bodies.md` Constants.

## Randomness

| Body | Re-seed | Draws, in order |
|---|---|---|
| do 12, hit 20, do 13, hit 21, hit 25 | — | none of their own (summon and state helpers: skills / monsters specs) |
| hit 28, do 15, hit 29, do 16, hit 32, hit 33 | — | none of their own |
| hit 31 | — | `elem_roll` (missile seed) |
| do 19, hit 35, do 20, do 21, hit 37, hit 48 | — | none of their own (do 19's corpse callbacks: skills spec) |
| hit 39 | — | `roll(n)` on the **room** seed (room +0x6C), then monster creation's own draws |
| hit 40 | missile seed := `init_low(target x)` | `roll(2r)` × 2 per target (x first), only when n > 1 and r ≥ 2 |
| hit 43 | — | `roll(hi − lo)` on the missile seed |
| hit 47 | — | §R6.2 rolls; `area_damage` per unit |
| do 27 | — | §R6.2 rolls on the missile seed each pulse; `area_damage` per unit |
| do 30 | — | `roll(2r + 1)` × 2 on the **infected unit's** seed per contagion (x first) |
| hit 50, hit 51, do 29, do 33, hit 53, do 32, hit 54, hit 55, do 36, hit 57, hit 59, do 37, hit 5, hit 6, hit 11, hit 23 | — | none of their own (seams named in each section may draw) |
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
8. Server-do 20 / 21 ignore the flight result and return 1: the missile
   is never removed by flight while its owner lives.
9. `spawn_for_level` (§47) dies on a level with no `mon` entries or none
   with `isSpawn`, and on an invalid class in the list.
10. Server-hit 47 bursts at expiry in 1.14d (D2MOO 1.10f: returns 2
    without a burst).
11. Server-do 32 never sets the sub-missile's position, so Tiger Fury's
    trail is never created.
12. Server-do 30's contagions belong to the infected unit, not to the
    plague's owner, and draw on that unit's seed.
13. Server-do 27 makes the tornado itself the damage source.

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
| scatter_at_target n = 8, r = 2, target (50, 50) | `init_low(50)`; 8 × two draws `roll(4)`; points in 48…51 × 48…51 | synthetic, §48 |
| catapult spike ball, `calc4` 5 | n = 5, r = 1 → one spike at the target, no draw | synthetic, §48 |
| healing vortex, lo = hi | v = lo, no draw | synthetic, §49 |
| baal inferno, mana 1000 (8.8: 256 000), `sHitPar1` 50 | loss 128 000 → mana 128 000 | synthetic, §59 |
| baal inferno, mana 1 | loss = min(max(0, 1), 1) = 1 → 0 | synthetic, §59 |
| recycler delay, life 50 / 100, `calc1` 20, elapsed 45 | life 70 (<< 8 stored) | synthetic, §55 |
| plague vines trail, total 100, elapsed 84 | 84 < 85 → return 2; elapsed 85 → 0 | live row, §52 |
| towerchestspawner, frames left 250 | chest drop; data +0x2C := 1; 250 mod 8 ≠ 0, no gold | live row, §43 |

## Provenance

- 1.14d `Game.exe`: `0x005AECA0`, `0x005AB370`, `0x005AB2A0`,
  `0x005AEDA0`, `0x005AB500`, `0x005ADD20`, `0x005AB630`,
  `0x005ADCD0`, `0x005A89A0` (zeroes its record), `0x005A9BF0`,
  `0x005AB820`, `0x005AB700`, `0x005ABA10`, `0x004E6CA0`,
  `0x004E6C70`, `0x004F4110`, `0x005AF030`, `0x005ABB00`, `0x005AF170`,
  `0x005ABD70`, `0x005ABE50`, `0x005AF300`, `0x005ABEB0`; tables
  `0x006E25F8`–`0x006E2637`, `0x006E2638`–`0x006E2837`,
  `0x006E2A78`–`0x006E2C77` dumped; `0x005B0940`, `0x0056DDE0`,
  `0x0056DCC0`, `0x005AD8F0`, `0x005AD910`, `0x005ABEE0`, `0x005AF540`,
  `0x005A99E0`, `0x005AF590`, `0x005AC020`, `0x005AC1D0`, `0x005B3570`,
  `0x005AC250`, `0x005D5BF0` (callers `0x005AC32F`, `0x005D5DF4`),
  `0x005AC350`, `0x005AC550`, `0x004638A0`, `0x005AC6D0`, `0x005AC800`,
  `0x005AFA30`, `0x005AC870`, `0x005AFD70`, `0x005AFEC0`, `0x005B0010`,
  `0x00626090`, `0x005ACA50`, `0x005B03E0`, `0x005ACAF0`, `0x006444A0`,
  `0x005ACB60`, `0x005B0A40`, `0x005AD970`, `0x005444B0`, `0x005ACF20`,
  `0x005B0AA0`, `0x005ABC40`, `0x005AA1C0`, `0x005B0870`, `0x005ACFC0`;
  table `0x006E2838`–`0x006E2A37` dumped.
  Table entries checked against `0x0073C768` / `0x0073C840`.
- Unit find: `0x0056DCC0`, `0x0065A950`, `0x0065AC70`, `0x0065A6B0`,
  `0x0065A710` (register arguments read in the disassembly),
  `0x00619730`, `0x00619790`, `0x0065A900`, `0x0065AA40`, `0x0061AB00`,
  `0x006426A0`, `0x00451F30`, `0x0046ACE0`; mask `0x006CE26C` = 2
  (read from `Game.exe`). `spawn_for_level` null paths: `0x0061A1B0`,
  `0x0066BAB0`, `0x0061DB70`; floor drop with no room: `0x00555DA0`,
  `0x0064DEA0`, `0x00463740` (null room → none).
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
3. *Answered* (`impl-missile-bodies-2` Q6): the unit find is now
   specified under §44 (rooms, found order, default filter); flags
   0x3002 = dead monsters (mode 12) outside towns within r.
   `monsters/ai-bodies-3.md` Open question 3 can link it.
4. `0x005D0C40` (D2MOO `SKILLS_ApplyRedemptionEffect`) has no spec: the
   skills spec should own it (Redemption's per-corpse effect; called
   with last = 1 on the final frame).
5. `0x005C7DB0` (rabies poison from a contagion, D2MOO `sub_6FCFEDD0`)
   has no spec; the skills spec should own it.
6. Server-do 32 (§57): confirm with a Tiger Strike / Royal Strike
   recording that no tigerfurytrail missile is ever created.
7. `0x0058E920` (Tyrael's spawn in the Worldstone Chamber) and the quest
   test `0x005444B0`: the quests spec should own both.
8. *Answered* (`impl-missile-bodies-2` Q2): `elem_roll` returns the
   rolled amount (`missiles.md` §R9.6 return list), so the fire head
   heals by the fire roll (§41 step 2).
9. *Answered* (`impl-missile-bodies-2` Q4): `tyrael` refreshes the room
   after the quest-test branch, whatever the test gave (§60;
   `0x005AD970`, `0x005B0A40`).
10. *Answered* (`impl-missile-bodies-2` Q5): in `spawn_for_level` an
    invalid class is a null read inside the search, so the −1 test is
    dead; no room is fatal too but unreachable (§47 steps 1, 4). The
    tower chest's floor drop with no room finds no spot (§43 step 4.2).
11. *Answered* (`impl-missile-bodies-2` Q6): the room seed is the
    active room's seed (+0x6C, `drlg/rooms.md` §1), which the DRLG
    active-room code already provides; the unit find is §44.
