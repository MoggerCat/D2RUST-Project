# q-warps: level transitions through warp tiles

Stitching session `q-warps`, branch `claude/q-warps`. Read `specs/`,
`docs/`, `crates/`, `tools/` only.

## The path, with the links that were missing

| Link | State before | Now |
|---|---|---|
| DRLG warp link (`drlg/rooms.md` §3.3) and the warp tile preset (`path-placement.md` §12.1) | existed | unchanged |
| Tile units on the server | none: population places only type-1 presets | `View::spawn_preset_tiles` (`d2-sim/src/wiring/action/warp_tile.rs`), called from `WorldSim::spawn_presets` (tick step 3) before the monster walk, once per DRLG room (flag 0x4000000, `rooms.md` §8 r6; q-fix-warp-tile-unit / -restore: the live level types add the presets through `WorldTypes::warp_unit`, deactivation stores and frees the tiles, the restore re-creates them) |
| S→C 0x09 AssignLevelWarp, client model | existed | unchanged (the tile reaches the client's `ClientWorld.units`) |
| Client interact sender for a tile | `bridge/objects/interact.rs` ignored type 5 | sends C→S 0x13 (type 5, GUID), as for the other unit types |
| Server 0x13 with unit type 5 | no route | `world::route` → `System::Warps` → `WorldHost::warp_tile` → `ActionSim::warp_tile_message` → `View::warp_tile_message` → `wiring::path::place::warp_player` |
| `LevelView::warp_destination` (`0x006195A0`, §12.2 r1) | trait default `None`, so every warp was `NoDestination` | `wiring/path/warp_dest.rs`: the source room's link of the tile class, the destination room's link back, the destination room streamed (`stream_room`), its tile of the back record's class (spawned if missing) |
| `ExitWalkX/Y` | not in `DrlgData` | `DrlgData::warp_exits` (parallel to `warps`, from `Lvlwarp`) |
| Placement, 0x07 / 0x15 / 0x0D, room switch | existed (`warp_player`, `place_unit`) | unchanged; the client's level, tiles and units follow from them |
| Synthetic world | town ↔ Blood Moor border only | Den of Evil (level 8) joined to the Blood Moor by a warp pair (`single_player.rs`: `DEN_OF_EVIL`, `BLOOD_MOOR_TO_DEN`, `DEN_TO_BLOOD_MOOR`, lvlwarp rows, `preset_units` of the synthetic `Types`) |

Test: `crates/d2-client/tests/app_level_warp.rs`: the tile reaches the
client, `Bridge::interact` on it, the server player ends in level 8, the
client model follows (player level 8, the Den's way-back tile in
`units`), no rejected message. Fails before the change (checked: with the
client's tile arm removed it never leaves the Blood Moor).

## PROVISIONAL (REC-99 in `docs/HANDOFF.md` §7)

- Where 1.14d allocates the tile units, and what the 0x13 tile case checks
  before `0x005550B0` (the spec names only the caller `0x00548C32`): no
  range test here.
- `client/model.md` §8 rule 7 lists no tile case for the interact sender;
  `ui/controls.md` §6 r9.2 sends 0x13 (T, g). Followed the latter.
- Everything is marked `// d2rs-own, unverified`; nothing counts as done.

## What's left

- Walking onto the tile (not clicking it): the preview's click path
  (`world_view/interact.rs`) sends the interact on arrival for the
  hovered unit; whether a tile can be hovered / picked in `play` is
  untested (the synthetic test calls `Bridge::interact` directly).
- Warps that change the act go through `Pending::warp` (the act change is
  not wired); only same-act warps are covered.
- Palette / act switch on a cross-act warp comes with the 0x09-driven
  act change, not tested here.

## The user's local check (game files)

```
cargo run -p d2-client --release -- play --new sorceress Test
```

Walk east out of the Rogue Encampment to the Blood Moor, then left-click
the cave entrance of the Den of Evil (north-west of the Blood Moor).
Expect: the player walks to it, the screen switches to the Den's tiles
(the first cave level), no panic and no `rejected` line in the log. If
the click does nothing, the tile is not pickable yet (see "What's left");
record it and the console lines.
