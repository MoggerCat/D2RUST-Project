# Spec: Combat — Vitals: creation values, stat points, level-up, experience, client sync

- **Status:** draft: creation (§1), stat points (§2), level-up (§3), the
  client vitals sync (§5) and
  the experience table lookups (§4.1) read in full from the 1.14d
  `Game.exe`; the experience-on-kill level factor (§4.2) read in full;
  the rest of §4 (experience ratio, party share, hireling experience) is
  D2MOO 1.10f structure that is only partly confirmed (Open question 2).
  No trace check yet.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::combat::vitals` (player creation and
  level-up may live in `d2-sim::units::player`)
- **Related specs:** `sim/stat-lists.md` §10.1 (owns regeneration:
  timer event 3, life / mana / stamina per tick, poison and open-wounds
  life loss through stat 74, monster regeneration and death by
  regeneration) and §7.2 (current life/mana/stamina rescaled when the
  maximum changes; monster `DamageRegen` → stat 74); `sim/stats.md` (op
  9: item vitality / energy → max life / mana / stamina); `combat/damage.md`
  (§7.2 kill, which triggers experience); `skills/levels.md` (skill
  points, max skill level from the same table); monster vitals at spawn:
  the monsters spec (Open question 5).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 43–57 |
| Inputs | 58–66 |
| Outputs / state changes | 67–70 |
| Rules | 71–72 |
|   1. Creation values | 73–95 |
|   2. Spending stat points (message 0x3A) | 96–145 |
|   3. Level-up `0x00570880` (D2MOO `PLAYERSTATS_LevelUp`) | 146–167 |
|   4. Experience | 168–335 |
|   5. Client vitals sync (`0x00548760`) | 336–472 |
| Constants & data dependencies | 473–489 |
| Randomness | 490–493 |
| Edge cases & original bugs | 494–505 |
| Test vectors | 506–526 |
| Provenance | 527–553 |
| Open questions | 554–586 |
<!-- /index -->

## Summary

A new player gets its attributes and life, mana and stamina from
`charstats.txt`. Spending a stat point raises the base attribute and,
for vitality and energy, the base maximum and current life, stamina and
mana by quarter points per point. Gaining a level adds the per-level
amounts, refills life (if alive), mana and stamina, and grants stat and
skill points. Levels come from the experience table. Experience for a
kill is scaled by the level difference. Regeneration over time is not
here (`sim/stat-lists.md` §10.1).

Units: life, mana and stamina stats (6–11) are 1/256 points. The
charstats per-level and per-point columns are quarter points, so a
column value `v` adds `v << 6` units.

## Inputs

| Name | Type | Source |
|---|---|---|
| `charstats.txt` (0xC4-byte records at `[0x744304]+0xBC4`) | str +0x30, dex +0x31, int +0x32, vit +0x33, stamina +0x34, hpadd +0x35, LifePerLevel +0x43, StaminaPerLevel +0x44, ManaPerLevel +0x45, LifePerVitality +0x46, StaminaPerVitality +0x47, ManaPerMagic +0x48, StatPerLevel +0x50 (i8) | class id |
| experience table `[0x0096C8A8]` | u32 rows of 8 (Amazon … Assassin, ExpRatio) | `experience.txt` |
| client message 0x3A | `client-messages.tsv` | §2 |
| stats | base getter `0x006253B0`, unit getter `0x00625480`, set `0x00627260`, add `0x006272B0` | `sim/stats.md` |

## Outputs / state changes

Base stats 0–5, 6–13, 19, 20, 30 (`nextexp`), 67–69 of the player.

## Rules

### 1. Creation values

`init_player_stats(game, unit, act)` = `0x005706D0` (D2MOO
`PLAYERSTATS_SetStatsForStartingAct`). `act ≥ 5` is treated as 4. With
the class's charstats record (none → nothing), set base stats:

| Stat | Value |
|---|---|
| 0 strength, 1 energy, 2 dexterity, 3 vitality | `str`, `int`, `dex`, `vit` |
| 19 tohit, 20 toblock | 0 |
| 6 hitpoints, 7 maxhp | `(vit + hpadd) << 8` |
| 8 mana, 9 maxmana | `int << 8` |
| 10 stamina, 11 maxstamina | `stamina << 8` |
| 12 level | 1 |
| 30 nextexp | `threshold(class, 1)` (§4.1) |
| 68 attackrate, 67 velocitypercent, 69 other_animrate | 100 |

Then, if `act > 0` (and `act <` the table count 5 at `0x007326B4`,
else index 0): raise experience to the target level of table
`0x006E1520` {1, 15, 20, 26, 32} by index `act` (`0x0057EB10`, D2MOO
`SUNITDMG_SetExperienceForTargetLevel`: add `threshold(class, target) −
experience` if positive, through the add §4.5, which levels up).

### 2. Spending stat points (message 0x3A)

Handler `0x0054BD10`: size must be 3, else 3. Byte +1 is the stat id
`s`, byte +2 is `count − 1`. `s > 15` or `count − 1 > 99` → 3. Repeat
`count` times `spend(unit, s)` (`0x00570D60`); the first failure stops
and returns 2; else 0. (`client-messages.tsv` describes the field as a
u16 stat: Open question 4.)

`spend(unit, s)`: `statpts(4)` (unit getter) = 0 → fail. By `s`:

- 0 strength, 2 dexterity: `statpts −= 1`, stat `+= 1` (add), then the
  refresh `0x0064C040(unit)`.
- 1 energy: `gain_energy(unit, 1)` (`0x00570A80`).
- 3 vitality: `gain_vitality(unit, 1)` (`0x00570B60`).
- any other id (4…15): fail.

`gain_energy(unit, n)`: `statpts −= n`; `energy += n`; if `n > 0`: `mana
+= (ManaPerMagic × n) << 6`; `maxmana += (ManaPerMagic × n) << 6`; if
the unit's mana (unit getter) > its max mana: set mana = max mana.

`gain_vitality(unit, n)`: `statpts −= n`; `vitality += n`; `maxhp +=
(LifePerVitality × n) << 6`; if `n > 0`: `hitpoints += (LifePerVitality
× n) << 6`; life > max life → life = max life; `maxstamina +=
(StaminaPerVitality × n) << 6`; if `n > 0`: `stamina += (…) << 6`;
stamina > max stamina → clamp.

All additions go to base stats (add `0x006272B0`). A negative `n`
(stat reset, §2.1) lowers the maximum but not the current value, which
is then clamped.

#### 2.1 Stat reset `0x00570C80`

Players only: for strength, energy, dexterity, vitality in that order,
`d = charstats start value − base value`; strength and dexterity: if `d
≠ 0`, `statpts −= d`, stat `+= d`, refresh; energy: `gain_energy(unit,
d)`; vitality: `gain_vitality(unit, d)`. Fastcall ECX game, EDX
player; a non-player, a class outside `charstats` or no record →
nothing. Start values: `charstats` +0x30 str, +0x32 int (energy),
+0x31 dex, +0x33 vit (u8); base values through the base getter
`0x006253B0` (stats 0, 1, 2, 3).

Callers (exactly two, 1.14d-confirmed by `disasm.py xref 0x570C80`),
both running the skill reset `0x00570360` (`skills/levels.md` §6.5)
immediately before, then the sound `0x00553380(player, 2)` after:

| Call site | Caller | Trigger | Spec |
|---|---|---|---|
| `0x0055E552` | `0x0055E170` (use grid item) | using a `toa` (Token of Absolution); the token is then consumed (`0x0055E000`) | `items/inventory.md` §7.11 |
| `0x0057A24B` | `0x00579D60` (NPC action) | Akara's respec, when quest slot 41 bit 1 is set; then `0x0058FD50` | `world/npc.md` §8.2 |

### 3. Level-up `0x00570880` (D2MOO `PLAYERSTATS_LevelUp`)

1. `old = level(12)` (base); `new = level_from_exp(class,
   experience(13))` (§4.1); set level = `new`; set `nextexp(30) =
   threshold(class, new)`.
2. `d = new − old`; `d ≤ 0` → stop.
3. `maxhp = base maxhp + (LifePerLevel × d) << 6`; if life (unit getter)
   > 0: life = max life (`0x00625D10`).
4. `maxmana = base + (ManaPerLevel × d) << 6`; mana = max mana
   (`0x00625D60`).
5. `maxstamina = base + (StaminaPerLevel × d) << 6`; stamina = max
   stamina (`0x00625DB0`).
6. `statpts(4) += StatPerLevel × d` (signed byte; 5 for every 1.14d
   class); `newskills(5) += d`.
7. Notifications: `0x00536850` (party roster, every living player),
   `0x00553380(unit, 2)` (level-up sound), `0x005538D0(game, unit,
   0x00570850)`, `0x0055F500`, `0x0055FDE0(…, 1)` (client updates),
   host callback `[0x00883D50]+0x2C` if present.

The caller of level-up (the add, §4.5) triggers unit event 12 (`levelup`) after
it (`0x0057E58A`).

### 4. Experience

#### 4.1 Table lookups

`[0x0096C8A8]` points to `experience.txt` as u32 rows of 8 columns
(classes 0–6, then `ExpRatio`): row 0 is `MaxLvl`, row `L + 1` is level
`L`. Class ids outside 0–6 use class 0.

- `max_level(class)` = row 0 (99 for every 1.14d class; `0x00611830`).
- `threshold(class, L)` = `0x00611800`: row `L + 1` (the experience
  needed to reach level `L + 1`; level 1 → 500).
- `level_from_exp(class, exp)` = `0x00611860`: `i = 0`; while `exp ≥
  row(i + 1)` (unsigned) and `i < max_level`: `i += 1`. Return `i`.
  (Level 0 has threshold 0, so any experience gives at least 1.)

#### 4.2 Level factor `0x0057E2F0(exp, alvl, dlvl)`

`dlvl ≤ alvl`: `f = T1[min(alvl − dlvl, 10)]`, `T1` (`0x006E1668`) =
256, 256, 256, 256, 256, 256, 207, 159, 110, 61, 13. `dlvl > alvl`: if
`alvl ≥ 25` and `dlvl > 0` → result `pct(exp, alvl, dlvl)`; else `f = T2[min(dlvl −
alvl, 10)]`, `T2` (`0x006E1694`) = 256, 256, 256, 256, 256, 256, 225,
174, 92, 38, 5. Result `f = 256` → `exp`; else `pct(exp, f, 256)`
(`combat/damage.md` §0). Signed comparisons.

#### 4.3 Gain `0x0057E480`

Registers: EAX experience e, EBX gainer U, EDI attacker level alvl;
stack game, defender level dlvl. Returns the gain.

1. e > 0x7FFFFF → e := 0x7FFFFF; else e ≤ 0 → return 1.
2. alvl ≥ `max_level(class)` (U's class when U is a player, else class
   0; §4.1) → 0.
3. g := level factor(e, alvl, dlvl) (§4.2).
4. `ExpRatio` step `0x0057E390(g, alvl)`: ratio(L) = `0x00613E60(L)`:
   no table → 0; L < 1 → the `MaxLvl` row's `ExpRatio`; 1 ≤ L ≤ that
   row's class-0 value (99) → the `ExpRatio` of level L (table word 8·L
   + 15); above → 0. g ≤ 0 → unchanged. r := ratio(alvl), s :=
   ratio(0) (10 in 1.14d); s − 1 ≥ 31 unsigned → unchanged. limit :=
   0x7FFFFFFF >> (((r >> s) + s) & 31); g > limit (signed) → (g >> s)
   · r; else (r · g) >> s (32-bit signed, arithmetic shifts). Live
   `ExpRatio`: 1024 for levels 0–69, −48 per level to 256 at 85, then
   192, 144, 108, 81, 61, 46, 35, 26, 20, 15, 11, 8, 6, 5 (86–99).
   Players and hirelings alike (`world/hirelings.md` §7.2 rule 3).
5. x := U's total `item_addexperience` (85); x ≠ 0 → g += pct(g, x,
   100) (`combat/damage.md` §0).
6. Cap `0x0057E3F0(U, alvl, game, g)`: U a player → g. Otherwise U's
   owner (`0x0058F0D0`) must be a player with a pet node for U and a
   hireling row: g := min(g, (threshold(alvl + 1) − threshold(alvl))
   >> 6) (unsigned; `world/hirelings.md` §7.2 rule 4); no such owner,
   node or row → 0.

#### 4.4 Distribution on a kill `0x0057E990(game, attacker A, defender D)`

Called by the kill (`combat/damage.md` §7.2 step 2) when D lacks unit
flag 0x04000000. Stats here are **base** reads (`0x006253B0`) unless
marked total.

1. A or D none, or A neither player nor monster → nothing. e := D's
   experience (13) ≤ 0 → nothing.
2. Credited player P (`0x0057E7B0`): A a player → A. A a monster: P :=
   A's owner (`0x0058F0D0`); then a stat list with flag 0x800
   (`0x00625760`) is looked up on A (when A has a stat holder) and then
   on D (when D has one; D's result replaces A's, even when none); a
   list found → P := the unit of that list's owner type and GUID
   (`0x00552F60`). P must be a player, else nothing.
3. dl := D's level (12). Hireling H of P (`0x00574EC0(game, P, 7,
   0)`): g := gain(e, U = H, alvl = H's level, dlvl = dl); A ≠ H → g :=
   g · 86 / 256 (signed, toward zero); add to H (`0x0057E860`,
   `world/hirelings.md` §7.3).
4. pl := P's level (12). P in a party (`0x00554630(P)` ≠ 0xFFFF) →
   party share (rule 6) with (e, D, pl, dl). Else g := gain(e, P, pl,
   dl); add(P, pl, g) (§4.5).
5. The hireling share does not reduce the player's.
6. **Party share** `0x0057E6C0`: members are collected in order by
   `0x005405A0(game, P, cb, ctx)`: P without a room or without a party
   → P only; else each party member (`0x00540290` list, GUID →
   player) whose room's level (`0x0061A1B0`) equals P's. The callback
   `0x0057E5A0` skips a dead member (`0x005541B0`); with D, it skips a
   member when D's x or the member's x is 0, or when (Δx² + Δy²) >
   6400 (unsigned; positions from the static or dynamic path); a 9th
   member is fatal (assert 0xF25). Each kept member i stores the unit,
   L_i := its **total** level (12), and adds L_i to S. n := count.
   - n ≤ 0 or S ≤ 0 → nothing.
   - n = 1 → g := gain(e, **P**, pl, dl); add(P, pl, g) (P, not the
     counted member).
   - n ≥ 2: t := e + ((n − 1) · e · 89) / 256 (32-bit signed, toward
     zero). q := t / S as x87 `fild`/`fidiv` stored to a float32. For
     each member in order: share := L_i × q (x87 `fild` L_i × float32
     q) truncated toward zero by `0x00682FD0`; g := gain(share, member,
     L_i, dl); add(member, L_i, g).

#### 4.5 Add `0x0057E510(game, level L0, gain g)` (ESI = player U)

U none or not a player → nothing. old := U's experience (13, base);
new := old + g (32-bit); cap := threshold(class, max_level(class) − 1)
(§4.1); new > cap (unsigned) → cap. Stat 29 (`lastexp`) := new − old;
stat 13 := new. `level_from_exp(class, new)` ≠ L0 → level-up §3
(`0x00570880`) and unit event 12 (`0x005C0C30(game, 12, U, 0, 0)`).

#### 4.6 Death penalties `0x00535AB0(game, player P, killer K)`

Caller `0x00580F59` (player death). In order:

1. **Gold** `0x005357D0` (ESI P; game, K). L := level (12, total), gi
   := gold (14), gs := goldbank (15) (totals), T := gi + gs. q :=
   (min(L, 20) × T) / 100 (signed, toward zero); keep := T − q; base :=
   L × 500. pvp := K present, K ≠ P, and K a player or a monster whose
   owner is a player.
   - Game type (game +0x6A) = 3 (the value on every tick of the
     single-player traces, `sim/intents-events.md`): keep < base → q :=
     max(T − base, 0). Then q := min(q, gi).
   - pvp, q > gi (other game types only): stat 15 := gs − (q − gi) (0
     when < 0 or above the stash limit `0x00623460`); stat 14 := q (0
     when < 0 or above the gold limit `0x00622E70`). Then (pvp, any q)
     drop q gold (`0x00535510(game, P, P's GUID, q)`,
     `items/inventory.md` §7.22).
   - Not pvp, q ≤ gi: drop gi − q gold the same way; then stat 14 := 0.
     Not pvp, q > gi (other game types only): stat 15 := gs − (q − gi)
     with the same clamps; stat 14 := 0.
   - Finally stat 175 (`goldlost`) := q (0 when q < 0).
