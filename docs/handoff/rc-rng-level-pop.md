# rc-rng-level-pop hand-back
Branch `claude/rc-rng-level-pop` (from `claude/specs-staging-7`). REC ids: none used.

## Checks (gen-lvl, 92 levels of the "rng: frame 2 ... create.rs:265" rows; 78 skipped)
| | rng MATCH | state all frames equal | equal ticks | EQUAL rows |
|---|---|---|---|---|
| before (re-run, this session) | 80 / 92 | 76 / 92 | 22614 / 24655 | 76 |
| after | 83 / 92 | 79 / 92 | 22992 / 24687 | 79 |

The 93 ledger rows were stale: on re-run, 76 levels were already equal on both
channels. All 92 rows are now settled in `docs/handoff/ledger/rc-rng-level-pop.tsv`.

## Root cause fixed (largest surviving group: lvl 73, 102, 118)

At frames 51, 101 and 151 (every 50), 1.14d rolls `0x0056E0C0` (elemental damage, n = 256 / 5376)
on the monster's own seed. That's the type-8 aura timer's do. d2rs never gave world
monsters their aura: `InitHost::give_aura` (umod 30, `0x005A1650` → `0x0056DEB0` +
`0x005701B0`) and the AI's `add_right_skill` (Duriel §14) were no-op seams.
- New `Pending::monster_right_aura` hook (routed by `UseRest` hosts, e.g.
  `LocalSeams`) → `skill_events::monster_right_aura`: it records the skill's base level
  and runs the existing monster aura select (`summon.rs`, made `pub(super)`).
- `WorldHost::give_aura` and AI `add_right_skill` (aura skills) call it. AI `hand_skill`
  now reads the unit's skill list, so Duriel's add runs only once.
- Specs: `monsters/init.md` §19.5 and `monsters/ai-bodies-2.md` §14 record the observed timer.
- Test: `a_monster_aura_gives_the_skill_and_schedules_the_aura_form`.

## Open (13 rows DIVERGED, grouped by first difference)

- 4 × state-only, frame 21: monster x/y off by 1–3 at placement (lvl 31, 33, 68, 70); rng equal. M.
- 2 × AI extra draw `monsters/ai/bodies.rs:58`, mode 14 vs 1 (lvl 61 f62, 99 f87). M.
- 2 × AI extra draw `monsters/ai/mod.rs:412` (lvl 94 f54 after mode diff f43, 110 f29 after tx diff f23). M.
- 2 × creation order: 0x573f8f vs `init/create.rs:178` (lvl 55), 0x573a03 vs `create.rs:256` (lvl 69). M.
- 2 × population game-seed: extra `population/room.rs:193` (lvl 104), missing 0x54ed96 (lvl 106). M.
- 1 × combat: 0x57b5f7 vs `combat/damage.rs:319`, player hp (lvl 97 f26). S–M.

Notes:
- `tools/coord/route.py` gives the regenerated `fidelity-ledger.{tsv,md}` and my part to
  q-fix-seed-order (generic route); the code and spec files are unrouted.
- One full `d2-sim` nextest run failed `prop_walk_motion::chase_a_moving_target` under load.
  It passed alone (×3) and in a second full run (4755/4755).
