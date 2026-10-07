# Coverage pass: specs/monsters/*.md

Branch claude/cov-missiles-monsters. Tests run with `cargo test -p d2-sim --lib monsters` (nextest not installed in the cloud box). Clippy `-D warnings` clean, `tools/coverage.py --check` 0 errors.

## Covered, per spec (rules newly claimed)

| Spec | Newly covered | Still uncovered |
|---|---|---|
| umod-init-bodies | 23 of 26 | §2 r2, §4 r2, §4 r3 |
| umod-callbacks | 12 | 13 (below) |
| init | 7 | 9 (below) |
| population | 1 (§11.5 r6) | 7 |
| ai | 21 | 5 |
| ai-bodies | 3 | §9.12 text (only "1.14d-confirmed") |
| ai-bodies-3 / -4 | 0 | edge r6 / edge r4 |
| ai-bodies-6 | 47 (now 100%) | none |
| ai-bodies-7 | 27 | §27 r3, r5, r7-r11 (ShadowMaster) |

## Code fixes and new code found by tests

- `monsters/init/umods.rs`: elemental umods and teleport now return for a unit that is not a monster (umod-init-bodies §2 r1, §4 r1). `add()` skips a zero add (stat-lists §5 r3).
- `wiring/worldgen/init_units.rs`: `set_ai_flag` implemented on the world host, so umod 26 sets AI flag 0x20 (it was a no-op seam).
- `monsters/init/find.rs`: `room_rejected` implements the 0x0065A710 overlap test, including the negative-radius case (umod-callbacks §3.1 l3 r2).
- New modules, tested against fakes only, not yet wired to the world host:
  - `ai/skill_check.rs`: `0x005FD470` (ai.md §7.4).
  - `ai/forced.rs`: forced targets `0x005DD610` (ai.md §5.1).
  - `ai/scans.rs`: scan table and modes (ai.md §5.4).
  - The AI host methods (`ai_skill_check`, `forced_target`, secondary/scan seams) still use their defaults in the real world.
- `units/gap_tests.rs` Probe gained `monster_umods` logging (umod mode 0/1 sites, umod-callbacks §2 r2).

## Left, with reason

- umod-init-bodies §2 r2 (null game) is unreachable. §4 r2/r3: `give_skill` (assign skill 184, set mode 4) has no implementation on the world host, only the call is checked.
- umod-callbacks: §1 r3/r5 are host-side conventions (position/owner/dead lookups). §2 r5 (mode 4, defender reaction) is wired nowhere; it belongs to the damage reaction (`Pending::reaction`). §2 text is prose. §28.*: client crate (Bevy) not built here; §28.3 client burst/ring not implemented. Edge r12-r14 are client hooks.
- init: §1 (entry-point index), §25.1 (x86 register conventions of tool calls) and §25.2 r1/r3 are tool-side, with no d2rs code. §4.1 r1-r3 (monster follow-ups after add: `0x005735A0`, think restart/cleanup, path settings) are marked PROVISIONAL in `wiring/path/units.rs` and not run. Wiring them changes every spawn's timers, so it needs its own task. Edge r13 (hover pointer) is not modelled.
- population: §1 r2 (the callers: portal partner, portal creation, A2Q6 are not wired; `tick::populate_room` itself is tested). §1 r3 is a recording statement. §14 r2-r6 are notes and negative rules with nothing to assert.
- ai: §2.1 r4 (bad code pointer is fatal; not modelled), §3.3 text and §5.1 text (call-site prose), §5.3 (secondary-target/alternative helper details underspecified), edge r13 (not observable: halved D is always >= 12).
- ai-bodies-3 edge r6, ai-bodies-4 edge r4: crash/unreachable paths; ai-bodies §9.12 text.
- ai-bodies-7 §27 ShadowMaster r3, r5, r7-r11 (large body, not reached in time).
