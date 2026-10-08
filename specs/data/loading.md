# Spec: Data — Table loading (which files 1.14d loads)

- **Status:** verified by `data-tool tables` (2026-10-05). Rules checked against the 1.14d `Game.exe` (Ghidra
  exports plus raw bytes) and the 1.14d data files on 2026-10-05, except
  where an Open question says otherwise; see Provenance. Implemented in
  `d2-data::bin` (§2–§4, §6, §8, §10.8), `d2-data::fixup` (part of
  §7.4, see the d2rs paragraph there) and checked by `data-tool tables`
  (§11, "d2rs cross-check").
- **Target version:** 1.14d
- **Crate/module:** `d2-data::bin` (loader, checks); `d2-data::crosscheck` (§11)
- **Related specs:** `specs/formats/mpq.md` (archive reads, archive set),
  `specs/formats/tbl.md` (string tables, loaded before any excel table),
  `specs/data/txt-format.md` (`.txt` reader, record numbering, column
  binding, cell conversions, linkers, error policy),
  `specs/data/field-types.md` (type vocabulary, string keys, callbacks,
  `@tc`), `specs/data/calc-expressions.md` (code buffers),
  `specs/data/patch-layers.md` (mod layers), `specs/data/fixups.md` and
  `specs/data/runtime-maps.md` (§7.4 rules), `specs/formats/animdata.md`
  (`AnimData.d2`, `expfield.d2`).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 48–61 |
| Inputs | 62–69 |
| Outputs / state changes | 70–77 |
| Rules | 78–83 |
|   1. Paths | 84–96 |
|   2. Archive search order | 97–153 |
|   3. Choosing `.bin` or `.txt` | 154–217 |
|   4. The `.bin` container | 218–275 |
|   5. `.txt` record counting | 276–287 |
|   6. Load order and record sizes (runtime tables) | 288–375 |
|   7. Links and dependencies | 376–512 |
|   8. Post-load checks (fatal in 1.14d) | 513–561 |
|   9. Combined index spaces | 562–576 |
|   10. Special cases | 577–646 |
|   11. Txt vs bin cross-check | 647–687 |
| Constants & data dependencies | 688–708 |
| Randomness | 709–712 |
| Edge cases & original bugs | 713–729 |
| d2-data policy | 730–756 |
| Test vectors | 757–793 |
| Provenance | 794–882 |
| Open questions | 883–974 |
<!-- /index -->

## Summary

