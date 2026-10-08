// Spec: specs/client/model.md (§13 r6)
//! Gives the bridge the visibility predicate of the position check
//! (`client/model.md` §6 rule 6, §13 r6) from the world view
//! ([`crate::world_view::visibility`]): the unit art the original UI's
//! loader fills ([`super::ui::UnitArt`]) and the last drawn frame's
//! camera ([`WorldViewState::camera`]).
//!
//! Without the unit art (synthetic data: no unit is drawn) the predicate
//! still runs over an empty art store, so every unit reads as not visible
//! and the check corrects (rule 7) instead of refusing the message: no
//! COF is resident for rule 2 (PROVISIONAL REC-286, with the module doc
//! of `world_view::visibility`).

use std::sync::Arc;

use bevy::prelude::*;

use crate::bridge::BridgeResource;
use crate::world_view::unit_assets::UnitLooks;
use crate::world_view::visibility::ViewVisibility;
use crate::world_view::WorldViewState;

/// Installs the predicate on the bridge. Call it after the bridge, the
/// world view and (when there is one) the original UI's unit art exist.
pub fn add_visibility(app: &mut App) {
    let world = app.world();
    let Some(camera) = world
        .get_resource::<WorldViewState>()
        .map(|s| s.camera.clone())
    else {
        return;
    };
    let (looks, art) = world.get_resource::<super::ui::UnitArt>().map_or_else(
        || (Arc::new(UnitLooks::default()), Default::default()),
        |a| (a.0.looks.clone(), a.0.art.clone()),
    );
    let visible = ViewVisibility { looks, art, camera }.into_fn();
    if let Some(mut bridge) = app.world_mut().get_resource_mut::<BridgeResource>() {
        bridge.0.set_visibility(Some(visible));
    }
}
