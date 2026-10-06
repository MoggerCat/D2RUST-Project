# Handoff: open wiring seams, second pass (`d2_sim::wiring`)

Cloud implementation session, 2026-10-06, task class: integration from
clear specs, medium (METHODS M14). Branch `claude/wire-open-seams`, from
`main` at `fd37fba`. Repo only. Scope of every claim: this branch,
synthetic tables, fixed seeds, no game files (M09). For the coordinator
to fold into `docs/HANDOFF.md` / `docs/PLAN.md` (not edited here).

## 1. State

**Wired, unverified** (M02): every module behind these adapters is a
draft-spec implementation; the adapters add no rule of their own.
All changes are inside `crates/d2-sim/src/wiring/`; no module outside
`wiring` changed.

Gate (all pass, see §8): `cargo fmt --all -- --check`, `cargo clippy
--workspace --all-targets -- -D warnings`, `cargo test --workspace`,
`cargo run -p depcheck`, `python3 tools/spec_index.py --check`,
`python3 tools/methods.py check`, `python3 tools/coverage.py --check`.
The tick replay (`cargo test -p conformance`) passes unchanged.

## 2. Closed seams

| Seam (from) | Now | Code | Integration test (real modules, fixed seed) |
|---|---|---|---|
| Timer events 5, 8, 9 → skill use (`wire-interaction.md` §6) | `ActionHooks`'s `active_state`, `periodic_skills`, `apply_item_aura` hand the event to `Pending::skill_event`; a seam value that also implements `UseRest` routes it to `interaction::skill_events::route` → `skills::use_::{active_state_event, periodic_event, item_aura_event}` on `UseView` | `action/units.rs`, `action/pending.rs` (`SkillEvent`), `interaction/skill_events.rs` | `interaction/tests/skill_events.rs`: aura form (−1) on the real queue runs the do core and reschedules at `period` (frames 1, 6, 11 → next 16); a non-aura right skill is not rescheduled; event 9 reads stat 151 (layer = skill) and runs the do core, stat 151 = 0 cancels the unit's type-9 events; event 5 calls the aura state's `srvactivefunc` (145), a1 = 0 does nothing |
| Monster event 7 → `init::handle_event7` (`wire-worldgen.md` §5 follow-ups) | `WorldSim::run_event` runs the unit dispatch with `WorldHooks` (action hooks + world state); `monster_umod` → the umod dispatcher, mode 2 | `worldgen/events.rs`, `worldgen/dispatch.rs` | `worldgen/tests/events.rs`: umod 41 reschedules event 7 at +75 twice; umod 21's bodiless callback lands in `MonsterStore::unhandled` with mode 2; a frozen monster's event 7 is dropped by the dispatcher (`tick.md` §5.6) |
| `MonsterStore::remove` and minion / owner maps on unit removal (`wire-worldgen.md` §5) | `WorldHooks::free_kind` = action free + `WorldState::forget` (monster data, minion list, owner link, superunique tail); `WorldSim::remove_unit` runs `units::lifecycle::remove` with these hooks | `worldgen/events.rs` | `events::removal_frees_the_world_state_of_the_unit` (AI control gone too) |
| Tick step 8 quests (`wire-action.md` §4 "TickHooks not routed: quests (economy)") | `economy::QuestTick` wraps any `TickHooks + UnitSide` and runs `QuestControl::update` on `EconomyQuests` as `update_quests`; every other hook and event is the wrapped one's | `economy/quest_tick.rs` | `economy/tests/quests.rs::quest_updater_runs_at_tick_step_8` (`tick::tick` × 100: updater ticks at frames 20, 40, …; the Den of Evil status timer runs at the second update, status 5, 0x5D sent, timer removed) |
| `QuestRest::mercenary_reward` → `NpcControl::quest_mercenary` (`wire-interaction.md` §6) | `Desk::quest_message(npc, player, msg)` (C→S 0x31) collects the rewards during the quest call (`EconomyQuests::mercenaries`) and runs them on the NPC control block right after | `interaction/quest_npc.rs`, `economy/quest_items.rs` | `interaction/tests/quest_npc.rs`: message 92 with the reward pending → flags granted, record state 5, Kashya's first offered slot hired from the real hire list, 0x50, merc spawned in mode 4 and initialised; without the pending bit nobody is hired |
| Vitals / regeneration on the combined dispatcher (`wire-interaction.md` §4.5) | already routed (event 3 in the unit dispatch); now shown through `tick::tick` on `ActionSim` | — | `interaction/tests/regen.rs`: mana +94 per frame for 3 ticks, event 3 rescheduled at frame + 1 each time |

