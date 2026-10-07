# Handoff: PC 2 spec worker — hirelings (`claude/spec-hirelings`)

Local spec session, 2026-10-07. Owner files: `specs/world/hirelings.md`,
`specs/world/npc.md` (GN1 and hireling rules only). Inputs:
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

## Still open

- `hirelings.md` OQ1 (save layout: save spec), OQ2 (dead hireling after
  level change: recording), OQ3 (corpse-list restore), OQ4 (`Head`…
  readers), OQ5 (string 0xD7C), OQ9 rest (level-up, death, resurrect,
  give / take recordings).
- §6 rule 6: why the first teleport had an extra hireling 0x15.
- `npc.md` OQ5 (0x9B bytes), OQ6 rest (resurrect, heal, Cain, services).

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

## Recording list

- R2-27 rest: level-up (0xA1 / 0xA2 and the second stats batch),
  death (0x9B name id + cost, 0x7A remove), resurrect at an NPC (0x9B
  `ffff 00000000`, 0x81, 0x2A code 5), give / take an item (two 0x540E60
  notices, new GUIDs). Same character `bdMercTwo`.
- OQ7 confirmation: a two-player game where the second client sees the
  owner's hireling level up; expected: the second client also gets the
  hireling's 0x9E–0xA0 stats, not the 0xA1 / 0xA2 delta.
- OQ2: die, change level, return and resurrect.
