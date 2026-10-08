# q-weapon-combat: equipped weapon and worn stats in combat

Nothing here is verified against 1.14d (rule 10); fills are `d2rs-own, unverified`, REC-158.

## Links connected
- Equip/unequip now link the item's stat list to the wearer (`InvDesk::link_item_stats`/`unlink_item_stats`, called from `InvWorld::stat_link`, `MovePending::stat_link`, `body_leave_effects`, `stat_unlink`). Item base damage (stats 21/22/23/24) and gem/rune fillers (already attached to their target by q-sockets) reach the wearer's totals.
- Combat reads the weapon: `LocalSeams` `Pending::current_weapon`/`weapon`/`item_at`/`wield_type` answer from `app/weapons.rs` `Weapons` (grip 2 for two-handed base items).

## Tests
- d2-sim `wiring::inventory::tests::equip::a_worn_weapon_gives_its_damage_to_the_wearer`, `a_socketed_gem_reaches_the_wearer`.
- d2-client `app::weapons::tests::combat_reads_the_weapon_in_use`.

## Left
Weapon swap slots, requirement/durability effects, dual wield, saved-character reload of links, a server-level melee hit test, client attack art.

## Local check
`cargo run -p d2-client --release -- play --new barbarian Test`, equip a bought sword in the right hand (`I`): the character panel damage shows its range; kill a monster and compare hit sizes to bare hands. Headless: `cargo nextest run -p d2-sim equip` and `-p d2-client weapons`.
