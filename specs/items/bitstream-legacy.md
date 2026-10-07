# Spec: Items — Item record reader by save version (0x47–0x60)

- **Status:** draft: every gate read from the 1.14d `Game.exe` reader
  (`0x0062E430`, `0x0062CBE0`, `0x0062A970`, `0x0062AC80`, `0x0062AE20`,
  `0x00558CB0`) and the version-0x47 path (`0x00533350`, `0x00532F30`,
  `0x00530F40`, `0x00531040`, `0x00531390`); disassembly checked for
  every register argument and jump table. No pre-1.09 save was loaded
  (Open questions 1–2). Answers `formats/d2s.md` Open question 2 and
  `formats/d2s-legacy.md` Open question 1.
- **Target version:** 1.14d
- **Crate/module:** `d2-formats::d2s` (record decode by version) and
  `d2-sim::items::bitstream` (reader); `d2-server` character storage
  (version-0x47 creation and placement)
- **Related specs:** `items/bitstream.md` (the record layout, writer
  side: §2–§5 there are the reference this file names "W §n"),
  `formats/d2s.md` §8.2 (item lists of saves 0x5C–0x60),
  `formats/d2s-legacy.md` §8 (item lists of saves 0x47–0x5B),
  `world/vendors.md` §7.3.1 (the values the reader rebuilds instead of
  reading), `items/generation.md` §9, §11 (forced and format-0
  creation), `items/inventory.md` §2.4, §4.6, `items/inventory-moves.md`
  §7.14, §7.19 (placement routines), `sim/stat-lists.md`,
  `data/runtime-maps.md` §6 (version-0 list), `data/fields.tsv`
  (`itemstatcost`: `save bits`, `save add`, `save param bits`,
  `1.09-save bits`, `1.09-save add`).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 49–60 |
| Inputs | 61–69 |
| Outputs / state changes | 70–74 |
| Rules | 75–82 |
|   1. Entry, create and peek | 83–114 |
|   2. Version gates (v = the save version argument) | 115–166 |
|   3. Full record (`0x0062CBE0`) | 167–229 |
|   4. Stat-list entries (`0x0062D93C`–`0x0062E1D3`) | 230–279 |
|   5. Compact record (`0x0062A970`, EAX unit, EDI reader) | 280–295 |
|   6. Version-0x47 record (`0x00532F30`, ECX record, EDX bytes) | 296–330 |
|   7. Version-0x47 creation (`0x00530F40`, ECX game, ESI R) | 331–345 |
|   8. Version-0x47 placement | 346–385 |
| Constants & data dependencies | 386–402 |
| Randomness | 403–408 |
| Edge cases & original bugs | 409–436 |
| Test vectors | 437–461 |
| Provenance | 462–477 |
| Open questions | 478–483 |
<!-- /index -->

## Summary

One reader decodes every item record of a save of version 0x48–0x60
and every network / copy record (`0x0062E430`); it takes the save
version as an argument v and changes field widths, offsets, stat ids
and value columns for old versions. The writer has no version: it
always writes the current format (W §1–§5; `0x0062FFF0`, `0x0062AF80`,
`0x006312B0` hold no version compare). Version 0x47 (1.00–1.06) saves
use a separate fixed record (`0x00532F30`) that is turned into a forced
creation request (`0x00530F40`, item format 0) and placed by its own
routines (`0x00531040` player, `0x00531390` corpse).

## Inputs

| Name | Type | Source |
|---|---|---|
| bytes, length | the record start and the bytes left in the file | list reader (`0x005335E0`, `0x00533350`) |
| v | save version (u32 at file +4) | `0x005335E0`, `0x0056ACE0` pass the file's; every other caller 0x60 (§1 rule 4) |
| save | 1 on every save path; 0 on the wire | caller |
| tables | `itemstatcost` (record 0x144 bytes), items, `sets`, `setitems`, `uniqueitems`, `skills`, the version-0 list | `data/fields.tsv`, `data/runtime-maps.md` §6 |

## Outputs / state changes

An item unit with the fields of §2–§5 (or §6–§8 for v = 0x47), the
bytes used, and the socket child count; or a failure (§1 rule 3).

