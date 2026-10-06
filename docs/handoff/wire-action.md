# Handoff: wiring of the action seams (`d2_sim::wiring::action`)

Branch `claude/wire-action`, from `claude/bold-ptolemy-jvyvxy` at
`ed7236e` (cloud session, 2026-10-06). Task class: integration from
clear specs, medium (METHODS M14). Scope of every claim: this branch,
synthetic tables and a synthetic DRLG level, no game files (M09). For the
coordinator to fold into `docs/HANDOFF.md` and `docs/PLAN.md` (not edited
here).

## 1. State

**Wired, unverified** (M02): every module behind these adapters is a
draft-spec implementation; the adapters add no rule of their own.

Gate (all pass): `cargo fmt --all -- --check`, `cargo clippy -p d2-sim -p
conformance --all-targets -- -D warnings`, `cargo test -p d2-sim -p
conformance` (d2-sim 514 pass, 3 ignored; the tick trace replay passes
unchanged), `cargo run -p depcheck`, `python3 tools/spec_index.py
--check`, `python3 tools/methods.py check`, `python3 tools/coverage.py
--check`. `cargo check -p d2-server` also builds (new tick hook has a
default).

19 integration tests in `crates/d2-sim/src/wiring/action/tests/` run the
real modules together (fixture: act 0 DRLG, level 2 generated as two 8×8
preset rooms, streamed into the real `UnitLists`; real `StatLists` on the
synthetic itemstatcost of `stats::tests`; real timer queue; fixed seeds):

| Seam pair | Test(s) | Spec vector / rule |
|---|---|---|
| combat ↔ stats/units | `combat::hit_test_reads_real_stats_and_draws_on_the_attacker_seed` | `hit.md` vectors: chance 83, seed (1, 0) → 85 miss, 82 hit / 83 miss |
| | `combat::missile_damage_lowers_real_life_after_resistances` | `damage.md` vector 2560 / DR 3 / resist 20 → 1433 off real life |
| | `combat::damage_reschedules_monster_regen_and_kills_at_zero_life` | `damage.md` §5.2 steps 14–15 (type-3 timer at f + 1, life 0, events 10/9) |
| | `combat::state_lists_expire_through_the_type_12_event` | state list on real `StatLists`, expired by the unit dispatch (`stat-lists.md` §10.4) |
| missiles ↔ combat/units/DRLG | `missiles::missile_flies_hits_a_real_unit_and_deals_real_damage` | §R2 creation (real allocation, every-tick event), §R4 flight over the real grid, §R5 to-hit on the owner seed, §R6 damage 2560 applied, collide-kill removal (unit, timers, stat list, store) |
| | `missiles::missed_to_hit_removes_the_missile_without_damage` | §R5 step 5 |
| | `missiles::missile_barrier_in_the_room_grid_removes_without_a_hit` | `rooms.md` §10.4 step 4 (fill LOS → 0x4), §R4 step 6 |
| | `missiles::missile_expires_after_range_runs` | §R7 rule 2 (Range 50 → 50 runs) |
| AI ↔ units/timers | `ai::think_runs_through_the_unit_dispatch_and_reschedules` | `ai.md` Idle every 200, through the real queue |
| | `ai::frozen_monster_drops_think_and_reset` | `tick.md` §5.6 freeze drop (types 2 and 10) |
| | `ai::ai_mode_change_runs_the_monster_mode_set` | `units.md` §4.6 neutral start (think at f + aidel), state-54 refusal |
| | `ai::think_scheduled_with_state_54_clears_it_first` | `tick.md` §5.2 rule 4 / `ai.md` §1.1 |
| DRLG ↔ act room lists | `rooms::streamed_rooms_are_active_in_the_act_room_list` | `unit-order.md` §4 prepend order, `rooms.md` §6 adjacency, room search `0x00463740` |
| | `rooms::inactive_rooms_are_removed_by_tick_step_9` | `rooms.md` §7.2 / §8 (removed on the 11th pass, frame 132) |
| | `rooms::client_room_change_keeps_its_rooms_active` | `tick.md` §6.5 + `rooms.md` §4.1 |
| waypoints ↔ DRLG | `waypoints::operate_sets_the_level_bit_and_schedules_endanim` | `waypoints.md` §5.2 |
| | `waypoints::init_consumes_an_arrival_in_the_object_room` | §5.1 |
| | `waypoints::spawn_search_returns_an_active_room_of_the_level` | `levels.md` §10 |
| end to end | `e2e::tick_runs_the_combined_dispatcher_deterministically` | 30 × `tick::tick`: client joins room A, monster thinks at 2 → 202, its missile hits the player on tick 3; two runs identical |

## 2. Code map rows (for `docs/HANDOFF.md` §3)

