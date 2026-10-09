# q-fix-cold-plains: Cold Plains room count (2026-10-09)

Branch `claude/q-fix-cold-plains` (from staging `389227a`, merged with
`claude/q-realdata-run` `8288be4`). Row `q-fix-cold-plains-rooms`;
finding 1 of `docs/handoff/q-realdata-run.md`. Methods M22, M23, M25.

## Cause

A border substitution's replacement stamped its presets with file −1,
so the first stamp of a preset not yet in the build list drew a
build-list roll (`0x0067438F`). The recording has none:

- Cold Plains: level-seed draws (seed {4014346872, 666}) 1551–1554 were
  the rolls of 15, 12, 14, 13 stamped by the type 1 replacement at
  (3, 1). The type 2 variant roll then landed on draw 1751 instead of
  the recorded 1747 (seq 8644): variant 0 instead of 1 at (6, 6), so the
  two blank pattern cells were not (8, 9) / (9, 9), the special presets
  moved and the build had 97 rooms (62 + 35).
- Blood Moor: the same two extra rolls (12, 13), so 17 build-list draws
  against the recorded 15; its room count happened to hold.

Found draw by draw from the spec's recorded values, not the raw
recording (not in the cloud): each traced `lo'` was mapped to its draw
index on the level seed (a Python step of `sim/rng.md` §2) and compared
with the recorded seqs.

## Change

| Where | What |
|---|---|
| `specs/drlg/outdoor-tilesub.md` §2.3, Randomness | a replacement's stamp makes no build-list roll (recorded); its file is **PROVISIONAL REC-404** (0, build list untouched) |
| `specs/drlg/outdoor.md` Randomness | the same, in the Act I build order |
| `d2-sim` `drlg/outdoor/tilesub.rs` | `sub_replace` stamps with `SUB_STAMP_FILE` (0) instead of −1 |
| `d2-sim` `drlg/outdoor/mutant_tests.rs` | the §2.3 rule model follows the new text |
| `d2-sim` `drlg/outdoor/tests.rs` | `border_substitution_stamps_draw_no_build_list_roll` |
| `docs/HANDOFF.md` | REC-404 / R-SUBFILE-1 in the recording list and the provisional list; a seq check in the local run queue (below) |

No expected value changed.

## Result on the real install

`outdoor_levels_generate_through_the_dispatcher`: Cold Plains 98 = 61
preset + 37 outdoor; grid differences none; substitutions
(1, 0, (3, 1), v 2, lo' 1833932632), (2, 1, (6, 6), v 1, lo' 3559729267),
(3, 8, (3, 6), v 0, lo' 1651351014), (3, 11, (0, 5), v 0, lo'
2564466130), all equal to the recording (draws 1550, 1747, 3307, 3602).
Build-list draws: Blood Moor at seq 2562–2571, 4464, 6027, 6129, 6230,
6331 (all as recorded); Cold Plains at 6897–6904, 9481, 11622, 11752,
11881 as recorded, the last (30) at 12010.

## Open

- REC-404: the file a replacement's stamp gets (only a multi-file piece
  4–7 stamped by a replacement would show it; the room tile files of such
  a cell need a recording or the Ghidra read of `0x0066F520`).
- The spec says preset 30's roll is seq 12009 and the cells start at
  12010; the stride of the three rolls before it puts it at 12010.
  Queued in `docs/HANDOFF.md` §5 as a check of the raw recording.
