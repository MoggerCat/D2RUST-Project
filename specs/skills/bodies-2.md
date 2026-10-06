# Spec: Skills — Start and do function bodies, batch 3

- **Status:** draft: every body and helper below read from the 1.14d
  `Game.exe` disassembly (`py tools/ghidra/disasm.py fn|at`; several
  entries have no Ghidra function, `use.md` Open question 1); D2MOO 1.10f
  `SkillAma.cpp`, `SkillSor.cpp`, `SkillNec.cpp`, `SkillPal.cpp`,
  `SkillBar.cpp`, `SkillDruid.cpp`, `SkillAss.cpp`, `MonsterMode.cpp`
  compared, differences noted. No recording covers these bodies yet
  (Open questions).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::skills::use_::bodies` (same module as
  `bodies.md`; `functions.tsv` rows whose note says `bodies-2.md`)
- **Related specs:** `skills/bodies.md` (conventions §1, helpers §2 and
  §6, earlier bodies); `skills/use.md` (start / do cores §5.3, §5.4);
  `skills/levels.md`; `combat/hit.md` §4; `combat/damage.md`;
  `missiles/missiles.md` §R2; `monsters/init.md` §8; `items/generation.md`
  §3; `items/treasure.md` §7; `sim/pathing.md` §2, §11; `sim/pets.md`;
  `skills/functions.tsv`.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 43–52 |
| Inputs | 53–57 |
| Outputs / state changes | 58–63 |
| Rules | 64–65 |
|   1. Conventions | 66–77 |
|   2. Shared helpers, batch 3 | 78–652 |
|   3. Bodies, required level 1 | 653–828 |
|   4. Bodies, required level 6 | 829–1038 |
|   5. Bodies, required level 12 | 1039–1212 |
|   6. Bodies, required level 18 | 1213–1511 |
|   7. Bodies, required level 24 | 1512–1797 |
|   8. Bodies, required level 30 | 1798–2034 |
| Constants & data dependencies | 2035–2069 |
| Randomness | 2070–2108 |
| Edge cases & original bugs | 2109–2178 |
| Test vectors | 2179–2216 |
| Provenance | 2217–2232 |
| Open questions | 2233–2260 |
<!-- /index -->

## Summary

Batch 3 of the skill function bodies: the 85 `srvstfunc` / `srvdofunc`
slots that exactly one class skill uses (Constants, "Batch 3 order"),
ordered by that skill's required level, then class (Amazon, Sorceress,
Necromancer, Paladin, Barbarian, Druid, Assassin), then skill id, start
before do. Conventions, record layouts and the helpers of batches 1–2
are those of `bodies.md` and are linked, not restated; new shared helpers
are in §2.

## Inputs

As `bodies.md` (game, unit, skill, level L, `skills.txt` record R, target
T, frame F, difficulty).

## Outputs / state changes

As `bodies.md`: damage records and combat entries, missiles, summons,
state lists, timers, items, unit flags, used-entry params, and the value
the start / do core reads.

## Rules

### 1. Conventions

`bodies.md` §1 applies unchanged: calling convention (ECX game, EDX unit,
stack skill, L; `ret 8`), R, T = `target(game, unit)` (§2.1), `eval(c)`,
positions, state and list operations, "timer 12 at e", "anim refresh".
Helpers of `bodies.md` are cited as `bodies.md` §n; helpers of this file
as §2.n. E = the used skill entry (`0x00620250`); its params are at +0x18
(param 1, `0x00644560` set / `0x006444A0` get), +0x1C, +0x20.
"Rewind(p)" = `0x0056E210(unit, p)` → `0x00553B10(game, unit, p)`
(`sim/units.md` §4.2 variants). "Zeroed record" = a 0x70-byte damage
record (`combat/damage.md` §1) filled with 0.

### 2. Shared helpers, batch 3

#### 2.1 Monster mode damage `0x005A4F50`

`mode_damage(unit, mode)` (ECX unit, EDX mode; D2MOO `sub_6FC627B0`).
Rewrites a monster's base damage stats for the attack mode about to be
used.

1. Unit none, not a monster (type 1), no monster data (+0x14) or no
   monstats record (data +0x00) → nothing.
2. S = minion skill damage {min, max, to-hit} (§2.2), zero by default.
3. List = the unit's stat list with flag 1 (`0x00625760(unit, 1)`, the
   base list of `monsters/init.md`); none → nothing. Remove all its stats
   (`0x00627340`, `sim/stat-lists.md`).
4. d = difficulty (game +0x6D); o = L-flag (game +0x6A ≠ 0 or game +0x74
   ≠ 0), as in `monsters/init.md` §8.1. lv = the unit's `level(12)`.
5. mult = 0. The unit's alignment (`0x006259B0`) is 0 (evil): n =
   max(`monster_playercount(100)`, 1); mult = `0x005A4F20(game, n)`: d = 0
   or n < 2 → 0; n < 9 → table `0x006E21F4`[n] = (0, 0, 8, 16, 24, 32, 40,
   48, 56)[n]; else 8n − 16.
6. Stats by level (`monsters/init.md` §8.1, `0x006538A0(class, o, d, lv,
   flags, out)`, out zeroed, 0x38 bytes):

   | mode | flags | min | max | to-hit |
   |---|---|---|---|---|
   | 5 (A2) | 0x10 | A2 min + S.min | A2 max + S.max | TH + S.to-hit |
   | 7, 8 (SC, S1) | 0x20 | S1 min + S.min | S1 max + S.max | TH + S.to-hit |
   | other | 8 | A1 min + S.min | A1 max + S.max | TH + S.to-hit |

   With `noRatio` the columns are monstats `A2MinD`/`A2MaxD`/`A2TH`
   (`S1…`, `A1…`) for d; otherwise pct(monlvl `DM` / `L-DM` for d, the
   monstats value, 100) and TH from monlvl `TH` / `L-TH` (flag 8 row of
   §8.1, the same rule for flags 0x10 and 0x20).
7. Expansion game (game +0x70 ≠ 0): mult ≠ 0 → min, max and to-hit each
   += trunc(v × mult / 128). Classic: d ≠ 0 and monstats `Align` (+0x4C)
   ≠ 1 → min := trunc(10·min / 12), max := trunc(10·max / 12), to-hit :=
   trunc(10·to-hit / 15).
8. List set 21 `mindamage` := min, 22 `maxdamage` := max, 19 `tohit` :=
   to-hit (`0x00627150`).
9. For i = 0, 1, 2 (element slots 1–3): skip unless `El{i+1}Mode` (+0xFE
   + i, u8) ≠ 0 and = mode, and p = `El{i+1}Pct` for d (+0x104 + 3i + d,
   u8) ≠ 0. p < 100: one draw `roll(100)` on the **unit's** seed
   (`0x0045C390`, `rng.md` §3); r ≥ p → skip. Then:
   1. Stats by level with flags 0x40 + i (class from `0x00451F60`). The
      0x40 bit always selects the **El1** columns (0x41 / 0x42 only add
      the HP / AC outputs): min = pct(monlvl `DM`/`L-DM` for d,
      `El1MinD` for d, 100), max likewise from `El1MaxD`, len =
      `El1Dur` for d. A `noRatio` class gets min = max = 0 (its raw
      values land in the S1 output slots) and len = `El1Dur` (Edge case
      1).
   2. Expansion with mult ≠ 0: min, max, len each += trunc(v × mult /
      128).
   3. t = `El{i+1}Type` (+0x101 + i). t = 10 (`rand`): t = roll(5) + 1
      (one more draw on the unit's seed); len = 0 → 25.
   4. By t (`elemtypes`), list set: 1 fire 48 := min, 49 := max; 2
      lightning 50, 51; 3 magic 52, 53; 4 cold 54, 55, 56 `coldlength`
      := len; 5 poison 57 := 10·min, 58 := 10·max, 59 `poisonlength` :=
      2·len; 6 life drain 60, 61; 7 mana drain 62, 63; 8 stamina drain
      64, 65; 9 stun 66 `stunlength` := len; 11 burn 316 `burningmin`,
      317 `burningmax`, 315 `firelength` := len; other t: nothing.

1.14d differs from D2MOO 1.10f: mode 6 (block) takes the A1 row (D2MOO:
S1); the El1-column and `noRatio` bugs of step 9.1 are in the 1.14d
`0x006538A0`.

#### 2.2 Minion skill damage `0x0056E050`

`minion_damage(unit, &S)` (ECX unit; `ret 4`): unit a monster with a
minion owner O (`0x0058F0D0`), a monstats record, and k = `SkillDamage`
(+0xA8, i16) with 0 < k ≤ skills count (the count itself is accepted):

1. lvl = `0x00644AA0(O, k)`: O's highest entry of k (`levels.md`
   `highest_entry`) none → 0; else its base level (+0x28), plus
   `bonus_level(O, k)` when the entry's owner item GUID (+0x34) is −1;
   negative → 0; capped at the maximum character level
   (`0x00611830(0)`).
2. S.min = `phys_min(O, k, lvl, 1)` >> 8; S.max = `phys_max(O, k, lvl,
   1)` >> 8 (`levels.md` §3.3); S.to-hit = `to_hit(O, k, lvl)`
   (`levels.md` §5).

Otherwise S stays as given (zero).

#### 2.3 Jitter callback `0x005C9290`

Missile init callback (`missiles.md` §R2.3 step 21; ECX missile, EDX the
record's callback argument a):

1. n = total frames (missile data +0x0E, `0x0064A300`). n ≥ 78 → n = 77;
   total and current frames := 77 (`0x0064A2B0`, `0x0064A330`).
2. P = the missile's path (+0x2C). Missile seed (+0x20) := `init_low(P
   target x + a)` (`0x00648A00`, `0x00650E40`; `rng.md` §3).
3. Path type := 10 (`0x00648CF0`; charged bolt, `sim/pathing.md` §2,
   §11.2); path step counts (+0x90, +0x91) := n (`0x00648E70`, capped
   77); compute the path (`0x00649970(P, missile, 0)`).

Every missile of one cast gets a different seed (a = 0, 1, …) and so a
different jittered path.

#### 2.4 Potion code `0x005D8100`, item drop `0x0056DAB0`

`potion_code(unit, game, skill)` (EAX unit; stack game, skill; `ret 8`):

1. Room of the unit (`0x00620BB0`) none → 0. a = act of the room's level
   (`0x0061A1B0`, `0x006427F0`, `drlg/levels.md`); a > 4 → 0.
2. i = a + 5·difficulty.
3. One draw on the **unit's** seed: r = `lo' mod 100` (unsigned).
4. c = 1 (mana) when r < `Param3` (+0x150, `0x004EFC50`); else c = 2
   (rejuvenation) when r < `Param3` + `Param4` (+0x154, `0x004EFC80`);
   else c = 0 (healing).
5. i ≤ 15 (`0x00741B54`) → the code in row i, column c of the 4-byte code
   table `0x00741B58` (rows of 3 codes); else 0.

| i (act + 5·difficulty) | c = 0 | c = 1 | c = 2 |
|---|---|---|---|
| 0 | `hp2` | `mp2` | `rvs` |
| 1, 2 | `hp3` | `mp3` | `rvs` |
| 3, 4, 5 | `hp4` | `mp4` | `rvl` |
| 6 | `hp4` | `mp5` | `rvl` |
| 7…14 | `hp5` | `mp5` | `rvl` |

(Row 15 would read past the table; i ≤ 14 always.)

`drop_item(game, U, code, quality)` = `0x0056DAB0` (ECX game, EDX U;
`ret 8`): item = items index of code (`0x00633680`); −1 → nothing. Floor
spot: `0x00555DA0(room of U, U's position (0x00620870), &out, size 1,
fallback 1)` as `items/treasure.md` §7 step 2; none → nothing. Request
(`items/generation.md` Inputs, zeroed): unit U, game, ilvl =
`0x00558200(U)` (`items/treasure.md` §7 step 3), item, spawn mode 3, x,
y, room from the spot, init flags 1, format = game +0x78, quality :=
quality. Create (`0x00558D90(game, request, 0)`); the result is not
read.

#### 2.5 Kick damage `0x005D54B0`

`kick_damage(game, record, T, skill, L)` (unit in EBX; `ret 0x14`).
Rolls the physical part itself and then runs `fill` for the rest.

1. Before-roll charges `0x005D3AC0(unit, record)` (`bodies.md` §2.14).
   Hit flags |= 3 (skip the physical and the weapon roll in `fill`).
2. mn = `phys_min(unit, skill, L, 0)`, mx = `phys_max(unit, skill, L, 0)`
   (`levels.md` §3.3). E = record +0x0C (enhanced damage %). mn' = mn +
   pct(mn, E, 100); mx' = mx + pct(mx, E, 100) (`combat/damage.md` §0,
   inlined with the same overflow branches).
3. Kick stats `0x00646280(unit, &kmin, &kmax, &E2)` with kmin = kmax = 0,
   E2 = E (`levels.md` §3.5 "Kick damage"). kmin <<= 8, kmax <<= 8; kmin'
   = kmin + pct(kmin, E2, 100); kmax' = kmax + pct(kmax, E2, 100).
4. lo = kmin' + mn'; hi = kmax' + mx'; d = hi − lo. d < 1 → x = 0 (no
   draw); else one draw on the unit's seed, x = `roll(d)` (`rng.md` §3:
   power of two → `lo' & (d − 1)`, else `lo' mod d`). Physical (+0x08)
   += lo + x.
5. `roll_elemental(unit, record, skill, L)` (`levels.md` §3.6).
6. Inventory (unit +0x60) none → return. W = the weapon in use
   (`0x0063BEF0`); O = the item in the other hand (body location 5 when
   W's location (`0x00627D40`) is 4, else 4; `0x0063BDE0`), none without
   W. W counts only when of item type 45 (`0x00629BB0`), usable
   (`0x0062A4E0`) and active on the unit (`0x00625820(W, 0)`); then its
   stat lists are toggled off (`0x00627910(unit, W, 0)`), else W :=
   none. O ≠ W with the same three tests → toggled off too.
7. `fill(game, unit, T, record, 1, 128)` (`combat/damage.md` §3.1);
   after-roll charges `0x005D3BA0(unit, record)`.
8. W → toggled back on (`0x00627910(unit, W, 1)`). O is **not** toggled
   back (Edge case 2).

#### 2.6 Kick hit `0x005D5880`

`kick_hit(game, T, R, L, knock)` (unit in EAX, skill in EDI; `ret 0x14`):

1. bonus = `to_hit(unit, skill, L)` + the unit's
   `progressive_tohit(325)`.
2. Zeroed record; result = `melee_result(game, unit, T, bonus, 0)`
   (`combat/hit.md` §4).
3. Hit (result & 1): knock ≠ 0 → result |= 0xC (get-hit, knockback).
   Enhanced damage % (+0x0C) := `Param1` + (L − 1)·`Param2` (+0x148,
   +0x14C) when the skill is valid and L ≥ 1, else 0. `kick_damage(game,
   record, T, skill, L)` (§2.5).
4. Hit class (+0x60) := 1. `start_combat(game, unit, T, record, SrcDam
   of R, or 128 when 0)` (`combat/damage.md` §3).

#### 2.7 Knockback chance by target kind

Used by §3.10 and §3.12. The column is chosen by T's kind, then
evaluated with (skill, L):

| T | column |
|---|---|
| a player, or a hireling (`0x0063EE90`) | `calc4` (+0x144) |
| a monster that is a boss (`0x0063E9F0`) | `calc3` (+0x140) |
| a monster with type flag 8 (`0x005A0180(T, 8)`, unique) | `calc2` (+0x13C) |
| any other | `calc1` (+0x138) (§3.10) or no formula, chance 100 (§3.12) |

Both bodies first compute L' = clamp(T `level` + L − T `level`): both
reads are of T, so L' = L for every L (vestigial; reproduce by passing
L).

#### 2.8 Uninterruptable `0x005544B0`

`set_uninterruptable(unit, v)` (ECX unit, EDX v): state 54
(`uninterruptable`) := v (`0x00639DB0`). A null unit is a fatal
assertion. The unit's game (+0x80) none → done. v = 0 and the unit has
state 92 (`death_delay`): state 92 off; K = `0x00553010(game, unit)`; K
→ reaction `0x0057CEE0(game, K, unit, record)` with a zeroed record whose
result is 2 (will die); no K → kill `0x0057CCB0(game, unit, 0, 1)`
(`combat/damage.md` §7.2). Then a monster has its type-2 timers deleted
(`0x00540E60(game, unit, 2, 0)`).

#### 2.9 Nearest accepted unit `0x0056BBC0`

`nearest(game, unit, r, test)` (ECX game, EDX unit; stack r, test):
`scan_unit(game, unit, 0, 0, r, 0x8783, cb, ctx, noaura 0)`
(`bodies.md` §2.12) with ctx {best none, best d² 0x7FFFFFFF, the unit's
position, test}. Per scanned U: `test(game, U)` = 0 → skip; d² from the
unit's position; d² < best d² (strictly) → best := U. Return best (the
first of equally near units in scan order).

#### 2.10 Scatter `0x005D5BF0`

`scatter(game, unit, m, n, r, skill, L)` (ECX game, EDX unit; stack
owner, m, n, r, skill, L; `ret 0x18`; owner = origin = unit):

