# Handoff: walk and run — `d2_sim::path::walk` (`claude/impl-walk`)

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Cloud implementation session, 2026-10-06, task class: implementation from
a clear spec, medium (METHODS M14). Base: `claude/specs-staging` at
`c6e40f9` (pathing.md §5.2 steps renumbered 5 and 6). Repo only,
synthetic grids, no game files (M09). Spec: `specs/sim/pathing.md`
(draft) with `sim/path-tables.tsv` and the rows 0x0D, 0x0F, 0x10, 0x15,
0x96 of `sim/server-messages.tsv`. Parallel sessions `impl-path-core`
(path-placement §1–§6) and `impl-path-place` (§7–§12): no code shared;
this module meets them through the seam traits below.

## 1. State

**Implemented, unverified.** Every rule of pathing.md §1–§10 for path
types 1, 2 and 7 is in code; per-tick positions have no recording yet
(pathing.md open question 1, its own queue item), so nothing is
confirmed against 1.14d. The test vectors W1–W6, W9, D1–D4, M1, M2, S1,
S2 pass exactly as printed (M1: 13 steps of 0x6000 and arrival at
0x698000 in tick 14; M2: all six (x, y) pairs). V1–V3: see question 1.

Files: `crates/d2-sim/src/path/mod.rs` (new, `pub mod walk;` only),
`crates/d2-sim/src/path/walk/` (new), one line in `lib.rs`
(`pub mod path;`). No change to `units/`, `wiring/`, `d2-server`, specs or
other crates.

| File | Spec |
|---|---|
| `tables.rs` | Constants: strict parser of the embedded `path-tables.tsv` (`include_str!`); 582 rows, unknown table / missing / repeated row / bad header are errors |
| `seams.rs` | `WalkPath` (the §2.3 fields walk uses), `PathWorld`, `WalkUnits`, `PathInfo`, `WalkError` |
| `geom.rs` | §5.1 octant, steps, path distance, ray test; §8.3 direction vector; §8.5 facing; §9.5 unit distance |
| `find.rs` | §2 set type / type reset, §3 compute, §4 preparation and push, §5.2 toward, §6 straight, §7 A* |
| `velocity.rs` | §8.1 mode velocity, setter, §8.2 run stat value, §8.4 aim |
| `step.rs` | §9.2 player event 0, §9.3 step, §9.4 movement, §9.5 arrival, §9.6 one step / cell walk / footprint move / set position / room recache, §9.7 reset, §9.8 room-change messages, §9.9 run drain, §9.10 re-path |
| `request.rs` | §1.1 C→S 0x01–0x04 handler, §1.2 request, §1.3 mode check, §1.4 interrupt check, §1.5 start, neutral start |
| `messages.rs` | §10: byte builders 0x0D, 0x0F, 0x10, 0x15, 0x96 (bit-packed per the `bits:` rule), `mode_update` (rule 2), `reassign_flag` (rule 3) |

No floating point: 16.16 positions on u32 with the original's wrapping;
the x87 target lead (open question 4) is the seam
`WalkUnits::target_lead` (default `None`: target unchanged). Randomness:
only §1.4 rule 5's `roll(100)` on the unit seed (`Seed::roll`).

## 2. Public API

- `path::walk::request::handle_message(t, w, u, game, player, id, a, b)`
  → `(0, Option<Outcome>)`: the server handler of 0x01–0x04 after
  `intents-events.md` §2.4 (a, b = x, y or type, GUID).
- `request(t, w, u, game, unit, skill, mode, WalkTarget, reentry)` →
  `Outcome` (`NoTargetUnit`, `ModeRefused`, `InterruptRefused`,
  `KnockbackIgnored`, `OtherMode`, `Neutral`, `Moving(n)`); `mode_check`,
  `interrupt_check`, `start_movement`, `set_mode_and_velocity`,
  `neutral_start`.
- `Walk { t, w, u }.player_event0(game, unit)` → `Step::{Moving,
  Stopped}`: the every-tick event 0 of player modes 2, 3, 6, 19 (timer
  step 4). `Walk::step` / `movement` / `repath` / `set_position` / `reset`
  / `room_change_messages` / `run_drain` / `target_check` for the monster
  mode functions (§9.1, owner: monster modes) and other callers.
- `find::{compute, set_type, reset_type, prepare, toward, straight,
  astar}`; `velocity::{mode_velocity, set_velocity, run_velocity_bonus,
  aim}`; `geom::*`; `messages::*`; `PathTables::embedded()`.

## 3. Seams

**`PathWorld`** (provider: `impl-path-core`, path-placement §1–§6):
`load_path` / `store_path` (copy the record to and from `WalkPath`),
`cell_room` (§4 r1), `room_rect`, `room_in_town` (`0x0061AB00`),
`pattern_collides` (`0x0064D910`), `remove_footprint` / `add_footprint`
(§5.2, at the **stored** position), `try_move` / `forced_move` /
`missile_move` (§6 r1–r3), `room_list_remove` / `room_list_insert`,
`queue_for_update`, `room_clients` (`drlg/rooms.md` §7). Contract: entry
points load the record, work on the copy and store it at the end; during
a compute the position, pattern, mask and room are unchanged before the
footprint calls, so the stored record is what the footprint ops read.