At startup 1.14d loads its game data ("excel" tables) once, in a fixed
order, from `DATA\GLOBAL\EXCEL\` inside the MPQ archive set. In normal play
it reads only the precompiled `.bin` files: a u32 record count followed by
fixed-size records whose size is hard-coded per table. It never falls back
to `.txt`; a missing `.bin` is a fatal error. The `.txt` files are used only
by the `-txt` command-line mode, which compiles them to `.bin`, and by two
exceptions that read `.txt` at runtime (`sounds.txt` and `soundenviron.txt`,
read by the sound system). This spec lists every file 1.14d reads, its live
source, record count and size, the load order, the link dependencies
between tables, the checks that abort loading, and how the shipped `.txt`
files relate to the live `.bin` files.

## Inputs

| Name | Type | Source |
|---|---|---|
| archive set | opened MPQs, searched by priority | user's install (`game/*.mpq`) |
| table name | lowercase ASCII, e.g. `itemtypes` | fixed list (§6) |
| string tables | `string.tbl`, `patchstring.tbl`, `expansionstring.tbl` | loaded first (§10.1) |

## Outputs / state changes

- Per table: its record count and `count × record_size` record bytes.
- Combined arrays for tables that the game concatenates (§9).
- Two runtime-parsed `.txt` tables for the sound system (§3.4).
- Four raw formula-code buffers (§4.3).
- Loading is all-or-nothing: any failure in §8 ends the process.

## Rules

Abbreviations: **P** = `patch_d2.mpq`, **X** = `d2exp.mpq`,
**D** = `d2data.mpq`. "Data line" = a CR LF-terminated line after the
header (`txt-format.md` §5).

### 1. Paths

1. Every excel file path is `DATA\GLOBAL\EXCEL\` + table name + extension.
   The game builds it as the directory string, a backslash, the name, then
   `.bin` or `.txt`.
2. Table names are the lowercase names in §6. MPQ name lookup is
   case-insensitive (`mpq.md` §3), so `PlrMode.txt` in X is the same file
   as `plrmode.txt`.
3. Two other files load inside the same load sequence but are not excel
   tables: `DATA\GLOBAL\AnimData.d2` (after `experience`) and
   `DATA\GLOBAL\expfield.d2` (after `inventory`). Formats:
   `formats/animdata.md`.

### 2. Archive search order

1.14d opens archives with a priority value. A file is read from the first
archive, in search order, that contains it (`mpq.md` §5 lookup). The search
order is: higher priority first; among equal priorities, the archive opened
later comes first.

| Archive | Priority | Opened |
|---|---|---|
| `d2data.mpq` | 1000 | startup group, 1st |
| `d2sfx.mpq` | 1000 | startup group, 2nd |
| `d2speech.mpq` | 1000 | startup group, 3rd |
| `d2delta.mpq` | 1000 | startup group, 4th (optional; absent in this install) |
| `d2kfixup.mpq` | 1000 | startup group, 5th (optional; absent in this install) |
| `patch_d2.mpq` | 5000 | startup group, 6th |
| `d2exp.mpq` | 3000 | startup group, 7th |
| `d2char.mpq` | 1000 | second group |
| `d2music.mpq` | 1000 | second group |
| `d2xmusic.mpq` | 3000 | second group, LoD only |
| `d2xtalk.mpq` | 3000 | second group, LoD only |
| `d2xvideo.mpq` | 3000 | second group, LoD only (a video path can also open it at 1000) |
| `d2video.mpq` | 1000 | video path |

LoD only: d2exp exists (§10.7) and the launcher configuration's callback
at +0x211, when present, returns non-zero.

Excel files exist only in P, X and D (name-hash probe of every candidate
name, `.txt`/`.bin`/`.xls`, in all 11 archives). Their priorities differ,
so the **excel search order is P → X → D**, independent of the tie rule
(41 X `.bin` files have an older record size, §11, so P must come first).

d2rs: `d2_formats::mpq::ArchiveSet` searches in this order, assuming the
second group opens after the startup group and the video path last (Open
question 1): `patch_d2`, `d2xvideo`, `d2xtalk`, `d2xmusic`, `d2exp`,
`d2video`, `d2music`, `d2char`, `d2speech`, `d2sfx`, `d2data`. Excel
lookups need only P → X → D, so they do not depend on that assumption.

**Loose files.** The archive layer's open (`0x004068E0` → `0x00419980`
→ `0x004192F0`) consults the disk only through its direct-access flags
`0x00779058` (read by `0x00418B50`). Those are set once at startup from
the launcher config byte +0x204, the `-direct` switch
(`tools/original-hooks.md` §5.1; `0x00405C55` → `0x00406870` →
`0x00415F70`, the only writer). Bit 0 or 1 set: before the first archive
is searched, the lookup `0x00417F10` tests the name on disk
(`0x00415B00`; bit 1: a path derived from the name by `0x00415A80`;
bit 0: the name itself, used as-is when it starts with `\` or contains
`:\` or `\\` (a UNC path); a hit is an existing non-directory per
`GetFileAttributesA`, `0x00415940`) and opens that file with
`CreateFileA` instead (result kind 2). With the flags 0 and at least one
archive open, no disk file is ever read. Normal play passes no
`-direct`, so 1.14d reads every data file from the archives. (With no
archive open and flags 0, `0x00418B50` returns bit 0: disk only; not a
case d2rs meets.)
With this order every live `.bin` resolves to the archive §6 names (56 P,
17 X; confirmed by bin cross-check). d2rs does not open `d2delta` or
`d2kfixup`.

### 3. Choosing `.bin` or `.txt`

#### 3.1 Normal play: `.bin` only

Per table, with the compile switch (§3.2) off:
1. Read `DATA\GLOBAL\EXCEL\<name>.bin` through the archive set.
2. If the file is not found: fatal error ("Unrecoverable internal error"),
   process exit. There is **no `.txt` fallback** (the `.txt` branch behind
   a load-from-bin switch that is constant 1 is dead code).
3. Record count = the u32 at offset 0. Records = the bytes from offset 4.

#### 3.2 `-txt` mode (compile; not reproduced by d2rs)

The command-line option `-txt` sets the compile switch (0 otherwise).
Then, for each table in §6, before step 1 above:
1. Read `DATA\GLOBAL\EXCEL\<name>.txt` (when the table name is
   `leveldefs`, in any case: `levels.txt`). Missing file: fatal error.
2. Parse and compile it (`txt-format.md`, `field-types.md`): `count × record_size`
   bytes (zero-filled, then fields written). Write
   `DATA\GLOBAL\EXCEL\<name>.bin` to the disk path relative to the current
   directory: u32 count, then the records. If the file cannot be created,
   the write is skipped without an error.
3. Continue with §3.1 (the `.bin` is read back as usual).

The compile switch also enables the compile-only tables (§7.2), which exist
only to build name/code links (§7) while compiling. d2rs never runs this
mode; it is described because it defines how a `.txt` maps to a `.bin`.

#### 3.3 Server-only files must be absent

| Check (before loading) | 1.14d result |
|---|---|
| `runessrv.txt`, `runessrv.bin` or `runessrv.xls` found (before `runes`) | fatal error |
| `cubeserver.bin` or `cubeserver.txt` found (before `cubemain`) | fatal error |

None of these files exists in any archive of the 1.14d install.

#### 3.4 Runtime `.txt` exceptions (sound system)

The client sound system parses two `.txt` files itself, at its first
initialization, with the same parser (§5). They have no `.bin` path:

| File | Live source | Data lines | Columns | In-memory record |
|---|---|---|---|---|
| `sounds.txt` | P (505,304 bytes) | 4,699 | 25 | 142 bytes |
| `soundenviron.txt` | P (5,741 bytes) | 50 | 24 | 88 bytes |

`sounds.bin` (P, 4,699 × 2) is a compile-mode by-product (§7.2) and is never
read in normal play. 1.14d does not check that either file exists or
parses (a missing one crashes at the record count); d2rs: load error.

#### 3.5 Client composite loader

The client's unit-graphics (composite) code loads its own copies of five
tables, lazily on first use, with its own load-from-bin switch (also a
constant 1). Same path rule, same fatal error on a missing file, same
record sizes:

| Table | Record size | Note |
|---|---|---|
| `itemtypes` | 228 | same file as §6 step 2 |
| `hitclass` | 4 | the only runtime reader of `hitclass.bin` (X, 14 × 4) |
| `weapons`, `armor`, `misc` | 424 | concatenated in that order, as in §9 |

### 4. The `.bin` container

#### 4.1 Layout

All integers little-endian.

| Offset | Size | Field |
|---|---|---|
| 0x0 | 4 | `count` (u32; the game treats it as a signed int) |
| 0x4 | `count × record_size` | records, packed back to back |

There is no header magic, version, padding or trailer. `record_size` is
not stored in the file; it is fixed per table by the code (§6).

#### 4.2 Validation

1.14d does **not** check the file size; it trusts `count`. d2rs is strict:

1. `file_len ≥ 4`.
2. `file_len == 4 + count × record_size`, with `record_size` from §6,
   computed without overflow (count 0 passes). Otherwise reject the file
   (it is from another version or corrupt).
3. Apply the 1.14d post-load checks (§8) and the d2rs count checks
   (§10.8).

Every live `.bin` in the 1.14d install passes rule 2 exactly (§6, §11).

File records hold little-endian fields at fixed offsets; bytes no field
writes are 0 (`field-types.md` §9). The game's in-memory records are the
file records plus the post-load fix-ups of §7.4. Decode fields with
explicit reads at the offsets given by each table's spec, then apply
§7.4.

#### 4.3 Formula code buffers (no count header)

Four raw `.bin` files hold compiled formula code that table fields point
into (by byte offset). Each is read whole; its byte length is kept as its
size. There is no count header and no record size.

| File | Live source | Bytes | Loaded right after | Compiled from |
|---|---|---|---|---|
| `misscode.bin` | P | 196 | `missiles` | `missiles` |
| `skillscode.bin` | P | 5,891 | `skilldesc` | `skills` |
| `skilldesccode.bin` | P | 4,252 | `skillscode.bin` | `skilldesc` |
| `itemscode.bin` | P | 158 | `misc` | `weapons`, `armor`, `misc` |

They exist only in P. In `-txt` mode each is rewritten from the formula
columns of its "Compiled from" tables (`calc-expressions.md` §1.1). A
missing code file does not abort loading: the code-file loader
`0x00613E90` reads through `0x00612280` → `0x00517079`, whose open step
`0x00516E46` on failure only writes the log line `Error opening file: %s`
(`0x00410610`, a non-fatal log) and returns 0. The loader then stores a
null buffer pointer and copies its never-written local size variable
into the size field (an uninitialized stack value). The evaluator
entries test the pointer before the size (e.g. `0x0064B812`), so every
formula of that family evaluates to 0 (`calc-expressions.md` §3.1).
d2rs treats a missing code file as a load error.

### 5. `.txt` record counting

Lines, header, `Expansion` removal, record numbering and errors:
`txt-format.md` §2–§5 and §9. d2rs uses that strict reader for every
`.txt`, the sound tables (§3.4) included. Record `i` of a compiled `.bin`
is record `i` of the `.txt`. The original's handling of an unterminated
last line is not reproduced (`txt-format.md` E6, §10).

Observed in the 1.14d files: every excel `.txt` uses CR LF and ends with
CR LF; no file has a LF-only line; the widest is `skills.txt` (P, 256
columns).

### 6. Load order and record sizes (runtime tables)

The full load is one call made once at process start, after the string
tables (§10.1). Steps run in this order. "Size" is the record size passed
by the 1.14d code; "Live" is the archive whose `.bin` the game reads.
"txt rows" lists every `.txt` copy as archive + record count (§5).

| # | Table | Size | Live | Records | `.bin` bytes | txt rows | Notes |
|---|---|---|---|---|---|---|---|
| 1 | compcode | 4 | P | 115 | 464 | P 115 | only always-loaded table of the lookup group (§7.2) |
| 2 | itemtypes | 228 | P | 103 | 23,488 | P 103, X 91 | also §3.5 |
| 3 | montype | 12 | P | 59 | 712 | P 59, X 575, D 410 | |
| 4 | pettype | 224 | P | 20 | 4,484 | P 20 | count ≤ 255 |
| 5 | overlay | 132 | P | 293 | 38,680 | P 293, X 293, D 190 | |
| 6 | itemstatcost | 324 | P | 359 | 116,320 | P 359, X 326, D 177 | count ≤ 511 |
| 7 | properties | 46 | P | 268 | 12,332 | P 268, X 244 | |
| 8 | missiles | 420 | P | 684 | 287,284 | P 684, X 644, D 385 | then `misscode.bin` |
| 9 | states | 60 | P | 185 | 11,104 | P 185, X 160 | count ≤ 255 |
| 10 | skills | 572 | P | 357 | 204,208 | P 357, X 319, D 221 | count ≤ 32,766 |
| 11 | skilldesc | 288 | P | 221 | 63,652 | P 221 | count ≤ 32,766; then `skillscode.bin`, `skilldesccode.bin` |
| 12 | charstats | 196 | P | 7 | 1,376 | P 7, X 7, D 5 | |
| 13 | arena | 28 | X | 1 | 32 | X 1, D 1 | count not kept |
| 14 | chartemplate | 240 | X | 30 | 7,204 | X 30, D 30 | §8 |
| 15 | weapons | 424 | P | 306 | 129,748 | P 306, X 306, D 175 | §9; also §3.5 |
| 16 | armor | 424 | P | 202 | 85,652 | P 202, X 202, D 92 | §9 |
| 17 | misc | 424 | P | 151 | 64,028 | P 151, X 139, D 94 | §9; then `itemscode.bin` |
| 18 | magicsuffix | 144 | P | 747 | 107,572 | P 747, X 675, D 115 | §9 |
| 19 | magicprefix | 144 | P | 669 | 96,340 | P 669, X 600, D 129 | §9 |
| 20 | automagic | 144 | P | 36 | 5,188 | X 36 | §9; no P txt |
| 21 | raresuffix | 72 | P | 155 | 11,164 | X 155, D 155 | §9; no P txt |
| 22 | rareprefix | 72 | P | 46 | 3,316 | X 46, D 46 | §9; no P txt |
| 23 | uniqueitems | 332 | P | 402 | 133,468 | P 402, X 384, D 129 | count ≤ 32,766 |
| 24 | sets | 296 | P | 32 | 9,476 | P 32 | count ≤ 32,766 |
| 25 | setitems | 440 | P | 127 | 55,884 | P 127, X 32, D 16 | count ≤ 32,766 |
| 26 | gems | 192 | P | 68 | 13,060 | P 68, X 68, D 35 | |
| 27 | books | 32 | P | 3 | 100 | P 3, X 3, D 3 | |
| 28 | qualityitems | 112 | X | 8 | 900 | X 8, D 8 | |
| 29 | lowqualityitems | 34 | X | 4 | 140 | X 4, D 4 | |
| 30 | runes | 288 | P | 169 | 48,676 | P 169, X 169 | §3.3 check first |
| 31 | itemratio | 68 | P | 6 | 412 | P 6, X 6, D 2 | |
| 32 | gamble | 12 | X | 125 | 1,504 | X 125, D 107 | every code must be an item (§8) |
| 33 | plrtype | 52 | X | 7 | 368 | X 7, D 5 | §9 |
| 34 | plrmode | 52 | X | 20 | 1,044 | P 20, X 20, D 20 | §9; txt in P, bin in X |
| 35 | monmode | 52 | P | 16 | 836 | P 16, X 16, D 16 | |
| 36 | objtype | 52 | X | 573 | 29,800 | X 573, D 410 | §9 |
| 37 | objmode | 52 | X | 8 | 420 | X 8, D 8 | §9 |
| 38 | composit | 52 | X | 16 | 836 | X 16, D 16 | count not kept |
| 39 | armtype | 52 | X | 3 | 160 | X 3, D 3 | count not kept |
| 40 | experience | 32 | P | 101 | 3,236 | P 101, X 101, D 101 | count not kept; then `AnimData.d2` |
| 41 | uniquetitle | 2 | P | 16 | 36 | P 16, X 16, D 16 | 41–44: one u16 per row, the string id of `Name` |
| 42 | uniqueprefix | 2 | P | 53 | 110 | P 53, X 53, D 53 | |
| 43 | uniquesuffix | 2 | P | 69 | 142 | P 69, X 69, D 69 | |
| 44 | uniqueappellation | 2 | P | 25 | 54 | P 25, X 25, D 25 | |
| 45 | monlvl | 120 | P | 111 | 13,324 | P 111 | |
| 46 | treasureclassex | 736 | P | 853 | 627,812 | P 853, X 717 | §10.6 |
| 47 | monstats2 | 308 | P | 609 | 187,576 | P 609 | |
| 48 | monprop | 312 | P | 13 | 4,060 | P 13 | |
| 49 | monsounds | 148 | P | 141 | 20,872 | P 141 | |
| 50 | monseq | 6 | P | 1,010 | 6,064 | P 1,010 | |
| 51 | monstats | 424 | P | 734 | 311,220 | P 734, X 575, D 410 | count ≤ 32,766 |
| 52 | monumod | 32 | P | 43 | 1,380 | P 43 | |
| 53 | superuniques | 52 | P | 66 | 3,436 | P 66, X 66, D 42 | §8 |
| 54 | monpreset | 4 | P | 229 | 920 | P 229 | |
| 55 | hireling | 280 | P | 120 | 33,604 | P 120, X 60 | §8 |
| 56 | npc | 76 | P | 17 | 1,296 | P 17, X 17 | |
| 57 | monequip | 28 | P | 45 | 1,264 | P 45 | count ≤ 32,766 |
| 58 | levels | 544 | P | 137 | 74,532 | P 137, X 133, D 109 | count ≤ 1,023 |
| 59 | leveldefs | 156 | P | 137 | 21,376 | — | compiled from `levels.txt`; count not kept (§10.2) |
| 60 | lvltypes | 1,928 | P | 36 | 69,412 | P 36, X 36, D 29 | |
| 61 | lvlprest | 432 | P | 1,091 | 471,316 | P 1,091, X 1,090, D 863 | |
| 62 | lvlwarp | 48 | X | 88 | 4,228 | X 88, D 71 | |
| 63 | lvlmaze | 28 | P | 81 | 2,272 | P 81, X 79, D 64 | |
| 64 | lvlsub | 348 | P | 34 | 11,836 | P 34, X 34, D 28 | |
| 65 | automap | 44 | X | 3,286 | 144,588 | X 3,286, D 2,603 | §8 |
| 66 | objects | 448 | P | 573 | 256,708 | P 573, X 573, D 410 | |
| 67 | objgroup | 52 | X | 133 | 6,920 | X 133, D 97 | |
| 68 | shrines | 184 | P | 23 | 4,236 | P 23, X 23, D 23 | |
| 69 | inventory | 240 | X | 32 | 7,684 | P 32, X 32, D 12 | count must be 32; txt in P, bin in X; then `expfield.d2` |
| 70 | belts | 264 | X | 14 | 3,700 | X 14, D 7 | count ÷ 2 must be 7 |
| 71 | monitempercent | 4 | X | 2 | 12 | X 2, D 2 | |
| 72 | cubemain | 328 | P | 151 | 49,532 | P 151, X 271 | §3.3 check first |
| 73 | difficultylevels | 88 | P | 3 | 268 | P 3, X 3, D 3 | count must be 3 |

Totals: 73 record tables (56 live in P, 17 in X) + 4 code buffers + 1
client-only table (`hitclass`, X) = 78 `.bin` files, plus 2 runtime `.txt`.
All 18 live X files have exactly the 1.14d record size; X is the real
1.14d source for them, not a stale copy.

### 7. Links and dependencies

#### 7.1 Links

A **link** is a name→index (or code→index) map built from one table's key
column. Another table's column that names a row (e.g. an item type code, a
skill name) is stored in the `.bin` as that row's index, resolved through
the link at compile time. So in normal play the `.bin` files already hold
indices; load order still matters for the post-load fix-ups and runtime
maps (§7.4), which read other tables. For a `.txt` compiler the order is
mandatory: a table can be compiled only after every table it links to
(`field-types.md` §6.5).

#### 7.2 Compile-only lookup tables

With `-txt`, these load at step 1 (`sounds` after step 3; `monstats` and
`skilldesc` inside step 10) only to build links. Their `.bin` files are
by-products that normal play never reads, except `hitclass.bin` (§3.5).
A **code** key is a `key(code4)` field (type 10): the record stores the
4-byte code (`compcode.bin` starts `nil `, `lit `). A **name** key is a
`key(name16)` field (type 17): the record stores its find-or-add index as
u16 (= the row index while keys are unique; duplicates: `field-types.md`
§6.2).

| Table | Key column | Kind | Size | `.bin` present | `.txt` present |
|---|---|---|---|---|---|
| playerclass | code | code | 4 | X 7 | X 7 |
| bodylocs | code | code | 4 | X 11 | X 11 |
| storepage | code | code | 4 | X 4 | X 4 |
| elemtypes | code | code | 4 | P 13 (X 12) | P 13, X 12 |
| hitclass | code | code | 4 | X 14 | X 14 |
| colors | code | code | 4 | X 21 | X 21 |
| hiredesc | code | code | 4 | X 9 | X 9 |
| monmode | code | code | 4 | overwritten by step 35 | P 16 |
| plrmode | code | code | 4 | overwritten by step 34 | P 20 (only P has `Code`) |
| monai | AI | name | 2 | P 148 | P 148 |
| monplace | code | name | 2 | P 37 | P 37 |
| skillcalc | code | code | 4 | P 73 | P 73 |
| misscalc | code | code | 4 | P 43 | P 43 |
| skills | skill | name | 2 | overwritten by step 10 | P 357 |
| events | event | name | 2 | P 13 | P 13 |
| sounds | Sound | name | 2 | P 4,699 | P 4,699 |
| monstats | Id | name | 2 | overwritten by step 51 | P 734 (loaded inside step 10) |
| skilldesc | skilldesc | name | 2 | overwritten by step 11 | P 221 (loaded inside step 10) |

`compcode` (step 1) uses the same mechanism but is always loaded.

#### 7.3 Dependency table

"Links to" lists the tables whose links a table's `.txt` columns use
(from the 1.14d field tables, and for formula columns
`calc-expressions.md` §4.4); `*` marks a compile-only link (§7.2).
Every dependency points to an earlier step, so §6 order is a valid
compile order. Every table with `strkey` fields also needs the string
tables (§10.1) at compile time (`field-types.md` §7).

| Table | Links to |
|---|---|
| itemtypes | bodylocs*, playerclass*, storepage* |
| itemstatcost | events* |
| properties | itemstatcost |
| missiles | elemtypes*, misscalc*, overlay, skillcalc*, skills*, sounds* |
| states | colors*, events*, itemstatcost, itemtypes, missiles, overlay, skills*, sounds* |
| skills, skilldesc | elemtypes*, events*, itemstatcost, itemtypes, misscalc*, missiles, monmode*, overlay, pettype, playerclass*, plrmode*, skillcalc*, sounds*, states; compile-only monstats and skilldesc name links and `@skillrange` (§10.3) |
| charstats | bodylocs*, skills |
| weapons, armor, misc | hitclass*, itemstatcost, itemtypes, sounds*, states |
| magic affixes | colors*, itemtypes, playerclass*, properties |
| rare affixes | itemtypes |
| uniqueitems, sets, setitems | colors*, properties, sounds* |
| gems | properties, item codes |
| runes | itemtypes, properties, item codes |
| books | skills |
| qualityitems | properties |
| monsounds | monmode*, skills, sounds* |
| monstats2 | monmode*, skills |
| monprop | properties |
| monseq | monmode* |
| monstats | elemtypes*, missiles, monai*, monmode*, monprop, monsounds, monstats2, montype, skills, `@tc` (§10.6) |
| monumod | montype |
| superuniques | monsounds, monstats, `@tc` |
| hireling | hiredesc*, skills |
| npc | monstats |
| monequip | bodylocs*, monstats, item codes |
| treasureclassex | own resolver (§10.6): items, item types, other TCs |
| levels | monstats |
| cubemain | playerclass*, properties |
| all others in §6 | — |

#### 7.4 Post-load fix-ups and runtime maps

Right after its `.bin` loads (and before the next step of §6), a loader
rewrites some record bytes and may build maps outside the records. The
rules are owned by `fixups.md` (record bytes) and `runtime-maps.md`
(maps); this table lists them by load step. "→ id" is a string id
(`fixups.md` §1).

| Step | Table | Fix-up (owner §) |
|---|---|---|
| 2 | itemtypes | rebuilds its code link (`field-types.md` §6.6); equivalence matrix (`runtime-maps.md` §2) |
| 3 | montype | equivalence matrix (`runtime-maps.md` §2) |
| 6 | itemstatcost | op clamp, op-base lists +0x5E, op-stat tables +0xDE, flags +0x51–0x53 and +0x04 bits 5–8 (`fixups.md` §2); `stuff`, description list (`runtime-maps.md` §3) |
| 8 | missiles | +0x183 capped at 8 (`fixups.md` §10) |
| 9 | states | flag bitsets and lists (`runtime-maps.md` §4) |
| 10 | skills | class and passive lists (`runtime-maps.md` §5); pettype lists +0xBC/+0xC0 (`fixups.md` §3) |
| 12 | charstats | class name, wide, +0x00 (`fixups.md` §4) |
| 15–17 | weapons, armor, misc | combined array (§9); item code map: `code` (+0x80) of the combined records, in order, code-linker add (`field-types.md` §6.1, §6.6); version-0 list (`runtime-maps.md` §6) |
| 18–22 | magic, rare affixes | name → id (`fixups.md` §7) |
| 23 | uniqueitems | index, name link, name → id (`fixups.md` §6) |
| 24–25 | sets, setitems | index, name link, name → id, set attachment (`fixups.md` §6) |
| 26 | gems | +0x2C id, items `gemoffset` +0xF0 (`fixups.md` §5) |
| 28–30 | qualityitems, lowqualityitems, runes | → id (`fixups.md` §7) |
| 32 | gamble | sorted index list and level thresholds; records freed (`runtime-maps.md` §7) |
| 50 | monseq | sequence index (`runtime-maps.md` §8) |
| 51 | monstats | class chain +0x4A/+0x4B, BaseId repair, speeds +0x36/+0x38 from `AnimData.d2` (`fixups.md` §8) |
| 52 | monumod | count clamp 256 (`fixups.md` §10) |
| 53 | superuniques | hcIdx map (§8) |
| 54 | monpreset | per-act ranges (`runtime-maps.md` §8) |
| 55 | hireling | names → ids (`fixups.md` §7), §8 checks, id tables (`runtime-maps.md` §8) |
| 57 | monequip | monstats +0x2A, loc clears (`fixups.md` §9) |
| 58 | levels | wide names +0x16E/+0x1BE, monster counts +0x33–0x35 (`fixups.md` §11) |
| 59 | leveldefs | portal list (`runtime-maps.md` §9) |
| 60, 61, 64 | lvltypes, lvlprest, lvlsub | tile path prefix `DATA\GLOBAL\TILES\`, `/` → `\` (`fixups.md` §12); lvlsub type index (`runtime-maps.md` §9) |
| 65 | automap | converted records, level-name ranges; records freed (`runtime-maps.md` §10) |
| 66 | objects | wide name +0x40, `FrameCnt0`–`7` × 256 (`fixups.md` §13) |

Every table and map in this list was compared with 1.14d memory after
the load (`dump_tables.py`, 2026-10-06; `fixups.md`, `runtime-maps.md`
Provenance). `AnimData.d2` (§1.3) loads before `monstats`, whose fix-up
reads it.

d2rs (`d2-data::fixup`, applied to a copy of the loaded set so the
shipped bytes stay comparable, §11) applies every row of the table above
(2026-10-06; `fixup::PENDING` is empty), with `AnimData.d2` read by
`d2-formats::animdata`. Before that step `data-tool dump-compare` gave 57
of 70 tables identical; the full comparison on game files is queued
(`docs/HANDOFF.md` §5).

### 8. Post-load checks (fatal in 1.14d)

Each failure prints "Unrecoverable internal error" and exits. d2rs reports
them as load errors.

Offsets are hex record offsets; ids are the §7.4 string ids.

| Table | Condition that must hold |
|---|---|
| any table | its `.bin` (or `.txt` in `-txt` mode) is found |
| pettype | count ≤ 255 |
| itemstatcost | count ≤ 511 |
| states | count ≤ 255 |
| skills, skilldesc | count ≤ 32,766 each |
| chartemplate | Level (u8 +0x1F) never smaller than an earlier row's |
| uniqueitems | count ≤ 32,766 |
| sets, setitems | count ≤ 32,766 each |
| gamble | each row's code (u32 +0x00) is a key of the item code map (§7.4), exact 4 bytes |
| treasure classes | TC count (§10.6) ≤ 65,534 |
| monstats | count ≤ 32,766 |
| superuniques | every value 0–65 (fixed 66) occurs as hcIdx (u32 +0x08) in some row; the first such row is used; values ≥ 66 are ignored. A count above 511 is cut to 512 (rows from 512 on are ignored; d2rs reproduces this, no error) |
| hireling | only if d2exp exists, every row: Id (u32 +0x04) ≤ 255; NameFirst id (+0x114) ≠ 0; NameLast id (+0x116) > NameFirst id |
| monequip | count ≤ 32,766 |
| levels | count ≤ 1,023 |
| automap | LevelName is in list A and TileName in list B (below); Cel1 (i32 +0x1C) ≠ −1 |
| inventory | count = 32 |
| belts | count ÷ 2 (integer) = 7, i.e. 14 or 15 |
| difficultylevels | count = 3 |
| runes, cubemain | §3.3 server files absent |

automap names: a value whose first byte is the character `0` (0x30)
gives 0 without a compare; otherwise an exact, case-sensitive compare
against the list, and the converted value is the list index (from 0). A
value in neither case, the empty string included, fails the check (fatal
0x4A5 for LevelName, 0x4D0 for TileName; Cel1 = −1: 0x50D). The
converted form: `runtime-maps.md` §10.
- A (LevelName, 36): `None`, `1 Town`, `1 Wilderness`, `1 Cave`,
  `1 Crypt`, `1 Monestary`, `1 Courtyard`, `1 Barracks`, `1 Jail`,
  `1 Cathedral`, `1 Catacombs`, `1 Tristram`, `2 Town`, `2 Sewer`,
  `2 Harem`, `2 Basement`, `2 Desert`, `2 Tomb`, `2 Lair`, `2 Arcane`,
  `3 Town`, `3 Jungle`, `3 Kurast`, `3 Spider`, `3 Dungeon`, `3 Sewer`,
  `4 Town`, `4 Mesa`, `4 Lava`, `5 Town`, `5 Siege`, `5 Barricade`,
  `5 Temple`, `5 Ice`, `5 Baal`, `5 Lava`.
- B (TileName, 20): `fl`, `wl`, `wr`, `wtlr`, `wtll`, `wtr`, `wbl`, `wbr`,
  `wld`, `wrd`, `wle`, `wre`, `co`, `sh`, `tr`, `rf`, `ld`, `rd`, `fd`,
  `fi`.

X `automap.bin` uses 34 of the names and 14 of the codes.

### 9. Combined index spaces

Some tables are concatenated into one array right after loading. The
combined index is what other data and code use.

| Combined array | Parts, in order | 1.14d sizes |
|---|---|---|
| items | weapons, armor, misc | 306 + 202 + 151 = 659 |
| magic affixes | magicsuffix, magicprefix, automagic | 747 + 669 + 36 = 1,452 |
| rare affixes | raresuffix, rareprefix | 155 + 46 = 201 |
| player tokens | plrtype, plrmode | 7 + 20 = 27 |
| object tokens | objtype, objmode | 573 + 8 = 581 |

Suffixes come before prefixes in both affix arrays.

### 10. Special cases

1. **String tables first.** `string.tbl`, `patchstring.tbl`, then
   `expansionstring.tbl` (only when `d2exp.mpq` exists) load before the
   excel tables. Several loaders turn name keys into string ids during
   load (§7.4).
2. **levels and leveldefs.** Both come from `levels.txt`: `levels` with
   544-byte records, `leveldefs` with 156-byte records (different columns
   of the same rows). Only in `-txt` mode is the name swapped; normal play
   reads `leveldefs.bin`. Its count is not kept: row `i` of leveldefs
   belongs to row `i` of levels (both 137 in 1.14d).
3. **Skill code table.** The compile-only `skills` entry (§7.2) builds the
   skill-name link used by `missiles` and `states`, which load before the
   real `skills` table (step 10). During step 10 the compiler also builds
   temporary name links from `monstats.txt` (column `Id`) and
   `skilldesc.txt` (column `skilldesc`), plus a fixed code link
   (`@skillrange`, `field-types.md` §6.4) with the five codes `none`,
   `h2h`, `rng`, `both`, `loc` (in that order), and frees them after
   step 11.
4. **sounds.** Compile-only for the data tables (name link, 2-byte
   records). The sound system reads `sounds.txt` at runtime (§3.4).
5. **Expansion separator rows** are removed by the reader
   (`txt-format.md` §5) and leave no trace in `.bin` files. X
   `objgroup.txt` has a row starting `EXPANSION`, which stays a record
   (index 97, all zero bytes).
6. **treasureclass vs treasureclassex.** 1.14d loads only
   `treasureclassex`; `treasureclass.txt` (X, D) is never read. The TC
   routine builds the TC list and its name link `@tc` (`field-types.md`
   §6.4) in this order:
   1. TC 0: a placeholder with an empty name.
   2. Automatic TCs: for each itemtypes record with byte 0x1D ≠ 0, in
      record order (1.14d: `bow`, `weap`, `mele`, `armo`, `abow`), 32 TCs
      named code (pad spaces removed) + level, for levels 3, 6, …, 96
      (`bow3`, not `bow 3`); level `L` holds the items with level in
      (L−3, L].
   3. `treasureclassex` rows in order, stopping at the first row whose
      name is empty.

   1.14d: 1 + 160 + 852 = 1,013 TCs. Row `i` of `treasureclassex` is TC
   161 + `i`; row 852 (the last, all cells empty) is not a TC. The routine
   then looks up the `Act N … Chest …` TCs by name and adds none. Item
   lists and picks: treasure-class spec.
7. **Classic vs expansion.** Every table loads in every case; there is no
   classic-only or expansion-only table. Install-dependent processing
   exists only for `expansionstring.tbl` (§10.1), the hireling name check
   (§8), and the DS1 handling inside the `lvlprest`/`lvlsub` loaders, which
   skips rows whose u32 `Expansion` column (lvlprest +0x20, lvlsub +0x158)
   is non-zero when d2exp is absent (details: levels spec, Open question
   9). "d2exp exists" means `d2exp.mpq` is found in the game folder or,
   failing that, relative to the current directory (checked once, then
   cached). Game-type differences come from per-row columns, not from
   loading.
8. **Counts not kept.** `arena`, `composit`, `armtype`, `experience` and
   `leveldefs` discard their count; the code relies on fixed sizes or on
   another table (levels). d2rs checks, besides the file size: `leveldefs`
   count = `levels` count, and the lowest counts the code relies on
   (Open question 12): `arena` ≥ 1 (only row 0 is read: the accessor
   `0x00664AB0` is fatal for a type ≥ 1, error 0x41, and the game's arena
   type +4 is zeroed at creation `0x0053F4D3` and never written);
   `composit` ≥ 16 (the client composite init reads rows 0–15,
   `0x004DA7A4`–`0x004DA7C2`, no bound in the accessor `0x0065B5E0`);
   `armtype` ≥ 3 and > the largest `rArm`, `lArm`, `torso`, `legs`,
   `rspad`, `lspad` byte of any weapons / armor / misc row (items
   +0x116–+0x11B; rows 0–2 read at `0x004DA891`–`0x004DA8AB`, and the
   item-graphics path `0x0063E5BC` indexes it with that byte through
   `0x0064F500`, no bound in the accessor `0x0065B620`; 1.14d: largest
   byte 2); `experience` ≥ max(MaxLvl) + 2 (MaxLvl = row 0's 7 class
   values; Open question 12). Rows past these limits are loaded and never read. 1.14d
   counts: 1, 16, 3, 101.

### 11. Txt vs bin cross-check

Source of each runtime table: the highest-priority `.txt` (P → X → D;
`levels.txt` for `leveldefs`). Its record count (§5) equals the live
`.bin` count for all 73 tables (per-file line and `Expansion` counts:
`txt-format.md` Survey). Five pairs are cross-archive: `automagic`,
`raresuffix`, `rareprefix` (X `.txt` → P `.bin`) and `plrmode`,
`inventory` (P `.txt` → X `.bin`).

**d2rs cross-check** (`cargo run --release -p data-tool -- tables`,
2026-10-05; confirmed by bin cross-check). The live set loads and passes
§4.2, §8 and §10.8 (TC count 1,013; every gamble code is an item; every
hireling, automap, superuniques and chartemplate row passes). Compiling
every list of `tables.tsv` in order from the highest-priority `.txt`
gives, against the live `.bin`:

- 69 of the 73 runtime tables byte-identical, including all five
  cross-archive pairs and `leveldefs` from `levels.txt`.
- `monstats`, `monstats2`, `monpreset`, `cubemain`: identical in every
  byte inside a field footprint. The other differing bytes (12,200 /
  118,780 / 430 / 1,657 in 734 / 609 / 213 / 151 records) lie outside
  every footprint, where the table callbacks write
  (`field-types.md` §8.3); plus `monstats` record 707 `NameStr`
  (`field-types.md` §10).
- The four code buffers byte-identical (196, 5,891, 4,252, 158 bytes).
- `hitclass` (X, §3.5) and the 12 shipped by-products of §7.2
  (`playerclass`, `bodylocs`, `storepage`, `elemtypes`, `colors`,
  `hiredesc`, `monai`, `monplace`, `skillcalc`, `misscalc`, `events`,
  `sounds`) byte-identical.

Older-generation copies (never loaded): X `.bin` files with a record size
different from 1.14d's, shadowed by P: itemtypes 236, montype 52, overlay
128, itemstatcost 36, properties 16, missiles 220, states 16, skills 360,
charstats 164, weapons/armor/misc 592, magic affixes 132, rare affixes 60,
uniqueitems 228, setitems 1,332, books 68, runes 264, itemratio 52,
experience 28, uniquetitle/prefix/suffix/appellation 756, treasureclassex
732, monstats 864, superuniques 216, hireling 332, npc 136, levels 708,
leveldefs 132, lvltypes 1,921, lvlprest 428, lvlmaze 20, lvlsub 344,
objects 464, cubemain 352, difficultylevels 52 (41 tables). X copies with
the 1.14d size, also shadowed: gems, monmode, shrines.

## Constants & data dependencies

- Excel directory: `DATA\GLOBAL\EXCEL`. Extensions: `.bin`, `.txt`
  (`.xls` only in the `runessrv` probe).
- Record sizes, order and limits: §6 and §8.
- Files present but never read by 1.14d in any mode: `cubemod.txt/.bin`,
  `cubetype.txt/.bin`, `weaponclass.txt` (X); `aiparms.txt`,
  `monname.txt`, `treasureclass.txt` (X, D). No code references their
  names.
- Files read only in `-txt` mode: the `.txt` of every table in §6 and §7.2
  (`levels.txt` for `leveldefs`), except `sounds.txt` (§3.4).
- `.bin` files never read in normal play: the §7.2 by-products
  (`playerclass`, `bodylocs`, `storepage`, `elemtypes`, `colors`,
  `hiredesc`, `monai`, `monplace`, `skillcalc`, `misscalc`, `events`,
  `sounds`) and the shadowed X copies (§11).
- Excel files per archive: P 62 `.txt` + 67 `.bin` (found by probing 107
  names; P has no listfile, so files under other names cannot be ruled
  out, but 1.14d reads no other names), X 75 `.txt` + 70 `.bin`, D 56
  `.txt` + 0 `.bin` (X and D from their listfiles, confirmed by the probe).
  None of the 107 names exists in the other 8 archives.

## Randomness

None.

## Edge cases & original bugs

1. 1.14d never checks a `.bin`'s size. A short file makes it read past the
  buffer; a long one is silently accepted. d2rs rejects both (§4.2).
2. A record whose cells are all empty is a real record (P `uniqueitems.txt`
  record 401; `txt-format.md` §5). Its compiled bytes are not all zero
  (empty code cells and empty link cells: `field-types.md` §5.2, §6).
  Observed: bytes 40–43 (`code`) = `20 20 20 20`; 56–57 (`chrtransform`,
  `invtransform` → colors) = `FF FF`; `FF FF FF FF` at 140 + 16k, k = 0–11
  (`prop1`–`prop12` → properties); the linkers of the `FF` fields hold no
  empty key. All other bytes are 0.
3. Original-only `.txt` reader behaviors (unterminated last line, its
  `Expansion` quirk): `txt-format.md` §10.
4. The live data relies on P→X fallback: 17 tables (plus `hitclass`) have
  their only live `.bin` in X, while P carries newer `.txt` for two of them
  (`inventory`, `plrmode`).

## d2-data policy

Decided 2026-10-05 and logged in the `docs/PLAN.md` decisions log.

1. **Game truth is the `.bin` set 1.14d loads**: the 73 record tables of
   §6, the 4 code buffers (§4.3) and `hitclass.bin` (§3.5), resolved
   P → X → D, validated by §4.2, §8 and §10.8, then fixed up (§7.4). Plus
   `sounds.txt` and `soundenviron.txt` parsed as `.txt` (§3.4).
2. Typed records are decoded from `.bin` bytes with explicit
   little-endian reads at per-table offsets from `fields.tsv` /
   `tables.tsv` (`schema.md`).
3. A separate `.txt` compiler serves mod authoring. Its acceptance test:
   compiling each table's highest-priority `.txt` (with the cross-archive
   sources of §11, and `levels.txt` for `leveldefs`) reproduces the live
   `.bin` byte for byte, for all 73 tables and the 4 code buffers, except
   the bytes `field-types.md` §10 lists.
4. Mod patch layers: `patch-layers.md` (they patch `.txt` cells only;
   under `Ruleset::Mod` every table is compiled from text). Never by
   editing or shipping `.bin` files.
5. Not reproduced: `-txt` mode, loose files on disk (`-direct`, §2), the
   dead `.txt` runtime branch, classic installs (`d2exp.mpq` is required, so every
   "d2exp exists" branch takes the expansion path; Open question 6).
   Presence of `runessrv.*` or `cubeserver.*` is reported as an error,
   like 1.14d.
6. Survey tools list exactly the never-read files of Constants as known
   unused; shadowed copies and §7.2 by-products are not reported.

## Test vectors

| Input | Expected output | Source |
|---|---|---|
| P `levels.bin`, 74,532 bytes, size 544 | count 137; 4 + 137 × 544 = 74,532: valid | 1.14d install |
| P `leveldefs.bin`, 21,376 bytes, size 156 | count 137; valid | 1.14d install |
| X `inventory.bin`, 7,684 bytes, size 240 | count 32; valid; inventory check passes | 1.14d install |
| X `armor.bin`, 119,588 bytes, count 202, size 424 | 4 + 202 × 424 = 85,652 ≠ 119,588: reject (it is the 592-byte layout) | 1.14d install |
| P `compcode.bin` | count 115, size 4; record 0 = `6E 69 6C 20` (`nil `) | 1.14d install |
| P `sounds.bin` | count 4,699, size 2; record `i` = `i` (u16) | 1.14d install |
| lookup `data\global\excel\inventory.bin` | P: absent; X: found → X | archive probe |
| lookup `data\global\excel\inventory.txt` | P: found (8,843 bytes) | archive probe |
| lookup `data\global\excel\leveldefs.txt` | absent in P, X, D | archive probe |
| lookup `data\global\excel\armor.bin` | P (85,652 bytes), although X also has one (119,588) | §2 |
| P `itemtypes.txt` | 104 data lines, 1 `Expansion` → 103 records = `itemtypes.bin` count | §5 |
| X `objgroup.txt` | 133 data lines, `EXPANSION` row kept → 133 records = `objgroup.bin` count; record 97 all zero | §5 |
| `"a\tb\r\n1\t2\r\nExpansion\r\n3\t4"` | E6, line 4 (1.14d: 1 record (`1`,`2`); `3\t4` dropped) | `txt-format.md` §9 |
| `"a\tb\r\n\t\r\n"` | 1 record, all cells empty | §5 |
| `"a\tb\r\n"` | E2 (1.14d: the parser returns nothing) | `txt-format.md` §9 |
| `"a\tb\r1\t2\r\n"` | E4, line 1 (1.14d: fatal 92) | `txt-format.md` §9 |
| header with 281 columns | E7 (1.14d: fatal 103); 280 columns accepted | `txt-format.md` §9 |
| a 3-byte file / a 4-byte file with count 0 | load error / valid size (§8, §10.8 still apply) | §4.2 |
| `inventory` file of 4 + 31 × 240 bytes, count 31 (size valid); same for 33 | §8 load error | §8 |
| `belts` count 15 / 16 (size valid) | 15 passes (15 ÷ 2 = 7); 16 fails | §8 |
| `difficultylevels` count 4 (size valid) | load error | §8 |
| count limits (size valid): `pettype` 255 / 256, `itemstatcost` 511 / 512, `levels` 1,023 / 1,024, `skills` 32,766 / 32,767 | valid / load error | §8 |
| combined indices | items 306 = armor 0, 508 = misc 0; magic affixes 747 = magicprefix 0, 1,416 = automagic 0; rare affixes 155 = rareprefix 0; player tokens 7 = plrmode 0; object tokens 573 = objmode 0 | §9 |
| `automap` row with LevelName `6 Town` | load error | §8 |
| `chartemplate` rows with Level 9, then 3 | load error | §8 |
| `experience` count 100, or `leveldefs` count ≠ `levels` count | load error | §10.8 |
| `arena` 0 / 2, `composit` 15 / 17, `armtype` 2 / 4 records (size valid) | load error / valid | §10.8 |
| `armtype` 3 records and an armor row with `torso` 3 | load error | §10.8 |
| `experience` 102 records (row 0 MaxLvl 99 for all classes) / 101 with one class MaxLvl 100 | valid / load error | §10.8 |
| P `monstats.bin` record 0 `TreasureClass1` (u16 +0x86) | 430 = 161 + 269 (`treasureclassex` row 269 `Act 1 H2H A`) | §10.6 |
| P `uniqueitems.bin` record 5, after fix-up | u16 +0x00 = 5 (file: 0) | `fixups.md` §6 |
| an archive set (or test lookup stub) in which `runessrv.txt` resolves | load error before `runes` | §3.3 |

## Provenance

All 1.14d facts come from the Ghidra exports of `Game.exe` (1.14.3.71) and
from reading its bytes (PE sections, string table, call targets), plus
the 1.14d data files. Addresses are virtual addresses in `Game.exe`.

- **Table loader** `0x006122F0` (the 1.14d form of D2MOO's
  `DATATBLS_CompileTxt`), path builder `0x00612280`, `.bin` writer
  `0x006121F0`. Its 92 call sites were enumerated by scanning `.text` for
  direct calls; each passes the table name and record size used in §6.
  Name strings at `0x006DCC70` (`misc`), `0x006E9E34` (`sets`),
  `0x006EA170` (`gems`), `0x006EC678` (`npc`) were read from `.rdata`.
- **Switches:** load-from-bin at `0x00744308` = 1 in `.data`; all 30 code
  references are compares (no writes). Compile switch at `0x0096C8B4`
  (`.bss`, 0) is written only by `0x006125A0`, called once from the config
  callback `0x0044D9C0`, which passes (byte `+0x215` == 0) to it; it
  stores (argument == 0), so compile is on exactly when config byte
  `+0x215` is non-zero. The command option record at
  `0x007063A4`–`0x007063FF` (names `TXT`, `TXT`, `txt`; target offset
  0x215 in the dword at `0x007063FC`) sets that byte. The `.bin` writer
  skips the write when `fopen(…, "wb")` fails.
- **Load-all routine** `0x00619300` (single caller `0x0044B8A0`, after the
  string-table loader `0x005259C0`) gives the order in §6, one load
  routine per step except 10–11, 15–17, 18–20, 21–22, 24–25, 33–34, 36–37
  and the monster group 41–57 (`0x0065A400`, with the TC group
  `0x0065A390` before 46); lookup group `0x00612750` (step 1); it frees the
  compile-only `sounds` link at the end. D2MOO 1.10f
  `DATATBLS_LoadAllTxts` has the same sequence; the record sizes and the
  post-load checks were taken from 1.14d.
- **Code buffers:** `0x00613E90` with names `skillscode`,
  `skilldesccode`, `itemscode`, `misscode` read from `.rdata` at its four
  call sites.
- **Txt parser:** `0x006BD640` (header/rows/`Expansion`, column limit
  0x119), case-sensitive compare `0x00413550` (strncmp, length 10).
- **Runtime txt:** sound header loader `0x00481950` (`sounds.txt` 0x8E,
  `soundenviron.txt` 0x58 bytes per record).
- **Client composite loader** `0x00504430` with switch `0x0072EF2C` = 1
  (compares only), callers `0x00506000`/`0x00504570`, and `0x00504920`
  (`itemtypes`).
- **Server-file checks:** `runessrv` probe `0x00639420` (extensions `.txt`,
  `.bin`, `.xls`); `cubeserver` in `0x00669130`. Fatal path:
  `0x00408A60` ("Unrecoverable internal error %08x") then exit. D2MOO
  only warns here; 1.14d aborts.
- **Post-load checks (§8):** the compare-and-abort blocks in each loader
  (pettype `0x00618D90`, itemstatcost `0x00637A00`, states `0x00618100`,
  uniqueitems `0x006342B0`, sets `0x00634DC0`, gamble `0x00638AE0`,
  monstats `0x00651210`, superuniques `0x006552E0`, hireling `0x00655720`,
  monequip `0x00659E60`, levels `0x0061C540`, automap `0x0061FCF0` with
  name lists at `0x006E7D50` (16-byte entries, via `0x0061FC10`) and
  `0x006E7F90` (8-byte, via `0x0061FC80`), inventory `0x0065B660`, belts
  `0x00660290`, difficultylevels `0x00617D90`, chartemplate `0x00664B10`,
  TC total `0x0065A390`).
- **TC routine (§10.6):** `0x006541C0` (empty-name TC, then the automatic
  TCs, format `%s%d`), `0x006547D0` (row loop stops at an empty name),
  `0x0065A2C0` (chest TC lookups only), TC allocator `0x00653F90`
  (add-always into the TC link). Data: P `monstats.bin` TC links equal
  161 + row for `treasureclassex` names.
- **Fix-ups (§7.4):** loader addresses and evidence in `fixups.md` and
  `runtime-maps.md` Provenance. String ids through `0x00524D30`, the
  `strkey` lookup. Confirmed by post-load dump (`dump_tables.py`,
  2026-10-06): every row of the §7.4 table, and the d2rs paragraph's
  monequip finding (`data-tool dump-compare`).
- **Dependencies (§7.3):** extracted from the field tables in each 1.14d
  loader: each link is created right before its owner table loads, and
  other loaders' field entries point at it. Lookup key columns and kinds
  (§7.2) from `0x00612750` (type 10 = code, 0x11 = name).
- **Expansion installed:** `0x00408F20`: file-attribute check of
  `d2exp.mpq` in the game folder, then of the bare name; the result is
  cached. Used by the string-table, archive, lvlprest, lvlsub and hireling
  code; the d2x archives also call the configuration callback +0x211
  (`0x004FAEC0`). Expansion columns from the field lists (`fields.tsv`):
  lvlprest 0x20, lvlsub 0x158, lvltypes 0x784.
- **Combined arrays (§9):** the copy loops after loading in `0x006315D0`,
  `0x00633730`, `0x00633F40`, `0x0065ADC0`, `0x0065B1B0`.
- **Archive priorities (§2):** `0x004FAB90`, `0x004FAEC0`, `0x004FAD10`
  pass the priority to the archive opener; name strings at
  `0x006DC728`–`0x006DC7B4` and `0x006CD3A0`. Search order from the Storm
  list insert `0x0041B6B0` (walks from the head, inserts before the first
  archive whose priority is ≤ the new one) and the lookup `0x00417F10`
  (walks from the head, first hit wins).
- **Data:** a name-hash probe (scratch script implementing `mpq.md` §3/§5)
  over all 11 archives for 107 candidate names (all excel names in the X
  and D listfiles, every name the 1.14d loaders use, the code buffers,
  server files, and leftovers) × `.txt`/`.bin`/`.xls`. P files were
  extracted by name; file sizes, counts, `.txt` record counts (§5) and
  field comparisons (§11) were computed from them.
- D2MOO (MIT, 1.10f) was used to name functions and fields and to find the
  loader; every rule here was re-derived from the 1.14d binary or data.

## Open questions

1. Search order among equal-priority archives (newest first) is derived
   from the list-insert code only, and the call timing of the second group
   and the video path was not traced. It does not affect excel files. For
   other files §2 contradicts `mpq.md`'s provisional order (e.g. `d2data`
   last here, 6th there); `mpq.md` open question 1 should take §2 as
   input.
2. Answered (static, 1.14d `0x004192F0`, `0x00417F10`, `0x00418B50`,
   `0x00415F70`): loose files are read only with `-direct` (§2 "Loose
   files"); d2rs ignores loose files, as 1.14d without `-direct`.
3. Answered (follows from 2): the table loader `0x006122F0` and the
   code-file loader `0x00613E90` read the `.bin` through `0x00612280` →
   `0x00517079`, the archive open. In `-txt` mode without `-direct` the
   MPQ copy is read back (the `.bin` just written to
   `DATA\GLOBAL\EXCEL` on disk is ignored); with `-direct` the disk
   `.bin` wins.
4. Answered: the field compiler rules are `txt-format.md` §6–§8 and
   `field-types.md`.
5. Answered: byte-exact equality of every live `.bin` with its `.txt`
   holds except the table-specific callback bytes (`field-types.md` §8.3)
   and `monstats` record 707 `NameStr` (`field-types.md` §10); confirmed
   by bin cross-check (§11, d2rs cross-check).
6. How a 1.14d install without `d2exp.mpq` would load the 18 `.bin` files
   that exist only in X. Out of scope (d2rs requires LoD).
7. Answered: a missing code file leaves the buffer null and evaluation
   gives 0 (§4.3, `calc-expressions.md` §3.1). d2rs requires the files.
8. When the client composite loader (§3.5) and the sound loader (§3.4)
   first run relative to game start (both after the excel load).
9. The DS1 handling inside the `lvlprest`/`lvlsub` loaders (which rows,
   and the `lvlsub` abort 0x3C8 tied to it) was not analyzed. It is gated
   by the load-all routine's inputs, which its only caller sets to
   input 2 = 0 and input 3 = (config byte +0x19 == 6). Input 2 also gates
   a `lvltypes` tile step between steps 60 and 61 (`0x0061FAE0`), which
   therefore never runs. *Partly answered* (static): the `lvlprest`
   DS1 preload never runs either: `0x0061EBB0` receives input 2 in EDX
   (call `0x0061941D`) and calls the DS1 loader `0x00665F40` only when it
   is non-zero (`0x0061EFAD`; `drlg/preset.md` OQ 6). The `lvlsub` loader
   (`0x0061F130`) takes no such input; its abort 0x3C8 is still open.
   *Answered* (static, 1.14d asm; correction: `0x0061F130` is the
   `lvlwarp` loader, the `lvlsub` loader is `0x0061F500`, call
   `0x00619438`): the `lvlsub` loader gets input 2 in EDX and input 3
   on the stack. Only when one of them is non-zero does it allocate a
   slot array (one pointer per row) and, for each row in order whose
   `Expansion` (+0x158) is 0 or with d2exp present (`0x00408F20`),
   load the row's DS1 (`0x00665F40`, `drlg/preset.md` §5) and abort
   with 0x3C8 when that DS1 record's group count (+0x4C) is 0. Input 2
   is always 0 and input 3 is 1 only when config +0x19 (`-gametype`,
   `tools/original-hooks.md` §5.1) is 6, so in single player (game type
   0) no `lvlsub` DS1 is preloaded and the abort cannot occur; d2rs
   omits the preload.
10. `0x00653DB0`, a generic 2-byte-record loader, has no callers (dead
    code); not part of the load.
11. Record layouts of the sound tables (142 and 88 bytes). Answered for
    the rest: `AnimData.d2` and `expfield.d2` are `formats/animdata.md`.
12. The index limits the code relies on for `arena`, `composit`,
    `armtype` and `experience` (e.g. the highest level read from
    `experience`); until known, d2rs requires the 1.14d counts (§10.8).
    *Partly answered* (static, 1.14d) for `experience`: no reader checks
    the row count (table `[0x0096C8A8]`; readers `0x00611800`,
    `0x00611830`, `0x00611860`, `combat/vitals.md` §4.1).
    `level_from_exp` reads row i + 1 before its `i < MaxLvl` test
    (`0x0061189F` then `0x00611895`), so it reads up to row MaxLvl + 1:
    at least max(MaxLvl) + 2 rows (101 in 1.14d, MaxLvl 99). `threshold`
    reads row L + 1 with no bound; its 13 callers pass the unit's level,
    level − 1 (`0x00535A24`), level + 1 (`0x0058C62D`), MaxLvl − 1
    (`0x0057E53B`) or constants. Whether the level + 1 caller can run at
    level MaxLvl (row MaxLvl + 2, past 1.14d's 101 rows) is not traced;
    `arena`, `composit`, `armtype` still open.
    *Answered* (static, 2026-10-08): the level + 1 caller (`0x0058C5C0`)
    first compares the unit's level (stat 12, `0x00625480`) with its
    class's MaxLvl (`0x00611830`) and skips the call when level ≥ MaxLvl
    (signed, `0x0058C61E` / `0x0058C624`), so it reads at most row
    MaxLvl + 1 (`threshold` row index L + 1, `0x00611814`). The highest
    row is MaxLvl + 1 for both readers: `experience` needs
    max(MaxLvl) + 2 rows. Limits of `arena`, `composit`, `armtype`:
    §10.8.
13. Answered: the §7.4 algorithms are `fixups.md` and `runtime-maps.md`,
    which also cover the further maps found in the loaders (states,
    montype, monseq, monpreset, hireling, leveldefs, lvlsub, items; a
    scan of every loader's code after its load call).
    Still open there: the treasure-class runtime form (`runtime-maps.md`
    Open question 3) and the readers of several maps.
14. Answered: the special first byte is the character `0` (0x30); an
    empty name is fatal like any unknown name (§8; `0x0061FC13`).
15. Answered: a dump of the tables in 1.14d memory after the load
    (`tools/trace-recorder/dump_tables.py`, compared by `data-tool
    dump-compare`) confirms every §7.4 rule byte for byte (70 tables,
    27 maps). It found d2rs's monequip rule wrong (§7.4, d2rs paragraph)
    and four fix-ups the old §7.4 table missed (lvltypes, lvlprest,
    lvlsub tile paths; objects names and frame counts).
