# PC 1 autotest loop: test d2rs against real 1.14d, unattended

Started 2026-10-08 (third coordinator, session_01QeN5r8PoLZsUhwAH2iDzLJ).
Goal: run every check against the original 1.14d that needs **no person at
the keyboard**, as many and as fast as PC 1's hardware allows, and turn
every result into either a verified claim or a precise finding. No token
limit (user, 2026-10-08); the only limits are PC 1's CPU, RAM, disk and
the one-game-at-a-time rule below.

This loop **replaces** `docs/handoff/pc1-loop.md` while it runs: if a
pc1-loop session is running, stop it (its Lane A becomes Lane F here).

## Ground rules (from CLAUDE.md, unchanged)

- **No Blizzard files in git:** MPQs, Game.exe, extracted assets or
  tables, saves and raw recordings stay in `game/` and `traces/raw/`, which
  are gitignored. Converted traces go where `traces/FORMAT.md` says.
  The pre-commit hook enforces this. Never bypass it.
- **No code in `crates/`** beyond the `// Covers:` claim unlocks of
  `docs/LOCAL-RUN.md` §0.2. Every code bug becomes a row in
  `docs/handoff/build-queue.tsv` (`q-fix-<topic>`: spec rule, observed vs
  expected, the failing command); the cloud coordinator launches it. A
  spec fact you learn from the binary or a trace goes into its owner spec.
- **Never weaken a check.** A mismatch is a finding: never change the
  expected value to make it pass (M01). Change it only when the 1.14d
  observation *and* the spec agree on the new value; say so in the commit.
- **Restore state:** registry values and settings you change go back
  after each run. Copy test saves read-only into the save folder and
  remove them afterwards.
- **Hand-back:** work on `claude/local-pc1-test`, created from
  `origin/claude/specs-staging-7` (staging is ahead of main and is where
  fixes land). Before each push, merge (never rebase)
  `origin/claude/specs-staging-7`, then run `py tools/spec_index.py`,
  `py tools/spec_index.py --check` and `py tools/coverage.py --check`.
  Push after every finished item. After each push, if `send_message`
  is available, send session_01QeN5r8PoLZsUhwAH2iDzLJ the branch, the
  head SHA and one line (passes / findings / rows added).
- **REC ids for new provisional notes:** REC-350..REC-399 only.

## Hardware budget (detect once, then obey)

At start, read the core count, RAM and free disk (`Get-CimInstance`,
`Get-PSDrive`) and write them at the top of `docs/handoff/pc1-autotest-log.md`.
Then:

- **Game slot: exactly one `Game.exe` at a time.** Debugger recorders,
  autostart runs and screenshot runs all hold it. Queue the game jobs and
  run them one after another, with no idle gap.
- **Build slot:** one cargo build at a time, `CARGO_BUILD_JOBS` = cores − 2
  while no game runs, and cores / 2 while one does, so the game keeps
  frame timing. Build release once (`cargo build --release -p d2-client -p
  scenario-run -p data-tool -p mpq-tool -p d2s-tool`), then reuse it.
- **CPU slot:** checks that need neither the game nor a build run in
  parallel, up to cores / 2 at once: `check_*.py`, `scenario-run
  compare`, `data-tool`, `mpq-tool`, and the `--ignored` tests of a crate
  already built (`cargo nextest run --run-ignored only` with
  `--test-threads` = cores / 2).
- **GPU slot:** one GPU verify at a time.
- **Disk:** keep 15 GB free. Delete processed raw recordings after
  converting or checking them; keep only the converted traces and the
  logs. `cargo clean -p` crates you no longer build.
- **Subagents:** up to 6 at once, for analysis only: reading results,
  diffing traces, writing findings and spec text, preparing scenario
  scripts. Only the main session starts the game or a build.

## Lanes, in priority order (run A–C back to back; fill idle slots with D–F)

### Lane A: the local run queue that needs no player

`docs/LOCAL-RUN.md`: §0.1, then Batches 1, 2, 3 and 5. Next, the entries
of `docs/HANDOFF.md` §5 marked game files / GPU only (section C), then the
"Local check" blocks of every `docs/handoff/q-*.md` that print counts or
compare data (no window).

Record per `LOCAL-RUN.md` §0.2: a Done block with the command, counts and
pass / fail, the `// Covers:` unlocks, and `coverage.py --summary`
figures in §1.

### Lane B: unattended recordings of 1.14d (the PC 2 list, [AUTO] entries)

`docs/HANDOFF.md` §7 "PC 2 recording list": run every entry tagged
**[AUTO]** in its listed order, with `tools/trace-recorder/autostart.py
--auto CHAR [--seed N] [--input SCRIPT]` under the entry's `record_*.py`.

