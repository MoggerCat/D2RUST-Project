# Spec: Skills — Start and do function bodies

- **Status:** draft: every body and helper below read from the 1.14d
  `Game.exe` disassembly (addresses per rule; `py tools/ghidra/disasm.py`,
  the Ghidra export lacks several entries, `use.md` Open question 1);
  D2MOO 1.10f `Skills.cpp`, `SkillPal.cpp`, `SkillSor.cpp`,
  `SkillNec.cpp`, `SkillAss.cpp`, `SkillBar.cpp`, `SkillDruid.cpp`,
  `SkillAma.cpp` compared, differences noted. No
  recording covers these bodies yet (Open questions 1–3, 9).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::skills::use_::bodies` (start / do bodies,
  `functions.tsv` status `spec'd-here`); helpers beside them
- **Related specs:** `skills/use.md` (pipeline: §5.3 start core, §5.4 do
  core, §7 periodic and aura scheduling, §8 tables); `skills/levels.md`
  (`skill_level`, `highest_entry`, `eval`, `to_hit`, mana cost,
  `roll_elemental`); `combat/hit.md` §4 (`melee_result`);
  `combat/damage.md` (§1 damage record, §3 `start_combat`, §3.1 `fill`,
  §3.2 `bonuses`, §5.1 `apply_melee`, §5.4 unit events);
  `missiles/missiles.md` §R2 (missile creation); `sim/stat-lists.md`
  (lists, attach §8.1, detach §8.2, states §9); `sim/units.md` (modes,
  flags, animation rate); `sim/unit-order.md` §5 (room unit lists);
  `drlg/rooms.md` §6 (adjacency arrays); `sim/tick.md` §5 (timers);
  `data/fields.tsv` (column offsets); `skills/functions.tsv` (which
  slots are specified here).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 49–61 |
| Inputs | 62–72 |
| Outputs / state changes | 73–80 |
| Rules | 81–82 |
|   1. Conventions | 83–114 |
|   2. Shared helpers | 115–534 |
|   3. Start functions (srvst) | 535–625 |
|   4. Do functions (srvdo) | 626–822 |
|   5. `srvmissile` path | 823–838 |
|   6. Shared helpers, batch 2 | 839–1177 |
|   7. Start functions (srvst), batch 2 | 1178–1244 |
|   8. Do functions (srvdo), batch 2 | 1245–1668 |
| Constants & data dependencies | 1669–1715 |
| Randomness | 1716–1734 |
| Edge cases & original bugs | 1735–1784 |
| Test vectors | 1785–1805 |
| Provenance | 1806–1843 |
| Open questions | 1844–1869 |
<!-- /index -->

## Summary

A skill's start function (`srvstfunc`) runs when its animation starts and
may refuse it; its do function (`srvdofunc`) runs at the action frame
(`skills/use.md` §5). This spec gives the 1.14d bodies of the slots used
by the basic skills, the level-1 skills of the five classic classes, the
Act I monster skills that have a start function, the aura, curse and
buff do functions, and the `srvmissile` path, with the helpers they
share: target lookup, ammunition and quantity, skill missiles, state
lists, aura and curse area scans with the `aurafilter` bits, unit event
handlers and Assassin progressive charges. Combat rolls, missile flight
and stat arithmetic belong to their own specs; bodies here call them.

## Inputs

| Name | Type | Source |
|---|---|---|
| game, unit | game, player or monster | start / do core (`use.md` §5.3, §5.4) |
| skill id, level L | i32 | core arguments (level from `skill_level(unit, entry, 1)`) |
| `skills.txt` record R | 0x23C bytes | columns by `data/fields.tsv` offset |
| target | unit | the unit's path target (§2.1) |
| frame F | u32 | game +0xA8 |
| difficulty | u8 | game +0x6D |

## Outputs / state changes

Damage records and combat entries (`combat/damage.md` §3), missiles
(`missiles.md` §R2), state stat lists on the unit and targets, state
bits, type-12 timers, unit event handlers, item quantity and durability
stats with message 0x3E, unit flag 0x40, the used skill entry's params,
and the return value read by the core (0 = refused / nothing happened).

## Rules

### 1. Conventions

- Calling convention of every body: ECX game, EDX unit, stack (skill,
  L); `ret 8`. "Return v" is the value the core sees (`use.md` §5.3
  step 6.6, §5.4 step 6).
- R = the skill's record; "R invalid" = skill < 0 or ≥ the skills count.
  Columns are named by `fields.tsv` (`aurastate` +0x80, `calc1` +0x138,
  …). `eval(c)` = `levels.md` `eval(unit, c, skill, L)` with the acting
  unit unless another unit is named.
- T = `target(game, unit)` (§2.1). F = game frame.
- Position of a unit: types 2, 4, 5 read the static path (+0x0C x,
  +0x10 y); other types the dynamic path x / y (`0x006488C0`,
  `0x00648900`), 0 without a path (`units.md`).
- State operations (`stat-lists.md` §9): "state on/off" = `0x00639DB0`
  (toggle with update-queue insert); "mark changed" = `0x00639E30(unit,
  s, 1)`; "list of s" = `0x006256B0(unit, s)`; "has s" = `0x00639DF0`;
  "group g" = `0x0063A7B0(unit, g)` (any state with flag bit g).
- List operations (`stat-lists.md`): alloc `0x006251F0(pool, flags,
  expire, owner type, owner GUID)` (no unit → type 6, GUID −1); set
  state id `0x006252D0`; attach `0x00626E10(unit, list, 1)` (§8.1);
  remove callback := `0x00625CE0(list, f)` (list +0x38); "detach and
  free" = `0x006277E0(unit, list)` then `0x00626CD0(list)` (§8.2, §8.3;
  detach runs the remove callback); "list set" = `0x00627150(list, s,
  v, 0)` (§5 rule 2); list stat read `0x00625D00(list, s, 0)`.
- "Timer 12 at e" = `0x005417D0(game, unit, 12, e, 0, 0)` (remove-state
  timer, `tick.md` §5); another one per call.
- "Anim refresh" = `0x00623F50(unit)` (`units.md` §4.3).
- Stat 350 `modifierlist_skill`, 351 `modifierlist_level` mark which
  skill and level own a state list.
- Mask table `0x006CE268` holds `1 << k`; flag tests below name the
  `fields.tsv` bit they select.

### 2. Shared helpers

#### 2.1 Target `0x00553540`

`target(game, unit)` (stdcall):

1. Refresh `0x00553490` (a missing unit, game or path is a fatal
   assertion): if the path has a target unit (`0x00648BF0`), read its
   stored (type, GUID) (`0x00648BB0`) and look the unit up
   (`0x00552F60`). Found unit ≠ the stored pointer (including none) →
   clear the path target (`0x00648B90(path, 0)`). Found unit is an item
   (type 4) in mode 1 or 2 → clear it too.
2. Return the path target unit, or none when it is the unit itself.

#### 2.2 Pair record `0x0057D690`

`pair_record(attacker, defender)` (ECX, EDX): none if either is null;
else the first entry of the attacker's combat list (+0xAC, next +0x84)
whose attacker type / GUID (+0x04, +0x08) and defender type / GUID
(+0x0C, +0x10) match; return a pointer to its damage record (entry
+0x14, `combat/damage.md` §1). Writes through it change what
`apply_melee` later applies.

#### 2.3 Weapon and ammunition

- `is_bow(unit)` = `0x0064F460`: weapon class of the unit's composit
  (`0x0064F060`; through the disguise remap `0x00645270` when unit +0xC8
  bit 3 is set) is 1 (bow) or 7 (crossbow).
- `bow_missile(unit, &lvl)` = `0x00645F00`: none → 0. Player (type 0):
  hand class `0x00623C60` = 1 → arrows, = 7 → bolts, else −1. Other
  types → arrows. Arrows / bolts: the unit's `item_magicarrow(157)`
  (item/skill getter `0x00625500`) > 0 → `lvl` := it, return 27 (magicarrow); else
  `item_explosivearrow(158)` > 0 → `lvl` := it, return 41
  (explodingarrow); else arrows 0 (arrow), bolts 31 (bolt).
- `has_ammo(unit)` = `0x0056C4E0`: non-players → 1. Player: weapon W
  (`0x00535BC0`) none → 0; used skill entry (`0x00620250`) none → 0.
  `any = 1`; if W's item type has a `shoots` value (itemtypes +0x0C,
  `0x0062E6F0`): `any = 0`, and if the used skill's id is 0 (Attack)
  and W has `item_magicarrow(157)` → 1. Then right hand (body location
  4), then left (5) (`0x0063BDE0`): an item I there, accepted when `any
  = 0` or I = W, that is a stack (`stackable` items +0x132,
  `0x006289F0`; or its type `throwable`, itemtypes +0x10, `0x0062BA80`;
  or `item_throwable(125)` ≠ 0) → return `has_qty(I)`. Else 0.
- `has_qty(I)` = `0x0062A310`: `quantity(70)` > 0 → 1; else
  `item_throwable(125)` ≠ 0 → set quantity 0, 1; else 1 when the
  maximum stack (`0x006295B0`) ≤ 0, otherwise 0.

#### 2.4 Skill missiles `0x0056ECB0`, `0x0056EE90`

`skill_missile(game, missile, unit, skill, L, dx, dy, tx, ty, quant)`
(ECX game, EDX missile class; `ret 0x20`) fills a zeroed 0x5C parameter
record (`missiles.md` §R2.1) and creates the missile:

| Field | `0x0056ECB0` (straight) | `0x0056EE90` (lob) |
|---|---|---|
| flags | 0x21 (position given, target absolute) | 0x420 (target absolute, frames from distance) |
| owner | unit | unit |
| origin | — | unit |
| x, y | unit position + (dx, dy) | unit position + (dx, dy) |
| target | (tx, ty) | (tx, ty) |
| skill, level | skill, L | skill, L |
| attack bonus | monster with `tohit(19)` ≠ 0: flag 0x1000, bonus = it | — |

Steps: tx = 0 or ty = 0 → target position `0x0056D2C0(game, unit,
&tx, &ty)` (target unit's position, else the path's target point
`0x00648A00` / `0x00648A10`; returns 0 when either is 0; a stale
target unit is cleared and the stored point used, `sim/pathing.md`
§13.2) and return 0
if that fails; still tx = 0 or ty = 0 → 0. `quant ≠ 0` and the unit is
a player: `dec_quantity` (§2.5) < 1 → 0. Return `0x0059FA30(game,
record)` (the missile, or none).

#### 2.5 Quantity `0x0056C3F0`

`dec_quantity(game, unit)` (`decquant` in `use.md` §5.4 step 9):

1. Not a player → 0.
2. W = weapon (`0x00535BC0`). `weapon_only = 1`, or 0 when W is of
   item type 27 (bow) or 35 (crossbow) (`0x00629BB0`).
3. For the right hand (4), then the left (5): item I there, accepted
   when `weapon_only = 0` or I = W, and a stack (as in `has_ammo`) →
   return `use_one(game, unit, I)`.
4. Return 0.

`use_one(game, player, I)` = `0x0056C310`: `q = quantity(70) − 1`
(unit getter); `q < 0` → `q = 0`, empty. Set stat 70 := q
(`0x00627260`); quantity-replenish timer `0x00558580(game, I)`
(`items/generation.md`); message 0x3E to the player's client
(`0x005531C0`, then `0x0053D130(client, I, 1, 70, q, 0)`). m = maximum
durability (`0x00625E00`); m ≠ `durability(72)` → set 72 := m and send
0x3E for stat 72. Attack-mode cleanup `0x00580310(game, player)`
(`units.md` §4.5; §2.16). Return 0 when empty, else q + 1.

#### 2.6 Stat fills `0x005C6CC0`, `0x005C6DC0`

`aura_fill(unit, list, R, skill, L)` = `0x005C6CC0` (ECX unit, EDX
list; R null → looked up from skill, invalid → nothing; unit or list
null → nothing): for i = 1…6: s = `aurastat_i`; s valid (0 ≤ s <
itemstatcost count): `v = eval(unit, aurastatcalc_i)`; v ≠ 0 → list
set s := v (`0x006270B0`), and s = 68 (`attackrate`) → also stat 69 :=
v. After the loop: anim refresh of the unit.

`passive_fill` = `0x005C6DC0`: the same over `passivestat1–5` (+0x98) /
`passivecalc1–5` (+0xA4).

The unit given is the one formulas are evaluated on.

#### 2.7 State lists: `apply_state` `0x0056E970`

Request (ECX): +0x00 source, +0x04 target, +0x08 skill, +0x0C level,
+0x10 duration, +0x14 stat (−1 none), +0x18 value, +0x1C state, +0x20
remove callback (0 → `0x0056E900`). Returns the list or none. D2MOO
`sub_6FD10EC0`.

1. State invalid (not 0 ≤ s < states count) → none.
2. Target a monster: its monstats `npc` (+0x0C bit 8) set → none; no
   monstats2 record (`0x00451FE0`) or `isAtt` (+0x04 bit 9) clear →
   none.
3. Curse state (flag group 11 `curse`, `0x0063A420`): r =
   `curse_resistance(109)` of the target (unit getter); r ≥ 100 → none;
   r ≠ 0 → duration −= `pct(duration, r, 100)` (`0x00483360`). Target
   has state 57 (`attract`) → none. Old = the target's first list with
   flag 0x20 (`0x00625760`); new flags = 0x20.
   Not a curse: old = list of s; new flags = 0.
4. Old exists: same state, same skill (+0x1C) and same level (+0x20) →
   refresh: duration ≠ 0 → expiry := F + duration (`0x00625310`), timer
   12 at it; return old. Same state and skill, request level < old
   level → none. Otherwise detach and free old.
5. Queue the target for update (`0x0064C040`); state s on.
6. duration ≠ 0: e = F + duration, timer 12 at e on the target, flags
   |= 2. State has `exp` (group 30, `0x0063A6B0`) → flags |= 0x800.
   State `aura` (states +0x10 bit 1) → flags |= 8.