## 3. Code map rows (for `docs/HANDOFF.md` §3)

| Path | What | Spec |
|---|---|---|
| `wiring/action/pending.rs` | + `SkillEvent`, `Pending::skill_event` (default nothing) | `stat-lists.md` §10.2, §10.3; `use.md` §7 |
| `wiring/interaction/skill_events.rs` | `route`: events 5 / 8 / 9 on `UseView` | `use.md` §7, `stat-lists.md` §10.2–§10.3 |
| `wiring/interaction/quest_npc.rs` | `Desk::quest_message` (0x31 + deferred mercenary reward) | `quests.md` §7.3, §10.2; `npc.md` §7.5 |
| `wiring/worldgen/events.rs` | `WorldHooks` (UnitHooks / LifecycleHooks / StatHost = action hooks + world state), `WorldState::forget`, `WorldSim::remove_unit` | `init.md` §22, `units.md` §3.2, §5 |
| `wiring/economy/quest_tick.rs` | `UnitSide` (for `UnitSystem`, `ActionSim`, `WorldSim`), `QuestTick` | `tick.md` §3 step 8, `quests.md` §5 |

## 4. Signature changes

No existing seam signature changed; nothing outside `wiring` changed.
Additions a coordinator should know when merging other branches:

1. `wiring::action::Pending::skill_event(h, sim, ev) where Self: Sized`:
   new associated function with a default (nothing). Implementors need
   no change (`d2-server` fakes `NoPending`, `TestPending` untouched). A
   seam value that implements `UseRest` must override it to get skill
   events (the interaction test fake `Open` does).
2. `wiring::action::ActionHooks<X>` now implements `UnitHooks::
   {active_state, periodic_skills, apply_item_aura}` (were defaults).
   With `NoPending` the behaviour is unchanged.
3. `wiring::economy::EconomyQuests` has a new public field
   `mercenaries: Option<&mut Vec<(UnitId, u16)>>`. Struct-literal
   constructions break; the workspace has none left (`grep -rn
   'EconomyQuests {' crates` → only the definition). Use
   `EconomyQuests::new`.
4. `WorldSim::run_event` no longer forwards to `ActionSim::run_event`; it
   runs `units::dispatch::dispatch` itself with `WorldHooks` (same
   error recording into `action.sys.errors`).

## 5. Remaining seams and why

| Seam | Why not closed |
|---|---|
| Event 9's second call `0x0056CE70`(game, unit, a1, skill, l, 0) | no body in any spec (`stat-lists.md` §10.3 names it only); `TODO` in `skill_events.rs`. So nothing reschedules a type-9 event |
| Event 14 / callback `0x00554570` (`cooldown_end`) | `use.md` §6: never scheduled in 1.14d; body not specified |
| `LifecycleHooks::init_kind` → `init::type_init` for monsters | the world state is not reachable from `ActionHooks`: the action adapters (missiles, AI, `View::allocate`) build views over `ActionHooks` alone, and population already runs `type_init` right after its own allocation (`wire-worldgen.md` §3), holding the world state. Routing it needs the world state inside `ActionHooks` (a lent store like the AI / missile stores) and population's explicit call removed in the same change |
| World-state free for removals made inside the action adapters (missile collide-kill, future corpse code) | same reason: they call `ActionHooks::free_kind`. Only `WorldSim::remove_unit` and removals during a `WorldSim` timer event reach `WorldState::forget` |
| Action `Pending` monster-data queries (`monster_flag`, level, AI state …) → `WorldState::monsters` | same reason (no world state in `ActionHooks`) |
| Event 10 `ai_reset` (`0x00573120`) | monster-data reset not specified |
| Umod 41's `InitHost::run_ai_tick` (`0x00573780`) | `WorldHost` keeps the default (nothing); `ai.md` §1 names it, the think vs tick split is not stated |
| The kill → `kill_experience` | the kill `0x0057CCB0` is call-level only (`damage.md` §7.2) and experience distribution is `damage.md` OQ7 / `vitals.md` OQ2 |
| `TickHooks` environment, presets (non-population), messages, items, compress units | no spec body (`wire-action.md` §4) |
| `NpcRest`, `VendorRest`, `UseRest`, `VitalsRest`, `CubeRest`, `QuestRest` members | unchanged from `wire-interaction.md` §6 / `wire-economy.md` §5 (unwritten owner specs) |
| One combined host (`QuestTick` around `WorldSim`, the `Desk`, `UseRest`) | `HANDOFF.md` §2 step 2; each piece composes now, but no struct owns economy + quests + world + interaction state |

