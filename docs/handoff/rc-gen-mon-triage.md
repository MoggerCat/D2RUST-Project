# rc-gen-mon-triage hand-back (2026-10-10)

Branch = staging-7 + `claude/integ-r16` + this fix; REC-2165..2169 unused.

## Checks (gen-mon family, 335 checks, state + rng)

| run | EQUAL | DIVERGED |
|---|---|---|
| ledger before (stale rows) | 139 | 196 |
| r16, fresh 1.14d recordings | 229 | 106 |
| r16 + this fix | **235** | 100 |

`suite.py --checks-dir traces/checks/gen --filter 'gen-mon-*' --orig-cache
traces/orig-cache` (no 1.14d timeouts); no check went EQUAL -> DIVERGED.

## What changed

- Largest cluster (19 checks): a missile fired by a naturally spawned
  monster had `lvl` (stat 12) 1; 1.14d gives monstats `Sk<i>lvl`
  (+ skill bonus): the used skill entry (`0x00620250`, list +0x10) is the
  init step 14 entry. d2rs: the client's monster seam (`MonsterAi`)
  mirrored only summon entries and fell back to 1. `sync_seams` now
  mirrors `natural_skills` too (summons override, as `ai.rs`
  `skill_level`). The same fix reached staging meanwhile (46add4d6, per-skill
  merge); the sync kept staging's code, so this branch adds the
  `specs/monsters/init.md` §6 step 14 note only.
- 613–616, 722, 723 EQUAL; the other 13 diverge later (causes file).
- `docs/handoff/rc-gen-mon-causes.tsv`: the 100 remaining first
  differences in 20 clusters (cause, count, example, suspected 1.14d fn). Ledger part `docs/handoff/ledger/rc-gen-mon-triage.tsv`: 265 rows re-settled on r16 (rows rc-maggot-seed / rc-mon-fr / rc-mon-tx / rc-pets hold left to them).

## Open (sizes)

- monster x off by 2 at the first steps: 14 (M); missile 1.14d lacks
  (class 144/675): 11 (M); draw at `0x005A55BA` on hit / player mode 4 vs 5:
  10 (M); AI think extra draw `monsters/ai/mod.rs:412`: 8 (S); game draw
  at unit removal `lifecycle.rs:180`: 7 (S); monster dies early: 7 (M);
  player sp 64 vs 128: 7 (S); the rest ≤ 6 each (causes file).
- `fr` (3) belongs to rc-mon-fr; monster item `own` (3) to rc-seed-order.
- Pre-existing on r16, not mine: `d2-client --test e2e_full_loop`
  (3) panics at `e2e_full_loop.rs:2362` (0x9C slice of 4) with or without this fix.
