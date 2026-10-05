# Spec: Data — Field types and txt-to-record compilation

- **Status:** draft. Checked against 1.14d: 92 field lists recovered from
  `Game.exe`; compiling the live `.txt` files with these rules reproduces
  82 live `.bin` tables byte for byte (see Provenance). Implemented in
  `d2-data::compile` (callbacks of §8.3 not yet: they write nothing); the
  d2rs cross-check reproduces the §10 result (confirmed by bin
  cross-check, `loading.md` §11).
- **Target version:** 1.14d
- **Crate/module:** `d2-data::compile` (suggested: type vocabulary, record
  compiler, linkers, string keys, callbacks)
- **Related specs:** `specs/data/txt-format.md` (splitting text into
  cells, column binding; its §7–8 describe the same conversions by type ID
  and must agree with this spec), `specs/data/loading.md` (which file is
  live, the `.bin` container, load order, link dependencies, code buffers),
  `specs/formats/tbl.md` (string tables), `specs/data/calc-expressions.md`
  (formula bytecode), per-table specs (field lists, table callbacks; not
  yet written, Open question 1).

Implementable now: §1–§9 with the synthetic test vectors. §10 runs per
table once its field list is published.

## Summary

1.14d keeps every data table as fixed-size binary records. The records
come from the table's `.txt` through one generic compiler, driven by the
table's *field list*: for each field a column name, a type, a length, a
record offset and a link. This spec defines the record side of that
compiler: the type vocabulary that per-table specs use, the exact bytes
each type writes for a cell, an empty cell and a missing column, the
integer conversion, the code and name key tables ("linkers") behind
cross-references, string-key resolution, callback fields, the record
layout, and how to compare a compiled `.txt` with a shipped `.bin`.
Splitting text into cells and binding columns to fields are in
`txt-format.md`.

In normal play 1.14d reads only `.bin` files (`loading.md` §3). It runs
the compiler at runtime only for `sounds.txt` and `soundenviron.txt`, and
for every table under the `-txt` switch. d2rs needs the compiler for mod
patch layers, which edit cells, and its output must be the bytes the game
would produce.

## Inputs

| Name | Type | Source |
|---|---|---|
| header and records of cells | byte strings | `txt-format.md` §4–5 |
| field list | ordered (column, type, length, offset, link) | per-table spec |
| record size | bytes | per-table spec, `loading.md` §6 |
| linkers | key → index tables | own keys of tables compiled earlier (§6) |
| string tables | `patchstring.tbl`, `expansionstring.tbl`, `string.tbl` | `tbl.md`, `loading.md` §10.1 |
| code buffers | growable byte buffers | calc fields (§8.1) |

## Outputs / state changes

- `count × size` record bytes (`count` from `txt-format.md` §5).
- Own-key fields add keys to their linker.
- Calc fields append bytecode to a code buffer.

## Rules

### 1. Field lists

| Part | Meaning |
|---|---|
| column | header name the field reads; binding per `txt-format.md` §6 (ASCII case-insensitive, whole name, leftmost duplicate column wins) |
| type | type ID 0–26 (§3) |
| length | per type: string length, bit number, callback argument; else 0 |
| offset | byte offset in the record; callbacks get it passed through |
| link | a linker (own-key and link types), a callback (types 22–25), else none |

- The list ends at its first entry of type 0. At most 280 fields, the end
  entry not counted.
- Field names are unique within every 1.14d list. d2rs rejects a list with
  duplicate names (the original binds them in reverse list order,
  `txt-format.md` §6).
- A field bound to no column is *missing*.
- Header columns + missing fields must be ≤ 280 (the original keeps both
  in one 280-slot map and would overrun it).
- In 1.14d every 2-byte field sits at an even offset and every 4-byte field
  at a multiple of 4. The compiler itself does not require it.
- Errors use the codes of `txt-format.md` §9: a field list that fails
  `txt-format.md` §6.1 (size, type IDs, names, record size, `u8?` runs,
  linker kinds and existence, callbacks, field footprints) is E13; too
  many slots E14; zero records E3; bad name-key bytes E11; `key(code1/2)`
  overflow E12. This spec adds no codes.

### 2. Compile procedure

```
records = count × size bytes, all 0x00
if some bound column's field has type 10, 12, 14, 16, 17 or 18:   # own keys
    pass 1: for each record r in file order:
                for each bound column, left to right:
                    if its field has one of those types: apply it (§5–6)
pass 2: for each record r in file order:
            for each bound column, left to right, not an own-key type:
                apply its field's type to the cell (§4–8)
            for each missing field, in field-list order:
                write its missing-column value (§3); callbacks run with no text
```

- Pass 1 finishes for all records before pass 2 starts, so a table can
  look up its own keys, later rows included.
- Writes land in the order above. Where fields overlap, the later write
  wins. 1.14d lists have overlaps: `levels` `camt1`–`camt4` are all u16 at
  offset 220 (the rightmost bound column wins); a `str(N)` terminator can
  land on the next field (`pettype` `baseicon` str(32) at 15 and `micon1`
  at 47; `treasureclassex` `Treasure Class` str(32) at 0 and `Picks` at
  32). A missing field is written after the record's bound columns, so it
  wins over a bound field it overlaps.
- Fatal in the original: record count 0 (E3), record size 0 (E13).

### 3. Type vocabulary

