# Spec: Formats — AnimData.d2 (animation length, speed and events by COF name)

- **Status:** verified: the monstats speeds it supplies to the fix-ups
  match 1.14d memory (`data-tool dump-compare`, `fixups_on_live_set`,
  2026-10-06); `d2-formats::animdata` passes the synthetic vectors (the
  real-file vectors are not yet tests). Layout,
  lookup and defaults confirmed against the
  1.14d `Game.exe` loader and lookup code and measured on the 1.14d file
  (exact size, every record in its hash bucket, frames/speed/events equal
  to the matching `.cof` for every non-duplicate record).
- **Target version:** 1.14d
- **Crate/module:** `d2-formats::animdata`
- **Related specs:** `specs/data/loading.md` §1.3 (when it loads), §2
  (archive order); `specs/data/fixups.md` §8 (monstats speeds read it);
  `specs/formats/cof.md` (the files this table caches).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 40–49 |
| Inputs | 50–56 |
| Outputs / state changes | 57–63 |
| Rules | 64–67 |
|   1. Source file | 68–82 |
|   2. Layout | 83–110 |
|   3. Default record | 111–116 |
|   4. Name lookup (`0x0066A8F0`) | 117–139 |
|   5. Record by unit, class and mode (`0x0066A9B0`) | 140–150 |
|   6. Info query (`0x0066AA80`) | 151–164 |
|   7. Speed setter (debug) | 165–170 |
| Constants & data dependencies | 171–181 |
| Randomness | 182–185 |
| Edge cases & original bugs | 186–208 |
| Test vectors | 209–241 |
| expfield.d2 | 242–277 |
| Provenance | 278–303 |
| Open questions | 304–346 |
<!-- /index -->

## Summary

`AnimData.d2` is a precomputed cache of three facts per animation (COF
name such as `SKWL1HS` = token `SK`, mode `WL`, weapon class `1HS`): the
frame count, the animation speed, and one event byte per frame. The game
loads it once during the data-table load and looks records up by name
through a 256-bucket hash. A name that is not in the file gets a fixed
default record. A sibling file, `expfield.d2`, loads in the same load
sequence and is described at the end (§expfield.d2).

## Inputs

| Name | Type | Source |
|---|---|---|
| file bytes | `AnimData.d2` | MPQ, `DATA\GLOBAL\AnimData.d2` |
| query name | up to 8 bytes, NUL-terminated | built by the caller (e.g. `fixups.md` §8 monstats speeds) |

## Outputs / state changes

- A table of 256 buckets, each a list of 160-byte records in file order.
- A default record (frames 2048, speed 256, no events).
- Lookup: name → record or default. Info query: name → frames, speed,
  first event frame, found flag.

## Rules

All integers are little-endian.

### 1. Source file

- Path: `DATA\GLOBAL\AnimData.d2`, built as `DATA\GLOBAL` + `\AnimData.d2`
  (`0x0066A770`). Read whole through the archive layer (`0x00517079`), so
  the `loading.md` §2 search order applies.
- 1.14d copies (name-hash probe of all 11 archives): `d2exp.mpq` (X,
  570,304 bytes, 3,558 records) and `d2data.mpq` (D, 411,104 bytes, 2,563
  records). Priority 3000 beats 1000, so **the X copy is the one loaded**;
  the D copy is a shadowed classic-only version. `patch_d2.mpq` has none.
- Loads once, during the excel load right after `experience`
  (`loading.md` §1.3; call in `0x00619300`). The table pointer is a global
  read back by `0x00611D20`. Freed by `0x0066A830`.
- A missing file is not handled: the bucket walk reads through a null
  buffer. d2rs: missing file is a load error.

### 2. Layout

No header, no trailer: 256 **blocks**, one per bucket 0..255, in order.

| Offset (in block) | Size | Field |
|---|---|---|
| 0 | i32 | `count`: records in this bucket |
| 4 | `count` × 160 | records |

Record (160 bytes):

| Offset | Size | Field |
|---|---|---|
| 0x00 | 8 bytes | name: ASCII, NUL-terminated, rest NUL-padded |
| 0x08 | u32 | frames: animation length in frames |
| 0x0C | u32 | speed (the value `fixups.md` §8 copies into monstats) |
| 0x10 | 144 bytes | event byte for frames 0..143 (0 = none) |

The loader (`0x0066A770`) only walks the blocks: bucket `b` starts where
bucket `b−1` ends (`4 + count × 160` bytes later). It checks nothing:
not the file size, not the counts, not that a name hashes to its
bucket. Trailing bytes after bucket 255 would be ignored.

d2rs validation (stricter; the 1.14d file passes): every `count` ≥ 0,
the 256 blocks end exactly at the end of the file, every name has a NUL
within its 8 bytes. Records are kept as read (no re-bucketing, duplicates
kept in order).

