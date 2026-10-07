# Spec: Formats — Character save appearance bytes (.d2s +0x88..+0xA7)

- **Status:** draft: read from the 1.14d `Game.exe` appearance fill
  (`0x0063E510` and its helpers, with `tools/ghidra/disasm.py` for the
  register arguments); the token rule was simulated over the 1.14d
  `patch_d2` item tables and reproduces all 8 component values measured
  in the saves of this PC (Test vectors). Split out of
  `formats/d2s.md` §2.8 (that spec was at its size limit); answers its
  Open question 17.
- **Target version:** 1.14d
- **Crate/module:** `d2-formats::d2s` (writer of header +0x88..+0xA7)
- **Related specs:** `formats/d2s.md` §2.1, §2.6, §2.7, §2.8;
  `items/inventory.md` (body grid, weapon in use); `data/fields.tsv`
  (`weapons`/`armor`/`misc`, `armtype`, `magicprefix`, `setitems`,
  `uniqueitems`, `gems`, `states` offsets); `data/runtime-maps.md` §2
  (itemtypes is-a matrix); `data/txt-format.md` (`Expansion` rows
  removed, so the runtime itemtypes index of a row after line 58 is one
  less than its file row).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 41–51 |
| Inputs | 52–61 |
| Outputs / state changes | 62–66 |
| Rules | 67–73 |
|   1. The token table (`0x0063D710`, built on first use) | 74–102 |
|   2. Token lookup (`0x0063D900`) | 103–108 |
|   3. The fill (`0x0063E510`(player, components, colours)) | 109–120 |
|   4. Helm and hand items (`0x0063DA70`) | 121–144 |
|   5. Body armour (composite branch of `0x0063E510`) | 145–156 |
|   6. Colour byte (`0x0062C100`(player, item, &byte, 0)) | 157–185 |
| Constants & data dependencies | 186–199 |
| Randomness | 200–203 |
| Edge cases & original bugs | 204–222 |
| Test vectors | 223–234 |
| Provenance | 235–250 |
| Open questions | 251–279 |
<!-- /index -->

## Summary

On every save the writer fills the 16 component bytes (+0x88..+0x97)
and the 16 colour bytes (+0x98..+0xA7) with 0xFF and then lets
`0x0063E510` write, for each equipped helm, body armour or hand item,
a graphics token index into the component byte of the body part it
draws, and a palette-shift byte into the colour byte of the same part.
The token index is a position in a 255-entry table the game builds once
from the item tables; the colour byte is `Transform` × 32 + colour + 1.
Character select (`formats/d2s.md` §2.7) reads 11 of each.

## Inputs

| Name | Type | Source |
|---|---|---|
| the player's inventory item list (link order) and its body grid | units | `items/inventory.md` |
| items record: `code` +0x80, `alternategfx` +0x90, `component` +0x115, `rArm`..`lSPad` +0x116..+0x11B, `type` +0x11E, `hasinv` +0x137, `Transform` +0x141, `gemoffset` +0xF0, `wclass` +0xC0 | per item | `data/fields.tsv` |
| `armtype` record (0x34 bytes): `token` +0x20 | 3 rows (`lit`, `med`, `hvy`) | `data/fields.tsv` |
| affix +0x56 `transformcolor`; `setitems` +0x40 `chrtransform`; `uniqueitems` +0x38 `chrtransform`; `gems` +0x2F `transform`; `states` +0x2A `itemtype`, +0x2C `itemtrans` | link8 colour (row of `colors.txt`) | `data/fields.tsv` |
| player's states; player's weapon class (`0x0064F380`) | — | unit |

## Outputs / state changes

The 32 header bytes +0x88..+0xA7 of the save being written. Nothing
else (the token table is built once per process, rule 1.1).

## Rules

Byte i of the components (+0x88 + i) and of the colours (+0x98 + i)
is body part i: 0 HD, 1 TR, 2 LG, 3 RA, 4 LA, 5 RH, 6 LH, 7 SH, 8 S1,
9 S2, 10 S3, 11–15 S4–S8 (the order of the composit tokens; the stub's
fill of `formats/d2s.md` §2.6 lights 0–4, 8, 9).

### 1. The token table (`0x0063D710`, built on first use)

1. A static table at `0x0096CC68` of 255 entries of (u32 code, i32
   item type), entry 0 unused. Built once (flag `0x0096D464`) on the
   first lookup (`0x0063D900` calls `0x0063D710`): zero it; entries
   1, 2, 3 := `lit `, `med `, `hvy ` with type 3; count n := 4.
