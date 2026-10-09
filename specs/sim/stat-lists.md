# Spec: Simulation — Stat lists, modifiers and state lists

- **Status:** draft (no `d2-sim` code yet); every rule read from the
  1.14d `Game.exe` code. Checker ready and self-tested
  (`check_stats.py --selftest`: synthetic recording of the test vectors,
  4 perturbations each reported at the changed record); unverified on the
  running game until a `record_stats.py` recording passes (queued).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::stats::lists` (lists, arrays, chains,
  propagation), `d2-sim::stats::states` (state bits, expiry), timer
  handlers 3, 5, 9, 12 in `d2-sim::units`
- **Related specs:** `sim/stats.md` (keys, values, readers, evaluation
  and op formulas this spec calls); `sim/stat-ops.tsv`; `sim/tick.md`
  §5 (timer queue, the dispatch of events 3, 5, 9, 12); `sim/units.md`
  (unit records, unit creation; points here for the handlers);
  `sim/intents-events.md` (stat and state messages); `data/fixups.md` §2.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 44–57 |
| Inputs | 58–66 |
| Outputs / state changes | 67–73 |
| Rules | 74–75 |
|   1. Records | 76–124 |
|   2. Flags (+0x10) | 125–162 |
|   3. Stat arrays | 163–178 |
|   4. Allocation and ownership | 179–202 |
|   5. Base writes | 203–227 |
|   6. Full values | 228–293 |
|   7. Value-change notification | 294–330 |
|   8. Chain operations | 331–438 |
|   9. States | 439–475 |
|   10. Timer event handlers | 476–563 |
|   11. Mod array and stat messages | 564–581 |
| Constants & data dependencies | 582–593 |
| Randomness | 594–597 |
| Edge cases & original bugs | 598–621 |
| Test vectors | 622–658 |
| Provenance | 659–682 |
| Open questions | 683–712 |
<!-- /index -->

## Summary

Every unit with stats owns one **extended** list (unit +0x5C). Items,
states, skills and auras add **lists** of their own: an item's extended
list, or a plain list holding a state's bonuses. Attaching a list to a
unit adds its values to the unit's **full** values; detaching subtracts
them. Each list keeps a sorted array of base values; an extended list
also keeps a sorted full array (base + attached lists + op terms,
`stats.md` §6), a "mod" array of changed player stats to re-send, and the
unit's state bits. A base write propagates the change up the chain of
parents, recomputing the full entries that depend on it, and notifies the
unit's value-change callback for flagged stats. State lists carry an
expiry frame; the REMOVESTATE timer event frees the expired ones.

## Inputs

| Name | Type | Source |
|---|---|---|
| base writes | list, stat, layer, value | skills, items, states, regeneration, level-up, … |
| attach / detach / free | unit, list | equip, state set/clear, skills, death, unit removal |
| state toggles | unit, state, on/off | state code (skills, potions, curses) |
| timer events 3, 5, 9, 12 | unit, args | `tick.md` §5.6 dispatch |

## Outputs / state changes

Base, full and mod arrays, chain links, list flags, state bits;
value-change callbacks (§7), messages queued through the mod array
(§11) and the life-fraction message (§10.1); timer events scheduled or
cancelled by the handlers (§10).

## Rules

### 1. Records

**List** (`0x006251F0` allocates 0x3C bytes, zeroed; D2MOO
`D2StatListStrc`):

| Offset | Field |
|---|---|
| +0x00 | memory pool |
| +0x04 | unit the list is attached to (set by attach, cleared by detach) |
| +0x08, +0x0C | owner type, owner GUID (given at allocation) |
| +0x10 | flags (§2) |
| +0x14 | state id (`0x006252D0` sets, `0x006252F0` gets) |
| +0x18 | expire frame, i32 (§10.4); `0x00625310` / `0x006260B0` set it and, when > 0, set flag 2 |
| +0x1C, +0x20 | skill id, skill level (callers' bookkeeping) |
| +0x24 | base array: pointer, i16 count (+0x28), i16 capacity (+0x2A) |
| +0x2C, +0x30 | prev, next sibling |
| +0x34 | parent list |
| +0x38 | remove callback (ECX unit, EDX state, list) |

**Extended list** (`0x00626D40`, 0x64 bytes; D2MOO `D2StatListExStrc`):
the fields above, plus

| Offset | Field |
|---|---|
| +0x3C | last attached active child (head of the active chain) |
| +0x40 | last attached parked child (head of the parked chain, flag 0x2000) |
| +0x44 | owner unit |
| +0x48 | full array: pointer, i16 count, i16 capacity |
| +0x50 | mod array: pointer, i16 count (+0x54), i16 capacity (+0x56); 4-byte keys |
| +0x58 | state bits: 2 × W u32, W = (states count + 31) / 32 (signed division) |
| +0x5C | value-change callback (§7) |
| +0x60 | game |

Array entry: 8 bytes, u16 layer, u16 stat, i32 value, so the first dword
is the key (`stats.md` §1.3).

The base array of an extended list is the common field at +0x24 (i16
count +0x28, capacity +0x2A): a unit's own base stats (unit +0x5C) live
there, and the base reader `0x006253B0` searches it through
`0x00624ED0` (list +0x24) for plain and extended lists alike. The full
array (+0x48) is separate and holds the totals (§6). A tool that records
a unit's base stats reads +0x24 / +0x28 (`tools/original-hooks.md` §4
rule 5).

Chains: a parent's +0x3C (or +0x40) points at its newest child; each
child links to the older one through prev (+0x2C) and to the newer one
through next (+0x30). Walking "the chain" means from the head through
prev.

### 2. Flags (+0x10)

| Bit | D2MOO name | Rule |
|---|---|---|
| 0x00000001 | BASIC | kept on the unit at death (§8.8); the only bit of the allocation argument an extended list keeps |
| 0x00000002 | NEWLENGTH | the list expires (§10.4); set with an expire frame > 0, and on the unit's list when a TEMPONLY list is attached |
| 0x00000004 | TEMPONLY | §8.1 |
| 0x00000080 | OVERLAY | §8.8 |
| 0x00000100 | (unk) | on the unit's list: "remove the overlay list's stats" (§8.8); also kept at death |
| 0x00002000 | SET | **parked**: lives in the parked chain, counts nowhere (§8.5) |
| 0x20000000 | PERMANENT | set when a full entry of a stat with A53 is written (§6.3); never cleared |
| 0x40000000 | DYNAMIC | its damage-related stats do not count in the parent (§8.1, §8.6) |
| 0x80000000 | EXTENDED | the 0x64-byte form |

Callers pass other bits (e.g. 0x08, 0x20, 0x40) for their own lookups
(§9.3); this spec gives them no rule. Who sets each bit (1.14d, every
allocation site of `0x006251F0` (70) and `0x00626D40` (9), and every
store to list +0x10 in `0x00625000`–`0x00628000`):

| Bit | Set by |
|---|---|
| 0x1 | the monster base list only (`0x00573CB0` at `0x0057407D`, `monsters/init.md` §6) |
| 0x2 | expiry setters `0x00625310` (4 callers) and `0x006260B0` (26), always on a plain list just allocated or found as a state's list (`0x006256B0`); allocations with 2 / 0x802; attach rule 4 on the unit's own list |
| 0x4 | allocations by `0x0056E520`, `0x0056F1F0`, `0x005D7EA0`, `0x00620E80` (server), `0x004C7BE0`, `0x004C7950` (client); `0x005A6380` with 0x4004 |
| 0x8 | `apply_state` `0x0056E970` for a state with `aura` (`skills/bodies.md` §2.7 step 6) |
| 0x20 | `apply_state` for a curse state (step 3) |
| 0x40 | item class-skill lists: `0x00557AB0` (generation), `0x0062A970` (load), `0x005C0D70`, `0x005C0F90` (staffmods, `items/generation.md` §6.2), `0x00534C70` (start items); client `0x0045F190`, `0x004C1910` |
| 0x80 | the overlay list `0x00621E40` (§8.8) |
| 0x100 | `0x00625A00(unit)` on the unit's own list, called only from `0x00621E40`; cleared by `0x00627410` |
| 0x800 | `apply_state` for a state with `exp` (step 6); allocations 0x802 by `0x005D0350` and `0x005D7430` (Conversion) |
| 0x4000 | `0x005A6380` only (with TEMPONLY) |

Variable flags: `0x0056E970` (above), item property lists
`0x0065CBF0` (flags from its 14 callers, `items/properties.md`),
item load `0x0062CBE0` (its argument), `0x005543B0` and `0x00621C30`
(0: they allocate only when no list was found). Every extended list is
allocated with flags 0.

### 3. Stat arrays

1. Entries are kept sorted by key ascending (signed), keys unique.
   Binary search: lo = 0, hi = count; mid = lo + (hi − lo) / 2;
   key > mid → lo = mid + 1, key < mid → hi = mid, equal → found
   (`0x00624B90`, insertion point `0x00624BD0`).
2. Insert (`0x00624C80`): at the insertion point, value 0, later
   entries move up. Remove (`0x00624D10`): later entries move down.
   Capacity grows by 4 when full and shrinks by 4 when it exceeds the
   count by more than 8; this is memory only and has no observable
   effect.
3. Mod array (+0x50): sorted unique 4-byte keys, same search
   (`0x00624D90`, insert in `0x00624DD0`).
4. A base array never holds a value 0 (every writer removes the entry,
   §5). A full array holds 0 only for `keepzero` stats (§6.3).

### 4. Allocation and ownership

1. List: `0x006251F0`(pool, flags, expire, owner type, owner GUID).
   Expire is stored without setting flag 2.
2. Extended list: `0x00626D40`(unit, flags, callback, game): frees the
   unit's current list (if extended), allocates, owner type/GUID = the
   unit's type/GUID, owner = the unit, flags = (flags & 1) | EXTENDED,
   state bits zeroed, unit +0x5C := list. Free: `0x00626D10` (unit).
3. Callers (1.14d): server player `0x005348C0`, server monster
   `0x00573CB0` (both with callback `0x0055B800`, §7.2), server item
   `0x00555D20`, server missile `0x0059FA30` (no callback); client
   units `0x00460BF0` (callback `0x004609F0`), `0x004AE8D0`,
   `0x004AEDD0`, `0x004C1910`, `0x004CD540`. Objects and tiles have no
   list. All pass flags 0.
4. A freed list is never passed again by 1.14d code: the specs found
   no caller that keeps a list pointer past its free (§8.3) and then
   reads, writes, attaches, detaches, frees or toggles it. So only the
   null list has original behaviour (`stats.md` §4.2: reads 0; §5.1 /
   §5.3: no-op; §8.4: nothing). A d2rs handle naming a freed list is
   outside fidelity: an implementation may treat it as the null list
   where a null rule exists and do nothing / answer 0 elsewhere, and a
   replay that reaches such a call is itself a mismatch to report, not
   a behaviour to match.

### 5. Base writes

Key k = (s << 16) + layer.

1. **Set** (`0x006270B0`(list, s, value, layer); with a callback unit
   `0x00627170`(list, s, value, layer, unit); null list → no-op):
   absent and value = 0 → return 0. Absent → insert 0. d := value − old;
   d = 0 → return 0. value = 0 → remove, else store. Propagate (§6.1)
   with d and the unit (null for `0x006270B0`). If the list is extended
   and its owner type (+0x08) is 0: mod insert (§11). Return 1.
2. Unit wrappers: `0x00627260`(unit, s, value, layer) = set on unit
   +0x5C, then (redundant) mod insert for a player. `0x00627150`: set on
   a non-null list.
3. **Add** (`0x00627030`(list, s, d, layer); `0x006272B0` on unit
   +0x5C): null list or d = 0 → nothing. Absent → insert 0; value += d;
   result 0 → remove. Propagate with d (null unit); extended player →
   mod insert.
4. **Remove all** (`0x00627340`(list)): while the base array is not
   empty: take its first entry (lowest key), remove it, propagate −value,
   mod insert as in rule 1. (A 0 value would loop forever; rule §3.4
   makes it unreachable.)
5. **Merge** (`0x006274F0`(target, source)): Add for each source base
   entry, in order.
6. The stat id is not validated (`stats.md` §1.1).

### 6. Full values

#### 6.1 Propagate (`0x00626920`(list L, key k, d, unit))

1. d = 0, invalid stat, or L parked (0x2000) → nothing.
2. cur := L if L is extended, else L's parent.
3. While cur: if A51(T) = 0 and A52(T) = 0: **add-full** (§6.3) of d
   at k on cur; else **recompute** (§6.4) k on cur. Then stop if cur is
   parked, or if T is `damagerelated` and cur is DYNAMIC; else cur :=
   cur's parent.

The DYNAMIC test is on the list just updated, so a DYNAMIC extended list
keeps its own damage-related changes, but a change in a plain DYNAMIC
child still reaches its parent (original behaviour).

#### 6.2 Evaluation

eval(L, k): `stats.md` §6.

#### 6.3 Writing a full entry

**Set-full** (`0x00625150`(L, k, v, record, unit)): absent and v = 0 →
return. Absent → insert 0. old := entry. v = 0 and not `keepzero` →
remove; else store v, and if A53(T): L |= PERMANENT. If old ≠ v: notify
(§7.1).

**Add-full** (`0x006250B0`(L, k, d, record, unit)): absent → insert 0;
new := old + d (wraps), stored; new > 0 and A53(T) → PERMANENT; new = 0
and not `keepzero` → remove. Notify (always; d ≠ 0 so new ≠ old).

#### 6.4 Recompute (`0x006266C0`(L, k, record, unit))

1. v := eval(L, k).
2. A51(T) = 0: set-full(k, v); return v.
3. v = 0: set-full(k, 0).
4. update := true. For j = 0, 1, 2: S := op stat j of T (u16 +0x58 +
   2j); 0xFFFF ends the loop. vS := recompute(L, S, …); set-full(S, vS).
   If vS ≠ 0, apply `recompute_block` of T's own op (`stat-ops.tsv`;
   op 0: none), with e = entry j of entries(T) (`stats.md` §3; an unused
   entry reads base 0):
   - `always`: update := false;
   - `listtype_pm` / `listtype_item`: false when L's owner type is 0/1 /
     is 4;
   - `listtype_pm_and_entrybase_list_total_pos`: false when L's owner
     type is 0/1, e.base ≠ 0xFFFF and L's raw total of e.base > 0;
   - `unit_pm_and_entrybase_unit_total_pos`: false when L's attached unit
     is a player or monster, e.base ≠ 0xFFFF and that unit's total of
     e.base (minimum rule) > 0;
   - `none`.
5. If update and v ≠ 0: set-full(k, v); then for each d in deps(T)
   (≤ 64): set-full(d, recompute(L, d)).
6. Return v.

Consequences (reproduce): when update is false, this call does not
write T's own full entry. Its callers decide what happens next: a
recompute reached from rule 4 (T an op stat of another stat) or rule 5
(T a dependency) is followed by the caller's set-full(T, returned v)
(`0x00626786`, `0x006268CF`), so T's entry is written there; a recompute
reached from propagate (§6.1) or from attach/detach (§8.1.7, §8.2.4) has
no such write, and T's entry keeps its previous value. For op-2 stats
(e.g. 216) on a player, entry j of the stat's own target table is unused
(base 0 = strength), so any strength > 0 blocks: a change of the
per-level stat itself leaves its full entry stale, but a recompute of a
stat that lists it as a dependency writes it. When v = 0 the deps are
not recomputed.

### 7. Value-change notification

#### 7.1 Rule

Notify(L, k, old, new) calls L's value-change callback (+0x5C) when it is
set and the stat has `fCallback`: callback(ECX game (+0x60), EDX owner
(+0x44), unit, k, old, new), unit being the propagation's unit argument.
It runs inside the write, before the propagation continues; anything it
does (more base writes, attach, detach) happens at that point.

#### 7.2 The server callback (`0x0055B800`)

Players and monsters (§4.3). Invalid stat → return. Then:

1. If `itemevent1` (i16 +0x48) > 0, item event registration on the
   owner (`units.md` §6.6), with k the full stat key: new ≠ 0 → if no
   record (kind 2, key k, v0 k) exists (`0x005C0BE0`), add (`0x0056E740`)
   event `itemevent1` with function index `itemeventfunc1` (i16 +0x4C),
   kind 2, key k, v0 k, v1 0; then, if `itemevent2` (i16 +0x4A) > 0,
   add `itemevent2` with `itemeventfunc2` (+0x4E) the same way (an
   existing record skips both). new = 0 → remove every record of kind 2
   and key k (`0x005C0B50`).
2. By stat:
   - 7, 9, 11 (max life, mana, stamina), with current = 6, 8, 10: if
     new ≠ old and old > 0 and c := unit total(current) > 0: o := old,
     or 256 when old ≤ 256; q := the x87 product new / o · c stored as a
     float32 and truncated toward zero (`0x00682FD0`); m := max(q, 1);
     set the unit's base current to new when m ≥ new, else to m (set,
     §5.2). For stat 7 on a monster whose monstats `DamageRegen` (u32
     +0x1A0) ≠ 0: set stat 74 := ((new >> 8) · DamageRegen) >> 4.
   - 83, 97, 98, 107, 126, 127, 151, 188, 204: skill and state handlers
     (skills spec). Stats 97 / 107, 83 / 188 / 126 / 127 and 98 and the
     pet-maximum helper `0x0056BD90` are `skills/levels.md` §7 "Skill
     stat callbacks" (owner; dispatch inside `0x0055B800`).

The client callback `0x004609F0` is client-only.

### 8. Chain operations

#### 8.1 Attach (`0x00626E10`(unit U, list L, reset))

1. R := U +0x5C; R missing or not extended → nothing.
2. Detach L (§8.2).
3. If L is R or one of R's ancestors → stop.
4. L TEMPONLY → R |= NEWLENGTH.
5. L parked: L.prev := R.parked head (its next := L), L.next := 0, head
   := L, L.parent := R, L.unit := U; stop (no values move).
6. Else: L.prev := R.active head (its next := L), L.parent := R, head :=
   L, L.unit := U.
7. If L is extended and PERMANENT: recompute (§6.4, unit null) each key
   of L's full array whose stat has A53 (first 16, array order).
8. Values A := L's full array if extended, else its base array; owner :=
   L's owner if extended, else none. reset ≠ 0: clear L's DYNAMIC; for
   each entry of A in order, propagate(R, key, value, owner). reset = 0:
   set DYNAMIC; the same, skipping `damagerelated` stats.

#### 8.2 Detach (`0x006269F0`(list L); `0x006277E0`(unit, L))

1. P := parent. If P: if P is extended and its active head is L, head :=
   L.prev; if its parked head is L, that head := L.prev; L.parent := 0.
2. Unlink L from its siblings (next.prev := L.prev, prev.next := L.next),
   L.prev := L.next := 0; old unit := L.unit; L.unit := 0.
3. L parked → stop (no values move, no callback).
4. L extended and PERMANENT: recompute L's A53 stats as in §8.1.7 (with
   the unit already cleared, so ops 4/5 contribute 0).
5. If P: for each entry of A (§8.1.8), skipping `damagerelated` stats
   when L is DYNAMIC: propagate(P, key, −value, owner).
6. If the old unit and L's remove callback are set: callback(old unit,
   L's state, L).

#### 8.3 Free (`0x00626C00`(L); `0x00626CD0` frees only plain lists)

1. Detach L.
2. If L is extended: walk its active chain from the head: each child
   gets parent := 0, unit := 0; an extended child is kept and the walk
   goes on to its prev; a plain child is removed from the head if it is
   the head, freed (rule 1 for it), and the walk restarts at L's head.
   Parked children are left as they are.
3. Release the arrays and the record.

#### 8.4 Equip and swap (`0x00627910`(unit U, item I, reset))

I's list missing → nothing. I in a swap body location (11 or 12): detach
I's list. Else if the list is attached to U: reset ≠ 0 and the list
DYNAMIC → make static (§8.6); reset = 0 and not DYNAMIC → make dynamic.
Else attach(U, I's list, reset) and set the list's unit to U.

#### 8.5 Park and unpark (`0x006279A0`(unit, state, park))

The list of that state (§9.3, `0x00625650`) is found or nothing happens.
Park: if not parked: detach, set 0x2000, attach with reset 1. Unpark: if
parked: detach, clear 0x2000, attach with reset 1. Returns 1 when it
changed the list, else 0.

#### 8.6 Dynamic toggles (`0x00627860` static, `0x00627A40` dynamic; (U, I))

If I's list is not attached to U: equip(U, I, 1 / 0) instead. Else, with
U's list extended: static: if I's list is DYNAMIC, clear it and
propagate(U's list, key, +value, I) for each `damagerelated` entry of its
full (or base) array; dynamic: if not DYNAMIC, set it and propagate
−value the same way.

#### 8.7 By-time refresh (`0x006276C0`(U, I))

U a player or monster with an extended list: for each full entry of I's
list whose stat has op 6 or 7, for each of its op stats S (until 0xFFFF):
set-full(U's list, S, eval(U's list, S), unit U). Caller `0x005627F4`.

#### 8.8 Death and overlay

1. `0x00627540`(unit): unit null, or its list missing or not extended
   → nothing. Walk the unit's active chain from the head (next := prev,
   read before the test); a list whose owner type is not 4, whose flags
   have none of 0x181 and whose state does not stay on death (below) is
   freed if plain; in both cases (freed or extended) the walk restarts
   at the head. Callers `0x0057F33D`, `0x005A659A`.
   **Stays on death** (`0x0063A4A0`(unit, state)): state < 0 or ≥ the
   states count → no. Unit a monster (type 1) → the state has flag
   `monstaydeath` (states flag bit 14, bitset 14 of `runtime-maps.md`
   §4, read at data tables +0x104); any other unit, a null unit included
   → flag `plrstaydeath` (bit 13, data tables +0x100). `bossstaydeath`
   is not read here.
2. `0x00627410`(unit): clear 0x100 on the unit's list, then remove all
   (§5.4) from the first active child with OVERLAY (`0x006256E0`).
   Called from the room update queue step (`tick.md` §3 step 6,
   `0x00553220`) when the unit's list has 0x100.

#### 8.9 Temporary lists (`0x006272E0`(unit))

Run by every real mode change (`0x00624690`, `sim/units.md` §4.1: a
new mode, not for unit type 5) after the mode is written, and by the
client's twin (`0x004B0D5A`). R := the unit's list; R missing or
without NEWLENGTH → nothing. Else walk R's active chain from the head:
each TEMPONLY list whose state (+0x14) is non-zero first turns that
state off (`0x00639DB0`(unit, state, 0), §9.2); a non-extended one is
then freed (§8.3); after any TEMPONLY list the walk restarts at the
head (an extended TEMPONLY list would loop forever, but extended lists
keep only bit 0x1 of their allocation flags, §4, so none exists).
Other lists are passed over (next := prev link). Finally R's NEWLENGTH is
cleared, even when expiring lists are still attached (they then wait
for the next attach that sets it again). So a TEMPONLY list (§8.1 step
4; e.g. Bash's attack-rate list, `skills/bodies.md` §3.8) lives until
the next mode change. 1.14d-confirmed (asm of `0x006272E0`,
`0x00624690`).

### 9. States

#### 9.1 Bits

State bits of a unit = the first W words at +0x58 (bit s of word s/32);
the second W words mark "changed" bits. Units without an extended list
have no states.

#### 9.2 Toggle (`0x00625A70`(unit, state, on); with an update-queue insert `0x00639DB0`)

Set or clear bit s. If it changed: set bit s of the second half; if the
state has flag `disguise` (states flag bit 16): on → unit +0xC8 |= 8;
off → clear it unless another disguise state is still on (`0x0063A7B0`).
`0x00639E30` sets or clears a second-half bit only.

`0x00639DB0`(unit, s, on): s outside 0 … states count − 1 → nothing
(no toggle, no queue). Else the toggle above, then the update-queue
insert (`unit-order.md` §6.2) **always**, whether or not the bit
changed. 1.14d-confirmed (asm of `0x00639DB0`).

#### 9.3 Queries

- Has state (`0x00639DF0`): unit type 0, 1 or 3 and 0 ≤ s < count and
  the bit is set; other unit types never have a state.
- Has any state of flag group g (`0x0063A7B0`(unit, g), g < 40):
  intersects the unit's bits with the flag bitset g (`runtime-maps.md`
  §4). `0x0063A750` = group 32 (`life`).
- List of a state (`0x00625650`(unit list, s)): first in the active chain
  with list state = s, else first in the parked chain. By flags
  (`0x006256E0`): parked chain when 0x2000 is asked, else active chain;
  first list with any asked flag. By state and flags (`0x006257D0`(unit,
  s, flags)): the unit's list missing or not extended → none; parked
  chain when flags has 0x2000, else the active chain; f := flags without
  0x2000; walking from the head, the first list whose state = s and,
  when f ≠ 0, whose flags share any bit with f (not every bit); f = 0
  matches on the state alone. None found → null.

### 10. Timer event handlers

#### 10.1 Event 3, regeneration

**Player** (`0x00580810`(game, unit, a1, a2)):

1. Schedule event 3 for the unit at frame + 1 with a1, a2.
2. Unit dead (`0x005541B0`: null; +0xC6 bit 0; player mode 0 or 17;
   monster mode 0 or 12; any other unit type) → stop.
3. Life (`0x00580610`): r := total(74). If r ≠ 0: hp := total(6) + r;
   if hp > max life: hp := max life, and the unit's list of state 100
   (`healthpot`), if any, is detached and freed when plain; hp < 256 →
   256; set stat 6 := hp. Then f := life fraction (`stats.md` §9.3); if
   |f − (stat 352 & 0xFF)| > 4: message `0x00571A10`(unit, f) and set
   stat 352 := f.
   Both the life write and the fraction update are inside "r ≠ 0" (r =
   0 jumps straight to the return). `0x00580610` returns 1 on every
   path (`0x005806DD`), so the "regenerate if `0x00580610`" test of
   `units.md` §6.1 never stops steps 4 and 5 (2026-10-07, implementation
   question in `docs/handoff/impl-units-stats.md` §5 item 3). The handler
   runs steps 4 and 5 only when it returns non-zero (`0x00580844`), so
   stamina and mana run on every frame the unit is not dead, whatever r
   is.
4. Stamina (`0x00580500`): s := total(10), b := total(28). By unit mode:
   1, 5 → shift 8; 2 → shift 9, but only if s & 0xFFFFFF00 ≠ 0; 6 →
   shift 9; any other mode → only if b ≥ 1000, shift 8. Else stop.
   m := max stamina; s ≥ m → stop. i := m >> shift; if b ≠ 0: i += (i ·
   b) / 100; s := min(s + i, m); set stat 10 := s.
5. Mana (`0x005806F0`): v := total(8), m := max mana, i := 0. Unless the
   unit has state 85 (`nomanaregen`): q := charstats `ManaRegen` · 25
   (7500 if 0); i := max(m / q, 1); i := MulDiv(i, total(27) + 100,
   100). Then i += total(26). If v ≥ m and i > 0: the list of state 106
   (`manapot`) is detached and freed. i := min(i, m − v), i := max(i,
   −v); i ≠ 0 → add stat 8 += i.

**Monster** (`0x005A6920`(game, unit, …)):

1. r := total(74); if the unit has a `life`-group state (§9.3): r −= base(74).
2. State 52 (`preventheal`) and r ≥ 0 → stop (no reschedule).
3. Schedule event 3 at frame + 1 (args 0, 0). r = 0 → cancel all the
   unit's type-3 events (`0x00540E60`, argument 0 = any; including that
   one) and stop.
4. hp := total(6), m := max life. r < 0 and hp < 256: continue only if
   the unit's room exists and is not in a town level (`0x0061AB00`:
   the room's level is 1, 40, 75, 103 or 109, `drlg/levels.md`).
5. hp += r; hp > m → cancel all type-3 events, hp := m; hp < 1 →
   0; set stat 6 := hp; fraction update as for players (`0x005A5650`).
6. hp = 0 and mode ∉ {0, 12}: killer := owner of the unit's state-2
   (`poison`) list, else state-62 (`openwounds`) list (by type and GUID,
   `0x00552F60`); unit +0xB0 := 0. With state 54 (`uninterruptable`):
   state 92 (`death_delay`) on, stop. Else `0x0057CCB0`(game, unit,
   killer) and the death events `0x005C0C30` (monster spec).

#### 10.2 Event 5, active state (`0x0056D790`, players and monsters)

a1 = skill id, must be 1 … skills count − 1; s := its `aurastate`
(+0x80); state record `srvactivefunc` (u16 +0x36) f < 191 and table
`0x007322B0`[f] set → call f(game, unit, skill, a2). Nothing else; the
functions belong to the skills spec.

#### 10.3 Event 9, periodic stats (`0x0056FE40`)

a2 = skill id. Skill or its `aurastate` invalid, or the unit dead (as
§10.1.2) → cancel the unit's type-9 events whose first argument is a1
(all of them when a1 = 0; `0x00540E60`). Else l :=
total(151 `item_aura`, layer = skill); l ≤ 0 → the same cancel; else
`0x0056F7F0`(game, unit, skill, l, 1, 1, 0) and `0x0056CE70`(game, unit,
a1, skill, l, 0) (aura application, skills spec).

#### 10.4 Event 12, expiry (`0x00627460`(unit, frame))

Handlers: player `0x00580800`, monster `0x005A7EF0`, item `0x0055F130`;
each passes the game frame. With the unit's list extended, walk its
active chain from the head (next := prev, read before the test): a list
with NEWLENGTH and expire ≤ frame (signed) is freed if plain, and the
walk restarts at the head. An expired extended list is not freed, but
the restart happens all the same (`0x006274A9`): the walk finds it again
and, with the frame unchanged, never ends (confirmed in the 1.14d code;
whether any 1.14d path creates such a list: open question 5). Parked
lists never expire. Frame 0 (client): each NEWLENGTH list's expire −= 1
first; expired at ≤ 0 (the restart decrements every NEWLENGTH list
again on each pass).

The event only triggers the walk: every due list of the unit goes,
whichever event was scheduled for it. State code schedules type 12 at
the list's expire frame (e.g. `0x0054CD31`, `0x0056F33F`); the state
clears through the list's remove callback (§8.2.6).

### 11. Mod array and stat messages

1. Mod insert (`0x00624DD0`(list, key)): list extended; stat not 6, 8,
   10, 13, 14; stat `Saved`; key not present → insert. 1.14d `Saved`
   stats are 0–15, so the array holds changed base values of 0–5, 7, 9,
   11, 12, 15 (any layer).
2. Flush: in the per-client update (`tick.md` §6.5) `0x006258D0`(unit,
   …, sender `0x00548520`) sends, for each mod key in order, the unit's
   current base value of that key (0 when absent). Message layout:
   `intents-events.md`.
3. Clear: `0x00625960` empties the array when the unit is processed by
   the room update queue (`tick.md` §3 step 6, `0x00553220`), after the
   flush of the same tick.
4. Single stats: `0x00625870`(unit, client, stat, sender) sends the
   base value (layer 0) when the key is not in the mod array and is
   present in the base array; used for 67, 68, 12, 0, 2 by `0x00580860`.
   Sender arguments: (ECX unit, EDX stat, value, client).

## Constants & data dependencies

| Constant | Value |
|---|---|
| list / extended list size | 0x3C / 0x64 bytes |
| array entry | 8 bytes (u16 layer, u16 stat, i32) |
| op-base buffer in attach/detach | 16 keys |
| states | count `[0x744304]+0xC4` (185 in 1.14d); flag bitsets `[0x744304]+0xCC` + 4g |
| active-state function table | `0x007322B0`, 191 entries |
| states used by handlers | 2 `poison`, 52 `preventheal`, 54 `uninterruptable`, 62 `openwounds`, 85 `nomanaregen`, 92 `death_delay`, 100 `healthpot`, 106 `manapot` (states.txt rows, 1.14d) |
| stats used by handlers | 6, 7, 8, 9, 10, 11, 26, 27, 28, 74, 151, 352 |

## Randomness

None.

## Edge cases & original bugs

1. Base setters store invalid stat ids (`stats.md` §1.1).
2. Recompute leaves T's own full entry stale when update is false and
   the recompute was not reached as an op stat or dependency (§6.4
   consequences); per-level stats reach a player's full array only
   through such a caller.
3. A plain DYNAMIC child's damage-related base changes still reach its
   parent (§6.1).
4. Expiry of an extended list loops forever (§10.4); the death walk
   (§8.8 rule 1) loops the same way on an extended child whose owner
   type is not 4, without 0x181 and with a state that does not stay on
   death (item lists have owner type 4 and are skipped). The state the
   expiry spins in is fixed: every expired plain list met before it in
   the walk is freed, nothing behind it changes. d2rs stops there instead
   of hanging and reports `StatListError::EndlessExpiry(list)` with the
   lists in that state; a recording that finished the walk contradicts
   the loop and is a mismatch.
5. Attach/detach collect at most 16 A53 keys (a 16-slot buffer, no
   bound check); 1.14d data has 4 such stats.
6. Free leaves parked children pointing at the freed parent (§8.3).
7. The max-rescale of §7.2 uses x87 floating point (open question 1).
8. Remove-all over a 0 base value would loop (§5.4; unreachable).

## Test vectors

Synthetic (`check_stats.py --selftest` replays them as a recording; the
values are computed by hand here, not by the checker):

| Step | Expected |
|---|---|
| player P (class with LifePerVitality 16, StaminaPerVitality 4): set 0 = 30, 12 = 10, 3 = 25, 7 = 12800, 6 = 12800 | full = base; one callback (7: 0 → 12800); mod {0, 3, 7, 12} (6 excluded) |
| item list I: 3 = 10, 216 = 8, 19 = 5; attach(P, I, reset 1) | callbacks in order: (7: 12800 → 23050) with, inside it, set 6 = 23050; (11: 0 → 2560). P full: 0 = 30, 3 = 35, 6 = 23050, 7 = 23050, 11 = 2560, 12 = 10, 19 = 5; no 216 (§6.4) |
| detach I | callbacks (7: 23050 → 12800, set 6 = 12800 inside), (11: 2560 → 0); 11 and 19 leave P's full array |
| state list S (plain, NEWLENGTH, state 30, expire 20, base 0 = 5) attached | P full 0 = 35; state bit 30 on |
| set 74 = 100, 6 = 10000; player life regeneration | set 6 = 10100 |
| frame 20, event 12 | S freed (only due list); P full 0 = 30 |

Recorded: none yet (queued, see Open questions 4).

Comparison (exact), on a `record_stats.py` recording (`stats-raw-1`,
`tools/trace-recorder/README.md`):

1. Replaying every recorded base write, attach, detach, free,
   park/unpark, dynamic toggle, by-time refresh and state toggle by the
   rules of this spec and `stats.md`, every snapshot's lists equal the
   model: base array, full array and mod array as sorted (stat, layer,
   value) / key lists, state bits, chain links (parent, prev, next, both
   heads) and the DYNAMIC and PERMANENT flags.
2. The value-change callbacks recorded are exactly the ones §7.1
   predicts, in order, with the same list, key, old and new value, and
   the operations recorded inside each are replayed in place.
3. Each expiry frees exactly the lists §10.4 selects, in order.
4. Each player life, stamina and mana regeneration and each monster
   regeneration writes the value §10.1 gives.

`check_stats.py` exits 1 on any difference; `--perturb-snap N` and
`--perturb-cb N` must be reported at exactly the changed record (M08).
Until `d2-sim` exists the model is the checker; afterwards `d2-sim`
replaces it under the same comparison.

## Provenance

- **1.14d `Game.exe`** (SHA-256 `631066c1…adaaf`), disassembly
  (`tools/ghidra/disasm.py`) of every function cited; register arguments
  from the disassembly. Tables read from the file image: recompute switch
  `0x0062690C`/`0x006268F4`, mod exclusion switch `0x00624EB8`, stamina
  mode switch `0x005805B8`, value-change switch `0x0055BE04`/`0x0055BDD8`,
  handler tables of `tick.md` §5.6 and item `0x006E117C` (event 12 =
  `0x0055F130`). Callers found with `disasm.py xref`.
- **D2MOO** (1.10f) `D2Common/src/D2StatList.cpp`, `include/D2StatList.h`
  (`D2StatListStrc`, `D2StatListExStrc`, flags, `sub_6FDB64A0`,
  `sub_6FDB6C10`, `D2Common_ExpireStatList_6FDB6E30`,
  `D2COMMON_10475_PostStatToStatList`, `STATLIST_UpdateStatListsExpiration`,
  merge/toggle functions): same layout, flags and algorithms, each
  re-read in 1.14d. Differences in 1.14d: attach recomputes A53 stats
  only when the incoming list is PERMANENT (D2MOO: any extended list);
  the op 4/5 recompute switch only ever clears update (D2MOO assigns
  `value <= 0`); remove-all removes entries directly (D2MOO calls set 0,
  same effect); the add-full PERMANENT test is new > 0 (as D2MOO's
  comment says). D2Game handlers (regeneration, events 5, 9, 12, the
  value-change callback) were read from 1.14d only.
- **Data**: state and stat names from 1.14d `states.txt` / `itemstatcost.txt`
  (patch_d2); charstats and monstats offsets from `data/fields.tsv`.

## Open questions

1. ~~§7.2 max-rescale: x87 precision control during the division and
   product (24-, 53- or 64-bit) decides the float32 result in rare cases;
   settle with a recording of max-life changes (`check_stats.py`
   compares the nested set) or by reading the FPU control word at the
   call.~~ → PC 2 recording list. 53-bit precision: settled for Game.exe
   2026-10-09 by the CRT start-up read (`items/treasure.md` OQ5:
   `0x0068E70A`, PC = 53, never changed by game code). PROVISIONAL
   (REC-610) only for a video runtime DLL changing it; a
   `check_stats.py` max-life recording confirms.
2. Answered (2026-10-07): `0x0061AB00(room)` (§10.1 step 4) is "room in a town": the
   room's level id (`0x0066BAB0`) is 1, 40, 75, 103 or 109 (`0x006426A0`,
   byte table `0x006426C8`); null room → 0. Also used by the unit find
   (`monsters/umod-callbacks.md` §3.1).
3. Answered (2026-10-07): §2, table after the flag list (every
   allocation site and flag store of 1.14d).
4. ~~Recording: the whole spec is unverified until `record_stats.py`
   passes `check_stats.py` (queued).~~ → PC 2 recording list.
5. Answered (2026-10-07): no. All 9 extended allocations pass flags 0;
   the expiry setters `0x00625310` / `0x006260B0` are called only on
   plain lists (a fresh `0x006251F0` list or a state's list, which
   `apply_state` allocates plain); attach rule 4 sets NEWLENGTH only on
   the unit's own list, the root, which is never in an active chain;
   and every TEMPONLY list (§2 table) is attached to a player or monster,
   never to an item's extended list. The loop
   itself is confirmed (§10.4). The TEMPONLY allocations (flags 4) are
   at `0x0056E53C`, `0x0056F3D4`, `0x005A63AE`, `0x005D8058` and
   `0x00620E9E` (`0x00620E80` attaches to a player or monster unit,
   whose own list is never walked).
