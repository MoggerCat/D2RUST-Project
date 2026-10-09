# cloud-game: the original 1.14d Game.exe headless under Wine

Runs the original `Game.exe` (and the trace recorders, which are Windows
Python debuggers) in a Linux cloud container: Wine 9.0 on an Xvfb virtual
display. The install comes from the private data repo (CLAUDE.md rule 1);
nothing here contains or writes game files into this repository. Our own
code. State and open points: `docs/handoff/q-cloud-game.md`.

| File | Does |
|---|---|
| `fetch.sh [--partial] [DEST]` | Sparse clone (or update) of `MoggerCat/D2RUST-private-repo`, checks every piece in `install/manifest.json` is present, rebuilds the install with its `tools/assemble.py` (size + sha256 per file) into `DEST` (default `$HOME/game`); `--partial` rebuilds only complete files while an upload is in progress |
| `run.sh [opts] [-- ARGS]` | Starts Xvfb (`:99`, 1024×768×24) and a Wine prefix (`$HOME/.wine-d2`), runs `Game.exe -w -ns` (or `--exe PATH`) with a time limit, keeps stdout/stderr, exit code, X screenshots at `--shot-at` times, the X window list, `run.json`. `--python`: runs Windows Python with a script (the recorders) |
| `setup_winpy.sh [DEST]` | Windows CPython 3.12.7 x64 from NuGet (python.org is blocked here) into `$HOME/winpy`, sha256-pinned |
| `prepare_saves.sh` | The recorders' `--auto` characters (`ScnAma`, `ScnSor`, `TestSor`, expansion) made by `d2s-tool` in the prefix's `Saved Games\Diablo II` (needs the full install: d2s-tool reads the tables) |
| `standin/standin.c`, `standin/build.sh` | Stand-in for `Game.exe`: a 32-bit mingw program at image base 0x400000 with a window, click/key echo, and RNG steps of the `specs/sim/rng.md` shape (two helpers, a setter, one inline site) |
| `xinput.sh "T X Y; T key K; T text S"` | X-level input on the Xvfb display at second T after its start (window client point X, Y; the window sits at 112, 98): drives the 1.14d front end, which ignores `PostMessage` clicks under Wine (run it in the background next to `run.sh`, with the recorder's `--menu`) |
| `prov_recording_groups.py [G..]` | the `recording` provisional points grouped by settling scenario (`docs/handoff/q-prov-recording.md`) → `docs/handoff/q-prov-recording.tsv` |
| `wine_probe.py` | Under Windows Python: `record_rng.py` unchanged, pointed at the stand-in, then `check_rng.py`; then autostart's window side (find window by pid, PrintWindow, PostMessageW click/key, WM_CLOSE) |

## Setup (once per container)

```sh
sh tools/cloud-setup.sh                      # apt: wine, wine32:i386, xvfb, mingw-w64, ... (D2_NO_WINE=1 skips)
tools/cloud-game/setup_winpy.sh              # Windows Python under Wine
tools/cloud-game/fetch.sh                    # install -> $HOME/game
export D2_GAME_DIR=$HOME/game
tools/cloud-game/prepare_saves.sh            # characters for --auto
```

## Use

```sh
tools/cloud-game/run.sh --seconds 30 --shot-at 5,15,28            # Game.exe -w -ns, screenshots
tools/cloud-game/run.sh --python --seconds 120 -- \
    tools/trace-recorder/record_tick.py --game "$D2_GAME_DIR/Game.exe" --seconds 60 --auto ScnAma --seed 1234
# stand-in proof (no game files):
tools/cloud-game/standin/build.sh /tmp/standin
tools/cloud-game/run.sh --python --seconds 60 -- tools/cloud-game/wine_probe.py /tmp/standin/standin.exe /tmp/standin/standin.syms
```

Outputs land in `$HOME/cloud-game-runs/<time>/` (or `--out`); recorder
traces in `traces/raw/` and captures in `game/captures/` as on PC 1 (both
gitignored).

## Wine notes (measured 2026-10-08, wine-9.0 Ubuntu 9.0~repack-4build3)

- Windows CPython exits at start-up with `init_sys_streams: Invalid handle`
  when stdout or stderr is a file or pipe; with a pty it runs. `run.sh
  --python` runs it under `script` and writes `stdout.clean.txt`.
- `PrintWindow` returns a black image (autostart's `shot` PNGs are blank);
  `run.sh`'s X screenshots (`import -window root`) show the screen.
- Debug-event throughput through the Win32 debug API: about 1,160 events/s
  (stand-in `--busy`), so hook-heavy recorders run slower than on PC 1.
- `wine: Read access denied for device Z:\` lines on stderr are harmless.
- There is no window manager: windows have no frame and sit at their
  creation position.
