// Spec: specs/client/bridge.md (§7, §8), specs/client/render-pipeline.md (A1, A9)
//! `d2-client play`: a window running the local single-player game.
//!
//! Each Bevy frame: the bridge frame in `PreUpdate` (`pump` the
//! in-process server: drain → tick → flush; `receive` and dispatch,
//! `bridge.md` §8), then the world view in `Update` (draw list from the
//! client world model, composed by the GPU compositor's render-graph node,
//! presented at the integer scale, render-pipeline §A1, §A9).
//!
//! What the window shows is what the specs allow: no S→C message has an
//! owner spec (`bridge-dispatch.tsv`), so the client world model stays
//! empty, and every rule of how a unit, tile or panel looks is a
//! `TODO(spec: …)` hook answered by `world_view::Unspecified` (draw
//! nothing). Placement is the original's: the world view builds each
//! frame through `rules::OriginalView` with the camera of
//! `render/camera.md` §3, fed by `world_view::NoFeed` until the owner
//! specs state the local player's position, the open mode, the shake,
//! unit positions and the map (no player: no camera, nothing placeable).
//! One frame per server tick (§9). The frame is the composed empty list: palette index 0 over
//! the whole view. The frame palette is the act's `pal.pl2`
//! (`render/composition.md` §4, `ViewAssets::from_pl2`); the model states
//! no level, so no act: all zeros until it does.
//!
//! Frames come from the frame store (`ViewAssets::frames`, the store of
//! verify-map; empty until a rule names a frame set to load), UI text goes
//! through `ui::text::layout_text` (`world_view::text_sprites`), and the
//! audio core plays from the sound pool after each bridge frame
//! ([`super::sound`]; the user's archives with `D2_GAME_DIR`).

use bevy::prelude::*;
use d2_formats::palette::{Palette, Rgb};

use d2_server::host::SystemClock;

use super::single_player::{self, GameData};
use super::sound::{self, AudioParts, GameAudio};
use crate::bridge::mirror::DynLink;
use crate::bridge::{Bridge, BridgeError, BridgePlugin, BridgeResource};
use crate::world_view::node::NodeRuns;
use crate::world_view::{NoFeed, Unspecified, ViewAssets, WorldViewPlugin, WorldViewState};

/// Frames between two progress lines in the log.
const LOG_EVERY: u64 = 250;

/// The frame palette while the model states no level for the player:
/// all zeros (black). TODO(spec: the S→C owner spec of the player's
/// level): then the act's `pal.pl2` (`render/composition.md` §4,
/// `ViewAssets::from_pl2`).
pub fn unspecified_palette() -> Palette {
    Palette {
        colors: [Rgb { r: 0, g: 0, b: 0 }; 256],
    }
}

/// Adds the bridge (on `link`), the world view and the audio frame (every
/// hook at its placeholder, an empty file source) to `app`: the play
/// mode's wiring, shared by the window and the headless tests. Add it
/// after Bevy's render plugin when one is used (the GPU node needs the
/// render world; without one the CPU reference is presented). Inserting
/// a [`WorldViewState`] or [`GameAudio`] afterwards replaces the defaults.
pub fn add_game(app: &mut App, link: DynLink, gpu: bool) -> Result<(), BridgeError> {
    let bridge = Bridge::new(link)?;
    app.add_plugins((BridgePlugin, WorldViewPlugin { gpu }))
        .insert_resource(BridgeResource(bridge))
        .insert_resource(WorldViewState::new(
            ViewAssets::new(unspecified_palette()),
            Box::new(Unspecified),
            Box::new(NoFeed),
        ))
        .add_systems(Last, log_progress);
    sound::add_audio(app, AudioParts::empty());
    Ok(())
}

/// One log line every [`LOG_EVERY`] bridge frames.
fn log_progress(
    bridge: Res<BridgeResource>,
    state: Res<WorldViewState>,
    runs: Option<Res<NodeRuns>>,
    audio: Option<Res<GameAudio>>,
) {
    let w = bridge.0.world();
    if w.frames == 0 || !w.frames.is_multiple_of(LOG_EVERY) {
        return;
    }
    info!(
        "frame {}: {} server ticks, {} units in the model, unowned S→C ids {:?}; last view {:?}; node frames {}; audio {:?}",
        w.frames,
        w.server_ticks,
        w.units.len(),
        bridge.0.log().unowned,
        state.last,
        runs.map_or(0, |r| r.get()),
        audio.map(|a| a.stats.clone()),
    );
}

/// What `d2-client play` runs.
pub struct PlayConfig {
    pub data: GameData,
    pub seed: u32,
    /// Close after this many frames (smoke test).
    pub exit_after: Option<u32>,
}

#[derive(Resource)]
struct ExitAfter(u32);

fn exit_after(limit: Res<ExitAfter>, mut seen: Local<u32>, mut exit: MessageWriter<AppExit>) {
    *seen += 1;
    if *seen >= limit.0 {
        info!("play: exiting after {} frames", *seen);
        exit.write(AppExit::Success);
    }
}

/// Opens the window and runs the game until it is closed.
pub fn run(config: PlayConfig) -> anyhow::Result<AppExit> {
    let archives = match &config.data {
        GameData::Live(d) => Some(d.archives.clone()),
        GameData::Synthetic => None,
    };
    let (link, started) = single_player::start(config.data, config.seed, SystemClock::default())?;
    // Before the app exists, so not through Bevy's log.
    println!(
        "single player: seed {}, player unit {:?}, waypoint unit {:?} (GUID {})",
        config.seed, started.player, started.waypoint, started.waypoint_guid
    );
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "d2rs".into(),
            ..default()
        }),
        ..default()
    }));
    add_game(&mut app, Box::new(link), true)?;
    if let Some(archives) = archives {
        app.insert_resource(GameAudio::new(AudioParts::unspecified(archives)));
    }
    sound::add_output(&mut app);
    if let Some(frames) = config.exit_after {
        app.insert_resource(ExitAfter(frames))
            .add_systems(Update, exit_after);
    }
    Ok(app.run())
}
