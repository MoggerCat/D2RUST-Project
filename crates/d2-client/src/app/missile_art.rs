// Spec: specs/render/unit-composite.md (§9)
//! The play app's effect rows (`skills`, `missiles`, `overlay`, `states`
//! `.bin`), read from the user's tables and handed to the world view's
//! client missile layer (`world_view::missiles`).

use std::sync::Arc;

use bevy::prelude::*;
use d2_data::bin::TableFiles;
use d2_data::tables::{decode_all, Missiles, Overlay, Skills, States};

use crate::assets::path::FileSource;
use crate::world_view::missiles::{EffectRows, Missiles as MissileLayer};
use crate::world_view::WorldViewState;

/// The effect rows of the user's tables. A table that is not there reads
/// as no rows (nothing is drawn for it).
pub fn effect_rows(archives: &dyn TableFiles) -> Result<EffectRows, String> {
    let set = d2_data::bin::load_from(archives, "eng").map_err(|e| e.to_string())?;
    fn all<R: d2_data::tables::Record>(
        set: &d2_data::bin::BinSet,
        name: &str,
    ) -> Result<Vec<R>, String> {
        match set.table(name) {
            Some(t) => decode_all(t).map_err(|e| e.to_string()),
            None => Ok(Vec::new()),
        }
    }
    Ok(EffectRows::from_tables(
        &all::<Skills>(&set, "skills")?,
        &all::<Missiles>(&set, "missiles")?,
        &all::<Overlay>(&set, "overlay")?,
        &all::<States>(&set, "states")?,
    ))
}

/// Hands the effect rows to the world view's missile layer (with `source`
/// for the DCC / DC6 files). Call after the world view state exists.
pub fn add_missiles(app: &mut App, source: Arc<dyn FileSource>, rows: EffectRows) {
    if let Some(mut state) = app.world_mut().get_resource_mut::<WorldViewState>() {
        state.missiles = MissileLayer::new(source, rows);
    }
}
