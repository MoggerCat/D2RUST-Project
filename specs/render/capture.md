# Spec: Render — Frame capture from 1.14d (design of link 1)

- **Status:** draft (2026-10-06). Recorder written
  (`tools/trace-recorder/record_frames.py`, `--selftest` passes: PNG round
  trip and hash sensitivity); never run against the game yet.
- **Target version:** 1.14d
- **Crate/module:** `tools/trace-recorder/record_frames.py` (capture),
  `d2-client::verify` case kind `scene` (comparison)
- **Related specs:** `render/composition.md` (framebuffer, frame cycle,
  palette), `render/camera.md` (state recorded with each frame),
  `client/render-pipeline.md` §A10 (link 1), `traces/FORMAT.md` §Render
  captures (committed format), `sim/tick.md` (server tick hook)

## Summary

A capture is the exact index frame 1.14d presented, read from process
memory at the in-game `EndScene` call, with its palette, the server tick it
follows and the camera/player state it was drawn from. Images stay local
(Blizzard art); the repo keeps hashes and state. Stability is proven
first: frames drawn from the same recorded state must be identical.

## Inputs

| Name | Type | Source |
|---|---|---|
| reference `Game.exe` | 1.14d, SHA-256 checked | `traces/reference-install.toml` |
| game arguments | `-w -ns` | GDI driver (`composition.md` §1) |
| player actions | per capture case (§8) | the person at the game |

## Outputs / state changes

`traces/raw/<time>-frames.jsonl` (gitignored, format `frames-raw-1`, §5),
`game/captures/<time>/frame-<draw>.png` (gitignored), and, after
selection, committed traces under `traces/render/` (`FORMAT.md`).

## Rules

### 1. Configuration

`Game.exe -w -ns`: display type 1 (GDI, `[0x007C8CB0] = 1`), resolution
800 × 600 (`[0x007C9138] = 800`, `[0x007C913C] = 600`; the in-game
resolution option must be 800 × 600). Frames in any other configuration
are logged as refused, never captured.

### 2. Where the frame is read

INT3 on the first byte of `EndScene` `0x004F6190` (first bytes
`55 8B EC 83 EC 10`). Only in-game frames are taken: the return address
`[ESP]` must be `0x0044CB4F` (the call at `0x0044CB4A` in the in-game draw
`0x0044C990`). At that point every draw of the frame is done and the
present (`Blit`, slot `+0x20`) has not run (`composition.md` §3), so the
memory read equals the presented frame.

### 3. What is read

| Item | Where | Notes |
|---|---|---|
| index frame | `W × H` bytes at `[0x007C9154]` | row-major, top row first, stride W |
| palette | color table `0x00989C40`, 256 × (B, G, R, 0) | stored as R, G, B (768 bytes) |
| tick | frame number `f` of the last server tick hook `0x0052D870` (`record_tick.py`) before this frame | §4 |
| draw counter | `[0x007A0494]` | in-game draws so far (main path) |
| player | unit `[0x007A6A70]`: type `+0x00`, mode `+0x10`, `+0x44`; path `+0x2C`: fixed `+0/+4`, client `+8/+0xC` | `camera.md` §2 |
| tile origin, view rect | view `[0x007A0640]` `+0x24/+0x28`, `+0x04..+0x10` | `camera.md` §1, §3 |
| unit origin | `[0x007A520C]`, `[0x007A5208]` | `camera.md` §3 |
| panel | open mode `[0x007A5210]`, shift `[0x007A5214]` | `camera.md` §1 |
| shake | `[0x007B9534]`, `[0x007B9538]`, `[0x007B8D20]` | `camera.md` §8 |
| post-draw clear counter | `[0x0070F2C0]` | `composition.md` §3 |

### 4. Tie to ticks

