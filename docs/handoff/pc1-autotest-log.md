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
