# PC 2 recording list: objects / hirelings lane (`claude/pc2-objects-hirelings`)

Entries not already queued in `docs/HANDOFF.md` (objects.md OQ1, OQ10,
OQ14, hirelings.md OQ9 and §8 rule 5 are queued there already).

## OP-1 `world/object-population.md` OQ1: object population of a fresh room

- Spec + question: `world/object-population.md` §3–§7 — draw order
  (room seed per group slot, then the populate function's control-seed
  or room-seed draws, with the init / PreOperate draws of each
  allocation in between), object classes and positions.
- Recording: RNG trace and packets at the same time is not possible
  (one debugger per process); run twice from the same character:
  1. `py tools/trace-recorder/record_rng.py --seconds 90` (Game.exe
     -w -ns). Single player, new Normal character, create the game,
     walk out of Rogue Encampment west into the Blood Moor and walk
     straight on for ~60 s so new rooms populate; stop.
  2. Same character, a new game, same route, with
     `py tools/trace-recorder/record_packets.py --seconds 90`.
- What to look for: in the RNG trace, per first-time populated room,
  eight active-room-seed steps (one per `ObjGrp` slot) plus one more per
  passing slot, then the populate function's steps (populate 3: one
  control-seed `% 100` then two `roll`s per spot try); objects in the
  0x51 messages of the packets run: classes from the Blood Moor
  `ObjGrp` groups, positions inside the room rectangle away from its
  border; no corpse-on-stick (57/58) from population (§7.7).
