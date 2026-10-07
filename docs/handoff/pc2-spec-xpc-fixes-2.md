# PC 2 spec worker: xpc-fixes-2

Branch `claude/spec-xpc-fixes-2` (from `claude/local-pc2-integration`
8c49c1a). Applies two PC 1 requests from `docs/handoff/xpc-to-pc2.md`.

## xpc-to-pc2 lines done

- Line 24 (`world/npc.md` §8.2 reset addresses swapped): fixed in place;
  `0x00570360` = skill reset (`skills/levels.md` §6.5, call `0x0057A242`),
  then `0x00570C80` = stat reset (`combat/vitals.md` §2.1, call
  `0x0057A24B`). Verified with `disasm.py at 0x0057A230`. Commit:
  `15b9899`.
- Line 25 (`items/inventory.md` §7.1 rule 2 and §7.11 `toa`): §7–§11 now
  live in `items/inventory-moves.md` (split of `inventory.md`, commit
  82ef79d, same owner), so the edits are there: §7.1 rule 2 links
  `combat/vitals.md` §4.7 for the experience part and keeps the take-back
  `0x00562F30` (call `0x0057FBE5`) in `inventory.md` §4.9; §7.11 `toa`
  names skill reset `0x00570360` (§6.5, call `0x0055E549`) then stat
  reset `0x00570C80` (§2.1, call `0x0055E552`). `inventory.md` §4.9's
  "corpse spec, not specified here" replaced with the §4.7 link.
  Verified with `disasm.py xref` and `index/calls.tsv`. Commit: `15b9899`.

## Answered

None (factual corrections only).

## Still open

None.

## CODE-TABLE CHANGE commits

None.

## Cross-file requests

None.

## Recording list

None.

## Decisions

- Edited `specs/items/inventory-moves.md` although the task named only
  `inventory.md`: the requested rules (§7.1, §7.11) were moved there
  unchanged by the split, and it is the same owner's file.
