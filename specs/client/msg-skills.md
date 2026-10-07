# Spec: Client — Skill list and skill messages (0x21, 0x22, 0x23, 0x94)

- **Status:** draft: handlers and the shared skill-list functions read in
  the 1.14d `Game.exe` (addresses below); layouts checked against the
  join of both single-player recordings
  (`traces/raw/20261006-015956-packets.jsonl`, `-022633-packets.jsonl`);
  unverified: no executable check runs it yet.
- **Target version:** 1.14d
- **Crate/module:** `d2-client::bridge` handlers for 0x21, 0x22, 0x23,
  0x94 over the model of `client/model.md`.
- **Related specs:** `client/model.md` §1–§3 (unit table, local player);
  `client/msg-units.md` §1.1 (a player's skill list is allocated at
  creation); `skills/levels.md` §1 (entry fields, `skill_level`,
  `highest_entry`), §6 (`max_level`); `items/inventory.md` §5.5 (server
  side of 0x22: tome and scroll quantities); `client/stat-lists.md`
  (what the passive refresh adds to the stat totals); `ui/panels.md`
  §10 (skill tree).

Owned ids: 0x21, 0x22, 0x23, 0x94.

## Summary

Every client unit with skills has a skill list (unit +0xA8), the same
structure and the same shared functions as on the server. At join the
server sends 0x94 with the player's native skills and levels, then 0x22
for tome / scroll quantities and 0x23 for the left and right skill.
0x21 sets one skill's hard-point level (spent skill points, quest
rewards). Assigning a skill also refreshes its passive state's stat
list, which feeds the character totals (`client/stat-lists.md`).

## Inputs

| Name | Type | Source |
|---|---|---|
| message | id + bytes | `client/bridge.md` §2, §6 |
| unit | set S lookup (`client/model.md` §2 rule 2) | model |
| tables | `skills` (record 0x23C bytes: +0x10 `anim`, +0x11 `monanim`, +0x80 `aurastate`, +0x94 `passivestate`, +0x96 `passiveitype`, +0x98 `passivestat1–5`, +0xA4 `passivecalc1–5`; `data/fields.tsv`), skill count `[0x00744304]` +0xBA0 | `data/loading.md` |

## Outputs / state changes

`ClientUnit.skills` (§1) of player units; passive-state stat lists on
the unit (§2 rule 4, consumed by `client/stat-lists.md`); a skill-tree
redraw flag for 0x21. No `client/bridge.md` §10 output.

## Rules

### 1. The client skill list (unit +0xA8)

1. d2rs: `ClientUnit.skills: Option<SkillList>`, added by this spec
   (`client/model.md` §1 rule 1). `None` when the unit has no list
   (+0xA8 null): every operation below then does nothing. Players get a
   list at creation (`client/msg-units.md` §1.1, `0x006438B0`); monsters
   too (§1.2 step 7).
2. `SkillList` holds the entries in list order (the chain from +0x04;
   new entries are appended at the tail) and three entry references:
   left skill (+0x08), right skill (+0x0C), current skill (+0x10).
3. Entry (0x40 bytes, zeroed at creation): skill id (via the record
   pointer +0x00), mode (+0x08), base level (+0x28), level bonus
   (+0x2C), quantity (+0x30), owner GUID (+0x34; −1 = native), charges
   (+0x38), has-charges flag (+0x3C). Field meanings and the level
   formula: `skills/levels.md` §1 (single owner).
4. "The entry of (skill, owner)" = the first entry in list order with
   that skill id and owner GUID (`0x006439B0(unit, skill, owner)`); the
   native entry is owner −1. Every handler here asks for owner −1.
5. Where the local player's skills come from: native entries and levels
   from 0x94 (join) and 0x21 (later changes); quantities of tome and
   scroll skills from 0x22; left / right from 0x23. Item-granted
   entries (owner = item GUID, charges) and the level bonus +0x2C have
   other client writers (open questions 1, 2).

### 2. Shared skill-list operations

The same code runs on the server; this spec states it for the client
handlers (the server specs link here for the steps).

1. **Add** `0x00647110(unit, skill)`: no unit, skill outside 0 …
   count − 1, or no list → none. The skill's passive state
   (`0x00643690`), if any, is turned on (`0x00639DB0(unit, state, 1)`).
   A native entry exists → its base += 1 when base < `max_level(skill)`
   (`skills/levels.md` §6 rule 1) or the unit is a monster; then refresh
   (rule 4); return it. Else a new entry: mode := `monanim` for a
   monster; 0x10 for a player of class 6 and skill 5; else `anim`; base
   1, owner −1; appended at the tail; refresh; return it.
