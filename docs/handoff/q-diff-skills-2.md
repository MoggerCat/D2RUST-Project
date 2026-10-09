# Handoff: q-diff-skills-2 (`claude/q-diff-skills-2`)

Diff-driven area F2: player skills of the Barbarian, Druid and Assassin,
plus hirelings. Loop: a `.check` in `traces/checks/`, `python3
tools/scenario-diff/scenario_diff.py <check>`, fix the first divergence,
re-run. REC block 770–779 (none used yet).

## Checks

| Check | What | State (2026-10-09) |
|---|---|---|
| `merc-rogue-town-bar` | saved Act I rogue restored at the join, idle in town, 80 frames | **equal** in every compared field (PARTIAL only for the snapshot's own gaps) |
| `bar-battle-orders`, `dru-hurricane`, `dru-werewolf`, `dru-grizzly`, `ass-fade`, `ass-burst-of-speed`, `ass-lightning-sentry` | warp to the Blood Moor (variant `blood-moor-empty`), one right-click cast at frame 20, no target | **equal** in every compared field for 70 frames |
| `ass-shadow-master` | same | frame 28: the shadow's six items (monequip `0x005D6B60` → item creation `0x00573B20`) are not made in d2rs: no monster item creation exists (the `InitHost::create_equip_item` seam is a no-op), so game seed, life and mana differ |
| `bar-war-cry`, `bar-whirlwind`, `bar-leap-attack`, `dru-tornado`, `dru-fissure`, `dru-volcano`, `ass-fire-blast`, `ass-mind-blast` | same, with a Fallen at (+4, 0) as the target (the variant gives fallen1 the Idle AI: killable, no draws) | being re-run |
| `merc-rogue-fallen`, `merc-desert-fallen`, `merc-sorc-fallen`, `merc-barb-fallen` | a hireling of each act follows the warp and fights the idle Fallen | being re-run |

Layout of the skill checks: `at 4 poke warp 2` (arrival at the Den of
Evil entrance tile, 5143, 4263), `at 10 poke spawn 179 @x+4 @y normal`
(the variant's idle Fallen keeps monster AI, area B, out of the comparison;
a `cow`, class 179, looked idle too but is a neutral critter without life),
`input frame 20; rclick X Y` with X, Y = 544, 336 (directional: the
point (+8, +1), past the Fallen) or 330, 300 (self casts, away from it).
Both keep the click off d2rs' stand-in hover box
(`bridge::combat::hover_at`, which picks a monster up to 4 sub-tiles
down-right of the click and turns the cast into a unit cast, C→S 0x0D).
`rng` and `packets` take no pokes on 1.14d, so the checks run `state`.

## Fixed (each found by a check, first divergence)

1. **Saved hireling never restored** (`merc-rogue-town-bar`): the play
   host never turned on `ActionHooks::hireling_calls`; the restore had
   no lent monster world (its monster init, 9 seed steps, was skipped);
   the join follow ran a tick late. Now: queue on, the restore drained
   right after the load's items and corpses with the world lent, and
   `WorldHost::session_work` runs the hireling calls after a session
   message. `d2s-tool --merc ID,NAME,SEED,EXP` writes the block.
   Test `app_hireling_restore`.
2. **Passive skills of a loaded save not applied** (`bar-battle-orders`
   frame 2: stamina 121 vs 502 with Increased Stamina 20): the load's
   assign turns the passive states on and fills their lists
   (`skill_events::passive_refresh_all`). Test `app_passive_load`.
3. **Room presets in the wrong order** (`bar-battle-orders` frame 4):
   objects, monsters, then warp tiles; 1.14d walks objects and tiles in
   list order first (`drlg/rooms.md` §6). `View::spawn_preset_units`.
4. **Town monsters kept thinking after the player left** (every check
   after `warp 2`, frame 24): a room left with no client must give its
   monsters `0x005738D0` (cancel types 2 and 3, `intents-events.md` §7.8
   r3.2). Test `a_room_left_without_clients_cancels_...` (d2-sim).
5. **Server animation rate** (`dru-werewolf`, `ass-burst-of-speed`):
   the server used the AnimData speed as is; now `units.md` §4.7 on the
   draw identity (`0x00645270`), the anim refresh of a stat fill re-rates
   the running mode, the move rate's item/skill getter is the unit total.
   Test `app_skill_rates`.

## Open

- Monster item creation (`0x00573B20`; monequip `monsters/init.md` §12,
  summon equipment `skills/bodies.md` §6.5 step 9): not implemented
  (seam no-op). Blocks `ass-shadow-master` (and Shadow Warrior, and every
  monster with `monequip` rows).
- Dual-wield average of the attack rate (`units.md` §4.7 step 8.3):
  TODO in `ActionHooks::spec_anim_rate` (the body-location items are not
  reachable from the action wiring).
- Blood Moor population from frame 5 on the base install (monsters 8, 9
  class 63 placed at other points, then the classes differ) and the
  Fallen AI out of town (one unit-seed step short per think): area B,
  sent to q-diff-combat-a1 (2026-10-09). The skill checks avoid both
  (variant, idle Fallen).
- `scenario-diff.md` §3 r8.2 says the headless click has no hover pick;
  the code falls back to `combat::hover_at` (other areas' attack checks
  rely on it). Not changed.