2. **Experience** `0x005359F0`, skipped when K is a player or a
   monster owned by a player: c := P's class, L := level (total); L ≤ 1
   → nothing. lo := threshold(c, L − 1), hi := threshold(c, L); x :=
   experience (13, total); loss := (`DeathExpPenalty` × (hi − lo)) /
   100 (32-bit unsigned product and division; `difficultylevels` +0x04
   of the game's difficulty: 0 / 5 / 10). loss = 0 → nothing. new := x
   − loss; new ≤ lo (unsigned) → loss := max(x − lo, 0) (signed), new
   := lo + 1. `0x005391E0(client, loss)` on P's client (`0x005531C0`):
   not a message: it stores `loss` in the client record at +0x508
   (fatal assert when `loss` < 0); stat 13 := new. The stored loss is
   what the corpse later returns (§4.7).

So a death never lowers the level: experience stops at the level's
first point plus one.

#### 4.7 Corpse experience

Client +0x508 ("experience lost", u32) starts at 0 (the client record
is zero-filled at allocation, `0x00539A30`). Its only writer is §4.6
rule 2 (`0x005391E0`, one caller `0x00535A8A`); its only reader is the
getter `0x00539210` (one caller, `0x0057F80F`). 1.14d-confirmed (the
three `+0x508` references to the client record in `all.asm`).

1. **Corpse creation** `0x0057F700` (from the mode-17 `DD` start
   `0x0057FCA0` at `0x0057FD1C`, `sim/units.md` §4.5): the corpse C is a new player-type
   unit of P's class in mode 17 holding P's items (`items/inventory.md`).
   v := client +0x508; C's stat 13 (`experience`) := `pct(v, 75, 100)`
   (`combat/damage.md` §0; inline at `0x0057F816`–`0x0057F86C`: v >
   0x100000 → (v / 100) × 75, else v × 75 / 100, 32-bit, toward
   zero); then client +0x508 := 0. A death with no experience loss
   (pvp killer, level 1, `DeathExpPenalty` 0) leaves the field 0, so
   C holds 0.
2. **Corpse pickup** `0x0057FB70(game, player P, corpse C)`
   (the 0x16 PickItem path on a dead player, `items/inventory.md`
   §7.1 rule 2): C must have state 7
   (`playerbody`) and P must be allowed to take it (`0x0057FAF0`: C's
   owner GUID, from C's inventory `0x0063D450`, equals P's GUID, or
   the owner is a player for whom `0x0055B300(owner, P, 1)` holds).
   Only when the owner GUID is P's own: x := C's stat 13; x ≠ 0 → C's
   stat 13 := 0 and `0x0057EAB0(game, P, x)`, which for a player is the
   add §4.5 with L0 = P's base level (12) (a monster with a player
   owner takes the owner path `0x0057E860` instead). The item take-back
   follows (`0x00562F30`).

