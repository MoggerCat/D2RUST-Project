# Spec: Formats — String tables (.tbl)

- **Status:** implemented. All 29 string-table copies in 1.14d (20
  distinct paths, 10 language folders) parse, all keys are ASCII, every
  version byte is 1 and every non-ASCII value is valid UTF-8 (2026-10-06
  survey, §Live tables). Every key resolving to its own slot was checked
  by the older `mpq-tool formats` run.
- **Target version:** 1.14d
- **Crate/module:** `d2-formats::tbl`

## Summary

`.tbl` files map string keys (e.g. `"WarrivAct1Intro"`) and string numbers
to localized text. The game loads three tables, and their numbers combine
into one ID space: `string.tbl` (IDs 0–9,999), `patchstring.tbl` (IDs from
10,000) and `expansionstring.tbl` (IDs from 20,000). Combining tables is a
data-layer concern; this spec covers one file.

## Inputs

| Name | Type | Source |
|---|---|---|
| file bytes | `.tbl` | MPQ, `data\local\lng\<lang>\*.tbl` |

## Outputs / state changes

- For each element number `i` in `0..num_elements`: key bytes and value
  bytes, or "empty".
- Lookup of a value by key.

## Rules

All integers are little-endian.

### Header (21 bytes)

| Offset | Size | Field |
|---|---|---|
| 0x00 | u16 | crc (not verified) |
| 0x02 | u16 | num_elements |
| 0x04 | u32 | hash_table_size |
| 0x08 | u8 | version (1 for 1.14d; not checked, recorded) |
| 0x09 | u32 | data_start: file offset where string data begins |
| 0x0D | u32 | max_tries: probe limit for key lookup |
| 0x11 | u32 | file_size: should equal the file length |

Then `num_elements` u16 **indices** (element number → hash slot), then
`hash_table_size` **hash entries** of 17 bytes:

| Offset | Size | Field |
|---|---|---|
| 0 | u8 | used (0 = empty slot) |
| 1 | u16 | index (element number) |
| 3 | u32 | hash value |
| 7 | u32 | key_offset (absolute file offset) |
| 11 | u32 | value_offset (absolute file offset) |
| 15 | u16 | value_length, **including** the terminating NUL |

The headers must fit in the file. Every index must be < hash_table_size.

### Strings

- **Key** of a used entry: the bytes from `key_offset` up to (not
  including) the first NUL.
- **Value**: `value_length − 1` bytes at `value_offset` (the final byte is a
  NUL). A `value_length` of 0 means an empty value.
- Offsets and lengths must stay inside the file.
- The keys and values of all used entries together (without NULs) must not
  exceed the file length: strings that don't overlap never do, and entries
  sharing one string could otherwise copy it once per entry (quadratic in
  the file size; see Open questions).
- Strings are raw 8-bit text, kept as bytes here. 1.14d decodes them as
  UTF-8 when the tables load (`ui/text.md` §2; the color-code lead `ÿ` is
  stored as `C3 BF`); color codes are `ui/text.md` §5. The live data
  agrees (§Live tables): of 63,167 used entries, 16,786 values hold a
  byte ≥ 0x80 and **all** of them decode as strict UTF-8; none holds a
  raw `FF` byte; 130 hold `C3 BF`. Read as Windows-1252 instead, all
  16,786 decode to different text and 7,553 contain a byte 1252 leaves
  undefined (CHI / JPN / KOR / ESP / ITA / POL tables), so a 1252 reader
  is wrong for the 1.14d files.

### Live tables (1.14d)

Language folders `data\local\lng\<lang>` (listfile spelling upper case):
CHI, DEU, ENG, ESP, FRA, ITA, JPN, KOR, POL, POR, plus the subfolder
ENG\BETA. There is no 11th language: all 17,576 three-letter folder
codes were probed for the three table names in every 1.14d MPQ, and only
these ten hit (`d2data`, `d2exp`, `Patch_D2`; none elsewhere).

