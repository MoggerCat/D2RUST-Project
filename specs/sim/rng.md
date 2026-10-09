# Spec: Simulation — Seeded RNG

- **Status:** conformance-passing: `tools/trace-recorder/check_rng.py`
  recomputes every draw recorded from the running 1.14d game (32,543
  draws, 1,226 seed writes in `traces/raw/20261005-232125-rng.jsonl`,
  plus three earlier runs) with no mismatch and no unexplained seed
  state; `d2-sim::rng` replays all committed traces exactly
  (`cargo test -p conformance`, `tests/rng_traces.rs`).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::rng` (`crates/d2-sim/src/rng.rs`); trace
  replay in `conformance::rng`
- **Related specs:** `traces/FORMAT.md` (trace files, `rng_draw` events);
  per-system specs (items, DRLG, monsters, combat) own the draw *order*
  and what each draw decides; this spec owns the generator, the helper
  math and the seed sources.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 39–51 |
| Inputs | 52–59 |
| Outputs / state changes | 60–65 |
| Rules | 66–67 |
|   1. Seed state | 68–74 |
|   2. Step | 75–96 |
|   3. Draw helpers | 97–126 |
|   4. Setting and reading seeds | 127–143 |
|   5. Where seeds come from | 144–218 |
|   6. Inlined draws | 219–239 |
|   7. Which systems draw from which seed | 240–270 |
| Constants & data dependencies | 271–281 |
| Randomness | 282–286 |
| Edge cases & original bugs | 287–299 |
| Test vectors | 300–348 |
| Provenance | 349–386 |
| Open questions | 387–422 |
<!-- /index -->

## Summary

All game randomness in 1.14d that must be reproduced comes from one
generator: a 64-bit state split into two u32 words, advanced by a
multiply-with-carry step. Each game object that needs randomness owns its
own state ("seed"): the game, every unit, every item, the DRLG, every
level and room, the monster-region system, plus a few client-side
globals. Seeds are initialized from a value with the high word set to 666,
and child seeds are derived by stepping a parent seed. The step is inlined
at 846 places in `Game.exe`; six small helper functions wrap it with
range math. Start-up, the title screen, the main menu and the character
screens draw nothing; the first seed is written when a game is created.

## Inputs

| Name | Type | Source |
|---|---|---|
| seed state | `{lo: u32, hi: u32}` | owned by the object that draws (§5) |
| range arguments | `n: i32`, `min: i32` | the caller |
| time value | `time() + GetTickCount() + caller value` | only for the root game seed (§5.1) |

## Outputs / state changes

- A draw replaces the seed state with the stepped state and returns a
  u32 derived from the new low word (§3).
- Seed setters overwrite the whole state (§4).

## Rules

### 1. Seed state

1. A seed is 8 bytes: `lo` (u32, offset 0) then `hi` (u32, offset 4).
   Read as one little-endian u64, `lo` is the low half.
2. The state carries no other fields. Copying the 8 bytes copies the
   generator.

### 2. Step

With `K = 0x6AC690C5` (1,791,398,085):

```
v   = lo × K + hi          (u64; lo, hi, K zero-extended)
lo' = v mod 2^32
hi' = v >> 32
```

The state becomes `{lo', hi'}`. A step's raw output is `lo'`.

- `v` never overflows 64 bits: the maximum is (2^32−1)·K + 2^32−1 <
  2^64.
- `hi` is the carry of a multiply-with-carry generator: `hi' < K` always
  (proved by the bound above: `hi' ≤ K`; `hi' = K` only for
  lo = hi = 2^32−1).
- Given two consecutive low words `lo` and `lo'` of one seed, the old
  `hi = (lo' − lo·K) mod 2^32` (the recorder uses this, §Provenance).
- `{0, 0}` is a fixed point (steps to `{0, 0}` forever). No game path
  writes it: every setter writes hi = 666 or copies a live state.

### 3. Draw helpers

Every draw is one step followed by math on `lo'`. Six non-inlined helpers
exist; inlined draws (§6) use the same math. Arguments are signed i32
where the table says so.

