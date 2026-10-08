# q-server-loose: server loose ends (`claude/q-server-loose`)

> Stitching session. Synthetic fixtures only; nothing verified against
> 1.14d (rule 10). Preview fills are `// d2rs-own, unverified`.
> Follows `stitch-server-core.md` §3 items 1–3 and `play-fix2.md` (item owner).

## Links connected

| Link | Where | Test |
|---|---|---|
| Reaction (`damage.md` §7.1): a monster the hit does not kill enters get-hit (mode 3, toward the attacker) when the get-hit test lets it through, else is "soft" (queued, unit flag 0x8000 → S→C 0x0C). Players: soft only. | `d2-sim/src/wiring/action/reaction.rs` | `tests/death.rs` `a_hit_puts_the_monster_into_get_hit_or_marks_it_soft` |
| `run_to`: a skill use out of reach makes the server player run to the target (`pathing.md` §1.2 unit form, skill remembered). Falls back to the host seam without a path provider. | `wiring/path/walk.rs` `PathCtx::run_to_unit`, `wiring/interaction/skill_use.rs` | `d2-client/tests/app_server_core.rs` `a_left_skill_on_a_far_monster_runs_the_server_player_to_it` |
| 0x15 resync: an out-of-range point target queues the player with flag-ex 0x10000 (as C→S 0x4B on itself); the next update sends 0x15, flag 1. | `d2-server/src/adapters/handlers/player.rs` `resync`, `sim.rs` `queue_resync` | `an_out_of_range_walk_resyncs_the_client_with_0x15` |
| Item target owner: `live_facts` answers items (owner from `InvItem::owner_guid`, act from the room or the owner, static position). | `handlers/world/wired.rs` `item_facts` | `an_item_target_is_staged_with_its_owner` |

## PROVISIONAL (REC-123, HANDOFF §7)

- Monster get-hit is a direct mode change, not the AI mode-request record
  (`0x005A7E60` / `0x005A7C20`); steps 4.1, 4.2, 4.4, 4.5 (knockback, sand
  leaper, block), the umod mode 4 call and the life-percent soft test (4.8)
  are not done.
- Player branch (§7.1 step 5) makes no mode request (dodge / avoid / evade,
  block, get-hit, death start); only the soft marking.

## Left

Player get-hit / block modes; knockback and block for monsters; item facts
for items in other players' inventories (single player: not needed).

## Local check (Windows, 1.14d files)

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
cargo run -p d2-client --release -- play --new barbarian Test
```

Click a far Fallen: the Barbarian now runs to it and attacks. Hit Fallens
play a get-hit animation. Headless:
`cargo nextest run -p d2-client --test app_server_core`.
