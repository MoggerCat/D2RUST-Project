# Handoff: the position check's visibility predicate in play — `claude/q-fix-visibility`

> Not yet folded into `docs/HANDOFF.md` / `docs/PLAN.md` (the REC-286 line is in `docs/HANDOFF.md` §7). The coordinator folds the rest.

Cloud session, 2026-10-08. Fixes break 2 of `docs/handoff/q-smoke-combat.md` (on `claude/q-smoke-combat`): no code called `Bridge::set_visibility` (`client/model.md` §13 r6), so in the play window every S→C 0x96 whose point differed from the client's cell on both axes was refused with `Unspecified("model.md open question 7: the visibility predicate 0x004DBF20")`. Base: `claude/specs-staging-7` (merged; it was already up to date).

## Links connected

| # | Link | Where |
|---|---|---|
| 1 | The seam: `VisibleFn` was a context-free `fn(&ClientUnit, i32, i32) -> bool`. It is now a cloneable `Fn + Send + Sync` closure (`VisibleFn::new`, `VisibleFn::visible`); the bridge only calls it and stays Bevy-free. | `bridge/world.rs` (`VisibleFn`), `bridge/check.rs` rule 6 |
| 2 | The cel fields: the unit art loader records each loaded component file's cel w, h, xoff, yoff per file direction and frame (`render/sprite-placement.md` §3: the DC6 frame header as stored; the DCC frame header's width, height, x / y offset unchanged). | `world_view/unit_assets.rs` (`UnitArt::cels`, `cel_boxes`) |
| 3 | The camera: `build_frame` returns the camera it placed the frame with (`WorldFrame::camera`), and the view publishes it to `WorldViewState::camera` (shared). The origin getters of §13 r1 return the last drawn frame's globals. | `world_view/feed.rs`, `world_view/mod.rs`, `world_view/present.rs` (one line) |
| 4 | The predicate: `world_view::visibility::ViewVisibility` runs §13 rules 1–5 (`rules::unit_visibility::unit_visible`) from that camera, the unit's COF (draw identity and drawn mode as `UnitRules` resolves them), the COF's TR layer, the TR file at the unit's direction and frame `+0x44 >> 8`, and the recorded cel. | `world_view/visibility.rs` |
| 5 | The install: `app::visibility::add_visibility` builds it over the original UI's unit art (or an empty art store on synthetic data) and calls `Bridge::set_visibility`; `play::run` calls it after `add_walk`. | `app/visibility.rs`, `app/play.rs` (one line) |

Tests (each fails before the change):

- `world_view::visibility::tests` (3 tests): the COF box and the cel box at the camera origin, edges included (cel left + w = −2 fails, 0 passes; COF x_min and y_max edges); no COF / no TR layer / no TR file / a frame past the file → not visible; no camera yet → origin 0, and the shared camera is read on each call.
- `tests/app_play_visibility.rs` `a_diagonal_walk_checks_0x96_through_the_view_predicate`: the play preview headless (the `app_play_e2e.rs` wiring and fixtures, plus `add_visibility`), a diagonal walk in the synthetic town. Asserts no refused 0x96 and that the predicate was asked. Without `add_visibility` it fails with the refused 0x96 (checked in the session).

## PROVISIONAL points (REC-286)

1. Before the first drawn frame (no camera) the origin and `shiftX` read as 0 (globals taken as zero-initialised).
2. The cel direction is the view's preview facing (`UnitArt::dir64`, REC-51); the frame is `+0x44 >> 8` as §13 r3 says, which can differ from the preview's drawn frame while the model's frame is 0.
3. A unit with no resident COF, no TR layer or no loaded TR file is not visible, so the check corrects (§6 rule 7: C→S 0x5F for the local player, a move for others) instead of refusing. On `play --synthetic` no unit has art, so every diagonal 0x96 of the local player within tolerance now asks the server for a resync (0x5F) rather than logging a refusal.
4. The cel context fields beyond frame / component / direction and the `0x006001F0` load arguments stay `model.md` OQ7.

## Other findings

- The synthetic new character has no max life (no `itemstatcost` / `charstats`), so the server's vitals sync never sends 0x96 on `play --synthetic` (`combat/vitals.md` §5.3 step 1). The test gives the player 100 life the way `app_stamina.rs` does. On live data the sync runs.
- The synthetic town room spans x 80–119, y 0–39 with units at y ≥ 20; a click whose target is outside the walkable part gets no movement from the server (the prediction still moves). The test clicks below the player.

## What's left

- `claude/q-smoke-combat`'s rig sets `set_visibility(Some(|_, _, _| true))`. After this merge that line no longer compiles: it becomes `set_visibility(Some(VisibleFn::new(|_, _, _| true)))`.
- Break 3 of q-smoke-combat (the left skill does not kill) is not touched here.

## The user's local check

```
cargo run -p d2-client --release -- play --new sorceress Test
```

With `D2_GAME_DIR` set (live data): walk diagonally and in circles near monsters outside town for a minute. Expected: no `S→C 0x96 refused: … visibility predicate` lines in the log (before this fix they appeared on diagonal moves). Note any other `refused` lines. The character should not rubber-band while walking on screen; if it does, note when (a 0x5F correction means the predicate read the player as not visible).
