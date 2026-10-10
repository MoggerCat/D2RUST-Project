# rc-coverage-warps: hand-back (REC-2160..2164, none used)

Base: claude/specs-staging-7 + claude/integ-r16. Coverage cluster: 29 checks (16
named warps, 11 milestones, combat-elements, warp-cold-plains), `suite.py --filter …
--orig-cache traces/orig-cache --no-playthrough --workers 3`.

## Checks before / after (this branch)

| | MATCH | PARTIAL | DIVERGED | ticks equal |
|---|---|---|---|---|
| r16 as merged | 15 | 23 | 6 | 5050 / 5325 (94.8%) |
| + this fix | 15 | 23 | 6 | 5156 / 5325 (96.8%) |

- r16 already clears every warp: their rng channels (were DIVERGED at frame 2, seed order)
  are MATCH, state channels PARTIAL (all frames equal).
- milestone-hellforge, milestone-hephasto: 91/177 -> 144/177 frames equal.

## Root cause fixed (one, two sides of it)

1.14d `0x005CDFB0` SrvDo148 DoomKnightMissile fires `srvmissilea + nComponent[10]`
(S3, monster data +0x0E) at the used entry's level. d2rs read both wrongly:
- `component()` in `UseView` and the AI host went to `Pending::ai_component`, a stub
  returning 0 in the live game: every monster's components read 0 in skill bodies/AI.
  Now read from the lent monster world (`ActionHooks::monster_data`), seam as fallback.
- `ActionHooks::used_skill_of` for a monster without a sim skill list took the client
  seam's base (summons only, else 1). Now the base of its `monster_skills` entry, else
  its `natural_skills` entry (`Sk<i>lvl` + bonus), as the AI's `skill_level` reads them.
Result frame 83: three doomknight2 fire 324/323/324 at level 3, as 1.14d.
Spec note: specs/skills/bodies-4.md §3.25. Test: `used_skill_takes_the_monster_entry_level`.

## Ledger
- Part docs/handoff/ledger/rc-coverage-warps.tsv: 74 coverage rows re-settled from this
  run (63 DIVERGED -> NO-CHECK/PARTIAL, 11 DIVERGED with the new first frame). Coverage
  rows don't take a session part's override in `ledger.py merge` (first coverage part
  wins), so the same rows were also written into coverage-checks.tsv.
  Coverage group: DIVERGED 113 -> 68, NO-CHECK 217 -> 262, UNKNOWN 269 (unchanged).
  Ledger EQUAL 894 -> 894 (the state channel of every passing check is PARTIAL).
- docs/handoff/checks-status.md: the 44 rows of these 29 checks replaced from this run
  (noted under its header); `ledger.py --fix` then rewrote last_verdict in
  integrator, monsters, world, rc-pc1late-world, q-fix-d3-player-mode, q-fix-d9-arcane.

## Open (sizes)
- Hellforge/Hephasto frame 145: an extra d2rs missile 675 (owner g 64, lvl 7) — M.
- milestone-baal-throne / worldstone-portal frame 82: succubuswitch3 (476) mode 9 vs 2 — M.
- milestone-nihlathak frame 30: class 472 mode 1 vs 2 — M. milestone-izual frame 28:
  game seed differs — M.
- 269 UNKNOWN coverage rows have no check; the `aN-warp-lNN-*` checks that would cover
  them have no orig-cache recording (each needs a 1.14d run under Wine) — L.
- Pre-existing: `tools/coverage.py --check` fails on crates/d2-sim/src/debug/state/tests.rs:409
  (malformed rule `§2 \`own\``), a file this branch does not touch.
