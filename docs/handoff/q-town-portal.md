# q-town-portal: Town Portal scroll and tome

Stitching session `q-town-portal`, branch `claude/q-town-portal`. Read
`specs/`, `docs/`, `crates/`, `tools/` only. Nothing here is verified
against 1.14d (rule 10).

## The path, with the links that were missing

| Link | Before | Now |
|---|---|---|
| Right-click an inventory item | no client send | `ItemsUi::use_press` (`ui/panels/inv_items.rs`), called from `InventoryUi::event` (`ui/original.rs`): C→S 0x20 UseGridItem with the player's point |
| 0x20 → `use_grid_body` → `use_item_at` | the seam answered false | `InvDesk::use_portal_item` (`d2-sim/src/wiring/inventory/town_portal.rs`): `tsc ` / `tbk ` recorded in `InvState::portal_requests`; a tome loses a charge; `consume_item` of a scroll goes to `remove_used_item` (0x9D flag 0x20) |
| Server handler → action wiring | none | `items::moves::handle` takes the requests from `MoveRun` and calls `WorldHost::town_portal` → `ActionSim::open_town_portal` → `View::create_town_portal` |
| Portal pair | `Pending::create_portal` was a no-op everywhere | `wiring/action/town_portal.rs`: class 59 in mode 1 near the player, class 59 in mode 2 at the act town's spawn point (`wiring/path/place.rs::level_spawn`), `PortalLinks` (`ActionHooks::portals`), owner and `InteractType` set; S→C 0x51 to the player for the field portal |
| Operate 15 (`objects.md` §12) | `portal_partner`, `player_portal_guid`, `remove_portal` defaulted | `ObjectHost` seams (`wiring/action/objects.rs`) read `PortalLinks` first; removal is queued (the object state is lent during the call) and run by `flush_portal_removals` after `operate_object`, with S→C 0x0A to the operator |
| Travel | existing `misc::portal` | unchanged; arrival in town by `place_unit` (room switch sends the town portal) |
| Synthetic tables | no class 59, no `leveldefs` | `WaypointTables::synthetic` has the portal row (operate 15, init 11) and blank `leveldefs`; `LocalSeams::object_quest_record` answers true |

Tests (synthetic): `d2-client/tests/app_town_portal.rs` (Den of Evil →
cast → field portal reaches the client → click → town → click the town
portal → back in the Den, both portals gone on the server and the client;
fails before: no pair), `wiring::inventory::tests::town_portal` (request,
tome charge, other items), `d2-server` `use_town_portal_scroll_and_tome`
(0x20 through the host), `inv_items_tests` right-click.

## PROVISIONAL (REC-117 in `docs/HANDOFF.md` §7)

- Item-use table entry of `tsc` / `tbk` unwritten: codes stand in for it.
- `0x0056D130` body unwritten: spots, owner, links, player data +0x48 =
  the field portal's GUID (so rule 12 removes the pair on the way back
  from town, as the queue row says); a cast in town makes nothing and
  still spends the scroll; a new cast removes the previous pair.
- Removal notice (0x0A) to the operator only; state 102 not set; no
  quest records in the preview.
- The portal's 5000 ms hostile delay (`objects.md` §12 rule 2) is read
  from the host clock, so it refuses a click in the first 5 s of a game.

## What's left

- Casting in town to reach the last field level (needs the last portal
  level recorded; the original opens the portal to it).
- The 0x82 ownership message needs `Pending::portal_owner` (no provider
  in the preview), so the client shows no owner name.
- The Town Portal as a skill (skill 220 through `srvdo 113`) is not
  reached; only the item use.
- Not run against game data here (cloud).

## The user's local check (game files)

```
cargo run -p d2-client --release -- play --new sorceress Test
```

You need a Town Portal scroll in the inventory (add one with the start-item
cheat, or buy `Scroll of Town Portal` from Akara). Walk out to the Blood
Moor, open the inventory (**I**), **right-click** the scroll. Expect: the
scroll leaves the inventory, a blue portal appears next to you; wait 5 s
from game start, click it: you arrive in the Rogue Encampment next to a
second portal; click that one: you are back in the Blood Moor and both
portals are gone. Send log lines with `portal`, `object` or `rejected` on
failure.
