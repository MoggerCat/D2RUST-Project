# Handoff: q-fix-client-crash (2026-10-09)

Branch `claude/q-fix-client-crash`. Area: client tracks and local
movement (`bridge::motion`, `bridge::predict`).

## Done

1. **Client crash `geom.rs:197`** (`direction_vector` negative `tan`
   index from `MonsterMotion::frame`, Cold Plains, autoplay seed 1234).
   Reproduced with the dropped autoplay bot (`git show
   2260f437:tools/autoplay/{autoplay,acts,nav}.py`, `--act 1 --plan probe
   --seed 1234 --keep-going`, with `bridge::click::can_act` letting mode 7
   through locally to get past attack-freeze; not committed). Input at the
   panic: monsters 44 / 45 in WL with S→C 0x68 code 0 (walk to unit
   0:1, the local player); the goal was the player's **model** cell
   (4873, 4228), its town placement, while the player stood at
   (5164, 4564) on the walk prediction. That is more than 258 sub-tiles,
   so `127 × l` wrapped. 1.14d's path to the unit (`0x00480780`,
   `client/model.md` §19 r4 code 0x00 / 0x18) aims at the player's
   client path position, which is the walk prediction
   (`seams/movement-prediction.md` §2.9 r1–r2). Fix: `motion::target_cell`
   reads `ClientWorld::local_cell()` for the local player (the
   PlayerWalk set runs before MonsterWalk, so it is this frame's value).
   No clamp. Regression test
   `bridge::motion::tests::a_monster_walks_to_the_local_players_predicted_position`
   (panics at `geom.rs:197` without the fix). Re-run of the bot on the
   fix: Cold Plains, its waypoint, Stony Field and Burial Grounds are
   reached (12/14). The two misses are waypoint trips that the bot
   explores for inside Burial Grounds, which has no waypoint (bot
   navigation, not a game finding).
2. **Walk re-target while walking**: already right on staging
   (`Predict::path_step` re-targets from the precise position,
   q-scenes-compare item 13). New check
   `traces/checks/walk-click-walk-town-ama.check` (clicks at frames 10,
   18 and 30; the second and third re-target a running walk, the third
   reverses it). Against 1.14d under Wine (`scenario_diff.py`, state
   channel): the player (type 0) is equal on every field in all 70
   frames, including the clicked cells (4880, 4223), (4879, 4238) and
   (4878, 4217) and the stop. The check still reports DIVERGED because
   of Warriv (monster 1:7, class 155) from frame 24 (NPC walk
   position, owned by q-fix-real-unit-seed-order). The soak "player
   position desync" was already withdrawn as a tool error
   (`q-tool-soak.md`).

## Open

- Nothing in this area. Warriv's walk position (the check's only
  difference) belongs to the act1-town / seed-order session.

## Repro

```sh
cargo nextest run -p d2-client -E 'test(a_monster_walks_to_the_local_players_predicted_position)'
D2_GAME_DIR=$HOME/game python3 tools/scenario-diff/scenario_diff.py \
    traces/checks/walk-click-walk-town-ama.check   # needs tools/cloud-game setup
```
