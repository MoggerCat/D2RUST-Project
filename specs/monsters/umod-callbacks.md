# Spec: Monsters — Umod callbacks (boss-modifier hooks)

- **Status:** draft: every callback body of the umod callback table
  (`monsters/init.md` §22, table `0x0073C0B8`) read from the 1.14d
  `Game.exe` disassembly (addresses per section; register arguments
  checked in `re/exports/all.asm`), with the helpers they need that no
  other spec owns (unit find, area damage, elemental fill, think
  restart). No recording covers a callback yet (Open question 1).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::monsters::init` (dispatcher and callbacks,
  beside `umods.rs`)
- **Related specs:** `monsters/init.md` (umod catalogue §19, dispatcher
  §22, `umods.tsv`); `sim/units.md` (§4.6 monster mode set, §6.2 monster
  events); `sim/tick.md` (§5.2 scheduling, §5.6 dispatch);
  `sim/unit-events.tsv` (scheduler sites); `sim/rng.md` (`roll`, unit
  seed); `combat/damage.md` (damage record §1, apply §5.2, reaction
  §7.1); `combat/hit.md` (hostility `0x00554200`); `missiles/missiles.md`
  (parameter record §R2.1, creation §R2.3, hit flags `0x005AD730` §R6);
  `skills/bodies.md` (conventions §1, skill missile §2.4, `apply_state`
  §2.7, ring §6.7); `skills/bodies-2.md` (jitter callback §2.3, mode
  damage §2.1); `skills/levels.md` (`eval`, skill entries and levels);
  `sim/pets.md` §6 (pet remove); `monsters/ai.md` (AI params, owner
  data, minion lists); `world/quests*.md` (quest death effects).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 69–79 |
| Inputs | 80–89 |
| Outputs / state changes | 90–96 |
| Rules | 97–98 |
|   1. Scope, order and conventions | 99–131 |
|   2. Where the dispatcher runs | 132–169 |
|   3. Shared helpers | 170–322 |
|   4. Death event `0x005A3800` (mode 1; umods 10, 18, 31, 32, 42) | 323–332 |
|   5. Umod 7 curse: mode 3 `0x005A2530` | 333–362 |
|   6. Umod 9 fire | 363–388 |
|   7. Umod 10 poisondead: mode 2 `0x005A2C20` | 389–394 |
|   8. Umod 14 spcdamage: mode 0 `0x005A3B50` (traps) | 395–413 |
|   9. Umod 15 partydead: mode 1 `0x005A2D10` | 414–423 |
|   10. Umod 17 lightning | 424–444 |
|   11. Umod 18 cold: mode 2 `0x005A2BD0` | 445–451 |
|   12. Umod 19 hireable | 452–467 |
|   13. Umod 20 scarab | 468–478 |
|   14. Umod 21 killself: mode 2 `0x005A3AA0` | 479–489 |
|   15. Umod 22 questcomplete: mode 1 `0x005A3250` | 490–509 |
|   16. Umod 23 poisonhit: mode 0 `0x005A3490` | 510–514 |
|   17. Umod 24 thief: mode 3 `0x005A30E0` | 515–531 |
|   18. Umod 27 spectralhit | 532–552 |
|   19. Umod 29 multishot: mode 5 `0x005A3610` | 553–574 |
|   20. Umod 31 goboom: mode 2 `0x005A2840` | 575–581 |
|   21. Umod 32 firespike_explode: mode 2 `0x005A3D20` | 582–592 |
|   22. Umod 33 suicideminion_explode | 593–617 |
|   23. Umod 34 ai_after_death | 618–643 |
|   24. Umod 35 shatter_on_death: mode 1 `0x005A3A80` | 644–647 |
|   25. Umod 40 worms_on_death: mode 1 `0x005A4200` | 648–654 |
|   26. Umod 41 always_run_ai: mode 2 `0x005A4230` | 655–660 |
|   27. Umod 42 lightningdeath: mode 2 `0x005A2910` | 661–674 |
| Constants & data dependencies | 675–690 |
| Randomness | 691–709 |
| Edge cases & original bugs | 710–739 |
| Test vectors | 740–741 |
|   Synthetic (CI-safe) | 742–766 |
|   Real 1.14d values (live tables; `#[ignore]`, `D2_GAME_DIR`) | 767–774 |
| Provenance | 775–814 |
| Open questions | 815–850 |
<!-- /index -->

## Summary

A monster's umod list (`init.md` §19) can attach callbacks to five
moments: mode 0 before a mode start, mode 1 after it, mode 2 on a type-7
event (MONUMOD), mode 3 after the monster lands a hit, mode 4 after it
takes a non-stunning hit, and mode 5 when it creates a missile. This
spec gives each callback body: death explosions (fire, poison, corpse,
suicide), death novas, the lightning burst, curses on hit, revival,
party kills, trap damage, hireling secondary damage, spectral hit and
multishot copies, plus where in the engine the dispatcher runs.

## Inputs

| Name | Type | Source |
|---|---|---|
| game | frame (+0xA8), difficulty d (+0x6D, u8), L-flag (+0x6A ≠ 0 or +0x74 ≠ 0), expansion (+0x70) | `tick.md`, `init.md` §8.1 |
| unit | monster with monster data (umod list +0x1C, type flags +0x16) | `init.md` Outputs |
| umod, unique | callback stack arguments | `init.md` §22 |
| arg | the missile (mode 5 only) | `missiles.md` §R2.3 step 28 |
| tables | monstats, monlvl, monumod `constants`, difficultylevels, skills (row 66), missiles | `data/fields.tsv` |

## Outputs / state changes

Timer events (types 2 and 7), missiles, area damage on units, stat
values on a monster's base list (flag 1) or on a missile, states 104 and
107, monster mode changes (death), pet removal, owner data, monster
data +0x16 bits 0x80 and 0x100 and +0x18, and unit-seed draws.

## Rules

### 1. Scope, order and conventions

