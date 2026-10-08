# q-gamble: gamble windows and repair in the play preview

> Stitching session, 2026-10-08. Read only `specs/`, `docs/`, `crates/`,
> `tools/`. Nothing here is verified against 1.14d (rule 10). No audio.
> PROVISIONAL points: REC-162 in `docs/HANDOFF.md` §7.

The server side was already written (`vendors.md` §5 gamble list, §8
repair, §9.4 gamble price) and C→S 0x35 repair all had a sender. What
kept a gamble window and a repair from working in `play` were glue
links:

## Links connected

| Link | Before | Now | Where |
|---|---|---|---|
| Store / gamble placement in the play rest | `AppRest::place_in_store` and `place_in_gamble` returned `false` ("no room"): a trade open in `play` placed no store item and no gamble list (the e2e rest returned true) | return `true` (preview has no NPC grid model, REC-162) | `d2-client` `app/rest.rs` |
| Items in the shop grid | positions came from the server's grid (none in the preview) | the panel packs each page row by row in arrival order | `ui/shop_ui.rs` `page_items` |
| Gamble flag | `ClickEnv::gamble_shop` was fixed `false`: a click sent C→S 0x32 without the gamble bit and the server refused it | the Gamble menu row records the choice (`NpcMenuState::gamble`); the shop copies it when it opens (`ShopState::gamble`) and the click sends transaction bit 2 | `ui/npc_menu_ui.rs`, `ui/shop_ui.rs` |
| Gamble price | the host published the buy price (transaction 0) for every shown item | a gamble open publishes transaction 2 (`InteractionState::shown_gamble`) | `d2-sim` `wiring/interaction/{mod,npc_vendors}.rs`, `d2-server` `world/wired.rs` |
| Repair-all cost | not published | the host publishes the total (`repair_all_quote`, the rule-3 sum, now shared with the repair handler) under item GUID 0; the panel draws it above the button bar and gives it to the click | `d2-sim` `world/vendors/trade.rs`, `wired.rs`, `ui/shop_ui.rs` (`REPAIR_ALL_KEY`) |

Tests (synthetic), `crates/d2-client/tests/e2e_vendor.rs` (the harness
now takes the NPC class: `Fx::with_class`; Gheed with a two-item gamble
index and Charsi are in `e2e_support`):
- `gamble_window_lists_prices_and_buys`: the NPC menu's Gamble row →
  C→S 0x38 action 2 → the player's gamble list as S→C 0x9C action 11 →
  the shop opens as a gamble window → a click sends C→S 0x32 with bit 2
  → the gold paid equals the price the panel shows. Fails with the flag
  fixed `false` (the buy is refused).
- `charsi_repairs_one_item_and_repair_all_answers`: C→S 0x35 on a
  damaged backpack item restores its durability and charges the repair
  cost; the shop's repair-all button leaves as one C→S 0x35 and the
  server answers S→C 0x2A (nothing equipped, nothing charged). These two
  server paths already worked; the test pins them.

## PROVISIONAL / not done

See REC-162. Left: the single-item repair button (frame 6) needs the
inventory panel's clicks; no confirm dialog; the repair-all total is
only checked against the sim's own rule (an equipped damaged item is
not staged in the harness); Gheed, Charsi, Elzix and Jamella are not
placed in the preview towns (Act 1 has Akara and Kashya only; Act 2 and
Act 4 belong to q-a2-town / q-a4), so in `play` the gamblers are Alkor
(Act 3) and Anya (Act 5), the repairers Hratli (Act 3) and Larzuk
(Act 5). The synthetic `play` game has empty vendor and item tables, so
the flow can be tried only on live game files.

## Local check (Windows, 1.14d files)

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
$env:RUST_LOG = "warn,d2_client=debug"
git fetch origin claude/q-gamble; git checkout claude/q-gamble
cargo run -p d2-client --release -- play --new sorceress Test
```

Get to Act 3 or 5 (or a character already there), click Alkor / Anya →
the menu → Gamble. See: the window opens with up to 14 items, packed
from the top left; hovering shows a price; a click buys it (gold drops by
that price; the item arrives unidentified). Hratli / Larzuk: damage an
equipped item, Trade, click the repair-all button (the total shows above
the buttons); the item repairs and the gold drops. Copy any `ui:` or
rejected-message line into `docs/HANDOFF.md`.
