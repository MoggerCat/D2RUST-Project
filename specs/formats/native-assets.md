# Spec: Formats — Native assets (converted locally from the player's install)

- **Status:** draft: d2rs-own design (`docs/PLAN.md` decisions log,
  "Native assets (converted locally)"); no code, no check run yet. Every
  native format here is ours; the original formats it reproduces are owned
  by the specs it links to.
- **Target version:** 1.14d (the source install); the native formats
  themselves are d2rs-own, so this spec holds no `Game.exe` addresses.
- **Crate/module:** `d2-native` (new: native readers and writers, manifest,
  no Bevy), `tools/d2-convert` (new: the converter binary), the native
  `FileSource` in `d2-client::assets` (§5), the table source switch in
  `d2-data` (§5.3).
- **Related specs:** `formats/mpq.md` (archive set, listfiles),
  `data/loading.md` (archive search order §2, `.bin` choice §3, txt/bin
  cross-check §11), `data/txt-format.md`, `data/field-types.md` (§10 and
  OQ 7: monstats record 707), `data/patch-layers.md` (table patches),
  `client/assets.md` (§A1 canonical paths, §A5 pools), `client/audio.md`
  (§A1 decode path), and one format spec per kind (§1 table).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 43–58 |
| Inputs | 59–67 |
| Outputs / state changes | 68–73 |
| Rules | 74–75 |
|   1. Scope and inventory | 76–125 |
|   2. Native formats | 126–341 |
|   3. Folder layout and manifest | 342–413 |
|   4. Converter behaviour | 414–522 |
|   5. Runtime switch | 523–558 |
|   6. Mod layering | 559–585 |
|   7. Tests and checks | 586–630 |
|   8. Implementation plan | 631–645 |
| Constants & data dependencies | 646–652 |
| Randomness | 653–656 |
| Edge cases & original bugs | 657–667 |
| Test vectors | 668–678 |
| Provenance | 679–689 |
| Open questions | 690–711 |
<!-- /index -->

## Summary

The game never reads the MPQs at run time. A converter (`d2-convert`), run
once on the player's machine, opens the player's own D2 LoD install,
decodes every file d2rs loads with the existing `d2-formats` / `d2-data`
readers, writes each one in a simple, standard, lossless native format
into a local folder (the *native root*), reads every written file back and
compares it with the original decode. A file whose comparison fails is
reported and recorded as failed, never left as a silent good file. At run
time the client and server load native files only, through a native
`FileSource` and a native table source; a development switch keeps the
MPQ path. Mods add or override native files and patch tables (`d2patch`)
in a defined order above the converted base. Native files are Blizzard
content in a new format: they live outside the repo, are never committed
and never distributed (`CLAUDE.md` rules 1 and 9).

## Inputs

| Name | Type | Source |
|---|---|---|
| Install folder | path | the player picks it (CLI `--game <dir>`; tests: `D2_GAME_DIR`) |
| Language | string, e.g. `ENG` | install default (`data\local\use`, `tbl.md`) or `--lang` |
| Native root | path | §3.4 |
| Converter version | semver + git commit | the `d2-convert` build |

## Outputs / state changes

The native root (§3): one or more native files per converted source file,
`manifest.toml`, `files.tsv`, `report.txt`. Nothing is written inside the
install folder, and nothing outside the install folder is read.

## Rules

### 1. Scope and inventory

1. **Converted set.** A source file is converted when a d2rs reader loads
   it today: every kind in the §1.1 table. A file of an unlisted kind
   (`.bik`, `.smk`, `.xls`, `.mpq` data with no reader) is not converted;
   the report lists its count per extension.
2. **Which copy.** Only the copy the archive set returns (search order:
   `loading.md` §2) is converted; shadowed copies (the 41 older X `.bin`
   copies of `loading.md` §11 and the like) are never loaded by the game
   and are skipped. Excel files resolve P → X → D as `loading.md` §2 says.
3. **Names.** MPQ archives hold names only in `(listfile)` (`mpq.md` §14;
   `patch_d2.mpq` has none). The name set is the union of: every archive's
   listfile; the extra-name list `mpq-tool formats` already keeps (names
   missing from every listfile); and every path a converted file names
   (DS1 file lists, COF and component names from the tables, sound file
   names from `sounds.txt`, the palette and font paths the client builds).
   The last set is closed under repetition (a newly found DS1 adds its DT1
   names) until no new name appears. Every name is folded to its canonical
   path (`client/assets.md` §A1) before lookup; two spellings are one file.
4. **Unnamed blocks.** Hash-table entries no name reaches are counted per
   archive in the report and not converted. A runtime read of a path that
   has no native file is a load error (§5 r4), never an MPQ read.

#### 1.1 Inventory and native targets

Paths are canonical (lowercase, `/`). `P` is the canonical source path;
native file names append to it (§3.2). Counts are the live 1.14d counts
of the owning spec (all copies, all archives); the converter's counts are
the winning copies only and go in the manifest.