Per-table specs give each field's type as one of these words. IDs in the
same row compile identically; a spec may add the ID for cross-checking.
`u`/`i` only says how the game reads the bytes; the compiled bytes are the
same. "−1" means 0xFF, 0xFFFF or 0xFFFFFFFF by width.

| Vocabulary | ID | D2MOO name | Pass | Bytes | Bound cell | Missing column | 1.14d |
|---|---|---|---|---|---|---|---|
| `u8` / `i8` | 4, 6 | BYTE, UNKNOWN2 | 2 | 1 | integer (§4), low 8 bits | 0 | both used |
| `u16` / `i16` | 3 | WORD | 2 | 2 | integer, low 16 bits | 0 | used |
| `u32` / `i32` | 2, 8 | DWORD, DWORD2 | 2 | 4 | integer | 0 | both used |
| `u8?` | 5 | UNKNOWN1 | 2 | 1 | integer, low 8 bits; empty cell writes nothing | 0 | unused |
| `bit(n)` | 26 | BIT | 2 | 1 bit | integer ≠ 0 sets bit n, 0 clears it | nothing | used |
| `str(N)` | 1, 7 | ASCII, BYTE2 | 2 | ≤ N+1 | text + NUL (§5.1) | byte 0 = 0 | 1 used, 7 unused |
| `code4` | 9 | RAW | 2 | 4 | code (§5.2) | `00 00 00 00` | used |
| `key(code4)` | 10 | ASCIITOCODE | 1 | 4 | code; added to own linker | `00 00 00 00`, no key | used |
| `key(code1)` | 12 | UNKNOWN4 | 1 | 1 | code; added; first code byte | 0 | unused |
| `key(code2)` | 14 | UNKNOWN5 | 1 | 2 | code; added; first 2 code bytes | 0 | unused |
| `key(str(N))` | 16 | UNKNOWN6 | 1 | ≤ N | first N−1 bytes + NUL; added (add-always) | byte 0 = 0 | unused |
| `key(name16)` | 17 | NAMETOINDEX | 1 | 2 | name; find-or-add; index | 0, no key | used |
| `key(name32)` | 18 | NAMETOINDEX2 | 1 | 4 | name; find-or-add; index | 0, no key | used |
| `link8(K)` | 13 / 21 | CODETOBYTE / NAMETOWORD2 | 2 | 1 | index in K, low 8 bits; miss −1 | −1 | used |
| `link16(K)` | 15 / 20 | CODETOWORD / NAMETOWORD | 2 | 2 | index in K, low 16 bits; miss −1 | −1 | used |
| `link32(K)` | 11 / 19 | UNKNOWN3 / NAMETODWORD | 2 | 4 | index in K; miss −1 | −1 | used |
| `strkey` | 22 | KEYTOWORD | 2 | 2 | string ID (§7) | 0 | used |
| `cb(F)` | 23, 24, 25 | CUSTOMLINK, UNKNOWN7, CALCTODWORD | 2 | any | callback F (§8) | F with no text | 23, 25 used; 24 unused |
| `end` | 0 | NONE | — | — | ends the list | — | used |

- **Link IDs.** A code linker K gives IDs 13 / 15 / 11, a name linker K
  gives 21 / 20 / 19. The vocabulary needs no ID: K decides.
- **K notation.** `<table>.<column>` names the own-key column that fills the
  linker (its type is a `key(...)`); `items.code` is the one linker filled
  by three tables (§6.4). `@<name>` names a hand-built linker (§6.4).
  Example: `link16(itemtypes.code)`. A linker is identified by its key
  column: where 1.14d has two lists on one name column (a compile-only
  and a runtime list on `skills.skill`, `monstats.Id`,
  `skilldesc.skilldesc`; `loading.md` §7.2), find-or-add gives both the
  same indices, so d2rs builds one linker. A code linker is filled by
  exactly one list, except `items.code`; d2rs never fills one twice
  (a second fill would bump every key).
- **Callback notation.** `calc(<buffer>)` (§8.1), `param` (§8.2), or
  `cb(<table>.<name>)` for a callback defined in that table's spec.
- D2MOO's `NAMETOWORD2` writes **1** byte in 1.14d (`skills` `pettype` at
  190; the next field, `summode`, is at 191).
- `key(code1)` and `key(code2)` store code bytes, not the index.
  `key(name16)` stores the low 16 bits of the index, unchecked, as 1.14d
  does (largest 1.14d name linker: 4,699 keys).
- "Unused" = the type ID appears in none of the 92 recovered 1.14d lists.
  Its rules come from the 1.14d code alone.

Example per-table column table:

| Column | Offset | Type |
|---|---|---|
| `code` | 128 | `key(code4)` (fills `items.code`, shared by weapons, armor, misc) |
| `namestr` | 244 | `strkey` |
| `type` | 286 | `link16(itemtypes.code)` |
| `mindam` | 254 | `u8` (ID 6) |
| `calc1` | 164 | `calc(itemscode)` |

### 4. Integers (`u8` … `u32`, `u8?`, `bit`)

The D2 integer rule. It is not C `atoi`:

```
neg = the cell's first byte is '-' (0x2D);  if neg: skip that byte
v = 0                                    # 32 bits, wrapping
for each remaining byte b:
    s = b if b < 0x80 else b − 256       # bytes are signed
    v = v × 10 + s − 48
if neg: v = −v
```

- Every byte counts as a digit. Spaces, `+`, `.`, `x` and letters are not
  special and the loop never stops early. The empty cell gives 0. There is
  no range check: the field keeps the low 8, 16 or 32 bits.
