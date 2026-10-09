# q-fix-audit-rest: open seam / flow / proto / save / prov audit rows (2026-10-09)

Branch `claude/q-fix-audit-rest` (from staging-7). Status read from
`docs/HANDOFF.md`, `docs/handoff/q-fix-*.md`. Done rows are not listed.

## Open at start (one line each)

| Row | State at start |
|---|---|
| q-fix-seam-beltable | `fits_belt` is a fixed code list (inv_items.rs:164); scrolls never reach the belt |
| q-fix-seam-store-grid | shop repacks store items (shop_ui.rs page_items) instead of the stream's x, y |
| q-fix-seam-grid-facts | stack / book / cube / ctrl-sell facts hard-coded off; cube click mode 0 |
| q-fix-seam-click-sounds | ClickOut::Sound and interact outputs dropped |
| q-fix-seam-npc-dialog-capture | NPC menu level / unidentified count read at draw, not receive |
| q-fix-seam-shake-camera | pick cameras unshaken (latent: no shake starts) |
| q-fix-seam-room-order | code done, spec line + test missing |
| q-fix-seam-stamina-scale | needs spec decision (client/model.md OQ2) -> PC 1 |
| q-fix-seam-vitals-delta, q-fix-proto-vitals-dx-sign | needs the binary -> PC 1 |
| q-fix-proto-state-param-sign | needs the binary -> PC 1 |
| q-fix-proto-one-type | refactor across d2-proto + client (bytes identical today) |
| q-fix-proto-docs (rest) | spec-session items |
| q-fix-save-gaps | mouse skills fallback (1); (2)-(4) hireling / golem / status credits |
| q-fix-flow-join-load | J2 done: a refused load sends the direct S->C 0xB4 with the result before the client is removed. J1 done for the quest entry: `world/quests.md` §3 names the caller single player takes (`0x005344B0`, mode 0; the stub load of a new character calls mode 1 first), so the join runs `QuestControl::player_enters` through `QuestEnter` and sends its 0x5E, 0x28 (type 6), 0x29 (0x89 when due) in rule 3.1 (e)'s place (`SessionState::join_quest`, before 0x0B); the `PLAYER_STARTED_GAME` callbacks and the chain sequence functions now run at the join. **Left**: the loader's 0x22 / 0x21 per-item conditions (`pc1-data.md` Step 4 item 16) | `synthetic_game::session_refusals_stop_the_sequence` (0xB4 bytes); real install `app_single_player::the_join_sends_the_quest_entry_messages_before_the_player_record` (twice each for a new character) |
| q-fix-flow-client-order | C5 (Esc on dead player x3), C7 (doubled registrations) |
| q-fix-flow-act-change | steps 1, 6, 7, 10 left open |
| q-fix-prov-rec-ids | REC id clashes in HANDOFF |
| q-fix-prov-handoff-stale | SUPERSEDED marks on REC bullets |
| q-fix-prov-hireling-search | two d2rs-own hireling searches |
| q-fix-prov-frame-clock | 40 ms tick const repeated |

## Results (2026-10-09, branch `claude/q-fix-audit-rest`)

