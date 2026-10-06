# Spec: Missiles — server-do and server-hit bodies (continued)

- **Status:** draft: server-do 17, 28, 34, 35 and server-hit 58 read
  from the 1.14d `Game.exe` disassembly (addresses per section, register
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

## Inputs

| Name | Type | Source |
|---|---|---|
| missile unit | type 3 | ECX game, EDX missile (server-do); + stack unit (server-hit, unused by 58) |
| missiles.txt record | 0x1A4 bytes | data tables +0xB64, count +0xB6C |
| skills record | 0x23C bytes | `aurarangecalc` +0x64, `calc4` +0x144 (server-do 28) |

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

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| cairn portal object | class 60, destination `Param4`, last argument 1 | `0x005A9930` |
| volcano record flags | 0x520 | `0x005AFB80` |
| chaos ice turn | (4a ∓ b) / 4, (4b ± a) / 4 | `0x005B0640` |
| taunt lightning flags | 0x20 | `0x005ACDF0` |
| missiles.txt | `Param1..5`, `sHitPar1`, `SubMissile1..3`, `HitSubMissile1`, `Range` | `data/fields.tsv` |
| skills.txt | `aurarangecalc` +0x64, `calc4` +0x144 | `data/fields.tsv` |

## Randomness

| Body | Re-seed | Draws, in order |
|---|---|---|
| 17 | §R9.3 helper: x + elapsed | helper's two rolls |
| 28 | data +0x28 | `roll(2r + 1)` × 2 (x first); low word saved |
| 34 | missile x | `roll(n)` (one step) |
| 35 | data +0x28 | one step, bit 0; low word saved |
| 58 | missile x | `roll(2r + 1)` × 2 (x first) |

Created missiles draw on their own seeds (`missiles.md` §R2.3).

## Edge cases & original bugs

1. Server-do 34 and server-hit 58 re-seed from the position: equal
   positions give equal choices.
2. Server-do 17 marks the portal done (data +0x28 := 1) before creating
   it; a failed creation is not retried.
3. Server-do 28 with no owner skips the throw but still flies.

## Test vectors

| Input | Expected | Source |
|---|---|---|
| chaos ice a = 8, b = 0, bit 1 | a' = 8, b' = 2 (path target x + 8, y + 2) | synthetic, §4 |
| chaos ice a = 8, b = 0, bit 0 | a' = 8, b' = −2 | synthetic |
| chaos ice a = 1, b = 1, bit 1 | a' = (4 − 1) / 4 = 0 → 1, b' = 5 / 4 = 1 | synthetic |
| cairnstones, frames left 150, data +0x28 = 0 | no portal (150 > 140); sparks if elapsed even | live row, §1 |
| baal taunt control, elapsed 24 | body 3 only, no draw | live row, §3 |

## Provenance

- 1.14d `Game.exe`: `0x005AF240`, `0x005A9930`, `0x005AFB80`,
  `0x005B04A0`, `0x005B0640`, `0x0064A780`, `0x0064A760`,
  `0x005ACDF0`; Ghidra decompile read first, every call's arguments
  checked in the disassembly (`tools/ghidra/disasm.py fn`). The
  `0x0056D130` argument order matches `world/quests.md` (class 59 / 60
  portal calls).
- Live `patch_d2` missiles.txt rows 288, 479, 546, 569, 654.
- D2MOO 1.10f `MissMode.cpp` (`MISSMODE_SrvDo17_CairnStones`, …): names
  only; the rules above are 1.14d reads.

## Open questions

1. No recording: record the Cairn Stones portal opening, Baal's taunt
   in the Worldstone Chamber and a Royal Strike, and compare created
   missiles and their ticks.
2. Flag 0x100 of the volcano record is not among the flags
   `0x0059FA30` reads (`missiles.md` §R2.1): confirm it is ignored.
