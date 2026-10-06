// Spec: specs/client/render-pipeline.md
//! Composite units (§A7): from a parsed COF, a COF direction and a frame,
//! the slot order of the components (back to front) and one scene
//! [`DrawItem`] per drawn component. Plain Rust, no Bevy types.
//!
//! Only the mechanism is ours. Everything §A7 hands to a §B owner spec goes
//! through [`ComponentResolver`], whose methods are the `TODO(spec: …)`
//! hooks: this module never picks a file, a frame inside it, a screen
//! position, a shade chain or a blend op on its own.
//!
//! The scene id of the chosen frame is not a hook: [`build_with`] looks
//! `(FrameSetKey, index)` up in a frame store ([`FrameIds`], implemented by
//! [`crate::frames::FrameStore`]).

#[cfg(test)]
mod store_tests;
#[cfg(test)]
mod tests;

use d2_formats::cof::{Cof, CofLayer, COMPONENTS};

use crate::frames::{FrameSetKey, FrameStore, StoreError};
use crate::scene::{BlendOp, DrawItem, DrawKey, FrameId, ItemTag, Rect, SceneError, ShadeChain};

/// Errors of composite building. Strict input (METHODS M07): a COF this
/// module cannot read without guessing is an error, never a skipped slot.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompositeError {
    #[error("direction {dir} out of range ({directions} in the COF)")]
    Direction { dir: usize, directions: u8 },
    #[error("frame {frame} out of range ({frames} per direction in the COF)")]
    Frame { frame: usize, frames: u8 },
    #[error("COF has {count} layer records for {layers} layers")]
    LayerCount { layers: u8, count: usize },
    #[error("layer {index}: component {component} is not 0..{COMPONENTS}")]
    LayerComponent { index: usize, component: u8 },
    #[error("layers {first} and {second} both carry component {component}")]
    DuplicateLayer {
        component: u8,
        first: usize,
        second: usize,
    },
    #[error("COF draw order has {len} bytes, header needs {expected}")]
    DrawOrderLength { len: usize, expected: usize },
    #[error("slot {slot}: component {component} has no layer record")]
    NoLayer { slot: u8, component: u8 },
    /// A [`ComponentResolver`] hook could not answer (frame not resident,
    /// unknown variant, …). `what` names the hook.
    #[error("slot {slot} ({component}): {what}: {message}")]
    Unresolved {
        slot: u8,
        component: u8,
        what: &'static str,
        message: String,
    },
    #[error("slot {slot}: {error}")]
    Scene { slot: u8, error: SceneError },
}

/// One entry of a frame's slot order (§A7 step 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    /// Slot index, back to front; becomes the draw key's `sub` (§A7 step 4).
    pub slot: u8,
    /// Component ID (0–15, `COMPONENT_NAMES` in `d2-formats`).
    pub component: u8,
    /// Index of the layer record carrying `component`.
    pub layer: usize,
}

/// Checks the parts of `cof` this module indexes. The parser already
/// guarantees them for parsed files; a hand-built `Cof` may not.
fn check(cof: &Cof) -> Result<(), CompositeError> {
    let layers = usize::from(cof.layers_count);
    if cof.layers.len() != layers {
        return Err(CompositeError::LayerCount {
            layers: cof.layers_count,
            count: cof.layers.len(),
        });
    }
    let expected = usize::from(cof.directions) * usize::from(cof.frames) * layers;
    if cof.draw_order.len() != expected {
        return Err(CompositeError::DrawOrderLength {
            len: cof.draw_order.len(),
            expected,
        });
    }
    let mut seen: [Option<usize>; COMPONENTS] = [None; COMPONENTS];
    for (index, layer) in cof.layers.iter().enumerate() {
        let c = usize::from(layer.component);
        let Some(entry) = seen.get_mut(c) else {
            return Err(CompositeError::LayerComponent {
                index,
                component: layer.component,
            });
        };
        if let Some(first) = *entry {
            // Two records for one component: which one a slot means is not
            // stated anywhere (cof.md), so refuse rather than pick.
            return Err(CompositeError::DuplicateLayer {
                component: layer.component,
                first,
                second: index,
            });
        }
        *entry = Some(index);
    }
    Ok(())
}

