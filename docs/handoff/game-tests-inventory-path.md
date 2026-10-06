# Handoff: game-file tests for the inventory grids, belts and path tables (branch `claude/game-tests-inventory-path`, 2026-10-06)

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Cloud test session, repo only (no `game/`), medium effort, from
`claude/tender-meitner-mphas3` at `edd9925`. Read: `specs/items/inventory.md`,
`specs/sim/path-placement.md`, `specs/sim/pathing.md`,
`specs/sim/path-tables.tsv`, `tools/trace-recorder/path_tables.py`,
`docs/`, `crates/`. The diff adds one test file and this note. It changes
no library code, spec, `HANDOFF.md` or `PLAN.md`.

## 1. State

- `crates/d2-sim/tests/game_inventory_path.rs`: 6 `#[ignore]` tests that
  read `D2_GAME_DIR`, plus 2 CI tests that check the harness itself.
  The ignored tests compile and pass clippy, but **none has run**: there
  are no game files in the cloud (M02).
- **Expected values unconfirmed.** Every value is a spec table fact
  (`inventory.md` §1.3 measured table, §3.1, D1/D2) or an invariant a
  spec rule states. Following the `HANDOFF.md` §8 lesson, no test
  carries a `Covers:` claim. Each has a `// Claim once the first local
  run passes (note §1): …` line instead. Coverage is unchanged here.
- Loading: the shared `items_treasure_live` loader (`ArchiveSet::open_dir`
  → `bin::load` → `fixup::read_animdata` → `fixup::apply`), then
  `InvTables::from_fixed`, the projection the sim uses. The path test reads
  `D2_GAME_DIR/Game.exe`, the same place `path_tables.py` looks first.
- Already in-crate: `items::inventory::tests::real_grid_belt_and_type_tables`
  (D1–D3 on the raw extracted `.bin` files, ignored, queued by
  `impl-inventory`). The tests here go through the projection and the
  d2rs functions (`grid_record`, `page_grid_size`, `search`, `similar`,
  `body_location_allowed`) instead. They don't repeat D3.

## 2. Tests

| Test | Checks (source) | Intended claim |
|---|---|---|
| `live_inventory_records_both_resolutions` | 32 records (typed and projected); records 0–15 = the §1.3 table (10 × 4 except 5: 10 × 10, 8: 6 × 4, 9: 3 × 4, 12: 6 × 8, 13: 0 × 0); records 16–31 = copies of 0–15, record 29 255 × 255 | `inventory.md` §1.3 |
| `live_page_grids_every_owner` | `page_grid_size` for classes 0–6 × pages 0–5, 0xFF × classic / expansion: page 0 and others → the class record (10 × 4), trade 10 × 4, cube 3 × 4, stash 6 × 4 / 6 × 8; class 7 → none; monster 10 × 10 on every page; objects 0x152 / 0x153 → 10 × 4, other objects none; `BODY_GRID` 13 × 1, `BELT_GRID` 16 × 1 (§1.2, §1.3) | `inventory.md` §1.2 |
| `live_belt_capacities` | 14 `belts` records = 12, 8, 4, 16, 8, 12, 16 twice (D2, §3.1); default record 2 → 4; every count ≤ 16; every item that equips at body location 8 names an existing belts record (§3.1 rule 1). Prints the belt-item count | `inventory.md` §3 r1 |
| `live_potion_groups_similar` | §3.4 on the live misc records: `hp1`–`hp5`, `mp1`–`mp5`, `rvl`/`rvs` exist; `similar` is true inside a group and false across groups | `inventory.md` §3 r4 |
| `sweep_every_item_size_places` | every distinct weapons / armor / misc size (the search reads only w × h) × every distinct non-empty grid of records 0–15 × player / other: an empty grid filled with the item through `search` gets only in-bounds, fitting spots (§2.1); player spots have weight > 0; the search fails exactly when no fitting cell is left (edge case 9); zero sizes never place (§2.2). Prints the sizes, the zero-size count, items larger than 10 × 4, beltable / beltable 1 × 1 counts | none (invariants only) |
| `live_path_tables_equal_game_exe` | every row of `sim/path-tables.tsv` equals `Game.exe` at its `va` (i32, `altdir` u8; zero past a section's raw data, as `path_tables.py`); rows of a table are contiguous (va = first + index · columns · size); d2rs's `FIELD_DX` / `FIELD_DY` equal the exe bytes | none (the TSV = exe fact is a spec status line, already shown by `path_tables.py`) |
| `path_tsv_shape_and_mismatch_report` (CI) | 582 rows, the 18 tables and row counts `pathing.md` Constants lists; on a PE image built from the TSV the comparison reports nothing, and exactly one line for a changed `snap9` row (M08) | — |
| `fill_check_on_the_spec_grid_sizes` (CI) | the sweep's body on the §1.3 sizes and items up to 3 × 5 reports nothing; `any_fit` sees a blocked fit | — |

## 3. Not done, and why

- **Path-placement §11 spawn points on the live DRLG.** The spec's real
  points are R1 (4863, 5653) and R2 (4673, 4548), from
  `20261006-015956` / `20261006-022633`. The spec doesn't state the map
  seeds of those recordings. R1's tile (972, 1130) lies inside the
  Rogue Encampment rect that `world_data/tests/game.rs` derives for map
  seed 644409375 (960, 1112, 56 × 40). R2's tile (934, 909) doesn't, so
  the two recordings used different seeds. Also, no live
  `CollisionView` / `LevelView` provider exists (only test grids
  implement them). `Drlg::spawn_room` exists, but the free-point search
  needs the collision grid of the streamed rooms. Settle: a spec session
  states each recording's map seed (`.d2s` 0xAB) in path-placement Test
  vectors, and the wiring gets a live collision view. A test can follow
  then.
- **The path TSV against live tables.** The specs say the TSV is
  `Game.exe` constant data, not table data. So the check reads the
  executable, and d2rs's own copies (only `FIELD_DX` / `FIELD_DY` exist in
  Rust) are checked against the same bytes. The SHA-256 check stays in
  `path_tables.py` (d2-sim has no hash dependency).
