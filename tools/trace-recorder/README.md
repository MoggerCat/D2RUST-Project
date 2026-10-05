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
