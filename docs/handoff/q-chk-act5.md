# q-chk-act5 (REC-14100)

## Done
- 39 Act 5 checks authored by `tools/chk-act5/gen.py` (rerun to regenerate), run under Wine with
  `tools/scenario-diff/suite.py --filter 'a5-*' --workers 3 --no-playthrough` (all state channel):
  - `a5-npc-{larzuk,malah,nihlathak-town,qual-kehk,cain5}`: `goto preset 109 1:<cl>` then `msg 0x13 1 @1:<cl>`.
  - `a5-wp-30..38-*`: all nine Act 5 waypoints: warp, `goto preset <lv> 2:<obj>`, open (0x13 type 2), travel to Harrogath (`TakeOrCloseWp`).
  - `a5-su-*` (19): `poke superunique <row>` (rows 42-60) beside the player in level 110, life 1, Fire Bolt missile; drops are ut 4 units.
  - `a5-town-npc-sweep`: placement sweep of Harrogath (only Larzuk 511 and Malah 513 are in the active rooms; both sides agree).
- Verdicts: 0 EQUAL, 37 DIVERGED, 2 PARTIAL (the pre-existing `a5-warp-*` state). 14 ledger rows settled in
  `docs/handoff/ledger/q-chk-act5.tsv` (5 NPCs, 9 waypoints), all DIVERGED. (ledger.py warnings about missing
  specs/checks come from running it outside the integrated tree.)

## First divergences, routed
| Checks | First divergence | Route |
|---|---|---|
| a5-npc-*, a5-wp-30, a5-town-npc-sweep | frame 24-30, town NPC (511/514) mode 2 vs 1 (wander); masks the talk | town NPC idle/wander, same as `a5-harrogath-arrival-ama` (owner of NPC/town AI: q-play-act5 / monsters AI) |
| a5-wp-31..36, 38 | frame 450, waypoint object mode after the 0x13: 1.14d 1 vs d2rs 2 | waypoints owner (owners.tsv row 16, claude/q-fix-pc1-proto-items) |
| a5-wp-37 | frame 51 level-118 monster unit seed | D1, q-fix-real-unit-seed-order |
| a5-su-* (all 19) | frame 7 level-110 monster 522 tx 4324 vs 4321: warp-110 population differs, before the spawn | level 110 population (q-prov-recording / DRLG outdoor); the kill itself is not reached, so spawn/kill/drops are unverified |

## Open (not done)
- Quest flag sequences (a5q1..a5q6, act intro): impossible with today's tools. The state channel's `q` field is written by d2rs only;
  the 1.14d recorder (`record_state.py`) does not read it ("not compared"). Needs the recorder to read it
  (owner claude/q-tool-state-diff). Then author per-quest checks from `traces/checkpoints/a5-*`.
- Store open/hire/heal/resurrect/identify: need the packets channel with interact pokes (q-tool-interact-pokes); only the 0x13 talk is driven.
- Drehya/Anya (512) has no town preset; Qual-Kehk, Nihlathak, Cain are reached by `goto preset` but absent from the sweep's active rooms.
- Superunique natural placement and the Ancients/Baal fights: existing `milestone-*` checks; not rerun.
- Superunique checks should move to a level whose population matches once level 110 does.

## Repro
`python3 tools/chk-act5/gen.py && python3 tools/scenario-diff/suite.py --filter 'a5-*' --workers 3 --no-playthrough`
(needs `D2_GAME_DIR`, Wine setup via `tools/coord/session-setup.sh`).
