# q-a5-town: Harrogath in the play preview

Branch `claude/q-a5-town`. Nothing is verified against 1.14d (rule 10). Open point: REC-144 (`docs/HANDOFF.md` §7).

## Links connected

| Link | Where |
|---|---|
| Act 4 (the fifth act) DRLG on the synthetic and live level sources; Harrogath (level 109) is a preset level with waypoint index 27 | `app/single_player.rs` (`ACT5_TOWN`, `LevelSource.acts`, `WaypointTables::synthetic`, `synthetic_types`) |
| Synthetic Act V room holds Larzuk, Anya, Malah, Nihlathak, Qual-Kehk, Cain (`ACT5_NPCS`) and a waypoint object | same file, allocation after Lut Gholein's |
| Their `monstats` / client unit rows | the single list `synthetic_npc_classes()` (`.chain(ACT5_NPCS)`) |
| Qual-Kehk's hire list | `synthetic_hire_rows()`: three `hireling` rows (act 5, barbarian class 560) |
| Menus, act change | existing (`q-npc-menu`, `q-act-travel`, the act-generic host of `q-act-worlds`) |

Test: `tests/app_a5_town.rs` (act change to 109 through the hook queue, waypoint and all six NPCs in the client model, each NPC answers C→S 0x13 with S→C 0x28).

## PROVISIONAL / left

- REC-144: positions, hire rows and the room are d2rs-own.
- Larzuk's socketing (service 35) is the sim's `npc/services.rs`; not driven end to end here (needs a socketable item in the test).
- Vendor stock needs game files (`q-vendor-items`). Nihlathak's palace and the Ancients' way are not built.
- Live: act 4 is now created at game start (like acts 0 and 1); its town room is not pre-streamed, so check the first act change into Harrogath.

## The user's local check (game files)

```
D2_GAME_DIR=<install> cargo run -p d2-client --release -- play --new sorceress Test
```
Use a save in Harrogath (or Tyrael's act change). Click each NPC: expect a menu box and no `rejected` lines; with a socketable item, Larzuk's Socket entry. Record missing or silent NPCs.
