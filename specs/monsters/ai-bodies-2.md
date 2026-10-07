# Spec: Monsters — AI think bodies, Act II

- **Status:** draft: the 14 AI functions used by Act II monsters that
  were still unread, one alternate function, and the special-state
  thinks 10/17, 11 (with its init) and 12, read from the 1.14d
  `Game.exe` disassembly (addresses per section; register arguments
  checked in `all.asm`). No recording covers these bodies yet (Open
  questions 1–2).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::monsters::ai` (per-AI functions beside the
  `ai.md` §9 bodies)
- **Related specs:** `monsters/ai.md` (owner of scheduling §1, dispatch
  §2, AI control and tables §3, aip reads §4, target search §5,
  distances §6, tactics helpers §7, commands §8, the conventions of §9);
  `monsters/ai-functions.tsv` (catalogue; rows flipped to `spec'd-here`
  link here); `sim/path-placement.md` (patterns, footprints, free
  points, placement); `combat/damage.md` §7.2 (kill); `sim/stat-lists.md`
  §9 (states); `monsters/init.md` §1 (spawn wrappers),
  `monsters/population.md` §9 (spawn ring); `world/quests.md` (quest
  seams); skills spec (skill add / assign, skill checks).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 53–73 |
| Inputs | 74–82 |
| Outputs / state changes | 83–89 |
| Rules | 90–91 |
|   1. Scope and order | 92–127 |
|   2. PantherJavelin (95) `0x005E1080` | 128–154 |
|   3. GreaterMummy (22) `0x005F2B10` | 155–208 |
|   4. Mummy (21) `0x005F2850` | 209–221 |
|   5. PantherWoman (18) `0x005F22B0` | 222–236 |
|   6. MaggotLarva (38) `0x005F6220` | 237–247 |
|   7. SandLeaper (17) `0x005F20A0` | 248–264 |
|   8. MaggotEgg (40) `0x005F6530` | 265–278 |
|   9. PinHead (39) `0x005F6340` | 279–293 |
|   10. ClawViper (16) `0x005F1B60` | 294–316 |
|   11. Vulture (23) `0x005F3170` | 317–382 |
|   12. BatDemon (29) `0x005F5040`, alternate `0x005F4FD0` | 383–425 |
|   13. SandMaggotQueen (66) `0x005F9CF0` | 426–447 |
|   14. Duriel (44) `0x005F67B0` | 448–466 |
|   15. Summoner (53) `0x005F85C0` | 467–499 |
|   16. Special-state thinks 10/17, 11, 12 | 500–554 |
| Constants & data dependencies | 555–572 |
| Randomness | 573–582 |
| Edge cases & original bugs | 583–597 |
| Test vectors | 598–615 |
| Provenance | 616–643 |
| Open questions | 644–664 |
<!-- /index -->

## Summary

Act II levels (levels.txt `Act` = 1: `mon1`–`mon25`, `nmon*`, `umon*`),
the Act II superuniques (rows 10–19) and the bosses Radament, the
Summoner and Duriel, plus the minions and spawns those rows name
(monstats `minion1`, `minion2`, `spawn`), give 70 monstats rows. Their
AIs not yet specified in `ai.md` §9 are written here, in descending
order of how many of those rows use them (§1). Every body follows the
`ai.md` §9 conventions: T, D, C from the dispatch record, P(aipN) one
unit-seed step tested `lo' % 100 < aipN`, draws listed in order, "idle
N" = `0x005DE080(N)` (neutral, then think at +N). Two more scheduling
helpers appear often here:

- "wait N" = `0x005DE0F0(N)`: delete the unit's thinks and schedule one
  at frame + N; the anim mode is **not** changed (`ai.md` §1.2).
- "mode m at (x, y)" = `0x005DE440(x, y, m)`: build the mode request
  for mode m (`0x005A7E60`), set the path step count 1 (`0x00649070`),
  target point (x, y), no target unit, and request it (`0x005A7C20`).

Bracketed values are the Normal values of the named live row.

## Inputs

| Name | Type | Source |
|---|---|---|
| tick record | `ai.md` §2.1 | T (+0x08), D (+0x14), C (+0x18), monstats (+0x1C) |
| AI control | `ai.md` §3.1 | params 0–2 at +0x14, +0x18, +0x1C |
| monstats | 424 bytes | `aip1`..`aip8` (+86 + 6(N − 1) + 2·difficulty, signed), `Skill1`..`Skill5` (+0x170..+0x178), `Sk1mode`..`Sk5mode` (bytes +0x180..+0x184), `Velocity` (+0x32), `Run` (+0x34), flag bits at +12 |
| game | | +0x6D difficulty, +0xA8 frame |

## Outputs / state changes

Mode requests, think schedules, AI control params, unit flags (+0xC4),
path pattern and move mask (Vulture), stats 74 `hpregen` (BatDemon),
states 90 / 91 (ClawViper), spawned monsters (SandMaggotQueen), skill
list (Duriel), unit-seed draws in the order given.

## Rules

### 1. Scope and order

Rows counted by script over the live `patch_d2` monstats.txt,
levels.txt and superuniques.txt (Provenance). AI index, rows, status
before this spec:

