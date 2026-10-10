# rc-run-2 hand-back (runner, no code changes)

Stopped 12:20Z (partial). Command: `suite.py --filter 'b*,c*,d*,h*,i*,j*,m*,n*,p*,r*,s*,u*,w*' --orig-cache traces/orig-cache --fill-cache --workers 3 --no-playthrough`.
Its own `--json` is written only at exit, so the suite was still running when this was written; verdicts are read from `traces/raw/suite/*/result.json`.

- Checks finished: 183 of 393 in scope (the unfinished ones are the later names, i.e. mostly the second half, rc-run-2b's).
- EQUAL under the ledger rules (REC-2055/2056): 113; DIVERGED: 70. "Before" count not measured by this runner.
- Ledger part: `docs/handoff/ledger/rc-run-2.tsv` (87 rows EQUAL, 70 DIVERGED). Rows for the unfinished checks are not in it.
- Causes: `docs/handoff/rc-run-2-causes.tsv`, one row per distinct first difference, largest first.
- `docs/handoff/checks-status.md`: rows of the 183 checks replaced.

Largest causes (checks): monster record field `m` at difficulty a3-a5 hell/nm (23, specs/monsters/init.md, M);
state PARTIAL outside the REC rules because of an `ignore` line (8) or input/send without a packets MATCH (7);
item drop streams (hp/mp/rare codes, ~30 checks, specs/items/treasure.md, M each);
hireling resurrect extra monster unit (4, client-messages.tsv); one draws CelDraw op (draws-town-arrival-ama); one packets s2c id (rng-town-idle-sor, S).

Open: `ledger.py --check` reports ~150 contradictions in older parts' rows (their last_verdict is older than the refreshed checks-status); my part overrides them in the merge, so the integrator should run `ledger.py --fix`. I did not.
`traces/orig-cache` recordings were refilled by the run and are left uncommitted on purpose (brief: don't commit them).