## Rules

"bits(s)" / "add(s)" = the width / offset of stat s in the column set
of §2 rule 1; "read n" = n bits LSB first (W §1 rule 1). A read past
the given length returns only the bits that remain (missing high bits
0), sets the reader's overflow flag and stays at the end
(`0x00410F60`); no item reader tests the flag.

### 1. Entry, create and peek

1. **Peek** `0x0062AE20`(bytes, length, save, out, v): save → u16 must
   be 0x4D4A, else 0 (no peek). Then 32 bits flags F (out +0x0C), 10
   bits skipped, 3 bits mode (+0x08); mode 3 / 5: 16 bits x (+0x04), 16
   bits y (+0x06); else 4 bits body location (+0x11), 4 bits x, 4 bits
   y, 3 bits page + 1 (+0x10 := value − 1). Code: F & 0x10000 → `ear `;
   else 32 bits, and when v < 0x5D a code `nec ` becomes `neg `. Class
   (+0x00) := the code's items index (`0x00633680`, −1 when none). Child
   count (+0x14) := 3 bits when F lacks 0x200000 and 0x2000000, else 0.
2. **Create** `0x00558CB0`(ECX game, EDX room, bytes, length, save,
   peek, &used, v): no game → 0. Peek class < 0 or ≥ the items count →
   used := 0, no item. The unit is allocated (`0x00555230`: type 4, the
   peek's class, x, y, game, room, init 1, the peek's mode); none → 0.
   Then the decode (rule 3) with (unit, bytes, length, save, v); used
   := its byte count. Success → item flag 0x80000 set, 0x2000 cleared,
   replenish timers (`formats/d2s.md` §8.2 rule 7); failure → the unit
   is freed (`0x00555600`) and no item is returned (used stays).
3. **Decode** `0x0062E430`: save and u16 ≠ 0x4D4A → failure, used 0.
   Read F; alt := F & 0x2000000, removed from F. Item data: flags +0x18
   := F without 0x80000; item level := 1; prefix and suffix slots
   (+0x38–+0x42) and rare names (+0x32, +0x34) := 0. F & 0x200000 →
   compact reader (§5; child count 0); else full reader (§3) with (save,
   alt, v). A result < 1 is a failure. The byte count is whole bytes,
   a partial last byte counted (`0x00410E90`).
4. Callers and v: the save lists (`0x005335E0`, both list readers of
   `formats/d2s.md` §8.2 and `formats/d2s-legacy.md` §8 rule 1) and the
   golem item (`0x0056ACE0`) pass the file's version; the item copy
   (`0x0055A2FE`) and `0x005419AF` pass 0x60 with save 1; the client
   (`0x004C0F66`, `0x004C1036`, `0x004C3B53`) passes 0x60 with save 0.
   So every gate of §2 is reached only through a save file.

### 2. Version gates (v = the save version argument)

1. **Column set.** v < 0x5D: bits(s) := `1.09-save bits` (record
   +0x1A), add(s) := `1.09-save add` (+0x20), param bits := record
   +0x28 (no txt column; 0 in every row of the live
   `itemstatcost.bin`). v ≥ 0x5D: `save bits` (+0x19), `save add`
   (+0x1C), `save param bits` (+0x24). The set is used for defense,
   max durability, durability, the socket count and every stat-list
   value; not for the compact record's quest difficulty, gold or
   quantity.
2. The gates, in record order:

