# Spec: Render — Unit composites (COF/DCC files, variants, directions, frames, offsets)

- **Status:** draft (2026-10-06, RE on 1.14d `Game.exe` plus surveys of the
  1.14d archives; no capture yet). Every rule names its 1.14d address; the
  pixel proof is the capture case of §Test vectors (unverified until it
  runs).
- **Target version:** 1.14d
- **Crate/module:** `d2-client::composite` (slot order, `ComponentResolver`),
  `d2-client::world_view` (`ViewRules::unit_pose`, `component_frame`),
  `d2-client::rules` (`ViewSource::unit_offset`)
- **Related specs:** `formats/cof.md`, `formats/dcc.md`, `formats/dc6.md`
  (file layouts), `formats/animdata.md` (§5 composer use), `render/camera.md`
  (§4 draw position X, Y), `render/sprite-placement.md` (cel placement),
  `render/draw-order.md` (which units are listed, passes, keys),
  `client/render-pipeline.md` §A7, §B4, `data/callbacks.md` §5 (monstats2
  component choices), `data/fixups.md` §8 (monster COF name)

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 43–55 |
| Inputs | 56–66 |
| Outputs / state changes | 67–73 |
| Rules | 74–75 |
|   1. Which draw path | 76–122 |
|   2. COF file | 123–183 |
|   3. Direction and frame | 184–232 |
|   4. Pre-test: COF box culling | 233–242 |
|   5. The slot loop (`0x00470EC0`) | 243–363 |
|   6. Component file and cel | 364–397 |
|   7. Colormap source per component | 398–422 |
|   8. Extra offsets (`0x004DA0B0`, `0x004DA0D0`, `0x004DA0F0`) | 423–483 |
|   9. Single-cel units (missiles, items) | 484–498 |
|   10. d2rs mapping | 499–513 |
| Constants & data dependencies | 514–527 |
| Randomness | 528–531 |
| Edge cases & original bugs | 532–546 |
| Test vectors | 547–567 |
| Provenance | 568–615 |
| Open questions | 616–654 |
<!-- /index -->

## Summary

A unit of type 0–2 (player, monster, object) is drawn as a **composite**:
one COF chosen by (token, mode, weapon class) lists up to 16 component
layers and, per direction and frame, the order to draw them in; each layer
is one DCC (DC6 for a few bosses) chosen by (token, component, armor class,
mode, weapon class). Missiles (3) and items (4) draw one cel from one file.
This spec owns the file names, the variant (armor class) of each
component, which colormap source each component uses (not the maps
themselves), the direction and frame indices, the slot loop and the
per-unit pixel offsets added to the draw position. Placement of a cel at
(X, Y) is `sprite-placement.md`; X, Y is `camera.md` §4.

## Inputs

| Name | Source |
|---|---|
| unit type, class, mode, direction (0–63), frame | client unit (§3); the S→C owners of each |
| equipped items (players) | client inventory (`items/inventory.md`) |
| monster component choices (16 bytes) | client monster data, from the S→C monster spawn owner |
| client motion record (§8) | client effects (§8 lists the creators) |
| tables | `plrtype`, `plrmode`, `monmode`, `objmode`, `composit`, `armtype`, `monstats`, `monstats2`, `objects`, `missiles`, `charstats`, `armor`/`weapons`/`misc`, `compcode` (`data/loading.md`) |
| files | COF, DCC, DC6 from the archives (`client/assets.md`) |

## Outputs / state changes

Per drawn unit: the COF, its direction and frame row, and per slot the
component's file, file direction and frame, colormap source and draw
position. One client-state write happens during the draw (§3 r5); the
motion record of §8 is updated in the unit update, not the draw.

## Rules

### 1. Which draw path

The unit draw `0x00471EC0` (ECX unit, EDX light, stack X, Y, flag a, flag
b) returns at once for type 5. Types 0–2 (`0x004DB180`) take the composite
path `0x00470EC0` (§2–§7); types 3 and 4 draw one cel (§9). Before either,
X and Y get the unit's extra offsets (§8). The unit's draw identity (type,
class, mode) passes through `0x00645270` everywhere below (a substitution
when the unit's flag-ex bit 3 is set; §1.1); mode is
`0x00621190`: types 0 and 1 with a running sequence (unit +0x30 ≠ 0) use
the sequence mode (+0x40), else the unit mode (+0x10).

#### 1.1 Draw identity substitution (`0x00645270`)

Inputs (type, class, mode) start as the unit's own. Nothing changes
unless flag-ex (`+0xC8`) bit 3 is set. Then the states of the list at
data tables `+0x17C` (count `+0x180`, i16 state ids; built at load) are
tested in list order; for the first one the unit has (`0x00639DF0`) whose
`states` `gfxtype` (`+0x2D`) is 1 or 2 (others are passed over):

| `gfxtype` | New type | New class | Mode |
|---|---|---|---|
| 1 | 1 (monster) | `gfxclass` (`+0x2E`) | if the unit is a player: player→monster map below |
| 2 | 0 (player) | `gfxclass` | if the unit is a monster: monster→player map below |

Player→monster (`0x00645190`): `m = T1[mode]`, T1 (`0x006EB348`, by
player mode 0…19) = 0, 1, 2, 15, 3, 1, 2, 4, 5, 6, 7, 4, 11, 8, 9, 10, 11,
12, 14, 13; then, while the class's `monstats2` mode bit `m` (`+0xF0`,
mDT … bits) is clear: `m` := fallback(`m`), with fallback WL, GH, A1 → NU;
A2 → A1; BL → GH; SC → A1; S1 → NU; S2, S3, S4 → S1; DD, KB, SQ → NU;
RN → WL; DT and anything else → NU (NU ends the loop). Monster→player
(`0x006EB308`, by monster mode 0…15): 0, 1, 2, 4, 7, 8, 9, 10, 13, 14,
15, 16, 17, 19, 18, 3, except `gfxclass` 6 with monster mode 4 → 12.
In 1.14d the states with `gfxtype` ≠ 0 are 63 `dopplezon` (2, class 0),
93 `valkyrie` (2, class 0), 119 `shadowwarrior` (2, class 6), 139 `wolf`
(1, class 430), 140 `bear` (1, class 431), 176 `monsterset` (1, class
135), 177 `delerium` (1, class 212).

