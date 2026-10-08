# Spec: Client — Asset loading, caching and memory budget

- **Status:** draft; d2rs-own design draft (2026-10-06, architecture
  session). Part (a) is our design; part (b) lists original behavior it
  depends on (owner specs to be written locally). Builds on the Phase 1b
  `mpq://` source (`crates/d2-client/src/assets.rs`), which exists.
- **Target version:** 1.14d
- **Crate/module:** `d2-client::assets` (Bevy source and loaders),
  `d2-client::assets::cache` (residency, budgets; plain Rust core)
- **Related specs:** `formats/mpq.md` (Archive set), `data/loading.md` §2
  (archive search order), `client/render-pipeline.md` §A2–A3 (frames,
  atlas), `client/audio.md`, `client/ui.md`

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 31–38 |
| Inputs | 39–46 |
| Outputs / state changes | 47–50 |
| Rules | 51–52 |
|   A. d2rs design (ours) | 53–199 |
|   B. Original behavior to reproduce (not specified here) | 200–207 |
| Constants & data dependencies | 208–211 |
| Randomness | 212–215 |
| Edge cases & original bugs | 216–222 |
| Test vectors | 223–235 |
| Provenance | 236–241 |
| Open questions | 242–253 |
<!-- /index -->

## Summary

All game assets are read at runtime from the user's own archives through
one `ArchiveSet` and the Bevy asset source `mpq://`. Decoded data lives
only in memory, under fixed byte budgets with deterministic eviction. The
client never writes a game file, decoded or not, anywhere outside `game/`;
in this design it writes none at all.

## Inputs

| Name | Type | Source |
|---|---|---|
| archive set | `d2_formats::mpq::ArchiveSet` | opened from `D2_GAME_DIR` in `data/loading.md` §2 order |
| asset requests | archive paths | render, UI, audio |
| budgets | byte limits | `ClientConfig` (ours, §A5) |

## Outputs / state changes

Bevy asset handles and atlas slots for decoded assets; no files.

## Rules

### A. d2rs design (ours)

#### A1. Paths and identity

- The archive set is opened once at startup; its open order is
  `data/loading.md` §2 and `ArchiveSet` resolves names (owned there).
