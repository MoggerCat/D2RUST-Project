# Handoff: PC 2 spec worker — quest fix-ups (`claude/spec-quests-fixups`)

Spec session, 2026-10-07, from `claude/local-pc2-integration`. Owns every
quest file (`specs/world/quests.md`, `quests-act1*.md`, `quests-act2*.md`,
`quests-act3*.md`, `quests-act4*.md`, `quests-act5*.md`, `quests.tsv`,
`quest-messages.tsv`). Evidence: the 1.14d `Game.exe` (exports,
`tools/ghidra/disasm.py`, image bytes), live `objects.txt`.

## Answered

| Id | Spec § | Answer |
|---|---|---|
| objects → quests (WW-6 rows) | `quests.md` §9.6, Randomness, OQ15 | inits 7, 9, 61 and operate 33 confirmed and linked to `quests-act1-rest.md` §9 items 8–11 (33: a null object would fault); init 46 `0x005506D0` (trapped-soul cluster spawner, control-seed draws, order stated), init 59 `0x0054FE10` and operate 43 `0x00584D00` (Duriel / guild portal warp, returns 0) written in full |
| QE-7 placement | `quests-act5-2.md` OQ8 | the 15 `0x00732FF8` rows are already right after the header (39dabf1); values equal the image; order only matters within a table (`QuestTables::messages_for` filters, keeps file order): no move, no CODE-TABLE commit |
| I-2 (`0x006416D0` seam) | `quests-act2.md` §10 Tyrael row | `0x0059DF50` exactly: players without state 7 via `0x005538D0` / `0x0059DF30`, `0x006416D0(player, Tyrael)` < 12; `0x006416D0` is a distance, the radius test is the caller's |
| act3 OQ2 | `quests-act3.md` OQ2 | already Answered (QC-3a); appended the QD-3 confirmation and the `MonLvl[Ex]` source |

## Still open

## CODE-TABLE CHANGE commits

None yet.

## Cross-file requests

## Recording list

None new.
