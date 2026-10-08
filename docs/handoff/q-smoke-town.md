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
| `act1_town_every_npc_talks_and_cancels` | Akara, Kashya, Gheed, Charsi: walk up by ground clicks, click the NPC (hover pick → walk → C→S 0x13), menu opens from S→C 0x28, cancel row → C→S 0x30, menu closes | passes |
| `every_act_town_npc_talks_and_cancels` | after the act change to Lut Gholein, Kurast Docks, the Pandemonium Fortress and Harrogath: every NPC's menu opens with its `npc-menus.tsv` rows and cancels cleanly | passes (failed before fix 1) |
| `act1_trade_and_gamble_rows_open_the_shop` | Akara / Charsi Trade and Gheed Gamble rows send C→S 0x38 action 1 / 2; the store items and the shop panel should follow | `#[ignore]`d: break 3 |

## Breaks found

1. **Kurast Docks had no NPCs** (fixed). The synthetic Act III town room
   had a waypoint but no NPC unit, so after the act change no Act III
   NPC could be met (the Act III rigs placed them in the Act I room).
   Fix: `town_npcs::act3_docks` and the allocation next to the Kurast
   Docks waypoint in `single_player::build_with_town` (REC-278).
   Changed expectation: `app_single_player`'s game-seed count adds one
   step per new NPC allocation (`rng.md` §5.3), as the earlier town fills
   did.
2. **The synthetic new character has no creation stats** (not fixed,
   staged in the rig). `GameParts::synthetic` has `vitals: None`, so
   `start_stats` (`init_player_stats`, `combat/vitals.md` §1) never runs:
   stat 67 `velocitypercent` is 0 and the server walks at the 25 % floor
   (`pathing.md` §8.1 r2), a quarter of the client's prediction. A click
   on an NPC more than ~8 sub-tiles off sends C→S 0x13 when the client's
   walk ends while the server player is still far behind: no talk, no
   approach. The rig sets stat 67 = 100 as `app_level_border.rs` does.
   Live data has vitals tables, so `play` with game files is not
   affected. Proper fix: synthetic vitals tables (the synthetic
   `charstats` rows plus an `experience` table); it also makes the
   synthetic start items run (`has_inventory`), so every synthetic app
   test needs a re-check: a task of its own.
3. **No vendor store in the synthetic game** (not fixed). `GameParts::
   synthetic` has `vendors: VendorTables::default()` and the synthetic
   item tables hold only the Act IV quest items, so a trade or gamble
   open shows no store item: the shop panel (ui 0x0C) never opens and
   buy, sell, repair one / all and gamble cannot be driven through the
   play app. The server and the UI pieces are tested on their own
   harness (`e2e_vendor.rs`: buy, sell, repair one / all, gamble).
   Fix: synthetic store items (e.g. `cap ` / `buc ` armor rows with
   vendor columns, as `e2e_support::vendor_tables`) in
   `synthetic_items` and the vendor tables in `GameParts::synthetic`;
   the armor rows shift the misc indices the Hellforge tests use, so it
   needs care. `act1_trade_and_gamble_rows_open_the_shop` is the check.
4. **The Resurrect row is never inserted** (not fixed). `ui/panels-2.md`
   (§14.2 builder) inserts Resurrect for 150, 198, 252, 367 and 515 when
   the mercenary is dead (`[0x00725494]` ≠ −1); `NpcMenus::apply_builder`
   says the insert is not implemented, and S→C 0x9B only records the
   calls (`msg_ui_more.rs`). With a dead mercenary the menu offers no
   Resurrect, so the C→S 0x62 the server answers (`app_mercs_acts.rs`)
   has no UI path. Open point before a fix: the initial value of
   `[0x00725494]` (the model's `merc_state` starts at 0, which the rule
   would read as "dead").
5. **A click while standing on the stash's cell operates the stash**
   (seen, not isolated). With the stash at Akara's feet, the walk to
   Akara ended on the stash cell; the next click on Kashya sent a walk
   to the stash and C→S 0x13 on it (ui 0x19 opened). The rig moved the
   stash off the NPC row. Suspects: the hover pick vs the click's
   camera, or the client walk ending inside an object footprint
   (`q-client-collision`).

## Not reached (still to drive)

Hire (Kashya needs character level > 7; Greiz / Asheara / Qual-Kehk are
covered by `app_mercs_acts.rs` through the bridge, not the menu), Cain's
identify (the synthetic game makes no unidentified item), heal (no
creation stats: break 2), stash items and gold, the cube (the synthetic
start items are not made without vitals: no cube in the pack). Each
needs breaks 2 / 3 first.

## Rig notes

- The synthetic join sends no skill list: the rig injects S→C 0x94 +
  0x23 as every app rig does.
- The synthetic act towns have no walkable path across their room
  (`app_a2_town.rs`): when ground clicks make no way the rig stands the
  player beside the NPC on the server and sends the 0x13 through the
  bridge (the test teleport sends no placement, so the client view is
  stale afterwards).

## The user's local check

```
cargo run -p d2-client --release -- play --new sorceress Test
```

Travel to Kurast Docks (Act III): Ormus, Asheara, Hratli, Alkor,
Natalya, Meshif and Cain stand in the town (with game files, at their
preset places); each opens its menu. In every town, click an NPC from
across the screen: the player walks there and the menu opens without a
second click. Trade with Akara / Charsi and gamble with Gheed: the shop
shows items (game files only; break 3 is synthetic data).
