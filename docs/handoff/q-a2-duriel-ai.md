# q-a2-duriel-ai: Duriel's AI from the world, Tyrael's portal spot

Branch `claude/q-a2-duriel-ai`. Nothing verified against 1.14d (rule 10). PROVISIONAL: REC-254 (`docs/HANDOFF.md` §7).

## Links connected
| Link | Before | Now |
|---|---|---|
| Duriel's AI | started by `app_a2_tyrael.rs` (`start_ai`) | `spawn_presets` starts it for host monsters the host names (`WorldPending::host_monster_ai`, Duriel): control, install state 0, think at frame + 1 |
| Partner-less portal arrival | type-12 spawn point, no free-spot step | type-12 spawn, then `free_point_step(3, 0xBE11, 7)` (`View::level_spawn_point`) |

Test: `crates/d2-client/tests/app_a2_tyrael.rs` (no AI hook; arrival spot near the type-12 spawn).

## PROVISIONAL (REC-254)
Only Duriel is started (Blood Raven, Izual, Tyrael unchanged). The portal rule ignores the quest gate (+0x3C). The "radius 7" of `quests-act2.md` is taken as the step argument of `0x0064E7E0`. All `d2rs-own, unverified`.

## What's left
Exact arrival-spot trace; Charge/Jab; AI for the other host monsters.

## The user's local check
```
cargo test -p d2-client --test app_a2_tyrael
cargo run -p d2-client --release -- play --new sorceress Test
```
In `play`: enter Duriel's Lair; Duriel walks to you and attacks without any test hook; after his death Tyrael's portal puts you in Lut Gholein near its spawn point.
