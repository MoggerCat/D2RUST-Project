# Handoff: q-fix-monster-ai-2 (`claude/q-fix-monster-ai-2`)

Re-verified on bfff6fa3 (after the specs-staging-7 sync and the fix): items 1 and 3a still equal for 160/160 frames.

Monster AI / combat owner after q-fix-monster-ai, q-diff-combat-a1 and
q-fix-b-monster-combat (their `tools/coord/owners.tsv` rows now point
here). REC block 1660–1669: none used (no new PROVISIONAL point).

Method: each check run under Wine (1.14d) and headless d2rs with
`scenario_diff.py <check> --orig-cache --fill-cache --channels state,rng`;
the fresh 1.14d sides are in `traces/orig-cache/` (stony field, cold
plains, De Seis early, l77, l110), so later runs only rebuild d2rs.

## Items routed here

| # | Check | First divergence (state + rng) | Root | Outcome |
|---|---|---|---|---|
| 1 | `ass-lightning-sentry-hit` | none: state equal 160/160 frames | — | **equal** on this branch (integ-r7 + q-fix-ass-traps); the survivor Fallen 1:9 walks to (5143,4270) and dies as in 1.14d. No change needed |
| 2 | `a4-deseis-seal-early` | f36, Oblivion Knight 1:52 (class 312): 1.14d mode 8, d2rs mode 2 | same draw values (86, 35, 79); 1.14d takes `ai-bodies-4.md` §11 step 3 (secondary target S found, Skill1 at S), d2rs finds no S (`0x005DDC30` has candidates only for player-side units) and circles | **moved f36 → f46** with q-fix-a3a5-hosts' `0x005DDC30` merged here (2b: 1:52 now equal). New first monster divergence f46: the pack spawned at f44 (1:88–1:93) never takes the player as T in d2rs (T = 1 and the mask-4 line blocked; 1.14d targets at once): PC 1 item on the room LOS-draw test `0x0061AA40`. (The `q` field differs from f2: player quest field, not AI.) |
| 3a | `a3-warp-l77-jungle-2-ama` | none: state equal 160/160 frames | — | **equal** (the `fr` 5120 vs 0 of q-chk-levels is gone; the sequence drawn frame of q-fix-ass-traps) |
| 3b | `a5-warp-l110-siege-1-ama` | was f23: Death Maulers 1:16–1:18 never targeted the Barbarians (no think draw) | the target-node lists held players only: `0x005B1990` was a no-op, so slot 8 (NpcBarb) was empty | **fixed** (below): the maulers now match at f23; first divergence is now the Barbarian 1:15's own pick (scan 5, PC 1 item) |
| 4 | `combat-pop-stony-field` | f5 game seed; monsters 1:20+ get shifted seeds, packs differ | 1.14d creates object class 144 at (5121,5148) and its two items (class 523, game draws 729–732) right before the class-20 pack; d2rs creates neither. Every game-seed value is identical, d2rs just allocates fewer units | **not monster AI**: objects / population (sent to the coordinator) |
| — | `combat-pop-cold-plains` (coordinator note: shaman 1:28 one step short at f7) | f4: the player and the waypoint (object 2:18, class 119) arrive at x 5168/5169 in 1.14d vs 5183/5184 in d2rs | level layout / arrival, before any think; the shaman's draws are downstream | **not monster AI**: DRLG / warp arrival (sent to the coordinator) |

## Done

- **Target-node lists in d2-sim** (`monsters/ai.md` §5.2): `Game::target_nodes`
  (game +0x10F8) holds the nodes joined through `0x005B1990`; the
  wiring's `AiSummons::register_target_node` stores them and sets unit
  +0xD0; `AiTargets::target_nodes` appends them to the host's slot heads,
  so the evil search walks slots 8 and 9; the death clean-up (`0x005B1A90`)
  and `Game::remove_unit` unlink them. Tests:
  `registered_good_npcs_join_target_list_8_newest_first`, the death test's
  node check.
- Merge leftovers: `Sequence::drawn` in `unit_update/tests.rs` (the
  ass-traps field), a stale duplicate q-fix-room-links line in
  `pc1-data.md`.

## Open

0. **`0x0061AA40` room LOS-draw test** (`ai.md` §5.2 step 2): unspecified, false in every host; De Seis f46 (above). PC 1 item queued.
1. **Scan 5 callback `0x005DCA70`** (the not-evil main search): no spec;
   d2rs uses the client preview `nearest_foe`. PC 1 item
   "[q-fix-monster-ai-2] Scan 5 callback" in `pc1-data.md` Step 4 has the
   measured case (l110 f23: 1.14d picks 1:17 among three maulers at
   no-size distance 7). Once specified, run scan 5 in d2-sim and re-run
   `a5-warp-l110-siege-1-ama`.
2. **Player slots 0–7** are still the host's (`0x005B1880` at join never
   sets the player's +0xD0), so `0x005B1900` (`skills/bodies.md` §6.3,
   summons attached after their player) never inserts: monsters do not
   target summons through the lists. Needs the join to set the slot; the
   `BodyEffect::NodeInsert` / `RaiseStep::NodeInsert` effects are then
   one `Game::target_nodes.push_front` each.
3. Inactive-room restore (`0x005424F0`) does not re-register slot 8/9
   nodes (`units.md` §4.7); NpcBarb re-registers on its next think since
   its +0xD0 comes back 11.
4. The rng channel of every check shows the same attribution-only rows at
   frame 2 (seed init drawn on the `[x, 666]` seed, step vs roll labels);
   game-seed draws are equal. q-fix-seed-order fixes the attribution.

## Repro

```sh
export D2_GAME_DIR=$HOME/game          # tools/cloud-game/fetch.sh
python3 tools/scenario-diff/scenario_diff.py traces/checks/a5-warp-l110-siege-1-ama.check --orig-cache
python3 tools/scenario-diff/scenario_diff.py traces/checks/a4-deseis-seal-early.check --orig-cache --channels state,rng
python3 tools/scenario-diff/scenario_diff.py traces/checks/combat-pop-stony-field.check --orig-cache --channels state,rng
```
