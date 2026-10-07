# Spec: Monsters — Umod init bodies (elemental enchants, teleport)

- **Status:** draft: the five init functions of umods 9, 17, 18, 23, 25
  and the one of umod 26 read from the 1.14d `Game.exe` disassembly
  (addresses per section); live values from 1.14d `monlvl.txt` and
  `monumod.txt`. No recording covers them yet (Open question 1).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::monsters::init` (`umods.rs`)
- **Related specs:** `monsters/init.md` (§18 caller and order, §19 umod
  catalogue, §19.3 resistance tail `0x005A1370`, §8.1 L-flag and monlvl
  columns); `monsters/umods.tsv` (one row per umod);
  `monsters/umod-callbacks.md` (the same umods' callbacks; §3.4 is the
  missile-side elemental fill); `sim/stat-lists.md` §5 (stat add / set);
  `client/msg-skills.md` §2 (skill assign); `monsters/ai.md` §2.4, §3.1
  (AI flag 0x20 and the teleport precheck).

## Summary

`init.md` §18 runs a boss's umod init functions (table `0x0073C008`) on
the boss with unique = 1 and on each of its minions with unique = 0.
This spec gives the bodies of the elemental enchants (fire 9, lightning
17, cold 18, poison 23, mana burn 25), which share one body that differs
only in stat ids, an optional duration stat and a scale, and of teleport
(26), which gives the monster the MonTeleport skill and the AI's
teleport flag.

## Inputs

| Name | Type | Source |
|---|---|---|
| unit | monster (type 1) | ECX |
| umod | umod id | EDX (passed on to the resistance tail) |
| unique | 1 boss, 0 minion | stack (`ret 4`) |
| game | unit +0x80 (`0x00554010`, fatal on a null unit): difficulty d (+0x6D, u8), L-flag (+0x6A ≠ 0 or +0x74 ≠ 0) | `init.md` §8.1 |
| tables | monlvl (row size 0x78, rows at data tables +0xB70, count +0xB74), monumod `constants` (row size 0x20, +0x1C; rows at +0xC50, count +0xC54), skills row 184 | `data/fields.tsv` |

## Outputs / state changes

Elemental umods: stat adds on the unit's own stat list (unit +0x5C,
layer 0) for a min damage, a max damage and (cold, poison) a length
stat; then the resistance tail. Teleport: one skill entry, its mode, AI
control flag 0x20. No function returns a value; none draws RNG.

## Rules

### 1. Conventions

1. K[i] = monumod row i `constants` read through `0x005A00F0`: i < 0 or
   i ≥ monumod row count → 0. Live 1.14d (43 rows): K[16…21] = 0, 33,
   33, 0, 50, 50; K[28…33] = 66, 66, 66, 100, 100, 100.
2. "Level" = the unit's `level(12)` total (`0x00625480(unit, 12, 0)`).
3. "Add s v" = `0x006272B0(unit, s, v, 0)` = stat add on unit +0x5C
   (`stat-lists.md` §5 rule 3): v = 0 changes nothing (no entry is
   created).
4. Divisions are signed 32-bit and truncate toward zero (the multiply
   by 0x51EB851F, shift 5, add sign bit = / 100).

### 2. Elemental body (`0x005A1990`, `0x005A1B00`, `0x005A1C70`, `0x005A1E00`, `0x005A1F90`)

Arguments (unit, umod, unique) as in Inputs. Per-umod values in §3.

1. unit null, or unit type ≠ 1 → return (nothing, no resistance tail).
2. game null → return.
3. d' = 0 when d = 0; 2 when d ≥ 2; else d (= 1). o = 1 when game +0x6A
   ≠ 0 or game +0x74 ≠ 0, else 0.
4. lv = level. Row r: when max(lv, 1) ≥ rows − 1 → r = rows − 1, and
   r < 0 → return; else r = 1 when lv ≤ 1, else r = lv. (Live rows =
   111, so r = clamp(lv, 1, 110).)
5. r ≥ rows → return. The monlvl row pointer (table base + r × 0x78) is
   null (only with a null table and r = 0) → return. A return here
   skips the resistance tail.
6. v = the row's dword at index 18 + 3o + d' (`DM` for o = 0, `L-DM`
   for o = 1, column of difficulty d'; `init.md` §8.1).
7. unique ≠ 0: kmin = K[d' + 28], kmax = K[d' + 31]; unique = 0: kmin =
   K[d' + 16], kmax = K[d' + 19].
8. min = kmin × v / 100; max = kmax × v / 100 (rule 1.4). Scale
   (§3): min := min × 256, max := max × 256 (shift left 8, after the
   division).
9. Add s_min min; then add s_max max.
10. Length stat (§3), if any: add s_len f(r), with r of step 4 (not the
    raw level).
11. Resistance tail `0x005A1370(unit, umod, unique)` (`init.md` §19.3;
    it does nothing when unique = 0).

### 3. Per-umod values

| Umod | Function | s_min / s_max | Scale | s_len, f(r) | Tail (§19.3) |
|---|---|---|---|---|---|
| 9 fire | `0x005A1990` | 48 firemindam / 49 firemaxdam | 1 | none | fireresist(39) +75 |
| 17 lightning | `0x005A1B00` | 50 lightmindam / 51 lightmaxdam | 1 | none | lightresist(41) +75 |
| 18 cold | `0x005A1C70` | 54 coldmindam / 55 coldmaxdam | 1 | 56 coldlength, 5 × r + 100 | coldresist(43) +75 |
| 23 poisonhit | `0x005A1E00` | 57 poisonmindam / 58 poisonmaxdam | 1 | 59 poisonlength, 2 × (5 × r + 150) | poisonresist(45) +75 |
| 25 manahit | `0x005A1F90` | 62 manadrainmindam / 63 manadrainmaxdam | × 256 | none | magicresist(37) +20 |

### 4. Teleport (umod 26, `0x005A1600`)

1. unit null, unit type ≠ 1, or unique = 0 → return.
2. Assign skill 184 (MonTeleport) at level 1 with remove flag 1
   (`0x0056DEB0(unit, 184, 1, 1)` → `0x00647280`, `client/msg-skills.md`
   §2 rule 2: a new entry gets mode `monanim`, base 1, owner −1; an
   existing native entry gets base := 1), then the passive refresh
   `0x00646F20(unit)`; a player would also get `0x00575900` (not
   reached: type 1). Skill index ≥ skills count → nothing.
3. The unit's native entry of skill 184 (`0x006439F0(unit, 184)`):
   its mode (+0x08) := 4 (`0x00644340`; no entry → nothing).
4. AI control (monster data +0x28) flags (+0x08, u16) |= 0x20
   (`0x005DD250(unit, 1)`; `ai.md` §3.1 "may teleport", read by the
   teleport precheck `ai.md` §2.4). A unit without monster data or AI
   control writes near address 0 (crash; unreachable for a created
   monster).

## Constants & data dependencies

| Item | Value / column |
|---|---|
| monlvl | `DM`, `L-DM` (and their `(N)`, `(H)`) = row dwords 18 … 23 |
| monumod | `constants` of rows 16 … 21 (minions) and 28 … 33 (bosses) |
| stats | 37, 39, 41, 43, 45 (tail); 48 … 51, 54 … 59, 62, 63 |
| skills | 184 MonTeleport; skill mode 4 |
| constants | coldlength 5 × r + 100; poisonlength 2 × (5 × r + 150); mana scale 256; AI flag 0x20 |

## Randomness

None. Neither body nor the resistance tail draws; the skill assign of
§4 draws nothing.

## Edge cases & original bugs

1. Minions (unique = 0) on Normal get kmin = kmax = 0 (live K[16],
   K[19]): no damage stat is added, but cold and poison minions still
   get the length stat.
2. The length stats use the clamped monlvl row, not the level: a level
   0 monster gets 105 coldlength, a level ≥ 110 one 650 (live).
3. A level outside the monlvl table is clamped, never rejected; only an
   empty table (rows ≤ 0) returns early, and then the resistance tail is
   skipped too.
4. Mana burn truncates before the × 256, so its values are whole
   multiples of 256.
5. Teleport ignores minions; other elemental bodies run on minions
   (damage, length) but their tail does not.

## Test vectors

### Synthetic (CI-safe)

Inputs: live K (§1 rule 1); v given directly.

| Input | Expected | Source |
|---|---|---|
| umod 17, unique 1, d 0, v 4 | lightmindam +2, lightmaxdam +4 | §2 r7–r9 |
| umod 25, unique 1, d 0, v 19 | manadrainmindam +3072 (12 × 256), manadrainmaxdam +4864 | §2 r8 |
| umod 18, unique 0, d 0, v 7, r 10 | no damage stats; coldlength +150 | edge 1 |
| umod 23, unique 0, d 1, v 25, r 30 | poisonmindam +8, poisonmaxdam +12, poisonlength +600 | §2, §3 |
| level 0 / 1 / 30 / 200, rows 111 | r = 1 / 1 / 30 / 110 | §2 r4 |
| umod 26, unique 0 | nothing | §4 r1 |
| umod 26, unique 1, no entry of 184 | entry 184 base 1, mode 4; AI flags \|= 0x20 | §4 |

### Real 1.14d values (live tables; `#[ignore]`, `D2_GAME_DIR`)

