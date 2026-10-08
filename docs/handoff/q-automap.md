# q-automap: the automap in the play preview

Branch `claude/q-automap`. Nothing here is verified against 1.14d (rule 10): the fills are marked `d2rs-own, unverified`; open point REC-47 in `docs/HANDOFF.md` §7.

## Links connected

| Link | Where |
|---|---|
| Session built from the live tables (`Leveldefs` Layer / LevelType, `Levels` Act, `monstats` → `monstats2` automapCel, `objects` AutoMap, `fixed.automap` picker) | `app/automap.rs` (`live_source`, `add_automap`) |
| Session and cel source handed to the world view | `app/play.rs` (live data only; map files next to the character save, else none), `WorldViewState.automap_view` |
| Tab toggle, per-frame reveal, act load, teardown | already wired (`present.rs`); teardown is now also run when the window closes (`play.rs`) |
| Draw sink: the pass (`Automap::draw_pass`) → frame items; cel files read from the archives on demand; lines drawn as 1-pixel cels; the player marker | `world_view/automap_view.rs`, one call in `present.rs` |

Tests: `world_view/automap_view_tests.rs` (closed draws nothing; open draws cells and marker lines; a missing cel file is logged once; line rasteriser).

## PROVISIONAL (all in the module doc of `automap_view.rs`)

- Every cel draws opaque (fade draw modes of §10 r4 need blend tables the preview does not push).
- No header / name text (§11 r7, §13): no text path for them yet.
- Drawn in pass 9, between world and panels (not "UI pass step 3").
- Only the local player is a marker unit (near-room unit lists are not turned into markers); player byte +0x18 = 0.
- Marker palette: nearest match on the presented palette.
- Options store is in memory (no registry); no key for mini/full (spec: options menu only, §8 r3), F10 / F11 / F12 / V (fade, party, names, side) are not bound.

## Left

Mini-map toggle (needs the options menu or a key), fade modes, header and name text, monster / object cels as markers from the near rooms, bind F9 (re-centre).

## Local check (needs `game/`)

```
D2_GAME_DIR=<install> cargo run -p d2-client -- play
```
Walk around town and out; press Tab: the revealed cells of the map draw over the world with the player's cross marker; Tab again hides it. Expected log: no `automap (d2rs-own, unverified)` warning (a warning names a cel file the archives lack).
Unit tests (no game files): `cargo test -p d2-client --lib automap_view`.
