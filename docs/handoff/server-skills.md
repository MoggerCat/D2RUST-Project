# Handoff: `d2-server` skill and combat intent handlers

Branch `claude/server-skills`, from `claude/bold-ptolemy-jvyvxy` at
`1470723` (cloud session, 2026-10-06). Task class: implementation from
clear specs, medium (METHODS M14). Scope of every claim: this branch,
synthetic tables, no game files (M09). For the coordinator to fold into
`docs/HANDOFF.md` and `docs/PLAN.md` (not edited here).

## 1. State

**Implemented, unverified** (M02): the handlers run the draft-spec
`d2-sim` functions (`skills::use_`, `skills::levels`, `combat::vitals`)
on the wired sim (`d2_sim::wiring::action::ActionSim`); no recording has
been replayed through them.

Gate (all pass): `cargo fmt --all -- --check`, `cargo clippy -p
d2-server -p d2-sim --all-targets -- -D warnings`, `cargo test -p
d2-server -p d2-sim` (d2-server 54, of which 11 new; d2-sim 828 + 5
ignored, unchanged), `cargo run -p depcheck`, `python3
tools/spec_index.py --check`, `python3 tools/methods.py check`, `python3
tools/coverage.py --check` (0 errors).

## 2. Client ids (skill / combat) and their owners

| Id | Name | Owner spec | Status here |
|---|---|---|---|
| 0x05–0x07, 0x0C–0x0E | skill at point / on unit | `skills/use.md` §1–§3 | handled: `use_::handle_message` |
| 0x08–0x0A, 0x0F–0x11 | hold forms | `skills/use.md` §1 rule 6 | handled: `use_::handle_hold` (via `handle_message`) |
| 0x0B | Unused0B | `client-messages.tsv` ("nothing, returns 0") | handled: returns 0, no `pierce_idx` (§2.4 rule 5) |
| 0x3A | AddStatPoint | `combat/vitals.md` §2 | handled: `vitals::handle_add_stat_point` |
| 0x3B | AddSkillPoint | `skills/levels.md` §6.4 | handled: `check_skill_point` → `spend_skill_point` → step 5 seam |
| 0x3C | SelectSkill | `skills/use.md` §7; `intents-events.md` §2.4 rule 7 | handled: `use_::select_skill` |
| 0x12 | EndInferno | none (TSV request column only) | stub kept |
| 0x41 | Resurrect | none (TSV request column only) | stub kept |
| 0x51 | BindHotkey | `intents-events.md` §2.4 rule 7 gives the field checks only; where hotkeys are stored is unwritten | stub kept |
| 0x01–0x04 | walk / run | no pathing spec | stub kept (task scope) |

Not counted as skill ids: 0x60 (weapon switch, items), 0x48 (busy
state), 0x46/0x47/0x61/0x62 (mercenary). The table is
`handlers::skills::IDS`; `ids_match_client_tsv` checks it against
`client-messages.tsv` (names; `kind` = handler for handled ids), with a
perturbation test (M05, M08).

S→C messages: none of these handlers has an S→C message with a layout.
0x15 (resync, 11 bytes) goes to `SimGame::resyncs` like the
dispatcher's; 0x5A ("can't do that", `5A 0E 01 …`, `use.md` OQ9) is
recorded in `SkillHost::unsent`. Neither is queued until
`server-messages.tsv` gives the layout. Every test asserts the client
receives exactly nothing.

## 3. Code map rows (for `docs/HANDOFF.md` §3)

| Path | What | Spec |
|---|---|---|
| `crates/d2-server/src/adapters/handlers/mod.rs` | handler modules (skills; items and world sessions add theirs) | `sim/intents-events.md` |
| `crates/d2-server/src/adapters/handlers/skills/mod.rs` | `IDS` (owner, status), `SkillHost<D>` (object-safe), `Call`, `Staged`, `Handled`, routing `handle` (staging, point-accept write-back, resync) | `intents-events.md` §2.4; `use.md` §1, §7; `levels.md` §6.4; `vitals.md` §2 |
| `crates/d2-server/src/adapters/handlers/skills/wired.rs` | `WiredSkills<S>`: `SkillHost<ActionSim<X>>`; `run` (id → `d2-sim` handler), `add_skill_point` (0x3B) | same |
| `crates/d2-server/src/adapters/handlers/skills/world.rs` | `World`: `SkillUnits`, `ManaUnits`, `SkillFunctions`, `UseMissiles`, `UseWorld`, `LearnUnits`, `VitalsUnits` on the game + wired unit system + staged facts + `SkillSeams` | `use.md`, `levels.md`, `vitals.md` |
| `crates/d2-server/src/adapters/handlers/skills/seams.rs` | `SkillSeams`: every seam part with no provider (defaults = nothing, as `wiring::action::Pending`) | |
| `crates/d2-server/src/adapters/handlers/skills/tests.rs` | 11 tests through `Host::frame` on `SimGame<ActionSim<NoPending>>` | |

## 4. How it is put together

