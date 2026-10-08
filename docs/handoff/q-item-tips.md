# Handoff: item tip gaps (`claude/q-item-tips`)

> Stitching session, 2026-10-08. Synthetic tests only; nothing verified against 1.14d.

## Links connected
| Link | Where |
|---|---|
| `descfunc` 1–28 line shapes (`+n`, `n%`, per-level, class / tab / skill levels, skill-on-event, aura, charges, printf, negative forms) | `ui/item_tip_desc.rs` (new), `ui/item_tip.rs::property` |
| Set item: set lists green; set name, then every partial step (green) and the full-set bonus (orange) from the sim's own set-bonus rule | `ui/item_tip_set.rs` (new), `item_tip.rs::set_lines` |
| Shop hover: tip box with `Price: n`, white lines red when base str / dex / level are short; falls back to the bare price without tips | `ui/shop_ui.rs`, `item_tip.rs::{shop_marks, shop_lines, can_use}` |
| Right press on Town Portal scroll / tome sends C→S 0x20 (server `use_portal_item` already exists) | `ui/panels/inv_items_tip.rs::is_portal` |

Tests: `item_tip_desc` (24 shapes + printf), `item_tip` (two-piece set lines, shop marks), `inv_items_tests` (portal right press).

## PROVISIONAL: REC-242 (see `docs/HANDOFF.md` §7).

## Not done
- **`equip_rules` stays off.** Setting `InvState::equip_rules = true` in `d2-server` `InvParts::new` makes `adapters::handlers::items::moves::tests::swap_one_handed_with_two_handed` hang (>450 s) and the run was killed; the preview rest (`PreviewMoveRest`, `handlers/world.rs`) does not answer the equipment seams (skill list, mouse slots, stat links) that the rules need. Needs those seams first.
- Belt item tips (belongs to q-belt-stash), identify via Cain, Tome of Identify beyond the existing right press.

## The user's local check
`cargo run -p d2-client --release -- play --new amazon Test`: press **I**, hover the start items (names, requirements, quantity as before). Right-click a `tsc` / `tbk` in the grid (needs one in the pack): a portal use is sent. Open a vendor (once an NPC trade exists) and hover a store item: tip box with a Price line; red lines if the character is too weak.
