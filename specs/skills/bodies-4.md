# Spec: Skills — Start and do function bodies, batch 4 (continued)

- **Status:** draft: every body and helper below read from the 1.14d
  `Game.exe` disassembly (`py tools/ghidra/disasm.py fn|at`); tables read
  from the image; use counts from the 1.14d `patch_d2` `skills.txt`.
  D2MOO 1.10f names only. No recording covers these bodies yet (Open
  questions).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::skills::use_::bodies` (`functions.tsv` rows
  whose note says `bodies-4.md`)
- **Related specs:** `skills/bodies-3.md` (first part of batch 4: §1
  conventions, §3 helpers, order table in Constants); `skills/bodies.md`;
  `skills/bodies-2.md`; `skills/use.md`; `skills/levels.md`;
  `combat/damage.md`; `missiles/missiles.md`; `monsters/init.md`;
  `monsters/ai.md`; `sim/tick.md`; `sim/rng.md`; `skills/functions.tsv`.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 36–45 |
| Inputs | 46–50 |
| Outputs / state changes | 51–56 |
| Rules | 57–58 |
|   1. Conventions | 59–67 |
|   2. Shared helpers, batch 4 (continued) | 68–198 |
|   3. Bodies used by one monster skill (continued) | 199–522 |
|   4. Bodies used by no monster skill row | 523–651 |
| Constants & data dependencies | 652–663 |
| Randomness | 664–679 |
| Edge cases & original bugs | 680–712 |
| Test vectors | 713–723 |
| Provenance | 724–734 |
| Open questions | 735–756 |
<!-- /index -->

## Summary

The second part of batch 4 (`bodies-3.md` Summary): the monster skill
bodies from Mosquito (skill 206) to NecromageMissile (338) in the order
of `bodies-3.md` Constants, "Batch 4 order", then the 11 slots no live
`skills.txt` row names in `srvstfunc` / `srvdofunc`: the Assassin
progressive functions (`srvprgfunc1`–`3`), the Hurricane / Armageddon /
attached state functions (`states.txt` `srvactivefunc`, run by type-5
events) and the Chain Lightning item effect.

## Inputs

As `bodies.md` (game, unit, skill, level L, `skills.txt` record R, target
T, frame F, difficulty).

## Outputs / state changes

As `bodies.md`: combat records, missiles and their paths, monsters, state
lists, timers, unit fields, the AI and its commands, and the value the
core reads.

## Rules

### 1. Conventions

`bodies-3.md` §1 (with `bodies.md` §1 and `bodies-2.md` §1) applies.
Helpers of `bodies-3.md` are cited as `bodies-3.md` §n. "S" is a local
seed (`sim/rng.md` §4: `init_low(x)` = {x, 666}) that the body draws on
instead of a unit seed. "Source of U" = the unit linked by `link_source`
(`bodies.md` §6.20), read by `0x00552FD0(game, U)` (none when U +0xC8 bit
0x400 is clear).

### 2. Shared helpers, batch 4 (continued)

#### 2.1 Weapon skill roll `0x0057E270`

`weapon_roll(game, unit, T, skill, L, record)` (ECX game, EDX unit; T
unread): R invalid → nothing. mn = `phys_min(unit, skill, L, 0)`, mx =
`phys_max(unit, skill, L, 0)` (`levels.md` §3.3); physical (+0x08) :=
`bonuses(unit, get 0, item none, mn, mx, 0, 0, 128)` (`combat/damage.md`
§3.2); `roll_elemental(unit, record, skill, L)`.

#### 2.2 Stacking state list `0x00570590`

`state_list(game, src, U, s, skill, L)` (ECX game, EDX src; `ret 0x10`):
R invalid → none. e = `eval(src, auralencalc)`. Alloc (game pool, flags
2, expire F + e, owner src's type / GUID, 6 / −1 without src); failure →
none. State id := s; remove callback := default `0x0056E900`; attach to
U; state s on for U; timer 12 at F + e on U; `aura_fill(src, list, R,
skill, L)` (`bodies.md` §2.6). Return the list.

No existing list of s is looked for: every call adds one more list.

#### 2.3 Imp possess and release

- **Possess** `0x005D18E0(game, imp, K, skill, L)`: imp and K non-null,
  the imp's `BaseId` (`0x00463860`) = 492 (`imp1`) and K's `BaseId` ∈ {435
  `barricadetower`, 441 `siegebeast1`}; else nothing. State 143
  (`attached`) on the imp; `link_source(imp, K)`, `link_source(K, imp)`;
  delete the imp's type-5 timers with argument skill; every-tick type-5
  event with arguments (skill, L) (`0x00541650`, `sim/tick.md` §5.3: that
  table's "a druid skill" caller is this helper); imp flags (+0xC4) &=
  ~0xE; AI state 16 (`0x005B0E00(game, imp, the imp's AI control or none,
  16)`, `monsters/ai.md` §3.3).
- **Release** `0x005D19D0(game, imp, K, skill, L)`: the same class test;
  state 143 off; `link_source(imp, none)`, `link_source(K, none)`. The
  imp's life percent (`0x00621F20`: trunc(100·(life >> 8) / (max life >>
  8)), 0 when the maximum is below 256) < 10 → kill the imp
  (`0x0057CCB0(game, imp, none, 1)`) and stop. Else imp flags |= 0xE;
  delete its type-5 timers with argument skill; AI state 0 (control from
  `0x00541860`).

The type-5 event runs state 143's `srvactivefunc`, srvdo 147 (§4.10),
every tick while the imp rides.

#### 2.4 Lightning fan `0x005D4870`

`zigzag(game, unit, m, x, y, skill, L, step)` (EDX v = a value the caller
drew; stack the rest; `ret 0x20`; Fists of Fire `srvprgfunc1`, Royal
Strike `srvprgfunc2`):

1. S := `init_low(v)`.
2. Record: flags 3 (position given, target relative), owner the unit,
   class m, position (x, y), skill, L, init callback `0x005D4680`.
3. For i = 0, step, 2·step, … while i < 64: target offset := the ring
   offset i (radius 30, `bodies.md` §6.7); one step of S, callback
   argument := S's `lo'`; create M. M made and m = 568
   (`royalstrikechainlightning`) → M data +0x28 := `Param2` of skill 280
   (Royal Strike, `0x004EFC20`) + 1.

