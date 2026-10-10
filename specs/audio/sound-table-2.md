# Spec: Audio — Sound table part 2 (other users of the sound seed; options sliders)

- **Status:** draft: static answer to `audio/sound-table.md` open
  question 3, from the 1.14d `Game.exe` call graph and disassembly
  (addresses inline); not yet compared with a seed trace. Part of
  `audio/sound-table.md`, split off to keep that spec under 60 KB (its
  §1–§13 are claimed by code, so no section was moved; this part adds
  §14). §15 (fourth pass): static answer to `audio/sound-table.md` open
  question 9 (options-menu slider mapping), from the option records
  and handlers read in the image. §16–§17 (fifth pass): the sample
  cache's exact use order, load, eviction and unload, and the start
  failures after a slot is taken (refining part 1 §7 and §10).
- **Target version:** 1.14d
- **Crate/module:** `d2-client::audio` (variant picks, `client/audio.md`
  §A3) and every client system listed in §14.3.
- **Related specs:** `audio/sound-table.md` (part 1; §4 r5 is the draw
  helper, §6.1 the sound tick; its Constants, Edge cases, Provenance and
  open questions cover this part too), `sim/rng.md` §2–§3, §7,
  `render/capture.md` §3.3, `render/camera.md` §8–§9,
  `render/draw-order-2.md` §11, `render/lighting.md` §10,
  `audio/triggers.md` Randomness, `audio/environment.md` §7.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 36–46 |
