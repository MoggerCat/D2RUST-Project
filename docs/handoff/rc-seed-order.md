# Hand-back: rc-seed-order

Branch `claude/rc-seed-order` (from `claude/specs-staging-7` 97f27b87).
One root cause, fixed and verified.

## Checks (traces/checks/gen, rng + state channels)

| check | before | after |
|---|---|---|
| gen-mon-540 ancientbarb1 | rng DIVERGED f29 (game draw #1 missing, 1.14d site 0x552e31), state DIVERGED f30 | rng MATCH 133/133, state PARTIAL 150/150 |
| gen-mon-541 ancientbarb2 | ledger DIVERGED@29 (same site) | rng MATCH, state PARTIAL 150/150 |
| gen-mon-542 ancientbarb3 | ledger DIVERGED@29 (same site) | rng MATCH, state PARTIAL 150/150 |
| gen-mon-11 bighead2 | ledger DIVERGED@31; re-run on 97f27b87: rng MATCH, state PARTIAL | unchanged (already fixed upstream) |

EQUAL in the cluster: 0 -> 4 (ledger total 883 -> 887).

## Cause and fix

1.14d `0x005B1C50` (ancient barbarian equipment, `init.md` §14.3) creates
4 items through `0x00573B20`; each item allocation `0x00555230` steps the
game seed for the new unit's seed (`0x00552DF0`, the draw at 0x552e31).
d2rs implemented the step (`monsters/init/create.rs::ancient_equip`), but
the real `InitHost` (`wiring/worldgen/init_units.rs`) left
`create_boss_item` and `item_tier_code` at their no-op defaults, so no
item and no draw was made. They now route to the monequip helper
(quality 4) and to `wiring::economy::tier_code` (items row `ubercode` /
`ultracode` on difficulty 1 / 2; unit test added).

## Open (not touched)

- bighead3/4 (gen-mon-12/13): likely fixed with bighead2; not re-run.
- a2-warp-arcane, a5-warp-wsk, inv.item-use, boss.mephisto,
  boss.radament (and boss.summoner, clawviper7/10, cr-lancer6-8, which share
  the seed-order note): not re-run. They are separate causes (M each). The
  cr-lancer rows name a different site (0x56e132, frame 112).
- Ancient equipment on difficulty 1/2 has no check yet (unverified).