| AI (index) | Act II rows | Rows | § |
|---|---|---|---|
| SkeletonMage (64) | 7 | skmage_cold4, skmage_fire3/4, skmage_ltng3/4, skmage_pois3/4 | `ai.md` §9.19 |
| PantherJavelin (95) | 5 | slinger1, 2, 4, 5, 6 | §2 |
| Scarab (20), Skeleton (2) | 4 each | | `ai.md` §9.29, §9.11 |
| GreaterMummy (22) | 4 | radament, unraveler1–3 | §3 |
| Mummy (21) | 4 | mummy1–4 | §4 |
| PantherWoman (18) | 4 | pantherwoman1–4 | §5 |
| SandRaider (8), SandMaggot (15) | 3 each | | `ai.md` §9.26, §9.28 |
| MaggotLarva (38) | 3 | maggotbaby1–3 (sandmaggot egg spawns) | §6 |
| SandLeaper (17) | 3 | sandleaper1–3 | §7 |
| MaggotEgg (40) | 3 | maggotegg1–3 (sandmaggot `spawn`) | §8 |
| PinHead (39) | 2 | blunderbore1, 2 | §9 |
| ClawViper (16) | 2 | clawviper2, 3 | §10 |
| Vulture (23) | 2 | vulture1, 2 | §11 |
| SkeletonBow (37), Wraith (9), Swarm (19) | 2 each | | `ai.md` |
| BatDemon (29) | 1 | batdemon1 | §12 |
| SandMaggotQueen (66) | 1 | maggotqueen1 (Coldworm) | §13 |
| Duriel (44) | 1 | duriel | §14 |
| Summoner (53) | 1 | summoner | §15 |
| Vampire, BloodHawk, Baboon, Zombie, Goatman, Brute, FoulCrowNest | 1 each | | `ai.md` |

Town NPCs of Act II (Towner 41, Vendor 42, JarJar 81, Npc 32) are not
level monster rows and are not counted. 64 of the 70 rows have the
monstats `switchai` bit, so the special states 10, 11 and 12 apply to
them (`ai.md` §3.2); §16 gives those thinks. Special state 13 is
installed only for the Countess (`monsters/init.md` §20 rule 5); the
installers of states 2, 3, 9, 14 and 16 are skill or summon paths not
reached by these rows as far as read (Open question 3).

### 2. PantherJavelin (95) `0x005E1080`

Brackets: slinger1 [70, 70, 12, 10, 15, 20]. S, E := secondary target
and its distance (`0x005DDC30`, second argument 0; E starts as D and is
overwritten, 0x7FFFFFFF when there is no S).

1. E < 8 and P(aip4) [10] → escape from T by 16 with think delete
   (`0x005DEFE0(T, 16, 1)`); end, started or not.
2. E > aip6 − 6 [14] and P(aip1) [70] → wander near T 4
   (`0x005DF530(T, 4)`). End.
3. No S, or E ≥ aip6 [20]: pack scan (below). A pack mate M found and
   its squared distance > aip3 [12] (**not** squared) → walk to M with
   flags 7 (`0x005DEC80`). Else idle aip5 [15]. End.
4. P(aip2) [70] → A1 at S (the javelin). Else idle aip5.

Draws: step 1's P only when E < 8; step 2's only when E > aip6 − 6; one
in step 4; the helpers' draws (wander 3–4). Same as D2MOO.

**Pack scan** (`0x005DD0B0` scan 1, every unit of the adjacent rooms,
callback `0x005B0D00`, arg {best = 0, best squared distance =
0x7FFFFFFF}). A unit U is a candidate when it is a monster, not the
scanner, has the scanner's monstats `BaseId` (`0x00463860`: monstats +2
of the class; −1 for a missing row), and is not in mode 0 or 12
(`0x0063EA40`). Its squared distance d (`0x005B0BD0`: (Δx)² + (Δy)² of
the two positions) replaces the best when strictly smaller. The scan
never stops early. No draws.

### 3. GreaterMummy (22) `0x005F2B10`

Brackets: unraveler1 [70, 30, 40, 60, 24]; radament [85, 45, 55, 85,
24]. `Skill1` Resurrect2, `Skill2` Bestow, `Skill3` UnHolyBolt (all
`Sk*mode` `seq_mummyres`).

1. C and P(aip1) [70] → A1 at T. End.
2. D < 5 and P(aip1) → A2 at T. End.
3. Scan arg R := {corpse 0, hurt 0, matches 0, seen 0, max squared
   distance aip5² [576], wide 0, normal 0}. If the unit's class is 229
   (radament; −1 when monstats has ≤ 229 rows): quest seam
   `0x00599420(game, unit)` (D2MOO `ACT2Q1_OnRadamentActivated`,
   `world/quests.md`); max := (aip5 + 10)²; wide := 1; normal := 1 on
   Normal difficulty (game +0x6D = 0).
4. Scan 1 with callback `0x005F2A00` (below).
5. `Skill2` ≥ 0 and R.hurt ≠ 0: P(aip3) [40] → set the path target to
   R.hurt (`0x00620C10` → `0x00648B90`), use `Skill2` in `Sk2mode` at
   R.hurt (`0x005DEAD0`, point (0, 0)). End.
