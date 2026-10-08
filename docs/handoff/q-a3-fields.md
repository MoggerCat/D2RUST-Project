# q-a3-fields: Act III jungle and Kurast

Stitching session `q-a3-fields`, branch `claude/q-a3-fields`.

## Finding

The Act III jungle placer, Kurast chain and generator (`d2-sim` `drlg/outdoor/act3.rs`, `jungle.rs`, `kurast.rs`) were unit-tested on hand-built vectors but never ran through `Drlg::create(2, ..)` over game-shaped tables, a streamed room pass, or a played session. They work: no sim change was needed. The act-generic play host (`Session::new_in_act`, REC-134) takes act 2 as is.

## Links connected

| Link | Before | Now |
|---|---|---|
| Act III-shaped synthetic set | none | `crates/test-fixtures/src/act3.rs`: levels 0..=83, LevelTypes 20 / 21 / 22, §1 sizes, lvlprest rows 1..=658 (`Def` = row; jungle pieces 530..604 at 32 × 32 with the spec's `Files`, head and tail 64 × 32 `Files` 0, causeway 48 × 16, Travincal pieces), a floor DT1 with the main-index-1 tile the jungle / Kurast floor cells (`0x120000` / `0x100000`) look up |
| Placer + Kurast chain end to end | untested | `tests/act3_game.rs::the_jungle_placer_and_kurast_chain_link_the_levels`: docks rect, three 64 × 192 jungles ordered by y with 76 touching the docks, 79..83 centred on 78 and climbing north (§9.2) |
| Generate + stream of 75..83 | untested | `every_act3_level_generates_and_streams`: jungles give the recorded 192 rooms each, Travincal 64 preset rooms, no waypoint room flag (no Act III waypoint placer, §4), no provider error |
| Player walk and population | untested | `tests/act3_play.rs`: a session in act 2 starts in Kurast Docks, walks Spider Forest → Great Marsh → Flayer Jungle → Lower Kurast → Bazaar → Upper Kurast → Causeway → Travincal; Spider Forest is populated and the client gets 0xAC for its monsters; nothing rejected |

## PROVISIONAL

REC-138 (`docs/HANDOFF.md` §7). Made-up data; no `Covers:` claim.

## Left

- Outdoor waypoint objects (Act III Waypoints in Kurast come from preset content): waypoint travel to Spider Forest / Great Marsh / Flayer Jungle / Lower Kurast / Bazaar / Travincal is untested.
- The synthetic Kurast pieces are one 8 × 8 cell each; real piece sizes and the random-placer fits (`outdoor.md` §9.4) run against 1.14d `lvlprest` only locally.
- `single_player.rs` untouched; with game files `WorldTypes` already builds every act.
- The walk helper stages along each level edge (the jungles meet along a stretch only) and dodges lvlsub obstacles by lateral offsets; it lives in `act3_play.rs`.

## The user's local check (game files)

```
cargo run -p d2-client --release -- play --new sorceress Test
```
Take a save in Kurast Docks (Act III), walk north: Spider Forest, Great Marsh, Flayer Jungle, then the Lower Kurast → Bazaar → Upper Kurast chain, the Causeway and Travincal. Expect jungle tiles with river and clearings, monsters, a waypoint object in each waypoint level, no panic, no `rejected` line. Synthetic check: `cargo nextest run -p test-fixtures --test act3_game --test act3_play`.
