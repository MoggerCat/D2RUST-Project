# Handoff: PC 2 spec worker — hirelings (`claude/spec-hirelings`)

Local spec session, 2026-10-07. Owner files: `specs/world/hirelings.md`,
`specs/world/npc.md` (GN1 and hireling rules only; third pass: npc.md open questions too), `specs/world/hirelings-ai.md` (new, third pass). Inputs:
`docs/handoff/impl-hirelings.md` HL1–HL9, `docs/HANDOFF.md` §7 Ninth set
(HL-1 … HL-9, WW-2, R2-27, R2-28), `docs/handoff/gaps-night-specs.md`
GN1, recordings of `origin/claude/local-buddy-recordings-2026-10-07`
(Pass 2 B) and `origin/claude/local-buddy-q-rec-2026-10-07` (entry 80).
Evidence: 1.14d `Game.exe` exports + `tools/ghidra/disasm.py`; live
`experience.txt`, `hireling.txt`, `skills.txt`, `pettype.txt`. The 0x7A
layout (owner @5, pet @9) was left as it is.

## Answered

| Id | Spec § | Answer |
|---|---|---|
| HL1 / HL-1 (§7.2 ExpRatio kill share) | `hirelings.md` §7.2 rule 3 | `0x0057E390`: r = `ExpRatio` of alvl, s = `MaxLvl` row `ExpRatio` (10); e > 0x7FFFFFFF >> ((r >> s) + s) → (e >> s)·r, else (r·e) >> s |
| HL2 | — | not reopened (layout settled owner @5, pet @9; `hirelings.md` §13 rule 2 already says so) |
| HL3 / HL-3 | §5 rule 3 | full-list eviction is `0x00574850(head, kill 1)` directly: one 0x7A broadcast + the kill; `pets.md` §5 r2 is right |
| HL4 / HL-4 | §5 rule 3 | `0x00575900`: per-skill `petmax` max, else `basemax`; type 7 always `basemax` 1 (no skill has pettype hireable) |
| HL5 / HL-5 | §3.2 rule 6 | the add is void; the init continues (rules 7–11) |
| HL6 / HL-6 | §7.1 rule 2 | defender level, defender exp, merc level: base reads (`0x006253B0`) |
| HL7a/b / HL-7a/b | §11 rule 4 | old-item copy after "cursor := none", before the refreshes; all four refresh calls act on the merc; first 0x540E60(9) to the merc, second to the player |
| HL8 / HL-8, OQ7 | §13 rule 4 | stats queued on the merc, flushed per client in the next client pass to every client updating the merc; twice for a unit new to the client (recorded); damage sums sent as stat ids 21 / 22; experience delta goes at once to the owner only |
| HL9 / HL-9 | §1.2 rule 3 | `0x00663750(exp, class 0, name)`: `0x00656440` matches **class** (never with class 0), fallback `0x00656390` = name range |
| OQ8, WW-2 | §8 rule 1 | `0x00457490(class, 15)` = `monstats` `killable`; flag 0 only from `0x00574450` (expired pet), 1 at the other 14 sites |
| OQ6, GN1 | §9 rule 3, edge case 5; `npc.md` §7.4 step 2, edge case 11 | a living hireling is charged then freed and used after free (undefined); both specs now say: d2rs refuses with 0x2A code 9, nothing changed |
| OQ9 (part), R2-27 (part) | Test vectors (Recorded) | hire recorded and checked: 0x81 bytes, 0x27 speech, list, 0x2A code 5, no 0x7A, stats twice next tick, values = §4 for Id 0 L 7, price 160 |
| R2-28 (part) | §6 rule 6 | town-portal follow: 0x0A in the input phase, merc 0xAC at the destination before the player's 0x15 |
| `npc.md` OQ6 (hire part) | `npc.md` Test vectors | hire order recorded (frames 969–1731) |
| §6 extra 0x15 (third pass) | `hirelings.md` §6 rule 7 | it answers the client's C→S 0x4B (type 1, GUID 1) of the 2919 input phase: `0x0054C6D0` sets flags 2 0x10000 + queue → next client pass `0x00598220` sends 0x15 at `0x005982A8`; the warp's own flag is cleared unsent (client pass on the old room, `0x0053B000`) |
| OQ1 | `hirelings.md` OQ1 | settled by `formats/d2s.md` §2.5 / §8.4 |
| OQ2 | §8 rule 5 | a dead hireling in a deactivated room is kept roomless (`0x005431F0` → `0x005752B0`), flags 2 0x100, revive warps it |
| OQ3 | OQ3 | unreachable: `0x0063D470` (only writer, 3 callers) always adds flag-1 corpse nodes |
| OQ4 | §1.1 rule 6, `hirelings-ai.md` §1 | `DefaultChance` read only by AI skill pick `0x005E4D30`; Head/Torso/Weapon/Shield never read |
| OQ5 | §13 rule 8 | 0xD7C = `merclevelup` "I feel much stronger now" |
| OQ9 rest | OQ9 | **Needs recording** (exact contents in the spec) |
| `npc.md` OQ1 | `npc.md` OQ1 | `0x00457490(class, 9)` = `interact` |
| `npc.md` OQ2 | OQ2 | slot 1: AI control +0x14 := 40 (effect: AI spec) |
| `npc.md` OQ4 | edge case 13 | two 0x58 result 7, item dropped next to the player |
| `npc.md` OQ5 | OQ5 | `9b ffff 00000000` from `0x0053E0E0` args at `0x00579CF5`/`0x00579D08` |
| `npc.md` OQ6 rest | OQ6 | **Needs recording** |
| relay (ui worker) | `npc.md` §4 client senders, Test vectors | client 0x38 action 3 for classes 252/198/515/150 with player GUID @9 (`0x004B48E8`); 0x36 id u16 widened to u32 (`0x004B1E94`), server reads u16 (`0x0054BBE6`) |