6. R.corpse ≠ 0: P(aip2) [30] and the skill check `0x005FD470(Skill1,
   corpse, 0, 0)` ≠ 0 → set the path target to the corpse; sequence
   skill `Skill1` on it (`0x005DE000`, point (0, 0)). End. (The check
   runs only after the draw passes.)
7. `Skill3` ≥ 0: P(aip4) [60] and a secondary target S (`0x005DDC30`,
   second argument 0; searched only after the draw passes) → `Skill3`
   in `Sk3mode` at S. End.
8. R.seen ≤ 0 → velocity request (method unchanged, speed 50, steps 0);
   walk to T with 3 steps (`0x005DEF80`). End.
9. Draw `lo' % 100` ≥ 50 → idle 6; else circle 3 at T (`0x005DF7D0`, no
   delete; one more draw).

**Callback `0x005F2A00`** (scanner M, unit U). U counts when all hold:

1. U is a monster other than M, with the same alignment pairing
   (`0x00650D70(M, U)`: M evil and U evil, or M good and U good; M
   neutral never), U not good (alignment ≠ 2, `0x006259B0`), U unit
   flag 0x2 set (+0xC4), U without a state of flag group 33 `udead`
   (`0x0063A770`, data tables +0x150 = bitset 33, `data/runtime-maps.md`
   §4).
2. Undead: R.wide = 0 → U's monstats `lUndead` bit (byte +0xD & 0x08);
   R.wide ≠ 0 → `lUndead` or `hUndead` (`0x0063E990`, masks 0x08 and
   0x10 at `0x006CE274`, `0x006CE278`).
3. R.normal ≠ 0, or U is not unique (`0x005A0180(U, 8)` = 0).
4. Squared distance M→U ≤ R.max.

Then R.seen += 1, and: U in mode 12 (dead) without a state of flag group
2 `hide` (`0x0063A320`, bitset 2) → R.matches += 1, R.corpse := U.
Otherwise, U's life (stat 6, `0x00625480(U, 6, 0)`) ≠ its max life
(`0x00625D10`) and U not in mode 0 → R.matches += 1, R.hurt := U. The
callback returns 0, so the last match of each kind wins.

So Radament on Normal also raises and heals uniques, over a radius 10
larger, and counts high undead. 1.14d-confirmed; same as D2MOO
(`bRadament`, `bNormal`).

### 4. Mummy (21) `0x005F2850`

Brackets: mummy1 [5, 60, 100, 65, 10].

1. AI state 3/19 (`0x005DD2B0`) and not C → walk to T with flags 7. End.
2. D > aip1 [5]: P(aip2) [60] → wander 3; else idle aip5 [10]. End.
3. Not C → walk to T with flags 7. End.
4. P(aip3) [100] → P(aip4) [65] → A1, else A2, at T. End.
5. P(aip3) failed: idle aip5, **then** walk to T with flags 7 (both
   requests are made; the walk replaces the neutral request).

1.14d-confirmed; same as D2MOO, bug kept (step 5).

### 5. PantherWoman (18) `0x005F22B0`

Brackets: pantherwoman1 [70, 70, 8, 6, 0].

1. C: P(aip2) [70] → A1 at T; else idle aip4 [6]. End.
2. P(aip1) [70] → velocity request (method unchanged, speed 75, steps
   0); lunge (`0x005DED40(T, 7)`: velocity method 13, walk to T, flags
   7). End.
3. Pack scan of §2. M found and its squared distance > aip3² [64] →
   velocity (speed 75); walk to M with flags 7. End.
4. Draw `lo' % 100` < 25 → circle 3 at T (no delete; one more draw);
   else idle aip4.

1.14d-confirmed. Unlike PantherJavelin, aip3 is squared here.

### 6. MaggotLarva (38) `0x005F6220`

Brackets: maggotbaby1 [75, 20, 60, 15]. AI param 0 = "bit last think".

1. Not C: param 0 := 0; P(aip3) [60] → walk to T with flags 1; else
   idle aip4 [15]. End.
2. C and param 0 = 0: P(aip1) [75] → param 0 := 1, A1 at T, end.
3. Param 0 := 0; idle aip2 [20].

So a larva never bites on two thinks running. 1.14d-confirmed.

### 7. SandLeaper (17) `0x005F20A0`

Brackets: sandleaper1 [20, 50, 30, 50]; `Skill1` Leap
(`seq_leaperleap`).

1. `Skill1` ≥ 0, D < 5, P(aip1) [20], and the skill check
   `0x005FD470(Skill1, T, 0, 0)` ≠ 0 → `Skill1` in `Sk1mode` at T
   (point (0, 0)). End.
2. C: P(aip2) [50] → A2 at T; else idle 10. End.
3. D > 10 → velocity request (method unchanged, speed 75, steps 0);
   wander near T 5 (`0x005DF530`). End.
4. P(aip3) [30] → walk to T with flags 7. End.
5. P(aip4) [50] → circle 4 at T (no delete). End.
6. Idle 10.

1.14d-confirmed.

### 8. MaggotEgg (40) `0x005F6530`

