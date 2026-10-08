# Build loop: shared instructions for stitching sessions

The coordinator starts each session with a short prompt that names its task (`<task>`, for example `q-strings`). Everything else is here.

You are a STITCHING session for MoggerCat/D2RUST-Project's playable Bevy build. The game rules are written and tested; your job is the glue that makes one feature work in the running `play` preview. About 60 minutes, no subagents; keep it cheap with small, focused changes. Push only to claude/<task>; it already exists, so pushes are fast-forwards. If a push fails with "Internal Server Error", retry with a 20 s backoff, up to 10 times. Commit and push every 15 minutes.

Setup, started in the background at once: `sudo apt-get install -y pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev; sh tools/hooks/install.sh; CARGO_INCREMENTAL=0 cargo build -p d2-client --tests &`. If the pinned toolchain won't install, use RUSTUP_TOOLCHAIN=stable.

Read: CLAUDE.md; docs/METHODS.md M22 "Provisional over blocked"; the "First playable preview (D1–D3)" decision in docs/PLAN.md; docs/local/2026-10-07/PLAYABLE.md (the user's real run); and the docs/handoff/play-*.md and stitch-*.md notes that touch your area. Read only specs/, docs/, crates/ and tools/ (never re/ or ../refs/). No Blizzard files; tests use the synthetic fixtures. The server is authoritative (the client sends intents and draws state); game logic stays out of Bevy; d2-sim stays deterministic. Preview-only fills are marked `// d2rs-own, unverified`. Spec gaps follow M22: PROVISIONAL, with a new REC id in docs/HANDOFF.md §7. Parallel sessions collide on REC ids: right before your final push, `git fetch origin claude/specs-staging-7`, take the highest REC-N in that branch's docs/HANDOFF.md plus one (never a made-up form like REC-XYZ-1), and renumber yours to that. Reuse existing systems in staging; never add a second copy of one (for example, the player skill list is d2-sim `skill_lists`, and unit facts come from `live_facts`/`refresh_targets`). Sound is deferred by the user: don't wire audio.

Other sessions run in parallel on other features. Stay in your area. Put new code in new modules you create. In shared files (app/play.rs, app/ui.rs, bridge/mod.rs, world_view/mod.rs, single_player.rs, ui/original.rs, present.rs) make only small, additive edits: one registration or call line each, so merges stay clean.

YOUR FEATURE: the row of docs/handoff/build-queue.tsv whose first column is your task name (the branch name without `claude/`).

Method:
1. Trace the real path end to end (input → bridge intent → server handler → sim → S→C messages → client model → UI / world view → draw) and list every missing link with file:line.
2. Connect the links in order, each with a synthetic end-to-end test that fails before your change.
3. If you finish early, stop. Don't start other features.

Tests: never weaken an existing assertion to make it pass (for example turning `dropped.is_empty()` into "expect this message dropped"). If your change breaks an assertion, fix the cause; change an expectation only when the spec says the new value, and say why in the commit.

Before each push: `cargo fmt --all`, `cargo clippy -p <crates touched> --all-targets -- -D warnings`, `cargo nextest run -p d2-sim -p d2-server -p d2-client --no-fail-fast` (all three, whatever you touched; install with `cargo install cargo-nextest --locked` if missing) (nextest, not `cargo test`: some d2-server tests need per-process temp dirs), and `python3 tools/coverage.py --check`.

Every commit message ends with:
Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
Claude-Session: <your session URL>

Write docs/handoff/<task>.md (links connected, PROVISIONAL points, what's left, the user's local check: exact commands and what they should see), push, then send_message to session_01HvRxiwWBvDAgp82ZPnvPPR with the head SHA and a 4-line summary. Then stop.

