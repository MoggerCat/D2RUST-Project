# Spec: Formats — Character save, legacy loader (.d2s versions 0x47–0x5B)

- **Status:** draft: read from the 1.14d `Game.exe` legacy loader
  `0x00534020` and the functions below (disassembly; addresses inline).
  No legacy save was loaded for a check (no pre-1.09 save on this PC;
  Open questions 1–2). Answers `formats/d2s.md` Open question 1.
- **Target version:** 1.14d
- **Crate/module:** `d2-formats::d2s` (legacy framing) and `d2-server`
  character storage (load effects), as `formats/d2s.md` /
  `formats/d2s-load.md`.
- **Related specs:** `formats/d2s.md` (version dispatch §1 rule 6, the
  0x5C–0x60 loader, result codes §10), `formats/d2s-load.md` (load
  effects, §1 new character, §8 join values), `items/bitstream.md` and
  `items/generation.md` §9 (item records by save version),
  `world/quests.md` §1.6, `world/waypoints.md` §3,
  `world/hirelings.md` §10, `sim/stats.md`, `data/runtime-maps.md` §5.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 45–56 |
| Inputs | 57–65 |
| Outputs / state changes | 66–70 |
| Rules | 71–75 |
|   1. Dispatch and order (`0x00534330`, `0x00534020`) | 76–88 |
|   2. Header (130 bytes, `0x00532690`) | 89–153 |
|   3. Quests (`0x00532BE0`) | 154–160 |
|   4. Waypoints (`0x00532C70`) | 161–166 |
|   5. NPC flags (`0x00532D00`) | 167–172 |
|   6. Stats (`0x00532DA0`) | 173–185 |
|   7. Skills (`0x00532E90`) | 186–193 |
|   8. Player items (`0x005337F0`) | 194–228 |
|   9. Corpses (`0x005339A0`) | 229–237 |
|   10. Hireling (`0x00533C70`) | 238–285 |
|   11. Trailing block (`0x00533F70`) | 286–293 |
|   12. Post-load (`0x00534020` tail) | 294–300 |
| Constants & data dependencies | 301–309 |
| Randomness | 310–314 |
| Edge cases & original bugs | 315–329 |
| Test vectors | 330–349 |
| Provenance | 350–361 |
| Open questions | 362–371 |
<!-- /index -->

## Summary

`0x00534330` sends a file whose version (u32 at +4) is below 0x5C to
the legacy loader `0x00534020` (1.00–1.08 saves). It has a 130-byte
header, then the same fixed quest, waypoint and NPC sections as the
current format, a short stats bit field of 16 stats, the class skill
bytes, the player item list, the corpses, the hireling and a trailing
hotkey / weapon-swap block. It keeps no checksum and no size check.
Its return value is the **result code itself** (no internal-code
table, unlike `formats/d2s.md` §10 rule 1), and errors in the item,
corpse and hireling parts are ignored: the load still succeeds.

## Inputs

| Name | Type | Source |
|---|---|---|
| file bytes, length n | ≤ 0x2000 bytes read (`formats/d2s.md` §1 rule 3) | save file |
| game: expansion +0x70, difficulty +0x6D, type +0x6A, +0x84, map seed +0x7C, arena flags (`0x0053FD40`, `0x0053FCE0`) | game record | game |
| client: name (`0x00538830`), class +0x08, status +0x0A | client | session |
| host callbacks `[0x00883D50]` | 0 in single player | `formats/d2s.md` §2.2 |

## Outputs / state changes

A player unit with the effects below, or a result code; for a header
with the new-character bit, a new character (`formats/d2s-load.md` §1).

## Rules

All integers little-endian. "Error r" = the load returns result r
(`formats/d2s.md` §10 result column; `ui` text owner there).

### 1. Dispatch and order (`0x00534330`, `0x00534020`)

1. Fewer than 8 bytes or a bad magic → 9 (`formats/d2s.md` §10 rule
   2). Version < 0x5C → this loader; else the current one.
2. n < 0x82 → error 9. Then in order, each from the byte after the
   previous: header (§2), quests (§3), waypoints (§4), NPC (§5), stats
   (§6), skills (§7), mouse skills selected (§2 r9), player items (§8),
   corpses (§9), hireling (§10), trailing block (§11), post-load (§12).
   A failing header, quest, waypoint, NPC, stats or skills part returns
   its error at once (the unit, if already created, is not freed).
   A failing item, corpse or hireling part skips the rest of §8–§11
   and goes on with §12; the result is then 0 (success).

