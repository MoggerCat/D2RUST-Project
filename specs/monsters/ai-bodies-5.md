# Spec: Monsters — AI think bodies, Act V

- **Status:** draft: the 22 AI functions used by Act V monsters that
  were still unread, with their init and alternate functions, the Baal
  target pick, score, choice and execution, and the scan callbacks they
  use, read from the
  1.14d `Game.exe` disassembly (addresses per section; register
  arguments checked in `all.asm`). No recording covers these bodies yet
  (Open question 1).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::monsters::ai` (per-AI functions beside the
  `ai-bodies.md` §9 bodies)
- **Related specs:** `monsters/ai.md` (owner of scheduling §1, dispatch
  §2, AI control and tables §3, aip reads §4, target search §5,
  distances §6, tactics helpers §7, commands §8, the conventions of §9);
  `monsters/ai-bodies-3.md` (conventions reused here),
  `monsters/ai-bodies-4.md` (boss pick §7.1); `monsters/ai-functions.tsv`
  (catalogue); `sim/stat-lists.md` (stat lists, states);
  `skills/levels.md` (skill entries and levels); `world/quests.md`
  (quest seams); `drlg/rooms.md` (room unit lists); skills spec (skill
  checks, skill bodies).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 61–83 |
| Inputs | 84–93 |
| Outputs / state changes | 94–100 |
| Rules | 101–102 |
|   1. Scope and order | 103–139 |
|   2. Minion (116) `0x005E1B60` | 140–156 |
|   3. Imp (122) `0x005E2FF0`, init `0x005E2FD0` | 157–189 |
|   4. Succubus (118) `0x005E1E00` | 190–224 |
|   5. BloodLord (125) `0x005E36F0` | 225–235 |
|   6. SuccubusWitch (119) `0x005E2120` | 236–260 |
|   7. Overseer (120) `0x005E27A0` | 261–296 |
|   8. ReanimatedHorde (114) `0x005E1540` | 297–314 |
|   9. ClawViperEx (142) `0x005F1DE0` | 315–332 |
|   10. DeathMauler (130) `0x005EE260` | 333–343 |
|   11. PutridDefiler (137) `0x005EFA90` | 344–368 |
|   12. Ancient (133) `0x005EF1A0` | 369–423 |
|   13. AncientStatue (132) `0x005EEAA0` | 424–431 |
|   14. FrozenHorror (124) `0x005E3530` | 432–446 |
|   15. SiegeBeast (115) `0x005E1900` | 447–474 |
|   16. SuicideMinion (117) `0x005E1D30` | 475–486 |
|   17. BaalMinion (141) `0x005EF910` | 487–498 |
|   18. BaalTaunt (136) `0x005EF710` | 499–515 |
|   19. BaalToStairs (138) `0x005EF620` | 516–531 |
|   20. BaalThrone (134) `0x005EF320`, init `0x005EF310` | 532–576 |
|   21. BaalCrab (135) `0x005FCFE0`, alternate `0x005FCF30` | 577–688 |
|   22. BaalCrabClone (140) `0x005FD210`, alternate `0x005FCF30` | 689–705 |
|   23. Nihlathak (128) `0x005EE5D0`, init `0x005EE5C0`, alternate `0x005E5280` | 706–743 |
| Constants & data dependencies | 744–762 |
| Randomness | 763–771 |
| Edge cases & original bugs | 772–786 |
| Test vectors | 787–801 |
| Provenance | 802–827 |
| Open questions | 828–866 |
<!-- /index -->

## Summary

Act V levels (levels.txt `Act` = 4), the expansion superuniques
(superuniques.txt rows 42–65) and the bosses Baal (throne, crab, clone,
stairs form) and the Ancients' statues, plus minions and spawns, give
200 monstats rows (Provenance). The unread AIs among them are written
here in descending order of rows (§1). Conventions are those of
`ai-bodies-3.md` Summary (T, D, C, P(aipN), "draw" = `lo' % 100`, idle,
wait, A1 / A2 / S1 / S2 / S3 = mode 4 / 5 / 8 / 9 / 10 at a unit,
"Skill k at U" = `0x005DEAD0(Skk mode, Skill k, U, 0, 0)`, L = own life
percent, frame compares signed). Two more helpers:

- "walk to U" = `0x005DECA0(U)`: path step count 1, mode 2 toward U (the
  point (0, 0) when U = 0), requested without failure handling (D2MOO
  `AITACTICS_WalkToTargetUnit`).
- "teleport in range r" = `0x005DF850(unit, r, skill, mode)`: n := 2r;
  x := own x + d − r, then y := own y + d' − r, where each d is one step
  `lo' & (n − 1)` when n is a power of two, else `lo' % n` (unsigned),
  and 0 without a draw when n < 1; then `0x005DEAD0(mode, skill, 0, x,
  y)`.

Bracketed values are the Normal values of the named live row.

## Inputs

| Name | Type | Source |
|---|---|---|
| tick record | `ai.md` §2.1 | control, T, D, C, monstats |
| AI control | `ai.md` §3.1 | params 0–2, commands |
| monstats | 424 bytes | `aip1`..`aip8`, `Skill1`..`Skill8`, `Sk*mode`, `BaseId`; fixed rows 492–495 (Imp) |
| skills | 0x23C bytes | `aurastate` (+0x80), `auratargetstate` (+0x82), `Param5` (+0x158) |
| game | | +0x6D difficulty, +0xA8 frame, data tables +0xC4 (states count) |

## Outputs / state changes

