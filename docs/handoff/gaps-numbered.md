# gaps-numbered (cloud implementation session)

Gap tests for the numbered rules that spec-unit-numbering exposed as
uncovered. Each test is synthetic, checks one rule and claims it with
`// Covers:`. New tests live in `gaps_numbered_tests.rs` files (plus a few
integration-test files). No spec files were edited.

## `--summary`, before → after

| | rules | unit | any | verified |
|---|---|---|---|---|
| before | 3190 | 2844 (89.2%) | 2897 (90.8%) | 227 (7.1%) |
| after | 3190 | 2881 (90.3%) | 2934 (92.0%) | 227 (7.1%) |

| spec | any before → after |
|---|---|
| client/bridge | 85.4 → 100% |
| render/camera | 85.7 → 100% |
| render/sprite-placement | 58.3 → 83.3% |
| render/map-preview | 72.2 → 88.9% |
| render/composition | 61.1 → 77.8% |
| world/cube | 82.5 → 93.7% |
| drlg/preset | 70.6 → 79.4% |
| formats/animdata | 72.2 → 77.8% |
| data/patch-layers | 68.2 → 86.4% |
| data/loading | 70.0 → 74.0% |

fixups, txt-format, callbacks, runtime-maps and mpq had no uncovered
numbered `rN` left; what remains there is prose or survey text
(`fixups §2 text`, `§11 text`, `txt-format §1`).

## Findings (code vs spec)

1. **Fixed: sprite-placement edge r4.** The original refuses a DC6 with a
   version other than 6 or with flags bit 2 set. d2rs accepted bit 2, and
   accepted any version on a `Dc6` built in code. `FrameSet::from_dc6`
   now returns `FrameError::Dc6Header` (`crates/d2-client/src/frames/mod.rs`).
2. **Fixed: composition §3 r3.** In screen open mode 3 the world is
   skipped. d2rs drew it. `build_frame` still computes the camera and shake
   (§3 r1) and then builds through a new `NoWorld` view: no tiles, no
   units, UI unchanged (`world_view/feed.rs`). Nothing sets mode 3 yet,
   because `open_mode` is still `TODO(ui/panels.md)`.
3. **Fixed: patch-layers §8 sort order.** The spec says B findings sort by
   table, but the code sorted by code first. `Finding::sort_key` class 2 now
   sorts by table, with the code only breaking ties.
4. **Open: loading policy r5.** The spec says `d2exp.mpq` is required, but
   `bin::load` (`crates/d2-data/src/bin.rs`) still loads a classic install.
   Fixing it means changing existing tests that load classic installs.
   Left for a decision.

## Not covered (and why)

- sprite-placement **edge r2**: d2rs has no L/R column-clip setting, and
  the spec says the branch is dead in play.
- sprite-placement **§6**: index-0 transparency. It needs the game-file
  check that the spec's open question 1 already queues.
- map-preview **edge r2**: the spec never defines "translucent", so it
  can't be checked. **Edge text** is an intro sentence.
- composition **§1 r1, §1 r2**: facts about the 1.14d binary and the trace
  recorder, not d2rs behaviour.
- cube **edge text**: policy ("All reproduced by default"). **§3 r2**:
  "date read once" needs a call counter on the 60-method fake world.
  **§6.1 r3**: a definition that only §6.3 uses, and §6.3 is covered.
- preset **edge r3**: about the parser's lack of bounds checks; the sim
  gets an already-parsed `Ds1Input`. **§2 r3**: a d2-data rule.
  **§3.1 r4**: a pointer. **§3.2 r4**: the client automap is not
  modelled (an existing TODO). **§5.2 r4/r7/r9/r11**: fields that
  `Ds1File` doesn't keep.
- preset **edge r8** is claimed for d2rs's existing choice: an item id ≥ 1
  is rejected with `ItemCodeBeyondTable`. The spec only says the original
  "would read beyond".
- patch-layers **§1**: no `Ruleset` type exists yet. **§11**: a versioning
  policy, and the D4 reverse-order A05 result is not in its list.
  **Edge r1**: see the queue below.
- loading **§1 r3, §3.2 r2/r3**: `-txt` mode and expfield, which d2rs does
  not reproduce (policy r5). **§10 r6**: the automatic TC lists are built
  in `d2-sim/src/treasure/runtime.rs`; it needs a combined claim.
  **§10 r7**: DS1 Expansion rows are not in d2-data. **Policy r6**: no
  survey tool lists the never-read files.
- bridge **§9 r3** (versioned save of the bridge state) is about a future
  feature. Its test is a tripwire: it fails if a file writer appears in
  `bridge/`.

## Local run queue (game files)

1. `cargo test -p d2-data --test patch_gaps_game -- --ignored` with
   `D2_GAME_DIR` set. Expected on 1.14d:
   - armor columns 63/64/161/162 are `mindam,maxdam,mindam,maxdam`, and
     only 63/64 bind;
   - `automap` `Type2` and `chartemplate` `SkillName` repeat, and no copy
     binds;
   - weapons column 18 has an empty name;
   - `set mindam@2` gives N02 and `mindam@1` does not.

   If it passes, add
   `// Covers: specs/data/patch-layers.md §edge-cases-original-bugs r1`
   above `edge_duplicate_columns_in_1_14d_headers`.
2. Because of finding 1: scan every `*.dc6` in the 1.14d MPQs and confirm
   none has version ≠ 6 or flags bit 2. There is no tool command for this
   yet. Any hit is a file the new build-time error would refuse.

## Other changes

`tools/data-tool` gained a `[dev-dependencies] test-fixtures` (plus the
`Cargo.lock` line). It is used by `tools/data-tool/tests/patch_cli.rs`,
which runs the binary against the synthetic install (patch-layers §10).
