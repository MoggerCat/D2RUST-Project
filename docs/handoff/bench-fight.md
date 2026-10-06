# Handoff: loaded tick bench and live-shaped sprite benches — `claude/bench-fight`

Cloud tooling session, 2026-10-06, medium (HANDOFF §2 step 7r). Base:
`claude/tender-meitner-mphas3` at `01dff69`. Repo only, synthetic
fixtures, no game files. No optimisation was made; only fixture moves,
fixture writers and bench cases. For the coordinator to fold into
`docs/HANDOFF.md` / `docs/PLAN.md` (neither is edited here).

## How to run

```
cargo bench -p d2-sim --bench sim -- sim_fight         # the loaded tick
cargo bench -p d2-formats --bench formats -- sprites_live
# quick pass: append  --warm-up-time 1 --measurement-time 3
```

## (a) Combat / missile fixtures behind `bench-fixtures`, "fight" case

- **Moved** (verbatim) out of `crates/d2-client/tests/e2e_single_player.rs`
  into `crates/d2-sim/src/wiring/action/tests/fight.rs`: `MULTI`,
  `PLAYER_SC`, `MONSTER_DT`, `monster_class`, `skill_rec`, `arrow`,
  `skills`, `combat_tables`, `vitals`, `anim_data`. They
  are reached as `d2_sim::bench_fixtures::combat::*`; the e2e imports
  them from there (its dev-dependency on `d2-sim` now enables
  `bench-fixtures`). `drop_tables` stays in the e2e: since the merge of
  `unify-items` it builds on the e2e's own item world
  (`game_item_tables`, `GOLD_REC`). No test changed; the e2e's 3 tests pass. Only the
  e2e-combat-path fixtures were touched (`prop_worldsim` / `prop_handle`
  copies are `test-support-fold`'s). The e2e's `TestPending` stays in the
  test: it implements `d2-server` traits (`WorldPending`, `Outbox`,
  `UseRest`, `LearnRest`) that `d2-sim` cannot name.
- `wiring::action::tests` is now `cfg(any(test, feature =
  "bench-fixtures"))` (as `worldgen::tests` already was), with
  `allow(unused, dead_code)` outside tests; its test submodules stay
  `cfg(test)`. `bench_fixtures::combat::ActionFx` re-exports its `Fx`.
- **`Fight`** (same file): `Fx` (act 0, level 2, rooms A and B streamed)
  with the moved tables, AnimData (death animation) and experience table;
  N rows (≤ 36) in room B of a player (with a client, so the rooms stay
  active) and a monster 18 sub-tiles away; each row fires an arrow every
  5 frames (staggered) through the real `create_missile`; the missile's
  damage (10 points) is set by the fixture, as in the e2e (the damage
  setup `0x0059F900` is the skills spec's). A monster dies on its 8th hit:
  death mode, kill steps, experience, level-up check. Test
  `fight_kills_every_monster_cleanly_and_repeats`: 36 rows × 200 ticks,
  1440 arrows, every monster dead by frame 60 (last kill on frame 57),
  each player 100 experience, no wiring error, same result twice.
- Bench group `sim_fight` in `crates/d2-sim/benches/sim.rs`:
  `fight_36_rows_60_ticks` (the fight), `fight_36_rows_200_ticks` (plus
  the steady load after it), `fight_setup_only`. Setup is outside the
  timed part (`iter_batched`).

## (b) Live-shaped DC6 / DCC (`test-fixtures`)

`crates/test-fixtures/src/sprites.rs` (new): a DC6 writer (`dc6.md`
encoding: bottom row first, skip / literal runs ≤ 127, row ends) over
generated frames, and a DCC writer that uses every sub-stream of `dcc.md`
(equal cells, pixel masks incl. partial masks that keep earlier colours,
encoding type, raw-pixel codes, displacement-coded codes incl. early stop
and runs of 15, 1- and 2-bit pixel indices, a non-identity `PV`) and runs
the spec's stage 2 on its own choices to know the expected frames. Shapes:
a moving elliptic silhouette with per-pixel colours on a transparent
ground, frame boxes jittered by a few pixels per frame (so the direction
box is larger than each frame and frame cells shift against direction
cells). Tests: the spec's cell-size vectors; both DC6 shapes and both
DCC shapes decode exactly to the written frames (sizes, boxes, every
pixel), every DCC sub-stream non-empty, < 8 PCD leftover bits; a flipped
PCD bit changes the decode (M08). `d2-formats` takes `test-fixtures` as a
dev-dependency (bytes only cross; a dev-dependency cycle Cargo allows).

| Shape | Directions × frames | Frame size | File | Pixels |
|---|---|---|---|---|
| `DC6_PANEL` (UI panel, opaque with windows) | 1 × 4 | 256 × 256 | 222 KB | 262,144 |
| `DC6_SPRITE` (object / overlay) | 8 × 16 | ~96 × 96 | 705 KB | 1,183,747 |
| `DCC_MONSTER` (monster walk) | 8 × 8 | ~70 × 100 | 211 KB | 448,004 |
| `DCC_LARGE` (large player / death) | 16 × 24 | ~110 × 130 | 2.28 MB | 5,490,263 |

## Baseline numbers

Machine: cloud container, 4 vCPU Intel Xeon @ 2.10 GHz, Linux 6.18, rustc
1.99, `bench` profile. Median of criterion's interval, quick settings
(1 s warm-up, 3 s measurement). Order of magnitude.

| Case | Time | Per unit |
|---|---|---|
| fight, 36 rows, 60 ticks (36 kills, ~122 missiles in flight) | 6.45 ms | ~107 µs / tick (0.27% of 40 ms) |
| fight, 36 rows, 200 ticks | 23.3 ms | ~116 µs / tick |
| fight setup (Fx + 72 units + 36 clients) | 145 µs | — |
| (for comparison, same run) idle tick, 24 monsters, 200 ticks | 61.5 µs | ~0.3 µs / tick |
| DC6 panel 1 × 4 × 256² | 17.5 µs | ~15 Gpx/s (mostly literal copies) |
| DC6 sprite 8 × 16 × ~96² | 109 µs | ~10.8 Gpx/s |
| DCC monster 8 × 8 × ~70 × 100 | 5.4 ms | ~83 Mpx/s (~0.68 ms / direction) |
| DCC large 16 × 24 × ~110 × 130 | 67 ms | ~82 Mpx/s (~4.2 ms / direction) |

The old shallow cases ran in the same pass at 12.4 µs (DC6 32 × 64²) and
62 µs (DCC 8 × 32²), as in `bench-baselines`.

## Hot spots (described, not changed)

1. **DCC decode: ~82 Mpx/s, ~25 ns (≈ 50 cycles) per pixel**, 130× slower
   per pixel than DC6. Callgrind on the decode test (release; the
   writer's own share left out): of the decoder's ~820 M instructions
   about 42% are `dcc::Bits::read` (each call bounds-checks and assembles
   5 bytes for a 1–2-bit pixel index or a 4-bit displacement), about 27%
   the per-pixel code → palette mapping collected through `Result`
   (`decode_direction`, "Map codes to palette indices": the map closure
   and the `GenericShunt` collect), about 23% the stage-1/2 loops in
   `decode_direction`, about 7% `copy_rect`. A whole large direction
   costs ~4.2 ms, so a client that decodes on demand (not from a cache)
   spends a quarter of a 60 Hz frame on each new large direction.
   Candidates for a later task: a buffered bit reader (refill a u64, no
   per-call bounds assembly), PV mapping as one plain table pass after a
   single code-range check (no per-pixel `Result`), row-slice copies
   instead of per-pixel iterator chains. Bytes stay identical
   (`mpq-tool formats` is the check).
2. **Fight tick: ~110 µs for ~122 missiles + 72 units** — far inside the
   budget. Callgrind of the 60-tick case: ~17% in the fixture's position
   lookup (`ActionHooks::path_position` → `TestPending::position`, a
   `BTreeMap`), ~8% the fixture's `step`; the missile class handler,
   default flight and hit handler ~6%, 6%, 4%; the room unit-list walk of
   the hit search ~4%; malloc / free ~7%. The position lookups are called
   per unit per missile step (the hit search asks every unit of the room),
   so the cost grows as missiles × units in the room; with the path
   provider enabled (`ActionHooks::enable_paths`) the lookups go to path
   records instead. Not a hot spot at this load; re-measure when the path
   and movement specs land (the fight then pays for real paths).
3. **The corpse keeps its collision bit** (no movement / death-collision
   spec yet), so arrows after the kill still hit it; the 200-tick case is
   a steady 122-missile load rather than a falling one. Fixture behaviour,
   not a claim about the original.

## Open questions

None new for the specs. The fixtures answer the same seams as the e2e
(`e2e-combat-path.md` §4: missile damage setup, path, death start body).

## Local checks

None needed (no game files; the benches and fixtures are synthetic).

## Gate

`sh tools/gate.sh` (fmt, depcheck, clippy workspace `-D warnings`, tests
incl. d2-client, coverage `--check` / `--selftest`, spec index, methods,
doc-tests): results in the session summary / commit message.
