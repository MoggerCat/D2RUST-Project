# merge-c-sim: impl-c-sim, impl-c-client and staging-7 into the staging head

Branch `claude/merge-c-sim` (staging head e7ae40f0). Two tasks:
merge `origin/claude/impl-c-sim` (b020f06e, `--no-ff`), then fix the six
waypoint-fixture tests that were already red on staging.

## Task 1: conflict resolution (hunk by hunk)

| File | Hunk | Resolution |
|---|---|---|
| `d2-server/src/adapters/character.rs` | HEAD `refusal_message` / impl `pub mod save;` | both kept |
| `d2-sim/src/items/mod.rs` | `recharge` / `set_state` | both modules kept |
| `d2-sim/src/units/mod.rs` | `replenish` / `sound` | both modules kept |
| `d2-sim/src/wiring/action/mod.rs` | `hirelings` / `inactive` | both modules kept |
| `d2-sim/src/wiring/action/pending.rs` | HEAD dropped `object_stamp_footprint` / `object_free_footprint` (bf38a7d4 implements them); impl dropped `object_sound` (no implementor: the sound slot `units::sound` replaces it) | both removals kept: neither seam has a caller on the merged tree |
| `d2-sim/src/path/line.rs` (add/add) | both implemented the line test `0x0064E260`; impl also the collision line `0x00622AA0` | see below |
| `d2-sim/src/wiring/economy/quest_host.rs` (2 hunks) | both implemented the quest drop `0x00559A30` | see below |

### Line test `0x0064E260` (path/line.rs)

Both specs describe the same function: `sim/pathing.md` §13.3 and
`render/draw-order-2.md` §16. Kept **HEAD's** `line_test` (returns
`LineTest::{Clear, Blocked(Point)}`): it follows §13.3, which is the
more precise of the two (rule 1 writes `to := from`; rule 4 blocks when
the found room does not contain the cell, which impl's version did not
check). impl-c-sim's `LineUnit`, `pull`, `SIZE_CAP` and
`units_line_blocked` (`0x00622AA0`, §15.1) now call it
(`.blocked()`). The spec header names both specs.

Callers moved to the kept API: `wiring/interaction/skill_use.rs`
`line_blocked` (`Point` arguments, `.blocked()`). `wiring/action/missiles.rs`
already used it.

Tests: HEAD's inline tests renamed `mod tests_pathing` (unchanged);
impl's `path/line/tests.rs` ported to the kept API through a local
`lt(...)` wrapper on tuples. Changed expectations:

- `line_crosses_rooms_and_reports_the_stop_cell`: a null room and a start
  in no room were `Err(None)` (no stop cell); now `Blocked(from)`
  ((1, 1) and (30, 1)). `pathing.md` §13.3 rule 1 states `to := from`;
  `draw-order-2.md` §16 rule 1 only says "blocked", so the test was
  corrected to §13.3.
- Every other assertion is the same value in the new type
  (`Ok(())` → `LineTest::Clear`, `Err(Some((x, y)))` → `Blocked((x, y))`).

### Quest drop `0x00559A30` (quest_host.rs and the drop pickers)

Both sides implemented the function: HEAD as
`wiring::economy::drop_helpers::source_drop` on the pure pickers
`treasure::class_pick` (`world/objects-2.md` §20.4–§20.6, also used by
the object drops), impl-c-sim as `wiring::economy::unit_quest_drop` on
the pure `treasure::quest_drop` with its own sub-pickers
(`items/treasure.md` §9, §9.1). The two specs describe the same function.

One implementation kept:

- **Pickers**: `treasure::class_pick` (`part_pick`, `random_class`,
  `source_class`). `treasure::quest_drop`'s `sub_pick`, `class_pick`
  and `quest_class` are now thin wrappers over it
  (`PickData::picks()` → `ClassPicks::of_items`). Its own constants alias
  `class_pick`'s. `ClassPicks` gained `parts` (the header's (start,
  count) per part, `None` slot = absent part) and `of_items`;
  `PickRow: From<&ItemRec>`.
- **Wiring**: `drop_helpers::source_drop`. `unit_quest_drop` keeps its
  signature (umod callback, quest host, tests) and calls `source_drop`
  with the action tables' `levels`. When the drop state has no pick rows,
  it picks from its item tables (the same combined array).
  `HostQuests::econ_quest_drop` (impl's: an unknown unit keeps the rest's
  answer) now runs `unit_quest_drop` through HEAD's `with_drop_state`.
  Both conflict hunks take impl's call through `econ_quest_drop`.
