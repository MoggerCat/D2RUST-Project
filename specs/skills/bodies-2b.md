# Spec: Skills — Start and do function bodies, batch 3, required levels 18–30 (§6–§8 of batch 3)

- **Status:** draft: every body and helper below read from the 1.14d
  `Game.exe` disassembly (`py tools/ghidra/disasm.py fn|at`; several
  entries have no Ghidra function, `use.md` Open question 1); D2MOO 1.10f
  `SkillAma.cpp`, `SkillSor.cpp`, `SkillNec.cpp`, `SkillPal.cpp`,
  `SkillBar.cpp`, `SkillDruid.cpp`, `SkillAss.cpp`, `MonsterMode.cpp`
  compared, differences noted. No recording covers these bodies yet
  (Open questions).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::skills::use_::bodies` (same module as `bodies.md` and `bodies-2.md`)
- **Related specs:** `skills/bodies-2.md` (owner of batch 3's conventions §1, helpers §2, bodies §3–§5, and of its Constants, Randomness, Edge cases, Test vectors, Provenance and Open questions; this file holds §6–§8 moved out of it unchanged, rule ids kept); `skills/bodies.md`; `skills/use.md`; `skills/functions.tsv`.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 24–30 |
| Rules | 31–32 |
|   6. Bodies, required level 18 | 33–335 |
|   7. Bodies, required level 24 | 336–636 |
|   8. Bodies, required level 30 | 637–895 |
<!-- /index -->

## Summary

§6–§8 of `skills/bodies-2.md`, split out of that file to keep it readable
(section numbers and rule ids are unchanged, so a reference names
this file and the same §). Status, evidence and open questions are
those of `skills/bodies-2.md`.

## Rules

### 6. Bodies, required level 18

#### 6.1 srvdo 11 Charged Strike `0x005DB850`

1. Unit flags |= 0x40. R invalid → 0. T none → 0.
2. `apply_melee(game, unit, T)` (the hit stored by srvst 6, `bodies.md`
   §7.2).
3. n = `eval(calc1)`. (tx, ty) = T's position; (ux, uy) = the unit's.
4. m = `prog_missile(unit, skill)`; invalid → 0.
5. Record (zeroed): flags 0x21, owner the unit, class m, position (tx,
   ty), target (2tx − ux, 2ty − uy), skill, L, init callback `bodies-2.md` §2.3. For i
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

PROVISIONAL: with no walk (a cast at a point) the path's target point of
step 3 is the point kept at the mode start (because the preview's cast
sends no path target); settled by REC-154.

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
5. `burst(game, unit, T, srvmissilea, skill, L, 0, 2, 0)` (`bodies-2.md` §2.16): 8
   missiles from T's position at the even offsets. Return 1.

#### 6.8 srvst 35 Vengeance `0x005CFE10`

1. R invalid → 0. E none → 0. T none → 0.
2. Zeroed record; result = `melee_result(game, unit, T, to_hit(unit,
   skill, L), 0)`.
3. Hit:
   1. Physical := `bonuses(unit, get 1, item none, 0, 0, 0, 0, SrcDam
      raw)` (`combat/damage.md` §3.2).
   2. Result |= `ResultFlags`; hit flags := `HitFlags` | 1.
   3. b = `base_roll(unit)` (`bodies-2.md` §2.20, one draw).
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
2. T = target. Aim (`bodies-2.md` §2.19) fails: T present and in melee range → Swing
   (`bodies-2.md` §2.15), return 1; else 0.
3. The unit's room none → 0. Pattern stamp `0x0064EA90(the unit's room,
   x, y, pattern, 0x80)`.
4. `set_uninterruptable(unit, 1)` (`bodies-2.md` §2.8).
5. E param 1 := x, param 2 := y; T → param 3 := T type, param 4 := T
   GUID; no T → param 3 := 6.
6. E flags := 0x1080. State 18 (`skill_move`) on.
7. T has state 54 → `0x00570420(game, T, 100)`, which acts only on a
   target **without** state 54 and so does nothing (Edge case 17).
   Return 1.

#### 6.12 srvdo 78 Leap Attack `0x005DA7E0`

1. E = the unit's entry of the skill; none → 0.
2. By E flags:
   - 0x100 (in flight): Land (`bodies-2.md` §2.13) succeeds: K = Pick (`bodies-2.md` §2.19); none →
     delete type-1 timers, type-1 timer at F + 4, return 0; K → animation
     from frame 16 (`0x00553DC0(game, unit, 16)`), return 1. Land fails
     → return 1.
   - 0x80: return Launch (`bodies-2.md` §2.13).
   - 0x200 (landed): return Strike (`bodies-2.md` §2.19) with (game, skill, L).
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
   L)` (`bodies-2.md` §2.18).
