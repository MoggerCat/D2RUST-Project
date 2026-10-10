# rc-a5-nihlathak-2: act5.play nihlathak-killed

Checks: act5.play 6/13 consecutive (12/13 reached) -> 13/13. No game-code change.

Bisect (first-parent, b6cd4c761 good, integ-r11 bad): the first bad commit is
0b03e942a "Merge q-fix-pt-sweep into staging". Not 4ca669b4: act5 already
failed at 54316e7b7, before the rc-mon-missile merge.

Cause: q-fix-pt-sweep replaced the sweep in `nihlathak-killed` with
`goto preset 124 1:526` and pokes at frame 100. The preset drops the lv 70,
109-hp sorc inside Nihlathak's 30-monster group (class 458); the group hits at
f64 and the player is dead at f75, 25 frames before the pokes. The old script
moved Nihlathak to open ground within 2 frames of finding him.

Fix (script only, `traces/playthrough/act5.play`): the hp/position pokes move
to f50 and Nihlathak goes to open ground (12705,5032) with the bolt from
(12705,5040) at f52 (the pre-sweep geometry). A first try with the relative
`@x+3` geometry failed: `@x` is the player, and the bolt hit an adjacent minion.
Killed at f58.

Open: nothing in game code. Whether 1.14d's group also kills a 109-hp player this
fast is unmeasured (d2rs-only run); a Windows check would settle it (small).
