# Spec: Client — UI messages (0x5D quest status, 0x63 waypoint menu, 0x77 UI action)

- **Status:** draft: handlers read in the 1.14d `Game.exe` (addresses
  below; jump tables read from the image); recorded bytes from
  `traces/raw/20261006-015956-packets.jsonl` and `-022633-packets.jsonl`;
  unverified: no executable check runs it yet.
- **Target version:** 1.14d
- **Crate/module:** `d2-client::bridge` handlers for 0x5D, 0x63, 0x77;
  their outputs are applied by `d2-client::ui::original`.
- **Related specs:** `client/bridge.md` §10 (output channel);
  `client/model.md` §1 (model fields), §2 (unit sets); `ui/panels.md` §2
  (`SetUIState`), §11 (stash), §12 (cube), §13 (waypoint menu);
  `ui/ui-states.tsv` (state ids); `world/quests.md` §1 (quest flags),
  §6.3 (0x5D sender); `world/waypoints.md` §2 (record, load copy), §5.3
  (0x63 layout); `world/cube.md` §1 (0x77 senders); `audio/triggers.md`
  §3, §11 (sounds); `render/lighting.md` §9.2 r3 (eclipse);
  `render/composition.md` §4 (state-loop video flags).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 39–49 |
| Inputs | 50–57 |
| Outputs / state changes | 58–64 |
| Rules | 65–66 |
|   1. 0x5D quest status (`0x0045E540` → `0x004A2CB0`) | 67–139 |
|   2. 0x63 waypoint menu (`0x0045E670` → `0x0049CF90`) | 140–163 |
|   3. 0x77 UI action (`0x0045E800` → `0x004B8CF0`) | 164–201 |
| Constants & data dependencies | 202–213 |
| Randomness | 214–218 |
| Edge cases & original bugs | 219–230 |
| Test vectors | 231–254 |
| Provenance | 255–270 |
| Open questions | 271–290 |
<!-- /index -->

Owned ids: 0x5D, 0x63, 0x77.

## Summary

Three S→C messages whose 1.14d handler is a UI dispatch. Each handler
here does the model part itself (a few 0x5D cases write `ClientWorld`)
and hands the rest to the UI layer as one output (`client/bridge.md`
§10) carrying the message fields; the UI layer runs the dispatch below.
0x5D is one quest's status (`world/quests.md` §6.3): it updates the
quest log, plays quest sounds and sets a few one-off flags. 0x63 opens
the waypoint menu with the player's waypoint record. 0x77 is a code that
opens or closes the stash, the cube or the trade screen.

## Inputs

| Name | Type | Source |
|---|---|---|
| message | id + bytes | `client/bridge.md` §2, §6 |
| local player, unit set S | `client/model.md` §2, §3 | model |
| UI state flags | `ui/panels.md` §2 | UI layer, at delivery |

## Outputs / state changes

- Model: `exit_requested` (§1 r3), the eclipse (§1 r3), unit field
  `quest_untargetable` (§1 r4).
- Outputs: `QuestUi` (0x5D), `WaypointMenu` (0x63), `TradeAction`
  (0x77), each applied by the UI layer (§1 r5, §2, §3).

## Rules

### 1. 0x5D quest status (`0x0045E540` → `0x004A2CB0`)

1. Layout (6 bytes, `world/quests.md` §6.3): chain u8@1 (c), record
   flags u8@2 (f), status u8@3 (s), extra @4 read as i16 (v; the sender
   writes a u16 count). `0x0045E540` copies the 6 bytes and calls
   `0x004A2CB0(1, copy)`; the first argument is unused.
