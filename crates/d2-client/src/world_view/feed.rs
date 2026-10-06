// Spec: specs/render/camera.md (§3, §8, §9, §10), specs/render/composition.md (§3 steps 1, 3), specs/client/render-pipeline.md (A1 stage 1)
//! The camera of a drawn frame, fed from the client world, and the frame
//! built through the original's view rules ([`rules::OriginalView`]).
//!
//! A [`ViewFeed`] answers what the client world model does not hold yet:
//! the local player's position (camera §3), the screen open mode (§1),
//! the running screen shake and the player seed it draws from (§8), and
//! (as a [`ViewSource`]) unit positions, unit offsets and the map tiles.
//! Each is a `TODO(spec: …)` hook of its owner; [`NoFeed`] is the
//! placeholder: no local player, no map, no shake, and an error for
//! anything that would need a rule.
//!
//! [`frame_camera`] computes the camera once per drawn frame (§3) with
//! the d2rs time base of §9: the shake envelope runs on `t = 40 × (server
//! ticks since the shake started)`, and the frame shows the model as it
//! stands after the presented tick (no interpolation; the caller draws
//! once per tick). [`build_frame`] builds through `OriginalView`; with no
//! local player there is no camera, and the frame is built through
//! [`NoCamera`], which refuses every tile and unit (nothing can be placed
//! without the §3 origins) and passes the UI through.

use d2_sim::rng::Seed;

use crate::bridge::world::ClientWorld;
use crate::bridge::ClientUnit;
use crate::composite::{ComponentFrame, ComponentRequest, CompositeError, UnitParams};
use crate::frames::IndexFrame;
use crate::rules::camera::shake_offsets;
use crate::rules::{
    Camera, FrameSize, MapTile, OpenMode, OriginalView, Shake, UnitPosition, ViewSource,
};
use crate::scene::{BlendOp, ShadeChain};
use crate::ui::{ImageRequest, TextRequest, UiDraw};

use super::WorldFrame;
use super::{build, TileDraw, UiRules, UiSprite, UnitPose, ViewAssets, ViewError, ViewRules};

const CAMERA: &str = "render/camera.md";

/// A screen shake started on server tick `start_tick` (camera §8, §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunningShake {
    pub shake: Shake,
    pub start_tick: u64,
}

/// The camera inputs the client world model does not hold yet, plus the
/// [`ViewSource`] answers. One per app (it keeps the client's copy of
/// the player seed between frames).
pub trait ViewFeed: ViewSource {
    /// TODO(spec: the S→C owner spec of the local player's position)
    /// (camera §3, `[0x007A6A70]`): the local player's position as the
    /// client keeps it; `None` = the model states no local player.
    fn player(&self, world: &ClientWorld) -> Result<Option<UnitPosition>, ViewError>;

    /// TODO(spec: ui/panels.md) (camera §1): the screen open mode.
    fn open_mode(&self, world: &ClientWorld) -> Result<OpenMode, ViewError>;

    /// TODO(spec: the effect specs that call `0x00476A80`) (camera §8):
    /// the shake running at this frame, if any.
    fn shake(&self, world: &ClientWorld) -> Result<Option<RunningShake>, ViewError>;

    /// TODO(spec: render/camera.md open question 6): the client's copy of
    /// the local player unit's seed (`unit +0x20`), advanced by the two
    /// draws of each shaking frame.
    fn player_seed(&mut self, world: &ClientWorld) -> Result<&mut Seed, ViewError>;
}

/// The placeholder feed: the client world states no local player, no map
/// and no shake (the model holds none of them: `bridge.md` §5), and every
/// question that would need a rule is an error.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoFeed;

impl ViewSource for NoFeed {
    fn unit_position(&self, _: &ClientUnit) -> Result<UnitPosition, String> {
        Err("TODO(spec: the S→C owner spec of unit positions): no rule yet".into())
    }

    fn unit_offset(&self, _: &ClientUnit, _: &UnitPose) -> Result<(i32, i32), String> {
        Err("TODO(spec: render/unit-composite.md): no rule yet".into())
    }

    fn map_tiles(&self, _: &ClientWorld, _: &ViewAssets) -> Result<Vec<MapTile>, ViewError> {
        Ok(Vec::new())
    }
}

impl ViewFeed for NoFeed {
    fn player(&self, _: &ClientWorld) -> Result<Option<UnitPosition>, ViewError> {
        Ok(None)
    }

    fn open_mode(&self, _: &ClientWorld) -> Result<OpenMode, ViewError> {
        Err(ViewError::unresolved("screen open mode", "ui/panels.md"))
    }

