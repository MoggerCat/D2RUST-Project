# q-run-missiles-states: hand-back (REC-1730 used; 1731-1734 unused)

## Done
- `check-gen` families `missile` (265 checks, one per Missiles.txt row a missile.* ledger area names) and `state` (184, one per states.txt row), `traces/checks/gen/gen-missile-<id>`, `gen-state-<id>`. `--check` and `--selftest` green. `ledger-areas.tsv` gained the 481 missile.*/state.* areas.
- `suite.py --dir DIR` and comma lists in `--filter` (flags only). `tools/check-gen/census.py` (`run`, `post`, `report`) runs the checks in batches through the suite, re-compares each run from the poke frame on (state ignoring `q`, missile units alone, every unit, the frames before the poke, rng, packets) and writes `docs/handoff/q-run-missiles-states-census.tsv` (one line per check) and the ledger part `docs/handoff/ledger/q-run-missiles-states.tsv`. `ledger.py --check`: 0 errors.
- Missile check: `poke missile <id> @x @y @x+6 @y [skill <id> <lvl>]` at frame 12 in the Blood Moor next to a cow (class 179); channels state rng packets. State check: `poke state` on the player and the cow at frame 12, off at 24; channels state packets. Both carry `ignore q` (d2rs does not write the quest flags, REC-1625: it hides every other first difference from frame 2).
- 1.14d recordings are in `traces/orig-cache/gen-*` (text only).

## Verdicts (481 rows: 297 missile, 184 state)
| | rows | DIVERGED | PARTIAL (no difference in anything compared) | exercised on 1.14d |
|---|---|---|---|---|
| missile.* (skills 172+8, coverage 117) | 297 | 5 | 292 | 293 yes / 4 no |
| state.* | 184 | 0 | 184 | 171 yes / 13 no |
PARTIAL rows are ledger state NO-CHECK as in the other parts (the state channel never says MATCH: `own` and the headless client are not compared). No row is EQUAL for that reason. The 13 states with no S->C state message on 1.14d (not sent): avoid, criticalstrike, dodge, evade, golem-mastery, inferno, itemset6, justhit, nomanaregen, runeword, skel-mastery, summonresist, warmth.
No row is no-check-possible: every poke ran on both sides.

## Divergences, by owner (route.py), repro `python3 tools/scenario-diff/suite.py --dir traces/checks/gen --filter <check> --no-playthrough --workers 1 --orig-cache` (then `census.py post <check>`)
- **claude/q-fix-seed-order** (missile rng draws): gen-missile-546 (baal taunt control, rng f38: 1.14d draws at 0x5b0597, d2rs none), gen-missile-407 (catapult charged ball, f36: d2rs draws in `path/walk/missile.rs:122`, 1.14d none), gen-missile-411 (catapult spike ball, f36: 1.14d draws at 0x5d5c89, d2rs none).
- **claude/q-fix-join-items** (S->C 0xA8 SetState): gen-missile-149 (shout, packets f12: 0xA8 missing in d2rs).
- Shared cause of everything else: none; 292+184 rows equal in what is compared.

## Open
- The ledger's other 8 missile rows that had old DIVERGED/PARTIAL verdicts from cast checks (fire-trauma, hurricane, tornado, volcano, war-cry, battle-orders, mind-blast, eruption) now read PARTIAL here: the poked missile alone matches; the cast checks' divergences are in the cast, not the missile body. Both stay in the notes of the skill rows.
- A poked missile tests the missile body and its hits, not the srvdo path that a real cast takes (skill cast checks do that); the monster-skill casts (monster AI chooses the skill) are not covered.
- Level choice (20 class skills / 10 monster skills) is PROVISIONAL REC-1730.
- `tools/coord/ledger.py` only knows `traces/checks/*.check`, not `gen/`, so the part's `checks` column is `-` and the check names are in the note (coordinator: teach `Repo.checks` the gen folder, then the column can name them).
- Repro of the whole census: `python3 tools/check-gen/census.py run --family missile; ... --family state; ... report` (about 4 h, 3 workers).
