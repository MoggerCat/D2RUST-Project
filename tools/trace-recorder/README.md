# trace-recorder

Records behavior of the original 1.14d `Game.exe` with a small debugger
and turns it into traces (`traces/FORMAT.md`). Version 0.1.0 records the
seeded RNG (`specs/sim/rng.md`). Python 3.10 (`py`), standard library
only, Windows only. Our own code; nothing here is derived from Blizzard
code. Spec-role tool: the addresses it hooks are documented in
`specs/sim/rng.md` (Provenance).

| File | Does |
|---|---|
| `record_rng.py` | Launches `game/Game.exe` under the Win32 debug API, hooks every seeded-RNG step, writes `traces/raw/<time>-rng.jsonl` (gitignored) |
| `check_rng.py` | Recomputes every recorded event from its own seed-before with `d2rng.py`; exit 1 on any mismatch |
| `convert_rng.py` | Splits a raw file into seed chains and writes chosen chains as `traces/sim/rng/sim-NNNN.json` |
| `d2rng.py` | The RNG rule from the spec (step and the five helpers), used by the two above |
| `rng_owners.py` | Owner post-pass of a `record_rng.py --frames` file: `game`, `unit T:G` or `other:…` on every draw and seed write (`specs/tools/rng-trace.md` §2); `--selftest` |
| `rng_diff.py` | Compares a 1.14d and a d2rs (`d2-client state-dump --rng`) RNG recording: per frame, per owner, draw by draw; first divergence, next N, the game seed's draws in order, per-owner summary; exit 0/1/2/3 as `state_diff.py`; `--selftest`. Spec: `specs/tools/rng-trace.md` |
| `record_packets.py` | Launches `game/Game.exe` under the debugger, logs every client→server message at the server's queue read and dispatch (with result), every server→client message as queued and every flushed buffer, with tick markers and the frame number; `--ticks N` ends after tick N's flush; writes `traces/raw/<time>-packets.jsonl` (gitignored; format `packets-raw-1`). Specs: `specs/sim/intents-events.md`, `specs/tools/packets-trace.md` |
| `packets_diff.py` | Compares a 1.14d packets recording with a d2rs one (`d2-client state-dump --packets`): per frame the C→S, S→C and flushed-buffer streams record by record (id, size, bytes; `scenario-masks.tsv` masks; transport rows excluded); first divergence with context, next N, summary; exit 0/1/2/3; `--selftest`. Spec: `specs/tools/packets-trace.md` |
| `path_tables.py` | Checks `specs/sim/path-tables.tsv` against the reference `Game.exe` file image (no process); `--write` regenerates it, `--perturb N` must report row N, `--selftest` perturbs every row. Spec: `specs/sim/pathing.md` |
| `check_packets.py` | Checks a packets recording against `specs/sim/intents-events.md` and its two TSVs (rules R1–R7); `--perturb N` must report seq N; `--selftest` runs a synthetic trace and every single-byte perturbation |
| `record_tick.py` | Launches `game/Game.exe` under the debugger, logs each server tick and its step markers, every timer event scheduled, cancelled and run, every unit/room/update-queue list change, and list snapshots every 25 frames; writes `traces/raw/<time>-tick.jsonl` (gitignored). Specs: `specs/sim/tick.md`, `specs/sim/unit-order.md` |
| `check_tick.py` | Replays a tick recording through a model of those specs: must predict every timer run and reproduce every snapshot; `--perturb-ex N`, `--perturb-snap N` must fail at the changed record; `--selftest` runs a hand-built recording of the specs' test vectors |
| `record_stats.py` | Subclass of `record_tick.py`'s `TickRecorder` (tick and step hooks only): logs every base write, attach, detach, free, dynamic toggle, by-time refresh, state toggle, expiry and value-change callback on server stat lists, the regeneration entry points, and snapshots of the players' and monsters' list trees; writes `traces/raw/<time>-stats.jsonl` (gitignored). Specs: `specs/sim/stats.md`, `specs/sim/stat-lists.md` |
| `check_stats.py` | Replays a stats recording through a model of those specs: must predict every callback, expiry and regeneration value and reproduce every snapshot; `--perturb-snap N`, `--perturb-cb N` must fail at the changed record; `--selftest` runs a hand-built recording of the specs' test vectors; `--files game` checks the specs' itemstatcost facts on the 1.14d table |
| `record_state.py` | Subclass of `record_tick.py`'s `TickRecorder` (tick entry hook only, plus the tick return `0x0052FD1E`): after every server tick (or every N-th) writes the game seed and every server unit (type, GUID, class, mode, position, fraction, target, direction, animation frame / count / speed, unit seed, act, level id, life / mana / stamina and base stats) to `traces/raw/<time>-state.jsonl` (gitignored; format `state-1`); `--selftest` checks the reader on a synthetic game (exact records, every source byte perturbed). Spec: `specs/tools/state-snapshot.md` |
| `check_units.py` | Checks the per-kind timer-event rules U1–U11 of `specs/sim/units.md` on a tick recording (tables from a `dump_tables.py` directory); `--perturb N` must fail at the changed record; `--selftest` runs a hand-built recording |
| `record_frames.py` | Launches `game/Game.exe -w -ns` under the debugger (base: `record_tick.py`), and at each in-game `EndScene` (`0x4F6190`) reads the 8-bit index framebuffer and the GDI palette, ties the frame to the last server tick and logs the camera/player, level, cursor, seed, light-quality and weather state, and with `--draws-every N` every draw call of every N-th frame; writes `traces/raw/<time>-frames.jsonl` (format `frames-raw-3`: also per cel draw the cel header w/h/xoff/yoff from the rasterizer, per tile draw its DT1 file and index, `compfile` records naming unit component files, the player's direction) and palettized PNGs `frame-<seq>.png` to `game/captures/<time>/` (both gitignored); prints the stability verdict (`capture.md` §7); `--selftest` checks the PNG writer, the state readers (perturbation), the stability count and the front-end mode. `--front-end --front-end-script SCRIPT`: no game start; every `EndScene` call is a frame (any caller: menu `0x4F9A67`, loading `0x4567D4`, in game `0x44CB4F`; numbered `present`, no tick, no game state unless in game), D2Win cel files named through `0x4FA9B0`; SCRIPT is autostart's input script with `shot NAME [change|0xRET]` (capture the next presented frame with its draw log as scene NAME; `change`: the next frame from another caller, e.g. the loading screen), mouse and keys as X input under Wine (`--x-input`). Spec: `specs/render/capture.md` (§2a) |
| `facts_join.py` | Turns one `record_packets.py` raw file (`packets-raw-1`) into message facts (`join-facts-1`): every C→S message the client sent and every S→C message queued, with frame, id, size, caller and bytes, from the start through `--frames N` (default 2: the join) or a later window (`--from F --frames 0`, `--skip` ids), into `facts/join/<name>.tsv` (or `--out-dir`) with the command and the run in the header. q-prov-recording |
| `facts_render.py` | Turns one `record_frames.py` capture (`frames-raw-3`; `frames-raw-2` with the raw-3 cells `?`) into rendering facts in the format of `specs/tools/facts-render.md` §1–§4 (read by `d2-client facts-compare`): `facts/render/scenes/<scene>/draws.tsv` (every draw call of one frame), `frame.tsv` (state and the index / palette digests) and `facts/render/sprites.tsv` (distinct cel file, direction, frame with the cel's w, h, xoff, yoff; merged). Measurements and digests only; `--shot NAME` picks a `--front-end` shot (game keys `-`, `tick` `?`); `--selftest`. Plan: `docs/handoff/pc1-data.md` Step 3 |
| `autostart.py` | Unattended start for every `record_*.py`: `--auto CHAR [--seed N] [--input SCRIPT]` starts a single-player game with that expansion character (no player at the keyboard), optionally with a fixed map / game seed, then plays a scripted input (clicks, keys, screenshots) into the window; `--try CHAR` runs it alone; `--selftest` |
| `check_drlg_acts.py` | Checks the `dumpdrlg` records of an `--auto` run against `specs/drlg/levels.md` §3–§4 (rules D1–D7); `--perturb N`; `--selftest` |
| `dump_tables.py` | Launches `game/Game.exe` under the debugger, stops when the excel load and its fix-ups have finished, writes every loaded table and the runtime maps it knows to `traces/raw/<time>-tables/` (gitignored); compared by `data-tool dump-compare` |
| `poke.py` | Subclass of `record_tick.py`'s `TickRecorder` (tick hook plus the tick return `0x0052FD1E`): runs poke directives (`specs/tools/poke.md`) between two server ticks, calling the game's own creation functions or writing the state field; `--poke-file FILE` (ticks relative to F0), `--poke "F D ..."` (absolute frame), `--forms FILE` (call forms, `CALL_FORMS`); writes `traces/raw/<time>-poke.jsonl` (format `poke-raw-1`, gitignored); `PokeLayer` / `add_options` for other recorders; `--selftest` |

## Use

```
py tools/trace-recorder/record_rng.py --seconds 45          # Game.exe -w -ns
py tools/trace-recorder/check_rng.py traces/raw/<file>.jsonl
py tools/trace-recorder/convert_rng.py traces/raw/<file>.jsonl --list
py tools/trace-recorder/convert_rng.py traces/raw/<file>.jsonl --chain 3 --limit 64 --note "..."
```

`record_rng.py` options: `--seconds N` (default 30; the game is killed
then), `--no-inline` (helpers and setters only: fast, but misses most
draws), `--max-events N`, `--out FILE`, `--game PATH`, then Game.exe
arguments after `--` (default `-w -ns`). The script refuses any
`Game.exe` whose SHA-256 is not the reference 1.14d one
(`traces/reference-install.toml`). It never writes to `game/`.

The game is always terminated when the script ends: time limit, Ctrl+C,
an exception (`finally`), or the debugger process dying (kill-on-exit).

## How it hooks the RNG

1.14d has no single RNG function: the step is inlined at 846 places and
called through six small helpers (`specs/sim/rng.md` §3, §6).

| Hook | Where | Logged |
|---|---|---|
| helper entry | 6 helpers (INT3 on the first byte) | seed address (ECX), arguments (EDX, `[ESP+4]`), seed before, return address |
| helper return | temporary INT3 on the return address, per thread | returned value (EAX), seed after |
| seed setter | 3 setters (init, init-low, set) | seed address, old and new value, call site |
| inline step | INT3 on each `mov ecx|edx, 0x6AC690C5` found by scanning `.text` | see below |

Inline steps: from the `mov`, the thread is single-stepped. The first
`mul ecx|edx` executed while that register still holds the multiplier is
the step (this follows either branch where one `mov` serves two `mul`s);
EAX there is the old low word. Stepping continues to the `add`/`adc` pair
that builds the new state; their destination registers give the new
(lo, hi). The old hi word follows from `lo' - low32(lo * K)`, so an
inline record's real test is its new hi word and its link to the previous
state of the same seed.

While any thread single-steps, all other threads are suspended, so no
thread can run through a breakpoint that is temporarily removed.

Cost: every inline draw is about six debug events. With all hooks the
game runs far slower (entering Act 1 takes over 40 s instead of ~2 s);
`--no-inline` is near full speed.

### Frames and owners (`--frames`, version 0.2.0)

```
py tools/trace-recorder/record_rng.py --frames --ticks 40 --auto ScnAma --seed 1234 --seconds 240
py tools/trace-recorder/rng_diff.py traces/raw/<time>-rng.jsonl traces/raw/<name>.d2rs.rng.jsonl
```

`--frames` also hooks the tick entry `0x0052D870` (ECX = game; the first
game that ticks), writes a `tick` record per tick (`f` = game +0xA8 + 1,
the game seed at game +0xD0, every unit of the five server hash lists
with its seed at unit +0x20), puts `frame` (0 before the first tick) on
every record and, on helper / setter records, an owner hint (`game_seed`
or `unit` "T:G" when seed address − 0x20 is a server unit); after the run
`rng_owners.py` adds `owner` to every record. `--ticks N` stops at the
entry of tick N + 1. `--skip-inline drlg` (or `LO-HI,…`) leaves the
DRLG inline sites unhooked (faster; those draws are missing). The run
prints its speed. Measured under Wine (2026-10-09, `ScnAma`, seed 1234,
all 846 inline sites): arrival at 58 s, 40 ticks recorded by 63 s;
16,152 records, 103,471 debug events (1,643/s). Details and the
comparison: `specs/tools/rng-trace.md`.

## Raw format (`rng-raw-1`, JSON lines)

- First line `{"type": "header", ...}`: tool, date, `Game.exe` SHA-256,
  arguments.
- `{"type": "draw", "via": "helper"|"inline", "op", "site", "before":
  [lo, hi], "after": [lo, hi], "ret", "n"?, "min"?, "seed"?, "fn"?, "mul"?,
  "tid", "seq", "ms"}`. `site` is the call site (helper) or the `mov`
  address (inline). `seed` is the seed's address (helpers only).
- `{"type": "seed_set", "op": "init"|"init_low"|"set", "seed", "old",
  "new", "site", ...}`.
- Last line `{"type": "footer", ...}`: counts, notes, debug-event and
  foreign-exception counts.

## Traces

`convert_rng.py` writes format-1 traces, area `sim`, behavior `rng`,
`spec` `specs/sim/rng.md`. No format change was needed: RNG draws are a
new event kind, `rng_draw`, whose data fields are listed in the converter
and in `specs/sim/rng.md` (Test vectors). `tick` is 0 (untimed), and the
1.14d `site` is kept for reference and listed in `compare.ignore`.

## record_frames.py --front-end: the front-end scenes

The 4 front-end scenes in one run (cloud: under `tools/cloud-game/run.sh --python
--seconds 150 -- ...` with `--game 'Z:\root\game\Game.exe'`; the menu is up about 20 s
after launch under the recorder; times are for Wine, PC 1 is faster):

```
py tools/trace-recorder/record_frames.py --seconds 140 --front-end --front-end-script \
  "wait 25; shot main-menu; wait 5; click 400 307; wait 10; shot char-select; wait 5; click 117 498; \
   wait 12; shot char-create; wait 5; click 97 555; wait 10; shot loading change; key ENTER; wait 25; end"
for s in main-menu char-select char-create loading; do
  py tools/trace-recorder/facts_render.py traces/raw/<time>-frames.jsonl --shot $s; done
```

`shot NAME` arms the draw hooks at the next `EndScene` and captures the frame after it
(a frame is everything drawn between two `EndScene` calls, so the loading frame's
`ClearScreen` before its `StartDraw` is logged); `shot NAME change` keeps logging until a
frame comes from another `EndScene` caller than the one it was armed at (the loading
screen after OK / Enter). Character select needs a save (`prepare_saves.sh`: ScnAma is
the selected slot); `key ENTER` there starts the game. Under Wine the clicks and keys go
as X input (`xdotool`, the steps of `tools/cloud-game/xinput.sh`; `--x-input auto`), on
Windows by `PostMessage`. The footer notes list every `EndScene` caller with its first
present.

## autostart.py: unattended start and scripted input

```
py tools/trace-recorder/autostart.py --selftest                          # no game needed
py tools/trace-recorder/autostart.py --try ScnAma --seed 1234            # start, report, kill (~9 s)
py tools/trace-recorder/record_tick.py --seconds 40 --auto ScnAma --seed 1234
py tools/trace-recorder/record_packets.py --seconds 60 --auto ScnAma --seed 7 \
    --input "wait 2; click 600 300; wait 2; click 250 420; wait 2; end"
```

Every recorder (`record_rng`, `record_packets`, `record_tick`,
`record_stats`, `record_frames`) takes the same options: `--auto CHAR`,
`--seed N`, `--auto-after S` (default 6: the main menu must be up),
`--input SCRIPT`, `--shots DIR`. Without `--auto` nothing changes. The
recorder's debug loop calls `AutoStart.poll` (at most every 50 ms); the
header's `args` keep the game arguments, so a recording names its
character and seed.

| Step | 1.14d fact |
|---|---|
| character, seed, no-save | game options `-name` (launcher config +0xBD), `-seed` (+0x21A), `-nosave` (+0x219), from the option table `0x705040` (0x5C-byte rows, reader `0x405450`); `--auto` appends `-nosave -name CHAR [-seed N]`, so the save file is never written |
| leave the menu | in launcher mode 4 (`0x74C704`): next mode 1 into `0x7795E8`, 0 into the menu loop flag `0x72DDD4` (as `dump_tables.py`); client mode `0x44B8A0` starts a single-player game (config game type +0x19 = 0) |
| seed | `-seed N` sets the fixed-seed global (`specs/sim/rng.md` §5.2): game seed `{N, 666}`, game +0x7C = N; the client act's init seed (`[0x7A0634]+0x0C`) is N. Without `-seed` it is the save's map ID (`.d2s` 0xAB), as `rng.md` §5.4 observed |
| arrival | client player `0x7A6A70` → path +0x1C → room +0x10 → level +0x58 → id +0x1D0 (level 1 = Rogue Encampment); logged with the position and the act init seed |
| input | `PostMessageW` to the game's visible top-level window: `WM_MOUSEMOVE`, `WM_L/RBUTTONDOWN/UP` (client pixels), `WM_KEYDOWN/UP`; works with the window in the background |
| screenshots | `PrintWindow` from a helper thread (the window thread may sit at a debug event only the debugger thread continues), RGB PNG: for looking only; pixel captures are `record_frames.py` |

The game creates an **expansion** game: the character must be an
expansion character (`.d2s` 0x24 bit 0x20). A classic character gets "A
Diablo II character cannot join a game created by a Diablo II Expansion
character" and the client returns to the menu. Characters used so far:
`ScnAma`, `ScnSor` (expansion, level 1) in the save folder of this PC.

Input script (`;`-separated, run in order from the arrival): `wait S`,
`move X Y`, `click X Y`, `rclick X Y`, `hold X Y S`, `key K [S]` (letter,
digit, ESC, TAB, ENTER, SPACE, SHIFT, CTRL, ALT, F1–F12 or a number),
`text STRING` (WM_CHAR per character, e.g. chat), `shot NAME`,
`waitlevel ID [S]` (wait until the client player is in level ID; ends the
recording after S seconds, default 120), `goto TYPE CLASS[,CLASS…] [S
[DX DY]]` (walk toward the nearest unit of set S `0x7A5E70` with that
type and class by clicking toward it, wait until the player stands
still, click it at its draw point + (DX, DY), default (0, −8); screen
position from `render/camera.md` §2–§4), `dumpdrlg [LABEL]` (log the
client act's DRLG and level list, `drlg/levels.md` §1 offsets: act no,
init seed, DRLG seed, `dwStartSeed`, tombs, jungle bit, per level id,
DRLG type, flags, rooms, rect, level type, seed, jungle fields, warp
centre count), `end` (stops the recording; the game is killed as
always). Every log line also goes to the recorder's footer notes.
Arrival takes about 6.3 s after launch (about 23 s with `record_rng.py`
inline hooks).

Frame-anchored steps (the shared input form of
`specs/tools/scenario-diff.md` §2 rule 4, which d2rs `state-dump --input`
and `play --input` also take): `frame F` waits for the tick-return stop
`0x0052FD1E` of game frame F − 1 (game +0xA8); the steps after it are
posted while the game is stopped there, so the window takes them before
frame F's drain. After a `frame` step, `click` / `rclick` / `key` post
all their messages at once and `hold X Y N` holds N frames (the release
posted at the stop of frame F + N − 1). The stop is the one `poke.py`
uses (`AutoStart.attach` arms it only when nobody did): `record_state.py`,
`record_frames.py` and `poke.py` attach it when the script has a `frame`
step; other recorders never reach it (a note says so). Each posted step
is a footer note `autostart: frame F: <step> posted at the stop of frame
S` (S > F − 1: late, noted). Measured 2026-10-09 under Wine:
`record_state.py --auto ScnAma --seed 1234 --ticks 60 --input "frame 10;
click 600 300"` twice: identical snapshots; the player walks (mode 6,
target (4880, 4223)) from frame 10 and stops at frame 32.

Proved: a click at (600, 300) walks the player; `goto 2 119` opens the
Rogue Encampment waypoint; with `TestSor` (expansion, `d2s-tool new
--waypoints all --quests acts=4`; the panel draws the other waypoints
grey but takes them) the script `goto 2 119; click 178 75; click 200 138;
waitlevel 40` travels to Lut Gholein, and the same with tabs III / IV
(`click 240 75` / `click 300 75`, first entry `click 200 138`; town
waypoints 156, 237, 398) reaches Kurast Docks (75) and the Pandemonium
Fortress (103): Acts I–IV in 27 s with no hooks. The panel has no Act V
tab for this save.

The waypoint panel (800 × 600): act tabs at y 75, x 115 / 178 / 240 /
300; entries at x 200, y 138 + 35·k.

`check_drlg_acts.py` checks `dumpdrlg` records (footer notes of a raw
file, or a `--try` log) against `drlg/levels.md` §3–§4: init seed copy,
`dwStartSeed`, Act II tombs, Act III jungle bit, zero choices in the
other acts, `{dwStartSeed + id, 666}` for every unbuilt maze / outdoor
level, client flags; `--perturb N` must fail at record N; `--selftest`.

## dump_tables.py: the excel tables in 1.14d memory

```
py tools/trace-recorder/dump_tables.py                 # Game.exe -w -ns
cargo run --release -p data-tool -- dump-compare traces/raw/<time>-tables
```

Options: `--seconds N` (give up after N s, default 60), `--force-after N`
(default 6), `--no-force`, `--out DIR`, `--game PATH`, Game.exe arguments
after `--`. Same reference-hash check and kill guarantees as
`record_rng.py`, whose Win32 code it imports. One run takes about 7 s.

When the tables load: 1.14d starts in the menu (launcher mode 4) and loads
the excel tables only when client mode starts, i.e. when a game is
started. The client entry `0x0044B8A0` loads the string tables
(`0x005259C0`) and then calls the load-all routine `0x00619300` once. With
no table loaded after `--force-after` seconds and the game in mode 4
(`0x0074C704`), the script ends the menu the way starting a game does: it
writes next mode = 1 to `0x007795E8` (returned by the menu routine
`0x004359D0`) and 0 to the menu message-loop flag `0x0072DDD4`. Nothing
else is touched, and the dump is taken before client mode does anything
beyond the load. `--no-force` waits for a game to be started by hand
instead (Single Player, any character). An unforced dump and the forced
ones are byte-identical except pointers.

| Hook / address | Use |
|---|---|
| `0x006122F0` entry | table loader (stdcall: context, name, field list, `int *count`, record size); a one-shot INT3 on its return gives the record pointer (EAX) and the count |
| `0x00619300` entry | load-all routine; a one-shot INT3 on its return address (`0x0044B93C`) is the dump point, after every §7.4 fix-up |
| data-tables globals `0x0096BCAC`–`0x0096D62C` | pointer and count of each table (`GLOBALS` in the script; each checked against the hooked loader return) |
| combined arrays | weapons/armor/misc `0x0096CA60/68/70`, magic affixes `0x0096CA84/88/8C`, rare affixes `0x0096CAA8/AC`, plrtype/plrmode `0x0096D4E0/E4`, objtype/objmode `0x0096D4D0/D4`: the loader's own buffer is freed after the copy |
| freed tables | gamble, automap, treasureclassex: raw records freed after conversion; only their maps are dumped |
| runtime maps | `MAPS` in the script (`specs/data/runtime-maps.md`): itemtypes and montype equivalence matrices, per-class skill lists and the passive list (`skills_desc_list`), the itemstatcost description list, gamble index and level brackets, automap runtime form and level index, superunique hcIdx map, itemstatcost record-0 global, item lists and counts, states bitsets and lists, hireling, leveldefs, monseq and lvlsub indexes |

Output (`traces/raw/<time>-tables/`, gitignored): `<table>.bin` (records
only, no count header), `map-<name>.bin`, `manifest.json` (every loader
call with pointer, count, call site; per table address, count, record
size, source global; per map address and size; notes) and `manifest.tsv`
(the table and map list read by `data-tool dump-compare`).

## record_packets.py: client↔server messages

```
py tools/trace-recorder/record_packets.py --seconds 180      # Game.exe -w -ns; play by hand
py tools/trace-recorder/check_packets.py traces/raw/<time>-packets.jsonl
py tools/trace-recorder/check_packets.py traces/raw/<time>-packets.jsonl --perturb <seq>
py tools/trace-recorder/check_packets.py --selftest
```

Same reference-hash check, kill guarantees and Win32 code as
`record_rng.py` (imported). Hooks (INT3 at function entry unless noted;
addresses and rules in `specs/sim/intents-events.md`):

| Event | Address | Logged |
|---|---|---|
| `drain`, `flush` | `0x0052CFE0`, `0x0052FD90` | loop markers |
| `tick`, `tick_end` | `0x0052D870` entry; `0x0052FD1E`, `0x00564608` (after the call) | frame (game +0xA8 + 1), game, game type (+0x6A) |
| `c2s`, `c2s_sys` | `0x0053F3D0`, `0x0053F100` | client id, size, bytes (≤ 0x1FC) |
| `dispatch`, `result` | `0x0054D750`; `0x0053F45E` (after the call) | id, size, unit GUID, frame; result code (EAX) |
| `s2c` | `0x0053B280` | client id, size, bytes, caller |
| `net` | `0x0052B330` | type, client id, size, bytes, caller (`0x52e3b5` = flush) |
| `client_send`, `client_out` | `0x00478350`, `0x0052AE50` | the client's message before and after its duplicate filter |

Raw format `packets-raw-1` (JSON lines): header and footer as in
`rng-raw-1`; one record per event with `type`, the fields above, `frame`
(last tick's frame), `phase` (`input` after a drain, `tick`, `post`,
`flush`), `tid`, `seq`, `ms`.

## record_tick.py: the server tick and unit lists

```
py tools/trace-recorder/record_tick.py --seconds 200          # Game.exe -w -ns; play by hand
py tools/trace-recorder/check_tick.py traces/raw/<time>-tick.jsonl
py tools/trace-recorder/check_tick.py traces/raw/<time>-tick.jsonl --perturb-ex 500
py tools/trace-recorder/check_tick.py --selftest
py tools/trace-recorder/check_units.py traces/raw/<time>-tick.jsonl [--tables traces/raw/<time>-tables]
py tools/trace-recorder/check_units.py --selftest
```

Same reference-hash check, kill guarantees and Win32 code as
`record_rng.py`. 39 persistent INT3s (each stepped over and re-armed);
the expected bytes of every hook are checked before arming. Hooks and
offsets: the constants block at the top of the script, each naming its
spec section (`tick.md` §3, §5; `unit-order.md` §2, §4–§6; `units.md`
§4). Only the
first game that ticks is recorded; client-side calls of the shared room
code are dropped (server-unit flag, act membership). Options:
`--seconds`, `--ticks N`, `--snap-every N` (default 25; 0 = none),
`--out`.

Raw format `tick-raw-1` (JSON lines, key `k`): `header`, `game`, `tick`
(frame), `step` (name), `set` (timer, list `d`/`i`, class, type, final
and requested expire, unit type/GUID, args, callback), `cancel` (timer,
deferred), `ex` (timer run: class, list, type, expire, unit, args),
`hin`/`hout` (hash list), `rin`/`rout` (room list), `qin`/`qout`/`qclear`
(update queue), `ract`/`rdeact` (act room list), `snap` (every list, by
unit type and GUID; rooms by address with their adjacent-room arrays),
`footer`. The game runs near full speed under the recorder (4,902 ticks
in a 200 s run, start-up included).

Version 0.2.0 (same format name; both additions are optional, so
`check_tick.py` reads old and new files alike; `convert_tick.py` skips
`anim`, which a `sim/tick` trace does not hold):

- `set` gains `site` (address of the call to the public scheduling
  function: the return address two frames above the scheduler, minus 5;
  one more frame for an every-tick event made through the timed API),
  `cl` (unit class id) and `m` (unit mode, +0x10) at schedule time.
- New record `anim`, written when a mode animation schedule starts
  (`units.md` §4.2): after the frame-bonus call in `0x5539B0`
  (`0x5539CC`) and at the entries of `0x553B10`, `0x553C70`,
  `0x553DC0`. Fields: `fn` (scheduler), `f`, `ut`, `g`, `cl`, `m`, `seq`
  (sequence animation), `cur` (+0x44), `fc` and `sp` (frame count and
  speed: +0x34/+0x3C with a sequence, else +0x48/+0x4C), `b` (frame
  bonus, main form) or `arg` (variants), and from the AnimData record
  (+0x50): `ad` (name), `ad_frames`, `ad_speed`, `ev` (non-zero event
  bytes as `[index, value]`).

## record_stats.py: stat lists

```
py tools/trace-recorder/record_stats.py --seconds 240        # Game.exe -w -ns; play by hand
py tools/trace-recorder/check_stats.py traces/raw/<time>-stats.jsonl
py tools/trace-recorder/check_stats.py traces/raw/<time>-stats.jsonl --perturb-snap 3
py tools/trace-recorder/check_stats.py --selftest
py tools/trace-recorder/check_stats.py --files game
```

Imports `record_tick.py` unchanged and subclasses `TickRecorder`; keeps
only its tick and step hooks and adds 25 (addresses and registers: the
constants block of the script, each naming its section of
`specs/sim/stat-lists.md`). The expected bytes of every hook are checked
before arming (37 in all). Recording starts at the first server tick
(the tables are read then). A list is recorded only when it belongs to
the tree of a server unit's list; the first time a tree is touched it is
dumped whole before the operation runs. Options: `--seconds`, `--ticks`,
`--snap-every N` (default 25 frames; 0 = none), `--snap-monsters N`
(default 24), `--out`.

Raw format `stats-raw-1` (JSON lines, key `k`; lists by address `L`,
units as `"type:guid"`; stat keys are `(stat << 16) + layer`, signed):
`header` (format, tool, date, `Game.exe` SHA-256), `tick` and `step`
(as `tick-raw-1`), `stab` (runtime itemstatcost rows: flags, valshift,
minaccr, keepzero, op, param, base, op stats, +0x51/+0x52/+0x53, op
entries, op-base dependants; charstats per class [ManaRegen,
LifePerVitality, StaminaPerVitality, ManaPerMagic]; `life` group states),
`senv` (per act: environment time / period, after the tick's environment
step), `ssd` (seed: list dumps and unit info), `ssn` (snapshot, same
dumps), `sax` (extended list allocated), `ss`/`sa`/`sr` (set, add,
remove-all at entry, with the list's flags), `sat` (attach, after its own
detach), `sdt` (detach), `sfr` (free, after its own detach), `sdy`
(dynamic toggle), `sbt` (by-time refresh), `stg` (state toggle), `sxp`
(expiry start: the chain's flags and expire frames), `sxf` (a free the
expiry decides), `sxe` (expiry end), `scb`/`scx` (value-change callback
and its return; records between them happened inside it), `sgl`, `sgs`,
`sgm`, `sgx` (player life, stamina, mana and monster regeneration
entry), `footer`. A list dump: `L`, `ext`, `fl`, `st`, `ex`, `ot`, `og`,
`u`, `par`, `prev`, `next`, `b` (base `[stat, layer, value]`), and for
extended lists `last`, `setl`, `ow`, `F` (full), `m` (mod keys), `cb`,
`sb` (state bits).

## record_state.py: game-state snapshots

```
py tools/trace-recorder/record_state.py --auto ScnAma --seed 1234 --ticks 200
py tools/trace-recorder/record_state.py --snap-every 25 --seconds 240   # play by hand
py tools/trace-recorder/record_state.py --selftest                     # no game, any OS
```

Imports `record_tick.py` unchanged and subclasses `TickRecorder`; keeps
only its tick entry hook `0x0052D870` (the first game that ticks is the
one recorded) and adds the tick driver's return `0x0052FD1E` (ESI =
game, expected bytes `8B 76 18` as `record_packets.py`), the snapshot
point of `specs/tools/original-hooks.md` §3 rule 3: the state after
tick N, N = game +0xA8. Options: `--seconds` (default 120), `--ticks N`
(stop after N recorded ticks), `--snap-every N` (default 1: every frame;
the first recorded tick is always snapshotted), `--out`, `--game`, Game.exe
arguments, and the `autostart.py` options (`--auto CHAR --seed N --input
SCRIPT`).

Format `state-1` (JSON lines, key `k`): `header` (`side` `orig`, the
command line, `fields`, `gaps`, `Game.exe` SHA-256, game arguments),
one `snap` per snapshot frame (`f`, game `seed`, `units` sorted by
type and GUID), `footer` (`snaps`, notes: limits reached and reader
notes such as a unit list that is not extended). Fields, their 1.14d
reads and the comparison: `specs/tools/state-snapshot.md` §1–§2. Not
written: `own` (no 1.14d address in a spec; listed in `gaps`). Each
unit record and path is read once per snapshot (Wine-friendly), level
ids are cached per room.
## poke.py: set up state at tick N

```
py tools/trace-recorder/poke.py --poke-file traces/pokes/spawn-town.poke --auto ScnAma --seed 1234
py tools/trace-recorder/poke.py --auto ScnAma --poke "400 spawn 19 @x+3 @y+3 normal" --poke "400 time 2 0"
py tools/trace-recorder/poke.py --selftest                      # no game needed (Linux too)
# under Wine (cloud):
tools/cloud-game/run.sh --python --seconds 120 -- tools/trace-recorder/poke.py \
  --game "$D2_GAME_DIR/Game.exe" --poke-file traces/pokes/spawn-town.poke --auto ScnAma --seed 1234
```

The 1.14d side of `specs/tools/poke.md` (§4). At each stop on the tick
return `0x0052FD1E` (ESI = game, game +0xA8 = the frame just run) it runs
the directives due there: a poke-file line `at t` at the stop whose +0xA8
= F0 + t, a `--poke "F ..."` line at the stop whose +0xA8 = F − 1 (before
frame F runs). F0 is the frame of the tick in which client 0 first
reached state 4 (client list game +0x88, state +0x04;
`original-hooks.md` §1 rule 5), or `--start-frame F0`. Calls follow
`original-hooks-spawn.md` §5: one RWX scratch page with an INT3 return
trap at +0, the full thread context (FPU and SSE included) saved, the
arguments written above the return address, the registers of the
function's call form set (any of EAX, EBX, ECX, EDX, ESI, EDI; the others
keep the saved values), EIP = the entry; at the trap the next call starts
again from the saved context, and after the last one the saved context is
restored and the hook's byte is stepped over as for any INT3. Debug events
of other threads during a call go to the recorder's own handler.

