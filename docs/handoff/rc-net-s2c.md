# rc-net-s2c: hand-back (S->C message divergences, one root cause)

## Grouping (q-run-net's 57 S->C rows)
- Join/goto frame shift (biggest group by records): 0x07/0x15/0x51/0xAC/0x7E rows first
  diverge in the a2-npc-* checks at frames 2-6. The streams were equal message for
  message; d2rs put each `goto` step's 0x07 one frame late. **Fixed here.**
- Missing in d2rs (~31 rows): kills, party, merc, stash, chat (not touched).
- Byte/size (~10 rows): 0xA8 15 vs 12, 0xAA 11 vs 10, 0x2A, 0x26, 0x29, 0x9C (not touched).
- 0x7E byte 4: unwritten on 1.14d (09/21/ea/fe/b8 across the cache), already masked.

## Cause and fix
`state-dump` ran `goto` steps between frames; 1.14d's poke hook runs at the tick end
(`0x0052FD1E`, before the flush). Each step's placement 0x07 (`Pending::send`) also
stayed in the sim until the next tick's `take_sent`. Now:
- `d2-client` `app/poke.rs` `split_tick_end`: a frame with `goto` runs at the tick end like
  `operate`/`talk`. The old error for goto plus talk in one frame is gone. New `queue_sent_now`.
- `app/state_dump.rs`: the tick-end hook steps the walks (state kept between frames) and
  queues the sim's sends into this frame's buffer. The between-frames runner no longer handles goto.
- `d2-server` `Host::queue_now` (buffer queue + packet tap), test
  `a_tick_end_queue_leaves_in_the_same_frames_flush`.
- `specs/tools/poke.md` §5 rule 5 updated.

## Checks (packets channel, 1.14d from traces/orig-cache; all 14 goto checks with a cache)
EQUAL 0 -> 0. Divergent frame/stream pairs 157 -> 61. First divergence frame 3 -> the goto
landing frame (atma 3->12, cain 3->14, drognan 3->11, meshif 3->16, elzix/fara/lysander 3->6,
greiz 3->7). jerhyn, warriv and kashya-hirelist land on the first step, so they stay at frame 3, #0 -> #2.

## Open (next cause, not mine)
- **Recorder bug, owner claude/q-fix-replay-hooks:** `tools/trace-recorder/poke.py` `_goto`
  calls `self._settle(...)` twice in a row (the found branch). `poke.md` §6 r3.2 settles once.
  On 1.14d the landing gives 3x 0x07 (d2rs 2) and 0x15 y +1. That is the first divergence in all 14
  checks. Fix: drop the duplicate call, then re-record those 14 checks' orig-cache (S, needs Wine).
- My edit touches `app/state_dump.rs` / `app/poke.rs`, which belong to claude/q-fix-replay-hooks.
- Not run: the 17 goto checks without a cached 1.14d side (a1/a2 quest, a2-wp-*).

## Ledger
`docs/handoff/ledger/rc-net-s2c.tsv`: net.s2c.0x07/0x15/0x51/0xac stay DIVERGED with the new cause (REC ids unused).

## Repro
`python3 tools/scenario-diff/scenario_diff.py traces/checks/a2-npc-atma-talk.check --orig-cache traces/orig-cache --channels packets`
