# Handoff: units, stats, stat lists (implementation)

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Branch `claude/impl-units-stats`, from `claude/bold-ptolemy-jvyvxy` at
`4c7a7c7` (cloud session, 2026-10-06). Task class: implementation from a
clear spec, medium (METHODS M14). For the coordinator to fold into
`docs/HANDOFF.md` and `docs/PLAN.md`.

## 1. State

Implemented, **unverified** (the specs are drafts; their checks are queued
in `docs/HANDOFF.md` §5 "Phase 3 units/stats recordings"):

- `specs/sim/stats.md`: keys, values, the itemstatcost facts, readers and
  the minimum rule, MulDiv, the op loop (`stat-ops.tsv`), by-time, life
  fraction, clamp to max.
- `specs/sim/stat-lists.md`: lists and extended lists, sorted arrays, base
  writes, propagate/recompute/set-full/add-full, value-change callback
  (server `0x0055B800` incl. the max rescale without floats), attach,
  detach, free, equip, park, dynamic toggles, by-time refresh, death,
  overlay, state bits and queries, expiry, mod array; handlers 3, 5, 9, 12.
- `specs/sim/units.md`: unit record, allocation (seeds per `rng.md` §5.3)
  and removal, set mode, prepare animation, the §4.2 schedule (main form
  and the three variants), every-tick movement, player mode starts and
  events 0/1, monster mode set with the mode table, neutral start (AI
  delay), join, missile setup, the dispatch of every (kind, event type)
  per `unit-handlers.tsv`, events 6 and 11.

Changes outside my modules: one line `pub mod stats;` in
`crates/d2-sim/src/lib.rs`; `mod` lines in `crates/d2-sim/src/units/mod.rs`.
Nothing else (no `tick`, `lists.rs`, `game.rs`, `rng.rs`, Cargo or spec
edits).

Gate (all pass): `cargo fmt --all -- --check`, `cargo clippy -p d2-sim
--all-targets -- -D warnings`, `cargo test -p d2-sim` (115 tests),
`cargo run -p depcheck`, `python3 tools/spec_index.py --check`,
`python3 tools/methods.py check`.

Tests cover every synthetic test vector of the three specs (stats.md
MulDiv / ByTime / evaluation; stat-lists.md steps 1–6 with the callback
order and the nested set; units.md §4.2 ten vectors), the edge cases, and
the TSVs (M05): `stat-ops.tsv` ↔ `stats::ops::OP_ROWS`,
`unit-handlers.tsv` ↔ `units::dispatch::HANDLERS` (and agreement with
`tick::events`), `unit-events.tsv` rows of the 24 scheduler sites d2rs
reproduces, plus U1 for objects/items. Each TSV check has a perturbation
test (M08).

## 2. Code map rows

| Path | What | Spec |
|---|---|---|
| `crates/d2-sim/src/stats/mod.rs` | stat ids, keys, `StatTable` (from the fixed-up itemstatcost: typed columns + op bytes +0x51…+0xF5), `ClassStats`, `StatData`, `muldiv`, `by_time`, `life_fraction`, `x87_rescale` | `sim/stats.md` |
| `crates/d2-sim/src/stats/ops.rs` | op table as data, checked against `stat-ops.tsv` | `sim/stats.md` §6, `sim/stat-ops.tsv` |
| `crates/d2-sim/src/stats/lists.rs` | `StatLists` arena (generational `ListId`), readers, evaluation, writes, propagate/recompute, callback, chain ops, queries, expiry, mod array; `StatHost` seam | `sim/stat-lists.md`, `sim/stats.md` §4, §6 |
| `crates/d2-sim/src/stats/states.rs` | `StateTable` (states + runtime bitsets), state bits, toggle, has-state / group queries | `sim/stat-lists.md` §9 |
| `crates/d2-sim/src/stats/tests.rs` | synthetic itemstatcost built through `d2_data::fixup::records::stat_ops`; spec vectors | |
| `crates/d2-sim/src/units/record.rs` | `UnitRecord`, `Units`, flags, `AnimRecord`, `Sequence`, dead test | `sim/units.md` §2 |
| `crates/d2-sim/src/units/lifecycle.rs` | `allocate` (`0x00555230`), `remove` (`0x00555600`), `LifecycleHooks` | `sim/units.md` §3 |
| `crates/d2-sim/src/units/anim.rs` | §4.2 schedule (pure) + apply, cancel 0/1, §4.4 every-tick | `sim/units.md` §4.2, §4.4 |
| `crates/d2-sim/src/units/modes.rs` | set mode, prepare animation, player starts/events, monster mode table/set/neutral start, join, missile setup, `UnitError` | `sim/units.md` §4, §6.1, §6.3 |
| `crates/d2-sim/src/units/dispatch.rs` | `HANDLERS` (checked against `unit-handlers.tsv`), `dispatch`, `UnitSystem` (the `EventDispatch`), regeneration, events 5, 6, 9, 11, 12 | `sim/units.md` §5–§6, `sim/stat-lists.md` §10 |
| `crates/d2-sim/src/units/hooks.rs` | `Sim`, `UnitData` (monstats/monstats2), `UnitHooks` seam | `sim/units.md` |
| `crates/d2-sim/src/units/tests.rs` | §4.2 vectors, TSV checks, dispatch, regen, modes, allocation on fakes | |

