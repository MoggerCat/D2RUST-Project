# Spec: World — Hirelings (mercenary data, stats, ownership, death, revive, items)

- **Status:** draft: every rule is read from the 1.14d `Game.exe`
  (addresses below) and the formulas are evaluated against the live
  1.14d `hireling.txt` (Test vectors); no hire, level-up, death or
  resurrect recording exists yet.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::world::hirelings`
- **Related specs:** `world/npc.md` §7 (owner of the hire list, its
  draws, the hire / resurrect / quest-grant handlers and S→C 0x4E, 0x4F,
  0x2A), §5 (NPC heal, which also heals pets); `world/quests.md` (the
  Kashya and Qual-Kehk gates and the quest-granted hireling hook:
  link only); `items/inventory.md` §4.2 (requirements), §4.6 (equip from
  the cursor), §7.23 (C→S 0x61 give / take, allowed item types);
  `combat/vitals.md` §4.2–§4.3 (experience level factor, kill
  distribution); `combat/damage.md` (hireling damage rules);
  `monsters/ai.md` and `skills/` (hireling AI and skills: another
  owner); `sim/units.md` (unit flags, modes, update pass);
  `sim/unit-order.md` (broadcast `0x005538D0`); `sim/path-placement.md`
  rule 6 (teleport calls §6 here); `sim/stats.md`, `sim/stat-lists.md`;
  `sim/server-messages.tsv`, `sim/client-messages.tsv`; `data/fields.tsv`
  (`hireling`, `pettype`), `data/runtime-maps.md` §8 (hireling id
  tables), `data/fixups.md` §7 (name ids).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 53–67 |
| Inputs | 68–79 |
| Outputs / state changes | 80–85 |
| Rules | 86–87 |
|   1. `hireling.txt` rows | 88–145 |
|   2. Offer values and price (`0x006637F0(expansion, player, seed, act0, diff0, out)`) | 146–186 |
|   3. Creating the hireling | 187–238 |
|   4. Level stats (`0x00572840(game, player, merc, level)`) | 239–280 |
|   5. Owner link and pet list | 281–320 |
|   6. Following the player | 321–352 |
|   7. Experience and level-up | 353–399 |
|   8. Death (`0x0057CCB0` → `0x005751A0`) | 400–417 |
|   9. Revive | 418–441 |
|   10. Restoring from a save | 442–475 |
|   11. Items (expansion) | 476–513 |
|   12. Services (links) | 514–523 |
|   13. Messages | 524–558 |
| Constants & data dependencies | 559–582 |
| Randomness | 583–593 |
| Edge cases & original bugs | 594–628 |
| Test vectors | 629–661 |
| Provenance | 662–698 |
| Open questions | 699–725 |
<!-- /index -->

## Summary

A hireling is a monster unit (classes 271, 338, 359, 561) owned by a
player through the player's pet list (pet type 7, `hireable`). The hire
list and the hire / resurrect handlers live in `npc.md` §7. This spec
owns what happens around them: the `hireling.txt` row model (§1), the
offer values the client shows and the price (§2), creating and
initialising the hireling unit (§3), its level-dependent stats and
skills (§4), the owner link and pet node (§5), following the player
across levels and acts (§6), experience and level-up (§7), death (§8),
revive (§9), restoring from a save (§10), the item swap (§11) and the
messages (§13). Classic games keep the hireling inside its act;
expansion games keep it across acts, let it die and be resurrected, and
let it wear items.

## Inputs

| Name | Type | Source |
|---|---|---|
| `hireling` rows (record 0x118 bytes) | table | `fields.tsv`, §1 |
| `pettype` row 7 (`hireable`) | table | `fields.tsv`, §6 |
| `experience` `MaxLvl` (class 0) | table | §7 |
| `skills` `reqlevel` (+0x174, u16) | table | §4 |
| hire slot {name id u16, seed u32} | `npc.md` §7.1 | §2, §3 |
| player level (stat 12), gold, quest state | unit stats | §2, §7 |
| C→S 0x36, 0x46, 0x47, 0x61, 0x62 | intents | `client-messages.tsv` |

## Outputs / state changes

Hireling unit (flags, mode, stats, skills, owner, alignment, team),
the player's pet list (node per hireling), unit removal, S→C 0x81, 0x7A,
0x9B, 0x9E–0xA2, 0x27 (hireling speech), item duplicates on give / take.

## Rules

### 1. `hireling.txt` rows

#### 1.1 Row model

1. A row is one level bracket of one hireling subtype. Columns that
   select a row: `Version` (0 classic, 100 expansion; u16 +0x00), `Id`
   (subtype, u32 +0x04), `Class` (monstats id, +0x08), `Act` (1-based,
   +0x0C), `Difficulty` (1-based, +0x10), `Seller` (NPC monstats id,
   +0x14), `Level` (bracket start, +0x1C). Every difficulty has its own
   `Id` (e.g. Act 1 expansion: 0 Fire Normal, 1 Ice Normal, 2 Fire
   Nightmare, 3 Ice Nightmare, 4 Fire Hell, 5 Ice Hell).
2. Rows of one `Id` and version are consecutive and sorted by `Level`
   (the lookup of §1.2 rule 2 relies on it; live data has this order).
3. Name ids: `NameFirst` / `NameLast` keys resolve to string ids u16
   +0x114 / +0x116 at load (`fixups.md` §7). All rows of an act share
   one range.
4. Live 1.14d (`patch_d2` `hireling.txt`, 120 rows): 60 rows version 0
   and 60 version 100, same split per act:

   | Act | Class | Seller | Ids | Rows per version | Name keys | Names |
   |---|---|---|---|---|---|---|
   | 1 | 271 `roguehire` | kashya 150 | 0–5 | 12 | merc01–merc41 | 41 |
   | 2 | 338 `act2hire` | greiz 198 | 6–14 | 18 | merca201–merca221 | 21 |
   | 3 | 359 `act3hire` | asheara 252 | 15–23 | 18 | merca222–merca241 | 20 |
   | 5 | 561 `act5hire2` | qual-kehk 515 | 24–29 | 12 | MercX101–MercX167 | 67 |

   Normal ids have 3 brackets, Nightmare 2, Hell 1. Expansion Normal
   bracket levels: Act 1 3/36/67, Act 2 9/43/75, Act 3 15/49/79, Act 5
   28/58/80; classic: 3/25/49, 9/31/55, 15/37/61, 28/42/75.
5. Unused in 1.14d: class 560 (`act5hire1`, no row; inventory.md §7.23
   still lists it); the version-0 Act 5 rows (no classic Act 5);
   `WType1`/`WType2` (not in the binary record, `fields.tsv`); the
   item types a hireling may use are code constants (§11). The columns
   `Head`/`Torso`/`Weapon`/`Shield` (+0x68–+0x74) have no reader on the
   paths of this spec (open question 4).

#### 1.2 Lookups

1. **Candidates of an act** (`0x00656580(expansion, act0, diff0,
   after)`): rows in table order with act = act0 + 1, difficulty =
   diff0 + 1 and version = 100 if expansion else 0; `after` = 0 starts
   at row 0 and accepts any level; otherwise the scan starts after
   `after` and accepts only rows whose `Level` equals `after`'s. The
   candidate list of an act is the first match followed by every later
   match with the same `Level`: one row per subtype of that difficulty,
   its lowest bracket.
2. **Row of an id at a level** (`0x006562F0(expansion, id, L)`): id >
   255 → none. Start at the first row of (version, id)
   (`runtime-maps.md` §8 table; negative → none) and walk forward: a row
   of this id and version becomes the result, unless a result exists
   already and L < this row's `Level` (then the previous result is
   returned); a row of a higher id ends the walk. So: the last bracket
   with `Level` ≤ L, and the **first** bracket when L is below all of
   them.
3. **Act of a name** (`0x00663750(name)`): `0x00656440` (row whose name
   range holds the name), else `0x00656390`; result `Act − 1`; none → 0.
   (`npc.md` §7.3 step 4 uses it.)

### 2. Offer values and price (`0x006637F0(expansion, player, seed, act0, diff0, out)`)

The hire handler (`npc.md` §7.3 step 5), the hireling init (§3) and the
client (callers `0x004B10F3`, `0x004B5EA7`) run the same routine, so the
offer screen, the price and the hired unit agree on subtype and level.

1. Local seed := `init()`, `init_low(seed)` (`rng.md` §3).
2. Candidates = §1.2 rule 1 for (expansion, act0, diff0); none → return
   0 (no output).
3. row = candidates[roll(local, count)].
4. One step of the local seed: L = (lo' mod 5, unsigned) + player level
   (stat 12, total) − 5; L < 2 → 2.
5. d = L − row `Level` (may be negative). `>> 3` below is an arithmetic
   shift (rounds toward −∞).
6. Output words (u32 each):

   | Word | Value | Clamp |
   |---|---|---|
   | 0 | row `Id` | |
   | 1 | L | |
   | 2 | life = HP + HP/Lvl·d | ≤ 40 → 40 |
   | 3 | strength = Str + (Str/Lvl·d >> 3) | ≤ 10 → 10 |
   | 4 | dexterity = Dex + (Dex/Lvl·d >> 3) | ≤ 10 → 10 |
   | 5 | price = Gold·(100 + 15·d) / 100 (signed, truncated) | < Gold → Gold |
   | 6 | experience = (L + 1)·Exp/Lvl·L·L (32-bit wrap) | ≤ 0 → 0 |
   | 7 | defense = Defense + Def/Lvl·d | ≤ 0 → 0 |
   | 8 | min damage = Dmg-Min + (Dmg/Lvl·d >> 3) | ≤ 0 → 0 |
   | 9 | max damage = Dmg-Max + (Dmg/Lvl·d >> 3) | ≤ 1 → 1 |
   | 10 | `Share` (+0x4C) | ≤ 0 → 0 |
   | 11 | `Resist` (+0x5C; no per-level term) | ≤ 0 → 0 |
   | 12 | `HireDesc` index (u8 +0xD2) | |
   | 13 | 1 | |
   | 14 (u16) | 0 | |

7. Return 1.

The offer always uses the candidate row (lowest bracket of the act and
difficulty) for every value; the unit's real stats use the bracket of
its level (§4), so they differ once L reaches the next bracket (edge
case 2). Attack rating is not part of the offer.

### 3. Creating the hireling

#### 3.1 Unit creation (callers)

1. Hire (`npc.md` §7.3 step 7): `0x005B23C0(class, 1, 4, 0)` near the
   NPC, then the player; class from the NPC record (`0x00575FF0`).
2. Quest grant: `npc.md` §7.5 (spawn modes 4, 6, 12).
3. Restore (`0x005774F0`, §10): `0x00555230` with mode 12 (dead) when
   the saved hireling is dead, else 1.

#### 3.2 Init (`0x00573270(game, player, merc, saved_id, slot, restore)`)

`slot` = {name id u16 @0, seed u32 @4}; `saved_id` = the saved
hireling `Id` (u16), or 0xFFFF for a new hire. Nothing happens unless
player, merc and slot exist and the player is unit type 0.

1. Merc unit flags (+0xC4) |= 0x04020200 (D2MOO names: NOXP, NOTC,
   ISMERC).
2. Alignment: `0x005543B0(merc, 2, 0)`: stat 172 (`alignment`) := 2 in
   the stat list of state 105 (`alignment`), creating that list if
   missing.
3. restore = 0: join the player's team: `0x005B1900(game, merc, 0,
   player +0xD0)` links the merc into the game's target group list of
   the player's group (game +0x10F8 + 4·group; group < 8; only when the
   merc's +0xD0 is 11 = none).
4. **Replace**: if the player has any hireling node (living or dead,
   `0x00574EC0(7, 1)`) whose unit exists: send S→C 0x9B (u16 0xFFFF, u32
   0) to the player's client; clear the old unit's owner
   (`0x0058F030(old, −1, 1)`); remove its pet node (§5 rule 5, with no
   unit kill); queue the old unit's removal from its room
   (`0x0061A270(room, 1, GUID)`) and free it (`0x00555600`). Its items
   are freed with it (no drop code on this path).
5. act0 = act of the slot's name (§1.2 rule 3); offer = §2 with (game
   expansion, player, slot seed, act0, game difficulty +0x6D). No offer
   → stop (the unit stays without owner or pet node).
6. Add the pet node (§5 rule 3) with {seed, name, row `Id`}; this
   broadcasts S→C 0x81 (§13).
7. Merc flags |= 0x80000000 (D2MOO ISREVIVE; the damage rules of
   `damage.md` read it).
8. Owner link `0x0058F030(merc, player GUID, player type 0)` (§5 rule
   1).
9. New hire only (`saved_id` = 0xFFFF): row = §1.2 rule 2 (offer `Id`,
   offer L); none → stop. Experience (stat 13) := offer word 6; level
   stats §4 with level = offer L; send the stats (§13 rule 4, flag 1).
10. AI hook `0x005A4850(game, merc, 0x13, 0)`: appends 19 to the
    monster's AI hook bytes (monster data +0x1C, first free of 9) and
    calls its handler (AI spec).
11. Monster data (+0x14) bytes +4, +5, +9, +10, +11 := 0.

`npc.md` §7.3 step 8 sets the slot's hired word before the init, and
resends the list afterwards.

### 4. Level stats (`0x00572840(game, player, merc, level)`)

Used by the new hire (§3.2 rule 9), level-up (§7.3) and restore (§10).

1. Asserts: game, player non-null, player type 0. merc = the argument,
   else the player's living hireling; none → nothing.
2. level = the argument; 0 → merc level (stat 12, total) + 1.
3. Speech: S→C 0x27 to the player's client (§13 rule 6), string 0xD7C,
   unit = merc (`0x005DE330(game, merc, player, 0xD7C, 1)`).
4. Stat 12 (level) := level.
5. row = §1.2 rule 2 with the pet node's `Id` (`0x00574BD0`) and level;
   none → stop. d = level − row `Level`. `÷8`, `÷4` below are signed
   divisions truncated toward 0 (not the shift of §2).
6. Stat 30 (`nextexp`) := threshold(level + 1) if level < MaxLvl − 1
   (98), else 0; threshold(n) = (n + 1)·n·n·Exp/Lvl (`0x00663790`,
   32-bit).
7. Stat 13 (experience) := threshold(level) if it is lower (unsigned
   compare of the total value), else unchanged.
8. Stats, each set as a base value on the unit (`0x00627260`, layer 0):

   | Stat | Value | Clamp |
   |---|---|---|
   | 0 strength | Str + Str/Lvl·d ÷ 8 | < 10 → 10 |
   | 2 dexterity | Dex + Dex/Lvl·d ÷ 8 | < 10 → 10 |
   | 7 maxhp, 6 hitpoints | (HP/Lvl·d + HP)·256 | < 0x2800 → 0x2800 |
   | 31 armorclass | Defense + Def/Lvl·d | < 0 → 0 |
   | 23 secondary_mindamage | Dmg-Min + Dmg/Lvl·d ÷ 8 | < 0 → 0 |
   | 24 secondary_maxdamage | Dmg-Max + Dmg/Lvl·d ÷ 8 | < 1 → 1 |
   | 19 tohit | AR + AR/Lvl·d | < 0 → 0 |
   | 39, 41, 43, 45 resists | Resist + Resist/Lvl·d ÷ 4 | < 0 → 0 |
   | 74 hpregen | base maxhp / 2000 (signed) | < 0 → 0 |

   Life is set to the new maximum every time (level-up heals fully).
9. Skills: for slot i = 1 … 6 in order: stop at the first slot whose
   skill id is < 1 or ≥ the skill count, or whose `Mode` byte (signed)
   > 15. If the skill record exists and its `reqlevel` (signed u16) ≤
   level: s = (LvlPerLvl·d >> 5) + `Level` (bytes read signed; `>> 5`
   arithmetic); s < 0 → 0; s ≥ 32 → 32; s ≥ 1 → set the merc's skill
   level to s (`0x0056DEB0`); s = 0 → skip. A skill whose `reqlevel` is
   above the level is skipped (later slots still run). How the merc uses
   them (`Chance`, `Mode`, `DefaultChance`) is the AI's.

### 5. Owner link and pet list

1. Owner: `0x0058F030(game, merc, GUID, type, …)` writes the AI control
   block (monster data +0x28 → +0x2C GUID, +0x30 type, +0x28 game); its
   last two flags (both 0 on every path here) would restart AI
   (`0x005DD230`). Owner query `0x0058F0D0` reads it back.
2. Pet list: player data +0x44 → array of `pettype` count (datatbls
   +0xBF0) entries of 12 bytes {u32 head, i32 count, i32 max}; type 7
   is the hireling. Node (24 bytes, `PlayerPets.cpp`):

   | Off | Field |
   |---|---|
   | +0x00 | flags: bit 0 = dead |
   | +0x04 | unit GUID |
   | +0x08 | seed (the slot seed) |
   | +0x0C | name id |
   | +0x10 | `hireling` `Id` |
   | +0x14 | next |

3. Add (`0x00575E90` → `0x00575C70`): max = 0 → recompute it
   (`0x00575900`); still 0 → the new unit is killed (`0x00574450`) and
   nothing is added. count = max → the oldest node is removed with kill
   (rule 5) first (type 7: max = `basemax` 1, and §3.2 rule 4 already
   removed the old hireling). Append the node at the tail; count += 1.
   Then broadcast 0x81 / 0x7A (`0x005538D0` over players,
   `unit-order.md`; §13).
4. Find (`0x00574EC0(game, player, type, any)`): first node of the list
   with any ≠ 0 or bit 0 clear, mapped to its unit by GUID
   (`0x00552F60`; may be null). "Living hireling" = `(7, 0)`, "any
   hireling" = `(7, 1)`. Node by GUID over types 1… (`0x00574BD0`)
   returns +0x08 ({seed, name, id}).
5. Remove (`0x005750E0(game, player, GUID, kill)` → `0x00574850`):
   unlink the node, broadcast 0x7A action 0 (§13), then for the unit:
   kill ≠ 0 → kill as an expired pet (`0x00574450`: flags |= NOXP
   0x4000000, death event or `0x0057CCB0`); kill = 0 → clear flag
   0x80000000. Count −= 1 (negative → fatal assert).
6. Join sync (`0x00574F80`, called from `0x0053CA60`): for every pet
   type 0 … count − 1, every living node with a unit: type 7 → 0x81,
   other types → 0x7A action 1, to the joining client.

### 6. Following the player

1. Pet follow `0x005754B0(game, player, x, y)` (called by the player's
   teleport, `path-placement.md` rule 6, by act change rule 3 below and
   by revive §9): for each pet type 1 … count − 1 by its `pettype` flag
   byte +4: `warp` (bit 0x1, mask `0x006CE268`) → every living node's
   unit is moved to the player (`0x00574D90` → `0x00574CC0`); else
   `range` (bit 0x2, `0x006CE26C`) → living pets farther than 1600
   (`0x006492A0` distance) are removed with kill (§5 rule 5); else →
   all nodes of the type are freed (`0x00574C60`).
2. Hireling row 7 (live): `warp` 1, `range` 0, `partysend` 1,
   `unsummon` 0, `automap` 1, `drawhp` 1, `basemax` 1. So a living
   hireling always warps with the player; a dead one (bit 0) stays.
3. Act change (`0x0053ACC0`, player): classic game → `0x00575BC0`
   first; after the player is placed in the new act → pet follow (rule
   1) to the new position. In an expansion game the hireling therefore
   travels to the new act.
4. `0x00575BC0` (classic act change): for each pet type t = 1 …
   count − 1: `0x00574570(player, list t, keep = keep_dead = (t = 7))`.
   For t ≠ 7 every pet unit is killed (`0x00574450`) and its node freed.
   For t = 7 each node's unit, if it exists: broadcast 0x7A action 0,
   queue its room removal; if the node was living: bit 0 := 1, the
   list head := this node, death event on the unit when it is in a
   room, S→C 0x9B (name id, resurrect cost §9 rule 1) to the owner.
   The node stays, so the classic hireling is lost (no 0x62 in
   classic) and the next hire replaces it (§3.2 rule 4).
5. Warp of one pet (`0x00574CC0`): leave the old act's room list when
   the act differs, place at the target room and position
   (`0x00650BE0`), act byte +0x18 and act pointer +0x1C := the new
   room's, queue for update, flags 2 (+0xC8) |= 0x10000, `0x00573780`,
   path reset (`0x00648C30(path, 0x100)`) when not moving.

### 7. Experience and level-up

#### 7.1 Share on a kill (`0x0057E990(game, attacker, defender)`)

Owner of the generic distribution: `vitals.md` §4.3. The hireling part,
read in 1.14d:

1. Attacker type 0 or 1; defender experience (stat 13, base) > 0;
   player P = the attacker if a player, else its player owner
   (`0x0057E7B0`); none → nothing.
2. If P has a living hireling H: g = gain(defender experience, alvl =
   H level (base), dlvl = defender level) (rule 7.2); if the attacker is
   not H: g = g·86/256 (signed, truncated toward 0); add g to H (§7.3).
3. Then the player's own share (party or solo: `vitals.md` §4.3). The
   hireling's share does not reduce the player's.

#### 7.2 Gain (`0x0057E480`, for the hireling unit)

1. exp > 0x7FFFFF → 0x7FFFFF; exp ≤ 0 → return 1.
2. alvl ≥ MaxLvl (class 0, 99) → 0.
3. Level factor (`vitals.md` §4.2), then `ExpRatio` of alvl
   (`0x0057E390`, shift from the `MaxLvl` row), then + pct(gain, stat
   85 of the hireling, 100) when stat 85 ≠ 0 (`item_addexperience` on
   hireling items counts).
4. Cap (`0x0057E3F0`) for a non-player whose owner is a player with a
   pet node and row (§1.2 rule 2 at alvl): gain := min(gain,
   (threshold(alvl + 1) − threshold(alvl)) >> 6) (unsigned).

#### 7.3 Add (`0x0057E860(game, player, merc_level; gain, merc)`)

1. Nothing unless gain > 0, the pet node and row (at merc_level) exist,
   and merc_level < the player's level (stat 12, total).
2. new = experience (stat 13, base) + **2·gain** (1.14d; D2MOO 1.10f
   adds gain once).
3. merc_level ≥ MaxLvl − 1 (98) → nothing (no stat change).
4. Stat 13 := new; S→C 0xA1 / 0xA2 / 0xA0 for stat 13, old → new, to
   the player's client (§13 rule 5).
5. lvl := merc_level; repeat: next = lvl + 1; threshold(next) > new
   (unsigned) → stop; lvl := next; continue while next < 98. So one
   gain can raise several levels and the loop never passes 98.
6. lvl > merc_level: level stats §4 at lvl; send the stats (§13 rule 4,
   flag 0); event 0x5B on the merc for the player (`0x00553380`); unit
   event 12 (level-up) on the merc (`0x005C0C30(game, 12, merc, 0, 0)`).

The hireling never gains while its level is ≥ the player's, but one
gain may carry it past the player's level.

### 8. Death (`0x0057CCB0` → `0x005751A0`)

1. A monster with mode ≠ 0 (death) and ≠ 12 (dead) that dies, whose
   `0x00457490` test passes, with the death flag argument ≠ 0 and a
   player owner (`0x0058F0D0`, type 0): `0x005751A0(game, owner,
   merc)`.
2. `0x005751A0`: pet type of the unit (`0x00574A20`); type ≠ 7 → remove
   its node (§5 rule 5, no kill). Then in the type's list find the node
   with the unit's GUID: bit 0 := 1 (dead) and send S→C 0x9B (u16 name
   id from the node, u32 resurrect cost §9 rule 1 at the current level)
   to the owner's client. Broadcast 0x7A action 0 with only the GUID
   set (pet type, class and owner 0).
3. The unit stays in the world as a corpse, keeps its inventory and
   equipped items (no `NOTC` drop: unit flag 0x20000), its stats and
   experience. Nothing is dropped.
4. Unit flags get 0x10000 (dead) through the monster death mode
   (`sim/units.md`); the restore path sets it explicitly (§10).

### 9. Revive

1. Resurrect cost (`0x006637B0`): L = level (stat 12, total); cost =
   ((L·L)/2)·15 (signed division), > 50000 (unsigned) → 50000. Used by
   0x9B (§8, §6 rule 4) and by C→S 0x62 (`npc.md` §7.4).
2. C→S 0x62 (`npc.md` §7.4) takes **any** hireling node (`(7, 1)`),
   charges the cost, clears unit flag 0x10000, sets mode 1 (`0x00624690`)
   and life := max, then calls `0x00579AA0(game, player, merc)`, sends
   0x9B (u16 0xFFFF, u32 0) and 0x2A code 5.
3. `0x00579AA0`: if a **living** hireling node exists (`(7, 0)`), it is
   removed as in §3.2 rule 4 (owner cleared, node removed, room removal
   queued, unit freed).
4. Mode 1 again; life := max (stat 6 := `0x00625D10`); mark the node
   living (`0x00574AB0`: bit 0 := 0, broadcast 0x81 for it); join the
   team (`0x005B1900`, §3.2 rule 3).
5. Remove states 1 (`freeze`), 107 (`shatter`), 154 (`recycled`).
6. Flags +0xC4 |= 0x0402000E (NOXP, NOTC, ISVALIDTARGET, CANBEATTACKED,
   TARGETABLE); flags 2 (+0xC8) &= ~0x40000.
7. Re-apply the item stats of every equipped item that is active
   (`0x00577470` → `0x006277F0`, `0x00627910`).
8. Pet follow (§6 rule 1) to the player's position (`0x006488C0` /
   `0x00648900`): the revived hireling appears at the player.
9. Send the stats (§13 rule 4, flag 0).

### 10. Restoring from a save

Save layout is owned by a save spec that does not exist yet (open
question 1). Behavior of the 1.14d loaders (`0x0056AA50` for save
version ≥ 0x5C, `0x00533C70` for older):

1. No hireling when seed, name and experience are all 0.
2. row = §1.2 rule 2 (`Id`, level 1) = the first bracket; none → error
   ("Unable to load merc." in the old loader). Name id := row
   `NameFirst` + saved name index; above `NameLast` → `NameFirst`.
3. `0x005774F0(game, player, name, seed, Id, dead)`: classic game →
   only if the act of the name equals the player's act (client act
   `0x005382B0` when the player has a client, else unit +0x18);
   expansion → always. row = §1.2 rule 2 (Id, 1) (or rule 1 by act and
   difficulty when Id = 0xFFFF); name clamped into the row's range; in the
   hire list of the NPC record found by `0x00535EA0` (its class argument
   was not read) the slot of that name gets hired := 1 **only when it is
   not currently offered**; create the unit (mode 12 if dead, else
   1) and init it (§3.2 with saved_id = Id: no offer stats, no
   experience reset).
4. Pet node seed/name/Id := the saved values (`0x005749B0`).
5. Experience (stat 13) := the saved value if higher; queue stat 13
   (§13 rule 4).
6. Level: start 1; while next = level + 1 ≤ MaxLvl (99) and
   threshold(next) ≤ experience (unsigned, `Exp/Lvl` of the row at the
   current level): level := next. Then §4 at that level. The loader can
   reach 99; §7.3 stops at 98 (edge case 6).
7. Dead: `0x005751A0` (§8 rule 2), flags |= 0x10000, mode 12
   (`0x00553570`), `0x005738D0`. No inventory → create one.
8. The hireling's items follow in the save (writer `0x005699A0`
   section `jf`, expansion only, written when any hireling node
   exists); after they load: refresh, life := max (also when dead),
   send the stats (flag 1).

### 11. Items (expansion)

C→S 0x61 and the allowed item types: `inventory.md` §7.23 (owner). The
swap itself (`0x0054CED0(game, player, merc, item C)`), reached when §7.23
allows C:

1. Classic → 3. No player inventory → 0. The hireling gets an inventory
   if it has none.
2. Locations: C's two body locations (`0x0062EA80`, itemtypes
   `BodyLoc1`/`BodyLoc2`); target = location 1, old = the merc's item
   there. Act 3 hireling (359) with a shield (type 2) → location 2
   instead (left hand).
3. Target empty: **duplicate** C into the merc (`0x0055A2A0`), set the
   copy's mode 4, two item-removal notices for C's GUID
   (`0x00540E60(9, GUID)`), equip the copy at the target from the
   cursor path (`inventory.md` §4.6, skip requirements 1), consume C
   (`0x0055EEA0`), the player's cursor := none. Refresh
   (`0x0055DF00`, `0x0055F4F0(0)`), `0x00540E60(3, 0)`, event 3 at
   frame + 1 (`0x005417D0`). Result 1.
4. Target occupied: unlink old from the merc (must be the item at that
   slot), clear the slot, refresh the merc's stats (`0x0055C730`), then
   requirements of C on the merc (`inventory.md` §4.2, equipping 0)
   with old's bonuses gone:
   - pass: old gets item flags 0x10 (`0x00628170`) and 0x20
     (`0x006280D0`), leaves the merc's inventory, `0x00621000(merc, 1)`;
     then rule 3, and after C's copy is equipped a **duplicate of old**
     goes to the player (`0x0055A2A0(player)`) and becomes the cursor
     item (`0x0055FB10`). Result 1.
   - fail: old goes back to its slot (page 3, body location, mode 1,
     `0x00628280(old, 0xFF)`), merc refresh (`0x0055C460`,
     `0x0055F4F0(0)`). Result 0.
5. Every give or take therefore creates new item units (new GUIDs);
   take (`inventory.md` §7.23) duplicates as well.
6. Potions given to the hireling are used on it (`inventory.md`
   §7.23, item-use spec); C→S 0x26 `on_merc` (belt) likewise.
7. Death keeps the items (§8 rule 3); a replaced hireling's items are
   freed with it (§3.2 rule 4).

### 12. Services (links)

| Service | Owner | Hireling-side rule here |
|---|---|---|
| hire (C→S 0x36) | `npc.md` §7.3 | offer §2, init §3, replace §3.2 rule 4 |
| resurrect (C→S 0x62) | `npc.md` §7.4 | cost §9 rule 1, revive §9 |
| heal on chat open | `npc.md` §5 step 5 | life to max, curable states; mana not touched |
| quest-granted hireling | `npc.md` §7.5, `quests.md` | init §3 |
| command (C→S 0x46, 0x47) | AI spec | — |

### 13. Messages

1. **S→C 0x81 AssignMerc** (20 bytes, `0x0053CB80`, buffer zeroed
   first): u8 pet type @1 (7), u16 monstats class @2, u32 owner GUID
   @4, u32 hireling GUID @8, u32 seed @12, u32 name id @16. Sent per
   player client by the pet-add broadcast (`0x00574930`, when the
   broadcast record has a seed or a name; otherwise 0x7A action 1), on
   join (§5 rule 6) and on revive (§9 rule 4).
2. **S→C 0x7A PetAction** (13 bytes, `0x0053CB30`, zeroed first): u8
   action @1 (1 add, 0 remove), u8 pet type @2, u16 class @3, u32 owner
   GUID @5, u32 pet GUID @9 (owner of the layout: `sim/pets.md` §8;
   corrected 2026-10-07, evidence there). Removal records built by §5 rule 5, §6
   rule 4 and §8 rule 2 carry only the GUID (other fields 0).
3. **S→C 0x9B** (7 bytes, `0x0053E0E0`): u16 @1, u32 @3. Death /
   classic act change: name id and resurrect cost. Replace, resurrect:
   0xFFFF and 0.
4. **Hireling stats** (`0x005726C0(game, player, flag)`): for the
   living hireling, queue on the merc unit (`0x005718C0`, list +0xEC /
   +0xF0, unit queued for update), in this order: 12 level (total), 0
   (base), 2 (base), 7 (base), 6 (total), 31 (base), 13 (total), 30
   (total), 23 + 21 (base sum), 24 + 22 (base sum), 39, 41, 43, 45
   (base). The per-unit flush `0x00571CD0` sends each as
   `0x0053BEE0`: value < 0xFF → 0x9E (u8 stat @1, u32 GUID @2, u8 @6),
   < 0xFFFF → 0x9F (u16 @6), else 0xA0 (u32 @6). Stat id > 0xFE → fatal
   assert. No reader of `flag` was found in the body.
5. **Experience delta** (`0x0053BFD0(client, merc, stat, old, new)`):
   δ = new − old (unsigned); δ < 0xFF → 0xA1 (u8 δ @6, 7 bytes); δ <
   0xFFFF → 0xA2 (u16 δ @6, 8 bytes); else 0xA0 with the **old** value
   (edge case 7). u8 stat @1, u32 GUID @2.
6. **Speech** (S→C 0x27, 40 bytes, `0x005DE330` with flag 1): byte 0
   0x27, byte 1 = 1, u32 merc GUID @2, byte 6 = 1, byte 8 = 3, u16
   string id @10 (0xD7C); the other bytes of the stack buffer are not
   written (as in `npc.md` §9 bytes 3–6). Sent on every §4 call.
7. Hire list 0x4F / 0x4E and 0x2A results: `npc.md` §7.2, §9.

## Constants & data dependencies

- `hireling` offsets: version +0x00 (u16), id +0x04, class +0x08, act
  +0x0C, difficulty +0x10, seller +0x14, gold +0x18, level +0x1C,
  exp/lvl +0x20, hp +0x24, hp/lvl +0x28, defense +0x2C, def/lvl +0x30,
  str +0x34, str/lvl +0x38, dex +0x3C, dex/lvl +0x40, ar +0x44, ar/lvl
  +0x48, share +0x4C, dmg-min +0x50, dmg-max +0x54, dmg/lvl +0x58,
  resist +0x5C, resist/lvl +0x60, skill1–6 +0x78, mode1–6 +0xC0 (i8),
  level1–6 +0xC6 (i8), lvlperlvl1–6 +0xCC (i8), hiredesc +0xD2, name
  ids +0x114/+0x116 (`fields.tsv`).
- `pettype` row 7 flags byte +4: warp 0x1, range 0x2 (masks
  `0x006CE268`, `0x006CE26C`); `basemax` u16 +10.
- MaxLvl class 0 (`0x00611830(0)`): 99. Level caps: 98 by experience,
  99 by restore.
- Clamps: life 40 points (offer) / 0x2800 (unit, 1/256 units); str,
  dex 10; skill level 1–32; hpregen = maxhp/2000; resurrect cost 50000;
  gain 0x7FFFFF and the 1/64-level cap.
- Hireling classes 271, 338, 359, 561 (560 has no row); act 3 shield
  rule class 359.
- Stats: 0, 2, 6, 7, 12, 13, 19, 23, 24, 30, 31, 39, 41, 43, 45, 74,
  85, 172; states 1, 105, 107, 154.
- Unit flags: +0xC4 |= 0x04020200 (init), 0x80000000 (owned), 0x10000
  (dead), 0x0402000E (revive); +0xC8 bit 0x40000 cleared on revive.

## Randomness

| When | Seed | Draws |
|---|---|---|
| offer / hire / init (§2) | local {slot seed, 666} | roll(candidates), one step (level) — the same values every time the slot is evaluated |
| hire list | NPC-control | `npc.md` §7.1 |
| unit creation | monster spawn spec | `0x005B23C0` / `0x00555230` |

Level stats, experience, death, revive, follow and the item swap draw
nothing here (item duplication: `items/generation.md`).

## Edge cases & original bugs

Reproduced by default.

1. The offer's level depends on the player's level at the moment it is
   evaluated: the client, the price check and the init each re-run §2,
   so a level-up between list and hire changes L and the price.
2. Offer stats (§2) use the lowest bracket row; the unit's stats (§4)
   use the bracket of its level, so the hire screen under-reports
   higher-level hirelings (e.g. Act 1 Ice, L 40: offer life 378, unit
   414) and never shows attack rating or per-level resists.
3. Rounding differs: offer `>> 3` rounds toward −∞, unit `÷ 8` toward 0.
   With L below the bracket (L = 2, bracket 3) the offer shows strength
   33 and the unit gets 34.
4. Skills with level 0 after the formula are not added (Act 1, L 2:
   Inner Sight gets (10·−1 >> 5) + 1 = 0 → none).
5. C→S 0x62 accepts a living hireling: §9 rule 2 charges the cost and
   §9 rule 3 then finds the same unit as "living" and frees it before
   the rest of §9 runs on it. Not traced (open question 6); d2rs refuses
   a living hireling with code 9 until it is.
6. Experience gains stop at level 98, but a save reload recomputes the
   level from experience up to 99.
7. 0xA0 for a large experience delta carries the old experience, not
   the new one; the client is one step behind until the next stat
   message.
8. Restore marks the seller's slot hired only when it is not on the
   current offer, so a restored hireling's name can still be offered
   and hired again.
9. Hireling experience is 2·gain (1.14d), and the non-killer share
   86/256 is applied before doubling.
10. Life is set to max after a restore even for a dead hireling (§10
    rule 8).
11. Classic: changing acts turns the hireling into a dead node with no
    way to revive it; only a new hire clears it.

## Test vectors

Formulas of §2 / §4 evaluated on the live 1.14d `patch_d2`
`hireling.txt` (expansion rows) and `skills.txt` `reqlevel`; game-file
tests (`#[ignore]`, `D2_GAME_DIR`). The offer columns use the lowest
bracket row of the Id; the unit columns the row of §1.2 rule 2.