| Rules | 47–48 |
|   14. Other users of the local player's client unit seed | 49–195 |
|   15. Options-menu sliders (`audio/sound-table.md` open question 9) | 196–258 |
|   16. Sample cache, exact (refines `audio/sound-table.md` §10 r2–r6) | 259–309 |
|   17. Start failures on a channel (refines `audio/sound-table.md` §7) | 310–329 |
| Test vectors | 330–352 |
| Provenance | 353–385 |
<!-- /index -->

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
| receive | placement of the local player `0x004654C0` → `0x00472C20(flag)` (`0x0046569F`) → `0x004726F0(flag)`, only for flag ≠ 0: S→C 0x15 with u8@0xA ≠ 0 (`0x0045D199`); the callers `0x00460FD8` and `0x004C8C4B` pass 0 | outside act V 3 (`0x00472610`); act V 3 + 1, and `0x00472400` (1 `roll_range`) under the snow lock | `render/draw-order-2.md` §11.8 |
| draw (first frame after a loading screen) | act load `0x004547B0` (`0x0044C999`) → `0x00472890` → `0x004726F0(1)`, then once per process `0x00472610` | 3 (act V as above) + 3 once | `render/draw-order-2.md` §11.8 |
| receive / update | Den of Evil lights `0x0046AF70` (rooms of level 8) | 2 per try, up to 25 tries | `render/lighting.md` §10 r1 |
| receive / update | overlay create `0x00470390(unit, overlay, type, a, …, b)` for **any** unit's overlay | type 6: 1 `roll(frames)` (start frame, +0x18); a ≠ 0: 1 `roll(a × 256)` (+0x18); b ≠ 0: 1 `roll(b × 16)` added to +0x14 (`0x004704DD`, `0x004705B6`, `0x004705D7`). Callers that can pass these: item `0x004C1B48` (type 6, a = 8), skill `0x004C661C`, states `0x004D95B0`, `0x004D961A`, `0x004D9A92`; the other 58 call sites pass type ≠ 6 and a = b = 0 | none yet (`render/unit-composite.md` names the creation link only) |
| update | client monster class 344 at creation (`0x004AE4F0`, `0x004AE567`) | 1 (path direction := lo' & 0x3F) | `client/msg-units.md` |
| update | `0x004A3150` (pointer from `0x004BDE40`), only while byte `[0x007C025F]` ≠ 0 | 1 (lo' mod 3 + 2 client monsters) | `world/objects-client.md` §26.17 |
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

#### 14.5 Seed at the first sound tick

1. **Which seed** (1.14d-confirmed, read 2026-10-10, PC 1 today): the
   variant picks (footsteps `0x004CAF60`, warcry and every other start,
   `audio/sound-table.md` §4), the greeting picks and the ambience
   event draws (`audio/environment.md` §7) all go through `roll`
   `0x004E40A0`, which steps unit `[0x007A6A70]` + 0x20 / +0x24: the
   same local player seed the weather (`0x00473F50` passes unit + 0x20)
   and the cursor step. There is no second sound or weather seed.
2. **Its value**: `client/model.md` Randomness rule 4 (owner): `S[16]`
   = {0xE4CA4C4E, 0x3A4FDE2B} at the first sound tick (T 0, C 1) of the
   first game of a process that starts in the Rogue Encampment, for
   every game seed; the weather values drawn on the way are
   `render/draw-order-2.md` §11.3. In the Rogue Encampment (`SoundEnv`
   1: no event; bed 70, river 2,599, torch 2,578, rain 64 all `Group
   Size` 0) the T 0 tick draws nothing, so the first sound draw is the
   first NPC footstep (2,768, `Group Size` 4; T 27 in
   `traces/audio/win/audio-town-ambience-ama.orig-win.jsonl`), with the
   seed advanced by the weather and cursor steps of the frames between.
3. **Confirming recording** (REC-2439; one run, ScnAma `-seed 1234`,
   sound on, fresh process, no mouse input). At each hook log C
   `[0x007A0498]`, T `[0x007BC9BC]`, and the seed {u32 at P + 0x20, u32
   at P + 0x24} with P = `[0x007A6A70]` (skip while P = 0):

   | Hook (execute) | Also log | Expected |
   |---|---|---|
   | `0x00460D12` (player init, before the step) | ESI = new unit, EBX = local player; seed of ESI | {1, 666}; after the step (`0x00460D3E`) `S[1]` |
   | `0x0045D160` entry and `0x0045D19E` (S→C 0x15) | ECX = message at entry, byte [ECX + 0xA] | flag 1; `S[1]` → `S[4]` |
   | `0x00472610` entry | return address [ESP] | 4 calls before T 0, return addresses `0x0047277F` (0x15), `0x0047277F` (act load), `0x004728B1` (once per process), `0x0047277F` (phase 1), at `S[1]`, `S[4]`, `S[7]`, `S[11]` |
   | `0x0044C990` entry (frame) | `[0x007A2888]` (loading cel ≠ 0 = act load pending) | first frame: C 1, `S[4]` |
   | `0x00473E50` entry and its `ret` | phase `[0x007A8A24]`, countdown `[0x007A8A3C]`, length `[0x007A8A38]`, peak `[0x007A89C0]`, target `[0x007A89E0]` | first: 0, 0 at `S[10]`; at return phase 1, length 498, countdown 497, peak 255, `S[15]` |
   | `0x00473090` entry (particle spawn) | EAX = seed pointer | first at C 2 |
   | `0x004681C0` entry (cursor step) | state `[0x007A6AF0]`, `GetTickCount` | first at C 1, state 1, `S[15]` → `S[16]` |
   | `0x00482C20` entry (sound tick, call `0x0044F2B5`) | — | T 0, C 1: `S[16]` |
   | `0x004E40A0` entry and `ret` | ECX = n, return address [ESP], EAX at return | none at T 0; first from the footstep pick |

   The seed at each sound-tick entry and before each `roll` is the
   input a d2rs conformance run needs when frames are dropped or the
   cursor's wall-clock tests differ (§14.4).

   *Recorded (2026-10-10, PC 1 today, Windows, sound on; the last row
   only: a breakpoint at `0x00482C20`, `traces/pc1/client-seed-town-ama.tsv`):*
   the seed at the first sound tick is {0xE4CA4C4E, 0x3A4FDE2B} =
   `S[16]`, as derived; the next entries read {0xE55EB81F, 0x16F60A97}
   (ticks 1–2), {0x740F9FB2, 0x35B12C70} (3–4), {0x8DF2DB76, 0x625E0E95}
   (5): the seed moves only with drawn frames, not with every sound
   tick. The other rows of the table stay PROVISIONAL (REC-2439).

### 15. Options-menu sliders (`audio/sound-table.md` open question 9)

The in-game options menu keeps its entries as 0x550-byte records
(array `[0x007BC940]`, selected index `[0x007BC938]`; static records,
e.g. `Sound` at `0x00716218`): +0x00 kind (0 action, 1 choice, 2
slider, −1 skipped), +0x0C name, +0x110 enabled test, +0x114 apply,
+0x118 init, +0x120 position count n, +0x124 position p. This section
owns how the three audio sliders turn into the §9 settings of
`audio/sound-table.md`; the menu layout and drawing are `client/ui.md`'s.

1. **Audio sliders** (n = 21, positions 0–20):

   <!-- rows -->
   | Entry | Record | Enabled (`+0x110`) | Apply (`+0x114`) | Init (`+0x118`) | Setter |
   |---|---|---|---|---|---|
   | `Sound` (Master Volume) | `0x00716218` | `0x0047CD90` | `0x0047CDA0` | `0x0047CDC0` | `0x00514CD0` |
   | `Music` (Music Volume) | `0x00716768` | `0x0047CDE0` | `0x0047CDF0` | `0x0047CE10` | `0x00514D00` |
   | `3DBias` (Positional Bias) | `0x00717758` | `0x0047CE60` | `0x0047CE70` | `0x0047CE90` | `0x00514D30` |

   `Sound` and `Music` are enabled while the sound device is up
   (`0x004DF880`: `[0x007C8C78]`); `3DBias` only when, in addition,
   the mixer mode is 1 or 2 (`0x004DF980`). A disabled entry ignores
   every input (each handler calls the enabled test first).
2. **Init** (menu open): p := trunc((n − 1) × (v − 0 + 1) / (100 − 0))
   = ⌊(v + 1) / 5⌋ for the stored value v 0–100 (`0x0047CC90`, x87,
   truncating control word 0xC00; the result is exact for every v
   because 20 × (v + 1) / 100 = (v + 1) / 5). Opening the menu writes
   nothing.
3. **Apply** (every position change): v := trunc(0 + (100 − 0) /
   (n − 1) × p) = 5 × p (`0x0047CD00`), then the setter stores v and
   writes it to the settings store (`0x004150E0`, key of §9). So a
   setting is a multiple of 5 once its slider has moved; a stored value
   that is not (e.g. 37 from the settings store) stays as it is until
   then, and shows at p = ⌊38 / 5⌋ = 7.
4. **Inputs** (handler table `0x006D6034`–`0x006D6090`): left arrow
   (`0x0047D9A0`): p − 1, clamped at 0 (unsigned test against n − 1);
   right arrow (`0x0047DA90`): p + 1, clamped at n − 1; mouse button
   down (`0x0047D7F0`): selects the entry under the pointer
   (`0x0047D520`) and starts a drag; while dragging (`0x0047D670`,
   also from the menu update `0x0047E3D0`), with W = screen width
   `[0x0071146C]`, h = W / 2 (truncating) and x0 = h − 133 (h − 48
   when the record's +0x53C is non-zero; the audio sliders have 0):
   x < x0 → p = 0; x > x0 + 265 → p = n − 1; else p =
   trunc(trunc((x − x0) / f + 1.0) / 2) with f = (f32)(265 / (n − 1) ×
   0.5) = 6.625 (`0x006D73F8` 265.0, `0x006CEF10` 0.5, `0x006CEEF0`
   1.0, `_ftol2` `0x00682FD0`), i.e. the nearest of the 21 stops 13.25
   pixels apart. A drag starts only with h − 144 < x < h + 145 (h − 59
   … h + 230 for the other layout).
5. **Sounds**: after an input changes p (compared with the value
   before the input), the apply runs and then 1 `cursor_pass` is
   requested (no unit, delay 0: `0x0047DA69`, `0x0047DB59`,
   `0x0047D7E1`); no change → no apply, no sound. Up / down arrow
   (`0x0047D8A0`, `0x0047D920`: next / previous enabled entry, kinds −1
   skipped, wrapping) request 1 as well (`0x0047D8F6`, `0x0047D971`).
   Enter or a click released on the selected entry (`0x0047DB80`,
   `0x0047D840` → `0x0047D5C0`): kind 1 → p + 1 wrapping to 0 past n −
   1, apply, request 1 (`0x0047D646`); kind 0 → apply, request 2
   `cursor_select` (`0x0047D667`); kind 2 → nothing.
6. d2rs: the settings are integers 0–100; a d2rs options UI that copies
   the original gives 21 stops of 5 and runs the setter on each change.
   The volume chain (`audio/sound-table.md` §8.2) reads the setting
   every update, so a change is heard from the next sound tick.

### 16. Sample cache, exact (refines `audio/sound-table.md` §10 r2–r6)

1. **Use order.** The cache keeps a list of ids (`[0x007BC9B8]`, one
   node per id) in least-recently-used order: every load start (sync or
   async, `0x00482970`) and every use stamp (`0x00482860`: last-use
   tick := T, at a load and at every §6.3 r5 update of a playing
   request) removes the id's node, if any, and appends it at the
   **tail** (`0x00516980`, `0x00516A70`); an unload (`0x004823E0`)
   removes it. The head is the least recently used id.
2. **Load** `0x00482970(id, sync, recent)`. Nothing when the row's
   file failed (+0x81) or it is loaded (state 2). State 1 (async
   pending): sync = 0 → nothing; sync ≠ 0 → finish it now (the collect
   of `audio/sound-table.md` §10 r5, blocking). State 0: open the file
   (path §3; missing → +0x81 := 1, done); size = the file's size; if
   total + size > limit (unsigned) the eviction r3 runs with (need =
   total + size − limit, sync, recent); eviction failed → the stamp
   `[0x007BC9C4]` := T (the "last failed eviction" of §10 r5) and no
   load. Else sync ≠ 0: read the whole file (failure fatal `0x302`),
   total += size, use stamp (r1), state 2, format check
   (`formats/wav.md` §4). Sync = 0: open an async read (no handle →
   nothing), total += its size, size field := it, use stamp, pending
   loads += 1, state 1. Callers: a start (`audio/sound-table.md` §7 r2)
   passes sync = (`Async Only` = 0), recent = 0; the preload (§10 r5)
   passes sync = (T = 0), recent = 1. **Correction** to §10 r5: at T = 0
   the preload loads **synchronously** (the whole file, at once), not
   async; from T = 25 on it starts async reads.
3. **Eviction** `0x004824A0(need, second, recent)`: target = total −
   need (u32; a need above the total wraps, the target exceeds the
   total and the call succeeds at once without evicting). While total >
   target: walk the list from the head; a row is **kept** when (walk 0
   and (lock count > 0 or `Cache`)) or (recent ≠ 0 and T − its last use
   < 750, unsigned) or a channel is playing its id (`0x004DF9D0`); the
   first row not kept is unloaded (r4) and the walk stops there. With
   second ≠ 0 a walk 1 follows from the head with the lock / `Cache`
   protection off (it can unload a second row even when the first was
   enough). If no walk of this round unloaded anything → fail (0); else
   repeat the while test. Success → 1.
4. **Unload** `0x004823E0(id)` (state ≠ 0 only): a playing channel of
   the id is fatal (`0x259`); total −= size; the node leaves the list;
   last use := 0; an async handle is closed (the read is abandoned),
   then sample := 0, state 0. The pending-loads counter is **not**
   decremented for an abandoned async read (original bug, reproduce):
   it counts down only in the collect (`0x00481720`) and is reset only
   by sound init, so each pending read lost to an eviction keeps one
   count until the next game start, and at 15 the T ≠ 0 preload starts
   nothing more.
5. d2rs (no load latency, `audio/sound-table.md` §10 last paragraph):
   the cache bytes are modelled for these rules only; "size" is the
   file's size in the archive listing (the uncompressed WAV size, as
   Storm reports it), so the same evictions happen.

### 17. Start failures on a channel (refines `audio/sound-table.md` §7)

1. After a slot is taken (§7 r3; a steal has already stopped and ended
   its victim): non-stream rows attach the cached sample to the voice
   (`0x005155D0`); failure → the start fails with the slot left free
   (its request pointer was not yet set). Then the slot's request :=
   this one, volume and pan are computed and sent (`0x004DFC20`), and
   the voice plays (`0x005156A0`) or the stream opens and plays
   (`0x00515D70`, path §3, start offset × 4, `Loop`).
2. A play or stream open that fails clears the slot's request again and
   the start fails (`0x004E034D`–`0x004E0351`); the variant pick, the
   history and the overwritten request id stay. A stream failure sets
   no file-failed flag (+0x81 is written only by the load, §16 r2), so
   a `Loop` stream request waits and retries on every later update
   (`audio/sound-table.md` §6.3 r3, with a new variant pick each time
   when its id opens a group); a one-shot is dropped by §6.3 r4.
3. The duplicate test of §7 r1 and the eviction keep test (§16 r3) use
   `0x004DF9D0(id)`: among slots whose voice is playing and whose
   request's current id is id, the one with the smallest start tick.

## Test vectors

| Input | Expected | Source |
|---|---|---|
| sound tick with no starts, ambience off, `soundchaosdebug` off | seed unchanged across `0x00482C20` | §14.1 r1 |
| first sound tick (T 0) of the first game of a process, start in the Rogue Encampment, any game seed | seed {0xE4CA4C4E, 0x3A4FDE2B} (16 steps from {1, 666}); no draw in that tick | §14.5 r2 |
| local player casts a skill with `cltdofunc` 34 | the sound seed steps during the receive of the S→C 0xA3 that starts it | §14.3 r2 |
| item with overlay of type 6, a = 8 created on the client | 2 steps of the sound seed (`roll(frames)`, `roll(2048)`) | §14.3 table |
| menu opened with Music Volume 50 / 100 / 0 / 37 / 4 | slider position 10 / 20 / 0 / 7 / 1 | §15 r2 |
| right arrow on `Sound` at position 20 | position stays 20; no apply, no sound | §15 r4, r5 |
| left arrow on `Music` at position 7 | position 6, Music Volume := 30, then request 1 | §15 r3, r5 |
| drag on `3DBias`, W = 800, x = 267 (h 400, x0 267) | position trunc(trunc(0 / 6.625 + 1) / 2) = 0 | §15 r4 |
| drag on `Sound`, W = 800, x = 300 | trunc(trunc(33 / 6.625 + 1) / 2) = trunc(5 / 2) = 2 → Master Volume 10 | §15 r4 |
| drag on `Sound`, W = 800, x = 533 (x0 + 266) | position 20 | §15 r4 |
| `3DBias` with mixer mode 0 | every input ignored | §15 r1 |
| LRU [5, 9, 7]; id 9 used | [5, 7, 9] | §16 r1 |
| limit 100, total 90, load size 30, sync, LRU [5 (20, unlocked), 7 (25, `Cache`)] | need 20, target 70: walk 0 unloads 5 (total 70), walk 1 unloads 7 (total 45); load: total 75 | §16 r3 |
| same, sync = 0 (async start) | walk 0 only: unloads 5; total 70 → load starts, total 100 | §16 r3 |
| eviction finds nothing to unload at T 400 | no load; failed-eviction stamp 400; the preload starts nothing before T 650 | §16 r2, `sound-table.md` §10 r5 |
| preload at T = 0 of a `Cache` row | synchronous load, state 2 at once | §16 r2 |
| pending 3; one pending row evicted | pending stays 3 | §16 r4 |
| `Loop` `Stream` request whose stream fails to open | fails; waits; tried again next update | §17 r2 |

## Provenance

1.14d `Game.exe` (Ghidra export: `index/calls.tsv` closure of
`0x00482C20`, all 837 step sites of `all.asm` with the multiplier
0x6AC690C5 and every call of the six `sim/rng.md` helpers, each traced
back to its seed pointer with our scratch script and the listed ones
read by hand in `tools/ghidra/disasm.py`); function-pointer tables read
from the image with `pefile` (`0x00727BA8`, 130 entries). Client loop
order `0x0044EFA0` (`0x0044F167`, `0x0044F19A`, `0x0044F28B`,
`0x0044F2B5`). D2MOO not used.
§14.3 placement and act-load rows, §14.5 (2026-10-10, PC 1 today, static
asm / exports): `0x004654C0` (`0x0046569C` flag → `0x00472C20`), its
callers `0x0045D199`, `0x00460FD8`, `0x004C8C4B`; `0x004726F0`,
`0x00472610`, `0x00472890`, `0x004547B0`, `0x0044C990`, `0x00473F50`,
`0x00473E50`, `0x00473D00`, `0x004681C0`, `0x004E40A0`, `0x00460D12`;
recordings `traces/orig-cache/draws-town-arrival-ama` (frame seeds),
`traces/orig-cache/packets-town-arrival-ama` (0x59 at (0, 0), 0x0B,
0x15 flag 1), `traces/audio/win/audio-town-ambience-ama.orig-win.jsonl`
(requests by T); `soundenviron`, `sounds` rows from patch_d2.
§15: option records `0x00716218`, `0x00716768`, `0x00717758` and the
handler table `0x006D6034`–`0x006D6090` read from the image with
`pefile`; `tools/ghidra/disasm.py` of `0x0047CC90`, `0x0047CD00`,
`0x0047CDA0`–`0x0047CE90`, `0x0047D5C0`, `0x0047D670`, `0x0047D7F0`,
`0x0047D840`, `0x0047D8A0`, `0x0047D920`, `0x0047D9A0`, `0x0047DA90`,
`0x004DF880`, `0x004DF980`, setters `0x00514CD0`, `0x00514D00`,
`0x00514D30` (each has no direct caller: only the apply thunks jump to
them).
§16–§17 (2026-10-07, disassembly): `0x00482970`, `0x004824A0`,
`0x004823E0`, `0x00482860`, `0x00481720`, `0x00482B40` (its tail store
`0x00482C0B`), `0x00482260` (counter resets `0x00482293`–`0x004822A2`),
list helpers `0x00516950`, `0x00516980`, `0x00516A70`; start
`0x004E01B0`, `0x004DF9D0`, `0x005155D0`, `0x005156A0`, `0x00515D70`.
