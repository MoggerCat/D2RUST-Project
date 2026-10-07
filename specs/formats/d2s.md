# Spec: Formats — Character save (.d2s, version 0x60)

- **Status:** draft: every layout, check and error path read from the
  1.14d `Game.exe` save writer and loaders (addresses below);
  `itemstatcost` values measured on the 1.14d `patch_d2` table.
  `tools/d2s_check.py` confirms the layout on 9 real 1.14d saves (7
  fresh level-1 expansion characters, one per class, and 2 played
  level-1 characters) and a new-character stub (§1 rule 7): magic,
  version, size, checksum, every section marker and offset, the stats
  stream, the skill bytes, every item record's length and the file end
  all match; no layout mismatch was found. One binary reading was
  corrected (§6 rule 3: field A has callers). The waypoint section
  also matches the saves measured for `world/waypoints.md`. Still
  unmeasured: a hireling's items, an Iron Golem
  item (Open questions). A corpse was measured on the final `bdDead`
  save (§8.3 rules 6–7). A live hired rogue's header block and its
  empty `jf` list were measured on `bdMercTwo`, and a Clay Golem's
  empty `kf` on `bdGolem` (§2.5 rule 3, §8.4 rule 5, §8.5 rule 4).
  Classic saves and game re-saves of d2rs-generated files were
  measured in the C66 run (Open question 3; §2.8, §8.2 rule 7, §9
  rule 6).
- **Target version:** 1.14d
- **Crate/module:** `d2-formats::d2s` (byte layout, checksum, section
  framing); the load effects (§9) belong to `d2-server` character
  storage.
- **Related specs:** `formats/d2s-appearance.md` (header +0x88..+0xA7, §2.8); `formats/d2s-load.md` (new-character start and load
  effects, §9 rules 6–7); `items/bitstream.md` (one item record, the
  per-item `JM` marker and socketed children); `world/quests.md` §1
  (quest flag records, load normalisation §1.6, NPC intro bits §6.7);
  `world/waypoints.md` §2–§3 (waypoint records and the `WS` section
  bytes); `world/hirelings.md` §10 (what the loader does with the
  hireling fields); `sim/stats.md`, `sim/stat-lists.md` (stat values,
  base list order); `data/runtime-maps.md` §5 (class skill lists);
  `data/fields.tsv` (`itemstatcost` offsets); `sim/rng.md` §5.4 (map
  seed); `sim/tick.md` §6.3 (autosave timing);
  `sim/server-messages.tsv` 0xB3, `sim/client-messages.tsv` 0x6C
  (save transfer).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 64–78 |
| Inputs | 79–88 |
| Outputs / state changes | 89–94 |
| Rules | 95–96 |
|   1. File layout and framing | 97–142 |
|   2. Header (335 bytes) | 143–377 |
|   3. Checksum (`0x00411130`) | 378–388 |
|   4. Quest section (298 bytes at 0x14F) | 389–409 |
|   5. Waypoint section (80 bytes at 0x279) | 410–415 |
|   6. NPC flag section (52 bytes at 0x2C9) | 416–461 |
|   7. Stats and skills | 462–554 |
|   8. Item sections | 555–739 |
|   9. Load sequence (`0x0056B180`) | 740–764 |
|   10. Errors | 765–811 |
| Constants & data dependencies | 812–831 |
| Randomness | 832–836 |
| Edge cases & original bugs | 837–903 |
| Test vectors | 904–943 |
| Provenance | 944–1023 |
| Open questions | 1024–1149 |
<!-- /index -->

## Summary

A `.d2s` file holds one character: a fixed 335-byte header, then
fixed-size quest, waypoint and NPC sections, then variable sections
(stats bit field, skill levels, the player's item list, the corpse, and
in an expansion game the hireling's items and the Iron Golem's item).
All integers are little-endian. The game writes version 0x60 (96) and
loads 0x5C–0x60 through the code in this spec; older versions go to a
legacy loader that is not specified. The file is at most 8,192 bytes.
A 32-bit rotate-and-add checksum over the whole file (checksum field
zeroed) and the file size in the header are both checked on load. This
spec owns the byte layout and the loader's checks; the meaning of
quest bits, waypoint indices, hireling fields and item records lives
in the specs listed above.

## Inputs

| Name | Type | Source |
|---|---|---|
| file bytes | ≤ 8,192 bytes | `<save dir><name>.d2s` (format `%s%s.d2s`, `0x006D4230`) |
| `itemstatcost` `CSvBits` (u8 +10), `CSvParam` (u8 +11), `CSvSigned` (bit 13 of u32 +4); record size 0x144 | per stat | `data/fields.tsv` |
| class skill lists and the largest class skill count | u16 lists, 30 | `data/runtime-maps.md` §5 |
| game: expansion (+0x70), ladder (+0x74), difficulty (+0x6D), game type (+0x6A), +0x84, map seed (+0x7C) | game record | game |
| client record: name (+0x0D), class (+0x08), status word (+0x0A), act (+0x1AC), create time (+0x488), weapon switch (+0x45C), hotkeys (+0x3DC, 16 × 8 bytes), byte +0x480 | client | session |

## Outputs / state changes

- Write: the file bytes (§1–§8). Read: a player unit with stats,
  skills, quest/waypoint/NPC records, items, possibly a corpse, a
  hireling and an Iron Golem item (§9), or an error code (§10).

## Rules

### 1. File layout and framing

1. Sections in file order:

   | Offset | Size | Section | Marker bytes | Writer | Reader |
   |---|---|---|---|---|---|
   | 0x000 | 335 | header (§2) | `55 AA 55 AA` | `0x00568F20` | `0x0056A090` |
   | 0x14F | 298 | quests (§4) | `57 6F 6F 21` ("Woo!") | `0x00569320` | `0x0056A370` |
   | 0x279 | 80 | waypoints (§5) | `57 53` ("WS") | `0x005693E0` | `0x0056A3E0` |
   | 0x2C9 | 52 | NPC flags (§6) | `01 77` | `0x00569490` | `0x0056A470` |
   | 0x2FD | var | stats (§7.1) | `67 66` ("gf") | `0x00569540` | `0x0056A620` |
   | — | 2 + 30 | skills (§7.2) | `69 66` ("if") | `0x005696F0` | `0x0056A710` |
   | — | var | player item list (§8.1) | `4A 4D` ("JM") | `0x005697B0` | `0x0056A7E0` |
   | — | var | corpse (§8.3) | `4A 4D` | `0x005697F0` | `0x0056A830` |
   | — | var | hireling items, expansion only (§8.4) | `6A 66` ("jf") | `0x005699A0` | `0x0056AC10` |
   | — | var | Iron Golem item, expansion only (§8.5) | `6B 66` ("kf") | `0x00569A00` | `0x0056AE50` |

   Markers are u16 or u32 values written little-endian (`gf` is u16
   0x6667). The writer master is `0x00569AD0`, the loader master
   `0x0056B180`; both run the sections in this order.
2. "Expansion only" means the game is an expansion game (game +0x70 ≠
   0); a classic game neither writes nor reads `jf` / `kf`.
