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

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 38–49 |
| Inputs | 50–58 |
| Outputs / state changes | 59–63 |
| Rules | 64–65 |
|   1. The list of a client unit | 66–101 |
|   2. Items | 102–188 |
|   3. States (S→C 0xA7, 0xA8, 0xA9) | 189–491 |
|   4. Skills | 492–523 |
| Constants & data dependencies | 524–534 |
| Randomness | 535–539 |
| Edge cases & original bugs | 540–547 |
| Test vectors | 548–559 |
| Provenance | 560–572 |
| Open questions | 573–612 |
<!-- /index -->

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
   the server's `0x0055B800` (its effects: rule 4).
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
4. **Client callback** `0x004609F0` (2026-10-08). Called as
   `sim/stat-lists.md` §7.1 says (only for stats with `fCallback`):
   ECX game, EDX owner O (list +0x44), stack unit I (the propagation's
   unit, e.g. the item), key k, old, new; `ret 0x10`. O none → nothing.
   By stat = k >> 16 (jump tables `0x00460B70` / `0x00460B5C`, stats
   89…204); layer = k & 0xFFFF:

   | Stat | Effect |
   |---|---|
   | 89 `item_lightradius` | `0x00460930(O, new)`: O a player or monster with a light (+0x64): radius := base + new, 0 when ≤ 0 (`0x004742D0`); base 13 for a player, for a monster max(`monstats2` `Light` +0x11E, the component light `0x0063EBD0`) (no monstats2 row → nothing); `render/lighting.md` §6.2 |
   | 90 `item_lightcolor` | `0x004609A0(O, new)`: O a player or monster with a light: new = 0 → R = G = B = 255, else R, G, B = bits 16–23, 8–15, 0–7 (`0x00474390`) |
   | 97 `item_nonclassskill`, 107 `item_singleskill` | O a player only; s = layer; E = O's native entry of s (`0x006439B0(O, s, −1)`). new = 0: E exists and `skill_level(O, E, 0)` < 1 → remove s (`0x006470F0(O, s)`). new ≠ 0: no E, and (stat 97, or s's skills record (`0x0045C4B0`) exists with `charclass` (+0x0C) = O's class) → assign `0x00647280(O, s, 0, 0)` (`client/msg-skills.md` §2 r2). No refresh, no message, no pet maximum, no left / right reselect (the server's `skills/levels.md` §7.1 does those) |
   | 204 `item_charged_skill` | O a player only; I none → nothing; s = layer >> shift, l = layer & mask (data +0xC6C / +0xC70); c := total(I, 204, layer) & 0xFF; c > 0 and I's list hangs under O (`0x00625820(I, 0)` = O) → `0x00647530(O, I's GUID, s, l, c, 0)` (set charges); else `0x00647530(O, I's GUID, s, l, 0, 1)` (remove); as `skills/levels.md` §7.6 without the pet maximum |
   | every other stat | nothing (no life / mana / stamina clamp, no state, class-skill or aura handling on the client) |

   The callback reads only O's type, class, light and skill list, so the
   client list needs no other client state to call it.

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
   the left and right skills saved at entry are re-selected (rule 3.1).
   1. In detail (2026-10-08; answers open question 3). `0x0045D3E0(P)`:
      P without an inventory (+0x60) → nothing. The skill list (+0xA8)
      is read without a null test (the local player always has one,
      `client/msg-units.md` §1.1). Saved: the skill id of the left
      entry (+8) and of the right entry (+0xC), 0 for no entry. Walk
      the inventory's item nodes (`0x0063B2C0` first, `0x0063DFA0`
      next, read before the item is handled; `0x0063DFD0` the node's
      unit); an item unit (type 4) is re-equipped when
      `0x00625820(item, &r)` ≠ 0: the item's list L (+0x5C) has a
      parent (L +0x34, `sim/stat-lists.md` §1) that is EXTENDED (flag
      0x80000000); the result is the parent's owner unit (+0x44), and r
      := 1 when L is not DYNAMIC (flag 0x40000000), else 0. So every
      item whose list is attached (active or parked) is detached and
      attached again with the same static / dynamic state
      (`sim/stat-lists.md` §8.2, §8.4); an item whose list has no
      parent (never attached, or detached, e.g. by rule 2.3) is
      skipped.
      Then select left (saved left id, owner −1) and select right
      (saved right id, owner −1) (`0x00643BC0` / `0x00643C50`,
      `client/msg-skills.md` §2 rule 3): a hand that was on an
      item-granted entry moves to the native entry of the same skill
      when one exists, else stays (select not found → unchanged).