Brackets: maggotegg1 [250, 18]; `Skill1` MaggotEgg (`seq_maggotegg`,
the hatch). AI param 0 = "hatched".

1. `Skill1` ≥ 0 and param 0 ≠ 1: P(aip2) [18] → `Skill1` in `Sk1mode`
   at T (point (0, 0)); wait aip1 [250]; param 0 := 1. End.
2. Param 0 = 1 → kill the egg: `0x0057CCB0(unit, K, 1)` with K = the
   unit's path target (`0x00553540`; 0 when that is the unit itself)
   (`combat/damage.md` §7.2).
3. Idle aip1.

1.14d-confirmed; same as D2MOO.

### 9. PinHead (39) `0x005F6340`

Brackets: blunderbore1 [75, 15, 60, 12, 40, 0]; `Skill1` Smite
(`seq_pinheadsmite`). AI param 0 = "attacked last think".

1. Not C: param 0 := 0; P(aip3) [60] → walk to T with flags 7; else
   idle aip4 [12]. End.
2. Param 0 ≠ 0 and P(aip1) [75] fails → idle aip2 [15]. End.
3. Param 0 := 1. `Skill1` ≥ 0 and P(aip5) [40] → `Skill1` in `Sk1mode`
   at T. End.
4. `Skill2` ≥ 0 and P(aip6) → `Skill2` in `Sk2mode` at T. End.
5. A1 at T.

1.14d-confirmed.

### 10. ClawViper (16) `0x005F1B60`

Brackets: clawviper1 [40, 8, 75, 50, 15, 1]; `Skill1` SerpentCharge
(`seq_serpentcharge`). AI param 0 = "has charged". G = aip6 as a glow
selector: 0 none, 2 state 91, any other value state 90 (D2MOO blue /
red glow; same states as SandRaider, `ai.md` §9.26).

1. Param 0 ≠ 0 and G ≠ 0 → state off (`0x00639DB0(unit, state, 0)`).
   Param 0 is never cleared, so this runs every think after the first
   charge.
2. Not C:
   1. `Skill1` ≥ 0 and D < aip2 [8]: P(aip1) [40] and the skill check
      `0x005FD470(Skill1, T, T.x, T.y)` ≠ 0 (T's position by
      `0x0045ADF0` / `0x0045AE20`) → `Skill1` in `Sk1mode` at T (point
      (0, 0)); G ≠ 0 → state on (`0x00639DB0(unit, state, 1)`); param 0
      := 1. End.
   2. Draw `lo' % 100` < 50 → walk to T with flags 7; else idle aip5
      [15]. End.
3. C: P(aip3) [75] fails → idle aip5. End. P(aip4) [50] → A1, else A2,
   at T.

1.14d-confirmed.

### 11. Vulture (23) `0x005F3170`

Brackets: vulture1 [70, 8, 75, 30, 40]. AI params: 0 = flight counter
p (−1 just landed, 0 on the ground, 1 land now, ≥ 2 flying thinks
left), 1 and 2 = flight destination (x, y). O = own position; F :=
squared distance unit→T (`0x005B0BD0`) > 144.

