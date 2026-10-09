# q-fix-act5-play (REC-1170)

## Done
- **q-fix-p3-quest-superunique-spawn:** `HostQuests::preset_superunique_spawn`
  (`d2-sim` `wiring/economy/quest_host.rs`) now runs `0x0054E600` through the
  lent monster world (`MonsterWorld::spawn_preset` ->
  `population::preset::preset_spawn`, class = monstats rows + superunique):
  placement, superunique init, hcIdx minions (Nihlathak's 10-minion group),
  quest links. The bare-allocate path stays only for a game without a lent world.
- **q-fix-p6-ancients-link:** the by-class chain-35 link for classes 540-542 in
  `d2-server` `wired/quest_events.rs` is gone; hcIdx 43-45 get chain 35 at
  creation (`umods.rs superunique_quest`), reaching the quest control as a
  `Link` event.
- `traces/playthrough/act5.play` `nihlathak-killed`: Nihlathak now has his group
  and a missile started at the sweep's end position dies at birth (blocked
  spot); the milestone moves him to open ground (12705, 5032) and fires from
  (12705, 5040).

## Results (headless playthrough, `D2_GAME_DIR=/home/user/game`)
- act5.play: with the changes, before the milestone edit 12/13 (first blocker
  nihlathak-killed); after 13/13. The pre-change count was not measured.
- milestones-a3-baal.play: 4/4.

## Open
- No unit test yet for "Ancient from the statue event has chain 35 right after
  creation, event 8 runs once" (queue row's test); needs a fixture with a lent
  world.
- The Ancients are `Stacks`-checked by `superunique_placed`; a statue reset and
  respawn was not exercised.
- `cargo nextest` is not installed here; `cargo test -p d2-sim -p d2-server` passed.

## Repro
`python3 tools/playthrough/playthrough.py traces/playthrough/act5.play --build`
