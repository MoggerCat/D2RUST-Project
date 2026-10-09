# q-tool-orig-cache: shared cache of recorded 1.14d sides

Done (REC-1590..1594, none provisional)
- `tools/scenario-diff/orig_cache.py`: key, store, lookup, selftest. Spec: `specs/tools/scenario-diff.md` §4 rule 3b.
- `--orig-cache [DIR]` (default `traces/orig-cache`), `--fill-cache`, `--cache-no-read` on `scenario_diff.py`; `--orig-cache`/`--fill-cache` on `suite.py` (`--fresh` never reads the cache).
- Key per check x channel: sha256 of the check file, the `--time 1` save, `Game.exe`, the private repo's `install/manifest.json`, and the channel's recorder script plus the same-folder modules it imports. Any change is a miss (selftest: changed check, save, recorder module, exe).
- Filled on claude/integ-r4 for all 89 checks, 109 entries (state 85, rng 19, packets 5, draws 1... see `find traces/orig-cache -name cache.json | wc -l`). 120 MB of text, mostly state jsonl (largest 7 MB). Text only; PNG/binary is refused by the store.

Use (no Wine needed, only the game dir for the key's Game.exe hash and the private repo for the manifest hash)
    D2_GAME_DIR=$HOME/game python3 tools/scenario-diff/scenario_diff.py traces/checks/<n>.check --orig-cache
    python3 tools/scenario-diff/suite.py --filter <glob> --orig-cache --no-playthrough
A miss records 1.14d as before; add `--fill-cache` to store it. After changing a check or recorder, re-fill: `--orig-cache --fill-cache`, commit `traces/orig-cache/<check>/`.

Open
- A recorder change invalidates only the channels whose recorder imports it, but any d2s-tool change that alters the `--time 1` save misses everything of that check.
- Entries were recorded with d2rs rng binary missing on the first pass; the 1.14d side was unaffected.
- Repo size: refill commits of large state files add up; consider pruning old entries if it grows.
