# rc-gen-skills hand-back (REC-2530..2539 unused)

Task: checks for the 164 NO-CHECK player-skill rows (skill.ama/ass/bar/dru/nec/pal/sor).

## Result
- The `gen-skill-<cls>-<id>` family (fam_skill in tools/check-gen/check_gen.py) already covers
  every class skill row (210 checks); no generator change was needed.
- Ran all 210 with suite.py (orig-cache hit, d2rs side fresh, 70 frames each):
  203 PARTIAL (70/70 frames equal, own/client fields not compared), 7 DIVERGED.
- Ledger part docs/handoff/ledger/rc-gen-skills.tsv: 164 rows, each now lists its gen-skill check.
  157 stay NO-CHECK (PARTIAL cannot be EQUAL: scenario-diff does not compare own/client),
  7 -> DIVERGED. EQUAL count unchanged: 1488 -> 1488.

## Open (causes, not fixed; game code not touched)
- ass-257 (frame 28 game seed differs), ass-279 (frame 48 monster class 418 `m` 7 vs 1),
  bar-155 (frame 28 player `st` 133196 vs 128550), dru-222/231/241 (frame 67 monster tx 5143 vs 5144),
  nec-93 (frame 32 missile 193 ty 4268 vs 4269). Each is S-sized; owners: skills/sim.
- Blocker for EQUAL on all rows: state_diff does not compare own/client fields (M-size, tooling).
