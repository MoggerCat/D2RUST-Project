# rc-drlg-a3: Act III warp seed (level 76)

Before -> after (state/rng EQUAL ticks):
- a3-warp-l76-jungle-1-ama: state 20/160 -> 160/160 (PARTIAL), rng DIVERGED -> MATCH.
- a3-warp-l77-jungle-2-ama: unchanged, 160/160, rng MATCH.

Cause: not DRLG. The level generates identically; the divergence was the
four gold piles (class 523) that 1.14d creates at frame 21 from the
gold placeholder objects (init 28, `0x0054F8C0`; drop `0x00559300`). The
client built `DeathDrops` without `ClassPicks`, so `find_code("gld ")`
found nothing: the placeholder's own draws ran, the item-seed step and
the gold-amount draws (`0x557118`) never did, and the game seed drifted.

Changed: `d2-server` `world_data::tables::class_picks(set)`;
`d2-client` single_player sets the picks on the object drop state.
Spec note in specs/world/objects-2.md §17.

Open: the 6 Nightmare/Hell a3 non-bm checks were not run here (they need
the rc-difficulty branch's check files); the same cause should apply.
gen-lvl 75..102: 28 checks, 3 still DIVERGED at frames 26-87 (gen-lvl-97
player hp, -98 missile, -99 monster mode): combat/AI, not seed or items.
orig-cache for l76 recorded but not committed.
