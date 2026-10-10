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
        let ticked = world.server_ticks != self.seen_ticks;
        self.seen_ticks = world.server_ticks;
        let walks = self.tap.take();
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
    // `msg-units.md` §3 r2: the living monsters' footprints for the
    // client path (REC-706: at their model positions).
    let others = other_units(bridge.0.world(), &bridge.0.inputs().tables.monsters);
    let objects = other_objects(bridge.0.world(), &bridge.0.inputs().objclient.rows);
    walk.predict.set_others(others, objects);
    walk.frame(bridge.0.world());
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

/// The model's monsters not dying or dead (mode 0, 12) with a cell and a
/// `monstats2` row: their footprint inputs (`msg-units.md` §3 r2).
pub fn other_units(
    world: &ClientWorld,
    classes: &[Option<crate::bridge::world::MonsterClass>],
) -> Vec<crate::bridge::client_path::OtherUnit> {
    world
        .units
        .values()
        .filter(|u| u.key.unit_type == crate::bridge::world::MONSTER && !matches!(u.mode, 0 | 12))
        .filter_map(|u| {
            let (x, y) = u.position?;
            let c = classes.get(u.class as usize)?.as_ref()?;
            Some(crate::bridge::client_path::OtherUnit {
                x,
                y,
                size_x: c.size_x,
                npc: c.npc,
                in_town: c.in_town,
                interact: c.interact,
            })
        })
        .collect()
}

/// The model's objects whose footprint is on the client grid (stamped by
/// the 0x51 object init `0x004BC720` when `HasCollision[mode]` ≠ 0, not
/// yet freed by `0x00623830`: [`crate::bridge::world::ObjectData::footprint`],
/// `msg-units.md` §1.3 r2) with an `objects` row: their footprint inputs
/// (`bridge::client_path`).
pub fn other_objects(
    world: &ClientWorld,
    rows: &[crate::bridge::objects::ObjClientRow],
) -> Vec<crate::bridge::client_path::OtherObject> {
    world
        .units
        .values()
        .filter(|u| {
            u.key.unit_type == crate::bridge::world::OBJECT
                && matches!(&u.kind, crate::bridge::world::KindData::Object(d) if d.footprint)
        })
        .filter_map(|u| {
            let (x, y) = u.position?;
            let r = rows.get(u.class as usize)?;
            Some(crate::bridge::client_path::OtherObject {
                x,
                y,
                shape: r.shape,
            })
        })
        .collect()
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
