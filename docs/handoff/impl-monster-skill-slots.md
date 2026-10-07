# impl-monster-skill-slots — batch 4 skill bodies (monster slots)

Branch `claude/impl-monster-skill-slots`, on `claude/specs-staging-2` at
`b222388`. Specs: `specs/skills/bodies-3.md`, `specs/skills/bodies-4.md`,
`specs/skills/functions.tsv`. Implementation only; no spec edited.
Stopped early on a coordinator budget cut: see "Left".

## Done

- `table.rs`: every `functions.tsv` `spec'd-here` row is `SpecdHere`
  (213 slots: 64 start, 149 do; only srvdo 53 / 138 / 142 stay
  `unreferenced`).
- Bodies for all 83 batch 4 slots:
  - `bodies/b4_helpers.rs`: `bodies-3.md` §3 (spawn-column class,
    Inferno channel end / frames / animation, Throw, monster swing, next
    action event, revive, resurrect mode, `dir8` and the offset pairs)
    and `bodies-4.md` §2 (weapon roll, stacking state list, imp possess /
    release, lightning fan / ring with their init callbacks over a new
    `PathMissile` trait, DiabWall callback, whip state, transform, vines).
  - `bodies/b4_mon.rs`: `bodies-3.md` §4–§5.
  - `bodies/b4_more.rs`: `bodies-4.md` §3–§4 and the pregnant remove
    callback (`callback::PREGNANT`; `remove_callback` now also takes the
    combat tables).
  - `run_start` / `run_do` dispatch them; `START_BODIES` / `DO_BODIES`.
- Seams: new `BodyWorld` methods (dir64, action frame +0x4E, +0x3C,
  +0x4C, missile frames, sequence / AnimData events, action-event test,
  chain position, books row, inventory nodes, unit find, point collision,
  `spawn_monster(MonsterSpawn)`, `place_unit_flag`, component byte); new
  `BodyEffect` variants (fixed-pattern stamp / clear, dead footprint,
  evil counter, quest chain link, KillBy, PetRemove, UseItem, mode request
  build / send, minion command, class change, wait think, every-tick
  event, path follow) and `PathOp::TurnToward`. `UseView` wires the unit
  fields it has (+0x4E, +0x4C, sequence, AnimData, chain via
  `ai_chain_index`, component via `ai_component`); the rest go to new
  `Pending::body_*` defaults (narrowest reading).
- Answers of `bodies-3.md` §2 applied where touched: `summon_class`
  writes the mode only for a valid class (answer 3).
- Fix: `dos::curse_unit` returned 0 for `aurastat1` = −1; `bodies.md`
  §4.4 step 2 says it goes on with v1 = 0 (MonCurseCast's Life Tap /
  Decrepify contexts need it).
- `combat::free_records` made public (bodies free T's records directly).
- Tests: the six target tests pass (`function_tables_match_tsv`,
  `function_table_check_reports_perturbations`, `bodies_match_tsv_notes`,
  `bodies_check_reports_perturbations`, `table_check_mutants::{
  repeated_slot_names_kind, index_bounds}`). Perturbations that used
  `mapped` rows now use unreferenced rows; new perturbations for a
  `bodies-3.md` note and a `bodies-4.md` demotion. The srvdo 54 one now
  matches the TSV's `bodies-2.md §8.14` note. Fixtures that used
  now-bodied slots as "seam" stand-ins: d2-sim `skill_bodies`
  (srvdo 53), `skill_events` (srvdo 53 / 138); d2-server lib skills tests
  (srvst 52 Emerge, checked through its flags, and srvdo 53).

## Gate (this container)

- `cargo test -p d2-sim --lib`: all pass except
  `world::quests::tests::tables_parse_and_check` (red on the base,
  another session's).
- `cargo test -p d2-server --lib`: all pass except
  `tests::gaps::out_of_scope_rows` (red on the base).
- clippy -D warnings, fmt, `coverage.py --check`, `spec_index.py --check`:
  clean.

## Left (red because of this branch; next session)

1. d2-server integration fixtures still use srvst 42 / srvdo 3 / srvdo 111
   as seam stand-ins, which now run bodies:
   `tests/mutants_handlers_skills.rs` (10 tests), `tests/prop_handle.rs`,
   `tests/e2e_night_world.rs::a_population_monster_killed_with_a_missile`
   (via `d2-sim::wiring::action::tests::fight::skills`, srvst 42 / srvdo
   3), and d2-client `e2e_full_loop.rs` / `e2e_single_player.rs`. No
   start slot without a body exists any more: the "srvst N skill lvl"
   log assertions need another observation (proposal: srvst 52 + used
   entry + mana for the level, or a test `Pending` override that logs a
   body's effect); do slots can move to srvdo 53. Do not weaken them.
2. No unit tests yet for the batch 4 bodies (spec test vectors:
   `bodies-3.md` / `bodies-4.md` Test vectors); coverage claims are
   missing for those rule units.
3. Questions (TODO(spec) in code): throw mastery for a non-throw item
   (§3.3 step 6, read via `weapon_mastery`); FetishAura's finder room
   (OQ5); DiabPrison object target (OQ6); Baal Tentacle spawn info for a
   non-Baal unit (OQ4, nothing spawned); zigzag step ≤ 0 (Edge case 1,
   nothing created instead of a hang); Imp Teleport's point when the
   target position fails (read (0, 0)); "E flags bit 2" read as mask 2
   (the move-ended flag of `use.md` §5.2).