### 2. Header (130 bytes, `0x00532690`)

<!-- rows -->
| Offset | Size | Field | Use |
|---|---|---|---|
| 0x00 | u32 | magic 0xAA55AA55 | r1 |
| 0x04 | u32 | version | r1 |
| 0x08 | 16 | name (byte 0x17 forced to NUL) | r2 |
| 0x18 | u32 | status: bit 0 new character, 2 hardcore, 3 dead, 5 expansion, 8–12 progression, 13 weapon switch | r3–r6 |
| 0x1C | u16 | skill count | r8 check only |
| 0x1E | u16 | stat count | r8 check only |
| 0x20 | u16 | header size, must be 0x82 | r1 |
| 0x22 | u16 | class (low byte used) | r7 |
| 0x24 | 34 | not read | — |
| 0x46 | 8 × 2 | hotkeys 0–7: skill byte, flag byte | r9 |
| 0x56 | u8 | left mouse skill | r9 |
| 0x57 | u8 | right mouse skill | r9 |
| 0x58 | u8 | act (low nibble), difficulty (high nibble) | r7 |
| 0x59 | u16 | → client +0x45E | r5 |
| 0x5B | 3 | → client +0x460 (4th byte 0) | r5 |
| 0x5F | 28 | NUL-terminated text, at most 27 characters → client +0x464 | r5 |
| 0x7B | 3 | → client +0x480, +0x481, +0x482 | r5 |
| 0x7E | u32 | map seed | r7 |

1. Magic ≠ 0xAA55AA55 → error 1; header size ≠ 0x82 → error 14;
   version outside 0x47–0x60 → error 1 (single player; with host
   callbacks these three set the new-character flag instead and return
   0, or 9 for the version, `items/generation.md` §10.3).
2. Name: the client name (first 15 characters) against the header name,
   case-insensitive (`_stricmp`); different → error 1.
3. Expansion: status bit 5 set and a classic game → 24; clear and an
   expansion game → 23.
4. Hardcore (status bit 2): dead (bit 3) → 21; else the arena flags
   lack 0x800 → 20. Softcore and the arena flags have 0x800 → 19.
   Progression p = (status >> 8) & 0x1F: p < 8 and game difficulty ≥ 2
   → 18; (expansion bit and p < 5, or p < 4) and difficulty ≥ 1 → 17.
5. Client fields (meaning not traced; no d2rs rule reads them): +0x45E
   := u16 at 0x59; +0x460 := the 3 bytes at 0x5B; +0x464 := the text at
   0x5F; +0x480..+0x482 := the bytes at 0x7B..0x7D (+0x480 is the byte
   the current format stores at +0xCF, `formats/d2s.md` §2.1).
6. **New character** (status bit 0): status bit 0 cleared, client
   status := it, and the loader starts a new character
   (`0x00532590`, the same steps as `formats/d2s-load.md` §1 rule 1:
   unit, start stats, start items, `StartSkill`, quest entry mode 1);
   nothing more is read. In single player this is the only new-character
   path of a legacy file.
7. Else: client weapon switch (+0x45C) := status bit 13; client class
   := byte 0x22; client status := the status word. Act a = byte 0x58 &
   0xF, difficulty d = byte 0x58 >> 4: a ≥ 5 or d ≥ 3 → error 9. If the
   game type is 3, +0x84 = 0 and d = the game difficulty: game map seed
   := u32 at 0x7E. Client act := a when d = the game difficulty, else
   0. Then the player unit is created (type 0, class u16 at 0x22),
   set up as a new unit (`0x00532520`: mode 1, player data
   +0x50..+0x5C := 0) and its add messages sent (`0x00571F90`).
8. Counts: u16 at 0x1E > 16 or u16 at 0x1C > the `skills` row count →
   error 1 (single player). Neither count is used afterwards: the stats
   part always reads 16 stats, the skills part the class list length.
