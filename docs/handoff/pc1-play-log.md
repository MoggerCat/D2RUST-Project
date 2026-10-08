# PC 1 play-test log (`claude/local-pc1-play`)

One line per push: SHA, what broke in `d2-client play` on the real 1.14d
files, what was fixed. Newest last.

- 4dac88b8 — live play panicked ("drawn area leaves the gradient block"), exit panicked (WorldViewState gone), 0x23 SetSkill 65535 refused (empty StartSkill), every 0x96 refused (no visibility input) → floor block draws clipped to the gradient rows, exit tolerant, empty StartSkill = none, preview select of no skill sends nothing.
- 875b61c6 — running rubber-banded / player off screen → run stat list (0x900), mode change frees temporary lists, local walk cell in the position check (later replaced by staging's version).
- f656b1a6 (merged) — no warp tiles on live levels (no cave / dungeon / tower entrances) → q-fix-warp-tile-unit, q-fix-warp-tile-restore; live test walks Rogue Encampment → Den of Evil → back.
- 429cc756 (merged 60070f4c) — act change / portal pair / TP item use d2rs-own → q-fix-act-change, q-fix-portal-pair, q-fix-tp-use per spec.
- 7ce313b5 — front end panicked on close ('non-send resource does not exist'), choice lost → outcome kept outside the app; flaky world_data_tables temp dir; coverage claim lines fixed.
- a0828325 — menus wrong vs 1.14d screenshots (one background tile, stacked button tiles, wrong logo / short button files) → full tile grids, tiles side by side, frame offsets, real file names; new ignored test game_front_end_shots.
- 023c800c — starting the game from the menu panicked ('Failed to build event loop: RecreationAttempt') → `play` launches the menu and the game as child processes.
- b54d2128, 0f3883c3, eba3fb57 — wrong strings (NPC lines on menus), wrong create-screen palette, missing heroes, label / text layout → string id lookup per ui/text.md §2 r2, index-space compose with the screen palettes, text-control layout, create screen heroes / fire / title, trademark screen. Headless shots match the 1.14d screenshots except character select class portraits.

Open: character select class portraits; intro cinematics (no video playback);
gate on this PC: d2-client release test crates hit a rustc STATUS_STACK_BUFFER_OVERRUN
(retry with RUST_MIN_STACK), d2-sim `missiles::tests::cov_text` fails on Windows paths only.

Not merged at hand-off: origin/claude/specs-staging-7 (2026-10-08 evening) conflicts with this branch in crates/d2-sim/src/wiring/action/inactive.rs (warp-tile restore vs staging), specs/client/model.md and specs/sim/pathing.md (index blocks); the merge was aborted so the coordinator resolves it.
