# fin-items-saves coverage note

Short session; only part of the file set was reached.

## Covered counts (unit "any" column)

| File | Before | After |
|---|---|---|
| formats/d2s.md | 88 / 133 | 92 / 133 |
| formats/d2s-load.md | 5 / 19 | 6 / 19 |
| formats/d2s-appearance.md, items/generation.md, treasure.md, inventory.md, inventory-moves.md, properties.md | unchanged | unchanged |

## Changes

- `d2-server::adapters::character::refusal_message` (new): load §5 r2 table
  (result → m → string id, with the 0x20 config-bit variants). Test in
  `character/tests_fitems.rs`.
- `d2-formats/src/d2s/tests_fitems.rs`: claims d2s.md §2.4 r3, r7, §2.5 r1, r3.

No code fixes to existing behavior.

## Left, with reasons

- d2s-appearance.md (all 21 rules): no code exists. The token table, hand
  owners, body-armour parts and colour byte need the item, armtype, affix and
  state tables wired into a new module; too large for this slot. Next
  session should add it (d2-sim items or d2-server adapter) and the
  `Header::reset_appearance` call site.
- d2s-load §3, §4, §6, §7, §8 r2/r4: need the join/load wiring.
- Not reached: generation §10.3/§12, inventory §4.9/§5.7/§5.8, inventory-moves
  §8.5, properties §13, treasure §9, remaining d2s.md §8.x rules.