1. Target position (`bodies.md` §2.4) → (tx, ty); failure → 0.
2. The **unit's** seed := `init_low(tx)` (`0x00650E40`, `rng.md` §3).
3. Record (`missiles.md` §R2.1, zeroed): flags 0x420 (target absolute,
   frames from distance), owner, origin, class m, skill, L.
4. n ≤ 1 or r < 2: target := (tx, ty); create once. Return 1.
5. Else n times: x = tx − r + `roll(2r)`, y = ty − r + `roll(2r)` (two
   draws on the unit's seed, `0x0045C3E0`, x first); target := (x, y);
   squared distance from the unit's position ≥ 4 → create; else nothing.
   Return 1.

The re-seed makes the scatter pattern a function of the target x only.

#### 2.11 Progressive count `0x005D3DA0`

`prog_count(unit, skill, L)` (ECX unit, EDX skill; stack L): R invalid →
0. R `progressive` (flags bit 2), `aurastate` valid, `aurastat1` valid
and the unit has a list of `aurastate`: n = list[`aurastat1`] clamped to
1…3 → `eval(prgcalc_n)` (+0x38, +0x3C, +0x40). Every other case:
`eval(prgcalc1)`.

#### 2.12 Claw hit `0x005D6200`

`claw_hit(game, T, L)` (skill in EBX, unit in ESI; `ret 0xC`):

1. R invalid → nothing.
2. Zeroed record; result = `melee_result(game, unit, T, to_hit(unit,
   skill, L) + progressive_tohit(325), 0)`.
3. Hit: hit flags |= 2; before-roll charges `0x005D3AC0(unit, record)`;
   enhanced damage % += `eval(calc1)`; `EType` ≠ 0: c = `eval(calc4)`;
   conversion % := c; c > 0 → conversion element := `EType`;
   `roll_elemental(unit, record, skill, L)`. `fill(game, unit, T,
   record, 0, SrcDam)` (`combat/damage.md` §3.1; 0 is treated as 128);
   after-roll charges `0x005D3BA0`.
4. `start_combat(game, unit, T, record, 128)` (also on a miss; hit flag 2
   skips its own fill).

#### 2.13 Leap helpers

Used by Leap (§4.6, §4.7) and Leap Attack. E flags (+0x0C, `0x00644660`
set, `0x006446A0` get) carry the phase: 0x80 launch, 0x1101 in flight
(bit 0x1 = moving skill, `use.md` §5.2), 0x200 landed.

- **Room test** `0x005D9CE0(unit, x, y)` (skill in EAX): R invalid, the
  unit's room none, or no room at (x, y) (`0x00463740` from the unit's
  room) → 0. R lacks `InTown` (flags bit 8) and that room is in town →
  0. Else 1.
- **Clamp** `0x005D9A10(unit, L, &x, &y)` (skill in EAX): R invalid → 0.
  (ux, uy) = the unit's position; r = `eval(aurarangecalc)`; d =
  distance to (x, y) (`0x006417F0`), < 2 → 1. r < d → x := (x − ux)·r / d
  + ux, y := (y − uy)·r / d + uy (signed, truncating). a = clamp(y − uy,
  −2, 2), b = clamp(ux − x, −2, 2). Up to three candidates (cx, cy): (x,
  y), then (x + a, y + b), then (x − a, y − b). For each: free point
  `0x0064E7B0(room at (cx, cy), &(cx, cy), unit size (0x00620510), mask
  0x1C09, fallback 1)` (`sim/path-placement.md` §7) gives room R'; R'
  found, `0x0064D910(R', cx, cy, the unit's path pattern (0x00649180),
  0x1C09)` = 0 (no cell collides), the line from (ux, uy) to (cx, cy) is
  clear (`0x00645910` → `0x0064E260(R', from, to, 0x804)` = 0), and the
  distance to (cx, cy) ≤ r → x, y := cx, cy; return 1. After three
  failures → 0.
- **Monster pre-hit** `0x005D9C80(game)` (T in EAX, E in EBX, unit in
  EDI): `mode_damage(unit, 4)` (§2.1); `0x005A5490(unit, game)`: T' =
  `target(game, unit)`, zeroed record, result = `melee_result(game, unit,
  T', 0, 0)`, `start_combat(game, unit, T', record, 128)`;
  `apply_melee(game, unit, T)`; E param 3 := T type, param 4 := T GUID.
  Return 1 (0 when T or E is none).
- **Launch** `0x005D9F70(E)` (unit in EDI): path P (unit +0x2C) none → 0.
  E flags := 0x1101. P move-test mask := 0 (`0x00648CE0`), footprint mask
  := 0 (`0x00648C30`). (x, y) = E params 1, 2; either 0 → 0. P target
  point := (x, y) (`0x00648AD0`); path type := 9 (leap, `sim/pathing.md`
  §2). v = monstats `Run` (+0x34, i16) of a monster's class (no record →
  0, nothing more) or charstats `RunVelocity` (+0x41, u8) of a player's
  class; P velocity := v << 8 (`0x00648690`); compute the path
  (`0x00649970(P, unit, 0)`). Return 1.
- **Land** `0x005DA120(game, E)` (unit in EAX): P none or the unit's room
  none → 0. (x, y) = E params 1, 2. b = the unit's monstats `BaseId`
  (`0x00463860`; −1 for players or out of range).
  1. The unit stands at (x, y):
     - Player: pattern clear `0x0064EC10(room, x, y, pattern, 0x80)`
       (`sim/path-placement.md` §5.1); footprint mask := 0x80; move-test
       mask := 0x1C09; path type := 7; E flags := 0x200; delete the
       unit's type-1 timers; type-1 timer at F + 1.
     - Monster: pattern clear with mask 0x100; footprint mask := 0x100;
       move-test mask := 0x3C01; path type := 2. b = 78 (`sandleaper1`
       family): hop `0x005DA020` below. Else E flags := 0x200; frame
       count (unit +0x48) −= 0x100.
     - Both: `set_uninterruptable(unit, 0)` (§2.8); landing message
       `0x00571B70(unit, E's skill)`: state 18 (`skill_move`) off, queue
       message 0xA5 {unit type, GUID, skill} on the unit's list (+0xEC /
       +0xF0, `sim/server-messages.tsv`), queue the unit for update.
       Return 1.
  2. Not there yet: player → restart the animation at frame 10
     (`0x00553DC0(game, unit, 10)`, `sim/units.md` §4.2 variants);
     return 0. Monster → frame event index (unit +0x38 bits 8+,
     `0x006212C0`) := 8 for b = 78, for b = 540 (`ancientbarb1` family)
     12 when the distance to (x, y) < 2 else 10, otherwise 0; frame count
     += 0x100; return 0.
- **Sandleaper hop** `0x005DA020(game, x, y)` (E in EAX, unit in EDI): E
  or P none → 0. K = the unit of (E param 3, E param 4) (`0x00552F60`);
  none → 0. E flags := 1; frame event index := 12; frame count += 0x100;
  snap P's position to its sub-tile centre (`0x00650590`); P step counts
  := 5 (`0x00648E70`); `0x00649070(P, 1)`; P target unit := none
  (`0x00648B90`). Point (3x − 2·ux, 3y − 2·uy) (ux, uy the unit's
  position): a room there in town → 0. P target point := it; path type
  := 8; `0x00648E40(P, 5)`; compute the path. Return 1.

#### 2.14 Weapon wear `0x005DAA40`

`wear(game, chance, amount)` (W in EBX, unit in ESI; `ret 0xC`):

1. W not of item type 45 (`weap`) → nothing.
2. W a stack (`0x006289F0`): its `quantity(70)` > 0 → one draw on the
   unit's seed, r = `lo' mod 100`; r < chance → `dec_quantity(game,
   unit)` (`bodies.md` §2.5).
3. Else W has durability (`0x00629930`): one draw as above; r < chance:
   d = W's `durability(72)` − amount. d > 0 → set 72 := d; message 0x3E
   for stat 72 to the unit's client (`0x005531C0`, `0x0053D130(client,
   W, 1, 72, d, 0)`). d ≤ 0 → `0x0055F850(game, unit, W)` (zero
   durability, items spec).

#### 2.15 Charge helpers

- **Swing** `0x0056E230(game, unit, T)` (all three non-null, else a
  fatal assertion): mode := 1 (`0x00553570(game, unit, 1)`); used skill
  := none (`0x00620210`); E0 = the unit's entry of skill 0 (Attack,
  `0x006439B0(unit, 0, −1)`). Player: mode request, unit form
  (`0x00580A70`, `sim/pathing.md` §1.2) with E0, mode 7 (A1), T's type /
  GUID, re-entry 1. Monster: mode request 4 at T (`0x005A7E60`,
  `0x005A7C20(game, &req, 1)`). Return 1.
- **Hit frame** `0x005CF8C0(unit)`: the used skill's sequence
  (`0x006633B0` frame list of 6-byte records, `0x006633F0` count;
  `sim/units.md` §4.2 sequences); none or count ≤ 0 → 7. The loop tests
  the event byte (+5) of **record 0** at every step: 0 → −1; non-zero →
  7 (Edge case 12). Charge (`seqnum` 4) has event 1 in record 0: 7.

#### 2.16 Missile burst `0x005A9370`

`burst(game, owner, origin, m, skill, L, step2, step1, loops)` (ECX
game, EDX owner; `ret 0x1C`). m without a missiles record, owner none or
origin none → 0.

1. Record (`missiles.md` §R2.1, zeroed): flags 0x17 (position given,
   target relative, velocity given, velocity fixed point); loops > 0 →
   flags 0x1F, loops (+0x34) := loops. Owner, origin, class m, position
   = origin's position, skill, L.
2. Ring 1: velocity := missile `Param1` (missiles +0x38) << 7. For i = 0,
   step1, 2·step1, … while i < 16: target offset (X[i], Y[i]); create.
   (step1 < 1 is treated as 1.)
3. step2 ≠ 0: ring 2: velocity := `Param2` (+0x3C) << 7; for i = 0,
   step2, … while i < 15: offset (X[i + 1], Y[i + 1]); create.
4. Return 1.

Offset tables (16 entries, `0x006E2510` X, `0x006E24D0` Y): X = 0, 1, 2,
2, 2, 2, 2, 1, 0, −1, −2, −2, −2, −2, −2, −1; Y = 2, 2, 2, 1, 0, −1, −2,
−2, −2, −2, −2, −1, 0, 1, 2, 2.

#### 2.17 Skill result `0x0056E680`, element length `0x0056C840`

- `skill_result(game, unit, T, R, skill, L, record, range)` = `0x0056E680`:
  R's `ResultFlags` (+0x12E) bit 1 set → result := `ResultFlags` (no
  roll). Else result = `melee_result(game, unit, T, to_hit(unit, skill,
  L), range)`; hit → result |= `ResultFlags`. The result is stored in
  the record (+0x04) and returned.
- `set_len(unit, record, len, skill)` = `0x0056C840`: by the skill's
  `EType`: 4 cold length (+0x30), 5 poison length (+0x2C), 9 stun length
  (+0x44), 11 burn length (+0x18), 12 freeze length (+0x34) := len;
  other types nothing.

#### 2.18 Plague `0x005C7DB0`

`plague(game, unit, T, len, skill, L)` (ECX game, EDX unit; `ret 0x10`):
L < 1 → 0. Infect `0x005C7B10`; infected → spreader `0x005C7CE0`. Return
1.

- **Infect** `0x005C7B10(game, unit, T, len, skill, L)`: R invalid or
  `auratargetstate` not in 0…states count (the count itself accepted) →
  0. T already has a list of the state → 0. e = F + len. Alloc (game pool,
  flags 2, expire e, owner T's type / GUID; failure → 0), state set,
  callback `0x0056E900`, attached to T, state on, expiry e, timer 12 at
  e on T; `aura_fill(unit, list, R, skill, L)` (`bodies.md` §2.6;
  formulas on the caster). Return 1.
- **Spreader** `0x005C7CE0(game, unit, len, L)` (skill in EAX, T in
  EBX): R invalid → nothing. m = `prog_missile(unit, skill)`; m ≤ 0 →
  nothing. Record (zeroed): flags 0x8000 (range given), owner = origin
  = T, class m, skill, L, range := len. Created M: data +0x28 := the
  caster's type (6 when none), data +0x2C := its GUID (−1 when none)
  (`0x0064A710`, `0x0064A760`).

#### 2.19 Leap Attack helpers

- **Aim** `0x00645CA0(unit, T, &x, &y)`: T present and in melee range →
  0. Point (px, py) = T's position, or the path's target point
  (`0x00648A00`, `0x00648A10`) without T. d = distance from the unit
  (`0x006417F0`); d = 0 → 0. (x, y) = (px + 2(px − ux) / d, py + 2(py −
  uy) / d) (signed, truncating). The unit's room none → fatal assertion.
  (x, y) collides (`0x0064D910(room, x, y, pattern, 0x1C09)`) → free point
  `0x0064E7B0(room, &(px, py), unit size, 0x1C09, 0)` (searched from the
  original point), none → 0, (x, y) := it. The line from the unit to
  (x, y) is clear (`0x00645910`, mask 0x804, the free point's room or the
  unit's) → out (x, y); return 1 when a room exists at (x, y) and is not
  in town (`0x00645C60`), else 0. Line blocked → 0.
- **Pick** `0x005DA490(game, unit)` (E in EBX): E none → none. T =
  target in melee range → K = T. Else K = the unit of (E param 3, E param
  4) unless param 3 = 6; none → K = `next_unit(game, unit, 0, 0, melee
  range + 4, 0x20003, −1, null)`; none → E param 3 := 6, return none. E
  params 3, 4 := K type, GUID; return K.
- **Strike** `0x005DA660(game, skill, L)` (unit in EAX): R invalid or E
  (the unit's entry of the skill) none → 0. K = Pick; none → 0. Zeroed
  record; result = `melee_result(game, unit, K, to_hit(unit, skill, L),
  0)`. Hit: result |= 8 (knockback); hit class := the weapon hit class
  (`0x00623C20`); hit flags |= 0x20; enhanced damage % := `eval(calc1)`;
  `EType` conversion as §3.1; `roll_elemental`; `start_combat(game,
  unit, K, record, SrcDam or 128)`; `apply_melee(game, unit, K)`;
  `srvoverlay` (+0x4E) in 1…overlay count → overlay on K; K's list of
  state 21 (`stunned`) → detach and free. Return 1 (also on a miss).

#### 2.20 Base weapon roll `0x005CFD20`

`base_roll(unit)` (unit in ESI): no weapon (`0x00535BC0`): mn =
max(`mindamage(21)`, 1), mx = max(`maxdamage(22)`, 2). Weapon: wield type
(`0x0063D340`) 2 → stats 23 / 24, else 21 / 22. mn <<= 8, mx <<= 8; mn
< 1 → 256; mx ≤ mn → mx = mn + 256. Return mn + `roll(mx − mn)` (one
draw on the unit's seed; none when mx − mn < 1).

#### 2.21 Unit counts and the 0xA3 message

- `next_unit(game, source, x, y, r, f, g, &count)` (`0x0056BD10`,
  `missiles.md` §R9.6): the optional last argument receives the number of
  units the scan accepted (every accepted unit adds 1, whichever of the
  two kept slots it updates).
- `count_units(game, source, x, y, r, f)` = `0x0056BC80`: `scan_unit`
  with f | 0xA783, noaura 0; returns the number of accepted units.
- `msg_a3(unit, T, skill, L, x, y, v)` = `0x00571AA0`: queue a 0x28-byte
  record on the unit's message list (+0xEC / +0xF0) and queue the unit
  for update: +0x04 0xA3, +0x08 v (u8), +0x0A skill (u16), +0x0C L
  (u16), +0x10 / +0x14 the unit's type / GUID (6 / −1 without one),
  +0x18 / +0x1C T's type / GUID (GUID −1 without T), +0x20 x, +0x24 y
  (S→C 0xA3, `sim/server-messages.tsv`).

#### 2.22 Pack and alignment helpers

- **Leave pack** `0x0056E580(T)`: T's minion owner (`0x0058F0D0`) is a
  monster: T itself → `0x0058F260(T)` (the pack is dissolved: every
  minion of T's list is released); another unit → `0x0058F160(T)` (T is
  removed from its leader's minion list). `monsters/ai.md` §8.
- **Alignment** `0x005543B0(U, a, v)`: a > 2 → fatal assertion; else U's
  state 105 list gets `alignment(172)` := a (`monsters/init.md`);
  0 evil, 1 neutral (confused, attracted), 2 good (converted).
- **Target list** `0x005B1990(game, U, 0, slot)` (slot 8 or 9): U in no
  list (+0xD0 = 11), a player or monster → a node {U, 0} is **prepended**
  to game list `slot` and U +0xD0 := slot (`bodies.md` §6.3 inserts after
  the head instead). `0x005B1A90(game, U)` unlinks U and sets +0xD0 :=
  11.

#### 2.23 Conversion

Used by srvdo 79 (§7.13) and srvdo 51 (§7.19) with (T, caster, state s,
expiry e, remove callback c):

1. T's list of s, or a new one (game pool, flags 0x802, expire e, owner
   the caster; failure → return 0), state set, callback c, attached, s
   on.
2. Expiry := e; timer 12 at e on T.
3. Leave pack (§2.22); alignment := 2; `node_insert(game, T, 0, caster
   +0xD0)` (`bodies.md` §6.3); the caster's path target := none; every
   list of T with flag 8 (auras) detached and freed (repeat
   `0x00625760(T, 8)` until none). (Mind Blast clears the lists before
   the path target; no draws either way.)
4. tl = T's `level(12)`, cl = the caster's. tl ≠ 0, cl < tl and T has
   no list of state 109 (`conversion_save`): alloc (game pool, flags 0,
   expire 0, T's type / GUID; failure → skip), state 109 set, attached
   (state 109 not turned on). M = T's maximum life >> 8; h = T's
   `hitpoints(6)` >> 8. List set 176 (`conversion_level`) := tl, 177
   (`conversion_maxhp`) := M. M' = pct(M, cl, tl) << 8, at least 1; h' =
   pct(h, cl, tl) << 8 clamped to 1…M'. T base level := cl, stat 6 :=
   h', stat 7 := M'.
5. Return 1.

Remove callbacks (ECX T, EDX state):

- Conversion `0x005D01A0`: state off; alignment := 0; target list remove
  (§2.22). T's list of 109 present: lvl = list[176], M = list[177]; m =
  T's maximum life >> 8, h = life >> 8; v = pct(M, h, m) when m ≠ 0, else
  1. T base level := lvl; stat 6 := v << 8; stat 7 := **M** (not shifted:
  Edge case 22); list 109 detached and freed; every flag-8 list detached
  and freed.
- Mind Blast `0x005D7310`: the same, except v is clamped to 1…M and
  stat 7 := M << 8.

#### 2.24 Frenzy helpers

- **Charge** `0x005D8C70(game, unit, skill, L)`: R invalid, E (used
  entry) none, or E param 1 = 0 (the last swing missed) → 0. `aurastate`
  valid: e = F + `eval(auralencalc)`; the unit's list of it, or a new
  one (game pool, flags 2, expire e, owner the unit, state set, callback
  `0x0056E900`, `skill_frenzy(169)` := 0, attached, state on; failure →
  0); expiry := e; timer 12 at e; mark changed; n = min(list[169] + 1,
  L); list set 169 := n; `aura_fill(unit, list, R, skill, n)`
  (`bodies.md` §2.6; formulas use the charge count as the level). Return
  1.
- **Swing** `0x005D8B10(game, unit, T, skill, L)`: R invalid or T none →
  0. Path target := T (`0x00620C10`). Zeroed record; result =
  `melee_result(game, unit, T, to_hit(unit, skill, L), 0)`. Hit: result
  |= `ResultFlags`; hit flags |= `HitFlags`; `HitClass` ≠ 0 → hit class
  := it; enhanced damage % := `eval(calc1)`; `EType` conversion as §3.1;
  `roll_elemental`. `start_combat(game, unit, T, record, SrcDam or
  128)`; `apply_melee(game, unit, T)`. E param 1 := 1 on a hit, else 0
  (when E exists). Return 1.

#### 2.25 Whirlwind helpers

- **Attack frames** `0x0062A710(unit, W)`: W must be an item of type 45
  (else a fatal assertion). The COF name of the unit's mode 7 (A1) with
  W's class (`0x0064F5B0`, `data/fixups.md`) is looked up in AnimData
  (`0x0066AA80`, `formats/animdata.md` §6); not found → 45; frames f = 0
  → fatal. Return (f << 8) / ((100 + W's `item_fasterattackrate(93)`
  (item getter) + the unit's `attackrate(68)`) × speed / 100) (signed,
  truncating).
- **Two melee weapons** `0x005D92D0` (unit in EAX): the inventory's
  items for `0x0063C050(inventory, 6)` and `(inventory, 5)` both exist
  and are of item type 46 (`mele`) → 1, else 0 (Open question 10).
- **Pacing** `0x005D9320(game, E)` (unit in EAX): a monster → one step
  of its seed, return (`lo'` & 1) xor 1. Classic game (game +0x70 = 0) →
  1. n = E param 4 (+0x24); n = 0 → E param 4 := F + 4, return 1. F < n
  → 0. d = 10 without a weapon; else by f = attack frames: f < 12 → 4, <
  15 → 6, < 18 → 8, < 20 → 10, < 23 → 12, ≤ 25 → 14, else 16. next = n +
  d; next < F → next = F + d. E param 4 := next. Return 2 with two melee
  weapons, else 1.
