# Spec: Client — Skill list and skill messages (0x21, 0x22, 0x23, 0x94, 0x99, 0x9A, 0xA3)

- **Status:** draft: handlers and the shared skill-list functions read in
  the 1.14d `Game.exe` (addresses below); layouts checked against the
  join of both single-player recordings
  (`traces/raw/20261006-015956-packets.jsonl`, `-022633-packets.jsonl`);
  unverified: no executable check runs it yet. 2026-10-07: §7–§8
  (skill events 0x99, 0x9A, 0xA3) read the same way; none recorded.
- **Target version:** 1.14d
- **Crate/module:** `d2-client::bridge` handlers for 0x21, 0x22, 0x23,
  0x94, 0x99, 0x9A, 0xA3 over the model of `client/model.md`.
- **Related specs:** `client/model.md` §1–§3 (unit table, local player);
  `client/msg-units.md` §1.1 (a player's skill list is allocated at
  creation); `skills/levels.md` §1 (entry fields, `skill_level`,
  `highest_entry`), §6 (`max_level`); `items/inventory.md` §5.5 (server
  side of 0x22: tome and scroll quantities); `client/stat-lists.md`
  (what the passive refresh adds to the stat totals); `ui/panels.md`
  §10 (skill tree).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 47–56 |
| Inputs | 57–64 |
| Outputs / state changes | 65–73 |
| Rules | 74–75 |
|   1. The client skill list (unit +0xA8) | 76–117 |
|   2. Shared skill-list operations | 118–253 |
|   3. 0x94 BaseSkillLevels (`0x0045DD60`) | 254–263 |
|   4. 0x21 UpdateItemOSkill (`0x0045DCD0`) | 264–273 |
|   5. 0x22 UpdateItemSkill (`0x0045DDB0`) | 274–283 |
|   6. 0x23 SetSkill (`0x0045DE10`) | 284–290 |
|   7. 0x99 / 0x9A skill events (`0x0045DE80` / `0x0045DEC0` → `0x004CA060`) | 291–334 |
|   8. 0xA3 skill do (`0x0045D5E0`) | 335–348 |
|   9. 0x93 skill bonus by element and page (`0x0045DD10` → `0x004C7990`) | 349–382 |
|   10. 0xA5 skill end on a unit (`0x0045D6A0`) | 383–397 |
| Constants & data dependencies | 398–409 |
| Randomness | 410–413 |
| Edge cases & original bugs | 414–423 |
| Test vectors | 424–449 |
| Provenance | 450–475 |
| Open questions | 476–510 |
<!-- /index -->

Owned ids: 0x21, 0x22, 0x23, 0x94, 0x99, 0x9A, 0xA3; §9–§10: 0x93, 0xA5.

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
redraw flag for 0x21. Outputs (`client/bridge.md` §10): `SkillEvent`
(0x99, 0x9A, §7) and `SkillDo` (0xA3, §8), both for the client effect
layer; the skill events write no model state. 0x93 writes level bonuses
(§9); 0xA5 turns state 18 off and emits `SkillEndFx` (§10).

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
   entries (rule 6); the level bonus +0x2C: §2 rule 7.
6. **Item-granted entries** (2026-10-08; answers open question 1). No
   message carries charges or item skills. The only client writer of
   charge entries is the local and remote players' stat-list callback
   `0x004609F0` (installed by the player init `0x00460BF0` at
   `0x00460C38` through `0x00626D40(U, 0, 0x004609F0, 0)`; rules in
   `client/stat-lists.md` §1 r4): stat 204 `item_charged_skill`
   (`fCallback` = 1) → `0x00647530` → `0x00647320(O, item GUID, s, l,
   c, remove)`, the set / remove of `skills/levels.md` §7.6 (owner of
   the entry fields: base +0x28, charges +0x38, has-charges +0x3C,
   owner +0x34); stats 97 / 107 assign native entries through §2 r2.
   The callback runs when an item's stat 204 changes inside a list
   chain that reaches the player's list: the item's list attached by
   equip (`client/stat-lists.md` §2 r2) or detached (§2 r4.1), and
   0x3E stat 204 (`client/msg-stats-items.md` §5 r1.1, whose write
   `0x00627220` → `0x00627170` carries the item as callback unit). The
   charge values come from the item's own stream (`items/bitstream.md`
   §4.6) and 0x3E. The setter `0x00643B70` is unreferenced in the
   image (no call, no pointer: a scan of the image for its address
   finds none).

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
   the unit, `sim/stat-lists.md` §8.1; arguments `client/stat-lists.md`
   §4 r3) is updated only when its stat
   351 ≠ L: for i = 1…5 while `passivestat_i` is a valid stat: set
   (`0x00627150`) stat `passivestat_i` with layer `passiveitype` (0 when
   ≤ 0) to `eval(passivecalc_i, skill, L)` (`0x00646CA0`); then stat 350
   := skill, stat 351 := L (layer 0); mark state p for update
   (`0x00639E30`). Otherwise (no E, or the aura state is on) the state
   list of p, if any, is detached and freed (`0x006277E0`,
   `0x00626CD0`). L = 0 removes the list (`0x00643620` with level 0).
