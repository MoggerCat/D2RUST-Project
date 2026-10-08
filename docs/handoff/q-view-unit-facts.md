# q-view-unit-facts

Links connected (draw order's unit facts, `draw-order.md` §3 r4 / §5, `draw-order-2.md` §15):
- `world_view/unit_facts.rs`: `UnitFactTables` (`unflatDead`, `DrawUnder`, `LOSDraw`) loaded by `unit_facts::load` from the user's tables; `fill_model` / `model_facts` build `UnitFacts` from `ClientUnit` (flags, flag-ex, states 7 / 143 / 146).
- `ViewFeed::set_unit_fact_tables` (default no-op) -> `ModelFeed::unit_tables`; `near_rooms` and `unit_facts` use it (strict path: `model_facts`; preview: overlay on the preview fills).
- `app/play.rs`: loads the tables next to `add_preview_tinted`.

PROVISIONAL: REC-273 (see `docs/HANDOFF.md` §7).

Follow-up: the sight test runs over `world.drlg` (`ClientRooms`, `sight_with`; test: a collision-bit-2 cell between player and unit hides it in a LOSDraw level).

Left: flag bits no model rule writes; sight uses model cells, not the walk prediction.

Local check: `cargo run -p d2-client -- play ...` (as in `docs/local/2026-10-07/PLAYABLE.md`), kill a monster whose corpse lies flat (it draws under standing units) and stand next to a DrawUnder object (e.g. a floor decoration); both draw below units. Unit tests: `cargo nextest run -p d2-client unit_facts`.
