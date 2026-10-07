# Spec: Missiles — server-do and server-hit bodies (continued)

- **Status:** draft: every body here (§1 onward; the use-count table
  under Constants lists them) read from the 1.14d `Game.exe` disassembly (addresses per section, register
  and stack arguments checked in `all.asm`). No recording covers them
  (Open question 1); implemented 2026-10-06 in
  `d2-sim::missiles::bodies_ext`, unverified.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::missiles` (server-do / server-hit bodies,
  beside those of `missiles.md` §R9.5, §R9.6)
- **Related specs:** `missiles/missiles.md` (owner of the parameter
  record §R2.1, creation §R2.3, dispatch §R3, default flight §R4, hit
  handler §R5, the tables and TSV columns §R9.1–§R9.2, the seeded
  sub-missile helper §R9.3, the re-seed list §R9.4 and the conventions
  of §R9.5 / §R9.6); `missiles/srvdo.tsv`, `missiles/srvhit.tsv`
  (catalogue rows flipped to `spec'd-here` link here); `sim/rng.md`
  (`init_low`, `get_lo`, `roll`); `skills/bodies.md` (`eval`);
  `sim/pathing.md` §3 (path rebuild); `world/quests.md` (portal object
  creation `0x0056D130`); `missiles/bodies-2.md` (continuation, §31 on).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 66–88 |
| Inputs | 89–96 |
| Outputs / state changes | 97–101 |
| Rules | 102–103 |
|   1. Server-do 17 Cairn Stones `0x005AF240` | 104–123 |
|   2. Server-do 28 Volcano `0x005AFB80` | 124–146 |
|   3. Server-do 34 Baal taunt control `0x005B04A0` | 147–169 |
|   4. Server-do 35 Royal Strike chaos ice `0x005B0640` | 170–190 |
|   5. Server-hit 58 Baal taunt lightning control `0x005ACDF0` | 191–205 |
|   6. Server-hit 2 Plague Javelin, gas potions `0x005A9D80` | 206–246 |
|   7. Server-do 6 Fire Wall maker, Molten Boulder `0x005AE680` | 247–264 |
|   8. Server-hit 3 potions, bomb on ground `0x005A9F90` and server-hit 44 Exploding / Ice Javelin `0x005A9E10` | 265–295 |
|   9. Server-hit 14 Meteor center, catapult meteor, royal strike meteor `0x005AABB0` | 296–337 |
|   10. Server-hit 36 missile in air `0x005ABF70` | 338–354 |
|   11. Server-hit 10 Guided Arrow, Bone Spirit `0x005AA650` | 355–397 |
|   12. Server-hit 16 Spider goo, vines trail, vines wither `0x005AAE10` | 398–426 |
|   13. Server-hit 18 Shout, Battle Command, Battle Orders `0x005AB0B0` | 427–441 |
|   14. Server-hit 26 Grim Ward start `0x005AB8D0` | 442–456 |
|   15. Server-do 14 Grim Ward `0x005AEF70`, server-hit 27 `0x005ABA00` | 457–476 |
|   16. Server-hit 52 Blade Fury `0x005AC940` | 477–495 |
|   17. Server-hit 7 Holy Bolt, Fist of the Heavens bolt `0x005A9FB0` | 496–524 |
|   18. Server-do 22 lightning trailing javelin `0x005AF620` | 525–541 |
|   19. Server-hit 45 lightning trailing javelin `0x005AC480`, server-hit 38 catapult charged ball `0x005AC0A0` | 542–598 |
|   20. Server-do 23 / 24 Succubus fireball, firestorm maker `0x005AF790` | 599–611 |
|   21. Server-do 26 Vines, Plague Vines `0x005AF980` | 612–622 |
|   22. Server-do 31 Wake of Destruction maker, Baal cold maker `0x005B01F0` | 623–642 |
|   23. Server-hit 56 Armageddon / Diablogeddon control `0x005ACC50` | 643–659 |
|   24. Server-hit 8 Blaze `0x005AA180` | 660–671 |
|   25. Server-hit 9 Immolation Arrow `0x005AA250` | 672–709 |
|   26. Server-do 9 bat lightning bolt `0x005AE940` | 710–721 |
|   27. Server-hit 15 spider goo lay `0x005AAD40` | 722–731 |
|   28. Server-hit 17 Howl `0x005AAFB0` | 732–748 |
|   29. Server-do 11 finger mage spider `0x005AEB60` | 749–768 |
|   30. Server-hit 19 finger mage spider `0x005AB110` | 769–778 |
| Constants & data dependencies | 779–823 |
| Randomness | 824–842 |
| Edge cases & original bugs | 843–872 |
| Test vectors | 873–901 |
| Provenance | 902–932 |
| Open questions | 933–952 |
<!-- /index -->

## Summary

Missile bodies split out of `missiles.md` for size. §1–§5: the
Cairn Stones spark and portal opener (server-do 17), the Volcano debris
thrower (28), Baal's taunt controller (34), the zig-zagging Royal
Strike chaos ice (35) and Baal's taunt lightning scatter (server-hit 58).
Conventions are those of `missiles.md` §R9.5: "flight" = default flight
§R4 (`0x005AE1F0`) and its result is returned; "body 3" = the server-do
3 body (`0x005AE480`, §R9.5 item 2); position = the missile's path x /
y; owner = `0x00552FD0`; level and skill = missile data +0x0C / +0x0A;
frames left = `0x0064A380`, elapsed = `0x0064A3B0`; data +0x28 =
`0x0064A730` / `0x0064A710` (read / write). Missile columns: `Param1`
+0x38 … `Param5` +0x48, `sHitPar1` +0x4C, `SubMissile1..3` +0x18..+0x1C,
`HitSubMissile1` +0x24, `Range` +0x96 (`data/fields.tsv`). Draws are
on the missile's own unit seed (unit +0x20) after the re-seed named,
unless a section names another seed; helpers in other specs (area
damage, creation) make their own.

From §6 on, the bodies that were still `summarized` or `D2MOO-only` in
the catalogues follow, ordered by how many live `missiles.txt` rows use
them (table under Constants), ties by the lowest row; from §31 on they
continue in `missiles/bodies-2.md`.

## Inputs

| Name | Type | Source |
|---|---|---|
| missile unit | type 3 | ECX game, EDX missile (server-do); + stack unit (server-hit, unused by 58) |
| missiles.txt record | 0x1A4 bytes | data tables +0xB64, count +0xB6C |
| skills record | 0x23C bytes | `aurarangecalc` +0x64, `calc1`…`calc4` +0x138…+0x144, `Param1`…`Param8` +0x148…+0x164; count data tables +0xBA0, records +0xB98 |

## Outputs / state changes

Created missiles and objects, missile data +0x28 / +0x2C, the
missile's path target and path, draws on the missile seed.

## Rules

### 1. Server-do 17 Cairn Stones `0x005AF240`

Row cairnstones (288): `Param1` 40, `Param2` 2, `Param3` 17, `Param4`
38, `Param5` 100, `SubMissile1` cairnstonessky, `Range` 300.

1. No record → return 2.
2. f = frames left. `SubMissile1` > 0 (i16), f < `Range` − `Param1` and
   f > `Param1` → §R9.3 helper (missile, range `Param3`, interval
   `Param2`, class `SubMissile1`, mask 5).
3. `Param4` ≥ 0, data +0x28 = 0 and f ≤ `Param5` + `Param1` → open the
   portal (`0x005A9930`): data +0x28 := 1; owner O; create a portal
   object `0x0056D130(game, O, the missile's room, x, y, destination
   level Param4, 0, object class 60, 1)` (`world/quests.md`); refresh the
   missile's room (`0x0061AED0(room, 1)`). Once per missile.