2. **Assign** `0x00647280(unit, skill, level, remove, …)`:
   1. level ≠ 0: entry := the native entry, else add (rule 1); if an
      entry results, base := level. Refresh (even when no entry).
   2. level = 0, remove ≠ 0: remove the native entry (`0x00646FD0`: its
      passive state off, and a left / right / current reference to it
      is cleared (left and right reset to skill 0 owner −1 through
      `0x00643BC0` / `0x00643C50`, current := none) before it is
      unlinked); refresh.
   3. level = 0, remove = 0: the native entry, else add then look up
      again; none → stop (no refresh); base := 0; refresh.
3. **Select** `0x00643BC0(unit, skill, owner)` (left) /
   `0x00643C50` (right): skill outside the table → fatal 0x668; the
   entry of (skill, owner) found → left (right) := it; not found →
   unchanged.
4. **Refresh** `0x00646D60(unit, skill)` (passive-state stat list): only
   when the skill's `passivestate` p > 0 and in range. E :=
   `highest_entry(unit, skill)` (`skills/levels.md` §1). When E exists
   and (`aurastate` ≤ 0 or the unit does not have state `aurastate`):
   L := `skill_level(unit, E, 1)`; the state list of p (`0x00643620`:
   found, or created with the unit as owner, state p, and attached to
   the unit, `sim/stat-lists.md` §8.1) is updated only when its stat
   351 ≠ L: for i = 1…5 while `passivestat_i` is a valid stat: set
   (`0x00627150`) stat `passivestat_i` with layer `passiveitype` (0 when
   ≤ 0) to `eval(passivecalc_i, skill, L)` (`0x00646CA0`); then stat 350
   := skill, stat 351 := L (layer 0); mark state p for update
   (`0x00639E30`). Otherwise (no E, or the aura state is on) the state
   list of p, if any, is detached and freed (`0x006277E0`,
   `0x00626CD0`). L = 0 removes the list (`0x00643620` with level 0).

### 3. 0x94 BaseSkillLevels (`0x0045DD60`)

1. Layout: count n u8@1, player GUID u32@2, then n entries of 3 bytes
   from @6: skill u16, level u8. Size 6 + 3n (`sim/server-messages.tsv`
   `u8@1*3+6;min=9`).
2. Look up (0, GUID) (players only, `client/model.md` §2 rule 2); none
   → nothing. For each entry in order: assign (§2 rule 2) (skill, level,
   remove 0).
3. No skill-tree flag is set (contrast 0x21).

### 4. 0x21 UpdateItemOSkill (`0x0045DCD0`)

1. Layout (12 bytes, server `0x0053C4A0`): unit type u8@1, remove flag
   u8@2, GUID u32@3, skill u16@7, base level u8@9, bonus level u8@10
   (the server's `bonus_level` of the native entry, `skills/levels.md`
   §1), byte @11 never written by the sender (stack garbage).
2. Look up (u8@1, GUID); none → nothing. Assign (skill, u8@9, remove
   u8@2) (§2 rule 2), then the skill-tree redraw flag `[0x007C0C3C]` :=
   0 (`0x004AA8F0`). Bytes @10 and @11 are not read.

### 5. 0x22 UpdateItemSkill (`0x0045DDB0`)

1. Layout (12 bytes, server `0x0053C520`): unit type u8@1, byte @2 not
   written by the sender, GUID u32@3, skill u16@7, quantity u8@9, byte
   @10 not written, flag u8@11 = 1 when the unit has state 7 at send.
2. Flag ≠ 0 → nothing. Else look up (0, GUID) (players only, whatever
   u8@1 says); none → nothing. The native entry of the skill; none →
   fatal 0xAD6 (bridge: handler error). Its quantity (+0x30) := u8@9
   (`0x00645120`).

### 6. 0x23 SetSkill (`0x0045DE10`)

1. Layout (13 bytes, server `0x0053C590`): unit type u8@1, GUID u32@2,
   hand u8@6 (≠ 0 left, 0 right), skill u16@7, owner GUID u32@9.
