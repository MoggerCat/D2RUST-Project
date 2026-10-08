# q-fix-proto: the q-proto-audit fix rows

Session q-fix-proto, 2026-10-08, branch `claude/q-fix-proto` (from `claude/q-proto-audit`,
staging `specs-staging-7` merged in). Method: M22 (provisional over blocked), M23 (contract
checks at seams). Audit and index: `docs/handoff/q-proto-audit.md`,
`docs/handoff/proto-index.tsv` (rows updated for every id below).

Each fix has a contract test in the `tests_proto_*` pattern: the d2-sim / d2-server builder's
bytes == the d2-proto encode (or the spec packing) == what the client handler reads.

## Done

| Row / finding | What changed | Tests |
|---|---|---|
| audit "rubber-banding" suspect 2 (stale staged position) | `d2-server adapters/sim.rs` `refresh_targets`: the world's live facts (position, act) win over the staged copy whenever the world has the unit; the app staged (0, 0) act 0 and only the tick moved the copy, so in-range walks before the first tick, after a same-frame warp or across an act were refused and 25 frames later S→C 0x15 snapped the player back. `action.rs` `live_facts` answers `None` for a unit without a path (its (0, 0) is not a position). | `tests::adapters::point_parse_reads_the_live_position_over_a_stale_staged_one` |
| q-fix-proto-item-stat-0x3e (P2) | `d2-sim units/messages.rs::update_item_stat`; the item moves (`wiring/inventory/pending.rs`), the vendors (`wiring/interaction/vendor_world.rs`) and the skill bodies (`wiring/interaction/skill_use.rs`) build S→C 0x3E from the item's layer-0 base value and send it through their transport; `VendorRest::send_item_stat` and `Pending::send_item_stat` removed (the play rest only logged). **PROVISIONAL** widths (narrowest that holds; param 0): `client/msg-stats-items.md` §5 r1.3, **REC-336** (the assigned REC-296 is q-render-compare's in staging, so the next free id was taken; please renumber if 336 is someone else's). | `tests_proto_d::update_item_stats_0x3e_sim_builder_to_client`; stack / tome tests now assert the 0x3E bytes (`wiring::inventory::tests::stack`, `d2-server items::moves::tests`, `skill_bodies`, `mutant_tests`) |
| q-fix-proto-missing-producers, 0x22 (P3) | `units/messages.rs::update_item_skill`; the inventory desk's `send_skill_quantity` sends it (tome / scroll quantities); `InvRest::send_skill_quantity` removed. | `tests_proto_a::update_item_skill_0x22_one_layout` (sim == proto == client); `wiring::inventory::tests::host::skill_quantity_sends_0x22` |
| q-fix-proto-roster-join (P4) | No spec blocker any more: `client/msg-units.md` §8 r3 has the 1.14d sender's words. Builders `player_joined` / `player_left` / `player_kill_count` / `player_event` (d2-server's `player_event` now re-exports it). `ActionSim::join_sequence` (`wiring/action/dispatch.rs`): 0x5B (level = stat 12, party 0xFFFF), 0x65 (count 0), the join 0x5A to every client in state 4. `SessionFlow::leave`: 0x5C to the remaining clients before the leave 0x5A. 0x8D (parties) not sent. **PROVISIONAL** multi-client distribution: `intents-events.md` §8.3, **REC-337**. | `tests_proto_b::roster_join_and_leave_sim_builders_to_client`; `test-fixtures synthetic_game::join_and_leave_send_the_roster_messages` (single-player join end to end, then a second client sees 0x5C + 0x5A); `e2e_night_flows` and `rooms` join expectations extended |
| q-fix-proto-quest-special (P6) | `s2c::parse`: code 1 with bytes 9–14 zero → `s2c::QuestSpecial`; every other form → new `Message::QuestSpecialForm(gen::QuestSpecial)`. Audit 0x50 → built; `s2c-builders.md` updated. | `tests_proto_b::quest_special_0x50_parse_every_quest_form` un-ignored and extended (codes 2, 4, 13, 23, 1); `d2-proto s2c::tests` mercenary form |
| q-fix-proto-event-records (P7), 0xA3 / 0xA4 | `wiring/action/event_records.rs`: the per-unit record list (unit +0xEC), writers 0xA3 (`queue_progressive`, x = roll, y = 0) and 0xA4 (AI class preload), flush `send_event_records` in the monster update step 3, the monster add part A, the player part B and the object update; freed by the room clean-up step 2. `Pending::queue_progressive` / `ai_preload_class` removed. | `tests_proto_d::skill_npc_baal_0xa3_0xa4_0xa5_0xab_one_layout` (sim == proto == client); `wiring::action::tests::sound::pending_event_records_are_sent_then_freed` |
| q-fix-proto-npc-enchants (P8), 0x57 | `units/messages.rs::npc_enchants`; monster update step 10 (unit flag 0x800 and monster data +0x5C bit 1, `intents-events.md` §7.3 r2.10, now written). | `tests_proto_b::npc_enchants_0x57_one_layout` (sim == proto == client); `sound::monster_update_sends_npc_enchants` |
| q-fix-proto-packed-cut (P12) | `d2_proto::schema::packed_put` cuts a wide value to its field (as `combat/vitals.md` §5.4) instead of panicking. | `tsv::tests::packed_put_cuts_wide_values` (replaces the panic test); `tests_proto_c::life_mana_update2_0x95_one_layout` typed over-wide case |
| q-fix-proto-chat15-check (P15) | `d2-server dispatch.rs` parse: 0x15 msg strlen < 256 and strlen + 4 < size, else 2. **PROVISIONAL** code: `intents-events.md` §2.4 r6, **REC-338**. | `tests::messages::chat_string_checks` |
| q-fix-proto-4c-route (P14), 0x4C | Marked `NoOwner` with the reason (`world/cube.md` §10: the Transmogrify body belongs to the unwritten item-use spec; which item its always-sent 0x3F names is not stated). | id-table tests |
| q-fix-proto-docs (P16), part | 0x5A TSV `account:cstr16@24` (regenerated; `event_message_0x5a_one_layout` extended); 0x96 "never sent" fixed in `walk.rs` and HANDOFF 3ag; `npc.rs` 0x27 "partial" comment. | — |

