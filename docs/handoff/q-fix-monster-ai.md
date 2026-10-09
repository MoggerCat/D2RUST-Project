# Handoff: q-fix-monster-ai (`claude/q-fix-monster-ai`)

REC block 1110–1119 (1110, 1111 used). Area: monster AI hosts in d2-sim.
Method: 1.14d recorded under Wine (`tools/cloud-game`), d2rs rerun with
`scenario_diff.py <check> --work DIR [--reuse-orig]`; the compare of the
poked units follows them by GUID (population differences at f5 are
q-fix-bloodmoor-population's).

## Done

1. **q-fix-c2-fallen-s2-choice**: already fixed on staging (0a725676,
   `set_owner_data` / `add_minion` in the world host). Verified:
   `combat-fallen-hits-player`: the 3 poked Fallens equal (mode, position,
   unit seed, life) for all 200 frames, player life equal.
2. **q-fix-c3-quillrat-choice**: `MonsterData.ai_state` (+0x54) stored;
   `ActionHooks::ai_state` reads it; reaction steps 4.5/4.7 write it
   (`ActionHooks::set_monster_ai_state`); the monster mode set applies
   `0x005A68E0` to the mode it leaves (`leave_monster_mode`). Test
   `ai_state_is_stored_and_follows_the_mode_set`. `combat-arrow-quillrat`:
   the rat's modes now equal 1.14d for all 130 frames (A2 at f59, f87,
   f115); its seed differs by one step from f46 (quill vs player: the
   player is no missile target, row `q-fix-c6-player-flags`, not mine).
3. **Fallen AI outside town** (`combat-melee-fallen`): three causes, all
   in the AI hosts:
   - `AiHost::in_melee_range` used the client preview (max axis <= 3);
     now `0x00622C40` (hit.md §7.2): MeleeRng + 1 against the unit distance
     `0x00641530` and the 0x804 line (PROVISIONAL REC-1110: MeleeRng 255,
     weapon class, = 0).
   - The room last-dead ring (`0x0061AFA0`, units.md §4.6 rule 1.3) was
     never written, so the corpse check (ai-bodies §9.4 step 2) never
     fled: `ActionHooks::last_dead` + `push_last_dead` at the death start.
     Test `last_dead_ring_keeps_the_last_four_in_slot_order`.
   - The AI velocity request's speed (P +0x10) was stored and never read;
     now a temporary stat-67 list per walk/run start (PROVISIONAL
     REC-1111; 1.14d flees at 125/75 of the walk speed).
   Result: Fallens 19 (leader) and 20 equal to f150 (mode, position, seed).

## Open

- `combat-melee-fallen`: the Fallen killed by the player's swing (1.14d
  GUID 21, d2rs 19) differs in unit seed from f46 (death; q-fix-monster-death
  / q-fix-player-hit).
- `combat-arrow-quillrat`: needs `q-fix-c6-player-flags` for the f46 rat step.
- Settling captures: REC-1111 needs another AI speed-bonus recording
  (e.g. a Fallen shaman or `circle`), REC-1110 a MeleeRng 255 monster.

## Repro

```sh
export D2_GAME_DIR=$HOME/game
python3 tools/scenario-diff/scenario_diff.py traces/checks/combat-fallen-hits-player.check
python3 tools/scenario-diff/scenario_diff.py traces/checks/combat-melee-fallen.check
python3 tools/scenario-diff/scenario_diff.py traces/checks/combat-arrow-quillrat.check
```
