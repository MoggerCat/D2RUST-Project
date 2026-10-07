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
   | golem | Iron Golem item (skill 90 check), mode 3, handed to the client | `0x0056AE50` | §8.5 rules 2 and 6 (re-cast at game entry, `0x005394A0`) |
   | post-load | gold limits, stamina, item indices → GUIDs, mouse skills selected, hitpoints and mana restored, stats 67–69 and 30 | `0x0056AF80`, `0x0056AF20`, `0x005701B0` | §9 rule 4, §2.4 rule 6 |

   The player's game entry after a successful load (the quest entry
   with mode 0 and the messages) is the caller's (`0x005344B0` /
   `0x00534520` → `0x00546270`; `world/quests.md` §3,
   `sim/intents-events.md`).

## Constants & data dependencies

`charstats` `StartSkill` (record +0xAC, `data/fields.tsv`); the rest is
in the owner specs named in load §2.

## Randomness

None in the load itself; the new character's start items and stats are
the owner specs' (`items/generation.md` §10.3).

## Edge cases & original bugs

1. The right skill is set from `StartSkill` only when the client act is
   0 (load §1 rule 1); a stub for a later act gets none.

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

## Open questions

1. None. `formats/d2s.md` Open questions 15 (golem item) and 17
   (appearance bytes) are answered there (§8.5 rule 6, §2.8 rules
   4–11); their remaining measurements are recording list IT-6 and
   IT-7.
