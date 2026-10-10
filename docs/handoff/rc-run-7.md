# rc-run-7 hand-back (runner; first half of gen-lvl/gen-skill by sorted name = 173 checks)

- Run: suite.py --filter gen-lvl-*,gen-skill-* (--fill-cache, 3 workers). The process died at ~11:31Z
  after 133 checks; relaunched 12:03Z for the 40 missing first-half checks.
- Verdicts at 12:25Z: 164 of 173 done: 128 EQUAL (REC-2055/2056 rules), 9 DIVERGED, 27 PARTIAL.
- PARTIAL (27): gen-skill-* checks have an `input` line and no packets channel, so REC-2055 cannot
  settle them; their ledger rows are UNKNOWN with last_verdict PARTIAL (state 0 differences, rng/state clean).
  Adding a packets channel to the generated skill checks (tools/check-gen) would settle them. Size S.
- DIVERGED (9): see docs/handoff/rc-run-7-causes.tsv (9 distinct first differences, mostly rng draw
  site mismatches in monsters init/create.rs, ai/mod.rs, population/room.rs, combat/damage.rs).
  Crate/size columns are a first guess from the d2rs site named, not investigated.
- Left: 9 checks not finished by the stop: gen-skill-ama-8, gen-skill-ama-9, gen-skill-ass-251..257.
- Second half (173 checks) belongs to rc-run-7b. No code changes.
- Ledger part: docs/handoff/ledger/rc-run-7.tsv (136 rows; ledger.py --check clean).
- traces/orig-cache recordings are not committed (reverted).