5. **Remove in detail** `0x00646FD0` (unit in EBX, skill id s, flag d;
   2026-10-08, read for `client/model.md` §17 r4; rule 2.2 passes its
   `remove` argument as d, `0x006470F0(unit, s)` passes d = 1). No unit,
   no skill list (+0xA8) or an empty list → nothing. In order: the
   passive state of s, if any, off (`0x00639DB0(unit, state, 0)`); left
   (+8) is the native entry of s (skill id = s, owner +0x34 = −1) →
   select left (0, −1) (rule 3); same for right (+0xC, `0x00643C50`);
   current (+0x10) is that entry → current := none. Then the native
   entry of s in the list: none → refresh (rule 4) only; else with d = 0
   it is unlinked and freed; with d ≠ 0 its base (+0x28) −= 1 and it is
   unlinked and freed only when the base is now < 1; then refresh
   (rule 4) for s.
6. **A hand left on the removed entry** (2026-10-08; answers
   `docs/handoff/impl-client-msgs-3.md` §3 Q6). The hand resets of rule
   5 run before the unlink and only re-point the hand when select finds
   the entry (0, −1) (rule 3: not found → unchanged). Two cases leave
   left (+8) or right (+0xC) pointing at the entry that rule 5 then
   frees (`0x0040B480`, `Skills.cpp` line 0x4AC): (a) the unit has no
   native skill-0 entry; (b) s = 0, so select finds the entry being
   removed (still linked) and keeps it. 1.14d keeps the freed pointer
   (no test, no assert); the next read of that hand reads freed memory
   (undefined). With d ≠ 0 and the base still ≥ 1 after the decrement
   nothing is freed and the hand stays valid. **d2rs:** an assign or
   remove that would leave a hand on a freed entry is refused as a
   handler error (`client/bridge.md` §2.4; recorded, model unchanged),
   the same choice as `client/model.md` §9 rule 5 for a 1.14d access
   violation. Whether a 1.14d server ever sends such a removal is not
   established.