## 6. Questions (each has a `TODO` at its site)

- **Q1 — the spec conflict `wire-interaction` found (I3), recorded, not
  decided.** Akara's respec (`npc.md` §8.2, lines 400–405) says: slot 41
  bit 1 set → "reset stats (`0x00570360`) and skills (`0x00570C80`)
  (player spec)". `vitals.md` §2.1 (line 124) names `0x00570C80` the
  **stat** reset (players only; for strength, energy, dexterity,
  vitality: `d = charstats start − base`, strength / dexterity `statpts
  −= d` and stat `+= d` with refresh, energy `gain_energy(d)`, vitality
  `gain_vitality(d)`), implemented as `combat::vitals::reset_stats`.
  So `0x00570C80` is a skill reset in one spec and the stat reset in the
  other, and `0x00570360` appears only in `npc.md`. Code path:
  `world::npc` respec service → `NpcWorld::reset_stats` /
  `reset_skills` → `wiring/interaction/npc_world.rs` `Desk` →
  `NpcRest::reset_stats` / `reset_skills` (the rest; neither is wired to
  `vitals::reset_stats`). Narrowest reading kept: both stay seams,
  `TODO(npc.md §8.2 vs vitals.md §2.1)` at `npc_world.rs`. A spec
  session should check both addresses in the 1.14d binary and correct
  one spec.
- **Q2 — mercenary reward order** (`quest_npc.rs`, `quests.tsv` row
  chain 37). The reward is deferred to right after the 0x31 list
  dispatch. Chain 37 (Act I intro) is visited after chain 2 and has an
  event-11 function `0x0058F870` without a body (logged as
  `unhandled 37 0x58f870`). In 1.14d it runs after the reward; here
  before. Same order only if `0x0058F870` does nothing observable for
  message 92 — a spec session should write its body.
- **Q3 — event 5 level argument** (`skill_events.rs`): `stat-lists.md`
  §10.2 passes `f(game, unit, skill, a2)`; the skill pipeline's do
  function takes a level, read as a2 (the use module's
  `active_state_event` already documents the other do arguments as the
  core's defaults).

## 7. Checks to queue (local, `docs/HANDOFF.md` §5)

1. With an aura recording (`use.md` §7; `impl-skilluse-vitals.md` check
   1): replay a paladin's aura right skill through `ActionSim` with a
   recording-backed `Pending + UseRest`; compare type-8 timer frames
   (≡ 1 mod `perdelay`) and the do calls per frame.
2. With the quest recordings (Den of Evil kill → status 5 after the
   updater; Kashya message 92 → 0x50 / merc spawn), replay through
   `QuestTick` and `Desk::quest_message`; compare message bytes and their
   order against the recording (settles Q2).
3. Re-run `cargo test -p conformance --test tick_replay` after any
   change to `WorldSim` / `ActionSim` event routing.

## 8. Gate results

All commands in §1 passed on this branch's head: `cargo test
--workspace` 1843 pass, 0 fail, 61 ignored (d2-sim 1199 pass, 5
ignored; conformance `tick_replay` 7 pass, unchanged); 12 new
integration tests (`skill_events` 4, `regen` 1, `quest_npc` 2,
worldgen `events` 4, economy `quest_updater_runs_at_tick_step_8` 1, plus
the existing `skill_use` tests now running with the skill events
routed); `coverage.py --check` 3180 claims, 0 errors; depcheck OK (8
crates).
