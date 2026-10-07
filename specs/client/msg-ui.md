# Spec: Client — UI messages (quest status, waypoint menu, UI actions, chat, NPC text, hire list)

- **Status:** draft: handlers read in the 1.14d `Game.exe` (addresses
  below; jump tables read from the image); recorded bytes from
  `traces/raw/20261006-015956-packets.jsonl` and `-022633-packets.jsonl`;
  unverified: no executable check runs it yet. 2026-10-07: §4–§11
  (0x26, 0x27, 0x4E, 0x4F, 0x50, 0x58, 0x8A, 0x91, 0x78) read the same
  way; their UI consumers' display rules are PC 2's (`ui/*`).
- **Target version:** 1.14d
- **Crate/module:** `d2-client::bridge` handlers for 0x5D, 0x63, 0x77,
  0x26, 0x27, 0x29, 0x4E, 0x4F, 0x50, 0x52, 0x58, 0x5E, 0x78, 0x8A, 0x91,
  0x9B;
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
| Summary | 56–71 |
| Inputs | 72–79 |
| Outputs / state changes | 80–92 |
| Rules | 93–94 |
|   1. 0x5D quest status (`0x0045E540` → `0x004A2CB0`) | 95–167 |
|   2. 0x63 waypoint menu (`0x0045E670` → `0x0049CF90`) | 168–191 |
|   3. 0x77 UI action (`0x0045E800` → `0x004B8CF0`) | 192–229 |
|   4. 0x26 chat and overhead text (`0x0045DFC0` → `0x0049F490`) | 230–280 |
|   5. 0x27 NPC text (`0x0045E0A0` → `0x004A1600`) | 281–300 |
|   6. 0x4E hire offer and 0x4F hire list reset | 301–313 |
|   7. 0x50 quest special (`0x0045E370` → `0x004B9210`) | 314–337 |
|   8. 0x58 UI open (`0x0045E490` → `0x004C0550`) | 338–357 |
|   9. 0x8A NPC wants to interact (`0x0045EA40` → `0x004B3380`) | 358–376 |
|   10. 0x91 NPC intros (`0x0045E580` → `0x004B3510`) | 377–387 |
|   11. 0x78 trade partner (`0x0045E810` → `0x004B9010`) | 388–396 |
|   12. 0x29 game quest flags (`0x0045D3A0` → `0x004B2620`) | 397–406 |
|   13. 0x52 quest log status (`0x0045CC00` → `0x004A40D0`) | 407–417 |
|   14. 0x5E game quest availability (`0x0045E570` → `0x004B92B0`) | 418–425 |
|   15. 0x9B hireling revive state (`0x0045EAC0` → `0x004B6980`) | 426–436 |
| Constants & data dependencies | 437–448 |
| Randomness | 449–453 |
| Edge cases & original bugs | 454–465 |
| Test vectors | 466–505 |
| Provenance | 506–538 |
| Open questions | 539–578 |
<!-- /index -->

Owned ids: 0x26, 0x27, 0x29, 0x4E, 0x4F, 0x50, 0x52, 0x58, 0x5D, 0x5E,
0x63, 0x77, 0x78, 0x8A, 0x91, 0x9B.

## Summary

Three S→C messages whose 1.14d handler is a UI dispatch. Each handler
here does the model part itself (a few 0x5D cases write `ClientWorld`)
and hands the rest to the UI layer as one output (`client/bridge.md`
§10) carrying the message fields; the UI layer runs the dispatch below.
0x5D is one quest's status (`world/quests.md` §6.3): it updates the
quest log, plays quest sounds and sets a few one-off flags. 0x63 opens
the waypoint menu with the player's waypoint record. 0x77 is a code that
opens or closes the stash, the cube or the trade screen. The others
(§4–§11) carry chat and overhead text, NPC text, the hire list, quest
specials, a few dialogs, NPC alerts and NPC intro marks; apart from
the few model writes named in their tables, all their state is UI
state (`0x007BExxx`–`0x007C5xxx`, unit record +0xA4), kept by the UI
layer.

## Inputs

| Name | Type | Source |
|---|---|---|
| message | id + bytes | `client/bridge.md` §2, §6 |
| local player, unit set S | `client/model.md` §2, §3 | model |
| UI state flags | `ui/panels.md` §2 | UI layer, at delivery |