**Linked-unit inventory.** Where a draw reads the unit's inventory
(`+0x60`: §5.1 r3, the shadow draw `render/blend-modes.md` §5 r3), it
uses the inventory of the linked unit (`0x004639D0`: flag-ex bit 10, the
unit of type `+0x94` and GUID `+0x98`) instead when flag-ex bit 3 is set,
the unit has a state with `states` flag `bossinv` (`0x0063A7B0(unit,
0x25)`: state-flag list 37 at data tables `+0xCC + 4·37`, tested against
the unit's state bits) and the linked unit exists. In 1.14d only state 63
`dopplezon` has `bossinv`: the Decoy monster is drawn as an Amazon
wearing its owner's items.

### 2. COF file

Composed by `0x0064F5B0` (full form, `param_8` = 1) as

`<root>\<T>\COF\<T><M><W>.COF`

with root `DATA\GLOBAL\CHARS` (type 0), `DATA\GLOBAL\MONSTERS` (1),
`DATA\GLOBAL\OBJECTS` (2). Each part is the first 3 bytes of a 4-byte code
with every 0x20 turned into 0, so a part ends at its first space (the
`fixups.md` §8 convention).

| Part | Player | Monster | Object |
|---|---|---|---|
| T token | `plrtype` token of the class | `monstats` Code (+0x10) | `objects` Token |
| M mode | `plrmode` token of the mode, then the player override table (r2) | `monmode` token, then the monster override table (r2) | `objmode` token |
| W weapon class | §2.1 | §2.1 | `hth` |

1. The short form (no root, `<T><M><W>`) is the AnimData key (`animdata.md`
   §5); both forms come from the same function.
2. Mode override tables: pairs (token, mode) at `0x00745900` (players,
   count `[0x00745910]`) and `0x00745914` (monsters, count
   `[0x0074591C]`); every pair whose mode equals the unit's replaces M (the
   last match wins). Both are initialized `.data` with no writer (only the
   two reads in `0x0064F5B0`): players (`gh`, 18), (`gh`, 19), count 2;
   monsters (`gh`, 13), count 1. So player SQ (18) and KB (19) and monster
   KB (13) use the GH (get hit) COF, e.g. `AMGH1hs.COF` for a knocked-back
   Amazon.
3. Player mode 11 (TH, throw): if W is not one of `1hs`, `1ht`, `1js`,
   `1jt`, `1ss`, `1st`, W becomes `hth`.
4. A COF that is not in the archives: the unit is not drawn (the gfx mode
   node is missing, `0x00470EC0` returns 0; load path `0x0046E5A0` /
   `0x0046FA80`).

#### 2.1 Weapon class (`0x0064F060`, via `0x0064F380`)

- Monster: `hth` in mode DT (0) or DD (12) unless the monstats2
  `compositeDeath` bit is set (`0x004638A0` bit 16); else monstats2 BaseW
  (+0x10). No monstats2 row: `hth`.
- Object: `hth`.
- Player: modes DT (0) and DD (17) → `hth` in the COF name (`0x0064F5B0`)
  but the `charstats` weapon class (+0x4C) in the component request
  (§5.1, which calls `0x0064F380` directly); every 1.14d `baseWClass` is
  `hth`, so both agree. Otherwise the
  hand item is the valid item in body location 4 if its `component` is 5 or
  6, else the valid item in location 5 (with its component). No hand item,
  or its component not 5/6 → the `charstats` row's weapon class (+0x4C).
  Two hand items, both of item type 45: Barbarian (class 4) → `1ss` /
  `1st` / `1js` / `1jt` by the two items' type classes (r-dual below);
  Assassin (class 6) → `ht2`. Else the hand item's
  `2handedwclass` when the grip test `0x0063D340` returns 2, else its
  `wclass`.
- r-dual (Barbarian, `0x0064F1EE`–`0x0064F2D5`): `A` = the weapon in use
  (inventory `+0x1C`, `0x0063BEF0`); with none, the right-hand item
  (location 4) is taken and made the weapon in use (`0x0063D1D0`: a
  draw-time write). `B` = the other hand item. Type class
  (`0x00629FE0`): the item's `wclass` (`weapons` `+0xC0`) looked up in
  the static table `0x007446A0` (8 entries, `[0x007446E0]` = 8): `bow`
  1, `1hs` 2, `1ht` 3, `stf` 4, `2hs` 5, `2ht` 6, `xbw` 7, `ht1` 12,
  otherwise 0. Result: (`A`, `B`) = (2, 3) → `1js`; (3, 3) → `1jt`;
  (3, 2) → `1st`; anything else → `1ss`.

### 3. Direction and frame

1. **Direction** `dir64` (0–63): dynamic path direction (`0x006487F0`)
   for types 0, 1, 3; static path +0x1C byte for types 2 and 4
   (`0x00620100`).
2. **Frame**: unit +0x44 >> 8; frame count unit +0x48 >> 8, objects
   `FrameCnt[mode]` (`0x00621810`). The draw reads neither AnimData nor
   the COF rate: speed and frame advance belong to the animation code
   (`animdata.md` OQ1). The frame is used as is for the COF row (r4) and
   for each component's cel (§6).
