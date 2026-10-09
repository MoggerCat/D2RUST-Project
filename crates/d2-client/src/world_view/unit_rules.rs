// Spec: specs/render/unit-composite.md (§2, §3, §5.1, §6, §10), specs/render/draw-order.md (§10), specs/render/shading.md (§3 r1)
//! [`UnitRules`]: the unit hooks of [`ViewRules`] answered through
//! `rules::unit_composite` from the unit art the [`UnitArtLoader`] made
//! resident. Every other hook (tiles, UI) is the wrapped rules'.
//!
//! The play preview (D1, `docs/PLAN.md` "First playable preview") fills
//! what the model lacks, each fill marked `d2rs-own, unverified`:
//! - direction: the model holds no client path record; `dir64` is the
//!   unit art's preview facing (`UnitArt::dir64`: the predicted facing of
//!   the local player, else the facing toward the walk target or the last
//!   position change, else 0), mapped to the COF row with §3 r3's
//!   expected count (`UnitArt::expected_directions`);
//! - frame: objects take the model's +0x44 frame as is (§3 r2; the
//!   client object update animates it); players, monsters and missiles
//!   the COF's animation rate advanced by the server tick count (the
//!   client model runs no animation for them yet);
//! - the COF box pre-test (§4) is the view's (`OriginalView`, which
//!   knows the final screen position);
//! - draw key: pass 6 at major / minor 0 when the source states no draw
//!   order (`OriginalView` overwrites it when it does);
//! - shade: none (full bright, `shading.md` §3 r1 `v = 0xFF`); blend:
//!   opaque (COF layer translucency not applied).
//!
//! [`UnitArtLoader`]: super::unit_assets::UnitArtLoader

use std::sync::Arc;

use crate::bridge::world::{ClientWorld, MONSTER, OBJECT};
use crate::bridge::ClientUnit;
use d2_formats::cof::Cof;

use crate::composite::{
    ComponentDraw, ComponentFrame, ComponentRequest, CompositeError, UnitParams,
};
use crate::frames::IndexFrame;
use crate::rules::draw_order::OrderKey;
use crate::rules::unit_composite::{
    component_cel, file_format, frame_index, unit_direction, CompositeKind,
};
use crate::scene::{order::pass, BlendOp, DrawItem, ItemTag, Rect, ShadeChain};
use crate::ui::{ImageRequest, TextRequest};

use super::unit_assets::{component_codes, unit_cof, SharedUnitArt, UnitLooks};
use super::{SlotCall, TileDraw, UiRules, UiSprite, UnitPose, ViewAssets, ViewError, ViewRules};

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
        // §3 r2: +0x44 >> 8 as is (no bound, §3 r6) where the model
        // animates the unit: objects (the client object update,
        // `world/objects-client.md` §26 generic step).
        if unit.key.unit_type == OBJECT {
            return frame_index(unit.frame as u32);
        }
        // Monsters: the model's own frame (`bridge::monster_anim`, measured
        // against 1.14d for the Act I town NPCs).
        if unit.key.unit_type == MONSTER && unit.frame_count > 0 {
            return frame_index(unit.frame as u32);
        }
        // §3 r2 (measured for players; PROVISIONAL (REC-512) for monsters
        // without a model frame and for missiles): the draw of tick T has
        // had T − 1 advances of the 8.8 rate.
        tick_frame(world.server_ticks, frames, rate)
    }
}

/// `blend-modes.md` §5 r3 revision (PROVISIONAL, REC-518): object
/// `class` in `mode` casts its composite shadow when its `BlocksLight` of
/// that mode is ≠ 0; a class without a row casts it.
pub fn object_casts_shadow(looks: &UnitLooks, class: u32, mode: u32) -> bool {
    looks.object_blocks_light.get(&class).is_none_or(|b| {
        usize::try_from(mode)
            .ok()
            .and_then(|m| b.get(m))
            .is_some_and(|&v| v != 0)
    })
}

/// The player's walk speed (`sim/units.md` §4.7 step 7: w = 213 for a
/// player not running, p = 100 without item / skill velocity).
pub const PLAYER_WALK_SPEED: u64 = 213;

