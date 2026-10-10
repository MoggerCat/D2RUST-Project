# rc-gen-wine168 hand-back

Generator for the Wine-recordable ledger rows with no check yet (rc-pc1-audit list).

## Done
- check_gen.py: new family `npc` (24 `gen-npc-*` talk checks: goto, interact 0x13,
  chat open/close; state + packets); `itemq` checks now carry ledger areas
  (ITEMQ_AREAS): the quality census reaches quality dispatch, affix picks, unique/set
  picks, sockets and ethereal rolls (not forced one by one; stream compared whole).
- Ran 58 checks (24 npc, 24 itemq, 10 gen-lvl for level rows) under Wine, orig-cache filled
  locally (not committed). Results: MATCH 39 / PARTIAL 24 / DIVERGED 29 checks (channels).
- Part `ledger/rc-gen-wine168.tsv`: 56 rows now have checks, verdict and needs_pc1=n
  (items 20+, npc 24, quest gossip 3, level 9). No row EQUAL: every state channel is
  PARTIAL (never counts as equal) or DIVERGED. EQUAL 2689 -> 2689; checked rows +56.

## Causes (not fixed; game code untouched)
- itemq: stream diverges (identified flag 0x10 missing, extra byte in magic/rare/unique
  streams; see item.* rows' notes).
- npc: drehya, tyrael3 DIVERGED at frame 12/7; natalya, tyrael1 at 13 (goto/interact
  differs: NPC not placed or not interactable in d2rs).
- level 55, 69, 94, 104, 106, 110: DIVERGED (state+rng at frame 21-54, DRLG).

## Still NO-CHECK (no scenario can reach, or needs a recorded scenario of size M)
- inv.* (pickup 0x9C), item.props.*, item.runeword/runes-gems, item.gen.forced-requests/
  format0/special-kinds, vendor.identify, cube.*: poke has no item/option for them.
- quest.* (27 rows): no quest scenario drives flags without full play-through.
- object.preset.57x-582, shrine.0/4/5/16: rows never picked by the object init.
- drlg.*, monster.population*: need a population comparator, not a check file.
