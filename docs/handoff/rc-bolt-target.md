# rc-bolt-target — hand-back

Cause (REC-1990): not the missile target filter. `0x005A8730` / `0x00641CB0`
in d2-sim already match 1.14d. In items-drops-nor-04 the player is killed at
f38-39 by the neighbouring monsters; 1.14d's player death (dead clean-up
`0x0057F330`) calls the dead-body footprint `0x00649F70(P,1)`, replacing the
0x80 player footprint with a 3x3 0x8000 box. d2rs kept 0x80, so the bolt
poked at f40 found a non-zero mask at its first subtile and hit monster 1:21.
With the 0x80 gone 1.14d sees mask 0 there, skips, and the bolt flies on.

Fix: `wiring/path/units.rs` `dead_body_footprint` (split out of
`death_footprint`, no `deadCol` gate), called from the `player_death` hook in
`wiring/action/units.rs`. Spec note in `specs/combat/vitals.md` §4.6 r1.5.

Checks (d2rs side, cached 1.14d):
- items-drops-nor-04: 2/5 -> 4/5 frames equal (first divergence f42 -> f54).
- death-town-ama 120/120, ass-death-sentry 70/70 unchanged.
- combat-*: 14 PARTIAL, 5 DIVERGED (combat-pop-stony-field f5, combat-potion-midfight f3 ...
  both far from any player death); no before-run (it was too slow), not compared.
- gen-missile-*: run was still going at hand-back; no player deaths in them,
  not expected to change. d2-sim nextest 4749 pass, clippy clean.

Open:
- items-drops-nor-04 remaining: scy drop f54 vs f53 and mon 1:19 death packets
  (mode 6 at f48, 8 at f53 in 1.14d; d2rs has none): monster death timing, size S.
- Other player-death clean-up steps of `0x0057F330` (states keep mask, client
  bit) were not checked for presence; only the footprint was settled.