- 1.14d data has such cells, and the shipped bins hold exactly what the
  rule gives (`misc` `TMogMin` `" "` → 0xF0, `misc` `rarity` `999` → 0xE7).
- `u8?`: the same, except that an empty cell writes nothing (the byte keeps
  its value, normally 0). `"-"` writes 0. The original also requires a
  `u8?` entry with length L > 1 to be followed by L − 1 more `u8?` entries
  (exact rule and E13: `txt-format.md` §6.1).
- `bit(n)`: n is the field's length. Byte `offset + (n >> 3)`, mask
  `1 << (n & 7)`. A value ≠ 0 sets the bit, 0 clears it. So n numbers the
  bits of a little-endian bit string that starts at `offset` (`skills` uses
  n = 0–38 at offset 4). `" "` is −16, so a lone space sets the bit.
  Missing column: nothing is written.

### 5. Text, codes and names

**5.1 `str(N)`.** N is the field's length.
- Copy the first min(L, N) cell bytes to `offset`, then write 0x00 at
  `offset + min(L, N)`. Longer text is cut at N bytes without warning.
- The field can use N + 1 bytes. Bytes after the NUL are not written (they
  stay 0).
- Empty cell → one 0x00. Missing column → the byte at `offset` is 0.
- Lists usually give N = room − 1 (`lowqualityitems` `Name` str(31) in a
  34-byte record), but not always: `pettype` `baseicon` is str(32) with
  `micon1` 32 bytes later, so a 32-byte text puts its NUL on `micon1`'s
  first byte (§2 decides which write wins).

**5.2 Code.** The first min(L, 4) cell bytes, padded on the right with
spaces (0x20) to 4 bytes. Case is kept; bytes past the 4th are dropped.
The empty cell gives `20 20 20 20`. As a u32 it is little-endian (first
character = low byte). `code4` writes these 4 bytes and ignores any link in
the field list. A missing `code4` or `key(code4)` column writes
`00 00 00 00`: zeros, not spaces.

**5.3 Name.** The first min(L, 31) cell bytes, then each byte `A`–`Z`
(0x41–0x5A) becomes `a`–`z`. Other bytes are unchanged. Bytes ≥ 0x80: the
original indexes its lowercase table with a signed byte and reads
unrelated memory (mostly 0, which ends the key). No 1.14d key has such
bytes; d2rs rejects a byte ≥ 0x80 within the key's first 31 bytes, in
every name key including `param` lookups (`txt-format.md` E11).

### 6. Linkers (key → index)

A linker is a key table of one kind, code or name, with a counter `n` that
starts at 0. The table whose own-key field fills it owns it (or a loader,
§6.4). In the original a code operation on a name linker, or the reverse,
is fatal or crashes, and a lookup in a linker that was never created
crashes; d2rs: E13.

**6.1 Code linker** (filled by `key(code4/1/2)`, read by IDs 11, 13, 15):
- *add(code):* `k = code`; while `k` is already present: `k = k + 1`
  (u32, wrapping). Store `k → n`. Result `n`; then `n += 1`.
  - Every add takes the next index, so when each record adds one code,
    index = record number. Empty cells add `"    "` like any code.
  - A duplicate is stored under a bumped value and can be found only by
    that value: adding `abc ` twice stores the second as `bbc `
    (0x20636262). The plain code finds the first record. The record keeps
    the code as written, not the bumped value.
  - `key(code1)` / `key(code2)`: E12 if the result exceeds 255 / 65,535.
- *find(code):* exact 4-byte match → its index, else −1. Case-sensitive.
- 1.14d: `itemtypes.txt` rows 0, 1, 14, 17 and 23 have empty codes. Row 0
  owns `"    "`, so an empty `link16(itemtypes.code)` cell gives 0, not −1.
  24,442 empty code lookups in the live tables hit a key this way
  (`txt-format.md` §8).

**6.2 Name linker** (filled by `key(name16)`, `key(name32)`,
`key(str(N))`; read by IDs 19, 20, 21). Keys are normalized names (§5.3):
case-insensitive, first 31 bytes.
- *find-or-add* (types 17, 18): if the key exists, the result is its index
  and `n` does not change. Else store `key → n`, result `n`, `n += 1`.
  - A duplicate gets the first occurrence's index and takes no index of its
    own, so after a duplicate, index ≠ record number. 1.14d: `monstats`
    record 723 `cr_lancer8` stores 617 (record 617 has the same Id);
    `monseq` has 1,010 records and 60 distinct `sequence` names, stored as
    0–59.
- *add-always* (type 16): `n += 1` every time; the key is stored only if
  new.
- *find(name):* its index, or −1.
- The empty name is a key. 1.14d: record 0 of `sounds`, `montype`,
  `monsounds` and `monseq` has an empty key, so empty lookups into them give
  0 (7,638 live cells). `@tc` holds the empty name at index 0 (§6.4), so
  the 4,455 empty `TreasureClass*` / `TC*` cells give 0.
- Names longer than 31 bytes are compared on their first 31. `sounds.txt`
  has 33 such names (`barbarian_act1_complete_andariel` → key
  `barbarian_act1_complete_andarie`); none collide.

**6.3 Lookups** (pass 2). The cell is turned into a code (§5.2) or a name
(§5.3) according to K's kind and looked up. A miss gives −1, cut to the
field width. There is no range check: an index ≥ 256 in a `link8` keeps
its low byte. 1.14d has 14 non-empty cells that miss (e.g. `*enr`,
commented-out property codes); they hold −1.

