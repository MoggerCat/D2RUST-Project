# Spec: Tools — One-command checks against 1.14d (`scenario-diff`)

- **Status:** draft: the check-file format and the run; the `state`
  channel runs on both sides, `draws` reuses the rendering-facts tools,
  `packets` runs on both sides (`tools/packets-trace.md`), `rng` runs on
  both sides (`tools/rng-trace.md`), `items` compares the items both
  sides' packet recordings show created (§3 rule 13), `save` compares the
  written `.d2s` (§3 rule 14); one shared input script drives
  both sides at the same server frames (§2 rule 4, §3 rule 8).
- **Target version:** 1.14d (the original side); the format is d2rs-own.
- **Crate/module:** `tools/scenario-diff/scenario_diff.py`.
- **Related specs:** `tools/state-snapshot.md` (state channel),
  `tools/facts-render.md` (draws channel), `tools/scenario.md`
  (message-injection scenarios, a different run model), `tools/original-hooks.md`
  §5 (starting 1.14d unattended).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 36–44 |
| Inputs | 45–51 |
| Outputs / state changes | 52–63 |
| Rules | 64–65 |
|   1. Files | 66–71 |
|   2. Syntax | 72–144 |
|   3. Run | 145–639 |
|   4. Suite | 640–767 |
| Constants & data dependencies | 768–771 |
| Randomness | 772–775 |
| Edge cases & original bugs | 776–811 |
| Test vectors | 812–834 |
| Provenance | 835–838 |
| Open questions | 839–901 |
<!-- /index -->

## Summary

A **check file** names a character save, a game seed, a tick count and
optional scripted input. `scenario_diff.py <check>` builds the save
with `d2s-tool`, runs 1.14d (under Wine in the cloud, natively on
Windows) with one recorder per channel, runs d2rs headless with the same
save and seed, and prints the first difference of each channel, game
state first. It is the default way to compare a behaviour with 1.14d.

## Inputs

| Name | Type | Source |
|---|---|---|
| check file | text, §2 | `traces/checks/<name>.check` (committed) |
| the install | 1.14d `Game.exe` and data | `D2_GAME_DIR` (private data repo, `tools/cloud-game/fetch.sh`) |

## Outputs / state changes

- Work dir `traces/raw/check-<name>/` (gitignored): the save, each
  side's files (`orig.state.jsonl`, `d2rs.state.jsonl`, frames, draws),
  the Wine run folders.
- The 1.14d save folder gets `<Char>.d2s` (the run uses `-nosave`, so the
  game never writes it back; the `save` channel runs without it and
  restores the file, §3 rule 14).
- stdout: per channel the comparator's report; a summary line per
  channel. Exit code: the worst channel's (0 match, 1 diverged,
  2 partial, 3 error).

## Rules

### 1. Files

1. `traces/checks/<name>.check`, UTF-8, `<name>` matches `[a-z0-9-]+`
   and equals the `name` line. Inputs only (numbers, names, options);
   never game data.

### 2. Syntax

1. Line based; `#` starts a comment (not on `input` lines, whose script
   is taken verbatim). Blank lines skipped.
2. First line `check 1` (format version; others rejected).
3. Keywords (each once unless noted; unknown keyword, repeat, missing
   required line or bad value: an error naming the line):