| Row | Result | Test |
|---|---|---|
| q-fix-seam-beltable | done: `ItemArtRow::beltable` (itemtypes `beltable` of the item's type and 1 x 1) read by `fits_belt(art, code)` and `BeltParts::beltable`; scrolls reach the belt | `inv_items::tests::belt::the_belt_test_reads_the_tables_not_a_code_list`; real install `seam_belt_tables::the_clients_belt_test_equals_the_sims_over_every_item` (every items row, `#[ignore]`, passes) |
| q-fix-seam-store-grid | done: the shop draws and hits store items at the stream's (x, y), no repacking (`shop_ui::page_items` / `in_grid`) | `shop_ui::tests::store_items_stay_at_the_servers_cells` |
| q-fix-seam-grid-facts | partly: stack (0x21), scroll-to-book (0x29), item-to-cube (0x2A) clicks now sent, from the item tables and the items' streams (`ui/panels/inv_items_facts.rs`: stack test §4.5, spell kind, cube room §2.3); cube grid mode 0x0E was already in. **Left**: the Ctrl-click sell (0x33 needs the open store's NPC, price and `sellable` `0x0062A130`; the shop panel's own sell stays) and a real-install click test (d2rs-own: a compact record reads as quality 2) | `inv_items_facts::tests` (stack rules, spell kind, cube room) |
| q-fix-seam-click-sounds | done: `world_clicks` returns the interact outputs; click `Sound` and the object sound outputs reach `UiSounds` in click order (`present::click_sounds`, `audio_request`); effect outputs still have no consumer | `present::click_sound_tests::a_world_clicks_sounds_become_player_event_requests_in_order` |
| q-fix-seam-npc-dialog-capture | done: `NpcDialog` carries `level`, `unidentified`, `expansion` captured at receive | `tests_ui_more::the_dialog_captures_the_menu_facts_at_receive` |
| q-fix-seam-shake-camera | done by q-fix-render-rest (`b174fc2`, the pick inverts the shaken camera); the remaining `(0, 0)` cameras are test helpers (`corpse_click::camera_for`) | n/a |
| q-fix-seam-room-order | done: spec line in `client/bridge.md` §1 r1 (revision note); tests exist in `tests_drlg` | `tests_drlg` `set_order` tests |
| q-fix-seam-pause | done earlier (q-fix-flow-server item 5) | `smoke_frontend::the_esc_menu_pauses_the_single_player_game` |
| q-fix-save-gaps | (1) mouse skills on load: exact (skill, owner), skill 0 / 5 never selected, no class fallback, header comment fixed. REC-265 (2): the classic Diablo credit raises the progression (`0x005B4D77`, step 4); the Act II / III sites go to PC 1 (`pc1-data.md` Step 4 item 15). (2)-(4), (5), (6), (7) of the row text were done by q-fix-flow-save / REC-265 (status, hireling block, golem, plain mouse words, swap pairs, hotkeys) | `app_save_gaps::absent_and_unknown_mouse_entries` (expectations follow `d2s.md` §2.4 r6.3); `quests::act4::q2` `classic_diablo_kill_and_end_of_game` (progression 0x0400) |
| q-fix-flow-join-load | J2 done: a refused load sends the direct S->C 0xB4 with the result before the client is removed. J1 **not done**: the quest entry `0x00546270` (0x5E, 0x28 type 6, 0x29, 0x89 and the `PLAYER_STARTED_GAME` callbacks) has no production caller (`QuestControl::player_enters`), and which caller single player takes is open (`quests.md` OQ 4); the loader's 0x22 / 0x21 per-item conditions are in the item specs. Queued: `pc1-data.md` Step 4 item 16 | `synthetic_game::session_refusals_stop_the_sequence` (0xB4 bytes) |
| q-fix-flow-client-order | C5 done: `save_on_death` and `leave_dead` run after `death_screen`. C7 is the install-then-replace of `add_visibility` / `GameAudio` (`play.rs`), by design, no behaviour effect; `UiSounds` is an idempotent `init_resource`. C2, C4 done by q-fix-flow-server | `app_play_death`, `app_play_hardcore` (real install, pass) |
| q-fix-flow-act-change | nothing left for single player: steps 1, 6, 7, 10 (arena flag test, classic pet drop, disguise check, removal to other clients) have no single-player effect | n/a |
| q-fix-prov-rec-ids | done: maze exit-warp entry is REC-480; REC-271 halves (a)/(b); repeats of REC-269/272/273 dropped (newest REC-273 kept) | `tools/provisional_index.py` regenerated |
| q-fix-prov-handoff-stale | done: SUPERSEDED marks (REC-109, 152, 154, 150, 158, 126, 105, 172, 186) | n/a |
| q-fix-prov-hireling-search | done: `stand_in_drives` names it (a hireling with the owner link takes the real Hireable think, search 35; the stand-in, 20, only drives hirelings without it and pets); the radius itself is `pc1-data.md` Step 4 item 3 | `hireling_drive::tests::a_hireling_with_the_owner_link_is_left_to_the_real_think` |
| q-fix-prov-frame-clock | done: one `rules::camera::CLIENT_TICK_MS`; `PentClock` steps the Esc-menu pentagram by the > 50 ms rule | `esc_art::tests::the_pentagram_steps_when_more_than_50_ms_passed` |

## Not done, and why

- q-fix-seam-vitals-delta, q-fix-proto-vitals-dx-sign, q-fix-proto-state-param-sign: need the binary (`pc1-data.md` Step 4 items 1, 13).
- q-fix-seam-stamina-scale: spec decision (`pc1-data.md` Step 4 item 14).
- q-fix-proto-one-type: a refactor across d2-proto and ~30 call sites in two crates with identical bytes today; left for a session with no behaviour work (other sessions edit d2-proto).
- q-fix-proto-docs (rest): spec-session items (0x50 sixth word, 0x18 `life_pred`, 0x26 `on_merc`, 0x7F / 0x90).
- Ctrl-click sell in q-fix-seam-grid-facts (above).
