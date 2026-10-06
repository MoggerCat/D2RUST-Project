# Handoff: `scene` verify case kind (1.14d frame captures) — `claude/render-capture`

> Waiting to be folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md`; this file stays as the detailed record.

Cloud implementation session, 2026-10-06, medium (METHODS M14). Branch
`claude/render-capture`, from `claude/specs-staging` at `c6e40f9` (repo
only: no `game/`, no GPU, no recording). Specs: `specs/render/capture.md`
(§4–§8, edge cases), `traces/FORMAT.md` §Render captures,
`specs/client/render-pipeline.md` §A10 (link 1). Recorder read, not
changed: `tools/trace-recorder/record_frames.py`. No spec, HANDOFF, PLAN,
`scene/`, `gpu_compositor/`, `world_view/` or `app` edit.

## State

**Implemented; capture checks proven on synthetic captures, never run
on a real recording.** A capture recorded with `record_frames.py`
becomes a pass/fail with `d2-client verify --cases
crates/d2-client/capture-cases --case <id>` the same day:

- `check = "stability"` (capture.md §7) is complete and needs no
  renderer: every PNG re-hashed against its record, frames with
  `clear_counter > 0` must be all index 0 (capture.md edge cases),
  state groups (same key fields as the recorder) must each have one
  `index_sha256`, and at least two keys must be seen at least twice.
  `--perturb N` flips one byte in each of the first N saved frames:
  exactly N re-hashes differ (§7 perturbation).
- `check = "compare"` (capture.md §6) runs the capture's integrity
  checks (PNG size, `index_sha256`, `palette_sha256` against the record),
  then asks a `SceneSource` for the d2rs scene of the recorded state,
  composes it with the CPU reference (`scene::compose`), compares the
  index bytes (count + first mismatch) and the 768 palette bytes
  (differing entries + first), then the GPU half through the existing
  `GpuCompositor` seam (indices and RGBA vs the capture's indices through
  its own palette). `--perturb N` corrupts N pixels of each capture after
  its integrity check: every compared frame fails with exactly N on both
  halves (M08). **No scene source is wired**: `main.rs` passes
  `SceneNotWired`, so compare cases report the new status
  `SCENE NOT WIRED` (exit 2, never a pass) after their integrity checks.
  Turning a recorded state (player, origins, panel, shake; `camera.md`)
  into tiles, units and UI is the world view's job (`ViewRules`, owned by
  render-camera-placement / p6-integrate); its provider implements
  `verify::capture_case::SceneSource` and replaces `SceneNotWired` in
  `main.rs` `verify`.

Default frame selection for compare: every captured frame except the
first (its uncleared bottom rows predate the recording, capture.md edge
cases); `draws = [...]` selects exact draw counters. The source gets the
previous draw's captured image when that draw was captured (the original
keeps unwritten pixels, `composition.md` §3, §6); `scene::compose` still
starts from index 0 (its `TODO(spec: render/composition.md)`, owned by
render-composition), so until the CPU reference takes an initial buffer
a source must either draw the whole frame or the bottom 47 rows will
differ.

Frames sharing a tick are flagged in the report (capture.md §4; Open
question 2 data). Refused frames are counted and skipped.

## Code map rows

| Path | What | Spec |
|---|---|---|
| `crates/d2-client/src/verify/capture.rs` | `frames-raw-1` reader (strict; `format` checked first; unknown kind, missing / mistyped field, record after footer, footer `counts` / `ticks` disagreeing with the lines are errors naming the line and key); PNG reader (8-bit color type 3, 768-byte PLTE, indices raw via `png`); `sha256_hex`, `palette_bytes`, `Image::check`; `StateKey`, `stability`, `repeated_ticks` | capture.md §3–§7, FORMAT.md |
| `crates/d2-client/src/verify/capture_case.rs` | case kind `scene`: `SceneSource` / `SceneJob` / `SceneOutcome` seam, `SceneNotWired`, `resolve` (`raw = "latest"` = newest `traces/raw/*-frames.jsonl`, images default `game/captures/<stamp>`), `run_capture` (stability, compare) | capture.md §4, §6–§8 |
| `crates/d2-client/src/verify/capture_tests.rs` | 17 tests on captures generated in the test (temp dir; never game pixels) | capture.md §4–§8 |
| `crates/d2-client/capture-cases/*.toml` | `stability-0001`, `placement-0001`, `camera-0001`, `composition-0001` (capture.md §8), all `raw = "latest"` | §8 |
| `verify/case.rs` | `kind = "scene"`: `raw`, `images`, `check`, `draws` (strict) | §A10 |
| `verify/mod.rs` | `Status::SceneNotWired`, `Summary.scene_not_wired` (exit 2) | §A10 |
| `main.rs` | `verify` dispatches `CaseKind::Scene` (one arm + doc line) | |
| `crates/d2-client/Cargo.toml` | `png = "0.18"` (the image crate's copy), `sha2`, `serde_json` (workspace); no new download | |

The capture cases are in their own directory so a bare `d2-client
verify` (render-cases) is unchanged and never needs a recording.

## Tests (M08)

`cargo test -p d2-client --lib capture_tests`: 17 pass. The PNG reader
decodes the exact bytes `record_frames.py` `png_bytes` writes for its
selftest image (embedded hex) and reproduces the hashes `hashlib` prints
for it. Perturbation: `--perturb N` for N in 1, 7, 64, all pixels →
exactly N on CPU indices, GPU indices and GPU RGBA, all 6 frames fail;
N + 1 above the pixel count is an error; stability `--perturb 1..=7` →
exactly N re-hashes differ, 8 > 7 saved frames is an error; one byte
changed in one PNG on disk → exactly that frame. By hand: dropping the
palette comparison and relaxing "two keys seen twice" each failed a test.

## Recorder findings (record_frames.py not changed)

1. `FrameRecorder.stability()` counts every group, singletons included,
   and never applies §7's "at least two keys seen at least twice each",
   so its printed `0 with differing frames` can show while §7 does not
   hold. The `stability-0001` verify case applies the full rule; trust
   its verdict over the recorder's line.
2. The raw file does not name its image directory (header has no
   `images`); the case derives `game/captures/<stamp>` from the raw file
   name, which matches the recorder's defaults only. With `--out`
   elsewhere, give `images =` in the case.
3. Frames before the first server tick carry `"f": null` (`TickRecorder.
   frame` starts as `None`); the reader takes it as "no tick" and
   `repeated_ticks` ignores them.

## Local run queue (for `docs/HANDOFF.md` §5 A; not edited here)

`raw = "latest"` reads the newest recording, so run each verify right
after its recording (or set `raw` to the file).

1. `py tools/trace-recorder/record_frames.py --selftest` → `selftest ok`.
2. **stability-0001** (capture.md §8 row 1): `py
   tools/trace-recorder/record_frames.py --seconds 90`, player: Single
   Player, any character, Den of Evil cleared, dead end away from doors,
   stand still 30 s, no panels. Then `cargo run --release -p d2-client
   -- verify --cases crates/d2-client/capture-cases --case
   stability-0001`. Expect `re-hash: 0 of N frames differ`, `stability:
   G state groups, K seen at least twice, 0 with differing frames` with
   K ≥ 2, `PASS stability-0001 (scene)`, exit 0. Then the same with
   `--perturb 1`: `re-hash: 1 of N frames differ`, FAIL, exit 1. A
   differing group or K < 2 is a capture.md finding (Open question 1:
   the state key misses an input), not a reason to loosen the check.
   Record any `flag: … ticks carry more than one frame` lines (Open
   question 2).
3. **placement-0001**, **camera-0001**, **composition-0001** (§8 rows
   2–4): record each as the table says, then `… verify --cases
   crates/d2-client/capture-cases --case <id>`. Expect today: no ERROR
   (every PNG matches its record), `N frames selected`, `frames: 0 match,
   0 differ or fail, N scene not wired`, `SCENE NOT WIRED <id> (scene)`,
   exit 2. Once a `SceneSource` is wired: PASS, and `--perturb 5` →
   every frame `CPU: 5 of 480000 bytes differ`, FAIL, exit 1.

## Open questions

1. Who provides the `SceneSource`: the world view needs a `ClientWorld`
   (level, tiles, units) for the recorded frame, which the capture does
   not hold (only player and camera state). Either the recording grows
   a level / room / unit snapshot (capture.md §3 change) or the source
   replays the recorded game's seed through the local server; decide in
   the capture or world-view spec.
2. Initial framebuffer: `scene::compose` takes no previous frame (see
   State); the source gets one via `SceneJob::previous`.
3. Exit code 2 for `SCENE NOT WIRED`, as for `GPU NOT WIRED`.

## Gate run

`cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --
-D warnings`; `cargo test -p d2-client`; `cargo run -p depcheck`;
`python3 tools/spec_index.py --check`; `python3 tools/methods.py check`;
`python3 tools/coverage.py --check` (0 errors; capture.md 6 of 9 units
claimed at unit tier) and `--selftest`. Results in the commit message.
