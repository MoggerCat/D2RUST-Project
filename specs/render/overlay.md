# Spec: Render — Unit overlays (create, per-update advance, removal)

- **Status:** draft (2026-10-07, RE on 1.14d `Game.exe`; no capture yet).
  Every rule names its 1.14d address.
- **Target version:** 1.14d
- **Crate/module:** `d2-client::world_view` (overlay list per client unit)
- **Related specs:** `render/unit-composite.md` §5 r3–r4 (overlay draw:
  back / front split, frame, position, blend), `render/lighting.md` §6
  (overlay light row), `sim/rng.md` §7 (the seed the rolls use),
  `client/model.md` §5 (client update order), callers' owners
  (`client/msg-units.md`, `client/msg-skills.md`, `client/msg-ui.md`,
  `client/stat-lists.md`, `client/model.md` §15), `data/fields.tsv`
  (`overlay` columns)

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 34–43 |
| Inputs | 44–53 |
| Outputs / state changes | 54–59 |
| Rules | 60–61 |
|   1. The record (0xA8 bytes, `Fog_AllocClientMemory`, zeroed) | 62–83 |
|   2. Create (`0x00470390`: ECX = U, EDX = id; stack: kind, a, A, B, C, b) | 84–120 |
|   3. Per-update advance (`0x00470C30`) | 121–192 |
|   4. Randomness | 193–205 |
|   5. Create call sites (1.14d) | 206–232 |
| Constants & data dependencies | 233–244 |
| Edge cases & original bugs | 245–258 |
| Test vectors | 259–270 |
| Provenance | 271–283 |
| Open questions | 284–292 |
<!-- /index -->

## Summary

An overlay is an animated `overlay.txt` cel drawn on a client unit (auras,
hit flashes, level-up, shrine effects). Each client unit's graphics record
(unit `+0x54`) holds a singly linked list of overlay records at `+0x2C`.
`0x00470390` creates a record and pushes it at the head. The per-unit
client update advances every record once (`0x00470C30`) and removes the
ones whose kind says they ended. The draw (`unit-composite.md` §5 r4)
only reads the list. Nothing here runs on the server.

## Inputs

| Name | Type | Source |
|---|---|---|
| unit U, overlay id, kind, args | create call | callers (§5) |
| `overlay.txt` row | 0x84-byte record, data tables `+0xBBC`, count `+0xBC0` | `data/fields.tsv` `overlay` |
| expansion flag | `0x00408F20` (cached file test) | install |
| local player's client unit seed | unit `0x00463DD0` `+0x20` | `sim/rng.md` §7 |
| wall clock (kind 6 only) | `GetTickCount` | §3 r4 |

## Outputs / state changes

The overlay list of U's graphics record; one light per overlay row with
`Radius` ≠ 0 (`render/lighting.md`); up to 3 rolls on the local player's
client unit seed per create (§4).

## Rules

### 1. The record (0xA8 bytes, `Fog_AllocClientMemory`, zeroed)

| Offset | Field | Set by |
|---|---|---|
| +0x00 | kind 0–9 (§3) | create |
| +0x04 | overlay id (`overlay.txt` row) | create |
| +0x08 | arg A: kind 0 mode; kind 1 countdown; kind 2 / 6 "running" flag; kind 4 / 9 first follow-up id; kind 8 cycle counter | create, update |
| +0x0C | arg B: kind 4 / 9 second follow-up id | create |
| +0x10 | arg C: kind 9 state | create |
| +0x14 | rate, 1/256 frame per update | create |
| +0x18 | frame position, 1/256 frame (`f = +0x18 >> 8`) | create, update |
| +0x1C | frame count × 256 | create |
| +0x20, +0x24 | x, y draw offsets | create (§2 r8) |
| +0x28 | `LoopWaitTime` (ms) | create |
| +0x2C | update stamp (`GetTickCount` of the last update that ran it) | update |
| +0x30 | kind 6 wait start (ms) | update |
| +0x34 | `PreDraw` (back = ≠ 0) | create |
| +0x38 | kind 8 state | create |
| +0x3C | kind 8 active flag | create, §3 r6 |
| +0x40 | light handle (0 = none) | create |
| +0xA4 | next record | create, removal |

### 2. Create (`0x00470390`: ECX = U, EDX = id; stack: kind, a, A, B, C, b)

1. `id` < 0 or ≥ the overlay count (`+0xBC0`): nothing.
2. Classic install (`0x00408F20` = 0) and the row's `version` (+0x42,
   16-bit signed) ≥ 100: nothing.
3. U given: U's flag-ex word (+0xC8) has bit 0x40000 → nothing. U a
   monster (type 1) whose `monstats2` row (monstats +0x18) has `noOvly`
   (`0x004638A0(class, 2)`) → nothing.