Mode requests, think schedules, AI control params (own and other
monsters'), velocity requests, unit flags (ReanimatedHorde), states
12, 90, 91, 142, 146, stat lists (BaalThrone), unit removal
(BaalToStairs), S→C message 0xA4 (BaalThrone), unit-seed draws.

## Rules

### 1. Scope and order

Rows counted by the script of `ai-bodies-3.md` (Act 4 levels,
superuniques 42–65, bosses baalthrone, baalcrab, baalclone,
baalcrabstairs, ancientstatue1–3, closed over `minion1`, `minion2`,
`spawn`):

| AI (index) | Act V rows | § |
|---|---|---|
| Minion (116) | 10 | §2 |
| Imp (122), init `0x005E2FD0` | 8 | §3 |
| Succubus (118) | 8 | §4 |
| BloodLord (125) | 7 | §5 |
| SuccubusWitch (119) | 7 | §6 |
| Overseer (120) | 6 | §7 |
| ReanimatedHorde (114) | 6 | §8 |
| ClawViperEx (142) | 5 | §9 |
| DeathMauler (130) | 5 | §10 |
| PutridDefiler (137) | 4 | §11 |
| Ancient (133) | 3 | §12 |
| AncientStatue (132) | 3 | §13 |
| FrozenHorror (124) | 3 | §14 |
| SiegeBeast (115) | 3 | §15 |
| SuicideMinion (117) | 1 | §16 |
| BaalMinion (141) | 1 | §17 |
| BaalTaunt (136) | 1 | §18 |
| BaalToStairs (138) | 1 | §19 |
| BaalThrone (134), init `0x005EF310` | 1 | §20 |
| BaalCrab (135), BaalCrabClone (140), alternate `0x005FCF30` | 1 each | §21, §22 |
| Nihlathak (128), init `0x005EE5C0`, alternate `0x005E5280` | 1 | §23 |

Ties are listed by index. 179 of the 200 rows have `switchai`
(`ai-bodies-2.md` §16). SuicideMinion's think is also the
special-state think 15 (`ai.md` §3.2); BaalThrone re-installs state 0
when it becomes the stairs Baal. No other body here installs a special
state.

### 2. Minion (116) `0x005E1B60`

Brackets: minion1 [70, 15, 50, 15]. U := T, F := 0 ("ordered").

1. Current command K (`0x0058EE80`): K type 1 and frame < K param 3:
   V := `0x00552F60(game, K param 1 (type), K param 2 (GUID))`; V exists
   and is not dead (`0x005541B0`) → F := 1; V ≠ T → U := V, C := melee
   range unit→V (`0x00622C40`) (a distance `0x006416D0` is also
   computed and discarded). Any other K (or a failed test) → free the
   current command (`0x0058ED10`).
2. Not C: F = 0 and draw ≥ aip3 [50] → idle aip4 [15]; else walk to U
   with flags 0 (`0x005DEC80`). End.
3. C: F = 0 and draw ≥ aip1 [70] → idle aip2 [15]. End. Draw < aip2 →
   A2 at U; else A1 at U.

aip2 is both the stall and the A2 chance. 1.14d-confirmed.

### 3. Imp (122) `0x005E2FF0`, init `0x005E2FD0`

The AI reads its aips from the fixed monstats rows I1..I4 = 492..495
(imp1..imp4) of the difficulty, whatever the unit's own row (a missing
row is read through a null pointer). Normal values: I1 [25, 14, 10],
I2 [5], I3 [10, 40, 22, 25], I4 [–, –, 13, 60]. AI param 0 = mount
GUID m (the siege beast it rides to, set by §15). `Skill1` Imp
Teleport, `Skill4` Imp Fire Missile.

0. Init: m := −1.
1. m ≠ −1 and own alignment 0 (`0x006259B0`): B := `0x00552F60(game,
   1, m)`. B missing, dead, with an owner (`0x00552FD0`) or alignment ≠
   0 → m := −1. Else squared distance unit→B (`0x005B0BD0`) > I2.aip1²
   [25] → walk to B with flags 0, end; else `Skill1` ≥ 0 → `Skill1` at
   B, end.
2. C: `Skill1` ≥ 0 and L (`0x005DD280`) < I1.aip1 [25] → teleport in
   range I1.aip2 [14] with `Skill1` / `Sk1mode`; end. Draw < I3.aip2
   [40] and escape from T by 5 with think delete started → end.
3. `Skill1` ≥ 0 and draw < I1.aip3 [10] → teleport in range I1.aip2;
   end.
4. D < I3.aip1 [10] and draw < I3.aip2 → escape from T by 5 without
   delete; end.
5. `Skill4` ≥ 0 and S, E := secondary target and distance
   (`0x005DDC30`, second argument 0): E < I3.aip3 [22] → draw < I3.aip4
   [25] → `Skill4` at S, end (a failed draw goes to step 6). E ≥
   I3.aip3 → E < I4.aip3 [13] and draw < I4.aip4 [60] → `Skill4` at S,
   end. (Live I4.aip3 13 < I3.aip3 22 on every difficulty: the second
   branch never fires.)
6. Draw < 33 → walk to T with 4 steps (`0x005DEF80`); else draw < 20 →
   wander 8; else idle 10.

1.14d-confirmed; same as D2MOO.

### 4. Succubus (118) `0x005E1E00`

Brackets: succubus1 [90, 50, 50, 15, 15, 15, 3, 0]; `Skill1` Defense
Curse, `Skill2` Blood Mana, `Skill3` SuccubusBolt. Skill tests here are
**> 0**. "Cursed" = T has an active stat list with list flag 0x20
(`0x00625760(T, 0x20)`, `sim/stat-lists.md` §9; D2MOO `STATLIST_CURSE`).
`0x00625760(unit, f)` answers 0 unless the unit's own list is extended
(bit 0x80000000); then `0x006256E0` returns the first list of the active
chain whose flags share a bit with f. The skill-3 test reads max life
(`0x00625D10`, stat 7) and max mana (`0x00625D60`, stat 9) of T: cast when
max mana < max life, T is absent or T is not a player. Both reads are
real stat reads in the wiring (a stub of 0 / false made a monster cast a
curse again on an already cursed player; gen-su-54, first difference
frame 41 -> 68).

1. T not cursed, D < aip4 [15] and P(aip3) [50]:
   1. `Skill1` > 0 and T's life percent ≥ aip7 [3] → `Skill1` at T. End.
   2. `Skill2` > 0 and L ≤ aip8 [0] → `Skill2` at T. End.
   3. `Skill3` > 0 and (T's max mana `0x00625D60` < its max life
      `0x00625D10`, or T is not a player) → `Skill3` at T. End.
   4. `Skill4` > 0 and T is a player → `Skill4` at T. End.
2. C: P(aip1) [90] → A1 at T; else idle aip5 [15]. End.
3. `Skill5` > 0, aip8 > 0, a secondary target S (`0x005DDC30`) and
   draw < aip8 → `Skill5` at S. End.
4. P(aip2) [50] → walk to T with flags 0; else idle aip6 [15].

1.14d-confirmed; same as D2MOO.

