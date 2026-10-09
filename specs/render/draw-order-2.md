# Spec: Render — Draw order, part 2 (weather, level backgrounds, edge floors, sight test)

- **Status:** draft (2026-10-06, RE on 1.14d `Game.exe`; no capture
  check yet). Every rule names its 1.14d address. Continues
  `render/draw-order.md` (owner of the passes, the grid and the lists);
  section numbers continue from its §10.
- **Target version:** 1.14d
- **Crate/module:** `d2-client::rules::draw_order` (passes 1, 4, 9; floor
  side effects), `d2-client::scene` (weather state)
- **Related specs:** `render/draw-order.md` (§1 passes, §5 r3 sight test,
  §6 r2 floors), `render/blend-modes.md` (§1 draw modes, §8 lines and
  rectangles), `render/shading.md` §5 (`nearest`), `render/camera.md` §1
  (open mode, `W`, `H`), `render/capture.md` §3.4, `sim/rng.md` (§3 helpers,
  §5.5 client-only seeds), `drlg/rooms.md` §10 (collision grid),
  `drlg/levels.md` §11 (coordinate records)

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 38–48 |
| Inputs | 49–59 |
| Outputs / state changes | 60–65 |
| Rules | 66–67 |
|   11. Weather (passes 4 and 9; water floors) | 68–349 |
|   12. Level backgrounds (pass 1) | 350–403 |
|   13. Pass 8 (`0x00475B20`) | 404–412 |
|   14. Edge floors (`0x004DE6C0`, `0x004DE630`) | 413–438 |
|   15. Sight test (`draw-order.md` §5 r3) | 439–485 |
|   16. Line test (`0x0064E260`) | 486–515 |
| Constants & data dependencies | 516–526 |
| Randomness | 527–538 |
| Edge cases & original bugs | 539–555 |
| Test vectors | 556–577 |
| Provenance | 578–610 |
| Open questions | 611–686 |
<!-- /index -->

## Summary

The world draw of `draw-order.md` §1 has four passes that draw no map
tile and no unit: the level background (pass 1, Arcane Sanctuary and
Arreat Summit only), the environment pools (pass 4: rain splashes and
mud bubbles), a dead debug pass (pass 8) and the lightning flash plus the
rain or snow particles (pass 9). The floor pass also spawns the splashes
and bubbles and, in some outdoor levels, draws edge floors. This part
owns those passes, the weather state they read, and the line-of-sight
test that hides units (`draw-order.md` §5 r3).

## Inputs

