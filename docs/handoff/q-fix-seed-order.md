# Handoff: q-fix-seed-order — `claude/q-fix-seed-order`

> Not yet folded into `docs/HANDOFF.md` / `docs/PLAN.md`. The coordinator folds it.

Cloud session, 2026-10-09 (REC block 1120–1129; REC-1120 used). Base:
`claude/specs-staging-7` at `0a297048`. Oracle: 1.14d `Game.exe` under
Wine (`tools/cloud-game/`), install from the private repo.

## Done

### 1. Blood Moor population from frame 5 (`combat-pop-blood-moor`)

Cause (measured): **d2rs had no Act I path floor** (`drlg/outdoor.md`
§7.5.3, a TODO in `outdoor/rooms.rs`). On a path cell 1.14d's room floor
is `(s << 8) | 0x82`, which fails the substitution fixed test
(`outdoor-tilesub.md` §4.3: no bit of 0x3F0FF00). Without it d2rs pasted
Trees / scatter groups where 1.14d retried, so three of the nine Blood
Moor rooms built at the warp took 1, 1 and 131 fewer room-seed draws
before their active-room seed step (`rooms.md` §5 r4). Their active-room
seeds differed, so the room population (ambient step, pack placement,
classes) differed, then every later unit and the game seed.

How it was found: the rng channel showed equal game-seed states but
different consumers at frame 5; the second room's ambient-spawn seed
differed (`{1053638713, 666}` vs `{284688285, 666}`); a full-inline
1.14d recording (no `--skip-inline`) gave each room seed's chain from its
reset; the chains agreed in sites up to a substitution (x, y) pair;
probes of `0x0066FCF0` (fixed test: x, y on the stack, group record in
EAX) showed 1.14d's group at (3, 1) failing where d2rs passed; the room's
grids (`0x00680A70`: EBX = room 9×9 grid record, then the 11×11 path
grid record) showed the path through that cell.

Fix: `d2-sim` `drlg/outdoor/path_floor.rs` (new): the per-room path grid,
the segment draw `0x0067C8E0`, the neighbour mask and style table
(`drlg/outdoor-path-floor.tsv`); called from `room_grids` for Act I
before the substitutions.

The segment rule was **measured**, not guessed: 6 probe runs (levels 2
and 3; `-seed` 1234, 99, 5, 777, 4242), the path grid read before and
after each call, 1,437 segments, every one equal to the rule written in
`outdoor.md` §7.5.3 (including 32 with a negative minor direction). Not
measured: a tie |dx| = |dy| (y-major chosen, `PROVISIONAL`, REC-1120).

### 2. RNG tool: false divergences at a monster's creation (`q-fix-tool-rng-creation-draws`)

- `rng_owners.py` rule 7 (`rng-trace.md` §2 r7): a new monster's seed is
  set (hint `1:0`, read before the GUID is written) and stepped once
  inline (`0x00573A8E`) before its first hinted helper draw. The forward
  pass started the unit's chain at that helper, so the old rule 6 counted
  the unit as "reached the next tick" and the backward pass never took
  the inline step. A chain that starts at a hinted draw of an owner with
  no known value is no longer "anchored". Selftest case added.
- `rng_diff.py` (`rng-trace.md` §5 r3): against a 1.14d `roll(n)`,
  n ≥ 1, a d2rs `roll_range(min, n)` (1.14d adds `min` inline,
  `0x00573F8F`) compares as `roll(n)` with `ret − min`, and a d2rs
  `step` whose `lo'` the caller takes mod n (the AI chance `0x005F05A6`
  = `roll(100)`, d2rs `monsters/ai/mod.rs` `chance`) as `roll(n)` with
  `ret mod n`. Selftest cases added (both sides, wrong ret, wrong n).

### 3. Unit seed order at game start (`q-fix-real-unit-seed-order`)

The row's cause (player seed draw at the load) was fixed by the earlier
session (`q-fix-real-unit-seed-order.md`, merged). With the tool fixes
the town arrival's rng check now matches draw for draw.

## Checks: first divergent frame, before → after

