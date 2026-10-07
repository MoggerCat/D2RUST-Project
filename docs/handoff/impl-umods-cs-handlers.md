# impl-umods-cs-handlers (2026-10-07, branch `claude/impl-umods-cs-handlers` on `claude/specs-staging-2` `b222388`)

## Done
1. **Umod callbacks** (`monsters/umod-callbacks.md`): all 32 callback bodies of `umods.tsv`
   in `d2-sim::monsters::init::callbacks` (dispatcher `run`), helpers §3.2 area damage,
   §3.3 cross burst, §3.4 elemental fill, §3.5 think restart; §3.1 unit find in
   `monsters::init::find` (`FindWorld`). New `InitHost` callback seams (defaults: nothing).
   `MonsterData.last_burst` (+0x18). `Unhandled::Assert` for §8 step 3.
   Dispatcher sites §2 r1/r2 moved inside `units::modes::monster_set_mode`
   (`UnitHooks::monster_umods`: mode 0 before the start fn, never GH; mode 1 after the
   animation prepare). Mode 4 (`umod_mode::GET_HIT`) is not wired: its sites are inside
   `Pending::reaction`'s monster branches (damage.md §7.1 OQ3).
   Wired providers (`wiring/worldgen/umod_host.rs`, `init_units.rs`): stats, base list,
   states/groups, positions, mode set (re-lends the world so the nested dispatcher runs),
   owners (missile store / `Pending::ai_owner`), minion links, missile creation, missile
   flags/velocity/skill-level, unit find on act rooms, line test, `0x005AD730` with a
   caller's record (`View::missile_record_hit`, refactored out of `apply_damage`),
   skill-66 aura fields and `eval_skill`, footprint test, area level.
   `WorldPending` seams (defaults): `umod_target`, `umod_target_position`,
   `umod_apply_state`, `clear_owner_data`, `remove_pet`, `quest_death`, `steal_belt_item`,
   `spawn_near`, `ai_param0`/`set_ai_param0`, `can_raise`, `ai_use_skill`, `skill_level`.
2. **unit-events.tsv**: the 15 rows now in `IMPLEMENTED_SITES`; population class 438
   (`0x0054ea84`) schedules event 7 at f+250+roll(50) on its own seed
   (`WorldPending::schedule_monumod` removed).
3. **C→S §9 handlers** (subagent): `d2-server/src/adapters/handlers/player.rs` (+ action
   provider, tests); 0x12, 0x4B, 0x51, 0x53/0x54 fully real; 0x14, 0x3F, 0x41, 0x46/0x47,
   0x48, 0x4D, 0x3D partly via new `Pending` seams; 0x5F → `handle_resync`; 0x44, 0x60
   entries real, bodies seams returning `None` (stay stubs); 0x15, 0x3E, 0x4C, 0x59 stubs
   with owners recorded. `d2-sim/src/units/mode_set.rs` (`0x00624690`) duplicates
   `modes::set_mode`'s body (TODO fold).
4. **0x77**: the sender already exists (`world::cube::trade_action`, items tests V24);
   the gap test now asserts it as a sim row.

## Gate (on the pushed head)
`cargo test -p d2-sim -p d2-server -p d2-proto`: green except the base reds owned by
others (skills::* 6, world::quests::tests::tables_parse_and_check). clippy -D warnings,
fmt, coverage --check (0 errors), spec_index --check: clean.

## Left / questions
- U1: wiring test of the mode-1 site in GH needs a GH animation record in the fixture.
- U2: multishot (§19) creating missiles inside a missile creation hits the lent missile
  store (`Reentrant("missiles")` error, no copies) — needs a store-lend redesign.
- U3: §3.1 filter: the excluded unit is tested only in the player branch (literal reading);
  flag 0x200 / room-box test for r < 0 unprovided. Confirm.
- U4: §22.2 A1 damage = pct(DM, A1MinD/MaxD, 100) (init.md §8.1 flag 8 reading); confirm.
- U5: §3.5 base class: "row missing" vs "outside the table" treated the same (-1).
- U6: `minion_owner` uses `WorldState::owners`; `clear_owner_data` drops that link.
- Subagent questions: 0x14 empty text (§2.4 r6 → 2 vs §9 r3 → 0); 0x59 owner is
  `ai-bodies.md` §9.9 not `ai.md`; 0x44 result vs `quests-act2-2.md` §3.2; text test
  `0x00413490`, `0x0053FDF0`, 0x41 flag bit 2 = "tile"?, `0x0058EF40` extra args,
  `0x005724C0` intro field, initial hot-key slots.
- No local run queue entry added (no game-file check needed beyond umod-callbacks OQ1).