"Cursed" is live state: a Skill1 cast sets a curse list (flag 0x20) on T,
so the next think of the same Succubus skips step 1 **before** its
P(aip3) draw (gen-mon-469 frame 46 cast, frame 69 only the step-4 draw
at `0x005E20AC`). The test is `0x00625760` (T's list must be extended)
then the active chain of `0x006256E0`; d2rs reads it from the stat
lists (`wiring/action/ai.rs`), as it reads T's max mana (REC-2240).

### 5. BloodLord (125) `0x005E36F0`

Brackets: bloodlord1 [90, 85, 50, 10]; `Skill1` BloodLordFrenzy.

1. Not C: P(aip2) [85] → walk to T; else idle aip4 [10]. End.
2. C: P(aip1) [90] → draw < aip3 [50] → `0x005DEAD0(5, Skill1, T, 0,
   0)` (mode 5 fixed, `Skill1` not tested); else A1 at T. Else idle
   aip4.

1.14d-confirmed.

### 6. SuccubusWitch (119) `0x005E2120`

Brackets: succubuswitch2 [90, 25, 0, 0, 90, 15, 80, 66]; `Skill1`
Amplify Damage, `Skill2` Weaken, `Skill3` Defense Curse, `Skill4` Blood
Mana, `Skill5` SuccubusBolt. E := D. "Escape" = escape from T by aip4
(as a byte) with think delete.

1. T ≠ 0, T not cursed (§4), D < aip4 [0] and P(aip3) [0]: the four
   curse tests of §4 step 1 (`Skill1`..`Skill4` > 0; aip7 [80] and aip8
   [66]), except that step 1.3 has no T = 0 test. End on a cast.
2. C: draw ≥ aip3, or the escape does not start → draw < aip1 [90] →
   A1 at T, else idle aip6 [15]. End. (Escape started → end.)
3. Not C:
   1. `Skill5` ≥ 0, aip8 > 0, draw < aip5 [90], S, E := secondary
      target (searched only after the draw) and draw < aip8 → `Skill5`
      at S. End.
   2. E < aip4: draw < aip3 and the escape starts → end.
   3. `Skill5` < 0, S, E := secondary target, S found and draw < aip5
      → S2 (mode 9) at S. End.
   4. Draw < aip2 [25] → walk to T with flags 0. End. Draw < 50 and
      circle 6 at T with delete (`0x005DF7D0(T, 6, 1)`) started → end.
      Else idle aip6.

1.14d-confirmed.

### 7. Overseer (120) `0x005E27A0`

Brackets: overseer1 [250, 50, 50, 17, 7, 100, 50]; `Skill1` Cry Help,
`Skill2` Healing Vortex, `Skill3` Overseer Whip, `Skill4` Smite;
minion minion1. AI param 0 = cry cooldown frame. "Has k" =
`0x005DD2D0(unit, Skill k)`: the unit's class lists that skill among
`Skill1`..`Skill8`.

1. Quest seam `0x00587900(game, unit)` (acts only when the unit is
   superunique 42, Shenk; `world/quests.md`).
2. V := the unit's last attacker (`0x005DE830`: the last-attacker
   lookup `0x00553010`, `skills/bodies-2.md` §2.8; kept when it is a
   player not in mode 0 or 17, or a monster not in mode 0 or 12 that is
   hostile `0x00554200(game, unit, V)`; else 0).
3. `Skill1` ≥ 0, AI state 3/19, frame > param 0, V and has 1 →
   `Skill1` at V; param 0 := frame + aip1 [250]. End.
4. C: draw < aip6 [100] → draw ≥ aip7 [50] → A1 at T, else A2 at T.
   End.
5. Minion scan: scan 1, callback `0x005E26D0`, arg {576, 400, 50, hurt
   0, whip 0, n 0}. U counts when it is a monster other than the
   scanner with `BaseId` 453 (minion1), not in mode 0 or 12, not
   hostile (`0x00554200(game, scanner, U)` = 0) and without state 141
   (`bloodlust`): n += 1; no hurt yet, squared distance ≤ 576 and life
   percent < 50 → hurt := U; no whip yet, squared distance ≤ 400 and U
   not unique (`0x005A0180(U, 8)` = 0) → whip := U.
6. `Skill2` ≥ 0, hurt, draw < aip2 [50] and has 2 → `Skill2` at hurt.
   End.
7. `Skill3` ≠ 0, whip, draw < aip3 [50], own alignment ≠ 2 and has 3 →
   `Skill3` at whip. End.
8. n = 0: draw ≥ 60 → idle 10; else walk to T with flags 0. End.
9. D < aip4 − aip5 [10] → escape from T by aip4 − D (as a byte)
   without delete. End. D ≤ aip4 + aip5 [24] → draw < 50 → circle 5 at
   T (no delete); else idle 10. End. Walk to T with flags 0.

1.14d-confirmed.

### 8. ReanimatedHorde (114) `0x005E1540`

Brackets: reanimatedhorde1 [30, 20, 12, 20, 20, 65, 25]; `Skill2`
Charge (`Skill1` Self-resurrect is not used here).

1. Unit flags +0xC4 |= 0x0E (every think).
2. C: P(aip1) [30] → A1 at T; else idle aip2 [20]. End.
3. `Skill2` ≥ 0, direct line to T (`0x005DC640(unit, T)`, `ai.md` §6),
   D < aip3 [12], 5 < D (tested in this order) and U `roll(100)` (the
   helper, `0x005E161A`; the other draws of this function are inline
   steps) < aip4 [20] → `Skill2` at T. End. Recorded
   (`gen-mon-436`, frame 31: D 7, direct line clear, roll 42 then the
   two step-4 draws, idle).
4. Draw < aip5 [20] → walk to T with flags 0. End. Draw < aip6 [65] →
   walk in radius of T (`0x005DE6D0(T, 4, 0)`). End. Idle aip7 [25].

1.14d-confirmed.

### 9. ClawViperEx (142) `0x005F1DE0`

Brackets: clawviper6 [60, 10, 90, 85, 5, 1, 18, 50]; `Skill1` Charge
(`seq_serpentcharge`). AI params: 0 = has charged, 1 = A1 cooldown
frame. G = aip6 as in ClawViper (`ai-bodies-2.md` §10): 2 → state 91,
other nonzero → 90.

1. Param 0 ≠ 0 and G ≠ 0 → state off.
2. C: P(aip3) [90] → A2 at T; else idle aip5 [5]. End.
3. D < aip7 [18] and P(aip4) [85] → frame > param 1 → A1 at T, param 1
   := frame + aip8 [50]; else idle aip5. End.
4. `Skill1` ≥ 0, D < aip2 [10], P(aip1) [60] and the skill check
   `0x005FD470(Skill1, T, T.x, T.y)` ≠ 0 → `Skill1` at T; G ≠ 0 →
   state on; param 0 := 1. End.
5. Draw < 50 → walk to T with flags 7; else idle aip5.

1.14d-confirmed.

### 10. DeathMauler (130) `0x005EE260`

Brackets: deathmauler1 [75, 65, 15, 50]; `Skill1` DeathMaul.

1. C: P(aip1) [75] → A1 at T; else idle 15. End.
2. `Skill1` ≥ 0, D < aip3 [15] and draw < aip4 [50] → `Skill1` at T.
   End.
