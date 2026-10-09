# Spec: UI — Item tool tips (hover description builder)

- **Status:** draft (2026-10-08, static RE of 1.14d `Game.exe`: the hover
  builder `0x0048DD90` and every function it reaches for item text; English
  strings read from the 1.14d `string.tbl` / `patchstring.tbl` (Patch_D2) /
  `expansionstring.tbl`). Unverified until the capture cases of §Test
  vectors run (`ui/text.md` capture `text-0002`).
- **Target version:** 1.14d, English install
- **Crate/module:** `d2-client::ui::item_tip` (`item_tip.rs`,
  `item_tip_desc.rs`, `item_tip_set.rs`), `d2-client::ui::shop_ui`
- **Related specs:** `ui/text.md` (§5 colour codes, §7 bottom-up lines, §14
  formatter `0x005269D0`), `ui/inventory.md` §5 (hover state and anchors),
  `ui/panels.md` §9 (inventory family, mode `[0x007BCBF0]`), `ui/menus.md`
  §4 (store transactions), `world/vendors.md` §9 (prices),
  `items/inventory.md` §4.2 / §4.8 (requirements), `items/properties.md`
  §10 / §11 / §13 (runeword and set lists), `sim/stats.md` (stat reads, by-time
  values), `data/runtime-maps.md` §3 (description order),
  `data/field-types.md` §7 (string ids), `ui/control-panel.md` r8 (belt hover
  text, which reuses §5 and §6 here)

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 47–58 |
| Inputs | 59–71 |
| Outputs / state changes | 72–76 |
| Rules | 77–85 |
|   1. Entry and dispatch (`0x0048DD90`) | 86–102 |
|   2. Main builder: blocks | 103–144 |
|   3. Main builder: line shapes | 145–314 |
|   4. Name colour | 315–325 |
|   5. Item name (`0x0048C060(I, buffer, max)`) | 326–368 |
|   6. Property lines (`0x004E6410` → `0x004E60A0`) | 369–420 |
|   7. One stat line (`0x004E4D80`) | 421–477 |
|   8. Damage groups (`0x004E49C0`, `0x004E5A20`) | 478–495 |
|   9. Set item tip (`0x0048D1D0`) | 496–534 |
|   10. Other tips | 535–548 |
|   11. Store lines (`0x0048CEA0`, `0x004B2AD0`) | 549–568 |
| Constants & data dependencies | 569–589 |
| Randomness | 590–593 |
| Edge cases & original bugs | 594–614 |
| Test vectors | 615–649 |
| Provenance | 650–675 |
| Open questions | 676–687 |
<!-- /index -->

## Summary

When the mouse rests on an item in the inventory family of panels
(inventory, stash, cube, store, trade), the client builds one UTF-16 text
and queues it as the hover pop-up. The text is a stack of blocks; each
non-empty block gets a colour code in front; the string is drawn from its
first line at the bottom upward (`ui/text.md` §7), so the builder appends
the bottom block first and the item name last. This spec gives the block
order, every line shape and string id, the colours, the store price line,
the set-item variant (which set bonuses show) and the stat-line engine with
all 28 `descfunc` shapes.

## Inputs

| Name | Type | Source |
|---|---|---|
| hovered item I | client item unit | `[0x007BCBF4]`, hover flags `[0x007BCBE4]` / `[0x007BCBE8]` (`ui/inventory.md` §5) |
| panel unit U | unit | first argument of `0x0048DD90`; replaced by the local player when the second argument ≠ 0 and a player exists (`0x00463DD0`) |
| local player P | unit | `0x00463DD0` / `0x00463DE0` (`client/model.md`) |
| inventory mode | int | `[0x007BCBF0]` (`ui/panels.md` §9 r1; 1–9 = NPC store states) |
| store-item flag | int | `[0x00721E38]` = 1 when the hovered item is the player's own (set by `0x004873A0`, `0x004889D0`); 0 for a store item |
| gamble store | int | `[0x007C0DB0]` ≠ 0 (`0x004B3500`, `ui/menus.md` §4): 1 in a store window opened by the Gamble option (`ui/menus.md` §4.2 r2) |
| anchors | int | `[0x00721E3C]` x, `[0x00721E40]` top, `[0x00721E48]` bottom (`ui/inventory.md` §5 r1) |
| tables | | items, itemtypes, itemstatcost, charstats, skills/skilldesc, sets/setitems, uniqueitems, affixes, gems, monstats, montype, string tables |

## Outputs / state changes

One pop-up queued with `0x00502280(text, x, y, colour, centred 1)`
(`D2Win_SetPopUpUnicodeText`). Nothing else changes.

## Rules

Notation. `Pfx(B, k)` = `0x004521C0`: if block B is non-empty, B := `ÿc` +
the unit `'0' + k` + B (an empty block gets nothing). NL = string 3998
(LF), SP = 3995 (space). "Bonus of stat s" = unit total − base
(`0x00625560`, `sim/stats.md`). Numbers are decimal (`%i` / `%d`,
negative with `-`). "Append" is `0x00526700`, "copy" `0x005267E0`. Block
lists below are in **string order = bottom line first**.

### 1. Entry and dispatch (`0x0048DD90`)

1. Before the item part the function queues panel button tool tips (close,
   gold, swap); those belong to `ui/panels.md` / `ui/panels-2.md`.
2. Item part runs only when a hover flag is set, I exists, U has an
   inventory (+0x60), there is no cursor item (`0x0063C1E0`), and (when UI
   state 0x17 is closed, `0x004538D0`) I is in U's inventory
   (`0x0063AD50`). Otherwise nothing.
3. **Gamble**: store item (`[0x00721E38]` = 0) in a gamble store with mode
   1–9 → §10.3.
4. Cursor mode (`0x00468830`) = 8, or I's items `Transmogrify` (+0x139) ≠
   0 (`0x0062A0A0`) → §10.2.
5. Quality (`0x00627E70`) 5 and I identified (item flag 0x10) → §9.
6. Primary type (`0x0062B400`) 18 (`book`) → §10.1.
7. Else the main builder §2. An items record must exist (`0x006335F0`;
   none → fatal 0xFC2).

