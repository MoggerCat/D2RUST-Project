# Handoff: property tests of the inventory model and the item-move intents — `claude/prop-inventory`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Cloud test session, 2026-10-06. Task class: property tests from specs plus
root-cause fixes, medium effort (METHODS M14). Base:
`claude/tender-meitner-mphas3` at `edd9925`. Repo only, synthetic data, no
game files (M09: every claim holds on this branch, debug build, overflow
checks on). Inputs read: `specs/items/inventory.md` §1–§5, `specs/items/inventory-moves.md` §6–§7, `docs/`,
`crates/d2-sim/src/items/{inventory,moves}`. Parallel sessions
`wire-inventory-sim` (`wiring/inventory/`) and `mutants-inventory`
(`mutant_tests.rs` files) were left alone: this session's work is two new
test files, one module line and one root-cause fix.

## 1. State

| File | Property | Model (spec) |
|---|---|---|
| `crates/d2-sim/tests/prop_inventory.rs` | `inventory_matches_the_model`, `regress_place_near_i32_max` | State machine over random worlds: owner kind (player classes 0–6, monsters 0x1A1 / 0x21C / 0x230), expansion, `inventory.bin` grid sizes (0–10 × 0–10), item sizes 0–3 × 0–4, itemtypes body locations / beltable / quiver / class / equivalence, item flags and stats, unit stats; `belts.bin` `numboxes` are the spec's measured values (§3.1). Ops: §2.2 place at a position, §2.3 search then place, §2.4 page placement from the cursor (find / position, send / not, every page incl. the player's page-1 cursor quirk and page-2 trade hook), §1.4 unlink, §4.6 equip from the cursor, §3.7 slot placement, §3.5 free slot then placement, §3.8 compaction, flag and mode changes, and the pure checks §4.3 (with and without an item), §4.4, §4.5, §4.7, §5.1 (all six). Reference model written from the rules: §1.3 grid records, §1.4 link order / count / cursor / update list, §2.1 fit, §2.3 brute-force search in the spec's four orders with the four-side weight, §3.4–§3.8, §4.1–§4.5, §4.7, §5.1, §5.3 (0x3F queue in list order, flag 0x4 cleared) |
| `crates/d2-sim/src/items/moves/prop_tests.rs` (declared in `items/moves/mod.rs`) | `handlers_on_arbitrary_payloads` | Every id of `HANDLED` (plus random foreign ids and sizes ±2) on the `items::moves` test fake with random state (items in every mode, foreign items, cursor or not, body / belt slots, a hireling with equipment, extreme stat values) and random seam knobs; payload words are item GUIDs by role (cursor, stored, belt, equipped, ground, owned, the player; resolved against the live state), the missing GUID, small numbers, coordinates near the player, or any u32 |

Invariants checked after every step (`prop_inventory.rs`): the item list
equals the model in link order; cursor, count and update list equal the
model, no duplicate on the update list; every grid has its §1.2 / §1.3
size, its list equals the linked items placed in it (list order), every
item covers exactly its w × h cells inside the grid and no cell holds
anything else (no overlap), every cell equals the model; item fields
after §2.2 (owning inventory, x, y, node grid, node kind, owner GUID,
page); unlinked items carry no node; room removals (mode 3 placements),
trade hooks and 0x3F queue equal the model. The free-position search
returns exactly the reference's spot, and a spot iff one fits anywhere
(sliding argument: a fitting spot slid left stops at an edge or a
neighbour, so its weight is > 0). Accepted equips: the location is
allowed (§4.1) and was empty; §4.7 never names a forbidden or occupied
location; §3.5 never names a slot ≥ the belt's `numboxes`.

Handler properties (`prop_tests.rs`): no panic (the fake also panics if a
handler reads a field of an item that does not exist); `None` exactly for
an id outside `HANDLED` or an empty message; results 0–3 (or a named
`MoveFatal`); a wrong size → 3 with no state change; and every rejection
§7 orders before the first effect returns the spec's code with the state
unchanged (items, units, inventories, sent messages, GUID counter; the
seam call log is not state): 0x16 r1 / r2.1, 0x17 1–2, 0x18 1–3, 0x19
1–3, 0x1A, 0x1B up to the requirement failure, 0x1C up to the equip
check, 0x1D up to the equip check, 0x1E, 0x1F 1–2, 0x20, 0x21, 0x22 (3
for every owned item), 0x23, 0x24, 0x25, 0x26, 0x27, 0x28 step 1, 0x29,
0x50, 0x61 1–2, 0x63 1–2.

