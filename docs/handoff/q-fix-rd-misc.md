# q-fix-rd-misc

Branch `claude/q-fix-rd-misc` (REC ids 1565–1569; used: 1565, settled).
Real-data failures routed here: `game_town_run` (x 4899 vs 4901) and the
three `play_smoke` tests. `app_cain_quest` and `seam_drlg_coords` were fixed
by q-fix-client-realdata (merged through staging) and pass here.

## Done
| Test | First divergence / cause | Fix | Commit |
|---|---|---|---|
| d2-server `game_town_run` | not d2-sim movement: the 12-sub-tile run target (4901, 5634) is a Torch1 Tiki preset (objects class 37, 1 × 1, HasCollision in every mode); the size-2 pattern meets it from x 4900 to 4902, so target preparation (`pathing.md` §4 r2–3) moves the request to 4899, the walk too | the free leg is 10 sub-tiles; the torch leg is its own test (`a_run_onto_the_torch_stops_at_the_first_free_cell`: ends at 4899) | a1eb0a1d |
| `play_smoke the_scripted_play_run` | the run-to-unit (0x04) toward the waypoint 36 sub-tiles away stops at the torches on its row (toward walk: no free step for the size-2 pattern, §5.2 r4; A* only within 18, §6 r3), so the interact was sent from 30 away | the harness goes round the torches 5 rows south before the run to the unit | a68bf699 |
| `play_smoke the_spec_npc_ui_on_the_install`, the scripted run's 4 "run leg" findings | the client path had no object footprints: drawn through the torches at (4886, 4221), (4925, 4210) while the server stopped short | client object footprints (with q-fix-walk-desync's parallel `OtherObject` path, merged): read from `re/`, `0x004BC720` stamps with `0x00620A70` when objects +0x120 + mode ≠ 0, once at the 0x51 mode; `0x00623830` frees it. d2rs keeps it as `ObjectData::footprint` (set at 0x51, cleared by the generic step 1 → 2 turn and the door finish). `client/msg-units.md` §1.3 r2; REC-1251 and REC-1565 settled | b523610e, 8c3ea02c, merge 0b56cfd7 |
| `play_smoke the_live_run` (dropped 0x6D) | Gheed (class 147, guid 4): 0x0A when the player's room switched 3 → 9; he then walked from room 8 into room 10 (adjacent to 9) and was never re-announced: `PathCtx` left `send_unit_add` / `send_unit_removal` as the seam's no-op defaults, so the §9.8 room-change merge sent nothing; his 0x6D at tick 76 was dropped | `PathCtx` sends 0x0A (`0x00571600`) and the add messages (`0x00571F90`, `View::add_messages`) | 78e380c2 |
| `play_smoke the_live_run` (gold) | the drop was a lone scroll (`tsc `); the test asserted gold for any drop | the gold check runs when gold was dropped | 8c3ea02c |

## Open
- `the_live_run` after the pickup: see "Last run" below.
- No 1.14d scenario check yet for §9.8 (a monster crossing into the
  client's rooms) or for the client object footprints. A packets check
  with a town walk would compare the 0xAC / 0x0A, but d2rs's game seed
  order differs from frame 2 (walk-town-ama ignores `seed`), so NPC walks
  diverge before that; a client-grid recording at a torch (as REC-706 did
  for Warriv) would check the footprints.
- The `game_town_run` town (INIT 644409375) has no 1.14d recording; the
  torch there comes from the DS1 preset (the recorded seed-1234 town has
  torches at (4925, 4210), (4911, 4210), (4886, 4221), ...).

## Repro
```
export D2_GAME_DIR=$HOME/game        # private repo tools/assemble.py
cargo run -q -p data-tool -- excel-dir $D2_GAME_DIR/extracted/patch_d2/data/global/excel $D2_GAME_DIR
cargo nextest run -p d2-server --run-ignored only --test game_town_run
cargo nextest run -p d2-client --run-ignored only --test play_smoke --no-fail-fast
cargo nextest run -p d2-client --run-ignored only --test seam_drlg_coords --test app_cain_quest
```
Disk: never build every d2-client test binary (it fills the allowance);
one `--test` at a time, delete `target/debug/deps/<binary>-*` and
`target/debug/incremental` after.

Ledger rows: `docs/handoff/ledger/q-fix-rd-misc.tsv` (3 rows, NO-CHECK).