| Name (d2rs) | 1.14d | Arguments | Result | Steps |
|---|---|---|---|---|
| `step` | `0x0045C370` | — | `lo'` (the helper returns the whole u64 `{lo', hi'}`) | 1 |
| `roll(n)` | `0x0045C390`, `0x0045C3E0` (two identical copies) | `n` (i32) | `n < 1`: 0. Else if `n & (n−1) == 0`: `lo' & (n−1)`. Else `lo' mod n` (unsigned 32-bit) | 0 if `n < 1`, else 1 |
| `mask(n)` | `0x00472210` | `n` (u32) | `lo' & (n−1)` (no range check; `n = 0` gives `lo'`) | 1 |
| `mask_range(min, n)` | `0x00472240` (no callers in 1.14d) | `min`, `n` | `(lo' & (n−1)) + min` (mod 2^32) | 1 |
| `roll_range(min, n)` | `0x00472280` | `min` (i32), `n` (i32) | `n < 1`: `min`. Else `roll(n) + min` (mod 2^32) | 0 if `n < 1`, else 1 |

Details:

1. `n < 1` is a signed test: `n = 0x80000000` or any negative value makes
   `roll` return 0 (and `roll_range` return `min`) **without stepping**.
   The seed is untouched; draw counts depend on this.
2. `n = 1` steps and returns 0 (the power-of-two branch).
3. The power-of-two branch and the modulo branch give the same value for
   powers of two; the split matters only for speed, not results. An
   implementation may use `lo' % n` for every `n ≥ 1`.
4. The modulo is unsigned on the u32 `lo'` (`div`, not `idiv`), never a
   64-bit modulo of the whole state.
5. `roll` results lie in `[0, n)`; `roll_range` results in
   `[min, min+n)`.
6. Callers pass `(seed address, n)` in ECX/EDX (fastcall); `min, n`
   helpers take `n` on the stack.

### 4. Setting and reading seeds

| Name (d2rs) | 1.14d | Effect |
|---|---|---|
| `init()` | `0x00650E30` | state = `{1, 666}` |
| `init_low(x)` | `0x00650E40` | state = `{x, 666}` |
| `set(lo, hi)` | `0x00650E60` | state = `{lo, hi}` |
| `get(&lo, &hi)` | `0x00650E70` | reads both words |
| `get_lo()` / `get_hi()` | `0x00650E50` / `0x00650EA0` | reads one word |

1. 666 = 0x29A is the fixed high word of every freshly initialized seed.
2. Callers usually call `init()` and then `init_low(x)`; the `init()` is
   redundant (overwritten) but harmless.
3. Some objects write `{x, 666}` inline instead of calling `init_low`
   (search for stores of `0x29A`): `0x00428C60`, `0x004294C0`,
   `0x004456D0`, `0x004A2CB0` (client). Same effect.

### 5. Where seeds come from

"Derived from P" means: one step of seed P, then `init_low(lo')` on the
child, so the child starts at `{lo', 666}`. Offsets are 1.14d; D2MOO
names in brackets.

#### 5.1 Time value (the only non-deterministic input)

`time_value(v)` (`0x00650DE0`): `x = time() + GetTickCount() + v` (u32
wrap); apply `x = x·0x19660D + 0x3C6EF35F` (mod 2^32) three times; return
`x & 0x7FFFFFFF`. Used for the root game seed (§5.2), the unit-seed
fallback (§5.3) and two client particle globals (§5.5). d2rs takes this
value as an input (determinism rule), never from the clock.

#### 5.2 Game

<!-- rows -->
| Seed | Where | Initial value |
|---|---|---|
| game seed, game +0xD0 [`pGameSeed`] | `0x0052C280` at game creation | `init()`, then `init_low(time_value(QueryPerformanceCounter low word))`. A fixed-seed global `0x00731004` (≠ −1) would replace this; its writer `0x0052C320` is called by the `-seed N` switch handler `0x0044D860` (N ≠ 0) and by `0x00451909`; that branch (game +0x84 := 1, +0x7C := N, seed `{N, 666}` unstepped) is `tools/original-hooks.md` §2 rule 3. |
| `dwInitSeed`, game +0x7C | `0x0052C280` | one game-seed step, then `time_value(lo')`. Single player's DRLG does not use it; it uses the character's map seed (§5.4, observed). |

