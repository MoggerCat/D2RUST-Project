# impl-skill-bodies — the 1.14d skill start / do bodies in d2-sim

Branch `claude/impl-skill-bodies`, on `claude/tender-meitner-mphas3` at
`b435f5a`, then merged with `7f684ad` (coordinator request). Spec: `specs/skills/bodies.md` (local session, commit
`208d2a5`), with its updates to `skills/functions.tsv`, `levels.md`,
`use.md` and `combat/damage.md`. Implementation only; no spec edited.

## What landed

- **Bodies** (`crates/d2-sim/src/skills/use_/bodies/`, was `bodies.rs`):
  - `mod.rs`: the `BodyWorld` seam (extends `UseWorld`; `type List`,
    `type Combat: CombatWorld` reached by `combat()`), `run_start` /
    `run_do` for the 16 `spec'd-here` slots (`START_BODIES` = srvst 1,
    2, 3, 4, 5, 15, 29, 32, 33, 46, 65; `DO_BODIES` = srvdo 1, 2, 18,
    30, 65), `PURE_START` / `start` (Attract, run by the start core as
    before), the remove callback ids and `remove_callback`, `BodyTables`
    (itemstatcost `direct` / `updateanimrate` / `maxstat`, states
    `group` / `aura`, overlay count; `from_bin`, `from_tables`),
    `Handler`, `ScanRoom`, `MissileRequest`, `ProgressiveMsg`.
  - `helpers.rs` (§2): target, pair record, `is_bow` / `bow_missile` /
    `has_ammo` / `has_qty`, `skill_missile` (§2.4 record fill),
    `dec_quantity` / `use_one`, `aura_fill` / `passive_fill`,
    `apply_state`, the four remove callbacks, `clear_group`, `scaled`,
    `accepts` (`aurafilter`), `scan_unit` / `scan_point`, `register` /
    `unregister`, progressive `charges_before` / `charges_after` /
    `finisher`, `shape_start`.
  - `starts.rs` (§3), `dos.rs` (§4: Attack, melee-with-state, buff,
    curse with `can_switch`, basic aura with its callback and
    `0x005CEC90`).
- **Levels**: `add_element` (`0x0056C8E0`, §3.6) and `roll_elemental`
  now takes the record and places the value (was: returned `(EType, v,
  len)` for the caller to place; levels.md OQ4 answered).
- **Code mirror of functions.tsv**: `table::Status::SpecdHere`
  (`spec'd-here`) on the 16 rows; `check_tsv` compares it; perturbation
  tests: a status change (srvdo 65 → mapped) is reported, and
  `tests/bodies.rs` checks every `spec'd-here` row ⇔ `START_BODIES` /
  `DO_BODIES` ⇔ a "body: bodies.md §" note (four perturbations).
- **Core** (`use_/mod.rs`): the do core and the type-5 handler call
  `w.srvdo` directly (there are no world-free do bodies); the start
  core still runs `bodies::start` (Attract) itself.
- **Wiring** (`wiring/interaction/skill_use.rs`): `UseView` implements
  `BodyWorld` and its `SkillFunctions::srvst` / `srvdo` run
  `bodies::run_start` / `run_do` first, the `UseRest` seam otherwise.
  The d2-server `World` delegates to `UseView`, so the server host runs
  the bodies too. Real on the view: stat lists (alloc, attach, detach,
  free, list stats, expiry, remove callback), states (toggle + update
  queue, changed bits, group clear, `stays_on_death` from the stat
  host), type-12 timers, room scans (adjacency arrays, town test, room
  unit lists, `find_room` for the target point), combat (`melee_result`,
  `fill`, `start_combat`, `apply_melee`, combat lists), the missile store
  (creation, owner), unit flags, anim frame (+0x44), max life / mana /
  stamina. `UseMissiles::create_skill_missile` (the `srvmissile` path,
  §5) and `UseWorld::dec_quantity` now run `bodies::skill_missile` /
  `bodies::dec_quantity`.
- **ActionHooks**: `bodies: Option<Arc<BodyTables>>` (None by default)
  and `handlers: BTreeMap<UnitId, Vec<Handler>>` (unit +0x90, prepend;
  dropped when the unit is freed).
- **Pending** (new defaults, narrowest reading): `allied` (same unit),
  `pet_unsummonable`, `set_entry_param`, `composit_weapon_class`,
  `hand_class`, `item_shoots`, `item_stackable`, `item_max_stack`,
  `item_max_durability` (None → durability unchanged), `quantity_timer`,
  `send_item_stat`, `attack_cleanup`, `weapon_cleanup`,
  `passive_refresh`, `buff_refresh`, `skill_resync`,
  `passive_state_apply`, `set_ai_state`, `blood_mana`,
  `queue_progressive`.
