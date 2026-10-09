# q-fix-ui-play-wiring (`claude/q-fix-ui-play-wiring`)

> Fix session, 2026-10-09, from `claude/q-ui-audit`, merged with
> `claude/specs-staging-7` before each push. Rows: `q-fix-ui-npc-menu`,
> `-shop`, `-npc-talk`, `-cursor`, `-stash-close`, `-waypoint`,
> `-hud-small` (q-ui-audit finding 1 and §4–§6), the shop's store tip
> context (after q-fix-items-tips), and the two seam rows
> `q-fix-seam-beltable` / `q-fix-seam-store-grid` (see below). The
> play path now runs the spec modules; the d2rs-own preview adapters
> (`npc_menu_ui.rs`, `imbue_ui.rs`) are deleted.

## What changed

| Row | Code | Play-path tests |
|---|---|---|
| npc-menu | `ui/npc_box.rs` (ui 8: `build_npc_menu`, anchor, Imbue insert, confirm, waiting note, 0x2A) | `npc_box_tests.rs`; real data `play_smoke::the_spec_npc_ui_on_the_install` |
| shop | `ui/shop_ui.rs` (ui 0x0C, `caller_args`, confirm, repair button) | `e2e_vendor.rs`, `play_smoke` |
| npc-talk | `ui/npc_talk.rs` (topic box, dialog panel, socket dialog for Imbue) | `npc_talk_tests.rs`, `e2e_imbue.rs` |
| cursor | `ui/cursor_ui.rs` (`panels-3.md` §23; OS cursor hidden) | `the_cursor_cel_follows_the_pointer_and_the_buttons` |
| stash-close | stash / cube rect + inventory close rectangle | `the_inventory_close_button_closes_the_stash` / `_cube` |
| waypoint | `ui/waypoint_ui.rs`: quest-gated open tab and tab clicks (`menus.md` §1.4, `panels.md` §13.3), outside-press close, close hover, level names | `original_tests::waypoint_play` |
| hud-small | mini-panel automap (state 0x0A drives the automap) and quest log, sides / offsets, tips with key names, run tip keys, `stambarblue`, Show HP / MP text in `settings.toml` `[hud]`, belt cursor highlight and short hover text, skill tree band / free points / tips, character and stash close tips, empty equipment-slot pictures, measured item frame sizes, belt-key Shift / ui 9 / death gates | `original_tests::hud_small`, `skill_tree_play`, `equip_backgrounds_play`, `inv_items` belt tests, `bridge::belt`, `app::items` |
| store tip | `shop_ui::store_tip_ctx` (mode, own item 0, gamble, `Cost:`) | real data: `play_smoke` reads "Cost: 25" |

## Seam rows

- `q-fix-seam-beltable`: landed on staging through q-fix-audit-rest
  (`ItemArtRow.beltable`, `seam_belt_tables.rs`); this branch's parallel
  version was dropped at the merge.
- `q-fix-seam-store-grid`: the client half (draw at the stream's cells)
  landed on staging (be47a4c9). **Open, server side**: on the install
  every store item arrives at cell (0, 0), so the shop now draws Akara's
  ten items on one cell. The NPC grid placement `0x00560200`
  (`vendors.md` §3.1 r4) is `AppRest::place_in_store`, a stub, and the
  store fill (`store::open`, interaction wiring) does not reach the
  inventory model (`InvParts`) whose x / y the 0x9C stream carries
  (`InvVendors` only wraps the trade calls). Repro (known bug, skipped by
  the real-data gate): `play_smoke::the_shop_finds_each_store_item_at_its_server_cells`.
  Needs a server task: the store fill on the inventory model (an NPC
  `Inventory`, record 5 per page, find-free placement, unlink on take /
  remove).

## Findings for other rows

- The store tip of a Stamina Potion carries a line "Unable to ignore %s"
  (item-tips builder string lookup; q-fix-items-tips' module).
- `play_smoke::the_live_run` fails as before this branch ("monster 3
  died: life Some(256)", q-fixture-migrate F1–F3).

## Deferred (d2rs-own, unverified)

- Sell / repair / identify prices of player items with the shop open
  (the client has no such prices yet; their tip stays the plain hover).
- The trade buttons' inventory modes other than repair (4) and idle (1).
- Character close tip centring; the key names of tips are the play
  bindings' names, not the §5 r13 strings.