Game creation (`0x00530930`; `0x00530BF0` with a client) then derives,
in this order, each from one game-seed step: monster-region seed and
`dwMonSeed` (+0xEC, `0x00547D20`), object-control seed and `dwObjSeed`
(+0x80, `0x00546C60`), NPC-control seed (`0x00536070`), quest seed
(`0x00545D80`). After that every unit allocation takes one game-seed step
and every item a second one (§5.3); the first step after creation is the
joining player's unit seed, drawn by its load (`sim/units.md` §3.1 r4.1,
recorded 2026-10-09: `-seed 1234` gives 2972047412, 1542758918,
1961566614, 2016663226 for the four creation steps and 4048349444 for
the player). Recorded (sim-0002): game-seed steps
at `0x0052C2C6`, then in `0x00547D20`, `0x00546C60`, `0x00536070`,
`0x00545D80`, then `0x00552DF0` / `0x00552E90` per unit / item.

#### 5.3 Units, items, missiles

<!-- rows -->
| Seed | Where | Initial value |
|---|---|---|
| server unit seed, unit +0x20; `dwInitSeed` +0x28 [`SUNIT_InitSeed`] | `0x00552DF0`, at every unit allocation (`0x00555230`) but a player's, and the player-load / corpse paths (`sim/units.md` §3.1 r4.1) | derived from the game seed; `dwInitSeed = lo'`. Without a parent seed: v := `time_value(2·c)` with c the counter `0x008846E8` before its increment (`0x00552E5C`–`0x00552E78`; c starts at 0: `.bss`, no other writer), `dwInitSeed` := v, seed := `init_low(v)`. |
| item seed, item data +0x04 (data pointer at unit +0x14); start seed at data +0x10 [`ITEMS_InitItemSeed`] | `0x00552E90` | reset to `{1, 666}` (`0x00627DC0`), then derived from the game seed |
| item seed re-init | `0x005572A0`–`0x00557450` (quality downgrade chain, after a failed quality routine) | `init_low(s)`, s = the item seed's low word saved by the dispatch before the failed routine; s is also written as the new start seed (`items/quality.md` §5) |
| item seed forced | `0x00558D90` | the drop request's seed values (+0x48 / +0x4C) |
| item seed from a save | `0x0062CBE0` (`0x0062A970` sets 0) | 32 bits v from the item bit stream, read only when the reader's save flag is set (`0x0062E430` argument 4, the same flag as the `JM` test): unit `dwInitSeed` +0x28 := v and the **unit** seed +0x20 / +0x24 := `init_low(v)` = {v, 666} (`0x0062CD06`–`0x0062CD16`; whole seed, not the low word only). The reader does not write the item data seed +0x04 or the start seed +0x10 (their only writers are `0x00552E90` at allocation, `0x00627E20` callers `0x00552EB6`, `0x005572B2`, `0x0055732E`, `0x00557394`, `0x005573FE`, `0x00558E69`); a loaded item keeps the values its allocation gave them. The load path allocates as any item: `0x00558CB0` calls `0x00555230` with type 4 (`0x00558CFC` `mov ecx, 4`) before the reader (`0x00558D23`), and `0x00555230` runs `0x00552E90` for type 4 (`0x0055532A`), so a loaded item's +0x04 / +0x10 come from that allocation's game-seed step. |
| client unit seeds | `0x00465FD0`, `0x00466200`, `0x00466360` | derived from the client room seed (+0x6C); stays `{1, 666}` without a room; client missiles: `missiles/client.md` §C14 |
| missile seeds | `0x005A9820`, `0x005ACDF0`, `0x005AFB80`, `0x005B04A0`, `0x005B0640`, `0x005C9290`, `0x005CD110`, `0x005D40F0`, `0x005D4680` | `init_low` of geometry (unit x, target x, first path point x, plus frames left; per missile function); pierce test `0x0059F940` uses a pierce stat |

#### 5.4 DRLG

<!-- rows -->
| Seed | Where | Initial value |
|---|---|---|
| DRLG seed, drlg +0x00 | `0x00642DA0` | `init_low(map seed)`. Observed: the character's saved map ID (`.d2s` offset 0xAB; 644409375 for the test character), identical across runs. |
| `dwStartSeed`, drlg +0x470 | `0x00642DA0` | `lo'` of the first DRLG-seed step (observed 4014346869, sim-0003). Later steps choose act layouts (act 2: two `lo' % 7` per try until they differ; act 3: `lo' & 1`). |
| level seed, level +0x1C4 | `0x00642AE0` (alloc), `0x006424A0` (init) | `init_low(dwStartSeed + level id)`; observed 4014346870/71/72 for level ids 1/2/3 |
| DRLG room seed, room +0x14; `dwInitSeed` +0x04 | `0x0066B3E0` (reset `0x0066EE40`) | derived from the level seed; then `dwInitSeed` = a step of the room seed |
| active room seed, room +0x6C | `0x00619890` (from `0x006422A0`) | derived from the DRLG room seed |

