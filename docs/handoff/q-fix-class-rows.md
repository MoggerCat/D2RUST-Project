# q-fix-class-rows — handoff (session of 2026-10-09/10)

Branch `claude/q-fix-class-rows`. Rows: q-fix-pt-whirlwind, q-fix-pt-right-aura,
q-fix-pt-pet-warp-follow, q-fix-pt-blessed-hammer. All four playthrough cells now
pass on normal / nightmare / hell (`classes.play`: bar main-skill-kill 1/1, pal
aura-applies 1/1, ama/nec/dru/ass summon-follows-wp 1/1 each, pal 4/4).

## Done

- **Whirlwind** (REC-1060..). Three real causes, found with 1.14d runs under Wine:
  1. the client melee-range seam used a fixed reach of 2 and a max-axis distance, so a
     monster 3 sub-tiles away counted as "in melee range" for a *player* (and the weapon
     was ignored). A player's reach is now the weapon in use's `rangeadder` and the
     distance is the size-adjusted unit distance `0x00641530` (`single_player.rs`,
     `weapons.rs`, `InvItemRec::rangeadder`); monsters keep the preview rule.
  2. the unit-form mode start never set the path's target unit (`use.md` §4 "Where the
     target goes"; `body_path::point_target`).
  3. **the 1.14d result for the cell**: a unit-form Whirlwind on a monster that is in melee
     range is a plain swing (Swing, `bodies-2.md` §2.15): the player goes to mode 7 and the
     monster dies at frame 26 (`traces/checks/bar-whirlwind-unit.check`, `rclickunit 1 19`).
     d2rs never ran that swing: `BodyEffect::UnitModeRequest` had no player handler
     (`skill_use.rs` `effect`: now used skill + the path code's mode request + target).
  The point-form check `bar-whirlwind.check` now kills the Fallen at frame 22 and 26 as 1.14d.
- **Right-hand aura** (Holy Fire): the saved right skill is assigned at load; for an
  `aura` skill the assign (`use.md` §7 "0x3C SelectSkill") switches the aura state on /
  runs an `immediate` do, then schedules the aura form. d2rs only wrote `list.right`.
  `skill_events::assign_right_aura`, called after `join_gaps`. 1.14d fact (new check
  `pal-holyfire.check`, no click needed): all three Fallen die at frame 51 (first tick,
  `perdelay` period); d2rs now equal (hp, death frame, monster seeds).
- **Pet follow** (valkyrie / golem / spirit wolf / shadow warrior): the follow
  `0x005754B0` (`hirelings.md` §6 r1) only covered the hireling list. New
  `ActionHooks::summon_follow` (warp types moved with `warp_pet`; `range` types beyond
  1600 removed with kill via `summon::summon_follow`), `pettype_flags` in `BodyTables`,
  host call in `WiredWorld::pet_follows`.
- **Blessed Hammer**: the hit lookup `units_at` matched only units exactly on the missile's
  cell; the spec search (`path-placement.md` §4 r6) overlaps the missile's shape with each
  unit's. Staging had the same fix (merged, theirs taken). Verified on 1.14d:
  `pal-blessed-hammer.check` (hit at frame 29) and `pal-hammer-rat.check` (130 frames
  equal). **The playthrough cell was wrong, not the sim**: on 1.14d a Quill Rat 3 sub-tiles
  from the Paladin survives three casts (the spiral does not reach it); d2rs equals that.
  `classes.play` now has `main-skill-kill-pal` (rat at x+1, y+1, dies at frame 50 on both).

## Open (not done here)

- Whirlwind speed / velocity: on 1.14d the animation speed is 304 (rate step 7, `units.md`
  §4.7: 213·p/100 with p = 100 + Increased Speed) and the step is 8.58 vs d2rs 6.0 sub-tile
  fractions; d2rs applies no rate for the sequence mode 18. First field difference of
  `bar-whirlwind.check` (`sp`). Needs `refresh_anim_rate` for players.
- Player `fr` (anim frame) differs after a Swing from mode 5 (1.14d 0, d2rs 5120) in
  `bar-whirlwind-unit.check`; dead monster `d` (death direction, 1.14d 23 / 32) in
  `pal-holyfire`, `bar-whirlwind-unit` (monster death owner).
- Unit-form Whirlwind out of melee range (walk toward the unit): not run on 1.14d; spec
  §9.5 arrival says a player's path stops only at unit distance 0.
- Input lock after a cast (q-fix-input-lock) not touched.
- Hammer missile positions after frame 31 differ from 1.14d in `pal-blessed-hammer.check`
  (older cow variant, staging's version): type 14 path precision (pathing.md OQ9).

## Repro

```
export D2_GAME_DIR=/home/user/game
python3 tools/playthrough/playthrough.py traces/playthrough/classes.play --build --class bar|pal|ama|nec|dru|ass --difficulty normal
python3 tools/scenario-diff/scenario_diff.py traces/checks/{bar-whirlwind-unit,pal-holyfire,pal-hammer-rat}.check
```
New checks: `bar-whirlwind-unit`, `pal-holyfire`, `pal-holyfire-noclick`, `pal-hammer-rat`.
Wine setup in the cloud: `sh tools/cloud-setup.sh; tools/cloud-game/setup_winpy.sh`.
Tip: `rclickunit 1 <class>` makes both sides send the unit form (0x0D).
