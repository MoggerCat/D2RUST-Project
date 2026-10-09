# Handoff: q-fix-ass-traps (`claude/q-fix-ass-traps`)

Task: the assassin's Lightning Sentry never killed. REC block 1270–1279
(1270, 1271 used).

## Done

Under Wine (1.14d, `tools/cloud-game/`) the coordinator's
`ass-lightning-sentry-hit` check (a Fallen pack, 160 frames) showed these
first divergences, fixed in order; each fix was re-run through the check:

1. **Trap target scan** (`monsters/ai.md` §5.3, REC-1270): d2rs picked the
   nearest foe with no filters. `View::secondary_target` now runs the scan
   6 callback in d2-sim over the host's foe list
   (`Pending::secondary_candidates`, client `LocalSeams`): distance < 49,
   `nThreat` main / alternative, the mask 4 line test, and a monster
   candidate in the scanner's melee range skipped (1.14d: never the Fallen
   at distance 1, the next one at 2–3). PROVISIONAL.
2. **Summon skill level** (`skills/bodies.md` §6.5 step 6): the trap's
   used skill entry was level 1 (`MonsterAi::used_skill` stub); it now
   reads the sim's `monster_skills` (missile `lvl` 20 as in 1.14d).
3. **Sequence drawn frame** (`skills/sequences.md` §2 step 3, §3): a
   sequence mode now stores the drawn frame · 256 in +0x44 (`Sequence::drawn`,
   load and each advance); the trap's `fr` was stuck at 0.
4. **Unit hit search** (`sim/path-placement.md` §4 rule 6, `missiles.md`
   §R4 step 9): `units_at` found only a unit on the exact sub-tile; it now
   applies `r` = missile size and the overlap table, mode skips and unit
   size. The bolt now hits the frame 1.14d does (RNG draws equal).
5. **Kill direction** (`combat/damage.md` §7.2 step 3): the death request
   faces the killer (`path_dir64` + the snap `0x006488A0`,
   `View::path_snap_direction`); `d` of a dead monster was 0.

New check `traces/checks/ass-lightning-sentry-kill.check` (Fallen pack,
click 3 sub-tiles from the nearest): **equal for 160 frames**.
`ass-lightning-sentry-hit` is equal to frame 75; its first divergence
(frame 76) is the Fallen AI after its leader dies (1.14d: the survivor
walks off to (5143, 4278) and takes the dead pack leader as owner; d2rs
leaves it standing). That is monster AI (`monsters/ai-bodies*`), not the
trap: route to the monster-AI owner.

Playthrough (`python3 tools/playthrough/playthrough.py
traces/playthrough/classes.play --build --class ass --difficulty normal`):
**5/6** (was 3/6 with main-skill-kill and trap-kills stuck). The two
milestones assumed a trap laid on or next to the rat kills it; 1.14d's
sentry skips a monster in its melee range, so `trap-kills` clicks at
(x-1, y) and a new `main-skill-kill-trap` replaces `main-skill-kill` for
the assassin (`only class` on the old one). Remaining blocker:
`summon-follows-wp` (Shadow Warrior, class 417, absent after the waypoint
warp): not the traps; belongs to the summon owner.

## Open

- REC-1270 / REC-1271 settle with the two PC 1 items in
  `docs/handoff/pc1-data.md` Step 4.
- The sentry stands still after its charges; death (`charges`), Wake of
  Fire / Inferno / Charged Bolt / Death Sentry bodies were not compared:
  queue checks like `ass-lightning-sentry-kill` for each.
- An idle poked cow (hp 0 in both snapshots) was shot by d2rs but not by
  1.14d: the poke's hp 0 may make it "dead" to the scan; not pursued.

## Repro

```sh
export D2_GAME_DIR=$HOME/game          # tools/cloud-game/fetch.sh
python3 tools/scenario-diff/scenario_diff.py traces/checks/ass-lightning-sentry-kill.check
python3 tools/scenario-diff/scenario_diff.py traces/checks/ass-lightning-sentry-hit.check
python3 tools/playthrough/playthrough.py traces/playthrough/classes.play --build --class ass --difficulty normal
```