| Check | Channel | Before | After |
|---|---|---|---|
| `combat-pop-blood-moor` | state | frame 5 (game seed, then monsters 1:8, 1:9 class 63 positions, 1:10 class 19 vs 5, Quill Rats 1:17, 1:18 missing) | no difference in 400 frames (PARTIAL: only the documented `own` / client gaps) |
| `combat-pop-blood-moor` | rng | frame 2 (false: units 1:3, 1:4, 1:6, 1:7; roll vs roll_range of 1:1), then frame 5 (real) | MATCH (the 1.14d recording reaches frame 132 in its 900 s) |
| `rng-town-arrival-ama` | rng | frame 2 (the same false divergences) | MATCH (40 frames) |
| `a1-warp-den-ama` | state, rng | not run before | state no difference (PARTIAL), rng MATCH |
| `combat-pop-cold-plains` | state | frame 4 (player arrival x 5168 vs 5183, q-diff-combat-a1) | frame 7: monster 1:28 (class 58, Fallen Shaman) unit seed |
| `combat-pop-cold-plains` | rng | not run before | frame 7 (false: AI chance `roll(100)` vs d2rs `step`, fixed in `rng_diff`) → frame 7, unit 1:28, draw #3 missing (real, below) |
| `warp-cold-plains-ama` | state | not run before | frame 13: the same monster 1:28 seed |
| `combat-pop-stony-field` | state | frame 4: object 2:18 class 37 vs 119 | unchanged (object creation order, objects area) |

d2rs binaries for the rows after the Blood Moor ones: this branch before
the staging merge `1a65f5e9` (the merge brought no DRLG / AI change).

The Fallen think "one unit-seed step short" seen by q-diff-skills-2 on
this path does not show here: unit seeds are equal on all 400 frames
(the monsters of this check do think and fight). Re-check
`variant blood-moor-empty` on its own branch if it still differs.

## Open

- **Monster AI, not this area** (for q-fix-monster-ai): Cold Plains
  frame 7, Fallen Shaman 1:28 (class 58): 1.14d's think draws four
  inline unit-seed steps (`0x005F1516`, `0x005F169A`, `0x005F16E1`,
  `0x005DF7DB`), d2rs three (`ai/mod.rs:412` ×2, `ai/tactics.rs:321`):
  the step at `0x005DF7DB` is missing. Then its walk is mirrored (xf / yf
  swapped from frame 8). Likely the same "one unit-seed step short per
  think" q-diff-skills-2 saw on Fallen. Repro: `scenario_diff.py
  traces/checks/combat-pop-cold-plains.check --channels rng`.

- REC-1120 PROVISIONAL: a diagonal path segment (|dx| = |dy|) is drawn
  y-major. Settle with the scratch probe below on a seed whose path has a
  diagonal (any 1.14d run where a jitter vertex pair is diagonal).
- Not run (disk): the whole d2-client real-data set; run were
  `app_unit_seed_order`, `app_level_border`, `app_act_travel`,
  `app_client_drlg`, `app_waypoint_warp` (results below), and the full
  real-data gate without the client.
- Pre-existing, not from this branch: `d2-server` real-data test
  `game_town_run::town_run_moves_at_the_run_velocity` fails on staging
  too (run ends at (4899, 5634), want (4901, 5634)).
- d2rs keeps 12 active rooms at frame 4 of the warp check where 1.14d
  has 10 (two Rogue Encampment rooms more; ambient steps on `other`
  seeds, not compared, no unit or game-seed effect in 400 frames). Room
  activation / removal area (`drlg/rooms.md` §4), not changed here.
- The other Act I outdoor checks (`combat-pop-cold-plains`,
  `combat-pop-stony-field`, `warp-cold-plains-ama`, `a1-warp-*`) build
  Act I rooms too and change with this fix: see "Suite" below.

## Repro

```sh
# setup: tools/cloud-game/README.md; export D2_GAME_DIR=$HOME/game
python3 tools/scenario-diff/scenario_diff.py traces/checks/combat-pop-blood-moor.check --channels state,rng
python3 tools/scenario-diff/scenario_diff.py traces/checks/rng-town-arrival-ama.check
cargo test -p d2-sim --lib path_floor
python3 tools/trace-recorder/rng_owners.py --selftest; python3 tools/trace-recorder/rng_diff.py --selftest
```

Probe (scratch, not committed; record_rng.py with extra breakpoints):
at `0x00680A70` read EBX → two 20-byte grid records (cells, row offsets,
w, h, flag): the room's 9×9 grid and the 11×11 path grid; at each
`0x0067C8E0` (EDX = vertex record: x, y, …, next at +0x10) read the path
grid before and at the return; `0x0066FCF0` takes (x, y) on the stack and
the group record (24 bytes) in EAX, returns 1 on pass.
