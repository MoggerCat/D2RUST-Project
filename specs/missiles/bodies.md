# Spec: Missiles — server-do and server-hit bodies (continued)

- **Status:** draft: every body here (§1 onward; the use-count table
  under Constants lists them) read from the 1.14d `Game.exe` disassembly (addresses per section, register
  and stack arguments checked in `all.asm`). No recording covers them
  (Open question 1).
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
  creation `0x0056D130`).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 51–70 |
| Inputs | 71–78 |
| Outputs / state changes | 79–83 |
| Rules | 84–85 |
|   1. Server-do 17 Cairn Stones `0x005AF240` | 86–105 |
|   2. Server-do 28 Volcano `0x005AFB80` | 106–128 |
|   3. Server-do 34 Baal taunt control `0x005B04A0` | 129–151 |
|   4. Server-do 35 Royal Strike chaos ice `0x005B0640` | 152–172 |
|   5. Server-hit 58 Baal taunt lightning control `0x005ACDF0` | 173–187 |
|   6. Server-hit 2 Plague Javelin, gas potions `0x005A9D80` | 188–225 |
|   7. Server-do 6 Fire Wall maker, Molten Boulder `0x005AE680` | 226–243 |
|   8. Server-hit 3 potions, bomb on ground `0x005A9F90` and server-hit 44 Exploding / Ice Javelin `0x005A9E10` | 244–274 |
|   9. Server-hit 14 Meteor center, catapult meteor, royal strike meteor `0x005AABB0` | 275–316 |
|   10. Server-hit 36 missile in air `0x005ABF70` | 317–333 |
|   11. Server-hit 10 Guided Arrow, Bone Spirit `0x005AA650` | 334–376 |
|   12. Server-hit 16 Spider goo, vines trail, vines wither `0x005AAE10` | 377–405 |
|   13. Server-hit 18 Shout, Battle Command, Battle Orders `0x005AB0B0` | 406–420 |
|   14. Server-hit 26 Grim Ward start `0x005AB8D0` | 421–435 |
|   15. Server-do 14 Grim Ward `0x005AEF70`, server-hit 27 `0x005ABA00` | 436–455 |
|   16. Server-hit 52 Blade Fury `0x005AC940` | 456–474 |
| Constants & data dependencies | 475–509 |
| Randomness | 510–523 |
| Edge cases & original bugs | 524–542 |
| Test vectors | 543–561 |
| Provenance | 562–585 |
| Open questions | 586–599 |
<!-- /index -->

## Summary

Five more missile bodies, split out of `missiles.md` for size: the
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
`HitSubMissile1` +0x24, `Range` +0x96 (`data/fields.tsv`). All draws are
on the missile's own unit seed (unit +0x20) after the re-seed named.

From §6 on, the bodies that were still `summarized` or `D2MOO-only` in
the catalogues follow, ordered by how many live `missiles.txt` rows use
them (table under Constants), ties by the lowest row.

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
3. Velocity field := `Param1` of the **created class's** row << 7
   (§R2.3 step 5 then shifts it << 8 and step 7 takes 75 %).
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
| plague ring velocity | created row `Param1` / `Param2` << 7, flag 4 | `0x005A9370` |
| damage record | 0x70 bytes, hit flags +0x00, result flags +0x04 | `0x005A9E10`, `0x005AABB0` |
| bone spirit re-aim | `Range` + `LevRange` × (L − 1); rebuild when d < 25 | `0x005AA460` |
| goo state length | max(`calc4`, 5) frames | `0x005AAE10` |
| grim ward range | `sHitPar1`, else max(`calc1`, 5) | `0x005AB8D0` |
| skill server-do table | `0x007322B0`, index ≤ 190 | `0x0056D810` |
| blade fury BX / BY | 8 entries each, ±16 (§16) | `0x006E2A58` / `0x006E2A38` |

Use counts (live `patch_d2` `missiles.txt`, 684 rows; `pSrvDoFunc` /
`pSrvHitFunc` cells holding that index; royalstrikechainlightning's
`pSrvHitFunc` cell `*12` compiles to −588 in `missiles.bin`, an index
≤ 0, so it counts for nothing). These are the functions that were
`summarized` or `D2MOO-only` before this spec, in writing order:

