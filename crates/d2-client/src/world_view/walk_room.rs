// Spec: specs/client/model.md (§3 rule 3, §12 rule 2, open question 2), specs/sim/unit-order.md (§5 rule 6)
//! The play preview's own-walk room recache (decision D2 in
//! `docs/PLAN.md`): after [`super::walk::preview_walk_frame`] stepped the
//! prediction, the local player is linked to the client room of its
//! predicted sub-tile ([`crate::bridge::Bridge::recache_local_room`]).
//!
//! Without it the local player stays in the room of its last placement
//! while the prediction walks it away: the near rooms (`draw-order.md`
//! §9, the adjacency array of the player's room) never follow it, so no
//! ground is drawn past the start room's neighbours, and a server room
//! leave (S→C 0x08) of that room unlinks the player from every room (no
//! map at all). The 1.14d client's own path step moves its room with it
//! (`client/model.md` §3 rule 3).
//!
//! d2rs-own, unverified. PROVISIONAL (client/model.md OQ2; REC-51). Only
//! `play` adds it.

use bevy::prelude::*;

use crate::bridge::BridgeResource;

use super::walk::{preview_walk_frame, PreviewWalk};

/// Recaches the local player's room at the predicted sub-tile, and
/// records the prediction for the position check's rule 8
/// (`Bridge::set_local_walk`, REC-277).
pub fn preview_walk_room(mut bridge: ResMut<BridgeResource>, walk: Res<PreviewWalk>) {
    let cell = walk.predict.cell();
    if let Some((x, y)) = cell {
        bridge.0.recache_local_room(x, y);
    }
    bridge.0.set_local_walk(cell);
}

/// Adds [`preview_walk_room`] after the walk prediction's frame, before
/// the unit mirror.
pub fn add_preview_walk_room(app: &mut App) {
    app.add_systems(
        PreUpdate,
        preview_walk_room
            .after(preview_walk_frame)
            .before(crate::bridge::mirror::mirror_units)
            .run_if(resource_exists::<BridgeResource>)
            .run_if(resource_exists::<PreviewWalk>),
    );
}
