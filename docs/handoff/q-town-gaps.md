# q-town-gaps: town and panel gaps in the play preview

Stitching session, 2026-10-08. Nothing here is verified against 1.14d (rule 10). PROVISIONAL points: REC-231 in `docs/HANDOFF.md` §7.

## Links connected

| Item | Before | Now | Where |
|---|---|---|---|
| Single-item repair | the repairer's third button (frame 6) did nothing | it toggles a repair mode; the next left click on one of the player's items (grid page 0 or worn) sends C→S 0x35 for it, no confirm dialog; the inventory press is not a pick-up while the mode is on; the mode ends with the shop | `ui/shop_ui.rs` (`repair_click`, `ShopState::repair_mode`), `ui/panels/inv_items_repair.rs` (`ItemsUi::item_under`), `ui/original.rs` (one hook in `InventoryUi`) |
| Gheed, Charsi | not in any preview town | Act I list has both (x 36 / 8); their `monstats` rows come with `synthetic_npc_classes` | `app/town_npcs.rs`, `app/single_player.rs` |
| Elzix, Jamella | said "not placed" in `q-gamble.md` | already placed by q-a2-town (`ACT2_NPCS`) and q-a4 (`synthetic_act4::NPCS`); the new test pins all four | `tests/app_town_gaps.rs` |
| Weapon swap | W produced nothing; C→S 0x60 was a stub | W (the unhandled `SwapWeapons` action) sends C→S 0x60; the item handlers take it before the player stub: hands (4/5) and swap set (11/12) trade places, stat lists unlink / link, moved items join the update list, S→C 0x97 flips the client's set | `world_view/swap_key.rs`, `d2-server` `handlers/items/moves.rs` (`swap_weapons`), `d2-sim` `wiring/inventory/swap.rs` |
| Panel totals | only attributes, defense and resists carried item bonuses, folded into the base (no blue) | the sum of the linked item lists for stats 0-3, 7, 9, 11, 19, 21-25, 31, 39-46 rides as the stat lists of pseudo states 0xFE / 0xFD (S→C 0xA8 SetState); the base messages are the base again, so the client's total is above base and the panel's cmp colour is blue (max life / mana / stamina are in the panel's colour set) | `d2-sim` `wiring/action/vitals_sync.rs` (`item_state_changes`) |

## Tests (synthetic)

- `d2-client` `tests/e2e_vendor.rs`: `the_repair_button_then_an_item_click_repairs_that_item` (button, then a grid click → one C→S 0x35 → durability restored, gold charged); `swap_weapons_is_answered_with_0x97` (0x60 → Done + S→C 0x97).
- `d2-client` `tests/app_town_gaps.rs`: Gheed, Charsi, Elzix and Jamella are monster units of the synthetic game.
- `d2-sim` `wiring::inventory::tests::equip::the_weapon_switch_trades_the_hands_with_the_swap_set` (damage follows the set); `vitals_sync::tests::linked_item_stats_follow_as_a_state_list` (base untouched, state 0xFE stream bytes, end on detach).
- Expectations changed by the new NPCs, not weakened: `app_single_player` (two more RNG steps for Gheed and Charsi, as Kashya's was) and `app_frame_loop` (3 handled-or-queued messages each). The REC-163 test that expected base + bonus in 0x1D messages was replaced by the state-list test above (the transport is REC-231's).

## PROVISIONAL / left

REC-231. The damage and attack-rating block of the character panel is not drawn in the preview at all (`char_details` is not fed by `ui/original.rs`), so stats 19 / 21-25 are in the model but not shown. The weapon switch does not recheck requirements or durability, draws no weapon-set tabs, and a saved character's items are still not stat-linked at the join. The gamble / repair flows need game files (the synthetic game has empty vendor tables).

## Local check

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
git fetch origin claude/q-town-gaps; git checkout claude/q-town-gaps
cargo run -p d2-client --release -- play --new barbarian Test
```

- Rogue Encampment: Gheed (gamble) and Charsi (repair) stand near Akara. Click each: the NPC menu.
- Damage a worn item, open Charsi's Trade, press the third button (it stays armed), then click the item in the inventory or on the paper doll: it repairs and the gold drops.
- Equip a sword (`I`), press **W**: it moves to the swap slot and the hands are empty; equip a second weapon, press **W** again: the first returns. The character panel (C) follows the set in the hands, with life / mana / defense / resist totals in blue where worn items add to the base.
