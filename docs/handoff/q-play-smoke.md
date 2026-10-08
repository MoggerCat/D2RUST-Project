# q-play-smoke: end-to-end play smoke test

Stitching session `q-play-smoke`, branch `claude/q-play-smoke`. Read only
`specs/`, `docs/`, `crates/`, `tools/`. Synthetic fixtures only; nothing
here is verified against 1.14d (rule 10). PROVISIONAL points: REC-277 in
`docs/HANDOFF.md` §7 (F1, (b) the room delete notice, (c) the position
check).

## What runs

`crates/d2-client/tests/play_smoke.rs`: the play app headless (bridge,
in-process server on its thread, walk prediction, original UI with memory
fixtures; no window). After every step `Run::check` asserts:

- the server refused nothing: every drained C→S message is
  `Handled::System` or `Dispatched(ResultCode::Done)` (`Probe` reads
  `LocalLink::last_frame` after each pump);
- the client never sent C→S 0x5F (the rubber band, F7), and after each
  run leg the drawn player stands within 2 sub-tiles of the server's
  (F8);
- the client model has no unhandled, dropped, rejected or discarded S→C
  message (`ReceiveLog`, `bridge.md` §6);
- no system panicked (a bridge error panics through Bevy's handler).

| Test | Data | Steps | State |
|---|---|---|---|
| `the_scripted_play_run` | synthetic `GameData` | join → Akara talk → Trade (shop opens, its close sends 0x30) → waypoint → Cold Plains | clean |
| `the_field_leg_kills_levels_up_and_spends_a_point` | synthetic + combat tables installed before the join | Cold Plains → kill (0x06) → level 2 → 0x3A | clean |
| `the_live_play_game_builds_on_the_five_act_install` | five-act install (`GameData::select`) | the play game's live build | clean |
| `a_blocked_run_is_drawn_where_the_server_stops` | five-act install, a wall stamped in both grids | a run into the wall: the server stops at it, the drawn player too | clean |
| `the_live_run` | five-act install, loaded as `d2-client play` loads the user's files | join → on foot into the Blood Moor (0x03 legs) → a spawned monster killed (0x06) → experience → its drop (gold, weapon, potion) → run to and pick up each (0x04, 0x16) → start weapon off and on (0x1C, 0x1A) → level 2, a stat and a skill point spent (0x3A, 0x3B; the client learns Firebolt) → Town Portal scroll read (0x20), the portal taken back to the Rogue Encampment → saved (`save::write_file`), the game dropped, the file loaded (`load_character`) into a new game → the same level, stats, skills and items | clean |

The whole smoke path of the task row runs end to end.

## Fixes (what broke and how)

| # | Break | Fix | Where |
|---|---|---|---|
| F1 | Trade with an empty store never opened the shop, so no C→S 0x30: the player stayed in the NPC interaction | the Trade / Gamble choice opens the shop on the next shop poll (REC-277) | `ui/npc_menu_ui.rs`, `ui/shop_ui.rs` |
| F2 | the play game's live build stopped at `Drlg(UnknownLevel(40))` on the Act I fixture install (every act is created) | `test_fixtures::acts::all_acts`: the five act variants merged into one install | `crates/test-fixtures/src/acts.rs` |
| F3 | the field leg's attack dealt no damage | fixture gap: a raw-allocated monster lacked unit flag 0x02 (monster init sets 0x0A) | test fixture |
| F4 | running out of town, S→C 0x96 was rejected (no visibility predicate) | taken from staging (q-fix-visibility); this branch's own version was dropped. q-smoke-town's `f952c9b2` is not merged (F7 replaces it) | — |
| F5 | a dropped item's 0x9C carried x = y = 0: the stream wrote the inventory cell, and the death drop gave the item no static path without the walk-back field | `bitstream.md` §4.1 r2: ground / dropping items write the static path position (`UnitHooks::static_position`); the drop's path part runs whenever there are path records (staging made the same death-drop change; its version is kept) | `d2-sim` `wiring/inventory/bits.rs`, `units/hooks.rs`, `wiring/action/units.rs`, `wiring/economy/death.rs` |
| F6 | a picked-up gold pile, freed on the server, stayed on the client's ground (no S→C 0x0A) | the play host answers the room delete notice with 0x0A (REC-277 (b)) | `d2-server` `handlers/world.rs` |
| F8 | the drawn player ran on through walls and objects (the straight-line prediction; q-smoke-town break 5: into the stash) | the prediction steps the player's own path with `d2-sim`'s request / compute / step over the client DRLG's grids (`ClientPath`, REC-277 (d)) | `d2-client` `bridge/client_path.rs`, `bridge/predict.rs` |
| F9 | a server run moved at walk speed (no run stat list, `pathing.md` §8.2), so the client path (at the spec's +50 %) arrived first and a late 0x96 snapped it back | ported from pc1's `875b61c6` unchanged (its sim, server and spec parts; not its local-cell client part): the run list attached on a run start, freed at the next mode change | `d2-sim` `wiring/path/walk.rs`, `units/modes.rs` |
| F7 | rubber band on real data: a blocked or rerouted run made the position check send C→S 0x5F with a cell the server had not walked (the last placement on staging; the straight-line prediction past the obstacle with the local-cell variants, q-smoke-town `f952c9b2` / pc1 `875b61c6`), and the server walked the player there (`pathing.md` §1.6) | while the preview predicts, rule 8 for the local player follows the server: no 0x5F, the prediction snaps to the server point. Rules 3–7 are unchanged, so staging's `ViewVisibility` is still what the check asks (REC-277 (c)) | `d2-client` `bridge/check.rs`, `bridge/world.rs`, `world_view/walk_room.rs` |

Fixture fills of the live set (in the test, the shared set unchanged):
`itemstatcost` to 359 rows (stat 67 velocitypercent was dropped: runs at
25 %), `monlvl` `L-` columns (single player reads them: no experience,
to-hit, damage), Attack's `anim` / `srvdofunc`, player AnimData,
`itemtypes` on 1.14d's row numbers (gold on row 7 was made as an ear),
`playerclass` row 7 with the empty code (an empty `class` link failed:
nothing equippable), `isSpawn`, every TC item dropped, `sb1` level 1,
a `scro` item type and the `tsc` misc row (`useable` 1: without it 0x20
is malformed), one `tsc` in the sorceress's start items, `objects` row
59 (TownPortal: OperateFn 15, InitFn 11; the client refused an object
class without a row). Each is a place where the fixture differs from
1.14d's own tables.

## Open findings (for the coordinator)

- **The client path (F8, REC-277 (d)).** The drawn player now stops at
  the wall the server stops at (`a_blocked_run_is_drawn_where_the_server_stops`
  fails on the straight line, passes on the path), and every run leg of
  the live run asserts the drawn player within 2 sub-tiles of the
  server's. Gaps: other units' footprints are not on the client grids
  (a path the server bends round a monster is drawn straight, then
  snapped by the next 0x96); the client step's inputs are a reading, not
  a recording (REC-51). The fixture tiles carry no collision, so the
  wall is stamped by the test in both grids. With F8, F7's rule-8 branch
  should rarely fire; it stays for the gaps.
- q-smoke-town's break-5 repro (the stash) is not on this branch yet; it
  should pass once merged (un-ignore it then).
- Staging carries two visibility predicates: q-smoke-town's always-true
  one installed by `play::add_game` (REC-278) and `ViewVisibility`
  installed by `play::run` (overriding it). Headless tests get the
  always-true one.
- New-character start items do run on live data (the sorceress holds a
  sword, potions); a dropped magic item is unidentified and cannot be
  equipped (`inventory.md` §4.2 r6), as 1.14d.

## Next steps

- Client units' footprints on the client grids (the monster gap above).
- Un-ignore q-smoke-town's break-5 repro once it reaches staging.
- Unify the two visibility predicate installs.
- The same run on real 1.14d tables with `D2_GAME_DIR` (local queue),
  where the fixture fills above are not needed.

## The user's local check

```powershell
git fetch origin claude/q-play-smoke
git checkout claude/q-play-smoke
cargo test -p d2-client --test play_smoke
```

All five pass. In the window (`cargo run -p d2-client --release -- play
--new sorceress Test`): Trade with Akara opens the shop even before items
show, and closing it lets you use the waypoint; a monster's drop lies
where it fell, gold picked up leaves the ground; running into a wall or
the stash, the player stops at it and is not pulled back (F7, F8);
running is half again as fast as walking (F9).
