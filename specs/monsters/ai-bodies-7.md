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
| Summary | 65–74 |
| Inputs | 75–79 |
| Outputs / state changes | 80–85 |
| Rules | 86–87 |
|   1. Scope and order | 88–120 |
|   2. Sarcophagus (45) `0x005F6A10`, init `0x005F6630` | 121–136 |
|   3. FlyingScimitar (47) `0x005F6CA0` | 137–147 |
|   4. GargoyleTrap (63) `0x005F9490` | 148–169 |
|   5. Trap-RightArrow (78) `0x005FB6C0`, Trap-LeftArrow (79) `0x005FB7E0` | 170–190 |
|   6. Trap-Poison (80) `0x005FB900`, Trap-Nova (92) `0x005FB9B0` | 191–204 |
|   7. JarJar (81) `0x005E7590` | 205–238 |
|   8. InvisoSpawner (82) `0x005E0160` | 239–256 |
|   9. BoneWall (84) `0x005E0400`, init `0x005E0390` | 257–263 |
|   10. Trap-Melee (87) `0x005FBA60` | 264–268 |
|   11. 7TIllusion (88) `0x005EA080` | 269–277 |
|   12. DarkWanderer (91) `0x005EA130` | 278–294 |
|   13. ArcaneTower (93) `0x005E0F60` | 295–310 |
|   14. Spirit (97) `0x005E3840` | 311–320 |
|   15. BladeCreeper (102) `0x005EA540`, init `0x005EA510` | 321–350 |
|   16. InvisoPet (103) `0x005EA7A0` | 351–365 |
|   17. DeathSentry (104) `0x005EA980`, init `0x005EA290` | 366–388 |
|   18. ShadowWarrior (105) `0x005EAFA0`, init `0x005EAF50` | 389–461 |
|   19. Raven (107) `0x005ECC10`, init `0x005ECB70` | 462–499 |
|   20. Vines (110) `0x005EC6C0`, init `0x005EC6A0` | 500–520 |
|   21. DruidBear (112) `0x005ED730` | 521–547 |
|   22. SiegeTower (113) `0x005E1860` | 548–562 |
|   23. GenericSpawner (129) `0x005E61B0`, init `0x005E6190` | 563–591 |
|   24. Wussie (131) `0x005EE3C0` | 592–619 |
|   25. UberIzual (144) `0x005F8C80` | 620–635 |
|   26. UberBaal (145), UberMephisto (146), UberDiablo (147) | 636–649 |
| Constants & data dependencies | 650–671 |
| Randomness | 672–679 |
| Edge cases & original bugs | 680–688 |
| Test vectors | 689–706 |
| Provenance | 707–727 |
| Open questions | 728–737 |
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
| ArcaneTower (93) | lightningspire | §13 |
| Spirit (97) | mephistospirit | §14 |
| BladeCreeper (102) | bladecreeper | §15 |
| InvisoPet (103) | invisopet | §16 |
| DeathSentry (104) | deathsentry | §17 |
| ShadowWarrior (105) | shadowwarrior | §18 |
| Raven (107) | druidhawk | §19 |
| Vines (110) | plaguepoppy | §20 |
| DruidBear (112) | druidbear | §21 |
| SiegeTower (113) | barricadetower | §22 |
| GenericSpawner (129) | evilhut | §23 |
| Wussie (131) | act5pow | §24 |
| UberIzual (144) | uberizual | §25 |
| UberBaal (145), UberMephisto (146), UberDiablo (147) | uberbaal, ubermephisto, uberdiablo | §26 |

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

### 13. ArcaneTower (93) `0x005E0F60`

Row lightningspire; brackets [1, 150, 33, 4, 150, 50]; `Skill1`
ArcaneTower, `MissA1` arcanelightningbolt. AI params: 0 = phase p (0
skill, 1 bolts), 1 = shots left n, 2 = next frame f.

1. n = 0 → n := aip1 [1].
2. frame < f → idle 10. End.
3. `Skill1` ≥ 0 and p = 0: `Skill1` at T; n −= 1; n < 1 → p := 1, n :=
   aip4 [4], f := frame + aip3 [33]; else f := frame + aip2 [150]. End.
