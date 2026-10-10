# rc-client-path-follow: hand-back (nothing to fix on this base)

Checks: `play_smoke the_live_run` (release, real data, `D2_GAME_DIR=$HOME/game`):
PASS on `claude/specs-staging-7` + `claude/rc-client-melee-approach` merged,
with no code change of mine. EQUAL counts unchanged (no scenario check run).

## What I did
- Merged `claude/rc-client-melee-approach` into staging-7 and ran the test
  first, before touching anything: it passes (31.8 s).
- Read the client side: `bridge/predict.rs::tick` re-reads the unit
  target's model position every tick and passes it to
  `ClientPath::tick` (`PathTo::Unit(.., pos)`), so the path already
  follows the model; the sim's arrival check (`pathing.md` §9.5 r3,
  re-path when the target moved > 5) gets the fresh position. Target
  lead is 0 in 1.14d (§3 step 4), so nothing is missing there.

## Open
- The melee-approach hand-back's failure no longer reproduces on this
  base; staging-7 apparently carries what it lacked. Its remaining items
  (real `melee_range` + pending-attack loop `0x00481600`, the 0x06/0x0D
  approach prediction) are unchanged and not needed for this test.
- I did not run the rest of the d2-client suite or the 1.14d reading
  of the client unit-target code: with the test green there was no
  divergence to chase.
