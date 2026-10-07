# Spec: Client — Stat list of client units (items, states, skills) and the totals

- **Status:** draft: the client call sites read in the 1.14d `Game.exe`
  (addresses below); the item stat contents wait for the item stream
  (`items/inventory.md` open question 1); unverified: no executable
  check runs it yet.
- **Target version:** 1.14d
- **Crate/module:** `d2-client::bridge::world` (the stat list of a
  client unit), used by the handlers that attach lists and by the
  character panel (`ui/panels.md` §8 r7).
- **Related specs:** `sim/stat-lists.md` (lists, attach / detach,
  equip, propagation, states: the code is shared, single owner of the
  mechanics); `sim/stats.md` §4.2 (readers: unit total `0x00625480`,
  unit base `0x006253B0`); `client/model.md` §1 rule 2 (`stats`, layer 0
  base); `client/msg-stats-items.md` (base writes 0x19–0x20, item
  actions); `client/msg-skills.md` §2 rule 4 (passive-state lists);
  `client/msg-units.md` §1.2 (monster umod lists, 0xAC).

## Summary

1.14d is one executable: the client units use the same stat-list code
as the server (`0x0062xxxx`). Each client unit with stats owns an
extended list (unit +0x5C); messages write its base values, and three
kinds of child lists are attached to it on the client: the lists of
equipped items, state lists (from S→C 0xA8 and the passive skills), and
the per-unit lists set up at creation (monster umods). The totals the
character panel shows are the unit total getter over that list (full
array), exactly as on the server. This spec says which client code
attaches what, so the client list can be built the same way.

## Inputs

| Name | Type | Source |
|---|---|---|
| base writes | stat, value | `client/msg-stats-items.md` §1 (0x19–0x20), `client/msg-units.md` §1 (creation) |
| item units and their placement | item action messages | `client/msg-stats-items.md` §2 |
| state messages | 0xA7, 0xA8, 0xA9 | this spec §3 (layouts); owner `client/msg-units.md` §6 |
| skill assignments | 0x21, 0x94 | `client/msg-skills.md` |

## Outputs / state changes

The unit's extended list and its attached child lists; the values read
by `total(unit, stat, layer)` and `base(unit, stat, layer)`.

## Rules

### 1. The list of a client unit

1. A client unit's stat list is a `sim/stat-lists.md` extended list at
   unit +0x5C, with the same records, flags, arrays, propagation and
   readers (`sim/stats.md` §4.2). The value-change callback of a client
   player's list is the client callback `0x004609F0` (installed by the
   player init, `0x00460C39`; `sim/stat-lists.md` §7.2 last line), not
   the server's `0x0055B800` (its effects: open question 1).
2. d2rs: the client holds, per unit with stats, a `d2-sim` stat list
   (the `sim/stat-lists.md` implementation, built with the client
   callback of rule 1), as `client/model.md` §12 does for the client
   DRLG. `ClientUnit.stats` (`client/model.md` §1 rule 2) is the layer-0
   view of that list's base array, so the existing base writes stay as
   they are.
3. **Character panel totals** (`ui/panels.md` §8 r7): value =
   `total(local player, stat, 0)` (`0x00625480`, full array of +0x5C),
   base = `base(local player, stat, 0)` (`0x006253B0`). They equal the
   base until rules 2–4 have attached lists.

### 2. Items

