# q-tool-soak: soak (seeded random input) and save/load round trip

Branch `claude/q-tool-soak` (from `claude/specs-staging-7`). d2rs only:
every finding is a d2rs bug or a d2rs-own gap, never a fidelity claim
(CLAUDE.md rule 10). Spec: `specs/tools/soak.md`.

## What was built

- `d2-client soak` (`crates/d2-client/src/app/soak/`): the play client
  headless (`play::add_live_client` on `MinimalPlugins`, the play smoke
  path) on the user's install, about 100 frames/s on a stepping clock.
  Seeded random input through the client only: clicks (UI panels,
  ground), clicks on units in view, right clicks, key actions (belt 1–4,
  skill slots 1–8, inventory / character / skill tree / quests / automap
  / minimap / hireling / message log, Esc, Alt, run, swap), item moves
  (C→S 0x16–0x1A, 0x20, 0x23, 0x26), portal scrolls and books, waypoints
  (interact, C→S 0x49). An item kit is poked onto the ground at the start.
  Checks after every frame (spec §3): panics, hangs (no tick outside the
  Esc pause), GUID duplicates / past the counter / reuse, negative
  vitals, room membership and bounds, an item in two places, the model's
  receive log, client/server desync (player GUID, position, life, its
  items) held for 50 checks.
- Input log `soak-log 1`: every generated input as a concrete line;
  `--replay LOG` reruns it exactly (checked: same report twice).
- `--roundtrip-at S,...` (and the `roundtrip` log action): Esc (pause),
  read the live state, Save and Exit (C→S 0x69, the server's leave
  writes the `.d2s`), read it, join a new game from it, compare the live
  state field by field, Save and Exit again, compare the two files field
  by field (save time, checksum and the load-cleared 0x2000 aside); the
  run goes on in a third game from the second file. A Save and Exit the
  random input clicks (Esc menu) is followed by a reload too.