## 3. Public API (for the other groups)

Stats (`d2_sim::stats`):

- `StatData::new(itemstatcost, charstats, StateTable::new(states, &maps),
  monstats, skills)` — all from the **fixed-up** set
  (`d2_data::fixup::apply`); `StatTable::from_fixed` needs the fix-up's op
  bytes.
- `StatLists::new(Arc<StatData>)`; per unit `alloc_extended(host, unit,
  type, guid, class, flags, callback)` (done by `units::lifecycle::allocate`
  for players, monsters, items, missiles); plain lists `alloc(flags,
  expire, owner_type, owner_guid)`.
- Reads: `base`, `total`, `unit_total`, `unit_base`, `unit_bonus`,
  `max_life/mana/stamina`, `percent_adjusted`, `eval`; `has_state`,
  `has_group`, `list_of_state`, `list_by_flags`, `state_list_owner`.
- Writes: `set`, `unit_set`, `add`, `unit_add`, `remove_all`, `merge`;
  `attach`, `detach`, `free`, `free_plain`, `equip`, `make_static`,
  `make_dynamic`, `park`, `by_time_refresh`, `death`, `remove_overlay`,
  `toggle_state` (returns the disguise bit for unit +0xC8),
  `set_expire`, `set_state`, `set_skill`, `set_remove_callback`,
  `set_flags`, `clamp_to_max`; mod array: `mod_values`, `clear_mods`,
  `single_stat`.
- Helpers: `stats::{key, muldiv, by_time, life_fraction, stat::*,
  states::state::*}`.

Units (`d2_sim::units`):

- `dispatch::UnitSystem<H: UnitHooks>`: owns `units`, `stats`, `data`,
  `hooks`; implements `tick::EventDispatch`; `with(game, |sim, hooks| …)`
  gives a `Sim` for any unit call. Handler errors land in `errors`.
- `lifecycle::{allocate, remove}`; `modes::{set_mode, animate,
  player_start, player_join, monster_set_mode, monster_neutral,
  missile_init}`; `anim::{schedule, run, every_tick_movement}`;
  `record::{Units, UnitRecord}`; `dispatch::{replenish_delay,
  replenish_start_delay, SKILL_COOLDOWN}`.

## 4. Seams (trait methods; default = nothing / the value that makes the caller do nothing)

`stats::StatHost` (also a supertrait of `UnitHooks`):

| Method | Expected provider |
|---|---|
| `act_time` (ops 6/7, `stats.md` §8) | world group (environment spec) |
| `item_event` (§7.2 rule 1) | items |
| `skill_stat_changed` (§7.2 rule 2, stats 83…204) | combat/skills |
| `list_removed` (remove callback +0x38) | combat/skills (state code) |
| `stays_on_death` (`0x0063A4A0`) | combat/skills |
| `on_callback` | conformance (trace log) |

