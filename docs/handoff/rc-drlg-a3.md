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

Regression (a3-warp*, 27 checks, after): 22 rng MATCH of 27; state/rng
DIVERGED only in l78, l94, l97, l98, l99, l102 (l97-l99, l102: combat/AI at
frames 26-87, not seed; l94: monster seed at frame 23).
Open:
- l78 jungle-3 (S): same frame-21 seed symptom but another cause. 1.14d
  creates 2 monsters of class 249 (guid 7, 8; draws `0x552e31`, `0x573a03`,
  `0x573f8f` after the pick draws `0x5b2737`); d2rs creates none, so its
  density-roll chain continues instead. Owner: monster population
  (specs/monsters/population.md).
- l94 temple-1 (S): monster 1:24 seed at frame 23, extra d2rs draw; not looked at.
- The 6 Nightmare/Hell a3 non-bm checks need the rc-difficulty check files;
  not run here, expected to follow l76 (same cause).
- gen-lvl-97..99 and l102: combat/AI frames 26-87, see above.
orig-cache for l76 recorded but not committed (rule: no recordings).
