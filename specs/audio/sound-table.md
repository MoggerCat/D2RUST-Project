# Spec: Audio — Sound table (sound id → playable voice)

- **Status:** draft; every rule below names its 1.14d `Game.exe` address
  or live measurement; nothing is confirmed by a trace yet (open
  questions 1–3 say which recordings settle the rest).
- **Target version:** 1.14d
- **Crate/module:** `d2-client::audio` (`SoundTable`, `SoundBank`,
  `VoicePolicy`, `GainCurve` hooks of `client/audio.md` §A2–§A4; §13
  below maps each rule to a hook). The table is parsed with the
  `d2-data` `.txt` reader.
- **Related specs:** `client/audio.md` (design; this spec owns its §B3
  and §B8), `data/loading.md` §3.4 (the two runtime `.txt` files),
  `data/txt-format.md` §5–§7 and `data/field-types.md` §3 (parsing and
  cell types), `data/fields.tsv` row `sounds` (the compile-only name
  linker other tables link through), `formats/wav.md` (WAV decoding; to
  be written), `audio/triggers.md` (when sounds are requested),
  `audio/environment.md` (`soundenviron` meaning, ambience, music),
  `formats/mpq.md` §11 (archive order),
  `sim/rng.md` §2–§3 (generator), `render/camera.md` §9 (client tick).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 49–63 |
| Inputs | 64–74 |
| Outputs / state changes | 75–82 |
| Rules | 83–84 |
|   1. Loading the table | 85–147 |
|   2. Sound environment table (load only) | 148–159 |
|   3. File path | 160–176 |
|   4. Groups and variants | 177–212 |
|   5. Requests | 213–252 |
|   6. Sound tick | 253–338 |
|   7. Starting on a channel | 339–368 |
|   8. Volume and pan | 369–430 |
|   9. Settings | 431–450 |
|   10. Sample cache | 451–476 |
|   11. Live data (1.14d) | 477–493 |
|   12. Edge cases kept | 494–502 |
|   13. d2rs mapping | 503–513 |
| Constants & data dependencies | 514–521 |
| Randomness | 522–529 |
| Edge cases & original bugs | 530–534 |
| Test vectors | 535–570 |
| Provenance | 571–589 |
| Open questions | 590–614 |
<!-- /index -->

## Summary

A sound id is a data-line index into `sounds.txt`. When the game asks
for a sound (`audio/triggers.md`), the client sound layer (1.14d
`Sound\SoundHdr.cpp` and the request layer around `0x004B9A00`) makes a
**request**, picks a variant of the sound's group with the local
player's client unit seed, resolves the file path from the id range,
loads or streams the sample, gives it one of 16 **channels** (stealing a
less important one if needed), and every sound tick recomputes an
integer volume 0–255 and an integer pan 0–255 from distance, falloff,
fades, ducking and the player's settings. This spec owns all of that:
the table's columns, the request and channel state, and the volume/pan
formulas. WAV decoding, the trigger rules and the environment table's
meaning are other specs.

## Inputs

| Name | Type | Source |
|---|---|---|
| `sounds.txt` | 4,699 data lines, 25 columns | P (`patch_d2.mpq`), `data/loading.md` §3.4 |
| `soundenviron.txt` | 50 data lines, 24 columns | P, same |
| sound request | id, unit or none, delay, flags, start offset | `audio/triggers.md` (callers of `0x004B9A00`, 222 call sites in 132 functions) |
| listener | local player unit `[0x007A6A70]` (position, client seed at +0x20) | `render/camera.md` §2 |
| settings | Sound Mixer, Master Volume, Music Volume, Positional Bias | §9 |
| sound tick | counter `0x007BC9BC` | §6.1 |

## Outputs / state changes

Per started voice: file path, start offset, loop flag, loop start; per
sound tick per voice: integer volume (0–255) and pan (0–255, 128 =
centre); stops. These are the values the voice log records
(`client/audio.md` §A5). Side effect: one RNG step on the local
player's client unit seed per variant draw (§4).

## Rules

### 1. Loading the table

`0x00481950` (called from sound init `0x00482260` when the sound
device is up, `0x00513B50`) reads `DATA\GLOBAL\EXCEL\sounds.txt`, then
`soundenviron.txt`, through the data `.txt` parser (`txt-format.md`
§5–§7; record count = data lines). A second call while either table is
loaded is a fatal error (`0x004819xx` tail, messages `0xFC`/`0xFD`).
Free: `0x004828B0`.

