# Handoff: stitching items (G16), `claude/stitch-items`

> Stitching session, 2026-10-07. Read only `specs/`, `docs/`, `crates/`,
> `tools/`. Scope: gap **G16** of `docs/handoff/first-playable-scope.md`
> (the item stream, inventory contents, ground items, pick up / drop /
> move, belt). No audio. All tests use synthetic data.

## The path, and which links were connected

| Link | Before | Now | Where |
|---|---|---|---|
| Server join → item messages (`intents-events.md` §8.2 r3.5) | the start items' queued 0x9C / 0x9D (`StartItems::sent`) were dropped | the loader hands them to `SessionState::join_items`, and `enter_game` sends them right after the stat messages, before the hot keys | `d2-sim/src/wiring/action/switch.rs` (`join_items`), `d2-server/src/adapters/session.rs` (`enter_game`), `d2-client/src/app/single_player.rs` (loader) |
| Later item changes (S→C) | `update_pass` runs each tick (`inventory-moves.md` §6.1) | unchanged; already wired | `d2-server/src/adapters/sim.rs:487` |
| Client model: decode / store | 0x9C / 0x9D stored the raw record; only the cursor was read from it | `bridge::items` reads each item's stream head (flags, mode, body / grid / belt location, ground sub-tile, 32-bit code; `bitstream.md` §2–§4.1). It gives `items`, `local_items`, `ground_items`, `cursor_item`, `belt`, and the intent builders 0x16 / 0x17 / 0x18 / 0x19 / 0x1A / 0x23 / 0x26 | `d2-client/src/bridge/items.rs` |
| Belt column-ready bytes (`msg-stats-items.md` §2 r6) | missing | `ClientWorld::belt_ready`, written by 0x0E, 0x0F and 0x15 mode 2 | `bridge/world.rs`, `bridge/msg/stats_items.rs` |
| Inventory panel (I) contents | art and close button only | page-0 grid items, equipped items, and the cursor item at the mouse. A press in the grid or on an equipment box sends 0x18 / 0x19 / 0x1F / 0x1A / 0x1C / 0x1D through the UI root outbox | `ui/panels/inv_items.rs` (new), small hooks in `ui/original.rs`, `ui/panels/mod.rs` (`UiFiles::add`), `world_view/panel_art.rs` (`*items\` names → `data\global\items\`) |
| Ground items | not drawn | drawn with the flippy DC6 at their sub-tile, in draw pass 5. A press on one sends 0x16. A world press while holding a cursor item sends 0x17 | `world_view/ground_items.rs` (new), small hooks in `world_view/present.rs` |
| Belt keys 1–4 | bound, but no action | `bridge::belt::send_keys`: ready-byte gate (§7 r2), then `controls::original::belt_use` (§7 r3), then C→S 0x26 | `bridge/belt.rs` (new), one line in `present.rs` |
| Play wiring | — | `app::items::item_parts` (the `weapons` / `armor` / `misc` art columns and the `inventory` layouts), `add_items` (feeds the ground items), `prepare_ui` (feeds the inventory panel before the UI's file list is taken) | `app/items.rs` (new), one line each in `app/play.rs` and `app/ui.rs` |

Tests (synthetic):
- `bridge::items` (3)
- `bridge::belt` (1)
- `ui::panels::inv_items_tests` (8)
- `world_view::ground_items_tests` (4)
- `tests/app_items.rs`: the synthetic play game joins over a `Bridge`. The test then feeds injected 0x9C messages and checks:
  - the belt potion fills `belt_ready[2]` and `items::belt`;
  - the ground cap shows in `items::ground_items`;
  - the belt key sends 0x26 to the server thread;
  - the pick-up 0x16 is accepted by the link.

Gate on the pushed head:
- `cargo fmt --all` and `cargo clippy -p d2-client -p d2-server -p d2-sim --all-targets -- -D warnings`: clean.
- `cargo test -p d2-client -p d2-server`: everything passes except `d2-server --test world_data_tables`, which fails on the base too. Its tests share one install directory and race on it ("failed to fill whole buffer"), so it is not caused by this branch.
- `python3 tools/coverage.py --check`: 0 errors.

## Preview fills (`d2rs-own, unverified`)

- **Removal flag:** a removal record (0x05 / 0x08 / 0x0F with flag 0x20) hides the item in the view.
- **Belt use:** every belt item counts as useable, not a quest or unique item, and not busy. Shift is never held, and ui 9 counts as closed.
- **Inventory panel:**
  - The frame height is estimated from the cell size when no DC6 size is measured.
  - Without `inventory.bin` there is a grid only.
  - The equipment-box click rule is not in a spec.
  - 0x18 uses the cursor cell, because `0x00486BD0` is not specified.
  - There are no stack, socket, belt or shop facts, and the cursor is a fixed 29 px cell.
- **Ground items:**
  - Items show the flippy's last frame, with no flip animation.
  - There is no unique / set flippy and no gold class.
  - Items are drawn unshaded.
  - Draw keys are pass 5 ordered by (sx + sy, GUID).
  - Pick-up fires on a press inside the drawn art, with no hover-and-walk (`controls.md` §6 r9.2).
- **Join order:** in the new-character join the item messages come from the start-item placement. The order relative to 0x7B follows rule 3.5, but the bytes have never been compared with a recording.

## What is left

- **Not tested end to end with items from the server.** The synthetic `GameData` has no item or inventory tables. A test on the `test_fixtures::act1` install fails for want of an Act II level (40), and also needed a waypoint `InitFn` and `pettype` rows. So the server half (start items → join 0x9C / 0x9D) is checked only by reading and by the local run below.
- **Possible duplicate sends.** It is not checked whether `update_pass` also sends the start items on the first tick. If it does, the client gets each item twice, which is harmless: the record is replaced.
- **Saved characters.** A `--save` character's items are not loaded on the server (save-load unapplied), so nothing is sent for them.
- **Not done:**
  - item name labels (no spec);
  - inventory tints, sockets, ethereal and colour remap (`ImageRequest` has no draw-mode or remap field);
  - the two-handed ghost, hover box, gold line, and the belt drawn in the control panel;
  - art chosen by quality or `VarInvGfx`;
  - shift + belt to the hireling.

## The user's local check

Run on Windows with `D2_GAME_DIR` set:

1. `cargo run -p d2-client --release -- play --new amazon Test`
   - The log shows no `join: new character: start items:` fault line.
   - Press **I**. The inventory panel shows the Amazon's start items:
     - the javelins and the buckler in their hand boxes;
     - the potions in the belt, or in the grid if they don't fit;
     - the scrolls in the grid.
   - Positions are the preview estimate, so they may be a few pixels off.
2. In the same run, click a grid item. It moves onto the mouse cursor. Then:
   - click an empty grid cell, and it goes back into the grid;
   - or close the panel and click the ground, and it drops: it is drawn on the ground in the world.
3. Click the dropped item on the ground. She walks to it and it returns to the inventory, or to the cursor.
4. Press **1**–**4**. A belt potion in that column is used: it disappears from the belt, and life or mana changes if the server applies it.
5. If any of these fails, send the log lines with `item`, `ground item`, `ui:` or `world click`, and say which step showed nothing.
