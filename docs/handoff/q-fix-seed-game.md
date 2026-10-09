# q-fix-seed-game — hand-off (2026-10-09)

Ledger D1, the half that is not the monster creation draw at frame 2
(q-fix-seed-order owns `monsters/init` and the Blood Moor population).
Method: scenario-diff `rng` + `state` under Wine, `rng_diff.py --owners
game` / `--from 3` to look past the frame-2 creation draw, then the
first differing draw or unit.

## Done (branch `claude/q-fix-seed-game`, commit "Fire Blast frames, Volcano seed word, preset chests 580/581")

1. **Fire Blast flew one frame** (`ass-fire-blast`). The lobbed missile
   (skill 251, `lob`, flag 0x400 "frames from distance") took its
   current frame from `MissilePath::target_distance`, which the real
   host left to `Pending` (0 → floor 1 → 1 frame). Now
   `0x006417F0` from the path position to the path target point
   (`missiles.md` §R2.3 step 19): 8 sub-tiles at v 2304 → 14 frames, as
   recorded (explodes at the target at frame 41).
   `crates/d2-sim/src/wiring/action/missiles.rs`; test
   `wiring/path/frames_tests.rs`.
2. **Volcano scatter seed** (`dru-volcano`). Volcano's srvdo 123 writes
   its `roll(256)` word to the missile's data +0x28 through
   `BodyEffect::MissileData28`; the real skill view dropped it, so
   server-do 28 re-seeded from 0. The view now writes +0x28 / +0x2C to
   the missile store (`skills/bodies-2b.md` §7.18 r6).
   `crates/d2-sim/src/wiring/interaction/skill_use.rs`; test
   `missile_data_words_reach_the_missile_store`.
3. **Preset chests 580 / 581** (`a3-warp-durance-ama`). DS1 preset class
   581 was `NotCovered`; §6 of `world/objects.md` answers it: one
   control-seed step picks the chest by act (Act III not level 83:
   {181, 183}[lo' & 1]), allocated in mode 0. 580 now runs through it at
   every level (the class draw also at level 25, which the old code and
   its test skipped), spark by level. 582 stays with the quest host.
   `crates/d2-sim/src/world/objects.rs`; tests
   `preset_chest_581_picks_by_act_on_one_control_step`,
   `preset_bounds_and_580`.

## Per check: first divergence before → after

| Check | Before | After | Next owner |
|---|---|---|---|
| ass-fire-blast | state f28 (missile 385 gone, 386 early) | state: no difference, 70 frames (PARTIAL = known gaps); rng only the f2/f9 creation draw (`create.rs`) | q-fix-seed-order (creation draw) |
| dru-volcano | state f34 missile 3:1 seed | state: no difference, 70 frames; rng only the creation draw | q-fix-seed-order |
| ass-shadow-master | state f28 game seed | state f48 monster 1:8 (418 Shadow Master) mode 7 vs 1 | skills / pet AI |
| a3-warp-durance-ama | state f21, 1.14d +3 units (chests 181, 183, orb monster) | state f21, 1.14d +1 unit: monster 366 (Compelling Orb's) at (4496, 1811) | act 3 quests (below, Open 4) |
| combat-* (12) | state f5 game seed | unchanged | q-fix-seed-order (Blood Moor population) |
| milestone-izual | state f28 game seed | unchanged | population (Open 2) |
| a3-warp-kurast-sewers-ama | state f21 game seed | unchanged | population (Open 2) |
| milestone-act5-entry | state f26 monster 1:5 (Larzuk) seed | unchanged | act 5 quests (Open 5) |
| milestone-baal-throne, milestone-worldstone-portal | state f50 monster 1:38 seed | unchanged | monster AI (Open 3) |
| a1-warp-tower-cellar-ama | state f45 monster 1:10 seed | unchanged | monster AI + population (Open 6) |

## Open (analysed, not fixed: outside this area or needing PC 1)

1. **combat-* frame 5.** Every game-seed draw through frame 5 matches
   value for value; 1.14d has 2 more because after the third density run
   it allocates 4 units (a pack) where d2rs allocates 2; monster 1:10 is
   class 19 on 1.14d vs 5 on d2rs (the room-seed class pick). Sent to
   q-fix-seed-order.
2. **Population elsewhere.** `milestone-izual` f28 (Plains of Despair):
   the first differing unit is 1:25 class 298 at another spot (pack
   placement), then pack sizes differ. `a3-warp-kurast-sewers-ama` f21:
   1.14d makes one more density draw (`0x54ed96`) at game draw #1091
   (one more try of a coordinate record: rect or tries count).
3. **SuccubusWitch secondary target** (baal throne f50). 1.14d draws 4
   times (step 3.1 finds S, then the `roll(100) < aip8` at `0x5e242e`),
   d2rs 3. S is the player (full-size distance ≈ 32 < 49).
   `AiTargets::secondary_target` (and `forced_target`,
   `good_target_search`, `choose_alternative`) in
   `wiring/action/ai.rs` go to `Pending`; the client's preview
   `LocalSeams::secondary_target` only sees monsters. Needs `0x005DDC30`
   per `ai.md` §5.3 in the sim (scan 6 callback `0x005DCBD0`, hostility
   = `combat::range::hostile`). The monster-AI session
   (session_019dnJCaEadJxjbHBUyvL71q) no longer exists, so this needs a
   new owner.
4. **Compelling Orb monster** (durance f21). Init 60 (`orb_init`) calls
   `QuestWorld::spawn_monster_at_unit` (`0x005B3090`), which the real
   quest host does not implement (default "unhandled"); the spec does
   not give the call's spread / flags (PC 1 item queued).
5. **Larzuk's path nodes** (act5-entry f26). Both sides think at f6 and
   f26 (and the room joins, S→C 0x07, match), but d2rs's Larzuk has no
   map-AI nodes, so it never draws `lo' % 100` at `0x5e70a5`.
   `quests-act5.md` §3.8: the preset spawner stores the map AI
   (`0x00587950` from `0x00545CB3`) and init 71 applies it; in d2rs
   `larzuk_map_ai` has no caller and the host's `apply_map_ai` is a stub.
6. **Pack leader acquires late** (tower cellar f45). The fallen3 leader
   (1:10, umods) thinks at f23 and f45 on both sides (mode-1 idle 22 at
   distance 32). On 1.14d it finds the player at f45 after minion 1:12
   saw it at f43; d2rs does not. `ai.md` §5.2: T comes from the vision
   record (monster data +0x50) +0x24, set by any monster sharing it;
   d2rs's `vision_seen` / `mark_seen` are `Pending` stubs. The record is
   the population coordinate record (`drlg/levels.md` §11: +0x24 "not
   written by this code"); which one each monster gets is unsettled (PC 1
   item queued).

## Repro

```sh
export D2_GAME_DIR=$HOME/game
python3 tools/scenario-diff/scenario_diff.py traces/checks/<check>.check --channels rng,state --work ../work/<check>
python3 tools/trace-recorder/rng_diff.py ../work/<check>/orig.rng.jsonl ../work/<check>/d2rs.rng.jsonl --owners game   # game seed only
python3 tools/trace-recorder/rng_diff.py ../work/<check>/orig.rng.jsonl ../work/<check>/d2rs.rng.jsonl --from 3       # past the creation draw
```

After a d2rs change: the same with `--reuse-orig`.