**6.4 Linker sources in 1.14d.**
- `weapons`, `armor` and `misc` share one code linker, filled in that
  order by their `code` columns; specs call it `items.code`. Item index =
  weapons row, 306 + armor row, 508 + misc row (1.14d counts
  306 / 202 / 151). `runes` record 0 `Rune1` = `r08` → 617 (misc row 109).
- `properties.code` is a **name** key (`key(name16)`): property codes are
  case-insensitive.
- `missiles.Missile` is `key(name16)` at offset 0.
- Hand-built linkers (no field list fills them):
  - `@tc` (name, add-always, filled by the TC routine in step 46,
    `loading.md` §10.6): index 0 = `""`; then the automatic TCs; then the
    `treasureclassex` names in record order up to the first empty name.
    1.14d: 1,013 indices; `treasureclassex` record `i` → 161 + `i`. Read
    by monstats `TreasureClass*` and superuniques `TC*` (`link16(@tc)`).
  - `@skillrange` (code): `none`, `h2h`, `rng`, `both`, `loc` → 0–4,
    built inside step 10 (`loading.md` §10.3); read by skills `range`.
- The compile-only `key(name16)` lists on `monstats.Id` and
  `skilldesc.skilldesc` inside step 10 are ordinary lists; they equal
  those linkers (§3 K notation).
- Which table fills and reads which linker: `loading.md` §7.

**6.5 Order.** A lookup sees only the keys added so far. d2rs compiles in
the 1.14d order: tables in `loading.md` §6 order, the compile-only lists of
`loading.md` §7.2 at their positions; per table, pass 1 then pass 2;
hand-built linkers built where their loader builds them (`@skillrange` in
step 10, `@tc` in step 46 before `monstats`). So `items.code` is filled
weapons, armor, misc, and calc appends follow this order
(`calc-expressions.md` §1.4). Every link targets an earlier step
(`loading.md` §7.3), so running every pass 1 before any pass 2 gives the
same bytes for the 1.14d lists (the Provenance model did this); d2rs does
not rely on it. A lookup into a linker whose owner step has not started is
E13.

**6.6 `.bin` mode.** In normal play the compiler does not run. Some
loaders rebuild a linker from the loaded records. Confirmed for item
codes: the items loader registers `code` (offset 128) of the combined
records in order with the §6.1 add, so duplicates bump as in a compile.
The other runtime maps: `loading.md` §7.4; whether each rebuild treats
duplicates the same way: Open question 3.

### 7. String keys (`strkey`)

```
text = first min(L, 256) cell bytes
if text is empty:                         id = 0
elif text is a key in patchstring.tbl:    id = element + 10,000
elif LoD and text is a key in expansionstring.tbl: id = element + 20,000
elif text is a key in string.tbl:         id = element
else:                                     id = 0
store u16 (id if id ≠ 0 else 5,382)
```

- Key lookup is exact and case-sensitive (`tbl.md` §Key lookup). "LoD"
  means `expansionstring.tbl` is loaded (`loading.md` §10.1).
- 5,382 (0x1506) is the no-string ID: `string.tbl` element 5382 has the
  key `dummy`. Empty and unknown keys both give 5382.
- `string.tbl` element 0 (`WarrivAct1IntroGossip1`) cannot be produced: its
  0 also becomes 5382.
- A key present in several tables resolves to the first in the order
  patch → expansion → base.
- Missing column → u16 0, with no lookup.
- IDs are element numbers of the string tables used at compile time.
  Compile with the tables of the installed language. This install has only
  `ENG`.
- Live 1.14d `strkey` cells: 8,716 empty, 15 unknown (e.g. `misc` `namestr`
  `hpo`), 3,217 base, 927 expansion, 613 patch. All match the bins except
  one (§10).

### 8. Callback fields (`cb`)

1. Bound column: `text` = first min(L, 256) cell bytes. The original calls
   F(text, record, offset, length, record index, column index). `offset`
   and `length` are the field-list values, passed through; F may use them
   as something else (e.g. an input number).
2. Missing column: F(no text, record, offset, length, record index, k),
   after the record's bound columns, where k = header column count +
   position of the field among the missing fields.
3. No F in the list: nothing is written.
4. F chooses what to write and where, and may update other state (code
   buffers). The compiler treats IDs 23, 24 and 25 the same way.

**8.1 `calc(<buffer>)`** (formula fields).
- No text, or empty text → u32 0xFFFFFFFF at `offset`.
- Otherwise compile the expression (`calc-expressions.md`, with the table's
  keywords). If the bytecode is not empty, append it to the buffer and store
  the byte offset where it starts (u32). Empty bytecode → 0xFFFFFFFF.
- 1.14d buffers: `skillscode` (skills), `skilldesccode` (skilldesc),
  `itemscode` (weapons, armor and misc, one buffer, in that order),
  `misscode` (missiles). They ship as raw `<buffer>.bin` files
  (`loading.md` §4.3).
- 1.14d bins: skills has 11,999 empty calc cells → 0xFFFFFFFF and 853
  non-empty → offsets 0–5,885 (`skillscode.bin` is 5,891 bytes); missiles
  has 4,763 empty → 0xFFFFFFFF and 25 non-empty → offsets 0–193
  (`misscode.bin` is 196 bytes); skilldesc and misc behave the same.
  Missing calc columns (weapons and armor lack `calc1`–`calc3`, `len`,
  `spelldesccalc`) → 0xFFFFFFFF.