1. Records are 142 bytes (`0x8E`), zero-filled, one per data line; the
   **sound id is the 0-based data-line index**. The `Sound` and `Index`
   columns are not read at runtime (live `Index` always equals the line
   index; `Sound` names reach other tables only through the compile-time
   name linker, `fields.tsv` row `sounds`, `loading.md` §3.4).
2. Field list (column → offset, type per `field-types.md` §3). The list
   is in `0x00481950`; it is not one of the 92 lists in `fields.tsv`
   (those are the data loader's), so it lives here:

<!-- rows -->
| Column | Offset | Type | Column | Offset | Type |
|---|---|---|---|---|---|
| `FileName` | 0x00 | `str(59)` | `Cache` | 0x4C | `u8` |
| `Volume` | 0x3C | `u8` | `Async Only` | 0x4D | `u8` |
| `Group Size` | 0x3D | `u8` | `Priority` | 0x4E | `u8` |
| `Loop` | 0x3E | `u8` | `Stream` | 0x4F | `u8` |
| `Fade In` | 0x3F | `u8` | `Stereo` | 0x50 | `u8` |
| `Fade Out` | 0x40 | `u8` | `Tracking` | 0x51 | `u8` |
| `Defer Inst` | 0x41 | `u8` | `Solo` | 0x52 | `u8` |
| `Stop Inst` | 0x42 | `u8` | `Music Vol` | 0x53 | `u8` |
| `Duration` | 0x43 | `u16` | `Block 1` | 0x54 | `i32` |
| `Compound` | 0x45 | `i16` (read signed) | `Block 2` | 0x58 | `i32` |
| `Falloff` | 0x47 | `i32` | `Block 3` | 0x5C | `i32` |
| `Reverb` | 0x4B | `u8` | | | |

   1.14d has no `Function` column (and the parser would ignore one).
3. Runtime fields after the columns (written by the code, never by a
   cell):

<!-- rows -->
| Offset | Size | Meaning | Written by |
|---|---|---|---|
| 0x60 | 4 | group base id (§4 r1) | `0x00481950` |
| 0x64, 0x68 | 4 + 4 | last two variants chosen through this id (newest first) | `0x004E0213` |
| 0x6C | 4 | loaded sample handle (sync load) | `0x00482970` |
| 0x70 | 4 | sample size in bytes (cache accounting) | `0x00482970` |
| 0x7C | 4 | sound tick of last use | `0x00482860` |
| 0x80 | 1 | block count: `Block 1..3` cells before the first −1 (0–3) | `0x00481950` |
| 0x81 | 1 | file open failed: never retried | `0x00482970` |
| 0x82 | 4 | async load handle | `0x00482970` |
| 0x86 | 4 | load state: 0 none, 1 async pending, 2 loaded | `0x00482970`, `0x004823E0` |
| 0x8A | 4 | lock count (preload pin, §10 r4) | `0x004822B0`, `0x00482370` |

4. Record access (`0x00481860`) returns nothing for an id outside
   `0..count−1`; most readers then read through a null record (crash).
   The request entry point rejects ids < 1 (§5 r1), so row 0 (empty
   `Sound`, `none.wav`, all zero) is never played.
5. After the group pass (§4 r1), `0x00481950` scans `soundenviron`
   column `Song` (offset 0, `i32`) over rows with `Song > 0` and keeps
   the minimum and maximum (`0x007BC9B0`, `0x007BC9B4`; read by
   `0x004817F0`, `0x00481800`). Live: 4,657 and 4,684. It allocates one
   dword per song id in that range (`0x007BC99C`, owner
   `audio/environment.md`).

### 2. Sound environment table (load only)

`soundenviron.txt` is parsed in the same call into 88-byte records
(`0x58`), one per data line. All 22 read columns are `i32` except
`Indoors` (`u8`): `Song` 0x00, `Day Ambience` 0x04, `Night Ambience`
0x08, `Day Event` 0x0C, `Night Event` 0x10, `Event Delay` 0x14,
`Indoors` 0x18, `Material 1` 0x1C, `Material 2` 0x20, then the 12
`EAX …` columns at 0x24, 0x28, …, 0x50 in header order. `Handle` and
`Index` are not read. The sound columns hold sound ids (line indices of
`sounds.txt`). What they mean is `audio/environment.md`; this spec uses
only the song range (§1 r5) and `Indoors` (§6.4 r3).

### 3. File path

`0x00482710(id, out)` builds the archive path:

1. Prefix `DATA\LOCAL` if `2934 ≤ id ≤ 4656` (`id − 0xB76 < 0x6BB`,
   unsigned), else `DATA\GLOBAL`.
2. Then `\MUSIC\` if `4657 ≤ id ≤ 4698` (`id − 0x1231 < 0x2A`), else
   `\SFX\`.
3. Then the `FileName` cell as written (backslashes, original case).
4. A path of 0x50 bytes or more is fatal (message `0x3AB`). Unreachable
   with a 59-character `FileName` (longest prefix 17 + 59 < 80).

The id ranges are fixed in code, not taken from the data: a mod that
inserts rows before 2,934 moves speech out of `DATA\LOCAL`. The path is
looked up in the archives in the usual order (`formats/mpq.md` §11;
case-insensitive).

### 4. Groups and variants

1. **Group pass** (`0x00481950`, after loading). Walk ids 1 to
   count − 1 with a current group (start `s`, size `g`; none at first).
   For each id `i`: its group base (+0x60) is `s` if a group is current
   and `s ≤ i < s + g`, else `i`. Then if its `Group Size` is non-zero
   it opens a new group (`s = i`, `g` = that size), else its `Group
   Size` is overwritten with 1. Row 0 keeps base 0 and size 0. A row
   with a non-zero size inside an earlier group gets the earlier base
   and opens its own group from there (7 live cases, e.g.
   `event_kurast_night_2` = 134 inside 133's group of 5).
2. **Block count** (+0x80, same pass): the number of `Block 1..3` cells
   before the first −1.
3. **Variant pick** (`0x00482680(id)`, called at start unless request
   flag bit 0 is set, §5 r2): with `n` = this id's `Group Size` (after
   r1):
   1. `n ≤ 1`: the id itself, no draw.
   2. `k = min(n − 1, 2)` = how many recent picks to avoid.
   3. If `n ≤ 3`: draw `roll(n + 1)`; if it is 0, `k −= 1`.
   4. Repeat: `v = id + roll(n)`; accept when `v` differs from the
      first `k` entries of this id's history (+0x64, +0x68). No retry
      limit.
   The variant is relative to the requested id, not the group base.
4. **History** (`0x004E0213`, at every start attempt that passes the
   duplicate check of §7 r1): on the *requested* id's record, +0x68 ←
   +0x64, +0x64 ← the chosen id. Histories start at 0.
5. **RNG.** `roll(n)` here is `0x004E40A0`, a third copy of `sim/rng.md`
   §3 `roll` (`n < 1` → 0 without a step; power of two → mask; else
   unsigned mod). It steps the seed at **local player unit + 0x20**
   (`[0x007A6A70]`, `0x00463DD0`): the player's client unit seed, which
   client particle and effect code also use (`sim/rng.md` §5.3, §7). The
   same picker is used by the NPC greeting code `0x004E0590`
   (`audio/triggers.md` §10 r1), which retries up to 20 times while the
   pick equals that NPC's last greeting; ambience cues use the same
   seed through their own draws (`audio/environment.md` §7).

### 5. Requests

A request is the game-side record of one sound use. Pool: 200 slots of
89 bytes at `0x007C0ED0`, allocated in slot order (`0x004B93F0`; full
pool → no sound), active list at `0x007C5458`. Fields used here: +0x04
id, +0x08 handle (from counter `0x007C5460`, pre-incremented), +0x0C /
+0x10 / +0x14 position x / y / z (f32, §8 r1), +0x18 distance² (f32),
+0x1C volume 0–255, +0x20 occlusion (f32), +0x24 start offset, +0x28
unit list, +0x2C channel, +0x30 state (0 waiting, 1 playing, 2 ended),
+0x34 start tick, +0x38 flags, +0x3C priority (u8), +0x3D stop flag,
+0x41 resume offset, +0x45 fade active, +0x49 / +0x4D fade start /
end volume, +0x51 / +0x55 fade start / end tick.

1. **Request** (`0x004B9A00(id, unit, delay, flags, offset)`): nothing
   (returns 0) if the sound system is off (`0x007C545C`), `id < 1`, or
   the record's `Volume` is 0 (70 live rows).
2. **Compound** (`Compound ≠ 0`, `0x004B9760`): if an active request
   whose id has the same group base is not stopping and (`Compound < 0`
   or `now − its start tick ≤ Compound`), no new request is made: the
   call returns that request's handle (after r4).
3. Otherwise a new request: flags (bit 0 = exact id, no variant; bit 1
   = no fade-in), offset, id, start tick = now + delay, volume 255,
   priority = the record's `Priority`. With a unit: position and
   distance² from the unit (§8 r1) and initial occlusion (§6.4 r2);
   without: position (0, 0, 0).
4. If the unit is the local player, the request's priority gets +80,
   wrapping as a byte (255 → 79). On the compound path this adds 80 to
   the *existing* request again on every merged call. Reproduce.
5. **Fade / stop** (`0x004B9EF0(handle, target, delay, len)`): when the
   request is not yet on a channel with a running fade to the same
   target: `target = 255` raises `len` to at least `Fade In`; `target =
   0` raises it to at least `Fade Out`. `len > 0` sets a fade from the
   current volume to `target` from `now + delay` to `now + delay + len`;
   a fade to 0 also sets the stop flag. `len = 0` with `delay ≠ 0` is
   fatal (`0x25C`); `len = 0`, `delay = 0` sets the volume to `target`
   at once (`0x004B9B50`, which only writes +0x1C; a target of 0 has
   already set the stop flag, so the next update stops it). Nothing
   happens when the request has more than one unit attached
   (`audio/triggers.md` §1 r3).

### 6. Sound tick

#### 6.1 Time base

`0x00482C20` runs one sound tick: environment/cache work, preload
(§10 r3), the request update `0x004BA020(now)`, channel upkeep, then
`0x007BC9BC += 1`. The client loop `0x0044EFA0` calls it at
`0x0044F2B5` on passes where a client tick (40 ms, `render/camera.md`
§9) ran, and at `0x0044F01D` on another path (open question 4). All
tick counts in the table (`Fade In`, `Fade Out`, `Duration`,
`Compound`) are in these ticks.

#### 6.2 Order

1. Each update first bubble-sorts the active list (most important
   first): higher priority; then smaller distance²; then later start
   tick; then higher slot address (= higher pool slot index). The same
   order decides channel stealing (§7 r3).
2. Requests with the stop flag and no fade (or a fade to non-zero):
   waiting → ended; playing → its channel is stopped.

#### 6.3 Per request, in list order

`audible` = Master Volume > 0, except for ids in the song range (§1 r5)
when Music Volume is 0. `max` = the falloff maximum (§8 r2).

1. **Ended** (state 2): if `Loop` and `Duration = 0` and no stop flag,
   it waits again (state 0; a pending fade's end volume becomes its
   volume); else it is freed.
2. **Fade**: while active, volume = start + (end − start) × (now −
   t0) / (t1 − t0) (signed 32-bit, truncating); before t0 the start
   volume; after t1 the end volume and the fade ends.
3. **Start**: a waiting request with `now ≥ start tick`, volume ≠ 0,
   `audible` and distance² ≤ `max²`:
   1. Instance rules. If `Defer Inst` or `Stop Inst` is set, find the
      playing request of the same group base with the smallest start
      tick (other than this one; with exactly one unit, only those of
      that unit) (`0x004B9690`). For ids 2934–4656 (speech) with
      exactly one unit, instead find any playing speech request of
      that unit (`0x004B9700`), whatever the flags.
   2. If one was found and (`Defer Inst = 0`, or the request has a
      unit list, or its position is (0, 0, 0)): with `Stop Inst = 0`
      this request gets the stop flag and is not started; with `Stop
      Inst = 1` the found one gets the stop flag and this one starts.
      (A positional request without units and with `Defer Inst = 1`
      starts alongside.)
   3. Fade-in: a stored resume offset (+0x41, §7 r3) moves to the start
      offset and gives a 3-tick fade-in; else `Fade In > 0`, no running
      fade and flag bit 1 clear give a `Fade In`-tick fade-in. A
      fade-in starts the voice at volume 0 and fades to the previous
      volume.
   4. Start on a channel (§7). Success → playing.
4. A due request (`now ≥ start tick`, volume ≠ 0) that has no channel
   after r3, whether it failed or was not audible or out of range, and
   is not `Loop`, is queued for removal (`0x00516950`, list emptied at
   `0x00516A10`), unless it is `Async Only` and its sample is still
   loading (state 1). One-shots never wait for a channel or for range.
5. **Playing**: stopped when `Duration > 0` and `now − start tick >
   Duration`, or not `audible`, or distance² > `max²`; else volume and
   pan are recomputed (§8) and a loaded sample's last-use tick is set.

#### 6.4 Tracking and occlusion

1. When `Tracking` is set, an option flag (`0x007A061C`) is on and the
   request has units, each tick the position becomes that of the
   nearest unit and the occlusion target is the mean of the units'
   values (r2); occlusion moves toward it by at most 0.05 per tick.
2. A unit's occlusion value: 0 if `Falloff = 4`; for group base 202
   (`event_thunder_*`) 0.5 if the current environment is `Indoors`,
   else 0; otherwise 0.5 if `0x00622AA0(player, unit, 2)` is non-zero,
   else 0 (`0x004B9890`).
3. Occlusion reaches the output only as the buffer's occlusion value
   (`0x00515A90`); in mixer mode 0 its effect is open question 5.

#### 6.5 Ducking

At the end of the update, if the tick advanced:

1. **Solo duck** `0x00727568` (70–100, start 100): −2 per tick while any
   active request whose record has `Solo` is not fading to 0; +2 per
   tick otherwise. Non-`Solo` voices are scaled by it (§8 r3).
2. **State duck** `0x00727564` (0–100): −5 per tick while the
   condition at `0x004BA640` (`0x0044DB30`, `0x00453A90`) holds, +5
   otherwise (open question 6). Scales every voice except ids 1–15,
   52–71 and 4657–4698.

### 7. Starting on a channel

`0x004E01B0(request)`:

1. Pick the variant (§4 r3) unless flag bit 0. If a channel already
   plays this exact id, its request is not stopping, its start tick is
   0 or 1 ticks before this one, and its priority ≥ this one's, fail
   (duplicate suppression).
2. History update (§4 r4). Unless `Stream` or already loaded, load
   synchronously (`Async Only = 0`) or start an async load (§10). Not
   loaded now, or the file failed (+0x81) → fail.
3. **Channel.** 16 slots of 32 bytes at `0x007C8A78`, each of a fixed
   kind (2D; stereo; 3D; EAX: set by the mixer mode, `0x004E0050`). The
   request's kind: `Stereo` → stereo; else by mixer mode (§9). Take the
   first idle slot of that kind. Else take the least important busy
   slot of that kind (§6.2 r1 order) and steal it if this request's
   priority is higher, or equal with a higher pool slot than the
   victim's (`0x004B9DA0` with distance and time keys off): the victim's play position is
   saved as its resume offset (+0x41), its channel stopped, its state
   set to ended (it restarts from there if it loops, §6.3 r1, r3.3).
   No slot → fail.
4. **Set up** (non-stream): loop flag = `Loop`; if the block count is
   exactly 1, loop start = `Block 1` × 2 bytes (16-bit sample frames;
   `Block 2/3` unused here); sample data from the cache. Mixer mode 2
   only: reverb send 1.0 if `Reverb ≠ 0`, else 0.0. Then volume and pan
   (§8) and play.
5. **Stream** (`Stream = 1`): path (§3), start offset (+0x24) and
   `Loop` go to the stream player (`0x00515D70`); no cache. Whether
   `Block 1/2` reach it is open question 7.

### 8. Volume and pan

#### 8.1 Position and distance

1. With a unit (`0x004B97D0`): from the client positions of player
   `P` and unit `U`, `x = U.x − P.x`, `y = 2 × (U.y − P.y)` (f32),
   `z = 0`. Distance² for ordering and range = clamp(x, ±2000)² +
   clamp(y, ±2000)² (`0x004B98F0`). Sound 2599 (`object_river`) uses a
   projected position (open question 8).
2. **Falloff** (`0x004825B0` min, `0x00482610` max):

<!-- rows -->
| `Falloff` | min | max | live rows |
|---|---|---|---|
| 0 | 60 | 400 | 62 |
| 1 | 60 | 700 | 2,777 |
| 2 | 200 | 1,000 | 132 |
| 3 | 400 | 1,500 | 5 |
| 4 | 2,000 | 2,000 | 1,723 |
| other | 60 | 700 | 0 |

#### 8.2 Volume chain (`0x004DFC20`)

Integer, each division truncating toward zero:

1. `v` = request volume (fade applied).
2. `Music Vol` set: `v = MusicVolume × v / 100`.
3. `v = MasterVolume × v / 100`.
4. State duck (§6.5 r2), if ≠ 100 and the id is not exempt:
   `v = duck × v / 100`.
5. Solo duck, if ≠ 100 and the record is not `Solo`: `v = duck × v /
   100`.
6. Modes 1–2 only, Positional Bias `b` ≠ 50: `c = (2b − 100) / 3`;
   3D-capable buffer and `c > 0`: `v = (255 − 255c/50) × v / 255`;
   3D buffer and `c < 0`: `v = (255c/50 + 255) × v / 255`.
7. If `v`, occlusion and position all equal the values last sent to
   this channel, nothing is sent this tick.
8. **Linear falloff**: `d = √(x² + y²)` (f32, unclamped). If `d > min`:
   `v = trunc((max − d) × v / (max − min))` in f32.
9. `v = Volume × v / 255` (record `Volume`).
10. **Mode 0, non-stereo** (`0x00516830`): with `X, Y, Z` = position ×
    0.003125: `r = √(X² + Y² + Z²)`, capped at 100; `t = 0` if `r < 2`,
    else `t = 6 × log2(r / 2)`; `gain = trunc(255 / 10^(t/20))`,
    clamped 0–255; `pan = trunc(X × 127 / 1.25 + 128)`, clamped 0–255.
    Then `v = gain × v / 255` and `pan` is sent. Stereo voices get no
    pan or gain.
11. `v` (0–255) is sent as the channel volume.

#### 8.3 Device curves

The values sent are the comparison values (voice log `vol`, `pan`). The
device conversion (`0x005165F0`) is `−2000 × log10(full / x)` hundredths
of a dB with `x ≤ 0.0001` → −10,000 and `x ≥ full − 0.0001` → 0:

1. Volume (`0x005157B0`, full = 255): amplitude = `v / 255`.
2. Pan (`0x00515890`, full = 127): `pan < 128` attenuates the right
   channel to `pan / 127`; `pan > 128` attenuates the left to
   `(255 − pan) / 127`; 128 → both full.

`GainCurve` (`client/audio.md` §A4) uses these ratios, not the dB
values.

### 9. Settings

Read at startup (`0x00514B60`) from the `Diablo II` settings store
(registry, `0x00414F10`), out-of-range values ignored:

<!-- rows -->
| Key | Range | Default | Use |
|---|---|---|---|
| `Sound Mixer` | 0–2 | 0 | mixer mode: 0 plain, 1 3D, 2 3D + EAX (`0x008817AC`) |
| `Master Volume` | 0–100 | 100 | §8.2 r3 (`0x008817B0`) |
| `Music Volume` | 0–100 | 50 | §8.2 r2, §6.3 (`0x008817B4`) |
| `Positional Bias` | 0–100 | 50 | §8.2 r6 (`0x008817B8`) |
| `NPC Speech` | 0–2 | 2 | not a table rule (`audio/triggers.md`) |
| `Options Music` | 0–1 | 1 | not a table rule (`audio/environment.md`) |

The options menu writes these (setters `0x00514CA0`, `0x00514CD0`,
`0x00514D00`, …). How a slider position maps to 0–100 is open question
9. d2rs reproduces mixer mode 0 only (ours: modes 1–2 are DirectSound3D
and EAX hardware paths).

### 10. Sample cache

1. Limit (`0x00481840`, set at `0x0045739C`): physical memory / 100,
   clamped to 3 MiB–5 MiB (5,242,880 bytes on any machine above 500 MB).
   Total cached bytes: `0x007BC9D0`.
2. **Load** (`0x00482970`): path (§3); file missing → +0x81 = 1, never
   tried again (the sound stays silent). If the cache would exceed the
   limit, evict (`0x004824A0`): drop loaded, unplaying samples, first
   skipping locked (+0x8A) and `Cache` rows and (when asked) samples
   used within the last 750 ticks; if not enough is freed the load is
   abandoned. Sync load reads the whole file; async opens a handle
   (pending loads counter `0x007BC9C8`).
3. **Preload** (`0x00482B40`, at most every 25 ticks): every row with
   state 0 and (lock count > 0 or `Cache`) starts an async load while
   fewer than 15 are pending and no eviction failed in the last 250
   ticks; finished async loads are collected.
4. **Locks** (`0x004822B0(id, ±1)` over the id's whole group,
   `0x00482370` clears): taken by the unit code at `0x004CC160` and
   `0x004E4240`; unlocking a zero count is fatal (`0x1DC`).

The cache decides only *when* a sample is available, which matters
through §6.3 r4 (one-shots dropped while not loaded). d2rs decodes
every file up front (`client/assets.md` §A5); it reproduces the
observable effect: an `Async Only` sound that is not loaded is
deferred, a sync one starts at once (open question 10).

### 11. Live data (1.14d)

- P `sounds.txt`: 4,699 rows (X 4,698, D 3,587 shadowed,
  `loading.md` §11). 70 rows have `Volume = 0`; 698 rows open a group
  (sizes 1–8); 7 nested groups; 187 `Loop`; 1,765 `Stream`; 9 `Cache`;
  9 `Duration > 0`; 590 `Compound ≠ 0` (49 are −1); block count 1 on 86
  rows, 2 on 4 (`music_jungle`, `music_town_4`, `music_wilderness`,
  `music_xsiege`), 3 on none.
- Files (§3 rule, hash lookup in all 11 archives): 4,435 distinct paths,
  4,399 present; 4,508 rows resolve (d2sfx 2,261, d2speech 1,224,
  d2exp 511, d2xtalk 465, d2music 33, d2xmusic 9, patch_d2 5); 191 do
  not: 157 rows name `none.wav` (absent everywhere) and 34 name missing
  files (e.g. `regurgitator_neutral_1..4`, the Act 3 `*_q2_*` lines,
  `tyrael_greeting_time_1..3`, `monster_diablo_taunt_ex`). All 191 stay
  silent by §10 r2.
- `soundenviron.txt`: 50 rows; song range 4,657–4,684.

### 12. Edge cases kept

1. Priority wrap for the local player (§5 r4).
2. Falloff 4 with `d > 2000` but clamped distance² ≤ 2000²: §8.2 r8
   divides by `max − min = 0` (f32 → ±inf; `0x00682FD0` then gives
   0x80000000). Open question 11.
3. Group variants run past the group when a mid-group id is requested
   with its own non-zero size (nested groups, §4 r1).

### 13. d2rs mapping

| Hook (`d2-client`) | Rules |
|---|---|
| `SoundTable` / `SoundBank::file` | §1, §3, §4 r1–r2 |
| variant choice (client RNG = local player client unit seed) | §4 r3–r5 |
| `VoicePolicy::admit` | §5 r1–r2, §6.3 r3, §7 r1, r3 |
| scheduler (fades, duration, loop restart, drops) | §5 r5, §6 |
| `GainCurve` | §8.3 |
| voice log `vol`, `pan` | §8.2 output |

## Constants & data dependencies

Columns: §1 r2. Fixed: id ranges 2934–4656 (speech, `DATA\LOCAL`),
4657–4698 (music path), duck exemptions 1–15 and 52–71; group base 202
(thunder); 200 requests; 16 channels; compound/defer windows in sound
ticks; falloff table §8.1; 0.003125 world→pan scale; 750 / 250 / 25
tick cache timers; cache 3–5 MiB.

## Randomness

One draw source: `roll` on the local player's client unit seed (§4 r5).
Per start without flag bit 0: if `Group Size` is 2 or 3, one `roll(n +
1)`; then one `roll(n)` per attempt until accepted. `Group Size ≤ 1`:
no draw. Environment cues draw from the same seed (`audio/environment.md`).
The draw order across one tick follows the request list order (§6.2).

## Edge cases & original bugs

§12. Also: requesting id 0 or an id with `Volume = 0` returns 0 with no
side effect; a sound whose file is missing never retries (§10 r2).

## Test vectors

Synthetic (CI):

| Input | Expected | Source |
|---|---|---|
| group sizes by id 1..7: 3, 0, 0, 0, 2, 0, 0 | bases 1, 1, 1, 4, 5, 5, 7; sizes after pass 3, 1, 1, 1, 2, 1, 1 | §4 r1 |
| nested: id 1 size 5, id 2 size 5, ids 3..7 size 0 | bases 1, 1, 2, 2, 2, 2, 7 | §4 r1 |
| pick id 100, `Group Size` 4, seed {lo 1, hi 666}, history empty | 103; seed after {1,791,398,751, 0} | §4 r3, `sim/rng.md` §2 |
| pick id 50, size 2, history [50, 0], seed {1, 666} | `roll(3)` = 0 → k = 0; `roll(2)` = 1 → 51; seed {791,599,131, 747,178,749} | §4 r3 |
| path id 1, `cursor\pass.wav` | `DATA\GLOBAL\SFX\cursor\pass.wav` | §3 |
| path id 2934 / 4656 / 4657 / 4698 | `DATA\LOCAL\SFX\…` / `DATA\LOCAL\SFX\…` / `DATA\GLOBAL\MUSIC\…` / `DATA\GLOBAL\MUSIC\…` | §3 |
| local player request, `Priority` 255 | request priority 79 | §5 r4 |
| fade 0 → 200, t0 100, t1 112; now 106 / 107 / 113 | 100 / 116 / 200 | §6.3 r2 |
| v 255, Master 50, no duck, `Volume` 210, stereo, d < min | 127 → 104 | §8.2 |
| same, `Falloff` 1, d = 380 | 127 → 63 → 51 | §8.2 r8 |
| mode 0 pan/gain, position (320, 0, 0) / (−320, 0, 0) / (1280, 0, 0) | gain 255 pan 229 / gain 255 pan 26 / gain 127 pan 255 | §8.2 r10 |
| device volume 255 / 0 | 0 / −10,000 hundredths dB | §8.3 |

Real (`#[ignore]`, `D2_GAME_DIR`):

| Input | Expected | Source |
|---|---|---|
| P `sounds.txt` | 4,699 records; id = line index for all | §1 r1 |
| id 1 `cursor_pass` | path `DATA\GLOBAL\SFX\cursor\pass.wav`, found in d2sfx; `Volume` 255, `Priority` 100 | §3, §11 |
| id 202 `event_thunder_1` | `Group Size` 3; bases of 202–204 = 202 | §4 r1 |
| id 309 `weapon_bow_1` | `Group Size` 5, `Compound` 4; id 314 opens a group of 3 | §4, §5 r2 |
| id 2934 `amazon_cantcarry_1` | `DATA\LOCAL\SFX\common\amazon\ama_cantcarry.wav`, d2speech | §3 |
| id 4657 `music_caves` | `DATA\GLOBAL\MUSIC\act1\caves.wav`, d2music; `Loop` 1, `Stream` 1, block count 1 | §3, §4 r2 |
| id 4679 `music_wilderness` | block count 2 | §4 r2 |
| id 4698 `music_quest_shenk` | found in d2xmusic | §3 |
| id 1595 `regurgitator_neutral_1` | `Volume` 0 (request returns 0); file missing | §5 r1, §11 |
| id 4640 `monster_diablo_taunt_ex` | file missing → silent | §10 r2 |
| whole table | 4,508 rows resolve, 157 `none.wav`, 34 missing; 698 group openers, 7 nested | §11 |
| P `soundenviron.txt` | 50 records; song range 4,657–4,684 | §1 r5 |

## Provenance

1.14d `Game.exe` (sha256 631066c1…adaaf), Ghidra export and
`tools/ghidra/disasm.py` (register arguments): loader `0x00481950`;
accessors `0x00481860`–`0x00481920`; path `0x00482710`; variants
`0x00482680`, RNG copy `0x004E40A0`, player `0x00463DD0`; cache
`0x004822B0`–`0x00482B40`, limit `0x00457300`; request layer
`0x004B93F0`–`0x004BAAB0` (request `0x004B9A00`, fade `0x004B9EF0`,
update `0x004BA020`, comparator `0x004B9DA0`); start `0x004E01B0`,
channels `0x004DF9D0`, `0x004DFB50`; volume/pan `0x004DFC20`,
`0x00516830`, `0x005165F0`, `0x00515890`, `0x005157B0`; settings
`0x00514B60`. Float constants read from the image (`0x006D9100`–
`0x006D9118` falloff; `0x006DEB04`–`0x006DEB60` curves; `0x006DBA00`
0.003125; `0x006DA690` 0.5; `0x006DA6B0` 0.05; `0x006DA694` −2000).
Live counts: `sounds.txt` / `soundenviron.txt` extracted from P;
file presence by MPQ hash-table lookup in all 11 archives (our script,
2026-10-06; listfiles are incomplete, so `mpq-tool list` was not used
for presence). D2MOO not used.

## Open questions

1. Voice-log conformance: record 0x004E01B0 starts and the values passed
   to `0x005157B0`/`0x00515890` with the sound tick (`client/audio.md`
   §B7) in a scene with known unit positions; settles §6–§8 as a whole.
2. Which client positions `0x00620900` returns (units of `x`, `y` in
   §8.1 r1) for each unit type; a trace of `0x004B97D0` inputs.
3. Whether variant draws interleave with other users of the player's
   client unit seed within one tick (draw order across systems); an
   RNG trace (`sim/rng.md` tooling) filtered on seed address player+0x20.
4. When the second sound-tick call (`0x0044F01D`) runs (pause? menu?);
   decides tick counting while paused.
5. Effect of the occlusion value (`0x00515A90`) in mixer mode 0.
6. The state-duck condition (`0x0044DB30`, `0x00453A90` at `0x004BA640`):
   which game state (likely a menu or pause) lowers it.
7. Whether streamed music uses `Block 1/2` (loop window) inside the
   stream player or elsewhere (`0x004DCAA0`, `0x004DCD40` read sound
   records).
8. The `object_river` (2599) position projection in `0x004B97D0`.
9. Options-menu slider → 0–100 mapping (owner `client/ui.md`).
10. Whether async-load latency changes which one-shots play in practice
    (d2rs has no load latency); compare voice logs for a fresh start.
11. Falloff-4 division by zero result (§12 r2); a debugger read at
    `0x004DFEFD` with a unit beyond 2,000 on one axis.
