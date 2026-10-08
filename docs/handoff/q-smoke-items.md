# Handoff: item smoke test (`claude/q-smoke-items`)

> Readiness check, 2026-10-08. Synthetic data only; nothing verified
> against 1.14d (rule 10). PROVISIONAL points: **REC-281**
> (`docs/HANDOFF.md` §7). Builds on staging's REC-260 ground
> announcement (merged, one implementation).

## What runs

`crates/d2-client/tests/smoke_items.rs` runs the real play path
headless: the app's synthetic game on its server thread, the Bevy app
(`add_game`, `add_walk`, `add_client_data`, the item tables the play app
installs), the bridge and the client model. No window.

After every step the rig checks:

- no C→S intent the server dispatch refused;
- no S→C message unowned, dropped, rejected or discarded since the join;
- the client's items equal the server's store, compared by GUID →
  (code, mode, body in mode 1, page, x, y);
- no panic.

| Test | Covers |
|---|---|
| `drops_pick_up_grid_belt_cursor_and_ground` | Monster drop, chest drop, gold pickup (pile removed, gold stat), auto pickup with auto-equip, auto pickup to the grid, cursor ↔ grid, drop to the ground, potion cursor ↔ belt |
| `a_far_pick_up_runs_to_the_item_and_picks_it` | 0x16 at distance ≥ 5: the server runs the player there and picks the item up on arrival |
| `equip_unequip_requirements_and_weapon_swap` | Cap on the head and the defense total, unequip (0x1C), sword in the right hand, W (0x60) both ways, axe refused at strength 10 and worn at 40 |
| `sockets_charm_identify_potion_and_scrolls` | A gem in the sword's socket. An unidentified charm counts for nothing; identified by scroll (0x20 then 0x27), its +3 strength counts on both sides. Drinking a potion from the belt raises life. A TP scroll read in town is used up |
| `stash_cube_and_cain_identify` | The stash opened, the cap in and out (page 4), closed; the cube page in (page 3), the cube opened by its use, the cap out, closed; Cain identifies the charm for 100 gold |

## Breaks found and fixed

1. **Drops had no position** without the walk-back field. They are now
   placed at their spot, and the model reads the path position for a
   new ground item.
2. **A cursor drop was not re-announced** (REC-260 announced each item
   once). Announced items that leave the ground are forgotten.
3. **Picked-up gold stayed on the client's ground.** Freed ground items
   now get removal records and S→C 0x0A.
4. **A far pick-up did nothing.** `walk_to_item` was unanswered. It now
   runs the player to the item and picks it up on arrival
   (`world/item_approach.rs`).
5. **A ground item's path kept its old spot** after the desk moved it.
   `add_to_room` now moves the static path.
6. **White drops were unidentified,** so they could not be worn and an
   auto pickup could not auto-equip them. Low or normal drops without
   affixes are now identified; charms stay unidentified.
7. **Auto-equip on pickup left the item nowhere.** `equip_picked`
   (§4.9) was unanswered. It is now implemented on the model
   (`equip_without_cursor`).
8. **The weapon switch moved items silently.** The moved items now get
   command flag 0x200000 and the owner is refreshed, so 0x9D action
   0x17 is sent.
9. **The armor's defense was missing from the character total.** The
   client's item list now carries stat 31.
10. **Stash inserts were refused.** The desk did not know the player was
    in town; it now reads the town test from the DRLG.
11. **The stash could not be closed in the synthetic game.** 0x4F
    needed cube parts, and the synthetic game had none; it now has cube
    data with no recipe.
12. **An identified charm stayed inactive on the server.** Identify now
    runs the inventory pass.

Expectations changed (each spec-backed; reasons are in the commits):

- `death.rs` free-spot test: the item is now placed;
- `e2e_full_loop.rs`: the gold removal, and the handled count;
- `app_play_objects.rs`: the chest class's six items;
- `prop_unified_items.rs`: auto pickup now auto-equips (§4.9 is
  written);
- `mutants_wiring_inventory`: `equip_picked` is answered on the desk.

## Still broken or not covered

- **Every walk produces a rejected S→C 0x96** (the visibility
  predicate, q-fix-visibility's). The far pick-up test passes the
  predicate as an input; the other tests keep the player still.
- **Tool tips are not covered.** `ItemTips` loads from the user's
  tables, so this is only checked by the local run.
- **One ground message per landing.** 1.14d sends §6.3 part 2 in every
  tick in which the item changed.
- **Removal records cover ground items only.**
- **A pick-up can stack drops on one cell:** `StartSpot` has no
  collision search (REC-108).
- **The equipped sword's socketed-gem stats** on the wearer were not
  checked.

## The user's local check

```
cargo run -p d2-client --release -- play --new amazon Test
```

1. Kill a Blood Moor monster. Its drop appears where it died.
2. Click an item a few steps away. The character walks there and picks
   it up.
3. Pick up a white helm or weapon with an empty slot. It goes straight
   onto the body.
4. Press **W**. The weapons in the hands swap with the second set in
   the inventory panel.
5. Open the stash and drag an item in and out, then close it. Moving
   again works with no stuck state.
6. Identify a charm with a scroll. The character screen's total rises
   by its bonus.
7. Hover items. The tool tips show names, requirements and properties.
   Send a screenshot if a line is missing.
