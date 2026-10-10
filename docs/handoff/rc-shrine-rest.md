# rc-shrine-rest hand-back

Checks (gen-shrine-*, state channel, orig-cache, 120 ticks):
- before: 16 clean (PARTIAL = no diff), 3 DIVERGED at f40 (19 hp, 21/22 item ik).
- after: 19 clean, 0 DIVERGED.

Changed:
- Storm shrine (`0x00582DA0`): `ShrineWorld::units_in_range` is wired on
  the unit finder (`0x0065A950` / `0x0065AC70`) with the shrine's own filter
  `0x00582710` (player mode not 0/17, monster mode not 0/12, other types
  refused) and the `0x005541B0` dead test. That filter has no distance
  test: the radius only picks the rooms. `unit_find_by` (missiles/
  bodies_ext2.rs) is the finder with a caller filter; `unit_find` uses it.
  The trait method is now `&mut self`. Unit mode is read from the unit
  record (the view's `MissileBodies::unit_mode` is the default 0).
- Exploding / poison potions (`0x005830E0`, `0x00583410`): the item
  request's quality word (+0x30) is 0, so the quality is rolled on the item
  seed (5 item-seed draws); the gem drop keeps quality 2.
  `near_player_drop` takes the quality.
- Spec: specs/world/objects.md §9.3. Ledger: ledger/rc-shrine-rest.tsv.

Open:
- Not run: gen-missile sample, other crates' clippy/nextest (d2-sim only:
  clippy and 4769 tests pass).
- 0x00582710 has no Ghidra function (bytes read from Game.exe, not in
  names.tsv); names.tsv not touched.
