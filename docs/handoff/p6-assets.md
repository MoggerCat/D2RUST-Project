# Handoff: Phase 6 C1 + C2, assets paths, loaders, residency (`d2_client::assets`)

Branch `claude/p6-assets`, based on `claude/bold-ptolemy-jvyvxy` at
`978e6c4` (2026-10-06, cloud). Spec: `specs/client/assets.md` §A1–A2,
§A4–A5 and Test vectors (d2rs-own design draft; no original behavior).
Tasks C1 and C2 of `docs/PLAN.md` Phase 6.

## State

**Implemented; design checks pass in CI; game-file check queued.** Every
§A1, §A2 and §A5 test vector of `assets.md` is a unit test that runs
without game files or a GPU. §A4 (synchronous load of missing keys,
stall metric) is covered by `resolve` tests. The §A7 vector ("a scene
rendered twice with different load orders → identical images") is not in
this task's sections; it belongs with the compositor/verify tasks (C4,
C6). This part has only the cache half: `eviction_is_independent_of_insert_order`.

Gate run on this branch: `cargo fmt --all -- --check`, `cargo clippy -p
d2-client --all-targets -- -D warnings`, `cargo test -p d2-client`,
`cargo run -p depcheck`, `python3 tools/spec_index.py --check`,
`python3 tools/methods.py check`, `python3 tools/coverage.py --check`.

Changes outside `crates/d2-client/src/assets/`: `crates/d2-client/src/
assets.rs` itself (this task's module: `mod` lines, canonical
`asset_path`, generic reader, three loaders). No change to `lib.rs`,
`main.rs`, `app.rs`, `bridge/`, `map/`, `render/`, specs, other crates or
Cargo dependencies.

## Code map

| Path | What | Spec |
|---|---|---|
| `crates/d2-client/src/assets.rs` | `mpq://` source (`MpqSourcePlugin` over `ArchiveSet`, new `FileSourcePlugin` over any `FileSource`); reader now goes through `path::read_asset` (canonical paths only, missing = `NotFound(path)`); `asset_path` now folds to canonical spelling; loaders `Pl2Asset`/`pl2`, `CofAsset`/`cof`, `TblAsset`/`tbl` added beside the 1b ones; `d2_loader!` split out of `d2_asset!` | §A1–A2 |
| `crates/d2-client/src/assets/path.rs` | `CanonicalPath` (strict: non-empty, ASCII, no leading `/`, no empty/`.`/`..` component, no `#`/`:`), `fold`, `FileSource` trait (+ impl for `ArchiveSet`), `read_asset`, `ReadError` (every variant names the path), `MemorySource` (case-insensitive in-memory source for tests/tools) | §A1 |
| `crates/d2-client/src/assets/tbl.rs` | `TblAsset::{Font(FontTable), Strings(StringTable)}`, chosen by `FontTable::is_font_table`; a broken font is the font error, never retried as strings | §A2, §Edge cases |
| `crates/d2-client/src/assets/size.rs` | `ByteSize` for every parsed asset type (`size_of` + owned buffer lengths), the "parsed files" pool unit | §A5 |
| `crates/d2-client/src/assets/cache.rs` | `Pool<K, V>`: byte budget, LRU by last frame used with key-order ties (`BTreeSet<(frame, key)>`), current frame never evicted, `Overrun` logged once per frame, `resolve` (mark resident keys first, then load missing ones synchronously, `Stall` events + `StallMetric`), `CacheEvent` log, `Budgets` defaults, `Clock`/`WallClock` | §A4–A5 |

## Behavior choices (d2rs-own, within the design)

