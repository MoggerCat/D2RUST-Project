# Spec: Skills — Player skill sequences (`seqnum`)

- **Status:** draft: read from the 1.14d `Game.exe` disassembly and image
  (addresses below); `sequences.tsv` dumped from the image (table
  `0x007483B8`, 23 sequences × 14 weapon classes, every record checked:
  bytes 0, 1 and 4 of each frame are 0, the two counts are equal).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::units` (sequence load, `UnitHooks::load_sequence`);
  table compiled from `sequences.tsv`
- **Related specs:** `sim/units.md` §4.1–§4.3 (mode start, the event
  schedule, rate); `skills/use.md` §5.1–§5.2 (mode of a skill, frame
  events); `data/callbacks.md` §4 (monster skill sequences, `monseq`);
  `skills/bodies-2.md` §2.13, `bodies-2b.md` §6.12, §7.20, §8.11 (bodies
  that rewind a sequence).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 36–45 |
| Inputs | 46–54 |
| Outputs / state changes | 55–61 |
| Rules | 62–63 |
|   1. Lookup `0x00663310` | 64–90 |
|   2. Load `0x00621260` | 91–111 |
|   3. Frame queries | 112–307 |
|   4. The table (`sequences.tsv`) | 308–339 |
|   5. Users in 1.14d | 340–356 |
| Constants & data dependencies | 357–365 |
| Randomness | 366–369 |
| Edge cases & original bugs | 370–389 |
| Test vectors | 390–404 |
| Provenance | 405–419 |
| Open questions | 420–425 |
<!-- /index -->

## Summary

A player in mode 18 (SQ, the "sequence" mode) does not play one AnimData
animation: it plays a **sequence**, a list of frames that each name a
drawn mode, a frame of that mode and an event byte. The sequence comes
from the used skill's `seqnum` and the unit's COF weapon class. The event
bytes drive the skill's frame events (`use.md` §5.2) exactly as AnimData
event bytes do for other modes; the frame count replaces the AnimData
frame count. Monsters in mode 14 use `monseq.txt` instead (§1).

## Inputs

| Name | Type | Source |
|---|---|---|
| used skill | skill entry | unit (`0x00620250`) |
| `seqnum` | u8, skills record +0x13 | `skills.txt` |
| COF weapon class | 0–13 | `0x0064F380(unit, inventory, &c, −1, 1)` |
| sequence table | image | `sequences.tsv` (this spec) |

## Outputs / state changes

Unit fields (`sim/units.md` §2 table): +0x30 sequence (frame list), +0x34
sequence length · 256, +0x38 sequence position (8.8), +0x3C sequence
speed, +0x48 frame count · 256, +0x40 drawn mode, +0x44 drawn frame · 256,
+0x4E action byte, unit flag 0x4000 (+0xC4).

## Rules

### 1. Lookup `0x00663310`

`seq(unit)` (unit in ESI) → a sequence record {frame list, length,
count}, or none:

1. No used skill → none.
2. s = `0x00643D00(unit, used)`:
   - player (unit type 0): the skill's `seqnum` (skills record +0x13, u8);
   - any other unit: skill id 199 (`DiabPrison`) → 0; else the slot i
     (0–7) whose monstats `Skill<i+1>` (i16 at +0x170 + 2i) equals the
     skill id → the slot's sequence (i16 at +0x188 + 2i,
     `data/callbacks.md` §4); no slot → 0.
3. s ≤ 0 → none.
4. Player: R = the per-class array of sequence s (`0x007483B8`[s]); the
   record is R[k] where k is the index whose class code equals the unit's
   COF weapon class (map `0x00748418`: 14 pairs {k, class}; in 1.14d k =
   class for every entry, so the record index is the class itself). No
   match → fatal assertion (line 0x533). s is not bounds-checked (§Edge
   cases 2).
5. Monster (type 1): monstats record of the class exists (`0x00451F80`)
   → `monseq` record `0x00659E30(s)`; else none.
6. Other unit types → none.

Accessors (each calls the lookup; none → 0): frame list `0x006633B0`
(record +0), length · 256 `0x006633D0` (+4 shifted), count `0x006633F0`
(+8).

### 2. Load `0x00621260`

Run by the mode start's prepare step `0x005533D0` (`sim/units.md` §4.1)
when the unit is a player in mode 18 or a monster in mode 14 **and** the
frame list is non-null; otherwise the plain AnimData path runs (sequence
:= none). Also called twice by the mode-change re-init `0x00624390`.

1. +0x30 := frame list; null → stop.
2. +0x34 := length · 256; +0x38 := 0; +0x3C := 256; +0x48 := count · 256.
3. Frame setup `0x00621210(unit, 0)` (§3) from frame 0: +0x40 := its
   drawn mode, +0x44 := its drawn frame · 256, +0x4E := its event byte.
4. Unit flag 0x4000 (+0xC4) set.

In the sequence branch the prepare step then computes the rate
(`0x00623F50`, `sim/units.md` §4.3, §4.7, which sets the sequence speed
+0x3C) **whether or not the unit has a path**, and stops: no AnimData
lookup (`0x00620F00`) and no AnimData frame count; +0x48 stays count ·
256 from step 2 (`0x0055341A`–`0x0055343B`). Only the non-sequence
branch sets +0x48 from the AnimData record (`0x00553460`–`0x00553469`,
`0x0055347F`–`0x00553488`).

### 3. Frame queries

A frame record is 6 bytes: +2 drawn mode (player mode code, `plrmode`
row), +3 drawn frame, +5 event byte (0 none, 1–4 the codes of `use.md`
§5.2); +0, +1 and +4 are 0 in every 1.14d record.

- **Event byte** `0x006634C0(list, pos, &e)`: e := the event byte of
  frame `pos >> 8`; list null → e := 0. No bound test. The schedule of
  `sim/units.md` §4.2 reads E[i] through it when the unit has a sequence.
- **Event byte by index** `0x00621920(unit, i)`: i ≥ 144 → 0. Player in
  mode 18 or monster in mode 14: the sequence event byte of frame i (no
  bound test against the length); else the AnimData event byte i.
- **Frame range** `0x00663410(list, a, b, &mode, &frame, &x, &e)`: mode,
  frame and byte 4 from frame `a >> 8`; e := that frame's event byte when
  a = b, else the last non-zero event byte of frames `(b >> 8) + 1` …
  `a >> 8` (0 when none or the range is empty). List null → all four
  outputs 0.
- **Frame setup** `0x00621210(unit, b)` (unit in ESI): the frame range
  with a = +0x38; +0x44 := frame · 256, +0x40 := mode, +0x4E := e; returns
  1 when the drawn mode changed. Sequence branch of the frame advance
  `0x00623E00` (its AnimData branch and callers: `audio/triggers-2.md`
  §15): +0x4E := 0; flag 0x4000 cleared; p = +0x38 + +0x3C, minus +0x34
  once when ≥ +0x34; +0x38 := p; +0x48 −= +0x3C; frame setup with b = the
  old +0x38; drawn mode changed → flag 0x4000 set.

**Client mode-18 machine** (2026-10-09, REC-702..704; 1.14d asm). The
client player update `0x00463390` drives a mode-18 unit by the mode row
`0x00711E00` + 12·18 = {kind 2, advance 1, end 3}, in this order each
client update:

1. Used skill E (`0x00620250`) with flags bit 1 (moving): path step
   `0x004807C0` (`render/camera.md` §9); "arrived" = the step returned 0
   (`0x00650840`: no move, or the step that reached the last point). Arrived
   → E flags |= 2 and the client do `0x004C68F0` → `0x004C6680`
   (`cltdofunc`, table `0x00727BA8`).
2. Not arrived, unit flag 0x40 (+0xC4) clear and +0x4E ∈ {1, 2, 3} →
   the client do. +0x4E is the byte the previous update's advance wrote.
3. Advance: animation not complete (`0x006217C0`: +0x48 < 1) → frame
   advance `0x00623E00` (above).
4. Mode 18 with flag 0x4000 → graphics refresh `0x00470610`.
5. End: `0x006217C0` true → the mode end of `client/model.md` §20 r3
   (skill end `0x004611F0`, used skill := none, mode set NU / TN).

Request timing: the 0x4C / 0x4D mode request (codes 0x16 / 0x15,
`client/model.md` §8 rule 4 and rule 7) is a unit-queued message,
drained after the unit's per-type update (`client/model.md` §4 r5), so
its update loads the sequence at frame 0 (§2) and runs the client start
(`cltstfunc` table `0x00727A90`) with **no** mode-18 machine step; the
next update is the first step (do test on frame 0's byte, then the
advance to frame 1). Code 0x16's record: r2 = target unit type (EDX),
r3 = GUID (ECX) of `0x00463990` at `0x004C6ECF` (`0x004C6EB0`), r4 =
level. That request path is for units other than the local player.

Local player (2026-10-09, REC-670 settled; 1.14d asm): the local
client starts mode 18 itself, at the click, and never from a server
message:

1. The click's skill code (`ui/controls.md` §6 r7) goes to
   `0x00481030(code, P, a, b)`. Gate `0x00480BA0`: P's used skill
   lacks `interrupt` (row byte +7 & `[0x006CE284]` = 0x80) and P's mode
   is not 1 / 5 → nothing (no request, no send). Else it builds a
   record r0 := the side's skill id (`0x00643CE0` of `0x00620190` left /
   `0x006201D0` right), r1 := its owner GUID (`0x00643AD0`), r2 / r3 :=
   a / b, r4–r6 := 0, and maps the code: 5, 8, 0xC, 0xF → mode request
   **0x15** (point); 6, 7, 9, 0xA, 0xD, 0xE, 0x10, 0x11 → **0x16**
   (unit: a / b = type / GUID) (`0x0048114E`, `0x00481186`,
   `0x004811BE`, `0x004811F6`).
2. `0x00480C10(0x15 | 0x16, P, record, flag 0)` (`0x004810B5`) runs
   at once: the player machine's code 0x15 / 0x16 (`client/model.md` §8
   r4) → the client skill start `0x004C6140` (§8 r7 there): mode set
   18 and sequence frame 0 (§2), `cltstfunc`. Only then (request
   returned non-zero) stat 0x148 += 1 and the C→S message
   (`0x00480B40`).
3. The server sends the own client no skill message: the skill-mode
   row `0x00548090` of the per-mode table skips the unit's own client
   unless E flags bit 0x4 (dodge / avoid only; `sim/pathing.md` §10
   r2).
4. Timing: the click runs in the input part of the client loop pass
   (button handlers, held repeat `0x0044F039`), before the receive
   `0x0044F167` and the client update `0x0044C790`, and outside any
   update. So the click is the start (frame 0, no machine step, as the
   queued request's update for other units), and the **first mode-18
   machine step is the first client update after the click** (do test
   on frame 0's byte, advance to frame 1).

   ```
   on click(code): if gate(P) { r = {skill, owner, a, b, 0, 0, 0};
       mode_request(code_map(code), P, r, 0) /* mode 18, frame 0 */;
       send(code, a, b) }
   next client update: machine step 1 (rules 1-5 above)
   ```

Leap (`cltstfunc` 30 `0x004C8D90`, `cltdofunc` 43 `0x004C8C60`) and
Leap Attack (`cltstfunc` 30, `cltdofunc` 44 `0x004C8ED0`) share start
and hold:

1. Start: target point stored in E; E flags := 0x1080.
2. Do with flag 0x80 (frame 5, event 5:1): `0x004C85B0` (E flags :=
   0x1101; path to the point at run speed: `charstats` +0x41 << 8, a
   monster `monstats` +0x34 << 8) and the motion record `0x004C8670`
   (`render/unit-composite.md` §8).
3. Do with flag 0x100 (frame 11, event 11:1): landing check
   `0x004C8970(E, 1)` (`audio/triggers-2.md` §13.3). Not landed
   (record z ≠ 0 and position ≠ the point) → sequence position +0x38 :=
   10·256 (`0x006212C0`), +0x48 := (+0x48 & ~0xFF) + 256, do returns
   0; the advance then reaches frame 11 again with +0x4E = 1, so the
   next update repeats the do: **frame 11 is held while airborne**, for
   Leap and Leap Attack alike (same function, same frame-11 event).
   Monsters: `BaseId` 78 → position 8; 540 → 10, or 12 when
   `0x006417F0` distance < 2.
4. E flags 0x1101 include bit 1, so machine step 1 also steps the leap
   path; its arrival sets flag 2 and runs the do. Landed: with flag 2
   → path end `0x004C8890`, E flags := 0; without → E flags := 0x1000
   (not for monsters); the sequence then runs to its end. Leap Attack
   additionally takes its target
   (`0x00644500`); none → +0x48 := 0x400 (four frames left), else the
   strike set-up (`0x00620C10`, `0x00623C20`, sounds).

Whirlwind (`cltstfunc` 31 `0x004C9120`, `cltdofunc` 45 `0x004C9320`):

1. Start: player or monster only; path speed := `charstats` +0x40
   (walk) << 8 for a player, `monstats` `Velocity` +0x32 << 8 for a
   monster (`0x00648690` at `0x004C9291`); path computed; E flags := 1,
   E point := the path's last point. One path step is then (0x400 ·
   speed) >> 6 = walk << 12 (16.16 sub-tiles) along the Euclidean
   direction (`sim/pathing.md` §9.4). Measured: the first step is on
   update 3 (request update = 0; the update whose advance reaches
   sequence position 3), the last (partial) step on update
   a = 2 + ⌈(path length << 16) / step⌉. Read (REC-815, 2026-10-09):
   nothing in the client chain holds the step. The start's path compute
   `0x00649970` sets path flag 0x20, the point count +0x28 and +0x24 :=
   0, then `0x00648690` sets velocity +0x7C; machine step 1 calls
   `0x004807C0` → `0x00650840` from update 1 (E flags & 1, mode row kind
   2). `0x00650840` (`0x00650869`–`0x00650897`) needs flag 0x20, +0x28 >
   0, +0x7C ≠ 0, `0x006503F0` (point path: true), +0x24 < +0x28 and a
   non-zero step vector (`0x006502D0`); **any failed gate ends the
   path** (`0x006507B0`: snap to the sub-tile centre, flag 0x20 and
   +0x24 / +0x28 := 0, return 0 = arrived), so a no-move step cannot
   occur and return "moving". PROVISIONAL (REC-900): the 2-update
   delay is not explained by `0x00463390` / `0x004C9120` / `0x00650840`;
   open: whether the start runs later than the click for this request
   (e.g. `0x004C52E0` / `0x004648F0` target not ready) or the
   recorder's `px` is not path +0x00. N below uses the measured a.
   *Recorded (2026-10-09, PC 1, Windows; `record_anim.py` with extra
   entry hooks on `0x004C6F40`, `0x004C6140`, `0x00649970`,
   `0x00650840`, `0x004804E0`, RecWw `-seed 1234`, two runs):* the
   recorder's `px` **is** path +0x00 (unit +0x2C → path → +0x00 /
   +0x04), and the start is **not** late: the click's `0x004C6F40` →
   `0x004C6140` → path compute (count 0 → 1) all run in update 0
   (C = 247 / 262), and the client path **does** step in update 0
   (5168.500 → 5168.667, 4658.500 → 4658.164). Before the step of
   update 1 and of update 2 the position reads the start again
   (5168.500, 4658.500) and steps to the same first point; from update
   3 it advances (5168.833 …). So the first point is reached three
   times; something resets the client path position to the start twice
   between client updates (during the server-tick part of the loop:
   it is read back at the first `0x00650840` call of the next pass). It
   is not the position check `0x004804E0` (no call on the player in
   updates 0–5). The writer is still open (a hardware write watch on
   path +0x00 would name it); the measured a stays right either way.
2. Do while moving (flags & 3 = 1): +0x38 := 3·256, +0x48 := 0x500
   (loop A1 3–6). Rate 256: +0x48 runs 2048 − 256·i to 256 at update 7,
   then 1024, 768, 512, 256 per loop; it is 256 after the advance of
   updates 7, 11, 15, … (event 7:1, +0x4E = 1).
3. Do on arrival (flags & 3 = 3; `0x004C9320`: `0x0064EC10` /
   `0x00648C30` collision moves, `0x00644660` E flags := 0, sound stop
   `0x004BA840`, returns 1): no rewind and no mode change. Later do calls
   (+0x4E = 1, flags & 3 = 0) do nothing. The end is the machine's
   step 5 only (`0x006217C0`: +0x48 < 1 after the advance). End rule:
   mode 18 lasts **N = max(8, 4·⌈a / 4⌉)** client updates (request
   update included; the unit shows the next mode on update N): the
   arrival update if +0x48 was 256 before its advance, else the end of
   the current A1 3–6 loop.
   ```
   a = 2 + ceil((d << 16) / (walk << 12))
   N = max(8, 4 * ceil(a / 4))
   ```
   Check (`facts/client/anim/a1-cold-plains-whirlwind-bar.tsv`, step
   0x6000): whirl 1 d 8 (x −8·65536): steps on updates 3..24 (21 full +
   8192), a = 24, N = 24 (measured 24); whirl 2 d √197 ≈ 14.04 (x +1, y
   +14): steps on 3..40, a = 40, N = 40 (measured 40). Both arrivals fall
   on a loop end, so these facts equally fit "ends on the arrival
   update" (N = a); the binary read above (no end in `0x004C9320`) gives
   the rounding. Settles REC-671. Settled (REC-816, 2026-10-09): the
   update-N mode change is machine step 5. The facts' `f` / `F` / `s`
   columns are +0x44 / +0x48 / +0x4C, not +0x38 / +0x48 / +0x3C: `f`
   reads 768 at sequence index 4 (A1.3) and 1536 at index 7 (A1.6,
   event 7:1), where +0x38 would read 1024 / 1792. The 213 after update
   N − 1 is the rate `0x00623F50` (`sim/units.md` §4.7) taking the
   velocity branch (`0x006214A0`: mode 18 is V-skill and E flags & 1,
   not 0x1000; player w = 213), which writes only the speed +0x4C; +0x3C
   is written only by the cast and attack branches. So +0x3C stays 256,
   the update-N advance leaves +0x48 = 0 and `0x006217C0` (+0x30 ≠ 0:
   +0x48 < 1) ends the mode through `0x004611F0`; the local player gets
   no server skill message (local-player rule 3 above).

### 4. The table (`sequences.tsv`)

One row per (sequence, distinct frame list); together the rows of a
sequence name all 14 weapon classes once.

| Column | Meaning |
|---|---|
| `seqnum` | 1–23 (slot 0 is null; the table has 24 slots) |
| `classes` | COF weapon class codes sharing this record (`weaponclass.txt` order: hth bow 1hs 1ht stf 2hs 2ht xbw 1js 1jt 1ss 1st ht1 ht2 = 0–13) |
| `count` | frames; 0 = no record (frame list null: the unit plays the plain mode-18 animation) |
| `frames` | space-separated `MODE.frame` per frame index, MODE the `plrmode` code of byte +2 |
| `events` | comma-separated `index:code` for each frame with a non-zero event byte |
| `list_va` | address of the frame list in the 1.14d image (0 for none) |

The length (+4) and the count (+8) of every record are equal, so +0x34 =
+0x48 at load.

Sequences used by the bodies in this folder (all classes unless noted):

| `seqnum` | Skill | Frames | Events (index: code) |
|---|---|---|---|
| 10 | Whirlwind | 8: A1 0–3, A1 3–6 (A1 frame 3 twice) | 3:1, 7:1; no record for bow, xbw |
| 13 | Leap | 15: S1 0–14 | 5:1, 11:1, 14:1 |
| 14 | Leap Attack | S1 0–14, then A1 | 5:1, 11:1, then the strike: |
| | hth, 1js, 1jt, 1ss, 1st, ht1, ht2 | 22: + A1 5–11 | 16:1 (A1 frame 6) |
| | 1hs, 1ht | 25: + A1 6–15 | 16:1 (A1 frame 7) |
| | 2hs | 27: + A1 6–17 | 17:1 (A1 frame 8) |
| | 2ht, stf | 28: + A1 6–18 | 18:1 (A1 frame 9) |
| | bow, xbw | none | — |
| 16 | Dragon Claw family | `bodies-2.md` §2.27 (hth, ht1: 12; ht2: 16) | 6:1 (ht2 also 10:1) |
| 21 | Dragon Flight | 23: SC 0–9, KK 0–12 | 9:1 (flight), 14:1 (kick, KK frame 4) |

### 5. Users in 1.14d

`skills.txt` (`seqnum` ≠ 0, with `anim`): 1 Jab (SQ); 2 Sacrifice (A1);
4 Charge, SerpentCharge (SQ); 5 Conviction (none); 6 Inferno, Imp Inferno
(SQ); 8 Impale (SQ); 9 Fend (A1); 10 Whirlwind; 11 Double Swing, Frenzy;
12 Lightning, Chain Lightning; 13 Leap; 14 Leap Attack; 15 Double Throw;
16 Fists of Fire, Dragon Claw, Claws of Thunder, Blades of Ice; 18
GargoyleTrap, Arctic Blast, Horror Arctic Blast; 19 Dragon Talon (KK); 21
Dragon Flight; 22 DiabRun (SQ), Wearwolf, Wearbear (SC); 23 Blade Fury;
26, 27, 34, 41 DesertTurret, ArcaneTower, Mosquito, QueenDeath (monster
skills). Sequences 3, 7, 17, 20 have no 1.14d user.

A sequence is loaded only in mode 18, which a player enters for an `anim`
SQ skill (`use.md` §5.1). So `seqnum` is dead for Sacrifice, Fend (A1),
Conviction, Dragon Talon (KK: plain KK animation, the kicks repeat by the
srvdo 42 rewind, `bodies-2.md` §3.11), Wearwolf and Wearbear (SC).

## Constants & data dependencies

- Table `0x007483B8` (24 pointers, slot 0 null) → per-class arrays of
  14 records of 12 bytes {frame list, length, count}.
- Class map `0x00748418` (14 pairs {index, class code}, identity in
  1.14d).
- `skills.txt` `seqnum` (+0x13), `anim`; monstats +0x170 / +0x188
  (monsters only).

## Randomness

None.

## Edge cases & original bugs

1. A weapon class without a record (count 0: bow or crossbow for
   Whirlwind or Leap Attack) loads no sequence; the unit falls back to the
   mode-18 AnimData animation. `itypea1` = `mele` keeps a player from
   starting those skills with a bow (`use.md` §2).
2. `seqnum` ≥ 24 on a player skill reads past the 24-slot table (into the
   class map). No 1.14d player skill has one (26, 27, 34, 41 are monster
   skills; monsters take the `monseq` path).
3. The event lookups do not test the frame index against the length; the
   schedule (`sim/units.md` §4.2) never passes one beyond it.
4. An `anim` SQ skill with `seqnum` 0 gets no sequence (s ≤ 0) and plays
   the mode-18 AnimData animation.
5. The server never advances a player's sequence position: `0x00623E00`
   has no server caller for players (client `0x00463480`, `0x00463492`;
   server monsters `0x005A7670`, `0x005A8670`). The player frame-event
   handler reuses +0x38 bits 8+ for the event index (`use.md` §5.2 rule
   1, `0x006212C0`); only the schedule (event bytes by index) matters
   there.

## Test vectors

Schedule times use `sim/units.md` §4.2 with frame bonus 0, speed s, start
at game frame f.

| Input | Expected | Source |
|---|---|---|
| player, Leap (`seqnum` 13), any class | 15 frames S1 0–14; type-0 events (1, 0), (1, 1), (1, 2) at f + ⌈256·i/s⌉ for i = 5, 11, 14: f + 5, f + 11, f + 14 at s = 256 | `sequences.tsv`, `0x007475A0` |
| player, Leap Attack, class 2ht | 28 frames; strike index 18 = A1 frame 9 | `0x007477C8` |
| Leap Attack after the landing, restart `0x00553DC0(16)`, s = 256 | hth, 1hs, 1ht: strike event (1, 0) at f + 1; 2hs: f + 1; 2ht, stf: f + 2; in general f + max(1, ⌈256·(i − 16)/s⌉) for strike index i | `sim/units.md` §4.2 variants |
| player, Dragon Flight, s = 256 | 23 frames; events (1, 0) at f + 9 (flight), (1, 1) at f + 14 (kick) | `0x00748058` |
| player, Whirlwind, class bow | no record: no sequence | `sequences.tsv` row `10 bow,xbw 0` |
| player, Dragon Talon | mode KK, no sequence (only mode 18 loads one) | `0x005533D0` |
| table check | 23 sequences × 14 classes, each class in exactly one row per sequence; `count` = number of `frames` entries | `sequences.tsv` |

## Provenance

- Lookup `0x00663310` (`0x00663341` table read, `0x006632C0` class map,
  fatal line 0x533), `0x00643D00` (player +0x13; monster +0x170 / +0x188,
  skill 0xC7), accessors `0x006633B0`, `0x006633D0`, `0x006633F0`.
- Load `0x00621260` (+0x30, +0x34, +0x38 := 0, +0x3C := 0x100, +0x48,
  `0x00621210`, flag 0x4000 at `0x006212A2`); callers `0x00553425`
  (prepare, player mode 0x12 / monster mode 0xE test at
  `0x005533FA`–`0x00553414`), `0x0062442F`, `0x006244B0`.
- Frame queries `0x006634C0`, `0x00621920`, `0x00663410`, `0x00621210`;
  advance `0x00623E00` (`0x00623E07`–`0x00623E45`).
- `sequences.tsv`: image dump of `0x007483B8` and its frame lists through
  the 14-pair map `0x00748418`; mode names from `plrmode` row order
  (`sim/units.md` §2). D2MOO not used.

## Open questions

1. Recording: one Leap Attack (hth and 2ht) with a monster at range: the
   type-0 event ticks and arguments after the landing (strike index 16 /
   18, `bodies-2b.md` §6.12), confirming the restart timing above.
