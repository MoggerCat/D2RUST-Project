# Handoff: one wired single-player host — `claude/host-merge`

> Not yet folded into `docs/HANDOFF.md` / `docs/PLAN.md` (neither is edited here); the coordinator folds it.

Cloud implementation session, 2026-10-06, task class: architecture
(high, METHODS M14). Base: `claude/tender-meitner-mphas3` at `4b5b0bf`
(main + wave-1 merges). Repo only, synthetic tables, fixed seeds, no
game files (M09). Closes `docs/HANDOFF.md` §7 J1, J4, the I7 / W16
part on the server host, `e2e-vendor-host.md` §4 findings 1–3 (with
the limits in §5), `e2e-next.md` §5 findings 1 and 3 (2 in part),
`server-world.md` §6 "fold into one host trait", the stale docs of
`quest-host.md` §5 and `server-skills.md` §5 item 9.

## 1. State

**Wired, unverified** (M02): no rule is added; the same `d2-sim`
modules run, now on one unit world behind one host value.

- **One host value (J1, `server-world.md` §6).** `SimGame<D, W>` has one
  host field, `world: W`; the fields `items: Option<ItemWorld>` and
  `skills: Option<Box<dyn SkillHost<D>>>` are gone. `WorldHost<D>` is
  the one host trait: besides `npc` / `vendors` / `waypoints` /
  `quests` it has `cube` (visitor `CubeCall`) and `skill` (a
  `skills::Call` → `Option<Handled>`), both defaulting to `None`
  (stub). Hosts: `NoWorld`; `ActionWorld<S = NoSkills>` (waypoints and
  the skill slot `S`); `WiredWorld<R, S = NoSkills>` (renamed from
  `TradeWorld`, file `world/wired.rs`): `ActionWorld<S>` + the economy
  parts (item tables, the one item store, unique bits) + `cube:
  Option<CubeParts>` + quests, NPC control, vendor tables, interaction
  state, rest. The skill slot is a type parameter, not a `dyn`: the
  slot's bound (`D::X: SkillRest`) is checked only where it is
  `WiredSkills`.
- **One unit store (J1, `e2e-next.md` §5 finding 1).** `ItemWorld`,
  `ItemHooks`, `ItemView` are deleted. The cube handlers run on the
  economy `WiredWorld::with_economy` builds from the action sim's own
  `sys.units` / `sys.stats` / `sys.data` / `sys.hooks` and the host's
  one `ItemStore` / `ItemTables`. `ServerCube` is generic over the hooks
  (`CubeHooks` = `LifecycleHooks + StatHost`). The cube's own state is
  `CubeParts` (cube tables, `Staged` inventories / date / sounds /
  targeting resets, creation info, `ItemPending`, errors) on the host.
- **One provider of the skill-use seams (J4).** `skills::seams`
  (`SkillSeams`) is deleted; `skills::world::World` is now a thin
  wrapper over `d2_sim::wiring::interaction::UseView`
  (`ActionSim::skill_use`): every seam call goes to `UseView` except
  what the server holds per message (staged player data +0x168,
  positions, owned-item / other-act results, reach), the server
  messages (collected: 0x15 → resync, others → `WiredSkills::unsent`),
  and the mode start (below, wanted change W-1). The skill list, skill
  bodies and the rest are the action wiring's `Pending` value as
  `UseRest`, plus the new `LearnRest` (`is_class_skill`,
  `add_skill_level`, `after_skill_point`); `SkillRest = Pending +
  UseRest + LearnRest`. 0x3A runs on the action wiring's `View` as
  `VitalsUnits` (refresh → `Pending::stats_refresh`); the vitals and
  skill tables are `ActionHooks::vitals` / `ActionHooks::tables`
  (`WiredSkills` holds only `unsent`; without `ActionHooks::vitals`,
  0x3A stays a stub).
