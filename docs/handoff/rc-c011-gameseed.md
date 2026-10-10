# rc-c011-gameseed: C011 "state game seed" (causes-99.tsv)

Branch `claude/rc-c011-gameseed` (from `claude/integ-r23`). REC ids
REC-3330..3349 (none used). Ledger part `docs/handoff/ledger/rc-c011-gameseed.tsv`.

## Cause 1: three missing / misplaced game-seed draws (pushed)

| Draw site (1.14d) | d2rs gap | Fix | Checks |
|---|---|---|---|
| `0x005A03A0` superunique hcIdx in the drop TC choice (`treasure.md` §3.2) | `Pending::superunique` default None: every superunique dropped from monstats column 3 (Act 5 Uitem C, Rare 800) instead of its superuniques TC (Act 5 Super Cx, Rare 972): item count and quality rolls differ, so the item-seed game draws differ | `ActionHooks::superunique` reads the monster data (type flag 0x02 → +0x26) | a5-su-anodized-elite, -bonesaw-breaker, -dac-farren, -magma-torquer, -pindleskin, -vinvear-molech: frame 16 → 36 |
| `0x005A4390` mode-3 umods from the damage apply of a umod area hit (`0x0057E090`, fire-enchanted death) | the monster world is out during the mode-2 callback: the nested dispatch fell back to the no-op seam, curse (umod 7, one unit draw) never ran | `umod_missile_hit` re-lends the world, as `umod_set_mode` does | a5-su-anodized-elite frame 20 → 36 |
| `0x006439F0` monster skill entry in Armageddon's state function `0x005C8520` (`bodies-4.md` §4.9) | a monster has no d2-sim skill list: the lookup answered none, the state function ended, no missile, no item seed | `UseView::find_entry` falls back to `ActionHooks::monster_entry_of` (init / summon entries) | gen-boss-333 frame 71 → 95 |

gen-mon-475 / 476 / 478 (succubuswitch, listed DIVERGED@117/116/114 game
seed) are already clean on this base: state 150/150 no difference, rng
MATCH; PARTIAL only through the check's `ignore q seed` (not REC-2055
promotable), rows set NO-CHECK with that reason.

EQUAL before → after: 3116 → 3116 (the moved checks now first differ on
other causes: a5-su-* frame 36 monster 1:18 class 529 hp, gen-boss-333
frame 95 unit seed of 1:8).

## Open (next causes, sizes)

- gen-ai-npcoutoftown (M): Cain's NpcOutOfTown portal quest calls
  `0x005944B0` / `0x005943B0` / `0x00594450` (ACT1Q4, object 189 at the
  stored portal coordinates, chain-4 extra +0x67, +0x95, +0x98, +0x9C,
  +0xA0) are unimplemented seams (`Pending` defaults): 1.14d spawns the
  portal object at frame 78, d2rs never.
- gen-ai-sandmaggotqueen (S?): frame 31 d2rs draws a missile path step
  (`path/walk/missile.rs:122`) on missile 3:1 that 1.14d does not.
- gen-boss-707, gen-su-37, gen-lvl-55/69/104/106, dru-*, ass-blade-sentinel,
  milestone-izual: not yet traced.
