# Coverage pass: specs/missiles (bodies.md, bodies-2.md, missiles.md)

Method: `// Covers:` lines on unit tests (COVERAGE.md §2). `python3
tools/coverage.py --check` reports 0 errors. All tests pass
(`cargo test -p d2-sim --lib missiles`, `cargo clippy -p d2-sim -- -D warnings`).

## Covered

- **bodies.md**: 33 of 34 uncovered units. `§N text` for N = 1..29 (sections
  2, 4, 7..29 claimed on the existing body tests in
  `crates/d2-sim/src/missiles/tests/ext.rs`, whose assertions already check
  the section's row values, Live notes and closing prose; extra assertions
  added for §7 "no draws", §25 "r < 0 makes nothing", §19 l2 r4 (return 1)).
  New tests in `tests/cov_text.rs`: §1 text + r1 (spark window 258..42,
  every 2 elapsed frames; no record -> 2), §3 text + edge r1 (same slot every
  frame), §5 text + edge r1 (unit argument not read), §6 text (8 + 15 clouds,
  3 loops, velocity 192 / 384, plaguejavlinexplode does not move), edge r3
  (volcano without owner still flies).
- **bodies-2.md**: 42 of 43. `§N text` for 31..59, 61 on the existing tests;
  §44 l3 r1-r4 and l4 r1-r5 claimed on the `unit_find` tests (rooms/found
  order, default filter), §44 text on the Radament test (+ town-room check
  added). New: edge r5 (heal roll vs damage-stage roll), edge r9 (empty /
  unspawnable list is fatal), edge r11 (Tiger Fury trail not placed at the
  missile).
- **missiles.md**: 4 of 6. §r2-2 (source scan: only `create.rs` calls
  `alloc_missile`), §r2-3 r9 (real world: one game-seed step, GUID counter
  +1, init flags/timers; `wiring/action/tests/missiles.rs`), §r9-3 text and
  §r9-5 text (server-do 8/10/17/25 all run the seeded helper from the
  missile's level, skill and frames left).

## Code fixes found by tests

None: every new test passed against the existing code.

## Left, with reason

- `bodies-2.md §edge-cases-original-bugs r2` (Battle Cry state request carries
  stat 0 / value 0): the body only calls the `apply_state` seam
  (`MissileWorld::apply_state`, documented "stat field 0 value 0"); the
  production implementation of that seam is not wired in
  `wiring/action/missiles.rs` (default `false`), so there is no code to test.
  Needs the seam provider.
- `bodies.md §edge-cases-original-bugs r6` (server-do 6 asks for the owner
  before the missile pointer): the owner lookup has no observable effect on
  the fake or the real world (both orders give result 2); nothing to assert.
- `missiles.md §r2-4-client-message` (message 0x73): not implemented. The
  layout of 0x73 is open (`server-messages.tsv` names fields as unknown;
  HANDOFF lists 0x73 as a gap) and its only caller is the unit-add code
  (`0x00571F90`, `wiring/action/switch.rs` says "not sent"). Bucket C-large:
  needs the layout recorded first, then a builder in `units/messages.rs`
  and a send in the switch.
- `missiles.md §edge-cases-original-bugs r10` (null entries would crash):
  d2rs deliberately logs `Unhandled::Null*` and keeps the missile instead of
  crashing (`tests.rs::null_table_entries_are_flagged` says so). The rule is
  a statement about the original, not about d2rs behaviour: not claimed.

Honesty note: the section `text` claims for bodies.md / bodies-2.md rest on
the section's main behaviour test; the row-id lists in those texts are table
data (catalogue) and are not re-checked against missiles.txt (game-file tier).
