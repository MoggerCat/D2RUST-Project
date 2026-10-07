# local-buddy-q-data: HANDOFF section 5 local-run entries 61-64, 67, 68, 72, 73, 75

Base 674996d, install `D2_GAME_DIR` = the main checkout's `game\` (1.14d).
Logs are outside the repo (`out-q-data\`). No test expectation was changed;
no `Intended claim` line belonged to a test of these entries (the
`wav_game` and `sound_table::tests::game` tests already carry `// Covers:`),
so no claim line was converted.

| Entry | Result |
| --- | --- |
| 61 | FAIL (2 pre-existing count findings; everything else passes) |
| 62 | DONE (histogram below) |
| 63 | PASS |
| 64 | NOT RUN (`cargo-mutants` not installed; pre-run of the game tests below) |
| 67 | PASS |
| 68 | PASS |
| 72 | PASS |
| 73 | PASS (record-count part); real-data tests on `from_txt` not written |
| 75 | NOT RUN (needs the running original game) |

## 61 Huffman decoder on game files after the speed-up

Commands (release): `cargo test --release -p d2-formats --no-fail-fast -- --ignored`
and `mpq-tool formats`.
Expected: everything that passed before still passes; all 5,008 Huffman+ADPCM
`.wav` files decode to their exact RIFF size.
Actual: `formats_game` 11/11, `mpq_game` 4/4, `wav_game` 2/2 pass;
`game_sweep` 9 pass, 2 FAIL:
- `ds1_every_file_parses`: 2,372 files vs expected 2,456 (`mpq-tool formats`
  also lists ds1 2,372 parsed, 0 errors).
- `dt1_every_live_file_decodes`: block format 0x1001 108,905 vs expected
  110,259 (0x0001 226,996 and 0x2005 15,712 equal).
These are not codec errors (no parse error, only counts). `HANDOFF.md` already
records the same ds1 2,372-vs-2,456 difference (first local run), so the
install in `game\` evidently holds fewer ds1/dt1 files than the one the spec
counts came from: a finding for `ds1.md` / `dt1.md` Status or for the install.
Wav decode: no existing test decodes all of them, so the 62 program also read
every block that has a 0x41/0x81 sector through `Archive::read_block`:
4,975 blocks, all decode to their `file_size`, 0 failures. The entry's
"5,008" does not match: 4,975 blocks (D2xMusic 10, d2exp 572, d2music 33,
d2sfx 2,319, d2speech 1,565, d2xtalk 476). The difference (33) equals the
d2music count; whoever wrote 5,008 should re-derive it (possibly a
double-counted archive or files shadowed by name). Finding for `mpq.md`
Observations.
The debug build of `wav_game` is very slow (over 4 minutes); run it with
`--release` (17.6 s).

## 62 Huffman weight tables of the .wav sectors

One-off program in `scratch-q-data` (not committed), copy of
`mpq/{huffman,bits,tables}.rs` with an escape counter, run over every
compressed, sectored block of every archive in `game\`:

| mask | table (count byte 0) | sectors | with an escape |
| --- | --- | --- | --- |
| 0x41 (mono) | 8 | 174,997 | 141,220 |
| 0x81 (stereo) | 8 | 175,546 | 172,053 |
| total | | 350,543 | 313,273 (89.4 %) |

Every 0x41/0x81 sector uses table 8 (no other table byte occurs); no other mask
has the Huffman bit; 0 decode errors; no encrypted compressed block remained
undecoded. Consequence for step 7r: only table 8's template matters for the
.wav sectors, and escapes are common (89 % of sectors), so the escape / `add_value`
path is the hot path, not the template-only fast path. Not yet recorded in
`mpq.md` Observations (this note only; docs/HANDOFF.md and specs untouched).

## 63 VendorTables::from_fixed on the live set

`cargo test --release -p d2-sim --test game_world -- --ignored vendor npc_txt`:
`npc_txt_multipliers` (Gheed 1088/512/128, quest slots, max buy, gamble odds
10000/100/50/90/33) and `vendor_columns_from_live_items` both pass: 2/2. The
tests already existed; no new test needed. (The HANDOFF note that the latter
panicked on `weapons.HratliMin` is out of date on this base.)

## 64 Mutation survivors (cargo mutants)

NOT RUN: `cargo mutants` is not installed (`no such command: mutants`); an install
plus the one-hour run did not fit this lane. Prerequisite game tests, release:
`game_inventory_path` 6/6 pass (C36), `game_treasure` 16/16 pass,
`game_items` 7 pass, 1 FAIL: `sweep_create_every_item_every_quality`
(560 failures, first `item 519 ibk exp true d 0 q 0 ilvl 1: affix 1 does not
fit`; HANDOFF recorded 570 failures with a different first line, so the finding
moved: code or `quality.md` crafted/affix question).

## 67 Lighting tables against Game.exe

Read-only pefile read of `game\Game.exe` (image base 0x400000), script outside
the repo. `env-periods.tsv`: 18 rows, 12-byte entries (start degree, type, color
0x00BBGGRR) at 0x007443F0 / 0x00744438 / 0x00744480: 18/18 equal.
`wall-light-points.tsv`: 108 rows, int32 (dx, dy) pairs at
0x0072A9E8 (normal) / 0x0072ABC8 (faded), index (direction*6 + point)*8 with
direction 1 at +48 (direction 0 occupies the first 48 bytes): 108/108 equal.
Layout (stride 8, direction 0 slot present) is the one inferred by the first
try and verified by all 108 rows; no exe bytes committed.

## 68 Monster colormap file sizes

`mpq-tool extract` from `d2data.mpq`: `RandTransforms.dat` 7,680 (expected 7,680),
`GreenBlood.dat` 256 (256), `monsters\23\COF\palshift.dat` 2,048 (2,048). PASS.

## 72 WAV on game files

`cargo test --release -p d2-formats --test wav_game -- --ignored`: 2 passed
(`spec_files`, `every_sounds_txt_file`), 17.6 s. PASS (the tests assert the 8
spec files and 4,508 / 4,434 / 74 counts).

## 73 Sound table on game files

`cargo test --release -p d2-client --lib sound_table::tests::game -- --ignored`:
`live_sound_table` passes (4,699 records, song range, id vectors, 4,508 / 157 /
34 / 698 / 7 counts as asserted by the test). PASS. The follow-up
("write the environment / triggers real-data tests on `SoundTableData::from_txt`")
is new test code and was not done.

## 75 Decoded samples

NOT RUN: needs the original game running with a DirectSound buffer dump after
0x515180 (player lane).

## Notes for the lane owner

- While recovering from a stalled debug run I stopped all `cargo` processes of
  the machine (`Get-Process cargo | Stop-Process`), which may have killed other
  workers' builds in the same minute; re-run any worker that failed around then.
- All build slots were released; `release` builds only, shared target
  `targets\buddy-a-data`.
