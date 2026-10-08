// Spec: specs/ui/controls.md (§6 r2, r8.3 object case), specs/client/model.md (§8 rule 7)
//! The object under the mouse and the object interact, for the `play`
//! preview's object clicks ([`crate::world_view::object_click`]). The
//! hover model of `0x00467A10` is not specified (`click.rs` module doc),
//! so a click on an object is resolved here: the mouse's world sub-tile
//! ([`super::click::screen_to_world`]) against the model's objects.
//! d2rs-own, unverified (stitch-objects).

use super::click::{ClickView, ModelClick};
use super::objects::interact;
use super::world::{UnitKey, OBJECT};
use super::{Bridge, BridgeError, ServerLink};

/// How far (sub-tiles, Chebyshev) from an object's cell a click still
/// picks it. d2rs-own, unverified.
pub const PICK_RADIUS: i32 = 3;

impl ModelClick<'_> {
    /// The object nearest the mouse's world sub-tile within
    /// [`PICK_RADIUS`]. Not gated on `flag_4`: the client's selectable
    /// bit is not set for every operable object yet (the server refuses
    /// what it will not operate). d2rs-own, unverified.
    pub fn object_at(&self, at: (i32, i32)) -> Option<UnitKey> {
        let cam = self.camera()?;
        let (wx, wy) = super::click::screen_to_world(&cam, at.0, at.1);
        self.world
            .units
            .iter()
            .filter(|(k, u)| k.unit_type == OBJECT && u.position.is_some())
            .filter_map(|(k, u)| {
                let (x, y) = u.position?;
                let d = (i32::from(x) - wx).abs().max((i32::from(y) - wy).abs());
                (d <= PICK_RADIUS).then_some((d, *k))
            })
            .min_by_key(|(d, k)| (*d, k.guid))
            .map(|(_, k)| k)
    }
}

impl<L: ServerLink> Bridge<L> {
    /// The object a click at `at` lands on (`None`: the ground).
    pub fn object_under(
        &self,
        view: ClickView,
        at: (i32, i32),
        local_at: Option<(u32, u32)>,
    ) -> Option<UnitKey> {
        ModelClick {
            world: &self.world,
            inputs: &self.inputs,
            view,
            local_at,
        }
        .object_at(at)
    }

    /// The interact sender for `key` (`interact::send`, C→S 0x13), sent
    /// at once.
    pub fn object_interact(&mut self, key: UnitKey) -> Result<(), BridgeError> {
        let out = interact::send(
            &mut self.world,
            &self.inputs,
            u16::from(key.unit_type),
            key.guid,
        )
        .map_err(BridgeError::Click)?;
        self.outputs.extend(out);
        self.send_outgoing()?;
        Ok(())
    }
}