| Name | Source |
|---|---|
| local player unit: client position, seed (`unit +0x20`), level | client units (`sim/rng.md` §5) |
| level record of the player's level: `Rain` (+0x05), `Mud` (+0x06), `DrawEdges` (+0x09) | `levels.txt` (`0x0061DBA0`, `0x0061DC20`, `0x0061DB70`) |
| leveldefs `LOSDraw` (+0x98) | `levels.txt` (`0x00642840`) |
| client update count (`0x0044DB00`), `GetTickCount` | frame cycle |
| camera origin delta of the last drawn frame `[0x007A0678]`, `[0x007A067C]` | `0x0044CA2D` (camera x / y minus the previous frame's, after the shake) |
| open mode, `W`, `H`, low-quality setting `[0x0072DA50]`, perspective `0x004F51D0` (0 on GDI), resolution mode `[0x007C8CB8]` | `camera.md` §1, `shading.md` §4 |

## Outputs / state changes

Pixels of passes 1, 4 and 9; the weather state of §11.1; draws of the
local player's seed (§11, every one listed in Randomness); the two
time-seeded background seeds (§12).

## Rules

### 11. Weather (passes 4 and 9; water floors)

#### 11.1 State

Three pools (`0x006BCA10`, created at client start `0x00472320`; record
+0x00 is a u16 tag, 0 = free; allocation `0x006BCB40` takes the first
free slot from the lowest free index; `+0x110` highest used index,
`+0x114` live count; clear `0x006BCAC0` frees all):

| Pool | Global | Slots × bytes | Name in memory | Drawn by |
|---|---|---|---|---|
| particles (rain drops or snow flakes) | `[0x007A8A04]` | 256 × 0x28 | `Environment Particles` | pass 9 (§11.7) |
| splashes | `[0x007A89FC]` | 512 × 0x18 | `Environment Splashes` | pass 4 (§11.6) |
| bubbles | `[0x007A8A00]` | 128 × 0x18 | `Environment Bubbles` | pass 4 |

Splash / bubble record: +0x04 x, +0x08 y (screen pixels), +0x0C cel kind
k (0–3), +0x10 frame, +0x14 countdown. Cel files (act load `0x00472890`,
`DATA\GLOBAL\UncompOverlays\`): splashes `Rain1`–`Rain4` (`[0x007A89B0]`
+ 4k, frame counts `[0x007A89C8]` + 4k), bubbles `bubble1`–`bubble4`
(`[0x007A893C]`, counts `[0x007A892C]`); the count is the cel file's
frame count (`0x006019F0`).

Particle record: +0x04 x, +0x08 y, +0x0C ground y, +0x10, +0x14 shape
values, +0x18 phase (0–511), +0x1C landed flag, +0x20 bounce count,
+0x24 color index, +0x25 alpha byte.

Scalars: rain on / mud on of the previous update (`[0x007A8A44]`,
`[0x007A8A40]`); snow mode `[0x007A8A14]` (Act V, `0x004726F0`); rain
cycle phase `[0x007A8A24]`, its length and countdown (`[0x007A8A38]`,
`[0x007A8A3C]`); peak and target particle counts (`[0x007A89C0]`,
`[0x007A89E0]`); intensity (float `[0x007A89A0]` = target × 1/256,
constant `0x006D6F08`); wind (`[0x007A89C4]` moving toward
`[0x007A89F8]`, retarget countdown `[0x007A89E4]`); lightning on
`[0x007A8A08]`, its countdown, phase and frame trigger (`[0x007A8968]`,
`[0x007A896C]`, `[0x007A89E8]`), thunder flag `[0x00712B4C]` (`.data` 1);
weather update mark `[0x007A8A0C]`.

Initial values: every scalar above except the thunder flag lies past the
`.data` section's file bytes (raw size 0x48000 from 0x00705000), so it is
0 at program start, and no code resets the cycle: phase, length and
countdown start at 0 and **carry over from game to game** (act load
`0x00472890` sets only the cycle tables, the colors, the cels and the
intensity). `[0x007A8A20]` has no writer at all (no instruction stores to
it): always 0.

#### 11.2 Weather update (`0x00473F50`, frame `0x0044CA5E`, before `StartDraw`)

1. `r`, `m` := `Rain`, `Mud` of the local player's level (both 0 without
   a player unit).
2. If `r` = 0 and the stored rain flag was set: clear the particle pool
   and the splash pool (each only when its live count ≠ 0). If `m` = 0
   and the stored mud flag was set: clear the bubble pool likewise.
   Store `r`, `m` as the flags.
3. `c` := client update count. No local player: fatal 0x547. If `c` >
   mark: mark := `c`; if `r` = 0: intensity := 0.0 (the target is
   kept); else, on the player's
   seed: move the particles if any are live (`0x004732C0`, §11.9), update the
   splashes (§11.5), run the rain cycle (§11.3, `0x00473E50`), top up
   particles and wind (§11.4, `0x004737B0`). Then, if `m` ≠ 0, update the
   bubbles (§11.5).

So the weather advances at most once per client update, however many
frames are drawn.

#### 11.3 Rain cycle (`0x00473E50`)

Runs unless the snow lock `[0x007A8A1C]` is set and the phase is not 2
(then, `[0x007A8A20]` being always 0, the phase becomes 2 and
`0x00472400` runs instead).

1. When the countdown is 0: phase `p` := (`p` + 1) mod 4; length `D` :=
   `roll_range(Min[p], N[p])` (`sim/rng.md` §3) with `Min` =
   `[0x007A8970 + 4p]` = 7,500, 250, 3,000, 125 and `N` = `[0x007A8958 +
   4p]` = 7,500, 250, 3,000, 50 (set at act load; `Min[2]` × 3 in acts III
   and V, `0x006427F0` = 2 or 4); countdown := `D`; phase entry
   `0x00473D00(p)` — p > 3: fatal 0x12A; snow lock set and p ≠ 2:
   nothing, the stored phase stays (so a locked cycle keeps phase 2
   while this call still uses p for r3); else phase := p and: 0 →
   target := 0; 1 → `0x004726F0(0)` (snow and
   lightning re-check, §11.8), peak := `roll_range(32, 224)`; 2 →
   `0x00472400` (level presets, §11.8); 3 → lightning off.
2. Countdown −1.
3. Phase 1: target := peak × (`D` − countdown) / `D`; phase 3: target :=
   peak × countdown / `D` (unsigned 32-bit product and division); phases
   0 and 2 keep it. Intensity := float(target) × (1/256).

Units are weather updates (one per client update with rain on).

#### 11.4 Particles: top-up, wind, lightning timer (`0x004737B0`)

1. While the live particle count < target: spawn one (`0x00473090`, r5).
2. Wind: step `s` = 1 in snow mode, else 2; the wind moves by `s` toward
   its goal and stops on it.
3. Retarget countdown −1; at 0: rain mode: countdown := 125 + (step mod
   375), goal := 92 + (next step mod 71) (two raw steps, `lo'`, unsigned
   `mod`); snow mode: countdown := 250 + (step mod 875), goal :=
   `roll_range(Gmin, Gn)` with `Gmin` = `[0x007A89D8]`, `Gn` =
   `[0x007A8998]` (42, 170 from client start; per level §11.8).
4. Lightning on: its countdown −1; at 0 the frame trigger := 1 (§11.7).
5. Spawn (`0x00473090`), rain mode: x := `roll_range(0, W)`; ground y
   `g` := `roll_range(40, H − 87)`; y := `roll_range(−20, g + 20)`;
   phase := step & 511; landed := 0; color := a step `mod 12` into one of
   three 12-entry tables by the act's day period (`0x0061C100`, 0–3):
   0 → `[0x007A8980]` with alpha 0x7F, 1 or 3 → `[0x007A894C]`, 2 →
   `[0x007A89A4]` (alpha 0xFF; the alpha byte is never read by the GDI
   line, `blend-modes.md` §8 r1); bounce count 3. Shape values: with
   `t` = float32((`g` − 40) / (`H` − 87)) (x87 divide, stored as a float;
   0 ≤ `t` < 1), +0x10 := 4 − trunc(−8.0 · `t`) and +0x14 := 15 −
   trunc(−15.0 · `t`) (constants `0x006D6E50`, `0x006D6E48`): exactly
   +0x10 = 4 + ⌊8(`g` − 40) / (`H` − 87)⌋ and +0x14 = 15 + ⌊15(`g` −
   40) / (`H` − 87)⌋ (the float rounding never crosses an integer for
   these ratios). +0x10 is the drop length (§11.7 r3).
   Snow mode, after the phase step: one more raw step (value unused),
   bounce count 1, +0x14 := 8 − trunc(−28.0 · `t`) = 8 + ⌊28(`g` −
   40) / (`H` − 87)⌋ (`0x006D6E58`), size `s` +0x10 := ((`g` − 40) × 7)
   / (`H` − 87) (+1 while `[0x007A8A18]` is set), `s` outside 0–7 fatal
   0x390; then `0x00472FB0` (no draws) sets alpha and color by the day
   period `p`: alpha := 200, 160, 80, 160 for `p` = 0, 1, 2, 3 (bytes
   `0x006D6E41` + 2`p`; it replaces the 0xFF), `p` > 3 fatal 0x356;
   `i` = (12`s`) / 8; video mode (`0x004F5140`, `ui/panels.md`) 1–3 or
   6: color := `S[i]`, `S[i / 2]` or `S[i / 4]` for `p` = 0, 1 or 3, 2
   (`S` = `[0x007A898C]`); other modes: `S'[i]` (`[0x007A89EC]`). The
   size bump flag `[0x007A8A18]` is set by §11.8.
   Color tables (act load `0x00472890`, for `i` = 0 … 11, `q` =
   ⌊80`i` / 12⌋, `nearest` = `shading.md` §5 over the current palette
   `0x0081E668`, `composition.md` §4):

   | Table | Entry `i` = `nearest(r, g, b)` |
   |---|---|
   | `[0x007A8980]` (rain, `p` 0) | `v` = 98 − `q`: (`v`, `v` + 25, `v`) |
   | `[0x007A894C]` (rain, `p` 1, 3) | `v` = 45 − ⌊40`i` / 12⌋: (`v`, `v` + 10, `v`) |
   | `[0x007A89A4]` (rain, `p` 2) | `v` = 25 − 2`i`: (`v`, `v` + 5, `v`) |
   | `[0x007A898C]` (snow `S`) | `v` = 120 + `q`: (`v`, `v`, `v`) |
   | `[0x007A89EC]` (snow `S'`) | `v` = 170 + `q`: (`v`, `v`, `v`) |

#### 11.5 Water floors: splashes and bubbles (`0x004DE410`, `0x00472DA0`, `0x00472EC0`)

The floor pass (`draw-order.md` §6 r2) keeps a context per frame
(`0x004DE730`, context `0x007C8A28`): `k` := trunc(intensity × 1000.0)
(`0x00682FD0`; constant `0x006DB9D0` = 1000.0), stored at +0x10, so `k`
= ⌊target × 1000 / 256⌋ (0 … 996; a rain peak of 32–224 gives 125–875);
when the update count `c` > `last_s` + 3: `last_s` := `c` (whatever `k`
is) and `splash` := 1 if `k` ≠ 0; when the level has `Mud` and `c` >
`last_b` + 25: `last_b` := `c`, `bubble` := 1. `last_s`, `last_b`
(+0x14, +0x18 = `0x007C8A3C`, `0x007C8A40`) are 0 at program start and
never reset; the flags are cleared at the start of each frame.

After **every drawn floor** whose DT1 material has bit 0x2 (water):

1. `r` := `roll_range(0, 1000)` on the local player's seed — drawn
   whether or not either flag is set (this is the draw `camera.md` §8
   and `capture.md` §3.4 must count).
2. `splash` and `r` < `k` (signed): spawn a splash (`0x00472DA0`) at
   the floor's screen position (X, Y) of `camera.md` §6 floors.
3. `bubble` and `r` < 100: spawn a bubble (`0x00472EC0`) there.

Spawn (GDI, perspective 0; both on the player's seed; nothing when the
pool is full): `a` := `roll_range(0, 80)`; `b` := 80 − 2·|`a` − 40|;
x := X + `roll_range(−b, 2b)` (no draw when `b` = 0); y := Y + `a`
(splash) or Y + `a` − 30 (bubble). Splash: kind := 2 + (one raw step,
`lo'` & 1) — `Rain3` or `Rain4`. Bubble: kind 2 (`bubble3`). Frame 0,
countdown 2.

Update (`0x00472C80` splashes, `0x00472D10` bubbles; §11.2 r3): each
live record: x −= `[0x007A0678]`, y −= `[0x007A067C]` (perspective 0);
countdown −1; below 1: frame +1, countdown := 2; frame ≥ the kind's
frame count → freed.

#### 11.6 Pass 4: environment pools (`0x00473C00` → `0x00473A70`)

Splash pool (cel files `Rain1`–`4`) when its live count ≠ 0, then the
bubble pool (`bubble1`–`4`) likewise. Records in slot order 0 …
highest used; a record draws (GDI) when, with `sx` = shiftX of
`camera.md` §1, `x + sx` lies in [`L`, `R`) and `y` in [0, `H` − 47):
`L`, `R` = 0, `W` (open mode 0, 3), 0, `W` − 2·(`W`/4) (mode 1), 2·(`W`/4),
`W` (mode 2). Draw: cel frame +0x10 of the kind's file at (`x + sx`, `y`)
through `D2GFX_DrawCelContext` with light −1 (unlit), draw mode 3
(`blend-modes.md` §1), palette 0.

#### 11.7 Pass 9: lightning flash and particles (`0x00473910`)

1. No local player: fatal 0x573.
2. Lightning on and frame trigger ≠ 0: trigger −1; when it reaches 0:
   lightning phase 0 → countdown := 3, phase 1; phase 1 → countdown :=
   `roll_range(500, 1500)`, phase 0, and if the thunder flag is set:
   sound 202 with volume `roll_range(25, 50)` (`0x004B9A00`); when a
   sound starts, `roll_range(−200, 400)` twice for its position
   (`0x004B99A0`); a cleared thunder flag is set instead (first strike
   silent). Then (whether or not the trigger reached 0), when the frame
   rate of the last second `[0x007BB390]` (written by `0x00477980`,
   frames counted per 1,000 ms) is ≥ 10 (unsigned): a rectangle of color
   255, draw mode 5 over (`L`, 0)–(`R`, `H` − 47) (`blend-modes.md` §8),
   and the pass ends (no particles that frame). Below 10 the pass goes
   on to r3: the particles draw, with no flash.
   Measured value (2026-10-09, pc1-day3-c, REC-576 (6)): read with no breakpoints on Windows
   (`-w -ns`, ScnAma in the Rogue Encampment, polled every 250 ms for 30 s), `[0x007BB390]` is 0 in
   the menus, 30–44 during the first ~10 s after the arrival, then 21–26 (25 in 50 of 72 samples).
   Under the trace-recorder debugger it reads 0–12 because the hooks slow the client. So in normal
   play the gate is always met and the flash draws; a fixed 25 gives the same result.
3. Else, when rain is on in the level and the particle pool is live,
   draw the particles (`0x00473470`), only when the low-quality setting is
   0; records in slot order, tested against [`L`, `R`) × [0, `H` − 47)
   (no shiftX):
   - snow mode, size `s` in 0–7, origin in range: two lines (`blend-modes.md`
     §8) from the 16-byte entries `T[s]` and `T[s + 1]` of the table at
     `0x006D6E78` (x0, y0, x1, y1 offsets): (x + a, y + b)–(x + c, y + d),
     color +0x24. `T` (9 entries, i32, read from `Game.exe`): 0 (0, 0, 0,
     1), 1 (−1, −1, 0, 0), 2 (0, −1, 0, 1), 3 (1, 0, −1, 1), 4 (1, 0, 0,
     1), 5 (2, −1, −1, 1), 6 (1, 1, −2, −1), 7 (2, 1, −2, −1), 8 (2, −1,
     −2, 2).
   - rain mode, landed: a one-pixel line at (x, y) if (x, y) is in range,
     color +0x24 (as every particle line).
   - rain mode, falling: a line from (x, y) to (x + u, y + v), drawn when
     either end is in range; with `w` the wind value and `Wt` the sine
     table of `lighting.md` §10 r1 (`0x00707800`, 512 floats): `u` =
     trunc(`Wt[(w + 128) & 511]` × len), `v` = trunc(`Wt[w & 511]` ×
     len), len = +0x10 (`0x0040B330`, `0x0040B350`); when `v` > `g` − y:
     `u` := (`g` − y) × `u` / `v` (C division), `v` := `g` − y.

Lightning is switched on only by `0x00472C50` (thunder flag := its
argument, trigger 1), called from `0x004E3C50`, which has no direct
caller (a table-dispatched handler; Open question 4). A level change
(`0x004726F0`, §11.8) clears it.

#### 11.8 Act load and level presets

At act load (`0x00454811` → `0x00472890`): snow / lightning re-check
`0x004726F0(1)`, the cycle tables of §11.3, the color tables, the eight
cel files, intensity := 0. `0x004726F0(arg)` (level entry and phase 1),
on the player's seed:

1. Level outside act V (`0x006427F0` ≠ 4): snow lock := 0; if snow mode
   was on, clear the particle pool (fatal 0x15B if it stays live); snow
   mode := 0; `0x00472610` (three raw steps: wind retarget countdown 125
   + (step mod 375), lightning countdown 500 + (step mod 1,500), wind and
   goal 92 + (step mod 71)).
2. Act V: `0x00472610` first; threshold `t` := 25 for levels 109–112
   (size bump `[0x007A8A18]` := 0), else 0; levels 117, 120, 121: size
   bump := 1, and lock := 1 for 120, 121. One raw step; if (step mod 100)
   < `t` and `arg` = 0: snow off as in r1 (no `0x00472610`). Else snow
   lock := lock; lock set → `0x00473D00(2)`; snow mode := 1
   (`0x00472590`: lightning off; the particle pool is cleared when the
   mode changes or `arg` ≠ 0).

`0x00472400` (phase 2, per level): 109–112 snow goal (42, 170), peak :=
`roll_range(32, 56)`; 117 goal (170, 56), peak := `roll_range(40, 112)`;
120, 121 goal (28, 28), wind and goal 28, peak := 256; other levels keep
the peak, lightning off, and in rain mode one raw step. Target := peak.

#### 11.9 Particle move (`0x004732C0`, §11.2 r3)

`Wt` is the 512-float sine table of §11.7 r3 (`0x00707800`); `Wc(a)` =
`Wt[(a + 128) & 511]` (`0x0040B330`), `Ws(a)` = `Wt[a & 511]`
(`0x0040B350`); `w` = the wind (`[0x007A89C4]`); trunc = the CRT
truncating conversion `0x00682FD0`; `dX`, `dY` = the camera delta of
the last drawn frame (`[0x007A0678]`, `[0x007A067C]`, Inputs table).

1. Speed factor `f` (float32): snow mode: max(|`Wc(w)`|, 0.25)
   (`0x006D6E70`); rain mode: float32(intensity × 0.15 + 0.85)
   (doubles `0x006D6E68`, `0x006D6E60`; intensity §11.1).
2. For each slot 0 … pool `+0x110` (highest used index), in index
   order, skipping free slots (tag 0):
   1. `A` := trunc(+0x14 × `f`); `ux` := trunc(`Wc(w)` × `A`); `uy` :=
      trunc(`Ws(w)` × `A`).
   2. y += `uy` − `dY`. y > ground y (+0x0C) → landed (+0x1C) := 1.
   3. Landed: bounce count (+0x20) = 0 → free the slot (`0x006BCCD0`)
      and, if the live count (+0x114) < target (`[0x007A89E0]`),
      spawn one particle (`0x00473090`, §11.4 r5; draws on the player
      seed). Bounce count ≠ 0 → `ux` := 0, +0x14 := 0, bounce count −1.
   4. Slot still live (tag ≠ 0): snow mode and not landed → `ux` +=
      trunc(2 × `Ws`((`F` × 512) / 25 + phase (+0x18))), with `F` =
      `[0x007A8A34]` and the product and quotient unsigned 32-bit (wraps).
      Then x += `ux` − `dX`; x ≥ `W` (`[0x0071146C]`) → x −= `W`; x < 0 →
      x += `W` (one correction each).
3. `F` += 1 (every call, even with an empty pool). `F` is `.bss` (0 at
   program start) and has no other writer: it never resets.

Spawn reuse: the spawn takes the lowest free slot (§11.1), which can be
the slot just freed; the loop then treats the new particle in step 2.4
with the freed particle's `ux` and,
in snow mode, the sway of its own phase. Reproduce.

### 12. Level backgrounds (pass 1)

Drawn first in the world draw (`draw-order.md` §1 row 1) when the local
player's level is 74 or 120. Both use their own time-seeded LCG
(`sim/rng.md` §5.5, `time_value` at first use), so a capture must record
the seed (`[0x00712C4C]` → seed for 74, `[0x00712C50]` → seed for 120)
or the case is not reproducible.

**Level 74, Arcane Sanctuary (`0x00476290`): stars.**

1. First call (flag `[0x007B955C]`, set to 1 at `0x004763D3` and never
   cleared: once per process): seed `[0x00712C4C]` := `init_low(
   time_value(shake start))` (`0x004762A5`–`0x004762C4`; the `init()`
   before it is overwritten), the argument being the screen-shake start
   tick `[0x007B8D10]` (`render/camera.md` §8; 0 when no shake has
   started since the process began); 8 colors: for
   `i` = 0…7, `base` = 128 + (128·`i`) / 7; `r`, `g`, `b` := min(255,
   `base` + (step & 63) − 32), one step each in that order; color[`i`] :=
   `nearest(r, g, b)`. Then 256 stars `j` (`0x00476190(j, 0)`): x :=
   `roll(W)`, y := `roll(H − 40)`, speed := −1 − (step mod 5), color :=
   color[step & 7].
2. Every call: for `j` = 0…255 a line from (x, y) to (x, y) (one pixel),
   color, alpha 0xFF.
3. Then, if `GetTickCount() − last` > 40 (unsigned): every star x +=
   speed; a star with x < 0 is re-made (`0x00476190(j, 1)`): x := `W` −
   1 + (step & 7), then y, speed, color as in r1; last := now (unchanged
   otherwise). `last` is `[0x007B57E0]`, a zero-filled `.data` global
   (past the file's raw data) written only at `0x00476453`; the
   first-call branch does not touch it and r3 runs on the first call
   too, so the first call moves the stars once (`now − 0 > 40` unless
   the tick count is ≤ 40). Answers OQ 12 (2026-10-09).
   ```
   if now - last > 40 { for j in 0..256 { x[j] += speed[j]; if x[j] < 0 { remake(j) } }; last = now }
   ```

**Level 120, Arreat Summit (`0x00476460`): mountains and clouds.**
Skipped while the client's exit flag `[0x007A0620]` is set.

1. `d` := player client x − 10,063 (`0x274F`; the first-call capture of
   the player's x is overwritten by this constant at once); `q` := `d` / 8,
   `c` := `q` rem 256, `t` := (`q` / 256) mod 4 in 0…3 (C division and
   remainder, then `t` made non-negative); `x0` := −`c`.
2. Cel file `data\global\ui\summit01` (loaded once): row y = 256 frames
   (`t` + k) & 3 at x0 + 256k for k = 0…3, and, when x0 > 0, frame
   (`t` + 3) & 3 at x0 − 256; row y = 512 the same frames + 4; row y = 768
   the same + 8 only in resolution mode 2. Light −1, draw mode 5.
3. Clouds `data\global\ui\cloud01` (10 clouds, first use, flag
   `[0x007B9560]`, once per process: seed `[0x00712C50]` := `init_low(
   time_value(GetTickCount()))` (`0x0047680F`–`0x0047682F`; the mix
   input is `time() + 2·GetTickCount()`); x16 := `roll(16·W)`, y := step mod 250, speed := (step & 15) +
   8). Every call: x16 += speed; x16 > 16·(`W` + 368) → x16 := −5,888,
   y := step mod 300. Draw frame 0 at (x16 / 16, y) and frame 1 at
   (x16 / 16 + 256, y), light `0xDDDDDDDD`, draw mode 3.

### 13. Pass 8 (`0x00475B20`)

Pass 8 runs when `[0x007B9564]` ≠ 0. Its only writer is the toggle at
`0x004769B0`, which has no caller and no pointer anywhere in `Game.exe`
(`disasm.py xref`); the global is in `.bss` (0). So pass 8 never runs
in 1.14d: it is a debug view handing the light map (`0x007B0E68`) and
the light list to driver slot `+0xCC` (`0x004F68B0`). d2rs draws
nothing for it.

### 14. Edge floors (`0x004DE6C0`, `0x004DE630`)

Active when the level's `DrawEdges` (+9) ≠ 0, open mode 0 and
resolution mode 2 (`0x004DE730`). While drawing floors the pass keeps
the sub-tile extents of the drawn floors (`0x004DDE80`: min / max of
the record's sub-tile x and y). After the last room, with (`px`, `py`)
the player's sub-tile and margins `A` = 30, `B` = 25 (perspective: 33,
23):

| Test | Strip start | Step |
|---|---|---|
| `px − min x` < `A` | (min x − 5, `py`) | (0, −5) |
| `max x − px` < `B` | (max x + 5, `py`) | (0, +5) |
| `py − min y` < `A` | (`px`, min y − 5) | (−5, 0) |
| `max y − py` < `B` | (`px`, max y + 5) | (+5, 0) |

A strip snaps its start to the tile grid (`(v / 5) × 5`, C division)
and draws three edge floors at start, start + step, start + 2 × step.
Each is the act's edge record (act `+0x18`, `0x00619720`) drawn through
the floor draw `0x004DE410` with filter argument 1 at the sub-tile's
floor position (the same handed position as a floor record of that tile,
`camera.md` §6 floors); it passes the same tests (flags 0x408, view
test, orientation 0), may itself spawn splashes (§11.5), and a drawn edge
floor widens the extents too (`0x004DDE80`). The four tests run in table
order and each reads the extents as left by the strips before it.

### 15. Sight test (`draw-order.md` §5 r3)

1. **Level gate** `0x00642840`: the leveldefs `LOSDraw` of the local
   player's level (`levels.txt`; 83 levels in 1.14d, all indoor: caves,
   crypts, Monastery and Catacombs, sewers, tombs, Arcane Sanctuary,
   Act III dungeons and temples, Act V ice caves). 0 → every unit passes.
2. **Unit line** `0x00622AA0(a = local player, b = unit, mask 2)`
   (§15.1; mask 2 = collision bit 0x0002, `drlg/rooms.md` §10.6):
   blocked → hidden.

#### 15.1 Collision line between two units (`0x00622AA0(a, b, mask)`)

Answers 0 (clear) or 1 (blocked). D2MOO name
`UNITS_TestCollisionWithUnit`; 1.14d read below.

1. `a` or `b` null → fatal error (lines 0x1290 / 0x1291 of the unit
   source, then exit). Room of `a` (`0x00620BB0`) null → 0 (clear).
2. Positions `(ax, ay)`, `(bx, by)`: sub-tile x / y (`0x0045ADF0` /
   `0x0045AE20`): objects, items and tiles (kinds 2, 4, 5) read the
   static path's x, y (+0x0C, +0x10); other kinds the dynamic path's
   current sub-tile (`0x006488C0` / `0x00648900`, high words of the
   precise position, `sim/path-placement.md` §1, §2.3); no path → 0.
3. Sizes `sa`, `sb` = `sim/path-placement.md` §3 size (`0x00620510`;
   a monster's `SizeX` is signed); each value > 2 becomes 2 (negative
   values are kept).
4. `dx` = |`bx − ax`|, `dy` = |`by − ay`| (signed 32-bit). `dx + dy` <
   `sa + sb` → 0 (touching units are never blocked).
5. Unless `sa` = `sb` = 0, pull the ends toward each other
   (`0x00622920`): if `dy` ≤ `dx`: when `ax` < `bx`, `ax` += `sa` and
   `bx` −= `sb`, else `ax` −= `sa` and `bx` += `sb`. If `dy` ≥ `dx`: the
   same on y (`ay` < `by` → `ay` += `sa`, `by` −= `sb`, else the
   reverse). `dx` = `dy` moves both axes.
6. Result = the line test §16 (`0x0064E260`)
   from `a`'s room, `(ax, ay)` → `(bx, by)`, with `mask` (collision
   bits, `drlg/rooms.md` §10.6). The stop cell it writes back is
   discarded.

Callers (16 sites in 1.14d) and the mask each pushes:

| Mask | Sites (function) | Use |
|---|---|---|
| 2 | `0x004B98CD` (`0x004B9890`), `0x004BA2F5` (`0x004BA020`) | client sound volume (`audio/sound-table.md`) |
| 2 | `0x004DC780` (`0x004DC710`) | client unit draw sight test (§15 r2) |
| 4 | `0x005DCB1B`, `0x005DCC27`, `0x005DCCA5`, `0x005DD294`, `0x005DD678`, three in `0x005DD7F0`, `0x005E4B9F`, `0x005EBE36`, `0x005F7C63` | monster AI line of sight (`monsters/ai.md` function table, `ai-bodies-*.md`) |
| 6 | `0x005F8D8B` (`0x005F8C80`, UberIzual) | `monsters/ai-bodies-7.md` §25 |
| 0x804 | `0x00622CD9` (`0x00622C40`) | melee range (`combat/hit.md` §7.2) |

### 16. Line test (`0x0064E260`)

Arguments: room `R`, from `(x0, y0)`, to `(x1, y1)` (sub-tiles), mask
`m`; result 0 (clear) or 1 (blocked; the stop cell is written back to
"to"). No owner spec had it; the line-of-sight users (`missiles/`,
`skills/`, `monsters/ai*.md`) cite it.

1. `R` null → blocked. If `(x0, y0)` is outside `R`'s sub-tile rectangle,
   `R` := the room containing it among `R` and its adjacent rooms
   (`0x00463740`); none → blocked.
2. Cell test: `mask(x, y) & m` ≠ 0 in the collision grid of the current
   room (`drlg/rooms.md` §10.3).
3. `dx` = |`x1 − x0`|, `dy` = |`y1 − y0`|, `sx`, `sy` = sign (+1 for 0).
   - `dx` = `dy` = 0: the one cell.
   - `dx` = 0: cells `y0`, `y0 + sy`, …, `y1` (`dy + 1` cells) at `x0`.
   - `dy` = 0: likewise along x.
   - `dx` < `dy`: per cell on y: test; y += `sy`; `e` += `dx`; when `e` ≥
     `dy`: `e` −= `dy`, x += `sx` (`e` starts at 0); ends after the cell
     of row `y1`.
   - `dx` ≥ `dy`: the same with x and y exchanged; ends after column `x1`.
4. Leaving the current room's rectangle switches to the adjacent room
   containing the next cell (`0x00463740`); none, or that room does not
   contain it → blocked at that cell.
5. The first cell that tests non-zero → blocked; reaching the end →
   clear.

The error term starts at 0, not at half the major distance, so the
minor coordinate steps late (a line from (0, 0) to (4, 1) tests (0, 0),
(1, 0), (2, 0), (3, 0), (4, 1)).

## Constants & data dependencies

Pools 256 × 0x28, 512 × 0x18, 128 × 0x18; cycle tables (7,500, 250,
3,000 (× 3 in acts III, V), 125) + `roll` (7,500, 250, 3,000, 50); peak
32 + `roll(224)`; splash gate 3 updates, bubble gate 25 updates; water
roll 1,000, bubble threshold 100; lightning flash 3 updates, wait 500 +
`roll(1500)`, flash gate > 9 frames per second; star tick 40 ms, 256
stars, 8 colors; summit anchor 10,063, tile 256, cloud wrap 368, 10
clouds; edge margins 30 / 25. Tables: `levels` `Rain`, `Mud`,
`DrawEdges`, `LOSDraw`. Level ids 74, 109–112, 117, 120, 121.

## Randomness

All weather draws use the **local player unit's seed** (`unit +0x20`;
`roll_range` `0x00472280`, raw steps inline), the same seed the screen
shake (`camera.md` §8) and the cursor (`capture.md` §3.3) step. Per
weather update (rain on): particle moves (`0x004732C0`, no draw except
spawns), rain cycle (`roll_range` at a phase change), spawns (rain: 3
`roll_range` + 2 raw steps each), wind retarget (2 raw steps); per drawn
frame: one `roll_range(0, 1000)` per drawn water floor plus 1–3 per
spawned splash or bubble (splash + 1 raw step); lightning (§11.7).
Levels 74 and 120 use their own seeds (§12), not the player's.