<!-- rows -->
| Field | Condition | Behaviour |
|---|---|---|
| peek code | v < 0x5D | `nec ` → `neg ` (rule §1.1; the full reader then keeps the class, edge case 4) |
| affix offsets P, A | v in the table of rule 3 | P_v := P − d_P, A_v := A − d_A |
| auto affix | v < 0x59 | the 11 bits are read and dropped; slot := 0 |
| auto affix | v = 0x59 | value + 1, then + A_v when ≠ 0 |
| rare / crafted affix slots | v < 0x5A | each read value + 1 (prefix: then + P_v when ≠ 0) |
| set file index | v < 0x5D | the 12 bits are a `sets` row (§3 rule 8) |
| trailer | save and v > 0x56 | read (W §5 rule 2); 32 more bits dropped when v > 0x5D |
| durability (72) | v < 0x60 | 8 bits (else bits(72)) |
| quantity (70) | v < 0x51 | 8 bits (else 9) |
| set list mask | v > 0x54 | the 5-bit mask is read for quality 5; else no set lists |
| stats 1, 3 | v < 0x56 | carry maxmana / maxhp (§4 rule 3) |
| stats 48, 49, 51, 55, 57, 58 | v < 0x52 | widths 6, 7, 7, 7, 7, 8 |
| stat 134 | v < 0x5A | 16 bits dropped, value 1 |
| stats 83–87, 107–109, 126, 179–213 | v < 0x5D | old packed forms (§4 rule 5) |
| stats 204–213 | v < 0x4A | 29 bits (else 30), inside the old form |
| stats 23, 24, 74, 79, 80 | v < 0x5A | widths / offsets of §4 rule 6 |
| stats 93, 96, 99, 102, 105 | v < 0x58 | width 6 |
| stats 92, 94, 95, 97, 98, 100, 101, 103, 104, 106 | v < 0x5D | stored under another id (§4 rule 6) |
| class skills | v < 0x5D | equal per-class values become stat 127 (§4 rule 8) |
| compact quest difficulty | v > 0x5C | read (§5 rule 4); else absent |

3. **Affix offsets** (`0x0062CDC4`, jump table `0x0062E294` /
   `0x0062E280`). P and A are the current part starts (747 and 1,416 in
   1.14d, W §4.1 rule 11, §4.2):

<!-- rows -->
| v | d_P | d_A | P_v / A_v in 1.14d |
|---|---|---|---|
| 0x48, 0x49, 0x50, 0x51, 0x52 | 661 | 1,262 | 86 / 154 |
| 0x53, 0x54 | 674 | 1,265 | 73 / 151 |
| 0x55, 0x56, 0x57 | 675 | 1,266 | 72 / 150 |
| 0x58 | 729 | 1,440 | 18 / −24 |
| any other (0x4A–0x4F, 0x59–0x60) | 0 | 0 | 747 / 1,416 |

   A_v is used only for v ≥ 0x59 (rule 2), so A_v for 0x48–0x58 is
   never applied.

### 3. Full record (`0x0062CBE0`)

Field order as W §4; each step names only what differs from the writer.

1. 10 bits item format → item data +0x30 (`0x0062A6C0`); 3 bits mode
   (unit mode `0x00624690`); location as W §4.1 rules 2–3 (x, y to the
   static path, body location +0x44, page := value − 1 at +0x45).
2. 32 bits code; class := its index when ≥ 0 (else the allocated class
   stays). Alt: item level 1, quality 1, success; nothing more.
3. 3 bits child count (returned). Save: 32 bits → unit +0x28, item seed
   initialised from it (`0x00650E40`, `sim/rng.md`).
4. 7 bits item level (< 1 → 1); 4 bits quality → +0x00; 1 bit, then 3
   bits gfx variant (+0x49) when 1.
5. 1 bit; when 1, 11 bits auto affix → +0x36 with §2 rule 2; when 0,
   +0x36 := 0.
6. By quality ("shown" = save or F & 0x10):
   1. 1, 3: 3 bits file index.
   2. 2: type `char` and shown: 1 bit; 1 → 11 bits + P_v (when ≠ 0) →
      prefix slot 0; 0 → 11 bits → suffix slot 0. Type `body` and not
      `play`: 10 bits file index. Type `scro` or `book`: 5 bits → suffix
      slot 0.
   3. 4, shown: 11 bits + P_v (when ≠ 0) → prefix slot 0; 11 bits →
      suffix slot 0 (no + 1 at any version).
   4. 5, shown: rule 8. 7, shown: 12 bits; ≥ the `uniqueitems` count →
      −1; → file index.
   5. 6, 8: shown: 8 + 8 bits rare names (+0x32, +0x34). Always, for i
      = 0, 1, 2: 1 bit, then 11 bits prefix (§2 rule 2) → +0x38 + 2i,
      else 0; 1 bit, then 11 bits suffix (+ 1 when v < 0x5A, no offset)
      → +0x3E + 2i, else 0.
   6. 9, shown: 8 + 8 bits rare names.
   7. Any other quality: nothing read, the record is marked failed
      (result −1 at the end) but reading goes on.
