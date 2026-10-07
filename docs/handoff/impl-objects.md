# Handoff: world objects (`d2-sim::world::objects`) and their wiring — `claude/impl-objects`

Cloud implementation session, 2026-10-06, task class: implementation from
a clear spec, medium (METHODS M14). Base: `claude/specs-staging` at
`5844674`. Repo only, synthetic tables, no game files (M09). Spec:
`specs/world/objects.md` (draft) and `specs/world/object-functions.tsv`;
`specs/sim/units.md` §6.4 for object events 1 and 11. Four parallel
sub-sessions on separate files (chests, shrines, doors/wells/torch/portal
plus core tests, wiring/server), merged here.

## 1. State

**Implemented, unverified** (M02): every rule is from the draft spec; no
recording of an object interaction exists (spec open question 1). The
unit tests prove the spec's synthetic vectors only.

| Part | File | Spec |
|---|---|---|
| Object control, per-object data, routes, create / init dispatch, §4 animation, inits 1, 2, 3, 5, 11, 12, 16, 57, presets 574–580 (580 level 25 only), operate entry and dispatch, timer events 1, 2, 4, 5, 6, 11, S→C 0x0E / 0x4D builders and the update pass | `crates/d2-sim/src/world/objects.rs` | §2–§7, §14; `units.md` §6.4 |
| Chests (operate 4, class 397, lock / key / assassin, sparkle), casket 1, urn 3, barrel 5, exploding barrel 7 (chain recursion), corpse 14, evil urn 68, trap arm and event 4 | `world/objects/chests.rs` | §8 |
| Shrine operate 2, all 24 effect codes, V(stat, a), gem / storm / exploding / poison, events 5, 6 | `world/objects/shrines.rs` | §9 |
| Doors 8, wells 22 (+ event 2), portal 15 rules 1–2, torch 11 | `world/objects/misc.rs` | §10–§13 |
| Wiring: `ObjectState` on `ActionHooks` (control, tables, host tick), `ObjectView` implementing `ObjectWorld`, routes from unit allocation, timer events, waypoint mode change, AI door operate, update pass, population `create_object` | `crates/d2-sim/src/wiring/action/objects.rs` (+ `mod.rs`, `units.rs`, `ai.rs`, `waypoints.rs`, `dispatch.rs`, `pending.rs`) | §3, §7, §14 |
| Server: C→S 0x13 with unit type 2 → `System::Objects` → `WorldHost::objects` (on `ActionWorld`, `WiredWorld`); `Dispatch::Waypoint` runs the existing `WaypointData::operate` | `crates/d2-server/src/adapters/handlers/world.rs`, `world/wired.rs` | §7.1; `waypoints.md` §5.2 |

Rows of `object-functions.tsv` marked `todo`, quest-owned and
waypoint-owned rows are not run by the module: `init_route` /
`operate_route` return `Route::{NotCovered, Quest, Waypoint}` and the
dispatch hands them back (`Dispatch::{NotCovered, Quest, Waypoint}`,
`Created::init`, `EventRun::{Quest, NotCovered}`, `Preset::NotCovered`);
the wiring logs them through `Pending::object_route`. A test checks both
route functions against every TSV row (with a perturbation test, M05,
M08).

Tests: `world::objects` 84 (core 23, chests 26, shrines 23, misc 12);
`wiring::action::tests::objects` 7; `d2-server` `world/tests/objects.rs`
3. Gate on this branch: `cargo test -p d2-sim -p d2-server` green except
the 10 known reds of other sessions (`missiles::tests_bodies::*`,
`monsters::ai::tests::specd_here_*`, `skills::use_::tests::*`,
`skills::mutant_tests::table_check_mutants::*`); clippy `-D warnings`,
fmt, `coverage.py --check` (objects.md 87 / 92 rule units claimed at unit
tier), `spec_index.py --check`, `depcheck` clean.

## 2. Seams and design choices

- **State:** `ObjectControl` (seed, regions, shrine lists, `data:
  BTreeMap<UnitId, ObjectData>`) is game state. The wiring keeps it in
  `ActionHooks::objects` and lends it per call (`with_objects`; a
  re-entrant call logs `WiringError::Reentrant("objects")`).
