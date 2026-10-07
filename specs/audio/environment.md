# Spec: Audio — Environment (music, quest stingers, level lines, ambience)

- **Status:** draft; every rule names its 1.14d `Game.exe` address and
  the live `soundenviron.txt`/`levels.txt` facts it uses; nothing is
  confirmed by a recording yet (Test vectors, "Checks").
- **Target version:** 1.14d
- **Crate/module:** `d2-client::audio::environment` (music, stinger,
  ambience state machines run once per sound tick; plain Rust).
- **Related specs:** `audio/sound-table.md` (request call, §1 r5 song
  range, §2 table layout, §6 sound tick, §9 settings), `audio/triggers.md`
  (§1 conventions and helpers used here, §3 player event lines),
  `client/audio.md` §B4, §B5, `data/fields.tsv` (`levels.SoundEnv`
  u8 +0x21C), `world/quests.md` (who sends quest events),
  `render/lighting.md` §9 (day cycle), `render/draw-order-2.md` §11
  (weather).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 41–58 |
| Inputs | 59–70 |
| Outputs / state changes | 71–75 |
| Rules | 76–77 |
|   1. Sound environment | 78–103 |
|   2. Music (`0x004DCAA0(T)`) | 104–147 |
|   3. Quest stingers (`0x004DCD40(M, dM, H, k, S, dS, play)`) | 148–198 |
|   4. Level-entry lines (`0x004CC270`) | 199–238 |
|   5. Ambience loop (`0x004E42E0(T)`, first part) | 239–253 |
|   6. Rain (`0x004E42E0`, second part) | 254–269 |
|   7. Event cues (`0x004E42E0`, third part) | 270–287 |
|   8. Sample pins on level change (`0x004E42E0`, last part) | 288–295 |
| Constants & data dependencies | 296–305 |
| Randomness | 306–314 |
| Edge cases & original bugs | 315–324 |
| Test vectors | 325–346 |
|   Checks (hook addresses for `record_sound.py`) | 347–356 |
| Provenance | 357–370 |
| Open questions | 371–397 |
<!-- /index -->

## Summary

Once per sound tick (`0x00482C20`), before the request update, the
client runs the **ambience** machine (`0x004E42E0`) and then the
**music** machine (`0x004DCAA0`). Both read the current sound
environment: the `soundenviron.txt` row named by the `SoundEnv` column
of the local player's level. Music plays the row's `Song`, waits 75
sound ticks after a level change before switching, fades songs in and
out by their `Fade In`/`Fade Out`, and remembers per song where to
resume (`Block 1..3`). Quest completions start a **stinger**: the
current song is faded out, a quest track and the hero's quest line are
scheduled on the client update counter, and normal music is held off
for a fixed time. Entering certain levels for the first time plays the
hero's "find" line. Ambience keeps one looping bed per environment
(day or night version, cross-faded over 250 ticks), an optional rain
loop that follows the weather intensity, and random event cues placed
left or right of the listener.

## Inputs

| Name | Type | Source |
|---|---|---|
| sound tick T | `[0x007BC9BC]` | `sound-table.md` §6.1 (passed as the argument of both machines) |
| client update counter C | `[0x007A0498]` | `audio/triggers.md` §1 r5 |
| level L | local player's room → level id (`0x00481780`) | client model |
| environment E | `soundenviron` row `levels[L].SoundEnv` (`0x004817A0`, `0x00481920`) | `data/fields.tsv` |
| day phase | first dword of the client act's environment (`0x0061C220([0x007A0634])`): the period index 0–5 | `render/lighting.md` §9.1 (+0x00), §9.2–§9.3 |
| weather | active (`0x00473C40`), intensity f32 `[0x007A89A0]` (`0x00473C80`) | `render/draw-order-2.md` §11 (§1 r5 here) |
| settings | Master Volume, Music Volume | `sound-table.md` §9 |

## Outputs / state changes

Requests and stops of song, stinger, speech, ambience, rain and event
sounds; the state variables named in each section.

## Rules

### 1. Sound environment

