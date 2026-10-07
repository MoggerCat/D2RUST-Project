# Handoff: DRLG answers and the Cold Plains grid (`fix-drlg-answers`)

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of the tenth fold (`claude/fold-handoff-10`); this file stays as the detailed record. Its open questions are in HANDOFF §7 "Tenth set" (PC 1 / PC 2).

Branch `claude/fix-drlg-answers`, from `claude/specs-staging-2` at
`ddbfe0b` (cloud, 2026-10-07). Implementation session (M14: medium),
repo only. Implements PC 1's answers to `impl-drlg-act3-5` Q1–Q13 and
`impl-room-population` §3 Q1–Q3 (spec commits `a4d3c19`, `2e6ac07`,
`70b1a1d`) and works the Cold Plains 97 vs 98 finding against the new
`outdoor.md` Test vectors "Cold Plains grid". **Unverified** (M02): the
live check is HANDOFF §5 C92.

## 1. Answers implemented

| Question | Spec | Code |
|---|---|---|
| act3-5 Q1 jungle code lookup outside rows 1..14 | `outdoor-act3-act5.md` §2.7 table | `jungle::code_to_id`: row 0 reads 0 (fatal 0x78C at the end), rows 545..572 slots 0 / 2 read 0, slots 1 / 3 → new `OutdoorError::Crash(CRASH_EXIT_ID)` (1.14d stores V and crashes in §3). Row 15 (L = 15 with an attach bit) is not in the table: still fatal 0x78C, TODO |
| Q2 drop past the C = 2 blocks | §2.4 step 4 | `place_jungles` step 4: n decrements only on a hit, a miss draws again (unreachable) |
| Q3 short jungle id array | §3 | `act3::jungle_stamping`: never short in 1.14d; a short array is `OutdoorError::JungleIdsShort` (after the step-1 roll), no longer read as 0 |
| Q4 Kurast border order, Q5 random placer R | `outdoor.md` §9.4 | already as answered; `random_presets` computes n in i32 (hi < lo no longer underflows) |
| Q6 ravine walk, Q7 barricade walk | §11 steps 2, 3 | ravine: out-of-range k or an endless walk → `Crash(CRASH_RAVINE_WALK)` (was the placeholder `Fatal(0x0067DEF0)`); barricade: equality-tested loop, guard → `Crash(CRASH_BARRICADE_WALK)` |
| Q8 caves / specials tall-wide, Q9 prisons helper | §11 steps 5, 8, 9 | already as answered; tests for w = h added |
| Q10 sorted insert tail, Q13 spiral branches | `maze.md` §7.1, §5.5 | already as answered |
| Q11 built rooms' links | `rooms.md` §1, OQ 18 | dropped (not observable); `maze/mod.rs` doc says why |
| Q12 link bit 0 | `preset.md` §6 step 10 | 1.14d never sets map +0x20: `link_grid` stays `None` in production; TODO removed |
| Q12 door flag 0x20 | `rooms.md` §9.5.1, `preset.md` §11 | `LevelTypes::door_unit` now returns whether the record gets 0x20 (`DoorOutcome::sets_record_flag`); `tiles::add_wall` sets `rec_flags::DOOR_UNIT` |
| room-population Q1 orientation > 19 | `levels.md` §11.3 step 6 | rule 0 kept (not reproducible, not reached); comment |
| Q2 blocker grid layout | §11.3 step 4 | (W+1) × (H+1) as built; `debug_assert` that own records are inside |
| Q3 out-of-grid record lookups | §11.4 table | `Drlg::coord_at`: with 0 ≤ cy ≤ H a block index inside (W+1)(H+1) reads that cell (column wraps); other cases no record |

## 2. Cold Plains 97 vs 98

- Synthetic: `drlg/outdoor/cold_plains_tests.rs`
  `cold_plains_grid_reproduces_the_table` builds level 3 on the recorded
  Act I geometry (`act1_data`) through the real build, with lvlsub files
  made so each substitution the spec derives (Border - Middle island,
  Border - Corner at (6, 6) with blanks (8, 9), (9, 9), the two Border -
  Border bumps) matches at exactly one place. Every border-piece and
  blank cell of the table is reproduced, the link midpoints carry
  0x30400, and §12.1 gives 98 rooms. The ring of §4–§6 (rect, three
  links) is the rules' own and matches the table before any
  substitution. M08: writing 0 instead of 0x100 for a blank cell fails
  it. Special presets (48, 44, 29, 30, 51, and 49 when the cottage draw
  allows) land where the synthetic draws put them, so only "a room, not a
  border piece" is asserted for those cells.
- Not found in the cloud: no code path produces a third blank on the
  synthetic build; blanks come only from `tilesub` blank cells and §6
  step 5, both as specified. The live difference must come from the live
  lvlsub rows / DS1 groups or a draw divergence before a substitution.
- Diagnostic: `OutdoorLevel::sub_hits` (`tilesub::SubHit`, not original
  state) records each border-substitution replacement with the level
  seed's lo' after the variant roll. The live test
  `outdoor_levels_generate_through_the_dispatcher` now prints the grid
  and both difference lists and asserts the grid table, then the four
  recorded hits (seq s = level-seed draw s − 6897: the spec's two lo'
  values sit at draws 1550 and 1747, which fixes the type-3 lo' at draws
  3307 and 3602), then 98. Queued as C92.

## 3. Open questions

1. `outdoor-act3-act5.md` §2.7: row 15 of T (L = 15 with an attach bit)
   is not in the "outside rows 1..14" table.
2. `rooms.md` §9.6 step 3: the re-run of the flag rules on a layer-above
   door record without flag 0x20 calls the door unit; which room (the
   current one or N) and position it passes is not stated. No call is
   made (TODO in `tiles::linked_found`).
3. `outdoor.md` Test vectors: "group 8", "group 11" (type 3) and "group
   1" (type 2) are read as indexes into the DS1 group list (C92 asserts
   them so).

## 4. Tests changed (spec-driven, none weakened)

- `act3_tests::code_to_id_table`: 0x32 is now `Crash` (row 548 slot 1 =
  V, §2.7 table); added 0x92 (slot 3, `Crash`) and 0xF1 (row 0, fatal).
- `tests::logic::grid_build_blockers_orientation_1`: (65, 100) reads cell
  (0, 1) (column wrap, §11.4 table) instead of no record; added the
  previous-row wrap and the two no-record cases.
- New: `jungle_stamping_rejects_a_short_id_array`,
  `ravine_walk_off_the_pieces_is_reported`, w = h cases in
  `caves_tall_and_wide` and `special_presets_rows_and_tall_wide`,
  `door_record_flag_0x20_follows_the_door_unit` (M08: dropping the OR
  fails it), `cold_plains_grid_reproduces_the_table`.

## 5. Gate

See the commit message for the exact results: `CARGO_INCREMENTAL=0
cargo test -p d2-sim -p d2-server -p test-fixtures -p conformance`,
`cargo clippy -p d2-sim -p d2-server --all-targets -- -D warnings`,
`cargo fmt --all -- --check`, `python3 tools/coverage.py --check`,
`python3 tools/spec_index.py --check`. `d2-client` not built.