3. The writer fills a caller buffer of 0x2000 bytes (`0x00532240`,
   `0x00532340` and `0x00531EB0` pass 0x2000; `0x00531D30` forwards its
   caller's size; overflow: §8.1 rule 5). Every loader reads at most 0x2000 bytes of
   the file (`fread` limit in `0x005343A0`, `0x0043C8A0`).
4. After all sections, the writer stores the total length at +0x08
   and then the checksum (§3) at +0x0C (`0x00569AD0`).
5. Nothing follows the last section. The loader does not require the
   cursor to reach the end after the last section (§10 rule 6).
6. Version dispatch (`0x00534330`): fewer than 8 bytes, or u32 at 0 ≠
   0xAA55AA55 → result 9. Version (u32 at +4) < 0x5C → legacy loader
   `0x00534020` (needs ≥ 0x82 bytes; not specified, Open question 1).
   Otherwise the loader of this spec, which rejects versions > 0x60
   (§2.2 rule 3).
7. Measured (`tools/d2s_check.py`, Provenance): in every 1.14d save of
   this PC, magic, version 0x60, size field = file length and the §3
   checksum match; `Woo!`, `WS` and `01 77` sit at 0x14F, 0x279 and
   0x2C9 and `gf` at 0x2FD; `if` starts at the byte after the stats
   stream's last bit; the player list, corpse header, `jf` and `kf`
   follow without gaps, and the `kf` byte is the file's last byte.
   A level-1 character that has never left the start camp is 958
   bytes (one starting hand item) or 980–981 bytes (two; the shield's
   defence roll changes nothing in the length, the weapon's record
   does).

### 2. Header (335 bytes)

#### 2.1 Fields

Written by `0x00568F20` into a zeroed 0x14F-byte block:

| Offset | Size | Field | Written value |
|---|---|---|---|
| 0x00 | u32 | magic | 0xAA55AA55 |
| 0x04 | u32 | version | 0x60 |
| 0x08 | u32 | file size | total length (§1 rule 4) |
| 0x0C | u32 | checksum | §3 |
| 0x10 | u32 | weapon switch | bit 0 = client +0x45C ≠ 0; other bits 0 |
| 0x14 | 16 | name | client name (+0x0D), NUL-terminated; the rest zero |
| 0x24 | u16 | status | client status word (+0x0A), OR 0x20 if the game is expansion, OR 0x40 if game +0x74 ≠ 0 (§2.3) |
| 0x26 | u16 | — | 0 |
| 0x28 | u8 | class | client class (0–6) |
| 0x29 | u8 | stat count | 0x10 (used only by the 0x5C–0x5E stats layout, §7.1 rule 7) |
| 0x2A | u8 | skill count | the largest class skill count (global +0xBA8; 30 in 1.14d) |
| 0x2B | u8 | level | stat 12 (`level`) unit total (`0x00625480`), low byte |
| 0x2C | u32 | create time | client +0x488 |
| 0x30 | u32 | save time | `time(NULL)` at the write |
| 0x34 | u32 | — | 0xFFFFFFFF (`0x005385C0` returns −1) |
| 0x38 | 16 × 4 | hotkeys | §2.4 |
| 0x78 | 2 + 2 | left skill, its item index | §2.4 |
| 0x7C | 2 + 2 | right skill, its item index | §2.4 |
| 0x80 | 2 + 2 | weapon-swap left skill, item index | §2.4 |
| 0x84 | 2 + 2 | weapon-swap right skill, item index | §2.4 |
| 0x88 | 16 | appearance components | pre-filled 0xFF, then filled by `0x0063E510` |
| 0x98 | 16 | appearance colours | pre-filled 0xFF, then filled by `0x0063E510` |
| 0xA8 | 3 | town per difficulty | byte [game difficulty] = client act (0–4) OR 0x80; the other two bytes 0 |
| 0xAB | u32 | map seed | game +0x7C |
| 0xAF | 32 | hireling block | §2.5 |
| 0xCF | u8 | client byte | client +0x480 (meaning not traced; D2MOO calls it guild emblem colour; Open question 7) |
| 0xD0 | 127 | — | 0 |

The writer returns without writing when the player has no client or
the name has no NUL inside its buffer (`0x00568D80`), or when fewer
than 0x14F bytes remain.

Measured on the fresh level-1 expansion saves of all seven classes
(Provenance): +0x10 = 0; status 0x0020; +0x26 = 0; +0x29 = 0x10;
+0x2A = 30; +0x2B = 1; +0x2C = 0 (§2.2 rule 10); +0x34 = 0xFFFFFFFF;
hotkeys and mouse skills as §2.4 rule 7; components +0x88..+0x97 all
0xFF except +0x8D (the right-hand weapon: Amazon 0x1B, Sorceress 0x25,
Necromancer 0x09, Paladin 0x11, Barbarian 0x04, Druid 0x0C, Assassin
0x2D) and +0x8F (0x4F with the starting buckler; 0xFF for Sorceress and
Necromancer); colours +0x98..+0xA7 all 0xFF; +0xA8..+0xAA = `80 00 00`
(act 0 of Normal); map seed non-zero and different per save; hireling
block zero; +0xCF = 0; +0xD0..+0x14E zero.

#### 2.2 Header checks on load (`0x0056A090`)

In this order; codes are internal (§10):

1. Fewer than 0x14F bytes in the buffer → 4.
2. Checksum: save the u32 at +0x0C, write 0 there **in the buffer**,
   compute §3 over all bytes read; differs → 6. Then the bytes read ≠
   u32 at +0x08 → 5.
3. Version not in 0x5C..0x60 → 7.
4. Byte +0x23 := 0 (the name is at most 15 characters); the client has
   no name → 3; name at +0x14 ≠ client name (case-insensitive
   `_stricmp`) → 7.
5. Status checks (`0x00569D80`, on the u32 at +0x24), in order:
   1. status & 0x20 and the game is classic → 8; status lacks 0x20 and
      the game is expansion → 9.
   2. Only when the service object `[0x00883D54]` exists (it supplies
      a FILETIME through its vtable +0x54): if status & 0x40 and the
      FILETIME at +400 of the loader's third argument is later than
      it: game +0x74 = 0 → 0x19, else pass. Otherwise game +0x74 ≠ 0 →
      0x1A. Single player has no such object (Open question 8).
   3. status & 0x04 (hardcore): status & 0x08 (dead) → 10; game not
      hardcore (`0x0053FD40` & 0x800 = 0) → 11. Status lacks 0x04 and
      the game is hardcore → 12.
   4. p = (status >> 8) & 0x1F (progression). Nightmare (difficulty
      1): p < 5 for an expansion character, or p < 4 → 13. Hell
      (difficulty ≥ 2): p < 10 for expansion, or p < 8 → 14.
6. Class byte +0x28 > 7 → 4. (Class 7 passes here; §7.2 rule 4.)
7. Client class := +0x28. Status bit 0 set (new character, §2.6) →
   clear it in the buffer, client status := the cleared word, cursor
   := 0x14F, result 2 (§9 rule 1).
8. Otherwise: client weapon switch := +0x10 & 1; client status := u16
   +0x24; client create time := +0x2C; t = byte +0xA8 + game
   difficulty; client act := t & 0x7F, or 0 when that is ≥ 5. If t &
   0x80, game +0x6A = 3 and game +0x84 = 0: game +0x7C := u32 +0xAB
   (the map seed of `sim/rng.md` §5.4; Open question 9).
9. The player unit is created, hotkeys and mouse skills are decoded
   (§2.4), client +0x480 := +0xCF, +0x481 := 0, +0x482 := 0; cursor :=
   0x14F; result 0.
10. The client create time is set only by rule 8 (its one setter,
    `0x00538760`, has `0x0056A090` as its only caller). Rule 7 returns
    before it and the new-character start (`0x00569F80`) does not set
    it, so the stub's +0x2C (§2.6) is lost: the first in-game save
    writes +0x2C = 0, and every later load reads 0 back. Confirmed on
    every non-stub save of this PC (all +0x2C = 0).

Not read on load: +0x29, +0x2B, +0x30, +0x34, +0x88..+0xA7 (the
character-select screen reads them, §2.7), +0xD0.. .

#### 2.3 Status word (+0x24)

| Bit | Meaning | Set by |
|---|---|---|
| 0x0001 | new character: file is the 335-byte stub (§2.6) | client at creation; cleared on first load |
| 0x0002 | set at creation when the creation screen's flag argument ≠ 0 (`0x0043C6A0`; D2MOO: realm character) | client |
| 0x0004 | hardcore | creation screen |
| 0x0008 | dead (hardcore) | game |
| 0x0020 | expansion character | creation (always for Druid 5 / Assassin 6), convert (§2.7), writer in an expansion game |
| 0x0040 | ladder | writer when game +0x74 ≠ 0 |
| bits 8–12 | progression (acts completed; §2.2 rule 5.4 thresholds) | game |

Bits 0x10, 0x80 and 13–15 are kept as found (no rule reads them).

Measured: bit 0x0008 is also set for a softcore character that died
and respawned in town (status 0x0028 in the save, §8.3 rule 6); the
writer copies the status from the client record (`0x00538640`) and
only ORs 0x20 / 0x40, so 0x08 is set in that record by the game on a
softcore death too (the setter is not traced).

#### 2.4 Hotkeys and mouse skills

1. Hotkey i (0..15) at 0x38 + 4i: u16 code, u16 item index. Writer
   (`0x005390D0` gives skill s, left flag L, item GUID g): s > 0x7FFF
   → fatal assert. code = (s & 0xFFFF) | (0x8000 if L ≠ 0). So "no
   skill" (s = −1) is 0xFFFF.
2. Item index (`0x00568DC0`): g = −1 or the player has no inventory →
   0. Else the 1-based position of the item with GUID g in the
   player's inventory item list (link order, `items/inventory.md`
   §1.4); not found → 0; position > 0x7FFE → fatal assert.
3. Mouse skills (0x78, 0x7C, 0x80, 0x84): u16 skill id of the skill
   entry, u16 item index of that skill's owner item (rule 2). Left
   (0x78) only when the player has a left skill (else both words 0);
   right always; the swap pair from `0x00623220` / `0x00623290`.
4. Decode (`0x00569EC0`), for hotkeys and mouse skills: code 0xFFFF →
   skill −1, left 0, item −1. Else skill = code & 0x0FFF, left = code
   >> 15, item = the index word, 0 → −1.
5. Hotkeys go to the client (`0x005390A0`); mouse skills to player
   data: left skill/item +0x74/+0x7C, right +0x70/+0x78, swap left
   +0x84/+0x8C, swap right +0x80/+0x88.
6. After the items load (§9 rule 3), every non-zero item index is
   turned into the GUID of the item at that 1-based position of the
   inventory item list (`0x0056AF20`; past the end → −1), and the left
   and right skills are selected with their item (`0x005701B0`).
