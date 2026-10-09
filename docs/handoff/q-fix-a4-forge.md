# q-fix-a4-forge: Act IV Hellforge quest (slot 27) playthrough

Branch `claude/q-fix-a4-forge`. REC ids reserved REC-1670..1674; none used.

## Result

`traces/playthrough/act4-forge.play`: **6/6** (was 4/6, blocker
`forge-smashed`). Command:
`python3 tools/playthrough/playthrough.py traces/playthrough/act4-forge.play --build`
(run without `--build` on the already-built binary: 6/6, 27.1 set at f210,
27.0 set at f340).

## Why 27.1 was never set

Not a d2rs bug. The `forge-smashed` and `forge-done` milestones carried no
input at all (their notes said "no headless input for it yet"), so nothing
ever operated the forge. The quest code (`world/quests/act4/q3.rs`,
`wiring/economy/quest_objects.rs` events 48/49) works as
`specs/world/quests-act4.md` §4.6 describes once driven:

- `operate` on the forge with `mss ` in the pack: mode 0 -> 1 -> 2
  (f80 -> f101).
- the hammer: `item hfh @x+2 @y`, `msg 0x16 4 <guid> 1` (to the cursor; with
  cursor 0 it goes straight into the pack and the wield test fails), then
  `msg 0x1D <guid> 4` (swap onto the weapon hand; the checkpoint's wand
  occupies it, so 0x1A refuses).
- three `operate`s: mode 3, quest 27 = 0x2002 (27.1 + 27.13), gem rounds
  every 20 frames.
- `forge-done`: the same, then `warp 103`, `goto unit 1:246`,
  `talk @1:246 quest:680 close` -> 27.0 set, 27.1 cleared.

Poke-writing notes: `@pN` does not resolve for a `goto` record (only
creating pokes), use `@2:376` after the walk has placed the forge (f69+);
an item poke's `@pN` does resolve.

## Check against 1.14d

`traces/checks/a4-hellforge-smash.check` (ScnAm4, quests 25.0 27.2 27.3,
seed 1234, 330 frames, state channel; `mss ` and `hfh ` put on the ground
by `item` pokes, so the save needs no item support). Verdict
**DIVERGED (122249 differences), not a forge verdict**: the first
differences are River of Flame monsters (class 300 from frame 26: mode,
position, seed), the game seed at frame 76, and the hammer's rolled stats
at frame 160 (item seed). The forge, quest flags and gem drops sit behind
that noise; the quest flags are not in the diff (`q` is one-side-only) and
no per-unit filter was run. So the 1.14d comparison of the smash itself is
**unverified**. Next: filter the diff to `ut 2 cl 376` and the dropped
items, or settle the River of Flame population first (separate owner).
The check file is a draft; do not count it as a pass.

## Found, not fixed

- Spec `quests.md` §9.2 branch 2: deleting an equipped quest item takes it
  off the body (mode 4, detached). d2rs leaves the smashed `hfh ` equipped
  (mode 1 in the snapshot after the third hit). Unchecked against 1.14d;
  `quest_host.rs` `delete_item` -> `find_on_host` is where to look.
- `operate` out of reach (`interact-operate-stash.check` note): d2rs does not
  start the walk; the play runs avoid it by standing at the forge.
