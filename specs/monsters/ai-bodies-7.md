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
| Summary | 66–75 |
| Inputs | 76–80 |
| Outputs / state changes | 81–86 |
| Rules | 87–88 |
|   1. Scope and order | 89–122 |
|   2. Sarcophagus (45) `0x005F6A10`, init `0x005F6630` | 123–138 |
|   3. FlyingScimitar (47) `0x005F6CA0` | 139–149 |
|   4. GargoyleTrap (63) `0x005F9490` | 150–171 |
|   5. Trap-RightArrow (78) `0x005FB6C0`, Trap-LeftArrow (79) `0x005FB7E0` | 172–192 |
|   6. Trap-Poison (80) `0x005FB900`, Trap-Nova (92) `0x005FB9B0` | 193–206 |
|   7. JarJar (81) `0x005E7590` | 207–241 |
|   8. InvisoSpawner (82) `0x005E0160` | 242–259 |
|   9. BoneWall (84) `0x005E0400`, init `0x005E0390` | 260–266 |
|   10. Trap-Melee (87) `0x005FBA60` | 267–271 |
|   11. 7TIllusion (88) `0x005EA080` | 272–280 |
|   12. DarkWanderer (91) `0x005EA130` | 281–297 |
|   13. ArcaneTower (93) `0x005E0F60` | 298–313 |
|   14. Spirit (97) `0x005E3840` | 314–323 |
|   15. BladeCreeper (102) `0x005EA540`, init `0x005EA510` | 324–360 |
|   16. InvisoPet (103) `0x005EA7A0` | 361–375 |
|   17. DeathSentry (104) `0x005EA980`, init `0x005EA290` | 376–398 |
|   18. ShadowWarrior (105) `0x005EAFA0`, init `0x005EAF50` | 399–476 |
|   19. Raven (107) `0x005ECC10`, init `0x005ECB70` | 477–515 |
|   20. Vines (110) `0x005EC6C0`, init `0x005EC6A0` | 516–586 |
|   21. DruidBear (112) `0x005ED730` | 587–613 |
|   22. SiegeTower (113) `0x005E1860` | 614–628 |
|   23. GenericSpawner (129) `0x005E61B0`, init `0x005E6190` | 629–657 |
|   24. Wussie (131) `0x005EE3C0` | 658–685 |
|   25. UberIzual (144) `0x005F8C80` | 686–701 |
|   26. UberBaal (145), UberMephisto (146), UberDiablo (147) | 702–754 |
|   27. ShadowMaster (106) `0x005EB970`, init `0x005EB490`; ShadowMasterNoInit (143), init `0x005EB5C0` | 755–936 |
| Constants & data dependencies | 937–959 |
| Randomness | 960–967 |
| Edge cases & original bugs | 968–984 |
| Test vectors | 985–1005 |
| Provenance | 1006–1037 |
| Open questions | 1038–1049 |
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
| ShadowMaster (106), ShadowMasterNoInit (143) | shadowmaster; none | §27 |

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
      &(hx, hy))` ≠ 0 → stay := 0 (the seam then has replaced the
      point; return and point rule: `world/quests-act2.md` §10).
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

d2rs (rc-skill-div-a, 1.14d `ass-blade-sentinel`): `0x0056EDE0` (step 4) and
`0x00621CE0(M, unit)` run on the real missile store (the missile's owner
field becomes the creeper); the skill's AI command (`0x0058EF40`, `bodies-2.md`
§4.11 step 7) is inserted before the current one. The first walk's path
point then differs from 1.14d's by 2 sub-tiles (open: the creeper's walk
around the cow standing on its line; the stop distance is 0 on both sides).

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
(`0x006442A0(O, entry, 1)`). "O's entry of a skill" here and in §19 /
§27 is `highest_entry(O, skill)` `0x006439F0` (`skills/levels.md`),
and "O's level of it" is `skill_level(O, that entry, 1)` `0x006442A0`
(`0x005EAF7B`, `0x005ECC78`, `0x005EB4DB`, `0x005EB5F5`).

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
class → fail; `pettype` (+0xBE, signed byte) valid (≥ 0 and < the
pettype row count, data tables +0xBF0; `0x005EAB94`–`0x005EAB9E`) and O
≠ 0 → pass when the unit's
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
   ≤ 0) with lvl := O's level of its `Skill1` entry (§18 init: highest
   entry, level with bonus). Otherwise (c was
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

**The vine's walks** (1.14d-confirmed, read 2026-10-10, PC 1 today:
`0x005EC6C0`, `0x005E45D0`, `0x005E3EA0` case 1 `0x005E4187`–
`0x005E4386`, `0x005DEE50`, `0x005A7E60`, `0x005A63F0`, `0x005A6380`,
`0x00554570`; answers `[rc-gen-skill-2]`). The think never calls the
mode set itself: every walk of a vine with no target comes from step 5,
pet follow (`ai-bodies-6.md` §2) → pet move → a walk helper →
`0x005A7C20`. The vine's size is 0 (monstats2 `SizeX`), so pet follow's
D is the plain (2·max + min) / 2 of the position difference, and R = 6
+ (O's pet count >> 1) = 6 with one pet.

Recorded (`traces/orig-cache/gen-skill-dru-222`, vine class 425 g 8,
owner O standing at (5143, 4263) since its level warp at frame 4, O's
path target point (5142, 4266) from the cast):

- Frame 54, vine at (5142, 4266): D = 3 ≤ R. Four draws and mode 2
  toward (5143, 4270) = the vine + (1, 4), no speed bonus (animation
  speed 256 · 75 / 100 = 192, velocity 1792 · 75 / 100 = 1344): pet
  move k 0 whose eight tries start nothing, ending in wander' 4 (d2rs
  takes this branch and agrees through frame 66). The trace does not
  hold O's final point F, so the k = 0 cause (target ≠ F on both axes)
  is read from the code, not recorded.
- Frame 67, the walk-end think (`ai.md` §1.4) at (5143, 4270): D = 7 >
  R → k = 1. The path distance from the vine to O's last placed point
  is not < 28, because that point is (0, 0) by then: the placement's
  type-14 callback cleared it 50 frames after the warp
  (`sim/path-placement.md` §10 rule 6), so k stays 1 (with the point still set it would be 4:
  D < 30). Pet move k 1: O is not walking; speed = 0 → s := `roll(40)`
  + 40, the frame's **one** draw (`lo'` = 4143816763, % 40 = 3 → s =
  43). History walk from the newest entry: the warp's placement (5143,
  4263) is non-zero and its path distance from the vine is 7 > 5 →
  velocity (method 0, speed 43, steps 100) then walk-del
  `0x005DEE50(5143, 4263)` (call site `0x005E4293`), which sets stop
  distance 1, builds the mode-2 record and calls `0x005A7C20(game,
  record, 1)`.
