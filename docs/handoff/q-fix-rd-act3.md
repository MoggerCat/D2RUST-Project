# q-fix-rd-act3

Branch `claude/q-fix-rd-act3` (REC-1555..1559; PROVISIONAL choices marked). Task: the three failing
real-data tests of `crates/d2-client/tests/app_play_act3.rs`.

## Result (`D2_GAME_DIR=$HOME/game`, `--run-ignored only`)

| Test | Before | Now |
|---|---|---|
| `kurast_docks_arrival_and_every_town_npc_talks` | `unit 1/[245] not reached` | pass |
| `the_golden_bird_from_the_jungle_boss_to_the_potion_of_life` | `j34` not picked up | pass |
| `the_blade_of_the_old_religion_from_hratli_to_ormus_and_asheara` | Hratli gave 466, not 571 | gets to the last `assert_clean` ("the Blade"); fails on host calls with no provider (open, below) |

The other four tests of the binary still pass.

## Causes and fixes

1. **Player without an inventory** (REC-1555, found by the Golden Bird): `join_player_items` returned early
   for a save with no items, so the server never gave the player an inventory (`0x0063ABD0` runs at
   allocation in the original). `can_pick` answered "no inventory", every ground pick-up was refused.
   `join_player_items` now always calls `load_items`, and `load_list` adds the inventory for an empty
   list (`item_save.rs`, `save_full.rs`).
2. **NPC chat did not see the held items** (REC-1555): the text list of the NPC chat (`Desk::quest_text_list`)
   ran on `EconomyQuests` whose `QuestRest::inventory` is empty in the play host, so "holds `j34`/`g34`" was
   always false and Cain's 527 never listed. `NpcInventory::items_of` added; the desk lends cursor + item
   list to `EconomyQuests::held`.
3. **Pick-up hook ITEMPICKEDUP (`0x00543D80`) was never raised** (REC-1556, PROVISIONAL order): the move
   call records the pick-ups (`MovePending::quest_item_picked` on `PreviewMoveRest`), `WiredWorld::moves`
   turns them into `item_picks`, `run_quest_events` dispatches quest event 4 to the item's chain after the
   tick (the original calls it inside the pick-up). The item-creation link (`quest` ≠ 0 → chain
   `quest` − 1, `0x00555D20`) is not wired either, so the item is linked at its first pick-up.
4. **Gidbinn boss seams without provider** (REC-1557): `has_act3`, `room_covering`, `player_in_rooms`,
   `spawn_monster_in_room`, `special_monster` on `HostQuests`; `MonsterWorld::spawn_random_boss`
   (population `random_boss`, champion allowed, warp check) for `0x005A43E0`.
5. **Test staging** (not behaviour): the first talk with an Act III NPC replays the introduction (chain 39,
   the client sends the first text of the list only: `msg-ui.md` §16 r4), the second talk the quest; the
   tests now talk twice (Hratli: gossip 466 then Blade 571). The Blade's boss timer (period 7, run every
   20th frame) needs 140 frames, the test waits 160. `0x20` UseGridItem carries the player's point (§7.11
   range check), not (0, 0). The Gidbinn decoy is reached with `goto preset 78 2:252` (the room tour does
   not place it, see open point 3). The Docks NPCs are reached with `goto preset` (walk stops, open
   point 2). The Blade's player gets staged life/mana (the boss pack kills a level 25 sorceress).

## Open

1. **Blade, unhandled host calls (`AppRest::log`)**: `sound 0 65` (`attach_sound`, Gidbinn pick-up),
   `sound 403 10`, `state stat 488 105 172 2`, `join team 488 0`, `hireling ai 488` (Asheara's quest
   mercenary `0x00579180`, `npc.md` §7.5), and `stat sent 0 6 …` from the staged life poke. Owners: sound
   attach / hireling (`HirelingRest`) providers in `d2-client/src/app/rest.rs`; none is specified as a message
   in `specs/`. After they are provided, the rest of the Blade (Ormus 587/593, Asheara 589, 19.0) still has to
   run; the test body past line ~890 was never reached.
2. **Walk on the Docks** (route to coordinator / walk owner): from (5139, 5087) the server accepts no walk
   to a point north of y 5082 although `drlg.collision_at` shows a free route (targets at y ≥ 5082 work);
   Cain stands at (5141, 5060). `app_support::approach` loops forever there.
3. **Flayer Jungle objects 251/252** are absent after touring every room of level 78; they exist only after
   `goto preset 78 2:252` (DRLG owner: are the altar/decoy presets placed with the room list?).
4. The ITEMPICKEDUP timing and the item-creation link are PROVISIONAL (above).

## Repro

```
export D2_GAME_DIR=$HOME/game
cargo run -q -p data-tool -- excel-dir $D2_GAME_DIR/extracted/patch_d2/data/global/excel $D2_GAME_DIR
cargo nextest run -p d2-client --test app_play_act3 --run-ignored only --no-fail-fast
```
