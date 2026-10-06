# Handoff: mutation testing of the merged host's handlers — `claude/mutants-handlers`

> Not yet folded into `docs/HANDOFF.md` / `docs/PLAN.md` (neither is edited here); the coordinator folds it.

Cloud test session, 2026-10-06, medium effort. Base:
`claude/tender-meitner-mphas3` at `c5a6327` (the merged host). Repo
only, synthetic tables, no game files (M09). METHODS M08 on
`crates/d2-server/src/adapters/handlers/**` with `cargo-mutants` 27.1.0:
`cargo mutants -p d2-server --file <dir> --timeout 120 -j 2 --test-tool
nextest`, split by subdirectory (`skills/`, `items*`, `world*`).
`mutants.out/` is not committed. The run's test command is
`d2-server`'s own tests; `stats::prop_tests::stat_lists_match_the_model`
(`d2-sim`) is not in it, so it never interfered.

No production code changed. The tests are in new files only, so nothing
collides with `wire-inventory-server`:

- `crates/d2-server/tests/mutants_handlers_skills.rs` (16 tests)
- `crates/d2-server/tests/mutants_handlers_items.rs` (21 tests)
- `crates/d2-server/tests/mutants_handlers_world.rs` (8 tests)
- `crates/d2-server/tests/mutants_handlers_fx/mod.rs` (a shared fixture,
  not a test target: the wired host on synthetic tables, as
  `prop_handle.rs` builds it)

One existing test changed (§5): `prop_transport.rs::inbox_any` was
flaky. It is fixed with four lines that touch only its expectation.

Every test sends a message through `SimGame::handle` (the handler API,
after the dispatcher). Each assertion checks the outcome the rule gives:
result code, mode, mana, used / left / right skill, timers, states, the
cube's output item, gold, interaction, warp, or the S→C bytes. Where a
rule names a provider call (run to the target, consume charges, repair,
recharge, the quest hook), the assertion is the staged provider's log
of that call.

## 1. Counts

`prop_transport::inbox_any` failed on some random inputs whatever the
mutation (§5), so some mutants counted as "caught" were caught only by
that flake: 5 before, 3 after. The corrected rows move those to
"missed".

| Run | Mutants | Caught | Missed | Timeout | Unviable |
|---|---|---|---|---|---|
| before, raw | 574 | 197 | 294 | 0 | 83 |
| before, corrected | 574 | 192 | 299 | 0 | 83 |
| after, raw | 574 | 366 | 125 | 0 | 83 |
| after, corrected | 574 | 363 | 128 | 0 | 83 |

| Split | Mutants (unviable) | Before missed (corrected) | After missed (corrected) |
|---|---|---|---|
| `skills/` | 245 (20) | 149 (`world.rs` 148, `wired.rs` 1) | 62 |
| `items*` | 210 (17) | 116 (all `items/cube_world.rs`) | 53 |
| `world*` | 119 (46) | 34 | 13 |

The "after" rows are the last run of each split: `skills/` and `items*`
after the last test was added; `world*` from the run before that (its
tests did not change afterwards).

Killed: 171 of 299. Of the 128 left, 85 are unobservable or equivalent
(§3) and 43 are spec-decided and left for later (§4).

The pre-merge baseline (`mutants-server.md`) had 338 handler survivors
in another file layout, so it cannot be compared row by row.

## 2. (a) Killed, by the spec rule that decides them

