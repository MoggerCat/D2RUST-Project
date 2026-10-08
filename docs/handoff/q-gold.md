# q-gold: gold in the play preview (`claude/q-gold`)

> Stitching session, 2026-10-08. Read only `specs/`, `docs/`, `crates/`,
> `tools/`. Nothing here is verified against 1.14d (rule 10); every
> preview fill is marked `// d2rs-own, unverified`; the open points are
> REC-103 in `docs/HANDOFF.md` §7 (M22). No audio wired.

## Links connected

| # | Link | Before | Now |
|---|---|---|---|
| 1 | Server: 0x16 on a gold pile | `PreviewMoveRest` held no places, so `distance` was out of range and the pick-up did nothing | `MoveRest::stage` (`handlers/items/moves.rs`) hands the rest the players' and items' place and room at the start of each move call (`wired.rs` `moves`); `PreviewMoveRest` answers `pos`, `set_pos`, `distance`, `room_at`, `free_spot` (`handlers/world.rs`). The sim's `gold_pickup` (§10.1) then adds the pile to stat 14 and frees it |
| 2 | Server: 0x50 DropGold | `gold_request` answered none, so no pile was made | `PreviewMoveRest::gold_request` (plain `gld` of the game's format, ground mode); `gold_piles` (§10.2) creates up to 32 piles, stat 14 falls |
| 3 | Gold to the client | the vitals sync is on in play (`enable_vitals_sync`) and sends 0x19 / 0x1D / 0x1E / 0x1F; the client handlers were already there | tested over the bridge (`tests/app_gold.rs`): the total of stat 14 follows, the inventory line (stitch-hud `inv_gold`) reads it |
| 4 | Client: gold pile amount | `ItemView` had no amount | `ItemView::gold` reads the compact `gld` record's flag + 12 / 32 bits (`bridge/items.rs`) |
| 5 | Client: pile art | one flippy cel for every item | `ground_items.rs` loads the direction of the amount class (`unit-composite.md` §9: < 100, < 500, < 5,000, else) of the `gld` flippy and draws its last frame |
| 6 | Client: click on a far item | 0x16 once; the server's `walk_to_item` is a seam, so nothing happened | the click still sends 0x16, then `GroundItems::frame` (called next to the interact pending in `present.rs`) sends a walk to the item, waits for the predicted walk to end, and asks again (up to 3 times); it ends when the item leaves the ground |
| 7 | Client: gold button and dialog | art only (`PENDING`) | new `ui/gold_dialog.rs` (a child of `ui::original`, installed above the inventory): press sets the pressed flag (the button's frame 1), release in the rectangle opens the kind-1 dialog (`GoldButtons`, `GoldDialog`, `can_open` of `ui::gold`); digits edit the amount (`DigitEdit`, max = stat 14), Enter sends C→S 0x50 `[player GUID][amount]` (`ok_action`), Escape cancels; while open it takes every pointer event, character and action |
| 8 | Typed characters | none reached the UI | `edge::key_chars` (digits, Backspace, Enter, Escape → `UiEvent::Char`), one line in `present.rs` |

## Preview fills (`d2rs-own, unverified`)

- Dialog box: dark fill tiles and Font16 English text ("How much gold / do you want to drop?", `OK`, `Cancel`) at positions chosen here; the §28 art, spinner and string 4033 by id are not used. Enter / Escape and a raw digit map stand for the box family's keys.
- Pile creation: a plain normal-quality item at ilvl 1; the free spot is the drop start point in the player's room (no collision search); distance is the larger axis in sub-tiles.
- The walk to a clicked item is the client's (the server's `walk_to_item` stays a seam).
- The gold button's sound 4 is not played.

## Tests (synthetic)

- `d2-server` `adapters::handlers::items::moves::tests::preview_rest_picks_up_a_gold_pile` and `preview_rest_drops_gold_into_a_pile` (the real host frame over `PreviewMoveRest`).
- `d2-client` `world_view::ground_items::tests`: gold class directions, compact gold amount, the click → walk → ask-again sequence.
- `d2-client` `ui::original::tests::the_gold_button_opens_the_dialog_and_ok_sends_drop_gold`; `ui::edge::key_tests::typed_keys_become_chars`.
- `crates/d2-client/tests/app_gold.rs`: a gold pile through 0x9C and the gold messages through the bridge.

## What is left

- **Monster / object gold drops** are q-monster-drops' (the treasure run on a kill). The pile it creates will draw and pick up through links 1, 4–6.
- **Stash gold** (kinds 3 / 4, stat 15) and the trade offer (kind 2) are not wired; the dialog opens only for kind 1.
- **Quest piles** and the gold limit message (S→C 0x2C event 19) are not checked.
- **The pile's ground message**: the live server sends the new pile to the client through the update pass; whether the first tick sends it was not run against the live data (the synthetic game has no item tables).
- A pile of 0 gold or past the limit is the sim's rule (§10.1), unchanged.

## The user's local check (Windows, PowerShell, 1.14d files)

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
$env:RUST_LOG = "warn,d2_client=info"
git fetch origin claude/q-gold
git checkout claude/q-gold
cargo run -p d2-client --release -- play --new sorceress Test
```

1. Press **I**. The gold line shows the character's gold (a new character has 0). Pressing the coin button next to it only dims it (frame 1).
2. With gold (after another branch's drops, or a saved character): press and release the coin button. A dark box asks "How much gold / do you want to drop?". Type digits (the box takes at most your gold), press **Enter**: your gold falls and a pile appears on the ground at your feet. **Escape** cancels, and 0 sends nothing.
3. Click the pile on the ground. The character walks to it and the pile disappears; the inventory gold rises by its amount. A small pile (< 100) and a large one (≥ 500) must show different art.
4. If a step shows nothing, send the log lines with `ground item`, `item`, `ui:` or `world click`, and say which step.

## Gate

`cargo fmt --all`, `cargo clippy -p d2-client -p d2-server --all-targets -- -D warnings` and `python3 tools/coverage.py --check` (0 errors) are clean. `cargo test -p d2-client -p d2-server` passes except `d2-server --test world_data_tables` (`every_game_view_builds_from_the_synthetic_install`, `drop_tables_hold_…`, `a_saved_hireling_…`), which fails on the base too (the tests share one synthetic install directory and race on it; see `stitch-items.md`).
