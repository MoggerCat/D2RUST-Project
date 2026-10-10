# rc-item-reader hand-back

Branch claude/rc-item-reader (from claude/rc-items 3f2b34259 + specs-staging-7).

## Cause
1.14d's reader `0x0062CBE0` reads the socket count only when the **stored
flags** (the wire header word, read by `0x0062E430`) have 0x800. The writer
sends the count for unidentified socketed items although the header has
0x800 cleared, so 1.14d's own reader never reads those 4 bits: it jumps to
the end of the record and ignores the rest of the packet.

## Changed
- `d2_proto::item_bits::decode`: the record is read as 1.14d reads it
  (`sockets` stays `None`, gate unchanged: header F & SOCKETED). For a full,
  non-save, unidentified record without 0x800 and with leftover that is
  not clean padding, it skips the ignored `Save Bits`(194) bits before the
  zero-padding check and sets `bits` accordingly.
- `specs/items/bitstream.md` §4.5 rule 5: reader-side behaviour + address.
- Proptest `prop_item_bits` unchanged.

## Checks
- `cargo nextest run -p d2-proto -p d2-server`: 477/477 pass (was 1 fail).
- `cargo nextest run -p d2-sim items`: 475/475 pass.
- fmt, clippy -D warnings, coverage, spec_index, ledger --check: clean.
- gen-item* 1.14d suite NOT re-run: the writer (d2-sim) is untouched, the
  change is reader-only in d2-proto, so rc-items' 57/57 MATCH is unaffected.
  Re-run on the next Wine session if wanted.

## Open
- Leftover of 4..7 bits on an unidentified record is accepted either way
  (indistinguishable from padding); size: none.
