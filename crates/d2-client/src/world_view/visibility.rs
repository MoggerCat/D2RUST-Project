// Spec: specs/client/model.md (§13 visibility predicate, §6 rule 6)
//! The visibility predicate `0x004DBF20` (`client/model.md` §13) from the
//! last built frame: §13 r6 has the bridge take it as an input and the
//! render side answer it from its camera, COF and cel state. Each frame
//! records the drawn units' COF and TR frame ([`Drawn`]); [`predicate`]
//! turns them, the frame's camera and the frame store into the bridge's
//! [`VisibleFn`] (rules 1–5, `rules::unit_visibility`).
//!
//! A unit the frame did not draw has no cel: its request fails, so it is
//! not visible (§13 r3–r4). The predicate answers for the frame before the
//! message (the bridge frame runs before the world view's).

use std::collections::BTreeMap;
use std::sync::Arc;

use d2_formats::cof::Cof;

use super::{UnitPose, ViewAssets};
use crate::assets::path::CanonicalPath;
use crate::bridge::world::{ClientUnit, UnitKey, VisibleFn};
use crate::composite::ComponentDraw;
use crate::rules::camera::Camera;
use crate::rules::unit_visibility::{unit_visible, CelBox};
use crate::scene::FrameId;

/// Component 1, TR (`render/unit-composite.md`), the cel of §13 r3.
const TR: u8 = 1;

/// One drawn unit: its COF and the frame of its TR layer, if drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drawn {
    pub key: UnitKey,
    pub cof: CanonicalPath,
    pub tr: Option<FrameId>,
}

impl Drawn {
    /// The record of a unit drawn with `pose` as `draws`.
    pub fn of(key: UnitKey, pose: &UnitPose, draws: &[ComponentDraw]) -> Self {
        Drawn {
            key,
            cof: pose.cof.clone(),
            tr: draws
                .iter()
                .find(|d| d.slot.component == TR)
                .map(|d| d.item.frame),
        }
    }
}

/// The frame's answer: the camera's unit origin and shift, the frame
/// size, and each drawn unit's COF and TR cel box.
struct Snapshot {
    origin: (i32, i32),
    shift_x: i32,
    size: (u32, u32),
    units: BTreeMap<UnitKey, (Cof, Option<CelBox>)>,
}

impl Snapshot {
    fn visible(&self, unit: &ClientUnit, a: i32, b: i32) -> bool {
        let Some((cof, cel)) = self.units.get(&unit.key) else {
            return false;
        };
        unit_visible(
            cof,
            *cel,
            a,
            b,
            self.origin,
            self.shift_x,
            self.size.0,
            self.size.1,
        )
    }
}

/// The predicate of a frame with `camera` that drew `drawn`; `None`
/// without a camera (no local player: nothing is placed, §13 r1 has no
/// origin).
pub fn predicate(
    camera: Option<Camera>,
    drawn: &[Drawn],
    assets: &ViewAssets,
) -> Option<VisibleFn> {
    let camera = camera?;
    let units = drawn
        .iter()
        .filter_map(|d| {
            let cof = assets.cofs.get(&d.cof)?.clone();
            let cel = d.tr.and_then(|id| assets.frames.frame(id)).map(|f| CelBox {
                w: f.width as i32,
                h: f.height as i32,
                ox: f.x_off,
                oy: f.y_off,
            });
            Some((d.key, (cof, cel)))
        })
        .collect();
    let s = Arc::new(Snapshot {
        origin: (camera.unit.x, camera.unit.y),
        shift_x: camera.view.shift_x,
        size: (camera.size.width as u32, camera.size.height as u32),
        units,
    });
    Some(VisibleFn::new(move |u, a, b| s.visible(u, a, b)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::camera::{ClientPos, FrameSize, OpenMode};

    // Covers: specs/client/model.md §13 r3
    #[test]
    fn an_undrawn_unit_is_not_visible() {
        let cam = Camera::new(
            FrameSize::D2RS,
            OpenMode::NONE,
            ClientPos::default(),
            (0, 0),
        );
        let assets = ViewAssets::new(crate::app::play::unspecified_palette());
        let p = predicate(Some(cam), &[], &assets).expect("a camera gives a predicate");
        let u = ClientUnit::new(UnitKey::new(1, 7));
        assert!(!p.visible(&u, 0, 0));
        assert!(predicate(None, &[], &assets).is_none());
    }
}
