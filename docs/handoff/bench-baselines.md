# Handoff: performance baselines (criterion) — `claude/bench-baselines`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Cloud tooling session, 2026-10-06, medium. Repo only, synthetic fixtures,
no game files. The game ticks at 25 Hz: the server has 40 ms per tick.

## How to run

```
cargo bench -p d2-sim --bench sim          # tick, DRLG, items, treasure, stat lists
cargo bench -p d2-formats --bench formats  # MPQ, DC6, DCC
cargo bench -p d2-proto --bench proto      # classify / split / decode
cargo bench -p d2-client --bench compose   # CPU compositor (builds Bevy: slow, big disk)
# quick pass: append  -- --warm-up-time 1 --measurement-time 2
```

Not run in CI. `cargo clippy --workspace --all-targets` compiles all four
(`cargo bench --no-run` too). Criterion is a workspace dev-dependency
(`default-features = false`: no plots, no rayon).

## Fixtures (what the benches reuse)

- `d2-sim` feature `bench-fixtures` (enabled for the benches by a self
  dev-dependency; off in every normal build) widens the `cfg(test)` of the
  worldgen test fixtures (`wiring/worldgen/mod.rs`: `mod tests`),
  `stats::tests`, `skills::fake`, `StateTable::synthetic` and `items::tests`
  to `cfg(any(test, feature = "bench-fixtures"))`, with
  `allow(unused, dead_code)` outside tests, and `d2_sim::bench_fixtures`
  re-exports what the benches need. These are the only source changes kept,
  and they are needed: the wired-sim, DRLG, item and stat-list fixtures
  exist nowhere else (`crates/test-fixtures` holds data tables, not a
  wired sim). No behaviour change.
- MPQ: the bench uses the base's `d2_formats::mpq::writer` (`test-support`
  feature, self dev-dependency); it writes PKWARE, so the Huffman path is
  not benched. (An earlier version of this branch had its own fixture
  writer; dropped on the merge with `claude/tender-meitner-mphas3`.)
- DC6, DCC, treasure, and the compositor scene are built inside the bench
  files from the specs' layouts.

## Baseline (first numbers)

Machine: cloud container, 4 vCPU Intel Xeon @ 2.10 GHz, Linux 6.18, rustc
1.99, `bench` profile (opt-level 3). Median of criterion's interval, quick
settings (1 s warm-up, 2–3 s measurement). Treat as order of magnitude.

| Case | Time | Per unit |
|---|---|---|
| sim tick: 24-monster level, 200 ticks (each monster thinks once) | 46 µs | ~0.23 µs / tick |
| populated level setup (create + generate + stream + populate, 24 monsters) | 954 µs | — |
| DRLG: create act 0 | 509 µs | — |
| DRLG: act 0 + generate the 40 × 18 preset level | 528 µs | ~20 µs for the level |
| `create_item` (normal armor / helm with sockets) × 100 | 21.5 µs | ~215 ns / item |
| treasure `walk` × 1000 (one TC, 16 items, 4 picks) | 331 µs | ~330 ns / drop |
| stat lists: player + 12 item lists attached, 40 total / recompute, detach | 36.7 µs | — |
| MPQ open (2 files) | 9.6 µs | — |
| MPQ read 256 KiB, PKWARE + encrypted 512-byte sectors | 936 µs | 267 MiB/s |
| MPQ read 64 KiB plain | 3.9 µs | 15.6 GiB/s |
| DC6 parse, 32 frames of 64 × 64 | 17.8 µs | ~0.55 µs / frame |
| DCC parse, 8 directions of 32 × 32 | 70 µs | ~8.8 µs / direction |
| proto: classify one C→S message per fixed-size game id | 606 ns | ~15 ns / message |
| proto: split a 500-message S→C buffer | 3.0 µs | ~6 ns / message |
| proto: decode + encode `Walk` and `SelectSkill` | 16 ns | — |
| compositor `order`, 400 items | 10 µs | — |
| compositor `compose`, 800 × 600, 400 items (shade + blend) | 4.75 ms | 101 Mpx/s |
| compositor `bin` | 50 µs | — |
| compositor `compose_binned` (the GPU's walk, on CPU) | 10.7 ms | 45 Mpx/s |
| `to_rgba`, 800 × 600 | 0.73 ms | 655 Mpx/s |

## Hot spots and caveats

1. **MPQ Huffman sectors: ~3 MiB/s (measured once, not kept).** An
   earlier version of this branch benched a Huffman-compressed 256 KiB
   file: 78 ms, ~150 µs per 512-byte sector, ~80× slower than PKWARE
   (267 MiB/s, table above). The likely cause is the adaptive tree being
   rebuilt per sector. The merged base's `mpq::writer` has no Huffman
   encoder, so the committed bench no longer covers it; a Huffman case
   returns when a writer for it exists. Not changed here.
2. **CPU compositor `compose`: 4.75 ms/frame** is 12% of a 40 ms tick if
   it ever ran in the loop; it is the reference (GPU is the real path),
   so only relevant for verify runs. `compose_binned` is 2.2× slower than
   `compose`, as expected for the per-pixel list walk.
3. **The sim numbers are floor values, not a load test.** The fixtures
   are tiny (one 40 × 18 preset level, an idle monster think every 200
   frames, no combat, missiles, pathing or clients): the tick is ~0.2 µs
   when idle, DRLG of the preset level ~20 µs. Real outdoor / maze levels
   and a 24-monster fight will cost orders of magnitude more. A loaded
   tick bench needs the combat / missile fixtures of
   `e2e-combat-path` (those live in `d2-client/tests`, not reusable) —
   next step: move them behind `bench-fixtures` and add a "fight"
   tick case.
4. DCC decode is shallow here (every pixel colour 0, frames in one cell
   column); real DCC with many frames and cells will be heavier.
5. No optimisation was made. The only non-bench edits are the
   `bench-fixtures` cfg widenings listed above (behaviour-neutral).

## Gate

`cargo fmt --check`; `cargo clippy --workspace --all-targets -- -D
warnings`; `cargo test --workspace`; `cargo run -p depcheck`;
`python3 tools/spec_index.py --check`; `python3 tools/methods.py check`;
`python3 tools/coverage.py --check` and `--selftest` (results in the
commit message / session summary).
