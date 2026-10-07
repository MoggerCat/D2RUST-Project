# Spec: Formats — Sound files (.wav)

- **Status:** verified for parsing: `cargo test -p d2-formats --test
  wav_game -- --ignored` (C72) passes on the 1.14d files (the 8 Test
  vector files, and every `sounds.txt` file: 4,508 parse, 4,434 mono, 74
  stereo; `docs/handoff/local-buddy-q-data.md` entry 72). Rules from the
  1.14d `Game.exe` loaders (addresses below) and a survey of all 4,992
  RIFF blocks; the samples handed to DirectSound are not yet dumped
  (Open question 1).
- **Target version:** 1.14d
- **Crate/module:** `d2-formats::wav`
- **Related specs:** `formats/mpq.md` §9–§12 (sector decompression, IMA
  ADPCM), `client/audio.md` §A1 / §B1 (decode path, exactness check 1),
  `client/assets.md` §A5 (sound pool)

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 36–47 |
| Inputs | 48–53 |
| Outputs / state changes | 54–59 |
| Rules | 60–63 |
|   1. Header | 64–70 |
|   2. Chunk walk (no pad bytes) | 71–106 |
|   3. Samples | 107–118 |
|   4. Format checks by the game | 119–131 |
| Constants & data dependencies | 132–139 |
| Randomness | 140–143 |
| Survey (1.14d data) | 144–196 |
| Edge cases & original bugs | 197–210 |
| Test vectors | 211–249 |
| Provenance | 250–265 |
| Open questions | 266–283 |
<!-- /index -->

## Summary

Every D2 sound (effects, speech, music) is a RIFF/WAVE file inside an MPQ.
The only compression is at the MPQ sector level (`mpq.md`: Huffman + IMA
ADPCM, PKWARE); after the MPQ layer every live file is plain 16-bit PCM
(format tag 1), 22,050 Hz, mono or stereo. No `.wav` uses an in-file codec
(no tag 0x11 IMA ADPCM, no MS ADPCM). The game finds the `fmt ` and `data`
chunks with a walk that ignores RIFF pad bytes, then copies the `data`
bytes unchanged into a DirectSound buffer whose format is fixed at
22,050 Hz, 16-bit: there is no resampling or conversion on load. This spec
covers one file's bytes → (format, samples).

## Inputs

| Name | Type | Source |
|---|---|---|
| file bytes | `.wav`, after MPQ decode | `data\global\sfx\…`, `data\local\sfx\…`, `data\global\music\…` (`sounds.txt` FileName with one of these prefixes) |

## Outputs / state changes

`Wav { format_tag, channels, rate, bits, block_align, samples: Vec<i16> }`
or an error. `samples` are interleaved (L, R for stereo); frames =
`samples.len() / channels`.

## Rules

All integers little-endian.

### 1. Header

- The file must be at least 32 bytes, with `RIFF` at offset 0 and `WAVE`
  at offset 8; otherwise the load fails (0x516760).
- The RIFF size field (offset 4) is **not read**. The walk below is bounded
  by the actual file length minus 12.

### 2. Chunk walk (no pad bytes)

From offset 12, with `remaining = file_len − 12`, a chunk search for an ID
repeats (0x5166D0):

1. If `remaining < 8`, not found. Else `remaining −= 8` and read the 8-byte
   header (ID, u32 size).
2. If the ID matches, found: the body starts right after the header and
   `remaining` still counts the body.
3. Else if `remaining < size`, not found. Else `remaining −= size` and the
   next header is at `body + size`. **The RIFF word-alignment pad byte after
   an odd-sized chunk is not skipped**, so an odd chunk before the one being
   searched for misaligns the walk (§Edge cases).

Load order (0x516760):

1. Search `fmt ` from offset 12. Its size must be ≥ 16 and ≤ `remaining`;
   then `remaining −= size`. The first 16 bytes are the PCM format fields:
   format tag u16, channels u16, sample rate u32, byte rate u32, block align
   u16, bits per sample u16. Bytes past 16 are skipped (`cbSize` is forced
   to 0).
2. Search `data` starting right after the `fmt ` body (chunks between them
   are skipped, so `fmt ` must come before `data`). Its size must be ≤
   `remaining` (a `data` chunk running past the file end fails the load).
3. Result: the format fields and the `data` body (`size` bytes).

