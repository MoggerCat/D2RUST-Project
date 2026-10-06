# Handoff: mutation testing of `d2-client` (plain-Rust parts) — `claude/mutants-client`

> Not yet folded into `docs/HANDOFF.md` / `docs/PLAN.md` (neither is edited here); a docs session folds it, then this file stays as the detailed record.

Cloud test session, 2026-10-06, base `claude/tender-meitner-mphas3`.
Method: METHODS M08 (prove the check can fail) applied to the unit tests
of `crates/d2-client` with `cargo-mutants`. Read `specs/`, `docs/`,
`crates/` only. No source file of `d2-client` was changed: every change
is the new test target `crates/d2-client/tests/mutants_client/` and this
note.

## 1. Result

| | Mutants | Caught | Missed | Unviable | Timeout |
|---|---:|---:|---:|---:|---:|
| Before (existing unit tests) | 1,303 | 962 | 220 | 120 | 1 |
| After (+ `tests/mutants_client`) | 1,303 | 1,171 | 12 | 120 | 0 |

All 12 survivors left are classified in §3: 8 cannot be killed by any
test (equivalent or unobservable), and 4 (`assets/path.rs`) die only to
the `#[ignore]` game-file test of §4, which CI does not run. No code was
found to be wrong against its spec (§5).

Per file (paths under `crates/d2-client/src/`):

| File | Mutants | Unviable | Missed before | Missed after |
|---|---:|---:|---:|---:|
| `assets/cache.rs` | 54 | 4 | 11 | 0 |
| `assets/path.rs` | 30 | 1 | 4 (+1 timeout) | 4 |
| `assets/size.rs` | 108 | 0 | 102 | 0 |
| `audio/log.rs` | 75 | 3 | 1 | 0 |
| `audio/mixer.rs` | 68 | 6 | 12 | 0 |
| `audio/mod.rs` | 36 | 5 | 3 | 0 |
| `bridge/dispatch.rs` | 27 | 5 | 5 | 0 |
| `bridge/intent.rs` | 7 | 1 | 0 | 0 |
| `bridge/link.rs` | 2 | 1 | 0 | 0 |
| `bridge/local.rs` | 15 | 6 | 1 | 1 |
| `bridge/mod.rs` | 35 | 6 | 1 | 0 |
| `bridge/receive.rs` | 12 | 0 | 1 | 0 |
| `bridge/world.rs` | 4 | 1 | 0 | 0 |
| `composite/mod.rs` | 14 | 2 | 0 | 0 |
| `controls/mod.rs` | 52 | 16 | 2 | 0 |
| `controls/names.rs` | 2 | 0 | 0 | 0 |
| `frames/atlas.rs` | 145 | 2 | 14 | 1 |
| `frames/mod.rs` | 19 | 5 | 1 | 0 |
| `scene/bins.rs` | 56 | 3 | 0 | 0 |
| `scene/cpu.rs` | 42 | 0 | 0 | 0 |
| `scene/item.rs` | 51 | 7 | 5 | 0 |
| `scene/mod.rs` | 33 | 2 | 0 | 0 |
| `scene/order.rs` | 25 | 3 | 0 | 0 |
| `ui/draw.rs` | 1 | 0 | 0 | 0 |
| `ui/frame.rs` | 28 | 1 | 4 | 4 |
| `ui/geom.rs` | 24 | 1 | 0 | 0 |
| `ui/panel.rs` | 6 | 1 | 3 | 0 |
| `ui/root.rs` | 38 | 8 | 2 | 1 |
| `ui/widget.rs` | 101 | 8 | 16 | 0 |
| `verify/case.rs` | 67 | 7 | 9 | 0 |
| `verify/mod.rs` | 126 | 15 | 23 | 1 |

The scene and CPU compositor (`scene/*`) and `composite` had no survivor:
their spec test vectors already pin every mutated operator.

## 2. Tests added (`crates/d2-client/tests/mutants_client/`)

One integration-test binary (`main.rs` plus one module per area), so a
mutation run links one extra binary per mutant, not ten. Each test names
the rule that decides the outcome. All are synthetic and run in CI
except the one `#[ignore]` test of §4.

| Module | Kills (before-survivors) | Decided by |
|---|---|---|
| `assets_size` | every `ByteSize` impl: `Cof`, `StringTable`, `TblAsset`, `Dc6`, `Dcc`, `Dt1`, `Ds1`, `Pl2`, `Palette` (102) | `assets.md` §A5 "parsed files" unit: `size_of` + owned buffer bytes (module doc of `assets::size`); distinct buffer lengths so every term shows |
| `assets_cache` | `Pool` accessors (`name`, `budget`, `len`, `is_empty`, `frame`, `contains`), `WallClock::now_micros`, `path::fold` (also the old timeout) | §A5 pools; §A4 stall duration (sleep 3 ms → ≥ 3,000 µs); §A1 canonical form |
| `audio` | `TriggerQueue::{len,is_empty}`, `present` same tick twice, `Sound` accessors, log ` ` refused, looped wrap keeps the phase fraction (`%=`/`>>`), one-shot ending on a block edge removed with that block | `audio.md` §A3 (ticks never go back; a frame without a tick presents the same tick, `bridge.md` §8 rule 2), §A4 (32.32 accumulator, nearest sample), §A5 + `audio::log` module doc (escape set), `mix` doc |
| `bridge` | `Box<L>::protocol_version`, TSV empty owner / empty name, `Rows` count past id 0xFF, `check` duplicate handler and equal owner guards, unowned counted per id, handled counted | `bridge.md` §9 rule 1, §6 rules 1–5 |
| `controls` | `find_clash` self-duplicate guard, clash line among several `[bindings]` entries | `ui.md` §A6 rule 2 (a clash is two actions), §A6 errors with line numbers |
| `frames` | `FrameSource::part_count`, `AtlasSlot::is_empty`, shelf gutter between frames on one shelf, bottom page gutter (`next_y + h + GUTTER`), max side 2046, `max_pages`, `CheckReport::mismatches`, `check` on an empty slot and the top ring row | `assets.md` §A3; `render-pipeline.md` §A2 (2048 pages, 1-pixel gutter of index 0, deterministic shelf packer) |
| `scene` | `FrameView::pixels`, `MapTable::is_empty` | §A2, §A4 |
| `ui` | `NoStrings::get`, `UiRoot::take_intents`, widget rects (`FrameImage`, `ScrollList`, `TextInput`), `CellGrid::{cols,rows,cell_rect}` bound, `ScrollList::{len,is_empty,row_at}` (offset list, partial last row), `TextInput::draw` | `ui.md` §A2 |
| `verify` | `compare` / `compare_indices` first-mismatch column/row and size check, `perturb` on bytes with the top bit set (`^=` vs `|=`), expectations in an offset view, perturbed-run report line, RGBA-only GPU mismatch fails the case, `Summary::add` / `exit_code`, `CaseKind::name`, table rules `src`/`dest` parse, TOML error line | `render-pipeline.md` §A10 (compare byte for byte, first mismatch, `--perturb N` fails with exactly N), case format doc of `verify::case` |