1. L = 0 (no local player or no room) → neither machine does anything
   this tick.
2. E = row `SoundEnv` of level L; an index outside the 50 rows → no
   row (ambience fields read as 0; music song 0).
3. **Day**: phase ∈ {1, 2, 3}; any other phase is night (`0x004E4317`).
   NPC time greetings use the same phase (`audio/triggers.md` §10 r1).
   The phase is the period index of the act environment
   (`0x0061C220` returns dword +0x00 of the record at act +0x04,
   `0x0061AA60`); `render/lighting.md` §9 owns how it is set (S→C 0x53)
   and advanced. All six indices occur; with the normal table day is
   the periods starting at 340°, 0° and 160° (`render/env-periods.tsv`).
4. Live data (P `soundenviron.txt`, `levels.txt`): 50 rows; 48 are used
   by levels, rows 11 (`ANDARIEL_LAIR`) and 35 (`GUILD`) by none; 13
   rows have different day and night ambience ids (36 the same non-zero
   id), 13 different day and night event ids; 6 rows have no event
   (both 0); `Event Delay` 150–800 (250 in 28 rows); `Indoors` = 1 in
   26 rows.
5. **Weather** (`0x00473C40`, `0x00473C80`): active when the snow-mode
   flag `[0x007A8A14]` is 0 and the local player's level has `Rain`
   (levels record +0x05, `0x0061DBA0`; no player → inactive); the
   intensity is the float `[0x007A89A0]` = rain target / 256, moved by
   the rain cycle (`render/draw-order-2.md` §11.1, §11.3 r3). Snow mode
   (`[0x007A8A14]` ≠ 0, Act V, `0x004726F0`) makes it inactive.

### 2. Music (`0x004DCAA0(T)`)

State: current song `cur` (`[0x007C89D8]`), last level `[0x007C89DC]`,
level-change time Tl `[0x007C89D0]`, song time Ts `[0x007C89E0]`, last
announced level `[0x007C8A04]`, resume table `resume[song]`
(`[0x007BC99C]`, one dword per song id in the song range).

1. song = E's `Song` if it lies in the song range 4,657–4,684
   (`sound-table.md` §1 r5), else 0. audible = Master Volume ≠ 0 and
   Music Volume ≠ 0 (`0x00514CC0`, `0x00514CF0`).
2. L ≠ last level → Tl := T, last level := L.
3. If cur ≠ song and (T − Tl > 74, or not audible, or cur = 0):
   cur := song, Ts := T. So a new level's song waits 75 ticks (3 s at
   25 ticks/s) unless nothing is playing.
4. Not audible, or cur = 0 → stop all songs (`audio/triggers.md` §1 r4;
   songs fade out over their `Fade Out`, 125 for every song row).
5. Stinger (§3 r4) runs here; while it holds, the rest is skipped.
6. If cur ≠ 0, audible, and no request with id cur exists
   (`0x004B9610`): o = resume[cur]; flags = 2 (no fade-in) if no
   request with an id in 4,657–4,698 is still active
   (`0x004B9C60`) and o = 0, else 0 (fade in over `Fade In`, 125);
   stop all songs; request(cur, none, 0, flags, o).
7. If L ≠ last announced and T − Tl ≥ 62: level-entry line (§4);
   last announced := L.
8. **Resume points.** If cur ≠ 0 and T > Ts + 125 and cur's request
   is playing: p = its play position (`0x004B9D50` → `0x004DF900`); r =
   `Block 1` if 0 ≤ p < `Block 1`, `Block 2` if `Block 1` ≤ p < `Block
   2`, `Block 3` if `Block 2` ≤ p < `Block 3`, else 0 (−1 cells never
   match); resume[cur] := r, Ts := T. Coming back to a song starts it
   at the next block boundary after where it was (or at 0).
9. **Reset** (`0x004DCA30`, from sound init `0x00482260` at every game
   start and from the video player `0x00482EF0` before a video, when
   `[0x007A0438]` +0x220 is 0): every variable of §2 and §3 and the
   resume table := 0. `[0x007A0438]` is the start-up config; +0x220 is
   the `-ns` / `-nosound` switch byte (`tools/original-hooks.md` §5.1
   switch table; test at `0x004DCA38`), so the reset runs whenever
   sound is enabled.
