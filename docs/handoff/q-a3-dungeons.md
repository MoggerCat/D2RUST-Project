# q-a3-dungeons: Act III dungeons in the play host

Stitching session `q-a3-dungeons`, branch `claude/q-a3-dungeons`. Nothing here is verified against 1.14d (rule 10). PROVISIONAL: REC-139 (`docs/HANDOFF.md` §7).

## Finding

The maze generator and the Act III builders (`drlg/maze.md` §3.3, §6: Spider, Dungeon, Act 3 Sewer, Kurast/Durance types) were in `d2-sim`, but nothing ran them as Act III levels: the fixture sets were Act I and II only, and the app's synthetic world (`synthetic_maze`, `synthetic_tower`) is act 0, whose DRLG cannot hold level ids 75+ (`levels.md` §6.3). So the Act III line is built on the act-generic host (`q-act-worlds`), not in the act-0 app world.

## Links connected

| Link | Before | Now |
|---|---|---|
| Act III-shaped set | none | `crates/test-fixtures/src/act3.rs` (q-a3-fields' set, extended; one Act III set): levels 84..=102 added to its 0..=83 (Kurast Docks → Travincal), maze levels 84–93 and 100/101 with their level types and `lvlmaze` rows, the temples 94–99 and Durance 3 (102) as preset levels, lvlprest `Def` = row to 1100 |
| Maze levels through `Drlg::create(2, ..)` (`TownOnly`) | untested | `tests/act3_dungeons.rs` (on a `TownOnly` DRLG): every maze level generates and streams as preset rooms only, no provider error; Spider Cave/Cavern are 4 cells, Sewers 1 is 18 cells (ring(5) + 2 stamps, spec §5.1/§6), Dungeon levels grow past the ring; a rebuild is identical; the temples and Durance 3 build |
| A play `Session` in act 2 | untested | `tests/act3_play.rs`: `Session::new_in_act(.., 2)` starts in Kurast Docks, runs frames clean; every dungeon level is built and streamed inside the live game's own DRLG next to the town, no rejected message |

Merged with staging (q-a3-fields); `lvltypes` now runs to 25, `LEVEL_COUNT` 103. `synthetic_maze.rs` / `synthetic_tower.rs` untouched: see Finding.

## PROVISIONAL (REC-139)

Cell size and content, `Rooms` / `Merge` / level rect of the lvlmaze rows, the vis chains, the preset temples. `Merge` 500 matches the Spider Cavern vector's draw (367 < 500). All made up.

## Left

- A player cannot enter a dungeon: the entrances are DS1 warp units of the outdoor levels (Spider Forest, Great Marsh, Flayer Jungle, Lower Kurast, Kurast Bazaar …), and Act III outdoors (jungle placer, `outdoor-act3-act5.md`) needs block data this set lacks. The host also has no warp step (the act-0 app does: `q-warps`).
- Spider Cavern def vector (`maze.md` Test vectors: 662/660/659/663): built rooms do not keep their def.
- Stamped objects (Durance 2 / Sewers waypoints, chests, the Compelling Orb) were not looked at.
- Quests: Khalim's Eye/Brain/Heart chests (Spider Cavern, Flayer Dungeon, Sewers) and the Durance of Hate entry belong to the Act III quest session.

## The user's local check (game files)

```
cargo run -p d2-client --release -- play --new sorceress Test
```
Take a save in Kurast Docks (Act III), walk Spider Forest → Spider Cavern / Arachnid Lair, Great Marsh → Swampy Pit, Flayer Jungle → Flayer Dungeon, Lower Kurast → the sewers, and Travincal → Durance of Hate 1–3. Expect maze-built caves joined with no void, the player on a floor tile, no panic and no `rejected` line; note the console lines if an entrance is not clickable. Synthetic check: `cargo nextest run -p test-fixtures --test act3_dungeons --test act3_play`.
