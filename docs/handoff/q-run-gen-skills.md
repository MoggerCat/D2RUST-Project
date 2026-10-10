# q-run-gen-skills: hand-back (2026-10-09, branch `claude/q-run-gen-skills`)

## Done

All 210 `traces/checks/gen/gen-skill-*` checks (30 skills x 7 classes) ran on
both sides (1.14d under Wine, d2rs `state-dump`), channel `state`, 70 frames
each. 1.14d sides are in `traces/orig-cache/gen-skill-*` (text only).

| Verdict | Checks |
|---|---|
| EQUAL | 0 |
| DIVERGED@2 | 210 (all; field `q`) |
| of which, `q` ignored: no difference (PARTIAL) | 161 |
| of which, `q` ignored: a later divergence | 49 |
| PARTIAL as raw verdict / no-check-possible | 0 |

Rows outside the 210: the 70 `skill.monster.*` / `item` / `sequences` / class
rows with last_verdict `-` have no check in this family; they need a scenario
that makes the monster or item cast (not a table-driven check). Not done here.

## Shared first divergence (all 210): `q`, frame 2

1.14d writes the player's quest flag record `[[0,24704],[1,801|802..],[2,768],
[16,24832],[17,..],[18,768],[32,24960],[33,..],[34,768]]` from frame 2;
d2rs writes `[]` (never initialises those words at game creation for these
saves). Not skill-specific. Owner: quests/join (`claude/q-fix-npc-interact`).
Spec: `specs/tools/state-snapshot.md` row `q`, `specs/world/quests.md` 1.1/1.4.
Repro: `python3 tools/scenario-diff/scenario_diff.py traces/checks/gen/gen-skill-ama-6.check --orig-cache traces/orig-cache`

## Skill-specific divergences (with `q` ignored), by first field

Re-diff: `python3 tools/trace-recorder/state_diff.py traces/raw/suite/<name>/orig.state.jsonl traces/raw/suite/<name>/d2rs.state.jsonl --ignore q`
(after the suite run below). Owner `claude/q-diff-skills-1`, `-2`.

| Field | Checks (frame) |
|---|---|
| tx (14) | ass-253, ass-275 @20, dru-222 @31, ... |
| m (10) | ass-268, ass-279 @48, dru-227 @58, ... |
| xf (6) | ama-32 @51, dru-225, dru-240 @30, ... |
| s (5) | dru-221 @29, pal-102, pal-114 @51, ... |
| seed (4) | ass-257 @28, dru-230 @39, dru-249 @37, ... |
| sp, st (2 each); fr, hp, ty, x, mp, y (1 each) | bar-151, pal-122, bar-155, pal-115 @2, ass-266 @40, ... |

Full list: this table is rebuilt by the ledger note of each row
(`docs/handoff/ledger/q-run-gen-skills.tsv`, column `note`).

## Run

`python3 tools/scenario-diff/suite.py --checks-dir traces/checks/gen --filter 'gen-skill-*' --workers 3 --no-playthrough --orig-cache --fill-cache`
(new flag `--checks-dir`; first run builds d2rs in release, ~12 min).

## Open

- Ledger merge: `q-chk-skills-4cls.tsv` / `-bda.tsv` / `skills.tsv` sort before this part and
  the first part per area wins, so 197 of my 210 rows are dropped as conflicts (13 landed).
  Coordinator: drop those areas from the earlier parts, or fold my `checks`/`last_verdict`/`note` in.
- `tools/coord/ledger.py` (not my area): `Repo.gen_checks` lets `checks` name `traces/checks/gen/*.check`
  (completeness rule unchanged). Without it --check fails 210 times.
- `last_verdict` is the raw DIVERGED@2 (q). The skill-specific verdicts are in `note`.