So a recovered corpse returns 75 % of the last death's loss, capped by
§4.5; a second death before pickup overwrites the field, and the
earlier corpse keeps its own stat 13.

### 5. Client vitals sync (`0x00548760`)

Sends a player's own client its life, mana, stamina, position, gold and
experience (S→C 0x18, 0x95, 0x96, 0x19–0x1F). Owner of these messages;
`items/inventory.md` §10.3 (gold bytes) and `sim/pathing.md` §10 rule 5
link here.

#### 5.1 When it runs

1. Caller `0x0052D980` (ESI = client), from the flush routine
   `0x0052E320` when its second argument is 1 and the client state
   (client +4) is 4 (in game): every flush of `0x0052FD90` (single
   player: once after each tick that ran, `sim/intents-events.md` §1
   rule 1), before the client's buffers are sent, so these messages end
   the tick's batch. The leave flush `0x005303D0` passes 0 (no sync).
2. force := 1 when client +0x1B0 ≥ 20, or when client +0x1B0 ≥ 10 and
   the client has a queued buffer (head, client +0x1B8 ≠ 0); else 0.
   Client +0x1B0 counts per-client updates (`sim/tick.md` §6 rule 5,
   +1 per tick) and is reset to 0 when the routine returns 1.
3. The client's unit (`0x00537860(client, 0)`) missing → nothing (the
   counter keeps counting). Not a player → fatal assert (`0x0052D9BD`,
   process exit): unreachable, a client's unit is always its player; an
   implementation asserts. The routine runs with ECX = EDX = that
   player.
