# rc-gen-skill hand-back

Base: staging-7 + integ-r16 (merged). gen-skill-* (210 checks), state channel.

## Checks
- Before: 14315/14630 ticks equal; PARTIAL 199, DIVERGED 10, ERROR 1 (gen-skill-ama-12: `data-tool variant build` fails).
- After (cause 1 fixed): ama-32 and ass-268 now PARTIAL (all 70 frames equal); DIVERGED 8, PARTIAL 201.

## Changed
- Cause: `own` of a summon's equipment item (Valkyrie, Shadow Master, ...) was absent; 1.14d reports the wearing monster (REC-2155, PROVISIONAL: monsters still have no inventory model).
- `d2-sim/src/debug/state.rs` `owner()`: item -> holder from `monster_equip`; `d2-client state_dump.rs` keeps it when the inventory model has no holder. Spec note in `specs/tools/state-snapshot.md` §2.

## Open (first differences)
- ass-279 (S): frame 48 monster class 418 mode 7 vs 1 (later divergence, now first).
- ass-257 (S): frame 28 game seed differs.
- bar-155 (S): frame 28 player stat st 133196 vs 128550.
- dru-222/231/241 (3 checks, S): frame 67 monster target x off by 1 (5143 vs 5144).
- dru-243: frame 20 player mode 5 vs 10.
- nec-93: frame 32 missile ty off by 1.
- ama-12: variant build error (check setup).
