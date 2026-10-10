# rc-shrine-m hand-back

Checks (gen-shrine-*, state channel, orig-cache `traces/orig-cache`, 120 ticks):
- before: 15 clean to f120, 4 DIVERGED at f40 `game.seed` (18, 19, 21, 22).
- after: 16 clean, 3 DIVERGED at f40 `game.seed` (19, 21, 22).

Changed:
- `ShrineWorld` for `ObjectView` now implements `drop_near_player` /
  `drop_potion_near_player` (`0x00582AC0`, objects.md §9.3): code drop at P
  with no source unit, quality 2, quantity := 1
  (`drop_helpers::near_player_drop`). Gem upgrade (18) is EQUAL.
- Spec note in `specs/world/objects.md` §9.3.

Open:
- Missiles (`ShrineWorld::create_missile`, `0x0059FA30` with the parameter
  record of `missiles/missiles.md` §R2.1) are still the no-op default: storm
  (19, 16 missiles), exploding/poison (21, 22, 6 missiles each). The
  game.seed gap at f40 for 19/21/22 is the missile creation's draws. Size M;
  belongs to the missiles area (`tools/coord/route.py`).
- `units_in_range` (storm life loss) and the item flag bit 0 / client item
  message of `0x00582AC0` are also unwired (PROVISIONAL, REC-2095).
- The potion count of 21/22 and the `opm`/`gpm` drops are untested until the
  missile draws line up.
- Pre-existing flake: d2-sim prop_walk_motion chase_a_moving_target fails intermittently on the base too (not touched).
