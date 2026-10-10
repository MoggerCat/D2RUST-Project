# rc-l78-population: Level 78 (jungle 3) population

Branch claude/rc-l78-population. REC ids REC-2035..2039: none needed.

## Checks (a3-warp-l78-jungle-3-ama, gen-lvl-78)
- Before: a3-warp-l78: rng DIVERGED 132/133 (frame 21, draw 1195, site
  0x54ed96 vs none); state diverged from frame 21 (units 7, 8 class 249 absent).
- After: a3-warp-l78: rng MATCH 133/133, state 160/160 frames equal, 0 differences
  (verdict PARTIAL = channel coverage only). gen-lvl-78: rng MATCH 133/133,
  state 160/160 equal, 0 differences.
- The 1.14d recording for gen-lvl-78 was made fresh; not committed (orig-cache rule).

## Cause
`tile_records` (crates/d2-sim/src/wiring/worldgen/population.rs) set `water: false`
for every floor record, so the frog-demon water point (0x005B2700, classes 247-249;
population.md §9.2) never found a tile. Class 249 (frogdemon3) presets in L78
were not created, which also left the density-roll chain 2 draws short.
Fix: water = DT1 tile material flags (0x00604BC0 returns u16 at tile +0x06) & 2,
read through `Drlg::tile_info`. The spec already described this; no spec edit.

## Objects 251/252
Not touched separately: with the class-249 monsters placed, the state channel has
no object differences in either check (the earlier "absent after touring rooms"
note in q-fix-rd-act3.md should be re-checked with its goto-preset scenario; I did
not re-run it).

## Open
- Run the q-fix-rd-act3 room-tour scenario for 251/252 (S) to confirm.
- Other levels with frog demons (L66-67 Kurast sewers are not; Flayer jungle
  levels 76-79 and Act 3 river) should now be re-checked: a3-warp l76/l77/l79 (S).
- No mutation/unit test added: the fake-host water test covers §9.2 logic; the
  host wiring has no unit-level DT1 fixture (S).