4. Before it, when the host setting at `0x00883D4C` is non-zero,
   `0x0052DA00` runs. The setting is the registry value `PlayerPos` of
   the `Diablo II` key (HKCU, then HKLM; `0x00414F10` → `0x00414B00`,
   `RegOpenKeyExA` / `RegQueryValueExA`, text parsed by `strtoul`),
   read once at server start (`0x00530690`); absent → 0. A standard
   1.14d install has no such value, so `0x0052DA00` never runs and
   d2rs does not run it (`Ruleset::Original`). What it does when on:
   head buffer B (`0x005392E0`), the client's unit and client +0x1B0 ≥
   10 all required; n = (499 − B's size, B +0x00) / 9 (signed,
   truncating); n ≤ 0 → nothing; n > 55 → fatal; else `0x00537FD0(client,
   n)` and `0x0053E130`:
   1. **Nearest players** `0x00537FD0` (ECX client, EDX n): client room
      (+0x1B4) null → fatal. C := the client's unit; none → nothing.
      (px, py) := C's sub-tile position. A list of up to n nodes {unit,
      d, next} is built from every unit of every room in the client
      room's room list (`0x00619790`, `drlg/rooms.md` §10.4), rooms in
      list order, units in room-list order (room +0x74, next +0xE8);
      only players (type 0, C included) count, d := |x − px| + |y −
      py| (`0x00537D10`). The list is kept sorted by ascending d: a new
      player with d below the first node's becomes the first node;
      otherwise it goes before the first node after the first whose d
      ≥ its own (so it ties after the first node but before any other),
      else at the end; once n are held, a
      player with d ≥ the current maximum is skipped, else the last
      node is dropped and its slot reused. Then `0x00537EE0` copies the
      list from its first node into client +0x1CC as 9-byte records
      {u8 type, u32 GUID, u16 x, u16 y} (players: path position) and
      the count into client +0x3BC, stopping **before** the node whose
      next is none: the farthest kept player is never copied, except
      when the list holds one player.
   2. **Send** `0x0053E130` (ECX client): count c = client +0x3BC; c = 0
      → nothing. Appends to the head buffer: u8 0x16, u16 size @1 =
      9c + 13, u8 c @3, then the c records from @4; the buffer length
      grows by 9c + 13, so the last 9 bytes of the message are left
      unwritten (whatever the buffer held).
   1.14d-confirmed (`0x0052DA00`, `0x00537FD0`, `0x00537D10`,
   `0x00537EE0`, `0x0053E130`).