7. F & 0x4000000: 16 bits → item data u16 +0x38. Ear (F & 0x10000): 3
   bits file index, 7 bits ear level (+0x48), 7-bit characters into
   +0x4A until a 0 (stored). Else F & 0x1000000: the name the same way.
   Then the trailer (§2 rule 2).
8. **Set item file index** (quality 5, shown): 12 bits n. v ≥ 0x5D:
   file index := `setitems` row n's index (+0x00, i16); no row → failed.
   v < 0x5D: `sets` row n (`0x00483410`); its member list (count +0x0C,
   pointers +0x110, `data/fixups.md` setitems rule 2) in order; the
   first member whose `item` code (+0x28) equals the record's code
   gives file index := that member's +0x00; no row or no match → failed.
9. Type values, column set of §2 rule 1:
   1. `armo`: bits(31) → stat 31 := value − add(31); then the rebuilt
      values of `world/vendors.md` §7.3.1 rule 2; bits(73) → stat 73;
      when ≠ 0: durability width of §2 rule 2 → stat 72 := value −
      add(72).
   2. Else `weap`: the rebuilt values of `world/vendors.md` §7.3.1 rule
      1, then 73 and 72 as in 1.
   3. Else `gold`: 1 bit, then 32 or 12 bits → stat 14.
   4. Items `stackable` (+0x132): 8 or 9 bits (§2 rule 2) → stat 70.
   5. F & 0x800: bits(194) → the socket setter `0x0062BE00` (count :=
      min(max(value, 1), min(`invwidth` × `invheight`, 6), max sockets);
      w × h = 0 → nothing; sets flag 0x800). No add.
   A missing items row in 1 or 2 is fatal (0x18B6 / 0x18CD).
10. Not save and F lacks 0x10: the lists are not read; result as rule
    12.
11. Lists: L := 5 when v > 0x54 and quality 5 (then 5 bits mask m),
    else 0; + 1 for a runeword. For c = −1 … L − 1 as W §4.6 rule 3,
    each list taken or created (state, flags) and read by §4; a set
    slot whose mask bit is clear is skipped (nothing read).
12. Result: 1, or −1 when any step marked the record failed.

### 4. Stat-list entries (`0x0062D93C`–`0x0062E1D3`)

1. Repeat: 9 bits id s. 0x1FF → end of the list. s ≥ the
   `itemstatcost` row count or no row → the list ends **without** a
   failure (the remaining bits are then read as whatever follows). s =
   0 directly after an entry with s = 0 → the record fails and the list
   ends. Else the case below, then the next id.
2. "ISC(s)" = read bits(s); value := read − add(s) (no `ValShift`);
   set (`0x00627150`, layer 0) in the list.
3. Fixed cases (every version):
   1. 0, 2: ISC(s).
   2. 1: ISC(1); v < 0x56 also ISC(9) stored as is (not shifted).
      3: ISC(3); v < 0x56 also ISC(7) × 256.
   3. 17: raise of `world/vendors.md` §7.3.1 rule 3 (`0x0062C9F0`(17)),
      ISC(17), raise (18), ISC(18).
   4. 48: ISC(48), ISC(49). 50: ISC(50), ISC(51). 52: ISC(52), ISC(53).
      54: ISC(54), ISC(55), ISC(56). 57: ISC(57), ISC(58), ISC(59), then
      stat 326 := 1. Widths for v < 0x52: §2 rule 2.
4. The 9-bit partners (18, 49, 51, 53, 55, 56, 58, 59) met alone fall to
   rule 6 (W §4.6 rule 4.3 writes them in the group).
5. **Old packed forms (v < 0x5D only; v ≥ 0x5D → rule 6):**