2. Then for every items record in record order (weapons, then armor,
   then misc; record size 0x1A8; the list `0x00633590`):
   1. g := `alternategfx` if non-zero, else `code`.
   2. If g equals the code of an entry 0..n−1 → next record.
   3. Eligible: its `type` is-a `weap` (45), `tors` (3), `shld` (51) or
      `helm` (37), and is not is-a `circ` (75) (`0x00629B50`, the
      itemtypes matrix). Not eligible → next record.
   4. Slot search from i := n: skip i while (the 1.00 reference table at
      `0x00744CA8`, 8-byte entries with the type at +4, has a type at i
      that is-a `weap` and the record's type is-a `weap`) or (its type
      at i is-a `armo` (50) and the record's type is-a `armo`) or entry
      i is occupied. If i ≥ 255, i := n.
   5. Entry i := (g, `type`); if i = n, n := n + 1. (An entry placed
      above n is not seen by step 2.2's duplicate test, so a code can
      occur twice; step 3 of rule 2 finds the lower one.)
3. In 1.14d this gives `lit`/`med`/`hvy` at 1–3, the weapon tokens
   from 4 (`hax` 4, `axe` 5, `lax` 6, … `wnd` 9, `clb` 12, `ssd` 17,
   `jav` 27, `bst` 37, `ktr` 45, …), the helms from 57 (`cap`), the body
   armours from 64, the shields from 79 (`buc`), and the class items up
   to 97; the throwing potions land at 125–134 (skipped slots), and the
   code at slot 0 is 0. A writer computes the table from its own item
   tables with this rule; it is not a constant.

### 2. Token lookup (`0x0063D900`)

1. Input two codes a, b (registers ESI, EDI). For i = 1..254: entry i's
   code = a or = b → i. None → 0. (So for an item, a = `alternategfx`,
   b = `code`, and the first entry holding either wins.)

### 3. The fill (`0x0063E510`(player, components, colours))

1. Walk the player's inventory item list from its first item (inventory
   +0x0C), next = item data +0x64; stop at a non-item or a null item
   data. Only items with unit mode 1 (unit +0x10, equipped) do anything.
   Their body location (item data +0x44, `0x00627D40`) picks the
   branch: 1 (head), 4 (right hand), 5 (left hand) → rule 4, unless the
   item's `type` is-a `circ` (75), which does nothing; 3 (torso) →
   rule 5; any other location (neck, rings, belt, boots, gloves, the
   swap slots 11–12) → nothing. Later items overwrite bytes written by
   earlier ones.

### 4. Helm and hand items (`0x0063DA70`)

1. The two hand owners from the body grid (grid 0, slots 0–10 in
   index order, `0x0063C050`): an item whose `wclass` index is 2, 3 or
   12 (`1hs`, `1ht`, `ht1`; `0x00629FE0`, table `0x007446A0`) counts
   as the right-hand owner R when it is the weapon in use (inventory
   +0x1C, `0x0063BEF0`) and as the left-hand owner L when it is not;
   any other item counts as R when its `component` is 5 and as L when
   it is 6. R and L are the first such items.
2. p := `component`; t := the token (rule 2) of `alternategfx` and
   `code`. t = 0 → when p < 16, components[p] := colours[p] := 0xFF;
   done.
3. Else p := 5 if the item is R, 6 if it is L; otherwise p must be <
   16 (else done). components[p] := t.
4. Weapon-class extras (`0x0063D930`, only for body location 4 or 5),
   with w = the player's current weapon class (`0x0064F380`, mode −1):
   w = `xbw ` and `component` 5 or 6 and R exists → components[6] := the
   token of R (rule 2), or components[6] := colours[6] := 0xFF when it
   is 0. w = `bow ` and `component` 6 → the item record with code
   `lit ` is looked up (`0x00633640`); 1.14d has none, so nothing
   happens.
5. colours[p] by rule 6; result 0 → colours[p] := 0xFF, else colours[p]
   := the byte rule 6 left there + 1.

### 5. Body armour (composite branch of `0x0063E510`)

1. For each body part c in 1 TR, 2 LG, 3 RA, 4 LA, 8 S1, 9 S2
   (`0x0064F420`): v := the armour record's `Torso`, `Legs`, `rArm`,
   `lArm`, `rSPad`, `lSPad` byte for c (`0x0063D690`, `0x0064F500`);
   the `armtype` row v (`[0x0096D4E8]` + v × 0x34, `0x0065B620`); t :=
   the token (rule 2) of that row's `token` (+0x20). If the item's
   class id (unit +4) ≤ 0 or t = 0 → components[c] := colours[c] :=
   0xFF. Else components[c] := t (1, 2, 3 for `lit`, `med`, `hvy`) and
   colours[c] as rule 4.5.