| Kind | Source (live count, owner) | Read today by | Native target | Check (§4.3) |
|---|---|---|---|---|
| `dc6` | `.dc6` sprites (1,657 files, `formats/dc6.md`) | `Dc6Loader`, `panel_art`, UI, items, fonts | `P.png` sheet + `P.toml` (§2.2) | C-DC6 |
| `dcc` | `.dcc` animations (21,717, `formats/dcc.md`) | `DccLoader`, `unit_assets` | `P.png` sheet + `P.toml` (§2.2) | C-DCC |
| `cof` | `.cof` (3,605, `formats/cof.md`) | `CofLoader`, `unit_assets` | `P.toml` (§2.4) | C-STRUCT |
| `dt1` | `.dt1` tiles (254, `formats/dt1.md`) | `Dt1Loader`, `tile_assets`, server `world_data` | `P.toml` + `P.d/<n>.png` per tile (§2.3) | C-DT1 |
| `ds1` | `.ds1` presets (2,456, `formats/ds1.md`) | `Ds1Loader`, server `world_data`, DRLG | `P.toml` (§2.3) | C-STRUCT |
| `pal` | `pal.dat` palettes (19, `formats/palette.md`) | `PaletteLoader` (`.dat`), render | `P.pal` JASC-PAL (§2.5) | C-STRUCT |
| `pl2` | `.pl2` palette transforms (17, `formats/palette.md`) | `Pl2Loader`, `tile_assets`, shading | `P.toml` + `P.png` map rows (§2.5) | C-STRUCT |
| `tbl` | string tables `data/local/lng/*/*.tbl` (29 copies, `formats/tbl.md`) | `d2-data::strings`, `TblAsset` | `P.tsv` + `P.toml` (§2.6) | C-TBL |
| `font` | font tables `data/local/font/*/*.tbl` (14, `formats/font-tbl.md`) | `ui` text, `FontMeasure` | `P.toml` (§2.4) | C-STRUCT |
| `wav` | `.wav` sounds, speech, music (4,992, `formats/wav.md`) | `SoundPool`, `D2Wav` | `P` as canonical PCM WAV (§2.7) | C-WAV |
| `excel` | `data/global/excel/*.txt`: the 73 runtime tables' highest-priority `.txt` (`loading.md` §11) plus `sounds.txt`, `soundenviron.txt` (§3.4 there) | `d2-data::bin`, `compile`, sound table | `P` verbatim TSV (§2.8) | C-TABLE |
| `animdata` | `data/global/animdata.d2` (1, `formats/animdata.md`) | `d2-formats::animdata`, fix-ups | `P.tsv` (§2.9) | C-STRUCT |
| `expfield` | `data/global/expfield.d2` (1, `formats/animdata.md` §expfield.d2) | path code (`d2-sim::path`) | `P.png` + `P.toml` (§2.9) | C-STRUCT |

The `.bin` files are not converted: they are the reference the C-TABLE
check compiles against (§4.3), and the runtime compiles the native `.txt`
instead (§5.3).

### 2. Native formats

#### 2.1 Common rules

1. **Choice.** Every native format is a standard one a modder can open
   with common tools (PNG, WAV, TOML, TSV, JASC-PAL), or plain text over
   one. No d2rs-own binary format.
2. **Indices, not colours.** Images store palette **indices**, never RGB.
   Every image is a PNG of colour type 3 (indexed), bit depth 8, not
   interlaced. The index of each pixel is the decoded original's index;
   index 0 is transparent in every kind (`dc6.md` §decoding, `dcc.md`,
   `dt1.md` block pixels). The `PLTE` chunk is a viewing aid only: the
   converter writes the act 1 `pal.dat` colours (§2.5) and a `tRNS` chunk
   giving index 0 alpha 0 and every other index 255. Loaders read the
   indices and ignore `PLTE` and `tRNS`; a PNG of any other colour type or
   depth is a load error naming the file (M07).
3. **Sidecars.** Every field of the decoded original that is not pixel or
   sample data goes in a TOML sidecar, under the field names the
   `d2-formats` structs use (D2MOO / community names). The comparison of
   §4.3 is equality of the decoded structs, so a field left out is a
   failed check, not a silent loss.
4. **Derived fields are not stored.** Offsets, lengths, record counts and
   other file-layout fields that the reader computes are not in the
   sidecar; the native reader recomputes them. A field the reader keeps
   but that only describes the original's encoding (DCC `coded_bytes`,
   `outsize_coded`, `pcd_leftover_bits`, `optional_data`, `final_dc6_size`;
   `tbl` hash slots) goes in an optional `[encoding]` table. Runtime code
   never reads `[encoding]`; the check compares it. A mod-authored file
   omits it, and its decoded struct then holds the documented default
   (0 / empty).
5. **Byte strings.** A field that is a byte string in the original (DS1
   file names, `tbl` keys and values, COF-name keys) is stored as a TOML
   or TSV string. `tbl` text is UTF-8 in all 1.14d tables (`tbl.md`); it
   is stored as that text. Any other byte string maps byte `b` to the
   character U+00`b` (Latin-1), which is exact and reversible; a native
   string with a character above U+00FF in such a field is a load error.
6. **Integers.** TOML integers in decimal; fields the format spec writes
   in hex (flags, masks, packed DS1 cells) as strings `"0x…"` with exactly
   8 hex digits for u32, 4 for u16, 2 for u8, lowercase. The reader
   rejects any other spelling (one spelling per value keeps the output
   deterministic, §4.2).
7. **Every sidecar starts with** `native = "<kind>"` and
   `native_version = 1`. The version is per kind; a reader rejects a
   version it does not know.
