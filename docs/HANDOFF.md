# Handoff: project state after Phases 0, 1 and 1b (2026-10-05)

Read this first in a new session, then `CLAUDE.md` and `docs/PLAN.md`.

## 1. Where things stand

| Phase | Status | Proof |
|---|---|---|
| 0 Setup | Done, except Git commit/push and CI on GitHub | builds, tests pass |
| 1 Formats | Done | every live file in the 1.14d MPQs decodes (`mpq-tool check`, `mpq-tool formats`) |
| 1b First pixels | Done | Act 1 town renders; GPU output byte-identical to CPU reference (`d2-client verify`) |
| 2 Data tables | **Next** | — |

**The repo has no Git commits yet.** Before a cloud session can use it:
```bash
git config user.name "Your Name"
```
```bash
git config user.email "you@example.com"
```
```bash
git add -A
```
```bash
git commit -m "Phases 0, 1, 1b: workspace, formats, map preview"
```
Then create a GitHub repo and push. The pre-commit hook blocks game files.
`game/`, `re/` and `target/` are gitignored. `../refs/` is outside the repo.

**Cloud sessions have no game files.** Tests that need them are `#[ignore]`
and read `D2_GAME_DIR`. Work that needs real data (surveys, render checks)
must run on a machine with the install, or the user must provide the
specific extracted tables. Never commit extracted Blizzard data.

## 2. Key decisions

- **Target:** D2 LoD **1.14d** (single merged `Game.exe`, version
  1.14.3.71). D2MOO documents 1.10f: use it as a guide and confirm against
  1.14d.
- **Ownership gate:** check that the player *has* the game (required MPQs
  present, valid, containing expected files) plus an online account. No
  exact-hash matching against a Blizzard release, and nothing read outside
  the chosen game folder. `tools/hash-manifest` only records the
  developer's reference install (`traces/reference-install.toml`).
- **Clean room:** specs in `specs/` are the source of truth. Code cites its
  spec (`// Spec: ...`). Implementation never copies decompiler output.
  Reference sources live in `../refs/` (D2MOO MIT, Riiablo Apache-2.0,
  CE_Database, a small 1.14d address list). Constant tables taken from
  Riiablo are credited in `THIRD_PARTY_NOTICES.md`.
- **Parsers are strict.** Odd files are investigated, not tolerated. Unused
  leftovers are listed with reasons; live-data quirks become spec rules.
- **Pins:** Rust 1.99.0 (`rust-toolchain.toml`), Bevy `=0.19.1` (client
  only, enforced by `cargo run -p depcheck`).

## 3. Environment (developer PC, Windows 10)

- Installed: Git 2.55, rustup (Rust 1.99.0), VS 2022 Build Tools (MSVC
  linker), Temurin JDK 21 and 25, Ghidra 12.1.4 at
  `%LOCALAPPDATA%\Programs\ghidra_12.1.4_PUBLIC`.
- `D2_GAME_DIR` = `<repo>\game` (set with `setx`).
- Ghidra project: `re/ghidra/D2_114d.gpr` (`Game.exe` analyzed, 66 community
  labels applied). Exports in `re/exports/` (`functions.tsv` + 13,048
  decompiled functions). Commands are in `tools/ghidra/README.md`.

## 4. Code map

```
crates/d2-formats   MPQ + all Phase 1 formats (no Bevy)
  mpq/              archive, crypto, PKWARE explode, Huffman, ADPCM, ArchiveSet
  palette, dc6, dcc, dt1, ds1, cof, tbl (strings), font (font .tbl)
crates/d2-client    Bevy app (only crate allowed to use Bevy)
  map/              DS1+DT1 assembly, CPU reference renderer (no Bevy types)
  assets.rs         mpq:// asset source + format loaders
  render/           palette material + palette.wgsl
  app.rs            view / verify modes
tools/mpq-tool      info, list, extract, check, formats, render
tools/depcheck      dependency rules (no Bevy outside d2-client)
tools/hash-manifest reference-install record
tools/ghidra        Ghidra scripts (label import, decompiled export)
specs/formats/      mpq, mpq-tables, palette, dc6, dcc, dt1, ds1, cof, tbl, font-tbl
specs/render/       map-preview
```

Useful commands (release builds; game files required):
```bash
cargo run --release -p mpq-tool -- check
```
```bash
cargo run --release -p mpq-tool -- formats
```
```bash
cargo run --release -p d2-client -- verify
```
```bash
cargo run --release -p d2-client -- view
```

## 5. Format facts learned from the 1.14d data

- **MPQ:** format v0, sector size 4096. Masks used: PKWARE (0x08), ADPCM
  (0x40/0x80), Huffman+ADPCM (0x41/0x81). No zlib/bzip2/LZMA/sparse.
  `patch_d2.mpq` uses the old IMPLODE flag and has no `(listfile)`.
  `d2sfx.mpq` lists only 31 of 2,360 files: names for unlisted encrypted
  files are unknown, but their keys are recovered from the sector offset
  table, so all 35,364 blocks decode. Only locale 0 appears.
- **Archive priority:** `specs/data/loading.md` §2 (priority descending,
  ties newest-opened first): patch_d2, d2xvideo, d2xtalk, d2xmusic, d2exp,
  d2video, d2music, d2char, d2speech, d2sfx, d2data.
