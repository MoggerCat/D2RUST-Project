# Handoff: MPQ Huffman encoder, bench and decoder speed-up — `claude/mpq-huffman`

Cloud implementation session, 2026-10-06, medium. HANDOFF §2 step 7r BB1.
Repo only, synthetic data, no game files. Spec: `specs/formats/mpq.md` §9,
§11 (no spec change).

## What changed

- **Encoder** (`mpq/huffman.rs` `compress`, now
  `cfg(any(test, feature = "test-support"))` instead of `cfg(test)`):
  builds the §11 tree for weight table 0–8 and updates it symbol by symbol
  the same way the decoder does (Increment, AddValue, two increments per
  escape on tables 1–8). Each byte is written as its leaf's root path, a
  byte with no leaf as escape + 8 bits, then the end symbol. Symbol lookup
  is O(1) (symbol → leaf array) and bits go through a word accumulator.
  The old test helper did a linear scan per byte. Same name and signature,
  so `robust_tests` and the model tests use it unchanged.
- **Writer** (`mpq/writer.rs`): `Method::Huffman { table, pkware }` writes
  COMPRESS sectors with mask 0x01 (Huffman), or with `pkware` mask 0x09
  (Huffman, then PKWARE over the Huffman stream; the reader explodes first).
  Under 0x09 a sector whose Huffman stream is longer than the sector is
  stored raw, because §9 caps every stage's output at `expected(i)`. Also
  `writer::huffman(table, data)` (pub, like `implode`), and
  `WriteError::BadHuffmanTable`. The dict-bits check also covers the PKWARE
  stage of `Huffman`.
- **Decoder** (`huffman::decompress`). Output and errors are unchanged
  (checked below). There are two changes:
  1. Leader lookup (Increment step 1). It was a `HashMap<u32, usize>` with
     four SipHash operations per tree level. Now it is a 256-slot cache,
     `weight % 256 → node`, used only as a hint: `Tree::leader` accepts an
     entry only if that node has weight `w` and its predecessor does not.
     The list is non-increasing, so only the true leader passes that test.
     If the entry fails, the lookup walks back from `n` through its weight
     block. So a stale or colliding entry can cost time but cannot change a
     result. Test: `leader_cache_is_only_a_hint`.
  2. Static fast path. Tables 1–8 change the tree only on an escape. Until
     the first escape the tree stays borrowed from the template (`Cow`, no
     clone), and each code starts with one lookup in a 1024-entry table
     built per template. The lookup resolves up to 10 bits. The window is
     zero-filled past the input, and the bits used are consumed through
     `consume`, so a code that runs past the end fails exactly as the
     bit-by-bit walk did.
  - Tried and dropped: a 32-bit window for the walk with the general tree
    (no measurable gain).
- **Bench** (`benches/formats.rs`, group `mpq_huffman`): reads 256 KiB in
  512-byte sectors:
  - text under tables 0–3;
  - text under table 0 + PKWARE (0x09);
  - small signed deltas (-3..=3, ADPCM-like) under tables 0, 1, 4, 5, 6.

  The bench checks that every case stores compressed sectors, so no case
  measures a raw copy. Text under tables 4–8 and deltas under 2, 7 and 8 do
  not shrink and were left out.
- **Tests** (`mpq/huffman_tests.rs`, new):
  - `huffman_round_trip`: every table, three data shapes, capped and full
    output, equal to the reference model `huffman::tests::Model`.
  - `decoder_matches_the_model`: arbitrary and truncated valid streams; the
    decoder gives the model's output or fails where the model fails.
  - `archive_huffman_round_trip`: all tables, ± PKWARE, sector shifts 0–3,
    single unit, encryption + FIX_KEY, SECTOR_CRC.
  - `huffman_then_adpcm`: masks 0x41 and 0x81 equal ADPCM over the raw
    payload.
  - Fixed cases for every table, checks of the mask counts the writer
    produces, and the writer error cases.

  The existing Huffman tests are unchanged, apart from `check()`: it now
  verifies `leader()` from every node, with and without the cache, in place
  of comparing the old map. The weight-overflow regression no longer edits
  the map.