### 3. Default record

Built by the loader in the table header, not read from the file: name
all zero, frames **2048**, speed **256**, all 144 event bytes 0
(`0x0066A770` writes 0x800 / 0x100 into a zeroed record).

### 4. Name lookup (`0x0066A8F0`)

1. **Uppercase in place:** each byte `a`..`z` (0x61–0x7A) becomes
   `A`..`Z`. The test is a signed byte compare, so bytes ≥ 0x80 are left
   alone. The caller's buffer is modified.
2. **Hash:** sum of the name's bytes up to the NUL, as unsigned bytes,
   mod 256. This is the bucket.
3. **Scan** the bucket's records in file order. A bucket whose count is
   ≤ 0 (signed) has no records. For each record:
   - fatal error (code 0xD9) if the query name is longer than 8
     characters; fatal error (code 0xDA) if the record's name is longer
     than 8 characters (its length is measured from offset 0 and can run
     into `frames` when the 8 bytes hold no NUL);
   - **match** when the query's first 8 bytes equal the record's 8 name
     bytes (two 32-bit compares). Bytes after the NUL take part, so the
     query must be NUL-padded to 8 bytes (callers zero their 8-byte buffer
     first) and records must be NUL-padded (all 1.14d records are).
4. The **first** match wins. No match → "not found".

Both fatal checks run only when the bucket is non-empty, once per record
tried. They print "Unrecoverable internal error" and exit (`0x00408A60`,
`0x00681E09`). d2rs reports them as errors.

### 5. Record by unit, class and mode (`0x0066A9B0`)

Composes the COF name of (unit, class, mode, unit type, inventory) into a
zeroed 8-byte buffer with the COF-name composer (`0x0064F5B0`, short form
`token + mode + weapon class`, no path), then does §4. Not found → the
default record (§3). Callers always get a record.

The composer's rules for monsters without a unit (the only case 1.14d
uses while loading) are in `fixups.md` §8 (monstats speeds). Player,
object and with-unit cases: Open question 2.

### 6. Info query (`0x0066AA80`)

Input: a name (uppercased in place by §4). Outputs, if found:
- frames, speed as stored;
- **first event frame** = the smallest `i` with `i < frames`, `i < 144`
  and event byte `i` ≠ 0; if there is none, `min(frames, 144)`;
- result "found" (1).

If not found: frames 2048, speed 256, first event frame 2048 (= the
default frames), result 0.

(D2MOO, 1.10f, returns first event frame 0 when a found record has no
event; 1.14d returns `min(frames, 144)` — confirmed in the 1.14d loop.)

### 7. Speed setter (debug)

`0x0066AA20` composes a unit's name and, if found, overwrites that
record's speed (+0x0C) in the loaded table. Not reachable from normal
play as far as traced; d2rs does not implement it.

## Constants & data dependencies

| Constant | Value |
|---|---|
| buckets | 256 |
| record size | 160 (0xA0) |
| name field | 8 bytes |
| event bytes | 144 (0x90) |
| default frames / speed | 2048 / 256 |
| loader table size | 0x4A4 bytes: buffer pointer, 256 bucket pointers, default record at +0x404 |

## Randomness

None.

## Edge cases & original bugs

1. **Duplicates:** 29 names occur twice in the X file, always in the same
  bucket. 20 pairs are identical. 9 differ: `VMS1HTH`, `VMGHHTH`,
  `MINUHTH`, `VMWLHTH`, `VMNUHTH`, `64A1HTH`, `64NUHTH`, `VMA1HTH`,
  `3DNUHTH`. The game uses the **first** copy. The `.cof` found by the
  lookup of the COF cross-check row matches the second copy for 6
  (`3DNUHTH`, `VMA1HTH`, `VMGHHTH`, `VMNUHTH`, `VMS1HTH`, `VMWLHTH`; e.g.
  `VMS1HTH`: first copy speed 200, `.cof` and second copy 160) and the
  first copy for 3 (`64A1HTH`: speed 208, event[16] = 2, second copy 200,
  event[13] = 2; `64NUHTH`: speed 112, second 256; `MINUHTH`: the
  `objects` COF, 1 frame, second copy 8 frames). `3DNUHTH` has two COFs:
  `monsters` (16 frames, 176) = second copy, `objects` (1 frame, 256) =
  first copy (ignored game test `animdata_edge_cases`, 2026-10-06; the
  earlier "second copy in all 9" was wrong). Reproduce: first copy wins.
2. **Frames above 144:** `42DTHTH` has 200 frames; only 144 event bytes
  exist, so the first-event scan stops at 144.
3. **Speed 0:** 4 records have speed 0 (kept as read).
4. **Names of exactly 8 characters** would need a 9-byte query buffer; the
  1.14d callers have 8. No 1.14d record name is longer than 7.