| Mutants | Test | Spec |
|---|---|---|
| `World::{has_player_data, last_point_frame, set_last_point_frame}` (`→` true / 0 / 1 / −1 / `()`, the `u == player` compare) | `point_validator_in_the_handler`: no player data → 2; far → 1, resync only after > 25 frames (974 vs 975); in range → +0x168 = frame | `use.md` §1 r1 |
| `World::{in_own_inventory, same_act, within_reach}` (`→` true / false, `==`, `&&`, the first `\|\|`) | `unit_validator_in_the_handler`: an owned item passes even when far; another act → 2; 51 subtiles → 1 | `use.md` §1 r2 |
| `left_skill → Some(default)` | `left_message_without_left_skill` | `use.md` §1 r3 |
| `can_dual_wield`, `equippable`, `item_at`, `item_is`, `attack_param4`, `set_attack_param4` | `dual_wield_alternates_hands`: LHS (Param4 5), Attack (0), LHS; each missing condition → Attack | `use.md` §2 r2 |
| `in_melee_range`, `run_to`, `bow_equipped`, `state_mask` | `run_or_use_by_range`: h2h in / out of range, both with and without a bow, rng with mask 0x26 | `use.md` §3 r6 |
| `owner`, `has_state` | `attached_target_is_its_owner` (state 143) | `use.md` §3 r4 |
| `cursor_item`, `mode`, `endanim_expire`, `class_id` | `can_change_mode_gates`: cursor; A1 at E + 5 / E + 6; S1 for an Amazon and a Paladin | `use.md` §4 |
| `seed`, `state_stat` | `interrupt_gate_draws_and_blocks`: one `roll(100)` on the unit seed; stat 164 = 100 blocks, = 1 does not; state 15 blocks | `use.md` §4 |
| `start_mode` flag clear (`\|=`, `!`) | `mode_start_clears_flag_0x40` | `use.md` §4 |
| `set_mode` | the neutral reset after a failed start (`start_mana`, `start_line_of_sight`) | `use.md` §5.3 r7 |
| `target_position`, `line_clear` | `start_line_of_sight`: mask 0x804, position, blocked | `use.md` §5.3 r6 |
| `shapeshifted`, `consume_charges`, `pay_life`, `has_state`, `stat` | `start_mana`: `srvdofunc` 116 free when shapeshifted; shrine (+2 levels → 3,840); blood mana pays life; an item skill pays charges | `use.md` §5.3 r6, `levels.md` §4 |
| `is_alive`, `dec_quantity`, `formula_stat`, `mode`, `event_arg`, `has_state_list` (`→ true`), `create_delay_list`, `set_state_list_expiry`, `schedule`, `delete_timers` | `immediate_aura_do_and_cooldown`, `sequence_mode_delay_needs_event_arg_0`, `dead_unit_do_charges_nothing`: the do once at 0x3C, mana at do, `decquant`, delay = strength (formula CALL 5) → state 121 list, expiry, type-12 timers; one type-8 timer | `use.md` §5.4 r9, §6, §7 |
| `free_aura_state`, `set_aura_state` | `replacing_an_aura` | `use.md` §7 |
| `wired.rs` 0x3A guard `==` → `!=` | `no_vitals_stubs_only_add_stat_point` | `intents-events.md` §2.4 r5 |
| `expansion`, `game_type`, `ladder`, `difficulty`, `player_class` | `version_needs_expansion`, `ladder_record_needs_ladder_or_game_type`, `min_difficulty`, `class_record` | `cube.md` §4 |
| `local_date`, `stat` | `date_ops` (ops 1, 2), `stat_op` (op 3) | `cube.md` §5 |
| `class_is_type`, `item_quality`, `item_file_index`, `item_sockets`, `item_flags` | `input_item_tests` (§6.2 tests 1–5), `file_index_op` (op 27) | `cube.md` §6.2, §5 |
| `item_level`, `set_item_level` | `captured_level` | `cube.md` §6.4, §7.1 r2 |
| `game_seed`, `item_format` | `type_pick_draws_on_the_game_seed`: same output and game seed as an item-code run on a seed advanced by `roll(N)`, `roll(1)`; a version-100 amulet needs format ≥ 100 | `cube.md` §7.5 |
| `duplicate`, `set_item_class`, `item_init`, `set_item_mode`, `tempered_affix`, `set_tempered` | `mod_copy_takes_the_output_class`, `useitem_tempered` | `cube.md` §7.3 |
| `drop_runeword_stats`, `socketed`, `repair`, `recharge`, `set_stat`, `max_sockets`, `add_sockets` | `rem_keeps_the_socketed_items`, `rep_and_rch`, `output_quantity`, `sock_output` | `cube.md` §7.6 |
| `free_item`, `quest_item_hook`, `cow_portal` | `placement_outcomes`, `cow_portal_decides` | `cube.md` §8, §7.2 |
| `put_item_check` guard `m > 4`, `trading` (`→ true`, `&&`) | `put_item_mode_above_cursor`, `put_while_trading` | `cube.md` §2 r1, r3 |
| `InfoRest::player_info` (all) | `ear_gets_the_players_name`: name and NAMED = hardcore | `generation.md` §9 r5 |
| `WiredWorld::waypoints`, every `HostWaypoints` call 0x49 makes (`difficulty`, `records`, `object`, `reset_interact`, `interact_guid`, `hostile_delay`, `attach_sound`, `send`, `warp`, `spawn_room`, `set_player_mode_arrival`) | `wired_travel`, `wired_close`, `wired_refusal_closes_only_its_own_menu`, `wired_record_of_the_game_difficulty`, `wired_hostile_delay`, `wired_missing_and_other_act` | `waypoints.md` §6.1–§7 |
| `WiredWorld::fault`, `ActionWorld::fault` | `unqueued_message_is_a_recorded_fault` (a refusing sink) | `waypoints.md` §7 r7, the handlers' fault path |
| `WiredWorld::vendors`, `record_of` (`→` scratch, `==`) | `sale_uses_the_npcs_own_record_and_client`: with Gheed's record beside Akara's, the cap is Akara's permanent code (no copy) | `vendors.md` §7.2 r8, r10 |

## 3. (b) Not killed: unobservable or equivalent, with the reason

- **`InfoRest` (37, `items/cube_world.rs` 51–117).** Every method except
  `player_info` is `unreachable!()`: `ServerCube` answers those calls
  before they reach the economy. A replaced body is never run.
- **Cube `open` (9): `unit_class` (3), `interacting_with_stash` (5),
  `set_interaction` (1), and `RestInteract::set_interact` (world).**
  Only `CubeData::open` calls them (the item-use path, `cube.md` §1 row
  1). No handler here routes item use, so they are unreachable from
  0x2A / 0x4F.
