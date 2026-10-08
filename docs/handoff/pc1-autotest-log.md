# PC 1 autotest log (newest first)

Loop: `docs/handoff/pc1-autotest.md`. Branch `claude/local-pc1-test`.

Hardware (2026-10-08): Intel i7-7700HQ, 4 cores / 8 threads, 15.9 GB RAM,
NVIDIA GeForce GTX 1050 (plus Intel HD Graphics 630), C: 20 GB free after
removing `target/debug`. Slots: build jobs 6 with no game / 4 with the game
running; CPU checks 2-4 in parallel; GPU 1; game 1; subagents up to 6.

Start state: `origin/claude/specs-staging-7` merged with
`origin/claude/local-pc1-s8` (its Lane B rounds 1-2 already ran LOCAL-RUN
§0.1, Batches 1-5 and the HANDOFF §5 game-file entries on this PC; see the
two 2026-10-08 Done blocks in HANDOFF §5), so Lane A starts from that
recorded state and reruns only what changed or failed.

## Round 1 (2026-10-08) — stopped by the user after setup

The user chose to run the tests themselves; the loop stopped here.

- Built: release d2-client, scenario-run, data-tool, mpq-tool, d2s-tool
  (4 min 28 s). The test-binary build was stopped part-way.
- `autostart.py --selftest` ok; `--try ScnAma --seed 1234`: player in level 1
  at 6.4 s, act init seed 1234 (characters made with `d2s-tool new`, copied
  read-only, removed again after the session).
- Lane E smoke: `d2-client play --frames 600` on live data still panics
  (exit 101, draw item 653 leaves the gradient block) = existing row
  `q-fix-play-gradient-block`.
- Lane B group 1.5 run A (`record_packets.py`, TestSor seed 644409375, Act
  I-IV waypoint route): only 525 server ticks in 200 s with a cargo build
  running beside the game; the player clicked the waypoint but never left
  Act I. Not a result. Lesson: run game recordings with no build beside
  them and with `--seconds` sized for the hooked game speed (~2.6 ticks/s
  here). Raw file `traces/raw/pc1t-r05a-packets.jsonl` (gitignored).
- Plans written in the session scratchpad (not in git): Lane A (113 merged
  q-notes still without a local check; ~27 headless test targets; new
  `--ignored` tests C13, C20 and others), Lane B (7 priority-1 groups, 11
  priority-2 groups, ~27 missing recorder hooks, 15 characters to make).
