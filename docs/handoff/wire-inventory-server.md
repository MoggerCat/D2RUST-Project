# Handoff: item-move handlers on the wired host — `claude/wire-inventory-server`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Cloud implementation session, 2026-10-06, task class: integration from
clear specs, medium (METHODS M14). Base: `claude/tender-meitner-mphas3`
at `c5a6327` (the merged host), with `origin/claude/wire-inventory-sim`
at `516f9fc` merged in (it had landed: `d2_sim::wiring::inventory`, not
duplicated here). Repo only, synthetic tables, fixed seeds, no game
files (M09): every claim below holds on this branch.

## 1. State

**Wired, unverified** (M02): `inventory.md` is a draft, no recording
R1–R6 exists; no rule is added here, the server maps a message to
`d2_sim::items::moves` on the inventory wiring and sends what it builds.

- **Handlers.** The 23 ids of `items::moves::HANDLED` (C→S 0x16–0x29,
  0x50, 0x61, 0x63) now run on the wired host: `SimGame::handle` →
  `handlers::items::moves::handle` → `WorldHost::moves` (new; on
  `WiredWorld` over the economy `with_economy` builds from the action
  wiring's units, stats and hooks and the host's one item store) →
  `InvDesk::new(econ, tables, state, rest)` → `items::moves::handle`.
  Result 0–3 → `ResultCode`; a `MoveFatal` (the original's fatal
  assert) or a queueing failure → `WorldHost::fault` (new variant
  `WorldError::Move`) and `Malformed`, as the world handlers do. A host
  without `WiredWorld::inventory` (or without a player) keeps the stub.
  0x4C stays unowned (`cube.md` §10).
- **Direct sends** (§6.4: 0x63's 0x9D action 5 / 0x9C action 0xE, a
  stack merge's 0x42, the targeting reset's 0x3F) leave in the handler
  through `MoveRest::take_sent` (the rest's `MovePending::send`
  collection), to the receiving unit's client.
- **Deferred item messages** (§6.1–§6.2, §11): `handlers::items::moves::
  update_pass`, called by `SimGame::tick` after `d2_sim::tick::tick` and
  the host's tick sends. For each client (client-list order), for each
  player in the client room's adjacent rooms: `items::moves::player_update`
  (update-list pass when +0xC8 bit 0, then 0x47, 0x48) → that client.
  Then the clean-up of every player of the pass: `InvDesk::update_done`
  (command flags 0, update list freed) and +0xC8 bits 0, 1 cleared
  (reading, §5 R1). Failures → `SimGame::tick_faults`.
- **Id table.** `MOVE_IDS` (id, `client-messages.tsv` name, owner
  section §7.x); `handlers::items::ITEM_IDS` now names
  `specs/items/inventory-moves.md §7.x` for each of them.
- **Tests.** `handlers/items/moves/tests.rs`: 22 host-frame tests (send
  → drain → handle → tick → flush → receive) on `SimGame<ActionSim,
  WiredWorld>` with a barbarian in a generated field room, real item
  units in the host's store; the only fake is `MoveRest` (`MRest`,
  staged answers + log). Every id: result codes, state (mode, cursor,
  grid / body / belt cells, item data, stats, room membership, unit
  freed, gold pile) and the **exact** S→C bytes of the frame (the 0x9C /
  0x9D headers with the empty item bit stream of OQ1, 0x47 / 0x48, the
  direct sends of 0x63), plus 0x29's fatal (spell mismatch → `Malformed`
  + the fault), 0x61 classic → 3, and the stub without inventory parts.
  Table check `move_ids_match_client_tsv_and_the_module` (name, size,
  kind `handler`, `HANDLED` both ways, `ITEM_IDS` owners) with
  `move_ids_check_reports_perturbations` (M08: a renamed row, a stubbed
  row, a resized row: each one report, nothing else).
- **e2e** (`crates/d2-client/tests/e2e_single_player.rs`, 3 tests pass):
  the host now carries the inventory parts (`e2e_support::InvFx`,
  staged answers). Step 5b, pick-up of the kill's gold (0x16), runs the
  handler and **stops** (asserted): result 1, "item missing" — the
  gold's item record is in `DeathDrops::items`, not the host's store
  (`host-merge.md` W-5); nothing changes, nothing is sent, nothing is
  unhandled any more. New steps 16–21 (frames 26–31) on a cap made in
  the host's store beside the player: pick-up to the cursor (0x16 → 0x9C
  1), grid placement (0x18 → 0x9C 4), lift (0x19 → 0x9D 5), equip (0x1A
  → 0x9D 6), unequip (0x1C → 0x9D 8), drop (0x17 → ground, nothing
  sent), each frame's bytes exact, plus 0x47 / 0x48. Client: 33 frames,
  32 ticks; unowned 0x9C ×2, 0x9D ×3, 0x47 / 0x48 ×5. The transcript
  adds `inv_log`; `same_seed_same_run` and `other_seed_other_run` pass.

## 2. Code map rows (for `docs/HANDOFF.md` §3)

