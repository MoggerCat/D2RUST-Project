# Spec: Tools — One-command checks against 1.14d (`scenario-diff`)

- **Status:** draft: the check-file format and the run; the `state`
  channel runs on both sides, `draws` reuses the rendering-facts tools,
  `packets` runs on both sides (`tools/packets-trace.md`), `rng` runs on
  both sides (`tools/rng-trace.md`); one shared input script drives
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
| Summary | 33–41 |
| Inputs | 42–48 |
| Outputs / state changes | 49–59 |
| Rules | 60–61 |
|   1. Files | 62–67 |
|   2. Syntax | 68–112 |
|   3. Run | 113–239 |
| Constants & data dependencies | 240–243 |
| Randomness | 244–247 |
| Edge cases & original bugs | 248–266 |
| Test vectors | 267–277 |
| Provenance | 278–281 |
| Open questions | 282–310 |
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
  game never writes it back).
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
| `channels <ch>...` | no (`state`) | from `state`, `draws`, `rng`, `packets` |
| `draws-at <tick>` | with `draws` | the server tick whose frame is compared (≤ `ticks`; 1.14d's last drawn tick at or before it, §3 rule 7.2) |
| `input <script>` | no | the shared input script of rule 4, given to both sides (excludes the two lines below) |
| `input orig <script>` | no | `autostart.py` input script (seconds, client pixels) |
| `input d2rs <script>` | no | `d2-client play --input` script (server ticks; `state-dump` takes it only in rule 4's form) |
| `ignore <field>...` | no, repeatable | state fields not compared (`state_diff.py --ignore`) |
| `at <frame> poke <directive> <args...>` | no, repeatable | state injection (`tools/poke.md` §1, §2 rule 6; `spawn` lines as §1 rule 3): `<frame>` ≥ 1 is the absolute game frame (the `f` of `tools/state-snapshot.md`); applied after frame `<frame>` − 1 and its snapshot, before frame `<frame>`'s drain; passed in file order to every 1.14d recorder and to d2rs `state-dump` / `play` as `--poke "<frame> <directive> <args...>"` |

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
      and up in one pass.
   5. No other step (`wait`, `text`, `shot`, `goto`, … are
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
      --at-tick <the compared tick> [--input <input d2rs>]` (a stale
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
9. **packets** (`tools/packets-trace.md`; work dir files in brackets):
   1.14d `record_packets.py` with the rule 2 start
   [`orig.packets.jsonl`]; d2rs `d2-client state-dump --save --seed
   --difficulty --ticks <ticks> --out d2rs.packets-state.jsonl --packets
   d2rs.packets.jsonl` (with `--input` as the state channel takes it);
   `packets_diff.py orig.packets.jsonl d2rs.packets.jsonl --next N`, whose
   exit code is the channel's. `record_packets.py` has no poke layer: a
   check with `at … poke` lines reports the channel as not compared
   (partial). First run (2026-10-09, `packets-town-arrival-ama.check`,
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
      call the window path makes after the UI): no panel, no hover pick
      (a click on a unit is a point click), no shake, run off (mods 0);
      the held repeat runs once per pass (one pass per server frame)
      while a `hold` lasts. The local player the click reads is `play`'s
      walk prediction (`bridge::predict`, d2rs-own, unverified: the
      client model's cell stays at a walk's start, REC-51). `key` steps
      are refused (no controls layer headless). Each applied step is a
      stderr line and a footer note `input: frame F: <step> after frame
      L`.
   3. d2rs `play --input` (Bevy): `frame F` waits for the bridge's
      server tick F − 1; the events reach the UI on the next loop pass;
      `hold` releases N ticks later; `key` steps are skipped.
   4. Measured (2026-10-09, `walk-town-ama.check`: ScnAma, seed 1234,
      `frame 10; click 600 300`): two 1.14d runs give byte-identical
      snapshots (60 frames); the player's mode turns 6 (town walk) at
      frame 10 on both sides; the path target is (4880, 4223) on 1.14d
      and (4880, 4222) on d2rs (open question 3), so `xf` / `yf` differ
      from frame 10, `x` from frame 14, `y` from frame 20, and the walk
      ends (mode 5) at frame 32 on 1.14d, 34 on d2rs. The step length
      per frame is the same (|(Δxf, Δyf)| 24 571 vs 24 572).
10. **rng** (`tools/rng-trace.md` §6; work dir files in brackets): 1.14d
    `record_rng.py --frames` with the rule 2 start [`orig.rng.jsonl`];
    d2rs `cargo run --release -p d2-client --features rng-trace --
    state-dump --save --seed --difficulty --ticks <ticks> --out
    d2rs.rng-state.jsonl --rng d2rs.rng.jsonl`; `rng_diff.py
    orig.rng.jsonl d2rs.rng.jsonl --next N`, whose exit code is the
    channel's. `at … poke` lines or a shared `input`: not compared
    (partial; `record_rng.py` has neither); `input orig` / `input d2rs`:
    1.14d only, partial at best. First run: `rng-town-arrival-ama.check`
    (`tools/rng-trace.md` Open questions 1).

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
3. Two 1.14d runs at the same time on Linux share the Wine prefix and
   the `:99` display unless told apart: the first `run.sh` to end runs
   `wineserver -k` and kills the other's game (seen 2026-10-09: the
   capture stopped at tick 5, exit 1, no message). Give a parallel run
   its own `WINEPREFIX` (a `cp -al` copy of `~/.wine-d2` costs no disk)
   and `D2_DISPLAY`; the save goes to that prefix (rule 1).

## Test vectors

| Input | Expected |
|---|---|
| `--selftest` | the parser accepts the sample and every committed check; each malformed line kind is rejected; a dry run issues the recorder, state-dump and comparator commands with the check's save and seed |
| `--selftest` (draws) | the dry run issues `record_frames.py --every 1 --draws-every 1 --no-save`, `facts_render.py --frame`, one `cargo build`, the binary's `play --dump-draws … --at-tick <draws-at>` and `facts-compare … --ignore tick`; a one-side run issues no compare; the frame pick returns the frame at the tick, else the last earlier one with a draw log; a scene's `frame.tsv` tick is read back |

| `--selftest` (input) | a shared `input` line reaches `record_state.py`, `state-dump`, `record_frames.py` and `play` as `--input '<script>'`; a script not starting with `frame`, with `wait` / `shot`, a frame 0 or going back, a `hold` without N or non-integer arguments, and a shared line next to `input orig` are rejected; an `input d2rs` in play's tick form goes to `play` only |
| `autostart.py --selftest` | `frame` / framed `hold` parsing and rejections; at simulated tick-return stops the click, hold press, key and move are posted at the stop of F − 1, the hold's release at the stop of F + N − 1; a late stop runs the step there |
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
   rule 8); `state-dump --input` applies it through the bridge.
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
4. Not exact on the d2rs side of §3 rule 8.2: no hover pick (clicks on
   units), the held repeat once per server frame (1.14d runs it every
   client loop pass, which the debugger slows), the walk prediction
   standing in for the 1.14d client's own path of the local player, no
   keys.
