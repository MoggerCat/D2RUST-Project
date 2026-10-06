# Handoff: shared e2e fixture, handlers' early refusals, `anim` in the tick converter — `claude/test-support-fold`

Cloud test session, 2026-10-06, task class: test refactor and tooling
from clear specs, medium (METHODS M14). Base: `claude/tender-meitner-mphas3`
at `01dff69`. Repo only (no `game/`, `re/`, `../refs/`). Scope of every
claim: this branch, `cargo test` in a debug build. Task: `docs/HANDOFF.md`
§2 step 7m (PK3 / FS2, PK1, CH2) and 7t(f).

## 1. State

### (a) One shared test-support module (PK3, FS2, 7t(f))

- New `crates/d2-client/tests/e2e_support/world.rs` (`e2e_support::world`,
  1,021 lines): the wired single-player world's fixture that
  `e2e_single_player.rs` and `prop_worldsim.rs` each had a copy of
  (constants, `TestPending`, `Spot`, `Book` / `Inner`, the DS1 / DT1
  sources, the DRLG / preset / outdoor / maze / monster / levels / stat /
  skill / missile / combat / vitals / AnimData / drop / waypoint tables).
  Items are `pub`; both files `use e2e_support::world::*`. The
  e2e-only constants (`PLAYER_GOLD`, the stat ids other than `GOLD`) stay
  in `e2e_single_player.rs`; the cube's item world stays there too
  (`prop_worldsim` never had it).
- The two copies differed in one behaviour: `prop_worldsim`'s
  `TestPending` kept its own interact record (`set_interact` /
  `reset_interact` / `interact_guid`), the e2e's answered `Pending`'s
  defaults. The shared `TestPending` has
  `interact: Option<BTreeMap<..>>`: `None` (default) answers the
  defaults, `Some` keeps the record; `prop_worldsim` sets `Some`. Both
  tests therefore run exactly what they ran before. The other
  differences were doc comments only.
- `d2-server/tests/prop_handle.rs`'s `trade` module dropped its copy of
  `e2e_support`'s `Rest`, `equiv`, `item_tables`, `vendor_tables`,
  `monstats` and constants (≈ 400 lines) and includes the module by path
  (`#[path = "../../d2-client/tests/e2e_support/mod.rs"] mod
  e2e_support;`; every dependency of the module is a `d2-server`
  dependency). One behaviour-neutral difference: the shared `Rest` logs
  its calls (`log`), which `prop_handle`'s snapshot does not read.
- `e2e_support::Rest` now derives `Debug` (for the refusal digests).
- Net: −2,412 / +622 lines over the branch, part (b) included.

Test names and counts, before → after (same names):

| Test binary | Before | After |
|---|---|---|
| `d2-client --test e2e_single_player` | 3 pass | 3 pass |
| `d2-client --test e2e_vendor` | 4 pass | 4 pass |
| `d2-client --test prop_worldsim` | 4 pass | 4 pass |
| `d2-server --test prop_handle` | 5 pass | 5 pass + 7 new (part b) |

`e2e_single_player` asserts its whole transcript byte for byte and
`prop_worldsim` its digests, so a fixture drift would have failed them.

### (b) Handlers' early refusals change nothing (PK1)

For each `d2-server` handler whose spec has a refusal ordered before any
effect, a test sends the refusal and checks three things: the spec's
result code, an unchanged digest, and nothing sent. The digest is the
game; units and stats; item store; inventory / cube / interaction /
NPC / quest state; player fields; stubs; resyncs; faults; the seams'
logs and outboxes. "Nothing sent" means the client's buffers and the
seams' queued messages. The messages go through the dispatcher with no
tick, so only the handler's effect is compared.

