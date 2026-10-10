# rc-vampire-hp hand-back

Checks (suite.py, gen, orig-cache; 1.14d re-recorded here):

| check | before (staging) | after |
|---|---|---|
| gen-mon-132, -133, -134, -696 | already state 150/150, rng MATCH | same |
| gen-mon-135 vampire5 | state 134/150 DIVERGED@135 (player m 4 vs 5), rng DIVERGED site 0x57cb95 | state 150/150, rng 149/149 |
| gen-mon-695, -697 (vampire6, 8) | not in the brief | state 150/150, rng MATCH |

The brief's cause ("monster hp 0 vs 35328") no longer reproduces: it was the
cold-list/anim-refresh fix of rc-mon-hit-after (already on staging). The one
remaining vampire divergence was a different cause:

Cause (fixed): the area scan behind missile area damage (srvhit 1 fireball,
`area_damage`, `next_unit`, `scan_unit` of the area bodies) had no provider on
the wired missile view (`Pending::missile_area_units` returned none), so
VampireFireball (class 129) exploded on the player with fire 1409 but hit
nobody (1.14d: player life 11623 -> 10214, get-hit, draw at 0x0057CB00).
Changes (d2-sim): `Pending::missile_area_units` is now a static fn on the
missile `View` (+ `noaura`); `skill_events::missile_area_units` runs the real
`scan_unit` (0x0056B7E0) on the `UseView`; `MissileWorld::area_units` /
`scan_units` take `&mut Game`; d2-client overrides the hook. Spec note:
missiles/missiles.md §R9.6. d2-sim nextest 4776/4776.

Ledger: docs/handoff/ledger/rc-vampire-hp.tsv (7 rows EQUAL).

Open: the area scan is now live for every srvhit/srvdo area body, not only
vampires; regression sample of 36 AI-caster gen-mon checks: 34 MATCH + 35 PARTIAL(100% ticks equal), 0 DIVERGED; gen-mon-714 and -688 rng ERROR = 1.14d recorder timeout (421 s), not re-run.
