# Spec: Formats — COF (animation composition)

- **Status:** implemented. All 3,605 live `.cof` files in 1.14d parse (one
  junk leftover excepted, see Edge cases). Every version byte is 20.
- **Target version:** 1.14d
- **Crate/module:** `d2-formats::cof`
- **Related specs:** `specs/formats/dcc.md` (the layer graphics)

## Summary

A COF describes one animation mode of a composite unit (e.g. a Barbarian
walking with a one-hand-swing weapon class). It lists which component layers
are used (head, torso, legs, arms, weapon, shield, ...), per-frame events
(attack, missile, sound, skill), and the draw order of the layers for every
direction and frame.

## Inputs

| Name | Type | Source |
|---|---|---|
| file bytes | `.cof` | MPQ, e.g. `data\global\chars\ba\cof\bawlhth.cof` |

## Outputs / state changes

Header fields, layer records, frame events and the draw-order table.

## Rules

All integers are little-endian.

### Header (28 bytes)

| Offset | Size | Field |
|---|---|---|
| 0 | u8 | layers `L` |
| 1 | u8 | frames per direction `F` |
| 2 | u8 | directions `D` |
| 3 | u8 | version (20 for 1.14d; recorded, not checked) |
| 4 | 4 bytes | unknown |
| 8 | i32 | bounding box x min |
| 12 | i32 | bounding box x max |
| 16 | i32 | bounding box y min |
| 20 | i32 | bounding box y max |
| 24 | u32 | animation rate |

### Layer records (`L` × 9 bytes)

| Offset | Size | Field |
|---|---|---|
| 0 | u8 | component (0–15: HD, TR, LG, RA, LA, RH, LH, SH, S1–S8) |
| 1 | u8 | shadow |
| 2 | u8 | selectable |
| 3 | u8 | override translucency |
| 4 | u8 | new translucency level |
| 5 | 4 bytes | weapon class: 3 ASCII characters + NUL (e.g. `hth\0`) |

A component ≥ 16 is an error.

### Frame events and draw order

The rest of the file is `K` bytes of frame events followed by `D × F × L`
bytes of draw order, where `K = length − 28 − 9L − D·F·L`.
- `K` must be ≥ `F`. The first `F` bytes are the events for frames 0..F−1:
  0 none, 1 attack, 2 missile, 3 sound, 4 skill. Any extra `K − F` bytes are
  padding and are kept as read.
- Draw order: for direction `d`, frame `f`, slot `s` (back to front), the
  byte at `(d·F + f)·L + s` is a component ID (< 16).

## Constants & data dependencies

Component names: HD head, TR torso, LG legs, RA right arm, LA left arm, RH
right hand, LH left hand, SH shield, S1–S8 specials.

## Randomness

None.

## Edge cases & original bugs

- Event padding: Riiablo special-cases a 42-byte, 1-layer, 1-frame,
  1-direction COF with 4 event bytes. The rule above covers it (K = 4,
  F = 1) without hard-coding the size. 1.14d has 3 such files.
- **Unused junk file:** `data\global\chars\am\cof\amblxbow.cof`
  (`d2char.mpq`) is 72 bytes of non-COF data. Its name doesn't follow the
  COF pattern (token + mode + 3-letter weapon class); the real file is
  `amblxbw.cof`, which parses. It's never loaded and is not supported.

- **Game read of the draw order:** 1.14d reads the draw-order rows as if
  the event block were exactly `F` bytes, so the three padded files read
  their order 3 bytes early (`render/unit-composite.md` §3 r6).

## Test vectors

| Input / seed | Expected output | Source |
|---|---|---|
| L=1, F=2, D=1, events `01 00`, order `01 01` | 2 events (attack, none), order [[TR],[TR]] | §Rules |
| L=1, F=1, D=1, 42 bytes total | K = 4, event = byte 0, 3 padding bytes | §Edge cases |
| component 16 in a layer | error | §Layer records |
| every `.cof` in the 1.14d archives | parses | survey |

## Provenance

Community documentation of COF (Phrozen Keep), cross-checked against
Riiablo (Apache-2.0) `file/Cof.java`. No Blizzard code or decompiler output
was consulted.

## Open questions

1. Units of the animation rate, and how events and the bounding box are
   used: animation spec (Phase 3/6).
