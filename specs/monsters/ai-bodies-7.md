# Spec: Monsters — AI think bodies: one-row AIs (traps, quest NPCs, summons, Ubers)

- **Status:** draft: the AI functions left `unread` after
  `ai-bodies-6.md` that one live monstats row uses (and
  ShadowMasterNoInit, which none uses), in AI-index order, with their
  init and alternate functions, read from the 1.14d `Game.exe`
  disassembly (addresses per section; register arguments checked in the
  disassembly). No recording covers these bodies yet (Open question 1).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::monsters::ai` (per-AI functions beside the
  `ai.md` §9 bodies)
- **Related specs:** `monsters/ai.md` (scheduling §1, dispatch §2,
  control and tables §3, aips §4, target search §5, distances §6,
  tactics helpers §7, skill check §7.4, commands §8, conventions §9, Npc
  helpers §9.9); `monsters/ai-bodies-6.md` (conventions of its Summary;
  pet helpers §2: wander', search capped, pet move, pet follow, run-del;
  the corpse search §24.3); `monsters/ai-bodies-4.md` §7 (boss pick and
  score) and `ai-bodies-5.md` §21 (Baal); `monsters/ai-functions.tsv`
  (catalogue); `monsters/population.md` (footprints §9.3, free spot,
  class for level §11.6); `monsters/init.md` §1 (spawn);
  `world/quests-act2.md`, `-act3.md`, `-act4.md` (quest seams);
  `skills/bodies.md` (summon skills).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 51–60 |
| Inputs | 61–65 |
| Outputs / state changes | 66–71 |
| Rules | 72–73 |
|   1. Scope and order | 74–92 |
|   2. Sarcophagus (45) `0x005F6A10`, init `0x005F6630` | 93–108 |
|   3. FlyingScimitar (47) `0x005F6CA0` | 109–119 |
|   4. GargoyleTrap (63) `0x005F9490` | 120–141 |
|   5. Trap-RightArrow (78) `0x005FB6C0`, Trap-LeftArrow (79) `0x005FB7E0` | 142–162 |
|   6. Trap-Poison (80) `0x005FB900`, Trap-Nova (92) `0x005FB9B0` | 163–176 |
|   7. JarJar (81) `0x005E7590` | 177–210 |
|   8. InvisoSpawner (82) `0x005E0160` | 211–228 |
|   9. BoneWall (84) `0x005E0400`, init `0x005E0390` | 229–235 |
|   10. Trap-Melee (87) `0x005FBA60` | 236–240 |
|   11. 7TIllusion (88) `0x005EA080` | 241–249 |
|   12. DarkWanderer (91) `0x005EA130` | 250–266 |
| Constants & data dependencies | 267–279 |
| Randomness | 280–287 |
| Edge cases & original bugs | 288–296 |
| Test vectors | 297–310 |
| Provenance | 311–324 |
| Open questions | 325–330 |
<!-- /index -->

## Summary

The conventions are those of `ai-bodies-6.md` (Summary): T, D, C from
the dispatch record; "draw" = one unit-seed step read as `lo' % 100`;
P(aipN) = a draw `< aipN`; "idle N" = `0x005DE080(N)`; "wait N" =
`0x005DE0F0(N)`; A1 / A2 = mode 4 / 5 at T; "Skill k at X" =
`0x005DEAD0(Sk<k>mode, Skill<k>, X, 0, 0)`; "mode m at (x, y)" =
`0x005DDFC0(m, x, y)` (`ai.md` §7.1); "death" = mode 0 at (0, 0).
Bracketed values are the Normal values of the named live row.

## Inputs

As `ai-bodies-6.md` (tick record, AI control params 0–2 at +0x14,
+0x18, +0x1C, monstats aips and skills, game frame and difficulty).

## Outputs / state changes

Mode requests, think schedules, AI control params, unit flags, path
direction (GargoyleTrap), spawned monsters (spawners), quest seam
calls, unit-seed draws in the order given.

## Rules

### 1. Scope and order

One live monstats row each (by script over `patch_d2` monstats.txt;
equal to `ai-functions.tsv` `monstats_rows`), by AI index:

| AI (index) | Row | § |
|---|---|---|
| Sarcophagus (45) | sarcophagus | §2 |
| FlyingScimitar (47) | flyingscimitar | §3 |
| GargoyleTrap (63) | gargoyletrap | §4 |
| Trap-RightArrow (78), Trap-LeftArrow (79) | trap-horzmissile, trap-vertmissile | §5 |
| Trap-Poison (80), Trap-Nova (92) | trap-poisoncloud, trap-nova | §6 |
| JarJar (81) | act2guard2 | §7 |
| InvisoSpawner (82) | invisospawner | §8 |
| BoneWall (84) | bonewall | §9 |
| Trap-Melee (87) | trap-melee | §10 |
| 7TIllusion (88) | seventombs | §11 |
| DarkWanderer (91) | darkwanderer | §12 |

