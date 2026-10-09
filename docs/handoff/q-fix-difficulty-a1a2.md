# q-fix-difficulty-a1a2

## Done
- Ran act2.play for druid and assassin on Normal/Nightmare/Hell: 14/15 in all
  6 cells, first blocker `radament-killed` (unit 229 never present).
- Compared Normal vs NM/Hell: identical results in every Acts I-II cell, so no
  difficulty-specific fix was needed. Matrix updated in playability-matrix.md.

## Open
- `radament-killed` blocks every act2 cell (owner: q-fix-boss-damage / q-fix-act2-play).
- act1/act2 cells for the other classes were not re-run on the newest staging.
- No scenario-diff of NM/Hell monster stats against 1.14d was done.

## Repro
    D2_GAME_DIR=/home/user/game python3 tools/playthrough/playthrough.py \
      traces/playthrough/act2.play --class dru,ass --difficulty normal,nightmare,hell --jobs 4 --build
Note: concurrent rustup installs of the pinned toolchain corrupt it; install once first.
