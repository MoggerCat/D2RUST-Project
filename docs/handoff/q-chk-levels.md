# q-chk-levels: hand-back (2026-10-09)

Branch `claude/q-chk-levels`. REC ids: none used.

## Done
- `tools/chk-levels/gen_level_checks.py` (`--selftest`): one level-warp check per row of `levels.txt`
  that has no `poke warp N` check yet, in the `a*-warp-*` pattern (save of the level's act, idle to
  frame 20, `poke warp N`, 160 ticks, channels `state rng`). 108 files `traces/checks/aN-warp-l<id>-<slug>-ama.check`
  (levels not generated already have a hand-written warp check). q-tool-check-gen's generator had not landed; this one covers only the level family.
- All 108 run on both sides under Wine with `tools/scenario-diff/suite.py --filter 'a?-warp-l*'` (3 workers, ~14 min).
  Results: 0 MATCH, 156 DIVERGED, 60 PARTIAL channel runs (216 = 108 x state+rng).
- Ledger part `docs/handoff/ledger/q-chk-levels.tsv` (108 rows; validated with ledger.py: 0 errors). All rows are DIVERGED
  because the rng channel of every check diverges (D1); levels 133-136 (Pandemonium) were not in the integrator rows, so the part adds them.

## Results
- rng, all 108: first divergence frame 2, unit draw #0, 1.14d site 0x573f8f vs d2rs `crates/d2-sim/src/monsters/init/create.rs:265`: **D1**, owner q-fix-real-unit-seed-order.
- state: **60 levels equal for all 160 frames** (rooms, presets, populated units; verdict PARTIAL because the channel has unverified parts): a1/l1, a1/l10, a1/l11, a1/l12, a1/l15, a1/l16, a1/l17, a1/l19, a1/l20, a1/l22, a1/l23, a1/l24, a1/l26, a1/l27, a1/l28, a1/l30, a1/l32, a1/l35, a1/l36, a1/l39, a2/l41, a2/l45, a2/l46, a2/l50, a2/l51, a2/l52, a2/l53, a2/l54, a2/l56, a2/l57, a2/l58, a2/l59, a2/l65, a2/l67, a2/l71, a2/l72, a3/l79, a3/l81, a3/l82, a3/l83, a3/l84, a3/l86, a3/l87, a3/l89, a3/l90, a3/l91, a4/l108, a5/l111, a5/l112, a5/l115, a5/l117, a5/l120, a5/l121, a5/l123, a5/l125, a5/l126, a5/l127, a5/l129, a5/l130, a5/l135.
- state diverged, 48 levels:
  - 33 at frame 21 on the game seed after level generation (D1 consequence, same owner).
  - 3 on a monster unit seed (`a2-warp-l73`, `a3-warp-l102`, `a5-warp-l118`): D1.
  - 5 object class differs at frame 20, 1.14d class 37 vs d2rs 119 (`a1-warp-l5`, `l6`) or 156 (`a2-warp-l42`, `l43`, `l44`): **new, objects/DRLG owner**; unrouted here.
  - 2 monster mode m (`a2-warp-l60` 4 vs 1, `a2-warp-l61` 2 vs 1): D2, NPC/AI owner.
  - 3 monster placement off by 1-3 units: `a1-warp-l33` y 4947 vs 4948, `a2-warp-l68` y 8043 vs 8046, `a2-warp-l70` x 22536 vs 22534 (D4 family, q-scenes-compare / population owner).
  - `a3-warp-l77` monster field fr 5120 vs 0; `a5-warp-l110` monster tx 4324 vs 4321: new, population/AI owner.

## Open
- Re-run after q-fix-seed-order lands: 36 of the 48 state divergences should clear; the rest are listed above.
- The 18 levels with hand-written warp checks keep their existing verdicts (not re-run here).
- `a4` has only 3 generated checks (the rest already have checks).

## Repro
    python3 tools/chk-levels/gen_level_checks.py --selftest
    python3 tools/chk-levels/gen_level_checks.py           # regenerate (--check: fail when stale)
    D2_GAME_DIR=$HOME/game python3 tools/scenario-diff/suite.py --filter 'a?-warp-l*' --no-playthrough --workers 3 --json out.json