Readings the references follow (the code's, each an open point already
listed in `impl-inventory.md` §5): §3.5 a similar column without an empty
slot tries the next column (item 4); §4.7 "type ≠ 38" is the primary type
and "an equipped hand weapon" is either hand (item 6). §2.4 step 5's link
check always succeeds here (owner not an item), so item 2 is not reached.

Counts: default 256 cases (`prop_inventory`, 1–60 ops each) and 512
(`prop_tests`, 1–12 messages each); `PROPTEST_CASES` overrides, no failure
persistence (convention of `prop-sim-core.md`). Runs on this branch:
20,000 cases of each, then 30,000 handler cases after the stat extremes
were added, all pass.

M08 (each mutant through the code, each caught, all restored):
search tie `>` → `>=` (find_free_position); h = 1 non-player order
reversed (find_free_position); unlink count not decremented (count);
§4.3 hand row "compatible → 1" → 2 (equip_check); §3.5 column walk past
`numboxes` (free_belt_slot); compaction without flag 0x400; belt check
failing owned belt items (belt item check); stored-or-equipped check
without mode 1 (stored or equipped check); 0x1D empty location → 0
instead of 1 (early rejection); 0x19 clearing the unit flag before the
gate (rejected with an effect). Three of them first survived and showed
generator gaps, fixed before the record: hand rows were rare (body
locations now favour 4 / 5, equip locations favour the item's own),
owned items were never in mode 2 (the harness now sets the mode the
callers set after a placement), and the handler world's knobs were read
past the end of the setup bytes (always the zero default: the gate never
refused; setup bytes are now long enough, measured: 622 / 305 read).

No coverage claims were added (as `prop-sim-core.md`: the properties
overlap the unit-tier claims; a docs session may add them).

## 2. Bugs found and fixed (root cause, input, fix)

1. **`grid::in_bounds` (§2.1) overflowed `x + w` / `y + h` in i32.**
   Reachable from a client payload: 0x18 passes x, y (u32 @5, @9) as i32
   to §2.4, whose `max(x, 0)` keeps `i32::MAX`. Debug build: panic; release:
   the sum wraps negative, the bound passes, the fit range `x..x + w` is
   empty so the fit passes, and the item is linked at x = 0x7FFFFFFF
   covering no cell. Minimal input: a 1 × 1 item from the cursor, 0x18
   x = 0x7FFFFFFF, y = 0 (any page) → `place_at_page` → `in_bounds`.
   Fix (`items/inventory/grid.rs`): the sums are taken in i64, so the
   placement is refused (0x18 → 3, nothing changed). Regression test
   `regress_place_near_i32_max` (fails on the old code). The original's
   32-bit behaviour near 2^31 is not in the spec: `TODO(spec: …)` at the
   site; question below.

## 3. Findings without a code change

- **Out of the spec's domain, not generated**: a player level above
  214,748 makes `level × 10000` (§7.22 gold limit) overflow; the code
  wraps (32-bit reading), the spec does not say. A unit position near
  `i32::MAX` overflows `x + 2` in §9.1's start point. Neither state is
  reachable (levels cap at 99; positions are subtiles in a level); the
  generators keep realistic values.
- The handlers' own arithmetic on stats (0x21 merge, 0x29 book count,
  0x50 gold) already wraps; extreme stat values (quantity, max stack,
  gold `i32::MAX`, negative) produced no panic in 30,000 cases.

## 4. Questions for a spec session (local, `re/`)

1. §2.1 / §2.2: how `0x0063AFD0`'s bound test treats x or y near 2^31
   (signed or unsigned compare, 32-bit wrap). d2rs refuses; if the
   original wraps and places, 0x18 with such a payload would differ.

## 5. Gate

`sh tools/gate.sh all` on this branch: **GATE: PASS** (spec_index,
methods, coverage check and selftest, trace checkers, hook selftest, fmt,
depcheck, workspace clippy, d2-sim + conformance tests, the rest, d2-client
tests, doc-tests). The test code allows `clippy::if_same_then_else` (the
§7 oracle keeps each step a branch) and one `too_many_arguments`.
