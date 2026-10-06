# Handoff: end-to-end combat path — `claude/e2e-combat-path`

Cloud implementation session, 2026-10-06, task class: integration from
clear specs, medium (METHODS M14). Base: `claude/bold-ptolemy-jvyvxy` at
`35484d4` (= `main` `b966c6b` + wire-open-seams, PR #18). Repo only,
synthetic tables, no game files (M09). For the coordinator to fold into
`docs/HANDOFF.md` / `docs/PLAN.md` (neither is edited here).

## 1. State

**Wired, unverified** (M02): every module behind the new adapters is a
draft-spec implementation; no recording has been replayed.

The single-player e2e (`crates/d2-client/tests/e2e_single_player.rs`)
now runs a right-skill cast to its end: steps 4 and 5a of
`e2e-single-player.md` §2 **run** (rows updated there).

| Frame | What happens | Owner (spec) |
|---|---|---|
| 2 | C→S 0x0C → 0; mana 4000 − 3328; srvst 4; mode SC; AnimData record of the composed name → event 0 at 5 (args 2, 0), event 1 at 9 | `use.md` §1, §5.3; `units.md` §4.1–§4.2; `animdata.md` §3–§5 |
| 5 | player event 0 → action frame `0x00580460` → `use_::attack_frame_event` → do core: srvdo 8 (seam) + `srvmissile` 0 through `missiles::create_missile` | `units.md` §4.5; `use.md` §5.2, §5.4 |
| 6–15 | missile flies (fixture path) and hits on 15: to-hit, damage 10 points ≥ 5 life → life 0, result 3, events 10 / 9 | `missiles.md` §R4–§R6; `damage.md` §5.2 |
| 15 | reaction → kill: steps in order, death mode DT with target = player (death start `0x005A6FF0` → fixture sets DT, drop), death animation end at 19; experience 100 | `damage.md` §7.1, §7.2; `units.md` §4.6; `vitals.md` §4.2–§4.3 |
| 15 | drop: gate, TC 1, walk on the monster seed, gold item on the game seed at (x + 2, y + 3), in the monster's room, mode 3, stat 14 ∈ 1..6 | `treasure.md` §3.1–§3.5, §7, §8; `generation.md` §3 |

Every frame's S→C bytes are asserted empty: the wiring sends no S→C
message on this path (no written spec ties a mode, missile, death or
item message to it). Determinism: `same_seed_same_run` compares the
whole transcript (now also experience, dropped items' GUID / seed /
position / mode, gold); `other_seed_other_run` still differs on seeds
and agrees on the wire.

New d2-sim integration tests (`wiring/action/tests/death.rs`, 7): AnimData
schedule from the composed name (event 0 at f + 4, end at f + 8), the
default record for a name not in the file (2048 frames), no name → no
record; a killing missile (event order, kill steps, death target, death
animation end, experience); kill guards (dead monster, state 54 →
`death_delay` only); the drop as a real item in the room (exact seed
steps: monster `roll(1)`, game seed twice); the gate (flag 0x20000, door
bit).

## 2. Code map rows (for `docs/HANDOFF.md` §3)

| Path | What | Spec |
|---|---|---|
| `wiring/action/units.rs` | `UnitHooks::{anim_record, anim_rate, frame_bonus, player_action_frame, monster_mode_function}` on `ActionHooks`; `anim_record()` (format record → §4.2 fields) | `units.md` §4.1, §4.3, §4.5, §4.6; `animdata.md` §3–§5 |
| `wiring/action/reaction.rs` | `reaction` (state-54 rule, monster will-die → kill), `kill` (§7.2 guards and steps, death mode with target, experience), `VitalsUnits` on `View` | `damage.md` §7.1, §7.2; `vitals.md` §4 |
| `wiring/action/pending.rs` | + `anim_name`, `anim_rate`, `frame_bonus`, `KillStep` / `kill_step`, `superunique`, `minion_owner`, `party_size`, `quest_tc_open`, `stats_refresh`, `level_up_notify`, `level_up_event`, `monster_death_start`, `action_frame` | (seams) |
| `wiring/action/mod.rs` | `ActionHooks::{anim_data, vitals, mode_target}`; `WiringError::AnimData` | |
| `wiring/interaction/skill_events.rs` | + `action_frame` (event 0 → `attack_frame_event` on `UseView`) | `use.md` §5.2 |
| `wiring/economy/death.rs` | `DropTables`, `DeathDrops`, `FreeSpot`, `monster_death_drop` | `treasure.md` §3, §7 |
| `crates/d2-sim/Cargo.toml` | + `d2-formats` (the parsed `AnimData` is an input; no I/O) | |

## 3. Signature changes

No existing seam signature changed; no implementor outside this branch's
files had to change. Additions a coordinator should know:

1. `Pending` gained the methods listed in §2, all with defaults equal to
   the previous behaviour (no record, rate 0, bonus 0, action result 1,
   death start "started, nothing done", queries false / `None`).
   `NoPending` and the d2-server fakes compile unchanged.
2. `ActionHooks` has three new public fields (`anim_data`, `vitals`,
   `mode_target`), all `None` from `ActionHooks::new`. No struct literal
   of `ActionHooks` exists in the workspace.
3. `CombatView::reaction` now runs `wiring::action::reaction::reaction`:
   `Pending::reaction` first (unchanged call), then the state-54 rule and
   the monster kill. With `NoPending` and no state 54 the only new effect
   is the kill of a monster whose hit result has bit 2.
4. `UnitHooks::player_action_frame` and `monster_mode_function` are now
   implemented on `ActionHooks` (were trait defaults); with `NoPending`
   the results are the same (1 / true).
5. `WiringError` has a new variant `AnimData(FormatError)`.
6. `d2-sim` depends on `d2-formats` (allowed by depcheck).

Not touched: `d2-server/src/adapters/handlers/world*`, vendor code, any
d2-server file.

## 4. Where the path stops at a seam (no written spec; never invented)

| Seam | Why | Fixture answer in the e2e |
|---|---|---|
| `Pending::anim_name` — COF-name composer `0x0064F5B0` with a unit | `animdata.md` OQ2 (weapon class needs equipment / inventory) | `SOSCHTH` (player SC), `M0DTHTH` (monster DT), else none |
| `Pending::anim_rate` — `0x00623F50` | animation-rate spec not written (`units.md` §4.3) | the AnimData speed |
| `Pending::frame_bonus` — `0x00623B10` | same | default 0 |
| `SkillSeams::srvdo` / `UseRest::srvdo` — do function bodies | `use.md` OQ10 (catalogued only) | logged, result 0 |
| `UseRest::skill_missile_fill` | `use.md` §5.4 step 7 record fill not specified | absolute target = cast point |
| `Pending::missile_damage_setup` — `0x0059F900` | skills spec | the test sets the missile's damage stats (2560) after creation |
| Path: `step`, `set_target_point`, `crossed_subtiles`, positions; monster target flags and collision bit | path / kind-init / movement specs not written | one sub-tile per frame toward the target; flags and bit set by the test |
| `Pending::reaction` (town rule, hit class store, player branch, monster knockback / block / get-hit / soft hit) | `damage.md` §7.1 call level only (OQ3) | logged |
| `Pending::kill_step` (pet credit, attacker bookkeeping, facing, quest kill, barricade doors) | `damage.md` §7.2 call level only | logged |
| `Pending::monster_death_start` — body of `0x005A6FF0` | not written (only the drop gate and `0x00547E50` are named) | sets mode DT (a start sets its mode), then `monster_death_drop` |
| `FreeSpot` — `0x0064E810` | collision spec (`treasure.md` OQ 8) | the start spot |
| Monster rank, minion owner, party, quest TC | monsters / units / party / quests specs | defaults (normal, none, none, false) |

## 5. Questions (each has a `TODO` at its site)

- **C1 — reaction order** (`reaction.rs`): `Pending::reaction` runs
  before the state-54 rule and the kill; in 1.14d the state-54 test sits
  between the hit class and the defender branches, and the monster kill
  between the "knockback without KB → get-hit" adjustment and the
  knockback / block / get-hit / soft-hit changes. Same result as long as
  the seam's branches are empty for a state-54 defender. Settled by
  `damage.md` OQ3 (read `0x0057CEE0` branch by branch).
- **C2 — experience in the kill** (`reaction.rs`): where `0x0057CCB0`
  gives experience is not stated (`damage.md` OQ7, `vitals.md` OQ2);
  given last, to the attacker, players only, without pet credit / party
  share / `ExpRatio` / stat 85. No draws, so only the order of stat
  writes against the other kill steps can differ.
- **C3 — drop inside the death start** (`death.rs`): the gate's position
  in `0x005A6FF0` relative to the rest of its body is unknown; the
  collision word outside every room grid is read as 0; the free-spot
  search gets the room the start-offset search found, else the
  monster's room.
- **C4 — game seed**: the drop's item creation runs on
  `ActionHooks::game_seed` (copied into `GameFields::seed` and written
  back), one game seed for units and items (`rng.md` §5.3, W16).

## 6. Checks to queue (local, `docs/HANDOFF.md` §5)

1. With the real `AnimData.d2` (`D2_GAME_DIR`): the e2e's anim lookups
   against real names once the composer is specified (animdata OQ2);
   until then `SKA11HS`-style vectors through `ActionHooks::anim_record`
   with a fixed name (event bytes → event-0 frames, `units.md` §4.2).
2. A recording of a sorceress cast on a monster that dies (group A,
   `record_packets.py` + timer hooks): event-0 / event-1 frames after
   0x0C, the missile's creation frame, the hit frame, the kill's mode
   change and the experience delta, the drop's item GUID and gold;
   replay through this e2e with recording-backed seams (settles C1–C3).

## 7. Gate

`cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --
-D warnings`; `cargo test --workspace`; `cargo run -p depcheck`;
`python3 tools/spec_index.py --check`; `python3 tools/methods.py check`;
`python3 tools/coverage.py --check`. Results in the commit message.
