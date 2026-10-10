# rc-render-ui: cursor start position (REC-2175)

Branch `claude/rc-render-ui` (staging-7 + integ-r16). Scenes via `tools/sidebyside/build.py --groups panels,act4out,act5out` (pages local, none pushed).

## Checks before / after
- EQUAL scenes: 0 -> 0. All stay DIVERGED (first diff tick 3 everywhere).
- Scene match %: inventory 99.35 -> 99.35, character 98.89 -> 98.89, skilltree 97.82, automap 93.99, esc 98.16, cube 99.94 -> 99.96, belt 98.68 (was 98.71), a4 outdoor 63.66, a5 outdoor 78.53 (a4/a5 not re-run after the fix).

## Changed
- `ui/original.rs`: the cursor starts at (320, 240), not the screen centre. 1.14d init `0x004680B0` reads the display size before `0x0044BA20` sets 800x600 (caller `0x0044F360` runs first), so W x H = 640 x 480 (measured: capture cursor 320,240 at 800x600). Spec `panels-3.md` §23 r3 updated.

## Open
- Shared tick-3 difference is not only the cursor: ~4800 px of animation phase (torches, player/NPC idle frames, fire) differ on frame 2 of every town scene; cursor was one blob of it. Next step: compare sprite frame numbers at tick 3 in `sprites.tsv` (S-M).
- Sounds: call 0 is 1.14d id 0 unit 0:1 vs d2rs id 6, every scene.
- a4 extra TrappedSoul class 380 and a5 arrival dir/frame: not touched (d2-sim / client world owners, see rc-render-world.md).
- Not run: nextest, coverage, spec_index, ledger checks; no ledger part written; no coordinator message sent.