## Outputs / state changes

- Model: `exit_requested` (§1 r3, §7 code 23), the eclipse (§1 r3),
  unit field `quest_untargetable` (§1 r4), `outgoing` (§7 code 23),
  the local player's `cursor_item` (§8 code 5).
- Outputs: `QuestUi` (0x5D), `WaypointMenu` (0x63), `TradeAction`
  (0x77), `ChatLine` (0x26), `NpcText` (0x27), `HireOffer` (0x4E),
  `HireListReset` (0x4F), `QuestSpecial` (0x50), `OpenUi` (0x58),
  `NpcInteract` (0x8A), `NpcIntro` (0x91), `TradePartner` (0x78),
  `GameQuestFlags` (0x29), `QuestLog` (0x52), `QuestAvailability`
  (0x5E), `MercRevive` (0x9B), each
  applied by the UI layer.

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

### 4. 0x26 chat and overhead text (`0x0045DFC0` → `0x0049F490`)

1. Layout (`sim/server-messages.tsv`; senders `sim/intents-events.md`
   §3.5, §9 rule 16): type u8@1, lang u8@2, unit type u8@3, GUID u32@4,
   u8@8, u8@9, name cstr @10, text cstr after the name's NUL. The
   handler copies bytes 0–9, the name (`0x004135D0`, at most 16 bytes)
   and the text (at most 256 bytes) into a 0x11C-byte record and calls
   `0x0049F490(record)`. Model state written: none (the overhead record
   of rule 4 is UI-layer state).
2. The handler emits one `ChatLine` output: the record fields, plus the
   values `0x0049F490` reads from the model, captured at receive
   (`client/bridge.md` §10 r3): whether (u8@3, GUID) is in set S
   (`0x00463990`), and for a player unit (type 0) its `name`
   (`client/msg-units.md` §1.1 r5; `0x00622150`).
3. The UI layer, at delivery, runs `0x0049F490` on it, in order:
   1. u8@3 = 0 and the roster squelch test `0x0047A170(GUID)` is 0 (a
      roster entry for GUID, `0x004792E0`, with relation 4 set between
      the local player and it, `0x004DC440`) → nothing. Relations come
      only from out-of-scope messages (0x8B, 0x8C), so single player
      always passes.
   2. When the unit was present and a player, or the name is non-empty:
      n := the captured player name if present, else the message name
      (a present non-player unit with a non-empty name is the fatal
      assert 0xFA8 of `0x00622150`); the text filter object
      (`0x00611560`) tests n (method +8) and the text (method +0x14);
      either non-zero → nothing.
   3. Text conversion `0x0049E280(text, lang)` fails → nothing:
      languages 6, 7, 9, 12 convert with `MultiByteToWideChar`; when
      lang differs from the client's own language id (`0x00525150`,
      0–13) and the pair is not {7, 12}, a text with a character
      ≥ 0x80 is refused.
   4. By type: 4 → screen message of the text (`0x0049E3A0`); 6 → a
      formatted line (`0x00452300`, then `0x0049E3A0`); 2 (whisper) and
      1 (broadcast; with u8@3 ∈ {0, 1} a plain screen message) → a line
      with the name (`0x0049E3A0`); 5 → overhead text of the unit (rule
      4) when the unit was present; 7 → `0x0048BBE0(u8@8)` after a wide
      conversion; any other type → nothing. Line formats, colours and
      the chat area: `ui/*` (PC 2, `docs/handoff/xpc-to-pc2.md`).
4. **Overhead text** (`0x0049F410(unit, text, lang)`; unit record
   +0xA4): an empty text frees the unit's record and clears +0xA4;
   otherwise a new record (`0x00661110`, 0x110 bytes: duration d =
   8 · min(n, 254) + 125 at +0, end = `[0x007BF20E]` + d at +4, lang
   at +8 (`0x00661230`; the creator's default is the client's own
   language), text ≤ 254 chars at +0x10) replaces the old one. The
   counter `[0x007BF20E]` steps once per overhead draw (`0x004A0E70`);
   the draw frees a record whose end has passed (`0x004A0A00`). Only
   UI code (0x26, 0x27 §5 r2.1, the draw) reads or writes +0xA4, so
   d2rs keeps the record in the UI layer keyed by the unit key; a unit
   removed from the model loses it (`client/model.md` §2 rule 5 frees
   it).

