# q-fix-p6-client-arrival-guids — hand-back

Branch `claude/q-fix-p6-client-arrival-guids` (from `claude/specs-staging-7`,
with `claude/q-fix-real-unit-seed-order` merged in). Implementation session:
specs/, docs/, crates/, facts/, traces/ and the private data repo only.

## Rows

| Row | What changed | Check |
|---|---|---|
| q-fix-real-client-guid-start | `bridge/objects/mod.rs`: `ClientObjects::default` starts the counter at 1; `create_client_unit` takes counter + 1 (wrapping) and stores it first (a failed create still uses its GUID). PROVISIONAL REC-objclient-1 note removed. | unit test `the_client_guid_counter_starts_at_one_and_gives_the_value_after_the_add` (2, 3; −1 → 0) |
| q-fix-real-town-critters | New `bridge/critters.rs` `room_pass` (model.md §5 r6.1–6.3, population.md §11.7), run first in `update_pass`. Rooms of the client act list in list order whose flag bit 0 is clear: critters (`pct` per slot, `rand(MaxGrp − MinGrp) + MinGrp`, `camt` slot 0 = +0xDC, 10 tries `x0 + rand(w − 1)`, `y0 + rand(h − 1)`, size query 0x3F11, no GUID when all fail), then client presets (flag bit 0, list order, + room origin, type and mode), then bit 0 := 1. New seam `LevelTypes::client_presets` (`d2-sim` drlg; `Presets::client_presets`), `ClientDrlg::{client_presets, populated, set_populated, room_seed}`; the client `ActList` keeps the populated bit and returns it from `remove_active_room` (`other flags := flags & 1`). Data: `LevelRow::critters` (`cmon`, `cpct`, `camt`), `MonsterClass::{min_grp, max_grp}`. | real-data test below |
| q-fix-real-critter-ai | `critters::c_monsters` in the C monsters' walk after the S units: `0x0046D780` (T at monster data +0x30, 0 at the create; nearest player slot; D > 30 → T = min(D − 15, 200); chicken/bug body with the flee test, `seed % 100 < 30` walk step, else T = 5). | unit tests `a_new_critter_reads_t_zero_…`, `a_far_player_sets_the_timer_…` |
| q-fix-p6-client-arrival-guids | `crates/d2-client/tests/app_client_arrival_guids.rs` (ignored, D2_GAME_DIR): ScnAma seed 1234 join, the first update with client units: every set-C (GUID, class, type, x, y) equals the 205 `create` records of `traces/client/a1-arrival-creations-ama.jsonl`, and the 15 critters' first AI call read T = 0 (`think0`). | **passes** (205/205, 15/15) |

## PROVISIONAL notes (REC 741–749)

- **REC-741** (`client/model.md` §5 r6.4): the zoo body `0x0046D660`
  (`monstats` flag 22) and the class bodies other than `chicken_ai` are
  not read; d2rs runs nothing for those classes.
- **REC-742** (`client/model.md` §5 r6.4, §19): a C monster's client path
  is not specified. d2rs draws the step's two seed values, sets the mode
  of the request's code (1 → WL, 0x0C → S1, code 7 → neutral fallback)
  and keeps the unit in its cell; the critter's 0xAC set-up (first
  frame, light) is not run on set C. Needed: the C monster path record
  and walk end (the recorded chickens' positions from tick 8).
- REC-546 (existing): the placement size query reads the client grids
  with the server units' footprints stamped at the pass start plus each
  new critter's footprint.

## Not done

- The level-8 pass `0x0046BE60` (with `[0x007A745C]`) of r6.1.
- Outdoor-room substitution units carry no flag word in the model, so
  only preset-room units can be client presets (the arrival's 190
  presets all come from preset rooms; the test passes).
- Next on this branch: the monster-init RNG divergence
  (`monsters/init/create.rs:175` `roll_range` vs `roll`).

## Follow-up: soak findings (a) position desync, (b) dropped 0x6D

Reproduced with `crates/d2-client/tests/play_smoke.rs` (ignored, real
data): `the_spec_npc_ui_on_the_install` had 4 run legs 7–27 sub-tiles
off the server, `the_live_run` failed on `dropped {6D: 01}`.

(a) The local player's client path walked on grids the server's path
does not use:

- **Objects**: the server stamps each object's `SizeX` × `SizeY` box
  (mask 0x400 / 0x8000 / door masks, `sim/path-placement.md` §3, §5.2);
  the model stamped none, so the client ran through the town objects
  the server stops at. `stamp_unit_footprints_with` now stamps objects
  in a `HasCollision` mode; the grids without the local player
  (`ClientObjects::other_grids`) are the client path's base
  (`ClientPath::set_unit_footprints`, every tick and after a place).
- **Walk to a unit**: the server lifts the target's footprint for the
  compute (`sim/pathing.md` §3 step 6) and reads its size; the client
  path could not (`unit_size` 0, no removal). The stamp pass records
  each unit's size and footprint (`ClientObjects::footprints`), the
  path lifts the target's (`set_target_footprint`).
- **Dead monsters**: a monster killed in view keeps its footprint on
  the server (no spec rule changes it; `sim/path-placement.md` §5.3
  r3's "death code" names no call), so the model now stamps it too
  instead of skipping it; a monster added dead gets the corpse
  footprint (`msg-units.md` §1.2 r6.4, `monstats2` `deadCol`). If the
  1.14d death code does set the corpse footprint, server and client
  both change (open with the spec owners).
- **Attack approach**: C→S 0x06 / 0x0D (and holds) on a unit out of
  melee range make the server run to it (`skills/use.md` §3 r6); the
  prediction now does the same (`predict::approach_of`,
  `approach_walk`, `PreviewWalk::frame_with`).

Result: the NPC run has no divergence; the live run has one leg 4
sub-tiles off (an approach to a moving monster whose model position
lags the server's). PROVISIONAL REC-743 (unit footprints on the client
grids), REC-744 (the approach prediction), in `client/model.md` §5 r6.

Not fixed, for the coordinator: once the prediction diverges nothing
reconciles it. 1.14d's client sends C→S 0x5F with its own position on a
failed position check and the server resyncs to it (`sim/pathing.md`
§1.6); d2rs keeps the model at the server's position and never sends
0x5F (REC-277, a design choice). Making the client the position source
as 1.14d does is a design change, not taken here.

(b) A unit walking between rooms got no add or removal: the walk seams
`send_unit_add` / `send_unit_removal` (`sim/pathing.md` §9.8) were
default no-ops in the wiring. `PathCtx` now sends the add messages
(`View::add_messages`, `intents-events.md` §7.2) and S→C 0x0A. Gheed
walked out of the player's rooms (0x0A) and back in (no 0xAC), so his
0x6D was dropped; now he is added again. `the_live_run` passes that
check.

Also in `the_live_run`: the gold assertion now runs only when a gold
pile dropped (this seed's fourth kill drops light gloves only).
`the_scripted_play_run` still fails "the waypoint menu: not reached":
the server cannot path from (4929, 4207) to the waypoint 30 sub-tiles
away (client and server agree; not a desync), failing before this
change too.