7. **Level bonus +0x2C** (2026-10-08; answers open question 2). Only
   two functions write it, both with a native entry (owner −1) only;
   entries are allocated zeroed (rule 1) and assign (rule 2) never
   touches +0x2C.
   1. **Set** `0x00647AA0(unit, skill, v)`: no unit → nothing. E := the
      native entry; none and v ≤ 0 → nothing; none and v > 0 → add
      (rule 1), then the native entry is looked up again (adding once
      more when still none) and, when found, its base := 0 and refresh
      (rule 4). Then, when E is native: bonus := v, refresh.
   2. **Add** `0x00647B20(unit, skill, v)`: the same, with bonus += v
      and a negative result clamped to 0; no refresh after the add.
   3. **Split level** (callers below): with M := `maxlvl` of the skill
      (skills +0x12C, `skills/levels.md` §1; ≤ 0 or no row → 20;
      `0x004AA8B0`) and a level L: L > M → assign (skill, M, remove 0)
      then set (skill, L − M); else assign (skill, L, remove 0) and the
      bonus keeps its old value.
   Writers and when (`0x00647AA0` call sites `0x004C6295`, `0x004C6326`,
   `0x004D892F`; `0x00647B20` call site `0x004C7AC7`; no server call):
   - **Client skill start** `0x004C6140(U, skill S in EAX, owner o,
     level L)`, from the skill event (§7 r4.3 via `0x004C6660`, o = −1)
     and the player and monster mode machines (`0x004C6EB0` from
     `0x00461314`, `0x00480A34`, `0x004B0CA4`; `0x004C6F40` from
     `0x00461347`, `0x004B0D24`; S, o, L = request record entries 0, 1,
     4; `client/model.md` §8). (a) When U's current skill entry
     (`0x00620250`) is not of S: the entry (S, o) is looked up; none →
     add S, look (S, o) up again; if found, U is the local player and o
     = −1 → `0x00644660(entry, 8)`; then split level (rule 7.3) with L;
     still no entry (S, o) → the start returns 0. Then current := the
     entry (`0x00620210`). (b) o ≠ −1: the entry's charges check
     `0x00643B00` must pass, else the start returns 0. Then U not the
     local player → split level with L again (before the start function
     and sounds); the local player instead takes L := its level with
     bonuses (`0x006442A0(U, E, 1)`). So for remote units the event's level is
     what the client stores, the part above `maxlvl` in the bonus.
   - **State set-function 3** `0x004D88A0(U, state)` (setfunc table
     `0x0072A690` entry 3, `client/stat-lists.md` §3 r6.2): U not the
     local player and U has the state's list: S := its stat 350, L :=
     its stat 351; when the level with bonuses of U's native entry of S
     (0 when none) ≠ L → split level (rule 7.3) with L.
   - **0x93** (§9 rule 3): add with the message's bonus on each
     qualifying native entry.
   The local player's bonus thus changes through 0x93 and through its
   own skill start only in step (a) (the entry (S, o) missing); a
   remote unit's through every skill start, set-function-3 states and
   0x93.

8. **Native skills of a player** `0x00647EE0(unit)` (2026-10-08; called
   right after the list is created by the client player init
   `0x00460BF0` at `0x00460C31` and by the server player init
   `0x005348C0` at `0x0053490E`):
   1. The list already holds the native entry of skill 0 (skill id 0,
      owner +0x34 = −1) → skip to step 3.
   2. The unit's class (+0x04) outside 0 … charstats count − 1 (data
      tables +0xBC8; record 0xC4 bytes at +0xBC4) → nothing (no select
      either). Else add skill 0 (rule 1), then for each of the ten
      `charstats` words `Skill 1`…`Skill 10` (record +0xAE, i16, in
      order): 0 ≤ id < skill count (data tables +0xBA0) → add it (rule
      1); others are skipped.
   3. Left (+8) empty (`0x00620190`) → select left (0, −1) (rule 3);
      right (+0xC) empty (`0x006201D0`) → select right (0, −1).
   So every player starts with Attack (0) and its class's ten
   `charstats` skills (1.14d: Kick, Throw, Unsummon, Left Hand Throw,
   Left Hand Swing and the scroll / book skills), each base 1 owner −1,
   with Attack in both hands, before any S→C 0x94 / 0x23. Without it a
   click has no left skill and sends nothing (`ui/controls.md` §6 r8).

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

### 7. 0x99 / 0x9A skill events (`0x0045DE80` / `0x0045DEC0` → `0x004CA060`)

The server sends these for a unit's pending skill records
(`sim/intents-events.md` §3.5 rule 5, §7.9 rule 2): a skill event at a
level on a unit, toward a unit (0x99, the 16-byte form) or a point
(0x9A, the 17-byte form).

1. **0x99** layout (16 bytes): unit type u8@1, GUID u32@2, skill
   u16@6, level u8@8, target type u8@9, target GUID u32@10, w u16@14.
   The unit (u8@1, GUID) and the target (u8@9, u32@10) are looked up
   in S; either missing → nothing. Else `0x004CA200(unit, skill, level,
   target, w)` → `0x004CA060(unit, skill, level, target, 0, 0, w)`.
2. **0x9A** layout (17 bytes): unit type u8@1, GUID u32@2, skill u32@6
   (the handler reads the low u16), level u8@10, x u16@11, y u16@13, w
   u16@15. Unit missing → nothing; x = 0 or y = 0 → nothing
   (`0x004CA230`). Else `0x004CA060(unit, skill, level, none, x, y, w)`.
