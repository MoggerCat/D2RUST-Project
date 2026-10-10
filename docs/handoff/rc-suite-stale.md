# rc-suite-stale hand-back

REC ids used: none (REC-2310..2314 unused).

## Fixed
- `tools/scenario-diff/suite.py` reused `traces/raw/suite/*/orig.*` after a
  recorder change (key = check + save + Game.exe only), so verdicts could be
  computed against outdated 1.14d recordings (found by rc-rerun-r10).
- Key is now `suite-key-2`: adds `recorders` = sha256 of the recorder scripts
  of the check's channels (+ imported same-folder modules, via
  `orig_cache.recorder_version`) and `autostart.py`; `items` uses packets'.
  Old keys miss once (one re-record), by design.
- Selftest: recorder script / imported-module change misses, other channel's
  recorder unaffected, `key_matches` rejects a differing recorders digest.
  `suite.py --selftest`: 20 checks passed.
- Spec: specs/tools/scenario-diff.md §3 (key) and the selftest row.

## Not done
- Re-measuring docs/handoff/rc-queue.tsv on r18: needs the Wine/game setup
  (cloud-setup, assemble $HOME/game), which I skipped as the fix is tool-only.
  Existing raw suite dirs on a machine will re-record on next run (key-2 miss).
  Checks before/after (EQUAL counts): not measured.
- No game logic changed; no ledger rows settled (no ledger part written).

Checks run: suite selftest, spec_index --check, coverage --check, ledger --check.