| Id, L | Offer: life, str, dex, price, exp, def, min–max dmg, share, resist | Unit: str, dex, maxhp (1/256), def, dmg (23–24), tohit, resists, hpregen, nextexp, skills |
|---|---|---|
| 1 (Ice N), 6 | 72, 38, 51, 217, 26460, 39, 1–3, 0, 0 | row 3: 38, 51, 18432, 39, 1–3, 46, 6, 9, 41160, Inner Sight 1, Cold Arrow 1 |
| 0 (Fire N), 2 | 40, 33, 43, 100, 1200, 7, 0–2, 0, 0 | row 3: 34, 43, 10240, 7, 1–3, 0, 0, 5, 3600, none |
| 6 (Comb N), 20 | 285, 76, 56, 927, 924000, 166, 12–19, 1, 18 | row 9: 76, 56, 72960, 166, 12–19, 152, 40, 36, 1067220, Jab 6, Prayer 5 |
| 15 (Fire N), 30 | 295, 67, 55, 3250, 3069000, 155, 8–14, 0, 25 | row 15: 67, 55, 75520, 155, 8–14, 195, 51, 37, 3382720, Inferno 10, Fire Ball 8 |
| 24 (1hs N), 40 | 504, 123, 78, 25200, 7872000, 300, 25–29, 1, 56 | row 28: 123, 78, 129024, 300, 25–29, 390, 77, 64, 8472240, Bash 7, Stun 6 |
| 1 (Ice N), 40 | 378, 81, 119, 982, 6888000, 311, 10–12, 0, 0 | row 36: 82, 119, 105984, 339, 11–13, 502, 73, 52, 7413210, Inner Sight 13, Cold Arrow 13 |
| 1 (Ice N), 70 | 648, 118, 179, 1657, 36529500, 551, 17–19, 0, 0 | row 67: 119, 179, 253440, 810, 27–29, 1258, 124, 126, 38109960, Inner Sight 22, Cold Arrow 22 |
| 9 (Comb NM), 50 | 742, 125, 97, 16195, 15300000, 510, 27–34, 3, 83 | row 43: 125, 97, 189952, 510, 27–34, 553, 95, 94, 16230240, Jab 15, Thorns 7 |

