# Handoff: the server survives any client bytes — `claude/fuzz-server`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Cloud implementation session, 2026-10-06, task class: property tests and
crash fixes from clear specs, medium (METHODS M14). Base: `main` at
`edad871`. Repo only (no `game/`, `re/`, `../refs/`). Scope of every claim:
this branch, synthetic hosts, `cargo test` in a debug build (overflow
checks on).

## 1. State

Property tests (proptest, the same `config(n)` / `PROPTEST_CASES` pattern
as the parser robustness tests in `d2-formats`) over every place client
bytes enter the server, and over the bridge's link to it. Properties: no
panic, abort, debug overflow or unbounded allocation; the result code the
spec gives for invalid input; the state unchanged where the spec says a
rejection changes nothing.

| File | What it drives | Properties |
|---|---|---|
| `crates/d2-proto/tests/prop_messages.rs` | C→S `client_size`, `classify_client`; S→C `server_size`, `split_server_buffer`; the typed decode/encode of all 91 C→S and 24 S→C `FixedMessage` types | sizes bounded by their rule; a prefix's non-`Incomplete` result equals the full buffer's (rules read only their own fields); classifier result per `intents-events.md` §2.1 rule 4 on every truncation; split = messages + discarded tail, errors name an over-0x204 or past-the-end message (§3.3); decode Ok exactly for `SIZE` bytes with `ID`, error = first failing check, `decode(write(decode(b))) == decode(b)` |
| `crates/d2-server/tests/prop_transport.rs` | `transport::classify` on `ProtoSizes`, `ServerQueues` send/drain, `DuplicateFilter`, `ClientBuffers`, `Inbox` deliver/push, `dispatch::dispatch`, `process_game_message`, `Host::frame` (fake `Intents`) | server and `d2-proto` classifiers agree (gate open/closed); drain order system → game → admin, FIFO, 0x1FC copy with full size (§2.1 r7); filter drops only a repeat inside its window (§2.1 r1); buffers ≤ 0x200, whole messages, in order (§3.2); delivery conserves bytes, lists by id range (§3.3); **dispatch code = an independent model of §2.3 / §2.4 r1–4, r6 built from the TSV columns**, handler called exactly when everything passed, point accept recorded only then; §2.2 drop / fatal-assert / dispatch by lookup and record |
| `crates/d2-server/tests/prop_handle.rs` | `SimGame::handle` through `dispatch` + one tick per message on two wired hosts (below), every game id 0x01–0x66, plus the handled ids alone | no panic; stub → 0; wrong fixed size → 3, unit type ≥ 6 → 2, point target out of range → 1 (§2.4 r1, r3, r4); closed dead gate → 0 (§2.3 r3); the dispatcher's own rejections leave the game state (game, unit records, stat lists, item store, staged inventories) byte-identical in `Debug`; no handler or tick records a fatal path (`world.faults`, `sys.errors`, `items.errors`) |
| `crates/d2-client/tests/prop_bridge.rs` | `Bridge::send_bytes`, `intent::route`, `LocalLink` send/pump/receive, `Bridge::receive_chunk` (handlers), direct sends | the bridge refuses exactly what §4 r3 says; what it routes the link queues or filters, never errs; the server drains every queued message with its id, size and (0x1FC-capped) bytes in drain order; any S→C chunk is dispatched or refused whole with the split's error; every S→C id with arbitrary fields through the handlers |

Hosts of `prop_handle.rs`, as the server's handler tests build them:
- **action host**: `SimGame<ActionSim<_>, ActionWorld>` with `WiredSkills`
  (4-skill synthetic table, `vitals.md` charstats), waypoint tables, a
  player (any class 0–6, 5 stat and 5 skill points), a monster, a ground
  item and a waypoint object; `NoLevelTypes` DRLG.
- **item host**: `SimGame` with an `ItemWorld` (cube recipe ring →
  amulet), a player with the cube stored, a ring in any mode 0–5.

Field values: half from the field's own domain by layout name (x/y within
50 of the player, unit types 0–5, the host's GUIDs or −1, skill ids 0–4,
stat ids 0–7, small pages/body locations/slots), half from edge values
(0, 1, 6, 7, 0xFF, 0xFFFF, 0x7FFF, 0x8000, 0x7FFF_FFFF, 0x8000_0000,
u32::MAX, 49–51, 150–151, 358–360, the GUIDs, random); sizes exact (80%),
one short or one long. A spot check of 400 messages per handled id showed
every handled id reaching its handler with 0 and with refusals (0x2A on
the item host mostly 1: the generated ring is rarely in a mode the cube
takes).

Default case counts and debug-build runtimes: `prop_messages` 256/test
(0.2 s), `prop_transport` 256 and 64 (1.3 s), `prop_handle` 48/48/96
(≈ 1 s), `prop_bridge` 128 (≈ 0.5 s). Hunts run with `PROPTEST_CASES`:
20,000 (`prop_messages`), 5,000 (`prop_transport`), 3,000 (`prop_handle`,
`prop_bridge`): no failure left.

