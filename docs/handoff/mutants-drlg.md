# Mutation testing: d2-sim drlg (levels, rooms, preset, maze, outdoor)

> Not yet folded into `docs/HANDOFF.md` and `docs/PLAN.md`; a docs session folds it, then this file stays as the detailed record.

Cloud test session, 2026-10-06, METHODS M08 (prove the check can fail).
Branch `claude/mutants-drlg`, based on `claude/tender-meitner-mphas3` at
`4b5b0bf`. Repo only, synthetic data, no game files (M09). Read only
`specs/`, `docs/`, `crates/`. No code was changed: tests only (new files
`crates/d2-sim/src/drlg/{tests,maze,outdoor,preset}/mutant_tests.rs` +
`mod` lines). No coverage claims were added (the tests target mutants,
several assert parts of rules only; claims left to a docs/coverage pass).

## 1. Counts

`cargo mutants -p d2-sim --file 'crates/d2-sim/src/drlg/**' -j 4
--timeout 60 -- --lib` (cargo-mutants 27.1.0) on the base commit:
4,638 mutants listed; the run was stopped by the coordinator's wrap-up
after 4,583 outcomes (the last ~55, all in `preset/`, untested).

| Outcome (before the new tests) | Count |
|---|---|
| caught | 3,457 |
| missed | 969 |
| timeout | 77 |
| unviable | 80 |

Missed by area: active 17, collision 1, data 6, level 22, `mod.rs` 4,
room 30, seams 2, tiles 53, maze 169, outdoor 629, preset 36.

**After:** 131 new tests (tests 36, maze 26, outdoor 58, preset 11). The
re-run (`cargo mutants ... --iterate`, which re-tests only the missed)
was **not run** (budget). Every killed claim below was checked one by one
by applying the mutation by hand and running the module's
`mutant_tests` (about 330 mutants checked this way, all killed except
those listed in §3). Next session: run the iterate pass to get the exact
after count.

## 2. What the tests cover (spec rules)

- levels.md: closed containment, jungle bit (§3 r4), warp array level-0
  record, lvlwarp row of a slot (§7 r4), room_at near-array precedence
  (§8), spawn tiles by class pair (§10 r2), waypoint position and class
  573 (§10 r4), first warp room (§10 r3), room count.
- rooms.md: act-room flag round trip (§5, §8), adjacency ids, removal
  test outside towns and in towns (§8 r1), client membership (§7 r1),
  rooms-near gaps (§3 r1), freed rooms leave status lists, warp links
  (§3 r3: slot count c, level 133, unflagged slots), client-copy tile
  free at status 4, collision update in a neighbour's grid (§10.5), DT1
  mask bits (§9.3), door unit coordinates, blank keys, FillBlanks hidden,
  wall warp units, record flags, remap rows and keep/stop (§9.6), find
  rules (layer, shadow), linked exits, door edge cases, corner halves,
  animation (§9.7).
- maze.md: gaps / direction / shape tables, Gen primitives (place against
  a level link, pick, merge bounds, blank, extreme finders, bounding
  box), per-level builders (61, 114/116/119, 92), all §6 stamp tables by
  level, hub, lava cross, spiral, sewers 1, theme and rotation tables,
  barracks stamp order, river files, small flag.
- outdoor.md / outdoor-tilesub.md: table views, the LevelTypes dispatch
  adapter, grid ops, stamp, fits margins, shuffle, RandomDS1, FarAway (rule
  model), waypoint, shrines, shares-edge / direction, Act I warps,
  A1M band, level 27 direction, jungle blocks, neighbour entries, dt1
  mask / floor flags, generation dispatch by act, cells to rooms, room
  grids, sub-theme pick, room substitution (scattered, CheckAll, tests,
  apply), border substitution (rule model), style map, vertex polygon
  (rule model), to_cells, corner/border pieces, link vis, link flags,
  borders (rule model), Act I: Dir, tables, grid path (rule model),
  cliff marking (rule model), river caves, transitions, river and bridge
  (rule model), dirt paths (rule model), specials 17/39, cottage.
- preset.md: lvlprest/monpreset records, door TSV ranges, DS1 version
  boundaries, object id ≥ 150 and conversions, link grid by cell,
  direction from picked file, waypoint cells, navi.

"Rule model" tests re-state the spec text as a small reference function
and compare on pseudo-random inputs (deterministic seeds); for the two
unstated points of the grid path search they use the code's documented
reading (marked in the test).

## 3. Survivors with a disposition (equivalent / unobservable / not spec-decided)