## Edge cases & original bugs

- The water-floor draw happens in every frame and every level with water
  floors (rain off too): drawn floors step the player's seed even in the
  Rogue Encampment without rain effects.
- Splash and bubble positions are screen pixels corrected by the last
  drawn frame's camera delta once per update: with several frames per
  update the earlier deltas are lost, so pools drift against the map.
- The lightning flash is skipped below 10 frames per second.
- The flash rectangle never covers the last column and row (rectangle
  clip, `blend-modes.md` §8).
- Snow flake `s` reads table entries `s` and `s + 1` (overlapping 16-byte
  entries).
- Arreat Summit's reference x is the constant 10,063 (the captured player
  x is overwritten).
- Pass 8 is unreachable (§13).

## Test vectors

| Input | Expected | Source |
|---|---|---|
| line test (0, 0) → (4, 1), empty grid | cells (0,0), (1,0), (2,0), (3,0), (4,1); 0 | §16 |
| line test (0, 0) → (1, 4) | (0,0), (0,1), (0,2), (0,3), (1,4) | §16 |
| sight: `a` (10, 10) size 2, `b` (12, 11) size 1 | `dx + dy` = 3, not < `sa + sb` = 3; `dy` < `dx`: ax 10 + 2 = 12, bx 12 − 1 = 11; line test (12, 10) → (11, 11), mask 2 | §15 r2 |
| sight: `a` (10, 10) size 2, `b` (11, 11) size 1 | `dx + dy` = 2 < 3 → passes, no line test | §15 r2 |
| sight in level 1 (`LOSDraw` 0) | every unit passes | §15 r1 |
| water floor drawn, rain off | 1 draw of the player's seed, no spawn | §11.5 |
| splash spawn, `a` = 0 | `b` = 0: no x draw, x = X, y = Y | §11.5 |
| target 128 (intensity 0.5) | `k` = 500: `r` = 499 spawns, 500 does not | §11.5 |
| target 32 / 224 / 255 | `k` = 125 / 875 / 996 | §11.5 |
| rain spawn, `H` 600, `g` = 296 | +0x10 = 4 + ⌊2048 / 513⌋ = 7; +0x14 = 15 + ⌊3840 / 513⌋ = 22 | §11.4 r5 |
| snow spawn, `H` 600, `g` = 296, no bump | `s` = 1792 / 513 = 3; +0x14 = 8 + 13 = 21 | §11.4 r5 |
| snow color, `s` = 5, day period 2, video mode 3 | `i` = 60 / 8 = 7 → `S[7 / 4]` = `S[1]`; alpha 80 | §11.4 r5 |
| rain color tables, `i` = 11 | `[0x007A8980]`: `v` = 98 − 73 = 25 → `nearest(25, 50, 25)`; `[0x007A89A4]`: `v` = 3 → `nearest(3, 8, 3)` | §11.4 r5 |
| star re-make, step & 7 = 3, `W` 800 | x = 802 | §12 |
| summit, player client x 10,063 + 2,056 | `q` = 257, `c` = 1, `t` = 1, x0 = −1: frames 1, 2, 3, 0 at −1, 255, 511, 767 | §12 |
| flash, frame rate 9 / 10, open mode 0, 800 × 600 | no rectangle / columns 0–798, rows 0–552 set to 255 | §11.7, `blend-modes.md` §8 |
| capture `weather-0001`: Rogue Encampment in rain, run 2 frames with recorded player seed | pass 4 / 9 pixels equal | capture, queued |