| Rows | Functions (lowest row) → section |
|---|---|
| 6 | hit 2 (43) §6 |
| 4 | hit 3 (44) §8, do 6 (68) §7, hit 14 (101) §9, hit 36 (385) §10 |
| 3 | hit 10 (86) §11, hit 16 (146) §12, hit 18 (149) §13, hit 26 (249) §14, do 14 (250) §15, hit 27 (250) §15, hit 52 (505) §16 |
| 2 | hit 7 (55), hit 44 (429) §8, do 22 (431), hit 45 (431), do 23 (441), do 26 (471), do 31 (517), hit 56 (577) |
| 1 | hit 8 (67), hit 9 (85), do 9 (123), hit 15 (143), hit 17 (148), do 11 (177), hit 19 (177), do 12 (179), hit 20 (206), do 13 (207), hit 21 (219), hit 22 (233), hit 24 (238), hit 25 (239), hit 28 (259), do 15 (260), hit 29 (260), do 16 (262), hit 31 (277), hit 32 (288), do 18 (332), hit 33 (332), do 19 (347), hit 35 (368), do 20 (392), hit 37 (392), do 21 (393), hit 38 (407), hit 39 (409), hit 40 (411), hit 43 (425), hit 47 (452), hit 48 (453), hit 50 (475), do 27 (478), hit 51 (481), do 29 (498), do 30 (515), hit 53 (516), do 32 (520), do 33 (540), hit 54 (550), hit 55 (554), do 36 (625), hit 57 (625), hit 59 (655) |
| 0 | do 24 (shares do 23's address), do 37, hit 5, hit 6, hit 11, hit 23 |

## Randomness

| Body | Re-seed | Draws, in order |
|---|---|---|
| 17 | §R9.3 helper: x + elapsed | helper's two rolls |
| 28 | data +0x28 | `roll(2r + 1)` × 2 (x first); low word saved |
| 34 | missile x | `roll(n)` (one step) |
| 35 | data +0x28 | one step, bit 0; low word saved |
| 58 | missile x | `roll(2r + 1)` × 2 (x first) |
| hit 2, do 6, hit 36, hit 10, hit 16, hit 18, hit 26, do 14, hit 27, hit 52 | — | none of their own (do 14's skill function may draw: skills spec) |
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

## Test vectors

| Input | Expected | Source |
|---|---|---|
| chaos ice a = 8, b = 0, bit 1 | a' = 8, b' = 2 (path target x + 8, y + 2) | synthetic, §4 |
| chaos ice a = 8, b = 0, bit 0 | a' = 8, b' = −2 | synthetic |
| chaos ice a = 1, b = 1, bit 1 | a' = (4 − 1) / 4 = 0 → 1, b' = 5 / 4 = 1 | synthetic |
| cairnstones, frames left 150, data +0x28 = 0 | no portal (150 > 140); sparks if elapsed even | live row, §1 |
| baal taunt control, elapsed 24 | body 3 only, no draw | live row, §3 |
| plaguejavelin hit (`sHitPar1` 1, `sHitPar2` 2) | 8 clouds at RX/RY 0, 2, …, 14, then 15 at 1…15; 23 creations | live row, §6 |
| rancidgasepotion hit (`sHitPar1` 0) | 8 clouds, offsets (0, 2), (2, 2), (2, 0), (2, −2), (0, −2), (−2, −2), (−2, 0), (−2, 2) | live row, §6 |
| meteorcenter hit, L = 3 | 18 meteorfire, range field 60, first at (x + 2, y − 2) | live row + skills Meteor, §9 |
| catapult meteor ball (`sHitPar2` 0) | step 1: 18 pieces | live row, §9 |
| bonespirit, s = 2, frames left 0, no target in range 15 | total = left = 128 (`LevRange` 0); data +0x28 = 6; return 4 | live row, §11 |
| skill_range(Meteor, 0) | 0 (range field not set; table range) | §9 |
| grimwardsmallstart expiry, `calc1` 3 | grimwardsmall with range 5 | synthetic, §14 |
| grimwardsmall, elapsed 12, k > 0, L > 0 | skill server-do 30 runs with the missile as caster | live row, §15 |
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
  `0x005C37C0`, dumped), `0x005ABA00`, `0x005AC940`; table
  pointers checked (`disasm.py xref`: server-hit table `0x0073C840` +
  4 × index, server-do `0x0073C768` + 4 × index); offset tables dumped
  from `Game.exe` (`0x006E24D0`–`0x006E25DF`, `0x006E2A38`–`0x006E2A77`). Live rows listed per
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
3. Plague ring clouds get velocity `Param1` << 7 << 8 before the 75 %
   cut (49 152 for `Param1` 2; a `Vel` 24 javelin flies at 4 608): record Plague
   Javelin and compare the cloud positions per tick.
4. Blade Fury contacts create class-0 (arrow) missiles (§16): record a
   Blade Fury hit and check for the eight extra missiles and their
   damage.
