# Spec: Monsters — AI think bodies, Act IV

- **Status:** draft: the 10 AI functions used by Act IV monsters that
  were still unread, the Diablo alternate, the boss target pick and
  target score shared with later bosses, and the scan callbacks they
  use, read from the 1.14d `Game.exe` disassembly (addresses per
  section; register arguments checked in `all.asm`). No recording
  covers these bodies yet (Open questions 1–2).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::monsters::ai` (per-AI functions beside the
  `ai-bodies.md` §9 bodies)
- **Related specs:** `monsters/ai.md` (owner of scheduling §1, dispatch
  §2, AI control and tables §3, aip reads §4, target search §5,
  distances §6, tactics helpers §7, commands §8, the conventions of §9);
  `monsters/ai-bodies-3.md` (Act III bodies and the conventions this
  file reuses); `monsters/ai-functions.tsv` (catalogue);
  `monsters/population.md` §11.6 (class chains); `sim/pathing.md`
  (directions, path compute); `sim/path-placement.md` (collision
  queries); `skills/levels.md` (skill level); `world/quests.md` (quest
  seams); skills spec (skill checks, skill bodies).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 48–62 |
| Inputs | 63–73 |
| Outputs / state changes | 74–79 |
| Rules | 80–81 |
|   1. Scope and order | 82–107 |
|   2. VileMother (68) `0x005FA010` | 108–145 |
|   3. VileDog (69) `0x005FA280` | 146–155 |
|   4. FingerMage (70) `0x005FA380` | 156–179 |
|   5. Regurgitator (71) `0x005FA710` | 180–214 |
|   6. Megademon (89) `0x005E0C80` | 215–231 |
|   7. Diablo (51) `0x005E9170`, alternate `0x005E8480` | 232–361 |
|   8. Izual (55) `0x005F89B0` | 362–382 |
|   9. DoomKnight (72) `0x005FAA90` | 383–392 |
|   10. AbyssKnight (73) `0x005FAB80` | 393–414 |
|   11. OblivionKnight (74) `0x005FAF00` | 415–448 |
| Constants & data dependencies | 449–461 |
| Randomness | 462–470 |
| Edge cases & original bugs | 471–487 |
| Test vectors | 488–503 |
| Provenance | 504–528 |
| Open questions | 529–540 |
<!-- /index -->

## Summary

Act IV levels (levels.txt `Act` = 3), the Act IV superuniques
(superuniques.txt rows 36–38: Infector of Souls, Lord De Seis, Grand
Vizier of Chaos; row 41 Hephasto uses the Smith AI, `ai-bodies.md` §9.30),
Izual and Diablo, plus their minions and spawns, give 29 monstats rows
(Provenance). Their AIs not yet specified are written here, in
descending order of rows (§1). The conventions are those of
`ai-bodies-3.md` Summary: T, D, C, P(aipN), "draw" (`lo' % 100`),
"idle N", "wait N", A1 / A2 / S1 / S4 = mode 4 / 5 / 8 / 11 at a unit,
"Skill k at U" = `0x005DEAD0(Skk mode, Skill k, U, 0, 0)`, L = own life
percent (`0x00621F20`), positions by `0x0045ADF0` / `0x0045AE20`,
frame compares signed. Bracketed values are the Normal values of the
named live row.

## Inputs

| Name | Type | Source |
|---|---|---|
| tick record | `ai.md` §2.1 | control (+0x00), T (+0x08), D (+0x14), C (+0x18), monstats (+0x1C) |
| AI control | `ai.md` §3.1 | state (+0x00), params 0–2 at +0x14, +0x18, +0x1C, commands |
| monstats | 424 bytes | `aip1`..`aip8`, `Skill1`..`Skill8` (+0x170..+0x17E), `Sk1mode`..`Sk8mode` (+0x180..+0x187), `Velocity`, `Run`, `aidel` (+0x4F + difficulty), `threat` (+0x4E), chain byte +0x4B |
| skills | 0x23C bytes | `aurastate` (+0x80), `auratargetstate` (+0x82), `attackrank` (+0x18E) |
| target stats | | 22, 36, 37, 39, 41, 43, 49, 51, 53, 55, 58 (Diablo score) |
| game | | +0x6D difficulty, +0xA8 frame, +0x10F8 target-node lists (`ai.md` §5.2) |

## Outputs / state changes

