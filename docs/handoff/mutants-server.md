# Handoff: mutation testing of `d2-server` and `d2-net` — `claude/mutants-server`

> Not yet folded into `docs/HANDOFF.md` / `docs/PLAN.md` (neither is edited here); the coordinator folds it.

Cloud test session, 2026-10-06, base `claude/tender-meitner-mphas3`
`9b49081`. Repo only (M09). METHODS M08 applied to the server crates:
`cargo-mutants` 27.1.0, `cargo mutants -p <crate> --timeout 60`. No
production code changed; tests are in two new files only
(`crates/d2-server/tests/mutants_core.rs`, `mutants_adapters.rs`), so
nothing collides with `host-merge`'s work on `src/adapters/`.
`mutants.out/` is not committed.

## Counts

| Run | Mutants | Caught | Missed | Timeout | Unviable |
|---|---|---|---|---|---|
| `d2-net` | 0 (the crate is two doc-comment lines) | – | – | – | – |
| `d2-server`, before | 862 | 390 | 366 | 3 | 103 |
| `d2-server`, after | 862 | 415 | 341 | 3 | 103 |

The 3 timeouts are `ClientBuffers::pop → Some(vec![…])`: the flush loop
never ends, so the existing tests hang. A hang counts as detected.

Missed by file, before → after:

| File | Before | After |
|---|---|---|
| `buffers.rs`, `dispatch.rs`, `host.rs`, `transport.rs` | 18 | **0** |
| `adapters/sim.rs` | 2 | **0** |
| `adapters/handlers/skills/mod.rs` | 5 | **0** |
| `adapters/handlers/skills/world.rs` | 170 | 170 |
| `adapters/handlers/items/cube_world.rs` | 117 | 117 |
| `adapters/handlers/skills/seams.rs` | 46 | 46 |
| `adapters/handlers/world.rs`, `world/trade.rs`, `world/action.rs` | 8 | 8 |

## (a) Killed: 25 mutants, each by a spec vector

All 25 were verified killed in the second full run.

| Mutant(s) | Test | Spec |
|---|---|---|
| `dispatch`: id guard → `true`, `&&` → `\|\|`, `<` → `<=` | `system_ids_never_reach_a_game_handler`: 0x67 (46 bytes, its right transport size), 0x69 and 0x70 return 3 and the handler never runs | `intents-events.md` §2.3 rule 1 |
| `check_size`: arm 0x15 deleted | `chat_skips_the_exact_size_check`: 0x15 one byte past its rule size reaches the handler with size 12 | §2.4 rules 1, 6 |
| `select_skill` `>>` → `<<`; `bind_hotkey` `&` → `\|` / `^` | `skill_fields_right_hand`: the spec vectors with the hand bit clear | §2.4 rule 7 |
| `ServerQueues::is_empty` → `true` | `queues_report_waiting_messages` | §2.1 rule 6 |
| `ClientBuffers::queue` `>` → `>=` | `message_of_a_whole_buffer_fits` (0x200 bytes) | §3.2 rule 2 |
| `remove_client` → `()` | `removed_client_has_no_buffers` | §3.2 rule 1 ("client null → nothing") |
| `Inbox::push` `>` → `>=` | `delivery_accepts_the_largest_message` (0x204 bytes) | §3.3 rule 2 |
| `Inbox::deliver` guard → `true`, `&&` → `\|\|` | `split_ends_on_a_short_or_size_zero_message` (0x0A with 3 of its 6 bytes; 0x80) | §3.3 rule 3 |
| `Host::flush` `discarded +=` → `-=` / `*=` | `frame_reports_discarded_bytes` | §1 rule 3, §3.3 rule 3 |
| `Host::disconnect` → `()` | `disconnect_forgets_the_client` | §2.2 rule 2 (record per client in a game), §3.2 rule 1 |
| `SystemClock::now_ms` → 0 / 1 | `system_clock_advances` (30 ms sleep, asserts ≥ 30 elapsed) | `tick.md` §8 |
| `SimGame::unit_target`, both `&&` → `\|\|` | `owned_item_skip_needs_an_owned_item`: an unowned item and a monster the player owns, in another act, give "other act"; an owned item gives "owned item" | §2.4 rule 4 |
| `skills::handled` → `true`, `==` → `!=`, `&&` → `\|\|`; `skills::handle` `\|\|` → `&&` | `skill_routing`: 0x01, 0x41, 0x51 never reach the skill host; 0x3C does; no host means a stub | §2.4 rule 7, §4 rule 1 (the `IDS` table) |
| `skills::code` arm 1 deleted | `handler_codes` | §2.3 result codes |

