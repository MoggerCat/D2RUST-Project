# Handoff: client property tests (`d2-client` untrusted-input and state paths) — `claude/prop-client`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Cloud test session, 2026-10-06, medium (METHODS M14). Base
`claude/tender-meitner-mphas3` at `9b49081`. Repo only, no game files, no
GPU (M09: every claim below holds on this branch, synthetic inputs). Read
`specs/client/*.md`, `docs/`, `crates/` only.

Same method as the 2026-10-06 parser lesson (`docs/HANDOFF.md` §8:
property tests found 16 bugs valid files never hit): `proptest`
generates and shrinks, each case body runs under a deadline
(`tests/prop_support`), properties are the spec's rules or a model
written from them, never the code under test. Minimized failures stay as
`regress_*` tests.

## Files

| File | Inputs | Properties |
|---|---|---|
| `crates/d2-client/tests/prop_support/mod.rs` | — | `config(default)` (`PROPTEST_CASES` overrides), `bounded` (20 s deadline per case) |
| `tests/prop_client_bridge.rs` (beside fuzz-server's `prop_bridge.rs`, whose no-panic and delivery properties are not repeated) | arbitrary S→C chunks (raw bytes and runs of size-rule-valid messages with junk tails); random owned ids with a checking/rejecting handler; frame loop over a scripted link; any protocol version; arbitrary C→S bytes and a pool of valid messages with random clock steps on the real in-process host (`LocalLink`, `ProtoSizes`, a no-player game) | receive = model from `split_server_buffer` (refused chunk changes nothing; handled + unowned + rejected = messages; unowned per id; discarded first byte and count) §2 r2–r4, §6 r3–r4; frame counters and report sums, stop at a refused chunk §5 r3, §8 r1; version refused iff ≠ `PROTOCOL_VERSION` §9 r1; `addressed_unit` total §5 r4; send refused iff `route` refuses, nothing over 0x204 sent, `Filtered` exactly when the 1.14d duplicate rule (`intents-events.md` §2.1 r1, model written here) says so §4 r2, r3, r5 |
| `tests/prop_world_view.rs` | random unit add / remove / update sequences on `ClientWorld`; a valid COF and an arbitrary one (counts, components, draw order disagreeing); fixture rules with hidden units, out-of-range direction / frame / pass, wrapping placement; UI image draws; Bevy mirror over bridge frames that add and remove units (S→C 0x0E/0x0F with test handlers) | build deterministic (two builds equal, errors included); neutral rules draw nothing and hide every unit; on success: drawn + hidden = units, items sorted by key, frame table without repeats, every item frame resident, CPU frame 800×600×4 and byte-identical on two runs; mirror: one `UnitView` per model unit, `MirrorIndex` = model keys, after every frame (§7 r2–r3) |
| `tests/prop_controls.rs` | arbitrary text and bytes; line soups of TOML fragments (wrong types, unknown keys, dotted keys, inline and nested tables, arrays of tables, junk); line edits of valid files | never a panic; an error names a line of the file; a parse is clash-free, ≤ 2 distinct inputs per action, and its canonical rewrite parses to the same bindings; writer → parser round trip of clash-free sets (bindings and text, §A6 r1, r4); clashing sets refused (§A6 r2) |
| `tests/prop_ui.rs` | `UiRoot` over arbitrary panel trees (rects anywhere in i32, duplicate ids, widget rects, ignore / consume / intent answers), scripted `PanelRules` naming unknown ids or the opening panel; random open / close / toggle / event / hit / take sequences; `CellGrid`, `ScrollList`; `Presentation` on any window size and i64 cursor | root = §A2 model after every op (open order, routing top-most first over open panels containing the point, offered panels in order, intents queued only by the root, refused open changes nothing, hover, draw bottom-most first); grid cell ⇔ cell rect; scroll `first` in range; window → frame inside or `Outside`, frame point round trip (§A4) |
| `tests/prop_atlas.rs` | random frame sizes (0..MAX_SIDE+1, PAGE_SIZE, bad pixel counts) in random sets, random page clears, 1–3 pages; many equal frames into one page | slot inside its page with the gutter to the page edge; live slots never overlap another slot's gutter ring; slot bytes read back; `Atlas::check` clean on every live slot; a failed insert leaves the pages unchanged and names the right error; same op sequence → same slots (§A2) |
| `tests/prop_scene.rs` | random frames (transparent pixels, empty, wrong pixel count), items (positions and clips over all of i32, shade chains and blend tables naming present or missing maps, reserved `flip_x`), views anywhere (and the full frame) | error exactly when an item (or the view) is invalid; image = per-pixel model of §A4/§A5 written here; two runs identical; binned composition = direct (§A9); RGBA = palette lookup (§A8); `order` stable by key (§A6) |
| `tests/prop_cache.rs` | random begin_frame / get / insert / resolve (with failing loads) / set_budget sequences, budgets 0..any | pool = §A5 model after every op: resident keys, bytes used, events in order (evictions LRU by last frame then key, one overrun per frame), stalls; `resolve` leaves every frame key resident (§A4 r2); after an evicting op, over budget only with current-frame entries only |

Default case counts keep `cargo test -p d2-client` fast (the seven files
add about 25 s in a debug build; atlas is the slowest). Hunted harder
once with `PROPTEST_CASES` at 3000–5000 cases (bridge, controls,
ui, cache, scene) and 400 (world view, atlas), all clean after the fixes.

## Bugs found (input → fix), each a minimal root fix in `crates/d2-client/src`

| # | Where | Minimized input | Failure | Fix | Regress test |
|---|---|---|---|---|---|
| 1 | `ui/geom.rs` `Rect::{right,bottom,contains}` | `Rect::new(i32::MAX - 1, 0, 10, 1).contains(..)` | debug overflow panic (`x + w` in i32) | `right` / `bottom` return the exclusive edge in i64 (a rect may end past `i32::MAX`); `contains` compares in i64. Only caller was `contains` | `prop_ui::regress_rect_edge_past_i32` |
| 2 | `ui/widget.rs` `CellGrid::new` | origin `(i32::MAX - 10, 0)`, 2×1 cells of 10×1 | accepted; `cell_rect` overflow panic | refused with the existing `TooLarge` ("rect overflows i32") when a grid pixel would pass `i32::MAX` | `prop_ui::regress_cell_grid_past_i32` |
| 3 | `ui/widget.rs` `ScrollList::scroll` | `scroll(i64::MAX)` with `first > 0` | debug overflow panic | `saturating_add` before the clamp | `prop_ui::regress_scroll_by_i64_max` |
| 4 | `ui/frame.rs` `Presentation::to_frame` | `to_frame(i64::MIN, 0)` with bars (`left > 0`); `edge::window_pixel` floors `-inf` to `i64::MIN` | debug overflow panic | `saturating_sub` of the bars; far-off positions stay `Outside` | `prop_ui::regress_window_pixel_at_i64_min` |
| 5 | `scene` (`cpu::compose_binned`, `bins::Bins::rect`) | view `Rect::new(i32::MAX - 3, 0, 40, 1)` | `Bins::rect` wrapped the bin x to negative; `compose_binned` multiply overflow panic | a view whose pixels pass the i32 screen range is refused by `compose`, `compose_binned` and `bin` (new `SceneError::View`, `Rect::check_view`): items are placed in i32 and bin rects are `Rect`s, so no such pixel can be drawn | `prop_scene::regress_view_past_i32` |
| 6 | `bridge/intent.rs` `route` | system message 0x6A (valid size) in a 517-byte buffer | `classify_client` reads only the message's own size, so `route` passed it; the host's net send asserts ≤ 0x204 and the send failed as a `LinkError` (§4 r3) | **same bug as fuzz-server's**, already fixed on the base (`IntentError::TooLarge`, checked after the classifier); this branch's own fix was dropped in the merge and the base fix kept | fuzz-server's `prop_bridge::route_every_id_and_length` |

Bugs 1–4 were re-checked against the unfixed sources (`git stash` of
`src/`): each regress test panics there. Bug 5 was found by the property (`scene/cpu.rs:78` multiply overflow);
its regress test uses the new error variant. Bug 6 was found
independently by this branch and by fuzz-server; the base keeps
fuzz-server's fix.

Not changed, for the owner to judge:

- `assets::cache::Pool::insert` adds `bytes` with `+=`: a caller passing
  byte counts summing past `u64::MAX` panics in debug. Byte counts are
  sizes of resident memory (`assets::size::ByteSize`), so not reachable;
  the property draws sizes below 2^40.
- `Pool::begin_frame` does not evict: a pool left over budget by an
  overrun frame stays over budget until the next insert or budget change
  (module doc: "eviction happens when an insert needs room"). §A5 says
  the budget is exceeded "and kept until the frame ends"; the property
  checks the module's reading (over budget only right after an evicting
  op with current-frame entries only). If §A5 means eviction at frame
  end, `begin_frame` should call the eviction; that is a spec decision.

## Changes outside `crates/d2-client/tests/`

- None in `Cargo.toml` after merging the base (it already has the one
  `[dev-dependencies]` section with `proptest`).
- Root fixes 1–5 above (`ui/geom.rs`, `ui/widget.rs`, `ui/frame.rs`,
  `scene/mod.rs`, `scene/cpu.rs`, `scene/bins.rs`); `bridge/intent.rs`
  is the base's.

No spec, `docs/HANDOFF.md` or `docs/PLAN.md` change.

## Gate (this branch)

`cargo fmt --check`; `cargo clippy --workspace --all-targets -- -D
warnings`; `cargo test -p d2-client` (unit 220 passed / 5 ignored; every
integration test file passes); `cargo run -p depcheck`; `python3
tools/spec_index.py --check`; `python3 tools/methods.py check`; `python3
tools/coverage.py --check` and `--selftest`: all pass.