#### 5.2 Values

Per-client cache record at client +0x48C (`0x00539330`). The client
record (0x518 bytes, `0x00539A30`) is zero-filled at allocation and
nothing else writes +0x48C … +0x4A7, so every cache field starts at 0
and keeps its last sent value for the client's whole life (no reset on
level change or rejoin of the same record). With life cache 0, the
first sync after joining passes step 2 once d · 100 / M ≥ 10 (life ≥ 10
% of max) or force is 1 and sends 0x95 or 0x18 (lp / mp 0 equal the cache unless a
potion runs):

| Offset | Field |
|---|---|
| +0x00 | quiet counter (u32) |
| +0x04 / +0x06 / +0x08 | life / mana / stamina sent (u16) |
| +0x0A / +0x0B | life / mana prediction sent (u8) |
| +0x0C / +0x0E | x / y sent (u16) |
| +0x10 / +0x12 | dx / dy sent (u16, zero-extended bytes) |
| +0x14 | gold sent (stat 14) |
| +0x18 | experience sent (stat 13) |

Current values (stat totals, `0x00625480`; `>>` arithmetic):

- L = total(6) >> 8; M = max life (`0x00625D10`) >> 8; mana =
  total(8) >> 8; stamina = total(10) >> 8.
- X, Y = the unit's sub-tile position (`0x0045ADF0` / `0x0045AE20`);
  with a path: dx = (X − path target x, path +0x10) & 0xFF, dy = (Y −
  path target y, +0x12) & 0xFF; without a path dx = dy = 0.