A step ≤ 0 never ends the loop (Edge case 1).

Callback `0x005D4680` (ECX M, EDX argument a; M none → nothing):

1. n = M's total frames; n ≥ 78 → n = 77, total and left := 77.
2. M seed := `init_low(a)`. Path type := 10; step counts := n; pts =
   the path's point array (`0x006487D0`).
3. (px, py) = M's position; d = `dir64(M, P target x, P target y)`
   (`bodies-3.md` §3.8).
4. One step of M's seed: s = +1 when `lo'` is odd, else −1. One more
   step: k = (`lo'` mod 3) + 2 (constant `0x00741B10`).
5. For i = 0…n − 1: d += k·s; e = `dir8`(d with bits 6–7 cleared); (px,
   py) += (X[e], Y[e]) with X = (2, 2, 0, −2, −2, −2, 0, 2) (`0x006E32AC`),
   Y = (0, 2, 2, 2, 0, −2, −2, −2) (`0x006E328C`); point i := (px, py)
   (u16 each). i mod 15 (`0x00741B0C`) = 0 → s := −s and k := (`lo'` of
   one more step of M's seed mod 3) + 2.
6. P's point count := n (`0x00648790`, path +0x28, capped 77).

Nothing else of P is written: no compute-path call (unlike
`bodies-2.md` §2.3 step 3), the current point index (path +0x24) keeps
its creation value, and the flags (+0x34) change only through the
path-type setter of step 2 (`0x00648CF0`, `sim/pathing.md` §2). Answered
(2026-10-08, impl-missile-init MI4).

The path is written point by point: each missile zigzags from its ring
direction.

#### 2.5 Lightning ring `0x005D4150`

As §2.4 with three differences: init callback `0x005D40F0`; M data +0x28
:= Royal Strike `Param2` + 1 for **every** created missile, read through
M without a null test (a failed creation is a fatal access in the
original); no class test. Claws of Thunder `srvprgfunc3`.

Callback `0x005D40F0` (ECX M, EDX a): frames capped at 77 as above; M
seed := `init_low(a)`; path type := 10; step counts := n; compute the
path (`0x00649970(P, M, 0)`, the charged-bolt jitter of `sim/pathing.md`
§11.2).

#### 2.6 Whip helpers

- **Whip state** `0x005D1D70(T, skill, L, unit)` (ECX T, EDI skill, EBX
  L): R invalid → nothing. `apply_state` {source the unit, target T,
  skill, L, duration `eval(unit, auralencalc)`, stat −1, value 0, state
  `auratargetstate`, default callback} (`bodies.md` §2.7); a list →
  `aura_fill(unit, list, R, skill, L)`.
- **Transform** `0x005D1E10(unit, L)` (EAX skill, ESI T, EBX game): R
  invalid → nothing. c = `summon_class(unit, skill, L, &mode)`
  (`bodies.md` §6.1; mode overwrites the L slot); c < 0, T none or not a
  monster → nothing. k = the chain position of T's class (monstats
  +0x4B, `0x006510C0`); c' = `0x0054DA60(c, k)` (c's `BaseId` followed k
  steps along `NextInClass`, `monsters/population.md` §11.5 rule 3).
  Class change `0x00574370(game, T, c', mode)` (only when c' is a valid
  monstats row with `enabled`: monster data released `0x005736A0`, T
  class := c', monster data rebuilt `0x00574250(game, T's room, T, T
  GUID)` (`monsters/init.md` §3–§6, draws on T's seed), mode := mode
  `0x00624690`). AI state 15 (`0x005B0E00(game, T, T's AI control,
  15)`). Mode request mode for T (flag 1). State 142 (`changeclass`) on
  T. A list (game pool, flags 0, expire 0, owner the unit's type / GUID)
  attached to T with state 142 and stat 355 (`shortparam1`) := c'
  (failure → no list). `sumumod` (+0xE4) in 1…42 → `0x005A4850(game, T,
  sumumod, 0)`.

#### 2.7 Vine missiles `0x005D1C30`

`vines(game, unit, n, m, skill, L)` (EAX unit, EDI game, EBX n; stack m,
skill, L): record flags 3 (position given, target relative), owner the
unit, class m, skill, L, init callback jitter `0x005C9290`
(`bodies-2.md` §2.3). Start := the target position (`bodies.md` §2.4);
failure → 0. For i = 0…n − 1: target offset := (X, Y)[i & 3] = (0, 1),
(0, −1), (1, 0), (−1, 0) (`0x006E3254` / `0x006E3264`); argument := i;
create. Return 1.

### 3. Bodies used by one monster skill (continued)

Continues `bodies-3.md` §5 in the same order.

#### 3.1 srvst 55 Mosquito `0x005CD910`

1. R invalid → 0. E none → 0. T none → 0.
2. S := `init_low(T GUID + the unit's GUID)` (−1 for no unit;
   `0x00650E30` then `0x00650E40`).
