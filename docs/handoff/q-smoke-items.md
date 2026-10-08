# Handoff: item smoke test (`claude/q-smoke-items`)

> Readiness check, 2026-10-08. Synthetic data only; nothing verified
> against 1.14d (rule 10). PROVISIONAL points: **REC-281**
> (`docs/HANDOFF.md` §7).

## What runs

`crates/d2-client/tests/smoke_items.rs`, test
`drops_pick_up_grid_belt_cursor_and_ground`. It runs the real play
path headless: the app's synthetic game on its server thread, the
Bevy app with `add_game` / `add_walk` / `add_client_data`, the bridge
and the client model. No window.

After every step the rig checks four things:

- no C→S intent the server dispatch refused (a `Tap` link reads
  `LocalLink::last_frame`);
- no S→C message unowned, dropped, rejected or discarded since the join;
- the client's items equal the server's store, compared by GUID →
  (code, mode, body, page, x, y);
- no panic.

Steps covered:

1. A monster dies by the player's hand. Its TC drops a sword, a cap and
   gold, and all three appear in the client.
2. Chests open. One of them drops an axe, a potion, an identify scroll,
   a TP scroll, a charm and a gem; a quarter of plain chests drop
   nothing, so three are placed.
3. Gold is picked up. The pile leaves the client, and the gold stat
   reaches the client through the vitals sync.
4. The sword is picked up with auto placement. The cap goes to the
   cursor, then the grid, back to the cursor, and is dropped on the
   ground.
5. The potion goes from the cursor to the belt, from the belt to the
   cursor (0x24), and back to the belt.

## Breaks found and fixed

Breaks 1 to 4 are in one commit, `Ground items reach the client`,
because all of them are needed before a drop shows up and they share
files. Break 5 has its own commit.

1. **Ground items were never sent to the client.** The per-unit update
   and the add messages skipped items: "the item world is not reachable
   from the action wiring". So drops, chest loot and dropped items were
   invisible.
   - The action wiring now queues (receiver, item, 0x9C action) on
     `ActionHooks::item_updates`.
   - The server's item update pass builds those messages on the desk
     (`items::moves::item_world_action`).
2. **Dropped treasure had no position** when the game has no walk-back
   field.
   - `Spots::placed` now always puts the item at its spot.
   - The inventory model's new item data reads the path position
     (`LifecycleHooks::path_xy`).
3. **An item dropped from the cursor was never re-announced.** The
   desk's item mode set neither set unit flag 0x1 nor queued the unit,
   so §6.3 part 2 never fired. It now does both.
4. **Picked-up gold stayed on the client's ground.** Nothing sent
   removals: `send_removed_units` was a no-op. Freed ground items are
   now recorded and sent as S→C 0x0A, and tick step 7 frees the
   records.
5. **Chests dropped nothing in the play host.** `NoSpot` was used
   without the walk-back field. They now use `StartSpot`, as monster
   drops already did (REC-108).

Synthetic data, not code breaks:

- the smoke item rows, the two TCs and the smoke monster
  (`app/synthetic_items.rs` `smoke`);
- `compactsave` on gold, potions and scrolls;
- a property-less charm suffix (a charm needs an affix);
- in the test only, the base stats a new sorceress starts with
  (strength, dexterity, level, life), because the synthetic `charstats`
  give none.

### Expectations changed

These follow the spec (`inventory-moves.md` §6.3: one ground message per
tick in which the item changed). Each test had encoded the missing
message, several with a comment saying it was not wired:

- `d2-server` `moves/tests.rs` (6 tests);
- `prop_unified_items.rs`: the host settles one tick, and the pass
  check ignores the §6.3 actions 0 / 2 / 3, which no dispatcher row
  sends;
- `d2-sim` `death.rs` `without_the_field_the_drop_keeps_the_free_spot_seam`:
  the item is now placed;
- `e2e_single_player.rs` and `e2e_full_loop.rs`: the join's ring add,
  the gold pile add and removal, the cap drop, a room add during a walk,
  and the handled counts.

## Still broken or not covered

- **Any walk produces a rejected S→C 0x96.** The client's walk-verify
  check needs the visibility predicate `0x004DBF20` (`model.md` OQ7),
  and the play app never sets one (`Bridge::set_visibility` has no
  caller). This is not an item flow, so the smoke test keeps the player
  still: drops are placed within the pick-up reach.
- **A far pick-up does nothing.** `walk_to_item` is unanswered in the
  preview rest, and the client has no hover-and-walk
  (`controls.md` §6 r9.2).
- **Ground item positions live in two places.** The desk keeps them in
  the item data, while the path record keeps the drop spot. After a
  cursor drop the path is stale.
- **Not reached in the time box:**
  - equip / unequip / weapon swap (W) with requirements;
  - socketing;
  - the charm's effect;
  - identify by scroll and by Cain;
  - using potions and scrolls;
  - stash and cube moves;
  - tooltips and the character panel totals.

  The smoke rows for these are in place: the `axe ` has a strength
  requirement, the `ssd ` has 2 sockets, and the gem, `isc `, `tsc `
  and `cm1 ` are there. The next session can extend the same rig.
- The removal records cover ground items only.

## The user's local check

```
cargo run -p d2-client --release -- play --new amazon Test
```

1. Kill a monster in the Blood Moor. Its drop appears on the ground
   where it died (before this branch, nothing was drawn).
2. Open a chest. Its loot appears beside it.
3. Pick up a gold pile beside you. The pile disappears and the gold in
   the inventory rises.
4. Pick an item up to the cursor, close the panel and click the ground.
   The item is drawn on the ground again.
5. Send the log lines with `item` or `0x9C` if a step shows nothing.