/// The slot order for COF direction `dir`, frame `frame` (§A7 step 2):
/// `cof.component_at(dir, frame, s)` for `s = 0..L`, back to front, each
/// with its layer record.
///
/// TODO(spec: render/unit-composite.md): unit direction → COF direction
/// and the frame source (animdata vs COF rate) are §B4; the caller passes
/// COF indices and anything out of range is an error.
pub fn slot_order(cof: &Cof, dir: usize, frame: usize) -> Result<Vec<Slot>, CompositeError> {
    check(cof)?;
    if dir >= usize::from(cof.directions) {
        return Err(CompositeError::Direction {
            dir,
            directions: cof.directions,
        });
    }
    if frame >= usize::from(cof.frames) {
        return Err(CompositeError::Frame {
            frame,
            frames: cof.frames,
        });
    }
    (0..cof.layers_count)
        .map(|slot| {
            let component = cof
                .component_at(dir, frame, usize::from(slot))
                .expect("draw order length checked");
            let layer = cof
                .layers
                .iter()
                .position(|l| l.component == component)
                .ok_or(CompositeError::NoLayer { slot, component })?;
            Ok(Slot {
                slot,
                component,
                layer,
            })
        })
        .collect()
}

/// What a hook is asked about: one slot of one COF frame.
#[derive(Debug, Clone, Copy)]
pub struct ComponentRequest<'a> {
    pub cof: &'a Cof,
    /// COF direction and frame (§A7 step 2).
    pub dir: usize,
    pub frame: usize,
    pub slot: Slot,
    /// The layer record of `slot.component` (shadow, selectable,
    /// translucency override, weapon class).
    pub layer: &'a CofLayer,
}

/// Which C3 frame set and which frame in it a component draws (§A7 step 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentFrame {
    /// The component's DCC and direction (`frames::FrameSet` key).
    pub set: FrameSetKey,
    /// Frame index inside that set.
    pub index: usize,
}

