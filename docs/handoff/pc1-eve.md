# Hand-back — PC 1 evening, 2026-10-09 (branch `claude/local-pc1-eve`)

## For the coordinator (session_01KcnkwCTXbuv5ZbToEUpBSj)

The coordinator session is not reachable from PC 1 (no cross-session
route), so this section stands in for the "A done" message.

- **Merge this branch**: it also carries `claude/local-pc1-day4` (never
  merged before; Step 4 items 47–58, the attack / cast mode end
  `client/model.md` §20 that q-fix-input-lock needs, the boss-kill
  wall-cell setups that q-fix-boss-damage needs, 40+ q-fix rows).
- **A done (interact-pokes)**: `specs/tools/poke.md` §4 r10 (call forms
  and proposed `CALL_FORMS`), `world/objects.md` §7.4, `world/npc.md`
  §2–§4. Operate at once: `0x00584420` (ECX game, EDX player; stack type
  2, GUID, out pointer; `ret 0xC`), **run on 1.14d**: S→C 0x63 at once,
  no range, no walk. NPC flow 0x13 (`0x0054AA90` / `0x00548B00`; start
  without range `0x00573020`), 0x2F `0x0054B930` → `0x00572E60`, 0x38
  `0x0054BCA0` → `0x00579D60`, 0x30 `0x0054B9F0` → `0x00572F20`.
  Corrections: the 0x2F range is 10 sub-tiles per axis (50 is 0x38's);
  the NPC interaction state is monster data +0x30 (+0x28 is the AI
  record). PROVISIONAL REC-1150: a handler called at the tick-return stop
  vs the same message drained next frame (expected difference: the frame
  events are scheduled from).

## B

| # | Item | Answer | Rows |
|---|---|---|---|
| 59 | [q-fix-act4-play] De Seis seal on WingN1 | Recorded on 1.14d (seed 1): the seal at (7773, 5155) opens; dummy 131 at **(7770, 5226)** (spot (7734, 5188) + (36, 38)); De Seis (312) at **(7773, 5207)** with five 310 minions at f86; seal mode 2 at f99. 1.14d does not leave it shut (`world/quests-act4.md` §5.4) | `q-fix-pc1eve-deseis-wingn1` |
| 60 | [q-fix-room-links] town rooms after a waypoint return | Recorded on 1.14d: as d2rs, only near rooms populate (no Gheed / Charsi to f1249, 12 of 24 objects); the old level's units are freed 122 frames after the return (`drlg/rooms.md` §8) | — |
| D1 | Ledger D1, monster creation draw order | Equal on both sides; the frame-2 report comes from `rng_owners.py` attribution (the inline component step is left `other:inline`, the HP roll at `0x00573F8F` owns the unit) (`monsters/init.md` §4.3) | extended `q-fix-tool-rng-creation-draws` |

REC block of this session: 1150–1179 (1150 used).

## C

(filled when the PROVISIONAL sweep lands)

## D

No audio item from q-tool-audio-diff in pc1-data at the time of writing.

## Probes (scratch, not in git; `..\d2rs-probes\`)

`probe_operate.py` (a sentinel `stat` poke rerouted into a direct call;
used for `0x00584420`), plus the day-4 probes.