4. Return flight.

Live: sparks between frames left 260 and 40, every 2 elapsed frames
within range 17; the Tristram portal (level 38) when 140 frames are
left. Draws: only the helper's (re-seed x + elapsed, two rolls).

### 2. Server-do 28 Volcano `0x005AFB80`

Row volcano (479): `Param1` 0, `Param2` 0, `Param3` 2, `Param4` 128,
`Param5` 30, `SubMissile1` volcano debris 2.

1. No record or `SubMissile1` < 0 → return 2. Missile skill k invalid →
   return 2.
2. L = level; O = owner. O none → step 5.
3. r = `Param2`; r < 1 → r = max(`eval(O, k.aurarangecalc, k, L)`, 1)
   (`0x00646CA0`). Then i = `Param1`; i < 1 → i = max(`eval(O, k.calc4,
   k, L)`, 1).
4. e = elapsed. `Param3` < e < `Param4` and e mod i = 0 (signed):
   1. `init_low(data +0x28)` on the missile seed; dx = `roll(2r + 1)` −
      r, then dy = `roll(2r + 1)` − r (`0x0045C390`, two steps); data
      +0x28 := `get_lo()` (`0x00650E50`).
   2. Create `SubMissile1` (`0x0059FA30`) from a zeroed record: flags
      0x520 (0x20 target absolute, 0x100, 0x400 frames from distance),
      owner O, origin the missile (start at its position), target (x +
      dx, y + dy), gfx argument (+0x24) := `Param5`, skill k, level L.
5. Return flight.

The seed word in data +0x28 chains the scatter from throw to throw.

### 3. Server-do 34 Baal taunt control `0x005B04A0`

Row baal taunt control (546): `Param1` 25, `Param2` 3, `Param3` 45,
`Param4` 0, `SubMissile1` baal taunt lightning control, `SubMissile2`
baal taunt poison control.

1. No record → return 2. e = elapsed.
2. n = the number of leading slots j = 0, 1, 2 with `SubMissile(j+1)` ≥
   0 and interval `Param(j+2)` ≥ 1 (stops at the first slot failing
   either). n = 0 → return 2. Live: n = 2.
3. e < `Param1` → return body 3.
4. `init_low(missile x)` (`missiles.md` §R9.4); j = `roll(n)` (one step:
   `lo' & (n − 1)` when n is a power of two, else `lo' mod n`).
5. e mod `Param(j+2)` = 0 (signed) → create `SubMissile(j+1)` from a
   zeroed record (flags 0: start and target at the origin; owner, origin
   the missile, skill, level of the missile); created → hit handler
   `0x005ADF10(game, new missile, no unit, 1)` (§R5; result ignored).
6. Return body 3.

The seed is reset to the same x every run, so a missile that does not
move picks the same slot every frame (the control missile has no
velocity in body 3 when its path velocity is 0).

### 4. Server-do 35 Royal Strike chaos ice `0x005B0640`

Row royalstrikechaosice (569): `Param1` 3. Data +0x28 = seed word;
data +0x2C = direction (a, b) as two signed 16-bit halves, low = a
(`0x0064A780` / `0x0064A760`; set at creation by the skill, skills
spec).

1. No record → return 2. n = `Param1`, < 1 → 1. Elapsed mod n ≠ 0
   (signed) → return flight.
2. `init_low(data +0x28)`; one step; turn by `lo'` bit 0:
   - 1: a' = (4a − b) / 4, b' = (4b + a) / 4;
   - 0: a' = (4a + b) / 4, b' = (4b − a) / 4;
   signed division rounding toward 0; a' = 0 → 1, b' = 0 → 1.
3. Path target := (x + a', y + b') with no target unit
   (`0x00648AD0`); rebuild the path (`0x00649970(path, missile, 0)`,
   `sim/pathing.md` §3).
4. Data +0x28 := `get_lo()`; data +0x2C := (b' << 16) + (a' & 0xFFFF).
5. Return flight.

A turn of about 14° left or right every n frames.

### 5. Server-hit 58 Baal taunt lightning control `0x005ACDF0`

Row baal taunt lightning control (654): `sHitPar1` 10,
`HitSubMissile1` baal taunt lightning.

1. No record or `HitSubMissile1` < 0 → return 1. Owner O none → return
   1.
2. `init_low(missile x)`; r = `sHitPar1`, < 1 → 1.
3. x' = x − r + `roll(2r + 1)`, then y' = y − r + `roll(2r + 1)`.
4. Create `HitSubMissile1` from a zeroed record: flags 0x20 (target
   absolute), owner O, origin the missile, target (x', y'), skill and
   level of the missile. Return 1 (whatever the creation did).

The unit argument is not read.

### 6. Server-hit 2 Plague Javelin, gas potions `0x005A9D80`

Rows: plaguejavelin (43), rancidgasepotion (47), chokinggaspoition
(48), stranglinggaspotion (49), catapult plague ball (419),
plaguejavelin2 (436). `sHitPar1` 1, 0, 2, 1, 2, 1; `sHitPar2` 2 and
`sHitPar3` 3 on all six.

1. Missile none, no record, or `HitSubMissile1` < 0 (i16) → return 3.
2. O = the unit argument, or the missile itself when there is none.
3. `ring(game, owner, O, HitSubMissile1, skill, level, sHitPar1,
   sHitPar2, sHitPar3)`; its result is ignored. Return 3.

`ring(game, owner, origin, class, skill, level, a, b, loops)` =
`0x005A9370` (ECX game, EDX owner, seven stack arguments):

1. No record for `class`, owner none or origin none → return 0.
2. Zeroed parameter record (§R2.1): flags 0x17 (1 position given, 2
   target relative, 4 velocity given); loops > 0 → flags 0x1F and loops
   field := loops. Owner, origin, class; start = the origin's position
   (`0x0045ADF0` / `0x0045AE20`); skill, level.
3. Velocity field := `Param1` of the **created class's** row << 7.
   Flag 0x10 (part of 0x17 / 0x1F) marks it as already fixed point, so
   §R2.3 step 5 does **not** shift it again; step 7 takes 75 %:
   `Param1` 2 → 256 → 192 (a `Vel` 24 javelin flies at 24 << 8 × 75 % =
   4,608).
4. s = max(b, 1). For i = 0, s, 2s, … while i < 16: target offset
   (RX[i], RY[i]); create (`0x0059FA30`).
5. a ≠ 0: velocity field := the created row's `Param2` << 7; for i = 0,
   a, 2a, … while i < 15: offset (RX[i + 1], RY[i + 1]); create.
6. Return 1.

RX (`0x006E2510`) = 0, 1, 2, 2, 2, 2, 2, 1, 0, −1, −2, −2, −2, −2, −2,
−1; RY (`0x006E24D0`) = 2, 2, 2, 1, 0, −1, −2, −2, −2, −2, −2, −1, 0, 1,
2, 2: sixteen directions on a ring of radius 2, entry 0 = +y.

Live: plaguejavelin makes 8 clouds (even directions, `Param1` 2) and 15
more (directions 1–15, `Param2` 4), all with 3 loops; rancidgasepotion
(a = 0) only the 8. plaguejavlinexplode (plaguejavelin2) has no
`Param1`/`Param2`: flag 4 is set with velocity field 0, so its pieces
do not move (§R2.3 steps 5–8 skip the aim checks).