5. **Signed counts:** a negative count would make the loader step
  backwards; the 1.14d file has none (d2rs rejects them).

## Test vectors

Synthetic (CI-safe):

| Input | Expected | Source |
|---|---|---|
| hash of `""` | 0 | §4 |
| hash of `"A"` | 65 | §4 |
| hash of `"ZZZZZZZ"` | 7 × 90 = 630 → 118 | §4 |
| hash of `"skwl1hs"` | uppercased first → `SKWL1HS` → 13 | §4 |
| file of 256 zero counts (1,024 bytes); look up `"X"` | not found → default (2048, 256); info: 2048, 256, 2048, 0 | §3, §6 |
| record frames 4, events `00 00 02 00 …`; info | first event frame 2 | §6 |
| record frames 3, no events; info | first event frame 3 | §6 |
| 1,023-byte file, or one whose blocks end before the file does | d2rs load error | §2 |

Real 1.14d (`d2exp.mpq` copy; `#[ignore]`, `D2_GAME_DIR`):

| Input | Expected | Source |
|---|---|---|
| whole file | 570,304 = 256 × 4 + 3,558 × 160 bytes; 136 non-empty buckets, 120 empty; largest bucket 67; bucket 0 holds 54 (first u32 = 0x36), first record `L1OPHTH` | measurement |
| every record | name 7 characters, uppercase, NUL-padded; hashes to its own bucket | measurement |
| frames / speed ranges | frames 1–200 (one record > 144); speed 0–512, 41 distinct values | measurement |
| event bytes | non-zero values only 1 (433 bytes), 2 (148), 3 (4); none at index ≥ frames; 10 records with more than one | measurement |
| `SKWL1HS` | bucket 13, 41st record; frames 8, speed 128, no events | measurement |
| `SKA11HS` | bucket 220; frames 16, speed 224, event[10] = 1; first event frame 10 | measurement |
| `AMA1BOW` | bucket 232; frames 14, speed 256, event[6] = 2 | measurement |
| `10A1HTH` | frames 38, speed 256, event[14] = 3, event[17] = 1; first event frame 14 | measurement |
| `42DTHTH` | frames 200, speed 168, no events; first event frame 144 | measurement |
| `VMS1HTH` | first copy: bucket 11, 37th record, frames 17, speed 200, event[10] = 2 | measurement |
| `GOWLHTH` (hash 29) | not found → default 2048 / 256 | measurement |
| D copy (`d2data.mpq`) | 411,104 = 1,024 + 2,563 × 160; 130 empty buckets; 7 duplicate names | measurement |
| COF cross-check | 3,529 of 3,558 names have a `.cof` at `data\global\{monsters,chars,objects}\<first 2 chars>\cof\<name>.cof` (P, X, `d2char`, D); for each, frames = COF frames, speed = COF animation rate, events = COF events (zero-filled to 144), except, for each of the 9 differing duplicates, the copy that does not match (see Duplicates) | scratch probe using `d2-formats::cof` |

## expfield.d2

Loaded in the same sequence, right after `inventory` (`loading.md`
§1.3), by `0x0066A700` (path `DATA\GLOBAL\expfield.d2`, whole-file read)
and parsed by `0x0066A2B0`. Only in `d2data.mpq` (65,546 bytes).

| Offset | Size | Field |
|---|---|---|
| 0x00 | u16 | not read (266 = 0x010A in 1.14d) |
| 0x02 | u32 | rows `H` (256) |
| 0x06 | u32 | columns `W` (256) |
| 0x0A | `W × H` bytes | cells; copied to a new buffer, file buffer freed |

Each cell holds a step direction 0–8 toward the centre cell
(`W/2`, `H/2`) = (128, 128), whose value is 8:

| Value | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
|---|---|---|---|---|---|---|---|---|---|
| dx | 0 | +1 | +1 | +1 | 0 | −1 | −1 | −1 | 0 |
| dy | −1 | −1 | 0 | +1 | +1 | +1 | 0 | −1 | 0 |

(dx table at `0x00749780`, dy at `0x007497A4`, 9 × i32 each.)

Step (`0x0066A5D0`), given an origin (x0, y0) and a point (x, y): read
the cell at row `(y − y0) + H/2`, column `(x − x0) + W/2` — the row
stride is a fixed 256, not `W` — add its dx/dy to the point, and report
whether the cell at the new point is not 8 (i.e. the centre is not yet
reached). A null origin is fatal (code 0xBF). No bounds check: the point
must stay within ±128 of the origin. `0x0066A670` repeats steps until a
caller predicate accepts a point; its callers are path code
(`0x0064DEA0`, `0x0065AA40`), out of scope here.

