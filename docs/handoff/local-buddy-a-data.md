# Local buddy a-data results (2026-10-07)

Branch `claude/local-buddy-a-data-2026-10-07`, from origin/main e5e7c94. Game dir: the 1.14d install.

## 1.5 `cargo test -p d2-data -p d2-formats -- --ignored`

- Expected: 53 pass, 5 fail (all in game_sweep, finding G1) per the previous run on main 0472619.
- Actual: 54 pass, 5 fail. Result: FAIL (five failures, all in `game_sweep`, same count as before; one more passing test than yesterday).
- Failures (all `crates/d2-formats/tests/game_sweep.rs`):
  - `cof_every_live_file_parses` line 87: `data\global\chars\am\cof\amblxbw.cof: file not found: data\global\chars\am\cof\amblxbw.cof`
  - `string_tables_every_key_resolves` line 413: left 29, right 33
  - `ds1_every_file_parses` line 255: left 2372, right 2456
  - `dc6_every_file_decodes` line 142: left 1653, right 1657
  - `dt1_every_live_file_decodes` line 227: left 250, right 254
- The dc6/dt1/ds1/tbl failures are the expected-count inflation (finding G1; actual is lower than expected by 4, 4, 84, 4). The cof failure is a name that is listed but does not read. Whether these are the identical five as yesterday cannot be confirmed from a stored list; the failing set matches the G1 description (count mismatches plus the case-sensitive name set).
- Other suites in the run: 11, 15, 4, 5, 1, 1, 11 passed, 0 failed; game_sweep 6 passed, 5 failed.

## patch_gaps_game (gaps-numbered queue item 1)

- Command: `cargo test -p d2-data --test patch_gaps_game -- --ignored`
- Expected: 1 test, `edge_duplicate_columns_in_1_14d_headers`, passes.
- Actual: `1 passed; 0 failed`. PASS.
- Added `// Covers: specs/data/patch-layers.md §edge-cases-original-bugs r1` above the test (replacing its "No claim yet" doc line).
- `py tools/coverage.py --check`: `coverage: 4241 claims, 0 errors`. PASS.
- `py tools/coverage.py --summary`: total 3339 rules, 2895 game (86.7%), 198 verified-only 5.9%; verified (game-file or trace) 228/3339 rules (6.8%). Before the change: 4240 claims, verified 227.

## DC6 scan (gaps-numbered queue item 2)

- Method: throwaway program outside the repo (`C:\Users\zffit\Desktop\D2test\scratch-dc6`, depends on d2-formats by path, not committed). It reads every listed `*.dc6` of every archive in the game dir and checks header version and flags.
- Expected: no file with version != 6 or flags bit 2.
- Actual: 1651 archive entries (1633 distinct lowercase names), 0 unreadable, 0 short. Versions: {6: 1651}. Flags: {1: 1651}. Hits: 0. PASS.
- Note: game_sweep counts 1653 files with its own dedup and the test expects 1657; the scan counts every listed entry per archive, so the totals differ slightly. Also the `dc6_every_file_decodes` loop asserts version == 6 for each file before its count assert, and it passed that for all of them.
