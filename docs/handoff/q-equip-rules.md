# q-equip-rules: equipment rules on in play

## Links connected
- `InvParts::new` (d2-server `items/moves.rs`) turns on `InvState::equip_rules` and `link_item_stats`: requirements (§4.2, strength / dexterity / level / identified) and body-location rules apply in play and in the move tests.
- The hang: `inventory_pass` step 4 (`items/inventory/bookkeeping.rs`) looped while a sweep switched an item on, and "linked" (`InvWorld::item_active_on`, `wiring/inventory/inv_world.rs`) was a rest seam that answered false, so the loop never ended. It now reads the stat list (attached to the wearer) when `link_item_stats` is on; `EquipWorld::stat_unlink` uses `unlink_item_stats`. The sweep count is also capped (d2rs-own guard).
- A body remove ran the inventory pass directly, leaving its owner refresh queued until the next message (extra 0x47 / 0x48 bytes). `InvDesk::flush_equip` (d2-sim `wiring/inventory/mod.rs`) runs at the end of `MoveRun::call`.
- Tests: `swap_one_handed_with_two_handed` unchanged and passing with the switch on; new `equip_over_strength_requirement_is_refused`.

## PROVISIONAL: REC-253 (see `docs/HANDOFF.md` §7).

## What's left
Skill list / mouse-slot seams of the preview rest are still defaults (weapon bookkeeping selects no throw skill).

## The user's local check
`cargo run -p d2-client --release -- play --new barbarian Test`, press **I**: pick up an item with a strength requirement above the character's and try to wear it (it stays on the cursor); a start item wears normally and its stats show on the character screen.
