# Handoff: unit-test gaps of the specs implemented tonight — `claude/gaps-night-specs`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of the tenth fold (`claude/fold-handoff-10`); this file stays as the detailed record. Its open questions are in HANDOFF §7 "Tenth set" (PC 1 / PC 2).

Cloud test session, 2026-10-07, task class: tests from specs, medium
(M14). Base: `main` at `674996d`. Repo only, no game files. One subagent
took `formats/d2s.md` (separate files: `crates/d2-formats/src/d2s/tests.rs`).
Not touched (another session splits them): `monsters/ai.md`,
`world/quests.md`, `items/inventory.md`, `skills/bodies-2.md`,
`missiles/bodies.md`. No spec was edited.

## Result

`py tools/coverage.py --summary` total, unit tier: 5,846 → 5,932 of 7,342
units (79.6% → 80.8%); any tier 5,934 → 6,020 (82.0%). 8,250 claims.
Every new or extended claim sits on a `#[test]` that checks the unit's
behavior; no assertion of an existing test was changed or weakened.

| Spec | Units | Unit-covered before | After | Left (reason below) |
|---|---|---|---|---|
| `world/objects.md` | 92 | 87 | 91 | 1 |
| `world/hirelings.md` | 117 | 87 | 104 | 13 |
| `sim/pets.md` | 30 | 30 | 30 | 0 |
| `world/quests-act2.md` | 83 | 79 | 83 | 0 |
| `world/quests-act3.md` | 89 | 85 | 88 | 1 |
| `world/quests-act4.md` | 75 | 67 | 74 | 1 |
| `world/quests-act5.md` | 53 | 47 | 51 | 2 |
| `world/quests-act5-2.md` | 50 | 49 | 49 | 1 |
| `render/lighting.md` | 90 | 81 | 86 | 4 |
| `render/shading.md` | 32 | 29 | 32 | 0 |
| `ui/panels.md` | 96 | 66 | 75 | 20 |
| `audio/environment.md` | 73 | 67 | 72 | 1 |
| `audio/sound-table.md` | 108 | 99 | 103 | 4 |
| `audio/triggers.md` | 129 | 116 | 123 | 6 |
| `client/audio.md` | 10 | 8 | 8 | 2 |
| `formats/d2s.md` | 100 | 59 | 73 | 27 |
| `client/model.md` (§12 asked) | 78 | 63 | 63 | §12 r4 (and §2, §5, §8, §11, §13, not in scope) |

Gate (this branch, whole workspace, `CARGO_INCREMENTAL=0`): `cargo test
--workspace` 5,303 passed, 0 failed, 218 ignored; `cargo clippy
--workspace --all-targets -- -D warnings` clean; `cargo fmt --check`,
`py tools/coverage.py --check` (0 errors) and `py tools/spec_index.py
--check` clean.

## Code changes

No fidelity code changed: nothing tested disagreed with an unambiguous
spec. Test-support changes only:

- `crates/d2-sim/src/world/quests/tests.rs`: the shared quest `Fake` got
  `drop_at_fails` (`drop_item_at` returns false, still logs) for the
  `quests-act4.md` edge case 6 test. Default false, so no existing test
  changes. (This file belongs to the `world/quests.md` tests; the split
  session may see a two-line merge conflict.)
- `crates/d2-client/src/ui/panels/character.rs` test `View` got a
  `language` field (was a constant 0) for the §8 r8 language-6 case.

Environment note: `d2-client` needs `libwayland-dev libasound2-dev
libudev-dev libxkbcommon-dev pkg-config` (CI installs them); a fresh cloud
container lacks them and `cargo test --workspace` fails in `wayland-sys`
until `apt-get install` runs.

## Units left uncovered, by reason

**No code yet (feature not wired or not implemented):**
- `objects.md` §14 r2: `MiscWorld::update_extras` is a no-op hook (sound,
  hover, flags-2 calls not implemented).