Mode requests, think schedules, AI control params, velocity requests,
commands of type 10 (Diablo home), state 12 off (Megademon, Diablo),
the quest seam call (Izual), unit-seed draws in the order given.

## Rules

### 1. Scope and order

Rows counted by the script of `ai-bodies-3.md` (Act 3 levels, rows
36–38 and 41, bosses izual, diablo, hephasto, closed over `minion1`,
`minion2`, `spawn`):

| AI (index) | Act IV rows | Rows | § |
|---|---|---|---|
| VileMother (68) | 3 | vilemother1–3 | §2 |
| VileDog (69) | 3 | vilechild1–3 | §3 |
| FingerMage (70) | 3 | fingermage1–3 | §4 |
| Regurgitator (71) | 3 | regurgitator1–3 | §5 |
| Megademon (89) | 3 | megademon1–3 | §6 |
| Diablo (51) | 1 | diablo | §7 |
| Izual (55) | 1 | izual | §8 |
| DoomKnight (72) | 1 | doomknight1 | §9 |
| AbyssKnight (73) | 1 | doomknight2 | §10 |
| OblivionKnight (74) | 1 | doomknight3 | §11 |

Ties are listed by index. The other Act IV rows (batdemon5, bighead5,
blunderbore4, maggotbaby5, maggotegg5, sandleaper5, sandmaggot5,
willowisp3, hephasto) use AIs already specified. 25 of the 29 rows
have `switchai` (special states 10–12, `ai-bodies-2.md` §16). No body
here installs a special state; the Diablo alternate re-installs the
control's current state.

### 2. VileMother (68) `0x005FA010`

Brackets: vilemother1 [16, 5, 30, 80, 70, 30, 10]; `Skill1` Nest
(`seq_vileres`); `spawn` vilechild1. AI param 0 = births b.

1. Child class Y := `0x0054DA60(301, n)`: the `BaseId` of row 301
   (vilechild1; −1 when monstats has ≤ 301 rows) followed n steps along
   `NextInClass` (`monsters/population.md` §11.5 rule 3), with n = the
   monstats byte +0x4B of the unit's class (`0x006510C0(class, 0, 0)`;
   class −1 → 0). So vilemotherK gives vilechildK.
2. b < aip1 [16], the unit's alignment is 0 (`0x006259B0`) and P(aip3)
   [30]: count scan, scan 12 (`ai.md` §5.4, callback `0x005DCE10`),
   arg {Y, 25, 0}: a unit counts when it is a monster of class exactly
   Y, not dead (`0x005541B0`), with full-size distance from the scanner
   (`0x005DC380`, `ai.md` §6) ≤ 25. Count < aip2 [5] and the **birth**
   below succeeds → b += 1. End.
3. C: P(aip4) [80] → A1 at T; else idle aip7 [10]. End.
4. b < aip1 and draw ≥ aip5 [70] → draw < aip6 [30] → circle 4 at T
   (`0x005DF7D0(T, 4, 0)`), else idle 15. End.
5. Walk to T with flags 7 (`0x005DEC80`).

**Birth** `0x005F9E30(game, unit, T, monstats)` → 1 / 0. d := the
64-step direction from the unit's position to T's (`0x00621DC0` →
`0x0064FDC0`, `sim/pathing.md` §8.3), reduced to 8 directions by table
`0x00745600` (0–3 → 0, then 8 per step, 60–63 → 0). For i = 0..7: k :=
(d + i) & 7; e := {6, 4, 2, 2, 2, 0, 6, 6}[k]; offset := 3 × (dx[e],
dy[e]) with dx = {0, −1, −1, −1, 0, 1, 1, 1} (`0x006EA998`), dy = {−1,
−1, 0, 1, 1, 1, 0, −1} (`0x006EA978`), i.e. k 0 (3, 0), 1 (0, 3), 2–4
(−3, 0), 5 (0, −3), 6–7 (3, 0). P := own position + offset. `Skill1`
≥ 0 and the footprint test `0x005FD350(298, room, P.x, P.y, 0)` passes
(class 298 vilemother1, −1 when monstats has ≤ 298 rows; for this
class and fifth argument 0: the room at P, `0x00463740`, exists and
`0x0064D9B0(that room, P, 2, 0x3C01)` = 0; `monsters/population.md`
§9) → `0x005DEAD0(Sk1mode, Skill1, 0, P.x, P.y)`, return 1. After 8
failures return 0. No draws.