<!-- rows -->
| Line | Required | Meaning |
|---|---|---|
| `name <name>` | yes | §1 |
| `save <Char> [options]` | yes | `d2s-tool new --name <Char> [options]`; the same file is the 1.14d save and d2rs' `--save` |
| `seed <1..2^31−1>` | yes | 1.14d `-seed N` (`original-hooks.md` §2 rule 3); d2rs `--seed N` |
| `ticks <n>` | yes | snapshots / ticks recorded on both sides |
| `seconds <n>` | no (300) | 1.14d wall-clock limit per recorder run |
| `difficulty normal\|nightmare\|hell` | no (normal) | d2rs `--difficulty` |
| `channels <ch>...` | no (`state`) | from `state`, `draws`, `rng`, `packets`, `items`, `save`, `frontend` |
| `draws-at <tick>` | with `draws` | the server tick whose frame is compared (≤ `ticks`; 1.14d's last drawn tick at or before it, §3 rule 7.2) |
| `input <script>` | no | the shared input script of rule 4, given to both sides (excludes the two lines below) |
| `input orig <script>` | no | `autostart.py` input script (seconds, client pixels) |
| `input d2rs <script>` | no | `d2-client play --input` script (server ticks; `state-dump` takes it only in rule 4's form) |
| `ignore <field>...` | no, repeatable | state fields not compared (`state_diff.py --ignore`) |
| `variant <name>` | no | both sides run on the test variant install `traces/variants/<name>/<name>.d2stack` (`tools/test-variants.md`), §3 rule 11 |
| `at <frame> poke <directive> <args...>` | no, repeatable | state injection (`tools/poke.md` §1, §2 rule 6; `spawn` lines as §1 rule 3): `<frame>` ≥ 1 is the absolute game frame (the `f` of `tools/state-snapshot.md`); applied after frame `<frame>` − 1 and its snapshot, before frame `<frame>`'s drain; passed in file order to every 1.14d recorder and to d2rs `state-dump` / `play` as `--poke "<frame> <directive> <args...>"` |
| `at <frame> send <Name> <field>=<value>...` / `at <frame> send hex <byte>...` | no, repeatable | a scripted C→S message (§3 rule 12): a typed message of `sim/client-messages.tsv` or raw bytes, written, encoded and resolved exactly as a scenario step's `msg` / `hex` (`tools/scenario.md` §3 rules 1–5, references included); `<frame>` as for `poke`; injected in the drain before tick `<frame>`, after that point's pokes and input; several sends of one frame in file order |

4. The shared input script (`input <script>`): steps separated by `;`,
   run in order; the first is `frame F`. Steps:
   1. `frame F` (F ≥ 1, never below an earlier `frame`): the steps after
      it, up to the next `frame` or `hold`, apply after server frame
      F − 1 ran (game +0xA8 / d2rs `Game::frame` = F − 1) and after its
      snapshot, before frame F's drain: the poke point (§2 row `at`).
      A stop seen after F − 1 (the script was still waiting) applies
      them there; each side notes it ("late").
   2. `move X Y`, `click X Y`, `rclick X Y`: the cursor to client pixel
      (X, Y) of the 800 × 600 window (0 ≤ X < 800, 0 ≤ Y < 600); a click
      is the press and the release in that one client pass.
   3. `hold X Y N` (N ≥ 1): left press at (X, Y), released before frame
      F + N's drain (F: the frame the hold applies before); the next
      step waits for the release and applies with it.
   4. `key K` (a letter or digit, ESC, TAB, ENTER, SPACE, SHIFT, CTRL,
      ALT, F1–F12 or a number: the Windows virtual-key code): key down
      and up in one pass. `key ESC` with no panel open opens the
      single-player menu, which pauses 1.14d (no tick after it). On d2rs `state-dump` only the keys of §3 rule
      8.2 run; any other is refused before the run.
   5. `clickunit T C[,C..]|* [DX DY]` / `rclickunit ...` (T the unit
      type 0–5; C one or more classes, decimal or `0x` hex, up to 8, or
      `*`: any class, but no unit in mode 0 or 12; DX DY default 0 −8):
      a left / right click, as `click` / `rclick`, at the screen point
      of the unit of type T and a listed class whose drawn point is
      nearest the frame centre (400, 300) (squared distance), plus
      (DX, DY), with one hover frame: the cursor goes to the point with
      the other steps of `frame F` (before frame F's drain), the press
      and release before frame F + 1's drain; the steps after it wait
      for the click (as after `hold`). 1.14d picks the hovered unit while
      it draws (`0x00467A10`), so a press posted in the same pass as its
      move is a point click (measured 2026-10-09 under Wine:
      `frame 40; click 368 300` on a fallen walks to the point, `frame
      39; move 368 300; frame 40; click 368 300` attacks it); a plain
      `click` on a unit needs the same `move` a frame earlier. The drawn
      point is `render/camera.md` §4 with no extra
      offset: X = px − cx_u + shiftX, Y = py − cy_u + 8 from the unit's
      client pixels (§2: static units 2, 4, 5 by the static rule). No
      such unit, or a point outside the frame: the step clicks nothing
      and each side notes it. 1.14d: `autostart.py`'s `nearest` /
      `screen_of` over the client unit table (the 16.16 position; ties:
      the first in the table walk); d2rs: `world_view::input_script::
      unit_point` over the model (the cell centre; ties: the lower unit
      key; d2rs-own, unverified where the two differ).
   6. No other step (`wait`, `text`, `shot`, `goto`, … are
      side-specific: `input orig` / `input d2rs`).

### 3. Run

1. Build the save once (`cargo run --release -p d2s-tool -- new ...`),
   copy it to the 1.14d save folder: `D2_SAVE_DIR`, else on Windows
   `%USERPROFILE%\Saved Games\Diablo II`, else
   `$WINEPREFIX/drive_c/users/$USER/Saved Games/Diablo II`
   (`original-hooks.md` §5.3 rule 2a).
2. Per channel, one 1.14d run: the channel's recorder with `--auto
   <Char> --seed N --ticks <ticks> --seconds <seconds>` and `input
   orig`; under Linux through `tools/cloud-game/run.sh --python`.
3. **state**: `record_state.py --snap-every 1` and `d2-client
   state-dump --save --seed --difficulty --ticks` (`tools/state-snapshot.md`),
   compared by `state_diff.py`. **draws**: `record_frames.py --every 1
   --draws-every 1`, `facts_render.py` on the frame tied to `draws-at`,
   d2rs `play --dump-draws --at-tick`, `d2-client facts-compare
   --ignore tick` (`tools/facts-render.md`); details in rule 7.
4. A channel with no d2rs recorder is reported as not compared
   (partial), never as a match (every channel has one now).
5. Pokes (§2 `at` lines) go to both sides of every channel as
   `--poke "<frame> <directive> <args...>"`. Each side writes one
   `poke` record per result (`{"k":"poke","f","frame","i","d","r",
   "guid"?,"note"?,"src"}`, `tools/poke.md` §3 rule 3 with `f` for `t`)
   into its state file, between the snapshots of frames f − 1 and f;
   `state_diff.py` does not compare them (the snapshots after them
   show the effect).
6. `--orig-only` / `--d2rs-only` run one side; `--reuse` keeps outputs
   already in the work dir; `--dry-run` prints the commands.
7. **draws**, step by step (work dir files in brackets):
   1. 1.14d: `record_frames.py --every 1 --draws-every 1 --no-save`
      with the rule 2 start [`orig.frames.jsonl`].
   2. The compared tick: the capture record (`"k":"frame"`, `seq`,
      `f` = last server tick, `draws`) whose `f` equals `draws-at`, else
      the last one before it (a note names it); none: an error. Which
      ticks 1.14d draws depends on its frame pacing, which the debugger
      slows (measured 2026-10-09 under Wine, same check: one run drew
      ticks 3, 5, …, 79 in 6 min while a cargo build shared the CPU,
      the next ticks 3–18 then 20, 22, …, 74 in 30 s). The same tick
      goes to d2rs (step 3), so both sides always compare one tick.
      `facts_render.py <capture> --scene s --frame <seq> --out
      draws-orig` [`draws-orig/scenes/s/`, `draws-orig/sprites.tsv`].
   3. d2rs: `cargo build --release -p d2-client` once, then the binary
      `play --save --seed --difficulty [--poke …] --dump-draws draws-d2rs
      --at-tick <the compared tick> [--input <input d2rs>] --frame-schedule
      frame-schedule.tsv` (step 5; a stale
      `draws-d2rs` is removed first; `--reuse` keeps one only if its
      `frame.tsv` tick is the compared tick; time limit 900 s).
      Windows: as is. Linux: an X display (the current `DISPLAY` if it answers and
      `D2_DRAWS_DISPLAY` is unset, else an own `Xvfb` on
      `D2_DRAWS_DISPLAY`, default `:98`, 1024×768×24, stopped after the
      run) and `WGPU_BACKEND=vulkan`; with no GPU the adapter is Mesa
      lavapipe ("llvmpipe"). Packages (Ubuntu 24.04, checked before the
      run, a missing one is an error naming it): `xvfb`, `x11-utils`,
      `mesa-vulkan-drivers`, `libvulkan1`, `libxkbcommon-x11-0`
      (`tools/cloud-setup.sh`). Success is a written
      `draws-d2rs/draws.tsv`; a non-zero exit after it is a warning, no
      `draws.tsv` an error.
   4. `d2-client facts-compare draws-orig/scenes/s draws-d2rs --ignore
      tick`: its verdict and first difference are the channel's report
      (exit 0 match, 1 diverged, 2 partial).
   5. **The recorded frame schedule and host clock** (decided
      2026-10-10, coordinator; settles REC-510). The host clock
      (`GetTickCount()`) and which client updates draw a frame are inputs
      of the recording, not game behaviour: the idle cursor steps the
      client seed once per drawn frame for 5,000 ms of host time
      (`ui/panels-3.md` §23 r8, `client/model.md` Randomness r4), and the
      weather update runs only in drawn frames (`render/draw-order-2.md`
      §11.2), so both feed the seed every later rain particle, sound
      variant and cursor frame draws on. The debugger slows 1.14d (one
      frame per update, 125 ms apart, instead of 40 ms), so a d2rs clock of
      its own can never meet a recording. d2rs therefore replays them:
      `[frame-schedule.tsv]` (format `frame-schedule 1`, written from the
      capture by `scenario_diff.py`: `# cursor_last`, `# cursor_idle` = the
      first frame's `cursor.last_step`, `cursor.idle_since`; one row `tick
      now` per captured frame, `tick` = `f`, `now` = the next frame's
      `cursor.last_step` (the capture reads the cursor at frame start,
      `render/capture.md` §3.3), `-` for the last frame). With it, `play`
      draws exactly the listed ticks, one frame each, and no frame, no
      weather update and no cursor step on any other tick; the cursor
      reads `now` of the drawn tick as its clock and starts from the
      recorded timers. A capture without a server tick or the cursor
      clock on any frame, or a schedule in which a frame other than the
      last lacks `now`, fails the check; d2rs never falls back to a clock
      of its own in a check run. Live play keeps the host clock. The input
      script follows the same schedule: 1.14d posts the steps of `frame F`
      at the stop of tick F − 1 (rule 8.1) and its message pump takes
      them at the start of the next client frame, after the frame that
      ran tick F − 1 has drawn; a d2rs pass runs its UI events before its
      draw, so on a drawn tick `play` times the script by the tick before
      (`present::script_tick`; measured 2026-10-10: `ui-draws-minipanel-ama`
      recorded on PC 1 draws even ticks and shows the click's mouse move
      at tick 40, recorded in the cloud it draws odd ticks and shows it
      at 41, not at 39). Open: the C→S of a click handled on an undrawn
      pass leaves with the next drawn pass in d2rs (`world_view_frame`
      handles events only on drawn ticks), one pass or more after
      1.14d's.
      Measured (`draws-town-arrival-ama`, 2026-10-10): with the schedule
      the client seed matches the capture's `seed_start` of every frame
      through tick 25 and the rain lines (endpoints and colors) are equal
      on every tick 4–24; tick 25 on differs by one extra footstep
      variant roll in d2rs (sound 2768 at T 22, 1.14d T 27: the audio
      owner's cause).
   6. **The recording's sound switch and pointer** (2026-10-10,
      rc-draw-row173). The recorders start Game.exe with `-w -ns`
      (`render/capture.md` §2): with `-ns` no sound device exists, so the
      sound init `0x00482260` never runs (it needs `[0x00881768]`), the
      request `0x004B9A00` returns at its first test (`[0x007C545C]` = 0,
      set only by that init's `0x004B9D00`), and no sound draw ever
      steps the client seed (measured: `draws-town-arrival-ama`, 76
      frames, every frame's seed steps are 5 per rain spawn plus the
      cursor's). `play --no-sound` is that switch (no sound driver);
      `scenario_diff.py` passes it when the capture header's `args` hold
      `-ns` (no header: the check fails). The pointer of a check run is
      the recording's: the input script's, or none; the host window's
      (an Xvfb display's centre) is ignored. With steps 5 and 6 the client
      seed of `draws-town-arrival-ama` equals the capture's at every frame
      through tick 73 and the rain rows are equal.
   7. **The recording host's registry** (decided 2026-10-10,
      coordinator). 1.14d reads `Diablo II` registry values at
      start (`0x00414F10`: HKCU, then HKLM; a REG_DWORD as is, a REG_SZ
      through `strtoul(s, NULL, 0)`, `ui/frontend-options.md` §O7 r1):
      `Mini Panel` decides whether the mini panel opens at entry
      (`ui/control-panel.md` §9 r9), `Help Menu` the help caption (§11
      r2), `PopupHireling` the hire pop-up (`ui/messages.md` §9 r2), the
      Options values their rows (§O6 r4). They differ between recording
      hosts (a clean game exit writes `Mini Panel`; a killed recording
      writes nothing), so they are inputs of the recording, like the
      clock. `record_frames.py` 0.3.2 records the whole key, HKCU and
      HKLM (32-bit view), before the game starts, as the header's
      `registry` (`render/capture.md` §5); `scenario_diff.py` writes it
      as `[registry.tsv]` (format `registry 1`: `# registry 1`, the header
      `scope name kind value`, one row per value: `hkcu` / `hklm`, the
      name, `dword` (decimal) / `sz` (text, `\\ \t \n \r` escaped) /
      `type<N>` (hex bytes)) and passes `play --registry registry.tsv`. A
      capture without `registry` fails the check (no default; a cache
      entry of an older recorder misses by the recorder key and is
      recorded again). `play` applies the Options rows to the settings
      and the UI values before the first frame; a value of another type
      that d2rs reads, or an Options value out of its row's range, is an
      error. Live play has no registry (`settings.toml` only).
9. **packets** (`tools/packets-trace.md`; work dir files in brackets):
   1.14d `record_packets.py` with the rule 2 start
   [`orig.packets.jsonl`]; d2rs `d2-client state-dump --save --seed
   --difficulty --ticks <ticks> --out d2rs.packets-state.jsonl --packets
   d2rs.packets.jsonl` (with `--input` as the state channel takes it);
   `packets_diff.py orig.packets.jsonl d2rs.packets.jsonl --next N`, whose
   exit code is the channel's. Pokes and sends reach both sides
   (`record_packets.py` runs `poke.py`'s and `send.py`'s layers,
   2026-10-09; it had no poke layer before). First run (2026-10-09, `packets-town-arrival-ama.check`,
   40 ticks, under Wine): frame 1's S→C messages and buffer match
   (0x01, 0x00, 0x02; the transport rows C→S 0x6D, S→C 0x8F and the
   direct 0xAF are excluded); the first divergence is C→S 0x67 (frame 1,
   bytes 2–16 and 28–34: the 1.14d client leaves non-zero bytes after the
   NUL of the empty game name and of the character name, the bridge
   sends zeros; no spec states them unwritten yet, so they are not
   masked); the first S→C divergence is frame 2 (the 0x6B join), message
   #1: S→C 0xAA is 12 bytes in 1.14d (`aa 00 01000000 0c 69 59 f9 ff
   1f`) and 8 in d2rs (`aa 00 01000000 08 ff`), with 109 against 102
   messages in that frame.

8. **input** (§2 rule 4): the shared script goes as `--input
   "<script>"` to every 1.14d recorder and to d2rs `state-dump` and
   `play`; with `input orig` / `input d2rs` each side gets its own line
   (`state-dump` only when `input d2rs` is in rule 4's form; else the
   state channel runs d2rs without it and reports at best partial).
   1. 1.14d (`autostart.py`, `AutoStart.attach`): the recorder's
      tick-return stop `0x0052FD1E` (ESI = game; the stop `poke.py`
      uses, one INT3 shared) hands game +0xA8 to the script; at the
      stop of frame F − 1 the due steps are posted to the game window
      (`PostMessageW`: `WM_MOUSEMOVE`, `WM_L/RBUTTONDOWN` / `UP`,
      `WM_KEYDOWN` / `WM_CHAR` / `WM_KEYUP`) while the game is stopped,
      so the window takes them before frame F's drain. Recorders with
      the stop: `record_state.py`, `record_frames.py`, `poke.py`. Each
      posted step is a footer note `autostart: frame F: <step> posted
      at the stop of frame S`.
   2. d2rs `state-dump --input`: after frame F − 1's snapshot and its
      pokes, the due steps' pointer events go through the bridge's
      world-click dispatcher (`world_view::ui_bind::world_clicks`, the
      call the window path makes after the UI): no panel, no shake; the
      hover target is `play`'s preview pick (`bridge::hover::pick`, taken
      at the end of the previous pass from the cursor then, as 1.14d
      hovers while it draws: a press posted in the pass of its `move` is a
      point click; objects pick inside ±48 px of their feet, units ±24,
      PROVISIONAL REC-2116: 1.14d hit-tests the sprite; the Act I waypoint
      click path of the effect scenes is the case that needs it; the
      `ClickView::pick` the window sets: the unit — monster, NPC,
      object or ground item, never a player — whose feet are nearest
      the click inside a box standing on them; d2rs-own, unverified,
      the original hit-tests the drawn sprites `0x00467A10`; measured
      2026-10-10 (rc-render-effect): the effect scenes' first left click after
      the waypoint, spell on the left button, walks in 1.14d (a left click
      on the ground without Stand Still is a walk, controls.md §6 r7) because
      nothing is hovered at the previous cursor (207,176), where d2rs's feet
      box (±24 wide, 96 above) picks a Fallen above its head; the hover
      candidate loop `0x00467AC0` tests each unit of the player's rooms with
      `0x00466870`, whose monster / object test `0x00470860` is the unit's
      drawn frame rectangle widened by 16 px on each side, not its feet
      box), so a click
      on a monster sends the skill on the unit (`ui/controls.md` §6 r8: C→S 0x06
      for a left click), on
      an NPC or object the walk to the unit, then on arrival the
      interact sender (C→S 0x13, `world_view::interact`, run once per
      server frame after the walk prediction ends), on a ground item the
      same (its pick-up); the held repeat runs once per pass (one pass
      per server frame) while a `hold` lasts. The local player the click
      reads is `play`'s walk prediction (`bridge::predict`, d2rs-own,
      unverified: the client model's cell stays at a walk's start,
      REC-51). `key` steps run the bound world action of the original
      key configuration (key mode 1: no panel open) through the handlers
      the window runs for an action no panel took, in the window's order
      (`world_view::present`): the run lock R (command 35: the mods word
      of later clicks), the belt keys 1–4 (commands 23–26: C→S 0x26,
      `bridge::belt`), then the world clicks, then weapon swap W (44: C→S
      0x60) and speech NumPad 0–7 (27–33, 55: C→S 0x3F). Every other key
      is refused before the run, naming the key and its command: panels
      and UI states (I, C, A, B, T, S, Q, H, M, P, O, Enter, Esc, Tab,
      F9–F12, V, Z, N, Space, `), the held modifiers (Shift, Ctrl, Alt:
      a down-and-up has no effect) and the skill hotkeys F1–F8 (the use
      sender of `ui/controls.md` §3.1 r2 is not named, and the window
      does not wire it either: switch skills with a C→S `send` line).
      Panel clicks (vendor buy / sell, sockets, cube, stash) stay
      unsupported headless (no UI): the click on the NPC opens the
      interaction (C→S 0x13 → S→C 0x27 / 0x28), and the panel's own
      messages go as C→S `send` lines. Each applied step is a stderr
      line and a footer note `input: frame F: <step> after frame L`
      (`clickunit`: the unit and point clicked, or why none; an
      interaction sent: `input: interact T:GUID`).
   3. d2rs `play --input` (Bevy): `frame F` waits for the bridge's
      server tick F − 1; the events reach the UI on the next loop pass;
      `hold` releases N ticks later; `key` steps go through the window's
      key path (bound action, typed character); `clickunit` reads the
      model camera (the local player's model position, open mode 0, no
      shake; d2rs-own) and is skipped with a warning when it resolves to
      nothing.
   4. Measured (2026-10-09, `walk-town-ama.check`: ScnAma, seed 1234,
      `frame 10; click 600 300`): two 1.14d runs give byte-identical
      snapshots (60 frames); the player's mode turns 6 (town walk) at
      frame 10 on both sides; the path target is (4880, 4223) on 1.14d
      and (4880, 4222) on d2rs (open question 3), so `xf` / `yf` differ
      from frame 10, `x` from frame 14, `y` from frame 20, and the walk
      ends (mode 5) at frame 32 on 1.14d, 34 on d2rs. The step length
      per frame is the same (|(Δxf, Δyf)| 24 571 vs 24 572).
   5. Measured (2026-10-09; 1.14d under Wine, state channel):
      `combat-melee-fallen.check` (CmbBar, seed 1234, Blood Moor,
      `frame 39; clickunit 1 19`): both sides click (368, 300), the same
      fallen (GUID 21 on 1.14d, 19 on d2rs: the party's GUIDs are offset
      by 2), the press before frame 40; d2rs sends `06 01000000
      13000000` (left skill on unit 1:19) after frame 39; on both sides
      the player is in mode 7 (attack 1) from frame 40 to 54 and the
      fallen's hp 256 → 0 at frame 46: the player and the clicked fallen
      are equal on every field compared, frames 30–150 (the rest of the
      party's AI and the level's own monsters differ). The 1.14d packets
      channel took no input: `record_packets.py` has no tick-return
      `AutoStart.attach`, so `frame` steps never run there. An NPC
      in the Rogue Encampment (d2rs only, `frame 10; clickunit 1 150`, the
      click before frame 11): C→S 0x59, then `02 01000000 03000000`
      (walk to unit 1:3) after frame 10, `13 01000000 03000000`
      (interact) after frame 59 when the walk ended, the server answers
      S→C 0x27 / 0x28 and the client 0x2F.
      `combat-potion-midfight.check` (`frame 70; key 1`): 1.14d drinks
      the slot-0 potion at frame 70 (hp 2560 → 2640, +80 a frame; the
      item leaves the state; belt items in mode 2). d2rs runs
      `belt_slot_1` at frame 70 but no C→S 0x26 leaves: the d2rs load
      leaves the two belt potions in mode 4 (cursor; `d2-sim`
      `wiring/inventory/load.rs` `belt_place` sets no belt mode) and
      the join sends no item of the belt (one S→C 0x9D, the sword), so
      the client's column-ready byte stays 0 (`client/msg-stats-items.md`
      §2 r6) — a server-side gap, not the key path (unit test
      `headless_belt_key_sends_0x26`).
