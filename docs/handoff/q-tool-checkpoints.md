# Handoff: q-tool-checkpoints — `claude/q-tool-checkpoints`

> Not yet folded into `docs/HANDOFF.md` / `docs/PLAN.md`. The coordinator folds it.

Cloud session, 2026-10-09 (tools; REC block 940–949: none used). Base:
`claude/specs-staging-7` (merged in before each push). Oracle: 1.14d
`Game.exe` under Wine (`tools/cloud-game/`), install from the private
repo at `9710830`.

## What exists now

| Piece | Where | Check |
|---|---|---|
| Checkpoint definitions, format `checkpoint 1` | `traces/checkpoints/*.checkpoint` (14), spec `specs/tools/checkpoints.md` | `python3 tools/checkpoints/make.py --selftest` |
| Save builder and load check, d2rs and 1.14d | `tools/checkpoints/make.py` (`--verify`, `--orig`, `--record`) | `traces/checkpoints/d2rs-start.tsv`, `traces/checkpoints/orig-start.tsv` |
| `d2s-tool --waypoints lv=LEVEL` | `tools/d2s-tool` | `cargo test -p d2s-tool --test synthetic waypoints` |
| `goto unit` / `goto preset` | spec `specs/tools/poke.md` §1, §6; d2rs `d2_sim::poke::goto_step` (state-dump, play); 1.14d `tools/trace-recorder/poke.py` (`_goto`, `goto_hop`, `free_cell`) | `cargo test -p d2-sim --lib poke`, `cargo test -p test-fixtures --test poke_goto`, `python3 tools/trace-recorder/poke.py --selftest` |
| Playthrough `checkpoint <name>` start, `goto <f> …` step | `tools/playthrough/playthrough.py`, spec `specs/tools/playthrough.md` §1 r5–r6 | `python3 tools/playthrough/playthrough.py --selftest` |
| Act IV forge objective file | `traces/playthrough/act4-forge.play` | d2rs: 4/6 (below) |

The checkpoints, one before each act boss and act-gating step: a1-den,
a1-andariel, a2-radament, a2-summoner, a2-duriel, a3-travincal,
a3-mephisto, a4-izual, a4-hellforge, a4-diablo, a5-anya, a5-nihlathak,
a5-ancients, a5-baal. Sorceress, levels 6–60, quest bits of every
earlier step (`quests.md` §1.9) and the act transitions, waypoints of
the levels passed, gear and belt potions; `start` pokes: `warp` to the
blocker's level, then `goto` to the boss or the step's object. Every id
was checked against the install's `Levels.txt`, `monstats.txt` and
`objects.txt`. Two targets are objects: Anya → dummy 460 (object 558
appears there 25 frames after the dummy's init, `quests-act5.md` §5.6),
Nihlathak → dummy 462 (the quest spawns him, §5.9).

## How `goto` works (spec `poke.md` §6)

A walk, one step per tick: BFS over the DRLG near arrays to the nearest
unseen room of the goal level; hop into the next room with the
placement `0x00554EA0` (exact 0) at that room's free cell (mask 0x1C09)
nearest its centre, read from the room's collision grid; rooms without
a free cell, or where the player does not land, are blocked (this is
what crosses River of Flame's lava). Lands when a unit of the target's
type and class is in the goal level; the result's GUID is the
target's. Two fixes came from the first real run: hopping to the room
centre sent the free-point search back to the same lava island
(→ the free-cell rule), and a room with no free cell as the first hop
of every route stalled the walk (→ the blocked set).

## Results

- d2rs (`make.py --verify --record traces/checkpoints/d2rs-start.tsv`):
  14/14 load (lvl, act, quest bits) and 14/14 start walks land.
- 1.14d under Wine (`make.py --orig --record
  traces/checkpoints/orig-start.tsv`, about 1 h for the 14): 14/14 load
  (`lvl`, `act`; quest bits not read on this side, below) at the same
  town cell as d2rs, and 14/14 start walks land (`warp` and `goto` `ok`).
  The end cell equals d2rs's for 9 checkpoints and is within a few
  sub-tiles for 5 (a2-duriel (22638, 15688) vs (22632, 15710),
  a3-travincal (4506, 1811) vs (4513, 1833), a4-diablo, a5-ancients,
  a5-nihlathak ±3): the walk is a start, not a comparison point
  (`poke.md` §6 r5).
- a4-hellforge on both: `goto preset 107 2:376` lands after 50 steps at
  f69, player (7779, 6133), forge (7781, 6135), Hephasto (7810, 6143);
  only the GUIDs differ (forge 94 d2rs / 96 1.14d).
- `act4-forge.play` (d2rs): river-of-flame, forge-reached,
  hephasto-present, hephasto-killed reached; forge-smashed (no headless
  input to place the soulstone and land the hammer hits,
  `quests-act4.md` §4.6) and forge-done (Cain msg 680: an NPC talk,
  `playthrough.md` Open question 3) are the expected blockers.

## Queued (HANDOFF §5)

- **q-tool-checkpoints (quest log)**: the 1.14d recorder reads no quest
  record (`q`), so the quest bits of each save are checked by eye in
  1.14d (entry in HANDOFF §5).

## How to use it (act sessions)

```sh
python3 tools/checkpoints/make.py a4-hellforge            # target/checkpoints/a4-hellforge.d2s + .start
target/release/d2-client state-dump --save target/checkpoints/a4-hellforge.d2s --seed 1 --ticks 300 \
    --out x.jsonl --poke "5 warp 107" --poke "20 goto preset 107 2:376"
python3 tools/checkpoints/make.py a4-hellforge --orig      # 1.14d, same start (Wine or Windows)
```

In a `.play` file: `checkpoint a4-hellforge` instead of `use`, then
`goto <f> unit <class>` / `poke …` steps (`act4-forge.play`).