| Archive | Tables (entries) |
|---|---|
| `d2data.mpq` | ENG `string` (5,391); ENG\BETA `string` (5,390) |
| `d2exp.mpq` | ENG `patchstring` (869), `expansionstring` (2,818); `string` (5,322: DEU, FRA; 5,391: ESP, ITA, CHI, JPN, KOR) and `patchstring` (59) for DEU, FRA, ESP, ITA, CHI, JPN, KOR; `patchstring` only for POL (59) and POR (17) |
| `Patch_D2.mpq` (no listfile) | `patchstring` (1,179) for ENG, DEU, FRA, ESP, ITA, POL, CHI, JPN, KOR (not POR) |

That is 2 + 18 + 9 = 29 copies of 20 distinct paths. In every one the
used-entry count equals `num_elements` and the header file size equals
the file length. The older counts (33 tables, 11 languages) came from the
case-sensitive `mpq-tool formats` name set: the tool's three lower-case
`eng\…` extra names were counted again where they exist (`string` in
`d2data`, `patchstring` in `d2exp` and `Patch_D2`, `expansionstring` in
`d2exp`: +4) and `eng` became a second folder (+1). POL and POR have no
`string.tbl` and POR no 1.14d `patchstring.tbl` in `Patch_D2`.

### Element access

Element `i` → slot `indices[i]`. If that slot's `used == 0`, the element is
empty.

### Key lookup

```
h = 0
for each key byte c:            # bytes as unsigned
    h = (h << 4) + c            # u32
    t = h & 0xF0000000
    if t != 0:
        h = (h & 0x0FFFFFFF) ^ (t >> 24)
slot = h mod hash_table_size
```

Probe `slot, slot+1, …` (wrapping), at most `max_tries` slots. An unused
slot means not found. A used slot whose key equals the requested key (exact,
case-sensitive byte comparison) is the answer.

## Constants & data dependencies

ID offsets for combining tables: 0 / 10,000 / 20,000 (data layer).

## Randomness

None.

## Edge cases & original bugs

- The `crc` algorithm is unknown and the field is not verified.
- Not every `.tbl` is a string table. Font tables (`"Woo!"` magic, see
  `font-tbl.md`) share the extension. `data\local\font\latin\DEFAULT.TBL`
  (an ISO 8859-1 → Unicode mapping) and `FONTER.TBL` (a code list) are plain
  text. Tools classify a `.tbl` by content: `"Woo!"` → font table, all
  printable text → text file, otherwise → string table.

## Test vectors

| Input / seed | Expected output | Source |
|---|---|---|
| hash of `""` | 0 | §Key lookup |
| hash of `"A"` (0x41), table size 1000 | 65 | §Key lookup |
| hash of `"AB"`, table size 1000 | (0x41 << 4) + 0x42 = 1106 → 106 | §Key lookup |
| synthetic table built by the test | element and key lookups round-trip | §Rules |
| `string.tbl`, `expansionstring.tbl`, `patchstring.tbl` (1.14d, eng) | every element's key resolves back to its own slot by key lookup (unless a duplicate key comes earlier in the probe sequence) | survey |
| every `data\local\lng\*\*.tbl` in `d2data`, `d2exp`, `Patch_D2` (case-insensitive names) | 29 copies, 10 language folders + ENG\BETA, version 1, 63,167 used entries, 16,786 non-ASCII values all valid UTF-8, 0 raw `FF` | §Live tables, 2026-10-06 |

## Provenance

Community documentation of the TBL format (Phrozen Keep), cross-checked
against Riiablo (Apache-2.0) `codec/StringTBL.java` (header layout, 17-byte
entries, hash function, probing). No Blizzard code or decompiler output was
consulted. §Live tables and the UTF-8 counts: a Python read of the 1.14d
archives (2026-10-06): names probed case-insensitively by MPQ hash in
`d2data.mpq`, `d2exp.mpq` and `Patch_D2.mpq` (which has no listfile),
every used entry's value decoded as strict UTF-8 and as Windows-1252.

## Open questions

1. Behavior of the original key hash for bytes ≥ 0x80 (signed char?). Keys
   in 1.14d are checked to be ASCII in the survey, so this doesn't matter
   for the original data.
2. Does the game read the plain-text `DEFAULT.TBL` / `FONTER.TBL` at all?
   Check in an RE session. Until then they're treated as unused tool
   leftovers.
3. String-size limit (§Strings) assumes entries never share key or value
   bytes: confirm on all 33 1.14d tables (`mpq-tool formats`).
