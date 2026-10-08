# q-levels-warps-all: every inter-level warp, all acts

Branch `claude/q-levels-warps-all`. Nothing here is verified against 1.14d (rule 10). Open point: REC-230 (`docs/HANDOFF.md` §7).

## Links connected

| Link | State before | Now |
|---|---|---|
| Warp pairs in the synthetic world | Act I (town, Blood Moor, Den, Cave 1, Black Marsh, Tower, Burial Grounds), Act II dungeons, Act IV only | plus chains for Cold Plains → Dark Wood → Tamoe Highland → Underground Passage 1, Act III (Kurast Docks 75 … 83, Durance 100–102) and Act V (Harrogath … 130): `app/synthetic_chains.rs`, wired into `single_player.rs` (DRLG vis/warp slots, lvlwarp ids and exits, room flags, preset tile units, room rects) |
| Act III as a world | act index 2 not created anywhere: an act change into Kurast found no DRLG and silently left the player | created in the synthetic act list, in `start_levels`, and in the live `LevelSource.acts` (town 75) |
| Warp tile of a reactivated room | with the inactive store off, a room that had gone inactive lost its tile units for good (the waypoint or act-change arrival never re-allocated them), so its warps could not be clicked | `WorldSim::restore_inactive_units` (d2-sim `wiring/worldgen/dispatch.rs`) allocates them again from the room's presets (idempotent) |
| Waypoint table acts | levels 75–102 counted as act 1 | act 2 |

Test: `crates/d2-client/tests/app_levels_warps_all.rs`. For every level of the synthetic DRLG that has warps (`single_player::synthetic_level_warps`, 80+ warps) the player is put in the level and every tile of it is clicked (C→S 0x13): the server player must end in the vis slot's level, the client's model follows, nothing is rejected. It failed before the dispatch change (Cold Plains had no tile).

Expectations changed (not weakened): `app_frame_loop` (the waypoint to Cold Plains now meets Cold Plains's own tile: 39 handled messages, one more room in the sight list, one more hidden unit).

## PROVISIONAL (REC-230)

Chain orders, ids and tile places are made up; Stony Field stays out of the chains (the waypoint test needs it unbuilt). Act III's live town room is not pre-streamed (like Harrogath): check the first act change into Kurast.

## Left

- Act I's remaining dungeons (Cave 2, Hole, Crypt, Mausoleum, Tristram, Cathedral, Catacombs) are not in the synthetic world; the live game builds them from real lvlwarp data, untested here.
- Walking onto a tile (as opposed to clicking it) is as in `q-warps`.

## The user's local check (game files)

```
cargo run -p d2-client --release -- play --new sorceress Test
```
Take the waypoint to Cold Plains and click its exits; use a save with Act III (Meshif's "sail west") and walk Kurast Docks → Spider Forest; use a save in Harrogath and leave by its gate. Expect the next level's tiles each time, no `rejected` lines. Record any tile that is not clickable and the console lines.
