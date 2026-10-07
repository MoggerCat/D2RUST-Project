# Handoff: impl-pc2-fixes (2026-10-07)

Cloud implementation session. Base `claude/specs-staging-5` @ `666e2f2`;
branch `claude/impl-pc2-fixes`. Task: the code fixes of
`docs/handoff/local2-2026-10-06.md` §3 (PC 2's list), numbered here as
in the task prompt (1–7).

## Per item

1. **Warp letter** (`path-placement.md` §12.1 rule 1). Done.
   `crates/d2-sim/src/path/warp.rs` `EXIT_LEFT` 11 → 10 (exit 10 → 'l',
   11 → 'r').
2. **Targeting reset seam** (`inventory.md` §5.3). Done.
   `InvWorld::targeting_probe` now takes the targeted unit, and
   `items::inventory::targeting_reset` passes the inventory's owner, never
   the item. The wiring answers it as `0x0044BE50` itself: the unit's type
   (`Owner::ty`), 6 for a missing unit (`wiring/inventory/inv_world.rs`).
   The `InvRest::targeting_probe` seam (default 1, so no 0x3F was ever
   sent) is removed. New tests with a mercenary owner tell the two apart:
   `items::inventory::tests::targeting_reset_in_list_order` (core, fake)
   and `mutants_wiring_inventory::targeting_reset_clears_and_queues_0x3f`
   (wiring: player → one 0x3F per flagged item; merc inventory → flags
   cleared, nothing sent; probe values 0/1/4/6 for player, monster, item,
   missing).
3. **Monster re-path budget** (`pathing.md` §9.10 / OQ8). Already
   implemented on this base (commit `9765412`): `path::walk::Walk::repath`
   tests path +0x94 (`DynamicPath::repath_budget`) for monsters only.
   No change. Spec gap: the setter call with 20 at `0x005A7C20` is
   attributed to `monsters/ai.md`, which does not state it, so nothing
   calls `set_repath_budget(20)` yet.
4. **Position history in d2-sim** (`path-placement.md` §10 r7). Already
   implemented on this base (`d2-sim/src/path/history.rs`, written by
   `wiring/path/place.rs` and the walk step; commit `9765412`). No change.
5. **Unit allocation order** (`units.md` §3.1). **Skipped.** Moving
   `SUNIT_Add` after the per-kind init changes what the inits read: in
   d2rs the monster type init (`level_id` → the unit's list room) and the
   object init (`View::object_init`: `game.lists.unit(unit).room()`) read
   the room from the list entry that `SUNIT_Add` fills. The spec does not
   say where 1.14d's per-kind inits `0x00574250` / `0x0054F5D0` get the
   room or level before the unit is linked (the allocation's room
   argument? the path?), so the reorder cannot be done without guessing.
   Needed from a spec session: the room/level source of each per-kind
   init in `units.md` §1 / §3.1 step 7. The seed/GUID rollback after a
   refused add was left as is: it only acts after the fatal duplicate GUID
   (`unit-order.md` §2.1), where 1.14d stops.
6. **Wall remap and population seams.**
   - `DrlgData::wall_remap` is now the 1.14d table, embedded from
     `specs/drlg/wall-remap.tsv` (`WALL_REMAP_TSV`, strict
     `WallRemap::parse`, `WallClass::{Table, Keep, Stop}` per new type
     0..19; no longer an `Option`; `DrlgError::MissingWallRemap` replaced
     by `WallRemapType(t)` for a type past the 20 rows). The merge
     (`drlg/tiles.rs` `merged_type` / `linked_found`) follows
     `rooms.md` §9.6 step 3 as now written: bit 7 → m = t; door edge
     rules; `table` → column R's type (≤ 7, column `r0` included), else
     stop; `keep` → m = t; `stop` → stop. `wall-remap.md` status →
     implemented.
   - `WorldPending`'s population seams: already answered from the act
     DRLG on this base (`wiring/worldgen/population.rs`, every read of
     `levels.md` §11.6; commit `3167883`). No change. The remaining TODO
     there (water tiles, `population.md` §9.2: which DT1 field
     `0x00604BC0` reads) is a spec gap, not §11.
   - Not fixed, seen while reading: the corner handling of §9.6 step 3
     says the record **after R in its chain** (R +0x20) gets flag 0x8,
     not R's own type-4 half; `linked_found` still hides `r.half`. Left
     for a follow-up (outside this item; needs the chain order modelled).
7. **S→C 0x22 TSV row.** Done. `type:u8@1 unit:u32@3 skill:u16@7
   quantity:u8@9 body_state:u8@11`, checked against
   `inventory-moves.md` §11 (bytes 2 and 10 unwritten); `gen-proto`
   regenerated `d2-proto` (only the 0x22 entry and its new
   `UpdateItemSkill` struct changed). Committed separately with the
   "CODE-TABLE CHANGE:" subject.

Also: `count_minus_one` → `repeat` in `crates/d2-client/tests/prop_worldsim.rs`
and `e2e_single_player.rs` (allowed edits; d2-client itself still does
not build on this base, fixed on `impl-pc1-final`).

## Changed test expectations

| Test | Old | New | Why |
|---|---|---|---|
| `path::warp::tests::warp_tile_presets` | type 11 → 'l', 10 → 'r' | 10 → 'l', 11 → 'r' | §12.1 r1 |
| `path::gap_tests::warp_tile_inputs_type_tile_and_packed_value` | added `[(31…), (30…)]` | `[(30…), (31…)]` | same |
| `prop_path_place::warp_tile_preset_random_tables` (model) | letter 'l' iff t = 11 | iff t = 10 | same |
| `items::inventory::tests::targeting_reset_in_list_order` | per-item probe (item b probe 6 → no 0x3F) | probe on the owner: every flagged item of a player sends 0x3F; a merc owner sends none | §5.3 |
| `prop_inventory` (model) | 0x3F iff the item's random probe = 0 | iff the owner is a player | §5.3 |
| `wiring::inventory::tests::host::checks_and_targeting_reset` | no 0x3F ("the probe answers 1") | one 0x3F (0xFF, GUID, 0xFFFF) to the player | §5.3 |
| `mutants_wiring_inventory::targeting_reset_clears_and_queues_0x3f` | loop over a forwarded probe 0/1 | player sends, merc does not; probe = unit type | §5.3 |
| `mutants_wiring_inventory` forwarding / bare-rest checks | `targeting_probe` forwarded to `InvRest` | seam removed | §5.3 |
| `d2-server items::tests::item_to_cube_puts_the_cursor_item_in`, `item_to_cube_stored_item_is_refused` | nothing sent | one 0x3F for the flagged cube | §5.3 (player owner) |
| `drlg::tests::mutant_tests::wall_remap_keep_reruns_the_flag_rules` → `wall_remap_keep_and_stop` | 8, 13 "keep R's type" + flag rules; 12 stops | 8 (off edge), 13 stop (no flag rules); 12 keep → type 12 + flag rules | `wall-remap.tsv`, §9.6 r3 |
| `drlg::tests::mutant_tests::non_door_over_door_on_neighbours_edge_stops` | off-edge new type 1 over door → door kept + flags | keep type 12 → 12 + flags; table type 1 over type 8 → stop | same |
| `drlg::tests::tiles::wall_merge_needs_the_remap_table` | `MissingWallRemap` | replaced by `wall_remap_is_the_transcribed_table` and `wall_remap_parse_is_strict` | table is built in |

New: `drlg::tests::mutant_tests::bit_7_merges_to_the_new_type` (§9.6 step 3
rule 1; the old code kept R's type).

## Checks (on this branch)

- `cargo fmt --all --check`: clean.
- `cargo clippy --workspace --exclude d2-client --all-targets -- -D warnings`: clean.
- `cargo nextest run --workspace --exclude d2-client --no-fail-fast`:
  4596 passed, 5 failed. All 5 also fail on the base `666e2f2`
  (confirmed by running them there):
  `d2-sim monsters::ai::tests::specd_here_matches_tsv` and
  `specd_here_check_catches_perturbations` ("index 41: status spec'd-here,
  mirror false"), and `scenario-run` `starters_parse_and_round_trip`,
  `every_starter_runs_twice_identically`,
  `comparator_finds_every_perturbed_record_of_a_real_run`
  (`traces/scenarios/vendor-buy-sell.scenario` line 29: "BuyItem has no
  field mode").
- `python3 tools/coverage.py --check`: 0 errors. `python3 tools/spec_index.py --check`: clean.

## Local run queue

Nothing new needs game files. The wall-remap values still have no check
against a recording (`rooms.md` open question 8), as before.
