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