8. **TSV.** UTF-8, tab-separated, `\n` line ends, one header row, no
   quoting. A cell escapes `\` as `\\`, tab as `\t`, LF as `\n`, CR as
   `\r`; no other escape exists. Excel tables (§2.8) are the exception:
   they are the original bytes, unescaped.

#### 2.2 Sprite sheets (DC6, DCC)

1. **One sheet per file.** `P.png` holds every frame of the file. Row `d`
   (top to bottom) is direction `d`; inside a row, frames go left to right
   in frame order, each at its own width, packed with no gap; each frame's
   top edge is the row's top edge. Row height = the tallest frame of that
   direction; a row whose frames are all 0 × 0 has height 0. Sheet width
   = the widest row; area outside frames is index 0. A file with no
   pixels (all frames 0 × 0) has no `P.png`, and the sidecar says
   `sheet = false`.
2. **Frame rects.** The sidecar lists every frame as
   `[[direction.frame]]` with `rect = [x, y, w, h]` (its place in the
   sheet) plus its fields. DC6 frame fields: `flip`, `offset_x`,
   `offset_y`, `unknown`, `next_block` (`[encoding]`), and `width` /
   `height` (= `w`, `h`). DC6 header: `version`, `flags`, `encoding`,
   `termination` (4 bytes as `"0x…"`), `directions`,
   `frames_per_direction`. DCC: file `version`, `frames_per_direction`,
   `tag`; per direction `compression_flags`, `x_min`, `y_min`, `width`,
   `height`; per frame `variable0`, `x_offset`, `y_offset`, `bottom_up`,
   `x_min`, `y_min`; encoding-only fields per §2.1 r4.
3. **Pixel order.** The sheet holds each frame as the decoder returns it
   (row 0 at the top of `pixels`); `flip` and `bottom_up` stay fields and
   are not applied to the image a second time.
4. **Why a sheet.** One image per file keeps the file count near the
   source count (about 23,400 sheets instead of one PNG per frame) and
   lets a modder edit a whole animation in one image; per-frame rects keep
   each frame exact.

#### 2.3 Levels (DS1, DT1)

1. **DT1 tileset.** `P.toml` lists the file fields (`version`,
   `minor_version`) and every tile as `[[tile]]` with all `Dt1Tile`
   fields except `blocks` (subtile flags as a 25-entry array of
   `"0x…"`), plus `image = "<n>.png"` and `layout`; then one
   `[[tile.block]]` per block with `x`, `y`, `unknown1`, `grid_x`,
   `grid_y`, `format`, `unknown2`. Tile `n` (0-based file order) has its
   image at `P.d/<n>.png`; a tile with no blocks has none.
2. **Tile image, `layout = "assembled"` (default).** The image covers the
   bounding box of the tile's blocks: `x0 = min block x`,
   `y0 = min block y`, right and bottom edges `max(x + 32)` and
   `max(y + h)` with `h` = 15 for format 1 (isometric) and 32 otherwise;
   the sidecar records `origin = [x0, y0]`. Each block's pixels are drawn
   at `(x − x0, y − y0)`; only the pixels the block's shape covers are
   drawn (format 1: the `SKIP` / `RUN` diamond of `dt1.md`; RLE: the
   pixels the decoder wrote, i.e. non-zero ones). Reading back, block
   pixels are taken from the same rectangle and shape; a format-1 pixel
   outside the diamond and an RLE pixel the image has as 0 are 0.
3. **Fallback `layout = "blocks"`.** When the assembled image does not
   read back to the decoded blocks (two blocks write the same pixel with
   different indices, or an RLE block's rectangle picks up a neighbour's
   pixel), the converter writes that tile as a strip instead: block `k`
   in the cell at `(0, 32k)`, 32 × 32, drawn the same way. The report
   counts fallback tiles per file. Both layouts are exact; the choice is
   deterministic (assembled when its read-back is equal, else blocks).
   PROVISIONAL: assembled first, because it is what a modder wants to edit;
   settled by the fallback count of the first full conversion (§7.2).
4. **DS1 level.** `P.toml` holds every `Ds1` field: header (`version`,
   `width`, `height`, `act`, `tag_type`, `unknown_header`), `files` (DT1
   names, §2.1 r5), the layers, `objects` (`[[object]]` with `kind`, `id`,
   `x`, `y`, `flags`), `unknown_groups`, `[[group]]`, `groups_truncated`,
   `[[path]]` with `[[path.point]]`, and `trailing` (hex string). A layer
   is `[[layer]]` with `kind` (`wall`, `orientation`, `floor`, `shadow`,
   `tag`), its index within the kind, and `rows`: `height + 1` strings,
   each `width + 1` cells of 8 lowercase hex digits separated by one
   space (the packed u32 cell, `ds1.md`). A grid as text diffs line by
   line and edits in any editor.
5. **Why not Tiled (`.tmx` / `.tmj`).** Tiled keeps flip flags in the top
   bits of a tile id and addresses tiles by a global id; DS1 cells use
   those bits for their own fields and address DT1 tiles by (orientation,
   main index, sub index) with rarity. Mapping one onto the other is not
   lossless without custom properties on every cell. A Tiled
   import / export tool over the native TOML is an option for later
   (Open question 2).

#### 2.4 Small structured files (COF, font tables)

`P.toml` with every struct field. COF: header fields, `[[layer]]`
(`component`, `shadow`, `selectable`, `override_translucency`,
`new_translucency`, `weapon_class` as a 4-character string per §2.1 r5), `events` (one integer per frame), `event_padding`,
`draw_order` as rows of `frames × layers` integers per direction. Font
table: header fields and `[[glyph]]` with every `Glyph` field.

#### 2.5 Palettes (`pal.dat`, PL2)

1. **`pal.dat` → `P.pal`**, JASC-PAL text: line 1 `JASC-PAL`, line 2
   `0100`, line 3 `256`, then 256 lines `R G B` in decimal. GIMP, Aseprite
   and Paint Shop Pro read it. The colour order on disk in `pal.dat`
   (`palette.md`) is the reader's business; the native file holds the
   decoded `Palette`.
2. **PL2 → `P.toml` + `P.png`.** The PNG is 256 wide; each row is one
   256-entry colour map, as indices; the rows are the maps of the `Pl2`
   struct in field order (`light_levels`, `inventory_variations`,
   `selected_unit_shift`, `alpha_blend` (outer then inner), …,
   `text_color_shifts`). The sidecar lists each group's name and row range,
   holds `base_palette` as 256 strings `"#rrggbb"` (lowercase hex) and
   `text_colors` the same way. The `PLTE` of `P.png` is `base_palette`.

#### 2.6 String tables (`tbl`)

1. `P.tsv`: header `id	key	value`; one row per element in element
   order (`id` = element number, the string index the game uses), text per
   §2.1 r5 and r8.
2. `P.toml`: `crc`, `version`, `hash_table_size`, `max_tries`; in
   `[encoding]` the hash slot of each element (`slots`, one integer per
   element) and the unused-slot records the decoder keeps.
3. **Hash rebuild.** The native reader rebuilds the hash table by
   inserting the rows in element order with the `tbl.md` hash and probe
   rule. With `[encoding]` present the result must equal the stored slots
   (else load error); without it (a mod's table) the rebuilt slots are
   used. PROVISIONAL: element-order insertion reproduces every live table,
   because the slots are then free to drop from the native file once
   proven; settled by C-TBL on the full conversion (§7.2): the report
   counts tables whose rebuild differs from `slots`.

#### 2.7 Audio (WAV)

1. `P` is a RIFF/WAVE file: `RIFF` size, `WAVE`, one `fmt ` chunk of 16
   bytes with the decoded `format_tag`, `channels`, `rate`, `byte_rate`,
   `block_align`, `bits`, then one `data` chunk with the decoded samples
   as little-endian i16. No other chunk. The samples are the ones the
   `wav.md` decoder returns after the MPQ layer (whose sector ADPCM is
   undone there), so the native file is PCM and lossless.
2. **Why WAV, not FLAC.** WAV needs no codec, every tool reads it, and it
   is the sample format the mixer already takes (`client/audio.md` §A1).
   FLAC would save disk but adds a decoder to the client and a codec to
   the check; it stays an option (Open question 3).

#### 2.8 Excel tables

1. `P` (e.g. `data/global/excel/weapons.txt`) is a byte-for-byte copy of
   the winning source `.txt`. It already is a tab-separated text table,
   the format `txt-format.md` parses and the format modders and
   `d2patch` (`patch-layers.md`) work on. No re-encoding: the bytes are
   raw 8-bit (`txt-format.md`), so a copy is the only exact form.
2. The live `.bin` is not copied. The record bytes the game runs on are
   recomputed from the `.txt` by the `d2-data` compiler (§5.3), which
   `data-tool tables` proves exact for 72 of 73 tables.
3. **The one explained cell.** `monstats` record 707 `NameStr` compiles to
   11154 from text but the live `.bin` holds 5382
   (`field-types.md` §10). Until `field-types.md` Open question 7 decides
   the policy, the converter writes `data/global/excel/_bin-overrides.toml`
   listing, per table, each (record, field) whose live `.bin` bytes differ
   from the compiled ones and that the owning spec lists as explained,
   with the live value; the compiler applies it after compiling. Any other
   difference fails C-TABLE. PROVISIONAL: overrides keep the runtime
   byte-identical to 1.14d; settled by `field-types.md` Open question 7.

#### 2.9 Other data files

1. **`animdata.d2` → `P.tsv`**: header `cof	frames	speed	events`; one
   row per record in file order (bucket by bucket, records in their file
   order), `events` as `frame:code` pairs separated by spaces for each
   non-zero event byte. File order is kept because duplicate names exist
   and lookup returns the first in its bucket (`animdata.md`).
2. **`expfield.d2` → `P.png` + `P.toml`**: an 8-bit indexed PNG of `W × H`
   cells, the cell value as the index (0–8); the sidecar holds the
   unread leading u16 (`header = 266` in 1.14d) and `W`, `H`.

#### 2.10 Higher-resolution and replacement assets (future, not in scope)

Upgraded graphics are a stated future goal, out of scope now. The native
layer leaves room for them without changing anything above: a mod layer
(§6) can hold a file at the same native path with a different kind
suffix (e.g. `P.hd.toml` with RGBA PNGs and a scale), and a renderer that
knows that kind prefers it; one that does not uses the indexed file.
Nothing in §2.1–§2.9 changes for it, and no kind suffix is reserved yet.

### 3. Folder layout and manifest

#### 3.1 Layout

```
<native root>/
  manifest.toml          identity, versions, counts (§3.3)
  files.tsv              one row per source file (§3.3)
  report.txt             the converter's report (§4.5)
  base/                  the converted install (read-only after a run)
    data/global/excel/weapons.txt
    data/global/ui/panel/buysellbtn.dc6.png
    data/global/ui/panel/buysellbtn.dc6.toml
    data/global/tiles/act1/town/floor.dt1.toml
    data/global/tiles/act1/town/floor.dt1.d/0.png
    data/global/sfx/…/x.wav
    …
  mods/                  mod layers, one folder each (§6)
  .work/                 temporary files of a running conversion (§4.6)
