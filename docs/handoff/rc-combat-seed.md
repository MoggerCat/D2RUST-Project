# rc-combat-seed hand-back (2026-10-10)

Branch `claude/rc-combat-seed` (from integ-r23). REC ids 2820-2822.

## Result
`system.combat.*`: 21 DIVERGED rows -> 21 EQUAL (ledger part
`docs/handoff/ledger/rc-combat-seed.tsv`); EQUAL 2080 -> 2101 in the merged ledger.

Checks (Wine 1.14d re-recorded, orig-cache reused):
- The "frame 5 game seed differs" cause is gone (seed-order fixes landed): the
  game seed is equal on every frame of all 17 `combat-*` checks.
- combat-kill-fallen, combat-fallen-hits-player, combat-elements: pokes-only, state
  equal on every frame, PARTIAL only for the RUN_GAPS client gap (REC-2055/2056) -> EQUAL.
- combat-melee-fallen: state equal; packets was DIVERGED (frame 46), now MATCH 150/150 -> EQUAL.
- combat-potion-midfight (not in the ledger rows): packets DIVERGED -> MATCH 150/150.
- Other 14 combat-* checks: state equal, PARTIAL (unchanged, no ledger rows named here).
- gen-mon sample (gen-mon-1x, 7 checks): state equal, rng MATCH.
- sys-intents-moves packets MATCH (kept; an earlier version of the reorder broke it).

## Fixes (three causes found by reading the packet diff, then 1.14d functions)
1. **REC-2820 player weapon hit class** (`0x00623C20` -> `0x0062A180`): a player's
   weapon hit class is the item row's `hit class` byte (+0x13C); d2rs returned 0, so the
   killing hit's class in unit +0xB0 (byte `e` of S->C 0x69) was wrong. `ItemFacts.hit_class`,
   `Weapons::weapon_hit_class`, `LocalSeams::weapon_hit_class` (d2-client). ssd -> 3.
2. **REC-2821 0x69 path end**: (a, b) of the mode message are `0x00648A40/A60`
   (last computed path point, (0, 0) without one), not the path target +0x10/+0x12
   (spec already said so, REC-594; the code read the target). New `ModeInput.path_end`;
   the skill message `0x00597D70` keeps the target. Also **arena kill record**
   (`wiring/action/arena.rs`): a player's kill of a monster raises its record and bit 0x400;
   the client pass sends S->C 0x65 (GUID, count); tick step 6 clears the bit.
3. **REC-2822 player update order**: item messages (0x9D, 0x47, 0x48) precede the
   0xA8 state message and the stat sends (`intents-events.md` §7.3 rule 1). The action
   wiring holds a player's step 5/7 sends (`ActionHooks::player_tail`, only when the player
   has item messages pending, update bit 0, and is not new) until the server's item pass
   ran (`WorldHost::flush_player_tail`).

Specs: `combat/damage.md` §5.1 step 4.4, `sim/intents-events.md` §7.6 rule 5, §7.3.
Tests: arena unit test, mode_message tests (path_end), e2e_night_world and
unit_update expectations gained the 0x65.

## Open
- PROVISIONAL REC-2820: `arena.txt` `MonsterKill` is the constant 1 (single row
  Deathmatch); the table is not loaded into the sim.
- Arena sync: only the client's own player (`0x0053FC20`); other in-game players'
  counts (`0x0053FB90`) and the PvP/death branches of `0x0053F720` not modelled.
- The 0x65 victim "queued for update" (`0x0064C040`) is not done (no observable effect in the checks).
- Pre-existing, not touched: join-stream gaps (a2-wp-* frame 4 0x0a vs 0x07, cube-* 0x3f vs 0x47,
  bar-whirlwind/rng-town-idle 0x23 vs 0x5e, ...), unrouted.