1.14d-confirmed.

### 3. VileDog (69) `0x005FA280`

Brackets: vilechild1 [80, 10, 80]. AI param 0 = "woken".

1. Param 0 = 0 → param 0 := 1; idle 5. End.
2. C: P(aip1) [80] → A1 at T; else idle aip2 [10]. End.
3. P(aip3) [80] → walk to T with flags 7; else idle 10.

1.14d-confirmed.

### 4. FingerMage (70) `0x005FA380`

Brackets: fingermage1 [40, 40, 50, 30, 15, 5, 40, 12]; `Skill1`
FingerMageSpider (`S1`). AI params: 0 = fleeing f, 1 = flee thinks n.
v = run speed bonus (`ai-bodies-3.md` §4).

1. AI state 3/19 (`0x005DD2B0`): f := 0. C → A1 at T, end. `Skill1` ≥
   0 and D < aip5 [15] → `Skill1` at T, end. Otherwise go on.
2. f = 1: n += 1. L > aip3 [50], or draw < 25, or n > aip6 [5] (each
   tested only when the earlier fail) → f := 0, idle 15, end. Else D <
   14 → velocity request (0, v, 0), escape from T by 14 without delete
   (`0x005DEFE0(T, 14, 0)`), end; else idle 15, end.
3. No minion owner (`0x0058F0D0` = 0) and L < aip4 [30] → f := 1, n :=
   0; escape from T by 9 without delete. End.
4. C: P(aip1) [40] → A1 at T. End. `Skill1` ≥ 0 and draw < aip2 [40] →
   `Skill1` at T. End. Circle 5 at T (no delete).
5. Not C, D < aip5: `Skill1` ≥ 0 and draw < aip2 → `Skill1` at T. End.
   T has state 84 (`fingermagecurse`, `0x00639DF0(T, 84)`) → walk to T
   with flags 0; else idle aip8 [12].
6. Not C, D ≥ aip5: D < aip7 [40] → walk to T with flags 7; else idle
   15.

1.14d-confirmed.

### 5. Regurgitator (71) `0x005FA710`

Brackets: regurgitator1 [70, 20, 40, 10, 3, 25]; `Skill1`
RegurgitatorEat (`S1`). AI params: 0 = state s, 1 = corpse GUID g, 2 =
approach steps k. K := the monster with GUID g (`0x00552F60(game, 1,
g)`). "Reset" = s, g, k := 0. "Go to corpse U" = walk to U with 1 step
(`0x005DEF80(U, 1)`), s := 2, g := U's GUID (+0x0C), k := 0.

1. s = 5: escape from T by 16 with think delete; s := 0. End.
2. s = 2: K exists and is in mode 12 (+0x10): squared distance unit→K
   (`0x005B0BD0`) > 4 → k ≥ 6 → reset, wander 8, end; else walk to K
   with 1 step, k += 1. Then s := 3; idle 8 (both requests are made, in
   this order). End. K missing or not in mode 12 → reset; wander 8.
3. s = 3: `Skill1` ≥ 0 and K exists in mode 12 → `Skill1` at K; s := 4.
   End. Else reset; wander 8.
4. s = 4: not C → A2 at T, s := 5. C → escape from T by 8 with delete.
   End.
5. Corpse scan: scan 1, callback `0x005FA690`, arg {best 0, max aip6²
   [625]}. U is taken when it is a monster other than the scanner, with
   unit flag 0x200 clear and 0x2 set, in mode 0 or 12 (`0x0063EA40` ≠
   0), without a `udead` state, alignment 0, with the monstats2 `soft`
   flag (`0x004638A0(class, 12)`), and squared distance d ≤ max; then
   best := U, max := d (ties go to the later unit).
6. s = 1: best found → g := its GUID; d > 2 → walk to it with 1 step,
   s := 2, k := 0; else idle 8, s := 3, k := 0. End. None: draw < 20 →
   reset with **nothing scheduled**; else wander 8. End.
7. Best found and ((d < 9 and draw < aip2 [20]) or draw < aip5 [3]) →
   go to corpse. End. (The first draw only when d < 9; the second only
   when the first branch fails.)
8. Draw r. C: r < aip1 [70] → A1 at T; else idle 15. End. Not C: r <
   aip3 [40] → walk to T with flags 0. End. Best found and draw < aip4
   [10] → go to corpse. End. Idle 8.