```

#### 3.2 Path mapping

1. The native path of a source file is `base/` + its canonical path
   (`client/assets.md` §A1) + the kind's suffix from §1.1: none for `wav`
   and `excel`, `.png` / `.toml` / `.tsv` / `.pal` / `.d/` otherwise.
   Lookup by the canonical path therefore stays one string operation, and
   the original file name stays visible to a modder.
2. Canonical paths are ASCII (`CanonicalPath` refuses others), so native
   paths are portable across Windows, macOS and Linux; lowercase avoids
   case clashes on case-insensitive file systems.

#### 3.3 Manifest

`manifest.toml` (format version from the first commit, `CLAUDE.md`
conventions):

| Key | Meaning |
|---|---|
| `format = "d2rs-native"`, `format_version = 1` | the layout of this spec; a reader rejects an unknown version |
| `converter.version`, `converter.commit` | the `d2-convert` build that wrote `base/` |
| `kinds` | `{ dc6 = 1, dcc = 1, … }`: each kind's `native_version` used |
| `source.language`, `source.lod` | language folder and LoD flag converted |
| `[[source.archive]]` | `name`, `size`, `sha256` of each archive opened; identity only (§4.7) |
| `counts` | per kind: converted, failed, skipped; plus unnamed blocks per archive |
| `complete` | `true` only after a run ends with 0 failed files (§4.5) |
| `files_sha256` | SHA-256 of `files.tsv` |

`files.tsv` header: `path	kind	archive	source_sha256	native	status`.
`path` is canonical; `archive` the winning archive; `source_sha256` the
SHA-256 of the source file's bytes as read from the archive; `native` the
native files as `name:size:sha256` separated by `;`, sorted by name;
`status` is `ok` or `failed:<check>`. Rows sorted by `path` (byte order).
No timestamps anywhere in `manifest.toml` or `files.tsv` (§4.2); the run
date goes in `report.txt`.

#### 3.4 Finding the native root

1. In order: the `--native <dir>` command-line option; the `D2_NATIVE_DIR`
   environment variable; `native_dir` in the d2rs config file; the
   default `<platform data dir>/d2rust/native` (`%APPDATA%` on Windows,
   `~/.local/share` on Linux, `~/Library/Application Support` on macOS).
2. The game starts only if `manifest.toml` exists, its `format_version`
   and every `kinds` version are known to the build, `complete = true`,
   and `converter.version` is at least the build's minimum native version
   (a constant in `d2-native`, raised when a native format changes).
   Otherwise it names the reason and tells the player to run the
   converter; it never falls back to the MPQs in a release build (§5).
3. The game checks `files_sha256` against `files.tsv` at start (one hash
   of a small file). Per-file hashes are checked by `d2-convert verify`
   (§4.4), not at every start.

### 4. Converter behaviour

#### 4.1 Steps

1. Open the install folder with `ArchiveSet::open_dir` (the archives of
   `loading.md` §2; missing optional ones are noted). Read nothing outside
   that folder.
2. Build the name set (§1 r3) and resolve each name to its winning copy.
3. Load the excel tables and string tables first (other kinds' names come
   from them, §1 r3), then every other kind, in canonical path order.
4. For each file, in path order: read the source bytes; decode with the
   `d2-formats` / `d2-data` reader; write the native files into `.work/`;
   read them back with the native reader; compare (§4.3); on success move
   them into `base/`, on failure leave them in `.work/failed/` and record
   the failure (§4.5). Append its `files.tsv` row.
5. Write `manifest.toml` last, with `complete = true` only if no file
   failed, then `report.txt`.

#### 4.2 Determinism

1. Same converter build + same source bytes → identical native bytes,
   `files.tsv` and `manifest.toml`. The converter iterates sorted lists
   only (no `HashMap` order), writes no timestamps or host names outside
   `report.txt`, and fixes every encoder setting.
2. PNG: chunks `IHDR`, `PLTE`, `tRNS`, `IDAT`, `IEND` only; filter type 0
   on every row; one zlib stream at a fixed compression level from a
   pinned encoder crate version. A different encoder version may change
   PNG bytes but not pixels; the converter version in the manifest names
   which wrote them.
3. TOML: keys in the order §2 lists them, one spelling per value (§2.1
   r6), `\n` line ends.
4. Parallelism (§4.6) may change the order work finishes, never the
   output bytes or the row order of `files.tsv`.

#### 4.3 Round-trip checks per kind

Every check is: decode the original with the existing reader → decode
the native file with the native reader → compare. A pass is equality;
anything else is a failure naming the file and the first difference.

| Check | Compared | Pass |
|---|---|---|
| C-DC6 | `Dc6` struct: header, every frame's fields and `pixels` (indices) | equal structs |
| C-DCC | `Dcc` struct: header, every direction's fields, every frame's fields and `pixels`, `[encoding]` fields | equal structs |
| C-DT1 | `Dt1` struct: file fields, every tile's fields, every block's fields and `pixels` | equal structs |
| C-STRUCT | the kind's decoded struct (`Cof`, `Ds1`, `Palette`, `Pl2`, `FontTable`, animdata records, expfield cells and header) | equal structs |
| C-TBL | `StringTable` minus the layout fields `data_start` and `file_size` (`tbl.md`), plus a lookup of every key through the rebuilt hash table returning the original's element | equal; and every key lookup equal |
| C-WAV | `Wav` struct: every `fmt ` field and `samples` | equal structs |
| C-TABLE | native `.txt` bytes vs source `.txt` bytes; then, once per run, compile every runtime table from the native set (with `_bin-overrides.toml`) and compare with each live `.bin` | identical bytes, both steps |

The structs already derive `PartialEq` (`d2-formats`); the check adds no
second notion of equality. Pixel equality is on indices, so a palette
change can never pass as equal.

#### 4.4 `d2-convert verify`

Re-reads every native file under `base/`, checks its size and SHA-256
against `files.tsv`, and, with `--deep`, re-runs the §4.3 checks against
the install. Exit 0 on all pass, 1 otherwise, naming each file.

#### 4.5 Errors and the report

1. **Never silent.** A file whose decode, write, read-back or comparison
   fails is recorded `failed:<check>` in `files.tsv`, its native files
   stay out of `base/`, and the run's `complete` is `false`. The run
   continues with the next file so one report lists every failure.
2. A source file the original reader rejects (`FormatError`) is
   `failed:decode`; the game would fail on it too, so it is not skipped.
3. `report.txt` (human-readable, not parsed): run date, install path,
   per-kind counts, every failure with its first difference, DT1 fallback
   tiles (§2.3 r3), tables with a hash rebuild difference (§2.6 r3),
   unnamed blocks and unconverted extensions (§1 r1, r4), and run time
   per kind.
4. Exit code: 0 complete; 1 one or more failures; 2 the install could not
   be opened as a D2 LoD install (§4.7).

#### 4.6 Resumability and run time

1. A run can stop at any point (crash, Ctrl-C). Files reach `base/` only
   by an atomic rename from `.work/` after their check passes, and their
   `files.tsv` row is appended after the rename, so `base/` never holds a
   half-written or unchecked file.
2. A new run reads the existing `files.tsv`; a row with status `ok`, the
   same `source_sha256`, the same `kind` version and native files whose
   size matches is kept without reconverting; everything else is redone.
   `--force` redoes all. A converter with a different `kinds` version for
   a kind redoes every file of that kind.
3. Work runs on a thread pool, one file per task; results are collected
   and written in path order (§4.2 r4).
4. **Run time is measured, not guessed.** The report records wall time
   per kind and in total; the first full run on the developer PC records
   it in the §7.2 queue entry, and that number becomes this spec's
   statement of expected run time.

#### 4.7 Ownership check

1. The conversion is the "has the game" check of the ownership gate
   (`EARLY_DECISIONS.md` 16, `d2-verify`): the install must open as a
   D2 LoD archive set (`d2data`, `d2exp`, `d2char`, `d2sfx`, `d2speech`,
   `patch_d2`, `d2music`, `d2xmusic`, `d2xtalk`; video archives optional),
   and every file d2rs needs must be present, decode, and round-trip. A
   missing required archive or a required file absent from every archive
   is exit 2 with its name.
2. No hash is compared against a fixed value. The archive hashes in the
   manifest identify *this* install (so a later run can tell the source
   changed and reconvert); they are never checked against a list of known
   releases, since official installs changed over time.
3. Only the chosen install folder is read, plus the native root it writes.

### 5. Runtime switch

1. **Native `FileSource`.** `d2-native` provides `NativeSource`, a
   `FileSource` (`client/assets.md`, `assets/path.rs`) over the native
   root: `read_file(name)` folds `name` to canonical, looks it up in the
   layer stack (§6) and returns the *decoded* asset's bytes in the form
   the existing loader takes. Two options were weighed; the second is
   chosen:
   - re-encode the native file into the original format in memory, so
     loaders stay unchanged (costs an encoder per kind, and those encoders
     are not exact for DCC);
   - **typed loaders**: `FileSource` gains `read_native(path) ->
     Option<Result<NativeAsset, String>>`, where `NativeAsset` is an enum
     of the decoded structs (`Dc6`, `Dcc`, `Dt1`, …). The Bevy loaders
     (`Dc6Loader`, …), `tile_assets`, `unit_assets`, `panel_art`, the
     sound pool and the server's `world_data` reader take the struct
     directly. The MPQ source implements `read_native` by decoding the
     original bytes with the same readers, so every caller has one code
     path for both sources.
2. **Asset source name.** The Bevy source stays `mpq://` during the switch
   (paths are canonical either way); renaming it `d2://` is a separate
   rename task once the MPQ source is development-only.