7. Measured on the fresh saves: all 16 hotkeys `FF FF 00 00`; all four
   mouse pairs `00 00 00 00` (left: no left skill, rule 3; right:
   skill 0, no item). A Sorceress that selected Fire Bolt as right skill
   (base level 0; +1 from the starting staff) has right = `24 00 00
   00` (skill 36, item index 0).

#### 2.5 Hireling block (+0xAF, 32 bytes)

| Offset | Size | Field |
|---|---|---|
| 0xAF | u32 | flags: 0x10000 = dead; other bits 0 |
| 0xB3 | u32 | seed |
| 0xB7 | u16 | name index (name id − the row's `NameFirst`) |
| 0xB9 | u16 | `Id` (hireling row id) |
| 0xBB | u32 | experience (stat 13) |
| 0xBF | 16 | 0 |

1. Written (`0x00568E60`) only when the player has a hireling pet node
   (type 7, `0x00574EC0(7, 1)`) whose hireling row is found
   (`0x006562F0`); else the block stays zero. Dead (`0x005541B0`) →
   flags = 0x10000.
2. Read (`0x0056AA50`) from a copy of all 32 bytes: no hireling when
   seed, name index and experience are all 0. Behaviour:
   `world/hirelings.md` §10. Written and read in classic and
   expansion games alike; only the hireling's items (§8.4) are
   expansion-only.
3. Measured on `bdMercTwo` (Provenance; a live Act I rogue hired from
   Kashya, hireling class 271): flags = 0; seed non-zero and equal to
   the seed of that hireling's S→C 0x81 message; name index 21 (name
   "Diane"); `Id` = 0 (the first `hireling.txt` row: Act I Rogue Scout,
   subtype Fire - Normal, class 271); experience = 39,482; +0xBF..+0xCE all 0. The block
   holds no level, life or GUID (the 0x81 GUID is not saved).

#### 2.6 New-character stub

The character-creation screen (`0x0043C540`, file written by
`0x0043C6B0`) writes a file of exactly 0x14F bytes: a zeroed header
with magic, version 0x60, size 0x14F, name, status = creation flags |
0x0001 (| 0x20 for class 5 or 6), class, +0x29 = 0x10, +0x2A = 0x1E,
+0x2B = 1, +0x2C = +0x30 = `time(NULL)`, +0x88..+0x97 = the 16 bytes at
`0x0070CCC8` (`01 01 01 01 01 FF FF FF 01 01 FF FF FF FF FF FF`),
+0x98..+0xA7 = 0xFF, and the checksum of those 335 bytes (computed
with +0x0C = 0) at +0x0C.

Confirmed on a real stub (an expansion Barbarian, read between
creation and the first in-game save): size 335, checksum matches,
+0x2C = +0x30, the 16 component bytes above, every other byte zero
(so +0x34 = 0, hotkeys `00 00 00 00`, town bytes and map seed 0).
Its status is 0x0021: an expansion character of a classic class gets
0x20 from the creation flags, not only classes 5 and 6. The stub stays
on disk until the game's first save of that character, which rewrites
the whole file (the same character's next file was 985 bytes; a second
character's file was also 335 bytes before its first save).

#### 2.7 Client-side header users

1. Character select (`0x0043C8A0`): accepts ≥ 0x14F bytes with
   version 0x5C..0x60 (legacy: ≥ 0x82 bytes and version ≥ 0x47);
   reads class +0x28, level +0x2B, status +0x24, 11 component bytes
   +0x88 and 11 colour bytes +0x98. No checksum check.
2. Convert to expansion (`0x0043CAA0`, from `0x0043B660`): reads ≤
   0x2000 bytes; version ≥ 0x5C and ≥ 0x14F bytes → status |= 0x20,
   +0x0C := 0, +0x0C := §3 over the bytes read, rewrite the file.
3. Saving received data (`0x0045C520`, S→C 0xB3 `DownloadSave`):
   magic and ≥ 0x14F bytes → keep u16 +0x24 in the client, write the
   received bytes unchanged as the file.

#### 2.8 Appearance bytes are rebuilt on every save

1. The writer (`0x00568F20`, `0x0056923A`–`0x00569269`) fills all 32
   bytes +0x88..+0xA7 with 0xFF and then calls `0x0063E510`(player,
   components +0x88, colours +0x98). That function walks the player's
   inventory item list and changes bytes only for items in mode 1
   (equipped, unit +0x10 = 1); every other byte stays 0xFF. Nothing
   else writes these 32 bytes, and the loader does not read them
   (§2.2), so the bytes a file holds before a load never reach the next
   save: they are a function of the items equipped at save time.
2. So a character with no equipped item saves 32 × 0xFF, whatever its
   file held before (the stub's `01` fill of §2.6 included). Measured:
   two generated characters (`TestAma`, `TestSor`, no equipped item)
   whose files held the stub's 16 component bytes were re-saved by the
   game with all 16 components 0xFF; `bdDead` (weapon on the corpse,
   nothing equipped) has all 0xFF; the fresh saves have bytes only at
   the right-hand (+0x8D) and shield (+0x8F) slots (§2.1).
3. A writer that builds a save of a loaded character must therefore
   recompute these bytes from the equipped items, not copy them from
   the loaded file. The per-item byte mapping (`0x0063DA70` and the
   composite branch of `0x0063E510`) is Open question 17.
4. The per-item mapping (token table, hand owners, body armour parts,
   colour byte) is `formats/d2s-appearance.md` (Open question 17,
   answered).

### 3. Checksum (`0x00411130`)

1. s = 0 (u32). For each byte b of the file, in order: s := rotl(s, 1)
   + b, modulo 2^32. (rotl by 1 = (s << 1) | (s >> 31); the code adds
   the old sign bit as a carry, which is the same value.)
2. The writer computes it with +0x0C = 0 and +0x08 already holding the
   size, over exactly the file's bytes, and stores s at +0x0C.
3. The loader zeroes +0x0C in its buffer before computing (§2.2 rule
   2), so a stored checksum never covers itself.
4. The buffer from the writer is the file: no padding.

### 4. Quest section (298 bytes at 0x14F)

| Offset | Size | Field |
|---|---|---|
| +0 | u32 | 0x216F6F57 (`57 6F 6F 21`, "Woo!") |
| +4 | u32 | 6 |
| +8 | u16 | 0x012A (section size) |
| +10 + 96d | 96 | quest flag record of difficulty d = 0, 1, 2 |

1. Write: zeroed, header, each record copied out unchanged
   (`0x0065C560`); fewer than 298 bytes left → writer error 1.
2. Read: fewer than 298 bytes left, or the u32s at +0 / +4 not
   0x216F6F57 / 6 → 15. The size at +8 is not read. Each record is
   copied in with normalisation (`world/quests.md` §1.6).
3. Record contents: `world/quests.md` §1 (slot q = bytes 2q, 2q+1).
4. Which bits a completed quest leaves in its slot (what a save of a
   character that finished a quest holds, and so what a save editor
   must write for "completed") is defined in `specs/world/quests.md`
   (quests-core owner), not here; this section only carries the 96
   bytes per difficulty.

### 5. Waypoint section (80 bytes at 0x279)

Byte layout, write and read rules: `world/waypoints.md` §3 (header
`57 53`, u32 1, u16 0x50, then 3 × {16-byte record, 8 zero bytes}).
Read failures are internal code 16.

### 6. NPC flag section (52 bytes at 0x2C9)

| Offset | Size | Field |
|---|---|---|
| +0 | u16 | 0x7701 (bytes `01 77`) |
| +2 | u16 | 0x0034 (section size) |
| +4 + 8d | 8 | difficulty d (0..2): NPC bit field A |
| +0x1C + 8d | 8 | difficulty d (0..2): NPC bit field B |

1. Each player has per difficulty an NPC record (player data +0x60 +
   4d) pointing to two 8-byte bit fields, A (record +0) and B (record
   +4). Writer `0x00569490` copies A and B of each difficulty
   (`0x00572520`, `0x00572530`); reader `0x0056A470` copies them back
   (`0x00572540`, `0x00572550`).
2. Bit n is byte n >> 3, mask 1 << (n & 7). An NPC class id maps to
   bit n through the table at `0x00732738` (count `0x00732734` = 35
   pairs {class id, bit}): class 147 → bit 1, 148 → 2, 150 → 3, 155 →
   4, 154 → 5, 265 → 6, 175–178 → 7–10, 202 → 11, 200 → 12, 210 → 13,
   201 → 14, 198 → 15, 199 → 16, 244–246 → 17–19, 251–257 → 20–26,
   264 → 27, 297 → 28, 511–515 → 29–33, 520 → 34; row 0 is {−1, 0}.
   A class id not in the table uses bit 0.
3. Field B is the intro record of `world/quests.md` §6.7 (set
   `0x00572420`, test `0x00572470`, clear `0x005724C0`; set in bulk
   per act list on act transitions). Field A (D2MOO 1.10f name
   `pQuestIntroFlags`, the first of the two buffers) holds the per-NPC
   first-talk bits of the act intro quests (`world/quests.md` §10.3,
   Act I intro chain 37): set `0x00572360`(player, game, NPC class) by
   the intro chains' event-11 callbacks. It has exactly five direct
   calls, all outside Ghidra function bodies (so `all.asm` and
   `index/calls.tsv` miss them; found with `disasm.py xref 0x00572360`
   and each read with `disasm.py at` as a `call 0x572360` after the
   class compare): `0x0058F8C2` in Act I `0x0058F870`, `0x00598464` in
   Act II `0x005983E0` (chain 38 event 11), `0x005B6CCF` in Act III
   `0x005B6C60`, and `0x0058E9D4`, `0x0058EA25` in code before the
   Act V intro init `0x0058EA50`. The setter scans the rule 2 pairs
   in order and sets only the bit of the first pair whose class id
   matches (no extra bit 0); only when no pair matches does it set
   bit 0. Tested
   by `0x005723C0` from those chains' event-0 and active functions.
   Confirmed on a save: after the player talked to Kashya (class 150,
   bit 3) in Normal, A of difficulty 0 is `08 00 …` and B stays zero.
4. Read: fewer than 52 bytes left or the u16 at +0 ≠ 0x7701 → 17. The
   size at +2 is not read.
5. Measured on the fresh saves (no NPC talked to): all 48 bytes of A
   and B are zero.

### 7. Stats and skills

#### 7.1 Stats (`gf`, at 0x2FD)

Version > 0x5E (0x5F, 0x60):

1. u16 0x6667, then an LSB-first bit stream (bit writer and reader:
   `items/bitstream.md` §1 rule 1; reader `0x00410F60`) from the next
   byte.
2. Writer (`0x00569540`): the player's base stat entries
   (`0x00625C90` on unit +0x5C, up to 512 entries {layer u16, id u16,
   value i32}, ascending key = id << 16 + layer, `sim/stat-lists.md`
   §3, §5). For each entry whose id < the `itemstatcost` count and
   whose `CSvBits` n ≠ 0:
   1. 9 bits id;
   2. if `CSvParam` p ≠ 0: p bits layer;
   3. n bits value v', where for n < 32: unsigned (`CSvSigned` = 0) →
      v' = 0 if v ≤ 0, 2^n − 1 if v ≥ 2^n − 1, else v; signed → v
      clamped to −2^(n−1) .. 2^(n−1) − 1 (written as its low n bits).
      n = 32 → v as is.
3. Then 9 bits 0x1FF. The section ends at the byte holding the last
   bit (byte length = whole bytes plus one for a partial byte).
4. Stats with `CSvBits` 0 and entries with value 0 (not stored in a
   stat list) are never written. Values are the stored values:
   life, mana and stamina (stats 6–11) in 1/256 points
   (`sim/stats.md` §2 rule 2), so hitpoints 55 is written as 14080.
5. Reader (`0x0056A4F0`): bytes left < 2 or u16 ≠ 0x6667 → 18. Loop:
   id := 9 bits; id > 0x1FE → done (cursor := start + byte length);
   id < 0, id ≥ the `itemstatcost` count or `CSvBits` = 0 → 18. Layer
   := `CSvParam` bits read **sign-extended**, then kept as u16 (0 if
   `CSvParam` = 0). Value := n bits, sign-extended when n < 32 and
   `CSvSigned`; else unsigned. Set base stat (`0x00627260`(unit, id,
   value, layer)).
6. Reads past the end of the buffer yield 0 bits (the reader's
   overflow flag is never tested); see edge case 2.

Versions 0x5C–0x5E (`0x0056A620`):

7. u16 0x6667; k = header +0x29; m = ceil(k / 8) mask bytes; then one
   u32 per set mask bit. Bounds: 2 + m bytes, then m + 4 × (set bits
   in all m bytes) must fit, else 18. For i = 0..k−1: mask bit i
   (byte i >> 3, mask 1 << (i & 7)) set → stat i := the next u32
   (layer 0); clear → stat i := 0. Cursor advances by m + 4 × (number
   of values consumed).

Measured on the 1.14d `itemstatcost` (359 rows; `.txt` and `.bin`
agree): 16 stats have `CSvBits` ≠ 0, none has `CSvParam` or
`CSvSigned`:

| Ids | Stats | CSvBits |
|---|---|---|
| 0–4 | strength, energy, dexterity, vitality, statpts | 10 |
| 5 | newskills | 8 |
| 6–11 | hitpoints, maxhp, mana, maxmana, stamina, maxstamina | 21 |
| 12 | level | 7 |
| 13 | experience | 32 |
| 14, 15 | gold, goldbank | 25 |

Code reads these columns from the loaded table (mod patches change
them); this list is a measurement, not a constant.

8. Measured: the fresh saves of all seven classes hold ids 0–3 and
   6–12 in ascending order (statpts, newskills, experience, gold and
   goldbank are 0 and absent, rule 4), then 0x1FF; the padding bits of
   the last byte are 0 and the stream is 281 bits = 36 bytes after
   `gf` (ends at 0x323). Values are the class's `charstats` start values (strength,
   energy = `int`, dexterity, vitality, `stamina`; hitpoints = maxhp =
   `hpadd` + vitality, mana = energy) with stats 6–11 × 256 (Barbarian:
   30, 10, 20, 25, life 14,080, mana 2,560, stamina 23,552). A
   character after a fight adds experience (id 13, 32 bits) and has
   hitpoints < maxhp (a fractional stored value, 11,887 = 46.43
   points).

#### 7.2 Skills (`if`)

1. Writer (`0x005696F0`): needs 2 + header-count (global +0xBA8)
   bytes, else 1. u16 0x6669; class > 6 → writer error 2. Then for k =
   0 .. count[class] − 1 (`data/runtime-maps.md` §5): one byte = the
   base level of the class's k-th skill (entry of that skill with
   owner −1, `skill_level(unit, entry, 0)`, `skills/levels.md`; no
   entry → 0). 1.14d: 30 bytes.
