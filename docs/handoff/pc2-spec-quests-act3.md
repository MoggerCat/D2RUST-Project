# Handoff: PC 2 spec worker, quests-act3 — `claude/spec-quests-act3`

Answers to the Act III implementation questions (`impl-quests-act3.md`
open questions 1–7; HANDOFF §7 Ninth set QC-1 … QC-7). The spec now
has a part 2 (`specs/world/quests-act3-2.md` §11) so that part 1 stays
under 60 KB.

## Answered

- QC-1 (impl Q1; 17.3) → `quests-act3-2.md` §11.1: no 1.14d code sets
  17.3 (all 332 `0x0065C360` sites checked); only a save already
  carrying it reaches the §3.7 branch. Edge case 18 in part 1.
- QC-2 (impl Q2; spec OQ5, `0x00538680`) → §11.2: `quests-act1-rest.md`
  §5 with step 3 (`0x005BC182`): progression := (4|5)·difficulty + 3
  unless already higher.
- QC-3a (OQ2, `&level`) → §11.3: an output written at `0x00559AF8`
  before any read; monster source → stat 12, object → area level.
- QC-3b (OQ3, monstats +0x0D bit 6) → §11.4: `flying` (mask 0x40 at
  `0x006CE280`).
- QC-3c (OQ4, `0x005A0180` / `0x0063E9F0`) → §11.4: type flags & 0x0E
  (superunique / champion / unique) or monstats `boss`; the spawn is the
  random boss (`monsters/init.md` §16.1, champion allowed). Edge case 19.
- QC-3d (OQ6, `0x0063BEF0`) → §11.5: weapon in use (inventory +0x1C
  GUID) at body location 5, else 4, item type `weap`.
- QC-4 (map AI) → §11.6: confirmed no caller of `0x005BD040` /
  `0x005B7230` (also `0x005B7120`); the apply branches are dead.
  Edge case 20.
- QC-5 (positions) → §11.7: init record (x, y) / room; decoy timer uses
  the covering Act III room; roomless wanderer (spawn fails before any
  draw, retries; walk target (X, Y − 20); no minion dummies). Edge
  case 21.
- QC-7 (OQ7) → cross-file request below.

## Still open

- QC-6 (impl Q6, host providers for the 11 `QuestWorld` seams and the
  §10 hook callers): implementation work, not a spec question.
- Spec OQ1 (status meanings, with `quests.md` OQ1) and OQ8 (Act III
  recording): unchanged.
- The impl note's 19 "readings taken literally" were not re-read here
  (they are trace items, not questions).

## CODE-TABLE CHANGE commits

None (quests.tsv is not owned here).

## Cross-file requests

- to PC 2 quests-core: `specs/world/quests.tsv`; QC-7 / `quests-act3.md`
  OQ7; the Act III rows are fully specified in `world/quests-act3.md`
  §2–§10 and `quests-act3-2.md` §11 (callbacks checked against the rows,
  2026-10-07); set column `spec` := `specified` on the rows with
  `index` 17 (chain 14), 18 (15), 19 (16), 20 (17), 21 (18), 22 (19),
  23 (20), 24 (28) and 39 (39) — nine cells, `catalogued` →
  `specified`, nothing else changes. Also `specs/world/quests.md` §2.4
  column table, row `spec`: name the owner per act (Act III rows:
  `quests-act3.md`) instead of only `quests-act1.md` §10.

## Recording list

None new. (Spec OQ8, the full Act III run, stays as queued in HANDOFF
§5 S9-A3.)