- That request (`ai.md` §7.5 rules 4–5; 4.6 the speed bonus): method 0 does not
  replace the record's path type byte 101 → **path type 13**; steps :=
  77 (100 capped by `0x005DE190`); speed bonus 43 → stat 67 +43 for the
  mode: p = 75 + 43 = 118, animation speed 256 · 118 / 100 = **302**,
  velocity 1792 · 118 / 100 = 2114 (33824 / 65536 of a sub-tile a
  frame, the recorded y step). The requested point is O's own cell, so
  the computed path ends next to it and the path target becomes its
  last point (5143, 4264) (`sim/pathing.md` §3 step 10).

So a recording confirms: draw count 1 at the walk-end frame with s =
`lo' % 40` + 40; path target = the last point of the type-13 path to
the newest history entry of O farther than 5; `sp` = 256 · (75 + s) /
100. The same holds for classes 426 and 427 (`gen-skill-dru-231` /
`-241`): CycleOfLife (`ai-bodies-6.md` §25) reaches the same pet
follow.

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
over the running AI (`ai.md` §3.3). 1.14d-confirmed (bytes at the
three addresses, AI table `0x0073CA18` records 145–147; `monstats.bin`
of `patch_d2` gives rows 704, 705, 709 the AI 146, 147, 145).