3. a = `eval(calc1)`, b = `eval(calc2)`; n = a + `roll_S(b − a)` (no draw
   when b ≤ a); n < 1 → 1.
4. E param 1 := n; param 2 := T GUID; param 3 := T type. Return 1.

The bite count depends only on the two GUIDs and the formulas.

#### 3.2 srvdo 107 Mosquito `0x005CDA00`

1. R invalid → 0. E none → 0.
2. K = the unit of (type E param 3, GUID E param 2); none, or not in melee
   range (`0x00622C40(unit, K, 0)`) → 0.
3. n = E param 1. Zeroed record.
4. a = `phys_min(unit, skill, L, 1)` >> 8, b = `phys_max(unit, skill, L,
   1)` >> 8. `roll_physical(unit, record, skill, L)`;
   `roll_elemental(unit, record, skill, L)`.
5. With d = b − a, three draws on the unit's seed: poison (+0x28) := 2·(a
   + `roll(d)`); poison length (+0x2C) := `elem_len(unit, skill, L, 1)`;
   mana leech (+0x3C) := (a + `roll(d)`) << 8; stamina leech (+0x40) :=
   (a + `roll(d)`) << 8. Result := 1.
6. Unit life (stat 6) := min(life + pct(physical, `eval(calc3)`, 100),
   maximum life).
7. `apply(game, unit, K, 1, record)`; K's life (raw) = 0 → result |= 2;
   reaction `0x0057CEE0(game, unit, K, record)`.
8. n − 1 > 0: E param 1 := n − 1; frame event index := `Param1`; frame
   count := (count & ~0xFF) + 0x100 (the bite repeats). Return 1.

#### 3.3 srvdo 112 MonCurseCast `0x005CE2B0`

The skill argument does not choose the curse.