2. Reader (`0x0056A710`): bytes left < 2 or u16 ≠ 0x6669 → 19; no
   client → 19. c = client class; for i = 0 .. header +0x2A − 1: stop
   when i > count[c]; skill = class list entry i (−1 when i ≥ count);
   byte ≠ 0 → add skill with that base level (`0x0056DEB0`(unit,
   skill, level, 1)). Cursor += header +0x2A bytes.
3. The reader does not check that header +0x2A bytes remain.
4. Class 7 passes the header check (§2.2 rule 6) but has no class
   list.
5. Measured: `if` directly follows the stats byte end (no padding);
   the 30 bytes of every level-1 save are 0 (no skill learned; item
   skill bonuses are not base levels).

### 8. Item sections

#### 8.1 Item list framing (`0x005317B0`)

1. u16 0x4D4A, u16 count, then `count` item entries.
2. An item entry is one complete item stream as written by
   `items/bitstream.md` with "save on, children on": it starts with the
   item's own `JM`, is padded to a whole byte, and is followed by its
   socketed children, each a complete stream of its own
   (`items/bitstream.md` §2 rule 5, §5). Count counts top-level items
   only; children are not counted.
3. Items written, in order:
   1. every item of the inventory item list (link order), except an
      item in node kind 3 (body location 1–10) whose body location is
      4 or 5 (the hands);
   2. the hands: no weapon in use (`0x0063BEF0`) → right hand (4) then
      left (5); weapon in use is the right-hand item → right then
      left; else left then right. Missing hands are skipped;
   3. the cursor item, if it is an item unit.
4. count = (1 if there is a cursor item) + the number of list nodes
   with an item (`0x00531750`), minus 1 for each item that writes 0
   bytes. An item writes 0 bytes when its unit +0xC8 has bit 0x8000
   (`0x005316D0`; meaning Open question 11). (The other skip path of
   `0x005316D0` depends on `0x00564CA0`, which always returns 1: dead.)
5. An item that does not fit the remaining space is a fatal assert
   (`0x005316D0` asserts a 0-byte stream).
6. If the caller's cache flag is set and the player data holds an
   uploaded item blob (+0x5C: size, bytes) that fits, the player list
   is that blob copied verbatim (`0x005675E0`; Open question 12).
7. Before the real write, the master writes all item sections once
   into a 32 KB scratch buffer (pass flag 1). With L = the scratch
   size of the player list, T = the scratch size of all item sections
   and R = the space left: T ≤ R → the player list may use all of R;
   else it may use R − (T − L). Overflow is still fatal (rule 5), so
   this only moves the failure into the player list.
8. Measured on the fresh saves (Provenance), each record decoded with
   `items/bitstream.md` in the save format and ending exactly where the
   next `JM` starts: count 8 (7 for Sorceress and Necromancer, which
   start without a shield); in file order four `hp1 ` in the belt
   (mode 2, x 0–3), `tsc ` (inventory 9,3) and `isc ` (inventory 9,2),
   all compact, 14 bytes each (109 bits: `JM`, flags, version 101,
   mode, location, code, trailer bit 0); then the right hand (body 4)
   and the left hand (body 5) as full records (weapon 23–26 bytes,
   buckler 25 bytes), as rule 3 orders them. Every item has quality 2,
   item level 1, no sockets, trailer bit 0; the save-only 32-bit field
   of `items/bitstream.md` §4.1 rule 7 differs per item. The flags
   word is stored as the item has it: the starting items carry 0x2000
   (`items/generation.md` §1.4) in eight saves, and it was clear on
   every item of one save of a character that had been played longer
   (what clears it is not traced here; it is the load, §8.2 rule 7).
