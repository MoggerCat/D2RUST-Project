# Spec: World — Objects part 3 (client object functions, `ClientFn` 0–18)

- **Status:** draft: every rule below was read from the 1.14d `Game.exe`
  disassembly (`tools/ghidra/disasm.py`, addresses inline) and the live
  1.14d `patch_d2` `objects.txt` (`ClientFn` column, 574 data rows); D2MOO
  1.10f was not used. No recording of these objects exists.
  Implemented, unverified: `d2-client::bridge::objects` (branch
  `claude/impl-final-objclient`; its PROVISIONAL choices are listed in
  `docs/handoff/impl-final-objclient.md`).
- **Target version:** 1.14d
- **Crate/module:** `d2-client` (client object update; the model is
  `client/model.md`), with the rows of `objects.txt` from `d2-data`
- **Related specs (§29 label):** `ui/control-panel.md` §5 r14 (pop-up
  slot), `ui/panels.md` §5 (UI pass), `render/camera.md` §2, §4,
  `data/fixups.md` §11–§13, `world/objects-2.md` §26.4 (shrine texts).
- **Related specs:** `world/objects.md` (part 1; §1 object data, §4
  animation at a mode change, §14 client messages), `world/objects-2.md`
  (part 2, §23 client side of 0x0E / 0x4D), `client/model.md` (§2 unit
  sets S and C, §5 update pass, §8 mode requests, §15 object mode
  requests), `audio/triggers-2.md` §20 (when objects make their mode
  sounds; read-only here), `audio/triggers.md` §7 (the mode sound call
  `0x004CB460`), `render/lighting.md` §8, §9 and open question 11
  (`0x004BC5E0`), `sim/units.md` §4.1 (mode set `0x00624690`), `sim/rng.md`
  §2 (seed step), `missiles/missiles.md` §R9.5 (`0x006416D0` distance),
  `client/msg-ui.md` §7 (S→C 0x50 code 36), `client/msg-units.md` §7 r10
  (`0x0046F870` graphics load), `world/quests-act5-2.md` §7.8 (object
  561, operate 69), `data/fields.tsv` (`objects` offsets), `data/fixups.md`
  §13 (`FrameCnt` × 256).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 50–61 |
| Inputs | 62–73 |
| Outputs / state changes | 74–81 |
| Rules | 82–83 |
|   25. Client object function dispatch | 84–220 |
|   26. The client object functions | 221–419 |
|   27. Client latches of the zoo and the preloads | 420–430 |
|   28. What d2rs must model for §25–§27 | 431–443 |
|   29. Object mouse-over label (`0x00454F30`, unit type 2; REC-239) | 444–522 |
| Constants & data dependencies | 523–544 |
| Randomness | 545–558 |
| Edge cases & original bugs | 559–576 |
| Test vectors | 577–611 |
| Provenance | 612–645 |
| Open questions | 646–661 |
<!-- /index -->

## Summary

Part 3 of `world/objects.md`; section numbers continue after part 2
(§25–§28). It owns the 19-entry client object function table (`objects`
column `ClientFn`, +0x1B4): when the client calls an entry (§25), what
each entry does (§26: client-only mode changes, `GetTickCount` timers,
draws on the object's own client seed, overlays, graphics loads, sound
requests, client unit spawns, the one C→S message), and the client-global
latches two of them use (§27). Every timer here runs on the wall clock
(`GetTickCount`, milliseconds); §25 r6 records the d2rs clock choice.
§29 owns the mouse-over label of a hovered object (text, anchor, pop-up).

## Inputs

| Name | Type | Source |
|---|---|---|
| U, the client object unit | client unit record (0xF4 bytes, zeroed at allocation `0x00620290`) | `client/model.md` §1, §2 |
| U's class row | `objects.txt` row (`ClientFn` +0x1B4, `Start`, `FrameCnt`) | `data/fields.tsv` objects 14–21, 63–70, 150 |
| U's mode, frame, client seed, U+0xD4 | +0x10, +0x44 (8.8 fixed), +0x20/+0x24, +0xD4 (u32) | `client/model.md` §1 |
| now | `GetTickCount()` (`[0x006CC260]`), u32 milliseconds | host clock (§25 r6) |
| local player P | `0x00463DD0` | `client/model.md` §3 |
| client quest flags | record `[0x007C0D43]` (`0x004B32D0`), bit test `0x0065C310(rec, q, b)` | `world/quests-status.md` |
| latches | bytes `[0x007C025D]`–`[0x007C0260]`, u32 `[0x007C0261]` | §27 |

## Outputs / state changes

Client-only state: U's mode, frame and U+0xD4; U's client seed; U's
overlays; graphics loads; one sound request (ClientFn 18); client-only
monster units (ClientFn 17); removal of a client-only object (ClientFn 9);
the latches of §27. One C→S message: 0x13 (ClientFn 13, §26.13). No
server state changes; no S→C message is read here.

## Rules

### 25. Client object function dispatch