<!-- rows -->
| Ids read | Bits | Stored |
|---|---|---|
| 83, 84, 85, 86, 87, 179, 180 | 3 | stat 83, layer 0, 3, 2, 1, 4, 5, 6 respectively, := value |
| 107, 108, 109, 181–187 | 14 | skill := bits 0–8, level := bits 9–13; when 0 < level ≤ 2^`save bits`(107) − 1 (7 in 1.14d) and the skill row exists with a valid `skilldesc` (`0x00627AF0`): stat 107, layer skill, := level; else nothing |
| 126 | 4 | stat 126, layer 1 |
| 188–193 | 10 | n := bits 0–4: class := n / 3, tab := n mod 3; value := min(bits 5–9, 7); stat 188, layer tab + 8 × class |
| 195–197, 198–200, 201–203 | 21 | skill bits 0–8, level bits 9–13, chance bits 14–20; stat 195 / 198 / 201, layer 64 × skill + level, := chance |
| 204–213 | 29 (v < 0x4A) or 30 | skill bits 0–8, level bits 9–13, charges bits 14–21, max charges bits 22–29; stat 204, layer (skill << shift) + (level & mask) (`items/properties.md` §5 rule 9), := charges + 256 × max charges |

   Also v < 0x5A: id 134 → 16 bits dropped, stat 134 := 1.
6. **Generic** (`0x0062AC80`, ECX v, EDX s, EAX column set, ESI the
   record of s): width w := bits(s), offset a := add(s), then for the
   read id s:
   1. v < 0x5A: 23 → w 5; 24 → w 6; 74 → w 5, a 10; 79 → w 8, a 20;
      80 → w 7, a 20.
   2. v < 0x58: 93, 96, 99, 102, 105 → w 6.
   3. v < 0x5D: the stored id t is 93 for s = 92, 94; 96 for 95, 97;
      99 for 98, 100; 102 for 101, 103; 105 for 104, 106 (the width and
      offset stay those of s). Else t := s.
   4. param := param bits > 0 ? read param bits : 0; value := (read w −
      a) << `ValShift`(s); set t at layer param.
7. Ids that are not special in rules 3–5 always use rule 6 (so at v ≥
   0x5D the reader is the inverse of W §4.6 rule 4 for every id).
8. **Class skills** (v < 0x5D, after each list): x := stat 83 layer 0;
   x > 0 and layers 1, 2, 3, 4 all equal x → stat 83 layers 0–6 := 0
   and stat 127 := x.

### 5. Compact record (`0x0062A970`, EAX unit, EDI reader)

1. 10 bits format, 3 bits mode, location as §3 rule 1.
2. Ear: 3 bits file index, 7 bits ear level, the name as §3 rule 7.
   Else 32 bits code (class := its index); gold: 1 bit, 12 / 32 bits →
   stat 14; type `scro`: code `tsc ` → suffix slot 0 := 0, `isc ` →
   suffix slot 0 := 1.
3. No version gate before rule 4.
4. v > 0x5C, items `quest` ≠ 0 and `questdiffcheck` ≠ 0: `save bits`
   (356) bits (current column, any v) → stat 356 := (value − `save
   add`) << `ValShift` in the (state 0, flag 0x40) list, created when
   missing. Fewer than 0x165 `itemstatcost` rows → fatal 0x1607.
5. Save and v > 0x56: the trailer (§2 rule 2).
6. Unit +0x28 := 0, item seed from it; item level := 1; quality := 2.
   Result 1 (no failure path; no items row → −1 only in rule 4).

### 6. Version-0x47 record (`0x00532F30`, ECX record, EDX bytes)

A 0x48-byte record R is zeroed, a bit reader is set on the 64 bytes
after the record's "JM" (no check against the file length), and 32 bits
F → R +0x24 are read. "s" = sign-extended (`0x00411030`); "lo8" = the
low 8 bits of the read value are stored.

