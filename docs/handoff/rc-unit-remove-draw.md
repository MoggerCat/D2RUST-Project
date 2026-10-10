# rc-unit-remove-draw (REC-2630..2639 unused)

Task: the "game-seed draw at unit removal" cluster (7 checks, example gen-mon-475, frame 117, `lifecycle.rs:180`).

## Result: no code change
- On this branch (staging-7 + integ-r23) `gen-mon-475` no longer shows the draw:
  rng channel MATCH 141/141; state channel 150/150 ticks equal (verdict PARTIAL
  with 100.0% match, unexplained, not looked into). The old frame-117 game-seed
  difference (rc-mon-fr ledger row) is gone, so the cluster's example is already
  fixed by earlier work (likely the REC-2065 frame-advance fix rc-mon-fr cites).
- `lifecycle.rs:180` is `draw_unit_seed`, i.e. an unit *allocation* draw, not a
  removal draw; the causes file label ("at unit removal") was a guess.

## Open
- The other 6 checks of the cluster are not named anywhere in the repo
  (causes file lists only the example). Finding them needs the full
  `gen-mon-*` run (335 checks, ~1.8 h on 3 workers). Not run: over budget for
  one cause. Whoever has the r16 suite json: list rows whose first difference
  is `lifecycle.rs:180` and re-run just those.
- gen-mon-475 state PARTIAL at 100%: check the verdict rule in suite.py.
