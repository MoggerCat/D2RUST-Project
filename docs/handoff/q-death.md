# q-death: player death in the play preview (`claude/q-death`)

> Stitching session, 2026-10-08. Read only `specs/`, `docs/`, `crates/`,
> `tools/`. Synthetic fixtures only. Nothing here is verified against
> 1.14d (rule 10); provisional parts are REC-95 and REC-96 in
> `docs/HANDOFF.md` §7. Sound not wired.

## 1. The path, and what was missing

| # | Link | Before | Now |
|---|---|---|---|
| 1 | Life reaches 0 → DT start `0x00580EC0` (penalties §4.6, mode 0) | the sim had the rules (`UnitHooks::player_death`, `death.rs`) but nothing called them | `wiring/action/dying.rs`: `ActionSim::player_deaths` (called from `WiredWorld::after_tick`, `d2-server/.../world/wired.rs`) starts DT for a player whose life (stat 6) is 0 after it was seen with life; `ActionSim::start_death` is the entry a lethal hit will use (`damage.md` §7.1 r5.4) |
| 2 | DT → DD (ENDANIM, `0x0057FCA0`) → corpse | `Pending::create_corpse` default `None`: no corpse | `ActionHooks::allocate_corpse` (on in the preview: `single_player.rs`, `death.allocate_corpses`): player-type unit, mode 17, state 7, owner GUID; the §4.7 experience and the pickup (`corpse_pickup`) use it |
| 3 | S→C | none for a player's death | 0x0D code 8 / 9 / 7, and 0x59 + 0x0D code 9 for the corpse (REC-95) |
| 4 | Client model | `bridge/modes.rs` already maps codes 8 / 9 / 7 to modes 0 / 0x11 / neutral | unchanged; the art follows `unit.mode`, so the death and corpse modes draw |
| 5 | Death screen, Esc | none | `app/death.rs`: text while mode 0 / 0x11 (REC-96); Esc in mode 0x11 sends C→S 0x41 (`panels.md` §3 r1) |
| 6 | C→S 0x41 → respawn in town | handler existed (`handlers/player.rs` `resurrect`) but the dispatch gate is staged at mode 1, so the dead gate never opened | `WorldHost::player_gate` refreshes the staged gate from the live unit mode after each tick, for players that have died (`sim.rs` `tick`) |
| 7 | Gold loss | rule in `death.rs` | runs at the DT start (stat 14 → 0, `goldlost`); no gold piles (REC-95) |

## 2. Tests

- `wiring/action/tests/player_death.rs` `a_player_with_no_life_dies_and_leaves_a_corpse`: DT once, the penalties, 0x0D code 8 to both players, ENDANIM → DD with the corpse (0x59 + code 9), the pickup by the owner only, code 7 after the respawn.
- `tests/app_play_death.rs`: the whole path over the synthetic game through the bridge: DT → client mode 0 and the screen up; DD → client mode 0x11 and one corpse unit on the client; Esc → C→S 0x41 → the player is back in town (mode 5), the screen is down, the corpse stays. (The synthetic stat table is empty, so the test starts DT with `start_death` instead of lowering life, and the synthetic anim table has no record, so ENDANIM is driven by hand.)

## 3. What is left

1. Something that takes life: monsters do not attack the player yet (`stitch-combat.md` rows 8, 12), so a death needs `start_death` or a real life stat reaching 0.
2. Items to the corpse and the corpse pickup click: the client does not send the pickup for a corpse (a click on a mode-17 player unit); the server side `corpse_pickup` is tested.
3. Gold piles at the death spot; the ear drop; the character save at the DD start.
4. The death animation's ENDANIM with real AnimData (on the user's files the DT animation should end by itself).

## 4. The user's local check (Windows, PowerShell, 1.14d files)

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
git fetch origin claude/q-death
git checkout claude/q-death
cargo test -p d2-client --test app_play_death
cargo test -p d2-sim --lib player_death
```

Both pass. In `play`, nothing kills the player yet (item 1), so the screen cannot be reached by hand.