The client builds its own DRLG copy from the same values: each level's
sequence was recorded twice per run, identical.

#### 5.5 Client-only globals

<!-- rows -->
| Seed | Where | Initial value | Drawn by |
|---|---|---|---|
| automap seed, global `0x0096C8C8` | `0x0061FCF0` at table load | `{0, 666}` | automap cell picker `0x0061FFF0` (`roll`), sim-0001 |
| particle globals `0x00712C4C` / `0x00712C50` | `0x00476290` / `0x00476460` | `time_value(...)` | particle colours and sizes; only in level ids 0x4A and 0x78 |

Start-up, title, main menu and character screens step no seed (recorded:
0 draws until game creation; the static call graph agrees). Menus and
Battle.net code use the C runtime `rand()` (separate LCG, `0x00687461`),
which never feeds game outcomes and is out of scope.

### 6. Inlined draws

The 846 inline sites compute the same step; what they do with `lo'`
belongs to each system's spec. Forms found, all on the **unsigned low
word** (no 64-bit modulo, no signed modulo, the high word never used as
a value):

<!-- rows -->
| Form | Approx. sites | Examples |
|---|---|---|
| `lo' % 100` (percent check) | 323 | `0x005E0490`, `0x005516C0`, `0x0064A850` |
| inlined `roll(n)` (§3 semantics, both branches) | ~250 | `0x0055A6D0`, `0x005D54B0` |
| `lo' % c`, constant c (3, 5, 7, 12, 20, 1000, 100000, …) | ~110 | `0x00642DA0` (% 7), `0x0054EC90` |
| `lo' & mask` (1, 3, 7, 0x3F, 0xFF, 0x3FF, 0x7FFF) | ~145 | `0x00672810`, `0x0055A6D0` |
| reduction plus signed offset, e.g. `lo' % 20 − 10 + base` | ~20 | `0x004D1BD0`, `0x00476290` |
| `lo'` used whole as a child seed | ~25 | §5 |
| step with the value discarded (advance only) | ~25 | `0x00472400`, `0x005DF7D0` |
| peek: save state, step, restore | 1 | `0x005D5220` (the only caller of the `step` helper) |

Offset forms are computed in 32-bit wrapping arithmetic and read as i32.

### 7. Which systems draw from which seed