3. **Tables.** `d2-data` gains a table source: `Mpq` (today: read `.bin`
   from the archive set, `loading.md` §3.1) or `Native` (read the native
   `.txt`, compile with `d2-data::compile`, apply `_bin-overrides.toml`,
   then mod patches, §6). Both produce the same record bytes, which C-TABLE
   proves for the base set. `StringTables::load` and the sound-table
   loader take the same switch.
4. **No silent fallback.** A path with no native file in any layer is a
   load error naming the path (M07, `assets.md` §A1), never an MPQ read.
5. **Development fallback.** A debug build accepts `--source mpq` (or
   `D2_SOURCE=mpq`) to run straight from `D2_GAME_DIR` as today; tests and
   tools keep `ArchiveSet`. A release build has only the native source.
   The default in debug builds is `native` once the converter exists;
   until then it stays `mpq`.

### 6. Mod layering

1. **Layers.** The stack is `base/` (the conversion), then each enabled
   mod folder under `mods/` in the order listed in
   `mods/order.toml` (`order = ["modA", "modB"]`; later wins). A mod
   folder has `mod.toml` (`name`, `version`, `native_format_version`,
   `requires`) and a `files/` tree laid out like `base/` (§3.2).
2. **Files.** A native file in a later layer replaces the same native
   path in an earlier one, whole: a mod's `P.toml` replaces the base
   `P.toml` and, for sheets and tilesets, the mod must supply the
   matching `P.png` / `P.d/` too (a mismatched pair is a load error). A
   mod may add new paths. A mod never removes a base file.
