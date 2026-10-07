# impl-spec-answers: pathing.md and path-placement.md

Scope: the answers added to `specs/sim/pathing.md` and
`specs/sim/path-placement.md` between `b435f5a` and HEAD. Code in
`crates/d2-sim/src/path/**`, `crates/d2-sim/src/wiring/path/**` and the
path property tests in `crates/d2-sim/tests/`.

## Answers implemented

| answer id | code change | test |
|---|---|---|
| PQ2 (§8.1 r4) | `velocity::mode_velocity` doc: `None` = a mode without the modifier keeps its velocity (behaviour was already that); TODO removed | `answers::mode_without_modifier_keeps_the_velocity` |
| PQ3 (§8.1 r3) | `set_velocity` doc (already matched) | same test |
| PQ4 (§9.5 r4) | already implemented (the arrival returns the re-path result) | `answers::arrival_result_is_the_repath_result_and_types_are_written_directly` |
| PQ5 (§9.2 step 2, step 3) | `step.rs::player_event0_on`: `state13_step` no longer returns, the step goes on; run exhaustion restarts with mode 2 (was 3) | `answers::state_13_step_goes_on_with_the_movement`, `answers::exhausted_run_restarts_as_walk` |
| PQ6 (§1.4 r5) | `request.rs::interrupt_check`: a failed concentration roll falls through to the state-15 test | `answers::failed_concentration_roll_falls_through_to_state_15` |
| PQ7, OQ8 (§9.10) | `DynamicPath::repath_budget` (+0x94) with `set_repath_budget` (`0x006490E0`, > 255 fatal → `PathError::RepathBudget`) and `add_repath_budget` (`0x00649140`, clamp 0..255); `Walk::repath`: only monsters test the budget, the budget (not +0x90) loses the index, types written to +0x3C directly, town access 0. Seam `WalkUnits::repath_budget` removed | `answers::arrival_result…`, `answers::repath_budget_setter_and_clamp`, `tests::repath_without_budget_stops` (rewritten) |
| PQ8 (§9.4 r2.4) | already passed no hint (comment) | M1 / M2 vectors |
| PQ9, edge case 12, W10 | no code change: `cell_walk` already overshoots; W10 reproduced exactly | `answers::w10_cell_walk_overshoot` |
| GR1 (§9.6 r2) | comment: dead in 1.14d | — (dead rule, not claimed) |
| §9.4 r2.5 | aim on "path type ≠ 4" instead of "not a missile" | `answers::path_type_4_does_not_aim_at_the_next_point` |
| §9.2 step 5, path-placement §10 r7 (PP7 history part, OQ4) | new `path::history::PositionHistory` (20-entry ring, placement write unconditional, walk write when d² > 45; 25 ms gate read as open); `WalkUnits::position_history` seam; `PlaceHost::history_write` called by `place_unit` rule 7; wired into `PathState::history` (freed with the path record) | `history::tests::ring_writes_and_the_walk_distance_gate`, `answers::walk_step_writes_the_position_history` |
| OQ4 (§3 target lead) | lead = 0: `WalkUnits::target_lead` seam removed; `refresh_point` returns the position | `answers::target_lead_adds_nothing` |
| OQ5 (§8.5) | none (server never turns; no server code calls the turn) | — |
| §1.6 C→S 0x5F | new `path::walk::resync` (`handle_resync`, `ResyncRing`, `resync_distance`): ignore / walk / snap branches, reachability compute with saved and restored fields, lock 125 or table `0x006E1064` via `lo' % 100`, S→C 0x15 fallback. New `WalkUnits` seams with defaults: `has_client`, `is_dead`, `place_resync`, `resync_ring`, `game_type`, `resync_lock`, `send_to_client` | `answers::resync_ignore_and_walk_branches`, `answers::resync_snap_branch` |
| §11 missile paths (OQ3 missile part) | new `path::walk::missile` (type 4 straight, 10 charged bolt with unit-seed draws, 14 blessed hammer), `path::walk::sine` (the 512 f32 sine table as bit patterns, generated from `f32(sin(i · f32(2π/512)))`, sha256 equal to the spec's; exact integer `trunc_mul` for the 53-bit product); `compute` dispatches flag 0x40000 there | `answers::straight_missile_path`, `answers::charged_bolt_path_draws_on_the_unit_seed`, `answers::blessed_hammer_spiral`, `sine::tests::*` |
| PC3 (§2.4 r4) | missile allocation sets type 4 through `set_path_type` (flags 0x60000) | `tests::missile_allocation_sets_type_4_through_set_type` |
| PC1, OQ2 (§4 r4) | `walk_box`: an empty box gives "no boxes" → 0x27; strip shapes confirmed (TODO removed) | `tests::box_split_empty_box_and_corner_strip` |
| PC2 (§5.1) | `apply_cells`: every cell looked up from the room argument; pattern 0 stamps / clears nothing | `tests::stamp_cells_are_looked_up_from_the_room_argument`, `tests::pattern_and_size_stamps` (updated) |
| PC4, WP4 (§6 r2, r4) | `forced_move_rooms(room1, old, room2, new)` (`forced_move` keeps the one-room form); teleport: player stamps from the destination room; missile clears first, then queries and stamps from the destination room; zero point clears the collided mask | `tests::teleport_uses_the_destination_room`, `wiring::path::tests::waypoint_warp_places_the_player_in_the_spawn_room` (updated) |
| PC5 (§5.2) | doc on `ObjectShape::collides_in` (modes > 7 do not exist in 1.14d) | `tests::object_modes_above_7_do_not_collide` |
| W5 (§3 size shapes) | core already right; test added | `tests::size_query_reads_the_shape_of_the_size` |
| W6 (§4 r6) | new `collision::unit_at_point` + `UnitsAtPoint` trait + `shapes_overlap` table | `tests::unit_at_point_search_order_and_hit_table` |
| PP1 (§7.3 r3) | comment (code already used the candidate's room) | existing vectors |
| PP2, PP3 (§8 r3) | coarse search n + 2 ≤ 1 reads `point_query(…, mask)` (masked); half-open already | `prop_path_place` reference model updated |
| PP4 (§11) | `game_entry` / `level_warp_place` take the unit's size (`size` parameter removed); game entry without spawn room → `PlaceError::NoSpawnRoom`; level warp → `Ok(false)` | `place::tests::game_entry_and_level_warp`, `prop_path_place::spawn_entry_warp_draws_and_points` |
| PP4 flag (§11 `0x00554850(flag 0)`) | wiring `add_player_to_world` sets path flag 0x2 | — (wiring) |
| PP5 (§11 Recipients) | `PlaceHost::send` doc: the player's own client (behaviour unchanged) | existing |
| PP6, PF1 (§12.2) | comments (already rule 2's point, wrapping) | existing |
| OQ3 / PP7 (§12.2 r1, `0x006195A0`) | `LevelView::warp_destination` doc states the rule; no provider | — |

`python3 tools/coverage.py`: pathing.md 115 → 139 of 146 rules, path-placement.md 80 → 82 of 85.

## Test expectations changed (and why the spec says so)

- `path::tests::pattern_and_size_stamps`: pattern 0 now stamps nothing (§5.1 "Pattern 0 stamps and clears nothing").
- `wiring::path::tests::waypoint_warp_places_the_player_in_the_spawn_room`: the destination cell now carries the player bit 0x80 (§6 rule 4: forced move's room2 = the destination room; the test's comment said a spec answer would change this line).
- `path::walk::tests::repath_without_budget_stops`: rewritten for §9.10 (monster-only budget at +0x94; +0x90 untouched; players re-path with budget 0).
- `path::walk::tests::gaps` (two closures): the budget is set on the path (`p.repath_budget = 5`) instead of the removed seam.
- `tests/prop_walk_motion.rs::chase_a_moving_target` and `tests/walk_rooms_fake`: budget seam removed; the budget lives on the monster's path and depletes by the index; players never stop on budget; the distance budget is no longer reduced by a re-path (§9.10).
- `tests/prop_path_core.rs` model and `tests/walk_rooms_fake::World::stamp`: pattern 0 stamps nothing (§5.1).
- `tests/prop_path_footprint_ops.rs::teleport_follows_the_spec` model: missile clear before the query, query and stamp from the destination room, zero point clears the collided mask (§6 rule 4).
- `tests/prop_path_place.rs`: `game_entry` / `level_warp_place` signature; game entry without spawn room is `Err(NoSpawnRoom)` (§11); coarse-search reference reads the masked value (§8 rule 3).
- `tests/prop_wired_path.rs`: `regress_second_warp_lands_on_an_unstamped_player` → `regress_second_warp_sees_the_first_player` (the first warped player is stamped, so the second lands elsewhere; the test said a spec answer would change it); `regress_a_warp_streams_its_spawn_room_back_in` allows the player's own footprint; the model no longer marks warped players unstamped and pattern 0 has no stamped cells.

## Open / not done

- d2-client `tests/e2e_full_loop.rs` step 5 (not edited): it asserts
  `Walk(Fatal("path type without a function"))` for the missile build and
  "no hit, no kill" after it. With PC3 + §11 the missile now gets a type-4
  straight path, so that expectation (and possibly the kill / drop /
  experience flow after it) changes. It could not be checked: the test
  currently stops earlier at `e2e_full_loop.rs:1355`
  (`Dispatch::from_spec()` → `Mismatch([NoHandler …])`), which is not caused
  by this work (it fails before any path code runs). Re-run and update step
  5 once that's fixed.

- §1.6 0x5F: library only. Not wired: no C→S 0x5F dispatch (intents / d2-server), and `PathCtx` keeps the seam defaults (`place_resync` false → always S→C 0x15 fallback is not even reached because the reachability compute runs path type 15, whose function is unspecified (`pathing.md` OQ3) and returns 0). Needs: type-15 function, the client ring storage, `0x005541B0`, the state-108 stat list with remove callback and event 12 (`stat-lists.md` §10.4).
- W6: `wiring/action/missiles.rs::units_at` still uses its own search (file outside this task); switch it to `path::collision::unit_at_point`.
- W5: `wiring/action/missiles.rs` and `wiring/worldgen/population.rs` keep the single-cell fallback when the path provider is off (outside this task).
- §12.2 r1 `0x006195A0`: no `LevelView::warp_destination` provider (needs the DRLG warp-link lists).
- GX4: the seeds are in the spec; no live §11 spawn-point test (needs a live `LevelView` provider); `game_inventory_path.rs`'s module doc still says the seeds are unstated (file shared with the inventory task, not edited).
- OQ9 (x87 precision during §11.3): 53-bit implemented; a 24-bit reading would round the product first.
- §11.1 rule 4: whether flag 0x1 is cleared when the target is inside the room is not stated: `TODO(spec: pathing.md §11.1 rule 4)` in `missile.rs` (only set).
- Suggested local check: compare `path::walk::sine::SINE_BITS` with `Game.exe` at `0x00707800` (2048 bytes).
- Unrelated failures seen while testing (not caused here): `d2-sim --lib` items / missiles catalogue / monsters AI catalogue / skills table tests and `d2-server` items handler tests (another agent's in-progress inventory work and spec-status tables).

## Test commands (results)

- `cargo test -p d2-sim --lib -q -- path:: wiring::path`: 131 passed, 0 failed, 1 ignored.
- `PROPTEST_CASES=256 cargo test -p d2-sim --test prop_path_core / prop_path_footprint_ops / prop_path_place / prop_walk / prop_walk_motion (1000 cases) / prop_walk_rooms`: all ok; `prop_wired_path` (64 cases): 8 passed.
- `cargo test -p d2-server --no-fail-fast`: every path / walk / wired-host target ok; only the 3 items-moves lib tests fail (another agent's work).
- `cargo clippy -p d2-sim --profile test --lib --test prop_path_* --test prop_walk* --test prop_wired_path -- -D warnings`: clean. `--all-targets` stops on `tests/mutants_wiring_inventory.rs` (inventory work in progress).
- `cargo fmt --all`: run; `--check` shows only `items/moves/tests/answers.rs` (not mine).
- `python3 tools/coverage.py --check`: 4521 claims, 0 errors.
