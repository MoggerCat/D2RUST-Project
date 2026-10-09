# q-fix-items-tips (`claude/q-fix-items-tips`)

> Fix session, 2026-10-08, from `claude/q-items-audit`, merged with
> `claude/specs-staging-7`. Rows: `q-fix-items-store-upgrade-unfound-code`,
> `-tip-affix-ids`, `-tip-descfuncs`, `-tip-groups`, `-tip-builder`,
> `-tip-set`, `-gamble-alt-code` (q-items-audit #2, #3, #4, #5, #8, #9,
> #28). Nothing here is verified against 1.14d (rule 10): the checks are
> the spec vectors; the `text-0002` capture cases of `ui/item-tips.md`
> settle the tip.

## What changed

| Row | Code | Tests |
|---|---|---|
| store-upgrade-unfound-code | `d2-sim world/vendors/store.rs` `upgrade`: ubercode / ultracode chosen when ≠ 0 / spaces; an unfound chosen code is class 0 and never matches (null item) | `an_unfound_upgrade_code_gives_no_item` |
| tip-affix-ids | `ItemTips::magic_name` / `rare_name`: row = id − 1, rare array suffixes first | `affix_ids_are_rows_plus_one` |
| tip-descfuncs | `ui/item_tip_desc.rs` rewritten: §7.1 r2 op value, all 28 shapes by string id | `item_tip_desc::tests::*` (replaces `each_descfunc_gives_its_line`) |
| tip-groups | new `ui/item_tip_props.rs`: §6 L, description order, dgrp, §8 damage groups, undead, indestructible, label, elixir; `stream_values` (partners unshifted) | `item_tip_props::tests::*` |
| tip-builder | new `ui/item_tip_build.rs` (§1–§5, §10, §11), `ItemTips::tip` / `tip_lines` / `text_lines`; `ui/item_tip_world.rs` (P / U, fillers, set facts from the model); fillers summed into L (`inv_items_socket::contents_lines` removed); `app/items.rs` TableDecoder keeps partners unshifted | `item_tip::tests::*` |
| tip-set | `ui/item_tip_set.rs` rewritten from received lists only; no `set_bonuses` on the client | `a_set_item_shows_only_received_bonus_lists` |
| gamble-alt-code | `InvDesk::store_stream` (alt exactly for vendor flag + quality 4–9 + unidentified), `MovePending::store_item_bits` / `store_messages`, `deferred::store_item_message`, the dispatcher ends its walk on a vendor item, `InvDesk::send_store_item` used by the shown-store flush (`d2-server world/wired.rs`); client reads an alt record as base code, ilvl 1, quality 1 | `store_stream_of_an_unidentified_rare_vendor_item_is_alt_code`, `an_alt_code_record_shows_a_white_base_name` |

## For the wiring (q-fix-ui-play-wiring)

The tip API is `ItemTips::tip(stream, &TipCtx) -> TipText` (the
spec's one text, bottom line first, plus the pop-up colour) and
`tip_lines` for the current box. `item_tip_world::hover_ctx` builds a
`TipCtx` from the model for the local player's panels; the inventory and
belt hovers use it. `shop_lines` still serves the shop (buy mode, `Cost:`);
a store host should pass `TipCtx { mode, own_item, gamble, price, .. }`
(§11 r3 decides the label and price, `PriceText`).

## Open seams (no value invented; the part is left out)

- Speed line: `TipUnit::attack_anim` (animdata query, §3.2 r1); none →
  s = 45 per the rule's "not found".
- Holy Shield adds (`0x00647BC0`, `0x00647D00`, `0x00645C10`) and the
  thrown-potion elemental calcs (§3.8 r1): host seams, not filled.
- `0x0062A1E0` (one- and two-handed for P) is not specified:
  `TipUnit::one_or_two_handed`, default false.
- `spelldesc` 2–4 need the calc value (`TipCtx::spell_value`) and the
  `stat1` adjustments `0x0062A5D0` / `0x0062A620` (not specified).
- Possessive `0x005272B0` (§5 r4–r5, ear names) is not specified:
  personalized names show unchanged.
- The client's quality of a compact record is not specified; read as 2
  (PROVISIONAL in `ItemTips::tip_of`).
- `0x0062A370` set-slot masks read as the worn pieces' setitems `slot`
  bits (PROVISIONAL in `item_tip_world.rs`).
- Item totals are base + L + op 13 (`sim/stats.md` §6.3); other ops are
  not run on the client copy.
- The dispatcher's store check sends to the client it is given; the
  "client trading with the owner" test and 0x39 (bit 4) are not done
  (TODO in `deferred::dispatch`).