- `tools/soak/soak.py`: `campaign` (all starts: new sorceress and
  barbarian, the five towns from `d2s-tool new` saves, Blood Moor, Cold
  Plains, Rocky Waste, Spider Forest, Outer Steppes, Bloody Foothills by
  the `warp` start poke; one child per run with a wall-time limit),
  `reduce` (cut after the finding, then ddmin over the log's actions),
  `saves` (the act saves at `target/soak/saves`, which the committed
  repro logs name), `selftest`.
- `crates/d2-client/tests/save_roundtrip.rs` (`#[ignore]`, real data):
  every class's new character round-trips twice (passes); every act town
  after the act change round-trips (passes); random play with two round
  trips (known-bug repro: q-fix-soak-cursor-reload); a new character's
  belt is in the model (known-bug repro: q-fix-soak-belt-model).

## Run it

```sh
cargo build --release -p d2-client -p d2s-tool
export D2_GAME_DIR=~/game
target/release/d2-client soak --new sorceress --seed 7 --steps 3000 --keep-going --log-out run.log
target/release/d2-client soak --replay run.log --steps 3000 --keep-going
python3 tools/soak/soak.py campaign --minutes 60 --out target/soak/camp --no-reduce
python3 tools/soak/soak.py reduce target/soak/camp/logs/town1-3.log --sig room:outside:Item --out target/soak/camp
```

## Round trips at every start and checkpoint

`python3 tools/checkpoints/make.py && python3 tools/soak/soak.py roundtrip`
(2026-10-09, 13 starts + 14 checkpoints): with no input every one
round-trips twice with no difference; after random play 6 of the 14
checkpoints differ, all from q-fix-soak-cursor-reload (an item on the
cursor at Save and Exit). The belt gap (q-fix-soak-belt-model) shows on
every checkpoint's belt too.

## Findings (rows in `build-queue.tsv`)

| Row | What | Repro |
|---|---|---|
| q-fix-soak-belt-model | a new character's 4 belt potions are on the server, never in the model | `tools/soak/repro/desync-belt-model.log`, no input |
| q-fix-soak-cursor-reload | an item saved on the cursor is reloaded into the inventory (`d2s.md` §8: back to the cursor), or lost when its old cell is taken | `tools/soak/repro/roundtrip-item-place.log`, 2 lines; `roundtrip-item-lost.log` (a1-andariel checkpoint), 4 lines |
| q-fix-soak-drop-room | a dropped item lies 2 sub-tiles outside the room it is linked to | `tools/soak/repro/drop-outside-room.log`, 9 lines |
| q-fix-soak-walk-pick-desync | a pick-up during a walk stops the server's player; the client walks on, 15-20 sub-tiles off, never corrected | `tools/soak/repro/desync-walk-then-pick.log` (a1-andariel checkpoint), 2 lines |
| q-fix-soak-static-leave-room | a static unit leaving its room keeps it (pick-up; the old act's ground items after an act change, whose room ids the new act reuses) | `tools/soak/repro/static-room-after-pickup.log`, 1 line |

Withdrawn (tool errors, fixed in the tool): the player-position desync
(the check read the model's placement cell, not the walk prediction the
client moves the local player by, `seams/movement-prediction.md` §2.9
r2); the hang during the Esc menu (the single-player pause); dropped
S→C 0x6D / 0x67 / 0x6B (a unit message for a unit the model lacks at
receive: 1.14d drops it too, `client/model.md` §4 r6).

Not reproduced: `room:list-vs-path:Object` (Object 69 mode 2, a4-hellforge
seed 200 of campaign 3) came from a run on the binary before the
staging merge at 870a539f; its log does not replay it on the current
build. Rerun the campaign to see whether it comes back.

## Owners (coordinator, 2026-10-09)

| Row | Owner session |
|---|---|
| q-fix-soak-belt-model, q-fix-soak-cursor-reload | items: session_01GXSY6R3mUS5ig6wuAQUtJP |
| q-fix-soak-walk-pick-desync | client model / track: session_01BQSDGiWHKAbHXoWk7HThvM |
| q-fix-soak-static-leave-room, q-fix-soak-drop-room | progression / act travel: session_01UWEn2ZtdtLRHPZRdytF7Gc (the act-change half; the pick-up and drop halves touch items) |

Rows are filed; the repros were not sent to the owners (the sessions
were paused at wrap-up). Route them with `tools/coord/route.py` on resume.

## State at wrap-up and how to go on

- Campaigns run: 3 (about 250 runs of 2,500 frames, about 600k frames;
  campaign 3 with a round trip every 700 frames, 79 runs) over the 13
  starts and 14 checkpoints. Logs were under `target/soak/` (not
  committed).
- Unreduced at wrap-up: `room:outside:Object` (a4-diablo checkpoint,
  campaign 3 seed 202: Object 59 at (5163, 5067), its room RoomId(12)
  at sub-tiles (20040, 5040)), likely the static-room family of
  q-fix-soak-static-leave-room; and `roundtrip:live.items.count` on
  a2-duriel seed 201 (an item lost on reload), likely
  q-fix-soak-cursor-reload. Rerun to confirm:
  `python3 tools/soak/soak.py run --start ck-a4-diablo --seed 202 --steps 2500 --out target/soak/x`
  (after `python3 tools/checkpoints/make.py`), then `soak.py reduce` on
  its log with `--sig room:outside:Object`.
- Rerun everything: `cargo build --release -p d2-client -p d2s-tool`,
  `python3 tools/checkpoints/make.py`, `python3 tools/soak/soak.py saves`,
  `python3 tools/soak/soak.py roundtrip`, and
  `python3 tools/soak/soak.py campaign --minutes N --out target/soak/campN --no-reduce --roundtrip-every 700`.
  Reduce a new signature with `soak.py reduce LOG --sig SIG --out DIR`
  (2–3 in parallel on a 4-core box beside the campaign). Rebuild the
  binary only between campaigns: a campaign's logs replay on the build
  that made them.
