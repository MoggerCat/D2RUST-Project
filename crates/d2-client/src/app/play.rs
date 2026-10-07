// Spec: specs/client/bridge.md (§7, §8), specs/client/render-pipeline.md (A1, A9)
//! `d2-client play`: a window running the local single-player game.
//!
//! Each Bevy frame: the bridge frame in `PreUpdate` (`pump` the
//! in-process server: drain → tick → flush; `receive` and dispatch,
//! `bridge.md` §8), then the world view in `Update` (draw list from the
//! client world model, composed by the GPU compositor's render-graph node,
//! presented at the integer scale, render-pipeline §A1, §A9).
//!
//! What the window shows is what the specs allow: the S→C handlers of
//! `client/model.md`, `msg-units.md` and `msg-stats-items.md` fill the
//! client world model, and every rule of how a unit, tile or panel looks
//! is a world-view hook (each names its owner spec) answered here by
//! `world_view::Unspecified` (draw nothing: the model lacks their inputs,
//! `model_feed::PENDING`). Placement is the original's: the world view builds each
//! frame through `rules::OriginalView` with the camera of
//! `render/camera.md` §3, fed by `world_view::ModelFeed` (the local
//! player's position, unit positions and BlankScreen from the model; the
//! open mode is the original UI's, else 0, `ui/panels-2.md` §22 r5; the
//! shake and the map stay `NoFeed`'s, `model_feed::PENDING`). No player
//! in the model: no camera, nothing placeable.
//! One frame per server tick (§9). The frame is the composed empty list: palette index 0 over
//! the whole view. The frame palette is the act's `pal.pl2`
//! (`render/composition.md` §4) of the model's palette act
//! (`client/model.md` §11), presented by [`super::palette`] when the
//! user's archives are read; without them it is [`unspecified_palette`].
//!
//! Frames come from the frame store (`ViewAssets::frames`, the store of
//! verify-map; empty until a rule names a frame set to load), UI text goes
//! through `ui::text::layout_text` (`world_view::text_sprites`), and the
//! audio core plays from the sound pool after each bridge frame
//! ([`super::sound`]; the user's archives with `D2_GAME_DIR`).
//!
//! With the user's files ([`run`] on live data) the original UI is added
//! ([`super::ui`]: the `ui/panels.md` panels in the world view's root,
//! hotkeys from the `dev` bindings, panel art from the archives, the UI
//! flags' open mode as the camera's) and the audio runs the original
//! sound layer (`AudioParts::original`: `sounds.txt`, the 1.14d WAV
//! decoder, one sound tick per server tick, the UI's sound requests).

use bevy::prelude::*;
use d2_formats::palette::{Palette, Rgb};

use d2_server::host::SystemClock;

use super::palette::{self, ActPalettes};
use super::single_player::{self, GameData};
use super::sound::{self, AudioParts, GameAudio};
use super::ui;
use crate::bridge::drlg::DrlgSource;
use crate::bridge::mirror::DynLink;
use crate::bridge::world::{ClientTables, LevelRow};
use crate::bridge::{Bridge, BridgeError, BridgePlugin, BridgeResource};
use crate::world_view::node::NodeRuns;
use crate::world_view::{
    ModelFeed, NoFeed, Unspecified, ViewAssets, WorldViewPlugin, WorldViewState,
};

/// Frames between two progress lines in the log.
const LOG_EVERY: u64 = 250;

/// The frame palette before an act palette is presented: all zeros
/// (black). With the user's archives [`super::palette`] replaces it with
/// the `pal.pl2` of the model's palette act (`render/composition.md` §4,
/// `client/model.md` §11 rules 2, 4); without them (synthetic data) there
/// is no palette file and it stays. Index 0, the only index an empty
/// frame shows, is black in every act palette (`composition.md` §4).
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
            Box::new(ModelFeed::<NoFeed>::default()),
        ))
        .add_systems(Last, log_progress);
    sound::add_audio(app, AudioParts::empty());
    Ok(())
}