| Test | Handlers, spec rules |
|---|---|
| `prop_handle::skill_refusals_change_nothing` | 0x3C entry missing → 3 (`use.md` §7); 0x3A id > 15 → 3, first spend → 2 (`vitals.md` §2); 0x3B id past the table → 2 (`levels.md` §6.4); 0x06 / 0x07 / 0x0D / 0x0E no skill → 3 (`use.md` §1 r3, r6) |
| `prop_handle::waypoint_refusals_change_nothing` | 0x49 object missing → 1, level past the table → 3 (`waypoints.md` §6.2), with no interaction (§6.3 r1 not met) |
| `prop_handle::cube_refusals_change_nothing` | 0x2A item missing, cursor-mode item not the cursor, cube missing → 1 (`cube.md` §2 steps 1, 2); 0x4F 0x17 / 0x18 with another interaction → 3 (§1) |
| `prop_handle::npc_refusals_change_nothing` | 0x13 / 0x2F / 0x30 NPC missing → 1 (`npc.md` §2, §3); 0x34 at a non-Cain interact unit → 0, no message (§6 step 2) |
| `prop_handle::vendor_refusals_change_nothing` | 0x33 item missing → 1, no message (`vendors.md` §7.2 r1); 0x37 item missing → 2, not the last bought → 3 (§5.5) |
| `prop_handle::quest_refusal_changes_nothing` | 0x58 quest ≥ 0x2A → 2 (`quests.md` §1.7) |
| `prop_handle::refusal_check_sees_a_late_refusal` | M08: the cube's §2 step 3.4 refusal (after the 3.1 targeting reset) fails the check |
| `handlers::items::moves::tests::early_refusals_change_nothing` | `inventory.md` §7.1–§7.24: 0x16 own player → 3, missing / too far → 1; 0x17, 0x18, 0x1A, 0x28 cursor check → 1; 0x18 page 1 → 2, page 2 → 3; 0x19, 0x20, 0x63 stored check → 1; 0x1A–0x1D location 11 → 2; 0x1B / 0x1E not a hand → 3; 0x1D / 0x1E empty → 1; 0x21 same item → 3; 0x22 → 3; 0x27 owned check → 1; 0x50 amount / unit → 3; 0x61 classic → 3 |
| `handlers::walk::tests::early_refusals_change_nothing` | 0x02 / 0x04 target missing (`pathing.md` §1.2 r1), 0x01 / 0x03 in mode DT (§1.3): result 0, nothing changed or queued |

Left out on purpose, because the spec orders a write before the refusal:

- `cube.md` §2 step 3.1: the targeting reset comes before 3.2 / 3.4.
- `npc.md` §2: the path, AI and think steps come before `start` rule 1.
- `waypoints.md` §6.3 r1: refusals while the waypoint is the
  interaction.
- The point forms: the validator stores the frame first
  (`intents-events.md` §2.4 r4).
- `inventory.md`:
  - §7.3 placement → 3 and §4.6 → 3 come after the reset.
  - §7.10 refuses after T has moved.
  - §7.14 and §7.19 refuse after the reset.
  - The item-move gate's 0 (§5.4).

Refusals that answer with an S→C 0x2A are not "early" by this
definition; their bytes are already asserted by the handler tests (§3).

No refusal changed anything. All codes matched the specs, so there are no
bugs and no non-test code changed. Two test-design notes:

- 0x34 needs Akara to be the interact unit (talk first), or step 1
  answers 0x2A code 9.
- Monster and player GUIDs are separate spaces, so the player's GUID in
  0x2F's NPC field can name Akara.

### (c) `convert_tick.py` skips `anim` (CH2)

- `tools/trace-recorder/convert_tick.py` skips `anim` records
  (`record_tick.py` 0.2.0). The units harness reads them from the raw
  file, and a `sim/tick` trace does not hold them.
- `set`'s optional 0.2.0 fields (`site`, `cl`, `m`) were already ignored.
- New selftest case `synthetic_v02`: the synthetic recording with
  0.2.0's `set` fields and one `anim` record per schedule converts to the
  same setup / inputs / expected as without them. Any other unknown kind
  (`anym`) is still refused (M07).
- M08: with the skip disabled, the selftest fails with "record 14:
  unknown record kind anim".
- Docstring and README note updated. The converter's `TOOL` version is
  unchanged: the output for a given input is the same.

## 2. Bugs found

None.

## 3. Open questions / follow-ups

1. Not covered by (b), each needs a fixture step this session did not
   take:
   - 0x38's unit check (`Rest::unit_check` always answers 0).
   - 0x2F / 0x30 "other act" → 2 and "not a monster with a list" → 3.
   - 0x49's other §6.2 refusals (act, reach, index).
   - 0x24–0x26 belt refusals.
   - 0x29's scroll / book checks.
   - Walk's interrupt check (§1.2 r3).
2. 0x32 / 0x35 / 0x36 / 0x62 answer with S→C 0x2A. "State unchanged"
   holds for them by the handler tests' bytes and partial state, not by
   a full digest.
3. The shared module lives in `d2-client/tests` and `d2-server` includes
   it by path. If that is unwanted, a `crates/test-support` crate (or
   moving the module to `d2-server/tests`) is the alternative; it touches
   `Cargo.toml` and depcheck.

## 4. Local checks

None (no game files involved).

## 5. Gate

`sh tools/gate.sh` on this branch, 2026-10-06: every step PASS (spec_index, methods, coverage check / selftest, trace checkers, hook selftest, fmt, depcheck, clippy workspace, tests d2-sim + conformance, rest, d2-client, doc-tests). `GATE: PASS`.
