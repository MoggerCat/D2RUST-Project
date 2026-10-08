# Spec: Formats — DS1 (map presets)

- **Status:** implemented. All 2,372 `.ds1` files in 1.14d parse
  (`mpq-tool formats`, names compared case-insensitively, measured
  2026-10-08; the earlier 2,456 counted names case-sensitively, so a file
  under two spellings counted twice). Versions seen: 3, 8, 12, 13,
  15, 16, 17, 18 (1,926 at v18).
- **Target version:** 1.14d
- **Crate/module:** `d2-formats::ds1`
- **Related specs:** `specs/formats/dt1.md`

## Summary

A DS1 is a preset map piece: a grid of tile cells in several layers (walls
with orientations, floors, shadow, substitution tags), the DT1 files it
uses, object/monster spawn points, substitution groups and NPC paths. The
format is versioned (1–18), and fields appear depending on the version.

## Inputs

| Name | Type | Source |
|---|---|---|
| file bytes | `.ds1` | MPQ, `data\global\tiles\...` |

## Outputs / state changes

All header fields, layer grids (raw u32 cells), objects, groups and paths.

## Rules

All integers are little-endian i32/u32. `v` is the version. Fields are read
in this order:

1. `version` (must be 1..=18).
2. `width_minus_1`, `height_minus_1`. Grid size is `W = width_minus_1 + 1`
   by `H = height_minus_1 + 1`.
3. If `v ≥ 8`: `act` (0-based). Else act = 0.
4. If `v ≥ 10`: `tag_type` (substitution method). Else 0.
5. If `v ≥ 3`: `file_count`, then that many NUL-terminated strings (tile
   file paths).
6. If `9 ≤ v ≤ 13`: 8 unknown bytes (kept).
7. If `v ≥ 4`: `wall_count`; then if `v ≥ 16`, `floor_count`, else floor
   count is 1. If `v < 4`: one wall layer and one floor layer.
   Walls must be ≤ 4 and floors ≤ 2.
8. **Layers.** Tags are present if `v < 4` or `tag_type` is 1 or 2.
   Stream order:
   - `v < 4`: wall 0, floor 0, orientation 0, tag, shadow.
   - otherwise: for each wall `i`: wall `i`, orientation `i`; then each
     floor; then shadow; then tag (if present).
   Each layer is `W × H` u32 values, row by row (y outer, x inner).
   Orientation values for `v < 7` are mapped through `ORIENTATION_LOOKUP`
   (a value ≥ 25 is an error); for `v ≥ 7` they are stored directly.
9. **Objects**, if `v ≥ 2`: `object_count`, then per object: `type`, `id`,
   `x`, `y`, and `flags` if `v ≥ 6`.
10. **Groups**, if `v ≥ 12` and `tag_type` is 1 or 2: if `v ≥ 18`, 4
    unknown bytes (kept); then `group_count`, then per group: `x`, `y`,
    `width`, `height`, and `unknown` if `v ≥ 13`. Group records may be cut
    off by the end of the file: each group field is read only if 4 bytes
    remain, otherwise it is 0 (DS1-editor behavior). Implementations may
    reject counts above 65,536.
11. **Paths**, if `v ≥ 14` and bytes remain: `path_count`, then per path:
    `point_count`, `x`, `y`, then `point_count` points of (`x`, `y`, and
    `action` if `v ≥ 15`). `x`, `y` identify the NPC object the path
    belongs to (by position).
12. Trailing bytes after the last section are kept and reported, not an
    error.

Counts must be consistent with the remaining file length (no record may
extend past the end).

### ORIENTATION_LOOKUP (v < 7)

```
[0x00, 0x01, 0x02, 0x01, 0x02, 0x03, 0x03, 0x05, 0x05, 0x06,
 0x06, 0x07, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
 0x0F, 0x10, 0x11, 0x12, 0x14]
```

### Cell interpretation (informational, used by later specs)

For wall, floor, shadow and tag cells, with bytes `b0..b3` of the u32
(little-endian):
- the cell is empty if the whole value is 0 (for walls and floors, see the
  map spec)