3. **Expected direction count** `n` (`0x004DAF70`): players 8, or 16 for
   the local player while `[0x007A8928]` = 0; monsters monstats2 `d<mode>`
   (+0xF4 + mode), but 4 when that is 8 and the class's graphics-ready
   flag for the mode is 0 (`0x0046F9D0`: a 0xA0-byte record per
   `monstats` row at `[0x007A80FC]`, dword `mode` = ready, dword 20 +
   `mode` = load requested; allocated zeroed per game by `0x00470200`,
   set to 1 by `0x0046FDA0` once the asynchronous load of that class and
   mode is no longer pending, `0x005FF300`); missiles
   `NumDirections`; others 1. A mode < 0 gives 1.
4. **COF row**: with `D` = COF directions (header byte 2), `dir64` is
   first snapped when (`D`, `n`) = (8, 4) or (16, 8) (`0x004DB290`):
   `dir64 := table[dir64 >> 3] << 3` with table `0x006E5DA0` = 1, 1, 3, 3,
   5, 5, 7, 7, or `dir64 := table[dir64 >> 2] << 2` with table `0x006E5DA8`
   = 0, 0, 2, 2, …, 14, 14. Then the COF direction is
   `cof_dir = ((dir64 + 32/D) >> log2(64/D)) mod D` (table `0x006E55A0`,
   indexed by `ffs(D)` = `0x00517ABB`; `D` must be a power of two below
   128, else fatal 0x1ED/0x1EE; `dir64` ≥ 64 is fatal 0x1EC, but
   `0x004DB2E0` first turns `dir64` ≥ 64 into direction 0 with `n` = 1).
   Values: `specs/render/unit-directions.tsv` column `cof_dir`.
5. **Snap write-back**: when `n` ≠ `D` (whether or not r4 snapped) and
   the unit is a dead player (mode DT/DD) or dead monster
   (DT/DD), `0x004DB1E0` writes the snapped `dir64` back into the unit's
   path (`0x006488A0`). Client state; reproduce.
6. **Draw-order row**: the `L` component bytes at file offset
   `28 + 9L + F + (cof_dir × F + frame) × L` (`0x004DB110`, `F` = header
   byte 1). This assumes the event block is exactly `F` bytes; the three
   1.14d COFs with `K` = 4 > `F` = 1 (`formats/cof.md` Edge cases:
   `objects\f9\COF\f9NUHTH`, `f9onhth`, `f9ophth`, `d2data.mpq`) are read 3
   bytes early: the game reads component 0 (HD) where the file's order
   says 1 (TR). No bound check on `frame`: a frame ≥ `F` reads the next
   direction's rows.

Direction storage differs between the files: COF rows are in angular
order (r4), DCC/DC6 directions are interleaved (§6 r3). Survey (2,254
live 8- and 16-direction COFs with more than one component): the summed
difference between neighbouring rows is 199,280 slots in stored order and
359,770 in the DCC order, so the stored order is the angular one, as the
code says. (Riiablo uses one index for both; 1.14d does not.)

### 4. Pre-test: COF box culling

A composite is drawn only if, with (X, Y) the final screen position (after
§8, origins and panel shift, `camera.md` §4), the COF header box
(`cof.md` x min, x max, y min, y max) satisfies all of
`x_min + X < W − 1`, `x_max + X ≥ 0`, `y_max + Y ≥ 0`,
`y_min + Y < H − 1` (`0x004709A0`, called with centering off). This is the
unit culling of `camera.md` OQ2 for types 0–2; types 3 and 4 have no
pre-test (§9).

### 5. The slot loop (`0x00470EC0`)

For `s` = 0 … L − 1, component `c` = row byte `s` (§3 r6):

1. `c` = 14 (S7): no own graphic. If the unit has a linked unit
   (`0x004639D0`: flag-ex bit 10 set, the unit of owner type +0x94) that
   is a missile or has state 143 `attached`, that unit is drawn here
   through the unit draw entry (`0x004DC7B0` with "inline" = 1). Units
   with state 143 or 146 `invis` are skipped in the normal lists
   (`draw-order.md` §5), so attached units appear only inline.
2. Otherwise build the component request (`0x004DBB50` → `0x004DB7B0`,
   §5.1); if it fails the slot draws nothing (no error). Then the cel
   (§6) is drawn at (X, Y) with the slot's colormap source (§7) and draw
   mode (`render/blend-modes.md`: COF layer override fields read by
   `0x004DB050` / `0x004DB140`, `0x004DB360`, selection highlight mode 7,
   light ×2 clamped to [0x40, 0xFF] when highlighted).
3. Overlays: with flag b set, `0x0046E300` draws the unit's back overlays
   (third argument 1) before slot 0 and its front overlays (0) after the
   last slot (r4).
4. Overlay draw (`0x0046E300(unit, light, back, x, y, …)`): the list is
   gfx (`+0x54`) `+0x2C`, next `+0xA4`; a new overlay is pushed at the
   head (`0x00470390`), so the newest draws first. Record `+0x34` is the
   `overlay.txt` `PreDraw` byte (`+0x48`) copied at creation: records with
   `+0x34` ≠ 0 draw only in the back call, the others only in the front
   call. Skipped: kind (`+0x00`) 6 with `+0x08` = 0; kind 8 with `+0x3C`
   = 0. Frame `f = +0x18 >> 8`, drawn only while `f < +0x1C >> 8`, cel
   from `overlay.txt` row `+0x04` (0x84-byte rows) with the unit's
   direction byte (`0x00620100`) through `0x004DBB50`. Position
   (GDI): `X = px − (cx_u − shiftX) + ox + (+0x20)`,
   `Y = py − (cy_u − 8) + oy + oz + (+0x24)` with the motion offsets of
   §8; items (type 4) on the ground take the (x, y) the caller passes
   when both are not −1. Draw slot `+0x84` with light byte = the unit's,
   draw mode `Trans` (`+0x7C`), palette argument always 0 (no `P`):
   with `LocalBlood` (`+0x81`) ≠ 0 and the green-blood switch on
   (`render/shading.md` §6 r7) the code fetches the blood map
   (`0x00477680`) but discards it (original bug, reproduce: no remap). The overlay records' creation, timing and
   files beyond this are the overlay owner's (no spec yet).

