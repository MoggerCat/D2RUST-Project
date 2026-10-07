// Spec: specs/render/lighting.md (§11 r1, §13), specs/render/blend-modes.md (§3, §7), specs/render/shading.md (§3, §5, §10)
//! The world view's `shade` and `blend` hooks answered by the original's
//! rules: [`LitRules`] wraps the view rules of a frame and gives every
//! unit component the cel ops of `blend-modes.md` §2 with the draw mode
//! of §3 and the light value of `lighting.md` §11 r1 read from the
//! frame's [`LightMap`]. Every other hook goes to the wrapped rules.
//!
//! The per-unit inputs the client model does not hold yet (the unit's
//! sub-tile, the ghostly flag, the unit override inputs, the hover
//! target, the component's remap) come from a [`LookFeed`]; an
//! implementation that cannot answer returns an error, never a default.

use crate::bridge::world::ClientWorld;
use crate::bridge::ClientUnit;
use crate::composite::{ComponentFrame, ComponentRequest, CompositeError, UnitParams};
use crate::frames::IndexFrame;
use crate::scene::{BlendOp, MapId, ShadeChain};
use crate::ui::{ImageRequest, TextRequest};
use crate::world_view::{TileDraw, UiRules, UiSprite, UnitPose, ViewAssets, ViewError, ViewRules};

use super::super::blend::{cel_ops, component_mode, unit_override, OverrideInput, MODE_HIGHLIGHT};
use super::super::shading::{hover_light, ShadeTables};
use super::map::LightMap;

/// The `blend-modes.md` §3 and `shading.md` §6 inputs of one component
/// draw that the client model does not state yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentLook {
    /// `g`: a monster whose monster-data flags have bit 0x40 (§3).
    pub ghostly: bool,
    /// The inputs of the unit override `r` (§3 r1–r3); `None` = the
    /// unit has no override (no fade, no transparent or ethereal item).
    pub override_input: Option<OverrideInput>,
    /// `h`: the unit is the hover target and highlightable (§3,
    /// `blend::hover_highlighted`).
    pub hovered: bool,
    /// The component's remap `P` (`shading.md` §6, `unit-composite.md`
    /// §7), `None` = no `P`.
    pub remap: Option<MapId>,
}

/// The per-unit inputs of [`LitRules`], each from the client model as its
/// owner spec states it; an implementation whose input the model does not
/// hold returns an error.
pub trait LookFeed {
    /// The unit's sub-tile (its position, `render/camera.md` §2;
    /// `client/model.md` §2, §3, §5); its light is the light-map cell read
    /// at sub-tile × 8 (`lighting.md` §11 r1, `0x004DD600`).
    fn light_subtile(&self, unit: &ClientUnit) -> Result<(i32, i32), String>;

    /// The component's look inputs: the colormap source and remap `P`
    /// (`render/unit-composite.md` §7, `render/shading.md` §6 r4), the
    /// ghostly flag, unit override and hover of the draw mode
    /// (`render/blend-modes.md` §3).
    fn look(&self, unit: &ClientUnit, req: &ComponentRequest<'_>) -> Result<ComponentLook, String>;
}

/// The lighting state of one drawn frame: the act's tables in the view's
/// map table and the frame's light map (`lighting.md` §1 r3: rebuilt per
/// drawn frame).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameLight {
    pub tables: ShadeTables,
    pub map: LightMap,
}

/// The draw mode of a component (`blend-modes.md` §3 decision): the COF
/// layer's override byte (byte 3) ≠ 0 gives `lv` (byte 4).
pub fn layer_mode(look: &ComponentLook, req: &ComponentRequest<'_>) -> u8 {
    let layer = (req.layer.override_translucency != 0).then_some(req.layer.new_translucency);
    let r = look.override_input.as_ref().and_then(unit_override);
    component_mode(look.ghostly, r, look.hovered, layer)
}

