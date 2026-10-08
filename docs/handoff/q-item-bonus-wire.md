# q-item-bonus-wire (REC-188)

Links connected
- `d2-client/src/bridge/item_lists.rs`: decodes the item stream (0x9C / 0x9D) into `ItemData::props` (stat, layer, value << ValShift) when the record arrives (`msg/stats_items.rs` `item_action`); `attached_to` decides from the last record whether the list counts (body 1-10 / 13+, not 11 / 12; item flag 0x4000 clear; charm on page 0); `ClientWorld::total` adds `item_lists_total`. Move to grid / ground / belt detaches (spec §2 r4.1).
- Tables: `Bridge::set_item_tables`, installed in `app/play.rs` next to `ItemTips` (`ItemTips::tables`).
- Server: the pseudo states 0xFE / 0xFD are gone from `d2-sim` `vitals_sync.rs`; the 0x1D stays base only. Test `linked_item_stats_are_never_sent_and_the_base_stays` replaces `linked_item_stats_follow_as_a_state_list` (transport changed to the spec's, not a weakened check).
- e2e: `d2-client/tests/app_item_bonus.rs` (+5 str, +10 max life, +20 fire res: total = base + item, base unchanged, unequip restores).

PROVISIONAL: see REC-188 in HANDOFF §7 (set items, which lists attach; gems).

User's local check
- `cargo run -p d2-client --release -- play` (see LOCAL-RUN), equip an item with +resist / +stats: the character panel shows the total, blue when above base. Unequip restores it.
