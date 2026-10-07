// Spec: specs/client/bridge.md
//! Bevy mirror (§7): the bridge as a resource, one bridge frame per Bevy
//! frame in `PreUpdate` (§8), and entities that mirror the client world
//! model's units. Components here are views, never game state.

use std::collections::BTreeMap;

use bevy::prelude::*;

use super::link::ServerLink;
use super::output::Output;
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

/// The outputs of the last bridge frame (§10 rule 4), handed over whole
/// at the end of [`bridge_frame`]; the output dispatcher takes them
/// before the input and UI systems. A frame replaces what an earlier
/// frame left, so one frame's outputs never mix with the next frame's.
#[derive(Resource, Default, Debug)]
pub struct FrameOutputs(pub Vec<Output>);

/// Mirror entity of each unit key.
#[derive(Resource, Default, Debug)]
pub struct MirrorIndex(pub BTreeMap<UnitKey, Entity>);

/// Adds the bridge frame and the mirror to `PreUpdate`, in that order.
pub struct BridgePlugin;

impl Plugin for BridgePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MirrorIndex>()
            .init_resource::<FrameOutputs>()
            .add_systems(
                PreUpdate,
                (bridge_frame, mirror_units)
                    .chain()
                    .run_if(resource_exists::<BridgeResource>),
            );
    }
}

/// One bridge frame (§8 rule 1), then its outputs are handed over (§10
/// rule 4). Errors go to Bevy's error handler.
pub fn bridge_frame(
    mut bridge: ResMut<BridgeResource>,
    mut outputs: ResMut<FrameOutputs>,
    time: Option<Res<Time<Real>>>,
) -> Result {
    // The host clock as `GetTickCount` (`world/objects-client.md` §25 r6):
    // wrapping milliseconds since the app started. An app without Bevy's
    // time (headless tests) keeps the value it set.
    if let Some(time) = time {
        bridge.0.set_now(time.elapsed().as_millis() as u32);
    }
    bridge.0.frame()?;
    outputs.0 = bridge.0.take_outputs();
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