3. Model state written: none. Each emits one `SkillEvent` output
   {unit key, skill, level, target key or point (x, y), w}, consumed by
   the client effect layer, which runs rule 4 at delivery against the
   units it resolves from the keys (open question 4).
4. **Client skill event** `0x004CA060(U, skill, level, target T,
   x, y, w)`:
   1. Skill outside 0 … count − 1 → nothing (returns 0).
   2. Save U's current target unit (`0x004648F0`), its path target
      point (`0x00648A00` / `0x00648A10`) and bit 0x40 of U's flags
      +0xC4. Set U's target: T (`0x00620C10`) or the point (x, y)
      (`0x00648AD0`).
   3. When the skill has `ItemCltCheckStart` (bit 34: record byte +8
      & `[0x006CE270]` = 4): if T exists, T is dead (`0x00464820`:
      unit flag +0xC4 bit 0x10000, or a player in mode 0 or 17, or a
      monster in mode 0 or 12) and T has state 118, state 118 is turned off
      on T (`0x00639DB0(T, 118, 0)`) for the call; the client start
      `0x004C6660(U, skill, level, −1)` → `0x004C6140` (skill start,
      `audio/triggers.md` §8 r1); state 118 is turned back on after a
      non-zero start; a zero start skips step 4.
   4. `ItemCastSound` (+0x122) ≥ 0 → sound request on U
      (`0x004B9A00`); `ItemCastOverlay` (+0x124) valid (< the overlay
      count, data +0xBC0) → overlay on U (`0x00470390(U, overlay, 2,
      …)`); then the client do `0x004C6680(U, skill, level, w)` (the
      skill's `cltdofunc` +0xF4 through table `0x00727BA8`, its sounds
      `audio/triggers.md` §8 r2).
   5. Restore U's target (unit, or the saved point) and its flag bit
      0x40 (set or cleared as saved).

### 8. 0xA3 skill do (`0x0045D5E0`)

1. Layout (24 bytes; server record `skills/bodies-2.md` §2.21, builder
   `0x0053C0E0`): v u8@1, skill u16@2, level i16@4, unit type u8@6,
   GUID u32@7, target type u8@0xB, target GUID u32@0xC, x u32@0x10, y
   u32@0x14.
2. Unit missing → nothing. Target looked up (may be none). Skill
   outside 0 … count − 1 → nothing. The skill's `progressive` bit
   (bit 2, record byte +4 & `[0x006CE270]`) → `0x004C6AC0(U, T, skill,
   level, x, y, v)`, else `0x004C6930(U, T, skill, level, x, y, v)`
   (client do / target, `audio/triggers.md` §8 r2).
3. Model state written: none. One `SkillDo` output {unit key, target
   key or none, skill, level, x, y, v} for the client effect layer.

### 9. 0x93 skill bonus by element and page (`0x0045DD10` → `0x004C7990`)

1. Layout (8 bytes; sender `0x0053C6F0`): player GUID u32@1, bonus
   i8@5 (u8@5 > 0x80 → u8@5 − 0x100; 0x80 stays +128), element u8@6,
   page u8@7.
2. Player (0, GUID) absent → nothing. Bonus 0 → fatal 0x96B; no skill
   list → fatal 0x96D (bridge: handler errors).
3. For each entry E of the list, in order (the next entry is read
   before E is handled), with skill s := E's skill id; E qualifies when
   all hold:
   - s is a valid skill row and its `skilldesc` (+0x194) is a valid
     skilldesc row D;
   - E's level with bonuses `0x006442A0(U, E, 1)` > 0;
   - s is `enhanceable` (skills bit 17, `0x00645FB0`);
   - element u8@6 = 0, or `EType` (+0x1DC) = u8@6;
   - page u8@7 = 4, or D's `skillpage` (+2, signed) = u8@7 + 1;
   - E is native (owner +0x34 = −1, `0x00643AD0`).
   A qualifying E gets the level bonus `0x00647B20(U, s, bonus)`
   (+0x2C, `skills/levels.md` §1; it adds a native entry when none
   exists and the bonus is ≥ 1); then, when the native entry of s now
   exists with level-with-bonuses 0, it is removed (`0x006470F0` →
   `0x00646FD0(entry, 1)`, §2 rule 2.2).