10. Live: 28 song rows, all `Loop`, `Stream`, `Stereo`, `Music Vol`,
    `Defer Inst`, `Volume` 110, `Fade In`/`Fade Out` 125, `Priority`
    255; `Block 1` set on 19 of them, `Block 2` on 4
    (`sound-table.md` §11). Row 4,668 `music_options` (front end,
    open question 5) is in the range but no environment names it; the
    other 27 songs are each named by at least one row.

### 3. Quest stingers (`0x004DCD40(M, dM, H, k, S, dS, play)`)

State (`[0x007C89E4]`–`[0x007C8A00]`): active, M and its time tM, M
pending, S and its time tS, S pending, hold end tH. Times are in **C**.

1. If cur ≠ 0 and cur's request is playing: resume[cur] := `Block k`
   of cur's row if k ≠ 0, else 0; Ts := T (`sound-table` tick).
2. Stop all songs (fades out over 125).
3. active := 1; M pending, tM := C + dM; S pending := play, tS := C +
   dS; tH := C + H.
4. **Each music tick while active** (§2 r5): M pending and C ≥ tM →
   request(M, none), M not pending. S pending and C ≥ tS → if P is
   alive (`0x00464820(P)` = 0) request(S, P); S not pending. C < tH →
   skip the rest of §2 this tick. Else active := 0 and, if audible,
   stop every request of id M (`0x004BA890`).
5. **Re-arm** (`0x004DCE10(dt, play)`, 0x2C event 92 with (25, 1)):
   only while active: play ≠ 0 → tS := C + dt, S pending, tH := C + dt
   + 50; play = 0 → tH := tS + 50, S not pending.
6. Callers. From player events (`audio/triggers.md` §3 r4; S = the
   class quest line base + e − 33):

<!-- rows -->
| Event | M | dM | H | k | dS | play |
|---|---|---|---|---|---|---|
| 33 | 4,685 `music_quest_andariel` | 0 | 475 | 0 | 425 | 1 |
| 34 | 4,686 `music_quest_bloodraven` | 475 | 800 | 1 | 525 | 1 |
| 35 | 4,688 `music_quest_den` | 0 | 300 | 1 | 250 | 1 |
| 37 | 0 (none) | 0 | 1,500 | 0 | 0 | 0 |
| 50 | 4,693 `music_quest_radament` | 475 | 1,100 | 1 | 525 | 1 |
| 52 | 4,694 `music_quest_tainted` | 0 | 425 | 1 | 375 | 1 |
| 66 | 4,692 `music_quest_mephisto` | 0 | 425 | 0 | 375 | 1 |
| 75 | 4,695 `music_quest_diablo` | 0 | 475 | 0 | 325 | 1 |
| 80 | 4,698 `music_quest_shenk` | 0 | 500 | 1 | 475 | 1 |
| 82 | 4,697 `music_quest_nihlathak` | 0 | 550 | 1 | 400 | 1 |
| 83 | 4,696 `music_quest_baal` | 0 | 525 | 1 | 425 | 1 |

   And without speech: 4,687 `music_quest_compelling` (H 350,
   `0x0046B850`) and 4,691 `music_quest_izual` (H 250, `0x0046BC10`),
   both dM = 0, k = 0, play = 0. 4,689 `music_quest_forge` and 4,690
   `music_quest_horadric` are referenced by no code (unused).
7. Stinger tracks (4,685–4,698) are outside the song range: they are
   played and kept even at Music Volume 0 (`sound-table.md` §6.3
   `audible` exempts only the song range), and "stop all songs" does
   not stop them; they end by themselves (none has `Loop`) or by r4.
   12 of the 14 have no `Music Vol` flag, so only Master Volume and
   their `Volume` (110; 200 Nihlathak; 255 Shenk, Baal) scale them;
   4,686 (Blood Raven) and 4,693 (Radament) are scaled by Music
   Volume.
   Event 37 (Forgotten Tower) only silences music for 1,500 updates
   and never plays its line. Reproduced.