9. Every save-format call of the item writer `0x006313E0` passes save =
   1, children = 1 and alt-code = 0: `0x00531712` (`0x005316D0`), the
   five calls of `0x005317B0` (`0x00531899`, `0x005318C3`, `0x005318F0`,
   `0x00531929`, `0x0053195A`), `0x00541B5C` (`0x00541B10`) and
   `0x0055A2E1` (`0x0055A2A0`); the remaining two calls are the network
   senders (`items/bitstream.md` Open question 1). So a game-written
   save never holds an alt-code record (header bit 0x2000000).
10. Children are written for every item that has its own inventory
    (`0x006312B0`: alt-code 0, children 1 and unit +0x60 ≠ 0), compact
    or full, whatever its type's `hasinv`; the "filled sockets" count
    the reader uses (§8.2 rule 4) is written only in a full record and
    counts the inventory only when `hasinv` ≠ 0 (`0x0062A900`, 3 bits).
    The writer does not compare the two (edge case 15).

#### 8.2 Item list reading (`0x005337F0` → `0x005335E0`)

1. The item record reader is chosen by the save version: 0x47 →
   `0x00533350`; 0x48..0x60 → `0x005335E0`; else 1. Record decoding
   per version is the item reader's (Open question 2).
2. u16 ≠ 0x4D4A → 14. For each of `count` entries: peek the record
   (`0x0062AE20`); an unreadable head → 14. Create the item
   (`0x00558CB0`), which reports the bytes it used; no item and < 1
   byte used → 14; no item but bytes used → the entry is skipped.
3. Place the item (`0x00531210`, or `0x00531520` when the flag
   argument of `0x0056A7E0` is set: 1 for the corpse list, 0 for the
   player and hireling lists); placement fails → the item is freed.
4. Then its socketed children, as many as the parent's "filled
   sockets" field: each read the same way and inserted into the parent
   (`0x00531210`(child, parent)); with no parent they are read and
   freed.
5. A top-level item with the runeword flag (0x4000000) that no longer
   matches a runeword (`items/properties.md` §10.1) is passed to
   `0x00563470` (refresh by item mode; Open question 13).
6. Errors inside the player list surface as internal 20 (`0x0056A7E0`).
7. Item flags on load. Every save item (top-level, child, corpse,
   hireling and golem lists alike: `0x005335E0` at `0x00533665` /
   `0x00533712` and `0x0056ACE0` both create through `0x00558CB0`)
   gets, after its record is decoded (`0x0062E430`, which drops 0x80000
   and the alt-code bit 0x2000000 from the stored flags): flag 0x80000
   set and flag 0x2000 (instore) cleared (`0x00558D37`–`0x00558D4C`,
   flag setter `0x006280D0`(item, mask, on) on item data +0x18), then
   the replenish timers of `items/generation.md` §9 step 6
   (`0x00558530`, `0x00558580`). So a file's 0x2000 never survives a
   load, and the next save writes the flags without it (the writer's
   own changes, 0x80000 cleared and 0x800000 set, are
   `items/bitstream.md` §2 rule 1). Only items created since the last
   load (start items, drops, vendor items) are saved with 0x2000.
   Measured on two generated characters: a compact `hp1 ` saved with
   flags 0x00A02010 and a full `lsd ` with 0x00802010 were re-saved by
   the game as 0x00A00010 and 0x00800010, every other record bit
   unchanged (record lengths 14 and 23 bytes, trailer, item level,
   position, quality); the fresh characters' start items keep 0x2000
   (§8.1 rule 8) because they were created, not loaded.
8. Child count. The number of children read after an entry (rule 4)
   comes from the record peek `0x0062AE20`: the 3-bit "filled sockets"
   value of a full record, and 0 when the flags have 0x200000
   (compact) or 0x2000000 (alt-code), so compact and alt-code entries
   never have children on load.

#### 8.3 Corpse section

1. Writer (`0x005697F0`): over the player's corpse list (`0x0063D570`
   on the player's inventory) take each corpse unit that has an
   inventory with items; score = 1 + the repair cost (`world/vendors.md`
   §9.2, t = 3) of each of its items that is not gold (item type 4).
   The corpse with the highest score is saved (ties → the later one).
2. Layout: u16 0x4D4A, u16 n (1 if a corpse was chosen, else 0); if n
   = 1: u32 unknown, u32 x, u32 y, then the corpse's item list (§8.1,
   cache flag 0).
3. The first u32 is never assigned: it holds stack data (original bug;
   d2rs writes 0, Open question 14). x, y = the corpse's coordinates
   (`0x0045ADF0`, `0x0045AE20`).
4. Reader (`0x0056A830`), errors → 21 (with a sub-code for
   diagnostics): no client; fewer than 4 bytes; u16 ≠ 0x4D4A; n ≥ 2.
   For each corpse: 12 bytes must fit, they are skipped (never read);
   a player corpse unit is created for the player's class and the
   item list is read into it, then linked to the player.
5. Measured: every save without a corpse has `4A 4D 00 00` here.
6. Measured (a softcore Sorceress that died in Blood Moor, respawned
   in town and saved without touching the corpse): n = 1; the first
   u32 is non-zero (stack data, rule 3; never write 0 as the
   original's value); x = 0 and y = 0; the corpse list holds the
   equipped weapon (right hand, body 4, mode 1), while the belt
   potions and the inventory scrolls stay in the player list.
7. x and y come from the corpse unit's path: for unit types 0, 1, 3
   the getters (`0x0045ADF0`, `0x0045AE20`) return 0 when unit +0x2C
   (path) is null, else the path's u16 at +2 / +6 (`0x006488C0`,
   `0x00648900`); types 2, 4, 5 read the static path's +0x0C / +0x10.
   The 0 measured in rule 6 means the town-respawn save sees the
   Blood Moor corpse with no path or a zero path position (which one
   is not traced). The reader skips the 12 bytes (rule 4), so the
   values have no effect on load.

#### 8.4 Hireling items (`jf`, expansion only)

1. Writer (`0x005699A0`): u16 0x666A; if the player has a hireling pet
   node (type 7): the hireling's item list (§8.1, cache flag 0); else
   nothing more.
2. Reader (`0x0056AC10`): fewer than 2 bytes left → the section is
   treated as absent (no error, cursor unchanged). u16 ≠ 0x666A → 22.
   If the hireling was restored from the header (§2.5), its item list
   is read (errors → 22); else no list is read.
3. After the list: `world/hirelings.md` §10 rule 8.
4. Measured: an expansion save without a hireling (hireling block
   zero) has `6A 66` and no list.
5. Measured (`bdMercTwo`): an expansion save with a live hireling that
   carries no items has `6A 66` followed by an empty item list
   `4A 4D 00 00` (count 0), then `kf`; the list is present because the
   hireling exists, not because it has items.

#### 8.5 Iron Golem item (`kf`, expansion only)

1. Writer (`0x00569A00`): u16 0x666B, then u8 g. g = 1 when the
   player has a golem pet node (type 3) whose unit class is 0x123
   (291), the player has a skill 90 entry (Iron Golem, owner −1) and
   the golem's inventory has a first item; else 0. g = 1 → that one
   item as an item entry (§8.1 rule 2, no `JM` list header, no count);
   if it writes 0 bytes, g is rewritten as 0.
2. Reader (`0x0056AE50`): fewer than 2 bytes left → absent (no
   error). u16 ≠ 0x666B → 23. g ≠ 0 → the player must have a skill 90
   entry (`0x006439F0`), else 23; read one item entry with its children
   (`0x0056ACE0`; failure → 23); the item gets mode 3 (`0x00624690`) and
   is handed to the client for the golem's re-summon (`0x00538700`,
   Open question 15). The cursor moves past the u8 even when g = 0.
3. Measured: a save without an Iron Golem ends `6B 66 00`; that 0 is
   the file's last byte.
4. Measured (`bdGolem`): a Necromancer saved with a live Clay Golem
   (class 289, skill 75) writes g = 0 and the file ends `6B 66 00`, as
   rule 1 requires (class ≠ 0x123): a Clay Golem is not saved.
5. The reader checks only that the 2 marker bytes fit
   (`0x0056AE73`–`0x0056AE78`); it then reads g at the cursor without a
   bounds check (`0x0056AE85`). A classic game returns before any read
   (game +0x70 = 0, `0x0056AE5B`), so rule 2 applies to expansion
   games only. Edge case 16.

### 9. Load sequence (`0x0056B180`)

