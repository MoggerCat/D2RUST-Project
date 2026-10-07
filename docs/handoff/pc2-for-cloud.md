# PC 2 spec answers that require implementation changes (for the cloud)

PC 2 (local spec orchestrator) does not edit `crates/`. Each line names the
spec change and the code it invalidates. Branch: `claude/local-pc2-integration`.

- `world/quest-messages.tsv` +15 Act V intro rows (CODE-TABLE `39dabf1`) and `quests.tsv` row 40 (`b242ee7`): d2-sim `tables_parse_and_check` must change `messages.len()` 779 → 794 and `rows[40]` (no longer unknown; 2 callbacks).
- `world/object-functions.tsv` owner changes (CODE-TABLE `d44907e`; init 13 → `objects-2.md` §17, init 51 / operate 48 → §18.4; init 37 → `quests-act2.md`): d2-sim `world/objects/tests.rs` `routes_match_function_table` hard-codes the old routes (exact rows: see `docs/handoff/pc2-spec-objects.md`).
- `world/hirelings.md` edge case 5 / `world/npc.md` §7.4 step 2, edge case 11 (GN1): C→S 0x62 with a living hireling → 0x2A code 9, no gold taken (1.14d charges and then uses a freed unit at `0x00579AA0`). `world/npc/hire.rs` `resurrect` does not refuse yet.
- `world/quests-act1-rest.md` §9 (QA-2): an object left without a room after a room free: 1.14d skips / does nothing at every quest site except the gibbet event's portal (fatal at `0x0056D147`); replace d2rs's `QuestError::Fatal` at the other sites by the tabled effects. §9 item 7: quest InitFns run inside the object allocation `0x00555230` (draining them later is wrong for inits 6, 7, 9, 54, 61; C79).
- `world/quests-act5.md` §4.7 (QE-8c): freed = 15 → 36.5, = 14 → 36.6, any other → 36.7 (equality test at `0x00588B96`); the impl reading "freed > 15 → 36.5" is wrong. `quests-act5-2.md` §8.5 (QE-8e): no victim room at Baal's kill ends the whole callback in a non-intro game (no missile 625). §6.8 (QE-8a): status "to all" clears flags unless the rule says "flags kept".
- `ui/panels.md` §12.4 (C71): `crates/d2-client/tests/game_panels.rs` must not expect zero offsets for `menu\horadric`: frames 0 and 30 = 2 × 2 at (0, 0), frame 1 = 92 × 121 at (−205, 17), frame 15 = 239 × 255 at (−280, 82); then C71 can be re-run locally.
- `ui/panels.md` §8.7: level (stat 12) uses the compare color and `%ld`, not digit grouping (only stats 13 and 30 are grouped by `0x00525350`); §10.6 skill-tree close offsets for all 7 classes; §12.7 cube close; `ui/menus.md` §1 waypoint input.
- `formats/d2s.md` edge case 15: the d2rs writer refuses an item whose child count differs from its record (1.14d does not check; decision pending with the user).