- Asset paths are `mpq://` + the archive path with `\` → `/`.
- **Canonical form:** lowercase ASCII, `/` separators, no leading `/`.
  MPQ names are case-insensitive (`mpq.md` §3), Bevy paths are not, so
  every path is canonicalized before it reaches the `AssetServer`. Two
  spellings of one file give one handle. A test asserts
  `asset_path("DATA\\Global\\X.DC6") == asset_path("data\\global\\x.dc6")`.
- A missing file is an error naming the path (M07); there is no fallback
  asset.

#### A2. Loaders

One Bevy `AssetLoader` per format, each a thin wrapper over the
`d2-formats` parser (no logic in the loader):

| Extension | Asset | Parser | Exists |
|---|---|---|---|
| `ds1`, `dt1`, `dat`, `dc6`, `dcc` | `*Asset` | `d2-formats` | yes (1b) |
| `pl2` | `Pl2Asset` | `palette::Pl2` | to add |
| `cof` | `CofAsset` | `cof::Cof` | to add |
| `tbl` | `TblAsset` = `Font(FontTable)` or `Strings(..)`, chosen by `FontTable::is_font_table` (`font-tbl.md` "Woo!" magic) | `font`, `tbl` | to add |
| `wav` | `SoundAsset` (decoded i16 samples) | `wav` parser, `client/audio.md` §B1 | waits for `formats/wav.md` |
| `txt` (runtime: `sounds.txt`, `soundenviron.txt`) | via `d2-data` reader | `data/loading.md` §3.4 | to add with audio |

Excel tables are not client assets: the client gets what it needs from
the server snapshot or from `d2-data` loaded once (read-only), never by
loading `.bin` itself through Bevy.

#### A3. Derived assets

Parsing gives a file; drawing needs frames. Derived assets are keyed by
`(path, direction)` for DCC/DC6 and `(path, tile)` for DT1:

```
FrameSetKey { path: CanonicalPath, dir: u8 }   ->  FrameSet { frames: Vec<IndexFrame> }
```

DCC directions are decoded per direction (the parser's output is per
direction, `dcc.md`), so a unit facing one way does not decode 31 other
directions. FrameSets are built in plain Rust (`assets::cache`) and
uploaded to the atlas (`render-pipeline.md` §A2) by the render stage.

#### A4. Residency

Draw correctness comes before smoothness (CLAUDE.md rule 10): a frame is
never presented with a missing sprite.

1. Stage 1 of the render pipeline lists every `FrameSetKey` it uses.
2. **Resident** keys draw. **Missing** keys are loaded **synchronously**
   on the main thread (MPQ read + decode + atlas upload) before compose.
   Each synchronous load is counted and logged with its duration
   (`stall` metric); in `verify` any stall is reported in the case result
   (not a failure; the image is still checked).
3. **Prefetch:** when the bridge reports a newly active room or a new
   unit type, its assets are queued for asynchronous load (Bevy task
   pool). Prefetch only changes timing, never the image.

#### A5. Budgets and eviction

| Pool | Default budget | Unit counted |
|---|---|---|
| parsed files (CPU) | 128 MiB | parser output size estimate (`len` of owned buffers) |
| FrameSets (CPU) | 256 MiB | `Σ width × height` + headers |
| atlas pages (GPU) | 256 MiB (64 pages of 2048² R8) | whole pages |
| sounds (CPU, `client/audio.md`) | 64 MiB | samples × 2 bytes |

Budgets are fields of `ClientConfig` (a versioned file, §A6). Eviction
is LRU by **last frame used**, ties broken by key order (deterministic,
no hash iteration; `BTreeMap` index). An entry used by the current frame
is never evicted; if the current frame alone exceeds a budget, the
budget is exceeded, logged, and kept until the frame ends (never a
dropped draw). Atlas eviction frees whole pages: the least recently used
page is cleared and its slots invalidated; atlas packing restarts in
that page (deterministic shelf order).

The defaults were set before measuring and the measurement below keeps
them (no change to `ClientConfig`).

**Measured set sizes** (1.14d, 2026-10-08: every distinct `.dcc`, `.dc6`,
`.dt1` name `known_names` lists across `patch_d2`, `d2exp`, `d2data`,
`d2char`, one copy per name, `Patch_D2` > `d2exp` > `d2data` > `d2char`; parsed by `d2-formats`, pixels counted as
bytes = Σ width × height; scratch program on the repo crates, no repo
changes). One FrameSet = one (file, direction).

| Quantity | Measured |
|---|---|
| files parsed | 23,595 (DCC 21,717; DC6 1,633; DT1 245); the 6 known-unused DT1 (`mpq-tool formats`, `formats/dt1.md`) fail to parse and are excluded |
| whole live set, decoded | DCC 2,610,729,000 B (2.43 GiB); DC6 168,604,107 B (161 MiB); DT1 232,942,528 B (222 MiB); about 2.8 GiB: the set cannot be resident, eviction is required |
| DCC directions per file | 1: 2,768 files; 4: 56; 8: 4,361; 16: 14,483; 32: 49 (271,176 FrameSets) |
| DCC FrameSet size | mean 9,627 B; median of per-file mean 4,394 B; p90 39,552 B; p99 276,132 B; largest 1,822,696 B (1.74 MiB) |
| largest whole parsed file | DCC 4,744,474 B (`monsters\th\s1\ths1litdthth.dcc`); DC6 3,190,514 B (`monsters\42\tr\42trlitdthth.dc6`); DT1 6,265,184 B (`tiles\expansion\siege\cliff.dt1`) |
| DC6 | one direction in all 1,633 files: FrameSet = whole file; median 4,940 B, p90 296,730 B |
| largest frame (w × h) | DCC 345 × 324 (`monsters\gt\tr\gttrlita1hth.dcc`); DC6 319 × 256 (`ui\logo\logo.dc6`); DT1 tile image 160 × 864 (`tiles\expansion\siege\cliff.dt1`); DT1 block 32 × 15 or 32 × 32 |
| hero, one mode, all 16 directions, one layer set | 0.4 to 1.6 MiB per mode (sum over layers of the mean layer file, `lit` armor, 7 classes; NU 0.5–0.8, WL 0.5–0.9, TN 0.8–1.6, A1 1.1–1.6) |
| monster token, whole token (all modes, all directions) | 233 tokens: median 1.17 MiB, p90 8.05 MiB, max 45.3 MiB |
| monster token, modes NU WL A1 GH DT only | 219 tokens: median 0.93 MiB, p90 5.78 MiB, max 17.0 MiB |
| act 1 town tile set | 4 DT1 (`fence floor objects trees`) = 4,674,720 B (4.46 MiB) |
| largest tile directory | `tiles\expansion\siege` 23 DT1 = 30,838,784 B (29.4 MiB); next `expansion\town` 18.2 MB, `act1\outdoors` 12.7 MB |
| missiles / overlays / objects / UI panel | all 391 missile files 84.1 MiB; all 385 overlay files 41.5 MiB; 1,761 object files 115.8 MiB; `ui\panel` 61 files 2.95 MiB |

**Scenes** (parsed bytes, whole files, an upper bound on the pixels
actually resident because only the used directions become FrameSets):

| Scene | Estimate |
|---|---|
| act 1 town | tiles 4.5 + hero 5 modes about 6 + town NPC tokens 0.01–3.2 each (about 8 for the encampment's) + `ui\panel` 3.0 = about 22 MiB |
| busy field or dungeon | largest tile directory 29.4 + hero 8 modes about 9 + 8 monster tokens at p90 (8 × 5.8) 46 + missiles/overlays/objects about 20 = about 105 MiB |
| pessimistic | the same with 20 monster tokens at p90: about 175 MiB |

Rules from the numbers:

- `parsed_files` 128 MiB holds the busy scene (105) and any one file
  with room to spare; the pessimistic scene overruns it by about 1.4×,
  which §A5 already handles (logged overrun, never a dropped draw). Keep.
- `frame_sets` 256 MiB is far above any scene: FrameSets hold one
  direction (about 1/16 of a DCC file), so the busy scene's FrameSets are
  under 40 MiB (tiles resident whole, the rest at 1/16).
  Keep; it may be lowered without a visible effect.
- Every frame fits an atlas page with large margin (largest 345 × 324,
  or 160 × 864 for a DT1 tile image, against 2048 × 2048): the packer
  never needs a "frame larger than a page" case.

#### A6. Writes

- The client writes no game data and no derived game data: no decoded
  cache on disk, no extracted files, no screenshots of game art outside
  `game/` (render captures for verify go to `game/renders/` and
  `game/captures/`, as today).
- d2rs's own files (no Blizzard bytes): `ClientConfig` and the controls
  file (`client/ui.md` §A6) in the platform config directory
  (`<config_dir>/d2rs/`), each versioned (M20). Nothing else.
- If a disk cache is ever wanted it is a new decision: it must live
  under `game/cache/`, be versioned, and be keyed by archive content
  hashes. Not part of this design.

#### A7. Threads and determinism

Asset loading never affects simulation (the sim never reads client
assets). Loading order and timing may vary between runs; the rendered
image may not: atlas slot positions are not visible in the output (the
CPU reference ignores the atlas), so packing order cannot change pixels.

### B. Original behavior to reproduce (not specified here)

| # | Behavior | Owner spec (to write) | Measure | Comparison |
|---|---|---|---|---|
| B1 | Which archive a client asset resolves from when several hold it (same rule as data, or a client-specific order; video/sound archives) | `data/loading.md` §2 (extend) | 1.14d file-open calls for graphics and sound | identical archive and bytes per path for a recorded asset list |
| B2 | Locale directory for fonts and local UI art (`data\local\...`, e.g. `latin`) and how it is chosen | `client/ui.md` §B owner (`ui/text.md`) | 1.14d font load path | identical file paths opened |
| B3 | Graphics the original loads per scene (to size prefetch and budgets; optional for fidelity) | measured only | file-open trace of a town and a dungeon | none (informational) |

## Constants & data dependencies

Budgets §A5; atlas page size from `render-pipeline.md` §A2.

## Randomness

None.

## Edge cases & original bugs

- Files present in several archives: resolved by `ArchiveSet` (§B1).
- A `.tbl` with the font magic is a font, otherwise a string table
  (`font-tbl.md`); a font `.tbl` that fails to parse is an error, never
  retried as a string table.

## Test vectors

| Input | Expected output | Source |
|---|---|---|
| `DATA\Global\X.DC6` and `data\global\x.dc6` | the same canonical path and handle | §A1 |
| missing path | error naming the path | §A1 |
| `.tbl` starting `Woo!` | `TblAsset::Font` | §A2 |
| `.tbl` without the magic | `TblAsset::Strings` | §A2 |
| LRU with budget 2 entries, use a, b, c | a evicted | §A5 |
| equal last-use frames | lower key evicted first | §A5 |
| entry used this frame, budget exceeded | not evicted, overrun logged | §A5 |
| a scene rendered twice with different load orders | identical images | §A7 |

## Provenance

Design decided 2026-10-06 (architecture session) from the existing
`assets.rs`, `mpq.md`, `data/loading.md` §2 and CLAUDE.md rule 1. No
original behavior is stated here.

## Open questions

1. ~~Budgets §A5: measure the decoded size of the live graphics set and of
   a busy scene.~~ Answered 2026-10-08 (§A5 "Measured set sizes"): the whole
   set is about 2.8 GiB, a busy scene about 105 MiB parsed, the largest
   frame 345 × 324 (DT1 tile image 160 × 864); the defaults stay.
2. §B1–§B3.
3. Whether `d2-data` tables needed by the client (e.g. item graphics
   names) come through the bridge snapshot or a read-only client-side
   `d2-data` load: decide with the Phase 5 bridge design
   (`claude/phase5-bridge`).