- **End** `0x005D9460(game, unit, E, skill, x, y)`: E flags := 0;
  `set_uninterruptable(unit, 0)` (§2.8); landing message `0x00571B70(unit,
  E's skill)` (§2.13). Path none → 0. k = 0x80 for a player, 0x100
  otherwise; x and y ≠ 0 → pattern clear `0x0064EC10(the unit's room, x,
  y, pattern, k)`. Footprint mask := k; move-test mask := 0x1C09
  (player) or 0x3C01. `aurastate` valid → state off, its list detached
  and freed. Return 1.

#### 2.26 Blade Shield pulse `0x005D7CE0`

Table slot srvdo 142 (`functions.tsv` "unused"), called directly by
srvdo 54 (§8.14). (game, unit, skill, L):

1. R invalid → 0. r = `prog_count(unit, skill, L)`; 0 → r =
   `eval(aurarangecalc)`.
2. Zeroed record D: hit flags := 2; `roll_physical`, `roll_elemental`;
   `HitClass` ≠ 0 → hit class := it.
3. Context {&D, h = `to_hit(unit, skill, L)` when `ResultFlags` bit 1 is
   clear (else 0), `ResultFlags`, `HitFlags`, `SrcDam`}.
4. `scan_unit(game, unit, 0, 0, r, aurafilter, 0x005D7C40, context,
   noaura 0)`. Return 1.

Callback `0x005D7C40` (ECX scan context, EDX unit U): copy D; result =
`0x0056E6F0(game, unit, U, ResultFlags, h, copy, 1)` (`ResultFlags`
bit 1 → `ResultFlags`; else `melee_result(game, unit, U, h, range 1)`,
on a hit | `ResultFlags`). Hit: hit flags |= `HitFlags`; `SrcDam` ≠ 0 →
`fill(game, unit, U, copy, 0, SrcDam)`; `start_combat(game, unit, U,
copy, 128)`; `apply_melee(game, unit, U)`. Return 1.

### 3. Bodies, required level 1

#### 3.1 srvdo 7 Jab `0x005DB2D0`

1. R invalid → 0. T none → 0.
2. Zeroed record. h = `to_hit(unit, skill, L)`. The unit is a monster →
   `mode_damage(unit, 8)` (§2.1).
3. Result = `melee_result(game, unit, T, h, 0)`.
4. Hit: enhanced damage % := `eval(calc1)`. `EType` ≠ 0: c =
   `eval(calc4)`; conversion % (+0x68) := c; c > 0 → conversion element
   (+0x65) := `EType`. `roll_elemental(unit, record, skill, L)`.
5. `start_combat(game, unit, T, record, SrcDam or 128 when 0)`;
   `apply_melee(game, unit, T)` (`combat/damage.md` §5.1; a second stack
   argument 0 is unread). Return 1.

Each frame event of the Jab animation runs this do once (`use.md`
§5.2); the start (srvst 5, `bodies.md` §3.5) only checks T.

#### 3.2 srvdo 17 Charged Bolt `0x005C9300`

Also PrimeBolt, BoltSentry, ImpBolt (monster rows).

1. Unit flags |= 0x40.
2. R invalid → 0. m = `prog_missile(unit, skill)` (`bodies.md` §6.6);
   not 0 ≤ m < missiles count → 0.
3. n = `eval(calc1)`.
4. Record (`missiles.md` §R2.1, zeroed): flags 0x21 (position given,
   target absolute), owner the unit, class m, position = the unit's
   (`0x0045ADF0`, `0x0045AE20`), skill, L, init callback (+0x54) =
   §2.3.
5. For i = 0…n − 1: callback argument (+0x58) := i; target position
   (`bodies.md` §2.4, re-read each time) into the record (+0x1C, +0x20);
   success with both coordinates ≠ 0 → create (`0x0059FA30`); else this
   i makes nothing.
6. Return 1 (also for n ≤ 0).

#### 3.3 srvdo 64 Sacrifice `0x005CE8E0`

Run after the start (`bodies.md` §3.7), which stored the hit in a combat
entry.

1. R invalid → 0. T none → 0.
2. p = `pair_record(unit, T)` (`bodies.md` §2.2); none → return 1
   (nothing applied).
3. Zeroed record; hit flags := 0x1000 (ignore hostility). v = min(p
   physical (+0x08), T's `hitpoints(6)`); self damage s = pct(v,
   `eval(calc2)`, 100) (`0x00483360`); physical (+0x08) := s, total
   (+0x4C) := s.
4. `apply(game, unit, unit, 0, record)` (`combat/damage.md` §5.2);
   reaction `0x0057CEE0(game, unit, unit, record)` (§7.1).
5. `apply_melee(game, unit, T)`. Return 1.

The self damage uses the stored (pre-resistance) physical roll, capped at
T's current life; calc2 = `par3` (8 %) in 1.14d.

#### 3.4 srvdo 150 Smite `0x005CE9F0`

1. R invalid → 0. T none → 0. T not in melee range (`0x00622C40(unit,
   T, 0)`) → 0.
2. Zeroed record.
3. **Player** (unit type 0):
   1. Shield: `0x0063C8F0(inventory, &S)` = 0 → 0 (`combat/hit.md`);
      item record of S's class (`0x006335F0(S +0x04)`, −1 without S)
      none → 0.
   2. mn = armor `mindam` (+0xFE, u8) << 8; mx = `maxdam` (+0xFF) << 8.
   3. The unit has state 101 (`holyshield`) with a list H: k = H's skill
      (`0x006260E0`), l = H's level (`0x00626100`); mn += `phys_min(unit,
      k, l, 1)`; mx += `phys_max(unit, k, l, 1)`.
   4. Physical (+0x08) := `bonuses(unit, get 0, item S, mn, mx,
      eval(calc1), 0, 128)` (`combat/damage.md` §3.2; one draw).
   5. st = `eval(calc2)`. `EType` ≠ 0: c = `eval(calc4)`; conversion
      as §3.1. `roll_elemental(unit, record, skill, L)`. Stun length
      (+0x44) := st.
   6. Hit flags := 2 (skip `fill`). Result = `melee_result(game, unit, T,
      0, 0)` | 1 (always a hit).
4. **Other** (monster, or no unit): `mode_damage(unit, 5)` (§2.1); stun
   length := `eval(calc2)`; result = `melee_result(game, unit, T, 0, 0)`.
5. Hit: result |= `ResultFlags` (+0x12E); hit flags |= `HitFlags`
   (+0x130).
6. Hit class (+0x60) := 0x65. `start_combat(game, unit, T, record,
   128)`; `apply_melee(game, unit, T)`. Return 1.

Draw order for a player: `bonuses` roll, `roll_elemental`, then
`melee_result` (its block / hit draws, `combat/hit.md` §4). A player's
Smite never misses but can still be blocked or avoided by the flags
`melee_result` sets.

#### 3.5 srvdo 69 Find Potion `0x005D81C0`

1. R invalid → 0. T none → 0.
2. `0x00645590(T)` (corpse test, `bodies.md` §3.9) = 0 → 0. T has state
   118 (`corpse_noselect`) → 0.
3. State 118 on for T (`0x00639DB0(T, 118, 1)`); queue T for update
   (`0x0064C040`).
4. p = `eval(calc1)`. One draw on the **unit's** seed, r = `roll(100)`
   (`0x0045C390`).
5. r < p: code = `potion_code(unit, game, skill)` (§2.4, one more draw on
   the unit's seed); `drop_item(game, T, code, 2)` (§2.4; quality 2
   normal). Return 1 (also when r ≥ p: the corpse is used up).

#### 3.6 srvdo 114 Raven `0x005C6910`

1. R invalid → 0. c = `summon_class(unit, skill, L, &mode)`
   (`bodies.md` §6.1); < 0 → 0. pt = `pettype` (+0xBE, signed); < 0 or
   ≥ pettype count → 0.
2. Unit flags |= 0x40.
3. m = spawn (`bodies.md` §6.2) {flags 0, owner unit, class c, AI state
   0, mode, x = y = 0 (target position of the owner), pt, pet max
   `eval(petmax)`}; none → 0.
4. m flags (+0xC4) &= ~0x4. m base stat 74 (`hpregen`) := 0
   (`0x00627260`).
5. `base_stats(game, unit, m, max(eval(calc2), 1), L)` (`bodies.md`
   §6.4); `skill_stats(game, unit, m, skill, L, 0)` (§6.5). Return 1.

No target-node insert and no target test (unlike `bodies.md` §8.1).

#### 3.7 srvdo 117 Firestorm `0x005C7160`

1. R invalid → 0. m = `prog_missile(unit, skill)`; invalid → 0.
2. Unit flags |= 0x40.
3. n = `eval(calc1)`; n ≤ 0 → 0.
4. Then exactly `fan(game, unit, n, m, skill, L, first = 1)`
   (`bodies.md` §6.18, inlined): one straight `skill_missile` at the
   target, then n − 1 missiles with the jitter callback (§2.3), argument
   0…n − 2, all at the target position read once. Return 1.

#### 3.8 srvst 22 Psychic Hammer `0x005D30F0`

T none → 0. The unit's room in town (`0x0061AB00`) → 0; T's room in town
→ 0. Return 1 when T is a player or monster (type < 2), else 0. Skill
and level are not read.

#### 3.9 srvdo 33 Psychic Hammer `0x005D3140`

1. R invalid → 0. The start test (§3.8) fails → 0.
2. Zeroed record; `roll_physical(unit, record, skill, L)` then
   `roll_elemental(unit, record, skill, L)` (`0x0057E240`, `levels.md`
   §3.6).
3. `apply(game, unit, T, 1, record)` (`combat/damage.md` §5.2).
4. Result |= 1.
5. k = `eval` of the §2.7 column (`calc1` for an ordinary monster) with
   (skill, L). k > 0: one draw on the unit's seed, r = `lo' mod 100`; r <
   k → result |= 8 (knockback).
6. T's `hitpoints(6)` < 256 → result |= 2 (will die).
7. Reaction `0x0057CEE0(game, unit, T, record)` (`combat/damage.md`
   §7.1). Return 1.

The damage is applied (step 3) before the knockback and death flags are
set; only the reaction sees them.

#### 3.10 srvst 24 Dragon Talon `0x005D5970`

1. R invalid → 0. E none → 0.
2. T none → E param 1 := 0; return 0.
3. T not in melee range (`0x00622C40(unit, T, 0)`) → 0.
4. n = `eval(calc1)` (kicks, `lvl/6+1`). E param 1 := n − 1.
5. `kick_hit(game, T, R, L, knock = (n − 1 = 0))` (§2.6). Return 1.

The start makes the first kick itself.

#### 3.11 srvdo 42 Dragon Talon `0x005D5A30`

1. R invalid → 0. T none → 0.
2. p = `pair_record(unit, T)`; finisher `0x005D5220(game, unit, p)`
   (`bodies.md` §2.14). `apply_melee(game, unit, T)`.
3. E none → 0. n = E param 1 − 1; n < 0 → 0. E param 1 := n.
4. knock = 0. n = 0 (the last kick): k = the §2.7 column with (skill,
   L), or 100 for an ordinary monster (no formula). k ≥ 1: one draw on
   the unit's seed, r = `roll(100)`; r < k → knock = 1.
5. `kick_hit(game, T, R, L, knock)` (§2.6); `apply_melee(game, unit, T)`.
6. n > 0: unit flags &= ~0x40; rewind(100) (§1): the animation replays
   for the next kick. Return 1.

With n kicks the start makes kick 1 and each do one more; every do also
applies the previous kick's stored record first (step 2).

### 4. Bodies, required level 6

#### 4.1 srvdo 20 Static Field `0x005C9800`

1. R invalid → 0. D = difficultylevels record of the game's difficulty
   (`0x00611D30`); none → 0.
2. Context C: C.floor = `eval(calc2)`, C.pct = `eval(calc1)`, C.cap =
   `StaticFieldMin` (difficultylevels +0x40) in an expansion game (game
   +0x70 ≠ 0), else 0; C.len = `elem_len(unit, skill, L)` (`levels.md`
   §3.2); C.e = `EType`.
3. `scan_unit(game, unit, 0, 0, eval(aurarangecalc), aurafilter,
   0x005C96A0, C, noaura 0)` (`bodies.md` §2.12). Return 1.

Callback `0x005C96A0` (ECX scan context, EDX unit U; source = the
caster):

1. h = U's `hitpoints(6)` >> 8; h < 1 → 0.
2. C.cap ≠ 0 and h ≤ pct(U's maximum life >> 8, C.cap, 100) → 0.
3. v = pct(h, C.pct, 100); h − v < 1 → v = h − 1. v <<= 8; v < C.floor
   → v = C.floor.
4. Zeroed record; `add_element(source, record, C.e, v, C.len, &res,
   &e)` (`levels.md` §3.6; a random element draws here once and e keeps
   it). res ≥ 0 and U's stat res (unit getter) r < 0 → v = pct(v, 100,
   100 − r) (negative resistance does not raise the damage).
