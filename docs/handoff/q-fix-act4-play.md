# q-fix-act4-play (REC-1160..1169, none used)

Area: Act IV progression in `tools/playthrough/playthrough.py`. Baseline
2026-10-09 (staging 0a297048): `act4.play` 10/12 reached (9/12 consecutive),
`act4-blockers.play` 5/6. Now: **12/12 and 6/6**.

## Done

- `diablo-present` / `diablo-killed` (`traces/playthrough/act4.play`): the
  old milestone warped to the Sanctum and waited for Diablo with no seal
  opened, so Diablo (spawned only after five seals are open and three seal
  bosses are dead, `quests-act4.md` §5.4-§5.6) never appeared. They now run
  the real flow: `goto preset 108 2:<seal>` + `msg 0x13 2 <guid>` for the five
  seals, the three bosses (Infector 362, De Seis 312, Vizier 306) pulled next
  to the player at 1 hp and shot with Fire Bolt, then a `goto` to Diablo's
  start point (object 255): the timer needs the start-point object loaded (its
  init 55 stores the GUID; a stale GUID after the room was dropped makes the
  timer retry forever, as the spec says), and Diablo appears at f2840.
  Diablo's kill then follows (f2907). No spawn poke, no quest poke.
- `fallen-fire-bolt-105` (`act4-blockers.play`): a test defect, not a game
  bug. The spawn is a pack of four and `dead` needs every match dead; it now
  names the spawned leader (`g @p1`).

## Open

1. **Seed 5 only.** With the level seed picking WingN1 (seeds 1-4, 6-9, 11)
   the De Seis seal (394) never opens: its boss spot (seal + (-39,+33)) lies 6
   sub-tiles west of the DS1 (the seal is at in-file x 33; WingN2 has it at
   x 75). `free_spot` (`quests-helpers.md` §1, implemented as written,
   edge-case-1 quirk included) accepts only (7780,5160) after 48 rings, and
   the spot's room lookup from R0 is null, so the dummy is not created and
   the seal stays in mode 0. The spec follows the binary, so this is either
   what 1.14d does for that layout or a difference in adjacency / room
   boxes. PC 1 item added (pc1-data.md Step 4): open the De Seis seal on a
   WingN1 game.
2. `goto`'s result GUID cannot be named by `@pI` (`POKE_TYPES` has no
   `goto`): the milestone hardcodes the seed-5 GUIDs (122, 127, 150, 202,
   195, 230 for the gotos; 146/147/148 bosses; 149 Diablo). Any change to
   earlier spawns shifts them. Suggest to q-fix-pt-sweep: let `@pI` resolve a
   `goto` (type from its filter) and a boss by class+superunique.
3. The player is given 30000 life (`stat 7 / 6`) for the walk: the Sanctuary
   packs kill the level-50 sorceress otherwise (mode 17 by f500).

## Repro

```
export D2_GAME_DIR=/home/user/game
python3 tools/playthrough/playthrough.py traces/playthrough/act4.play traces/playthrough/act4-blockers.play --build
```