## Provenance

1.14d `Game.exe`: pools `0x00472320`, `0x006BCA10`, `0x006BCAC0`,
`0x006BCB40`, `0x006BCCD0`; update `0x00473F50`, `0x004732C0`,
`0x00472C80`, `0x00472D10`, `0x00473E50`, `0x00473D00`, `0x004737B0`,
`0x00473090`, `0x00472610`, `0x00472400`, `0x004726F0`, `0x00472590`,
`0x00472890`; floor context `0x004DE730`, floor draw `0x004DE410` (call
sites `0x004DE5C3`–`0x004DE60C`), spawns `0x00472DA0`, `0x00472EC0`; pass
4 `0x00473C00`, `0x00473A70`; pass 9 `0x00473910`, `0x00473470`; frame
rate `0x00477980`; lightning start `0x00472C50`; backgrounds
`0x00476290`, `0x00476190`, `0x00476460`; pass 8 `0x00475B20`,
`0x004769B0`, `0x004F68B0`; edge floors `0x004DE6C0`, `0x004DE630`,
`0x00619720`, `0x00643260`; sight `0x004DC710`, `0x00642840` (leveldefs
+0x98 = `LOSDraw`, `data/fields.tsv`), `0x00622AA0`, `0x00622920`,
`0x0064E260`; §15.1 (2026-10-07) also `0x0045ADF0`, `0x0045AE20`,
`0x00620510`, the register order of the `0x00622920` call, and the 16
call sites with their pushed masks (`disasm.py xref`, `all.asm`).
Register arguments (`roll_range` min in EDX, seed in ECX)
read from the disassembly, not the decompile. 2026-10-07 (answers
W1–W7): `0x004DE730` (`fmul [0x006DB9D0]` before `0x00682FD0`, the
`last_s` store before the `k` test), `0x00473F50` (rain off: `fldz` into
the intensity), `0x00473D00` (fatal 0x12A, lock test), `0x00473090` /
`0x00472FB0` (spawn steps, constants `0x006D6E48`–`0x006D6E58` and alpha
bytes `0x006D6E40` read from the file), the color loop of `0x00472890`
(`0x004FB180` → `0x00605210` over `0x0081E668`), `0x00473470` (snow
table `0x006D6E78` read from the file, landed and falling lines),
`0x0040B330` / `0x0040B350`, `0x00473910` (flash test `jb` 10); no
store to `0x007A8A20` anywhere in `all.asm`; section layout from the PE
header. Level lists from
`patch_d2` `levels.txt`. Rain seen in run 2 (`20261006-141725`, Rogue
Encampment): splash ripples on the river, drop lines.
- 2026-10-09 (pc1-day3-c, REC-576 (6)): `[0x007BB390]` polled with no breakpoints on Windows (scratch probe on `autostart.py`'s hook-free debugger), §11.7 r2.

## Open questions

1. A capture with the player seed recorded per frame and the weather
   state at frame end (`capture.md` §3.4 lists rain / snow flags and the
   lightning countdown; add the three pools) checks §11 pixel for pixel.
2. The act edge record (act `+0x18`, 0x30 bytes): who fills it and with
   which DT1 tile; Ghidra search for writes of act `+0x30`.
   *Answered* (static): act allocation `0x006194A0` passes act +0x18 to
   `0x00642A30(drlg, record)` right after the environment
   (`drlg/levels.md` §2 rule 3). It zeroes the 0x30 bytes, looks up
   (`0x00604AE0`, `drlg/rooms.md` §9.3, at most 40 entries) the key
   (orientation 0, main, sub) in the act's base tile library (drlg +0x0C,
   `levels.md` §3 step 5) by act (drlg +0x480): Act I (0, 0, 0), Act II
   (0, 0, 1), Act III (0, 29, 12); no match is fatal 0x44C. The first
   entry of the result is written into the record by the tile-record
   fill `0x0066DDE0` (position arguments 0). Acts IV and V do no lookup:
   their record stays zero (no tile; the base libraries of acts 4–5 are
   empty). No draw on any seed.