Coverage claims (`docs/COVERAGE.md`): one, `bridge::boxed_link_reports_the_inner_version`
claims `specs/client/bridge.md §9 r1` (its assertions check the whole
rule: foreign version refused, own accepted). The others check part of a
section or an implementation detail the spec leaves to the code doc, so
they claim nothing (`docs/handoff/coverage-claims.md` §1).

## 3. Survivors left (12), with reasons

| Mutant | Why it survives |
|---|---|
| `ui/frame.rs:72:39`, `73:39` (`fw - 1` → `fw + 1`, `fw / 1`; same for `fh`), 4 mutants | **Equivalent.** The clamp's upper bound is unreachable: the bar check above returns `Outside` unless `dx < fw·s`, so `dx / s ≤ fw − 1` already (the code comment says so). |
| `ui/root.rs:36` `NoPanelRules::on_open` → `vec![]` | **Equivalent:** `vec![]` is `Vec::new()`. |
| `bridge/local.rs:128` `LocalLink::protocol_version` → `1` | **Equivalent today:** `d2_proto::PROTOCOL_VERSION` is 1. The existing version tests kill it as soon as the version is bumped. |
| `frames/atlas.rs:21` `PAGE_SIZE - 2 * GUTTER` → `2 / GUTTER` | **Equivalent while `GUTTER` = 1** (both give 2046). |
| `verify/mod.rs:790` `print_report` → `()` | **Unobservable in-process:** it only prints to stdout; no spec fixes the console text. |
| `assets/path.rs:121` `ArchiveSet::read_file` → `None`, `Some(Ok(vec![]))`, `Some(Ok(vec![0]))`, `Some(Ok(vec![1]))`, 4 mutants | **Needs game files:** `ArchiveSet` opens real MPQs; the repo has no MPQ writer. Killed by the `#[ignore]` test of §4 (local run). |

## 4. Local run queue (to fold into `docs/HANDOFF.md` §5)

```
D2_GAME_DIR=... cargo test -p d2-client --test mutants_client -- --ignored
```

Look for: `assets_cache::archive_set_reads_files_it_holds` passes
(`data\global\palette\act1\pal.dat` reads as 768 bytes, a missing name
reads as `None`). That run kills the 4 `assets/path.rs:121` survivors.

## 5. Code vs spec

No mutant exposed code that disagrees with its spec. Every killed
survivor was a missing assertion, not a bug. Two observations, no action
needed:

- `controls::Bindings::find_clash` keeps its `first != action` guard only
  for bindings built in code: the parser already refuses a repeated input
  (`RepeatedInput`), so through files the guard is unreachable.
- `atlas::Layout::place` keeps one spare row at the page bottom when a
  shelf would end on row 2047; the new test pins that the bottom gutter
  is kept, not a tighter packing (the spec fixes the gutter, not packing
  density).

## 6. How it was run (to repeat)

- `cargo install cargo-mutants --locked` (27.1.0).
- Scope (`-f`): `scene`, `frames`, `composite`, `controls`, `ui`, `audio`,
  `assets`, `bridge`, `verify`. Excluded (`-e`) as Bevy or GPU code:
  `frames/upload.rs`, `ui/edge.rs`, `audio/output.rs`, `assets/tbl.rs`,
  `bridge/mirror.rs`, `verify/map.rs`, `verify/gpu.rs`. Also excluded:
  `fmt` functions (`-E fmt`). Not in scope: `map`, `world_view`, `render`,
  `gpu_compositor`, `app`, `main`.
- Tests run per mutant: `--cargo-test-arg=--lib` for the "before" pass;
  `--lib --test mutants_client` for "after". The `e2e_*` integration tests
  were in neither pass.
- Cost: about 20 s per mutant (incremental build of `d2-client` plus
  link). Copy mode (`-j`) rebuilt all of Bevy in every copy (mtimes not
  kept), so instead: three `cp -a` copies of the repo with `target/`
  (mtimes kept, no rebuild), one `--in-place --shard i/3` run per copy,
  about 2.5 h for 1,303 mutants on 4 cores. Re-measure with `--iterate`
  over a combined output directory (only the survivors run again).
- `mutants.out/` is not committed.