- `asset_path(&str) -> String` keeps its signature (used by `app.rs`) and
  stays total: it only folds (`\` → `/`, `A`–`Z` → `a`–`z`, matching
  `mpq.md` §3, which folds no other bytes). Validity is enforced at the
  reader: a path that is not canonical is an error naming it, so a
  second spelling can never load as a second handle. New code should use
  `CanonicalPath::new(..)?.asset_path()`.
- Eviction happens only when an insert needs room (or `set_budget`), not
  at frame boundaries: an over-budget frame's entries stay until a later
  insert needs the bytes. `begin_frame` requires a strictly increasing
  frame number; inserting a resident key is an error.
- `resolve` marks every listed resident key used before loading any
  missing key, so loading cannot evict an entry the same frame still
  needs (test `resolve_counts_stalls_and_protects_listed_keys`).
- Stall durations use a `Clock` (wall clock in the client; logged only,
  never used for a decision). No sim code reads any of this.

## Seams for other tasks

- **Render stage (C3/C4/C5):** owns the frame counter; per frame call
  `begin_frame(n)`, then `resolve(&keys, &mut clock, load)` with the
  stage-1 `FrameSetKey` list before compose; drain `drain_events()` and
  log (`Stall`/`Overrun` as warnings). `verify` (C6) puts
  `FrameReport::stalls` in the case result.
- **FrameSets (C3):** `Pool<FrameSetKey, FrameSet>` with
  `bytes = Σ width × height + headers` computed by C3 (the type is C3's).
  `FrameSetKey` needs `Ord + Clone + Debug`.
- **Atlas (C3):** whole-page eviction fits `Pool<PageId, ()>` with
  `bytes = 2048 × 2048`; clearing the page and invalidating its slots,
  and restarting packing in it, stay in the atlas code. The pool's
  `Evicted` event names the page to clear.
- **Audio (C10):** sounds pool = `Pool<CanonicalPath, SoundAsset>`,
  `bytes = samples × 2`, budget `Budgets::sounds`.
- **Prefetch (§A4 step 3):** not implemented; it needs the bridge's
  "newly active room / new unit type" report (`bridge/` is not this
  task's). A prefetch inserts into the same pools from a Bevy task; it
  must not mark entries as used by the current frame. Add with the
  bridge hook.
- **`ClientConfig` (§A6):** not written here. `Budgets` is the plain
  struct its budget fields deserialize into; the versioned file (M20)
  belongs to the config task. No persisted format is introduced by this
  task (nothing written to disk).

## `TODO(spec)` hooks

None needed in code: §A1–A2 and §A4–A5 state no original behavior. The
original-behavior items this design depends on stay with their owner
specs: §B1 (which archive wins for client assets, `data/loading.md` §2;
today `ArchiveSet::find` decides) and §B2 (locale font directory, e.g.
`data\local\font\latin`, `ui/text.md`); the test above uses that path
only as a synthetic name.

## Open questions

1. Budgets are the §A5 guesses until measured (spec §Open questions 1).
2. Whether non-ASCII archive names exist in the 1.14d set: the strict
   check refuses them. The queued check below would show any.

## Local run queue (to add to `docs/HANDOFF.md` §5 at merge)

- **Canonical paths over the live archive set.** Every listfile name of
  the 1.14d archives must canonicalize (`CanonicalPath::new` ok) and read
  back through the canonical path with identical bytes. There is no
  command for it yet: add an `#[ignore]` test reading `D2_GAME_DIR`
  (`crates/d2-client/tests/assets_game.rs`, `cargo test -p d2-client
  --test assets_game -- --ignored`) in a local session. Expect 0 refused
  names and 0 byte differences.
- **New loaders on live files.** In the same test: every `.pl2`, `.cof`,
  `.tbl` in the set parses through `Pl2Asset`/`CofAsset`/`TblAsset`
  (`mpq-tool formats` already parses these with the same parsers; the new
  part is the `.tbl` font/strings split: expect every `data\local\font\**`
  `.tbl` → `Font`, every `data\local\lng\**` `.tbl` → `Strings`).
- **Budget measurement** (spec §Open questions 1): decoded size of every
  DCC/DC6/DT1 by `ByteSize` and of a full town scene.
