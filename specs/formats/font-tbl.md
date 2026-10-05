# Spec: Formats — Font tables (font .tbl)

- **Status:** implemented. All 14 font tables in 1.14d parse, each with 256
  records (`mpq-tool formats`).
- **Target version:** 1.14d
- **Crate/module:** `d2-formats::font`
- **Related specs:** `specs/formats/dc6.md` (glyph images),
  `specs/formats/tbl.md` (string tables, an unrelated format with the same
  extension)

## Summary

Each D2 font is a pair: `data\local\font\<locale>\<name>.dc6` holds the
glyph images, and `<name>.tbl` beside it holds per-character metrics and the
DC6 frame index for each character. Font `.tbl` files start with the magic
`"Woo!"`; string tables don't, which is how the two are told apart.

## Inputs

| Name | Type | Source |
|---|---|---|
| file bytes | font `.tbl` | MPQ, `data\local\font\<locale>\*.tbl` |

## Outputs / state changes

- Header (line height, cell width) and one glyph record per entry.

## Rules

All integers are little-endian.

### Header (12 bytes)

| Offset | Size | Field | Notes |
|---|---|---|---|
| 0 | 4 bytes | magic | `"Woo!"` |
| 4 | u16 | version | must be 1 |
| 6 | 4 bytes | unknown | informational |
| 10 | u8 | height | line height / cell height |
| 11 | u8 | width | cell width |

### Glyph records (14 bytes each)

The remainder of the file is a whole number of records, `(len − 12) / 14`.
A remainder that isn't a multiple of 14 is an error. Latin fonts are
expected to have 256 records, CJK fonts more (see survey).

| Offset | Size | Field |
|---|---|---|
| 0 | u16 | code: character code this record describes |
| 2 | u8 | unknown1 |
| 3 | u8 | width: advance width in pixels |
| 4 | u8 | height |
| 5 | u8 | unknown2 (usually 1) |
| 6 | u16 | unknown3 |
| 8 | u8 | frame: index of the glyph's frame in the font's DC6 |
| 9 | u8 | unknown4 |
| 10 | u32 | unknown5 |

Unknown fields are kept as read. How glyph width, cell size and frame
offsets combine into text layout belongs to the UI rendering spec.

## Constants & data dependencies

None.

## Randomness

None.

## Edge cases & original bugs

- `frame` is a single byte, so a font can address at most 256 DC6 frames
  per file, even if it has more than 256 records. See survey.

## Test vectors

| Input / seed | Expected output | Source |
|---|---|---|
| `"Woo!"`, version 1, 2 records | 2 glyphs with fields as written | §Rules |
| wrong magic | error | §Header |
| 12 + 15 bytes | error (partial record) | §Glyph records |
| every font `.tbl` in the 1.14d archives | parses | survey |

## Provenance

Community documentation of D2 font tables, cross-checked against Riiablo
(Apache-2.0) `codec/FontTBL.java` (`"Woo!"`, version 1, 12-byte header,
14-byte records). No Blizzard code or decompiler output was consulted.

## Open questions

1. Meaning of the unknown fields, and the layout rules: decide in the UI
   rendering spec against the original game.