1. Header (§2.2). Result 2 (new character) with the cursor exactly at
   the end of the buffer (the file is the 335-byte stub) → a new
   character of the client's class is started (`0x00569F80`) and the
   load succeeds; result 2 with more bytes → error.
2. Quests, waypoints, NPC flags, stats (§4–§7.1). Then hitpoints
   (stat 6) and mana (stat 8) are remembered.
3. Skills, player items, corpse, hireling (§2.5), hireling items, golem
   (§7.2, §8).
4. Post-load (`0x0056AF80`): gold (stat 14) < 0 or above the carry
   limit (`0x00622E70`: stat 12 level × 10,000) → 0; goldbank (15) < 0
   or above the stash limit (`0x00623460`: 2,500,000) → 0; stamina (10)
   := the unit's `maxstamina` (11) total (`0x00625DB0`); item
   indices resolved (§2.4 rule 6); hitpoints and mana := the values
   remembered in rule 2 (item bonuses do not change them); stats 67,
   68, 69 (`velocitypercent`, `attackrate`, `other_animrate`) := 100;
   stat 30 (`nextexp`) := `0x00611800`(class, level).
5. On any error after the player unit exists, the unit is removed.
6. New-character start (`0x00569F80`, rule 1): `formats/d2s-load.md`
   §1 (what it creates, and why a 335-byte stub grows to a full save).
7. Load effects in the master's order, each with its code and owner
   spec (what `d2-server` character storage applies after
   `d2-formats` parsed the bytes): `formats/d2s-load.md` §2.

### 10. Errors

1. Internal codes (returned by the section readers) map to the
   result through the table at `0x006E1208`; a code < 0 or > 0x1A
   gives 1:

   | Internal | Cause | Result |
   |---|---|---|
   | 0 | success | 0 |
   | 1 | (unused here) | 14 |
   | 2 | new-character flag with more data (§9 rule 1) | 9 |
   | 3 | no player / no client name | 14 |
   | 4 | header too short; class > 7; no player data | 9 |
   | 5 | size ≠ header +0x08 | 14 |
   | 6 | checksum mismatch | 14 |
   | 7 | version outside 0x5C..0x60; name mismatch | 1 |
   | 8 | expansion character, classic game | 24 |
   | 9 | classic character, expansion game | 23 |
   | 10 | dead hardcore character | 21 |
   | 11 | hardcore character, non-hardcore game | 20 |
   | 12 | softcore character, hardcore game | 19 |
   | 13 | Nightmare not unlocked | 17 |
   | 14 | Hell not unlocked | 18 |
   | 15 | quest section | 2 |
   | 16 | waypoint section | 3 |
   | 17 | NPC section | 11 |
   | 18 | stats | 4 |
   | 19 | skills | 5 |
   | 20 | player items | 7 |
   | 21 | corpse | 8 |
   | 22 | hireling items | 10 |
   | 23 | golem | 10 |
   | 24 | (unused here) | 14 |
   | 25 (0x19) | current-season ladder character, non-ladder game (§2.2 rule 5.2) | 26 |
   | 26 (0x1A) | other character, ladder game (§2.2 rule 5.2) | 25 |

2. Before the master: bad magic or < 8 bytes → 9 (`0x00534330`); no
   file → no result (the caller's output stays 0, `0x005343A0`).
3. The result values are what the game shows the player; their texts
   are not traced (Open question 16).
4. The file is read with a 0x2000-byte limit; a longer file is cut and
   then fails the checksum or size check (6 or 5).
5. A wrong section order or a missing fixed section fails on that
   section's marker.
6. Bytes after the last section are not checked (covered by the size
   and checksum only).

## Constants & data dependencies

| Constant | Value | Where |
|---|---|---|
| magic | 0xAA55AA55 | `0x00568F20`, `0x00534330` |
| written version | 0x60 | `0x00568F20`, `0x0043C540` |
| loader range | 0x5C..0x60 (legacy < 0x5C) | `0x0056A090`, `0x00534330` |
| header size | 0x14F | `0x00568F20` |
| buffer / max file | 0x2000 | callers of `0x00569AD0`; loaders |
| quest section | "Woo!", 6, 0x12A | `0x00569320` |
| NPC section | 0x7701, 0x34; table `0x00732734` (35 pairs) | `0x00569490` |
| stats terminator | 0x1FF in 9 bits | `0x00569540` |
| markers | `gf` 0x6667, `if` 0x6669, `JM` 0x4D4A, `jf` 0x666A, `kf` 0x666B | writers |
| Iron Golem | monster class 0x123, skill 90 | `0x00569A00`, `0x0056AE50` |
| error map | table `0x006E1208` (27 entries) | `0x0056B180` |

Columns read: `itemstatcost` `CSvBits`, `CSvParam`, `CSvSigned`
(stats); the class skill lists (skills); item columns through
`items/bitstream.md`.

## Randomness

None. The hireling seed and the map seed are stored values, not
draws.

## Edge cases & original bugs

1. The corpse header's first u32 is uninitialised stack data (§8.3
   rule 3); the loader skips it.
2. A stats section without a 0x1FF terminator before the end of the
   buffer never ends: past the end, reads give id 0 (strength, valid)
   with value 0 forever. Only a hand-made file can do this (size and
   checksum must still match). d2rs rejects it with 18 instead of
   hanging (no observable output to match).
3. Item indices in hotkeys and mouse skills are positions in the
   pre-save link order (§2.4 rule 2), but loading links items in file
   order, where the hands come after the rest (§8.1 rule 3). An index
   may point to a different item after a reload (Open question 5).
4. The skills reader trusts header +0x2A for its byte count and can
   read past the buffer (§7.2 rule 3); index i = count[c] looks up
   skill −1 (`>` instead of `≥`).
5. The 0x5C–0x5E stats bounds check counts set bits in the whole last
   mask byte, including bits ≥ k, while the cursor advances only by
   the values consumed.
6. Gold or stash gold above the limit becomes 0, not the limit (§9
   rule 4).
7. The `CSvParam` layer is read sign-extended (§7.1 rule 5); no 1.14d
   stat has `CSvParam`.
8. A hireling whose row is missing on load creates no hireling, so its
   `jf` list is not consumed and the `kf` marker check then fails
   (23).
9. The writer ORs 0x20 / 0x40 into the status but never clears them.
10. `jf` and `kf` may be missing at the end of an expansion file
    (fewer than 2 bytes left) and the load still succeeds.
11. The creation time written into the stub never survives: the first
    load takes the new-character path, which does not copy +0x2C, so
    every later save stores 0 there (§2.2 rule 10). d2rs reproduces
    the 0.
12. The corpse's x and y can be saved as 0 even though the corpse lies
    in Blood Moor (§8.3 rules 6–7); the loader never reads them, so
    the corpse's saved position carries no information.
13. A Clay Golem is not saved: `kf` count 0 (§8.5 rule 4); only an
    Iron Golem's item is kept, so no other summon survives a save.
14. Alt-code records (item header bit 0x2000000) never occur in a
    game-written save (§8.1 rule 9). A hand-made one is still accepted:
    the record ends after its base code (no unit +0x28, no trailer;
    `items/bitstream.md` §4.1 rule 4) and it has no children (§8.2
    rule 8). d2rs reads it the same way and never writes one.
15. Children vs. the filled count (§8.1 rule 10): the writer writes a
    child for every item in an item's inventory, the reader reads as
    many as the 3-bit filled count (0 for compact records, 0 when
    `hasinv` = 0, at most 7). A mismatch would make the reader take a
    child as the next top-level entry. All 13 saves of this PC parse
    to their last byte, so none has a mismatch; d2rs's
    writer refuses an item whose child count differs from the count its
    record carries, since the game could not read such a file back.
16. A file that ends right after the `kf` marker (`6B 66`, no g byte)
    makes the reader take g from the byte after the file's data (§8.5
    rule 5). In single player that byte is uninitialised stack in the
    8,192-byte read buffer of `0x005343A0` (filled only up to the file
    length by `fread`), so the outcome is not defined by the file: g =
    0 loads, anything else needs skill 90 and an item record from 0
    remaining bytes and fails with 23 (result 10). Only a hand-made
    file can do this (size and checksum must match); d2rs rejects it
    with 23.
17. Item flag 0x2000 (instore) in a file has no effect: the loader
    clears it on every item (§8.2 rule 7). A writer may set or clear it
    on hand-built items; the game-equivalent choice for a character
    that was loaded at least once is clear.
18. The 32 appearance bytes are never carried over from a loaded file
    (§2.8); copying them makes a save differ from the game's.

## Test vectors

Synthetic, written from this spec (CI-safe); none is copied from a
save.

