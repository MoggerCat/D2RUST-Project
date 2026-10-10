# rc-shrine-missile hand-back

Checks (gen-shrine-*, state channel, orig-cache, 120 ticks):
- before: 16 clean, 3 DIVERGED at f40 `game.seed` (19, 21, 22).
- after: 16 clean, 3 DIVERGED at f40, but on a later field: the game.seed gap
  is closed. Not yet EQUAL; gen-missile sample not run.

Changed:
- `ObjectView` `ShrineWorld::create_missile` (`0x0059FA30`) now builds the
  parameter record (flags, owner, origin, start = shrine position, target
  relative or absolute by flag 0x20, level) and calls
  `missiles::create_missile` on the store. Spec: objects.md §9.3a.

Open:
- 19: f40 `hp` 13824 vs 27648: storm life loss needs `units_in_range`
  (unwired, REC-2095). Size S-M.
- 21/22: f40 item `ik` (item seeds of the dropped potions) differ; drop
  order/seed of `0x00582AC0` potions vs missiles to trace. Size M.
- Not run: clippy for other crates, nextest, gen-missile sample.
