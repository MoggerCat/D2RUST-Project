# q-ledger-skills handoff

Done: `docs/handoff/ledger/skills.tsv`, 588 rows (210 class skills = 7 x 30, hireling skill sets per hireling type, item-granted skills, generic/monster-skill pointer rows, states.txt rows, missiles grouped by the skill that makes them, 5 system rows). States: NO-CHECK 562, DIVERGED 26 (every per-skill check is DIVERGED or PARTIAL in checks-status.md; PARTIAL counts as DIVERGED).

Limits (stated in the rows):
- `last_verdict` comes from `checks-status.md` (q-fix-check-triage, suite of 17:34 UTC). Skills with a per-skill check are bar-, dru-, ass-; ama/sor/nec/pal have none (exercised only by classes.play milestones).
- `exercised` is `yes` only where a check runs the skill; `provisional` counts provisional-index.tsv rows that name the skill (a text match, so it undercounts).
- Spec per skill is derived from `specs/skills/functions.tsv` (srvst/srvdo index to body spec).
- Monster skills (charclass blank, 156-356) are one pointer row; they belong to the monsters part.

Repro: sparse-clone the private repo `extracted`, then run the generator (kept in the session scratchpad, not committed): reads `Patch_D2.mpq/data/global/excel/{skills,states,Missiles,hireling}.txt`.
