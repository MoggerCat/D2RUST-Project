# rc-unit-guid-order: hand-back

Branch `claude/rc-unit-guid-order` (from `claude/integ-r10`). **No code
change: the cause does not reproduce on integ-r10.**

## Checks (`--filter 'gen-lvl-*,gen-ai-*'`, 282 checks, 418 runs)

| | EQUAL checks (every channel MATCH) | runs MATCH / PARTIAL / DIVERGED |
|---|---|---|
| before | 0 | 104 / 184 / 130 |
| after | 0 (no code change) | same |

State first divergences: game seed 34, monster `m` 20 / `fr` 19, others
(tx, x, y, s, hp); none is a unit present on one side only, apart from
two missiles that 1.14d creates and d2rs doesn't (gen-ai-clawviperex f43 3:1
class 652, gen-lvl-98 f78 3:6 class 17). No GUID offset in any of them.

## GUIDs per frame, both sides (state channel, seed 1234)

- `combat-kill-fallen` (ama) and `combat-melee-fallen` (bar), Blood Moor
  with its population: monsters 1–7 town NPCs, 8–18 the population
  (classes 63 / 19 / 5, same order), the poked fallen party 19–21; objects
  1–22 and warps 1–2 are the same on both sides at frames 2, 4, 5 and 30.
- `gen-ai-fallen` setup (variant blood-moor-empty), 1.14d recorded
  fresh: the fallen party is 8–10 on both sides.
- `combat-melee-fallen` packets: C→S 0x06 is `06 01000000 15000000` (GUID
  21) on both sides; the "21 vs 19" in q-tool-replay-diff item 4 and the
  `net.c2s.0x06` ledger note is stale (population fixed upstream). The
  check now diverges elsewhere: state f46 game seed, packets f3 S→C 0x07
  missing in d2rs (not unit order).

## Changed

- `docs/handoff/ledger/rc-unit-guid-order.tsv`: `net.c2s.0x06` note
  updated (bytes equal; still DIVERGED through the check's other ids).

## Open

- The 1.14d recordings of the 282 checks went into `traces/orig-cache`
  (local, not committed); later sessions on this container reuse them.
- First divergences above belong to the AI / seed owners (sizes M each).