- Appends follow compile order (record, then column). Offsets equal the
  shipped ones only if the expression compiler emits the same bytes.

**8.2 `param`** (property parameter; `magicprefix`, `magicsuffix`,
`automagic`, `uniqueitems`, `sets`, `setitems`, `gems`, `runes`).
- No text, or empty text → u32 0.
- First byte `-` or `0`–`9` → C `atol(text)`: optional sign, decimal digits,
  stop at the first non-digit (`41` → 41, `-5` → −5, `12abc` → 12).
- Otherwise look up the name (§5.3, E11 included) in the skills linker (`skills.skill`),
  then `montype.type`, then `states.state`. The first hit gives the index.
  No hit → 0. A linker that does not exist yet is skipped.
- 1.14d: `runes` record 27 `t1param4` `Battle Command` → 155 (skills record
  155); `runes` record 6 `t1param4` `41` → 41.

**8.3 Table-specific callbacks in 1.14d** (defined in their table specs):
`monstats` `Sk1mode`–`Sk8mode`; `monstats2` `HDv`, `TRv`, `LGv`, `Rav`,
`Lav`, `RHv`, `LHv`, `SHv`, `S1v`–`S8v`; `monpreset` `Place`; `cubemain`
`input 1`–`input 7`, `output`, `output b`, `output c`. They write bytes
outside their own offset.

`strkey` also goes through a function pointer in the original, but its
behavior is fixed (§7), so the vocabulary does not treat it as a callback.

### 9. Records

- `.bin` container: u32 count, then `count × size` bytes; nothing else
  (`loading.md` §4, including size validation).
- Inside a record: fields at their list offsets; integers little-endian;
  codes as 4 raw bytes; strings NUL-terminated. The compiler inserts no
  padding and no alignment.
- Bytes no field writes are 0. Measured on the live bins of the 82 tables
  checked: every byte outside the field footprints is 0. There are no
  uninitialized or garbage bytes. Only table-specific callbacks (§8.3)
  write outside field footprints.
- `leveldefs` records come from `levels.txt`: same rows, its own field
  list.
- Compilation is a pure function of the text, the field list, the linkers,
  the string tables and the callbacks. It has no other input.

### 10. Comparing a compiled `.txt` with a shipped `.bin`

Acceptance test for the compiler (`#[ignore]`, reads `D2_GAME_DIR`):

1. Take the source `.txt` and live `.bin` per `loading.md` §11, including
   the cross-archive pairs (d2exp `automagic`, `rareprefix`, `raresuffix`
   `.txt` → patch_d2 `.bin`; patch_d2 `inventory`, `plrmode` `.txt` → d2exp
   `.bin`) and `levels.txt` → `leveldefs.bin`.
2. Compile with the table's field list, every linker available (§6.5) and
   the install's string tables.
3. The counts must be equal, and every record byte, except:

| Table | Bytes | Shipped | Compiled | Cause |
|---|---|---|---|---|
| monstats | record 707 (`uberandariel`), `NameStr` u16 at offset 6 | 5382 (`06 15`) | 11154 (`92 2B`) | the 1.14d `patchstring.tbl` has `Lilith` at element 1154; the bin holds the no-string ID (inferred: compiled against tables without that key) |

4. Calc fields and the code buffers: `calc-expressions.md` (its
   acceptance test reproduces them).
5. Bytes written by table-specific callbacks (§8.3): equal once their
   specs exist and are implemented.

Nothing else may differ. The shipped bins hold no garbage bytes (§9).

Result of the d2rs implementation (confirmed by bin cross-check,
2026-10-05): every runtime table matches under these rules, 69 of 73
byte-identical; in `monstats`, `monstats2`, `monpreset` and `cubemain`
the only other differences are bytes outside every field footprint
(step 5) and the row above. With the §8.3 callbacks writing nothing,
those bytes are 12,200 / 118,780 / 430 / 1,657. The callbacks are called
5,872 (`monstats` `Sk*mode`, 734 × 8), 9,744 (`monstats2`, 609 × 16), 229
(`monpreset` `Place`), 1,057 (`cubemain` inputs, 151 × 7) and 453
(`cubemain` outputs, 151 × 3) times.

## Constants & data dependencies

| Constant | Value |
|---|---|
| fields per list / header columns / columns + missing fields | ≤ 280 each |
| name key | first 31 bytes, `A`–`Z` → `a`–`z` |
| code | 4 bytes, space (0x20) padded |
| `strkey` and callback text | first 256 bytes |
| string ID bases | patch +10,000; expansion +20,000; base +0 |
| no-string ID | 5,382 (0x1506) |
| link miss, link missing column | −1 by width |
| calc empty or missing | 0xFFFFFFFF |
| `param` empty or missing | 0 |
| bit mask | `1 << (n & 7)` in byte `offset + (n >> 3)` |

## Randomness

None.

## Edge cases & original bugs

- Missing `code4` / `key(code4)` column → zeros; empty cell → spaces.
- Missing `strkey` column → 0; empty cell → 5382.
- Missing link column → −1, even when the target has an empty key; an
  empty link cell → that empty key's index.
