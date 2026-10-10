# rc-level-pop-2 hand-back
Branch `claude/rc-level-pop-2` (`claude/specs-staging-7` + `claude/integ-r17`). REC ids: none used.

## Checks (gen-lvl, `traces/checks/gen`, 136 checks re-run)
| | rc-rng-level-pop's 92 rows EQUAL | all 136: state frames all equal | DIVERGED |
|---|---|---|---|
| before (rc-rng-level-pop hand-back) | 79 / 92 | - | 13 of its rows |
| after | 83 / 92 | 125 / 136 (rng MATCH 126) | 11 |

All four rows of the largest group are now equal: lvl 31, 33, 68, 70 (frame 21 monster
x/y off by 1–3 with rng equal). Rows are in `docs/handoff/ledger/rc-level-pop-2.tsv`.

## Root cause fixed
1.14d's alignment setter `0x005543B0` passes old = 4 to its tail `0x00554340` when the
state-105 list is new. A monster's first alignment set therefore always reaches the path
reset `0x00649CA0`, which recomputes the collision pattern from the stored size and
restamps it. That undoes the wraith pattern 5 set at path allocation (§2.4), so every
placed wraith ends up with pattern 1 and its 0x1000 marker on its centre. The marker
then blocks the plus test of the next group member: gen-lvl-31 wraith2 1:32 rejects
y 8156..8158 and lands on 8159. d2rs never ran the tail.
- `path::footprint::reset_pattern` (`0x00649CA0`) and `View::path_reset`.
- `View::set_alignment`: the class/flag gate (classes 351–353, a ≠ 0, or flag bit 31
  clear), old value (stat 172, or 4 for a new list), and the tail's path reset for a monster
  in a room that isn't in mode 0 or 12.
- Specs: `sim/path-placement.md` §5.3 rule 5 (new); `combat/hit.md` gives the body of
  `0x00554340`. Test: `path_reset_gives_a_wraith_its_size_pattern`.
- Private repo `re/exports/names.tsv`: 0x00649CA0, 0x00554340, 0x005543B0.

## Open (11 DIVERGED, first difference after this fix)
- 2 × creation order: lvl 55 (0x573f8f vs `init/create.rs`), lvl 69 (0x573a03). M.
- 2 × population game seed: lvl 104 (extra `room.rs:193`), lvl 106 (missing 0x54ed96). M.
- 2 × combat 0x57b5f7 vs `combat/damage.rs`, player hp: lvl 97 f26, lvl 61 f64 (61 was AI f62). S–M.
- 3 × AI: lvl 94 f43 mode (`ai/mod.rs:412`), lvl 99 f89 player mode / extra AI draw (was f87),
  lvl 110 f23 tx (`ai/mod.rs:412`). M.
- lvl 132 f112: tentacle 562 missing AI draw 0x5ef875 (spawned monsters get no AI). M.
- lvl 4 f21: object class 61 mode 0 vs 2. This was already diverged (quest_link stub,
  q-fix-d11-stony-objects). M.
- Not wired: the tail's region count `0x00547DD0` (+0x2CC, changes only when the
  alignment goes 0 ↔ 1/2 with old ≠ 4). It's in the spec, but no current check reaches it. S.

Notes: `tools/coord/route.py` leaves the touched code and spec files unrouted. The
d2-sim nextest run passed 4769/4769.