7. Alloc (target's pool, flags, e or 0, source type / GUID), set state
   s, skill, level (`0x006260D0`, `0x006260F0`); stat ≠ −1 → list set
   stat := value; attach to the target; remove callback := request +0x20
   or `0x0056E900`. Return it.

#### 2.8 Remove callbacks

Called by detach (`stat-lists.md` §8.2 step 6) with ECX unit, EDX state,
stack list.

- **Default** `0x0056E900` (D2MOO `sub_6FD10E50`): unregister the unit's
  handlers of (1, state) (§2.13). If the unit is alive (`0x005541B0` = 0)
  or the state is not "stay on death" for it (`0x0063A4A0`): state off;
  anim refresh; passive refresh `0x00646F20(unit)`; a player also gets
  the pet-maximum resync `0x00575900(game, unit)` (§2.17). "State off"
  is `0x00639DB0(unit, state, 0)` (`0x0056E92D`–`0x0056E931`): the
  toggle **with** the update-queue insert (`sim/stat-lists.md` §9.2), so
  the unit is queued and the next client pass sends its S→C 0xA9; the
  anim refresh is `0x00623F50(unit)` (1.14d-read 2026-10-09, settles
  REC-731).
- **Self aura** `0x005CEC50`: state off; has 85 (`nomanaregen`) → 85
  off; re-enable passive states `0x0056DFA0` (for each skill entry with
  a `passivestate` > 0: state on, `0x00646D60(unit, entry)`); clamp
  `0x0056B6C0` (life 6, mana 8, stamina 10 each set to its maximum
  `0x00625D10` / `0x00625D60` / `0x00625DB0` when above it, then anim
  refresh).
- **Buff** `0x005C9420`: unregister (1, state); if alive or not
  "stay on death": state off, anim refresh, passive refresh
  `0x0056DE40(unit)`.
- **AI curse** `0x005C3370` (Dim Vision, Terror): unregister (1, state);
  a monster gets AI state 0 back (`0x005B0E00(game, unit, AI control,
  0)`, `monsters/ai.md` §3.3; control = monster data +0x28, none
  without data); then the default callback.

#### 2.9 Same-group removal `0x0056C740`

`clear_group(unit, s, include_self)`: g = states `group` (+0x1E) of s;
s invalid or g = 0 → 0. For each state t in index order: skip t = s
when `include_self = 0`; if t's group = g and the unit has t: state t
off, detach and free t's list (if any); result 1. Return the result.

#### 2.10 Resistance scaling `0x005C3540`

`scaled(target, s, v)` (ECX target, EDX s): for s ∈ {36
`damageresist`, 37 `magicresist`, 39 `fireresist`, 41
`lightresist`, 43 `coldresist`, 45 `poisonresist`} (byte table
`0x005C35B0`), v ≤ 0, a target that is not a player (none counts as
"not a player") and not a hireling (`0x0063EE90` = 0), and the target's
base stat s (`0x006253B0`) ≥ 100: return v / 5 (signed, truncating).
Else v.

#### 2.11 Unit filter `0x0056B3E0` (`aurafilter`)

`accepts(source, unit, f)` (ECX source, EDX unit; f = 0 is replaced by
0x583 in `scan_unit` only, §2.12). By the unit's type (jump table
`0x0056B6AC`), then the common tests:

| Type | Needs | Mode rule | Extra |
|---|---|---|---|
| 0 player | f & 0x1 | f & 0x1000: mode 17 (dead) only; else not mode 0 or 17 | — |
| 1 monster | f & 0x2 | f & 0x1000: mode 12 (dead) only; else not mode 0 or 12 | f & 0x4: undead (monstats `lUndead` or `hUndead`, `0x0063E990`); f & 0x4000: not `boss`; f & 0x40000: not `primeevil` |
| 2 object | f & 0x10 | — | — |
| 3 missile | f & 0x8 | — | row has `Explosion` (missiles +0x04 bit 1) |
| 4 item | f & 0x20 | — | — |
| other | refused | | |

Common tests, in order (all must pass):

| Bit | Test |
|---|---|
| 0x80 | unit flags +0xC4 bit 0x4 |
| 0x400 | unit flags +0xC4 bit 0x8 |
| 0x100 | the unit's room not in town (`0x0061AB00`) |
| 0x10000 | ally: source (a missile source is replaced by its owner `0x00552FD0`, none → refused) and unit pass `0x00554DE0` (same unit after monster owner resolution `0x0058F0D0`, or two players in one party; `units.md`); a monster unit must not be `npc` |
| 0x8000 | hostile: `0x00554200(game, source, unit)` (missile → owner) |
| 0x20000 | melee range `0x00622C40(source, unit, 0)` (missile → owner) |
| 0x80000 | the unit lacks state 86 (`justhit`) |
| 0x200 | both rooms exist and the line test `0x0064E260(source room, source xy, unit xy, mask 4)` is clear |

Bit 0x2000 is read by the scans (§2.12). 1.14d `skills.txt` values:
Might, Prayer and the other party auras use 73731 = 0x12003 (players,
monsters, allies, no town rooms); the curses use 2 or 3.

#### 2.12 Area scans `0x0056B7E0`, `0x0056E780`

**Around the unit** `scan_unit(game, source, x, y, r, f, cb, arg,
noaura)` = `0x0056B7E0` (ECX game, EDX source; D2MOO `sub_6FD0FE80`):

1. Game, source or cb null, or r ≤ 0 → nothing. Room = the source's
   room (`0x00620BB0`); none → nothing.
2. x or y = 0 → the source's position. f = 0 → 0x583. Context {game,
   source, count 0, arg}.
3. For each room of the room's adjacency array in order (`0x00619790`,
   `drlg/rooms.md` §6, includes the room itself): f & 0x2000 and the room
   is in town → skip it. The room-box test `0x0056B770` always passes
   for r > 0 (its comparisons never fail; Edge case 5). For each unit of
   the room's unit list (head +0x74, next +0xE8, `unit-order.md` §5)
   except the source: d² = (ux − x)² + (uy − y)² (`0x006492A0`); d² ≤
   r² and `accepts(source, unit, f)` and not (noaura and a monster with
   monstats `noaura`, `0x00457490(class, 27)`) → `cb(ECX context, EDX
   unit)`; non-zero → count += 1.

**Around the target point** `scan_point(game, f, source, r, cb, arg)`
= `0x0056E780` (ECX game, EDX f; returns 1, or 0 when cb is null, the
target position fails or no room contains it):

1. Center = target position `0x0056D2C0` (§2.4). Room = the room
   containing it, searched from the source's room (`0x00463740`).
2. As above over that room's adjacency array (f & 0x2000 town skip;
   f = 0 is **not** replaced), without the source exclusion and the
   noaura test, distance from the center: `accepts(source, unit, f)` →
   `cb(ECX unit, EDX arg)`.

#### 2.13 Unit event handlers

Handler record (0x20 bytes, list head unit +0x90): +0x00 event (u8),
+0x02 flags (u16), +0x04 key type, +0x08 key (a state), +0x0C skill,
+0x10 level, +0x14 function, +0x18 previous, +0x1C next.

- Register `0x0056E740(game, unit, event, skill, L, func, type, key)`:
  func > 49 (unsigned compare: negative func too) or table
  `0x007325B0[func]` null → 0. Else `0x005C0AD0`: allocate the record
  and **prepend** it to the unit's list. The table has 50 u32 slots
  (0…49, dumped from the file): slot 0 and slots 32…49 are null, 1…31
  hold functions (addresses `0x005BF670`…`0x005CAD40`), so every func
  outside 1…31 returns 0 and allocates nothing.
- Unregister `0x005C0B50(game, unit, type, key)`: for each record with
  this type and key, in list order: flags bit 0 set (running) → flags
  |= 2; else unlink and free it.

The iteration by `0x005C0C30` (`combat/damage.md` §5.4): §2.18.

#### 2.14 Progressive charges

The progressive (`pgsv`) states (`data/runtime-maps.md` §4: 122–127)
carry a list with 350 = skill, 351 = level and the charge count in the
skill's `aurastat1`. Common lookup for a list P: skill k = P[350];
`lvl = max(P[351], skill_level(unit, highest_entry(unit, k), 1))`;
proceed only when k's record exists, lvl > 0, `aurastat1` valid and n =
P[`aurastat1`] > 0.

- **Before the roll** `0x005D3AC0(ECX unit, EDX record)`: record
  null or result lacks 1 (hit) → nothing. For each pgsv state the unit
  has with a list whose skill is > 0 and valid with `prgdam` (+0x44) =
  1: `0x005D3680` — record +0x0C (enhanced damage %) += n ×
  `eval(calc1)` (evaluated with k, lvl); then if T exists and k's
  `tgtoverlay` (+0x108) is in 1…count − 1: overlay on T
  (`0x00621E40(T, overlay, 0)`).
- **After the roll** `0x005D3BA0(ECX unit, EDX record)`: same guard
  and loop, by `prgdam`:
  - 2 `0x005D3790`: x = `ln12` of k at lvl (`0x004E6CA0`); n = 1:
    life leech (+0x38) += x; n = 2: life and mana leech (+0x3C) += x;
    n = 3: both += 2x; other n: nothing.
  - 3 `0x005D3880`: `roll_elemental(unit, record, k, lvl)`
    (`levels.md` §3.6); k's `EType` = 4 (cold), n ∈ {2, 3} and `Param2`
    ≠ 0 → freeze length (+0x34) += cold length (+0x30) / `Param2`
    (signed). Then result |= 0x4000 whatever the freeze test gave (it
    follows the clause, not inside it), on every charge that reached
    the roll.
  - 4 `0x005D3970`: `roll_elemental`; `EType` = 4, n = 3 and `Param5` ≠
    0 → freeze length += cold length / `Param5`; result |= 0x4000
    (always, as in 3); p =
    `eval(calc1)` > 0 → p = min(p, 100); c = min(pct(physical, p, 100),
    physical); physical −= c; `0x0056C8E0(unit, record, EType, c, 0, 0,
    0)` adds c as that element (`levels.md` §3.6).
- **Finisher** `0x005D5220(game, unit, record)`: record null or no hit
  → nothing. T none → nothing. Not in melee range
  (`0x00622C40(unit, T, 0)`) → nothing. Unit flags |= 0x40. For each
  pgsv state the unit has, with a list P:
  1. Record of k = P[350] missing → skip to step 6.
  2. lvl as above; lvl ≤ 0 or `aurastat1` invalid → step 6.
  3. n = P[`aurastat1`] clamped to 1…3. Peek one draw of the unit's
     seed (save, step, restore; `rng.md` §3: the seed is unchanged) →
     r.
  4. With `prgstack` (flags +0x04 bit 7): for i = 1…n − 1: list set
     `aurastat1` := i; `srvprgfunc_i` (+0x30 + 2(i − 1)) in 1…190 with a
     non-null srvdo entry → call it (game, unit, k, lvl). Then list set
     `aurastat1` := n.
  5. `srvprgfunc_n` valid and non-null → call it. Queue message 0xA3
     on the unit (`0x00571AA0`: unit +0xEC/+0xF0 list, record {u8 n,
     u16 k, u16 lvl, unit type / GUID, T type / GUID, r, 0}).
  6. Detach and free P.
  After the loop all `pgsv` states are cleared at once (`0x0063A2D0` →
  `0x0063A180`: changed bits set, bits cleared, unit queued).

#### 2.15 Shape-shift attack start `0x005C8980`

`shape_start(game, unit, L)`: T none → 0. bonus = 0, s = 128. For each
`disguise` state (`runtime-maps.md` §4) the unit has, with a valid
state record and a list: k = list[350] valid → bonus += `to_hit(unit, k,
L)` (`levels.md` §5; L is the Attack level, not k's); k's `SrcDam`
(+0x1A5) ≠ 0 → s = it. Zeroed record; result = `melee_result(game,
unit, T, bonus, 0)`; hit class 1; `start_combat(game, unit, T, record,
s)`. Return 1.

#### 2.16 Attack-mode cleanup `0x00580310`, `0x00580380`

Both take ECX game, EDX unit and do nothing for a non-player or a player
without an inventory.

**Empty-hand refill** `0x00580310`:

1. Save the left skill (skill list +0x08, `0x00620190`), then the right
   skill (list +0x0C, `0x006201D0`), each as (skill id `0x00643CE0`,
   owner GUID `0x00643AD0`; id 0 and GUID −1 when none) (`0x005801E0`).
2. For the item at body location 4, then at 5 (`0x0063BDE0`), when
   present, `0x00580030(game, item)` (player in EBX):
   - an item that is neither a stack (`stackable` `0x006289F0`,
     `throwable` type `0x0062BA80`) nor has stat 125 `item_throwable` does
     nothing; one whose quantity (stat 70) > 0 does nothing; with stat
     125: stat 70 := 0, stop;
   - a weapon (item type 45) with `0x0062A0F0` true and not broken
     (item flag 0x100) is broken (`0x0055F850`, below), stop;
   - else (an empty stack): keep its class code (`0x0062E7E0`),
     `0x0062B400`, its body location (`0x00627D40`) and `0x0062E830`;
     the player's mode := 1; the item is removed (`0x00560CD0`) and
     `0x00628170(item, 1, 1)`. When the code is non-zero, the player's
     inventory is searched in order (`0x0063CD80`) for the first item
     of that code that is not broken and has the same class
     (`0x00451F60` vs item `+4`); if one exists, and the emptied body
     location is now free, it is equipped there (`0x00562A30(game,
     player, its GUID, location)`). When that does not happen and the
     `0x0062E830` value was non-zero: `0x0057FF70(game, location)`.

   Helpers: `0x0062A0F0(item)` = an item (type 4) whose quality (item
   data +0x00) is 4…9 (magic or better); so a normal or superior empty
   throwing weapon is used up, a magic or better one breaks.
   `0x0062E830(item)` = the item type's `reequip` (itemtypes +0x12,
   u8; 0 for a non-item or invalid type). `0x0057FF70(game, location)`
   (player in EDI): the item whose GUID is in player data +0x90 (the
   "re-equip" item); if it exists, the player has an inventory, the
   item's +0x45 byte is 0, it belongs to that inventory (item data
   +0x5C) and the placement test `0x0055D710(player, item, &location,
   0)` succeeds: the location must be free (occupied → fatal error),
   and the item is equipped there (`0x00562A30`). In every case player
   data +0x90 := 0 (`0x006233A0(player, none)`).
3. Restore the saved skills, **left then right** (`0x00580280`): a
   saved skill whose id ≠ 0, that still exists (`0x006439B0(player, id,
   owner)`), differs from the current skill of that side (left
   `0x00620190`, right `0x006201D0`) and whose `0x00647960` kind is
   neither 2 nor 7 is selected again (`0x005701B0(player, EDX side, id,
   owner)`: EDX 1 sets the left skill, EDX 0 the right; `bodies-3.md` §2
   answer 10).