### 5. 0x27 NPC text (`0x0045E0A0` → `0x004A1600`)

1. Layout (40 bytes, `sim/intents-events.md` §6 rule 6): unit type
   u8@1, GUID u32@2, count u8@6, entry k < 8: kind u8@8+4k, string id
   u16@10+4k. The handler copies the 40 bytes and calls `0x004A1600`.
   Model state written: none. One `NpcText` output: the 40 bytes, plus
   (captured at receive) whether (1, GUID) (type 1) or (2, GUID) (type
   2) is in S and, for type 2, the object's `class` (0 when absent).
2. The UI layer, at delivery (`0x004A1600`):
   1. Type 1, unit present, count 1 and kind0 3: the unit's overhead
      text (§4 r4) := the decimal string of str0 (`"%d"`), default
      language. Otherwise type 1: the NPC text list `[0x007BF250]` is
      freed (`0x006612A0`) and rebuilt from bytes 6–39 (`0x00661240`,
      `0x00661510`), then started (`0x006616E0`).
   2. Type 2: kind0 3 → `0x004A1510(str0)` (a timed text box). Else
      `[0x007BF234]` := the object's class, `[0x007BF206]` := GUID,
      `0x004A1320(str0, 0, 1)` (the NPC dialog text panel).
   3. Any other type: the list is freed and `[0x007BF250]` := 0.
   What the list, box and panel show: `ui/*` (PC 2).

### 6. 0x4E hire offer and 0x4F hire list reset

1. **0x4E** (`0x0045E3D0` → `0x004B3240`): layout name u16@1, seed
   u32@3 (`world/npc.md` §7.2). The hire table `0x007C0C85` has 10
   entries of 16 bytes: name u16 +0, seed u32 +4, used u32 +8. The
   first entry with used = 0 := {name, seed, used 1}; all used →
   nothing.
2. **0x4F** (`0x0045E3C0` → `0x004B3290`, 1 byte): used := 0 in all 10
   entries.
3. Model state written: none. Outputs `HireOffer` {name, seed} and
   `HireListReset` {}; the UI layer applies rules 1–2 to its hire
   table at delivery. The hire list panel: `ui/*` (PC 2).

### 7. 0x50 quest special (`0x0045E370` → `0x004B9210`)

1. Layout (15 bytes): code u16@1, u16 words @3, @5, @7, @9, @11, @13
   (which are written per code: `sim/intents-events.md` §6 rule 6).
2. Dispatch on the code (byte table `0x004B9284` for codes 1–36,
   pointer table `0x004B9268`; other codes nothing):

   | Code | Action | Part |
   |---|---|---|
   | 1 | `[0x007BF2A4]` := u16@3, `[0x007BF2A8]` := u16@5, `[0x007BF2AC]` := u16@7 (`0x004A28A0`; the quest-log values of §1 r7) | output |
   | 2 | the hire-table entry (§6) whose name = u16@3, or the end of the table, → `0x004939B0` (the hire popup) | output |
   | 3 | `[0x007C0D29]` ≠ 0 and the monster (1, `[0x007C0D25]`) in S → `[0x007C0D39]` := 1, `[0x007C0D3D]` := 3 (`0x004B25C0`) | output |
   | 4 | `[0x007BF098..0x007BF0A3]` := bytes 3–14, `[0x007BF254]` := 0 (`0x0049FF60`) | output |
   | 6 | `0x004B25C0`: does nothing (code ≠ 3) | – |
   | 13 | `0x0049FF60`: does nothing (code ≠ 4) | – |
   | 23 (0x17) | C→S **0x69** (`0x00477EE0`; appended to `outgoing`, system queue; in single player, game type `[0x007A0610]` = 0, it writes nothing else); `[0x0070EE8C]` := 0 (`0x0044B880`), `[0x007A0674]` := 1 (`0x0044C860`); `exit_requested` := 1 (`0x0044D520`) | model: the send and `exit_requested`; output: the two flags |
   | 36 (0x24) | `[0x007C025F]` := 1, `[0x007C0265]` := u16@3 (`0x004A3100`) | output |
   | other | nothing | – |

3. The handler performs the model part at receive; codes 1, 2, 3, 4,
   23 and 36 also emit one `QuestSpecial` output {code, the six
   words}, applied by the UI layer. Code 3's monster lookup uses a UI
   field, so the UI layer resolves it at delivery (open question 7).