/// The local client's C→S 0x67 ([`single_player::create_request`])
/// through the bridge's send path (system queue, `client/bridge.md` §4),
/// before the first frame: the first pump drains it (game creation,
/// `intents-events.md` §8.1). The bridge answers the 0x02 that follows
/// with C→S 0x6B on its own (`client/model.md` §7 rule 3): the join.
pub fn send_create_game(app: &mut App) -> Result<(), BridgeError> {
    send_create_game_for(app, &single_player::Character::New)
}

/// [`send_create_game`] for `character` (a save's class and name,
/// [`single_player::create_request_for`]).
pub fn send_create_game_for(
    app: &mut App,
    character: &single_player::Character,
) -> Result<(), BridgeError> {
    app.world_mut()
        .resource_mut::<BridgeResource>()
        .0
        .send(&single_player::create_request_for(character))?;
    Ok(())
}

/// The client data of the game (`client/model.md` §11, §12 rule 1): the
/// client DRLG's source and the `Levels.txt` rows go to the bridge; the
/// rows also answer BlankScreen in the world view's feed.
pub fn add_client_data(app: &mut App, drlg: DrlgSource, levels: Vec<LevelRow>) {
    let world = app.world_mut();
    let mut bridge = world.resource_mut::<BridgeResource>();
    bridge.0.set_drlg_source(Some(drlg));
    bridge.0.set_tables(ClientTables {
        levels: levels.clone(),
        ..ClientTables::default()
    });
    let mut state = world.resource_mut::<WorldViewState>();
    state.feed = Box::new(ModelFeed {
        levels: Some(levels),
        ..ModelFeed::<NoFeed>::default()
    });
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
    /// The character the join loads (`--save`).
    pub character: single_player::Character,
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

/// `0x00410A80` (`render/lighting.md` §10 r4): wall-clock seconds, a
/// client-only host input.
fn wall_seconds() -> i32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i32)
}

/// Opens the window and runs the game until it is closed.
pub fn run(config: PlayConfig) -> anyhow::Result<AppExit> {
    let archives = match &config.data {
        GameData::Live(d) => Some(d.archives.clone()),
        GameData::Synthetic => None,
    };
    let drlg_source = single_player::client_drlg_source(&config.data);
    let level_rows = single_player::client_level_rows(&config.data);
    let object_rows = single_player::client_object_rows(&config.data);
    let request = config.character.clone();
    let (link, started) = single_player::start_with(
        config.data,
        config.seed,
        config.character,
        SystemClock::default(),
    )?;
    // Before the app exists, so not through Bevy's log.
    println!(
        "single player: seed {}, waypoint unit {:?} (GUID {})",
        config.seed, started.waypoint, started.waypoint_guid
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
    send_create_game_for(&mut app, &request)?;
    add_client_data(&mut app, drlg_source, level_rows);
    app.world_mut()
        .resource_mut::<BridgeResource>()
        .0
        .set_object_rows(object_rows);
    if let Some(archives) = archives {
        let skills = single_player::client_skill_rows(&archives)?;
        let skill_tables = single_player::client_skill_tables(&archives)?;
        {
            let mut bridge = app.world_mut().resource_mut::<BridgeResource>();
            bridge.0.set_skill_rows(skills);
            bridge.0.set_skill_tables(std::sync::Arc::new(skill_tables));
        }
        let units = single_player::client_unit_rows(&archives)?;
        app.world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .set_unit_rows(units);
        app.world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .set_wall_seconds(wall_seconds);
        let palettes = ActPalettes::live(&archives).map_err(anyhow::Error::msg)?;
        palette::add_act_palettes(&mut app, palettes);
        let parts = ui::UiParts::live(archives.clone()).map_err(anyhow::Error::msg)?;
        ui::add_original_ui(&mut app, parts)?;
        let table = sound::sound_table_live(&archives).map_err(anyhow::Error::msg)?;
        app.insert_resource(GameAudio::new(AudioParts::original(archives, table)));
    }
    sound::add_output(&mut app);
    if let Some(frames) = config.exit_after {
        app.insert_resource(ExitAfter(frames))
            .add_systems(Update, exit_after);
    }
    Ok(app.run())
}
