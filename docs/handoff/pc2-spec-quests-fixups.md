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
| `quests.md` OQ8 | OQ8, §11 | Acts II–V state machines are in the per-act files; every `quests.tsv` row now `specified` |
| `quests.md` OQ9 | OQ9 | events 1, 7, 12: no callback stored anywhere (TSV + `.text` byte scan); event 6 raiser `0x00543DE0` unreachable (no call, no pointer) |
| act1-rest OQ2 | OQ2 | objects spec now owns mode sets (`objects.md` §4), event 1 (`objects-2.md` §18.6); quest bodies §9 / `quests.md` §9.6 |
| act2 OQ8 | OQ8 | init 37 `0x0059DA50` reachable only through table `0x00731BC0`[37], read only by `0x0054F5D0` by `InitFn`; no live row has 37 |
| act2 OQ9, act4 OQ12, act5 OQ6 | `quests.tsv`; act2-2 §4 | rows 8–16, 25–28, 30–36, 38 → `specified`; act2-2 §4 names the Act II row addresses part 1 left unnamed |
| act4 OQ11 | OQ11 | already fixed in `quests-act3.md` §8.6 (104 = Outer Steppes, `cmp eax, 0x68` at `0x005BCC0B`) |
| act5 OQ2 | OQ2 | `0x00558200`: player base stat 12 / monster total stat 12 / else area level; ≤ 1 → 1; Anya's item = character level |
| act5 OQ3 | OQ3 | the resist list stacks: `0x00589FF0` always allocates and attaches a new list, never frees the old one |
| act5-2 OQ3 | OQ3 | `0x0052E2A0`: game types 1/2 only: save each client's character (`0x00532400`) and flush 0xB3 DownloadSave (`0x0052E110`); host code |
| act5-2 OQ4 | OQ4 | the `zoo` column (`bit(22)` @12 = byte +0x0E mask 0x40; `0x0058E8B8`, mask `0x006CE280`) |

## Still open

| Id | Why |
|---|---|
| `quests.md` OQ1 (and act2/3/4/5 OQ1) | status meanings per quest: needs the client quest-log code (`0x0045CC00` 0x52 handler and its tables), a separate client-side read |
| `quests.md` OQ2, OQ4, OQ14; act1-rest OQ3, OQ5, OQ12; act2 OQ10, OQ31 / act2-2 OQ1; act3 OQ8; act4 OQ2, OQ13; act5 OQ7; act5-2 OQ7 | Needs recording (already listed by their writers) |
| act1-rest OQ4 | progression readers: save spec |
| act2 OQ2, OQ3, OQ4 | quest-chest gate / treasure (objects spec), Act II light change (environment spec), 0x27 scroll-text bytes (`0x005456A0`, NPC/chat spec) |
| act4 OQ10 | host ownership of `0x00530590` / `0x0052E2A0`: a design decision (effect now in act5-2 OQ3) |
| act5 OQ5; act5-2 OQ5, OQ6 | states / AI / monster / missile specs |
| QC-6 | implementation work (host seams) |

## CODE-TABLE CHANGE commits

| SHA | File |
|---|---|
| 375bb6b | `specs/world/quests.tsv` (`spec` column: 21 rows → `specified`) |

`quest-messages.tsv`: no change needed (QE-7 rows already placed by
39dabf1; checked against the image).

## Cross-file requests

- to PC 2 objects (`specs/world/object-functions.tsv`): the owner cell
  `world/quests.md` of inits 7, 9, 46, 59, 61 and operates 33, 43 is now
  valid (`quests.md` §9.6 states 46, 59, 43 and links the rest). Optional
  precision: inits 7, 9, 61 and operate 33 → `world/quests-act1-rest.md`
  (§9 items 8–11); no change is required.
- to the implementation (d2-sim quests; no spec change): `crates/d2-sim/src/world/quests/tests.rs`
  `tables_parse_and_check` still asserts `t.messages.len() == 779` and
  `t.rows[40].unknown && t.rows[40].callbacks.is_empty()`; since 39dabf1 /
  b242ee7 the TSVs give 794 messages and row 40 known with 2 callbacks
  (the 375bb6b commit message says "unchanged: 794", meaning the TSV
  count; the test itself still needs the 779 → 794 edit). Also rebind the
  staging seam `living_player_within(unit, radius)`: `0x006416D0` is a
  distance; the caller `0x0059DF50` loops players without state 7 and
  tests < 12 (`quests-act2.md` §10 Tyrael row).

## Recording list

None new.
