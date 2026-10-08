# q-tp-gaps: Town Portal gaps in the play preview

Stitching session `q-tp-gaps`, branch `claude/q-tp-gaps`. Nothing here is verified against 1.14d (rule 10). PROVISIONAL points: REC-243 in `docs/HANDOFF.md` §7.

## Links connected

| Item | Before | Now | Where |
|---|---|---|---|
| Cast in town | made nothing (scroll spent) | opens the pair to the field level of the player's latest field cast; none yet → nothing | `d2-sim` `wiring/action/town_portal.rs` (`create_town_portal`, `PortalLinks::last_field`) |
| S→C 0x82 owner name | `Pending::portal_owner` had no provider, client showed no owner name | sent after 0x51 from the object's owner, the owner's join name and the pair GUID; the client stores it in the object's `owner_name` | `wiring/action/switch.rs` (`portal_owner_from_links`) |
| State 102 on the portal user | `object_just_portaled` was empty | stat list, event 12, state on until `f + 75` | `wiring/action/objects.rs` (`just_portaled`) |
| Vendor buy 0x9C action 12 | the taken store item was never told to the client | `VendorDesk::take_from_store` records it (`InteractionState::taken`), the vendors host flushes it as 0x9C action 12 | `wiring/interaction/vendor_world.rs`, `d2-server` `handlers/world/wired.rs` (`flush_taken`) |

## Tests (synthetic)

- `d2-client` `tests/app_town_portal.rs`: `casting_in_town_opens_to_the_last_field_level_and_the_owner_name_arrives` (town cast before any field cast makes nothing; Den cast → the client's portal has `owner_name`; through it to town; cast in town replaces the pair and leads back to the Den).
- `d2-sim` `wiring::action::tests::objects::portal_use_sets_state_102` (the app's synthetic game has no `states` table).
- `d2-client` `tests/e2e_vendor.rs`: `buying_a_store_item_sends_0x9c_action_12` (the buckler, not the permanent cap).

## Left

Action 4 for the bought copy is not sent here; "last field level" is only set by a cast (a walk back to town does not set it); the client does not yet remove the store item on action 12 (it ignores the message's effect on the shop grid).

## Local check

```
cargo run -p d2-client --release -- play --new sorceress Test
```

Cast a Town Portal in the Blood Moor and go through it. In the Encampment cast another scroll: a portal opens beside you and leads back to the Blood Moor. Hover a portal: it carries your name. Buy a non-permanent item from Akara or Charsi: it leaves the shop grid.