3. Draw < aip2 [65] → walk to T with flags 0; else idle 15.

1.14d-confirmed.

### 11. PutridDefiler (137) `0x005EFA90`

Brackets: putriddefiler1 [15, 5]; skill 300 Impregnate.

1. T ≠ 0 and C → A1 at T. End.
2. Host scan: scan 1, callback `0x005EFA50`, arg {0, 25}; the scan
   returns the first unit U for which the Impregnate target test
   `0x005D2140(scanner, U)` holds (a living monster whose `BaseId` is
   not 546 putriddefiler1 or 551 painworm1, without state 110
   `pregnant`, alignment ≠ 2) and the full-size distance scanner→U ≤
   25.
3. No host: D < aip1 [15] → escape from T by aip2 (byte) [5] without
   delete; else idle 25. End.
4. Host H in melee range → `0x005DEAD0(8, 300, H, 0, 0)` (mode and
   skill fixed). Else walk to H.

1.14d-confirmed.

Implemented (rc-mon-frame31, REC-1997): putriddefiler1 has monstats
`boss` = 1, so the precheck C boss sound (`ai.md` §2.4 rule 1, boss test
`0x0063E9F0` = monstats byte +0x0C & 0x40) fires on its first think (a
player within 20): sound 16, flag 0x10, idle 20; the escape starts 20
frames later. gen-mon-546..549: EQUAL.


### 12. Ancient (133) `0x005EF1A0`

Target mode 2 (T may be 0). The function dispatches on the unit's exact
class: 540 (ancientbarb1) → A, 541 → B, 542 → K; other classes do
nothing (no think scheduled). "Gate" = the quest test `0x0058CF90(game)`
(D2MOO `ACT5Q5_IsNotActivatable`, `world/quests.md`). "Pick" = the boss
pick of `ai-bodies-4.md` §7.1 with no command and the cull "same level
as the unit" (`0x005EEB10`: level ids of both rooms equal). X := the
target used; m := melee range unit→X; d := `0x006416D0(unit, X)`
(`missiles/missiles.md`).

**A `0x005EEB70`** (Talic; brackets ancientbarb1 [15, 25, 75, 8];
`Skill1` Whirlwind):

1. Gate, or the unit has state 54 → idle 25. End.
2. X := T; T = 0 → X := pick; none → idle 25. End.
3. `Skill1` > 0, has `Skill1` (`0x005DD2D0`), d < aip1 [15] and draw <
   aip2 [25] → whirlwind: (tx, ty) := X's position; s :=
   `0x006417F0(unit, tx, ty)` (0 → 1); offset := (aip4 [8] × (tx − own
   x) / s, aip4 × (ty − own y) / s) (signed); mode request built for
   `Sk1mode` (`0x005A7E60`), current skill := the unit's highest
   `Skill1` entry (`0x006439F0` → `0x00620210`), path step count 1,
   request point (tx + ox, ty + oy), request byte +0x15 := 100 (path
   type "no path", `ai.md` §7.1 mode request record),
   requested with flag 0 (`0x005A7C20(game, req, 0)`). End.
4. Not m → walk to X; wait 10. End.
5. Draw < aip3 [75] → `roll(2)` ≠ 0 → A1 at X, else A2 at X. Else idle
   25.

**B `0x005EEDA0`** (Madawc; brackets ancientbarb2 [15, 50, 25, 25, 7];
`Skill1` Shout):

1. Gate → idle 25. End. X := pick; none → idle 25. End.
2. m: draw < aip4 [25] and escape from X by aip5 (byte) [7] with think
   delete started → wait 25. End.
3. `Skill1` > 0, its skills row exists, (`aurastate` < 1 or > the
   states count or the unit lacks it), (`auratargetstate` < 1 or > the
   count or X lacks it), draw < aip3 [25] and `Skill1` with no target
   succeeds → end.
4. d < aip1 [15]: draw < aip2 [50] and A1 at X succeeds → end.
5. d < aip1, or walk to X with d − aip1 steps (byte, `0x005DEF80`)
   does not start → idle 10.

**K `0x005EF010`** (Korlic; brackets ancientbarb3 [15, 50, 75];
`Skill1` Leap Attack):

1. Gate → idle 25. End. X := pick; none → idle 25. End.
2. `Skill1` > 0, d < aip1 [15], draw < aip2 [50] and has `Skill1` →
   sequence skill `Skill1` at X (`0x005DE000`). End.
3. Not m → walk to X; wait 10. End.
4. Draw < aip3 [75] → `roll(2)` ≠ 0 → A1, else A2, at X. Else idle 25.

1.14d-confirmed. D2MOO differs in B step 5 (it also walks when d <
aip1, with a negative step count).

### 13. AncientStatue (132) `0x005EEAA0`

Target mode 0. A5Q5 activatable (`0x0058CFE0(game)` ≠ 0, D2MOO
`ACT5Q5_IsActivatable`), no portal yet (`0x0058CFB0(game)` = 0) and the
unit lacks state 146 (`invis`) → clear the room's portal flag
(`0x0061AED0(room, 0)`); `0x005DEAD0(4, 302 MinionSpawner, 0, own x,
own y)`. Otherwise idle 25. 1.14d-confirmed.

### 14. FrozenHorror (124) `0x005E3530`

Brackets: frozenhorror1 [60, 40, 60, 10]; `Skill1` Horror Arctic Blast.
R := level (with bonus) of the unit's highest `Skill1` entry
(`0x006439F0`, `0x006442A0`), 0 when none; negative → 0.

1. `Skill1` ≥ 0, D < R, P(aip3) [60] and the unit lacks state 12 →
   `Skill1` at T. End.
2. State 12 set → off.
3. Not C: P(aip2) [40] → walk to T, end. C: P(aip1) [60] → A1 at T,
   end.
4. Idle aip4 [10].

1.14d-confirmed.

### 15. SiegeBeast (115) `0x005E1900`

Brackets: siegebeast1 [25, 50, 1, 15, 1, 50, 100]; `Skill1` Siege Beast
Stomp, `Skill2` Charge (not used here); minion imp1. "Stomp" =
`0x005DEAD0(Sk1mode, Skill1, 0, 0, 0)`.

