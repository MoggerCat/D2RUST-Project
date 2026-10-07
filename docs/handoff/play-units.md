# Play preview: units (session S-B)

> Implementation session, 2026-10-07, branch `claude/play-units`. Scope:
> `docs/handoff/first-playable-scope.md` §5 row S-B (gaps G6, G7, G9),
> under decisions D1–D3 (`docs/PLAN.md` decisions log). Read only
> `specs/`, `docs/`, `crates/`, `tools/`. Nothing here is verified
> against 1.14d: every preview fill is marked `// d2rs-own, unverified`.

## What was done

| Gap | State | Where |
|---|---|---|
| G6 real `ViewRules` for units | **closed (preview)** | `world_view/unit_rules.rs`: `UnitRules<R>` answers `unit_pose`, `unit_params`, `component_frame` / `component_slot_frame`, `shade`, `blend` through `rules::unit_composite` (§2 COF name, §3 r4 direction, §5.1 request, §6 cel); `tiles`, `place` and the UI hooks go to the wrapped rules |
| G7 COF / DCC / DC6 loader | **closed** | `world_view/unit_assets.rs`: `UnitLooks::live` reads `plrtype`, `plrmode`, `monmode`, `objmode`, `composit`, `monstats`, `monstats2`, `objects` `.bin` from the user's archives; `UnitArtLoader::ensure` loads each listed unit's COF into `ViewAssets.cofs` and every direction of each component file into `ViewAssets.frames` (format by §6 r2), once per file |
| G9 component inputs | **D1 fill only** | player: no items, no body armor (every armor class `lit`, weapon class `hth`); monster: choices all 0, no `monstats2` counts (every component `lit`), weapon class = `monstats2` `BaseW` (§2.1) |
| Install | done | `app/ui.rs` `install_unit_rules` (called by `add_original_ui`): rules are now `PanelArtRules { UnitRules { Unspecified } }`; a `PreUpdate` system `load_unit_art` (after `mirror_units`) makes the frame's unit art resident before the world view draws |
| Edge clip (D1) | done, **off by default** | `rules/view.rs` `OriginalView::edge_clip` / `.with_edge_clip(true)`: a cel cut by the frame edge is placed and left to the frame clip instead of failing the frame. Strict path unchanged |
| Atlas guard | done | `frames/atlas.rs`: the packer's `expect` is `AtlasError::TooLarge` |

D1 preview fills in `UnitRules` (module doc): `dir64` = 0 (the model holds
no client path direction); frame = the model's +0x44 frame when non-zero,
else the COF animation rate (8.8) × server ticks mod COF frames; no COF box
pre-test (§4); draw key pass 6 at 0/0 unless `OriginalView`'s source has a
draw order (it overwrites); shade none (full bright); blend opaque (COF
layer translucency not applied). Any unit art that fails to load is
skipped with one `warn!` line and never retried; a unit whose COF is
missing is not drawn (§2 r4); a slot whose file is missing draws nothing
(§5 r2).

Tests (synthetic fixtures only, `world_view/unit_rules/tests.rs`):
COF / component names from the looks; an object loads once, draws and
animates by tick; missing files log once and skip. `rules/tests.rs`:
`the_preview_edge_clip_places_a_cut_cel` (beside the strict
`a_cut_top_down_unit_cel_is_an_error`, still passing).

## Seams other sessions must connect

1. **S-A (`world_view/feed.rs` / `preview.rs`)**: build the preview's view
   with `OriginalView::new(camera, rules, source).with_edge_clip(true)`
   (both `build_lit` arms and `NoWorld`). Without it, a unit near the
   frame edge still fails the frame (or is skipped by S-A's per-item
   policy, G19).
2. **S-A (G8)**: `ViewSource::unit_offset` and `unit_position` must answer
   for every near unit; `OriginalView::place` calls them for each cel. With
   `NoFeed`'s refusal the unit errors.
3. **S-A (`app/play.rs`)**: if play replaces `WorldViewState.rules` after
   `add_original_ui`, keep the unit rules: wrap with
   `app::ui::install_unit_rules(app, source)` (returns
   `UnitRules<Unspecified>`; call it once, it also adds the loader
   system). With game files today, `add_original_ui` already installs them.
4. **S-A light**: when the feed states a light, `LitRules` answers unit
   `shade` / `blend` and mine are not asked (intended).
5. **`world_view/mod.rs`** (shared): two lines added after `pub mod
   ui_bind;`: `pub mod unit_assets;`, `pub mod unit_rules;`.
6. **S-D (`bridge/predict.rs`, D2)**: the walk direction is not in the
   model; once the client steps the player, a `dir64` per unit (path
   direction) can replace `UnitRules::dir64` (now 0 for every unit).

## Local checks for the user (needs `D2_GAME_DIR`)

```
set D2_GAME_DIR=C:\path\to\Diablo II
set RUST_LOG=warn,d2_client=info
cargo run -p d2-client --release -- play --new amazon Test
```

(`--new` needs S-D; with an existing save use `--save X.d2s`.) Needs S-A's
map and placement (G2–G4, G8) to see units at the right place.

Look for:
- the character standing at the screen centre, light armor, bare hands
  (no items: D1);
- Akara, Kashya, Charsi, Gheed, Warriv and the other town NPCs, and the
  waypoint, cycling through their neutral animation (one COF frame per
  `animation rate / 256` ticks);
- `warn` lines `unit art: <path>: in no archive` only for component files
  of the D1 fills (for example a player `RH` / `LH` `lit` file); a `.COF`
  line for a town NPC or `unit tables not loaded` is a bug to report;
- no `atlas` / `frame ... exceeds` error; no panic.

Every unit drawn is **unverified** (rule 10): direction 0 for all, no
item looks, full bright, opaque. Queue in HANDOFF §5 once S-A and S-D
are merged: the capture compare cases for units, REC-51 for walk motion.

## What is left

- Monster component choices from S→C 0xAC (`client/msg-units.md`) into
  `MonsterLook.choices`, with `monstats2` counts and `compcode` codes
  (G9 proper); act II override tables need `act_two` from the room.
- Player looks from items (G16 / `msg-stats-items.md` OQ3), then §2.1
  weapon class from hands.
- Unit direction from a client path record; §3 r3 expected direction
  counts; §4 COF box pre-test (needs the screen position in `unit_pose`).
- COF layer translucency (`blend-modes.md` §3) and §7 colormap source
  (`shading.md` §6) in place of the opaque / full-bright fills.
- Animation from the model (+0x44 advance by the mode machines) instead of
  the tick fill.
- `docs/PLAN.md` checklist rows for G6 / G7 / G9: left to the coordinator
  (not an owned file).