5. Fresh zeroed record; `add_element(source, record, e, v, C.len, 0, 0)`.
   Hit class (+0x60) |= 0xD; hit class fixed (+0x64) := 1; result :=
   0x4001 (hit, soft hit).
6. `apply(game, source, U, 1, record)`; reaction `0x0057CEE0(game,
   source, U, record)`. Return 1.

#### 4.2 srvdo 21 Telekinesis `0x005C98F0`

1. R invalid → 0. The unit none or not a player → 0. Busy
   (`0x00535060`, `items/inventory.md` §5.2) → 0. T none → 0.
2. Unit flags |= 0x40.
3. By T's type (jump table `0x005C9B34`):
   - **Player or monster:** not hostile (`0x00554200(game, unit, T)`),
     T's room in town or the unit's room in town → 0. knock = (one draw
     on the unit's seed, `roll(100)` < `Param2`). Zeroed record;
     `roll_physical`, `roll_elemental` (`levels.md` §3.6); result |=
     `ResultFlags`; hit flags |= `HitFlags`; `HitClass` ≠ 0 → hit class
     := it; knock → result |= 9 (hit, knockback). `apply(game, unit, T,
     1, record)`; reaction. Return 1.
   - **Object:** operate `0x00584420(game, unit, 2, T GUID, &out)`
     (`world/waypoints.md`). Return 1.
   - **Item:** of item type 22 (`scro`), 4 (`gold`), 38 (`tpot`), 56,
     9 (`poti`) or 41 (`key`) (`0x00629BB0`, tested in this order) →
     auto pickup `0x00563560(game, unit, T GUID, &out)`
     (`items/inventory.md` §8.1). Else sound event 0x13 on the unit
     (`0x00553380(unit, 0x13)`, refused pickup). Return 1.
   - **Missile or other:** return 1.

#### 4.3 srvst 16 Poison Dagger `0x005C30A0`

1. T none → 0. T's room in town → 0. R invalid → 0.
2. Zeroed record; result = `melee_result(game, unit, T, to_hit(unit,
   skill, L), 0)`.
3. Hit: enhanced damage % := `eval(calc1)`. `EType` ≠ 0: c =
   `eval(calc4)`; conversion % := c; c > 0 → conversion element :=
   `EType`; `roll_elemental(unit, record, skill, L)` (only with an
   `EType`).
4. `start_combat(game, unit, T, record, SrcDam or 128)`. Return 1.

#### 4.4 srvdo 32 Poison Dagger `0x005C4CD0`

Unit flags |= 0x40. T none → 0. `apply_melee(game, unit, T)`. Return 1.

#### 4.5 srvdo 55 Corpse Explosion `0x005C4DF0`

Also mon death sentry, NihlathakCorpseExplosion.

1. T none → 0. `0x00645680(T)` (corpse test, `bodies.md` §7.5) = 0 →
   0.
2. State 104 (`corpse_nodraw`) on for T; queue T for update.
3. skill = 0 → 0 (an out-of-range skill reads a null record: fatal).
4. h = T's base `maxhp(7)` (`0x006253B0`). T a monster: h = (minHP +
   maxHP) << 7 from stats by level (`monsters/init.md` §8.1, flag 1;
   class, L-flag, difficulty, T's `level`): the class average.
5. (x, y) = T's position. r = `eval(aurarangecalc)`; r1 = r / 2, r2 =
   (r + 1) / 2 (signed, truncating).
6. lo = pct(h, `eval(calc1)`, 100); hi = pct(h, `eval(calc2)`, 100); v =
   lo + `roll(hi − lo)` on **T's** seed (`0x0045C3E0`).
7. tl = T's `level(12)`, ul = the unit's `level(12)`; tl ≠ 0 and ul < tl
   → v = pct(v, ul, tl).
8. Zeroed record D. p = `eval(calc3)`; ≤ 0 → 0; ≥ 100 → 100. p > 0 and
   `EType` ≠ 0: `add_element(unit, D, EType, pct(v, p, 100),
   elem_len(unit, skill, L), 0, 0)`; v = pct(v, 100 − p, 100).
9. D physical := v.
10. `scan_unit(game, unit, x, y, r2, aurafilter, 0x005C4D10, C, noaura
    0)` with C {&D, x, y, r1²}. Return 1.

Callback `0x005C4D10` (ECX scan context, EDX unit U): C none → 0. Copy
D; squared distance from (x, y) to U > r1² → copy physical := 0.
`apply(game, source, U, 1, copy)`; reaction. Return 1.

With an odd range the outer ring (r1 < d ≤ r2) takes only the element
part. The scan skips the caster (`bodies.md` §2.12) but not T.

#### 4.6 srvst 40 Leap `0x005D9D50`

1. R invalid → 0. E none → 0.
2. **Player** (not a monster): target position (x, y) (`bodies.md`
   §2.4) fails → 0. Room test (§2.13) fails → 0. Clamp (§2.13) with L
   fails → 0.
3. **Monster:** T = target.
   - T none: target position (x, y) fails → used skill := none
     (`0x00620210(unit, 0)`), return 0; else E param 4 := −1.
   - T present: monster pre-hit (§2.13) on T; the unit is dead
     (`0x005541B0`) → 0. (x, y) = 2·T position − the unit's position
     (the point beyond T).
   - Free point `0x0064E7B0(the unit's room, &(x, y), unit size, mask
     0x3C01, fallback 0)`; none → used skill := none, return 0.
4. Reserve the landing: pattern stamp `0x0064EA90(room at (x, y), x, y,
   the unit's path pattern, 0x80)`.
5. `set_uninterruptable(unit, 1)` (§2.8). State 18 (`skill_move`) on.
6. E param 1 := x, param 2 := y. A monster with T: param 3 := T type,
   param 4 := T GUID. E flags := 0x80. Return 1.

#### 4.7 srvdo 77 Leap `0x005DA370`

1. R invalid → 0. E = the unit's entry of the skill (`0x006439F0`); none
   → 0.
2. Delete the unit's type-1 timers (`0x00540E60(game, unit, 1, 0)`).
3. By E flags:
   - Bit 0x100 (in flight): landed = Land (§2.13). Landed and the unit
     is a player: zeroed record, result := 9 (hit, knockback);
     `area_damage(game, unit, 0, 0, eval(calc1), record, 0)`
     (`missiles.md`; filter 0x8583, around the unit). Return 1.
   - Bit 0x80 (launch): return Launch (§2.13).
   - Otherwise: a player gets E flags := 0. Return 1.

The first do (flags 0x80) starts the flight; the path step finishing
(`use.md` §5.2) runs the do again with 0x1101 until Land succeeds.

#### 4.8 srvdo 70 Double Swing `0x005D8470`

1. T none → 0.
2. i = frame event index (unit +0x38 bits 8+). i odd (signed mod 2 ≠
   0): unit flags |= 0x40; T' = `next_unit(game, unit, 0, 0, melee range
   (0x00622870) + 4, 0x20003, T's GUID, null)` (`0x0056BD10`,
   `missiles.md`); none → 0; path target := T' (`0x00620C10`); T := T'.
3. Run the srvst 32 body (`bodies.md` §3.8, Bash) with (game, unit,
   skill, L) — it reads the target again.
4. `apply_melee(game, unit, T)`. Return 1.

Even events leave flag 0x40 as it was; the odd one sets it after
switching to the next target.

#### 4.9 srvdo 71 Taunt `0x005D8570`

1. R invalid, or `auratargetstate` not in 0…states count − 1 → 0.
2. T = target. T none, or the taunt test fails, or not hostile
   (`0x00554200(game, unit, T)`) → T = `nearest(game, unit, 20, taunt
   test)` (§2.9); none → 0.
3. d = `eval(auralencalc)`.
4. Curse context (`bodies.md` §4.4 step 5 layout): game, unit, ai = 1,
   upd, skill, L, d, stats / values from `aurastat1–6` /
   `aurastatcalc1–6` (stop at the first invalid stat, stat −1; stop
   without a record; `updateanimrate` → upd = 1), state =
   `auratargetstate`.
5. Run the curse unit callback `0x005C35C0(T, context)` (`bodies.md`
   §4.4 "Per unit U"; state 27 `taunt` → AI kind 12); its result is not
   read.
6. T's AI control (a monster with data: data +0x28, else none): AI
   params unchanged (`0x005B0D70(control, −666, −666, −666)`); leash
   owner := none (`0x005DD330(T, none)`: control +0x0C := −1, +0x10 :=
   1).
7. Delete T's type-2 timers; type-2 timer at F + 1 on T; T's path target
   := the unit (`0x00620C10(T, unit)`). Return 1.

Taunt test `0x005D8510(game, U)`: U a monster, `can_switch(U, 12)`
(`bodies.md` §4.4), alive (`0x005541B0` = 0), and its minion owner
(`0x0058F090`: AI control +0x2C GUID, +0x30 type) is none (GUID −1) or
not a player (type ≠ 0).

#### 4.10 srvdo 43 Shock Field `0x005D5D70`

1. R invalid → 0. m = `prog_missile(unit, skill)`; m < 0 → 0.
2. n = `prog_count(unit, skill, L)` (§2.11); n ≤ 0 → 0.
3. Return `scatter(game, unit, m, n, eval(aurarangecalc), skill, L)`
   (§2.10).

#### 4.11 srvdo 44 Blade Sentinel `0x005D6020`