9. Skill bytes (hotkeys, mouse skills, §11): v in 0x9C–0xD8 → v + 0x41
   (156–216 → 221–281, the expansion class skills), 0xFF → −1, else v.
   Hotkey i (0–7) := (skill, flag byte, item −1) (`0x005390A0`). After
   the skills part (§7): the left skill, if non-zero, is selected for
   hand 1 and the right, if non-zero, for hand 0 (`0x005701B0`, item
   −1; each sends S→C 0x23), then player data +0x74 := left, +0x70 :=
   right.

### 3. Quests (`0x00532BE0`)

298 bytes, laid out as `formats/d2s.md` §4: u32 "Woo!", u32 6 (else
error 2; no length check), the u16 at +8 not read, three 96-byte
records copied in with normalisation mode 1 (`0x0065C4D0`,
`world/quests.md` §1.6).

### 4. Waypoints (`0x00532C70`)

80 bytes: u16 0x5357 ("WS", else error 3), then from +8 three 24-byte
records; each record's first u16 must be 0x0102, 0x0101 or 0 (else
error 3) and is copied in by `0x00661030` (`world/waypoints.md` §3).

### 5. NPC flags (`0x00532D00`)

52 bytes as `formats/d2s.md` §6: u16 0x7701 (else error 11), fields A
at +4 + 8d and B at +0x1C + 8d copied in (`0x00572540`,
`0x00572550`).

### 6. Stats (`0x00532DA0`)

1. u16 0x6667 ("gf", else error 4); then a 3-byte mask at +2; then one
   u32 per set bit of the first 16 mask bits, from +5. For stat i =
   0..15: bit i (byte +2 + (i >> 3), mask 1 << (i & 7)) set → base stat
   i := the next u32; clear → 0 (`0x00627260`). Length = 5 + 4 × (set
   bits). No bounds check against the file length.
2. Then, when the arena flags have 0x2 (`0x0053FCE0`): hitpoints (6) :=
   the maxhp total, mana (8) := the maxmana total. Always: stamina (10)
   := the maxstamina total.
3. The loader then remembers hitpoints and mana, and clamps gold (14)
   and goldbank (15) as `formats/d2s.md` §9 rule 4.

### 7. Skills (`0x00532E90`)

u16 0x6669 ("if", else error 5); then one byte per entry of the client
class's skill list (`data/runtime-maps.md` §5, in list order); a
non-zero byte b adds b base levels to that skill (`0x0056DEB0`). Then
S→C 0x94 (`0x0053C5D0`) and the skill refresh `0x0056DE40`. Length = 2 +
the list length.

### 8. Player items (`0x005337F0`)

u16 0x4D4A ("JM"), u16 count c, then c items.

1. **Versions 0x48–0x5B** (`0x005335E0`): each item is one record of
   the item reader at this save version (`0x0062AE20`, `0x00558CB0`;
   `items/bitstream.md`, `items/bitstream-legacy.md` §1–§5,
   `formats/d2s.md` Open question 2), placed into
   the player (`0x00531210`) or, failing, freed; its socketed children
   follow (count from the parent record) and go into it. A record that
   cannot be read → error 14 (ignored, §1 r2). A parent with item flag
   0x4000000 that no longer forms a runeword gets the runeword refresh
   (`0x00563470`, `formats/d2s-load.md` §6).
2. **Version 0x47** (`0x00533350`): each item starts with u16 "JM" and
   is a fixed legacy record (`0x00532F30`, `items/bitstream-legacy.md`
   §6–§8). Every record is placed (`0x00531040` for the player,
   `0x00531390` for a corpse): the list keeps a table of five
   identifying values per item for a duplicate skip, but its count
   starts at 0 (`0x005333B2`) and grows only inside the compare loop,
   which runs only when the count is above 0 (`0x005333FE`), so no
   entry is ever stored and the skip never fires. Socketed children
   (count from the placed parent) follow their parent and go into it.
   Failures:
   1. A record that does not start with "JM" → error 14 (`0x005335C2`).
   2. A top-level item of the player list that fails placement (result
      ≠ 0) is passed over: the list goes on with the next entry
      (`0x0053352F` → `0x005334EB`), so that parent's child records
      are then read as top-level entries of the count.
   3. A child item that fails placement, or any item of a corpse list
      that fails, ends the list with its code (0xC player, 0xD corpse;
      `0x005335A9`, `0x00533497`); the byte count is not written.
   4. No length check: the remaining-length word is decremented per
      record but never tested.