4. Kind 8 and U already has a record with this id (`0x0046E100`): nothing.
5. Unless `id` ∈ {140, 141, 142, 158}: remove U's first record matching
   `id` (`0x0046F0C0`, §3 r9). Those four ids stack.
6. Allocate and zero the record (fatal 0xC49 when out of memory).
   +0x00 := kind, +0x04 := id, +0x1C := the frame count of the overlay
   cel file for U's direction byte (`0x00620100`) times 256 (`0x004DBE20`,
   the `unit-composite.md` §5.1 request with the row). A file that does
   not resolve or load gives +0x1C = 1, so the record never draws (§5 r4
   of `unit-composite.md`: `f` < 0 never holds).
7. Kind 2: +0x08 := 1. Kind 6: +0x18 := `roll(+0x1C)` (§4 draw 1).
   Every other kind: +0x08, +0x0C, +0x10 := A, B, C.
8. +0x20 := `Xoffset` (+0x54). +0x24 := `Yoffset` (+0x58) plus a height
   chosen by U (`0x006223A0`): player → `Height2` (+0x60); monster →
   `monstats2` `overlayHeight` h (`0x00451FE0`, byte +0x0C): h = 0 → +75,
   h = 1…4 → `Height1`…`Height4` (+0x5C…+0x68), h ≥ 5 → nothing added, no
   `monstats2` row → `Height1`; any other type or no U → `Height1`.
9. `Radius` (+0x74) ≠ 0: one light (`0x00474160(U, 1, InitRadius, 0xFF,
   Red, Green, Blue)`, `render/lighting.md` overlay row) → +0x40; when
   `Radius` ≠ `InitRadius` (+0x70) its target radius is set
   (`0x00474290`).
10. U's graphics record (`0x00463E10`: U +0x54) null → fatal 0xC7A (U
    null included). +0x34 := `PreDraw` (+0x48). Push at the list head
    (+0xA4 := old head, gfx +0x2C := record).
11. Kind 8: +0x38 := A (the state), then the sync of §3 r6 (`0x0046DEF0`).
12. a ≠ 0: +0x18 := `roll(a × 256)` (§4 draw 2; replaces rule 7's).
13. +0x14 := `AnimRate` (+0x6C) × 16; b ≠ 0: +0x14 += `roll(b × 16)`
    (§4 draw 3).
14. +0x28 := `LoopWaitTime` (+0x78).

### 3. Per-update advance (`0x00470C30`)

1. When: once per client unit per client update, from the per-unit
   update `0x00480810` (`0x004808B7`), before the per-type update
   (`client/model.md` §5 r2). U's graphics record null → fatal 0x2FE.
2. Walk: let `now` be `GetTickCount` read once. Repeat: take the first
   record in list order whose stamp (+0x2C) ≠ `now`; none → done; stamp
   it `now` and run r3–r5 on it. Each step restarts from the head, so
   records created or removed while the list runs are handled: a record
   created during the walk (stamp 0) runs in the same update. d2rs: the
   stamp is the client update number (edge case 1).
3. Keep test by kind (removal = §3 r8, then the walk goes on, with no
   advance for that record):
   - 0: kept while U's mode (`0x0046DA60`: `0x00621190`, through the draw
     identity substitution `0x00645270` when U's flags +0xC8 bit 3 is set,
     `unit-composite.md` §1.1) equals +0x08; else removed.
   - 1: +0x08 ≠ 0 → +0x08 −= 1, kept; +0x08 = 0 → removed.
   - 2: +0x08 = 0 → removed.
   - 6: when +0x08 = 0 (waiting): `GetTickCount − +0x30` < +0x28 →
     skipped this update (no advance; not removed); else +0x08 := 1.
   - 8: U no longer has state +0x38 (`0x00639DF0`) → removed. Else when
     +0x3C ≠ 0: +0x08 < 50 → +0x08 += 1; else the cycle step (r6).
   - 3, 4, 5, 7, 9: always kept here.
4. Advance: +0x18 += +0x14.
5. End: when +0x18 ≥ +0x1C:
   - 1, 3, 8: wrap, +0x18 −= +0x1C.
   - 0: nothing (the frame runs past the end and stops drawing).
   - 2: +0x08 := 0 (removed next update).
   - 4: remove it; then create kind 3 (loop) overlays A, then B, on U,
     each unless −1 (`0x00470390(U, A or B, 3, 0, 0, 0, 0, 0)`).
   - 5: hold the last frame: +0x14 := 0, +0x18 := +0x1C − 1, +0x08 := 1.
   - 6: wrap, +0x08 := 0, +0x30 := `GetTickCount` (hidden until r3's
     wait passes).
   - 7: remove it; create kind 4 overlay 140 with A = 141, B = 142.
   - 9: remove it; if U still has state C (`0x00639DF0`): create kind 8
     overlays A, then B (each unless −1) with state C.