### 2. Main builder: blocks

Built in this order (bottom → top), each `Pfx` with the colour given:

| # | Block | Condition | Colour | Rule |
|---|---|---|---|---|
| A | ethereal / sockets line | I is-a 45 `weap` or 50 `armo` (`0x00629BB0`) | 3 | §3.1 |
| B | property lines | identified | 3 | §6 (`0x004E6410(I, 0x1000, 1, 0)`) |
| C | `Unidentified` (3455) + NL | not identified | 1 | |
| D | speed line | is-a 45 | 0 | §3.2 |
| E | `Required Level:` | identified | 1 if the level test fails, else 0 | §3.3 |
| F | `Required Strength:` | is-a 45 or 50, items `reqstr` ≠ 0 | 1 if the strength test fails | §3.3 |
| G | `Required Dexterity:` | is-a 45 or 50, `reqdex` ≠ 0 | 1 if the dexterity test fails | §3.3 |
| H | class-only line | item class (`0x0062C0B0`) ≠ 7 | 1 if U's class (U +4; no U → −1) ≠ that class | §3.4 |
| I | durability | I not a quiver (`0x0062E740` = 0) | 0 | §3.5 |
| J | socket-filler text | is-a 53 `sock` | 0 | §3.6 |
| K | `Keep in Inventory to Gain Bonus` (20438) + NL | is-a 13 `char` | 0 | |
| L | quantity, then spell line | §3.7 | 0 | §3.7 |
| M | damage lines | is-a 45, `0x00625EF0(I,0)` ≥ 0 and `0x00625E60(I,0)` ≥ 0 | 0 | §3.8 |
| N | smite / kick | §3.9 | 0 | §3.9 |
| O | block chance | is-a 51 `shld` | 0 | §3.10 |
| P | defense | is-a 50 and I's own total of stat 31 (`0x00626020`) > 0 | 0 | §3.11 |
| Q | rune letters | items `hasinv` ≠ 0 (`0x00629900`) | 4 | §3.12 |
| R | name | always | §4 | `0x0048C060(I, buffer, 256)`, §5 |

Then:

1. Store modes 1–9: text := `0x0048CEA0` over A…R (§11), else A…R.
2. Quest item (items `quest` +0x12A ≠ 0) whose code is not `leg`: when
   mode = 0 and (no I or I's mode, unit +0x10, = 0): code `bkd` → text :=
   string 2205 (`Right Click to Read`) + NL + text; code `box` → string 2204
   (`Right Click to Open`) + NL + text. Then (any such quest item) `Pfx(text,
   4)` and pop-up colour 4. Otherwise pop-up colour 0.
3. A text of ≥ 1023 units is cut to 1023 units (`0x0048ED19`).
4. Height H and width from `0x00502520`; y := `[0x00721E40]` when
   `[0x00721E40]` − H > 0, else `[0x00721E48]` + H; x := `[0x00721E3C]`;
   `0x00502280(text, x, y, colour, 1)`.

U is used for the requirement and class tests (E–H) and the defense line;
P (local player) for speed, damage, smite / kick, block, spell line and
every per-level value of §7.1.

### 3. Main builder: line shapes

#### 3.1 Ethereal and sockets (`0x00484B10`)

Ethereal (item flag 0x400000) → `Ethereal (Cannot be Repaired)` (22745).
Socketed (flag 0x800) → (`, ` when ethereal) + `Socketed` (3453) + SP +
`(n)`, n = the socket count byte (`0x006299B0`), + NL. Ethereal and not
socketed → the ethereal text + NL. Neither → empty.

#### 3.2 Speed line (`0x004861D0`)

1. s := `0x0062A710(P, I)`: frames F and speed V from the animation-data
   query (`formats/animdata.md` §6, `0x0066AA80`) for P's class, mode 7
   (attack 1) and I's weapon class (`0x00628500`, COF name composer
   `0x0064F5B0`); a = 100 + I's stat 93 (`item_fasterattackrate`, item or
   skill total `0x00625500`) + I's stat 68 (`attackrate`); s = (F × 256) /
   ((a × V) / 100) (signed, truncating). Query not found → s = 45. (F = 0
   → fatal 0x1538.)
2. Category c: s ≥ 28 → 5; s < 10 → 1; else c := T[s − 10][k] with T the
   18 × 5 table `0x00721F10` and k := table `0x00722078` at 2 × class + b
   (class = P +4, b = 1 when I is-a 27 `bow` or 35 `xbow`). k by class
   (b = 0 / 1): Amazon 0 / 2, Sorceress 1 / 4, Necromancer 1 / 4, Paladin
   0 / 3, Barbarian 0 / 3, Druid 1 / 4, Assassin 0 / 3. (No P: r1 is
   already fatal 0x151F.)

   | s | k 0 | 1 | 2 | 3 | 4 |
   |---|---|---|---|---|---|
   | 10–12 | 1 | 1 | 1 | 1 | 1 |
   | 13 | 1 | 1 | 2 | 1 | 1 |
   | 14 | 2 | 1 | 2 | 2 | 1 |
   | 15 | 2 | 1 | 2 | 2 | 2 |
   | 16 | 2 | 2 | 3 | 2 | 2 |
   | 17 | 3 | 2 | 3 | 3 | 2 |
   | 18 | 3 | 2 | 3 | 3 | 3 |
   | 19 | 3 | 2 | 4 | 3 | 3 |
   | 20 | 4 | 3 | 4 | 4 | 3 |
   | 21 | 4 | 3 | 4 | 4 | 4 |
   | 22 | 4 | 3 | 5 | 4 | 4 |
   | 23 | 5 | 4 | 5 | 5 | 4 |
   | 24–25 | 5 | 4 | 5 | 5 | 5 |
   | 26–27 | 5 | 5 | 5 | 5 | 5 |