2. Dispatch, first match wins (byte table `0x004A2F94` + pointer table
   `0x004A2F74` for f bit 0, codes 3–33; `0x004A2FD0` + `0x004A2FB4` for
   f bit 1, codes 4–36). "Sound n" = UI sound request n
   (`0x004B9A00(n, no unit, 0, 0, 0)`, `audio/triggers.md` §11); T = the
   quest-log tail (r6). A case with no listed action does nothing.

   | f | c | Action | Part |
   |---|---|---|---|
   | bit 0 set | 3 | screen message of string 3708 (0xE7C) (`0x0049E3A0(text, 0)`) | output |
   | bit 0 set | 4 | every monster of set S of class 146 (`cain1`): unit flag +0xC4 bit 0x2 cleared (`0x00464990` with `0x004A2C70`) | model (r4) |
   | bit 0 set | 10 | eclipse (`0x0044C820`, `render/lighting.md` §9.2 r3) | model |
   | bit 0 set | 13 | `0x0046F870(211, 1)` (client effect of monstats row 211; open question 1) | output |
   | bit 0 set | 15 | screen message of string 3710 (0xE7E) | output |
   | bit 0 set | 23 | `exit_requested` := 1 (`0x0044D520` → `[0x007A0620]`, `client/model.md` §1) | model |
   | bit 0 set | 33 | sound 237 | output |
   | bit 0 set | other | nothing | – |
   | bit 0 clear, bit 1 set | 4 | sound 241 | output |
   | bit 1 | 8, 15, 18, 22, 35 | sound 7 | output |
   | bit 1 | 23 | expansion game (`0x0044DCC0`: `[0x007A04F4]` ≠ 0) → `[0x007BC9D8]` := 1; else `0x0044EC80` (video-5 flag `[0x007A0604]` := 1, `render/composition.md` §4, and the character record's word +0x1EF bits 10–12 := difficulty + 1) then `[0x007BC9D4]` := 1 | output |
   | bit 1 | 32 | sound 217 | output |
   | bit 1 | 33 | sound 243 | output |
   | bit 1 | 36 | video-7 flag `[0x007A0628]` := 1 (`0x0044D530(1)`, `render/composition.md` §4) | output |
   | bit 1 | other | nothing | – |
   | = 0x10 | 10 | sound 2456, then sound 2474 | output |
   | = 0x10 | 33 | sound v | output |
   | = 0x10 | other | nothing | – |
   | bit 5 set (bits 0, 1 clear, f ≠ 0x10) | 32 | `[0x007BF2AC]` := v, then T | output |
   | bit 5 | 33 | every monster of set S of class 527 (`drehyaiced`): unit flag +0xC4 bit 0x2 cleared (`0x004A2C90`) | model (r4) |
   | bit 5 | other | r7 | output |
   | none of the above (bits 0, 1, 5 clear, f ≠ 0x10) | any | T | output |

3. The handler performs the rows marked model at receive, in the order
   of the table (only one row matches). Eclipse: `render/lighting.md`
   §9.2 r3 (with no client act it sets the pending flag). `exit_requested`
   is the field 0x06 also writes.
4. **`quest_untargetable`** (d2rs field on `ClientUnit`, added by this
   spec): true once a 0x5D row above has cleared the unit's flag bit 0x2
   (+0xC4; D2MOO `UNITFLAG_TARGETABLE`, a hint). Units are visited in the
   monster update order of `client/model.md` §5 rule 3 (`0x00463C90`
   over the type-1 heads of set S, `0x007A6070`); only units in S at
   receive are changed. The rest of the +0xC4 word is not modelled
   (open question 2).
5. Every row marked output becomes one `QuestUi` output {c, f, s, v};
   the UI layer runs that row (and T or r7) at delivery. The UI layer
   reads its own state (latch, UI flags) then, not at receive.
6. **Quest-log tail T.** The quest-log table `0x00723F30` has 41
   entries of 16 bytes: +2 slot within the act tab (0–5), +3 act (0–4;
   9 = unused), +8 chain (37 = none); "the entry of c" is the first
   entry whose +8 = c (`0x004A1910`, `0x004A2C30`), else none.
   1. Latch `[0x007BF298]` = 0: `SetUIState(17 UI_QUESTLOG, on, 0)`
      (`ui/panels.md` §2; the quest-log alert button). Returned 1 → the
      entry of c, if any, sets the selected slot of its act
      (`[0x007BF280 + 4 · act]` := slot) (`0x004A2C30`), and latch := 1.
      Returned 0 → nothing.
   2. Latch ≠ 0: ui 15 (UI_QUESTSCREEN) open (`0x004538D0`) and an entry
      of c exists → its status byte (`[0x007BF356 + index]`) := s; and
      when `[0x007BF2B3]` = 0 and the entry's act = `[0x007C0255]`
      (the act tab shown): `[0x007BF2B9]` := the entry's slot.
7. **f bit 5, other c** (the Den of Evil counter path): when the client
   quest flags (`0x004B32D0` → record `[0x007C0D43]`, `world/quests.md`
   §1) have quest 1 bit 0 or quest 1 bit 1 set (`0x0065C310`) →
   nothing. Else `[0x007BF2A4]` := v when 0 ≤ v ≤ 5, else 666; then:
   v = 5 and c = 23 → `0x0046BFD0(5)` (counter `[0x007A7464]` := a draw
   of `0x00410A80` + 5 when it is 0; `render/lighting.md` §10); v = 5
   and c ≠ 23 → T; v ≠ 5 → the status byte of the entry whose +8 = 1
   := s (no T).

### 2. 0x63 waypoint menu (`0x0045E670` → `0x0049CF90`)

1. Layout and record: `world/waypoints.md` §5.3 (object GUID u32@1,
   16-byte record @5). Model state written: none. The handler emits one
   `WaypointMenu` output {GUID, record bytes as received}.
2. The UI layer, at delivery (`0x0049CF90`):
   1. `SetUIState(20 UI_WAYPOINT, on, jump 1)` (`ui/panels.md` §2,
      §13 r1). Returned ≠ 1 → nothing more (the record is not stored).
   2. Input reset `0x0044DA40`.
   3. Tab `[0x007BF086]`: the local player's room → level → act index a
      (`0x006427F0`); no player, room or level → tab 0. Else
      `0x0049C760(a)`: a ≥ the tab count `[0x007224E4]` (5 in the
      image) → 0; else start at tab a and step down while the act's
      gate is closed: tab 4 needs quest 28 bit 0, tab 3 quest 23 bit 0,
      tab 2 quest 15 bit 0, tab 1 quest 7 bit 0 in the client quest
      flags (`0x004B32D0`, `0x0065C310`); tab 0 has no gate.
   4. Row rebuild `0x0049C7F0` (from the record stored so far).
   5. Waypoint GUID `[0x007BF07D]` := GUID; the record is stored with
      the load copy (`0x00661030`, `world/waypoints.md` §2 rule 5:
      magic 0x0102 kept, 0x0000 / 0x0101 wipe to index 0 only) into the
      buffer `[0x007BF081]`; close latch `[0x007BF085]` := 0.
   6. Row rebuild again, now from the new record. Drawing and messages:
      `ui/panels.md` §13 r2–r7.

### 3. 0x77 UI action (`0x0045E800` → `0x004B8CF0`)

1. Layout: code u8@1 (2 bytes). Model state written: none. The handler
   emits one `TradeAction` output {code}.
2. The UI layer, at delivery, dispatches on the code (pointer table
   `0x004B8F7C`, 22 entries; code > 0x15 → nothing). Trade state
   `[0x007C0E7C]` (0 none … 7 refused); "decline" = C→S 0x4F button 2
   (p1 = p2 = 0, `0x00478600`); "refuse" = `0x004B8CB0`: decline, then,
   unless the state is 7, `[0x007C0E74]` := 1, `[0x007C0E70]` :=
   `GetTickCount`, state := 7. "Close trade(x)" = `0x004B8940(x)`
   (rule 3).

   | Code | Action | Sent in single player by |
   |---|---|---|
   | 0x00 | state ≠ 0 → refuse. Else ui 23 (UI_MPTRADE) closed: `SetUIState(23, on, 0)`; refused → refuse; else `[0x007BCE28]` := 0, `[0x007C0E70]` := −1. Then state := 1, `0x004B8BF0`, `0x00489360(0)` | – (trade) |
   | 0x01 | state ≠ 0 → refuse. Local player has a cursor item, or `0x004B85E0` ≠ 0, or `0x004B8AD0` = 0, or ui 5 (chat) open → decline. Else ui 23 open → state 2, `0x00489360(0)`; closed → `SetUIState(23, on, 0)`: refused → refuse, else `[0x007C0E70]` := −1, state 2, `0x00489360(0)` | – |
   | 0x02 | `0x004897E0(0, 0)`, close trade(0) | – |
   | 0x05 | ui 23 open: `[0x007C0E70]` := −1, `0x00489360(1)`; state 3 → state 5, `0x004897E0(0, 1)`; 4 → state 6, `0x004897E0(1, 1)`; 5 → `0x004897E0(0, 1)`; 6, 7 → nothing; other → refuse. Ui 23 closed → nothing | – |
   | 0x06 | ui 23 open: C→S 0x4F button 8 (p1 = u16 `[0x007C0E6A]`, p2 = u16 `[0x007C0E68]`), `[0x007C0E70]` := −1, `0x00489360(1)`; state 1 → `[0x007C0E78]` := 0, `0x004897E0(0, 0)`, state 3, free the list `[0x007C0E6C]` (`0x004B8020`) if any; 2 → refuse; 3–7 → `0x004897E0(0, 0)`, state 3; other → refuse | – |
   | 0x09 | player event sound 23 on the local player (`0x004CB9C0`, `audio/triggers.md` §3) | – |
   | 0x0A | player (0, `[0x007C0E60]`) in S → player event sound 23 on it | – |
   | 0x0C | state := 0, close trade(0) | `world/cube.md` §1 (C→S 0x4F with no interaction) |
   | 0x0D | state := 0, close trade(1) | – |
   | 0x0E | `[0x007BCE28]` := 1 | – |
   | 0x0F | `[0x007BCE28]` := 0 | – |
   | 0x10 | open the stash (`0x00489E00`, `ui/panels.md` §11 r1) | stash object (recorded `77 10`) |
   | 0x11 | stash close (`0x00489F50`): inventory mode 0x0C or 0x0D → mode 0 and `SetUIState(25 UI_STASH, off, 0)`; free the item lists `[0x007BCC2C]`, `[0x007BCC3C]` (`0x00478A00`) | `world/cube.md` §1 (cube opened while at the stash) |
   | 0x15 | open the cube (`0x0048A460`, `ui/panels.md` §12 r1) | `world/cube.md` §1 |
   | 0x03, 0x04, 0x07, 0x08, 0x0B, 0x12–0x14 | nothing | – |

3. **Close trade(x)** (`0x004B8940`): `[0x007C0E80]` := 0; free the
   lists `[0x007C0E6C]` and `[0x007C0E64]` (`0x004B8020`) if set. Unless
   the state is 7: ui 23 open → `SetUIState(23, off, 0)`, `[0x007BCE28]`
   := 0, `0x00487B30`; then `0x00463DF0()` = 0 and x ≠ 0 →
   `SetUIState(1 inventory, toggle, 0)`; ui 5 open → `SetUIState(5, off,
   0)`. Then state ≠ 0 → decline and state := 0. (0x0C and 0x0D set the
   state to 0 first, so they never decline.)

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| UI states | 15 QUESTSCREEN, 17 QUESTLOG, 20 WAYPOINT, 23 MPTRADE, 25 STASH, 5 chat, 1 inventory | `ui/ui-states.tsv` |
| quest-log table | `0x00723F30`, 41 × 16 bytes | §1 r6 |
| 0x5D jump tables | `0x004A2F94`/`0x004A2F74` (31 codes from 3), `0x004A2FD0`/`0x004A2FB4` (33 codes from 4) | §1 r2 |
| waypoint tab count | `[0x007224E4]` = 5 | §2 r2 |
| 0x77 pointer table | `0x004B8F7C` (22 codes); state tables `0x004B8FD4` (states 3–7), `0x004B8FE8` (states 1–7) | §3 |
| strings | 3708, 3710 | §1 r2 |
| monster classes | 146 `cain1`, 527 `drehyaiced` (`monstats` `hcIdx`, `patch_d2`) | §1 r2 |

## Randomness

None on the game seeds. §1 r7's counter uses the client draw
`0x00410A80` (`render/lighting.md` §10 owns it).

## Edge cases & original bugs

- 0x5D: a case with f bit 0 set never reaches the bit-1 table, even for
  a code only the bit-1 table lists (e.g. f = 3, c = 8 → nothing).
- 0x5D f bit 5 with c ∉ {32, 33}: the gate tests quest 1 (Den of Evil)
  whatever c is, and writes the status of the quest-1 entry, not of c.
- 0x63: when the waypoint UI cannot open (gate refusal), the record and
  GUID are not stored; the next menu uses whatever an earlier 0x63
  stored.
- 0x63: the first row rebuild runs on the old record.
- 0x77 codes 0x00 and 0x01 can send C→S 0x4F during the receive.

## Test vectors

Synthetic unless a recording is named ("A" = `-015956`, "B" =
`-022633`).

| Input | Expected | Source |
|---|---|---|
| `5d 01 00 01 0000`, latch 0, UI_QUESTLOG gate allows | no model change; output `QuestUi` {1, 0, 1, 0}; UI: QUESTLOG on, act 0 selected slot := 0, latch 1 | A seq 129118 (UI state synthetic) |
| `5d 0a 01 00 0000` with a client act | eclipse set (index 5, ticks 0); no output | §1 r3 |
| `5d 17 01 00 0000` | `exit_requested` = 1; no output | §1 r2 |
| `5d 04 01 00 0000`, monsters (1, 5) class 146, (1, 6) class 147 | (1, 5) `quest_untargetable`; (1, 6) unchanged | §1 r4 |
| `5d 21 20 00 0000`, monster (1, 9) class 527 | (1, 9) `quest_untargetable` | §1 r2 |
| `5d 08 02 00 0000` | output; UI plays sound 7 | §1 r2 |
| `5d 08 03 00 0000` | output; UI does nothing (bit-0 table has no 8) | edge case 1 |
| `5d 21 10 00 0500` | output; UI plays sound 5 | §1 r2 |
| `5d 05 20 03 0200`, quest 1 bits 0, 1 clear | `[0x007BF2A4]` = 2; status byte of the chain-1 entry := 3 | §1 r7 |
| `5d 05 20 03 0700`, quest 1 bits clear | `[0x007BF2A4]` = 666; status byte of the chain-1 entry := 3 | §1 r7 |
| `63 0a000000 0201 0300 0000…` | output `WaypointMenu` {0x0A, record}; UI: waypoint on, GUID 0x0A, record stored, tab by the act | B seq 7852 |
| `63 …` with the waypoint UI refused | nothing stored | §2 r2.1 |
| `77 10` | output `TradeAction` 0x10; UI opens the stash | A seq 104179, B seq 120653 |
| `77 15` | UI opens the cube | `world/cube.md` §1 |
| `77 0c`, trade state 0 | UI: close trade(0), no C→S message | §3 r3 |
| `77 16` | nothing | §3 r2 |

## Provenance

1.14d `Game.exe`, read with `tools/ghidra/disasm.py` and the Ghidra
exports (spec session 2026-10-07): 0x5D `0x0045E540`, `0x004A2CB0`
(tables `0x004A2F74`, `0x004A2F94`, `0x004A2FB4`, `0x004A2FD0` read from
the image), `0x004A1910`, `0x004A2C30`, `0x004A2C70`, `0x004A2C90`,
`0x00464990` → `0x00463C90`, `0x0044D520`, `0x0044D530`, `0x0044DCC0`,
`0x0044EC80`, `0x00483300`, `0x00483340`, `0x0046BFD0`, `0x004B32D0`,
`0x0065C310`, quest-log table `0x00723F30` (dumped); 0x63 `0x0045E670`,
`0x0049CF90`, `0x0049C760` (table `0x0049C7DC`), `0x0049C7F0`,
`0x0044DA40`; 0x77 `0x0045E800` → `0x004B8CF0` (tables `0x004B8F7C`,
`0x004B8FD4`, `0x004B8FE8`), `0x004B8940`, `0x004B8CB0`, `0x00489F50`,
`0x0048A6E0`, `0x0048A6F0`, `0x00453A90`, `0x004538D0`. Monster classes
from `patch_d2` `monstats.txt`. Recorded messages: A (0x5D, 0x63, 0x77)
and B (0x63 × 2, 0x77), split with the S→C size rule.

## Open questions

1. `0x0046F870(row, 1)` (0x5D f bit 0, c 13; also `render/lighting.md`
   §10 r4): which client effect a monstats row starts; owner the client
   effect spec.
2. The full client unit flag word +0xC4 (initial value per kind, other
   writers), so `quest_untargetable` can become a real flag field;
   owner `client/msg-units.md`.
3. Who resets the quest-log latch `[0x007BF298]` and the meaning of
   `[0x007BF2A4]`, `[0x007BF2AC]`, `[0x007BF2B9]` in the quest-log draw
   (Phase 6 quest-log spec).
4. The client quest flags record `[0x007C0D43]`: which messages write it
   (0x28 / 0x29 / 0x52 / 0x5E are unowned) — needed by §1 r7 and §2
   r2.3.
5. The trade helpers `0x004B8BF0`, `0x004B85E0`, `0x004B8AD0`,
   `0x00489360`, `0x004897E0`, `0x00487B30` (trade screen; out of Phase
   0–6 single-player scope except code 0x0C's close path).
6. Code 0x10's single-player sender (stash object operate): owner
   `world/objects.md`; recorded twice.
