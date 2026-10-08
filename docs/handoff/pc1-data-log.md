# PC 1 data loop log (newest first)

Loop: `docs/handoff/pc1-data.md`. Branch `claude/local-pc1-facts`; private
repo `MoggerCat/D2RUST-private-repo` (`main`).

## 2026-10-08

- Step 2 extract (local, not yet pushed): `mpq-tool` at `1d3a4576` over
  all 11 MPQs into `extracted/<archive>/`: 35095 files, 3066588211 bytes,
  none over 95 MiB; per-folder commands and counts in the private
  `extracted/README.md`.
  - **Failure:** `mpq-tool extract Patch_D2.mpq "*"` →
    `Error: archive has no (listfile)`; `d2sfx.mpq`'s listfile names only
    31 of its 2365 block-table entries. Fix: new `mpq-tool extract-names`
    (commit `1d3a4576`) with a candidate list (private `tools/names.py`):
    d2sfx 2199 files, Patch_D2 114 of 210 entries. The remaining names are
    unknown; `install/` holds both archives whole.
- Step 1 upload: the install (the `game/` folder minus the project's own
  `extracted/`, `renders/`, `patch-example/`, the empty `save/` and the
  game-written `default.key`): **23 files, 2014663095 bytes**, split by
  `tools/split.py` (95 MiB parts), `.gitattributes` `install/** -text` so
  Git keeps bytes exact. `assemble.py --src install`: `0 mismatches`.
  Pushed in 4 batches under 2 GB; the first (≈450 MB) took ≈23 min.
- Disk: 9.3 GB free at start (handoff floor 15 GB); removed the
  rebuildable `target/release` (11 GB) → 20 GB free before the upload.
- Tool: `tools/trace-recorder/facts_render.py` 0.1.0 (capture → `draws.tsv`,
  `frame.tsv`, `sprites.tsv`). frames-raw-2 lacks sprite width / height /
  offsets, unit component cel paths, DT1 file and tile index, and the
  player direction; the outputs list these in a `# missing:` line.
  `sprites.tsv` therefore holds keys only until `record_frames.py` reads
  cel frame headers.