- Life prediction lp (`0x005485B0`): with a state 100 (`healthpot`) list
  and M ≠ 0: q = (total_list(74) · (list expire frame − game frame) +
  total(6)) >> 8; v = q · 100 / M (signed, truncated); lp = the **low
  byte** of v, then 100 if that byte is above 100. Else lp = 0.
- Mana prediction mp (`0x00548640`): with a state 106 (`manapot`) list,
  max mana m (`0x00625D60`, 8.8) ≠ 0 and a valid class row: per-frame
  regen i as `sim/stat-lists.md` §10.1 rule 5 (q = charstats `ManaRegen`
  · 25, 7500 if 0; i = max(m / q, 1)), except that the stat 27 scaling
  is truncated, i = trunc(i · (total(27) + 100) / 100) + total(26), and
  state 85 is not tested; v = ((list expire − frame) · i + total(8)) ·
  100 / m; mp = low byte of v, 100 if above 100. Else mp = 0.

#### 5.3 Steps

1. Client or unit missing, or M ≤ 0 → return 0 (nothing changes).
2. force = 0: d = |cache life − L|; d · 100 / M < 10 → return 0. Else if
   L = 0 and cache life ≠ 0 → return 0 (a drop to zero life is not
   sent here).
3. Pick at most one message:
   1. lp or mp differs from the cache → 0x18 (life L, mana, stamina,
      lp, mp, X, Y, dx, dy; `0x0053C230`).
   2. Else L or mana differs → 0x95 (L, mana, stamina, X, Y, dx, dy;
      `0x0053C320`).
   3. Else stamina differs → 0x96 (stamina, X, Y, dx, dy; `0x0053C3F0`).
   4. Else, quiet counter > 3 and |cache x − X| ≥ 2 or |cache y − Y| ≥ 2
      → 0x96.
   5. Else nothing: quiet counter += 1.
   After a message: cache x, y, dx, dy := X, Y, dx, dy; quiet counter :=
   0.
4. Gold: total(14) ≠ cache → `0x0053E9B0(new, old)` (`items/inventory.md`
   §10.3 bytes); cache := new.