1. Calling convention of every callback: ECX game, EDX unit (mode 5:
   the missile), stack (umod, unique); `ret 8`. unique = type flag 8 of
   the **monster whose list is walked** (in mode 5 the missile's owner).
   No callback returns a value.
2. Sections are ordered by how many umods share the callback function:
   `0x005A3800` (5 umods) first (§4), then the single-umod callbacks in
   umod id order (§5–§27). Every callback address of `umods.tsv` has a
   body here.
3. Conventions of `skills/bodies.md` §1 apply: position of a unit
   (static path +0x0C / +0x10 for types 2, 4, 5; else the dynamic path,
   0 without a path), F = game frame, "state on" `0x00639DB0`, "has s"
   `0x00639DF0`, list set `0x00627150`, unit set `0x00627260`
   (`stat-lists.md` §5 rule 2). "Level" = the unit's `level(12)` total
   (`0x00625480(unit, 12, 0)`). "Base list" = the unit's stat list with
   flag 1 (`0x00625760(unit, 1)`). "Monstats row" M = class (unit +4)
   × 0x1A8 + table base, none when the class is outside 0 … count − 1
   (count at data tables +0xA80); per-difficulty columns are read at
   column + 2d (signed 16-bit).
4. "Event 7 at F + n" = `0x005417D0(game, unit, 7, F + n, 0, 0)`
   (`tick.md` §5.2); "mode set m" = build the mode-change record
   (`0x005A7E60(unit, m, &rec)`) and run `0x005A7C20(game, &rec, 1)`
   (`units.md` §4.6); mode 0 = DT (death), 3 = GH, 12 = DD (dead).
5. "Dead" = `0x005541B0(unit)` ≠ 0. "Owner" O(u) = `0x00552FD0(game,
   u)`: when unit flags +0xC8 bit 10 is set, the unit stored as its
   owner (+0x98 type / GUID, looked up by `0x00552F60`), else none.
   "Minion owner" = `0x0058F0D0` (`ai.md` §3.1). Alignment =
   `0x006259B0`. "Hostile" = `0x00554200(game, a, b)` (`combat/hit.md`).
6. U = the unit's own seed (unit +0x20, `rng.md` §5.3). "One U step" =
   one generator step; lo' = its new low word. `roll(n)` = `rng.md`
   §3 (no step when n < 1).

### 2. Where the dispatcher runs

The dispatcher `0x005A4270` and its table are `init.md` §22. Its call
sites, in engine order:

1. **Mode 0** (`0x005A4350`) inside the monster mode set `0x005A7C20`
   (`units.md` §4.6), only when the requested mode is **not 3** (GH):
   after the path / AI bookkeeping and the mode damage rewrite
   `0x005A4F50` (`skills/bodies-2.md` §2.1), **before** the new mode's
   start function. The unit's mode field still holds the old mode.
2. **Mode 1** (`0x005A4360`) inside `0x005A7C20` for every requested
   mode: after the start function (or the neutral start that replaces
   it), the flag 0x80000, the animation prepare `0x005533D0` and the AI
   hook `0x005A6380`; **before** the cancel of events 0 / 1 and the
   animation schedule. The mode field holds the new mode. A type-7
   event a mode-1 callback schedules is therefore inserted before the
   mode's own animation events (`tick.md` §5.2 order).
3. **Mode 2** (`0x005A4370`) is the monster handler of event type 7
   (`units.md` §6.2). **Every** type-7 event of the monster runs every
   mode-2 callback of its list, whoever scheduled it (umods, quests,
   skills, population `0x0054E600`). A frozen live monster drops
   type 7 (`tick.md` §5.6).
4. **Mode 3** (`0x005A4390`) in damage apply `0x0057C6C0`
   (`combat/damage.md` §5.2 step 9) on the **attacker**, when it is a
   monster and the result has "hit".
5. **Mode 4** (`0x005A43A0`) in the reaction `0x0057CEE0`
   (`combat/damage.md` §7.1), monster-defender branch, on the
   **defender**, at two sites: (a) result has get-hit (4) and the
   defender is stunned (state 21) or the get-hit test `0x0057CB00`
   (`damage.md` §6.2) says it enters get-hit: mode set 3 (GH), then
   mode 4; (b) otherwise (get-hit refused by `0x0057CB00`), or result
   has soft hit (0x4000) without get-hit: queue for update, unit flags
   |= 0x8000, `0x005734C0(defender, 19)`, then mode 4. Knockback,
   block and death branches do not reach it.
6. **Mode 5** (`0x005A43B0(game, owner, missile)`) in missile creation
   `0x0059FA30` (`missiles.md` §R2.3 step 28): owner a monster, missile
   non-null.

### 3. Shared helpers

#### 3.1 Unit find `0x0065A950` / `0x0065AC70` / `0x0065AA40`

This spec owns the unit find for every caller (area damage §3.2,
`missiles/bodies-2.md` §44 corpse effect, `monsters/ai-bodies-3.md`
wisps, and the other callers listed below).

**Finder record** (on the caller's stack). Init `0x0065A950(pool,
finder, room R, x, y, r, cb, A)` (`ret 0x20`; finder null → fatal):
+0x00 pool, +0x04 result array (0x3C bytes = 15 unit pointers,
allocated from the pool; failure → fatal), +0x08 R, +0x0C cb, +0x10 A,
+0x14 room flags G := 0, +0x18 found count := 0, +0x1C capacity := 15,
+0x20 x, +0x24 y, +0x28 r. The caller may then write G (+0x14); collect
`0x0065AC70(finder)` (`ret 4`); free `0x0065AA00`.

**Filter record A** (built by the caller, 0x38 bytes, zeroed): +0x00
flags F, +0x08 excluded unit E, +0x0C x, +0x10 y, +0x14 radius r,
+0x18 limit, +0x1C accepted count, +0x20 line iterator (F & 0x200),
+0x24 extra test (F & 0x800).

Collect `0x0065AC70`:

1. R none → found count := 0, return 0.
2. Rooms: if the square x ± r, y ± r lies strictly inside R
   (`0x0065A6B0`: R's subtile box (x0, y0, w, h) from `0x00619730`,
   active room +0x4C; x0 < x − r, y0 < y − r, x + r < x0 + w, y + r <
   y0 + h) → R only; else R's adjacency array (`0x00619790`, active room
   +0x00, count +0x24, `drlg/rooms.md` §1 / §6, R included), in array
   order.
3. Per room: G & 0x2000 and the room is in a town (`0x0061AB00` →
   `0x006426A0`: the room's level id is 1, 40, 75, 103 or 109; byte
   table `0x006426C8`) → skip. Then the overlap test `0x0065A710`, which
   rejects only when x + r < x0 and x − r > x0 + w, or y + r < y0 and
   y − r > y0 + h: for r ≥ 0 and a box of non-negative size it never
   rejects (original bug; distance is left to the filter).
4. Per unit U of the room's list (head +0x74, next +0xE8, the next link
   read before the test), in list order: cb set → it must be a valid
   code pointer (`IsBadCodePtr`, else fatal) and cb(ECX U, EDX A) ≠ 0
   accepts; cb none → the filter `0x0065AA40(U, A)` ≠ 0 accepts.
   Accepted units are appended; when the count reaches the capacity the
   array grows by 15 (`0x0065A900`), without limit.
5. Found count (+0x18) := the number appended; return it. **Found
   order** = room order, then each room's list order.

G (+0x14) is set only by the callers `0x0056DCC0` (G := the request's
flags, so corpse effects with 0x2000 skip town rooms), and the callers
at `0x004675C2` and `0x004F1524`; `0x0057E090` (§3.2), `0x0056DC61`,
`0x0056E418`, `0x00582E6D`, `0x005D302D`, `0x005DFCCB`, `0x005F3F14`
leave G = 0 (no town-room skip; the filter's own 0x100 test still
applies). `0x0056C282` is a wrapper that passes the caller's finder.

Filter `0x0065AA40(U, A)` (`ret 8`):

1. A none → reject. F & 0x40 and accepted count (+0x1C) ≥ limit
   (+0x18) → reject.
2. Position of U (static path for types 2, 4, 5; else the dynamic
   path); d² = (uy − y)² + (ux − x)² (32-bit) > r² → reject (distance
   ≤ r passes).
3. By U's type: player (0) needs F & 1; F & 0x1000 → mode 17 (dead)
   only, else mode 0 or 17 rejects; U = E rejects. Monster (1) needs
   F & 2; F & 0x1000 → mode 12 only, else mode 0 or 12 rejects; F & 4 →
   must be undead (`0x0063E990`). Object (2) needs F & 0x10. Missile
   (3) needs F & 8 and a missiles row (`0x0046ACE0`) **without**
   `Explosion` (+0x04 bit 1). Item (4) needs F & 0x20. Tile (5) and
   other types reject.
4. F & 0x80 → unit flag 0x4 (+0xC4, `0x00451F30(U, 4)`) required;
   F & 0x400 → unit flag 0x8 required; F & 0x100 → U's room in a town
   (as collect step 3) rejects; F & 0x200 and U has a room → step the
   line iterator at A +0x20 (`0x0066A5D0`); any point whose collision in
   U's room (`0x0064CB30(room, px, py, mask 4)`) equals 4 rejects;
   F & 0x800 → the extra test A +0x24 (ECX U, EDX A) non-zero rejects.
5. Accept: accepted count += 1, return 1.

Flags used in this spec: 3 (players, monsters), 0x581 (players only,
live, unit flags 4 and 8, not in town), 0x583 (the same plus monsters).
Corpse effect (`missiles/bodies-2.md` §44) passes 0x3002: monsters in
mode 12 only, outside towns, within r.

#### 3.2 Area damage `0x0057E090`

`area_damage(game, src, x, y, r, rec, hit_owner, hit_owner2, cb, F)`
(ECX game, EDX src; `ret 0x20`); returns 1 when it hit something:

1. R = the room containing (x, y), searched from src's room
   (`0x00463740`). F = 0 → 0x583. Find (§3.1) with {F, E none, x, y,
   r} from R.
2. O1 = owner(src); none → free, return 0. O2 = owner(O1).
3. For each found unit V in found order: skip V = O2 unless
   `hit_owner2`; skip V = O1 unless `hit_owner`; hostile(game, O1, V)
   required; line clear: `0x00645950(x, y, V, 0x805)` (the line test
   `0x0064E260` from (x, y) to V's position in V's room, mask 0x805,
   returns 0). Copy `rec` (0x70 bytes); cb ≠ null → cb(ECX game, EDX
   src, V) must be non-zero (an invalid cb pointer is a fatal
   assertion); then `0x005AD730(game, src, V, copy)` (`missiles.md`
   §R6: hit flags, block, damage apply and reaction); result 1.

`0x005AD730` does nothing when src is not a missile (type 3) or has no
owner, so only a missile source deals damage. Callers: §6.2, §20, §21,
§22.2 and a monster mode function (call at `0x005A729E`).

#### 3.3 Cross burst

Used by §10.2 (missile 195, `lightunique`), §13.2 (225, `buglightning`)
and §16 (321, `queenpoisoncloud`). With level L: a 0x5C parameter
record (zeroed; `missiles.md` §R2.1): flags 0x21, owner = the unit,
class m, (x, y) = the unit's position, level L, skill 0, init callback
`0x005C9290` (`skills/bodies-2.md` §2.3). For i = 0 … 3 with (X, Y) =
(0, −1), (1, 0), (0, 1), (−1, 0) (tables `0x006E2188` / `0x006E2178`,
§13.2 and §16 use copies at `0x006E21A8` / `0x006E2198` and on the
stack), for j = 0, 1: target = (x + X_i, y + Y_i), init argument j;
create (`0x0059FA30`, result ignored). 8 missiles, i outer, j inner.

#### 3.4 Elemental damage fill `0x005A21D0`

`elem_fill(unit, s_min, s_max, s_len)` (EAX game, stack unit and the
three stat ids):

1. unit is a missile: no missiles row, or the row has `NoUniqueMod`
   (flags +0x04 bit 13, mask table `0x006CE27C`) → nothing.
2. d' = min(d, 2); o = L-flag. lv = level; row = min(max(lv, 1), monlvl
   rows − 1); rows − 1 < 0 → nothing.
3. v = monlvl `DM` (o = 0) or `L-DM` (o = 1) for d' at that row.
4. a = K[28] when monumod has ≥ 29 rows, else 0; b = K[31] when ≥ 32
   rows, else 0 (K = `constants`; live 66, 100). min = a × v / 100,
   max = b × v / 100 (signed, truncating).
5. Monster: base list set s_min := min, s_max := max. Other units: unit
   set (`0x00627260`).
6. s_len ≠ −1: t = s_len total + 40; set s_len := t the same way.

#### 3.5 Think restart `0x00573780`

ECX game, EDX unit (11 call sites; §26 is one). No draws.

1. Cancel the unit's type-2 events (`0x00540E60(game, unit, 2, 0)`).
2. Mode 1 (NU): cancel type 2 again, event 2 at F + 2; go to 3. Mode 0
   or 12: return. Other modes: go to 3.
3. Base class (`0x00463860`: monstats `BaseId`, or the class itself
   when the row is missing; −1 for a class outside the table) ∈ {110
   vulture1, 118 willowisp1, 136 batdemon1, 247 frogdemon1} (jump table
   `0x005737FD`): cancel type 2, event 2 at F + 2.

So a moving or attacking monster of another base class loses its
pending think until something reschedules it.

#### 3.6 Monster data fields used here

| Field | Use |
|---|---|
| +0x16 bit 0x80 | multishot copy in progress (§19) |
| +0x16 bit 0x100 | set when the lightning burst fires, cleared when it is refused (§10.2); nothing here reads it |
| +0x18 (i32) | frame of the last lightning burst (§10.2) |

### 4. Death event `0x005A3800` (mode 1; umods 10, 18, 31, 32, 42)

1. unique = 0 and umod = 18 → nothing.
2. Unit mode ≠ 0 → nothing.
3. Event 7 at F + 4.

So cold (18) schedules only for a unique; poisondead (10), goboom (31),
firespike_explode (32) and lightningdeath (42) for any holder. Each
mode-2 callback then runs at F + 4 (§7, §11, §20, §21, §27).

### 5. Umod 7 curse: mode 3 `0x005A2530`

1. unique = 0 → nothing.
2. One U step; lo' & 3 = 0 → nothing (cast on 3 of 4 hits).
3. L = level / 5 + 1 (truncating); L < 1 → 1.
4. Skill 66 (`amplifydamage`) must exist (skills count > 66). r =
   `eval(unit, aurarangecalc, 66, L)` (`skills/levels.md`), clamped to
   1 … 40.
5. `0x0056DBC0(game, F 3, unit, L, r, cb 0x005A23D0)`: center = the
   unit's target position (`0x0056D2C0`, `skills/bodies.md` §2.4);
   fails → nothing. R = the room containing it, searched from the
   unit's room; none → nothing. Find (§3.1) with {3, E = unit, center,
   r}; for each found V in order: cb(game, unit, V, L). A null cb is a
   fatal assertion.

Per target `0x005A23D0(game, unit, V, L)`:

1. hostile(game, unit, V) fails → nothing.
2. Skill 66 record; `aurastat1` < −1 or ≥ itemstatcost count, or
   `auratargetstate` < 0 or ≥ states count → nothing.
3. `apply_state` (`skills/bodies.md` §2.7) with source unit, target V,
   skill 66, level L, duration `eval(unit, auralencalc, 66, L)`, stat
   `aurastat1`, value `eval(unit, aurastatcalc1, 66, L)`, state
   `auratargetstate`, remove callback 0 (default). None → nothing.
4. For i = 2 … 6: `aurastat_i` in 0 … count − 1 and v =
   `eval(unit, aurastatcalc_i, 66, L)` ≠ 0 → list set `aurastat_i` :=
   v.

No resistance scaling (`skills/bodies.md` §2.10) and no curse AI.

### 6. Umod 9 fire

#### 6.1 Mode 1 `0x005A25F0`

unique ≠ 0 and unit mode 0 → event 7 at F + 4.

#### 6.2 Mode 2 `0x005A2620` (death explosion)

No unique or mode test (any type-7 event of a holder runs it).

1. (x, y) = the unit's position. m = `skill_missile(game, 117
   monstercorpseexplode, unit, skill 0, L 1, 0, 0, x, y, quant 1)`
   (`skills/bodies.md` §2.4). None → nothing.
2. H = maxHP output of stats by level (`init.md` §8.1, `0x006538A0(class,
   L-flag, d, level, flags 1, out)`, out zeroed 0x38 bytes).
3. Difficultylevels record of d (`0x00611D30`); none → nothing.
4. a = pct(H, `MonsterCEDamagePercent`, 100) (`init.md` §8.2). Then by
   d: 0 → a −= a / 4; 1 → a −= a / 3; 2 → a := a / 8 (all signed,
   truncating toward zero); other d → unchanged.
5. b = pct(a, 60, 100) (inlined; same branches as `init.md` §8.2).
6. dmg = b + `roll(a − b)` on U.
7. Damage record (zeroed 0x70): physical (+0x08) = fire (+0x10) =
   dmg << 6.
8. `area_damage(game, m, x, y, r = d + 4, rec, 0, 0, null, 0x581)`
   (§3.2): players only.

### 7. Umod 10 poisondead: mode 2 `0x005A2C20`

L = level; L < 2 → 1. `skill_missile(game, 155 corpsepoisoncloud, unit,
skill 0, L, 0, 0, x, y, quant 1)` at the unit's position. No other
test.

### 8. Umod 14 spcdamage: mode 0 `0x005A3B50` (traps)

1. n = 2. The unit's room has a level id (`0x0061A1B0`) ≠ 0 → n =
   area level `0x0061DCA0(level id, d, expansion)` (`init.md` §7 rule
   2); n < 2 → 1.
2. Unit set level(12) := n; `tohit(19)` := min(n + 50, 90).
3. Not a monster → fatal assertion. Base list B; h = n >> 1 (arithmetic):

| Class | B sets |
|---|---|
| 326 trap-firebolt | mindamage(21) 0, maxdamage(22) 0, firemindam(48) h, firemaxdam(49) 3n >> 1 |
| 327 trap-horzmissile, 328 trap-vertmissile | 21 := min(h + 1, n), 22 := n |
| 329 trap-poisoncloud | 21 0, 22 0, poisonmindam(57) n, poisonmaxdam(58) 2n, poisonlength(59) 2n |
| 330 trap-lightning, 369 trap-nova | 21 0, 22 0, lightmindam(50) h, lightmaxdam(51) 3n >> 1 |
| other (e.g. 354 trap-melee) | nothing more |

Mode 0 runs after the mode damage rewrite (§2 r1), so these values
replace that rewrite's damage on every mode set except GH.

### 9. Umod 15 partydead: mode 1 `0x005A2D10`

1. Unit mode ≠ 0 → nothing. O = minion owner; none → nothing.
2. For each minion of the unit (`0x0058F380(unit, game, 0, cb
   0x005A2CC0)`, `ai.md`): free its own minion list (`0x0058F160`);
   owner data := (−1, 1, 0, 0) (`0x0058F030(game, minion, −1, 1, 0,
   0)`); its mode ≠ 0 → mode set 0.
3. Owner data of O := (−1, 1, 0, 0).
4. O ≠ unit and O a monster → mode set 0 on O.

### 10. Umod 17 lightning

#### 10.1 Mode 1 `0x005A37D0`

unique ≠ 0 and unit mode 3 (GH) → event 7 at F + 2.

#### 10.2 Mode 2 `0x005A29A0` (burst)

1. unique = 0 → nothing.
2. t = data +0x18 (0 without monster data). |F − t| < 10 → clear
   +0x16 bit 0x100; nothing more.
3. +0x18 := F; +0x16 |= 0x100.
4. L = level / 2 (truncating); L < 1 → 1. Cross burst (§3.3) of
   missile 195 `lightunique` with L.

#### 10.3 Mode 4 `0x005A2BA0`

unique ≠ 0 and unit mode ≠ 3 → §10.2 with the same arguments. So a
hit that puts the unique into GH bursts through §10.1 two frames later;
a soft or refused hit bursts at once (§2 r5).

### 11. Umod 18 cold: mode 2 `0x005A2BD0`

unique = 0 → nothing. L = level / 2; L ≤ 1 → 1. `ring(game, unit,
unit, 194 coldunique, skill 0, L, v)` (`skills/bodies.md` §6.7, 64
missiles) with v = velocity of missile 119 `frostnova` at level 0
(`0x00663270(119, 0)`).

### 12. Umod 19 hireable

#### 12.1 Mode 0 `0x005A2D80`

a = mindamage(21), b = maxdamage(22) totals. b ≤ 0 or class 271
(roguehire) → nothing. a' = a − 1 + `secondary_mindamage(23)`, b' = b −
1 + `secondary_maxdamage(24)`, each clamped ≥ 0; base list set 21 :=
a', 22 := b'. No unique test.

#### 12.2 Mode 5 `0x005A2E00`

Unit = the missile (type 3, else nothing). a, b = its 21, 22 totals;
b ≤ 0 → nothing. O = owner(missile); none or class ≠ 271 → nothing.
a' = a − 256 + 256 × O's 23, b' = b − 256 + 256 × O's 24, each clamped
≥ 0; unit set on the missile.

### 13. Umod 20 scarab

#### 13.1 Mode 1 `0x005A2EB0`

Unit mode 3 or 0 → event 7 at F + 2. No unique test.

#### 13.2 Mode 2 `0x005A2EE0`

Cross burst (§3.3) of missile 225 `buglightning` with L = level (no
clamp). No unique or mode test.

### 14. Umod 21 killself: mode 2 `0x005A3AA0`

1. Dead → nothing.
2. Has state 54 (`uninterruptable`) → event 7 at F + 3; nothing more.
3. O = minion owner. O is a player (type 0) → `0x005750E0(game, O,
   unit GUID (+0x0C), kill 1)` (`sim/pets.md` §6).
4. Otherwise → mode set 0 on the unit.

Umod 21 is given with a type-7 timer by temporary summons
(`skills/bodies.md`, `skills/bodies-2.md`).

### 15. Umod 22 questcomplete: mode 1 `0x005A3250`

By class (unit +4), jump tables `0x005A3414` / `0x005A3428` (classes
156…256) and `0x005A331C` / `0x005A332C` (479…709); class 267 is
tested directly. When the unit's mode is 0, call (ECX game, EDX unit):

| Class | Call |
|---|---|
| 156 andariel | `0x005DFE00` |
| 229 radament | `0x005DFE20` |
| 242 mephisto | `0x005DFDB0` |
| 256 izual | `0x005E0020` |
| 267 bloodraven | `0x005DFD90` |
| 479 overseer1 (Shenk) | `0x005E0040` |
| 540–542 ancientbarb1–3 | `0x005E0060` |
| 704, 705, 709 ubermephisto, uberdiablo, uberbaal | `0x005E0070` |

Other classes and other modes: nothing. The callee bodies (quest death
effects) belong to the quests specs (Open question 4).

### 16. Umod 23 poisonhit: mode 0 `0x005A3490`

Cross burst (§3.3) of missile 321 `queenpoisoncloud` with L = level.
No unique test: it runs on every mode set except GH (§2 r1).

### 17. Umod 24 thief: mode 3 `0x005A30E0`

Unreachable in 1.14d: monumod row 24 has `enabled` 0 (never picked,
`init.md` §17.3), superunique `Mod` columns skip 24 (`init.md` §20
step 2; Lord De Seis lists it), and no assign site passes 24. Body:

1. unique = 0 → nothing. One U step; lo' % 100 < 30 → nothing.
2. T = target (`skills/bodies.md` §2.1); not a player → nothing.
3. First non-empty belt slot i = 0 … 15 of T's inventory (+0x60,
   `0x0063C7F0`); none → nothing. The item must be type 4, mode 2, and
   T must have no cursor item (`0x0063C1E0`); else nothing.
4. Copy = `0x0055A2A0(game, item, T, 1)`; free spot near T
   (`0x00620870`, `0x00555DA0`; none is a fatal assertion); copy item
   flag 0x2000 (`0x006280D0`); drop it there (`0x00558AA0`); remove the
   original from the belt (`0x00561E70`) and refresh the belt
   (`0x0055EDC0`). Items spec seams.

### 18. Umod 27 spectralhit

#### 18.1 Mode 0 `0x005A3040`

unique = 0 → nothing. One step of the **target unit's** seed (the
monster here); k = lo' mod 5 (unsigned). `elem_fill` (§3.4) with row k
of table `0x006E21B8`:

| k | s_min | s_max | s_len |
|---|---|---|---|
| 0 | 48 firemindam | 49 firemaxdam | −1 |
| 1 | 50 lightmindam | 51 lightmaxdam | −1 |
| 2 | 52 magicmindam | 53 magicmaxdam | −1 |
| 3 | 54 coldmindam | 55 coldmaxdam | 56 coldlength |
| 4 | 57 poisonmindam | 58 poisonmaxdam | 59 poisonlength |

#### 18.2 Mode 5 `0x005A30B0`

unique ≠ 0 and arg is a missile → §18.1 on the **missile** (its seed
steps; `elem_fill` sets its stats unless the row has `NoUniqueMod`).

### 19. Umod 29 multishot: mode 5 `0x005A3610`

1. unique = 0, or arg not a missile → nothing. m = its class.
2. O = owner(missile); none → nothing. m outside the missiles table,
   or the row has `NoMultiShot` (flags bit 12, mask `0x006CE278`) →
   nothing. O's type flag 0x80 set (`0x005A0180(O, 0x80)`) → nothing.
3. Target point: T = target(game, O) → T's position (`0x0045ADF0` /
   `0x0045AE20`); none → the missile path's target point (`0x00648A00`
   / `0x00648A10`).
4. A local seed {lo 'SEIS' (0x53454953), hi = O's GUID} is built
   (`0x00650E30`, `0x00650E40`) and never drawn from.
5. sx = sign(O.x − tx), sy = sign(O.y − ty) (−1, 0, 1). m in 63 … 66
   (`mummy1`–`mummy4`) → sx = sy = 0.
6. O's +0x16 |= 0x80.
7. `skill_missile(game, m, O, skill k, level l, 0, 0, tx − sy, ty + sx,
   quant 0)` then the same with target (tx + sy, ty − sx); k, l = the
   missile's skill and level (`0x0064A280`, `0x0064A210`).
8. O's +0x16 &= ~0x80.

The copies run O's mode-5 callbacks too (§12.2, §18.2), but step 2
stops their own multishot.

### 20. Umod 31 goboom: mode 2 `0x005A2840`

m = `skill_missile(game, 117, unit, 0, 1, 0, 0, x, y, 1)` at the unit's
position; none → nothing. Record: physical = fire = 0x6400 (100 <<
8). `area_damage(game, m, x, y, 6, rec, 0, 0, null, 0x583)`. No unique
test.

### 21. Umod 32 firespike_explode: mode 2 `0x005A3D20`

1. M = monstats row. n = `El1MaxD` − `El1MinD` (Normal columns +0x120,
   +0x10E, whatever d). v = (`El1MinD` + `roll(n)` on U) × 256.
2. r = level × `aip3` for d (+0x62 + 2d).
3. Record: fire (+0x10) = v. `area_damage(game, unit, x, y, r, rec, 0,
   0, null, 0)` with the **unit** as source.

Because the source is not a missile, `0x005AD730` applies nothing
(§3.2): the call has no effect beyond the U draw (Edge case 4).

### 22. Umod 33 suicideminion_explode

#### 22.1 Mode 1 `0x005A3E70`

1. Unit mode 3 (GH): mode set 0 on the unit, then event 7 at F + 4.
2. Unit mode 0 or 12: event 7 at F + 4.
3. Other modes: nothing.

The mode set in step 1 runs the dispatcher again: its mode-1 pass sees
mode 0 and schedules an event 7 at F + 4 first, so a hit that reaches
GH leaves two events (Edge case 5).

#### 22.2 Mode 2 `0x005A3EF0`

1. M = monstats row; a = `aip1` for d. Stats by level (`init.md` §8.1,
   flags 8) at the unit's level: A1 min p, A1 max q (out +0x14, +0x18).
2. v = (p + `roll(q − p)` on U) × 256.
3. Record (zeroed): result flags |= 8 (knockback); physical = v. a = 1:
   fire = v, missile 427 `suicidefireexplode`. a ≥ 2: cold (+0x24) =
   v, cold length (+0x30) = a, missile 428 `suicideiceexplode`. a ≤ 0:
   missile 426 `suicidecorpseexplode`.
4. m = `skill_missile(game, missile, unit, 0, 1, 0, 0, x, y, 1)`; m ≠
   none → `area_damage(game, m, x, y, r = aip4 for d (+0x68 + 2d), rec,
   0, 0, null, 0x581)` (players only).

### 23. Umod 34 ai_after_death

#### 23.1 Mode 1 `0x005A3840`

1. Unit mode ≠ 0 or alignment ≠ 0 → nothing.
2. Cancel the unit's type-7 events (`0x00540E60(game, unit, 7, 0)`).
3. One U step; lo' % 100 (unsigned) < `aip8` for d → event 7 at F + 1 +
   10 × `aip1` for d.

#### 23.2 Mode 2 `0x005A3910` (revive)

1. Cancel the unit's type-2 events.
2. Alive → nothing. Has a state of flag group 33 `udead`
   (`0x0063A770`, `data/runtime-maps.md` §4) → nothing. Unique (type flag 8) → nothing. Alignment 2 → nothing.
3. Footprint occupied: `0x0064D870(room, x, y, pattern 1, mask 0x3C01)`
   ≠ 0 (`sim/path-placement.md`) → event 7 at F + 1 + 10 × `aip1` for
   d; nothing more.
4. n = AI param 0 (`0x0058EC50(unit, 1)`); n ≥ 2 → nothing.
5. The raise test `0x00645510(unit, 0)` fails or alignment ≠ 0 →
   nothing.
6. `0x005DEAD0(game, unit, mode 8 (S1), skill 293 Self-resurrect, 0, 0,
   0)` (`ai.md` skill use); unit flags +0xC4 &= ~0x04020000; AI param 0
   := n + 1 (`0x0058EC00`); cancel type 2; event 2 at F + 51.

So a monster revives at most twice.

### 24. Umod 35 shatter_on_death: mode 1 `0x005A3A80`

Unit mode 0 → state 107 (`shatter`) on.

### 25. Umod 40 worms_on_death: mode 1 `0x005A4200`

Unit mode 0 → `0x005B2490(game, unit, class 551 painworm1, mode 1,
spread 1, flags 0)` (`init.md` §1: only in a populated room
(`0x0061A1F0`), class remapped for the level `0x0063EC70`, spawned near
the unit), then state 104 (`corpse_nodraw`) on.

### 26. Umod 41 always_run_ai: mode 2 `0x005A4230`

Dead → nothing. Else think restart (§3.5), then event 7 at F + 75. The
first event is scheduled by the init function (`init.md` §19.6). Every
extra type-7 event starts another 75-frame chain (Edge case 6).

### 27. Umod 42 lightningdeath: mode 2 `0x005A2910`

1. unique = 0 → nothing.
2. L = 1. O = minion owner; O has a skill entry of 268 (`Shadow
   Warrior`, `0x006439F0`) → k = skill level (`0x006442A0(O, entry,
   1)`) / 2 + 1; k > 1 → L = min(k, 15).
3. Life percent (`0x00621F20`) > 10 → L = 1.
4. `ring(game, unit, unit, 90 nova, skill 0, L, v = velocity of missile
   90 at level 0)` (`skills/bodies.md` §6.7).

Shadow Warrior and Shadow Master give umod 42 (`sumumod`). The event
comes from §4 (death), where life is 0, so step 3 changes L only on
another type-7 event.

## Constants & data dependencies

| Item | Value / column |
|---|---|
| monstats | `BaseId` (+0x02), `aip1` +0x56, `aip3` +0x62, `aip4` +0x68, `aip8` +0x80 (each + 2d), `El1MinD` +0x10E, `El1MaxD` +0x120 (Normal only); row size 0x1A8 |
| monlvl | `DM` / `L-DM`; via §8.1: `HP`/`L-HP`, `DM`/`L-DM` (A1) |
| monumod | `constants` of rows 28, 31 (§3.4) |
| difficultylevels | `MonsterCEDamagePercent` +0x3C (live 50 / 35 / 20) |
| skills row 66 | `aurarangecalc` +0x64, `auralencalc` +0x60, `aurastat1–6` +0x54…, `aurastatcalc1–6` +0x68…, `auratargetstate` +0x82 |
| missiles | 90 nova, 117 monstercorpseexplode, 119 frostnova (velocity), 155 corpsepoisoncloud, 194 coldunique, 195 lightunique, 225 buglightning, 321 queenpoisoncloud, 426–428 suicide explosions; flags `NoMultiShot` bit 12, `NoUniqueMod` bit 13 |
| classes | 156, 229, 242, 256, 267, 271, 326–330, 369, 479, 540–542, 551, 704, 705, 709; base classes 110, 118, 136, 247 |
| states | 21 stunned, 54 uninterruptable, 104 corpse_nodraw, 107 shatter; bitset 33 `udead` |
| skills | 66 amplifydamage, 268 Shadow Warrior, 293 Self-resurrect |
| delays | 2, 3, 4 frames (event 7), 51 (event 2), 75 (umod 41), 10 (lightning cooldown) |
| finder flags | 3, 0x581, 0x583 |

## Randomness

Per callback, in order (U = the monster's seed unless named):

| Callback | Draws |
|---|---|
| §5 curse | U one step; then none (apply_state draws nothing) |
| §6.2 fire | missile creation (`missiles.md`: game seed step for the missile, its init); U `roll(a − b)`; per damaged unit the damage path's draws on the missile seed (`missiles.md` §R6) and in damage apply |
| §7, §20, §22.2 | missile creation; §22.2 draws U `roll(q − p)` **before** it; §20 none on U; area damage as §6.2 |
| §10.2, §13.2, §16 | 8 missile creations (cross burst) |
| §11, §27 | 64 missile creations (ring) |
| §17 | U one step (unreachable) |
| §18.1 | U one step; §18.2: the missile's seed one step |
| §19 | none of its own (local seed unused); two missile creations |
| §21 | U `roll(El1MaxD − El1MinD)` (none when ≤ 0) |
| §23.1 | U one step (after the type-7 cancel) |
| §23.2 | none of its own; the skill use (`ai.md`) |
| §3.5, §4, §6.1, §8, §9, §10.1, §12, §13.1, §14, §15, §22.1, §24–§26 | none of their own (§9, §14, §22.1 mode sets and §25's spawn draw per their specs) |

## Edge cases & original bugs

1. Mode-0 callbacks never run for a GH mode set (§2 r1); mode 1 runs
   for every mode, including the same mode again.
2. Any type-7 event (quests, skills, population, umod 41) runs every
   mode-2 callback: e.g. a fire-enchanted monster that also has umod 41
   explodes every 75 frames while alive (§6.2 has no death test).
3. Fire, suicide (and every 0x581 caller) damage players only; goboom
   (0x583) also damages hostile monsters. The exploding monster's owner
   and the owner's owner are never hit.
4. Firespike explode (§21) applies no damage: `0x005AD730` ignores a
   non-missile source. Only its U draw remains.
5. A suicide minion hit into GH schedules two type-7 events at F + 4
   (§22.1), so §22.2 runs twice (two missiles, two damage passes, two
   U draws), unless something cancels one (Open question 2).
6. Each umod 41 event re-schedules itself; an extra type-7 event adds a
   second chain.
7. 1.14d scales the fire explosion by difficulty (§6.2 step 4: ×3/4,
   ×2/3, ×1/8) after `MonsterCEDamagePercent`; D2MOO 1.10f has neither
   the step nor the difficulty field read here.
8. Spectral hit uses the Normal-unique constants K[28], K[31] on every
   difficulty (§3.4).
9. A hit into GH bursts two frames later through event 7 (§10.1);
   any other non-stunning hit bursts at once (§10.3). Bursts closer
   than 10 frames apart are refused (§10.2 step 2).
10. §3.5 cancels a moving monster's think and schedules none (except
    the four base classes).
11. Lightningdeath's level rule reads the owner's Shadow Warrior skill
    even for a Shadow Master (skill 279).

## Test vectors

### Synthetic (CI-safe)

| Input | Expected | Source |
|---|---|---|
| fire, d 0, H 100, CE 50 | a = 50 − 12 = 38; b = 22; dmg = 22 + roll(16); rec physical = fire = dmg × 64; radius 4 | §6.2 |
| fire, d 1, H 1000, CE 35 | a = 350 − 116 = 234; b = 140; roll(94); radius 5 | §6.2 |
| fire, d 2, H 2000, CE 20 | a = 400 / 8 = 50; b = 30; roll(20); radius 6 | §6.2 |
| fire, a = 1 | b = 0; dmg = roll(1) = 0, one U step | §6.2, `rng.md` |
| curse, level 4 / 34 / 0 | L = 1 / 7 / 1; cast iff lo' & 3 ≠ 0 | §5 |
| lightning, level 1 / 9 | L = 1 / 4 | §10.2 |
| lightning, last burst F − 9 / F − 10 | refused (bit 0x100 cleared) / bursts, +0x18 := F | §10.2 |
| cold, level 3 / 40 | L = 1 / 20 | §11 |
| spcdamage, area level 10 | level 10, tohit 60; class 326: fire 5…15; 327: 6…10; 329: poison 10…20, length 20; 330: light 5…15 | §8 |
| spcdamage, room without a level id | level 2, tohit 52; 327: 2…2 | §8 |
| spcdamage, area level 50 | tohit 90 | §8 |
| hireable mode 0, min 3 max 7 sec 2, 5, class ≠ 271 | 4, 11 | §12.1 |
| hireable mode 5, missile 512…1024, owner 271 with sec 2, 5 | 768, 2048 | §12.2 |
| multishot, O at (10, 10), target (20, 10) | sx = −1, sy = 0; copies at (20, 9), (20, 11) | §19 |
| multishot, missile 64 (mummy2) | both copies at the target point | §19 |
| spectral k = 3, monlvl `L-DM` 19 (level 30, Normal, L-flag 1), coldlength total 0 | coldmindam 12, coldmaxdam 19, coldlength 40 | §3.4, §18 |
| ai_after_death mode 1, aip8 30, aip1 10, lo' % 100 = 29 / 30 | event 7 at F + 101 / none | §23.1 |
| revive, AI param 0 = 2 | nothing (type-2 events cancelled) | §23.2 |
| lightningdeath, owner SW level 1 / 2 / 40, life 0 % | L = 1 / 2 / 15 | §27 |
| think restart, mode 2 (WL), base class 19 / 110 | type 2 cancelled, none scheduled / event 2 at F + 2 | §3.5 |

### Real 1.14d values (live tables; `#[ignore]`, `D2_GAME_DIR`)

| Case | Expected | Source |
|---|---|---|
| fire-enchanted unique fallen1, Normal, level 4, L-flag 1 | H = pct(15 (`L-HP` level 4), 61 (`maxHP`), 100) = 9; a = pct(9, 50, 100) = 4 → 3; b = 1; dmg ∈ {1, 2}; fire = physical = 64 or 128 | §6.2 with monlvl / monstats / difficultylevels |
| suicideminion1, Normal, level 31, L-flag 1 | `L-DM` 20: p = 20, q = 30; v = (20 + roll(10)) × 256; `aip1` 15 → cold = v, cold length 15, missile 428; radius `aip4` 4 | §22.2 |
| wakeofdestruction (umod 32), Normal | v = (5 + roll(5)) × 256; r = level × 15; no damage applied | §21 |

## Provenance

- 1.14d `Game.exe` (`re/exports/all.asm`, `disasm.py fn/at`, table
  bytes from `game/Game.exe`): dispatcher sites in `0x005A7C20`
  (`0x005A7D42` mode 0 after `0x005A4F50` at `0x005A7D39`, skipped by
  the mode-3 test at `0x005A7CC9`; `0x005A7DC8` mode 1 after
  `0x005533D0` and before `0x00553990`), `0x0057C83F` (mode 3),
  `0x0057D0ED` / `0x0057D122` (mode 4), `0x0059FEAC` (mode 5);
  callbacks `0x005A2530`, `0x005A23D0`, `0x0056DBC0`, `0x005A25F0`,
  `0x005A2620`, `0x005A2840`, `0x005A2910`, `0x005A29A0` (tables
  `0x006E2178`, `0x006E2188`), `0x005A2BA0`, `0x005A2BD0`, `0x005A2C20`,
  `0x005A2CC0`, `0x005A2D10`, `0x005A2D80`, `0x005A2E00`, `0x005A2EB0`,
  `0x005A2EE0` (`0x006E2198`, `0x006E21A8`), `0x005A3040` (table
  `0x006E21B8` dumped: 48 49 −1, 50 51 −1, 52 53 −1, 54 55 56, 57 58
  59), `0x005A30B0`, `0x005A30E0`, `0x005A3250` (jump tables decoded),
  `0x005A3490`, `0x005A3610`, `0x005A37D0`, `0x005A3800`, `0x005A3840`,
  `0x005A3910`, `0x005A3A80`, `0x005A3AA0`, `0x005A3B50`, `0x005A3D20`,
  `0x005A3E70`, `0x005A3EF0`, `0x005A4200`, `0x005A4230`; helpers
  `0x005A21D0`, `0x0057E090`, `0x0065A950`, `0x0065AC70`, `0x0065AA40`,
  `0x0065A6B0`, `0x0065A710`, `0x00645950`, `0x00552FD0`, `0x005AD730`
  (type-3 guard), `0x00573780` (jump table `0x00573830` / `0x00573838`
  decoded: 110, 118, 136, 247), `0x00463860`, `0x005B2490`; the reaction
  `0x0057CEE0` branches around `0x0057D0B2`–`0x0057D12D`.
- Fire step 4 arithmetic: d = 1 uses `imul 0x55555555; sub; sar 1;
  add sign` = signed division by −3, checked numerically for −4 … 302.
- Assign sites of `0x005A4850` (all 52 scanned for immediate umods):
  restore `0x005424F0`, hireling `0x00573270` (19), bone prison / wall
  `0x005AEDA0`, `0x005C58B0`, `0x005C5BC0` (15), temporary summons
  `0x005C07A0`, `0x005C55C0`, `0x005D6E70`, `0x005DC000`, `0x005DFBF0`
  (21), summon `sumumod` `0x005C4470`, `0x005D1E10`, `0x005D6CF0`
  (skills.txt: 262, 272, 276 → 32; 268, 279 → 42; 295 → 33),
  superuniques `0x005A49B0`, boss mods `0x005B1CF0`.
- Live tables `game/extracted/patch_d2/data/global/excel/`: monumod
  (row 24 `enabled` 0), superuniques (`Mod` values), difficultylevels,
  monstats (class ids by `hcIdx`), missiles (`Id`), states, skills.
- D2MOO (1.10f) `MonsterUnique.cpp` used as a map (callback table and
  names); each body above was re-read on 1.14d. Differences: the fire
  explosion difficulty step (Edge case 7) and its 0x581 finder flags,
  the lightning burst bookkeeping fields, the uber cases of §15.

## Open questions

1. No recording covers a callback: record a fire-enchanted unique dying
   next to a player (rng and timer hooks) to confirm §2, §4, §6 and the
   draw order.
2. Edge case 5: whether the death start or the event handler drops the
   second suicide-minion event 7; settle with a recording of a suicide
   minion hit into GH.
3. Answered (2026-10-07): §3.1 is the single owner of the unit find;
   `missiles/bodies-2.md` §44 and `monsters/ai-bodies-3.md` Open
   question 3 point to it. Merged from bodies-2 §44 and re-read on
   1.14d: the finder callback that replaces the filter, the finder
   record layout, the room-flag field +0x14 (written by the caller after
   init, not the filter flags), the town level ids, the overlap test
   that never rejects, filter rule 1 (A none) and the F & 0x200 line
   iterator.
4. Quest death effects `0x005DFD90`, `0x005DFDB0`, `0x005DFE00`,
   `0x005DFE20`, `0x005E0020`, `0x005E0040`, `0x005E0060`,
   `0x005E0070` (§15) have no spec: the quests specs should own them.
5. Answered (2026-10-07): no fixed site assigns 40 or 41. The
   table-driven assigns of `0x005B21B0` give 14, 33, 34, 35 and 22
   (`init.md` §14.1), the boss mods 6, 8, 12, 17, 18, 22, 23, 29, 30, 31
   (`init.md` §14.3), the superunique cases 22 (§20.1). Of the 52
   `0x005A4850` sites only four take a non-constant umod: the restore
   copy `0x005424F0` (saved lists) and the summon sites `0x005C4470`,
   `0x005D1E10`, `0x005D6CF0`, which pass skills `sumumod` (+0xE4) when
   it is 1…42; 1.14d skills.txt uses 32, 33 and 42 only. The
   data-driven appends cannot reach them either: superuniques.txt
   `Mod1`–`Mod3` use 1, 5–9, 17, 18, 23–28, 30 and monumod rows 40, 41
   have no `cpick` / `upick`. So 40 and 41 occur only through modded
   data or a saved list.
6. A missile's `level(12)` (§3.4 step 2 for §18.2): `missiles.md`
   §R2.3 step 23 does not list stat 12 among a missile's stats; if it
   is 0 the monlvl row is 1. Settle with the missile stat list dump of
   a spectral-hit unique's missile.