3. Partly answered (`impl-draw-order-2` W2–W5): f₁, f₂, f of §11.4 r5,
   the drop vector of §11.7 r3 and the splash threshold of §11.5 are
   now exact. Open: the particle move `0x004732C0` (its FPU values from
   `0x0040B330` / `0x0040B350`); an asm read of it settles it.
   *Answered* (static, asm of `0x004732C0`, `0x0040B330`, `0x0040B350`;
   constants read from `Game.exe`): §11.9.
4. Which event calls `0x004E3C50` (lightning start, sound flag 0) — a
   table-dispatched client handler; search the pointer tables for it.
   *Answered* (static): its only reference is entry 84 (`0x00727CF8`) of
   the client skill function table `0x00727BA8` (entry 30 = `0x004F3530`,
   `lighting.md` §9.2), the table both `cltdofunc` and `cltprgfunc1` index.
   `0x004C6930` calls entry `cltprgfunc1` (skills +0xF6) of the skill it
   is given; its caller is the S→C 0xA3 handler `0x0045D5E0`
   (`audio/triggers.md`, rule "Skill do / target"). The only 1.14d skill with
   any client function 84 is Thunder Storm (57, `cltprgfunc1` 84; patch_d2
   `skills.txt`), so lightning flashes start from a Thunder Storm
   progress event; validation inside `0x004E3C50`: skill id in 0 …
   count − 1.