1. **Item list.** When the client sets an item's mode (`0x004C1910(item,
   x, y, room?, mode, fresh)`, 13 callers among the item action
   handlers), a fresh item (`fresh` ≠ 0) gets item flag 0x10, its list
   reset (`0x00626D40`) and a new extended list (`0x006251F0(0, 0x40, 0,
   4, item GUID)`) attached to the item itself (`0x00626E10(item, list,
   1)`); when `0x00629900(item)` ≠ 0 and the item has no inventory
   (+0x60), it gets one (`0x0063ABD0`). The
   item's own stat values come from the item stream (open question 2).
2. **Equip.** `0x004C0D20(force)` (item in ESI, owner in EDI; 17 call
   sites in the action handlers `0x004C2AD0`–`0x004C4C70`) runs after an
   item is placed:
   1. No owner → nothing.
   2. Socket fillers: an item of type 20 (`gem`, itemtypes equivalence
      `0x00629BB0`) needs an item owner (type 4, else fatal 0x30) and
      applies its gem record to the owner (`0x0065FEC0(2, 0, item, …)`;
      no gems record → fatal 0x32 / 0x34); type 74 (`rune`) the same
      with mode 5 and the owner (fatal 0x3A / 0x3C / 0x3E); any other
      item: `0x00663CC0(owner, item, 0, 0)`.
   3. Item flag 0x4000 set (requirements not met, `client/msg-stats-items.md`
      §3 rule 3) → nothing more: the item's list stays detached.
   4. Owner a player (type 0) with `0x006235A0(owner)` ≠ 0: reset := 1,
      except 0 when the owner's inventory has an item X
      (`0x0063BEF0`), X ≠ the item and the item is of type 45 (`weap`).
      Item flag 0x100 set and force = 0 → nothing; else equip
      (`0x00627910(owner, item, reset)`, `sim/stat-lists.md` §8.4).
   5. Otherwise: item flag 0x100 clear or force ≠ 0 → equip with
      reset 1.
3. **Level change** of the local player (`0x0045D3E0`, the level hook of
   `client/msg-stats-items.md` §1 rule 5): for every item of its
   inventory (`0x0063B2C0`, `0x0063DFD0`) that is an item unit with a
   list for which `0x00625820(item, &r)` ≠ 0: detach it
   (`0x006277F0`) and equip again (`0x00627910(player, item, r)`); then
   the left and right skills saved at entry are re-selected (open
   question 3).
4. Unequip and removal: an item unit freed by `client/model.md` §2
   rule 5 frees its lists (detach first, `sim/stat-lists.md` §8.2–§8.3);
   per-action detach (swap locations 11 / 12 through §8.4's swap rule):
   open question 4.

### 3. States (S→C 0xA7, 0xA8, 0xA9)

1. **0xA8 SetState** (`0x0045EE20`; size u8@6): unit type u8@1, GUID
   u32@2, size u8@6, state u8@7, then a bit stream (from @8, size − 8
   bytes, LSB-first reader `client/model.md` §10). Unit not in S →
   nothing. Else state on (`0x004D9B20(unit, state)`, rule 3), then
   repeat: stat id := 9 bits; 0x1FF ends; a stat outside the
   itemstatcost table or with `Send Bits` (+0x08) = 0 ends; param := the
   next `Send Param Bits` (+0x09) bits read signed (`client/model.md`
   §10 rule 3) when that is > 0, else 0; value := the next `Send Bits`
   bits, read signed when `Send Bits` < 32 and the stat's flags (+0x04)
   have bit `[0x006CE26C]` (open question 5), else unsigned; add the stat to the state's list (rule 2). After the
   stream: `0x004D9E60(unit, state)` (state on hooks: the states-table
   `setfunc` (+0x1A, table `0x0072A690`, 31 entries) and, when the state
   has a missile (+0x30 ≥ 0), a client missile from stats 350 / 351).
2. **State stat** `0x004D9D70(unit, list, state, stat, value, param)`:
   no list given → the unit's list of that state (`0x006256B0`), else a
   new list (owner = the unit's type and GUID, `0x006251F0`), its state
   set (`0x006252D0`) and attached to the unit (`0x00626E10(unit, list,
   1)`). Then the stat is set on it (`0x00627150(list, stat, value,
   param)`, `sim/stat-lists.md` §5). Stat 172 (0xAC) also calls
   `0x00463C00(value)`; a stat whose flags (+0x05) have bit
   `[0x006CE26C]` also runs `0x00623F50(unit)`.
3. **State on** `0x004D9B20(unit, state)` (also 0xA7): a state with the
   flag `[0x006CE284]` (+0x14) on a dead unit (monster mode 12, player
   mode 17) only sets the state bit (`0x00639DB0`, `sim/stat-lists.md`
   §9.2). Otherwise an existing list of the state is freed unless the
   state has the flag `[0x006CE278]` (`0x00627340`); the bit is set;
   then overlay, light and anim refreshes (open question 6).
4. **0xA7 DelayedState** (`0x0045EDE0`) and **0xA9 EndState**
   (`0x0045EF60`): 7 bytes, unit type u8@1, GUID u32@2, state u8@6;
   unit in S → 0xA7: state on (rule 3) then `0x004D9E60`; 0xA9: state
   off `0x004D9F40` then `0x004D9C30` (the list is detached and freed:
   open question 6).
5. These three ids keep owner `TBD` in `client/bridge-dispatch.tsv`
   until open question 6 is settled; this section is their stat-list
   rule.

### 4. Skills

1. Passive skills attach a state list per passive state with the
   `passivestat` values (`client/msg-skills.md` §2 rule 4), run by every
   assign (0x94, 0x21) and add.
2. Item-granted skill levels (stats 97, 107, 126, 127, 188 …) reach the
   skill list through the client callback (rule 1.1 and
   `client/msg-skills.md` open question 2), not through this spec.

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| item list allocation | flags 0x40, owner type 4 | `0x004C1910` |
| item types | 20 `gem`, 74 `rune`, 45 `weap` (`patch_d2` ItemTypes rows) | `0x004C0D20` |
| item flags | 0x4000 (requirements), 0x100, 0x10 | `0x004C0D20`, `0x004C1910` |
| 0xA8 stat id width / end | 9 bits / 0x1FF | `0x0045EE20` |
| itemstatcost | record 0x144 bytes: +0x04 / +0x05 flags, +0x08 `Send Bits`, +0x09 `Send Param Bits` | `0x0045EE20` |
| states record | 0x3C bytes; +0x14 flags, +0x1A setfunc, +0x30 / +0x32 missile | `0x004D9B20`, `0x004D9E60` |

## Randomness

None in the code above (client draws inside the state hooks, if any,
belong to the overlay and missile specs).

## Edge cases & original bugs

- An item whose requirements are not met (flag 0x4000) stays in its
  body location with its list detached: its stats do not count in the
  totals until the requirement refresh clears the flag.
- 0xA8 stops reading at the first stat with `Send Bits` 0 or an id out
  of range; the stats before it stay set.

## Test vectors

Synthetic (the recordings carry no 0xA8 and no item stream spec yet).

| Input | Expected | Source |
|---|---|---|
| player base strength 10, no lists | panel strength value 10, base 10 | §1 rule 3 |
| 0xA8 on (0, 1), state 30, stream {stat 0 (Send Bits 8): 5}, 0x1FF | state 30 list attached to (0, 1) with strength 5; total strength 15 with base 10 | §3 rules 1–2 |
| 0xA8 for a GUID not in S | no change | §3 rule 1 |
| equipped item with list {strength 3}, flag 0x4000 clear, owner player | total strength base + 3 | §2 rule 2 |
| same with flag 0x4000 set | total = base | §2 rule 2.3 |

## Provenance

1.14d `Game.exe` (spec session 2026-10-07): attach / equip call sites
from `tools/ghidra/disasm.py xref` of `0x00626E10` and `0x00627910`;
`0x004C1910`, `0x004C0D20` (callers listed by xref), `0x0045D3E0`,
`0x0045EE20`, `0x0045EDE0`, `0x0045EF60`, `0x004D9B20`, `0x004D9D70`,
`0x004D9E60`, `0x00643620`, client callback `0x004609F0` (installed at
`0x00460C39`). Item type rows from `patch_d2` `ItemTypes.txt`.

## Open questions

1. The client callback `0x004609F0`: its per-stat effects (a switch on
   the stat id: light radius 89 / 90, skill stats, …); needed before the
   client list's values match the original after item and skill
   changes.
2. The item stream's stat section (which lists an item gets: base,
   magic, set, runeword) — `items/inventory.md` open question 1 and
   `client/msg-stats-items.md` open question 3.
3. `0x00625820` (which items are re-equipped on a level change) and the
   left / right skill restore of `0x0045D3E0`.
4. Detach per item action (unequip, swap, move to the grid): which
   handlers detach the list and when.
5. The flag masks `[0x006CE26C]`, `[0x006CE274]`, `[0x006CE278]`,
   `[0x006CE284]` (itemstatcost / states flag bits read through globals):
   dump their values.
6. The rest of the state on / off paths (`0x004D97F0`, `0x004D9920`,
   `0x004D9AD0`, `0x004D9F40`, `0x004D9C30`) and their owner (a client
   states spec taking 0xA7–0xA9). A recording with a buff (e.g. a
   shrine) gives 0xA8 bytes to check the stream rule.
