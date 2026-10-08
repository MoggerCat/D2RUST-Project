# q-a4-endgame: Act IV endgame in the play preview

Branch `claude/q-a4-endgame`. Nothing is verified against 1.14d (rule 10). Open point: REC-163 (`docs/HANDOFF.md` §7).

## Links connected

| Link | Where |
|---|---|
| Act IV quest objects had no route: Hellforge (init 48 / operate 49), Diablo start point (55), seals (operate 52, 54, 55, 56; init 56 `ret`), seal-boss dummy (init 59, new `q2::dummy_init`), Harrogath portal (init 78 / operate 73) | `d2-sim` `wiring/economy/quest_objects.rs`; addresses match `object-functions.tsv` (existing table test, plus `act4_functions_are_stated`) |
| `HostQuests` had none of the endgame seams (all reported `unhandled`) | `wiring/economy/quest_host.rs`: frame count, dead, alignment, level monsters, quest-object spawn, room portal flag, superunique spawn (plain monster, chain 23), place object, client idle, act change |
| Diablo and Hephasto had no link to chains 23 / 24 | `d2-server` `wired/quest_events.rs`: link by class before the kill parse (as Mephisto's) |
| Synthetic game | `single_player.rs` objects rows from `synthetic_act4::OBJECT_ROWS`; killable boss classes; the Chaos Sanctuary room is 40 x 40 tiles |

Tests: `crates/d2-client/tests/app_a4_endgame.rs` (three, all fail before the change):

- `the_seals_the_seal_bosses_and_diablo_run_chain_23`: walk the warp line to the Chaos Sanctuary; start point init (`start_known`); the five seals open; the three dummies spawn their bosses (36-38) 27 frames later; their kills reach chain 23 (`kills` 3), the Sanctum clears, Diablo spawns from the timer at the start point and his kill sets `killed`.
- `the_hellforge_answers_and_hephastos_death_reaches_chain_24`: operate 49 without a soulstone moves chain 24 to state 1; Hephasto's kill reaches `hephasto_killed` (the hammer drop is asked for).
- `the_portal_to_harrogath_refuses_then_asks_for_the_act_change`: sound 19 without 26.13; with it the accepted branch runs and the act change is queued and run.

## PROVISIONAL (REC-163)

All places and the plain-monster superunique spawn are `// d2rs-own, unverified`. Bosses and Diablo are killed by hand through the quest-parse kill step (as the Mephisto test), not fought.

## What is left

- Items: the synthetic game has no `mss ` / `hfh ` / gem rows, so the soulstone-to-forge, the hammer smash and the gem drops run only in the d2-sim unit tests (`q3_tests`).
- Fortress to Harrogath: the act change from act 3 to act 4 is requested and run, but the player stays in the Fortress room in the synthetic game (act 0 to act 4 works, `app_a5_town`). Looks like a placement/room-list issue in `wiring/path/act_change.rs`; not investigated further.
- Harrogath waypoint activation (`0x005B4FF0`), `wielded_weapon_code`, Tyrael's portal spawn (message 20000), the classic end-of-game timer.
- Real superunique spawn (mods, minions) and creation-time chain links.

## The user's local check (game files)

```
cargo nextest run -p d2-client --test app_a4_endgame
D2_GAME_DIR=<install> cargo run -p d2-client --release -- play --new sorceress Test
```
With an Act IV save: in the Chaos Sanctuary click the five seals (expect the seal animation, the three seal bosses, no `unhandled 254/255` lines naming 0x5B5630 or 0x0055_5230), kill them and watch Diablo spawn at the start point; in the River of Flame use the Hellforge with and without Mephisto's soulstone; after Diablo, click Tyrael's portal. Send log lines with `unhandled`.