- **Live belt vectors (B1–B5).** They need an `InvWorld`
  implementation (30 methods). The synthetic B tests already cover the
  rules. Only the table side (§3.1, §3.4) is checked here.

## 4. Local run queue (fold into `HANDOFF.md` §5 C)

With `D2_GAME_DIR=<install>` (the install with the MPQs and `Game.exe`):

```
cargo test -p d2-sim --test game_inventory_path -- --ignored --nocapture
```

Expected: 6 passed, 0 failed (`live_inventory_records_both_resolutions
live_page_grids_every_owner live_belt_capacities
live_potion_groups_similar sweep_every_item_size_places
live_path_tables_equal_game_exe`). Without `--ignored` the 2 CI tests
pass. Look at the printed lines: belt-item count, item sizes, zero-size
items, items larger than 10 × 4, beltable counts.

Cross-check: `py tools/trace-recorder/path_tables.py` must agree (exit 0)
with `live_path_tables_equal_game_exe`.

Record the results in `HANDOFF.md` §5 Done and §1. For each test that
passes, turn its `Claim once…` line into a `// Covers:` claim (same IDs:
`inventory.md` §1.3, §1.2, §3 r1, §3 r4). Then rerun
`py tools/coverage.py --check`. If a test fails, fix the test from the
observation, or turn the spec fact into an open question. Points to look
at first:

1. Record 29 = 255 × 255: the spec says this record is a copy of 13
   (0 × 0), yet 255 × 255. The test reads "16–31 copies of 0–15" as
   record r = record r − 16, except 29.
2. Pages 5 and 0xFF: §1.3 "other (0)" is read as every page other than
   1–4, so it gets the class record.
3. Body location 8 items: read through `body_location_allowed` (itemtypes
   `bodyloc1` / `bodyloc2`), so an item type with `bodyloc` 8 that isn't a
   belt would also be checked.

## 5. Gate (this branch)

`sh tools/gate.sh` after `sh tools/cloud-setup.sh` and `cargo install
cargo-nextest --locked`: GATE PASS (all 13 steps; nextest, the 6 new
ignored tests listed as skipped, the 2 CI tests pass; coverage 3,542
claims, 0 errors, unchanged). Non-source files in the diff: this note only.