- Characters come from `d2s-tool new`. Copy them read-only into the
  save folder.
- After each recording, run its `check_*.py` and convert what the entry
  says.
- Mark each entry `Taken by PC 1 <date>` before you start it and `Done`
  with the result. PC 2 skips taken entries.
- An entry that needs a hook the recorder lacks (**NEW HOOK**): add the
  probe to the recorder (a tool, not `crates/`; addresses from the entry
  or its spec), with its `--selftest` / perturbation, then record.

Priority inside the lane:

1. The settling captures of the newest REC points (REC-160 and up,
   `coordinator-resume.md`). These cover choices the cloud built on
   today: item bonus streams (equip +stat / +resist gear: 0x9C/0x9D, 0x1D,
   REC-163/177/188), the weapon switch W (0x60, 0x9D, 0x97), Whirlwind
   through a pack (REC-232), the Esc and options menus (REC-237, REC-257),
   the loading / act-change order (REC-221/223/251), chest drops (REC-260),
   Duriel's Lair arrival (REC-234/254).
2. RNG draw order and wire byte layouts (M22: these spread silently).
3. The rest of the list.

### Lane C: differential scenarios (d2rs vs 1.14d, same script)

This is the main autonomous fidelity test. For each scenario script
(`specs/tools/scenario.md`; existing scripts with the tool, new ones
written by you):

1. Record 1.14d playing it: `autostart.py --auto CHAR --seed N --input
   SCRIPT` under `record_packets.py`, plus `record_rng.py`,
   `record_tick.py` or `record_stats.py` as the scenario needs.
2. Run d2rs on the real data: `scenario-run run SCRIPT --game-dir
   $env:D2_GAME_DIR`.
3. `scenario-run compare <original> <d2rs>`: exit 0 is a match, 1
   diverged, 2 partial, 3 error.
4. On a divergence, find the **first** differing record. Name the system,
   the spec rule and the REC id if it is a provisional point. Write a
   finding: a `q-fix-<topic>` row when the code disagrees with the spec,
   or a spec correction (with addresses, from `re/` when needed) when the
   spec was wrong.

Scenario set to build up, each with a fixed seed and expansion
characters:

- **Basics:** town idle 30 s; walk Rogue Encampment → Blood Moor; cast
  each class's level-1 skills on a pack; pick up drops; open a chest; use
  a shrine.
- **Systems:** waypoint use (A1 → A2 → A1); town portal round trip; NPC
  buy / sell / repair / gamble; hire a mercenary; identify with Cain;
  stash and cube moves; equip, unequip and swap.
- **Generation:** one per act, entering each level with a fixed seed,
  compared against the `dumpdrlg` records (DRLG + population).
- **Saves:** level-ups and point spends; save and exit; reload. Also diff
  the `.d2s` files of 1.14d and d2rs with `d2s-tool` (C66).

Grow the set as scenarios pass, so every merged q-* feature gets one.

### Lane D: render and capture compares (GPU slot, unattended)

`LOCAL-RUN.md` Batch 3 and §6.1. Record frames with `record_frames.py`
under `autostart.py --auto` (no player needed when an input script drives
the camera). Run `d2-client verify` against the CPU reference and the
capture cases in `crates/d2-client/capture-cases/`. Then add capture cases
for today's UI work (Esc / options menu, character panel, belt, cube
transmute frames, loading screen) when a scripted input reaches them.

### Lane E: the d2rs client on real data (window, unattended)

`cargo run --release -p d2-client -- play --new <class> Test` and `--save
<file>` with a timeout. These are smoke runs: no panic, the game reaches
the world, and Save and Exit writes a `.d2s` that 1.14d loads (`d2s-tool`
check, plus a 1.14d `autostart --try` load). If an unattended input hook
exists, drive it; if not, launch it, check the log and the exit only, and
note that.

### Lane F: settle provisional points from the binary (when slots idle)

As `pc1-loop.md` Lane A: `git grep -n PROVISIONAL specs crates`. Answer
the binary-answerable ones from `re/` into their owner specs. Each answer
whose code disagrees becomes a `q-fix-*` row.

## Output per round

Append to `docs/handoff/pc1-autotest-log.md` (newest first):

- the hardware line;
- per lane: items run, pass / fail counts, findings with their spec rule
  and REC id, rows added, recordings taken, coverage figures before and
  after.

Push, hand back, then start the next round from a fresh merge of staging,
since the cloud lands new code every hour. Keep running rounds until
Lanes A–C are empty for the current staging head; then idle-poll staging
every 30 minutes for new merges and rerun the scenarios they touch.