3. **Tables.** Excel `.txt` files are never replaced by a mod (rule 9:
   mods ship patches, not copies). A mod's tables are `d2patch` files
   (`patch-layers.md`) in `files/patches/`, applied in layer order after
   the base tables compile; their stack order is the layer order. A mod
   that ships a `data/global/excel/*.txt` is refused at load with the
   file named. String tables (`tbl`) follow the same rule: a mod adds
   strings through a patch kind `patch-layers.md` will own (Open
   question 4), not by replacing `P.tsv`.
4. **Ruleset.** Layers change data and assets only; behaviour differences
   stay behind `Ruleset::Mod` (`CLAUDE.md` rule 8).
5. **Checks.** A mod layer is not round-trip-checked against the install
   (its files are new content); its files are checked for format at load
   (§2 readers, strict), and `d2-convert verify --mod <name>` checks a
   mod folder against its own `files.tsv` when it ships one.

### 7. Tests and checks

#### 7.1 Unit tests (CI, synthetic fixtures)

Per kind, from the synthetic fixtures the `d2-formats` tests already
build (no game files):

1. Round trip: decode fixture → write native → read native → equal
   struct, for every kind in §1.1, including edge fixtures (0 × 0 DC6
   frames, a DCC direction with no frames, a DT1 tile with 0 blocks, a
   DS1 with `groups_truncated`, a stereo and a mono WAV).
