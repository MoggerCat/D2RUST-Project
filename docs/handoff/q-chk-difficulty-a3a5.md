# q-chk-difficulty-a3a5: hand-back (2026-10-09, branch `claude/q-chk-difficulty-a3a5`)

Nightmare / Hell for Acts III–V: monster stats and drops (scenario-diff against 1.14d)
and the class × difficulty matrix cells (d2rs only).

## Done

- **Generator** `tools/chk-difficulty/gen_diff.py` writes 36 checks `traces/checks/diff-a{3,4,5}-{nm,hell}-{normal,champion,unique}[-bm].check`:
  the act's save at that difficulty (lower difficulties' quest bits as the matrix saves), the act-native `nmon1`
  monster (Act III baboon3 / class 50, Act IV willowisp3 / 120, Act V snowyeti1 / 446) spawned normal, champion (`umod 16`)
  or unique (`umod 19`), a Fire Bolt volley, channels `state items`. `-bm`: Blood Moor under the new variant
  `traces/variants/blood-moor-empty-nh` (MonDen, MonDen(N), MonDen(H), MonWndr = 0; the Normal-only variant `blood-moor-empty`
  leaves Nightmare / Hell populated — found by the first run); without `-bm`: warp to the act level (level generation included).
- **Tool blocker found and routed**: the 1.14d side always started in Normal (row `q-tool-autostart-difficulty`,
  fixed by that session: autostart writes config +0x210). All 36 checks ran on 1.14d under Wine; the stored 1.14d sides are in
  `traces/orig-cache/diff-*` (reuse: `scenario_diff.py <check> --orig-cache`).

## Verdicts (36 checks, all DIVERGED; first divergence per check in `traces/checks/` run output, summarised here)

| What | Result |
|---|---|
| Spawned monster record (level, hp, hpx, speed, mode …) NM and Hell, Acts III–V, normal / champion / unique | **EQUAL to 1.14d from the spawn (frame 30) until the first later event** in all 18 `-bm` checks (levels 36 / 38 / 39 in NM, 67 / 69 / 70 in Hell for Blood Moor; hp scaling equal). The difficulty scaling of stats is not where the checks diverge. |
| Frame 60–62, Acts III/V normal + champion | `hp`: 1.14d lower than d2rs by one Fire Bolt hit (e.g. a5 NM 257861 vs 270592); a3 NM normal: monster `s` seed. The poked Fire Bolt does not damage the monster in d2rs (Normal control `diff-ctl` diverges the same way: `m` 3 vs 1). Route: missiles / skills (`q-diff-skills-2`, `q-fix-class-rows`). |
| Frame 60–61, Acts III/V unique | `player fc` 1.14d 256 vs d2rs 0 (player facing after the missile poke). Same family, same owner. |
| Act IV (willowisp3) normal / champion | `m` (mode) 1.14d 2 / 7 vs d2rs 1 at frames 53–74: AI action differs (the Mesa monster walks / acts earlier on 1.14d). Route: `q-fix-b-monster-combat` (monster AI). |
| Act IV unique | frame 40: the **player is dead (m 0) on 1.14d**, alive on d2rs (m 5): the NM/Hell unique willowisp kills the level-1 test character in 10 frames; d2rs' monster does not attack in that time. Route: `q-fix-b-monster-combat` (attack timing / damage). |
| Act-level warp (non-`-bm`) Act III | frame 5 `game seed`: level generation / seed at the warp. Route: `q-prov-recording` (DRLG). Act V non-bm: same hp divergence as bm; Act IV non-bm: as bm. |
| Drops | **not checked**: no kill happens (items channel 0 items both sides; NM/Hell hp 250 k–5 M vs 15 Fire Bolts). Needs a kill route; ledger row `monster.difficulty.drops-a3a5` NO-CHECK. |

Ledger: `docs/handoff/ledger/q-chk-difficulty-a3a5.tsv` (6 act × difficulty rows DIVERGED + the drops row NO-CHECK; `python3 tools/coord/ledger.py --check` has no error for it).

## Matrix (class × difficulty, Acts III–V, d2rs only)

The matrix (`playthrough.py act3/act4/act5.play --class … --difficulty nightmare,hell`) is slow (about 1 h per cell with 4 jobs on 4 cores).
Cells finished before a container restart (act3.play): sor NM 15/15, ama NM 15/15, sor Hell 14/15, ama Hell 14/15
(first blocker both: `council-killed`, stuck at the f1000 deadline, Hell Council member class 345 never dies; player level 83).
The rest is in the section "Matrix, later cells" if it finished (below).

## Open / next

1. Drops: add a kill route to the checks (a `stat` poke setting the monster's life low, then the volley) and compare the items channel.
2. The first-hit divergence (missile hit) hides everything after frame 60 in every check; once the owner fixes it re-run
   `for c in traces/checks/diff-*.check; do python3 tools/scenario-diff/scenario_diff.py $c --orig-cache; done` for resists / damage / AI over the rest of the 400 ticks.
3. NM/Hell Act III–V rows with no check yet: champion/unique mod variety (only umod 16 / 19), several monsters per act, boss rows per difficulty.

## Repro

    python3 tools/chk-difficulty/gen_diff.py          # regenerate (needs the excel view)
    D2_GAME_DIR=$HOME/game python3 tools/scenario-diff/scenario_diff.py traces/checks/diff-a3-nm-normal-bm.check --orig-cache
    D2_GAME_DIR=$HOME/game python3 tools/playthrough/playthrough.py traces/playthrough/act3.play --class sor --difficulty nightmare,hell --jobs 4