| Directive | 1.14d (`CALL_FORMS` names) |
|---|---|
| `object` | `alloc` `0x00555230` (ECX 2, EDX class; x, y, game, room, 1, mode, 0) |
| `superunique` | `superunique` `0x005A49B0` (ECX game, EDX room; x, y, row) |
| `missile` | `missile` `0x0059FA30` (ECX game, EDX = a 0x5C-byte record in the scratch page: flags 0x21, owner, origin = owner, class, x, y, tx, ty, skill, level) |
| `spawn` | `scenario.md` §3.1 sequences. `normal`: `spawn` entry 1 `0x005B2F20` (mode 1, spread −1, flags 0); `random-boss`: `random_boss` `0x005A43E0` (no list, champion allowed, no warp check), then `champion_minions` `0x0054E1E0` (ESI boss, EDI game; cl 0, class); `champion`: `boss_spawn` `0x005A09E0` (EDI game, EBX class; room, cl 0, x, y, GUID −1, warp 0), `champion_mark` `0x005A48C0` with the umod, `champion_minions`; `unique`: `boss_spawn`, the umods appended to monster data (unit +0x14) +0x1C while fewer than 9, then `boss_minions` `0x005A2120` (ECX 3, EDX cl 0, EAX 6; game, boss, 1). Forms: `monsters/init.md` §25.1, §25.3 |
| `seed-game`, `seed-unit`, `time` | field writes: game +0xD0; unit +0x20/+0x24; environment record +0x00 / +0x08 of the player's act |
| `freeze` | the debugger sleeps at the stop |
| `pos` | `teleport` `0x00650BE0` (stack: unit path, unit, room, x, y; `ret 0x14`; 1/0), else `place` `0x00554EA0` (ECX game, EDX unit; room, x, y, exact 1, alt 0; `ret 0x14`; 1/0); room by entry 6 from the unit's room; only unit types 0, 1, 3. Forms: `path-placement.md` §6 r4, §10 |
| `warp` | `warp` `0x0053AEC0` (ECX game, EDX player; level, tile, default 0; `ret 8`; EAX not a status, result `ok`). Form: `waypoints.md` §7 r5 |
| `item` | index = the first combined items record whose code (+0x80) matches (array header `0x0096CA58`: count, records; 424-byte records; `items/treasure.md` §9.1, `data/loading.md` §6–§9; no `0x00633640` call); then `item_create` `0x00558D90` (ECX game, EDX request; use seed 0; `ret 4`; EAX the item) with a 0x84-byte request at scratch +0x200: unit 0, game, ilvl (default 1), item, mode 3, x, y, room (entry 6), init flags 1, format (game +0x78), quality (0 or 1–8), rest 0. Form: `items/generation.md` §3 |
| `stat` | `stat_set` `0x00627260` (stack: unit, stat, value, layer; `ret 0x10`); a unit with +0x5C = 0 → `failed`. Form: `stat-lists.md` §5 r2 |
| `state` | `state_set` `0x00639DB0` (stack: unit, state, 1/0; `ret 0xC`): toggle and update-queue insert. Form: `stat-lists.md` §9.2 |

