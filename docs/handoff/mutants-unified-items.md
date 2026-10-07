# Handoff: mutation testing of the unified item host — `claude/mutants-unified-items`

> Not yet folded into `docs/HANDOFF.md` / `docs/PLAN.md` (neither is edited here); the coordinator folds it.

Cloud test session, 2026-10-06 (METHODS M08: prove the checks can fail),
medium effort. Branch `claude/mutants-unified-items` from
`claude/tender-meitner-mphas3` at `6cd6480`. Repo only (M09): every count
below holds on this branch, with `cargo-mutants` 27.1.0. Target: the
non-test code that `unify-items` landed (merge `41d66c1`,
`docs/handoff/unify-items.md`). None of those files changed between
`41d66c1` and `6cd6480`.

## 1. Run

The mutants are limited to the lines the merge added (`--in-diff`), using the
merge's diff of the non-test files:

```
git diff 41d66c1^1 41d66c1 -- \
  crates/d2-sim/src/wiring/action/mod.rs crates/d2-sim/src/wiring/economy/death.rs \
  crates/d2-sim/src/wiring/inventory/host.rs crates/d2-sim/src/wiring/inventory/mod.rs > sim.diff
git diff 41d66c1^1 41d66c1 -- \
  crates/d2-server/src/adapters/handlers/items.rs \
  crates/d2-server/src/adapters/handlers/items/cube_world.rs \
  crates/d2-server/src/adapters/handlers/items/moves.rs \
  crates/d2-server/src/adapters/handlers/items/vendor_inv.rs \
  crates/d2-server/src/adapters/handlers/world/wired.rs > server.diff
cargo mutants -p d2-sim --in-diff sim.diff -j 3 --timeout 180 --build-timeout 900 \
  -o <scratch> -- --lib -- wiring::
cargo mutants -p d2-server --in-diff server.diff -j 3 --timeout 120 --build-timeout 900 \
  -o <scratch> -- --lib
```

The counts are conservative. A mutant counts as missed if it is caught
only by tests outside the run:
- d2-sim tests outside `wiring::`;
- d2-server integration tests (`tests/prop_handle.rs` and the others);
- the d2-client e2e tests (`e2e_single_player`, `e2e_vendor`), which
  were the only tests driving `InvVendors` before this session.

`mutants.out` is not committed.

| Crate | | Mutants | Caught | Missed | Unviable |
|---|---|---|---|---|---|
| d2-sim | before | 29 | 22 | 1 | 6 |
| d2-sim | after | 29 | 23 | **0** | 6 |
| d2-server | before | 247 | 39 | 167 | 41 |
| d2-server | after | 247 | 206 | **0** | 41 |

Missed by file, before → after:

| File | Before | After |
|---|---|---|
| `d2-sim/src/wiring/inventory/host.rs` | 1 (`fillers` → `vec![]`) | 0 |
| `d2-sim/src/wiring/economy/death.rs`, `action/mod.rs`, `inventory/mod.rs` | 0 | 0 |
| `d2-server/.../items/vendor_inv.rs` | 112 | 0 |
| `d2-server/.../items/cube_world.rs` | 54 | 0 |
| `d2-server/.../world/wired.rs` | 1 (`vendors` → `None`) | 0 |
| `d2-server/.../items.rs`, `items/moves.rs` | 0 | 0 |

The 47 unviable mutants (6 + 41) do not compile: a `Default::default()`
of a type with no `Default` (`Ref`, `InvDesk`, `UnitId`, `ItemError`,
the generic `T` / `C::Out`). None was run.

**Survivors: none.** None had to be classified as equivalent, unobservable,
a seam default, or waiting on a spec.

## 2. Tests added (new files only)

- `crates/d2-sim/src/wiring/inventory/tests/mutant_tests.rs` (1 test),
  declared in `tests/mod.rs` (`mod mutant_tests;`).
  `fillers_are_the_items_own_inventory_list`: an item's own inventory
  (`inventory.md` §1, `inventory-moves.md` §7.19 step 3) with two fillers linked. `fillers`
  reads them back in link order, and they are not the player's items.
