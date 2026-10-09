# Handoff: the original 1.14d Game.exe under Wine in the cloud — `claude/q-cloud-game`

> Not yet folded into `docs/HANDOFF.md` / `docs/PLAN.md`. The coordinator folds it (§5 queue entry and REC-290 below).

Cloud session, 2026-10-08. Goal: record traces of the original game in a
cloud container instead of only on PC 1 (M23: real data before green).
Base: `staging` at `aefd2b82`. No code in `crates/`; no game file
committed. Tools: `tools/cloud-game/` (README there).

## What works (proved in this session)

| # | Step | Proof |
|---|---|---|
| 1 | Wine + Xvfb in the container. `apt` reaches the Ubuntu archive. Versions: wine 9.0~repack-4build3 (`wine`, `wine32:i386`, `wine64`), xvfb 2:21.1.12-1ubuntu1.8, mingw-w64 11.0.1-3build1, imagemagick 8:6.9.12.98, x11-utils, xdotool (Ubuntu 24.04). The lines are in `tools/cloud-setup.sh` (`D2_NO_WINE=1` skips them). | `dpkg --add-architecture i386 && apt-get update && apt-get install -y --no-install-recommends wine wine32:i386 wine64 xvfb x11-apps x11-utils imagemagick xdotool mingw-w64 bc`: exit 0 |
| 2 | `run.sh`: Xvfb, a Wine prefix, a time limit, stdout/stderr, exit code, X screenshots, window list. | The stand-in: exit 7 (its own code), the 800×600 window titled "Diablo II" in `windows.txt`, the screenshot shows its bars and frame counter |
| 3 | The real `Game.exe` (sha256 `631066c1…`, the recorders' reference) starts under Wine with `-w -ns` and opens its window. | With the partial install (below) it stops at its own **"Insert Play Disc"** dialog: expected, `d2music.mpq` / `d2video.mpq` were not uploaded yet |
| 4 | Windows Python under Wine: CPython 3.12.7 x64 from NuGet (`setup_winpy.sh`; python.org is blocked by the network policy, NuGet is not). | `3.12.7 … 64 bit (AMD64)` |
| 5 | The recorder's Win32 debugger under Wine, unchanged: `wine_probe.py` runs `record_rng.py`'s `Recorder` on the stand-in (CreateProcessW DEBUG_ONLY_THIS_PROCESS, INT3 at helpers / setter / inline site, Wow64Get/SetThreadContext, single-stepping through `mul` / `add` / `adc`, kill-on-exit), then `check_rng.py`. | 166 events (1 seed set, 33 step, 99 roll of which 33 no-draw, 33 inline), `OK: every recorded draw follows specs/sim/rng.md`, 0 foreign exceptions |
| 6 | autostart's Win32 side: window found by pid (`EnumWindows`), `PostMessageW` click and key delivered, `WM_CLOSE`. | The stand-in prints `click 600 300` and `key 27`, exits 7 |
| 7 | `record_rng.py` on the **real Game.exe** under Wine: the process starts under the debugger, the 1.14d byte checks pass, every hook is planted, the time limit kills the game. | `6 helpers, 3 setters, 846 inline sites hooked` (846 = `specs/sim/rng.md` §3), `time limit 12.0s reached`, no Game.exe left; 0 draws (the game sits at the disc dialog) |
| 8 | The recorders' self-tests under Windows Python in Wine: `autostart.py --selftest`, `record_frames.py --selftest`. | Both exit 0 |
| 9 | `fetch.sh`: sparse blob-less clone of the private repo, piece check against `install/manifest.json`, rebuild with its `assemble.py`. | `--partial` at private-repo `6b18585`: 20 files rebuilt, 0 sha256 mismatches, 3 skipped (pieces not uploaded) |

## Round 2 (2026-10-08/09): the full install under Wine

Private repo `5174f0c` (`install: complete` `6a5b2aa7` + `extracted:
complete`); merged `origin/claude/specs-staging-7` (`40e9e1ec`).

### Step 1 — menu and a game: works

| Check | Result |
|---|---|
| `fetch.sh` | 23 files → `$HOME/game`, 0 mismatches |
| `run.sh --seconds 20 --shot-at 15` | the LoD title screen at 15 s (800×600 window). The very first run after the install showed no window in 30 s (first-run set-up; not seen again) |
| `prepare_saves.sh` | `ScnAma`, `ScnSor`, `TestSor` written and `check` OK in the prefix's `Saved Games\Diablo II` |
| `autostart.py --try ScnAma --seed 1234` | menu left at 6.0 s, player in level 1 (Rogue Encampment) at 6.5 s, position [4873, 4228], act init seed 1234 (PC 1: arrival ≈ 6.3 s) |
| `record_frames.py --auto ScnAma --seed 1234 --every 25 --draws-every 200 --input "wait 8; end"` | 11 frames, 154 ticks in 17 s; every frame video type 1 (GDI), 800×600, not refused, level 1 act 0; draw logs of 214 (seq 1) and 297 (seq 9) calls; the PNGs (`game/captures/<time>/`, gitignored) show the town with Warriv and the player: the framebuffer path replaces PrintWindow |

Under Wine the game also logs ALSA "cannot find card" lines (no sound
device; `-ns`) and wined3d "no GL" errors (unused: `-w` is GDI). Both
harmless.

### Step 2 — REC-290 (Wine fidelity): RNG equal to PC 1; ticks repeatable

**RNG against PC 1.** The committed RNG traces `traces/sim/rng/sim-0001`–`0005` come
from one hand-played PC 1 run (`20261005-232125-rng.jsonl`). Three of their
seeds do not depend on the clock: the automap seed `{0, 666}` (sim-0001),
the DRLG seed from the character's map seed 644409375 (sim-0003), and the
level-2 seed derived from it (sim-0005). A new expansion character
`RecMap` (`d2s-tool new --name RecMap --class ama --expansion --map-seed
644409375`) under Wine:

```
tools/cloud-game/run.sh --python --seconds 1500 -- tools/trace-recorder/record_rng.py \
    --game "$D2_GAME_DIR/Game.exe" --seconds 1400 --auto RecMap --input "wait 3; end" --out rng.jsonl
python3 tools/cloud-game/rec290_rng.py rng.jsonl
```

| Trace | Result |
|---|---|
| sim-0001 automap seed | **EQUAL** 64 of 64 events (every field, `site` too); the Wine chain has 737 draws, as PC 1's note says |
| sim-0003 DRLG seed | **EQUAL** 7 of 7 |
| sim-0005 level 2 rooms | **EQUAL** 64 of 64; the Wine chain has 3,899 draws, as PC 1's |
| sim-0002 game seed, sim-0004 monster-region seed | no chain: both start from the time-based game seed (expected; a fixed `-seed` run on both sides would compare them) |

27,065 records (24,207 inline steps). With every inline hook the arrival
took 192.8 s (PC 1: about 23 s), so Wine is about 8× slower in this mode.

**Ticks.** No PC 1 tick trace exists for a scripted start (sim-0006–0008
are hand-played), so there is nothing to compare yet. Under Wine,
`record_tick.py --auto ScnAma --seed 1234 --ticks 600` gives 601 ticks
(arrival at 7.0 s, near full speed), and `check_tick.py` passes: 1,017
timer runs, 25 snapshots, 0 errors. A **second identical run is byte-equal**
in every record except `ms` (14,436 records; heap addresses included).
Wine runs therefore repeat each other.
Still open for REC-290: the same command on PC 1, compared record by
record (the script above, with `ms` and the header dropped).

`tools/cloud-game/rec290_rng.py RAW [TRACE...]` splits the raw file with
`convert_rng.py`'s own chain rule, converts with its `draw_data`, and
compares each trace's `expected` in order. Exit 1 on any difference.

### Step 3 — `frames-raw-3` and `facts_render.py` 0.2.0: works

Merged `origin/claude/local-pc1-facts` (for `facts_render.py`).
`record_frames.py` 0.3.0 writes `frames-raw-3` (`capture.md` §2, §3.5,
§3.6, §5):

- **cel header per cel draw**: hook on the common rasterizer `0x006014C0`
  (its first argument is the cel context; the drawn cel is context
  `+0x3C`: orientation, w, h, xoff, yoff), attached to the cel op just
  logged;
- **unit component paths**: two hooks right after the path `sprintf` in
  `0x005FE610` (`0x005FE77C` .dcc, `0x005FE7A8` .dc6) emit `compfile`
  `{name, path}`; a draw's five context tokens compose the name;
- **DT1 file and tile index** per tile draw, from the loaded-DT1 list
  `[0x008ADBB4]`;
- **player direction** (path `+0x64`).

`facts_render.py` was rewritten to the exact `facts-render.md` §1–§4
columns that `d2-client facts-compare` reads (the PC 1 version wrote its
own columns, which `facts-compare` refuses).

Live run under Wine (`record_frames.py --auto ScnAma --seed 1234 --every
25 --draws-every 100`), frame seq 1, 214 draws:

| Measure | Result |
|---|---|
| tile draws named by DT1 file + index | 134 of 134 |
| cel draws with a header | 50 of 53. Without: the empty shield component (`AMSHlitTN1ht`), and the two `Hlthmana` orbs drawn by `CelDrawEx`, which does not reach the rasterizer |
| `compfile` records | 1,451 |
| headers against the real files (private `extracted/`) | 12 DC6 frames (`800ctrlpnl7` 0/1/4/5, `overlap` 0/1, `minipanelbtn` 0/14, `Skillicon` 2, `level` 0/2, `protate` 0): **all equal** w, h, xoff, yoff |
| `facts_render.py` | 214 rows, 38 sprite rows; `?` cells: file 8 (font cels of `0xb5730c`, not loaded through `0x004788B0`), w/h/xoff/yoff 11, mode 13, light/pal 12 |

`--selftest` passes for both tools (perturbation of each new source
field). Spec updates: `capture.md` (status, hooks, §3.5 cel header and DT1
lookup, §3.6 `compfile`, §5 raw-3, OQ 3 / 4 done), `facts-render.md`
(inputs, §2 r3 component files and the CelDraw argument order, §4 r1
source, OQ 3 / 4 answered, OQ 5 primitive colour argument).

### Step 4 — scene compare: the tools work; d2rs cannot draw the scene yet

**Scene.** `a1-town-arrival-ama`: ScnAma (`d2s-tool new --class ama
--expansion`), `-seed 1234`, idle at the arrival point in the Rogue
Encampment, player at client (10320, 72816). This is the arrival point,
not "by the waypoint": d2rs `play` has no input script, so arrival is the
one place both sides can show. The 1.14d facts are committed in
`facts/render/scenes/a1-town-arrival-ama/` (frame seq 51, tick 73, 267
draws) and `facts/render/sprites.tsv` (39 rows):

```
tools/cloud-game/run.sh --python --seconds 400 -- tools/trace-recorder/record_frames.py \
    --game "$D2_GAME_DIR/Game.exe" --seconds 300 --every 1 --draws-every 10 --ticks 120 --auto ScnAma --seed 1234
python3 tools/trace-recorder/facts_render.py <capture> --scene a1-town-arrival-ama --frame 51
```

Format check: `d2-client facts-compare <scene> <scene>` → `PARTIAL` (no
difference; only `?` cells), so `facts-compare` accepts the converter's
output.

**d2rs side.** Built with `cargo build --release -p d2-client`. It runs
under Xvfb on Mesa lavapipe (`mesa-vulkan-drivers`, plus
`libxkbcommon-x11-0` and Bevy's X libraries; adapter "llvmpipe"):

```
DISPLAY=:98 WGPU_BACKEND=vulkan target/release/d2-client play --save ScnAma.d2s --seed 1234 \
    --dump-draws DIR --at-tick N
```

| `--at-tick` | Result |
|---|---|
| 73 (the 1.14d frame), 3 | **fails**: `export: compose: draw item 653: drawn area Rect { x: 560, y: 296, width: 32, height: 32 } leaves the gradient block Rect { x: 560, y: 296, width: 32, height: 15 }` (exit 101; the same error stops the play window's frame system). A d2rs bug in `world_view` composition on the real install; not fixed here (no code in `crates/`) |
| 1 | dump written: 6 items (the control panel `800ctrlpnl7.dc6` frames 0–5 only; no world yet) |

**First difference** (`facts-compare` 1.14d seq 1 tick 3 vs d2rs tick 1,
`--ignore tick`): `DIVERGED` at `draws.tsv` row 0, column `op`. The original
has `StartDraw` (`1 0`, at `0x44cad5`) and d2rs has `CelDraw
800ctrlpnl7.dc6`. Before that stage, the frame inputs d2rs measures
(`w`, `h`, `open_mode`) are equal, and its 6 sprite rows (`800ctrlpnl7`
frames 0–5: 117×104, 128×55 ×3, 86×55, 117×104, offsets 0) equal
1.14d's. Two findings for the d2rs side (owner: `d2-client`):

1. The exporter writes no `StartDraw`, `ClearScreen` or primitive rows
   (`facts-render.md` §5 r1 lists only draw items), while 1.14d's list
   starts with `StartDraw` and has `DrawBox`, `DrawLine`, `ClearScreen`
   rows. Position matching (§6 r2) therefore diverges at row 0 of every
   scene. Fix either the exporter (emit the frame's start / clear /
   primitive rows) or §6 (skip the ops d2rs does not model).
2. The composition error above blocks every populated frame of this
   scene. It comes first: no world draw can be compared until it is
   fixed.

The 1.14d frame's cursor (`protate.dc6` at 320, 240) and rain (`rain 1`)
are known extra rows on the original side (`facts-render.md` edge cases).

### Round 3 — exporter rows, the waypoint scene, the first real differences

The coordinator gave the exporter gap to this branch (d2-client edits allowed).

**Exporter (`d2-client::facts::export`, `facts-render.md` §5 r1, r9–r11)**:
- `StartDraw(bClear = BlankScreen, 0, 0, 0)` first and `ClearScreen(0)` last
  (when the plan clears after drawing), as `composition.md` §3 and the 1.14d
  log;
- pass 9's weather calls as one `DrawLine` / `DrawBox` row each (d2rs draws
  them as 1×1 pixel items; `WorldFrame::sky` carries the calls) with
  `mode` = color (`blend-modes.md` §8). `facts_render.py` fills the same
  column from the 1.14d arguments;
- §5 r1 merge fixed. A tile's blocks are separate frames with their own
  offsets, so consecutive tile items giving the same row (same tag) are now
  one row (1,995 floor rows → 94);
- `play --input "wait N; move X Y; click X Y; rclick X Y"` (server ticks):
  scripted pointer input, the window pointer ignored while it runs.

Gate on each push: fmt, clippy `-D warnings`, nextest (2,296 passed after
the staging merge), `coverage.py`, `spec_index.py`.

**Scene `a1-town-waypoint-ama`** (1.14d, Wine): `--input "wait 1; click
700 300; wait 15; click 650 300; wait 15; move 320 240; wait 6; end"` →
player idle by the waypoint at (10864, 72848), dir 47, cursor at
(320, 240): frame seq 91, tick 159, 301 draws. The same end position on 3
runs. The d2rs counterpart: `play --input "wait 25; click 700 300; wait
150; click 650 300; wait 150; move 320 240"`. Not run yet: d2rs drops
most frames (below).

**Compare `a1-town-arrival-ama`** after the crash fix (d2rs: dev build,
`play --save ScnAma.d2s --seed 1234 --dump-draws DIR --at-tick 73`; the
first drawn frame was tick 8,569, player still idle at arrival;
`facts-compare ... --ignore tick`):

- Equal: every frame input d2rs measures (act 0, level 1, tile origin
  9920/72536, unit origin 9920/72532, open mode 0, shift 0), the palette
  sha256, row 0 `StartDraw 1 0`, and floor rows 1–15 in file and position.
- **First difference: `draws.tsv` row 1, column `frame`.**
  `act1/town/floor.dt1` at (800, −56): 1.14d tile **132**, d2rs **39**.
  Both carry the header key 0.0.0.1, shared by 12 of the file's 144 tiles
  (35–44, 132, 133): the variant pick differs. Of the 88 positions both
  draw, the file is equal 88 times and the index 32 times. → queue
  `q-fix-render-real-floor-variant`.
- With `--ignore tick,frame`: first difference at row 16, column `x`
  (1.14d (160, −56), d2rs (800, 184)). d2rs omits the left column
  x = −80 (8 tiles), so its floor walk differs. →
  `q-fix-render-real-floor-range`.
- Totals: units 25 vs 2, walls 32 vs 13, CelDraw 42 vs 18, rain lines 26
  vs 0. The d2rs log says "1 units in the model". →
  `q-fix-render-real-town-units`.
- Most d2rs frames are dropped ("block shade … is not a block of the tile
  in order"), and `play` panics at `app/play.rs:557` after the dump's exit.
  → `q-fix-render-real-frames-dropped`.

## What the recorders need from Windows, and the Wine plan

Read: `tools/trace-recorder/README.md`, `record_rng.py` (the Win32 base),
`record_tick.py`, `record_frames.py`, `autostart.py`.

| Need | Used by | Under Wine |
|---|---|---|
| Python ≥ 3.10 for Windows, standard library + `ctypes` | all | NuGet CPython 3.12.7 x64 (`setup_winpy.sh`). Must be **64-bit**: the debugger reads the 32-bit game through `Wow64GetThreadContext` / `Wow64SetThreadContext` (WOW64_CONTEXT, `STATUS_WX86_*` codes). Works (row 5, 7) |
| kernel32 debug API: `CreateProcessW` (DEBUG_ONLY_THIS_PROCESS), `WaitForDebugEvent`, `ContinueDebugEvent`, `DebugSetProcessKillOnExit`, `Read/WriteProcessMemory`, `VirtualProtectEx`, `FlushInstructionCache`, `Suspend/ResumeThread`, `TerminateProcess`, `WaitForSingleObject`, `CloseHandle` | `record_*.py`, `dump_tables.py` | Works (rows 5, 7). Throughput about **1,160 debug events/s** (stand-in `--busy`): the hook-heavy modes (`record_rng.py` with inline hooks: about six events per draw) will be much slower than on PC 1; `record_tick.py` / `record_frames.py` should be usable, measured once the install lands |
| user32 / gdi32: `EnumWindows`, `GetWindowThreadProcessId`, `IsWindowVisible`, `PostMessageW`, `MapVirtualKeyW` | `autostart.py` (`--auto`, `--input`) | Works (row 6) |
| `PrintWindow` + `GetDIBits` | `autostart.py` `shot` | **Black image** under Wine 9.0. Only for looking (pixel captures are `record_frames.py`, which reads the framebuffer from memory); use `run.sh --shot-at` X screenshots instead. Open: a `BitBlt`-from-window-DC fallback in `autostart.screenshot_png` |
| Game.exe GDI driver at 800×600 (`-w`, display type 1; registry `Resolution` missing → 1 = 800×600, `ui/frontend-loading.md`) | `record_frames.py` (`capture.md` §1) | `-w` opens a GDI window under Wine; the framebuffer read is process memory, independent of Wine's drawing. To verify with the full install: the `res_mode` / refused fields |
| Expansion characters in the save folder (`ScnAma`, `ScnSor`, `TestSor`) | `--auto` | Not in the private repo. `prepare_saves.sh` makes them with `d2s-tool new … --expansion` in `$WINEPREFIX/drive_c/users/$USER/Saved Games/Diablo II` (the folder 1.14d uses without `NewSavePath`, `tools/original-hooks.md` §5.3 rule 2a). Needs the full install (d2s-tool reads `arena.bin` from d2exp.mpq) |
| A console for Python's std streams | all | Windows Python fails at start-up (`init_sys_streams: Invalid handle`) when stdout or stderr is a file or pipe; `run.sh --python` gives it a pty with `script` |
| Paths | all | Unix paths given to Windows Python resolve through drive `Z:`; `--game "$D2_GAME_DIR/Game.exe"` (the default `game/Game.exe` in the repo also works if `game/` is a link to the install) |

`winedbg` was not needed: the recorders' own debugger runs under Windows
Python unchanged.

## What is open

1. **The install is incomplete** at private-repo `6b18585`: `d2exp.mpq`,
   `d2music.mpq`, `d2video.mpq` pieces missing (PC 1 is uploading). Until
   then Game.exe stops at "Insert Play Disc".
2. Everything after the menu is unproved under Wine: the menu itself, the
   `--auto` start, arrival in the Rogue Encampment, the GDI framebuffer
   and palette reads of `record_frames.py`, recorder speed on the real game.
3. **REC-290 (Wine fidelity of recordings)**: a recording under Wine is
   only usable if the game logic in it equals a Windows recording of the
   same run. The game's own outcomes (RNG, ticks, packets) should not
   depend on the OS, but Wine timing (frames per tick, `GetTickCount`
   pacing, debugger slowdown) can change *which* ticks a scripted input
   lands on. Check: `record_tick.py --auto ScnAma --seed 1234 --ticks 600`
   (no input script) on PC 1 and in the cloud; `check_tick.py` passes on
   both, and the two `tick`/`ex`/`snap` sequences are equal (the `ms`
   fields and thread ids aside). Until it passes, cloud recordings are
   "unverified" (CLAUDE.md rule 10) and do not replace PC 1 ones.
4. `PrintWindow` black under Wine (above): autostart `shot` PNGs blank.
5. `tools/cloud-setup.sh` now installs Wine by default (about 1 GB, a few
   minutes); environments that never run the game can set `D2_NO_WINE=1`.

## Commands once `install:` is complete

```sh
sh tools/cloud-setup.sh                                  # if Wine is not installed yet
tools/cloud-game/setup_winpy.sh                          # Windows Python -> $HOME/winpy
tools/cloud-game/fetch.sh                                # full install -> $HOME/game, exit 0, 0 mismatches
export D2_GAME_DIR=$HOME/game
tools/cloud-game/prepare_saves.sh                        # ScnAma, ScnSor, TestSor: `check` OK

# 1. the game reaches its menu (look: shot-20.png shows the main menu, not "Insert Play Disc")
tools/cloud-game/run.sh --seconds 25 --shot-at 5,12,20

# 2. unattended start to the Rogue Encampment (look: "arrival" line with level 1, exit 0)
tools/cloud-game/run.sh --python --seconds 90 --shot-at 20,40 -- \
    tools/trace-recorder/autostart.py --game "$D2_GAME_DIR/Game.exe" --try ScnAma --seed 1234

# 3. tick recording + check (REC-290 left half; compare with the same command on PC 1)
tools/cloud-game/run.sh --python --seconds 300 -- \
    tools/trace-recorder/record_tick.py --game "$D2_GAME_DIR/Game.exe" --seconds 240 --ticks 600 --auto ScnAma --seed 1234
tools/cloud-game/run.sh --python --seconds 120 -- tools/trace-recorder/check_tick.py traces/raw/<time>-tick.jsonl

# 4. frames (look: frames not "refused", PNGs in game/captures/<time>/ show the town)
tools/cloud-game/run.sh --python --seconds 300 -- \
    tools/trace-recorder/record_frames.py --game "$D2_GAME_DIR/Game.exe" --seconds 120 --auto ScnAma --seed 1234
```

`check_*.py` are plain Python and also run with Linux `python3` directly.
`autostart.py --try` takes `--game`, `--seed`, `--seconds` (checked).

## Proof commands for the stand-in (no game files)

```sh
tools/cloud-game/standin/build.sh /tmp/standin
tools/cloud-game/run.sh --exe /tmp/standin/standin.exe --seconds 20 --shot-at 4 -- -w -ns -seconds 6
tools/cloud-game/run.sh --python --seconds 60 -- tools/cloud-game/wine_probe.py /tmp/standin/standin.exe /tmp/standin/standin.syms
tools/cloud-game/run.sh --python --seconds 60 -- tools/cloud-game/wine_probe.py /tmp/standin/standin.exe /tmp/standin/standin.syms --busy   # throughput
```
