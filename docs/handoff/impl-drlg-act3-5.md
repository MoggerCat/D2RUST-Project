# Handoff: DRLG Acts III–V level generation (`impl-drlg-act3-5`)

Branch `claude/impl-drlg-act3-5`, from `claude/specs-staging` at
`5844674` (cloud, 2026-10-06). Specs: `specs/drlg/outdoor-act3-act5.md`,
`specs/drlg/outdoor.md` §9.1–§9.4, §11, `specs/drlg/maze.md` and
`specs/drlg/preset.md` (the link / door rules of spec commit f71ee9b).
Implementation session (M14: medium), four parallel agents on separate
files.

## State

**Implemented, unverified** (no Act III / Act V recording exists; every
vector below is derived from the spec, `outdoor-act3-act5.md` Test
vectors).

| Part | Code | Vectors / tests |
|---|---|---|
| Act III jungle placer (`outdoor-act3-act5.md` §2, DRLG seed) | `drlg/outdoor/jungle.rs` (`place_jungles`, `river_and_attach`, `connect_rivers`, `attach`, `code_to_id`, S / T tables); `place.rs` `jungles` stores ids / clearing counts in `OutdoorLevel::{jungle_ids, jungle_clearings}` (+0x1BC, +0x1B8) and returns level 78 | init 644409375: jungle positions, W 6 × H 14, 123 DRLG-seed steps, seed after {4015082244, 577631236}, rects, ids, clearings 3/3/2: **reproduced** |
| Kurast chain anchored on level 78 (`outdoor.md` §9.2; was the docks) | `place.rs` `kurast_chain` | 79..83 rects reproduced |
| Jungle stamping (§3, level seed) | `outdoor/act3.rs` `jungle_stamping` | stamp lists, steps 7/7/8 and seeds after for 76..78; rooms 11 / 10 + 32 / 11: **reproduced** |
| Kurast and Travincal (`outdoor.md` §9.4) | `outdoor/kurast.rs` (`kurast`, border rows, random placer R `0x0067EED0`, `travincal`) | 13 synthetic tests (`kurast_tests.rs`) |
| Act V build (`outdoor.md` §11 steps 1–9, `outdoor-act3-act5.md` §5) | `outdoor/act5.rs` (`barricade_borders`, `ravine`, `entrances`, `caves`, `connect_to_siege`, type-12 border substitution, `prisons`, `act5_specials`) | Act V placement rects 110/111/112/117 and the 110 siege-strip stamps **reproduced**; 13 tests (`act5_tests.rs`) |
| Maze link rules (f71ee9b) | `maze/cells.rs` (links prepended §2.4; cross-level sorted insert §7.1; room free removes links back, `rooms.md` §2.1; §3.7 probe), `maze/layout.rs` (§5.5 spiral files after all branches; §9 steps 3–4 links moved to the room BuildArea returned), `MazePresets::build_map` now returns `Option<DrlgRoomId>` (wiring `maze_presets.rs` passes F = 0) | `maze/links_tests.rs` (6 tests) |
| Preset door flag (`preset.md` §11) | `preset/room.rs` `DoorOutcome::sets_record_flag` | `door_record_flag_outcomes` |

Structure change: `outdoor/acts.rs` now holds Acts II and IV only; Act
III is `act3.rs` (+ `jungle.rs`, `kurast.rs`), Act V `act5.rs`. New error
`OutdoorError::Fatal(id)` (the original's fatal ids 0x27, 0x47, 0x219,
0x259, 0x63E, 0x64D, 0x64E, 0x78C, 0x799).

