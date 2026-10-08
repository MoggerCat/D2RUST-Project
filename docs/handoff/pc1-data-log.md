# PC 1 data loop log (newest first)

Loop: `docs/handoff/pc1-data.md`. Branch `claude/local-pc1-facts`; private
repo `MoggerCat/D2RUST-private-repo` (`main`).

## 2026-10-09

- **Step 2 done:** core extract (5 parts, branch `extracted`, merged into
  `main` as `aed059e1`), then the media folders (8 parts) and the
  `extracted: complete` marker on the private `main` (00:21–00:39):
  11 archives, 35095 files, 3066588211 bytes. Patch_D2 and d2sfx are
  partial (see 2026-10-08); both archives are whole in `install/`.
- **Step 1 done:** `install: complete` pushed 00:16 (private `main`
  `6a5b2aa7`): 23 files, 2014663095 bytes. After two more 408s on the
  first ≤250 MB part, every later push took 1–2 min (upload rose from
  ≈0.55 to ≈3–4 MB/s; the cause of the earlier limit is unknown).
- Next (PC 1, later): Step 3 (rendering facts); the seven `Fact*`
  characters are ready.

- **Failure (00:03):** install batch 3 (d2exp + d2music, ≈595 MB) and
  the core extract (≈680 MB) pushed side by side each failed 3 times:
  `error: RPC failed; HTTP 408 curl 22 The requested URL returned error:
  408` / `send-pack: unexpected disconnect while reading sideband packet`.
  The whole PC uploads at ≈550 KB/s (one or two connections alike), so
  each push ran past GitHub's request timeout. Fix: one push at a time,
  commits of ≤ 250 MB (≈8 min each), each pushed by its hash: the
  install as d2exp, d2music 1/2, 2/2, d2video 1/2, 2/2, `install:
  complete`; then the core extract on branch `extracted` (5 parts),
  to be merged into `main`.

## 2026-10-08

- Plan for tonight (PC 1 closes ≈01:00; upload ≈0.6 MB/s): install
  batches 2–4 + `install: complete` (≈00:00), then the **core** of
  `extracted/` (d2data, d2exp, Patch_D2, d2char, d2sfx; ≈1.07 GB; ≈00:35).
  **Deferred to 2026-10-09:** the extracted media folders (d2music,
  D2xMusic, d2speech, d2xtalk, d2video, D2XVIDEO; ≈2.0 GB; their MPQs are
  whole in `install/`), the `extracted: complete` marker, and Step 3.
  Step 3 is prepared: `facts_render.py`, and seven fixed scene characters
  `FactAma`…`FactAss` (`d2s-tool new --class C --expansion --level 1
  --waypoints all --quests acts=4 --map-seed 1 --time 1760000000`) in the
  PC's save folder.

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
