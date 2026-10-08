# Handoff: unique, set and rare items (`claude/q-item-uniques`)

## Links connected
- Already working (no change): drops roll quality, unique/set fixed stats and rare affixes in d2-sim (`items/quality.md`, `affixes.md`); the tip colours the name per quality (`ui/item_tip.rs`).
- Missing link found: worn items never fed the wearer (`stat link 0x0063D1D0` unwritten; the preview rest answered nothing), so fixed stats and set bonuses did nothing.
- Now: `InvState::link_item_stats` (`d2-sim` `wiring/inventory/item_link.rs`, on in `preview_inv_parts`): equip attaches the item's stat list to the player, leaving the body detaches it; set pieces run the set-item update and set bonuses (queued, because the owner's inventory is lent out during the call); a leaving piece frees the owner list and the pieces still worn are re-evaluated.
- Tip: a set item ends with its set name (gold).

Tests: `wiring::inventory::tests::link` (3: stats reach and leave the wearer, switch off, two-piece set bonus on and off); `ui::item_tip::tests` (unique gold, set green + set line).

## PROVISIONAL (REC-161)
See `docs/HANDOFF.md` §7. Set bonus lines are not drawn in the tip; body slots 1–10 only count for sets.

## Left
- Not checked on live tables. No end-to-end drop-to-tip test on the synthetic host (it has no unique/set tables).

## Local check
`cargo run -p d2-client --release -- play --new amazon Test`, kill monsters until a green/gold item drops (or use a save with one), pick it up, hover it in the inventory (I): the name is gold (unique) or green (set) with the set name last. Equip it: the character panel stats change by the item's properties; with two pieces of one set the set bonus adds; removing one takes it off.