    fn shake(&self, _: &ClientWorld) -> Result<Option<RunningShake>, ViewError> {
        Ok(None)
    }

    fn player_seed(&mut self, _: &ClientWorld) -> Result<&mut Seed, ViewError> {
        Err(ViewError::unresolved("local player seed", CAMERA))
    }
}

fn camera_error(what: &'static str, message: String) -> ViewError {
    ViewError::Unresolved {
        what,
        spec: CAMERA,
        message,
    }
}

/// The frame's shake offsets `(dx, dy)` (camera §8) on the d2rs time base
/// (§9): `t = 40 × (server ticks − start tick)`.
pub fn frame_shake<F: ViewFeed + ?Sized>(
    world: &ClientWorld,
    feed: &mut F,
) -> Result<(i32, i32), ViewError> {
    let Some(running) = feed.shake(world)? else {
        return Ok((0, 0));
    };
    let ticks = world
        .server_ticks
        .checked_sub(running.start_tick)
        .ok_or_else(|| {
            camera_error(
                "screen shake",
                format!(
                    "shake starts at tick {}, after the frame's tick {}",
                    running.start_tick, world.server_ticks
                ),
            )
        })?;
    let ticks = u32::try_from(ticks)
        .map_err(|_| camera_error("screen shake", format!("{ticks} ticks exceed 32 bits")))?;
    let a = running
        .shake
        .amplitude(Shake::time_of(ticks))
        .map_err(|e| camera_error("screen shake", e.to_string()))?;
    match a {
        None | Some(0) => Ok((0, 0)),
        Some(a) => Ok(shake_offsets(a, feed.player_seed(world)?)),
    }
}

/// The camera of the frame (camera §3), or `None` without a local player.
pub fn frame_camera<F: ViewFeed + ?Sized>(
    world: &ClientWorld,
    feed: &mut F,
) -> Result<Option<Camera>, ViewError> {
    Ok(camera_and_mode(world, feed)?.map(|(camera, _)| camera))
}

/// [`frame_camera`] and the open mode it was computed with.
fn camera_and_mode<F: ViewFeed + ?Sized>(
    world: &ClientWorld,
    feed: &mut F,
) -> Result<Option<(Camera, OpenMode)>, ViewError> {
    let Some(player) = feed.player(world)? else {
        return Ok(None);
    };
    let mode = feed.open_mode(world)?;
    let shake = frame_shake(world, feed)?;
    Ok(Some((
        Camera::new(FrameSize::D2RS, mode, player.client(), shake),
        mode,
    )))
}

/// The open mode whose frames draw no world (`render/composition.md` §3
/// step 3: `0x00476BC0` is skipped in screen open mode 3).
const NO_WORLD_MODE: u8 = 3;

/// Builds the frame through the original's view rules: the camera once
/// (§3), then [`OriginalView`] over `rules` and `feed`; without a local
/// player, through [`NoCamera`]. In screen open mode 3 the camera (and its
/// shake draws) is still computed, but the world is skipped and only the
/// UI is built ([`NoWorld`], `render/composition.md` §3 steps 1 and 3).
pub fn build_frame<R, F>(
    world: &ClientWorld,
    ui: &[UiDraw],
    rules: &R,
    feed: &mut F,
    assets: &ViewAssets,
) -> Result<WorldFrame, ViewError>
where
    R: ViewRules + UiRules + ?Sized,
    F: ViewFeed + ?Sized,
{
    match camera_and_mode(world, feed)? {
        Some((camera, mode)) if mode.get() == NO_WORLD_MODE => build(
            world,
            ui,
            &NoWorld {
                view: &OriginalView::new(camera, rules, &*feed),
            },
            assets,
        ),
        Some((camera, _)) => build(world, ui, &OriginalView::new(camera, rules, &*feed), assets),
        None => build(
            world,
            ui,
            &NoCamera {
                rules,
                source: &*feed,
            },
            assets,
        ),
    }
}

/// The view of a frame without a camera (no local player): map tiles and
/// units cannot be placed (camera §3 needs the player), so any tile the
/// source lists and any unit the rules draw is an error; the UI, which
/// does not use the camera, goes to the wrapped rules.
#[derive(Debug, Clone, Copy)]
pub struct NoCamera<'a, R: ?Sized, S: ?Sized> {
    pub rules: &'a R,
    pub source: &'a S,
}

const NO_PLAYER: &str = "no local player position: the camera origins (§3) are undefined";

