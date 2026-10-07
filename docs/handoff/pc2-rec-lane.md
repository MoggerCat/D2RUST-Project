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
| D2 | `pc2rec-d2-sweep.log` | `autostart.py --try TestSor --seeds 1,2,3,7,42,1000,31337,65535,123456789,2147483647,987654321,555 --seconds 120 --input "wait 1; dumpdrlg act1; goto 2 119 60; wait 2; click 178 75; wait 1; click 200 138; waitlevel 40 40; wait 2; dumpdrlg act2; goto 2 156 60; wait 2; click 240 75; wait 1; click 200 138; waitlevel 75 40; wait 2; dumpdrlg act3; end"` (no hooks) | 12 games, ~1 min each | `check_drlg_acts`: 36 records, 0 errors, 271 unbuilt level seeds; `--perturb 4` / `--perturb 5`: 1 error each | mass check (M08) of `drlg/levels.md` §3 step 2–4 and §4.3 over 12 seeds × Acts I–III: Act II tombs, Act III jungle bit (seed 555: 0, the others 1), every `{dwStartSeed + id, 666}` |
| D1b | (log only) | `autostart.py --try TestSor --seed 644409375 --input "...; click 240 75; click 200 138; waitlevel 75; dumpdrlg act3"` | 25 s | — | Act III jungle block ids (+0x1BC array) 76: 541, 533, 543, 565, 570, 582, 571, 575, 570, 575, 537, 0; 77: 554, 577, 541, 0, 539, 534, 0, 541, 576, 569, 576, 566; 78: 0, 533, 535, 565, 570, 582, 539, 534, 558, 565, 541, 581 = the derived vector of `outdoor-act3-act5.md` |
| P1 | `pc2rec-p1-packets.jsonl` | `record_packets.py --seconds 300 --auto TestSor --seed 644409375 --input "wait 3; key ENTER; text hello pc2; key ENTER; …(3 chat lines)…; goto 2 119 60; click 178 75; click 200 138; waitlevel 40 60; wait 3; goto 2 156 60; click 115 75; click 200 138; waitlevel 1 60; wait 3; end"` | 35 s in game, 688 ticks | `check_packets` OK | `world/waypoints.md` OQ1 answered (spec edited; new OQ8 on the grey panel entries); `sim/intents-events.md` OQ14 / OQ7 → `xpc-to-pc1.md` |
| P2 | `pc2rec-p2-packets.jsonl` | `record_packets.py --seconds 120 --auto TestSor --seed 644409375 --input "<6 chat lines with whisper prefixes *, /m, /msg, /whisper, @>"` | 30 s | `check_packets` OK | the SP client sends every prefix as plain text, empty target (`xpc-to-pc1.md`) |

Raw files are also copied to the main checkout's `traces/raw/` on PC 2.

DRLG per seed without hooks (`autostart.py --try TestSor --seed 1234`,
Acts I–IV in 27 s): `check_drlg_acts` 4 records 0 errors (tombs 67 / 66,
jungle bit 0). The same check runs on any `--auto` recording with
`dumpdrlg` (perturbation: `--perturb N`, and `--selftest`).

## Not doable unattended (and why)

| Entry | Why |
|---|---|
| Fights, kills with drops, hireling level-up / death / resurrect (`world/hirelings.md` R2-27 rest), umod deaths, skills on monsters (`skills/bodies*.md`, `monsters/umod-callbacks.md`) | the script has no combat loop: aiming at moving monsters and surviving needs feedback the input script does not have (`spawn.py` on `local-buddy-q9-rec` places monsters by debugger call, which is the better base for these) |
| Quest chains (`world/quests*.md` OQs needing completed quests, orifice, Cain) | progression: the saves at hand have no completed quest; `d2s-tool --quests` flags do not give the in-world quest states |
| Act V entries (levels 109–132: `outdoor.md` OQ9, `outdoor-act3-act5.md` OQ3 part 2) | `TestSor`'s waypoint panel has tabs I–IV only (no Act V access in the save) |
| Two-client games (hireling seen by a second client, R1 on a hosted game) | needs a second game client joined over TCP/IP; not attempted |
| `render/draw-order.md` OQ7 (TownE1 tile (950, 933)) | `local-buddy-q9-rec` R2-1: the cells are off-screen from every reachable spot |
| Classic-game messages (`monsters/init.md` OQ1 0x67 in classic SP) | the forced start always makes an expansion game; a classic character is refused |
