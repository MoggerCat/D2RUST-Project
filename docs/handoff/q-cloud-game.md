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