### 4. Level-entry lines (`0x004CC270`)

Table `0x0072A2C4`: 14 records of (10 level ids, quest q, event e):

<!-- rows -->
| Levels | q | e (line) |
|---|---|---|
| 17 | 2 | 38 `act1_find_burial` |
| 34, 35, 36, 37 | 6 | 40 `find_catacombs` |
| 8 | 1 | 41 `find_den` |
| 29, 30, 31 | 6 | 42 `find_jail` |
| 26, 27 | 3 | 43 `find_monastery` |
| 20, 21, 22, 23, 24, 25 | 5 | 44 `find_tower` |
| 38 | 6 | 46 `find_tristram` |
| 2, 3, 4, 5, 6, 7 | 1 | 47 `find_wilderness` |
| 74 | 12 | 55 `act2_find_arcane` |
| 58 | 13 | 56 `act2_find_clawviper` |
| 110 | 31 | 76 `act5_find_wilderness` |
| 120 | 35 | 78 `act5_find_mountaintop` |
| 121 | 34 | 77 `act5_find_nihlathak` |
| 132 | 36 | 79 `act5_find_worldstone` |

1. Called from §2 r7. Nothing if L ≥ the level count, or L equals the
   last level checked (`[0x007C88CC]`, then set to L), or L is already
   flagged (`[0x007C78B8 + 4·L]`).
2. The first record holding L: flag every level of that record. Then,
   if q = 0 or the client quest check `0x004A4180(q)` passes (quest q
   open and not done in the client quest state, owner
   `world/quests.md`), and C − P+0x7C > 62, and `any_speech` is false:
   player event e on P (`audio/triggers.md` §3 r4: the class line base
   + e − 33, delay per that rule).
3. The flags are set even when r2 plays nothing, so a line skipped
   because the hero spoke within 62 updates is lost for that game.
4. **Reset** (`0x004CA280`, from sound init `0x00482260`, which the
   client game state `0x0044F360` calls once per game start): the
   1,024 level flags `[0x007C78B8]` := 0 (and `[0x007C88B8]`,
   `[0x007C88BC]`, `[0x007C88C4]`, `[0x007C88C8]` := 0, `[0x007C88C0]` :=
   90). The last level checked `[0x007C88CC]` is never reset: 0 at
   process start, it carries over from game to game.

### 5. Ambience loop (`0x004E42E0(T)`, first part)

State: current bed id `[0x007C8C88]`, its handle `[0x007C8C80]`.

1. a = E's `Day Ambience` by day, else `Night Ambience` (0 without E).
2. If a ≠ current: swap := (a, current) is (day, night) or (night,
   day) of E. swap → fade the current handle to 0 over 250
   (`0x004B9EF0`). Not swap → stop ambience beds 52–71 except groups a
   and 64 (`scene_rain`, when raining, else 0), and event cues 72–201
   except E's current event id (`audio/triggers.md` §1 r4).
3. Then a = 0 → current := 0, handle := 0. Else handle := request(a,
   none); swap → volume 0 and fade to 255 over 250; current := a.
4. A bed that is the same id by day and night (36 rows, e.g. caves)
   keeps playing across the day change.

### 6. Rain (`0x004E42E0`, second part)

State: rain handle `[0x007C8C84]`, previous rain id `[0x007C8C8C]`.

1. r = 64 (`scene_rain`) while weather is active, else 0. v =
   trunc(intensity × 255.0) when weather is active and the intensity ≠
   0, else 0.
2. r = 0 → stop the rain handle (`0x004BA840`); handle := 0, previous
   := 0.
3. r ≠ 0: if v ≠ 0 and no handle: handle := request(previous, none)
   (the *previous* tick's id, so the first raining tick requests 0 and
   gets nothing; rain starts one sound tick later); handle → volume v.
   Then with a handle: w = its volume; w moves toward v by at most 6
   (w < v: min(w + 6, v); w > v: max(w − 6, v)); w = 0 → stop it,
   handle := 0; else volume w. previous := r.

