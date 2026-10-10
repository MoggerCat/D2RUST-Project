# rc-audio-cast-run hand-back (REC-2010..2014 unused)

## Checks (audio-diff, traces/audio/*.check)
- cast-frost-nova-sor: voices 1.14d 13 / d2rs 10, paired 7 -> 12 / 10, paired 9.
  Before: coldcast.wav at T 17/57 (1.14d 18/58). After: equal at T 18/58.
  Still DIVERGED (mixed 2/99 ticks): rain2.wav T 3 vs 9; an unknown
  35015-byte sound at T 25 and 65 (buffer 17/19) that d2rs never starts.
- monster-hit-ama, town-ambience-ama, walk-town-ama: all DIVERGED
  (not changed by this fix): rain2 start (1.14d T 3-4, d2rs T 8-10), footstep
  start ticks and variant picks, dev_vol/dev_pan, one unknown sound at T 68.

## Cause and fix
- The local click runs in the loop pass after that pass's sound tick, so 1.14d
  serves the 0x004C6514 request one tick later (request f 19, voice f 20).
  d2rs played it in the click's own tick. `audio/unit_feed.rs`: the local
  player's skill-start request is held one pass (`Track::held_start`).

## Open
- rain2.wav starts 5-6 ticks late in every check (weather start; size S).
- Unknown 35015-byte sound at T 25/65 in the cast check: sound id not named
  (run compare with --sounds; likely frost nova missile; size S).
- Footstep timing/variant/pan/vol and warcry variant picks (seeded choices)
  diverge in walk/monster/town checks (size M).
- Not added: a unit test for the hold (the plan loop needs a full world).
- Gates: fmt done; fmt, clippy -D warnings, nextest d2-client --lib audio (246 pass).
