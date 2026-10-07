# impl-pc1-s5: code for PC 1 session 5's specs (2026-10-08)

Branch `claude/impl-pc1-s5`, from `claude/specs-staging-6` @ `ea3a6d1`.
Input: `docs/handoff/pc1-specs-s5.md` "For the code". Gate result: see
the last section.

## What landed

1. **`bytesN` layout field** (`d2-proto` TSV parser, codegen, wire):
   `FieldType::Bytes(n)`; structs with an array over 32 bytes get a
   generated `Default`. `intents-events.md` §2.4 r10 names the type.
   S→C 0x28 (`flags:bytes96@7`), 0x29 (`record:bytes96@1`), 0x52
   (`status:bytes41@1`), 0x5E (`available:bytes37@1`) filled from
   `client/msg-ui.md` §12–§14, §16; `generated.rs` regenerated.
2. **Bridge §10 table**: `Producer::{Message, Update}`; `parse_table`
   accepts `update`. Variants `JoinRefused`, `TownExit`, `StateFx`
   (payload incl. hook number and two hook values, `stat-lists.md` §3
   r6.7) added to `Output` / `ROWS`.
3. **0xB4 handler** (`client/model.md` §7 r8): `session::join_refused`,
   the r8.1 code map → one `JoinRefused` output; no model write.
4. **`linked_found`** (`drlg/rooms.md` §9.6 C1–C7): link chains hold a
   type-3 record's half after it and are searched head first; the corner
   hides P = R +0x20; C4 is `DrlgError::LinkedCornerNoNext`; C5's pair
   enters this room's chain; both flag re-runs make the door unit call
   for this room. Vectors C3, C4, C5 as tests.
5. **`SUNIT_Add` after the per-kind init** (`sim/units.md` §3.1
   r7.1–r7.5): `lifecycle::allocate_unlinked` + `lifecycle::add`;
   `UnitLists::{reserve_unit, link_unit, check_guid_free}`;
   `View::allocate` runs deferred object inits before the add;
   `ObjectWorld::add_object` links an object allocated inside an object
   call after `objects::allocate`'s init; the monster type init and the
   object init read `View::init_room` (the allocation's room). A hash
   lookup of the unit's own GUID during its init misses it.
6. **Re-path budget** (`ai.md` §7.5): already in `monster_path_setup`;
   added the GH test (`every_request_but_get_hit_refills_the_budget_and_retargets`).
7. **`missile.rs`**: no path room sets the room-exit flag (§11.1 r4).
8. **Code follows the tables**: 73 S→C ids now `Generated` in the audit
   and decoded by `s2c::parse` (handoff `s2c-builders.md` updated);
   `d2-server/tests/sim_builders_layouts.rs` checks the d2-sim builders
   against the generated layouts (all agree); `d2-client::rules::umod_hooks`
   is the client hook table `0x00724E28` checked against umods.tsv's
   `cl_phase0`–`cl_phase4`.
9. **Stale TODO markers** (see the commit message for the list and
   behaviour changes), **pathing.md §12** (`path::walk::other`: circling,
   types 3, 8, 9, 11, 12, IDA* 0/16, wall follow 15; the
   `other_path_function` seam is removed; this unblocks impl-path-motion
   MV8), **missiles.md §R6.3** (`missiles::srv_dmg`, the 14 functions;
   the `srv_dmg` seam is removed).

## Changed test expectations

- `bridge::output::tests::a_changed_row_is_reported`: the perturbation
  string followed an older `TradeAction` row (test was already broken).
- `bridge::tests::owned_rows_are_exactly_the_registered_handlers`: 0xB4
  owned → `0xB5 − 13`, model 13.
- `drlg::tests::mutant_tests::corner_merged_away_hides_its_half` →
  `corner_merged_away_hides_the_previous_half` (C3: the half is never
  hidden; the first corner in its chain is C4).
- `drlg::tests::tiles::wall_merge_to_corner`: chain length 7 → 14.
- `s2c::tests::recorded_unbuilt_messages_size_and_refuse` →
  `recorded_messages_of_the_layout_batch_parse` (0x15 / 0x51 / 0x27 now
  parse); `parse_rejects_bad_messages` uses 0x12 for `Unbuilt`.
- `path_functions_receive_the_path_info_record` →
  `ida_star_receives_the_slack_of_step_4_doubled`; the circling test
  checks the real walk; `resync_snap_branch` walls the target for its
  unreachable case; `knockback_keeps_the_distance_budget` expects
  `Neutral` when the first cell toward the target is blocked.
- `missiles::tests::damage_stage_fills_then_server_damage`: checks
  function 4's effect instead of the stub log.
- `start_line_of_sight`: (TN, 0) → (SC, 1); `akara_respec`: skills reset
  before stats.

## Readings to confirm (spec questions)

- §12.7 rule 6: the backtrack's parent advance is taken literally (no
  "tries < 4" guard, so a parent at tries 4 draws in random mode); a
  reused child keeps its own child slot.
- §12.8 rule 5: a follower with no points uses its position as "last
  point".
- §R6.3 functions 7 / 9 write hit class 0x60, but `0x005AD730`'s "hit
  class from `HitClass`" runs after and (as coded) overwrites it; and
  `0x005AD730` is coded to OR its result flags (else functions 3 / 14
  would have no effect). Both need a 1.14d read.
- `0x004638A0(class, 10)` / `(class, 11)`: no wiring provider yet
  (`is_small_monster` / `is_large_monster` default false).

## Remaining gaps

- `StateFx` / `TownExit` are defined but not emitted: `stat-lists.md` §3
  r6 (effect calls, setfunc / remfunc bodies and their model writes)
  and the town-exit delivery of `bridge.md` §10 r11 are not coded; the
  UI-request path of §10 r10 neither.
- `skills/bodies-4.md` edge case 1 (zigzag step ≤ 0 loops forever).
- The level-change calls of `tick.md` §6 step 5 (quest event 3, vendor
  `level_changed`) are specified but not wired (host / economy layer).
- `world::objects::tests::routes_match_function_table` and
  `route_check_catches_perturbations` fail on the base already
  (`object-functions.tsv` vs routes; impl-pc2-s4-world's area).

## Gate (`CARGO_INCREMENTAL=0 sh tools/gate.sh`, head of this branch)

All steps pass except four tests, none from this branch:
`world::objects::tests::routes_match_function_table` and
`route_check_catches_perturbations` (fail on `ea3a6d1` too, checked in a
clean worktree) and `e2e_full_loop::{full_single_player_loop,
other_seed_other_run}` (step 13 expects the buy refused at the item
copy, which now succeeds: the fixture impl-items-wiring is fixing).
spec_index and coverage failures of the first run are fixed.