### 7. Server-do 6 Fire Wall maker, Molten Boulder `0x005AE680`

Rows: firewallmaker (68), vampirefirewallmaker (130), moltenboulder
(452), countessfirewallmaker (653); `SubMissile1` firewall,
vampirefirewall, moltenboulderfirepath, countessfirewall.

1. O = owner (`0x00552FD0`, called before any check). O none → return 2.
2. Missile none, no record, `SubMissile1` < 0 (i16) or no path →
   return 2.
3. Path new-step flag set (`0x006505C0`, as `sub_at_step`,
   `missiles.md` §R9.5 item 1): zeroed record, flags 0x21 (position
   given, target absolute); owner O; no origin; class `SubMissile1`;
   start = target = the missile's position; skill, level of the missile.
   Velocity from the created row (flag 4 clear). Create; result ignored.
4. Return flight.

One fire piece per sub-tile entered; no draws.

### 8. Server-hit 3 potions, bomb on ground `0x005A9F90` and server-hit 44 Exploding / Ice Javelin `0x005A9E10`

Server-hit 3 rows: oilpotion (44), explosivepotion (45),
fulminatingpotion (46), bomb on ground (386); `sHitPar1` 2, 3, 6, 0.
Server-hit 44 rows: explodingjavalin (429), icejavalin (434);
`sHitPar1` 2, 1.

Server-hit 3: unit given → return 0 (no bit: no damage stage, no exit;
`missiles.md` §R5 step 8 keeps the missile unless a4 = 1). Else run
server-hit 44 with no unit (same game, missile) and return its result.

Server-hit 44:

1. Missile none or no record → return 1.
2. O = owner (may be none). r = `sHitPar1` (i32). r = 0 exactly → k =
   the missile's skill; k's record missing (`0 ≤ k <` skills count,
   data tables +0xBA0, records +0xB98, 0x23C bytes) → return 1; r =
   max(`eval(O, k.aurarangecalc, k, level)`, 1) (`0x00646CA0`). A
   negative `sHitPar1` is used as it is.
3. Zeroed 0x70-byte damage record; full damage roll `0x005A89A0(missile,
   unit, record)` (`missiles.md` §R6.2: every element on the missile
   seed, then the unit terms when a unit is given).
4. Record hit flags (+0x00) |= `HitFlags` (+0xA8); result flags (+0x04,
   16-bit) |= `ResultFlags` (+0xAC).
5. a = `area_damage(game, O, x, y, r, record, 0)` (`missiles.md` §R9.6)
   at the missile's position. Return 1 when a ≠ 0, else 3; a is always 1.

Unlike server-hit 1 (`elem_roll`, `EType` only), this rolls every
element. bomb on ground (`sHitPar1` 0) takes its radius from Fire
Trauma's `aurarangecalc` (par1).

### 9. Server-hit 14 Meteor center, catapult meteor, royal strike meteor `0x005AABB0`

Rows: meteorcenter (101), vampiremeteorcenter (133), catapult meteor
ball (422), royalstrikemeteor (563); `sHitPar1` 0, 0, 1, 6;
`sHitPar2` 1, 1, 0, 1; `HitSubMissile1` meteorfire, vampiremeteorfire,
catapult meteor fire, royalstrikemeteorfire.

1. Missile none or no record → return 1.
2. k = missile skill, L = level. k's record missing → return 1. O =
   owner; none → return 1.
3. (x, y) = the missile's position. r = `sHitPar1`; r ≤ 0 → r =
   max(`eval(O, k.aurarangecalc, k, L)`, 1).
4. Damage record and flags as §8 steps 3–4.
5. c = 1 when `area_damage(game, O, x, y, r, record, 0)` ≠ 0, else 3
   (always 1).
6. h = `HitSubMissile1` (i16) ≥ 0: R = `skill_range(k, L)`; s =
   max(`sHitPar2`, 1); `scatter(game, missile, h, R, s)`.
7. Return c.

`skill_range(k, L)` = `0x004CC7C0` (ECX skill, EDX level): no record or
L ≤ 0 → 0; else skills `Param3` (+0x150) + (L − 1) × `Param4` (+0x154).
Live: Meteor, VampireMeteor and Royal Strike all 30 + 15 (L − 1).

`scatter(game, missile, class, R, s)` = `0x005AAA90` (EAX missile,
stack game, class, R, s):

1. class < 0 → nothing.
2. (x, y) = the missile's position; owner (none → every creation fails,
   §R2.3 step 1); zeroed record, flags 1 (position given, target = start);
   R > 0 → flags 0x8001 and range field := R. Owner, class, skill, level
   of the missile.
3. For i = 0, s, 2s, … while i < 18: start (x + MX[i], y + MY[i]);
   create.

MX (`0x006E2550`) = 2, −2, 0, 0, −3, 0, 3, −1, 1, −1, 2, −4, −3, −1, 0,
1, 3, 4; MY (`0x006E2598`) = −2, −2, 2, 5, 3, 3, 3, 2, 1, −1, −1, −2,
−2, −3, −4, −3, −3, −2.

royalstrikemeteor has no server-do, so it never flies or expires; it
explodes only when server-hit 4 runs it (`missiles.md` §R9.6 item 2,
royalstrikemeteorcenter `sHitPar1` 1).

### 10. Server-hit 36 missile in air `0x005ABF70`

Rows: bomb in air (385), shock field in air (388), catapult spike in
air (412), sentryspikeinair (496); `HitSubMissile1` bomb on ground,
shock field on ground, catapult spike on ground, sentryspikeonground.

1. Missile none, no record or `HitSubMissile1` < 0 → return 1.
2. Unit given → return 0.
3. N = `0x0056EDE0(game, owner, skill, level, HitSubMissile1, x, y)` at
   the missile's position (`skills/bodies.md` §6.13; owner none → none).
4. N created → N data +0x28 := this missile's data +0x28 (`0x0064A730`
   / `0x0064A710`).
5. Return 1.

These rows have `CollideType` 6, so only the expiry hit (no unit) runs
this: the thrown object lands where its flight ends.

### 11. Server-hit 10 Guided Arrow, Bone Spirit `0x005AA650`

Rows: guidedarrow (86), bonespirit (193), monbonespirit (329); `Param2`
15, 15, 0. s = data +0x28: bit 0 homing on a unit, bit 1 aimed at a
point (set by the skill at creation, `missiles.md` §R9.5 item 4), bit 2
re-targeted once (below).

1. The missile's room (`0x00620BB0`) in town (`0x0061AB00`) → return 1.
2. s = data +0x28. No unit and s bit 2 → return 1.
3. w = the collision word under the missile (`0x00648EB0`, as
   `missiles.md` §R4 step 6). No unit and w bit 2 (missile barrier) →
   return 1.
4. s bit 0: unit given and unit ≠ the missile's path target (`0x00553540`:
   refreshed; none when it is the missile) → return 4; else return 3.
5. s bit 1 clear → return 3.
6. Frames left > 0 → return 3 when s bit 2, else 4.
7. Frames left ≤ 0 (expiry while aimed at a point): `retarget(missile,
   game)`; it returns non-zero → return 1; else return 4.

`retarget` = `0x005AA5B0` (EAX missile, stack game):

1. No record → 1. Data +0x28 bit 2 → 1. Owner none → 1.
2. T = `next_unit(game, owner, x, y, Param2, 3, −1, no count)`
   (`missiles.md` §R9.6 item 3: flags 0xA783; with g = −1 no GUID is
   greater, so T is the qualifying unit with the smallest GUID, compared
   unsigned).
