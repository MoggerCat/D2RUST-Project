# Spec: Items — Magic, rare, crafted and automagic affixes

- **Status:** draft: every rule read from the 1.14d `Game.exe` code
  (addresses per rule, register arguments read from the disassembly); no
  recording of item creation yet; test vectors are synthetic.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::items::affixes`
- **Related specs:** `items/generation.md` (§1 conventions, §6.2 class
  skill mods, Inputs: the request's preferred affixes); `items/quality.md`
  (§4 which routine runs, §5 downgrades on failure);
  `items/properties.md` (§2 mode 0: how an affix's mods become stats);
  `data/loading.md` §9 (combined affix arrays); `sim/rng.md` §3.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 40–50 |
| Inputs | 51–58 |
| Outputs / state changes | 59–63 |
| Rules | 64–65 |
|   1. Affix ids and slots | 66–77 |
|   2. Affix level (alvl) | 78–83 |
|   3. Magic affix roller (`0x005C1560`, format ≥ 1) | 84–121 |
|   4. Fit tests | 122–147 |
|   5. Rare name pick (`0x005C1AB0`, format ≥ 1) | 148–159 |
|   6. Magic item (`0x005565E0`) | 160–175 |
|   7. Rare item (`0x005C21A0` → `0x005C1BF0`, format ≥ 1) | 176–195 |
|   8. Crafted item (`0x005C21D0`) | 196–210 |
|   9. Tempered item (dispatch case 9) | 211–218 |
|   10. Charm (`0x00556A60`, from the normal routine) | 219–232 |
|   11. Automagic (finishing step, `0x00557450`) | 233–239 |
| Constants & data dependencies | 240–256 |
| Randomness | 257–272 |
| Edge cases & original bugs | 273–287 |
| Test vectors | 288–304 |
| Provenance | 305–324 |
| Open questions | 325–330 |
<!-- /index -->

## Summary

Magic items get one prefix and/or one suffix; rare and crafted items get
a rare name (rare prefix + rare suffix) and up to six magic affixes;
expansion items with an `auto prefix` get one automagic affix. All
choices draw from the item seed. One roller picks a magic affix: a coin
flip (unless forced), then a frequency-weighted pick among the rows that
fit the item, its affix level and its other affixes. This spec owns the
affix slots, the affix level, the roller and its filters, rare names,
and the magic, rare, crafted, tempered, charm and automagic routines.

## Inputs

| Name | Source |
|---|---|
| item: record, type, quality, format, item level, current affix slots, item seed | `items/generation.md` |
| request prefix[0..2] (+0x68), suffix[0..2] (+0x74) | `items/generation.md` Inputs |
| magic affix array (magicsuffix, magicprefix, automagic), rare affix array (raresuffix, rareprefix), properties | tables |

## Outputs / state changes

Affix slots, the identified flag (cleared on success), property stats
(through `items/properties.md` mode 0), class skill mods, item-seed draws.

## Rules

### 1. Affix ids and slots

1. A magic affix id is its combined index + 1 in the magic affix array
   (suffixes 0 … 746, prefixes 747 … 1,415, automagic 1,416 … 1,451 in
   1.14d, `data/loading.md` §9); 0 = none. Rare affix ids likewise in the
   rare array (suffixes first).
2. The item holds prefix slots 0–2, suffix slots 0–2 (magic ids), a rare
   prefix and a rare suffix (rare ids), and an auto affix (magic id).
3. "Part" = the prefix part, the suffix part or the automagic part of the
   magic array; "index in part" = combined index − the part's first index.
   The request's preferred affix p > 0 means index in part p − 1.

### 2. Affix level (alvl)

i := max(item level, qlvl) (qlvl = items `level`); m := items `magic lvl`.
m ≠ 0 → a := i + m. Else h := qlvl / 2; a := i − h if i < 99 − h, else
2 × i − 99. Clamp: a ≤ 1 → 1; a ≥ 99 → 99. (Computed inside §3.)

### 3. Magic affix roller (`0x005C1560`, format ≥ 1)

Arguments: item, require-spawnable, force, assign, prefix, preferred p,
automagic group g. (Format 0: `0x005C12F0`, not specified.)

1. Part: g ≠ 0 → automagic; else prefix → prefixes; else suffixes.
2. **Coin:** one step of the item seed. lo′ even and not force → return 0.
   (The step happens even when forced.)
3. a := alvl (§2). Weighted mode w := (items `magic lvl` ≠ 0).
4. For each row i of the part, in order, skip it unless all hold:
   1. not require-spawnable, or `spawnable` ≠ 0;
   2. `version` < 100 or format ≥ 100;
   3. preferred (p > 0 and p − 1 = i), or (`level` ≤ a and (`maxlevel` = 0
      or `maxlevel` ≥ a));
   4. `rare` ≠ 0 or the item's quality is not 6, 8 or 9;
   5. the row fits the item (§4);
   6. g = 0 or `group` = g;
   7. `frequency` ≠ 0;
   8. `classspecific` = 0xFF, or the item's class (itemtype `class`, ≥ 7
      as 7) is 7, or equals `classspecific`;
   9. no prefix and no suffix on the item has the same `group` (§4.2).
   A preferred row passing these is returned at once (assign if asked).
   Otherwise append (id, row) to the candidate list (at most 511 entries;
   later rows are not appended but **their weight is still added**) and
   add its weight: `frequency` × `level` if w, else `frequency`.
5. No candidate → return 0.
6. r := roll(total + 1) (`roll_range(0, total + 1)`); walk the candidates
   subtracting each weight; the first that makes r negative wins; if none
   does, the last candidate wins.
7. If assign: properties of the row, mode 0 (`items/properties.md`).
   Return its id.

#### 3.1 Wrappers

`0x005C18E0` and `0x005C1940` call §3 for format ≥ 1 (format 0: the old
roller). Argument order: item, require-spawnable, force, assign, prefix,
preferred, group (the first two in ECX/EDX).

### 4. Fit tests

#### 4.1 Magic affix fits the item (`0x0065E620`)

1. Format < 100: stackable or throwable items fit nothing.
2. Unless the item is socketable with max sockets ≠ 0
   (`items/generation.md` §7.2): a row whose `mod1code` ≥ 0 and is out of
   range of properties, or whose property's `stat1` is 194
   (`item_numsockets`), does not fit.
3. Any of `etype1`–`etype5` (stop at the first < 1) that the item is →
   no fit.
4. Any of `itype1`–`itype7` (stop at the first < 1) that the item is →
   fit; else no fit.

#### 4.2 Group taken (`0x005C1500`)

Scan prefix slots 0–2 (stop at the first empty slot), then suffix slots
0–2 (stop at the first empty slot, answering no): any affix whose
`group` equals the candidate's → taken.

#### 4.3 Rare affix fits (`0x0065E710`)

Format < 100: stackable or throwable → no. `version` ≥ 100 and format <
100 → no. `etype1`–`etype4` (stop at < 1) match → no. `itype1`–`itype7`
(stop at < 1) match → yes; else no.

### 5. Rare name pick (`0x005C1AB0`, format ≥ 1)

Arguments: prefix (ECX), item. Part: rare prefixes or rare suffixes.
Candidates: rows of the part that fit (§4.3), in order (≤ 511). None →
0. Else r := roll(count) (`roll_range(0, count)`); return the rare id of
candidate r. No weights. (Format 0: `0x005C19A0`, same logic.)

Entry by format (`0x005C1BC0`, ECX item, EDX 1 = prefix / 0 = suffix):
item format (`0x0062A670`) ≥ 1 → `0x005C1AB0`, else `0x005C19A0`;
returns the rare id (0 = none). §9 and the cube (`world/cube.md` §7.3)
call this entry; §7 and §8 call the two pickers directly.

### 6. Magic item (`0x005565E0`)

p := request prefix[0], s := request suffix[0], forced := false.

1. p < 0: no prefix roll; forced := true. Else prefix := §3(spawnable,
   force = p > 0, assign, prefix, p); prefix slot 0 := it. It is 0 →
   forced := true; it is ≠ 0 and s < 0 → skip step 2.
2. s > 0 → forced := true. suffix := §3(spawnable, forced, assign, suffix,
   s); suffix slot 0 := it.
3. Both 0 → return 0 (downgrade). Else clear identified, class skill mods,
   return 1.

So a magic item without preferences has a prefix with 1/2 chance and,
if it has one, a suffix with 1/2 chance; with no prefix the suffix is
forced.

### 7. Rare item (`0x005C21A0` → `0x005C1BF0`, format ≥ 1)

1. itemtype `rare` = 0 → return 0.
2. rp := §5(prefix), rs := §5(suffix) (two draws). Either 0 → return 0.
   Rare prefix := rp, rare suffix := rs.
3. Count n: primary type `jewl` → 3 + roll(2) (`roll_range(3, 2)`); else
   one step, n := {3, 4, 4, 5, 5, 5, 6, 6}[lo′ & 7] (`0x006E3014`).
4. P := 0 prefixes, S := 0 suffixes, pDone, sDone false. For k := 0 while
   k < n:
   1. Both done → stop.
   2. Choose suffix if pDone, or if not sDone and a step gives lo′ odd;
      else prefix. (No step when pDone or sDone.)
   3. Roll §3(spawnable, force, no assign, that kind, request prefix[P]
      or suffix[S], g 0). 0 → that kind is done and k is not advanced.
      Else store in slot P (S), P += 1 (S += 1); 3 → done.
   4. k += 1.
5. P = S = 0 → return 0. Clear identified; properties (mode 0) of prefix
   0, suffix 0, prefix 1, suffix 1, prefix 2, suffix 2 (interleaved, empty
   slots skipped); class skill mods; return 1.

### 8. Crafted item (`0x005C21D0`)

1. rp, rs as §7 step 2 (both required). Rare prefix/suffix := rp, rs.
2. Minimum m from request ilvl: > 70 → 4, > 50 → 3, > 30 → 2, else 1.
   One step: n := lo′ mod 5; n < m → n := m.
3. P := 0, S := 0. n times:
   1. One step; suffix if P = 3, or if S ≠ 3 and lo′ odd; else prefix.
   2. Up to 252 tries: a := §3(spawnable, force, no assign, kind, request
      prefix[P] or suffix[S], 0). a is taken when any filled slot of the
      same kind holds a or an affix of a's `group`; not taken → store in
      slot P (S), advance, end the tries. All 252 taken → slot P (S) := 0
      (not advanced).
4. Clear identified; properties interleaved as §7 step 5; class skill
   mods; return 1 (also with no affix).

### 9. Tempered item (dispatch case 9)

Rare prefix and suffix := 0 first. rp := §5 entry `0x005C1BC0`(item,
1) (`0x0055782E`), rs := `0x005C1BC0`(item, 0) (`0x0055783A`), in that
draw order; both ≠ 0 → rare prefix/suffix := rp, rs, success (no
affixes, no properties); else downgrade to normal.
(Tempered quality is only reachable by a request quality of 9.)

### 10. Charm (`0x00556A60`, from the normal routine)

p := request prefix[0], s := request suffix[0], forced := false.

1. p > 0: prefix := §3(spawnable, force, assign, prefix, p). Else if s ≤
   0: prefix := §3(spawnable, no force, assign, prefix, s) (the suffix
   preference is passed, ≤ 0, so no effect); 0 → forced := true. Prefix
   slot 0 := the result.
2. s > 0: suffix := §3(spawnable, force, assign, suffix, s). Else if p ≤
   0: suffix := §3(spawnable, forced, assign, suffix, p). Suffix slot 0 :=
   the result (when rolled).
3. Neither → fatal error 0x372 (the game exits). Else clear identified.
   No class skill mods.

### 11. Automagic (finishing step, `0x00557450`)

With g := items `auto prefix`: a := §3(not spawnable-only, force, no
assign, prefix part ignored, p 0, group g) over the automagic part. a ≠ 0
→ auto affix := a, properties of row a (mode 0). Qualities 1, 2, 3, 4, 6,
8, 9 only, format ≥ 100 (`items/quality.md` §4.5).

## Constants & data dependencies

| Constant | Value | Where |
|---|---|---|
| rare affix count table | 3, 4, 4, 5, 5, 5, 6, 6 | `0x006E3014`, §7 |
| jewel rare count | 3 + roll(2) | §7 |
| crafted minimums | ilvl > 30 / 50 / 70 → 2 / 3 / 4 | §8 |
| crafted tries | 252 | §8 |
| candidate cap | 511 | §3, §5 |
| alvl clamp | 1 … 99 | §2 |

Magic affix columns (144-byte rows, `data/fields.tsv` `magicprefix`):
`version`, `mod1code`–`mod3max`, `spawnable`, `level`, `group`,
`maxlevel`, `rare`, `classspecific`, `itype1`–`7`, `etype1`–`5`,
`frequency`. Rare affix columns: `version`, `itype1`–`7`, `etype1`–`4`.
Items: `magic lvl`, `level`, `auto prefix`. Itemtypes: `rare`, `class`.

## Randomness

All on the item seed:

| Routine | Draws in order |
|---|---|
| §3 | 1 step (coin); roll(total + 1) if candidates; the row's property draws if assigning |
| §6 | §3 (prefix, assigning), §3 (suffix, assigning) unless skipped; class skill mods |
| §7 | §5 ×2; count step (or roll(2) for jewels); per affix: kind step (when both kinds open) then §3; then properties in interleaved order; class skill mods |
| §8 | §5 ×2; count step; per affix: kind step, then §3 per try; properties; class skill mods |
| §11 | §3 with force |

Note that affixes of rare and crafted items are chosen first and their
property values rolled afterwards, in slot order P0 S0 P1 S1 P2 S2;
magic items roll the prefix's values before the suffix is chosen.

## Edge cases & original bugs

1. Rows past the 511-candidate cap still add weight; a roll landing in
   that weight picks the last listed candidate.
2. r := roll(total + 1) can equal total: the last candidate then wins
   (a one-unit bias toward it).
3. Crafted: when §3 returns 0 (no candidate) while a same-kind slot is
   filled, the original reads the group of record 0, which does not exist
   (`0x00633EE0` returns none for id 0): a read at address 0x5C, which
   crashes the game. With no slot filled, 0 is "not taken" and is stored
   (slot stays empty but the count advances).
4. Charm with no fitting affix exits the game process.
5. The charm's prefix roll passes the suffix preference (harmless: it is ≤ 0).
6. The coin step is drawn even when force is set.

## Test vectors

Synthetic, from the rules:

| Input | Expected | Source |
|---|---|---|
| alvl: ilvl 30, qlvl 20, magic lvl 0 | 20 | synthetic |
| alvl: 95, 85, 0 | 91 | synthetic |
| alvl: 1, 5, 0 | 3 | synthetic |
| alvl: 40, 1, 3 | 43 | synthetic |
| alvl: 98, 60, 0 | 97 | synthetic |
| alvl: 69, 60, 0 | 39 | synthetic |
| rare count, item seed `{5, 666}` | lo′ 367056499, & 7 = 3 → 5 affixes | synthetic |
| rare count, seed `{6, 666}` / `{7, 666}` / `{8, 666}` | 3 / 5 / 4 | synthetic |
| §3 weights: freq 3, 0, 5 (no magic lvl), total 8, r = roll(9) = 3 | row 1 skipped (freq 0); 3 − 3 = 0 not < 0, 0 − 5 < 0 → second candidate | synthetic |
| §3 weighted (magic lvl ≠ 0): levels 10, 20, freq 2, 1 | weights 20, 20; total 40; roll(41) | synthetic |

## Provenance

- 1.14d `Game.exe`: roller `0x005C1560` (coin at `0x005C1602`, pick at
  `0x005C1839`), wrappers `0x005C18E0`, `0x005C1940`, group check
  `0x005C1500`, fit tests `0x0065E620`, `0x0065E710`, rare names
  `0x005C1AB0`, magic `0x005565E0`, rare `0x005C21A0`/`0x005C1BF0`
  (count table `0x006E3014`), crafted `0x005C21D0`, tempered
  dispatch case 9 at `0x00557801` (rare-name entry `0x005C1BC0`, §5), charm `0x00556A60`, automagic
  `0x005579C6`; affix table root `0x0096CA7C`, rare `0x0096CAA0`.
- D2MOO 1.10f (`ItemsMagic.cpp`: `ITEMS_RollMagicAffixesNew`,
  `D2GAME_RollRareItem_6FC53360`, `sub_6FC53CD0`, `D2GAME_RollRareAffix`;
  `Items.cpp`: `sub_6FC4D5E0`, `ITEMS_AssignCharmAffixes`;
  `ItemMods.cpp`: `ITEMMODS_CanItemHaveMagicAffix`,
  `ITEMMODS_CanItemHaveRareAffix`) was the map. Differences confirmed on
  1.14d: the alvl is computed inside the roller as in D2MOO's helper; the
  1.14d crafted routine passes the request's preferred affixes and checks
  all three slots of the kind (D2MOO's rewrite passes none); the D2MOO
  "fits" test is garbled in its socket clause, 1.14d's is §4.1 step 2;
  the coin is `lo′ & 1` (same as `roll(2)`).

## Open questions

1. No recording confirms the routines (request R1 in the session report).
2. Format-0 rollers (`0x005C12F0`, `0x005C19A0`, `0x005C1E80`) are not
   specified.
