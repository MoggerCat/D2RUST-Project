# Handoff: PC 2 spec worker — quests Act IV / Act V answers (`claude/spec-quests-act4-5`)

Answers to the questions `impl-quests-act4-5` raised (HANDOFF §7 Ninth
set QD-* / QE-*, and the Act IV / V items of
`docs/handoff/impl-quests-act4-5.md` §3). Evidence: 1.14d `Game.exe`
through `re/exports` and `tools/ghidra/disasm.py`; live tables read only.
QD-1, QD-2 and QD-4 are the impl note's Act IV items 1, 2 and 4 (OQ2,
OQ3, OQ9); QE-n are the Act V items in the same order.

## Answered

| Id | Spec § | Answer |
|---|---|---|
| QD-2 (act4 OQ3) | act4 §5.8, Edge case 22, OQ3 | bytes 3–14 of the uncredited 0x50 are unwritten stack of `0x005B4A80`; the client (`0x0045E370` → `0x004B9210`, id 23 → `0x004B924B`) reads only the u16 at 1: d2-sim zeros, traces mask them |
| QD-3 (act4 OQ4) | act4 §4.7, §4.8, Edge cases 16 / 21, OQ4 | `&level` of `0x00559A30` is an out-parameter written before use (`0x00559AF8`); item level = dropping unit's stat 12 (player / monster) or `levels.txt` MonLvl[Ex] of its room's level (`0x0061DCA0`); forge drops 27/52/77 classic, 27/57/85 expansion |
| QD-4 (act4 OQ9, I-3) | act4 OQ9 | `0x00538680` is `quests-act1-rest.md` §5; Act IV call classic-only at `0x005B4D77`, step 4 |
| QD-5a (act4 OQ5) | act4 §8, OQ5 | `0x005A4850(…, 22, 1)` = unique mark + umod 22 `questcomplete` appended, no init fn (`0x0073C008`[22] null) |
| QD-5b (act4 OQ6) | act4 §3.6, Edge case 19, OQ6 | `0x005E7350`: no one talking (`0x00572DC0` = 0) → mode 0 at (0, 0) via `0x005DDFC0(game, u, 0, 0, 0)`, return 1; the AI then sets mode 0 again (`0x005E7521`); path only when `0x005DDF20` finds no interacting player |
| QD-5c (act4 OQ7) | act4 §5.4, Edge case 20, OQ7 | `0x0061AED0(room, 0)` sets DRLG room flag 0x400000 (`0x0061BAC0`) = blocks room removal (`drlg/rooms.md` §8); dummy init 59 `0x0054FE10`: event 7 at f + 27, event 1 at f + FrameCnt1 + 1 |
| QD-5d (act4 OQ8) | act4 §5.4, OQ8 | +0xAE0 = hcIdx → row map from `0x006552E0` (`data/loading.md` §8) |
| QD-6 | act4 §1.4 | FrameCnt1 / FrameCnt3 read per call from the object's objects record `0x00640E90(class)` +0xDC / +0xE4, >> 8 |
| QD-7 | act4 §8 | chain-23 links are the generic `0x005436B0(game, unit, 23)` made by creation code: `0x005B1E36` (BaseId 243, class ≠ 705), `0x005A466D` (`0x005A4440`), `0x005A4C7D` (`0x005A49B0`) for hcIdx 36–38 |
| QD-8a | act4 §5.6 | confirmed: three tries (spread −1, 5, 10) at the same start-point x, y, room; a missing start-point object returns 0 (retried) |
| QD-8b | act4 §3.7 | confirmed: the status test is inside the state-2 / 25.0 test (`0x005B4140`) |
| QD-8c | act4 §6.2 | confirmed: second branch true only with 26.13 and 26.0 both clear (`0x005B6A46`, `0x005B6A53`) |
| I-2 (`0x006416D0`) | act4 §3.6 | it is the size-adjusted distance of two units (`missiles.md` §R9.5 item 4), no radius: the Act IV / V `distance_between` reading is right |

## Still open

| Id | Why |
|---|---|
| QD-1 (act4 OQ2) | Needs recording: the end-of-game delays read `GetTickCount` only (`0x005B4C79`, `0x005B4CB0`, `0x005B4CEC`) |

## CODE-TABLE CHANGE commits

None (this worker owns no TSV).

## Cross-file requests

- `world/quests-act2.md` §(hooks table, Tyrael AI row) / staging seam `living_player_within`; I-2; `0x006416D0(a, b)` returns the size-adjusted distance between two units (`missiles.md` §R9.5 item 4), it takes no radius (the "within 12" is the caller `0x0059DF50`'s comparison); rename the seam bound to `0x006416D0` to a distance and keep the radius test in the caller — to PC 2 quests-act2.
- `world/quests-act3.md` open question 2 (`0x00559A30`'s `&level`); 1.14d `0x00559A30` writes the level through the pointer at `0x00559AF8` before any read (player / monster stat 12, else `0x0061DCA0` area MonLvl[Ex], ≤ 1 → 1) and uses it as the item level (`0x00559C66`); mark it answered with a link to `quests-act4.md` §4.7 — to PC 2 quests-act3.

## Recording list

- R-QD-1 (act4 OQ2): one classic Diablo kill with game frames logged: kill frame (0x89 FX 13, `5D 17 02`), each credited player's `5D 17 01` / warp to 103, the uncredited player's `50 17 00`, the game-end frame; two runs (idle / loaded machine) to show whether the 2250 / 2375-frame offsets drift with wall-clock time.