1.14d-confirmed.

### 6. Megademon (89) `0x005E0C80`

Brackets: megademon1 [50, 40, 80, 70, 50, 75]; `Skill1`
MegademonInferno. AI param 0 = inferno cooldown frame. R := level of
`Skill1` (as `ai-bodies-3.md` §7 step 1); I := the unit has state 12.

1. `Skill1` ≥ 0, not C and D < R: I → state 12 off, step 3. Not I:
   frame > param 0 and draw < aip1 [50] → param 0 := frame + aip6 [75];
   `Skill1` at T; end. Else step 3.
2. Otherwise I → state 12 off (`0x00639DB0(unit, 12, 0)`). C:
   `Skill1` ≥ 0, frame > param 0 and draw < aip2 [40] → param 0 :=
   frame + aip6; `Skill1` at T; end. Draw < aip3 [80] → A1 at T, end.
   Draw < aip5 [50] → circle 3 at T (no delete); else idle 5. End.
3. Draw < aip4 [70] → walk to T with flags 7; else idle 10.

1.14d-confirmed.

### 7. Diablo (51) `0x005E9170`, alternate `0x005E8480`

`Skill1` DiabLight, `Skill2` DiabCold, `Skill3` DiabFire, `Skill4`
DiabWall, `Skill5` DiabRun, `Skill6` PrimeFirewall, `Skill7`
DiabPrison (skill 199), `Skill8` none. No aip is read. AI param 0 =
pending choice k (5 = keep channeling).

1. H := the command of type 10 (`0x0058EEF0(10, 0)`, `ai.md` §8);
   none → copy {10, own x, own y} (`0x0058EF40`) and look again (the
   home point).
2. X, M, n := boss pick (§7.1) with cull §7.1 and H.
3. k := choice (§7.3).
4. Aura: k ≠ 5, `Skill8` ≥ 0, its skills row exists, its `aurastate`
   ≥ 0, the unit lacks that state, and `Skill8` at X succeeds
   (`0x005DEAD0` ≠ 0) → param 0 := 0. End. (Diablo has no `Skill8`.)
5. By k (each ends with param 0 := 0 unless noted):
   - 1: velocity (0, 20, 0); walk to X's position (`0x005DED90`).
   - 2 / 3 / 4: A1 / A2 / S4 (mode 11) at X.
   - 5: `Skill1` < 0 → idle 2. State 12 set → state 12 off, idle 2.
     Else `Skill1` at X; param 0 := 5.
   - 6 / 7 / 8 / 9 / 10 / 13: `Skill3` / `Skill2` / `Skill4` / `Skill7` /
     `Skill5` / `Skill6` at X; that skill < 0 → idle 2.
   - 12: circle 4 at X (no delete).
   - 14 (return home): velocity (0, 50, 100); walk with think delete
     to H (`0x005DEE50(H.x, H.y)`); started → end. Else delete thinks
     (`0x00540E60(2, 0)`); d := path distance unit→H (`0x005DC5C0`);
     sx := −1 if own x < H.x, +1 if own x > H.x, else 0; sy likewise;
     velocity (0, 50, 100); walk with delete to (H.x + (d >> 1)·sx,
     H.y + (d >> 1)·sy); not started → delete thinks, idle 2.
   - 15: X not a player → idle 3. Portal := `0x00552F60(game, 2, G)`
     with G = X's portal GUID (`0x005353F0`); none → idle 2. `Skill7`
     < 0 → idle 2. Else `0x005DEAD0(Sk7mode, Skill7, 0, G, 2)` (no
     target; the point carries the portal GUID and unit type 2 for the
     skill).
   - 16: velocity (0, 20, 0); wander 5.
   - any other (0, 11): idle 12 on Normal, 8 on Nightmare, 4 on Hell.

**Alternate** `0x005E8480` (`ai.md` §3.3): (x, y) := params 1, 2 of the
type-10 command, 0 when none; state 12 off if set; re-install the AI
for the control's current state (`0x005B0E00`, which frees all
commands); no type-10 command now, x ≠ 0 and y ≠ 0 → copy {10, x, y};
idle 1. No draws.

1.14d-confirmed (all of §7); same as D2MOO.

#### 7.1 Boss pick `0x005E8F20(game, boss, &max, &count, H, cull)`

