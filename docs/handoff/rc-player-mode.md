# rc-player-mode (REC-2870..2879)

Cause assigned: "state player field m" (13 checks). Actual root cause: not
the player's mode logic but the missile **area scan** having no provider
in the live game. `Pending::missile_area_units` returned nothing
(`docs/handoff/rc-hit-draw.md` open item), so every area missile body
(server-hit 1, 12, 13 ...) hit nobody. 1.14d's player then enters get-hit /
death (mode 4 / 0) or knockback; d2rs's player never took the hit.

## Result (Wine 1.14d recorded, same tree, 2026-10-10)
EQUAL before 2 of 13 (436, 438 were fixed by rc-damage-draws before this
branch; 437 too) -> after 11 of 13: gen-boss-250, gen-mon-436, 437, 438,
441, 442, 443, 466, 595, 597, 598 (state 150/150, rng MATCH where recorded).

## What changed
My own code fix was identical in effect to REC-2660 (b0711a889, already in
claude/specs-staging-7); on merge I dropped mine and kept that one. This
branch carries only the diagnosis, spec note, ledger part and hand-back.
Re-run on the merged tree: same 11 EQUAL, 544 and 533 DIVERGED.
(Details below describe the equivalent change.)
- `Pending::missile_area_units` is now the `(h, sim)` hook form;
  `skill_events::missile_area_units` runs `scan_unit` (skills/bodies §2.12)
  on `UseView`; d2-client `single_player.rs` routes to it.
- `MissileBodies::area_units` / `scan_units` take `&mut Game`.
- Spec: `specs/missiles/missiles.md` §R9.6 (wiring note).
- Ledger: `docs/handoff/ledger/rc-player-mode.tsv` (11 rows EQUAL).

## Open
- gen-boss-544 f80 (BaalCorpseExplodeExplosion / Baal cold trail,
  classes 589/590) and gen-mon-533 f113 (Recycler Explosion, class 502):
  1.14d's player takes cold/physical damage (shatter draw `0x0057B0FD`,
  monster crit `0x005A55BA`) from an explosion missile in mode 3 on the
  player's cell; d2rs never calls `run_srv_hit` for them (no srv-hit
  index, `explosion` rows skip `damage_stage`). Which 1.14d function does
  the damage is not in `specs/missiles/`; needs a read of the explosion
  rows' srv-do / hit path. Size M. Both stay DIVERGED (player m 0 vs 1).
- Unchanged and pre-existing: gen-boss-242 f59 and gen-boss-267 f93
  (field s, seed order; not this cause).
- `scan_units(.., noaura true)` now has a provider too (same hook).

Regression sample (orig-cache): gen-boss-2*, gen-mon-43*/44*/46*/59*/10-14:
57 state/rng lines equal, only 242 and 267 diverge (as before).
Gates: fmt, clippy -p d2-sim, nextest -p d2-sim, coverage, spec_index,
ledger --check.
