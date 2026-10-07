# Spec: Audio — Sound table part 2 (other users of the sound seed)

- **Status:** draft: static answer to `audio/sound-table.md` open
  question 3, from the 1.14d `Game.exe` call graph and disassembly
  (addresses inline); not yet compared with a seed trace. Part of
  `audio/sound-table.md`, split off to keep that spec under 60 KB (its
  §1–§13 are claimed by code, so no section was moved; this part adds
  §14).
- **Target version:** 1.14d
- **Crate/module:** `d2-client::audio` (variant picks, `client/audio.md`
  §A3) and every client system listed in §14.3.
- **Related specs:** `audio/sound-table.md` (part 1; §4 r5 is the draw
  helper, §6.1 the sound tick; its Constants, Edge cases, Provenance and
  open questions cover this part too), `sim/rng.md` §2–§3, §7,
  `render/capture.md` §3.3, `render/camera.md` §8–§9,
  `render/draw-order-2.md` §11, `render/lighting.md` §10,
  `audio/triggers.md` Randomness, `audio/environment.md` §7.

## Summary

Every audio draw steps one seed: the local player's client unit seed
(unit `[0x007A6A70]` + 0x20, `audio/sound-table.md` §4 r5). Many
non-audio client systems step the same seed. This part lists every
1.14d code path that does, and when each runs relative to the sound
tick, so an implementation knows which draws sit between two sound
draws. Main result: the drawing code steps the seed once or more per
drawn frame, and the cursor steps it on wall-clock time, so the seed
value at a sound draw is not a function of game ticks alone.

## Rules

### 14. Other users of the local player's client unit seed

#### 14.1 Inside the sound tick

1. The direct-call closure of the sound tick `0x00482C20` (791
   functions) contains exactly one seed step site: `roll`
   `0x004E40A0`. Its only indirect calls are the DirectSound COM calls
   of the device layer (`0x00513C50`–`0x00516140`) and fatal-error
   paths, so no other code runs inside the tick. Within one tick the
   draws come in this order: ambience `0x004E42E0` (`0x004E4535`,
   `0x004E4544`, `0x004E458A`, `0x004E45A5`, `0x004E45B4`,
   `0x004E45DB`; `audio/environment.md` §7), then in the request update
   `0x004BA020` the `soundchaosdebug` draw if that switch is on
   (`audio/sound-table.md` §6.2 r3, `0x004BA062`), then the variant
   picks of the starts in list order (`0x00482680` from `0x004E01D0`,
   §4 r3, §6.3). Music `0x004DCAA0`, preload `0x00482B40` and upkeep
   `0x004DF890` do not draw.

#### 14.2 Order within a client loop pass

1. One pass of the client loop `0x0044EFA0` runs, in order: the receive
   `0x0044C6E0` (`0x0044F167`, every pass: S→C handlers), the client
   update `0x0044C790` (`0x0044F19A` / `0x0044F1F6`, passes where a tick
   ran), the in-game draw `0x0044C990` through `[0x007A0484]`
   (`0x0044F28B`, set at `0x0044E300`; not on skipped frames,
   `render/camera.md` §9), then the sound tick (`0x0044F2B5`, passes
   where a tick ran). So the draws between two sound ticks are those
   of the receives of every pass in between, of one client update and
   of at most one drawn frame.
2. Trigger rules draw through `0x004E40A0` outside the tick too
   (footsteps `0x004CAF60`, object `0x004CB460`, attack `0x004CB6A0`,
   flee `0x004CB950`, greetings `0x004E0590`, Init voice `0x004CC380`
   through `0x004E4100`; `audio/triggers.md` Randomness). These are
   sound draws; a hook on `0x004E40A0` sees them. The rows of §14.3
   are the steps such a hook does not see.

#### 14.3 Non-audio users

Explicit users take the unit from `0x00463DD0` or `0x00463DE0` (both
return `[0x007A6A70]`) and step its +0x20 (inline, or through
`roll` `0x0045C3E0` / `roll_range` `0x00472280`, `sim/rng.md` §3).