1. R invalid → 0. `summon_class(unit, skill, L, &mode)` < 0 → 0.
2. Unit flags |= 0x40.
3. Target position (tx, ty) fails → 0.
4. m = `sentry(game, unit, 0, 0, R, skill, L)` (`bodies.md` §6.9; at
   the unit's position); none → 0.
5. m base `level(12)` := the unit's `level(12)`.
6. Place m at the unit's position: `0x00554EA0(game, m, the unit's room,
   ux, uy, 0, 0)` (`sim/path-placement.md` §10).
7. AI command {type 0, ux, uy, tx, ty} made current on m
   (`0x0058EF40`, `monsters/ai.md` §8).
8. Delete m's type-2 timers; type-2 timer at F + 1. m flags (+0xC4) &=
   ~0x8. Return 1.

#### 4.12 srvst 25 Dragon Claw `0x005D6330`

Return 1 if T exists, else 0.

#### 4.13 srvdo 46 Dragon Claw `0x005D6340`

1. T none → 0.
2. `claw_hit(game, T, L)` (§2.12).
3. p = `pair_record(unit, T)`; p → finisher `0x005D5220(game, unit, p)`
   (`bodies.md` §2.14). `apply_melee(game, unit, T)`.
4. Frame event index even → unit flags &= ~0x40 (the do runs again for
   the second claw); odd → unit flags |= 0x40. Return 1.

### 5. Bodies, required level 12

#### 5.1 srvst 7 Impale `0x005DAB40`

1. R invalid → 0. T none → 0. Not hostile (`0x00554200(game, unit, T)`)
   → 0.
2. Zeroed record; result = `melee_result(game, unit, T, to_hit(unit,
   skill, L), 0)`.
3. Hit:
   1. Physical := `bonuses(unit, get 1, item none (current weapon), 0, 0,
      eval(calc1), 0, SrcDam)` (`combat/damage.md` §3.2; the raw
      `SrcDam`, 0 included).
   2. `EType` ≠ 0: c = `eval(calc4)`; conversion % := c; c > 0 →
      conversion element := `EType`. `roll_elemental(unit, record,
      skill, L)`.
   3. Hit flags := 1 (skip the physical roll in `fill`).
   4. W = the weapon (`0x00535BC0`); W with durability (`0x00629930`) →
      `wear(game, eval(calc2), eval(calc3))` (§2.14; calc2 evaluated
      first).
4. `start_combat(game, unit, T, record, 128)`. Return 1.

The do is srvdo 2 (`bodies.md` §4.2).

#### 5.2 srvdo 60 Bone Wall `0x005C58B0`

1. Target position (tx, ty) fails → 0. Room at (tx, ty) (from the
   unit's room) none or in town → 0.
2. Unit flags |= 0x40.
3. skill = 0 → 0. c = `summon_class(unit, skill, L, &mode)`; < 0 → 0.
   pt = `pettype` read unsigned; ≥ pettype count → 0.
4. m = spawn (`bodies.md` §6.2) {flags 9 (position given, keep unit flag
   0x80000000 clear), owner unit, class c, AI state 0, mode, tx, ty, pt,
   pet max `eval(petmax)`}; none → 0.
5. Owner data `0x0058F030(game, m, unit GUID, unit type, 0, 1)` (no unit:
   −1, 6). Umod 15 on m (`0x005A4850(game, m, 15, 0)`,
   `monsters/init.md`). `skill_stats(game, unit, m, skill, L, 0)`
   (`bodies.md` §6.5). `0x005B1990(game, m, 0, 9)` (alignment,
   `monsters/population.md`).
6. (mx, my) = m's position. n = `eval(calc2)` / 2 (signed, truncating);
   n ≤ 1, or `srvmissilea` not 0 ≤ s < missiles count → return 1.
7. (dx, dy) = the unit's position − (mx, my); both 0 → dx = 1.
8. Record (`missiles.md` §R2.1, zeroed): flags 0x21, owner the unit,
   class `srvmissilea`, position (mx, my), skill, L. Target (mx − dy, my
   + dx): create; a missile M → M data +0x28 := m's GUID (`0x0064A710`),
   data +0x2C := n (`0x0064A760`). Target (mx + dy, my − dx): the same.
   Return 1.

The two wall-maker missiles run perpendicular to the caster's line of
sight and carry the first segment's GUID and the remaining count
(`missiles/bodies.md` conventions for data +0x28 / +0x2C).

#### 5.3 srvst 31 Charge `0x005CF6B0`

1. R invalid → 0. E none → 0. Path P (unit +0x2C) none → 0.
2. T = target. T present and in melee range (`0x00622C40(unit, T,
   0)`):
   - Player: return Swing (§2.15).
   - Monster: mode := 1 (`0x00553570(game, unit, 1)`); used skill :=
     none; mode request m at T with m = 5 for class (`BaseId`, checked by
     `0x00463900`) 73 (`clawviper1`) or 211 (`duriel`), else 4; return
     `0x005A7C20(game, &req, 1)`.
3. T present, out of range, without state 54 → E param 1 := T type,
   param 2 := T GUID. Otherwise (no T, or T has state 54) → E param 1 :=
   6, param 2 := 0.
4. E param 4 := 7 (the hit mode).
5. v = 0x100. Player: charstats `RunVelocity` (+0x41) << 8 (record
   found). Monster: monstats `Run` (+0x34) << 8 (record found); class 211
   → E param 4 := 15; P +0x14 and +0x16 := 0 (`0x00649050`); P step
   counts := 20 (`0x00648E70`).
6. vp = max(the unit's `velocitypercent(67)`, 50). P velocity :=
   pct(v, `Param1` + vp, 100) (`0x00648690`); compute the path
   (`0x00649970(P, unit, 0)`).
7. E flags := 0x1001 (moving, `use.md` §5.2). State 18 (`skill_move`)
   on. Return 1.

#### 5.4 srvdo 67 Charge `0x005CF900`

1. R invalid → 0. E none → 0.
2. f = hit frame (§2.15; the sequence getters are also called once
   before it, results unused).
3. K = the unit of (E param 1, E param 2) (`0x00552F60`) unless param 1
   = 6; else none.
4. **E flags & 1 = 0** (the hit):
   1. E flags := 0; params 1, 2 := 0; landing message `0x00571B70(unit,
      skill)` (§2.13: state 18 off, message 0xA5).
   2. Animation from frame f (`0x00553DC0(game, unit, f)`); unit flags
      |= 0x40.
   3. K none → K = `next_unit(game, unit, 0, 0, 3, 3, −1, null)`; none
      → delete type-1 timers, type-1 timer at F + 1, return 0.
   4. K in melee range (`0x00622C40(unit, K, 3 for a monster, else 0)`)
      and the unit alive: zeroed record; a player: result =
      `melee_result(game, unit, K, to_hit(unit, skill, L), 0)` | 8
      (knockback); others: result := 9. Enhanced damage % :=
      `eval(calc1)`; `EType` ≠ 0: c = `eval(calc4)`, conversion as §3.1;
      `roll_elemental(unit, record, skill, L)` (on a miss too). A
      monster: `mode_damage(unit, m)` with m = 8 for class 73, 5 for 211
      or 436, else 4. Hit class := 0x70; `start_combat(game, unit, K,
      record, SrcDam)` (raw); `apply_melee(game, unit, K)`; overlay 147
      on K (`0x00621E40(K, 147, 0)`).
   5. Return 1.
5. **E flags & 1** (moving): r = 3 for a monster whose E flags have
   0x2 (path finished, `use.md` §5.2), else 0.
   1. K present and in melee range (r): E flags := 0; animation from
      frame f + 1; landing message. Return 1.
   2. Flags & 2 = 0 (still moving): F − (unit +0x44 >> 8) ≥ f →
      animation from frame 0 and E flags := 1. Delete type-0 timers;
      type-0 timer at F + 1 with arguments (1, 0); unit flags &= ~0x40.
      Return 1.
   3. Flags & 2 (arrived without a target in range): E flags := 0;
      landing message; K' = `next_unit(game, unit, 0, 0, 3, 3, E param
      2, null)`; K' in melee range (0) → E params 1, 2 := K' type, GUID;
      return 1. Else delete type-1 timers, type-1 timer at F + 1; return
      0 (the monster `BaseId` 436-and-441 test never exits early, Edge
      case 13).

#### 5.5 srvdo 74 Double Throw `0x005D88B0`

1. R invalid → 0. W = the weapon (`0x00535BC0`); none → 0.
2. m = `missiletype` of W's item class (weapons +0xFA, `0x006288A0`);
   not 0 < m < missiles count → 0.
3. W of item type 38 (missile potion) → lob `skill_missile`
   (`0x0056EE90`), else straight (`0x0056ECB0`), with (m, unit, skill,
   L, 0, 0, 0, 0, quant 1) (`bodies.md` §2.4).
4. Missile M made: M stat 19 (`tohit`) += `to_hit(unit, skill, L)`; M
   stat 25 (`damagepercent`) += `eval(calc1)` (`0x006272B0`). Return 1
   (also when none was made).

#### 5.6 srvst 34 Find Item `0x005D8760`

T none → 0. Return 1 when `0x006455E0(T)`: T a monster in mode 12, no
`udead`-group state (`0x0063A770`), monstats2 `corpseSel`
(`0x004638A0`); else 0. (No `soft` test, unlike `bodies.md` §3.9.)

#### 5.7 srvdo 72 Find Item `0x005D8780`

1. R invalid → 0. T none → 0. The start test (§5.6) fails → 0. T has
   state 118 → 0.
2. State 118 on for T; queue T for update.
3. p = `eval(calc1)`. One draw on the **unit's** seed, r = `roll(100)`;
   r ≥ p → **return 0** (the corpse is still used up).
4. Second draw r2 = `roll(100)` (unit's seed). q = 1; `Param1` ≤ r2 <
   `Param1` + `Param2` → 2; next `Param3` band → 3; next `Param4` band →
   4.
5. `0x005A8000(game, T, unit, q)` (`items/treasure.md` §3.6; q is not
   read). Return 1.

#### 5.8 srvdo 47 Cloak of Shadows `0x005D6630`

1. R invalid, or `aurastate` not in 0…states count − 1 → 0.
2. The unit already has `aurastate` → 0.
3. d = `eval(auralencalc)`. `apply_state` (`bodies.md` §2.7) {source =
   target = the unit, skill, L, d, stat `passivestat1`, value
   `eval(passivecalc1)`, state `aurastate`, callback `0x0056E900`};
   none → 0.
4. For i = 2…5: `passivestat_i` ≥ 0 and v = `eval(passivecalc_i)` ≠ 0 →
   list set. List set 350 := skill, 351 := L. Mark `aurastate` changed.
5. `auratargetstate` not in 0…count − 1 → **return 0** (the caster keeps
   its state).
6. Context: skill, L, d, state `auratargetstate`, stats[i] =
   `aurastat_i`, values[i] = `eval(aurastatcalc_i)` for stats ≥ 0 (else
   0), i = 1…6.
7. `scan_unit(game, unit, 0, 0, eval(aurarangecalc), aurafilter,
   0x005D6520, context, noaura 1)`. Return 1.

Callback `0x005D6520` (ECX scan context, EDX unit U):

1. U dead (`0x005541B0`) → 0.
2. `apply_state` {source = the caster, target U, skill, L, d, stat =
   stats[1] when > 0 else −1, value values[1], state, callback
   `0x005C3370` (AI curse, `bodies.md` §2.8)}; none → 0.
3. Stats 2…6 ≥ 0 → list set := value (also 0).
4. U a monster and `can_switch(U, 10)` (`bodies.md` §4.4) → AI special
   state 10 (`0x005B0E00(game, U, U's AI control, 10)`). Return 1.

### 6. Bodies, required level 18

#### 6.1 srvdo 11 Charged Strike `0x005DB850`

1. Unit flags |= 0x40. R invalid → 0. T none → 0.
2. `apply_melee(game, unit, T)` (the hit stored by srvst 6, `bodies.md`
   §7.2).
3. n = `eval(calc1)`. (tx, ty) = T's position; (ux, uy) = the unit's.
4. m = `prog_missile(unit, skill)`; invalid → 0.
5. Record (zeroed): flags 0x21, owner the unit, class m, position (tx,
   ty), target (2tx − ux, 2ty − uy), skill, L, init callback §2.3. For i
   = 0…n − 1: argument := i; create. Return 1.

The bolts start at the target and head on past it, each on its own
jittered path.

#### 6.2 srvdo 24 Fire Wall `0x005C9EA0`

Also VampireFirewall, PrimeFirewall, CountessFirewall (monster rows).

1. R invalid → 0. m = `srvmissilea`; invalid → 0.
2. `point_free(game, unit, m)` (`bodies.md` §6.12) = 0 → 0.
3. (cx, cy) = T's position, or the path's target point without T; x or
   y = 0 → 0.
4. The room at (cx, cy) (from the unit's room) is in town → 0.
5. Record (zeroed): flags 0x21, owner the unit, class m, position (cx,
   cy), skill, L. (ux, uy) = the unit's position. Target (cx − (uy − cy),
   cy + (ux − cx)): create. Target (cx + (uy − cy), cy − (ux − cx)):
   create.
6. `srvmissileb` valid → flags := 1, target (0, 0), class
   `srvmissileb`: create (at the centre). Return 1.

#### 6.3 srvdo 25 Enchant `0x005CA030`

1. R invalid, `aurastat1` < −1 or ≥ itemstatcost count, or `aurastate`
   not in 0…states count − 1 → 0.
2. U = T when T exists and is an ally (`0x00554DE0(game, unit, T)`),
   else the unit itself.
3. `apply_state` {source the unit, target U, skill, L, duration
   `eval(auralencalc)`, stat −1, value 0, state `aurastate`, default
   callback}; none → 0.
4. For i = 1…6: `aurastat_i` valid and v = `eval(aurastatcalc_i)` ≠ 0 →
   list set.
5. Mark `aurastate` changed on U. Return 1.

#### 6.4 srvdo 26 Chain Lightning `0x005CA1B0`

1. R invalid → 0. `srvmissilea` invalid → 0.
2. Unit flags |= 0x40. n = `eval(calc1)`.
3. M = `skill_missile(game, srvmissilea, unit, skill, L, 0, 0, 0, 0,
   quant 0)` (straight, `bodies.md` §2.4); none → 0.
4. M data +0x28 := n (`0x0064A710`; the jump count read by the missile
   function). Return 1.

#### 6.5 srvdo 27 Teleport `0x005CA360`

1. (x, y) = target position (`bodies.md` §2.4; its result is not
   tested).
2. The unit's room none → 0. Level record of the room's level
   (`0x0061A1B0`, `0x0061DB70`) none → 0. Its `Teleport` (levels +0x04)
   = 0 → 0.
3. `Teleport` = 2: the line from the unit to (x, y) is blocked
   (`0x006229F0(unit, x, y, 0x804)` ≠ 0) → 0.
4. Return `0x00554EA0(game, unit, room none, x, y, 0, 0)`
   (`sim/path-placement.md` §10: free point near (x, y), teleport, room
   messages).

R is not read: an invalid skill teleports as well.

#### 6.6 srvdo 61 Confuse `0x005C3F20`

1. R invalid, `aurastat1` < −1 or ≥ itemstatcost count, or
   `auratargetstate` invalid → 0.
2. Unit flags |= 0x40.
3. r = `eval(aurarangecalc)`; d = `eval(auralencalc)` / `AiCurseDivisor`
   (`0x005C37A0`, as `bodies.md` §4.4 step 4; skipped when 0).
4. Curse context (`bodies.md` §4.4 layout): game, unit, ai 0, upd,
   skill, L, d; stats / values filled from **`aurastat2`–`aurastat6`**
   into slots 2…6 (stop at the first invalid stat with −1; stop without
   a record; `updateanimrate` → upd = 1); slot 1 stays stat 0, value 0;
   state = `auratargetstate`.
5. Return `scan_point(game, aurafilter, unit, r, 0x005C3DE0, context)`
   (`bodies.md` §2.12).

Callback `0x005C3DE0` (ECX unit U, EDX context):

1. Confuse test `0x005C3D50(game, unit, U)`: U a monster, alignment 0
   (`0x006259B0`), hostile to the caster, alive, `can_switch(U, 11)`
   (`bodies.md` §4.4). Fails → 0.
2. v = `scaled(U, slot-1 stat, slot-1 value)` (`bodies.md` §2.10; stat
   0, value 0 → 0) → stat −1 when v = 0.
3. `apply_state` {source the caster, target U, skill, L, d, that stat
   and v, state, callback `0x005C3DB0`}; none → 0.
4. Slots 2…6 with a valid stat: v = `scaled(U, stat, value)` ≠ 0 → list
   set.
5. upd → anim refresh of U.
6. Alignment := 1 (`0x005543B0(U, 1, 1)`, `monsters/init.md`); target
   list 9 (`0x005B1990(game, U, 0, 9)`: when U is in no list, prepend a
   node {U, 0} to game list 9, U +0xD0 := 9); target override kind 3,
   GUID 0 (`0x00573090(U, 3, 0)`: a monster with `switchai`, kind < 5 →
   monster data +0x38 := 3, +0x34 := 0; `monsters/ai.md` §5.1).
7. Type-10 timer on U at F + d (`0x005417D0(game, U, 10, F + d, 0, 0)`).
   Return 1.

Remove callback `0x005C3DB0` (ECX U, EDX state): alignment := 0
(`0x005543B0(U, 0, 1)`); state off; remove U from its target list
(`0x005B1A90(game, U)`).

#### 6.7 srvdo 63 Poison Explosion `0x005C5E60`

1. skill = 0 → 0 (an out-of-range skill reads a null record: fatal).
2. `srvmissilea` invalid → **return 1**.
3. T none → 0. `0x00645680(T)` (corpse test, `bodies.md` §7.5) = 0 → 0.
4. State 118 (`corpse_noselect`) on for T; queue T for update.
5. `burst(game, unit, T, srvmissilea, skill, L, 0, 2, 0)` (§2.16): 8
   missiles from T's position at the even offsets. Return 1.

#### 6.8 srvst 35 Vengeance `0x005CFE10`

1. R invalid → 0. E none → 0. T none → 0.
2. Zeroed record; result = `melee_result(game, unit, T, to_hit(unit,
   skill, L), 0)`.
3. Hit:
   1. Physical := `bonuses(unit, get 1, item none, 0, 0, 0, 0, SrcDam
      raw)` (`combat/damage.md` §3.2).
   2. Result |= `ResultFlags`; hit flags := `HitFlags` | 1.
   3. b = `base_roll(unit)` (§2.20, one draw).
   4. p = `eval(calc1)`; p ≠ 0 and the unit's `passive_fire_mastery(329)`
      q ≠ 0 → p += pct(p, q, 100). Fire (+0x10) += pct(b, p, 100).
   5. p = `eval(calc2)` with `passive_cold_mastery(331)` the same way;
      cold (+0x24) += pct(b, p, 100).
   6. p = `eval(calc3)` with `passive_ltng_mastery(330)`; lightning
      (+0x1C) += pct(b, p, 100).
   7. Cold length (+0x30) += `elem_len(unit, skill, L)`.
   8. k = E param 1: hit class := 0x20 (k = 0), 0x30 (1), 0x40 (2); E
      param 1 := (k + 1) mod 3.
4. `start_combat(game, unit, T, record, 128)`. Return 1.

The do is srvdo 2 (`bodies.md` §4.2). The hit class cycles fire, cold,
lightning per hit.

#### 6.9 srvdo 73 Blessed Hammer `0x005D0040`

1. R invalid → 0. m = `prog_missile(unit, skill)`; m ≤ 0 → 0.
2. Target position (tx, ty) fails → 0.
3. Unit flags |= 0x40.
4. Record (zeroed): flags 0x20 (target absolute), owner = origin = the
   unit, class m, target (tx, ty), skill, L. Create; none → 0.
5. M's path: type 14 (blessed hammer, `sim/pathing.md` §11.3); compute
   (`0x00649970(P, M, 0)`).
6. c = Concentration bonus `0x006461D0(unit, skill)` (`levels.md` §3.5)
   ≠ 0 → M base stats 52 (`magicmindam`) and 53 (`magicmaxdam`) :=
   pct(base value, 100 + c, 100). Return 1.

#### 6.10 srvdo 81 Holy Freeze `0x005D0920`

`bodies.md` §8.5 (srvdo 66) step for step, with two differences:

1. Self context: remove callback (+0x4C) := `0x005D0770` instead of 0,
   so the self list gets it instead of `0x005CEC50`.
2. Scan callback `0x005D07C0` instead of `0x005CF2A0`.

Callback `0x005D07C0` (ECX scan context, EDX unit U):

1. U a monster with no monstats record or with `ColdEffect` for the
   difficulty (+0x168 + difficulty, i8) ≥ 0 → 0 (only cold-affected
   monsters; other unit types pass).
2. B.any → a fresh context with B's state, skill, L, d, stats, values;
   run `0x005CEDC0` on U (`bodies.md` §4.5).
3. B.record → copy; `apply(game, source, U, 1, copy)`; reaction.
4. One draw on **U's** seed, `lo' mod 100` < 20 → state 107 (`shatter`)
   on, else off. Return 1.

Remove callback `0x005D0770` (ECX unit, EDX state): unit alive, or the
state not "stay on death" (`0x0063A4A0`): state off; alive → state 107
off; anim refresh.

#### 6.11 srvst 41 Leap Attack `0x005DA540`

1. E = the unit's entry of the skill (`0x006439F0`); none → 0. (R is not
   tested.)
2. T = target. Aim (§2.19) fails: T present and in melee range → Swing
   (§2.15), return 1; else 0.
3. The unit's room none → 0. Pattern stamp `0x0064EA90(the unit's room,
   x, y, pattern, 0x80)`.
4. `set_uninterruptable(unit, 1)` (§2.8).
5. E param 1 := x, param 2 := y; T → param 3 := T type, param 4 := T
   GUID; no T → param 3 := 6.
6. E flags := 0x1080. State 18 (`skill_move`) on.
7. T has state 54 → `0x00570420(game, T, 100)`, which acts only on a
   target **without** state 54 and so does nothing (Edge case 17).
   Return 1.

#### 6.12 srvdo 78 Leap Attack `0x005DA7E0`

1. E = the unit's entry of the skill; none → 0.
2. By E flags:
   - 0x100 (in flight): Land (§2.13) succeeds: K = Pick (§2.19); none →
     delete type-1 timers, type-1 timer at F + 4, return 0; K → animation
     from frame 16 (`0x00553DC0(game, unit, 16)`), return 1. Land fails
     → return 1.
   - 0x80: return Launch (§2.13).
   - 0x200 (landed): return Strike (§2.19) with (game, skill, L).
   - Otherwise: E flags := 0x1000; return 1.

#### 6.13 srvst 57 Rabies `0x005C79E0`

1. R invalid → 0. E none, or E's skill (`0x00643CE0`) ≠ skill → 0. T
   none → 0.
2. `aurastate` valid and the unit lacks it → 0.
3. E param 1 := 0.
4. result = `melee_result(game, unit, T, to_hit(unit, skill, L), 0)` (no
   damage record).
5. Hit: `eval(calc1)` and, with an `EType`, `eval(calc4)` are evaluated
   and discarded; E param 1 := 1; return 1. Miss → 0.

#### 6.14 srvdo 121 Rabies `0x005C8AD0`

1. R invalid → 0. T none → 0. E none, E's skill ≠ skill, or E param 1 =
   0 → 0.
2. E param 1 := 0.
3. `shape_start(game, unit, L)` (`bodies.md` §2.15); `apply_melee(game,
   unit, T)`.
4. len = `elem_len(unit, skill, L)`. `plague(game, unit, T, len, skill,
   L)` (§2.18).
5. Second hit `0x005C7C20` (R in ECX, skill in EAX, unit in ESI; game,
   T, len, L): len < 10 → 10. Zeroed record; `skill_result(game, unit,
   T, R, skill, L, record, 0)` (§2.17). Hit: hit flags |= `HitFlags`;
   `HitClass` ≠ 0 → hit class := it; enhanced damage % :=
   `eval(calc1)`; `roll_elemental`. `set_len(unit, record, len, skill)`
   (§2.17). `start_combat(game, unit, T, record, 128)`;
   `apply_melee(game, unit, T)`.
6. Return 1.

#### 6.15 srvst 58 Fire Claws `0x005C7E00`

1. R invalid → 0. T none → 0.
2. Zeroed record; `skill_result(game, unit, T, R, skill, L, record, 0)`.
3. Hit: hit flags |= 2 | `HitFlags`; `HitClass` ≠ 0 → hit class := it;
   enhanced damage % := `eval(calc1)`; `fill(game, unit, T, record, 0,
   SrcDam)` (`combat/damage.md` §3.1); `roll_elemental`;
   `start_combat(game, unit, T, record, 128)`.
4. Return 1 (also on a miss, with no combat entry).

The do is srvdo 2.

#### 6.16 srvst 26 Blade Fury `0x005D69D0`

1. R invalid → 0. `prog_missile(unit, skill)` < 0 → 0. E = the unit's
   entry of the skill; none → 0.
2. The unit has a list of state 12 (`inferno`, the channel): expiry := F
   + 7; timer 12 at F + 7. E param 1 ≤ F → r = srvdo 48 (§6.17) with
   (game, unit, skill, L); r ≠ 0 → mana `consume_mana(game, unit, skill,
   L)` (`0x0056BFE0`, `levels.md` §4); return r. Else rewind
   `0x00553C70(game, unit, 1)`; return 1.
3. No channel: `startmana` (+0x184) > 0 and `mana(8)` < `startmana` << 8
   → 0.
4. Alloc (game pool, flags 2, expire F + 21, owner the unit; failure →
   0); timer 12 at F + 21; attach; remove callback `0x005D69B0` (state
   off, unit flags |= 0x40); state 12 set and on; E param 1 := 0. Return
   1.

#### 6.17 srvdo 48 Blade Fury `0x005D68A0`

1. E = the unit's entry of the skill; none → 0. m = `prog_missile`; < 0
   → 0.
2. n = `prog_count(unit, skill, L)` (§2.11); n − 1 ≤ 0 → 0.
3. Unit flags &= ~0x40.
4. Target position (tx, ty) fails → 0.
5. made = 0. E param 1 < F: record (zeroed) flags 0x20, owner = origin =
   the unit, class m, target (tx, ty), skill, L; create; made = 1; E
   param 1 := F + n − 1.
6. The unit has state 12 → rewind `0x00553C70(game, unit, 1)`.
7. Return made.

`prgcalc1` is the frame gap plus one: one blade every n − 1 frames.

#### 6.18 srvst 27 Dragon Tail `0x005D7090`

1. R invalid → 0.
2. Attack-rate list `0x0056E520(unit, Param4)` (`bodies.md` §3.8 step 2).
3. T none → 0.
4. Zeroed record; result = `melee_result(game, unit, T, to_hit(unit,
   skill, L) + progressive_tohit(325), 0)`.
5. Hit: `kick_damage(game, record, T, skill, L)` (§2.5);
   `start_combat(game, unit, T, record, SrcDam or 128)`; return 1. Miss
   → 0.

#### 6.19 srvdo 50 Dragon Tail `0x005D7180`

1. R invalid → 0. T none → 0. p = `pair_record(unit, T)`; none → 0.
2. C = a copy of p's record. Finisher `0x005D5220(game, unit, p)`;
   `apply_melee(game, unit, T)`.
3. T alive (`0x005541B0(T)` = 0): zeroed record D; D fire (+0x10) :=
   pct(C physical, `eval(calc1)` + the unit's
   `passive_fire_mastery(329)`, 100); D result := 9; `area_damage(game,
   unit, T x, T y, eval(aurarangecalc), D, 0)` (`missiles.md`, filter
   0x8583). Return 1.

### 7. Bodies, required level 24

#### 7.1 srvst 8 Strafe `0x005DACD0`

1. R invalid → 0. E none → 0.
2. `has_ammo(unit)` (`bodies.md` §2.3) = 0 → attack-mode cleanup
   (`0x00580310`, `0x00580380`); return 0.
3. r = `eval(aurarangecalc)`. T = target. T none → T = `next_unit(game,
   unit, 0, 0, r, 3, −1, &count)` (§2.21); T present → count =
   `count_units(game, unit, 0, 0, r, 3)`.
4. lo = min(`eval(calc3)`, `eval(calc1)`) (calc3 evaluated first); n =
   count clamped to lo…`calc1` (count < lo → lo; count > calc1 → calc1).
5. E param 1 := n. T → param 2 := T type, param 3 := T GUID; no T →
   param 2 := 1, param 3 := −1.
6. `dec_quantity(game, unit)` (`bodies.md` §2.5). Return 1.

#### 7.2 srvdo 12 Strafe `0x005DBA40`

1. R invalid → 0. E none → 0.
2. r = `eval(aurarangecalc)`. n = E param 1; 0 → 0.
3. K = the unit of (E param 2, E param 3); none → K = `next_unit(game,
   unit, 0, 0, r, 3, E param 3, null)`; none → **return 1**.
4. Path target := K (`0x00620C10`). (tx, ty) := (0, 0), then target
   position (`bodies.md` §2.4; result not tested).
5. m = `srvmissilea`; hand class (`0x00623C60`) ≠ 1 and `srvmissileb` ≥
   0 → m = `srvmissileb`. Not 0 ≤ m < missiles count → 0.
6. a = `eval(calc2)`. Record (zeroed): flags 0x21, owner the unit,
   position the unit's, target (tx, ty), class m, skill, L, init callback
   `0x005DB6A0` with argument a (a ≠ 0 → the arrow's
   `damagepercent(25)` := its value + a). Create.
7. E param 1 := n − 1. n − 1 ≤ 0 → return 0.
8. K2 = `next_unit(game, unit, 0, 0, r, 3, K's GUID (−1 without K),
   null)`; K2 → E param 2 := K2 type, param 3 := K2 GUID; rewind(`Param6`)
   (§1; `0x004F4110`, skills +0x15C). Return 1.

The last arrow returns 0, so the do core ends the skill (`use.md` §5.4).

#### 7.3 srvdo 15 Dopplezon `0x005DC000`

1. R invalid → 0. c = `summon_class(unit, skill, L, &mode)`; < 0 → 0.
   pt = `pettype` unsigned; ≥ pettype count → pt = 0 (no refusal).
2. Unit flags |= 0x40.
3. m = spawn (`bodies.md` §6.2) {flags 0, owner unit, class c, AI 0,
   mode, x = y = 0, pt, pet max `eval(petmax)`}; none → 0.
4. `link_source(m, unit)` (`bodies.md` §6.20).
5. h = pct(the unit's maximum life, `eval(calc3)`, 100); m stats 6 and 7
   := h. m base `level(12)` := the unit's `level(12)`.
6. `base_stats(game, unit, m, 0, L)`; `skill_stats(game, unit, m, skill,
   L, 0)` (`bodies.md` §6.4, §6.5).
7. Type-7 timer on m at F + `eval(calc2)`; umod 21 on m
   (`0x005A4850(game, m, 21, 0)`, temporary summon); overlay 171 on m
   (`0x00621E40(m, 171, 0)`); `node_insert(game, m, 0, unit +0xD0)`.
   Return 1.

#### 7.4 srvst 9 Fend `0x005DAE30`

1. R invalid → 0. E none → 0.
2. r = melee range (`0x00622870`) + 4 (`0x0056E510`). T = target. T
   none → T = `next_unit(game, unit, 0, 0, r, 0x20003, −1, &count)`;
   none → E param 1 := 0, return 1. T present → count =
   `count_units(game, unit, 0, 0, r, 0x20003)`.
3. E param 1 := min(count, `eval(calc1)`); param 2 := T type; param 3 :=
   T GUID. Return 1.

The do is srvdo 13 (`bodies.md` §8.11).

#### 7.5 srvst 13 Thunder Storm `0x005C91C0`

R invalid → 0. E = the unit's entry of the skill (`0x006439F0`); none →
0. E param 1 := −1, param 2 := 1. Return 1.

#### 7.6 srvdo 29 Thunder Storm `0x005CA4D0`

1. R invalid, `srvmissilea` invalid, or `aurastate` invalid → 0. E = the
   unit's entry of the skill; none → 0.
2. The unit lacks `aurastate`, or E param 2 ≠ 0 (first run after the
   start): `apply_state` {source = target = the unit, skill, L, duration
   `eval(auralencalc)`, stat −1, value 0, state `aurastate`, default
   callback}; list → set 350 := skill, 351 := L.
3. Otherwise (a strike): K = `next_unit(game, unit, 0, 0, Param7
   (+0x160, 0x005C8BC0), 3, E param 1, null)`. None → E param 1 := −1.
   K: `0x005CA470` — both the unit's and K's rooms not in town → M =
   `missile_at(game, unit, skill, L, srvmissilea, K x, K y)` (`bodies.md`
   §6.13); M made → hit handler `0x005ADF10(game, M, K, 1)`
   (`missiles.md` §R5), remove M (`0x00555600`), `msg_a3(unit, K, skill,
   L, 0, 0, 0)` (§2.21). E param 1 := K's GUID.
4. E param 2 := 0. Return 1.

The state run is the timer the storm repeats on (`use.md` §7); each
strike moves on to the next unit in GUID order (`next_unit`).

#### 7.7 srvst 18 Attract `0x005C3260`

Return 1 (no test).

#### 7.8 srvdo 59 Attract `0x005C3B90`

1. R invalid → 0.
2. T = target. Attract test `0x005C31F0(game, unit, T)`: T present,
   alignment 0, `can_switch(T, 19)` (`bodies.md` §4.4), alive, T's room
   not in town, hostile to the unit. Fails → 0.
3. `aurastat1` < −1 or ≥ itemstatcost count, or `auratargetstate`
   invalid → 0.
4. Unit flags |= 0x40.
5. T: alignment := 1; target list 9 (§2.22); leave pack (§2.22).
6. r = `eval(aurarangecalc)`; d = `eval(auralencalc)` /
   `AiCurseDivisor` (`0x005C37A0`).
7. `scan_point(game, aurafilter, unit, r, 0x005C3B30, context {game,
   unit, skill, L, T type, T GUID, d})` (`bodies.md` §2.12). Callback
   (ECX U, EDX context): attract test (game, caster, U) fails → 0; target
   override `0x00573090(U, 1 when T is a player (type 0) else 2, T GUID)`
   (§6.6 step 6); type-10 timer on U at F + d. Return 1.
8. `apply_state` {source the unit, target T, skill, L, d, stat −1, value
   0, state `auratargetstate`, callback `0x005C3B00`} (result unused).
   Return 1.

Remove callback `0x005C3B00` (ECX T, EDX state): alignment := 0; state
off; target list remove (§2.22).

#### 7.9 srvst 19 Bone Prison `0x005C3270`

T none → 0. Return 1 when T's room is not in town, else 0.

#### 7.10 srvdo 62 Bone Prison `0x005C5D00`

1. Target position (tx, ty) fails → 0.
2. Unit flags |= 0x40.
3. For k = 0…11 (offset tables `0x006E304C` X = −1, 1, 3, 4, 4, 3, −1,
   1, −3, −4, −4, −3; `0x006E307C` Y = −4, −4, −3, −1, 1, 3, 4, 4, 3, −1,
   1, −3): m = segment `0x005C5BC0(game, tx + X[k], ty + Y[k], L)`
   (unit in EAX, skill in EBX). m made:
   - no leader yet: leader := m; owner data `0x0058F030(game, m, unit
     GUID (0x00451F50), unit type, 0, 1)`;
   - else: owner data `0x0058F030(game, m, leader GUID, leader type, 0,
     0)`; add m to the leader's minion list (`0x0058F100(game, leader,
     m)`: m's GUID prepended to AI control +0x34);
   - `link_source(m, unit)`; m faces (tx, ty) (direction `0x0064FDC0(m
     x, m y, tx, ty)` set on its path, `0x006488A0`).
4. Return 1.

Segment `0x005C5BC0`: skill = 0 → none. c = `summon_class`; < 0 →
none. pt = `pettype` unsigned, ≥ count → 0. A room at (x, y) in town →
none. m = spawn {flags 9, owner unit, class c, AI 0, mode, x, y, pt, pet
max `eval(petmax)`}; none → none. `skill_stats(game, unit, m, skill, L,
0)`; umod 15 on m; target list 9 (§2.22). Return m.

#### 7.11 srvst 20 Iron Golem `0x005C32A0`

Return 1 when T is an item (type 4) in mode 3 (on the ground), its item
class has `bitfield1` bit 1 (items +0xDC, `0x00629CC0`), item flag 0x10
is set (`0x006280A0`, `items/generation.md` §1.4), and the item is not
active on a unit (`0x00625820(T, 0)` = 0). Else 0.

#### 7.12 srvdo 57 Iron Golem `0x005C5250`

1. The start test (§7.11) fails → 0.
2. Unit flags |= 0x40.
3. skill = 0 → 0. c = `summon_class`; < 0 → 0. pt = `pettype` unsigned,
   ≥ count → 0.
4. Spawn request {flags 1, owner unit, class c, AI 0, mode, position =
   T's position (the unit's when T is gone), pt, pet max
   `eval(petmax)`}; m none → 0.
5. T present: T leaves its room (`0x0061A270(room, T type, T GUID)`,
   `0x00623830`, `0x0064C370`; `items/inventory.md` §8); L = T's first
   allowed body location (`0x0062EA80`); T mode := 4 (`0x00624690`);
   equip T on m at L (`0x005606B0(game, m, T GUID, L, skip 1, out none)`,
   `items/inventory.md` §4.6).
6. `golem_stats(game, unit, m, skill, L)` (`bodies.md` §6.11); message
   0x7F about m to the unit's client (`0x005531C0`, `0x0053CDF0`, as
   `bodies.md` §8.9 step 7); `node_insert(game, m, 0, unit +0xD0)`.
   Return 1.

#### 7.13 srvdo 79 Conversion `0x005D0350`

1. R invalid → 0. Unit flags |= 0x40. T none → 0.
2. ok = 0. `pair_record(unit, T)` exists (the start, srvst 32, stored a
   hit) → ok = chance `0x005D02B0(unit, L, 1)` (skill in EAX, T in EBX):
   R valid, T not a hireling, a monster, alignment 0, `can_switch(T, 11)`;
   p = `eval(calc1)`; one draw on the unit's seed, `roll(100)` ≥ p → 0;
   else 1.
3. `auratargetstate` invalid or ok = 0 → `apply_melee(game, unit, T)`;
   return 1.
4. e = max(F + `eval(auralencalc)`, F + 1).
5. Conversion (§2.23) of T by the unit, state `auratargetstate`, expiry
   e, callback `0x005D01A0`. Return 1 (the stored hit is not applied).

#### 7.14 srvst 36 Holy Shield `0x005D0180`

Return 1 when the unit has an inventory and a shield equipped
(`0x0063C8F0(inventory, null)`, `combat/hit.md`), else 0.

#### 7.15 srvdo 9 Frenzy `0x005D8E00`

1. T none → 0.
2. Frame event index even: `apply_melee(game, unit, T)`; free the unit's
   combat records for T (`0x0057C9F0`, `combat/damage.md` §5.1);
   attack-mode cleanup (`0x00580310`, `0x00580380`); Charge (§2.24);
   return Swing (§2.24) on T.
3. Odd: unit flags |= 0x40; `apply_melee(game, unit, T)`; cleanup;
   Charge; T' = `next_unit(game, unit, 0, 0, melee range + 4, 0x20003,
   T's GUID, null)`; return Swing on T' (0 when none).

#### 7.16 srvdo 75 Grim Ward `0x005D89B0`

1. R invalid → 0. T none → 0. `0x00645590(T)` (corpse test, `bodies.md`
   §3.9) = 0 → 0.
2. (x, y) = T's position. T's room none, or no room at (x, y) → 0. Free
   point `0x0064E7B0(room, &(x, y), size 2, mask 0x1000, fallback 1)`;
   none → 0. T not a monster → 0.
3. m = `srvmissilea`; T's monstats2 record (`0x00451FE0`) none → 0.
   `large` (+0x04 bit 11) → m = `srvmissilec`; else `small` (bit 10) → m
   = `srvmissileb`. Invalid → 0.
4. Record (zeroed): flags 1, owner the unit, class m, position (x, y),
   skill, L; create (result not read).
5. T: state 104 (`corpse_nodraw`) on, state 118 (`corpse_noselect`) on;
   queue for update. Return 1.

#### 7.17 srvdo 122 Hunger `0x005C7F10`

1. R invalid → 0. T none → 0.
2. Unit flags |= 0x40.
3. Zeroed record; `skill_result(game, unit, T, R, skill, L, record, 0)`
   (§2.17).
4. Hit: hit flags |= 2 | `HitFlags`; result |= `ResultFlags`; `HitClass`
   ≠ 0 → hit class := it; `fill(game, unit, T, record, 0, SrcDam)`;
   `roll_elemental`; physical += pct(physical, `eval(calc1)`, 100); life
   leech (+0x38) += `eval(calc2)`; mana leech (+0x3C) += `eval(calc3)`;
   `start_combat(game, unit, T, record, 128)`; `apply_melee(game, unit,
   T)`.
5. Return 1.

#### 7.18 srvdo 123 Volcano `0x005C8080`

1. R invalid → 0. m = `prog_missile(unit, skill)`; m ≤ 0 → 0.
2. Target position (tx, ty) fails → 0. `point_free(game, unit, m)` = 0 →
   0.
3. Unit flags |= 0x40.
4. s = `roll(256)` on the unit's seed (one step, `lo' & 0xFF`).
5. Record (zeroed): flags 1, owner the unit, class m, position (tx, ty),
   skill, L. Create; none → 0.
6. M data +0x28 := s (the seed word read by server-do 28,
   `missiles/bodies.md` §2). `msg_a3(unit, none, skill, L, tx, ty, s)`.
   Return 1.

#### 7.19 srvdo 51 Mind Blast `0x005D76E0`

1. R invalid → 0. Target position (cx, cy) fails → 0.
2. r = `prog_count(unit, skill, L)` (§2.11); 0 → r =
   `eval(aurarangecalc)`.
3. Zeroed record D; `roll_physical`, `roll_elemental` (`levels.md` §3.6);
   result |= `ResultFlags`; hit flags |= `HitFlags`.
4. Context {&D, chance = `DM(L, Param5, Param6)` (`0x00645C10`,
   `levels.md` §2; 0 when L ≤ 0), `Param3`, `Param4`}.
5. `scan_unit(game, unit, cx, cy, r, aurafilter, 0x005D7680, context,
   noaura 0)`. Return 1.

Callback `0x005D7680` (ECX scan context, EDX unit U): convert
`0x005D7430` = 0 → copy D; `apply(game, source, U, 1, copy)`; reaction.
Return 1.

Convert `0x005D7430` (U in ECX, context in EAX; game, source): test
`0x005D72B0`: U a monster, not a hireling, alignment 0, `can_switch(U,
11)`, and one draw on the **source's** seed, `roll(100)` ≤ chance; fails
→ 0. e = `roll(Param4)` (source's seed) + F + `Param3`. Conversion
(§2.23) of U by the source, state 53 (`conversion`), expiry e, callback
`0x005D7310`. Return 1 (no damage to a converted unit).

#### 7.20 srvdo 52 Dragon Flight `0x005D7850`

1. R invalid → 0. T none → 0.
2. Frame event index even (the flight): the unit's room none → 0. Unit
   flags &= ~0x40. Target position (x, y) fails → 0. Level record of the
   room's level none, or its `Teleport` = 0 → 0; `Teleport` = 2 and the
   line from the unit to (x, y) blocked (`0x006229F0`, mask 0x804) → 0.
   `0x00554EA0(game, unit, room none, x, y, 0, 0)` (result not read).
   Return 1.
3. Odd (the kick): zeroed record; result = `melee_result(game, unit, T,
   to_hit(unit, skill, L) + progressive_tohit(325), 0)`. Hit: enhanced
   damage % := `ln12` (`0x004E6CA0`: `Param1` + (L − 1)·`Param2`, 0 for L
   ≤ 0); `kick_damage(game, record, T, skill, L)` (§2.5).
   `start_combat(game, unit, T, record, SrcDam or 128)`; p =
   `pair_record(unit, T)`; finisher `0x005D5220(game, unit, p)`;
   `apply_melee(game, unit, T)`. Return 1.

The start is srvst 12 (`bodies.md` §7.4).

### 8. Bodies, required level 30

#### 8.1 srvdo 16 Valkyrie `0x005DC1E0`

1. R invalid → 0. The unit none or not a player → 0. c =
   `summon_class(unit, skill, L, &mode)`; < 0 → 0. pt = `pettype`
   unsigned; ≥ count → pt = 0.
2. Unit flags |= 0x40.
3. m = spawn (`bodies.md` §6.2) {flags 0, owner unit, class c, AI 0,
   mode, x = y = 0, pt, pet max `eval(petmax)`}; none → 0.
4. `base_stats(game, unit, m, 0, L)`; `skill_stats(game, unit, m, skill,
   L, ilvl = eval(calc2))` (`bodies.md` §6.4, §6.5; the item level of its
   equipment).
5. Delete m's type-2 timers; type-2 timer at F + 20. m state 93
   (`valkyrie`) on. `link_source(m, unit)`; `node_insert(game, m, 0, unit
   +0xD0)`. Return 1.

#### 8.2 srvst 10 Lightning Strike `0x005DB020`

1. R invalid → 0. E none → 0. T none → 0.
2. Zeroed record; result = `melee_result(game, unit, T, 0, 0)` (no skill
   to-hit bonus).
3. Hit: a = `elem_min(unit, skill, L, 1)`, b = `elem_max(unit, skill, L,
   1)` (`levels.md` §3.1, in this order); lightning (+0x1C) := a +
   `roll(b − a)` (unit seed; no draw when b − a < 1). Enhanced damage %
   := `eval(calc1)`. `EType` conversion as §3.1 (no `roll_elemental`).
4. `start_combat(game, unit, T, record, SrcDam or 128)`. Return 1.

#### 8.3 srvdo 14 Lightning Strike `0x005DBE50`

1. R invalid → 0. T none → 0.
2. `apply_melee(game, unit, T)`.
3. n = `eval(calc1)`. K = `next_unit(game, unit, T x, T y, n, 3, T's
   GUID, null)` (around T, range n); none → 0.
4. m = `prog_missile(unit, skill)`; invalid → 0.
5. j = `eval(calc2)`. Record (zeroed): flags 0x20 (target absolute),
   owner the unit, origin T (start at T), class m, target K's position,
   skill, L. Created → data +0x28 := j (`0x0064A710`, the chain count of
   server-hit 12, `missiles.md` §R9.6). Return 1 (also when none was
   made).

#### 8.4 srvst 14 Hydra `0x005C9220`

The unit's room none → 0. Target position fails → 0. Room at the target
(from the unit's room) none → 0. Return 1 when that room is not in town.

#### 8.5 srvdo 144 Hydra `0x005CA910`

1. R invalid → 0. c = `summon_class`; < 0 → 0. pt = `pettype` signed; <
   0 or ≥ count → 0. The unit's room none, target position (tx, ty)
   fails, no room there, or that room in town → 0.
2. O = the unit; a hireling monster (`0x0063EE90`) with a minion owner →
   O := that owner.
3. t = F + `ln12(skill, L)` (`0x004E6CA0`).
4. For i = 0, 1, 2 (offsets `0x006E312C` X = −1, 0, 1; `0x006E3120` Y =
   −1, 0, −1): m = spawn {flags 1, owner O, class c + i, AI 0, mode, tx +
   X[i], ty + Y[i], pt, pet max `eval(O, petmax)`}. m made:
   `set_skill(m, skill, 1)` (`0x0056DEB0`, `bodies.md` §6.5 step 6);
   `skill_stats(game, O, m, skill, L, 0)`; made = 1; m's AI control
   (`0x00541860`) → AI param 0 := t (`0x005B0D70(control, t, −666,
   −666)`); O a monster → m's alignment := O's alignment (`0x005543B0(m,
   align, 1)`).
5. Return made (1 when any hydra was made).

The three classes are consecutive `monstats` rows (hydra1–3); AI param 0
is the hydra's expiry frame.

#### 8.6 srvst 21 Revive `0x005C3350`

T none → 0. Return the revive test `0x005C3300` (T in ESI): T a monster,
monstats2 `revive` (+0x04 bit 8, `0x004638A0(class, 8)`), dead
(`0x005541B0`), the raise corpse test `0x00645510(T, 0)` (`bodies.md`
§3.6) and `can_switch(T, 7)` (`bodies.md` §4.4). Else 0.

#### 8.7 srvdo 58 Revive `0x005C56C0`

1. skill = 0 → 0 (an out-of-range skill reads a null record: fatal). pt
   = `pettype` unsigned; ≥ count → 0.
2. T none, or the revive test fails → 0.
3. `raise_penalty(game, unit)` (`bodies.md` §6.14).
4. Stand T up `0x005C5430(game)` (T in ESI):
   1. Pattern clear `0x0064EC10(T's room, T x, T y, T's pattern,
      0x8000)`.
   2. Free point `0x0064E7B0(T's room, &(x, y) = T's position, T size,
      0x3C01, fallback 1)`; none → stop here (step 5 still runs, Edge
      case 26).
   3. T flags (+0xC4) |= 0x402000E. `0x00573780(game, T)` (`monsters/ai.md`
      §2). Mode request 1 for T (`0x005A7E60`, `0x005A7C20(game, &req,
      1)`). Leave pack (§2.22). Delete T's type-8 timers. T has a right
      skill (`0x006201D0`) → `0x00643C50(T, 0, −1)` (Open question 11).
   4. Place T: `0x00554EA0(game, T, the free point's room, x, y, 0, 0)`.
5. Life: stats by level (`monsters/init.md` §8.1, flag 1; class, L-flag,
   difficulty, T's `level`) → (minHP, maxHP); h = (minHP + `roll(maxHP −
   minHP + 1)`) << 8 on **T's** seed; stats 7 and 6 := h.
6. cl = the unit's `level(12)`, tl = T's; tl ≠ 0 and cl < tl: h =
   pct(h, cl, tl), at least 1; stats 7, 6 := h; T base level := cl.
7. `skill_stats(game, unit, T, skill, L, 0)`.
8. Revive setup `0x005C55C0(unit, skill, L)` (T in ESI, game in EDI):
   owner data `0x0058F030(game, T, unit GUID, unit type, 0, 0)`; leash
   owner := the unit (`0x005DD330(T, unit)`); delete T's type-2 timers;
   type-2 timer at F + 15; alignment := 2; target override cleared
   (`0x00573120`: monster data +0x38, +0x34 := 0); T flags |= 0x80000000;
   state 96 (`revive`) on; d = `eval(calc2)` > 0 → umod 21 on T and a
   type-7 timer at F + d.
9. Pet add `0x00575D90(game, unit, T, pt, eval(petmax))` (`sim/pets.md`
   §2).
10. `node_insert(game, T, 0, unit +0xD0)`; `0x00649CA0(T)` (path reset,
    `client/msg-units.md` §3). Return 1.

#### 8.8 srvdo 80 Fist of the Heavens `0x005D0670`

1. R invalid → 0. m = `prog_missile(unit, skill)`; m ≤ 0 → 0. T none →
   0.
2. Record (zeroed): flags 1, owner the unit, position T's, class m,
   skill, L. Create; none → 0.
3. M data +0x28 := T type, +0x2C := T GUID.
4. `srvoverlay` (+0x4E) in 0…overlay count − 1 → overlay on T. Return 1.

#### 8.9 srvdo 82 Redemption `0x005D0E90`

1. R invalid, or `aurastate` invalid → 0.
2. cost, mana, d as `bodies.md` §8.5 step 2.
3. Self context: state `aurastate`, skill, L, d; for i = 1…5: stat i =
   `passivestat_i`; a stat ≥ 0 gets value `eval(passivecalc_i)` when the
   unit is not a player or mana ≥ cost. Run `0x005CEDC0` on the unit
   (`0x0056B740`; remove callback `0x005CEC50`).
4. The unit's room in town → return 1.
5. Context C {skill, L, count 0}. `scan_unit(game, unit, 0, 0,
   eval(aurarangecalc), aurafilter, 0x005D0D70, C, noaura 1)`.
6. cost > 0 and a player: C.count > 0 → state 85 on and
   `0x0056C110(unit, cost)`; else state 85 off. Return 1.

Callback `0x005D0D70` (ECX scan context, EDX unit U; source the
caster):

1. R invalid or U none → 0. `0x006456D0(U)`: U a monster in mode 12, no
   `udead`-group state, monstats2 `corpseSel` (as §5.6); else 0.
2. p = `eval(source, calc1)`; one draw on the **source's** seed,
   `roll(100)` ≥ p → 0.
3. Source life := min(life + `eval(calc2)` << 8, maximum life); mana :=
   min(mana + `eval(calc3)` << 8, maximum mana (`0x00625D60`)).
4. U state 99 (`redeemed`) on; U flags (+0xC4) &= ~6; C.count += 1.
   Return 1.

Unlike srvdo 66 (`bodies.md` Edge case 9) the count is kept: a player
pays the mana cost for every run that redeems something.

#### 8.10 srvst 38 Whirlwind `0x005D8F50`

1. R invalid → 0. E = the unit's entry of the skill; none, or the target
   position (x, y) fails → landing message `0x00571B70(unit, skill)`,
   return 0.
2. The unit has state 12 → 0.
3. T present and in melee range → landing message; return Swing (§2.15).
4. Path P none → landing message; return 0. m0 = 0x1C09 for a player,
   else 0x3C01.
5. P move-test mask := 0xC01; path type := 7; compute (`0x00649970(P,
   unit, 0)`) = 0 → landing message, mask := m0, return 0.
6. P velocity := walk velocity `0x0056E5B0(unit)` (player: charstats
   `WalkVelocity` (+0x40) << 8; monster: monstats `Velocity` (+0x32) <<
   8; else 0x600). Move-test mask := 0x401.
7. n = P's point count (`0x006487D0`); n < 1 → landing message, mask :=
   m0, return 0. (x, y) := the last path point.
8. Pattern stamp `0x0064EA90(the unit's room, x, y, pattern, 0x80 for a
   player, else 0x100)`. `set_uninterruptable(unit, 1)`. State 18 on. E
   flags := 1; params 1, 2 := x, y; param 3 := −1; param 4 := 0.
9. `aurastate` valid: `clear_group(unit, aurastate, 0)` (`bodies.md`
   §2.9); `apply_state` {source = target = the unit, skill, L, duration
   `eval(auralencalc)`, stat −1, value 0, state, callback `0x005D8EF0`};
   none → **0**. `aura_fill` and `passive_fill(unit, list, R, skill, L)`
   (`bodies.md` §2.6); list set 350 := skill, 351 := L. `auraevent1` ≥
   0: unregister (1, `aurastate`) on the unit; for i = 1…3 while
   `auraevent_i` ≥ 0: register (`auraevent_i`, skill, L,
   `auraeventfunc_i`, 1, `aurastate`) (`bodies.md` §2.13). Mark
   `aurastate` changed.
10. Return 1.

Remove callback `0x005D8EF0` (ECX unit, EDX state): unregister (1,
state); alive, or not "stay on death" → state off, anim refresh, passive
refresh `0x0056DE40(unit)`.

#### 8.11 srvdo 76 Whirlwind `0x005D9580`

1. R invalid → 0. E = the unit's entry of the skill; none → 0.
2. (x, y) = E params 1, 2. Either 0 → End (§2.25); return 0.
3. E flags & 3 = 3 (moving and arrived) → End; return 1.
4. E flags & 1 and the unit alive:
   1. A non-player: frame event index := 0, frame count (+0x48) :=
      0x400. A player: animation from frame 3 (`0x00553DC0(game, unit,
      3)`).
   2. n = Pacing (§2.25).
   3. n times: K = `next_unit(game, unit, 0, 0, 5, 3, E param 3, null)`.
      None → E param 3 := −1 and stop after this round. K: E param 3 :=
      K's GUID; zeroed record; result = `melee_result(game, unit, K,
      to_hit(unit, skill, L), 0)`; hit: result |= `ResultFlags`; hit
      flags |= `HitFlags`; `HitClass` ≠ 0 → hit class := it; enhanced
      damage % := `eval(calc1)`; `roll_elemental`. `start_combat(game,
      unit, K, record, SrcDam or 128)`; `apply_melee(game, unit, K)`.
      Each round (with or without K) toggles E flags bit 0x2000 (the
      hand).
   4. Return 1.
5. Otherwise return 0.

#### 8.12 srvst 39 Berserk `0x005D97F0`

1. R invalid → 0. T none → 0.
2. Zeroed record; result = `melee_result(game, unit, T, to_hit(unit,
   skill, L), 0)`. Hit: result |= `ResultFlags`; hit flags |=
   `HitFlags`; `HitClass` ≠ 0 → hit class := it; enhanced damage % :=
   `eval(calc1)`; `EType` conversion as §3.1; `roll_elemental`.
3. `start_combat(game, unit, T, record, SrcDam or 128)`.
4. `aurastate` valid: e = F + `eval(calc2)`; e ≤ F → F + 10. The unit's
   list of it, or a new one (game pool, flags 2, expire e, owner the
   unit, state set, callback `0x0056E900`, attached, state on; failure →
   return 1); expiry := e; timer 12 at e; `aura_fill(unit, list, R,
   skill, L)`.
5. Return 1.

The do is srvdo 2.

#### 8.13 srvst 28 Blade Shield `0x005D7A00`

1. R invalid → 0. E = the unit's entry of the skill; none → 0.
   `prog_missile(unit, skill)` ≤ 0 → 0.
2. `eval(auralencalc)` ≤ 0, or `aurastate` invalid → 0.
3. `clear_group(unit, aurastate, 0)`; `apply_state` {source = target =
   the unit, skill, L, `eval(auralencalc)`, stat −1, value 0, state
   `aurastate`, default callback}; none → 0.
4. `aura_fill(unit, list, R, skill, L)`; list set 350 := skill, 351 :=
   L; mark `aurastate` changed. Return 1.

#### 8.14 srvdo 54 Blade Shield `0x005D7E10`

R invalid, or `aurastate` invalid → 0. E = the unit's entry of the
skill; none → 0. The unit's room not in town → Blade Shield pulse
(§2.26) with (game, unit, skill, L). Return 1.

## Constants & data dependencies

| Item | Value | Where |
|---|---|---|
| player-count damage multiplier | table `0x006E21F4` (0, 0, 8, 16, 24, 32, 40, 48, 56), 8n − 16 for n ≥ 9, ×v/128 | §2.1 |
| classic difficulty damage | ×10/12 (min, max), ×10/15 (to-hit) | §2.1 |
| monstats columns | `SkillDamage` +0xA8, `Align` +0x4C, `El1Mode`–`El3Mode` +0xFE, `El1Type`– +0x101, `El1Pct` +0x104 (3 per slot), `El1MinD` +0x10E, `El1MaxD` +0x120, `El1Dur` +0x132 | `fields.tsv` |
| armor columns | `mindam` +0xFE, `maxdam` +0xFF (u8) | §3.4 |
| skills columns | `Param1`–`Param4` +0x148–+0x154, `calc1`–`calc4` +0x138–+0x144, `ResultFlags` +0x12E, `HitFlags` +0x130 | `fields.tsv` |
| potion codes | table `0x00741B58`, bound `0x00741B54` = 15 | §2.4 |
| state ids | 101 holyshield, 118 corpse_noselect | |
| stat ids | 19, 21, 22, 48–66, 74 hpregen, 100 monster_playercount, 315–317, 325 progressive_tohit | |
| jitter cap | 77 frames / steps | §2.3 |
| burst offsets | 16 (X, Y) pairs, `0x006E2510` / `0x006E24D0` | §2.16 |
| state ids (batch 3) | 12 inferno (channel), 18 skill_move, 21 stunned, 27 taunt, 54 uninterruptable, 92 death_delay, 104 corpse_nodraw, 107 shatter | |
| stat ids (batch 3) | 52 / 53 magic min / max, 67 velocitypercent, 72 durability, 329–331 passive fire / lightning / cold mastery | |
| levels column | `Teleport` +0x04 (0 no, 1 yes, 2 line of sight) | §6.5 |
| monster classes tested | 73 clawviper1, 211 duriel, 436 (Charge); `BaseId` 78 sandleaper1, 540 ancientbarb1 (Leap) | §2.13, §5.3, §5.4 |

Batch 3 order: the slots of `bodies.md` Constants "count 1" (one class
skill of the 1.14d `patch_d2` `skills.txt` uses them), ordered by the
skill's `reqlevel`, then class (ama, sor, nec, pal, bar, dru, ass), then
skill `Id`, start before do.

| Level | Slots (skill) |
|---|---|
| 1 | do 7 (Jab), do 17 (Charged Bolt), do 64 (Sacrifice), do 150 (Smite), do 69 (Find Potion), do 114 (Raven), do 117 (Firestorm), st 22 + do 33 (Psychic Hammer), st 24 + do 42 (Dragon Talon) |
| 6 | do 20 (Static Field), do 21 (Telekinesis), st 16 + do 32 (Poison Dagger), do 55 (Corpse Explosion), st 40 + do 77 (Leap), do 70 (Double Swing), do 71 (Taunt), do 43 (Shock Field), do 44 (Blade Sentinel), st 25 + do 46 (Dragon Claw) |
| 12 | st 7 (Impale), do 60 (Bone Wall), st 31 + do 67 (Charge), do 74 (Double Throw), st 34 + do 72 (Find Item), do 47 (Cloak of Shadows) |
| 18 | do 11 (Charged Strike), do 24 (Fire Wall), do 25 (Enchant), do 26 (Chain Lightning), do 27 (Teleport), do 61 (Confuse), do 63 (Poison Explosion), st 35 (Vengeance), do 73 (Blessed Hammer), do 81 (Holy Freeze), st 41 + do 78 (Leap Attack), st 57 + do 121 (Rabies), st 58 (Fire Claws), st 26 + do 48 (Blade Fury), st 27 + do 50 (Dragon Tail) |
| 24 | st 8 + do 12 (Strafe), do 15 (Dopplezon), st 9 (Fend), st 13 + do 29 (Thunder Storm), st 18 + do 59 (Attract), st 19 + do 62 (Bone Prison), st 20 + do 57 (Iron Golem), do 79 (Conversion), st 36 (Holy Shield), do 9 (Frenzy), do 75 (Grim Ward), do 122 (Hunger), do 123 (Volcano), do 51 (Mind Blast), do 52 (Dragon Flight) |
| 30 | do 16 (Valkyrie), st 10 + do 14 (Lightning Strike), st 14 + do 144 (Hydra), st 21 + do 58 (Revive), do 80 (Fist of the Heavens), do 82 (Redemption), st 38 + do 76 (Whirlwind), st 39 (Berserk), st 28 + do 54 (Blade Shield) |

85 slots: 11 + 13 + 8 + 19 + 22 + 12.

## Randomness

No body draws except where a step says so; callees draw in the order the
steps call them (`bodies.md` Randomness). Draws named here:

| Where | Seed | Draw |
|---|---|---|
| §2.1 step 9 | unit | `roll(100)` per element slot with 0 < p < 100; `roll(5)` for a `rand` type |
| §2.3 | missile | re-initialised from the path target x + argument (no draw) |
| §2.4 `potion_code` | unit | one step, `lo' mod 100` |
| §2.5 step 4 | unit | `roll(hi − lo)` when ≥ 1 |
| §3.4 player | unit | `bonuses` roll, `roll_elemental`, then `melee_result` |
| §3.5 | unit | `roll(100)`, then `potion_code` on success |
| §3.9 step 5 | unit | `lo' mod 100` when k > 0 |
| §3.11 step 4 | unit | `roll(100)` on the last kick when k ≥ 1 |
| §2.10 `scatter` | unit | re-initialised from the target x, then `roll(2r)` twice per missile |
| §4.1 callback | source | one step only for a random `EType` (first `add_element`) |
| §4.2 | unit | `roll(100)` for the knockback, then `roll_physical`, `roll_elemental` |
| §4.5 | **T** | `roll(hi − lo)` |
| §2.14 `wear` | unit | `lo' mod 100` (one step) when reached |
| §5.1 | unit | `melee_result`, then `bonuses`, `roll_elemental`, `wear` |
| §5.4 hit | unit | player `melee_result`, then `roll_elemental` |
| §5.7 | unit | `roll(100)`, then a second `roll(100)` on success, then the treasure walk |
| §2.20 `base_roll` | unit | `roll(mx − mn)` |
| §6.8 | unit | `melee_result`, `bonuses`, then `base_roll` |
| §6.10 callback | **U** | `lo' mod 100` per scanned unit (shatter), after the aura's own draws |
| §6.14 | unit | `shape_start` (`melee_result`, `start_combat`), then the second hit's `melee_result` unless `ResultFlags` bit 1, then `roll_elemental` |
| §6.15 | unit | `skill_result`, `fill`, `roll_elemental` |
| §7.13 | unit | `roll(100)` when a stored hit exists |
| §7.18 | unit | `roll(256)` before the missile is made |
| §7.19 | unit, then **source** per scanned unit | `roll_physical`, `roll_elemental`; per unit `roll(100)`, and on success `roll(Param4)` |
| §7.15, §7.20 | unit | Swing / kick draws (`melee_result`, `roll_elemental`, `kick_damage`) |
| §2.25 Pacing | unit (monsters only) | one step, `lo'` & 1 |
| §8.2 | unit | `melee_result`, then `roll(b − a)` |
| §8.7 | **T** | `roll(maxHP − minHP + 1)` |
| §8.9 callback | **source** | `roll(100)` per corpse |
| §8.11 | unit | per round `melee_result`, `roll_elemental`, `start_combat` |
| §2.26 | unit | `roll_physical`, `roll_elemental` once; per unit `melee_result` |

## Edge cases & original bugs

1. `mode_damage` element slots 2 and 3 read the El1 columns (flags 0x41,
   0x42 keep bit 0x40), and a `noRatio` class gets 0 elemental min / max
   from its element slots (§2.1 step 9.1).
2. `kick_damage` leaves the other hand's claw stat lists toggled off
   after the fill (§2.5 step 8), like `levels.md` Edge case 6.
3. Psychic Hammer and Dragon Talon compute a level from T's level twice;
   the result is always L (§2.7).
4. Sacrifice with no stored combat entry (missed start) returns 1 and
   applies nothing, not even `apply_melee` (§3.3 step 2).
5. Find Potion marks the corpse used (state 118) before the chance draw:
   a failed roll still consumes it (§3.5).
6. Charged Bolt returns 1 even when n ≤ 0 or the target position fails;
   Firestorm returns 0 for n ≤ 0 (§3.2, §3.7).
7. Shock Field re-seeds the caster's own seed from the target x
   (§2.10 step 2): every later draw on the caster depends on where it
   aimed.
8. Corpse Explosion draws on the corpse's seed and, with an odd range,
   gives the outer ring (r1 < d ≤ r2) element damage only (§4.5).
9. Double Swing skips the R test and relies on the Bash start body
   (§4.8).
10. Taunt runs the curse callback with AI kind 12 (state 27): the
    `bodies.md` Constants note "27 → 12 coded, unused" holds only for
    srvdo 30; srvdo 71 uses it.
11. A monster's Leap first attacks its target (A1 damage through
    `mode_damage(unit, 4)`) and then lands behind it (§2.13 pre-hit).
12. Charge's hit frame loop (§2.15) re-reads the event byte of sequence
    record 0 instead of record i: the result is 7 or −1, never another
    frame.
13. Charge do: the early exit for a monster of `BaseId` 436 that is also
    441 can never be taken (§5.4 step 5.3).
14. Find Item returns 0 when its chance roll fails, Find Potion 1; both
    mark the corpse used first (§3.5, §5.7).
15. Cloak of Shadows with an invalid `auratargetstate` returns 0 after
    applying the caster's state (§5.8 step 5); its target callback treats
    `aurastat1` = 0 (strength) as "no stat".
16. Impale passes the raw `SrcDam` to `bonuses` (0 gives no physical
    damage) but 128 to `start_combat` (§5.1).
17. Leap Attack calls the stun helper `0x00570420` only for a target
    with state 54, and the helper acts only without it: the call never
    does anything (§6.11 step 7).
18. Confuse fills its context from `aurastat2` on; slot 1 (stat 0,
    value 0) always gives "no stat" in the request (§6.6).
19. Dragon Tail's explosion runs only when the kicked target is still
    alive after the stored hit is applied (§6.19 step 3).
20. Rabies' start evaluates `calc1` / `calc4` and discards them, and
    refuses (0) on a miss (§6.13).
21. Teleport does not test R or the target-position result (§6.5).
22. Conversion's remove callback restores `maxhp(7)` from the saved
    whole-point value without the << 8 the setter used, while Mind
    Blast's callback shifts it (§2.23). As read; a recording should
    confirm the converted monster's maximum life after expiry (Open
    question 7).
23. The Mind Blast conversion test passes on `roll(100)` ≤ chance (not
    <); Conversion's on `roll(100)` < `calc1` (§7.13, §7.19).
24. Strafe's last arrow returns 0, ending the skill without the do
    core's charge step for it (§7.2 step 7).
25. Dopplezon keeps an out-of-range `pettype` as 0 instead of refusing;
    Raven refuses (§3.6, §7.3).
26. Revive goes on to set life, stats, owner and pet entry even when the
    stand-up step found no free point and left the corpse in place
    (§8.7 step 4.2).
27. Whirlwind's hit pacing on a monster is a coin flip per do (0 or 1
    hits); a player's follows the weapon's attack frames (§2.25).