5. Experience: total(13) ≠ cache → `0x0053BDD0(new, old)`: δ = new − old
   (32-bit): δ unsigned > 0xFFFE → 0x1C [new u32]; δ ≥ 0xFF → 0x1B [δ
   u16]; else 0x1A [δ u8]. Cache := new.
6. Cache life, mana, stamina, lp, mp := current; return 1.

#### 5.4 Layouts

Bit-packed, LSB first from bit 0 of byte 0 (writer `0x00410EB0`; a value
is cut to its width); `sim/server-messages.tsv` holds the machine copy.

| Id | Size | Fields (bits) |
|---|---|---|
| 0x18 | 15 | id 8, life 15, mana 15, stamina 15, lp 7, mp 7, x 16, y 16, dx 8, dy 8 (115 bits) |
| 0x95 | 13 | id 8, life 15, mana 15, stamina 15, x 16, y 16, dx 8, dy 8 (101 bits) |
| 0x96 | 9 | id 8, stamina 15, x 16, y 16, dx 8, dy 8 (71 bits) |

## Constants & data dependencies

1.14d `charstats.txt` (fourths for the per-level / per-point columns):

| Class | str dex int vit | stamina | hpadd | Life/Lvl | Stam/Lvl | Mana/Lvl | Life/Vit | Stam/Vit | Mana/Magic | StatPerLevel |
|---|---|---|---|---|---|---|---|---|---|---|
| Amazon | 20 25 15 20 | 84 | 30 | 8 | 4 | 6 | 12 | 4 | 6 | 5 |
| Sorceress | 10 25 35 10 | 74 | 30 | 4 | 4 | 8 | 8 | 4 | 8 | 5 |
| Necromancer | 15 25 25 15 | 79 | 30 | 6 | 4 | 8 | 8 | 4 | 8 | 5 |
| Paladin | 25 20 15 25 | 89 | 30 | 8 | 4 | 6 | 12 | 4 | 6 | 5 |
| Barbarian | 30 20 10 25 | 92 | 30 | 8 | 4 | 4 | 16 | 4 | 4 | 5 |
| Druid | 15 20 20 25 | 84 | 30 | 6 | 4 | 8 | 8 | 4 | 8 | 5 |
| Assassin | 20 20 25 20 | 95 | 30 | 8 | 5 | 6 | 12 | 5 | 7 | 5 |

`experience.txt`: MaxLvl 99 for every class; level 1 → 500; level 99 →
3,837,739,017 (all classes equal).

## Randomness

None. (Regeneration: `sim/stat-lists.md`.)

## Edge cases & original bugs

1. Stat ids 4–15 in message 0x3A are accepted by the range check and then
   fail in `spend`.
2. `gain_energy` / `gain_vitality` with `n ≤ 0` do not change the current
   value except by the clamp.
3. Level-up refills life only for a living player; mana and stamina are
   always refilled.
4. Level-up for several levels at once multiplies every gain by `d`.
5. `level_from_exp` compares unsigned; experience above the level-99
   threshold stops at `max_level`.

## Test vectors

Real 1.14d data (game-file tests), from §1–§3:

| Case | Expected (1/256 units; points in parentheses) |
|---|---|
| Sorceress created | life 10240 (40), mana 8960 (35), stamina 18944 (74) |
| Barbarian created | life 14080 (55), mana 2560 (10), stamina 23552 (92) |
| Sorceress level 1 → 10 (d = 9), no stat points spent | max life 10240 + 9 × 4 << 6 = 12544 (49); max mana 8960 + 9 × 8 << 6 = 13568 (53); max stamina 18944 + 2304 = 21248 (83); statpts +45, newskills +9 |
| Barbarian level 1 → 10 | max life 14080 + 4608 = 18688 (73); max mana 2560 + 2304 = 4864 (19); max stamina 23552 + 2304 = 25856 (101) |
| Barbarian, +10 vitality spent | max life + 16 × 10 << 6 = +10240 (+40); max stamina +2560 (+10); statpts −10 |
| Sorceress, +10 energy | max mana +5120 (+20) |
| `level_from_exp(0, 499)` / `(0, 500)` | 1 / 2 |
| `threshold(0, 1)` | 500 |
| level factor: alvl 30, dlvl 22, exp 1000 | T1[8] = 110: `pct(1000, 110, 256)` = 429 |
| alvl 10, dlvl 18, exp 1000 | alvl < 25: T2[8] = 92: 359 |
| alvl 40, dlvl 50, exp 1000 | `pct(1000, 40, 50)` = 800 |