<!-- rows -->
| Phase | Code (1.14d) | Steps | Owner of the rule |
|---|---|---|---|
| draw | cursor step `0x004681C0` (from `0x00468310` after each cursor draw) | 1 in cursor state 1, only when more than 16 ms of `GetTickCount` time passed since the last step | `render/capture.md` §3.3 |
| draw | screen shake `0x00476D40` (from `0x0044CA03`) | 2 `roll_range` per drawn frame while a shake runs | `render/camera.md` §8 |
| draw | weather update `0x00473F50` (from `0x0044CA5E`): particles `0x004737B0` / `0x00473090`, rain cycle `0x00473E50`, `0x00473D00` | per weather update | `render/draw-order-2.md` §11, Randomness |
| draw | floor drawing `0x004DE410` (`0x004DE5CA`), splash `0x00472DA0`, bubble `0x00472EC0` | 1 `roll_range(0, 1000)` per drawn water floor, plus 1–3 per splash or bubble | `render/draw-order-2.md` §11 |
| draw | lightning `0x00473910` (from `0x00476BC0`, `0x0044CAF8`) | timer and position draws | `audio/triggers.md` §12, `render/draw-order-2.md` §11.7 |
| update | room change `0x004654C0` → `0x00472C20` → `0x004726F0` (1 step and 3 in `0x00472610`), `0x00472400` (1 `roll_range` or 1 step) | per room change of the local player | `render/draw-order-2.md` §11 |
| receive / update | Den of Evil lights `0x0046AF70` (rooms of level 8) | 2 per try, up to 25 tries | `render/lighting.md` §10 r1 |
| receive / update | overlay create `0x00470390(unit, overlay, type, a, …, b)` for **any** unit's overlay | type 6: 1 `roll(frames)` (start frame, +0x18); a ≠ 0: 1 `roll(a × 256)` (+0x18); b ≠ 0: 1 `roll(b × 16)` added to +0x14 (`0x004704DD`, `0x004705B6`, `0x004705D7`). Callers that can pass these: item `0x004C1B48` (type 6, a = 8), skill `0x004C661C`, states `0x004D95B0`, `0x004D961A`, `0x004D9A92`; the other 58 call sites pass type ≠ 6 and a = b = 0 | none yet (`render/unit-composite.md` names the creation link only) |
| update | client monster class 344 at creation (`0x004AE4F0`, `0x004AE567`) | 1 (path direction := lo' & 0x3F) | `client/msg-units.md` |
| update | `0x004A3150` (pointer from `0x004BDE40`), only while byte `[0x007C025F]` ≠ 0 | 1 (lo' mod 3 + 2 client monsters) | none |
| NPC interaction (input and receive paths) | `0x004B1680` (from `0x004B17A0`, `0x004B3E10`; up to 10 `roll`), `0x004B4DB0` (from `0x004B4FD0`, `0x004B6A30`; 1 advance-only step when the dialog with speech 3,386–3,390 opens) | as stated | `world/npc.md`, `ui/menus.md` |
| draw (UI pass `0x00456EE0`) | Nihlathak's hurry-up `0x004B4380` (`roll(30)`, `0x004B44B2`), once per interaction | 1 | `audio/triggers.md` open question 8 |
| receive | S→C 0x59 for the local player | 1 step of the new unit's seed | `client/msg-units.md` |

2. **Skills cast by the local player.** The client skill do functions
   (table `0x00727BA8`, 130 entries, called with ECX = the caster from
   `0x004C6680`, `0x004C6930`, `0x004C6AC0`, `audio/triggers.md` §8 r2)
   step the **caster's** unit seed in these entries, so a cast by the
   local player steps the sound seed: 34 `0x004C9490`, 54
   `0x004E1620` (and `0x004E14D0`), 56 `0x004E1A20` → `0x004E14D0`, 71
   `0x004EFDF0`, 82 `0x004F1430`, 86 `0x004C97C0`, 87 `0x004E3EB0`, 89
   `0x004F0F30`, 5 `0x004F2340` → `0x004C7700`, 24 `0x004E3330` / 77
   `0x004F0B50` → `0x004E31C0`, 32 `0x004F3C80` → `0x004F3A60`, 63
   `0x004E2530` → `0x004C7480`, 90 `0x004F1C80` → `0x004F1A30` (the
   callee gets the caster in ECX, ESI or EAX). Entries 9, 11, 15, 83
   draw on a local seed (`0x00650E40` init), not on a unit seed;
   entries 10, 25 (`0x004C7000`), 38, 40, 85, 93 (`0x004AF890`) and 53
   (`0x004CED60`) step the seed of another unit (a target, a summon or
   a missile), the sound seed only if that unit is the local player.
   Which skills use which entry is `skills.txt` `cltdofunc`.
3. Not users: client unit creation steps the new unit's seed except the
   local player's (`0x00460BF0` skips it, `0x00460D12`); client
   monster, object and missile code steps its own unit's seed
   (`0x0046D780` group, `0x004B13A0`, `0x004BD730`–`0x004BDB00`,
   `0x004CDDB0`–`0x004D8260`); object start frames (`0x00624390`, unit
   type 2) use the object's seed; level 74 / 120 particles use their own
   globals (`sim/rng.md` §5.5).

#### 14.4 Consequence

1. Draw-phase steps depend on how many frames are drawn (frames are
   dropped under load, `render/camera.md` §9) and the cursor step on
   wall-clock time, so in 1.14d the seed at a sound draw, and with it
   every variant, greeting and timer choice of `audio/triggers.md`, is
   not reproducible from the tick sequence alone. A conformance check
   of variant choices must take the seed recorded before each sound
   draw as input (the roll hook of `docs/handoff/local-buddy-q-rec.md`
   entry 74 records it) and compare the chosen value, not the seed
   sequence. d2rs (no shared seed with rendering) draws the sound seed
   only through the §14.1 and §14.2 r2 paths; which non-audio d2rs
   systems share it is the d2rs design's choice (`client/audio.md`
   §A3).

## Test vectors

| Input | Expected | Source |
|---|---|---|
| sound tick with no starts, ambience off, `soundchaosdebug` off | seed unchanged across `0x00482C20` | §14.1 r1 |
| local player casts a skill with `cltdofunc` 34 | the sound seed steps during the receive of the S→C 0xA3 that starts it | §14.3 r2 |
| item with overlay of type 6, a = 8 created on the client | 2 steps of the sound seed (`roll(frames)`, `roll(2048)`) | §14.3 table |

## Provenance

1.14d `Game.exe` (Ghidra export: `index/calls.tsv` closure of
`0x00482C20`, all 837 step sites of `all.asm` with the multiplier
0x6AC690C5 and every call of the six `sim/rng.md` helpers, each traced
back to its seed pointer with our scratch script and the listed ones
read by hand in `tools/ghidra/disasm.py`); function-pointer tables read
from the image with `pefile` (`0x00727BA8`, 130 entries). Client loop
order `0x0044EFA0` (`0x0044F167`, `0x0044F19A`, `0x0044F28B`,
`0x0044F2B5`). D2MOO not used.
