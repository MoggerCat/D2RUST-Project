# last-client coverage session — handoff

Session ran out of time (deadline moved to 20:22 UTC). **No coverage was added
and no code was changed in this push.**

## Counts (uncovered rules before = after)
| File | Uncovered |
|---|---|
| skills/descriptions.md | 67 |
| client/model.md | 17 |
| client/stat-lists.md | 13 |
| ui/panels.md | 15 |
| ui/panels-2.md | 17 |
| ui/controls.md | 6 |
| render/overlay.md | 6 |
| render/unit-composite.md | 7 |
| audio/triggers-2.md | 20 (18 are §21 owner-table rows: narration) |
| world/objects-client.md | 6 |

## Code fixes
None.

## State left behind
A subagent was still building/testing an unverified draft of
`crates/d2-client/src/ui/tests_lclient_skilldesc.rs` + `skill_desc.rs` +
`ui/mod.rs` (skills/descriptions.md tests) in the working tree of the cloud
session; it was NOT committed because it never passed fmt/clippy/tests.

## Rules left and why
- Everything above: not reached in the time available (cold Bevy build of
  d2-client did not finish).
- stat-lists.md §1 r1/r3/r4, §2, §3 r6, §4: client uses a simplified model
  (`ClientUnit.stats`, `state_lists`); the client callback 0x004609F0, item
  lists and equip/detach need new wiring in `bridge/world.rs` (item stream
  is open question 2).
- ui/panels-2.md §14: `ui/panels/npc.rs` predates the final spec. To do:
  Resurrect insert/remove (0x004B6440, string 0x1507), `record_for` fallback to
  record 0, talk-end decision (class set 146,251,266,331,377,378,406,408,521,
  527,537,538,539), 0x2F/0x30/0x31 byte builders, Cain count reset, shop button
  tables / tab pages / mouse rects (r10–r13). Tests go in
  `ui/panels/tests_lclient_ui.rs`.
- audio/triggers-2.md §21 rows: owner table, narration; edge r1 needs a sim
  test of the 0x2C last-event-wins rule.
- *edge-cases-original-bugs* sections: cover only where a test is feasible.
