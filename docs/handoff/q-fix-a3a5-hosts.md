# q-fix-a3a5-hosts — hand-off (2026-10-09)

Four 1.14d host paths that ended Act III–V checks (from
`docs/handoff/q-fix-seed-game.md` Open 3–6). REC ids used: REC-1695,
1696, 1698 (1697 dropped with item c, see below).

## Done (branch `claude/q-fix-a3a5-hosts`)

1. **(a) Secondary target `0x005DDC30` in d2-sim** (`monsters/ai.md` §5.3,
   REC-1695). `crates/d2-sim/src/wiring/action/ai_scan.rs`: forced target
   (host's), else room scan 6 over every unit of the scanner's near-room
   list (players included) with the `0x005DCBD0` rules (filter
   `0x005DC970`: player/monster, alive, not in town, unit flag 0x4, the
   state-146 rule, hostility; full-size d < 49; `nThreat` main/alt slots;
   mask-4 line test) and `0x005DD510`'s early-outs. Merged with
   q-fix-ass-traps' version: their REC-1270 melee-reach skip of a monster
   candidate is kept in the filter; `Pending::secondary_candidates` is no
   longer read (the client's list held monsters only, so no monster ever
   found the player). Tests `wiring/action/tests/ai_scan.rs`.
2. **(b) Compelling Orb spawn** (`quests-act3.md` §7.7, REC-1696):
   `QuestWorld::spawn_monster_at_unit` (`0x005B3090`) is now
   `spawn_monster_flags(room, x, y, class, mode, −1, 0)` at the unit's
   position. Test `world::quests::tests::spawn_at_unit_is_the_flags_spawn_with_spread_minus_1`.
3. **(c) Larzuk map AI**: dropped on the coordinator's word; it is
   q-fix-d2-town-mode's (`0x00545C90` store, REC-1380), merged here. My
   duplicate store was reverted before the merge.
4. **(d) Vision record** (`monsters/ai.md` §5.2, REC-1698):
   `MonsterData::vision` (+0x50) set by `set_coord_record`
   (`population.md` §9.6 step 3: the caller's record, else the one at the
   point), shared by identity (act, rect, index); `vision_seen` /
   `mark_seen` read/write `ActionHooks::vision_seen` (+0x24 := 1).

## Per check: first divergence before → after

| Check | Before | After (next) |
|---|---|---|
| milestone-baal-throne | state f50 monster 1:38 seed (S not found) | rng equal frames 3–82; state f68 player 0:1 mode 5 (TN, path target 0,0: in town on 1.14d) vs 1 — a level change, act travel / warp area |
| milestone-worldstone-portal | state f50 monster 1:38 seed | same f68 player mode 5 vs 1; also field `q` (player quest flags) empty in d2rs from f2 (newer recording carries `q`; state-dump / quests) |
| a3-warp-durance-ama | state f21, 1.14d +1 unit (orb monster 366) | state: no difference, 160 frames (PARTIAL = known gaps); rng only the frame-2/21 creation-draw attribution artifact (q-fix-seed-order) |
| milestone-act5-entry | state f26 Larzuk seed | state: no difference, 40 frames; rng equal from f5 (with q-fix-d2-town-mode merged) |
| a1-warp-tower-cellar-ama | state f45 monster 1:10 seed | rng equal frames 3–160 (leader 1:10 27/27 draws); state only `q` (player quest flags) from f2 |

Playthrough `traces/playthrough/milestones-a3-baal.play --build`: 4/4 reached.

## Open

- PC 1 items already queued cover the provisional choices: REC-1270's
  `0x005DC970` read (also settles REC-1695's draw order), the orb spawn
  flags and the vision record (`pc1-data.md` Step 4, [q-fix-seed-game]).
- `0x005DD510`'s trial path / scan 7 branch stays the host's
  (`Pending::choose_alternative`, default keep main).
- q-fix-monster-ai-2's `a4-deseis-seal-early` f36 (Oblivion Knight, S =
  None) should be fixed by (a); its orig cache is on their branch, they
  confirm it.

## Repro

```sh
export D2_GAME_DIR=/home/user/game
python3 tools/scenario-diff/scenario_diff.py traces/checks/<check>.check --channels rng,state --work ../work/<check>
python3 tools/trace-recorder/rng_diff.py ../work/<check>/orig.rng.jsonl ../work/<check>/d2rs.rng.jsonl --from 3
```
