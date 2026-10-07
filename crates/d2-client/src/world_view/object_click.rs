// Spec: specs/ui/controls.md (§6 r8.3 object case), specs/client/model.md (§8 rule 7), specs/world/objects.md (§7.1)
//! Clicking a world object in the `play` preview: the click walks as a
//! ground click does (the dispatcher has no hover model, `bridge/click.rs`),
//! and the object under the mouse becomes the pending interact target
//! (`controls/click.rs` `interact`: walk, then pend [`CODE_INTERACT`]).
//! Once the walk has brought the player near, C→S 0x13 goes out and is
//! re-sent every few frames until the object changes mode (the server
//! answers "walk" while it still counts the player out of range, and
//! operates, `Pending::object_preview_range`, once in). The client sends
//! intents only; the server decides the operate.
//! d2rs-own, unverified (stitch-objects).

use crate::bridge::click::ClickView;
use crate::bridge::link::ServerLink;
use crate::bridge::world::UnitKey;
use crate::bridge::{Bridge, BridgeError};
use crate::controls::click::{ClickOut, ClickState};
use crate::ui::{PointerButton, UiEvent};

use super::ui_bind::world_clicks;

/// The server's interact reach in sub-tiles (`LocalSeams::object_preview_range`).
pub const INTERACT_RANGE: i32 = 5;
/// The client sends 0x13 once the predicted player is this close (the
/// server's own test then decides).
const SEND_RANGE: i32 = INTERACT_RANGE + 2;
/// Frames between two sends of the same interact.
const RESEND_FRAMES: u64 = 8;
/// Sends before the target is dropped (the object never changed).
const MAX_SENDS: u32 = 30;

#[derive(Clone, Copy, Debug)]
struct Target {
    key: UnitKey,
    mode: u32,
    last_send: Option<u64>,
    sends: u32,
}

/// The pending object interact of the preview.
#[derive(Clone, Copy, Debug, Default)]
pub struct ObjectClick {
    target: Option<Target>,
}

impl ObjectClick {
    /// The object the player is walking to, if any.
    pub fn target(&self) -> Option<UnitKey> {
        self.target.map(|t| t.key)
    }

    fn tick<L: ServerLink>(
        &mut self,
        bridge: &mut Bridge<L>,
        local_at: Option<(u32, u32)>,
    ) -> Result<(), BridgeError> {
        let Some(t) = self.target.as_mut() else {
            return Ok(());
        };
        let world = bridge.world();
        let Some(obj) = world.units.get(&t.key) else {
            self.target = None;
            return Ok(());
        };
        if obj.mode != t.mode || t.sends >= MAX_SENDS {
            self.target = None;
            return Ok(());
        }
        let frame = world.frames;
        let (Some(me), Some((ox, oy))) = (
            local_at
                .map(|(x, y)| ((x >> 16) as i32, (y >> 16) as i32))
                .or_else(|| {
                    world
                        .local()
                        .and_then(|p| p.position)
                        .map(|(x, y)| (i32::from(x), i32::from(y)))
                }),
            obj.position,
        ) else {
            return Ok(());
        };
        let d = (me.0 - i32::from(ox))
            .abs()
            .max((me.1 - i32::from(oy)).abs());
        let due = t
            .last_send
            .is_none_or(|f| frame.wrapping_sub(f) >= RESEND_FRAMES);
        if d <= SEND_RANGE && due {
            t.last_send = Some(frame);
            t.sends += 1;
            let key = t.key;
            bridge.object_interact(key)?;
        }
        Ok(())
    }
}

/// [`world_clicks`] plus the object target: a left press on an object
/// sets it (any other left press clears it), and every pass sends the
/// interact once the player is close.
pub fn world_clicks_objects<L: ServerLink>(
    bridge: &mut Bridge<L>,
    oc: &mut ObjectClick,
    st: &mut ClickState,
    view: ClickView,
    unhandled: &[UiEvent],
    mods: u32,
    local_at: Option<(u32, u32)>,
) -> Result<Vec<ClickOut>, BridgeError> {
    for e in unhandled {
        if let UiEvent::Press {
            button: PointerButton::Left,
            at,
        } = *e
        {
            oc.target = bridge
                .object_under(view, (at.x, at.y), local_at)
                .and_then(|key| {
                    let mode = bridge.world().units.get(&key)?.mode;
                    Some(Target {
                        key,
                        mode,
                        last_send: None,
                        sends: 0,
                    })
                });
        }
    }
    let out = world_clicks(bridge, st, view, unhandled, mods, local_at)?;
    oc.tick(bridge, local_at)?;
    Ok(out)
}
