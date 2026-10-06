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
| Summary | 38–47 |
| Inputs | 48–52 |
| Outputs / state changes | 53–58 |
| Rules | 59–60 |
|   1. Conventions | 61–72 |
|   2. Shared helpers, batch 3 | 73–266 |
|   3. Bodies, required level 1 | 267–442 |
| Constants & data dependencies | 443–472 |
| Randomness | 473–488 |
| Edge cases & original bugs | 489–504 |
| Test vectors | 505–518 |
| Provenance | 519–530 |
| Open questions | 531–540 |
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

## Provenance

- 1.14d disassembly (`py tools/ghidra/disasm.py fn|at`) of every listed
  address; table `0x006E21F4` (multiplier) and `0x00741B54` /
  `0x00741B58` (potion codes) read from the image; column offsets from
  `data/fields.tsv`; values from the 1.14d `patch_d2` tables.
- D2MOO 1.10f compared: `SKILLS_SrvDo007_Jab`, `SrvDo017`, `SrvDo064`,
  `SrvDo150`, `SrvDo069`, `SrvDo114`, `SrvDo117`, `SrvSt22`, `SrvDo033`,
  `SrvSt24`, `SrvDo042`, `sub_6FC627B0` (mode damage), `sub_6FD14D20`.
  1.14d differences: §2.1 mode 6 uses A1; §2.1 step 9 El1 / `noRatio`
  bugs are in `0x006538A0`.

## Open questions

1. Recording: a Jab-using monster and a Smite-using monster: stats 21,
   22, 19 and the element stats before / after `mode_damage`, with a
   `noRatio` class having `El1Mode` set (Edge case 1).
2. Recording: Dragon Talon at L 6 and 12 on an ordinary monster: kicks,
   knockback on the last kick, E param 1 per do.
3. Recording: Find Potion in each act / difficulty (potion codes, unit
   seed draws).
