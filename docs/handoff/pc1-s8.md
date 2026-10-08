# PC 1 session s8 (2026-10-08)

Branch `claude/local-pc1-s8` from `origin/claude/specs-staging-7`
(`docs/handoff/pc1-loop.md`). REC block REC-300..349; none used yet.

## Lane A — provisional points from the binary

| REC | Result | Specs changed | Code | Queue row |
|---|---|---|---|---|
| REC-177 (3) | settled statically (`0x005616A0`–`0x00561AF9`): no requirement recheck or refusal; durability untouched; moves 4→11, 5→12, 11→4, 12→5 (all four leave first); mouse-skill swap sets trade; S→C 0x97, direct 0x23s, then next update 0x9D action 0x17 per item, 0x47, 0x48, two queued 0x23 | `items/inventory-moves.md` §7.25 (new owner), `sim/intents-events.md` §9 r14 + owner table + OQ16 answered, `items/inventory.md` §5.8 pointer | `swap_key.rs` matches (1-byte 0x60); `wiring/inventory/swap.rs` + `handlers/items/moves.rs` do not (gates bypassed at `adapters/sim.rs:485`, no 0x9D 0x17, no skill trade / 0x23, fail result 1 not 3) | `q-fix-weapon-switch` |

## Lane C — recordings PC 2 needs (to move into HANDOFF §7)

- **R-SWAP-1** [MANUAL] Settles: REC-177 (3) message order and fields.
  Expansion character, set 1 sword + shield, set 2 a two-hander that fails
  a requirement, a different left/right skill chosen in each set. Press W
  twice, then once more with both sets empty. Capture S→C in order: 0x97,
  the direct 0x23s, 0x9D action 0x17 ×n, 0x47, 0x48, the two queued 0x23;
  the 0x23 fields and the 0x9D bit streams (body location, flag 0x4000 on
  the unusable two-hander).