6. Kind 8 cycle (`0x0046DEF0` at create, `0x0046DF40` step): the kind-8
   records of one unit form groups by state; only one group is active
   (+0x3C = 1) and draws.
   - Sync at create: walk the list from the head; at the first other
     kind-8 record with the same state copy its +0x3C and +0x08; at the
     first one of another state that is active the new record gets +0x3C
     := 0; none found → +0x3C := 1, +0x08 := 0.
   - Step (an active record reached 50, or a kind-8 record was removed,
     with S its state): when U has S, S is turned off (`0x00639DB0(U, S,
     0)`), U is tested for any state with `states` flag `aura`
     (`0x0063A250`, state-flag list 1); a negative answer sets this
     record's +0x08 := 0; S is turned back on. Then every kind-8 record of
     state S: +0x3C := 0, +0x08 := −1. Next state := the smallest kind-8
     state > S on U, else the smallest kind-8 state on U (wrap; state 0
     cannot be chosen: 0 means none). Every kind-8 record of that state:
     +0x3C := 1, +0x08 := 0.
7. The wall clock of kind 6: d2rs uses `t = 40 ×` (client updates since
   game start) ms, as `camera.md` §9 does for the shake; a fresh record's
   first wait always passes (in 1.14d +0x30 = 0 and the uptime exceeds
   any `LoopWaitTime`). Pixel checks take the recorded wait state.
8. Removal of one record (`0x0046E040`): not in U's list → nothing.
   Free its light (`0x004743D0`) when +0x40 ≠ 0, unlink, a kind-8 record
   runs the cycle step (r6) for its state, free the record.
9. Remove by id (`0x0046F0C0(U, id)`): U or its graphics record null →
   nothing; id invalid → fatal 0xCA9. Removes the first record in list
   order with +0x04 = id, or of kind 4 / 9 with +0x08 or +0x0C = id.
   Callers: create r5, `0x004B2390` (`client/msg-ui.md`, overlay 72),
   `0x004BCBB0`, `0x004D8660`, `0x004D9480`, `0x004D9630`.
10. Remove all (`0x0046F170`): every record of U in list order (fatal
    0xCCF without a graphics record). Callers: `0x004AD5F0`
    (`monsters/umod-callbacks.md`; then kind 5 overlays 52 and 156) and
    `0x004AFF60` (twice).
11. `0x0046E150(U, id, n)` sets +0x08 := n of every kind-1 record with
    this id (fatal 0x205 when a record with the id has another kind); it
    has no caller in `Game.exe`.

### 4. Randomness

Seed: the local player's client unit (`0x00463DD0`) +0x20, `roll` =
`0x0045C3E0` (`sim/rng.md` §2), for an overlay on any unit. Per create, in
order, each only when its condition holds:

1. kind 6: `roll(+0x1C)` → +0x18 (`0x004704DD`);
2. a ≠ 0: `roll(a × 256)` → +0x18 (`0x004705B6`);
3. b ≠ 0: `roll(b × 16)` added to +0x14 (`0x004705D7`).

A create that stops at §2 r1–r5 draws nothing. The follow-up creates of
§3 r5 pass a = b = 0 and kinds 3, 4, 8: no draws.

### 5. Create call sites (1.14d)

| Caller | Kind | Id | Owner of the trigger |
|---|---|---|---|
| `0x00464E50` | 2 | from the message | `client/msg-units.md` (S→C 0x11) |
| `0x004656C0` | 2, 4, 2, 3, 3 | 186; per site; 270; 122 and 275 | `unit-composite.md` §5 |
| `0x00470C30` | 3, 8, 4 | follow-ups | §3 r5 |
| `0x00480D20` | 2 (7 sites) | per site | `client/model.md` |
| `0x004AD5F0` | 5 | 52, 156 | `monsters/umod-callbacks.md` |
| `0x004AEDD0` | 5 | 227 | `sim/stat-lists.md` |
| `0x004B3380` | 3 | 72 | `client/msg-ui.md` |
| `0x004BC8D0` | 3 | 182, 182, 71 | `client/msg-units.md` |
| `0x004BCF60` | 3 | 71 | `client/model.md` §15 |
| `0x004C1AD0` | 6, a = 8 | 70 | `client/model.md` (item update) |
| `0x004C4C70` | 2 | 70 | `client/msg-stats-items.md` |
| `0x004BDCF0` (`ClientFn` 15) | 3 | 72 | `world/objects-client.md` §26.15: object 558 in mode 0, every 500 ms; other modes remove it. Create has no duplicate test for kind 3, so each call adds a record; a kind-3 overlay loops (§3) until removed |
| `0x004C6140`, `0x004C6680`, `0x004C6930`, `0x004C6AC0`, `0x004CA060` | 2 | skill row | `client/msg-skills.md` |
| `0x004C8970` | 2 | 80 | — |
| `0x004CF3C0`, `0x004CF800` | 2 | per site | client missile functions (pointers `0x0072A52C`, `0x0072A548`) |
| `0x004D06F0` | 2, 2 | 151; per site | as above (pointer `0x0072A588`) |
| `0x004D6D40` | 3 | 182 | `render/lighting.md` (`horadric_light`) |
| `0x004D8B90` | 7, 7, 4 | per site | pointer `0x0072A6A8` |
| `0x004D8E60`, `0x004D9480` | 3 | per site | pointers `0x0072A6B0`, `0x0072A6D0` |
| `0x004D9920`, `0x004D9C30` | 9, 4, 3, 8; 4 | state overlays | `client/stat-lists.md` |
| `0x004E3C50`, `0x004E3D70` | 2 | per site | client skill table `0x00727BA8` entries 84, 132 |
| `0x004F2450`, `0x004F4EC0` | 2 | per site, 85 | client skill functions |

