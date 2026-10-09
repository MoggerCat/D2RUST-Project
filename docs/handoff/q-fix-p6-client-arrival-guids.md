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
