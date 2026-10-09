# q-fix-client-missiles-rest: client missile functions, motion record, town tests

Session `q-fix-client-missiles-rest`, 2026-10-09, branch
`claude/q-fix-client-missiles-rest` (from `claude/specs-staging-7`
`87736d7`). Cloud, with the private data repo (1.14d install assembled
under `$HOME/game`, `tools/cloud-game/fetch.sh`). Continues the client
missile rows of `q-fix-render-rest.md` (create, dispatch, straight path,
functions 1/4/5/6/8/11/23/25/43/49/60/63). REC block 540–549.

## Done

| Row | Spec | Change | Tests |
|---|---|---|---|
| bodies seam | missiles/client.md §C6; client-bodies.md §B1 | `client_missiles::Env` (rows, light quality, skills tables, `monstats` rows) through every body; `update_with` / `end_with` (the play update passes `ModelInputs::skill_tables` and `tables.monsters`); `update` / `end` stay as wrappers without tables. New bodies in `bridge/client_missiles_bodies.rs`. `ClientMissileRow` gains `CltParam4`–`5`, `CltCalc1`, `Town`, `CltSrcTown`, `Size`; `MonsterClass::size_x` (`monstats2` `SizeX`). | — |
| motion record | missiles/client.md §C3 r19, r21, §C6, §C7 r3, §C9 r4.5; render/unit-composite.md §8 | `ClientMissile::motion` (`rules::unit_composite::MotionRecord`): restarted at the create, timed arc with flag 0x100 then one update, updated before each dispatch; a landed arc ends the missile (§C7 r3); the explosion missile takes m's motion position (done, restart). Setters `restart`, `done`, `moving`, `timed_arc`. PROVISIONAL REC-540 (arc flags / vz), REC-541 (getter shifts); `client.md` Open questions 9–10, PC 1 Step 4 item 24. | `client_missiles::bodies_tests` `an_arc_missile_ends_when_its_motion_lands`, `the_explosion_takes_the_missiles_motion_position` |
| function 2 (blood) | missiles/client.md §C13 2; render/camera.md §2, §4 | `ClientWorld::unit_origin` (`UnitOrigin`: getters `0x0045AFC0` / `0x0045AFD0`, W, H − 40), set by the play app on each drawn frame (`Bridge::set_unit_origin`, `present.rs`); no origin yet is a handler error. | `function_2_holds_blood_on_screen_and_ends_it_off_screen` |
| function 3 | client-bodies.md §B4 3, §B3 r1 | the missiles evaluator `0x0064B7C0` (`d2_sim::skills::levels::eval_missile` on `passive::ClientSkills`) on `CltCalc1`; sub-at-step. | `function_3_drops_a_looping_sub_missile_on_each_new_sub_tile` |
| functions 59, 65 | client-bodies.md §B5 r4, r6 | 59: motion z window (−d28, d2C); 65: `baalfx spirit` (rnd(25), seed step mod 10, rnd(2), quarter turn around U, fixed point (15135, 5900) at 0xF00). | `function_59_lives_while_its_height_is_in_its_window`, `function_65_wakes_circles_and_leaves_for_the_fixed_point` |
| town tests | missiles/client.md §C6 r3–r4, §C7 r8 | the active room of the missile's sub-tile, its level a town level; `CltSrcTown` clamp with the owner's room. | `town_rooms_remove_missiles_without_town_and_clamp_src_town` |
| function 7 (guided) | client-bodies.md §B5 r2; ui/controls.md §6 r9.7 | town removal, stale target drop (d28 bit 0), dead / not hostile target ignored, re-path toward T every k updates at distance 4…24 (`objects::distance_at` with the unit sizes of `sim/path-placement.md` §3). `combat::hostile_between` (PROVISIONAL REC-542: relation flags, `alSel` / `noSel`, pets not in the model; alignment by set-up `Align`). | `function_7_steers_toward_a_hostile_living_target_in_range`, `hostility_between_client_units` |
| function 9 (meteor centre) | client-bodies.md §B5 r5 | the skill evaluator on the skill's `calc1`, light radius growth, S1 / S2 children with their motion position and velocity; the sound at elapsed a − 2 is not modelled. | `function_9_drops_its_meteor_and_grows_its_light` |
| two-row functions | client-bodies.md §B3 r3, §B4 17, 27, 46, 52; client-bodies-2.md §B11 51; client.md §C13 18, 37, 39 | 17 (curse centre: d04 / d06, the disc helper with `RandStart`), 18 (bone spear trail: facing, precise position, motion z), 27 (fire wall maker: flags-0 child, wander on one seed step), 37 (steps; its shake and sound not modelled), 39 (rewind when owned), 46 / 52 (pairs at ±(d28, d2C); 46 removes directly, light left: Edge case 2), 51 (burst on P1 with P3 extras, owner hidden on P2). `ClientMissile::d04` / `d06`, `ClientMissileRow::rand_start`. | `function_17_and_the_disc`, `function_18_lays_its_trail_with_the_spears_facing_and_height`, `function_27_drops_fire_and_wanders`, `function_39_rewinds_an_owned_missile`, `functions_46_and_52_lay_children_on_both_sides`, `function_51_bursts_then_hides_its_owner` |
| §B11 single-row bodies | client-bodies-2.md §B10 r3, §B11 10, 13, 19, 20, 44, 45, 47, 48, 53, 58, 68 | 19 / 20 (orb tables OX / OY, quarter-step nova), 10 / 13 (class pick, reseeded shard, point test mask 4, fall), 44 (follows the owner while it uses m's skill; direct removal), 45 (scatter), 47 (d28 := bounces left, then 6), 48 (reseeded eruption points, mask 0x45, dead flag), 53 (child then function 7), 58 (reseeded wander), 68 (loops P1). `ClientMissileRow::prog_sound`, `param` (server `Param1`–`2`). | `functions_19_and_20_the_orb_and_its_nova`, `functions_10_and_13_drop_falling_shards`, `functions_44_45_53_and_68`, `function_48_erupts_at_reseeded_points`, `function_58_turns_on_its_reseeded_draw`, `function_47_keeps_its_bounces_then_lays_fire` |
| body children's owner direction | missiles/client.md §C2 r8 | `create_from`: a child made by a missile body takes the parent's direction for the aim nudge (the model holds no unit direction). PROVISIONAL REC-543; found by the install run (a handler error on a user row). | `a_bodys_child_nudges_by_its_parents_direction` |
| cell walk (REC-451 resolved) | sim/pathing.md §9.4, §9.6; sim/path-placement.md §4, §6 r3; missiles.md §R4 r6; client.md §C7 r5, r10 | The default step's path step is the §9.4 movement with the §9.6 cell walk: steps halved to ≤ 0x10000, each cell change a missile move (`d2_sim::path::footprint::missile_move` over the client DRLG, read only: a missile's footprint mask is 0) with the §C8 move mask, refused on 0x1 / 0x4 (Q := the last free cell's centre), up to 10 saved steps, the new-step flag only on an unrefused walk with a saved step; set position stops the path when the cell has no room; §C7 r10 reads the cached collided mask (velocity 0: the size query with all bits). `ClientMissile::room`, `collided`, `steps`, `step_count`, `stopped`. A WALL-only cell (0x1) no longer stops a `CollideType` 3 missile (only 0x4 or no room): the wall fixtures now stamp WALL with the missile barrier. | `tests_drlg` `a_fast_client_missile_walks_its_cells_and_stops_before_a_thin_wall`, `a_still_client_missile_reads_the_wall_under_it`, `a_client_missile_ends_on_a_wall_of_the_client_drlg` (fixture: real wall bits) |
| unit hits | client.md §C7 r11–r12, §C8, §C9 r2, r4.1, r5, r8; sim/path-placement.md §4 r6 | the unit at a point over the room adjacency and room unit lists with the collide tests (common test, hostility, alignment, `CanDestroy`), on each saved step when the word has a unit bit; the end with a unit: NextHit / LastCollide / flag / hostility gates, pierce (r = 2 keeps m), state 86 (`ClientObjects::just_hit`, counted down per client update), `CollideKill`. PROVISIONAL REC-544 (flags 0x4 / 0x8; and: no unit footprints are stamped on the client grid, so in play the search is not reached yet), REC-545 (state 86 expiry; `client.md` Open question 11). | `a_client_missile_hits_the_monster_on_its_walk`, `piercing_and_next_hit_on_a_client_missile` |
| hit functions | client-bodies.md §B3 r4, §B7; client-bodies-2.md §B10 r4, r6, §B12 | `bridge/client_missiles_hits.rs`: 1 (fire disc), 2 (ring), 3, 10 (no model overlays), 14 (seeded shard pattern, facing), 18 (meteor), 19, 24 (returns 0 on a unit), 28 (ring of 8), 29, 30 (orb novas), 31, 44, 52 (rocks; a unit hit is a handler error until OQ1), 54, 55, 56. REC-452 narrowed to 9, 12, 13, 16, 25, 26, 53 and the open ones. | `client_missiles::hits_tests` (7) |
| client unit search, hits 13, 16, 25, 26 | client-bodies-2.md §B9, §B10 r1, §B12 | `search` (C's room adjacency, town skip, room unit lists, squared distance, the filter of §B9 with hostility, the line walk `0x0064E260` over the client DRLG, state 86), next-GUID pick, bolt to each; 13 (lock / retarget), 16 (chain to the next GUID), 25 (skip `noaura`: monstats flag 27 read from `data/fields.tsv`, `MonsterClass::no_aura`; `client-bodies-2.md` OQ1 part answered), 26 (V by (d28, d2C), no cap). Filter bits the model cannot test are handler errors. | `tests_drlg` `hit_16_chains_to_the_next_guid_in_range`, `hits_25_and_26_bolt_the_units_in_range`, `hit_13_retargets_the_lowest_guid` |
| unit footprints (REC-546) | sim/path-placement.md §3, §5.1; sim/pathing.md §13.3 r5; client/msg-units.md §3 r2 | `stamp_unit_footprints` each client update: living players and monsters stamped on grid copies read only by the missiles. | `tests_drlg` `client_missiles_see_the_units_footprints` |
| error in a hit body (REC-547) | client.md §C9 r7–r8 | the error is reported and the missile removed (light dies), not left to fail each update. | `a_hit_body_the_model_cannot_run_still_ends_the_missile` |
| screen shakes (q-fix-shake-starts) | render/camera.md §8 (rows, rule W); client/msg-ui.md §19 r3 | `ClientWorld::shake` (`start_shake`, one running shake on the server tick), read by `ModelFeed::shake`; started by S→C 0x5A code 0x12 and client functions 12, 29, 31, 36, 37, 38, 54 (`bodies::shaker`) and 66 (rule W, `worldstone_shake`; its `0x004D2520` call open). Not done: the skill 301 client do (no client skill-do layer). The feed's PROVISIONAL note is gone; the per-frame player-seed draws stay REC-62. | `client_functions_start_their_shakes`, `rule_w_shakes_in_the_worldstone_levels`, `tests_ui_more` `event_text_cuts_the_name_and_code_0x12_sets_the_eclipse`, `model_feed` `the_shake_is_the_models` |
| missile sounds, umod callback | client.md §C4 r28–r29, §C9 r4.4, r6; audio/triggers.md §8 r3, triggers-2.md §19 r3; monsters/umod-callbacks.md §28 | `Output::MissileSound` (new `client/bridge.md` §10 row 45: request / owner group 314 stop / travel detach) → audio `UnitRequest` / `GroupStop` / `GroupDetach` (`first_in_group`); the umod dispatcher's phase 4 with umod 29's multishot copies (guard flag). PROVISIONAL REC-548 (detach force), REC-549 (no monster target unit). | `missile_sounds_at_the_create_and_the_end`, `audio::triggers` `the_group_walk_finds_the_first_handle_newest_first`, `a_multishot_unique_monster_doubles_its_client_missiles`, `output` `each_table_row_is_one_variant_with_its_producer_and_consumer` |
| real install | — | `app_client_drlg` `the_users_client_missiles_run_their_functions` runs every user row of the 37 functions of its `RUN` list (1–11, 13, 17–20, 23, 25, 27, 37, 39, 43–49, 51–53, 58–60, 63, 65, 68) for 60 updates with the user's skills tables and a drawn frame's origin; every function has a row made. | install: 4 passed |

## Not done

- Functions 15 (needs the owner's target `0x004648F0`), 50 and 57 (need
  the owner's client target position `0x004C52E0`: the model holds no
  client path for units); the open helpers of 29, 38, 66 (`0x004CE850`,
  `0x004CE530`, `0x004CECC0`, `0x004D19D0`, `0x004D2520`) and the open
  parts of 31, 36, 54; the open bodies of §C12 wait on the spec.
- Hit functions 9 (ally / demon / undead tests the model lacks), 12
  (fire patch), 53 (`0x004DA340` unspecified), 52 with a unit (monstats2
  flag 11, `client-bodies-2.md` OQ1); the open ones of §B6 (REC-452).
- `ProgSound` of functions 9, 47, 51 (conditions: `audio/triggers.md`
  OQ 6), the default removal's travel sound stop (the unit free detaches
  it), the init callbacks of callers outside the model, the client event
  hooks, the second pass (§C7 r13).
- The skill 301 shake (client skill do, not in the model).

## Provisional points (REC 540–545)

REC-540 (timed arc flags / vz), REC-541 (motion getter shifts), REC-542
(hostility parts the model lacks), REC-543 (a body's child takes its
parent's direction for the aim nudge), REC-544 (unit flags 0x4 / 0x8), REC-545 (client state 86 expiry), REC-546
(unit footprints on grid copies for the missiles), REC-547 (a hit body
error still ends the missile), REC-548 (travel sound detach force),
REC-549 (umod 29 copies aim at the path target).
REC-451 resolved, REC-452 narrowed. Spec gaps: `missiles/client.md` Open
questions 9–11 (PC 1 Step 4 item 24); `client-bodies-2.md` OQ1 flag 27
answered from `data/fields.tsv`.

## Gate and real data

Each push: `cargo fmt --check`; `cargo clippy -p d2-sim -p d2-server -p
d2-client -p test-fixtures --all-targets -- -D warnings`; `cargo nextest
run` on those crates (7479 passed after the staging-7 merge);
`tools/coverage.py --check`, `--selftest`; `tools/spec_index.py
--check`; `tools/methods.py check`. Real install (`D2_GAME_DIR` from
`tools/cloud-game/fetch.sh`): `cargo test -p d2-client --test
app_client_drlg client_missile -- --ignored`, 4 passed.

Disk: the d2-client integration test binaries outgrow the cloud disk
allowance; build with `CARGO_INCREMENTAL=0
CARGO_PROFILE_DEV_STRIP=symbols CARGO_PROFILE_TEST_STRIP=symbols` (and
`CARGO_PROFILE_{DEV,TEST}_DEBUG=0`), and delete the private repo clone
once the install is assembled.
