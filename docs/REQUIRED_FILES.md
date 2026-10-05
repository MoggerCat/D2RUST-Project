# Required and helpful files

Nothing in this list gets committed to git. Game files go in `game/` and RE
work in `re/` (both gitignored). Reference sources go in `../refs/`, outside
the repo. Set `D2_GAME_DIR` to point at the install.

## Where things go

```
Claude code folder/
  d2rs/
    game/            copy of the 1.14d install: Game.exe, *.mpq (keep it unmodded)
    game/save/       your own .d2s saves (the game's own save folder)
    re/              Ghidra project and exports
  refs/
    D2MOO/           one folder per reference project, source directly inside
    riiablo/
    CE_Database/     Phrozen Keep ordinal/export names (1.10f)
    1.14d-notes/     community 1.14d address/struct lists
```

## 1. Required: original game files (legally owned, 1.14d)

### Data archives
- `d2data.mpq`: core classic data
- `d2exp.mpq`: Lord of Destruction data
- `d2char.mpq`: character graphics
- `d2sfx.mpq`, `d2speech.mpq`, `d2xtalk.mpq`: sound and speech
- `d2music.mpq`, `d2xmusic.mpq`: music
- `d2video.mpq`, `d2xvideo.mpq`: cinematics (optional for engine work)
- `patch_d2.mpq`: 1.14d patch data

These total about 2 GB and stay on your machine. Claude Code reads them
locally.

### Binaries for reverse engineering
- **1.14d (target):** `Game.exe` holds the game logic, client, graphics and
  networking code that 1.10f split across `D2Common.dll`, `D2Game.dll`,
  `D2Client.dll` and the other DLLs. Keep the few DLLs that ship with it.
- D2MOO's function map is for 1.10f and won't line up with 1.14d addresses.
  Community 1.14d address/struct lists bridge the gap.
- **1.10f binaries (optional, only if legally owned):** line up directly
  with D2MOO, which makes it easier to see what changed by 1.14d.

Keep `game/` unmodded (no PlugY, mods or loaders) so traces reflect the
original game. `traces/reference-install.toml` records which files they
came from.

### Saves and test data
- A set of your own `.d2s` character saves at various levels/classes, for
  save-format and stat-calculation tests.

## 2. Required: tools

| Tool | Purpose |
|---|---|
| Rust stable toolchain (rustup, cargo, clippy, rustfmt) | Engine |
| Bevy (pinned version, pulled by cargo) | Client only |
| Ghidra (+ the JDK it requires) | Decompiling `Game.exe`; can run headless |
| x32dbg (or similar 32-bit debugger) | Live observation, trace recording |
| Git | Version control (with the `.gitignore` provided) |
| Claude Code | Spec writing and implementation sessions |
| An MPQ extraction tool (or our own `d2-formats` once built) | Pull tables/assets locally |

## 3. Highly helpful references

- **D2MOO source** (MIT), in `../refs/D2MOO`: struct layouts, names and
  verified 1.10f logic. The single most valuable reference; confirm each
  behavior against 1.14d.
- **Community 1.14d address/struct lists and Ghidra labels**, in
  `../refs/1.14d-notes`.
- **Phrozen Keep file guides**: .txt column meanings, file format docs.
- **Riiablo source** (Apache-2.0): format parsers, client architecture.
- **OpenDiablo2 source** (GPL): read-only reference for formats.

## 4. Things to produce early (they pay off everywhere)

- `tools/trace-recorder`: captures original-game behavior (seeds, drops,
  map layouts, damage numbers) into `traces/` as JSON for conformance tests.
- A record of the reference install (`cargo run -p hash-manifest`). Dev
  only, not used to check players' files.