## 2. Bugs found

| # | Input | Failure | Fix | File |
|---|---|---|---|---|
| 1 | `Bridge::send_bytes` of a system message longer than 0x204 bytes whose size rule fits (e.g. 0x67 followed by 516 zero bytes: rule 46 ≤ 517, so the classifier queues it, §2.1 r4 "whole buffer") | the bridge routed it; `LocalLink::send` then failed with `SendError::TooLarge` (the transport's ≤ 0x204 assert, §2.1 r3), a link error that ends the bridge frame. `bridge.md` §4 r3: "The bridge never sends a message 1.14d's transport would drop or assert on." | `intent::route` refuses a message over 0x204 bytes with a new `IntentError::TooLarge(len)`, after the game-sender check (game messages keep `GameTooLarge`, which comes first in 1.14d) | `crates/d2-client/src/bridge/intent.rs` |

Regression: `route_every_id_and_length` (`prop_bridge.rs`, plain
`#[test]`, every id × lengths 1..=0x205) fails without the fix (`Ok(System)`
vs `Err(TooLarge(517))`) and passes with it (M08).

Nothing else failed: no panic, overflow, abort or recorded fatal path in
`d2-proto`, the `d2-server` transport, dispatcher and host, the wired skill,
waypoint and cube handlers, or the bridge's receive path.

Facts the properties had to learn (not bugs; for whoever writes the next
ones):
- An S→C size rule can read past the size it gives: 0x16 (`u16@1`, min 13)
  and 0xAC (`u8@12`, min 13) size a message shorter than the 13 bytes the
  rule reads. A split message re-evaluated alone is then `Incomplete`;
  evaluate it where the split did (on the rest of the buffer).
- A handler's refusal can follow state changes its spec orders first:
  `cube.md` §2 step 3.1 records a targeting reset before 0x2A returns 3.
  "State unchanged" is checked only for the dispatcher's own rejections.
- The host's first frame starts the tick driver without a tick
  (`tick.md` §1 r2).

## 3. Signature changes

- `d2_client::bridge::intent::IntentError` gains the variant
  `TooLarge(usize)`. No exhaustive `match` on it exists in the workspace
  (grep, 2026-10-06).

## 4. Fixes outside the owned files

- `crates/d2-client/src/bridge/intent.rs`: bug 1 (10 lines).
- `Cargo.toml` dev-dependencies: `proptest.workspace = true` in
  `d2-proto` (new `[dev-dependencies]` section), `d2-server`, `d2-client`
  (one line each in the existing section); `Cargo.lock` follows.

## 5. Not covered, questions

1. **`d2-net`** has no code yet (`lib.rs` is a doc comment): no
   `crates/d2-net/tests/prop_*.rs`. Its transport needs the same
   properties when Phase 7 writes it (framing, length prefix).
2. **`TradeWorld`** (NPC and vendor handlers: 0x13, 0x2F–0x38, 0x62) and
   **`WorldSim`** around `ActionSim` are not fuzzed: on `ActionWorld` those
   ids are stubs. The fixture is `crates/d2-client/tests/e2e_vendor.rs`'s
   rests (~800 lines); a follow-up can lift it into a shared test module.
   These handlers take GUIDs, item ids and costs from the client, so they
   are the next place to look.
3. Question for `bridge.md` §4 r3: the rule covers the 0x204 case only
   through "never sends a message the transport would assert on". Should
   it name it (and the error) next to the 0x200 game-sender case?
4. Not client-reachable, not changed: `dispatch::in_range` subtracts
   `i32` positions without wrapping; positions come from the server's
   staged `UnitFacts`, never from client bytes (targets are u16).
5. The queues take any number of messages per frame (the original's
   linked lists are unbounded too). A remote client could grow them
   without limit; that is network-mode policy (Phase 7, out of scope).

No `// Covers:` claims on the property tests (no precedent for claims
inside `proptest!`, and the coverage tool does not parse the macro); one
claim on the plain test `route_every_id_and_length`
(`bridge.md` §4 r2, §4 r3).

## 6. Gate

All passed on this branch, 2026-10-06:
- `cargo fmt --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace`: every suite ok (0 failed; ignored = game-file tests).
- `cargo run -p depcheck`: OK (8 crates).
- `python3 tools/spec_index.py --check`: OK.
- `python3 tools/methods.py check`: 21 methods OK.
- `python3 tools/coverage.py --check`: 3197 claims, 0 errors; `--selftest`: ok.
- Final hunts on the committed tests: `prop_handle` and `prop_bridge` at
  `PROPTEST_CASES=3000`, `prop_transport` at 5000, `prop_messages` at
  20000: all pass.