Helpers (positions via the unit's path, `sim/path-placement.md`):

- **Land** `0x005F2FC0` → 1 / 0: unit flags +0xC4 |= 0x0E (set before
  any test, also when it then fails); path pattern (path +0x48,
  `0x00649180`) already 1 → 0; place the unit at O with
  `0x00554EA0(room, O, exact 0, alt 0)` (`path-placement.md` §10),
  failure → 0; pattern stamp `0x0064EA90(room, O, 1, 0x100)`; pattern
  := 1 (`0x00649190`); move mask := 0x3C01 (path +0x50, `0x00648CE0`);
  wait 12; → 1.
- **Take off** `0x005F2EB0`: unit flags +0xC4 &= ~0x0E; clear bit 0x1000
  then bit 0x100 of the one cell at O (`0x0064CBE0`, own room); pattern
  := 5; move mask := 0; wait 12.
- **Touch down**: mode 9 toward the unit itself through `0x005DE4E0(unit,
  9, 2, 3)` (the walk-in-radius core of `ai.md` §7.2 with mode 9; with
  the unit as its own target the point is O); wait 12; p := −1.

Steps:

1. T is never 0 here (target mode 1). (1.14d with T = 0: p < 1 → wait 12;
   else land, and on failure the distance call reads a null unit.)
2. If T's room (`0x00620BB0`) ≠ the unit's room and F: p < 1 → walk in
   radius of T (`0x005DE6D0(T, 9, 0)`), end. Else land: success → touch
   down, end; failure → go on.
3. p = 0, the unit has no minion owner (`0x0058F0D0` = 0), F, and
   `roll(100)` < 60 (drawn only when the first three hold) → take off;
   p := mask(8) + 24 (`0x00472210(seed, 8)`, one step: `lo' & 7`);
   mode 8 toward T via `0x005DE4E0(T, 8, 8, 8)`; wait 12. End.
4. Carrion scan: scan 1, callback `0x005F30F0`, arg {best 0, max 121,
   pct aip3}. A unit U is taken when it is a monster or player, not the
   scanner, not a monster in mode 0 or 12 (`0x0063EA40`; dead players
   pass), with squared distance d ≤ the current max, and life (stat 6)
   ≤ (max life / 100) × aip3 [75] (signed division); then best := U and
   max := d (ties go to the later unit).
5. p ≥ 2 (flying):
   1. A carrion unit was found, or D < 6 and `roll(100)` < 15 (drawn only
      when none was found and D < 6) → p := 1; step 6.
   2. Else take off (again). If params 1, 2 are both nonzero and the
      path distance to them (`0x005DC5C0`, unsigned) > 1: free point
      near (params 1, 2) in T's room (`0x0064E7B0(room of T, &pt, size,
      0xFFFF, 1)`); found → params 1, 2 := pt; else the same search in
      the unit's own room; found → params 1, 2 := pt. Then p −= 1;
      velocity request (7, 0, 0); mode 8 at (params 1, 2); wait 12. End.
   3. Else (no destination, or reached): r = p + 8; x = T.x − r +
      `roll(2r)`, then y = T.y − r + `roll(2r)` (`0x0045C3E0`, two
      draws, x first); m = clamp(2r, 12, 36). While the path distance to
      (x, y) < m: x moves one step away from O.x (x < O.x → x − 1; x >
      O.x → x + 1), y likewise from O.y, and if then (x, y) = O both
      get + 1. Params 1, 2 := (x, y); velocity request (1 → 7, 0, 0);
      mode 8 at (x, y); wait 12; p −= 1. End.
6. p = 1 (also from 5.1): land. Success → touch down, end. Failure → p
   := 8; wait 12; go on to step 7 with the value p had before (≥ 1).
   p < 1 → step 7.
7. C: P(aip1) [70] → A1 at T; else wait aip2 [8]. End.
8. Not C: if p ≠ −1: `roll(100)` ≥ aip5 [40] → wait aip2, end. Then
   `roll(100)` < aip4 [30] → circle 6 at T (no delete; one more draw),
   else walk in radius of T (`0x005DE6D0(T, 9, 0)`). p := 0; wait 12.

Mode 8 is the flight mode, mode 9 the landing. 1.14d-confirmed.

### 12. BatDemon (29) `0x005F5040`, alternate `0x005F4FD0`

Brackets: batdemon1 [33, 20, 60, 50, 8]. AI params: 0 = state s (0
start, 1 flying, 2 hovering, 3 on the ground, 4 take off again), 1 =
flight think counter c, 2 = `hpregen` bonus h. L = own life percent
(`0x00621F20`). Stat 74 `hpregen` is read with `0x00625480(unit, 74, 0)`
and written with `0x00627260(unit, 74, v, 0)`.

1. s = 0 or 4: mode 10 with no target (`0x005DDF90(10, 0)`); wait 8; s
   := 1; c := 0; R = stat 74; R ≠ 0 → h := aip5 × R / 8 (signed,
   rounding toward 0), stat 74 := R + h; R = 0 → h := 0. End.
2. s = 1:
   1. c > 1 and (C, or D < 7, or AI state 3/19, or (D < 14 and L >
      50)): h ≠ 0 → stat 74 := stat 74 − h; mode 9 with no target; wait
      6; s := 3. End.
   2. Otherwise: mode 11 with no target; wait 10 if c = 0, else wait
      15; c += 1. End.
3. s = 2:
   1. L < aip1 [33] and escape from T by 15 (no delete) started → s :=
      4. End.
   2. `roll(100)` < 33 → walk to T (flags 0), s := 3. End.
   3. `roll(100)` < 15 → wander 6. End.
   4. Idle 10.
4. s = 3:
   1. L < aip1 and escape from T by 15 started → s := 4. End.
   2. Not C → walk to T (flags 0). End.
   3. c > 0 → A2 at T, c := 0. End.
   4. AI state 3/19, `roll(100)` < aip2 [20] and escape from T by 12 (no
      delete) started → s := 2. End.
   5. `roll(100)` < aip3 [60] → `roll(100)` < aip4 [50] → A2, else A1,
      at T. End.
   6. Idle 10.
5. Any other s: D < 15 → walk to T (flags 0), s := 3; else s := 4, idle
   15.

Each `roll(100)` is drawn only when the tests before it in its line
hold. **Alternate** (the think after a re-install over the running AI,
`ai.md` §3.3): anim mode 11 → s := 1, wait 1. Else s = 4 → s := 1,
mode 10 with no target. Else re-install the AI for the control's
current state (`0x005B0E00`; since the control now holds the alternate,
this is the full reset of `ai.md` §3.3 step 4) and idle 1. No draws.
1.14d-confirmed (both).

### 13. SandMaggotQueen (66) `0x005F9CF0`

Target mode 0. Brackets: maggotqueen1 [7, 12]. AI params: 0 = spawned
count, 1 = "laying", 2 = "resting".

1. Param 2 ≠ 0 → idle aip2 × 25 [300]; param 2 := 0. End.
2. Param 1 = 0: param 0 < aip1 [7] → mode 8 at the unit's own position;
   wait aip2 [12]; param 1 := 1; end. Otherwise return with **nothing
   scheduled** (bug kept: the queen stops thinking until something else
   schedules a think).
3. Param 1 ≠ 0: spawn info `0x0063EFA0(unit, &class, &x, &y, &mode, 0,
   0x0058F710)`; for base class 284 it gives class
   `0x0054DA60(68 sandmaggot1, …)` (Open question 4), (x, y) = (own x +
   8, own y), mode 8. Room at (x, y) from the unit's room
   (`0x00463740`); found → spawn `0x005B2F20(room, x, y, class, mode,
   spread 2, flags 0x42)` (`monsters/init.md` §1); spawned → its unit
   flags +0xC4 |= 0x04000000 (D2MOO `UNITFLAG_NOXP`), param 0 += 1.
   Wait aip2; param 1 := 0; param 2 := 1.

No draws in the AI itself (the spawn's own draws: `monsters/
population.md` §9). 1.14d-confirmed; same as D2MOO.

### 14. Duriel (44) `0x005F67B0`

Brackets: duriel [5, 33, 50, 0, 0]; `Skill1` Charge, `Skill2` Jab,
`Skill3` Smite, `Skill4` Holy Freeze (+0x176).

1. `Skill4` ≥ 0 and the unit has no right skill (`0x006201D0`): add
   `Skill4` at level aip1 [5] (`0x0056DEB0(unit, Skill4, aip1, 1)`) and
   make it the right skill (`0x005701B0`, EDX = 0, skill, owner −1)
   (skills spec). The aura.
2. Not C: `Skill1` ≥ 0 and P(aip5) [0] → `Skill1` in `Sk1mode` at T,
   end. Else velocity request (method 13, 0, 0); walk to T with flags
   7. End.
3. C: `Skill3` ≥ 0 and P(aip2) [33] → `Skill3` in `Sk3mode` at T, end.
   `Skill2` ≥ 0 and P(aip3) [50] → `Skill2` in `Sk2mode` at T, end.
   P(aip4) [0] → A2, else A1, at T.

Live data: aip5 = 0 on every difficulty, so the charge never fires but
its draw is still made each non-combat think. 1.14d-confirmed.

### 15. Summoner (53) `0x005F85C0`

Brackets: summoner [85, 5, 63, 40, 120, 33, 5, 40]; `Skill1` Glacial
Spike, `Skill2` Frost Nova, `Skill3` Fire Ball, `Skill4`
VampireFirewall, `Skill5` Weaken (+0x178, `Sk5mode` +0x184), all `SC`.
AI params: 0 = activated, 1 = frame before which no nova, 2 = frame
before which no firewall. Every skill below is used with
`0x005DEAD0(mode, skill, unit, X, Y)` where (X, Y) = T's position
(read in step 3), also when the unit is S.

1. Param 0 = 0 → quest seam `0x0059C330(game, unit)` (D2MOO
   `ACT2Q5_OnSummonerActivated`); param 0 := 1.
2. D < 5 and P(aip6) [33] → escape from T by 6 (no delete); the result
   is ignored and the think goes on.
3. K := T's stat 39 `fireresist` ≥ T's stat 43 `coldresist`
   (`0x00625480(T, s, 0)`; "use cold"). (X, Y) := T's position.
4. P(aip1) [85] fails → wander 4. End.
5. `Skill5` ≥ 0 and P(aip2) [5] → `Skill5` at T. End.
6. S := secondary target (`0x005DDC30`, second argument 0; its distance
   is not used). Draw `lo' % 100` > aip3 [63] → K := not K.