5. *Answered* (`impl-draw-order-2` W1): `[0x007A8A20]` has no writer
   (no store to it in `Game.exe`; `.bss`), so it is always 0 (§11.1,
   §11.3).
6. *Answered* (W2): the snow spawn's extra step is a second raw step
   after the phase step, value unused; `0x00472FB0` draws nothing and
   also sets the alpha (§11.4 r5).
7. *Answered* (W3): the snow line table `0x006D6E78` is listed in §11.7
   r3 (9 entries).
8. *Answered* (W4): the color-table ramps are in §11.4 r5; alpha 0x7F
   only for rain in day period 0 (never read by the GDI line); a landed
   drop uses color +0x24.
9. *Answered* (W5): the spec misread the threshold. `0x004DE730`
   multiplies the intensity (target / 256) by 1000.0 (`0x006DB9D0`)
   before truncating, so `k` = ⌊target × 1000 / 256⌋ (125–875 for rain
   peaks 32–224) and `r` = `roll_range(0, 1000)` < `k` spawns a splash,
   consistent with the splashes of run 2 (`20261006-141725`). Also
   corrected: `last_s` advances whenever `c` > `last_s` + 3, and rain
   off sets the intensity (not the target) to 0 (§11.2 r3, §11.5).
10. *Answered* (W6): below 10 frames per second the flash is skipped and
    the particles draw in that frame (§11.7 r2).
