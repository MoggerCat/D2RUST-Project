# Spec: World — Quests, client quest-log status meanings

- **Status:** draft: every rule below is read from the 1.14d `Game.exe`
  (disassembly of the client quest-log code; the tables are dumped from
  the image and the string ids resolved against the 1.14d English
  `string.tbl` / `patchstring.tbl` / `expansionstring.tbl`). No quest-log
  recording exists; nothing is verified against a trace.
- **Target version:** 1.14d
- **Crate/module:** `d2-client` UI layer (quest log panel; module not
  written yet). `d2-sim` needs nothing from this file: the server only
  sends the status byte (`world/quests.md` §6).
- **Related specs:** `world/quests.md` §1 (flag records and bits), §6
  (status values, S→C 0x52 / 0x5D / 0x50 senders); `client/msg-ui.md` §1
  (client 0x5D dispatch, the quest-log tail that stores a 0x5D status);
  `ui/panels.md` §2 (UI state 0x0F quest log, draw order); `ui/text.md`
  §2.2 (string-id lookup); `formats/tbl.md`; the per-act quest specs
  (`quests-act1.md` … `quests-act5-2.md`) for which status each quest
  sends.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 47–58 |
| Inputs | 59–70 |
| Outputs / state changes | 71–78 |
| Rules | 79–80 |
|   1. Client state the quest log reads | 81–112 |
|   2. The quest-log entry table `0x00723F30` | 113–141 |
|   3. Status tables and the tab build | 142–176 |
|   4. Row derivation (`0x004A1950`) | 177–225 |
|   5. Icon states (`0x004A34F0`, jump table `0x004A3E28`) | 226–252 |
|   6. Per-quest special cases (summary) | 253–282 |
|   7. Act I status tables | 283–387 |
|   8. Act II status tables | 388–496 |
|   9. Act III status tables | 497–610 |
|   10. Act IV status tables | 611–662 |
|   11. Act V status tables | 663–780 |
|   12. Client quest check `0x004A4180(c)` (level-entry lines) | 781–827 |
| Constants & data dependencies | 828–844 |
| Randomness | 845–848 |
| Edge cases & original bugs | 849–866 |
| Test vectors | 867–893 |
| Provenance | 894–908 |
| Open questions | 909–915 |
<!-- /index -->

## Summary

The server reports one status byte per quest-log slot (S→C 0x52, all
slots; S→C 0x5D, one quest). The client never shows that byte directly:
for each quest of the act tab it combines the byte with its own copy of
the player's quest flags (S→C 0x28) and of the game's quest flags (S→C
0x29), picks a status row of the quest's 15-row string table, and draws
the quest icon in one of four states. This file gives the derivation
(§4), the icon states (§5), the per-quest special cases (§6) and every
quest's table (§7–§11): status value → description string id, replay
speech string id.

## Inputs

| Name | Type | Source |
|---|---|---|
| status list `S[0..40]` | u8 × 41 at `[0x007BF356]` | S→C 0x52 bytes 1–41 (§1 rule 1); one byte rewritten by S→C 0x5D (`client/msg-ui.md` §1 r6.2, r7) |
| player record P | 96-byte flag record at `[0x007C0D43]` (`0x004B32D0`) | S→C 0x28 with unit type 6 (`0x0045D370` → `0x004B6DD0`, copy without normalize) |
| game record G | 96-byte flag record at `[0x007C0D47]` (`0x004B32E0`) | S→C 0x29 (`0x0045D3A0` → `0x004B2620`, copy without normalize) |
| counters D, Y, B | i32 `[0x007BF2A4]`, `[0x007BF2A8]`, `[0x007BF2AC]` | S→C 0x50 (§1 rule 2); D and B also from 0x5D (`client/msg-ui.md` §1 r2, r7) |
| last shown status `last[0..40]` | u8 × 41 at `[0x007BF380]` | written by §4 |
| game type | `[0x007A0610]` (0 = single player) | `sim/tick.md` |
| quest availability `A[0..36]` | u8 × 37 at `[0x007C0EA4]`, valid flag `[0x007C0ECC]` | S→C 0x5E (`client/msg-ui.md` §14); read only by §12 |

## Outputs / state changes

Per quest of the shown act tab, one quest-log row (0x26A bytes; up to 6
rows at `[0x007BF3A9]`, §3 rule 2): title string id, description text,
replay speech string id, shown status, "changed" flag, icon state. Side
effect: `last[i]`; for icon state 0, P bit 12 of the quest and one C→S
0x58 (§5 rule 1).

## Rules

### 1. Client state the quest log reads