7. K:
   1. `Skill2` ≥ 0, frame > param 1 and D < aip7 [5] → param 1 := frame
      + aip4 [40]; `Skill2` at T. End.
   2. `Skill1` ≥ 0, S and D < aip8 [40] → `Skill1` at S. End.
8. `Skill4` ≥ 0 and frame > param 2 → param 2 := frame + aip5 [120];
   `Skill4` at T. End.
9. `Skill3` ≥ 0, S and D < aip8 → `Skill3` at S. End.
10. Not K and the test of 7.1 holds → as 7.1. End.
11. `Skill5` ≥ 0 → `Skill5` at T. End.
12. Wander 4.

Frame = game +0xA8; "frame > param" is signed. 1.14d-confirmed.

### 16. Special-state thinks 10/17, 11, 12

Installed by curses and Taunt on monsters with `switchai` (`ai.md`
§3.2): the skill-state install `0x005C34B0` maps the skill's state
(skills +0x82) 23 → special state 10, 27 → 12, 56 → 11, any other → 0;
`0x005DDD00` installs 11 and `0x005D6520` installs 10 directly.

**10 / 17 `0x005E8020`** (dim vision; target mode 1 for 10, 2 for 17):

1. C, the class lacks the monstats `interact` bit (byte +0xD & 0x02 at
   `0x006CE26C`; a missing row counts as lacking it) and has mode A1
   (`0x0046C140(class, 4)`) → A1 at T. End.
2. Draw `lo' % 100` < 20 → wander 3; else idle 10.

