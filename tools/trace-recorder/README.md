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
| `record_packets.py` | Launches `game/Game.exe` under the debugger, logs every client→server message at the server's queue read and dispatch (with result), every server→client message as queued and every flushed buffer, with tick markers and the frame number; writes `traces/raw/<time>-packets.jsonl` (gitignored). Spec: `specs/sim/intents-events.md` |
| `path_tables.py` | Checks `specs/sim/path-tables.tsv` against the reference `Game.exe` file image (no process); `--write` regenerates it, `--perturb N` must report row N, `--selftest` perturbs every row. Spec: `specs/sim/pathing.md` |
| `check_packets.py` | Checks a packets recording against `specs/sim/intents-events.md` and its two TSVs (rules R1–R7); `--perturb N` must report seq N; `--selftest` runs a synthetic trace and every single-byte perturbation |
| `record_tick.py` | Launches `game/Game.exe` under the debugger, logs each server tick and its step markers, every timer event scheduled, cancelled and run, every unit/room/update-queue list change, and list snapshots every 25 frames; writes `traces/raw/<time>-tick.jsonl` (gitignored). Specs: `specs/sim/tick.md`, `specs/sim/unit-order.md` |
| `check_tick.py` | Replays a tick recording through a model of those specs: must predict every timer run and reproduce every snapshot; `--perturb-ex N`, `--perturb-snap N` must fail at the changed record; `--selftest` runs a hand-built recording of the specs' test vectors |
| `record_stats.py` | Subclass of `record_tick.py`'s `TickRecorder` (tick and step hooks only): logs every base write, attach, detach, free, dynamic toggle, by-time refresh, state toggle, expiry and value-change callback on server stat lists, the regeneration entry points, and snapshots of the players' and monsters' list trees; writes `traces/raw/<time>-stats.jsonl` (gitignored). Specs: `specs/sim/stats.md`, `specs/sim/stat-lists.md` |
| `check_stats.py` | Replays a stats recording through a model of those specs: must predict every callback, expiry and regeneration value and reproduce every snapshot; `--perturb-snap N`, `--perturb-cb N` must fail at the changed record; `--selftest` runs a hand-built recording of the specs' test vectors; `--files game` checks the specs' itemstatcost facts on the 1.14d table |
| `check_units.py` | Checks the per-kind timer-event rules U1–U11 of `specs/sim/units.md` on a tick recording (tables from a `dump_tables.py` directory); `--perturb N` must fail at the changed record; `--selftest` runs a hand-built recording |
| `dump_tables.py` | Launches `game/Game.exe` under the debugger, stops when the excel load and its fix-ups have finished, writes every loaded table and the runtime maps it knows to `traces/raw/<time>-tables/` (gitignored); compared by `data-tool dump-compare` |

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
`check_tick.py` reads old and new files alike):

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
