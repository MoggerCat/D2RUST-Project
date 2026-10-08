# Spec: Seam — bridge (client model) ↔ Bevy app (view, UI, audio)

- **Status:** draft. Written by the 2026-10-08 seam audit (`q-seam-audit`)
  from both sides' specs and code; two rules (§2.4, §2.5) have a contract
  test, the rest are findings (`docs/HANDOFF.md`, queue rows
  `q-fix-seam-*`). Nothing here is verified against 1.14d.
- **Target version:** 1.14d
- **Crate/module:** `d2-client::bridge` (model, outputs, `mirror`) ↔
  `d2-client::{world_view, app, ui, audio}`
- **Related specs:** `client/bridge.md` (§1, §7, §8, §10), `client/model.md`
  (§5, §17 r4, §18), `client/render-pipeline.md` (A1), `client/ui.md`
  (A4, A5), `client/audio.md` (A2, A3), `render/camera.md` §9,
  `render/unit-composite.md`, `sim/unit-order.md` §5 r7,
  `audio/triggers.md` §11. METHODS M23.

## Summary

The seam between the plain-Rust client model the bridge keeps
(`ClientWorld`, the frame's outputs, the bridge counters) and the Bevy app
that draws it, applies its UI and sound outputs, reads input and plays
sound. This spec restates nothing its owners state; it names, for every
value that crosses, its unit or space, its single owner and when in the
frame each side reads or writes it, so both sides can be tested against
the same rule. World↔screen projection, camera and picking, and the walk
prediction are other seams (not here).

## Inputs

| Name | Type | Source |
|---|---|---|
| `ClientWorld` | model, read-only to the app | bridge (`client/bridge.md` §5) |
| frame outputs | `Vec<Output>`, list order | `bridge_frame` → `FrameOutputs` (§10 r4) |
| `frames`, `server_ticks` | u64 counters | bridge (§5 r3) |
| window input | Bevy `ButtonInput`, cursor (physical px) | Bevy |

## Outputs / state changes

C→S messages through the bridge's send path; UI-requested model writes
returned to the bridge (§10 r10); the presented 800×600 image; mixer
voices.

## Rules

### 1. Frame order (one Bevy frame)

| Step | Schedule | System | Reads | Writes |
|---|---|---|---|---|
| 1 | PreUpdate | `bridge_frame` | link | model, counters, `FrameOutputs` |
| 2 | PreUpdate (after 1) | `deliver_outputs` | `FrameOutputs`, model | UI state, `UiSounds`, model (only §10 r10 writes via the bridge) |
| 3 | PreUpdate (after 1) | `present_act_palette` | model `palette_act` | `ViewAssets::palette` |
| 4 | PreUpdate (after 2) | `mirror_units` | model units | mirror entities |
| 5 | Update | `ui_input` | window | UI event queue |
| 6 | Update (after 5) | `world_view_frame` | model, UI, feed | C→S sends, `UiSounds`, room order (§2.8), image |
| 7 | PostUpdate | `audio_frame` | model, `UiSounds` | mixer |

### 2. Contract

#### 2.1 Time units

1. The model's clocks are `server_ticks` (40 ms ticks) and `frames`
   (bridge frames). The app draws once per new `server_tick`
   (`render/camera.md` §9); a frame with no new tick draws nothing.
2. Animation frames are 8.8 fixed point (`ClientUnit::frame`, +0x44,
   `client/model.md` §18 r1); the view's frame is `frame >> 8`. Every
   value of the field, 0 included, is a frame; "no frame" is not
   encoded in it.
3. The audio sound tick is one per server tick (`audio/triggers.md` §1
   r5); UI-cause triggers carry the client tick of the frame that made
   them (`client/audio.md` §A2).

#### 2.2 Ownership

1. The model is the bridge's (`client/bridge.md` §1 r1, §10 r6). The app
   writes it only through: the send path (§4), the UI-requested writes of
   §10 r10 / r11, and §2.8 below.
2. UI state (panels, open mode, interact NPC) is the UI layer's
   (`client/bridge.md` §10 r9). The camera's open mode and the click
   view's open mode are both read from it in the same frame.
3. Mirror entities and every app cache are views: rebuilt from the model,
   never read back by the bridge (§7 r2).

#### 2.3 Unit identity

1. A unit is `(type, GUID)`; a key that left the model despawns its
   mirror entity in the same frame (§7 r3) and drops every app cache
   keyed by it (unit art facing, unit sounds).
2. A unit free reaches audio as `UnitFreed` in list order (§10 r3.1).

#### 2.4 Output delivery

1. One frame's outputs are delivered whole, in list order, before any
   Update system (§10 r4); audio outputs become sound requests in that
   order, carrying the values captured at receive (§10 r3), the
   position included (§10 r3.1 b).
2. A UI consumer reads payload fields, not the model (§10 r3, r9); a
   value the UI needs from the model at receive time is a captured
   payload field.

#### 2.5 Sound requests reach the audio frame of their frame

Every sound request a frame makes (the outputs' in step 2, the UI's and
the world click's in step 6) is taken by the audio frame of the same
Bevy frame (step 7), in the order made. No request is dropped by the
app, and none waits for a later frame by schedule accident.

#### 2.6 Pause

`client/bridge.md` §8 r5 (owner): while UI state 9 or 11 is open, the
bridge frame runs no pump and no receive. The UI layer owns the states;
the app hands "paused" to the bridge before step 1, and the UI must
still run on paused frames (step 6 does not wait for a new tick then).

#### 2.7 Coordinate spaces

1. UI and hit tests are in frame pixels, 800×600 (`client/ui.md` A5); a
   window pixel maps by `ui::frame::Presentation` (integer scale, bars
   rounded down). The presented image is placed by the same
   `Presentation` (left, top), so the drawn pixel and the hit-tested
   pixel are the same.
2. Sound positions are model sub-tile cells; the listener is the local
   player's position as the view draws it (one position per frame for
   the camera, the click and the sound listener).

#### 2.8 Draw write-back

The one model write the draw makes: the fill's Y sort of each room's
unit list (`sim/unit-order.md` §5 r7), handed to `Bridge::set_room_order`
after the frame's build and before the next bridge frame. `client/bridge.md`
§1 r1 must list it as an exception (finding F8).

#### 2.9 Errors

A frame the audio layer cannot complete because a model input is pending
(`audio::driver::DriverError::Pending`) is named, never guessed (M02),
and does not end the app (finding F1).

#### 2.10 Caches across an act change

Every app cache keyed by a client DRLG room slot (`MapState` draw state)
is dropped when the client DRLG is rebuilt (S→C 0x03), since slots are
reused by the new act.

## Constants & data dependencies

Frame 800×600 (`client/ui.md` A5); tick 40 ms (`tick.md` §1).

## Randomness

None at the seam.

## Edge cases & original bugs

None specified here.

## Test vectors

| Input | Expected | Source |
|---|---|---|
| outputs [ServerSound U at (0x1241, 0x11C4), UnitFreed U], no UI | requests [Server U same at, UnitFreed U] | `tests/seam_bridge_app.rs` |
| a sound pushed in Update every frame | `UiSounds` empty after each frame | `tests/seam_bridge_app.rs` |

## Provenance

Seam audit of 2026-10-08 (`q-seam-audit`): read of the specs named above
and of `bridge/{mod,mirror,click,objects/interact}.rs`,
`world_view/{present,ui_bind,model_feed,unit_rules,unit_assets,near_rooms}.rs`,
`app/{play,sound,palette}.rs`, `audio/driver.rs`, `ui/{msg_ui,edge,frame}.rs`.
d2rs design only; no 1.14d reading.

## Open questions

1. §2.7 r2: which position is the sound listener while the walk
   prediction runs (the movement-prediction seam decides; REC-277).
2. §2.6: how the bridge learns "paused" (a bridge setter before the
   frame, or a link wrapper).