| Input | Expected output | Source |
|---|---|---|
| checksum of `01 02 03` | 0x0000000B | §3 |
| checksum of `FF FF FF FF` | 0x00000EF1 | §3 |
| s = 0x80000000, next byte 0x00 | s = 0x00000001 | §3 rule 1 |
| stub: 335 zero bytes, then +0x00 `55 AA 55 AA`, +0x04 `60 00 00 00`, +0x08 `4F 01 00 00`, +0x14 `54 65 73 74` ("Test"), +0x24 `21 00`, +0x29 `10`, +0x2A `1E`, +0x2B `01`; checksum over it with +0x0C = 0 | 0xC7373BCA (bytes `CA 3B 37 C7` at +0x0C); loader: zero +0x0C, recompute → equal; size 335 = +0x08 → accepted; status bit 0 + cursor at end → new character | §2.6, §3, §9 rule 1 |
| same stub, one byte changed after the checksum is stored | internal 6 → result 14 | §2.2 rule 2 |
| same stub with +0x08 = 336 and the checksum recomputed | internal 5 → result 14 | §2.2 rule 2 |
| same stub with version 0x61 (checksum recomputed) | internal 7 → result 1 | §2.2 rule 3 |
| stats: strength 30, hitpoints 55 (stored 14080), level 1 | `67 66` then `00 3C 30 00 70 03 18 04 FE 03` (74 bits) | §7.1 rules 1–4 |
| stats: none | `67 66 FF 01` | §7.1 rule 3 |
| stats: gold 40,000,000 (> 2^25 − 1) | clamped 33,554,431: `67 66 0E FE FF FF FF 07` (43 bits) | §7.1 rule 2.3 |
| stats: id 16 (`CSvBits` 0) in the stream | internal 18 → result 4 | §7.1 rule 5 |
| version 0x5E stats, k = 16, stats 0 = 30 and 12 = 1 | `67 66 01 10 1E 00 00 00 01 00 00 00` | §7.1 rule 7 |
| skills, Amazon, all 0 | `69 66` + 30 × `00` | §7.2 rule 1 |
| empty item list | `4A 4D 00 00` | §8.1 rule 1 |
| no corpse | `4A 4D 00 00` | §8.3 rule 2 |
| corpse count 2 | internal 21 → result 8 | §8.3 rule 4 |
| expansion, no hireling, no golem | `6A 66 6B 66 00` | §8.4, §8.5 |
| hotkey: skill 36, left flag, no item | `24 80 00 00` | §2.4 rule 1 |
| hotkey: none | `FF FF 00 00`; decodes to skill −1, item −1 | §2.4 rules 1, 4 |
| expansion, hireling present with no items, no golem | `6A 66 4A 4D 00 00 6B 66 00` | §8.4 rules 2, 5; §8.5 |
| a compact item stored with flags 0x00A02010, loaded, saved again (nothing else changed) | flags written 0x00A00010; the rest of its record unchanged | §8.2 rule 7; `items/bitstream.md` §2 rule 1 |
| a full item stored with flags 0x00802010, loaded, saved again | flags written 0x00800010 | §8.2 rule 7 |
| header +0x88..+0x97 = `01 01 01 01 01 FF FF FF 01 01 FF FF FF FF FF FF`, no equipped item, loaded and saved | +0x88..+0xA7 = 32 × `FF` | §2.8 rules 1–2 |
| a full record with flags bit 0x200000 or 0x2000000 followed by further bytes | child count 0: the next bytes are the next top-level entry | §8.2 rule 8 |

Real-save checks (`#[ignore]`, `D2_GAME_DIR` or the user's save
folder): every 1.14d `.d2s` passes §3 and §2.2 rule 2, its sections
parse in §1 order to the file end, and re-serialising its parsed
content reproduces the file byte for byte except +0x30 (save time)
and the corpse's first u32. `tools/d2s_check.py <save.d2s> …` (reads
the tables from `D2_GAME_DIR`) runs the structural part of this check
and prints every field; it holds no save data.

## Provenance

- 1.14d `Game.exe` (decompile exports and `tools/ghidra/disasm.py`
  disassembly): writer master `0x00569AD0` (callers `0x00531D30`,
  `0x00531EB0`, `0x00532240` debug dump, `0x00532340` server save →
  `0x00538CE0`); header `0x00568F20`, hireling block `0x00568E60`,
  item index `0x00568DC0`, name length `0x00568D80`; quests
  `0x00569320`; waypoints `0x005693E0`; NPC `0x00569490` with
  `0x00572520`/`0x00572530`; stats `0x00569540`; skills `0x005696F0`;
  lists `0x005697B0` → `0x00531BB0` → `0x005317B0`, item
  `0x005316D0`, count `0x00531750`, cache `0x005675E0`; corpse
  `0x005697F0`; `jf` `0x005699A0`; `kf` `0x00569A00`; checksum
  `0x00411130`; bit writer/reader `0x00410E40`, `0x00410EB0`,
  `0x00410E90`, `0x00410F60`, `0x00411020`, `0x00411030`, popcount
  `0x00411170`. Loader: dispatch `0x00534330`, file `0x005343A0`,
  client buffer `0x00534520`, master `0x0056B180`, header `0x0056A090`,
  status `0x00569D80`, slot decode `0x00569EC0`, new character
  `0x00569F80`, quests `0x0056A370`, waypoints `0x0056A3E0`, NPC
  `0x0056A470` (`0x00572540`/`0x00572550`), stats `0x0056A620`/
  `0x0056A4F0`, skills `0x0056A710`, items `0x0056A7E0` → `0x005337F0`
  → `0x005335E0`, corpse `0x0056A830`, hireling `0x0056AA50`, `jf`
  `0x0056AC10`, golem `0x0056AE50`/`0x0056ACE0`, post-load
  `0x0056AF80`/`0x0056AF20`, error table `0x006E1208` (read from the
  image). Client: creation `0x0043C540`/`0x0043C6A0`/`0x0043C6B0`,
  select `0x0043C8A0`, convert `0x0043CAA0`, download `0x0045C520`.
  Client-record accessors `0x005385C0`, `0x005385F0`, `0x00538640`,
  `0x00538790`, `0x00538830`, `0x005382B0`, `0x005390D0`,
  `0x00539190`, `0x00539220` (field offsets from their disassembly).
  Tables `0x00732734`/`0x00732738` and `0x0070CCC8` read from the image.
- 1.14d `patch_d2` `itemstatcost.bin` (359 records) and `.txt`:
  `CSvBits` / `CSvParam` / `CSvSigned` measured with a script; both
  agree.
- `world/waypoints.md` §3: section bytes and position measured on the
  1.14d test characters' saves (version 96) used there.
- Real 1.14d saves (`%USERPROFILE%\Saved Games\Diablo II`, version
  96, read-only; no bytes copied into the repo): `bdAma`, `bdSor`,
  `bdNec`, `bdPal`, `bdBar`, `bdDru`, `bdAss` (fresh level 1, 958–981
  bytes); `bdMerc` (Barbarian, read as the 335-byte stub and again at
  985 bytes after a fight and a talk with Kashya, no hireling hired yet);
  `bdDead` (Sorceress, 958 bytes after play, alive with no corpse when
  read).
  Final saves, read again with `tools/d2s_check.py`: `bdDead` 974
  bytes, 24 pass / 0 fail, corpse count 1 at 0x39B, corpse list count
  1 at 0x3AB, status 0x0028 — §2.3 note, §8.3 rules 6–7, edge case 12
  confirmed on bdDead (1.14d); the corpse writer `0x005697F0` and the
  getters `0x0045ADF0`/`0x0045AE20` → `0x006488C0`/`0x00648900` read
  with `tools/ghidra/disasm.py` (the first u32 slot `[ebp-0x2C]` is
  never written). `bdMerc` 985 bytes, 23 pass / 0 fail, no hireling
  (a fresh character cannot hire from Kashya); it is 5 bytes larger
  than `bdBar` (980) only because its stats stream is 5 bytes longer
  (ends 0x328 vs 0x323): it carries experience (stat 13, 32-bit
  value; 41 more bits) after a fight; Kashya's first-talk bit is set
  in NPC field A (§6 rule 3).
  `bdMercTwo` 1,079 bytes (Barbarian level 8, live rogue hired from
  Kashya: S→C 0x81 class 271, GUID 13, name Diane), 24 pass / 0 fail:
  hireling block flags 0, seed = the 0x81 seed, name index 21, `Id` 0,
  experience 39,482, 16 zero bytes; `jf` then a count-0 list; `kf`
  g = 0 — §2.5 rule 3, §8.4 rule 5 confirmed on bdMercTwo (1.14d,
  hired rogue). `bdGolem` 967 bytes (Necromancer level 6, saved with a
  live Clay Golem, class 289), 23 pass / 0 fail: hireling block zero,
  `jf` without a list, `kf` g = 0 — §8.5 rule 4, edge case 13. Both
  have status 0x0028 (each died once).
  Checked with `tools/d2s_check.py` and the 1.14d `patch_d2` tables:
  all pass every structural check (§1 rule 7); field values in §2.1,
  §2.4 rule 7, §2.6, §6 rules 3 and 5, §7.1 rule 8, §7.2 rule 5,
  §8.1 rule 8, §8.3 rule 5, §8.4 rule 4, §8.5 rule 3. The §6 field A
  setter call sites were found with `tools/ghidra/disasm.py xref`
  (raw rel32 scan; the Ghidra export lists the setter with 0
  callers).
