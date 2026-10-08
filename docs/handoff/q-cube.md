# q-cube: the Horadric Cube in the play preview

Branch `claude/q-cube`. Everything here is d2rs-own, unverified (rule 10); REC-116.

## The path, and where it was cut

Right press on the cube in the inventory → `ItemsUi::right_press` → C→S 0x20 UseGridItem → server `use_grid_item` → cube open (S→C 0x77 0x15, interaction type 4) → client `msg_ui` (ui 0x1A on, inventory mode 0x0E) → `CubeUi` → C→S 0x2A / 0x18 / 0x19 (grid), 0x4F 0x18 (Transmute), 0x4F 0x17 (close) → server `CubeParts` (`cubemain` recipes) → results placed on page 3 → 0x9C / 0x9D back.

| # | Link | Was | Now |
|---|---|---|---|
| 1 | Right press on the cube | **cut**: the right press knew only identify items | `inv_items_tip.rs::right_press` returns the intent: 0x20 with the item and the player's subtile position (`bridge/items.rs::use_grid`); `original.rs` forwards it |
| 2 | Server opens the cube | **cut**: 0x20 reached `use_item_at` (unwritten) and the cube was never opened (the e2e test staged the interaction) | `d2-sim` `use_grid_body` calls the new `MovePending::open_cube` for code `box `; `wiring/inventory/cube_open.rs` sets the interaction and queues 0x77 0x15 (0x77 0x11 first if the stash was open); the cube is not consumed |
| 3 | The cube is enabled in the play host | **cut**: `WiredWorld::cube` was `None`, so 0x2A / 0x4F were stubs | `CubeData::from_fixed` (`world/cube.rs`) loads `cubemain` + item/stat/experience columns from the user's tables; `single_player.rs` sets `world.cube = preview_cube_parts(..)` for live data (`PreviewCubePending`, `handlers/world.rs`) |
| 4 | A panel for ui 0x1A | **cut**: the state opened but no panel was installed | `ui/cube_ui.rs` (`CubeUi`, installed in `OriginalUi::install`): `CubePanel` art, close / transmute buttons with the original's press / release rules (`StashCubeInput`), page-3 items and grid click |
| 5 | Page-3 items drawn and clicked | **cut** | `ui/panels/cube_items.rs`: `draw_cube`, `press_cube` over the inventory's grid click with page 3; grid = `inventory.bin` record 9 / 25, estimated fallback |
| 6 | The inventory on the right half | gate refused ui 1 with the cube | `UiRoot::sync_states` shows the inventory with ui 0x1A too (as for the stash, REC-104) |
| 7 | Transmute results | server (`cube.md` §3–§8, already tested in `e2e_single_player`) | reached now through links 1–3; results come back as the existing 0x9C / 0x9D item messages |

## Tests (synthetic)

- `d2-sim` `wiring::inventory::tests::cube_open`: 0x20 on a `box ` item sets interaction (4, cube), queues 0x77 0x15, keeps the cube.
- `ui::panels::inv_items_tests`: right press on the cube sends 0x20 with the player position (and not for a potion / with a cursor item); the cube grid draws page-3 items only, lifts with 0x19, places a cursor item with 0x18 page 3.

## PROVISIONAL points

REC-116 (see `docs/HANDOFF.md` §7).

## What is left

- No cube in a new character's start items and no quest reward path in the preview: to try it you need a `box ` item (a drop or a changed start-item table). I did not add one.
- Nothing runs the whole chain on real tables headless (the synthetic `GameData` has no item or cube tables); the server half is covered by the existing `e2e_single_player` transmute, the sim half by the new test. `CubeData::from_fixed` is checked only by the user's local run.
- Transmute animation (`panels.md` §12.4), tool tips, the cube-gone close; strings are `q-strings`.
- The cube is not saved with the character (REC-105 / REC-115 item persistence).

## The user's local check

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
$env:RUST_LOG = "warn,d2_client=info"
git fetch origin claude/q-cube; git checkout claude/q-cube
cargo run -p d2-client --release -- play --new sorceress Test
```

1. The log shows no `cube tables` error at start. If it does, send that line.
2. You need a Horadric Cube (`box`) in the inventory (the preview gives none; use a changed start-item table or a drop).
3. Press **I**, right-click the cube: the cube panel opens on the left (art, close and transmute buttons, an empty 3 × 4 grid), the inventory on the right.
4. Click an item, then a cube cell: it is drawn in the cube. Put the recipe inputs in (for example three perfect gems of one kind) and click the transmute button: the inputs go and the result appears in the cube.
5. Close with the close button or Esc. Send the log lines with `item`, `cube` or `ui:` if a step shows nothing.
