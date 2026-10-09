# Handoff: q-diff-skills-2 (`claude/q-diff-skills-2`)

Diff-driven area F2: player skills of the Barbarian, Druid and Assassin,
plus hirelings. Loop: a `.check` in `traces/checks/`, `python3
tools/scenario-diff/scenario_diff.py <check>`, fix the first divergence,
re-run. REC block 770–779 (none used yet).

## Checks

| Check | What | State (2026-10-09) |
|---|---|---|
| `merc-rogue-town-bar` | saved Act I rogue restored at the join, idle in town, 80 frames | **equal** in every compared field (PARTIAL only for the snapshot's own gaps) |
| `bar-battle-orders` | warp to the Blood Moor, Fallen at (+4, 0), right-click Battle Orders at frame 20 | frames 1–4 equal; from frame 5 the Blood Moor monster population differs (area B, q-diff-combat-a1); the player's cast (mode 10, life / mana / stamina maxima) equal |
| `bar-war-cry`, `bar-whirlwind`, `bar-leap-attack`, `dru-*`, `ass-*` | same layout, one skill each | 1.14d recorded; d2rs side waits on the population (below) |

Layout of the skill checks: `at 4 poke warp 2` (arrival at the Den of
Evil entrance tile, 5143, 4263), `at 10 poke spawn 19 @x+4 @y normal`,
`input frame 20; rclick X Y` with X, Y = 544, 336 (directional: the
point (+8, +1), past the Fallen) or 330, 300 (self casts, away from it).
Both keep the click off d2rs' stand-in hover box
(`bridge::combat::hover_at`, which picks a monster up to 4 sub-tiles
down-right of the click and turns the cast into a unit cast, C→S 0x0D).
`rng` and `packets` take no pokes on 1.14d, so the checks run `state`.

## Fixed (each found by a check, first divergence)

1. **Saved hireling never restored** (`merc-rogue-town-bar`): the play
   host never turned on `ActionHooks::hireling_calls`; the restore had
   no lent monster world (its monster init, 9 seed steps, was skipped);
   the join follow ran a tick late. Now: queue on, the restore drained
   right after the load's items and corpses with the world lent, and
   `WorldHost::session_work` runs the hireling calls after a session
   message. `d2s-tool --merc ID,NAME,SEED,EXP` writes the block.
   Test `app_hireling_restore`.
2. **Passive skills of a loaded save not applied** (`bar-battle-orders`
   frame 2: stamina 121 vs 502 with Increased Stamina 20): the load's
   assign turns the passive states on and fills their lists
   (`skill_events::passive_refresh_all`). Test `app_passive_load`.
3. **Room presets in the wrong order** (`bar-battle-orders` frame 4):
   objects, monsters, then warp tiles; 1.14d walks objects and tiles in
   list order first (`drlg/rooms.md` §6). `View::spawn_preset_units`.

## Open

- Blood Moor population from frame 5 (monsters 8, 9 class 63 placed at
  other points, then the classes differ): area B. Asked q-diff-combat-a1
  (2026-10-09). The skill checks compare only the player and the Fallen
  once it is equal; until then `state_diff.py ... --types 0` shows the
  player.
- `scenario-diff.md` §3 r8.2 says the headless click has no hover pick;
  the code falls back to `combat::hover_at` (other areas' attack checks
  rely on it). Not changed.