28. Redemption counts redeemed corpses, so its mana cost is paid (unlike
    srvdo 66); Fist of the Heavens accepts overlay 0 (§8.8, §8.9).
29. The Blade Shield pulse lives in table slot srvdo 142, which
    `functions.tsv` lists as unreferenced by data rows (§2.26).

## Test vectors

| Case (1.14d data unless synthetic) | Expected |
|---|---|
| `mode_damage` multiplier, Nightmare, n = 3 / 9 / 1 | 16 / 56 / 0 |
| `mode_damage` multiplier, Normal, n = 8 | 0 |
| `potion_code`, Act 2 Nightmare (i = 6), r = 29 / 35 / 40 (Find Potion `Param3` 30, `Param4` 10) | `mp5` / `rvl` / `hp4` |
| `potion_code`, Act 1 Normal, r = 99 | `hp2` |
| Dragon Talon kick ED, L = 1 / 5 (`Param1` 5, `Param2` 7) | 5 / 33 |
| Dragon Talon L = 6: calc1 `lvl/6+1` = 2 | start: E param 1 = 1, knock 0; do: n = 0, knock rolled |
| Sacrifice, p physical 2560, T life 1280, calc2 8 (synthetic) | self physical = total = pct(1280, 8, 100) = 102 |
| Smite, hit class | 0x65 in every record |
| Jitter callback, total frames 90, argument 3 (synthetic) | frames 77 / 77, seed {target x + 3, 666}, path type 10, steps 77 |
| Static Field, h = 400, C.pct 25, floor 0 (synthetic) | v = 100 << 8 |
| Static Field, lightning resist −50 (synthetic) | v = pct(v, 100, 150) = 2v / 3 |
| Corpse Explosion r = 7 (synthetic) | scan radius 4; physical within d² ≤ 9 |
| Shock Field n = 1 or r = 1 | one missile at the target |
| Leap clamp, unit (0, 0), target (20, 0), r = 10 | (10, 0) before the free-point search |
| Leap candidates around (10, 0) from (0, 0) | (10, 0), (10, −2), (10, 2) |
| Charge velocity: Paladin `RunVelocity` 9, `velocitypercent` 100, `Param1` 150 (1.14d) | pct(9·256, 250, 100) = 5760 |
| Charge hit frame, `seqnum` 4 (record 0 event 1) | 7 |
| Bone Wall from (0, 0) at (10, 0), calc2 = 8 (synthetic) | n = 4; makers aimed at (10, −10) and (10, 10), data +0x2C = 4 |
| Find Item r2 with `Param1` 5, `Param2` 60, `Param3` 30, `Param4` 5 (1.14d) | r2 < 5 → 1; 5…64 → 2; 65…94 → 3; 95…99 → 4 |
| Fire Wall at (10, 0) from (0, 0) | wall missiles aimed at (10, −10) and (10, 10) |
| Charged Strike, unit (0, 0), T (4, 2) | bolts from (4, 2) toward (8, 4) |
| Poison Explosion burst (step 2) | 8 offsets (0, 2), (2, 2), (2, 0), (2, −2), (0, −2), (−2, −2), (−2, 0), (−2, 2) |
| Vengeance hit classes over 4 hits from E param 1 = 0 | 0x20, 0x30, 0x40, 0x20 |
| Blade Fury n = 3 at F = 100 (synthetic) | blade at 100, E param 1 = 102; next blade at F = 103 |
| Leap Attack aim, unit (0, 0), T (10, 0) out of range | (12, 0) before the collision test |
| Strafe count 7, calc3 2, calc1 5 (synthetic) | n = 5; count 1 → 2 |
| Fend count 3, calc1 5 (synthetic) | E param 1 = 3 |
| Conversion of a level-40 monster (M 400, h 200) by a level-20 caster (synthetic) | list 109: 176 = 40, 177 = 400; stats 12 = 20, 7 = 200·256, 6 = 100·256 |
| Bone Prison around (50, 50) | first segment at (49, 46), last at (47, 47) |
| Whirlwind pacing, attack frames f = 26 / 25 / 11 (synthetic) | d = 16 / 14 / 4 |
| Whirlwind pacing, f = 11, n = 100, F = 100 (synthetic) | d = 4; E param 4 = 104; 1 hit (2 with two melee weapons) |
| Hydra at (20, 20) | hydras at (19, 19), (20, 20), (21, 19) |
| Revive life, minHP 10, maxHP 20 (synthetic) | h = (10 + roll(11)) << 8 on T's seed |