2. Body parts 0, 5, 6, 7 and 10–15 are not touched by body armour.

### 6. Colour byte (`0x0062C100`(player, item, &byte, 0))

Write w(T, k) for the byte `0x0062A250` computes: (T × 32 + (k & 0x1F))
mod 256 when 1 ≤ T ≤ 8 and 0 ≤ k ≤ 20, else 0; and ok(T, k) for
`0x00600C20`: non-zero (a palette) when 1 ≤ T ≤ 8, T ≠ 3, T ≠ 4 and k <
21 (signed), else 0. T is always the item's `Transform` (+0x141).

1. State colours first: for each state of the data tables' state-colour
   list (`[0x00744304]` +0x18C, count +0x190) that the player has
   (`0x00639DF0`), whose `itemtrans` < 21 and whose `itemtype` the item
   is (`0x00629BB0`): result ok(T, `itemtrans`); the byte is **not**
   written (rule 4.5 then adds 1 to what is there).
2. Else by quality (item data +0x00):
   1. 4 magic, 6 rare: the first affix, suffixes (item data +0x3E,
      +0x40, +0x42) then prefixes (+0x38, +0x3A, +0x3C), whose record
      (`0x00633EE0`, ids 1-based, 0 → none) has `transformcolor` ≠ −1;
      none → the automagic affix (+0x36). Found k → byte := w(T, k),
      result ok(T, k); none → result 0.
   2. 5 set: the `setitems` row (item data +0x28); `chrtransform` < 0
      → 0; else byte := w(T, it), result ok(T, it).
   3. 7 unique: the `uniqueitems` row (item data +0x28) the same way.
   4. Any other quality: when `hasinv` ≠ 0, `0x0062BC20` (the type's
      socket limit for the item level) ≠ 0, the item has flag 0x800
      (socketed) and the first item of its own inventory is-a `gem`
      (20): k := that gem's `gems` row `transform` (via its items
      `gemoffset`), byte := w(T, k), result ok(T, k). Otherwise the
      automagic affix as in 2.1.
3. The result is used only as zero / non-zero (rule 4.5).

## Constants & data dependencies

Token reference table `0x00744CA8` (256 × 8 bytes, types in the
pre-1.08 numbering; only its is-a `weap` / `armo` test is used);
`wclass` index table `0x007446A0` (`bow` 1, `1hs` 2, `1ht` 3, `stf` 4,
`2hs` 5, `2ht` 6, `xbw` 7, `ht1` 12; count 8 at `0x007446E0`); the
data-table offsets of Inputs. In 1.14d `states.txt` the rows with an
`itemtype` are `enchant` (`weap`, `cred`) and `venomclaws` (`mele`,
`cgrn`). The list at +0x18C (`0x0096BDBC`, count u16 `0x0096BDC0`) is
built by the states loader (`0x00618100` → `0x00611E60`(list, count,
0x2A)): the row ids, in row order, of every `states` row (stride 0x3C)
whose `itemtype` (+0x2A, signed 16-bit) is > 0; so in 1.14d it holds
exactly those two rows (Open question 1).

## Randomness

None.

## Edge cases & original bugs

1. A state colour (rule 6.1) does not write the byte, so the saved
   colour is the previous value + 1: 0x00 when the byte was still 0xFF.
2. `Transform` 8 wraps in the byte: w(8, k) = k, so an item with
   `Transform` 8 and colour k saves k + 1. `Transform` 0, 3, 4 and 9 or
   more always save 0xFF (ok = 0).
3. A one-handed weapon that is not the weapon in use is drawn as the
   left hand (rule 4.1). Measured: a broken `hax` (flag 0x100,
   durability 0) and a broken `wnd` in the right hand saved component 6
   = 0x04 / 0x09 and component 5 = 0xFF (`bdMercTwo`, `bdGolem`), so
   the inventory's weapon in use was not that item (what clears it on
   breaking is not traced here).
4. Duplicate table entries above n (rule 1.2.5) are never returned by
   rule 2 when a lower copy exists (`ktr` at 45 and 243: the Assassin's
   katar saves 45).
5. Body armour never uses its own `code` or `alternategfx`: only the
   six `armtype` bytes.