2. Determinism: writing the same struct twice gives identical bytes;
   writing in a different thread order gives an identical `files.tsv`.
3. Perturbation (M08): change one pixel index, one sidecar field, one
   sample, one table byte → the check reports exactly that file and that
   first difference.
4. DT1 fallback: a fixture with two overlapping RLE blocks takes
   `layout = "blocks"` and round-trips.
5. Strict readers: a PNG with colour type 2, a sidecar with an unknown
   `native_version`, a hex value in the wrong spelling, a Latin-1 field
   with a character above U+00FF → load error naming the file.
6. Resume: kill the run after N files (simulated), rerun → same final
   bytes as an uninterrupted run, and no file in `base/` without its row.
7. Layers: a two-mod stack resolves later-wins; a mod `.txt` is refused;
   a sheet without its sidecar is refused.

#### 7.2 Local check (game files)

Queued in `docs/HANDOFF.md` §5 ("Blocked" until `d2-convert` exists):

```
cargo run --release -p d2-convert -- convert --game "$D2_GAME_DIR" --native "$TMP/d2native"
cargo run --release -p d2-convert -- verify --deep --game "$D2_GAME_DIR" --native "$TMP/d2native"
```

Expect: exit 0 both; `manifest.toml` `complete = true`; `counts` with 0
failed in every kind; per-kind converted counts equal to the winning
copies `mpq-tool formats` finds; C-TABLE 73 of 73 runtime tables
identical with exactly one override (`monstats` 707 `NameStr`). Record in
this spec: total and per-kind run time, the native root size, the DT1
fallback tile count (§2.3 r3), the `tbl` rebuild count (§2.6 r3),
unnamed blocks per archive. Then `d2-client play` with `--source native`
reaching the same `verify` and render-case results as with
`--source mpq` (identical pixels).

### 8. Implementation plan

Four sessions; the first three can run in parallel, the fourth after
them. Each owns its files; nobody edits another's.