#### 5.1 The component request (`0x004DB7B0`)

The request holds five 4-byte codes: unit token, component token, armor
class, mode token, weapon class. It fails (slot not drawn) when: the COF
has no layer record for `c` (the layer walk stops at the first record ≥ 16
or after `L` records); the unit token, component token or weapon class is
empty; the armor class ends up 0 (r3: an `armtype` index above 2).

| Code | Value |
|---|---|
| unit token | `0x004DAA00`: `plrtype` / `monstats` Code / `objects` Token |
| component token | `composit` token of `c` (table `0x007C8920`, filled by `0x004DA790` from `composit` rows 0–15) |
| mode token | `plrmode` (`0x007C88D0`, 20 rows), `monmode` (`0x007C8980`, 16), `objmode` (`0x007C8960`, 8) token of the mode; empty → `xxx ` |
| weapon class | §2.1 (`0x0064F380`), copied space-padded (`0x004DA720`) |
| armor class | r1–r3; default `lit ` |

1. **Objects**: `lit`.
2. **Monsters**: `v` = the monster's choice for `c` (client monster data
   +4 + c, `0x004DA770`). If `v` < monstats2 count for `c` (+0x15 + c):
   the `compcode` code of choice byte (+0x26 + 12c + v) (`0x00664860` →
   `0x006117D0`; layout `data/callbacks.md` §5); else none (→ `lit`). In
   mode DT/DD without `compositeDeath` the choice is not looked up (`lit`).
   Two override tables (§5.2) replace the code by component and `v` when
   the unit's room is in act II (`0x006427F0` = 1) and the monster's base
   class (`monstats` row +0x02, `0x00463860`, checked by `0x00463900`) is
   0 `skeleton1` (`0x007489A8`) or 170 `sk_archer1` (`0x00748A18`); the
   override is applied after the `compcode` lookup, only for `v` below the
   monstats2 count, and only for components with a table.
3. **Players**: no inventory → request fails with `lit`. Components TR,
   LG, RA, LA, S1, S2 (`0x0064F420`): in modes DT/DD `lit`; else `armtype`
   token (`0x007C89C0`) of the body armor's (body location 3, `0x004DAAB0`)
   byte `torso`, `legs`, `rArm`, `lArm`, `rspad`, `lspad` respectively
   (`0x0064F500`; armor +0x116 … +0x11B); no valid body armor → index 0
   (`lit`). Other components (`0x004DAD80`; the inventory is that of the
   linked unit under the condition of §1.1):
   - weapon class `xbw`: RH and LH show the primary hand weapon's
     `alternategfx`, else its `code` (no weapon → `lit`);
   - weapon class `bow`: RH `lit`;
   - SH with state 101 `holyshield` and an item in the shield hand
     (`0x0063C8F0`) → `hsh`;
   - otherwise the item for `c` (`0x0063C050`: the equipped item whose
     `component` is `c`; for 5 the primary weapon, for 6 the other hand
     item) gives its `alternategfx`, else its `code` (`0x006285F0`); no
     valid item → `lit`.
   Survey: player DCCs use `lit`/`med`/`hvy` for TR, LG, RA, LA, S1, S2
   and item codes for HD, RH, LH, SH (e.g. `cap`, `hax`, `hsh`).
4. Every code is space-padded to 4 bytes; the name parts of §6 stop at
   the first space.
5. **Layer walk** (`0x004DB7B0`, same walk in `0x004DB050` /
   `0x004DB090` / `0x004DB140` for the layer fields): records in file
   order; the first record whose component byte equals `c` is used, so a
   later duplicate record for `c` is never read. A duplicate is not an
   error in 1.14d.

#### 5.2 Act II skeleton armor classes (`0x00664860`)

Static tables (initialized `.data`, no writer): per component a pointer
to an array of 4-byte codes indexed by the choice `v`; a null pointer
keeps the `compcode` code; a zero code makes the request fail (§5.1).
Only the entries reachable with the 1.14d `monstats2` counts are listed
(the arrays overlap in memory; a larger count would read the neighbour).

| Base class | Component (count) | Codes for `v` = 0, 1, … | `monstats2` list (replaced) |
|---|---|---|---|
| 0 `skeleton1` | HD (7) | `lit lit des des hvy hvy hvy` | `lit,lit,lit,med,hvy,hvy,hvy` |
| 0 | TR (3) | `lit med hvy` | same |
| 0 | LG, RA, LA (3 each) | `lit des hvy` | `lit,med,hvy` |
| 0 | RH (10) | `axe axe fla fla hax hax mac mac scm scm` | `axe,fla,hax,hax,hax,mac,mac,mac,scm,scm` |
| 0 | SH (5) | 0, `buc lrg kit sml` | `nil,buc,lrg,kit,sml` |
| 0 | S1, S2 (12 each) | 0 × 9, `lit des hvy` | `nil` × 9, `lit,med,hvy` |
| 0 | LH, S3–S8 | no table | — |
| 170 `sk_archer1` | HD, LG, RA, LA (3 each) | `lit des hvy` | `lit,med,hvy` |
| 170 | TR (3) | `lit med hvy` | same |
| 170 | LH (1) | `sbw` | same |
| 170 | S1, S2 (0) | table present, unreachable | — |
| 170 | RH, SH, S3–S8 | no table | — |

