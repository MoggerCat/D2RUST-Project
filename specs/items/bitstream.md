# Spec: Items — Item bit stream (S→C 0x9C / 0x9D item data)

- **Status:** draft: every field read from the 1.14d `Game.exe`
  (`0x006313E0`, `0x006312B0`, `0x0062FFF0`, `0x0062AF80`, addresses
  below); a decoder written from these rules reproduces the exact length
  of all 144 recorded 0x9C / 0x9D item streams of
  `20261006-015956-packets.jsonl` and `20261006-022633-packets.jsonl`
  (every stream ends in its last byte; Test vectors B1–B10).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::items::bitstream` (writer); a reader for
  the client and the conformance harness
- **Related specs:** `items/inventory-moves.md` §11 (the 0x9C / 0x9D message
  header, the flag argument, the filler messages: owner), §6.2
  (dispatcher); `items/generation.md` §1.1–§1.4 (quality ids, item
  format, type tests, item flags); `items/affixes.md` §1 (affix ids);
  `items/properties.md` §9–§10 (fillers, runewords); `sim/stat-lists.md`
  (stat lists by state and flag); `sim/stats.md` (stat getters);
  `data/fields.tsv` (`itemstatcost`, `itemtypes`, `weapons` columns);
  `sim/server-messages.tsv` rows 0x9C, 0x9D; `items/bitstream-legacy.md`
  (the reader by save version).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 42–55 |
| Inputs | 56–66 |
| Outputs / state changes | 67–77 |
| Rules | 78–79 |
|   1. Writer | 80–96 |
|   2. Header (`0x006312B0`) | 97–125 |
|   3. Compact record (`0x0062AF80`) | 126–157 |
|   4. Full record (`0x0062FFF0`) | 158–317 |
|   5. Save-format extras (never on the wire) | 318–335 |
| Constants & data dependencies | 336–359 |
| Randomness | 360–363 |
| Edge cases & original bugs | 364–400 |
| Test vectors | 401–423 |
| Provenance | 424–448 |
| Open questions | 449–520 |
<!-- /index -->

## Summary

An item travels to the client as a little-endian bit stream appended to
S→C 0x9C (from byte 8) and 0x9D (from byte 13). The stream is a 32-bit
flags word, then either a compact record (potions, scrolls, gems, gold,
keys: items with `compactsave`) or a full record: version, mode,
location, item code, socket count, item level, quality, the quality's
fields, names, type-specific values (defense, durability, gold,
quantity, sockets) and, for identified items, the property stat lists,
each ended by 0x1FF. The same writer also produces the save format;
the network stream is the case "save off, children off".
The reader, with its gates for older save versions (0x47–0x5F),
is `items/bitstream-legacy.md`.

## Inputs

| Name | Type | Source |
|---|---|---|
| item unit | type 4 with item data (unit +0x14) | `items/generation.md` |
| item data | quality +0x00, file index +0x28, item level +0x2C, version +0x30, auto affix +0x36, magic prefixes +0x38/+0x3A/+0x3C, magic suffixes +0x3E/+0x40/+0x42, body location +0x44, page +0x45, ear level +0x48, gfx variant +0x49, name +0x4A (NUL-ended), item flags +0x18 | item data record |
| items record | `code` (+0x80), `compactsave` (+0x143), `hasinv` (+0x137), `quest` (+0x12A), `questdiffcheck` (+0x12B), `stackable` | `data/fields.tsv` `weapons` |
| itemtypes record | `varinvgfx` (+0x23) of the primary type | `data/fields.tsv` `itemtypes` |
| itemstatcost | `ValShift` (+0x18), `Save Bits` (+0x19), `Save Add` (+0x1C), `Save Param Bits` (+0x24); record size 0x144 | `data/fields.tsv` `itemstatcost` |
| arguments | save (0 on the wire), children (0 on the wire), alt-code flag | `0x006313E0(item, buffer, 0xF4, save, children, alt)` |

## Outputs / state changes

The stream bytes (whole bytes; the last byte is padded with zero bits).
Side effects on the server's item, kept after the write (every later
read, message and save sees them): a full record of an item whose
quality is not 1–9 writes quality 2 into item data +0x00 (§4.3 rule 7;
"rule 6" in older references), and one whose item level is < 1 writes
1 into +0x2C (§4.1 rule 8; > 99 is only clamped in the stream, not
stored). An implementation whose writer reads a copy must apply both
writes back to the item.

## Rules

### 1. Writer

1. Bits are written **LSB first**: a field of n bits takes bits 0..n−1
   of the value, placed from the current bit position upward; bit 0 of
   the stream is bit 0 of byte 0 (writer `0x00410EB0`, init
   `0x00410E40`, byte length `0x00410E90` = whole bytes, plus one for a
   partial byte).
2. Capacity: the buffer is 0xF4 bytes (1,952 bits). A write that would
   pass the end sets an overflow flag and writes nothing further;
   `0x006313E0` then returns 0 and the message carries no stream (the
   caller asserts the total size ≤ 0xFC).
3. **Clamp** (every numeric field below unless stated): a value v
   written in n < 32 bits is min(v as unsigned 32-bit, 2^n − 1); a
   negative value therefore becomes 2^n − 1. n = 32 writes v as is.
4. "ISC(s) value v" = clamp((v) + `Save Add`(s)) in `Save Bits`(s)
   bits (helper `0x0062AF50`; the same rule inline elsewhere).

### 2. Header (`0x006312B0`)

1. F := item flags (+0x18) with bit 0x80000 cleared and bit 0x800000
   set. Items `compactsave` ≠ 0 → F |= 0x200000. Alt-code flag ≠ 0 →
   F := (F & ~0x400000) | 0x2000000.
2. Save format: 16 bits 0x4D4A ("JM") first. Network: if F lacks 0x10
   (not identified), clear 0x800 (socketed) in F.
3. 32 bits F.
4. F has 0x200000 → compact record (§3); else full record (§4).
5. Save format with children: each item of the item's own inventory
   (list order) follows as a complete stream (§2, recursively). Never
   on the wire.
   Exact condition (`0x00631382`–`0x006313CC`): alt-code flag = 0,
   children ≠ 0 and the unit's inventory (unit +0x60) ≠ none; then for
   every item of that inventory (`0x0063B2C0` first, `0x0063DFA0` next,
   `0x0063DFD0` the item), compact or full parent alike and whatever the
   parent's `hasinv`: the writer first pads to a whole byte
   (`0x00411070`: when the bit position is inside a byte, skip to the
   next byte), then writes the child through `0x006312B0(child, buffer,
   save, children, alt 0)`. A child therefore goes through the same
   `0x0062FFF0` / `0x0062AF80`, so the write-backs of §4.1 rule 8 (item
   level < 1 → 1) and §4.3 rule 7 (quality outside 1–9 → 2) are applied
   to every child item too. Reader side: the header peek `0x0062AE20`
   gives the child count as 3 bits read after the item code only when
   the flags lack both 0x200000 (compact) and 0x2000000 (alt-code), else
   0; that count is the filled-socket field of §4.1 rule 6 (written from
   the inventory count only when `hasinv` ≠ 0), and the game never checks
   it against the number of child streams that follow.

### 3. Compact record (`0x0062AF80`)

1. 10 bits version (item data +0x30, `0x0062A670`; clamped to 0x3FF).
   The version is the item format of `items/generation.md` §1.2: the
   creation pipeline stores the request's format (request +0x2A) at
   item data +0x30 (`0x0062A6C0` called at `0x00558E42`), so both name
   one value (101 in expansion games).
2. 3 bits mode (unit +0x10, clamped to 7), then the location of §4.1
   rules 2–3 (`0x0062AFF0`–`0x0062B1B7`; 92 bits for B1 / B2).
3. Ear (F & 0x10000): 3 bits ear class (file index, `0x00629DA0`), 7
   bits ear level (+0x48), then the name (+0x4A) as 7-bit characters up
   to and **including** the terminating 0.
   Length (2026-10-07): the writer (`0x0062B250` loop) takes bytes from
   +0x4A until a 0 byte, with no bound; item data is 0x74 bytes zeroed
   at allocation (`0x00627C90`; only +0x0C := −1), and `all.asm` has no
   byte or word store with displacement 0x5A or 0x5B at all, so a
   16-character name (no 0 in +0x4A–+0x59) is followed by the 0 at
   +0x5A: the stream holds the 16 characters, then 0. Only the
   version-0x47 ear (edge case 7, Open question 4) writes past +0x59;
   its 17th character then precedes the 0. The reader (`0x0062D298`)
   reads 7-bit characters until it reads a 0 (stored too), no bound;
   the stream's own length is the only limit. d2rs: the setter keeps
   at most 15 characters (edge case 7), so the writer's 16-character
   case does not arise; a reader accepts any length up to its 0.
4. Else: 32 bits item code (`code`, four characters, first character in
   the low byte). Gold (type 4 with equivalence, `0x00629BB0`): 1 bit
   (gold ≥ 0x1000), then the gold (stat 14 total) in 32 bits when that
   bit is 1, else in 12 bits.
5. Items `quest` ≠ 0 and `questdiffcheck` ≠ 0: ISC(356) value of
   (total stat 356 >> its `ValShift`) (`questitemdifficulty`).
6. Save format only: the trailer of §5 rule 2.

### 4. Full record (`0x0062FFF0`)

#### 4.1 Head

1. 10 bits version (+0x30, the item format, §3 rule 1); 3 bits mode
   (unit +0x10).
2. Mode 3 (ground) or 5 (dropping): 16 bits x, 16 bits y (sub-tile
   position, static path +0x0C / +0x10).
3. Other modes: 4 bits body location (+0x44); 4 bits x and 4 bits y
   (the item's grid / slot position, static path +0x0C / +0x10, each
   capped at 15); 3 bits page + 1 (+0x45; page 0xFF → 0).
4. Alt-code flag ≠ 0: 32 bits items +0x84 (the base code; `code` when
   0; `0x006287D0`) and the record **ends**.
   Items +0x84 is the `normcode` column (`data/fields.tsv` `weapons`
   offset 132), so the alt code is the item's normal-tier code. The only
   sender with alt ≠ 0 is the store check of the dispatcher
   (`items/inventory-moves.md` §6.2): `0x0053EF30` with 0x38 (item +0xC8
   bit 2, the vendor item flag of `world/vendors.md` §4) sends S→C 0x9C
   action 0x0B to the trading client with alt = 1 exactly when the item's
   quality is 4–9 (`0x0062A0F0`) and it lacks item flag 0x10
   (`0x006280A0`, `0x0053EF84`–`0x0053EFB1`): the unidentified items of a
   gamble list (`world/vendors.md` §5.1 rule 7). Every other caller passes
   0 (Open question 1). Reader (`0x0062E430`): the flags word is stored
   with 0x2000000 and 0x80000 removed; `0x0062CBE0` with alt reads the
   32-bit code, sets the class from it (`0x00633680`), item level 1 and
   quality 1, and returns (no unit +0x28, no further field, no trailer).
5. 32 bits item code (`code`).
6. 3 bits filled sockets: the number of items in the item's own
   inventory when items `hasinv` ≠ 0, else 0 (`0x0062A900`).
7. Save format only: 32 bits unit +0x28.
8. 7 bits item level (+0x2C; a stored value < 1 is set to 1 in the
   item; above 99 → 99).
9. 4 bits quality (+0x00).
10. 1 bit: the primary type's `varinvgfx` ≠ 0 (`0x0062E8D0`); when 1, 3
    bits gfx variant (+0x49).
11. Auto affix a = +0x36 (u16); a > A → a − A, with A = the automagic
    part's first combined index (1,416 in 1.14d; `0x00633ED0`: table
    +0x10 − +0x04 over 0x90-byte records); 1 bit (a ≠ 0), then 11 bits
    a when 1.

#### 4.2 Affix ids

P = the prefix part's first combined index (747 in 1.14d; table +0x0C −
+0x04 over 0x90). A prefix id p is sent as p − P when p > P, else p;
suffix ids are sent unchanged (suffixes come first in the combined
array, `items/affixes.md` §1).

Reader (`0x0062CBE0`, format version ≥ 0x5A; older versions shift P
and A, `items/bitstream-legacy.md`): a prefix field p′ ≠ 0 → p′ + P,
p′ = 0 → 0; an auto field a′ ≠ 0 → a′ + A (§4.1 rule 11), 0 → 0. The
reader does not invert the writer for a prefix id p ≤ P or an auto id
a ≤ A (sent unchanged, read back + P / + A); an id is its combined
index + 1 (`items/affixes.md` §1 rule 1), so every 1.14d prefix id is
> P and every auto id > A, and the pair round-trips.

#### 4.3 By quality

"Shown" = save format, or F has 0x10 (identified).

1. Quality 1 (low): 3 bits file index (`0x00629DA0`).
2. Quality 3 (superior): 3 bits file index.
3. Quality 4 (magic), shown: 11 bits prefix slot 0 (+0x38, §4.2), 11
   bits suffix slot 0 (+0x3E). Not shown: nothing.
4. Quality 5 (set) and 7 (unique), shown: 12 bits file index (negative →
   0xFFF).
5. Quality 6 (rare) and 8 (crafted): shown: 8 bits rare prefix
   (`0x00627FE0`), 8 bits rare suffix (`0x00628040`). Then, always, for
   i = 0, 1, 2: prefix slot i (+0x38 + 2i, §4.2) as 1 bit (≠ 0) plus 11
   bits when 1; suffix slot i (+0x3E + 2i) the same.
6. Quality 9 (tempered), shown: 8 + 8 bits rare names.
7. Any other quality (2 normal, also 0 or > 9): a quality other than 2
   is **overwritten with 2** in the item and the property lists of §4.6
   are skipped (only the 0x1FF terminator of the main list is written).
   Quality 2 itself keeps its lists: the skip flag is set only on the
   overwrite branch (`0x00630724`–`0x00630737`; B10 is quality 2 with
   stat 107).
   Then: type 13 (`char`, equivalence) and shown: 1 bit (prefix slot 0
   after §4.2 ≠ 0), then 11 bits that prefix, or suffix slot 0 when the
   prefix is 0. Type 40 (`body`) and not type 7 (`play`): 10 bits file
   index. Type 22 (`scro`) or 18 (`book`): 5 bits +0x3E.

#### 4.4 Runeword and names

1. F & 0x4000000 (runeword): 16 bits the runeword record's +0x82
   (`0x0062BED0`; no record → 0xFFFF). The record is the match of
   `items/properties.md` §10.1 (owner); +0x82 is the record's name
   string id (`data/fixups.md` §7).
2. F & 0x10000 (ear): 3 bits ear class, 7 bits ear level, name as in §3
   rule 3. Else F & 0x1000000 (personalized): the name (+0x4A) as 7-bit
   characters including the terminating 0.
3. Save format only: the trailer of §5 rule 2.

#### 4.5 Type-specific values

In this order; "base" = the item's own value (`0x006253B0`), "total"
= `0x00625480`:

1. Type 50 (`armo`): ISC(31) of base 31 (defense); ISC(73) of base 73
   (max durability); when that base is ≠ 0, ISC(72) of total 72
   (durability).
2. Else type 45 (`weap`): ISC(73) of base 73; when ≠ 0, ISC(72) of total
   72.
3. Else type 4 (`gold`): 1 bit (total 14 ≥ 0x1000), then total 14 in 32
   or 12 bits.
4. Items `stackable` ≠ 0: 9 bits total 70 (quantity).
5. F & 0x800 (socketed; already cleared by §2 rule 2 when not shown):
   base 194 (`item_numsockets`) clamped to `Save Bits`(194), **no**
   `Save Add`.
6. Not shown (network, not identified): the record **ends** here.

#### 4.6 Property lists

1. Quality 5 (set): 5 bits mask m, bit i set when the item has a stat
   list of state S_i (states 165–169, table `0x006E90B8`) with flags
   0x2040, or else 0x40 (`0x006257D0`); L := (index of the highest set
   bit) + 1, 0 when m = 0. Other qualities: L := 0.
2. Runeword (F & 0x4000000): L += 1.
3. Lists c = −1, 0, …, L − 1: c = −1 is the main list (state 0, flag
   0x40); c = L − 1 with a runeword is the runeword list (state 171
   `0x00625790(item, 0xAB, 0x40)`); others the set list of S_c (flags
   0x2040, else 0x40).
4. For a list that exists and §4.3 rule 7 did not skip: its own stats
   (`0x00625C90`, up to 511 entries {param u16, id u16, value i32}, list
   order). For each stat s with value v:
   1. Skip when `Save Bits`(s) = 0, or v >> `ValShift`(s) = 0, or the
      value equals the one recorded for s by rule 3 below.
   2. 9 bits s.
   3. s = 17, 48, 50, 52, 54, 57 (grouped): ISC(s) of v >> `ValShift`;
      then the partners, read from the same list (`0x00625D00`, value
      without shift) and written as ISC of that value: 17 → 18;
      48 → 49; 50 → 51; 52 → 53; 54 → 55, 56; 57 → 58, 59. The partner
      values 18, 49, 51, 53, 55, 56, 58, 59 are recorded, so the
      partner's own entry, met later in the list, is skipped when equal.
      The record is per list: all 511 slots are zeroed before each
      list's stats are read (`0x00630E45`), so a partner written in the
      main list does not suppress an equal entry in a set or runeword
      list.
   4. s = 326: nothing more (only the id; unreachable with 1.14d data:
      `poison_count` has `Save Bits` 0).
   5. Other s: when `Save Param Bits` > 0, the param in that many bits
      (clamped); then ISC(s) of v >> `ValShift`.
5. Terminator: 9 bits 0x1FF after list c when c = −1, or the list
   exists, or the item is a runeword (so a runeword item ends every
   list slot, present or not).
6. Reader (`0x0062CBE0`): a set list of mask bit i is found or created
   with flags 0x2040 (never 0x40 alone); the runeword list with state
   171, flags 0x40 (`world/vendors-2.md` §7.3.1 rule 6).
   Lookup `0x00625790(item, state, flags)` (2026-10-08): state ≠ 0 →
   the item's list of that state, active chain first, then parked
   (`0x00625650`, flags ignored); state 0 (the main list) → the first
   active child with flag 0x40 (`0x006256E0`). A list not found is
   allocated with those flags, attached to the item with reset 1 and
   given the state (`0x0062D8F7`–`0x0062D937`); flag 0x2000 puts a set
   list in the item's parked chain (`sim/stat-lists.md` §8.1 step 5),
   so it counts nowhere until the set update unparks it
   (`items/properties.md` §13; client: `client/stat-lists.md` §2 rule
   5). Each stat is stored as (field − `Save Add`) << `ValShift` at its
   param (`0x0062AC80`). The main list of a client item already exists
   (empty) when the record is read (`client/stat-lists.md` §2 rule 1.1).

### 5. Save-format extras (never on the wire)

1. §2 rule 2 (the "JM" marker), §4.1 rule 7, children (§2 rule 5).
2. Trailer (`0x00629E40`, after §3 rule 5 and after §4.4): 1 bit 0, or
   1 bit 1 followed by two 32-bit values and then 32 bits 0.
   The two values are item data +0x1C and +0x20 (`dwRealmData[0..1]` in
   the D2MOO naming; getter `0x00629E40`). The bit is 1 exactly when
   +0x20 ≠ 0; then 32 bits +0x1C, 32 bits +0x20, 32 bits 0 (compact
   `0x0062B341`–`0x0062B387`, full `0x006309CD`–`0x00630A19`). Reader
   (save format only, and only when the save version argument > 0x56:
   compact `0x0062ABF9`–`0x0062AC34`, full `0x0062D2A9`–`0x0062D2EA`):
   1 bit; when 1: 32 bits a, 32 bits b, then when the version > 0x5D 32
   more bits that are discarded; a → +0x1C, b → +0x20 through the setter
   `0x00629EA0`, whose only two callers are these readers (`0x0062AC26`,
   `0x0062D2EA`). No other code path of the exports calls the setter, so
   in a game every item has +0x20 = 0 unless it was read from a save that
   carried the values, and its trailer is the single 0 bit.

## Constants & data dependencies

| Constant | Value | Where |
|---|---|---|
| buffer | 0xF4 bytes | `0x0053EAE0`, `0x0053CEF0` |
| version field | 10 bits; 1.14d expansion items carry 101 | `items/generation.md` §1.2 |
| prefix offset P / automagic offset A | 747 / 1,416 (1.14d combined magic array) | `0x00633ED0` |
| set list states | 165, 166, 167, 168, 169 | `0x006E90B8` |
| runeword list state | 171 | `0x0062FFF0` |
| list terminator | 0x1FF in 9 bits | `0x0062FFF0` |
| save marker | 0x4D4A | `0x006312B0` |

Columns read: itemstatcost `ValShift`, `Save Bits`, `Save Add`, `Save
Param Bits` (1.14d values used by the vectors: 9 `maxmana` shift 8, 8
bits, add 32; 17 / 18 9 bits; 19 10 bits; 21 6 bits; 22 `maxdamage` 7
bits, add 0; 31 11 bits add 10; 48 8 bits; 49 9 bits; 60
`lifedrainmindam` 7 bits, add 0; 72 9 bits; 73 8 bits; 75
`item_maxdurability_percent` 7 bits, add 20; 107 3 bits, param 9 bits;
194 4 bits; 356 2 bits; 22, 60, 75 read from live `patch_d2`
`itemstatcost.txt`, handoff LB1, and the only widths with which B5, B6
and B8 end in their last byte), items `code`, `compactsave`, `hasinv`, `quest`,
`questdiffcheck`, `stackable`, itemtypes `varinvgfx` and the
equivalence chain.

## Randomness

None.

## Edge cases & original bugs

1. Unidentified items lose the socketed flag in the header and their
   socket count (§2 rule 2, §4.5 rule 5), and their affix ids and lists.
2. §4.3 rule 7 rewrites a quality outside 1–9 to 2 on the server's item
   while serializing it.
3. A negative value with a `Save Add` too small becomes all ones (§1
   rule 3).
4. The rare affix slots of quality 6 / 8 are sent even when the item is
   not identified (§4.3 rule 5).
5. Stat 326 has a no-value branch that 1.14d data never reaches.
6. A runeword item writes a terminator for every set-list slot, present
   or not (§4.6 rule 5).
7. Names (§3 rule 3, §4.4 rule 2) are written up to the first 0 byte
   with no length bound (`0x0062B250` loop, `0x006308E9` loop); the
   name setter `0x00628370` and the readers (`0x0062D298`) are unbounded
   copies too. The buffer is 16 bytes (+0x4A–+0x59); the 1.14d names are
   player names (ears, personalization), which fit with their
   terminator, so a 16-character name is not expected (Open question 4: the one exception is a version-0x47 save's ear, up to 17 characters). A name with no 0 in its 16 bytes
   would read on into +0x5A…; d2rs stores at most 15 characters and
   rejects a longer one at the setter instead (handoff BV7).
8. A compact ear has no item code on the wire (§3 rule 4). The reader
   takes the class from code `ear ` for F & 0x10000 (`0x0062AEF1`, the
   header peek), and live `misc.txt` row `ear` has `quest` empty (0), so
   §3 rule 5 never applies to an ear: a reader writes no quest
   difficulty for it (handoff BV9).

9. A gamble list item reaches its client as an alt-code record (§4.1
   rule 4): the client's copy has the normal-tier code, item level 1,
   quality 1 and no affixes, whatever the server item is (an
   exceptional or elite upgrade of `world/vendors.md` §5.1 rule 5 shows
   as its `normcode` base).
10. The header peek `0x0062AE20` replaces the item code `nec ` by `neg `
    when the version argument is < 0x5D (`0x0062AF03`–`0x0062AF10`); the
    full reader `0x0062CBE0` then sets the class from the code it reads
    itself, without that replacement.

## Test vectors

Real (recorded on 1.14d; stream = message bytes from 8 for 0x9C, from 13
for 0x9D; decoded with the rules above; each stream ends exactly in its
last byte):

| Id | Message (frame) | Decoded |
|---|---|---|
| B1 | `9c0e1410010000001000a2006508008006170302` (1) | flags 0xA20010 (compact); version 101, mode 2 (belt), body 0, x 0, y 0, page+1 0; code `hp1 `; 92 bits |
| B2 | `9c041410060000001000a2006500529236370602` (1) | compact; mode 0, x 9, y 2, page+1 1 (inventory); `isc `; 92 bits |
| B3 | `9d061e0508000000000100000011008200658408801686078280c0c1e13f` (1) | owner type 0, GUID 1; flags 0x820011; mode 1, body 4, x 4, y 0, page+1 0; `hax `; sockets 0, ilvl 1, quality 2, no gfx, no auto; max dur 28, dur 28; main list empty (0x1FF); 134 bits |
| B4 | `9d0620060900000000010000001100820065a40a205637068280f0000606ff01` (1) | `buc `, body 5; quality 2; defense 5 (written 15, add 10); max dur 12, dur 12; 145 bits |
| B5 | `9c0b1e050c000000102880006500c0c416860702c3500f1133218025a2ff` (899) | flags 0x802810 (socketed, identified, in store); mode 0, x 0, y 6, page+1 2; `lax `; ilvl 6; quality 3, file index 5; max dur 30, dur 34; sockets 3; list: 19 `tohit` 1, 75 `item_maxdurability_percent` 14; 176 bits |
| B6 | `9c0b210511000000102080006500063437d6060203a18b5b5858b010801140e03f` (899) | `scm `, quality 4: prefix 186, suffix 183; max dur 22, dur 22; list: 22 `maxdamage` 1, 48 + 49 fire damage 1–4; 198 bits |
| B7 | `9c0b1a05140000001020800065000048b766060283404000d47f` (899) | `tkf `, quality 2; max dur 4, dur 4; quantity 160; 143 bits |
| B8 | `9c0b21061d00000010208000650070342776070203b10bb0504c88d0a1030fc27f` (899) | `sbw `, quality 4: prefix 187, suffix 352; max dur 20, dur 19; list: 17 + 18 = 29, 29; 60 `lifedrainmindam` 4; 199 bits |
| B9 | `9c0b1f002300000010208000650040321606070203011300348081418292ff` (899) | `cap `, quality 4: prefix 304, suffix 0; defense 3; max dur 12, dur 12; list: 9 `maxmana` 5 (written 37 = 5 + add 32; the list value >> 8 = 5); 184 bits |
| B10 | `9d062105060000000001000000110082006584083037470782804041610d89fc07` (`20261006-022633`, frame 1) | `sst `, quality 2; list: 107 `item_singleskill` param 36, value 1; 155 bits |

Synthetic (rules; CI): a 1-bit flag then a 12-bit gold of 4,095 →
bits `0` + 12 ones; gold 4,096 → `1` + 32 bits 0x1000. A stat value −1
with `Save Add` 0 in 8 bits → 0xFF.

## Provenance

- 1.14d `Game.exe`: entry `0x006313E0`; header `0x006312B0`; full
  record `0x0062FFF0` (disassembled: location branches, the affix and
  stat-group branches, jump tables `0x0063126C`, `0x00631290`); compact
  `0x0062AF80`; ISC writer `0x0062AF50`; bit writer `0x00410E40`,
  `0x00410EB0`, `0x00410E90`; getters `0x0062A670`, `0x00628590`,
  `0x006287D0`, `0x0062A900`, `0x0062E8D0`, `0x00633ED0`, `0x00627EC0`,
  `0x00627F80`, `0x00627FE0`, `0x00628040`, `0x00629DA0`, `0x0062BED0`,
  `0x006253B0`, `0x00625480`, `0x00625D00`, `0x00625C90`, `0x006257D0`,
  `0x00625790`; senders `0x0053EAE0` (0x9C), `0x0053CEF0` (0x9D),
  `0x0053EA50` (fillers). Table `0x006E90B8` read from the image.
  Re-read for the implementation questions: compact `0x0062AF80` (mode
  bits), `0x0062FFF0` (overwrite flag `0x00630737`, per-list reset
  `0x00630E45`), format setter `0x0062A6C0`, header peek `0x0062AE20`,
  name setter `0x00628370`.
- Recordings: all 144 0x9C / 0x9D of the two packet recordings decode
  to their exact byte length with the 1.14d `patch_d2` tables
  (`itemstatcost.txt`, `itemtypes.txt` without its `Expansion` row,
  `weapons` / `armor` / `misc.txt`); decoder kept in the spec session's
  scratchpad (to be ported as the conformance reader).
- D2MOO 1.10f `ITEMS_SerializeItem` was not used; the community format
  notes agree on the field order but not on the 1.14d header bits (F's
  forced 0x800000) or the clamps.

## Open questions

1. Answered (handoff `pc2-spec-d2s` DS-2a; re-read for this answer): §4.1
   rule 4. The save-format callers of `0x006313E0` all pass 0
   (`0x00531712`, `0x00531899`, `0x005318C3`, `0x005318F0`,
   `0x00531929`, `0x0053195A`, `0x00541B5C`, `0x0055A2E1`), so a game
   save never holds an alt-code record. Of the 25 network callers
   (`0x0053EAE0` 13 sites, `0x0053CEF0` 12) only `0x0053EFB1` (0x9C
   action 0x0B from the store check `0x0053EF30`) can pass 1: an
   unidentified quality 4–9 item shown to the trading client, i.e. a
   gamble list item; all others push 0.
2. Needs recording. The rules for these records are read from the
   1.14d writer and reader (§3–§5); the binary cannot confirm its own
   reading. The recording must show, for each of a set, unique, rare,
   runeword, ear, gold pile, tome and an item with a filled socket: the
   S→C 0x9C / 0x9D bytes when it is picked up and when it is stored
   (inventory R2 / R5 scenarios), so each stream decodes to the end with
   §3–§5 and re-encodes byte for byte (the 0x9D child stream included).
3. Answered (handoff `pc2-spec-d2s` DS-5): §5 rule 2. Item data +0x1C
   and +0x20 (D2MOO `dwRealmData`), written only when +0x20 ≠ 0; read only
   when the save version > 0x56, with one discarded u32 when > 0x5D; the
   setter `0x00629EA0` is called only by the two save readers.
4. Answered (2026-10-07, disassembly of the 20 callers of `0x00558D90`
   and of the legacy save reader): the edge-case-7 claim does **not**
   hold for one path. Of the 20 callers (`disasm.py xref 0x558D90`: all
   direct, no pointer), only `0x00530F40` sets the request's forced
   field +0x2C (`0x00530FDA`, := 1); the 19 others build their request
   at ebp−0x88 (`0x00579D60` at ebp−0xEC) and never store to +0x2C or
   to +0x58..+0x67, so after the zeroing memset they reach the
   unforced branches (`0x005590A4`, player name). `0x00530F40` (callers
   `0x00531040`, `0x00531390`, both only from `0x00533350`) copies the
   name with an unbounded byte loop (`0x00530FE3`–`0x00530FED`) from
   +0x32 of a 0x48-byte legacy record into request +0x58; that record
   is filled by `0x00532F30`, the reader `formats/d2s.md` §8.2 rule 1
   selects for save version 0x47. Its ear branches read 16 characters
   of 7 bits each into +0x32..+0x41: in the "flags 0x100000 clear"
   ear branch the loop stops at the first 0 and +0x42 (u16) was set to
   0, so at most 16 characters; in the "0x100000 set, 0x200000 set,
   0x10000 set" branch all 16 are read with no stop, and +0x42 holds
   the low byte of a 10-bit field read before them (+0x43 = 0), so a
   version-0x47 ear name can be 17 characters (16 + that byte) before
   its terminator. `0x00558D90` then passes request +0x58 to the
   setter `0x00628370` (unbounded) at `0x0055903B` (type 7 ear) and
   `0x0055910E` (flag 0x1000000), so up to 18 bytes (17 + 0) are
   written from item data +0x4A, i.e. 2 bytes past the 16-byte field
   (+0x5A, +0x5B). Every non-legacy path passes at most 15 characters
   (the five sites below). d2rs (edge case 7: at most 15 characters,
   rejected at the setter) therefore differs from 1.14d only for a
   version-0x47 save holding an ear whose name has ≥ 16 characters; the
   rejection stays a Ruleset choice and is recorded as such.
   Earlier partial answer (source of each call site, disassembled):
   `0x005590A4` and `0x0057A625` copy the player's name (player data
   +0x00, `0x006221A0`), which the character load bounds to 15
   characters (`formats/d2s.md` load rule 4: byte +0x23 := 0, and it must
   equal the client name); `0x00567086` and `0x005670CC` (`0x00567070`)
   copy another item's name (`0x00628340`, item data +0x4A); `0x0057A12B`
   copies a name that `0x00579D60` first copied from the same item
   (`0x0057A073`–`0x0057A08C`, unbounded byte loop into a stack buffer
   at ebp−0x68). So those five pass at most 15 characters whenever the
   names already held are. Open: `0x0055903B` (ear, forced request) and
   `0x0055910E` (personalized, forced request) copy the request's name
   field +0x58 (16 bytes; `items/generation.md` Inputs); the request
   builders seen zero the request (`0x00681EF0`); a first scan of the
   21 callers of `0x00558D90` found no direct store to +0x58, which does
   not rule out a copy through a pointer. Settle: find the writer
   of request +0x58 for an ear (player death drop) and check its bound.
   Original text: Edge case 7: that every caller of the name setter
   `0x00628370` passes a name of at most 15 characters.
5. Answered (handoff `impl-bitstream-vitals` BV1-BV4, BV5, BV7, BV9,
   LB1): §3 rules 1-2, §4.3 rule 7, §4.6 rule 4.3, §4.4 rule 1
   (`items/properties.md` §10.1), edge cases 7-8, Constants.