- `string.tbl` element 0 cannot be referenced by a `strkey`.
- A `str(N)` terminator can spill one byte into the next field (§5.1).
- Duplicate codes are stored under bumped values (§6.1).
- `link8` keeps the low byte of indexes ≥ 256 (no check in pass 2).
- Name keys compare on 31 bytes; longer names can collide.
- Non-ASCII bytes in name keys read unrelated memory in the original;
  d2rs rejects them.

## Test vectors

**Integers** (cell → u32, then low bits)

| Cell | u32 | i32 | u16 | u8 |
|---|---|---|---|---|
| `""` | 0x00000000 | 0 | 0x0000 | 0x00 |
| `"5"` | 0x00000005 | 5 | 0x0005 | 0x05 |
| `"-1"` | 0xFFFFFFFF | −1 | 0xFFFF | 0xFF |
| `"300"` | 0x0000012C | 300 | 0x012C | 0x2C |
| `"256"` | 0x00000100 | 256 | 0x0100 | 0x00 |
| `"999"` | 0x000003E7 | 999 | 0x03E7 | 0xE7 |
| `" "` | 0xFFFFFFF0 | −16 | 0xFFF0 | 0xF0 |
| `" 5"` | 0xFFFFFF65 | −155 | 0xFF65 | 0x65 |
| `"5 "` | 0x00000022 | 34 | 0x0022 | 0x22 |
| `"+5"` | 0xFFFFFFD3 | −45 | 0xFFD3 | 0xD3 |
| `"12abc"` | 0x0000442B | 17451 | 0x442B | 0x2B |
| `"0x10"` | 0x00001C2A | 7210 | 0x1C2A | 0x2A |
| `"1.5"` | 0x00000055 | 85 | 0x0055 | 0x55 |
| `"1e3"` | 0x00000279 | 633 | 0x0279 | 0x79 |
| `"-"` | 0x00000000 | 0 | 0x0000 | 0x00 |
| `"-0"` | 0x00000000 | 0 | 0x0000 | 0x00 |
| `"--5"` | 0x00000019 | 25 | 0x0019 | 0x19 |
| `"5-"` | 0x0000002F | 47 | 0x002F | 0x2F |
| `"65535"` | 0x0000FFFF | 65535 | 0xFFFF | 0xFF |
| `"65536"` | 0x00010000 | 65536 | 0x0000 | 0x00 |
| `"-32768"` | 0xFFFF8000 | −32768 | 0x8000 | 0x00 |
| `"2147483648"` | 0x80000000 | −2147483648 | 0x0000 | 0x00 |
| `"4294967295"` | 0xFFFFFFFF | −1 | 0xFFFF | 0xFF |
| `"4294967296"` | 0x00000000 | 0 | 0x0000 | 0x00 |
| `"99999999999"` | 0x4876E7FF | 1215752191 | 0xE7FF | 0xFF |
| `"\xE9"` | 0xFFFFFFB9 | −71 | 0xFFB9 | 0xB9 |

`u8?`: `""` writes nothing; `"-"` writes 0x00.
`bit(10)` at offset 16 (byte 17, mask 0x04): `"1"`, `"2"`, `"-1"`, `" "`
set it; `"0"`, `""`, `"-"` clear it.

**Text and codes**

| Type | Cell | Bytes at `offset` |
|---|---|---|
| `str(4)` | `"abc"` | `61 62 63 00` |
| `str(4)` | `"abcdef"` | `61 62 63 64 00` |
| `str(4)` | `""` | `00` |
| `str(2)` | `"NU0"` | `4E 55 00` |
| `key(str(4))` | `"abcdef"` | `61 62 63 00`; key `abc` |
| `code4` | `""` | `20 20 20 20` |
| `code4` | `"DT"` | `44 54 20 20` |
| `code4` | `"hax"` | `68 61 78 20` |
| `code4` | `"staff"` | `73 74 61 66` |
| `code4` | `"ring "` | `72 69 6E 67` |
| `code4` | `" a"` | `20 61 20 20` |
| `code4`, missing column | — | `00 00 00 00` |
| `key(code1)` | `"abc"` (index ≤ 255) | `61` |

**Linkers**

| Steps | Result |
|---|---|
| code add `abc`, `xyz`, `abc` | indices 0, 1, 2; keys `abc `, `xyz `, `bbc ` (0x20636262) |
| then find `abc`, `xyz`, `bbc`, `abd`, `""` | 0, 1, 2, −1, −1 |
| code add `""`, `""` | 0, 1; keys `"    "`, `"!   "` (0x20202021); find `""` → 0 |
| name find-or-add `Fire Bolt`, `fire bolt`, `Ice`, `""` | stored 0, 0, 1, 2; `n` = 3 |
| then find `FIRE BOLT`, `""`, `Fire` | 0, 2, −1 |
| name add-always `a`, `A`, `b` | `a` → 0, `b` → 2; `n` = 3 |
| two 40-byte names equal in their first 31 bytes | same key, same index |
| `link8` miss / `link16` miss / `link32` miss | `FF` / `FF FF` / `FF FF FF FF` |

**String keys** (1.14d `ENG` tables, LoD)

| Text | Stored |
|---|---|
| `""` | 5382 |
| `cap` | 1930 (`string.tbl`) |
| `Cap`, `CAP` | 5382 (case-sensitive) |
| `x` | 10016 (`patchstring.tbl` element 16) |
| `Cutthroat1` | 10000 |
| `ModStr5d` (patch element 1, also base element 3578) | 10001 |
| `A4Q2ExpansionSuccessTyrael` | 20000 |
| `ob1` (expansion element 281, also base element 1544) | 20281 |
| `Lilith` | 11154 |
| `dummy` | 5382 |
| `WarrivAct1IntroGossip1` (base element 0) | 5382 |
| `nonexistent_key_zz` | 5382 |