/// `sim/units.md` §4.7 step 7 revision (measured on `a1-walk-*`,
/// PROVISIONAL REC-516): the walk frame at server tick `tick` of a walk
/// that started on `since` (kept across re-targeting clicks):
/// `((tick − since) · 213 >> 8) mod frames`.
pub fn walk_frame(tick: u64, since: u64, frames: u8) -> usize {
    let frames = u64::from(frames.max(1));
    (((tick.saturating_sub(since) * PLAYER_WALK_SPEED) >> 8) % frames) as usize
}

/// §3 r2: the frame drawn at server tick `tick`, `((tick − 1) · rate
/// >> 8) mod frames`.
pub fn tick_frame(tick: u64, frames: u8, rate: u32) -> usize {
    let frames = u64::from(frames.max(1));
    ((tick.saturating_sub(1).wrapping_mul(u64::from(rate)) >> 8) % frames) as usize
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
        let n = art.expected_directions(unit, name.kind, cof.directions);
        let Ok(dir) = unit_direction(cof.directions, n, art.dir64(unit), dead) else {
            return Ok(None);
        };
        Ok(Some(UnitPose {
            cof: path,
            dir: usize::from(dir.cof_dir),
            dir64: dir.dir64,
            frame: if art.spin == Some(unit.key) {
                super::skill_motion::spin_frame(
                    world.server_ticks,
                    cof.animation_rate,
                    usize::from(cof.frames),
                )
            } else if let Some(since) = art
                .pose_since
                .filter(|(k, _)| *k == unit.key && matches!(unit.mode, 2 | 6))
                .map(|(_, s)| s)
            {
                walk_frame(world.server_ticks, since, cof.frames)
            } else {
                Self::frame(world, unit, cof.frames, cof.animation_rate)
            },
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

    /// The shadows of the composite just built (`unit_shadow`).
    fn unit_shadows(
        &self,
        _world: &ClientWorld,
        unit: &ClientUnit,
        pose: &UnitPose,
        at: Option<OrderKey>,
        draws: &[ComponentDraw],
        assets: &ViewAssets,
    ) -> Result<Vec<DrawItem>, ViewError> {
        let (Some(at), Some(cof)) = (at, assets.cofs.get(&pose.cof)) else {
            return Ok(Vec::new());
        };
        // `blend-modes.md` §5 r3 revision (PROVISIONAL, REC-518): an
        // object casts the shadow only when its mode's BlocksLight ≠ 0.
        if unit.key.unit_type == OBJECT && !object_casts_shadow(&self.looks, unit.class, unit.mode)
        {
            return Ok(Vec::new());
        }
        super::unit_shadow::draws(cof, unit.key.guid, at, draws, assets)
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

    /// §6 r4: the slots whose request succeeds but whose file is in no
    /// archive (the loader's `None` entry); 1.14d still calls the drawer.
    fn unit_slot_calls(
        &self,
        unit: &ClientUnit,
        pose: &UnitPose,
        cof: &Cof,
    ) -> Result<Vec<SlotCall>, ViewError> {
        let art = self.art.read().unwrap_or_else(|e| e.into_inner());
        let unit = &*art.posed(unit);
        let Some(name) = unit_cof(&self.looks, unit) else {
            return Ok(Vec::new());
        };
        let slots = crate::composite::slot_order(cof, pose.dir, pose.frame).map_err(|error| {
            ViewError::Unit {
                unit_type: unit.key.unit_type,
                guid: unit.key.guid,
                error,
            }
        })?;
        let mut calls = Vec::new();
        for slot in slots {
            let Some(codes) = component_codes(&self.looks, unit, &name, &cof.layers[slot.layer])
            else {
                continue;
            };
            if let Some(None) = art.files.get(&codes.name()) {
                let format = file_format(&codes, unit.class, unit.mode as u8);
                if let Ok(path) = codes.path(format) {
                    calls.push(SlotCall {
                        slot: slot.slot,
                        layer: slot.layer,
                        path,
                    });
                }
            }
        }
        Ok(calls)
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
        let n = art.expected_directions(unit, name.kind, req.cof.directions);
        let dir = unit_direction(req.cof.directions, n, art.dir64(unit), false)
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
