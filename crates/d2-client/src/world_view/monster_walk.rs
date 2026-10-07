// Spec: specs/client/model.md (§8 rule 1, open question 1); preview fills: docs/PLAN.md decisions D1–D2
//! The play preview's monster motion in the app: [`MonsterWalk`] holds
//! the [`MonsterMotion`] tracks and [`monster_walk_frame`] steps them on
//! the model after each bridge frame, before the unit mirror and the
//! draw (d2rs-own, unverified; see [`crate::bridge::motion`]). Only
//! `play` adds it.

use bevy::prelude::*;

use crate::bridge::mirror::{bridge_frame, mirror_units};
use crate::bridge::motion::MonsterMotion;
use crate::bridge::BridgeResource;

/// The monster tracks of the play preview.
#[derive(Resource, Debug, Default)]
pub struct MonsterWalk(pub MonsterMotion);

/// Steps the monster tracks after the bridge frame.
pub fn monster_walk_frame(mut bridge: ResMut<BridgeResource>, mut walk: ResMut<MonsterWalk>) {
    bridge.0.preview_motion(&mut walk.0);
}

/// Adds [`MonsterWalk`] and [`monster_walk_frame`].
pub fn add_monster_walk(app: &mut App) {
    app.init_resource::<MonsterWalk>().add_systems(
        PreUpdate,
        monster_walk_frame
            .after(bridge_frame)
            .before(mirror_units)
            .run_if(resource_exists::<BridgeResource>),
    );
}