## Not done, and why

- **q-fix-proto-vitals-dx-sign (P1)**: not this session's (needs the binary on PC 1); its ignored test is untouched.
- **q-fix-proto-0x92-client (P9)**: every effect of §5 r5 lands on structures the client model does not hold (inventory nodes, body slots, set-item stat links: `items/inventory.md` OQ1); the handler stays the size check with its PROVISIONAL note. Needs the client inventory model first.
- **q-fix-proto-state-param-sign (P11)**: both sides follow their specs; settling the read sign needs the local RE read of `0x0045EE20` / `0x00470E30` (`stat-lists.md` OQ5). No change.
- **q-fix-proto-one-type (P13)**: not started (a refactor across d2-proto and the client handlers; bytes are identical today and tested).
- **q-fix-proto-missing-producers, rest**: 0x11 (monster update step 9) needs the overlay count (data tables +0xBC0) in the action tables; 0x99 / 0x9A, 0xA5, 0xAB, 0xA6 and 0x23 records have no d2rs writer; 0x20 / 0x73 / 0x93 not looked at.
- **q-fix-proto-docs, rest**: 0x50's sixth client word (`msg-ui.md` §7 vs the TSV), 0x18 `life_pred` / `mana_pred` vs a / b, 0x26 `on_merc` vs `shift`, 0x7F / 0x90 sharing a sender: spec-session items.
- **Coordination**: `join_sequence` in `wiring/action/dispatch.rs` is q-fix-flow-server's hook; this branch adds its body (the roster messages). q-fix-flow-server should keep it when it merges (its 0x8D / host callback steps go around it).