Shared by Diablo here and later bosses (D2MOO
`AITHINK_GetTargetForBoss`). Score s(U) is §7.2; best := 0, max := 0,
count := 0; "take U" = if s > max: max := s, best := U.

1. Slots 0–7 of the target-node lists (`ai.md` §5.2): a head that is
   not a player is a fatal assert. If cull(boss, head) holds: the head
   scores 0 when its anim mode is 0 or 17 (death, dead), else s(head);
   take it; count += 1; then each following node of that slot: score,
   take, count += 1. A culled-out slot is skipped whole.
2. Slot 8: every node in the boss's act (unit +0x18 byte): score, take,
   count += 1.
3. Slot 9: A := the best-scoring node in the boss's act (strictly
   greater replaces). If A, the no-size distance from A to the boss's
   position (`0x005DC530`) < 5, and the boss is not in melee range of
   best (`0x00622C40(boss, best, 0)` = 0): set the boss's path target
   to best (`0x00648B90`), path type 2 (`0x00648CF0`), compute the path
   (`0x00649970(path, boss, 0)`, `sim/pathing.md` §3); no path points
   (`0x00648780` = 0) → best := A, max := s(A).
4. Write max and count; return best (0 possible). No draws.

**Diablo cull** `0x005E8EB0(boss, U)`: same act, and the no-size
distance from U to the boss's position < 1020 (D2MOO 1.10f: 55).

#### 7.2 Target score `0x005E8530(boss, U, H)`

1. m := melee range boss→U (`0x00622C40`). B := 100 when H exists, the
   half-size distance (`0x005DC480`, `ai.md` §6) from the boss to (H.x,
   H.y) > 85 and from U to (H.x, H.y) < 85; else 0.
2. Stats of U (`0x00625480(U, s, 0)`): r36 `damageresist`, r39
   `fireresist`, r41 `lightresist`, r37 `magicresist`, r43
   `coldresist` (0 unless m), d22 `maxdamage`, d49 `firemaxdam`, d51
   `lightmaxdam`, d53 `magicmaxdam`, d55 `coldmaxdam`, d58
   `poisonmaxdam`; cold := U has state 11 (`cold`); low := U's life
   percent ≤ 19.
3. K := `attackrank` (skills +0x18E) × level (`0x006442A0`, with
   bonus) of U's left skill (`0x00620190`) + the same for its right
   skill (`0x006201D0`); 0 for a missing skill.
4. Range G := 100 when m; else 75 when the line boss→U is clear
   (`0x005DD290`); else 0.
5. U a monster whose class has no monstats row or `threat` < 2 → score
   0.
6. score := (300·low + (r43 + 4·(r36 + 2·r37) + r41 + r39) / 15 +
   5·(B + G) + 2·(K / 4 + 200·cold + (d58 >> 8 + d55 + d53 + d51 + d49
   + d22) / 2)) / 22, with low and cold as 0/1, all divisions signed and
   truncating; 0 → 1.

#### 7.3 Choice `0x005E8810(control, boss, X, M, n, H)`

1. Param 0 ≠ 0 → return it.
2. X = 0: the boss's position collides with mask 0x40 over pattern 2
   (`0x0064D910(room, x, y, 2, 0x40)`) → 16. Else one step: `lo' %
   1000` = 0 → 4, else 11.
3. Facts. Portal P: X a player → `0x00552F60(game, 2, X's portal
   GUID)`; near := P exists, its level (`0x0061A1B0`) is 108 (Chaos
   Sanctuary) and `0x006417F0(P, H.x, H.y)` < 85 (`sim/pathing.md`).
   Special c := X's left or right skill is 56 (Meteor) or 59 (Blizzard),
   or 51 (Fire Wall) at level > 3, or 27 (Immolation Arrow) at level >
   7. far := half-size distance X→H > 85; further := > 105 (both 0
   without H). m := melee range boss→X; clear := `0x005DD290(boss, X)`;
   f39, f41 := X's stats 39, 41.
