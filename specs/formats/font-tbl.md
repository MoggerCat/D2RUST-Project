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
| 6 | u16 | unknown | 0 in all 14 1.14d fonts |
| 8 | u16 | count | number of records the by-code glyph lookup searches (`ui/text.md` §3); 256 in all 14 1.14d fonts |
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
| 8 | u16 | frame: index of the glyph's frame in the font's DC6 (1.14d reads a u16, `ui/text.md` §3; byte 9 is 0 in all 14 1.14d fonts) |
| 10 | u32 | unknown5 |

Unknown fields are kept as read. How glyph width, cell size and frame
offsets combine into text layout belongs to the UI rendering spec.

## Constants & data dependencies

None.

## Randomness

None.

## Edge cases & original bugs

- `frame` is a u16 (corrected 2026-10-06 from the 1.14d reader,
  `ui/text.md` §3). `d2-formats::font` still reads a u8 `frame` plus
  `unknown4`, and its test `frame_is_one_byte` asserts that reading: to
  change. The 1.14d Latin fonts are unaffected (byte 9 is always 0).

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
14-byte records). Header `count` and the u16 `frame` confirmed on 1.14d
`Game.exe` (`0x00501690` reads the count at header byte 8, `0x00501A80`
the frame as a u16 at record byte 8) and on the 14 font files.

## Open questions

1. Meaning of the remaining unknown fields (record `unknown1`, `height`,
   `unknown2`, `unknown3`, `unknown5`; header bytes 6–7): the 1.14d D2Win
   text path reads none of them (`ui/text.md` §3, OQ 7). Layout rules are
   `ui/text.md`.