<!-- rows -->
| System | Seed | 1.14d sites |
|---|---|---|
| treasure-class picks, drop quality | dropping unit's seed | `0x0055A6D0`, `0x00558640` |
| item base stats (gold, quantity, durability, defense, variable values) and low-quality durability | the item's **unit** seed (unit +0x20) | `0x00557AB0` (`items/generation.md` §2 r3, §4), low quality `0x005C2D40` (`items/quality.md` §6) |
| affixes, rare/set/unique, sockets, property values | item seed | `0x005C0D70`–`0x005C2D40` (low quality: unit seed, row above), `0x0065CF40`–`0x0065F6A0` |
| room monster population: density tests and the `sparsePopulate` roll | game seed | `0x0054EC90` (`monsters/population.md` §3, §7) |
| room monster population: class picks, boss type, spawn points | active room seed | `monsters/population.md` §3–§11 |
| ambient spawns, spawn positions | active room seed | `0x0054F060`, `0x005B2A00` |
| region monster lists | monster-region seed | `0x005475E0`, `0x005BDB20` (sim-0004) |
| AI decisions (largest group: 314 inline sites) | monster's unit seed | `0x005E0490`, `0x005E6320`, `0x005F1800` |
| client position resync lock (C→S 0x5F, `sim/pathing.md` §1.6) | player's unit seed | `0x0054CC40` |
| combat: damage, crit, deadly strike | attacker or defender unit seed | `0x0057B7D0`, `0x0057D760`, `0x0057DD60`, `0x0064A850` |
| skills | caster's unit seed | `0x005BE3F0`–`0x005DFBF0` |
| unique monster names | unit seed | `0x00653ED0`–`0x00653F50` |
| objects (shrines, chests) | object / object-control seed | `0x0054F5D0`–`0x00552B50` |
| DRLG layouts, mazes, outdoor presets | level seed | maze draws `0x006711A0`–`0x00673EC9` (`drlg/maze.md`), `0x006744F0`–`0x00674E40`, `0x0067EED0`–`0x00681240` |
| DRLG tile substitution (lvlsub shuffle and pick; lvlsub start index) | level seed (level +0x1C4) | `0x0066F690`, `0x0066F990` (`drlg/rooms.md` §9.8) |
| DRLG tiles: weighted tile choice, door units, per-room substitution | DRLG room seed (room +0x14), after the reset | `0x0066D820`, `0x0066D9E0`, `0x00670170` (`drlg/rooms.md` §9.8) |
| DRLG sub-theme pick | DRLG room seed **before** the reset | `0x006706D7` in `0x006706A0` (`drlg/rooms.md` §9.8) |
| client weather, particles, missiles, light colour | client seeds (§5.3, §5.5) | `0x00473090`, `0x00476190`, `0x004CDDB0`–`0x004D8260`, `0x004ACC70` |
| sound variants, NPC greetings, unit sound timers, ambience cues | local player's client unit seed (unit `[0x007A6A70]` +0x20, §5.3) | `0x004E40A0` (a third copy of `roll`) from `0x00482680`, `0x004E0590` and the trigger code; owners `audio/sound-table.md` §4, `audio/triggers.md` Randomness, `audio/environment.md` §7. The same seed is also stepped outside audio (full list and order: `audio/sound-table-2.md` §14, PC 2): the overlay create `0x00470390` for **any** unit's overlay (it takes the local player from `0x00463DD0`; type 6: `roll(frames)` at `0x004704DD`; argument a ≠ 0: `roll(a × 256)` at `0x004705B6`; argument b ≠ 0: `roll(b × 16)` at `0x004705D7`; owner `render/unit-composite.md` OQ 13); the client skill do functions of table `0x00727BA8` (`cltdofunc`) entries 5, 24, 32, 34, 54, 56, 63, 71, 77, 82, 86, 87, 89, 90, which step the caster's seed, so a cast by the local player steps it; and draw-phase users (cursor step `0x004681C0` on `GetTickCount` time, screen shake `0x00476D40`, weather `0x00473F50`, water floors `0x004DE410`, lightning `0x00473910`), whose count depends on drawn frames |

Helper call sites: `roll` 476 (mostly AI, skills, items, missiles),
`roll_range` 37, `mask` 14 (server AI/skills), `step` 1, `mask_range` 0.
Observed volume while entering Act 1 (45 s full-hook run): 32,543 draws
from 50 functions; DRLG tile code (`0x0066F690` alone: 15,042) and
client weather (`0x00473090`: 5,160) dominate.

## Constants & data dependencies

| Constant | Value | Use |
|---|---|---|
| `K` | `0x6AC690C5` | step multiplier (§2) |
| init high word | 666 (`0x29A`) | §4 |
| default seed | `{1, 666}` | `init()` |
| time-value LCG | `x·0x19660D + 0x3C6EF35F`, three rounds, then `& 0x7FFFFFFF` | §5.1 |

No `.txt` / `.bin` data is read by the generator.

## Randomness

This spec is the generator. Draw order per system belongs to the system
specs; §7 lists which seed each system draws from.

## Edge cases & original bugs

1. `roll(n)` with `n ≤ 0` (including `n = 0x80000000`) draws nothing and
   returns 0. Callers that compute `n` from data (e.g. a zero total
   weight) silently get 0 and leave the seed unchanged.
2. `mask(n)` assumes a power of two; with any other `n` it returns
   `lo' & (n−1)`, which is biased and can skip values. Reproduce as is.
3. `roll` uses `lo' mod n`: results are slightly biased toward small
   values when `n` does not divide 2^32. Reproduce as is.
4. Seeds are value types: a struct copy duplicates the generator. Two
   objects with equal seeds (e.g. client and server copies of a level)
   produce identical sequences; this is observed (sim-0005 notes).

## Test vectors

Synthetic (from the rule; CI-safe unit tests):

