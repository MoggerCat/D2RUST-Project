# Handoff: quest answers (Acts I–III, tables, wiring) — `claude/impl-quests-answers`

Cloud implementation session, 2026-10-07, task class: implementation from
clear specs, medium (METHODS M14). Base: `claude/specs-staging-2` at
`b222388`. Repo only, synthetic data (M09): nothing here is verified
against 1.14d (M02). Stopped early on a coordinator budget cut; the
"Left" list below is the remaining work.

## Done

| Item | Where | What |
|---|---|---|
| quests.tsv / quest-messages.tsv | `world/quests/tests.rs` | `tables_parse_and_check` green (794 messages, row 40 specified, 2 callbacks); new `act5_intro_matches_its_rows` keeps `act5::intro`'s own init and `TABLE` equal to row 40 and table `0x00732FF8` (M05). The row-40 filter TODO in `QuestControl::new` removed. |
| QA-1 / QA-2 (roomless objects) | `act1/q4.rs`, `act1/q5.rs`; new seam `QuestWorld::unit_xy` (default: `unit_position`'s) | gibbet event 7: no spawn / free spot with a null room, only the portal call is fatal (`Fatal(0x0056D147)`); marker init and "Cain leaves Tristram": null room spawns nothing; town Cain at marker: nothing; trap chest: no spawn without a room, missile still at the chest once T exists. `marker_init` takes `Option<RoomId>` (init 54 passes the init's room as is). |
| QA-3, QA-5, QA-6 | — | confirmed: the code already matched (no change). |
| QA-4 | `act1/q6.rs` credit, `raise_progression` returns `bool` | A1Q6 credit without a client → `Fatal(0x00538684)`. |
| WW-6 | `act1/q4.rs`: `gibbet_init`, `tree_init`, `cain_portal_init`, `cain_portal_event` (dispatcher class 0xBD Act I), `wirt_body_operate`; new Extra4 fields `town_portal_count` (+0x80), `b92` | dispatcher `wiring/economy/quest_objects.rs`: inits 7, 9, 61, operate 33. |
| WW-10 | `DrlgWorld::refresh_room` (flag 0x400000) | `HostQuests::refresh_room` uses it; null room → nothing. |
| N-2 | `HostQuests` (`wiring/economy/quest_host.rs`) | `unit_position`, `unit_xy` from the path provider and list room; `room_contains` from the active room box; `room_at` = `DrlgWorld::find_room`. Rest only for units / rooms the action wiring does not know. |
| §9 item 7 / C79 / N-3 (inits inside the allocation) | `objects::create` split into `create_init` (rules 1–6) and `create_rest` (7–9); `View::allocate` defers the object init until the allocation's seed step is back in the hooks; `ActionHooks::quest_host` (`QuestObjectHost`, provider `economy::QuestLoan`); `View::object_route` runs a quest route at once on the lent host (routes raised inside it are queued and run right after it) | `d2-server`: `WorldHost::run_tick` (default `tick::tick`), `SimGame::tick` calls it; `WiredWorld::lend_quests` lends quest control, rest, item tables, uniques around the tick and the 0x13 object case (`R: Default + 'static` now required). Drains after desk calls stay for allocations made inside a quest call. |
| Act II (QB-1–QB-20, act2-2 §2 Jerhyn, §3 orifice, Tyrael party tail) | subagent commit `0c97cc8`, merged | see that commit; open: palace spawn / init 18 without a free spot, non-orifice insert, 0x58 byte 6. Dispatcher wired: inits 18, 19; operate 25 → `orifice_operate_checked`. New seam `QuestWorld::set_drop_code` (default unhandled). |
| Act III (QC-1–QC-5, act3-2 §11) | subagent commit `f2959d7`, merged | Mephisto progression via `raise_progression`; helpers `gidbinn_kill_test`, `weapon_in_use`, `drop_item_level`; `InitPoint` for the decoy, altar, Hratli and wanderer inits. Open: QC-3b class with no monstats row. |

## Left (not done)

1. Tests for the lent quest host on the wired host (d2-server
   `world/tests/quest_objects.rs`: an init raised in a tick's room pass
   runs in that tick; e2e N-3 tick assertions in
   `test-fixtures/tests/e2e_night_flows.rs` can then be tightened to the
   same tick, and N-2's fatal assertion changes to Cain's spawn).
2. QC-6: Act III inits are not in the `quest_objects` dispatcher (they
   need `InitPoint` from the init's room, x, y); host providers for the
   Act III seams (`has_act3`, `spawn_monster_in_room`, `room_covering`,
   `special_monster` via `gidbinn_kill_test`, `weapon_code` via
   `weapon_in_use`, …) are not written.
3. C→S 0x44 → `act2::q6::item_to_object` is not routed.
4. DS-4 (`quests.md` §1.8): `tools/d2s-tool` `--quests all` still refused;
   rule 4 gives bit 0 of slots 0–40 minus 34 as the minimal accepted
   record (per-path bits wait on OQ14).
5. `set_room_portal(room, on)` (Act II, IV, V) stays on the rest: its
   `on` ↔ 0x0061AED0 `clear` mapping is not stated per site.
6. `QuestTick` (d2-sim only) still drains after hooks (no lent host).
7. New reds this branch leaves (each a test whose spec or finding
   changed; not edited, never weakened):
   - `test-fixtures` `e2e_night_flows::opening_cains_gibbet_and_its_event_7`:
     asserted N-2's `Fatal(0x00593290)`; with the provider the fault is
     gone. Update to Cain's spawn / §1.2 step 4 outcome.
   - `e2e_night_flows::reading_horazons_journal` (line 845): QB-7 (act2-2
     §1.7) now grants / flags only in not-intro games with state ≠ 5;
     re-derive the expected bytes.
   - `d2-server` `world::tests::quest_objects::a_quest_operate_no_spec_states_is_handed_back`
     and `quests_act1::sisters_to_the_slaughter_through_every_state`: not
     investigated (likely operate 33 now stated, and the A1Q6 client
     fatal of QA-4 on a host without client save flags).
8. clippy `-D warnings` and `spec_index --check` were not run after the
   last edits.

## Gate

`CARGO_INCREMENTAL=0 cargo test -p d2-sim -p d2-server -p test-fixtures`:
d2-sim 3244 passed, 6 failed (the known `skills::*`); d2-server lib 3
failed (known `gaps::out_of_scope_rows` + 2 new, item 7); test-fixtures
e2e_night_flows 2 failed (item 7). `tables_parse_and_check` green.
`python3 tools/coverage.py --check`: 8,480 claims, 0 errors.
