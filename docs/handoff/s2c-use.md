# Handoff: use the S→C builders (HANDOFF §2 step 7k) — `claude/s2c-use`

> Folded into `docs/HANDOFF.md` (§1–§5, §7, §8) and `docs/PLAN.md` as of the eighth fold (`claude/docs-fold-8`); this file stays as the detailed record.

Cloud implementation session, 2026-10-06, medium (METHODS M14). Branch
`claude/s2c-use` from `claude/tender-meitner-mphas3` at `01dff69`. Repo
only, no game files. Scope of every claim: this branch, synthetic inputs
(M09).

## 1. Blocker: `d2-sim` → `d2-proto` is forbidden, not allowed

The task (and HANDOFF step 7k, `s2c-builders.md` §5 item 3) says
depcheck allows `d2-sim` → `d2-proto`. It does not:
`tools/depcheck/src/main.rs` `FORBIDDEN` has `("d2-sim", "d2-proto")`
under "The sim does no I/O and knows nothing about transport or the
client", there since Phase 0 (`4d74928`). `wire-path-server.md` also
relies on it ("d2-sim does not depend on `d2-proto`"). Lifting a
layering rule is an architecture decision (`CLAUDE.md`: read
`ARCHITECTURE.md` / `EARLY_DECISIONS.md` first), not something an
implementation session does on its own, so **the dependency was not
added and the sim builders were not replaced**. M09 escape: an unscoped
"depcheck allows it" crossed two handoffs unchecked.

Decision needed (coordinator / user), options:

- **A. Keep the rule** (what this branch does): `d2-sim` keeps its byte
  builders; the one-maker property is a test in a crate that may depend
  on both (§2). Cost: two makers in code, tied by a check.
- **B. Lift the rule for `d2-proto` only**: delete the pair from
  `FORBIDDEN`, record it in `docs/PLAN.md` decisions, add `d2-proto` to
  `d2-sim`'s `[dependencies]`, and make each sim builder a one-line
  wrapper over `d2_proto::s2c::X { .. }.encode()`. `d2-proto` is pure
  (only `thiserror`), so determinism is unaffected; but `d2-proto` also
  holds `transport` (framing), which is what the rule kept out. The pins
  of §2 stay as the byte check of the switch.
- **C. Split**: move `s2c` (+ `generated::server`) into a crate with no
  transport, allowed for `d2-sim`.

## 2. What changed

- `crates/d2-sim/src/world/cube.rs`: new `pub fn trade_action(action) ->
  [u8; 2]` (S→C 0x77); the three inline `[0x77, …]` sends use it (bytes
  unchanged; the cube tests `f.sent == [[0x77, 0x0C]]` etc. still pass).
- `crates/d2-sim/tests/s2c_bytes.rs` (7 tests): pins the exact bytes of
  every sim S→C builder in scope for edge inputs: `npc::transaction`
  (0x2A), `npc::service_result` (0x58), `npc::resurrect_message` (0x9B),
  `waypoints::menu_message` (0x63), `waypoints::arrival_message` (the
  0x0D of the travel, incl. u16 truncation of x + 3 / y + 3),
  `path::walk::messages::player_stop` (the other 0x0D maker),
  `cube::trade_action` (0x77).
- `crates/conformance/tests/s2c_builders.rs` (7 tests, one per builder):
  sim bytes == `d2_proto::s2c::{NpcTransaction, OpenUi, Unknown9B,
  WaypointMenu, PlayerStop, TradeAction}::encode()` and
  `d2_proto::s2c::parse(sim bytes)` == the same `Message` variant, over
  every combination of field edge values plus a fixed xorshift sweep
  (0x77: all 256 actions). This settles J13 "one maker per id" as a
  check under option A: the sim builders and the proto types cannot
  drift. `arrival_message` maps to `PlayerStop { unit_type: 0, f6: 1,
  x: x + 3, y: y + 3, f11: 0, f12: 0 }` (`waypoints.md` §7 r7).
- M08: flipping bit 0 of `trade_action`'s action byte fails exactly
  `cube_trade_action_0x77` and `cube_trade_action_0x77_one_maker`
  (checked, reverted).

Not touched: items / inventory / economy wiring, `d2-client`,
`d2-proto`, depcheck, `docs/HANDOFF.md`, `docs/PLAN.md`.

## 3. `bridge-dispatch.tsv`: left all `TBD`

`client/bridge.md` §6: `owner` is the **spec path** that owns what the
message means for the client model, and rule 5 checks owner ≠ `TBD` ⇔
`dispatch::HANDLERS` registers a handler with that owner. No client
spec takes any S→C id yet (rule 2: "Today every row is TBD"; `audio.md`
names 0x2C only as an input, `ui.md` none), so the spec makes no owner
clear and every row stays `TBD`. Writing `s2c::Message` variant names in
the `owner` column would break rules 1 and 5.

For the client-model spec sessions, the typed variant ready for each id
a handler can match on (`d2_proto::s2c::parse` → `Message::…`): the 15
built ids (0x0D `PlayerStop`, 0x28 `QuestInfo`, 0x29 `GameQuestInfo`,
0x2A `NpcTransaction`, 0x4E `MercForHire`, 0x50 `QuestSpecial` quest
form, 0x52 `QuestLogInfo`, 0x58 `OpenUi`, 0x5D `QuestItemState`, 0x63
`WaypointMenu`, 0x77 `TradeAction`, 0x89 `UniqueEvent`, 0x8A
`NpcWantsInteract`, 0x91 `NpcGossipAct`, 0x9B `Unknown9B`), 0xAE
`WardenRequest`, and the 30 `generated` ids (`s2c-builders.md` §3);
every variant name equals the TSV `name`. If the table should carry the
variant, that is a spec change to `bridge.md` §6 rule 1 (a fourth
column) plus `dispatch::parse`, not an owner edit.

## 4. Open questions

1. §1: option A, B or C.
2. HANDOFF step 7k's spec side (`intents-events.md` §6 masked bytes
   0x2A 3–6, 0x58 6; `partial` layouts) is a spec task, untouched.

## 5. Gate (this branch, 2026-10-06)

`sh tools/gate.sh`: every non-client step **PASS** (spec_index, methods,
coverage `--check` / `--selftest`, trace checkers, hook selftest, fmt,
depcheck + determinism, `test d2-sim + conformance`, `test rest`,
doc-tests). The three `d2-client` steps (clippy workspace, test
d2-client, doc-tests d2-client) **FAIL in this container only**:
`wayland-sys v0.31.11`'s build script finds no `wayland-client` via
pkg-config (`pkg-config --exists wayland-client` → 1); this branch
touches no client code. `cargo clippy --workspace --exclude d2-client
--all-targets -- -D warnings` is clean after the fix of five `let mut`
warnings in the new conformance test. New tests: d2-sim `s2c_bytes` 7,
conformance `s2c_builders` 7, all pass. Local queue: none needed (no game
files); a local run of the client steps confirms the environment reading.