3. `aim(missile, T)` (`0x005AA460`, ESI missile); return 0.

`aim(missile, T)`:

1. No record → nothing. T dead (`0x005541B0` ≠ 0) → T none.
2. n = `Range` + `LevRange` × (L − 1) (both i16); total frames := n,
   frames left := n (`0x0064A2B0`, `0x0064A330`, clamped).
3. T: path target unit := T (`0x00648B90`); data +0x28 := 5; d =
   `0x006416D0(missile, T)` (`missiles.md` §R9.5 item 4).
4. No T: w = data +0x2C (`0x0064A780`); point = (x + low16(w), y +
   high16(w)), halves signed; path target point := it (`0x00648AD0`);
   data +0x28 := 6; d = `0x006417F0(missile, point)`.
5. d < 25 → rebuild the path (`0x00649970(path, missile, 0)`).

The spirit lives a second full lifetime after its first expiry, then
dies at the second (s bit 2 set → step 2 returns 1). No draws.

### 12. Server-hit 16 Spider goo, vines trail, vines wither `0x005AAE10`

Rows: spidergoo (146), vines trail (472), vines wither (473; no
`CollideType`, so only its expiry runs this). No missile column is
read; the skill k is the one stored at creation. F = game frame (game
+0xA8).

1. k = missile skill, L = level. k invalid → return 1. O = owner; none
   → return 1.
2. No unit → return 0.
3. s = k.`auratargetstate` (+0x82, i16); s < 0 or ≥ states count (data
   tables +0xC4) → return 1.
4. len = max(`eval(O, k.calc4, k, L)`, 5).
5. Lst = the unit's list of s (`0x006256B0(unit, s)`).
6. Lst none: alloc (game pool +0x1C, flags 2, expiry F + len, O type,
   O GUID) (`0x006251F0`); failure → return 1. Set state id s, remove
   callback `0x0056E900`, attach to the unit (`0x00626E10(unit, new,
   1)`), state s on (`0x00639DB0(unit, s, 1)`). **Lst stays none.**
7. `aura_fill(unit, Lst, k record, k, L)` (`skills/bodies.md` §2.6;
   formulas evaluated on the hit unit; nothing when Lst is none).
8. Mark s changed on the unit (`0x00639E30(unit, s, 1)`).
9. Lst expiry := F + len (`0x006260B0`; nothing when none).
10. Timer 12 at F + len on the unit (`0x005417D0(game, unit, 12, F +
    len, 0, 0)`).
11. Return 0 (no damage stage, no exit: the goo stays).

So the first contact puts the state on without its `aurastat` values;
a later contact, finding the list, fills them and pushes the expiry.

### 13. Server-hit 18 Shout, Battle Command, Battle Orders `0x005AB0B0`

Rows: shout (149), battlecommand (236), battleorders (237).

1. O = owner; none → return 1.
2. Unit given and the ally test `0x00554DE0(game, O, unit)` passes
   (`skills/bodies.md` §2.11 flag 0x10000) → `shout_state(game, unit, O,
   skill, level)` (`0x005D8290`, `skills/bodies.md` §6.8).
3. Return 0.

Result 0: the missile flies on through every unit (`CollideFriend` 1 on
all three). shout has no `LastCollide`, so a unit standing in its path
is met on every step; battlecommand and battleorders (`LastCollide`,
`NextHit`, `NextDelay` 4) skip the unit just met (`missiles.md` §R5).

### 14. Server-hit 26 Grim Ward start `0x005AB8D0`

Rows: grimwardsmallstart (249), grimwardmediumstart (252),
grimwardlargestart (255); `sHitPar1` 0; `HitSubMissile1` grimwardsmall,
grimwardmedium, grimwardlarge.

1. Missile none, no record or `HitSubMissile1` < 0 → return 1. O =
   owner; none → return 1.
2. k = missile skill, L = level; k invalid → return 1.
3. r = `sHitPar1`; r ≤ 0 → r = max(`eval(O, k.calc1, k, L)`, 5).
4. Zeroed record: flags 0x8001 (position given, range given); owner O;
   class `HitSubMissile1`; range r; skill k, level L; start = target =
   the missile's position. Create; result ignored.
5. Return 1.

### 15. Server-do 14 Grim Ward `0x005AEF70`, server-hit 27 `0x005ABA00`

Rows: grimwardsmall (250), grimwardmedium (253), grimwardlarge (256);
`Param1` 6, `Param2` 30.

Server-do 14:

1. Missile none or no record → return 2.
2. n = max(`Param1`, 1). Elapsed mod n ≠ 0 (signed) → return flight.
3. k = skill, L = level; k ≤ 0 or L ≤ 0 → return 2.
4. Skill server-do `Param2` with the **missile as the caster**:
   `0x0056D810(game, missile, Param2, k, L)` calls entry `Param2` of
   the skill server-do table `0x007322B0` (`skills/use.md`; index ≤ 190
   unsigned and entry non-null, else nothing) as (game, missile, k, L);
   its result is ignored. Live: entry 30, Curse (`skills/bodies.md`
   §4.4), every 6 frames.
5. Return flight.

Server-hit 27 returns 1 and does nothing else.

### 16. Server-hit 52 Blade Fury `0x005AC940`

Rows: bladefury1 (505), bladefury2 (507), bladefury3 (509); `sHitPar1`
1; `HitSubMissile1` bladefragment1–3.

1. Missile none, no record or `HitSubMissile1` < 0 → return 1. O =
   owner; none → return 1.
2. Zeroed record: flags 2 (target relative; start = the origin's
   position); owner O; origin = the missile; skill, level of the
   missile. **The class field is never written: the created class is 0
   (arrow)**; `HitSubMissile1` is only tested.
3. s = max(`sHitPar1`, 1). For i = 0, s, 2s, … while i < 8: target
   offset (BX[i], BY[i]); create; created → its data +0x28 := BX[i],
   data +0x2C := BY[i] (`0x0064A710`, `0x0064A760`).
4. Return 1.

BX (`0x006E2A58`) = 16, 16, 0, −16, −16, −16, 0, 16; BY (`0x006E2A38`) =
0, 16, 16, 16, 0, −16, −16, −16: eight directions, 16 sub-tiles out.

### 17. Server-hit 7 Holy Bolt, Fist of the Heavens bolt `0x005A9FB0`

Rows: holybolt (55), fistoftheheavensbolt (234); `sHitPar1` 1 (heal
allies), `sHitPar2` 1 (which monsters it damages: 0 all, 2 demons, any
other value undead).

1. Missile none, no record or no unit → return 1.
2. k = skill, L = level, O = owner.
3. Heal: `sHitPar1` ≠ 0, O exists, and the unit is O's pet
   (`0x005542C0(game, O, unit, k)`) or ally (`0x00554D20(game, O, unit,
   k)`; `skills/use.md` §5.3 step 2), tested in that order:
   1. R = k's record (`0x0045C4B0`); none → return 1.
   2. lo = `eval(O, R.calc1, k, L)` << 8; hi = `eval(O, R.calc2, k, L)`
      << 8 (calc1 first).
   3. v = lo + `roll(hi − lo)` on the **missile** seed (`0x0045C3E0`; no
      step when hi − lo < 1).
   4. v ≤ 0 → v = the §R6.2 per-element roll of stats 21 / 22 with no
      mastery (`0x005A8910`, missile seed).
   5. Unit life (stat 6) := min(life + v, max life) (`0x00625480`,
      `0x00625D10`, `0x00627260(unit, 6, ·, 0)`).
   6. `ProgOverlay` (+0x36, i16) > 0 → overlay on the unit
      (`0x00621E40(unit, ProgOverlay, 0)`).
   7. Return 1.
