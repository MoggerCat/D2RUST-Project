# Handoff: `prop_handle.rs` on the merged host — `claude/host-merge-integration`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Cloud implementation session, 2026-10-06, task class: test port (medium,
METHODS M14). Base: `claude/host-merge-integration` at `04cdcd6` (main
`63a706b` + `claude/host-merge`). Repo only, synthetic tables, no game
files. Only file changed besides this note:
`crates/d2-server/tests/prop_handle.rs`. No `src/` fix was needed.

## 1. State

`cargo check --workspace --all-targets` failed only in
`crates/d2-server/tests/prop_handle.rs` (`fuzz-server.md`), written
against the pre-merge API (`ItemWorld`, `ItemHooks`, `SkillSeams`,
`TradeWorld`). It is ported to the merged host (`host-merge.md` §3).
Every property is kept as it was (same generator, same pick pools,
same `run` assertions, same case counts 48 / 48 / 96 / 64); none is
weakened or deleted.

| Host | Before | Now |
|---|---|---|
| action | `SimGame<ActionSim<TestPending>, ActionWorld>`, `sim.skills = WiredSkills::new(vitals, Book)` (`Book: SkillSeams`), units with no room, `NoLevelTypes` | `SimGame<ActionSim<Book>, ActionWorld<WiredSkills>>`: one `Book` is the action wiring's `Pending` (positions, interaction, sends, skill list) + `UseRest` + `LearnRest` (same answers: the four-skill list, right hand Multiple Shot, usable, srvst/srvdo 1, class skill 3); `ActionHooks::vitals` set (0x3A); waypoints on `ActionWorld::waypoints`; all units in a real field room of a synthetic DRLG (`UseView` reads the room kind from the room; before, `SkillSeams::room` answered `Field`) |
| item | `SimGame` with an `ItemWorld` (own units, stats, `GameFields`) | `SimGame<ActionSim<_>, WiredWorld<_>>` with `cube: Some(CubeParts)` (same recipe ring → amulet, `local_date`, `ItemRest`), game creation `create_game(GameFields(0x5EED, expansion))`, units of the action sim, items in the host's one store, as `items/tests.rs` builds it |
| trade | `TradeWorld::new(action, GameFields, ...)` | `WiredWorld::new(action, ...)` (the seed is the action wiring's, as `e2e_vendor.rs`) |

What the properties check, per host, is unchanged; the checks are
stronger in three places:
- **Fault lists.** `faults()` now also reads `SimGame::tick_faults`
  (new) on every host, the action wiring's `hooks.errors` on the action
  host (only the trade host read it before) and the cube's `errors` on
  the wired hosts. The item host's snapshot (game, units, stats, item
  store, staged cube inventories) is the trade host's snapshot plus the
  cube's `Staged`; one `Snapshot` impl serves both (same type).
- **Quest ids.** 0x31, 0x40, 0x58 (`WiredWorld` now answers them;
  `fuzz-server.md` §1 recorded them as stubs) join `owned_id()`
  (`handled_ids_any_fields`, so they also run on the item host, now a
  `WiredWorld`); they were already in `trade_id()`.
  `hosts_reach_the_handlers` asserts each reaches its handler on the
  item and trade hosts.
- **Action host level.** The player's level (stat 12) is set to 1.
  Before and after the merge, `check_skill_point` compared level 0 with
  the learnable skill's required level (reqlevel 0 + base 1) and every
  0x3B stopped at code 3; now 0x3B also reaches the spend (code 0).

Spot check (200 generated messages per owned id, action and item
host): every skill id 0x05–0x11, 0x3A, 0x3B, 0x3C and 0x49 reaches its
handler on the action host with 0 and with refusals; 0x2A (mostly 1, as
before), 0x4F, 0x31, 0x40, 0x58 on the item host.

## 2. Found by the port (fixture, not host)