1. The unit in mode 0 or 12 (`0x0063EA40`) → return, nothing scheduled.
2. No owner (`0x00552FD0`) and own alignment 0: rider scan, scan 1,
   callback `0x005E1720`, arg {aip1² [625]}: the scan returns the first
   U that is a monster other than the scanner, not in mode 0 or 12,
   with `BaseId` 492 (imp1), the scanner's alignment pairing, alignment
   0, no owner, its AI param 0 = −1, and squared distance ≤ 625. Found
   → assign (`0x005E17E0(U, beast)`): U's current beast (its param 0)
   still exists (`0x00552F60(game, 1, ·)`) and is strictly nearer U
   than this beast by squared distance (`0x005B0BD0(U, old) <
   0x005B0BD0(U, this)`) → keep; else (ties included) U's param 0 :=
   this beast's GUID.
3. C: `Skill1` ≥ 0 and draw < aip3 [1] → stomp. End. Draw < aip2 [50]
   → A1 at T; else idle aip4 [15]. End.
4. `Skill1` ≥ 0, D < `Param5` of the `Skill1` skills row
   (`0x004E6C70`) and draw < aip5 [1] → stomp. End.
5. Direct line to T (`0x005DC640`) and draw < aip6 [50] → velocity
   request (0, aip7 clamped to 0..127 [100], 0); walk in radius of T
   (`0x005DE6D0(T, 12, 0)`). End.
6. Walk in radius of T (12, 0).

1.14d-confirmed.

### 16. SuicideMinion (117) `0x005E1D30`

Also the special-state 15 think. Brackets: suicideminion6 [15, 5, 50,
4, 0]. AI param 0 = detonation frame f.

1. f ≠ 0 and f < frame → mode 0 (death) at T (`0x005DDF90(0, T)`). End.
2. f ≠ 0 (not yet) → idle aip2 [5]. End.
3. f = 0: C → f := frame + aip5 [0]; idle aip2. Not C: draw < aip3 [50]
   → walk to T with flags 0; else idle aip2.

1.14d-confirmed.

### 17. BaalMinion (141) `0x005EF910`

Brackets: baalminion1 [90, 85, 50, 17]; `Skill1` Smite (`A2`).

1. T = 0 or not C: draw < aip2 [85] → walk to T.
2. C: draw < aip1 [90] → (`Skill1` ≥ 0 and draw < aip3 [50] → `Skill1`
   at T; else A1 at T); else idle aip3.
3. Always then: wait aip4 [17] (it replaces any think the steps
   scheduled).

1.14d-confirmed.

### 18. BaalTaunt (136) `0x005EF710`

Brackets: baaltaunt [3, 10, 20]; skill 284 Baal Taunt. AI param 0 =
counter n.

1. T = 0 → idle 25. End.
2. T is a player or monster in anim mode 1: n += 1; n > aip2 [10] → n
   := 0, `0x005DEAD0(4, 284, T, 0, 0)`, end. Otherwise (T in another
   mode) n := 0.
3. D > aip3 [20] and placing the unit at T's position succeeds
   (`0x00554EA0(game, unit, T's room, T.x, T.y, 0, 0)`,
   `sim/path-placement.md` §10) → idle 25. End.
4. D > aip1 [3] → walk to T. End.
5. Idle 25.

1.14d-confirmed.

### 19. BaalToStairs (138) `0x005EF620`

Brackets: baalcrabstairs [4].

1. Scan 1, callback `0x005EF5E0`, arg {0, 25}: the first object (type
   2) of class 563 (The Worldstone Chamber portal) with
   `0x006416D0(object, unit)` ≤ 25. None → idle 25. End.
2. `0x006416D0(unit, O)` < aip1 [4]: quest seam `0x0058E600(game)`;
   state 146 (`invis`) on; stop the path (`0x00648730`); delete thinks;
   the unit leaves its room (`0x0061A270(room, type, GUID)`,
   `0x00623830`, `0x0064C370`, `drlg/rooms.md`); remove it
   (`0x00555600(game, unit)`). End.
3. Velocity request (method 1 → 7, 0, 0); walk to O.

1.14d-confirmed.

### 20. BaalThrone (134) `0x005EF320`, init `0x005EF310`

Target mode 2. `Skill1` Decrepify. AI params: 0 = wave w, 1 = flags (1
wave cleared and corpse explosion done, 2 wave spawned), 2 = next frame
q. Waves are superuniques 61..65 (Baal Subject 1..5, table
`0x006E3528`), each mapped through `0x00586B30` and `0x00659B80(2, ·)`
(D2MOO `DATATBLS_MapSuperUniqueId`). The init does nothing.

1. No monstats row → idle 10. End.
2. frame < q → idle frame − q (a negative delay, Edge cases 4). End.
3. Ally scan: scan 3 (`ai.md` §5.4, callback `0x005DC870`), arg {0, 64,
   n 0, 64}: n counts the monsters other than the scanner that are not
   dead, within full-size distance < 64, and not hostile to it.
4. n ≠ 0: T ≠ 0, `Skill1` > 0, its skills row exists (`0x0045C4B0`),
   (`aurastate` < 0 or ≥ the states count or the unit lacks it),
   (`auratargetstate` < 0 or ≥ the count or T lacks it) and draw <
   aip1 [25] → `Skill1` at T, end; else idle 10. End.
5. n = 0, flag 1 clear: w ≤ 4 (unsigned) → wave record of w
   (`0x006556E0`); none or its class < 0 → return with nothing
   scheduled; else queue S→C message 0xA4 with the class on the unit
   (`0x00571C00`, client preload: a 12-byte pending event record {id
   0xA4 at +4, class as u16 at +8} appended to the unit's record list,
   head unit +0xEC, tail +0xF0, then the unit is queued for update
   `0x0064C040`, `sim/unit-order.md` §6; each client's update sends it,
   `sim/intents-events.md` §7.9, layout `sim/server-messages.tsv`
   0xA4), and for class 62 (fallenshaman5) also
   23 (fallen5), for 105 (unraveler5) also 381 (skmage_cold3). Then (any
   w) `0x005DEAD0(10, 285
   Baal Corpse Explode, unit, 0, 0)`; flags |= 1; q := frame + 250;
   sound 16 (`0x00553380(unit, 16, 0)`). End.
6. n = 0, flag 1 set, w ≥ 5: reinitialize the unit as class 559
   (baalcrabstairs) in mode 1 (`0x00574370(game, unit, 559, 1)`);
   re-install special state 0 (`0x005B0E00`); state 142 (`changeclass`)
   on; a stat list (flags 0, no expiry, owned by the unit;
   `0x006251F0`) attached (`0x00626E10`), its state := 142
   (`0x006252D0`), stat 355 (`shortparam1`) := 559 (`0x00627150`); idle
   5. End.
