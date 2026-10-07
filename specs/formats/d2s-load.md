# Spec: Formats — Character save load effects (.d2s, version 0x60)

- **Status:** draft: read from the 1.14d `Game.exe` loader
  (`0x0056B180` and the functions below) and checked on the C66 local
  run (a stub and two generated characters loaded and re-saved by the
  game). Split out of `formats/d2s.md` §9 (rules 6–7) for size; that
  spec owns the bytes, this one what a load does with them.
- **Target version:** 1.14d
- **Crate/module:** `d2-server` character storage (load effects);
  `d2-formats::d2s` parses the bytes (`formats/d2s.md`).
- **Related specs:** `formats/d2s.md` (layout, checks, §9 rules 1–5);
  `world/quests.md` §1.6, §3; `world/waypoints.md` §3;
  `world/hirelings.md` §10; `combat/vitals.md`;
  `items/generation.md` §9, §10.3; `items/inventory.md`;
  `sim/intents-events.md` §7.2.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 39–46 |
| Inputs | 47–53 |
| Outputs / state changes | 54–59 |
| Rules | 60–64 |
|   1. New-character start (`0x00569F80`) | 65–90 |
|   2. Load effects (`0x0056B180`) | 91–117 |
|   3. Join after the load: Iron Golem re-summon (`0x005394A0`) | 118–143 |
|   4. Hotkey and mouse-skill item indices after a reload | 144–161 |
|   5. Load failure: result codes and the message shown | 162–199 |
|   6. Runeword items that no longer match (`0x00563470`) | 200–225 |
|   7. Map seed restore in single player | 226–237 |
| Constants & data dependencies | 238–242 |
| Randomness | 243–247 |
| Edge cases & original bugs | 248–256 |
| Test vectors | 257–262 |
| Provenance | 263–297 |
| Open questions | 298–311 |
<!-- /index -->

## Summary

A `.d2s` load either starts a new character (the 335-byte stub) or
restores every section of a full save into the player unit and its
client. This spec lists both paths in the order the 1.14d loader runs
them and names the spec that owns each effect, so the server can apply
them after the format layer has parsed the file.

## Inputs

| Name | Type | Source |
|---|---|---|
| parsed save | sections of `formats/d2s.md` §1 | `d2-formats::d2s` |
| game, client record | — | session |

## Outputs / state changes

A player unit with stats, skills, items, quest / waypoint / NPC
records, possibly a corpse, a hireling and an Iron Golem item; or a
new level-1 character.

## Rules

A bare § number in this spec refers to `formats/d2s.md`; this spec's
own sections are written "load §1", "load §2".

### 1. New-character start (`0x00569F80`)

1. Reached from `formats/d2s.md` §9 rule 1 (the stub). In order:
   client +0x3D4 |= 1 (`0x00537970`); the player unit is allocated (`0x00555230`,
   type 0) and set up (`0x00569F20`: mode 1, player data +0x50..+0x5C
   := 0); its add messages (`0x00571F90`, `sim/intents-events.md`
   §7.2); start stats for the client act (`0x005706D0`,
   `combat/vitals.md` `init_player_stats`); start items
   (`0x00534F10`, `items/generation.md` §10.3); player data +0x70 and
   +0x74 (right and left skill) := 0; if the client act is 0 and the
   class's `charstats` `StartSkill` (record +0xAC) ≠ 0 and the unit
   has that skill (`0x006439F0`), it becomes the right skill
   (`0x005701B0`, item −1) and +0x70 := it; then the quest entry with
   mode 1 (`0x00546270`, `world/quests.md` §3). None of §4–§8 is read.
   The first save after this writes a complete file (§1), so a stub
   grows to a full save. Measured: a 335-byte classic Necromancer stub
   (status 0x0001) loaded and saved by the game became 953 bytes:
   status 0 (bit 0 cleared, §2.2 rule 7), +0x2C = 0 (§2.2 rule 10),
   +0x34 = 0xFFFFFFFF, town bytes `80 00 00`, a map seed, quest,
   waypoint and NPC sections, level-1 Necromancer stats, 30 skill bytes,
   7 start items (four `hp1 `, `tsc `, `isc `, `wnd `) in classic item
   format 2, the empty corpse header and no `jf` / `kf` (classic game,
   §1 rule 2). The fresh Sorceress and Necromancer saves' right skills
   36 and 70 are the 1.14d `patch_d2` `StartSkill` values (Fire Bolt,
   Raise Skeleton; §2.4 rule 7).

### 2. Load effects (`0x0056B180`)