- EQUIV active.rs:71 `|`→`^` (two sites): the three flag bits are disjoint, so OR = XOR.
- EQUIV active.rs:291 `&`→`|`/`^`: free_room_tiles re-tests HAS_ROOM itself (and the active room is already gone), so a wider test frees nothing more.
- UNOBS outdoor/mod.rs:175 delete `-` in sub_defs: the default for a level id with no leveldefs row; every level id the DRLG allocates has a row (levels.md §4), the spec defines no missing-row value.
- UNREACH outdoor/mod.rs:509 NonOutdoor::generate → Ok(()): the act placer's allocation view never generates (get_or_alloc_level does not call generate).
- NOTSPEC outdoor/mod.rs:437 Outdoor::reset_level → (): what 0x006754C0 frees is not stated (code TODO, levels.md §9.4); generation rewrites every field it reads.
- EQUIV room.rs:247 `&`→`|`/`^` (warp-mask test in build_near): build_warp_links acts only on slots whose own flag bit is set, so entering it for a room without warp flags does nothing.
- EQUIV room.rs:387 `>`→`>=` (handler 0): force_status to the current status returns at once.
- EQUIV room.rs:393 `&&`→`||` (handler 1): bring_up re-tests flag 0x100000 after load_room_data, whose two steps are guarded by their own flags; a room with an active room always has 0x100000.
- EQUIV room.rs:396, 406:38, 415 `>`→`>=` (handlers 1, 2, 3): forcing the current status is a no-op; at status 2 counts[1] is 0, so handler 2's follow-up never runs.
- UNOBS room.rs:403/404/405/406:26 (handler 2 readiness test): propagate is depth-first and every near array holds its own room, so handler 3 (or handler 1's build) has loaded the DT1s and added preset units before handler 2 runs; the test always passes.
- EQUIV room.rs:426 `>=`→`<` (recompute): the status is always the lowest non-zero count, so recomputing a status < 4 changes nothing; recompute is never reached at status 4 (unset 3 tests it first; unset 0–2 follow a decrement of a count that made the status < 4).
- EQUIV room.rs:436 `<`→`>` (unset handler): unset 0–2 then take the status-3 branch, which recomputes the same way; it frees tiles only on a client copy when the status becomes 4, and counts[s+1] ≥ counts[s] (each room is in its own near array) keeps the status < 4 at s < 3.
- EQUIV room.rs:472 `&&`→`||` (set_step): status ≥ s with a non-zero count at ≤ s means status = s, and every handler is a no-op at its own status.
- EQUIV seams.rs:73, 85 (LevelTypes default bodies → vec![] / Ok(Default)): the defaults already return exactly those values.
- NOTSPEC tiles.rs:193 `||`→`&&` (CellGrid::get bounds, three sites): the mutant panics on an out-of-range read; no spec rule reads a grid outside its size, and no caller does (the 0 for outside is a defensive default, not a rule).
- EQUIV tiles.rs:316 `>`→`>=` (rarity_walk len > 1): with one entry the walk ends at index 0 either way (i − 1 saturates).
- EQUIV tiles.rs:316 `<`→`<=` (rarity_walk i < len): r < total, so n reaches ≤ 0 before the end; the bound is never hit.
- EQUIV tiles.rs:427 `>`→`>=` (choose_tile total > 0): roll(0) returns 0 without a step (rng.md §3).
- UNOBS tiles.rs:908 (frame records keep the base record's `half`): frame records are in no link list, and `half` is read only for linked records (§9.6 corner handling).
- EQUIV tiles.rs:933 `&`→`|`/`^` (animate_tiles room flag test): a room without flag 0x8000000 has no animation entries, so not skipping it does nothing.
- EQUIV maze/cells.rs:108 `<`→`<=` (direction, W test): with B.x = A.x the second condition needs B.w = 0; cells have SizeX > 0.
- EQUIV maze/cells.rs:112 `<`→`<=` (direction, N test): as for W, B.y = A.y needs B.h = 0.
- UNOBS maze/cells.rs:269 Gen::free_unlisted → (): a freed cell is in no level list and DRLG room slots are never reused; nothing walks the cell map or the slot afterwards.
- UNOBS maze/layout.rs:23–24 (first-cell centre: 12 arithmetic mutants): every later step is translation-invariant (place / merge compare cells with each other; normalize shifts to the level origin; §7 shifts relative to the neighbour level), so the start position never reaches an output.
- UNOBS maze/layout.rs:727:67 `<`→`<=` (rotation range B' < def): no cell reaches def B' (mask 0): pick runs only on a cell after a W/N/E/S link is made, and the theme target a[k] = 0 matches no cell (§8).
- EQUIV outdoor/grid.rs:188 `|`→`^` (stamp: PRESET | F << 16): disjoint bits after the AND-NOT of the file field.
- EQUIV outdoor/grid.rs:322:31 `>`→`>=` (FarAway d formula): at ax = ay both branches give 3·ax.
- NOTSPEC outdoor/grid.rs:305 `||`→`&&` (FarAway W ≤ 0 or H ≤ 0 guard): the original divides by zero there; spec: not reached by 1.14d level sizes (code TODO).
- EQUIV outdoor/grid.rs:392 `||`→`&&` (shuffle_cells guard): with exactly one of W, H ≤ 0, A = W·H ≤ 0 and the entry and swap loops are empty (no draws), as the guard returns.
- EQUIV outdoor/place.rs:271, 275 `<`→`<=` (direction W / N tests): as maze/cells.rs:108, B.x = A.x needs B.w = 0.
- EQUIV outdoor/place.rs:445 (A1W Rogue guard `link >= 0` → true): level 1 appears only in row (Re, 1, link 2).
- EQUIV outdoor/place.rs:574, 611 (`unwrap_or(-1)` → 1 for the next row's R0): the R arrays hold −1 for every row past a table's end and no table fills all TABLE_ROWS rows, so the default is never read.
- UNOBS outdoor/place.rs:553 (alternatives reset to −1 → 1 before backtracking): reached only when a linker runs out of alternatives; no synthetic or recorded Act I–V placement backtracks past a row (left for a recorded vector).
- EQUIV outdoor/rooms.rs:152 `|`→`^` (room flags |= f1 | 0x80000): no grid-1 bit is 0x80000 (link vis 0x10–0x800, shrines 0x1000–0x8000, waypoints 0x10000/0x20000), so the bits are disjoint.
- UNOBS outdoor/rooms.rs:161 (OutdoorRoom flags_ex field dropped): flags ex is grid 3, which no implemented builder writes (all zero).
- UNOBS outdoor/rooms.rs:234 `|`→`^` (grid edges |= 0x4): no cell reaching the edge pass has bit 0x4 set by then in the implemented builders (the path floor is not drawn, tilesub patterns used by tests carry no 0x4); OR and XOR agree.
- EQUIV outdoor/tilesub.rs:388 `||`→`&&` (CheckAll skip on W ≤ 0 or H ≤ 0): with one of them ≤ 0 both method loops are empty.
- EQUIV outdoor/tilesub.rs:445 `>`→`>=` (Trials > 0): Trials 0 runs a loop of 0 tries.
- EQUIV outdoor/tilesub.rs:129 `>`→`>=` (style map P > 0): the only P = 0 row gives 0 + (v − lo) = 0 either way (v = lo = 30).
- EQUIV outdoor/tilesub.rs:186 `*`→`+` (border group A = W·H ≤ 0): when the sum is > 0 but the product is not, shuffle_cells of W ≤ 0 or H ≤ 0 is empty with no draws.
- UNOBS outdoor/tilesub.rs:290 `>`→`>=` (replace P > 0): Wild contexts give P = B + s ≥ B − 1 > 0 for every base (4, 364, 799); P = 0 comes only from the Act V style map, and no implemented builder runs the barricade substitution (Act V TODO, outdoor.md OQ 9).
- UNOBS outdoor/tilesub.rs:508 (grid set bounds, 5 mutants): every apply position lies inside the (w+1)×(h+1) room grids (x + i ≤ w for scattered and CheckAll positions), so the guard never rejects a write.
- EQUIV outdoor/vertex.rs:50 `|`→`^` (link | preset-link flags): disjoint bits.
- EQUIV outdoor/vertex.rs:320 `|`→`^` (0x1 | 0x2): disjoint bits.
- EQUIV outdoor/vertex.rs:337 `%`→`+` (index of the vertex after next): next_vertex reduces mod n itself.
- EQUIV outdoor/vertex.rs:343, 415 `|`→`^` (0x1 | 0x2): disjoint bits.
- EQUIV outdoor/wild.rs:184 `<`→`<=` (advance while tries < 4): a node climbs when its tries reach 3, so tries ≥ 4 never occurs.
- EQUIV outdoor/wild.rs:191 `<`→`<=` (climb at tries 3): one more advance past position 3 keeps the facing (pos 4 has no row entry) and re-runs the same deterministic subtree, which fails again before the climb; node slots are reused, so the outcome and node count are unchanged.
- EQUIV preset/map.rs:226 `/`→`*` (cell grid height): the height only sizes the cell vector; every index (y/8)·gw + x/8 stays below gw·(h/8 + 1).

Still alive after the new tests (checked by hand, no disposition yet):
`preset/map.rs` 510, 511, 538, 540, 543 (river objects: the new
`river_objects_by_the_rules` passes but does not kill these; the room
probably never reaches the river branch with the fixture, to check),
`preset/room.rs` 67 (`cx < 0 || cy < 0` → `&&`: rooms never lie left of
or above their map, likely unobservable). Every other entry of the 969
not named in §2–§3 was not examined one by one (largely outdoor/wild.rs
and tilesub.rs lines expected to fall to the rule-model tests).

## 4. Code vs spec

No code was found wrong against its spec. No fix made.

## 5. Gate (this branch)

All pass on this branch: `cargo fmt --check`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo test -p d2-sim`, `cargo run -p
depcheck`, `python3 tools/spec_index.py --check`, `python3
tools/methods.py check`, `python3 tools/coverage.py --check` and
`--selftest` (d2-sim 1,363 pass, 5 ignored; coverage 3,259 claims, 0 errors).

## 6. Next steps

1. `cargo mutants -p d2-sim --file 'crates/d2-sim/src/drlg/**' --iterate
   -o <dir with this run's mutants.out>` to get the after count, plus the
   ~55 untested preset mutants.
2. Disposition the remaining survivors (§3 list) and the river-object
   ones.
3. Add `Covers:` claims where a test checks a whole rule.