4. Then `0x00646F20(U)`: every skill of the passive list (data tables
   +0xBB4, count +0xBB0) whose `passivestate` (+0x94) > 0 and which the
   unit has (`0x00643810`) and whose state is on (`0x00639DF0`) is
   refreshed (§2 rule 4).
5. Model: the entries' level bonus and the passive-state lists. No
   output.

d2rs: `units::messages::skill_bonus` builds the message; no caller of
`0x0053C6F0` is named by any spec, so nothing sends it yet. PROVISIONAL:
the layout is the TSV's; the senders (an item or shrine bonus?) are
unknown; settled by REC-415 (a static caller search of `0x0053C6F0`).

### 10. 0xA5 skill end on a unit (`0x0045D6A0`)

1. Layout (8 bytes; `sim/intents-events.md` §3.5, pending record
   `skills/bodies-2.md` §2.13): unit type u8@1, GUID u32@2, skill
   u16@6.
2. Unit absent, or skill ≥ the skill count → nothing.
3. By the skill's `srvdofunc` (+0x2E, read signed; jump table
   `0x0045D738` / byte map `0x0045D748`, read from the image): 67 →
   `0x004CA000(U)`; 76 → `0x004C9420(U)`; 77, 78 → `0x004C8B80(U)`; any
   other value → none. Each is one `SkillEndFx` output {unit key, skill,
   srvdofunc} for the client effect layer (Phase 6).
4. Then, for every skill in range, state 18 off on U
   (`0x00639DB0(U, 18, 0)`, `client/stat-lists.md` §3). Model: U's
   states.

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| entry size | 0x40 bytes | `0x006471A5` |
| skills record | 0x23C bytes; count at `[0x00744304]` +0xBA0 | `0x0064713A` |
| passive stats | 5 (`passivestat1–5`); markers 350 (skill), 351 (level) | `0x00646D60` |
| fatal asserts | 0xAD6 (0x22 without the native entry), 0x668 (select, bad skill) | §5, §2 |
| skill-tree flag | `[0x007C0C3C]` := 0 | `0x004AA8F0` |
| skill flag masks | `[0x006CE270]` = 0x04 (byte +8: bit 34 `ItemCltCheckStart`; byte +4: bit 2 `progressive`) | image byte, `data/fields.tsv` `skills` |
| state | 118 (lifted around the start check) | `0x004CA10B` |

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
| 0x93 `93 01000000 01 00 04` on player (0, 1) with native entries of an `enhanceable` skill (level 1, desc page 1) and a non-enhanceable one | the first gets bonus +1; the second unchanged | synthetic, §9 |
| 0x93 with bonus byte 0 | handler error (fatal 0x96B) | synthetic, §9 r2 |
| 0x93 bonus byte 0x80 | bonus +128 | synthetic, §9 r1 |
| 0xA5 for a skill with `srvdofunc` 5 | no output; state 18 off on the unit | synthetic, §10 |
| `23 00 01000000 00 2400 ffffffff` without a skill-36 entry | right unchanged | B seq 227 (list state synthetic) |
| `21 00 00 01000000 2400 00 01 05` on a player without skill 36 | entry 36 added, then base 0; skill-tree flag 0 | B seq 46370 |
| `21 00 00 01000000 2400 01 01 05` | skill 36 base 1 | B seq 61126 |
| `21 00 01 01000000 2400 00 00 00` with entry 36 | entry 36 removed | synthetic, §2 rule 2.2 |
| 0x94 for an unknown GUID | no change | synthetic |
| `99 01 05000000 4000 03 00 01000000 0000`, (1, 5) and (0, 1) in S | no model change; `SkillEvent` {(1, 5), skill 64, level 3, target (0, 1), w 0} | synthetic, §7 r1 |
| `99 …` with the target missing | nothing | §7 r1 |
| `9a 01 05000000 40000000 03 0000 2000 0000` | nothing (x = 0) | §7 r2 |
| `a3 00 4000 0300 01 05000000 06 ffffffff 00000000 00000000`, (1, 5) in S | `SkillDo` {(1, 5), none, 64, 3, 0, 0, 0} | synthetic, §8 |

## Provenance

