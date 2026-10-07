# Spec: DRLG — Wall-remap table (`wall-remap.tsv`)

- **Status:** implemented: `DrlgData::wall_remap` parses this file
  (test `wall_remap_is_the_transcribed_table` holds every test vector);
  every value read from the 1.14d `Game.exe` file image (tables
  `0x006EF620`, `0x006EF578`, dword `0x006EF574`). No check against a
  recording yet (`drlg/rooms.md` open question 8).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::drlg` (`DrlgData::wall_remap`, embedded
  with `include_str!`)
- **Related specs:** `drlg/rooms.md` §9.6 (owner of the merge rules that
  read this table).

## Summary

When a linked wall cell finds an existing record R in a neighbour room
(`drlg/rooms.md` §9.6, `0x0066E740`), the new cell's type and R's type
give the merged type. This file is the machine-readable form of that
lookup; the rules around it (door edges, bit 7, corners, re-choice) are
in `drlg/rooms.md` §9.6.

## Inputs

New cell type t (0..19) and R's type r (0..7 when the table is read).

## Outputs / state changes

The class of t and, for class `table`, the merged type.

## Rules

### 1. Columns

| Column | Meaning |
|---|---|
| `new_type` | t, the type of the cell being linked (0..19; one row each) |
| `class` | `table`: merged = column `r<R type>` when R's type ≤ 7, **stop** when R's type > 7; `keep`: merged = t whatever R's type; `stop`: no merge (the record is left as it is, no flag rules) |
| `r0`..`r7` | merged type for R's type 0..7 (class `table` only; empty otherwise) |

### 2. Source of each value

1. Class: the index table `0x006EF620` by t (16 dwords, then 4 more
   dwords at `0x006EF660` that are −1): index ≥ 0 → `table` (row =
   index), −1 → `keep`, −2 → `stop`. Rows: t 1, 2, 3, 5, 6, 7 → index
   0..5.
2. `r1`..`r7`: the 6 × 7 table `0x006EF578`, row = index, column = R's
   type − 1 (values equal D2MOO's `nWallTileTypeRemap`).
3. `r0`: the code indexes the table with R's type 0 as column −1, which
   reads the dword before the row: the previous row's `r7` (rows 1..5)
   or the dword `0x006EF574` = 0 (row 0). R of type 0 in a non-floor
   link list is not known to occur; the values are listed so the read
   is reproduced.
4. Column `r4` is never read: the find step skips type-4 records.

## Constants & data dependencies

`wall-remap.tsv`: header plus 20 rows, 10 columns.

## Edge cases & original bugs

1. Types 8, 9 (doors) are `stop` except on this room's top or left
   edge, where the door keeps its type before the table is consulted
   (`drlg/rooms.md` §9.6).
2. A `table` type over an R of type > 7 stops; a `keep` type over the
   same R replaces R's type.

## Test vectors

| Input | Expected | Source |
|---|---|---|
| t 1, R 5 | 1 | `0x006EF578` row 0, column 4 |
| t 5, R 6 | 6 | row 3, column 5 |
| t 3, any R 1..7 except 4 | 3 | row 2 |
| t 2, R 0 | 1 (row 0's r7) | §2 rule 3 |
| t 0 or 10 | keep | `0x006EF620` |
| t 13 | stop | `0x006EF620` |

## Provenance

- **1.14d `Game.exe`**: `0x0066E740` (index read at `0x0066E747`, table
  read at `0x0066E7D3` with base `0x006EF574`, column = R's type);
  tables read from the file image with `pefile`.
- **D2MOO** (1.10f) `DrlgRoomTile.cpp`: same values.

## Open questions

1. Can a non-floor link list hold a type-0 record (column `r0`)? A dump
   of the link lists of every built room of the five acts would settle
   it.
