# Handoff: the monster mode message and the death pair — `claude/impl-monster-death`

Cloud implementation session, 2026-10-07. Task class: implementation from
a clear spec, medium (METHODS M14). Base: `claude/specs-staging-2` at
`ddbfe0b`. Repo only, synthetic data, no game files (M09). Specs:
`sim/intents-events.md` §7.3 rule 2 step 2, §7.4, §7.5, §7.7;
`sim/units.md` §4.1, §4.2, §4.6. Parallel session `impl-server-join-2`
(join, room switch): none of its areas touched (§7.1 rule 2.1 / §7.2
add messages and §7.8 left alone, see §4 item 1).

## 1. State

**Implemented, unverified** (M02): the §7.4 / §7.7 Test vectors pass as
unit tests; no recording of the wired host exists to compare against
(§5).

- **Builders and the mode message** (`d2-sim/src/monsters/mode_message.rs`,
  new, pure): the mode table `0x006E1D90` (`MODE_ROWS`), `ModeInput`
  (what `0x00597E20` reads), `mode_message` (§7.4 rules 3–6) →
  `ModeMessage::{Send, Stop (0x6D; caller adds 1 to stat 328), Skill,
  Nothing}`, and the nine builders of §7.7 rule 5 (`monster_move`,
  `knockback`, `move_to_unit`, `knockback_to_unit`, `monster_state`,
  `state_to_unit`, `monster_action`, `monster_attack`, `monster_stop`).
- **Wiring** (`d2-sim/src/wiring/action/unit_update.rs`, new):
  - `View::monster_update` — §7.3 rule 2 step 2 in the client pass:
    `ActionSim::send_unit_update` now routes monsters to it (path provider
    on only; without it no unit has a path record, as for players). Reads:
    the target `0x00553540` with the refresh (stale / picked-up target
    cleared on the path) and the room-client test `0x005387F0` (the DRLG
    active room's client array, `rooms.md` §7 rule 1), the path (cell,
    target, direction +0x64, type +0x3C, +0x90, +0x91, +0x93), the life
    fraction `0x005A5650`, stat 67, the skill in use (`Pending::used_skill`).
    Sends via `Pending::send` to the client's player, then unit flag
    0x80000 := 0. Errors: `WiringError::ModeMessage(NoPath)` (rule 4's
    fatal 0xE6) and `SkillMessage` (rule 3's 0x4C / 0x4D: no layout).
  - `View::room_cleanup` — the flag part of `0x00553220` (§7.5 step 3
    and the flag bits of step 7), run by `ActionSim`'s tick hook
    `unit_update` (tick step 6).
  - DT's event functions: `death_event0` (`0x005A7350`: base id 78 steps,
    `Pending::refresh_animation`, `anim_complete` = `0x006217C0`, then
    mode 12; others mode 12 at once) and `death_event1` (`0x005A72B0`:
    mode 12, unit event 13, `SplEndDeath` 1 / 2 → `Pending::death_end_action`
    with `minion1`), called from `ActionHooks::monster_mode_function`.
- **New `Pending` seams** (defaults; no spec owns their bodies):
  `unit_b0` (unit +0xB0, 0), `monster_flag_100` (`0x005A0180(unit,
  0x100)`, false), `refresh_animation` (`0x00623E00`), `death_end_action`.
- **Client side**: nothing to add — `client/msg-units.md` §4 already has
  0x67–0x6D in `bridge::msg::units` (queued unit handlers; dropped when
  the unit is not in the model).

## 2. Tests

- `monsters::mode_message::tests` (7): every §7.4 / §7.7 Test vector
  (death pair, mode 1, mode 6, mode 2 to point and to unit, mode 4 to
  unit, mode 8 to point, mode 0 without target), rule 3, the path-type
  rewrites and velocity clamp, mode 13 / mode 3 bytes, builder ids and
  sizes. `// Covers:` §7.4 r2–r7, §7.7 r3–r6.
- `wiring::action::unit_update::tests` (6, on the action fixture with
  the provider on): kill → code 8 in the next client pass, nothing until
  event 1 at f + 24 (24-frame DT at speed 256), code 9 in that tick,
  nothing after; unit event 13 once; `anim_complete`; the clean-up's
  flags per type; the no-path fatal; the target refresh and room test.
- **e2e** `crates/test-fixtures/tests/monster_death.rs` (CI): the
  synthetic install → `WorldSim` with the provider on → town room, a
  player and a `beast1` → `SimGame` + `Host::frame` (drain, tick, flush to
  the client). The kill (`reaction::kill`) runs in the drain before frame
  11's tick: the client receives exactly `69 01000000 08 0000 0000 00 06`
  in frame 11 and `69 01000000 09 1800 1400 00 00` in frame 34 (= 10 +
  24), nothing else from the monster; a second run is identical.
