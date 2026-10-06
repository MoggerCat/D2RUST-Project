# local-buddy a-client (2026-10-07)

Worktree base: origin/main e5e7c94. Debug profile, D2_GAME_DIR = original 1.14d install. Logs: out-a-client\ (outside repo).

| Check | Expected | Actual | Result | vs 2026-10-06 |
|---|---|---|---|---|
| mutants-client section 4: `cargo test -p d2-client --test mutants_client -- --ignored` | `assets_cache::archive_set_reads_files_it_holds` passes (pal.dat 768 bytes, missing name None) | 1 passed, 0 failed (0.08 s) | PASS | new (not run yesterday) |
| 2.15: `frames::tests::all_live_frame_sets_build_and_pack -- --ignored --nocapture` | C8: 23,595 files, 6 parse errors, 288,702 frame sets, 3,345,171 frames, largest 96x960 <= 2046 | 23595 files (6 parse errors), 288702 frame sets, 3345171 frames; largest 96x960 (data\global\tiles\expansion\baallair\worldstone.dt1 tile 50); ok in 681.58 s | PASS | same numbers; 685 s -> 682 s |

Claim lines: the mutants_client test carries no "Claim once the first local run passes" / "Intended claim" line (the unconfirmed "Intended claim" lines are in tests/game_assets.rs, not in this task's tests), so no `// Covers:` conversion was made.
No findings.