| Session | Owns | Delivers | Depends on |
|---|---|---|---|
| N1 native formats (images) | `crates/d2-native/` (new crate: `Cargo.toml`, `lib.rs`, `png.rs`, `sheet.rs` (DC6, DCC), `tileset.rs` (DT1), `expfield.rs`, `pal.rs` + PL2) | readers / writers for §2.1–§2.3 images, §2.5, §2.9 r2; §7.1 tests 1–5 for those kinds | `d2-formats` only |
| N2 native formats (text, audio) | in `crates/d2-native/`: `toml_kinds.rs` (COF, font, DS1), `tbl.rs`, `wav.rs`, `animdata.rs`, `tables.rs` (excel copy, `_bin-overrides.toml`) | §2.3 r4, §2.4, §2.6, §2.7, §2.8, §2.9 r1; §7.1 tests 1–5 for those kinds | `d2-formats`, `d2-data`; N1 creates the crate first, or N2 adds its modules to a stub `lib.rs` only it edits below a marked line |
| N3 converter | `tools/d2-convert/` (new) and `crates/d2-native/src/manifest.rs` | §1 name set, §3 layout and manifest, §4 steps, report, resume, verify; §7.1 tests 2, 3, 6; the §7.2 queue entry made runnable | N1, N2 APIs (calls them through `d2-native`'s `NativeAsset`; can start on manifest and name set at once) |
| N4 runtime switch and layers | `crates/d2-client/src/assets.rs`, `assets/path.rs`, `crates/d2-native/src/source.rs`, `crates/d2-native/src/layers.rs`, the table-source switch in `crates/d2-data/src/bin.rs` / `strings.rs`, `crates/d2-server/src/world_data/archive.rs` | §5, §6; §7.1 test 7 | N1–N3 |

`depcheck` must keep `d2-native` free of Bevy (rule 5). Each session
updates this spec's status and `docs/PLAN.md` when its part passes.

## Constants & data dependencies

- `native_version = 1` for every kind; `format_version = 1` for the
  manifest; the build's minimum native version (§3.4 r2).
- Required archives (§4.7 r1); search order (`loading.md` §2).
- DT1 `SKIP` / `RUN` (`dt1.md`), used for the format-1 block shape.

## Randomness

None.

## Edge cases & original bugs

- A source file the original reader rejects is a conversion failure, not a
  skip (§4.5 r2): the original game would fail on it in the same way.
- Names that differ only in case or separator are one file (§1 r3); the
  converter writes it once.
- The 0 × 0 DC6 / DCC frame, the tile with no blocks and the empty DCC
  direction have no pixels and must round-trip with no image (§2.2 r1,
  §2.3 r1).
- `monstats` record 707 `NameStr` (§2.8 r3).

## Test vectors

| Input | Expected | Source |
|---|---|---|
| DC6 with 2 directions × 2 frames of sizes (3×2, 0×0 / 4×1, 2×3) | sheet 6 × 5 (row 0 height 2, row 1 height 3), rects `[0,0,3,2] [3,0,0,0] [0,2,4,1] [4,2,2,3]` | §2.2 r1, synthetic |
| DT1 tile with one format-1 block at (0, 0) | image 32 × 15, `origin = [0, 0]`, the 256 pixels of the `SKIP`/`RUN` diamond hold the block's indices, every other pixel 0 | §2.3 r2, synthetic |
| DT1 tile with two RLE blocks at (0, 0) and (16, 0), both writing column 20 of row 0 with different indices | `layout = "blocks"`, strip 32 × 64 | §2.3 r3, synthetic |
| A pixel index perturbed in one native PNG | C-DC6 fails naming that file, direction, frame, pixel | §7.1 r3, synthetic |
| `wav` mono, 22,050 Hz, samples `[0, 1, -1, 32767, -32768]` | 44-byte header + 10 data bytes, samples equal | §2.7, synthetic |
| The user's 1.14d install | §7.2 expectations | local run (queued) |

## Provenance

d2rs-own design from the user decision of 2026-10-07 (`docs/PLAN.md`
decisions log, "Native assets (converted locally)"). Inventory taken from
the current loaders on branch `claude/spec-native-assets` (base
`2a8084a2`): `d2-client::assets` (loader list, `FileSource`),
`world_view::{tile_assets, unit_assets, panel_art}`, `audio::pool`,
`app::sound`, `d2-data::{bin, strings}`, `d2-server::world_data`. Live
counts quoted from each format spec's status line and `loading.md` §11;
not re-measured here. No `re/` or `../refs/` read.

## Open questions

1. Expected run time and native root size: measured by the §7.2 run, not
   guessed (§4.6 r4).
2. Tiled import / export for DS1 + DT1 over the native TOML: wanted, and
   when? (§2.3 r5). Decision for the user.
   **User decision 2026-10-07: yes**, Tiled import / export is wanted (a later implementation session; not part of N1–N4).
3. FLAC instead of WAV for music and speech to save disk (§2.7 r2):
   settled by the native root size of the §7.2 run. Decision for the user.
   **User decision 2026-10-07: deferred; no sound for now.** Audio kinds (`wav`, music, speech) are not converted or played until the user decides; the converter skips them.
4. How a mod adds or changes strings: a `tbl` patch kind in
   `patch-layers.md` (owner) — not designed yet (§6 r3).
5. DT1 assembled vs block-strip layout: PROVISIONAL assembled, settled by
   the fallback count of the §7.2 run (§2.3 r3).
6. `tbl` hash rebuild from element order: PROVISIONAL, settled by C-TBL
   on the §7.2 run (§2.6 r3).
7. `monstats` 707 override: PROVISIONAL, settled by `field-types.md` Open
   question 7 (§2.8 r3).
8. Whether the shipped mod format may ever carry a non-indexed (RGBA)
   image kind for upgraded graphics: future goal, not in scope (§2.10).
9. Music and speech archives (`d2music`, `d2xmusic`, `d2xtalk`): which
   tracks d2rs loads is `audio/environment.md`'s; until a reader names
   them, every `.wav` reached by a listfile is converted (§1 r3).