| Input | Expected | Source |
|---|---|---|
| step `{1, 666}` | `{1791398751, 0}` (= K + 666) | §2 |
| step `{0, 666}` | `{666, 0}` | §2 |
| step `{666, 0}` | `{3365183618, 277}` | §2; also recorded (sim-0001 draw 2) |
| step `{0xFFFFFFFF, 0xFFFFFFFF}` | `{0x95396F3A, 0x6AC690C5}` | §2 bound |
| 5 steps from `{1, 666}` | lo: 1791398751, 791599131, 671516612, 3064641593, 3217527747; hi: 0, 747178749, 330169957, 280084454, 1278238622 | §2 |
| `roll` from `{1, 666}`, n = 0, −5, 0x80000000 | 0, state stays `{1, 666}` | §3.1 |
| `roll` from `{1, 666}`, n = 1 / 8 / 10 / 100 / 0x7FFFFFFF | 0 / 7 / 1 / 51 / 1791398751, state `{1791398751, 0}` | §3 |
| `mask` from `{1, 666}`, n = 0 / 16 / 10 | 1791398751 / 15 / 9 | §3.2 |
| `mask_range(3, 8)` from `{1, 666}` | 10 | §3 |
| `roll_range` from `{1, 666}`: (5, 0) / (5, 10) / (−3, 8) | 5 (no step) / 6 / 4 | §3 |
| time value (§5.1) of input 0 / 12345 | 1372387049 / 185352726 | §5.1 |

Recorded from the running 1.14d game (`trace-recorder 0.1.0`,
2026-10-05; `Game.exe -w -ns`, character created in an earlier session,
Normal). Each trace gives a start seed and every draw on it, in order;
`check_rng.py` and the converter's replay check pass on all of them.

| Trace | Seed / start | Draws | What it covers |
|---|---|---|---|
| `sim-0001` | client global seed `{0, 666}` | 64 × `roll`, n ∈ 1..4 | helper `roll`; reproducible: identical first 581 draws in two separate runs |
| `sim-0002` | game seed `{971488495, 666}` (time-based) | 57 × step | game seed after creation; children derived from it (§5) |
| `sim-0003` | DRLG seed `{644409375, 666}` (character map seed) | 7 × step | first step = level start seed 4014346869 |
| `sim-0004` | monster-region seed `{664322703, 666}` | 64, inline steps mixed with `roll` | interleaving of inline and helper draws on one seed |
| `sim-0005` | level seed `{4014346871, 666}` (level 2) | 64 × step | DRLG; same 3,899-draw sequence in both DRLG copies and in two runs |

A few values from those traces, for unit tests that should not load JSON:

| Seed before | Op | Value | Seed after | Trace |
|---|---|---|---|---|
| `{0, 666}` | roll(3) | 0 | `{666, 0}` | sim-0001 #1 |
| `{666, 0}` | roll(3) | 2 | `{3365183618, 277}` | sim-0001 #2 |
| `{971488495, 666}` | step | 7657093 | `{7657093, 405200438}` | sim-0002 #1 |
| `{7657093, 405200438}` | step | 664322703 | `{664322703, 3193715}` | sim-0002 #2 |
| `{644409375, 666}` | step | 4014346869 | `{4014346869, 268778232}` | sim-0003 #1 |
| `{1936801471, 624310379}` | roll(3) | 2 | `{1281421670, 807825114}` | sim-0004 #4 |

`rng_draw` event data (traces/FORMAT.md, kind `rng_draw`): `op`
(`step` | `roll` | `roll_range` | `mask` | `mask_range`), `n`, `min`
(when the op takes them), `value` (the returned value; for `step` the new
`lo`), `state` (`{lo, hi}` after the op), `site` (1.14d call site,
informational, in `compare.ignore`). A replay starts from `setup.seed`
and applies each `op` in order; `value` and `state` must match.

## Provenance

- **1.14d `Game.exe`** (SHA-256 `631066c1…adaaf`, fixed image base
  `0x400000`, no ASLR). Helpers and setters: addresses in §3 and §4,
  read from the Ghidra export and raw bytes. The multiplier appears as an
  immediate 852 times in `.text`, always as `mov ecx|edx, K` followed
  within 14 bytes by `mul ecx|edx`; 6 are inside the helpers, 846 are
  inlined draws. Several inlined draws reuse one `mov` for two `mul`s
  (power-of-two and modulo branches of an inlined `roll`).