4. A1 at T; n −= 1; n < 1 → p := 0, n := aip1, f := frame + aip5 [150];
   else f := frame + aip6 [50].

The monai comments call aip5 "sk2 stall" and aip6 "sk2 long delay"; the
code uses them the other way round. No draws. 1.14d-confirmed.

### 14. Spirit (97) `0x005E3840`

Row mephistospirit. AI param 0 = struck.

1. param 0 ≠ 0 → idle 50. End.
2. C → param 0 := 1; A1 at T. End.
3. Idle 10.

1.14d-confirmed.

### 15. BladeCreeper (102) `0x005EA540`, init `0x005EA510`

Row bladecreeper (the Assassin's Blade Sentinel); `Skill1` Blade
Sentinel. Init: param 0 := −1 (expiry), param 1 := 1 (leg), param 2
:= 0 (missile made).

1. `Skill1`'s row missing or the unit has no entry of it (`0x006439F0`)
   → death. End. L := the entry's level (`0x006442A0(unit, entry, 1)`).
2. param 0 < 0 → param 0 := frame + `calc4` of `Skill1` at L
   (`0x00646CA0`, skills +0x144).
3. frame > param 0 → death. End.
4. param 2 = 0 and the skill's `srvmissilea` (skills +0x48) is a valid
   missile: O := the minion owner, or the unit itself; M := a missile
   from O (`0x0056EDE0(game, O, Skill1, L, srvmissilea, own x, own y)`,
   `missiles/bodies.md`); made → `0x00621CE0(M, unit)` (the missile
   remembers the creeper); the unit's stat 19 `tohit` := O's attack
   rating (`0x00622560`, `combat/hit.md`); stat 119
   `item_tohit_percent` := O's stat 119 + O's weapon mastery to-hit when
   O has a weapon (`0x00535BC0`, `0x00645830(O, weapon, 0, 0)`). param 2
   := 1 (also when no missile was made).
5. K := the current command (`0x0058EE80`); none → idle 3. End.
6. Velocity (0, 0, 20). param 1 = 0: walk-del to (K params 1, 2)
   started → end; param 1 := 1; walk-del to (K params 3, 4). param 1 ≠
   0: walk-del to (params 3, 4) started → end; param 1 := 0; walk-del to
   (params 1, 2). Neither started → wander near the minion owner 5
   (`0x005DF530`); not started → idle 5.

The creeper shuttles between the two points of its command (set by the
skill, `skills/bodies.md`). No draws (the wander's). 1.14d-confirmed.

### 16. InvisoPet (103) `0x005EA7A0`

Row invisopet; brackets [16, 15]. O := the minion owner (read through
a null pointer when there is none: unreachable in practice). AI param 0
= next frame f.

1. r := aip1 [16]; O in mode 3 (run) → r := aip1 / 2.
2. frame ≤ f, O not in mode 3 and AI state not 3/19 → idle aip2 [15].
   End.
3. x := O.x + `roll(2r)` − r, then y := O.y + `roll(2r)` − r (two steps
   when 2r ≥ 1).
4. `0x005DEAD0(9, 292 Teleport 2, 0, x, y)`; f := frame + aip2.

1.14d-confirmed.

### 17. DeathSentry (104) `0x005EA980`, init `0x005EA290`

Row deathsentry; brackets [30, 0, 50, 16]; `Skill1` mon death sentry
(`seq_chargesentry`), `Skill2` death sentry ltng. Init and charges as
AssassinSentry (`ai-bodies-6.md` §14). AI param 0 = last exploded
corpse's GUID g.

1. Charges (use 0) = 1 → end.
2. E := the unit's entry of `Skill1`; L := its level. E = 0 or L ≤ 0 →
   return, **nothing scheduled**.
3. S, E2 := `0x005DDC30(unit)`. S = 0 → idle aip2 [0 → 1]. End.
4. K := the corpse search `0x0056E390(unit, S, Skill1, L)`
   (`ai-bodies-5.md` §23 step 6); r := (`Param3` + (L − 1) ×
   `Param4`) / 2 of `Skill1` (`0x004CC7C0`, skills +0x150 / +0x154;
   signed halving).
5. K ≠ 0, K's GUID ≠ g and `0x006416D0(S, K)` < r: charges (use 1) =
   1 → end; g := K's GUID; `0x005DEAD0(9, Skill1, K, 0, 0)`. End.
6. E2 < aip4 [16] and `roll(100)` < aip3 [50]: charges (use 1) = 1 →
   end; `0x005DEAD0(14, Skill2, S, 0, 0)`. End.
7. Idle aip2.

1.14d-confirmed.

### 18. ShadowWarrior (105) `0x005EAFA0`, init `0x005EAF50`

Row shadowwarrior (the Assassin's Shadow Warrior). aip8's three columns
are read as three constants, whatever the difficulty: K0 := aip8
(Normal, +0x80) = 268 (the summoning skill), K1 := aip8(N) (+0x82) = 5,
K2 := aip8(H) (+0x84) = 64. Brackets [40, 30, 60, 1]. AI params: 0 =
next skill frame, 1 = mana pool m, 2 = mimic level λ. O := the minion
owner.

Init: λ := 1; O's entry of skill K0 exists → λ := its level with bonus
(`0x006442A0(O, entry, 1)`).

1. Delete the unit's thinks. O = 0 → idle 100. End.
2. H := K2 clamped to 1..256. m += −1 − aip4 [1]; m < 0 or m > 64H →
   m := 0.
3. dO := `0x006416D0(unit, O)`. D > aip1 [40] or dO > aip2 [30] → T :=
   0.
4. Pet follow (T, O, C, quiet 0, n 6) (`ai-bodies-6.md` §2) ≠ 0 → end.
5. T ≠ 0, O has a right skill R (`0x006201D0`) and a left skill L
   (`0x00620190`):
   1. Give the unit O's two skills (`0x00647280(unit, id, level, 0)`)
      at mimic levels: λ / 3 + (O's level of it with bonus) / 2, at
      least 1 (`0x005EAF00`). ER, EL := the unit's entries of them
      (`0x006439B0(unit, id, −1)`); either missing → step 6.
   2. X := `roll(2)` ≠ 0 ? EL : ER; id := X's skill (`0x00643CE0`).
   3. q := aip3 [60] − 2 × max(λ, 1), clamped to 5..100. C and
      `roll(100)` < q → X := the unit's entry of skill 0 (Attack), id :=
      0.
   4. Usable (below) X → step 5.6. Else X := the other of ER / EL (EL
      unless X was EL); usable → step 5.6. Else X := the unit's skill-0
      entry; none → give skill 0 at level 1 and take its entry.
   5. X = 0 → step 6.
   6. a := the skill's `range` (skills +0x14, `0x00645460(unit, X)`;
      value 3 "both" becomes 2 when `0x0064F460(unit)` ≠ 0, else 1, and
      1 again for a player with state 38). a ≠ 1 (not melee), or C ≠ 0 →
      `0x005DEAD0(X's mode, id, T, 0, 0)`; param 0 := frame + 18 + (the
      skill's `delay` (skills +0x190) evaluated at max(level of X, 1),
      `0x00646CA0`) / 3 (signed). End. a = 1 and C = 0 → run to T
      (`0x005DED20`). End.
6. Idle 25.

**Usable** `0x005EAD50(game, unit, O, X, id, C, tick)` → 1 / 0:

1. X, O, tick or its control missing → 0. The skill's `charclass`
   (skills +0x0C, `0x00645040`; negative → 7) ≠ O's class → 0.
2. Allowed `0x005EABF0(game, O, unit, id, tick)` = 0 → 0 (below).
3. id = 0 → 1.
4. c := mana cost at X's level (`0x006459F0`: ((`lvlmana` × (lvl − 1)
   + `mana`) << `manashift`) >> 8, at least 0; skills +0x18C, +0x18A,
   +0x188). `roll(100)` > 100 − 160c / 100 → 0.
