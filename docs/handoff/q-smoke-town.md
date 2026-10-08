# q-smoke-town: town smoke tests over the real play path

Branch `claude/q-smoke-town`. Readiness check before local testing.
Nothing here is verified against 1.14d (rule 10). PROVISIONAL: REC-278
(`docs/HANDOFF.md` §7).

## What runs

`crates/d2-client/tests/smoke_town.rs`: the play app headless (bridge on
the app's in-process server thread, wired sim, original UI; synthetic
fixtures, no window), with a link tap that records every C→S message,
every S→C message and the server's outcome of each drained game
message. `Rig::check` runs after every step and asserts: no rejected,
dropped or unowned S→C message and no discarded bytes in the bridge's
receive log; every C→S game message the step sent dispatched with
`Done` (no `Refused` / `Invalid` / `Malformed`); no S→C 0x2A with a
refusal code (7, 9–15, `npc.md` §9).

| Test | What it does | State |
|---|---|---|
| `act1_town_every_npc_talks_and_cancels` | Akara, Kashya, Gheed, Charsi: walk up by ground clicks, click the NPC (hover pick → walk → C→S 0x13), menu from S→C 0x28, cancel row → C→S 0x30, menu closes | passes |
| `every_act_town_npc_talks_and_cancels` | after the act changes: every NPC of Lut Gholein, Kurast Docks, the Pandemonium Fortress and Harrogath opens its `npc-menus.tsv` rows and cancels | passes |
| `act1_trade_and_gamble_rows_open_the_shop` | Akara / Charsi Trade and Gheed Gamble send C→S 0x38 action 1 / 2; the store items arrive, the shop opens | passes |
| `act1_traders_buy_sell_repair_and_gamble` | buy (cell click), sell (cursor onto the shop), repair one and repair all (equipped items, `vendors.md`), gamble, with the gold moves | passes |
| `asheara_hires_and_resurrects_a_mercenary_through_her_menu` | hire from Asheara's list, the mercenary killed, Resurrect row (cost) → C→S 0x62, the mercenary back | passes |
| `kurast_docks_heals_gambles_and_identifies` | Ormus heals (life staged low), Alkor gambles, Cain identifies a gambled item (100 gold) | passes |
| `act1_stash_keeps_an_item_and_gold` | stash clicked (ui 0x19), a buckler backpack → stash → backpack, deposit 1000 / withdraw 400 through the two gold buttons and the gold box | passes |
| `act1_cube_holds_an_item_and_transmutes` | the start cube opened by a right click (0x20 → ui 0x1A), a buckler into the cube grid and out, transmute (0x4F 0x18), close (0x4F 0x17) | passes |
| `a_click_on_an_npc_while_standing_on_the_stash_talks_to_the_npc` | break 5 | `#[ignore]` |

## Breaks found and fixed (one commit each)

1. Kurast Docks had no NPCs: `town_npcs::act3_docks`.
2. The synthetic new character had no creation stats (walked at 25 %,
   no life): synthetic `charstats` / `experience` (`synthetic_vitals`).
   The skill rigs clear the class skill list they install over.
3. No vendor store in the synthetic game: `synthetic_vendors`, the cap
   and buckler rows.
4. The vendor's gold cap read level 0, so every sale wiped the gold:
   `VendorDesk` gold cap = level × 10000, stash cap 2,500,000.
5. A sale from the cursor was never taken: `InvDesk::take_cursor`
   (0x42 naming the player).
6. The shop reopened at the nearest trader with a stale position, a late
   store copy reopened a closed shop, and an open inventory refused the
   next shop: `NpcMenuState::shop_for` with the store floor, the close
   closes the inventory.
7. No Resurrect row (`panels-2.md` §14.2): `NpcMenus::apply_resurrect`,
   `merc_state` from S→C 0x9B (initially 0xFFFF).
8. The join sent no stat messages: the synthetic `itemstatcost` marks
   0–15 `Saved`.
9. An act change lost the vitals / stats caches: they are dropped on
   the re-add, so the new act gets them again.
10. A new character's start items (staging's REC-244 cube) left the
    join's item messages empty and went out after 0x04 with 0x47 / 0x48:
    `update_list_pass` + reset at the start items (`§8.2` rules 3.5,
    3.10).
11. Every `add_game` app had no visibility predicate (0x96 rejected,
    §13 r6): `add_game` installs staging's `add_visibility`.
12. The stash gold never reached the client: stat 15 is watched by the
    vitals sync's stat changes.

## Still broken

- **Walk verify rubber-banding** (not mine to fix; reported to the
  coordinator). With the vitals on, the server's S→C 0x96 reaches the
  position check, which compares it with the model cell. That cell stays
  at the last placement while the prediction walks (decision D2), so the
  check corrects with C→S 0x5F of the stale cell and the server walks
  the player back. Red in the gate: `app_level_border`
  `walking_east…`, 7 `smoke_travel` walk-outs, `smoke_quests`
  `act1_tower…`. q-play-smoke's REC-277 (c) (the model follows the
  prediction) is the fix in kind. As it stands it breaks staging's
  `smoke_travel` model-position assertion and `app_play_visibility`'s
  predicate call. `app_level_border` also needs the prediction to cross
  the Blood Moor border (it stops at x = 120 while the server walks to
  135).
- **Break 5**: a click on an NPC while the player stands on the stash's
  cell operates the stash (REC-51 straight-line prediction; ignored
  test).

## Rig notes

- The synthetic join sends no skill list: the rig injects S→C 0x94 +
  0x23 as every app rig does.
- The synthetic act towns have no walkable path across their room: when
  ground clicks make no way the rig stands the player beside the NPC on
  the server and sends the 0x13 through the bridge.
- A new character carries the start cube at backpack (0, 0).
- The gold box opens pre-filled with the maximum: the rig clears it
  before typing.

## The user's local check

```
cargo run -p d2-client --release -- play --new sorceress Test
```

In every town, click each NPC from across the screen: the player walks
there and the menu opens. Trade with Akara / Charsi (buy, sell, repair),
gamble with Gheed, hire from Kashya at level 8+, heal at Akara, identify
at Cain. Open the stash, move an item in and out, deposit and withdraw
gold. Open the cube from the inventory (right click), move an item in,
transmute, take it out. Watch for the player snapping back while
walking (the rubber-banding above).