**11 init `0x005E80E0`** (terror): P = the unit's path target
(`0x00553540`). P a player → target override (`0x00573090(unit, 1,
P's GUID)`; `ai.md` §5.1); a monster → (unit, 2, GUID); a missile (type
3) → (unit, 4, GUID); none or another type → nothing.

**11 think `0x005E8140`**: v = `Run` × 100 / `Velocity` (signed,
truncating) − 100, clamped to 0..120, and 0 when `Velocity` ≤ 0 or the
quotient < 100.

1. Unit without state 56 (`0x00639DF0`) → install special state 0
   (`0x005B0E00`), delete thinks, add a think at frame + 1
   (`0x005417D0`). End.
2. R = AI param 0, 0 → 30. D > R → idle 10. End.
3. Param 2 = 0: param 2 := 1; clear monster data +0x34 / +0x38
   (`0x00573120`, `ai.md` §1.6); delete thinks; velocity request
   (method 2, speed v, steps 0); class has mode 15 (`0x0046C140(class,
   15)`) → run away from T by 30 (`0x005DF140`, no delete), else escape
   from T by 30 (`0x005DEFE0`, no delete). End.
4. C, class without `interact` (`0x00457490(class, 9)`) and with mode 4
   → A1 at T. End.
5. Velocity (2, v, 0); run (mode 15 classes) or walk away from T by 30
   with think delete; not started → wander 6.

**12 think `0x005E8340`** (taunt, target mode 0). P = the unit's path
target (`0x00553540`).

1. P exists and P's room is not in town (`0x0061AB00`):
   1. AI param 0 = 0 → param 0 := 1; lunge to P (`0x005DED40(P, 7)`).
      End.
   2. Unit in melee range of P (`0x00622C40(unit, P, 0)`) → A1 at P.
      End.
   3. X := the target-mode-1 finder `0x005DE890` (`ai.md` §2.3; when it
      finds nothing it has already wandered or idled). X and its combat
      flag → param 0 := 0, A1 at X. End.
   4. Param 0 := 1; path target := P (`0x00620C10`); lunge to P. End.
2. Else: install special state 0, delete thinks, add a think at frame +
   1.

No draws in 11 init, 11 or 12 except the helpers' (wander) and the
finder's. 1.14d-confirmed (all four).

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| pack scan | scan 1, callback `0x005B0D00`, squared distance, same `BaseId` | §2, §5 |
| mummy scan | scan 1, callback `0x005F2A00`, radius aip5 (Radament aip5 + 10) | §3 |
| carrion scan | scan 1, callback `0x005F30F0`, start 121, life ≤ aip3 % | §11 |
| vulture far | squared distance > 144 | `0x005F3170` |
| vulture flight | p = mask(8) + 24; radius r = p + 8; min distance clamp(2r, 12, 36) | `0x005F3170` |
| land / take off | flags 0x0E; pattern 1 / 5; move mask 0x3C01 / 0; stamp mask 0x100; cleared bits 0x1000, 0x100 | `0x005F2FC0`, `0x005F2EB0` |
| state masks | data tables +0x150 bitset 33 `udead`, +0xD4 bitset 2 `hide` | `0x0063A770`, `0x0063A320` |
| monstats flag bytes | +0xD & 0x02 `interact`, & 0x08 `lUndead`, & 0x10 `hUndead` | `0x006CE26C`–`0x006CE278` |
| ClawViper glow | aip6: 2 → state 91, other nonzero → 90 | `0x005F1B60` |
| queen spawn | (x + 8, y), mode 8, spread 2, flags 0x42, unit flag 0x04000000 | `0x005F9CF0`, `0x0063EFA0` |
| terror | default range 30, flee 30, speed bonus ≤ 120 | `0x005E8140` |
| special-state map | skill state 23 → 10, 27 → 12, 56 → 11 | `0x005C34B0` |
| monstats | `aip1..8`, `Skill1..5` +0x170..+0x178, `Sk1..5mode` +0x180..+0x184, `Velocity` +0x32, `Run` +0x34 | `data/fields.tsv` |

## Randomness

All draws are on the thinking monster's unit seed (`rng.md` §7), in the
order of each section; a test that is not reached draws nothing. Forms:
`lo' % 100` (P(…), `roll(100)` `0x0045C390`), `roll(2r)` (`0x0045C3E0`,
Vulture), `mask(8)` (`0x00472210`, Vulture), plus the helpers' draws
(`ai.md` §7.2: wander 3–4, circle 1). The callbacks and scans draw
nothing. Skill checks (`0x005FD470`), the spawn in §13 and the quest
seams have their own draws, owned by their specs.

## Edge cases & original bugs

1. Mummy §4 step 5: idle then walk on the same think.
2. SandMaggotQueen §13 step 2: at the spawn cap with param 1 = 0 the
   function schedules nothing.
3. Vulture land helper sets unit flags 0x0E before its tests; a failed
   landing leaves a flying vulture targetable (§11).
4. ClawViper §10 step 1: param 0 is never reset, so the glow state is
   cleared on every think after the first charge.
5. Duriel §14: aip5 = 0 makes the charge branch draw without firing.
6. Summoner §15 step 2: an escape is requested and then normally
   replaced by a later mode request of the same think.