L-flag 1 (expansion), values from `monlvl.txt` `L-DM`:

| Monster | Expected | Source |
|---|---|---|
| lightning-enchanted unique, Normal, level 4 (L-DM 4) | lightmindam +2, lightmaxdam +4, lightresist +75 (if < 2 immunities) | §2, §3 |
| cold-enchanted unique, Hell, level 30 (L-DM(H) 35) | coldmindam +23, coldmaxdam +35, coldlength +250 | §2, §3 |
| mana-burn unique, Normal, level 30 (L-DM 19) | manadrainmindam +3072, manadrainmaxdam +4864, magicresist +20 | §2, §3 |
| cold-enchanted unique, Normal, level 150 (row 110, L-DM 130) | coldmindam +85, coldmaxdam +130, coldlength +650 | edge 2 |
| poison minion, Nightmare, level 30 (L-DM(N) 25) | poisonmindam +8, poisonmaxdam +12, poisonlength +600; no resist change | §2, edge 5 |

## Provenance

- 1.14d `Game.exe` (`disasm.py fn`): `0x005A1990`, `0x005A1B00`,
  `0x005A1C70`, `0x005A1E00`, `0x005A1F90` (the same instruction
  sequence; they differ in the pushed stat ids 0x30/0x31, 0x32/0x33,
  0x36/0x37/0x38, 0x39/0x3A/0x3B, 0x3E/0x3F, the length expressions
  `lea [r + 4r + 0x64]` and `lea [r + 4r + 0x96]; add eax, eax` on the
  stored clamped row, and the `shl 8` pair of `0x005A1F90`);
  `0x005A1370` (the umod 9 / 17 / 18 / 23 / 25 tail cases at
  `0x005A14D1` … `0x005A1560`); `0x005A00F0` (constants getter);
  `0x006272B0` → `0x00627030`; `0x00554010`; `0x005A1600`,
  `0x0056DEB0`, `0x006439F0`, `0x00644340`, `0x005DD250`.
- Live `game/extracted/patch_d2/data/global/excel/monlvl.txt` (111
  level rows; e.g. level 4 DM 4/5/7, level 30 19/25/35, level 110
  130 for every column) and `monumod.txt` (`constants` of rows 0 … 33;
  rows 34 … 42 empty); `skills.txt` row 184 `MonTeleport`.
- D2MOO (1.10f) `MonsterUnique.cpp` was the map for the init table;
  every step above was re-read on 1.14d; no difference found (D2MOO
  also uses the clamped row for the length stats).

## Open questions

1. ~~No recording covers these bodies: record the 0xAC assign and stat
   messages of a lightning, cold or mana-burn unique (with minions) to
   confirm the values above.~~ → PC 2 recording list.
