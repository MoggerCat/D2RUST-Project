# fix-sweep-counts (branch `claude/fix-sweep-counts`)

## Finding

The enumeration was already the same as `mpq-tool formats`' (listfile union
plus `EXTRA_NAMES`, case/`/`-insensitive since `e3025503`), but as two
copies. The 2,456 DS1 and the 110,259 `0x1001` blocks come from the
**old case-sensitive `mpq-tool formats`, which counted some files twice**
(`docs/handoff/triage-game-findings.md` rows for `dt1`/`ds1`;
`local-buddy-2026-10-06.md` G1). Nothing here measures them with the fixed
tool. So the fix is not an enumeration change that can restore 2,456, and
no expected number was changed (no measured source).

DT1: the 250 live / 6 version-4 split is measured (spec session `6b8dc11`
and the sweep on two PCs). The 1,354 missing `0x1001` blocks are plausibly
the 4 double-counted DT1 files, but that is a guess: the corrected block
counts were never recorded. `camera.md` OQ7's 250 vs 251 is about a
different question (live files), and no name is missing from the sweep that
the tool would find, since both now share one name source.

## Change

- New `d2_formats::mpq::names` (`EXTRA_NAMES`, `name_key`, `name_set`,
  `known_names`), moved out of `tools/mpq-tool/src/formats.rs`.
- `mpq-tool formats` and `game_sweep.rs` both call `known_names`; the copied
  `EXTRA_NAMES` in the sweep is gone, so the scopes cannot drift.
- Unit tests moved with the code (`name_set` case/separator test) plus
  `extra_names_survive_without_listfiles`.
- Expected counts untouched (2,456 DS1 / 1,997 v18, DT1 block formats).

## Local rerun

1. Measure the expected values with the fixed tool (the missing source):
   `D2_GAME_DIR=<install> cargo run --release -p mpq-tool -- formats <install>`
   (use the CLI form `mpq-tool formats` takes). Record the `.ds1` total
   and, for `.dt1`, the file count; compare to the sweep's `ds1:` line
   (2,372 files, 1,926 at v18) and `dt1:` line (250 live, formats
   `{1: 226,996, 4097: 108,905, 8197: 15,712}`).
2. `cargo test --release -p d2-formats --test game_sweep -- --ignored --nocapture`

Expected: with the numbers still unchanged, `ds1_every_file_parses` and
`dt1_every_live_file_decodes` **still fail on the counts** until step 1
confirms the fixed tool prints 2,372 / 1,926 and the DT1 block counts above.
Then update the asserts, `ds1.md`/`dt1.md` Status and `PLAN.md` from those
printed values. If the fixed tool prints 2,456, the sweep is missing 84
names and the difference is in `known_names` (compare name lists).