**1.14d data** (live `.txt` → live `.bin`; record numbers 0-based)

| Table, record, column | Type, offset | Cell | `.bin` bytes |
|---|---|---|---|
| misc 45 (`KhalimEye`) `TMogMin` | `u8` (ID 6) @314 | `" "` | `F0` |
| misc 37 `rarity` | `u8` @252 | `999` | `E7` |
| skills 159 (`MaggotEgg`) `scroll` | `bit(36)` @4 | `" "` | byte 8 = `10` |
| objects 0 (`Dummy`) `Token` | `str(2)` @192 | `NU0` | `4E 55 00` |
| weapons 0 `code` | `key(code4)` @128 | `hax` | `68 61 78 20` |
| weapons 0 `normcode` | `code4` @132 | `hax` | `68 61 78 20` |
| magicprefix 444 `itype1` | `link16(itemtypes.code)` @106 | `staff` | `1A 00` (26, code `staf`) |
| magicsuffix 433 `itype2` | `link16(itemtypes.code)` @108 | `ring ` | `0A 00` |
| magicprefix 0 `itype1` | `link16(itemtypes.code)` @106 | `""` | `00 00` |
| armor 0 `namestr` | `strkey` @244 | `cap` | `8A 07` |
| misc 1 (`Healing Potion`) `namestr` | `strkey` @244 | `hpo` | `06 15` |
| monstats 723 `Id` | `key(name16)` @0 | `cr_lancer8` | `69 02` (617) |
| monseq 0 / 1–7 `sequence` | `key(name16)` @0 | `""` / `seq_pinheadsmite` | `00 00` / `01 00` |
| runes 0 `Rune1` | `link32(items.code)` @152 | `r08` | `69 02 00 00` (617) |
| monstats 0 `TreasureClass1` | `link16(@tc)` @134 | `Act 1 H2H A` (treasureclassex 269) | `AE 01` (430) |
| runes 27 `t1param4` | `param` | `Battle Command` | 155 |
| runes 6 `t1param4` | `param` | `41` | 41 |
| skills `pettype` (326 rows; was 324, corrected: confirmed by bin cross-check) | `link8(pettype.pet type)` @190 | `""` | `FF` |
| skills `pettype` | `link8(pettype.pet type)` @190 | `assassintrap` | `11` |
| armor 22 `mindam` (columns 63 and 161) | `u8` (ID 6) @254 | `1`, `0` | `01` (leftmost wins) |
| levels, all records, `mon11` (no column) | `link16(monstats.Id)` @74 | missing | `FF FF` |
| armor, all records, `hit class` (no column) | `link8(hitclass.code)` @316 | missing | `FF` |
| weapons, all records, `spelldescstr` (no column) | `strkey` @182 | missing | `00 00` |
| misc, all records, `setinvfile` (no column) | `str(31)` @96 | missing | `00` |
| weapons, all records, `BetterGem` (no column) | `code4` @188 | missing | `00 00 00 00` |
| weapons, all records, `calc1` (no column) | `calc(itemscode)` @164 | missing | `FF FF FF FF` |
| d2exp objgroup 97 (`EXPANSION`) | all fields | row kept | 52 × `00`; count 133 |

**Whole tables.** Compiling the live `.txt` of each of the 82 tables in
Provenance reproduces its live `.bin` byte for byte (calc fields masked;
`calc-expressions.md`); monstats differs as in §10.

## Provenance

**1.14d `Game.exe`** (Ghidra exports, instruction listings dumped by a
scratch script, and bytes read from the PE file; addresses are virtual):

| Address | Role |
|---|---|
| 0x6BD780 | compiler: pass 1 / pass 2, missing fields, record-end checks |
| 0x6BE0D8 (by ID − 1) → handlers 0x6BE0B8 | pass-2 class per type: text 1, 7; integer 2–6, 8, 26; code4 9; skip 10, 12, 14, 16–18; code link 11, 13, 15; name link 19–21; `strkey` 22; callback 23–25 |
| 0x6BE10C (by ID − 2) → 0x6BE0F4 | integer store width: 4 for 2, 8; 2 for 3; 1 for 4, 6; conditional 1 for 5; bit for 26 |
| 0x6BE128 (dword by ID − 1) | missing-column action (§3 column "Missing column") |
| 0x6BE090 / 0x6BE0AC | pass-1 detection and handlers (IDs 10–18) |
| 0x6BCE20 | column binding, 280-field limit, `u8?` grouping check |
| 0x6BD230 / 0x6BD130 | code linker add (sorted array, +1 on duplicate) / find |
| 0x6BD5A0 / 0x6BD500 / 0x6BD3C0 / 0x6BD490 | name linker find-or-add / add-always / find / tree insert |
| 0x4135D0, 0x4113C0, table 0x6CEB98 | 32-byte bounded copy (31 characters); lowercase map, `A`–`Z` only |
| 0x410B10 / 0x410B50, masks 0x6CE268 / 0x6CE2E8 | bit set / clear; masks `1 << k` / `~(1 << k)` |
| 0x6117B0 → 0x524D30 → 0x524C60 | `strkey`: empty or unresolved → 5382; table order patch (0x8829BC), expansion (0x8829C0, if loaded), base (0x8829B8) |
| 0x611BD0, 0x611C70, 0x631530, 0x6619A0 | calc callbacks (skills, skilldesc, items, missiles): append to buffer or 0xFFFFFFFF |
| 0x6336A0 | `param` callback (skills linker 0x96C7CC, montype 0x96C868, states 0x96BCF0) |
| 0x6122F0, 0x613E90 | load-or-compile routine; code buffer read/write |
| 0x6315D0 | items loader: with compile off, registers the combined records' `code` (offset 128) with 0x6BD230 (§6.6) |
| 0x65A390 → 0x6541C0, 0x6547D0, 0x653F90 | `@tc`: empty name first, automatic TCs, `treasureclassex` rows until an empty name; every entry added with add-always 0x6BD500 |
| 0x7063A8 | command-line option `txt` → configuration offset 0x215 → compile switch (0x44D9C0 → 0x6125A0) |