1. **Table.** `0x004BDEE0` (ECX = U) reads `ClientFn` of U's class row
   (`0x00640E90(class)`, byte +0x1B4). ≥ 19 → fatal assert 0x546
   (`0x00408A60`). 0 → returns 1 without a call. Else it tail-calls entry
   `ClientFn` of the table `0x007277F0` (19 dwords, entry 0 null) with
   ECX = U and returns that entry's byte result. Entries (1.14d address,
   live rows from `patch_d2` `objects.txt`):

   | ClientFn | Address | Live rows (class, name) | §26 |
   |---|---|---|---|
   | 0 | null | 551 rows | — |
   | 1 | `0x004BDB90` | 1: 45 `AmbientSound` | 26.1 |
   | 2 | `0x004BD730` | 4: 67–70 ripples | 26.2 |
   | 3 | `0x004BD7C0` | 0 | 26.3 |
   | 4 | `0x004BD860` | 1: 110 `drinker` | 26.4 |
   | 5 | `0x004BD900` | 1: 112 `gesturer` | 26.5 |
   | 6 | `0x004BD9C0` | 1: 114 `turner` | 26.6 |
   | 7 | `0x004BDA60` | 2: 259, 373 hell skeleton spawns | 26.7 |
   | 8 | `0x004BDCA0` | 1: 152 `orifice` | 26.8 |
   | 9 | `0x004BDBA0` | 1: 478 `clientsmoke` | 26.9 |
   | 10 | `0x004BDB00` | 1: 528 `sbub` (ice cave bubbles) | 26.10 |
   | 11 | `0x004BDAF0` | 1: 557 (Baal's lair dummy) | 26.11 |
   | 12 | `0x004BDBF0` | 1: 546 `ancientsaltar` | 26.12 |
   | 13 | `0x004BDC10` | 1: 561 (invisible Ancient) | 26.13 |
   | 14 | `0x004BDCC0` | 1: 39 `fire` (RogueBonfire) | 26.14 |
   | 15 | `0x004BDCF0` | 1: 558 `fana` (frozen Anya) | 26.15 |
   | 16 | `0x004BDE00` | 2: 563, 569 Baal's portals | 26.16 |
   | 17 | `0x004BDE40` | 1: 567 `Zoo` | 26.17 |
   | 18 | `0x004BDD50` | 1: 568 `Keeper` | 26.18 |

   The `d2data` (classic) `objects.txt` has `ClientFn` 1–7 only (rows as
   above for 1, 2, 4–7); the live file is `patch_d2`.
2. **Call site A, every client update of U** (`0x00480810` → the object
   update `0x004BDFF0`, `client/model.md` §5 rule 2; S and C objects
   alike): first the generic object animation and light step
   `0x004BCBB0` (`render/lighting.md` §8); then `ClientFn` read through
   U's object data (U+0x14 → +0x00 → +0x1B4): ≤ 3 → the mode sound call
   `0x004CB460` (`audio/triggers.md` §7) and no client function; ≥ 4 →
   §25 r1, then the mode sound call only when the function returned
   non-zero.
3. **Call site B, C objects only** (`0x00463CC0`, the walk of the C
   sets missiles, objects, monsters in `0x00465AA0`, `client/model.md`
   §5 rule 3): after a unit's update (r2) the walk looks the unit up
   again by (type, GUID) in its C set (`0x007A5270` + type·0x200); if it
   still exists and is type 2 → §25 r1 again, result ignored (type 1 →
   `0x0046D780`, not this spec). So a C object runs `ClientFn` ≥ 4 twice
   per update (A then B) and `ClientFn` 1–3 once (B only); an S object
   runs `ClientFn` ≥ 4 once and `ClientFn` 1–3 never.
4. **Which objects are C objects (1.14d).** The creators of client-only
   type-2 units through `0x00466730` (flags 0x600000) are: the room
   preset pass `0x00466820` (from `0x0044C750`) for preset units whose
   +0x1C bit 0 is set — only the river presets set it (`0x00666170`,
   `0x006661A0`: objects 40–42 and 65, all `ClientFn` 0; every other
   preset record starts with +0x1C = 0 at `0x0066BF30`) — and the monster
   update `0x004B13A0` (`0x004B15B3`: class 478 `clientsmoke`, mode 0, at
   the monster's position). With the live tables `ClientFn` 1, 2, 3 are
   therefore never called in 1.14d (their classes 45, 67–70 are server
   presets, set S); they are specified for completeness and for modded
   tables.
5. **Notation for §26.** now = `GetTickCount()`; T = U+0xD4 (u32, 0 at
   creation); "T < now" is an unsigned compare. `step(U)` = one step of
   U's client seed +0x20/+0x24 (`sim/rng.md` §2), lo' its new low dword.
   `range(lo, hi)` = `0x004BC500(&U.seed, lo, hi)`: hi ≤ lo (signed) →
   lo, no step; else n = hi − lo, one step, n a power of two → lo + (lo'
   & (n − 1)), else lo + lo' mod n (unsigned). `set_mode(U, m)` =
   `0x00624690(U, m)` (`sim/units.md` §4.1: a different mode is written,
   flag 1 set, temporary stat lists dropped, then the animation re-init
   `0x00624390` — frame event +0x4E := 0, frame := frame bonus × 256,
   which is 0 for objects; the same mode only sets flag 1).
   `refresh(U)` = the graphics refresh `0x00470610(U, 0)`. `reinit(U)` =
   `0x00624390(U)` alone. "write mode m" = U+0x10 := m without
   `set_mode`. `End(m)` = `FrameCnt[m]` (already × 256 in the binary
   row, `data/fixups.md` §13) − 256, the last frame. `sound(U)` = the
   mode sound call `0x004CB460(U)`. Frame values are U+0x44 (signed,
   8.8). A raw `Start[k]` write stores the byte unshifted (no × 256).
6. **Wall clock (d2rs choice, recorded).** 1.14d reads `GetTickCount`
   inside ClientFn 4, 5, 6, 7, 9, 10, 14, 15, 18; their timing is real
   milliseconds, not game ticks, so 1.14d is not tick-exact here (two
   runs differ). d2rs needs a tick model choice; the conservative choice
   recorded here: the client update takes `now: u32` (wrapping
   milliseconds) as an input from the bridge, the live client passes the
   host clock, headless tests and replays pass a scripted value. All
   compares and sums below are u32 wrapping exactly as stated.
7. **Mode changes here are client-only.** No §26 mode change is sent to
   the server; the server's copy of an S object keeps its own mode, and
   a later S→C 0x0E code 3 (`world/objects-2.md` §23) overwrites the
   client mode. After a 0x0E mode change the mode sound call runs inside
   it (`audio/triggers-2.md` §20 r3).
8. **Animation set-up of an S object (PROVISIONAL, REC-440).** The
   client runs the object branch of `world/objects.md` §4 (r1–r4) on U's
   client seed: at S→C 0x51 (`client/msg-units.md` §1.3) in the mode
   byte, and at every 0x0E code 3 mode change (`client/model.md` §8 r5)
   in the new mode, the same mode included. Frame := `Start[m]` · 256;
   speed (+0x4C) := `FrameDelta[m]` when `Sync` ≠ 0, else
   `roll(d >> 3)` + d − (d >> 4), 0 when ≤ 0, at most 0x7FFF; the
   generic step `0x004BCBB0` adds this speed per client update.
   Measured, not traced (`facts/render/scenes/a1-town-arrival-ama`, the
   Wine recording's object cels at f 3–113): N2 (36) speed 125 and RB
   (39) 124 are the first draw of their seeds; the three torches (37,
   init 8 sets mode 2, so the server sends a 0x0E) are 191, 188, 199,
   the second draw, and count their frames from one tick later. With
   this rule every object frame of ticks 13, 73 and 113 matches. Not
   measured: whether `reinit(U)` and `set_mode(U, m)` of §26 also draw a
   speed; d2rs draws nothing there and steps by `FrameDelta[m]` until
   the next set-up.
9. **Generic step** `0x004BCBB0` (EAX = U; 2026-10-09 read, b2a;
   call site A, r2). m = U's mode, C = `FrameCnt[m]` (objects +0xD8 +
   4m, already × 256), f = frame U+0x44, s = speed U+0x4C (i16), row
   offsets per `data/fields.tsv`:
   1. C = 0x100 (one frame) → nothing.
   2. `CycleAnim[m]` = 0 (non-cycling):
      1. `IsDoor` (+0x13A) ≠ 0 → the door step `0x004BCB20` instead;
         return.
      2. f ≥ C − 256 (the last frame reached, tested **before** any
         advance): class 189 in mode 2 or 3 → write mode m + 1, f :=
         `Start[m+1]` · 256, `refresh(U)`, `reinit(U)`, return. Mode ≠
         1 → return (**the frame no longer advances**). Mode 1 → write
         mode 2; `OrderFlag2` (+0x133) = 1 → U flag +0xC4 |= 0x100000;
         objects +0x194 (`Parm7`) ≠ 0 → `0x0046C320(U, 0x153 when it is
         0xFF else 0x97, 4)`; f := `Start2` · 256; `refresh(U)`;
         `reinit(U)`; `Overlay` ≠ 0 and class ∈ {354, 355, 356, 397,
         405, 406, 407} → `0x0046F0C0(U, 0, 0x47)`; U flag bit 2 :=
         `Selectable[2]` (`0x00457460(U, 2, …)`); light from `Red`,
         `Green`, `Blue` with `Lit2` (`0x004BC580`, `render/lighting.md`
         §8); `HasCollision2` = 0 and `HasCollision1` ≠ 0 →
         `0x00623830(U)`; return.
      3. Else go on to the advance (3).
   3. **Advance**: class 12 → f −= s; f < 0 → f += C. Class 189 in mode
      3 → f −= s; f < 0 → write mode 4, f := `Start4` · 256,
      `refresh(U)`, `reinit(U)`. Every other unit: f += s; f ≥ C →
      non-cycling: f := C − 256 (clamp); cycling: f := `Start[m]` · 256
      + (f − C) (one wrap, back to the mode's start frame, not to 0).
   So a non-cycling mode other than 1 stops on its last frame for good
   (the clamp step, then no advance); mode 1 turns into mode 2 on the
   update **after** the clamp, from the end test, not in the clamp step.

### 26. The client object functions

#### 26.1 ClientFn 1, `0x004BDB90` (45 `AmbientSound`)

Returns 1; nothing else.

#### 26.2 ClientFn 2, `0x004BD730` (ripples 67–70): frame-counted ripple

1. First call: when the whole dword U+0xC8 (flag-ex) is 0: `step(U)`,
   T := lo' & 0x3F, U+0xC8 := 1.
2. (T & 0x3F) = 0 and mode ≠ 1 → write mode 1, `reinit(U)`.
3. T += 1.
4. Mode = 2 → `step(U)`, T += lo' & 0x3F, write mode 0, `reinit(U)`.
5. Returns 1.

T counts calls here (not milliseconds). Mode 2 comes from the generic
step `0x004BCBB0` at the end of a non-cycling mode 1. Edge: in an
expansion game U+0xC8 already has bit 0x2000000 at creation
(`client/model.md` §2 rule 6), so rule 1 never runs, T starts at 0 and
U+0xC8 is not overwritten.

#### 26.3 ClientFn 3, `0x004BD7C0` (no live row)

§26.2 rules 1–4, then `sound(U)`, returns 1 (so through call site A it
would sound twice per update).

#### 26.4 ClientFn 4, `0x004BD860` (110 `drinker`)

1. T < now → T := now + `range(7000, 12000)`; frame := raw `Start5`
   (+0x12E); `set_mode(U, 3)`; `refresh(U)`.
2. Mode = 3 and frame ≥ End(3) (signed; `FrameCnt3` +0xE4) → frame :=
   raw `Start0` (+0x129); `set_mode(U, 0)`; `refresh(U)`.
3. `sound(U)`; returns 0 (call site A adds no second sound call).

When rule 1 fires while U is already in mode 3, `set_mode` changes
nothing, the frame stays at raw `Start5` and the animation restarts
from there. Live `Start0` = `Start5` = 0.

#### 26.5 ClientFn 5, `0x004BD900` (112 `gesturer`)

1. T < now → T := now + `range(23000, 33000)`; frame := raw `Start5`;
   then `step(U)`: lo' odd → m := 3, even → m := 4; `set_mode(U, m)`;
   `refresh(U)`.
2. Mode ∈ {3, 4} and frame ≥ End(mode) (signed) → frame := raw
   `Start0`; `set_mode(U, 0)`; `refresh(U)`.
3. `sound(U)`; returns 0.

#### 26.6 ClientFn 6, `0x004BD9C0` (114 `turner`)

1. T < now → T := now + `range(20000, 30000)`; frame := raw `Start5`;
   `set_mode(U, 3)`; `refresh(U)`.
2. Mode ≥ 3 (unsigned) and frame = End(mode) exactly → frame := raw
   `Start0`; `set_mode(U, 0)`; `refresh(U)`.
3. `sound(U)`; returns 0.

The equality is reached because the generic step clamps a non-cycling
mode at its last frame (`0x004BCDA1`–`0x004BCDBA`: frame += speed; frame
≥ `FrameCnt[m]` → frame := End(m)).

#### 26.7 ClientFn 7, `0x004BDA60` (259, 373 hell skeleton spawns)

1. Mode 0: T < now → T := now + 10000; frame := raw `Start1` (+0x12A);
   `set_mode(U, 1)`; `refresh(U)`.
2. Mode ≥ 1: T < now → T := now + `range(4500, 7500)`; `set_mode(U, 0)`;
   `refresh(U)`.
3. Returns 1 (call site A then makes the mode sound call).

So mode 1 (and the mode 2 the generic step reaches at its end) lasts
10,000 ms from its start; mode 0 lasts 4,500–7,499 ms. now is read once,
before the mode test.

#### 26.8 ClientFn 8, `0x004BDCA0` (152 `orifice`)

U ≠ null and mode ≠ 0 → `0x004A30A0`: latch `[0x007C025D]` = 0 → latch
:= 1, graphics load `0x0046F870(211, 1)` (monstats 211 `duriel`,
`client/msg-units.md` §7 r10). Returns 1. Once per latch reset (§27).

#### 26.9 ClientFn 9, `0x004BDBA0` (478 `clientsmoke`)

1. T = 0 → T := now; returns 1.
2. now − T (u32 wrapping) ≤ 1000 → returns 1.
3. d := `0x006416D0(U, P)` (`missiles/missiles.md` §R9.5; P is read
   without a null test). d > 25 (signed) → remove U (`0x00465F00(GUID,
   2)`: the client-only removal of `client/model.md` §5 rule 5); returns
   0 (so no sound call follows, and call site B finds no unit).
4. Else T := now; returns 1.

The smoke therefore lives while the local player stays within distance
25 at the once-per-second checks. As a C object it is called twice per
update (§25 r3); the second call in the same update stops at rule 2.

#### 26.10 ClientFn 10, `0x004BDB00` (528 ice cave bubbles)

§26.7 with T := now + 3000 in rule 1 and `range(2500, 7500)` in rule 2.

#### 26.11 ClientFn 11, `0x004BDAF0` (557)

Returns 1; nothing else.

#### 26.12 ClientFn 12, `0x004BDBF0` (546 `ancientsaltar`)

U ≠ null and mode ≠ 0 → `0x004A30C0`: latch `[0x007C025E]` = 0 → latch
:= 1, graphics loads `0x0046F870(537, 0)`, `(538, 0)`, `(539, 0)`
(monstats `ancientstatue1`–`3`). Returns 1.

#### 26.13 ClientFn 13, `0x004BDC10` (561, the invisible Ancient)

1. U ≠ null and mode ≠ 0 → returns 1.
2. P none → returns 1.
3. Client quest flags (`0x004B32D0`): `0x0065C310(rec, 39, 0)` = 0 →
   returns 1; `0x0065C310(rec, 39, 4)` ≠ 0 → returns 1 (Rite of Passage:
   39.0 completed, 39.4 heard the invisible Ancient,
   `world/quests-act5-2.md` §7.1; a null record is fatal 0x1F in
   `0x0065C310`).
4. `0x006416D0(U, P)` ≥ 25 (signed) → returns 1.
5. P has a path (P+0x2C) → P's path stops (`0x00648730`).
6. The local-player command 0x13 on (type 2, U's GUID): `0x00481030`
   (ECX 0x13, EDX P, stack 2, GUID, 0): the action gate `0x00480BA0`
   returns 0 → nothing more in this step; else P's mode request code 2
   with record {2, GUID, 0, …} (`0x00480C10`, flag 0; `client/model.md`
   §8 rule 4 → `0x00480930(2, GUID)`): U found in set S → P's path
   reset (`0x00648B90(path, 0)`), and when U's client flag +0xC4 bit 0x4
   is set P faces U (`0x00621C00`) and the client skill start
   `0x004C6EB0` runs with {skill, −1, 2, GUID}; then **C→S 0x13**
   (9 bytes: 0x13, type u32 = 2, GUID u32; `0x004786A0`, appended to
   `outgoing`). `0x00480B40` sends nothing more for command 0x13.
7. `set_mode(U, 2)`; `refresh(U)`; returns 1.

The server answers 0x13 with operate 69 (`world/quests-act5-2.md` §7.8:
scroll message 20169). The client mode 2 is local (§25 r7), so the auto
interaction fires once per client copy of U in mode 0. Gate
`0x00480BA0` (P in ESI): returns 1 when P's current skill
(`0x00620250`) is none, its id is out of range, its `skills` row byte +7
has the bit of `[0x006CE284]`, or P's mode is 1 or 5; else 0.

#### 26.14 ClientFn 14, `0x004BDCC0` (39 `fire`, RogueBonfire)

now > T → the day-period object refresh `0x004BC5E0(U, 0)`
(`render/lighting.md` open question 11, answered there: only `EnvEffect`
≠ 0 objects; class 39 has `EnvEffect` 1), then T := `GetTickCount()` +
500 (a second read). Returns 1. So the bonfire re-applies its day/night
mode and light every 500 ms instead of only at a day-period change.

#### 26.15 ClientFn 15, `0x004BDCF0` (558 `fana`, frozen Anya)

now > T →
1. mode 0 → overlay 72 (`overlay.txt` row 72 `npcalert`) created on U:
   `0x00470390(U, 72, 3, 0, 0, 0, 0, 0)` (type 3: no seed draw,
   `audio/sound-table-2.md` overlay row);
2. mode ≠ 0 → overlay 72 removed: `0x0046F0C0(U, 0, 72)`;
3. T := `GetTickCount()` + 500.

Returns 1. `0x00470390` has no duplicate test for type 3 (only type 8
tests, `0x0046E100`), so in mode 0 a new overlay record is added every
500 ms; how long a type-3 overlay lives is the overlay owner's
(`render/unit-composite.md` owns it).

#### 26.16 ClientFn 16, `0x004BDE00` (563, 569 Baal's portals)

Mode = 1 and frame = `Start1` × 256 exactly (+0x12A, shifted) → write
mode 2, frame := 0, `refresh(U)`, `reinit(U)`. Returns 1.
PROVISIONAL: the generic step reaches mode 2 itself at the end of mode 1 (because the rule cannot hold after the step has advanced the frame at a non-zero speed); settled by REC-45.
Why: at call site A the generic step `0x004BCBB0` has already
advanced the frame in the same update, so with a non-zero speed the
frame is past `Start1` × 256 (live `Start1` 0, `FrameDelta1` 256) and
the rule cannot hold; the generic step then reaches mode 2 itself at
the end of mode 1. Whether the object speed field +0x4C is ever 0 in
mode 1 for these classes (speed source: the graphics refresh
`0x00470610`, not traced) decides it; a recording of Baal's portal
opening (level 131 → 132) settles it.

#### 26.17 ClientFn 17, `0x004BDE40` (567 `Zoo`)

1. Latch `[0x007C025F]` = 0 (`0x004A3110`) → returns 1.
2. `0x004A3120` ≠ 0 → returns 1: true when `[0x007C0260]` ≠ 0 and the
   unit (type 1, GUID `[0x007C0261]`) exists in set C (`0x004639B0`).
3. (x, y) := U's position (an object: its static path +0x0C, +0x10).
4. `0x004A3150(x, y)`: latch `[0x007C025F]` = 0 → nothing. Else one step
   of **P's** client seed (P+0x20/+0x24, P read without a null test);
   n := lo' mod 3 + 2 (2–4); for i = 0 … n − 1: o := i mod 2; create a
   client-only monster of class 149 (`chicken`) at (x + o, y + o)
   (`0x00466730(149, x + o, y + o, 1, 1)`, GUID from the client counter
   `[0x00711F30]`); `[0x007C0261]` := its GUID (−1 when the create
   fails); `[0x007C0260]` := 1.
5. Returns 1.

So while the latch is on, the zoo refills 2–4 chickens whenever the last
one created is gone. The latch is set only by S→C 0x50 code 36
(`client/msg-ui.md` §7: `0x004A3100`, `[0x007C025F]` := 1,
`[0x007C0265]` := u16@3, never read).

#### 26.18 ClientFn 18, `0x004BDD50` (568 `Keeper`)

now > T → `step(U)`; lo' mod 100 < 10 (unsigned) → sound request
`0x004B9A00(2505, U, 0, 0, 0)` (2,505 `barbarian_grunt_small_1`,
`audio/triggers-2.md` §20 r2); `step(U)` again (lo''); T :=
`GetTickCount()` + (lo'' mod 60) × 1000. Returns 1 (then the mode sound
call). Two draws per firing, whether or not the sound is requested.

### 27. Client latches of the zoo and the preloads

1. Bytes `[0x007C025D]` (orifice preload done, §26.8), `[0x007C025E]`
   (altar preload done, §26.12), `[0x007C025F]` (zoo on, §26.17),
   `[0x007C0260]` (zoo has spawned) are cleared together by `0x004A2390`
   (from `0x004A3E40`), `0x004A3410` (game start,
   `world/quests-status.md` §1 r5) and `0x004A42A0`. `[0x007C0261]`
   (last chicken GUID) is only written by `0x004A3150`.
2. d2rs: these belong to the client session (`ClientWorld` or its UI
   state), reset with the client quest state at game start.

### 28. What d2rs must model for §25–§27

1. Per client object: T (u32), the client seed, mode, frame; the class
   row's `ClientFn`, `Start0`/`Start1`/`Start5`, `FrameCnt`.
2. The call order per update: generic step, then (S and C) call site A,
   then the queue drain (`client/model.md` §5 rule 2), then for C
   objects call site B; the mode sound call per §25 r2 and the function
   returns.
3. Outputs to other layers: sound requests (ClientFn 3–6 via `sound`,
   ClientFn 18 directly) to the audio driver in that order; overlay
   create/remove and graphics loads as render outputs; C→S 0x13 into
   `outgoing`; client unit create/remove into the model's set C.

### 29. Object mouse-over label (`0x00454F30`, unit type 2; REC-239)

Read 2026-10-08. The hovered-unit draw `0x00454F30` has no UI owner
(`ui/panels.md` §5 step 1 and open question 1); this section owns its
object case. Its other unit types are open question 3. The pop-up it
fills is owned by `ui/control-panel.md` §5 r14.

1. **When.** Each UI pass (`ui/panels.md` §5 step 1) clears the pop-up
   slot, then calls `0x00454F30(U)` with U = the hover target
   (`0x00467A10`; which objects can be hovered: `client/model.md`, hover
   selectability `0x00466870`). Nothing is drawn when U is none, the
   open mode is 3, a store is open (`0x004B3230` ≠ 0) or the modal text
   screen is up (`0x004A0000` = 1). The label shows on every frame the
   object is hovered and is gone on the first frame it is not.
2. **Anchor** (GDI path, `0x004F51D0` = 0): x = px − `0x0045AFC0`(),
   y = py − `0x0045AFD0`() (`0x00454FF5`–`0x00455018`). (px, py) = the
   object's client pixel point (`0x00620650` / `0x006206B0`,
   `render/camera.md` §2), so x = px − cx_u + shiftX and y = py − cy_u
   + 8 (`render/camera.md` §4): the object's draw point without its
   `Xoffset` / `Yoffset`. Then y += `NameOffset` (record +0x168, i32,
   `0x00622270`; `0x004551F8`). Live `NameOffset`: −250 … 0, −70 on 141
   rows (portals, waypoints, doors, wells), −60 / −50 on chests, −60 …
   −235 on shrines (class 2: −145). The 3D path (not used by d2rs) takes
   the projected point (`0x004F6760`; not on screen → nothing), ∓ W/4 in
   open modes 1 / 2, and y + 16.
3. **Text**, by `SubClass` (record +0x167) in this order:
   1. Bit 2 (portals 59, 60): the destination's name `0x00453E70`
      (object data +4, the destination level): no `levels` record →
      string 5389 "No Level Name"; level 8 → 5048 "Den of Evil" when
      client quest byte 1 (`0x004B92E0(0, 1)`, `world/quests-status.md`
      §12) ≠ 0, else 5047 "Cave Level 1"; else the level's wide
      `LevelName` (`levels` +0x16E, `data/fixups.md` §11). Game type
      `[0x007A0610]` ∉ {0, 1} (multiplayer) appends `" (%s) "` with the
      owner's name; never in single player.
   2. Bit 7 (9 rows: classes 129, 247, 248, 250, 256–258, 289, 364): no
      label.
   3. Bit 6 (the 16 `Waypoint` rows): the object's room (`0x00620BB0`)
      none → no label; else name(U) (rule 4), string 3995 (" "), string
      3998 (newline) and the wide text of `LevelName` (`levels` +0xF5,
      `0x00524E20`, `data/fixups.md` §12) of the room's level
      (`0x0061A1B0`, `0x0061DB70`), concatenated in that order.
   4. Otherwise name(U).

   The text goes to `0x00502280(text, x, y, colour 0, centre 1)`
   (`0x004552EC`, `0x0045539C`, `0x004553BF`).
4. **name(U)** (`0x00464A60`, object case), the first rule that applies:
   1. `OpenWarp` (+0x1B8) ≠ 0 and mode 2 (classes 74, 194, 195, 386):
      the first tile unit (type 5) in the unit list of the object's room
      (room +0x74, link unit +0xE8; `0x00464A10`); none → the same search
      in each room of the room's near list (`0x00619790`, list order,
      the own room skipped). Found in room R → the wide `LevelWarp`
      (`levels` +0x1BE) of the level R's warp of that tile class leads to
      (`0x00464A30`: `0x0061A1D0(R, tile class)`, `0x0061DB70`); no such
      level → a null text, which the pop-up clears (nothing drawn). No
      tile in any room → rule 2.
   2. `IsDoor` (+0x13A) ≠ 0: mode 0 → string 3225 "Closed Door"; 4 or 5
      → 3228 "Blocked Door"; 6 → 3229 "Locked Door"; any other mode →
      3227 "Open Door".
   3. A shrine (`SubClass` bit 0, `0x00621B00`): string 10809 + the
      interact byte (object data +4, the shrine id from S→C 0x51,
      `client/msg-units.md` §1.3 r3). These are the `Patch_D2.mpq`
      English `patchstring.tbl` keys `ShrId0`–`ShrId22`, e.g. 10810
      "Refilling Shrine", 10816 "Combat Shrine", 10825 "Enirhs Shrine".
      The `d2exp.mpq` copy has other strings at these ids; the game reads
      the `Patch_D2` one (`ui/text.md` §2). The name stays the same after
      the shrine is used.
   4. `Lockable` (+0x171) ≠ 0 and object data +4 bit 0x80 (locked):
      string 3262 "Locked Chest".
   5. Otherwise the record's wide name (+0x40): the text of the `Name`
      key, else the miss text (`data/fixups.md` §13). Examples: `chest`
      → "Chest", `Waypoint` → "Waypoint", `Well` → "Well".
5. **Draw.** The rules of the pop-up slot are `ui/control-panel.md` §5
   r14. It has one slot, so any later `0x00502280` call in the same frame
   replaces the label. It is drawn at UI pass step 10 in the font current
   then (Font16 in the plain game view, r14 rule 1). The box is W = text
   width + 8 wide, centred on x, with its bottom at y + 2, clamped to the
   screen. It has a colour-0 mode-2 backing rectangle; the text is colour
   0 and the lines are centred.

## Constants & data dependencies

| Constant | Value | 1.14d |
|---|---|---|
| table, size | `0x007277F0`, 19 entries (0 null) | `0x004BDEE0` |
| fatal on `ClientFn` ≥ 19 | assert 0x546 | `0x004BDEF6` |
| drinker / gesturer / turner timer | `range(7000, 12000)` / `range(23000, 33000)` / `range(20000, 30000)` ms | `0x004BD874`, `0x004BD918`, `0x004BD9D4` |
| skeleton spawn timers | 10000; `range(4500, 7500)` ms | `0x004BDA7E`, `0x004BDAB9` |
| bubble timers | 3000; `range(2500, 7500)` ms | `0x004BDB1E`, `0x004BDB59` |
| smoke check period, distance | 1000 ms, > 25 | `0x004BDBBA`, `0x004BDBCE` |
| invisible Ancient distance | < 25; quest 39 bits 0, 4 | `0x004BDC5B` |
| bonfire / Anya period | 500 ms | `0x004BDCDF`, `0x004BDD24` |
| Anya overlay | 72 (`npcalert`), type 3 | `0x004BDD16` |
| Keeper | 10 % per firing; next in 0–59 s | `0x004BDD93`, `0x004BDDDD` |
| zoo spawn | class 149, 2–4 units | `0x004A31C1` |
| preloads | 211 (flag 1); 537, 538, 539 (flag 0) | `0x004A30B4`, `0x004A30CB` |

Columns read: `objects` `ClientFn` (+0x1B4), `Start0` (+0x129),
`Start1` (+0x12A), `Start5` (+0x12E), `FrameCnt0`–`7` (+0xD8, × 256),
`EnvEffect` (via `0x004BC5E0`); `overlay` row 72; `monstats` rows 149,
211, 537–539.

## Randomness

All draws are client-side and never reach the server.

1. ClientFn 2/3: first call one `step(U)` (when U+0xC8 = 0); one
   `step(U)` per update in mode 2.
2. ClientFn 4, 6, 7 (mode ≥ 1 branch), 10 (mode ≥ 1 branch): one
   `range` on U's seed per timer firing.
3. ClientFn 5: `range(23000, 33000)` then one `step(U)` (mode parity),
   per firing.
4. ClientFn 18: two `step(U)` per firing.
5. ClientFn 17: one step of the local player's client seed per spawn
   wave (`audio/sound-table-2.md` already lists it).

## Edge cases & original bugs

1. ClientFn 1–3 are unreachable with the live tables (§25 r4).
2. ClientFn 2/3 test and overwrite the whole flag-ex dword U+0xC8 as an
   "initialised" marker; in an expansion game the test fails (§26.2).
3. ClientFn 4–6 write a raw `Start` byte as an 8.8 frame (no × 256), and
   `set_mode` then overwrites it unless the mode was already the target
   (§26.4). Live `Start` values are 0.
4. ClientFn 4 tests `FrameCnt3` for its end whatever the mode (it only
   runs the test in mode 3). ClientFn 6 tests equality, reached through
   the clamp (§26.6).
5. ClientFn 9 and 17 read the local player without a null test; ClientFn
   13 tests it.
6. ClientFn 15 adds an overlay every 500 ms in mode 0 (§26.15).
7. ClientFn 16 may never fire (§26.16, PROVISIONAL, REC-45).
8. ClientFn 14, 15, 18 set T from a second `GetTickCount` read after
   the work, not from now.

## Test vectors

Synthetic; seed {1, 666} for U (or P), so lo' = 1,791,398,751 (hi' 0)
and the next lo'' = 791,599,131 (`world/objects.md` test vectors).

| Input | Expected | Rule |
|---|---|---|
| ClientFn 0 | dispatcher returns 1; mode sound call at site A | §25 r1, r2 |
| ClientFn 19 in a modded row | fatal 0x546 | §25 r1 |
| S object, ClientFn 2, any state | never called | §25 r3 |
| ClientFn 7, mode 0, T 0, now 5,000 | T := 15,000; mode 1 | §26.7 r1 |
| ClientFn 7, mode 2, T 15,000, now 15,001 | 1,791,398,751 mod 3,000 = 2,751 → T := 15,001 + 7,251 = 22,252; mode 0 | §26.7 r2 |
| ClientFn 10, mode 1, T 0, now 1 | 2,500 + 1,791,398,751 mod 5,000 = 6,251 → T := 6,252; mode 0 | §26.10 |
| ClientFn 4, T 0, now 100 | T := 100 + 7,000 + 1,791,398,751 mod 5,000 = 10,851; mode 3; one sound call; returns 0 | §26.4 |
| ClientFn 5, T 0, now 100 | T := 100 + 31,751 = 31,851; lo'' 791,599,131 odd → mode 3 | §26.5 r1 |
| ClientFn 6, mode 3, `FrameCnt3` 21, frame 5,120 | mode 0 | §26.6 r2 |
| ClientFn 6, mode 3, frame 5,119 | stays mode 3 | §26.6 r2 |
| ClientFn 9, T 0, now 7,000 | T := 7,000; returns 1 | §26.9 r1 |
| ClientFn 9, T 7,000, now 8,000 | returns 1 (1,000 ≤ 1,000) | §26.9 r2 |
| ClientFn 9, T 7,000, now 8,001, distance 26 | U removed; returns 0 | §26.9 r3 |
| ClientFn 9, T 7,000, now 8,001, distance 25 | T := 8,001 | §26.9 r4 |
| ClientFn 13, mode 0, 39.0 set, 39.4 clear, distance 24, gate 1 | C→S 0x13 {2, GUID}; U mode 2 | §26.13 |
| ClientFn 13, 39.4 set | nothing | §26.13 r3 |
| ClientFn 17, latch on, no chicken, P seed {1, 666} | 1,791,398,751 mod 3 = 0 → n 2: chickens at (x, y), (x + 1, y + 1) | §26.17 |
| ClientFn 18, U seed {1, 666}, now > T | lo' mod 100 = 51 → no sound; lo'' mod 60 = 51 → T := read + 51,000 | §26.18 |
| ClientFn 2, U+0xC8 0, seed {1, 666} | T := 31, U+0xC8 := 1, no mode change, T := 32 | §26.2 |
| hovered `chest` (class 5, `NameOffset` −50, unlocked) standing on the local player's subtile, 800 × 600, open mode 0, no shake | pop-up "Chest", colour 0, centred on x = 400, y = 300 − 16 + 8 − 50 = 242 (box bottom 244) | §29 r2, r4.5, r5 |
| hovered shrine, interact 1 (any mode) | "Refilling Shrine" (string 10810) | §29 r4.3 |
| hovered locked chest (interact bit 0x80, `Lockable` 1) | "Locked Chest" (3262) | §29 r4.4 |
| hovered door (`IsDoor` 1), modes 0, 1, 2, 4, 6 | "Closed Door", "Open Door", "Open Door", "Blocked Door", "Locked Door" | §29 r4.2 |
| hovered portal 59, destination level 8, quest byte 1 = 0 / ≠ 0, single player | "Cave Level 1" / "Den of Evil" | §29 r3.1 |
| hovered waypoint in Blood Moor (level 2) | "Waypoint" + " " + newline + "Blood Moor" | §29 r3.3 |
| hovered object with `SubClass` bit 7 (e.g. class 129) | no label | §29 r3.2 |
| the hover target leaves the object | no label on that frame (slot cleared at UI pass step 1) | §29 r1 |

## Provenance

- 1.14d `Game.exe` via `tools/ghidra/disasm.py fn`: dispatcher
  `0x004BDEE0`; table `0x007277F0` read from the image; callers
  `0x004BDFF0` (from `0x00480810`) and `0x00463CC0` (from `0x00465AA0`);
  entries `0x004BD730`, `0x004BD7C0`, `0x004BD860`, `0x004BD900`,
  `0x004BD9C0`, `0x004BDA60`, `0x004BDAF0`, `0x004BDB00`, `0x004BDB90`,
  `0x004BDBA0`, `0x004BDBF0`, `0x004BDC10`, `0x004BDCA0`, `0x004BDCC0`,
  `0x004BDCF0`, `0x004BDD50`, `0x004BDE00`, `0x004BDE40`; helpers
  `0x004BC500`, `0x00624690`, `0x00624390`, `0x00470610` (entry only),
  `0x004BCBB0` (the clamp), `0x004BCF60`, `0x004A30A0`, `0x004A30C0`,
  `0x004A3100`, `0x004A3110`, `0x004A3120`, `0x004A3150`, `0x0046F870`
  (entry), `0x00466730`, `0x00466820`, `0x00666170`, `0x006661A0`,
  `0x0066BF30`, `0x004B15B3`, `0x00481030`, `0x00480B40`, `0x00480BA0`,
  `0x00480C10`, `0x00480930`, `0x006416D0`, `0x004B32D0`, `0x0065C310`,
  `0x00470390` (entry), `0x00620290`; data refs of `0x007C025D`–
  `0x007C0261` from `re/exports/index/datarefs.tsv`.
- Live rows: `patch_d2` `objects.txt` column `ClientFn` (index 155 of
  the header, `data/fields.tsv` objects 150), `Start*`, `FrameCnt*`;
  `overlay.txt` row 72; `monstats.txt` `hcIdx` 149, 211, 537–539. Script
  kept outside the repo (`scratch-objclient`).
- §29 (2026-10-08, `all.asm`): `0x00456EE0` (UI pass call site
  `0x00456F58`–`0x00456F5F`), `0x00454F30` (`0x00454F30`–`0x0045548B`,
  jump table `0x00455490`), `0x00464A60` (`0x00464A60`–`0x00464C1F`,
  jump table `0x00464C20`), `0x00464A10`, `0x00464A30`, `0x00453E70`,
  `0x00621B00`, `0x00622270`, `0x004B92E0`, `0x004B3230`; record field
  offsets from `data/fields.tsv` (`objects` +0x13A `IsDoor`, +0x167
  `SubClass`, +0x168 `NameOffset`, +0x171 `Lockable`, +0x1B8 `OpenWarp`;
  `levels` +0xF5 `LevelName`). Live rows: `patch_d2` `objects.txt`
  (`SubClass`, `NameOffset`, `OpenWarp`, `IsDoor`, `Lockable` columns).
  Strings: English `string.tbl` (d2data), `patchstring.tbl` (Patch_D2,
  1,179 entries; read with `d2-formats` `Archive::read`, as the archive
  has no listfile) by element number (`ui/text.md` §2).

## Open questions

1. §26.16: can ClientFn 16 fire (object speed in mode 1)? A recording of
   Baal's portal opening settles it.
   PROVISIONAL: ClientFn 16 never fires; the generic step reaches mode 2 (because the frame has advanced past `Start1` × 256 before the test); settled by REC-45.
2. What `0x0046F870`'s flag 1 (orifice preload) changes against 0
   (`client/msg-ui.md` open question 1, same point).
3. §29: no owner yet for the other unit types of `0x00454F30`: player
   (name at y − 0x48 through `0x005022F0`), monster (`0x00454AD0` hover
   bar, `0x00454A70`), item (`0x004527D0`), tile (level warp text at the
   tile's centre). This needs a UI spec for the hovered-unit draw.
4. §29 r3.3: the line order on screen of the waypoint text "Waypoint ⏎
   level" depends on how the pop-up draws a 2-line text (bottom-up,
   `ui/control-panel.md` §5 r14 rule 6). REC-239 capture (a hovered
   waypoint) checks it.