**`WalkUnits`** (defaults = the narrowest reading, like
`wiring::action::Pending`): unit type / class / GUID / mode / position /
size, `find_unit`, door orientation (objects), `first_type1_expire`
(tick), cursor item (inventory), states, `state_stat`, `stat`,
`item_stat`, base stat add / set, `seed`, used skill fields and setter,
`clear_queued_action`, `set_mode`, `cancel_events`, `schedule_event0`,
`start_other_mode` (start functions of modes ≠ 2, 3, 6, 19, incl. the
neutral re-entry), `attach_run_stats` (stat-lists), charstats / monstats
velocity, torso armor speed, `monster_can_be_in_town`, unit flag set,
`state13_step` (skills), AI room memo, `client_player`, unit add / removal
messages (unit-update spec), `target_lead` (OQ4), `other_path_function`
(circling `0x00679B30`, types 0, 3, 8, 9, 11, 12, 15, 16 and the missile
path: OQ3 / missiles spec), `repath_budget` (`0x00649120`, OQ8).

Integration notes: (a) `set_mode` must not run the velocity half of
`0x00623F50`; `set_mode_and_velocity` runs it right after the seam (and
attaches the run list first). The units' animation-rate provider owns the
rest of `0x00623F50`. (b) d2-server's 0x01–0x04 are still stubs (§1 3k);
wiring them is `handle_message` on providers of both traits. (c) The
update pass (`tick.md` §6 step 5) has no caller of `mode_update` /
`reassign_flag` yet.

## 4. Questions (spec / integration)

1. **V1–V3 vs §8.1.** The formula p = f + stat 67 (floor 25) gives V1 only
   when stat 67 holds its creation value 100 (`combat/vitals.md` §1 table);
   the vector says "stat 67 = 0". V2 then needs stat 67 = 150 (100 + run
   list 50). V3's printed p = 67 / 1029 drops the base; the formula gives
   p = 17 + 150 = 167 → 2565. Tests follow the formula with base 100; the
   spec's V-rows need rewording (spec session).
2. §8.1: a mode without the velocity modifier (neutral etc.) has no rule:
   `mode_velocity` returns `None` and the velocity is left as it is
   (`TODO(spec)` in `velocity.rs`).
3. §8.1 rule 3: read as "+0x38 := 15 only when the value differs; velocity
   and max velocity always set".
4. §9.5: where the rule says "→ re-path", the arrival result is the
   re-path's (non-zero passes).
5. §9.2 step 2: state 13 treated as a branch (the seam runs, the event
   returns "moving") — whether the step continues after `0x005C9D90` is
   not stated (`TODO(spec)`).
6. §1.4 rule 5: read as "state 42 → roll (r < v → rule 6, else allow);
   else state 15 → rule 6"; whether a failed roll falls through to the
   state 15 test is not stated (`TODO(spec)`).
7. §9.10: the town-access argument of the re-path's compute is not stated;
   0 is used (`TODO(spec)`).
8. §9.6 rule 8: the step passes no room hint to set position (the spec
   names none for movement).
9. Edge case 5 is unreachable through §9.4: (m · direction) is a 32-bit
   product, which wraps above 8 sub-tiles per tick on an axis, so the
   cell walk never sees more than 8 substeps; the test drives
   `cell_walk` directly with a 12-cell Δ.
10. Cell walk: a loop bound of 2^20 iterations reports a fatal error
    instead of hanging (no rule; unreachable on valid input).
11. A* edge case 2: a propagation stack over 200 is `WalkError::Fatal`
    as the spec asks.

## 5. Tests and gate

34 tests in `crates/d2-sim/src/path/walk/tests/` (`mod.rs`, `messages.rs`,
fakes in `fake.rs`: one room grid following path-placement §3–§6 for
patterns 0–5). Coverage: `pathing.md` 105 of 125 units (84.0%, unit tier).
Not claimed: §9.1 (monster mode functions), §9.2 r5 (host-only), §10 r4,
r5 (other owners), §3 text, §7 r1, §9.5 r1, §9.6 r1, r2, edge cases r1,
r2, r11 and some section texts. M08 by hand: open-list insert `>=` → `>`
fails W2; octant `dx & 1` → `min(dx, 1)` fails the octant test; the reach
test `<=` → `<` is not caught by any vector (no step lands exactly on
M). Messages: each builder is read back field by field from the
`server-messages.tsv` layout; one flipped x bit changes exactly one bit
of 0x96.

Gate on this branch: `cargo fmt --all -- --check`; `cargo clippy
--workspace --all-targets -- -D warnings`; `cargo test -p d2-sim` (1,273
pass, 5 ignored); `cargo run -p depcheck` (determinism lint clean);
`python3 tools/spec_index.py --check`; `python3 tools/methods.py check`;
`python3 tools/coverage.py --check` (3,403 claims, 0 errors) and
`--selftest`: all clean. `d2-proto` tests not run (known 0x96 `bits:`
failure until `claude/proto-bits` merges).

No local run queue item added: the check that would verify this module
is pathing.md open question 1 (a per-tick position recording), already
the spec's settle step.
