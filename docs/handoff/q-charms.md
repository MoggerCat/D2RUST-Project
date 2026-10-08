# q-charms: charms and inventory stat effects

Stitching session, 2026-10-08. Nothing here is verified against 1.14d (rule 10); preview fills are PROVISIONAL (REC-163, M22).

## Links connected

| Link | Before | Now |
|---|---|---|
| Worn items feed the wearer | done by REC-161 (`wiring/inventory/item_link.rs`, switch `InvState::link_item_stats`) | unchanged |
| Charm placed on inventory page 0 → stats count | nothing: the inventory pass (`inventory.md` §5.7 step 2) ran on the rest, a no-op in the play host | `InvWorld::inventory_pass` and the pending `inventory_pass` also run `link_charms` (`item_link.rs`): each active inventory item (§5.6: type `char`, page 0, not broken, requirements met) whose list is not linked gets `link_item_stats`. The sweep needs the owner's inventory, so it is queued as `EquipCall::Charms` when the inventory is lent out |
| Charm leaves page 0 (to cursor, swap, drop, belt) | list stayed linked | `charm_unlink` (pending.rs) also runs `unlink_item_stats` |
| Jewels | a jewel in a socket is the filler's list on the item (q-sockets); a loose jewel has no effect | the socketed jewel's stats reach the wearer through the worn item's list (REC-161 test `a_socketed_gem_reaches_the_wearer`). Jewels are not charms |
| Stat totals in the character panel | the server sent only base values of stats 0-5, 7, 9, 11, 12, 30; the client builds no item lists | `wiring/action/vitals_sync.rs`: the watched set adds defense (31) and the resists (39-46); for stats 0-3, 31, 39-46 the value sent is base + the sum of the item lists linked to the player. The client reads it as the base, so the panel shows the total. Sent once per change, so equipping, taking off, charm in, charm out all follow |

## PROVISIONAL (REC-163)

- The stat link/unlink of a charm is read from inventory pass step 2 and from `charm_unlink`; the bodies of `0x0063D1D0` / `0x0063D2B0` are unwritten (`inventory.md` OQ6).
- Item bonuses ride in the stat message value (base + items) because the client builds no item stat lists (`client/stat-lists.md` open question 2). The panel therefore shows no blue "bonus" colour: base equals total on the client. Max life, mana and stamina (7, 9, 11) and other stats (damage, attack rating, magic find) do not carry item bonuses yet.
- `EquipCall::Charms` runs at the end of the inventory call, not inline.

## Tests (synthetic)

- `d2-sim` `wiring::inventory::tests::equip::a_charm_in_the_inventory_counts_and_stops_when_picked_up`: charm to page 0 → defense +5; lifted → back; reinserted → +5.
- `d2-sim` `vitals_sync::tests::linked_item_stats_follow_as_stat_messages`: linking an item list sends 0x1D for defense and strength once; detaching sends the zeros.

## What is left

- Saved characters: items loaded from a save are not stat-linked at the join (no inventory pass runs there).
- Item bonuses on max life/mana/stamina, damage, attack rating and the other panel rows; the blue/red colouring.
- Charms with skills (+skill levels), and the client-side item lists.

## Local check

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
git fetch origin claude/q-charms
git checkout claude/q-charms
cargo run -p d2-client --release -- play --new sorceress Test
```

Pick up a charm (a drop, or one from a vendor) and put it in the inventory grid; open the character panel (C): defense/resists/attributes the charm adds appear. Pick the charm up again: they go. Headless: `cargo nextest run -p d2-sim wiring::inventory::tests::equip vitals_sync`.