`system_clock_advances` reads the real clock. It is the one test here
that is not fully deterministic, but it can only fail if the OS sleeps
for less than 30 ms.

## (b) Not killed: 341 mutants, all in `adapters/handlers/`

None of these was killed in this session. `host-merge` is restructuring
these files right now. A test written against today's internals would be
rewritten after the merge, and most of these cannot be reached from the
public API without a fully wired skill or cube pipeline.

1. **Forwarding glue, 259 mutants** (147 in `skills/world.rs`, 112 in
   `cube_world.rs`). These are methods of the `d2-sim` seams
   (`UseWorld`, `ManaUnits`, `VitalsUnits`, `CubeWorld`, `CubeRest`)
   that return a field or pass the call on. The behaviour belongs to
   `skills/use.md`, `combat/vitals.md` and `items/cube.md`, whose
   d2-sim tests use their own fakes. In d2-server only a few of these
   paths run end to end (`skills/tests.rs`, `items/tests.rs`). These
   could be killed, but only through end-to-end scenarios on the wired
   sim. They are not equivalent mutants.
2. **Seam defaults, 46 mutants** (`skills/seams.rs`). These are the
   default bodies of `SkillSeams`, for parts that no written spec
   provides yet (`docs/handoff/wire-action.md` §4: the narrowest
   answer). No spec decides them, so they are unobservable by spec.
   Asserting them would test a convention, not 1.14d behaviour. Leave
   them until their owner specs exist.
3. **Logic and wiring in the glue, 36 mutants, spec-decided.** These are the
   priority after `host-merge` lands:
   - `skills/world.rs` (23): `has_player_data`, `last_point_frame` and
     `set_last_point_frame` (player-data +0x168, §2.4 rule 3);
     `in_own_inventory` and `same_act` (§2.4 rule 4: owned item, other
     act, staged pairs); `endanim_expire` (smallest positive type-1
     expire, `use.md`); `start_mode` flag clear `&= !ATTACK_PENDING`
     (`use.md` §4 last paragraph); `is_alive` (`units.md` §2).
   - `cube_world.rs` (5): `interacting_with_stash` (3), `trading` (1),
     `put_item_check` guard `m > MODE_CURSOR` (1).
   - `world.rs` `record_of` `==` → `!=` and → scratch: no test has two
     vendor records, so the wrong NPC's record is never caught
     (`vendors.md` §7.1–§7.2). Same file: `client_of` →
     `Some(0)` (one client only in every test), and `take_sent`
     default → `vec![]`.
   - `world/trade.rs` `vendors`/`waypoints` → `None` and `fault` →
     `()`; `world/action.rs` `fault` → `()`: no test reaches these
     through a `TradeWorld` / `ActionWorld`, or checks a recorded
     fault.

## (c) Code against spec

None found. Every killed mutant's test passes on the current code. The
behaviour each one pins is the one the spec states.

## Proposed follow-up (coordinator)

After `host-merge`: a test session for the 36 mutants of (b)3,
in the new module layout. A second client and a second vendor record
in the trade fixture kill `client_of` / `record_of`. A two-act staged
fixture with a skill message on a unit kills `same_act` /
`in_own_inventory`. Then rerun `cargo mutants -p d2-server` and record
the counts here or in HANDOFF §1.

## Gate (this branch)

`cargo fmt --check`; `cargo clippy --workspace --all-targets -- -D
warnings`; `cargo test -p d2-server -p d2-net`; `cargo run -p depcheck`;
`python3 tools/spec_index.py --check`; `python3 tools/methods.py check`;
`python3 tools/coverage.py --check` and `--selftest`. The results are
in the commit message.
