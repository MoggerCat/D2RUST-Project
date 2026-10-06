# Handoff: controls file (`d2_client::controls`, PLAN Phase 6 C9)

Branch `claude/p6-controls`, based on `claude/bold-ptolemy-jvyvxy` at
`978e6c4` (2026-10-06, cloud). Spec: `specs/client/ui.md` §A6 (d2rs-own
design, draft) and its §Test vectors (controls rows).

## State

**Implemented; d2rs-own format, so there is nothing to verify against
1.14d.** `d2controls 1` is our format: the parser, writer, `dev` preset,
clash check and migration hook are checked by unit tests alone. The
original's default key configuration (`preset = "original"`) is
**not** implemented: it waits for `specs/ui/controls.md` (§B4), and the
parser rejects it with an error that says so.

Every §A6 rule and all 7 controls test vectors are unit tests (16 tests in
`crates/d2-client/src/controls/tests.rs`, inline strings, run in CI).
Clash-check perturbation test (M08): `clash_check_catches_every_perturbation`
binds each later action to an input of each earlier one and checks that
exactly that pair and input are reported for same-context pairs (>100
cases), never for cross-context ones.

Gate run on this branch: `cargo fmt --all -- --check`, `cargo clippy -p
d2-client --all-targets -- -D warnings`, `cargo test -p d2-client` (47 + 3
pass), `cargo run -p depcheck`, `python3 tools/spec_index.py --check`,
`python3 tools/methods.py check`, `python3 tools/coverage.py --check`: all
clean.

Changes outside `crates/d2-client/src/controls/`: `pub mod controls;` in
`crates/d2-client/src/lib.rs`; one dependency line in
`crates/d2-client/Cargo.toml` (+ its `Cargo.lock` entry), see below. No
spec, other module or other crate touched.

## Dependency (M10)

`toml_edit = { version = "0.25", default-features = false, features =
["parse"] }`. §A6 says the file is TOML; a real TOML parser is the proven
method over a hand-written subset. `toml_edit` 0.25.15 was already in
`Cargo.lock` (Bevy's `bevy_macro_utils` and `proc-macro-crate` build it),
so nothing new is downloaded. It was picked over `toml` because its
`Document` keeps byte spans for every key and value, which give the line
numbers §A6 requires. TOML itself rejects a key defined twice (with its
line), which covers a duplicate action in `[bindings]`.

## Code map

| Path | What | Spec |
|---|---|---|
| `crates/d2-client/src/controls/mod.rs` | `VERSION`, `MAX_INPUTS`, `Preset` (+ `DEV_PRESET` table), `Bindings` (`inputs`, `action_for`, `find_clash`), `Clash`, `ControlsFile` (`from_effective`, `effective`), `RawFile`/`Named`, `migrate`, `ControlsError`/`ErrorKind`, `parse`, `load`, `write` | `client/ui.md` §A6 |
| `crates/d2-client/src/controls/names.rs` | closed lists `Key` (portable input names), `Action` (with its `Context`), `Context` | §A6 rule 2–4 |
| `crates/d2-client/src/controls/tests.rs` | §Test vectors, strictness table, M08 perturbation | §Test vectors |

Pipeline: TOML → `RawFile` (structure; names as strings with lines) →
`migrate` → `ControlsFile` (names resolved) → `Bindings` (preset, then
`[bindings]`, then `[unbind]`; then the clash check).

## Design choices made here (not in the spec; review)

1. `preset` is **required** (strict, no default base set). Error on line
   1 when missing.
2. `[bindings]` and `[unbind]` are optional. `action = []` is an error
   ("list it in [unbind] instead"); one input listed twice in an action,
   more than 2 inputs, and an action listed twice in `[unbind]` are errors.
   An action in both `[bindings]` and `[unbind]` is allowed (unbind wins,
   test vector).
3. Each action has exactly one context (`Action::context`). A clash error
   points at the file line of the later action of the pair if the file
   binds it, else the earlier one; preset-only clashes cannot happen (the
   `dev` preset is tested clash-free).
4. Version handling: a version above `VERSION` is rejected before any
   other structure is read (a newer file may have other keys); a version
   below goes through `migrate`, which today rejects everything but 1
   (`unsupported version N`), since 1 is the first format.
5. The writer always emits `[bindings]` and `[unbind]` (possibly empty),
   entries in `Action` enum order; it is a fixed point (`write(parse(write
   (f))) == write(f)`). `ControlsFile::from_effective` builds the minimal
   file over a preset.
6. Inline tables (`bindings = { ... }`) and dotted keys are accepted, as
   TOML-equivalent spellings; arrays of tables are type errors.
7. No Bevy key mapping here: the spec header gives Bevy input → actions
   to `d2-client::input`; that module maps `bevy::input::keyboard::KeyCode`
   / `MouseButton` / wheel to `controls::Key`.

## Open questions

1. The `Action` list and its contexts are a d2rs placeholder (move/attack,
   panel toggles, 16 skill slots, 4 belt slots, chat and panel actions).
   `specs/ui/controls.md` (§B4) owns the original's configurable command
   list; when it lands, reconcile names (renames go through `migrate` and
   a format version bump if old files must keep working).
2. Wheel and modifiers: is `show_items = ["LeftAlt"]` a hold action and
   are wheel inputs bindable to every action? Semantics belong to
   `input` + §B4; the file accepts any `Key` for any action today.
3. Config location `<config_dir>/d2rs/controls.toml`: resolving
   `<config_dir>` per platform is not implemented (`load(path)` takes a
   path). Pick a method (a `dirs`-style crate or env vars) in the session
   that wires the client start-up; and decide whether a missing file means
   "write the `dev` file" or "refuse to start".

## Checks to queue

None: the format is ours and fully covered by CI unit tests. The
original-defaults check belongs to `specs/ui/controls.md` (§B4: identical
action list vs the 1.14d key config), to queue when that spec is written.
