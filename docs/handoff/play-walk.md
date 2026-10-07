# S-D1 Walking: handoff

> Implementation session, 2026-10-07, branch `claude/play-walk`. Read
> `specs/`, `docs/`, `crates/` only. Plan: `first-playable-scope.md`
> (G11 with decision D2, G12).

## What was done

| File | Change |
|---|---|
| `crates/d2-client/src/bridge/predict.rs` (new) | `Predict`: the local player's predicted position. Each server tick it moves in a straight line toward the target of the last walk / run C→S message (0x01–0x04) at the charstats speed (`Speeds::step`: `WalkVelocity`·256·p/100·16, with p = 100 walking and 100·`RunVelocity`/`WalkVelocity` running; `sim/pathing.md` §8.1 r2, §8.2, §9.4 r2, case M1: 6 → 0x6000 a tick, run 9 → 0x9000). When it reaches the target, the walk ends. Any change of the model's local position (0x15, a check correction) or server point (0x0F and the other checked messages) moves the prediction to that point. The walk target is kept. `PredictLink<L>` wraps any `ServerLink` and records the walks sent through it (`take_walks`). Everything is marked `d2rs-own, unverified`, `PROVISIONAL (client/model.md OQ2; REC-51)`. |
| `crates/d2-client/src/bridge/click.rs` | `RunMods` {run held, run lock, stand still} → the `mods` word (`ui/controls.md` §4.3 r1: 8 for run held or lock, 4 for stand still), `toggle_run` (command 35). `ModelClick::local_at`, `world_click_at`, `held_repeat_at`: the camera and the walk clamp read the predicted position, so a click lands where the player is drawn. The old `world_click` / `held_repeat` call them with `None` and behave as before. |
| `crates/d2-client/src/bridge/mod.rs` | One line: `pub mod predict;`. No other change. |

Tests (synthetic fixtures, 12 new): walk-speed and run-speed steps, M1
precise positions tick by tick, a diagonal step, arrival, no movement
without a tick, snaps on a server point and on a placement, a unit
target, no local player, the four walk codes, the mods word, and
walk / run / stand still on a ground click. Another test checks that a
click with the predicted position moves the target by the predicted
offset. Gate on the branch: `cargo fmt`, `clippy -p d2-client
--all-targets -D warnings`, `cargo test -p d2-client` (all green),
`tools/coverage.py --check` (0 errors).

## Gaps

- **G11 (D2)**: done on the code side (provisional), not yet wired in.
  It works only once seams 1–4 below are connected.
- **G12**: the mods word and toggle exist. It works once seams 2 and 5
  are connected. The **stamina** caveat below still applies.

Nothing here is "done" in the sense of rule 10. The motion is a preview
stand-in until REC-51 settles `client/model.md` OQ2.

## Seams other sessions must connect

1. **Link (`app/play.rs`, S-A):** wrap the play link with
   `bridge::predict::PredictLink::new(link)` before `Bridge::new`. Keep a
   `Predict`, a `RunMods` and the `Speeds` in the play / present state
   (preview only; the strict path never builds them).
2. **Bridge methods (`bridge/mod.rs`, coordinator):** add two methods
   that mirror `world_click` / `click_repeat` and take
   `local_at: Option<(u32, u32)>`, calling `click::world_click_at` /
   `click::held_repeat_at`:
   ```rust
   pub fn world_click_at(&mut self, st, view, kind, at, mods, local_at) -> … {
       let r = click::world_click_at(&mut self.world, &self.inputs, st, view, kind, at, mods, local_at)
           .map_err(BridgeError::Click)?;
       self.send_outgoing()?;
       Ok(r)
   }
   ```
3. **Per frame (`world_view/present.rs`, S-A):** after `bridge.frame()`:
   ```rust
   let walks = bridge.link_mut().take_walks();
   predict.frame(bridge.world(), walks, report.ticked, speeds);
   ```
   Walks sent by this frame's clicks are taken on the next frame.
4. **View (`world_view/feed.rs` camera ~:333, `model_feed.rs` local-player
   draw, S-A / S-B):** for the local player, use `predict.position()`
   (precise 16.16, the input of `rules::camera::moving_to_client`)
   instead of the model cell, both for the camera centre and the unit's
   draw position. Draw the pose with `predict.mode()` (2 walk / 3 run
   while moving; `None` = the model's mode).
5. **Input (`world_view/ui_bind.rs` `world_clicks`, ~:452 and the
   `click_repeat` call):** pass `run.word()` instead of `0`, and
   `predict.position()` through the seam-2 methods. Key bindings
   (`controls/mod.rs` dev set): `Action::ToggleRun` (R) →
   `run.toggle_run()`; `Action::StandStill` (LeftShift) down / up →
   `run.stand_still = true / false`. Run held (command 34, Ctrl) has no
   dev action yet; `run_held` stays false until one is bound. The mode
   change and C→S 0x53 / 0x54 of command 34 are not implemented.
6. **Speeds (play app):** `Speeds { walk: charstats.walkvelocity, run:
   charstats.runvelocity }` for the local player's class, from the
   d2-sim combat tables the server already loads
   (`CombatTables.charstats`). The client tables do not hold charstats.

## Caveats

- **Stamina gates the run.** The dispatcher sets the run flag only when
  stat 10 (stamina) ≠ 0 (`controls/click.rs` `dispatch`, §6 r2). The
  model gets stamina only from 0x1D–0x1F, which the join does not send
  yet (G15, S-C). Until then the R toggle has no effect: clicks walk
  (0x01), not run (0x03).
- **Snap detection compares values.** A snap to the cell the model
  already holds is not seen. A direct hook (a placement counter written
  by the 0x15 / 0x0F handlers) would fix that. It is not needed for the
  preview.
- **Straight line.** The server walks its own path (`pathing.md` §4)
  around walls. The prediction goes straight and is corrected only by
  the next snap. The server sends the walker nothing while it walks, so
  a blocked walk shows the player past the wall until a 0x15 arrives.
- No facing / direction is predicted. The unit's direction stays the
  model's.

## Local checks for the user (need game files and a screen)

1. `cargo run -p d2-client --release -- play --save <X>.d2s`, after
   seams 1–6 are merged.
   - Left-click the ground a few sub-tiles away. The view and the player
     move smoothly toward the click at about 9.4 sub-tiles/s (0x6000 a
     25 Hz tick) and stop on the clicked cell.
   - A second click from the new spot walks from where the player is
     drawn, with no jump back to the start.
   - R, then click: the player moves at about 14 sub-tiles/s, but only
     once stamina arrives (see Caveats).
   - Hold Shift and click: no walk.
   - `RUST_LOG=debug`: `world click: …` lines. The link sends 0x01
     (walk) or 0x03 (run) with the clicked point.
2. Queue in HANDOFF §5: **REC-51** (the original client's own-walk
   motion), to replace `bridge/predict.rs` and settle
   `client/model.md` OQ2.

## What's left

Seams 1–6 above (other sessions' files) and the stamina dependency
(G15). After that: the command-34 mode change and 0x53 / 0x54, predicted
facing, and the REC-51 replacement.