11. *Answered* (W7): phase, length, countdown, `last_s`, `last_b` are 0
    at program start and never reset, so the rain cycle continues across
    games (§11.1).
12. **Answered** (2026-10-09, asm read, pc1-data Step 4 item 19; §12
    l74 r1 / r3, l120 r3): `last` = `[0x007B57E0]`, 0 before the first
    call (zero-filled), r3 runs on the first call; seeds are `init_low`
    of `time_value(v)` with v = the shake start `[0x007B8D10]` (74) and
    `GetTickCount()` (120). Settles REC-420's argument / form; the cloud
    and star layouts stay time-seeded (compare only with the recorded
    seed). Was: (Added 2026-10-09, `q-fix-render-rest`; blocks level 74
    in d2rs.) §12 r3's star tick `last`: its global, its value before the first
    call of `0x00476290` (a `.bss` 0, or a write in the first-call branch
    of r1), and whether the r3 test runs on the first call. Also §12's
    two "seed := time value": the argument of `time_value`
    (`sim/rng.md` §5.1) and the seed form (`init_low(x)` or the raw word)
    for `[0x00712C4C]` and `[0x00712C50]` (d2rs reads `init_low(
    time_value(0))`, PROVISIONAL REC-420). Settled by an asm read of
    `0x00476290` / `0x00476460`'s first-call branches (local queue,
    `docs/HANDOFF.md` §5 entry 103).
