# Handoff: routing of the open event and callback paths (`d2_sim::wiring`)

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Cloud implementation session, 2026-10-06, task class: wiring
architecture, high (METHODS M14). Branch `claude/wire-routing`, from
`main` at `edad871`. Repo only, synthetic tables, fixed seeds, no game
files (M09). Covers `HANDOFF.md` §2 step 7b and §7 J3, J6 as far as a
spec states them. For the coordinator to fold into `docs/HANDOFF.md` /
`docs/PLAN.md` (neither is edited here).

## 1. State

**Wired, unverified** (M02): the routes add no rule of their own; every
module behind them is a draft-spec implementation. All changes are in
`crates/d2-sim/src/wiring/{action,worldgen}/`; no module outside
`wiring` changed, no `d2-server` / `d2-client` file changed.

How the world state reaches the action hooks: `ActionHooks` has a new
slot `monster_world: Option<Box<dyn MonsterWorld<X>>>` (trait in
`wiring/action/monsters.rs`); `WorldState` implements `MonsterWorld`
(`wiring/worldgen/monster_world.rs`). `WorldSim` keeps owning its
`world` field and **lends** it into the slot around every timer event
(`EventDispatch::run_event`) and every tick hook it forwards to the
action systems (`WorldSim::lend`; `WorldSim::world` holds an empty
placeholder for the call, the real state comes back by downcast).
Population and init calls (`WorldSim::host` / `population` / `init`)
hold the world state directly and lend nothing. A route that finds the
slot empty takes its previous `Pending` answer, so `ActionSim` alone
(d2-server tests, the conformance replay) behaves exactly as before.