In single player the client loop runs server tick, client update and
draw in one pass, one draw per tick (`camera.md` §9). A capture's `tick` is
the `f` of the most recent server tick of the recorded game that started
before the `EndScene` hit. Two captures with the same `tick` are a pause
or a skipped tick and are flagged. The recorded shake offsets are an input
of the comparison (the original's envelope runs on wall-clock time).

### 5. Raw format `frames-raw-1`

JSON lines. Header `{"k": "header", "format": "frames-raw-1", "tool",
"date", "game_exe_sha256", "args", ...}`; base-recorder `game` and `tick`
records (`record_tick.py`); per capture `{"k": "frame", "f", "video_type",
"w", "h", "index_sha256", "palette_sha256", "draw", "player", "view_rect",
"tile_origin", "unit_origin", "open_mode", "shift_x", "shake",
"clear_counter", "res_mode", "image"?}` or the same with `"refused"`;
footer with counts and notes. `image` names an 8-bit palettized PNG
(color type 3) whose IDAT holds the index bytes unchanged and whose PLTE
holds the 768 palette bytes.

### 6. Hashes and the comparison

- `index_sha256` = SHA-256 of the W × H index bytes (row-major, top row
  first). `palette_sha256` = SHA-256 of the 768 bytes R, G, B for indices
  0–255.
- Link 1 (`render-pipeline.md` §A10): the CPU reference composes the
  recorded state; the case passes when its index frame is byte-identical
  to the capture's (PNG indices and `index_sha256`) and its palette
  bytes equal the capture's. RGBA equality follows (`composition.md` §6).
  The report gives the mismatched pixel count and the first mismatch;
  `--perturb N` must fail with exactly N (M08).

### 7. Stability first

Before any other capture case counts, case `stability-0001` (§8) must
show: frames whose recorded state key (player record, tile origin, unit
origin, shake, open mode, palette hash) is equal have equal
`index_sha256`, with at least two keys seen at least twice each.
`record_frames.py` prints the group count and the groups with differing
frames; a failing group means the capture point or the state key misses
an input (fix the spec before any other case). Perturbation: changing one
byte of one saved PNG must make a re-hash differ for exactly that frame.

### 8. Capture cases

Each case: start `py tools/trace-recorder/record_frames.py --seconds S`
(default arguments `-w -ns`), then in the game window:

| Case | Player does | Proves |
|---|---|---|
| `stability-0001` | Single Player, any character, enter the Den of Evil, kill every monster, walk to a dead end away from doors, stand still 30 s, no panels open | §7 |
| `placement-0001` | Same place: stand still 5 s, open and close the inventory (right panel), stand 5 s | `sprite-placement.md` §2, §8; `camera.md` §1 modes 0/1 |
| `camera-0001` | Rogue Encampment: walk (not run) in a straight line 5 s, then run 5 s, then stand | `camera.md` §2–§4, §6 |
| `composition-0001` | Rogue Encampment: cast Town Portal (scroll), stand next to it 5 s | `composition.md` §5 (translucent draw) |

The debugger slows the game; a frame still shows the state of its tick,
but slow passes may skip draws (Open question 2).

## Constants & data dependencies

Addresses in §2–§3; frame size 800 × 600.

## Randomness

None in the capture. The scenes contain the game's own randomness; cases
compare against the recorded state, not a replay.

## Edge cases & original bugs

- The bottom 47 rows carry content from older frames (`composition.md`
  §3): a case's first captured frame may depend on frames before
  recording started; comparisons start from the second captured frame or
  record the previous frame as the initial buffer.
- Frames with `clear_counter > 0` are all index 0.

## Test vectors

| Input | Expected output | Source |
|---|---|---|
| `record_frames.py --selftest` | "selftest ok" (PNG keeps indices and palette; 1-byte change changes the hash) | §5, §6 |
| `stability-0001` run | every state group has one `index_sha256` | §7, queued |

## Provenance

Hook and memory locations from 1.14d `Game.exe` (`composition.md`
Provenance; `0x0044CB4A` call site, `0x0044EFA0` loop, `0x007A0494`
counter increment at `0x0044F28B` path, player global read by `0x00463DD0`).
Recorder design follows `tools/trace-recorder/record_tick.py`.

## Open questions

1. Whether the client player unit's `+0x44` is its animation frame on the
   client copy (as on server units, `sim/units.md`); if not, the state key
   of §7 needs the right field. The first `stability-0001` run shows it
   (groups that never repeat).
2. Whether the debugger slowdown makes the client loop skip draws
   (`camera.md` §9) often enough to matter for `camera-0001`: the run's
   `tick` sequence shows it.
