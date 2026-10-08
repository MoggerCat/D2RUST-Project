# PC 1 session s8 (2026-10-08)

Branch `claude/local-pc1-s8` from `origin/claude/specs-staging-7`
(`docs/handoff/pc1-loop.md`). REC block REC-300..349; none used yet.

## Lane A — provisional points from the binary

| REC | Result | Specs changed | Code | Queue row |
|---|---|---|---|---|
| REC-177 (3) | settled statically (`0x005616A0`–`0x00561AF9`): no requirement recheck or refusal; durability untouched; moves 4→11, 5→12, 11→4, 12→5 (all four leave first); mouse-skill swap sets trade; S→C 0x97, direct 0x23s, then next update 0x9D action 0x17 per item, 0x47, 0x48, two queued 0x23 | `items/inventory-moves.md` §7.25 (new owner), `sim/intents-events.md` §9 r14 + owner table + OQ16 answered, `items/inventory.md` §5.8 pointer | `swap_key.rs` matches (1-byte 0x60); `wiring/inventory/swap.rs` + `handlers/items/moves.rs` do not (gates bypassed at `adapters/sim.rs:485`, no 0x9D 0x17, no skill trade / 0x23, fail result 1 not 3) | `q-fix-weapon-switch` |
| REC-188 | settled: 0x1D–0x1F base only; client sums item lists itself; 0xFE/0xFD pseudo states d2rs-own; set lists are parked state 165+i children, the client runs the set update `0x00663CC0` at 8 sites; no 0xA8 from set lists | `client/stat-lists.md` §2 r1.1, r5, r6, OQ2 answered, new OQ8; `items/bitstream.md` §4.6 r6 | `vitals_sync.rs:172-260` d2rs-own transport (q-item-bonus-wire removes it); `item_bits` decode fine | `q-fix-client-set-lists` |

Follow-ups for a later worker (other owners): `client/msg-stats-items.md` §5 r4 (0x7D) and the 0x92 rule have the (1,1)/(0,0) labels swapped ((1,1) = removed / owner set list freed); `items/properties.md` §13 caller list should name the 7 other client sites (stat-lists §2 r5); `items/inventory.md` OQ1 and `client/msg-stats-items.md` OQ3 can point to stat-lists §2 r1.1.

## Lane C — recordings PC 2 needs (to move into HANDOFF §7)

- **R-SWAP-1** [MANUAL] Settles: REC-177 (3) message order and fields.
  Expansion character, set 1 sword + shield, set 2 a two-hander that fails
  a requirement, a different left/right skill chosen in each set. Press W
  twice, then once more with both sets empty. Capture S→C in order: 0x97,
  the direct 0x23s, 0x9D action 0x17 ×n, 0x47, 0x48, the two queued 0x23;
  the 0x23 fields and the 0x9D bit streams (body location, flag 0x4000 on
  the unusable two-hander).
- **R-SET-1** [MANUAL] Settles: REC-188 (stat-lists OQ8). Equip a
  two-hander (action 0x07) over a worn set shield while a partial set bonus
  shows; record 0x9C/0x9D and what follows, then read the character panel:
  does anything refresh the taken-off item's set list?
- **R-SET-2** [MANUAL] Settles: REC-188 (stat-lists §2 r6, set test
  vectors). Equip 2 then 3 pieces of one set, unequip one; record the
  0x9C/0x9D streams (set mask, lists), confirm no 0xA8 for states 165–170
  and base-only 0x1D–0x1F; compare panel totals with base + item lists +
  client-computed set bonuses.
