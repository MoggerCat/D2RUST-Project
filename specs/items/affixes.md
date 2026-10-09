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
| Summary | 41–51 |
| Inputs | 52–59 |
| Outputs / state changes | 60–64 |
| Rules | 65–66 |
|   1. Affix ids and slots | 67–89 |
|   2. Affix level (alvl) | 90–95 |
|   3. Magic affix roller (`0x005C1560`, format ≥ 1) | 96–133 |
|   4. Fit tests | 134–159 |
|   5. Rare name pick (`0x005C1AB0`, format ≥ 1) | 160–167 |
|   6. Magic item (`0x005565E0`) | 168–194 |
|   7. Rare item (`0x005C21A0` → `0x005C1BF0`, format ≥ 1) | 195–214 |
|   8. Crafted item (`0x005C21D0`) | 215–236 |
|   9. Tempered item (dispatch case 9) | 237–258 |
|   10. Charm (`0x00556A60`, from the normal routine) | 259–272 |
|   11. Automagic (finishing step, `0x00557450`) | 273–279 |
|   12. Format-0 routines (legacy items) | 280–359 |
| Constants & data dependencies | 360–376 |
| Randomness | 377–392 |
| Edge cases & original bugs | 393–431 |
| Test vectors | 432–450 |
| Provenance | 451–473 |
| Open questions | 474–512 |
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
4. Exception: on an item of type `scro` (22) or `book` (18) suffix slot 0
   is **not** a magic affix id. The normal-quality routine
   (`items/generation.md` §6.1 rules 4–5, `0x00556E80`) stores there the
   index of the `books` row whose `ScrollSpellCode` (`scro`, EDX = 1 at
   `0x00556EEE`) or `BookSpellCode` (`book`, EDX = 0 at `0x00556F0F`)
   equals the item code (`0x005C2540`), or the `books` row count when
   none does, through the suffix-slot setter `0x00627FB0`. With 1.14d
   `books.txt` (3 rows): `tsc`, `tbk` → 0; `isc`, `ibk` → 1; `0sc` (type
   `scro`, no row) → 3. The bit stream sends it as the 5-bit field of
   `items/bitstream.md` §4.3 rule 7. A check that reads suffix slots as
   affix ids (fit, group, part) must skip slot 0 of these types.

### 2. Affix level (alvl)

i := max(item level, qlvl) (qlvl = items `level`); m := items `magic lvl`.
m ≠ 0 → a := i + m. Else h := qlvl / 2; a := i − h if i < 99 − h, else
2 × i − 99. Clamp: a ≤ 1 → 1; a ≥ 99 → 99. (Computed inside §3.)

### 3. Magic affix roller (`0x005C1560`, format ≥ 1)

Arguments: item, require-spawnable, force, assign, prefix, preferred p,
automagic group g. (Format 0: `0x005C12F0`, §12.1.)

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
candidate r. No weights. (Format 0: `0x005C19A0`, same logic: its
instructions are the same as `0x005C1AB0`'s one for one, §12.2.)

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

Draw order (read 2026-10-09). Both calls go through the wrapper
`0x005C18E0` with assign = 1. The roller (§3) assigns the chosen row's
properties itself, as its last act (`0x0065FEC0` mode 0 after
`0x005C18A3`), so the item seed sees: prefix coin, prefix pick, the
prefix's property draws (`mod1` → `mod3` in record order, §3 of
`items/properties.md`), then the suffix's coin, pick and property draws,
then class skill mods (`0x005C1260`). It is **not** the rare item's
"choose all, then roll P0 S0 …" order (§7). The charge difference of
`items-vendor-akara-buy` is the store recharge (`items/properties.md`
§5 rule 9), not this order.

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
      Stores go into the item at once (prefix slot `0x00627EF0`, suffix
      slot `0x00627FB0`), so the next §3 call's group test (§4.2) sees
      them. The taken test reads slots 0–2 of the kind, skipping empty
      ones (it does not stop at the first empty slot, unlike §4.2), in
      slot order: slot record none → next slot; a = slot id → taken;
      else group(a) = group(slot) → taken. group(a) is read without a
      test that a has a record (edge case 3).