4. Otherwise by unit type: player → 3 when `sHitPar2` = 0, else 4.
   Monster → `sHitPar2` 0: 3; 2: 3 if demon (`0x0063E940`) else 4; any
   other: 3 if undead (`0x0063E990`) else 4. Other types → 4.

Result 3 = damage and die, 4 = pass through untouched.

### 18. Server-do 22 lightning trailing javelin `0x005AF620`

Rows: lightingtrailingjavalin (431), advlighttrailingjav (438);
`Param1` 3; `SubMissile1` lightjavalintrail, advlighttrailingjav2.

1. Missile none, no record, `SubMissile1` = 0 (equality: −1 passes and
   its creations fail) or no path → return 2.
2. (x, y) = position.
3. Elapsed < 2: d = path target point (path +0x10, +0x12: `0x00648A00`,
   `0x00648A10`) − (x, y); data +0x28 := −d.y, data +0x2C := d.x (the
   perpendicular of the flight direction).
4. Path new-step flag: zeroed record, flags 0xB (position given, target
   relative, add loops); owner (none → fails); start (x, y); skill,
   level; class `SubMissile1`; loops field `Param1`. Target offset (data
   +0x28, data +0x2C): create; offset negated: create.
5. Return flight.

### 19. Server-hit 45 lightning trailing javelin `0x005AC480`, server-hit 38 catapult charged ball `0x005AC0A0`

Server-hit 45 rows: lightingtrailingjavalin (431), advlighttrailingjav
(438); `sHitPar1` 10, 18; `sHitPar2` 1; `HitSubMissile1`
lightjavalinexplosion, advlightjavexplode. Server-hit 38 row:
catapultchargedball (407); `sHitPar1` 4, `sHitPar2` 2;
`HitSubMissile1` catapultchargedballbolt.

Server-hit 45:

1. Missile none, no record, `HitSubMissile1` < 0 or `sHitPar1` ≤ 0 →
   return 1.
2. Zeroed record, flags 3 (position given, target relative); start =
   the missile's position; skill, owner (may be none), class
   `HitSubMissile1`, level. `sHitPar2` ≠ 0 → init callback (+0x54) :=
   `zigzag` (`0x005AC040`), argument 0.
3. `nova(game, sHitPar1, record)`. Return 1.

Server-hit 38:

1. Missile none, no record or `HitSubMissile1` < 0 → return 1. O =
   owner; none → return 1.
2. Record as server-hit 45, owner O, init callback `zigzag` always.
3. n = `sHitPar1` + (L − 1) × `sHitPar2`. n ≤ 0: k's record
   (`0x0045C4B0`) none → return 1 (nothing made); n = max(`eval(O,
   k.calc4, k, L)`, 1).
4. `nova(game, n, record)`. Return 1.

`nova(game, n, record)` = `0x0056D4E0` (ECX game, EDX n, stack record):

1. (n − 1) / 5 (signed, truncating) > 7 → fatal assertion and process
   exit (n ≥ 41).
2. Target offset (14, −14); create.
3. c = n − 1. For j = 0…6:
   1. c = 0 → stop (**equality**; D2MOO 1.10f stops at c ≤ 0).
   2. For i = 0…3 while c > 0: offset (SX[i] × P[j], SY[i] × Q[j]),
      create; offset (SX[i] × Q[j], SY[i] × P[j]), create; c −= 2.
   3. Offset (EX[j], EY[j]); create; c −= 1.

SX (`0x006E1510`) = −1, 1, 1, −1; SY (`0x006E14E4`) = −1, −1, 1, 1; P
(`0x006E14F4`) = 18, 20, 17, 20, 15, 19, 18; Q (`0x006E14C8`) = 8, 2,
11, 4, 13, 6, 9; EX (`0x006E14A8`) = 20, −20, 0, 0, 14, −14, −14 (8th
entry 14 unused); EY (`0x006E1488`) = 0, 0, 20, −20, 14, 14, −14.
Exactly n missiles only when n − 1 is a multiple of 9; otherwise c
passes 0 inside a round and every remaining round still adds its EX/EY
missile (totals under Test vectors).

`zigzag(missile)` = `0x005AC040` (ECX missile; init callback,
`missiles.md` §R2.3 step 21):

1. Missile none → nothing. t = min(total frames (`0x0064A300`), 255).
2. **Re-seed** the missile seed: `init_low(path target x)` (path +0x10,
   `0x00648A00`; `0x00650E40`).
3. Path type := 10 (`0x00648CF0`; charged-bolt zigzag compute,
   `missiles.md` §R4.3); path distance := t (`0x00648E70`); rebuild
   (`0x00649970(path, missile, 0)`).

### 20. Server-do 23 / 24 Succubus fireball, firestorm maker `0x005AF790`

Rows (index 23): sucfireball (441, `Param1` 3, `SubMissile1`
sucfireballtrail), firestormmaker (458, no `Param1`, `SubMissile1`
firestorm). Index 24 points at the same function; no row uses it.

1. Missile none, no record or `SubMissile1` = 0 (equality) → return 2.
2. Path new-step flag (`0x006505C0`, 0 without a path): zeroed record,
   flags 1 (position given; target = start); `Param1` > 0 → flags 9 and
   loops field `Param1`; owner (none → fails); start = the missile's
   position; skill, level; class `SubMissile1`. Create.
3. Return flight.

### 21. Server-do 26 Vines, Plague Vines `0x005AF980`

Rows: vines (471), plague vines (474); `Param1` 9; `SubMissile1`
vines trail, plague vines trail.

1. Missile none, no record or `SubMissile1` = 0 → return 2.
2. Elapsed mod max(`Param1`, 1) = 0 (signed) → `0x0056EDE0(game, owner,
   skill, level, SubMissile1, x, y)` at the missile's position
   (`skills/bodies.md` §6.13).
3. Return flight (tail jump).

### 22. Server-do 31 Wake of Destruction maker, Baal cold maker `0x005B01F0`

Rows: wake of destruction maker (517), baal cold maker (589);
`SubMissile1` wake of destruction, baal cold trail. Data +0x28 / +0x2C
hold an offset (a, b) set at creation (skills spec).

1. Missile none, no record or `SubMissile1` < 0 → return 2.
2. O = owner. None → the hit handler's no-unit path inlined
   (`missiles.md` §R5 with unit none, a4 = 0): c = 0 with `Explosion`,
   else 2; `CollideKill` → c |= 1; `AlwaysExplode` → `justhit` (nothing
   without a unit), unit event 0 with no unit (`0x005C0C30`), then
   `pSrvHitFunc` in 1…70 → c = its result (game, missile, no unit), c
   & 4 → return 2; then c & 1 and the missile has a room → clear
   collision 0x40 under it with its size (`0x0064EBA0`). Return 2
   whatever c.