### 2. Sarcophagus (45) `0x005F6A10`, init `0x005F6630`

Brackets: sarcophagus [125, –, 16]; `Skill1` Nest (`seq_mummyspawn`).
Init (`ai.md` §9.17): param 0 := frame, param 1 := 0 (summons made n).

1. D > 25 → idle 25. End.
2. n > aip3 [16; 20, 24] → unit flag 0x20000 (no drop); death. End.
3. `Skill1` ≥ 0, |frame − param 0| ≥ aip1 [125; 110, 90] and the
   footprint test `0x005FD350(228 sarcophagus (−1 when monstats is too
   short), room, x, y, 0)` passes (`monsters/population.md` §9.3: point
   (x, y + 2)) → n += 1; param 0 := frame; `Skill1` at T. End.
4. Idle `lo' % 10` + 20 (one step).

As FoulCrowNest (`ai.md` §9.17) with a range gate of 25 and the cap
test `>` instead of `≥`. 1.14d-confirmed.

### 3. FlyingScimitar (47) `0x005F6CA0`

Brackets: flyingscimitar [90, 90, 8, 40].

1. Not C: draw < aip1 [90] → walk in radius of T (`0x005DE6D0(T, 8,
   1)`). End. Draw < aip1 → walk to T with flags 7. End. Idle aip3 [8].
2. C: draw < aip2 [90; 95, 99] → A1 at T. End. Draw < aip4 [40; 50,
   60] → circle 2 at T (`0x005DF7D0(T, 2, 0)`). End. Idle aip3.

1.14d-confirmed (the second not-C test reads aip1 again).

### 4. GargoyleTrap (63) `0x005F9490`

Brackets: gargoyletrap [24, 20, 12, 15]; `Skill1` GargoyleTrap
(`seq_gargoyletrap`). AI param 0 = pending stall s.

1. s > 0 → idle s; s := 0. End.
2. Path target unit := T (`0x00620C10`).
3. (tx, ty) := T's position, (ux, uy) := the unit's; dx := tx − ux, dy
   := ty − uy. P := (ux, ty) when |dx| < |dy|, else (tx, uy).
4. e := the 64-step direction from the unit to P (`0x00621DC0`); the
   path direction is snapped (`0x006488A0`, `sim/pathing.md`) to
   {31, 49, 0, 17}[q] (table `0x006E353C`) with q := table
   `0x006E3540`[e]: e 0–8 → 0, 9–24 → 1, 25–40 → 2, 41–56 → 3, 57–63 →
   0.
5. |dx| > 5 and |dy| > 5 → idle aip4 [15; 7, 0]. End.
6. `Skill1` ≥ 0, D < aip1 [24; 26, 28] and draw < aip2 [20; 50, 80] →
   `Skill1` at T; s := aip3 [12; 10, 8]. End.
7. Idle aip4.

The gargoyle faces one of four directions and fires only along its row
or column (within 5 of T on one axis). 1.14d-confirmed.

### 5. Trap-RightArrow (78) `0x005FB6C0`, Trap-LeftArrow (79) `0x005FB7E0`

Brackets: trap-horzmissile [5, 15, 30, …]; `MissA1` trap spike,
`Skill1` PoisonBallTrap. AI params: 0 = next frame f, 2 = skill frame
(written, never read). The two functions differ only in the axis: 78
compares x, 79 compares y.

1. D < aip1 [5] or D > aip2 [15] → idle 40. End.
2. |own x − T.x| < 3 (79: |own y − T.y| < 3) and frame > f: f := frame
   + aip3 [30; 23, 18]; k := the level's trap kind (below); k ≠ 1 → A1
   at T (the spike missile); end. k = 1 and `Skill1` ≥ 0 → param 2 :=
   frame + aip4; `Skill1` at T; end.
3. Idle 30.

**Trap kind** `0x005FB650(game, unit)`: the monster region of the unit's
level (`0x00547BB0(game +0xF0, level)`, `monsters/population.md` §2.2)
caches the kind at +0x2D4 (−1 = unset). Unset: level id < 40 (Act I) →
0; else `range(0, 3)` (`0x004BC500`: one step on the trap's seed,
`roll(3)`). Stored and returned; later traps of the level reuse it.
1.14d-confirmed.

