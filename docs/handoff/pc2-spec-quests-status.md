# Handoff: PC 2 spec worker — quest-log status meanings (`claude/spec-quests-status`)

Spec session, 2026-10-07, from `claude/local-pc2-integration`. Owns every
quest file. New file: `specs/world/quests-status.md`. Evidence: the 1.14d
`Game.exe` (exports, `tools/ghidra/disasm.py`, image bytes dumped with
scripts in `C:\Users\zffit\Desktop\D2test\scratch-quests-status\`), the
1.14d English `.tbl` files (extracted to the same scratch folder).

## Answered

| Id | Spec § | Answer |
|---|---|---|
| `quests.md` OQ1 | `quests-status.md` §1–§7 | client 0x52 `0x0045CC00` → `0x004A40D0` copies the list to `[0x007BF356]`; row build `0x004A1950` combines it with the client's P (0x28) / G (0x29) records; entry table `0x00723F30`; per-quest 32-word status tables (title, completed speech, pending step, 14 × (text, replay speech)); icon states 0–3 in `0x004A34F0` |
| act2 OQ1 | `quests-status.md` §8 | Act II tables; Seven Tombs rows 5–7 and tomb symbol |
| act3 OQ1 | `quests-status.md` §9 | Act III tables; q 19 / q 21 exceptions |
| act4 OQ1 | `quests-status.md` §10 | Act IV tables; Fallen Angel (no pending row), Hell's Forge (row 10 with completed icon) |
| act5 OQ1, act5-2 OQ1 | `quests-status.md` §11 | Act V tables; Siege, Rescue counter, Prison of Ice, Betrayal |
| relay (d2s worker): intro field-A setter callers | `quests-act1.md` §10.3 item 4 | five calls confirmed with their class jump tables and message ids; `0x0058E990`'s chain-31 state := 1 side effect confirmed (`0x0058E9E9`–`0x0058E9F5`); already stated in `quests-act5-2.md` §9 |

## Still open

| Id | Why |
|---|---|
| `quests-status.md` OQ1 | Needs recording (quest-log open per state) |
| (unchanged from `pc2-spec-quests-fixups.md`) | the recording / other-spec items listed there |

## CODE-TABLE CHANGE commits

None (no TSV change needed).

## Cross-file requests

- to PC 1 (`specs/client/bridge-dispatch.tsv` row 0x52, spec `TBD`):
  0x52 QuestLogInfo client handler `0x0045CC00` → `0x004A40D0` copies the
  42 bytes to `[0x007BF355]` (list[i] → `[0x007BF356 + i]`), clears
  `[0x007BF2B0]`, and when `0x00483350` = 0 reloads the quest-log cels
  (latch 2) and rebuilds the shown tab (`0x004A3220(tab, 1)`); model state
  none, output = a quest-log refresh. Meaning of the bytes:
  `world/quests-status.md` §1, §4. Change: set the spec column to
  `specs/world/quests-status.md` (or a client msg spec that cites it).
- to PC 1 (`specs/client/msg-ui.md` §1 r6/r7): the status byte the tail
  writes is read by `world/quests-status.md` §4; the 0x50 counters
  (`0x004A28A0`: `[0x007BF2A4]`, `[0x007BF2A8]`, `[0x007BF2AC]` from u16@3,
  @5, @7 when u16@1 = 1) feed §4 rule 7 and §5 rule 6. Change: add a
  "Related" link; no rule change.
- to PC 2 ui-panels (`specs/ui/panels.md` open question on the quest log
  panel): the quest-log draw `0x004A34F0`, tab build `0x004A3220`, cels
  `0x004A23D0` and replay `0x004A27D0` are described in
  `world/quests-status.md` §3, §5; a quest-log panel spec can cite them.

## Recording list

- `world/quests-status.md` OQ1: open the quest log in states of one quest
  (e.g. Den of Evil: started; D = 3 monsters left; just completed;
  completed in an earlier game) and record the screen plus packets: drawn
  text, icon frame, and the C→S 0x58 sent after the completion animation.
