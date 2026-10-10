# rc-wp-last3 hand-back

Branch `claude/rc-wp-last3` (specs-staging-7 + integ-r17). Checks: `gen-wp-*` (39).

## Before / after (state ticks equal of 17940)
- Before (rc-wp-seed400): 17809, DIVERGED 3 (wp-2, 28, 37).
- After: 17880, DIVERGED 1 (wp-28). 38 PARTIAL (state equal).

## What changed
1. Outdoor level reset `0x006754C0` (`drlg/outdoor/mod.rs::reset_level`) now clears
   outdoor flags 0x20 and 0x40 as 1.14d does. d2rs kept them, so a level freed
   after inactivity and regenerated (the waypoint trip at frame 400 back to
   Stony Field / Glacial Caves) skipped the cliffs and cave placement and laid
   out a different level than the first generation. Fixed wp-2 (player
   position, waypoint at tile 1019,1035) and wp-37 (unit seed). Note in
   `specs/drlg/levels.md` 9.4.
2. Object init 13 (class 61, chain 4 link): `ObjectWorld::quest_link` is now
   answered by the lent quest control (`QuestObjectHost::object_link`,
   `wiring/economy/quest_objects.rs`); it stayed `false`, so the object went to
   mode 2 where 1.14d keeps 0 (wp-2 frame 401, also `combat-pop-stony-field`
   frame 5). Note in `specs/world/objects-2.md` 17.

## Open
- gen-wp-28 (size M): level 106 population. All 1313 game-seed draws before the
  end of frame 21 match; 1.14d then takes 2 more density steps (site
  `0x54ed96`, the per-cell loop of `0x0054EC90`) after d2rs's last room (room 13,
  coordinate list 13 rects, 140 tries). 403/404 (trapped souls) appear in 1.14d
  between pack members. Either a coordinate rect is missing or one more room
  is populated; the grid build (`levels.md` 11.3) reads the same as
  `0x0066CA50`. Needs the 1.14d coordinate lists: queued in `pc1-data.md` Step 4.
  `gen-lvl-106` and `gen-lvl-104` (DIVERGED@21) probably share it.
