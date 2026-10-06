# Handoff: property tests of free-point searches, placement and warps — `claude/prop-path-place`

> Not yet folded into `docs/HANDOFF.md` (§1, §3, §4, §8) and `docs/PLAN.md`; a docs session folds it, then this file stays as the detailed record.

Cloud test session, 2026-10-06. Task class: property tests from a clear
spec plus root-cause fixes, medium (METHODS M14). Base:
`claude/tender-meitner-mphas3` at `edd9925` (has `d2_sim::path::{search,
place, warp}` from `impl-path-place`). Repo only, synthetic data, no game
files (M09). Inputs: `specs/sim/path-placement.md` §4, §7–§12,
Randomness, Edge cases; `docs/handoff/impl-path-place.md`; `crates/`.
Parallel sessions `impl-path-core` / `impl-walk` own other files under
`path/`; `path/mod.rs` untouched.

## 1. State

New file `crates/d2-sim/tests/prop_path_place.rs` (10 properties, public
API only, fakes of `CollisionView`, `PlaceHost`, `LevelView`,
`WarpTileView`). The fake view follows §4 (lookup from the hint, then
its adjacency array; missing room / grid 0x27 unmasked; size and box
queries from the centre's / low corner's room). Worlds: 1–3 random rooms
(random adjacency subsets in random order; overlapping only in the pure
§7 / §8 reference tests) or 1–4 non-overlapping all-adjacent rooms (the
"iff" tests); 3 of 4 cases wall the start and crowd its radius-4
neighbourhood so the ring / scan order decides.

| Property | Spec | Checks |
|---|---|---|
| `nearest_matches_ring_reference` | §7.2 | `nearest_free_point` = brute force from §7.2's ring order, any step 1–4, max distance 0–59, size 0–4, 5 masks, fallback, null room |
| `nearest_with_field_matches_reference` | §7.2, §7.3 | same with random valid walk-back fields (8 only at the centre; every byte a step one Chebyshev ring inward) and random origins; walk-back reference written from §7.3 rule 3 |
| `free_point_iff_within_radius` | §7.2 | `0x0064E7B0`: a point iff a free cell within radius 49 (or the start is free); independent key form: smallest ring, then smallest d, then columns by y left-first, then rows by x top-first; fallback room / null |
| `coarse_matches_reference` | §8 | `coarse_free_box` = reference from rules 1–4 (incl. the row lookup at the previous x, the rect last read), result and point |
| `coarse_iff_and_first_in_scan` | §8 | one room: found iff a free cell with dx ≡ dy (mod 2), both in [−49, 47]; winner = smallest (pass, dy, dx); on failure point y = Y0 + 47 |
| `floor_drop_never_blocked` | §9 | rule 1 start, = reference; a found point has size query 0 vs 0x3E01 and walks back through 0x801; otherwise only the fallback start; view unchanged; deterministic |
| `place_unit_never_blocked` | §10 | `NoPath`; rule 2 lookup (null hint dead); non-exact placements pass the unit-size 0x1C09 query; one teleport; host calls exactly rule 5 / rule 6 order; no draw; deterministic on cloned state |
| `spawn_entry_warp_draws_and_points` | §11 | `level_spawn_point` / `game_entry` / `level_warp_place`: one `spawn_room` call (tile 0 for entry), the RNG draws are exactly that seam's, in the seed's order (counting RNG), none added; `SpawnNotFree` iff the reference finds nothing; entry: 0x07, add, 0x15 flag 1; level warp teleports to the spawn point; deterministic |
| `warp_tile_preset_random_tables` | §12.1 | random lvlwarp tables (edge-biased i32 offsets): no panic; `NoLvlWarp` iff no record; far edge → nothing; else the one preset |
| `warp_player_random_tables` | §12.2 | random destinations, edge-biased `ExitWalkX/Y`, gated levels, gate values, sizes 0–5: no panic, no draw, outcome order of rules 1–4, Arrived: walk target and 0x0D |

RNG: the spec's Randomness says no function of §7–§12 draws; the only
draw on these paths is the DRLG's `spawn_room` (`drlg/levels.md` §10),
which the fake makes with a counting `Seed`. The placement functions take
no RNG, so "zero draws" is also structural.

M08: each check was shown to fail on a deliberate change of
`search.rs` (reverted): left/right column order swapped (5 properties
fail), tie rule `<` → `<=` (7), last ring bound `>=` → `>` (4), coarse
stop skipping dx = 0 (`coarse_matches_reference`). The warp properties
fail on the unfixed `warp.rs` (below).

## 2. Fix (root cause, `crates/d2-sim/src/path/warp.rs`)

Overflow panic (debug) in §12.2 rule 5 `point + ExitWalkX/Y` and §12.1
rule 3 `5·lx + OffsetX/Y` with large lvlwarp values (table data). Now
`wrapping_add` / `wrapping_mul`: 32-bit integer arithmetic as the
original's, identical results wherever the sum fits. Reading, not a spec
statement: the spec does not say what happens past i32 (it never does in
1.14d data, whose offsets are small) — a spec session may confirm.

## 3. Readings followed (unchanged from `impl-path-place.md` §4)

The references follow the code's `TODO(spec)` readings: §7.3 walk-back
point tests from the candidate's room; §8 rect rows / columns half-open.
They do not decide those questions.

## 4. Commands

`cargo test -p d2-sim --test prop_path_place` (default 256 / 128 / 512
cases; `PROPTEST_CASES=2000` passes in about 70 s, debug). Gate: `sh
tools/gate.sh all` PASS on `3a5cf4e`, and again on `2693d79` after merging
`claude/tender-meitner-mphas3` at `41b7686` (tests ported to
`path::Point` / `drlg::TileRect`, `wire-path-sim.md` §3) (all 13 steps: spec_index, methods,
coverage, trace checkers, hook selftest, fmt, depcheck, clippy workspace,
tests d2-sim + conformance / rest / d2-client, doc-tests).
