# q-fix-a1-den-wp: Act I Den of Evil and Cold Plains waypoint

Branch `claude/q-fix-a1-den-wp` (merged with staging at 7ff68608).
Playthrough `traces/playthrough/act1.play --build`: reached 14/17, 5 consecutive
(was 12/17, 3). First blocker is still `den-of-evil-done`: the headless click
does not pick Akara (it walks), so quest bit 1.0 is never set. That is the
tool's limit (playthrough.md OQ 3), not the Den code; q-fix-quest-load
(REC-1686) delivers talk messages with `--send`.

## Done (Den clear side; unit-tested, not verified end to end against 1.14d)
- `attach_quest_chain` (`wiring/worldgen/init_units.rs`) was never implemented:
  level-8 monsters got no chain-1 link, so event 8 never ran. Now reads the
  level row's `Quest` (quests.md §4.6).
- `count_kill` was never called: new `MonsterWorld::count_death`, called from the
  client's `monster_drop::death_start` (population.md §13 item 3).
- `QuestWorld::den_region` returned (0,0,0,0): `HostQuests::den_region` now reads
  level 8's region (`MonsterWorld::den_counts`) and the populated-room count
  (`Drlg::populated_room_count_if_present`; an absent level has no rooms).
- Tests: `death_counts_in_the_region_unless_flag_2_or_aligned`,
  `creation_links_the_level_quest_chain` (`wiring/worldgen/tests/init.rs`).
- Not run: `cargo nextest`, `tools/coverage.py --check`, `tools/spec_index.py --check`
  on the final tree (only `cargo test -p d2-sim` on the init tests, clippy clean).
- Open: the server's own `Pending` (d2-server) does not call `count_death`; only the
  client's death start does.

## Cold Plains waypoint (+15, +5): resolved on staging by another session
q-fix-d4-placement reports (after merging staging at 0b03e942) that `warp-cold-plains-ama` arrival equals 1.14d, (5168, 4658), frames 1-12; I made no change for it. My notes from before that follow.
`traces/checks/warp-cold-plains-ama.check` with `--d2rs-only`: ours waypoint
(5184, 4664), player (5183, 4663); 1.14d (5169, 4659) = exactly 3 tiles x, 1 tile y.
Hypothesis (unconfirmed): the waypoint-room tile substitution (outdoor-tilesub.md
§4.2, `Trials` -1: one group roll + 36 shuffle pairs on the room seed) picks a
different spot. Next: run the check with `--channels rng` under Wine
(`tools/cloud-game/README.md` Setup) and compare the room's draws. Not started
on 1.14d. q-fix-d4-placement was told this is mine; I did not fix it.

## Repro
```
export D2_GAME_DIR=/home/user/game
python3 tools/playthrough/playthrough.py traces/playthrough/act1.play --build
cargo test -p d2-sim --lib wiring::worldgen::tests::init
python3 tools/scenario-diff/scenario_diff.py traces/checks/warp-cold-plains-ama.check --work /tmp/cp --d2rs-only
```
