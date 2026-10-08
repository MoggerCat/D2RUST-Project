# q-a1-andariel: Sisters to the Slaughter in the play preview

Stitching session `q-a1-andariel`, branch `claude/q-a1-andariel`. Nothing is verified against 1.14d (rule 10). Open point: REC-132 (`docs/HANDOFF.md` §7).

## The path, with the links that were missing

| # | Link | Before | Now |
|---|---|---|---|
| 1-3 | Kill and level change reach the quest control | no live caller | q-a1-tower's shared path (`d2-server` `wired/quest_events.rs`, `Pending::take_quest_events`), reused here; no second queue |
| 4 | Andariel's link to chain 6 | none | `wired/quest_events.rs` links a class-156 victim to chain 6 before the kill parse (PROVISIONAL) |
| 5 | Client rest answers | monster kind / level | `SnapUnit.level` / `.class` from the sync snapshot feed `unit_level`, `unit_kind` (chains are q-a1-tower's) |
| 6 | Credit, gems, portal timer, Warriv scroll 183, Warriv travel | existing quest code (`quests/act1/q6.rs`) and q-act-travel's act change | now reached from the live kill / level change |
| 7 | Synthetic world | no Catacombs 4, no Warriv | level 37 as a flat level, Warriv (class 155) `npc`/`interact` in the synthetic `monstats` and the client rows |

Test: `crates/d2-client/tests/app_andariel.rs` `killing_andariel_completes_the_act_and_warriv_travels_east`: the player warps to Catacombs 4 (A1Q6 state 3), Andariel is killed (bits 6.13 and 6.1 set), Warriv's 0x31 message 183 grants the reward (6.0), C→S 0x38 action 0 moves the player to Lut Gholein (client act 1, nothing rejected). Fails before the change at the kill assertion (checked by disabling the kill queue: only bit 4 is set).

## PROVISIONAL (REC-132)

Andariel's link to chain 6; the quest events run when the tick returns (not inside the kill / client update); a level change's old level is the last level noted (0 first). All `d2rs-own, unverified`.

## What is left

- Reaching Catacombs 4 on the live data by the stairs (Catacombs 1-4 maze warps rely on DS1 warp units; `q-act1-dungeons`), and Andariel's real spawn.
- Andariel's AI (`ai.md` §9.12) and poison missiles/skills on the live monster tables: the body exists in d2-sim and was not run in `play`.
- The portal timer's visible portal and the quest log row for Sisters to the Slaughter; Warriv's travel row text needs the string table (q-strings).
- The monster-kill gems (`drop_item_at` in `AppRest`) are the existing seam.

## The user's local check (game files)

```
cargo test -p d2-client --test app_andariel
cargo run -p d2-client --release -- play --new sorceress Test
```
In `play`, reach Catacombs Level 4 (or use a save already there), kill Andariel, return to the Rogue Encampment, talk to Warriv and choose Travel: expect the screen to switch to Lut Gholein (Act II). Record the console lines if Andariel does not attack or dies without a reward (the usual suspect: her chain link, REC-132).
