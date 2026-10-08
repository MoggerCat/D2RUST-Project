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
|   3. Frame queries | 112–136 |
|   4. The table (`sequences.tsv`) | 137–168 |
|   5. Users in 1.14d | 169–185 |
| Constants & data dependencies | 186–194 |
| Randomness | 195–198 |
| Edge cases & original bugs | 199–218 |
| Test vectors | 219–233 |
| Provenance | 234–248 |
| Open questions | 249–254 |
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