1. Load effects of a full save, in the master's order, and the spec
   that owns each
   (what `d2-server` character storage must apply after `d2-formats`
   has parsed the bytes):

   | Step | Effect | Code | Owner |
   |---|---|---|---|
   | header | client weapon switch, status, create time, act; map seed (§2.2 rules 7–8); hotkeys and mouse skills decoded (§2.4 rules 4–5); client +0x480..+0x482 | `0x0056A090` | §2.2, §2.4 |
   | quests | each 96-byte record copied in with normalisation | `0x0056A370` → `0x0065C4D0` | `world/quests.md` §1.6 |
   | waypoints | records copied in | `0x0056A3E0` | `world/waypoints.md` §3 |
   | NPC | fields A and B copied in | `0x0056A470` | §6; `world/quests.md` §6.7, §10.3 |
   | stats | base stats set; hitpoints and mana remembered | `0x0056A4F0` / `0x0056A620` | §7.1; `sim/stats.md` |
   | skills | base skill levels added | `0x0056A710` → `0x0056DEB0` | §7.2; `skills/levels.md` |
   | player items | each entry created (§8.2 rule 7: 0x80000 set, 0x2000 cleared, replenish timers), placed (`0x00531210`), its children inserted; runeword refresh | `0x0056A7E0` | §8.2; `items/inventory.md`, `items/generation.md` §9 |
   | corpse | a corpse unit of the player's class with its item list (placed with `0x00531520`), linked to the player | `0x0056A830` | §8.3 |
   | hireling | hireling restored from the header block | `0x0056AA50` | `world/hirelings.md` §10 |
   | hireling items | the hireling's item list, then `world/hirelings.md` §10 rule 8 | `0x0056AC10` | §8.4 |
   | golem | Iron Golem item (skill 90 check), mode 3, handed to the client | `0x0056AE50` | §8.5; Open question 15 |
   | post-load | gold limits, stamina, item indices → GUIDs, mouse skills selected, hitpoints and mana restored, stats 67–69 and 30 | `0x0056AF80`, `0x0056AF20`, `0x005701B0` | §9 rule 4, §2.4 rule 6 |

   The player's game entry after a successful load (the quest entry
   with mode 0 and the messages) is the caller's (`0x005344B0` /
   `0x00534520` → `0x00546270`; `world/quests.md` §3,
   `sim/intents-events.md`).

### 3. Join after the load: Iron Golem re-summon (`0x005394A0`)

1. The `kf` reader stores the golem item's GUID (unit +0x0C; −1 when
   there is no item unit) in the client at +0x484 (`0x0056AF03` →
   setter `0x00538700`, its only caller). The join handler `0x00530190`
   calls `0x00539760` (the load, `0x005345A0` → `0x005344B0` /
   `0x00534520`) and, when it succeeds, `0x005394A0`, which at
   `0x005396C8`–`0x0053974C`:
   1. client +0x484 is 0 or −1 → nothing.
   2. Else the item unit with that GUID is looked up in the game
      (`0x00552F60`, unit type 4); found and the player has a skill 90
      entry (`0x006439F0`): L := the skill's level with bonuses
      (`0x006442A0`(player, skill, 1), base + bonus, at least 0, capped
      by `0x00611830(0)`); the item's static position := the player's
      x, y (`0x00620470`, `0x006204C0`); the item becomes the player's
      target (`0x00620C10` → path target `0x00648B90`); then the skill
      do core `0x0056F7F0`(game, player, skill 90, L, 1, 0, 0) runs
      (`skills/use.md` §5.4), so Iron Golem is cast at level L on that
      item.
   3. In every case after 2 starts, client +0x484 := −1. A loaded item
      that is not found or whose player lacks the skill entry is left
      in mode 3 (no other code frees it here).
2. Only `0x00539760` sets +0x484 (through the `kf` reader); there is no
   other reader than `0x005394A0`. So the Iron Golem item of a save is
   only ever used to re-cast the golem when the character joins.

### 4. Hotkey and mouse-skill item indices after a reload

1. The writer's item index is the 1-based position in the inventory
   item list in link order (§2.4 rule 2). The loader places the
   top-level entries in file order (§8.2 rule 3), and linking appends
   (`items/inventory.md` §1.4 rule 1), so the reloaded list is in file
   order: every item of the list except the two hands in their old
   order, then the hands in the §8.1 rule 3 order (the cursor item is
   not in the list either way; `items/inventory.md` §1.4 rule 3).
2. So an index written for an item that came after a hand item in the
   pre-save list, or for a hand item that was not already last, resolves
   (§2.4 rule 6, after the items are linked) to whatever item sits at
   that position in file order, possibly another item. After one reload
   the list equals file order, so the next save writes the same order
   and the indices stay stable until an item is unlinked and relinked
   (moved, dropped, picked up) or the weapon in use changes the hand
   order.

