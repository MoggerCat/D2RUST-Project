# rc-census-missing: hand-back

EQUAL before -> after: 2080 -> 2080 (no row flipped: the census re-run covered only the 60 `items-drops-*` checks, and a row is EQUAL only when every check carrying it is).

## What was wrong
Census first divergences over the 60 `items-drops-*` checks (packets channel, d2rs vs 1.14d):
`1.14d 9c vs d2rs 69` (item 0x9C before the dead monster's 0x69) was the commonest; then `5a vs 0d` (27) and `8e vs 0d` (29).

## Changed (REC-2810..2811)
- Ground-item 0x9C now goes out at the item's place in the client pass's update-queue walk (newest queued first, `unit-order.md` §6), not in a pass after it. `ActionHooks::item_marks` makes the pass send a host-only mark (`GROUND_ITEM_MARK`); `d2-server` `announce_marked` replaces it with the announcement. Play host only.
- 0x69 (a, b) of a mode message is the path END (`0x00648A40/60`: last computed point, (0,0) for none), not the target +0x10/+0x12 (REC-594, was in the spec, not in code). Two fixtures that encoded the old value were updated.
- Arena kill event `0x0053F720` and the per-client sync `0x0053FC20` / clear `0x0053FAE0` (`wiring/action/arena.rs`): 0x65 PlayerKillCount after a kill. Row from `arena.bin` (`ActionTables::arena`). PROVISIONAL: the other-players loop `0x0053FB90`.
- Player death sends the 0x5A death notice (code 6) and the arena event whenever a killer is present (`vitals.md` §4.8 rule 1.8 corrected: not gated by the ear test). PROVISIONAL: a minion killer is not resolved to its owner.
- Specs: `intents-events.md` §7.6 (order confirmed by recording), arena kinds; `combat/vitals.md` rule 8.

## Result on items-drops-cha-00
First divergence frame 37 -> 73 (then 73: killer GUID, 1.14d 10 vs d2rs 26, an AI/combat difference; frame 96: corpse sequence). Over the 60 checks the `9c vs 69` and `5a vs 0d` groups are gone.

## Open
- `8e CorpseAssign vs 0d` is now the first divergence in ~47 of 60 drops checks (size L): at the player's DD start 1.14d sends 0x8E, 0x0D(9), 0x59, 0x75, 5x 0x20, 0x74, 0xAA (corpse as an added unit), 0x76, 0xA8; d2rs special-cases 0x59 + 0x0D in `dying.rs` and sends the rest never. Fix: let the corpse unit take the normal new-unit add path (`add_messages`, §7.2) in queue order.
- Who kills the player differs (guid 10 vs 26 in cha-00): monster AI/target choice (owner: AI).
- Ledger part `ledger/rc-census-missing.tsv`: only net.s2c.0x65 (NO-CHECK -> DIVERGED). 0x15/0x1b/0xac read DIVERGED in this subset only through cascades; left as in the base.
- Census output is not committed; `traces/orig-cache` was refilled locally and left uncommitted.

## Round 2: 0x8E CorpseAssign vs 0x0D (REC-2812)
- DD start broadcasts 0x8E first; the corpse then goes through the normal new-unit add (0x59 with the owner's name, 0x75 after every 0x59, 0x74 flag 1 owner+corpse, 0xAA with states 7 and 105, the mode function 0x0D, 0x76) instead of the `dying.rs` 0x59 + 0x0D special case. The player's DT/DD 0x0D carries unit byte +0xB0. Corpse fill sends no 0x47/0x48 when nothing moved (PROVISIONAL, with items moved the d2rs refresh stays).
- Result: items-drops-cha-00 matches 1.14d through frame 96 (only the 0x5A killer GUID differs, frame 73). Over the 60 drops checks the `8e vs 0d` group (~47) is gone.
- Next first divergences (60 drops checks): `0c vs 67` MonsterHit before MonsterMove (10), `a9 vs 65` EndState before the kill count (6), `69 vs 4d` (4), then scattered "missing in d2rs" (killer attribution, AI).
- No ledger row flipped (subset only).
