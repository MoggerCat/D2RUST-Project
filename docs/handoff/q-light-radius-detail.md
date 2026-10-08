# q-light-radius-detail: other light sources in the `play` preview

Branch `claude/q-light-radius-detail`. Builds on `stitch-lighting.md`.
Everything here is `d2rs-own, unverified` (rule 10). Provisional note: REC-170
in `docs/HANDOFF.md`.

## Links connected

| Link | Before | Now |
|---|---|---|
| Monster / missile light columns | not read | `world_view/light_sources.rs` `load` reads `monstats`, `monstats2` (`light`, `light-r/g/b`) and `missiles` (`Light`, `Red/Green/Blue`); `app/play.rs` `add_preview_lit` hands them to `PreviewLight::sources` |
| Object light | none | radius `Lit<mode>` / 2, colour from the model's `objects` rows (`ClientWorld::tables.objects`) |
| Light list | only the player's record | `PreviewLight::refresh` adds one plain record per live monster, object and missile with a radius (§8 rows) |

`add_preview` keeps its signature (player light only); `add_preview_lit` is
the new entry the live play path uses.

## PROVISIONAL (REC-170)

- Records are rebuilt every frame from the units, not kept in `ClientWorld::lights`: no radius walk (§6.4), missile flicker, umod 3 light, skill cast light, overlay light, Den of Evil light.
- Monster radius is `monstats2` `Light` only (no component `L_c`, no level 8 quest override); dead monsters give no light.
- Missile `Light` gate (high quality) taken as on.

## Left (not done)

- Per-block floor / wall gradients (§11 r2–r3): the tile art carries one flat `ShadeChain`; needs a per-block light grid in `TileArt` and the draw.
- Blocks-light flags (§4): needs the collision point test on the client map.
- `Levels.txt` / near-room ambient and scripted overrides (§3, §10).
- Act day/night: already advanced by the environment (stitch-lighting); no new work.

## Your local check

```
cargo test -p d2-client --lib light_sources
cargo test -p d2-client --lib preview_light
D2_GAME_DIR=<D2 install> cargo run -p d2-client -- play --new sorceress Test
```
Expect: besides the player's circle, monsters with a `Light` value (for
example fallen shamans), lit objects (torches, fires) and spell missiles
brighten the ground around them; `D2RS_FULLBRIGHT=1` still gives the flat
full-bright view. If the live table read fails a warning
`light rows (d2rs-own, unverified)` is logged and only the player lights.