## Test vectors

| Input | Expected output | Source |
|---|---|---|
| fresh Amazon: `jav` right hand (weapon in use), `buc` left | components 5 = 0x1B, 7 = 0x4F, rest 0xFF; colours all 0xFF | `bdAma`, `ScnAma` |
| fresh Sorceress `sst` (`alternategfx` `bst`) | component 5 = 0x25 | `bdSor`, `ScnSor` |
| Necromancer `wnd`; Paladin `ssd` + `buc`; Barbarian `hax` + `buc`; Druid `clb` + `buc`; Assassin `ktr` + `buc` | component 5 = 0x09; 0x11, 7 = 0x4F; 0x04, 0x4F; 0x0C, 0x4F; 0x2D, 0x4F | `bdNec`, `bdPal`, `bdBar`, `bdDru`, `bdAss` |
| broken `hax` in the right hand, `buc` | components 6 = 0x04, 7 = 0x4F, 5 = 0xFF | `bdMercTwo` |
| equipped normal `qui` (Torso 0, Legs 0, rArm 0, lArm 0, rSPad 1, lSPad 1) | components 1–4 = 0x01, 8–9 = 0x02; colours 1–4, 8, 9 = 0xFF | synthetic, rules 5, 6.2.4 |
| magic `buc` (`Transform` 8) whose prefix has `transformcolor` `dblu` (row 5) | component 7 = 0x4F, colour 7 = 0x06 | synthetic, rule 6, edge case 2 |
| equipped circlet (`ci0`, `alternategfx` `lit`) | no byte changed | synthetic, rule 3.1 |

## Provenance

1.14d `Game.exe`, `tools/ghidra/disasm.py` (register arguments) and
the Ghidra exports: save writer `0x00568F20` → fill `0x0063E510`;
`0x00627D40` (item data +0x44), `0x0063DA70`, `0x0063C050`,
`0x0063BEF0`, `0x00629FE0`, `0x00628660` (+0x115), `0x0063D900`,
`0x0063D710`, `0x0063D930`, `0x0064F380`, `0x00633640`, `0x0063D690`,
`0x0064F420`, `0x0064F500`, `0x0065B620`, `0x00451F60`, `0x0062C100`,
`0x0062A250`, `0x00600C20`, `0x00633EE0`, `0x0062BC20`, `0x00629B50`;
static data at `0x00744CA8` and `0x007446A0` read from the image. Field
offsets from `data/fields.tsv`. The token rule was run by a scratch
script over the 1.14d `patch_d2` `weapons`/`armor`/`misc`/`itemtypes`
tables (outside the repo) and gives the eight measured component
values of the Test vectors; saves read with `tools/d2s_check.py`
(read-only, 2026-10-07).

## Open questions

1. **Answered** (Constants, `0x00618100`, `0x00611E60`; data tables
   base `[0x00744304]` = `0x0096BC30`, +0x18C = `0x0096BDBC`): the list
   is every `states` row with `itemtype` > 0, in row order. A save made
   while Enchant is active (colour 5 = 0x00 expected for a weapon
   `Transform` of 1, 2 or 5–8) stays a useful check: recording list
   `docs/handoff/pc2-rec-pc2-items.md` IT-6.
2. Not measured: any colour byte other than 0xFF (needs a save with a
   magic, set, unique or gem-socketed coloured item equipped), and any
   body armour or helm (none of this PC's saves has one equipped).
   Needs a local save, not a recording: one character with a helm, a
   body armour and a coloured magic or unique item equipped (recording
   list IT-6).
3. The bytes of the reference table `0x00744CA8` (rule 1.2.4) are not
   in this spec. PROVISIONAL: slots 57–124 are `weap` slots and no other
   slot is reserved (because that is the smallest table that gives rule
   1.3's positions: weapons 4–56, `cap` 57, `buc` 79, the throwing
   potions 125–134; it does not give the second `ktr` at 243 of edge
   case 4, which rule 2 never returns); settled by reading the 256
   entries from the 1.14d image (local run queue; REC-91) or by the IT-6
   saves (`docs/handoff/pc2-rec-pc2-items.md`).
4. An empty `alternategfx` (code 0) in rule 2. PROVISIONAL: compared
   like any code, so it matches the first unfilled entry (98 in 1.14d)
   before a later entry holding `code` (because rule 2 states the
   comparison without an exception); only items whose `code` sits above
   the first hole (the throwing potions) or in no entry are affected;
   settled by a save with such an item in a hand (IT-6; REC-92).