### 5. Load failure: result codes and the message shown

1. A failed load (`0x00539760` ≠ 0) makes the join handler send S→C
   0xB4 (`0x0053B260`: byte 0xB4, u32 result; `sim/server-messages.tsv`
   `ConnectionRefused`), then calls `0x00539DA0`. The client
   handler (`0x0045C6D0`, jump table `0x0045C7E8` for results 1–26,
   any other value → 9) calls `0x0044E380`(m): m ≥ 0x1D → 9; m is
   stored in `0x007A05D4` and the front end later shows string
   `0x0070F384`[m] (u16 string ids; `0x0044CB60` → `0x00524A30`), except
   that with config byte +0x1EF bit 0x20 set (`[0x007A0438]`; meaning
   not traced) m = 20 shows 0x5522 and m = 21 shows 0x5521.
2. Result → m → string id (`formats/d2s.md` §10 rule 1 gives the
   causes):

   | Result | m | String id | | Result | m | String id |
   |---|---|---|---|---|---|---|
   | 1 | 0 | 5365 | | 14 | 17 | 5380 |
   | 2 | 1 | 5366 | | 15 | 18 | 5381 |
   | 3 | 2 | 5367 | | 16 | 19 | 5360 |
   | 4 | 3 | 5368 | | 17 | 20 | 5364 (0x5522 with the bit) |
   | 5 | 4 | 5369 | | 18 | 21 | 5363 (0x5521 with the bit) |
   | 6 | 5 | 5371 | | 19 | 22 | 5362 |
   | 7 | 10 | 5373 | | 20 | 23 | 5361 |
   | 8 | 11 | 5374 | | 21 | 24 | 5359 |
   | 9 | 12 | 5375 | | 22 | 9 | 5372 |
   | 10 | 13 | 5376 | | 23 | 25 | 10101 |
   | 11 | 14 | 5377 | | 24 | 26 | 10102 |
   | 12 | 15 | 5378 | | 25 | 27 | 5370 |
   | 13 | 16 | 5379 | | 26 | 28 | 5371 |

   The texts are the user's string tables' entries for these ids
   (`formats/tbl.md`); d2rs shows them by id and never stores them.
3. The join itself also refuses before or after the load
   (`0x00539760`): 0x18 when the game is classic and the client class
   is above 4, and after the load 0x17 / 0x18 (expansion status vs
   game), 0x13 / 0x14 / 0x15 (hardcore checks) on the client status
   word, the same values as the loader's own checks (§2.2 rule 5).

### 6. Runeword items that no longer match (`0x00563470`)

1. Called for a top-level player-list item with flag 0x4000000 whose
   runeword check `0x0062BED0` gives 0 (`0x00533782`–`0x005337A9`),
   after its children are inserted, with (game, player, item); by the
   item's mode:
   1. 0 (stored): item data +0x47 := its page (+0x45); when the
      player has a client (`0x005531C0`), an item message with flag
      0x20 (`0x0053D010`); then `0x0055DF10`(game, player, item, 0):
      the item is unlinked from the inventory (`0x0063AD90`), its page
      byte := 0xFF and the unit is freed (`0x00557FD0`). The item is
      gone.
   2. 1 (equipped): `0x00560CD0`(game, player, body location
      `0x0063C150`, 1): when the cursor is empty and the body slot
      check `0x0063DE60` gives 3 or 4, the item is taken off its body
      location (`0x0063E490`, `0x0062A360`, `0x0063D2B0`, unlinked),
      gets flag 0x20 (because the 4th argument is 1, instead of
      becoming the cursor item), mode 4 (`0x00624690`),
      `0x00628170`(item, 0x10, 1), its
      update entry (`0x0063CC70`) and the player's stats are refreshed
      (`0x00621000`). Where the item ends up after that is not traced
      (`formats/d2s.md` Open question 13).
   3. 4 (cursor): `0x0055EEA0`: when it is the cursor item, a message,
      the cursor is cleared (`0x0063C180`) and the unit is freed.
   4. Any other mode: nothing.

### 7. Map seed restore in single player

1. Single player has client game type `0x007A0610` = 0, so the create
   message carries game type 3 (`0x00477CDF`; `tools/original-hooks.md`
   §5.2) and the game record gets +0x6A = 3 (`0x0053F17A` →
   `0x00530CFF`); game +0x84 is 0 unless the `-seed` switch set a fixed
   seed. So the §2.2 rule 8 restore happens on every single-player load
   of a save whose town byte for the game's difficulty has 0x80: every
   game-written save loaded in the difficulty it was saved in (the
   writer sets 0x80 only in that byte, `0x00569231`). Loaded in another
   difficulty, that byte is 0: act 0 and the game's own new map seed.