`units::UnitHooks`: animation (`anim_record`, `anim_rate`, `frame_bonus`,
`load_sequence`, `has_path`, `reinit_anim`: animation-rate / sequence /
path specs, not written); `drop_combat_entries` (combat); `room_flag`
(`0x0061AB00`, DRLG/world); player (`player_request_check`,
`player_death`, `player_corpse`, `player_knockback_path`,
`player_skill_start`, `player_movement_step`, `player_action_frame`,
`player_item_row_flagged`, `player_attack_cleanup`, `player_refresh`,
`update_trade`); monster (`monster_mode_bookkeeping`,
`monster_class_record`, `monster_mode_function` by address,
`uninterruptable_check`, `ai_think`, `monster_umod`, `ai_reset`,
`monster_death`: missiles/AI group); skills (`active_state`,
`periodic_skills`, `apply_item_aura`, `cooldown_end`: combat/skills);
`missile_do` (missiles/AI); `object_event` (world group); `item_replenish`
(items); `send_life_fraction` (d2-server messages); `free_hover`.
`units::lifecycle::LifecycleHooks`: `init_kind`, `free_kind` (each kind's
spec).

Wiring for the coordinator: `d2-server`'s `SimGame<D: EventDispatch>` can
take `UnitSystem<H>` as `D`; its staged `PlayerFields` / `UnitFacts`
(mode, state 54, unit acts) move to `Units` / `StatLists`.

## 5. Open questions (each has a `TODO` in code)

1. `stat-lists.md` OQ1: x87 precision of the max rescale. d2rs emulates
   it with integers at a configurable precision (`StatData::
   rescale_precision`, default 53 bits) and takes the low 32 bits of the
   64-bit conversion for out-of-range results.
2. `stat-lists.md` §7.2: the monster stat-74 write is read as a step of
   its own after the rescale (not under its condition).
3. `stat-lists.md` §10.1 vs `units.md` §6.1: units.md gates stamina and
   mana on `0x00580610`; stat-lists.md runs all three steps. d2rs follows
   stat-lists.md; the player fraction update is read as inside the
   r ≠ 0 branch.
4. `stat-lists.md` §10.1 step 6: unit +0xB0 := 0 not modelled (field not
   described).
5. `stat-lists.md` §6.4: an op stat ≥ n other than 0xFFFF ends the loop
   (as the fix-up does).
6. `stat-lists.md` §6.2: the general "r = 0 → skip" is applied to the
   charstat contributions too.
7. `stat-lists.md` §10.3: "invalid skill" read as ≥ skills count.
8. `stat-lists.md` §10.4 / OQ5: an expired extended list panics in d2rs
   (an endless loop in 1.14d).
9. `stat-lists.md` §8.8: the stay-on-death rule `0x0063A4A0` is not
   written (seam, default false).
10. `units.md` §3.1: allocation without `SUNIT_Add` (flags bit 1 clear)
    returns `UnitError::NotAdded`: d2rs units always have list entries;
    and d2rs adds before the per-kind init (unobservable here). Item
    start seed taken as the derived `lo'`.
11. `units.md` §4.1: the frame count is set from the AnimData record in
    both branches of the prepare step; without a path the speed is left
    unchanged.
12. `units.md` §4.2: with speed 0 the variants are read as not writing
    +0x44; a variant reading an event byte outside the record (≥ 144) is
    an error, not a guess; negative speed is an error.
13. `units.md` §4.5: a failed `0x0057EEC0(4, 1, 0)` at the end of a
    knockback does nothing.
14. `units.md` §6.1: event 6 without a hover does nothing.
15. Real-data vectors of `stats.md` (entries(7), deps(12) = 214…250, A53
    on 214/215/218/219): checked on the synthetic table only; an ignored
    game-file test in `d2-sim` needs `d2-formats` as a dev-dependency
    (`ArchiveSet`) — **dependency request**, or run the check from
    `crates/conformance` instead.

## 6. Checks to queue (local)

1. With game files: build `StatData` from the fixed-up live set and
   assert the real-data rows of `stats.md` Test vectors (entries(7),
   deps(12), A53 set) — after the dependency decision of question 15.
2. Once `record_stats.py` recordings exist (HANDOFF §5): replay them
   through `StatLists` in `crates/conformance` (base/full/mod arrays,
   chains, flags, callbacks with nested writes, expiry, regen writes);
   this also settles open question 1.
3. Once `record_tick.py` 0.2.0 recordings exist: compare the schedules
   of `units::anim` and the dispatch against U4/U11 (anim records).