- **Game-creation fields: one home (I7, W16; `e2e-vendor-host.md` §4
  findings 2–3).** The home is the action wiring: `ActionHooks::game_seed`
  (seed), `ActionHooks::ai_info` (difficulty +0x6D, game type +0x6A,
  ladder +0x74 as `game_type_ex`), `UnitData::expansion` (+0x70); the
  item format +0x78 follows from the expansion (`ItemGame::item_format`,
  `generation.md` §1.2). `ActionEvents::create_game(&GameFields)` writes
  them at game creation (and the copies `UnitData::difficulty`,
  `WorldState::{init_info, pop_info}` for `WorldSim`). The readers
  delegate: the host's economy builds `GameFields::from_action(..)` per
  call and writes the seed and unique bits back (`with_economy`, also for
  the cube); `WiredWorld` no longer holds a `GameFields` (only
  `uniques`); the waypoints' difficulty on the wired host is
  `ai_info.difficulty` (`HostWaypoints`); the death drop
  (`wiring::economy::death`) builds its fields from the same home and
  reads only `uniques` from `DeathDrops::fields`.
- **One owner of the player's interaction (+0x64/+0x68/+0x6C,
  `e2e-vendor-host.md` §4 finding 1, `e2e-next.md` §5 finding 3).** On
  `WiredWorld` the owner is the trade rest's `NpcRest::{interact_unit,
  set_interact, reset_interact}`: the NPC handlers ask it directly; the
  cube asks it through the new `items::Interact` (`Staged::interactions`
  and `Interaction` are gone; the reset reads back as `None`); the
  waypoints through `HostWaypoints` (a `WaypointWorld` wrapper over the
  action wiring's view that answers `set_interact`, `reset_interact`,
  `interact_guid` and the interaction part of `player_busy` from the
  owner, everything else from the action wiring). `ActionWorld` alone
  (no NPC rest) keeps `Pending`'s interaction calls.
- **Tick messages (`docs/HANDOFF.md` §2 step 3).** `SimGame::tick` now
  drains `WorldHost::take_sent` after `d2_sim::tick::tick` and queues
  each message to its player's client (§3.2 rule 1); a queueing failure
  goes to `SimGame::tick_faults`. So whatever a tick path sends through
  `Pending::send` (or a host rest) reaches the client in that tick's
  flush instead of riding the next world handler.
- **J2** was already done on the base (`SimGame::tick` runs `D`'s
  `TickHooks`).
- **Stale docs fixed:** `world/action.rs` header (`quest-host.md` §5);
  `SimGame::handle`'s `pierce_idx` TODO (`server-skills.md` §5 item 9:
  `use_::handle_message` does it).

Tests (all through `Host::frame`): d2-server lib 106 pass (unchanged
count; the item tests now run on `SimGame<ActionSim, WiredWorld>`, the
skill tests on `SimGame<ActionSim<Book>, ActionWorld<WiredSkills>>` with
the units in a real field room). `e2e_single_player` 3 pass and
`e2e_vendor` 4 pass on the new constructors (the cube now on the same
units, store and seed as the rest of the run; the waypoint and cube
interactions staged at the one owner).

## 2. Code map rows (for `docs/HANDOFF.md` §3)

| Path | What | Spec |
|---|---|---|
| `crates/d2-server/src/adapters/sim.rs` | `SimGame<D = Unspecified, W = NoWorld>`: one host `world: W`; `tick_faults`; `SimParts` / `parts()`, `player_of`, `client_of`; `handle` tries items (cube), world, skills, then the stub; `Tick` queues the host's tick-time sends | `intents-events.md` §2–§4, §3.2; `tick.md` §3 |
| `crates/d2-server/src/adapters/handlers/world.rs` | `WorldHost<D>` (the one host trait: `npc`, `vendors`, `waypoints`, `quests`, `cube`, `skill`, `take_sent`, `fault`), `NoWorld`, the world handlers | `world/*.md`, `intents-events.md` §2.4, §3.2 |
| `crates/d2-server/src/adapters/handlers/world/action.rs` | `ActionEvents` (+ `create_game`), `ActionWorld<S = NoSkills>` (waypoints, skill slot), `Outbox` | `waypoints.md` §6, §7.1; `rng.md` §5.3 |
| `crates/d2-server/src/adapters/handlers/world/wired.rs` | `WiredWorld<R, S = NoSkills>` (the wired single-player host: NPC, vendors, quests, cube, waypoints via `HostWaypoints`, skills), `with_economy` (fields from the home), `Parts` (+ `cube`), `TradeRest`, `HostWaypoints` | `npc.md`, `vendors.md`, `quests.md`, `cube.md` §1–§2, `waypoints.md` §6 |
| `crates/d2-server/src/adapters/handlers/items.rs`, `items/cube_world.rs` | `ITEM_IDS`, `handle` (via `WorldHost::cube`), `CubeParts`, `CubeCall`, `CubeHooks`, `Interact`, `Staged`, `Inventory`, `ItemPending`, `ItemError`; `ServerCube<H>` | `cube.md` §1, §2, §8 |
| `crates/d2-server/src/adapters/handlers/skills/{mod,wired,world}.rs` | `SkillHost<D>` (slot: `Option<Handled>`), `NoSkills`, `LearnRest`, `SkillRest`; `WiredSkills` (`unsent`); `World` over `UseView` | `use.md`, `levels.md` §6.4, `vitals.md` §2 |
| `crates/d2-sim/src/wiring/economy/game_fields.rs` | `GameFields::from_action`, `GameFields::ai_info` | `generation.md` Inputs |
| `crates/d2-sim/src/wiring/economy/death.rs` | drop fields from the action wiring's home (`uniques` from `DeathDrops::fields`) | `treasure.md` §3 |

## 3. Signature changes

`d2-server` (public):

- `SimGame`: fields `items` and `skills` removed; field `tick_faults:
  Vec<(ClientId, WorldError)>` added; methods `player_of`, `client_of`,
  `parts` and struct `SimParts` added. `impl Tick for SimGame<D, W>` now
  needs `D: EventDispatch + TickHooks, W: WorldHost<D>` (was `D:
  TickHooks`, any `W`).
- `handlers::world::WorldHost<D>`: methods `cube`, `skill` added (with
  defaults).
- `handlers::world::TradeWorld<R>` → `WiredWorld<R, S = NoSkills>`
  (module `world::trade` → `world::wired`); field `fields: GameFields`
  removed, fields `uniques: UniqueBits` and `cube: Option<CubeParts>`
  added, `action: ActionWorld<S>`; `new(action, tables, quests, npc,
  vendor_tables, rest, now)` (the `fields` argument removed). `Parts`
  gained `cube: Option<&mut CubeParts>`. New pub struct `HostWaypoints`.
- `handlers::world::ActionWorld` → `ActionWorld<S = NoSkills>` with field
  `skills: S`; `ActionEvents::create_game(&mut self, &GameFields)`
  added (provided).
- `handlers::items`: `ItemWorld`, `ItemView`, `ItemHooks`,
  `Interaction` removed; `Staged::interactions` removed; `handle` is now
  `handle<D, W: WorldHost<D>>(sim: &mut SimGame<D, W>, client, msg,
  out)`; added `CubeParts`, `CubeCall`, `CubeHooks`, `Interact`.
- `handlers::skills`: module `seams` (`SkillSeams`) removed;
  `SkillHost<D>` is now `fn handle(&mut self, Call<'_, D>) ->
  Option<Handled>` (no `unsent()`); added `NoSkills`, `LearnRest`,
  `SkillRest`; `wired::WiredSkills` is no longer generic and has only
  `unsent` (`new(vitals, seams)` removed, use `Default`);
  `wired::run` / `add_skill_point` take `World<'_, '_, X: SkillRest>`
  and `run` an `Option<&VitalsTables>`; `world::World<'v, 'a, X>` wraps
  `&mut UseView<'a, X>` (fields `u`, `staged`, `sends`, `point_accept`).

`d2-sim` (public, additions only): `GameFields::from_action(seed, &ai::GameInfo,
expansion, uniques)`, `GameFields::ai_info()`. Behaviour change in
`economy::death::monster_death_drop`: the creation fields come from the
action wiring (`ActionHooks::ai_info`, `UnitData::expansion`) instead of
`DeathDrops::fields` (equal in every existing test and fixture).

`d2-client` test files only (`tests/e2e_single_player.rs`,
`tests/e2e_vendor.rs`, `tests/e2e_support/mod.rs`): constructors updated.
`d2-client/src/bridge/local_tests.rs` (not mine) compiles unchanged
(`ActionWorld` defaults to `NoSkills`).

## 4. Wanted changes in files this session does not own (`wiring/{action,interaction}`)

- **W-1** `wiring::interaction::UseView::start_mode` should run the mode
  start itself (`modes::set_mode` → `UseRest::clear_target` →
  `modes::animate` → clear flag 0x40, `use.md` §4) instead of handing it
  to `UseRest::start_mode`, which has no access to the unit records;
  then `skills::world::World::start_mode` (the server's copy) goes. Until
  then the timer path's start (a fixture's `UseRest::start_mode`) and
  the message path's differ.
- **W-2** `wiring::interaction::Desk`'s `NpcWorld::item_format` /
  `VendorDesk`'s `VendorWorld::item_format` should answer from the
  economy's fields (`ItemGame::item_format(econ.fields)`, game +0x78)
  instead of `NpcRest::item_format`; the rest then loses the call. The
  fixtures answer 1 while the fields give 2 (classic): a real
  difference once a store item's version ≥ 100 matters
  (`vendors.md` §3 rule on `+0x78 < 100`).
- **W-3** the interaction owner: `Pending::{set_interact,
  reset_interact, interact_guid, busy}` and `NpcRest`'s interaction
  calls are two seams for one field; a player-data provider in
  `d2-sim` should own +0x64/+0x68/+0x6C and both wirings read it (then
  `HostWaypoints` goes).
- **W-4** the unique bits (+0x1B24) still have two copies when a game
  has both drops and the host's economy: `DeathDrops::fields.uniques`
  (in the action wiring's `Pending`, for the death drop) and
  `WiredWorld::uniques`. A unique dropped by a monster is unknown to the
  cube / store. Fix: a game-level home reachable from both (e.g. on
  `ActionHooks`), then the economy and the drop read it.
- **W-5** the drop's item store (`DeathDrops::items`, inside `Pending`)
  is still separate from the host's (`e2e-next.md` §5 finding 2): the
  dropped gold cannot be sold or cubed. Same fix shape as W-4 (the
  store reachable from the death-start hook).
- **W-6** the init write of difficulty 2 (`wiring::worldgen::init_units`
  `set_difficulty`, `init.md` §9) updates `ai_info`, `init_info`,
  `pop_info` but not `UnitData::difficulty`, which the waypoint view
  reads on `ActionWorld` (`WiredWorld` reads `ai_info`).

## 5. Findings and questions

1. With one interaction owner the e2e run now shares the talk's
   interaction (Akara, type 1) with the later cube and waypoint steps;
   the run stages the cube's and the waypoint's interaction at the owner
   as before (no close message 0x30 is sent between them).
2. The `set_mode` the skill pipeline calls (`UseWorld::set_mode`, "a
   plain mode set, reenter 1, no gates") is now `UseView`'s
   `units::modes::set_mode` (`0x00553570`); the server's old copy used
   `modes::player_start`, which also asks the request-check hook (the
   wiring's hook accepts, so no test changed).
3. The skill handlers now read the room kind from the DRLG
   (`CombatWorld::room`); the old seam answered `Field`. A unit outside
   every room makes `start_core` return 0 (`use.md` §5.3), so the skill
   tests now place their units in a real field room.
4. Q: `ActionHooks::ai_info.game_type_ex` holds game +0x74 as `u32`,
   `GameFields::ladder` as a flag; `from_action` reads `≠ 0`
   (`quality.md` §8). If any reader needs the full +0x74 word, the
   economy's field should become `u32`.

## 6. Local checks to queue

None new. The replays already queued (`e2e-vendor-host.md` §8,
`quest-host.md` §7, `server-items.md` §6) now run through one host
(`Host` + `WiredWorld`).

## 7. Gate

All pass at this commit: `cargo fmt --all -- --check`; `cargo clippy
--workspace --all-targets -- -D warnings`; `cargo test --workspace`
(1,936 passed, 0 failed, 62 ignored); `cargo run -p depcheck` (OK, 8
crates, determinism lint clean); `python3 tools/spec_index.py --check`;
`python3 tools/methods.py check` (21 OK); `python3 tools/coverage.py
--check` (3,259 claims, 0 errors) and `--selftest` (ok).
