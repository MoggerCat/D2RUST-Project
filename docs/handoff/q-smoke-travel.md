# q-smoke-travel: travel flows end to end (readiness check)

Branch `claude/q-smoke-travel`. Test file:
`crates/d2-client/tests/smoke_travel.rs` (bridge + in-process server +
sim, no window, synthetic game).

## What the smoke covers

Each move runs through `Rig::travel` / `Rig::arrive`, which checks:

- no C→S message the server did not take (every drained message's
  `Outcome` is a dispatch with result 0; read from `LocalLink::last_frame`
  after each pump);
- no S→C message rejected, dropped (unit not in the model), unowned or
  discarded by the client (`ReceiveLog`);
- the server player's level and the client's `player_level` agree;
- the model position is where the last S→C 0x15 / 0x59 of the move put
  the player (`client/model.md` §3 r3), and the client room holds it;
- every unit the model placed in the old level is gone from the model
  once its room is freed (`model.md` §16; 0x4B → 0x0A round trip).

| Test | Flow | Result |
|---|---|---|
| `the_act_{i..v}_waypoint(s)_…` | town waypoint operated (0x13, menu 0x63), closed (0x49 level 0), every field waypoint of the act taken (0x49) | pass |
| `the_waypoint_tabs_…` (2) | town → next act's town, town → previous act's town (0x49) | pass |
| `a_town_portal_in_act_{i,v}_…` | portal pair in a field, to town through it, back through the town portal, the pair goes | pass |
| `a_town_portal_in_act_iii_…` | the same in Spider Forest | **ignored: B4** |
| `warriv_…` (2), `meshif_sails_east_…`, `tyrael_…`, `cain_…` | NPC travel rows (0x13 walk-up, chat 0x28, 0x38) | pass |
| `meshif_sails_back_west` | Meshif in Kurast Docks | **ignored: B5** |
| `every_act_{i..v}_warp_goes_down_and_back_up` | every level warp of the synthetic game (Duriel's Lair aside), depth first, down and back up | pass |
| `a_town_keeps_its_npcs_and_waypoint_after_a_long_game` | repro of B2 | **ignored: B2** |

Stand-ins (the synthetic world has no such objects; nothing in the
engine is bypassed): a field waypoint is learned by its record bit (no
field waypoint objects); a first arrival a test needs without a travel
row uses the act-change queue (`Rig::put`), and so does Act III → IV
(no Mephisto portal in the synthetic Durance); the Town Portal is opened
by `ActionSim::open_town_portal`, which the scroll's C→S 0x20 ends in
(the synthetic item tables have no `tsc` / `tbk`); an object or NPC is
walked to with a ground-click walk (C→S 0x01) before its 0x13, not with
the click dispatcher's walk-to-unit.

## Breaks found and fixed

1. **Warriv missing from the Act I town** (`app/town_npcs.rs`, commit
   "town_npcs: place Warriv …"). The synthetic Rogue Encampment had his
   class rows but no unit, so Act I → II by NPC was impossible in the
   preview. REC-280 (his place).
2. **Client panic after a portal or level change** (`bridge/predict.rs`,
   commit "predict: end the walk on a level change"). The prediction
   kept its walk target across a placement; after a Town Portal the
   target was a point of the old level hundreds of sub-tiles away, and
   `d2_sim::path::walk::geom::direction_vector`'s `127 * ly` overflowed
   i32 into a negative `tan` index: `index out of bounds` on the Bevy
   task thread. A change of the player's level now ends the walk.
   Unit test `a_level_change_ends_the_walk`.

## Still broken (not fixed here)

- **B2: idle rooms lose their units (high).** The server frees a room
  no client has been near for a while (`drlg/active.rs`
  `remove_active_room` → `UnitLists::free_room`). Its units are only
  unlinked (room `None`) because the play host never turns on the
  inactive store (`ActionHooks::enable_inactive_store`), and the
  store's restore seams (`Pending::restore_monster`, `restore_other`)
  are no-ops in every host. The synthetic towns' NPCs and waypoints are
  placed by hand at build (live towns get them from presets at
  population), so after a short game an unvisited town comes back with
  tiles but no NPCs or waypoint. Repro: Cold Plains, town, Cold Plains,
  then the act change to Lut Gholein. Fix: enable the store in the play
  host and implement the §3.4 restores (`sim/units.md`), or make the
  synthetic towns' units presets of their rooms. The smoke tests use a
  fresh game per act because of it.
- **B3: the arrival walk-outs are not drawn (open question).** A warp
  (`path-placement.md` §12.2 r5–6) and a Town Portal (`objects.md` §12
  r11) put the player at the arrival point and walk it out (S→C 0x0D
  code 1 to the walk target, then 0x15 at the arrival point); the server
  player ends at the target, the drawn player stays at the arrival
  point. The waypoint arrival sends the same 0x0D shape (x + 3, y + 3,
  `waypoints.md` edge case 5) while its server player does not move, so
  no client rule built from the message order alone matches both. How
  the 1.14d client moves its own player on these requests is
  `client/model.md` open question 2, settled by REC-51. The smoke checks
  the model position (placement) only.
- **B4: the Kurast Docks town portal does not take the player back**
  to Spider Forest (the server player stays in town after the click;
  Act I and Act V do the same flow and pass). Not diagnosed.
- **B5: Meshif is not in the client model** at a Kurast Docks arrival
  by the act-change queue (the waypoint and the room's tiles are; the
  Act III NPCs are placed by `town_npcs::ACT3` offsets that may lie
  outside the rooms the client gets). Not diagnosed; may be B2.
- Bare C→S 0x13 to an out-of-range object or NPC is answered with
  nothing (result 0): correct server behavior, but tests that use
  `Bridge::interact` without walking up only work when the spawn is in
  range (the existing Act III / V waypoint test sends its 0x49 without
  ever seeing the menu).

## Local check

```
cargo nextest run -p d2-client --test smoke_travel
cargo nextest run -p d2-client --test smoke_travel --run-ignored only   # B2, B4, B5 repros (fail)
```

In `play` (synthetic): talk to Warriv in the Rogue Encampment (left of
Charsi) after Sisters to the Slaughter; take a Town Portal and walk again
right after arriving (no crash).
