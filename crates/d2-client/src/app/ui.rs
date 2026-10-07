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

use std::sync::Arc;

use bevy::prelude::*;
use d2_data::tables::{decode_all, Inventory};
use d2_formats::mpq::ArchiveSet;

use crate::assets::path::FileSource;
use crate::controls::Preset;
use crate::ui::layout::Screen;
use crate::ui::original::{InvArea, OriginalUi, OriginalUiError, UiConfig};
use crate::ui::{NoPanelRules, NoStrings, UiRoot};
use crate::world_view::panel_art::{PanelArtLoader, PanelArtRules};
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
/// the placeholder rules. The 800 × 600 frame is resolution mode 2.
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
    if let Some(mut state) = app.world_mut().get_resource_mut::<WorldViewState>() {
        state.rules = Box::new(PanelArtRules {
            rules: Unspecified,
            files,
        });
    }
    Ok(())
}