3. Speed text S := string `0x00721E88`[c] (6-byte rows): 0 4088 `Fastest
   Attack Speed`, 1 4089 `Very Fast …`, 2 4090 `Fast …`, 3 4091 `Normal …`,
   4 4092 `Slow …`, 5 4093 `Very Slow …` (6 4094 `Slowest …` unused).
   I's bonus of stat 93 ≠ 0 → `Pfx(S, 3)`.
4. Class word: the first row of `0x00721EB0` (15 rows of type, string) whose
   type I is-a: 26 `staf` 4085 `Staff Class`, 28 `axe` 4078, 30 `swor`
   4079, 32 `knif` 4080 `Dagger Class`, 38 `tpot` 4081 `Equip to Throw`,
   44 `jave` 4082, 33 `spea` 4083, 27 `bow` 4084, 34 `pole` 4086, 35 `xbow`
   4087, 67 `h2h` 21258 `Claw Class`, 88 `h2h2` 21258, 68 `orb` 4085, 25
   `wand` 4085, 57 `blun` 4077 `Mace Class`. Found → line = word + SP + `-`
   (3996) + SP + S + NL; none → S + NL.

#### 3.3 Requirements

1. Tests: `0x0062EAF0(I, U, 0, &str_ok, &dex_ok, &lvl_ok)`
   (`items/inventory.md` §4.2, "equipping" = 0).
2. Bonuses: p := I's stat 91 (`0x00625500`); p ≠ 0 → bs := pct(reqstr, p,
   100), bd := pct(reqdex, p, 100) (`0x00483360`); ethereal → bs −= 10, bd
   −= 10.
3. Strength (`0x004850A0`): v := reqstr + bs; v ≤ 0 → empty; else
   `Required Strength:` (3458) + SP + v + NL. Dexterity (`0x00485170`):
   the same with reqdex, bd, 3459 `Required Dexterity:`.
4. Level (`0x00484FF0`): R := `0x0062BA60(I, U)` (`items/inventory.md`
   §4.8); only when R > 1: `Required Level:` (3469) + SP + R + NL.

#### 3.4 Class-only line

String 10917 + class (`(Amazon Only)` 10917, Sorceress 10918, Necromancer
10919, Paladin 10920, Barbarian 10921, Druid 10922, Assassin 10923) + NL.

#### 3.5 Durability (`0x00484E90`)

Only when I has durability (`0x00629930`), max durability m
(`0x00625E00`) > 0 and I is not throwable (`0x0062BA80`): `Durability:`
(3457) + SP + d + SP + `of` (3463) + SP + M + NL, d = I's stat 72, M = m
with `Pfx(M, 3)` when I's bonus of stat 75 ≠ 0. For a quiver the builder
calls `0x00484DB0` instead, whose text (quantity + NL + base name) is
written into the final buffer and then overwritten (dead; nothing shows).

#### 3.6 Socket fillers (`0x004865D0`)

`0x004E6850` (gems: I is-a 20 `gem`; runes: is-a 74 `rune`; else nothing;
the gems row comes from items `gemoffset`, `0x006372C0`; none → fatal) then
`Can be Inserted into Socketed Items` (11080) + NL. `0x004E6850`: NL,
then for slot labels `Shields:` (11074, gems slot 2), `Helms:` (11073, 1),
`Armor:` (11076, 1), `Weapons:` (11075, 0) in this order: the slot's mods
are applied to a temporary list of I (`0x0065FEC0`, mode 2 for a gem, 5 for
a rune), described with §6 (`0x004E6410`, gems one-line, runes multi-line,
label = word + SP), the list freed (`0x00625790`, `0x006277E0`,
`0x00626CD0`), and a final LF removed; between slots NL; after the last two
NL.

#### 3.7 Quantity and spell line

1. Identified and not socketed: `0x00486100`: I's stat 70 > 0 or total max
   stack (`0x006295B0`) > 0 → `Quantity:` (3462) + SP + stat 70 + NL.
2. Always appended: `0x00486370(I)`: needs items `spelldesc` (+0xB4) ≠ 0, a
   local player, and `spelldescstr` (+0xB6) ≠ 5382. By `spelldesc`:
   1 → str + NL; 2 → v := `0x00627C20(P, I, calc1 +0xA4)`
   (`data/calc-expressions.md`), adjusted by items `stat1` (+0x9E): 6 or
   74 → `0x0062A5D0(P, v)`, 8 or 26 → `0x0062A620(P, v)`; line = str + SP
   + v + NL; 3 → as 2 without the adjustment; 4 → `format(str, v)`
   (`ui/text.md` §14) + NL; other → nothing.

#### 3.8 Damage (`0x00485410`)

Value pair `D(smin, smax)` (`0x00485240`): defaults 1 / 2; with P: the
item's totals of smin / smax read while I's list is linked to its owner
(P, or the unit I's list hangs under, `0x00625820`; link `0x00627910`,
restore `0x006277F0` / `0x00627910`); blue := base(smin) < total(smin) or
base(smax) < total(smax); max := max(max, min); plus I's stat 272
(`item_maxdamage_bytime`) by time to max and stat 273 as a percent of max
(each by-time value `0x0065CA30` with the owner's act, `sim/stats.md` §8;
non-zero → blue).

1. I is-a 38 `tpot` → thrown potion: skill k := `0x006288A0(I)`; min :=
   elemental min (`0x0064B100`), max := elemental max (`0x0064B1D0`), both
   level 1 for P; element e (`0x0064B0C0`): 1 colour 1, 2 colour 4, 3
   colour 0, 4 colour 3, 5 colour 2 and both divided by max(1, length
   (`0x0064B2A0`) / 25); then min := (min + `0x0064AF20`) >> 8, max :=
   (`0x0064AFF0` + max) >> 8, max := max(max, min). Line: `Throw Damage:`
   (3467), `ÿc0`, SP + `Pfx(min, e-colour)`; if max ≠ min: SP + `to`
   (3464) + SP + `Pfx(max, colour)`; NL.
2. Else, `0x0062A1E0(P, I)` ≠ 0 (both one- and two-handed for P): two
   lines, each `ÿc0` + label + SP + [`ÿc3` when blue] min + SP + `to` + SP
   + max + NL (numbers cut to 4 digits): first `Two-Hand Damage:` (3466)
   with D(23, 24), then `One-Hand Damage:` (3465) with D(21, 22). One-hand
   shows above two-hand.
