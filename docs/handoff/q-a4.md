# q-a4: Act IV in the play preview

Branch `claude/q-a4`. Nothing is verified against 1.14d (rule 10). Open point: REC-143 (`docs/HANDOFF.md` §7).

## Links connected

| Link | Where |
|---|---|
| Act index 3 in the synthetic game (`LevelSource.acts` is a `Vec`; live adds the Fortress as the act's town) | `app/single_player.rs` |
| Fortress room: Tyrael, Jamella, Halbu, Cain and a waypoint (levels row `waypoint` 27) | `synthetic_act4.rs`; allocation next to Lut Gholein's |
| NPC rows for the server's `interact_classes` and the client's unit rows | the one list `synthetic_npc_classes` (chained `synthetic_act4::NPCS`); `SYNTHETIC_MONSTATS` raised to 410 (Jamella is 405) |
| Outer Steppes → Plains → City → River → Chaos Sanctuary as flat levels in a warp chain (lvlwarp 31–40, room flags, preset tiles) | `synthetic_act4.rs`, `Types`, `synthetic_drlg_data` |
| Izual host-placed in the Plains, linked to chain 22; his kill reaches A4Q1 through `LocalSeams::kill_step` and the shared quest event path | `host_monster_created`, `synthetic_monstats` (killable) |

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