## Provenance

- 1.14d disassembly (`py tools/ghidra/disasm.py fn|at`) of every listed
  address; table `0x006E21F4` (multiplier) and `0x00741B54` /
  `0x00741B58` (potion codes) read from the image; column offsets from
  `data/fields.tsv`; values from the 1.14d `patch_d2` tables.
- D2MOO 1.10f read as a hint for `SKILLS_SrvDo007_Jab` and
  `sub_6FC627B0` (mode damage) only; every other body here was read from
  1.14d alone (slot names from `functions.tsv`). 1.14d differences from
  D2MOO: §2.1 mode 6 uses A1; the §2.1 step 9 El1 / `noRatio` bugs are
  in the 1.14d `0x006538A0`.
- Sequence data of `seqnum` 4 (Charge) read from the image through
  `0x007483B8` (frame records of 6 bytes, event byte +5).
- Jump tables read from the image: `0x005A5444` (§2.1 element types),
  `0x005C9B34` (§4.2 target types).

## Open questions

1. Recording: a Jab-using monster and a Smite-using monster: stats 21,
   22, 19 and the element stats before / after `mode_damage`, with a
   `noRatio` class having `El1Mode` set (Edge case 1).
2. Recording: Dragon Talon at L 6 and 12 on an ordinary monster: kicks,
   knockback on the last kick, E param 1 per do.
3. Recording: Find Potion in each act / difficulty (potion codes, unit
   seed draws).
4. Recording: Leap and a monster Leap (Sand Leaper, Ancient Barbarian):
   E flags per do, landing frame, area knockback, the 0xA5 message.
5. Recording: Shock Field: caster seed before / after (re-seed from the
   target x, §2.10).
6. `0x00553010` (the killer looked up by `set_uninterruptable`) and
   message 0xA5 layout: owner `sim/server-messages.tsv`.
7. Recording: Conversion on a higher-level monster: stats 12, 6, 7 while
   converted and after the state expires (Edge case 22).
8. Recording: Holy Freeze on cold-affected monsters: state 107 per pulse
   (draw on the target seed, §6.10).
9. `items` `bitfield1` bit 1 (Iron Golem start, §7.11) and item flag 0x10:
   name both from the items specs.
10. `0x0063C050(inventory, 6 / 5)` (Whirlwind two-weapon test, §2.25):
    which equipped items it returns; owner `items/inventory.md`.
11. `0x00643C50(T, 0, −1)` in Revive (§8.7 step 4.3): what it changes on
    a monster with a right skill.
12. Recording: Whirlwind with one and two weapons: E param 4 and hits per
    do (§2.25 pacing).
