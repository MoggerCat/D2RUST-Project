// Spec: specs/ui/controls.md (§6 r9.2 "pend"), specs/client/model.md (§8 rule 7); preview fill: docs/PLAN.md decisions D1, D2
//! The `play` preview's pending interaction: a click on a unit out of
//! reach walks toward it and keeps the interaction pending (§6 r9.2,
//! [`ClickOut::Pend`]); when the walk ends the interact sender runs
//! (`client/model.md` §8 rule 7, [`crate::bridge::Bridge::interact`]),
//! which sends C→S 0x13 (or the item pickup).
//!
//! d2rs-own, unverified. In 1.14d the pending record (player data
//! +0x150..+0x15C) is consumed by the client path's arrival, which the
//! model does not hold (no client path record; the server sends the
//! walker nothing, `client/model.md` OQ2, REC-51). The preview reads
//! "arrived" as: the predicted walk ([`crate::bridge::predict`]) was seen
//! and has ended, or no walk was seen within [`NO_WALK_FRAMES`] frames
//! (the click was already in reach of the walk clamp). A new press that
//! sets no pending record drops the old one.

use crate::bridge::link::ServerLink;
use crate::bridge::output::Output;
use crate::bridge::{Bridge, BridgeError};
use crate::controls::click::{ClickOut, Pending};

/// Frames to wait for the walk to show before interacting anyway.
/// d2rs-own, unverified.
pub const NO_WALK_FRAMES: u32 = 8;

/// The pending interaction of the preview (module doc).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PreviewInteract {
    pub pending: Option<Pending>,
    /// The predicted walk toward the target was seen.
    seen_walk: bool,
    /// Frames since the pending record was set.
    frames: u32,
}

impl PreviewInteract {
    /// Notes one loop pass's click results: the last [`ClickOut::Pend`]
    /// sets (or clears) the record; a press with none clears it.
    pub fn note(&mut self, outs: &[ClickOut], pressed: bool) {
        let mut set = None;
        for o in outs {
            if let ClickOut::Pend(p) = o {
                set = Some(*p);
            }
        }
        match set {
            Some(p) => {
                *self = PreviewInteract {
                    pending: p,
                    ..Default::default()
                }
            }
            None if pressed => *self = PreviewInteract::default(),
            None => {}
        }
    }

    /// One frame: with `walking` the prediction's walk state, runs the
    /// interact sender on the pending target once it is reached.
    pub fn frame<L: ServerLink>(
        &mut self,
        bridge: &mut Bridge<L>,
        walking: bool,
    ) -> Result<Vec<Output>, BridgeError> {
        let Some(p) = self.pending else {
            return Ok(Vec::new());
        };
        self.frames += 1;
        if walking {
            self.seen_walk = true;
            return Ok(Vec::new());
        }
        if !self.seen_walk && self.frames < NO_WALK_FRAMES {
            return Ok(Vec::new());
        }
        *self = PreviewInteract::default();
        bridge.interact(p.target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::world::UnitKey;

    fn pend(g: u32) -> ClickOut {
        ClickOut::Pend(Some(Pending {
            code: 0x13,
            target: UnitKey::new(2, g),
        }))
    }

    #[test]
    fn a_pend_is_kept_and_a_bare_press_drops_it() {
        let mut i = PreviewInteract::default();
        i.note(&[ClickOut::Retarget, pend(7)], true);
        assert_eq!(i.pending.unwrap().target, UnitKey::new(2, 7));
        i.note(&[], false);
        assert!(i.pending.is_some(), "no press: kept");
        i.note(&[ClickOut::Retarget], true);
        assert!(i.pending.is_none(), "a press without a pend drops it");
    }
}
