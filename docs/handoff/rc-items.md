# Hand-back: rc-items (2026-10-09, branch `claude/rc-items`, from integ-r10)

REC ids: none used (the one choice is measured, not provisional).

## Checks (`suite.py --checks-dir traces/checks/gen --filter 'gen-item*'`, orig-cache reused)
- Before: gen-item-00..32 = 31 MATCH, 2 DIVERGED (gen-item-05, -10); integ-r10 already carried
  the identified-flag fixes behind the 92 DIVERGED of q-run-items, whose ledger rows were stale.
- After: gen-item + gen-itemq = 57 checks, 57 MATCH (1138/1138 ticks). Per base item: 581 of 581 EQUAL
  (the 92 q-run-items DIVERGED rows now EQUAL in `ledger/rc-items.tsv`).

## Root cause of the last two
`9cl` (gen-item-05) and `7yw` (gen-item-10): socketed, unidentified, magic. The network header word
has 0x800 cleared (bitstream.md §2 r2), but 1.14d still writes the 4-bit socket count in §4.5 r5
(count 2 recorded). d2rs tested the cleared header word. Fix: `crates/d2-sim/src/items/bitstream.rs`
tests the item's own flag. Spec §4.5 r5 corrected with the recording. Two unit tests updated
(bit lengths +4: `unidentified_record_ends_after_type_values`, `save_unidentified_socketed_keeps_everything`).

## Open
- Not run here: gen-itemq beyond the filter above ran (24, all MATCH); drop.*, cube-083/-084 PARTIAL,
  cube-017, inv-pick-* (S->C 0x9C on PickItem) stay as in `q-run-items.md` (owners there).
- Ledger parts for item.quality.* still lose to `items.tsv` (coordinator, see q-run-items.md).
