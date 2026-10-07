// Spec: specs/ui/panels.md (§2, §4.4, §5, §9.2), specs/client/ui.md (A2, A4, A6)
//! The play mode's UI: the original panels ([`OriginalUi`]) installed in
//! the world view's [`UiRoot`], the d2rs `dev` key bindings as the action
//! source (§A6; the original key table is `ui/controls.md`), and the
//! panel art read from the user's archives ([`PanelArtLoader`]) and drawn
//! by [`PanelArtRules`]. The root forwards the panels' intents through
//! the bridge; the UI flags' open mode is the camera's.
//!
//! Inert without game files: the art is the user's, so `play` adds the
//! UI only with `D2_GAME_DIR` (a frame that names a file no archive holds
//! is an error, never skipped).
//!
//! Units draw through [`UnitRules`] (the play preview of decision D1:
//! `world_view::unit_rules`), their COF and component files made resident
//! by [`UnitArtLoader`] before each frame. Without the unit tables in the
//! source, no unit is drawn and a log line says why.

use std::sync::Arc;

use bevy::prelude::*;
use d2_data::tables::{decode_all, Inventory};
use d2_formats::mpq::ArchiveSet;

use crate::assets::path::FileSource;
use crate::bridge::mirror::mirror_units;
use crate::bridge::BridgeResource;
use crate::controls::Preset;
use crate::ui::layout::Screen;
use crate::ui::original::{InvArea, OriginalUi, OriginalUiError, UiConfig};
use crate::ui::{NoPanelRules, NoStrings, UiRoot};
use crate::world_view::panel_art::{PanelArtLoader, PanelArtRules};
use crate::world_view::unit_assets::{SharedUnitArt, UnitArtLoader, UnitLooks};
use crate::world_view::unit_rules::UnitRules;
use crate::world_view::{UiSounds, Unspecified, WorldViewState, WorldViewUi};

/// What the play mode's UI reads from the install.
pub struct UiParts {
    /// The user's archives (panel DC6 files, §7.1).
    pub source: Arc<dyn FileSource>,
    /// `inventory.bin` `inv` rectangles by record (§9.2); `None`: the
    /// right panels take no pointer event.
    pub inv_areas: Option<Vec<InvArea>>,
    /// `d2exp.mpq` present (§Inputs).
    pub expansion_installed: bool,
}

impl UiParts {
    /// The parts of a live install: the `inventory` table of the user's
    /// `.bin` set (a load error is an error, not a fallback).
    pub fn live(archives: Arc<ArchiveSet>) -> Result<Self, String> {
        let set = d2_data::bin::load(&archives, "eng").map_err(|e| e.to_string())?;
        let table = set.table("inventory").ok_or("inventory not loaded")?;
        let rows: Vec<Inventory> = decode_all(table).map_err(|e| e.to_string())?;
        let expansion_installed = archives.has_archive("d2exp.mpq");
        Ok(UiParts {
            source: archives,
            inv_areas: Some(rows.iter().map(inv_area).collect()),
            expansion_installed,
        })
    }
}

/// An `inventory.bin` record's `inv` rectangle (§9.2: `0x0065C180`).
pub fn inv_area(r: &Inventory) -> InvArea {
    InvArea {
        left: r.invleft as i32,
        right: r.invright as i32,
        top: r.invtop as i32,
        bottom: r.invbottom as i32,
    }
}

/// Adds the original UI to an app set up by [`super::play::add_game`]:
/// the world view's UI (root with the wired panels, `dev` bindings, art
/// loader), [`UiSounds`] for the audio frame, and [`PanelArtRules`] over
/// [`UnitRules`] over the placeholder rules, with the unit art loader
/// ([`install_unit_rules`]). The 800 × 600 frame is resolution mode 2.
pub fn add_original_ui(app: &mut App, parts: UiParts) -> Result<(), OriginalUiError> {
    let config = UiConfig {
        screen: Screen::R800,
        expansion_installed: parts.expansion_installed,
    };
    let original = OriginalUi::new(config, parts.inv_areas)?;
    // `UiStates` is the authority and the root mirrors it (§2): the
    // root's own rules are never asked.
    let mut root = UiRoot::new(Box::new(NoPanelRules));
    original.install(&mut root)?;
    let files = original.files();
    let mut ui = WorldViewUi::new(root, Box::new(NoStrings));
    ui.original = Some(original);
    ui.bindings = Preset::Dev.bindings();
    ui.art = Some(PanelArtLoader {
        source: parts.source,
        files: files.clone(),
    });
    app.insert_non_send(ui).init_resource::<UiSounds>();
    let units = install_unit_rules(app, parts.source);
    if let Some(mut state) = app.world_mut().get_resource_mut::<WorldViewState>() {
        state.rules = Box::new(PanelArtRules {
            rules: units,
            files,
        });
    }
    Ok(())
}

/// The unit loader as a resource: [`load_unit_art`] runs it each frame.
#[derive(Resource)]
pub struct UnitArt(pub UnitArtLoader);

/// The unit rules of the play preview over [`Unspecified`], and the
/// loader system that fills their art from `source` (after the bridge
/// mirrored the frame's units, before the world view draws). The unit
/// tables come from `source`; when they do not load, the rules draw no
/// unit (D1: logged, not an error).
pub fn install_unit_rules(app: &mut App, source: Arc<dyn FileSource>) -> UnitRules<Unspecified> {
    let looks = match UnitLooks::live(source.as_ref()) {
        Ok(l) => l,
        Err(e) => {
            warn!("unit art: unit tables not loaded, no unit is drawn: {e}");
            UnitLooks::default()
        }
    };
    let looks = Arc::new(looks);
    let art = SharedUnitArt::default();
    app.insert_resource(UnitArt(UnitArtLoader {
        source,
        looks: looks.clone(),
        art: art.clone(),
    }))
    .add_systems(
        PreUpdate,
        load_unit_art
            .after(mirror_units)
            .run_if(resource_exists::<BridgeResource>)
            .run_if(resource_exists::<WorldViewState>),
    );
    UnitRules {
        rules: Unspecified,
        looks,
        art,
    }
}

/// Makes the unit files of the model's units resident (D1: a failed file
/// is logged once and skipped).
pub fn load_unit_art(
    loader: Res<UnitArt>,
    bridge: Res<BridgeResource>,
    mut state: ResMut<WorldViewState>,
) {
    for line in loader.0.ensure(bridge.0.world(), &mut state.assets) {
        warn!("{line}");
    }
}
