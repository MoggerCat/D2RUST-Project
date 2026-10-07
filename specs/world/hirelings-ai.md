# Spec: World — Hireling AI skill pick (`hireling.txt` chance columns)

- **Status:** draft: read from the 1.14d `Game.exe` disassembly and
  decompiled exports (addresses inline) on 2026-10-07; no recording.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::world::hirelings`
- **Related specs:** `world/hirelings.md` (owner of the hireling rules;
  §1.1 rule 6 names the row readers, §14 links here); `monsters/ai.md`
  §3.2 (Hireable AI `0x005E52D0`, think call site, tactics helpers);
  `sim/rng.md` (rand(n)); `data/fields.tsv` (`hireling`, `skills`,
  `monstats` offsets).

## Summary

The Hireable AI think (`0x005E52D0` → `0x005E5050`) picks the
mercenary's action with `0x005E4D30`: a weighted roll over the
`hireling.txt` row's `DefaultChance` and the six `Skill` / `Chance` /
`ChancePerLvl` / `Mode` slots, with a per-class fallback. This file
holds that function only; when to call it is the AI spec's.

## Inputs

| Name | Type | Source |
|---|---|---|
| merc | monster unit | AI think |
| class | monstats id | the merc's class |
| Id | hireling row id | pet node +8 (`0x00574BD0`) |
| target | unit or null | AI think |
| seed | AI seed | AI think |

## Outputs / state changes

One AI tactic call (use skill, select aura then idle, attack, idle) and
at most one draw of the AI seed.

## Rules

### 1. Skill pick (`0x005E4D30`)


The one reader of `DefaultChance` and the `Chance` columns (§1.1 rule
6). Called once per think by `0x005E5050` (AI think `0x005E52D0`,
`monsters/ai.md` §3.2 row 4) with the merc's class, the `Id` of its pet
node (node +8, `0x00574BD0`), the target and the AI seed. The AI spec
owns the call site and the actions; this is what the function does:

1. class < 0 or ≥ the `monstats` count → nothing. L = merc stat 12
   (`0x00625480`); row = §1.2 rule 2 (game +0x70 expansion, `Id`, L);
   none → nothing.
2. d = L − row `Level`, < 0 → 0. acc = `DefaultChance`.
3. For slot i = 1 … 6: skill s = `Skill`i; s < 1 or ≥ the skill count
   → stop. w_i := 0. The slot counts when the merc has the skill
   (`0x006439B0`) at level `0x006442A0(merc, s, 1)` > 0, unless the
   skill's `aitype` (+0x230) is 1 and its `aurastate` (+0x80) is a valid
   state the merc already has (`0x00639DF0`). For s = 41 (Inferno) with
   a target: if distance(merc, target) (`0x006416D0`) > level / 2 + 4
   (signed) the slot does not count. A counting slot: acc +=
   trunc(`ChancePerLvl`i · d / 4) (signed, toward zero) + `Chance`i;
   w_i := acc.
4. Roll: n = acc + 1; n < 1 → r = 0, no draw; else one draw of the AI
   seed: r = rand(n) (`rng.md`: power of two → low word & acc, else low
   word mod n).
5. r ≥ `DefaultChance`: the first slot i with w_i ≥ r; its skill s > 0
   and valid: s not an `aura` skill (skills byte +4 bit 5, mask
   `0x006CE27C`) → `AITACTICS_UseSkill` `0x005DEAD0(Mode`i`, s, target,
   0, 0)`, done; aura → `0x005701B0(s, −1)` then idle 10
   (`0x005DE080(10)`), done.
6. Otherwise (r < `DefaultChance`, no slot, or a bad skill): class 271
   (`roguehire`) → `0x005DEAD0(monstats +0x180 (resolved Sk1mode),
   Skill1 +0x170, target, 0, 0)`; classes 338, 359, 560, 561 →
   melee range (`0x00622C40(merc, target, 0)`) → `0x005DDF90(4,
   target)` (attack mode 4), else idle 10; any other class → idle 10.
7. When no slot counted, `DefaultChance` > 0 and r = acc (=
   `DefaultChance`), every written w is 0 < r, so the search of rule 5
   runs past the slots written in this call (live rows have at most 3
   skills) and compares w entries this call never wrote (stack, edge
   case 12).

## Constants & data dependencies

`hireling` (`data/fields.tsv`): `Level` +0x1C, `DefaultChance` +0x64,
`Skill1`–`6` +0x78, `Chance1`–`6` +0x90, `ChancePerLvl1`–`6` +0xA8,
`Mode1`–`6` +0xC0 (u8). `skills`: `aura` (byte +4 bit 5), `aurastate`
+0x80, `aitype` +0x230. `monstats`: `Skill1` +0x170 and the resolved
`Sk1mode` byte +0x180. Skill 41 (Inferno) is the only id tested by
number. Live expansion rows: `DefaultChance` 75 (Act I), 30 (Act II),
10 (Act III), 50 (Act V); at most 3 skills per row.

## Randomness

Rule 4: one draw of the AI seed when acc + 1 ≥ 1 (rand(acc + 1)); none
otherwise.

## Edge cases & original bugs

1. Rule 7: with no counting slot and r = `DefaultChance` > 0 the
   search compares stack words not written in this call; 1.14d's choice
   is not defined by the inputs. d2rs: an unwritten w counts as −1, so
   the search ends in the rule 6 fallback (`world/hirelings.md` edge
   case 12).

## Test vectors

| Input | Expected | Source |
|---|---|---|
| Act II row Id 6 (L 9, d 0), Jab and Prayer both counting | acc = 30 + 70 + 10 = 110; rand(111); r < 30 → fallback (melee range → attack mode 4, else idle 10); 30 ≤ r ≤ 100 → Jab in mode 14; 101 ≤ r ≤ 110 → Prayer is an aura → `0x005701B0(Prayer, −1)`, idle 10 | rules 3–6, live `hireling.txt` (synthetic) |

## Provenance

`0x005E4D30` (decompiled export and disassembly; reads at `0x005E4D9F`,
`0x005E4F06`), caller `0x005E5050` (`0x005E51EA`, `0x005E5229`) ←
`0x005E52D0` (`0x005E55BB`); skill aura mask `0x006CE27C` = entry 5 of
the bit table `0x006CE268`; tactic names from `monsters/ai.md` §2.
D2MOO not used for this function.

## Open questions

1. The Hireable AI think around the call (`0x005E52D0`, `0x005E5050`:
   its threshold (98 for classes 338, 560, 561, else a counter + 40 +
   2·level capped at 95), the 50 % branch with `0x005DF530` /
   `0x005DEFE0`) is unread (`monsters/ai-functions.tsv`
   row 61 "unread"); PC 1's AI spec should cover it.