- **Recorded:** `tools/trace-recorder/record_rng.py` hooks the six
  helpers (entry + return), the three setters and all 846 inline sites
  (single-stepping from the `mov` to the `mul` and the add/adc that form
  the new state). Four runs on 2026-10-05; the full-hook runs logged
  every draw on the main thread (the only thread that drew), and every
  draw's seed-before was either a setter's value or the seed-after of an
  earlier draw (0 unexplained states), which also shows that no draw
  site was missed in those runs. `check_rng.py` confirms §2 and §3 on
  all of them.
- **D2MOO** (1.10f) `D2Common/D2Seed.h/.cpp` describes the same step,
  `roll` (`SEED_RollLimitedRandomNumber`), init values and time value.
  Differences: D2MOO's time-value multiplier is `0x2F490A95`, 1.14d
  applies the LCG three times (combined multiplier `0xAF490A95`); both
  agree after the final `& 0x7FFFFFFF`. D2MOO's `SEED_RollPercentage`
  takes a 64-bit modulo of the whole state; 1.14d has no such helper
  (see §6 for how 1.14d computes percentages). D2MOO writes some
  modulos as signed (`Missile.cpp`, `Monsters.cpp`); the 1.14d
  equivalents (`0x0064A850`, `0x006637F0`) are unsigned. DRLG seed
  offsets moved (DRLG 0x14 → 0x00, level 0x14 → 0x1C4, DRLG room
  0xD4 → 0x14, active room 0x3C → 0x6C); game, unit and item offsets
  match D2MOO.
- **Static survey** of all 428 drawing functions and their callers
  (seed owners, offsets, consumer forms) for §5–§7.
- §7 sound-seed row, non-audio users (PC 2 request, 2026-10-07):
  `0x00470390` read in `tools/ghidra/disasm.py` (`0x004703E1` local
  player, steps at `0x004704DD`, `0x004705B6`, `0x004705D7`); table
  `0x00727BA8` entries read from the image with `pefile`; the rest as
  surveyed in `audio/sound-table-2.md` §14.

## Open questions

1. §5–§7 seed owners, offsets and system attributions come from a static
   survey of the Ghidra export, where register arguments are often
   dropped. Each system spec must confirm its seed and draw order with a
   trace before relying on them.
2. Map seed source in single player: observed equal to the `.d2s` map
   ID. Confirm for a brand-new character and after save-and-exit.
   Static part answered in `drlg/levels.md` OQ 5: the join routine
   `0x00532690` copies the join record's +0x7E into game +0x7C (game
   type 3, no `-seed`, same difficulty); the recording question stays.
   *Answered* (static, 1.14d asm): single player's client game type
   `0x007A0610` is 0, so the create message (`0x00477CA0`) carries game
   type 3 (`0x00477CDF`), which `0x0053F17A` passes to `0x00530BF0` and
   `0x00530CFF` stores at game +0x6A. Game +0x84 is 0 unless `-seed`
   ran (§5 table, game seed row). The header read `0x0056A090` then sets
   game +0x7C := `.d2s` +0xAB when game +0x6A = 3, game +0x84 = 0 and the
   town byte `.d2s` +0xA8 + difficulty (game +0x6D) has bit 0x80
   (`0x0056A1D4`–`0x0056A217`). So every single-player load in the
   difficulty the save was written in restores the saved map seed; in
   another difficulty that byte lacks 0x80 and the game keeps its own
   new seed (owner of the load rule: `formats/d2s-load.md` §7, PC 2).
3. *Answered* (static, disassembly of `0x0054FCB0`): the D2MOO form is
   there, inline: the last step of the object-control seed (`0x0054FD32`)
   gives `lo'`, reduced `lo' mod 65534` by a multiply-shift (magic
   0x20005, `imul 0xFFFE` at `0x0054FD5E`), plus 1, then `init_low` of
   the object's unit seed (unit +0x20) with it (`0x00650E40`). Owner of
   the rule and its draw order: `world/objects.md` §5.2 (init 3, chest;
   init 57 runs it too).
4. *Answered* (static): 0. `0x008846E8` lies in `.data` beyond the
   file's raw data (zero-filled at load) and its only writer is the
   increment at `0x00552E67`; no GetTickCount. The argument to the time
   value is twice the pre-increment counter (§5.3 table).
5. Do other threads draw in hosted multiplayer games? In single player
   only the main thread drew.