1.14d `Game.exe` (spec session 2026-10-07, `tools/ghidra/disasm.py` and
the Ghidra exports): handlers `0x0045DD60`, `0x0045DCD0`, `0x0045DDB0`,
`0x0045DE10`; senders `0x0053C4A0` (with `0x00644300` → `0x00644180`),
`0x0053C520`, `0x0053C590`; list functions `0x006439B0`, `0x00647110`,
`0x00647280`, `0x00646FD0`, `0x00643BC0`, `0x00643C50`, `0x00645120`,
`0x00646D60`, `0x00643620`, `0x004AA8F0`. §7–§8: `0x0045DE80`,
`0x0045DEC0`, `0x004CA200`, `0x004CA230`, `0x004CA060`, `0x004C6660`,
`0x004C6680`, `0x004648F0`, `0x00464820`, `0x0045D5E0`, `0x0053C0E0`;
`[0x006CE270]` read from the image; skill columns by offset from
`data/fields.tsv`. Recorded join messages of A
and B, split with the S→C size rule (0x94 n = 10 and 8 in A and B; B's
later 0x94 at seq 127716 has n = 9 with skill 36 added). D2MOO 1.10f
`D2SkillStrc` names the entry fields (hint; offsets read in 1.14d).

Area 4 session (2026-10-07): §9 `0x0045DD10`, `0x004C7990`,
`0x00647B20`, `0x006470F0`, `0x00646F20`, `0x00645FB0` (byte +6 & 2 =
bit 17 `enhanceable`, mask table `0x006CE268`), `0x00643AD0`,
`0x006442A0`; §10 `0x0045D6A0` and its jump tables.

Gap pass (2026-10-08, PC 1 lane D): §1 r6 from `0x00460BF0`
(`0x00460C38`), `0x004609F0` (jump tables `0x00460B70` / `0x00460B5C`),
`0x00647530`, `0x00647320`, `0x00627220` → `0x00627170`; image scan for
the dword `0x00643B70` (none).

## Open questions

1. *Answered (2026-10-08)*: §1 r6 (the player stat callback
   `0x004609F0`, stat 204 → `0x00647320`; no charge message;
   `0x00643B70` unreferenced). REC-09 stays as the check of the
   rule, no longer a settling capture. Original question: item-granted
   entries on the client (owner = item GUID, charges +0x38, has-charges
   +0x3C): the set-charges function `0x00643B70` has no direct caller
   in the export; find the client path (item equip,
   `client/stat-lists.md` §2) and the message that carries charges.
2. *Answered (2026-10-08)*: §2 rule 7 (all writers, the split level,
   when each runs). Original question: the level bonus +0x2C on the client: writers `0x00647AA0` (set,
   called from `0x004C6140` and `0x004D88A0`) and `0x00647B20` (add,
   from `0x004C7990`); which stats or states drive them (oskills,
   `item_singleskill`). *Partly answered*: the `0x004C7990` path is
   S→C 0x93 (§9: element / page bonus on `enhanceable` skills).
3. Skill-tree inputs (`ui/panels.md` §10 r3): which entry fields the
   tree shows (base, bonus, quantity) and the redraw flag
   `[0x007C0C3C]`; Phase 6 UI spec.
4. The skill events (§7, §8) run 1.14d's client skill code at receive;
   the effect layer runs it at delivery, after every message of the
   frame. They differ when a later message of the same frame changes
   the unit, its target or state 118 (`client/bridge.md` §10 r3).
   Settle with the effect layer's spec: either the layer runs inside
   the receive for these outputs, or a recording shows no such frame.
   The bodies of `0x004C6140`, `0x004C6680`, `0x004C6930`,
   `0x004C6AC0` (client missiles, overlays) are owned by the client
   skill effect spec (not written).
5. The client functions of 0xA5 (`0x004CA000`, `0x004C9420`,
   `0x004C8B80`) and which skills have `srvdofunc` 67, 76, 77, 78:
   Phase 6 client skill effect spec; and the name of state 18.
6. *Answered (2026-10-08)* (`docs/handoff/impl-client-msgs-3.md` §3
   Q6): a remove that leaves the left or right hand on the freed entry
   (no native skill-0 entry, or skill 0 itself): §2 rule 6.