1. **Form A** (F lacks 0x100000), not ear (F lacks 0x10000): 5 bits
   body location (+0x00), 3 bits socket children (+0x01), 12 bits item
   level (lo8, +0x02), 10 bits s items index i, 3 bits mode (+0x0A), 4
   bits quality (+0x0B), 20 bits s quantity (+0x0C), 8 bits min
   durability (+0x10), 8 bits max durability (+0x14), 5 bits x (+0x18),
   3 bits y (+0x1C), 10 bits s file index (+0x20), 32 bits unit seed
   (+0x28), 32 bits item seed (+0x2C), 8 bits page (+0x30). Class
   (+0x06) := the version-0 list slot of i (`0x006335A0`; i outside 0 …
   items count − 1 → −1), code (+0x44) := that items row's `code`;
   i < 0, slot −1 or no row → class −1, code 0. Format (+0x42) := 0.
   Length 25 bytes.
2. **Form A, ear** (F & 0x10000): 5 bits body location, item level :=
   1, 10 bits s dropped, class := `ear `, 3 bits mode, 5 bits x, 3 bits
   y, 10 bits s file index (ear class), 8 bits page, 8 bits ear level
   (+0x31), then up to 16 characters of 7 bits into +0x32, ending after
   the first 0. Code `ear `, format 0. Length 25 bytes.
3. **Form B** (F & 0x100000, not 0x200000): 10 bits format (lo8,
   +0x42), 32 bits code, then the fields of rule 1 from the body
   location to the page, without the items index. Class := the code's
   index (−1 when none). Length 29 bytes (227 bits).
4. **Form C** (F & 0x100000 and 0x200000): 10 bits format (lo8), 5 bits
   body location, item level := 1, 3 bits mode, 5 bits x, 3 bits y, 8
   bits page; ear: code `ear `, 3 bits file index, 8 bits ear level, 16
   characters of 7 bits read with no stop; else 32 bits code. Class
   from the code. Length 13 bytes (98 bits), ear 24 (189 bits).
5. Fields a form does not read stay 0 (quality 0 = "roll it" in the
   request, child count 0, durability 0, seeds 0).

### 7. Version-0x47 creation (`0x00530F40`, ECX game, ESI R)

1. A zeroed 0x84-byte request (`items/generation.md` Inputs): game :=
   ECX; ilvl := R +0x02; item := R +0x06; spawn mode := R +0x0A (byte);
   x, y := R +0x18, +0x1C; room := 0; init flags := 1; format := R
   +0x42; force := 1; quality := R +0x0B (byte); quantity := R +0x0C;
   min / max durability := R +0x10 / +0x14; index := R +0x20; flags1 :=
   R +0x24; seed := R +0x28; item seed := R +0x2C; ear level := R
   +0x31; name := R +0x32 copied up to its 0 (no bound; W Open question
   4); unit, prefixes, suffixes, flags2 0.
2. `0x00558D90` with (game, request, 0) (`items/generation.md` §3, §9,
   §11 for format 0). No item → none.
3. Then file index := R +0x20 (`0x00629DF0`) and page := R +0x30
   (item data +0x45, `0x00628280`), over what creation chose.

### 8. Version-0x47 placement

1. **Player list** `0x00531040`(ECX game, EDX player, R, &parent,
   &children): children := 0; create (§7); none → 0xC. By the new
   unit's mode (allocation from R +0x0A; jump table `0x005311E8`):
   1. 0: mode := 4; position := (R +0x18, R +0x1C); `0x00560200`(game,
      player, item, x, y, find free 0, send 1) (`items/inventory.md`
      §2.4).
   2. 1: mode := 4; `0x005606B0`(game, player, item, L := R +0x00,
      skip 1) (`items/inventory.md` §4.6).
   3. 2: mode := 4; position := (R +0x18, R +0x1C); `0x0055E9B0`(game,
      player, item, slot R +0x18, find 0) (`items/inventory-moves.md`
      §7.14).
   4. 4: mode := 4; cursor := item (`0x0063C180`); `0x0055FB10`(game,
      player, item).
   5. 6: mode := 4; no parent → 0xC; `0x00562660`(game, player, item,
      parent, &out, refresh 0, clear cursor 1, mode check 0, recharge 0)
      (`items/inventory-moves.md` §7.19 steps 2–3).
   6. 3, 5, > 6: 0xC.
   A routine result 0 → 0xC. Success: when R +0x01 ≠ 0, parent := the
   item; children := R +0x01; result 0. The unit of a failure is not
   freed here.