| Task item | Route now | Code | Test (`wiring/worldgen/tests/routing.rs`) |
|---|---|---|---|
| (a) `ActionHooks::init_kind` → `init::type_init` | `LifecycleHooks::init_kind` on `ActionHooks`: a monster allocation runs the monster type init `0x00574250` (`init.md` §5) on the lent world, wherever the allocation comes from (`View::allocate`, missiles, skills) | `action/units.rs`, `worldgen/monster_world.rs` | `action_allocation_runs_the_monster_type_init` (monster data class / level id, AI control with a function, flags 0x0A; the same allocation unlent gets none); `population_allocation_keeps_one_type_init` (population's own call stays the only one: unit seed and life equal to an action-code monster in an identical game, and the type init did draw) |
| (b) event 7 → `init::handle_event7` | `UnitHooks::monster_umod` on `ActionHooks` → dispatcher mode 2 (was on `WorldHooks`, removed: `WorldSim::run_event` now runs `ActionSim`'s unit dispatch with the world lent) | `action/units.rs`, `worldgen/dispatch.rs`, `worldgen/events.rs` | existing `worldgen/tests/events.rs` (umod 41 rescheduling at +75, umod 21 → `unhandled` mode 2, frozen drop) now run through the lent path, unchanged |
| (b) mode change `0x005A7C20` → dispatch 0 / 1 | `View::monster_set_mode` (every monster mode change in the wiring: AI, get-hit, the kill's death mode) runs the dispatcher in mode 0 then mode 1 after a successful mode set | `action/units.rs` | `monster_mode_change_runs_umod_modes_0_then_1` (umods 14, 15 → `0x005A3B50` mode 0, then `0x005A2D10` mode 1; unlent: none) |
| (b) combat 3 | `CombatView::monster_hit_hook` (`0x005A4390`, `damage.md` §5.2 step 9) → dispatcher mode 3; unlent → `Pending::monster_hit_hook` | `action/combat.rs` | `monster_hit_hook_runs_umod_mode_3` (umod 7 → `0x005A2530`) |
| (b) combat 4 | **not routed** (seam): `0x005A43A0` is called from the reaction `0x0057CEE0` at two sites, and `damage.md` §7.1 (call level only) does not list the call or place it | — | — |
| (b) missile creation 5 | `MissileHooks::unique_mod_missile` on `View` (`0x005A43B0`, `missiles.md` rule 28) → dispatcher mode 5 with the missile; unlent → `Pending::unique_mod_missile` | `action/missiles.rs` | `missile_hook_runs_umod_mode_5_on_the_missile` (umod 29 → `0x005A3610` with the missile as the callback's unit) |
| (c) `free_kind` → `MonsterStore::remove`, minion / owner maps | `ActionHooks::free_kind` also calls `WorldState::forget` on the lent world, so a removal by an action adapter (missile collide-kill, `View::remove`) frees it too; `WorldSim::remove_unit` = lent `View::remove` | `action/units.rs`, `worldgen/events.rs` | `action_removal_frees_the_world_state` (monster data and minion list gone; unlent: stale); existing `events::removal_frees_the_world_state` unchanged |
| (d) `monster_flag` `0x005A0180` | `ActionHooks::monster_flag(unit, mask)`: monster data type flags & mask; no monster data → `Pending::monster_flag`. Used by `CombatView` (`SkillUnits` / `CombatWorld`) and the AI's `is_unique` / `is_champion` | `action/monsters.rs`, `action/combat.rs`, `action/ai.rs` | `monster_data_queries_read_the_lent_world` |
| (d) monster level | `AiUnits::monster_level` on `View`: stat 12 (`level`, `init.md` §7 rule 4) unit total for a unit with monster data; else `Pending` (TODO R2) | `action/ai.rs` | same test (7 after a set; unlent 0) |
| (d) AI state | **not routed** (seam): monster data `dwAiState` (+0x54) has no writer in any spec (`ai.md` OQ5) and `MonsterData` has no field for it; `Pending::ai_state` stays | — | — |
| (e) `player_action_frame`, timer types 5, 8, 9 | already routed at base (`wire-open-seams`, `e2e-combat-path`): `ActionHooks` → `Pending::{skill_event, action_frame}` → `interaction::skill_events::{route, action_frame}` → `use_::{active_state_event, periodic_event, item_aura_event, attack_frame_event}` on `UseView`, for a seam value that also implements `UseRest`. Through `WorldSim` the events now reach them via the lent `ActionSim` dispatch (same hooks) | `interaction/skill_events.rs` (unchanged) | `interaction/tests/skill_events.rs` (5, 8, 9), `action/tests/death.rs` + the d2-client e2e (action frame) |
| (e) type 12 (cooldown expiry) | already routed: the unit dispatch's type-12 handler `0x00580800` expires the state-121 list (`stat-lists.md` §10.4) | — | `interaction/tests/skill_use.rs::cooldown_list_and_its_expiry_timer_are_real` |
| (e) type 14, the delay list's remove callback `0x0056E900` | **not routed** (seam): `use.md` §6: type 14 is never scheduled in 1.14d and neither body is specified | — | — |

M08: each route's test runs the same call once more with no world lent
and asserts the effect is gone. By hand: cutting only the mode-change
umod call fails only `monster_mode_change_runs_umod_modes_0_then_1`;
cutting only the free's `forget` fails only
`action_removal_frees_the_world_state`; cutting only the combat route
fails only `monster_hit_hook_runs_umod_mode_3`; cutting the `init_kind`
route fails all seven (each needs the action-allocated monster's data).

## 2. Code map rows (for `docs/HANDOFF.md` §3)

| Path | What | Spec |
|---|---|---|
| `wiring/action/monsters.rs` | `MonsterWorld<X>` (type init, umod dispatcher, forget, monster data, `into_any`), `umod_mode`, `ActionHooks::{with_monster_world, monster_data, run_umods, monster_flag}` | `init.md` §5, §22; `units.md` §3.1, §3.2, §4.6 |
| `wiring/action/mod.rs` | + `ActionHooks::monster_world` (lent slot), private `monster_world_out` | |
| `wiring/action/units.rs` | + `init_kind` (monster type init), `monster_umod` (event 7), `free_kind` → world forget, `View::monster_set_mode` → umod modes 0, 1 | `units.md` §3, §4.6; `init.md` §5, §22 |
| `wiring/action/{combat,missiles,ai}.rs` | umod mode 3, mode 5; `monster_flag`, `is_unique`, `is_champion`, `monster_level` from monster data | `init.md` §7, §22; `damage.md` §5.2; `missiles.md` rule 28 |
| `wiring/worldgen/monster_world.rs` | `MonsterWorld` for `WorldState`; `WorldSim::{lend, with}` | `init.md` §5, §22 |
| `wiring/worldgen/events.rs` | `WorldState::forget`, `WorldSim::remove_unit` (lent) | `units.md` §3.2 |
| `wiring/worldgen/dispatch.rs` | `WorldSim`: timer events and the forwarded tick hooks with the world lent (`lent!` macro); population hooks on `WorldHost` | `tick.md` §3, §4, §5 |
| `wiring/worldgen/tests/routing.rs` | 7 route tests (declared from `worldgen/mod.rs` with `#[path]`, since `worldgen/tests/mod.rs` is not this session's file) | |

## 3. Signature changes

Nothing outside `crates/d2-sim/src/wiring/{action,worldgen}` changed.
Inside `wiring`, public API a coordinator merging other branches should
know:

1. `ActionHooks<X>` gained the public field `monster_world` (and a
   private one): no struct literal of `ActionHooks` exists in the
   workspace (all use `ActionHooks::new`), but one in another branch
   would break.
2. **Removed:** `wiring::worldgen::events::WorldHooks` (the delegating
   unit hooks of `wire-open-seams`). Its job is now the lent world in
   `ActionHooks`; a branch that adds a `UnitHooks` method no longer has
   to delegate it there. Nothing in the workspace used it outside
   `events.rs`.
3. New: `wiring::action::{MonsterWorld, monsters::umod_mode}`,
   `ActionHooks::{with_monster_world, monster_data, run_umods,
   monster_flag}`, `WorldSim::{lend, with}`.
4. Behaviour of `ActionHooks` with a lent world: `init_kind`,
   `free_kind`, `monster_umod`, `View::monster_set_mode`,
   `CombatView::{monster_flag, monster_hit_hook}`,
   `View::unique_mod_missile`, AI `is_unique` / `is_champion` /
   `monster_level` no longer go to `Pending` (for a unit with monster
   data); without one, unchanged.
5. `WorldSim::run_event` and the forwarded tick hooks run with the world
   lent: during those calls `WorldSim::world` is a placeholder (a host
   reading it from inside a hook would see an empty state).

## 4. Seams reached (stop here; no spec)

- **Umod mode 4** (`0x005A43A0` in the reaction `0x0057CEE0`): not
  placed by `damage.md` §7.1 (call level only, OQ3); stays inside
  `Pending::reaction`.
- **AI state** (`dwAiState`, `ai.md` OQ5): no writer specified.
- **`is_boss` `0x0063E9F0`, superunique `0x005A03A0`, minion owner
  `0x0058F0D0`**: not routed. `0x0063E9F0`'s test is not specified;
  `0x005A03A0` reads "hcIdx ≠ −1" but `init.md` does not state the value
  of +0x26 for a monster that is not a superunique (`MonsterData`
  defaults it to 0, which is a real row); `0x0058F0D0` reads monster
  data +0x2C (`ai.md` §3.1) while `WorldState::owners` is written by
  `0x005DD330`: whether they are one field is not stated.
- **Type 14 / callback `0x00554570`, remove callback `0x0056E900`**
  (`use.md` §6): no bodies.
- **WG8 / J6 (first think of a created monster, which step starts a
  mode)**: still not stated by `init.md` §5 or `units.md` §4.6; nothing
  added. `type_init` does not set a mode, so an action-allocated monster
  thinks only once something changes its mode or schedules its think.
- **Message paths from `d2-server`** reach `ActionSim` directly
  (`ActionEvents for WorldSim` returns `&mut self.action`), so a skill
  message handled there runs without the world lent: monster routes in
  that path answer `Pending` (today a cast only starts a mode; its
  missile is created by the action frame inside a timer event, which is
  lent). Fix (d2-server's file): run the handler inside
  `WorldSim::lend`, e.g. `ActionEvents::with_action(&mut self, f)`
  instead of returning the reference.
- **`wiring::economy`** (not this session's files): `death.rs` reads
  `h.x.monster_flag` / `h.x.superunique` directly; it should call
  `ActionHooks::monster_flag` to see the lent world's flags.
  `QuestTick`'s `UnitSide for WorldSim` hands out the action hooks
  without lending (quest code that allocates or removes a monster would
  miss the world state).

## 5. Questions (each has a `TODO` at its site)

- **R1** (`action/units.rs` `View::monster_set_mode`; `init.md` §22,
  `units.md` §4.6): where in `0x005A7C20` the two dispatcher calls
  (`0x005A4350` mode 0, `0x005A4360` mode 1) sit is not stated. The
  mode-1 callbacks read the new mode, so they follow the start
  function. Both run after the whole mode set (start, animation,
  schedule), mode 0 first: a type-7 event they schedule follows the
  mode's animation events in the timer queue, which decides the run
  order of events on the same frame.
- **R2** (`action/ai.rs` `monster_level`; `ai.md` §2.4 step 2): "level"
  of the teleport heal: the getter is not named; stat 12 unit total.

## 6. J4 (left alone): how to settle it

`d2-server`'s `handlers::skills::World` + `SkillSeams` and `d2-sim`'s
`wiring::interaction::UseView` both implement `SkillUnits`, `ManaUnits`,
`SkillFunctions`, `UseMissiles`, `UseWorld` (and `World` also
`VitalsUnits`, `LearnUnits`). Proposal: keep `UseView` as the one
provider (it is what the timer events use, J3); `WiredSkills::run`
builds it through `ActionSim::skill_use` (inside `WorldSim::lend` for a
world host); the parts only the server has (the staged positions and
player data +0x168, `same_act` / `within_reach` of SK8) move into the
seam value's `UseRest` / `Pending` methods, so `SkillSeams` becomes that
value; `World` and its duplicate impls go. One owner: the d2-server
skills session, after this branch (it needs `WorldSim::lend`).

## 7. Local checks to queue (`docs/HANDOFF.md` §5)

1. With the spawn RNG recording of `init.md` OQ4 (one population pass,
   rng hook with caller addresses): a monster created by action code
   through the lent world must take the same unit-seed draws as
   population's creation. Command: replay the recorded pass through
   `WorldSim` with recording-backed `WorldPending`; expect the per-unit
   draw sequences equal (`cargo test -p conformance` once the replay
   exists).
2. With a recording of a unique monster with a mode-1 umod dying (e.g.
   fire enchanted, umod 9; group A, `record_tick.py` with timer hooks):
   the type-7 timer's frame (death frame + 4) and its position among the
   death animation's timers in the queue; settles R1.
3. Re-run `cargo test -p conformance --test tick_replay` after any change
   to `WorldSim` / `ActionSim` routing; expected: 7 pass, unchanged (the
   replay uses its own hooks).

## 8. Gate results

On this branch's head (all pass): `cargo fmt --all -- --check`; `cargo
clippy --workspace --all-targets -- -D warnings`; `cargo test
--workspace` (1873 pass, 0 fail, 61 ignored; d2-sim 1213 pass, 5 ignored, of which 7 new; conformance `tick_replay` 7 pass, unchanged); `cargo run -p depcheck` (8 crates OK);
`python3 tools/spec_index.py --check`; `python3 tools/methods.py check`
(21 methods OK); `python3 tools/coverage.py --check` (3197 claims, 0
errors); `python3 tools/coverage.py --selftest` (ok). Non-source files in
the diff: this note only.
