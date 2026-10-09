# q-fix-a4-deseis — the De Seis seal on WingN1 games (2026-10-09)

Task: in the Chaos Sanctum (108) the De Seis seal (394) never opened when the
level seed picks WingN1 (seeds 1-4, 6-9, 11), so Diablo could not be
reached; the act4.play Diablo milestones ran on seed 5 only
(`q-fix-act4-play.md` Open 1).

## Done

- **Cause found, and it is 1.14d's behaviour; no engine change.** On
  WingN1 the boss spot (seal + (-39,+33), seed 1: (7734,5188)) lies in the
  lava room west of the arm. The free-spot search (`quests-helpers.md` §1)
  looks rooms up with `0x00463740`, which only sees active rooms, so it
  finds a spot in the arm room south of the seal, (7740,5200) - (7780,5240),
  only while that room is loaded. The milestone operated the seal 400
  frames after the `goto` had walked through that room, when it had been
  unloaded: the search ended on (7780,5160), a spot whose room is not next
  to the start room, and the seal stayed in mode 0, as the spec says.
- **Measured against 1.14d under Wine** (seed 1, scenario-diff, both sides):
  | check | 1.14d | d2rs |
  |---|---|---|
  | `a4-deseis-seal-early` (operate at f100, right after the walk) | opens; dummy 131 at (7770,5226); De Seis GUID 105 at (7773,5207) | same |
  | `a4-deseis-seal-loaded` (into the south room and back, operate f380) | opens; dummy 131 at (7770,5218); De Seis GUID 105 | same |
  | `a4-deseis-seal-unloaded` (idle at the seal 200 frames, operate f500) | stays in mode 0, no dummy | same |
  The state channel's first divergence in these runs (f36, a Sanctuary
  monster's mode 8 vs 2) is monster AI, outside this task.
- **Milestones** (`traces/playthrough/act4.play`): the Diablo flow now
  walks 33 + 41 sub-tiles south of the De Seis seal and back before
  operating it (what a player coming up the arm does), puts the player on
  each seal before msg 0x13, reaches far-spawned bosses with two `pos`
  hops, and kills with Ice Bolt (the Grand Vizier is fire enchanted, so
  the old Fire Bolt left him at 1 hp) from six sides in turn (a minion can
  block a bolt). `diablo-present` / `diablo-killed` run on seed 1
  (WingN1), `diablo-present-wingn2` on seed 5. The old seed-5 GUIDs no
  longer matched after the integ-r4 merge (`pos 1/147` failed).
- **Seeds 1-11:** `docs/handoff/q-fix-a4-deseis-seeds.py <seed>` runs the
  same flow with the GUIDs resolved per run: Diablo present (f2840; seeds
  6, 7 f2860) and killed (f2909) on all 11 seeds.
- `pc1-data.md` Step 4: the q-fix-act4-play De Seis item is marked
  answered (cloud, Wine); a Windows rerun is optional.

## Open

1. Spec note for the act4 owner (`specs/world/quests-act4.md` §5.4, owner
   per `tools/coord/owners.tsv`, not edited here): the boss-spot room and
   the free-spot lookups see only active rooms, so on WingN1 the seal
   opens only while the arm room south of the seal is loaded (measured,
   checks above). Suggested text: "No spot: on WingN1 the spot lies in a
   lava room; the search reaches the arm's room south of the seal only
   while it is active, so the seal stays shut when that room is unloaded
   (1.14d, `a4-deseis-seal-*.check`)."
2. The milestones still hardcode GUIDs per seed (`goto` results cannot be
   named by `@pI`; q-fix-act4-play Open 2, for q-fix-pt-sweep).

## Repro

```
export D2_GAME_DIR=$HOME/game
python3 tools/playthrough/playthrough.py traces/playthrough/act4.play --build   # 13/13
python3 docs/handoff/q-fix-a4-deseis-seeds.py 1        # any seed 1-11
python3 tools/scenario-diff/scenario_diff.py traces/checks/a4-deseis-seal-unloaded.check   # Wine
```