- `hirelings.md` §6 r3 (no act-change caller of `classic_act_change` /
  `follow`), §7.1 text, §7.1 r3 (`kill_share` has no production caller;
  the player share is `vitals.md`'s), §8 r1 (death trigger, open question
  8), §10 r8 and edge 8, edge 10 (restore is not wired: `restore_plan` has
  no caller, no item load / life := max after it), §11 r6 (potion use is
  the item-use spec's).
- `quests-act5.md` §1.4: dispatch by objects.txt init / operate number to
  the Act V functions has no caller (only the object-event 7 classes are
  wired).
- `render/lighting.md` §1 r3, §2 r4: `LightMap::build` has no frame-loop
  caller in the client yet, and q (r4) is the caller's.
- `ui/panels.md` §9 r1 (inventory mode word), §9 r7 / §11 r6 (grid
  click intents), §11 r1 / §12 r1 (open paths), §12 r3 (cube grid), §12 r4
  (frame-30 draw and start tick, marked "Partial" in the code), §12 r5,
  §13 r4 (the filled hover rectangle is not drawn, only queried), §14
  r1–r5 (each marked "Partial" by the implementing session: talk / hire
  senders, the Resurrect insert, the menu box, shop grid / buttons, the
  other shop messages), §7 r4 (tool-tip queue `0x00502280`).
- `ui/panels.md` §8 r7: level, experience and next-level values need the
  thousands grouping of `0x00525350`, which the spec does not describe;
  the code leaves them undrawn.
- `audio/triggers.md` §4.1 r1, r5: `mode_sound` / `skill_start` have no
  client mode-set caller yet. `audio/sound-table.md` §1 t2 row4, §10 r1:
  no cache size accounting or limit (code TODO §10 r2); §7 r5: stream
  sample path is a TODO.
- `client/model.md` §12 r4: nearest-free-point fallback is a TODO in
  `bridge/msg/units.rs` (needs the client collision maps).
- `formats/d2s.md`: 27 units (load effects on client / game state, writer
  inputs from live game state, client readers, item reader choice by
  version, meanings not traced); list in the subagent report below.

**Not unit-checkable (live data, original-only or open):**
`hirelings.md` §1.1 r4 (live table counts: game tier), §1.1 r5 (unused
columns), §12 (link table), §13 r7 (links to `npc.md`);
`environment.md` §2 r10 (live rows); `lighting.md` §12 r4, r5 (capture
observations); `sound-table.md` §6.4 r3 (open question 5);
`triggers.md` §1 r10 (definitions), §9 r6, §10 r4, §12 (routing / open
questions 8, 10); `client/audio.md` §a1 text (exactness check 1 needs
1.14d buffers), §b (owner table); "no caller in 1.14d" edge cases:
`quests-act3.md` edge 17, `quests-act4.md` edge 18, `quests-act5.md` edge
7, `quests-act5-2.md` edge 10; `ui/panels.md` §6 r3, §8 r10, §9 r6,
§10 r7 (open questions).

`formats/d2s.md` (subagent): left §2.2 r8, r9, §2.4 r5, r6, §9 r2, r5,
edge 6 (load effects owned by `d2-server`, no d2s code there yet); §2.4
r2, r3, §2.5 r1, §8.1 r3, r4, r6, r7, §8.3 r1, edge 3, edge 9 (writer
inputs from live state: the model stores what it is given); §2.7 r1–r3
(client readers); §8.2 r1, r3, r5, §8.4 r3 (item reader by version, spec
open question 2); §6 r3, §10 r3 (meanings not traced); edge 4 (the
`i = count[c]` → skill −1 lookup is not in the codec).

## Questions

- GN1 (`hirelings.md` edge 5 vs `npc.md` edge 11): hirelings says d2rs
  refuses C→S 0x62 with a living hireling (code 9) until traced; npc.md
  says the living hireling is charged and reaches the revive (reproduced).
  `world/npc/hire.rs` `resurrect` takes `pet(7, 1)` (any node) and does not
  refuse, while `life::revive`'s comment says `npc.md` §7.4 refuses. The
  specs disagree, so the code was left; a spec session picks one.
- GN2 (`formats/d2s.md` §7.2 r3, edge 4): the game does not check that
  header +0x2A bytes remain; d2rs rejects a short skills section with 19
  (claimed by `skills_section`). The spec states no d2rs policy here
  (unlike edge 2, "d2rs rejects it with 18"); should it say "rejects with
  19"?
- GN3 (`audio/sound-table.md` §9): the line "9. d2rs reproduces mixer mode
  0 only" is a wrapped sentence that starts with "9.", so the coverage
  tool reads it as list item `§9 r9`. It is claimed as is
  (`mixer_modes_1_and_2_play_as_mode_0`); a spec edit that rewraps it
  removes the unit and its claim must go with it.
