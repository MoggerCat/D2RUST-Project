# Hand-back — PC 1 night, 2026-10-09 (branch `claude/local-pc1-night`)

Branch = `origin/claude/specs-staging-7` + `integ-r5` + `local-pc1-eve` +
`coord-resume-3` (one pc1-data conflict: kept the numbered item 59).

## For the coordinator (session_01KcnkwCTXbuv5ZbToEUpBSj)

The coordinator is not reachable from PC 1; this section stands in for
messages.

- **A done (hireling creation, for q-fix-hire-create)**:
  `world/hirelings.md` §3.1.1 (spawn spot + ordered draw table, recorded),
  `world/npc.md` §7.3 step 7 points there. Row `q-fix-pc1night-hire-spawn`.
  - Spot: `0x005B23C0(npc, class, mode 1, spread 4, flags 0)` →
    `0x005B2A00` = `monsters/population.md` §9.3 ring search r = 4 (d = 3,
    6, 9, 12) in the NPC's room box, mask from monstats2 `spawnCol`, size
    `SizeX`, 4 draws of the **NPC's room seed** per ring; null → same near
    the player. Kashya (4891, 4226) → hireling (4892, 4223) (recorded;
    replayed from the four draws).
  - Game seed: **one step**, at `0x00552E31` (unit-seed derivation of the
    allocation). Everything else is the local offer seed (twice), the room
    seed and the unit seed (`monsters/init.md` §4.3 rows a, b, d).
  - d2rs: `spawn_near` places at (x+2, y+2) with no search, and the game
    seed step lands in `ActionHooks::game_seed`, which the npc_world hire
    path does not sync with the economy's seed the way `quest_host.rs`
    does (likely cause of "unit seed right, game seed unchanged").

## C — numbered items

| # | Item | Answer | Rows |
|---|---|---|---|
| 62 | [q-fix-d9-arcane] A2Q4 event 3 with new level 74 | From the binary (`0x0059F0D2`–`0x0059F17C`): the level-74 branch returns on every path, so the old-level-40 block never runs for a move into 74; level 50 falls through. REC-1405 settled, d2rs right (`world/quests-act2.md` §6.6, `quests-act2-2.md` §2 item 3) | — |

## D — REC-1150 and binary-settleable PROVISIONALs

- **REC-1150 settled by recording** (`tools/poke.md` §4 rule 10): direct
  operate `0x00584420` at the tick-return stop vs C→S 0x13 sent for the
  same frame, on a spawned chest (class 5) in the Act I town: all 40
  state snapshots equal (every unit field, game seed); chest mode 2 from
  the same snapshot. No row (tool spec only; `operate` can go into
  `CALL_FORMS` as proposed).
- `grep PROVISIONAL | grep ghidra|binary|address` leaves three, none
  settleable by a binary read: `drlg/outdoor-tilesub.md` (heap history),
  `formats/d2s-legacy.md` REC-44 (needs 1.07/1.08 saves, deferred),
  `ui/control-panel.md` REC-610 (D3D/Glide run, full-screen OK needed).
- `tools/provisional_index.py` classes 32 lines as "binary": all are
  crate comments or spec history of RECs already settled (REC-80/81,
  REC-415, REC-235, REC-742 recorded on day 4) or front-end shell
  markers (REC-168/180, d2rs-own choices). None needs a new read; the
  crate markers are cleanup for the owning fix sessions.

## Recordings (local, `traces/raw/`, not committed)

`check-hire-kashya/orig.{state,rng}.jsonl` (scenario_diff `--orig-only
--channels state,rng` of the q-chk-hirelings check `hire-kashya`);
`rec1150/{send2,poke2}.state.jsonl` (REC-1150).