With the units in a real room and the action wiring's errors read, a
0x49 travel to waypoint index 0 (level 1) recorded
`WiringError::Drlg(NoSpawnRoom)`: the synthetic DRLG had rooms only on
Cold Plains while the waypoint table lists levels 1, 3 and 40. That is
the wiring reporting a data set with no spawn room (`drlg` §10), not a
host crash; the fixture now builds one room on each waypoint level
(acts 0 and 1), as the server's waypoint tests do. No crash, panic,
overflow or recorded fatal path in the merged host.

## 3. Runs

`cargo test -p d2-server --test prop_handle` (debug, overflow checks):
5 pass, ≈ 0.7 s. Hunts: `PROPTEST_CASES=3000` all (20 s),
`PROPTEST_CASES=6000` trade host alone (36 s): no failure. Gate:
`sh tools/gate.sh`: GATE PASS (every step, d2-client included).

## 4. Second port: `crates/d2-client/tests/prop_worldsim.rs`

Base: `claude/host-merge-integration` at `64ed44b` (the coordinator's
merge of `claude/tender-meitner-mphas3` 729c76e, which added
`prop_worldsim.rs` against the pre-merge API: `SkillSeams`,
`TradeWorld`). Ported as `e2e_single_player.rs` already is on the merged
host; no `src/` change.

- Host: `SimGame<WorldSim<TestPending>, WiredWorld<Rest, WiredSkills>>`
  (was `TradeWorld<Rest>` + `sim.skills = WiredSkills::new(vitals,
  book)`); the skill handlers sit in `ActionWorld::skills`; the vitals
  were already on `ActionHooks::vitals`; `WiredWorld::new` without the
  `GameFields` argument (the seed is the action wiring's).
- Skill seams: `Book`'s `SkillSeams` impl becomes inherent helpers
  (`find_entry`, `entry_mode`, `srvst`, `srvdo`) that `TestPending`'s
  `UseRest` already called; the message path now reads the same
  `TestPending` (one provider, `host-merge.md` J4). `LearnRest` added
  with the pre-merge `SkillSeams` defaults (`is_class_skill` false).
  Gone with `SkillSeams`: `Book::room` (`Field`; now the units' real room
  through `UseView`, they stand in the generated level) and
  `Book::create_skill_missile` (a log line no assertion read; the
  message path now creates the missile through the action wiring, as
  the timer path did).
- Properties, assertions (same count) and case count (8) unchanged;
  `errors()` now also reads `SimGame::tick_faults`.

Runs: `cargo test -p d2-client --test prop_worldsim` 4 pass (≈ 11 s);
`PROPTEST_CASES=200`: 4 pass (287 s), no failure.
`cargo check --workspace --all-targets --keep-going`: clean.
Gate after the second port: `sh tools/gate.sh` GATE PASS (every step).

## 5. Third port: `claude/tender-meitner-mphas3` 002b244 merged in

Merged `origin/claude/tender-meitner-mphas3` at `002b244` (bench-baselines,
prop-client, mutants-server, conformance-path-render, docs-fold-6) into
`claude/host-merge-integration` (no conflict). Only
`crates/d2-server/tests/mutants_adapters.rs` broke: it set
`sim.skills = Some(Box::new(Recorder))` and implemented the old
`SkillHost` (`handle -> Handled`, `unsent`). Ported: the test's game is
`SimGame<Unspecified, SkillSlot>`, where `SkillSlot(Option<Recorder>)`
is a minimal `WorldHost<Unspecified>` whose `skill` slot forwards to
the recorder (`None`: no skill handlers, the "stays a stub" check);
`Recorder::handle` returns `Some(Handled)`. The three tests and their
assertions are unchanged (routing of 0x01 / 0x41 / 0x51 vs 0x3C, codes,
the owned-item act skip). Every other new test built and passed as is.
`cargo check --workspace --all-targets --keep-going`: clean;
`sh tools/gate.sh`: GATE PASS.