- `SimGame` got one field, `skills: Option<Box<dyn SkillHost<D> + Send +
  Sync>>` (default `None`), and one line at the top of `handle`: if
  `handlers::skills::handle` returns a code, that is the result. Without a
  host every id keeps the stub behaviour, so existing tests and the
  `Unspecified` sim are unchanged. `adapters/mod.rs` got `pub mod
  handlers;`. `d2-server` got `d2-data` as a dev-dependency (test
  tables). No `d2-sim` change; no accessor was needed.
- The dispatcher has already gated, size-checked and parsed (§2.2–§2.4).
  `d2-sim`'s handlers re-run their own validators (`validate_point`,
  `validate_unit`) on the same staged facts (`Staged`: the player's player
  data +0x168, the staged positions, owned-item and other-act results of
  `SimGame::unit_target`), so they agree with the dispatcher and are
  idempotent; the point validator's +0x168 write comes back through
  `Handled::point_accept`.
- Provider of each seam part (`world.rs` header): frame, unit lookup,
  timers → `Game`; stats, states, state lists, seeds, unit type / class /
  mode / flags → the action wiring's `CombatView` / `View` (real
  `StatLists`, unit records); items → the wiring's `Pending` (as for
  combat); mode starts → `units::modes` (`set_mode` + `animate` for
  `start_mode`; `player_start` for the reenter-1 `set_mode`); the
  cooldown list → `StatLists` per `use.md` §6 (flags 2, remove callback
  `0x0056E900`); everything else → `SkillSeams`.
- `WiredSkills::vitals` holds the `charstats` / `experience` tables;
  skills tables come from `ActionHooks::tables`.

## 5. Findings and open questions (each has a `TODO` at its site or is a wiring gap)

1. **Mode starts fail after the mode set on the wired sim**: the action
   wiring routes no AnimData record (`UnitHooks::anim_record` default,
   `wire-action.md` §4 "animation records"), so `units::modes::animate`
   returns `AnimError::NoRecord`: the mode is set (unit +0x10 = SC) and
   flag 0x40 cleared, but no type-0 / type-1 timers are scheduled. The
   error is recorded in `ActionHooks::errors`; tests assert it. Until
   the animation spec is wired, no skill do runs from a message.
2. **Action frames and periodic events are not routed**: `ActionHooks`
   does not route `player_action_frame` (`0x00580460`) to
   `use_::attack_frame_event`, nor timer types 5, 8, 9, 12 to
   `use_::{active_state_event, periodic_event, item_aura_event}` /
   the cooldown expiry. 0x3C schedules the type-8 timer (Might: 1251 from
   1234, tested), but its handler does not run. Owner: the action wiring
   (it needs the `SkillHost`'s seams, so the routing likely belongs in
   `d2-server` or a `UseWorld` provider in `d2-sim`).
3. `use.md` §1 rule 2 leaves the codes of a bad type and a failed
   distance open; `intents-events.md` §2.4 rule 4 gives them (2 and 1).
   `wired::run` maps `MsgResult::Unspecified` with that rule (never
   reached: the dispatcher refuses first). Spec edit suggestion: point
   `use.md` §1 rule 2 to `intents-events.md` §2.4 rule 4.
4. `levels.md` §6.4 step 5 ("the handler then calls …"): read as running
   after the spend whether or not a level was added; result 0 (OQ5).
5. `use.md` §4: where the mode starts store the point / unit target is
   not stated; `start_mode` does not keep it.
6. The reenter-1 mode set goes through `units::modes::player_start`,
   which asks the request-check hook that reenter 1 skips in 1.14d; the
   wiring's hook accepts, so the result is the same today.
7. Cooldown list attach `reset`: not stated (`stat-lists.md` §8.1); 1 as
   the action wiring's state lists.
8. `same_act` for a pair the message did not stage falls back to the unit
   records' act (unit +0x18); `within_reach` uses staged positions, else
   `SkillSeams::position`.
9. `SimGame::handle`'s doc still says every handler is a stub and carries
   the `pierce_idx` TODO (now done by `use_::handle_message`): left
   unedited to keep the shared lines conflict-free; fix when folding.

## 6. Checks to queue (local, `docs/HANDOFF.md` §5)

1. Group A (player), with `record_packets.py` and a stat hook: cast a
   right skill at a point, on a monster, with no mana, and spend stat /
   skill points; expect per message the same result code (debugger
   return of the handler), `pierce_idx` (stat 328) +1 per skill message
   (also failed uses, not for 0x0B or refusals before the handler), the
   0x5A bytes after the no-mana cast (fills `use.md` OQ9), and the
   vitals deltas of the spec's vectors. Replay through `Host::frame` with
   a recording-backed `SkillSeams`.
2. After finding 1 is wired: a recording's type-0 / type-1 timers after a
   cast message (`use.md` vector "request at frame 886 → type 0 at 892,
   type 1 at 900") through this path.

## 7. Coordination

- `handlers/mod.rs` holds only `pub mod skills;`; the items and world
  sessions add their `mod` lines (trivial merge).
- If they add a host field to `SimGame` the same way, expect a trivial
  merge in the struct, `with_events` and the top of `handle`.
- `crates/d2-server/Cargo.toml` `[dev-dependencies] d2-data`: may merge
  with an identical line from another session.
