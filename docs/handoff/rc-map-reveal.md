# rc-map-reveal hand-back (REC-2290)

Branch claude/rc-map-reveal (staging-7 + integ-r18).

## Cause
The missing S->C 0x07 at frame 3 is not missing: d2rs sent it one frame late.
1.14d sends the destination room's 0x07 inside the warp placement
(`0x0053AEC0` -> `0x00554EA0` rule 6, builder `0x0053BC50`), and its poke hook
runs every poke at tick end (before the frame's flush), so it leaves in the
same frame. `state-dump` ran a `warp` between frames (poke.md §5 rule 5 covered
only operate / talk / goto), so the 0x07 left in the next frame's flush.
Fix: `pokes::split_tick_end` also takes `Directive::Warp` (crates/d2-client/src/app/poke.rs).
Spec: specs/tools/poke.md §5 rule 5. No d2-server/d2-sim change was needed
(the join and placement 0x07 code was already right).

## Checks (packets channel, 1.14d via orig-cache; equal ticks)
- combat-melee-fallen: 147/150 -> 149/150 (state 150/150 unchanged); -msg state 260/260.
- a2-quest-arcane 108 -> 110/114; a2-quest-tombs 130 -> 132/150; a2-super-coldworm 92 -> 93/120;
  a2-wp-42 48 -> 49/50; a2-wp-74 6 -> 7/50; bar-whirlwind-unit 62 -> 64/70.
- State channel equal counts: identical before and after on all of these.

## Open
- combat-melee-fallen packets frame 46: s2c 0x69 MonsterState bytes[6] (1.14d 0 vs d2rs 0x17) plus 1.14d
  has one more record in the frame. Not a map-reveal cause. Size S-M.
- a2-super-coldworm / a2-wp-42 frame 4: 1.14d sends 0x0A RemoveUnit first, d2rs 0x07
  (act-change / leave-room message order, intents-events §7.8 rules 2-3). Size S.
- a2-wp-74 frame 4: 1.14d 0x5D vs d2rs 0x07; its state also diverges at frame 9 (game seed). Size M.
- bar-whirlwind-unit frame 2: player AddUnit 0xAA 157 bytes vs 12 (same as rc-packets-s2c open item). Size M.
- Not run: `cargo nextest run -p d2-client` (disk full while building the test binary; fmt, clippy -D warnings, coverage, spec_index and ledger checks pass).