- d2rs guard: impl's 2²⁰-pick stop for the unbounded quality-4 loop moved
  into `class_pick::source_class` (`MAGIC_LOOP_GUARD`, new
  `PickError::Hang` → `TreasureError::QuestDropHang`). 1.14d hangs there
  (objects-2 edge case 4), so this changes nothing a 1.14d run can show.
- New `TreasureError::NoGold`: a missing `gld ` row is fatal 0x17C (or 0x17F
  for index 0) before the roll (`objects-2.md` §20.6). impl's picker
  returned −1 there. `treasure.md` §9 says nothing on it, so §20.6 decides.

Changed test expectations (the tests contradicted §20.6, or the fatal's
sink moved):

- `treasure::quest_drop::tests::magic_loop_retries_eleven_class_picks_then_weapons`:
  the fixture had no `gld ` row. A non-magic `gld ` row was added at
  index 1, which the gold band now picks and retries like −1. The
  expected result (`QuestDropHang`, 12 steps; `Ok(-1)`, 1 step) is
  unchanged.
- `treasure::quest_drop::tests::magic_loop_keeps_first_magic_class`: same
  fixture change (gold row at index 1); expectations unchanged.
- `treasure::quest_drop::tests::quest_drop_minus_one_class_creates_nothing`:
  same fixture change; expectations unchanged.
- `wiring::action::tests::death::the_quest_drop_with_an_unknown_code_is_fatal`:
  the fatal 0x9EA was `d.errors == [TreasureError::DropCode("zzz ")]`. The
  single implementation records pick fatals in `d.pick_errors`, so the
  test now expects `d.pick_errors == [PickError::Code("zzz ")]` and
  `d.errors` empty. Same fatal, different sink.
- `wiring::action::tests::objects::picks_fx` and `class_pick::tests::picks`:
  `ClassPicks` literals gain `parts: None` (construction only).

Follow-up, not done here: `treasure::quest_drop::quest_drop` (the pure
§9 rules 2–5 driver over a `DropSink`) has no production caller now
(the wiring is `source_drop`). It is kept with its tests as the pure model
of §9. A later task can fold its tests onto `source_drop` and remove it.

### impl-c-sim PROVISIONAL points → REC ids (HANDOFF §7, Priority 2)

- REC-91: `formats/d2s-appearance.md` §1 r2 / OQ3 (reference table
  `0x00744CA8`), with local run queue item 99.
- REC-92: `formats/d2s-appearance.md` §2 r1 / OQ4 (empty `alternategfx`).
- REC-93: `audio/triggers-2.md` §14 r2 (chest at-once key sound sent twice).

The PROVISIONAL comments in `d2s/appearance.rs`, `wiring/action/objects.rs`,
the two specs and `docs/handoff/impl-c-sim.md` now cite them.

## Task 2: the six waypoint-fixture tests

