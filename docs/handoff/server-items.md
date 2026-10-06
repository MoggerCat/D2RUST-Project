# Handoff: `d2-server` item / inventory / cube intent handlers

Cloud implementation session, 2026-10-06. Task class: implementation
from clear specs, medium effort (METHODS M14). Branch
`claude/server-items`, from `claude/bold-ptolemy-jvyvxy` at `1470723`.
Repo only. For the coordinator to fold into `docs/HANDOFF.md` /
`docs/PLAN.md` (neither is edited here).

## 1. State

**Implemented, unverified.** `cube.md` is a draft and no transmute or
put-in recording exists (the checks are queued in `impl-world.md` check 3
and `wire-economy.md` §7.3).

- Real handlers for **C→S 0x2A** (ItemToCube) and **C→S 0x4F**
  (ClickButton, the cube's buttons). They run `d2_sim::world::cube`
  (`CubeData::put_in`, `CubeData::click_button`, so also `transmute`)
  through the economy wiring (`d2_sim::wiring::economy::EconomyCube`:
  real unit records, stat lists, item store, item creation).
- Every other item-related id has no written owner spec and **stays a
  stub** (§2).
- **No `d2-sim` change.** No public accessor was needed.
- Tests: 10 new tests in `adapters/handlers/items/tests.rs`. They run
  real host frames (send → drain → tick → flush → receive) with
  `ProtoSizes` and a `SimGame` holding an `ItemWorld` on synthetic
  tables. Items are real units from `Economy::create_item`. The only
  fake is `ItemPending`, the calls no spec owns. Results: `cargo test -p
  d2-server` 53 pass (43 before); `cargo test -p d2-sim` 828 pass, 5
  ignored.
- M08: setting the ground range to 11 fails `item_to_cube_ground_item`.
  Cutting the sent 0x77 to one byte fails both tests that check its
  bytes.
- Gate (all pass): `cargo fmt --all -- --check`, `cargo clippy -p
  d2-server -p d2-sim --all-targets -- -D warnings`, `cargo test -p
  d2-server -p d2-sim`, `cargo run -p depcheck`, `python3
  tools/spec_index.py --check`, `python3 tools/methods.py check`,
  `python3 tools/coverage.py --check` (2,405 claims, 0 errors).

## 2. Item-related C→S ids: owner and status

Also in code as `handlers::items::ITEM_IDS`, which a test checks.

| Id | Name | Owner spec | Status |
|---|---|---|---|
| 0x16–0x29 | PickItem … ScrollToBook (pick, drop, grid, equip, swap, use, stack, belt, socket, tome) | none (inventory / item-use specs not written) | stub |
| 0x2A | ItemToCube | `world/cube.md` §2 | **implemented** |
| 0x4C | Transmogrify | none: `cube.md` §10 routes it to the item-use spec (not written) | stub |
| 0x4F | ClickButton | `world/cube.md` §1 (no interaction → 0x77 0x0C; buttons 0x17 / 0x18) | **implemented** for the cube's rows; any other button with an active interaction → `click_button` gives `None` → stub |
| 0x50 | DropGold | none | stub |
| 0x61 | MercItem | none | stub |
| 0x63 | ItemToBeltShift | none | stub |

The vendor ids (0x32–0x35, 0x37, 0x38: `world/vendors.md`) belong to the
world handlers, not here. 0x3E and 0x44 are quest ids.

## 3. Code map rows

| Path | What | Spec |
|---|---|---|
| `crates/d2-server/src/adapters/handlers/mod.rs` | handler modules root | `sim/intents-events.md` |
| `…/handlers/items.rs` | `ITEM_IDS`, `handle` (0x2A, 0x4F), `ItemWorld` (units, stat lists, unit data, hooks, `GameFields`, `ItemTables`, `ItemStore`, `CubeData`, `Staged`, creation info, `ItemPending`, errors), `ItemView`, `Staged`, `Interaction`, `Inventory`, `ItemPending`, `ItemHooks`, `ItemError` | `world/cube.md` §1, §2 |
| `…/handlers/items/cube_world.rs` | `ServerCube`: `CubeWorld` that forwards the item, stat and creation calls to `EconomyCube` and answers the rest from `Staged`, the `cube.md` §2 checks and `ItemPending`; `InfoRest` (the one `CubeRest` call `EconomyCube` makes itself: `player_info`) | `world/cube.md` §1, §2, §8 |
| `…/handlers/items/tests.rs` | host-frame vectors | |

Edits outside these files (registration only):

- `adapters/mod.rs`: `pub mod handlers;`.
- `adapters/sim.rs`:
  - a field `pub items: Option<ItemWorld>` (initialised to `None`);
  - in `handle`, one `if let Some(r) = handlers::items::handle(...)`
    before the stub (the `out` parameter lost its `_`);
  - a new impl block with the private `item_view` (split borrow of
    game, item world and staged facts);
  - one doc-comment sentence.
- `crates/d2-server/Cargo.toml`: dev-dependency `d2-data` (for the
  synthetic tables of the tests).

**Merge note** for the parallel `world` / `skills` handler sessions:
`handlers/mod.rs` gets one `pub mod …;` line per session. In
`SimGame::handle`, each session adds its own `if let Some(r) = …` line
before the stub. The order of these lines does not matter while the id
sets are disjoint. 0x4F is claimed here (`cube.md` is its only owner
spec). If `world/npc.md` or `vendors.md` later owns another button,
`click_button`'s `None` is where that handler goes.

## 4. Design points

1. **One wrapper, exact call order.** `EconomyCube`'s rest (`CubeRest`)
   cannot see the economy, but `cube.md` §2 needs both: the item checks
   read unit modes and the targeting reset clears item flags. So
   `ServerCube` implements `CubeWorld` itself and forwards to
   `EconomyCube`. Every call then runs inline, in the module's order.
   `InfoRest` answers `player_info`, the only rest call `EconomyCube`
   makes internally. Every other `InfoRest` method is `unreachable!`.
2. **Staged, as with `UnitFacts`.** `cube.md` describes the interaction
   state (+0x64 GUID, +0x68 type, +0x6C active; set only when none is
   active; reset to GUID −1, type 6, inactive). It also uses the
   inventory list and cursor item, the local date (host input), and the
   sound events (unit +0x6E/+0x70). `d2-sim` holds none of these, so the
   caller stages them in `Staged`. Sound events are recorded, not sent:
   which message carries them is `cube.md` OQ 2. Positions and acts come
   from `SimGame`'s staged `UnitFacts`. A ground item or player without
   them counts as missing (→ 1), the same as the unit-target lookup.
3. **`ItemPending`** collects the calls whose owner spec is not written:
   - inventory pass `0x0055FA40`;
   - placement `0x00560200`;
   - the 0x9D and removal part of `0x00564F30`;
   - the socketed list;
   - `duplicate`, `tempered_affix` (WE6), `drop_runeword_stats`,
     `repair`, `recharge`;
   - the quest item hooks;
   - the Cow portal.

   `ItemWorld` requires a provider. No default behaviour is invented:
   with `SimGame::items = None` (as in `SimGame::new`), 0x2A and 0x4F
   stay stubs.
4. **Messages** go only to the acting client, in call order, after the
   module returns. A send to another player is an `ItemError`. The only
   S→C message these paths build with known bytes is 0x77 (2 bytes: id,
   value; `cube.md` §1), and it is byte-exact in the tests. 0x3F (no
   layout in `server-messages.tsv`) and 0x9D (bytes open, `cube.md`
   OQ 1) are not built here.
5. **Two unit stores.** `ItemWorld` owns its own `Units` / `StatLists`.
   `ActionSim` (action wiring) owns another pair. A wired single-player
   host (`docs/HANDOFF.md` §2 step 4) must merge them. A `TODO` sits on
   `ItemWorld`.

## 5. Open questions (each has a `TODO` at the site)

- **SI1** (`cube_world.rs` `put_item_check`, `cube.md` §2 step 1): the
  failed range-10 test is written as "non-zero". d2rs returns 1, the
  "out of range" code of `intents-events.md` §2.3. Settle: Ghidra
  `0x00549350`.
- **SI2** (`cube_world.rs` `remove_cube_item`, `cube.md` §8 step 1): the
  spec frees through `0x0055DF10` → `0x00557FD0`. d2rs frees with
  `0x00555600` (`units.md` §3.2, the economy wiring's `free_item`) after
  `ItemPending::remove_cube_item`. Settle: confirm that `0x00557FD0`
  reaches `0x00555600` and does nothing else that the sim sees.
- **SI3** (`items.rs` `Staged::targeting_resets`, `cube.md` §2 step
  3.1): the argument of `0x0044BE50` is not named, and S→C 0x3F has no
  layout in `server-messages.tsv`. The flag 0x4 clearing is done (over
  the inventory list; whether the cursor item counts is not written).
  The 0x3F is only recorded.
- **SI4** (`cube.md` OQ 1, OQ 2): the bytes of placement, removal and
  sound messages. These need the V1 / V22 recordings already queued.

## 6. Checks to queue (for `docs/HANDOFF.md` §5)

1. Once a packet recording of a cube session exists (put-in via 0x2A,
   transmute V1 record 23, close), replay its C→S messages through
   `Host` with an `ItemWorld` from the live tables. Compare each
   handler result, the 0x77 bytes, and (once SI4 is settled) the
   placement and removal messages.
2. SI1 and SI2 are Ghidra reads (group B): `0x00549350` failure value;
   `0x0055DF10` → `0x00557FD0` free path.
