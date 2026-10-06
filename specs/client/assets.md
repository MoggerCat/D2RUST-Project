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

The budgets are guesses until measured: the local run queue gets
"decoded size of every live DCC/DC6/DT1 and of a full town scene" (§Open
questions 1).

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

1. Budgets §A5: measure the decoded size of the live graphics set and of
   a busy scene (local, `mpq-tool` extension), then set defaults.
2. §B1–§B3.
3. Whether `d2-data` tables needed by the client (e.g. item graphics
   names) come through the bridge snapshot or a read-only client-side
   `d2-data` load: decide with the Phase 5 bridge design
   (`claude/phase5-bridge`).