4. Clear identified; properties interleaved as §7 step 5; class skill
   mods; return 1 (also with no affix).

### 9. Tempered item (dispatch case 9)

rp := §5(prefix), rs := §5(suffix); both ≠ 0 → rare prefix/suffix := rp,
rs, success (no affixes, no properties); else downgrade to normal.
(Tempered quality is only reachable by a request quality of 9.)

Exact (2026-10-07, `disasm.py fn 0x005C1BC0` and the dispatch at
`0x00557801`–`0x0055786C`): the dispatch first clears rare prefix and
rare suffix (`0x00628010`(item, 0), `0x00628070`(item, 0)) and saves
S := the dword at item data +0x04 (`0x00627D90`, `0x00650E50`) for the
downgrade D(2) (`items/quality.md` §4 table, §5),
then calls `0x005C1BC0`(ECX item, EDX 1) and `0x005C1BC0`(ECX item,
EDX 0). `0x005C1BC0`(item, prefix) is the **rare name pick by format**:
format (`0x0062A670`) ≥ 1 → §5 (`0x005C1AB0`, ECX = prefix, item);
format 0 → §12.2 (`0x005C19A0`); it returns that routine's rare id (0
= no candidate) and draws what that routine draws (one item-seed step
when there is a candidate, none otherwise). It is not a separate
"tempered" roll: `world/cube.md` §7.3's two calls (quality byte 9) are
this same pick, prefix then suffix. Both ids ≠ 0 → rare prefix := rp
(`0x00628010`), rare suffix := rs (`0x00628070`); either 0 → the
normal downgrade (`0x005572A0`) with the saved seed.

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

### 12. Format-0 routines (legacy items)

Format 0 (item data +0x30 = 0) is reached only by items of a version-0x47
save (`items/generation.md` Open question 1); every dispatch below tests
the format with `0x0062A670` (u16 < 1).

#### 12.1 Magic affix roller (`0x005C12F0`)

Arguments (stdcall, 6): item, require-spawnable, force, assign, prefix,
preferred p; no group (the wrappers `0x005C18E0` / `0x005C1940` drop
it, so a format-0 call with a group rolls a plain prefix). Returns a u16.

1. Part: prefix ≠ 0 → prefixes (magic array header `0x00633ED0` +0x0C
   to +0x10), else suffixes (+0x08 to +0x0C); f := the part's first
   combined index. No part → 0.
2. **Coin:** one item-seed step; lo′ even and not force → return 0.
3. a := item level (`0x006281E0`) + 2, then ≤ 1 → 1, ≥ 99 → 99 (no
   `level` / `magic lvl` of the items row, unlike §2).
4. For each row i of the part, in order, keep it when all hold: not
   require-spawnable or `spawnable` ≠ 0; `version` < 100 or format ≥
   100 (so with format 0: `version` < 100); `level` ≤ a; fits the item
   (§4.1). Kept rows are appended as (f + i, row) while fewer than 511
   are listed. No `maxlevel`, `rare`, `group`, `frequency`,
   `classspecific` or taken test, no weights, no early return for the
   preferred row.
5. None kept → 0. Else r := roll(n) (`roll_range(0, n)`, n = the
   count); the pick is candidate r.
6. Preferred p > 0: the pick becomes the candidate whose combined index
   is f + p − 1; if none is listed, the id becomes −1 and the row stays
   candidate r.
7. assign ≠ 0 → properties of the row, mode 0 (`0x0065FEC0(0, 0, item,
   row, 0, 0)`). Return id + 1.
   So a preferred row that is not a candidate returns 0 but, when
   assigning, still applies candidate r's properties.

#### 12.2 Rare name pick (`0x005C19A0`)

§5 exactly (rare array header `0x00634250`, rows of 0x48 bytes, fit
§4.3, ≤ 511 candidates, `roll_range(0, n)`, rare id = combined index +
1).

#### 12.3 Rare item (`0x005C21A0` → `0x005C1E80`)