Synthetic (CI-safe):

| Input | Expected | Source |
|---|---|---|
| §1.2 rule 2, brackets 3/36/67, L = 2 / 3 / 35 / 36 / 99 | 3 / 3 / 3 / 36 / 67 | §1.2 |
| §2 candidates Act 1 Normal expansion | rows of Id 0 and Id 1 at level 3 (count 2); Act 2 Normal: Ids 6, 7, 8 (count 3) | live data |
| slot seed 22752887, 2 candidates, player level 10 | candidate 1 (Id 1), L = 6 (`npc.md` vector) → price 217 | §2 |
| cap: Exp/Lvl 105, alvl 6 | (41160 − 26460) >> 6 = 229 | §7.2 rule 4 |
| gain 229, merc killed it, exp 26460 | new 26918; S→C `a2 0d <guid> ca 01` | §7.3, §13 rule 5 |
| gain 229, player killed it | g = 76 (229·86/256); new 26612; `a1 0d <guid> 98` | §7.1, §7.3 |
| old 0, new 70000 (δ ≥ 0xFFFF) | `a0 0d <guid> 00 00 00 00` | edge case 7 |
| death, name id 0x0F21, level 30 | 0x9B `9b 21 0f 5e 1a 00 00` (6750) | §8, §9 rule 1 |
| stat value 300 / 70000 / 5 | 0x9F / 0xA0 / 0x9E | §13 rule 4 |
| merc level 97, exp jumps above threshold(99) | level 98 (§7.3); after reload 99 (§10) | edge case 6 |

