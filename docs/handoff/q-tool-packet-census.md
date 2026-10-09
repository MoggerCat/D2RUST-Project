# q-tool-packet-census: hand-back

## Done
- `tools/packet-census/packet_census.py` (`--selftest` green): `run` records every check with the packets channel only, `report` pairs 1.14d and d2rs records per window and stream like `packets_diff.diff` and counts per (stream, id).
- All 89 checks recorded on both sides (traces/raw/census, gitignored; ~10 min first build, then the rest).
- `docs/handoff/packet-census.md` / `.tsv`: per id, checks carrying it, records, equal / diverged pairs, first equal / first diverged.
  - C->S: EQUAL 10, DIVERGED 4, not carried 99.
  - S->C: EQUAL 19, DIVERGED 41, carried but never paired 15, not carried 106.
- `docs/handoff/ledger/q-tool-packet-census.tsv`: the 294 net.<c2s|s2c>.0xNN rows with state from the data: DIVERGED 45, EQUAL 29, NO-CHECK 179, NOT-IMPLEMENTED 41 (validated with ledger.py from claude/q-fidelity-ledger: 0 format errors). It duplicates the areas of the systems part; the merge keeps the first part in name order, so the coordinator should drop those rows from systems.tsv when merging.
- The last section of the census lists the uncarried ids per stream with the check shape that would carry them (a `send` line per C->S id after the poke that opens its gate; a reply or poke for S->C), for check-gen.

## Open
- "Carried but never paired" (15 S->C ids): the id only appears after a divergence in its window or after the last compared tick, so no equal/diverged verdict; they stay NO-CHECK.
- Divergences found are routed, not fixed. First ones per id are in the part file notes (e.g. net.c2s.0x67 a1-town-arrival-ama@1, net.s2c.0xaa size 12 vs 39 ass-burst-of-speed@2).
- Uncarried ids need new scripted checks (see the census); not built here.

## Repro
```
sh tools/cloud-setup.sh; tools/cloud-game/setup_winpy.sh; tools/cloud-game/prepare_saves.sh   # + assemble the install into $HOME/game, excel view via data-tool excel-dir
python3 tools/packet-census/packet_census.py run
git show origin/claude/q-fidelity-ledger:docs/handoff/fidelity-ledger.tsv > /tmp/ledger.tsv
python3 tools/packet-census/packet_census.py report --md docs/handoff/packet-census.md --tsv docs/handoff/packet-census.tsv --ledger /tmp/ledger.tsv --part docs/handoff/ledger/q-tool-packet-census.tsv
```
