# Spec: Data — Excel text tables (.txt) reader

- **Status:** verified by `data-tool tables` (2026-10-05). revised after review (2026-10-05). Every rule was
  checked against the 1.14d `Game.exe` code; record counts against all 128
  same-archive `.txt`/`.bin` pairs; conversions and linkers by compiling
  the live `.txt` files with the 91 field lists recovered from `Game.exe`
  and comparing with the live `.bin` files (Provenance). Implemented in
  `d2-data::txt` (reader, binding) and `d2-data::compile` (conversions,
  linkers). The d2rs compile of all 91 called lists reproduces the
  survey (193 files, `Aiparms.txt` E8 at line 13, 128 count pairs) and the
  §9 diagnostic counts exactly: IntSyntax 9, IntRange 9, TextCut 46,
  LinkMiss 14, DupColumn 11, DupCode 4, DupName 0 (confirmed by bin
  cross-check).
- **Target version:** 1.14d
- **Crate/module:** `d2-data::txt` (reader, column binding, cell
  conversions, linkers)
- **Related specs:** `specs/formats/mpq.md` (reading the bytes from the
  archives); `specs/data/loading.md` (which files 1.14d reads as text and
  when, compile mode, load order, which linker each table fills);
  `specs/data/field-types.md` (the type vocabulary per-table specs use,
  string keys, callbacks; it builds on §6–§8 here and must agree with
  them); per-table specs (field lists, record sizes).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 50–66 |
| Inputs | 67–75 |
| Outputs / state changes | 76–91 |
| Rules | 92–93 |
|   1. Where this reader is used | 94–108 |
|   2. Bytes | 109–119 |
|   3. Lines | 120–132 |
|   4. Header row | 133–145 |
|   5. Data rows and record numbering | 146–171 |
|   6. Column binding | 172–228 |
|   7. Cell conversions | 229–338 |
|   8. Linkers | 339–399 |
|   9. Strictness policy | 400–475 |
|   10. Original-only behaviors not reproduced | 476–484 |
| Constants & data dependencies | 485–497 |
| Randomness | 498–501 |
| Edge cases & original bugs | 502–525 |
| Survey (1.14d data) | 526–692 |
| Test vectors | 693–878 |
| Provenance | 879–937 |
| Open questions | 938–967 |
<!-- /index -->

## Summary

The game's data tables (`data\global\excel\*.txt`) are tab-separated text
exported from Excel. This spec covers what is the same for every table:
splitting a file into a header and records, numbering the records, binding
header columns to a table's fields, converting each cell to bytes by field
type, and the code/name *linkers* that turn cross-references into record
indices. A per-table spec supplies the field list and the record size.

The rules are those of 1.14d `Game.exe` when it compiles a `.txt`, plus a
strict error policy (§9). Each rule states the d2rs behavior; "(1.14d: …)"
gives the original's outcome where it differs. "Fatal N" means the
original calls its error handler with code N and exits.

Numbering: columns from 0 (header order), lines from 1 (the header is
line 1), records from 0. Error reports and diagnostics use the same bases.

## Inputs

| Name | Type | Source |
|---|---|---|
| file bytes | `.txt`, raw 8-bit | archives, `data\global\excel\<name>.txt` (`mpq.md`; which file: `loading.md` §3) |
| field list | ordered entries (name, type id 1–26, offset, len, linker or callback) | the table's spec |
| record_size | bytes per record | the table's spec (`loading.md` §6) |
| linkers | code and name key tables, kept across tables (§8) | data layer; created per `loading.md` §7 |

## Outputs / state changes

- **Reader:** the header (column names as byte strings) and the records:
  for record `i`, one byte string per column and its line number. Also the
  line numbers of removed `Expansion` lines.
- **Binding:** for every field, the column it reads or "missing".
- **Conversion:** `R × record_size` bytes, records in order; the linkers
  with this table's registrations added; the diagnostics (§9), a list of
  (kind, line, column, field name).
- **Failure:** one error code (§9) with line, column and field name where
  known.

Text stays bytes. The reader never decodes, trims, unquotes or normalizes a
cell. Decoding for display is a presentation step (English 1.14d text is
Windows-1252).

## Rules

### 1. Where this reader is used

- Which files 1.14d reads as text, and when: `loading.md` §3. In normal
  play only `sounds.txt` and `soundenviron.txt` (`loading.md` §3.4).
- Compile mode (`loading.md` §3.2) is on exactly when configuration byte
  +0x215 is non-zero; the `txt` command option sets that byte. Then the
  table loader first compiles `<name>.txt` (`levels.txt` for `leveldefs`)
  with these rules and writes `DATA\GLOBAL\EXCEL\<name>.bin` relative to
  the current directory: u32 record count, then the records. If that file
  cannot be created, the write is skipped without an error. In both modes
  the loader then reads `<name>.bin` through the archive set.
- d2rs uses this reader for the two runtime sound tables and for the
  mod-authoring compiler. The compiler's output must equal the `.bin` 1.14d
  writes in compile mode: same records, same order (§5), same bytes (§7).

### 2. Bytes

- Raw 8-bit bytes; no encoding is assumed. Only 0x09 (tab), 0x0D (CR) and
  0x0A (LF, right after CR) are structural. Every other byte, including
  `"`, spaces and bytes ≥ 0x80, is cell content.
- A BOM is an error (E10). (1.14d: it becomes part of the first column
  name.)
- A NUL byte is an error (E9). (1.14d: the splitter turns every tab and CR
  into a NUL and the compiler walks cells by their NULs, so an extra NUL
  ends the cell early and shifts every later cell of the file.)

### 3. Lines

- A line ends with CR LF (0x0D 0x0A). That pair is the only line end.
- A LF not preceded by CR is an error (E5). (1.14d: ordinary cell
  content.)
- A CR not followed by LF, including a CR as the last byte, is an error
  (E4). (1.14d: fatal 92 in the header, 117 in a data line; a CR as the
  last byte is compared with the byte past the buffer.)
- Bytes after the last CR LF are an error (E6). (1.14d: that final line is
  not a record and is dropped without a message; see also §10.)
- No limit on line length, file size or cell length. Field types cut what
  they read (§7).

### 4. Header row

- The first line is the header. Its cells, split on tab, are the column
  names. Column count `C` = number of tabs + 1.
- `C` > 280 is an error (E7). (1.14d: fatal 103.)
- Names are kept byte-exact (no trimming). Empty and duplicate names are
  allowed; §6 decides what they bind.
- A name has no special syntax. Columns no field uses carry names like
  `*desc` or `description - not loaded` by convention; they are unbound
  because no field has that name, not because of the `*`.
- The header is never checked for `Expansion`: `Expansion\r\n1\r\n` has the
  header `[Expansion]` and one record `[1]`.

### 5. Data rows and record numbering

1. The lines after the header are data lines.
2. A data line whose first cell is exactly `Expansion` is removed: the 9
   bytes `45 78 70 61 6E 73 69 6F 6E` followed by a tab or the line end.
   The comparison is case-sensitive and exact: `EXPANSION`, `expansion`,
   `Expansion ` and `Expansion2` start ordinary records. The other cells of
   a removed line are ignored, whatever they hold, and its cell count is
   not checked.
3. The remaining lines are the records, numbered 0, 1, 2, … in file order.
   Removed lines take no number.
4. Every record has exactly `C` cells (`C − 1` tabs); otherwise E8.
   (1.14d: not checked when splitting. The compiler walks `C` cells per
   record and then requires the next byte to be the line's LF: fatal 389
   in pass 1, 646 in pass 2. A short record continues into the next line:
   its next cell is that LF plus the next line's first cell. A long record
   stops the walk inside its own line. Both normally fail the LF check; a
   short record goes unnoticed only when later lines' cell counts bring the
   walk back to a line end.)
5. Records with an empty first cell, and records whose cells are all
   empty, are normal records. With `C = 1` an empty line is a record with
   one empty cell.
6. A header with no line after it is E2; zero records after removal is
   E3. (1.14d: for E2 the splitter returns nothing and the loader reads
   the count through a null pointer; E3 is fatal 274.)

### 6. Column binding

Two names are equal when they have the same length and the same bytes
after mapping `A`–`Z` to `a`–`z` (ASCII only; other bytes compare as they
are). No trimming.