### 8. 0x58 UI open (`0x0045E490` → `0x004C0550`)

1. Layout (7 bytes): GUID u32@1 (−1 without a unit), code u8@5, arg
   u8@6 (written by the sender only with code 5,
   `sim/intents-events.md` §3.5).
2. Dispatch on the code (table `0x004C05F8`); code > 7, 2 or 3 →
   fatal 0x354 (bridge: handler error).

   | Code | Action | Part |
   |---|---|---|
   | 0 | `[0x007C5470]` := 0, `[0x007C547C]` := GUID, `0x004C0380` (the dialog opens; its refusal path sends a C→S message through the send path) | output |
   | 1 | `0x004C0090` (close) | output |
   | 4 | `[0x007C5474]` := 2; the list `[0x007C5484]` freed (`0x004B8020`) and cleared | output |
   | 5 | the local player's cursor item is taken out of its inventory (`0x0063C180(inventory +0x60, none)` → `0x0063AAF0`): `cursor_item` := none (`client/msg-stats-items.md` §2 r5; the item unit stays in S); then UI sound 5 when arg ≠ 0, else 3 (`0x004B9A00`), then `0x004C0090` | model: `cursor_item`; output: the rest |
   | 6, 7 | `0x004C0090`, then `0x004B3FB0` | output |

3. Every code that does not assert emits one `OpenUi` output {GUID,
   code, arg} after its model part. The dialogs: `ui/*` (PC 2);
   senders: `world/quests-act2.md` §8.6 (orifice).

### 9. 0x8A NPC wants to interact (`0x0045EA40` → `0x004B3380`)

1. Layout (6 bytes): unit type u8@1 (the sender writes 1), GUID u32@2.
   Model state written: none.
2. One `NpcInteract` output {unit key, and, captured at receive: unit
   present, its `class`, its monster-data field +0x3C (`0x004AE130`;
   −1 when absent), and whether S holds an object of class 318
   (`eunuch harem blocker`) in mode 2 (`0x004649D0` over the type-2
   heads, predicate `0x004B3360`)}.
3. The UI layer, at delivery: unit absent → nothing. Class 534
   (`act5pow`) → UI sound 4603 on the unit when field +0x3C ≠ −1, else
   4607. Otherwise, unless the unit is the interact NPC (`[0x007C0D29]`
   set and its GUID = `[0x007C0D25]`): overlay 72 on the unit
   (`0x00470390(unit, 72, 3, …)`, `render/unit-composite.md`); then
   class 331 (`act2guard2`), `0x004B1620()` = 0, the client quest flags
   `[0x007C0D43]` set with quest 12 bits 8 and 1 both clear
   (`0x0065C310`) and no blocker in mode 2 → UI sound 3983 on the unit.
   Sounds: `audio/triggers.md` §11 request path.

### 10. 0x91 NPC intros (`0x0045E580` → `0x004B3510`)

1. Layout (26 bytes, `world/quests.md` §6.7): u8@1 (act; not read),
   12 NPC class slots u16@2+2k (0xFFFF = empty). Model state written:
   none. One `NpcIntro` output {the 12 slots}.
2. The UI layer, at delivery: for each slot value v < the monstats row
   count (data +0xA80), every entry of the intro table `0x00726850`
   (`[0x0072554C]` entries of 0x16 bytes, NPC class at +0) whose class
   = v gets its flag byte +0x12 := 1. Nothing is cleared here. What the
   flag shows: `ui/*` (PC 2).

### 11. 0x78 trade partner (`0x0045E810` → `0x004B9010`)

1. Layout (21 bytes): name 16 bytes @1, GUID u32@17 (multiplayer
   trade, out of scope). Model state written: none. One `TradePartner`
   output {name, GUID}.
2. The UI layer: the name (byte 15 forced to 0) is converted to wide
   text into `[0x007C0E84]` (`0x00526F20`); `[0x007C0E60]` := GUID (the
   partner §3 code 0x0A plays its sound on).

### 12. 0x29 game quest flags (`0x0045D3A0` → `0x004B2620`)

1. Layout (97 bytes; server `0x0053D700`, `world/quests.md` §1): the
   game's quest record, 96 bytes @1. Model state written: none. One
   `GameQuestFlags` output {96 bytes}.