2. **Corpse list** `0x00531390`(game, R, &parent, &children; EBX the
   corpse): create; none → 0xD. By mode:
   1. 0: mode := 4, position from R, `0x00560200`(game, corpse, item,
      x, y, 0, 0, 0).
   2. 1: L := item data +0x44 (`0x00627D40`); L = 0 → 0xD. Put at L in
      the corpse inventory (`0x0063BDB0`), link kind 3 (4 for L 11, 12;
      `0x0063B210`), body location := L, unit flag 0x2 cleared, mode 1,
      page 0xFF, command flag 0x8, unit flag 0x2000000 cleared, update
      list, owner refresh (`0x00621000`(corpse, 1)), quest hook
      ITEMPICKEDUP (`0x00543D80`). A failure → 0xD.
   3. 6: no parent → 0xD; mode 4; `0x00562660` as rule 1.5 with the
      corpse as owner.
   4. Other modes: 0xD.
   Success as rule 1 (parent, children), result 0.
3. The current loader's placements (`0x00531210`, `0x00531520`) take
   the mode and position from the decoded item instead of R, and
   `0x00531210` passes clear cursor 0 to `0x00562660`.

## Constants & data dependencies

| Constant | Value | Where |
|---|---|---|
| column switch | v < 0x5D → 1.09 columns | `0x0062D2F1` |
| affix offset table | §2 rule 3 | `0x0062E294`, `0x0062E280` |
| legacy record | 0x48 bytes, reader window 0x40 bytes, forms 25 / 29 / 13 / 24 bytes | `0x00532F30` |
| request | 0x84 bytes | `0x00530F40` |

Columns: `itemstatcost` `save bits`, `save add`, `save param bits`,
`1.09-save bits`, `1.09-save add`, `ValShift`; items `code`,
`stackable`, `quest`, `questdiffcheck`; `sets` member list; `setitems`
`item`; `uniqueitems` count; `skills` `skilldesc`; the version-0 list.
1.14d 1.09 widths used by the vectors: 1 7 / add 32, 7 8 / 32, 9 8 / 32,
31 10 / 10, 72 8 / 0, 73 8 / 0, 79 9 / 100, 92 6 / 20, 93 7 / 20
(live `patch_d2` `itemstatcost.bin`).

## Randomness

None in the readers. The version-0x47 path draws through the creation
pipeline (`items/generation.md` Randomness, format 0) from the forced
seeds R +0x28 / +0x2C.

## Edge cases & original bugs

1. A stat id beyond the table ends its list with no failure; the rest
   of the record is then misread (§4 rule 1).
2. Two consecutive id-0 entries fail the record; one id 0 is a valid
   entry (strength).
3. For v < 0x56 the maxmana partner of energy is stored unshifted (1/256
   of the value) while the maxhp partner of vitality is × 256 (§4 rule
   3.2).
4. A `nec ` code of a save < 0x5D is created as `neg ` (Hellspawn
   Skull): the full reader's own lookup of `nec ` finds nothing and
   keeps the peek's class (W edge case 10).
5. Versions 0x4A–0x4F get no affix offset correction (§2 rule 3).
6. A quality outside 1–9 is read on to the end and only then fails, so
   the bytes used are those of the misread record (`formats/d2s.md`
   §8.2 rule 2: the entry is skipped).
7. Version 0x47: an equipped corpse item (mode 1) always fails (§8 rule
   2.2): no creation step writes item data +0x44, so L is 0 and the
   corpse list stops with 0xD.
8. Version 0x47: an items index past the version-0 list's filled slots
   (361 in 1.14d) maps to slot value 0, i.e. item 0 (`hax `), not −1.
9. Version 0x47: a socketed child placed after a cursor item clears the
   player's cursor (§8 rule 1.5, clear cursor 1); the cursor item stays
   in mode 4 with no owner slot.