4. Weights W[0..16]:
   - m: {0, 0, 40, 70, 0, 40, 24, 40, 15, 0…}. X's life percent < 20 →
     W2 := 50. X has state 11 → W7, W8 := 0. f39 > f41 → W6 −= 10;
     f39 < f41 → W6 += 10. Not clear → W5, W6 := 0. near → W15 := 10.
   - Not m, clear: {0, 0, 0, 0, 0, 25, 25, 0, 15, 20, 10, 0, 20, 0…}.
     Full-size distance boss→X (`0x005DC380`) > 25 → W8 −= 5, W10 :=
     20, W5 := 0. f39 > f41 → W6, W8 −= 10; f39 < f41 → W6, W8 += 10.
     n < 2 → W6 −= 10; n > 3 → W6 += 5. M > 60 → W9 += 10. Player-count
     record of the boss (`0x00573930`, `monsters/init.md` §9): n < 2 and
     its difficulty field (+0xC) < 2 → W9 := 0. c and not far → W6 +=
     10, W10 := 30, W13 := 15. far → W1 := 0, W12 := 10, W10 := 0, W13
     := 15, W9 = 0 → W9 := 10. further → W9 := 20, W14 := 60. near →
     W15 := 15.
   - Not m, not clear: {0, 0, 0, 0, 5, 0, 25, 0, 25, 40, 0, 0, 25, 0…}.
     n < 2 → W8 −= 5, W6 := 0, W1 := 25, W9 := 0. c and not far → W13
     := 15. far → W1 := 0, W12 := 15, W13 := 25, W9 = 0 → W9 := 20, n
     < 2 → W9 −= 5. further → W14 := 60. near → W15 := 20.
5. Skill check `0x005FD470(199, X, 0, 0)` ≠ 0 → W9 := 0. W15 ≠ 0, P
   exists and `0x005FD470(199, P, 0, 0)` = 0 → W15 := 0.
6. Σ := sum of W. Σ > 0: one step; r := `lo' & (Σ − 1)` when Σ is a
   power of two, else `lo' % Σ` (unsigned). Σ ≤ 0: r := 0, no draw.
   Return the first k with r < W0 + … + Wk (signed); none → 11.

### 8. Izual (55) `0x005F89B0`

Brackets: izual [45, 50, 66, 0, 20, 3]; `Skill1` Frost Nova (`SC`). AI
params: 0 = activated, 1 = pending stall s, 2 = swings left w. (x, y)
:= T's position (read first). "Nova" = `0x005DEAD0(Sk1mode, Skill1, T,
x, y)`, s := aip5 [20], w := aip6 [3].

1. Param 0 = 0 → param 0 := 1; quest seam `0x005B4390(game)` (D2MOO
   `ACT4Q1_OnIzualActivated`; `world/quests.md`).
2. s ≠ 0 → idle s; s := 0. End.
3. C: w > 0, or P(aip1) [45] (drawn only when w ≤ 0) → w > 0 → w −= 1;
   A1 at T. End. Else w := 0; `Skill1` ≥ 0 and draw < aip4 [0] → nova;
   else idle `aidel` of the difficulty (monstats u8 +0x4F + d). End.
4. Not C, `Skill1` ≥ 0 and D < 10: w > 0 → walk to T with flags 7, end;
   draw < aip3 [66] → nova, end.
5. w < 1 and draw ≥ aip2 [50]: D < 11 → idle `aidel`; else walk in
   radius of T (`0x005DE6D0(T, 6, 9)`). End.
6. Walk to T with flags 7.

1.14d-confirmed; same as D2MOO.

### 9. DoomKnight (72) `0x005FAA90`

Brackets: doomknight1 [90, 10, 80, 10].

1. Not C: P(aip3) [80] → lunge to T (`0x005DED40(T, 7)`); else idle
   aip4 [10]. End.
2. C: P(aip1) [90] → A1 at T; else idle aip2 [10].

1.14d-confirmed.

### 10. AbyssKnight (73) `0x005FAB80`

Brackets: doomknight2 [40, 80, 90, 10, 6, 1, 70, 40]; `Skill1`
DoomKnightMissile (`S1`), `Skill2` MonBoneArmor, `Skill3`
MonBoneSpirit. AI param 0 = shot cooldown c. "Shot allowed" = the unit
is not a monster with monster data whose byte +0x0E (D2MOO
`nComponent[10]`) is ≥ 4.

1. `Skill2` ≥ 0, its skills row exists, its `aurastate` ≥ 0, the unit
   lacks that state, L < aip1 [40] and draw < aip2 [80] → `Skill2` with
   no target, point (0, 0). End.
2. C: P(aip3) [90] → A1 at T; else idle aip4 [10]. End.
3. D < aip5 [6] and c ≤ 0 → c := aip6 [1].
4. `Skill1` ≥ 0, c = 0 and shot allowed → `Skill1` at T; c := aip6.
   End.
5. c > 0 → c −= 1.
6. Draw < aip7 [70] → velocity (2, 0, steps: next draw `lo' & 1` + 6);
   walk to T with flags 7. End.
