# q-stash: the town stash in the play preview

Branch `claude/q-stash`. Everything here is d2rs-own, unverified (rule 10).

## The path, and where it was cut

Click → `world_clicks` → `Bridge::object_interact` (C→S 0x13) → server operate
entry → `bank` (`world/objects/mech.rs`: class 267, both units in a town →
`set_interact`, S→C 0x77 0x10) → client `msg_ui` (`UiStates` ui 0x19 on, inventory
mode 0x0C) → `UiRoot::sync_states` → panel draw / input → C→S intents.

| # | Link | Was | Now |
|---|---|---|---|
| 1 | Click on the stash object, walk, 0x13, server operate, 0x77 0x10, state open | wired by stitch-objects and `msg_ui`, never run on a stash | checked by the new test (`single_player::start_with_objects` puts a synthetic stash, class 267 / operate 32, in the town) |
| 2 | A panel for ui 0x19 | **cut**: the state opened but no panel was installed (`original.rs` listed it as not wired) | `ui/stash_ui.rs` (`StashUi`, installed in `OriginalUi::install`): `StashPanel` art, close press / release with the original's rules (`StashCubeInput`, one or two C→S 0x4F 0x12), the page-4 items and grid click |
| 3 | Page-4 items drawn and clicked | **cut** (`ItemsUi` knew page 0 only) | `ui/panels/stash_items.rs`: `draw_stash`, `press_stash` over the inventory's grid click with page 4 and inventory mode 0x0C (lift 0x19, place 0x18 with page 4, swap 0x1F) |
| 4 | The inventory on the right half | **cut** (the gate refuses ui 1 while the stash is open) | `UiRoot::sync_states` opens the inventory panel with ui 0x19 (REC-103) |
| 5 | Moves between inventory and stash | server (`items/moves`, sim `game_inventory_path`) handles page 4; the client now sends it | not run end to end: the synthetic game has no item tables (see below) |
| 6 | The stash persists through the save | **cut**, and not connected here | REC-104: no item of a played character is written or loaded yet |

## Tests (synthetic)

- `tests/app_play_stash.rs`: click the stash → the server operates → ui 0x19 is open and the inventory panel is drawn beside it → a press and release on the close button closes it (fails without link 2).
- `ui::panels::inv_items_tests`: stash grid record (12 / 28, 8 / 24), a page-4 item draws at its stash cell and not in the inventory, a cursor item on an empty stash cell sends 0x18 with page 4, a stash item lifts with 0x19, a click outside sends nothing.

## PROVISIONAL points

REC-103 (grid fallback, inventory beside the stash), REC-104 (no persistence).

## What is left

- **Save / load of items** (REC-104): the task's "persists through stitch-save's save" is not met. It needs the sim item unit → `StreamItem` → `write_save` and `create_items` on load; both are item-persistence work (G14 / G16), larger than a stitch.
- The GoldMax line (needs strings by id) and the stash gold button and dialog (`panels-2.md` §21).
- An end-to-end item move with items from the server: the synthetic `GameData` has no item tables, so the server half (page-4 insert accepted, 0x9C back) is only checked by the sim's own tests.
- The stash object in the real town: it comes from the town's DS1 objects; if clicking it does nothing locally, check the log for `world click` and that object class 267 is listed in the model.

## The user's local check

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
$env:RUST_LOG = "warn,d2_client=info"
git fetch origin claude/q-stash; git checkout claude/q-stash
cargo run -p d2-client --release -- play --new sorceress Test
```

1. In the Rogue Encampment, left-click the stash chest (west of Kashya's tents). The character walks to it and the stash panel opens on the left, the inventory on the right.
2. Click an inventory item (it goes to the cursor), click an empty stash cell: it is drawn in the stash. Click it again: it lifts. (Positions are the preview's estimate, REC-103.)
3. Click the stash's close button (lower right of the panel) or press Esc: both halves close.
4. Close the game and reload with `--save`: the stash will be empty (REC-104, not done).
5. If step 1 shows nothing, send the log lines with `world click`, `ui:` or `item`.