3. Path new-step flag: zeroed record, flags 2 (target relative; start =
   the origin's position); owner O; origin = the missile; skill, level;
   class `SubMissile1`. Offset (a, b): create; offset (−a, −b): create.
4. Return flight.

### 23. Server-hit 56 Armageddon / Diablogeddon control `0x005ACC50`

Rows: armageddoncontrol (577), diablogeddoncontrol (670); `sHitPar1`
3; `HitSubMissile1` armageddonfire, diablogeddonfire.

1. Missile none or no record → return 1. O = owner; none → return 1.
2. k = skill, L = level, (x, y) = position.
3. Zeroed 0x70 record; full damage roll `0x005A89A0(missile, no unit,
   record)` (§R6.2 draws on the missile seed).
4. r = `sHitPar1`; r ≤ 0: k invalid → return 1; r = max(`eval(O,
   k.aurarangecalc, k, L)`, 1).
5. Hit flags |= `HitFlags`, result flags |= `ResultFlags`;
   `area_damage(game, O, x, y, r, record, 0)`.
6. `HitSubMissile1` ≥ 0 → `0x0056EDE0(game, O, k, L, HitSubMissile1, x,
   y)` (`skills/bodies.md` §6.13).
7. Return 1.

### 24. Server-hit 8 Blaze `0x005AA180`

Row: blaze (67).

1. No unit → return 2.
2. O = owner. O exists, the unit is O, and O has state 13 (`blaze`,
   `0x00639DF0(O, 13)`) → return 0.
3. Else return 2.

A blaze piece never burns its own caster while the caster's Blaze state
is on; anyone else gets the damage stage (result 2, no death bit).

### 25. Server-hit 9 Immolation Arrow `0x005AA250`

Row: immolationarrow (85); `sHitPar1` 0, `sHitPar2` 0, `SHitCalc1`
"100", `HitSubMissile1` immolationfire.

1. Missile none or no record → return 1. k = skill, L = level; k
   invalid → return 1. O = owner (may be none).
2. `HitSubMissile1` ≥ 0:
   1. Missile stats 56 (`coldlength`) := 0 and 134 (`item_freeze`) := 0
      (`0x00627260(missile, s, 0, 0)`).
   2. r = `sHitPar1`; r ≤ 0 → max(`eval(O, k.calc1, k, L)`, 1).
   3. h = missiles evaluator `0x0064B7C0(missile, O, SHitCalc1 (+0x88),
      class, L)` (`data/calc-expressions.md`).
   4. `fire_disc(game, missile, r, HitSubMissile1, h)`.
3. r2 = `sHitPar2`; r2 ≤ 0 → max(`eval(O, k.calc2, k, L)`, 1).
4. Zeroed 0x70 record; `elem_roll(game, missile, unit, record)`
   (`missiles.md` §R9.6: `EType` element only); hit flags |=
   `HitFlags`, result flags |= `ResultFlags`.
5. `area_damage(game, owner, x, y, r2, record, 0)` (owner fetched
   again). Return 3.

`fire_disc(game, missile, r, class, h)` = `0x005A9530` (ECX game, EDX
missile, stack r, class, h):

1. The missile has no room → nothing. h < 0 → h = 0.
2. Zeroed record, flags 1 (position given; target = start), | 0x8000
   when h ≠ 0 (range field := h before each creation); owner, skill,
   level of the missile; class.
3. (X, Y) = the missile's position. For a = −r…r (outer), b = −r…r
   (inner): skip unless the missile still has a room and a² + b² ≤ r²;
   skip when the line test `0x0064E260(room, (X + a, Y + b), (X + 2a,
   Y + 2b), mask 4)` reports a hit; skip when no room contains (X + a,
   Y + b) (`0x00463740(missile room, X + a, Y + b)`) or that room is in
   town (`0x0061AB00`). Else start = (X + a, Y + b); create.

r < 0 makes nothing. The line test runs from the cell outward, away
from the centre, not from the centre to the cell.

### 26. Server-do 9 bat lightning bolt `0x005AE940`

Row: bat lightning bolt (123); `SubMissile1` bat lightning trail.

1. Missile none or no record → the code reads `SubMissile1` through the
   null record (fatal, as server-do 8). With a record `SubMissile1` is
   not tested.
2. (x, y) = position. Path new-step flag: zeroed record, flags 1
   (position given; target = start); owner (none → fails); start (x, y);
   skill, level; class `SubMissile1`. Create.
3. Return flight.

### 27. Server-hit 15 spider goo lay `0x005AAD40`

Row: spidergoolay (143); `HitSubMissile1` spidergoo.

1. Missile none, no record or `HitSubMissile1` < 0 → return 1. O =
   owner; none → return 1.
2. Zeroed record, flags 1; owner O; start = the missile's position;
   skill, level; class `HitSubMissile1`. Create.
3. Return 0 (no damage, no death on a unit contact).

### 28. Server-hit 17 Howl `0x005AAFB0`

Row: howl (148).

1. k = skill, L = level; k invalid → return 1. O = owner; none →
   return 1.
2. No unit, or the unit is not a monster (type 1) → return 0.
3. s = k.`auratargetstate`; s < 0 or ≥ states count → return 1.
4. The unit has s (`0x00639DF0`) → return 0.
5. O's level (stat 12, `0x00625480(O, 12, 0)`) + L + k.`Param2`
   (+0x14C, `0x004EFC20`) ≤ the unit's level (stat 12) → return 0.
6. Terror install `0x005DDD00(game, O, unit, k, a, b)` (special state
   11, `monsters/ai.md`, `monsters/ai-bodies-2.md` §16): a = `Param3` +
   (L − 1) × `Param4` (`0x004CC7C0`), b = `Param5` + (L − 1) × `Param6`
   (`0x004EFCB0`); both 0 when L ≤ 0.
7. Return 0.

### 29. Server-do 11 finger mage spider `0x005AEB60`

Row: fingermagespider (177); `Param1` 5, `Param2` 20, `Param3` 2.

1. Missile none or no record → return 2.
2. T = the missile's path target (`0x00553540`, `skills/bodies.md`
   §2.1). O = owner (always fetched). No T: O exists → T = O's path
   target; still none → return flight.
3. n = `Param1`, ≤ 0 → 5. Frames left mod n ≠ 0 (signed) → return
   flight.
4. `0x006416D0(missile, T)` (`missiles.md` §R9.5 item 4) > `Param2`
   (signed) → return flight.
5. s = max(`Param3`, 1); dx = sign(T.x − x) × s, dy = sign(T.y − y) × s
   (0 when equal). Path target point := (x + dx, y + dy) (`0x00648AD0`);
   rebuild (`0x00649970(path, missile, 0)`).
6. Return flight.

The spider steps toward its target one short leg (2 sub-tiles per axis)
every 5 frames while within 20.

### 30. Server-hit 19 finger mage spider `0x005AB110`

Row: fingermagespider (177).