**Break zero-durability weapons** `0x00580380`: walk the item list from
its head; an item with node kind 3 (equipped, `0x0063E020`), item type
45, breakable (`0x00629930`: `nodurability` = 0, `durability` > 0, max
durability ≠ 0, stat 152 `item_indesctructible` ≤ 0), durability (stat 72) ≤
0 and not broken: the first time, the player's mode := 1; the item is
broken (`0x0055F850`) and the walk restarts from the head.

**Break** `0x0055F850(game, unit, item)`: item flag 0x100 set
(`0x006280D0`); stats unlinked (`0x0063D2B0`); an item in mode 1 is
deactivated (`0x0055C730`); inventory pass (`items/inventory.md` §5.7,
send 0); `0x0063CC70`; unit update flag (`0x00621000`); stat 72 := 0
and message 0x3E (stat 72 = 0) to the owner's client; then
`0x00663CC0`, `0x00553380` (client refresh).

#### 2.17 Pet-maximum resync `0x00575900`

ECX game, EDX unit; only a player with a pet list (player data `+0x44`)
and a skill list (unit `+0xA8`):

1. A table `m[t]` per pet type (zeroed). For each skill of the list in
   list order (`0x00643910`, next `0x006438F0`): `0x006442A0(unit,
   skill, 1)`; when its `skills` `pettype` (`+0xBE`) is a valid pet
   type `t` > 0: `v = max(1, eval(petmax, skill, level))` (`+0xC0`,
   `0x00646CA0`); if `v > m[t]`: `m[t] := v` and `set_max(t, v)`.
2. Then for every pet type `t` (0 … count − 1) with `m[t] ≤ 0` and a
   `pettype` row: `set_max(t, basemax)` (`+0x0A`).

`set_max(t, v)` = `0x00575850(game, player, t, v)`: for `t` = 1 only
`v` = 1 is applied. The pet list entry `t` (12 bytes: count `+4`,
maximum `+8`) gets maximum := `v`; then, unless `t` = 7, while count >
`v` and count > 0, one pet of type `t` (`0x005747B0(player, t, 1)`) is
removed (`0x005750E0`). No message is sent by the resync itself.

#### 2.18 Event iteration `0x005C0C30`

`run(ECX game, EDX event, unit, a, b)`: nothing when the unit is null.
Walk the handler list (§2.13) from its head; for each record whose event
byte equals `event`: flags |= 1 (running); `r = func(ECX game, unit, a,
b, skill, level)`; flags &= ~1; the next record is read **after** the
call; then, when flags bit 2 is set (unregistered while running) **or**
the key type (`+0x04`) is 0, the record is unlinked and freed (so
key-type-0 handlers run once). Returns the `r` of the last matching
handler, 0 when none matched.

### 3. Start functions (srvst)

#### 3.1 1 Attack, Left Hand Swing `0x0056CA40`

1. Unit +0x44 (animation frame, 8.8) := frame bonus `0x00623B10(unit)`
   << 8 (`units.md` §4.3).
2. Group 38 (`meleeonly`: states 139 wolf, 140 bear) → return
   `shape_start(game, unit, L)` (§2.15).
3. `is_bow` and not `has_ammo` → attack-mode cleanup `0x00580310(game,
   unit)`, `0x00580380(game, unit)` (§2.16); return 0.
4. Return 1.

#### 3.2 2 Kick `0x0056CAF0`

T none → 0. Zeroed record: hit flags 2 (no roll), result 9 (hit,
knockback), physical = `MinDam` (+0x1A8) << `HitShift` (+0x1A4), hit
class 1. `start_combat(game, unit, T, record, 128)`. Return 1. (An
invalid skill reads a null record: fatal in the original.)

#### 3.3 3 Unsummon `0x0056CBA0`

Return 1 only when: T is a monster; the unit is a player; T's minion
owner (`0x0058F0D0`) is the unit; T's pet type in the player's pet
lists (`0x00574A20(unit, T GUID)`, `sim/pets.md` §9) has a pettype record with
`unsummon` (+0x04 bit 3); the used skill entry exists — then entry
param 1 (+0x18, `0x00644560`) := T's GUID. Otherwise 0.

#### 3.4 4 Arrow/Bolt `0x005DA8B0`, 65 Throw `0x0056CAB0`

Identical bodies: `has_ammo(unit)` → 1; else attack-mode cleanup
(`0x00580310`, `0x00580380`) and 0.

#### 3.5 5 Jab `0x005DA8F0`

R invalid → 0; return 1 if T exists, else 0.

#### 3.6 15 Raise Skeleton, Raise Skeletal Mage `0x005C3070`

T none, or T's room in town → 0. Return `0x00645510(T, 0)`: T is a
monster in mode 12 (dead), has no state of group 33 (`udead`), its
monstats2 has `corpseSel` (`0x004638A0(class, 7)`), and its monstats
`Velocity` (+0x32) ≠ 0.

#### 3.7 29 Sacrifice `0x005CE790`

1. R invalid, no used skill entry, T none, or T not in melee range
   (`0x00622C40(unit, T, 0)`) → 0.
2. Zeroed record; result = `melee_result(game, unit, T, to_hit(unit,
   skill, L), 0)`.
3. Hit (result & 1): physical = `bonuses(unit, get 1, item none, 0, 0,
   pct = eval(calc1), 0, s = SrcDam)` (`combat/damage.md` §3.2; SrcDam 0
   is passed as 0). `EType` ≠ 0 and c = `eval(calc4)` > 0 → conversion
   % (+0x68) = c, conversion element (+0x65) = `EType`.
   `roll_elemental(unit, record, skill, L)`. Result |= `ResultFlags`
   (+0x12E); hit flags = `HitFlags` (+0x130) | 1.
4. `start_combat(game, unit, T, record, 128)` (also on a miss). Return
   1.

#### 3.8 32 Bash, Stun, Concentrate, Conversion, BearSmite `0x005D7EA0`

1. R invalid, T none, or T's room in town → 0.
2. `0x0056E520(unit, eval(calc3))`: a list (pool 0, flags 4, expire 0,
   owner the unit) attached with `attackrate(68)` := the value, anim
   refresh. The list has no state; it is TEMPONLY, so it goes at the
   unit's next mode change (`sim/stat-lists.md` §8.9).
3. Zeroed record; result = `melee_result(game, unit, T, to_hit(unit,
   skill, L), 0)`.
4. Hit: result |= `ResultFlags`; hit flags |= `HitFlags`; `HitClass`
   (+0x134) ≠ 0 → hit class := it; enhanced damage % (+0x0C) :=
   `eval(calc1)`; `EType` ≠ 0 and `eval(calc4)` > 0 → conversion as in
   §3.7; `roll_elemental`.
5. `start_combat(game, unit, T, record, SrcDam or 128 when 0)`.
6. p = `pair_record(unit, T)`; p → p.physical += `eval(calc2)` << 8
   (after the roll and the "will die" test).
7. `aurastate` (+0x80) valid (≥ 0, < count): its list on the unit, or a
   new one (flags 4, expire 0, owner the unit, state set, attached,
   callback `0x0056E900`); state on; `aura_fill(unit, list, R, skill,
   L)`.
8. Return 1.

#### 3.9 33 Find Potion, Grim Ward `0x005D80C0`

T none → 0. v = `0x00645590(T)`: T a monster in mode 12, no group-33
state, monstats2 `corpseSel` and `soft` (`0x00621F60`) → 1, else 0. v
≠ 0 and T has state 118 (`corpse_noselect`) → 0. Return v.

#### 3.10 46 AndrialSpray `0x005CB4D0`

T none or no used skill entry → 0. Entry param 1 (+0x18) := T's x,
param 2 (`0x006445A0`) := T's y (position as in §1). Return 1.

### 4. Do functions (srvdo)

#### 4.1 1 Attack, Left Hand Swing `0x0056F070`

1. Unit flags |= 0x40.
2. Weapon (`0x00535BC0`) of item type 38 (missile potion,
   `0x0062B400`) → return 0.
3. `is_bow`: m = `bow_missile(unit, &L)` (L replaced by the arrow
   stat for magic / exploding arrows); `skill_missile` straight
   (`0x0056ECB0`) with (m, unit, skill, L, 0, 0, 0, 0, quant = 0 when m
   = 27, else 1). Return 1 (whatever the creation gave).
4. T none → 0.
5. Zeroed record; result = `melee_result(game, unit, T,
   progressive_tohit(325) of the unit, 0)`; hit flags |= 2.
6. Before-roll charges `0x005D3AC0` (§2.14); `fill(game, unit, T,
   record, 0, 128)` (`combat/damage.md` §3.1); after-roll charges
   `0x005D3BA0`; `start_combat(game, unit, T, record, 128)` (its own
   `fill` is skipped by hit flag 2).
7. Group 38 (wolf / bear): return `0x0056C2D0(game, unit, L)` — flags
   |= 0x40, T none → 0, `apply_melee(game, unit, T)`, 1.
8. Else p = `pair_record(unit, T)`; p → finisher `0x005D5220(game, unit,
   p)` (§2.14). `apply_melee(game, unit, T)` (`combat/damage.md` §5.1;
   a second stack argument 0 is unread). Return 1.

1.14d differs from D2MOO 1.10f: the wolf / bear path applies the melee
at once instead of only draining durability.

#### 4.2 2 Kick, Power Strike, … `0x0056F1F0`

Skills: Kick, Power Strike, MonIceSpear, Impale, Bash, Stun,
Concentrate, BearSmite, Vengeance, Berserk, Fire Claws, MonPowerStrike
(`functions.tsv`).

1. R invalid → 0.
2. `srvoverlay` (+0x4E) in 1…overlay count (inclusive; Edge case 3): T
   none → 0; p = `pair_record(unit, T)` none → **return 1** (the rest is
   skipped); p hit → overlay on T (`0x00621E40(T, srvoverlay, 0)`).
