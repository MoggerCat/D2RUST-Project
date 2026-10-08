# Q: mercenary follows and fights (`claude/q-hire-follow`)

Nothing here is verified against 1.14d (rule 10); fills are `d2rs-own, unverified`.

## Links connected

| Link | Where |
|---|---|
| Per-frame hireling think (follow owner, attack hostile monsters) | new `d2-server/.../world/hireling_drive.rs`, called from `WiredWorld::after_tick` (`wired.rs`, one line) |
| Hire list rows show name, level, cost | new `d2-client/src/app/hire_stats.rs` (`hire_init` over the `hireling` rows, the same call the server prices with); installed in `app/play.rs` after `set_waypoint_map` |

## Tests

- `e2e_night_world::the_hireling_attacks_a_hostile_monster_beside_it`: a merc beside a hostile monster goes to attack mode 4 and the monster loses life (fails without the driver).
- `app::hire_stats::tests::a_row_resolves_to_the_servers_offer`.

## PROVISIONAL (REC-HIRE-DRIVE, REC-HIRE-STATS in HANDOFF.md §7)

The real Hireable think needs target providers the host lacks; the driver is a stand-in (module docs). Life / Def in the rows read 0.

## Left

1. **Follow is untested end to end.** The night-world fixture has no monstats `Velocity` for the merc class, so its walk modes end at once (path computed, velocity 0). The walk branch uses the same `change_mode` as the AI; check it with live data (below).
2. `AppRest::HirelingRest` is still a logging stub (`warp_to`, `set_owner`...): the merc is not warped when the player teleports except through the existing `pet_follows` (rest `warp_to` has no provider).
3. No XP share, drops, get-hit, skills; merc death handling is the existing `pet_deaths`.

## Your local check (Windows, 1.14d files)

```powershell
git fetch origin claude/q-hire-follow; git checkout claude/q-hire-follow
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
cargo run -p d2-client --release -- play --new sorceress Test
```
Hire a mercenary from Kashya: the list rows now read "Name - Lvl: N Life: 0 Def: 0 Cost: N". Leave town: the merc should walk after you (run when far) and, near a monster, play its attack and drain the monster's life. If it stands still, report its mode in the server log.
