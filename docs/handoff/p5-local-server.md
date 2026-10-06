# Handoff: in-process server behind the bridge (branch `claude/p5-local-server`, 2026-10-06)

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Scope: PLAN Phase 5 "In-process server running d2-sim" + bridge wiring
(`specs/client/bridge.md` §3, open question 1). Base: `claude/bold-ptolemy-jvyvxy`
at `ac01471`. Implementation session; repo only.

## 1. What changed

- `crates/d2-client/src/bridge/local.rs` (new): `LocalLink<G, S, H, C>`,
  the `ServerLink` on `d2_server::host::Host` with the §3 rule 1 mapping:
  `send(Game)` → `Host::send_game` (duplicate filter → `Sent::Filtered`),
  `send(System)` → `Host::send_system`, `pump` → `Host::frame` (drain →
  tick driver → flush; the host's `FrameReport` is kept as
  `last_frame()`), `receive` → `Host::receive(LOCAL_CLIENT)`.
  `protocol_version` = `d2_proto::PROTOCOL_VERSION`. `new` connects
  client 0. The clock is the host's injected `Clock` (§3 rule 3);
  `SinglePlayer<G>` = `ProtoSizes` + `SystemClock`, built with
  `LocalLink::single_player(game)`. Errors (`SendError`, `HostError`, a
  server-classifier drop the bridge classifier passed) become
  `LinkError::Server`. Nothing is persisted; no message is interpreted.
- `PendingSession`: the `SessionHandler` until the session spec exists
  (system messages 0x67..=0x70): records each drained message, answers
  nothing.
- `crates/d2-client/src/bridge/local_tests.rs` (new): headless end-to-end
  on the real host, `ProtoSizes` and `SimGame<ActionSim, ActionWorld>`
  (two-act synthetic DRLG, seed 1234, a sorceress at (42, 20) beside a
  Cold Plains waypoint; fixture shape of the server's waypoint tests).
  Tests: C→S 0x49 TakeOrCloseWp → drained, dispatched, result 0, tick,
  flush → exact S→C `0D 00 <guid> 01 2D00 1700 0000` reaching the bridge
  → a synthetic 0x0D handler adds unit (0, guid) to `ClientWorld`
  (`frames` 2, `server_ticks` 1); same with the spec table (0x0D
  unowned, counted); the link's duplicate filter; protocol version check
  (wrapper reporting version + 1 refused, real link accepted); unknown /
  unowned ids (C→S 0x80 refused before the host; stub 0x3F recorded in
  `SimGame::unhandled`; 0x6B to the session handler; S→C 0x1A direct send
  unowned).
- `crates/d2-client/Cargo.toml`: `d2-server` dependency (depcheck allows
  client → server, never the reverse); dev-dependencies `d2-sim`,
  `d2-data` for the fixture.
- `bridge/mod.rs`: `pub mod local`, test module, re-exports
  `LocalLink`, `SinglePlayer`.
- No d2-server changes (its public API sufficed).

## 2. Gate (all pass)

`cargo fmt --all -- --check`; `cargo clippy -p d2-client -p d2-server
--all-targets -- -D warnings`; `cargo test -p d2-client -p d2-server`;
`cargo run -p depcheck`; `python3 tools/spec_index.py --check`;
`python3 tools/methods.py check`; `python3 tools/coverage.py --check`.

## 3. Notes for the fold-in (docs/HANDOFF.md, docs/PLAN.md, spec)

- `bridge.md` open question 1 can be closed: the adapter exists
  (`bridge::local`). The spec text in §3 rule 1 matches the code.
- Bevy: `BridgeResource` holds `Box<dyn ServerLink + Send + Sync>`.
  Whether a given `SimGame<D, W>` is `Send + Sync` depends on its
  providers (e.g. `DrlgWorld`'s boxed `TileSource` / `LevelTypes`); the
  app-side construction of the real game is not wired here (no app code
  changed). Check when the app builds its single-player game.
- `PendingSession` is a placeholder for the session code (game create /
  join / leave, Phase 5 session spec); join is done today with
  `SimGame::join` before the link is made.
- No game-file check needed (synthetic data only).
