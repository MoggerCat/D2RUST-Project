# Hand-back: q-fix-hire-create (`claude/q-fix-hire-create`)

FIX session for the hire frame of the 15 hireling checks of
`q-chk-hirelings`. REC-1595..1604: none used (no unsettled choice).

## Done

The hire frame (C->S 0x36, `npc.md` §7.3 step 7) is now EQUAL in every
check that hires (11 of 15); `hire-kashya` and `hire-greiz` are EQUAL on
all 90 frames (state PARTIAL: only the known `own` / client gaps).

Two causes, both in the hireling creation path (none in shared placement
code, so no note to q-scenes-compare / q-fix-d4-placement):

1. **Game seed lost.** d2rs did draw the allocation's unit-seed step
   (`rng.md` §5.3; the rng channel showed the game draws equal), but on
   `ActionHooks::game_seed` while the NPC call runs on the economy's copy
   (`GameFields::seed`), which `with_economy_on` writes back over the hooks
   at the end: the step was overwritten. `LifecycleHooks::spawn_near` now
   takes the call's game seed (`&mut Seed`, as `town_portal_cast` does) and
   lends it to the hooks for the allocation.
2. **Spawn tile.** `ActionHooks::spawn_near` placed the mercenary at a
   d2rs-own (+2, +2) offset. It is now `0x005B23C0(game, near, class, mode,
   4, 0)`: population's §9 placement and creation (`placement::place_at`
   through `MonsterWorld::spawn_at`) around the NPC's path position, ring
   search with spread 4 on the active-room seed, flags 0 (extras, init,
   party). The (+2, +2) allocation stays only as the fallback for a host
   with no lent monster world.

Fixture: `e2e_night_world.rs` population tables now carry the mercenary
class row (the §9 placement reads monstats and monstats2 of the class).

## Next first divergence per check (state channel, after this fix)

| Check | Hire frame | Next first divergence |
|---|---|---|
| hire-kashya | 14 EQUAL | none (90/90) |
| hire-greiz | 14 EQUAL | none (90/90) |
| hire-asheara | 20 EQUAL | frame 24 hireling 1:12 class 359 unit seed `s` (follow AI draws); tile from frame 64 |
| hire-qual-kehk | 14 EQUAL | frame 29 monster 1:10 class 514 (NPC) mode 2 vs 1, ty 5117 vs 5127 (NPC walk; hireling equal) |
| hire-resurrect-kashya | 14 EQUAL | frame 71 hireling 1:13 after 0x62: mode 2 vs 1, tile (4890,4222) vs (4894,4223) |
| hire-resurrect-greiz | 14 EQUAL | frame 70 hireling 1:22 after 0x62: mode 1 vs 2, tile (5029,5045) vs (5028,5041) |
| hire-resurrect-asheara | 20 EQUAL | frame 24 as hire-asheara |
| hire-resurrect-qual-kehk | 14 EQUAL | frame 29 as hire-qual-kehk |
| hire-follow-warp-kashya | 14 EQUAL | frame 54 hireling warp follow: mode 2 vs 4, tile (5147,4260) vs (5144,4266) |
| hire-follow-waypoint-kashya | 14 EQUAL | frame 93 player 0:1 mode 5 vs 6 (waypoint travel) |
| hire-items-kashya | 14 EQUAL | frame 30 item 4:1 class 306 (cap) present in 1.14d, absent in d2rs |
| merc-levelup-a1/a2/a3/a5 | no hire | frame 4 (unchanged): the saved hireling follows the `poke warp 2` one frame late in d2rs (1.14d frame 4 at (5145,4266), d2rs frame 5); same as merc-*-cow, routed q-scenes-compare |

`docs/handoff/checks-status.md` rows updated.

## Open

- hire-asheara frame 24: the Act III hireling's follow AI draws on its
  unit seed differ (`hirelings-ai.md`; skills/hireling owner
  q-diff-skills-2).
- Resurrect (0x62): the revived hireling's mode and tile (§9 revive).
- Warp follow one frame late (merc-levelup, merc-*-cow) and the
  hire-follow-warp frame 54 mode are likely the same pet-follow timing.
- rng channel (hire-kashya): the game seed is equal on all 40 draws; the
  per-unit first divergence is frame 2 (population units 1:3, 1:4, ...:
  1.14d's recorder shows no `roll(n=1)` at create.rs:265, before the hire),
  not this session's.

## Repro

```
export D2_GAME_DIR=$HOME/game CARGO_INCREMENTAL=0
python3 tools/scenario-diff/scenario_diff.py --channels state,rng traces/checks/hire-kashya.check
for c in traces/checks/hire-*.check traces/checks/merc-levelup-*.check; do
  python3 tools/scenario-diff/scenario_diff.py $c; done
cargo nextest run -p d2-sim -p d2-server
```
