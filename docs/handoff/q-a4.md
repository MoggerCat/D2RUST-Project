# q-a4: Act IV in the play preview

Branch `claude/q-a4`. Nothing is verified against 1.14d (rule 10). Open point: REC-143 (`docs/HANDOFF.md` §7).

## Links connected

| Link | Where |
|---|---|
| Act index 3 in the synthetic game (`LevelSource.acts` is a `Vec`; live adds the Fortress as the act's town) | `app/single_player.rs` |
| Fortress room: Tyrael, Jamella, Halbu, Cain and a waypoint (levels row `waypoint` 27) | `synthetic_act4.rs`; allocation next to Lut Gholein's |
| NPC rows for the server's `interact_classes` and the client's unit rows | the one list `synthetic_npc_classes` (chained `synthetic_act4::NPCS`); `SYNTHETIC_MONSTATS` raised to 410 (Jamella is 405) |
| Outer Steppes → Plains → City → River → Chaos Sanctuary as flat levels in a warp chain (lvlwarp ids after `synthetic_act2::last_warp()`, room flags, preset tiles) | `synthetic_act4.rs`, `Types`, `synthetic_drlg_data` |
| Izual host-placed in the Plains, linked to chain 22; his kill reaches A4Q1 through `LocalSeams::kill_step` and the shared quest event path | `host_monster_created`, `synthetic_monstats` (killable) |

Warp ids start after `synthetic_act2::last_warp()` (the coordinator's range rule). Act-generic host: `test-fixtures/src/act4.rs` (the Act IV set: 103 town, 104–106 outdoor via the real placer, 108 the A4C level) and `tests/act4_play.rs` (a `Session::new_in_act(.., 3)` walks Fortress → Outer Steppes → Plains → City of the Damned; Outer Steppes populated and the client told). Made-up data, no `Covers:` claim. The Chaos Sanctuary's generator (§10 stamps) is not walked by that test.

Test: `crates/d2-client/tests/app_a4.rs` (NPCs and waypoint in the model, each NPC answers 0x28; warp line to the Plains, Izual's kill sets chain 22 to state 4; nothing rejected).

## PROVISIONAL (REC-143)

All level places, tile places, NPC rows and Izual's place are `// d2rs-own, unverified`.

## Left

- Hephasto, the Hellforge, the seals, the seal bosses and Diablo (A4Q2/A4Q3 objects and kills).
- Tyrael's talk-driven quest start (client 0x31 reply) is not driven by the test.
- The act change is the hooks queue (as in q-a2-town); Act III is not a real predecessor.

## The user's local check (game files)

```
D2_GAME_DIR=<install> cargo run -p d2-client --release -- play --new sorceress Test
```
With an Act IV save: click Tyrael, Jamella, Halbu and Cain in the Fortress (expect the menu box, no `rejected` lines), walk out to the Outer Steppes and Plains of Despair, kill Izual and check the quest log. Synthetic check: `cargo nextest run -p d2-client --test app_a4`.

## Existing tests changed (with reasons)

- `app_single_player` (`game_creation_derives_the_four_controls...`): four more NPC allocations and one waypoint allocation, one RNG step each (`rng.md` §5.3), as q-a2-town did for Act II. The spec value, not a weakened assertion.
- `app_play_monster_ai`: kept on the default seed; added `a_missed_first_attack_is_followed_by_more_attacks` (seed 9). The failure was a real gap, not a lucky seed: the Zombie's first attack missed (a legitimate 5% roll, hit chance 95) and it never attacked again, because the attack do sets unit flag 0x40, the per-frame event runs the do only while it is clear (`use.md` §5.2 rule 3), and the monster Attack start never cleared it. Fixed in `monster_attack_start` (PROVISIONAL, REC-143; the player starts and the SQ start clear it). Open: a trace of a monster's second attack to confirm where the original clears it.

## Merge note

Staging's Harrogath (q-a5-town) kept `rooms[4]` and act 4; the Fortress room is `rooms[5]` (act 3). Both towns had claimed waypoint index 27: the Fortress keeps 27 (the real index), Harrogath's synthetic one is now 35 (its real index).
