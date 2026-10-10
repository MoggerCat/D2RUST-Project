# rc-mon-spawn-think: hand-back (2026-10-10, branch `claude/rc-mon-spawn-think`)

Base: `claude/integ-r10` + `claude/q-run-gen-bosses` (merged for its checks).

## Checks (suite, `traces/checks/gen`, orig-cache; EQUAL = every channel MATCH/PARTIAL-clean)

| Set | Before | After |
|---|---|---|
| the 82 checks of q-run-monsters' "frame 31, field m" cause | 21 / 82 | 21 / 82 |
| `gen-su-*,gen-boss-*,gen-umod-*` | 53 / 133 | 53 / 133 |

- The cause as filed (no think on the spawn frame) was already gone on integ-r10:
  pokes `at 30` run between ticks 29 and 30, so the +2 think of `0x00573780`
  lands on frame 31 on both sides; 21 of the 82 were already EQUAL before any change.
- What was left at frame 31 is per-AI (22 checks, 4 groups).

## What changed

- `0x005DC640` "can reach directly" was a `Pending` stub that always said false (no
  provider). Implemented from the Ghidra export: full-size distance -> offset k
  (2/3/4/3), three line probes `0x006229F0` (unit to point, point size 2, mask
  0x805), true when any is clear (`path::line::can_reach_directly`, wired in
  `View`). Unit tests added. Affects ReanimatedHorde, precheck C's special walk
  (`!can_reach` was always true), bodies 6/7 secondary-target filters.
- ReanimatedHorde step 3 draws with the `roll(100)` helper, as 1.14d does (`0x005E161A`).
- Specs: `ai.md` §6 (0x005DC640, 0x006229F0 rows) + Provenance; `ai-bodies-5.md` §8.
- Result: gen-mon-436/437/438 first divergence 31 -> 62 (player `m` 1.14d 5 vs d2rs 19:
  next layer, the Charge hit). No check got worse.

## Open (first divergence frame 31, field m)

- Megademon 362/686/687/712 (1.14d stays neutral, d2rs walks): 1.14d draw #1 at
  `0x005E0EF4` that d2rs lacks; read `0x005E0C80`. S.
- PutridDefiler 546-549 (same symptom; rng channel gave no first-diff). S-M.
- VileMother 298-300/675/676 and ClawViper 77 (1.14d mode 14, d2rs 1). M.
- FetishShaman 279-281/662/664 (1.14d 4, d2rs 2): coordinator: `0x005F169A`
  secondary target, row q-fix-pc1late-secondary-target (not mine).
- Other first causes in the 82: monster `fr` (20, from frame 36), game seed at
  85/135 (6: SandMaggot egg 0x552e31), player `s` at 53 (5).

## Ledger

`docs/handoff/ledger/rc-mon-spawn-think.tsv`: 82 rows (23 EQUAL, 59 DIVERGED) re-verdicted.
They repeat areas of `q-run-monsters.tsv`, which wins in name order, so `ledger.py`
lists them as conflicts: coordinator, please drop or replace those rows there.