- sub index = `b1`
- main index = `(b2 >> 4) | ((b3 & 0x03) << 4)`
- hidden = `b3 & 0x80`

## Constants & data dependencies

`ORIENTATION_LOOKUP` above.

## Randomness

None.

## Edge cases & original bugs

- Riiablo's newer decoder rejects 4 wall layers and 2 floor layers, and
  sizes the shadow layer by the floor count. This spec allows up to 4 walls
  and 2 floors and always has exactly one shadow layer.
- **Truncated groups in live data:** `data\global\tiles\ACT1\OUTDOORS\trees.ds1`
  (v12, tag_type 1, used by `LvlSub.txt` "Trees") declares 14 groups, but
  the file ends 4 bytes into the 14th. Its embedded tile path is a
  developer path (`C:\D2\DATA\GLOBAL\TILES\ACT1\TOWN\trees.tg1`).

## Test vectors

| Input / seed | Expected output | Source |
|---|---|---|
| synthetic v18, 2×2 grid, 1 wall, 1 floor, tag_type 0, no objects/groups/paths | layers: wall, orientation, floor, shadow (no tag) | §Rules |
| synthetic v18, tag_type 1 | tag layer present, groups section read | §Rules |
| synthetic v6, orientation value 7 | stored as 0x05 | §ORIENTATION_LOOKUP |
| wall_count 5 | error | §Rules step 7 |
| every `.ds1` in the 1.14d archives | parses | survey |

## Provenance

Community DS1 documentation and Paul Siramy's DS1 editor format notes
(version conditions, layer order, orientation lookup), cross-checked
against Riiablo (Apache-2.0) `map5/Ds1Decoder.java`. No Blizzard code or
decompiler output was consulted.

## Open questions

1. Meaning of the unknown bytes (steps 6 and 10).
   *Answered* (static, 1.14d parser `0x00665950`): no meaning in 1.14d.
   The parser skips both without reading them: 8 bytes when 9 ≤ v ≤ 13
   (`0x00665A06`), 4 bytes before the group count when v ≥ 18
   (`0x00665DC3`). d2rs keeps them only for round-tripping.
2. Exact empty-cell rule and flag bits for map building: map spec.
   *Answered* in the owners: what the parser keeps is `drlg/preset.md`
   §5.2; the cell bits, hidden / empty handling and tile choice are
   `drlg/rooms.md` §9.4–§9.5.
3. What the original game uses for the missing fields of a truncated group
   (0, or whatever follows in memory): check in an RE session. It only
   affects `trees.ds1`'s 14th group.
   Partly answered (static): the parser reads past the file end into the
   0x320 bytes of uninitialized slack the loader allocates after the
   buffer (`drlg/preset.md` §5.2, OQ 2); so the value is not 0 by rule.
   `trees.ds1` is v12, so even its whole groups have no variant value
   (`drlg/outdoor-tilesub.md` OQ 4). A memory read settles both (PC 2
   recording list).
   PROVISIONAL: 0 for the missing fields of a truncated group (because d2rs uses 0, `drlg/preset.md` §5.2 OQ 2); settled by REC-35.
4. **Trailing data in v12–13 files.** 54 files (v12/v13) end with 4 zero
   bytes after the last documented section. `ACT1\OUTDOORS\swamp2.ds1` (v13,
   tag_type 1) ends with the u32s `[1, 5, 3, 3, 0, 15, 0, 0, 0, 0, 0, 0]`.
   Hypothesis: these versions already carry an NPC-path count (0, or one
   path cut short in swamp2). It's unconfirmed, so the bytes are kept as
   `trailing`. Check in an RE session against the DS1 loader.
   *Answered* (static): the 1.14d parser reads paths only when v ≥ 14
   (`0x00665E46`) and ignores anything after the last section
   (`drlg/preset.md` §5.2 rules 10–11), so the trailing bytes of v12/v13
   files are never read: no NPC paths for those versions.
