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
| motion record | missiles/client.md §C3 r19, r21, §C6, §C7 r3, §C9 r4.5; render/unit-composite.md §8 | `ClientMissile::motion` (`rules::unit_composite::MotionRecord`): restarted at the create, timed arc with flag 0x100 then one update, updated before each dispatch; a landed arc ends the missile (§C7 r3); the explosion missile takes m's motion position (done, restart). Setters `restart`, `done`, `moving`, `timed_arc`. PROVISIONAL REC-540 (arc flags / vz), REC-541 (getter shifts); `client.md` Open questions 9–10, PC 1 Step 4 item 21. | `client_missiles::bodies_tests` `an_arc_missile_ends_when_its_motion_lands`, `the_explosion_takes_the_missiles_motion_position` |
| function 2 (blood) | missiles/client.md §C13 2; render/camera.md §2, §4 | `ClientWorld::unit_origin` (`UnitOrigin`: getters `0x0045AFC0` / `0x0045AFD0`, W, H − 40), set by the play app on each drawn frame (`Bridge::set_unit_origin`, `present.rs`); no origin yet is a handler error. | `function_2_holds_blood_on_screen_and_ends_it_off_screen` |
| function 3 | client-bodies.md §B4 3, §B3 r1 | the missiles evaluator `0x0064B7C0` (`d2_sim::skills::levels::eval_missile` on `passive::ClientSkills`) on `CltCalc1`; sub-at-step. | `function_3_drops_a_looping_sub_missile_on_each_new_sub_tile` |
| functions 59, 65 | client-bodies.md §B5 r4, r6 | 59: motion z window (−d28, d2C); 65: `baalfx spirit` (rnd(25), seed step mod 10, rnd(2), quarter turn around U, fixed point (15135, 5900) at 0xF00). | `function_59_lives_while_its_height_is_in_its_window`, `function_65_wakes_circles_and_leaves_for_the_fixed_point` |
| town tests | missiles/client.md §C6 r3–r4, §C7 r8 | the active room of the missile's sub-tile, its level a town level; `CltSrcTown` clamp with the owner's room. | `town_rooms_remove_missiles_without_town_and_clamp_src_town` |
| function 7 (guided) | client-bodies.md §B5 r2; ui/controls.md §6 r9.7 | town removal, stale target drop (d28 bit 0), dead / not hostile target ignored, re-path toward T every k updates at distance 4…24 (`objects::distance_at` with the unit sizes of `sim/path-placement.md` §3). `combat::hostile_between` (PROVISIONAL REC-542: relation flags, `alSel` / `noSel`, pets not in the model; alignment by set-up `Align`). | `function_7_steers_toward_a_hostile_living_target_in_range`, `hostility_between_client_units` |
| function 9 (meteor centre) | client-bodies.md §B5 r5 | the skill evaluator on the skill's `calc1`, light radius growth, S1 / S2 children with their motion position and velocity; the sound at elapsed a − 2 is not modelled. | `function_9_drops_its_meteor_and_grows_its_light` |
| real install | — | `app_client_drlg` `the_users_client_missiles_run_their_functions` runs every user row of functions 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 23, 25, 43, 49, 59, 60, 63, 65 for 60 updates with the user's skills tables and a drawn frame's origin; every function has a row made. | install: 4 passed |

## Not done

- Functions with one or two live rows not yet run (§B4 17, 27, 46, 52;
  §B11; §C13 18, 37, 38, 39, 66; the open bodies of §C12 wait on the
  spec).
- Unit hits (§C7 r12), the client hit functions (§B6–§B7, REC-452),
  the §9.4 cell walk (REC-451), missile sounds (audio).
- Screen shakes of functions 12, 29, 31, 36, 37, 38, 54, 66: the row
  `q-fix-shake-starts` (REC-62).
