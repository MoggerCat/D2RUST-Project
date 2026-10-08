# Handoff: identify and item info (`claude/q-identify`)

> Stitching session, 2026-10-08. Read only `specs/`, `docs/`, `crates/`,
> `tools/`. No audio. All tests use synthetic data.

## The path, and which links were connected

| Link | Before | Now | Where |
|---|---|---|---|
| Hover over an item (inventory panel) | nothing | the item under the mouse in the page-0 grid or an equipment box is found; its last 0x9C / 0x9D stream is decoded and a tool tip box is drawn over everything | `ui/panels/inv_items_tip.rs` (new, `item_at`, `hover_lines`), `bridge/items.rs::stream`, `ui/original.rs` (`BorderUi::draw`, one call) |
| Tip text | — | name line (base name, magic prefix / suffix, rare / unique / set name) in the quality colour, a red "Unidentified" line, Defense, Durability, Quantity, requirements, properties | `ui/item_tip.rs` (new) |
| Tip data | — | `ItemTips` built from the user's tables (`d2_sim::items::ItemTables` for the stream reader, `namestr`, `reqstr/reqdex/levelreq`, `itemstatcost` descriptions, affix / unique / set name keys) and the string tables | `app/items.rs::item_tips`, `app/play.rs` (live data only) |
| Identify: pick the scroll | — | a right press on a scroll (`isc`) or tome (`ibk`) in the grid sets the identify cursor (cursor state 6, the used item); a second right press cancels | `ui/panels/inv_items_tip.rs::right_press`, `ui/original.rs` (`InventoryUi::event`) |
| Identify: click the item | `grid_click` could produce `GridMsg::TargetUsed` but the panel dropped it | a left press on a grid item sends C→S 0x27 (target, used) and ends the state | `ui/panels/inv_items.rs::grid_press` |
| Identify: server effect | 0x27 reached `use_item`, which answered "not used" (the item-use spec `0x005BF240` is unwritten) | an unidentified target gets item flag 0x10 and 0x1 and is queued for the owner; the next update pass sends 0x9D action 0x15 (`UpdateStats`); the scroll is removed (0x9D action 5, flag 0x20) | `d2-sim/src/wiring/inventory/identify.rs` (new), `pending.rs` (`use_item`, `consume_item`) |
| Client after the update | the 0x9D 0x15 record replaces the item's stream | the tip now shows the properties of the identified item | existing `bridge/msg/stats_items.rs` |

Tests (synthetic):
- `d2-sim` `wiring::inventory::tests::identify` (3): the scroll identifies a stored item, sends 0x9D 0x15 and is consumed; an identified target keeps the scroll; a non-identify item or a non-item target does nothing.
- `ui::item_tip::tests` (4): identified and unidentified magic item lines, a compact stream decoded to its name, the box has one centered line each and stays on the screen.
- `ui::panels::inv_items_tests` (2 new): right press + grid click sends 0x27, hovered item lookup.

## PROVISIONAL points

- **REC-113** (identify effect): see `docs/HANDOFF.md` §7.
- **REC-114** (item tip text and the right-press identify cursor): see `docs/HANDOFF.md` §7.

## What is left

- **Deckard Cain (C→S 0x34):** `AppRest::identify` (`app/rest.rs`) still only logs; the NPC rest has no inventory access (`inventory_entries` is empty). The identify scroll path above is complete; Cain's needs the rest to share the inventory model.
- **Tome of Town Portal:** a right press on `tbk` / `tsc` is not handled (the use effect `0x005BF240` and the town-portal skill are not wired in the preview). The tip shows the tome's name and quantity.
- **Gamble identify (0x37)** is wired in `d2-sim` already; no client UI.
- **Tip details not done:** `descfunc` shapes other than 1–4 (skills, charges, per-level, grouped stats), set bonus lines, the damage / speed lines of weapons, ethereal / sockets contents, gold amount line, the belt and shop grids' tips, the original's anchor (`inventory.md` §5 r1) and box pixels (`text.md` §8).
- Not run here: any check against the original game; the tip is unverified (rule 10).

## The user's local check

Run on Windows with `D2_GAME_DIR` set:

1. `cargo run -p d2-client --release -- play --new amazon Test`
   - The log shows no `item tips (d2rs-own, unverified)` warning. If it does, send that line.
2. Press **I** and move the mouse over a start item (the buckler, a javelin stack, a potion). A dark box appears above the mouse with the item name (white for normal items), `Required ...` lines where the item has them, and `Quantity` for a stack.
3. The start items are identified, so to see "Unidentified" you need an unidentified item; this preview has no drops of those yet, so that line is covered by the unit tests only.
4. Identify (needs a scroll of identify, code `isc`, in the grid; the Amazon does not start with one, so this step needs an item from a drop or a changed start-item table): right-click the scroll, left-click an unidentified item. The scroll disappears and the item's tip shows its properties. A second right-click before the left-click cancels.
5. If any step fails, send the log lines with `item`, `ui:` or `world click` and say which step showed nothing.