Every `monstats` row with base 0 (`skeleton1`–`skeleton8`) or base 170
(`sk_archer1`–`sk_archer11`) is affected in act II levels; elsewhere the
`compcode` codes stand. So act II skeletons wear the `des` variant where
the other acts show `med` (and `lit` for HD choice 2), and use the
`hax` / `mac` choices in a different order.

### 6. Component file and cel

1. **Name** (`0x005FE2B0`): unit token + component token + armor class +
   mode token + weapon class, each up to 3 characters, stopping at the
   first space. **Path** (`0x005FE610`):
   `<root>\<T>\<C>\<name>.dcc` (`%s\%s\%s\%s.dcc`, string `0x006E3618`),
   root as §2, `T` and `C` the unit and component tokens with spaces → 0.
   Example: `DATA\GLOBAL\CHARS\AM\HD\AMHDcapNU1hs.dcc` (archives match
   case-insensitively).
2. **DC6 instead of DCC** (same path, `.dc6`, `0x006E3608`): always for
   monsters 242 `mephisto`, 251 `tyrael1`, 367 `tyrael2`, 521 `tyrael3`,
   704 `ubermephisto` and objects 342 `portal`, 563 `The Worldstone
   Chamber`; in mode 0 (DT) only for monsters 243 `diablo`, 284
   `maggotqueen1`, 333 `diabloclone`, 544 `baalcrab`, 559
   `baalcrabstairs`, 570 `baalclone`, 705 `uberdiablo`, 709 `uberbaal`
   (row indices of the loaded tables); and the name `OYTRlitTNhth`
   (compared ASCII case-insensitively over the whole name: `0x00413590`
   → `_strnicmp` with length 0x7FFFFFFF, only while `CompressedData` ≠ 0). All
   DCC otherwise, given the registry value `CompressedData` (default 1,
   `0x005FE280`; Open question 12). Survey: the archives hold 55 monster
   and 13 object `.dc6` composite parts, all under tokens MP, TX, TY, DI,
   MQ, 42, 1Y, 4X.
