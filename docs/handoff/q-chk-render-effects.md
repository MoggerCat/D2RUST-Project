# q-chk-render-effects: effect rendering against 1.14d (T4)

Cloud session, 2026-10-09/10, branch `claude/q-chk-render-effects`. REC-1545 used (the FX group definitions); no provisional choices.

## Done
- 20 effect groups in `tools/cloud-game/scene_defs.py` (`FX` table; one level-30 character per skill, `tools/cloud-game/prepare_effect_chars.sh`, names `Fx<skill id>`): Sorceress Ice Bolt, Fire Ball, Lightning, Charged Bolt, Frost Nova, Nova, Frozen Armor (state); Necromancer Teeth, Bone Spear, Amplify Damage (curse); Paladin Holy Bolt, Might (aura); Barbarian Bash, War Cry, Howl; Druid Firestorm, Molten Boulder; Assassin Fire Trauma, Shock Field; Amazon Inner Sight. Each group: the "fight" click path (waypoint, Cold Plains, the Fallen group), the cast clicked 14 times; 4 scenes (cast, flight, hit, later) = 80 scenes.
- All 80 recorded on 1.14d under Wine and rendered on d2rs with `tools/sidebyside/build.py`; ledger rows `docs/handoff/ledger/q-chk-render-effects.tsv` (80 rows, valid with `tools/coord/ledger.py --check`; only the merged files are stale until the coordinator reruns it).
- Pages: private repo `reports/side-by-side/2026-10-10-q-chk-render-effects/` (one page per group, README with commands).

## Result
All 80 scenes DIVERGED, 14-16% of pixels equal at the scene frame, first differing tick 3. The first difference in every scene is the same and is not an effect: `frame.tsv` row 5 `level`: 1.14d is in Cold Plains (3) at the scene tick, d2rs is still in the Rogue Encampment (1). d2rs does not reach/use the Act I waypoint on this click path (feel rows: the waypoint menu click turns into a walk), the same finding as `q-chk-render-world.md` 1 and 2 (walk-mode / click timing; owner by `owners.tsv`: client action state, `claude/q-fix-client-crash` / `claude/q-fix-input-lock`; click timing: `claude/q-tool-state-diff`). So no effect sprite (missile, overlay, cast animation, hit/death, flippy) was compared with 1.14d at the same place: every effect verdict stays UNVERIFIED.

## Open
- Re-run every group after the waypoint travel is fixed in d2rs (`python3 tools/sidebyside/build.py --out ~/sbs/fx --groups <group> --reuse`; the 1.14d captures are reproducible, ~10 min per group under Wine).
- To compare effects before the world matches: a check that cast in the Rogue Encampment? Skills are refused in town, so no; the alternative is a scene without the waypoint (a character saved in Cold Plains: `d2s-tool --act`/area byte is a town byte only). Needs a spec'd way to start in a field level (poke / `--level-start`): owner scenario-diff.
- The 1.14d draw table names missile/overlay sprites `?` (unresolved file), so a path-level effect comparison needs `facts_render.py` to resolve them (tools/trace-recorder, owner q-tool-state-diff).
- Not covered: lighting (light radius, torches: seen only in `q-chk-render-world` a4), day/night, item-drop flippy (the "fight" group's `a1-cold-plains-drop` exists), Amazon bow skills (need an equipped bow).

## Repro
```sh
export D2_GAME_DIR=$HOME/game
tools/cloud-game/prepare_effect_chars.sh
python3 tools/sidebyside/build.py --out ~/sbs/fx --groups fxfireball --jobs 2
```
Checks run: `python3 tools/coord/ledger.py --check`; no Rust changed.
