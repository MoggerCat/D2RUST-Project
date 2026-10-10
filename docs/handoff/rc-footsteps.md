# rc-footsteps hand-back (REC-2255..2259 unused)

Base: claude/specs-staging-7 + claude/integ-r17 (merged). EQUAL count: 0 -> 0.

## Checks (tools/audio-diff, orig recorded under Wine; differences)
| check | before | with cursor step on shared seed (tried, reverted) |
|---|---|---|
| walk-town-ama | 57 | 60 (mixed first diff T 1) |
| monster-hit-ama | 36 (r16 note) | 30 |
| town-ambience-ama | 39 (r16 note) | 41 |
| cast-frost-nova-sor | 2 | 2 |
Mixed result, no EQUAL, rain broke at T 1 in walk: the change was reverted.

## Found (walk-town-ama, seed chain)
- New recorder hooks (tools/audio-diff/record_audio.py): `roll` records for
  0x004E40A0 / 0x0045C3E0 / 0x00472280 on the player seed (+0x20), and the
  seed on every request. 1.14d runs 3 and 4 were identical (the chain is
  deterministic under Wine); a run with the first hook set differed slightly.
- d2rs and 1.14d are on the same LCG chain (start S0 = {3064641593,
  280084454}). Footstep variants differ only because the step count at each
  pick differs. Request-time step index (1.14d / d2rs): T3 24/21, T8 44/31,
  T13 60/47, T18 92/73, T22 114/96.
- Weather: each spawn is 3 roll_range + 2 raw steps = 5; the d2rs spawn
  cadence matches 1.14d except one extra spawn at C8 in 1.14d (a landed
  drop respawned; landing depends on the camera delta of drawn frames).
- Unattributed steps, ~1-2 per client update in 1.14d, are the cursor step
  0x004681C0 (one per drawn frame >16 ms apart). d2rs steps a UI copy of
  the seed (ui/cursor_ui.rs, not written back). Writing it back to the
  shared seed gave T3 exact (24) but overshoots later (T13 70 vs 60):
  d2rs draws 1-2 cursor frames per update, 1.14d fewer and uneven.

## Open
- Frames drawn per client update in 1.14d (drives cursor steps and the
  landing delta dY): needs a recorded per-update draw count (record_frames
  style) as the clock for the cursor; then write the cursor step back to the
  shared seed (size M). Sound pan/volume diffs follow once variants match.
- C8 extra respawn (particle landing with camera delta) (size S after the
  draw count).
- Local ask: pc1-data Step 4 not needed; the recorder now logs the seed.