1. **S→C 0x52** (client handler `0x0045CC00` → `0x004A40D0`): the 42
   message bytes are copied to `[0x007BF355]` (so byte 1 + i, the
   server's `list[i]`, lands at `S[i]` = `[0x007BF356 + i]`), and
   `[0x007BF2B0]` := 0. Then, when `0x00483350` (byte `[0x007BC9D7]`)
   is 0: if the quest-log latch `[0x007BF298]` = 2, `0x004A23D0` (loads
   the panel cels under `DATA\GLOBAL\ui\menu\`: `questbackground`
   `[0x007BF2D5]`, `questtabs` or, expansion installed and expansion
   game, `expquesttabs` `[0x007BF2D9]`, `questsockets` `[0x007BF2D1]`,
   `questdone` `[0x007BF2DD]`, `invps` `[0x007BF351]`, `questlast`
   `[0x007BF34D]`); then
   `0x004A3220(shown act tab [0x007C0255], 1)` (§3 rule 3).
2. **S→C 0x50** (`0x0045E370` → `0x004B9210`; the type-1 body at
   `0x004A28A0`): when u16@1 = 1: D := u16@3 (Den of Evil monsters
   left), Y := u16@5 (staff tomb, `world/quests.md` §6.2 rule 3), B :=
   u16@7 (barbarians left). Other types: not quest-log state.
3. **S→C 0x5D** writes `S[e]` of the entry e whose chain is the message's
   chain (`client/msg-ui.md` §1 r6.2) only while the quest screen (UI
   state 15) is open; the f bit 5 path writes D (or 666) and `S[1]`
   (r7). The server index i of `list[i]` is the record's filter
   (`world/quests.md` §6.2), and entry i of §2 has quest id i, so 0x52
   and 0x5D address the same byte.
4. P and G are kept exactly as received (copy-in with normalize 0,
   `world/quests.md` §1.6): bits 13, 14 and 15 are the server's live
   values. "P.b" below = bit b of P's slot q (`0x0065C310(P, q, b)`);
   "P[s].b" names another slot s.
5. Reset: `0x004A3410` (game start; also clears the counters, latch,
   selection) and `0x004A3020` set every `S[i]` and `last[i]` to 0.
6. S→C 0x4C is UnitSkillOnUnit (`client/bridge-dispatch.tsv`), not a
   quest message; no other client handler writes `S`.

### 2. The quest-log entry table `0x00723F30`

41 entries of 16 bytes; entry i: +0 enabled (u8), +1 icon index (u8,
0–26), +2 slot within the act tab (u8, 0–5), +3 act tab (u8, 0–4; 9 =
none), +4 status-table pointer (u32, 0 = none), +8 server chain (u32,
37 = none), +0xC quest id q (u32; = i for every entry). Enabled entries
(27):

| Tab | Slot 0 | Slot 1 | Slot 2 | Slot 3 | Slot 4 | Slot 5 |
|---|---|---|---|---|---|---|
| Act I | 1 Den of Evil | 2 Burial Grounds | 4 Search for Cain | 5 Forgotten Tower | 3 Tools of the Trade | 6 Sisters to the Slaughter |
| Act II | 9 Radament | 10 Horadric Staff | 11 Tainted Sun | 12 Arcane Sanctuary | 13 Summoner | 14 Seven Tombs |
| Act III | 20 Golden Bird | 19 Blade of the Old Religion | 18 Khalim's Will | 17 Lam Esen's Tome | 21 Blackened Temple | 22 Guardian |
| Act IV | 25 Fallen Angel | 27 Hell's Forge | 26 Terror's End | – | – | – |
| Act V | 35 Siege on Harrogath | 36 Rescue on Mount Arreat | 37 Prison of Ice | 38 Betrayal of Harrogath | 39 Rite of Passage | 40 Eve of Destruction |

(Numbers are entry = quest id = filter.) The icon index k names cel
file `DATA\GLOBAL\ui\menu\<name>` with name from the pointer table
`0x006DA2C8`: `a1q1`…`a1q6` (k 0–5), `a2q1`…`a2q6` (6–11), `a3q1`…`a3q6`
(12–17), `a4q1` (18), `a4q2` (19), `a4q3` (20), `a5q1`…`a5q6` (21–26);
Terror's End (entry 26) uses `a4q3` and Hell's Forge (27) `a4q2`.
Per-tab entry range for the icon loads: `0x00723F08` pairs (0, 5),
(6, 11), (12, 17), (18, 20), (21, 26) (§3 rule 3).

Disabled entries: 0, 7, 8, 15, 16, 23, 24, 28–34 (chain 37 except entry
34, which carries chain 30 with tab 4). A 0x5D for chain 30 (respec)
therefore writes `S[34]`, never shown; a 0x5D for a chain with no entry
(14, 21, 26–29, …) writes nothing (`0x004A1910` returns −1).

### 3. Status tables and the tab build

1. Each status table is 32 u16 (stride 0x40): word 0 = title string id;
   word 1 = "completed speech" (§4 C); word 2 = reward-pending step p
   (0xFFFF = none; low byte used); for status s = 1…14: word 1 + 2s =
   description text id, word 2 + 2s = replay speech id. Word 31 is 0.
   Row 0's pair is words 1–2 (never used as a status row, §4). Rows
   whose words are 3725 / 3725 are "null rows".
2. Tab draw (`0x004A34F0`, UI state 0x0F, `ui/panels.md` §2 step 4):
   the 6 row buffers are zeroed; entries are visited in table order;
   an entry is shown when its tab = the shown tab, it is enabled, its
   icon ≤ 26 and its icon cel is loaded, and fewer than 6 rows are
   built. For each, quest id q = entry +0xC is written to the row (+1)
   and §4 builds the row (`0x004A1950`, row in ESI).
3. Tab open `0x004A3220(tab, reset)`: the Act V tab (4) is built only
   when the expansion is installed (`0x00408F20`) and the game is an
   expansion game (`0x0044DCC0`). It loads the tab's icon cels, then,
   unless a slot was remembered for the tab (`[0x007BF280 + 4·tab]` ≠
   −1), runs §4 on each shown entry to choose the selected slot
   `[0x007BF2B9]`: the first entry whose row has "changed" set or icon
   state 0 wins; else (only if some entry is in state 1, 2 or 3) the tab's last
   clicked selection `[0x007BF2BD + 4·tab]` when it is ≠ −1, else the
   first entry in state 3 (if any), else none. The scan visits the 41
   entries in order and stops at the first winner. Order of effects
   (`0x004A3220`): selected := −1 first, always; a refused Act V tab
   ends there (no cels, no reset); a remembered slot is taken as the
   selection without a scan; reset ≠ 0 clears the five remembered slots
   (`[0x007BF280]` … `[0x007BF290]` := −1) only after the selection was
   made (so a remembered slot still wins on the call that resets).
4. The selected row's title (unless 3724) is drawn in the description
   pane, and its text (wrapped at 270 px, `0x00502970(0x10E)`) unless
   a speech replay runs (`[0x007BF2B3]` ≠ 0). The replay button
   (`questlast` cel, `0x004A27D0`) plays the row's replay speech through the NPC text
   panel `0x004A1320(speech, 1, 0)` unless it is 3724 or 3725.

### 4. Row derivation (`0x004A1950`)

Notation: q quest id, i its entry (= q), T its table, L = `S[i]`, T[s]
= row s (text, speech). Start: title := 3724, speech := 3724, text
empty, changed := 0, shown := L. T null → icon state 2, end.

1. **Siege (T = `0x00723D24`, entry 35) with P.1:** if L = 0, L := 4
   (shown stays the received value). Go to M (rule 7).
2. **Seven Tombs (T = `0x00723A64`, entry 14) with P.0 clear:** P.13 →
   rule 4. Else, if (P[12].0 or P[12].13) and L = 1 → X(7); else P[14].3
   → X(5); else P[14].4 → X(6); else rule 5. X(s): shown := s, title,
   text T[s].text, speech T[s].speech, last[i] := s, icon state 3
   (changed stays 0).
3. **Generic** (everything else, including Siege without P.1 and Seven
   Tombs with P.0): P.0 and P.13 → C(13, 3726); P.0 without P.13 →
   C(11, 3728); P.13 without P.0 → rule 4; neither → rule 5.
   **C(s, id):** shown := s, title, text := string id (not the table
   row), speech := word 1, last[i] := s; icon state 1 if P.12 else 0.
4. **Reward pending:** word 2 = 0xFFFF → rule 5. P.1 clear → rule 6.
   Fallen Angel (T = `0x00723C64`) → rule 5. Else s := p + 1: shown,
   title, text T[s].text, speech T[s].speech, changed := (s ≠ last[i]),
   last[i] := s, icon state 3.
5. **Reward still pending from earlier:** P.1 clear or P.15 clear →
   rule 6. Else s := 10, except q = 37: P.8 set and P.9 clear → 6, P.8
   clear → 5; q = 38: 5 if P.4 else 4. shown := s. T[s].text = 3725 →
   icon state 2, end. Else title, text, speech of row s, last[i] := s,
   icon state 1 for q = 27 (Hell's Forge), else 3.
6. L ≠ 0 → M (rule 7). L = 0 → Z (rule 8).
7. **M (the received status):** title := word 0.
   1. Den of Evil (title 3714): L = 3 or 4: D > 1 → text := T[L].text
      followed by D in decimal (`%d`, `0x00526700` append); D ≤ 1 →
      text := 3739 (`qstsa1q140`, one monster left). Other L: T[L].text.
      shown := L.
   2. Other quests: text id := 3729 + (game type ≠ 0) when (P.0, P.13,
      P.1 all clear, G present, G.13 set and q ≠ 19) or P.14 is set;
      else T[L].text, and 3727 (`qstsother`) becomes 3729 in single
      player. Rescue on Mount Arreat (title 22622) with text id 22624
      (`qstsa5q22`, a `%d` format): B = 0 → text := 3729; else text :=
      the format with B (`0x005269D0`, 300 chars).
   3. Both: changed := (L ≠ last[i]), last[i] := L, speech := T[L].speech,
      icon state 3; L = 13 → icon state 1 if P.12 else 0.
8. **Z (status 0):** G.13 set: q = 21 with P.0 clear and P.4 set →
   text 989 (`qstsa3q53`); else P.13 or P.1 → icon state 2, end; else
   text 3729 + (game type ≠ 0). G.13 clear (G absent counts as clear):
   text 3729 + (game type ≠ 0) only when G.15 is set and P.13 and P.1
   are both clear; any other case (G.15 clear, or P.13, or P.1) → icon
   state 2, end (`0x004A218C` … `0x004A220A`). Each text case:
   speech 3725, icon state 3, title stays 3724 (no title drawn).

### 5. Icon states (`0x004A34F0`, jump table `0x004A3E28`)

Row n of the tab (0–5), icon cel K of the entry, position from the slot
table `0x00723EA8` (x) / `0x00723EAC` (y), 16 bytes per slot.

1. **0, just completed:** per draw, in this order (`0x004A34F0` case
   0): (a) frame = counter `[0x007C0225 + 4n]` as it is before this
   draw's step; counter ≥ 25 → frame 24, P.12 := 1 and C→S 0x58 with
   u16 q (`0x004785B0`, `world/quests.md` §1.7); (b) the cel is drawn;
   (c) now = `GetTickCount`; stamp `[0x007C023D + 4n]` = 0 → stamp :=
   now; (d) now − stamp > 100 → stamp := now, counter += 1, and a
   counter of 1 plays UI sound 14 (`0x004B9A00`, expansion installed
   only). So the first draw only stamps, and the 0x58 goes out on the
   draw after the step that reached 25. Counters and stamps of all six
   rows are zeroed by `0x004A2220` / `0x004A2300`. `0x004A2760`
   (callers `0x004A28D0`, `0x004A3E40`) does the same set-and-send at
   once for every row in state 0.
2. **1, completed:** frame 24; while its slot is the pressed slot
   `[0x007BF2B5]`, cel `questdone` frame = icon index instead.
3. **2, not available:** frame 26; no title or text.
4. **3, in progress:** frame 0; pressed → frame 25.
5. A selection frame (`questsockets`, frame 1 for the selected slot,
   0 otherwise) is drawn over every shown row.
6. Seven Tombs: when a row has title 928 and shown status 7 and the
   selected slot is 5, the tomb symbol (cel `invps`, `[0x007BF351]`) frame Y (0–6,
   else 0) is drawn at (x + 0x6E, y − 0x6E + screen height).

### 6. Per-quest special cases (summary)

| Quest | Rule | Effect |
|---|---|---|
| A1Q1 Den of Evil | 7.1 | statuses 3, 4 append the monsters-left count D, or show 3739 when D ≤ 1 |
| A2Q6 Seven Tombs | 2, 5.6 | without P.0: rows 7 / 5 / 6 from P[12], P[14] bits 3 / 4; tomb symbol at status 7 |
| A3Q3 Blade (q 19) | 7.2 | never gets the "goal done by the game" text |
| A3Q5 Blackened Temple (q 21) | 8 | status 0 with G.13, P.0 clear, P.4 → 989 |
| A4Q1 Fallen Angel | 4 | no reward-pending row (rule 4 skipped) |
| A4Q3 Hell's Forge (q 27) | 5 | row 10 drawn with the completed icon (state 1) |
| A5Q1 Siege | 1 | P.1 → received status used, 0 read as 4 |
| A5Q2 Rescue | 7.2 | row 2's `%d` filled with B; B = 0 → 3729 |
| A5Q3 Prison of Ice (q 37) | 5 | pending row 6 / 5 / 10 by P.8, P.9 |
| A5Q4 Betrayal (q 38) | 5 | pending row 5 / 4 by P.4 |

Common string ids: 3724 `qstsxxx` (invalid quest value: placeholder,
never drawn as a title), 3725 `qstsnull` (invalid state), 3726
`qstsComplete` (quest completed), 3727 `qstsother` (another player
completed it first), 3728 `qstsprevious` (completed in a previous game),
3729 `qstsThankYouComeAgain` (cannot complete in this game; make a new
game), 3730 `qstsThankYouComeAgainMulti` (… or join a different game).
Speech ids are the NPC dialogue strings (`world/npc.md`).

Reading the tables: rows 11 and 13 are reached through the table only
by rule 7 (a received 11 or 13 while P.0 is clear); with P.0 set the
client shows 3728 / 3726 whatever the table holds (rule 3). Row 10 is
the rule-5 row. Row 0 is never a status row. Every status not listed
in a table is a null row: rule 5 makes it icon state 2; rule 7 draws
the 3725 text.

### 7. Act I status tables

#### Entry 1: A1Q1 Den of Evil

Chain 1, filter 1; tab 1 slot 0; icon `a1q1` (index 0); table `0x007237A4`; title 3714 `qstsa1q1`; completed speech (word 1) 76 `A1Q1SuccessfulAkara`; reward-pending step (word 2) 4 → status 5.

| Status | Text | Replay speech |
|---|---|---|
| 1 | 3735 `qstsa1q11` | 64 `A1Q1InitAkara` |
| 2 | 3736 `qstsa1q12` | 64 `A1Q1InitAkara` |
| 3 | 3737 `qstsa1q13` | 64 `A1Q1InitAkara` |
| 4 | 3738 `qstsa1q14` | 64 `A1Q1InitAkara` |
| 5 | 3740 `qstsa1q15` | 64 `A1Q1InitAkara` |
| 10 | 3740 `qstsa1q15` | 64 `A1Q1InitAkara` |
| 11 | 3728 `qstsprevious` | 3725 (null) |
| 12 | 3727 `qstsother` | 3725 (null) |
| 13 | 3726 `qstsComplete` | 64 `A1Q1InitAkara` |

Statuses 6, 7, 8, 9: 3725 / 3725 (null).

#### Entry 2: A1Q2 Sisters' Burial Grounds

Chain 2, filter 2; tab 1 slot 1; icon `a1q2` (index 1); table `0x007237E4`; title 3715 `qstsa1q2`; completed speech (word 1) 92 `A1Q2SuccessfulKashya`; reward-pending step (word 2) 2 → status 3.

| Status | Text | Replay speech |
|---|---|---|
| 1 | 3741 `qstsa1q21` | 81 `A1Q2InitKashya` |
| 2 | 3742 `qstsa1q22` | 81 `A1Q2InitKashya` |
| 3 | 3743 `qstsa1q23` | 81 `A1Q2InitKashya` |
| 10 | 3743 `qstsa1q23` | 81 `A1Q2InitKashya` |
| 11 | 3728 `qstsprevious` | 3725 (null) |
| 12 | 3727 `qstsother` | 3725 (null) |
| 13 | 3726 `qstsComplete` | 81 `A1Q2InitKashya` |

Statuses 4, 5, 6, 7, 8, 9: 3725 / 3725 (null).

#### Entry 3: A1Q3 Tools of the Trade

Chain 3, filter 3; tab 1 slot 4; icon `a1q3` (index 2); table `0x00723824`; title 3716 `qstsa1q3`; completed speech (word 1) 163 `A1Q3SuccessfulCharsi`; reward-pending step (word 2) none (0xFFFF).

| Status | Text | Replay speech |
|---|---|---|
| 1 | 3755 `qstsa1q31` | 146 `A1Q3InitCharsi` |
| 2 | 3756 `qstsa1q32` | 146 `A1Q3InitCharsi` |
| 3 | 3733 `qstsa1q3x` | 146 `A1Q3InitCharsi` |
| 4 | 3732 `Qstsyouarenot8` | 3725 (null) |
| 10 | 3757 `qstsa1q32b` | 163 `A1Q3SuccessfulCharsi` |
| 11 | 3728 `qstsprevious` | 3725 (null) |
| 12 | 3727 `qstsother` | 3725 (null) |
| 13 | 3726 `qstsComplete` | 146 `A1Q3InitCharsi` |

Statuses 5, 6, 7, 8, 9: 3725 / 3725 (null).

#### Entry 4: A1Q4 The Search for Cain

Chain 4, filter 4; tab 1 slot 2; icon `a1q4` (index 3); table `0x00723864`; title 3717 `qstsa1q4`; completed speech (word 1) 123 `A1Q4QuestSuccessfulCain`; reward-pending step (word 2) none (0xFFFF).

| Status | Text | Replay speech |
|---|---|---|
| 1 | 3744 `qstsa1q41` | 97 `A1Q4InitAkara` |
| 2 | 3745 `qstsa1q42` | 97 `A1Q4InitAkara` |
| 3 | 3746 `qstsa1q43` | 97 `A1Q4InitAkara` |
| 4 | 3747 `qstsa1q44` | 97 `A1Q4InitAkara` |
| 5 | 3748 `qstsa1q45` | 97 `A1Q4InitAkara` |
| 6 | 3749 `qstsa1q46` | 97 `A1Q4InitAkara` |
| 7 | 3734 `qstsa1q4x` | 97 `A1Q4InitAkara` |
| 10 | 3750 `qstsa1q46b` | 97 `A1Q4InitAkara` |
| 11 | 3728 `qstsprevious` | 3725 (null) |
| 12 | 3727 `qstsother` | 3725 (null) |
| 13 | 3726 `qstsComplete` | 97 `A1Q4InitAkara` |

Statuses 8, 9: 3725 / 3725 (null).

#### Entry 5: A1Q5 The Forgotten Tower

Chain 5, filter 5; tab 1 slot 3; icon `a1q5` (index 4); table `0x007238A4`; title 3718 `qstsa1q5`; completed speech (word 1) 127 `A1Q5InitQuestTome`; reward-pending step (word 2) none (0xFFFF).

| Status | Text | Replay speech |
|---|---|---|
| 1 | 3751 `qstsa1q51` | 127 `A1Q5InitQuestTome` |
| 2 | 3754 `qstsa1q52` | 127 `A1Q5InitQuestTome` |
| 3 | 3752 `qstsa1q51a` | 127 `A1Q5InitQuestTome` |
| 4 | 3753 `qstsa1q51b` | 127 `A1Q5InitQuestTome` |
| 11 | 3728 `qstsprevious` | 3725 (null) |
| 12 | 3727 `qstsother` | 3725 (null) |
| 13 | 3726 `qstsComplete` | 127 `A1Q5InitQuestTome` |

Statuses 5, 6, 7, 8, 9, 10: 3725 / 3725 (null).

#### Entry 6: A1Q6 Sisters to the Slaughter

Chain 6, filter 6; tab 1 slot 5; icon `a1q6` (index 5); table `0x007238E4`; title 3719 `qstsa1q6`; completed speech (word 1) 184 `A1Q6SuccessfulCain`; reward-pending step (word 2) 9 → status 10.

| Status | Text | Replay speech |
|---|---|---|
| 1 | 3758 `qstsa1q61` | 166 `A1Q6InitCain` |
| 2 | 3759 `qstsa1q62` | 166 `A1Q6InitCain` |
| 3 | 3761 `qstsa1q63` | 166 `A1Q6InitCain` |
| 10 | 3760 `qstsa1q62b` | 166 `A1Q6InitCain` |
| 11 | 3728 `qstsprevious` | 3725 (null) |
| 12 | 3727 `qstsother` | 3725 (null) |
| 13 | 3726 `qstsComplete` | 184 `A1Q6SuccessfulCain` |

Statuses 4, 5, 6, 7, 8, 9: 3725 / 3725 (null).

### 8. Act II status tables

#### Entry 9: A2Q1 Radament's Lair

Chain 8, filter 9; tab 2 slot 0; icon `a2q1` (index 6); table `0x00723924`; title 923 `qstsa2q1`; completed speech (word 1) 334 `A2Q1SuccessfulAtma`; reward-pending step (word 2) 2 → status 3.

| Status | Text | Replay speech |
|---|---|---|
| 1 | 941 `qstsa2q11` | 304 `A2Q1InitAtma` |
| 2 | 942 `qstsa2q12` | 304 `A2Q1InitAtma` |
| 3 | 943 `qstsa2q13` | 304 `A2Q1InitAtma` |
| 10 | 943 `qstsa2q13` | 3725 (null) |
| 11 | 3728 `qstsprevious` | 3725 (null) |
| 12 | 3727 `qstsother` | 3725 (null) |
| 13 | 3726 `qstsComplete` | 304 `A2Q1InitAtma` |

Statuses 4, 5, 6, 7, 8, 9: 3725 / 3725 (null).

#### Entry 10: A2Q2 The Horadric Staff

Chain 9, filter 10; tab 2 slot 1; icon `a2q2` (index 7); table `0x00723964`; title 924 `qstsa2q2`; completed speech (word 1) 339 `A2Q2SuccessfulStaffCain`; reward-pending step (word 2) none (0xFFFF).

| Status | Text | Replay speech |
|---|---|---|
| 1 | 944 `qstsa2q21` | 3725 (null) |
| 2 | 945 `qstsa2q22` | 335 `A2Q2EarlyReturnScrollCain` |
| 3 | 946 `qstsa2q23` | 338 `A2Q2EarlyReturnCubeCain` |
| 4 | 948 `qstsa2q25` | 3725 (null) |
| 5 | 947 `qstsa2q24` | 339 `A2Q2SuccessfulStaffCain` |
| 6 | 947 `qstsa2q24` | 3725 (null) |
| 9 | 3730 `qstsThankYouComeAgainMulti` | 3725 (null) |
| 10 | 947 `qstsa2q24` | 3725 (null) |
| 11 | 3728 `qstsprevious` | 3725 (null) |
| 12 | 3727 `qstsother` | 3725 (null) |
| 13 | 3726 `qstsComplete` | 339 `A2Q2SuccessfulStaffCain` |

Statuses 7, 8: 3725 / 3725 (null).

#### Entry 11: A2Q3 Tainted Sun

Chain 10, filter 11; tab 2 slot 2; icon `a2q3` (index 8); table `0x007239A4`; title 925 `qstsa2q3`; completed speech (word 1) 3725 (null); reward-pending step (word 2) none (0xFFFF).

| Status | Text | Replay speech |
|---|---|---|
| 1 | 950 `qstsa2q31a` | 3725 (null) |
| 2 | 951 `qstsa2q32` | 3725 (null) |
| 3 | 952 `qstsa2q33` | 348 `A2Q3AfterInitDrognan` |
| 10 | 952 `qstsa2q33` | 3725 (null) |
| 11 | 3728 `qstsprevious` | 3725 (null) |
| 12 | 3727 `qstsother` | 3725 (null) |
| 13 | 3726 `qstsComplete` | 3725 (null) |

Statuses 4, 5, 6, 7, 8, 9: 3725 / 3725 (null).

#### Entry 12: A2Q4 Arcane Sanctuary

Chain 11, filter 12; tab 2 slot 3; icon `a2q4` (index 9); table `0x007239E4`; title 926 `qstsa2q4`; completed speech (word 1) 3725 (null); reward-pending step (word 2) none (0xFFFF).

| Status | Text | Replay speech |
|---|---|---|
| 1 | 954 `qstsa2q41a` | 3725 (null) |
| 2 | 953 `qstsa2q41` | 373 `A2Q4InitDrognan` |
| 3 | 953 `qstsa2q41` | 377 `A2Q4AfterInitJerhyn` |
| 4 | 955 `qstsa2q42` | 377 `A2Q4AfterInitJerhyn` |
| 5 | 956 `qstsa2q43` | 396 `A2Q4SuccessfulNarrator` |
| 10 | 956 `qstsa2q43` | 396 `A2Q4SuccessfulNarrator` |
| 11 | 3728 `qstsprevious` | 396 `A2Q4SuccessfulNarrator` |
| 12 | 3727 `qstsother` | 3725 (null) |
| 13 | 3726 `qstsComplete` | 396 `A2Q4SuccessfulNarrator` |

Statuses 6, 7, 8, 9: 3725 / 3725 (null).

#### Entry 13: A2Q5 The Summoner

Chain 12, filter 13; tab 2 slot 4; icon `a2q5` (index 10); table `0x00723A24`; title 927 `qstsa2q5`; completed speech (word 1) 3725 (null); reward-pending step (word 2) none (0xFFFF).

| Status | Text | Replay speech |
|---|---|---|
| 1 | 957 `qstsa2q51` | 3725 (null) |
| 2 | 958 `qstsa2q52` | 3725 (null) |
| 3 | 3726 `qstsComplete` | 3725 (null) |
| 4 | 959 `qstsa2q53` | 3725 (null) |
| 10 | 959 `qstsa2q53` | 3725 (null) |
| 11 | 3728 `qstsprevious` | 3725 (null) |
| 12 | 3727 `qstsother` | 3725 (null) |
| 13 | 3726 `qstsComplete` | 3725 (null) |

Statuses 5, 6, 7, 8, 9: 3725 / 3725 (null).

#### Entry 14: A2Q6 The Seven Tombs

Chain 13, filter 14; tab 2 slot 5; icon `a2q6` (index 11); table `0x00723A64`; title 928 `qstsa2q6`; completed speech (word 1) 442 `A2Q6SuccessfulJerhyn`; reward-pending step (word 2) 4 → status 5.

| Status | Text | Replay speech |
|---|---|---|
| 1 | 960 `qstsa2q61` | 430 `A2Q6InitJerhyn` |
| 2 | 962 `qstsa2q62` | 431 `A2Q6AfterInitJerhyn` |
| 3 | 11030 `qstsa2q63f` | 431 `A2Q6AfterInitJerhyn` |
| 4 | 964 `qstsa2q63a` | 431 `A2Q6AfterInitJerhyn` |
| 5 | 965 `qstsa2q64` | 302 `TyraelGossip1` |
| 6 | 966 `qstsa2q65` | 442 `A2Q6SuccessfulJerhyn` |
| 7 | 961 `qstsa2q61a` | 396 `A2Q4SuccessfulNarrator` |
| 8 | 3729 `qstsThankYouComeAgain` | 3725 (null) |
| 9 | 3730 `qstsThankYouComeAgainMulti` | 3725 (null) |
| 10 | 965 `qstsa2q64` | 3725 (null) |
| 11 | 3728 `qstsprevious` | 3725 (null) |
| 12 | 3727 `qstsother` | 3725 (null) |
| 13 | 3726 `qstsComplete` | 442 `A2Q6SuccessfulJerhyn` |

### 9. Act III status tables

#### Entry 17: A3Q1 Lam Esen's Tome

Chain 15, filter 17; tab 3 slot 3; icon `a3q1` (index 12); table `0x00723AE4`; title 930 `qstsa3q1`; completed speech (word 1) 564 `A3Q1SuccessfulAlkor`; reward-pending step (word 2) none (0xFFFF).

| Status | Text | Replay speech |
|---|---|---|
| 1 | 968 `qstsa3q11` | 549 `A3Q1InitAlkor` |
| 2 | 969 `qstsa3q12` | 549 `A3Q1InitAlkor` |
| 8 | 3730 `qstsThankYouComeAgainMulti` | 3725 (null) |
| 9 | 3729 `qstsThankYouComeAgain` | 3725 (null) |
| 11 | 3728 `qstsprevious` | 3725 (null) |
| 12 | 3727 `qstsother` | 3725 (null) |
| 13 | 3726 `qstsComplete` | 3725 (null) |

Statuses 3, 4, 5, 6, 7, 10: 3725 / 3725 (null).

#### Entry 18: A3Q2 Khalim's Will

Chain 16, filter 18; tab 3 slot 2; icon `a3q2` (index 13); table `0x00723B24`; title 931 `qstsa3q2`; completed speech (word 1) 548 `A3Q2SuccessfulCain`; reward-pending step (word 2) none (0xFFFF).

| Status | Text | Replay speech |
|---|---|---|
| 1 | 970 `qstsa3q21` | 543 `A3Q2InitCain` |
| 2 | 971 `qstsa3q22` | 543 `A3Q2InitCain` |
| 3 | 972 `qstsa3q23` | 543 `A3Q2InitCain` |
| 4 | 973 `qstsa3q24` | 543 `A3Q2InitCain` |
| 5 | 974 `qstsa3q25` | 543 `A3Q2InitCain` |
| 6 | 975 `qstsa3q26` | 548 `A3Q2SuccessfulCain` |
| 7 | 976 `qstsa3q21a` | 3725 (null) |
| 11 | 3728 `qstsprevious` | 3725 (null) |
| 12 | 3727 `qstsother` | 3725 (null) |
| 13 | 3726 `qstsComplete` | 3725 (null) |

Statuses 8, 9, 10: 3725 / 3725 (null).

#### Entry 19: A3Q3 Blade of the Old Religion

Chain 17, filter 19; tab 3 slot 1; icon `a3q3` (index 14); table `0x00723B64`; title 932 `qstsa3q3`; completed speech (word 1) 587 `A3Q3SuccessfulOrmus`; reward-pending step (word 2) none (0xFFFF).

| Status | Text | Replay speech |
|---|---|---|
| 1 | 993 `qstsa3q31a` | 3725 (null) |
| 2 | 977 `qstsa3q31` | 571 `A3Q3InitHratli` |
| 3 | 978 `qstsa3q32` | 571 `A3Q3InitHratli` |
| 4 | 979 `qstsa3q33` | 571 `A3Q3InitHratli` |
| 5 | 980 `qstsa3q34` | 587 `A3Q3SuccessfulOrmus` |
| 6 | 981 `qstsa3q35` | 587 `A3Q3SuccessfulOrmus` |
| 7 | 3730 `qstsThankYouComeAgainMulti` | 3725 (null) |
| 8 | 3729 `qstsThankYouComeAgain` | 3725 (null) |
| 12 | 3728 `qstsprevious` | 3725 (null) |
| 13 | 3727 `qstsother` | 3725 (null) |
| 14 | 3726 `qstsComplete` | 587 `A3Q3SuccessfulOrmus` |

Statuses 9, 10, 11: 3725 / 3725 (null).

#### Entry 20: A3Q4 The Golden Bird

Chain 18, filter 20; tab 3 slot 0; icon `a3q4` (index 15); table `0x00723BA4`; title 933 `qstsa3q4`; completed speech (word 1) 3725 (null); reward-pending step (word 2) 4 → status 5.

| Status | Text | Replay speech |
|---|---|---|
| 1 | 982 `qstsa3q41` | 3725 (null) |
| 2 | 983 `qstsa3q42` | 527 `A3Q4Init1CainAct3` |
| 3 | 984 `qstsa3q43` | 529 `A3Q4Init2MeshifAct3` |
| 4 | 985 `qstsa3q44` | 531 `A3Q4Init3CainAct3` |
| 5 | 986 `qstsa3q45` | 534 `A3Q4AfterInitAlkor` |
| 6 | 3729 `qstsThankYouComeAgain` | 3725 (null) |
| 7 | 3730 `qstsThankYouComeAgainMulti` | 3725 (null) |
| 10 | 986 `qstsa3q45` | 534 `A3Q4AfterInitAlkor` |
| 11 | 3728 `qstsprevious` | 3725 (null) |
| 12 | 3727 `qstsother` | 3725 (null) |
| 13 | 3726 `qstsComplete` | 3725 (null) |

Statuses 8, 9: 3725 / 3725 (null).

#### Entry 21: A3Q5 The Blackened Temple

Chain 19, filter 21; tab 3 slot 4; icon `a3q5` (index 16); table `0x00723BE4`; title 934 `qstsa3q5`; completed speech (word 1) 3725 (null); reward-pending step (word 2) 3 → status 4.

| Status | Text | Replay speech |
|---|---|---|
| 1 | 994 `qstsa3q51a` | 3725 (null) |
| 2 | 987 `qstsa3q51` | 594 `A3Q5InitOrmus` |
| 3 | 988 `qstsa3q52` | 594 `A3Q5InitOrmus` |
| 4 | 989 `qstsa3q53` | 594 `A3Q5InitOrmus` |
| 10 | 989 `qstsa3q53` | 594 `A3Q5InitOrmus` |
| 11 | 3728 `qstsprevious` | 626 `A3Q5SuccessfulCainAct3` |
| 12 | 3727 `qstsother` | 3725 (null) |
| 13 | 3726 `qstsComplete` | 626 `A3Q5SuccessfulCainAct3` |

Statuses 5, 6, 7, 8, 9: 3725 / 3725 (null).

#### Entry 22: A3Q6 The Guardian

Chain 20, filter 22; tab 3 slot 5; icon `a3q6` (index 17); table `0x00723C24`; title 935 `qstsa3q6`; completed speech (word 1) 3725 (null); reward-pending step (word 2) 4 → status 5.

| Status | Text | Replay speech |
|---|---|---|
| 1 | 995 `qstsa3q61a` | 3725 (null) |
| 2 | 990 `qstsa3q61` | 628 `A3Q6InitOrmus` |
| 3 | 991 `qstsa3q62` | 628 `A3Q6InitOrmus` |
| 4 | 992 `qstsa3q63` | 628 `A3Q6InitOrmus` |
| 5 | 935 `qstsa3q6` | 628 `A3Q6InitOrmus` |
| 6 | 3729 `qstsThankYouComeAgain` | 3725 (null) |
| 7 | 3730 `qstsThankYouComeAgainMulti` | 3725 (null) |
| 10 | 3728 `qstsprevious` | 3725 (null) |
| 11 | 3728 `qstsprevious` | 3725 (null) |
| 12 | 3727 `qstsother` | 3725 (null) |
| 13 | 3726 `qstsComplete` | 3725 (null) |

Statuses 8, 9: 3725 / 3725 (null).

### 10. Act IV status tables

#### Entry 25: A4Q1 The Fallen Angel

Chain 22, filter 25; tab 4 slot 0; icon `a4q1` (index 18); table `0x00723C64`; title 937 `qstsa4q1`; completed speech (word 1) 675 `A4Q1SuccessfulIzual`; reward-pending step (word 2) none (0xFFFF).

| Status | Text | Replay speech |
|---|---|---|
| 1 | 996 `qstsa4q11` | 670 `A4Q1InitTyrael` |
| 2 | 997 `qstsa4q12` | 670 `A4Q1InitTyrael` |
| 3 | 998 `qstsa4q13a` | 670 `A4Q1InitTyrael` |
| 4 | 999 `qstsa4q13` | 675 `A4Q1SuccessfulIzual` |
| 10 | 999 `qstsa4q13` | 3725 (null) |
| 11 | 3728 `qstsprevious` | 3725 (null) |
| 12 | 3727 `qstsother` | 3725 (null) |
| 13 | 3726 `qstsComplete` | 676 `A4Q1SuccessfulTyrael` |

Statuses 5, 6, 7, 8, 9: 3725 / 3725 (null).

#### Entry 26: A4Q2 Terror's End

Chain 23, filter 26; tab 4 slot 2; icon `a4q3` (index 20); table `0x00723CA4`; title 938 `qstsa4q2`; completed speech (word 1) 3725 (null); reward-pending step (word 2) none (0xFFFF).

| Status | Text | Replay speech |
|---|---|---|
| 1 | 1004 `qstsa4q21` | 681 `A4Q2InitTyrael` |
| 2 | 1005 `qstsa4q22` | 681 `A4Q2InitTyrael` |
| 11 | 3728 `qstsprevious` | 3725 (null) |
| 12 | 3727 `qstsother` | 3725 (null) |
| 13 | 3726 `qstsComplete` | 3725 (null) |

Statuses 3, 4, 5, 6, 7, 8, 9, 10: 3725 / 3725 (null).

#### Entry 27: A4Q3 Hell's Forge

Chain 24, filter 27; tab 4 slot 1; icon `a4q2` (index 19); table `0x00723CE4`; title 939 `qstsa4q3`; completed speech (word 1) 3725 (null); reward-pending step (word 2) none (0xFFFF).

| Status | Text | Replay speech |
|---|---|---|
| 1 | 1000 `qstsa4q31` | 678 `A4Q3InitHasStoneCain` |
| 2 | 1001 `qstsa4q32` | 678 `A4Q3InitHasStoneCain` |
| 3 | 1002 `qstsa4q33` | 678 `A4Q3InitHasStoneCain` |
| 4 | 1000 `qstsa4q31` | 679 `A4Q3InitNoStoneCain` |
| 6 | 3729 `qstsThankYouComeAgain` | 3725 (null) |
| 7 | 3730 `qstsThankYouComeAgainMulti` | 3725 (null) |
| 10 | 3728 `qstsprevious` | 3725 (null) |
| 11 | 3728 `qstsprevious` | 3725 (null) |
| 12 | 3727 `qstsother` | 3725 (null) |
| 13 | 3726 `qstsComplete` | 3725 (null) |

Statuses 5, 8, 9: 3725 / 3725 (null).

### 11. Act V status tables

#### Entry 35: A5Q1 Siege on Harrogath

Chain 31, filter 35; tab 5 slot 0; icon `a5q1` (index 21); table `0x00723D24`; title 22618 `qstsa5q1`; completed speech (word 1) 3725 (null); reward-pending step (word 2) none (0xFFFF).

| Status | Text | Replay speech |
|---|---|---|
| 1 | 22619 `qsta5q11` | 20077 `A5Q1InitLarzuk` |
| 2 | 22620 `qsta5q12` | 20077 `A5Q1InitLarzuk` |
| 3 | 22621 `qsta5q13` | 20077 `A5Q1InitLarzuk` |
| 4 | 21786 `qsta5q14` | 20090 `A5Q1SuccessfulLarzuk` |
| 6 | 3729 `qstsThankYouComeAgain` | 3725 (null) |
| 7 | 3730 `qstsThankYouComeAgainMulti` | 3725 (null) |
| 10 | 21786 `qsta5q14` | 20090 `A5Q1SuccessfulLarzuk` |
| 11 | 3728 `qstsprevious` | 3725 (null) |
| 12 | 3727 `qstsother` | 3725 (null) |
| 13 | 3726 `qstsComplete` | 3725 (null) |

Statuses 5, 8, 9: 3725 / 3725 (null).

#### Entry 36: A5Q2 Rescue on Mount Arreat

Chain 32, filter 36; tab 5 slot 1; icon `a5q2` (index 22); table `0x00723D64`; title 22622 `qstsa5q2`; completed speech (word 1) 3725 (null); reward-pending step (word 2) none (0xFFFF).

| Status | Text | Replay speech |
|---|---|---|
| 1 | 22623 `qstsa5q21` | 20096 `A5Q2InitQualKehk` |
| 2 | 22624 `qstsa5q22` | 20104 `A5Q2EarlyReturnQualKehkMan` |
| 3 | 22625 `qstsa5q23` | 20096 `A5Q2InitQualKehk` |
| 4 | 22626 `qstsa5q24` | 20096 `A5Q2InitQualKehk` |
| 5 | 21789 `qstsa5q21a` | 20096 `A5Q2InitQualKehk` |
| 6 | 3729 `qstsThankYouComeAgain` | 3725 (null) |
| 7 | 3730 `qstsThankYouComeAgainMulti` | 3725 (null) |
| 10 | 22625 `qstsa5q23` | 20096 `A5Q2InitQualKehk` |
| 11 | 3728 `qstsprevious` | 3725 (null) |
| 12 | 3727 `qstsother` | 3725 (null) |
| 13 | 3726 `qstsComplete` | 3725 (null) |

Statuses 8, 9: 3725 / 3725 (null).

#### Entry 37: A5Q3 Prison of Ice

Chain 33, filter 37; tab 5 slot 2; icon `a5q3` (index 23); table `0x00723DA4`; title 22627 `qstsa5q3`; completed speech (word 1) 3725 (null); reward-pending step (word 2) none (0xFFFF).

| Status | Text | Replay speech |
|---|---|---|
| 1 | 22628 `qstsa5q31` | 20116 `A5Q3InitMalah` |
| 2 | 21788 `qstsa5q31a` | 20116 `A5Q3InitMalah` |
| 3 | 22629 `qstsa5q32` | 20116 `A5Q3InitMalah` |
| 4 | 22630 `qstsa5q33` | 20116 `A5Q3InitMalah` |
| 5 | 22631 `qstsa5q34` | 20116 `A5Q3InitMalah` |
| 6 | 22632 `qstsa5q35` | 20116 `A5Q3InitMalah` |
| 7 | 3730 `qstsThankYouComeAgainMulti` | 3725 (null) |
| 10 | 22631 `qstsa5q34` | 20116 `A5Q3InitMalah` |
| 11 | 3728 `qstsprevious` | 3725 (null) |
| 12 | 3727 `qstsother` | 3725 (null) |
| 13 | 3726 `qstsComplete` | 3725 (null) |

Statuses 8, 9: 3725 / 3725 (null).

#### Entry 38: A5Q4 Betrayal of Harrogath

Chain 34, filter 38; tab 5 slot 3; icon `a5q4` (index 24); table `0x00723DE4`; title 22633 `qstsa5q4`; completed speech (word 1) 3725 (null); reward-pending step (word 2) none (0xFFFF).

| Status | Text | Replay speech |
|---|---|---|
| 1 | 22634 `qstsa5q41` | 20137 `A5Q4InitAnya` |
| 2 | 21787 `qstsa5q42a` | 20137 `A5Q4InitAnya` |
| 3 | 22635 `qstsa5q42` | 20137 `A5Q4InitAnya` |
| 4 | 22636 `qstsa5q43` | 20137 `A5Q4InitAnya` |
| 5 | 21790 `qstsa5q43a` | 20148 `A5Q4SuccessfulAnya` |
| 6 | 3729 `qstsThankYouComeAgain` | 3725 (null) |
| 7 | 3730 `qstsThankYouComeAgainMulti` | 3725 (null) |
| 10 | 21790 `qstsa5q43a` | 3725 (null) |
| 11 | 3728 `qstsprevious` | 3725 (null) |
| 12 | 3727 `qstsother` | 3725 (null) |
| 13 | 3726 `qstsComplete` | 3725 (null) |

Statuses 8, 9: 3725 / 3725 (null).

#### Entry 39: A5Q5 Rite of Passage

Chain 35, filter 39; tab 5 slot 4; icon `a5q5` (index 25); table `0x00723E24`; title 22637 `qstsa5q5`; completed speech (word 1) 3725 (null); reward-pending step (word 2) 3 → status 4.

| Status | Text | Replay speech |
|---|---|---|
| 1 | 22638 `qstsa5q51` | 20153 `A5Q5InitQualKehk` |
| 2 | 22639 `qstsa5q52` | 20153 `A5Q5InitQualKehk` |
| 3 | 22640 `qstsa5q53` | 20002 `AncientsAct5IntroGossip1` |
| 4 | 22639 `qstsa5q52` | 20002 `AncientsAct5IntroGossip1` |
| 6 | 3729 `qstsThankYouComeAgain` | 3725 (null) |
| 7 | 3730 `qstsThankYouComeAgainMulti` | 3725 (null) |
| 10 | 22639 `qstsa5q52` | 20002 `AncientsAct5IntroGossip1` |
| 11 | 3728 `qstsprevious` | 3725 (null) |
| 12 | 3727 `qstsother` | 3725 (null) |
| 13 | 3726 `qstsComplete` | 3725 (null) |

Statuses 5, 8, 9: 3725 / 3725 (null).

#### Entry 40: A5Q6 Eve of Destruction

Chain 36, filter 40; tab 5 slot 5; icon `a5q6` (index 26); table `0x00723E64`; title 22641 `qstsa5q6`; completed speech (word 1) 3725 (null); reward-pending step (word 2) none (0xFFFF).

| Status | Text | Replay speech |
|---|---|---|
| 1 | 21792 `qstsa5q61a` | 20169 `A5Q6InitAncients` |
| 2 | 21881 `qstsa5q62b` | 20169 `A5Q6InitAncients` |
| 3 | 22643 `qstsa5q62` | 20169 `A5Q6InitAncients` |
| 4 | 22644 `qstsa5q63` | 20169 `A5Q6InitAncients` |
| 5 | 22645 `qstsa5q64` | 20169 `A5Q6InitAncients` |
| 10 | 3728 `qstsprevious` | 3725 (null) |
| 11 | 3728 `qstsprevious` | 3725 (null) |
| 12 | 3727 `qstsother` | 3725 (null) |
| 13 | 3726 `qstsComplete` | 3725 (null) |

Statuses 6, 7, 8, 9: 3725 / 3725 (null).

### 12. Client quest check `0x004A4180(c)` (level-entry lines)

The only caller is the level-entry line machine `0x004CC270`
(`0x004CC322`, `audio/environment.md` §4 r2), which passes the record's
q (record +0x28) when it is ≠ 0. `c` arrives in ECX. Result 1 = "the
quest is open and not done for this player"; 0 otherwise. Steps, first
failing test returns 0:

1. **0x5E byte:** `0x004B92E0(0, c)` reads byte c of the last S→C 0x5E
   (`[0x007C0EA4 + c]`, `client/msg-ui.md` §14). Before any 0x5E
   (`[0x007C0ECC]` = 0) it is fatal (error string 0x60); c ≥ 37 is
   fatal (0x65). The byte must be ≠ 0. The 0x5E bytes are in
   init-table row order (`world/quests.md` §3 step 5: byte r = the
   not-intro byte of row r's chain record), but c indexes them
   directly, so for c ≥ 8 the byte read is row c's (`quests.tsv`
   `index` = c), not chain c's. Reproduced.
2. **Entry:** the first entry i of §2 (i = 0…40) whose server chain
   (entry +8) = c; none → 0. q := entry +0xC (= i). q = 42 → 0 (no
   1.14d entry has it).
3. **Game record:** G must have been received (`0x004B32E0` ≠ null)
   and G.13 (slot q) must be clear.
4. **Player record:** P must have been received (`0x004B32D0` ≠ null);
   P.1, P.0 and P.14 (slot q, tested in that order) must all be clear.
5. **Den of Evil (c = 1 only):** a zeroed 0x26A-byte row with quest id
   q at +1 is built by §4 (`0x004A1950`); its shown status (+0x265)
   must be < 5. §4's side effect stands: `last[1]` is rewritten by this
   check (so the quest log's "changed" mark for Den of Evil can be
   consumed by a level entry). Other c skip this step.
6. Return 1.

Per caller value (the 11 q of `audio/environment.md` §4; c, the 0x5E
byte read, the entry whose bits are tested):

| c | 0x5E byte = not-intro of chain | Entry q tested |
|---|---|---|
| 1, 2, 3, 5, 6 | c | c (Act I) |
| 12 | 11 Arcane Sanctuary | 13 The Summoner |
| 13 | 12 The Summoner | 14 The Seven Tombs |
| 31 | 31 Siege on Harrogath | 35 Siege on Harrogath |
| 34 | 34 Betrayal of Harrogath | 38 Betrayal of Harrogath |
| 35 | 35 Rite of Passage | 39 Rite of Passage |
| 36 | 36 Eve of Destruction | 40 Eve of Destruction |

(Rows 31–36 hold chains 31–36, so the Act V lines read the matching
byte; the two Act II lines read the byte of the chain one below the
entry they test. Reproduced as read.)

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| entry table | `0x00723F30`, 41 × 16 bytes (§2) | image |
| status tables | 27 × 64 bytes, `0x007237A4` … `0x00723E64` (§7–§11) | image |
| icon names | pointer table `0x006DA2C8`, 27 names | image |
| tab entry ranges | `0x00723F08`, 5 × (first, last) | image |
| slot positions | `0x00723EA8` / `0x00723EAC`, 16 bytes per slot | image |
| strings | `string.tbl` (ids < 10000), `patchstring.tbl` (10000–19999; 11030 in Seven Tombs), `expansionstring.tbl` (≥ 20000; Act V) | `ui/text.md` §2.2 |
| client buffers | `S` `[0x007BF356]`, `last` `[0x007BF380]`, rows `[0x007BF3A9]` (6 × 0x26A), counters `[0x007BF2A4]` / `[0x007BF2A8]` / `[0x007BF2AC]` | §1 |

Row layout (0x26A bytes): +0 drawn, +1 quest id (u32), +5 title id
(u16), +7 text (UTF-16, 300 chars), +0x25F replay speech id (u16), +0x261
slot, +0x262 icon index, +0x263 changed, +0x264 valid, +0x265 shown
status, +0x266 icon state (u32).

## Randomness

None.

## Edge cases & original bugs

1. Siege's L := 4 leaves the row's shown status at the received byte
   (only the text/speech rows use 4); the shown status is read only by
   §5 rule 6.
2. Rule 5's quest-37 and quest-38 tests run independently (two `if`s);
   only one can match a given q.
3. Rescue on Mount Arreat with B = 0 shows 3729 ("cannot complete in
   this game"), even in multiplayer.
4. Blade of the Old Religion's table is shifted by one from row 11 on
   (12 = 3728, 13 = 3727, 14 = 3726); since a rewarded player takes rule
   3, a received 11 / 13 reaches the table only with P.0 clear.
5. Text 3727 is replaced by 3729 only in single player and only in rule
   7.2 (not for Den of Evil, rule 7.1).
6. A 0x5D for chain 30 (respec) writes the never-shown `S[34]` (§2).
7. A status ≥ 15 would read past the 32-word table (no 1.14d sender
   sends one, `world/quests.md` §6.1).

## Test vectors

Synthetic (computed from §4 and the image tables; no recording).
"P{q: w}" = slot q's word; G empty unless given; last = 0; single player.

| Input | Expected row | Rule |
|---|---|---|
| q 1, L 1, P {} | title 3714, text 3735, speech 64, changed 1, state 3 | 7 |
| q 1, L 4, P {1: 0x0004}, D 3 | text 3738 + "3", shown 4, state 3 | 7.1 |
| q 1, L 4, P {1: 0x0004}, D 1 | text 3739 | 7.1 |
| q 1, L 0, P {1: 0x2001} | shown 13, text 3726, speech 76, state 0 (then C→S 0x58 q 1 after 25 frames) | 3, §5.1 |
| q 2, L 0, P {2: 0x2002} | shown 3, text 3743, speech 81, changed 1, state 3 | 4 |
| q 14, L 1, P {12: 0x0001} | shown 7, text 961, speech 396, state 3 | 2 |
| q 36, L 2, B 3 | text = 22624 formatted with 3, speech 20104, state 3 | 7.2 |
| q 38, L 0, P {38: 0x8012} | shown 5, text 21790, speech 20148, state 3 | 5 |
| q 27, L 0, P {27: 0x8002} | shown 10, text 3728, state 1 | 5 |
| q 3, L 0, G {3: 0x2000}, multiplayer | title 3724, text 3730, speech 3725, state 3 | 8 |
| q 35, L 0, P {35: 0x0002} | text 21786, speech 20090, shown 0, state 3 | 1 |
| q 5, L 12 | text 3729 (3727 replaced), state 3 | 7.2 |
| q 9, L 0, P {}, G {} | state 2 | 8 |
| §12: c 1, 0x5E byte 1 = 1, P {1: 0x0004}, L 1, G {} | 1 (row shown 1 < 5) | 12 |
| §12: c 1, 0x5E byte 1 = 1, P {1: 0x0004}, L 5 | 0 (shown 5) | 12 |
| §12: c 2, 0x5E byte 2 = 0 | 0 | 12 r1 |
| §12: c 12, 0x5E byte 12 = 1, P {13: 0x0001} | 0 (Summoner's P.0) | 12 r4 |
| §12: c 34, byte 34 = 1, G {38: 0x2000} | 0 (G.13) | 12 r3 |
| §12: c 5, byte 5 = 1, P {5: 0x4000} | 0 (P.14) | 12 r4 |

## Provenance

1.14d `Game.exe`, read with `tools/ghidra/disasm.py` and the Ghidra
exports (spec session 2026-10-07): 0x52 `0x0045CC00` → `0x004A40D0`;
0x50 `0x0045E370` → `0x004B9210` (`0x004A28A0`); 0x28 `0x0045D370` →
`0x004B6DD0`; 0x29 `0x0045D3A0` → `0x004B2620`; records `0x004B32D0`,
`0x004B32E0`; row build `0x004A1950` (register use and string ids from
the disassembly: the export drops the ECX string id of `0x00524A30`);
tab build `0x004A3220`; draw `0x004A34F0`; cels `0x004A23D0`; replay
`0x004A27D0`; acknowledge `0x004A2760`; chain lookup `0x004A1910`;
resets `0x004A3020`, `0x004A3410`; quest check `0x004A4180` (caller `0x004CC322`), 0x5E byte read `0x004B92E0` (copy `0x004B92B0`). Tables `0x00723F30`, the 27 status
tables, `0x006DA2C8`, `0x00723F08` dumped from the image with a script
outside the repo; string keys from the 1.14d English `.tbl` files
(`d2data.mpq`, `d2exp.mpq`, `Patch_D2.mpq`). D2MOO not used.

## Open questions

1. ~~Confirm with a recording of one quest-log open per state (e.g. Den of
   Evil: started, D = 3, rewarded now, rewarded earlier): the drawn
   text, icon frame and the C→S 0x58 after the completion animation.~~
   Needs recording: R-PQ-14 (`docs/handoff/pc2-rec-pc2-quests.md`).