3. `auratargetstate` (+0x82) valid: T none → 0; p none → return 1; p
   hit: `len = max(eval(auralencalc), 1)`, e = F + len. List of the
   state on T, or a new one (game pool +0x1C, flags 2, expire e, owner
   T's type / GUID; failure → return 1). Then (both cases) timer 12 at e on
   T; state on (T); set state id; attach to T; callback `0x0056E900`;
   `aura_fill(T, list, R, skill, L)` (formulas on T). An existing list
   keeps its old expiry.
4. `aurastate` (+0x80) in 1…count − 1: list on the unit, or a new one
   (flags 4, expire 0, owner the unit, state set, attached, callback
   `0x0056E900`; failure → return 1); state on; `aura_fill(unit, …)`.
5. Unit flags |= 0x40; T none → 0; `apply_melee(game, unit, T)`;
   return 1.

#### 4.3 18 Defensive buff `0x005C9480`

Skills (11): Frozen Armor, Shiver Armor, Chilling Armor, Bone Armor,
Cyclone Armor, Fade, Holy Shield, Quickness, Venom, MonBoneArmor,
MonFrozenArmor.

1. R invalid or `aurastate` invalid (< 0 or ≥ count) → 0.
2. Unit flags |= 0x40.
3. `clear_group(unit, aurastate, 1)` (§2.9): every state of the same
   `group`, this one included, is removed first.
4. `apply_state` {source = target = unit, skill, L, duration =
   `eval(auralencalc)`, stat −1, state `aurastate`, callback
   `0x005C9420`} → list; none → 0.
5. `aura_fill(unit, list, R, skill, L)`; `passive_fill(unit, list, R,
   skill, L)`; list set 350 := skill, 351 := L.
6. `auraevent1` (+0x84) ≥ 0: unregister (1, `aurastate`); for i = 1…3
   while `auraevent_i` ≥ 0: register (event_i, skill, L,
   `auraeventfunc_i` (+0x8A + 2(i − 1)), 1, `aurastate`) (§2.13).
7. Mark `aurastate` changed. Return the list (non-zero).

#### 4.4 30 Curse `0x005C37C0`

Skills (10): Amplify Damage, Dim Vision, Weaken, Iron Maiden, Terror,
Life Tap, Decrepify, Lower Resist, Blood Mana, Defense Curse.

1. R invalid, `aurastat1` < −1 or ≥ itemstatcost count, or
   `auratargetstate` invalid → 0.
2. Unit flags |= 0x40.
3. ai = 1 when `auratargetstate` is 23 (`dimvision`) or 56 (`terror`)
   (`0x005C3400`), else 0.
4. r = `eval(aurarangecalc)`; d = `eval(auralencalc)`. ai → d = d /
   `AiCurseDivisor` (difficultylevels +0x1C of the game's difficulty;
   skipped when 0; 1.14d: 1, 2, 4).
5. Context (0x68 bytes, zeroed first): game, unit, ai, upd, skill, L,
   d, stats[6], values[6], state = `auratargetstate`, `auraevent1–3`,
   `auraeventfunc1–3`. For i = 1…6: `aurastat_i` < 0 or ≥ count → stat
   −1, stop; no itemstatcost record → stop; record has `updateanimrate`
   (+0x04 bit 9) → upd = 1; stat = it, value = `eval(aurastatcalc_i)`.
   Slots after a stop (and slot i after a no-record stop) keep the
   zeroing: stat 0, value 0. Per unit, stat 0 is "valid" and
   `scaled(U, 0, 0)` = 0, so such a slot sets nothing (step 6); in slot
   1 it makes v1 = 0 and the unit is skipped (step 2).
6. Return `scan_point(game, aurafilter, unit, r, 0x005C35C0, context)`
   (§2.12).

Per unit U (`0x005C35C0`, ECX U, EDX context):

1. ai: U must be a monster, its alignment (`0x006259B0`) ≠ 1, and
   `can_switch(U, k)` (k = 10 for state 23, 11 for 56; Open question 7);
   else 0.
2. v1 = 0; stat1 ≥ 0 → v1 = `scaled(U, stat1, value1)` (§2.10), and
   v1 = 0 → 0. stat1 = −1 (no `aurastat1`) goes on with v1 = 0.
3. `0x005C3420(game, unit, U)`: U non-null; a monster U needs a walk
   mode (`0x0046C140(class, 2)`) and no type flag 0x20 (possessed,
   `0x005A0180`); U flags +0xC4 bits 0x4, 0x8 and 0x2 all set; U alive;
   hostile (`0x00554200`). Fails → 0.
4. `apply_state` {source unit, target U, skill, L, d, stat1, v1, state,
   callback ai ? `0x005C3370` : default} → none → 0.
5. ai → install the curse AI (`0x005C34B0(game, unit, U, skill, L)`:
   `0x005B0E00(game, U, U's AI control or none, k)`, `monsters/ai.md`
   §3.3).
6. Stats 2…6 valid → v = `scaled(U, stat, value)`; v ≠ 0 → list set.
7. upd → anim refresh of U.
8. `auraevent1` ≥ 0 and `auraeventfunc1` > 0: unregister (1, state) on
   U; then **three times**, while event 1 ≥ 0 and func 1 > 0 (always
   the first pair): register (event_i, skill, L, func_i, 1, state) on U
   (Edge case 4).
9. Return 1.

`can_switch(U, k)` = `0x0056E2F0`: a monster whose base class
(monstats +0x02) is 492 and which has state 143 (`attached`) → 0;
else `0x005DD480(U, k)`: U a monster, k ≤ 19, no state 54, its class
switch-capable (`0x00623470`: has a walk mode, not `boss`, `switchai`),
(unit flags 0x4 set or U alive), no type flag 0x2 or 0x8 (superunique,
unique); k = 19 → 1; else `0x005B0DA0(U, monstats record of U's
class, k, 1)`: k ∈ {10, 11, 12} → not superunique (type flag 2) and
switch-capable; other k → 1. The monstats argument is never read (the
register is overwritten before any use; the class is re-read from U).
The per-unit test of step 1 computes k = 10 for state 23, 12 for 27, 11
for 56, else 0, but only runs when ai = 1 (states 23 and 56): k = 12 is
never used. Live 1.14d: 543 of 734 monstats rows are switch-capable
(`switchai` 1, `boss` 0, monstats2 mode WL); Terror and Dim Vision
switch exactly those classes, minus units that are unique or
superunique, have state 54, base class 492 with state 143, or fail
step 3.

#### 4.5 65 Basic aura `0x005CF010`

Skills (17): Might, Prayer, Resist Fire, Resist Cold, Resist Lightning,
Thorns, Defiance, Blessed Aim, Cleansing, Concentration, Vigor,
Meditation, Fanaticism, Salvation, Barbs Aura, Wolverine Aura, Oak Sage
Aura; run by the aura timer and the immediate run (`use.md` §7).

1. R invalid → 0.
2. cost = `0x00644B10(skill, L)` (`levels.md` §4, no level clamp);
   mana = the unit's `mana(8)` (unit getter).
3. Context (0x50 bytes, zeroed): state, skill, L, duration = `period(…)`
   − F + 1 (`use.md` §7 `0x0056CD50`), stats[6], values[6], count (+0x40),
   list (+0x44), passivestate (+0x48) = `passivestate` (+0x94) when > 0,
   remove callback (+0x4C) = 0.
4. Not a player, or mana ≥ cost: for i = 1…6: `aurastat_i` < 0 → stop;
   stat i = it, value i = `eval(aurastatcalc_i)` (on the caster).
   A player with mana < cost keeps all six stats 0 (nothing is applied,
   but state lists are still created or refreshed).
5. `aurastate` (+0x80) in 1…count − 1: state = it; run the callback on
   the unit itself (`0x0056B740`: context {game, unit, 0, arg}). Then if
   the context list is set, mana > cost (strict) and `passivestate` ≤
   0: for i = 1…5: v = `eval(passivecalc_i)`; v ≠ 0 and `passivestat_i`
   in 1…count − 1 → list set.
6. `auratargetstate` (+0x82) in 0…count − 1: state = it; `scan_unit(game,
   unit, 0, 0, eval(aurarangecalc), aurafilter, callback, context,
   noaura = 1)` (§2.12). Then, for a player with cost > 0: count > 0 →
   state 85 on and `0x0056C110(unit, cost)` (players only; blood mana
   state 114 → `0x005D2B60`, `levels.md` §4 `pay_with_life`; mana < cost
   → nothing; else mana −= cost (`0x006272B0`); result ignored); count
   = 0 → state 85 off.
7. Return 1.

Callback `0x005CEDC0` (ECX scan context, EDX unit U):

1. `apply_state` {source = context unit, target U, skill, L, duration,
   stat −1, state; callback: context +0x4C, else `0x005CEC50` when U is
   the source, else default}. None → 0. Context list := it.
2. For i = 1…6 with value_i ≠ 0 and stat_i in 1…count − 1:
   - stat 110 (`item_poisonlengthresist`): `0x005CEC90(game, U, v)` → 1
     → count += 1. It scales the remaining time of U's state 2
     (`poison`) list and of each `curse` state U has that is `curable`
     (group 12): new expiry = F + pct(expiry − F, v, 100), timer 12 at it;
     returns 1 when U has a poison list or any curse state with a list
     (curable or not).
   - Else v' = `scaled(U, stat, v)`; v' = 0, or no itemstatcost
     record → next.
   - Stat not `direct` (+0x04 bit 4): if the list's value ≠ v', mark the
     state changed on U. Stats 67 / 68: v' = max(v', floor), floor =
     `ColdEffect` of U's monstats for the difficulty (+0x168 +
     difficulty, i8) or −50 for non-monsters (`0x0057AF30`). List set
     stat := v' (68 → also 69). count += 1.
   - `direct`: b = U's stat (unit getter); w = b + v'; capped at U's
     value of the stat's `maxstat` (+0x32) when that is valid; w ≠ b →
     set U's stat := w (`0x00627260`), count += 1.
3. Anim refresh of U; list set 350 := skill, 351 := L.
4. passivestate > 0: U's list of it → detach and free, mark it changed.
5. Return 1.

Context fields after a run: count adds the self call and every scanned
unit; list = the last list made (the self list right after step 5).

### 5. `srvmissile` path

`use.md` §5.4 step 7 in full (do core `0x0056F7F0`, `0x0056F96D`–
`0x0056FA3B`):

1. `srvmissile` (+0x46) ≥ 0 with a missiles record (`0x0046ACE0`);
   else skip.
2. Unit flags |= 0x40.
3. dx = dy = tx = ty = 0. `item ≠ 0` and `aim ≠ 0`: target position
   (`0x0056D2C0`); both coordinates non-zero → tx, ty = it; dx = tx −
   unit x, dy = ty − unit y; tx += dx, ty += dy.
4. `lob` (flags +0x04 bit 1) → `0x0056EE90`, else `0x0056ECB0`, with
   (srvmissile, unit, skill, L, dx, dy, tx, ty, quant = 0) (§2.4).
5. r = 1 whatever the creation gave. Ammunition is spent by the core's
   `decquant` step (§2.5).

### 6. Shared helpers, batch 2

Bodies of §7–§8 were picked by use count (Constants, "Batch 2 order").

#### 6.1 Summon class `0x0056E620`

`summon_class(unit, skill, L, &mode)` (ECX unit, EDX 0; stack skill, L,
&mode, 0, 0; D2MOO `D2GAME_GetSummonIdFromSkill_6FD15580`) calls
`0x0063EA70(unit, 0, skill, L, &mode, null, null)`:

1. With EDX = 0 (every caller here) the skill path runs: R invalid →
   −1. c = `summon` (+0xBC, i16); c < 0 or ≥ monstats count → −1. mode
   := `summode` (+0xBF, signed byte); outside 0…15 → 1. Return c.
   (EDX ≠ 0 and a monster unit read the monstats `spawn` columns
   instead, `monsters/population.md` §14.)
2. Back in `0x0056E620`: c valid (0 ≤ c < monstats count) → c. Else c'
   = monster data +0x14 → AI control +0x28 → +0x3C (`nMinionSpawnClassId`,
   `monsters/ai.md` AI control fields) of the unit (`0x0058F710`; a non-monster
   reads through a null pointer: fatal); 0 < c' < count → c'; else −1.

#### 6.2 Summon spawn `0x0056D940`

Request (ECX game, EDX request; D2MOO `D2SummonArgStrc`): +0x00 flags,
+0x04 owner, +0x08 monster class, +0x0C AI special state, +0x10 mode,
+0x14 x, +0x18 y, +0x1C pet type, +0x20 pet max. Flags: 1 position
given, 2 replace the owner's linked unit, 4 no second try, 8 keep unit
flag 0x80000000 clear.

1. Flags & 1 → (x, y) from the request; else target position of the
   owner (`0x0056D2C0`, §2.4; its result is not tested).
2. Room = the room containing (x, y), searched from the owner's room
   (`0x00620BB0`, `0x00463740`); none → none.
3. m = `0x005B2F20(game, room, x, y, class, mode, spread −1, flags
   0x42)` (`monsters/init.md` §1). None: flags & 4 → none; else m =
   the same call with spread 4; none → none.
4. Finish `0x0056D8D0`: flags & 8 clear → m flags (+0xC4) |= 0x80000000.
   Flags & 2: `0x0056D840` — the owner's linked unit (GUID `0x00554070`,
   looked up as a monster `0x00552F60`) if any: its +0xC8 bit 8 set →
   removed (`0x00555600`); else flags |= 0x4000000 (no experience) and
   `0x0057CCB0(it, 1)`; then the link := −1 (`0x00554040`); then the
   link := m's GUID. Last: pet list add `0x00575D90(game, owner, m, pet
   type, max(pet max, 1))` (players only; `sim/pets.md` §2).
5. m flags |= 0x20000 (no drop).
6. Owner data `0x0058F030(game, m, owner GUID, owner type, 0, 0)` (no
   owner: GUID −1, type 6).
7. m has state 54 (`uninterruptable`) → fatal assertion.
8. AI `0x005B0E00(game, m, m's AI control, request AI special state)`
   (`monsters/ai.md` §3.3); `0x00573780(game, m)`; delete m's type-2
   timers (`0x00540E60(game, m, 2, 0)`); type-2 timer at F + 25
   (`0x005417D0`). Return m.

#### 6.3 Target-node insert `0x005B1900`

`node_insert(game, m, 0, slot)` (ECX game, EDX m): slot = the owner's
node index (unit +0xD0). Nothing unless m +0xD0 = 11 (in no list),
slot < 8, m is a player or monster, and the game's list `slot` (game
+0x10F8 + 4·slot, `monsters/ai.md` §5.2) has a head with a unit. Then a
0x10-byte node {unit m, 0, next, prev} is linked right after the head
and m +0xD0 := slot. Effect: monster AIs see the summon with its owner.

#### 6.4 Summon base stats `0x005C49E0`

`base_stats(game, owner, m, p, L)` (ECX game, EDX owner; `ret 0xC`):

1. p ≤ 0: c = owner `level(12)` (unit getter); p = L + (3c) / 4
   (signed, truncating); p < 1 → 1; p ≥ c → c (so c ≤ 0 gives p = c).
2. p ≠ 0 → m stat 12 := p (`0x00627260`).
3. i = min(p, monlvl count − 1); not 0 ≤ i < count → return 0.
4. d = difficulty (game +0x6D), capped at 2; e = 1 when game +0x6A ≠ 0
   or game +0x74 ≠ 0, else 0. m stat 31 (`armorclass`) += monlvl[i]
   `AC` column d of the classic (e = 0) or `L-AC` (e = 1) group; m stat
   19 (`tohit`) += `TH` / `L-TH` likewise (`0x006272B0`; `fields.tsv`
   monlvl). Return 1.

#### 6.5 Summon skill stats `0x005C4470`

`skill_stats(game, owner, m, skill, L, ilvl)` (ECX game, EDX owner; `ret
0x10`; D2MOO `D2GAME_SetSummonPassiveStats_6FD0C530`). Formulas are
evaluated on the owner with (skill, L).

1. skill = 0 → return 0.
2. For i = 1…5: s = `passivestat_i` valid (0 ≤ s < itemstatcost count):
   m stat s += `eval(passivecalc_i)` (`0x006272B0`). Stat event: s's
   `itemevent1` (itemstatcost +0x48) > 0 and m has no handler with
   (type 2, key m GUID, skill field = s) (`0x005C0BE0`) → register (§2.13)
   (`itemevent1`, skill field s << 16, level 0, `itemeventfunc1` (+0x4C),
   type 2, key m GUID); `itemevent2` (+0x4A) > 0 → also (`itemevent2`,
   s << 16, 0, `itemeventfunc2` (+0x4E), 2, m GUID).
3. List = none. For i = 1…6: s = `aurastat_i` valid; v =
   `eval(aurastatcalc_i)`; v ≠ 0: no list yet → alloc (game pool +0x1C,
   flags 0, expire 0, m type, m GUID; failure → return 0) and attach to
   m; list set s := v; the stat event step of 2.
4. `aurastate` (+0x80) in 1…states count (count itself accepted): m state
   on; list → its state id := it.
5. h = m's maximum life (`0x00625D10`); h' = h + `pct(h,
   eval(calc1), 100)` (`combat/damage.md` §0, inlined); m stat 7
   (`maxhp`) := h', stat 6 (`hitpoints`) := h'.
6. For i = 1…5: k = `sumskill_i` (+0xC4) in 1…skills count − 1, v =
   `eval(sumsk_i_calc)` (+0xD0) > 0: `set_skill(m, k, v)` (`0x0056DEB0`:
   the entry of k (owner −1, `0x006439B0`), added when missing
   (`0x00647110`); base level (+0x28) := v; `0x00646D60(m, k)`; passive
   refresh `0x00646F20(m)`; a player also `0x00575900`); k has `aura`
   (flags bit 5) → right skill := k (`0x005701B0(m, 0, k, −1)`,
   `use.md` §7).
7. `auraevent1` ≥ 0: unregister (1, `aurastate`) on m; for i = 1…3 while
   `auraevent_i` ≥ 0: register (`auraevent_i`, skill, L,
   `auraeventfunc_i`, 1, `aurastate`) on m.
8. `sumumod` (+0xE4) in 1…42 → `0x005A4850(game, m, sumumod, 1)`
   (`monsters/init.md` umods). `sumoverlay` (+0xE6) in 1…overlay count −
   1 → overlay on m (`0x00621E40(m, it, 0)`).
