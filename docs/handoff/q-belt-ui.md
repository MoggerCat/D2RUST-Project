# Handoff: belt UI and potions (`claude/q-belt-ui`)

> Stitching session, 2026-10-08. Read only `specs/`, `docs/`, `crates/`,
> `tools/`. Nothing here is verified against 1.14d (rule 10). No audio.

## Links connected

| Link | Before | Now | Where |
|---|---|---|---|
| Belt row in the control panel | only the rules existed (`ui/panels/control/belt.rs`), nothing drew them | belt items drawn in their boxes, pop-up rows (`ctrlpnl_popbelt`) while the mouse is over the belt strip; the belt type is read from the worn belt's `armor.txt` `belt` column | `ui/hud_belt.rs` (new), small hooks in `ui/hud.rs`, `ui/original.rs`, `app/items.rs` |
| Belt click | `over_belt: false`, `BeltClick` ignored | a release on a box sends 0x24 (take), 0x23 (put) or 0x25 (switch) through `BeltState::click`; the press is consumed | `ui/hud.rs` `event` |
| Shift-click to belt | `shift: false` in the grid context | `ItemsUi::shift` (set from the Shift keys each frame) and a code-based `fits_belt`; `GridMsg::ToBelt` sends C→S 0x63 | `ui/panels/inv_items.rs`, `world_view/present.rs` |
| Potion use (0x26) | `use_item` seam answered false: nothing happened | `InvDesk::use_potion`: healing potion → `healthpot` list (state 100, stat 74), mana potion → `manapot` (106, stat 26), rejuvenation → instant share; `remove_used_item` sends the 0x9D removal (flag 0x20) and frees the item. The existing regen tick (`stat-lists.md` §10.1), list expiry and the life / mana prediction in 0x95 do the rest, so the orbs follow | `d2-sim/src/wiring/inventory/potion.rs` (new), two small hooks in `pending.rs` |

Tests (synthetic): `wiring::inventory::tests::belt::use_belt_item` (list, stat 74, expiry, removal message),
`d2-server` `use_belt_item`, `inv_items_tests::belt::*` (draw in box, click 0x24 / 0x23, shift-click 0x63).

## PROVISIONAL (REC-102 in `docs/HANDOFF.md` §7)

- Item-use spec `0x005BF240` is unwritten: potion codes, amounts (hp 45/90/150/270/480, mp 30/60/120/225/450), duration (100 frames), rejuv shares (35 % / 70 %) are `d2rs-own, unverified`.
- Belt UI: resolution index 0 below mode 2 else 1; every belt item usable and unblocked; no box highlights, key labels or hover text (no line draw / string table in play); `fits_belt` is a code list (the server still checks).
- The 0x26 `on_merc` target is not routed to potions (player only).

## What is left

- Belt key labels and hover text (string table by id: `TableStrings::by_id` once the panel has a string source).
- The cursor-highlight rectangles (green / yellow / red) need a rectangle draw.
- Not run against game data here (cloud): `belts.bin` row layout is read via `BeltRecord::from_bytes`; a failed load logs a `belt (d2rs-own, unverified)` warning and leaves the belt undrawn.
- `d2-server --test world_data_tables` fails on the base as well (shared install dir race).

## The user's local check

`cargo run -p d2-client --release -- play --new amazon Test` with `D2_GAME_DIR` set:

1. The belt strip right of the globes: the Amazon's start potions show in their boxes; moving the mouse over the strip pops the extra rows.
2. Take damage (or use Shift to read life), press **1**–**4**: the potion leaves the belt and the life / mana orb refills over about 4 s.
3. Click a belt potion: it moves to the cursor; click an empty box: it goes back.
4. Open **I**, hold **Shift** and click a potion in the grid: it moves into the belt.
5. On failure send log lines with `belt`, `item` or `ui:`.
