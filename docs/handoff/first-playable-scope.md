# First playable build: scope

> Scoping session, 2026-10-07, branch `claude/playable-scope`. Plan only:
> no code changed. Read from `crates/`, `specs/`, `docs/`, `tools/` (no
> `re/`, no `../refs/`). Nothing built or run; every finding is from reading
> the code, with file:line at the time of writing (`main` = `51c4880`).

Goal: on the user's Windows PC with 1.14d files, `cargo run -p d2-client
--release -- play [--save X.d2s]` should (1) load a save or create a
character, (2) show the Rogue Encampment's tiles, the player and the NPCs,
(3) walk with mouse clicks and the run toggle, (4) if cheap, open the
inventory and character panels and walk into the Blood Moor to see
monsters.

## 1. Where the path stops today

The join is not the problem. With game files, the server joins the
character and the client model gets a local player
(`tests/app_single_player.rs:99`, `tests/app_frame_loop.rs:155`). The
window is black because **the play app draws through placeholder rules**,
and on live data the server **crashes after about 104 ticks**.

| Stage | State | Where |
|---|---|---|
| `main.rs:342` `play` → `app/play.rs:180` `run` | wired | `--save` loads through `single_player::load_character` (`single_player.rs:544`); no flag creates a new character |
| server thread → `d2-server` host → `d2-sim` | wired | `ThreadLink` (`app/server_thread.rs`), `start_with` → `build_with`; acts 0 and 1 from the user's DS1/DT1 through `WorldTypes` |
| join C→S 0x67 / 0x6B → S→C | wired | `d2-server/src/adapters/session.rs:284-395` sends 0x59, 0xAA, 0x76, 0x0B, 0x5F, 0x7B, 0x23, 0x95, 0x03, 0x53; `wiring/path/place.rs:337` sends 0x07, 0x15, 0x7E; the room switch (`wiring/action/switch.rs:177`) adds players (0x59), objects (0x51), tiles (0x09), monsters (0xAC) |
| **live tick ~104** | **panic** | `d2-sim/src/drlg/room.rs:152` `expect("live DRLG room")`: a freed room id is looked up (LOCAL-RUN 4.2 result, `local-buddy-2026-10-06.md`; `triage-game-findings.md:53`, cause unconfirmed) |
| S→C → bridge → `ClientWorld` | wired | `bridge/mirror.rs:64` → `Bridge::frame` (`bridge/mod.rs:158`) → `receive.rs:66` → dispatch (`dispatch.rs:257`, all 182 ids have an owner) |
| model → draw list | **placeholder** | `play.rs:84-88` `rules: Unspecified`, `feed: ModelFeed<NoFeed>`; `app/ui.rs:88` `PanelArtRules { rules: Unspecified }`. `Unspecified` (`world_view/mod.rs:309-360`): no tiles, `unit_pose` = None |
| map | **off** | `play.rs:128-131` builds `ModelFeed` without `.with_map()`; `tile_art` falls through to `NoFeed`'s refusal (`model_feed.rs:259` → `feed.rs:152`); no DT1 is ever loaded into `ViewAssets.frames` |
| units | **no rules type** | `rules::unit_composite::unit_pose` (`rules/unit_composite.rs:587`), `composite::build_with`, `FrameSet::from_dcc`/`from_dc6` (`frames/mod.rs:167/201`) exist; no non-test `ViewRules` impl calls them; `ViewAssets.cofs` never filled |
| palette | wired | `ActPalettes::live` (`app/palette.rs:39`), switched by `palette_act`; `ViewAssets.maps` (PL2 shade / blend tables) stays empty |
| camera | wired | `feed.rs:333-346` `Camera::new` from `feed.player()` each drawn frame |
| compose → Bevy | wired | `present.rs:480` → `build_frame` (`feed.rs:358`) → CPU or GPU node (`present.rs:638-668`, `node.rs`); proven on real GPU (LOCAL-RUN batch 3) |
| input → intents | wired, partial | only with game files: `present.rs:410` → `ui_bind.rs:423` `world_clicks` → `Bridge::world_click` (`bridge/mod.rs:259`) → `controls/click.rs` 0x01/0x03; `mods` hard-coded 0 (`ui_bind.rs:452`) |
| own walk shown | **no** | the server sends the walking player nothing (`pathing.md` §10 r2; `tests/e2e_walk.rs:22`); the client moves a unit only when a message places it (`model_feed.rs:15-17`, `model.md` §3 r3, open question 2) |

Why it was left this way: the strict rule (METHODS M07; `world_view/mod.rs`
docs) says a hook whose owner spec cannot answer from the model is an
error, never a default. The owner specs exist (`render/*.md`), but parts of
their inputs are not in the model: light (no 0x53 handler, no light
records), unit facts (`draw-order-2.md` §15 flags, `unflatDead`,
`DrawUnder`, states), equipped items, monster component choices
(`model_feed.rs:64-97` `PENDING`). **A first playable build therefore needs
decision D1 (§4)**: whether a clearly labelled, unverified preview fill is
allowed for those inputs.

## 2. Gaps

Kind: **W** wiring of existing code, **N** new code from an existing spec,
**S** needs a spec (or a decision standing in for one). Size: S < 100
lines, M 100–400, L > 400. **G** = needs game files to verify.

| # | What is missing (seam) | Spec | Kind | Size | Depends on | G |
|---|---|---|---|---|---|---|
| G1 | Live crash: freed `DrlgRoomId` looked up, `d2-sim/src/drlg/room.rs:152` (likely `near`/link arrays not cleared by `free_inactive_levels`) | `drlg/levels.md` §9.2–9.4, `drlg/rooms.md` §4 | N (bug fix) | S–M | a local `RUST_BACKTRACE=1` run (LOCAL-RUN 4.2) to name the caller | G |
| G2 | Map on: `ModelFeed::with_map()` at `app/play.rs:128-131` | `render/draw-order.md` §9, §10 | W | S | G3, G4 | |
| G3 | `tile_art` answer (DT1 entry → frame set, shade, blend) at `world_view/model_feed.rs:259`; `ViewSource::tile_blocks` | `drlg/rooms.md` §9.3, `render/shading.md` §4, `render/blend-modes.md` §6, `render/lighting.md` §11 | N + D1 (light input not in model) | M | D1 | G |
| G4 | DT1 loader into `ViewAssets.frames` (on demand per near room, like `PanelArtLoader::ensure`, `world_view/panel_art.rs:178`), called before `build_frame` at `present.rs:562` | `client/assets.md` §A4, `formats/dt1.md` | N | M | — | G |
| G5 | `unit_facts` for near-room units (`feed.rs:103`, used at `model_feed.rs:232`) | `render/draw-order-2.md` §15 | N + D1 (flags not in model) | S | D1 | |
| G6 | A real `ViewRules` type at the `state.rules` seam (`app/ui.rs:88`): `unit_pose` via `rules::unit_composite`, `component_frame`, `unit_params`, `shade`, `blend` | `render/unit-composite.md` §2–§10, `render/draw-order.md` §10, `render/shading.md` §6, `render/blend-modes.md` §3 | W + N | L | G7, D1 | G |
| G7 | COF / DCC / DC6 loader into `ViewAssets.cofs` and `.frames` for each listed unit (player class token, NPC and monster tokens from the bridge's unit rows) | `client/assets.md` §A1–§A5, `render/unit-composite.md` §5.1 | N | M | — | G |
| G8 | `unit_offset` answer (`model_feed.rs:138` → `NoFeed` error `feed.rs:247`) | `render/unit-composite.md` §8, `render/sprite-placement.md` | W (code in `rules/unit_composite.rs`) | S | — | |
| G9 | Player / monster component inputs: no items (server sends no item messages), monster components from 0xAC | `unit-composite.md` §5, `client/msg-units.md` (0xAC), `msg-stats-items.md` OQ3 | N + D1 (default looks with no items) | S | G6 | G |
| G10 | `ViewAssets.maps` (PL2 shade / blend rows) empty in play; any lit or translucent draw fails | `render/composition.md` §4 | W | S | — | G |
| G11 | Own walk not shown: client motion of the local player between messages | `client/model.md` §3 r3, open question 2 (REC-51 recording settles it) | S (+ D2) | M | D2 | G |
| G12 | Run toggle / stand still: `mods` = 0 at `world_view/ui_bind.rs:450-452`; `ToggleRun`/`StandStill` exist in `controls/names.rs:139-141` | `ui/controls.md` §4.3 r1, §6 | W | S | — | |
| G13 | New character: no class / name choice; `Character::New` fixed to Sorceress (`app/single_player.rs:157-159`); character-select menus unspecified | no spec (`ui/menus.md` has no select / create menu) | S → CLI stand-in `--new <class> <name>` (D3) | S | D3 | |
| G14 | New character has no starting items: `start_items` "unapplied" (`d2-server/src/adapters/character.rs:466`) | `formats/d2s-load.md`, `data` charstats `item1..10` | N | M | — | G |
| G15 | Character panel values: no S→C 0x1D–0x1F from the join (`session.rs:50-59`, choice not specified); model totals not bound to `ui/original.rs` (PENDING `:62-111`) | `sim/intents-events.md` §8.2, `ui/panels.md` §8.4–8.9, `client/stat-lists.md` | S (send choice) + W (binding) | M | — | G |
| G16 | Inventory contents: no item messages sent (rule 3.5), item stream decode open; `ui/inv_grid.rs` not used by a panel | `msg-stats-items.md` OQ3, `ui/inventory.md` | S + N | L | G14 | G |
| G17 | Town NPCs and Blood Moor monsters on live data: code exists (`monsters/population/preset.rs:316`, `wiring/worldgen/dispatch.rs:145-161`), never checked live; only the first room of levels 1, 3, 40 pre-streamed (`single_player.rs:1147-1170`) | `drlg/rooms.md` §4.1, `monsters/population.md` §11 | W (check) | S | G1 | G |
| G18 | Town → Blood Moor on foot: neighbour levels made on demand (`drlg/room.rs:285-301`), client DRLG builds from 0x07; never tested across a level border | `drlg/rooms.md` §3.3, §4; `client/model.md` §9, §12 | W (test) | S | G1, G2–G4 | G |
| G19 | Strict frame errors: one `ViewError` fails the whole frame (`world_view/mod.rs:77`), so one bad sprite blacks the screen | METHODS M07 | D1 (log + skip per item in preview) | S | D1 | |

Count by primary kind (19 gaps): **W 6** (G2, G8, G10, G12, G17, G18),
**N 8** (G1, G3, G4, G5, G6, G7, G9, G14), **S 4** (G11, G13, G15, G16),
**D1-only 1** (G19). Five N gaps (G3, G5, G6, G9, G19) also need D1. G16 (inventory contents) is not cheap: skip it for the first
build; the inventory panel opens empty.

## 3. Risks: crash or garbage on the first run

| Risk | Effect | Mitigation |
|---|---|---|
| G1 room panic | server thread dies ~4 s in, app exits 101 | fix first; needs one local backtrace run |
| Decoder edge cases never swept on the user's files: DC6 `flip` ∉ {0,1} refused, DCC odd `variable0` refused, zero bytes in DT1 / DC6 runs (HANDOFF item 52; sweeps `d2-formats/tests/game_sweep.rs:136-360`, `frames/tests.rs:600`, `composite/tests.rs:439` all `#[ignore]`, not run) | a refused frame set → `ViewError` → whole frame black (G19) | run the sweeps in the first local run (LOCAL-RUN batch 2); preview mode skips and logs per item |
| `ViewAssets.maps` empty (G10) | first shaded draw errors | load PL2 rows with the palette |
| `world_view/ui_bind.rs:161` `try_into` of a 256-byte PL2 row | panic on a short row | turn into an error |
| `frames/atlas.rs:218` expect: frame larger than `MAX_SIDE` | panic on a huge DCC frame | check DCC frame sizes in the sweep |
| Light / unit facts filled by preview (D1) | wrong brightness or draw order vs 1.14d | expected; labelled unverified, never "done" (rule 10) |
| No walk motion (G11) | character teleports to the target when the walk ends, or not at all (no message to the walker) | D2 |
| Placement check `OriginalView::place` (`rules/view.rs` ~400) errors when a cel is cut by the frame edge | units at the screen edge fail the frame | preview: clip instead of error (D1), `sprite-placement.md` §4 TODO |

## 4. Decisions the user makes before the sessions start

| # | Question | Recommendation |
|---|---|---|
| D1 | Allow a **preview fill** for inputs the model lacks (full-bright light, zero unit flags, no-item component defaults, per-item skip on error, clip at the frame edge)? | Yes, as `world_view::preview`, on in `play` (a later flag can turn it off once the owner specs' inputs land), every answer marked `// d2rs-own, unverified`; the strict path stays and the capture compare keeps using it. Without D1 the first build stays black. |
| D2 | Own-walk motion before REC-51: advance the local player along the server's path in the client model? | Yes, provisional: client steps the player toward the clicked target at the charstats walk / run speed, snapped by every 0x15 / 0x0F; only in preview; replaced when REC-51 settles `model.md` OQ2. |
| D3 | New character without the original's select / create screens? | CLI `play --new <class> <name>` (class by name or 0–6), writes nothing to disk; the menus are a later spec. |

## 5. Sessions (parallel, no shared files)

Shared seams: S-A owns `app/play.rs` and turns the preview on there (no
new CLI flag in the first build, so S-A never touches `main.rs`); S-B
exposes `world_view::unit_rules` and installs it in `app/ui.rs`; S-D owns
`main.rs` (`--new`). S-A and S-B meet only at the existing `ViewRules` /
`ViewFeed` traits. Each session's tests use its own fixtures, so the four
branches build alone. Any merge order works.

| Session | Files (owned) | Gaps | Est. |
|---|---|---|---|
| **S-A Map** | `app/play.rs`, `world_view/model_feed.rs`, new `world_view/tile_assets.rs`, new `world_view/preview.rs` (light / facts / error policy), `world_view/present.rs`, `world_view/feed.rs` | G2, G3, G4, G5, G8, G10, G19 (preview half of D1) | 4 h |
| **S-B Units** | new `world_view/unit_rules.rs`, new `world_view/unit_assets.rs`, `app/ui.rs`, `rules/unit_composite.rs` (only if a helper is missing), `rules/view.rs` (edge clip) | G6, G7, G9 | 4 h |
| **S-C Server / sim** | `d2-sim/src/drlg/*`, `d2-server/src/adapters/{session,character}.rs`, `d2-server/src/adapters/handlers/world.rs`, new tests in `crates/d2-client/tests/app_level_border.rs` | G1 (after the local backtrace), G14, G15 send half (0x1D–0x1F at join, needs a spec line), G17, G18 | 3.5 h |
| **S-D Input / character / panels** | `world_view/ui_bind.rs` (incl. the `:161` panic fix), `bridge/click.rs`, new `bridge/predict.rs`, `main.rs`, `app/single_player.rs`, `ui/original.rs` | G11 (D2), G12, G13 (D3), G15 binding half | 3 h |

Total: **~14.5 session-hours**, about **4 h wall clock** with four parallel
sessions, plus **one local run first** (G1 backtrace and the batch 2
sweeps, ~30 min) and **one local run at the end** (~30 min).

### Done-checks the user sees (`cargo run -p d2-client --release -- play [--save X.d2s | --new <class> <name>]`)

| After | User sees |
|---|---|
| S-C alone | no crash: the log runs to `--frames 3000` with server ticks ≈ 25/s; `RUST_LOG=info` shows units > 20 in the model in town (NPCs, objects, waypoint) |
| S-A alone | the Rogue Encampment's floor and walls, centred on the (invisible) player, full bright; the right act 1 palette |
| S-A + S-B | the player character standing (class from `--save` or `--new`), Akara, Kashya, Charsi, Gheed, Warriv and the other town NPCs in their animation, the waypoint |
| + S-D | left click walks there (the view follows); the run toggle (R in the `dev` bindings) switches walk / run; `--new amazon Test` starts a new Amazon; C opens the character panel with name, level and stats (once S-C sends 0x1D–0x1F) |
| all four | walk out of the east gate into the Blood Moor: new tiles stream in, Fallen and Quill Rats stand there (they move by server messages). I opens an empty inventory panel (contents need G16, deferred) |

### What stays unverified

Everything drawn is **unverified** against 1.14d (rule 10): preview light,
draw order without unit facts, walk motion (D2). The checks to queue in
HANDOFF §5 once the build runs: the capture compare cases
(`capture-cases/*.toml`, need a `SceneSource`), REC-51 for walk motion, and
the frame-capture recordings of LOCAL-RUN 6.1.