| Path | What | Spec |
|---|---|---|
| `crates/d2-server/src/adapters/handlers/items/moves.rs` | `MOVE_IDS`, `is_move_id`, `MoveRest` (`InvRest` + `take_sent`), `InvParts` (inventory tables, `InvState`, the rest), `MoveCall`, `handle`, `update_pass` | `inventory-moves.md` §6–§11; `intents-events.md` §2.3–§2.4, §3.2 |
| `crates/d2-server/src/adapters/handlers/items/moves/tests.rs` | host-frame tests per id, the id table check + M08 | `inventory-moves.md` §7 |
| `crates/d2-server/src/adapters/handlers/world/wired.rs` | `WiredWorld::inventory: Option<InvParts>`; `moves` (parts lent out for the call) | |
| `crates/d2-client/tests/e2e_support/mod.rs` | `InvFx` (the e2e's `MoveRest`) | |

## 3. Edits outside my files (listed) and signature changes

- `d2-sim` `wiring/inventory/{mod,inv_world,ops,pending,units}.rs`:
  `R: InvRest` → `R: InvRest + ?Sized` on `InvDesk` and its six impls
  (so the host holds the rest as `Box<dyn MoveRest + Send + Sync>`; no
  behaviour change).
- `d2-server` `handlers/world.rs`: `WorldHost::moves` (default `None`),
  `WorldError::Move(MoveFatal)`. `world/wired.rs`: field
  `WiredWorld::inventory` (`None` from `new`), the `moves` impl.
  `handlers/items.rs`: `pub mod moves;`, `ITEM_IDS` owners, one doc
  line. `sim.rs`: the dispatch line in `handle` (after the cube's), the
  `update_pass` line at the end of `Tick::tick`, doc. `items/tests.rs`
  (`unowned_item_ids_stay_stubs`): the owned set now includes
  `MOVE_IDS`; 0x17 still a stub there (that host has no inventory
  parts).
- Public additions only; nothing removed. A `match` over `WorldError`
  elsewhere would need the new arm (none in the workspace).

## 4. What a host must supply

`InvParts::new(InvTables::from_fixed(..), Box::new(rest))` on
`WiredWorld::inventory`; `parts.state.add_inventory(player,
UnitKind::Player { class }, guid)` when a player is created (`0x0063ABD0`
is the unit spec's; nothing in the server does it yet); item data for
ground items made outside the move code (`InvState::items`, the position
in `x`, `y`); the client joined with its room (the pass walks the
client room's adjacent rooms). The rest answers the seams of
`wire-inventory-sim.md` §5 (path distance / free spot, player data,
item use, sockets, hirelings, sounds, the item bit stream).

## 5. Readings and findings

- **R1** (`moves.rs` `UpdateRun`, `TODO(spec)`): which +0xC8 bits the
  room clean-up `0x00553220` clears is not written; read as the two the
  owner refresh sets (§6.1 rule 1: bit 0, bit 1 for players). Without a
  clear the pass would resend 0x47 / 0x48 every tick.
- **R2** (`update_pass` doc): the pass runs after `d2_sim::tick::tick`,
  not inside the client pass (`tick.md` §6 step 5): the action wiring
  implements neither the per-client unit update nor `0x00553220`, so no
  tick step reads what the pass changes. One difference is known: the
  client's room is read after the tick's room switch (`0x00537B50`); in
  the tick of a switch 1.14d walks the old room's adjacency. When the
  tick hooks gain `send_unit_update` / `unit_update` bodies, the pass
  should move there (a `TickHooks` provider holding `InvParts`).
- **R3**: the ground items' unit update (§6.3) is not run: on real units
  it builds nothing (`wire-inventory-sim.md` WV1).
- **F1** (W-5, now visible on the wire): the kill's gold cannot be
  picked up (result 1). Fix: one item store per game (the drop creating
  into the host's store), as `host-merge.md` W-5 says.
- **F2**: three inventory copies remain for the player: the vendor
  rest's staged set (`Rest::inventory`, `owns_item`), the cube's
  `Staged::inventories` (list + cursor) and the move code's `InvState`.
  An item placed by 0x18 is unknown to the vendor's and the cube's
  checks and vice versa. Fix: the vendor and cube seams read `InvState`
  (`InvDesk`) — a wiring task on `world/wired.rs`, `items.rs`
  (`ItemPending::place` / `remove_cube_item` → §2.4 / §1.4 on the
  player's `Inventory`).
- **F3** (e2e): the sorceress fixture had dexterity 0, and §4.2 refuses a
  stat below 1 (the equip returned 0 with nothing equipped); the fixture
  now sets a synthetic dexterity 25 (the creation stats are the
  character spec's).

## 6. Local checks to queue

None new runnable now. When R1–R6 (`inventory.md` Test vectors) are
recorded: replay their C→S messages through `Host` with this host
(`cargo test -p d2-server --lib items::moves` shape, recording-backed
`MoveRest`) and compare result codes and the S→C bytes per frame (0x9C /
0x9D bit streams once OQ1 is written).

## 7. Gate

`sh tools/gate.sh` (all, d2-client included): **GATE: PASS** (spec_index,
methods, coverage 3,813 claims 0 errors + selftest, trace checkers, hook
selftest, fmt, depcheck + determinism, clippy workspace, tests d2-sim +
conformance, rest, d2-client, doc-tests).

Update: merged `origin/claude/tender-meitner-mphas3` at `5413b24` (it
carries `wire-inventory-sim` as `60a4998`, `wire-path-sim`, `render-wire`,
`prop-path-place`; no conflict). `sh tools/gate.sh` on the merge: every
step PASS except `test d2-sim + conformance`, which failed only on the
known seed-dependent `stats::prop_tests::stat_lists_match_the_model`;
rerun of that step: 1,548 passed, 0 failed.