- Second pass (DS questions of `docs/handoff/impl-d2s.md`, local run
  C66): addresses and saves in `formats/d2s-load.md` Provenance.
- D2MOO 1.10f `PlrSave2.h`/`.cpp` (hint for names: `dwWeaponSwitch`,
  `dwCreateTime`, `nGuildEmblemBgColor`, `D2MercSaveDataStrc`,
  client save flags). Every rule above was read in the 1.14d code.
  Differences seen: D2MOO names +0x34 a play time (1.14d always writes
  −1) and +0xD0/+0xD4/+0xD8 last level / town / difficulty (1.14d
  writes 0 and never reads them); the hotkey's second word is an item
  index in 1.14d; the quest normalisation covers 42 slots
  (`world/quests.md` §1.6).

## Open questions

1. Legacy loader (`0x00534020`, versions < 0x5C, ≥ 0x82 bytes): not
   specified. Settle: only if pre-1.09 saves must load; Ghidra on
   `0x00532690`–`0x00533F70`.
2. Item record decoding for save versions below 0x60 (the version is
   passed to `0x0062AE20`/`0x00558CB0`). Settle: the item reader spec.
3. **Answered except the classic part** (9 saves and a stub of this PC; §1 rule
   7, §2.1, §7.1 rule 8, §8.1 rule 8): every fresh-character field
   matched. Still open: a classic (non-expansion) character, to see
   status without 0x20 and a file ending after the corpse section
   with no `jf`/`kf`.
   **Answered** (classic part; C66 run 2026-10-07, `local-buddy-q-saves.md`):
   13 saves round-trip byte for byte; the classic `TestAma` and the
   classic stub `TestStub` were loaded and re-saved by 1.14d: status
   without 0x20 (`TestStub` 0), and the file ends after the corpse
   header with no `jf` / `kf` (§1 rule 2, §8.5 rule 5 `0x0056AE5B`).
   The game's re-saves differ from d2rs-generated files only as §2.8
   and §8.2 rule 7 explain.
4. **Answered** (confirmed on bdMercTwo (1.14d, hired rogue); §2.5
   rule 3, §8.4 rule 5): a live Act I rogue writes flags 0, its seed,
   name index, `Id` 0 and experience, and `jf` carries a count-0 list
   (`world/hirelings.md` Open question 1). Still missing: a dead
   hireling (flags 0x10000) and a hireling carrying items (the `jf`
   item records).
5. Item index stability (edge case 3): a save with a hotkeyed Tome of
   Town Portal and an equipped weapon, saved, reloaded and saved again.
   **Answered from the binary** (`formats/d2s-load.md` §4): linking
   appends, so the reloaded list is in file order (hands last); an
   index can resolve to another item once, then stays stable until an
   item is relinked. The save above is that file's Open question 3.
6. **Answered** (§8.3 rules 6–7, edge case 12; bdDead, 1.14d): a
   dead softcore character whose corpse is on the ground at save time
   writes n = 1, a non-zero first u32 (stack data), x = y = 0, and its
   equipped weapon as the corpse list. Still untraced: whether x, y
   are 0 because the path is null or its position is 0 (no effect on
   load).
7. Header +0xCF (client +0x480): any save with a non-zero byte there
   (Battle.net-style emblem) or Ghidra on `0x00539BE0`. Every save of
   this PC has 0.
   **Answered**: set only from the file (setter `0x005391C0`, callers
   `0x0056A343`, legacy `0x00532997`), written back by `0x005692A9`;
   other readers are realm-only (`0x0052C9AB`, `0x0052CA84` need
   `[0x00883D50]`; summary `0x00539BE0`). Kept as found.
8. Ladder checks (§2.2 rule 5.2): the service object `[0x00883D54]`
   and player +400 are not traced; single player never runs them.
   **Answered**: dead. `[0x00883D50]`/`[0x00883D54]` (zeroed .data)
   are written only by `0x0052C0E0`, which nothing calls, jumps to,
   points to or exports; rule 5.2 never runs, results 25/26 never occur.
9. Map seed restore needs game +0x6A = 3 and +0x84 = 0: confirm that a
   single-player game has type 3 (`sim/rng.md` Open question 2): two
   loads of one save produce the same map.
   **Answered** (`formats/d2s-load.md` §7): single player has client
   game type 0 → create message byte 3 (`0x00477CDF`) → game +0x6A = 3
   (`0x00530CFF`); +0x84 = 0 without `-seed`. The seed is restored on
   every load in the difficulty the save was written in.
10. **Answered** (§6 rule 3): field A is the act intro quests'
    first-talk bits (D2MOO `pQuestIntroFlags`), set by `0x00572360`
    from its five direct calls `0x0058E9D4`, `0x0058EA25`,
    `0x0058F8C2`, `0x00598464`, `0x005B6CCF`, which the Ghidra export
    missed (they lie outside function bodies); the setter writes only
    the first matching pair's bit. A save after talking to Kashya has
    her bit set in A. Still
    unmeasured: the other Act I NPCs' bits (expected from the same
    table, §6 rule 2).
11. Item unit +0xC8 bit 0x8000 (items skipped by the writer, §8.1
    rule 4): which items carry it.
    **Answered**: none. Of all writes to +0xC8/+0xC9 in `all.asm`, no
    immediate is 0x8000, server register writes are 0x200 / 0x800000
    (`0x00542903`, `0x005679E0`, `0x00567B00`, `0x00567C70`), setter
    `0x0045C430` is unreferenced, the rest are client code. The skip
    never happens.
12. Cache flag and the uploaded item blob (§8.1 rule 6): which callers
    pass the flag (C→S 0x6C upload path).
    **Answered**: the flag = "interacting with a player": `0x00532400`
    (`0x00532437`–`0x00532464`) sets it when unit +0x6C ≠ 0 and interact
    type +0x68 = 0, for all three writers (`0x00531EB0`, `0x00532240`,
    `0x00532340`); then player data +0x5C (D2MOO `pTrade`: u32 size,
    pointer) is copied (`0x00531BB0` → `0x005675E0`). Its filler (trade
    code) is not traced; single player never sets the flag.
13. `0x00563470` behaviour on a runeword item that no longer matches
    (§8.2 rule 5).
    **Answered except the equipped case** (`formats/d2s-load.md` §6):
    a stored or cursor item is unlinked and freed (deleted); an
    equipped one is unequipped by `0x00560CD0` with flag 0x20; its end
    place is that file's Open question 2.
14. Corpse first u32: whether any reader (client, realm) uses it; d2rs
    writes 0.
    **Answered** (`Game.exe`): unused. Readers: the loader skips it
    (`0x0056A830`); `0x0043C8A0` reads the header, `0x0043CAA0` only
    checksums, `0x0045C520` copies. Comparisons must mask these 4 bytes.
15. Iron Golem item on load: how `0x00538700` and the re-summon use it.
    Settle: a Necromancer with an Iron Golem made from an item. Still
    missing: that save; none was produced automatically because it
    needs a Necromancer able to cast Iron Golem (skill points and
    level well past a fresh character). A Clay Golem save (`bdGolem`)
    writes `kf` count 0 (§8.5 rule 4), so it does not settle this.
    **Answered from the binary** (`formats/d2s-load.md` §3): the loader
    keeps only the item's GUID (client +0x484); on joining,
    `0x005394A0` finds that item, places it at the player and casts
    Iron Golem on it at the skill's level with bonuses (`0x0056F7F0`),
    then clears +0x484. Still unmeasured (needs that save).
16. Result texts: the strings shown for results 1–26 (character
    select error dialog).
    **Answered** (`formats/d2s-load.md` §5): S→C 0xB4 carries the
    result; the client maps it (`0x0045C7E8`) to a message index and
    shows string `0x0070F384`[index]: ids 5359–5381, 10101, 10102 per
    result (table there). Texts come from the user's string tables.
17. Appearance byte mapping (§2.8 rule 3): which of the 16 component
    and 16 colour bytes each equipped item sets, and to what value
    (`0x0063DA70`, the composite branch of `0x0063E510` via
    `0x0064F420`/`0x0064F500`/`0x0063D900`/`0x0062C100`, and the item
    class from `0x00627D40`). Settle: Ghidra on those functions, checked
    against saves with a weapon, a shield, a helm, a body armour and
    dyed or coloured items equipped. Needed for a d2rs writer to
    reproduce +0x88..+0xA7; the loader never reads them.
    **Answered** (`formats/d2s-appearance.md`, from `0x0063E510`,
    `0x0063DA70`, `0x0063D710`/`0x0063D900`, `0x0062C100`): component
    byte = index of the item's `alternategfx`/`code` (body armour: the
    `armtype` token of each of its six part bytes) in a 255-entry token
    table built once from the item tables; helm → part 0, the hand items
    → 5 (one-hander in use, or `component` 5) / 6, others their
    `component`; colour = (`Transform` × 32 + colour) mod 256 + 1, or
    0xFF. The rule reproduces all eight measured component values;
    colours and armour still unmeasured (that file's Open question 2).