- **UseRest**: `skill_missile_fill` and `dec_quantity` removed (the
  bodies spec both); their impls removed from the 7 implementors
  (d2-sim, d2-server, d2-client tests).
- **StatLists**: `remove_callback(l)` getter.

## Missile bodies (second request: `missiles.md` §R9.5 / §R9.6)

After the coordinator's merge of `origin/claude/tender-meitner-mphas3`
(`7f684ad`, spec branch `claude/spec-skill-bodies` @ `9f50a51`): the 11
`srvdo.tsv` / `srvhit.tsv` rows flipped to `spec'd-here` (27 rows in all
with the 16 skill slots) have bodies.

- `crates/d2-sim/src/missiles/bodies.rs`: server-do 2, 3, 5, 7, 8, 10,
  25 and server-hit 1, 4, 12, 13 with `sub_at_step` (`0x005A9720`),
  `unit_distance` (`0x006416D0`), `elem_roll` (`0x005A8C70`), `elem_len`
  (`0x005A8F20`), `area_damage` (`0x0056BAD0`), the per-unit
  `area_hit` (`0x0056B9C0`, over `CombatWorld`), `next_unit`
  (`0x0056BD10`). `catalogue::run_srv_do` / `run_srv_hit` dispatch
  them; `SRV_DO_IMPLEMENTED` / `SRV_HIT_IMPLEMENTED` updated.
- New seam `missiles::seams::MissileBodies` (part of `MissileWorld`,
  narrow defaults): missiles / skills formulas, path new-step flag and
  target, collision OR / stamp, animation frame, alive, demon / undead,
  the area scan and the area hit. The wired `View`
  (`wiring/action/missiles.rs`) implements all of them on real parts
  (eval on the skill tables, path provider flags / target,
  `drlg` collision, `path::footprint::stamp_size`, unit records, the
  combat view) except the scan: `Pending::missile_area_units` (default
  none; the scan of `skills/bodies.md` §2.12 needs `UseRest` for its
  line test, which the action view lacks; open SB10).
- `combat::monster_crit` extracted from `fill` step 13 (shared with the
  area hit; no behaviour change).
- Tests: `missiles/tests/r9.rs` (rule tests for each body on the missile
  fake); the stub tests that used server-do 2 / server-hit 1 as
  observable stubs now use 6 / 2 (still unspecified); the catalogue
  perturbation test's expectations follow the new rows.

## Tests

- `bodies/tests.rs`: event-table bounds, callback ids, constants,
  filter 73731 bits; ignored `body_tables_from_the_1_14d_tables`.
- `skills/tests.rs`: `add_element_by_etype` (every element, e = 10
  draw); `rolls_step_unit_seed` updated to the record form.
- Wired host (`wiring/interaction/tests/skill_bodies.rs`, through the
  start core / do core on `UseView`): Kick start (srvst 2; spec vectors:
  no T → 0, T → hit flags 2, result 9, physical 0, class 1, no draw);
  buff do (srvdo 18: same-group clear, duration, stats, 350/351,
  callback, timer 12); Amplify Damage (srvdo 30: −100 → −20 at base
  resist 100, list flags 0x22, duration 200); Dim Vision in Hell (175 /
  4 = 43, AI install k = 10, AI-curse callback); curse resistance 50
  halves the duration, a lower-level recast is refused, 100 refuses;
  Might (srvdo 65: F 1251 → expiry 1302 + timer, refresh at 1301 →
  1352, self-aura callback); `dec_quantity` (q 1 → 0 + 0x3E, q 0 → 0);
  progressive prgdam 2 n 3 → leech +20 / +20; the view runs exactly the
  16 body slots and forwards the rest to the seam.
- Other sessions' fixtures changed only where they used a body slot as
  a fake seam: d2-server `skills/tests.rs`, `mutants_handlers_skills.rs`,
  `prop_handle.rs`, d2-client `e2e_full_loop.rs`, `e2e_single_player.rs`,
  `prop_worldsim.rs` use srvst 6 (mapped) for Multiple Shot's 4 and srvdo
  66 for Might's 65, with the expected logs; d2-sim
  `tests/skill_events.rs` uses srvdo 66 for its aura. In
  `mutants_handlers_skills::immediate_aura_do_and_cooldown` the
  `"decquant 2"` log of the removed `UseRest::dec_quantity` is gone (the
  body runs; the core's call stays covered by the `use_` fake's log).