Tests changed (spec-driven, no assertion weakened):
- `outdoor/gaps_tests.rs` `act_wide_placement_per_act`: Act III used
  32×32 jungles (with the real placer one block row never reaches 2
  attach points and §2.6 restarts forever); now 64×192, and the chain is
  expected anchored on 78 (79's neighbour is 78, not 75), per §9.2.
- `outdoor/mutant_tests.rs` `generation_dispatches_by_act`: the Act III
  stub's `roll(14)` is gone; level 76 without block ids is fatal 0x27
  (§3 r2). `one_room_level` callers moved from level 79 (now stamps
  border rows) to level 107 (Act IV, stamps nothing); asserts unchanged.
- Maze test fakes return a built room from `build_map`; expectations
  unchanged.

Gate at the head of this branch: `cargo fmt --all -- --check`,
`cargo clippy -p d2-sim -p d2-server --all-targets -- -D warnings`,
`cargo run -p depcheck`, `python3 tools/methods.py check`,
`python3 tools/spec_index.py --check`, `python3 tools/coverage.py
--check`: clean. `cargo test -p d2-sim -p d2-server --no-fail-fast`:
all green except the ten known reds of other sessions
(`monsters::ai::tests::specd_here_*`, `skills::use_::tests::*`,
`skills::mutant_tests::table_check_mutants::*`,
`missiles::tests_bodies::*`). `drlg` lib tests: 367 pass.

Coverage (unit tier): `outdoor-act3-act5.md` 0 → 40/45,
`outdoor.md` 65 → 78/82. Unclaimed: act3-act5 §rules text, §4 r3, §5
text, edge cases r3 (head/tail idx += 2 is not observable with SXb = 2),
r6 (memory only); outdoor.md §3 text, §7 text, §7.5 text, §12.2.

## Open questions (TODOs in code)

1. **Jungle code lookup** (§2.7, edge case 1, act3-act5 OQ 2): a chained
   lookup whose row is outside 1..14 (also row 0 slot 3 = S[16] / T[0][3])
   is `Fatal(0x78C)`; the image value there is not given.
2. **Drop** (§2.4 step 4): fewer than i+1 blocks with C = 2 is not
   described; nothing is dropped, n still decrements.
3. **Jungle id array shorter than SXb·SYb** at build (leveldefs 76 changed
   between creation and build): read as 0.
4. **Kurast border order** (§9.4): top row, bottom row, sides, corners as
   the table's columns (level 80: top/bottom alternate per i as stated);
   decides which first stamp draws its build-list roll first. Confirm
   against `0x0067EAD0`, `0x0067EC30`, `0x0067ED70`.
5. **Random placer R**: stamps at the tried cell, no margin / offset.
6. **Ravine walk** (§11 step 3): grid value − B outside 0..11, or a walk
   that never reaches (0, gh−2), is not described: placeholder
   `Fatal(0x0067DEF0)` (not an original id).
7. **Barricade walk** (§11 step 2): fixed count of 2-cell steps; ends
   that never meet are not described.
8. **Caves / specials tall-wide** (§11 steps 5, 9): level tile rect used
   (same as the grid for all data).
9. **Prisons helper `0x0045C390`** read as `roll` (`rng.md` §3).
10. **Maze cross-level sorted insert**: "otherwise at the tail" when no
    later record is preceded.
11. **Built rooms' orth links** are dropped after maze generation:
    `drlg::room` rooms have no link list (needs a field on the shared
    room struct).
12. **Link bit 0** (`preset.md` §6 step 10) needs a link-record type;
    **door flag 0x20** is not set by the tile code (`rooms.md` §9.5.1;
    `sets_record_flag` exists, the seam returns nothing).
13. `maze.md` §5.5: the spiral geometry never makes branches edge-adjacent,
    so the after-all-branches pass is indistinguishable from the old one.

## Checks to queue (HANDOFF §5, local)

- **C65** (game files): `D2_GAME_DIR=<install> cargo test -p d2-sim
  --test game_drlg_tables act3_act5 -- --ignored`: `act3_act5_table_values`
  (leveldefs 76..78 64×192, offsets −1; 111/112 −1; 110 240×48 at
  (760, 1000); 117 128×80; lvlprest 573/574 64×32 Files 0, 530..544 Files
  3 (541: 5), 545..572 and 575..604 Files 1, all 32×32; 865 SizeX 16; 652
  48×16; 653..658 Files 1) and `act3_act5_placement_on_live_tables` (the
  derived Act III / Act V vectors on live tables). A mismatch is a
  finding for `outdoor-act3-act5.md`.
- **A (player)**: `outdoor-act3-act5.md` OQ 1 and 3: record entering Act 3
  (DRLG-seed draws `0x00677966`…`0x006784D9`, levels 76..78 +0x1C..+0x28,
  +0x1B8, +0x1BC) and builds of 76..78, 111, 112, 117; compare with the
  unit vectors above.