3. **Cel** (`D2CMP_GetCelFromCelContext` `0x00601840`): with the file's
   own direction count `Df` and frames per direction `Ff`, the cel index is
   `Ff × dcc_dir + frame`, `dcc_dir` = table `0x006E45A0` [`ffs(Df)`]
   [`dir64`] (column `dcc_dir` of `unit-directions.tsv`): the interleaved
   order 4, 0, 5, 1, 6, 2, 7, 3 for `Df` = 8 (angular step `k` →
   position `P[k]`). `dir64` here is the value after the snap of §3 r4.
   Fatal unless the file version is 6, `dir64` < 64 and `frame` ≤ `Ff` (so
   `frame` = `Ff` passes and reads the next direction's first cel).
   A file with `Df` = 2 always shows direction 0.
4. A file that does not load leaves the slot empty (`0x006001F0` fails);
   e.g. `lit` exists for player RH only in 14 files.

### 7. Colormap source per component

The draw call takes one 256-byte colormap pointer per slot (`render/
shading.md` owns the maps; `0` = none). Selection (`0x00470EC0`):

| Component | Colormap source |
|---|---|
| S8 (15) | none; for monsters whose monstats2 `localBlood` (+0x11C) ≠ 0 while `0x0044DC60` ≠ 0: `0x00477680` (Open question 6) |
| S7 (14) | none (inline unit, §5 r1) |
| others | the unit map `U`, replaced for players by the item map of r2 |

1. `U`: unit +0x6C (palette index) `p` ≠ 0 → shift table row `p − 1`
   (the caller passes `p − 1`: `0x00471000` here, `0x0047200F` for §9;
   `0x004FB0C0` returns the 256 bytes at `0x007D6468 + 256 × i`; the
   table's meaning is `render/shading.md` §6 r1); else monsters'
   palette shift (`0x00477530`, `palshift.dat`), else none.
2. Item map (`0x004DB570`): used when `0x0063A790(unit)` = 0, or the
   unit is the local player and `0x00477750` ≠ 0; never for a player
   whose palette index (+0x6C) is 0x6C. The item is `0x0063C050(c)`, or
   for LG/RA/LA/S1/S2 without one the item whose `component` is TR;
   used when its flags have neither 0x100 nor 0x4000 and its gfx code
   (r3 of §5.1) is not `lit` (`lit` → no map at all); the map is
   `0x0062C100(unit, item)` (item colour; owner items/shading), `U` when
   that returns 0.

### 8. Extra offsets (`0x004DA0B0`, `0x004DA0D0`, `0x004DA0F0`)

The unit draw adds `X += ox`, `Y += oy + oz` from the unit's **client
motion record** (the 0x4C-byte record at gfx +0x30, gfx = unit +0x54;
`0x0046F060`), 0 when there is none; then for objects (§1)
`objects` Xoffset/Yoffset (+0x148/+0x14C), skipping the unit when Draw
(+0x150) is 0; for missiles (unit draw entry `0x004DC7B0`) `missiles`
xoffset to X, yoffset + zoffset to Y (i16, +0xA2/+0xA4/+0xA6), not drawn
without a missiles row. (`camera.md` §4, OQ3.)

Record fields (i32; positions 16.16 subtile, so `>> 11` is 1/32 subtile):

| Index | Field |
|---|---|
| 0 | flags: 1 done, 2 timed, 4 bounce, 0x10 follow linked unit, 0x20 stop when reaching limits from below |
| 1–3 | x, y, z |
| 4–6 | velocity |
| 7–9 | acceleration |
| 10–12 | limits (1/32 subtile) |
| 13–15 | ox, oy, oz (pixels, read by the draw) |
| 16 | bounces left |
| 17 | bounce factor (percent) |
| 18 | ticks left (flag 2) |

**Update** `0x004DA350`, once per client unit update (`0x00480810`, the
per-unit client tick, before the type update); nothing when flag 1 is set:

1. x += vx; y += vy; vx += ax; z += vz; vy += ay; vz += az (in this
   order; 32-bit wrap).
2. Flag 2: if ticks left = 0: set flag 1, x := 0, y := 0; else decrement.
3. Flag 4 (bounce): if `z >> 11` ≤ limit z: vz := −trunc(factor × vz /
   100) (32-bit product), z := limit z (unshifted, as 1.14d writes it);
   then, still inside this hit branch, if bounces left ≠ 0 decrement it,
   else set flag 1. When `z >> 11` > limit z nothing of r3 happens (and
   r4, r5 are skipped: flag 4 excludes them).
4. Else flag 0x10: offsets follow the linked unit `K` (`0x004639D0`; no
   `K` → nothing more this update, r6 skipped too). If this unit is a
   missile, `K` must be a monster (else fatal 0x1A9) and nothing happens
   while `K` is in mode DT (0) or DD (12). (`a`, `b`) := the `xoff`,
   `yoff` of `K`'s component 14 (S7) cel for `K`'s current frame (`+0x44
   >> 8`) and direction (`0x004706E0` with EDX = 14 at `0x004DA494`;
   request `0x004DBB50`, cel offsets `0x00601920` / `0x00601950`), or
   (0, 0) when `K`'s COF for its draw mode is not loaded, its loaded-COF
   field `+0x14` is 0, or the request fails. Then ox := `a` + `K`'s ox,
   oz := `b` (+ 10 for a missile) + `K`'s oz (`0x004DA0B0`,
   `0x004DA0F0`); oy is not changed; x, y := `0x00643510`(ox, oy) =
   ((2·oy + ox) >> 5, (2·oy − ox) >> 5) (arithmetic shifts); z := −oz ×
   2,048.
5. Else: stop when (flag 0x20 clear and `x>>11` ≤ lx, `y>>11` ≤ ly,
   `z>>11` ≤ lz) or (flag 0x20 set and all three ≥ their limits): x, y, z
   := limits << 11, set flag 1.
6. Unless flag 0x10: `a = x >> 11`, `b = y >> 11`, `ox = (a − b) >> 1`,
   `oy = (a + b) >> 2` (`0x00643290`, arithmetic shifts), `oz = −(z >> 11)`.

Created (`0x004DA000`, zeroed) and freed (`0x004DA080`) by client effect
code: 16 creation sites (`0x00465716` … `0x004F0BCC`), fields set through
`0x004DA1D0` (position, `<< 11`), `0x004DA200` (velocity), `0x004DA250`,
`0x004DA2A0` (others). Which effects create one and with which values
belongs to those effect specs (knockback, leap, item drop, corpse throw…;
Open question 7). With no record every offset is 0.

### 9. Single-cel units (missiles, items)

Missiles: `DATA\GLOBAL\MISSILES\<CelFile>.dcc` (`missiles` +0x138), DCC
when `CompressedData`, cel by §6 r3 with the missile's direction and frame.
Items: mode 0, 1, 2, 4, 6 → inventory graphics, mode 3, 5 → ground
(`0x004DAA70`); a gold pile (`0x0062B400` = 4) replaces its direction by
the amount class of stat 14: < 100 → 0, < 500 → 1, < 5,000 → 2, else 3; a ground item (mode 3) uses its `flippyfile`, or the
unique (`uniqueitems` +0x3A) / set (`setitems` +0x42) flippy file when its
quality is 7 / 5 and that name is not empty (`0x004DABC0`), path
`DATA\GLOBAL\items\<name>.dc6`. Draw mode 5, 7 when highlighted; missiles
3 / 4 by `missiles` Trans 1 / 2 (`render/blend-modes.md`); colormap `U`
for units with +0x6C ≠ 0 other than the local player, missiles with
LocalBlood use `0x00477680`, items from `0x0062C100`. Only the front
overlays are drawn (after the cel).

### 10. d2rs mapping

| Hook | Answer |
|---|---|
| `ViewRules::unit_pose` | `None` unless type 0–2 with a loaded COF passing §4; `cof` = §2, `dir` = `cof_dir` (§3 r4), `frame` = §3 r2 |
| `composite::slot_order` | row offset `28 + 9L + F` (§3 r6); a component without a layer record is a slot that draws nothing (not `NoLayer`); a row past the file end is an error (unreproducible) |
| `ComponentResolver::frame` / `ViewRules::component_frame` | §5.1 + §6: file path, `dcc_dir` from the snapped `dir64`, cel `Ff × dcc_dir + frame`; failed request or missing file → no draw for the slot |
| `ComponentResolver::shade` | the source of §7 (map contents: `render/shading.md`) |
| `ViewSource::unit_offset` | `(ox, oy + oz)` of §8 plus the object / missile offsets of §8 |
| `UnitParams.sub` | slot index; back overlays share sub 0 and are built before slot 0, front overlays sub 255, an inline unit (§5 r1) the host's slot (stable sort keeps build order) |
| duplicate layer records | first match (§5.1 r5); not an error |
| `armtype` index above 2 (Edge cases) | the request fails (slot not drawn) and d2rs reports it: the original reads unrelated memory, unreproducible; no 1.14d armor row reaches it |
| inputs of an open question | refused as unresolved, never guessed. Since 2026-10-06 OQ1 (mode overrides, §2 r2), OQ2 / OQ3 (§1.1, §2.1), OQ4 (graphics-ready flag, §3 r3: with synchronous loading d2rs treats a loaded class and mode as ready), OQ5 (§5.2) and the follow branch of OQ7 (§8 r4) are specified; still open: motion-record creators (OQ7) |
| §3 r5 write-back | applied once per drawn frame (`camera.md` §9), to the client unit state |

## Constants & data dependencies

Tables `0x006E55A0` (COF directions) and `0x006E45A0` (file directions):
`unit-directions.tsv` (448 rows, read from `Game.exe`; row `ffs(D)` = 0
unused). Both follow formulas, so a unit test can regenerate the file:
`cof_dir` as §3 r4; `dcc_dir = P_D[cof_dir]` with `P_1 = [0]`,
`P_2 = [0, 0]`, `P_4 = [0, 1, 2, 3]`, `P_8[2j] = 4 + j`, `P_8[2j+1] = j`,
and for `D` ≥ 8: `P_2D[2j] = P_D[j]`, `P_2D[2j+1] = D + j` (checked on
all 448 rows). Snap tables `0x006E5DA0`,
`0x006E5DA8` (§3 r4); the variants `0x006E5DB8` (3, 3, 5, 5, 7, 7, 1, 1)
and `0x006E5DC0` (2, 2, 4, …, 14, 0, 0) are selected by `0x00600CB0`'s
fourth argument, which the composite path passes as 0. Codes `lit ` =
0x2074696C, `xxx ` (mode default), `hth `, `ht2 `, `hsh `, `xbw `, `bow `.

## Randomness

None.

## Edge cases & original bugs

- The `f9` objects (3 COFs) draw nothing in 1.14d (§3 r6).
- Frames ≥ `F` / ≥ `Ff` read neighbouring rows / cels instead of failing.
- The bounce branch sets z to the unshifted limit (§8 r3); harmless for
  limit 0.
- A 2-direction file shows only direction 0 (§6 r3).
- Remote players use 8 directions even with 16-direction COFs (§3 r3–r4).
- An armor `torso` … `lspad` byte above 2 indexes past the 3-entry
  `armtype` table (`0x007C89C0`): the next dwords are `[0x007C89CC]`
  (the screen-fade timer, `draw-order.md` §1) and beyond. No 1.14d armor
  row has such a byte (`patch_d2.mpq` `Armor.txt`, 203 rows, columns
  `rArm` … `lSPad`: all ≤ 2).
- `0x00643340`-style negative rounding does not occur here.

## Test vectors

| Input | Expected | Source |
|---|---|---|
| `D` = 16, `dir64` 0 / 2 / 6 / 62 | `cof_dir` 0 / 1 / 2 / 0 | §3 r4, TSV |
| `D` = 8, `dir64` 3 / 4 / 59 / 60 | 0 / 1 / 7 / 0 | §3 r4 |
| `Df` = 8, `dir64` 0 / 4 / 12 / 60 | `dcc_dir` 4 / 0 / 5 / 4 | §6 r3 |
| `Df` = 16, `dir64` 2, `Ff` = 8, frame 3 | cel 8 × 8 + 3 = 67 | §6 r3 |
| remote player, 16-dir COF (`n` = 8), `dir64` 6 | snapped 0 → `cof_dir` 0, `dcc_dir` (Df 16) 4 | §3 r3–r4 |
| local player, same | `cof_dir` 2, `dcc_dir` 0 | §3 r4, §6 r3 |
| Amazon, mode NU, weapon `1hs` | `DATA\GLOBAL\CHARS\AM\COF\AMNU1hs.COF` | §2 |
| player mode TH with `2hs` | W = `hth` | §2 r3 |
| Amazon HD with a `cap`, mode NU, `1hs` | `DATA\GLOBAL\CHARS\AM\HD\AMHDcapNU1hs.dcc` | §5.1, §6 |
| body armor `torso` = 2 | TR armor class `hvy` | §5.1 r3 |
| `f9NUHTH.COF` (42 bytes, L 1, F 1, D 1, K 4) | row read at offset 38 = 0 (HD); no HD layer → nothing drawn | §3 r6 |
| monster 242, any mode | `.dc6`; monster 243 mode 0 `.dc6`, mode 1 `.dcc` | §6 r2 |
| COF box x −30 … 30, y −90 … 0 at (X, Y) = (−31, 300), W × H = 800 × 600 | culled; at (−30, 300) drawn; at (400, 690) culled (y_min + Y = 600 ≥ 599) | §4 |
| motion record: pos (0, 0, 0), vel 0, limits (10, 10, 10), flags 0 | one update → done; ox 0, oy 5, oz −10; draw adds (0, −5) | §8 |
| motion record: x = 32 << 11, y = 0, limits (0, 0, 0), flags 1 | no update; ox, oy as last computed | §8 |
| capture `unit-0001`: local player standing still, re-faced through 16 directions, each frame captured (`capture.md`) | CPU reference per §2–§7 equals the capture per direction | capture, queued |

## Provenance

1.14d `Game.exe`: unit draw `0x00471EC0`, composite `0x00470EC0`,
`0x004DB180`, culling `0x004709A0`, COF name `0x0064F5B0`, weapon class
`0x0064F060`/`0x0064F380`, direction count `0x004DAF70`, snap
`0x004DB290`/`0x00600CB0`, COF direction `0x00600E20`, write-back
`0x004DB1E0`, row `0x004DB110`, request `0x004DBB50`/`0x004DB7B0`, token
tables `0x004DA790`, armor `0x004DAAB0`/`0x0064F420`/`0x0064F500`/
`0x004DAD80`/`0x0063C050`/`0x006285F0`/`0x00664860`, name `0x005FE2B0`,
path `0x005FE610`, cel `0x00601840`, colormaps `0x004DB570`/`0x004FB0C0`/
`0x00477530`, motion `0x004DA000`/`0x004DA350`/`0x004DA0B0`–`0x004DA0F0`/
`0x00480810`, single cels `0x004DA8E0`/`0x004DAA70`/`0x004DABC0`. Field
offsets cross-checked against `specs/data/fields.tsv` (armor `component`
+0x115, `rArm` … `lspad` +0x116 … +0x11B, `code` +0x80, `alternategfx`
+0x90; objects `OrderFlag` +0x131, Draw +0x150, `DrawUnder` +0x1B7;
missiles xoffset +0xA2, Trans +0x18D, NumDirections +0x1A0, LocalBlood
+0x1A2; monstats2 counts +0x15, choices +0x26, `d<mode>` +0xF4,
`localBlood` +0x11C). Surveys (scratch scripts on `d2char`/`d2data`/
`d2exp`, `Patch_D2` has no listfile): 3,512 COF paths (directions 1/4/8/16
= 1,759/14/956/782; S7 in 19 COFs, all `64`, `65`, `ac`, `ox`; S8 in 28,
all death modes except `bta2hth`), the row-smoothness count of §3, 20,941
composite DCC/DC6 names. D2MOO (1.10f) `D2Win/D2Comp.cpp` (character
screen composite) and Riiablo `Entity.java` were hints for the name
pattern only. Implementation follow-ups (2026-10-06): palette row `p − 1`
at `0x00471000` / `0x0047200F`; layer walks `0x004DB7B0`, `0x004DB050`,
`0x004DB090`, `0x004DB140` (first match); `OYTRlitTNhth` compare
`0x005FE610` → `0x00413590` (`_strnicmp`); bounce branch of `0x004DA350`
re-read. Implementation readings (`impl-unit-composite` §4, 2026-10-06):
UC1 (duplicate layer records) is §5.1 r5 (first match, not an error);
UC2 (`armtype` index above 2) is the Edge cases entry and the §10 row (the
request reads the dword after the table, `[0x007C89CC]`, usually 0, so it
fails; no 1.14d armor row reaches it: d2rs fails the request); UC3
(bounce count) is §8 r3 (decrement inside the hit branch); UC4
(`OYTRlitTNhth`) is case-insensitive (`_strnicmp`, §6 r2); UC5 (refused
inputs) is the §10 row. Mode override tables `0x00745900`/`0x00745914`
and the act II tables `0x007489A8`/`0x00748A18` read from the file
(`.data`, `disasm.py xref`: reads only); `0x00664860`, `0x00463860`,
`0x00463900`; graphics-ready table `0x00470200`, `0x0046FDA0`,
`0x005FF300`; follow branch `0x004DA350` (`0x004DA494`), `0x004706E0`,
`0x00643510`; `0x004FAC90` (string `d2char.mpq` at `0x006DC77C`).
Ghidra backlog (2026-10-06): substitution `0x00645270` (mode maps
`0x00645190`, tables `0x006EB348`/`0x006EB308` and fallback jump table
`0x00645248`/`0x00645260` read from the file), state-flag test
`0x0063A7B0` → `0x0063A130`; Barbarian classes `0x0064F1EE`–
`0x0064F2D5`, `0x00629FE0`, static table `0x007446A0`; overlays
`0x0046E300`, creation link `0x00470390`. Live data: `states` gfxtype /
gfxclass / bossinv columns of `patch_d2`.

## Open questions

1. ~~Contents of the mode override tables~~: answered in §2 r2 (static
   `.data`: player SQ, KB and monster KB → `gh`).
2. ~~The draw-identity substitution and the linked-unit inventory~~:
   answered in §1.1.
3. ~~Item type class `0x00629FE0`~~: answered in §2 (r-dual; static
   table `0x007446A0`).
4. ~~Monster direction count 4~~: answered in §3 r3 (per-class, per-mode
   graphics-ready flag). Open: whether 1.14d ever draws a monster before
   its mode's flag is set (first frames after a mode change to a not yet
   loaded mode); a capture of a monster's first attack.