7. n = 0, flag 1 set, w < 5: spawn wave (`0x005EF210`): assign skill 286
   (BaalMonsterSpawn) at level 1 (`0x00647280`); its entry
   (`0x006439F0`; none → fatal) gets param := the mapped superunique id
   (`0x00644560`); `0x005DEAD0(10, 286, 0, own x, own y + 13)`; w += 1;
   q := frame + 100; flags |= 2; flags &= ~1.

1.14d-confirmed; same as D2MOO.

### 21. BaalCrab (135) `0x005FCFE0`, alternate `0x005FCF30`

Target mode 0. Skills used by id: 315 Baal Tentacle, 316 Baal Nova, 317
Baal Inferno, 318 Baal Cold Missiles; from monstats `Skill5` Baal
Teleport, `Skill6` Defense Curse, `Skill7` Blood Mana. AI param 1 =
clones made c. No aip is read.

1. X, M, n := Baal pick (§21.1).
2. H := the command of type 10 (`0x0058EEF0(10, 0)`); none → copy {10,
   own x, own y} and look again (the home point).
3. k := Baal choice (§21.2).
4. Execute k (§21.3); then wait 25 (it replaces any think the execution
   scheduled).

**Alternate** `0x005FCF30` (also BaalCrabClone's): the same as the
Diablo alternate (`ai-bodies-4.md` §7): keep the type-10 command's
point across the re-install, state 12 off, re-install the control's
state, idle 1.

#### 21.1 Baal pick `0x005FBDD0`

The boss pick of `ai-bodies-4.md` §7.1 with the Diablo cull
(`0x005FC000`: same act, distance < 1020) and the Baal score
`0x005FBB00` instead of §7.2, called without a command (so B = 0). The
Baal score is §7.2 except that r43 `coldresist` is read whether or not
the boss is in melee range.

#### 21.2 Baal choice `0x005FC630(game, unit, control, X, M, n, H)`

1. Param 0 ≠ 0 → return it.
2. X = 0: own position collides with mask 0x40 over pattern 2
   (`0x0064D910`) → 4. Else one draw: < 8 → 9, else 1.
3. close := X is a player whose portal object (`0x00552F60(game, 2,
   0x005353F0(X))`) lies in level 132 (The Worldstone Chamber) and (no
   H, or `0x006417F0(portal, H.x, H.y)` < 75). medium := H and the
   half-size distance X→(H.x, H.y) > 75; far := > 100. c := X's left or
   right skill is Blizzard or Meteor, or Fire Wall at level > 3, or
   Immolation Arrow at level > 7 (`0x005FC0C0`, as `ai-bodies-4.md` §7.3
   step 3). m := melee range unit→X; clear := `0x005DD290(unit, X)`.
4. Weights W0..W15 (b is the W1 base; L = own life percent):
   - m (`0x005FC1C0`): {0, b + 100 − L, 0, 0, 0, 0, 0, 0, 20, 30, 150,
     10, 10, 70, 40, 0}, b = 50; on Normal b = 75 and W13 = 45. X's life
     percent < 33 → W10 := 200. X has state 11 → W8, W13, W11 := 0. Not
     clear → W11, W13, W12 := 0.
   - Not m, clear (`0x005FC450`): {0, b + 100 − L, 5, 5, 5, 0, 5, 0, 40,
     40, 0, 70, 80, 60, 20, W15}, b = 100; on Normal b = 125 and W13 =
     40; W15 := 10 × (2 − c), 0 if negative. Full-size distance d
     (`0x005DC380`): d > 35 → W6 := 15, W13 := 0; d > 25 → W14 := 30,
     W11 := 0. n < 2 → W11 −= 10; n > 3 → W13 += 25. M > 60 → W8 := 70.
     Player-count record of the unit (`0x00573930`): n < 2 and its
     difficulty field (+0xC) < 2 → W8 := 0. medium → W6 += 25, W14 +=
     35, W2 := 0, W3 := 25; else c → W6 += 30, W11 += 10, W14 += 15. far
     → W5 := 60. close → W7 := 0.
   - Not m, not clear (`0x005FC300`): {0, b + 100 − L, 20, 20, 20, 0, 20,
     0, 80, 70, 0, 0, 0, 0, 0, 0}, b = 100 (125 on Normal). n < 2 → W2
     := 45, W10 := 25. medium → W13 := 25, W2 := 0, W6 := 0, W3 := 35,
     W11 := 25; else c → W13 := 50, W14 := 50. far → W5 := 60.
5. Σ := sum; Σ > 0: one step, r := `lo' & (Σ − 1)` for a power of two,
   else `lo' % Σ` (unsigned); Σ ≤ 0: r := 0, no draw. Return the first k
   with r < W0 + … + Wk; none → 1.

#### 21.3 Baal execution `0x005FCB60(game, unit, control, X, k, H)`

The unit's monstats row must exist (else nothing). By k:

- 1: idle 35 on Normal, 15 on Nightmare, 5 on Hell.
- 2, 6: walk in radius of X (`0x005DE6D0(X, 12, 0)`).
- 3: circle 6 at X with think delete.
- 4: wander 16.
- 5: H → walk to (H.x, H.y) (`0x005DED90`); else idle 5.
- 8: no X → idle 5. `Skill6` > 0 and (X's max mana < its max life, or X
  not a player) → `Skill6` at X. Else `Skill7` > 0 and X a player →
  `Skill7` at X. Else nothing.
- 9: `0x005DEAD0(9, 315, X, 0, 0)`.
- 10: A2 at X.
- 11: assign skill 316 at level 1 (`0x00647280`); `0x005DEAD0(10, 316,
  X, 0, 0)`.
- 12: `0x005DEAD0(8, 317, X, 0, 0)`.
- 13: `0x005DEAD0(4, 318, X, 0, 0)`.
- 14: no X → idle 5. d := `0x006416D0(unit, X)`, 0 → 1; P := own
  position + 25 × (own − X's position) / d per axis (signed). Class 709
  (uberbaal) → `Skill5` with no target at X's position. Else a free
  point near P in the unit's room for its class (`0x0054DC40(…, room,
  class, &P.x, &P.y, 0)`, `ai.md` §2.4) → `Skill5` with no target at P;
  none → idle 5.
- 15: clone (below); then idle 5.
- Any other (0, 7): idle 5.

**Clone** `0x005FC860`: nothing when the unit's minions include a
living one (`0x0058F380` with callback `0x005FC830`) or the unit has a
living owner (`0x00552FD0`). Class := 570 (baalclone; −1 when monstats
has ≤ 570 rows), mode 1, through the spawn info `0x0063EFA0(unit,
&class, &x, &y, &mode, difficulty, 0)` (`ai-bodies-2.md` §13.1).
For the unit's `BaseId` 544 (baalcrab; uberbaal shares it) and class
570 the spawn info keeps the class, draws x := own x + `roll(24)` −
12, then y := own y + `roll(24)` − 12 (two unit-seed steps,
`0x0045C390`) and sets mode 1; the clone overwrites x and y below, so
those two steps are spent and discarded. (For another class it would
draw `roll(2)` + difficulty into `0x0054DA60(562, …)`, then the same
two point draws, mode 4.) Any other `BaseId` not in its table gives
class 0, point (0, 0), mode 1 with no draws.
Base := the position of the unit's path target unit (`0x00553540`),
else its own. x := base.x + `lo' % 24` − 12, then y := base.y + `lo' %
24` − 12 (two steps, x first). Room at (x, y) (`0x00463740`); found →
spawn `0x005B2F20(room, x, y, class, mode, −1, 0)` (`monsters/init.md`
§1); spawned clone C: unit flags +0xC4 |= 0x04020000; wait 15 on C; the
unit's owner data := itself (`0x0058F030`), C joins its minion list
(`0x0058F100`) and C's owner := the unit; C stat 7 (`maxhp`) := the
unit's max life / 3, stat 6 (`hitpoints`) := its life / 3, stat 74
(`hpregen`) := 0 (`0x00627260`); each stores the other as owner
(`0x00621CE0`); sound 16 on the unit; c += 1.

### 22. BaalCrabClone (140) `0x005FD210`, alternate `0x005FCF30`

1. The unit's minion owner (`0x0058F0D0`) exists and is dead → kill the
   unit (`0x0057CCB0(game, unit, 0, 1)`, `combat/damage.md` §7.2). End.
2. Its owner (`0x00552FD0`) is missing or dead → kill it. End.
3. X, M, n := Baal pick (§21.1); H as §21 step 2; k := Baal choice; k
   ∈ {7, 9, 14, 15} → 2.
4. X → execute k (§21.3), wait 25. No X → idle 15.

1.14d-confirmed (§21, §22); same as D2MOO (1.10f cull 55).

rc-boss-570 (REC-2250, 1.14d-confirmed by check `gen-boss-570`): a clone
spawned with no owner takes step 2 on its first think and is killed with
`0x0057CCB0(game, unit, 0, 1)` (`combat/damage.md` §7.2): it is in death
mode (0) on that frame. The kill seam must run the real kill, not answer
"nothing".

### 23. Nihlathak (128) `0x005EE5D0`, init `0x005EE5C0`, alternate `0x005E5280`

Target mode 2. Brackets: nihlathakboss [30, 20, 80, 75, 8]; `Skill1` Imp
Teleport, `Skill2` Overseer Whip, `Skill3` NihlathakCorpseExplosion,
`Skill4` Horror Arctic Blast (`seq_nihlathakarctic`), `Skill5`
MinionSpawner. E1 := the unit's highest `Skill1` entry (`0x006439F0`).
"Blink" = teleport in range aip2 [20] with E1's skill id
(`0x00643CE0`) and E1's mode (entry +0x08, `0x00644360`). The init does
nothing.

1. Quest seam `0x0058BC40(game, unit)` (`world/quests.md`).
2. State 12 set → off.
3. T = 0: E1 and AI state 3/19 → blink; else idle 25. End.
4. E1, C and draw < aip1 [30] → blink. End.
5. D < aip5 [8], draw < 40 and escape from T by 5 with think delete
   started → end.
6. `Skill3` > 0 and its entry E3 exists: draw < aip3 [80] → K := corpse
   search `0x0056E390(unit, T, Skill3, level of E3 with bonus)`
   (skills spec; a unit find of size 10, flags 0x1002, around T's
   position); K and
   `Skill3` at K succeeds → end.
7. `Skill4` > 0 and its entry exists: draw < 60 and D < 14 → `Skill4` at
   T. End.
8. `Skill2` > 0: draw < aip4 [75] and has `Skill2` (`0x005DD2D0`): whip
   scan (§7 step 5's callback, arg {0, 625, 50, 0, 0, 0}: only the whip
   slot fills — a non-unique minion1 within squared distance 625);
   found → `Skill2` at it, end. Else `Skill5` > 0 and the footprint test
   `0x005FD350(528, room, own x, own y, 0)` passes (class 528 evilhut;
   −1 when monstats has ≤ 528 rows; `monsters/population.md` §9) →
   control +0x3C (minion spawn class) := `0x0063EC70(room, 453)`
   (`population.md` §11.6), `Skill5` at T, end. Else wander 6. End.
9. `Skill4` > 0, its entry exists and D ≤ 13 → `Skill4` at T. End.
10. Draw < 60 → walk to T; then idle 5 (both).

**Alternate** `0x005E5280` (also the Hireable alternate, `ai.md` §3.2):
state 12 off; re-install the control's current state; idle 1. No
draws. 1.14d-confirmed.

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| Imp rows | 492–495 aips; mount range I2.aip1 | `0x005E2FF0` |
| teleport in range | two draws over 2r | `0x005DF850` |
| curse test | stat list flag 0x20 | `0x00625760` |
| Overseer scan | minion1 `BaseId` 453; radii² 576 / 400; hurt < 50 %; state 141 | `0x005E26D0` |
| Shenk seam | superunique 42 | `0x00587900` |
| Impregnate | skill 300, mode 8; host radius 25; states 110 | `0x005EFA90`, `0x005D2140` |
| Ancients | classes 540–542; whirlwind reach aip4; state 54 | `0x005EF1A0` |
| statue | skill 302, mode 4; state 146 | `0x005EEAA0` |
| siege beast | imp `BaseId` 492; stomp range skills `Param5` | `0x005E1900`, `0x005E1720`, `0x005E17E0` |
| Baal taunt | skill 284 mode 4; counter aip2 | `0x005EF710` |
| Baal stairs | object 563; vanish at < aip1 | `0x005EF620` |
| Baal throne | waves superuniques 61–65; skills 285, 286; delays 250 / 100; spawn point (x, y + 13); class 559, state 142, stat 355 | `0x005EF320`, `0x005EF210` |
| Baal | skills 315–318; teleport 25 away; weights §21.2; idles 35 / 15 / 5; clone class 570 at ±12, a third of life, flags 0x04020000 | `0x005FC630`, `0x005FCB60`, `0x005FC860` |
| Nihlathak | blink range aip2; corpse search size 10 at T; spawner footprint class 528, spawn class 453 for level | `0x005EE5D0` |

## Randomness

All draws are on the thinking monster's unit seed (`rng.md` §7), in the
order of each section; a test that is not reached draws nothing. Forms:
`lo' % 100`, `roll(2)` (Ancients), the two range draws of "teleport in
range", plus the helpers' draws (`ai.md` §7.2). Scans, finds and the
boss pick draw nothing. Skill checks, the quest seams and the skills
used have their own draws.

## Edge cases & original bugs

1. Imp §3 reads imp1..imp4's aips for every Imp row.
2. Minion §2 uses aip2 both as the stall and the A2 chance.
3. SiegeBeast §15 step 1: a dying or dead beast schedules nothing; step
   5 clamps aip7 to 127, above the velocity request's assert limit of
   126 (`ai.md` §7.3) — unreachable with live data (100).
4. BaalThrone §20 step 2 idles by a negative amount (frame − q);
   `0x005DE080` passes it to the timer queue as frame + (frame − q)
   (`tick.md` §5 decides what a past frame does).
5. BaalThrone §20 step 5 returns with nothing scheduled when a wave
   record is missing.
6. SuicideMinion §16 with aip5 = 0 detonates on the second think after
   reaching melee.

## Test vectors

Synthetic (CI-safe), draws given as `lo' % 100`:

| AI, input | Draws | Result |
|---|---|---|
| Minion, no command, C = 1, minion1 | 50 (< 70), 10 (< 15) | A2 at T |
| BloodLord, C = 1, bloodlord1 | 50 (< 90), 60 (≥ 50) | A1 at T |
| DeathMauler, C = 0, D = 10, deathmauler1 | 30 (< 50) | DeathMaul at T |
| SuicideMinion, f = 0, C = 1, suicideminion6 | none | f := frame; idle 5; next think: death |
| BaalTaunt, T a player in mode 1, n = 10 | none | n := 0, taunt at T |
| FrozenHorror, `Skill1` level 4, D = 3, no state 12 | 50 (< 60) | Arctic Blast at T |

Game-file vectors: Open question 1.

## Provenance

- 1.14d `Game.exe`: `0x005E1B60`, `0x005E2FD0`, `0x005E2FF0`,
  `0x005DF850`, `0x00552FD0`, `0x005E1E00`, `0x00625760`,
  `0x006256E0`, `0x005E36F0`, `0x005DECA0`, `0x005E2120`, `0x005E27A0`,
  `0x005E26D0`, `0x00587900` (entry only), `0x005DE830`, `0x005DD2D0`,
  `0x005E1540`, `0x005F1DE0`, `0x005EE260`, `0x005EFA90`, `0x005EFA50`,
  `0x005D2140`, `0x005EF1A0`, `0x005EEB70`, `0x005EEDA0`, `0x005EF010`,
  `0x005EEB10`, `0x0058CF90`, `0x005EEAA0`, `0x005E3530`, `0x005E1900`,
  `0x005E1720`, `0x005E17E0`, `0x004E6C70`, `0x005E1D30`, `0x005EF910`,
  `0x005EF710`, `0x005EF620`, `0x005EF5E0`, `0x005EF320`, `0x005EF310`,
  `0x005EF210`, `0x005DC870`, `0x00571C00`, `0x005DE080`,
  `0x005FCFE0`, `0x005FCF30`, `0x005FD210`, `0x005FBDD0`, `0x005FC000`,
  `0x005FBB00`, `0x005FC630`, `0x005FC1C0`, `0x005FC300`, `0x005FC450`,
  `0x005FC0C0`, `0x005FCB60`, `0x005FC860`, `0x005EE5D0`, `0x005EE5C0`,
  `0x005E5280`, `0x0056E390` (entry only). Ghidra
  decompile read first; register and stack arguments checked in the
  disassembly; table `0x006E3528` read from the file.
- Live data (`patch_d2`): monstats.txt, levels.txt (Act 4 rows),
  superuniques.txt (rows 42–65), skills.txt (284, 285, 286, 300, 302),
  states.txt (110, 141, 142, 146), objects.txt (563) — counted by the
  throwaway script of `ai-bodies-3.md`.
- D2MOO 1.10f `AiThink.cpp`: names and param meanings; compared for
  every body here: same rules except Ancient B step 5; `AiBaal.cpp`
  for §21–§22 (same, with the 1.10f cull 55).

## Open questions

1. No recording of any Act V AI: record a Bloody Foothills / Arreat
   Summit / Worldstone Chamber run (tick recorder) and compare think
   schedules and draws with §2–§23.
2. Answered (2026-10-07): the free-point search `0x0054DC40` draws on
   the room seed, x then y per try, up to 20 tries
   (`monsters/population.md` §8); the clone's spawn info is
   `ai-bodies-2.md` §13.1 (key 544, as OQ10).
3. Answered (2026-10-07): besides SiegeBeast §15 (`0x005E17E0`) the
   imp's param 0 is written by its init (−1), its own think (§3) and
   the inactive restore `0x00541E20` (param 0 := the stored record's
   +0x4C, every monster; `sim/units.md` §3.4). The 0xA4 record is §20
   step 5; what the client does with it belongs to the client specs.
4. Answered (`docs/handoff/impl-ai-acts2-5.md` reading 11): §3 step 5's
   "else" is the distance test (E ≥ I3.aip3), not a failed I3 draw; a
   failed I3 draw goes to step 6 (`0x005E2FF0`). A missing imp row
   (monstats count ≤ 492..495) is a null read in 1.14d; live data has
   all four rows, so an implementation asserts them.
5. Answered (reading 12): §6 step 3.1 draws aip8 only when S was found
   (the tests run left to right and stop at the first failure;
   `0x005E2120`).
6. Answered (reading 13): §7 step 4, C with a draw ≥ aip6 goes on to
   the minion scan of step 5 (`0x005E27A0`).
7. Answered (reading 14): request byte +0x15 is the path type of the
   move (`ai.md` §7.1 mode request record): 100 = no path computed. A
   pending velocity request's method replaces it.
8. Answered (reading 15): "nearer" is squared distance, strict: equal
   distances reassign the imp to the scanning beast (`0x005E17E0`).
9. Answered (reading 16): §20 step 7 reads no wave record: the skill
   param is `0x00659B80(2, 0x00586B30(table[w]))` (superunique map +
   the monstats count) whatever the record. A missing record already
   returns in step 5 before flag 1 is set, so step 7 is never reached
   for it; no −1 and no fatal path exists (`0x005EF320`, `0x005EF210`).
10. Answered (reading 17, half of OQ2): the clone's spawn info for base
    544 draws two `roll(24)` steps (discarded) and keeps class 570 and
    mode 1 (§21.3 Clone). The free-point search draws of case 14
    remain `ai.md` open question 4.
