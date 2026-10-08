// Spec: specs/client/model.md (§13), specs/render/camera.md (§3, §4), specs/render/unit-composite.md (§2, §3 r1, §4, §5.1, §6), specs/render/sprite-placement.md (§3)
//! The visibility predicate `0x004DBF20` (`client/model.md` §13) answered
//! from the world view's state, for the bridge's position check (§6 rule
//! 6, §13 r6: the render side gives the bridge the predicate,
//! [`crate::bridge::Bridge::set_visibility`]).
//!
//! What it reads, all shared with the view (no Bevy types here):
//! - rule 1: the unit origin and `shiftX` of the last drawn frame's camera
//!   ([`SharedCamera`], written from [`super::WorldFrame::camera`]: the
//!   origin getters return the globals the last draw set);
//! - rule 2: the unit's COF (the unit art's resident COFs, the unit's
//!   draw identity and drawn mode as [`super::unit_rules::UnitRules`]
//!   resolves them);
//! - rules 3–4: the TR component file of that COF at the unit's
//!   direction and frame `+0x44 >> 8` (the unit art's loaded files);
//! - rule 5: that cel's w, h, xoff, yoff ([`UnitArt::cels`]).
//!
//! The rule arithmetic is `rules::unit_visibility`.
//!
//! PROVISIONAL (REC-286): before the first drawn frame (no camera yet) the
//! origin and `shiftX` are read as 0, the zero-initialised globals
//! `0x007A520C` / `0x007A5208` / `0x007A5214`; and the direction is the
//! preview facing the view draws with (`UnitArt::dir64`, itself
//! PROVISIONAL REC-51), as the model holds no client path record.
//!
//! [`UnitArt::cels`]: super::unit_assets::UnitArt::cels

use std::sync::{Arc, RwLock};

use crate::bridge::world::VisibleFn;
use crate::bridge::ClientUnit;
use crate::rules::camera::{Camera, FrameSize};
use crate::rules::unit_composite::{component_cel, frame_index, unit_direction};
use crate::rules::unit_visibility::{unit_visible, CelBox};

use super::unit_assets::{component_codes, unit_cof, SharedUnitArt, UnitArt, UnitLooks};

/// The last drawn frame's camera, shared by the view (writer) and the
/// predicate (reader).
pub type SharedCamera = Arc<RwLock<Option<Camera>>>;

/// Component id of the torso (`composit` row 1, the cel request's
/// component byte, §13 r3).
const TR: u8 = 1;

/// The predicate's inputs: the unit tables, the unit art and the camera.
#[derive(Clone)]
pub struct ViewVisibility {
    pub looks: Arc<UnitLooks>,
    pub art: SharedUnitArt,
    pub camera: SharedCamera,
}

impl ViewVisibility {
    /// `visible(U, a, b)` (§13 rules 1–5); W × H is the d2rs frame.
    pub fn visible(&self, unit: &ClientUnit, a: i32, b: i32) -> bool {
        let camera = *self.camera.read().unwrap_or_else(|e| e.into_inner());
        // PROVISIONAL (REC-286): no frame drawn yet → zeroed globals.
        let (origin, shift_x) =
            camera.map_or(((0, 0), 0), |c| ((c.unit.x, c.unit.y), c.view.shift_x));
        let art = self.art.read().unwrap_or_else(|e| e.into_inner());
        let posed = art.posed(unit);
        let unit = &*self.looks.shapes.identity(&posed);
        let Some(name) = unit_cof(&self.looks, unit) else {
            return false;
        };
        let Some(cof) = name.path().ok().and_then(|p| art.cofs.get(&p)) else {
            return false;
        };
        let cel = tr_cel(&self.looks, &art, unit, &name, cof);
        let size = FrameSize::play();
        unit_visible(
            cof,
            cel,
            a,
            b,
            origin,
            shift_x,
            size.width as u32,
            size.height as u32,
        )
    }

    /// The predicate as the bridge takes it.
    pub fn into_fn(self) -> VisibleFn {
        VisibleFn::new(move |unit, a, b| self.visible(unit, a, b))
    }
}

/// Rules 3–4: the TR cel of `unit` at its direction and frame `+0x44 >>
/// 8`; `None` when the COF has no TR layer, the component request fails,
/// or the file or cel did not load.
fn tr_cel(
    looks: &UnitLooks,
    art: &UnitArt,
    unit: &ClientUnit,
    name: &crate::rules::unit_composite::CofName,
    cof: &d2_formats::cof::Cof,
) -> Option<CelBox> {
    let layer = cof.layers.iter().find(|l| l.component == TR)?;
    let codes = component_codes(looks, unit, name, layer)?;
    let (path, facts) = art.files.get(&codes.name())?.as_ref()?;
    let n = art.expected_directions(unit, name.kind, cof.directions);
    let dir = unit_direction(cof.directions, n, art.dir64(unit), false).ok()?;
    let frame = frame_index(unit.frame as u32);
    let cel = component_cel(path, facts.directions, facts.frames, dir.dir64, frame).ok()?;
    let crate::frames::FramePart::Dir(d) = cel.set.part() else {
        return None;
    };
    art.cels
        .get(path)?
        .get(usize::from(d))?
        .get(cel.index)
        .copied()
}

#[cfg(test)]
mod tests;
