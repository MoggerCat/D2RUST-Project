# Handoff: `bits:` layouts in d2-proto — `claude/proto-bits`

> Not yet folded into `docs/HANDOFF.md`; this file is the detailed record.

Cloud implementation session, 2026-10-06, base `claude/specs-staging`
`c6e40f9`. Repo only (M09). Spec: `specs/sim/intents-events.md` §5
(server-messages.tsv `layout` row: "`bits:` prefix = bit-packed message,
fields `name:width` written LSB-first from bit 0 of byte 0").

## Problem fixed

`cargo run -p data-tool -- gen-proto` failed with
`server-messages.tsv:152: 0x96: bad layout field `bits:`` and d2-proto's
`generated_file_is_current` failed: the TSV gained the `bits:` form
(0x96 WalkVerify) and layouts for 0x0D/0x0F/0x10/0x15 without a
regenerate. Both now pass; `generated.rs` is regenerated.

## What changed (crates/d2-proto only)

- `schema.rs`: `FieldType::Packed { bit, width }` (absolute bit offset
  from bit 0 of byte 0, id byte included, as the spec states; the
  field's `offset` is `None`). `packed_get` / `packed_put`: field bit
  `i` is bit `(bit+i) % 8` of byte `(bit+i) / 8`; put ORs into a zeroed
  buffer and panics on a value wider than the field.
  `PACKED_MAX_WIDTH = 32`.
- `tsv.rs`: `layout()` accepts `bits: name:width ...`. Errors (M07):
  width outside 1..=32 or not plain decimal, any `@`, a token without
  `:width`, an empty `bits:`, a non-fixed size, more bits than
  `8 × size`, and a `bits:` layout in client-messages.tsv (the spec
  defines it for S→C only).
- `codegen.rs`: packed layouts get a typed struct. The `id:8` field at
  bit 0 is the message id, so the struct leaves it out (decode checks it,
  encode writes it, like every typed message); a `bits:` layout with any
  other field over bits 0..8 gets no struct. Field types: width ≤ 8 → u8,
  ≤ 16 → u16, else u32. 0x96 → `server::WalkVerify { stamina, x, y, dx, dy }`.
- Regenerated `generated.rs`: also new typed `PlayerStop`, `PlayerMove`,
  `PlayerToTarget`, `ReassignPlayer` from layouts already in the TSV.

## Tests

- `tsv::tests::bits_layout_0x96_bit_ranges`: 0x96 parses to
  id 0..8, stamina 8..23, x 23..39, y 39..55, dx 55..63, dy 63..71.
- `tsv::tests::bits_layout_strict_errors` (M08): `dy:10` (73 bits > 72)
  rejected, `dy:9` (72) accepted; size 8 rejected; widths 0, 33, `08`;
  `@`, missing width, empty `bits:`, non-fixed size, client table.
- `tsv::tests::bits_0x96_round_trip`: a 9-byte message built by hand as
  a little-endian bit stream (`0x96 | stamina<<8 | x<<23 | ...`, bytes
  `96 BC 5A 1A 89 77 DF 3F 40`) equals `packed_put` output, decodes with
  `packed_get` and `WalkVerify::decode`, and re-encodes; bit 71 is ignored.
- `codegen::tests::packed_struct_fields`; `prop_messages` covers the five
  new typed structs.

## Open / for the coordinator

- No consumer changes: `d2-server` / `d2-client` matches on `FieldType`
  all have wildcard arms; code that reads `Field::offset` skips packed
  fields (offset `None`).
- Whether 1.14d's builder clears bit 71 is not in the spec; the typed
  encode writes 0 there (as for every unlisted bit).
- No `PROTOCOL_VERSION` bump: the wire bytes of 0x96 are unchanged; only
  typed access was added.
