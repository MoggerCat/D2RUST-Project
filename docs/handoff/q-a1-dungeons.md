# q-a1-dungeons: Act I's dungeons in the synthetic world

Branch `claude/q-a1-dungeons`. Nothing here is verified against 1.14d (rule 10). Open point: REC-249 (`docs/HANDOFF.md` §7).

## Links connected

| Link | State before | Now |
|---|---|---|
| Act I levels past the Burial Grounds / Tower | only Black Marsh, Den, Cave 1, Tower 1–6, Burial Grounds, Cold Plains → Dark Wood → Tamoe → UP1 chain | `app/synthetic_chains.rs` is a tree (`ACT1_TREE`, parent, vis slot, child) plus the Act III and V lines: Black Marsh → Dark Wood → Tamoe → Monastery Gate … Jail 1–3 … Cathedral → Catacombs 1–4; branches UP1–2, Hole 1–2, Pit 1–2, Cave 2, Crypt, Mausoleum, Tristram |
| Several ways on per level | a chain level had slot 0 (back) and slot 1 (on) only | any vis slot: flags `WARP_0 << slot`, preset tiles (`tile_xy`: slots 0/1 on the diagonal as before, further slots off it) and the DRLG vis/warp rows come from `edges()`; existing hosts (Black Marsh, Burial Grounds, Cold Plains) get the extra tiles after their own |
| Catacombs 4 room | fixed rect (0, 24), overlapping the Burial Grounds | placed with the other tree levels |

Test: `crates/d2-client/tests/app_a1_dungeons.rs`: from the Blood Moor the player clicks the warp tile of each level on the way to Catacombs 4 (Black Marsh, Dark Wood, Tamoe, Monastery Gate, Outer Cloister, Barracks, Jail 1–3, Inner Cloister, Cathedral, Catacombs 1–4), then back one level, then enters and leaves every branch once. The server player and the client model follow each step, nothing is rejected. Before the change level 26 and the other dungeons had no tile. `app_levels_warps_all` (every warp tile of every level) still passes over the larger tree.

Expectation changed (not weakened): `app_frame_loop`: Cold Plains's warp tile now leads to Cave Level 2 (level 13, room at tile (24, 40)) instead of the Dark Wood (level 5, (8, 40)); the Dark Wood moved under the Black Marsh so the Blood Moor's tile count and the "unvisited level is freed" test stay as they were.

## PROVISIONAL (REC-249)

Parents, slots, ids, tile places and level neighbours are made up (`// d2rs-own, unverified`). The caves, Tower cellars and Den are still the maze levels of earlier tasks; the other dungeons are flat 8×8-tile rooms, not maze or preset builds from `drlg/maze.md`.

## Left

- Real dungeon builds (maze/preset rooms from the specs) for the flat dungeon levels, and warp tiles from their DS1 units.
- Stony Field stays unbuilt (the waypoint test needs it so); Tristram hangs off the Dark Wood here, the original uses Stony Field.

## The user's local check (game files)

```
cargo run -p d2-client --release -- play --new sorceress Test
```
With the live data the dungeons are built from the real specs; walk Rogue Encampment → Blood Moor → … → Catacombs 4 and note any level whose exit is not clickable plus the console lines. The synthetic walk needs no game files: `cargo nextest run -p d2-client --test app_a1_dungeons`.
