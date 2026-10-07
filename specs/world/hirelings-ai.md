# Spec: World — Hireling AI skill pick (`hireling.txt` chance columns)

- **Status:** draft: read from the 1.14d `Game.exe` disassembly and
  decompiled exports (addresses inline) on 2026-10-07; no recording.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::world::hirelings`
- **Related specs:** `monsters/ai-bodies-6.md` §7 (owner of the skill
  pick, see §1); `world/hirelings.md` (owner of the hireling rules;
  §1.1 rule 6 names the row readers, §14 links here); `monsters/ai.md`
  §3.2 (Hireable AI `0x005E52D0`, think call site, tactics helpers);
  `sim/rng.md` (rand(n)); `data/fields.tsv` (`hireling`, `skills`,
  `monstats` offsets).

## Summary

The Hireable AI think (`0x005E52D0` → `0x005E5050`) picks the
mercenary's action with `0x005E4D30`: a weighted roll over the
`hireling.txt` row's `DefaultChance` and the six `Skill` / `Chance` /
`ChancePerLvl` / `Mode` slots, with a per-class fallback. The rule now
lives in `monsters/ai-bodies-6.md` §7 (step 7, "Hireling skill"); §1
here is a pointer.

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

Moved (2026-10-07) to `monsters/ai-bodies-6.md` §7 step 7 "Hireling
skill" `0x005E4D30`, PC 1's AI spec, which now owns the pick together
with the think around it (`0x005E52D0`, "Hireling attack" `0x005E5050`).
That text was reconciled with this one on 1.14d (node `Id` at +8 of the
`0x00574BD0` record, `0x005E50ED`; unwritten weights = its edge case 4)
and the rules agree. This file keeps no rule of its own.

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

1. Moved with §1 (2026-10-07): now `monsters/ai-bodies-6.md` edge
   case 4 (unwritten weights read off the stack; same d2rs choice: an
   unwritten weight counts as −1, so the search ends in the fallback;
   `world/hirelings.md` edge case 12).

## Test vectors

| Input | Expected | Source |
|---|---|---|
| Act II row Id 6 (L 9, d 0), Jab and Prayer both counting | acc = 30 + 70 + 10 = 110; rand(111); r < 30 → fallback (melee range → attack mode 4, else idle 10); 30 ≤ r ≤ 100 → Jab in mode 14; 101 ≤ r ≤ 110 → Prayer is an aura → `0x005701B0(Prayer, −1)`, idle 10 | `monsters/ai-bodies-6.md` §7 step 7.3–7.6, live `hireling.txt` (synthetic) |

## Provenance

`0x005E4D30` (decompiled export and disassembly; reads at `0x005E4D9F`,
`0x005E4F06`), caller `0x005E5050` (`0x005E51EA`, `0x005E5229`) ←
`0x005E52D0` (`0x005E55BB`); skill aura mask `0x006CE27C` = entry 5 of
the bit table `0x006CE268`; tactic names from `monsters/ai.md` §2.
D2MOO not used for this function.

## Open questions

1. Answered (2026-10-07) by `monsters/ai-bodies-6.md` §7 (Hireable
   `0x005E52D0` steps 1–13 and "Hireling attack" `0x005E5050`: a := 98
   for 338, 560, 561, else p + 40 + 2·level capped at 95; the ranged
   wander / escape branch), 1.14d-confirmed there. Original question:
   the Hireable AI think around the call (`0x005E52D0`, `0x005E5050`:
   its threshold, the 50 % branch with `0x005DF530` / `0x005DEFE0`) was
   unread; PC 1's AI spec should cover it.
