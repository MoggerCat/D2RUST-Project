// Spec: specs/render/unit-composite.md (§2, §3, §5.1, §6, §10), specs/render/draw-order.md (§10), specs/render/shading.md (§3 r1)
//! [`UnitRules`]: the unit hooks of [`ViewRules`] answered through
//! `rules::unit_composite` from the unit art the [`UnitArtLoader`] made
//! resident. Every other hook (tiles, UI) is the wrapped rules'.
//!
//! The play preview (D1, `docs/PLAN.md` "First playable preview") fills
//! what the model lacks, each fill marked `d2rs-own, unverified`:
//! - direction: `dir64` = 0 (the model holds no client path record);
//! - frame: the model's +0x44 frame when set, else the COF's animation
//!   rate advanced by the server tick count (the client model runs no
//!   animation yet);
//! - no COF box pre-test (§4: the screen position is the placement
//!   hook's; the view clips at the frame edge);
//! - draw key: pass 6 at major / minor 0 when the source states no draw
//!   order (`OriginalView` overwrites it when it does);
//! - shade: none (full bright, `shading.md` §3 r1 `v = 0xFF`); blend:
//!   opaque (COF layer translucency not applied).
//!
//! [`UnitArtLoader`]: super::unit_assets::UnitArtLoader

use std::sync::Arc;

use crate::bridge::world::ClientWorld;
use crate::bridge::ClientUnit;
use crate::composite::{ComponentFrame, ComponentRequest, CompositeError, UnitParams};
use crate::frames::IndexFrame;
use crate::rules::unit_composite::{component_cel, frame_index, unit_direction, CompositeKind};
use crate::scene::{order::pass, BlendOp, ItemTag, Rect, ShadeChain};
use crate::ui::{ImageRequest, TextRequest};

use super::unit_assets::{component_codes, unit_cof, SharedUnitArt, UnitLooks};
use super::{TileDraw, UiRules, UiSprite, UnitPose, ViewAssets, ViewError, ViewRules};

/// `rules` with the unit composite hooks of the play preview.
pub struct UnitRules<R> {
    pub rules: R,
    pub looks: Arc<UnitLooks>,
    pub art: SharedUnitArt,
}

impl<R> UnitRules<R> {
    /// The COF frame of `unit` this tick (module doc: d2rs-own,
    /// unverified).
    fn frame(world: &ClientWorld, unit: &ClientUnit, frames: u8, rate: u32) -> usize {
        let frames = usize::from(frames.max(1));
        if unit.frame > 0 {
            return frame_index(unit.frame as u32) % frames;
        }
        // d2rs-own, unverified (D1): 8.8 animation rate per tick.
        ((world.server_ticks.wrapping_mul(u64::from(rate)) >> 8) % frames as u64) as usize
    }

    /// `dir64` of the unit (d2rs-own, unverified: 0).
    fn dir64(_: &ClientUnit) -> u8 {
        0
    }
}

fn unresolved(req: &ComponentRequest<'_>, what: &'static str, message: String) -> CompositeError {
    CompositeError::Unresolved {
        slot: req.slot.slot,
        component: req.slot.component,
        what,
        message,
    }
}

impl<R: ViewRules> ViewRules for UnitRules<R> {
    fn tiles(&self, world: &ClientWorld, assets: &ViewAssets) -> Result<Vec<TileDraw>, ViewError> {
        self.rules.tiles(world, assets)
    }

    /// §10: `None` unless the unit has a COF the loader made resident.
    fn unit_pose(
        &self,
        world: &ClientWorld,
        unit: &ClientUnit,
    ) -> Result<Option<UnitPose>, ViewError> {
        let art = self.art.read().unwrap_or_else(|e| e.into_inner());
        let unit = &*art.posed(unit);
        let Some(name) = unit_cof(&self.looks, unit) else {
            return Ok(None);
        };
        let Ok(path) = name.path() else {
            return Ok(None);
        };
        let Some(cof) = art.cofs.get(&path) else {
            return Ok(None);
        };
        let dead = name.kind != CompositeKind::Object && unit.is_dead();
        let Ok(dir) = unit_direction(cof.directions, cof.directions, Self::dir64(unit), dead)
        else {
            return Ok(None);
        };
        Ok(Some(UnitPose {
            cof: path,
            dir: usize::from(dir.cof_dir),
            frame: Self::frame(world, unit, cof.frames, cof.animation_rate),
        }))
    }

    fn unit_params(
        &self,
        _: &ClientWorld,
        unit: &ClientUnit,
        _: &UnitPose,
    ) -> Result<UnitParams, ViewError> {
        // d2rs-own, unverified (D1): `OriginalView` sets the draw key
        // from the draw order when its source has one, and the clip.
        Ok(UnitParams {
            pass: pass::WALLS_UNITS,
            major: 0,
            minor: 0,
            clip: Rect::FRAME,
            tag: ItemTag::Unit(unit.key.guid),
        })
    }

    fn component_frame(
        &self,
        unit: &ClientUnit,
        pose: &UnitPose,
        req: &ComponentRequest<'_>,
    ) -> Result<ComponentFrame, CompositeError> {
        self.component_slot_frame(unit, pose, req)?
            .ok_or_else(|| unresolved(req, "component frame", "the slot draws nothing".into()))
    }

    /// §5.1, §6: a failed request or a file that did not load draws
    /// nothing (§5 r2, §10).
    fn component_slot_frame(
        &self,
        unit: &ClientUnit,
        _: &UnitPose,
        req: &ComponentRequest<'_>,
    ) -> Result<Option<ComponentFrame>, CompositeError> {
        let art = self.art.read().unwrap_or_else(|e| e.into_inner());
        let unit = &*art.posed(unit);
        let Some(name) = unit_cof(&self.looks, unit) else {
            return Ok(None);
        };
        let Some(codes) = component_codes(&self.looks, unit, &name, req.layer) else {
            return Ok(None);
        };
        let Some(Some((path, facts))) = art.files.get(&codes.name()) else {
            return Ok(None);
        };
        let dir = unit_direction(
            req.cof.directions,
            req.cof.directions,
            Self::dir64(unit),
            false,
        )
        .map_err(|e| unresolved(req, "direction", e.to_string()))?;
        // d2rs-own, unverified (D1): a cel past the file is skipped, not
        // a frame error.
        Ok(component_cel(path, facts.directions, facts.frames, dir.dir64, req.frame).ok())
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

    /// d2rs-own, unverified (D1): full bright, no remap.
    fn shade(
        &self,
        _: &ClientUnit,
        _: &ComponentRequest<'_>,
    ) -> Result<ShadeChain, CompositeError> {
        Ok(ShadeChain::EMPTY)
    }

    /// d2rs-own, unverified (D1): opaque.
    fn blend(&self, _: &ClientUnit, _: &ComponentRequest<'_>) -> Result<BlendOp, CompositeError> {
        Ok(BlendOp::Opaque)
    }
}

impl<R: UiRules> UiRules for UnitRules<R> {
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

#[cfg(test)]
mod tests;