## Constants & data dependencies

`charstats` `StartSkill` (record +0xAC, `data/fields.tsv`); the rest is
in the owner specs named in load §2.

## Randomness

None in the load itself; the new character's start items and stats are
the owner specs' (`items/generation.md` §10.3).

## Edge cases & original bugs

1. The right skill is set from `StartSkill` only when the client act is
   0 (load §1 rule 1); a stub for a later act gets none.
2. A stored runeword item that no longer matches a runeword is deleted
   on load (load §6 rule 1.1).
3. A hotkey's item index can point at another item after a reload
   (load §4).

## Test vectors

| Input | Expected output | Source |
|---|---|---|
| a classic Necromancer stub (status 0x0001), loaded and saved | full save: status 0, +0x2C = 0, town `80 00 00`, seven start items, no `jf` / `kf`; 953 bytes | load §1 rule 1 (C66 run) |

## Provenance

- Second pass (DS questions of `docs/handoff/impl-d2s.md`, local run
  C66 of `local-buddy-q-saves.md`), 1.14d `Game.exe` with
  `tools/ghidra/disasm.py`: item create from a record `0x00558CB0`
  (flag calls `0x00558D37`–`0x00558D4C`), flag setter `0x006280D0`,
  record decode `0x0062E430`, peek `0x0062AE20`, full reader
  `0x0062CBE0`, compact reader `0x0062A970`, trailer getter / setter
  `0x00629E40` / `0x00629EA0`, writer header `0x006312B0`, filled count
  `0x0062A900`, the save call sites of `0x006313E0` (§8.1 rule 9),
  appearance fill `0x00568F20` → `0x0063E510`, `kf` reader
  `0x0056AE50`, file read `0x005343A0`, new character `0x00569F80`
  (`0x00537970`, `0x00569F20`, `0x005706D0`, `0x00534F10`,
  `0x005701B0`, `0x00546270`); `charstats` +0xAC = `StartSkill`
  (`data/fields.tsv`). Real saves (C66 run, 2026-10-07, read-only):
  13 saves of this PC round-trip byte for byte through d2rs; the
  generated `TestAma` (classic, 843 bytes), `TestSor` (expansion level
  30, `hp1 ` and `lsd `) and the stub `TestStub` loaded and were
  re-saved by the game; the re-saves differ from the generated files
  only in the checksum, +0x30, +0x88..+0x97 (§2.8) and item flag
  0x2000 (§8.2 rule 7); `TestStub` grew 335 → 953 bytes (load §1 rule 1).
- Third pass (formats/d2s.md Open questions 5, 9, 13, 15, 16), 1.14d
  `Game.exe` with `tools/ghidra/disasm.py`: golem GUID setter
  `0x00538700` (sole caller `0x0056AF03`), its uses `0x005396C8` /
  `0x0053974C` in `0x005394A0` (`0x00552F60`, `0x006439F0`,
  `0x006442A0`, `0x00620470`, `0x006204C0`, `0x00620C10`, `0x0056F7F0`);
  join `0x00530190` → `0x00539760` → `0x005345A0`; refusal sender
  `0x0053B260`, client handler `0x0045C6D0` with jump table
  `0x0045C7E8`, `0x0044E380`, `0x0044CB60`, string id table `0x0070F384`
  read from the image; runeword refresh `0x00563470` (call
  `0x005337A9`), `0x0055DF10`, `0x00560CD0`, `0x0055EEA0`; create
  message `0x00477CA0` (game type byte at `0x00477CDF`), handler
  `0x0053F100` → `0x00530BF0` (+0x6A at `0x00530CFF`); town byte
  `0x00569231`.

## Open questions

1. None of its own; see `formats/d2s.md` Open questions 15 (golem item)
   and 17 (appearance bytes).
2. ~~Load §6 rule 1.2: where an equipped non-matching runeword item
   ends up after `0x00560CD0` (flag 0x20 set, not the cursor).~~ Struck
   (2026-10-07): needs `0x0055C730`, `0x0055C5C0` and `0x0055DBC0`
   (three bodies). A game-written save holds a stale runeword only
   after a data change (modded runes), so Phases 0–6 do not reach it.
   Recording list `docs/handoff/pc2-rec-pc2-items.md` IT-8.
3. ~~Load §4 measured confirmation.~~ Struck (2026-10-07): the rule is
   derived from the binary; the confirming save is recording list
   IT-3.