As §12 (server-hit 16) with these differences: no unit → return 1 (not
0); len = max(`eval(O, k.auralencalc (+0x60), k, L)`, 5); the result is
3 (damage, die). The checks run in the order k valid, owner, unit,
`auratargetstate` valid (each failing → 1). The fresh-list quirk of §12
step 6 is the same.

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| cairn portal object | class 60, destination `Param4`, last argument 1 | `0x005A9930` |
| volcano record flags | 0x520 | `0x005AFB80` |
| chaos ice turn | (4a ∓ b) / 4, (4b ± a) / 4 | `0x005B0640` |
| taunt lightning flags | 0x20 | `0x005ACDF0` |
| missiles.txt | `Param1..5`, `sHitPar1`, `SubMissile1..3`, `HitSubMissile1`, `Range` | `data/fields.tsv` |
| skills.txt | `aurarangecalc` +0x64, `calc4` +0x144 | `data/fields.tsv` |
| plague ring RX / RY | 16 entries each, radius 2 (§6) | `0x006E2510` / `0x006E24D0` |
| meteor scatter MX / MY | 18 entries each (§9) | `0x006E2550` / `0x006E2598` |
| plague ring velocity | created row `Param1` / `Param2` << 7, flags 4 + 0x10 (no further << 8) | `0x005A9370`, `0x0059FA30` |
| damage record | 0x70 bytes, hit flags +0x00, result flags +0x04 | `0x005A9E10`, `0x005AABB0` |
| bone spirit re-aim | `Range` + `LevRange` × (L − 1); rebuild when d < 25 | `0x005AA460` |
| goo state length | max(`calc4`, 5) frames | `0x005AAE10` |
| grim ward range | `sHitPar1`, else max(`calc1`, 5) | `0x005AB8D0` |
| skill server-do table | `0x007322B0`, index ≤ 190 | `0x0056D810` |
| blade fury BX / BY | 8 entries each, ±16 (§16) | `0x006E2A58` / `0x006E2A38` |
| nova SX, SY, P, Q, EX, EY | §19 | `0x006E1510`, `0x006E14E4`, `0x006E14F4`, `0x006E14C8`, `0x006E14A8`, `0x006E1488` |
| nova limit | (n − 1) / 5 ≤ 7, else fatal | `0x0056D4E0` |
| zigzag path | type 10, distance min(total frames, 255) | `0x005AC040` |
| blaze state | 13 | `0x005AA180` |
| immolation stats cleared | 56, 134 | `0x005AA250` |
| howl level test | O level + L + `Param2` > unit level | `0x005AAFB0` |
| spider step | sign × max(`Param3`, 1) every `Param1` (≤ 0 → 5) frames left, within `Param2` | `0x005AEB60` |

Use counts (live `patch_d2` `missiles.txt`, 684 rows; `pSrvDoFunc` /
`pSrvHitFunc` cells holding that index; royalstrikechainlightning's
`pSrvHitFunc` cell `*12` compiles to −588 in `missiles.bin`, an index
≤ 0, so it counts for nothing). These are the functions that were
`summarized` or `D2MOO-only` before this spec, in writing order:

