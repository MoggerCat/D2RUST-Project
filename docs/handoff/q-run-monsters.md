# q-run-monsters: hand-back (2026-10-09, branch `claude/q-run-monsters`)

Scope done: the per-class monster rows `monster.<id>` of the ledger (335, group
coverage; all NO-CHECK before). New check-gen family `mon` (one check per
monstats row that has a ledger row; `gen-mon-<hcIdx>`): Blood Moor, empty
variant, `spawn <hcIdx> @x+5 @y-4 normal` at frame 30, 150 ticks, channels
`state rng`, `ignore q seed` (the player's quest list and the game seed differ
on every check of every family at frame 2 / 30: harness level, not the
monster; with them ignored a clean state run is PARTIAL with 0 differences,
which the harvest reads as EQUAL together with rng MATCH).

## Result: 335 rows verdicted

| Verdict | Rows |
|---|---|
| EQUAL (state 150/150 and rng all frames) | 112 |
| DIVERGED | 223 |
| PARTIAL / no-check-possible | 0 |

Ledger part: `docs/handoff/ledger/q-run-monsters.tsv` (group `monsters`, so
the coverage row of the same area only adds `exercised`); `ledger.py --check`
0 errors. Causes with counts and example classes:
`docs/handoff/q-run-monsters-causes.tsv`. Owner column from `route.py`; where
it says "unrouted" the row names `claude/coord-resume-3` (monster AI/combat).

## Divergences, grouped (forward to the owners; repro `python3 tools/scenario-diff/scenario_diff.py traces/checks/gen/gen-mon-<N>.check --orig-cache traces/orig-cache`, needs Wine; the 1.14d side is not committed, see Open)

| First divergence | Rows | Owner | Example |
|---|---|---|---|
| frame 31, monster field `m`: 1.14d mode 2 (walk) on the first frame after the spawn, d2rs mode 1 (neutral): 1.14d runs the AI think on the spawn frame | 87 | claude/coord-resume-3 (monster AI) | gen-mon-69 |
| player `fc` / `m` / `sp` / `hp` / `s` at frame 35–145: the monster's first attack or hit-recovery lands on other ticks | 33+9+4+5+5 | claude/coord-resume-3, hp: q-fix-join-items | gen-mon-140, gen-mon-93 |
| monster `fr` (anim frame) at frame ~100 | 28 | claude/coord-resume-3 (anim rate) | gen-mon-117 |
| monster `tx` / `ty` / `x` (walk target / step) at frame 31–38 | 17+4+8 | rendering/path owner (route.py) | gen-mon-39, gen-mon-492 |
| monster seed `s` at frame 31 (spawn consumes other draws) | 6 | claude/q-fix-seed-order | gen-mon-11 |
| game seed at frame 41 | 6 | claude/q-fix-seed-order | gen-mon-190 |
| rng: game draw #1 at frame 29, site 0x552e31 missing in d2rs (classes 540–542, Ancients) | 3 | claude/q-fix-seed-order | gen-mon-540 |
| rng: unit draw #4 at frame 112, 1.14d site 0x56e132 vs d2rs skills/levels.rs:1123 | 3 | claude/q-fix-seed-order | gen-mon-615 |
| missile `lvl` at frame 45 (classes 530–533: d2rs gives 1, 1.14d the monster skill level) | 4 | claude/q-fix-join-items | gen-mon-530 |
| missile `cl` at frame 44 (24 vs 22) | 1 | unit/missile owner | gen-mon-60 |

The dominant cause (87 of 223, plus most of the player rows that follow it) is
one: the spawn frame. Fixing the AI's first think on the spawn frame will move
many first divergences later, so expect the second layer to show then.

## Changes outside check-gen (small, flagged for the coordinator)

- `tools/scenario-diff/suite.py`: new flag `--checks-dir` (default
  `traces/checks`) so the suite runs `traces/checks/gen`.
- `tools/coord/ledger.py`: `Repo.checks` also globs `traces/checks/gen/*.check`
  (otherwise a ledger row cannot name a gen check). `tools/coord/` is not in
  my area: please keep or redo this one-line change.
- `tools/check-gen/harvest_mon.py`: suite `--json` results -> the ledger part
  and the causes table (`--selftest` not needed: it is a reporting step).

## Open

- Rows of the `monsters` group left out on purpose (named by q-chk-monsters /
  q-run-gen-ai / q-run-gen-bosses, or needing a hand-authored scenario): the
  `monster.ai.*` / `superunique.*` / `umod.*` / `boss.*` rows (generated
  checks exist; they have the other sessions' scope), `monster.quest.*` (12),
  `monster.population.*` (3), and the system rows corpse-death, minions,
  missiles, ai-bodies. No PC 1 item was needed.
- The 1.14d recordings of the 335 checks (977 MB of text, mostly the rng
  channel) are **not committed**: too big for the repo. Each costs about 30 s
  of Wine to regenerate; to keep them, compress or sample them first.
- The class-level checks spawn at one place in Blood Moor. Monsters that need
  their own level (Act 4/5 only graphics, bosses' lair setup) are compared
  there too; a divergence rooted in the level would show in `lvl` checks.

## Commands

```sh
export D2_GAME_DIR=$HOME/game
python3 tools/check-gen/check_gen.py            # writes traces/checks/gen incl. gen-mon-*
python3 tools/scenario-diff/suite.py --checks-dir traces/checks/gen --filter 'gen-mon-1??' \
    --workers 3 --orig-cache --fill-cache --no-playthrough --json out.json
python3 tools/check-gen/harvest_mon.py out*.json   # -> ledger part + causes
python3 tools/coord/ledger.py --fix && python3 tools/coord/ledger.py --check
```
(6xx and 7xx classes need `gen-mon-6??` / `gen-mon-7??` as well.)
