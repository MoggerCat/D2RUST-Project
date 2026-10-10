# Spec: Tools — RNG draw traces and their comparison (1.14d vs d2rs)

- **Status:** draft: the format, the owner rules, the d2rs log
  (`d2_sim::debug::rng_trace`, `rng-trace` feature; `d2-client
  state-dump --rng`), the 1.14d side (`record_rng.py --frames`,
  `rng_owners.py`) and the comparator (`rng_diff.py`) implement it;
  first real run `traces/checks/rng-town-arrival-ama.check`
  (Open questions 1); the fast 1.14d side (DRLG sites unhooked, emulated
  steps: §4 r6–r7, §6 r4) records the same compared draws 6× faster
  (Open questions 3).
- **Target version:** 1.14d (the original side); the format is
  `rng-raw-1`, the raw format `record_rng.py` already wrote, plus frames
  and owners.
- **Crate/module:** `d2-sim::debug::rng_trace` (log, owners, lines),
  `d2-sim::rng` (the record calls), `d2-sim::tick` (frame marker);
  `d2-client` `app::rng_dump`, `bridge::rng_trace`;
  `tools/trace-recorder/record_rng.py`, `x86emu.py`, `rng_owners.py`,
  `rng_diff.py`;
  `tools/scenario-diff/rng_channel.py` (the `rng` channel).
- **Related specs:** `sim/rng.md` (the generator, helpers, seed homes:
  game +0xD0 §5.2, unit +0x20 §5.3, DRLG §5.4), `sim/tick.md` §2–§3
  (frames, tick entry), `tools/state-snapshot.md` §2 (unit offsets),
  `tools/scenario-diff.md` (one-command run).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 46–55 |
| Inputs | 56–62 |
| Outputs / state changes | 63–67 |
| Rules | 68–69 |
|   1. Format `rng-raw-1` with frames and owners | 70–92 |
|   2. Owners | 93–132 |
|   3. The d2rs log | 133–152 |
|   4. The 1.14d recorder (`record_rng.py --frames`) | 153–189 |
|   5. Comparison (`rng_diff.py`) | 190–219 |
|   6. The `rng` channel of `scenario-diff` | 220–241 |
| Constants & data dependencies | 242–245 |
| Randomness | 246–249 |
| Edge cases & original bugs | 250–259 |
| Test vectors | 260–269 |
| Provenance | 270–274 |
| Open questions | 275–311 |
<!-- /index -->

## Summary