- Perturbation check (M08), run by hand and not committed:
  - Ignoring the `consume` error of the fast path: caught (truncated
    streams decode where the model fails).
  - Breaking the leader test (`&&` → `||`): the tests never finish (the
    wrong leader breaks the tree and the walk), so they cannot pass.
  - Writing wrong nodes into the cache: tests still pass, as designed (the
    cache is only a hint).

## Numbers

Cloud container, 4 vCPU Xeon 2.10 GHz, rustc 1.99, `cargo bench -p
d2-formats --bench formats -- mpq_huffman --warm-up-time 1
--measurement-time 3`. "Before" is the base decoder with the same encoder
and bench (criterion baseline `before`). The figures are criterion
medians, so treat them as order of magnitude.

| Case (256 KiB, 512-byte sectors) | Before | After | Change |
|---|---|---|---|
| text, table 0 (adaptive) | 74.7 ms, 3.35 MiB/s | 22.2 ms, 11.3 MiB/s | 3.4× |
| text, table 0 + PKWARE (0x09) | 72.4 ms, 3.45 MiB/s | 25.2 ms, 9.9 MiB/s | 2.9× |
| text, table 1 | 6.10 ms, 41 MiB/s | 1.78 ms, 140 MiB/s | 3.4× |
| text, table 2 | 3.82 ms, 65 MiB/s | 1.90 ms, 132 MiB/s | 2.0× |
| text, table 3 | 6.01 ms, 42 MiB/s | 2.03 ms, 123 MiB/s | 3.0× |
| deltas, table 0 | 51.3 ms, 4.9 MiB/s | 13.8 ms, 18 MiB/s | 3.7× |
| deltas, table 1 | 7.43 ms, 34 MiB/s | 2.27 ms, 110 MiB/s | 3.3× |
| deltas, table 4 | 6.22 ms, 40 MiB/s | 5.97 ms, 42 MiB/s | ≈ (escapes early) |
| deltas, table 5 | 9.31 ms, 27 MiB/s | 7.39 ms, 34 MiB/s | 1.26× |
| deltas, table 6 | 7.36 ms, 34 MiB/s | 7.27 ms, 34 MiB/s | ≈ (escapes early) |

The BB1 report of about 3 MiB/s reproduces: it is table 0. The cause was
not the tree being rebuilt per sector. The template is built once, and
cloning it costs 30 ns for table 0 and 1.2 µs for the 515-node tables.
The cost was the adaptive update: one Increment per tree level per
symbol, each doing about four SipHash map operations. Table 0's tree
stays shallow, about 4.4 bits per symbol on text.

What is left:
- Table 0 now runs at about 4 ns per tree level, mostly pointer chasing.
- On static tables, a stream falls back to the bit-by-bit walk (about
  12 µs per 512 bytes) after its first escape.

Possible next steps, not done: a compact node layout with u32 links (the
nodes are 72 bytes now), and rebuilding the fast table after an escape.
Which steps are worth it depends on which tables the 1.14d `.wav` files
use (see the queue below).

## Gate

`sh tools/gate.sh --no-client` passes (fmt, clippy -D warnings, tests: 1876 + 607 run, all pass; coverage --check, depcheck). `d2-client` is not touched and does not use `writer::Method`; the client half of the gate was not run (Bevy build, disk).
coverage --check, depcheck. Results are in the session summary. No
`d2-client` change.

## Local run queue (for the coordinator to fold into HANDOFF §5)

1. **Huffman decoder on game files after the speed-up.**
   - Command: `D2_GAME_DIR=<install> cargo test -p d2-formats -- --ignored`
     and `mpq-tool formats`.
   - Expect: everything that passed before still passes. In particular,
     all 5,008 Huffman + ADPCM `.wav` files decode to their exact RIFF size
     (mpq.md Observations). The decoder is held to the reference model by
     property tests, but no game-file sector has run through the new code.
2. **Which weight tables the `.wav` sectors use** (decides whether further
   decoder work pays).
   - How: count byte 0 of the Huffman stage over the 0x41 / 0x81 sectors.
     No tool prints this today; add a `--huffman-tables` count to
     `mpq-tool formats`, or use a one-off test.
   - Also record: how many of those sectors contain an escape.

## Open questions

None for the spec. The encoder's escape order and its double increment
follow §11 exactly, and the decoder reads the encoder's streams.