## Gate

`sh tools/gate.sh` at `18c188f` (after the merge with `7f684ad`): every
step passes (spec index, methods, coverage, trace checkers, hook, fmt,
depcheck, clippy, tests of every crate but two, doc-tests) except two
test steps, failing only on tests that fail without this branch too:

- d2-sim: `monsters::ai::tests::implemented_matches_catalogue` and
  `monsters::ai::tests::rules::d2moo_only_act1_ais_are_stubs` (the AI
  `functions.tsv` rows flipped to `spec'd-here` by `9f64a3a`, waiting for
  their implementation).
- d2-client: the bridge dispatch-table tests (`NoHandler` for 51
  server message ids of the new client specs; `bridge::tests::*`,
  `bridge::local_tests::*`). Before the merge 45 client tests failed on
  the base (checked by stashing this branch: the same list); after it,
  the bridge table tests remain.

## Open questions (new; owner in brackets)

- SB1 [levels.md §3.6] `add_element` names a hit class per element
  (0x20, 0x40, 0x30, 0x50, 0x60) without saying how record +0x60 is
  written (set, or-ed, high nibble). d2rs leaves the record's hit class
  unchanged and returns the value (`ElementAdded::hit_class`).
- SB2 [bodies.md §2.13] register accepts func ≤ 49 but the event table
  has 32 entries: what 1.14d reads for 32–49. d2rs refuses them.
- SB3 [bodies.md §2.14] prgdam 3 / 4: whether `result |= 0x4000` is part
  of the freeze clause. d2rs sets it on every prgdam 3 / 4 charge.
- SB4 [wiring; stat-lists.md §8.2] The remove callbacks run when a body
  detaches a list (`UseView::detach_free`), not when a list expires
  (timer 12 → `expire_lists` → the stat host's `list_removed`, a no-op
  in `ActionHooks`: the hooks have no unit records). Until a host routes
  expiry through the view, expired buff / curse / aura lists leave their
  state bits on.
- SB5 [bodies.md §1] `0x00639DB0` "toggle with update-queue insert":
  d2rs queues the unit after every toggle (changed or not).
- SB6 [bodies.md §4.4] The curse context's unused stat slots: d2rs
  starts them at −1 (values 0, so nothing is set either way).
- SB7 [wiring] `CombatWorld::curse` (open wounds, `damage.md` §8) still
  goes to `Pending::curse`; it is `apply_state` (§2.7), which needs a
  `BodyWorld` (the combat view alone is not one).
- SB8 [hosts] No host loads `BodyTables` yet: set
  `hooks.bodies = Some(Arc::new(BodyTables::from_bin(set)?))` where the
  game hosts build `ActionHooks` (d2-server game host, `WorldSim`).
  Without it: no itemstatcost record (curse stats stop at stat 1, aura
  stats skip), every state group 0 (`clear_group` does nothing), no
  overlays.
- SB9 [bodies.md §2.1] The target refresh (`0x00553490`) is the
  provider's `UseRest::target`.
- SB10 [wiring] `Pending::missile_area_units`: the missile area bodies
  (server-hit 1, 12, 13) find no units on the wired host until a
  provider runs `bodies::scan_unit` for the action view (it needs the
  line test of `UseRest`).
- SB11 [missiles.md §R9.6] Evade (`block_or_dodge` result 8) has no
  listed result bit in the area hit; d2rs only clears the hit bit.
  Server-hit 13's "len = sHitPar2, or eval(…)": read as the formula
  when sHitPar2 ≤ 0. Server-do 8 / 10 with no record (fatal in 1.14d):
  d2rs keeps the missile (1).
- Spec open questions 1–7 of `bodies.md` stand (recordings, `0x00580310`
  / `0x00580380`, Bash's attack-rate list removal, `0x00575900`, handler
  iteration, `0x005B0DA0`).

## Local run queue (game files)

1. `D2_GAME_DIR=… cargo test -p d2-sim --lib body_tables_from_the_1_14d_tables -- --ignored --nocapture`
   Expect: pass (one record per itemstatcost row, 185 states, stats 6 /
   8 / 10 `direct` with `maxstat` 7 / 9 / 11). Record the printed
   overlay count and the non-zero state groups (§2.9 `clear_group` reads
   them for the buffs of §4.3).
2. The spec's recordings (bodies.md OQ1–3) are unchanged; when they
   land, compare `0x0056E970` / `0x005CF010` results with the
   `skill_bodies` wired tests (expiry, flags, list stats).