10. Version 0x47 form C ear names are 16 characters read with no stop;
    with no 0 among them the request copy runs on into R +0x42 (W Open
    question 4).

## Test vectors

Synthetic (CI; P = 747, A = 1,416):

| Input | Expected | Source |
|---|---|---|
| v 0x57, rare prefix bit 1, 11 bits 5 | slot 5 + 1 + 72 = 78 | §2 r2, r3 |
| v 0x59, the same | 5 + 1 + 747 = 753; v 0x5A → 752 | §2 r2 |
| v 0x57, rare suffix 11 bits 5 | 6; v 0x5A → 5 | §3 r6.5 |
| v 0x53, magic prefix 10 | 83; v 0x4C → 757 | §3 r6.3 |
| auto affix bit 1, 11 bits 7 | v 0x58 → 0; 0x59 → 1,424; 0x60 → 1,423 | §2 r2 |
| armor, v 0x5C, defense field | 10 bits, value − 10 | §2 r1 |
| max durability 40, v 0x5F | durability in 8 bits; v 0x60 → 9 bits | §2 r2 |
| stackable, v 0x50 / 0x51 | quantity 8 / 9 bits | §2 r2 |
| v 0x55, id 1: 7 bits 42, then 8 bits 37 | energy 10, maxmana 5 (unshifted) | §4 r3.2 |
| v 0x55, id 3: 7 bits 42, then 8 bits 37 | vitality 10, maxhp 1,280 | §4 r3.2 |
| v 0x5C, id 84, 3 bits 2 | stat 83 layer 3 = 2 | §4 r5 |
| v 0x5C, ids 83–87 each 1 | stat 127 = 1; stat 83 layers 0–6 = 0 | §4 r8 |
| v 0x5C, id 92, 6 bits 30 | stat 93 := 10 | §4 r6.3 |
| v 0x59, id 79, 8 bits 45 | stat 79 := 25 | §4 r6.1 |
| v 0x5C, id 188, 10 bits (3 << 5) + 7 | class 2, tab 1 → layer 17, value 3 | §4 r5 |
| ids 0, 0 | record fails | §4 r1 |
| v 0x47 record, F = 0 / 0x10000 / 0x100000 / 0x300000 / 0x310000 | 25 / 25 / 29 / 13 / 24 bytes | §6 |
| v 0x47 form A, items index 400 | class 0 (`hax `) | §6 r1, edge case 8 |

## Provenance

1.14d `Game.exe`, `tools/ghidra/disasm.py fn` of `0x0062E430`,
`0x0062CBE0` (version compares `0x0062CE2A`–`0x0062E12E`, affix jump
table read from the image, stat switch `0x0062D9AE`, generic call
`0x0062E1A9`–`0x0062E1B1`, merge `0x0062E1DA`–`0x0062E24C`), `0x0062AC80`
(switch `0x0062ACB4`), `0x0062A970`, `0x0062AE20`, `0x00558CB0`,
`0x00410F60`, `0x00411030`, `0x00483410`, `0x00483440`, `0x0062BE00`,
`0x00627AF0`, `0x0065C7A0`, `0x0065CB10`, `0x0065CB60`, `0x0065C7D0`,
`0x00533350`, `0x00532F30`, `0x00530F40`, `0x00531040` (jump table
`0x005311E8`), `0x00531390`, `0x00562660` (argument use), `0x006335A0`;
callers of `0x0062E430` / `0x00558CB0` with their pushed version (§1
rule 4); `all.asm` (no version compare in the writer; stores to item
data +0x44 only in `0x00627D70`, `0x0062A970`, `0x0062CBE0`). Widths
from the live `patch_d2` `itemstatcost.bin`. D2MOO not used.

## Open questions

1. PROVISIONAL: items of 1.07 / 1.08 saves decode as §2–§4 (because the rules are read from the 1.14d reader); settled by REC-44 (Deferred).
2. PROVISIONAL: 1.00–1.06 saves decode as §6–§8 and edge cases 7–9 (because the rules are read from the 1.14d reader); settled by REC-44 (Deferred). No such
   save exists on this PC.