2. The UI layer: the game quest record `[0x007C0D47]` := the 96 bytes
   (`0x0065C4D0(record, bytes, 0x60, 0)`, the quest-record load of
   `world/quests.md` §1). The player's record `[0x007C0D43]` (read by
   §1 r7, §2 r2.3, §9) is written by 0x28 (open question 4).

### 13. 0x52 quest log status (`0x0045CC00` → `0x004A40D0`)

1. Layout (42 bytes; server `0x0053D840`): 41 status bytes @1, one per
   quest-log entry (§1 r6). Model state written: none. One `QuestLog`
   output {41 bytes}.
2. The UI layer: bytes 0–41 of the message are copied to `0x007BF355`
   (so entry i's status byte `[0x007BF356 + i]` := byte @1+i);
   `[0x007BF2B0]` := 0; then, when `0x00483350()` is 0: latch
   `[0x007BF298]` = 2 → `0x004A23D0`; and `0x004A3220([0x007C0255],
   1)` (the act tab shown). The quest log: `ui/*` (PC 2).

### 14. 0x5E game quest availability (`0x0045E570` → `0x004B92B0`)

1. Layout (38 bytes; server `0x0053D830`): 37 bytes @1. Model state
   written: none. One `QuestAvailability` output {37 bytes}.
2. The UI layer: `[0x007C0EA4..0x007C0EC8]` := the 37 bytes,
   `[0x007C0ECC]` := 1. Their meaning: `ui/*` (PC 2) and
   `world/quests.md`.

### 15. 0x9B hireling revive state (`0x0045EAC0` → `0x004B6980`)

1. Layout (7 bytes; server `0x0053E0E0`, `world/npc.md` §7.3): u16@1,
   u16@3 (the handler reads 16 bits; the sender writes u32@3). Model
   state written: none. One `MercRevive` output {u16@1, u16@3}.
2. The UI layer: `[0x00725494]` := u16@1 (−1: the hireling is alive,
   `ui/panels.md` §14), `[0x007C0DD0]` := u16@3; when u16@1 = 0xFFFF,
   `0x004B6440` runs with EAX = 11, 8, 24, 21, 43 in that order (each
   with stack argument 0; the NPC-menu Resurrect edit of `ui/panels.md`
   §14).

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
| `26 05 00 01 05000000 00 00 00 "hi" 00`, monster (1, 5) present | no model change; `ChatLine` {type 5, lang 0, (1, 5) present}; UI: overhead "hi" on (1, 5), end = counter + 141 | §4 r4 (d = 8·2 + 125) |
| `26 04 00 02 00000000 00 00 00 "x" 00` | `ChatLine`; UI: screen message "x" | §4 r3.4 |
| `26 09 …` | `ChatLine`; UI does nothing | §4 r3.4 |
| `27 01 06000000 01 00 00 00 2500` + 28 × `00`, (1, 6) present | `NpcText`; UI: list rebuilt, 1 entry (kind 0, string 0x25) | A seq 37351 |
| `27 01 06000000 01 00 03 00 2500 …`, (1, 6) present | UI: overhead text "37" on (1, 6) | §5 r2.1 |
| `4e 2a00 78563412` ×2, then `4f`, then `4e 0100 00000000` | UI hire table: entries 0, 1 used; after 0x4F none used; then entry 0 = {1, 0} | §6 |
| `50 0100 0300 0500 0700 0000 0000 0000` | `QuestSpecial`; UI: `[0x007BF2A4]` 3, `[0x007BF2A8]` 5, `[0x007BF2AC]` 7 | §7 |
| `50 1700 …` | `outgoing` += `69`; `exit_requested` = 1; `QuestSpecial` code 23 | §7 |
| `50 0600 …` | nothing, no output | §7 |
| `58 ffffffff 05 01`, local player with cursor item (4, 9) | `cursor_item` none, (4, 9) still in S; `OpenUi`; UI sound 5 | §8 |
| `58 ffffffff 02 00` | handler error (fatal 0x354) | §8 r2 |
| `8a 01 07000000`, (1, 7) class 148 present | `NpcInteract` {(1, 7), present, 148}; UI: overlay 72 on (1, 7) when not the interact NPC | B seq 1563 |
| `91 00 9400 ffff …` (10 × `ffff`) | `NpcIntro`; UI: intro entries of class 148 flag := 1 | §10 |
| `29` + 96 bytes | `GameQuestFlags`; UI: `[0x007C0D47]` record loaded | §12 |
| `52` + 41 × `00` | `QuestLog`; UI: 41 status bytes 0 | §13 |
| `9b ffff 0000 0000` | `MercRevive`; UI: `[0x00725494]` = 0xFFFF, the five menu edits | `world/npc.md` §7.3 |

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
and B (0x63 × 2, 0x77), split with the S→C size rule. §4–§11 (same
session, `tools/ghidra/disasm.py` and the exports): 0x26 `0x0045DFC0`,
`0x0049F490`, `0x0049F410`, `0x0049E280`, `0x00525150`, `0x0047A170`,
`0x00622150`, `0x00661110`, `0x00661230`, `0x004A0E70`, `0x004A0A00`;
0x27 `0x0045E0A0`, `0x004A1600`, `0x004A1510`, `0x004A1320`; 0x4E / 0x4F
`0x0045E3D0`, `0x004B3240`, `0x0045E3C0`, `0x004B3290`; 0x50
`0x0045E370`, `0x004B9210` (tables `0x004B9268`, `0x004B9284` read from
the image), `0x004A28A0`, `0x004B3340`, `0x004939B0`, `0x004B25C0`,
`0x0049FF60`, `0x004A3100`, `0x00477EE0`, `0x0044B880`, `0x0044C860`;
0x58 `0x0045E490`, `0x004C0550` (table `0x004C05F8`), `0x004C0380`,
`0x0063C180`, `0x0063AAF0`; 0x8A `0x0045EA40`, `0x004B3380`,
`0x004AE130`, `0x004649D0`, `0x004B3360`; 0x91 `0x0045E580`,
`0x004B3510`; 0x78 `0x0045E810`, `0x004B9010`; 0x29 `0x0045D3A0`,
`0x004B2620`; 0x52 `0x0045CC00`, `0x004A40D0`; 0x5E `0x0045E570`,
`0x004B92B0`; 0x9B `0x0045EAC0`, `0x004B6980`; 0x28 (open question 4)
`0x0045D370`, `0x004B6DD0`. Classes 331, 534 and
object 318 from `patch_d2` `monstats.txt` / `objects.txt`. Recorded:
0x27 × 7 (A), 0x8A (A, B).

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
   r2.3. *Partly answered* (§12–§14): 0x29 writes the game record
   `[0x007C0D47]`, 0x52 and 0x5E write quest-log bytes, not the record.
   `[0x007C0D43]` is written by 0x28 (`0x0045D370` → `0x004B6DD0`: type
   6, and the NPC-interaction path with a unit, both
   `0x0065C4D0([0x007C0D43], bytes @7, 0x60, 0)`); 0x28 stays unowned:
   its NPC path also writes the model (unit flag +0xC4 bit 2, the unit's
   path `0x00648730`, a send `0x004786A0`) between UI calls, so its
   split into model part and output needs that path read in full.
5. The trade helpers `0x004B8BF0`, `0x004B85E0`, `0x004B8AD0`,
   `0x00489360`, `0x004897E0`, `0x00487B30` (trade screen; out of Phase
   0–6 single-player scope except code 0x0C's close path).
6. Code 0x10's single-player sender (stash object operate): owner
   `world/objects.md`; recorded twice.
7. Lookups keyed by UI state (0x50 code 3 and 0x8A use the interact
   NPC `[0x007C0D25]`; 0x8A's unit test uses the message key): 1.14d
   looks the unit up at receive, the UI layer at delivery; they differ
   only if a later message of the same frame adds or removes that unit.
   Either `client/bridge.md` §10 r3 gets a rule for UI-keyed lookups,
   or the interact NPC GUID moves into the model (who writes
   `[0x007C0D25]` / `[0x007C0D29]`: the NPC interaction UI, PC 2).
8. `[0x0070EE8C]` and `[0x007A0674]` (0x50 code 23): what the client
   loop does with them after the exit (`0x0044B8A0`, `0x0044DD60`,
   `0x0044E0B0`, `0x0044E200`); owner `client/model.md` §7 if they are
   session state.
9. The text filter object `0x00611560` (methods +8, +0x14) used by
   §4 r3.2: what it rejects (a squelch / profanity list?); UI spec.
