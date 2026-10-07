# Spec: Formats — MPQ archives

- **Status:** implemented. Every block of every 1.14d archive decodes
  (`mpq-tool check`).
- **Target version:** 1.14d
- **Crate/module:** `d2-formats::mpq`
- **Related specs:** `specs/formats/mpq-tables.md` (constant tables)

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 40–47 |
| Inputs | 48–54 |
| Outputs / state changes | 55–60 |
| Rules | 61–65 |
|   1. Archive header | 66–90 |
|   2. Crypt table | 91–107 |
|   3. String hash | 108–127 |
|   4. Decryption | 128–148 |
|   5. Hash table | 149–173 |
|   6. Block table | 174–202 |
|   7. File key | 203–208 |
|   8. Reading file data | 209–239 |
|   9. Sector decompression | 240–258 |
|   10. PKWARE Data Compression Library ("implode") stream | 259–295 |
|   11. Huffman (Storm adaptive Huffman) | 296–354 |
|   12. IMA ADPCM (Storm variant) | 355–381 |
|   13. Recovering the key of an unnamed file | 382–399 |
|   14. Listfile | 400–405 |
| Constants & data dependencies | 406–410 |
| Randomness | 411–414 |
| Edge cases & original bugs | 415–424 |
| Archive set (D2-specific) | 425–431 |
| Observations (1.14d install) | 432–463 |
| Test vectors | 464–477 |
| Provenance | 478–495 |
| Open questions | 496–509 |
<!-- /index -->

## Summary

