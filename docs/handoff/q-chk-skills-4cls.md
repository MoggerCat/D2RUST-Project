# q-chk-skills-4cls hand-back (REC-1415..1419 reserved; no provisional choices made)

## Done
- 107 skill checks authored, one per non-passive skill of ama/sor/nec/pal: `traces/checks/{ama,sor,nec,pal}-<skill>.check` (dru-tornado pattern: save `Scn<Cls>` level 30, all skills 20, right skill = skill id; Blood Moor variant; cow class 179 at +4; one rclick at frame 20; state channel). Passives (no cast) skipped.
- All 107 run under Wine (scenario-diff, suite.py, 4 workers): **80 PARTIAL** (70 frames compared, no difference; PARTIAL only because of the known `own`/client gaps) and **27 DIVERGED**. No EQUAL because scenario-diff never reports MATCH for the state channel today.
- Ledger part `docs/handoff/ledger/q-chk-skills-4cls.tsv` (107 rows; validated with ledger.py, 0 format errors). PARTIAL rows are state NO-CHECK; DIVERGED rows carry first frame and owner.
- Routing: one row `q-fix-skill-chk-4cls` in `docs/handoff/build-queue.tsv`; owners in the ledger rows are q-fix-class-rows (summon mode, paladin auras, charge, blessed hammer) and q-diff-skills-2 (cast target, charged bolt, inferno, telekinesis, bone prison/spirit).

## Divergences (first difference)
- Cast target: nec raise-skeleton, skeletal-mage, bloodgolem, irongolem, firegolem, revive, corpse-explosion, poison-explosion: frame 20 player tx 0 (1.14d) vs 5151 (d2rs).
- Summon mode: ama-dopplezon/valkyrie f37 m 1 vs 2; sor-hydra f26 9 vs 2; nec-clay-golem f27 8 vs 2.
- Paladin: fanaticism f2 sp 172 vs 128; vigor f2 st; holy-fire/freeze/shock/sanctuary f51 state seed; prayer f51 mp; charge f20 x 5144 vs 5146; blessed-hammer f31 missile xf.
- sor-charged-bolt f27 missile x +1; sor-inferno f39 game seed; sor-telekinesis f20 m 1 vs 5; nec-bone-prison f20 m 1 vs 5; nec-bone-spirit / bone-wall f28 missile missing in d2rs.

## Open
- The generated checks of claude/integ-r6 (traces/checks/gen/) were not merged; these hand-made ones have a cow target, the generated ones none. Neither orig-cache was filled.
- Passive skills have no check. Weapon-needing skills (bow, javelin) cast without the weapon in both sides.

## Repro
`python3 tools/scenario-diff/suite.py --filter 'nec-*' --no-playthrough --workers 4` (D2_GAME_DIR=/root/game; `[ansp][mocra][arcl]-*` matches ama/sor/pal; `nec-*` separately), or one check: `python3 tools/scenario-diff/scenario_diff.py traces/checks/<name>.check`.
