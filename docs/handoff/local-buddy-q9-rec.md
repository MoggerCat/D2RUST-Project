# local-buddy: ninth-set recordings (task `q9-rec`)

Branch `claude/local-buddy-q9-rec-2026-10-07` (from `origin/claude/specs-staging`
913d3b0, then `origin/claude/local-buddy-q-rec-2026-10-07` merged; no
conflicts). Local PC, the user's own 1.14d install (reference `Game.exe` hash
checked on every run), one game at a time under `C:\d2slots\game.lock`. Raw
files only in `C:\Users\zffit\Desktop\D2test\traces-raw-buddy\` (never
committed). Scope: `docs/HANDOFF.md` §5 A S9-A1–S9-A7 and §7 ninth set,
PC 2 R2-1–R2-38. The session was **stopped early by the coordinator**
(wrap-up request) during the first new recording; most new items are
NOT STARTED. Earlier results are cited from `local-buddy-recordings.md`
(pass 1 / 2) and `local-buddy-q-rec.md`.

## New tool (committed)

`tools/trace-recorder/record_objects.py` 0.1.0 (subclass of `spawn.py`;
`--status`, `--trigger`, `--packets` as there). The `record_rng.py` hooks
are armed only inside windows: object allocation (`0x00555230`, ECX = 2),
operate dispatch (`0x00584420`, plus `--operate-ticks` frames), the four
first-population functions (`0x005559A0`, `0x00542B40`, `0x00552610`,
`0x0054EC90`, filtered by `--pop-levels`), and `--rng-file`. Inside a window
markers are logged at `0x0054F5D0`, every init function of `0x00731BC0` and
the animation setup `0x00624390`. At `0x0054EC90`'s entry the room's DRLG
room (rect, type, status, flags, rooms-near array) and its +0x64 logical
info (flags, count, every coordinate record) are dumped; `--dump-file`
dumps every DRLG room of the player's level. Format `objects-raw-0`
(provisional); `check_rng.py` reads it. Entry bytes (`55 8b ec`) are checked
before arming.

## Run `obj1` (the only new recording)

`record_objects.py --pop-levels 2,3,8 --status --dump-file --rng-file
--trigger --packets -- -w -ns -nosave -name bdMercTwo -bar`. Started in the
Rogue Encampment, walked east into Blood Moor (level 2) to about (4661, 5662)
with fights; closed with WM_CLOSE at the wrap-up request. No operate was
made (none of the five object kinds was reached in time).

- Raw: `obj1-objects.jsonl` (3,103 draws, 170 seed sets, 32 object
  allocations, 121 population windows), `obj1-objects-packets.jsonl`
  (42,339 events).
- `check_rng.py`: **OK** (2,977 chain_by_state, 28 chain_unexplained, 35
  draw sites). `check_packets.py`: **OK** (no 0x3E in the run, so F1 did not
  show).

### Object allocation with `Sync` = 0 (objects.md OQ 2): answered

26 of the 32 allocations have `Sync` = 0. Order seen inside `0x00555230`
for every one (seq numbers from `obj1-objects.jsonl`, e.g. seq 1–9, class 37
GUID 1, town, caller `0x555843` the preset spawner, mode argument 0):
`init` + one game-seed step (`0x552E31`) + `init_low` of the unit seed, then
`0x0054F5D0`, then the init function (index 8, `0x005500C0`), then
`0x00624390` (mode 2) with one `roll(25)` at `0x624563` on the object seed
(FrameDelta 200, r = 23, speed 211 = 23 + 200 − 12). Objects with `Sync` = 0
allocated in mode 0 whose init does not change the mode (class 39 `fire`,
35, 36; FrameDelta 128) get **no** animation call and no draw (speed 0,
frame count 0 at the return). So the allocation does **not** run the §4
animation setup before the init function: the stored mode at `0x005553E5`
skips it, and the only §4 draw comes from a mode set inside the init (or
§3 r8). Not examined: 2 of the class-37 allocations show speed 0 at the
return (mode 2); worth a look in the file.

### Blood Moor first population and logical lists (S9-A6, partly)

29 first populations of Blood Moor rooms (presets, restore, objects,
monsters windows each; 28 monsters created), each with the room seed and
game seed before / after and every draw (window label `pop:<fn>`). 29 DRLG
+0x64 dumps at the monster-population entry, e.g. rooms (888, 1152, 8, 8)
flag 1 count 1 (one record), (896, 1136, 8, 8) **flag 2** count 3 with 6
records. For the cloud: compare the dumps with `drlg::logic` on the same
tiles and the population draws with `WorldSim`. The crypt level (Den of
Evil) was not reached: NOT STARTED.

## Table

| Item | Status | Source / file | Check | Key numbers |
|---|---|---|---|---|
| R2-1 order-0003 river-bank cells | blocked (pass 1 item 13; not retried) | `oq7-frames.jsonl` | NOT ENOUGH (walking) | cells ~10 tiles off-screen from every reachable spot |
| R2-2 weather-0001 | partly answered | `wx1-frames.jsonl` | selftest ok | W5: splashes in 159 frames, all with int(intensity) = 0; pass 4 / 9 pixels and per-floor draws not done |
| R2-3 Arcane / Summit seed globals | blocked | — | — | Act II / V progression |
| R2-4 Logicals preset fade (18, 19) | NOT STARTED | — | — | — |
| R2-5 GDI line rule | partly answered (C70) | `wx1-frames.jsonl` | scratch compare | 24,427 lines, 99.26 % pixels; "exceeds" rule; (x1, y1) not drawn when short (27 of 15,824); no abs(dx) = abs(dy) line, tie open |
| R2-6 shadow position | NOT STARTED | (draws in `wx1-frames.jsonl`) | — | — |
| R2-7 env per update | answered (C69) | `env69-sound.jsonl` | `check_env.py` OK, 0 / 41,891 mismatches | day = 35,844 updates; light-map digest not run live |
| R2-8 frame-1 S→C order 0x59..0x15 | data recorded, not analysed | `join1-packets.jsonl` | `check_packets.py` OK | — |
| R2-9 game +0x80 | answered (C85) | `join1.jsonl` | OK | 0xF74A29B4 in all three |
| R2-10 client unit / room seed step | recorded (C83) | `join1.jsonl` | OK | 32 / 32 one step; bridge replay is the cloud's |
| R2-11 act palette switch | blocked | — | — | act change needs Andariel |
| R2-12 Cain rescue | blocked | — | — | quest chain |
| R2-13 Countess | blocked | — | — | level ~10+ |
| R2-14 Catacombs after Andariel | blocked | — | — | — |
| R2-15 save header after Andariel | blocked | — | — | — |
| R2-16 quest init order | blocked (C79) | — | — | no Cain rescue |
| R2-17 Cairn / gibbet / tome | blocked (C78) | — | — | — |
| R2-18 chest / shrine / door / well / portal, Sync 0 | **partly: Sync = 0 allocation answered (above)**; the five operates NOT STARTED | `obj1-objects.jsonl` | `check_rng.py` OK | see OQ 2 section; town portal packets exist (`tp80-packets.jsonl`, no RNG) |
| R2-19–R2-22 Act II–V runs | blocked | — | — | progression |
| R2-23 Act II–V monster AI | NOT STARTED | — | — | — |
| R2-24 Might party, Kick / Bash, Druid, Golem | partly answered | `kill1-`, `pal1-`, `dru1-`, `golem1-spawn*.jsonl` | `check_rng` OK; packets F1 only (kill1, pal1, dru1), OK (golem1) | Bash, Might solo, Raven, Clay Golem done; Kick and Might in a party blocked |
| R2-25 missile list | NOT STARTED | — | — | most need Acts II–V |
| R2-26 0x7A order / removes | answered from existing recordings | `dru1-`, `golem1-`, `merc1-spawn-packets.jsonl` | as above | Raven: 0x7A add + 0xAC in the cast frame +10; recast: 0x7A remove ×2, add, 0xAC. Golem: 0x7A add, 0x7F, 0xAC (frame 1621). Hire: **no 0x7A**, 0x81 only. Save And Exit: 0x7A remove (golem 3087, merc 3311) |
| R2-27 hireling hire … give / take | hire done; level-up, death, resurrect, give / take NOT STARTED | `merc1-spawn-packets.jsonl` | OK | 0x81 type 7 class 271 GUID 13 |
| R2-28 teleport with hireling | answered (C80) | `tp80-packets.jsonl` | OK | merc 0x0A in input phase; 0xAC at destination before the player's 0x15 |
| R2-29 act change with hireling | blocked | — | — | — |
| R2-30 Act III creation | blocked | — | — | — |
| R2-31 DRLG +0x64 lists | partly: outdoor (Blood Moor) dumped, crypt NOT STARTED | `obj1-objects.jsonl` `pop` records | — | 29 rooms; flag 1 and flag 2 rooms both seen |
| R2-32 first Blood Moor population vs spawn RNG | recorded (compare is the cloud's) | `obj1-objects.jsonl` | `check_rng.py` OK | 29 rooms, 28 monsters |
| R2-33 kill with drop | recorded (pass 1 item 1) | `kill1-spawn*.jsonl` | rng OK; packets 6 × F1 | drops in the death frame 5250 (GUIDs 9–11) |
| R2-34 sound request log | partly (C74) | `snd74*`, `snd74b*` | packets OK | cave and Blood Raven not done |
| R2-35 DirectSound buffers | NOT STARTED | — | — | — |
| R2-36 placement-0001, ui-0001, ui-0002 | NOT STARTED | — | — | — |
| R2-37 d2s round trip | partly (pass 2 `d2s_check.py`) | saves (local) | bdGolem 23 / 0, bdMercTwo 24 / 0 | round trip itself NOT STARTED |
| R2-38 d2s-tool characters in 1.14d | NOT STARTED | — | — | — |
| S9-A1 PC 1's list | as R2-1, -2, -3, -11, -12, -13, -24, -26, -27, -30, -33 | | | |
| S9-A2 Act I remainders | blocked | — | — | Andariel |
| S9-A3 Act II–V quest runs | blocked | — | — | — |
| S9-A4 monster AI / missiles | NOT STARTED | — | — | — |
| S9-A5 objects | partly (R2-18) | `obj1-objects.jsonl` | rng OK, packets OK | Sync 0 answered; operates not started |
| S9-A6 room population | partly (R2-31, R2-32) | `obj1-objects.jsonl` | rng OK | Blood Moor done, Den of Evil not |
| S9-A7 panel captures | NOT STARTED | — | — | — |

## Next (for a later pass)

- With `record_objects.py`: operate a chest / shrine / well in Blood Moor or
  Cold Plains (objects class 139 and 81 were seen at (4525, 5671) and
  (4617, 5652) in `bdMercTwo`'s Blood Moor, not identified), a town portal
  (the save has a scroll), and a door if one is found; enter the Den of
  Evil with `--pop-levels 8` and `--dump-file` for the crypt dump.
- `bdMercTwo` starts at life 42 / 169 (the save's state): heal at Akara
  before fighting.