3. Else one line: two-handed (`0x006289C0`) → D(23, 24) with 3466, else
   D(21, 22) with 3465; max ≤ min → max := min + 1; label + SP + [`ÿc3`]
   min + SP + `to` + SP + max + NL.
4. Throwable (`0x0062BA80`), after 1–3 (so above them): blue := I's bonus
   of stat 18, 17, 159 or 160 ≠ 0, or the blue of D(159, 160); line
   `Throw Damage:`, `ÿc0`, SP + `Pfx(min, k)` + SP + `to` + SP + `Pfx(max,
   k)` + NL with k = 3 if blue else 0.

#### 3.9 Smite and kick (`0x00485D40`)

Called for a shield (is-a 51) when U is a Paladin (class 3) and the item
class is none (`0x0062C060` = 0) or 3; for boots (is-a 15) when U is an
Assassin (class 6). min / max := items `mindam` / `maxdam` (+0xFE /
+0xFF); P has skill 117 (`0x006439F0`) and state 101 (`0x00639DF0`) →
min += `0x00647BC0(P, 117, level, 1)` >> 8, max += `0x00647D00(…)` >> 8
(level `0x006442A0(P, skill, 1)`). Label: shield `Smite Damage:` (3468),
boots `Kick Damage:` (21782). Line: label + SP + min + SP + `to` + SP + max
+ NL.

#### 3.10 Block (`0x00485BE0`)

b := I's stat 20 total; with P: b += charstats `BlockFactor` (+0x49) of P's
class; P has skill 117 and state 101 → b += `0x00645C10(level, 117)`; b >
75 → 75. b = 0 → empty. Line: `ÿc0` + `Chance to Block: ` (11018) + N,
N = `%d%%`-text of b + NL, `Pfx(N, 3)` when b > items `block` (+0x111).

#### 3.11 Defense (`0x00485EE0`, U, I)

