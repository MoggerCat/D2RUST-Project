# PC 2 recording lane (`claude/pc2-recordings`)

Unattended recordings of the original 1.14d game on PC 2 (Windows,
reference `Game.exe`, real GPU), no player at the keyboard. Raw files stay
in `traces/raw/` (gitignored); only normalized traces a spec asks for are
committed. Every run ends with the game killed (checked with `tasklist`
after each run: no `Game.exe` left).

## Wave F: unattended game start (done)

`tools/trace-recorder/autostart.py` (README "autostart.py"): every
`record_*.py` takes `--auto CHAR [--seed N] [--input SCRIPT]`.

- Path: the game's own options `-name CHAR -seed N -nosave` fill the
  launcher config (option table `0x705040`); the menu is left for client
  mode as `dump_tables.py` does (`0x7795E8` := 1, `0x72DDD4` := 0, only in
  mode 4, after 6 s so the main menu is up: at 3 s the title screen is
  still running and the forced client returns to the menu); the client
  starts a single-player expansion game. Arrival in level 1 about 6.3 s
  after launch.
- Character must be an expansion character: `TestAma` (classic, from
  `d2s-tool new` without `--expansion`) gets "A Diablo II character cannot
  join a game created by a Diablo II Expansion character" and the client
  returns to the menu. `ScnAma` / `ScnSor` (expansion, level 1) work.
- Seed: `-seed 1234` → client act init seed 1234 (twice, identical arrival
  position [4873, 4228]); `-seed 99` → 99, position [6023, 4933]; no
  `-seed` → 1248214865 = `ScnAma.d2s` map ID at 0xAB (confirms `rng.md`
  §5.4's observation). The save file's mtime is unchanged (`-nosave`).
- Input: `PostMessageW` clicks walk the player (click (600, 300): position
  [4873, 4228] → [4889, 4236], screenshot shows the walk). Window may be
  in the background.
- Proof run: `py tools/trace-recorder/record_tick.py --seconds 40 --auto
  ScnAma --seed 1234 --out traces/raw/auto-tick-1234.jsonl` → 840 ticks;
  `check_tick.py`: 0 errors (1452 timer runs, 34 snapshots);
  `check_units.py --tables <20261006-201456-tables>`: 0 errors (924
  schedules exactly checked, U11 site rule 923, U4 anim exact 1).

## Other recorder work on PC 2 branches (not merged here)

`origin/claude/local-buddy-q9-rec-2026-10-07` (and `-q-rec`) have
`spawn.py`, `record_join.py`, `record_objects.py`, `record_sound.py`,
`run_scenario.py`, `d2ui.py`, `make_saves.py`: `spawn.py` starts the game
the same way (`-name`, forced menu) and drives the window with
`SendInput` (foreground); `run_scenario.py --probe` covers the seed
override. `autostart.py` differs in: one option set for every
`record_*.py`, `-seed` checked against the act init seed, background
input (`PostMessageW`), memory-aimed `goto`, `waitlevel`, `dumpdrlg`.

## Recordings

| # | File (traces/raw/) | Command | Duration | Check | Settles |
|---|---|---|---|---|---|
| 0 | `auto-tick-1234.jsonl` | `record_tick.py --seconds 40 --auto ScnAma --seed 1234` | 40 s, 840 ticks | `check_tick` 0 errors; `check_units` 0 errors | proof of the unattended start |
| D1 | `pc2rec-d1-rng.jsonl` (sha256 8ac70b456cea9732…) | `record_rng.py --seconds 1500 --auto TestSor --seed 644409375 --input "wait 3; dumpdrlg act1; goto 2 119 300; wait 3; click 178 75; wait 2; click 200 138; waitlevel 40 400; wait 5; dumpdrlg act2; goto 2 156,157,237,238,288,323,324,398,402 300; wait 3; click 240 75; wait 2; click 200 138; waitlevel 75 400; wait 5; dumpdrlg act3; goto 2 <same> 300; wait 3; click 300 75; wait 2; click 200 138; waitlevel 103 400; wait 5; dumpdrlg act4; shot act4; end"` | 86 s (Act I at 22.8 s, II 49.9 s, III 64.4 s, IV 81.1 s), 96,978 events | `check_rng` OK (0 unexplained, 138 sites); `check_drlg_acts` 4 records 0 errors | `drlg/levels.md` OQ1, OQ3 (start and waypoint arrivals); `outdoor.md` OQ1 (rects), OQ4/OQ7; `outdoor-act3-act5.md` OQ1 (creation): all in `xpc-to-pc1.md` (PC 1 specs) |

Raw files are also copied to the main checkout's `traces/raw/` on PC 2.

DRLG per seed without hooks (`autostart.py --try TestSor --seed 1234`,
Acts I–IV in 27 s): `check_drlg_acts` 4 records 0 errors (tombs 67 / 66,
jungle bit 0). The same check runs on any `--auto` recording with
`dumpdrlg` (perturbation: `--perturb N`, and `--selftest`).

## Not doable unattended (and why)

| Entry | Why |
|---|---|