7. D < aip8 [40] → circle 3 at T (no delete); else idle 15.

1.14d-confirmed; same as D2MOO.

### 11. OblivionKnight (74) `0x005FAF00`

Brackets: doomknight3 [6, 25, 500, 50, 80, 30, 30, 9]; `Skill1`
DoomKnightMissile, `Skill3` MonBoneSpirit, `Skill4` Decrepify, `Skill6`
MonCurseCast; minion doomknight1. AI param 0 = curse cooldown frame.
"Shot allowed" as §10.

1. Knight scan (always): scan 1, callback `0x005FAE40`, arg {N 0, W 0,
   max 2500, 0x7FFFFFFF, 0x7FFFFFFF}. U counts when it is a monster
   other than the scanner, not in mode 0 or 12, with unit flag 0x2,
   `BaseId` 310 (doomknight1), squared distance d ≤ 2500, the
   scanner's alignment pairing and alignment 0: nearest → N; nearest
   with life percent < 40 → W (unused).
2. D < aip1 [6]:
   1. `Skill4` > 0, its skills row exists, its `auratargetstate`
      (+0x82) > 0 and T lacks that state → `Skill4` at T; param 0 :=
      frame + aip3 [500]. End.
   2. N exists, its squared distance > D², and walk to N with flags 4
      (`0x005DEC80(N, 4)`) started → end.
   3. Velocity (2, 50, 0); escape from T by 10 with delete; started →
      end.
   4. `Skill3` ≥ 0 → `Skill3` at T. End.
3. S, E := secondary target and distance (`0x005DDC30`, second argument
   0). S and E < aip2 [25]:
   1. `Skill6` > 0, frame > param 0 and draw < aip4 [50] → `Skill6` at
      T; param 0 := frame + aip3. End.
   2. Draw < aip5 [80]: `Skill3` ≥ 0 and draw < aip6 [30] → `Skill3` at
      S, end; `Skill1` ≥ 0 and shot allowed → `Skill1` at S, end.
4. D > aip8 [9] and draw < aip7 [30] → wander near T 6 (`0x005DF530(T,
   6)`). End.
5. Draw ≥ 70 → idle 10; else circle 3 at T (no delete).

1.14d-confirmed.

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| vile birth | child = row 301 chain by byte +0x4B; count radius 25; 8 offsets of 3 tiles; footprint class 298, size 2, mask 0x3C01 | `0x005FA010`, `0x005F9E30` |
| direction tables | 64→8 `0x00745600`; dx `0x006EA998`, dy `0x006EA978` | file |
| finger mage | flee 9, then 14; curse state 84 | `0x005FA380` |
| regurgitator | corpse `soft`, radius aip6; approach ≤ 6 steps; eat at squared ≤ 4 | `0x005FA710`, `0x005FA690` |
| Diablo | cull 1020; home command 10; weights §7.3; skill 199; level 108; idles 12 / 8 / 4 | `0x005E9170`, `0x005E8810` |
| boss score | stats 22, 36, 37, 39, 41, 43, 49, 51, 53, 55, 58; state 11; `attackrank`; `threat` < 2 → 0 | `0x005E8530` |
| Izual | nova range 10; walk radius (6, 9) | `0x005F89B0` |
| knights | component byte +0x0E < 4; knight `BaseId` 310; scan radius² 2500 | `0x005FAB80`, `0x005FAF00`, `0x005FAE40` |

## Randomness

All draws are on the thinking monster's unit seed (`rng.md` §7), in the
order of each section; a test that is not reached draws nothing. Forms:
`lo' % 100` (P(…), "draw", `roll(100)`), `lo' % 1000` and the weighted
pick of §7.3 (Diablo), `lo' & 1` (AbyssKnight steps), plus the helpers'
draws (`ai.md` §7.2). The boss pick, the score, the scans and the birth
draw nothing. Skill checks and the quest seam have their own draws.

## Edge cases & original bugs