Chunks other than `fmt ` and `data` (`PAD `, `LIST`, `smpl`, `cue `) are
never interpreted: no fourcc for them occurs anywhere in `Game.exe`
(search of the whole disassembly for `smpl`, `cue `, `LIST`, `PAD `). In
particular `smpl` loop points do not drive looping.

The Storm streaming loader (0x41B280, chunk search 0x41B210, used through
0x515D70) performs the same walk by seeking in the archive file: same
`fmt ` size ≥ 16 check, same pad-free skipping, `data` searched after
`fmt `.

### 3. Samples

The `data` body is copied byte for byte into a DirectSound buffer
(in-memory voices: `memcpy` in 0x515180; the remainder of a non-looping
buffer is filled with zero bytes; streams: 0x41B280, silence byte 0x80
for 8-bit, 0 otherwise). Every voice buffer is created with a fixed
format built by 0x516720: PCM, 22,050 Hz, 16-bit, `channels` = 1 or 2
(voices 0x5153C0, primary buffer 0x5140D0 is 22,050 Hz 16-bit stereo).
So the samples the game outputs are the `data` bytes read as i16
little-endian, interleaved by channel. d2rs: `samples[i]` = i16 at
`data[2i..2i+2]` for `i < size / 2`.

### 4. Format checks by the game