- **e2e** `d2-client/tests/e2e_full_loop.rs` (paths on, a real missile
  kill): the hit's frame now carries 0x69 code 8 (asserted byte for
  byte), the death end's code 9 arrives in a walk frame at f_hit + 4
  (new `Fx::due`, checked by `walk`), the client drops both (unit never
  announced). This file is red on the base (bridge dispatch table, see
  §6); run with the table check bypassed locally, it passes (as do
  `e2e_single_player`, `e2e_vendor`, `e2e_walk`).
- Changed expectations (spec answers, not weakenings):
  `walk/tests.rs::a_warp_within_the_level_sends_0x15_in_the_next_update_pass`
  (its `TODO(spec: 0x00553220 clean-up flags)` line: §7.5 step 3 clears
  flag-ex 0x10000, so the walk's mode set sends no second 0x15; now
  asserts the flags and the empty tick); `prop_worldsim.rs::fixture_reaches_the_wired_paths`
  (the monster is seen in DT and ends in DD 12 instead of staying in DT);
  `e2e_full_loop.rs`'s `used_skill` answers only for the player (the
  book is the player's; the dying monster read the player's skill in use
  and hit rule 3).
- M08: dropping 0x1 from the clean-up's unit flags fails
  `kill_sends_code_8_then_the_death_end_code_9` and
  `room_cleanup_clears_the_listed_flags`; the e2e does not catch it (the
  dead monster is never re-queued), by design it checks the pair.

## 3. Findings

1. **Test-vector label** (`intents-events.md` Test vectors, the first
   death vector): "cell (4757, 5461)" is the (a, b) of mode 0, which rule
   4 takes from the *path target*; the unit's cell is the mode-12
   vector's (4756, 5461). The test feeds the path target; the spec text
   could say "path target".
2. **Clean-up answered WS2 / PU1** (`HANDOFF` §7): unit flag 0x1 and
   flag-ex 0x10000 / 0x800 are now cleared in tick step 6, so the 0x15
   repeat and the per-tick 0x0F of PU1 are gone. WS1 (no 0x15 in the tick
   after a cross-room warp) is unchanged.
3. **Production hosts never reach DT**: `Pending::monster_death_start`
   (the DT start `0x005A6FF0`) has no body, so on a host whose `Pending`
   does not set the mode (e.g. `test_fixtures::game::Seams`) a kill never
   enters DT and no 0x69 is sent; the e2e fixtures set it.
4. The item update pass of `d2-server` runs after the tick, so it now
   sees unit flags 0x1 / 0x10 already cleared; that does not change it
   today (it reads +0xC8 bits 0 / 1 and the item / command flags), but
   wiring `item_unit_update` there needs it moved into the client pass
   first (IS2 / IS3, comment updated).

## 4. Left open (each has a `TODO` at its site)

1. §7.1 rule 2.1 / §7.2: a new monster's add messages (0xAC, 0x98, 0x21,
   part B's second mode message) — the add-message area of the join /
   room-switch work; only the §7.3 step-2 message is sent.
2. §7.3 rule 2 steps 1, 3–10 (0x15 for monsters, class / hireling
   messages, 0x0C hit, stat messages).
3. §7.4 rule 3's skill message: 0x4C / 0x4D have no field layout
   (`server-messages.tsv` `partial`); logged as `SkillMessage`.
4. §7.5 steps 1, 2, 4, 5 (in the server pass), 6, and of step 7 stat 29,
   the client record +0x34, monster data +0x5C bit 0x1, item flags
   0x20 / 0x2000.
5. §7.7 rule 3: the setter of mode 12 is not named (`0x00553570` used;
   `0x005A7C20`'s DD start `0x005A7390` has no body); `0x00623E00`,
   `0x00574370` / `0x00573780` / `0x00552FD0`, unit +0xB0's writers and
   `0x005A0180` are seams.

## 5. Local run queue (for `docs/HANDOFF.md` §5)

- Wired-host kill against the recording: a trace of a kill on the wired
  host compared with `20261006-015956` frames 2724 / 2748 (code 8 / 9, 24
  frames apart) once a host with a real death start, add messages and
  the trace converter (§6 rule 4) exist; until then the pair is
  unverified.

## 6. Gate

`CARGO_INCREMENTAL=0 cargo test --workspace --no-fail-fast`: 5,287
passed, 85 failed, 218 ignored — the 85 failures are exactly the base's
85 red d2-client tests (bridge dispatch table; same names before and
after, another session fixes them). `cargo clippy --workspace
--all-targets -- -D warnings` clean, `cargo fmt --all -- --check` clean,
`coverage.py --check` 8,351 claims / 0 errors, `spec_index.py --check`
OK, `methods.py check` OK, depcheck OK.