| Path | What | Spec |
|---|---|---|
| `crates/d2-sim/src/wiring/mod.rs` | module root (`action`; the economy session adds `economy`) | |
| `crates/d2-sim/src/wiring/action/mod.rs` | `ActionTables`, `DrlgWorld`, `ActionHooks` (shared state: DRLG, missile/AI stores, combat lists, hit-class byte, game seed, waypoint records), `View`, `WiringError` | `tick.md` §3, §5.6 |
| `crates/d2-sim/src/wiring/action/dispatch.rs` | `ActionSim`: the one `EventDispatch` + `TickHooks` (steps 9, 10, client room change); `with` / `missiles` / `ai` / `combat` / `waypoints` entry points | `tick.md`, `units.md` §5 |
| `crates/d2-sim/src/wiring/action/units.rs` | `UnitHooks` + `LifecycleHooks` + `StatHost` for `ActionHooks`; `View` helpers (seed, stats, states, state lists, allocate/remove, monster mode set) | `units.md` §3–§6, `stat-lists.md` |
| `crates/d2-sim/src/wiring/action/combat.rs` | `CombatView`: `CombatWorld` + `SkillUnits` | `hit.md`, `damage.md`, `levels.md` |
| `crates/d2-sim/src/wiring/action/missiles.rs` | `MissileWorld` on `View`; `damage_record`, `result_bits` | `missiles.md` §R2–§R6 |
| `crates/d2-sim/src/wiring/action/ai.rs` | `AiHost` on `View` | `ai.md` |
| `crates/d2-sim/src/wiring/action/rooms.rs` | DRLG lookups (room → DRLG room, level, town, grids, room search) and the step 9/10/room-change bodies | `rooms.md` §4–§10, `levels.md` §9 |
| `crates/d2-sim/src/wiring/action/waypoints.rs` | `WaypointView`: `WaypointWorld` | `waypoints.md` §5–§7, `levels.md` §10 |
| `crates/d2-sim/src/wiring/action/pending.rs` | `Pending`: every seam call without a provider, defaults = nothing; `NoPending` | (see §4) |
| `crates/d2-sim/src/wiring/action/tests/` | fixture + integration tests (§1) | |

## 3. How it is put together

- `ActionSim { sys: UnitSystem<ActionHooks<X>> }`. Timer events go to
  the unit dispatch of `units.md` §5 (handler tables + the monster freeze
  drop of `tick.md` §5.6). Its hooks route: missile events →
  `missiles::class_handler`; monster type 2 → `monsters::ai::think`;
  type 10 → the AI reset (monster data, pending).
  **Not** the `MonsterDispatch { next: MissileDispatch { next: … } }`
  chain of `impl-monsters.md` §3: that chain runs type 10 for a frozen
  monster (`MonsterDispatch` checks the freeze for type 2 only;
  `monsters::ai::tests::freeze_drops_thinks_and_type_10_resets` expects
  it), while `tick.md` §5.6 (conformance-passing) drops types 0, 1, 2, 6,
  7, 9, 10, 11, 13, 14 when frozen. Routing through the unit dispatch
  follows `tick.md`. Open question W1.
- Missile and AI stores are lent (`Option::take`) to the module for the
  call; a re-entrant call logs `WiringError::Reentrant` instead of
  aliasing.
- Units are `UnitId`s everywhere; items are `UnitId`s too (no item
  provider; all item queries pending).
- Errors (unit operations, DRLG) collect in `ActionHooks::errors` and
  `UnitSystem::errors`; tests assert both empty.

## 4. Remaining seams (pending) and why

All in `wiring::action::Pending` with the narrowest default; each names
its owner. None has a written-and-implemented provider in `d2-sim`:

- **Path and position** (`units.md` path, not written): position,
  placement at allocation, size, velocity / targets / masks / build /
  acceleration, unit step, crossed sub-tiles, cached collision word, AI
  path steps / blocked / stop / walk-in-radius. Unit run-time collision
  bits (0x40, 0x80, 0x100 …) are also movement's; tests set them.
- **Hostility `0x00554200`, alignment `0x006259B0`, size `0x00620510`,
  moving mode `0x00622D00`, melee range / test, line tests, reach,
  teleport spot, LOS draw, last-dead, `0x0046C140`, `0x00645270`,
  montype nest**: named by the specs, not specified.
- **Monster data** (`monsters/init.md`, not implemented): type flags,
  boss/demon/undead/prime evil/revived, monster level, AI state, vision,
  type-10 reset, interaction, last attacker, monster hooks, unique-mod
  missile hook, player-count bonus.
- **AI targets and skills** (ai.md OQ6, OQ7; `skills/use.md`): target
  nodes, forced/good/secondary targets, scans, skill usable, current
  skill.
- **Skill code**: missile damage setup `0x0059F900`, init callbacks,
  server-damage functions 1–14, curse, thorns.
- **Unit events `0x005C0C30`, reaction `0x0057CEE0` (and the kill),
  overlays, animation rate**: no event registry, reaction specified at
  call level only, animation specs not written.
- **Items** (economy group): item stat getter, skill lists, weapons,
  body items, item types, durability, shields, weapon class, dual wield.
- **Objects / interaction / messages** (waypoints): object modes,
  interact info, sounds, sends, warp (act change, free coordinates,
  placement), arrival player mode.
