# Hand-back — rc-drops-chickens (branch claude/rc-drops-chickens)

No code change pushed: neither item reached a fix I can justify from the 1.14d reading.

## Item 1 — REC-1453 (drops-nor one-frame lag)
Baseline `items-drops-nor-*` (14 checks, orig-cache, this container): **5 MATCH**
(00, 02, 05, 12, 13), 8 DIVERGED, 1 PARTIAL (01). After: unchanged (5).

Pairing each check's S->C 0x69 code 8 with its first 0x9C, both sides
(packets in traces/raw/suite/items-drops-nor-NN/):
- 03, 06, 08, 09, 10, 11, 07: death frames are **equal** (36=36, 60=60,
  72=72 ...). The lag is not a late death. First divergences there are drop
  content/count (extra or missing items, item GUID order), i.e. TC rolls.
- 04: d2rs kills monster 1:21 at frame 40 (bolt 2, poked at 40); 1.14d does
  not (monster stays hp 113, mode 4 -> 1) and kills 1:22 at 42. Monster
  positions equal through 41. d2rs's finder (`units_at`, matches 0x00641CB0
  case table, checked) returns 1:21 on the bolt's first crossed subtile
  (5144,4263); 1.14d does not hit it. Bolt 1 (34) agrees on both sides.
- `combat-kill-fallen` (PC1's case): the fallen now dies at frame 36 on both
  sides with equal positions; first divergence is frame 43 (missile 3:2).
  The earlier "fallen stands still" lag no longer reproduces.
- Not the cause (read 0x005A7C20, 0x005A8730, 0x00554200, 0x00620510,
  0x0064A470, 0x005A6520): kill request is synchronous; filter flags and
  unit size are not mode-dependent; NextHit is empty for firebolt (58).
- Experiment: skipping mode-4 monsters in `units_at` moved 04's first
  divergence 40 -> 43 but regressed 02/03 (suite: 5 -> 4 MATCH), so it is
  wrong; reverted. Open: why 1.14d's bolt 2 passes (5144,4264) — likely the
  collision-mask gate (step 9 `collision_mask`, footprint of size-2 monsters,
  path-placement.md §5) or the bolt's creation-tick crossing list. Needs a
  1.14d probe of missile 2's crossed subtiles at frame 40/41 (PC1 item).

## Item 2 — REC-742 (chickens)
Not started beyond reading. d2-client has no client path model: set-S and
set-C monsters only change mode (`bridge/modes.rs` monster(), `critters.rs`
`request`). A fix needs 0x004804A0/0x00649970 (client path to point), the
per-tick client step and the walk end (stop one sub-tile short, model.md
§5 r6 recorded block), plus the 0xAC set-up. Size: large (new client path
module), and needs a per-tick 1.14d chicken track (record_frames.py --every 1).

## Setup notes
`prepare_saves.sh` needs `$HOME/game`; game was assembled at /home/user/game
(symlinked /root/game). Wine recording of 14 checks takes ~12 min.