3. **Hotkey indices**: none; legacy hotkeys carry no item (§2 r9).

### 9. Corpses (`0x005339A0`)

u16 "JM" (else error 14), u16 count; per corpse (`0x00533850`): a
type-0 unit is created (`0x00555230` with mode argument 0x11; none →
error 8), its state 7 set (`0x00639DB0`), linked to the player (`0x0063D430`, `0x0063D470`, message
`0x0053DF80`), then 12 bytes skipped and an item list as §8 placed into
the corpse (`0x00531390` / `0x00531520`). Versions outside 0x47–0x60 →
error 1.

### 10. Hireling (`0x00533C70`)

1. **Version 0x47**: u16 "JM" (else error 14); when u32 at +2 and u16
   at +6 are both non-zero, a hireling is created (`0x005774F0` with
   name index = u16 at +6, seed = u32 at +2, `Id` 0xFFFF, 0, not dead;
   the argument order of r4). Length 8.
2. **Versions 0x48–0x5B**: first byte 0 → no hireling, length 1, no
   item list. Else fewer than 0x12 bytes left → error 14. Block: u32 at
   +2 flags (0x10000 = dead), u32 at +6 seed, u16 at +0xA name index,
   u16 at +0xC hireling `Id`, u32 at +0xE experience. The `hirelings`
   row (`Id`, level 1) of the game's mode (game +0x70) must exist, else
   error 14.
3. Versions below 0x5A: the experience is converted (`0x00533BF0`): L
   = 1 + the number of consecutive n = 2, 3, … (n ≤ 99) with F(Id, n) ×
   (n + 1) × n × 3 ≤ exp (u32), F from the table of r6; then exp :=
   max(exp, the current formula `0x00663790`(L, row(`Id`, L) +0x20))
   when that row exists.
