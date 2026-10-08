# q-a5-fields: Act V fields

Stitching session `q-a5-fields`, branch `claude/q-a5-fields`.

## Finding

The Act V placement and build (`d2-sim` `drlg/outdoor/act5.rs`, the A5 / A5T linkers, type-12 border substitution) were unit-tested on small fakes but never ran through `Drlg::create(4, ..)` over game-shaped tables, a streamed room pass, or a played session. They work: no sim change was needed. The act-generic host (`Session::new_in_act(.., 4)`) takes act 4 as is.

## Links connected

| Link | Before | Now |
|---|---|---|
| Act V-shaped synthetic set | none | `crates/test-fixtures/src/act5.rs`: levels 0..=136 (109 preset, 110 / 111 / 112 / 117 outdoor, 134 and 136 for the A5U driver), LevelTypes 29 / 30 / 31, §1 sizes and offsets, lvlprest rows 1..=990 (`Def` = row; 2 × 2-cell pieces, 2 × 6 siege strip 865..879, 955 / 956 with 2 files), a floor DT1 with main indexes 0, 1, 6, and a type-12 `lvlsub` file whose three groups stamp the three prison pieces (915..917) |
| Placement end to end | untested | `tests/act5_game.rs::the_act5_placement_links_the_levels`: rects of 109 / 110 / 111 / 112 / 117 equal the `outdoor-act3-act5.md` vector (the same seed's B1 / B2 draws) |
| Generate + stream of 109..117 | untested | `every_act5_outdoor_level_generates_and_streams`: the Foothills siege strip gives 15 pieces (180 rooms of 8 × 8), 111 / 112 / 117 build through barricade, ravine, entrances, caves, siege connection, border substitution, prisons and specials with no provider error and no waypoint room flag |
| Player walk and population | untested | `tests/act5_play.rs`: a session in act 4 starts in Harrogath, walks to Bloody Foothills, Frigid Highlands (populated, client gets 0xAC) and Arreat Plateau; nothing rejected |

## PROVISIONAL

REC-146 (`docs/HANDOFF.md` §7). Made-up data; no `Covers:` claim.

## Left

- Waypoint objects (Act V waypoints in Harrogath / Frigid Highlands / Arreat Plateau come from preset content): waypoint travel is untested.
- Frozen Tundra (117) generates but the player does not reach it (it joins only through cave presets and warps).
- Ice caves (113..116, 118, 119) and Nihlathak's temple / halls are mazes/presets outside this row's synthetic set.
- Barricade rows are walls in the original; the synthetic tiles have no collision, so the walk goes along the middle rows only to keep clear of the border rows (the stall was seen on the north border row).
- `single_player.rs` untouched; with game files `WorldTypes` already builds every act.

## The user's local check (game files)

```
cargo run -p d2-client --release -- play --new sorceress Test
```
Take a save in Harrogath (Act V), walk west: Bloody Foothills (siege strip), Frigid Highlands, Arreat Plateau. Expect snow tiles with barricades and the ravine, prisons in Frigid Highlands, monsters, a waypoint object in each waypoint level, no panic, no `rejected` line. Synthetic check: `cargo nextest run -p test-fixtures --test act5_game --test act5_play`.