2. Look up (0, GUID) (players only); none → nothing. Hand ≠ 0 → select
   left (skill, owner), else select right (§2 rule 3).

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| entry size | 0x40 bytes | `0x006471A5` |
| skills record | 0x23C bytes; count at `[0x00744304]` +0xBA0 | `0x0064713A` |
| passive stats | 5 (`passivestat1–5`); markers 350 (skill), 351 (level) | `0x00646D60` |
| fatal asserts | 0xAD6 (0x22 without the native entry), 0x668 (select, bad skill) | §5, §2 |
| skill-tree flag | `[0x007C0C3C]` := 0 | `0x004AA8F0` |

## Randomness

None.

## Edge cases & original bugs

- 0x94 with level 0 creates the entry and leaves it at base 0 (§2 rule
  2.3); 0x21 with level 0 and the remove flag deletes it.
- 0x22 and 0x23 ignore their unit-type byte and look up players only.
- The add of §2 rule 1 lets a monster exceed `max_level`.
- 0x22 / 0x21 carry bytes the server never writes (@2, @10 / @11); they
  are not read and differ between recordings (0x22 @2 = 0x19, @10 =
  0x55 or 0x41).

## Test vectors

"A" = `-015956`, "B" = `-022633` (join, frame 2).

| Input | Expected | Source |
|---|---|---|
| `94 0a 01000000 0000 01 0200 01 0100 01 d900 01 da00 01 db00 01 dc00 01 0400 01 0500 01 0300 01` on player (0, 1) with an empty list | entries in order: skills 0, 2, 1, 217, 218, 219, 220, 4, 5, 3; each base 1, owner −1 | A seq 273 |
| `94 08 01000000 …` (8 entries ending `0300 01`), then 0x22 `22 00 19 01000000 db00 01 41 00` | 8 entries; skill 219 quantity 1 | B seq 227 |
| `22 00 19 01000000 db00 00 55 00` | skill 219 quantity 0 | A seq 273 |
| `22 …` with byte @11 = 1 | no change | §5 rule 2 |
| `22` for a skill the player lacks | handler error (fatal 0xAD6) | synthetic |
| `23 00 01000000 01 0000 ffffffff`, then `23 00 01000000 00 0000 ffffffff` | left = skill 0, right = skill 0 | A seq 274 |
| `23 00 01000000 00 2400 ffffffff` without a skill-36 entry | right unchanged | B seq 227 (list state synthetic) |
| `21 00 00 01000000 2400 00 01 05` on a player without skill 36 | entry 36 added, then base 0; skill-tree flag 0 | B seq 46370 |
| `21 00 00 01000000 2400 01 01 05` | skill 36 base 1 | B seq 61126 |
| `21 00 01 01000000 2400 00 00 00` with entry 36 | entry 36 removed | synthetic, §2 rule 2.2 |
| 0x94 for an unknown GUID | no change | synthetic |

## Provenance

1.14d `Game.exe` (spec session 2026-10-07, `tools/ghidra/disasm.py` and
the Ghidra exports): handlers `0x0045DD60`, `0x0045DCD0`, `0x0045DDB0`,
`0x0045DE10`; senders `0x0053C4A0` (with `0x00644300` → `0x00644180`),
`0x0053C520`, `0x0053C590`; list functions `0x006439B0`, `0x00647110`,
`0x00647280`, `0x00646FD0`, `0x00643BC0`, `0x00643C50`, `0x00645120`,
`0x00646D60`, `0x00643620`, `0x004AA8F0`. Recorded join messages of A
and B, split with the S→C size rule (0x94 n = 10 and 8 in A and B; B's
later 0x94 at seq 127716 has n = 9 with skill 36 added). D2MOO 1.10f
`D2SkillStrc` names the entry fields (hint; offsets read in 1.14d).

## Open questions

1. Item-granted entries on the client (owner = item GUID, charges
   +0x38, has-charges +0x3C): the set-charges function `0x00643B70` has
   no direct caller in the export; find the client path (item equip,
   `client/stat-lists.md` §2) and the message that carries charges.
   Settle with a recording equipping a charged item.
2. The level bonus +0x2C on the client: writers `0x00647AA0` (set,
   called from `0x004C6140` and `0x004D88A0`) and `0x00647B20` (add,
   from `0x004C7990`); which stats or states drive them (oskills,
   `item_singleskill`).
3. Skill-tree inputs (`ui/panels.md` §10 r3): which entry fields the
   tree shows (base, bonus, quantity) and the redraw flag
   `[0x007C0C3C]`; Phase 6 UI spec.