## Provenance

- 1.14d `Game.exe` (decompile in `re/exports/funcs`, register use from
  `tools/ghidra/disasm.py`): offer `0x006637F0` (shift types and clamps
  read from the instructions at `0x006638A9`–`0x006639B0`); lookups
  `0x00656580`, `0x006562F0`, `0x00663750`; threshold `0x00663790`;
  cost `0x006637B0`; init `0x00573270` (argument roles from the
  disassembly); alignment `0x005543B0`; team `0x005B1900`; owner
  `0x0058F030`; level stats `0x00572840`; stats send `0x005726C0`,
  `0x005718C0`, `0x00571CD0`, `0x0053BEE0`; pets `0x00574EC0`,
  `0x00575E90`, `0x00575C70`, `0x005750E0`, `0x00574850`, `0x00574450`,
  `0x00574BD0`, `0x00574AB0`, `0x005749B0`, `0x00574F80`; follow
  `0x005754B0`, `0x00574D90`, `0x00574CC0`, `0x00575380`, `0x00574C60`;
  act change `0x0053ACC0`, `0x00575BC0`, `0x00574570`; experience
  `0x0057E990` (86/256 at `0x0057EA2D`), `0x0057E480`, `0x0057E390`,
  `0x0057E3F0`, `0x0057E860` (`lea ecx,[esi+ebx*2]` at `0x0057E8CE`);
  death `0x0057CCB0`, `0x005751A0`; resurrect `0x00579C00`, revive
  `0x00579AA0`, `0x00577470`; restore `0x0056AA50`, `0x00533C70`,
  `0x005774F0`, writer `0x005699A0`; item swap `0x0054CED0`; messages
  `0x0053CB80`, `0x0053CB30`, `0x0053E0E0`, `0x0053BFD0`, `0x005DE330`
  (byte stores read from the disassembly).
