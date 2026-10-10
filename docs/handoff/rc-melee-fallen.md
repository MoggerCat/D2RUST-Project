# rc-melee-fallen hand-back (REC-2140)

Branch claude/rc-melee-fallen (on staging-7 + integ-r16).

Checks (state channel, 1.14d via orig-cache): before r16 re-run
combat-melee-fallen 45/150, -msg 39/260 (first diff: frame 46 seeds).
After: 150/150 and 260/260; also combat-kill-fallen 146/146,
combat-fallen-hits-player 200/200, combat-elements 226/226,
combat-champion-pack 176/176. rng channel of combat-melee-fallen: MATCH.

Shared cause (one kill hit, frame 46), three missing seams in the client's
`LocalSeams` (they returned the `Pending` defaults):
1. `str_dex_bonus`: weapon StrBonus/DexBonus (damage range 1280 -> 1664).
2. `item_has_durability`: the attacker's durability draw (4th player draw).
3. `composit_shield`: a monster's block roll (fallen carries sml/buc
   shield choice) was never drawn, shifting the TC walk by one draw and
   dropping an item 1.14d did not.
Data: `InvItemRec` gains strbonus/dexbonus/durability/nodurability;
`UnitLooks.shield_choices` from monstats2 SH choices + compcode + item
type 2 (0x006225F0). `weapons::sync` fills `LocalSeams.shielded`.
Spec notes: combat/hit.md §5, combat/damage.md §9.

PROVISIONAL (REC-2140): item_has_durability uses the row test only (not
max-durability / indestructible stats); `durability_loss` remains a no-op
(a loss needs r < 4%; not exercised by these checks).

Open:
- combat-melee-fallen packets 142/150: s2c 0x07 MapReveal missing at
  frame 3 (not the melee path; belongs to the map/level owner), ~8 frames.
- tools/coverage.py --check fails on crates/d2-sim/src/debug/state/tests.rs:409
  ('§2 `own`' malformed rule): present on the base, not mine.
- Full d2-client nextest not confirmed (disk limit hit mid-session); d2-sim
  combat tests 215/215 pass, clippy -D warnings clean.