5. ~~Monster armor-class override tables~~: answered in §5.2 (act II,
   base classes `skeleton1` / `sk_archer1`). A capture of an act II
   skeleton (e.g. Halls of the Dead) confirms the `des` files.
6. S8 blood map: `0x0044DC60` (an option?) and `0x00477680`; owner
   `render/shading.md`.
7. Motion record creators and their initial values (16 sites; the
   follow branch is answered in §8 r4). Open: what the loaded-COF field
   `+0x14` tested by `0x004706E0` means. Ghidra reads; a capture of a
   knockback or item drop.
8. ~~`[0x007A8928]`~~: set once per game (`0x00470200` →
   `0x004FAC90`): 0 when `d2char.mpq` is found (install directory or
   current directory, `GetFileAttributesA`), else 1 unless `[0x0074C82C]`
   ≠ 0. A full install has it, so the local player uses 16 directions.
9. Draw mode inputs `0x004DB360`, `0x00464370`, the shadow argument:
   owner `render/blend-modes.md`.
10. Cross-spec (`camera.md`, not edited here): §4 answers camera OQ2 for
    types 0–2 and §8 answers camera OQ3; camera §4's object and missile
    offset sentence should link here.
11. Cross-spec (`formats/cof.md`): the game's row offset ignores event
    padding (§3 r6); `cof.md` keeps the file-format reading.
12. `CompressedData` = 0 (every composite part DC6): never the case in the
    reference install; d2rs supports 1 only until a capture needs 0.
13. Overlay files and their back/front split: the split, order,
    position and blend are answered in §5 r4; the creation of overlay
    records (who adds which `overlay.txt` row, frame advance) stays with
    the overlay owner (no spec yet; `0x00470390`).
