# rc-rng-player-draws hand-back

Task: 1.14d draws on the player at 0x57b0fd (3) and 0x57cbc0 (2), on a
minion at 0x5e2cda (3). One cause? **Not entirely: one cause for most.**

## Cause (fixed, REC-2660)
`Pending::missile_area_units` returned no units, so every missile area
body (`area_damage` 0x0056BAD0: fireball, glacial spike, ...) hit
nothing. The skipped hit carries the monster crit (0x005A5560, site
0x5a55ba), the cold shatter draw (0x0057AF80, 0x57b0fd) and the get-hit
test (0x0057CB00, 0x57cbc0): all downstream of the missing hit.
Fix: the hook is now `(h, sim)` like the other missile hooks, routed in
`LocalSeams` to `skill_events::missile_area_units` (UseView + `scan_unit`).
`MissileBodies::area_units` / `scan_units` take `&mut Game`.

## Checks (gen-mon, rng + state, orig-cache)
- Before: 441, 618(already MATCH on this base), 645 DIVERGED/MATCH mix:
  645 rng DIVERGED@52, 441 DIVERGED@90 (0x5a55ba), 442/443/619/304/654 as
  the triage rows. After: 441, 442, 443, 618, 619, 645, 304, 654 all
  rng MATCH, state 150/150 (PARTIAL = unmeasured fields).
- 0x5e2cda (441) was the same cause: its remaining divergence was the
  missing 0x5a55ba hit; the possess init itself already matched.

## Open
- gen-mon-466 (suicide minion explosion, frame 59): 1.14d draws 0x5a55ba +
  0x57b0fd on the player after the minion's explosion; d2rs makes no hit.
  Not a missile: the monster's own explosion/attack hit. M.
- gen-mon-77 (clawviper5, frame 34): 1.14d chain 0x57b5f7, 0x57a8ba,
  0x57bc47, 0x5a55ba, 0x57b0fd on the player; d2rs makes no draw (player
  mode 19 vs 5, charge hit). Same as rc-mon-mode's charge note. M.
- Gather-then-hit order of the scan: PROVISIONAL (REC-2660), no case.

Gates: fmt, clippy d2-sim d2-client, nextest d2-sim missile (292 pass),
coverage, spec_index, ledger --check clean. Ledger: 7 rows to EQUAL.