**Field lists.** At every call of 0x6122F0 the loader builds its field
list on the stack. A scratch script rebuilt all 92 lists (column name, ID,
length, offset, linker or callback) from the decompiled assignments; names
behind data addresses were read from `.rdata`. Only these facts were
taken. Every record size passed equals the live `.bin` size. 92 = 73
runtime tables (`loading.md` §6) + 18 compile-only lists (`loading.md`
§7.2) + 1 in the dead loader 0x653DB0; `txt-format.md` counts the 91 live
ones. They cover 86 table names, because monmode, plrmode, skills,
monstats and skilldesc have both a compile-only and a runtime list.

**Data check.** A scratch Python model of §2–§8 (with `txt-format.md` for
splitting and binding) compiled the live `.txt` of every table with a
recovered list, in the order of §6.5, and compared whole records with the
live `.bin` (count, then every byte):

- Byte-identical (82): armor, armtype, arena, automagic, automap, belts,
  bodylocs, books, charstats, chartemplate, colors, compcode, composit,
  difficultylevels, elemtypes, events, experience, gamble, gems, hiredesc,
  hireling, hitclass, inventory, itemratio, itemstatcost, itemtypes,
  leveldefs, levels, lowqualityitems, lvlmaze, lvlprest, lvlsub, lvltypes,
  lvlwarp, magicprefix, magicsuffix, misc, misscalc, missiles,
  monai, monequip, monitempercent, monlvl, monmode, monplace, monprop,
  monseq, monsounds, montype, monumod, npc, objects, objgroup, objmode,
  objtype, overlay, pettype, playerclass, plrmode, plrtype, properties,
  qualityitems, rareprefix, raresuffix, runes, setitems, sets, shrines,
  skillcalc, skilldesc, skills, sounds (2-byte link table), states,
  storepage, superuniques, treasureclassex, uniqueappellation,
  uniqueitems, uniqueprefix, uniquesuffix, uniquetitle, weapons.
- Masked: calc fields (skills, skilldesc, weapons, armor, misc, missiles;
  reproduced by `calc-expressions.md`). Treasure-class links, masked in
  the first run, were then checked with the §6.4 `@tc` model: all 8,808
  monstats and 198 superuniques cells match. The `param` callback was
  modeled and matches in all 8 tables that use it.
- Not identical: monstats, monstats2, monpreset, cubemain. Every differing
  byte lies outside all modeled field footprints (their table-specific
  callbacks), plus the monstats `NameStr` of record 707.
- Counts in the text above (empty-key hits, `strkey` outcomes, calc cells)
  come from the same run.

**String tables:** `ENG` `string.tbl` (d2data), `expansionstring.tbl`
(d2exp), `patchstring.tbl` (patch_d2), read with `tbl.md`.

**D2MOO (MIT, 1.10f):** type IDs and names (`D2C_TxtFieldTypes`,
`D2BinFieldStrc`), the CompileTxt flow and the roles of the item calc and
property callbacks. D2MOO does not contain the compiler itself
(`Fog/src/Excel/Excel.cpp` is empty); every rule above comes from 1.14d.
1.10f logs a missing string key; 1.14d does not.

**Cross-check:** `txt-format.md` (an independent analysis of the same
functions) agrees with every rule here.

## Open questions

1. The 92 recovered field lists exist only in a scratch file. Before §10
   can run per table they must be published as facts (column, type,
   length, offset, link, record size, loader), in per-table specs or one
   generated field-list spec.
2. The table-specific callbacks (§8.3) were not analyzed.
3. Runtime linker rebuilds other than the item codes (§6.6): which loaders
   rebuild which linker, and whether duplicates bump as in §6.1.
4. IDs 5, 7, 12, 14, 16 and 24 appear in no 1.14d list. Their rules come
   from the code only.
5. The duplicate-code bump (§6.1) comes from the code only. No 1.14d lookup
   uses a bumped key.
6. `atol` edge cases in `param` (overflow, MSVC CRT behavior) were not
   checked; all 1.14d `param` numbers are small.
7. Policy for monstats record 707 `NameStr`: keep the shipped 5382 or the
   compiled 11154 when d2rs compiles that table (§10).
8. Whether string-table element numbering is the same in every language.
   The shipped bins hold one set of IDs; this install has only `ENG`.
9. The client sound tables' field lists (`sounds.txt` 142-byte and
   `soundenviron.txt` 88-byte records) were not examined; they use the same
   compiler functions.
10. Bytes ≥ 0x80 in name keys: the original's mapping is not reproduced.