Cause: 4eb119b0 implemented `waypoints.md` §7 r2 / edge case 6 (travel
to level 0 or to the waypoint object's own level only closes the menu).
The fixtures travelled from a Cold Plains waypoint to Cold Plains. The
d2-sim travel code is unchanged; the fixtures now travel from a waypoint
in another level of the same act (the act check of §6.2 still passes).

| Test | Fixture change | Expectations |
|---|---|---|
| d2-server `world::tests::waypoints::sorceress_at_22_travels_with_the_arrival_message` | the fixture gains level 4 (act 0, its own room) with a third waypoint at (20, 20) (`Fx::side_wp`), spawned after the player so the other GUIDs are unchanged; the test stages and sends that waypoint | unchanged (warp log, 0x0D at 45, 23, arrival node) |
| d2-server `walk::tests::a_warp_within_the_level_sends_0x15_in_the_next_update_pass` | new `travel_from(fx, wp_room, level)`; the waypoint object sits in room C (the town, act 0) and the player travels to Cold Plains (`travel` = `travel_from(fx, A, …)` for the other tests) | unchanged (0x15 reveal, placement in A, 0x15 on tick 1) |
| d2-server `tests/mutants_handlers_world.rs` `wired_travel` | `world()` gains level 4 (waypoint index 2) and a waypoint in it (`Wps { wp, far_wp, side_wp }`, spawned after the player) | unchanged |
| d2-server `tests/mutants_handlers_world.rs` `unqueued_message_is_a_recorded_fault` | both halves send from `side_wp` | unchanged (Malformed, one `Sink` fault) |
| d2-client `bridge::local_tests::waypoint_travel_end_to_end`, `spec_table_drops_a_unit_message_for_an_unknown_unit` | see below | see below |

d2-client `bridge/local_tests.rs`: the shared `game()` fixture gains the
Rogue Encampment (level 1, act 0, waypoint index 0) with its own room;
the waypoint object moves there (still at (20, 20), still allocated
before the player). The player stays a sorceress at (42, 20) in Cold
Plains, so the act check and the 22-sub-tile reach are unchanged and the
player is in Cold Plains' spawn room for the arrival. Both tests' expectations are unchanged:
`waypoint_travel_end_to_end` (0x49 bytes, `Done`, 0x0D at (45, 23), warp
3 0, arrival mode, menu closed) and
`spec_table_drops_a_unit_message_for_an_unknown_unit` (counters, dropped
`{0x0D: 1}`).

Also changed by the merge (not one of the six): `tests/e2e_single_player.rs`
step 25. impl-c-sim's sound slot now puts the transmute's sound 4 on the
wire, as its notes predicted. The test now expects S→C 0x2C (unit type 0,
the player's GUID, sound 4) after the update pass's 0x47/0x48, in the
player's update (`cube.md` §8 "Exact" item 3 / OQ2). The handled count
goes from 53 to 54. This fixes all three e2e tests
(`single_player_end_to_end`, `same_seed_same_run`, `other_seed_other_run`).

## Task 3: merge claude/impl-c-client (c2d3b39e)

The conflicts were in `d2-client/src/bridge/skills.rs` (3 hunks) and `bridge/msg/skills.rs`
(2 hunks). Both sides were combined against `client/msg-skills.md` §2:

- `skills::remove(list, rows, skill, d)`: staging's `d` flag, working
  copy and dangling refusal (rule 6) are kept. impl-c-client's passive
  `SkillFx::StateOff` is pushed into the working copy, so a refused
  remove leaves the list and its fx unchanged. The final refresh is
  impl-c-client's `refresh(list, rows, skill)`, which pushes
  `SkillFx::Refresh`. The no-native-entry path refreshes the same way.
- `assign` with the remove flag: staging's `remove(list, rows, skill, true)`
  (it refreshes).
- `add_for_bonus` / `bonus_write` (staging) call the new `refresh(list, …)`.
- The 0x?? bonus handler (`msg/skills.rs`) keeps staging's `unit_class` /
  `Owner` and `add_bonus`, with its error converted to `HandlerError`.
  Its remove is `remove(list, rows, s, true)` followed by
  `passive::apply(w, inputs, key, fx)` on the drained fx
  (impl-c-client, §2 r4).

Changed test expectations:

- `skills::tests::remove_with_d_decrements_and_frees_below_one`: the
  passive case expected `Err(SkillError::PassiveState { skill: 7, state: 3 })`.
  That variant no longer exists on impl-c-client, where the state is owed
  to the unit as fx. The test now expects `Ok`, base 2, and
  `fx == [StateOff(3), Refresh(7)]`.
- `SkillList` literals in the staging tests gain `fx: Vec::new()`.

## Staging merge

`origin/claude/specs-staging-7` (d17f8aac, with cov-world-objects) was
merged with `--no-ff` without conflicts.

## Gate

On the final head:
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `python3 tools/coverage.py --check`: 12496 claims, 0 errors.
- `python3 tools/spec_index.py --check`: clean.
- `cargo nextest run --workspace --no-fail-fast`: 7262 run, 7261 passed,
  1 failed, 237 skipped.

The one failure is **`scenario-run::scenarios server_messages_are_captured_at_their_tick`**.
It was already red on staging (e7ae40f0 carries both the test and 4eb119b0)
and was not in the list of six, but it has the same cause. Its
`travel()` rewrites the waypoint-travel scenario to travel town → town
(level 1), which §7 r2 now makes close-only, so nothing is sent at tick 50.
The fixture cannot express a fix cleanly:
the shared synthetic install (`test-fixtures/src/content.rs`) has exactly
one waypoint level per act (town 1 in act 0, town 5 in act 1). Two attempts
were tried and reverted:
1. Cross-act travel to level 5: still no reply at tick 50; the scenario
   host does not complete an act-change warp.
2. Giving the field (level 2) waypoint index 2 and travelling there: still
   no reply at tick 50, and extra messages appeared at tick 32. Making it
   work needs a waypoint object in the field's preset and the host's warp
   into a non-town level. That widens the shared synthetic data many
   suites use.
Proposed follow-up (its own task): add an act-0 waypoint level with a
waypoint object to the synthetic content, then point `travel()` at it.