**No other driver.** Nothing in 1.14d replaces these thinks:

- The AI table is read only by the lookup `0x005B15D0` (by the
  monster data's monstats row +0x1E); no code writes an AI function
  pointer as a constant, and every `0x005B0E00` call site installs a
  special state (`ai.md` §3.3) that the three rows (no `switchai`)
  resolve back to their base record.
- Boss mods `0x005B1CF0` (`monsters/init.md` §14.2) give only umods:
  704 ubermephisto {22, 30, 17, 8, 6}, 705 uberdiablo {22, 8, 6}, 709
  uberbaal {22, 18, 8, 6} (questcomplete, aura, lightning, resist,
  fast, cold). None installs an AI or teleport (umod 26), and the umod
  callbacks act on mode sets and event 7 only (`umod-callbacks.md`).
- Monster event 7 (`0x005A4370`) is the umod event; the Pandemonium
  room code `0x005559A0` only places presets (`population.md` §11.1);
  the uber death handler `0x005E0070` (umod 22, game +0x1DE8 / +0x1DEC
  / +0x1DF0, the torch drop) runs at death.
- The 1.11 uber helpers survive without callers: minion summons
  `0x005F7F10` (Mephisto), `0x005E9DD0` (Diablo's demonhole),
  `0x005FD0F0` (Baal), the uber-minion target pick `0x005F8010` and the
  scan callback `0x005DD140` that counts ubers and their minions near a
  unit. No pointer to them exists in the file.

**Resulting behaviour in 1.14d** (a rule, not a guess: it follows from
the dispatcher, `ai.md` §2, with an empty AI function). Each think runs
the prechecks: stun idles 3; the door check (`opendoors` = 1) needs a
blocked path, which never exists since the AI starts no move; target
mode 1 finds a target or schedules by distance (`ai.md` §2.3; with AI
state 3/19 and no target it wanders 5); with a target, precheck C plays
the boss sound once (all three are `boss`) and idles 20; no teleport
(no umod 26), no special walk (`isMelee` 0). After that, a think with a
target returns from the stub with **nothing scheduled**, so the
monster stands until something else schedules a think: a mode end
(gethit, knockback, block; then `aidel`, in Hell 6 / 12 / 12 for
Mephisto / Diablo / Baal), the end of a
freeze, a player entering its room while it is neutral (+2), or an
install over the running think (UberBaal and UberDiablo then run their
alternate for one think: command 10 kept, state 12 off, idle 1). They
never start an attack or a skill from AI code; their aura (umod 30)
and umod effects still work; the AI never casts `Skill8`. 1.14d-confirmed by
the reads above; the recording check is Open question 2.

### 27. ShadowMaster (106) `0x005EB970`, init `0x005EB490`; ShadowMasterNoInit (143), init `0x005EB5C0`

Target mode 2. Row shadowmaster (the Assassin's Shadow Master; AI 143
shares the think and has no live row). Most aips are read at **fixed
column offsets**, whatever the difficulty: A1n := aip1 (+0x56) [20],
A1m := aip1(N) (+0x58) [10], A1h := aip1(H) (+0x5A) [10], A2n := aip2
(+0x5C) [20], A2m := aip2(N) (+0x5E) [36], A2h := aip2(H) (+0x60)
[36], K0 := aip8 (+0x80) [279 Shadow Master]; only aip3 [60] is read by
difficulty. AI params: 0 = forced follow-up count, 1 = forced skill, 2
= λ (the owner's level of skill K0). O := the minion owner. "Use (s,
X, x, y)" = the use helper `0x005EB8B0` (below) with M := melee range
unit→X (`0x00622C40`).

**Init 106** `0x005EB490`: params 0, 1 := 0; λ := 1. O a player: λ :=
O's level of K0 with bonus when O has it (§18 init lookup); the unit gets skill 0
(Attack) at level 1 when it lacks it; its left and right skills := skill
0 (`0x00643BC0`, `0x00643C50`); then for each skill s of O's class
skill list (data tables +0xBA4 counts, +0xBAC ids by class,
`0x00646140` / `0x006460F0`) that passes the pet test of §18
(`0x005EAB20`): give the unit s at level clamp(v / 2 + λ / 2, 1, 24),
v := O's base level of s (`0x006442A0(O, entry, 0)`, 1 when O lacks
it) (`0x005EB420`). Implemented (rc-c008-monmode, 2026-10-10): the unit's list is its init
entries plus the assigned ones (`ActionHooks::monster_skills`, kept in
skill-id order: PROVISIONAL REC-3510, 1.14d's is insertion order), the
entry mode is the skill's `monanim`, and O's levels are read from the
player's list (`ActionHooks::skill_lists`: base + bonus, highest entry
for λ). Settled by `ass-shadow-master` (frame 48 cast of Fade, frame 62
neutral rate 194: the pet's skill levels come from O's, not 1; state
70/70).
**Init 143** `0x005EB5C0`: λ := 1 / O's level of
K0; skill 0 ensured; left and right := 0; no class skills.

**Think:**

1. The unit has no skill list (+0xA8) → idle 100. End.
2. Delete the unit's thinks. O ≠ 0 and squared distance unit→O > A2h²
   [1296] → pet follow (0, O, C, quiet 0, n 6) ≠ 0 → end.
3. param 0 > 0: the unit's path target unit (`0x00553540`), when set,
   replaces T. T = 0 → params 0, 1 := 0. Else param 0 −= 1; use
   (param 1, T, 0, 0) with the dispatch C as M ≠ 0 → end.
4. D > A2m [36] → T := 0. T = 0 → **buff pass** over the unit's skills
   (list order, `0x00643910` / `0x006438F0`):
   - `aitype` 1, `aurastate` > 0, the unit lacks it, and (no state of
     the same `group` (states +0x1E) is active, `0x005EB7F0`, or
     `roll(100)` < 4): draw < 60 and the unit's entry E of the skill
     exists → its `range` is 1 (melee) and C = 0 → run to unit 0 with
     flags 4 (`0x005DED00(0, 4)`); else `0x005DEAD0(E's mode, s, 0, 0,
     0)`. Started → end.
   - `aitype` 6, the unit has no left skill and draw < 20 → left skill
     := s.
5. X := 0. O ≠ 0: X := O's path target unit; X dead or not hostile to
   the unit (`0x00554200`) → X := 0; else T := X. Squared distance
   unit→O ≤ 144 → pet follow (T, O, C, quiet 0, n 6) ≠ 0 → end.
6. T = 0 → idle 25. End.
7. q := aip3 − 2 × max(λ, 1), clamped 5..100. C and `roll(100)` < q →
   use (0, T, 0, 0) with M := C ≠ 0 → end.
8. **Scan**: scan 1, callback `0x005EB6D0`, arg {O, best 0, dist
   0x7FFFFFFF, near 0, bestO 0, distO 0x7FFFFFFF, nearO 0, n 0, shadows
   0, boss 0}. For each U ≠ the unit, alive: U a monster with the unit's
   alignment pairing and `BaseId` 410–413, 415 or 416 (the assassin
   traps) → shadows += 1, next. Else U with unit flag 0x4 and hostile
   to the unit: O ≠ 0 → dO := squared distance O→U; dO ≤ 100 → nearO +=
   1; dO < distO → bestO := U, distO := dO. dU := squared distance
   unit→U; dU > 1024 → next U (the O counters above still counted it).
   Else n += 1; dU ≤ 100 → near += 1; dU < dist → best := U, dist :=
   dU; U is "notable" (below) → boss := U. So near, best and boss only
   ever hold units within squared distance 1024 of the unit; a far
   notable U never becomes boss. 1.14d-confirmed (asm): `0x005EB79B`
   `cmp eax, 0x400` / `jg 0x5EB7CD` jumps past the n, near, best and
   notable blocks (`0x005EB7A2`–`0x005EB7CA`) to the return; the dO
   block (`0x005EB771`–`0x005EB78F`) runs before it.
9. C = 0: T := X if set, else bestO if set, else boss when its squared
   distance < 1024. Then T not notable, T's minion owner (`0x0058F0D0(T)`)
   alive and within squared distance 1024 → T := that leader.
10. L := own life percent; dT := squared distance unit→T; pg := A1h ≥ 1
    and the unit has a `pgsv` state (state flag bit 4, `0x0063A2B0`);
    clear := the line unit→T is clear (`0x00622AA0(unit, T, 4)` = 0).
11. near > 3 and `roll(32)` < 2·near: O ≠ 0 and squared distance
    unit→O > 36 → run to O (`0x005DED20`). End. Else run away from T by
    8 with think delete (`0x005DF140`) started → end.
12. **Scoring.** Candidates: slot 0 := {T, skill 0, score 0}; a skill
    is appended only when its score s is strictly greater than the last
    appended score. For each skill k of the unit's list in list order
    (lvl := its level with bonus; skip a missing row): base := `aibonus`
    (skills +0x232) + `reqlevel` (+0x174) / 4 + lvl − R / 10, R := T's
    resist for the skill's `EType` (+0x1DC): 0 → stat 36, 1 → 39, 2 →
    41, 3 → 37, 4 and 12 → 43, 5 → 45, else 0 (all divisions signed,
    toward 0). "Pick" := `roll(A2n)` + base. The candidate's target is T
    unless noted. By `aitype` (+0x230):
    - 1: `aurastate` > 0 and the unit lacks it → s := 0. dist ≤ 25 → −6.
      Same-group state active (`0x005EB7F0`) → −10, else +10. s := pick;
      target := **the unit**. "Lacks it → s := 0" ends the case: no
      dist / group test, no `roll(A2n)` draw, and s = 0 is never
      appended. 1.14d-confirmed (asm): `0x005EC029` `je 0x5EC4F5` goes
      to the append test, past the draw at `0x005EC05C`.
    - 2: the unit has `aurastate` (> 0) or T has `auratargetstate` (> 0)
      → s := 0. Else dist ≤ 25 → −10; s := pick.
    - 3: shadows > 5 → −2·shadows; dist ≤ 25 → −7; n < 3 → −10; s :=
      pick + 3n − 9.
    - 4 and 12 (12 only when T is not a monster (`0x0044BE50`), or T's
      monstats `Drain` of the difficulty (+0xA0 + d, through the monster
      data's record pointer `0x0055B7E0`) ≥ 25; else s := 0):
      dT > A1n² → −10; + A1m; C or dT ≤ 25 → +10. The skill
      `progressive` (flags +4 bit 2): the unit has `aurastate` (> 0)
      with a stat list holding `aurastat1` = v → charges += v, v ≥ 3 →
      s := 0; otherwise (4) + A1h, s := pick; (12) s := pick, +8 when L <
      75, +12 more when L < 50. Not progressive: (4) A1h > 0 and not pg
      → −10, else + 4·charges + 3; s := pick. (12) the same rule as the
      progressive case with no charge stop: s := pick, +8 when L < 75,
      +12 more when L < 50 (no A1h, pg or charges term). 1.14d-confirmed
      (asm): in the aitype-12 case the progressive test `0x005EC406`
      jumps on clear straight to the draw `0x005EC45F` and the L
      bonuses `0x005EC46B`–`0x005EC478`; only the aitype-4 case
      (`0x005EC124` onward) has the A1h / pg branch.
    - 5 and 11: clear, and (`srvmissile` ≥ 0, or `srvmissilea` < 0, or
      its missile row missing, or dT < (missile `Range` − 1)²);
      otherwise s := 0. dist ≤ 25 → −5; dT ≤ 25 → −5; pg → −5. (5) s :=
      pick; (11) s := pick + 3n.
    - 6: no score (s := 0); the unit has no left skill → `roll(100)` <
      20 → left skill := k; has one → `roll(100)` < 6 → left skill := k.
    - 7: s := pick; L > 66 → s := 0, else target := none, and when O = 0
      two `roll(40)` steps are drawn and discarded (the point they would
      give is never stored); s += 10, +10 more when L < 45.
    - 8: r := pick; L > 66 → s := 0; else target := **the unit**, s :=
      2r, 4r when L < 45.
    - 13: + A1m; A1h > 0 and not pg → −5, else + charges. (L < 50 or
      near > 3), bestO ≠ 0, nearO < 4 and squared distance unit→bestO >
      25 → +20, target := bestO. Else dT < 25 → s := 0; dT > 324 → +10.
      s := pick.
    - 9, 10, other: s := 0.
    "dist" is the scan's best squared distance; "charges" accumulates
    over the list.
13. **Choice**: from the last candidate back to slot 0, each with one
    step `lo' & 3` ≠ 0 (3/4): use (skill, target, 0, 0). The first that
    starts ends the think; when its skill's `srvdofunc` (+0x2E) is 19:
    param 1 := that skill, param 0 := 25.
14. T ≠ 0 → use (0, T, 0, 0) ≠ 0 → end. Idle 15.

**Use helper** `0x005EB8B0(game, O, unit, s, M, X, x, y)`: X ≠ 0 and X
= O or X = the unit → 0 (so the self-target candidates of aitypes 1
and 8 never fire: bug kept). E := the unit's entry of s (owner −1);
none → 0. X ≠ 0 and (X lacks unit flag 0x4, or the unit's room is in
town) → 0. The skill's `range` (§18) is 1 and M = 0 → run to X with
flags 4 (`0x005DED00(X, 4)`). Else `0x005DEAD0(E's mode, s, X, x, y)`.
Returns the started result.

**Notable** `0x005EB650(unit, U)`: U a living monster ≠ the unit whose
class lacks the monstats `npc` bit and has `killable`, and that is a
boss (`0x0063E9F0`), or `primeevil` (`0x0063EDC0`), or of monster type
flags 0x0E (`0x005A0180`, `monsters/init.md`).

Draws, in order: step 4's `roll(100)` and draws; step 7; step 11's
`roll(32)`; per scored skill its pick roll (and case 6's, case 7's two
`roll(40)`); step 13's steps; the helpers'. 1.14d-confirmed
(`0x005EB970`, jump tables `0x005EC634` / `0x005EC650` (`EType`) and
`0x005EC660` (`aitype`), `0x005EB490`, `0x005EB5C0`, `0x005EB420`,
`0x005EB6D0`, `0x005EB650`, `0x005EB7F0`, `0x005EB8B0`, `0x0063A2B0`).

**List order and the first think** (1.14d-confirmed `0x00647110`,
`0x00643910`, `0x006438F0`, `0x005EB490`, read 2026-10-10, PC 1 today;
answers the `[q-fix-skills-bda]` question). A new skill entry is
appended at the **tail** of the unit's list (`0x00647110` walks the
next pointers +0x04 from the list's first entry +0x04; an entry the
unit already has is reused in place), and steps 4 and 12 walk from the
first entry. So the Shadow Master's list is: its monstats skills from
spawn (`Skill1`… = Fists of Fire, Blade Fury, Blades of Ice, …;
`monsters/init.md`), skill 0, then the owner's class skills not yet
present, in class-list order (`data/runtime-maps.md` §5: record order;
the Assassin's 251…280). Its `aitype` 1 skills with an `aurastate` are,
in that order, 258 Burst of Speed (`quickness`, state group 2), 267 Fade
(`fade`, group 2), 277 Blade Shield (`bladeshield`), 278 Venom
(`venomclaws`); all `range` none and monster mode `SC` (7).

With no target (T = 0 from the dispatch, or D > A2m) the first think
therefore takes the step-4 buff pass on **258 Burst of Speed**: the unit
lacks `quickness`, no group-2 state is active (so the `roll(100)` < 4
is not drawn), the one draw `lo' % 100` is < 60, the entry exists,
`range` ≠ 1 → `0x005DEAD0(mode 7, 258, unit 0, 0, 0)`: mode 7 with path
target point (0, 0) and no target unit (`ai.md` §7.5 rule 6). Recorded
(`traces/orig-cache/ass-shadow-master`, class 418 g 8, owner alone, the
poked cow not attackable): frame 48 exactly one step of the unit seed,
`lo'` = 1016988915, % 100 = 15 → mode 7, path target (0, 0); mode 1
again at frame 62; no further draw through frame 70. A draw ≥ 60 skips
that skill for the think and the pass goes on to 267 (same test, its
own draw), 277, 278; once `quickness` is on, 267 Fade needs the
`roll(100)` < 4 first (same group). d2rs idles 100 at step 1 while the
host does not provide the unit's skill list (`has_skill_list`,
`unit_skills`, `class_skills`, `entry_mode`, `skill_base_level` of the
AI host).

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
| shadow master | fixed aip columns; owner leash 36²; ignore range 36; scan radii² 100 / 1024; escape when > 3 near (`roll(32)`); scores by `aitype` with `aibonus`, `reqlevel` / 4, level, −resist / 10, `roll(aip2)`; 3/4 per candidate; follow-up 25 thinks for `srvdofunc` 19 | `0x005EB970` |

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
5. ShadowMaster §27: candidates of `aitype` 1 and 8 target the unit
   itself and the use helper refuses self targets, so they never fire
   (they still take part in the choice draws); `aitype` 7 draws two
   `roll(40)` steps for a point it never stores.
6. ShadowWarrior §18 and ShadowMaster §27 read aip8's three columns
   (and ShadowMaster aip1 / aip2's) as fixed constants whatever the
   difficulty.
7. Uber Mephisto, Diablo and Baal think functions are empty (§26).

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
| ShadowMaster scan (§27 step 8), one hostile notable U at squared distance 2000, no other unit | none | n = near = 0, best = boss = none |
| ShadowMaster scoring, aitype 1 skill, `aurastate` > 0 not on the unit | none (no `roll(A2n)`) | s = 0, not appended |
| ShadowMaster scoring, aitype 12, not progressive, T a player, L = 40, base 5 after the A1n / A1m / C terms, `roll(A2n)` = 7 | 1 | s = 7 + 5 + 8 + 12 = 32 (no A1h / pg term) |

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
  `0x005E9DF0`, `0x005E9DD0`, `0x005EB970`, `0x005EB490`, `0x005EB5C0`,
  `0x005EB420`, `0x005EB6D0`, `0x005EB650`, `0x005EB7F0`, `0x005EB8B0`,
  `0x0063A2B0`, `0x0044BE50`, `0x0055B7E0`; tables `0x006E353C`, `0x006E3540` read
  from the file; jump tables `0x005EC634`, `0x005EC650`, `0x005EC660`
  read from the file. Decompiler text read first; every call's register and
  stack arguments checked in the disassembly (`tools/ghidra/disasm.py`).
- §26 driver search (2026-10-07): AI table records 145–147 dumped;
  readers of `0x0073CA18` / `0x0073D358` (only `0x005B15D0`); the 33
  `0x005B0E00` call sites; `0x005B1CF0` boss-mod cases 0x2C0 / 0x2C1 /
  0x2C5; `0x005A4370`; `0x005559A0`; `0x005E0070`; every
  `cmp`/`push` of 0x2C0, 0x2C1, 0x2C5 in `all.asm`; no-reference checks
  of `0x005F7F10`, `0x005F8010`, `0x005E9DD0`, `0x005FD0F0`,
  `0x005DD140` (rel32 and 4-byte pointer scans); `monstats.bin`
  (`patch_d2`) AI words of rows 704–709.
- Live data (`patch_d2`): monstats.txt (`AI`, `aip*`, skills, missiles),
  skills.txt (rows named) — read by a throwaway script (scratch, not
  committed).
- D2MOO 1.10f `AiThink.cpp`: names and aip meanings only.

## Open questions

1. No recording covers any AI of this file: record traps, the Act II
   palace, the Dark Wanderer and the Ubers (tick recorder) and compare
   think schedules and draws.
2. Answered statically (2026-10-07): nothing drives them; §26 "No
   other driver" lists what was read (AI table readers, every
   `0x005B0E00` site, boss mods, umods, event 7, the Pandemonium room
   code, the orphaned 1.11 helpers) and "Resulting behaviour" gives the
   rule. Left as a check, not a question: an Uber Tristram recording
   (PC 2 list) should show no AI-started attack or skill mode.