`0x005C21A0` first requires itemtype `rare` ≠ 0 (`0x0062E990`), then
format ≥ 1 → §7, format 0 → this routine (ESI item, one stack argument
passed on to the class skill mods).

1. rp := §12.2(prefix), rs := §12.2(suffix) (each dispatched by
   format). Either 0 → return 0. Rare prefix := rp, rare suffix := rs.
2. Count n: primary type `jewl` (58) → `roll_range(3, 2)`; else
   `roll_range(4, 3)` (4–6; not the §7 table). n = 0 → return 0
   (unreachable).
3. P := 0, S := 0. n times (every pass counts):
   1. One item-seed step, always; suffix when P = 3, or when S ≠ 3 and
      lo′ is odd; else prefix.
   2. Up to 252 tries: a := the roller (format 0: §12.1(item, 1, 1, 0,
      kind, 0); a format ≥ 1 item would take §3(item, 1, 1, 0, kind, 0,
      0)): require-spawnable, forced, not assigning, preferred 0 (the
      request's affixes are not read). a = 0 → the pass ends with no
      store. Else the taken test of §8 step 3.2 (slots 0–2 of the kind,
      empty ones skipped, a equal or same `group`): not taken → slot P
      (S) := a, P (S) += 1, the pass ends; taken → next try. All 252
      taken → slot P (S) := 0, not advanced.
4. P = S = 0 → return 0. Else clear the identified flag (0x10),
   properties (mode 0) of prefix 0, suffix 0, prefix 1, suffix 1, prefix
   2, suffix 2 (empty slots skipped), class skill mods (`0x005C1260`),
   return 1.

Unlike §7: no affix preferences, count 4–6 (jewels 3–4), the kind step
is drawn on every pass, a failed roll ends the pass without closing the
kind, and the group / duplicate test is the §8 one (with up to 252
tries) on top of the roller's none.

#### 12.4 Crafted item, format 0

`0x005C21D0` takes §12.2 for the rare names (`0x005C21F6`,
`0x005C2214`) and §12.1 for the affixes (`0x005C2339` prefix,
`0x005C2419` suffix: (item, 1, 1, 0, kind, request prefix[P] or
suffix[S])), else as §8.

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
   Confirmed on 1.14d (handoff triage Q2): `0x00633EE0` returns none for
   ids ≤ 0 and > the count; at `0x005C2371` / `0x005C2451` the result is
   used at +0x5C (`0x005C237C` prefix, `0x005C2460` suffix) with no test.
   §3 is called with force 1, so its coin never returns 0: a = 0 means
   no row passed step 4. Reachable whenever the kind's remaining rows
   all share a group with a filled slot, which is common at low alvl on
   bases with few fitting rows; d2rs reproduces it as a fatal error
   (`Ruleset::Original`).
   Measured on the live tables (`local-buddy-tri-sim` 2026-10-07, the
   creation sweep over every item, qualities 0–9, ilvl 1 / 30 / 60 / 99,
   difficulty 0 / 2, both game kinds): 10 crafted requests end here, all
   at ilvl 1 (4 classic d 0, 1 classic d 2, 3 expansion d 0, 2 expansion
   d 2). So "common" holds only for item level 1 requests; the 560
   other sweep failures (`affix 1 does not fit` on `ibk`) are not this
   edge case but rule 4 of §1 (a books index in suffix slot 0).
4. Charm with no fitting affix exits the game process.
5. The charm's prefix roll passes the suffix preference (harmless: it is ≤ 0).
6. The coin step is drawn even when force is set.
7. Crafted: the taken test of §8 step 3.2 is not redundant with §4.2.
   A stored 0 (edge case 3, no slot of the kind filled) leaves slot P
   empty and advances P, so later picks of that kind land after a hole.
   §4.2 stops at the hole and no longer sees them; only the taken test
   (which skips empty slots) keeps a second affix of their group out.
   (A 0 store means no row of that kind passed §3 step 4; later calls
   of that kind exclude at least as much, so the hole is followed by
   more 0 stores unless the request names a different preferred affix
   for the next slot, §3 step 4.3.)

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
| §8, synthetic table: one suffix row (group 5) fits; n = 2, both picks suffix | pick 1 stores it in suffix slot 0; pick 2: §3 excludes group 5 → a = 0; slot 0 filled → fatal (edge case 3) | synthetic |
| §8, same table, both picks prefix (no prefix row fits) | a = 0 twice, nothing filled → prefix slots 0, 1 := 0, P = 2; success, no prefix | synthetic |

## Provenance

- 1.14d `Game.exe`: roller `0x005C1560` (coin at `0x005C1602`, pick at
  `0x005C1839`), wrappers `0x005C18E0`, `0x005C1940`, group check
  `0x005C1500`, fit tests `0x0065E620`, `0x0065E710`, rare names
  `0x005C1AB0`, magic `0x005565E0`, rare `0x005C21A0`/`0x005C1BF0`
  (count table `0x006E3014`), crafted `0x005C21D0` (disassembled again
  for the triage questions: slot setters `0x00627EF0`, `0x00627FB0`,
  slot getters `0x00627EC0`, `0x00627F80`, record lookup `0x00633EE0`),
  tempered
  `0x005C1BC0` (dispatch `0x0055782E`), charm `0x00556A60`, automagic
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
2. Answered (2026-10-07, `disasm.py fn` on `0x005C12F0`, `0x005C19A0`,
   `0x005C1AB0`, `0x005C1E80`, `0x005C18E0`, `0x005C1940`,
   `0x005C1BC0`, `0x005C21A0` and the format-0 calls of `0x005C21D0`):
   §12. The format-0 magic roller has no weights and none of the
   `maxlevel` / `rare` / `group` / `frequency` / class filters, alvl =
   ilvl + 2, and a preferred row that is not a candidate returns 0;
   the rare name pick is §5 instruction for instruction; the rare
   routine draws 4–6 affixes (jewels 3–4) with the crafted taken test.
   Reached only by items of version-0x47 saves.
3. Answered (handoff `triage-game-findings` Q1, Q2): §8 step 3.2 (picks
   written into the item at once; taken test skips empty slots), edge
   cases 3 (the crash is real: no test before the +0x5C read) and 7.
   The d2rs creation sweep's ~570 crafted requests ending in edge case
   3 are expected 1.14d behavior for those bases; a live craft at a low
   resulting ilvl on a base with one fitting group would show it.
   Corrected 2026-10-07 (local sweep `local-buddy-tri-sim`): only 10 of
   those requests are edge case 3 (all ilvl 1); see the edge case.
4. Answered (local sweep `local-buddy-tri-sim`, "affix 1 does not fit",
   item 519 `ibk`): §1 rule 4. Suffix slot 0 of `scro` / `book` items is
   a `books` row index, set by `0x00556E80` through `0x005C2540` and
   `0x00627FB0`; the item is correct 1.14d output, the sweep's affix
   check is what must skip it. Expected from the rule: every created
   `ibk`, `isc` (value 1) and `0sc` (value 3, expansion only) fails that
   check, 160 + 160 + 80 = 400 requests if every request quality ends in
   the normal routine for these bases. The remaining sweep failures, if
   any after the check skips the slot, need the next local run to print
   all failures grouped by (item, error).
5. Answered (handoff `impl-items` OQ-A1): the roller's force argument
   is the value 1 in all three routines. Pushes at the calls of
   `0x005C1560` (item, spawnable, force, assign, prefix, p, g): rare
   `0x005C1CFC` / `0x005C1D79` (1, 1, 0, 0 or 1, p, 0); crafted
   `0x005C2322` / `0x005C2402` (1, 1, 0, 1 or 0, p, 0); automagic
   `0x005579E1` through `0x005C1940` (EDX spawnable 0; force 1, assign 0,
   prefix 1, p 0, g = items +0xF8 `auto prefix`). So §7, §8 and §11 call
   §3 with force = 1 (the coin of edge case 6 is drawn and ignored).