1. Columns are processed left to right.
2. A column binds the field with an equal name if that field is not yet
   bound. Otherwise the column is unbound and its cells are skipped.
3. With duplicate column names the leftmost binds; the later ones are
   unbound (diagnostic DupColumn). A column with an empty name binds
   nothing, because field names are not empty (§6.1).
4. A field that no column binds is *missing*. It gets its missing-column
   value (§7).
5. The slot count is `C` + the number of missing fields. More than 280 is
   an error (E14).

(1.14d: a column binds the not-yet-bound field with an equal name that
comes **last** in the field list. With unique names, which every 1.14d
list has, that is rule 2. An empty column name would bind a field with an
empty name; no 1.14d list has one.)

1.14d data: patch_d2 `armor.txt` has `mindam`, `maxdam` at columns 63/64
and again at 161/162. The copies differ in 54 records; `armor.bin` holds
the values of columns 63/64 in all of them.

#### 6.1 Field-list checks

A field list is checked before the file is read. Any failure is E13.

| Check | 1.14d without the check |
|---|---|
| at most 280 entries | fatal 204 at the 281st (the type-0 terminator of the original's lists does not count) |
| every type id is 1–26 | ids above 26 index past the dispatch tables |
| names are non-empty and unique (equality as in §6) | not checked; see the binding note above |
| `record_size` > 0 | fatal 275 |
| type-5 runs (below) | fatal 196 |
| types 10–15 name a code linker, types 16–21 a name linker, and the linker exists | registering into the other kind: fatal 984 (code), 1206 / 1232 (name); a name lookup in a code linker: fatal 1141; a code lookup in a name linker: crash; registering into a missing linker: fatal 977 / 1202 / 1228; a lookup in one: crash |
| types 22–25 name a callback | a bound cell writes nothing; a missing field writes u16 0 (type 22) or nothing (23–25) |
| every field fits in the record (below) | not checked |

*Type-5 runs.* Scan the list from the start. At an entry of type 5 with
`len` > 1, set `i = 1` and repeat: step to the next entry, which must be of
type 5; `i += 1`; stop when `i` ≥ that entry's own `len`. Continue the scan
after the last entry checked. With equal lens `L` this requires `L` type-5
entries in a row.

*Field fits.* `offset + width` ≤ `record_size`, with width `len + 1` for
types 1 and 7; `max(len, 1)` for 16; 4 for 2, 8, 9, 10, 11, 18, 19; 2 for
3, 14, 15, 17, 20, 22; 1 for 4, 5, 6, 12, 13, 21. Type 26 needs
`offset + (len >> 3)` < `record_size`. Types 23–25 are not checked; the
callback's spec bounds what it writes.

All 91 field lists of 1.14d pass: at most 253 entries, unique non-empty
names, no type 5, one kind per linker, a callback on every type 22–25
field, every field inside its record.

### 7. Cell conversions

Every record starts as `record_size` zero bytes. A field writes at
`offset` inside its record. `len` is the field's length value; its meaning
depends on the type. `L` is the cell's length in bytes.

**Order of evaluation** (it matters for linkers and overlapping fields):

1. **Pass 1:** for each record in order, for each bound column left to
   right, convert the cells of types 10, 12, 14, 16, 17 and 18 (they
   register keys, §8). Skip other types.
2. **Pass 2:** for each record in order: each bound column left to right,
   all other types; then each missing field, in field-list order, writes
   its missing-column value.

- Every registration of a table happens before any lookup in it, so a
  table can reference its own later rows.
- A missing field only writes its missing-column value. It never
  registers a key and never advances a linker's count.
- A missing-column value is written after the record's bound columns, so
  it wins over a bound field it overlaps. Among bound fields the later
  write wins.

**Field types.** Ids and names as in D2MOO's `TXTFIELD_*`. "Checked" =
1.14d values the compile reproduces, as bound cells / missing-field values
(Provenance); "unused" = in no 1.14d list; "cb" = defined by the callback.

| Id | Name | Pass | Reads from the cell | Stores at `offset` | Missing column | Checked |
|---|---|---|---|---|---|---|
| 1 | ASCII | 2 | first min(L, len) bytes | those bytes, then 0x00 at `offset + min(L, len)` | u8 0 | 23,512 / 151 |
| 2 | DWORD | 2 | integer (below) | u32 | u32 0 | 179,213 / 1,624 |
| 3 | WORD | 2 | integer | low 16 bits | u16 0 | 53,257 / 957 |
| 4 | BYTE | 2 | integer | low 8 bits | u8 0 | 255,177 / 3,293 |
| 5 | UNKNOWN1 | 2 | integer | low 8 bits; an empty cell writes nothing | u8 0 | unused |
| 6 | UNKNOWN2 | 2 | integer | low 8 bits | u8 0 | 7,553 / 9,178 |
| 7 | BYTE2 | 2 | as ASCII | as ASCII | u8 0 | unused |
| 8 | DWORD2 | 2 | integer | u32 | u32 0 | 1,398 / — |
| 9 | RAW | 2 | code (below) | the 4 code bytes | u32 0 (not spaces) | 8,207 / 2,175 |
| 10 | ASCIITOCODE | 1 | code; registered | the cell's 4 code bytes (never a bumped key) | u32 0 | 1,072 / — |
| 11 | UNKNOWN3 | 2 | code; looked up | u32 index, miss 0xFFFFFFFF | u32 0xFFFFFFFF | 1,082 / — |
| 12 | UNKNOWN4 | 1 | code; registered; index > 0xFF is E12 | first code byte | u8 0 | unused |
| 13 | CODETOBYTE | 2 | code; looked up | low 8 bits of the index, miss 0xFF | u8 0xFF | 12,814 / 389 |
| 14 | UNKNOWN5 | 1 | code; registered; index > 0xFFFF is E12 | first 2 code bytes | u16 0 | unused |
| 15 | CODETOWORD | 2 | code; looked up | low 16 bits of the index, miss 0xFFFF | u16 0xFFFF | 25,432 / 1,566 |
| 16 | UNKNOWN6 | 1 | first min(L, max(len, 1) − 1) bytes; then registered as a name (register-always) | those bytes + 0x00 | u8 0 | unused |
| 17 | NAMETOINDEX | 1 | name key; find-or-register | u16 index | u16 0 | 9,224 / — |
| 18 | NAMETOINDEX2 | 1 | name key; find-or-register | u32 index | u32 0 | 33 / — |
| 19 | NAMETODWORD | 2 | name key; looked up | u32 index, miss 0xFFFFFFFF | u32 0xFFFFFFFF | 19,969 / — |
| 20 | NAMETOWORD | 2 | name key; looked up | low 16 bits, miss 0xFFFF | u16 0xFFFF | 50,547 / 9,213 |
| 21 | NAMETOWORD2 | 2 | name key; looked up | low **8** bits, miss 0xFF | u8 0xFF | 357 / — |
| 22 | KEYTOWORD | 2 | first min(L, 256) bytes → key callback | u16 result | u16 0, no call | 12,020 / 508 |
| 23 | CUSTOMLINK | 2 | first min(L, 256) bytes → field callback | cb | field callback, no text | cb |
| 24 | UNKNOWN7 | 2 | as 23 | as 23 | as 23 | unused |
| 25 | CALCTODWORD | 2 | as 23 | as 23 | as 23 | cb |
| 26 | BIT | 2 | integer; bit `k = len` | byte `offset + (k >> 3)`, mask `1 << (k & 7)`: set if the value ≠ 0, cleared if 0 | nothing | 74,125 / — |

"Nothing" leaves what the record holds (0 unless another field wrote
there).

**Integer rule** (types 2–6, 8, 26). No validation and no early stop:

```
neg = cell starts with '-' (0x2D);  if neg: skip that byte
v = 0                                   # u32, wrapping
for each remaining byte b:
    s = b if b < 0x80 else b − 256      # signed byte
    v = v·10 + s − 48
if neg: v = −v
```

The empty cell gives 0. `+`, spaces, `.`, letters and bytes ≥ 0x80 all feed
the formula (vectors below). 1.14d data has such cells: `misc` `TMogMin` /
`TMogMax` `" "` (6 cells) → 0xF0; `missiles` record 568 `pCltHitFunc`
`*16` → 0xFDB8 and `pSrvHitFunc` `*12` → 0xFDB4; `skills` record 159
`scroll` (BIT) `" "` → set; `missiles` record 82 `Explosion` and
`NoMultiShot` (BIT) `2` → set.

**Code** (types 9–15): the first min(L, 4) bytes, padded on the right with
spaces (0x20) to 4 bytes. Case is kept. The empty cell gives `"    "`. As a
number: u32 little-endian (first byte = low byte).

**Name key** (types 16–21): the first min(K, 31) bytes, then each byte
0x41–0x5A replaced by that byte + 0x20. K = L for types 17–21; for type 16,
K = the length of the text it stored. A byte ≥ 0x80 among those bytes is
E11.

**Callbacks** (types 22–25). Each callback is defined by the spec of the
table that uses it (string keys and formulas: `field-types.md` §7–§8). The
compiler calls them with this contract:

- *Key callback* (type 22): `key(text: &[u8]) -> u16`. Called for a bound
  cell with its first min(L, 256) bytes; the result is stored as u16
  little-endian at `offset`. A missing type-22 field stores u16 0 and makes
  no call (1.14d data: `weapons` and `armor` lack `spelldescstr`; all
  1,016 bytes are 0).
- *Field callback* (types 23, 24, 25; the compiler treats them alike):
  `field(text: Option<&[u8]>, record: &mut [u8], offset: u32, len: u32,
  record_index: u32, slot: u32)`. `record` is the whole record
  (`record_size` bytes); `offset` and `len` are the field-list values;
  `record_index` is the record number.
  - Bound cell: called in pass 2 at the cell's column, with
    `text = Some(first min(L, 256) bytes)` and `slot` = the column number.
  - Missing field: called after the record's bound columns, in field-list
    order among the missing fields, with `text = None` and `slot` = `C` +
    the field's position among the table's missing fields (from 0).
  - The compiler writes nothing for these types. The callback writes only
    inside `record` and its own state (e.g. a code buffer).
- (1.14d: the text is copied into a 256-byte buffer; longer cells are cut
  at 256 bytes.)

### 8. Linkers

A linker turns a key into a record index. It has a kind, code or name,
and a count `n` that starts at 0.

- A linker lives across tables. `n` is never reset; a registration's index
  is `n` at that moment. Several tables can register into one linker.
  1.14d: `weapons`, `armor` and `misc` register their `code` column into
  one item-code linker, in that order, so an item's index is its combined
  index: weapons 0–305, armor 306–507, misc 508–658. `runes` record 0
  `Rune1` = `r08` (misc record 109) stores 617.
- Which table fills which linker, and when each linker is created:
  `loading.md` §6–§7, `field-types.md` §6.4.
- A lookup sees the keys registered so far: those of earlier tables and
  pass 1 of the current table.

**Code linker** — a set of (u32 key, index) pairs:

- *Register(code):* `k = code`; while `k` is already a key: `k = k + 1`
  (u32, wrapping). Add `(k, n)`; the result is `n`; `n += 1`. Every
  registered cell takes an index, empty cells included (code `"    "`).
- *Lookup(code):* the index of key `code` (all 4 bytes, case-sensitive),
  or −1. A duplicate is found only under its bumped key: after registering
  `abc`, `abc`, the key `bbc ` (0x20636262) belongs to the second, and a
  later real `bbc` is bumped to `cbc `.

**Name linker** — a map from name key to index:

- *Register-always (type 16):* if the key is new, map it to `n`. `n += 1`
  either way, so a duplicate takes an index nothing maps to.
- *Find-or-register (types 17, 18):* if the key exists, the result is its
  index and `n` is unchanged. Otherwise map the key to `n`; the result is
  `n`; `n += 1`. A duplicate gets the first occurrence's index and takes
  none, so after a duplicate the stored index is below the record number.
- *Lookup (types 19–21):* the key's index, or −1. Keys are normalized
  (§7), so lookups are case-insensitive and ignore bytes past the 31st.

1.14d data for find-or-register:

- `monstats` `Id` (type 17): records 617 and 723 are both `cr_lancer8`.
  Record 723 stores 617; records 724–733 store 723–732. The compile-only
  `monstats` link built while `skills` compiles (`loading.md` §10.3) also
  uses type 17 on this column and gets the same indices.
- `monseq` `sequence` (type 17): 1,010 records, 60 distinct keys stored as
  0–59; record 1009 stores 59.

**Empty keys.** The empty cell is a key like any other (code `"    "`,
name `""`). A lookup of an empty cell hits when the linker holds the empty
key and is −1 otherwise. A missing lookup field is −1 in either case (§7).
In 1.14d every empty-key registration is at index 0, the first record of
its table:

| Linker (table `column`) | Kind | Empty key registered by |
|---|---|---|
| `bodylocs` `code`, `elemtypes` `code`, `hitclass` `code`, `hiredesc` `code` | code | record 0 |
| `itemtypes` `code` | code | records 0, 1, 14, 17, 23, as keys 0x20202020, 0x20202021, 0x20202022, 0x20202023, 0x20202024 (`"    "`, `"!   "`, `"\"   "`, `"#   "`, `"$   "`) with indices 0, 1, 14, 17, 23 |
| `sounds` `Sound`, `montype` `type`, `monsounds` `Id`, `monseq` `sequence` | name | record 0 |

In the 1.14d compile (Provenance), 24,442 empty code cells and 7,638
empty name cells hit an empty key; all match the `.bin`.

### 9. Strictness policy

The reader is strict. A condition is an error unless 1.14d data needs it;
then it is a quirk the reader accepts and reproduces. Some accepted
conditions are also reported as diagnostics.

**Check order.** Checks run in this order; the first failure is reported.

0. E13 (the field list, before the file is read).
1. E10 (bytes 0–2).
2. E1.
3. One scan in byte order; the first offending byte decides: NUL (E9), CR
   not followed by LF (E4), LF not preceded by CR (E5).
4. E6.
5. E7.
6. E2.
7. E8, at the first bad line.
8. E3.
9. E14 (after binding).
10. E11 and E12 in conversion order: pass 1 by record then column, then
    pass 2 by record then column.

**Errors** (reject the file):

| Code | Condition | Reported with | Original 1.14d outcome |
|---|---|---|---|
| E1 | no CR LF pair in the file (empty, LF-only, CR-only) | — | splitter returns nothing; the loader reads through a null pointer |
| E2 | nothing after the header's CR LF | — | as E1 |
| E3 | 0 records after removing `Expansion` lines | — | fatal 274 |
| E4 | CR not followed by LF, including a CR as the last byte | line | fatal 92 (header), 117 (data) |
| E5 | LF not preceded by CR | line | accepted as cell content |
| E6 | bytes after the last CR LF | line | line dropped (§10) |
| E7 | more than 280 header columns | line 1 | fatal 103 |
| E8 | record with a cell count ≠ `C` (removed `Expansion` lines excepted) | line | misread or fatal 389 / 646 (§5) |
| E9 | NUL byte | line | cells shift (§2) |
| E10 | file starts with `EF BB BF`, `FF FE` or `FE FF` | line 1 | BOM becomes part of the first column name |
| E11 | byte ≥ 0x80 among the bytes that form a name key (§7) | line, column, field | the lowercase step reads outside its table; the key is mangled and can end early |
| E12 | type 12 registration index > 0xFF, or type 14 index > 0xFFFF | line, column, field | fatal 342 / 346 |
| E13 | field list fails §6.1 | field | see §6.1 |
| E14 | `C` + missing fields > 280 | — | column map overrun (Edge cases) |

**Quirks** (accept and reproduce; all occur in 1.14d data):

| Quirk | 1.14d example |
|---|---|
| duplicate column names, leftmost binds | patch_d2 `armor.txt` `mindam`/`maxdam` |
| empty column name, never binds | patch_d2 `weapons.txt` column 18 |
| `Expansion` line removed whatever its other cells hold | 13 such lines (e.g. `misc.txt` line 96) |
| other spellings of Expansion are records | d2exp `objgroup.txt` `EXPANSION` (line 99 = record 97 of 133) |
| empty first cell; all-empty record | `magicprefix.txt`: 32 records with an empty first cell; `uniqueitems.txt`: last record all empty |
| leading/trailing spaces kept | `armor.txt` name `Studded Leather ` |
| quotes are literal bytes | `cubemain.txt` `"hpot,qty=3"`, `skills.txt` `"min(24,ln12)"` |
| bytes ≥ 0x80 kept raw outside name keys | `uniqueitems.txt` `*type` `Hunter` 0x92 `s Bow` |
| integer cell with non-digits uses the integer rule | `misc.txt` `" "`, `missiles.txt` `*16`, `*12` |
| integer wider than the field keeps its low bits | `misc.txt` `rarity` `999` → byte 0xE7 |
| text longer than an ASCII field is cut | `objects.txt` `Token` `NU0`, len 2 → `NU` |
| empty lookup cell hits an empty key | `missiles.txt` `TravelSound` empty → 0 (§8) |
| duplicate names compact find-or-register indices | `monseq.txt`, `monstats.txt` (§8) |

**Diagnostics** (accept, reproduce, report). Output: a list of (kind,
line, column, field name), in the order the checks run. The counts are
from the 1.14d compile (Provenance).

| Kind | Condition | Line, column, field | 1.14d count |
|---|---|---|---|
| IntSyntax | integer type (2–6, 8, 26), cell non-empty and not `-?[0-9]+` | the cell's | 9 |
| IntRange | cell matches `-?[0-9]+` and its exact value is outside the type's range: BYTE, UNKNOWN1, UNKNOWN2 −128..=255; WORD −32,768..=65,535; DWORD, DWORD2 −2³¹..=2³²−1; BIT 0..=1 | the cell's | 9 |
| TextCut | cell longer than its type reads: ASCII, BYTE2 `len`; UNKNOWN6 min(max(len, 1) − 1, 31); code types 9–15: 4; name types 17–21: 31; types 22–25: 256 | the cell's | 46 |
| LinkMiss | non-empty lookup cell (types 11, 13, 15, 19–21) gives −1 | the cell's | 14 |
| DupColumn | column name equal (§6) to an earlier column's | line 1, the later column, no field | 11 |
| DupCode | code registration whose code is already a key (stored under a bumped key, §8) | the cell's | 4 |
| DupName | type-16 registration of a key already present | the cell's | 0 |

Find-or-register duplicates (types 17, 18) are not reported: 1.14d data
relies on them (`monseq`). Empty lookup cells that miss are not reported.

### 10. Original-only behaviors not reproduced

- 1.14d also runs the `Expansion` test on a final unterminated line. If its
  first cell is exactly `Expansion` (followed by a tab, or by a 0 byte past
  the buffer), the record count drops by one and the last real record is
  lost. Unreachable here because of E6.
- A CR as the last byte makes 1.14d read one byte past the buffer.
  Unreachable because of E4.

## Constants & data dependencies

| Constant | Value |
|---|---|
| column separator / line end | 0x09 / 0x0D 0x0A |
| removed-row marker | first cell == `Expansion` (exact, case-sensitive) |
| max header columns / field-list entries / columns + missing fields | 280 each |
| code width | 4 bytes, space (0x20) padded |
| name key | first 31 bytes, `A`–`Z` → `a`–`z` |
| callback text (types 22–25) | first 256 bytes |
| lookup miss; missing lookup field | −1, stored as 0xFF / 0xFFFF / 0xFFFFFFFF by width |
| 1.14d fatal codes | 92, 103, 117 (splitter); 196, 204 (field list); 274, 275, 342, 346, 389, 646 (compile); 977, 984, 1141, 1202, 1206, 1228, 1232 (linkers) |

## Randomness

None.

## Edge cases & original bugs

- Missing RAW (9) or ASCIITOCODE (10) column: u32 0; an empty bound cell
  gives `"    "`. Both occur in 1.14d (`weapons.txt` lacks `BetterGem`: 0
  in `weapons.bin`).
- ASCII writes its NUL at `offset + min(L, len)`, so the field uses
  `len + 1` bytes.
- NAMETOWORD2 (21) stores one byte despite its name (`skills` `pettype` at
  offset 190; the next field starts at 191).
- Types 12 and 14 store code bytes, not the index.
- Type 10 stores the cell's code; a bumped key exists only in the linker.
  1.14d `itemtypes.bin` records 1, 14, 17 and 23 hold `20 20 20 20` while
  their keys are 0x20202021–0x20202024.
- A bumped duplicate can take the key of a later real code, which is then
  bumped itself (vectors).
- An empty lookup cell hits an empty key; a missing lookup column is −1
  even then.
- Column map (1.14d): 280 u16 slots, columns first, then missing fields.
  Slots 280–407 fall into the 256-byte callback text buffer, which a bound
  type 22–25 cell then overwrites before the record's missing fields are
  processed; later slots overwrite other locals. d2rs: E14. 1.14d lists
  reach at most 256 slots (`skills`: 256 columns, 0 missing; `monstats`
  255; `armor` 164 + 39 = 203; `levels` 140 + 45 = 185).

## Survey (1.14d data)

Strict reader (§2–§5) over every extracted `.txt`: 193 files, 7,080,588
bytes; patch_d2 62, d2exp 75, d2data 56.

- No BOM, no NUL, no lone CR, no lone LF. Every line ends with CR LF.
- 191 files parse (48,291 records). `Aiparms.txt` (d2exp and d2data) fails
  with E8 at line 13: 234 lines (1.14d would count 234 records) against a
  1-column header; 142 lines have 2–3 cells, 40 are empty. `Game.exe`
  contains no string `aiparms`; it is a documentation leftover. Also absent
  from `Game.exe`, but valid: `MonName.txt`, `WeaponClass.txt`,
  `cubemod.txt`, `cubetype.txt`.
- Max header columns 256 (`skills.txt`), max line 2,675 bytes (`skills.txt`
  header), max cell 199 bytes (`skills.txt` line 253 column 246). Min
  columns 1.
- Header names: all ASCII, none with leading or trailing spaces.
  Duplicates: `armor.txt` (`mindam`, `maxdam`; all three archives), d2exp
  and d2data `AutoMap.txt` (`Type2`), `CharTemplate.txt` (`SkillName`, 9
  copies), `monstats.txt` (`Comment`). Empty name: patch_d2 `weapons.txt`
  column 18 (between `maxmisdam` and `rangeadder`).
- Non-ASCII bytes: 9 cells, all in comment columns no 1.14d list binds.
  0x85 in patch_d2/d2data `objects.txt` (`description - not loaded`) and
  d2exp/d2data `skills.txt` (`effect`); 0x92 in patch_d2 `uniqueitems.txt`
  (`*type`), d2exp/d2data `UniqueItems.txt` (`type`) and d2exp
  `skills.txt` (`effect`); 0xA1 0xAD in d2exp `objects.txt`.
- `Expansion` lines: 23 files in patch_d2, 32 in d2exp, 0 in d2data; one
  line each, all with `C` cells; 13 (5 in patch_d2) have non-empty other
  cells.
- Record count = `.bin` count for all 128 same-archive pairs (patch_d2 59,
  d2exp 69). `leveldefs.bin` (137) = `levels.txt` (137).

**patch_d2** (`C` = columns, `R` = records, `X` = `Expansion` lines,
`.bin` = record count of the same-archive `.bin`; ✓ = equal to `R`):

| File | C | R | X | .bin | Notes |
|---|---|---|---|---|---|
| armor | 164 | 202 | 1 | 202 ✓ | dup `mindam`, `maxdam` |
| books | 11 | 3 | 0 | 3 ✓ | |
| charstats | 79 | 7 | 1 | 7 ✓ | |
| compcode | 2 | 115 | 0 | 115 ✓ | |
| cubemain | 105 | 151 | 0 | 151 ✓ | quotes |
| difficultylevels | 23 | 3 | 0 | 3 ✓ | |
| elemtypes | 2 | 13 | 0 | 13 ✓ | |
| events | 2 | 13 | 0 | 13 ✓ | |
| experience | 9 | 101 | 0 | 101 ✓ | |
| gems | 41 | 68 | 1 | 68 ✓ | |
| hireling | 73 | 120 | 0 | 120 ✓ | |
| inventory | 73 | 32 | 1 | — | no `.bin` in patch_d2 |
| itemratio | 20 | 6 | 0 | 6 ✓ | |
| itemstatcost | 53 | 359 | 0 | 359 ✓ | |
| itemtypes | 37 | 103 | 1 | 103 ✓ | 5 empty codes |
| levels | 140 | 137 | 1 | 137 ✓ | |
| lvlmaze | 9 | 81 | 1 | 81 ✓ | |
| lvlprest | 25 | 1091 | 1 | 1091 ✓ | |
| lvlsub | 24 | 34 | 1 | 34 ✓ | |
| lvltypes | 37 | 36 | 1 | 36 ✓ | |
| magicprefix | 41 | 669 | 1 | 669 ✓ | 32 empty first cells |
| magicsuffix | 39 | 747 | 1 | 747 ✓ | 6 empty first cells |
| misc | 168 | 151 | 1 | 151 ✓ | |
| misscalc | 2 | 43 | 0 | 43 ✓ | quotes |
| missiles | 171 | 684 | 0 | 684 ✓ | quotes |
| monai | 10 | 148 | 0 | 148 ✓ | |
| monequip | 13 | 45 | 0 | 45 ✓ | |
| monlvl | 31 | 111 | 0 | 111 ✓ | |
| monmode | 3 | 16 | 0 | 16 ✓ | |
| monplace | 1 | 37 | 0 | 37 ✓ | |
| monpreset | 2 | 229 | 0 | 229 ✓ | |
| monprop | 92 | 13 | 0 | 13 ✓ | |
| monseq | 6 | 1010 | 0 | 1010 ✓ | 1 empty first cell |
| monsounds | 41 | 141 | 0 | 141 ✓ | 1 empty first cell |
| monstats | 255 | 734 | 1 | 734 ✓ | |
| monstats2 | 126 | 609 | 1 | 609 ✓ | quotes |
| montype | 7 | 59 | 0 | 59 ✓ | 1 empty first cell |
| monumod | 19 | 43 | 0 | 43 ✓ | |
| npc | 19 | 17 | 0 | 17 ✓ | |
| objects | 160 | 573 | 1 | 573 ✓ | quotes, 0x85 |
| overlay | 26 | 293 | 1 | 293 ✓ | |
| pettype | 22 | 20 | 0 | 20 ✓ | |
| plrmode | 3 | 20 | 0 | — | no `.bin` in patch_d2 |
| properties | 36 | 268 | 1 | 268 ✓ | quotes |
| runes | 49 | 169 | 0 | 169 ✓ | |
| setitems | 94 | 127 | 1 | 127 ✓ | |
| sets | 69 | 32 | 1 | 32 ✓ | |
| shrines | 13 | 23 | 0 | 23 ✓ | quotes |
| skillcalc | 2 | 73 | 0 | 73 ✓ | quotes |
| skilldesc | 114 | 221 | 0 | 221 ✓ | quotes |
| skills | 256 | 357 | 0 | 357 ✓ | quotes |
| soundenviron | 24 | 50 | 0 | — | read as text at runtime |
| sounds | 25 | 4699 | 0 | 4699 ✓ | read as text at runtime; 1 empty first cell; 33 names > 31 bytes |
| states | 72 | 185 | 0 | 185 ✓ | |
| superuniques | 21 | 66 | 1 | 66 ✓ | |
| treasureclassex | 33 | 853 | 0 | 853 ✓ | quotes; 1 empty first cell |
| uniqueappellation | 1 | 25 | 0 | 25 ✓ | |
| uniqueitems | 70 | 402 | 1 | 402 ✓ | last record all empty; 0x92 |
| uniqueprefix | 1 | 53 | 0 | 53 ✓ | |
| uniquesuffix | 1 | 69 | 0 | 69 ✓ | |
| uniquetitle | 2 | 16 | 0 | 16 ✓ | |
| weapons | 166 | 306 | 1 | 306 ✓ | empty column name (column 18) |

**d2exp files with no `.txt` in patch_d2** (the highest-priority copy of
these names; patch_d2 was probed for all 30 and has only `automagic.bin`,
`rareprefix.bin`, `raresuffix.bin`):

| File | C | R | X | .bin | Notes |
|---|---|---|---|---|---|
| Aiparms | 1 | 234 | 0 | — | E8 (ragged); `R` is the 1.14d count; unused |
| Arena | 8 | 1 | 0 | 1 ✓ | |
| ArmType | 2 | 3 | 0 | 3 ✓ | |
| automagic | 38 | 36 | 0 | 36 ✓ | patch_d2 `.bin` also 36 |
| AutoMap | 13 | 3286 | 1 | 3286 ✓ | dup `Type2`; quotes |
| belts | 68 | 14 | 1 | 14 ✓ | |
| bodylocs | 2 | 11 | 0 | 11 ✓ | |
| CharTemplate | 95 | 30 | 0 | 30 ✓ | dup `SkillName` |
| colors | 2 | 21 | 0 | 21 ✓ | |
| Composit | 2 | 16 | 0 | 16 ✓ | |
| cubemod | 2 | 12 | 0 | 12 ✓ | unreferenced |
| cubetype | 2 | 15 | 0 | 15 ✓ | unreferenced |
| gamble | 2 | 125 | 1 | 125 ✓ | |
| hiredesc | 2 | 9 | 0 | 9 ✓ | |
| HitClass | 2 | 14 | 0 | 14 ✓ | |
| lowqualityitems | 1 | 4 | 0 | 4 ✓ | |
| LvlWarp | 14 | 88 | 1 | 88 ✓ | |
| MonItemPercent | 5 | 2 | 0 | 2 ✓ | |
| MonName | 1 | 43 | 0 | — | unreferenced |
| objgroup | 28 | 133 | 0 | 133 ✓ | `EXPANSION` line is a record |
| ObjMode | 2 | 8 | 0 | 8 ✓ | |
| ObjType | 3 | 573 | 1 | 573 ✓ | |
| PlayerClass | 2 | 7 | 1 | 7 ✓ | |
| PlrType | 2 | 7 | 1 | 7 ✓ | |
| qualityitems | 33 | 8 | 0 | 8 ✓ | |
| RarePrefix | 16 | 46 | 0 | 46 ✓ | patch_d2 `.bin` also 46 |
| RareSuffix | 16 | 155 | 0 | 155 ✓ | patch_d2 `.bin` also 155 |
| StorePage | 2 | 4 | 0 | 4 ✓ | |
| TreasureClass | 28 | 707 | 0 | — | quotes; superseded by `treasureclassex` |
| WeaponClass | 2 | 15 | 0 | — | unreferenced |

**The other 45 d2exp files** (older versions of patch_d2 names; pairs
match their own d2exp `.bin`), as C/R/X: armor 158/202/1, books 11/3/0,
charstats 63/7/1, cubemain 170/271/0, difficultylevels 14/3/0, ElemTypes
2/12/0, experience 8/101/0, gems 41/68/1, hireling 72/60/0, inventory
73/32/1, itemratio 16/6/0, ItemStatCost 16/326/1, ItemTypes 39/91/1, Levels
172/133/1, LvlMaze 7/79/1, LvlPrest 24/1090/1, LvlSub 23/34/1, LvlTypes
36/36/1, MagicPrefix 41/600/1, MagicSuffix 39/675/1, misc 146/139/1,
Missiles 71/644/1, MonMode 2/16/0, monstats 239/575/1, MonType 18/575/1,
npc 19/17/0, objects 160/573/1, Overlay 25/293/1, PlrMode 2/20/0,
Properties 8/244/1, Runes 43/169/0, SetItems 255/32/1, shrines 13/23/0,
skills 98/319/1, SoundEnviron 24/50/0, Sounds 25/4698/0, states 1/160/0,
SuperUniques 10/66/1, TreasureClassEx 28/717/0, UniqueAppellation 37/25/0,
UniqueItems 54/384/1, UniquePrefix 37/53/0, UniqueSuffix 37/69/0,
UniqueTitle 38/16/0, weapons 160/306/1.

**d2data** (every name also exists in d2exp, so these are always
shadowed; X = 0 in all), as C/R: Aiparms E8 (1.14d count 234), Arena 8/1,
armor 141/92, ArmType 2/3, AutoMap 13/2603, belts 68/7, books 11/3,
charstats 62/5, CharTemplate 95/30, Composit 2/16, difficultylevels 9/3,
experience 6/101, gamble 2/107, gems 40/35, inventory 73/12, itemratio
13/2, ItemStatCost 8/177, Levels 169/109, lowqualityitems 1/4, LvlMaze
7/64, LvlPrest 24/863, LvlSub 23/28, LvlTypes 36/29, LvlWarp 12/71,
MagicPrefix 39/129, MagicSuffix 39/115, misc 132/94, Missiles 63/385,
MonItemPercent 5/2, MonMode 2/16, MonName 1/43, monstats 229/410, MonType
18/410, objects 160/410, objgroup 28/97, ObjMode 2/8, ObjType 3/410,
Overlay 25/190, PlrMode 2/20, PlrType 2/5, qualityitems 33/8, RarePrefix
20/46, RareSuffix 20/155, SetItems 125/16, shrines 13/23, skills 83/221,
SoundEnviron 24/36, Sounds 25/3587, SuperUniques 7/42, TreasureClass
34/95, UniqueAppellation 37/25, UniqueItems 49/129, UniquePrefix 37/53,
UniqueSuffix 37/69, UniqueTitle 38/16, weapons 164/175.

## Test vectors

Inputs are byte strings with Rust escapes. "#i [..]" is record `i` and its
cells. Field lists are written `name TYPE@offset` (with `len` where it
matters, `→ L` for a linker or callback). "(no column)" marks a field the
header lacks.

**Reader**

| Input | Expected | Rule |
|---|---|---|
| `"a\tb\r\n1\t2\r\n"` | header `[a, b]`; #0 `[1, 2]` (line 2) | §4, §5 |
| `"code\tv\r\nx\t1\r\nExpansion\t\r\ny\t2\r\n"` | #0 `[x, 1]` (line 2), #1 `[y, 2]` (line 4); removed line 3 | §5 |
| `"code\tv\r\nExpansion\t9\r\nx\t1\r\n"` | #0 `[x, 1]` | §5 |
| `"a\tb\r\nExpansion\r\n1\t2\r\n"` | #0 `[1, 2]`; line 2 removed (1 cell, no E8) | §5 |
| `"Expansion\r\n1\r\n"` | header `[Expansion]`; #0 `[1]` | §4 |
| `"c\r\nEXPANSION\r\nexpansion\r\nExpansion \r\nExpansion\r\n"` | #0 `EXPANSION`, #1 `expansion`, #2 `Expansion ` (trailing space); line 5 removed | §5 |
| `"a\tb\r\n\t\r\n"` | #0 `["", ""]` | §5 |
| `"a\r\n\r\n"` | #0 `[""]` | §5 (`C` = 1) |
| `"n\r\n\"a,b\"\r\n  x \r\n"` | #0 `["\"a,b\""]`, #1 `["  x "]` | §2 |
| `"n\r\n\x85\x92\r\n"` | #0 `[b"\x85\x92"]` | §2 |
| `"a\tb\r\n\r\n"` | E8, line 2 | §5 |
| `"a\tb\r\n1\t2\t\r\n"` | E8, line 2 (trailing tab = third cell) | §5 |
| `"a\r\n1\r\n2"` | E6, line 3 (1.14d: 1 record `[1]`) | §3 |
| `"a\r\nb"` | E6, line 2 (not E2) | §9 order |
| `"a\r\n1\r2\r\n"` | E4, line 2 | §3 |
| `"a\r\n1\r"` | E4, line 2 (not E6) | §9 order |
| `"a\r\n1\n2\r\n"` | E5, line 2 (1.14d: #0 `["1\n2"]`) | §3 |
| `"a\r\n1\n"` | E5, line 2 (not E6) | §9 order |
| `"a\r\nx\x00y\r\n"` | E9, line 2 | §2 |
| `"a\r\n1\r\n\x00"` | E9, line 3 (not E6) | §9 order |
| `"a\n1\n"`, `""`, `"a\r"` | E1 | §9 |
| `"a\r\n"` | E2 | §5 |
| `"a\r\nExpansion\r\n"` | E3 | §5 |
| header of 280 columns + 1 record | OK; 281 columns: E7 | §4 |
| `"\xEF\xBB\xBFa\r\n1\r\n"` | E10 | §2 |
| `"\xFF\xFEa\x00\r\x00\n\x00"` (UTF-16) | E10 | §9 order |

**Binding and field lists** (field list → column bound per field)

| Fields | Header | Result |
|---|---|---|
| `name`, `level` | `Name\tLEVEL\tname` | `name` ← 0, `level` ← 1; column 2 unbound, DupColumn (line 1, column 2) |
| `name` | ` name\tname ` | both columns unbound; `name` missing |
| `mindam` | `mindam\tmaxdam\tmindam` | `mindam` ← 0; DupColumn (line 1, column 2) |
| `a` | `\ta` | column 0 unbound; `a` ← 1 |
| `x`, `X` | any | E13 (duplicate names) |
| a field named `""` | any | E13 |
| 281 fields | any | E13; 280 fields: OK |
| `a BYTE@0`, record_size 0 | any | E13 |
| `a UNKNOWN1@0 len 2`, `b BYTE@1` | any | E13 (type-5 run) |
| `a UNKNOWN1@0 len 2`, `b UNKNOWN1@1 len 2` | any | OK |
| `a WORD@11`, record_size 12 | any | E13; `a WORD@10`: OK |
| `a ASCII@8 len 4`, record_size 12 | any | E13 (needs 13 bytes); `len 3`: OK |
| `a BIT@0 len 95`, record_size 12 | any | OK (byte 11, mask 0x80); `len 96`: E13 |
| `a CODETOWORD@0 → N` with N a name linker | any | E13 |
| `zz BYTE@0` | 280 columns, none named `zz` | E14 (281 slots); 279 columns: OK |

**Integer rule** (cell → u32; then as WORD / BYTE)

| Cell | u32 | i32 | WORD | BYTE |
|---|---|---|---|---|
| `""` | 0x00000000 | 0 | 0x0000 | 0x00 |
| `"12"` | 0x0000000C | 12 | 0x000C | 0x0C |
| `"-12"` | 0xFFFFFFF4 | −12 | 0xFFF4 | 0xF4 |
| `"-"` | 0x00000000 | 0 | 0x0000 | 0x00 |
| `"007"` | 0x00000007 | 7 | 0x0007 | 0x07 |
| `"1 "` | 0xFFFFFFFA | −6 | 0xFFFA | 0xFA |
| `" 1"` | 0xFFFFFF61 | −159 | 0xFF61 | 0x61 |
| `" "` | 0xFFFFFFF0 | −16 | 0xFFF0 | 0xF0 |
| `"+5"` | 0xFFFFFFD3 | −45 | 0xFFD3 | 0xD3 |
| `"1.5"` | 0x00000055 | 85 | 0x0055 | 0x55 |
| `"--1"` | 0x0000001D | 29 | 0x001D | 0x1D |
| `"*16"` | 0xFFFFFDB8 | −584 | 0xFDB8 | 0xB8 |
| `"*12"` | 0xFFFFFDB4 | −588 | 0xFDB4 | 0xB4 |
| `"0x10"` | 0x00001C2A | 7210 | 0x1C2A | 0x2A |
| `"abc"` | 0x0000154B | 5451 | 0x154B | 0x4B |
| `"999"` | 0x000003E7 | 999 | 0x03E7 | 0xE7 |
| `"65536"` | 0x00010000 | 65536 | 0x0000 | 0x00 |
| `"2147483648"` | 0x80000000 | −2147483648 | 0x0000 | 0x00 |
| `"4294967296"` | 0x00000000 | 0 | 0x0000 | 0x00 |
| `"99999999999"` | 0x4876E7FF | 1215752191 | 0xE7FF | 0xFF |
| `"\x85"` | 0xFFFFFF55 | −171 | 0xFF55 | 0x55 |

BIT with len 10 at offset 16 (byte 17, mask 0x04): `"1"`, `"2"`, `"-1"`,
`" "` set it; `"0"`, `""`, `"-"` clear it.

**Diagnostics**

| Field, cell | Stored | Diagnostic |
|---|---|---|
| BYTE `999` | 0xE7 | IntRange |
| BYTE `" "` | 0xF0 | IntSyntax |
| BYTE `-` | 0x00 | IntSyntax |
| BYTE `""` | 0x00 | none |
| BYTE `-128` / `255` | 0x80 / 0xFF | none |
| BYTE `-129` | 0x7F | IntRange |
| WORD `65535` / `65536` | 0xFFFF / 0x0000 | none / IntRange |
| DWORD `-1` | 0xFFFFFFFF | none |
| DWORD `4294967296` | 0x00000000 | IntRange |
| BIT `2` | bit set | IntRange |
| ASCII len 2 `NU0` | `4E 55 00` | TextCut |
| CODETOWORD `staff` (looks up `staf`) | index of `staf` | TextCut |
| NAMETOWORD `zzz`, not a key | `FF FF` | LinkMiss |
| NAMETOWORD `""`, not a key | `FF FF` | none |
| code Register `abc`, then `abc` | — | DupCode on the second |

**Text and codes**

| Type, len | Cell | Bytes at `offset` |
|---|---|---|
| ASCII, 4 | `"abc"` | `61 62 63 00` |
| ASCII, 4 | `"abcdef"` | `61 62 63 64 00` |
| ASCII, 4 | `""` | `00` |
| UNKNOWN6, 4 | `"abcdef"` | `61 62 63 00`; registered name `abc` |
| UNKNOWN6, 0 | `"abc"` | `00`; registered name `""` |
| RAW / code | `""` | `20 20 20 20` |
| RAW / code | `"ab"` | `61 62 20 20` |
| RAW / code | `"abcdef"` | `61 62 63 64` |
| RAW / code | `" a"` | `20 61 20 20` |
| RAW, missing column | — | `00 00 00 00` |
| UNKNOWN4 | `"abc"` | `61`; key `abc ` registered |
| UNKNOWN5 | `"abc"` | `61 62`; key `abc ` registered |

**Whole records** (record bytes after the compile)

| Fields, record_size | Text | Records |
|---|---|---|
| `k ASCIITOCODE@0 → L`, `n CODETOBYTE@4 → L`, `m CODETOWORD@6 → L2` (no column), `r RAW@8` (no column); 12 | `"k\tn\r\na\tb\r\nb\ta\r\n"` | #0 `61 20 20 20 01 00 FF FF 00 00 00 00`; #1 `62 20 20 20 00 00 FF FF 00 00 00 00`; L: `a   ` → 0, `b   ` → 1; L2 unchanged |
| `k ASCIITOCODE@0 → L`; 4 | `"k\r\nabc\r\nabc\r\n"` | #0 `61 62 63 20`; #1 `61 62 63 20` (the cell's code, not the bumped key); L: `abc ` → 0, `bbc ` → 1; DupCode at line 3 |
| `a BYTE@4`, `k ASCIITOCODE@0 → L` (no column); 5 | `"a\r\n1\r\n"` | #0 `00 00 00 00 01`; L gets no key, `n` stays 0 |
| `a WORD@0`, `z NAMETOWORD@0 → N` (no column); 2 | `"a\r\n5\r\n"` | #0 `FF FF` (missing value written last) |
| `b BYTE@0`, `u UNKNOWN1@0 len 1`; 1 | `"b\tu\r\n7\t\r\n7\t-\r\n"` | #0 `07` (empty UNKNOWN1 writes nothing); #1 `00` |
| `s NAMETOWORD2@0 → N`, N empty; 2 | `"s\r\nx\r\n"` | #0 `FF 00` |
| `d NAMETODWORD@0 → N`, `e NAMETODWORD@4 → N` (no column), N = {`x` → 0}; 8 | `"d\r\ny\r\nX\r\n"` | #0 `FF FF FF FF FF FF FF FF`; #1 `00 00 00 00 FF FF FF FF` |
| tables A, B: `c ASCIITOCODE@0 → I`; table C: `i UNKNOWN3@0 → I`; 4 | A `"c\r\nx\r\ny\r\n"`, then B `"c\r\nz\r\n"`, then C `"i\r\nz\r\nx\r\n"` | C #0 `02 00 00 00`, #1 `00 00 00 00` (I is shared; `z` got index 2) |

**Callbacks** (mock callbacks that record their calls)

| Fields, record_size | Text | Calls and records |
|---|---|---|
| `a DWORD@0`, `f CUSTOMLINK@4 len 7 → M`, `g CUSTOMLINK@8 len 9 → M` (no column); 12; M writes nothing | `"f\ta\r\nxy\t5\r\n\t6\r\n"` | M(`Some("xy")`, offset 4, len 7, record 0, slot 0); M(`None`, 8, 9, 0, slot 2); M(`Some("")`, 4, 7, 1, 0); M(`None`, 8, 9, 1, 2). #0 `05 00 00 00` + 8 × `00`; #1 `06 00 00 00` + 8 × `00` |
| `k KEYTOWORD@0 → K`, `m KEYTOWORD@2 → K` (no column); 4; K(text) = 0x100 + text length | `"k\r\nabc\r\n"` | K called once, with `abc`; #0 `03 01 00 00` |
| `k KEYTOWORD@0 → K`; 2 | a 300-byte cell | K gets the first 256 bytes; TextCut |

**Linkers**

| Steps | Result |
|---|---|
| code Register `abc`, `xyz`, `abc` | indices 0, 1, 2; keys `abc `, `xyz `, `bbc ` (0x20636262) |
| then Lookup `abc`, `xyz`, `bbc`, `abd`, `""`, `ABC` | 0, 1, 2, −1, −1, −1 |
| code Register `abc`, `abc`, `bbc` | keys `abc `, `bbc `, `cbc `; Lookup `bbc` → 1, `cbc` → 2 |
| code Register `""` five times | keys 0x20202020–0x20202024, indices 0–4; Lookup `""` → 0 |
| CODETOBYTE / CODETOWORD of a miss | 0xFF / 0xFFFF |
| NAMETOINDEX cells `Fire Bolt`, `fire bolt`, `Ice`, `""` | stored 0, 0, 1, 2; `n` = 3 |
| then Lookup `FIRE BOLT`, `""`, `Fire` | 0, 2, −1 |
| UNKNOWN6 (register-always) `a`, `A`, `b` | `a` → 0, `b` → 2; `n` = 3; DupName on `A` |
| two 40-byte names equal in their first 31 bytes | same key, same index |
| NAMETOINDEX `b"Caf\xE9"` | E11, line 2, column 0 |
| NAMETOINDEX, a 40-byte ASCII cell with 0xE9 at byte 35 | accepted (0xE9 is outside the key); key = first 31 bytes lowercased; TextCut |

**1.14d data** (`#[ignore]` tests reading `D2_GAME_DIR`; P = patch_d2,
X = d2exp)

| Input | Expected |
|---|---|
| every `.txt` in patch_d2, d2exp, d2data | parses, except `Aiparms.txt` in d2exp and d2data (E8, line 13); `C`, `R`, `X` as in Survey |
| each `.txt` with a `.bin` in the same archive (128 pairs) | `R` = the `.bin`'s first u32 |
| P `armor.txt` | 202 records, 1 removed line; `mindam` binds column 63, `maxdam` 64; columns 161, 162 unbound (DupColumn) |
| P `weapons.txt` | column 18 has an empty name and binds nothing |
| X `objgroup.txt` | 133 records; the `EXPANSION` line (line 99) is record 97 |
| P `uniqueitems.txt` | 402 records; record 401 has 70 empty cells |
| P `itemtypes` `code` (10@0) | records 0, 1, 14, 17, 23 empty; `.bin` bytes 0–3 `20 20 20 20` in each; keys 0x20202020–0x20202024 → 0, 1, 14, 17, 23 |
| P `magicprefix` record 0 `itype1` (15@106) | `""` → `00 00` |
| P `runes` record 0 `Rune1` (11@152) | `r08` → 617 |
| P `missiles` `TravelSound` (20@18) | `""` in 344 records → `00 00` |
| P `monstats` `Id` (17@0) | records 617, 723 `cr_lancer8` → 617, 617; 724 `overseer6` → 723; 733 `willowisp8` → 732 |
| P `monseq` `sequence` (17@0) | record 0 `""` → 0; 1 `seq_pinheadsmite` → 1; 1009 `seq_crlancerjab` → 59 |
| P `weapons`, `armor` `spelldescstr` (22, no column) | `00 00` in every record |
| P `misc` record 37 `rarity` (4@252) | `999` → 0xE7; IntRange |
| P `misc` record 45 `TMogMin` (6@314) | `" "` → 0xF0; IntSyntax |
| P `missiles` record 568 `pSrvHitFunc` (3@14) | `*12` → `B4 FD`; IntSyntax |
| P `missiles` record 82 `Explosion` (26@4 len 1) | `2` → byte 4 bit 1 set; IntRange |
| P `skills` record 159 `scroll` (26@4 len 36) | `" "` → byte 8 mask 0x10 set; IntSyntax |
| P `objects` record 0 `Token` (1@192 len 2) | `NU0` → `4E 55 00`; TextCut |

## Provenance

**1.14d `Game.exe`** (Ghidra 12.1.4 project `re/ghidra/D2_114d`:
decompiled exports, instruction listings dumped by scratch scripts;
jump-table and constant bytes read from the PE file):

| Address | Role |
|---|---|
| 0x6BD640 | splitter: tab and CR → NUL, LF kept; CR without LF fatal 92 / 117; no CR, or nothing after the header → returns null; > 280 columns fatal 103; `Expansion` removal (`strncmp` 0x413550, n = 10, `Expansion\0` at 0x6DCF44; the line is deleted by moving the rest of the buffer) |
| 0x6BCDE0 / 0x6BCDA0 | record-count getter (no null check) / free |
| 0x6BCE20 | binding: type-5 runs at 0x6BCE44–0x6BCE7C (fatal 196); field count at 0x6BCEA0 (fatal 204 at the 281st); Storm hash 0x413CE0 and `_strnicmp` 0x413590 (n = 0x7FFFFFFF); fields inserted at bucket heads (so the last field in the list is found first); a bound field's hash is zeroed; missing fields appended to the map in list order |
| 0x6BD780 | compile: count 0 fatal 274, record size 0 fatal 275; map of 280 u16 slots followed by the 256-byte text buffer; pass-1 detection 0x6BE090 / 0x6BE088 and handlers 0x6BE0AC / 0x6BE09C (types 10–18); type 12 / 14 index checks fatal 342 / 346; row-end LF checks 0x6BDAC1 (389) and 0x6BE037 (646); pass-2 class table 0x6BE0D8 (by type − 1) → 0x6BE0B8; integer store table 0x6BE10C (by type − 2) → 0x6BE0F4; missing-field table 0x6BE128 (by type − 1) |
| 0x6BDFD4, 0x6BDFFC, 0x6BE022 | missing-field actions: u16 0 for types 3, 14, 17, 22 (no call); field callback with text 0 for 23–25 when one is set; nothing for 26 |
| 0x6BDEA4 / 0x6BDEF2 | key callback (type 22; text in a 256-byte buffer; result stored as u16) / field callback (23–25; text, record, offset, len, record index, column) |
| 0x6BD230 / 0x6BD130 | code linker register (sorted array; an equal key is incremented and the search retried; fatal 977 null, 984 name linker) / lookup |
| 0x6BD500, 0x6BD5A0, 0x6BD3C0, 0x6BD490 | name linker register-always (fatal 1202 / 1206) / find-or-register (1228 / 1232) / lookup (1141 on a code linker) / tree insert |
| 0x4135D0, 0x4113C0, table 0x6CEB98 | 32-byte bounded copy (31 characters); lowercase map, `A`–`Z` only, indexed by a signed byte (0x6CEB18–0x6CEB97 hold values 0–5) |
| 0x410B10 / 0x410B50 | bit set / clear |
| 0x6122F0, 0x6121F0 | loader: compile block when 0x96C8B4 ≠ 0 (`levels` for `leveldefs`; `.txt` at 0x6D48E0); writer `fopen(…, "wb")`, write skipped when it returns null; then the shared `.bin` read (0x744308 = 1, never written) |
| 0x7063A4–0x7063FF, 0x44D9C0, 0x6125A0 | option record `TXT` / `TXT` / `txt` with target offset 0x215 (dword at 0x7063FC; neighbours: `nocompress` 0x206, `build` 0x223); the init stub passes (byte +0x215 == 0) to 0x6125A0, which stores (arg == 0) in 0x96C8B4 and is its only writer |
| 0x6315D0 | item loader: one linker (0x96BCC4) and one field list for `weapons`, `armor`, `misc`; with compile mode off it registers the combined records' `code` (offset 128) in order with 0x6BD230 |
| 0x481950 | `sounds.txt` / `soundenviron.txt` through 0x6BD640 / 0x6BD780 |

**1.14d data** (scratch scripts in the session scratchpad, not
committed):

- Survey: all 193 extracted `.txt` with the strict reader (numbers above).
- Record counts: 128 of 128 same-archive `.txt`/`.bin` pairs.
- Compile: a scratch model of §5–§8 compiled the live `.txt` (P → X → D;
  `levels.txt` for `leveldefs`) with each of the 91 field lists recovered
  from `Game.exe` (`field-types.md` Provenance), in the load order of
  `loading.md` §6, with linkers kept across tables, and compared count and
  every byte with the live `.bin`. 82 lists are identical except type-25
  fields (callback output) and lookups into hand-built linkers (treasure
  classes; `skills` `range`; `chartemplate` `class`). `monstats2`,
  `monpreset`, `cubemain`: identical in every non-callback field (their
  type-23 callbacks write elsewhere in the record). `monstats`: only record
  707 `NameStr` differs (2 bytes; string tables, `field-types.md` §10). 5
  compile-only lists have no `.bin` of their record size.
- "Checked" in §7 counts bound cells and missing-field values in the 85
  identical lists whose bytes no other field shares.
- The same run: 354,126 empty and 17,040 negative integer cells; 357
  columns bound only through case folding; 11 name lookups that match only
  case-insensitively; empty-key hits 24,442 code and 7,638 name (of which
  `missiles` 505 and 1,600); find-or-register duplicates in `monseq` (950
  duplicates, 1,008 indices ≠ record number) and `monstats` `Id` (1
  duplicate, 11 indices, in the table list and again in the compile-only
  link); no field outside its record; diagnostics as counted in §9 (TextCut:
  ASCII 1, code 12, name 33; DupColumn: `armor` 2, `automap` 1,
  `chartemplate` 8; DupCode: `itemtypes` 4).
- Cross-archive sources compile to the live `.bin`: d2exp `automagic`,
  `RarePrefix`, `RareSuffix` `.txt` → patch_d2 `.bin`; patch_d2
  `inventory`, `plrmode` `.txt` → d2exp `.bin`.

**D2MOO (MIT, 1.10f)**: names only (`D2BinFieldStrc`, `TXTFIELD_*` ids,
linker names) and the CompileTxt flow (`DataTbls.cpp`). D2MOO does not
contain the Fog text parser (`Fog/src/Excel/Excel.cpp` is empty); every
rule above comes from the 1.14d binary and data.

## Open questions

1. In compile mode, whether the `.bin` read after writing is the loose file
   just written or the archive copy: `loading.md` open questions 2–3.
2. Table-specific field callbacks (types 23–25): what each 1.14d callback
   does with a bound cell and with no text. Per-table specs
   (`field-types.md` §8).
3. Bytes ≥ 0x80 in name keys: 1.14d's lowercase step reads the 128 bytes
   before its table (values 0–5; a 0 ends the key). Not reproduced (E11);
   no 1.14d key has such a byte.
4. Rules confirmed from the binary only (1.14d data cannot show them):
   types 5, 7, 12, 14, 16, 24 (in no 1.14d list); the missing-column values
   of types 5, 7, 8, 10, 11, 12, 14, 16, 17, 18, 19, 21, 26 (no 1.14d table
   lacks such a column); the field-callback arguments, `slot` in
   particular; the code-linker +1 bump (no 1.14d lookup uses a bumped key);
   the 31-byte name-key cap (33 longer names, none collide); the 256-byte
   callback text cap (longest cell 199 bytes); the 280 limits (max 256
   columns, 253 entries, 256 slots); the row-end LF checks (no ragged
   1.14d row); the type-5 run check and the other fatal codes of §6.1.
5. Quote handling inside callback columns (cube inputs, treasure-class
   items, calc expressions, `monstats2` component lists) is up to those
   callbacks. Not examined here.
6. Linkers rebuilt from `.bin` records at runtime other than the item codes
   (0x6315D0, same register routine, same order): per-table specs.
7. The sound tables' field lists (`SoundHdr.cpp`) are not examined
   (`loading.md` open question 11).
8. `field-types.md` to align: its empty-key hit counts (23,937 code, 6,038
   name) leave out `missiles` (505, 1,600); its Edge-cases note on the
   type-22 missing column is settled by §7 (u16 0, no call).
