# rc-player-fc hand-back (2026-10-10)

Branch `claude/rc-player-fc` (staging-7 + rc-player-hit merged). REC-1960 used.

## Cause
The `fc` 256 vs 0 was not the player. At the player's death the 1.14d
corpse (second player-type unit, g 2, mode 17) reads fc 256, sp 256.
A player allocation leaves mode 0, so `0x0057F700`'s `0x00624690(C, 17)`
is a new mode and runs the re-init `0x00624390` (frame count, rate).
d2rs wrote `mode = 17` directly (`wiring/action/dying.rs new_corpse`), so
the corpse kept fc 0 / sp 0.

## Fix
- d2-sim `new_corpse`: set the mode through `units::modes::write_mode`.
- d2-client `snapshot_host`: a mode 17 player without a host quest
  record gets `q: []` (1.14d corpse reads `[]`). PROVISIONAL REC-1960.
- Spec: `specs/sim/units.md` §4.1 (corpse paragraph).

## Checks (orig-cache reused)
- gen-boss-708: 82 -> 100 equal ticks; first divergence frame 83 (fc) -> frame 101 (monster seed).
- gen-boss-* (25): no first divergence on `fc` any more; 4 PARTIAL, 21 DIVERGED, 2032/3750 ticks equal.
- diff-a3/a5: no `fc` first divergence; first divergences now monster `m`
  (2 vs 1), missile class 58 missing, game seed after warp (a3 non-bm).
  Before-count not re-measured (EQUAL was 0/36 before and is still 0).

## Open
1. Corpse quest record: where 1.14d allocates it for mode 17 (S, needs Ghidra read of `0x005B1880` path).
2. gen-boss: monster `m`/`fr` at frames 31-60 (monster AI, M); seed after
   hit (gen-boss-708 frame 101, M); item/missile ty offsets (156, 267).
Gates: fmt, clippy (d2-sim, d2-client), nextest d2-sim 4738 pass.