Both games log every seeded draw: op and arguments, the seed before and
after, the returned value, the draw site, the frame and the seed's
owner (the game seed, a server unit's seed, or another seed). The
comparator aligns the two logs by frame and, within a frame, compares
each owner's draws in order, so a draw-order slip shows at the draw it
happens (frame, owner, index, field) instead of as a state difference
ticks later.

## Inputs

| Name | Type | Source |
|---|---|---|
| 1.14d recording | `rng-raw-1` with frames | `record_rng.py --frames --ticks T --auto C --seed N` |
| d2rs recording | `rng-raw-1` with frames and owners | `d2-client state-dump --save --seed --ticks T --out S --rng FILE` (built with `--features rng-trace`) |

## Outputs / state changes

The two recordings (gitignored, `traces/raw/`); the comparator's report
and exit code (§5). Neither game changes (§3 rule 2).

## Rules

### 1. Format `rng-raw-1` with frames and owners

1. JSON lines. First line `{"type": "header", "format": "rng-raw-1",
   ...}`: 1.14d `tool`, `date`, `game_exe_sha256`, `args`, `side`
   `orig`, `frames` true, `max_ticks`, `skip_inline`; d2rs `side`
   `d2rs`, `tool`, `date`, `command`, `seed`, `frames`, `owners`. Last
   line `{"type": "footer", "events", "notes", ...}`.
2. A draw: `{"type": "draw", "via", "op", "n"?, "min"?, "site", "seed"?,
   "before": [lo, hi], "after": [lo, hi], "ret", "frame", "owner", "seq",
   "tid"}`. `op` is a `sim/rng.md` §3 name; `n`, `min` are the
   arguments as u32. `via`: 1.14d `helper` or `inline` (an inlined step:
   op `step`, `ret` = new `lo`); d2rs always `helper` (every d2rs draw is
   a call of a `Seed` method; `derive` is a `step`). `site`: 1.14d the
   call site or the inline `mov` address; d2rs the caller's `file:line`.
   `seed`: 1.14d the seed's address (helpers and setters); d2rs the owner
   label, or `d2rs#k` (k-th distinct unknown seed address). `frame`: the
   frame of the last tick started (§4 rule 2), 0 before the first tick.
   `owner`: `game`, `unit T:G`, or a value starting `other`.
3. 1.14d also writes `seed_set` records (setters, `sim/rng.md` §4) and
   `tick` records `{"type": "tick", "f", "game", "gseed": [lo, hi],
   "units": [[type, guid, lo, hi], ...]}` (§4 rule 3); d2rs writes
   neither (a d2rs seed set is a value store, not a call).

### 2. Owners

1. The game seed (game +0xD0, `sim/rng.md` §5.2) is `game`; a server
   unit's seed (unit +0x20, §5.3) is `unit T:G` (type, GUID); every other
   seed (item data, DRLG, rooms, regions, client) is `other`.
2. Forward: starting from the owners' values at the last point both
   values are known (1.14d: the tick record; d2rs: the previous drain), a
   draw whose seed-before equals an owner's current value is that
   owner's, and its seed-after becomes the owner's value.
3. Backward: from the owners' values at the next known point, a draw not
   yet owned whose seed-after equals an owner's value is that owner's,
   and its seed-before becomes the value; an owner whose forward chain
   already reached that value takes no more draws backward.
4. A draw that does not step (`roll`, `roll_range` with n < 1) links
   nothing; it keeps an owner only from its address (1.14d helpers;
   d2rs a match of address and value).
5. Copies: a seed initialised from the same value replays the owner's
   states (with `-seed N` the DRLG seed starts at `{N, 666}`, the game
   seed's own start, `sim/rng.md` §5.2, §5.4). So the game owner needs
   the game seed's address: d2rs takes a draw for `game` only at the
   game seed's address (it never moves); 1.14d takes a helper or setter
   at game +0xD0, and never an inline draw in the DRLG code
   (`0x642000`–`0x643000`, `0x66B000`–`0x682000` without `0x67A240`–`0x67A390`, the
   charged-bolt path compute `FUN_0067a240`, whose inline draws step a missile's unit
   seed (REC-3120); the `sim/rng.md` §7 DRLG rows), which is `other:drlg`.
6. A 1.14d helper or setter whose seed address minus 0x20 reads as a
   server unit (type ≤ 5, unit +0xC8 bit 0x04000000) at the time of the
   draw names that unit, if a tick record lists the unit. An owner not
   found by rules 2–7 is `other:<address>` (helper, setter),
   `other:inline` or `other:drlg`.
7. A forward chain that starts at a rule 6 draw of an owner with no known
   value yet (a unit created during the frame: its seed is set and
   stepped before the next tick record lists it) is not anchored: rule 3's
   "already reached" test does not hold for it, so the backward pass
   takes the draws before that one. Case: a new monster's seed is set
   (`0x00552E50`, its hint read before the GUID is written: `1:0`) and
   stepped once inline (`0x00573A8E`) before its first helper draw
   (`0x00573F8F`); measured `combat-pop-blood-moor` frame 2, units 1:3,
   1:4, 1:6, 1:7 (2026-10-09).

### 3. The d2rs log

1. `d2-sim` feature `rng-trace`, off by default and in every normal
   build. With it every `Seed` draw (`step`, `roll`, `mask`,
   `mask_range`, `roll_range`, `derive`) appends one entry to a
   per-thread log, the draw site through `#[track_caller]`, and the tick
   marks the frame after `frame += 1`; the log records only after
   `rng_trace::start()` on that thread, and only the debug export drains
   it. `d2-client` feature `rng-trace` turns the `d2-sim` one on; without
   it `state-dump --rng` is an error.
2. The log copies values the draw computed: no draw, value, state or
   order depends on the feature or on the log being on (CLAUDE.md rule
   6). The dump's game and its state lines are the same with or without
   `--rng`.
3. `state-dump --rng FILE` starts the log on the server thread before
   the game is built (game creation is frame 0), drains it after every
   bridge frame with the game and unit seeds read at that moment (through
   the bridge, `bridge::rng_trace`), assigns frames and owners (§2) and
   writes the lines.

### 4. The 1.14d recorder (`record_rng.py --frames`)

1. Hooks the tick entry `0x0052D870` (ECX = game; `sim/tick.md` §3) as
   `record_tick.py` does; only the first game that ticks is recorded.
2. Frame = game +0xA8 + 1 read at the tick entry; every record after it
   carries it (0 before the first tick).
3. At each tick entry writes a `tick` record: the game seed (game
   +0xD0) and every unit of the five server hash lists (game +0x1120,
   128 buckets per type, next unit +0xE4) with its seed (+0x20).
4. Each helper and setter record gets its owner hint (§2 rule 6) at the
   time it is written; after the run `rng_owners.py` assigns every
   owner (§2) and rewrites the file.
5. `--ticks N` ends the recording at the entry of tick N + 1 (that
   tick's draws are not written).
6. `--skip-inline LO-HI,...` (preset `drlg`: the §2 rule 5 ranges) does
   not hook the inline sites in those ranges: faster, those draws are
   missing; the header lists them. Ranges inside the §2 rule 5 DRLG
   ranges lose nothing the comparison uses: every inline draw there is
   `other:drlg` and takes no part in the owner chains (§2 r5), so §5
   compares the same draws with the same owners; any other skipped range
   makes the comparison at best partial (§5 r5).
7. `--emulate on` (default): at a breakpoint the instructions the
   single steps would run are run by `x86emu.py` on the stopped thread's
   registers and memory instead: an inline site's trace from the `mov`
   to the add/adc pair (the same rules pick the `mul` and the pair), and
   the one instruction under a helper, setter, tick or return breakpoint.
   The result (registers, eip, status flags, buffered memory writes) is
   committed only when every instruction was decoded (32-bit integer
   moves, ALU ops, `mul`, `jcc`, `jmp`, `push`, `pop`, `lea`, `movzx`; no
   prefix, call or return), no other breakpoint lies on the path, the
   §4 step limits hold and every status flag is defined; otherwise the
   thread is single-stepped as before. Return breakpoints stay armed
   after their call returns (a hit without a pending call writes
   nothing). `--emulate check` single-steps everything and compares
   each emulated result with the real one (counts `emu_check:*_ok` /
   `_diff` in the footer); `--emulate off` is the single-step recorder.

### 5. Comparison (`rng_diff.py`)

1. Both headers `rng-raw-1`; every draw needs a `frame`; a 1.14d file
   without owners gets them from its tick records (§4 rule 4).
2. Compared: draws of owners `game` and `unit T:G` (`--owners`), frames
   from 0 to the last frame both files reach; that last frame is compared
   as a prefix per owner (the 1.14d run stops at the next tick's entry).
   Draws without a state, draws that do not step and `other` draws are
   counted, not compared.
3. Per frame ascending, per owner (`game` first, then units by type and
   GUID), position by position: `before`, `after`; when the 1.14d draw
   went through a helper, also `op`, `n`, `min`, `ret` (`--state-only`
   skips them). Against a `roll(n)` with n ≥ 1 on the other side
   (`sim/rng.md` §3: `roll(n)` = `lo' mod n`), two other forms are the
   same draw: `roll_range(min, n)` (1.14d may add `min` inline after the
   helper, as at `0x00573F8F`), compared as `roll(n)` with `ret − min`;
   and a d2rs `step` whose `lo'` the caller takes mod n (the AI chance
   `0x005F05A6` = `roll(100)`), compared as `roll(n)` with `ret mod n`. A position one side lacks is `missing` (1.14d has it) or
   `extra`.
4. Report: the first divergence (frame, owner, index, field, both
   records with their sites), the next `--next N`, the game seed's draws
   across all frames in order (first differing position, and the shift
   k, |k| ≤ 4, for which d2rs draw i has the state of 1.14d draw i + k
   longest), per owner the draw counts, the positions differing and the
   first divergence, and the counts not compared.
5. Verdict: `DIVERGED` (exit 1) on any divergence; `PARTIAL` (2) when
   nothing diverged but inline sites outside the DRLG ranges were
   skipped (§4 r6; DRLG-only skips are a note), inline draws lack a
   state or no frame was compared; else `MATCH` (0); 3 on an error.

### 6. The `rng` channel of `scenario-diff`

1. 1.14d: `record_rng.py --frames` with the check's `--auto`, `--seed`,
   `--ticks`, `--seconds` (and `input orig`); d2rs: `cargo run --release
   -p d2-client --features rng-trace -- state-dump ... --rng
   d2rs.rng.jsonl`; then `rng_diff.py`.
2. A check with an `input orig` / `input d2rs` line runs that input on
   1.14d only: the verdict is at best partial.
3. `at ... poke`, `at ... send` and a shared frame-anchored `input` run
   on both sides: `record_rng.py --frames` stops at the tick return
   `0x0052FD1E` (pokes, the send layer's frame, `frame F` input steps)
   and, with sends, at the drain call `0x0044F136` (`--send`,
   `tools/original-hooks.md` §1 rule 4), as `record_packets.py`; d2rs
   `state-dump` takes the same `--poke` / `--send` / `--input`. Measured
   2026-10-09 under Wine: `items-vendor-akara-buy` (4 pokes ok, 4 sends
   EAX 1) and `walk-town-ama` (`frame 10` click posted at the stop of
   frame 9) record and compare (first divergence unchanged: frame 2,
   unit 1:1, draw #12).
4. The channel records with `--skip-inline drlg` (§4 r6: nothing
   compared is lost, the verdict is not downgraded) and the default
   `--emulate on` (§4 r7).

## Constants & data dependencies

Offsets only (§2, §4, from the specs named there); no table is read.

## Randomness

None. Both logs record draws the games make; neither draws.

## Edge cases & original bugs

1. 1.14d inline draws have no seed address: their owner comes from the
   value chains only, so an inline draw on a unit that is created and
   removed between two ticks, with its seed written inline, stays
   `other:inline`.
2. Two owners with equal seed values tie; the d2rs side prefers the
   owner at the draw's address, the 1.14d side the first owner in label
   order.

## Test vectors

| Input | Expected |
|---|---|
| Fight fixture, 40 ticks, log off vs on | identical game; draws and 40 frame marks logged (`rng_trace::tests`) |
| `rng_diff.py --selftest` | a synthetic pair matches; each perturbation (before, after, ret, op, n, owner, a dropped, added or moved draw) is reported first at its place; a one-step lag of the game seed gives shift +1; other seeds never compared; skipped inline ranges give PARTIAL; no frames is an error |
| `x86emu.py --selftest` | one 64-bit step (`mov`, `mul`, `add`, `adc`) exact; jcc taken / not taken; add, adc, cmp flags against their definitions; memory, SIB, byte and stack operands; `mul`'s undefined flags refuse a jcc and an early commit; prefixes, call, ret, div refused |
| `record_rng.py --emulate check` on `rng-town-arrival-ama` (Wine, 2026-10-09, all 846 sites) | 13,081 inline traces and 5,168 single instructions emulated, every one equal to the single-stepped registers, eip, flags and memory (`emu_check:*_diff` 0); 38 fell back (opcodes `66`, `C1`, `F7 /2`) |
| `rng_owners.py --selftest` | game, unit (by address, forward, backward), other; an untrusted unit hint; a DRLG copy and a late copy of the game seed stay other |

## Provenance

d2rs-own tool. The 1.14d hooks and offsets restate `sim/rng.md`,
`sim/tick.md`, `sim/unit-order.md` and `tools/state-snapshot.md`.

## Open questions

1. First run (2026-10-09, under Wine, `rng-town-arrival-ama.check`:
   ScnAma, seed 1234, 40 ticks; 1.14d side 73 s, 16,142 records,
   1,418 debug events/s): frame 0 (game creation) matches: the game
   seed's four steps (monster region `0x00547D38`, objects `0x00546CB9`,
   NPC control `0x005360D8`, quests `0x00545F27`; d2rs `region.rs`,
   `objects.rs`, `npc.rs`, `quests.rs`). First divergence: frame 1, game
   seed, draw #0, missing in d2rs: 1.14d steps the game seed at
   `0x00552E31` (the unit allocation `0x00552DF0`, `sim/rng.md` §5.3)
   when the player is allocated; d2rs `units/lifecycle.rs` skips the
   step for players. From then on every game draw is one step behind
   (same states, one frame later: game draw #4 is frame 1 on 1.14d,
   frame 2 on d2rs), so every unit seed made from the game seed in frame
   2 (7 monsters, 9 objects) differs. Owner of the fix:
   `sim/units.md` §3.1 / `sim/rng.md` §5.3 (player unit seed).
2. 1.14d inline draws have no seed address (edge case 1); recording the
   address of the store that writes the new state back would remove the
   value-chain step for them.
3. Speed (measured 2026-10-09, Wine 9.0, `rng-town-arrival-ama`: ScnAma,
   seed 1234, 40 ticks, `D2_AUTO_AFTER=0`). Single-step recorder (all
   846 sites, `--emulate off`): 70.5 s wall, the level at 63.2 s,
   16,142 records, 103,419 debug events (about 1,500/s; ~7 per inline
   draw, 11,074 of the 13,079 inline draws in the DRLG code). DRLG sites
   unhooked only: 21.5 s, 24,697 events. Emulation only (all sites):
   22.2 s (18,393 events). Both (the channel's default): 11.1 s wall (8.8 s recording,
   the level at 7.2 s), 5,068 records, 7,289 debug events. Proof:
   `rng_diff.py` between the single-step and the fast recording: MATCH
   (either file as orig); every `game` and `unit` owner's draw sequence
   (op, site, n, min, before, after, ret, frame) and all 40 tick records
   are identical; besides the 11,074 DRLG draws only the client weather
   draws differ (`0x00473090`, ±10: drawn-frame dependent, `sim/rng.md`
   §7, `other`). Game creation is not a lever: the menu steps no seed
   (`sim/rng.md` §5.5) and is left at 0.6 s. Left: one breakpoint event
   per helper return (2,129) and per helper or setter entry; emulating
   the helper bodies to their return would remove the return events.