9. ilvl = 0 → ilvl = 3L, < 1 → 1, ≥ owner `level(12)` → that level.
   Equipment `0x005D6B60(game, owner, m, skill, L, ilvl, 0)`
   (`monsters/init.md` §12 with: no `oninit` test (last argument 0),
   rows skipped while `level` > L, items at item level ilvl, and an
   item code of four spaces copies the base code of the owner's item at
   that location when the owner has one there (`0x00628590`); draws on
   m's seed). Return 1.

#### 6.6 Progressive missile `0x005D3CF0`

`prog_missile(unit, skill)` (ECX unit, EDX skill): R invalid → −1. R
`progressive` (flags bit 2), `aurastate` and `aurastat1` valid and the
unit has a list of `aurastate` (`0x006256B0`): n = list[`aurastat1`]; n
≥ 2 → the missile column `srvmissile` + min(n, 3) (`srvmissileb`,
`srvmissilec`); else `srvmissilea`. Every other case: `srvmissilea`
(+0x48).

#### 6.7 Missile ring `0x0056D400`

`ring(game, owner, at, m, skill, L, v)` (ECX game, EDX owner; `ret
0x14`): record (§R2.1, zeroed) flags 3 (position given, target
relative), owner, class m, position of `at` (§1), skill, L; v ≠ 0 →
flags |= 4, velocity := v. For i = 0…63 in order: target offset (x_i,
y_i); create (`0x0059FA30`, result ignored). With c = 30, 29, 29, 28,
27, 26, 24, 23, 21, 19, 16, 14, 11, 8, 5, 2, 0 (c_j, j = 0…16; table
`0x006E1288` / `0x006E1388`, = trunc(30 cos(2πi/64))): x_i = c_i (i ≤
16), −c_(32−i) (16…32), −c_(i−32) (32…48), c_(64−i) (48…63); y_i =
x_((i − 16) mod 64).

#### 6.8 Shout state `0x005D8290`

`shout_state(game, T, src, skill, L)` (ECX game, EDX T; `ret 0xC`):

1. R invalid or `aurastate` not in 0…count − 1 → 0.
2. e = F + `eval(src, auralencalc)`.
3. T's list of `aurastate` exists → keep it. Else alloc (game pool,
   flags 2, expire e, owner src type / GUID; no src → 6 / −1; failure →
   0), set state, callback `0x0056E900`, attach to T, T state on.
4. Mark `aurastate` changed on T; expiry := e (`0x006260B0`); timer 12 at
   e on T; `aura_fill(src, list, R, skill, L)` (§2.6, formulas on src);
   passive refresh `0x0056DE40(T)`. Return 1.

A recast refreshes the expiry of an existing list (unlike §4.2 step 3).

#### 6.9 Sentry spawn `0x005D5E10`

`sentry(game, unit, x, y, R, skill, L)` (ECX game, EDX unit; `ret
0x14`; D2MOO `sub_6FCF8610`):

1. c = `summon_class(unit, skill, L, &mode)` (§6.1); invalid → 0.
2. pt = `pettype` (+0xBE, signed); outside 0…pettype count − 1 → pt =
   0. pm = `eval(petmax)` (+0xC0).
3. x = 0 or y = 0 → (x, y) := the unit's position.
4. While the unit is a monster: unit := its minion owner
   (`0x0058F0D0`); none → 0. (A monster caster lays the trap for the
   player that owns it.)
5. x = 0 or y = 0 → target position of that owner (§2.4); failure → 0.
6. Room containing (x, y) from the owner's room; none → 0. R lacks
   `InTown` (flags bit 8) and that room is in town (`0x0061AB00`) → 0.
7. Spawn (§6.2) {flags 1, owner, class c, AI state 0, mode, x, y, pt,
   pm}; none → 0.
8. `base_stats(game, owner, m, 0, L)` (§6.4: level from the owner);
   `skill_stats(game, owner, m, skill, L, 0)` (§6.5).
9. Alignment `0x005543B0(m, 2, 1)` (`monsters/init.md`); monster mode
   change of m to `mode` (`0x005A7E60(m, mode, &req)` then
   `0x005A7C20(game, &req, 1)`, `monsters/ai.md`). Return m.

#### 6.10 Charge add `0x005D3320`

`charge_add(game, unit, skill, L, s, c)` (unit in EBX; stack game,
skill, L, s, c; `ret 0x14`): s = the charge state (`aurastate`), c = the
counter stat (`aurastat1`).

1. R invalid → 0.
2. e = F + `eval(auralencalc)`.
3. The unit's list of s exists → keep it. Else alloc (game pool, flags 2,
   expire e, owner the unit; failure → 0), set state s, remove callback
   `0x005D3310` (state s off, nothing else), attach, list set 350 :=
   skill, 351 := L.
4. Expiry := e (`0x006260B0`); timer 12 at e on the unit.
5. n = list[c]; n' = min(n + 1, 3). n' = n → return 1 (already 3).
6. List set c := n'. `aurastat2` (+0x56) valid → list **add** (`0x00627030`)
   `aurastat2` += `eval(aurastatcalc2)`. State s on; mark s changed.
   Return 1.

Every charging hit refreshes the expiry; the counter stops at 3.

#### 6.11 Golem stats `0x005C50C0`, summon resistance `0x005C40D0`

`golem_stats(game, owner, m, skill, L)`: `base_stats(game, owner, m, 0,
L)` (§6.4); `skill_stats(game, owner, m, skill, L, 0)` (§6.5);
`summon_resist(owner, m)`.

`summon_resist(owner, m)` (ECX owner, EDX m; D2MOO
`D2GAME_SetSummonResistance_6FD0C2E0`): r = owner
`passive_summon_resist(349)` (unit getter); 0 → nothing. Alloc a list
(m's pool unit +0x08, flags 0, expire 0, m type / GUID; failure →
nothing), attach to m. m `item_absorbfire_percent(142)` ≤ 0 → list set
39 := r; `item_absorblight_percent(144)` ≤ 0 → 41 := r;
`item_absorbcold_percent(148)` ≤ 0 → 43 := r; always 45 := r.

#### 6.12 Free target point `0x0056E450`

`point_free(game, unit, m)` (ECX game, EDX unit; `ret 4`): missile record
of m (none → fatal assertion). The unit's room none → 0. Target position
(§2.4); x or y = 0 → 0. Return 1 when the collision query
`0x0064D800(room, x, y, Size, Size, mask 5)` is 0 (`Size` missiles
+0x18A, u8; a Size ≤ 1 is a point query, else a Size × Size box,
`sim/path-placement.md` §4 rules 3–4), else 0.

#### 6.13 Missile at the target point `0x0056EDE0`

`missile_at(game, unit, skill, L, m, tx, ty)` (ECX game, EDX unit; `ret
0x14`): unit none → none. tx = 0 and ty = 0 → target position (§2.4);
still both 0 → none. Distance `0x006417F0(unit, tx, ty)` (max(|dx|,
|dy|) + min(|dx|, |dy|) / 2, `sim/pathing.md`) > 100 (unsigned) → none.
Record (§R2.1, zeroed): flags 1 (position given), owner the unit, (x, y)
= (tx, ty), class m, skill, L; no target. Return the creation's result.

#### 6.14 Paladin raise penalty `0x005C2FF0`

`raise_penalty(game, unit)`: only a player of class 3 (Paladin). h = max
life (`0x00625D10`); zeroed record: hit flags 0x1000, result 4, physical
(+0x08) = total (+0x4C) = h / 8 (signed, truncating); `apply(game, unit,
unit, 0, record)` (`combat/damage.md` §5.2); reaction `0x0057CEE0(game,
unit, unit, record)`.

#### 6.15 Skeleton components `0x005C4430`

`components(owner, m, skill, L)` (ECX owner, EDX m): by m's class: 363
(necroskeleton) → skeleton form, 364 (necromage) → mage form, other →
nothing.

1. lvl = 1; the owner has state 97 (`skel_mastery`) with a list → lvl =
   its stat 351. lvl > 10 → 10.
2. Skeleton: shield = 0; L > 2: p = `Param1` (+0x148) of the skill (0
   when invalid); one draw on the **owner's** seed, r = `lo' mod 100`
   (unsigned); r < p → shield = 1. Mage: shield = 0.
3. Set m's component bytes (monster data +0x04 + k; k = 0 HD, 1 TR, 2 LG,
   3 RA, 4 LA, 5 RH, 7 SH, 8 S1, 9 S2, 11 S4, 12 S5) from row lvl of the
   9-byte table `0x00741940` (t0…t8): HD := t0, TR := t2, S1 := t3, S2
   := t4, LG := t5, RA := t6, LA := t7; shield → SH := t1.
4. Mage: one draw on the owner's seed, c = `lo' & 3`; S4 := c, S5 := c;
   AI params 0 := 1, 1 := 0 (`0x005B0D70(control, 1, 0, −666)`; −666 =
   unchanged). Skeleton: RH := t8.

Table rows lvl 0…10 (t0…t8): 0–1 all 0; 2 (0,1,0,0,0,0,0,0,1); 3
(0,1,0,0,0,0,0,1,1); 4–5 (1,1,0,0,0,0,1,1,2); 6 (1,1,1,0,0,1,1,1,3); 7
(2,2,1,0,0,1,1,1,3); 8 (2,2,1,0,0,1,1,2,4); 9–10 (2,3,1,0,0,1,2,2,4).

#### 6.16 Inferno start `0x005C8E30`

`inferno_start(game, unit, skill, L, m)` (ECX game, EDX unit; `ret
0xC`; D2MOO `SKILLS_StartInferno`):

1. R invalid → 0.
2. A monster: used entry param 1 := F + max(`eval(calc2)`, 1).
3. The unit has a list of state 12 (`inferno`): rewind `0x00553C70(game,
   unit, 1)` (`sim/units.md` §4.2 variants); expiry := F + 6; timer 12
   at F + 6; `inferno_do(game, unit, skill, L, m)` (§6.17); return 1.
4. Else: alloc (game pool, flags 2, expire F + 20, owner the unit;
   failure → 0); timer 12 at F + 20; attach; remove callback `0x005C8BF0`
   (state off, then unit flags |= 0x40); state 12 set and on; used entry
   param 1 := 0. Return 1.

#### 6.17 Inferno do `0x005C8CA0`

`inferno_do(game, unit, skill, L, m)` (D2MOO `SKILLS_DoInferno`):

1. R invalid, or not 0 ≤ m < missiles count → 0.
2. Target position (§2.4) → (tx, ty); failure → 0.
3. E = used skill entry; none → 0.
4. E param 1 ≠ 0: record (§R2.1, zeroed): flags 0x8020 (target absolute,
   range given), owner = origin = the unit, class m, target (tx, ty),
   skill, L, range = max(`eval(calc1)`, 1); create.
5. E param 1 := 1.
6. Not a monster: rewind `0x00553C70(game, unit, 1)`; return 1.
7. Monster: unit +0x44 := 0xB00 (frame 11). F < E param 1 (= 1) and
   state 12 on → delete type-1 timers (`0x00540E60(game, unit, 1, 0)`),
   type-0 timer at F + 2 args (4, 0). Else (`0x005C8C10`): state 12 off;
   delete type-0 timers; type-1 timer (ENDANIM) at F + d, d =
   monstats2 `InfernoLen` (+0x108) of the class, 1 without a record.
   Return 1.

The first call after a fresh start (param 1 = 0) creates nothing; every
later one creates one missile. Step 5 overwrites the monster timeout of
§6.16 step 2, so a monster always takes the "else" branch of step 7
(Edge case 16).

#### 6.18 Missile fan at the target `0x005C7040`

`fan(game, unit, n, m, skill, L, first)` (ECX game, EDX unit; `ret
0x14`):

1. first ≠ 0: `skill_missile(game, m, unit, skill, L, 0, 0, 0, 0, 0)`
   (§2.4, straight); n −= 1; n ≤ 0 → return 1.
2. Record (§R2.1, zeroed): flags 0x21 (position given, target
   absolute), owner the unit, position = the unit's (§1), class m,
   skill, L, init callback (+0x54) `0x005C9290` (re-seeds each missile,
   `missiles.md` §R2.3 step 21).
3. Target position (§2.4) into the record; failure → return 1.
4. For i = 0…n − 1: callback argument (+0x58) := i; create. Return 1.

#### 6.19 Shadow stats `0x005D6CF0`

`shadow_stats(game, R, skill, L)` (m in ESI; `ret 0x10`). Formulas are
evaluated on the **shadow** m.

1. L ≤ 1 → nothing.
2. p = `Param1` (+0x148; 0 for an invalid skill); h = m's maximum life;
   h' = h + `pct(h, (L − 1)·p, 100)`; m stat 7 := h', stat 6 := h'.
3. Alloc (game pool, flags 0, expire 0, m type / GUID; failure → stop),
   attach to m.
4. For i = 1…6: `aurastat_i` valid → list set it := `eval(m,
   aurastatcalc2)`. For i = 1…5: `passivestat_i` valid → list set it :=
   `eval(m, passivecalc2)`. (Always the second formula: Edge case 18.)
5. `sumumod` in 1…42 → `0x005A4850(game, m, sumumod, 1)`.

#### 6.20 Source-unit link `0x00621CE0`

`link_source(m, owner)`: owner given → m +0x94 := owner type, +0x98 :=
owner GUID (`0x00621C30`); if m has a stat holder (+0x5C): state 98
(`sourceunit`) on, its list (or a new one: m's pool, flags 0, expire 0,
m type / GUID; state 98, attached; failure → stop before the flag) gets
stat 353 (`source_unit_type`) := type and 354 (`source_unit_id`) :=
GUID; then m +0xC8 |= 0x400 (also without a holder). Owner none → both
fields 0; with a holder: state 98 off, its list detached and freed;
then +0xC8 &= ~0x400.

### 7. Start functions (srvst), batch 2

#### 7.1 23 Tiger Strike, Fists of Fire, Cobra Strike, Claws of Thunder, Blades of Ice, Royal Strike `0x005D32F0`

T none → 0. Return the melee range test `0x00622C40(unit, T, 0)` (1 in
range). Skill and level are not read.

#### 7.2 6 Power Strike, Charged Strike `0x005DA940`

Also MonPowerStrike, MonIceSpear.

1. T none → 0. R invalid → 0.
2. Zeroed record; result = `melee_result(game, unit, T, 0, 0)` (no skill
   to-hit bonus).
3. Hit: enhanced damage % (+0x0C) := `eval(calc1)`. `EType` ≠ 0: c =
   `eval(calc4)`; conversion % := c; c > 0 → conversion element :=
   `EType`. `roll_elemental(unit, record, skill, L)` (whatever `EType`).
4. `start_combat(game, unit, T, record, SrcDam)` (0 passed as 0). Return
   1.

#### 7.3 11 Inferno, Arctic Blast `0x005C8FA0`

1. R invalid → 0.
2. The unit lacks state 12 (`inferno`) and its `mana(8)` < `startmana`
   (+0x184) << 8 → 0.
3. m = `srvmissilea`; not 0 ≤ m < missiles count → 0.
4. Return `inferno_start(game, unit, skill, L, m)` (§6.16).

#### 7.4 12 Telekinesis, Dragon Flight `0x005C9030`

