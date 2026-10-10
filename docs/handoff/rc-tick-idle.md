# rc-tick-idle hand-back

Branch claude/rc-tick-idle (staging-7 + integ-r16). REC ids unused.

## Checks (EQUAL ticks, state / rng)
| check | before (ledger) | after |
|---|---|---|
| sys-tick-idle-a2 | DIVERGED@24 | state 300/300, rng 244/244 |
| sys-tick-idle-a5 | DIVERGED@24 | state 300/300, rng 285/285 |
| sys-units-census | DIVERGED@61 (r16: @91) | state 140/140, rng 141/141 |

The idle divergence (frame 24) was already gone on r16 before any change;
the verdict reads PARTIAL only because of unlisted fields, not a difference.

## What changed
- Census cause: the population host's `set_owner` (`0x005DD330`) only filled
  the world owner map. 1.14d writes the minion's AI control +0x0C (owner
  GUID) and +0x10 (owner type), which precheck A's minion leash reads
  (`0x005B0FF0`). Pack and boss minions therefore never leashed: a minion
  over 20 away from its boss must wander near it (`0x005DF530(owner, 19)`).
  Fix: `wiring/worldgen/population_init.rs` `set_owner` sets `ctl.owner`.
- Spec: specs/monsters/population.md §6 step 4 names the control write.
- Ledger: docs/handoff/ledger/rc-tick-idle.tsv (20 rows, EQUAL).

## Open
- None from this cluster. Other checks that spawn packs may now change
  (leash draws appear); not re-run (rule: cluster checks only).
