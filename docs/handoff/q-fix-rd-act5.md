# q-fix-rd-act5 (REC-1560..1564)

## Done
play_act5 real-data tests: 8 of 9 pass (baseline: 5 failing). Fixed:
- **Test staging** (`crates/d2-client/tests/play_act5.rs`): `dead()` compared a recycled `UnitId` (a drop reused the dead monster's slot) -> now checks type + GUID; the player is re-strengthened at warp and at every hop (it died on arrival in level 110 with 1 life); Baal's room is kept active while waves are cleared.
- **REC-1560** (PROVISIONAL): inactive-unit restore (`wiring/action/inactive.rs`) re-runs the boss-mod `Chain` steps; a restored prison door (434) lost its chain-32 link, so its death never reached the rescue quest (`the_caged_barbarians_are_rescued`, shenk via hop fix).
- **REC-1561** (PROVISIONAL): Ancients' gate `0x0058CF90` answers `!armed` and no longer clears the armed byte (`single_player.rs`, `Pending::set_ancients_armed`, published by `quest_events.rs`). Spec §7.9 reads it as "armed := 0", under which no Ancient death counts. PC1 item queued.
- **REC-1562** (PROVISIONAL): the bridge feeds `inputs.objclient.quest_flags` from the last `Output::QuestFlags` (`bridge/mod.rs take_outputs`); ClientFn 13 (object 561) was fatal 0x1F every frame after the Ancients fell.

## Open
- `baal_falls_and_the_game_is_finished`: Baal's Decrepify (Skill1, mode 10 = S3) never ends: the generic S3 mode has no schedule; Baal (543) uses per-class mode records (units.md §4.5, `0x006E22D0`..) that are not specified. Throne AI then never thinks again, so waves stop. PC1 item queued (REC-1563 reserved). Needs the per-class record spec, then an S3 end for class 543.
- Ledger rows: not written (ledger.py / task format were not reachable from this branch).

## Repro
`D2_GAME_DIR=/home/user/game cargo nextest run -p d2-client --release --test play_act5 --run-ignored only --no-fail-fast`
(excel view first: `cargo run -q --release -p data-tool -- excel-dir $D2_GAME_DIR/extracted/patch_d2/data/global/excel $D2_GAME_DIR`). d2-sim test build needs `--profile dev` (release rustc SIGSEGV).