- **`item_guid` (2).** `CubeWorld::item_guid` has no caller in
  `d2_sim::world::cube`.
- **`World::last_point_frame` match guard (3) and `same_act` `==`
  (265:52, 268:67), `&&` (269:22).** These are equivalent on every
  staging the server makes. `point_accept` is `Some` only after the
  validator succeeds, and it is not read again in that message. The
  other-act target is never the player. The staged positions hold
  either both units or neither.
- **`World::target`, `clear_target`, `unit_flags`, `is_hostile`,
  `is_pet`, `is_ally` (10).** `start`'s target checks (`use.md` §5.3
  steps 2 and 4) run after `World::start_mode`, which clears the target
  (§4 last paragraph). Where `0x0057FE90` / `0x0057FEF0` store the new
  target is not stated (TODO in `start_mode`), so on the message path
  the target is always `None`. These become reachable once the spec
  says where the target is stored (open question for the `use.md`
  owner).
- **Tick-only seams (7): `owns_skill`, `set_used_skill_flags`,
  `set_event_arg`, `step_path`; and `used_skill_flags` (2).** `World`
  serves messages only. The type-0 frame event and the type-8 handler
  run on `UseView` in the tick, not on `World`. `used_skill_flags` is
  read on the message path only in do-core step 1, which needs a room
  that is none or a town (the fixture's room is a field room).
- **`HostWaypoints::{frame, set_object_mode, schedule_endanim,
  player_busy, set_interact}` (9).** Only object init 17 and operate 23
  call them (`waypoints.md` §5.1–§5.2). C→S 0x13 with unit type 2 is a
  stub (no object-interaction spec), so no handler reaches them.
- **`WorldHost::take_sent` default `→ vec![]` (1).** Every host that sends
  overrides it. `NoWorld` and test hosts send nothing.
- **`has_state_list → false` (1).** A second delay list for state 121 is
  created where there should be one. This is observable only in
  stat-list internals that `state_list` hides (it returns one list). Not
  asserted.

## 4. Not killed, spec-decided, left for a later session

- **Skill-formula reads (34):** `item_stat`, `stat_entries`,
  `current_weapon`, `weapon`, `itype_is`, `wield_type`, `item_damage`,
  `str_dex_bonus`, `item_flag_throw`, `missile_level`. These are reached
  only through skill formula specials (`levels.md` §3: physical and
  elemental damage, mastery, kick damage) in a start or do of the
  message path. Each needs a synthetic formula and item set.
- **`create_skill_missile`, `set_unit_flags` (2):** do-core step 7
  (`srvmissile`) needs missile tables on the action wiring.
- **`unique_found`, `set_unique_found`, `add_craft_property`,
  `item_seed` (5):** the `reg` unique restore (`cube.md` §7.4) and the
  craft mods (§7.6 r3) need unique and property tables.
- **`ActionEvents for WorldSim::create_game` (1):** it has no caller in
  any test (the d2-client e2e builds `WorldSim` without it). It needs a
  `WorldSim` fixture in `d2-server`.
- **`WiredWorld::skill → None` (1):** `d2-client`'s `e2e_single_player`
  (outside this `-p d2-server` run) uses this slot. A `d2-server` test
  needs a `SkillRest` provider on the wired host's `Pending`.

## 5. (c) Code against spec

- **Handler code: none found.** Every new test passes on the current
  code, and the outcome each one pins is the one its rule states. One
  gap is recorded above (§3: the start's target checks). It is an open
  question in `use.md` §4, not a deviation.
- **Test code: fixed.** `crates/d2-server/tests/prop_transport.rs`
  `inbox_any` accepted only `BadId` from `Inbox::deliver`. A random
  buffer can hold a message whose S→C size rule gives more than 0x204
  bytes. `intents-events.md` §3.3 rule 2 makes that a fatal assert,
  which d2rs returns as `BadSize`, and the test's own doc says "a bad id
  or size is an error". The fix adds one arm,
  `Err(QueueError::BadSize(n)) => prop_assert!(n > MAX_MESSAGE)`.
  - Reproduced: without it, `PROPTEST_CASES=20000` fails
    (`BadSize(517)`); with it, the same run passes.
  - In the mutants runs it was the only failing test for 8 mutants
    across the before / after / final runs. These are the flake-only
    kills of §1. They are in `skills/world.rs` and `items/cube_world.rs`,
    and no mutant there can reach transport code.
  - It is the same kind of flake as the known
    `stats::prop_tests::stat_lists_match_the_model`, but in `d2-server`,
    so it does interfere with `cargo mutants -p d2-server`.

## 6. Gate (this branch)

`sh tools/gate.sh all`: GATE PASS. Every step passed: spec_index,
methods, coverage `--check` / `--selftest` (0 errors), trace checkers,
hook selftest, fmt, depcheck, clippy workspace, tests for `d2-sim` +
conformance, the rest, and `d2-client`, and the doc-tests.
