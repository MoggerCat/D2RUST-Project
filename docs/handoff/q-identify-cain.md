# Handoff: Cain's identify and belt tips (`claude/q-identify-cain`)

> Stitching session, 2026-10-08. Synthetic tests only; nothing verified against 1.14d (rule 10).

## Links connected

| Link | Where |
|---|---|
| Server side already wired (C→S 0x34 → `AppRest::identify` → `WiredWorld::npc` stages `npc_entries` and applies `identify_unit`, then the 0x9D 0x15 update) | `d2-server` `wired.rs`, `d2-sim` `wiring/inventory/identify.rs` |
| Cain's menu counts the unidentified items from the client model; n = 0 drops the Identify row, else the caption is "Identify Items: " + `100 × n` (`menus.md` §2.3) | `ui/npc_menu_ui.rs` (`unidentified_count`, `Row::cost`), `ui/msg_ui.rs::npc_dialog` |
| Identify row click → C→S 0x34 [GUID] | existing `panels/npc.rs::option_intent` |
| Belt potion hover → tool tip at (box left + 14, top) | already wired (`HudBelt::hover_tip`, `original.rs` `BorderUi::draw`); now tested |

Tests: `tests/app_play_act3_town.rs::cains_identify_row_shows_the_cost_and_sends_0x34` (two unidentified items injected by S→C 0x9C, menu shows cost 200, the row sends 0x34); `inv_items_tests::belt::hovering_a_belt_item_yields_its_tip_at_the_box`.

Changed expectation (spec-driven, `menus.md` §2.3): `every_kurast_docks_npc_opens_its_menu` now expects Cain's menu as `[Talk, Cancel]` because that character carries no unidentified item (n = 0 skips the row).

## PROVISIONAL: REC-255 (see `docs/HANDOFF.md` §7).

## Discrepancy with the queue text
The queue says Cain "charges nothing in 1.14d per the spec". `world/npc.md` §6 step 4 charges `100·n` unless quest 4 (Search for Cain) bit 0 or 1 is set, and the server follows that (`cain_identifies_three_items_for_300`). I kept the spec's rule. Free after the quest; 100 per item before.

## Not done
- Synthetic game data has no inventory model (`GameParts::synthetic` sets `inventory: None`), so the full identify effect (flags set, 0x9D 0x15) is only reachable with live data; the server pieces are tested separately.
- The quest-4 bits are not in the client model (caption always shows the cost).

## The user's local check
`cargo run -p d2-client --release -- play --new amazon Test`: with an unidentified item in the pack (from a drop), talk to Cain: the menu shows "Identify Items: <100 × n>"; with none, no such row. Click it: the items identify (tip no longer says Unidentified) and gold drops by 100 × n unless the Search for Cain quest is done. Hover a belt potion (move to the belt strip, over a box): its tip appears.
