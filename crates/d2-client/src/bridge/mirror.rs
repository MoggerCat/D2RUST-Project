// Spec: specs/client/bridge.md
//! Bevy mirror (§7): the bridge as a resource, one bridge frame per Bevy
//! frame in `PreUpdate` (§8), and entities that mirror the client world
//! model's units. Components here are views, never game state.

use std::collections::BTreeMap;

use bevy::prelude::*;

use super::link::ServerLink;
use super::world::{ClientUnit, UnitKey};
use super::Bridge;

/// The link type the Bevy app holds.
pub type DynLink = Box<dyn ServerLink + Send + Sync>;

/// The bridge, owned by the Bevy world. Insert it before the first frame;
/// the plugin's systems run only while it exists.
#[derive(Resource)]
pub struct BridgeResource(pub Bridge<DynLink>);

/// View of one client unit (§7 rule 2): overwritten from the model.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct UnitView {
    pub key: UnitKey,
}

impl UnitView {
    fn of(unit: &ClientUnit) -> Self {
        Self { key: unit.key }
    }
}

/// Mirror entity of each unit key.
#[derive(Resource, Default, Debug)]
pub struct MirrorIndex(pub BTreeMap<UnitKey, Entity>);

/// Adds the bridge frame and the mirror to `PreUpdate`, in that order.
pub struct BridgePlugin;

impl Plugin for BridgePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MirrorIndex>().add_systems(
            PreUpdate,
            (bridge_frame, mirror_units)
                .chain()
                .run_if(resource_exists::<BridgeResource>),
        );
    }
}

/// One bridge frame (§8 rule 1). Errors go to Bevy's error handler.
pub fn bridge_frame(mut bridge: ResMut<BridgeResource>) -> Result {
    bridge.0.frame()?;
    Ok(())
}

/// Spawns, updates and despawns mirror entities from the model (§7
/// rule 3), in key order.
pub fn mirror_units(
    mut commands: Commands,
    bridge: Res<BridgeResource>,
    mut index: ResMut<MirrorIndex>,
    mut views: Query<&mut UnitView>,
) {
    let units = &bridge.0.world().units;
    index.0.retain(|key, &mut entity| {
        let keep = units.contains_key(key);
        if !keep {
            commands.entity(entity).despawn();
        }
        keep
    });
    for (key, unit) in units {
        let view = UnitView::of(unit);
        match index.0.get(key) {
            Some(&entity) => {
                if let Ok(mut v) = views.get_mut(entity) {
                    v.set_if_neq(view);
                }
            }
            None => {
                index.0.insert(*key, commands.spawn(view).id());
            }
        }
    }
}
