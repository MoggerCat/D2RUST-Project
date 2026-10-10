# rc-packets-join-order hand-back (2026-10-10)
Branch `claude/rc-packets-join-order` = `claude/specs-staging-7` + `claude/integ-r17` + this work.
## Census (524 generated checks with a packets channel)
`docs/handoff/rc-packets-census.tsv`. The first divergence is the same
layered pair in 448 checks (gen-missile 264, gen-state 184):
1. frame 2, s2c #1 0xAA 12 vs 39: the join burst queued one frame late
   (rc-packets-s2c owns it; not touched here).
2. frames 3-4, s2c #0 0x07: the `4 warp 2` poke ran between frames on d2rs;
   1.14d's hook runs it at the tick return of frame 3, so the landing room's
   0x07 leaves in frame 3's flush. **Fixed here.**
Behind those: gen-missile-149 (0xA8 missing, frame 12), gen-netc2s-60
(0x23 vs 0x47, frame 20). gen-missile-101 errored on the first run (variant
build race between parallel workers), ran clean on the second.

## Change
`d2-client state-dump` runs every poke due after a tick (frame >= 2) at the
tick-end hook (`Host::frame_with`, before the flush), the 1.14d point
(`specs/tools/poke.md` §4 r2, §5 r5); only pokes before the first tick stay
between frames (`app/poke.rs` `split_tick_end`, `app/state_dump.rs`).
A narrower try (warp only) left the `stat` poke's update one frame late
(a2-npc-fara-heal frame 7, combat-potion-midfight frame 59), so all pokes move.

## Checks before -> after (orig-cache, same 1.14d recordings)
- gen packets checks: packets MATCH 74 -> 74 (the join burst still comes
  first in 450); equal packet frames 27494 -> 28459; suite equal ticks
  57893 -> 58940 (97.7% -> 99.2%); `packets_diff --from 3`: 74 -> 522 MATCH.
  No check lost an equal frame in any channel (state 25840 -> 25910,
  rng 4559 -> 4571).
- 31 hand-written poke checks with a cached 1.14d packets recording (after
  only; the pre-fix run died on a full disk): packets MATCH 22/31 under
  the general rule vs 21 under warp-only; equal ticks 3227 vs 3221.

## Open (sizes)
- S: after a `pos` poke d2rs sends an S->C 0x15 (d2rs-own reassign
  request, poke.md §1) that 1.14d does not, at either poke point:
  interact-operate-waypoint, interact-talk-akara, items-vendor-akara-buy
  frame 4. Newer than checks-status.md (r17).
- S: gen-missile-149 0xA8 frame 12; gen-netc2s-60 0x23 frame 20.
- `d2-client` tests: lib tests only (the full debug test build does not
  fit the cloud disk next to the release suite build).