5. frame < param 0 → 0.
6. lo := K1 clamped 1..128; hi := K2 clamped 1..256. m < lo or m > 32hi
   → m := lo.
7. a := `roll(m)`, b := `roll(100)`; b < a → 0.
8. m += (320 − λ) × c / (λ + 100) (signed). Return 1.

**Allowed** `0x005EABF0`: the skill's row exists and the pet test
`0x005EAB20` passes (skills `summon` (+0xBC) = 0 → pass; = the unit's
class → fail; `pettype` (+0xBE) valid and O ≠ 0 → pass when the unit's
pet type in O's lists (`0x00574A20`, `sim/pets.md` §9) ≠ `pettype`;
otherwise pass). t := skills `aitype` (+0x230). C = 0 and t ∈ {4, 13}
→ 0; C ≠ 0 and t ∉ {4, 13} → 0. t = 1, `aurastate` ≥ 1 and the unit
has it → 0. T = 0 and t ∈ {2, 4, 5, 11, 12, 13} → 0. t = 2 and (the
unit has `aurastate` (≥ 1), or T has `auratargetstate` (≥ 1)) → 0. The
skill is `progressive` (flags +4 bit 2), `aurastate` ≥ 1, the unit has
it and its stat list (`0x006256B0`) holds `aurastat1` (skills +0x54) ≥
3 → 0. Else 1.

