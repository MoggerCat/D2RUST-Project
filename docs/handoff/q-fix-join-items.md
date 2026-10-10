# q-fix-join-items hand-back (2026-10-09)

Owner of the join order (`specs/flows/game-join.md` §2, `sim/intents-events.md` §8.2).

## Cause

A saved character's items were made by the save load (`WiredWorld::load_items`), which put
them on the player's update list (`inventory-moves.md` §6.1 rule 1) but did not run the list.
The join therefore had no item messages at §8.2 rule 3.5. The first tick's player unit update
(`0x00580860`) sent them instead: 0x9D / 0x9C, then 0x47 and 0x48, after the join sequence
(0x5B, 0x65, 0x8D, 0x5A). The flag-ex bit 21 refresh (0x48 before 0x04) was missing too.
1.14d sends them at rule 3.5, after the first stat messages and before the 0x23 pair.

## Done

- `d2-server` `world/item_save.rs` `load_list`: for a player's own list (not a corpse or a
  hireling), the update-list pass `0x00597890` (rule 3.5) and its reset `0x00597B00`
  (rule 3.10) run at the end of the list, as `WiredWorld::start_items` already did for a
  new character (REC-278). Their messages are the join's item messages (`session::enter_game`
  sends them at rule 3.5 and sets flag-ex bit 21). Moving the two calls from the join to the
  load is d2rs-own; the recordings below match it byte for byte.
- Test: `d2-client` `app_single_player::a_saved_characters_items_are_the_joins_item_messages`
  (real data, ignored): the potion check's save, 1.14d's 0x9D and 2 × 0x9C bytes, between
  0x5F and the 0x23 pair.
- 1.14d caches filled (`--fill-cache`): `traces/orig-cache/cube-005-3-small-rejuvs-one-large`
  (packets), `traces/orig-cache/save-items-ama` (packets, state).

## Results (packets channel, `--orig-cache`)

| check | before | after |
|---|---|---|
| cube-005-3-small-rejuvs-one-large | DIVERGED f2 s2c #17 (0x9C vs 0x23) | **MATCH** (all 24 frames) |
| combat-potion-midfight | DIVERGED f2 s2c #19 (0x9D vs 0x23) | frames 1-2 equal; first divergence f3 (poke attribution, below) |
| save-items-ama | (no cache) | frames 1-2 equal; first divergence f4 (poked item 0x9C byte 8) |

## Open (not the join; routed)

1. **Poke attribution (q-fix-a2-join-mapreveal; q-tool-state-diff area).** 1.14d's recorder
   runs an `at F poke` at the tick-return stop of tick F-1 (poke record `f`=4, `frame`=3,
   before `tick_end` / `flush` 3), so the poke's own messages (warp / goto: 0x07
   `07 00 04 50 03 02`) are flushed in frame F-1. d2rs `state-dump` applies the poke before
   frame F's drain, so the same 0x07 goes out in frame F; the rest of frame F is equal. This is
   the frame-3 MapReveal of every a2-* check (all have `at 4 poke goto ...`) and of
   combat-potion-midfight. Proposed fix: `state-dump` / `play` apply frame-F pokes after tick
   F-1 and before frame F-1's flush. The q-tool-state-diff session (session_019spHN3NFfPQsq6m1Cjqotm)
   was not reachable; routed to the coordinator.
2. **save-items-ama does not share the join cause** (sent to q-fix-d7d8-items-net): the poked
   ground potion's 0x9C byte 8 is 0x10 in 1.14d, 0x00 in d2rs (frame 4). d2rs's headless client
   sends no C→S for `clickunit 4 *` (1.14d: 0x01 Walk f11, 0x16 PickItem f42 and f72), so
   nothing is picked up and the save lacks the potion and the gold.

## Repro

```
export D2_GAME_DIR=$HOME/game
python3 tools/scenario-diff/scenario_diff.py traces/checks/cube-005-3-small-rejuvs-one-large.check --orig-cache --channels packets
python3 tools/scenario-diff/scenario_diff.py traces/checks/combat-potion-midfight.check --orig-cache --channels packets
python3 tools/scenario-diff/scenario_diff.py traces/checks/save-items-ama.check --orig-cache --channels save,packets
cargo nextest run -p d2-client --test app_single_player --run-ignored all a_saved_characters_items
```
