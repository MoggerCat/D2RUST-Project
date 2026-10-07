# Handoff: save load effects and hireling answers — `claude/impl-d2s-load-hirelings`

Cloud implementation session, 2026-10-07, implementation from specs
(medium). Base: `claude/specs-staging-2` at `b222388`. Repo only, no game
files (M09): every claim holds on this branch, on synthetic data. Stopped
early on the coordinator's budget cut; "Left" lists what is not done.

## 1. Done

### Hirelings (`specs/world/hirelings.md`, `npc.md` §7.4)

| Item | Change |
|---|---|
| GN1 / OQ6 | `world/npc/hire.rs` `resurrect`: a living `(7, 1)` node (it is what `(7, 0)` returns) answers 0x2A code 9 before the cost; nothing else changes. Test `resurrect_refuses_a_living_hireling` |
| HL1 | `ExpRatios` (`world/hirelings.rs`, `0x00613E60` + step `0x0057E390`) on `HirelingTables::exp_ratios`; `level::gain` / `kill_share` use it (the closure parameter is gone). `from_tables` now takes `experience` (MaxLvl from it). Test `exp_ratio_vectors` (all four Test vectors) |
| HL2 (0x7A layout) | `pets::pet_action` writes owner @5, pet @9 (spec corrected; recorded remove `7a 00 … 0d000000`) |
| HL3 | eviction unchanged (unlink + kill), comment states the answer |
| HL4 | `pets::recompute_max` (`0x00575900`, type 7): `HirelingWorld::skill_pet_max`, else `basemax`; `HirelingRest::skill_pet_max` has a default `None` (no skill of `pettype` 7 in 1.14d) |
| HL5, HL6 | TODOs removed (init continues; base reads) |
| HL7 | `items::swap`: notices merc then player; `0x0055DF00` after `0x0055C730` before the requirement check; all four refresh calls on the merc (`HirelingItems` refresh seams take one unit) |
| HL8 / OQ7 | `level::send_stats` queues on the merc (`HirelingWorld::queue_stat`, wiring: `InteractionState::unit_stats`, `unit_stat_messages`, `free_unit_stats`); damage sums under ids **21 / 22** (were 23 / 24); `level::flush_stats` for the client pass; `restore_experience` queues stat 13 |
| HL9 | `HirelingRows::act_of(expansion, class, name)` / `act_of_name(expansion, name)`: version filter, class match first, then name range; `npc/hire.rs` step 4 the same (its "none" gave `u32::MAX`, now 0) |
| OQ8 / WW-2 | `life::on_kill` (§8 rule 1: flag, player owner → `death`); `HirelingWorld::player_by_guid`; `ActionHooks::pet_deaths` queue filled by `reaction::kill`, drained by `WiredWorld::pet_deaths` (after handlers / ticks, beside `pet_follows`; TODO: 1.14d runs it inside the kill) |
| OQ9 part | `hirelings/tests/game.rs` (`#[ignore]`, `D2_GAME_DIR`): unit columns of the Test-vector table, the recorded hire of Diane (Id 0, L 7, the 14 queued stats), the live `ExpRatio` column |

### Saves

| Item | Change |
|---|---|
| d2-formats | `item_flags_on_load` + `ITEM_FLAG_*` (§8.2 r7), `ItemEntry::record_flags` / `set_record_flags`, `Header::reset_appearance` (§2.8), `Npcs::set_intro_a` (§6 r3 setter `0x00572360`); tests for the C66 / §8.4 r5 vectors |
| DS-1..DS-5 | item reader already matched DS-2a/2b (alt-code ends at base code, no children for compact / alt); DS-3 keeps 23; DS-4: `--quests all` stays refused, message cites `quests.md` §1.8 r3 / OQ14; DS-5 unchanged (1-bit 0 trailer) |
| d2s-tool | `new`: items without 0x2000, appearance 32 × 0xFF; `set`: `save::resave` (0x2000 cleared in every record incl. children, corpse, hireling, golem; appearance reset unless an item is equipped → note, OQ17) |
| d2-server | `adapters::character` (`d2s-load.md` §1 / §2): `load` in the master's order on `CharacterWorld`, `header_load`, `normalise_quests` (`quests.md` §1.6), `gold_limits`, golem skill 90 check, post-load; `ActionCharacter` provider on the action `View` (stats, start stats, `StartSkill`, nextexp); `session::enter_game_from_save` = load, quest entry mode 0 (full save), then `enter_game` in the save's act |
| test-fixtures | `synthetic_game` `a_full_save_loads_before_the_join_sequence`, `a_stub_starts_a_new_character_before_the_join_sequence` |

Tests corrected to the spec (old behaviour asserted; listed per the rule):
`hirelings::tests::level::send_stats_order_and_sums` (ids 21/22, queued),
`…::level::one_gain_raises_several_levels`, `…::level` reload test
(stat 13 queued), `…::life::revive_order` (stats queued),
`…::items::{empty_target_…, occupied_pass_…, occupied_fail_…}` (HL7
units and order), `…::pets::pet_action_vector` (owner @5), `…::rows::act_of_name`
(new signature, version filter), `world::npc::tests` fake (`(7, 0)` only
for a living node).

## 2. Gate

`CARGO_INCREMENTAL=0 cargo test -p d2-sim -p d2-server -p d2-formats -p d2s-tool`
(+ `-p test-fixtures`): green except 8 tests red on the base too
(`d2-sim` skills table checks ×6, `world::quests::tests::tables_parse_and_check`;
`d2-server` `tests::gaps::out_of_scope_rows`), checked on `b222388`.
clippy `-D warnings`, fmt, `coverage.py --check` (0 errors),
`spec_index.py --check` clean.

## 3. Left

- Load providers: every `Unapplied` step of `ActionCharacter` (header
  client fields, quests on the host rest, waypoints, NPC fields, skills,
  item creation from records, corpse, hireling restore unit creation —
  place not stated in `hirelings.md` §10 r3 —, item indices, mouse
  skills, start items, quest entry on `WiredWorld`).
- Question: does the caller run the quest entry mode 0 after a
  new-character start (`d2s-load.md` §1)? Not run.
- The client pass flush of `InteractionState::unit_stats` (no client
  pass 0x67–0x6D yet); `HirelingTables` production loader.
- `d2-client/tests/e2e_pet_action.rs` (ignored, I-1) should now pass
  with the 0x7A fix; not built here (client).
- HANDOFF.md / PLAN.md fold of this note; the local run: the three
  `hirelings/tests/game.rs` tests and C66 (2) again with the new
  `d2s-tool new`.