1.14d data: cell counts 0: 8,128, 1: 8,192, 2: 8,256, 3: 8,256, 4: 8,256,
5: 8,191, 6: 8,128, 7: 8,128, 8: 1 (the centre). Row 128, columns
120–127 hold 2 (step +x) and 129–136 hold 6 (step −x).

## Provenance

1.14d `Game.exe` (Ghidra exports in `re/exports/`, checked against the
raw disassembly where Ghidra lost register arguments):

- `0x0066A770` loader: path format, 0x4A4-byte table, 256-bucket walk
  (`4 + count × 160` per block), default frames 0x800 / speed 0x100 at
  table +0x40C / +0x410. `0x00517079` whole-file archive read.
  `0x00619300` call site (after `experience`), global read by
  `0x00611D20`; `0x0066A830` free.
- `0x0066A8F0` lookup: signed `a`–`z` uppercase, unsigned byte sum
  `& 0xFF`, signed count test, two `strlen` (`0x006C9A38`) checks against
  8 with error codes 0xD9 / 0xDA, 8-byte compare as two dwords, first hit.
- `0x0066A9B0`: zeroed 8-byte name buffer, composer `0x0064F5B0` (short
  form), default record at table +0x404 on a miss.
- `0x0066AA80` info query (bounds `i < 0x90` and `i < frames`; default
  outputs from +0x40C / +0x410). `0x0066AA20` speed setter.
- expfield: `0x0066A700`, `0x0066A2B0`, `0x0066A5D0`, `0x0066A670`, tables
  `0x00749780` / `0x007497A4`.
- Data: name-hash probe of the 11 archives with `d2-formats::mpq`; file
  measurements and the COF cross-check by scratch scripts on the
  extracted X and D copies (2026-10-06).
- D2MOO (MIT, 1.10f) `AnimTbls.cpp` named the fields and functions; every
  rule above was re-derived from 1.14d. Difference found: §6 first event
  frame when no event.

## Open questions

1. Units and meaning of `speed` (how the animation system advances frames
   with it) and of event values 1–3 (`cof.md` names 1 attack, 2 missile,
   3 sound, 4 skill): owned by a future animation spec; check in the
   1.14d animation code.
   *Answered* in the owners: `speed` is the base of the unit's
   animation speed +0x4C, in 1/256 frame per tick (`0x00623F50` scales
   it by the rate stats and states, `sim/units.md` §4.3 / §4.7), and
   the frame count is `frames` · 256 (`0x005533D0`); the event bytes
   drive the schedule of `sim/units.md` §4.2 (1, 2, 4: numbered action
   events; 3: an action event with argument 0; others ignored) and the
   client's +0x4E (`audio/triggers-2.md` §15).
2. The full COF-name composer `0x0064F5B0` / weapon-class resolver
   `0x0064F060` for players, objects and units with an inventory (which
   weapon class, the `gh` mode override table at `0x00745900`–
   `0x0074591F`): answered in `render/unit-composite.md` §2 (table
   contents: its OQ1).
3. Whether any 1.14d code path looks up a name of 8 characters (would
   overflow the 8-byte buffers into the stack cookie).
   *Partly answered* (static + data, 2026-10-08): the short form is
   `sprintf("%s%s%s", T, M, W)` (format `0x006D5514`, call `0x0064F9E9`)
   of three parts that are each the first 3 bytes of a 4-byte code with
   spaces → 0 and a forced 0 4th byte (`0x0064F900`–`0x0064F9A0`), so a
   composed name is at most 9 characters. In `0x0066A9B0` the buffer is
   `[ebp−0xC]`–`[ebp−5]` and the stack cookie is at `[ebp−4]`: a name of
   8 characters writes its NUL into the cookie's low byte (the check
   `0x00681A48` then fails unless that byte was already 0); 9 writes two
   bytes. 1.14d data: every `monstats` Code (734), `PlrType` token (7),
   mode token (`PlrMode` 20, `MonMode` 16, `ObjMode` 8) is 2 characters
   and every `WeaponClass` code 3, so player and monster names are 7
   characters. The one exception is `objects` row 0 (`Dummy`, "test
   data", Token `NU0`), whose names would be 8 characters. Still open:
   whether an object of class 0 ever reaches `0x0066A9B0` (callers
   `0x00620F3B`, `0x00650F48`, `0x00650FAE`).
4. Whether `expfield.d2`'s unread u16 (266) is a version, and what the
   path callers use the step field for.
   *Answered* for the u16 (static): it has no meaning in 1.14d; the
   parser `0x0066A2B0` starts reading at +2 (`0x0066A2BC`) and nothing
   else reads the file buffer, which is freed after the copy. What the
   path callers (`0x0064DEA0`, `0x0065AA40`) use the step field for
   belongs to the pathing owner.
