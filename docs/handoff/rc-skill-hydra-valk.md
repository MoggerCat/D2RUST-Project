# rc-skill-hydra-valk hand-back (no REC ids used)

Input: the two `q-fix-skills-4cls` checks still DIVERGED. Both causes read
from the 1.14d export (no PC 1 needed); both checks now compare for all 70
frames (PARTIAL only for the known `own` / client gaps), Wine, 2026-10-10.

| Check | Before | After |
|---|---|---|
| sor-hydra | DIVERGED@42 | PARTIAL 70/70 |
| ama-valkyrie | DIVERGED@51 | PARTIAL 70/70 |

Sample: `sor-*`, `ama-*`, `nec-*` (orig-cache): every check 70/70 PARTIAL
(sor-frost-nova-twice 100/100); `items-load-mixed` 60/60 PARTIAL.
EQUAL (no difference) count in that cluster: 2 more than before.

## What changed

- `monsters/ai/mod.rs` `mode_end` (`0x005A8030`, spec `monsters/ai.md` §1.4):
  the neutral request carries the path's target unit (`0x00553540`; none
  when it is the unit itself), else the point (0, 0) (`0x005A7C20` rule 2).
  d2rs passed the unit itself. The Hydras never have a target unit, so
  their path target becomes (0, 0). Tests updated (`mode_end_inline_think`,
  `spl_end_generic_cases`, `think_rhythm_table`).
- `path/walk/velocity.rs` `mode_velocity_as`, `wiring/path/monsters.rs`:
  `0x00623F50` runs on the draw identity `0x00645270` (state 93 `valkyrie`:
  `gfxtype` 2, class 0), so a summoned Valkyrie's velocity base is the
  Amazon's WalkVelocity 6 (6 · 256 · 70 / 100 = 1075), not monstats
  `Velocity` 11 (1971). Spec `sim/pathing.md` §8.1 rule 2; unit test
  `v_velocity_draw_identity`. Dopplezon (63) and Shadow Warrior (119) use
  the same path.
- `d2-client/src/app/state_dump.rs` `overlay_item_places`: only items owned
  by a player; the Valkyrie's seven equipment items (owner none, in the
  inventory model with x 0) had their body-location x overwritten with 0
  (first divergence frame 30 after the fresh recording). `items-load-mixed`
  unchanged.
- Ledger part `ledger/rc-skill-hydra-valk.tsv`; the two PC 1 items removed
  from `pc1-data.md` Step 4 with a note.

## Open

- The `ama-valkyrie` orig-cache entry was stale (save changed on this base):
  Wine re-recorded 1.14d locally (not committed); the cache entry in the
  repo still has the old key, refill with `--fill-cache` if wanted (S).
- `cargo nextest -p d2-client` did not run here (disk full while building
  the test binary); `cargo clippy -p d2-sim -p d2-client --all-targets`
  is clean and the d2-sim suite passes (4738).
