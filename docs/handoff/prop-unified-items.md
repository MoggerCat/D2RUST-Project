# Handoff: state-machine properties of the one item store — `claude/prop-unified-items`

> Not yet folded into `docs/HANDOFF.md` / `docs/PLAN.md` (neither is edited here); the coordinator folds it (§1 state, §3 code map, §7 questions).

Cloud implementation session, 2026-10-06, task class: property tests,
medium (METHODS M14). Base: `claude/tender-meitner-mphas3` at `6cd6480`
(unify-items merged). Repo only, synthetic tables, no game files (M09):
every claim below holds on this branch.

## 1. State

**New test file only** (no code change was needed: every counterexample
found follows a written spec rule or an unwritten routine's seam
default, so each is pinned and asked below instead of "fixed").

`crates/d2-server/tests/prop_unified_items.rs` builds one wired host,
`SimGame<ActionSim<_>, WiredWorld<_>>`: the field room (Cold Plains, one
8 × 8-tile room), the player (class 1, 5000 gold, strength / dexterity
15) in it for client 0, Akara (outside the rooms, as `e2e_vendor.rs`),
the vendor tables (cap, buckler), the cube's parts (ring → amulet
recipe), the inventory model (`InvParts`) with staged seams; the
player's buckler, cap and cube stored through §2.4; a ring and a cap on
the ground. Ops, one frame each (dispatch, then the host tick with its
update pass): C→S 0x16–0x29, 0x50, 0x61, 0x63 (GUIDs aimed mostly at the
item each message acts on: the cursor item, a stored item, a ground
item, a body location the item's type allows), talk + chat + trade with
Akara, 0x32 buy (store items), 0x33 sell, 0x2A put-in, 0x4F transmute
(the cube's opening staged), and ground items created mid-run by the
economy wiring (a drop's item creation).

Invariants after every frame (the module doc has the exact list):

1. one place per live item (page grid, belt, body, cursor, ground, NPC
   store / gamble list), its unit mode matching, item data in the one
   store and nowhere else; "limbo" only on the four spec paths below;
2. no grid overlap: cells equal the listed items' rectangles, node grid
   and page agree;
3. the views agree: item list = grids' lists, count (+0x28) = list
   length, cursor in no grid, owning inventory, model GUID / record /
   page vs the unit record and the store, `InvState::holds` /
   `items_of` / `cursor_of` / `body_items`;
4. live GUIDs distinct and stable per allocation; nothing refers to a
   freed unit;
5. the S→C stream: buffers split whole, 0x9C / 0x9D size byte and GUID,
   0x9D owner = the player, 0x47 / 0x48 close every pass that sent item
   messages, the last deferred message agrees with the end place, every
   held item that moved was announced.

Tests (d2-server, 7, all pass; `cargo test -p d2-server --test
prop_unified_items`):

- `item_moves_keep_one_place` (proptest, default 64 cases × 1–40 ops;
  20 000 cases pass in 99 s debug, so the nightly `wire` group of
  `tools/props-deep.sh` picks it up as is: `binary(/^prop_/)`);
- `host_reaches_every_system` (pick, cube, transmute, trade, sale);
- `generator_reaches_every_move` (fixed runner: ground → cursor, cursor
  ↔ page, cursor ↔ body, cursor → ground, store items, a sale and a
  transmute happen; guards the generator against rot);
- `checks_catch_perturbations` (M08: a wrong mode, an item in two
  places, an item nowhere, shifted cells, stale item data, and five bad
  streams are each reported);
- three pinned counterexamples (Q1–Q3 below).

Generator faults found on the way (test-side, fixed before commit): C→S
0x1A/0x1B/0x1D/0x1E and 0x26, 0x33 built short (the layout's trailing
pad); every built message is now padded to and checked against its
`client-messages.tsv` size.

## 2. Code map rows (for `docs/HANDOFF.md` §3)

| Path | What | Spec |
|---|---|---|
| `crates/d2-server/tests/prop_unified_items.rs` | state-machine properties of the one item store + inventory model + S→C stream on one wired host; pinned counterexamples Q1–Q3 | `inventory.md` §1–§2, `inventory-moves.md` §6, §7, §8.1, §11; `vendors.md` §7; `cube.md` §2, §8 |

## 3. Counterexamples and spec questions

Each was found by the property, shrunk, and kept as a fixed test that
asserts the current (spec-literal) behaviour; the property accepts
"limbo" only on these paths, with the mode each leaves.

- **Q1** (`swap_with_a_failed_placement_leaves_the_cursor_item_nowhere`;
  `inventory-moves.md` §7.10): 0x1F takes the target first (unlinked, cursor
  := T, mode 4, command flag 0x40000, update list), then the cursor
  item's §2.2 placement at (x, y) fails (e.g. a 2 × 2 at x = 9) → out 1 →
  result 3. The old cursor item is left in mode 4, linked nowhere and
  not the cursor; no owner refresh runs, so T's move is not sent in that
  frame and T's update-list entry (command flag 0x40000) is sent at the
  next refresh, wherever T is by then (the property saw a 0x9C 0xD for
  an item on the ground two frames later). Question for the spec
  session: does `0x00561B00` really take T before C's fit test (an
  original quirk like §7.24's, then list it under Edge cases), or does
  it test C's fit first / restore T on failure?
- **Q2** (`transmute_with_an_item_on_the_cursor_clears_the_cursor`;
  `inventory.md` §2.4 step 7, `cube.md` §2 step 3.5, §8 step 3):
  `0x00560200` sets "cursor := none" unconditionally. When the placed
  item is not the cursor item — a transmute output (pinned), or by the
  same rule a ground item put in by 0x2A (not pinned) — while the player
  holds an item on the cursor, that held item is left in mode 4, linked
  nowhere and not the cursor. Question:
  does step 7 clear the cursor only when it holds the placed item (or
  does the client never allow the transmute / ground put-in with a
  cursor item)? Record R2 or a new recording (hold an item, transmute)
  would answer it.
- **Q3** (`auto_pickup_with_auto_equip_leaves_the_item_nowhere`;
  `inventory-moves.md` §8.1 step 5): the auto pick-up takes the item out of
  its room, then calls `0x00562E00(item, 0)`, which no spec writes; its
  seam (`MovePending::equip_picked`) defaults to failure, so every auto
  pick-up of an identified equippable item with a free slot leaves the
  item in mode 3, in no room and no inventory, and sends nothing
  (`items/moves/ground.rs` already carries the TODO). Question: write
  `0x00562E00` (it is likely §4.6 without the cursor; vendors.md §7.1
  rule 9.7 calls it too), and the result of its failure.

No fix was made in `crates/`: Q1 and Q2 are what the specs say; Q3 is
an unwritten routine. When a spec answers one, change the pinned test's
expectation and narrow `limbo_mode` in the property.

## 4. What the property does not reach

0x1B / 0x1E (nothing is two-handed in the synthetic tables), the belt
(no beltable item: 0x23–0x26, 0x63's success path), item use (0x20,
0x26, 0x27: the item-use spec), sockets (0x28), tomes (0x29), gold piles
(0x50: `InvRest::gold_request` has no layout, WE9), hirelings (0x61),
vendor buys past the item copy (`0x0055A2A0` unwritten: 0x32 always
stops at code 9), the vendor's own store grid (`unify-items.md` F2: the
rest's list here). Each opens when its seam gets a provider.

## 5. Local checks to queue

None. The questions above are spec-session work (`re/`); a recording
for Q2 (hold an item on the cursor, transmute; hold one, 0x2A a ground
item if the client sends it) would settle it against 1.14d.

## 6. Gate

`sh tools/gate.sh` (after `tools/cloud-setup.sh`): **GATE: PASS**, every step (d2-sim + conformance 2034 tests, rest 606 incl. the 7 new, d2-client 363).
