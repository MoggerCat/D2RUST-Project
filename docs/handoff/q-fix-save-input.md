# q-fix-save-input (2026-10-09, branch claude/q-fix-save-input)

Real-data build (`/home/user/game` from the private repo), staging 0a297048 merged.

## Done
- **q-fix-b45-createxgame-byte18** (REC-1130, PROVISIONAL): C→S 0x67 byte 18
  (class) is 0 for an existing character (`single_player::create_request_for`,
  `Character::Save`). Evidence: the 1.14d packets of combat-potion-midfight
  (frame 1, c2s now matches). The server's join (`loader`) allocates the
  player from the save's class. Unproven: a character picked in the real
  front end (autostart's menu script does give 0).
- **q-fix-b45-save-item-seed** and **q-fix-b45-belt-key-no-send**: already
  right on the merged staging; I changed no code for them. Verified with
  combat-potion-midfight, d2rs-only vs the 1.14d recording of this run:
  `ik`/`ss` of the loaded items at frame 2 match (short sword
  `[4040195123, 666]`), C→S `26 02000000 …` goes out in frame 69, player hp
  2560 → 2640 at frame 70, +80 a frame.
- **q-fix-soak-cursor-reload** (REC-1131): a saved mode-4 item goes back to
  the owner's cursor (`d2s.md` §8.2 r3; `moves::handlers::load_to_cursor`,
  called from `InvDesk::load_entry_in`); it keeps its saved page and cell
  (a stash-page cursor item saved page 4 and reloaded as page 0 before).
  This was also the "played" round-trip item difference the coordinator
  reported (item entry bytes 8–9). Unit test
  `a_cursor_item_returns_to_the_cursor`; `random_play_round_trips` is no
  longer a known-bug repro.

## Checks
- `python3 tools/soak/soak.py roundtrip`: 0 differences at all 14
  checkpoints, still and played.
- `target/release/d2-client soak --replay tools/soak/repro/roundtrip-item-place.log --steps 500 --keep-going --save-dir DIR` and
  `…roundtrip-item-lost.log --steps 400 --keep-going`: 0 findings.

## Open
- combat-potion-midfight still DIVERGES: packets, frame 2, s2c: 1.14d sends
  0x9D (ItemActionOwned, belt potion) where d2rs sends 0x23 in the join —
  the belt-potion mode after load, owned by q-fix-items-shop. State: first
  difference at frame 5, the Blood Moor warp population
  (q-fix-b-bloodmoor-warp-population).
- q-fix-soak-belt-model (not mine) is untouched.

## Repro
```
export D2_GAME_DIR=/home/user/game
python3 tools/scenario-diff/scenario_diff.py traces/checks/combat-potion-midfight.check --work DIR
python3 tools/soak/soak.py roundtrip
```
(the original-game run needs `sh tools/cloud-setup.sh` and
`tools/cloud-game/setup_winpy.sh`).