7. PantherJavelin §2 compares a **squared** distance with an unsquared
   aip3 (12): pack mates closer than √12 tiles are "near".

## Test vectors

Synthetic (CI-safe), unit seed draws given as the `lo' % 100` values:

| AI, input | Draws | Result |
|---|---|---|
| Mummy, mummy1 Normal, D = 4, C = 1 | 40 (aip3 100 passes), 70 (≥ 65) | A2 at T |
| Mummy, D = 4, C = 1, aip3 = 0 | 10 | idle 10, then walk to T flags 7 |
| Mummy, D = 9, C = 0 | 75 (≥ 60) | idle 10 |
| MaggotLarva, C = 1, param 0 = 1 | none | param 0 := 0, idle 20 |
| PinHead, C = 1, param 0 = 0, blunderbore1 | 39 (< 40) | param 0 := 1, Smite at T |
| PantherWoman, C = 0, pack mate at squared distance 64 | 80 (≥ 70), circle draw 30 (≥ 25) | idle 6 (64 is not > 64) |
| PantherJavelin, S at E = 25, no pack mate | 50 (< 70, E > 14) | wander near T 4 (helper draws follow) |
| Duriel, C = 0, Skill1 ≥ 0 | 0 (not < 0) | velocity 13, walk to T flags 7 |
| Summoner, D = 3, frame 100, params 1, 2 = 0, K = 1 | 50 (≥ 33: no escape), 10 (< 85), 7 (≥ 5), S search, 20 (≤ 63) | param 1 := 140, Frost Nova at T |

Game-file vectors: Open question 1.

## Provenance

- 1.14d `Game.exe`: `0x005E1080`, `0x005B0D00`, `0x005B0BD0`,
  `0x00463860`, `0x0063EA40`, `0x005F2B10`, `0x005F2A00`, `0x00650D70`,
  `0x0063E990`, `0x0063A770`, `0x0063A320`, `0x00620C10`, `0x00599420`
  (entry only), `0x005F2850`, `0x005F22B0`, `0x005F6220`, `0x005F20A0`,
  `0x005F6530`, `0x005F6340`, `0x005F1B60`, `0x0045ADF0`, `0x0045AE20`,
  `0x005F3170`, `0x005F2FC0`, `0x005F2EB0`, `0x005F30F0`, `0x005DE4E0`,
  `0x005DE440`, `0x005DE6D0`, `0x00649180`, `0x00648CE0`, `0x0064CBE0`,
  `0x005F5040`, `0x005F4FD0`, `0x005F9CF0`, `0x0063EFA0` (case 284
  only), `0x005F67B0`, `0x005F85C0`, `0x005E8020`, `0x005E80E0`,
  `0x005E8140`, `0x005E8340`, `0x00457490`, `0x005DE890`, `0x005C34B0`,
  callers of `0x005B0E00` (33 sites, state pushes read in `all.asm`);
  `0x005A49B0` for the Countess-only state 13. Ghidra decompile read
  first, every call's register and stack arguments checked in the
  disassembly (`tools/ghidra/disasm.py`); data words `0x006CE26C`,
  `0x006CE274`, `0x006CE278` read from the file.
- Data-tables offsets +0xD4 / +0x150 as flag bitsets 2 / 33: the
  bitset layout of `data/runtime-maps.md` §4 (pointer k at +0xCC + 4k,
  pgsv list after it at +0x16C, as `skills/bodies.md` uses).
- Live data (`patch_d2`): monstats.txt (`AI`, `aip*`, skills,
  `switchai`, `minion1`, `spawn`), levels.txt (Act 1 rows, `mon*`,
  `nmon*`, `umon*`), superuniques.txt (rows 10–19) — counted by a
  throwaway script (scratch, not committed).
- D2MOO 1.10f `AiThink.cpp`: names and param meanings; compared for
  PantherJavelin, Mummy, GreaterMummy, MaggotEgg, SandMaggotQueen and
  Duriel step 1 (same); the others are 1.14d reads only.

## Open questions

1. No recording of any Act II AI: record a Far Oasis / Lost City /
   Arcane Sanctuary run (tick recorder) and compare think schedules and
   draws with §2–§15.
2. Vulture §11 and BatDemon §12 flight: confirm modes 8 / 9 / 10 / 11
   and the land / take-off footprints with a recording that logs mode
   changes and collision.
3. Installers of special states 2, 3, 9, 14, 16 (`0x0056D940`,
   `0x005C07A0`, `0x005D18E0`, `0x005D19D0`, `0x005EF320`, …): confirm
   none applies to the Act II rows; their thinks stay unread.
4. `0x0054DA60` and `0x006510C0` in the spawn-info function
   `0x0063EFA0` (which sandmaggot row the queen spawns): owner is
   `monsters/population.md`; read them.
5. Who calls the BatDemon alternate (`ai.md` §3.3 re-install while
   running): the skill or event path that re-installs AI 29.
6. Answered (`docs/handoff/impl-ai-acts2-5.md` reading 1): Vulture
   with T = 0 is unreachable (target mode 1, `ai.md` §2.3); the 1.14d
   null read in §11 step 1 is not a rule. An implementation asserts T
   ≠ 0 rather than stopping.
