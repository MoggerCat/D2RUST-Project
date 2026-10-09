# Playtest on Windows (no Rust build)

Every push to the staging branch (`claude/specs-staging-7`) and to `main`
builds a Windows `d2-client.exe` in GitHub Actions
(`.github/workflows/windows-build.yml`). The download holds only our
compiled code; the game reads **your own Diablo II 1.14d install**
(`CLAUDE.md` hard rule 1).

## 1. Download

1. Open the workflow's runs:
   <https://github.com/MoggerCat/D2RUST-Project/actions/workflows/windows-build.yml>
   (signed in to GitHub; artifacts need a login).
2. Click the newest green run of the branch you want.
3. Under **Artifacts**, download `d2rs-windows-<commit>` (a zip, kept 14
   days). To build a branch on demand: **Run workflow** on the same page.

The zip holds `d2-client.exe`, `d2_client.pdb` (symbols: file and line
numbers in the crash log) and `README-PLAY.txt`.

## 2. Where to put it

The client looks for the game, in this order:

1. `--game-dir <folder>` on the command line;
2. the `D2_GAME_DIR` environment variable;
3. the folder of `d2-client.exe`, then the current folder, if it holds
   `d2data.mpq`.

Simplest: unzip `d2-client.exe` and `d2_client.pdb` into your Diablo II
folder (the one with `Game.exe` and `d2data.mpq`) and double-click
`d2-client.exe`. It never writes into that folder except the crash log.

Elsewhere: `d2-client.exe --game-dir "C:\Program Files (x86)\Diablo II"`.

A console window opens next to the game window; it shows the log
(`game folder: ...`, `play: ...`). On an error it stays open until Enter.

## 3. Start a character or load a save

Double-click starts at the main menu: **Single Player** opens character
select (saves in the save folder) and create.

From a command prompt (skips the menu):

| Command | What |
|---|---|
| `d2-client.exe --new sorceress MyName` | new character (amazon, sorceress, necromancer, paladin, barbarian, druid, assassin; name 1–15 of A–Z a–z 0–9 `-` `_`) |
| `d2-client.exe --save "C:\...\MyName.d2s"` | load a save |
| `--difficulty normal\|nightmare\|hell` | difficulty |
| `--res 640x480` | the 640 × 480 frame (default 800 × 600) |
| `--seed N` | a fixed game seed (to replay a bug) |

Saves go to `Documents\d2rs\saves` (`--save-dir` to change), never into
the game folder. Original 1.14d `.d2s` files can be loaded with `--save`;
copy them first, since d2rs writes back to the file it loaded.
Settings: `Documents\d2rs\settings.toml`.

## 4. Keys

The original game's default key configuration (`specs/ui/controls.md`
§3): left click walk / attack / interact, right click the right skill,
`Ctrl` held run, `R` run / walk toggle, `Shift` stand still, `Alt` show
items, `A` / `C` character, `I` / `B` inventory, `T` skill tree, `S`
skill choice, `Q` quests, `M` message log, `H` help, `O` hireling,
`W` swap weapons, `Tab` automap (`F9`–`F12` its options), `V` minimap,
`F1`–`F8` skill hotkeys, `1`–`4` belt, `` ` `` show belt, `Space` clear
screen, `Esc` game menu (Options, Save and Exit). Rebind in the menu's Configure Controls,
or edit `%APPDATA%\d2rs\controls.toml`.

Which of these do something today, and what is missing, is in
[`docs/handoff/playability.md`](handoff/playability.md) (known gaps).
Nothing is verified against 1.14d yet unless that file says so: a
difference from the original is a bug worth reporting.

## 5. Reporting a bug

Send:

1. **The crash log** if it crashed: `d2rs-crash.log` next to
   `d2-client.exe` (the panic or error, a backtrace, the command line and
   the last steps: game folder, menu, game start with seed and save).
   It is overwritten by the next crash: copy it first.
2. **A screenshot** of the window (and of the console if it shows an
   error).
3. **The save**: `Documents\d2rs\saves\<name>.d2s` (or the file given to
   `--save`).
4. The artifact name (`d2rs-windows-<commit>`), what you did, and what
   you expected (what 1.14d does).

## 6. What the build is

`cargo build -p d2-client --release` on `windows-latest` with the pinned
toolchain (`rust-toolchain.toml`), release with line tables. Before
upload, a smoke step checks that the exe without game files stops with
the "no game files" error and writes the crash log, and that
`d2-client.exe crash-test` (a deliberate panic) writes it with a
backtrace. It does not run the game: there are no game files in CI.