4. Name index += the row's `NameFirst` (+0x114); above `NameLast`
   (+0x116) → `NameFirst`. The hireling is created as
   `world/hirelings.md` §10 (`0x005774F0` with name, seed, `Id`, the
   row's +8 and the dead flag); creation fails → length 0x12, success
   (the item bytes that follow are then read as §11). On success: its
   level := the largest L ≤ the maximum level (`0x00611830`) whose
   experience threshold (`0x00663790`(L, row(`Id`, L) +0x20); a missing
   row is fatal `0x1660`) ≤ exp; stat 13 := exp when larger; dead →
   the dead hireling setup (flag 0x10000, `world/hirelings.md` §10);
   then its item list (§8, at +0x12), inventory refreshes, hitpoints :=
   maxhp. Length 0x12 + the item list.
5. A failure here is ignored (§1 r2).
6. **F(Id, n)** (`0x00533A70`; signed compares):

<!-- rows -->
| `Id` | F |
|---|---|
| 0, 1 | n < 25: 100; n < 49: 200; else 400 |
| 2, 3 | n < 49: 220; else 440 |
| 4, 5, 12, 13, 14, 21, 23 | 480 |
| 6, 7, 8 | n < 31: 105; n < 55: 210; else 420 |
| 9, 10, 11 | n < 55: 230; else 460 |
| 15, 17 | n < 37: 110; n < 61: 220; else 440 |
| 16 | n < 37: 115; n < 61: 230; else 260 |
| 18, 20 | n < 61: 230; else 460 |
| 19 | n < 61: 240; else 480 |
| 22 | 500 |
| 24, 25 | n < 42: 220; n < 75: 440; else 880 |
| 26, 27 | n < 75: 460; else 920 |
| ≥ 28 | 960 |

### 11. Trailing block (`0x00533F70`)

Only when the player has a client (`0x005531C0`). Version > 0x48:
hotkeys 8–15 from 8 entries of 6 bytes (skill byte as §2 r9, flag
byte, 4 bytes not read); length 0x30. Version > 0x51 also: u32 at
+0x30 → weapon-swap left skill, u32 at +0x34 → weapon-swap right skill
(`0x00623300`, `0x00623350`, item −1); length 0x38. No error.

### 12. Post-load (`0x00534020` tail)

Hitpoints and mana := the values remembered in §6 r3; stats 67, 68, 69
:= 100; stat 30 (`nextexp`) := `0x00611800`(class, level); the client's
weapon switch set → S→C 0x97 (`0x0053E110`). Result 0. No golem item
(`kf`) and no hireling item list beyond §10 exist in this format.

## Constants & data dependencies

Markers "Woo!" 0x216F6F57 + 6, "WS" 0x5357, 0x7701, "gf" 0x6667, "if"
0x6669, "JM" 0x4D4A; header 0x82; versions 0x47–0x5B (0x47 own item
and hireling forms; > 0x48 extra hotkeys; > 0x51 swap skills; < 0x5A
hireling experience conversion); table r6 of §10. Tables: `skills`
(row count, class lists), `hirelings` (`NameFirst`, `NameLast`, +0x20),
`charstats` (`StartSkill`).

## Randomness

None in the loader; a new character's start items are
`items/generation.md` §10.3.

## Edge cases & original bugs

1. Item, corpse and hireling failures still give a successful load
   with whatever was placed so far (§1 r2).
2. No checksum and no file-size check; the stats part reads its values
   without a length check (§6 r1).
3. A hireling that fails to be created leaves its item bytes to be read
   as the trailing block (§10 r4).
4. A header that fails its count check (§2 r8) has already created and
   announced the player unit.
5. Version 0x47: a duplicate item record is placed like any other
   (the duplicate skip never fires, §8 r2); a failed top-level player
   item makes its socketed children count as top-level entries, which
   shifts the rest of the list (§8 r2.2).

## Test vectors

Synthetic (CI, the framing only):

| Input | Expected | Source |
|---|---|---|
| version 0x5C | current loader, not this one | §1 r1 |
| version 0x59, 0x81 bytes | error 9 | §1 r2 |
| header size field 0x80 | error 14 | §2 r1 |
| version 0x46 | error 1 | §2 r1 |
| status 0x20, classic game | error 24 | §2 r3 |
| status 0x0004 (hardcore), arena flags without 0x800 | error 20 | §2 r4 |
| byte 0x58 = 0x15, game difficulty 1 | act 5 → error 9 | §2 r7 |
| byte 0x58 = 0x13, game difficulty 0 | client act 0 (difficulty differs), seed not restored | §2 r7 |
| skill byte 0x9C / 0xFF / 0x24 | 221 / −1 / 36 | §2 r9 |
| stats mask `03 00 00`, values 10, 20 | stat 0 = 10, stat 1 = 20, stats 2–15 = 0; length 13 | §6 r1 |
| hireling first byte 0 | no hireling, length 1 | §10 r2 |
| version 0x59, hireling `Id` 0, exp 2,000 | F(0, 2) × 18 = 1,800 ≤ 2,000; F(0, 3) × 36 = 3,600 > 2,000 → L = 2 | §10 r3 |
| corrupt player item list, version 0x59 | load result 0, items so far kept | §1 r2 |

## Provenance

1.14d `Game.exe`, `tools/ghidra/disasm.py` of `0x00534330`,
`0x00534020`, `0x00532690` (header copy at ebp−0xA4; every field
offset from its stack slot), `0x00532590`, `0x00532520`, `0x00532BE0`,
`0x00532C70`, `0x00532D00`, `0x00532DA0`, `0x00532E90`, `0x005337F0`,
`0x005335E0`, `0x00533350`, `0x00533850`, `0x005339A0`, `0x00533C70`,
`0x00533BF0`, `0x00533A70` (switch read from the decompile), `0x00533F70`;
client setters `0x00539120`, `0x00539140`, `0x00539170`, `0x005391C0`,
`0x00539230`, `0x00538620`, `0x00538630`, `0x005382E0`; `0x0053FCE0`.
D2MOO not used.

## Open questions

1. The version-0x47 item record (`0x00532F30`, 1,055 bytes) and its
   placement (`0x00531040`, `0x00531390`): Pending; settle with a
   Ghidra read of those three functions (only 1.00–1.06 saves use it).
   **Answered**: `items/bitstream-legacy.md` §6 (record `0x00532F30`),
   §7 (request `0x00530F40`), §8 (placement `0x00531040`,
   `0x00531390`); the list itself is §8 rule 2.
2. PROVISIONAL: the rules above for 1.07/1.08 saves (version 0x57 / 0x59) (because they are read from the loader binary); settled by REC-44 (Deferred).
