# Handoff: q-fix-npc-menus (2026-10-09)

Branch `claude/q-fix-npc-menus`. REC-1040 used.

## Done

1. **Natalya (297), Halbu (257), Jamella (405), Nihlathak (515) open no menu.**
   Root cause is not the server: each stands on a "Dummy" object (objects
   row 382, `Selectable0`–`7` all 0). The d2rs-own preview hover pick
   (`bridge/hover.rs`) took the nearer object, so the click sent C→S 0x13
   for the object (type 2) and no NPC menu came. The pick now skips an
   object whose current mode is not `Selectable` (`ClientObjects::selectable`,
   set with the object rows). Still d2rs-own, unverified (REC-1040).
   Tests (real install, `#[ignore]`): `natalya_opens_her_menu`,
   `act4_and_act5_town_npcs_the_pick_missed_open_their_menus` in
   `crates/d2-client/tests/app_play_act3.rs`; unit test
   `a_non_selectable_object_under_an_npc_is_not_picked`.
2. **Esc in an NPC dialog** sends C→S 0x30 (menu: one Esc; topic box: Esc) and
   later talks still open: `esc_in_an_npc_menu_ends_the_chat_and_later_talks_still_open`.
   Already correct on this base (no change needed).
3. **Esc with the "quest log" open: no code change.** The panel the finding
   calls the quest log is ui 0x11 (UI_QUESTLOG); the Q-key quest screen is
   0x0F (Esc-closable; Esc closes it only). Per `panels.md` §2 r9 ui 0x11 has
   Esc flag 0 and per `frontend-options.md` §O1 r2 Esc then opens the game
   menu, which closes it and the next Esc restores it (keep = 1). The client
   does exactly that, so the report "Esc also opens the Esc menu" matches the
   spec. The coordinator's premise (Esc closes the panel only) is not what the
   specs say. Pinned by `esc_opens_the_game_menu_only_when_nothing_is_closable`.
   Open question queued for PC 1 (pc1-data.md Step 4, unnumbered).

## Open

- No 1.14d `tools/scenario-diff` check: driving a click on these NPCs in 1.14d
  needs the Windows game. The pick is d2rs-own, so there is nothing to diff.
- `AppRest::or_unit_flags` has no provider: warping to Harrogath logs
  "unit flags N 0x3000000" as unhandled host calls (preset units, `warp_tile.rs`
  `mark_preset_unit`). The Nihlathak test tolerates only that note. Owner
  unknown (`tools/coord/route.py` could not route `app/rest.rs`).
- `app_play_act3::kurast_docks_arrival_and_every_town_npc_talks` failed at
  Cain (`approach`: "unit 1/[245] not reached") before and after this change.
  Not investigated.
- Playthrough count not run (the playthrough uses state-dump, not clicks).

## Repro

```sh
export D2_GAME_DIR=/home/user/game
cargo test -p d2-client --test app_play_act3 -- --ignored natalya act4_and_act5 esc_
```
