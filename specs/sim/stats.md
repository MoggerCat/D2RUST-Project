# Spec: Simulation — Stats (identity, values, op formulas)

- **Status:** draft (no `d2-sim` code yet); every rule read from the
  1.14d `Game.exe` code. Table facts verified: `check_stats.py --files
  game` (359 stats, 84 ops, 42 op targets) passes on
  `patch_d2/itemstatcost.bin`. The formulas are unverified on the running
  game until a `record_stats.py` recording passes `check_stats.py`
  (queued).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::stats` (keys, reads, evaluation, helpers);
  lists and their maintenance: `sim/stat-lists.md`
- **Related specs:** `sim/stat-lists.md` (lists, base/full arrays, when a
  full value is recomputed, callbacks, state lists, event handlers 3, 5,
  9, 12); `sim/stat-ops.tsv` (the op table, machine-readable);
  `data/fixups.md` §2 (the load-time op tables +0x51…+0xF5 this spec
  reads); `data/fields.tsv` (itemstatcost, charstats columns);
  `sim/intents-events.md` (stat messages; send bits).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 43–55 |
| Inputs | 56–64 |
| Outputs / state changes | 65–69 |
| Rules | 70–71 |
|   1. Identity | 72–86 |
|   2. Values | 87–106 |
|   3. itemstatcost columns read by the simulation | 107–137 |
|   4. Reading a stat | 138–172 |
|   5. MulDiv (`0x00483360`) | 173–187 |
|   6. Evaluation (the value a full entry is set to) | 188–263 |
|   7. Where evaluation happens | 264–273 |
|   8. By-time adjustment (`0x0065CA30`) | 274–290 |
|   9. Derived stats and clamps | 291–306 |
| Constants & data dependencies | 307–321 |
| Randomness | 322–325 |
| Edge cases & original bugs | 326–337 |
| Test vectors | 338–361 |
| Provenance | 362–382 |
| Open questions | 383–392 |
<!-- /index -->

## Summary

A stat is a (stat id, layer) pair with a signed 32-bit value. Values
live in stat lists (`stat-lists.md`): a unit's list keeps its own base
values and a cached **full** value per stat, which is the base value,
plus the contributions of every list attached to it, plus the stat's
**op** terms (percent bonuses, per-level bonuses, vitality→life, …). This
spec owns the key and value encoding, what each itemstatcost column means
to the simulation, how a value is read (base, total, minimum rule), the
op formulas that build a full value, the integer helpers they use
(MulDiv, by-time), and derived stats. When a full value is recomputed is
`stat-lists.md` §6.

## Inputs

| Name | Type | Source |
|---|---|---|
| stat id, layer | u16, u16 | callers (skills, items, states, regeneration, …) |
| itemstatcost records | 0x144 bytes, n = 359 in 1.14d | `data/fields.tsv`; load fix-ups `data/fixups.md` §2 |
| charstats records | 0xC4 bytes | `ManaRegen` +0x3A, `LifePerVitality` +0x46, `StaminaPerVitality` +0x47, `ManaPerMagic` +0x48 |
| act environment | base time per act | `tick.md` §3 step 1 (day cycle) |

## Outputs / state changes

None by itself: reads and pure evaluation. The writes are
`stat-lists.md` §5–§6.

## Rules

### 1. Identity

1. A stat id s is valid when 0 ≤ s < n (n = itemstatcost count, global
   `[0x744304]+0xBD4`; records at `[0x744304]+0xBCC`, 0x144 bytes each,
   lookup `0x0045C4F0`). Every getter returns 0 for an invalid id. The
   base setters do **not** check the id: an invalid id is stored in a
   base array and never propagated (`stat-lists.md` §5, edge case 1).
2. Layer (D2MOO `nLayer`, the "param") is a u16: skill id for skill
   stats, state id, monster type, etc. Most stats use layer 0.
3. **Key** := (s << 16) + layer, 32-bit, compared as **signed** i32
   (binary searches use `jl/jg`). With s < 0x8000 every key is
   non-negative, so the order is s ascending, then layer ascending.
4. Op evaluation, op bases, op targets and the by-time refresh always use
   layer 0 of the stats they read (§6), whatever the caller's layer.

### 2. Values

1. Values are i32. Additions wrap (two's complement); nothing saturates
   unless a rule says so.
2. `ValShift` (u8 +0x18) is the fixed-point shift of a stat. 1.14d:
   8 for stats 6–11 (`hitpoints`, `maxhp`, `mana`, `maxmana`, `stamina`,
   `maxstamina`) and 216, 217 (`item_hp_perlevel`, `item_mana_perlevel`);
   0 for all others. So life, mana and stamina are stored in 1/256
   points; displayed points = value >> 8.
3. The shift is applied only where a rule names it: the minimum rule
   (§4.3), op bases (§6.3 ops 2–5) and message/save encoding
   (`intents-events.md`). Arithmetic on stored values never rescales.
4. `Signed`, `Send Bits`, `Send Param Bits`, `Send Other`, `CSvBits`,
   `CSvParam`, `CSvSigned`: message encoding only (`intents-events.md`);
   not read by the code of this spec or `stat-lists.md`.
5. `Save Bits`, `Save Add`, `Save Param Bits`, `1.09-Save *`, `Encode`:
   save and item serialization (save and item specs, Phase 3+).
6. Stat 352 `last_sent_hp_pct` holds the life fraction last sent to
   clients, 0–128 (§9.3).

### 3. itemstatcost columns read by the simulation

Offsets in the runtime record (`data/fields.tsv`, bits of the u32 at
+0x04). Only these drive stat-list behaviour:

| Column | Offset | Rule |
|---|---|---|
| `damagerelated` | +0x04 bit 2 | dynamic lists skip it (`stat-lists.md` §6.1, §8) |
| `fMin`, `MinAccr` | bit 10; u32 +0x2C | minimum rule §4.3 |
| `fCallback` | bit 11 | value-change notification (`stat-lists.md` §7) |
| `Saved` | bit 12 | mod array, i.e. which player stats are re-sent (`stat-lists.md` §11) |
| `ValShift` | u8 +0x18 | §2.2 |
| `keepzero` | u8 +0x50 | a full value of 0 stays in the full array (`stat-lists.md` §6.3) |
| `op`, `op param`, `op base`, `op stat1–3` | u8 +0x54, u8 +0x55, u16 +0x56, u16 +0x58/5A/5C | §6 |
| load-time op data | +0x51, +0x52, +0x53, +0x5E (64 × u16), +0xDE (16 × 6 bytes) | built by `data/fixups.md` §2; used by §6 and `stat-lists.md` §6 |
| `itemevent1/2`, `itemeventfunc1/2` | u16 +0x48, +0x4A, +0x4C, +0x4E | read by the value-change callback (`stat-lists.md` §7.2); item events: items spec |

Derived names used below: for stat T, **A51(T)** = byte +0x51 (T is an op
base or has op stats), **A52(T)** = +0x52 (T is the target of some op),
**A53(T)** = +0x53 (T has op 4 or 5 with a valid base), **deps(T)** =
the +0x5E list (stats whose op base is T, until 0xFFFF), **entries(T)** =
the +0xDE table (ops that target T, in table order, each: u16 base, u16
source stat, u8 op, u8 param; the list ends at the first entry whose op
is 0; 16 at most).

Not read by the code of this spec or `stat-lists.md` (verified by
reading every function they cite): `Direct`, `MaxStat` (1.14d: 6→7,
8→9, 10→11, 72→73), `UpdateAnimRate` (67, 68, 69), `itemspecific`,
`Add`, `Multiply`, `Divide`, description columns. Their readers belong
to the item, property and animation specs (open question 1).

### 4. Reading a stat

#### 4.1 Arrays read

A list has a **base** array (its own values) and, if extended, a
**full** array (`stat-lists.md` §1, §6). "Absent" means the key is not in
the array; an absent value reads as 0 and the minimum rule is not applied
to it.

#### 4.2 Readers

| Reader | 1.14d | Reads |
|---|---|---|
| base value of a list | `0x00625350` (list, s, layer) | base array, minimum rule |
| total value of a list | `0x00625420` (list, s, layer) | full array if extended, else base array; minimum rule |
| unit total | `0x00625480`, `0x00625500` (unit, s, layer) | total of unit +0x5C |
| unit base | `0x006253B0` (unit, s, layer) | base of unit +0x5C |
| unit bonus | `0x00625560` (unit, s, layer) | total − base (each with its own minimum rule) |
| max life / mana / stamina | `0x00625D10` / `0x00625D60` / `0x00625DB0` (unit) | unit total of 7 / 9 / 11, layer 0 (minimum rule of that stat) |
| percent-adjusted value | `0x006255A0` (list, s, pct stat, flag) | base(s) + MulDiv(base(s), total(pct stat), 100) + (flag ? total(s) − base(s) : 0), all on the list, layer 0, minimum rules applied (one caller) |

A null list or unit reads 0.

#### 4.3 Minimum rule (`fMin`)

When the stat has `fMin`, the list is extended and its owner unit
(+0x44) is a player or a monster, and the present value v < `MinAccr`
(signed compare, unshifted): the reader returns `MinAccr << ValShift`.
Otherwise v. The comparison is against the unshifted minimum while the
result is shifted (original bug, reproduced): `maxhp` raw 0 reads as 256
(1 point), but raw 100 (0.39 points) reads as 100. 1.14d: `fMin` on 0, 1,
2, 3 (`MinAccr` 1), 7 (1), 9 and 11 (0). The rule applies only to reads
through these readers, never to the evaluation of §6.1 (sums use raw
array values).

### 5. MulDiv (`0x00483360`)

MulDiv(a, b, c), all i32, used for every percent in this spec
(1.14d; D2MOO 1.10f uses floating point here):

1. c = 0 → 0.
2. a > 0x100000: if c ≤ (a >> 4): (a / c) · b (32-bit product, wraps);
   else (a · b) / c with a 64-bit product, low 32 bits of the quotient.
3. Else, b > 0x10000: if c ≤ (b >> 4): (b / c) · a; else 64-bit
   (a · b) / c.
4. Else: (a · b) / c with a **32-bit** product (wraps; e.g. a = 0x100000,
   b = 0x10000 gives 0).

Divisions truncate toward zero; comparisons are signed.

### 6. Evaluation (the value a full entry is set to)

`0x00626200` (ECX list, EDX key) computes eval(L, k) for an extended list
L; it has no side effects.

#### 6.1 Sum

v := L.base[k] + Σ over the lists attached to L (the active chain,
`stat-lists.md` §1, from the most recently attached backwards) of
c.full[k] if c is extended, else c.base[k]; a child c is skipped when the
stat is `damagerelated` and c has the DYNAMIC flag (`stat-lists.md` §2).
Lists in L's parked (SET) chain never count. Raw values, i32 wrap, no
minimum rule. (`0x00624FE0`.)

If A52(T) = 0: eval = v.

#### 6.2 Op loop

Else, with acc := v and prev := v, for each entry e of entries(T) in
order (stop at op 0; 16 at most; ops > 13 skipped), with S = e.source
stat and r := eval(L, S << 16) where a rule needs it (recursive, same
list), apply the row of `sim/stat-ops.tsv` for e.op. Each row gives:

- **guard** (skip the entry unless true): `owner` L's owner unit (+0x44)
  is set; `owner_item` owner type 4; `owner_player` owner type 0 (and a
  charstats record exists for its class); `owner_pm_prev` prev ≠ 0 and
  owner type 0 or 1; `owner_act` owner set and its act (unit +0x1C) set;
  `listtype_pm` the list's own owner-type field (+0x08) is 0 or 1;
  `unit_pm` the unit L is attached to (+0x04) is set and of type 0 or 1;
  `never`.
- **prev**: `owner_item_base`: when the owner is an item, prev := the
  owner item's own base value of T (unit base, layer 0); `keep`.
- **operand** x (ops 2–5): e.base = 0xFFFF or an invalid stat → skip;
  `opbase_list_total`: x := total of key (e.base << 16) on L itself, raw
  (no minimum rule); `opbase_unit_total`: x := unit total of e.base on the
  attached unit (minimum rule of e.base); then x := x >> ValShift(e.base)
  (arithmetic); x ≤ 0 → skip.
- **contribution** (r = 0 → skip, unless noted):
  - `muldiv_prev_r`: prev = 0 → skip; acc += MulDiv(prev, r, 100);
  - `shift_r_x`: acc += (r · x) >> e.param (32-bit product, arithmetic
    shift);
  - `muldiv_prev_shift_r_x`: acc += MulDiv(prev, (r · x) >> e.param, 100);
  - `bytime_r`: acc += ByTime(r, owner's act) (§8);
  - `muldiv_acc_bytime_r`: acc += MulDiv(acc, ByTime(r, owner's act), 100)
    (acc as accumulated so far);
  - `charstat_mana_bonus`: d := r − L.base[S << 16] (0 if absent); d = 0
    → skip; acc += (ManaPerMagic(class) · d) << 6;
  - `charstat_vit_bonus`: same with LifePerVitality, or StaminaPerVitality
    when T = 11.
- **recompute_block**: used by `stat-lists.md` §6.4, not here.

eval = acc after the last entry. prev never follows acc; only
`owner_item_base` rows change it, and the change persists for later
entries.

#### 6.3 The op table

| Op | Meaning in 1.14d data | Contribution |
|---|---|---|
| 1 | % of another stat (162, 163 → maxstamina) | MulDiv(prev, r, 100); prev = item base when the owner is an item |
| 2 | per-level on the list (216, 217, 220–250: base 12 `level`) | (r · level) >> param |
| 3 | % per-level (unused in 1.14d data) | MulDiv(prev, (r · level) >> param, 100) |
| 4 | per-level of the wearer (214, 218) | (r · wearer level) >> param |
| 5 | % per-level of the wearer (215, 219) | MulDiv(prev, (r · wearer level) >> param, 100) |
| 6 | by-time (268, 270–272, 274–303) | ByTime(r) |
| 7 | % by-time (269, 273) | MulDiv(acc, ByTime(r), 100) |
| 8 | energy → maxmana (stat 1) | (ManaPerMagic · bonus energy) << 6 |
| 9 | vitality → maxhp, maxstamina (stat 3) | (LifePerVitality or StaminaPerVitality · bonus vitality) << 6 |
| 10, 12 | none | nothing |
| 11 | % (76 → maxhp, 77 → maxmana) | MulDiv(prev, r, 100), owner player/monster |
| 13 | % of an item's own base (16, 17, 18, 75, 94) | MulDiv(item base of T, r, 100), owner item |

"Bonus" for ops 8/9 = eval of the source minus the list's own base value
of the source: points from items, states and other attached lists. The
`<< 6` turns quarter points (charstats) into 1/256 points.

### 7. Where evaluation happens

Evaluation runs only when `stat-lists.md` §6 recomputes a full entry (on
a base change, attach, detach, refresh). Reading never evaluates: a full
value that depends on something that changed without a recompute stays
stale until the next recompute (e.g. ops 4/5 of an item when the
wearer's level changes are refreshed only through `deps(level)` on the
item's own list, `stat-lists.md` §6.4; op 6/7 by-time values only by
`stat-lists.md` §8.7).

### 8. By-time adjustment (`0x0065CA30`)

ByTime(v, t) with v the packed value of an op 6/7 source stat and t the
act's base time:

1. p := v & 3 (period), lo := ((v >> 2) & 0x3FF) − 256, hi := ((v >> 12)
   & 0x3FF) − 256.
2. d := |t − 90·p|; a := ((d + 7) / 15) · 15 (truncating); a ≤ 0 → 0;
   a ≥ 359 → a := 1; else a > 180 → a := 360 − a.
3. Result := hi − ((hi − lo) · a) / 180 (32-bit product, truncating).

t = environment(act) time / period length: the act's environment record
(act +0x04), dword +0x08 divided by dword +0x28 (0 when +0x28 is 0),
`0x0061C100`. That record is advanced in `tick.md` §3 step 1; its fields
belong to the environment spec (Phase 3, not written). The period index
`0x0061C100` also returns is not used by ByTime.

### 9. Derived stats and clamps

1. Max life, mana, stamina: §4.2.
2. **Clamp current to max** (`0x006275B0`, unit): for (current, max) =
   (10, 11), (8, 9), (6, 7) in that order: if total(current) > max
   getter, add (max − current) to the unit's base current
   (`stat-lists.md` §5.3). Callers: `0x0055C83B` (server), `0x004C0F08`
   (client).
3. **Life fraction** (`0x005A5650`, also inline in player regeneration):
   h := total(6) >> 8, m := max life >> 8; if m > 0 and h < m: (h << 7) /
   m, else 128. A unit's stat 352 is updated, and the message
   `0x00571A10` sent, only when |fraction − stat 352 (low byte)| > 4
   (`stat-lists.md` §10.1).
4. When max life, mana or stamina changes, the current value is rescaled
   by the value-change callback (`stat-lists.md` §7.2).

## Constants & data dependencies

| Constant | Value | Source |
|---|---|---|
| itemstatcost count | 359 (≤ 511, `data/loading.md`) | `check_stats.py --files` |
| ValShift ≠ 0 | 6–11, 216, 217: 8 | same |
| keepzero | 8, 10 | same |
| fMin / MinAccr | 0, 1, 2, 3, 7: 1; 9, 11: 0 | same |
| Saved | 0–15 | same |
| fCallback | 35 stats (7, 9, 11, 78, 81, 83, …, 204) | same |
| damagerelated | 104 stats | same |
| ops used | 1:2, 2:33, 4:2, 5:2, 6:34, 7:2, 8:1, 9:1, 11:2, 13:5 stats; 42 op targets | same |
| flag bits used in +0x04 | 0, 1, 2, 3, 4, 9, 10, 11, 12 (+ 5–8 set at load, `fixups.md` §2) | same |
| bit-mask table | `0x006CE268` (1 << k, k = 0…31); complement table `0x006CE2E8` | file image |

## Randomness

None.

## Edge cases & original bugs

1. Invalid stat ids are stored by the base setters but never propagate
   or read (§1.1).
2. Minimum rule compares unshifted (§4.3).
3. MulDiv's 32-bit branch wraps (§5.4).
4. Ops 2/3 read the op base from the evaluated list itself (raw), ops
   4/5 from the attached unit (with its minimum rule) (§6.2).
5. A stat whose op entries reach itself recurses without end; 1.14d
   data has no cycle in the op graph (`check_stats.py --files`).
6. Op evaluation ignores the caller's layer for sources and bases (§1.4).

## Test vectors

Synthetic (hand-computed; `check_stats.py --selftest` asserts them):

| Input | Expected | Rule |
|---|---|---|
| MulDiv(250, 40, 100) / (−250, 40, 100) / (7, 9, 0) | 100 / −100 / 0 | §5 |
| MulDiv(0x200000, 3, 0x10) | 0x60000 (a/c·b) | §5.2 |
| MulDiv(0x200000, 0x100, 0x100000) | 0x200 (64-bit) | §5.2 |
| MulDiv(100, 0x20000, 0x1000) | 0xC80 (b/c·a) | §5.3 |
| MulDiv(0x100000, 0x10000, 1) | 0 (32-bit wrap) | §5.4 |
| ByTime(v = 356<<12 \| 306<<2 \| 0, t = 0 / 180) | 100 / 50 (lo 50, hi 100) | §8 |
| ByTime(same, period 1, t = 0) | 75 (d = 90) | §8 |
| player, class LifePerVitality 16, StaminaPerVitality 4; base vitality 25, maxhp 12800, level 10; item list +10 vitality, +8 `item_hp_perlevel` (op 2, param 3) attached | full maxhp = 12800 + (16·10 << 6) + (8·10 >> 3) = 23050; full maxstamina = 4·10 << 6 = 2560 | §6, `stat-lists.md` test vectors |

Real data (`check_stats.py --files game`, patch_d2 `itemstatcost.bin`):
the constants table above; entries(7) = (base 0xFFFF, source 3, op 9),
(0xFFFF, 76, 11), (12, 216, op 2, param 3), (0xFFFF, 270, op 6) in that
order; deps(12) = 214…250; A53 set on 214, 215, 218, 219 only.

Comparison (exact): every full value in a recording equals §6 applied at
the moments `stat-lists.md` §6 recomputes it, with the recorded inputs —
defined and checked in `stat-lists.md` (Test vectors).

## Provenance

- **1.14d `Game.exe`** (SHA-256 `631066c1…adaaf`), disassembly via
  `tools/ghidra/disasm.py`: readers `0x00624ED0` (base, min rule),
  `0x00624F60` (total, min rule), `0x00624FE0` (sum), `0x00626200`
  (evaluation; jump table at `0x00626688`, 13 entries read from the file
  image), MulDiv `0x00483360`, ByTime `0x0065CA30`, `0x0061C100`,
  `0x0061AA60`, getters `0x00625350`–`0x00625DB0`, clamp `0x006275B0`,
  life fraction `0x005A5650`. Charstats offsets from `data/fields.tsv`
  and the reads at `0x00626573`, `0x006265ED`/`0x006265FC`,
  `0x00580757`.
- **D2MOO** (1.10f) `D2Common/src/D2StatList.cpp` (`sub_6FDB5830`,
  `STATLIST_GetBaseStat_6FDB6340`, `STATLIST_GetTotalStat_6FDB63E0`,
  `STATLIST_ApplyMinValue`): same structure. Confirmed against 1.14d
  instruction by instruction. Differences: 1.14d computes every percent
  with integer MulDiv (D2MOO: `value * (op / 100.0)`, floating point);
  everything else in §4 and §6 matches.
- **Data**: `check_stats.py --files game` measures every 1.14d number in
  this spec from `game/extracted/patch_d2/data/global/excel/itemstatcost.bin`
  and rebuilds the load-time tables with `data/fixups.md` §2.

## Open questions

1. Readers of `Direct`, `MaxStat`, `UpdateAnimRate`, `itemspecific`:
   property application and animation code (items/animation specs);
   confirm none of them writes stat arrays outside `stat-lists.md` §5.
2. Op 3 is unused by 1.14d data; its formula is read from the code only.
3. Environment record fields +0x08 / +0x28 (time, period length) and
   their update: environment spec; until then op 6/7 inputs are recorded
   (`senv`) rather than computed.