5. Second hit `0x005C7C20` (R in ECX, skill in EAX, unit in ESI; game,
   T, len, L): len < 10 → 10. Zeroed record; `skill_result(game, unit,
   T, R, skill, L, record, 0)` (`bodies-2.md` §2.17). Hit: hit flags |= `HitFlags`;
   `HitClass` ≠ 0 → hit class := it; enhanced damage % :=
   `eval(calc1)`; `roll_elemental`. `set_len(unit, record, len, skill)`
   (`bodies-2.md` §2.17). `start_combat(game, unit, T, record, 128)`;
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
2. n = `prog_count(unit, skill, L)` (`bodies-2.md` §2.11); n − 1 ≤ 0 → 0.
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
5. Hit: `kick_damage(game, record, T, skill, L)` (`bodies-2.md` §2.5);
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
   unit, 0, 0, r, 3, −1, &count)` (`bodies-2.md` §2.21); T present → count =
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
   (`bodies-2.md` §1; `0x004F4110`, skills +0x15C). Return 1.

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
   L, 0, 0, 0)` (`bodies-2.md` §2.21). E param 1 := K's GUID.
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
5. T: alignment := 1; target list 9 (`bodies-2.md` §2.22); leave pack (`bodies-2.md` §2.22).
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
off; target list remove (`bodies-2.md` §2.22).

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
0)`; umod 15 on m; target list 9 (`bodies-2.md` §2.22). Return m.

#### 7.11 srvst 20 Iron Golem `0x005C32A0`

Return 1 when T is an item (type 4) in mode 3 (on the ground), its item
class has `bitfield1` bit 1 (items +0xDC, `0x00629CC0`: the metal
flag; 1.14d `armor.txt` `bitfield1` is 3 on 116 rows, chain / plate /
metal helms, and 1 on 86 leather and cloth rows), item flag 0x10
(identified) is set (`0x006280A0`, `items/generation.md` §1.4), and the item is not
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
   `0x00623830`, `0x0064C370`; `items/inventory-moves.md` §8); L = T's first
   allowed body location (`0x0062EA80`); T mode := 4 (`0x00624690`);
   equip T on m at L (`0x005606B0(game, m, T GUID, L, skip 1, out none)`,
   `items/inventory.md` §4.6).
6. `golem_stats(game, unit, m, skill, L)` (`bodies.md` §6.11); message
   0x7F about m to the unit's client (`0x005531C0`, `0x0053CDF0`, as
   `bodies.md` §8.9 step 7); `node_insert(game, m, 0, unit +0xD0)`.
   Return 1.

Second caller, game join (added 2026-10-07): when a loaded save carried
an Iron Golem item, the join step `0x005394A0` (`0x005396C8`–
`0x0053974C`) finds that item unit, makes it the player's target
(`0x00620C10`) at the player's position, and calls the do core
`0x0056F7F0(game, player, 90, L, charge 1, item 0, aim 0)` directly
(`use.md` §5.4; no start function, no `schedule_periodic`), L = the
player's skill-90 level with bonuses (`0x006442A0(player, entry, 1)`).
This body then runs as above with T = the saved item; step 1 re-tests
§7.11 on it. Iron Golem has a start function and no `usemanaondo` or
`delay` in 1.14d `skills.txt`, so the join cast spends no mana and sets
no cooldown (`use.md` §5.4 step 9). Which item, and the client field
that holds its GUID: `formats/d2s-load.md` §3 (PC 2).

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
5. Conversion (`bodies-2.md` §2.23) of T by the unit, state `auratargetstate`, expiry
   e, callback `0x005D01A0`. Return 1 (the stored hit is not applied).

#### 7.14 srvst 36 Holy Shield `0x005D0180`

Return 1 when the unit has an inventory and a shield equipped
(`0x0063C8F0(inventory, null)`, `combat/hit.md`), else 0.

#### 7.15 srvdo 9 Frenzy `0x005D8E00`

1. T none → 0.
2. Frame event index even: `apply_melee(game, unit, T)`; free the unit's
   combat records for T (`0x0057C9F0`, `combat/damage.md` §5.1);
   attack-mode cleanup (`0x00580310`, `0x00580380`); Charge (`bodies-2.md` §2.24);
   return Swing (`bodies-2.md` §2.24) on T.
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
   (`bodies-2.md` §2.17).
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
2. r = `prog_count(unit, skill, L)` (`bodies-2.md` §2.11); 0 → r =
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
(`bodies-2.md` §2.23) of U by the source, state 53 (`conversion`), expiry e, callback
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
   ≤ 0); `kick_damage(game, record, T, skill, L)` (`bodies-2.md` §2.5).
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
   := `eval(calc1)`. `EType` conversion as `bodies-2.md` §3.1 (no `roll_elemental`).
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
      1)`). Leave pack (`bodies-2.md` §2.22). Delete T's type-8 timers. T has a right
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
   `udead`-group state, monstats2 `corpseSel` (as `bodies-2.md` §5.6); else 0.