1. Regurgitator §5 step 6: with no corpse and a draw < 20 the think
   schedules nothing.
2. Regurgitator §5 step 2: a walk then an idle are requested on the
   same think.
3. Diablo §7.1: the slot-9 alternative replaces the best only when no
   path to the best exists; the slot-8 and slot-9 nodes are matched by
   act only (no distance cull).
4. Diablo §7.2 step 3: a left skill whose id is outside the skills
   table makes 1.14d read address 0x18E (crash); not reachable with
   valid skills.
5. Diablo §7.3 step 1: a pending choice (5, the channel) is returned
   without rescoring until the channel ends.
6. OblivionKnight §11 step 2.1 tests `Skill4` > 0 (not ≥ 0) and the
   curse state > 0.

## Test vectors

Synthetic (CI-safe), draws given as `lo' % 100` unless noted:

| AI, input | Draws | Result |
|---|---|---|
| VileDog, param 0 = 1, C = 1, vilechild1 | 85 (≥ 80) | idle 10 |
| DoomKnight, C = 0, doomknight1 | 50 (< 80) | lunge to T (velocity 13, walk flags 7) |
| Megademon, C = 1, frame ≤ param 0, megademon1 | 60 (< 80) | A1 at T |
| Izual, params 0 = 1, 1 = 0, 2 = 2, C = 1 | none | w := 1, A1 at T |
| AbyssKnight, no armor cast, C = 0, D = 10, c = 1, doomknight2 | 50 (< 70), next `lo'` odd | c := 0; velocity (2, 0, 7), walk to T flags 7 |
| Diablo choice, X in melee, clear, life ≥ 20 %, no cold, f39 = f41, not near | Σ = 229, r = `lo' % 229` = 100 | cumulative 40, 110 → k = 3 (A2) |
| Diablo choice, X = 0, no collision | `lo' % 1000` = 0 | k = 4 (S4 at X = 0) |

Game-file vectors: Open question 1.

## Provenance

- 1.14d `Game.exe`: `0x005FA010`, `0x006510C0`, `0x0054DA60`,
  `0x005DCE10`, `0x005F9E30`, `0x00621DC0`, `0x0064FDC0` (entry only),
  `0x0063E7A0`, `0x0063E7E0`, `0x004E6C50`, `0x005FD350` (case 298),
  `0x005FA280`, `0x005FA380`, `0x005FA710`, `0x005FA690`, `0x004638A0`,
  `0x005E0C80`, `0x005E9170` (jump table `0x005E9704`), `0x005E8480`,
  `0x005E8F20`, `0x005E8EB0`, `0x005E8530`, `0x005E8810`, `0x00573930`
  (record field +0xC), `0x006417F0`, `0x005F89B0`, `0x005B4390` (entry
  only), `0x005FAA90`, `0x005FAB80`, `0x005DE190` and `0x005A6260`
  (steps stored as a byte), `0x005FAF00`, `0x005FAE40`. Ghidra
  decompile read first; every call's register and stack arguments and
  every weight store checked in the disassembly (the decompiler's local
  names for the Diablo weight array are off by one slot; the stores at
  ebp −0x60..−0x20 were mapped by hand). Tables `0x00745600`,
  `0x006EA998`, `0x006EA978` read from the file.
- Live data (`patch_d2`): monstats.txt, levels.txt (Act 3 rows),
  superuniques.txt (rows 36–38, 41), skills.txt (ids 27, 51, 56, 59,
  199), states.txt (84 `fingermagecurse`) — counted by the throwaway
  script of `ai-bodies-3.md`.
- D2MOO 1.10f `AiThink.cpp`: names and param meanings; compared for
  every body here: same rules except the Diablo cull distance (55 in
  1.10f, 1020 in 1.14d; D2MOO's `D2_VERSION_HAS_UBERS` branch matches
  1.14d).

## Open questions

1. No recording of any Act IV AI: record a River of Flame / Chaos
   Sanctuary run including Diablo (tick recorder) and compare think
   schedules and draws with §2–§11.
2. Diablo §7.3: confirm the weight table on a recording that logs the
   chosen mode per think against the player's resistances and skills.
3. The skill 199 (DiabPrison) check reads the point (portal GUID, 2) in
   §7 case 15: owner is the skills spec (how the skill resolves it).
4. Who calls the Diablo alternate (`ai.md` §3.3 re-install while
   running).