### 7. Event cues (`0x004E42E0`, third part)

State: event id `[0x007C8C90]`, gap `[0x007C8C94]`, last cue
`[0x007C8C98]` (in T).

1. ev = E's `Day Event` by day, else `Night Event`; D = `Event Delay`.
   ev = 0 → nothing (state kept).
2. ev ≠ event id: gap := D + jitter(⌊D/3⌋); last := T − roll(gap);
   event id := ev.
3. T − last ≥ gap (unsigned) and request(ev, none) returns a handle h:
   s = roll(2) ≠ 0 ? +1 : −1; x = s × uniform(450, 750); y =
   jitter(100); position of h := (x, y, 640.0) (`0x004B99A0`; distance
   from x, y as `sound-table.md` §8.1 r1, z only in the mode-0 gain);
   last := T; gap := D + jitter(⌊D/3⌋).
4. A failed request (0) draws nothing and retries next tick.
5. After §5–§7: EAX room settings (`0x004DF6C0`, mixer mode 2 only;
   not reproduced, `sound-table.md` §9).

### 8. Sample pins on level change (`0x004E42E0`, last part)

When L ≠ `[0x007C8C7C]`: clear the locks of the footstep groups
2,720–2,876 (every 4th id), then lock the walk and run groups of
`Material 1` and of `Material 2` (`0x004E4240`); `[0x007C8C7C]` := L.
Cache only (`sound-table.md` §10 r4); d2rs: no observable effect
except through async loading (`sound-table.md` open question 10).

## Constants & data dependencies

`soundenviron` columns `Song`, `Day/Night Ambience`, `Day/Night Event`,
`Event Delay`, `Material 1/2` (`sound-table.md` §2 offsets); `levels`
`SoundEnv`; sound rows 64, 52–71 (beds), 72–201 (cues), 4,657–4,698
(music). Fixed: 74/75 (song switch wait), 62 (entry line wait, two
places), 125 (resume bookkeeping period), 250 (bed cross-fade), 6
(rain step), 255.0 (rain scale), 450–750 / ±100 / 640 (cue position),
⌊D/3⌋ (cue jitter), stinger table §3 r6, entry table §4.

## Randomness

Client RNG (`audio/triggers.md` §1 r7), per sound tick in this order
(ambience runs before music): on an event-id change jitter(⌊D/3⌋) =
roll(2⌊D/3⌋ + 1), then roll(gap); on a cue: roll(2), roll(301),
roll(201), then roll(2⌊D/3⌋ + 1); then the variant draws of the cue at
its start (`sound-table.md` §4; event rows 72–201 open groups of 4–8). Music
draws nothing itself; level lines draw only through their variants.

## Edge cases & original bugs

1. Rain starts one sound tick after the weather does (§6 r3).
2. Stingers ignore Music Volume (§3 r7).
3. Event 37 never plays its line (§3 r7).
4. A skipped level-entry line is never retried (§4 r3).
5. Music resumes at the *next* block boundary, so a song left before
   `Block 1` restarts at `Block 1`, and one left after its last block
   restarts at 0 (§2 r8).

## Test vectors

Synthetic (CI):

| Input | Expected | Source |
|---|---|---|
| level change at T 100, song A playing, new song B | cur stays A through T 174; at T 175 cur = B, A gets the stop flag and fades over 125, B is requested the same tick with flags 0 (fade-in 125, A still active) and offset resume[B] | §2 r3, r6 |
| first song after load (cur 0) | switches at once; flags 2, offset 0 | §2 r3, r6 |
| Music Volume 0 | all songs stopped; stinger M still requested at tM | §2 r4, §3 r7 |
| pos 1,000,000 in `music_caves` (B1 1,478,063) | resume = 1,478,063 | §2 r8 |
| pos 2,000,000 in `music_caves` | resume = 0 | §2 r8 |
| pos 4,000,000 in `music_wilderness` (B1 3,457,024, B2 6,755,328) | resume = 6,755,328 | §2 r8 |
| event 35 at C 1,000 | M 4,688 at C 1,000; line at 1,250; music held until 1,300 | §3 r6 |
| rain intensity 0.5, two ticks | tick 1: request(0) → no handle; tick 2: request(64), volume 127 | §6 r3 |
| rain volume 20 → target 0 | 14, 8, 2, then stop | §6 r3 |
| D = 250, seed known | gap = 250 + roll(167) − 83 | §7 r2 |