- `crates/d2-server/src/adapters/handlers/items/mutant_tests.rs` (8
  tests), declared as a child of `items/tests.rs` (`#[path =
  "mutant_tests.rs"] mod mutant_tests;`) so it can use that file's wired
  host fixture (`setup`, `store`, `item`). It also has a `Probe` rest
  (the quest tests' staged NPC / quest answers, copied from
  `trade_quests::Rest`, plus every `VendorRest` call logged and answered
  from its arguments). The `VendorRest` calls the model must answer
  (`has_cursor_item`, `owns_item`, `in_inventory`, `equipped_items`,
  `place_in_backpack`, `remove_stored`) are `unreachable!` there.
  - `inv_vendors_answer_from_the_model`: the cursor item (§1.4 r3),
    ownership (item list or cursor, R1's reading), the body items (grid
    0, §1.2, the ring put at body location 4 by `place_at_body`),
    `vendors.md` §7.1 r9.7 auto-place on §2.4 (a cursor item placed, a
    ground item refused), §7.2 r9 removal (unlink and free). With no
    inventory parts, nothing is held and placement fails.
  - `inv_vendors_pass_other_calls_through`: every other `VendorWorld` /
    `NpcLink` call gives the wrapped `VendorDesk`'s answer: game fields,
    NPC records, unit records, stats, quest slot, the `quests.md` §6.7
    0x91 of `town_entered`, item creation, copy, destroy, and the item
    reads and writes. It also checks the exact rest call log.
  - `wired_vendors_run_on_the_model`: `WiredWorld`'s `WorldHost::vendors`
    runs the call on `InvVendors`; a cursor item auto-placed by the call
    is in the model.
  - `server_cube_answers_from_the_economy`: `ServerCube`'s game, unit,
    stat and item calls (fields, seed, class, stats, GUIDs, class / type,
    quality, file index, level, flags, mode, sockets §7.2 / §7.3, item
    seed, tempered, unique bits, free). Each write is checked where it
    lands (the seed and unique bits written back by `with_economy`).
  - `server_cube_trading_and_stash`: `0x005678A0` (type 0 and a live
    player) and the stash (type 2, object class 0x10B), against the
    near misses.
  - `server_cube_socketed_reads_the_model`: `socketed` = the item's
    own inventory list.
  - `server_cube_message_to_another_player_is_an_error`: `cube.md` §8
    step 1 on another player's item. The direct 0x9D goes to that
    player. It is not sent on the acting client; it is
    `ItemError::OtherPlayer`.
  - `server_cube_item_routines_reach_the_economy`: the item format,
    item init `0x00557AB0` returning the item, a craft property
    (properties func 15) landing on the item's stats.

Outside these files, only the two `mod` lines changed. No non-test code
changed, and nothing in `wiring/economy` (drop-freespot's area).

**No `Covers:` claims.** Each test checks one clause of a unit that other
tests already claim, or a forwarding contract that no spec unit owns. As
in `mutants-inventory`, each test names its clause in a `// Rule (one
clause ...; no claim):` comment. Coverage numbers are unchanged.

## 3. Findings

- **F1 (test gap, closed here):** before this session, no `d2-server`
  test called `InvVendors`. Its 112 survivors were the whole wrapper:
  the model's answers and every pass-through call. Only the d2-client
  e2e tests (which need Bevy built) reached it. A forwarding slip there
  would have shown only in the client gate.
- **F2 (gap, not a mutant):** what the inventory rules queue during a
  vendor call (`WiredWorld::inv_sent`, appended last by `take_sent`,
  `wired.rs` R3) is still never non-empty in any test. The one producer
  is the §5.3 targeting reset's 0x3F, and the item-move fake (`MRest`)
  answers `0x0044BE50` with the seam default, so no 0x3F is queued. A
  statement deletion is not a `cargo-mutants` mutation, so the run does
  not see this. Follow-up for the owner of `moves/tests.rs`: add a knob
  to `MRest` for `0x0044BE50`, then assert the 0x3F's place in
  `take_sent` once R3 (the order against the 0x2A) is written.
- No code deviation from the specs found.

## 4. Open questions

None new. R1–R3 of `unify-items.md` stay as written. The tests pin R1's
current reading (owned = in the item list or the cursor item), not a
confirmed 1.14d behaviour.

## 5. Local checks to queue

None. Every count above holds on the repo with synthetic tables.

## 6. Gate

`sh tools/gate.sh` on this branch: every non-client step **PASS**
(spec_index, methods, coverage, trace checkers, hook selftest, fmt,
depcheck, tests d2-sim + conformance, tests rest, doc-tests).
`cargo clippy -p d2-server -p d2-sim --tests -- -D warnings` is clean.
The three client steps (clippy workspace, test d2-client, doc-tests
d2-client) **FAIL** for an environment reason only: this container
lacks `wayland-client` (pkg-config). `tools/cloud-setup.sh` was not run,
because the coordinator asked to wrap up. No d2-client file changed.
**Next step:** run `sh tools/cloud-setup.sh && sh tools/gate.sh` to
confirm the client steps.
