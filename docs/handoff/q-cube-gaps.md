# q-cube-gaps: cube in the start items, tool tips, cube-gone close

Branch `claude/q-cube-gaps`. d2rs-own, unverified (rule 10); REC-244 (renumber at the last fetch).

| # | Gap (q-cube "what is left") | Link made |
|---|---|---|
| 1 | No cube for a new character | `WiredWorld::start_extra` (`d2-server` `handlers/world/wired.rs`) + `start_items` (`handlers/world.rs`) append one inventory copy per extra code after the charstats slots; `app/single_player.rs` sets `[*b"box "]` |
| 2 | Panel stays open when the cube leaves | `OriginalUi::cube_poll` (`ui/cube_ui.rs`), called each pass beside `shop_poll` in `world_view/ui_bind.rs`: SetUIState(0x1A off) + C→S 0x4F 0x17 twice (`StashCubeInput::cube_gone`) |
| 3 | Button tool tips | `CubeUi::draw`: hover on the strict button rectangles draws `strClose` (4144) / "Transmute" (3341) at `cube_tooltips`; nothing while strings are `NoStrings` (they appear with q-strings) |

## Tests (synthetic)

- `test-fixtures/tests/start_items.rs::a_new_character_has_the_cube_when_the_host_names_it_as_an_extra` (new fixture module `test_fixtures::cube_item`; without the extra the character still has exactly the original's three items).
- `d2-client` `ui::cube_ui::tests::the_cube_panel_closes_when_the_cube_leaves_the_inventory` and `the_cube_buttons_show_their_tool_tips_on_hover`.

## PROVISIONAL / left

- The cube is a preview gift, not the Act II quest reward (REC-244).
- `StashCubeInput::cube_opened` (latch clear on open) has no non-test caller; a second open/close in a session may send one 0x17 only from the button path. Not changed here.
- Transmute animation (§12.4) still not drawn.

## The user's local check

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
git fetch origin claude/q-cube-gaps; git checkout claude/q-cube-gaps
cargo run -p d2-client --release -- play --new sorceress Test
```

1. Press **I**: the inventory holds a Horadric Cube beside the potions/weapon.
2. Right-click the cube: the cube panel opens left. Hover close / transmute: tool tips show once strings load (q-strings).
3. With the panel open, pick the cube up (left click) and drop it on the ground or in the stash: the cube panel closes at once.
