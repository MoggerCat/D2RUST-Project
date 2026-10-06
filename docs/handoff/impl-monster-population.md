# Handoff: monster population implementation (`claude/impl-monster-population`)

Session: cloud implementation (METHODS M14, medium), 2026-10-06. Base:
`claude/bold-ptolemy-jvyvxy` at `a5b323a` (PR #13). Inputs:
`specs/monsters/population.md` (+ `specs/monsters/preset-monsters.tsv`).
Scope of every claim below: this branch, synthetic tests only, no game
files (M09).

## 1. State

**Implemented, unverified** (M02): the spec is a draft; no draw sequence
has been compared with 1.14d (spec open question 2).

- `d2_sim::monsters::population`: regions (§2.1–§2.5: creation on the
  monster-region seed, list with ranged re-draws, appearance variants,
  entries on demand), room population (§3: guard, rectangles, density on
  the game seed, failed-pick early end, spawned count), pick (§4, incl.
  the overrun into MonDen), boss or pack (§5), random bosses with
  champion / unique minions (§6), packs incl. sparse and evilhut object
  (§7), spawn point (§8, 1.14d warp sense), placement search and creation
  call (§9: masks, water tiles, ring walk incl. the wrong-way start,
  footprints, creation flags, alignment, region count), parties and
  tentacle heads (§10), presets (§11: pass, class ranges, regular swaps,
  superuniques, the special ids of the TSV, class for level), ambient
  spawns (§12), bookkeeping (§13), `spawn_mode_xy` (§14.1).
- The spec names `monsters::placement` for §9; it is
  `monsters::population::placement` here (task scope: one submodule).
- Tests: 35 in `population::tests` (every synthetic vector of the spec,
  every edge case except 15 which no caller can reach, the TSV check
  with a perturbation test). `py tools/coverage.py`: population.md 156 of
  165 units claimed (unit tier). Unclaimed: §1 r2–r3 (recording facts,
  out-of-tick callers), §3.3 (dead branch, not implemented by rule),
  §14 r2–r6 (statements about other specs), edge case 15.
- Gate run: `cargo fmt --all -- --check`, `cargo clippy -p d2-sim
  --all-targets -- -D warnings`, `cargo test -p d2-sim` (547 pass, 3
  ignored), `cargo run -p depcheck`, `python3 tools/spec_index.py
  --check`, `python3 tools/methods.py check`, `python3 tools/coverage.py
  --check`: all clean.

Files changed: `crates/d2-sim/src/monsters/population/` (new) and the
`pub mod population;` line in `crates/d2-sim/src/monsters/mod.rs`.

## 2. Code map rows (for `docs/HANDOFF.md` §3)

| Path | What | Spec |
|---|---|---|
| `crates/d2-sim/src/monsters/population/mod.rs` | `CoordRect`, `RoomBox`, `TileRec`, `PresetUnit`, `GameInfo`, `PopState` (regions, mon seed, superunique bits), `Ctx`, `room_step` / `populate_once` (§1.1 order) | `monsters/population.md` §1 |
| `…/population/data.rs` | `PopTables` view from typed `levels` / `monstats` / `monstats2` / `superuniques`; `with_bins` adds chain length (+0x4A) and composit counts (+0x15…+0x25) from the `.bin` records | Constants & data dependencies |
| `…/population/region.rs` | `Region`, `Regions::create`, list draw, `variants`, `entry_for`, §13 counters | §2, §13 |
| `…/population/room.rs` | `populate_room`, `pick`, `boss_or_pack`, `tries`, `ambient` | §3–§5, §12 |
| `…/population/spawn.rs` | `random_boss`, `boss_spawn`, `champion_minions`, `unique_minions`, `pack`, `members`, `party`, tentacles | §6, §7, §10 |
| `…/population/placement.rs` | `place` (`0x005B2A00`), `place_at`, `place_near`, `spawn_point`, `ring_search`, creation flags | §8, §9 |
| `…/population/preset.rs` | `place_presets`, `preset_spawn`, superuniques, `SPECIAL_PRESETS` (checked against the TSV), `class_for_level`, `spawn_mode_xy` | §11, §14.1, `preset-monsters.tsv` |
| `…/population/seams.rs` | `PopWorld`, `MonsterInit` (= `PopHost`) | |

## 3. Seams and expected providers

Wiring: the tick room pass (`sim/tick.md` §4) calls
`population::room_step(cx, room, first)` per active room in act room-list
order; `first` = room +0x34 bit 0 clear (the tick owns the bit). Game
creation calls `Regions::create(tables, info, game_seed)` and stores the
result plus `lo'` in `PopState`. `PopTables::from_records(...)
.with_bins(&monstats_bin, &monstats2_bin)` builds the view from the
fixed-up set.

| Seam | Provider | Covers |
|---|---|---|
| `PopWorld` | DRLG (`drlg/*`), units, quests | game / room / unit seeds; room level and populated level; populated-room count `0x0061ABF0`; coordinate list / at point / index at point; room box; warp points; kind-11 spawn location; room at point; collision `0x0064D9B0` and `0x0064CB30`; tile records; preset list; client count; unit room and position; quest flags; Chaos state `0x005B5210`; nearest free point `0x0064E840` |
| `MonsterInit` | **`monsters/init.md`** (parallel session), units, AI, objects | allocation `0x00555230` (game-seed step + GUID); monster flag 2; coordinate record `0x00552D60`; alignment `0x005543B0`; unit flags; extras `0x005B21B0`; init `0x005B1CF0`; class / level / type flags; boss modifiers `0x005A0760` and their init tail; modifiers 16 / 22; xfer `0x005A0930`; owner data `0x0058F030`; minion list `0x0058F100`; owner `0x005DD330`; quest hook `0x00544E80`; superunique init; `0x005B24E0`; objects; barricade objects; `0x0058F000` / `0x00666120`; event 7 `0x005417D0`; `0x005B1990`; inactive restore `0x00542B40`; object population `0x00552610` |

Public helpers for AI / skills (§14.3): `placement::spawn_point`,
`placement::place*`, `spawn::random_boss`; `Regions::entry_for` for
monster init (§2.5); `Regions::{alignment_changed, count_kill,
inactive_unit, den_of_evil}` for the alignment setter, death, inactive
units and the Den of Evil quest (§13).

## 4. Open questions (code TODOs; narrowest reading taken)

1. §6.3: the boss creation mode is not stated; mode 1 (the TSV's id 2
   row also says mode 1).
2. §6.3 r4: with a GUID, the nearest-free-point fallback places in the
   room `0x0064E840` returns.
3. §6.5 r4 and §11.4 r6 (hcIdx 60): `0x0058F030` arguments not stated;
   dedicated seam methods (`unique_minion_owner_data`,
   `superunique_owner_data`).
4. §11.4 r6 hcIdx 10 (Radament): the seed of `roll(5)` and the mode are
   not stated; boss unit seed, mode 1, `place_near` r 4 flags 0x40 for
   the skeletons and the four mages.
5. §11.5 r4: the seed of the event-7 `roll(50)` is not stated; the whole
   schedule is the seam `schedule_monumod`.
6. §11.5: ids absent from the TSV (0, 1, 6, 7, 9, 12–16, 19–21, ≥ 33)
   spawn nothing.
7. §10.3 r1: "owner data and minion list as in 10.2.3" read as the same
   calls without the SetBoss condition.
8. §9.2 / spec open question 5: n = 1 tile record tests no tile.
9. §4 / §3.1: a null region at pick time or for the populated level
   after the guard: no pick / no population.
10. §2.4 r1: `(1 << (T & 31)) − v` compared signed (i32).
11. §13.2 / spec open question 6: old alignment 4 excluded as written.
12. §3.2 density: `lo' mod 100000 ≤ MonDen` compared as i64 (MonDen is
    ≤ 10000 after the clamp; negative stored values are not described).
13. `Region::new` clamps the difficulty index to 2 for the levels
    columns (difficulty ≥ 3 only matters to §11.4 r1).
14. `0x0052D0F0` (spec open question 1): exposed as `populate_once`
    (presets, restore, objects, population, without the ambient call).
15. Carried from the spec: open questions 1–8 stay open; 3, 4, 5, 6, 7
    and 8 are seams or TODOs above.

## 5. Checks to queue (`docs/HANDOFF.md` §5)

1. **Game files (C):** an `#[ignore]` test reading `D2_GAME_DIR` that
   builds `PopTables` from the live tables and checks the spec's "Real"
   test vectors (Blood Moor / Cold Plains / Den of Evil rows, Rarity,
   groups, parties, superuniques 0–9, placespawn and sparsePopulate
   rows). Command: `D2_GAME_DIR=… cargo test -p d2-sim population --
   --ignored` once written. Expected: every row equal.
2. **Recording (A, spec open question 2):** room-seed and game-seed draws
   (call site, `lo'`) during the first population of a Blood Moor room;
   replay through `populate_room` with a `PopHost` fed the recorded
   rooms and collision. Expected: identical draw sequence and units.
3. **Recording replay (tick traces):** group sizes and creation order of
   `20261006-015554` / `-021854` / `-022304` (Test vectors "Recordings")
   once `MonsterInit` and the DRLG providers exist.