/// The cel ops of a component: mode (§3), light byte `v` = the low byte
/// of the unit's light-map word (`lighting.md` §11 r1, `shading.md` §3),
/// doubled and clamped for mode 7 (`shading.md` §5; GDI then uses `H`).
pub fn component_ops(
    light: &FrameLight,
    subtile: (i32, i32),
    look: &ComponentLook,
    req: &ComponentRequest<'_>,
) -> (ShadeChain, BlendOp) {
    let mode = layer_mode(look, req);
    let mut v = light.map.read(subtile.0 * 8, subtile.1 * 8).i;
    if mode == MODE_HIGHLIGHT {
        v = hover_light(v);
    }
    cel_ops(&light.tables, mode, look.remap, v)
}

/// View rules `R` with `shade` and `blend` answered from the frame's light
/// and a [`LookFeed`].
#[derive(Debug, Clone, Copy)]
pub struct LitRules<'a, R: ?Sized, F: ?Sized> {
    pub rules: &'a R,
    pub feed: &'a F,
    pub light: &'a FrameLight,
}

fn look_error(req: &ComponentRequest<'_>, what: &'static str, message: String) -> CompositeError {
    CompositeError::Unresolved {
        slot: req.slot.slot,
        component: req.slot.component,
        what,
        message,
    }
}

impl<R: ?Sized, F: LookFeed + ?Sized> LitRules<'_, R, F> {
    fn ops(
        &self,
        unit: &ClientUnit,
        req: &ComponentRequest<'_>,
    ) -> Result<(ShadeChain, BlendOp), CompositeError> {
        let at = self
            .feed
            .light_subtile(unit)
            .map_err(|m| look_error(req, "unit light", m))?;
        let look = self
            .feed
            .look(unit, req)
            .map_err(|m| look_error(req, "component look", m))?;
        Ok(component_ops(self.light, at, &look, req))
    }
}

impl<R: ViewRules + ?Sized, F: LookFeed + ?Sized> ViewRules for LitRules<'_, R, F> {
    fn tiles(&self, world: &ClientWorld, assets: &ViewAssets) -> Result<Vec<TileDraw>, ViewError> {
        self.rules.tiles(world, assets)
    }

    fn unit_pose(
        &self,
        world: &ClientWorld,
        unit: &ClientUnit,
    ) -> Result<Option<UnitPose>, ViewError> {
        self.rules.unit_pose(world, unit)
    }

    fn unit_params(
        &self,
        world: &ClientWorld,
        unit: &ClientUnit,
        pose: &UnitPose,
    ) -> Result<UnitParams, ViewError> {
        self.rules.unit_params(world, unit, pose)
    }

    fn component_frame(
        &self,
        unit: &ClientUnit,
        pose: &UnitPose,
        req: &ComponentRequest<'_>,
    ) -> Result<ComponentFrame, CompositeError> {
        self.rules.component_frame(unit, pose, req)
    }

    fn component_slot_frame(
        &self,
        unit: &ClientUnit,
        pose: &UnitPose,
        req: &ComponentRequest<'_>,
    ) -> Result<Option<ComponentFrame>, CompositeError> {
        self.rules.component_slot_frame(unit, pose, req)
    }

    fn place(
        &self,
        unit: &ClientUnit,
        pose: &UnitPose,
        req: &ComponentRequest<'_>,
        image: &IndexFrame,
    ) -> Result<(i32, i32), CompositeError> {
        self.rules.place(unit, pose, req, image)
    }

    fn shade(
        &self,
        unit: &ClientUnit,
        req: &ComponentRequest<'_>,
    ) -> Result<ShadeChain, CompositeError> {
        Ok(self.ops(unit, req)?.0)
    }

    fn blend(
        &self,
        unit: &ClientUnit,
        req: &ComponentRequest<'_>,
    ) -> Result<BlendOp, CompositeError> {
        Ok(self.ops(unit, req)?.1)
    }
}

impl<R: UiRules + ?Sized, F: ?Sized> UiRules for LitRules<'_, R, F> {
    fn ui_image(&self, req: &ImageRequest, assets: &ViewAssets) -> Result<UiSprite, ViewError> {
        self.rules.ui_image(req, assets)
    }

    fn ui_text(&self, req: &TextRequest, assets: &ViewAssets) -> Result<Vec<UiSprite>, ViewError> {
        self.rules.ui_text(req, assets)
    }

    fn ui_pass(&self) -> Result<u32, ViewError> {
        self.rules.ui_pass()
    }
}