Real (`#[ignore]`, `D2_GAME_DIR`): the §1 r4 counts from P
`soundenviron.txt`/`levels.txt`; every `Song` lies in 4,657–4,684 or is
0; the §4 table's events give, for each class, a line whose name holds
`find`.

### Checks (hook addresses for `record_sound.py`)

| Check | Hook | Record |
|---|---|---|
| music | `0x004DCAA0` entry and every `0x004B9A00` call from it | T, C, L, cur, Tl, Ts, resume[cur] |
| stinger | `0x004DCD40` entry | ECX, EDX, 5 stack words, C |
| ambience | `0x004E42E0` entry/exit | T, day phase, weather flag and intensity, `[0x7C8C80..0x7C8C98]` |
| cue position | `0x004B99A0` entry | handle, x, y |
| entry lines | `0x004CC270` entry | L, flags, C − P+0x7C |

## Provenance

1.14d `Game.exe` (sha256 631066c1…adaaf), Ghidra decompile export and
`tools/ghidra/disasm.py` (register arguments of `0x004DCD40`,
`0x004E4100`, `0x004E4120`, `0x004B99A0`): sound tick `0x00482C20`;
music `0x004DCAA0`, `0x004DCA30`, `0x004DCD40`, `0x004DCE10`; group
stops `0x004BA840`–`0x004BAA50`; ambience `0x004E42E0`, pins
`0x004E4240`; level lines `0x004CC270`, quest check `0x004A4180`,
callers `0x004CB9C0` (`audio/triggers.md`). Constants read from the
image: 250 (`0x004E43E6`), 255.0 (`0x006DBD68`), 640.0 (`0x006DA698`),
tables `0x0072A2C4`, `0x0072A024`–`0x0072A2C0`. Live facts from P
`soundenviron.txt`, `levels.txt`, `sounds.txt` (our script). D2MOO not
used (no client sound code).

## Open questions

1. ~~Confirm §2–§7 with a recording (Checks): walk town → wilderness →
   cave and back; stand through a day change in the wilderness; kill
   Blood Raven.~~ Moved to the recording list
   (`docs/handoff/pc2-rec-pc2-render-audio.md` RA-E1).
2. ~~Units of the play position `0x004DF900` versus `Block` values
   (bytes or sample frames of the stream).~~ Partly static:
   `0x004DF900` → `0x005159B0` returns the Storm stream position
   (`0x00418140`, stream record +0x2C) shifted right by 2; the unit of
   +0x2C is set by the Storm refill thread (not read). Moved to the
   recording list (RA-E2).
3. *Answered* (`0x0061C220`, `0x0061AA60`): the phase is the act
   environment's period index, owned by `render/lighting.md` §9; all of
   0–5 occur (§1 r3).
4. *Answered* (`0x00473C40`, `0x00473C80`, `0x0061DBA0`): §1 r5; the
   weather state machine is `render/draw-order-2.md` §11 (thunder 202
   is `audio/triggers.md` §12).
5. ~~Front-end music (`Options Music` setting, `music_options`,
   `0x00514D80` callers `0x0042FB20`–`0x004FA160`): out of game, owner
   a front-end spec.~~ Needs more than two function reads (many
   callers); moved to the recording list (RA-E3).
6. *Answered* (`0x004CA280`, `0x00482260`, `0x0044F360`): §4 r4. The
   flags are cleared at every game start; `[0x007C88CC]` never.
7. *Answered* (`0x004DCA38`, `tools/original-hooks.md` §5.1): +0x220 is
   the `-ns` switch; §2 r9.