1. One step of the unit's seed: c = `lo'` mod 5. k = table `0x006E3228`[c]
   = (66 Amplify Damage, 72 Weaken, 82 Life Tap, 87 Decrepify, 91 Lower
   Resist)[c]. k invalid or k's `auratargetstate` not in 0…count − 1 → 0.
2. Unit flags |= 0x40.
3. r = `eval(k's aurarangecalc)`, d = `eval(k's auralencalc)` (evaluated
   with skill k and the caster's L).
4. Curse context (`bodies.md` §4.4 step 5 layout, zeroed): game, unit,
   ai 0, upd 0, skill k, L, duration d, stats 1–6 := −1, values 0, state
   := k's `auratargetstate`, events 0. Then by c (jump table
   `0x005CE4DC`):

   | c | k | context |
   |---|---|---|
   | 0 | 66 | stat 1 = 36 `damageresist`, value −100 |
   | 1 | 72 | stat 1 = 25 `damagepercent`, value −50 |
   | 2 | 82 | event 1 = 5, func 1 = 4 |
   | 3 | 87 | event 1 = 1, func 1 = 5; event 2 = 2, func 2 = 5 |
   | 4 | 91 | upd 1; stats 67 `velocitypercent` := k's `Param7`, 25 := −50, 36 := −50, 68 `attackrate` := `Param7` |

   (A default case giving the five resistances −`0x00645C10(L, k)` is
   unreachable: c ≤ 4.)
5. Return `scan_point(game, f = 3, unit, r, 0x005C35C0, context)`
   (`bodies.md` §2.12, the curse callback of §4.4).

Lower Resist's row puts the Decrepify stats in its context and Decrepify
gets event handlers; with `Param7` 0 in 1.14d the c = 4 slot-1 value is
0 and the callback skips every unit (Edge case 2).

#### 3.4 srvdo 108 RegurgitatorEat `0x005CDC10`

1. R invalid → 0. T none, not a monster, a hireling (`0x0063EE90`) or
   not in mode 12 (dead) → 0.
2. M_T = T's maximum life. Room delete notice `0x0061A270(T's room, 1, T
   GUID)`; remove T (`0x00555600(game, T)`, `sim/units.md`).
3. h = the unit's life, M = its maximum life. M_T > 0: life := min(max(h
   + pct(M_T, `eval(calc1)`, 100), 1), M) (M when the max(…, 1) ≥ M).
   Return 1.

#### 3.5 srvdo 125 Wake Of Destruction Sentry `0x005D1170`

1. R invalid → 0. m = `prog_missile(unit, skill)` (`bodies.md` §6.6); not
   0 ≤ m < count → 0.
2. Target position (tx, ty) fails → 0. Unit flags |= 0x40.
3. Record: flags 2 (target relative; starts at the origin), owner =
   origin = the unit, class m, skill, L, offset (dx, dy) = (tx − ux, ty −
   uy). Create M; none → 0.
4. M data +0x28 := −dy, +0x2C := dx (`0x0064A710`, `0x0064A760`: the
   perpendicular of the aim, read by the missile). Return 1.

#### 3.6 srvst 59 Imp Inferno `0x005D1280`

As srvst 53 (`bodies-3.md` §4.1) with `calc1` instead of `calc2`: R
invalid or E none → 0; E param 1 := F + max(`eval(calc1)`, 1); state 12
on if absent. Return 1.

#### 3.7 srvdo 126 Imp Inferno `0x005D1350`

1. R invalid → 0. m = `srvmissilea`; not 1 ≤ m < count (0 refused) or
   no record → 0. E none → 0.
2. T and P exist → P target point := T's position (`0x00648AD0`); path
   target := T (`0x00620C10`).
3. Record: flags 0x25, owner the unit, position the unit's, class m,
   skill, L, velocity `Vel` + trunc(`VelLev` × L / 8), target := P's
   target point.
4. Create M; M → n = `eval(calc2)` + missiles `Param2` (+0x3C); M's path
   step counts, total frames and frames left := n.
5. Inferno animation (`bodies-3.md` §3.2) with R.
6. F < E param 1 and state 12: delete type-1 timers; type-0 timer at F
   + 3. Return 1.
7. Else: delete type-0 timers; **type-2** timer (AI think) at F + 13.
   Return 1. State 12 stays on.

#### 3.8 srvdo 141 Baal Corpse Explode `0x005D2E80`

1. Request (`missiles/bodies-2.md` §44 layout): flags 0x3002, unit,
   skill **285** (fixed), (x, y) = the unit's position, radius := R
   valid and L > 0 ? `Param5` + (L − 1)·`Param6` (+0x158, +0x15C) : 0,
   level L, callback `0x005D2E50`.
2. `corpse_effect` run `0x0056DCC0` (`missiles/bodies-2.md` §44 steps
   1–5: radius 0 → skill 285's `Param1` + (L − 1)·`Param2`). Return 1.

Callback `0x005D2E50(ECX game, EDX unit; U, L, last)`: path target :=
U (`0x00620C10`); Corpse Explosion body srvdo 55 (`bodies-2.md` §4.5) on
(game, unit, 285, L) (`0x005C50B0` jumps to it). Return its value.

#### 3.9 srvst 60 Suck Blood `0x005D1570`

1. R invalid → 0. T none → 0.
2. Zeroed record; `skill_result(game, unit, T, R, skill, L, record, 0)`
   (`bodies-2.md` §2.17).
3. Miss → return 1 (no combat entry). Hit:
   1. Hit flags := 2 | `HitFlags`; `HitClass` ≠ 0 → hit class := it.
   2. `weapon_roll(game, unit, T, skill, L, record)` (§2.1).
   3. Physical += pct(physical, `eval(calc1)`, 100).
   4. Life leech (+0x38) += `eval(calc2)`; mana leech (+0x3C) +=
      `eval(calc3)`.
   5. `start_combat(game, unit, T, record, 128)`.
4. Return 1.

#### 3.10 srvdo 127 Suck Blood `0x005D16A0`

1. R invalid → 0. T none → 0. Unit flags |= 0x40.
2. p = `pair_record(unit, T)` (`bodies.md` §2.2); none → 0.
3. p hit: H = the unit's minion owner when the unit is a monster with
   one, else the unit itself when it is a monster; a non-monster gives no
   H (getters on none, Edge case 3). d = min(p physical, T's life (raw
   stat 6)); H life := min(max(H life + pct(d, `eval(calc1)`, 100), 1), H
   maximum life).
4. `apply_melee(game, unit, T)`. Return 1.

#### 3.11 srvdo 128 Cry Help `0x005D1800`

1. R invalid → 0. T none, unit none or not a monster → 0.
2. Command {+0x08 type 1, +0x0C T type, +0x10 T GUID, +0x14 F +
   max(`eval(calc1)`, 1), +0x18 never written (Edge case 4)} copied to
   every minion of the unit's minion owner (`0x0058F730`, `monsters/ai.md`
   §8).
3. `srvoverlay` in 1…overlay count − 1 → overlay on T. Return 1.

#### 3.12 srvst 61 Self-resurrect `0x005D1BF0`

No Ghidra function at this entry. Unit none or not a monster → 0.
`revive(game, unit)` (`bodies-3.md` §3.6); unit flags &= ~0xE; footprint
mask := 0x100. Return 1.

#### 3.13 srvdo 130 Vine Attack `0x005D1CD0`

1. R invalid → 0. m = `srvmissilea`; not 1 ≤ m < count → 0.
2. Unit flags |= 0x40.
3. n = `eval(calc1)`; n ≤ 0 → 0.
4. `vines(game, unit, n, m, skill, L)` (§2.7). Return 1.

#### 3.14 srvdo 131 Overseer Whip `0x005D1F70`

1. R invalid or `auratargetstate` invalid → 0.
2. T none or dead (`0x005541B0`) → 0.
3. T is a monster of `BaseId` 453 (`minion1`): p = `eval(calc1)`; r =
   `roll(100)` on the unit's seed. r ≥ p and T lacks `auratargetstate`
   → transform (§2.6) with (unit, L); return 1.
4. Every other case (not a minion, r < p, or the state present): whip
   state (§2.6) on T. Return 1.

In 1.14d (`summon` `suicideminion1`, `auratargetstate` `bloodlust`,
`calc1` `par1`) a whipped minion without `bloodlust` turns into the
matching `suicideminion` when r ≥ p; otherwise, or when already
bloodlusted, it gets (or refreshes) `bloodlust`.

#### 3.15 srvdo 132 Imp Fire Missile `0x005D2090`

1. R invalid → 0. m0 = `srvmissilea`; not 1 ≤ m0 < count → 0.
2. Unit flags |= 0x40.
3. m = m0 + the chain position of the unit's class (`bodies-3.md` §4.9;
   no range check on m).
4. Straight `skill_missile(game, m, unit, skill, L, 0, 0, 0, 0, quant
   0)`. Return 1.

#### 3.16 srvdo 133 Impregnate `0x005D2250`

1. R invalid → 0. Unit flags |= 0x40.
2. T none → 0. `0x005D2140(T)` = 0 → 0: T must be a living monster whose
   `BaseId` is neither 546 (`putriddefiler1`) nor 551 (`painworm1`), lack
   state 110 (`pregnant`) and have alignment ≠ 2.
3. `apply_state` {source the unit, target T, skill, L, duration 0, stat
   −1, state 110, callback `0x005D21B0`}. Return 1.

Remove callback `0x005D21B0` (ECX T, EDX state, stack list): state off.
c = 551, mode = 1; the list's skill valid: `summon` in 1…monstats count
− 1 → c := it; `summode` in 0…15 → mode := it. T dead → spawn near T
`0x005B2490(game, T, c, mode, 1, 0)` (`monsters/init.md` §1 table). No
handler unregistering, no anim refresh.

A pregnant monster that dies releases a `summon` (1.14d: `painworm1`).

#### 3.17 srvdo 134 Siege Beast Stomp `0x005D2320`

1. R invalid → 0. `srvmissilea` not in 1…count − 1 → 0 (the missile is
   not used otherwise).
2. Unit flags |= 0x40.
3. Zeroed record: hit flags := 1 | `HitFlags`; result |= `ResultFlags`;
   `HitClass` ≠ 0 → hit class.
4. `roll_physical`, `roll_elemental`.
5. `area_damage(game, unit, 0, 0, eval(aurarangecalc), record, 0)`
   (`missiles.md` §R9, filter 0x8583). Return 1.

1.14d Siege Beast Stomp has no `srvmissilea`: the body returns 0 at
step 1 and stomps nothing (Edge case 5).

#### 3.18 srvst 62 MinionSpawner `0x005D2420`

E none → 0. c = `spawn_class(unit, skill, L, &mode, &x, &y)`
(`bodies-3.md` §3.1; c is not tested). E params 1…4 := c, x, y, mode.
Return 1. (R is not tested.)

#### 3.19 srvdo 135 MinionSpawner `0x005D2490`

1. R invalid → 0. E none → 0.
2. c, x, y, mode := E params 1…4. Room at (x, y) none → 0.
3. m = `0x005B2F20(game, room, x, y, c, mode, spread −1, flags 0)`; none
   → the same with spread 4; none → 0.
4. m flags |= 0x4020000; `link_source(m, unit)`; `sumoverlay` in 0…count
   − 1 → overlay on m. Return 1.

#### 3.20 srvdo 136 DeathMaul `0x005D25B0`

1. R invalid → 0. m = `srvmissilea`; not 1 ≤ m < count → 0.
2. Target position (tx, ty) fails → 0. Unit flags |= 0x40.
3. M = `missile_at(game, unit, skill, L, m, 0, 0)` (`bodies.md` §6.13,
   which reads the target position again); none → 0.
4. a = `eval(calc1)` > 0 → M animation speed := clamp(a, 0, 0x7FFF).
5. f = M's total frames; d = max(distance(unit, tx, ty), 1)
   (`0x006417F0`). n = trunc(d·f / 24) + trunc(f / 2) for d < 10, else
   trunc(d·f / 12). M's total frames and frames left := n. Return 1.

#### 3.21 srvdo 137 fenris rage `0x005D26F0`

1. R invalid or `aurastate` invalid → 0.
2. Unit flags |= 0x40. T none → 0. T fails the corpse test
   `0x00645590` (`bodies.md` §3.9) → 0.
3. State 104 (`corpse_nodraw`) on T; queue T for update.
4. `state_list(game, unit, unit, aurastate, skill, L)` (§2.2). Return 1.

Each eaten corpse adds one more list of the state (Edge case 6).

#### 3.22 srvdo 140 Baal Tentacle `0x005D2C20`

1. Unit none → 0. (R is not tested.)
2. One step of the unit's seed: n = (`lo'` mod 3) + difficulty + 2.
3. Spawn info `0x0063EFA0(unit, &c, &x0, &y0, &mode, difficulty, 0)`
   (`monsters/ai-bodies-2.md` §13.1; x0, y0 unused). For Baal (`BaseId` 544)
   it compares the incoming c (an uninitialised local here, Edge case 7)
   with 570; otherwise: c := `0x0054DA60(562 baaltentacle1, roll(2) +
   difficulty)`, two `roll(24)` draws for its own x, y, mode := 4 (three
   draws on the unit's seed).
4. (bx, by) = T's position, or the unit's without T.
5. n times: x = bx + (`lo'` mod 18) − 9, y = by + (`lo'` mod 18) − 9 (two
   steps of the unit's seed); room at (x, y) none → next; m =
   `0x005B2F20(game, room, x, y, c, mode, −1, 0)`; m → flags |=
   0x4020000; think in 15 frames (`0x005DE0F0(game, m, 15)`,
   `monsters/ai-bodies-2.md` "wait N"); `link_source(m, unit)`.
6. Return 1.

#### 3.23 srvdo 139 Baal Cold Missiles `0x005D2940`

1. R invalid → 0. m = `srvmissilea`; not 1 ≤ m < count → 0.
2. Unit flags |= 0x40. Target position (tx, ty) fails → 0.
3. M = straight `skill_missile(game, m, unit, skill, L, 0, 0, 0, 0, quant
   0)`; none → 0.
4. M data +0x28 := −(ty − uy), +0x2C := tx − ux. Return 1.

#### 3.24 srvdo 129 Imp Teleport `0x005D1AB0`

1. R invalid or `aurastate` invalid → 0.
2. O = the source of the unit (§1). T = target.
3. O exists (riding): target position (x, y) (`0x0056D2C0`,
   `bodies.md` §2.4; result not tested: it always writes both
   coordinates, from the target unit or else the path's target point,
   and "fails" only when one of them is 0, which is then used as is);
   `0x00554EA0(game, unit, room none, x, y, 0, 0)` (result not read);
   release (§2.3) with (unit, O, skill, L). Return 1.
4. No O, no T → return srvdo 98 (`bodies-3.md` §4.3).
5. No O, T: T has a source → 0; T dead → 0;
   `0x00554EA0(game, unit, T's room, Tx, Ty, 1, 0)` = 0 → 0; possess (§2.3)
   with (unit, T, skill, L). Return 1.

#### 3.25 srvdo 148 DoomKnightMissile `0x005CDFB0`

1. R invalid → 0. T none → 0.
2. m = `srvmissilea`; < 0 → 0. A monster adds its component byte S3
   (monster data +0x0E, `monsters/init.md` §10); m ≥ count or no
   missiles record → 0.
3. Unit flags |= 0x40.
4. R `lob` → lob `skill_missile`, else straight, with (m, unit, skill,
   L, 0, 0, 0, 0, quant 0). Return 1.

S3 is the byte monster init wrote (§10 variant, then doomknight2/3's
own roll of 4), read live from the monster data, never a default 0;
L is the used entry's level, for a monster its init entry's base
(`Sk<i>lvl` plus the bonus, `monsters/init.md` §6), not 1. Measured:
`milestone-hellforge` frame 83, three doomknight2 (`Sk1lvl` 3) fire
missiles 324, 323, 324 at missile level 3 (rc-coverage-warps).

#### 3.26 srvdo 149 NecromageMissile `0x005CE0B0`

As §3.25 with component byte S4 (monster data +0x0F).

### 4. Bodies used by no monster skill row

#### 4.1 srvdo 36 Claws of Thunder `srvprgfunc2` `0x005D4DB0`

Also the `ItemEffect` of Frost Nova and Nova (`use.md` §5.4 step 5).

1. T none → 0. L ≤ 0 → 0. R invalid → 0. m = `prog_missile(unit,
   skill)`; m ≤ 0 → 0.
2. v = `Vel` + trunc(`VelLev` × L / 8) of m (`0x00663270`) +
   `eval(calc1)`.
3. `ring(game, unit, T, m, skill, L, v)` (`bodies.md` §6.7: 64 missiles
   around **T**). Return 1.

#### 4.2 srvdo 37 Claws of Thunder `srvprgfunc3` `0x005D4E70`

1. One step of the unit's seed: v = `lo'`.
2. R invalid → 0. m = `prog_missile`; ≤ 0 → 0. Target position (tx, ty)
   fails → 0.
3. s = `prog_count(unit, skill, L)` (`bodies-2.md` §2.11); 0 → s =
   `eval(aurarangecalc)`.
4. Lightning ring (§2.5) with (unit, m, tx, ty, skill, L, step s), seeded
   with v. Return 1.

#### 4.3 srvdo 38 Fists of Fire, Blades of Ice `srvprgfunc2` `0x005D3E80`

1. R invalid → 0.
2. T exists → `apply_melee(game, unit, T)`.
3. Target position (tx, ty) fails → 0.
4. r = `prog_count(unit, skill, L)`; 0 → `eval(aurarangecalc)`.
5. Zeroed record; `roll_physical`, `roll_elemental`; result |= 1; hit
   flags |= 1.
6. `scan_unit(game, unit, tx, ty, r, aurafilter, 0x005D3CB0, record,
   noaura 0)` (`bodies.md` §2.12): per unit U the callback copies the
   record and runs the area-damage unit step `0x0056B9C0(game, unit, U,
   copy, 0)` (`missiles.md` §R9 `area_damage`). Return 1.

#### 4.4 srvdo 39 Fists of Fire, Blades of Ice `srvprgfunc3` `0x005D3F90`

1. One step of the unit's seed: v = `lo'`.
2. R invalid → 0. Target position (tx, ty) fails → 0. m =
   `prog_missile`; ≤ 0 → 0.
3. r = `prog_count`; 0 → `eval(aurarangecalc)`.
4. Record: flags 1 (position given), owner the unit, class m, skill, L.
5. S := `init_low(v)`. r² times: a = r − `roll_S(2r)`, b = r −
   `roll_S(2r)`; a² + b² ≤ r² and a room exists at (tx + a, ty + b)
   (from the unit's room) → position := it; create.
6. Return 1.

#### 4.5 srvdo 40 Royal Strike `srvprgfunc1` `0x005D5010`

Target position (tx, ty) fails → 0. L ≤ 0 → 0. m = `prog_missile`; ≤ 0
→ 0. `missile_at(game, unit, skill, L, m, tx, ty)` (`bodies.md` §6.13).
Return 1. (R is not tested.)

#### 4.6 srvdo 41 Royal Strike `srvprgfunc3` `0x005D5080`

1. One step of the unit's seed; S := `init_low(lo')`.
2. R invalid → 0. Target position (tx, ty) fails → 0. m =
   `prog_missile`; ≤ 0 → 0. n = `prog_count(unit, skill, L)`; 0 → 0.
3. Record: flags 3, owner the unit, class m, position (tx, ty), skill, L.
4. n times: a = (`lo'` of S mod 40) − 20, b = (next `lo'` mod 40) − 20; a
   = b = 0 → a := 20. Target offset (a, b); create M; M → data +0x28 :=
   S's current `lo` (`0x00650E50`, no step), data +0x2C := (b & 0xFFFF)
   << 16 | (a & 0xFFFF).
5. Return 1.

#### 4.7 srvdo 143 Fists of Fire `srvprgfunc1`, Royal Strike `srvprgfunc2` `0x005D4F40`

1. One step of the unit's seed: v = `lo'`.
2. R invalid → 0. m = `prog_missile`; ≤ 0 → 0. Target position (tx, ty)
   fails → 0.
3. s = `prog_count`; 0 → `eval(aurarangecalc)`.
4. Lightning fan (§2.4) with (unit, m, tx, ty, skill, L, step s),
   seeded with v. Return 1.

#### 4.8 srvdo 145 Hurricane state function `0x005C8380`

`states.txt` `srvactivefunc` of `hurricane`, run by the type-5 event
that srvdo 124 schedules (`bodies.md` §8.22 step 6). "End" = delete the
unit's type-5 timers with argument skill; return 0.

1. R invalid, L ≤ 0, or `aurastate` invalid → end.
2. The unit lacks the state → end.
3. R lacks `InTown` and the unit's room is in town: the state's list (if
   any) detached and freed, state off → end.
4. r = `eval(aurarangecalc)` ≤ 0 → end.
5. Delete type-5 timers (argument skill); type-5 timer at F + `Param4`
   with (skill, L).
6. The unit's room none or in town → return 0.
7. Zeroed record; `roll_physical`, `roll_elemental`; result |=
   `ResultFlags`; hit flags |= `HitFlags`.
8. `area_damage(game, unit, 0, 0, r, record, aurafilter)`. Return 1.

#### 4.9 srvdo 146 Armageddon state function `0x005C8520`

Steps 1–3 and "end" as §4.8 (state `armageddon`).

4. m = `prog_missile`; ≤ 0 → end. E = the unit's entry of skill
   (`0x006439F0`); none → end. r = `eval(aurarangecalc)` ≤ 0 → end.
5. Re-arm the type-5 timer as §4.8 step 5.
6. The unit's room none or in town → return 0.
7. The unit's seed := `init_low(E param 1)` (the value srvdo 124 drew);
   E param 1 := `roll(1000000)` on it.
8. Up to 5 tries: x = ux + `roll(2r + 1)` − r, y = uy + `roll(2r + 1)` − r
   (unit seed); accept when the line test `0x00645950(x, y, unit, 0x805)`
   ≠ 0 (clear), a room exists at (x, y) and the point collision
   `0x0064CB30(room, x, y, 1)` = 0. None accepted → return 0.
9. `missile_at(game, unit, skill, L, m, x, y)`; `msg_a3(unit, none,
   skill, L, x, y, 0)` (`bodies-2.md` §2.21). Return 1.

#### 4.10 srvdo 147 Attached state function `0x005D2B20`

No Ghidra function at this entry. O = the source of the unit; O → the
unit's path takes O's precise position and room (`0x006505E0(unit, O)`
→ `0x0064FB90`; path flags +0x34: bit 8 set when the sub-tile differs,
cleared when equal; bit 1 set when the room differs). Return 1.

The riding imp (§2.3) follows its tower or siege beast every tick.

#### 4.11 srvdo 151 Chain Lightning item effect `0x005CA260`

1. R invalid → 0. m = `srvmissilea`; not 0 ≤ m < count → 0.
2. Unit flags |= 0x40. n = `eval(calc1)`.
3. (dx, dy) = T's position − the unit's, or (0, 0) without T.
4. M = straight `skill_missile(game, m, unit, skill, L, dx, dy, 0, 0,
   quant 0)` (starts on T, aims at the target position); none → 0.
5. M data +0x28 := n (jump count, as srvdo 26 `bodies-2b.md` §6.4).
   Return 1.

## Constants & data dependencies

| Item | Value | Where |
|---|---|---|
| curse table | `0x006E3228` = 66, 72, 82, 87, 91; jump table `0x005CE4DC` | §3.3 |
| vine offsets | X `0x006E3254` (0, 0, 1, −1), Y `0x006E3264` (1, −1, 0, 0) | §2.7 |
| zigzag steps | X `0x006E32AC`, Y `0x006E328C`; flip period 15 (`0x00741B0C`), turn base 2 (`0x00741B10`) | §2.4 |
| classes tested | `BaseId` 453 minion1, 492 imp1, 435 barricadetower, 441 siegebeast1, 546 putriddefiler1, 551 painworm1, 544 baalcrab, 562 baaltentacle1; missile 568 royalstrikechainlightning; skills 280 Royal Strike, 285 Baal Corpse Explode | |
| state ids | 104 corpse_nodraw, 110 pregnant, 142 changeclass, 143 attached | |
| stat ids | 25, 36, 67, 68, 355 shortparam1 | |
| skills columns | `Param1`–`Param7` +0x148–+0x160, `aurastate` +0x80, `auratargetstate` +0x82, `aurafilter` +0x50, `aurarangecalc` +0x64, `auralencalc` +0x60, `summon` +0xBC, `summode` +0xBF, `sumumod` +0xE4, `sumoverlay` +0xE6 | `fields.tsv` |

## Randomness

| Where | Seed | Draw |
|---|---|---|
| §3.1 | local S (T GUID + unit GUID) | `roll(b − a)` |
| §3.2 | unit | `roll_physical`, `roll_elemental`, then three `roll(b − a)` |
| §3.3 | unit | one step (`lo'` mod 5) before the scan |
| §3.9 | unit | `skill_result`, then `bonuses`, `roll_elemental` |
| §3.14 | unit | `roll(100)` for a minion |
| §3.17, §4.8 | unit | `roll_physical`, `roll_elemental` |
| §3.22 | unit | one step (count), spawn info (`roll(2)`, `roll(24)` ×2), then two steps per tentacle |
| §4.2, §4.7 | unit, then S | one unit step seeds S; S one step per missile; each missile's own seed in its callback (one + one per 15 points) |
| §4.4, §4.6 | unit, then S | one unit step seeds S; S two draws per try / missile |
| §4.9 | unit (re-seeded) | `roll(1000000)`, then two `roll(2r + 1)` per try |
| monster creation, class change | game / new unit / T | `monsters/init.md` |

## Edge cases & original bugs

1. The lightning fan and ring (§2.4, §2.5) step i by the progressive
   count: a count of 0 (after the `aurarangecalc` fallback) never ends
   the loop. Confirmed 2026-10-08 (impl-pc1-s5): the caller
   (`0x005D4F40`) passes `0x005D3DA0`'s count, or when that is 0
   `0x00646CA0(unit, aurarangecalc, skill, L)` unclamped; the loop keeps
   4·i (32-bit, wrapping) and tests it signed against 256
   (`0x005D4D9A`–`0x005D4DA2`), so a step ≤ 0 never ends it, and a
   negative step also reads the ring offsets from outside their
   64-entry tables. d2rs: a progressive step ≤ 0 stops the loop,
   creates no further missiles, and reports a fault (as
   `StatListError::EndlessExpiry`); the original hangs or faults.
2. MonCurseCast picks the curse from its own draw and pairs skill 91's
   state with the Decrepify stats (values from `Param7`, 0 in 1.14d, so
   nothing is applied) and skill 87's state with event handlers only
   (§3.3).
3. Suck Blood's do heals the minion owner, or the monster itself; a
   non-monster caster reads stats of no unit (§3.10).
4. Cry Help's command passes an uninitialised fifth field to the minions
   (§3.11).
5. Siege Beast Stomp's body requires an `srvmissilea` the 1.14d row lacks
   (§3.17): the stomp does nothing through this body.
6. fenris rage stacks a new state list per corpse (§3.21, §2.2).
7. Baal Tentacle passes an uninitialised class variable into the spawn
   info, which compares it with 570 (§3.22); implement as "not 570".
8. The lightning ring writes data +0x28 through a possibly null missile
   (§2.5).
9. Imp Inferno's do never turns state 12 off; it schedules an AI think at
   F + 13 instead (§3.7).
10. srvst 62 stores c without testing it; srvdo 135 spawns with it
    (§3.18, §3.19).

## Test vectors

| Case (1.14d data unless synthetic) | Expected |
|---|---|
| DeathMaul, f = 24, distance 5 / 20 (synthetic) | n = 5 + 12 = 17 / 40 |
| Baal Tentacle count, `lo'` = 7, Hell | 7 mod 3 + 2 + 2 = 5 |
| Royal Strike fn3 offsets, S draws 20 and 20 (synthetic) | a = b = 0 → a = 20: offset (20, 0) |
| Vine offsets, n = 6 | (0, 1), (0, −1), (1, 0), (−1, 0), (0, 1), (0, −1) |
| Imp release at life 9 % | imp killed, no AI change |
| MonCurseCast, `lo'` mod 5 = 1 | Weaken state, stat 25 = −50 per unit (scaled by `bodies.md` §2.10) |

## Provenance

- 1.14d disassembly of every listed address; data-only entries read with
  `at`: `0x005D1BF0`, `0x005D2B20`.
- Tables read from the image: `0x006E3228`, `0x005CE4DC`, `0x006E3254`,
  `0x006E3264`, `0x006E32AC`, `0x006E328C`, `0x00741B0C`, `0x00741B10`,
  `0x0063F5A0` / `0x0063F5BC` / `0x0063F630` / `0x0063F644` (spawn info
  jump tables, for the Baal case).
- Names of classes, states and stats from the 1.14d `patch_d2` tables
  (monstats `hcIdx`, states and itemstatcost row order).

## Open questions

1. Recording: Royal Strike / Claws of Thunder charges: missile paths
   (the zigzag points) and seeds per cast (§2.4, §2.5).
2. Recording: an Overseer whipping minions: transform rate and the new
   class (§3.14).
3. Recording: imps riding a barricade tower: state 143, the type-5 event
   and the release at low life (§2.3, §3.24, §4.10).
4. Answered: the spawn info `0x0063EFA0` is `monsters/ai-bodies-2.md`
   §13.1 (all keys, Baal `BaseId` 544 included; it names §3.22 as a
   caller), `0x0054DA60` is `monsters/population.md` §11.5 rule 3; both
   agree with §3.22.
5. Answered: `0x0056B9C0` (area-damage unit step) is specified in
   `missiles/missiles.md` (`area_damage` callback) and `0x0064CB30`
   (point collision) in `sim/path-placement.md` §4 (query table, rule
   5, masked value rule 2).
6. Answered (2026-10-08, `docs/handoff/impl-monster-skill-slots.md`): Imp Teleport's point
   when the target position "fails" (§3.24 step 3) is the pair that
   `0x0056D2C0` wrote anyway (target unit's position, else the path
   target), one or both coordinates 0; never a separate (0, 0) default.
   Read at `0x005D1B1A`–`0x005D1B3D`.