2. p = `eval(source, calc1)`; one draw on the **source's** seed,
   `roll(100)` ≥ p → 0.
3. Source life := min(life + `eval(calc2)` << 8, maximum life); mana :=
   min(mana + `eval(calc3)` << 8, maximum mana (`0x00625D60`)).
4. U state 99 (`redeemed`) on; U flags (+0xC4) &= ~6; C.count += 1.
   Return 1.

Unlike srvdo 66 (`bodies.md` Edge case 9) the count is kept: a player
pays the mana cost for every run that redeems something.

**Per-corpse effect** `0x005D0C40` (D2MOO
`SKILLS_ApplyRedemptionEffect`): `redeem(game, S, U, skill, L, last)`
(ECX game, unused; EDX source S; `ret 0x10`). The only callers are the
radament-death missile callbacks `0x005AD8F0` (last 0) and `0x005AD910`
(last 1) (`missiles/bodies-2.md` §44), so S is that missile and skill
is 124:

1. skill not in 0…skills count − 1, U none, or `0x006456D0(U)` fails
   (the corpse test of callback step 1) → 0.
2. last = 0: p = `eval(S, calc1)` (skills +0x138); one draw `roll(100)`
   on **S's** seed (S + 0x20, `0x0045C390`); draw ≥ p → 0. last ≠ 0: no
   draw, the corpse is always taken.
3. S life (stat 6) := min(life + `eval(S, calc2)` << 8, maximum life
   `0x00625D10`); S mana (stat 8) := min(mana + `eval(S, calc3)` << 8,
   maximum mana `0x00625D60`) (`0x00625480` / `0x00627260`).
4. U state 99 (`redeemed`) on (`0x00639DB0`); U flags (+0xC4) &= ~6.
   Return 1.

Steps 2–4 are callback steps 2–4 with the caster replaced by S and the
roll skipped on the last frame; no count and no mana cost. The heal of
step 3 goes to the missile, not to Radament.

#### 8.10 srvst 38 Whirlwind `0x005D8F50`

1. R invalid → 0. E = the unit's entry of the skill; none, or the target
   position (x, y) fails → landing message `0x00571B70(unit, skill)`,
   return 0.
2. The unit has state 12 → 0.
3. T present and in melee range → landing message; return Swing (`bodies-2.md` §2.15).
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
2. (x, y) = E params 1, 2. Either 0 → End (`bodies-2.md` §2.25); return 0.
3. E flags & 3 = 3 (moving and arrived) → End; return 1.
4. E flags & 1 and the unit alive:
   1. A non-player: frame event index := 0, frame count (+0x48) :=
      0x400. A player: animation from frame 3 (`0x00553DC0(game, unit,
      3)`).
   2. n = Pacing (`bodies-2.md` §2.25).
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
   `eval(calc1)`; `EType` conversion as `bodies-2.md` §3.1; `roll_elemental`.
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
(`bodies-2.md` §2.26) with (game, unit, skill, L). Return 1.