Draws, in order: `roll(2)`, the C draw, then per usable test its
`roll(100)`, `roll(m)`, `roll(100)`; the pet follow's. 1.14d-confirmed
(`0x005EAFA0`, `0x005EAF50`, `0x005EAF00`, `0x005EAD50`, `0x005EABF0`,
`0x005EAB20`, `0x00645040`, `0x006459F0`, `0x00645460`).

### 19. Raven (107) `0x005ECC10`, init `0x005ECB70`

Target mode 2. Row druidhawk (the Druid's ravens); brackets [10, 6, 5,
75, 35]; `Skill1` Raven. AI params: 0 = hits left c, 1 = next attack
frame f, 2 = orbit side σ. O := the minion owner.

Init: c := −1; f := aip3 + 1; σ := one step's `lo' & 1` (a draw at
creation).

1. O = 0 (or no monstats row) → idle 10. End.
2. c = −1 → c := 3, and when `Skill1` ≥ 0: c := `Param5` + (lvl − 1) ×
   `Param6` of `Skill1` (`0x004EFCB0`, skills +0x158 / +0x15C; 0 when lvl
   ≤ 0) with lvl := O's level of its `Skill1` entry. Otherwise (c was
   not −1) c = 0 (the hits are spent) → kill the unit with itself as
   killer (`0x0057CCB0(game, unit, unit, 1)`). End.
3. dO := full-size distance unit→O. dO > 50 → pet move k 3 (0, 0, 0).
   End.
4. v := `Run` × 100 / `Velocity` − 100, 100 when `Velocity` ≤ 0 or v ≥
   100. dO > 28 → pet move k 0 (run 0, speed v, n 0). End.
5. T0 := the main search for O with the raven's control; its distance
   to the raven (`0x006416D0`) is computed and not used.
6. mid := (aip2 + aip1) / 2 [8].
7. T ≠ 0 and f < frame: `roll(100)` < aip4 [75] and D < aip5 [35] → C
   → A1 at T, c −= 1, f := frame + aip3 × 10 [50]; not C → walk to T
   with flags 0. End.
8. dO > aip1 [10] or dO < aip2 [6] → step 9. Else orbit: velocity
   (σ ? 5 : 6, 0, 4) and mode 2 toward O with path step 1 and request
   flag 1 (`0x005ECBC0`) started → end; σ := ¬σ; the same with the new
   σ started → end.
9. Walk to radius mid of O (below); not started → pet move k 1 (0, 0,
   0).

**Walk to radius** `0x005DE6F0(game, U, ·, O, r)`: d := `0x006416D0(U,
O)`; d = 0 → 0. Point := O + (U − O) × r / d per axis (signed); mode 2
at that point with request flag 1 (no step count). Returns the result.

1.14d-confirmed.

### 20. Vines (110) `0x005EC6C0`, init `0x005EC6A0`

Target mode 2. Row plaguepoppy (Poison Creeper); brackets [100, 20,
25, 10, 35]; `Skill1` Vine Attack. Init: param 0 := 0. AI param 1 =
last attack frame f. O := the minion owner.

1. O = 0 → idle 25. End.
2. `0x006416D0(unit, O)` ≥ aip5 [35] → pet move k 3 (0, 0, n 6) ≠ 0 →
   end.
3. The unit's room in town → pet follow (0, O, 0, quiet 0, n 6) = 0 →
   idle aip3 [25]. End.
4. S, E, M := `0x005DDC30(unit)`. E ≥ aip2 [20] → S := 0.
5. Pet follow (S, O, M, quiet 0, n 6) ≠ 0 → end.
6. S ≠ 0: S has state 2 (`poison`) or S's stat 45 `poisonresist` =
   100 → escape from S by aip4 [10] (byte), no delete. End. M = 0 →
   walk to S with flags 7. End. `Skill1` ≥ 0 and frame > f + aip1 [100]
   → `Skill1` at S; f := frame. End.
7. Idle aip3.

1.14d-confirmed.

### 21. DruidBear (112) `0x005ED730`

Row druidbear; brackets [15, 40, 50]; `Skill1` BearSmite. AI param 2
= owner GUID. O := the minion owner. No pet follow.

1. O = 0: param 2 ≠ 0 and that player exists → pet move k 3 toward
   it; else idle 10. End.
2. param 2 := O's GUID. dO := full-size distance unit→O; m := O's
   mode.
3. dO > 50 → pet move k 3 (0, 0, 0). End.
4. v as §19 step 4. dO > 28 → pet move k 0 (run 0, speed 100, n 0).
   End.
5. dO > 18: m = 2 or 6 → pet move k 0 (0, 0, 0) started → end; m = 3
   → k 0 (0, 100, 0) started → end.
6. T0 := the main search for O with the bear's control. S, E, M :=
   search capped (unit, control, 28) (`ai-bodies-6.md` §2). S ≠ 0 and
   not directly reachable (`0x005DC640`) → S := 0. S = 0: T0 ≠ 0,
   full-size distance unit→T0 < 28 and T0 directly reachable → S :=
   T0.
7. M = 0 and S ≠ 0: `roll(100)` < aip2 [40] → velocity (0, v, 40); walk
   to S (`0x005DECA0`, `ai-bodies-5.md` Summary). End.
8. M ≠ 0 and S ≠ 0: `roll(100)` < aip3 [50] → sequence skill `Skill1`
   at S (`0x005DE000`). End. Else A1 at S; wait aip1 [15]. End.
9. dO < 17 → idle 15; else pet move k 0 (0, 0, 0).

1.14d-confirmed.

### 22. SiegeTower (113) `0x005E1860`

Row barricadetower; brackets [40].

1. The unit's owner (`0x00552FD0`) exists and is alive → idle aip1.
   End.
2. T ≠ 0: rider scan (`ai-bodies-5.md` §15 step 2, callback
   `0x005E1720`, arg {400}): a free imp1-based monster within squared
   distance 400 → assign it to this tower (`0x005E17E0(U, unit)`, §15
   rule) and idle aip1.
3. Idle aip1 (also after step 2: the second request replaces the
   first).

No draws. 1.14d-confirmed.

### 23. GenericSpawner (129) `0x005E61B0`, init `0x005E6190`

Row evilhut; brackets [80, –, 15]. Init: FoulCrowNest's init (param 0
:= frame, param 1 := 0, `ai.md` §9.17), then control +0x3C (minion
spawn class) := −1.

1. T = 0 → idle 20. End.
2. control +0x3C = −1 → spawn class pick `0x005E6020` (below); it
   always reports success, so the death branch after it (unit flag
   0x20000, death) is unreachable.
3. D ≥ 21 → idle 20. End.
4. param 1 ≥ aip3 [15] → unit flag 0x20000; death. End.
5. |frame − param 0| ≥ aip1 [80]: param 0 := frame; footprint test
   `0x005FD350(528 evilhut, room, x, y, 0)` (`monsters/population.md`
   §9.3: (x + 2, y + 4)) passes → param 1 += 1; sequence skill 167 Nest
   at T (`0x005DE000(167, T, 0, 0)`). End.
6. Idle 20.

**Spawn class pick** `0x005E6020(game, unit, &cls)`: cls := 453
(minion1; −1 when monstats is too short). R := the monster region of
the unit's level (monster data +0x58, `0x00573520`; `0x00547BB0`,
`monsters/population.md` §2.2). n := R's monster count (+0x10); i :=
`roll(n)` (one step when n ≥ 1). The class of entry i (+0x14 + 0x34·i)
with the monstats `genericSpawn` flag (bit 21) → cls := it. Else the
first entry 0..n − 1 with the flag → cls := it. Else cls := 496
(imp5). Returns success in every case.

1.14d-confirmed.

### 24. Wussie (131) `0x005EE3C0`

Target mode 2. Row act5pow (the caged barbarians of Act V quest 2).
Seams of `world/quests-act5.md` §4.7–§4.10.

1. Portal hook `0x00588D60(game, unit, &P)` ≠ 0 (the group's rescue
   portal exists):
   1. P = 0 → idle 25. End.
   2. Hook `0x00588830(game, unit)` ≠ 0, or the path distance to P <
      4 → leave: hook `0x00588880(game, 0, unit)`; stop the path
      (`0x00648730`); delete thinks; unlink from the room
      (`0x0061A270(room, type, GUID)`), `0x00623830(unit)`, remove from
      the room list (`0x0064C370`, `sim/unit-order.md`), remove the unit
      (`0x00555600`). End.
   3. Draw < 70 → walk to P with flags 7. End. `roll(100)` < 10 →
      wander 4. End. Idle 10.
2. Else P := the nearest interacting player within 15
   (`0x005DDF20`); P a player:
   1. Hook `0x00588E10(game, P, unit)` ≠ 0 → rescue `0x005888D0(game,
      P, unit)`; unit +0xD0 = 11 → register in target slot 8 (as NpcBarb,
      `ai-bodies-6.md` §26) and idle 25. End.
   2. Else hook `0x00588DD0(game, P, unit)`; `roll(1000)` < 100 → S→C
      0x8A NpcWantsInteract {1, the unit's GUID} to P's client
      (`0x005531C0`, `0x0053DFF0`).
3. Idle 25.

1.14d-confirmed.

### 25. UberIzual (144) `0x005F8C80`

Brackets: uberizual [45, 50, 66, 0, 20, 3]; `Skill1` Frost Nova,
`Skill2` Chilling Armor, `Skill3` MonTeleport. Params and the nova as
Izual (`ai-bodies-4.md` §8: 1 = pending stall s, 2 = swings w; (x, y)
:= T's position). No quest seam.

1. `Skill2` ≥ 0, its row exists, its `aurastate` ≥ 0 and the unit lacks
   that state → `0x005DEAD0(Sk2mode, Skill2, 0, 0, 0)` (the armor).
   End.
2. The line unit→T is blocked (`0x00622AA0(unit, T, 6)` ≠ 0) →
   `0x005DEAD0(Sk3mode, Skill3, 0, x, y)` (teleport to T). End.
3. Then Izual's steps 2–6 (`ai-bodies-4.md` §8), unchanged.

1.14d-confirmed.

### 26. UberBaal (145), UberMephisto (146), UberDiablo (147)

The think functions `0x005FD200`, `0x005F81C0` and `0x005E9DF0` are a
bare return in 1.14d: no draw, no mode, **nothing scheduled**. (D2MOO
reconstructs full bodies for these addresses; the 1.14d bytes are `ret
4` followed by padding.) The table gives UberBaal BaalCrab's alternate
(`0x005FCF30`, `ai-bodies-5.md` §21) and UberDiablo Diablo's
(`0x005E8480`, `ai-bodies-4.md` §7); they run only on a re-install
over the running AI (`ai.md` §3.3). What drives these monsters in play
is Open question 2. `0x005E9DD0` (a summon of class 711 demonhole
around a unit through `0x005B23C0`) sits beside the UberDiablo stub
with no caller found. 1.14d-confirmed (bytes at the three addresses,
AI table `0x0073CA18` records 145–147).

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
| arcane tower | volleys aip1 / aip4; delays aip2, aip3, aip5, aip6 | `0x005E0F60` |
| blade creeper | life `calc4`; to-hit from the owner | `0x005EA540` |
| death sentry | explosion radius (`Param3` + (L − 1) `Param4`) / 2 | `0x005EA980`, `0x004CC7C0` |
| shadow | aip8 columns = skill 268 / 5 / 64 (warrior); mimic level λ / 3 + lvl / 2; attack chance aip3 − 2λ in 5..100; mana pool 320 − λ rule; delay / 3 + 18 | `0x005EAFA0`, `0x005EAD50` |
| raven | hits `Param5` + (lvl − 1) `Param6`; orbit band aip2..aip1, radius (aip1 + aip2) / 2 | `0x005ECC10` |
| druid bear | ranges 50 / 28 / 18 / 17 | `0x005ED730` |
| generic spawner | footprint 528 at (x + 2, y + 4); skill 167; spawn class: region entry with `genericSpawn`, else 496 | `0x005E61B0`, `0x005E6020` |
| barbarians | portal reach 4; help message 0x8A at 10 % | `0x005EE3C0` |
| Ubers 145–147 | empty thinks | `0x005FD200`, `0x005F81C0`, `0x005E9DF0` |

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
| ArcaneTower, p = 0, n = 1, frame ≥ f, lightningspire Normal | none | `Skill1`; p := 1, n := 4, f := frame + 33 |
| Spirit, param 0 = 0, C = 1 | none | param 0 := 1, A1 |
| UberMephisto, any | none | nothing scheduled |
| SiegeTower, owner alive | none | idle 40 |

Game-file vectors: Open question 1.

## Provenance

- 1.14d `Game.exe`: `0x005F6A10`, `0x005F6630`, `0x005F6CA0`,
  `0x005F9490`, `0x006488A0`, `0x005FB6C0`, `0x005FB7E0`, `0x005FB650`,
  `0x00547BB0`, `0x004BC500`, `0x005FB900`, `0x005FB9B0`, `0x005E7590`,
  `0x005E0160`, `0x0063EC70`, `0x005E0390`, `0x005E0400`, `0x005FBA60`,
  `0x005EA080`, `0x005EA130`, `0x005E0F60`, `0x005E3840`, `0x005EA510`,
  `0x005EA540`, `0x005EA7A0`, `0x005EA980`, `0x004CC7C0`, `0x005EAF50`,
  `0x005EAFA0`, `0x005EAF00`, `0x005EAD50`, `0x005EABF0`, `0x005EAB20`,
  `0x00645040`, `0x006459F0`, `0x00645460`, `0x005ECB70`, `0x005ECC10`,
  `0x005ECBC0`, `0x005DE6F0`, `0x004EFCB0`, `0x005EC6A0`, `0x005EC6C0`,
  `0x005ED730`, `0x005E1860`, `0x005E6190`, `0x005E61B0`, `0x005E6020`,
  `0x00573520`, `0x005EE3C0`, `0x005F8C80`, `0x005FD200`, `0x005F81C0`,
  `0x005E9DF0`, `0x005E9DD0`; tables `0x006E353C`, `0x006E3540` read
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
2. Uber Mephisto, Diablo and Baal have empty think functions in 1.14d
   (§26): find what drives them (a special-state install, a quest or
   event callback of the Uber Tristram code, or a different AI index
   at spawn) — record an Uber Tristram run with AI think logging.
