# rc-goto-settle hand-back

Cause: `tools/trace-recorder/poke.py` `_goto` called `_settle` twice at the landing (poke.md §6 r3.2: once),
giving 1.14d an extra S->C 0x07 and 0x15 y+1. Fixed: one call.

## Checks (packets channel, 14 goto checks, orig-cache re-recorded under Wine, --fresh)
- Before (rc-net-s2c): EQUAL 0/14, 61 divergent frame/stream pairs, first divergence at the landing in all.
- After: EQUAL 8/14 (atma, drognan, elzix-trade, fara-trade, greiz-hire, greiz-hirelist, lysander,
  net-s2c-kashya-hirelist). Landing divergence gone everywhere.
- Still DIVERGED (6, 12 frames off in total, not pairs counted):
  - cain-talk f50 (1.14d buffer #0 missing in d2rs, 1 frame)
  - elzix-gamble f18 s2c #0 byte 13 (1.14d 0 vs d2rs 0x10, id 0x9c; 1 frame)
  - fara-heal f7 buffer #1 size 66 vs 53 (4 frames)
  - jerhyn-talk f14 s2c #3 0x8a missing in d2rs (3 frames)
  - meshif-talk f45 buffer #0 missing in d2rs (1 frame)
  - warriv-talk f24 s2c #0 id 0x8a vs 0x67 (2 frames)
  These are new first divergences, later than the landing; not investigated (one root cause per session).

## Notes
- orig-cache re-recordings are local only (not committed, per brief); rerun with
  `suite.py --dir traces/checks --filter 'a2-npc-*' --orig-cache traces/orig-cache --fill-cache --fresh`.
- Ledger part: docs/handoff/ledger/rc-goto-settle.tsv (0x07/0x15/0x51/0xAC rows, the 5 equal checks).
- Not run: the 17 goto checks without cache.
