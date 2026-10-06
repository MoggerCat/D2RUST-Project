# Handoff: the treasure drop's FreeSpot on the path provider — `claude/drop-freespot`

> Folded into `docs/HANDOFF.md` (§1–§5, §7, §8) and `docs/PLAN.md` as of the eighth fold (`claude/docs-fold-8`); this file stays as the detailed record.

Cloud implementation session, 2026-10-06, task class: wiring, medium
(METHODS M14). Base: `claude/tender-meitner-mphas3` at `6cd6480` (with
`unify-items` merged: one item store, `ActionHooks::items`). Repo only,
synthetic tables, the synthetic walk-back field `sign_field` (vectors
F1–F3), no game files (M09): every claim below holds on this branch.
Closes **WP2** of `docs/HANDOFF.md` §2 step 7p.

## 1. State

**Wired, unverified** (M02): no rule is added; `treasure.md` §7 step 2
now runs `path-placement.md` §7 / §9 when the path provider is on.

- **Seam change.** `wiring::economy::DropPlacer` is now
  `DropPlacer<H>`: `place(&mut self, econ: &mut Economy<'_, H>, x, y)`
  receives the economy the walk runs on, so a provider reaches the
  rooms and collision through `econ.hooks` (for the death drop:
  `ActionHooks::drlg` and `ActionHooks::paths`). New provided method
  `placed(&mut self, econ, item, spot)` (default: nothing), called by
  `ItemDrops::create` right after a successful creation, before the item
  is recorded in `placed`.
- **Position.** `economy/death.rs` reads `h.path_position(unit)` (the
  path record with the provider on, `Pending::position` without it)
  instead of `h.x.position`.
- **Free spot.** `death.rs`'s `Spots` implements
  `DropPlacer<ActionHooks<X>>`: with `ActionHooks::paths` on **and**
  `PathState::field` loaded, `wiring::path::place::floor_drop(drlg,
  field, room, from = the dropper's position, size 1, fallback true)`
  (§9 rule 1 is §7 step 2's start offset; both room lookups are the room
  then its adjacent rooms, so the start is the same); `Ok((None, _))` →
  no drop, `Err` → `WiringError::Place` in `ActionHooks::errors`.
  Otherwise the `FreeSpot` seam exactly as before (provider off, or on
  without the field: old behaviour, the `FreeSpot` signature is
  unchanged, so `e2e_single_player.rs` / `prop_worldsim.rs` compile as
  they were).
- **Each item sees the previous ones** (§7 step 2). With the provider
  and field on, `placed` runs the path part of `SUNIT_Add`
  (`View::path_place`, `path-placement.md` §2.5) for the created item at
  its spot: mode 3 → static path + footprint (item mask 0x200, inside the
  search mask 0x3E01). So the item has a position (`path_position`) for
  pick-up distance checks and the next drop avoids it. Without the
  provider nothing is placed (old behaviour).
- **Tests** (`wiring/action/tests/death.rs`, 4 new, action fixture,
  monster at (13, 10), player at (10, 10), room A = sub-tiles 0..40):
  - `with_the_path_provider_the_drop_lands_on_the_floor_drop_spot`:
    vector D2 translated (+3, 0): (15, 13); the item's static path at the
    spot and its 0x200 footprint; the monster's position comes from its
    path record.
  - `the_floor_drop_avoids_blocked_cells`: vector D1 translated (wall
    column x = 15 → (14, 13)) and D3 translated (item bit at the start →
    (14, 13)).
  - `a_dropped_item_blocks_the_next_drop`: two drops; the first item's
    footprint moves the second to (14, 13).
  - `without_the_field_the_drop_keeps_the_free_spot_seam`: provider on,
    no field: the seam's answer (the start, even on a wall), no item path.
  - M08: with `placed` disabled and the old `h.x.position` read, all 4
    fail; restored.
  `economy/tests/treasure.rs`'s fake placer ported to the new signature.

## 2. Code map rows (for `docs/HANDOFF.md` §3)

| Path | What | Spec |
|---|---|---|
| `crates/d2-sim/src/wiring/economy/treasure_items.rs` | `DropPlacer<H>::{place(econ, x, y), placed(econ, item, spot)}` | `treasure.md` §7 |
| `crates/d2-sim/src/wiring/economy/death.rs` | `Spots`: floor drop on the path provider (`path-placement.md` §9) else `FreeSpot`; item path at the spot (§2.5); position by `path_position` | `treasure.md` §3.1, §7; `path-placement.md` §2.5, §7, §9 |

## 3. Signature changes

`d2-sim` (public): `wiring::economy::DropPlacer` → `DropPlacer<H>`,
`place` takes `econ: &mut Economy<'_, H>` first; new provided method
`placed`. `ItemDrops`'s `DropSink` impl requires `P: DropPlacer<H>`.
`FreeSpot`, `monster_death_drop`, `DeathDrops` unchanged.

## 4. Readings and open questions

- **DF1** (kept, `death.rs` `TODO(treasure.md §7 step 2)`): the room
  passed to `0x0064E810` is read as the room the start-offset search
  found, else the monster's room. `path-placement.md` §9 says treasure
  passes "the same arguments" as `0x00555DA0`, whose room is the
  caller's; the hint is replaced by the first successful lookup, so the
  two readings differ only when a ring cell lies outside the start room
  and its neighbours but inside the monster room's. Spec question.
- **DF2** (`placed`): `0x00558D90` → `0x00555230` allocates the item; the
  path part (`SUNIT_Add` §2.5) is not run by `Economy::create_item`
  (it has no position: `ItemSpawn` carries none). The drop runs it from
  `placed`, at the spot. Other economy creations (cube output, vendor,
  quest drops on the floor) still give a ground item no path; a later
  task can move the §2.5 call into `create_item` once `ItemSpawn`
  carries a position.
- **DF3**: without the provider the drop still calls no
  `Pending::place` for the item (unchanged); with the provider and no
  field the item gets no path either (the `FreeSpot` branch is the old
  behaviour as a whole).
- The hosts (d2-server `WiredWorld`, the e2e) do not set
  `PathState::field` yet (no `ExpField.D2` loading outside game files):
  the server path keeps `FreeSpot` until the host loads the field and
  enables paths (step 7p's server handler).

## 5. Local checks to queue

None new runnable now. When `expfield_live` (§5 C38) has run, a drop
replay (monster kill with a recorded item position, R1–R3 style) through
the wired host with the provider on and the real field: compare the
item's (x, y) with the recording.

## 6. Gate

`sh tools/gate.sh` on this branch: **GATE: PASS**, every step (after
`tools/cloud-setup.sh` for the client's wayland libraries). d2-sim +
conformance 2,038 tests, rest 599, d2-client 363; coverage 4,139
claims, 0 errors.
