# cov-ui-quests-overlay (coverage session)

Branch `worktree-agent-a1bb68a969bccd0ed`, base `claude/cov-ui`. Specs: `world/quests-status.md`, `render/overlay.md`. All claims are `unit` tier (unverified against 1.14d).

## Counts (`python3 tools/coverage.py`, covered / rules, exempt rules excluded)

| spec | before | after |
|---|---|---|
| world/quests-status.md | 0 / 64 | 64 / 64 (5 exempt) |
| render/overlay.md | 0 / 32 | 26 / 32 (6 left, below) |

`coverage.py --check`: 0 errors. d2-client tests (`quest_log`, `overlay`): 94 pass. clippy `-D warnings` and `cargo fmt` clean.

## New code (bucket B, all new)

- `crates/d2-client/src/ui/quest_log/` (plain Rust, nothing Bevy):
  - `tables.rs`: the 41-entry table (§2), the 27 status tables (§7-§11, generated from the spec's entry sections by a script), icon names, tab ranges, `chain_entry` (`0x004A1910`).
  - `mod.rs`: `QuestFlags` (96-byte P / G records), `derive_row` (§4 rules 1-8).
  - `state.rs`: `QuestLog` (S, last, counters, remembered slots; 0x52 / 0x50 / 0x5D writes, resets; §1), `build_tab` / `open_tab` (§3).
  - `icons.rs`: icon animation and frames (§5), Seven Tombs symbol, `quest_check` (§12).
- `crates/d2-client/src/world_view/overlay.rs`: `OverlayList` over an `OverlayEnv` seam: create (§2 r1-r8, r11-r14), advance (§3 r2-r6), removals (§3 r8-r11).
- Not wired: `msg_ui.rs` still skips the quest-log table parts (`QUEST_LOG_TABLE`, `DEN_COUNTER` skips); a wiring session should call `QuestLog::receive_0x52/0x50/0x5d` from there and feed `open_tab` / `build_tab` to the panel draw. Nothing calls the overlay list yet either.

## Code fixes found by tests

None (no prior code existed for either spec).

## PROVISIONAL choices (marked `// PROVISIONAL (...; REC-nn)`; REC numbers not assigned, a spec session should file them)

1. quests-status §4 r8, "G.13 clear: G.15 clear, P.13 or P.1 → state 2": read as "G.15 clear OR P.13 OR P.1" (the test vector q 9 with empty P and G requires state 2).
2. §3 r3 selection: "the tab's last clicked selection, else the first entry in state 3": clicked slot if recorded, else first state-3 entry.
3. §5 r1: the 100 ms stamp is rewritten at each counter step (spec names the stamp but not its write).

## Spec inconsistency noticed (spec not changed)

quests-status §2 / §3 r3 number act tabs 0-4 (Act V = tab 4), but the entry sections §7-§11 write "tab 1..5" (Act V "tab 5") and entry 34 "tab 4". The code stores 0-based tabs (§2/§3) and the entry tests compare `tab + 1` to the entry sections.

## Left (overlay.md; not plain translation or belongs to another session)

- §2 r9: overlay light (lighting seam).
- §2 r10: graphics-record null fatal (the list push itself is done in `create`).
- §3 r1: per-update walk call site (`0x004808B7`).
- §3 r7: wall-clock seam of kind 6 (`advance` takes `now_ms` from the caller; the 40 ms x updates choice is the caller's).
- §5: create call sites table.
- edge case r5: light created before the graphics-record check.

The `OverlayList` is a standalone type; the ClientUnit overlay-list session should adopt it (or replace it) rather than add a second one.