- **UnitHooks not routed** (keep the trait defaults): animation records
  and rate, sequences, player mode hooks, monster mode functions and
  class records, skills hooks, objects, item replenish, life-fraction
  message, hover.
- **TickHooks not routed** (defaults): environment, population, presets,
  messages, quests (economy), items, compress units.

## 5. Changes outside `wiring/` (smallest, each provably needed)

1. `crates/d2-sim/src/lib.rs`: `pub mod wiring;`.
2. `tick::TickHooks::room_deactivated(game, act, room)` (default nothing),
   called in step 9 right after `lists.deactivate_room(room)`. The rest
   of `0x0061A910` is the DRLG's (`rooms.md` §8.2; tick open question T5,
   the seam request in `impl-drlg.md`); without it the DRLG removal could
   not run in the same pass. The trace replay is unchanged (default
   no-op).
3. `monsters::ai::AiUnits::level_id(&self, game, unit)` (was `(unit)`):
   the provider reads the unit's room from the game's lists and the
   room's level from the DRLG (`levels.txt` row of the unit's level);
   without `game` no real provider can answer. Two call sites
   (`functions.rs`, `target.rs`) and the test fake updated.
4. `missiles::MissileStore::remove(m)`: unit removal by other code
   (`0x00555600` → missile free `0x0059F8E0`, §R3.8) needs to drop the
   data; the store had no public removal (`impl-monsters.md` §3 "stale
   entries").
5. `missiles`: re-export `MissileParams` (creation parameters; the
   callers — skills — sit outside the module) and `result_flag` (the
   adapter maps the missile result bits to `damage.md` §1 bits).

## 6. Open questions (each has a `TODO` at its site)

- W1. `MonsterDispatch` runs type 10 for frozen monsters; `tick.md` §5.6
  drops it. The combined dispatcher follows `tick.md` (§3 above); the AI
  module test should be revisited by its owner.
- W2. `missiles.md` §R6.1: the `avoid` / `block` arguments of the
  block/dodge call and the hit flags from missile data flags 1, 2 are not
  stated; neither is applied (no block/dodge draw on missile hits yet —
  a draw-order difference to 1.14d until settled).
- W3. `missiles.md` §R6.2: where the 103/104/106 bypass flags go in the
  damage record (no field in `damage.md` §1); not carried. The "rolled"
  hit flag (0x20) is not set by the missile roll (not stated), so event 6
  `domissiledamage` never fires on missile hits.
- W4. Missile crit → result bit 0x2000 (critical strike, `damage.md` §1),
  read as the record's crit.
- W5. `0x0064D9B0` / `0x0064EBA0` size footprint not specified: the
  single sub-tile is read / cleared for every size.
- W6. `0x00641CB0` unit search order: rooms in adjacency order (room
  first), units in room-list order, filtered by position.
- W7. `0x00648EB0` with a moving path: the cached word is the path's
  (pending); without one the grid at the current position is read.
- W8. State lists (`justhit`, combat's `create_state_list`): allocation
  flags and attach `reset` not stated (flags 0, reset = 1); `justhit`
  list owner not stated (the hit unit).
- W9. AI `change_mode` "failed": read as the mode set returning an error
  (state 54, bad mode); `0x005A7C20` falls into the neutral start itself
  when a start function fails and reports nothing.
- W10. AI life % rounding: `100 · life / max`, truncating.
- W11. `knockback_to_gethit`: last-hit class 160 not stored (unit field
  not described).
- W12. `calc-expressions.md` §3.5 `stat(s, mode)`: mode 1 base, other
  modes total (stats OQ5).
- W13. Client level change: `0x00543B90` / `0x00537340` not specified;
  a change between acts is not handled (client keeps its room).
- W14. `0x00619E50` vs `0x0066B2B0`: whether the waypoint arrival test's
  search streams the room is not stated; the DRLG search (which streams)
  is used.
- W15. Missiles do not count toward a room's allied count (not stated,
  `unit-order.md` §5.2).
- W16. The game seed of `rng.md` §5.3 lives in `ActionHooks::game_seed`
  (no field on `Game` yet); hosts must seed it at game creation.

## 7. Checks to queue (local, `docs/HANDOFF.md` §5)

1. Once tick recordings with positions exist: replay a missile lifetime
   with a recording-backed `Pending` path (positions, crossed sub-tiles)
   through `ActionSim` and compare per-GUID runs and the RNG draws on the
   owner / missile / monster seeds (`impl-monsters.md` §6 item 3).
2. Re-run `cargo test -p conformance --test tick_replay` after every
   change to `ActionSim`'s tick hooks (only steps 9/10 and the client
   room change are routed; the replay's own hooks do not use them).

## 8. Coordination

- The economy session creates `crates/d2-sim/src/wiring/mod.rs` too:
  expect a trivial merge (`pub mod action;` + `pub mod economy;`), and the
  same for the `pub mod wiring;` line in `lib.rs`.
- `ActionHooks::waypoints` (player records) is player data the units
  group will own; move it when player data exists.