v := I's own total of stat 31 (`0x00626020`) read under the owner link of
§3.8; blue := base 31 (`0x006253B0`) ≠ v; I's stat 268 by time → v += it;
stat 269 by time → v += pct(v, it, 100) (`0x00483360`); either non-zero →
blue (by-time needs the owner's act, unit +0x1C). Line: `Defense:` (3461) +
SP + V + NL, V = v with `Pfx(V, 3)` when blue.

#### 3.12 Rune letters (`0x00486670`)

Over the fillers in I's inventory, in list order: a filler is-a 74 with a
gems row (items `gemoffset` > 0, `0x006372C0`): first one → `'` (20506);
then the row's letters (+0x20, ASCII, 5 units). At least one → `'` + NL.

### 4. Name colour

1. By quality: 4 → 3, 5 → 2, 6 → 9, 7 → 4, 8 → 8, 9 → 10 (`ÿc:`); 1–3 →
   5 when socketed (0x800) or ethereal (0x400000), else 0.
2. Not identified, mode 1–9 and a store item → 0.
3. Code `ceh`, `bet`, `fed`, `tes`, `toa`, `dhn`, `bey`, `mbr`, `pk1`,
   `pk2`, `pk3`, or I is-a 74 → 8.
4. Broken (flag 0x100) → 1.

Later rules win. Quest colours of §5 r6 are inside R and override.

### 5. Item name (`0x0048C060(I, buffer, max)`)

Bottom line first; "base" = items `namestr` (+0xF4) text with a leading
grammar tag removed (`0x004834A0`: a last `]` at index ≥ 3 with `[` three
units before → the text after it). `%n` templates are filled by
`0x0048BE80`: for English templates (no `:`) each `%n`, n = 0, 1, …, is
replaced by argument n; when the text built so far ends with SP and the
template unit after `%n` is SP, that SP is skipped (an empty first argument
leaves a leading SP, §Edge cases).

1. Runeword (flag 0x4000000): base + NL + `Pfx(W, 4)`, W = string of prefix
   slot 0 (`0x00627EC0`).
2. Not identified: base.
3. Identified, by quality:
   1. low: 1712 `%0 %1` (low-quality row name +0x20, base); no row → empty.
   2. normal: type 22 `scro` or 18 `book`: prefix slot 0 = 0 → `Tome of
      Town Portal` (2199, primary type 18) / `Scroll of Town Portal` (2200);
      = 1 → 2201 / 2202 Identify; else empty. Type 7 `play` (ear):
      [`Hardcore` (5126) + NL when flag 0x8000] + `Level` (4141) + SP +
      ear level (`0x006283A0`) + NL + class name (`0x00484A70`) + NL +
      possessive of the ear name with base (`0x005272B0`). Type 40 `body`:
      1716 `%0 %1` (monstats `NameStr` of the file index, base). Socketed
      with a filler (`0x0063CD60`) → 1715 `%0 %1` (`Gemmed` 1728, base).
      Else base.
   3. superior: 1711 `%0 %1` (`Superior` 1727, base).
   4. magic: 1714 `%0 %1 %2` (prefix name, base, suffix name); an affix
      row with name id 0 shows its key in parentheses; none → empty
      argument.
   5. set: setitems row (`0x00483440`) none → empty; else base + NL +
      name (+0x24) — possessive (§5 r5) when personalized, else 10089 `%0`.
   6. unique: no row → base; items `SkipName` (+0x144) = 0 → base + NL;
      then the unique name (+0x22), possessive when personalized.
   7. rare, crafted, tempered: base + NL + 1718 `%0 %1` (rare prefix,
      rare suffix; possessive when personalized).
   Magic and rare: length(prefix) + length(suffix) + length(base) + 3 ≥
   max → fatal assert.
4. Personalized (flag 0x1000000) and quality not 5–9 → the whole name
   becomes the possessive form (`0x00484C90`).
5. Possessive = `0x005272B0(player name of the item, 512, text, language)`.
6. Quest item (`quest` ≠ 0): `questdiffcheck` (+0x12B) ≠ 0 and I's stat
   356 < the current difficulty (`0x0044DCD0`) → `Pfx(name, 1)`; else code
   ≠ `leg` → `Pfx(name, 4)`.

### 6. Property lines (`0x004E6410` → `0x004E60A0`)

`0x004E6410(I, out, size, multi, label)` builds the text T of
`0x004E60A0(I, T, size, undead 1, state 0, rune state 171, flags 0x40,
multi)` and appends it to out:

1. Primary type 11 (`elix`, `0x0062B400`) → T is built by
   `0x004E5E90(I, T)` instead, and rules 2–6 do not run (rule 7 does).
   Table `0x0072D6C0`: 6 entries of 16 bytes (count `[0x0072D720]`):
   stat, kind, string id, mode = (0, 1, 3498 `Elixir of Strength`, 2),
   (1, 1, 3500 `Elixir of Energy`, 2), (2, 1, 3499 `Elixir of
   Dexterity`, 2), (3, 1, 3501 `Elixir of Vitality`, 2), (9, 1, 3502
   `Elixir of Mana`, 2), (7, 1, 3503 `Elixir of Life`, 2) (the string
   is stored twice, for v > 0 and v < 0; both are equal). For each
   entry in order whose stat = I's file index (item data +0x28,
   `0x00629DA0`):
   - v := I's stat 71 (`value`, layer 0, `0x00625480`); for entry stat
     6–11 v >>= 8 (arithmetic; here stats 7 and 9). v = 0 → next entry.
   - N := `%i` of v (`0x00413A40`, 10-byte buffer), widened to at most
     8 units (`0x00526320`, limit 9).
   - kind ≠ 1 → fatal (`0x004E6079`, error 0x141); mode ≠ 2 → fatal
     (error 0x13D). Every 1.14d entry passes both.
   - Line := the entry's string + SP (3995) + (`+` (4002) when v > 0) +
     N + NL (3998), appended to T.
   T starts empty, so with no matching entry or v = 0, T is empty. 1.14d:
   only the `elixir` misc row is type 11; creation gives it a file index
   from the same six stats and its `value` (`items/generation.md` §5.2).
   Example: file index 7, stat 71 = 512 → `Elixir of Life +2` + LF.
2. The shown list L = sum of: I's list of state 0 with flags 0x40
   (`0x006257D0`), I's list of state 171 (runeword list,
   `items/properties.md` §10) and, for each filler in I's inventory, its
   list of the same state and flags (`0x006274F0` adds stats). Callers
   that pass another state (§9) give rune state 0: no state-171 list.
3. Undead line: undead = 1, I is-a 57 `blun` and I's stat 122 = 0 → `+` +
   `50` + `%` + SP + `Damage to Undead` (3554) + NL first.
4. Group state (`0x004E49C0`, §8) is computed from L.
5. For each stat s of the description list (`data/runtime-maps.md` §3,
   ascending `descpriority`), for each (layer, value) of s in L
   (`0x006261B0`, up to 511) with value ≠ 0: if §8 consumes s, skip; else
   line := §7(s, layer, value); none → skip; s = 23 and L has stat 21, or
   s = 24 and L has 22 → skip. multi: line + NL appended; one-line: `, `
   (3852 + SP) before every line but the first.
6. Indestructible: I an item with items `nodurability` (+0x113) = 0,
   `durability` (+0x112) ≠ 0, stat 152 < 1 and max durability = 0 → line
   `Indestructible` (21240) (same separator rule).
7. Label (when non-empty): T without an inner LF (before its last unit) →
   out += label + T; else out += T up to its last inner LF, NL, label, the
   last line without its final LF, `,` (3852), NL.

Because lines are appended in ascending priority, the highest priority is
the top line.

### 7. One stat line (`0x004E4D80`)

#### 7.1 Value and strings

1. Invalid stat → no line.
2. v := value (`0x004E4C50`): itemstatcost `op` (+0x54) 2–5 → v := ((P's
   total of `op base` (+0x56) >> that stat's `valshift`) × v) >> `op param`
   (+0x55); then v >>= `valshift` (+0x18); stat 122 on an item is-a 57 → v
   += 50.
3. Group (`0x004E4CE0`): when `dgrp` (+0x3E) ≠ 0 and every stat with the
   same `dgrp` has, in L at layer 0, the same §7.1 r2 value as v: only the
   lowest-numbered member prints (others give no line), using `dgrpfunc`,
   `dgrpval`, `dgrpstrpos` / `dgrpstrneg`, `dgrpstr2` (+0x40 … +0x46).
   Else `descfunc`, `descval`, `descstrpos` / `descstrneg`, `descstr2`
   (+0x36 … +0x3C).
4. str := pos (v ≥ 0) or neg (v < 0) text; N := `%i` of v; `+` (4002), `%`
   (4001), SP. "S(x)" below = `+` + x when the sign test says so.
5. Placement by `descval` dv: 1 → value-part + SP + str; 2 → str + SP +
   value-part; 0 → str alone (functions marked "dv").

#### 7.2 `descfunc` table

| f | Value part / text | Notes |
|---|---|---|
| 1, 6 | `+N` when v > 0, else N | dv |
| 12 | as 1, but v = 1 → empty value part (dv 1 gives SP + str) | dv |
| 2, 7 | N + `%` | dv |
| 3, 9 | N | dv |
| 4, 8 | (`+` when v ≥ 0) + N + `%` | dv |
| 5, 10 | n + `%`, n = v × 100 / 128 truncated toward zero | dv |
| 11 | v > 0: t = 2500 / v; t ≤ 30 → format(21241 `Repairs %d durability per second`, 1); else format(21242 `Repairs %d durability in %d seconds`, 1, (t + 12) / 25); v ≤ 0 → format(21241, 25) | no str |
| 13 | v = 0 → no line; charstats(layer) `StrAllSkills` (+0x52) as str; value `+N` (v > 0) or N; dv 1 / 2 as above, dv 0 → empty line | |
| 14 | tab = layer & 7 (> 2 → no line), class = layer >> 3 (charstats row; none → no line); format(`StrSkillTab1+tab` (+0x54 + 2·tab), v) + SP + `StrClassOnly` (+0x5A) | |
| 15 | skill = layer >> 6, level = layer & 0x3F (`stuff` 6, `data/runtime-maps.md` §3); skill not in 1 … count − 1 → no line; format(descstrpos, v, 0, level, skill name) | `%%` eats the 0 (`ui/text.md` §14) |
| 16 | format(str, v, skill name of layer) | |
| 17, 18 | by time: period p = clamp(v & 3, 0, 3) → line 1 = string 21235 / 21237 / 21234 / 21236 (`(Increases During Daytime / Near Dusk / Nighttime / Near Dawn)`) + NL; x := `0x0065CA30(v, …)` with the client act's time (`[0x007A0634]`, `0x0061C100`), no act → the low bound ((v >> 2) & 0x3FF) − 256; value part `+x` (x ≥ 0), x (x < 0 and v < 0), else empty; f 18 adds `%`; dv 1 / 2, dv 0 → line 1 only | two lines |
| 19 | format(str, v) | |
| 20, 21 | n = −v: (`+` when n ≥ 0) + `%i`(n) + `%` | dv; 21 identical to 20 |
| 22 | as 4, then `:` (3997) + SP + montype row (layer) +0x0A text (`0x004E4640`; out of range → row 0) | dv 0 also gets the suffix |
| 23 | monstats row (layer, `0x00451F80`) none → no line; N + `%` placed by dv, then SP + monstats `NameStr` | |
| 24 | skill / level from layer as 15; `Level` (21249) + SP + level + SP + skill name + SP + format(str, v & 0xFF, v >> 8) | `(%d/%d Charges)`: current / max |
| 25, 26 | value part from −v's sign but the text N of v: `+N` when v < 0, else N | dv; no 1.14d stat uses them |
| 27 | v = 0 or no name → no line; (`+N` when v > 0, else N) + SP + `to` (4003) + SP + skill name (layer) + SP + `StrClassOnly` of the skill's class (`0x00645080`, class ≤ 6), nothing after the SP when classless | |
| 28 | skills row (`0x0045C4B0`) none or v = 0 → no line; unit = the described unit when it is a player (§9 r3), else P; skill class (+0x0C, signed) = unit class and v > 3 → v = 3; `+N` / N + SP + `to` + SP + skill name | |
| 0, > 28 | no line | |

Skill name = skilldesc `str name` of the skill's skilldesc row
(`0x004E6CE0(skill, 0)`); any miss gives 5382 (`an evil force`). After f
6–10 and 21: SP + `descstr2` text, or SP + 11091 `(Based on Character
Level)` when `descstr2` is 5382.

Stats per function in 1.14d (patch `itemstatcost`): 1 (39), 2 (24), 3 (19),
4 (24), 5 (1), 6 (20), 7 (10), 8 (5), 9 (1), 11 (1), 12 (2), 13 (1), 14
(6), 15 (6), 16 (1), 17 (17), 18 (18), 20 (9), 22 (2), 23 (1), 24 (1), 27
(1), 28 (1); groups: `dgrp` 1 (stats 0–3, `+N to all Attributes` 10977)
and 2 (39, 41, 43, 45, f 19 `All Resistances +%d` 10024).

### 8. Damage groups (`0x004E49C0`, `0x004E5A20`)

Pairs read from L (layer 0): min damage 21 (or 23 when 21 = 0) / max 22
(or 24); enhanced 18 / 17; fire 48 / 49; lightning 50 / 51; magic 52 /
53; cold 54 / 55; poison 57 / 58 with length 59 and count 326. A pair is
"on" when both values > 0. For stat s of §6 r5:

| s | Pair off | Pair on |
|---|---|---|
| 17, 49, 51, 53, 55, 58, 59 | normal line | skipped |
| 18 | normal line | `+N% Enhanced Damage` = `+` + `%d` of stat 18 + `%` + SP + 10023 |
| 21, 22, 23 | normal line | first of them: min ≥ max → both pair flags cleared, normal line; else format(3623 `Adds %d-%d damage\n`, min, max); later ones skipped |
| 24 | normal line | skipped |
| 48 / 50 / 52 / 54 | normal line | min ≥ max → format(3612 / 3616 / 3618 / 3614, max) (`+%d fire damage\n` …); else format(3613 / 3617 / 3619 / 3615, min, max) |
| 57 | normal line | c := max(count, 1); len := length / c; min' := (min × len + 128) >> 8; max' likewise; sec := len / 25; min' ≥ max' → format(3620, max', sec), else format(3621, min', max', sec) |

Group texts carry their own LF and are appended directly (no separator).

### 9. Set item tip (`0x0048D1D0`)

U' := U; when U is a monster (type 1) that `0x0063EE90` rejects, U' := P.
Blocks, bottom first:

1. **Member list**: for each member of the set (sets +0x110, count +0x0C,
   setitems order): name := 10089 `%0` of the member's name (+0x24) + NL, `Pfx(·, 2)` when U' owns the member
   (`0x00486770`: an item of U''s inventory with quality 5, identified, page
   0 / 3 / 4 / none and node kind 1, 3 or 4, of that setitems row; belt and
   cursor do not count), else `Pfx(·, 1)`. The list gets `Pfx(·, 2)`.
2. Set name (sets +0x02) + NL, colour 4.
3. Owner bonuses (`0x004E6680`, only when I's mode = 1, equipped): U''s
   list of a state 165–170 whose stat 71 = I's set id (`items/properties.md`
   §13 r6) described with §6 (state given, multi): when non-empty, NL +
   `Pfx(·, 4)`.
4. NL.
5. Item bonuses (`0x004E6560`, U', I): add func f (setitems +0x87): f = 1 →
   mask := set slots of U''s body items of the set without I
   (`0x0062A370(U', I, 0)`), n := I's slot (+0x2E); for i = 0 … 5, i ≠ n
   and mask bit i: describe I's list of state 165 + (i − 1 if i > n else
   i). f = 2 → b := popcount(`0x0062A370(U', I, 1)`) (I counts only when
   it is worn); describe I's lists 165 … 165 + b − 2. Each with §6 (state,
   multi, no undead line). `Pfx(·, 2)`.
