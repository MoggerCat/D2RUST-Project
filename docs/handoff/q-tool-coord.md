# q-tool-coord: hand-back (2026-10-09)

Branch `claude/q-tool-coord` (staging f17073a9 merged in). Tools for the
multi-session build loop, all under `tools/coord/` (usage:
`tools/coord/README.md`). Every tool has a `--selftest`, and all four pass.

## Done
- `sync.sh` (+ `sync-selftest.sh`, `union.py`): every session runs
  `sh tools/coord/sync.sh` every 20–30 min. It merges staging (never
  rebases) and auto-resolves three kinds of conflict:
  - spec `<!-- index -->` tables, then regenerates the index;
  - `provisional-index*.tsv`, which it regenerates;
  - append-only HANDOFF.md / build-queue.tsv / pc1-data.md (union).

  Any other conflict: exit 1 with the merge left open and the files listed.
  It refuses to commit while conflict markers are left. The selftest runs
  11 cases on toy repos.
- `route.py`: takes a first-difference report from state/packets/rng/draws or
  a playthrough blocker, as text or JSON. It prints the spec section, the
  crate modules and the owner.
  - Owners come from `owners.tsv`, the coordinator's list of 14:28 UTC.
  - A message can have its own row (`client-messages.tsv#0xNN`): C2S 0x01 and
    0x05–0x12 go to skills-2.
  - Exit 1 when anything is unrouted.
- `playtable.py`: `playthrough.py --all --json` → `docs/handoff/playability.md`.
  It writes the per-act table, the head and time, milestones gained and lost,
  and a 30-row history. Exit 1 when a milestone was lost. Only the selftest
  has run it so far: no real playthrough run yet.
- `realdata.py`: runs the game-file tests and compares them with
  `realdata-baseline.tsv`. It fails only on a new failure, and supports
  `-E` filters and `--update-baseline`. Its selftest passes, but **the
  baseline file does not exist yet** (see below).

## Found
- On staging 1297820b, rustc 1.99 **segfaults** compiling the d2-sim lib test
  crate at opt-level 3. LLVM's full-unroll pass recurses without end, and
  RUST_MIN_STACK up to 256 MB doesn't help. So `--release` real-data runs of
  d2-sim fail to build, `tools/realdata-gate.sh` included. opt-level 2 builds
  fine. `realdata.py` therefore builds with
  `--config profile.release.package.d2-sim.opt-level=2` in its own target dir
  (`target/coord-realdata`), so it doesn't fight `playthrough.py --build` over
  the opt-level-3 build.
- Disk: the full release test build of d2-client + d2-server + d2-sim +
  test-fixtures takes more than 11 GB. Together with a second target dir it
  filled a cloud session's disk allowance, and the linker died with SIGBUS.
  Keep only one release target.

## Next steps
1. Generate the baseline on a staging head (one cloud session, about 35 min
   build on 4 cores, at least 15 GB free):
   `D2_GAME_DIR=~/game python3 tools/coord/realdata.py --update-baseline`
   Commit `tools/coord/realdata-baseline.tsv` on its own and record the wall
   time in the README.
2. Run `python3 tools/coord/playtable.py` once on staging and commit
   `docs/handoff/playability.md`.
3. Keep `owners.tsv` current as sessions change.
4. Setup a fresh session needs: `cargo install cargo-nextest --locked` (the
   get.nexte.st download is blocked by the proxy) and Bevy's libraries
   (`tools/cloud-setup.sh`'s apt line). Assemble the install with
   `python3 <private>/tools/assemble.py ~/game` and the excel view with
   `cargo run --release -p data-tool -- excel-dir`, as in
   `tools/realdata-gate.sh`.
