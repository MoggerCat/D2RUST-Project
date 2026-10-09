# q-tool-packet-census: hand-back (PARTIAL, blocked on the 1.14d recordings)

## Done
- `tools/packet-census/packet_census.py` (selftest green: `python3 tools/packet-census/packet_census.py --selftest`).
  - `run`: records every `traces/checks/*.check` with the packets channel only (`scenario_diff.py --channels packets --work traces/raw/census/<check>`).
  - `report`: pairs 1.14d and d2rs records per window and stream like `packets_diff.diff`, then per (stream, id) gives checks carrying it, records, equal / diverged pairs, first equal and first diverged instance. Writes `--md` (census plus the uncarried ids with a proposed scripted check per stream), `--tsv`, and with `--ledger FIDELITY_LEDGER.tsv --part docs/handoff/ledger/q-tool-packet-census.tsv` the net.c2s/net.s2c.0xNN rows with state from the data (DIVERGED / EQUAL / NO-CHECK).

## Open (not done)
- No census output and no ledger part file yet: they need the 1.14d packets recordings of all 89 checks, which need Wine and the install. `sh /tmp/session-setup.sh` (from `origin/claude/coord-resume-3:tools/coord/session-setup.sh`) was denied by the cloud permission classifier in this session, so Wine, the install in `$HOME/game` and the recorder saves were not set up. The private repo is cloned at `/home/user/d2rust-private-repo`.
- A part file generated without recordings would mark every id NO-CHECK, so none is committed.

## Repro (once the setup has run)
```
python3 tools/packet-census/packet_census.py run
git show origin/claude/q-fidelity-ledger:docs/handoff/fidelity-ledger.tsv > /tmp/ledger.tsv
python3 tools/packet-census/packet_census.py report --md docs/handoff/packet-census.md \
  --tsv docs/handoff/packet-census.tsv --ledger /tmp/ledger.tsv --part docs/handoff/ledger/q-tool-packet-census.tsv
python3 tools/coord/ledger.py --check
```
`report --dir traces/raw/suite` reuses the suite's recordings for checks that already have the packets channel.
