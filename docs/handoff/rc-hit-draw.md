# rc-hit-draw hand-back

Cluster: gen-mon-295, 304, 441-443, 692, 693 (`suite.py --checks-dir
traces/checks/gen --orig-cache traces/orig-cache`, base specs-staging-7 + integ-r19).

| | equal ticks (state+rng) | verdicts |
|---|---|---|
| before | 1551 / 2053 | MATCH 1, DIVERGED 13 |
| after | 1789 / 2044 | MATCH 3, PARTIAL 2, DIVERGED 9 |

Root cause (fixed): the real game world (`View`, `wiring/action/missiles.rs`)
left the missile state-list seams at their defaults (`states_count` 0,
`new_state_list` false). Server-hit 19 (Groper/Strangler spider,
`0x005AB110`) therefore returned 1, never 3, so the spider hit dealt no
damage: no rolls `0x5A89F8`/`0x5A898E`, no owner crit `0x5A55BA`, player
not in GH (m 4). View now implements `states_count`, `state_list_expiry`,
`new_state_list` (flags 2, owner GUID, state, callback `0x0056E900`,
attach, state on + update), `aura_fill` (`0x005C6CC0` on the existing
list), `mark_state_changed`, `set_state_list_expiry`. Test
`missile_state_list_seams_use_the_real_stat_lists`.

Also: monster crit `0x005A5560` overlay polarity. 1.14d overlays 54 only
when `0x00554650` returns 0 (nibble already taken; `0x005A5636 jne`);
d2rs did the opposite. Fixed in code, `combat/damage.md` §3.1 step 13 and
`fill_monster_drains_and_crit`.

Results: 692/693 state 150/150, rng MATCH. 304 first divergence 98 -> 131.

Open:
- gen-mon-304 f131: extra d2rs game-seed draw (`crates/d2-sim/src/uni…`
  site), 1.14d none. Not read. Size M.
- gen-mon-441-443 f90 (also rc-siegebeast open item): the imp fireball
  (missile 404, server-hit 1 `area_damage`) finds no units.
  `Pending::missile_area_units` has no provider in `LocalSeams`
  (d2-client) and its signature `(&mut self, &Game, ..)` can't reach the
  world; it needs the `(h, sim)` hook form routed to the skill scan
  `scan_unit` (`skills/bodies.md` §2.12) on `UseView`, like
  `missile_summon_class`. Touches d2-sim pending.rs, skill_events,
  d2-client single_player.rs, test-fixtures. Size M.
- gen-mon-295 f71 player sp 64 vs 128 (baboon6 cold A1): not worked. Size M.
- View still leaves other MissileBodies seams at defaults (apply_state,
  overlay, unit_mode/set_unit_mode, shout_state, terror, create_monster,
  spawn_monster, ...): any body using them is silently inert. Size L.
