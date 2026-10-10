# rc-checkgen-selftest

Cause: `selftest.py` had no `__main__` (running it directly did nothing, so it "passed"), and
`check_gen.py --selftest` failed two ways: synthetic monstats lacked `Skill1..8` (fam_monskill),
and the write/--check step used the repo's real client-messages/census instead of the synthetic
ones (no `gen-netc2s-02`).
Fix (tools/check-gen/selftest.py only): synthetic Skill columns; pass `--client-messages`/`--census`
to the main() calls; `__main__` entry; assert every FAMILIES entry generates checks or is in an
explicit skip list (mon, monskill: need real ledger rows).
Result: both entry points run 241 assertions, no game files needed.
