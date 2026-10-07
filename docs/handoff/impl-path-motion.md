# Handoff: monster walk / run and missile flight on the path provider — `claude/impl-path-motion`

Cloud implementation session, 2026-10-07, medium (METHODS M14). Base:
`claude/specs-staging-5` at `cd03d30`. Repo only, synthetic tables and
DRLG (the action fixture), no game files (M09). Task: the MOTION items of
HANDOFF §1 row "3 not implemented" (walk / run for monsters, walk / run
without the path provider, missile flight on the provider, `pathing.md`
OQ3) and HANDOFF §2 step 7p blocker WP1. Specs: `sim/pathing.md` §2, §3,
§8.1, §9.1, §9.3–§9.5, §9.10, §11; `sim/path-placement.md` §2.3, §6;
`monsters/ai.md` §1.4, §2.2, §7.1, §7.2; `sim/units.md` §4.6;
`monsters/umod-callbacks.md` §2; `skills/bodies.md` §2.4;
`skills/bodies-2.md` §2.3; `missiles/bodies.md` §19; `missiles/bodies-2.md`
§46. Nothing is checked against 1.14d (no per-tick recording,
`pathing.md` OQ1): everything here is **wired, unverified**.

## 1. What landed

**WP1 is stale.** A missile built with the provider on gets its type-4
path (flag 0x40000 from the allocation's set type, `missiles.md` §R2.3
step 16) and `0x00649970` takes the missile branch to `pathing.md` §11,
implemented in `path::walk::missile` since `impl-walk`. The arrow of the
action fixture is built with one point and flies 0xC00 a frame (Vel 1 →
0x100 · 75 % = 0xC0; (0x400 · 0xC0) >> 6), test
`missile_path_is_built_and_flown_on_the_provider`. So a host can keep the
provider on while missiles fly; `crates/d2-client/tests/e2e_single_player.rs`
(not edited here: d2-client is impl-pc1-final's) still keeps it off and its
comments (lines 62, 1574) name WP1: turn the provider on there and drop
the comments. The live app (`d2-client::app::single_player`) and
`test-fixtures::game` already enable it.

**Monster walk** (`d2_sim::wiring::path::monsters`, provider on only):

- `ActionHooks::monster_path_setup` = the path part of `0x005A7C20`
  (`UnitHooks::monster_mode_bookkeeping`, every mode but GH): the AI
  request's target (unit `0x00648B90`, else point `0x00648AD0`), the
  re-path budget 20 (`0x006490E0`, `pathing.md` §9.10), then the movement
  set-up (`ai.md` §7.1): request byte 101 for a moving mode
  (`0x005A6B10`) → path type 13 computed, a 0-point compute retried with
  type 15; byte 100 → path type 0, nothing computed. The AI's request is
  staged by `AiModes::change_mode` in `PathState::mode_request` (new
  field); the kill's death change gives `ActionHooks::mode_target`.
- Walk / run starts `0x005A7520` / `0x005A7550` set their mode (reading
  of `umod-callbacks.md` §2 rule 2, MV1).
- `UnitHooks::anim_velocity` (new, default nothing; called by
  `units::modes::prepare_animation` right after `anim_rate`):
  `ActionHooks` runs `path::walk::velocity::mode_velocity` for monsters
  (`pathing.md` §8.1 rules 1–2, 4). Players are unchanged (their walk
  request sets the velocity itself).
- Walk event 0 `0x005A8490` (`pathing.md` §9.1): state 13 → the
  existing `WalkUnits::state13_step`, state 22 → new
  `WalkUnits::state22_step` (both skills-spec seams, default nothing),
  the step `0x00554CA0` (§9.3); a stop runs the mode end.
- Mode end `0x005A8030` (`ai.md` §1.4, `monsters::ai::mode_end` on the AI
  store) for the walk's stop **and** as the event-1 function of modes
  3–9 and 14 (`units.md` §4.6 table). This one is not gated on the
  provider (it is not a path function); no existing test changed.
- AI seams with the provider on: `AiUnits::path_target` (`0x00553540`:
  the path's target unit), `AiModes::path_blocked` (path flag 0x800,
  `ai.md` §2.2 rule 2).

Result: an AI walk (`ai::walk_to`, `0x005DEC80`) moves the monster
sub-tile by sub-tile to the player and the inline think runs on the frame
the path ends (`an_ai_walk_to_a_unit_moves_then_rethinks_at_the_path_end`);
a point walk reproduces vector M1 exactly for a monster with monstats
`Velocity` 6.

**Missile-body path seams** (`d2_sim::wiring::path::missiles`, on
`MissileBodies for View`, provider on; off: the trait defaults as before):
`path_target_point` (`0x00648A00` / `0x00648A10`), `target_position`
(`0x0056D2C0`), `set_path_type` (`0x00648CF0`), `set_path_distance`
(`0x00648E70`: +0x90 and +0x91 := min(n, 77)), `path_teleport`
(`0x00650BE0`: teleport §6 rule 4, then point count 0). The `zigzag` init
callback (`missiles/bodies.md` §19) now rebuilds a real charged-bolt path.

**Walk / run without the provider:** nothing to add. Without the provider
the walk start leaves the mode (no path to step) and every earlier test is
unchanged; every real host turns the provider on.

## 2. Seams and APIs

| Item | Where | Default |
|---|---|---|
| `PathState::mode_request` | `wiring/path/mod.rs` | `None` |
| `UnitHooks::anim_velocity(sim, unit)` | `units/hooks.rs`, called in `units::modes::prepare_animation` | nothing |
| `WalkUnits::state22_step(unit)` | `path/walk/seams.rs` | nothing |
| `Pending::monster_run_event0(unit)` | `wiring/action/pending.rs` | nothing (gap MV2) |
| `ActionHooks::{monster_path_setup, monster_mode_velocity, monster_motion_function}` | `wiring/path/monsters.rs` | — |
| `View::{path_target_xy, path_target_position, path_set_type, path_set_step_counts, path_teleport_to}` | `wiring/path/missiles.rs` | `None` without the provider |

Shared-file edits are one-liners delegating to these modules:
`wiring/action/{units,ai,missiles,pending}.rs`, `units/{hooks,modes}.rs`,
`path/walk/seams.rs`.

## 3. Tests

`cargo nextest run -p d2-sim motion_tests`: 11 tests in
`crates/d2-sim/src/wiring/path/motion_tests.rs`, each with its `Covers:`
line: missile built and flown (M08: out-of-range target fails the
create); monster point walk (M1 for a monster, footprint 0x100 moves);
M08: provider off, no movement; a non-moving mode targets the unit and
computes nothing (path type 0); a 0-point type-13 compute retries with
type 15; path flag 0x800; attack end → neutral and think at f + aidel;
AI walk to a unit → stop at (27, 30) after 19 ticks, think at f + 200;
zigzag init → charged-bolt path, 26 points, 25 draws (M08: another target
x); missile-body teleport.

Gates at `1a3acca`: fmt, clippy (workspace minus d2-client) clean;
nextest 4,619 run, 4,614 pass, the 5 known failures only (`monsters::ai`
`specd_here` × 2, `scenario-run` × 3); `coverage.py --check` 8,927 claims,
0 errors; `spec_index.py --check` clean.

## 4. Spec gaps (for PC 1)

- **MV1** `sim/units.md` §4.6: the monster start functions
  (`0x005A7520` WL, `0x005A7550` RN, and the others) have no body; walk /
  run are read as "mode set `0x00553570` with the requested mode,
  started" from `umod-callbacks.md` §2 rule 2 ("the mode field holds the
  new mode"). The attack / skill / BL / KB / SQ starts still leave the
  mode unchanged in the wiring (not this task's), so AI attacks never
  enter their mode.
- **MV2** `sim/units.md` §4.6 / `pathing.md` §9.1: the event-0 bodies of
  run `0x005A84F0`, knockback `0x005A8630` and sequence `0x005A8670` are
  not described (only walk's). A running monster sets mode 15 and stands
  still; `Pending::monster_run_event0` is the seam.
- **MV3** `pathing.md` §9.1: whether the state 13 / state 22 calls of the
  monster walk event gate the step; read as "then the step" (as the
  player's §9.2 step 2).
- **MV4** `ai.md` §7.1: the movement set-up's path-type write (set type
  `0x00648CF0` or direct) and the compute's town-access argument are not
  stated (set type, 0 used). Whether the re-path budget 20 is given for
  every mode or only moving ones (§9.10 "movement start") — given for
  every non-GH mode (only read by the re-path).
- **MV5** `ai.md` §7.1, §7.3: how a pending velocity request (method,
  speed, steps) "replaces" the request's +0x15 / +0x18 / +0x1C is not
  stated; the request is neither consumed nor cleared (circle, escape and
  `walk_to_method13` move with type 13).
- **MV6** `ai.md` §7.1: only the AI's mode changes and the kill's death
  change pass their request target; quests, pets and skill bodies call
  the mode set without one, so the path target is left as it was.
- **MV7** `path-placement.md` §2.3: the field `0x00649070(path, n)` (the
  AI's "path step count") writes is not named; stays
  `Pending::set_path_steps`. `AiModes::stop_path` (NPC interaction, `ai.md`
  §2 table) names no function; stays `Pending::stop_path`.
- **MV8** `pathing.md` OQ3: path types 0, 3, 5, 6 (circling `0x00679B30`),
  8, 9, 11, 12, 15 (`0x0067C2D0`), 16 still have no function spec;
  `WalkUnits::other_path_function` answers 0 points, so the type-15 retry
  never finds a path.
- **MV9** `skills/bodies.md` §2.4: `0x0056D2C0` with a stale target unit
  (GUID gone); the point is read.
- **MV10** `skills/bodies-2.md` §2.3: `0x00648E70` with a negative count.
- **MV11** the `0x0064E260` line test (`MissileBodies::line_hits`,
  `skills/bodies.md` §2.11) has no algorithm in `path-placement.md`.
- Observation: after a walk's mode end the every-tick event 0 stays
  scheduled (the mode end sets the anim mode only, `ai.md` §1.4); mode 1's
  event 0 `0x005A6DE0` is a stub, so nothing runs. Whether the original's
  think or `0x005A6DE0` cancels it is the AI spec's.

## 5. Left (cloud, repo only)

- Missile init callbacks (`Pending::missile_init_callback`:
  jitter `0x005C9290`, lightning fan / ring, DiabWall; `skills/bodies-2.md`
  §2.3, `bodies-4.md` §2.4–§2.5) need the missile frames from the store
  inside `create_missile`, like the `zigzag` dispatch in
  `missiles::create` step 21; the `JitterMissile` / `PathMissile` seams
  can then run on the provider (`set_steps` = `path_set_step_counts`).
- `e2e_single_player.rs`: provider on (WP1 stale), see §1.
- The monster movement messages 0x67 / 0x68 (`intents-events.md` §7.3)
  belong to the client pass (impl-session-flow).