Synthetic: message 0x3A bytes `3A 03 04` (vitality, count 5) with 3
stat points: three spends succeed, the fourth fails, result 2.

## Provenance

- §4.3–§4.6: `0x0057E480`, `0x0057E390`, `0x00613E60`, `0x0057E3F0`,
  `0x0057E2F0`, `0x0057E990`, `0x0057E7B0`, `0x0057E6C0`, `0x0057E5A0`,
  `0x005405A0`, `0x0057E510`, `0x00535AB0`, `0x005357D0`, `0x005359F0`
  (caller `0x00580F59`); 1.14d `difficultylevels.txt` `DeathExpPenalty`
  0 / 5 / 10, `experience.txt` `ExpRatio`, `itemstatcost.txt` 29
  `lastexp`, 175 `goldlost`.
- 1.14d `Game.exe` disassembly: `0x005706D0`, `0x0054BD10`,
  `0x00570D60` (jump table `0x00570DF8`), `0x00570A80`, `0x00570B60`,
  `0x00570C80`, `0x00570880`, `0x00611800`, `0x00611830`, `0x00611860`,
  `0x0057E2F0`. Image data: `0x006E1520`, `0x007326B4`, `0x006E1668`,
  `0x006E1694`. Charstats offsets match `data/fields.tsv`.
- D2MOO 1.10f `PlayerStats.cpp`, `SUnitDmg.cpp` for names; §1–§3 and
  §4.2 agree with it. Answers `skills/levels.md` Open question 2:
  `[0x0096C8A8]` is the experience table and entry 0 is `MaxLvl`.
- 1.14d `charstats.txt`, `experience.txt` (`patch_d2`).
- §5 (sync): `0x00548760`, `0x005485B0`, `0x00548640`, `0x0052D980`,
  `0x0052E320`, `0x00539330`, `0x005392E0`, builders `0x0053C230`,
  `0x0053C320`, `0x0053C3F0`, `0x0053BDD0`, bit writer `0x00410E40`,
  `0x00410EB0`, `0x00410E90`, counter `0x005380D0`; client record
  allocation `0x00539A30` (0x518 bytes, memset 0; the only reference to
  +0x48C is the getter `0x00539330`, by a scan of `all.asm`),
  `0x0052DA00`, `0x00530690`, `0x00414F10`, `0x00414B00` (strings
  `Diablo II`, `PlayerPos` at `0x006CC8B8`, `0x006E08A0`; imports
  `RegOpenKeyExA` / `RegQueryValueExA`).

## Open questions

1. No trace check. Recording request: hook `0x00570880` entry/exit and
   `0x00570D60` (log stats 4–13 before/after) during a level-up and
   while spending points.
2. Answered: §4.3–§4.5 read from `0x0057E480`, `0x0057E390`,
   `0x0057E3F0`, `0x0057E990`, `0x0057E7B0`, `0x0057E6C0`, `0x0057E5A0`,
   `0x005405A0`, `0x0057E510`. Open only for the x87 party share: the
   precision-control word in force (§4.4 rule 6, as
   `sim/stat-lists.md` open question 1); settle with a party recording
   (multiplayer, Phase 7).
3. Answered: `0x0057E2F0` takes ECX = experience, EDX = alvl, EAX =
   dlvl; for dlvl > alvl ≥ 25 it calls `pct(ECX exp, EDX alvl, stack
   dlvl)`, i.e. exp × alvl / dlvl as §4.2 states.
4. Answered: the handler (`0x0054BD29`–`0x0054BD3E`) reads byte +1 as
   the stat and byte +2 as count − 1 (§2); `0x0054BD10`: stat ≤ 15,
   count ≤ 100. `client-messages.tsv` row 0x3A is `stat:u8@1
   repeat:u8@2` (repeat = count − 1) since 997e92a.
5. Answered: monster stats at spawn are `monsters/init.md` §6–§9,
   §13 and §19.
6. Answered: death penalties are §4.6; the stat reset callers are §2.1
   (the `toa` token and Akara's respec, each after the skill reset
   `skills/levels.md` §6.5). `0x005391E0` sends nothing: it stores the
   loss in client +0x508, which the corpse turns into 75 % recoverable
   experience (§4.7); the client learns the new experience through the
   ordinary stat updates.
7. Answered: §5.1 rule 4 (registry `PlayerPos`, off in a standard
   install; `0x00537FD0` and `0x0053E130` specified there; not run by
   `Ruleset::Original`).
8. §5 has no trace check. Settle: R5 of `items/inventory.md` (gold) and
   any recording with damage, potions and running: every 0x18 / 0x95 /
   0x96 / 0x1A–0x1C byte and its tick.