D2 stores all of its data (tables, graphics, sound, video) in MPQ archives
(MoPaQ, Blizzard's archive format). This spec covers reading format version
0 archives as used by D2: locating the header, decrypting the hash and block
tables, looking up files by name, and decrypting and decompressing file
data. Writing archives is out of scope.

## Inputs

| Name | Type | Source |
|---|---|---|
| archive | file on disk | user's install (`game/*.mpq`) |
| file name | ASCII string, `\` or `/` separators | caller |

## Outputs / state changes

- An opened archive: header fields, decrypted hash table and block table.
- For a file: its decompressed bytes, exactly `file_size` bytes long.
- No state is changed. Archives are read-only.

## Rules

All multi-byte integers are little-endian. "dword" means a u32. All
arithmetic on dwords wraps modulo 2^32.

### 1. Archive header

The archive starts at the first offset `A` (a multiple of 0x200, starting at
0) where the 4 bytes are `4D 50 51 1A` ("MPQ\x1A"). If the 4 bytes at a
candidate offset are `4D 50 51 1B` (a user-data header), read the dword at
`+8` (header offset) and use `A = candidate + header_offset`. D2 archives
have the header at offset 0.

Header (32 bytes at `A`):

| Offset | Size | Field | Notes |
|---|---|---|---|
| 0x00 | 4 | magic | "MPQ\x1A" |
| 0x04 | 4 | header_size | must be ≥ 0x20 |
| 0x08 | 4 | archive_size | informational only, not validated |
| 0x0C | 2 | format_version | must be 0 for D2; others are rejected |
| 0x0E | 2 | sector_size_shift | sector size = 0x200 << shift |
| 0x10 | 4 | hash_table_pos | relative to `A` |
| 0x14 | 4 | block_table_pos | relative to `A` |
| 0x18 | 4 | hash_table_count | entries; must be a power of two, ≥ 1 |
| 0x1C | 4 | block_table_count | entries |

Both tables must lie completely inside the file, and the sector size must
not exceed 0x200 << 15. Otherwise the archive is rejected as corrupt.

### 2. Crypt table

A table `T` of 0x500 dwords is generated once:

```
seed = 0x00100001
for i in 0 .. 0x100:
    for t in 0 .. 5:                      # T[t*0x100 + i]
        seed = (seed * 125 + 3) mod 0x2AAAAB
        hi   = seed & 0xFFFF
        seed = (seed * 125 + 3) mod 0x2AAAAB
        lo   = seed & 0xFFFF
        T[t*0x100 + i] = (hi << 16) | lo
```

The products fit in 32 bits (seed < 0x2AAAAB).

### 3. String hash

`hash(s, type)` with `type` in 0..=3:

```
seed1 = 0x7FED7FED
seed2 = 0xEEEEEEEE
for each byte b of s:
    b = normalize(b)
    seed1 = T[type*0x100 + b] ^ (seed1 + seed2)
    seed2 = b + seed1 + seed2 + (seed2 << 5) + 3
return seed1
```

`normalize` maps `a`–`z` to `A`–`Z` and `/` to `\`. Every other byte is
unchanged, including bytes ≥ 0x80.

Hash types: 0 = hash-table index, 1 = name check A, 2 = name check B,
3 = encryption key.

### 4. Decryption

`decrypt(data, key)` works on `floor(len/4)` dwords. Trailing bytes (len mod
4) are left unchanged.

```
seed = 0xEEEEEEEE
for each dword d (in place):
    seed  = seed + T[0x400 + (key & 0xFF)]
    plain = d ^ (key + seed)
    key   = ((!key << 21) + 0x11111111) | (key >> 11)
    seed  = plain + seed + (seed << 5) + 3
    d     = plain
```

Encryption, needed only for tests, is the same loop with `cipher = plain ^
(key + seed)`. The seed update uses the plaintext dword in both directions.

Table keys: hash table `hash("(hash table)", 3)` = 0xC3AF3770, block table
`hash("(block table)", 3)` = 0xEC83B3A3.

### 5. Hash table

`hash_table_count` entries of 16 bytes at `A + hash_table_pos`, decrypted as
a whole with the hash-table key:

| Offset | Size | Field |
|---|---|---|
| 0x0 | 4 | name_a = hash(name, 1) |
| 0x4 | 4 | name_b = hash(name, 2) |
| 0x8 | 2 | locale (0 = neutral) |
| 0xA | 2 | platform (always 0) |
| 0xC | 4 | block_index |

`block_index` 0xFFFFFFFF = empty (never used), 0xFFFFFFFE = deleted.

**Lookup** of `name`:
1. `start = hash(name, 0) & (count - 1)`; `a = hash(name, 1)`;
   `b = hash(name, 2)`.
2. Visit entries `start, start+1, …` (wrapping), at most `count` entries.
3. Stop at an empty entry. Skip deleted entries.
4. An entry matches when `name_a == a`, `name_b == b` and
   `block_index < block_table_count`.
5. Among matches, choose the first with locale 0. If none has locale 0,
   choose the first match. No match means "file not found".

### 6. Block table

`block_table_count` entries of 16 bytes at `A + block_table_pos`, decrypted
as a whole with the block-table key:

| Offset | Size | Field |
|---|---|---|
| 0x0 | 4 | file_pos (relative to `A`) |
| 0x4 | 4 | compressed_size |
| 0x8 | 4 | file_size |
| 0xC | 4 | flags |

Flags:

| Bit | Name | Meaning |
|---|---|---|
| 0x00000100 | IMPLODE | sectors compressed with PKWARE DCL, no compression-mask byte |
| 0x00000200 | COMPRESS | sectors compressed, first byte of each is a compression mask |
| 0x00010000 | ENCRYPTED | data encrypted |
| 0x00020000 | FIX_KEY | encryption key adjusted by position and size |
| 0x00100000 | PATCH_FILE | patch file (not used by D2; rejected) |
| 0x01000000 | SINGLE_UNIT | file stored as one unit instead of sectors |
| 0x02000000 | DELETE_MARKER | file deleted (treated as not found) |
| 0x04000000 | SECTOR_CRC | offset table has one extra entry for a CRC block |
| 0x80000000 | EXISTS | entry is in use; reading a block without it is an error |

The data range `[A + file_pos, A + file_pos + compressed_size)` must lie
inside the file.

### 7. File key

For an ENCRYPTED file, `base = hash(plain_name, 3)`, where `plain_name` is
the part of the name after the last `\` or `/`. If FIX_KEY is set:
`key = (base + file_pos) ^ file_size`. Otherwise `key = base`.

### 8. Reading file data

Let `S` = sector size, `N = ceil(file_size / S)` (0 for an empty file),
`data` = the block's compressed range, and `expected(i) = min(S, file_size -
i*S)`.

**Empty file** (`file_size == 0`): the result is empty.

**SINGLE_UNIT:** the whole range is one unit. If ENCRYPTED, decrypt it with
`key`. If COMPRESS or IMPLODE is set and `compressed_size < file_size`,
decompress it (§9) to `file_size` bytes. Otherwise it is stored.

**Compressed (COMPRESS or IMPLODE, not SINGLE_UNIT):**
1. The sector offset table is at the start of `data`: `N + 1` dwords, plus
   one more if SECTOR_CRC. If ENCRYPTED, decrypt the table with `key - 1`.
2. Validate: `offset[0]` equals the table's byte length, the offsets never
   decrease, and `offset[N] ≤ compressed_size`.
3. Sector `i` is `data[offset[i] .. offset[i+1]]`. If ENCRYPTED, decrypt it
   with `key + i`.
4. If its length is less than `expected(i)`, decompress it (§9) to
   `expected(i)` bytes. If equal, it is stored as-is. If greater, the file is
   corrupt.
5. The SECTOR_CRC block (between `offset[N]` and `offset[N+1]`) is ignored.

**Uncompressed (neither flag, not SINGLE_UNIT):** `data` holds the file
contiguously and `compressed_size` must be ≥ `file_size`. If ENCRYPTED, each
sector-sized chunk `i` (the last may be shorter) is decrypted with `key + i`.

The result is the concatenation of the sectors and must be exactly
`file_size` bytes.

### 9. Sector decompression

**IMPLODE flag:** the sector is a PKWARE DCL stream (§10).

**COMPRESS flag:** byte 0 is the compression mask `M`. The rest is the
payload.
- `M == 0x12` is LZMA, and `M` containing 0x20 is sparse. Neither is used by
  D2, so both are unsupported.
- Allowed bits: 0x01 Huffman (§11), 0x02 zlib, 0x08 PKWARE DCL (§10),
  0x10 bzip2, 0x40 IMA ADPCM mono (§12), 0x80 IMA ADPCM stereo (§12). Any
  other bit, or 0x40 and 0x80 together, is an error.
- Decoders run in this fixed order, each taking the previous output:
  bzip2 → PKWARE → zlib → Huffman → ADPCM stereo → ADPCM mono.
- Each decoder's output is capped at `expected(i)` bytes.
- The final output must be exactly `expected(i)` bytes.

zlib (RFC 1950 stream) and bzip2 are standard formats. They are required
only if an archive uses them (see Observations).

### 10. PKWARE Data Compression Library ("implode") stream

Bit order: an LSB-first bit stream over the input bytes.

Header: byte 0 = literal mode (0 = binary, 1 = ASCII; anything else is an
error). Byte 1 = dictionary bits `D` (4, 5 or 6; otherwise an error). The
bit stream starts at byte 2. An input of 4 bytes or fewer is an error.

Tables (`mpq-tables.md` §B): `LenBits`, `LenCode`, `ExLenBits`, `LenBase`
(16 entries), `DistBits`, `DistCode` (64 entries), `ChBits`, `ChCode` (256
entries). Each `(code, bits)` pair is a prefix code already written in
LSB-first bit order: symbol `i` is matched when the next `bits[i]` stream
bits, read as an LSB-first integer, equal `code[i]`.

Decoding loop, until the end marker:
1. Read 1 bit (flag).
2. **Flag = 1: a copy.**
   a. Decode the length symbol `l` (0..15) with `LenCode/LenBits`.
   b. If `ExLenBits[l] > 0`: read `ExLenBits[l]` bits as `e`, and
      `l = LenBase[l] + e`.
   c. If `l == 0x205` (519), this is the **end marker**: stop.
   d. Copy length `n = l + 2` (2..=518).
   e. Decode the distance symbol `d` (0..63) with `DistCode/DistBits`.
   f. If `n == 2`: read 2 bits `r`, distance `= (d << 2) | r`. Otherwise
      read `D` bits `r`, distance `= (d << D) | r`.
   g. Copy `n` bytes, one at a time, from `distance + 1` bytes back in the
      output. Overlapping copies repeat the pattern. A distance reaching
      before the start of the output is an error.
3. **Flag = 0: a literal.** In binary mode, read 8 bits as the byte. In
   ASCII mode, decode a symbol with `ChCode/ChBits`; the symbol is the byte.
4. Output beyond the requested size is discarded.

End of input: if the input runs out in the middle of a symbol, the stream is
corrupt, except while reading the extra length bits of the end marker
(length symbol 15 with all extra bits 1). There, running out of input still
counts as the end marker.

### 11. Huffman (Storm adaptive Huffman)

Bit order: LSB-first bit stream. Byte 0 (the first 8 bits) selects the
weight table `t` (0..=8; otherwise an error) from `mpq-tables.md` §C. Table
`t` gives weights for symbols 0x00–0xFF. Symbol 0x100 (end of stream) and
symbol 0x101 (escape: a new byte value follows) both have weight 1.

**Structures.** Nodes are leaves (a symbol) or branches (child0, child1).
Every node has a weight. All nodes are kept in one ordered list `L`,
non-increasing in weight from head to tail. The tree root is the head of
`L`.

**Insert rule.** A node of weight `w` is placed immediately after the last
node in `L` whose weight is ≥ `w`. If no node has weight ≥ `w`, it becomes
the head.

**Build.**
1. For each symbol 0x00..=0xFF in increasing order with non-zero weight,
   create a leaf and insert it. Then insert leaf 0x100, then leaf 0x101,
   each with weight 1.
2. Let `c` = the tail of `L`. While `c` is not the head: `a = c`; `b` = the
   node before `a`. Create a branch with child0 = `a`, child1 = `b` and
   weight `a.w + b.w`, and insert it (insert rule). Then `c` = the node
   before `b`, looked up after the insertion.
3. The root is the head of `L`.

**Increment(n)** (adaptive weight update), repeated for `n`, then its
parent, and so on up to and including the root:
1. Let `w = n.w`. Let `lead` = the first node in `L` (closest to the head)
   with weight `w`.
2. If `lead != n`: swap `n` and `lead` in `L` (exchange positions), and swap
   them in the tree. If they have the same parent, swap that parent's two
   children. Otherwise each takes the other's place under the other's
   parent.
3. `n.w = w + 1`. Continue with `n`'s parent (after the swap).

**AddValue(v)** (on escape):
1. Create leaf `v` with weight 0 and insert it (insert rule). It becomes the
   tail.
2. Let `a` = that leaf and `b` = the node before it. Create a branch `m`
   with weight `b.w`, child0 = `a`, child1 = `b`. `m` takes `b`'s place
   under `b`'s parent. In `L`, `m` is placed immediately before `b`.
3. Return `a`.

**Decode.** `adaptive = (t == 0)`. Repeat while the output is not full:
1. Walk from the root, reading 1 bit per branch (0 → child0, 1 → child1)
   until a leaf is reached.
2. Symbol 0x100: stop.
3. Symbol 0x101: read 8 bits as `v`; `n = AddValue(v)`; `Increment(n)`; if
   not adaptive, `Increment(n)` again. The output symbol is `v`.
4. Otherwise the output symbol is the leaf's symbol, `n` = that leaf.
5. Append the symbol to the output. If adaptive, `Increment(n)`.

Running out of input in the middle of a code is an error.
Input that ends between codes (no 0x100, output not full) and an
Increment whose `lead` is the root or `n`'s parent are also errors in
d2rs (design choice: no valid stream of the shipped archives reaches
either; the Storm behaviour on corrupt input is not reproduced).

### 12. IMA ADPCM (Storm variant)

Tables (`mpq-tables.md` §D): `StepSize` (89 entries), `ChangeTable` (32
entries). `C` = channel count (1 for mask 0x40, 2 for mask 0x80). Output
samples are i16 little-endian. Arithmetic uses 32-bit signed integers.

1. Input byte 0 is ignored. Byte 1 is `shift`. Inputs shorter than 2 bytes
   produce no output.
2. For each channel `c`: read an i16 as `sample[c]`, output it, and set
   `index[c] = 44`. Stop if input or output runs out.
3. `ch = 0`. For each remaining input byte `op`, while the output is not
   full:
   - If `op & 0x80`:
     - `op & 0x7F == 0`: if `index[ch] > 0`, decrement it. Output
       `sample[ch]`. `ch = (ch+1) mod C`.
     - `== 1`: `index[ch] = min(index[ch] + 8, 88)`. `ch` unchanged.
     - `== 2`: `ch = (ch+1) mod C`.
     - otherwise: `index[ch] = max(index[ch] - 8, 0)`. `ch` unchanged.
   - Else (an encoded sample):
     - `base = StepSize[index[ch]]`; `diff = base >> shift`.
     - For bit `k` in 0..=5: if `op & (1 << k)`, `diff += base >> k`.
     - If `op & 0x40`: `sample[ch] = max(sample[ch] - diff, -32768)`.
       Otherwise `sample[ch] = min(sample[ch] + diff, 32767)`.
     - Output `sample[ch]`.
     - `index[ch] = clamp(index[ch] + ChangeTable[op & 0x1F], 0, 88)`.
     - `ch = (ch+1) mod C`.

### 13. Recovering the key of an unnamed file

This is a tooling aid, used to decode every block when names are unknown.
It applies to encrypted, compressed, non-single-unit files, whose offset
table starts with the known dword `P = (N + 1 + crc) * 4`, where `crc` is 1
if SECTOR_CRC and 0 otherwise.

Let `E0`, `E1` be the first two encrypted dwords and
`k12 = (E0 ^ P) - 0xEEEEEEEE`. For `i` in 0..=255:
- `k1 = k12 - T[0x400 + i]`.
- Decrypt `E0` with `k1` (§4). If the result equals `P`, decrypt `E1` as the
  second dword of the same stream.
- If that result is ≤ `P + S`, the candidate file key is `k1 + 1`.

Accept a candidate only if the whole file then decodes to `file_size`
bytes. FIX_KEY needs no special handling: the recovered key already
includes the adjustment.

### 14. Listfile

If the archive contains `(listfile)`, it is text listing file names,
separated by any of CR, LF or `;`. Empty entries are ignored. Names are used
as given. The listfile is the only source of names inside an archive.

## Constants & data dependencies

All constant tables are in `specs/formats/mpq-tables.md`. No .txt tables are
read.

## Randomness

None.

## Edge cases & original bugs

1. Encrypted lengths that aren't a multiple of 4 leave the last 1–3 bytes
  unencrypted (§4).
2. A compressed sector exactly `expected(i)` long is stored raw, even under
  COMPRESS (§8.4).
3. The PKWARE end marker may end exactly at the end of input (§10).
4. Huffman tables 1–8 aren't adaptive, but escaped values still get two
  increments (§11 Decode step 3).

## Archive set (D2-specific)

Lookup across archives (first match wins): the order is defined in
`specs/data/loading.md` §2 (priority descending, ties newest-opened first):
`patch_d2`, `d2xvideo`, `d2xtalk`, `d2xmusic`, `d2exp`, `d2video`,
`d2music`, `d2char`, `d2speech`, `d2sfx`, `d2data`.

## Observations (1.14d install)

Survey of all 11 archives (`mpq-tool check`, 2026-10-05): 35,364 blocks in
use, all decoded, 0 errors.

- Header: format version 0, offset 0, sector size 4096 (shift 3) in every
  archive.
- Locales: only 0 (neutral) appears in any hash table.
- Block flags seen: `0x80000000` (stored), `0x80000200` (COMPRESS),
  `0x80030200` (COMPRESS + ENCRYPTED + FIX_KEY), and `0x80000100` (IMPLODE,
  only in `patch_d2.mpq`). Never seen: SINGLE_UNIT, SECTOR_CRC, PATCH_FILE,
  DELETE_MARKER.
- Sector compression masks seen: `0x08` (PKWARE), `0x40`/`0x80` (ADPCM
  alone), `0x41`/`0x81` (Huffman + ADPCM). zlib, bzip2, LZMA and sparse are
  never used, so they stay unimplemented.
- Compressed sector counts over all 11 archives (block-table walk, §13
  key recovery for every encrypted block, 0 failures): mask 0x08 123,593,
  0x41 174,997, 0x81 175,546, 0x40 21, 0x80 8. Every one of the 350,543
  Huffman + ADPCM sectors (0x41 / 0x81) selects Huffman weight table 8
  (§11 byte 0); tables 0–7 never occur. 89.4 % of them contain at least
  one escape (symbol 0x101), PC 2's full-decode check (`formats/wav.md`
  Survey mirrors this).
- Encrypted + FIX_KEY is used for `.wav` files and a few special files.
- `(listfile)`: present in all archives except `patch_d2.mpq`, but often
  incomplete. `d2sfx.mpq` lists only 31 of its 2,360 files. §13 key recovery
  decoded every unnamed encrypted block (2,345 blocks).
- All 4,992 RIFF (`.wav`) blocks decode to their exact RIFF size (4,975 of
  them use ADPCM sectors; counts per archive in `wav.md` Survey).
  Decoded speech (`Cain_act1_gossip_01.wav`, 22,050 Hz mono) has no clipped
  samples, with mean |Δsample| / mean |sample| = 0.23, consistent with
  clean audio.

## Test vectors

| Input / seed | Expected output | Source |
|---|---|---|
| `T[0x000]`, `T[0x001]`, `T[0x100]`, `T[0x400]`, `T[0x4FF]` | 0x55C636E2, 0x02BE0170, 0x76F8C1B1, 0x193AA698, 0x7303286C | crypt-table formula; matches published StormLib/Riiablo tables |
| `hash("(hash table)", 3)` | 0xC3AF3770 | published MPQ constant |
| `hash("(block table)", 3)` | 0xEC83B3A3 | published MPQ constant |
| encrypt then decrypt any buffer, any key | original buffer | §4 |
| PKWARE `00 04 82 24 25 8F 80 7F` | ASCII `AIAIAIAIAIAIA` (13 bytes) | blast.c (zlib contrib) test vector; checked by hand against §10 |
| ADPCM mono, input `00 00 10 00 01` | samples `16, 1004` | §12: index 44 → base 494, diff 494 + 494 |
| ADPCM mono, input `00 00 10 00 80` | samples `16, 16` | §12 command 0 |
| every block of every 1.14d MPQ | decodes to `file_size` bytes | `game/` (ignored test) |
| `.wav` files (Huffman + ADPCM) | output starts with `RIFF`, RIFF size + 8 == file size | `game/` (ignored test) |

## Provenance

The MPQ format is publicly documented by the modding community. Sources:
- Community documentation of the MoPaQ format: header, tables, hash and
  encryption algorithms, flags, sector layout.
- Riiablo (Apache-2.0), `com.riiablo.mpq_bytebuf`: crypt tables, PKWARE,
  Huffman and ADPCM tables and decoding behavior. It decodes D2 archives in
  practice. The constant tables in `mpq-tables.md` were taken from it, with
  a notice in `THIRD_PARTY_NOTICES.md`.
- StormLib behavior (MIT) as documented publicly: decoder order,
  key-recovery idea, ADPCM 32-bit arithmetic.
- blast.c (zlib contrib): PKWARE test vector.

No Blizzard code or decompiler output was consulted. `re/` was not opened.

Deliberate difference from Riiablo: ADPCM uses 32-bit arithmetic (§12).
Riiablo uses 16-bit Java `short` for the step sum, which can overflow.

## Open questions

1. ~~Archive priority order for 1.14d (Archive set).~~ Answered by
   `loading.md` §2; the open timing of its second group and video path is
   its open question 1.
2. Locale handling: does 1.14d ever request a non-neutral locale? The
   observation survey records which locales appear in the tables.
3. ~~Which compression masks and flags 1.14d uses, and whether archives
   contain `(listfile)`.~~ Answered: see Observations.
4. File names for unlisted files (most of `d2sfx.mpq`, all of
   `patch_d2.mpq`): the engine opens files by name, so this only matters
   for tooling. Names can be collected from references inside the data
   (e.g. sound names in the sound tables) once `d2-data` exists.
