# q-ledger-skills handoff

Done: `docs/handoff/ledger/skills.tsv`, 588 rows (210 class skills = 7 x 30, hireling skill sets per hireling type, item-granted skills, generic/monster-skill pointer rows, states.txt rows, missiles grouped by the skill that makes them, 5 system rows). States: NO-CHECK 562, UNKNOWN 26.

Limits (stated in the rows):
- `last_verdict` is `-` everywhere: `checks-status.md` / branch `q-fix-check-triage` was not reachable. The 26 skills with a per-skill check (bar-, dru-, ass-; none exist for ama/sor/nec/pal) are therefore UNKNOWN, not EQUAL.
- `exercised` is `?` (coverage sessions fill it); `provisional` is 0 (not mapped per skill).
- Spec per skill is derived from `specs/skills/functions.tsv` (srvst/srvdo index to body spec).
- Monster skills (charclass blank, 156-356) are one pointer row; they belong to the monsters part.

Repro: sparse-clone the private repo `extracted`, then run the generator (kept in the session scratchpad, not committed): reads `Patch_D2.mpq/data/global/excel/{skills,states,Missiles,hireling}.txt`.
