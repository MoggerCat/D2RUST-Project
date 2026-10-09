// Spec: specs/client/model.md (open question 2), specs/sim/pathing.md (§8.1–8.2), specs/ui/controls.md (§4.3 r1, §6 r7)
//! The play preview's own-walk motion in the app (decision D2 in
//! `docs/PLAN.md`): [`PreviewWalk`] holds the local player's
//! [`Predict`], the run / stand-still modifiers ([`RunMods`]) and the
//! charstats [`Speeds`], and [`preview_walk_frame`] runs it after each
//! bridge frame:
//!
//! - the walks the [`crate::bridge::predict::PredictLink`] recorded since
//!   the last frame ([`WalkTap`]) set the target, and a server tick steps
//!   the prediction (`Predict::frame`);
//! - the predicted position goes to the feed (camera centre and the local
//!   player's draw, `ViewFeed::set_local_prediction`) and the predicted
//!   mode (2 walk / 3 run) and facing to the unit art
//!   (`UnitArt::pose_mode`, `UnitArt::pose_dir`).
//!
//! The world view reads [`PreviewWalk`] for its clicks: the `mods` word
//! (`RunMods::word`) and the predicted position as the click's local
//! player (`Bridge::world_click_at`).
//!
//! d2rs-own, unverified. PROVISIONAL (client/model.md OQ2; REC-51). Only
//! `play` inserts it; the strict path never has one.

use bevy::prelude::*;

use crate::bridge::click::RunMods;

use crate::bridge::predict::{Predict, Speeds, WalkTap};
use crate::bridge::world::ClientWorld;
use crate::bridge::BridgeResource;

use super::unit_assets::SharedUnitArt;
use super::WorldViewState;

/// The local player's walk prediction of the play preview (module doc).
#[derive(Resource, Debug, Default)]
pub struct PreviewWalk {
    pub predict: Predict,
    pub run: RunMods,
    /// The class's charstats speeds; `None`: no charstats row (synthetic
    /// data), so the prediction only follows the model's placements.
    pub speeds: Option<Speeds>,
    /// The walks the link recorded.
    pub tap: WalkTap,
    /// The unit art whose drawn mode follows the prediction, when units
    /// are drawn.
    pub art: Option<SharedUnitArt>,
    /// The server tick count at the last frame.
    seen_ticks: u64,
}

impl PreviewWalk {
    pub fn new(tap: WalkTap, speeds: Option<Speeds>) -> Self {
        PreviewWalk {
            tap,
            speeds,
            ..Default::default()
        }
    }

    /// One bridge frame: the recorded walks, then a step if the server
    /// ticked since the last frame.
    pub fn frame(&mut self, world: &ClientWorld) {
        self.frame_with(world, &[]);
    }

    /// [`Self::frame`] with the `skills` rows: a skill on a unit sent
    /// since the last frame adds the server's approach run
    /// ([`crate::bridge::predict::approach_walk`]).
    pub fn frame_with(&mut self, world: &ClientWorld, skills: &[crate::bridge::world::SkillRow]) {
        let ticked = world.server_ticks != self.seen_ticks;
        self.seen_ticks = world.server_ticks;
        let mut walks = self.tap.take();
        walks.extend(
            self.tap
                .take_approaches()
                .into_iter()
                .filter_map(|a| crate::bridge::predict::approach_walk(world, skills, a)),
        );
        if self.tap.take_waypoint() {
            self.predict.waypoint_sent();
        }
        match self.speeds {
            Some(speeds) => self.predict.frame(world, walks, ticked, speeds),
            // No speeds: snaps only (no step).
            None => self.predict.observe(world),
        }
    }

    /// The predicted position keyed by the local player, for the feed.
    pub fn local_at(&self) -> Option<(crate::bridge::UnitKey, (u32, u32))> {
        Some((self.predict.player()?, self.predict.position()?))
    }
}

/// Runs [`PreviewWalk::frame`] after the bridge frame and hands the
/// prediction to the feed and the unit art.
pub fn preview_walk_frame(
    bridge: Res<BridgeResource>,
    mut walk: ResMut<PreviewWalk>,
    mut state: ResMut<WorldViewState>,
) {
    walk.frame_with(bridge.0.world(), &bridge.0.inputs().tables.skills);
    state.feed.set_local_prediction(walk.local_at());
    if let Some(art) = &walk.art {
        let mut art = art.write().unwrap_or_else(|e| e.into_inner());
        art.pose_mode = walk
            .predict
            .player()
            .and_then(|k| Some((k, walk.predict.mode()?)));
        art.pose_dir = walk
            .predict
            .player()
            .and_then(|k| Some((k, walk.predict.facing()?)));
        let speed = match walk.predict.mode() {
            Some(3) => super::unit_rules::player_run_speed(walk.speeds),
            _ => super::unit_rules::PLAYER_WALK_SPEED,
        };
        art.pose_since = walk
            .predict
            .player()
            .and_then(|k| Some((k, walk.predict.walk_since()?, speed)));
    }
}

/// Adds `walk` and [`preview_walk_frame`] (after the bridge frame, before
/// the unit mirror and the unit art loader).
pub fn add_preview_walk(app: &mut App, walk: PreviewWalk) {
    super::present::configure_preview_order(app);
    app.insert_resource(walk).add_systems(
        PreUpdate,
        preview_walk_frame
            .in_set(super::present::PreviewOrder::PlayerWalk)
            .run_if(resource_exists::<BridgeResource>)
            .run_if(resource_exists::<WorldViewState>),
    );
}