- **Creation order:** `ActionSim::create_objects(tables)` /
  `WorldSim::create_objects` builds the control and steps the game seed
  once (§2). It must be called right after `create_regions` (`rng.md`
  §5.2); there is no single chained game-creation sequence yet (TODO in
  `wiring/worldgen/dispatch.rs`). It is not created lazily, since a late
  seed step would shift later unit seeds. Without it every object route
  keeps its old `Pending` behavior.
- **Host clock:** doors (500 ms) and portals (5000 ms) read
  `ObjectWorld::host_tick`, set by the host through
  `ActionHooks::set_host_tick(ms)`; `d2-sim` never reads a clock. The
  server does not set it yet (`Intents::handle` has no clock; TODO in
  `action.rs`).
- **Allocation inside an object call** (presets §6, trap fire objects
  §8.3): the module holds the control, so `objects::allocate` runs the
  §3 init itself after `ObjectWorld::allocate_object`; the wiring's
  `View::object_init` skips while the control is lent.
- **Extension traits:** `ChestWorld`, `ShrineWorld`, `MiscWorld` have a
  narrow default for every method (nothing / none / false); `ObjectView`
  implements them empty, so items, monsters, missiles, states, hovers,
  footprint masks, key test, interact range, etc. do nothing on the
  wired host yet. Providing them is the next wiring step (§3).
- `Pending` gained `object_*` methods (footprint stamp / free, sound, key
  test, interact range, player busy, cursor item, staff-tomb level,
  route, 0x60 portal message, approach); see `pending.rs`.

## 3. Next steps

1. Provide the extension seams on the wired host: chest drop
   (`items/treasure.md` §4), item quality / type, code drop, key test
   (inventory), footprints and cell masks (path placement), trap monster
   spawn (monsters), stats / states / hovers / missiles for shrines.
2. Call `create_objects` in the host's game-creation sequence after the
   regions; set the host tick from the server's clock.
3. Object population (`PopulateFn`, §15) once specified; then population
   creates objects with their real modes (today `create_object` uses
   mode 0, TODO).
4. Local run queue (add to HANDOFF §5): record `packets` + RNG traces of a
   chest, shrine, door, well and portal operate (spec open question 1)
   and an object allocation with `Sync` = 0 (open question 2).

## 4. Open questions (each a `TODO(objects.md …)` in code; narrowest reading used)

Spec:
- O1 §5.1 test vector 1 says the fifth class-4 pick is id 8, but the §2
  list {1, 6..15} has id 9 at index 4. The fixture gives id 9 `LevelMin`
  5 so the vector's draws and result (id 13) still hold; the spec's id
  needs fixing.
- O2 §4 rule 3: the speed sum is read in 32 bits (only the shift is
  stated as 16-bit), then clamped to 0..0x7FFF.
- O3 `units.md` §6.4 type 1: the footprint free is read as inside the
  mode-1 branch.
- O4 §14: the 0x4D "operator GUID" is written as the stored field − 1
  (open question 4). 0x60 layout not in the spec (handed to the host).
- O5 §6: 580 outside level 25 goes through 581's path (open question 6),
  581 and 582: `Preset::NotCovered`.
- Chests: a locked chest with no operator is "no key"; null drops in the
  class-397 bands count as neither magic nor non-magic; breakables
  return 1 on every path; the exploding-barrel distance metric and
  bound inclusivity are the provider's (`ChestWorld::within`); trap 8/9:
  the trap monster id is read after the control-seed step (open question
  3); fire objects 162/160 allocated in mode 0.
- Shrines: no shrine record → `Err(NoRow)` before any change; no operator
  → no effect; codes 4/5 written as `set_stat` of the new value; code 16
  does nothing; code 17 without a free spot creates nothing; storm life
  writer, loop nesting (i outer) and missile flags (0) not stated; the
  potion drop has no address (own seam).
- Doors / wells / portals: locked door's key test without the assassin
  exemption; debounce and hostile delay compared as wrapping u32 sums;
  well "used" = a value changed, refill mode set not queued (explicit
  queue after); `Parm2` = 0 → `ObjectError::WellCharges`; portal with a
  monster or no operator refused; portal rule 3 (open question 7) is
  `MiscWorld::portal_travel` (default: not run → `Dispatch::NotCovered`).
- Wiring: the init's footprint stamp sees no path record (d2rs places the
  path after the per-kind init); the waypoint init 17 needs the server's
  `WaypointData`, handed back through `object_route`; the 0x13 result
  after an operate or a walk is read as 0.