## Constants & data dependencies

`overlay.txt` (`data/fields.tsv` rows `overlay`): `version` +0x42,
`Frames` +0x44 (not read here: the count comes from the cel file),
`PreDraw` +0x48, `Xoffset` +0x54, `Yoffset` +0x58, `Height1`–`Height4`
+0x5C–+0x68, `AnimRate` +0x6C, `InitRadius` +0x70, `Radius` +0x74,
`LoopWaitTime` +0x78, `Trans` +0x7C (draw), `Red`/`Green`/`Blue`
+0x7D–+0x7F. `monstats2` `noOvly` (flags +4 bit 2), `overlayHeight` (+0x0C).
`states` flag `aura` (state-flag list 1, data tables +0xD0). Fixed: the
stacking ids 140, 141, 142, 158; kind 7's follow-up 140 → 141, 142; the
kind-8 cycle length 50 updates; the monster default height +75.

## Edge cases & original bugs

1. The walk's stamp is `GetTickCount`: two client updates in one tick
   value (a loop catching up) would run no record in the second. d2rs
   counts every update (`camera.md` OQ8 records whether updates ever
   share a pass); a capture showing a frozen overlay frame settles it.
2. A file that does not load leaves +0x1C = 1: the record lives by its
   kind's rule but never draws (reproduce).
3. Kind 0 does not wrap: after one play the record stays, invisible,
   until U's mode changes.
4. The kind-8 cycle cannot select state 0 (0 is its "none").
5. The light is created before the graphics-record check of §2 r10; that
   check is fatal, so the leak never matters.

## Test vectors

| Input | Expected | Source |
|---|---|---|
| kind 2, `AnimRate` 16, cel 10 frames | +0x1C 2,560, +0x14 256; frame n after U's n-th overlay update (n = 1…9; frame 0 draws only when the create ran after U's overlay update of that client update); update 10: +0x18 = 2,560, +0x08 := 0, not drawn; update 11: removed | §2 r6, r13; §3 r3–r5 |
| kind 1, A = 3 | kept on updates 1–3 (A 2, 1, 0), removed on update 4 | §3 r3 |
| kind 4, ids A = 5, B = −1, end reached | record removed, one kind 3 overlay 5 created and advanced in the same update | §3 r2, r5 |
| create id 140 twice | two records (stacking id) | §2 r5 |
| create kind 8, state 40, U already has kind 8 of state 30 active | new record +0x3C = 0 | §3 r6 |

Synthetic (from the rules); no capture yet.

## Provenance

1.14d `Game.exe`: create `0x00470390` (asm `0x004703A1`–`0x004705E2`),
frame count `0x004DBE20`, height choice `0x006223A0`, expansion test
`0x00408F20`, `noOvly` test `0x004638A0`, id test `0x0046E100`; update
`0x00470C30` and its call `0x004808B7`; mode `0x0046DA60`; kind-8 sync
`0x0046DEF0` and cycle `0x0046DF40` (with `0x00639DB0`, `0x00639DF0`,
`0x0063A250` → `0x0063A130`); removal `0x0046E040`, `0x0046F0C0`,
`0x0046F170`, `0x0046E150`; graphics record `0x00463E10`. Call sites from
`all.asm` (`call 0x470390`, pushes read before each); pointer-table
references from `tools/ghidra/disasm.py xref`. `0x0046E1B0` (overlays
drawn by `0x00453470` outside the world view) is not covered here.

## Open questions

1. Which game event each call site of §5 without an owner stands for
   (the missile and skill function entries): the owners of those tables
   name them; the overlay engine does not depend on it.
2. Where a unit's overlay records are freed when the unit's graphics
   record is freed (no caller of `0x0046F170` / `0x0046E040` on the unit
   free path was found in this read).
