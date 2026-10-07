# last-world-items coverage session

Time ran out (deadline moved to 20:22 UTC); two subagents ran in worktrees and
had little time (slow first builds). No code fixes, no spec changes.

## Covered
- specs/formats/d2s-load.md: §5 r1 claimed on the refusal-table test
  (the "other value -> 9" fallback is asserted there).
- specs/world/quests.md: §1.8 r1–r4 (completion test, bit 1 -> bit 15 on load,
  played-completion save/load and `reset_progress`, slots 41 / 4 bit 10) in
  `crates/d2-sim/src/world/quests/tests_lworld_world.rs`.

## Left (no work done unless listed above; none are blocked, all are codable from the cloud)
Uncovered lists are in plain `python3 tools/coverage.py`; `--rules <file>` lists all units.
- formats: d2s.md (31 uncovered, incl. §2.4 r5, r16), d2s-load.md (12: §3 r1,
  §4 r1, §5 r3, §6 r1, §7 r1, §8 text/r4, edge r2–r3 — need the Iron Golem
  join, runeword-mismatch and hotkey-reload wiring), wav.md (§2 text, §5 text),
  animdata.md (4).
- items (surveyed, nothing written): bitstream edge r9; generation §3 r9, §9 r2,
  §10.3/§10.4, §12.*; inventory-moves §7.10 r4–r6, §7.19 r4, §8.5, §12;
  inventory §4.9, §5.7, §5.8, edge r10–r14; quality §2, §8 r1 (testable: format-0
  durability x5), edge r5 (unreachable with 1.14d data), r7; treasure §6 r4,
  §9, §9.1, edge r9. Quality tests go beside
  `crates/d2-sim/src/items/tests/gaps_quality.rs` (helpers there).
- world: quests.md §9.2/§9.6 (need item/object-init fakes); quests-act2-2 (10);
  quests-act1-rest (7); hirelings (13); hirelings-2 (5); hirelings-ai (1);
  waypoints (11); plus quests-status, object-population, objects, objects-2,
  cube, vendors, npc remainders.