- §13 r1–r2 GUID order (2026-10-07, spec-client-msgs-3): both senders
  copy broadcast record +0x04 to the first GUID field (0x81 @4, 0x7A
  @5) and record +0x00 to the second (0x81 @8, 0x7A @9); the add
  `0x00575D90` builds the record with +0x00 = pet GUID (pet +0x0C) and
  +0x04 = owner GUID (player +0x0C) (`0x00575E4F`–`0x00575E67`), so 0x7A
  carries owner @5, pet @9 like 0x81 (owner @4, hireling @8).
- Live 1.14d data: `patch_d2` `hireling.txt` (120 rows), `pettype.txt`
  row 7, `skills.txt` `reqlevel`, `states.txt` 1/105/107/154,
  `itemstatcost.txt` stat names; test-vector table computed from them.
- D2MOO 1.10f `PlayerPets.cpp`, `SUnitDmg.cpp`, `Units.h` (flag names)
  used as a map; every rule above was re-read in 1.14d. Differences:
  D2MOO's `SUNITDMG_AddExperienceForHireling` adds the gain once
  (1.14d twice, §7.3 rule 2); the 86/256 share and the 1/64-level cap
  match D2MOO. This confirms the hireling part of `vitals.md` open
  question 2.

## Open questions

1. Save layout of the hireling fields and the `jf` item section: no
   save spec yet. Evidence for it: the ≥ 0x5C loader copies 32 bytes
   from header offset 0xAF (u32 flags with dead bit 0x10000, u32 seed
   @0xB3, u16 name index @0xB7, u16 `Id` @0xB9, u32 experience @0xBB);
   settle in the save spec with a saved character.
2. Where a dead expansion hireling's unit lives after the player leaves
   its level (it does not warp, §6 rule 2) and how it survives room
   freeing; settle with a recording (die, change level, resurrect).
3. `0x005394A0` (player placement) restores a hireling from inventory
   corpse-list nodes (`0x0063D570` …, `0x005774F0` with Id 0xFFFF): when
   such nodes exist; read the corpse-list writers.
4. Readers of `hireling` `Head`/`Torso`/`Weapon`/`Shield` (+0x68–+0x74)
   and `DefaultChance` +0x64: likely AI or appearance (grep the image).
5. String 0xD7C (3452) of the level speech: key and text (decode
   `string.tbl`).
6. Crafted 0x62 with a living hireling (edge case 5): debugger trace of
   `0x00579AA0` on that path.
7. Which clients `0x00571CD0` serves (only the owner, or every client
   that receives the merc's updates); settle with a two-player recording.
8. The `0x00457490` test in the death path (§8 rule 1) and the meaning
   of the death flag argument; read the 14 callers of `0x0057CCB0`.
9. No recording: hire, level-up, death, resurrect, give / take item;
   record one of each (`packets-0002`, `docs/HANDOFF.md` §5) to confirm
   message order and bytes.
