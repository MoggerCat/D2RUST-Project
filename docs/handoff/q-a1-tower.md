# q-a1-tower: the Forgotten Tower quest in the play preview

Branch `claude/q-a1-tower`. Nothing is verified against 1.14d (rule 10). Open point: REC-129 (`docs/HANDOFF.md` §7).

## Links connected

The quest rules (A1Q5, `act1/q5.rs`) and the object routes (tome operate/init) were done; the play host never fed the quest control any monster or level event.

| Link | Where |
|---|---|
| Monster init's chain link (superunique hcIdx 6 → chain 5) was a no-op default | `WorldHost` `InitHost::quest_chain` → `Pending::monster_quest_chain` (`d2-sim` `worldgen/init_units.rs`, `action/pending.rs`) → `LocalSeams` queue |
| Kill parse: `kill_step(QuestKill)` had no provider | `LocalSeams::kill_step` queues `QuestEvent::Kill` |
| Level change had no caller (`QuestControl::changed_level`) | per-tick level compare in `d2-server` `wired/quest_events.rs` |
| The queue reaches the quest control | `WiredWorld::after_tick` → `run_quest_events` (one line in `wired.rs`) |
| `QuestWorld::quest_chain` returned `None` in the app | `AppRest::chains` (`app/rest.rs`) |
| Synthetic world: no Black Marsh, no Tower, no tome | `app/synthetic_tower.rs` (levels 6, 20–25, warp ids 15–28, tome row 60); `synthetic_maze.rs` generalised to the Tower line; `single_player.rs` (drlg data, level types, tome allocation) |

Test: `tests/app_tower_quest.rs` — Blood Moor tile → Black Marsh, click the tome (state 2), the tiles down to Tower Cellar 5 (state 3), a Countess linked to chain 5 and killed (state 5). Expectations changed with the new world, not weakened: `app_single_player` (one more seed step for the tome's allocation, `rng.md` §5.3), `app_frame_loop` (23 joined, 32 handled: the Blood Moor's second warp tile).

## PROVISIONAL (REC-129)

Where in the frame the events run (once per tick after the tick's steps); the Black Marsh as a warp pair off the Blood Moor, the tome's place, the tile places; the Countess is spawned by the test (the synthetic game has no population). All `// d2rs-own, unverified`.

## What's left

- The tome's scroll text (S→C 0x27 type 2) and the client's 0x31 reply that sets status 1 are not driven by the test (no UI in it); the quest rules for them have server tests.
- The level `Quest` link (`attach_quest_chain`, Den of Evil etc.) is still unwired; the Den's kill count needs it.
- The Countess's rune/gold drop uses the generic monster drop (`monster_drop`, REC-108) with the user's treasure classes; not checked on live data.
- The chests of A1Q5 (`chest_init` / `0x173` event) are reached only through live object presets.

## The user's local check (game files)

```
cargo run -p d2-client --release -- play --new sorceress Test
```
No game-file Black Marsh shortcut exists: walk Stony Field → Dark Wood → Black Marsh, click the Moldy Tome (open the scroll, Q shows the quest), enter the Forgotten Tower, descend to Tower Cellar 5, kill the Countess (a superunique with a chest-less drop); expect the quest log to move to "complete", Akara/Kashya's reply (msg 142), and her drop on the ground. Note the console lines if the tome does not react or the kill does not complete the quest.