1. T none → 0. R invalid → 0.
2. r = `eval(aurarangecalc)`; d² = squared distance of the unit's and
   T's positions (`0x006492A0`); d² > r² (signed) → 0.
3. T a player or monster: hostile (`0x00554200(game, unit, T)`) and
   neither T's room nor the unit's room in town, else 0. Other T types
   (objects, items, missiles) skip these tests.
4. Return 1.

#### 7.5 17 Corpse Explosion, Poison Explosion `0x005C31C0`

Also NihlathakCorpseExplosion. T none → 0; T's room in town → 0. Return
`0x00645680(T)`: T a monster in mode 12, no `udead`-group state
(`0x0063A770`, as §3.6), monstats2 `corpseSel` (`0x004638A0(class, 7)`)
→ 1; else 0 (no `Velocity` test, unlike §3.6).

#### 7.6 37 Zeal, Fury `0x005DAF40`

Also BloodLordFrenzy.

1. R invalid → 0. E = used skill entry; none → 0.
2. r = melee range (`0x00622870`) + 4. T = target; none → T =
   `next_unit(game, unit, 0, 0, r, 0x20003, −1, null)` (§8.11 step 3;
   g = −1: the smallest GUID wins); none → E param 1 := 0, return 0.
3. E param 1 := `eval(calc1)` (the hit count for §8.11); param 2 := T
   type; param 3 := T GUID. Return 1.

#### 7.7 56 Feral Rage, Maul `0x005C7690`

1. R invalid → 0. E = used skill entry; none → 0. T none → 0.
2. Zeroed record; result = `melee_result(game, unit, T, to_hit(unit,
   skill, L), 0)`.
3. Hit: `EType` ≠ 0: c = `eval(calc4)`; conversion % := c; c > 0 →
   conversion element := `EType`. Enhanced damage % := `eval(calc1)`. (No
   `roll_elemental`.)
4. `start_combat(game, unit, T, record, SrcDam or 128)`.
5. E param 1 := 1 if the result has hit, else 0. Return 1.

### 8. Do functions (srvdo), batch 2

#### 8.1 119 Druid summon `0x005C7390`

Skills (6): Oak Sage, Summon Spirit Wolf, Heart of Wolverine, Summon
Fenris, Spirit of Barbs, Summon Grizzly.

1. R invalid → 0.
2. c = `summon_class(unit, skill, L, &mode)` (§6.1); < 0 → 0.
3. pt = `pettype` (+0xBE, signed byte); < 0 or ≥ pettype count → 0.
4. Unit flags |= 0x40.
5. Target position (§2.4) fails → 0 (the point is not used further).
6. m = spawn (§6.2) {flags 0, owner unit, class c, AI state 0, mode, x
   = y = 0, pt, pet max `eval(petmax)`}; none → 0.
7. `node_insert(game, m, 0, unit +0xD0)` (§6.3).
8. `base_stats(game, unit, m, max(eval(calc2), 1), L)` (§6.4).
9. `skill_stats(game, unit, m, skill, L, 0)` (§6.5). Return 1.

#### 8.2 68 Basic shout `0x005D83E0`

Skills (5): Shout, Battle Cry, Battle Orders, Battle Command, War Cry.

1. Unit flags |= 0x40 (also when the rest fails).
2. R invalid → 0.
3. m = `prog_missile(unit, skill)` (§6.6); not 0 ≤ m < missiles count →
   0.
4. `ring(game, unit, unit, m, skill, L, 0)` (§6.7): 64 missiles at table
   speed; the missiles carry the effect to others (`missiles.md`).
5. `shout_state(game, unit, unit, skill, L)` (§6.8) on the caster.
   Return 1.

#### 8.3 45 Sentry `0x005D6170`

Skills (5): Charged Bolt Sentry, Wake of Fire Sentry, Lightning Sentry,
Inferno Sentry, Death Sentry.

1. R invalid → 0.
2. Unit flags |= 0x40.
3. Target position (§2.4) → (x, y); failure → 0.
4. Return 1 if `sentry(game, unit, x, y, R, skill, L)` (§6.9) made a
   unit, else 0.

The laid trap is a monster (`pettype` `assassintrap`, `petmax` 5 for
Lightning Sentry): its skills come from the caster's `sumskill1..5`
(§6.5 step 6; Lightning Sentry: `sentry lightning` at L, Shock Field,
Charged Bolt Sentry, Death Sentry at their base levels); its AI
(target pick, shot, idle times, charges, death) is
`monsters/ai-bodies-6.md` §14. With 1.14d data (`lightningsentry`: AI
AssassinSentry, aip 100 / 10 / 15 / 25; `Skill1` `sentry lightning`,
`calc4` `par8` = 10) that gives 10 shots and no lifetime timer. Each
shot is `sentry lightning`'s `srvmissile` `sentrylightningbolt`
(`use.md` §5.4 step 7), damage from its `missiles.txt` `Skill` column
(Lightning Sentry).

#### 8.4 22 Nova attack `0x005C9B50`

Skills (class, 4): Howl, Frost Nova, Nova, Poison Nova (also 5 monster
skills, `functions.tsv`).

1. Unit flags |= 0x40.
2. R invalid → 0.
3. m = `prog_missile(unit, skill)`; invalid → 0.
4. v = `Vel` + (`VelLev` × L) / 8 of missile m (`0x00663270`; missiles
   +0x9A, +0x9B, u8; signed truncating division) + `eval(calc1)`.
5. `ring(game, unit, unit, m, skill, L, v)` (§6.7; flag 4: the missile
   creator still applies its 75 % factor, `missiles.md` §R2.3 step 7).
   Return 1.

#### 8.5 66 Holy Fire, Holy Shock, Sanctuary, Conviction `0x005CF3A0`

Run by the aura timer like §4.5 (`use.md` §7).

1. R invalid, or `aurastate` not in 0…states count − 1 → 0.
2. cost = `0x00644B10(skill, L)`; mana = the unit's `mana(8)`; d =
   `period(…)` − F + 1 (`0x0056CD50`).
3. Self context (0x50 bytes, zeroed; layout of §4.5 step 3): state =
   `aurastate`, skill, L, duration d; for i = 1…5: stat i =
   `passivestat_i`; value i = `eval(passivecalc_i)` when the unit is not
   a player or mana ≥ cost, else 0. Run the §4.5 callback `0x005CEDC0`
   on the unit itself (`0x0056B740`; remove callback `0x005CEC50`).
4. The unit's room is in town (`0x0061AB00`) → return 1 (no scan, no
   mana, state 85 unchanged).
5. Target context B (on the stack; D2MOO `D2DamageAuraParamStrc`):
   state = `auratargetstate`, skill, L, d, stats[6] (+0x10), values[6]
   (+0x28), count (+0x40) = 0, any (+0x48) = 0, record (+0x4C) = none.
   `auratargetstate` in 0…count − 1: for i = 1…6: stat i =
   `aurastat_i`; mana ≥ cost (no player test) → value i =
   `eval(aurastatcalc_i)`, non-zero → any = 1; else value i = 0.
6. Zeroed damage record D; `roll_elemental(unit, D, skill, L)`
   (`levels.md`) ≠ 0 → hit class (+0x60) |= 0xD; `HitClass` ≠ 0 → hit
   class := it; result (+0x04) |= `ResultFlags` | 0x20; hit flags
   (+0x00) |= `HitFlags`; B.record := D.
7. `scan_unit(game, unit, 0, 0, eval(aurarangecalc), aurafilter,
   0x005CF2A0, B, noaura = 0)` (§2.12).
8. cost > 0 and a player: B.count > 0 → state 85 on and
   `0x0056C110(unit, cost)`; else state 85 off. Return 1.

Callback `0x005CF2A0` (ECX scan context, EDX unit U):

1. B.any ≠ 0: a fresh 0x50 context with B's state, skill, L, d, stats and
   values (count, list, passivestate, callback 0); run `0x005CEDC0` on U
   with it (§4.5; U is never the source here, so the default remove
   callback).
2. B.record set: copy it (0x70 bytes); `apply(game, unit, U, 1, copy)`
   (`0x0057C6C0`, `combat/damage.md` §5.2); reaction `0x0057CEE0(game,
   unit, U, copy)` (§7.1).
3. Return 1.

Nothing increases B.count (the inner context counts instead): a player
with cost > 0 always ends with state 85 off and pays nothing per run
(Edge case 9). The element is rolled once per run, shared by all
targets.

#### 8.6 8 Multiple Shot, Teeth, Shock Wave `0x005DB410`

Also PrimePoisonball (monster).

1. Unit flags |= 0x40.
2. R invalid → 0.
3. Target position (§2.4) → (tx, ty); failure → **return 1** (nothing
   made).
4. n = `eval(calc1)`.
5. (dx, dy) = (tx, ty) − unit position. Grow (`0x0056D370`): s = dx² +
   dy²; s < 4 → dx ×= 4, dy ×= 4, s recomputed; s < 16 → dx ×= 2, dy ×=
   2. Side step (`0x0056D3B0`): a = −dx, b = dy; while a² + b² > 3: a =
   a / 2, b = b / 2 (signed, truncating); (px, py) = (b, a).
6. tx −= px·n / 2, ty −= py·n / 2 (signed, truncating).
7. m = `srvmissilea` (+0x48); the unit's hand class (`0x00623C60`) ≠ 1
   (not a bow) and `srvmissileb` (+0x4A) ≥ 0 → m = `srvmissileb`. Not 0
   ≤ m < missiles count → 0.
8. c = `eval(calc3)`; c = 0 → c = n.
9. Record (§R2.1, zeroed): flags 0x820 (target absolute, activate
   given), owner = origin = the unit, class m, skill, L, activate frames
   = `eval(calc2)`; flags |= 0x10000.
10. k = (n − c) / 2 (truncating). Repeat k times: target (tx, ty),
    create (`0x0059FA30`); tx += px, ty += py.
11. Flags &= ~0x10000; repeat c times (create, step).
12. Flags |= 0x10000; repeat n − k − c times (create, step). Return 1.

The c middle missiles lack flag 0x10000 (`missiles.md` §R2.1).

#### 8.7 115 Plague Poppy, Cycle of Life, Vines `0x005C6A80`

1. R invalid → 0.
2. c = `summon_class(unit, skill, L, &mode)`; < 0 → 0 (mode unused).
3. pt = `pettype` (signed byte); < 0 or ≥ pettype count → 0.
4. Unit flags |= 0x40.
5. m = spawn (§6.2) {flags 0, owner unit, class c, AI state 0, **mode
   8**, x = y = 0, pt, pet max `eval(petmax)`}; none → 0.
6. `node_insert(game, m, 0, unit +0xD0)`.
7. m state 150 (`vine_beast`) on.
8. m `level(12)` := max(`eval(calc2)`, 1) (`0x00627260`; no monlvl
   bonuses, §6.4 is not called).
9. `skill_stats(game, unit, m, skill, L, 0)` (§6.5). Return 1.

#### 8.8 34 Tiger Strike, Cobra Strike, Royal Strike `0x005D3490`

1. T none → 0.
2. R invalid, `aurastate` not in 0…states count − 1, or `aurastat1` not
   in 0…itemstatcost count − 1 → 0.
3. Zeroed record; result = `melee_result(game, unit, T, to_hit(unit,
   skill, L), 0)`; `start_combat(game, unit, T, record, SrcDam, or 128
   when 0)` (rolls the weapon damage on a hit).
4. p = `pair_record(unit, T)` (§2.2); p and p's result has hit (1) →
   `charge_add(game, unit, skill, L, aurastate, aurastat1)` (§6.10).
5. `apply_melee(game, unit, T)`. Return 1.

This body does not set unit flag 0x40 itself (`§8.10` does before
calling it).

#### 8.9 56 Clay Golem, BloodGolem, FireGolem `0x005C5100`

1. Unit flags |= 0x40.
2. skill = 0 → 0 (an out-of-range skill reads a null record: fatal).
3. c = `summon` (+0xBC); not 0 ≤ c < monstats count → 0 (no
   `summon_class` fallback).
4. pt = `pettype` read unsigned; ≥ pettype count → pt = 0. mode =
   `summode` unsigned; ≥ 16 → 1.
5. m = spawn (§6.2) {flags 0, owner unit, class c, AI state 0, mode, x =
   y = 0, pt, pet max `eval(petmax)`}; none → 0.
6. `golem_stats(game, unit, m, skill, L)` (§6.11).
7. Message 0x7F (AllyPartyInfo, `sim/server-messages.tsv`) about m to
   the unit's client (`0x005531C0`, `0x0053CDF0(client, m)`).
   The 10 bytes (`0x0053CDF0`): 0x7F, 1 when m is a player else 0, m's
   life percent (`0x00621F20`) u16, m's GUID u32, the level id of m's room
   u16; written to the client's buffer at once (not through the unit's
   record list). Recorded: `nec-clay-golem` frame 27
   (`7f 00 64 00 09000000 0200`).
8. `node_insert(game, m, 0, unit +0xD0)`. Return 1.

#### 8.10 35 Fists of Fire, Claws of Thunder, Blades of Ice `0x005D35D0`

1. The unit has an inventory (+0x60): A = item at body location 4, B =
   at 5 (`0x0063BDE0`). Dual claws when A ≠ B, both exist, both usable
   (`0x0062A4E0`: not broken, item flag 0x4000 clear,
   `items/inventory.md`) and both of item type 45 `weap` (`0x00629BB0`).
2. Dual claws: i = (unit +0x38 bits 8+, the frame event index of
   `use.md` §5.2) mod 2 (signed). i ≠ 0 → unit flags |= 0x40; i = 0 →
   unit flags &= ~0x40 (a later frame event runs the do again: the
   second claw).
3. Not dual: unit flags |= 0x40.
4. srvdo 34 body (§8.8) with (game, unit, skill, L). Return 1 (its
   result is ignored).

Which claw each run uses, whose stats count, and why a second frame
event exists only with two claws (sequence 16, `ht2`): `bodies-2.md`
§2.27.

#### 8.11 13 Fend, Zeal, Fury `0x005DBC60`

Uses the used skill entry E (`0x00620250`): param 1 (+0x18) hits left,
param 2 (+0x1C) target type, param 3 (+0x20) target GUID (set by the
start functions srvst 37, §7.6, and srvst 9, `bodies-2b.md` §7.4).

