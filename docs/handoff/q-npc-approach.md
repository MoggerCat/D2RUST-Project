# q-npc-approach: NPC approach in the play preview

Branch `claude/q-npc-approach`. Nothing is verified against 1.14d (rule 10). Open point: REC-151 (`docs/HANDOFF.md` §7).

## Finding

The client already walks toward a clicked NPC and sends C→S 0x13 when its predicted walk ends (`world_view/interact.rs`). The gap was server-side: at distance 7–8 `NpcWorld::approach` only logged (`app/rest.rs`), so no talk started. `npc.md` §2 rule 3 says the server runs the player to the NPC and repeats the 0x13 handling on arrival; the client sends no second 0x13.

Two more causes surfaced:
- The synthetic game had no `charstats` rows, so the server player's velocity was 0 and it never moved.
- The NPC seam `distance` was Euclidean, but the walk's arrival and the 6 / 8 thresholds use the unit distance `0x00641530` (`pathing.md` §9.5).

## Links connected

| Link | Where |
|---|---|
| `Desk::approach` queues (player, NPC) | `d2-sim` `wiring/interaction/npc_world.rs`, `InteractionState::approaches` |
| Run request to the NPC with no skill, mode 3 | `d2-sim` `wiring/path/walk.rs` `PathCtx::approach_unit` |
| Start the run after the tick, queue the interaction | `d2-server` `world/npc_approach.rs` `approaches` (from `after_tick`) |
| Talk on arrival: when the player's mode leaves 2 / 3 / 6, run 0x13 again | `npc_approach.rs` `arrivals` (start of `run_tick`, after the seam refresh) |
| A new walk request drops the queue | `wired.rs` `walk` → `drop_queued` |
| Unit distance in the snapshot | `d2-client` `app/npc_seams.rs` `Snap::distance` |
| Synthetic charstats (walk 6, run 9) | `single_player.rs` `synthetic_charstats` |

Test: `crates/d2-client/tests/app_play_npc_approach.rs`: one click on an NPC at four distances, the menu opens with no second click (fails at offset 32 without the approach).

## PROVISIONAL (REC-151)

- Arrival is read from the player's mode at the start of the next tick, not the step result of `0x00580C20`.
- All units have size 2 in `Snap::distance`; the synthetic charstats rows are made up.
- Beyond 8 sub-tiles the server does nothing (spec: 9..50), so the client's own walk must get close first; an NPC placed so far that the client walk clamp ends beyond 8 still needs a second click (offset 34 in the synthetic town).

## Left

- Check with game files; derive the arrival from the walk step result.

## The user's local check (game files)

```
cargo run -p d2-client --release -- play --new sorceress Test
```
Click Akara or Kashya from across the town: the player walks up and the menu opens with that one click. Click Akara, then click the ground before arriving: no menu opens later.