## Still open

- `hirelings.md` OQ9 rest (level-up, death, resurrect, give / take):
  Needs recording. OQ10 (new): readers of flags 2 bit 0x100 on a
  revived hireling.
- `hirelings-ai.md` OQ1: the Hireable AI think around the skill pick
  (PC 1 AI spec).
- `npc.md` OQ3 (record +0x24..+0x26 readers: the 29 functions that
  obtain a record read none; the record is passed on, so the whole-image
  sweep stays open), OQ6 rest (Needs recording), OQ7 (movement spec).
- §6 rule 7: why the client sends the 0x4B on the first teleport only
  (client behaviour, PC 1).

## CODE-TABLE CHANGE commits

None (no hireling TSV exists; no code-mirrored table touched).

## Cross-file requests

- to PC 1: `specs/combat/vitals.md` §4.3 / OQ2; `0x0057E480` applies the
  `ExpRatio` step `0x0057E390` after the level factor (`push edi` alvl at
  `0x0057E4CE`, gain in EAX): r = `0x00613E60(alvl)` (word 8·alvl + 15;
  alvl < 1 → word 7; alvl > word 0 → 0), s = `0x00613E60(0)` (10); s − 1
  ≥ 31 → unchanged; e > 0x7FFFFFFF >> (((r >> s) + s) & 31) → (e >> s)·r,
  else (r·e) >> s; then + pct(gain, stat 85, 100); state it for players
  (`world/hirelings.md` §7.2 rule 3 has the same text).
- to PC 1: `specs/combat/damage.md` §7.2; "pet kill credit to a player
  owner" is `0x005751A0` (pet death bookkeeping: node dead bit, 0x9B, 0x7A
  remove), run only when the 4th argument is non-zero (0 only from
  `0x00574450` at `0x005744D3`) and the owner is a player; the
  `killable` test is `0x00457490(class, 15)` at `0x0057CCF4`; link
  `world/hirelings.md` §8 rule 1.
- to PC 1: `specs/sim/pets.md` §5 r2 / SK-2a / OQ2; the full-list eviction
  in `0x00575C70` calls `0x00574850(head GUID, list, 1)` at `0x00575CE5`
  (one 0x7A broadcast, then `0x00574450`), asserts 0x312 / 0x316; max
  recompute `0x00575900` described in `world/hirelings.md` §5 rule 3
  (per-skill `petmax`, else `basemax`; `0x00575850` evicts with kill for
  types ≠ 7 while count > max); answers its OQ2 for type 7.

- to PC 1: `specs/monsters/ai.md` §3.2 / `ai-functions.tsv` row 61
  (Hireable, "unread"); the skill pick `0x005E4D30` (called by
  `0x005E5050` at `0x005E51EA` / `0x005E5229`) is specified in
  `world/hirelings.md`'s split file `world/hirelings-ai.md` §1
  (DefaultChance / Chance / ChancePerLvl roll, aura → `0x005701B0`,
  per-class fallback); the think `0x005E52D0` / `0x005E5050` around it
  is still to spec there.
- to PC 1: `specs/monsters/ai.md` (town NPC AI); `world/npc.md` §2 rule
  2: `0x00548D4A` calls `0x0058EC00(npc, 1, 0x28)` = AI control +0x14
  (`dwAiParam[0]`) := 40 on every 0x13 at distance ≤ 50; state what the
  NPC AI does with it.
- to PC 1: `specs/client/model.md` §5 rule 5; the 1.14d client sent C→S
  0x4B `4b 01000000 01000000` (own hireling) in the input phase after
  tick 2919 of `tp80-packets.jsonl` (town-portal teleport while the
  hireling walked; not on the second teleport, hireling standing); the
  server answer is `world/hirelings.md` §6 rule 7; explain which client
  rule set flag 0x800000 / flags-2 0x20 on that unit.

## Recording list

- R2-27 rest: level-up (0xA1 / 0xA2 and the second stats batch),
  death (0x9B name id + cost, 0x7A remove), resurrect at an NPC (0x9B
  `ffff 00000000`, 0x81, 0x2A code 5), give / take an item (two 0x540E60
  notices, new GUIDs). Same character `bdMercTwo`.
- OQ7 confirmation: a two-player game where the second client sees the
  owner's hireling level up; expected: the second client also gets the
  hireling's 0x9E–0xA0 stats, not the 0xA1 / 0xA2 delta.
- OQ2: die, change level, return and resurrect.
- `hirelings.md` OQ9 rest: level-up, death, resurrect, give / take,
  each with the exact messages listed in the spec (OQ9 a–d).
- `npc.md` OQ6 rest: resurrect, heal at Akara, Cain identify (3 items
  and none), imbue / socket / personalize, act travel (OQ6 a–d).
- Optional confirmation of `hirelings.md` §8 rule 5: hireling dies in
  the wilderness, player goes to town and waits > 11 room passes (132
  frames) so the room is freed, then resurrects: expected 0x81, the
  merc's 0xAC at the player, `9b ffff 00000000`, 0x2A code 5, and no
  0x4B / error.