/// The §B owner-spec answers a composite needs, one hook per question.
/// Every method is a `TODO(spec: …)` hook: implementations follow the named
/// owner spec once written; until then an implementation that cannot answer
/// returns an error (`CompositeError::Unresolved`), never a default.
pub trait ComponentResolver {
    /// TODO(spec: render/unit-composite.md) (§B4): component file path
    /// (token, armor class variant, mode, weapon class), file direction for
    /// the COF direction, and the frame inside that direction. Whether a
    /// slot is drawn at all is also §B4: until it says otherwise, every
    /// slot is drawn.
    fn frame(&self, req: &ComponentRequest<'_>) -> Result<ComponentFrame, CompositeError>;

    /// TODO(spec: render/sprite-placement.md) (§B1): screen top-left of the
    /// component's image, from the unit's position and the frame offsets.
    fn place(
        &self,
        req: &ComponentRequest<'_>,
        frame: &ComponentFrame,
    ) -> Result<(i32, i32), CompositeError>;

    /// TODO(spec: render/shading.md, render/unit-composite.md) (§B3/§B4):
    /// light level, per-component colormaps, selection highlight.
    fn shade(&self, req: &ComponentRequest<'_>) -> Result<ShadeChain, CompositeError>;

    /// TODO(spec: render/blend-modes.md) (§B5): the blend op, including the
    /// layer's translucency override fields.
    fn blend(&self, req: &ComponentRequest<'_>) -> Result<BlendOp, CompositeError>;

    /// The scene id of a resident frame (residency, `client/assets.md`
    /// §A4). Not resident is an error (render-pipeline §Edge cases).
    ///
    /// Read only by [`build`]. [`build_with`] takes the id from a frame
    /// store instead and never calls this; a resolver used only with
    /// [`build_with`] keeps the default, which refuses.
    fn frame_id(
        &self,
        req: &ComponentRequest<'_>,
        _frame: &ComponentFrame,
    ) -> Result<FrameId, CompositeError> {
        Err(CompositeError::Unresolved {
            slot: req.slot.slot,
            component: req.slot.component,
            what: "frame_id",
            message: "no frame store: use composite::build_with".into(),
        })
    }
}

/// `(FrameSetKey, index)` → scene [`FrameId`] for resident frames (§A7
/// step 3, `client/assets.md` §A4). Not resident, or an index past the
/// set's end, is an error.
pub trait FrameIds {
    fn frame_id(&self, set: &FrameSetKey, index: usize) -> Result<FrameId, StoreError>;
}

impl FrameIds for FrameStore {
    fn frame_id(&self, set: &FrameSetKey, index: usize) -> Result<FrameId, StoreError> {
        self.id(set, index)
    }
}

/// Per-unit draw parameters shared by all its components.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitParams {
    /// Draw key fields shared by all components (§A7 step 4).
    /// TODO(spec: render/draw-order.md) (§B6): the caller fills them.
    pub pass: u32,
    pub major: u32,
    pub minor: u32,
    /// TODO(spec: render/camera.md, render/draw-order.md): the view edge or
    /// UI clip; [`Rect::FRAME`] when nothing else applies.
    pub clip: Rect,
    /// Debug label, e.g. [`ItemTag::Unit`] with the unit GUID.
    pub tag: ItemTag,
}

/// One drawn component: its slot, the frame it references, and its item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentDraw {
    pub slot: Slot,
    pub frame: ComponentFrame,
    pub item: DrawItem,
}

/// The draw items of one unit for COF direction `dir`, frame `frame`
/// (§A7 steps 2–4), in slot order (back to front), so list order equals
/// key order. Keys share `unit`'s pass/major/minor with `sub` = slot index.
/// Any hook error fails the whole unit: no partial composite. Frame ids
/// come from the resolver's [`ComponentResolver::frame_id`].
pub fn build<R: ComponentResolver + ?Sized>(
    cof: &Cof,
    dir: usize,
    frame: usize,
    unit: &UnitParams,
    resolver: &R,
) -> Result<Vec<ComponentDraw>, CompositeError> {
    build_core(cof, dir, frame, unit, resolver, |req, cf| {
        resolver.frame_id(req, cf)
    })
}

/// [`build`] with frame ids from the frame store `frames`: the frame of
/// each component ([`ComponentResolver::frame`]) is looked up as
/// `(set, index)`; a frame not in the store fails the unit
/// (`Unresolved`, `what` = `"frame_id"`). The resolver's `frame_id` is
/// not called.
pub fn build_with<R: ComponentResolver + ?Sized, S: FrameIds + ?Sized>(
    cof: &Cof,
    dir: usize,
    frame: usize,
    unit: &UnitParams,
    resolver: &R,
    frames: &S,
) -> Result<Vec<ComponentDraw>, CompositeError> {
    build_core(cof, dir, frame, unit, resolver, |req, cf| {
        frames
            .frame_id(&cf.set, cf.index)
            .map_err(|e| CompositeError::Unresolved {
                slot: req.slot.slot,
                component: req.slot.component,
                what: "frame_id",
                message: e.to_string(),
            })
    })
}

fn build_core<R: ComponentResolver + ?Sized>(
    cof: &Cof,
    dir: usize,
    frame: usize,
    unit: &UnitParams,
    resolver: &R,
    frame_id: impl Fn(&ComponentRequest<'_>, &ComponentFrame) -> Result<FrameId, CompositeError>,
) -> Result<Vec<ComponentDraw>, CompositeError> {
    let slots = slot_order(cof, dir, frame)?;
    let mut out = Vec::with_capacity(slots.len());
    for slot in slots {
        let req = ComponentRequest {
            cof,
            dir,
            frame,
            slot,
            layer: &cof.layers[slot.layer],
        };
        let at = |error| CompositeError::Scene {
            slot: slot.slot,
            error,
        };
        let cf = resolver.frame(&req)?;
        let id = frame_id(&req, &cf)?;
        let (x, y) = resolver.place(&req, &cf)?;
        let mut item = DrawItem::new(id, x, y);
        item.clip = unit.clip;
        item.shade = resolver.shade(&req)?;
        item.blend = resolver.blend(&req)?;
        item.key = DrawKey::new(unit.pass, unit.major, unit.minor, slot.slot).map_err(at)?;
        item.tag = unit.tag;
        out.push(ComponentDraw {
            slot,
            frame: cf,
            item,
        });
    }
    Ok(out)
}