1. R invalid → 0. E none → 0.
2. r = melee range of the unit (`0x00622870`, `combat/hit.md`) + 4.
3. T = the unit of (E param 2, E param 3) (`0x00552F60`). T none or not
   in melee range (`0x00622C40(unit, T, 0)`) → T = `next_unit(game, unit,
   0, 0, r, 0x20003, E param 3, null)` (`missiles.md` §R9.6 item 3;
   filter 0x20003 | 0xA783); none → 0.
4. Zeroed record; result = `melee_result(game, unit, T, to_hit(unit,
   skill, L), 0)`.
5. Hit: enhanced damage % (+0x0C) := `eval(calc2)`. `EType` ≠ 0: c =
   `eval(calc4)`; conversion % (+0x68) := c; c > 0 → conversion element
   (+0x65) := `EType`; `roll_elemental(unit, record, skill, L)`.
6. `start_combat(game, unit, T, record, SrcDam or 128)`;
   `apply_melee(game, unit, T)` (second argument 4, unread).
7. n = E param 1 − 1; E param 1 := n. n ≤ 0 → 0.
8. T' = `next_unit(game, unit, 0, 0, r, 0x20003, T's GUID, null)`; none
   → 0.
9. E param 2 := T' type; param 3 := T' GUID. Animation rewind
   `0x0056E210(unit, Param2 of the skill)` = `0x00553B10(game, unit, p)`
   (`sim/units.md` §4.2 variants). Return 0.

The body always returns 0: the do core charges nothing and sets no
delay after it (`use.md` §5.4 step 8); mana is paid by the start
function.

#### 8.12 28 Meteor, Blizzard, Eruption, … `0x005CA3E0`

Class skills: Eruption, Blizzard, Meteor (also 9 monster rows,
`functions.tsv`).

1. R invalid → 0.
2. m = `srvmissilea` (+0x48); not 0 ≤ m < missiles count → 0.
3. Unit flags |= 0x40.
4. `point_free(game, unit, m)` (§6.12) = 0 → 0.
5. Return 1 if `missile_at(game, unit, skill, L, m, 0, 0)` (§6.13) made a
   missile, else 0.

#### 8.13 6 Inner Sight, Slow Missiles `0x005DB1C0`

1. R invalid, `auratargetstate` not in 0…states count − 1, or
   `aurastat1` not in 0…itemstatcost count − 1 → 0. (Unit flag 0x40 is
   not set.)
2. Context {skill, L, duration `eval(auralencalc)`, state
   `auratargetstate`, stat `aurastat1`, value `eval(aurastatcalc1)`}
   (evaluated in this order, then the range).
3. `scan_unit(game, unit, 0, 0, eval(aurarangecalc), aurafilter,
   0x005DB150, context, noaura = 1)` (§2.12). Return 1.

Callback `0x005DB150` (ECX scan context, EDX U): `apply_state` (§2.7)
{source the unit, target U, skill, L, duration, stat, value, state,
default callback}; return 1 whatever it gave.

#### 8.14 31 Raise Skeleton, Raise Skeletal Mage `0x005C4B00`

1. T none → 0. `0x00645510(T, 0)` (corpse test of §3.6) = 0 → 0.
2. `raise_penalty(game, unit)` (§6.14).
3. R invalid → 0.
4. c = `summon_class(unit, skill, L, &mode)`; < 0 → 0. pt = `pettype`
   (signed); outside 0…count − 1 → 0.
5. (x, y) = T's position (`0x0045ADF0`, `0x0045AE20`). Room delete
   record `0x0061A270(T's room, T type, T GUID)` (prepends {type, GUID}
   to the room's delete list +0x18 and flags the room); remove T
   (`0x00555600(game, T)`).
6. m = spawn (§6.2) {flags 1, owner unit, class c, AI state 0, mode, x,
   y, pt, pet max `eval(petmax)`}; none → 0 (the corpse is gone anyway).
7. `base_stats(game, unit, m, 0, L)` (§6.4); `components(unit, m, skill,
   L)` (§6.15); `skill_stats(game, unit, m, skill, L, 0)` (§6.5);
   `summon_resist(unit, m)` (§6.11); `node_insert(game, m, 0, unit
   +0xD0)`. Return 1.

1.14d differs from D2MOO 1.10f: the penalty runs before the record test,
and the components come between base and skill stats.

#### 8.15 116 Werewolf, Werebear `0x005C6EC0`

Also Delerium Change (monster).

1. R invalid, or `aurastate` not in 0…states count − 1 → 0.
2. Unit flags |= 0x40.
3. `clear_group(unit, aurastate, 1)` (§2.9) removed something (the unit
   was shifted): delay `0x0056F020(game, unit, skill, L)` (d =
   `eval(delay)` > 0 → `set_delay(game, unit, d)`, `use.md` §6); return
   0.
4. d = `eval(auralencalc)`. The unit already has a list of `aurastate`
   → 0.
5. Alloc (game pool, flags 2, expire F + d, owner the unit; failure → 0);
   set state; remove callback `0x005C6C50`; attach; state on; timer 12 at
   F + d.
6. `aura_fill(unit, list, R, skill, L)`; list set 350 := skill, 351 :=
   L. The unit's entry of the skill (`0x006439F0`) → its mode (+0x08) :=
   10 (`0x00644340`, values ≤ 0x20). Return 1.

Remove callback `0x005C6C50` (ECX unit, EDX state, stack list):
`clear_group(unit, state, 1)`; list given: entry of skill list[350] →
mode := 10 passed through the disguise remap (`0x005C6BF0` →
`0x00645270`); state off.

Shifting back returns 0: the core charges nothing, so the delay is set
here.

#### 8.16 19 Inferno, Arctic Blast `0x005C9640`

R invalid, or m = `srvmissilea` not 0 ≤ m < missiles count → 0. Return
`inferno_do(game, unit, skill, L, m)` (§6.17).

#### 8.17 23 Blaze, Energy Shield `0x005C9C10`

Also SpiderLay, PrimeBlaze.

1. R invalid, `aurastat1` < −1 or ≥ itemstatcost count (validated, not
   used), or `aurastate` not in 0…states count − 1 → 0. (Unit flag 0x40
   is not set; no same-group removal.)
2. `apply_state` (§2.7) {source = target = the unit, skill, L, duration
   `eval(auralencalc)`, stat −1, value 0, state `aurastate`, default
   callback} → none → 0.
3. `passive_fill(unit, list, R, skill, L)` (§2.6); list set 350 :=
   skill, 351 := L.
4. `auraevent1` ≥ 0: unregister (1, `aurastate`); for i = 1…3 while
   `auraevent_i` ≥ 0: register (`auraevent_i`, skill, L,
   `auraeventfunc_i`, 1, `aurastate`) (§2.13). Return 1.

#### 8.18 120 Feral Rage, Maul `0x005C77C0`

1. R invalid or `aurastate` not in 0…count − 1 → 0. E = used skill
   entry; none, or its skill (`0x00643CE0`) ≠ skill → 0.
2. Unit flags |= 0x40.
3. T exists → `apply_melee(game, unit, T)`.
4. E param 1 = 0 (the start missed) → return 1.
5. e = F + `eval(auralencalc)`. The unit's list of `aurastate`, or a new
   one (game pool, flags 2, expire e, owner the unit, state set, callback
   `0x0056E900`, attached, state on; failure → return 1).
6. Expiry := e; timer 12 at e; mark the state changed.
7. n = min(`eval(calc2)`, list[169 `skill_frenzy`] + 1); list set 169 :=
   n; 350 := skill; 351 := L.
8. `aura_fill(unit, list, R, skill, n)`: the aura formulas see the
   **charge count n as the level**. Return 1.

#### 8.19 10 Guided Arrow, Bone Spirit `0x005DB6D0`

Also MonBoneSpirit.

1. Unit flags |= 0x40. R invalid → 0.
2. T = target (may be none). Target position (§2.4) → (tx, ty); failure
   → 0.
3. v = `eval(calc1)`.
4. m = `srvmissilea`; hand class ≠ 1 and `srvmissileb` ≥ 0 →
   `srvmissileb` (as §8.6 step 7); invalid → 0.
5. Record (§R2.1, zeroed): flags 0x20, owner = origin = the unit, target
   unit T, class m, target (tx, ty), skill, L; v ≠ 0 → init callback
   `0x005DB6A0` (adds v to the missile's `damagepercent(25)`), argument
   v. The read is the unit total, layer 0 (`0x00625480`, `sim/stats.md`
   §4.2); the write sets the base, layer 0, on the missile's own list
   (`0x00627260`, `sim/stat-lists.md` §5 item 2). On a fresh missile
   (`missiles.md` §R2.3 step 21) both are equal. Answered (2026-10-08,
   impl-missile-init MI3). T none → flags := 0x420 (frames from distance).
6. Create; none → return 1.
7. Missile data +0x28 := 1 with T, 2 without (`0x0064A710`; homing bit,
   `missiles.md` §R9.5 item 4); data +0x2C := (ty − uy) << 16 + (tx −
   ux), each 16-bit signed, from the unit's position (`0x0064A760`).
   Return 1.

#### 8.20 118 Twister, Tornado `0x005C72F0`

1. R invalid → 0. m = `prog_missile(unit, skill)` (§6.6); invalid → 0.
2. Unit flags |= 0x40.
3. n = `eval(calc1)`; n ≤ 0 → 0.
4. Return `fan(game, unit, n, m, skill, L, 0)` (§6.18).

#### 8.21 49 Shadow Warrior, Shadow Master `0x005D6E70`

1. Unit flags |= 0x40. R invalid → 0.
2. c = `summon_class(unit, skill, L, &mode)`; < 0 → 0.
3. pt = `pettype` read unsigned; ≥ pettype count → 0.
4. m = spawn (§6.2) {flags 0, owner unit, class c, AI state 0, mode, x =
   y = 0, pt, pet max `eval(petmax)`}; none → 0.
5. m `level(12)` := the unit's `level(12)`.
6. `shadow_stats(game, R, skill, L)` on m (§6.19).
7. ilvl = `Param5` + (L − 1)·`Param6` (0 when L ≤ 0; `0x004EFCB0`);
   < 1 → 1; > the maximum level of class 0 (`0x00611830(0)`) → it.
   Equipment `0x005D6B60(game, unit, m, skill, L, ilvl, 0)` (§6.5 step
   9 notes).
8. Delete m's type-2 timers; type-2 timer at F + 20.
9. `aurastate` in 1…count − 1 → m state on.
10. `link_source(m, unit)` (§6.20).
11. d = `eval(auralencalc)` > 0 → type-7 timer at F + d on m and umod
    21 (`0x005A4850(game, m, 21, 0)`, temporary summon,
    `monsters/init.md`).
12. `node_insert(game, m, 0, unit +0xD0)`. Return 1.

No `base_stats` / `skill_stats` here.

#### 8.22 124 Armageddon, Hurricane `0x005C8190`

Also Diablogeddon.

1. R invalid or `aurastate` not in 0…count − 1 → 0. E = used skill
   entry; none, or its skill ≠ skill → 0.
2. Unit flags |= 0x40.
3. r = `roll(0x10000)` on the unit's seed (`rng.md` §3; one draw: `lo'
   & 0xFFFF`).
4. e = F + max(`eval(auralencalc)`, 1). The unit's list of `aurastate`,
   or a new one (game pool, flags 2, expire e, owner the unit, state set,
   callback `0x0056E900`, attached, state on; failure → 0).
5. Expiry := e; timer 12 at e; `aura_fill(unit, list, R, skill, L)`;
   list set 350 := skill, 351 := L.
6. t = F + `Param4` (+0x154, `0x004EFC80`). Delete the unit's type-5
   timers with argument skill (`0x00540E60(game, unit, 5, skill)`);
   type-5 timer at t with arguments (skill, L) (runs the state's
   `srvactivefunc`, `use.md` §7: srvdo 145 / 146).
7. E param 1 := r. Return 1.

## Constants & data dependencies

| Item | Value | Where |
|---|---|---|
| default aura filter | 0x583 | §2.12 |
| curse AI states / AI kinds | 23 → 10, 56 → 11 (27 → 12 coded, unused) | §4.4 |
| `AiCurseDivisor` | difficultylevels +0x1C (1, 2, 4) | §4.4 |
| velocity / attack-rate floor | monstats `ColdEffect` (+0x168 + difficulty), else −50 | §4.5 |
| resist scaling | v / 5 at base resist ≥ 100 | §2.10 |
| state ids | 2 poison, 23 dimvision, 54, 56 terror, 57 attract, 85 nomanaregen, 86 justhit, 114 blood mana, 118 corpse_noselect, 139 wolf, 140 bear, 143 attached | |
| flag groups | 11 curse, 12 curable, 30 exp, 33 udead, 38 meleeonly; lists pgsv (data tables +0x16C), curse (+0x174), disguise (+0x17C) | `runtime-maps.md` §4 |
| skills columns | srvoverlay +0x4E, aurafilter +0x50, aurastat1–6 +0x54, auralencalc +0x60, aurarangecalc +0x64, aurastatcalc1–6 +0x68, aurastate +0x80, auratargetstate +0x82, auraevent1–3 +0x84, auraeventfunc1–3 +0x8A, passivestate +0x94, passivestat1–5 +0x98, passivecalc1–5 +0xA4, tgtoverlay +0x108, ResultFlags +0x12E, HitFlags +0x130, HitClass +0x134, calc1–4 +0x138, Param2 +0x14C, Param5 +0x158, HitShift +0x1A4, SrcDam +0x1A5, MinDam +0x1A8, EType +0x1DC, prgdam +0x44, srvprgfunc1–3 +0x30 | `fields.tsv` |
| item types | 27 bow, 35 crossbow, 38 missile potion | |
| missiles | 0 arrow, 27 magicarrow, 31 bolt, 41 explodingarrow | §2.3 |
| skills columns (batch 2) | srvmissilea–c +0x48–+0x4C, summon +0xBC, pettype +0xBE (i8), summode +0xBF (i8), petmax +0xC0, sumskill1–5 +0xC4, sumsk1–5calc +0xD0, sumumod +0xE4, sumoverlay +0xE6, Param2 +0x14C; flags +0x04 bits 2 `progressive`, 5 `aura`, 8 `InTown` | `fields.tsv` |
| summon request flags | 1 position given, 2 replace linked unit, 4 no second spawn try, 8 no unit flag 0x80000000 | §6.2 |
| missile ring | 64 offsets, radius 30, tables `0x006E1288` (x) / `0x006E1388` (y) | §6.7 |
| state ids (batch 2) | 12 inferno, 54 uninterruptable (fatal on a fresh summon), 97 skel_mastery, 98 sourceunit, 150 vine_beast | §6, §8 |
| stat ids (batch 2) | 6 hitpoints, 7 maxhp, 12 level, 19 tohit, 25 damagepercent, 31 armorclass, 142 / 144 / 148 absorb %, 169 skill_frenzy, 349 passive_summon_resist, 353 source_unit_type, 354 source_unit_id | §6, §8 |
| component table | `0x00741940`, 11 rows × 9 bytes | §6.15 |

Batch 2 order: class skills (`charclass` one of the 7 classes) of the
1.14d `patch_d2` `skills.txt` per `srvstfunc` / `srvdofunc` slot not
`spec'd-here` before batch 2; descending count, ties by the lowest
`reqlevel` of the skills using it. The `srvmissile*` columns name
missiles, not function slots, and are not counted.

| Slot | Count | Skills (reqlevel) |
|---|---|---|
| srvst 23 | 6 | Tiger Strike (1) … Royal Strike (30) |
| srvdo 119 | 6 | Oak Sage (6), Summon Spirit Wolf (6) … Summon Grizzly (30) |
| srvdo 68 | 5 | Shout (6) … War Cry (30) |
| srvdo 45 | 5 | Charged Bolt Sentry (12) … Death Sentry (30) |
| srvdo 22 | 4 | Howl (1), Frost Nova (6), Nova (12), Poison Nova (30) |
| srvdo 66 | 4 | Holy Fire (6), Holy Shock, Sanctuary (24), Conviction (30) |
| srvdo 8 | 3 | Teeth (1), Multiple Shot (6), Shock Wave (24) |
| srvdo 115 | 3 | Plague Poppy (1), Cycle of Life (12), Vines (24) |
| srvdo 34 | 3 | Tiger Strike (1), Cobra Strike (12), Royal Strike (30) |
| srvdo 56 | 3 | Clay Golem (6), BloodGolem (18), FireGolem (30) |
| srvdo 35 | 3 | Fists of Fire (6), Claws of Thunder (18), Blades of Ice (24) |
| srvdo 13 | 3 | Zeal (12), Fend (24), Fury (30) |
| srvdo 28 | 3 | Eruption (12), Blizzard, Meteor (24) |
| count 2, in order | 2 | srvdo 6, srvdo 31, srvdo 116, srvst 6, srvst 11, srvdo 19, srvst 12, srvst 17, srvdo 23, srvst 37, srvst 56, srvdo 120, srvdo 10, srvdo 118, srvdo 49, srvdo 124 |
| count 1, in order | 1 | srvdo 7, 17, 64, 150, 69, 114, 117; srvst 22; srvdo 33; srvst 24; srvdo 42, 20, 21; srvst 16; srvdo 32, 55; srvst 40; srvdo 77, 70, 71, 43, 44; srvst 25; srvdo 46; srvst 7; srvdo 60; srvst 31; srvdo 67, 74; srvst 34; srvdo 72, 47, 11, 24, 25, 26, 27, 61, 63; srvst 35; srvdo 73, 81; srvst 41; srvdo 78; srvst 57; srvdo 121; srvst 58, 26; srvdo 48; srvst 27; srvdo 50; srvst 8; srvdo 12, 15; srvst 9, 13; srvdo 29; srvst 18; srvdo 59; srvst 19; srvdo 62; srvst 20; srvdo 57, 79; srvst 36; srvdo 9, 75, 122, 123, 51, 52, 16; srvst 10; srvdo 14; srvst 14; srvdo 144; srvst 21; srvdo 58, 80, 82; srvst 38; srvdo 76; srvst 39, 28; srvdo 54 |

114 slots: 13 of count ≥ 3, 16 of count 2, 85 of count 1.

## Randomness

No body here draws on its own. Draws happen inside the callees, in the
order the steps call them: `melee_result` (`combat/hit.md` §4),
`fill` / `start_combat` / `bonuses` (`combat/damage.md` §3),
`roll_elemental` and its random element (`levels.md` §3.6), missile
creation (`missiles.md` §R2). The finisher's peek (§2.14 step 3) steps a
copy and restores the unit seed: no net advance. Aura, curse and buff
bodies draw nothing.

Batch 2 (§6–§8), same rule: summons draw in monster creation
(`0x005B2F20`: game seed for the unit seed, placement, `monsters/init.md`)
and in the equipment step of §6.5 (m's seed); §8.5 calls
`roll_elemental` once per run on the caster's seed before the scan, then
`apply` per target; §8.8 and §8.11 draw in `melee_result`,
`start_combat` and (§8.11) `roll_elemental`; every missile created (64
per ring, n per Multiple Shot) takes one game-seed step
(`missiles.md` §R2.3 step 9).

## Edge cases & original bugs

1. A player whose mana is below an aura's cost still refreshes the
   aura state lists on itself and its targets, with no stats (§4.5 step
   4); the passive stats need mana strictly above the cost.
2. Bash's attack-rate list (§3.8 step 2) is attached on every start and
   removed with the unit's next mode change (TEMPONLY,
   `sim/stat-lists.md` §8.9).
3. Srvdo 2 accepts `srvoverlay` = overlay count (one past the table)
   and returns 1 without applying the melee when no combat record
   exists for the target (§4.2 steps 2–3).
4. The curse callback registers events 2 and 3 based on event 1's
   validity (§4.4 step 8); no 1.14d curse has `auraevent1`.
5. The room-box test `0x0056B770` of the scans never rejects a room
   (r > 0): every adjacent room is scanned unit by unit.
6. Shape-shift starts add the to-hit of each shape skill at the Attack
   level (§2.15).
7. `apply_state` refuses a lower-level recast of the same state and
   skill; a higher level replaces the list (§2.7 step 4).
8. Srvdo 2 refreshes an existing `auratargetstate` list's stats and
   timer but keeps its old expiry (§4.2 step 3).
9. Srvdo 66 never counts its targets (§8.5 step 8): Sanctuary (`mana` 1,
   the only one of the four with a cost) is never charged per run and
   state 85 stays off. Its target values need mana ≥ cost for monsters
   too.
10. §6.5 step 2 looks a stat-event handler up by skill field s but
    registers it with s << 16: for s ≠ 0 the lookup never matches, so a
    second `skill_stats` call on one summon adds duplicate handlers.
11. §6.1 falls back to the AI control's spawn class through a monster's
    data; for a non-monster caster with an invalid `summon` it reads a
    null pointer (fatal).
12. §6.5 step 4 accepts `aurastate` = states count (one past the table).
13. Srvdo 115 ignores `summode` and spawns in mode 8 (§8.7 step 5).
14. Srvdo 34 called directly (Tiger, Cobra, Royal Strike) leaves unit
    flag 0x40 as it was; srvdo 35 sets or clears it first (§8.10).
15. Srvdo 8 returns 1 without a missile when the target position fails;
    srvdo 13 returns 0 after a hit (§8.6 step 3, §8.11).
16. Monster Inferno: §6.17 step 5 sets the entry's param 1 to 1 before
    step 7 compares the frame with it, so the monster channel always ends
    after one call (state 12 off, ENDANIM at F + `InfernoLen`); the
    timeout of §6.16 step 2 is never used (as D2MOO).
17. A Paladin who casts Raise Skeleton / Skeletal Mage (item charges)
    takes max life / 8 physical damage before the record test, even when
    the raise then fails (§8.14 step 2).
18. Shadow stats (§6.19 step 4) evaluate `aurastatcalc2` for every
    `aurastat_i` and `passivecalc2` for every `passivestat_i` (as
    D2MOO 1.10f).
19. Feral Rage / Maul (§8.18 step 8) evaluate the aura formulas with the
    charge count as the skill level.

## Test vectors

| Case (1.14d data unless synthetic) | Expected |
|---|---|
| Kick start, T present (MinDam 0, HitShift 8) | record: hit flags 2, result 9, physical 0, hit class 1; `start_combat(…, 128)` without a roll; returns 1 |
| Kick start, no T | 0, no combat entry |
| Might L1 aura run at F = 1251 (perdelay 50) | period 1301 → duration 51; lists expire at 1302 with a timer 12 there; next run 1301 refreshes to 1352 |
| Amplify Damage L1 on a monster with base damageresist 100 | duration 200; stat 36 = −100 / 5 = −20 in the curse list (state 9) |
| Dim Vision L1 (`ln34` = Param3 175) on a monster, Hell | duration 175 / 4 = 43 |
| Aura filter 73731 | players and monsters, allies only, no units in town rooms; no 0x80 / 0x400 tests |
| `dec_quantity`, right-hand javelins q = 1 (synthetic) | quantity 0, 0x3E sent, returns 1 |
| `dec_quantity`, q = 0 (synthetic) | quantity 0, returns 0: no missile when called from `skill_missile` |
| Progressive prgdam 2, n = 3, ln12 = 10 (synthetic) | life and mana leech += 20 |
| Curse resistance 50, duration 200 (synthetic) | duration 100 |
| Ring offsets (table `0x006E1288` / `0x006E1388`) | i = 0 (30, 0); 8 (21, 21); 16 (0, 30); 40 (−21, −21); 63 (29, −2) |
| `base_stats` p = 0, owner level 30, L = 5 (synthetic) | p = 5 + 90 / 4 = 27 (< 30); stat 12 = 27 |
| `base_stats` p = 0, owner level 2, L = 5 (synthetic) | p = 6 ≥ 2 → 2 |
| Multiple Shot from (0, 0) at (10, 0), n = 5, calc3 = 1 (synthetic) | (px, py) = (0, −1); targets (10, 2), (10, 1) flag 0x10000; (10, 0) without; (10, −1), (10, −2) with |
| `charge_add`, counter 2 → 3, then again (synthetic) | 3 and `aurastat2` added once more; then unchanged (return 1, expiry refreshed) |
| Dual claws, frame event index 0 then 1 (synthetic) | flag 0x40 cleared, then set: two srvdo 34 runs per attack |

## Provenance

- 1.14d disassembly of every listed address (`py tools/ghidra/disasm.py
  fn|at`); tables `0x00732140` (srvst), `0x007322B0` (srvdo) for
  identification (`use.md` §8); jump tables `0x0056B6AC`, `0x005C35A8`
  / `0x005C35B0`, `0x0056CA0C` dumped from the image; offsets from
  `data/fields.tsv`; values from the 1.14d `patch_d2` tables.
- D2MOO 1.10f `SKILLS_SrvSt01…`, `SrvDo001/002/018/030/065`,
  `sub_6FCF5680`, `sub_6FCF5870`, `sub_6FD10EC0`, `sub_6FD10E50`,
  `AURAFILTER_*` for names. 1.14d differences: wolf / bear Attack do
  applies the melee (§4.1 step 7); the curse callback's event loop tests
  event 1 each time (as D2MOO); `0x005C3540` resistance scaling and the
  hireling exception are not in D2MOO's curse callback.
- Batch 2 (§6–§8): every address disassembled (`disasm.py fn|at`);
  ring tables `0x006E1288` / `0x006E1388` and the component table
  `0x00741940` read from the image; use
  counts from the 1.14d `patch_d2` `skills.txt`. D2MOO names compared:
  `SrvSt06/11/23`, `SrvDo006/008/013/019/022/028/031/034/035/045/056/066/068/115/116/119`,
  `SKILLS_StartInferno`, `SKILLS_DoInferno`, `D2GAME_SetUnitComponent_6FD0C3A0`,
  `D2SummonArgStrc`, `D2GAME_GetSummonIdFromSkill_6FD15580`,
  `D2GAME_SetSummonPassiveStats_6FD0C530`,
  `D2GAME_SKILLS_SetSummonBaseStats_6FD0CB10`,
  `D2GAME_SetSummonResistance_6FD0C2E0`, `sub_6FCF9580`. 1.14d
  matches D2MOO in the steps above except: §6.5 step 6 skips summon
  skill 0 (k must be ≥ 1). Also compared: `SrvSt12/17/37/56`,
  `SrvDo010/023/049/118/120/124`, `UNITS_StoreOwner`.
Ghidra backlog (2026-10-06): `0x00580310` (`0x005801E0`, `0x00580030`,
`0x00580280`), `0x00580380`, break `0x0055F850`, breakable test
`0x00629930`; `0x00575900` with `0x00575850`; `0x005C0C30`. Stat names
from live `itemstatcost`.
Implementation questions SB2, SB3, SB6, OQ4, OQ7 (2026-10-06), from
the asm: `0x0056E740` with the 50-slot table `0x007325B0` dumped from
the file; `0x005D3880`, `0x005D3970`; `0x005C37C0`, `0x005C35C0`;
`0x0062A0F0`, `0x0062E830`, `0x0057FF70`, `0x006233A0`, `0x00628250`,
`0x0063AD50`; `0x005B0DA0`, `0x005DD480`, `0x00623470`, `0x0046C140`
(monstats +0x18 → monstats2 mode bits +0xF0); the 543 count from the
`patch_d2` monstats.txt / monstats2.txt.

## Open questions

1. Recording: hook `0x005CF010` and `0x0056E970` (entry, return) with a
   Paladin running Might in a party: confirm duration, expiry, count and
   state 85 per tick.
2. Recording: Kick, Bash and Attack on a monster: `0x0057DBF0` record
   before / after (`combat/damage.md` Open question) to confirm §3.2,
   §3.8, §4.1.
3. Recording: Amplify Damage on an immune monster (stat 36 value in the
   list) and Dim Vision in Nightmare (expiry − F).
4. ~~Attack-mode cleanup bodies~~: answered in §2.16, with the
   helpers `0x0062A0F0`, `0x0062E830`, `0x0057FF70`; Bash's list:
   `sim/stat-lists.md` §8.9.
5. ~~`0x00575900`~~: answered in §2.17 (pet maxima; no message).
6. ~~Unit event handler iteration~~: answered in §2.18.
7. ~~`0x005B0DA0`~~: answered in §4.4 (`can_switch`): the monstats
   argument is unused; 543 live classes.
8. Answered: the player pet lists (add `0x00575D90`, lookup
   `0x00574A20`) are specified in `sim/pets.md`.
9. Recording: Raise a Druid summon and a Clay Golem: confirm stats 12,
   31, 19, 7, 6 on the summon (§6.4, §6.5) and the AI think at F + 25.

Batch 3 (the 85 slots used by one class skill, Constants "count 1") is in `skills/bodies-2.md`.

Batch 4 (monster slots) is in `skills/bodies-3.md` / `bodies-4.md`; its §2 answers the implementation questions on §2.16 (left / right skill order corrected), §6.1, §6.2, §6.5, §6.15.