6. `Pfx(·, 3)` of: §3.1 only when socketed, then §6 for state 0 (multi,
   undead line).
7. Base block: E level (red when failing), F, G, H (red only when U' is a
   player of another class), I durability, D speed (weapons), M damage, N
   smite (Paladin only; no kick), O block, P defense, then the name (§5)
   with `Pfx(·, 2)`, or `Pfx(·, 1)` when broken. No C, J, K, L, Q.
8. Store modes 1–9: price as §11 (text `0x004B2AD0`, store-item flag); no
   price → `Item cannot be traded here.` in red unless mode 4. No repair
   note.

So a set item shows: its blue lines; the green partial bonuses that the
worn pieces currently switch on; on an equipped piece only, the gold
set-wide bonuses currently active on U' (partial and, with every piece
worn, the full set); the set name in gold; the member names green / red.
Steps not reached are never listed.

### 10. Other tips

1. **Tome** (`0x0048CFF0`): quantity (§3.7 r1); mode 0 → `Right Click to
   Use` (2203) + NL + `Insert Scrolls` (2206) + NL; name (§5, 128 units);
   store price; no colour codes; pop-up colour 0.
2. **Transmogrify** (`0x0048DB80`): target row from items `TMogType`
   (+0xC8, `0x00633640`; none → fatal); text: target base + NL + `Right
   click to make` (5387) + NL + name (§5); store price; colour 0. No
   1.14d item has `Transmogrify` ≠ 0.
