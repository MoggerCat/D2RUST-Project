# q-a2-charge-jab: Duriel's Charge and Jab

Branch `claude/q-a2-charge-jab`. Nothing verified against 1.14d (rule 10). PROVISIONAL: REC-274 (`docs/HANDOFF.md` §7).

## Links connected
| Link | Before | Now |
|---|---|---|
| Charge start (srvst 31) on a monster | set the moving flag, target and hit mode through `Pending::entry_*`, which were no-ops for a unit without a skill list | `LocalSeams` keeps flags and params 1…4 per unit (`skill_rest.rs`); the monster frame event sees the moving flag, steps the path, and do 67 lands the hit |
| Duriel's AI 44 with `Skill1` Charge / `Skill2` Jab | never reached (no rows) | charges at a distance (`aip5`), jabs when near (`aip3`) |

Test: `crates/d2-client/tests/app_a2_charge_jab.rs` (synthetic Lair, Duriel from the world). Before the change Duriel entered Charge mode and returned to neutral without moving (distance stayed 8, no damage).

## PROVISIONAL (REC-274)
Skill rows, `Sk1mode`/`Sk2mode`, `aip3 = aip5 = 100`, `Run` and the animation frame codes are test fixtures. Live `aip5` is 0, so the real Duriel never charges.

## What's left
Smite / Holy Freeze rows; real animation event frames for the charge; AI for the other host monsters.

## The user's local check
```
cargo test -p d2-client --test app_a2_charge_jab
cargo test -p d2-client --test app_a2_tyrael
```
Both pass. (`play` with real tables: Duriel keeps his live `aip5 = 0`, so no charge is expected there.)