10. **rng** (`tools/rng-trace.md` §6; work dir files in brackets): 1.14d
    `record_rng.py --frames --skip-inline drlg` with the rule 2 start
    [`orig.rng.jsonl`] (the DRLG inline draws are never compared:
    `tools/rng-trace.md` §6 r4);
    d2rs `cargo run --release -p d2-client --features rng-trace --
    state-dump --save --seed --difficulty --ticks <ticks> --out
    d2rs.rng-state.jsonl --rng d2rs.rng.jsonl`; `rng_diff.py
    orig.rng.jsonl d2rs.rng.jsonl --next N`, whose exit code is the
    channel's. `at … poke` or `at … send` lines or a shared `input`: not
    compared (partial; `record_rng.py` has none of them); `input orig` /
    `input d2rs`: 1.14d only, partial at best. First run: `rng-town-arrival-ama.check`
    (`tools/rng-trace.md` Open questions 1).
12. **send** (§2 `at … send`): each line goes in file order, after the
    `--poke` options, as `--send "<frame> <canonical message>"` (the
    message re-written in `tools/scenario.md` §2 rule 6's canonical form:
    typed fields in layout order, hex bytes lower case) to every 1.14d
    recorder that has the send layer (`record_state.py`,
    `record_packets.py`, `record_frames.py`) and to d2rs `state-dump`;
    `play` takes none (the draws channel is then partial at best) and
    `record_rng.py` none (the rng channel is not compared). A line that
    does not parse is an error naming it (§2 rule 3).
    1. 1.14d (`tools/trace-recorder/send.py`): `tools/original-hooks.md`
       §1 rule 4: at the first stop at `0x0044F136` (bytes `E8 A5 DE 0D
       00`) after the tick-return stop `0x0052FD1E` of frame f − 1 (ESI
       = game, game +0xA8 = f − 1), for each message of frame f: the
       references resolved on the live unit lists (the hash lists of
       original-hooks §4 rule 1, the player's path position), the bytes
       written at scratch S+0x10, `0x0052AE50(size, 1, S+0x10)` called
       on the stopped thread with its return trapped at S+0, EAX read
       (1 = queued); then the saved context restored and the drain runs.
       Nothing is injected before §1 rule 5 holds (client 0 in state 4:
       the send is written as unresolved `@player`). `@wp` is a gap (no
       spec gives the 1.14d objects table's operate function).
    2. d2rs (`state-dump --send`, `app::send`): after frame f − 1's
       snapshot, its pokes and its input, through the bridge
       (`Bridge::inject`, `bridge::inject`): references resolved on the
       server game on the server thread (the unit lists the snapshot
       reads; `@wp` from the game's waypoint table), the bytes handed to
       the host's transport send (`Host::send_system`: the classifier and
       the queues, never the duplicate filter, `tools/scenario.md` §4
       rule 2 (a)). Before the local client is in state 4 nothing is
       sent (unresolved `@player`), as on 1.14d. `play --send` (the
       draws channel) runs the same injection from the server thread's
       before-pump hook, after that hook's pokes (`app::send::install`),
       so the drawn frame has the messages too; it prints the records on
       stderr.
    3. Each side writes one record per message into its output, between
       the snapshots of frames f − 1 and f: `{"k":"send","f","frame","i",
       "r","bytes"?,"eax"? (1.14d),"note"?,"src"}`, `r` = `ok` (queued),
       `dropped` (the classifier refused it), `unresolved` (nothing sent,
       `note` the reference), `gap`, `failed`; d2rs also prints it on
       stderr and in the footer notes. The comparators skip the kind;
       the messages themselves show in the packets channel (`c2s`,
       `dispatch`, `result` on 1.14d) and their effect in the snapshots.
    4. First run (2026-10-09, `items-vendor-akara-buy.check`: ScnBuy =
       ScnAma with 5000 gold, seed 1234, the player poked next to Akara,
       then 0x13, 0x2F, 0x38 action 1 and 0x32 for `@4` at frames 16, 18,
       20, 24; state and packets, 40 ticks, under Wine): on 1.14d every
       call returned EAX 1 and each message has its `client_out`, `c2s`
       and `dispatch` records in the drain before its frame; on d2rs each
       is queued and dispatched in the same frame. Both open the same
       41-item store (frame 20, GUIDs 1–41, same classes). First
       differences: frame 16, Akara's path target (`tx`, `ty`) is 0 on
       1.14d after the 0x13 (`world/npc.md` §2 rule 2 clears her path)
       and her position on d2rs; frame 16, S→C 0x27's text entries in
       another order; frame 17, the 1.14d client itself answers the 0x27
       with C→S 0x31 (the bridge did not; since q-fix-npc-interact the
       headless state-dump answers 0x28's dialog branch as the UI does,
       `client/msg-ui.md` §16 r4.3); frame 20, store items have
       no `x` / `y` / `d` in d2rs' snapshot; frame 24 (the 0x32): 1.14d
       answers 0x2A kind 4, code 0, GUID 42, gold 3744 (the copy, which
       appears in the inventory; store item 1 is removed), d2rs 0x2A
       kind 0, code 10 (`world/npc.md` §9: no room for the bought item),
       GUID −1, gold 5000, and nothing moves.
11. **variant** (§2 `variant <name>`): before the save is built, the
    variant install `<base>/../variants/<name>` (base: `D2_GAME_DIR`, the
    default of `tools/test-variants.md`) is built with `cargo run
    --release -p data-tool -- variant build
    traces/variants/<name>/<name>.d2stack --game <base> --out <dir>`
    unless it already holds `Game.exe`; then it is the game dir of
    every step (d2s-tool's tables, the 1.14d recorders' `--game`, d2rs'
    `D2_GAME_DIR`). Used to take a system out of a check (a level with
    no monster population, `traces/variants/blood-moor-empty`).
13. **items** (`tools/scenario-diff/items_channel.py`, comparator
    `items_diff.py`; work dir files in brackets): the two recordings of
    the packets channel (rule 9, same commands, same pokes, sends and
    input) [`orig.packets.jsonl`, `d2rs.packets.jsonl`], recorded once
    when a check asks for both channels; then `items_diff.py
    orig.packets.jsonl d2rs.packets.jsonl --list --next N`, whose exit
    code is the channel's. The hook is the S→C queue the packets
    recorders already pin (`tools/packets-trace.md` §2: 1.14d
    `0x0053B280`, d2rs `ClientBuffers::queue`; direct sends too), not
    the item-creation function: every item the game creates for a
    client's view (drop, store fill, gamble list, cube output, quest
    item, the save's items at join) reaches the client as S→C 0x9C /
    0x9D (`items/inventory-moves.md` §11, `items/item-actions.tsv`).
    1. **Created item.** In each recording, the `s2c` stream of the
       packets windows (`tools/packets-trace.md` §3 rule 1: window,
       then record order) is read for messages 0x9C / 0x9D of at least
       8 bytes; the first one carrying a given item GUID (bytes 4–7;
       −1 skipped) creates the item; later messages of that GUID
       (pick-up, moves, stat updates) are not this channel's. The item
       is (index in creation order, window = creation frame, id,
       action = the creating event, category byte 3, for 0x9D the owner
       type byte 8 and owner GUID 9–12, the bit stream from byte 8 of
       0x9C / 13 of 0x9D, `items/bitstream.md`).
    2. **Range.** Items whose creation frame is ≤ the smaller last
       complete window of the two sides; different last windows (or a
       side without a complete tick) make the result partial; no item
       on either side is partial too (nothing compared).
    3. **Comparison**, item k of 1.14d with item k of d2rs, first
       difference reported: creation frame, id, action, category, owner
       type, then an owner of type 4 by the owner's creation index (the
       socket fillers of 0x9D action 0x13), then the bit stream byte for
       byte (lengths last). An item on one side only is `missing in
       d2rs` / `missing in 1.14d`. The GUID and a non-item owner's GUID
       are not compared here (the packets channel compares those bytes;
       the two sides' GUIDs are offset when the party differs, §3 rule
       8.5); the size byte 2 follows from the stream length.
    4. **Report.** Both item lists (`--list`: per index, side, frame,
       id, action, the item code read from the stream head, GUID, stream
       length), the first divergence and the next N, each with both
       items, the stream bytes ±4 around the first differing byte and
       the head field holding the first differing bit (flags, version,
       mode, x / y or body / x / y / page, code: `items/bitstream.md`
       §2–§4.1; past the code, its bit offset). `--json`: diff-summary-1
       with `frames_compared` = items compared (the larger count),
       `frames_equal` = items without a difference, plus `items_orig`,
       `items_d2rs`.
    5. First runs (2026-10-09, 1.14d under Wine, recorders of rule 9):
       `items-vendor-akara-stock` (Akara's store, frame 20): 41 items on
       both sides, same order, codes, frame and action 0x0B; 39 streams
       identical, items #7 and #12 (wands) differ only in the charged
       skill (stat 204, param 4289: current charges 67 = max on 1.14d,
       65 and 64 on d2rs). `items-drop-gold-potion` (6 poked ground
       items, frame 4): same order, frame, action 0x00, positions and gold
       amounts; every stream lacks flag 0x10 (identified) on d2rs.
       `items-drop-monster-kill`: 1.14d drops gold at frame 36, d2rs a
       stamina potion (`vps`) at frame 37.

14. **save** (`channels save`; `tools/scenario-diff/save_channel.py`;
    work dir files in brackets): the `.d2s` each side writes when the
    check ends, compared byte for byte (`formats/d2s.md`).
    1. The end is Save and Exit on both sides at the same frame: the
       one-byte C→S 0x69 (`flows/save-exit.md` §1 r2, §2) injected as a
       last `at <ticks + 1> send hex 69`, so both servers run the leave
       (and with it the character writer) in the drain before the tick
       of frame `ticks + 1`. Both sides therefore run `ticks + 1` ticks;
       the snapshots of the run [`orig.save-run.jsonl`,
       `d2rs.save-run.jsonl`] are not compared (that is the `state`
       channel's job). The check's `at … poke`, `at … send` and shared
       `input` lines apply as for every channel.
    2. 1.14d: `record_state.py --write-save --save-watch <file>`. The
       game runs without `-nosave` (§5.4 of `tools/original-hooks.md`),
       so its server writes `<Char>.d2s` into its save folder (the
       Wine prefix's `Saved Games/Diablo II`, or `D2_SAVE_DIR`); the
       recorder ends 2 s after that file changed (a Save and Exit ends
       the game's ticks, so no tick limit can follow) and the file is
       copied to [`orig.saved.d2s`]. A file equal to the start save is an
       error (Save and Exit did not run). Whatever the run does, the
       start save is copied back into the folder afterwards, so the
       next channel starts from the same character.
    3. d2rs: `d2-client state-dump … --save-out <file> --send
       "<ticks + 1> hex 69"` [`d2rs.saved.d2s`]: the server's character
       writer of `play` (`app::save::FileStore`: the loaded save with
       the live values laid over it) is installed with that path, the
       leave of 0x69 runs it, and the dump ends when the client left the
       game. The start save is never written.
    4. Compare (`save_channel.py`): each file must have the magic, a
       size field equal to its length and a checksum equal to the one
       recomputed over the file with +0x0C = 0 (`formats/d2s.md` §3); a
       1.14d file that fails is an error (code 3), a d2rs file that
       fails is a difference. The save time at +0x30 (`time(NULL)` at the
       write, a wall-clock host input; `formats/d2s.md` §2.1) is zeroed
       in both and each checksum recomputed; no other byte is masked.
       The first byte that differs (the checksum itself is not reported:
       it follows from the others) is printed with its region: a header
       field, the quest, waypoint or NPC section, or the variable
       section of the nearest marker (`gf`, `if`, `JM`, `jf`, `kf`; a
       label found by the first occurrence of each marker in order, not
       a parse), both sides' four bytes there, the number of bytes that
       differ and the sizes. Exit 0 equal, 1 different, 3 error;
       `--json` writes `diff-summary-1` (`rows_compared` = bytes of the
       longer file, `rows_equal` = bytes equal) [`save.summary.json`].
       `py tools/scenario-diff/save_channel.py ORIG.d2s D2RS.d2s`
       compares two files; `--selftest` needs no game.
    5. What the compare cannot see: the file is the character's state at
       the leave, so anything the two servers hold differently at that
       frame (life regenerating a tick earlier or later, an item a tick
       earlier on the cursor) is a real difference of the game state and
       shows here as in the `state` channel; the layer-by-layer cause is
       found with the `state` channel of the same check, which has the
       same pokes and input.

15. **replaying recorded C→S** (`state-dump --no-own-c2s <id>[,<id>]`,
    ids decimal or `0x` hex, repeatable): the bridge's own C→S messages
    (the model's answers, `ui/controls.md` world clicks: `bridge::Bridge::
    send_outgoing`) with these ids are dropped instead of sent; `--send`
    and the `--input` clicks' dispatch are untouched except that a click's
    own-origin message with a listed id is dropped too. Each drop writes
    one footer note `own c2s dropped: frame F: <hex bytes>` (F: the frame
    after the previous snapshot's) and a stderr line. Use: replaying
    1.14d's recorded C→S with `--send` and no `--input` leaves the bridge
    without a walk prediction for the local player, so
    `bridge/check.rs` `correct()` would send its own 0x5F and the server
    would walk the player back (measured ScnAma seed 1234: 0x01 replayed
    at frame 10, d2rs 0x5F `5f 09 13 84 10` at frame 86, none on 1.14d);
    `--no-own-c2s 0x5F` removes it. Default: nothing dropped.

16. **frontend** (`channels frontend`; `tools/scenario-diff/frontend_channel.py`;
    `tools/frontend-sbs/frontend_sbs.py --script dolls`, `dolls_check.py`;
    REC-2295): front-end screens by X input on both sides (1.14d on
    `D2_DISPLAY`, d2rs on `D2_DRAWS_DISPLAY`), not ticks; the `save` line only
    names the character (the charselect lists every save of the folder, the
    three of `prepare_saves.sh`). 1.14d: 12 shots 0.37 s apart after
    charselect; d2rs: 70 back-to-back shots (all animation phases). Per slot
    figure rectangle: the doll mask is every pixel where a d2rs frame departs
    from the per-pixel median of the 70 (sum of channels > 30); each 1.14d
    shot takes the d2rs frame with the fewest differing mask pixels (phase is
    not tick-anchored, REC-2182); a pixel differs above 16 per channel (the
    screen-level brightness offset of REC-1550 is up to 8, dark noise 16);
    the slot is EQUAL when every shot differs in at most 1 % of its mask. The
    channel is MATCH when all three slots are EQUAL. Also reported: the dy
    probe (d2rs figure shifted -2..2 rows) and the feet-band brightness
    (shadow probe). Measured 2026-10-10: slots 0/1 differ 0 px, slot 2 at
    most 0.56 %; dy 0 is the only match (0 / 1 px against 430+ for +-1).

### 4. Suite

`tools/scenario-diff/suite.py`: every check (or `--filter GLOB` on the
name, `--area A[,B]`, the area being the name's first dash token)
through `scenario_diff.py`, in parallel, with a match % and the
playthrough's playability next to it.

1. Build once up front (`cargo build --release`, `CARGO_PROFILE_RELEASE_DEBUG=0`):
   `-p d2-client --features rng-trace` when a check runs `rng`, then
   `-p d2-client -p d2s-tool` (last, so `target/release/d2-client` stays
   the plain build). The binaries are hard-linked into
   `target/suite-bin/` (`d2-client`, `d2-client-rng`, `d2s-tool`) and
   given to `scenario_diff.py` as `D2RS_BIN_DIR`: with it every d2rs step
   runs the binary, never `cargo` (no worker waits on cargo's lock).
   `--no-build` uses the binaries already there.
2. Workers: `--workers N` (default min(cores − 1, 3)) threads, slowest
   checks first (draws, then rng, then ticks × channels). Worker k has its
   own Wine prefix `~/.wine-d2-suite-<k>` (made once: `cp -al` of
   `WINEPREFIX` or `~/.wine-d2`; the `*.reg` files and the `Saved Games`
   folder real copies, no `.run.lock`, so nothing writes through a hard
   link and no lock inode is shared), `D2_DISPLAY=:<90+k>` (1.14d) and
   `D2_DRAWS_DISPLAY=:<100+k>` (d2rs' draws window). The save step
   unlinks the prefix's `<Char>.d2s` before copying (rule 1).
3. Per check: work dir `traces/raw/suite/<name>/`, `scenario_diff.py
   <check> --work <dir> --json <dir>/result.json --next 5`, its output in
   `suite.log`. **Reuse of 1.14d**: the key (`suite.key`, format
   `suite-key-2`) is the sha256 of the check file, of the save that
   `d2s-tool` makes from the check's `save` line with `--time 1` (d2s-tool
   otherwise stamps the current time), of `Game.exe`, and of the recorders:
   the scripts (with the same-folder modules they import) of the check's
   channels plus `autostart.py`, hashed as in the orig-cache key (`items`
   uses packets'). A changed recorder therefore never reuses `orig.*` (a
   stale raw recording gave verdicts against outdated 1.14d output; found
   by rc-rerun-r10). Same key and
   no `--fresh`: `scenario_diff.py --reuse-orig` (keeps `orig.*`
   recordings, re-runs d2rs and the comparators); otherwise the 1.14d
   outputs and the key are removed and recorded again. The key is written
   after a run that left a 1.14d output.
3b. **Orig cache** (`orig_cache.py`; `scenario_diff.py` and `suite.py`
   `--orig-cache [DIR]`, default `traces/orig-cache`; `--fill-cache`): a
   shared store of the recorded 1.14d side, so a session compares with no
   Wine run. One entry per check and channel,
   `<DIR>/<check>/<channel>/cache.json` (format `orig-cache-1`: `key`, the
   `command` that made it, each file's size and sha256) plus the recorder's
   text output (state `orig.state.jsonl`, draws `orig.frames.jsonl`, rng
   `orig.rng.jsonl`, packets `orig.packets.jsonl`; the draws channel's
   `draws-orig` is derived from the frames file by `facts_render.py` and
   is rebuilt each run). Key (format `orig-cache-key-2`): sha256 of the check
   file's recording lines (its lines without comments, blank lines and the
   comparator-only `ignore` lines, an `input` line kept whole, each
   stripped and ended by a newline; `orig_cache.recording_text`), of the
   `--time 1` save (rule 3), of `Game.exe`, of the private
   repo's `install/manifest.json` (else of `Game.exe` alone, marked
   `game-exe:`), and of the channel's recorder (its script and, transitively,
   the same-folder modules it imports). A hit restores the file into the
   work dir and skips the recorder; any other key, a missing file or a file
   whose sha256 differs is a miss and records 1.14d as before. `--fill-cache`
   stores each fresh recording; `--fresh` never reads the cache (it refills
   it with `--fill-cache`). The cache holds small text only; a file with a
   PNG signature or a NUL byte, or over 64 MiB, is not stored, and
   rendered frames go to the private repo, never here (CLAUDE.md rule 1).
   `orig_cache.py --migrate-v1 DIR CHECKS_DIR...` rewrites each
   `orig-cache-key-1` entry whose check hash still equals the sha256 of
   its check file to `orig-cache-key-2` (the recorded files untouched);
   the others stay misses.
4. Comparator summaries: `state_diff.py`, `rng_diff.py` and
   `packets_diff.py` take `--json FILE` (format `diff-summary-1`:
   `channel`, `code`, `verdict`, `frames_compared`, `frames_equal`,
   `differences`, `first` {`frame`, `text`} or null) without changing
   their report or exit code; `scenario_diff.py` passes `--json
   <work>/<channel>.summary.json` and removes a stale one first. Ticks
   compared and equal: **state** the common frames, equal when no unit
   field, unit presence or game seed differs; **rng** every frame of the
   compared range (a frame without draws on both sides is equal), equal
   without a divergence; **packets** the frame windows, equal without a
   divergence in any stream (the first per stream and window is all the
   comparator looks at); **draws** one frame (equal on a facts-compare
   match), and the match % is the rows equal by position over the rows
   (columns `i`, `at`, `seq`, `tick` not compared, a `?` cell equal),
   computed by `scenario_diff.py` into `draws.summary.json`.
   `scenario_diff.py --json FILE` writes `scenario-diff-result-1`:
   `check`, `channels` {ch: {`code`, `verdict`, `summary`}}, `error`.
5. Report: per check and channel ticks, equal, match % and the first
   divergence (one line; an error's message); per area and overall:
   checks, channel runs, ticks compared, ticks equal, % (draws counts its
   one frame here, not rows) and verdict counts. A run that errored
   counts 0 ticks.
6. Playability: `playthrough.py traces/playthrough/act*.play --json`
   (`D2_GAME_DIR` defaulted as here) in a thread next to the checks; per
   act reached / total, furthest consecutive and the first blocker
   (milestone, kind, frame, evidence), from the act's own keys
   (`reached`, `total`, `consecutive`, `first_blocker`) when present,
   else counted from its milestones. A missing script or no `.play` file
   is reported as "not available". `--no-playthrough` / `--no-checks`
   skip either.
7. Output: the text report on stdout; `--json F` (format
   `suite-result-1`, every record and timing) and `--md F` (the two
   tables in Markdown, for handoff docs). Exit 0 whatever the checks
   found; 3 when the suite failed (build, prefix, no check matches).
8. 1.14d start cost (measured 2026-10-09, Wine 9.0, `record_state.py`
   40 ticks): the run was 9.1 s, of which 6.0 s autostart's menu delay
   (`--auto-after`, default 6), 0.5 s from leaving the menu to the
   player in the level, ~1.6 s the 40 ticks, the rest Wine and Python
   start and shutdown; the recorders stop the game at `--ticks`
   (`record_state` / `record_frames` / `record_packets` at the tick
   limit, `record_rng` at the entry of tick N + 1). Autostart leaves the
   menu only once the launcher mode is 4 (the menu is up), so the delay
   can be 0: `D2_AUTO_AFTER` sets autostart's default; with 0, six runs
   in a row reached the level (menu left at 0.2–0.3 s) and wrote 40
   snapshots byte-identical to the 6 s run; one run is then 3.4–3.9 s.
   suite.py sets `D2_AUTO_AFTER=0` (`--auto-after S` overrides).
   `record_rng.py` was the slow one (all 846 inline sites single-stepped:
   about 60 s before the level is reached, 70.5 s per run); with the
   DRLG sites unhooked and emulated steps (`tools/rng-trace.md` §4 r6,
   r7; measured 2026-10-09) the run is 11.1 s (`D2_AUTO_AFTER=0`; the
   level at 7.2 s), and `rng-town-arrival-ama.check` takes 11.9 s in
   all (17.7 s with the 6 s menu delay; d2rs already built).
9. `run.sh`'s prefix lock (fd 9) was held after a run by the screenshot
   subshell's `sleep` (up to `--seconds`), which serialized every run on
   a prefix behind the previous one's sleep; the subshell and Xvfb now
   close fd 9 and the sleep is killed with its subshell.
10. Measured 2026-10-09 (8 checks, 4 cores, a d2rs build already up to
    date): `--fresh --workers 1` 153.6 s; `--fresh --workers 3` 64.1 s
    (the rng check, 63 s, bounded it before the faster `record_rng.py`
    of rule 8, now about 12 s); a second run with the recordings
    reused: the checks 9.5 s (8 of 8 reused); the playthrough (act1 and
    act2, d2rs only) took about 6 min next to them.

## Constants & data dependencies

None beyond the tools named.

## Randomness

None in the tool. Both games run on `seed`.

## Edge cases & original bugs

1. With `input orig` / `input d2rs` (units differ between the two
   scripts) keeping the sides in step is the check author's job; the
   shared `input <script>` (§2 rule 4) times both by server frame.
2. Two 1.14d runs of the same check differ in the draws channel (measured
   2026-10-09, `draws-town-arrival-ama` vs the committed
   `a1-town-arrival-ama` scene, both tick 73): the floor and wall `light`
   column (the light hash differs from row 1 on) and the rain lines
   (`DrawLine` rows: positions and count; 259 vs 267 draws). Rows 0–197
   are otherwise equal. A difference in those columns against d2rs is
   not yet a d2rs finding.
   A second cause, on real Windows (measured 2026-10-10, PC 1 today,
   `ui-draws-*-ama`): 1.14d reads the **real pointer** for hover and the
   cursor draw, so a draws check whose input never moves the cursor
   leaves it wherever the desktop pointer was; one run then draws a
   unit's hover name (six glyph rows of "Warriv" at (475, 211) with the
   cursor at (520, 237)) and another does not (cursor at (720, 234)).
   A draws check therefore pins the cursor first (`frame 38; move 790
   10` in the checks of that day); with the pin three runs of
   `ui-draws-questlog-ama` and the skill-pick checks give the same UI
   rows but one cursor animation frame. A button of the control panel
   is clicked with `hold X Y 2`, not `click`: a press and release in one
   pass was lost in one run of three (`ui-draws-left-skill-pick-ama`).
3. Two 1.14d runs at the same time on Linux share the Wine prefix and
   the `:99` display unless told apart: the first `run.sh` to end runs
   `wineserver -k` and kills the other's game (seen 2026-10-09: the
   capture stopped at tick 5, exit 1, no message). Give a parallel run
   its own `WINEPREFIX` (a `cp -al` copy of `~/.wine-d2` costs no disk)
   and `D2_DISPLAY`; the save goes to that prefix (rule 1).
4. The items channel sees an item only when the server sends it to the
   client (§3 rule 13): an item created and freed in one frame, or
   created outside the client's view (no 0x9C), is not compared; a
   GUID the game frees and gives again to a later item counts once (the
   later item is read as a message of the first).

## Test vectors

| Input | Expected |
|---|---|
| `--selftest` | the parser accepts the sample and every committed check; each malformed line kind is rejected; a dry run issues the recorder, state-dump and comparator commands with the check's save and seed |
| `--selftest` (draws) | the dry run issues `record_frames.py --every 1 --draws-every 1 --no-save`, `facts_render.py --frame`, one `cargo build`, the binary's `play --dump-draws … --at-tick <draws-at>` and `facts-compare … --ignore tick`; a one-side run issues no compare; the frame pick returns the frame at the tick, else the last earlier one with a draw log; a scene's `frame.tsv` tick is read back |

| `--selftest` (variant) | `variant only-fallen` builds `<base>/../variants/only-fallen` with `data-tool variant build traces/variants/only-fallen/only-fallen.d2stack --game <base>` and the recorder gets `--game <that dir>/Game.exe`; a name outside `[a-z0-9-]` and a repeated `variant` are rejected |
| `--selftest` (input) | a shared `input` line reaches `record_state.py`, `state-dump`, `record_frames.py` and `play` as `--input '<script>'`; a script not starting with `frame`, with `wait` / `shot`, a frame 0 or going back, a `hold` without N or non-integer arguments, and a shared line next to `input orig` are rejected; an `input d2rs` in play's tick form goes to `play` only |
| `suite.py --selftest` | discovery by glob and area, slowest first; the cache key misses on a change of the check, the save, Game.exe or a recorder script/imported module; the 1.14d outputs and key removed, d2rs' kept; a prefix copy hard-links the bulk and copies `*.reg` and saves, no lock; the rng build before the plain one; match % per channel (draws by rows), area and overall; playthrough acts from the `--all` keys or from milestones; text and Markdown tables |
| comparators' `--selftest` (`--json`) | `state_diff`, `rng_diff`, `packets_diff`: the summary counts the frames with a difference and names the first; a match has every frame equal and no first |
| `items_diff.py --selftest` | items in creation order from synthetic recordings (a later message of a GUID is no creation; a 0x9D filler's owner by index); GUIDs offset by 2 on one side match; every stream byte perturbed is found at its item and byte, a changed code named `code`; a changed action, frame, filler owner and a missing item are reported; fewer ticks on one side and no item at all are partial; the summary counts items |
| `--selftest` (items) | with `packets` and `items` one 1.14d recording and one `state-dump --packets` serve both; `items` alone records them; the comparator gets both files and `--json <work>/items.summary.json` |
| `autostart.py --selftest` | `frame` / framed `hold` parsing and rejections; at simulated tick-return stops the click, hold press, key and move are posted at the stop of F − 1, the hold's release at the stop of F + N − 1; a late stop runs the step there |
| `--selftest` (save) | the dry run issues `record_state.py --ticks <ticks+1> --write-save --save-watch <save folder>/<Char>.d2s --send '<ticks+1> hex 69'` (after the check's own sends) and `state-dump --ticks <ticks+1> --save-out <work>/d2rs.saved.d2s --send '<ticks+1> hex 69'`, then the compare; a one-side run issues no compare |
| `save_channel.py --selftest` | on synthetic files: two saves that differ only in the save time are equal; a changed level byte is reported at 0x2B in `header level`, not at the checksum; a longer item section is reported with both sizes; a file whose checksum does not match its bytes is a difference; an empty or non-save d2rs file is a difference and the same 1.14d file an error; the `--json` summary counts bytes |
| d2rs unit test (`app::state_dump`) | `--save-out FILE` parses into `DumpArgs.save_out` |
| `record_state.py --selftest` | `--write-save` drops `-nosave` from the game arguments, `--save-watch FILE` is parsed |
| `--selftest` (send) | `at … send` lines parse to the canonical message (fields in layout order, hex lower case); a missing or unknown message, a missing field, a byte that is not two hex digits, frame 0, a chat row and a misspelt step are rejected; the dry run passes `--send '<frame> <message>'` after the pokes to `record_state.py`, `state-dump`, `record_frames.py` and `play`; the packets channel passes pokes and sends to both sides |
| `send.py --selftest` | `msg Walk x=10 y=20` → `01 0a 00 14 00`, `SelectSkill skill=36 left=0 item=0xFFFFFFFF` → `3c 24 00 00 00 ff ff ff ff` (`tools/scenario.md` test vectors), the recorded 0x32 of `world/vendors.md` §7.1 rule 10; references in GUID order and unresolved ones; the strict errors; the call layout ([ESP] = S, size, 1, S+0x10; ESP = saved − 16; EIP `0x0052AE50`); on a fake process: steps due at the first stop after tick f − 1, EAX 0 → `dropped`, the state-4 gate |
| d2rs unit tests (`app::send`, `conformance::scenario::script`) | `--send` parsing and encoding as scenario steps (same vectors), the canonical text, the `send` record lines, the strict errors |
| d2rs unit tests (`world_view::input_script`, `app::state_dump`) | the same parse and rejections; `Headless` events at frame F − 1, the hold release at F + N − 1, a late frame noted; `state-dump --input` refuses `wait`, `key` and a script not starting with `frame` |

## Provenance

d2rs-own tool; no 1.14d fact.

## Open questions

1. Resolved (2026-10-09, q-tool-poke with q-tool-state-diff): the
   `poke` directives run on both sides as a repeatable `--poke "<frame>
   <directive> <args>"` (`record_state.py`, `record_frames.py` through
   `poke.py`; d2rs `state-dump`, `play`), §2 and §3 rule 5.
2. Resolved (2026-10-09, q-tool-state-diff input channel): one shared
   input script for both sides, timed by server frame (§2 rule 4, §3
   rule 8); `state-dump --input` applies it through the bridge. Resolved
   for unit clicks and keys (2026-10-09, q-fix-b-headless-unit-click-keys):
   `clickunit` / `rclickunit` in the shared form (§2 rule 4.5, both
   sides), the hover pick and the world keys headless (§3 rule 8.2);
   UI panels stay unsupported headless.
   Input that is a fixed-size C→S message can also be injected on an
   exact frame on both sides with `at <f> poke msg <id> <value>...`
   (`poke.md` §1 `msg`).
3. The screen → world click point (`ui/controls.md` §6 r2,
   `0x0045AFF0`, PROVISIONAL controls-0001) is not 1.14d's: measured
   (2026-10-09, ScnAma seed 1234, eight clicks from a standing player,
   each a walk; the server path target `tx`, `ty` of the frame after
   the click). In the model target = floor((px + 2·py) / 32),
   floor((2·py − px) / 32) with px = sx − 400 + cx, py = sy − 300 + cy +
   k (cx, cy the player's client pixels from its 16.16 position,
   `render/camera.md` §2), d2rs' formula is k = 8 at every click; the
   first four 1.14d clicks ((600, 300), (300, 250), (450, 400), (333,
   333)) need k ∈ [13, 19] (k = 16 fits all four: the inverse would
   have no `− 8`), and the next four ((420, 280), (500, 350), (391,
   297), (407, 289)) fit no constant k on either side (an obstacle or
   walk-clamp adjustment?). A spec session should state `0x0045AFF0`
   and the server's target adjustment.
4. Not exact on the d2rs side of §3 rule 8.2: the hover pick is the
   preview's box over the model's unit cells, not 1.14d's sprite
   hit-test (`0x00467A10`); `clickunit` reads cell centres where 1.14d
   reads the 16.16 position; the held repeat once per server frame
   (1.14d runs it every client loop pass, which the debugger slows);
   the walk prediction standing in for the 1.14d client's own path of
   the local player; only the world keys of §3 rule 8.2. The 1.14d
   side of `combat-melee-fallen` / `combat-potion-midfight` settles the
   pick and the keys (`docs/HANDOFF.md` §5).
5. PROVISIONAL REC-1310: the items channel reads the created items from
   S→C 0x9C / 0x9D (§3 rule 13) rather than from a hook at the 1.14d
   item-creation function (`0x00558D90` and its family,
   `items/generation.md`): the queue hook is already pinned on both
   sides and carries the exact bit stream. A creation hook would also
   see items the client is never sent (edge case 4) and the creating
   call (drop / store / gamble / cube / quest) directly instead of the
   message action; it needs the d2rs creation path to write the same
   record.
6. DECIDED REC-2055 (owner, 2026-10-10: the client gap is a gap in d2rs' recording, not a
   difference in the game): a fidelity-ledger row counts as EQUAL when
   every channel of its checks is MATCH except a state channel whose
   only PARTIAL cause is the d2rs header's client gap (`state_dump.rs`
   `RUN_GAPS`: the headless bridge's C→S set), with every unit field of
   both sides compared (no `ignore` line, nothing one-sided), and either
   no `input` or `send` line in the check (pokes only) or a packets
   channel that MATCHes (its C→S stream equal: the gap's condition, a
   1.14d client sending another message, did not occur). The comparator's verdict stays PARTIAL (`state-
   snapshot.md` §1 rule 1, §4 rule 5); the ledger rule settles the row.
   Covering the 1.14d client's C→S set in the bridge removes the gap.
7. DECIDED REC-2056 (owner, 2026-10-10): likewise, an items channel PARTIAL whose only
   cause is "no item created on either side" (§3 rule 13, edge: nothing
   compared) counts as equal for such a row (zero items on both sides).
