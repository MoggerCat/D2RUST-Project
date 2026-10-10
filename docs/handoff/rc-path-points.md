# rc-path-points — hand-back

Brief: gen-su-12 / gen-su-48 differ on monster tx (1 path point in 1.14d vs 3
in d2rs when walking "toward").

## Result: premise no longer holds on staging-7 (cb9a3158); no code change
Checks (`--filter 'gen-su-12,gen-su-48'`, state channel, fresh 1.14d recording):
- gen-su-12: DIVERGED, 51/150 frames equal; first divergence frame 52,
  monster 1:11 class 75, field **m** (mode): 1.14d 14 vs d2rs 1.
- gen-su-48: DIVERGED, 49/150 frames equal; first divergence frame 40,
  monster 1:9 class 494, field **fr** (frame): 1.14d 1024 vs d2rs 7936.
- EQUAL before -> after: 0 -> 0 (nothing changed). The wider
  `gen-su-*,gen-boss-*` filter was not re-run: no fix to measure.

## What I read (1.14d, private re/exports)
- `0x00679C80` toward, `0x00679A60` next-position check, `0x00679720` ray
  test, vs `crates/d2-sim/src/path/walk/find.rs` `toward`/`next_position`
  and `geom.rs` `ray_test`: same structure, same step order, same
  slack test, same greedy walk and tail rule. No difference found.
- So a 1-vs-3 point split can only come from the inputs (ray blocked in
  d2rs, clear in 1.14d: collision query / footprints, or the target), not
  from the builder. Since the earlier tx divergence is no longer the first
  one, the staging fixes (reach gate `0x005DC640`, radius geometry
  `0x005DE4E0`) most likely removed it.

## Open (sizes)
- gen-su-12 frame 52: monster mode 14 vs 1 (1.14d enters mode 14, d2rs
  stays 1) — M, an AI/mode cause, not pathing.
- gen-su-48 frame 40: `fr` 1024 vs 7936 (animation frame counter of class
  494) — S/M, animation/speed-rate cause.
- Method: break `0x005A62F5` in 1.14d for the unit and compare the path
  record; not needed for these two now.

## Notes
- Setup: `cloud-setup.sh --no-wine` leaves no Wine; checks without a cached
  recording need the full setup (rerun without the flag).
- No ledger rows: nothing settled EQUAL/DIVERGED beyond the above.