### 6. Trap-Poison (80) `0x005FB900`, Trap-Nova (92) `0x005FB9B0`

Two copies of one body. Brackets: trap-poisoncloud [20, 1, 15]
(`Skill1` PrimePoisonNova); trap-nova (`Skill1` Trap Nova). AI params:
0 = shots n, 1 = toggle.

1. T ≠ 0, D ≤ aip1 [20] and n < aip2 [1]: `Skill1` ≥ 0 and toggle = 0
   → `Skill1` at T, n += 1, toggle := 1; else toggle := 0, idle aip3
   [15]. End.
2. Otherwise unit flag 0x20000 (no drop); death.

As Trap-Missile (`ai-bodies-6.md` §22) with `Skill1` instead of A1.
1.14d-confirmed.

### 7. JarJar (81) `0x005E7590`

Target mode 0. Row act2guard2 (the guard at Jerhyn's palace door). H
:= the command of type 10 (`0x0058EEF0(10, 0)`; the home point of
`ai.md` §9.9 step 1, params 1, 2 = x, y, param 4 = last walk frame w).
Quest seams of `world/quests-act2.md` (palace guard row of its seam
table).

1. Home `0x005E6800` ≠ 0 → end.
2. Seam `0x0059B8B0(game)` = 0 (the guard keeps the door):
   1. (hx, hy) := H's params 1, 2. stay := 1; seam `0x0059B8F0(game,
      &(hx, hy))` ≠ 0 → stay := 0 (the seam may move the point).
   2. w < frame and frame − w < 200 → idle 20. End.
   3. Path distance to (hx, hy) > 1 (unsigned) → walk to (hx, hy)
      (`0x005DED90`); w := frame. End.
   4. stay = 0 → idle 20. End.
   5. Interaction handler `0x005E68F0` (`ai.md` §9.9) ≠ 0 → w := frame.
      End. Else idle 20.
3. Seam ≠ 0 (the door is open): H = 0 → idle 120. End.
   1. P := the nearest interacting player within 15 (`0x005DDF20`); d
      := full-size distance unit→P.
   2. Seam `0x0059AEC0(game)` ≠ 0 and the path distance to (H.x, H.y −
      3) > 2 → walk to (H.x, H.y − 3). End. Path distance to it > 7 →
      walk there. End.
   3. Interaction handler ≠ 0, or P = 0, or P = the unit → idle 120
      (also after a handler that scheduled). End.
   4. P a player: d < 12 → `roll(1000)` < 100 → wander near P 2
      (`0x005DF530(P, 2)`); then idle 200. End. d ≥ 12 → idle 20, then
      step 3.5.
   5. `roll(1000)` < 50 → wander 3. Then idle 120.

The last request of a think wins (step 3.4 with d ≥ 12 makes up to
three). 1.14d-confirmed.

### 8. InvisoSpawner (82) `0x005E0160`

Brackets: invisospawner [4, 5, 125]. No init. Params: 0 = next frame
f, 1 = spawns left n.

1. f = 0 → n := aip1 [4] (until the first spawn).
2. D > aip2 [5] → idle 15. End.
3. n < 1 → death. End.
4. frame ≥ f: class := class for level (`0x0063EC70(the unit's room,
   96 mummy1)`, `monsters/population.md` §11.6; 96 → −1 when monstats
   is too short); free spot `0x0054DC40(game, room, 0, class, &x, &y,
   0)` (`ai.md` open question 4); found → spawn `0x005B2F20(game, the
   unit's room, x, y, class, mode 1, 2, 0x42)` (`monsters/init.md` §1);
   spawned → n −= 1, f := frame + aip3 [125].
5. Idle 15.

1.14d-confirmed.

### 9. BoneWall (84) `0x005E0400`, init `0x005E0390`

Row bonewall (the Necromancer's wall); `Skill1` Bone Wall. Init:
`Skill1` > 0 → param 0 := frame + the skill's `Param2` (skills +0x14C),
or frame when its row is missing. Think: frame > param 0 → death; else
idle 15. 1.14d-confirmed.

### 10. Trap-Melee (87) `0x005FBA60`

Brackets: trap-melee [70, 15]. C = 0 → idle 40. Draw < aip1 → A1 at T;
else idle aip2. 1.14d-confirmed.

### 11. 7TIllusion (88) `0x005EA080`

Row seventombs (`MissA1` chainlightning). AI param 1 = fired.

1. Param 1 = 1 → unit flag 0x20000; death. End.
2. Mode 4 at (own x − 10, own y); param 1 := 1.

Fires one A1 westward, then dies on the next think. 1.14d-confirmed.

### 12. DarkWanderer (91) `0x005EA130`

Row darkwanderer (Act III intro). Seams of `world/quests-act3.md` §9.2.
AI params: 0 = phase p, 1 = retries n.

1. Walk target `0x005BD0D0(game, unit, &x, &y)` = 0 → idle 10. End.
2. p = 0 → p := 1, n := 0, param 2 := 0.
3. T = 0 or D ≥ 20 → idle 40. End.
4. p = 1 → walk to (x, y); p := 2. End.
5. p = 2: path distance to (x, y) < 2 → unit flag 0x20000; death; seam
   `0x005BD4A0(game, unit)` (the minion hook). End. n < 3 → n += 1;
   velocity (1 → 7, 0, 0); walk to (x, y). End. Else flag, death,
   `0x005BD4A0`. End.
6. Any other p → idle 40.

No draws. 1.14d-confirmed.

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| sarcophagus | range 25; footprint class 228 at (x, y + 2); idle 20–29 | `0x005F6A10` |
| gargoyle | axis projection; facing table {31, 49, 0, 17}; axis window 5 | `0x005F9490`, `0x006E353C`, `0x006E3540` |
| arrow traps | window aip1..aip2; axis distance < 3; trap kind per level (Act I 0, else `roll(3)`) at monster region +0x2D4 | `0x005FB6C0`, `0x005FB7E0`, `0x005FB650` |
| palace guard | re-walk gap 200 frames; door point (x, y − 3), 2 / 7; `roll(1000)` < 100 / 50 | `0x005E7590` |
| inviso spawner | class for level of mummy1 (96) | `0x005E0160` |
| bone wall | life `Param2` frames | `0x005E0390` |
| seven tombs | A1 at (x − 10, y) | `0x005EA080` |
| dark wanderer | range 20; 3 retries | `0x005EA130` |

## Randomness

All draws are on the thinking unit's seed (`rng.md` §7), in the order
of each section; a test that is not reached draws nothing. Forms: `lo'
% 100`, `lo' % 10` (Sarcophagus idle), `roll(1000)` (JarJar), `roll(3)`
(the trap kind, once per level), plus the helpers' (`ai.md` §7.2).
Spawns, free spots and quest seams have their own draws.

## Edge cases & original bugs

1. FlyingScimitar §3 reads aip1 for both not-C tests.
2. The arrow traps write AI param 2 and never read it.
3. JarJar §7 step 3.3 idles 120 after an interaction handler that
   already scheduled; step 3.4 can request three things in one think.
4. The trap kind (§5) is drawn on the first trap's seed of a level and
   cached in the level's monster region for every later trap.

## Test vectors

Synthetic (CI-safe), draws given as `lo' % 100` unless noted:

| AI, input | Draws | Result |
|---|---|---|
| Sarcophagus, D = 10, n = 0, frame − param 0 = 125, footprint free, `Skill1` ≥ 0 | none | n := 1, param 0 := frame, Nest at T |
| GargoyleTrap, s = 0, T 3 east 10 south, D 10 | 10 (< 20) | P := (ux, ty); fire; s := 12 |
| 7TIllusion, param 1 = 0 | none | mode 4 at (x − 10, y); param 1 := 1 |
| BoneWall, frame ≤ param 0 | none | idle 15 |
| Trap-Melee, C = 1 | 69 (< 70) | A1 at T |

Game-file vectors: Open question 1.

## Provenance

- 1.14d `Game.exe`: `0x005F6A10`, `0x005F6630`, `0x005F6CA0`,
  `0x005F9490`, `0x006488A0`, `0x005FB6C0`, `0x005FB7E0`, `0x005FB650`,
  `0x00547BB0`, `0x004BC500`, `0x005FB900`, `0x005FB9B0`, `0x005E7590`,
  `0x005E0160`, `0x0063EC70`, `0x005E0390`, `0x005E0400`, `0x005FBA60`,
  `0x005EA080`, `0x005EA130`; tables `0x006E353C`, `0x006E3540` read
  from the file. Decompiler text read first; every call's register and
  stack arguments checked in the disassembly (`tools/ghidra/disasm.py`).
- Live data (`patch_d2`): monstats.txt (`AI`, `aip*`, skills, missiles),
  skills.txt (rows named) — read by a throwaway script (scratch, not
  committed).
- D2MOO 1.10f `AiThink.cpp`: names and aip meanings only.

## Open questions

1. No recording covers any AI of this file: record traps, the Act II
   palace, the Dark Wanderer and the Ubers (tick recorder) and compare
   think schedules and draws.