Section numbers prefixed "2:" are in `missiles/bodies-2.md` (this
spec's continuation past 60 KB).

| Rows | Functions (lowest row) → section |
|---|---|
| 6 | hit 2 (43) §6 |
| 4 | hit 3 (44) §8, do 6 (68) §7, hit 14 (101) §9, hit 36 (385) §10 |
| 3 | hit 10 (86) §11, hit 16 (146) §12, hit 18 (149) §13, hit 26 (249) §14, do 14 (250) §15, hit 27 (250) §15, hit 52 (505) §16 |
| 2 | hit 7 (55) §17, hit 44 (429) §8, do 22 (431) §18, hit 45 (431) §19, do 23 (441) §20, do 26 (471) §21, do 31 (517) §22, hit 56 (577) §23 |
| 1 | hit 8 (67) §24, hit 9 (85) §25, do 9 (123) §26, hit 15 (143) §27, hit 17 (148) §28, do 11 (177) §29, hit 19 (177) §30, do 12 (179) 2:§31, hit 20 (206) 2:§32, do 13 (207) 2:§33, hit 21 (219) 2:§34, hit 22 (233) 2:§35, hit 24 (238) 2:§36, hit 25 (239) 2:§37, hit 28 (259) 2:§38, do 15 (260) 2:§39, hit 29 (260) 2:§39, do 16 (262) 2:§40, hit 31 (277) 2:§41, hit 32 (288) 2:§42, do 18 (332) 2:§43, hit 33 (332) 2:§43, do 19 (347) 2:§44, hit 35 (368) 2:§45, do 20 (392) 2:§46, hit 37 (392) 2:§46, do 21 (393) 2:§46, hit 38 (407) §19, hit 39 (409) 2:§47, hit 40 (411) 2:§48, hit 43 (425) 2:§49, hit 47 (452) 2:§50, hit 48 (453) 2:§51, hit 50 (475) 2:§52, do 27 (478) 2:§53, hit 51 (481) 2:§54, do 29 (498) 2:§55, do 30 (515) 2:§56, hit 53 (516) 2:§56, do 32 (520) 2:§57, do 33 (540) 2:§55, hit 54 (550) 2:§58, hit 55 (554) 2:§59, do 36 (625) 2:§60, hit 57 (625) 2:§60, hit 59 (655) 2:§61 |
| 0 | do 24 (shares do 23's address) §20, do 37, hit 5, hit 6, hit 11, hit 23 (all 2:§62) |

## Randomness

| Body | Re-seed | Draws, in order |
|---|---|---|
| 17 | §R9.3 helper: x + elapsed | helper's two rolls |
| 28 | data +0x28 | `roll(2r + 1)` × 2 (x first); low word saved |
| 34 | missile x | `roll(n)` (one step) |
| 35 | data +0x28 | one step, bit 0; low word saved |
| 58 | missile x | `roll(2r + 1)` × 2 (x first) |
| hit 2, do 6, hit 36, hit 10, hit 16, hit 18, hit 26, do 14, hit 27, hit 52 | — | none of their own (do 14's skill function may draw: skills spec) |
| hit 7 | — | heal only: `roll(hi − lo)`, then if v ≤ 0 the stat 21/22 roll |
| zigzag callback (hit 38, 45) | path target x (path +0x10) | none in the callback; the new missile's zigzag path compute (path type 10) uses this seed (`missiles.md` Open question 12) |
| hit 56 | — | §R6.2 rolls; `area_damage` per unit |
| hit 9 | — | `elem_roll` (`EType` element) on the missile seed, then `area_damage` per unit |
| hit 8, do 9, hit 15, hit 17, do 11, hit 19 | — | none of their own (hit 17's terror install: monsters spec) |
| hit 44, hit 14 | — (allocation seed) | `missiles.md` §R6.2 rolls in element order; then `area_damage`'s per-unit draws (not on the missile seed) |

Created missiles draw on their own seeds (`missiles.md` §R2.3).

## Edge cases & original bugs

1. Server-do 34 and server-hit 58 re-seed from the position: equal
   positions give equal choices.
2. Server-do 17 marks the portal done (data +0x28 := 1) before creating
   it; a failed creation is not retried.
3. Server-do 28 with no owner skips the throw but still flies.
4. The plague ring (§6) loops forever on a negative `sHitPar1` (the
   index only falls); no live row has one.
5. Server-hit 44 uses a negative `sHitPar1` as the radius (only 0 falls
   back to the skill); server-hit 1 and 14 fall back for any value ≤ 0.
6. Server-do 6 asks for the owner before checking the missile pointer.
7. Server-hit 3 and 36 return 0 for a unit contact (no damage, no
   death); their rows have `CollideType` 6 and never touch units.
8. Server-hit 16 keeps the fresh list out of its own fill (§12 step 6):
   the first contact gives the state with no stat values.
9. Server-hit 52 never sets the class: Blade Fury contacts create class
   0 (arrow) missiles, with the Blade Fury skill and level.
10. `nova` (§19) stops only when its counter is exactly 0, so most
    counts overshoot (n = 18 makes 24 missiles); n ≥ 41 kills the
    process.
11. Server-do 22, 23 and 26 reject `SubMissile1` = 0 (class arrow), not
    an empty column (−1); server-do 6 and 31 reject < 0.
12. Server-do 31 without an owner removes the missile (result 2) even
    when the server-hit result would keep it.
13. Server-do 9 with no record reads through a null pointer (fatal).
14. `fire_disc` (§25) traces each cell's line outward (cell to 2 ×
    offset), so a wall just beyond a cell can veto it.
15. Server-hit 19 keeps §12's fresh-list quirk.

## Test vectors

| Input | Expected | Source |
|---|---|---|
| chaos ice a = 8, b = 0, bit 1 | a' = 8, b' = 2 (path target x + 8, y + 2) | synthetic, §4 |
| chaos ice a = 8, b = 0, bit 0 | a' = 8, b' = −2 | synthetic |
| chaos ice a = 1, b = 1, bit 1 | a' = (4 − 1) / 4 = 0 → 1, b' = 5 / 4 = 1 | synthetic |
| cairnstones, frames left 150, data +0x28 = 0 | no portal (150 > 140); sparks if elapsed even | live row, §1 |
| baal taunt control, elapsed 24 | body 3 only, no draw | live row, §3 |
| plaguejavelin hit (`sHitPar1` 1, `sHitPar2` 2) | 8 clouds at RX/RY 0, 2, …, 14, then 15 at 1…15; 23 creations | live row, §6 |
| plaguejavelin hit, created row plaguejavcloud (`Param1` 2, `Param2` 4) | first ring 2 << 7 = 256 → 192; second ring 4 << 7 = 512 → 384 | live row, §6 step 3 |
| rancidgasepotion hit (`sHitPar1` 0) | 8 clouds, offsets (0, 2), (2, 2), (2, 0), (2, −2), (0, −2), (−2, −2), (−2, 0), (−2, 2) | live row, §6 |
| meteorcenter hit, L = 3 | 18 meteorfire, range field 60, first at (x + 2, y − 2) | live row + skills Meteor, §9 |
| catapult meteor ball (`sHitPar2` 0) | step 1: 18 pieces | live row, §9 |
| bonespirit, s = 2, frames left 0, no target in range 15 | total = left = 128 (`LevRange` 0); data +0x28 = 6; return 4 | live row, §11 |
| skill_range(Meteor, 0) | 0 (range field not set; table range) | §9 |
| grimwardsmallstart expiry, `calc1` 3 | grimwardsmall with range 5 | synthetic, §14 |
| grimwardsmall, elapsed 12, k > 0, L > 0 | skill server-do 30 runs with the missile as caster | live row, §15 |
| nova n = 1, 2, 3, 5, 9, 10, 11, 18, 19, 28 | 1, 10, 10, 12, 16, 10, 18, 24, 19, 28 missiles | synthetic, §19 |
| nova n = 3 offsets | (14, −14), (−18, −8), (−8, −18), (20, 0), (−20, 0), (0, 20), (0, −20), (14, 14), (−14, 14), (−14, −14) | synthetic, §19 |
| catapultchargedball, L = 3 | n = 4 + 2 × 2 = 8 → 16 missiles | live row, §19 |
| trailing javelin at (50, 50), target (60, 50), elapsed 0 | data +0x28 = 0, +0x2C = 10; trails aimed at offsets (0, 10) and (0, −10) | synthetic, §18 |
| holybolt on a hostile non-undead monster | 4 (passes) | live row, §17 |
| fire_disc r = 1, open floor, h = 100 | 5 pieces at (X − 1, Y), (X, Y − 1), (X, Y), (X, Y + 1), (X + 1, Y), in that order, range 100 | synthetic, §25 |
| fire_disc r = 2 | 13 cells (a² + b² ≤ 4) | synthetic, §25 |
| fingermagespider at (10, 10), target (25, 5), both size 1, frames left 75 | d = (2 × 15 + 5) / 2 = 17 ≤ 20 → path target (12, 8) | synthetic, §29 |
| howl, O level 10, L 3, `Param2` 2, monster level 15 | 15 ≤ 15 → return 0, no terror | synthetic, §28 |
| bladefury1 hit at (100, 100) | 8 class-0 missiles aimed at (116, 100), (116, 116), (100, 116), (84, 116), (84, 100), (84, 84), (100, 84), (116, 84) | live row, §16 |

## Provenance

- 1.14d `Game.exe`: `0x005AF240`, `0x005A9930`, `0x005AFB80`,
  `0x005B04A0`, `0x005B0640`, `0x0064A780`, `0x0064A760`,
  `0x005ACDF0`; Ghidra decompile read first, every call's arguments
  checked in the disassembly (`tools/ghidra/disasm.py fn`). The
  `0x0056D130` argument order matches `world/quests.md` (class 59 / 60
  portal calls).
- Live `patch_d2` missiles.txt rows 288, 479, 546, 569, 654.
- §6–§11: `0x005A9D80`, `0x005A9370`, `0x005AE680`, `0x005A9F90`,
  `0x005A9E10`, `0x005AABB0`, `0x004CC7C0`, `0x005AAA90`, `0x005ABF70`,
  `0x005AA650`, `0x005AA5B0`, `0x005AA460`, `0x0056BD10`, `0x005AAE10`,
  `0x006260B0` (null-safe), `0x005C6CC0` (null-safe), `0x005AB0B0`,
  `0x005AB8D0`, `0x005AEF70`, `0x0056D810` (table entry 30 =
  `0x005C37C0`, dumped), `0x005ABA00`, `0x005AC940`, `0x005A9FB0`,
  `0x005AF620`, `0x00648A00` (path +0x10), `0x005AC480`, `0x005AC0A0`,
  `0x0056D4E0` (callers: only these two, `disasm.py xref`),
  `0x005AC040`, `0x005AF790`, `0x005AF980`, `0x005B01F0`,
  `0x005ACC50`, `0x005AA180`, `0x005AA250`, `0x005A9530`,
  `0x005AE940`, `0x005AAD40`, `0x005AAFB0`, `0x004EFC20`, `0x004EFCB0`,
  `0x005AEB60`, `0x005AB110`; ring velocity: `0x0059FA30` shifts a
  given velocity << 8 only when flag 0x10 is clear; table
  pointers checked (`disasm.py xref`: server-hit table `0x0073C840` +
  4 × index, server-do `0x0073C768` + 4 × index); offset tables dumped
  from `Game.exe` (`0x006E1488`–`0x006E151F`, `0x006E24D0`–`0x006E25DF`, `0x006E2A38`–`0x006E2A77`). Live rows listed per
  section; use counts from `patch_d2` `missiles.txt`, the `*12` cell
  from `missiles.bin` row 568. The D2MOO 1.10f bodies of the same names
  agree step for step (hints only; every step above is a 1.14d read).
- D2MOO 1.10f `MissMode.cpp` (`MISSMODE_SrvDo17_CairnStones`, …): names
  only; the rules above are 1.14d reads.

## Open questions

1. No recording: record the Cairn Stones portal opening, Baal's taunt
   in the Worldstone Chamber and a Royal Strike, and compare created
   missiles and their ticks.
2. Flag 0x100 of the volcano record is not among the flags
   `0x0059FA30` reads (`missiles.md` §R2.1): confirm it is ignored.
3. *Answered* (`impl-missile-bodies-2` Q1): the ring's flags 0x17 /
   0x1F hold 0x10, so `0x0059FA30` takes the velocity field as fixed
   point without the << 8 (its flag-4 branch shifts only when 0x10 is
   clear): `Param1` 2 → 256 → 192 after the 75 % cut (§6 step 3). A
   Plague Javelin recording (Open question 1) still checks the cloud
   positions per tick.
4. Blade Fury contacts create class-0 (arrow) missiles (§16): record a
   Blade Fury hit and check for the eight extra missiles and their
   damage.
5. `nova` (§19) stops on c = 0 where D2MOO 1.10f stops on c ≤ 0:
   record a lightning trailing javelin (`sHitPar1` 18) explosion and
   count the bolts (24 by this rule, 18 by D2MOO's).