| Path | Address | Check | Channels from |
|---|---|---|---|
| sound start | 0x4DF630 (called from 0x481720, 0x482970) | bits == 16 and rate == 22,050, else the sound is marked failed (not played) | file: stereo iff channels == 2 (written into the row's `Stereo`, +0x50) |
| UI preload ("D2SoundFast", 15 fixed names: `cursor\button.wav` and the 14 `cursor\intro\<class> select/deselect.wav`) | 0x514E10 (15 calls from 0x4359D0; one from 0x441B70) | rate == 22,050, else not cached | file: stereo iff channels == 2 |
| voice data attach | 0x5155D0 | none on format; fatal error if the voice's loop start ≥ data size | voice |

The format tag, byte rate and block align are never checked. d2rs: the
parser returns all fields; the audio layer accepts only tag 1, 16-bit,
22,050 Hz (every file `sounds.txt` names passes; Survey) and treats any
other file as failed to load, as 0x4DF630 does.

## Constants & data dependencies

- Fixed output format: 22,050 Hz, 16-bit (0x516720; primary buffer
  0x5140D0: byte rate 88,200, block align 4).
- `sounds.txt` `Stereo` does **not** match the file in 30 rows (see
  Survey); the decoded samples depend only on the file. Which voice plays
  them is `client/audio.md` §B3.

## Randomness

None.

## Survey (1.14d data)

Every block of every archive decoded (`mpq-tool check`: 0 errors) and each
`RIFF` block parsed. Counts of RIFF blocks per archive (`mpq-tool check`
"wav valid"; all have RIFF size + 8 == file length):

| Archive | Mono 16-bit 22,050 | Stereo 16-bit 22,050 | Other |
|---|---|---|---|
| d2sfx.mpq | 2,298 | 29 | 1 (8-bit mono 11,025 Hz, unnamed block 118) |
| d2speech.mpq | 1,565 | 0 | 0 |
| d2exp.mpq | 571 | 4 | 0 |
| d2xtalk.mpq | 474 | 2 | 0 |
| d2music.mpq | 0 | 33 | 0 |
| D2xMusic.mpq | 0 | 10 | 0 |
| Patch_D2.mpq | 5 | 0 | 0 |
| **total 4,992** | 4,913 | 78 | 1 |

- Format tag: 1 (PCM) in all 4,992. `fmt ` size: 16 in all. Byte rate =
  rate × block align and block align = channels × bits / 8 in all.
- Chunk orders (count): `fmt PAD data` 1,819; `fmt data LIST` 1,558;
  `fmt data` 1,375; `fmt data smpl` 98; `fmt cue LIST data` 60;
  `fmt data smpl LIST` 48; `fmt data cue LIST` 18;
  `fmt data LIST cue LIST` 8; `fmt data smpl cue LIST` 5;
  `fmt data smpl LIST cue LIST` 2; `fmt LIST PAD data` 1. `fmt ` is always
  first. `PAD ` bodies are all zero (4,044 bytes ×1,819, 3,968 ×1); `LIST`
  types `INFO` 1,617, `adtl` 93; `smpl` with one loop 106, none 47.
- Odd-sized chunks: 1, the `data` of d2sfx block 118 (31,361 bytes, then
  a pad byte and a `LIST`). No odd chunk precedes a `data` chunk, so the
  pad-free walk finds every `data` correctly. No trailing bytes after the
  last chunk in any file.
- `data` size is a multiple of block align in all; no `data` is truncated;
  no `data` is empty.
- MPQ layer: all `.wav` blocks are COMPRESS + ENCRYPTED + FIX_KEY except
  Patch_D2's 5 (IMPLODE, lossless). Mono files use ADPCM masks
  0x40/0x41 (4,897 files), stereo files 0x80/0x81 (all 78); the other
  sectors are PKWARE (0x08) or stored. 17 RIFF files have no ADPCM
  sector (lossless). For ADPCM files the samples are lossy and depend on
  bit-exact `mpq.md` §12 decoding. Every 0x41 / 0x81 sector (350,543:
  174,997 mono, 175,546 stereo) carries Huffman weight table 8 (count
  byte 0) and no other table occurs; 313,273 of them (89.4 %) use the
  escape path (`docs/handoff/local-buddy-q-data.md` entry 62, a counter
  over a copy of our decoder, 0 decode errors).
- Names: two `.wav` names in d2sfx are not RIFF: `cursor\wavindx.wav` and
  `cursor\curindx.wav` (72 bytes each, binary). No zero-length `.wav`.
- `sounds.txt` (4,699 rows): 4,508 rows resolve (2,777 under
  `data\global\sfx\`, 1,689 `data\local\sfx\`, 42 `data\global\music\`),
  4,399 distinct files; the other 191 rows name `none.wav`. All resolved
  files are tag 1, 16-bit, 22,050 Hz (4,434 mono, 74 stereo). `Stereo`=1
  with a stereo file: 44; `Stereo`=0 with a stereo file: 30 (the 10
  `cursor\intro` select/deselect files and 20 `ambient\scene\*.wav`).
  Block 118 and the two `*indx.wav` files are not named by `sounds.txt`
  nor by any `.wav` string in `Game.exe` (30 strings: 15 music, 15 UI).

## Edge cases & original bugs

| Case | Original | Live count |
|---|---|---|
| file < 32 bytes, or no `RIFF`/`WAVE` | load fails | 2 (`*indx.wav`, unreferenced) |
| RIFF size field wrong | ignored | 0 |
| odd chunk before `data` (or before `fmt `) | walk misaligned by the missing pad byte; normally "not found" → load fails | 0 |
| `data` before `fmt ` | `data` not found → load fails | 0 |
| `data` past end of file (truncated) | load fails | 0 |
| `data` size not a multiple of 2 × channels | bytes copied as is; a trailing odd byte is half a sample. d2rs: drop it | 0 |
| zero-length `.wav` | load fails (< 32 bytes) | 0 |
| not 16-bit or not 22,050 Hz | 0x4DF630 rejects; 0x5155D0 would play the bytes as 16-bit 22,050 | 1 (block 118, unreferenced) |
| format tag ≠ 1 | not checked: bytes played as PCM | 0 |

## Test vectors

Synthetic (build in the test; "fmt16" = `fmt ` size 16, tag 1, 1 channel,
22,050 Hz, byte rate 44,100, block align 2, 16 bits):

| Input | Expected | Source |
|---|---|---|
| `RIFF`, size 40, `WAVE`, fmt16, `data` size 4: `01 00 FF FF` (48 bytes) | mono, 22,050 Hz, samples `[1, -1]` | §2, §3 |
| same with RIFF size field 0 | same result | §1 |
| fmt16, `PAD ` size 4 (zeros), `data` as above | samples `[1, -1]` | §2 |
| fmt16, `LIST` size 3 + 1 pad byte, `data` size 4 (60 bytes) | error: the walk reads ID `00 'd' 'a' 't'`, size 0x461 > remaining 5 → `data` not found | §2 step 3 |
| `data` (size 4) before fmt16 | error: `data` not found | §2 |
| `data` size 6 with 4 body bytes at end of file | error | §2 |
| first 31 bytes of the 48-byte file | error | §1 |
| stereo fmt16 (2 ch, byte rate 88,200, align 4), `data` `01 00 02 00 03 00 04 00` | frames 2, samples `[1, 2, 3, 4]` (L R L R) | §3 |

MPQ-level IMA ADPCM vectors are in `mpq.md` Test vectors (no `.wav` holds
in-file ADPCM).

Game files (`#[ignore]`, `D2_GAME_DIR`; read through `ArchiveSet` so the
highest-priority archive wins). Sum = sum of all i16 values as i64;
first 8 = first 8 interleaved i16 values; CRC = CRC-32 (IEEE, as zlib) of
the `data` bytes; frames = `data` bytes / (2 × channels):

| File | Archive | Ch | Frames | Sum | First 8 | CRC |
|---|---|---|---|---|---|---|
| `data\global\sfx\cursor\pass.wav` | d2sfx | 1 | 1,124 | 95,592 | 0,-3,1,16,30,44,26,-41 | 236e8962 |
| `data\global\sfx\item\gem.wav` | Patch_D2 | 1 | 15,627 | -4,923 | -4,-2,-21,-43,-49,-39,-37,-46 | 03050e69 |
| `data\global\sfx\cursor\leveluphireling.wav` | d2exp | 1 | 15,065 | -6,073,869 | 0,0,1,1,2,3,5,6 | 06de9042 |
| `data\global\sfx\cursor\intro\amazon select.wav` | d2sfx | 2 | 54,397 | 245,877 | 0,0,0,0,0,0,0,0 | ffff477e |
| `data\local\sfx\act1\amazon\ama_act1_find_tristram.wav` | d2speech | 1 | 65,184 | 41,165 | -6,-6,-6,-6,-6,-6,-15,-7 | e3cfd2ff |
| `data\local\sfx\act1\druid\dru_act1_find_tristram.wav` | d2xtalk | 1 | 106,857 | -3,897 | -55,-70,-74,-106,-90,-65,-73,-60 | 6f43da9e |
| `data\global\music\act1\crypt.wav` | d2music | 2 | 5,971,968 | -993,081,113 | 79,209,-73,-11,-185,-189,-300,-384 | 106ab6a9 |
| `data\global\music\act5\baal.wav` | D2xMusic | 2 | 5,792,521 | -14,599,737 | 2,0,3,0,1,0,2,0 | 4c723350 |
| every file named by `sounds.txt` | — | — | — | parses; tag 1, 16-bit, 22,050 Hz; 4,434 mono, 74 stereo | — | — |

These values come from our own MPQ decoder (`d2-formats`), so they pin
regressions; equality with the original's output is Open question 1.

## Provenance

- 1.14d `Game.exe`: 0x516760 (RIFF/WAVE check, `fmt `/`data` lookup),
  0x5166D0 (in-memory chunk search), 0x41B280 / 0x41B210 (Storm streaming
  begin and chunk search, via 0x515D70), 0x516720 (voice WAVEFORMATEX),
  0x5153C0 (voice buffer create), 0x5140D0 (DirectSound init, primary
  format), 0x515180 (buffer fill, `memcpy`), 0x5155D0 (attach data to a
  voice), 0x4DF630 and 0x514E10 (format checks), 0x4359D0 (UI preload
  list). Register arguments read with `tools/ghidra/disasm.py`.
- Survey: `mpq-tool check` (RIFF counts) plus a throwaway dumper over
  `d2-formats` (scratch, not committed) that decoded every block, walked
  the chunks of every RIFF block, and resolved every `sounds.txt`
  FileName through `ArchiveSet`.
- RIFF/WAVE chunk structure: Microsoft multimedia format documentation
  (public). No D2MOO or Riiablo code used.

## Open questions

1. Equality with the original's samples is not measured: dump the
   DirectSound buffer contents after 0x515180 (or the `data` pointer
   returned by 0x516760) for the Test vectors files and compare with our
   decoded `data`. This settles both this spec and the bit-exactness of
   `mpq.md` §12 ADPCM on real files (`client/audio.md` §B1).
2. The Storm stream path (0x41B280) is assumed to copy bytes unchanged
   like 0x515180; its refill thread was not traced. Settled by the same
   dump on a `Stream`=1 sound (e.g. `music\act1\crypt.wav`).
3. Answered: on a stereo voice. All 30 rows are non-`Stream`, so their
   sample is loaded through `0x00482970` / `0x00481720`, both of which
   run 0x4DF630; on success it overwrites the row's `Stereo` byte with
   (channels = 2) (`0x004DF695`) before the start picks the channel kind
   (`audio/sound-table.md` §7 r7). The stereo slots are 2-channel buffers
   (`0x005153C0`). A `Stream` row would keep its cell (no load); none of
   the 30 is one.
