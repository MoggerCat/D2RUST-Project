# rc-mon-hit-after hand-back

Checks (`suite.py --checks-dir traces/checks/gen --no-playthrough`, orig-cache):

| check | before | after |
|---|---|---|
| gen-mon-295 | state 117/150 DIVERGED@71 (player sp 64 vs 128), rng MATCH | state 150/150 PARTIAL, rng MATCH 133/133 |
| gen-mon-132 | state 124/150 DIVERGED@125 (monster hp 0 vs 35328) | unchanged |

EQUAL (ledger part): monster.baboon6 DIVERGED -> EQUAL.

Cause (fixed): the cold list (0x0057AF80) sets remove callback 0x0057AD80
(state off, shatter 107 off when alive, then anim refresh 0x00623F50).
d2rs gave the list no callback and the expiry walk never refreshed the
anim rate, so the player kept the chilled sp 64 after the cold ended.
Changes (d2-sim): `COLD` callback id; `wiring/action/combat.rs`
create_state_list sets it for state 11; `wiring/action/units.rs`
lists_expired runs the anim refresh (and velocity) for the default, cold,
shrine callbacks, and shatter off for a living unit. Spec:
combat/damage.md §5.6 step 5b. clippy, d2-sim nextest 4776/4776 pass.

Open:
- The "monster dies / hp 0 vs 1.14d alive" and "player sp" rows were NOT
  one cause: gen-mon-132 f125 (monster hp 0 vs 35328) is a separate
  damage-too-high or missed-hit divergence in the 0x0057B420 family. Not
  read. Size M. The other 6 monster-hp checks and 6 other sp checks of
  rc-gen-mon-causes.tsv were not re-run (only 295 and 132); rerun them.
- The default callback's liveness test (0x005541B0 / 0x0063A4A0) and the
  0x00646F20 / 0x00575900 steps are still not in lists_expired. Size S.
