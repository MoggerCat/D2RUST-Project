# q-fix-pt-sweep — hand-back (2026-10-09)

Branch `claude/q-fix-pt-sweep`. REC-1080 (missile-passable landing),
REC-1081 (sweep milestones → `goto preset`).

## Done

- **Landing rule (REC-1080).** A landing cell has none of the mask bits
  0x1C0D (player move 0x1C09 + missile-blocking 0x4). `hop` skips
  candidates that fail it; `pos <ref> <x> <y> free` lands on the nearest
  passing cell of the room (plain `pos` stays raw); `goto` uses the mask
  for its hop cell and, after a found placement, places again with exact 1
  when the player's cell still has 0x4. Spec `specs/tools/poke.md` §1,
  §6 r3.2/r3.3 + a test-vector row. Both sides: `d2_sim::poke`
  (`LAND_MASK`, `nearest_free`, `settle_landing`) and
  `tools/trace-recorder/poke.py` (`free_cell`, `mask_at`, `_settle`).
  Tests: `test-fixtures/tests/poke_goto.rs`
  `pos_free_and_hop_land_on_missile_passable_cells`; `poke.py --selftest`
  (803 checks). **The 1.14d side has only run its selftest** (fake DRLG);
  queue: `scenario-diff` a check with `pos @player X Y free` on 1.14d.
- The hop was already ≤16 sub-tiles with a ring (q-tool-state-diff).
- **Playthrough evaluator** was quadratic (`seen`/`dead` rescans; a 244 MB
  state file took >30 min): per-predicate prefix cache, now 1.4 s. A
  reached milestone's state files are deleted (matrix disk use).
- **Sweeps → goto (REC-1081):** act2 summoner-present/-killed, act3
  flayer-jungle-decoy, act4 hellforge/hephasto-present, act4-blockers
  hephasto/hellforge, act5 frozen-anya, nihlathak-present/-killed,
  baal-throne now `poke 20 goto preset <lv> <t:cl>`. Merged
  q-fix-boss-damage (Sewers 0x5 report: `radament-present` reached).
- **Matrix:** all 63 Act III–V cells ran; table + routed blockers in
  `docs/handoff/playability-matrix.md`. `--all` table:
  `docs/handoff/playability.md` (A2 15/15, A3 15/15, A5 13/13, A4 10/12).

## Open (routed, not fixed here)

| Blocker | Cells | Route |
|---|---|---|
| act4 `diablo-present`/`-killed`: Diablo exists only after the 5 seals | 21/21 | `q-a4-endgame` |
| act3 `council-killed` on Hell | 7/7 Hell | `q-fix-boss-damage` |
| act5 `nihlathak-killed` on Nightmare/Hell (not sor, ass) | 10 | `q-fix-boss-damage` |
| act4-blockers `fallen-fire-bolt-105`, `forge-smashed` | — | Act IV owners |
| act1 `den-of-evil-done`, classes `cast-then-walk` | — | existing rows |

Not done: the remaining act1/act2 `sweep` milestones were left (they
reach); `coord/route.py` has no rule for `stuck … killed` rows.

## Repro

    D2_GAME_DIR=/home/user/game python3 tools/playthrough/playthrough.py \
      traces/playthrough/act3.play traces/playthrough/act4.play traces/playthrough/act5.play \
      --class all --difficulty all --jobs 3 --json m.json --markdown m.md
    python3 tools/playthrough/playthrough.py --all --json out.json && \
      python3 tools/coord/playtable.py --from-json out.json
    python3 tools/trace-recorder/poke.py --selftest
    cargo test -p test-fixtures --test poke_goto