4. Unequip and removal: an item unit freed by `client/model.md` §2
   rule 5 frees its lists (detach first, `sim/stat-lists.md` §8.2–§8.3);
   per-action detach (swap locations 11 / 12 through §8.4's swap rule):
   rule 4.1.
   1. **Detach per item action** (2026-10-08; answers open question 4).
      Every client path that takes an item out of an owner's grid, body
      location or belt runs, right after the removal (`0x0063AD90`,
      body slot `0x0063D2B0` / `0x0063BE30`, belt `0x0063C550`), the
      same two steps: owner a player → `0x0063BEF0(owner inventory)`
      (a getter whose result is dropped: no effect); then, when the
      item's flag 0x100 is clear (`0x006280A0`), detach its list
      (`0x006277F0(owner, item)` → `sim/stat-lists.md` §8.2). Sites:
      0x9C / 0x9D actions 0x03 (`0x004C2810`), 0x04 (`0x004C2AD0`,
      header flag 0x1 path), 0x05 (`0x004C2C80`), 0x07 (`0x004C3070`,
      the item taken off the other hand), 0x08 (`0x004C3380`), 0x09
      switch-out (`0x004C3760`), 0x0D (`0x004C3F60`), 0x0F
      (`0x004C42A0`), 0x10 (`0x004C44D0`), 0x11 (`0x004C4740`), 0x15
      (`0x004C4C70`, three sites: modes 0, 1, 2); the helpers
      `0x004C1C90` (from action 0x06 `0x004C2E90` and action 0x17
      through `0x004C3980`: the item leaving its old place) and
      `0x004C0EC0` (from 0x74, `0x00462F60`, `client/msg-units.md` §7
      r7, and from `0x00466CB0`; it detaches also when its third
      argument ≠ 0); S→C 0x92 (`0x004C23E0`); `0x004C1290` (a body
      slot cleared; from 0x7D `0x004C2270`, `client/msg-stats-items.md`
      §5 r4, and from `0x004C1350`; it detaches without the flag
      test). The requirement refresh `0x004C1350` also detaches and
      re-equips some body items itself (its rule:
      `client/msg-stats-items.md` §3 rule 3.1). An item placed
      again is attached by the equip rule (rule 2).

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
   have bit `[0x006CE26C]` (= 2, `signed`; open question 5), else unsigned; add the stat to the state's list (rule 2). After the
   stream: `0x004D9E60(unit, state)` (state on hooks, rule 6.2).
2. **State stat** `0x004D9D70(unit, list, state, stat, value, param)`:
   no list given → the unit's list of that state (`0x006256B0`), else a
   new list (owner = the unit's type and GUID, `0x006251F0`), its state
   set (`0x006252D0`) and attached to the unit (`0x00626E10(unit, list,
   1)`). Then the stat is set on it (`0x00627150(list, stat, value,
   param)`, `sim/stat-lists.md` §5). Stat 172 (0xAC) also calls
   `0x00463C00(value)`: ECX = the unit, DL = the old value (the
   existing state list's stat 172 at that param, `0x00625D00`; 4 when
   the list was given or newly made), stack = the new value; compared
   as bytes. A monster (type 1) with old ≠ new and a room
   (`0x00620BB0`): new = 2 → the room's allied count (+0x28) += 1
   (`0x00619EE0`); else old = 2 → allied count −= 1 (`0x00619F20`,
   fatal 0x27C when already < 1); in both cases (and for a change
   between two non-2 values) path reset `0x00649CA0(U)`. No other model
   field (`sim/unit-order.md` §5 r2 for the count; 2026-10-08, read of
   `0x004D9DF9`–`0x004D9E0A` and `0x00463C00`). A stat whose flags (+0x05) have bit
   `[0x006CE26C]` (= 2, `updateanimrate`) also runs `0x00623F50(unit)`.
3. **State on** `0x004D9B20(unit, state)` (also 0xA7): a state with the
   flag `[0x006CE284]` (+0x14 & 0x80 = `notondead`) on a dead unit
   (monster mode 12, player mode 17) only sets the state bit
   (`0x00639DB0`, `sim/stat-lists.md` §9.2). Otherwise, when the bit is
   already set, the existing list of the state is emptied (every base
   stat removed, `0x00627340`; the list stays attached) unless the state
   has the flag `[0x006CE278]` (+0x14 & 0x10 = `noclear`); the bit is
   set; then the effect calls and the dead-unit flags of rule 6.1; last,
   a state with `[0x006CE274]` (+0x10 & 8 = `transform`) runs
   `0x004D9AD0` (rule 6.4).
4. **0xA7 DelayedState** (`0x0045EDE0`) and **0xA9 EndState**
   (`0x0045EF60`): 7 bytes, unit type u8@1, GUID u32@2, state u8@6;
   unit in S → 0xA7: state on (rule 3) then `0x004D9E60`; 0xA9: state
   off `0x004D9F40` then `0x004D9C30` (rule 6.3).
5. Owner in `client/bridge-dispatch.tsv`: `client/msg-units.md` §6
   (2026-10-08: the earlier "TBD" here was stale); this section is their
   stat-list rule.
6. **State on / off in full** (2026-10-08; answers open question 6).
   States record (0x3C bytes, `data/fields.tsv` states rows): overlay1
   +2, overlay2 +4, castoverlay +0xA, removerlay +0xC, flag bits from
   +0x10, setfunc +0x1A, remfunc +0x1C, colorshift +0x21, onsound +0x26,
   offsound +0x28, cltevent +0x30, clteventfunc +0x32. Model writes are
   in bold; every other call is effect state (render, audio, client
   events) carried by one `StateFx` output per call of rules 6.1–6.3
   (`client/bridge.md` §10 table; captured: unit key, state, the phase,
   whether the bit was set before, the unit's dead test); the effect
   layer runs the named 1.14d calls for it. Confirmed 2026-10-08
   (impl-pc1-s5): table bounds setfunc < 31 (`0x004D9ED3`) and remfunc
   < 30 (`0x004D9F79`), setfunc 11 / 12 and remfunc 8 bodies (each tests
   U ≠ none first) as below; nothing here is open, so the `StateFx`
   emission and the bold writes are owed by code.
   1. On (`0x004D9B20`, after rule 3's notondead exit and list empty):
      bit already clear and onsound ≥ 0 → sound (`0x004B9A00`); **bit
      := 1**; colorshift ≠ 0 → `0x004D97F0` (color); `0x004D9920`
      (overlays castoverlay / overlay1 / overlay2, skipped for
      `nooverlays`, +0x14 & 8); the unit dead (`0x00464820`) and
      `0x0063A320(unit)` → **flag-ex +0xC8 |= 0x40000, flags +0xC4 :=
      (flags & ~0x2) | 0x20**; transform → rule 6.4.
   2. Hooks (`0x004D9E60`, after 0xA8's stream and 0xA7's state on):
      nothing for a `notondead` state on a dead unit (rule 3's test);
      else setfunc s (0 ≤ s < 31, table `0x0072A690`, non-null entry)
      is called; entry 3 is `0x004D88A0`, the **skill-level split** of
      `client/msg-skills.md` §2 r7 (model). Table read from the image:
      entries 0 and 20–30 are null; 1–19 are `0x004D86A0`,
      `0x004D87E0`, `0x004D88A0`, `0x004D8960`, `0x004D89B0`,
      `0x004D8B90`, `0x004D9090`, `0x004D8E60`, `0x004D9160`,
      `0x004D9180`, `0x004D9310`, `0x004D9320`, `0x004D9360`
      (`render/unit-composite.md`), `0x004D93B0`, `0x004D9450`,
      `0x004D9480`, `0x004D96B0`, `0x004D9740`, `0x004D97E0`; bodies:
      rule 6.5. Then cltevent ≥ 0 and the state's
      list exists → client event `0x004C7C40(stat 350, stat 351,
      clteventfunc, 1, state)` (effects).
   3. Off (0xA9: `0x004D9F40` then `0x004D9C30`): remfunc r (0 ≤ r <
      30, table `0x0072A710`: entries 1–12 `0x004D8760`, `0x004D89A0`,
      `0x004D8A60`, `0x004D90F0`, `0x004D8D90`, `0x004D9170`,
      `0x004D9260`, `0x004D9340`, `0x004D9380`, `0x004D93A0`,
      `0x004D9630`, `0x004D97E0`, the rest null) is called (bodies:
      rule 6.6);
      cltevent ≥ 0 and the list exists → `0x004DC190(state)` (event
      end). Then, when the bit is set: **the unit's list of that state,
      if its state id matches, is detached and freed** (`0x006277E0`,
      `0x00626CD0`), offsound ≥ 0 → sound; **bit := 0** (always);
      colorshift ≠ 0 → `0x004D97F0`; removerlay ≥ 0 → overlay
      `0x00470390(4, …)` (with `0x00664740` / `0x0045C3E0`); overlay1
      / overlay2 ≥ 0 → `0x004D8660`; transform → rule 6.4; last the
      animation-rate refresh `0x00623F50(unit)`.
   4. Transform `0x004D9AD0(unit)`: `0x0046F000` (gfx); a player in
      mode 0x12 → stop. A player or monster gets **the mode set
      `0x00480E70(unit, 1)`** (`client/model.md` §17 r1.7); then
      `0x00470610(unit, 1)` and `0x00624390(unit)` (animation reset,
      Phase 6), **unit +0x44 := 0**, **current skill := none**
      (`0x00620210(unit, 0)`).
   5. **Setfunc bodies** (2026-10-08, 1.14d asm). All are
      `__fastcall(U, state)`; R = the states record (+0xBC, 0x3C bytes,
      count +0xC4; an index outside → nothing), L = U's list of the
      state (`0x006256B0(U, state)`; none → nothing where L is read);
      `stat(n)` = L's stat n (`0x00625D00(L, n, 0)`); skill rows +0xB98
      (0x23C bytes, count +0xBA0), "k valid" = 0 ≤ k < count.
      Overlay calls (owner of the overlay records: none yet,
      `render/unit-composite.md` §5 r4 and open question 13): add
      `0x00470390(U, id, mode, …)` (stack zeros unless named), remove
      `0x0046F0C0(U, 0, id)`, remove group G(id) = `0x004D8660` (EBX id;
      id = −1 → nothing; else removes id … id + `1ofN` − 1, overlay
      +0x4C via `0x00664740`). ov1–ov4 = R +2/+4/+6/+8, cast = +0xA,
      pgsv = +0xE (negative → −1 unless stated).
      - 1 `0x004D86A0` (pgsv): R `pgsv` (+0x10 & 0x10), 0 < pgsv <
        overlay count (+0xBC0), 0 ≤ R `stat` (+0x18) ≤ itemstatcost
        count (+0xBD4; ≤, original) → remove pgsv, pgsv + 1, pgsv + 2;
        v := `0x00625480(U, stat, 0)`; 1 ≤ v ≤ 3 → add pgsv + v − 1,
        mode 3.
      - 2 `0x004D87E0` (darkness; state 153 `cloak_of_shadows`): U has
        a room (`0x00620BB0`), L, skill k = stat(350) valid → v :=
        `0x00646CA0(U, auralencalc +0x60, k, stat(351))`; darkness
        event `0x0046AE50(in 5, hold trunc(3v / 5), out trunc(2v / 5),
        level id 0x0061A1B0(room), 0)` (`render/lighting.md` §10 r3).
      - 3 `0x004D88A0`: rule 6.2 (model).
      - 4 `0x004D8960`: L; t := stat(353) < 6 (unsigned) → **source
        link `0x00621CC0(U, t, stat(354))`** (`client/msg-units.md`
        §1.2 r4; `skills/bodies.md` §6.20 semantics: +0x94, +0x98,
        state 98 list, +0xC8 |= 0x400).
      - 5 `0x004D89B0`: U a monster (type 1) with L: c := stat(355);
        m := U +0x10 (mode, read first); **`0x004AEDD0(U, c)`** (client
        monster re-init as class c, rule 6.9); **mode set `0x00480E70(U, 8 if m = 8 else 1)`**
        (`client/model.md` §17 r1.7); c = 543 → **facing
        `0x00649EF0(path +0x2C, x, y + 10, 1)`** (x, y = `0x0045ADF0` /
        `0x0045AE20`).
      - 6 `0x004D8B90` and 8 `0x004D8E60` (tiered overlays): L; a :=
        stat(132), b := stat(133), o := stat(355); tier t := 3, 2 when
        a < b / 3, then −1 when a < 2·(b / 3) (C division; so 1–3);
        **stat(355) := t** (`0x00627150(L, 355, t, 0)`).
        Setfunc 6 (cast kept as read): t > o → remfunc 3 body; ov3 ≠ −1,
        t ≥ 3 → add ov3 mode 7; ov4 ≠ −1, t ≥ 2 → add ov4 mode 7; cast,
        ov1, ov2 all ≠ −1 → add cast mode 4 with stack (0, ov1, ov2, 0,
        0). t < o → (o − t) times: G(ov1), G(ov2).
        Setfunc 8 (ov1–ov3): t > o → remfunc 5 body; then for (ov3, 3),
        (ov2, 2), (ov1, 1): ov ≠ −1, t ≥ n → add ov and ov + 1, mode 3.
        t ≤ o → ov3 ≠ 0, t ≤ 2 → G(ov3), G(ov3 + 1); ov2 ≠ 0, t ≤ 1 →
        G(ov2), G(ov2 + 1); ov1 ≠ 0 and **ov1** ≤ 1 → G(ov1), G(ov1 +
        1) (original: tests 0 not −1, so −1 removes overlay 0's group;
        the last test reads ov1, not t; reproduce).
      - 7 `0x004D9090`: L, k = stat(350) valid, `prgsound` (+0x10E) >
        0 → request `0x004B9A00(prgsound, U, 0, 0, 0)`
        (`audio/triggers.md` §8 r2).
      - 9 `0x004D9160` (= remfunc 6 `0x004D9170`): **passive refresh
        `0x00646F20(U)`** (`client/msg-skills.md` §9 r4).
      - 10 `0x004D9180`: R `skill` (+0x38) valid and its `cltmissilea`
        (+0xEA) in [0, missile count +0xB6C) → client missile create
        `0x004CD540` with the zeroed 0x5C record: +4, +8 := U, +0x10 :=
        cltmissilea, +0x2C := R skill, +0x30 := 1 (Phase 6 client
        missiles; `client/msg-units.md` §7 r6); then U ≠ none → **+0xC8
        |= 0x40000, +0xC4 |= 0x20**.
      - 11 `0x004D9310`: **U +0xC8 |= 0x40000** (flag-ex bit 18: not
        drawn, `render/draw-order.md` §5 r1). 12 `0x004D9320`: the same
        and **U +0xC4 |= 0x20** (no shadow, `render/blend-modes.md` §5
        r3).
      - 13 `0x004D9360`: motion record `0x004DA000(U)`, flag 0x10 set
        `0x004DA620(U, 1)` (`render/unit-composite.md` §8, creator
        `0x004D9364`).
      - 14 `0x004D93B0`: record of 7 i32 zeroed; [0], [1] := U's x, y
        (types 2, 4, 5: path +0xC / +0x10; else `0x006488C0` /
        `0x00648900`, 0 with no path); **mode request `0x00480C10(0x19,
        U, record, 0)`** (`client/model.md` §8); **direction
        `0x00648820(path, 0)`** (`sim/pathing.md`).
      - 15 `0x004D9450`: **mode set `0x00480E70(U, 1)`; U +0xC4 |=
        0x80000000** (PROVISIONAL: no client reader, the bit is model
        state only (because the generic flag test `0x00451F30` is called
        with mask 0x80000000 only from server code, `0x005543B0`,
        `0x0057C060`, and no immediate-operand test of that mask on
        +0xC4 lies in client code; register-form tests were not
        scanned); settled by REC-50); **path reset
        `0x00649CA0(U)`**.
      - 16 `0x004D9480` (progressive overlays): L, k = stat(350) valid,
        m := `0x00646CA0(U, calc2 +0x13C, k, stat(351))` > 0; c :=
        stat(169). For each range (ov1 … ov2), then (ov3 … ov4), lower
        ≥ 0 and ≤ upper, n = upper − lower + 1 (the clamps use the first
        range's n1: m := min(m, n1), c := min(c, n1)): j := (c − 1)·n /
        m (C division); for i in 0 … n − 1: i = j → add lower + i, mode
        3, when U lacks it (`0x0046E100(U, id)` = 0); else remove lower
        + i.
      - 17 `0x004D96B0`: R `missile` (+0x3A) in [0, missile count) →
        `0x004CD540` record: +4, +8 := U, +0x10 := missile, +0x2C := R
        skill (negative → 0), +0x30 := 1.
      - 18 `0x004D9740`: `0x004AF890(U, 5, 0)` (blood spray, rule 6.10),
        then setfunc 17's body.
      - 19 `0x004D97E0` (= remfunc 12): **skill-use lock
        `[0x007A04FC]` := 0** (`0x0044CE40(0)`; rule 6.8).
   6. **Remfunc bodies** (same conventions):
      - 1 `0x004D8760`: setfunc 1's guard → remove pgsv, pgsv + 1,
        pgsv + 2.
      - 2 `0x004D89A0`: **unlink `0x00621CE0(U, 0)`** (`skills/bodies.md`
        §6.20, owner none: +0x94, +0x98 := 0, state 98 off with its list
        freed, +0xC8 &= ~0x400).
      - 3 `0x004D8A60`: cast, ov1, ov2: each ≠ −1 → G three times; ov3,
        ov4 ≠ −1 → G once.
      - 4 `0x004D90F0`: L, k = stat(350) valid, `prgsound` > 0, h :=
        U's request of that sound (`0x004CA900(U, prgsound)`) ≠ 0 →
        detach `0x004BA790(h, U, 1)` (`audio/triggers.md` §1 r3).
      - 5 `0x004D8D90`: ov1, ov2, ov3: each ≠ −1 → G(ov), G(ov + 1).
      - 6 `0x004D9170`: setfunc 9.
      - 7 `0x004D9260`: U a monster with a current skill E
        (`0x00620250`) of id 167 (`0x00643CE0`); x, y := E's point
        (`0x006444D0`, `0x00644500`); either ≠ 0 → **E's point := 0, 0
        (`0x006445A0`, `0x006445E0`); room := cell lookup
        `0x00463740(U's room, x, y)`; found → pattern clear
        `0x0064EC10(room, x, y, 1, 0x100)`** (`sim/path-placement.md`).
      - 8 `0x004D9340`: **U +0xC8 &= ~0x40000, +0xC4 &= ~0x20**.
      - 9 `0x004D9380`: motion flag 0x10 clear `0x004DA620(U, 0)`,
        position `0x004DA1D0(U, 0, 0, 0)`.
      - 10 `0x004D93A0`: `0x004ADCE0(U)` (pain worms,
        `monsters/umod-callbacks.md` §28.2 umod 40).
      - 11 `0x004D9630`: remove every id of ov1 … ov2 and ov3 … ov4
        (lower ≥ 0).
      - 12 `0x004D97E0`: setfunc 19.
   7. **Hook values in `StateFx`**: bold writes in rules 6.5–6.6 run in
      the model; the rest is effects. A hooks or off `StateFx` carries
      the hook number (setfunc s / remfunc r; 0 when the body's
      model-side guards fail or it has no effect part) and two i32 the
      model captures at the call: setfunc 1 (v), 2 (v, level id), 6 and
      8 (o, t), 7 and remfunc 4 (k), 16 (m, c after the clamps); 0
      otherwise. Besides these the effect layer reads only static
      records and U's overlays.
   8. **Skill-use check** `0x004D9FC0(U, E)` (`client/msg-stats-items.md`
      restore hands): r := `0x00647960(U, E)`; r ≠ 0 → r. Else U is
      the local player and C (`0x0044DA90`, client update counter
      `[0x007A0498]`) < the lock `[0x007A04FC]` (`0x0044CE50`) → 8;
      else 0. The lock is set to C + min(calc, 12) by the client skill
      function at `0x004C68DA` and cleared by setfunc 19 / remfunc 12.
      `0x00647960`: E's record none or `InGame` clear → 3; **7: E's
      level with bonuses `0x006442A0(U, E, 1)` = 0**; `aura` → 6;
      `passive` → 5; **2: `0x00647640(E, U)` fails (rule 6.11), or the
      weapon-type test `0x00643F80(U)` (`itypea1` +0x18 / `etypea1`
      +0x24 against the items at body locations 4 and 5) fails, or the
      charge test `0x00647840` fails (E from an item, owner GUID +0x34 ≠
      −1: that item's stat 204 with param (base level `0x006442A0(U,
      E, 0)` & 0x3F) + skill·64 has low byte 0, no charges)**, the
      last after the 1 / 4 tests; else 1, 4 or 8 from `0x00647540`, `0x00644060`,
      `0x006440F0`, `0x006478F0` (not traced).
   9. **Client monster re-init** `0x004AEDD0(U, c)` (2026-10-08, one
      read; setfunc 5): c outside the monstats rows, or no monstats2 row
      → nothing. Then in order: graphics freed (`0x0046EC10`),
      `0x00466CB0(U, 0)`, inventory freed (`0x0063AC40(U +0x60)`, +0x60
      := 0), `0x00464930(U)`, `0x00643A00(U, 0)` ≠ 0 → `0x004743D0` on
      it; a monster with monster data whose name (+0x2C) is set: freed,
      := 0. U +0x6C := 0, **U +4 := c**. No path (+0x2C) → stop. Path
      speed `0x00648690(path, Velocity << 8)`; `0x00649FF0(U, 0)`;
      monstats `interact` and no inventory → inventory :=
      `0x0063ABD0(0, U)`. Stats cleared (`0x00626D40(U, 0, 0, 0)`), base
      stats (`0x00627260`): 68, 67, 69 := 100; d := difficulty
      (`0x0044DCD0` & 0xFF): 36, 37, 39, 41, 43, 45 := monstats
      `ResDm`, `ResMa`, `ResFi`, `ResLi`, `ResCo`, `ResPo` column d;
      7, 6 := 0x8000. Mode set `0x00624690(U, 1)`; **U +0x44 := roll on
      U's seed (+0x20) with range U +0x48** (`0x0045C3E0`, one draw).
      Flags from monstats2: +0xC4 bit 2 := `isSel`, +0xC8 &= ~0x40000,
      bit 0x20 := not `shadow`, |= 8, bit 4 := `isAtt`. Then
      `0x004AE0A0(U, 0)`, `0x004AE4F0(U, c)`, `0x004AE210(U)`,
      `0x004AD020(U)` (as at creation, `client/msg-units.md` §1.2 r7),
      U +0xA8 := `0x006438B0(0)`; skills i = 0…7: `Skill`i ≥ 0 → add
      (`0x00647280(U, skill, Sk`i`lvl` (signed byte), 0)`). Monstats
      `npc` clear → **one draw on U's seed, direction
      `0x006488A0(path, low32 & 0xFFFFFF3F)`**. Graphics
      `0x004DC510(U)`. Palette level t := `TransLvl` + 2 when
      `TransLvl` < 8, else 2; class 156, 211, 242, 243 → by difficulty
      0 / 1 / 2: t := 2 / 3 / 4; class 333 → t := 5; class 527 →
      overlay `0x00470390(U, 0xE3, 5, 0, 0, 0, 0, 0)`; then t ≥ 8 → 2
      (not for 333 and the difficulty cases, which skip the test);
      `0x00463E20(U, t)`, `0x00463E80(U, t)`.
   10. **Blood spray** `0x004AF890(U, n, roll)` (setfunc 18 passes (5,
      0)): roll ≠ 0 → one draw on U's seed, low32 % 25 ≠ 0 → stop. U a
      monster in mode 3 → one draw, low32 % 3 ≠ 0 → stop. n := U's
      stat 140 (`item_extrablood`) when ≠ 0. U's monstats2 `bleed`
      (+0x11D) = b; b = 0 or n ≤ 0 → nothing. Else n times: U has a
      path with `0x006486C0` ≠ 0 → one draw r, d := ((U direction
      `0x00620100` & 0xFF) >> 3) + r % 7 − 3; else one draw, d := r;
      d &= 7. From d, try up to 7 directions d, d + 1, … (mod 8): the
      cell (x + dx[d], y + dy[d]) (tables `0x006DA610` / `0x006DA5F0`;
      x, y as §6 conventions) with `0x0064CB30(room, x', y', 1)` = 0 is
      taken; all 7 blocked → the whole call stops. Then one draw r2,
      missile m := 18 + r2 % (2b) (a power of two → `& (2b − 1)`, same
      value) created at the cell by `0x004CDBA0(U, m, x', y', 0, 1)`
      (`monsters/umod-callbacks.md` §28 r4 "missile").
   11. **Skill item test** `0x00647640(E, U)`: U's inventory (+0x60)
      none → 0 (also for the cases below). E's record `scroll` flag
      (bit 36) → E +0x30 > 0. Else by the record's skill id (+0x00):
      the weapon pick `0x0063C9B0(inventory, &I, &loc, &inuse)`
      (`skills/bodies-3.md` §3.3 step 2), J := the item at the other
      location (`0x00643D60(loc)`), "throw-ok(X)" := X throwable type
      (`0x0062BA80`) or X's stat 125 (`0x00625500`). Skill 2 (Throw):
      U not dual-wielding (`0x006235A0` = 0) → pick ok, inuse ≠ 0 and
      throw-ok(I); dual → pick ok, throw-ok(inuse = 0 ? J : I). Skill 4
      (Left Hand Throw): pick ok, throw-ok(inuse = 0 ? I : J). Skill 5
      (Left Hand Swing): needs dual-wield; pick ok and (inuse = 0 ? I :
      J) is of type 45 `weap` (`0x00629BB0`). Other skills → 1.

### 4. Skills

1. Passive skills attach a state list per passive state with the
   `passivestat` values (`client/msg-skills.md` §2 rule 4), run by every
   assign (0x94, 0x21) and add.
2. Item-granted skill levels (stats 97, 107, 126, 127, 188 …) reach the
   skill list through the client callback (rule 1.1 and
   `client/msg-skills.md` open question 2), not through this spec.
3. **Passive state list** `0x00643620(L)` (2026-10-08; EDI unit U, EBX
   state p, stack L; `ret 4`; one caller, the refresh `0x00646D60` at
   `0x00646E30` with L = the skill level): list := U's child list of
   state p (`0x006256B0(U, p)`; none when U has no extended list).
   - L ≠ 0, list found → return it unchanged.
   - L ≠ 0, none → allocate `0x006251F0(pool = U +0x08, flags 0,
     expire 0, owner type = U's type, owner GUID = U +0x0C)` (U none:
     pool 0, type 6, GUID −1), set its state to p (`0x006252D0`),
     attach `0x00626E10(U, list, reset 1)` (`sim/stat-lists.md` §8.1:
     DYNAMIC cleared, every entry propagates, damage-related included;
     a U without an extended list attaches nothing), return it. The
     list is then filled by the refresh (`client/msg-skills.md` §2 r4).
   - L = 0: a found list is detached (`0x006277E0(U, list)`) and freed
     (`0x00626CD0`); return 0.
4. **State bits without a state list.** A state's on / off bit is not
   in the state's list: it is bit s of the state bit array of U's own
   extended list (unit +0x5C, list +0x58, `sim/stat-lists.md` §9.1).
   State on `0x00639DB0` (§3 r3, `client/msg-skills.md` §2 r1) sets it
   whether or not a list of that state exists; the list (rule 3, §3 r2)
   only carries the state's stats. A unit without an extended list has
   no state bits: the toggle `0x00625A70` does nothing (the update-queue
   insert still runs, `sim/stat-lists.md` §9.2). d2rs: keep the bits on
   the unit's own list object (§1 r2), independent of child lists.

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

2026-10-08: `all.asm` of `0x004609F0` (jump tables decoded from the
code), `0x00460930`, `0x004609A0`, `0x00643620` (caller `0x00646E30`),
`0x006256B0`, `0x00625A70`; mask table `0x006CE268` read with `pefile`.

## Open questions

1. Answered (2026-10-08, `docs/handoff/impl-client-msgs-3.md` Q10):
   the callback's effects are §1 r4. So build the client list now with
   the client callback (`ValueCallback::Client` implementing §1 r4);
   no waiting is needed. Its light effects go to the render light of
   the unit; its skill effects to the client skill list.
2. The item stream's stat section (which lists an item gets: base,
   magic, set, runeword) — `items/inventory.md` open question 1 and
   `client/msg-stats-items.md` open question 3.
3. *Answered (2026-10-08)*: §2 rule 3.1. Original question:
   `0x00625820` (which items are re-equipped on a level change) and the
   left / right skill restore of `0x0045D3E0`.
4. *Answered (2026-10-08)*: §2 rule 4.1. Original question: detach per
   item action (unequip, swap, move to the grid): which handlers detach
   the list and when.
5. The flag masks `[0x006CE26C]`, `[0x006CE274]`, `[0x006CE278]`,
   `[0x006CE284]` (itemstatcost / states flag bits read through globals):
   dump their values. *Partly answered* (2026-10-08): the table
   `0x006CE268` holds 1 << i (dumped: 1, 2, 4, …), so they are 2, 8,
   0x10, 0x80; itemstatcost +0x04 & 2 = `signed`, +0x05 & 2 =
   `updateanimrate` (`data/fields.tsv`); the states +0x14 bit names
   remain. *Answered (2026-10-08)*: §3 rules 1–3. The states flag bits
   start at record +0x10 (`data/fields.tsv` states rows: bit n at byte
   0x10 + n / 8, mask 1 << (n % 8)); `0x004D9B20` reads `[0x006CE284]`
   = 0x80 and `[0x006CE278]` = 0x10 at +0x14 (bits 39 `notondead`, 36
   `noclear`) and `[0x006CE274]` = 8 at +0x10 (bit 3 `transform`);
   itemstatcost +0x04 & `[0x006CE26C]` = `signed`, +0x05 & 2 =
   `updateanimrate`.
6. *Answered (2026-10-08)*: §3 rule 6 (model parts; effect calls as
   `StateFx`; setfunc / remfunc bodies: rules 6.5–6.7).
   Original question: the rest of the state on / off paths (`0x004D97F0`, `0x004D9920`,
   `0x004D9AD0`, `0x004D9F40`, `0x004D9C30`) and their owner (a client
   states spec taking 0xA7–0xA9). A recording with a buff (e.g. a
   shrine) gives 0xA8 bytes to check the stream rule.
7. Answered (2026-10-08, `docs/handoff/impl-client-msgs-3.md` Q5): the
   `0x00643620` arguments (pool, flags 0, expire 0, U's type and GUID),
   the attach with reset 1, and where the state bits live before a
   state list exists are §4 rules 3–4.