impl<R: ViewRules + ?Sized, S: ViewSource + ?Sized> ViewRules for NoCamera<'_, R, S> {
    fn tiles(&self, world: &ClientWorld, assets: &ViewAssets) -> Result<Vec<TileDraw>, ViewError> {
        let tiles = self.source.map_tiles(world, assets)?;
        if tiles.is_empty() {
            return Ok(Vec::new());
        }
        Err(camera_error(
            "tile placement",
            format!("{} map tiles; {NO_PLAYER}", tiles.len()),
        ))
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
        _: &ClientWorld,
        unit: &ClientUnit,
        _: &UnitPose,
    ) -> Result<UnitParams, ViewError> {
        Err(camera_error(
            "unit placement",
            format!(
                "unit ({}, {}) is drawn; {NO_PLAYER}",
                unit.key.unit_type, unit.key.guid
            ),
        ))
    }

    fn component_frame(
        &self,
        unit: &ClientUnit,
        pose: &UnitPose,
        req: &ComponentRequest<'_>,
    ) -> Result<ComponentFrame, CompositeError> {
        self.rules.component_frame(unit, pose, req)
    }

    fn place(
        &self,
        _: &ClientUnit,
        _: &UnitPose,
        req: &ComponentRequest<'_>,
        _: &IndexFrame,
    ) -> Result<(i32, i32), CompositeError> {
        Err(CompositeError::Unresolved {
            slot: req.slot.slot,
            component: req.slot.component,
            what: "placement",
            message: NO_PLAYER.into(),
        })
    }

    fn shade(
        &self,
        unit: &ClientUnit,
        req: &ComponentRequest<'_>,
    ) -> Result<ShadeChain, CompositeError> {
        self.rules.shade(unit, req)
    }

    fn blend(
        &self,
        unit: &ClientUnit,
        req: &ComponentRequest<'_>,
    ) -> Result<BlendOp, CompositeError> {
        self.rules.blend(unit, req)
    }
}

impl<R: UiRules + ?Sized, S: ?Sized> UiRules for NoCamera<'_, R, S> {
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

/// A frame without its world (`render/composition.md` §3 step 3, screen
/// open mode 3): no map tile and no unit is drawn (every unit counts as
/// hidden); the UI goes to the wrapped view.
#[derive(Debug, Clone, Copy)]
pub struct NoWorld<'a, V: ?Sized> {
    pub view: &'a V,
}

impl<V: ViewRules + ?Sized> ViewRules for NoWorld<'_, V> {
    fn tiles(&self, _: &ClientWorld, _: &ViewAssets) -> Result<Vec<TileDraw>, ViewError> {
        Ok(Vec::new())
    }

    fn unit_pose(&self, _: &ClientWorld, _: &ClientUnit) -> Result<Option<UnitPose>, ViewError> {
        Ok(None)
    }

    fn unit_params(
        &self,
        world: &ClientWorld,
        unit: &ClientUnit,
        pose: &UnitPose,
    ) -> Result<UnitParams, ViewError> {
        self.view.unit_params(world, unit, pose)
    }

    fn component_frame(
        &self,
        unit: &ClientUnit,
        pose: &UnitPose,
        req: &ComponentRequest<'_>,
    ) -> Result<ComponentFrame, CompositeError> {
        self.view.component_frame(unit, pose, req)
    }

    fn place(
        &self,
        unit: &ClientUnit,
        pose: &UnitPose,
        req: &ComponentRequest<'_>,
        image: &IndexFrame,
    ) -> Result<(i32, i32), CompositeError> {
        self.view.place(unit, pose, req, image)
    }

    fn shade(
        &self,
        unit: &ClientUnit,
        req: &ComponentRequest<'_>,
    ) -> Result<ShadeChain, CompositeError> {
        self.view.shade(unit, req)
    }

    fn blend(
        &self,
        unit: &ClientUnit,
        req: &ComponentRequest<'_>,
    ) -> Result<BlendOp, CompositeError> {
        self.view.blend(unit, req)
    }
}

impl<V: UiRules + ?Sized> UiRules for NoWorld<'_, V> {
    fn ui_image(&self, req: &ImageRequest, assets: &ViewAssets) -> Result<UiSprite, ViewError> {
        self.view.ui_image(req, assets)
    }

    fn ui_text(&self, req: &TextRequest, assets: &ViewAssets) -> Result<Vec<UiSprite>, ViewError> {
        self.view.ui_text(req, assets)
    }

    fn ui_pass(&self) -> Result<u32, ViewError> {
        self.view.ui_pass()
    }
}

#[cfg(test)]
mod tests;