The room for `object`, `superunique`, `spawn` and `item` comes from entry 6
`0x00463740` (ECX = the player's room, EDX = x; y); 0 gives `failed`.
References (`@player`, `@<type>[:<class>][#n]`, `<type>/<guid>`, `@x±N`,
`@y±N`) are resolved on the hash lists at the stop; `@wp` is a gap.

### Call forms (`CALL_FORMS`, `FIELDS`, `--forms`)

Every function a directive calls is one entry of `CALL_FORMS` in
`poke.py`: `Fn(address, argument names, result, cited spec, pc1-data
item, form)`. The argument names are what `poke.py` supplies (e.g.
`warp`: `game`, `player`, `level`, `tile`); the form says where each goes:

```python
Form(regs={"ecx": "game", "edx": "player"}, stack=["level", "tile"], ret=8, result="none")
```

`regs` maps EAX/EBX/ECX/EDX/ESI/EDI to an argument; `stack` lists the
stack arguments, first at [ESP+4]; either may hold a literal int the spec
names; each argument is placed exactly once; `ret` is the callee's `ret N`
(0 or 4 × stack count; the call does not need it); `result` is what EAX
means when the entry leaves it open (`unit`: created unit, 0 = failed;
`bool`: 0 = failed; `none`: ignored). `form=None` is a gap: every
directive calling that function returns `gap` naming the function, its
address and its pc1-data item. `FIELDS` holds the record offsets and table
addresses the directives read (item format, items array, umod list); None
there is a gap too.

Every entry has its form: `teleport`, `place` (`pos`), `warp`,
`item_create`, `stat_set`, `state_set` from `docs/handoff/pc1-data.md`
Step 4 item 22 (a)–(d) (read from the asm, not yet run on 1.14d); the
spawn kinds' functions (item 22 (e)) from `monsters/init.md` §25.1.

PC 1, for each answered function:

1. Write the form into the owning spec (e.g. `world/waypoints.md` §7 r5)
   and remove its gap row in `specs/tools/poke.md` §1.
2. Try it without editing `poke.py`: a `--forms` file,
   ```json
   {"format": "poke-forms-1",
    "forms": {"warp": {"regs": {"ecx": "game", "edx": "player"}, "stack": ["level", "tile"], "ret": 8}},
    "fields": {}}
   ```
   (`null` for a form sets it back to a gap; `fields` overrides `FIELDS`
   entries) and run
   `py tools/trace-recorder/poke.py --forms forms.json --auto ScnAma --seed 1234 --poke "400 warp 2"`;
   check the `poke` record (`r`, `eax`) and the game state. A form that
   names an unknown register or argument, or places one twice, stops the
   run before the game starts.
3. Copy the form into its `CALL_FORMS` entry (replace `None`, cite the
   spec section in `cite`), run `py tools/trace-recorder/poke.py --selftest`
   (it validates every form; the fake-process checks use their own forms,
   so they stay green), then run the directive on 1.14d once more without
   `--forms` and commit.

Options: `--seconds`, `--after N` (stop N frames after the last poke,
default 50), `--snap-every N` and `--tick-records` (keep `record_tick`
snapshots / all its hooks; default off), `--out`, the autostart options.

Raw format `poke-raw-1` (JSON lines, key `k`): `header` (format, tool,
date, `Game.exe` SHA-256, args), `poke_file` (path, SHA-256, start frame,
steps, the `--forms` file path and SHA-256, the forms and fields in
force), `game`, `tick` (as `tick-raw-1`), `poke_f0` (F0 and how it was
found), `poke` (one per directive: `f` = the frame the poke precedes,
`frame` = game +0xA8 at the stop, `t` for poke-file lines, `i` (index in
the stop), `d`, `r` (`ok`, `failed`, `unresolved`, `gap`), `guid` and
`eax` for a created unit, `via` for `pos` (`teleport` / `place`), `args`
as resolved, `note`, `src`), `footer`.
