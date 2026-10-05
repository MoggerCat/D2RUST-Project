# Spec: Formats — String tables (.tbl)

- **Status:** implemented. All 33 string tables in 1.14d (11 languages)
  parse, every key resolves to its own slot, all keys are ASCII, and every
  version byte is 1 (`mpq-tool formats`).
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
- Strings are raw 8-bit text. English 1.14d uses Windows-1252. Byte 0xFF
  followed by `c` and a character is an in-game color code. Text is kept as
  bytes; decoding to Unicode is a presentation step.

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

## Provenance

Community documentation of the TBL format (Phrozen Keep), cross-checked
against Riiablo (Apache-2.0) `codec/StringTBL.java` (header layout, 17-byte
entries, hash function, probing). No Blizzard code or decompiler output was
consulted.

## Open questions

1. Behavior of the original key hash for bytes ≥ 0x80 (signed char?). Keys
   in 1.14d are checked to be ASCII in the survey, so this doesn't matter
   for the original data.
2. Does the game read the plain-text `DEFAULT.TBL` / `FONTER.TBL` at all?
   Check in an RE session. Until then they're treated as unused tool
   leftovers.