- **DC6:** 140 frames with `flip = 1` (inventory item sheets) decode
  top-down. Verified upright visually.
- **DCC:** all 21,717 decode with every sub-stream exactly consumed. No
  bottom-up frames. The "equal cell" copy must read from a persistent
  direction buffer, not the previous frame (Riiablo's newer decoder gets
  this wrong).
- **DS1:** versions 3–18 seen. `ACT1\OUTDOORS\trees.ds1` (live, used by
  LvlSub) has truncated group records; missing fields read as 0. 55 v12/13
  files have trailing bytes, kept raw (open question).
- **TBL:** `.tbl` covers three things: string tables, font tables (`"Woo!"`
  magic) and two plain-text files (`font\latin\DEFAULT.TBL`, `FONTER.TBL`).
- **Known unused leftovers** (excluded, documented in specs): six
  version-4 DT1s in ACT1 (barracks, gargtrap, Catacombs, Cathedrl, Court,
  Outdoor1) and `chars\am\cof\amblxbow.cof` (junk).

## 6. Map rendering facts (Phase 1b)

- Cell origin: `sx = (x − y)·80`, `sy = (x + y)·40`. Floors are 160×80 diamonds.
- `WALL_BASE = 80`: walls and roofs are shifted down 80 px (roofs also up
  by roof height). Decided by rendering 0 and 80.
- Cells with bit 31 (hidden) are not drawn. This removed blue collision
  tiles along the river.
- Orientation 3 also draws orientation 4 at the same spot. 10, 11 and 13 in
  wall layers are skipped. Shadows are not drawn yet.
- Tile files come from the DS1's embedded list (`.tg1` → `.dt1`). The game
  actually uses `LvlTypes.txt` + `LvlPrest.txt` Dt1Mask; switch to that in
  Phase 2.
- GPU exactness requires: R8Uint index textures and an sRGB palette read
  with `textureLoad`, an sRGB target, `Msaa::Off`, `Tonemapping::None`,
  quads on pixel boundaries, and unique z per item.

## 7. Problems met and how they were solved

| Problem | Fix |
|---|---|
| No Rust/Git/MSVC on the PC | installed via winget (with approval) |
| Cargo can't extend workspace lints | d2-sim repeats them in its own `[lints]` |
| Ghidra 12.1 crashed on JDK 25 ("data file must be inside the data dir") | it targets JDK 21: installed Temurin 21, pinned via `JAVA_HOME_OVERRIDE` in Ghidra's `launch.properties` |
| A download failed because the temp folder was gone | recreate the scratch folder before downloading |
| Install contained third-party repack files | user replaced them; ownership gate redefined (section 2) |
| Clippy 1.99 lints (`as_chunks`, `is_multiple_of`, `too_many_arguments`) | code updated; Bevy system params grouped with `SystemParam` |
| Font and text files fail as string tables | classify `.tbl` by content |
| 8 files failed parsing in the survey | 7 unused leftovers documented; 1 live DS1 quirk became a spec rule |
| Walls 80 px off | `WALL_BASE = 80` |
| Solid blue strips on the map | skip hidden cells |
| First GPU capture partially rendered | verify judges a capture only when identical to the previous one; capped retries |
| Weak demo sprite (waypoint glow layer) | replaced with the bonfire DCC |

## 8. Open questions (carry forward)

1. **8 unflagged invisible collision tiles** in `townN1.ds1` (river.dt1
   tile 28, key 1/5/0, cells x 44|51, y 29|32|33|34) draw as blue patches.
   Ruled out: rarity, cell prop1 bits, LvlTypes/Dt1Mask file selection.
   Needs RE of the client's tile draw path.
2. DS1 v12/13 trailing bytes: possibly an early NPC-path section.
3. ~~MPQ archive priority order~~: answered by `specs/data/loading.md` §2.
4. DC6/DCC vertical placement (one-row disagreement between sources).
5. Meaning of the rendering tables in PL2 (Phase 6).
6. The original game's handling of truncated DS1 groups.

## 9. Starting Phase 2 (data tables, `d2-data`)

Goals from `docs/PLAN.md`: typed structs for all `.txt` tables,
cross-reference validation, and the mod patch-layer format and loader.

Facts and suggestions:
- Tables live at `data\global\excel\*.txt` in d2data/d2exp (274 `.txt` and
  116 `.bin` files listed). `patch_d2.mpq` may override some files; it has
  no listfile, so look up known names directly with `ArchiveSet::read`.
- 1.14d loads compiled `.bin` files at runtime. Decide early whether to
  parse `.txt` (readable, used by mods) or `.bin` (what the game uses), or
  both with a cross-check. Write that decision into the PLAN decisions log.
- Suggested order: a generic tab-separated reader with a spec, then
  `LvlTypes`/`LvlPrest`/`LvlSub` (they unblock correct map tile selection
  for the renderer), then the item, monster, skill and string-key tables.
- Mod patch layers: patches (add/change/remove rows and columns) applied to
  the user's tables at load time. Never write full modified tables to the
  repo.
- Keep the same workflow: spec first, unit tests from spec vectors, a
  survey over the real files, and visual or exact checks where possible.
