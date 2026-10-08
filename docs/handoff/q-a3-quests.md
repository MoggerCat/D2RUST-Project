# q-a3-quests: Act III quests in the play preview

Branch `claude/q-a3-quests`. Nothing is verified against 1.14d (rule 10). Open point: REC-142 (`docs/HANDOFF.md` §7).

## Links connected

| Link | Where |
|---|---|
| Act III quest objects had no route: tome (init 23 / operate 28), decoy (25 / 31), altar (39), sewer stairs and lever (41, 42 / 44, 45), wanderer (43), Hellgate (44), bridge (45), Hratli (49, 50), Natalya (52), stairs R (53), compelling orb (60 / 53), Khalim's heart, eye and brain chests (57–59) | `d2-sim` `wiring/economy/quest_objects.rs` (`init_fn`, `operate_fn`, the dispatch); addresses match `object-functions.tsv` (existing table test) |
| Mephisto's death never reached the Guardian (chain 20) | `d2-server` `wired/quest_events.rs`: a Mephisto-class kill adds the chain 20 link (as Andariel's, REC-132) |
| Town NPCs (Ormus, Asheara, Hratli, Alkor, Meshif, Cain), hire row | already in staging (`q-a3-town`, REC-137, `app/town_npcs.rs`); not duplicated here |

Test: `tests/app_a3_quests.rs` (Mephisto spawned, killed, chain 20 reaches state 6), `quest_objects::tests::act3_functions_are_stated`. The shared quest event path (`q-a1-tower`) and `HostQuests` are reused; no second NPC list.

## PROVISIONAL (REC-142)

Link by class for Mephisto; init functions needing a point do nothing for a null room. `// d2rs-own, unverified`.

## What's left

- No Act III levels in the synthetic world (Durance, Travincal, sewers), so the object routes run only on live presets; the Council members' and Gidbinn's kill links, the Durance warp call and the red portal to Act IV are unwired.
- Lam Esen's tome scroll text and the 0x31 reply are not driven.

## The user's local check (game files)

```
D2_GAME_DIR=<install> cargo run -p d2-client --release -- play --new sorceress Test
```
Take Meshif to Kurast Docks, talk to Alkor/Hratli/Ormus (quest messages), click Lam Esen's Tome in the Sewers, the Khalim chests, and kill Mephisto: expect the quest log to advance and no `unhandled` lines naming Act III functions. Send log lines with `quest`, `unhandled`.