3. **Gamble**: `Unidentified` + NL; class line by primary type 60–88:
   Amazon 60, 85, 86, 87; Barbarian 61, 71; Necromancer 62, 69; Paladin 63,
   70; Sorceress 64, 68; Assassin 65, 67, 88; Druid 66, 72, 73 (+ NL); name
   (`0x0048C060` 128 units); all `Pfx(·, 0)`; then §11; colour 0.

### 11. Store lines (`0x0048CEA0`, `0x004B2AD0`)

1. Mode 4 (repair) and I ethereal: + NL + `Pfx(22746 This item cannot be
   repaired., 1)`.
2. Price text `0x004B2AD0(I, sell = [0x00721E38], &price, buf, 64)`;
   success with a non-empty text → + NL + text (top line; no colour code,
   so it continues the name's colour). Failure: mode ≠ 4 → + NL +
   `Pfx(3333 Item cannot be traded here., 1)`.
3. `0x004B2AD0`: gold (primary type 4) → fail. Store item: label 3329
   `Cost: `, price = `0x0062FDC0(P, I, difficulty, quest flags, NPC class,
   t)`, t = 2 in a gamble store else 0. Player item (needs the active NPC
   `[0x007C0D25]`): by NPC class (`ui/menus.md` §4 r2 lists): sell classes
   → needs `0x0062A130(I)`, label 3331 `Sell value: `, t = 1; repair
   classes with the repair button on: not identified → text :=
   `Pfx(4022 Cannot repair unidentified items, 1)`, success; repairable
   (`0x004B1F80`) → 3330 `Repair cost: `, t = 3; else fail; identify
   classes: identified → empty text, success; else 3332 `Identify cost:
   `, price 0 when quest flag 4 is set (`0x0065C310`), else 100. Text =
   label + `%d` of the price; label length + 10 > 64 → fail.

## Constants & data dependencies

- Strings (ids per `data/field-types.md` §7; text English): 3453–3469,
  3554, 3612–3623, 3852, 3994–4003, 4077–4094, 4141, 5126, 5387, 10023,
  10024, 10089, 10917–10923, 10977, 11018, 11073–11076, 11080, 11091,
  1711–1718, 1727, 1728, 2199–2206, 3329–3333, 4022, 20438, 20506,
  21234–21242, 21249, 21258, 21782, 22745, 22746.
- Tables in `Game.exe`: speed categories `0x00721F10` (18 × 5 i32),
  class column `0x00722078` (16 i32; `0x00722070` = 5, 5 before it), speed
  strings `0x00721E88`, weapon class words `0x00721EB0`, by-time period
  strings `0x006DBD88`, set states `0x006DBD70` (165–170), popcount
  `0x006DBD90`, damage-group stat table `0x0072CDD0` (count `0x0072CDCC`).
- Columns: items `namestr`, `reqstr`, `reqdex`, `block`, `mindam`,
  `maxdam`, `durability`, `nodurability`, `quest`, `questdiffcheck`,
  `SkipName`, `gemoffset`, `spelldesc`, `spelldescstr`, `calc1`, `stat1`,
  `TMogType`, `Transmogrify`, `code`; itemstatcost `descpriority`,
  `descfunc`, `descval`, `descstr*`, `dgrp*`, `op`, `op param`, `op base`,
  `valshift`; charstats `BlockFactor`, `StrAllSkills`, `StrSkillTab1–3`,
  `StrClassOnly`; setitems `add func`, name; sets name; uniqueitems name;
  skilldesc `str name`; gems letters.

## Randomness

None.

## Edge cases & original bugs

1. Quiver quantity text (`0x00484DB0`) is overwritten before use (§3.5).
2. `%n` substitution with an empty first argument keeps the following SP:
   a magic item without a prefix gets a leading SP, without a suffix a
   trailing SP (the empty-text test reads the stack word below the buffer,
   a stack address in 1.14d, never SP). Reproduce.
3. F25 / F26 print the unnegated text with a sign from −v; F21 equals F20
   (with `%`).
4. F22 with an out-of-range montype uses montype row 0.
5. An identified item of quality 0 or > 9 reaches the fatal assert of
   `0x0048C060` (no name branch).
6. Set tip: an ethereal set item that is not socketed has no `Ethereal`
   line; a set item never gets the kick, charm, quantity or socket-filler
   blocks.
7. The class line's colour in the main builder treats "no U" as a
   mismatch (red).
8. Runes with several lines per slot put the label and a trailing `,` on
   the top line (§6 r7).
9. A skill without a skilldesc row is named `an evil force` (5382).

## Test vectors

Synthetic (string texts from the 1.14d English tables):

| Input | Expected | Source |
|---|---|---|
| `Pfx("", 3)`; `Pfx("Defense: 3\n", 0)` | `""`; `"ÿc0Defense: 3\n"` | §Rules notation |
| stat 0, v 5, f 1 dv 1 | `+5 to Strength` | §7.2 |
| stat 0, v −3 | `-3 to Strength` | §7.2 |
| stat 39, v 30, f 4 dv 2 | `Fire Resist +30%` | §7.2 |
| stats 39, 41, 43, 45 all 15 | one line `All Resistances +15` at priority 36; with 43 = 10: four lines | §7.1 r3 |
| stats 0–3 all 5 | `+5 to all Attributes` | §7.1 r3 |
| stat 80, v 25, f 2 dv 1 | `25% better chance of getting magic item` | §7.2 |
| stat 252, v 1 / 100 / −2 | `Repairs 1 durability in 100 seconds` / `Repairs 1 durability per second` / `Repairs 25 durability per second` | §7.2 f 11 |
| stat 83, layer 3, v 2 | `+2 to Paladin Skill Levels`; layer 5: `+2 to Druid Skills` | §7.2 f 13 |
| stat 188, layer 8 (Sorceress tab 0), v 3 | `+3 to Fire Skills (Sorceress Only)` | §7.2 f 14 |
| stat 195, layer (36 << 6) + 3 (skill 36 `Fire Bolt`), v 10 | `10% Chance to cast level 3 Fire Bolt on attack` | §7.2 f 15 |
| stat 204, layer (36 << 6) + 5, v 10 × 256 + 7 | `Level 5 Fire Bolt (7/10 Charges)` | §7.2 f 24 |
| stat 214, v 12, P level 20 | `+30 Defense (Based on Character Level)` | §7.1 r2, f 6 |
| stat 116, v 25 | `-25% Target Defense` | §7.2 f 20 |
| stat 113, v 1, f 12 dv 2 | `Hit blinds target ` (trailing SP) | §7.2 f 12 |
| stats 48 = 10, 49 = 16 | `Adds 10-16 fire damage` | §8 |
| stats 18 = 50, 17 = 50 | `+50% Enhanced Damage` | §8 |
| stats 57 = 256, 58 = 512, 59 = 75, 326 = 0 | `Adds 75-150 poison damage over 3 seconds` | §8 |
| blunt weapon, identified, no mods | bottom line of B: `+50% Damage to Undead` | §6 r3 |
| reqstr 100, stat 91 = −20, ethereal, U str 60 | F = `ÿc1Required Strength: 70` | §3.3 |
| store: Cost 500, magic item | top line `Cost: 500` drawn in colour 3 (the name's) | §11 r2 |

Capture cases (`text-0002` family, 1.14d, 800 × 600): a rare weapon with
requirements met and one failed; a set item hovered in the inventory with
0, 1, 2 and all pieces worn, and the same piece equipped; a store item the
player cannot use (buy, sell, repair modes); a rune and a gem; items with
`descfunc` 11, 13–18, 22–24, 27, 28; a magic item without prefix (leading
space position).

## Provenance

Static RE of 1.14d `Game.exe` (`re/exports/all.asm`, 2026-10-08):
`0x0048DD90` (entry, blocks, colours, quest and store tail), `0x00484B10`,
`0x00484DB0`, `0x00484E90`, `0x00484FF0`, `0x004850A0`, `0x00485170`,
`0x00485240`, `0x00485410`, `0x00485BE0`, `0x00485D40`, `0x00485EE0`,
`0x00486100`, `0x004861D0`, `0x00486370`, `0x004865D0`, `0x00486670`,
`0x00486770`, `0x0048BE80`, `0x0048BE00`, `0x004834A0`, `0x004834E0`,
`0x0048C060`, `0x0048CEA0`, `0x0048CFF0`, `0x0048D1D0`, `0x0048DB80`,
`0x004B2AD0`, `0x004E48B0`, `0x004E49C0`, `0x004E4C50`, `0x004E4CE0`,
`0x004E4D80` (jump table `0x004E59A0`), `0x004E5A20` (tables `0x004E5E28`,
`0x004E5E64`), `0x004E6410`, `0x004E60A0`, `0x004E6560`, `0x004E6680`,
`0x004E6720`, `0x004E67D0`, `0x004E6850`, `0x004E6CE0`, `0x0062A370`,
`0x0062A710`, `0x0062EAF0` (out-parameter order). Jump tables read from
the image: quality colours `0x0048EDD8`, gamble class `0x0048ED98` /
`0x0048EDB8`, thrown-potion colours `0x00485BC4`, spell modes `0x00486568`
/ `0x00486578` / `0x00486584`. String ids resolved against the 1.14d ENG
tables (Patch_D2 `patchstring.tbl` 1,179 entries). Data counts from patch
`itemstatcost.txt`, `setitems.txt` (add func 2: 82 rows, 1: 1, empty: 45),
`misc.txt` (one `elix` row, no `Transmogrify`). Elixir text (§6 r1,
2026-10-08): `0x004E60DC` type test, `0x004E5E90`, table `0x0072D6C0` /
count `0x0072D720` and format `%i` `0x006D6454` read from the image,
string ids 3498–3503, 3995, 3998, 4002 from `d2data.mpq` ENG
`string.tbl`. D2MOO has no D2Client item text; nothing here comes from
it.

## Open questions

1. `0x004E5E90` (primary type 11 `elix` property text) is not specified;
   only the unused `elixir` misc row reaches it.
   *Answered* (static, 2026-10-08): §6 r1. Strings read from the 1.14d
   ENG `string.tbl` (`d2data.mpq`).
2. Whether the leading / trailing SP of §Edge cases 2 shifts the centred
   line by half a space in the capture (expected yes).
3. Pop-up placement and box drawing of `0x00502280` belong to the pop-up
   queue owner (`ui/panels.md` §5 step 10); this spec gives only its
   arguments.
