# q-realdata-gate: the real-data gate (M23)

Status: script and inventory built and tested on the no-data and argument
paths only. **The run path has not run on real data**: the private repo's
install was incomplete when this was written (missing `d2char`, `d2data`,
`d2exp`, `d2music`, `d2video`, `Diablo II.exe`; no `extracted/`). First real
run is the first test of the script itself; expect to fix its parsing.

## Command

```sh
sh tools/realdata-gate.sh [--no-client] [--debug] [--no-fetch] [--list] [--help]
```

- `D2_GAME_DIR` set: used as is (must hold `d2data.mpq`, `d2exp.mpq`, `Game.exe`).
- Unset: `$HOME/game` if already assembled; else sparse-clone the private repo
  into `$HOME/d2rust-private-repo` (`$D2_PRIVATE_DIR`), check that every piece the
  manifest names is present, run `tools/assemble.py` (sha256 checked) into
  `$HOME/game`, and link the repo's `extracted/` to `$HOME/game/extracted`
  (several tests read `$D2_GAME_DIR/extracted/patch_d2/...`).
- Release builds by default (`game_sweep` takes ~11 min in debug, ~1 min release).

## What it runs

1. `data-tool tables`, `data-tool links`, `mpq-tool check`, `mpq-tool formats`
   (LOCAL-RUN 1.1-1.4).
2. Per crate that has `#[ignore]` tests (`d2-formats d2-data d2-sim d2-server
   conformance d2s-tool scenario-run seed-finder d2-client`):
   `cargo nextest run -p <crate> --run-ignored only --no-fail-fast` (falls back to
   `cargo test -- --ignored` when nextest is absent). This covers LOCAL-RUN 1.5,
   1.6, 1.8, 1.12, 2.1-2.16 and the other ignored tests the inventory lists.
3. `--no-client` drops `d2-client` (builds Bevy; its ignored tests are mostly
   headless data tests, so prefer to run it).

## What it skips (listed in the summary as needs-window)

- The 6 GPU tests (`gpu_*`), by name: `gpu_compositor_on_real_frames`,
  `gpu_half_matches_cpu_on_every_synthetic_case`, `gpu_half_matches_the_synthetic_capture`,
  `gpu_map_matches_cpu_reference`, `gpu_matches_cpu`, `gpu_perturb_reports_exactly_n`.
- The 2 tests that need `D2_TABLES_DUMP` (a local memory dump):
  `live_tcs_equal_memory_dump`, `memory_dump_perturbation_is_reported`.
- Not run at all: `d2-client play` / `verify` / `gpu_compare` (LOCAL-RUN Batches 3-5),
  recordings (Batch 6), `data-tool dump-compare`, `check_stats.py`, `patch check`
  (needs `game/patch-example/`), `d2s-tool real_saves` (needs `D2_SAVE_DIR`).

## Reading the summary

```
data-tool-tables  PASS
d2-sim            FAIL  110 passed, 1 failed; <log path>
ignored tests: N passed, M failed
needs-window (not run): ...
```

- Exit 0: every step passed. Exit 1: some step failed; the failing tests are
  printed and the full log paths are listed. Exit 2: no usable install or bad
  argument; nothing ran.
- A failure is a finding for the owner spec, not a reason to edit expected values
  (LOCAL-RUN header, rule 10). Known open ones from 2026-10-08 (LOCAL-RUN 1.5, 2.4):
  `ds1_every_file_parses`, `dt1_every_live_file_decodes` (count assertions) and
  `lvlprest_measurements` (80 vs 82).
- "N passed" counts only real-data tests; compare with `docs/handoff/realdata-tests.tsv`
  (`python3 tools/realdata_inventory.py` regenerates it): 245 rows = 239 `ignored`
  (231 run by the gate, 6 GPU and 2 dump skipped) + 6 `env` rows (non-ignored tests
  that read `D2_GAME_DIR`; the gate does not run them). The `files` column is a
  heuristic from names in the test body, not a guarantee.
