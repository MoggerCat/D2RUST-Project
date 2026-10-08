// Spec: specs/client/bridge.md (§7, §8), specs/client/render-pipeline.md (A1, A9), specs/flows/save-exit.md (§1, §4)
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
//! [`run`] turns the play preview on ([`add_preview`], decision D1 of
//! `docs/PLAN.md`): the feed builds the map from the client DRLG, its DT1
//! tiles are read from the user's archives (none on synthetic data: every
//! tile is skipped and logged) and drawn full bright, and the inputs the
//! model lacks get the labelled fills of `world_view::preview`
//! (`d2rs-own, unverified`). [`add_game`] alone stays strict.
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
use super::save;
use super::single_player::{self, GameData};
use super::sound::{self, AudioParts, GameAudio};
use super::ui;
use crate::bridge::drlg::DrlgSource;
use crate::bridge::mirror::DynLink;
use crate::bridge::predict::{PredictLink, WalkTap};
use crate::bridge::world::{ClientTables, LevelRow};
use crate::bridge::{Bridge, BridgeError, BridgePlugin, BridgeResource};
use crate::world_view::node::NodeRuns;
use crate::world_view::object_label::ObjectLabels;
use crate::world_view::preview::Preview;
use crate::world_view::tile_assets::TileAssets;
use crate::world_view::walk::{add_preview_walk, PreviewWalk};
use crate::world_view::weather_view::WeatherView;
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
        .add_systems(Last, log_progress)
        .add_systems(Update, super::save::end_of_game);
    sound::add_audio(app, AudioParts::empty());
    // The position check's visibility predicate over no unit art (every
    // unit reads as not visible, the check corrects); `run` installs it
    // again once the original UI's unit art exists.
    super::visibility::add_visibility(app);
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
    send_create_game_flags(app, character, None)
}

/// [`send_create_game_for`] with the 0x67 u32@0x27 the front end computed
/// (`front_start`, `start_flags`).
pub fn send_create_game_flags(
    app: &mut App,
    character: &single_player::Character,
    flags: Option<u32>,
) -> Result<(), BridgeError> {
    let mut request = single_player::create_request_for(character);
    if let Some(f) = flags {
        request.flags = f;
    }
    app.world_mut()
        .resource_mut::<BridgeResource>()
        .0
        .send(&request)?;
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

/// Turns the play preview on (decision D1, `world_view::preview`): the
/// model feed with the map from the client DRLG (`draw-order.md` §9),
/// the DT1 tiles and act shade tables of `tiles`, and the labelled fills
/// for the inputs the model lacks; a frame that still fails is logged,
/// not fatal. Nothing it draws is verified against 1.14d (rule 10).
pub fn add_preview(app: &mut App, levels: Vec<LevelRow>, tiles: TileAssets) {
    add_preview_lit(app, levels, tiles, None);
}

/// [`add_preview`] with the monster / missile light columns of the tables
/// (`world_view::light_sources`, d2rs-own, unverified).
pub fn add_preview_lit(
    app: &mut App,
    levels: Vec<LevelRow>,
    tiles: TileAssets,
    lights: Option<crate::world_view::light_sources::LightRows>,
) {
    add_preview_tinted(app, levels, tiles, lights, None);
}

/// [`add_preview_lit`] with the `colorpri` / `colorshift` of the `states`
/// table (`world_view::state_tint`, PROVISIONAL REC-245).
pub fn add_preview_tinted(
    app: &mut App,
    levels: Vec<LevelRow>,
    tiles: TileAssets,
    lights: Option<crate::world_view::light_sources::LightRows>,
    tints: Option<crate::world_view::state_tint::StateTints>,
) {
    let weather = WeatherView::new(tiles.source());
    let mut preview = Preview::new(tiles);
    preview.light.sources = lights.map(std::sync::Arc::new);
    preview.light.set_tints(tints.map(std::sync::Arc::new));
    let mut state = app.world_mut().resource_mut::<WorldViewState>();
    state.feed = Box::new(
        ModelFeed {
            levels: Some(levels),
            ..ModelFeed::<NoFeed>::default()
        }
        .with_preview(preview)
        .with_weather(weather),
    );
    state.preview = true;
}

/// `link` wrapped in the walk recorder of the play preview's own-walk
/// prediction (decision D2, `bridge::predict`; d2rs-own, unverified), and
/// the handle on what it records. Messages pass through unchanged.
pub fn predict_link(link: DynLink) -> (DynLink, WalkTap) {
    let link = PredictLink::new(link);
    let tap = link.tap();
    (Box::new(link), tap)
}

/// Turns the play preview's walk prediction on ([`PreviewWalk`] over the
/// walks `tap` records, at `speeds`), drawing the local player's
/// predicted mode through the unit art when the original UI installed it.
pub fn add_walk(app: &mut App, tap: WalkTap, speeds: Option<crate::bridge::predict::Speeds>) {
    let mut walk = PreviewWalk::new(tap, speeds);
    walk.art = app
        .world()
        .get_resource::<ui::UnitArt>()
        .map(|a| a.0.art.clone());
    add_preview_walk(app, walk);
    crate::world_view::monster_walk::add_monster_walk(app);
    crate::world_view::skill_motion::add_skill_motion(app);
    crate::world_view::walk_room::add_preview_walk_room(app);
}

/// One log line every [`LOG_EVERY`] bridge frames.
fn log_progress(
    bridge: Res<BridgeResource>,
    state: Res<WorldViewState>,
    runs: Option<Res<NodeRuns>>,
    audio: Option<Res<GameAudio>>,
    walk: Option<Res<PreviewWalk>>,
    mut seen_rejected: Local<usize>,
) {
    // Every refused S→C message once, as it happens (a refused 0x07 or
    // unit add leaves the model short with no other trace).
    let rejected = &bridge.0.log().rejected;
    for r in rejected.iter().skip(*seen_rejected) {
        warn!("S→C 0x{:02X} refused: {}", r.id, r.error);
    }
    *seen_rejected = rejected.len();
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
    let log = bridge.0.log();
    info!(
        "frame {}: {}; refused {}, dropped (unit not in the model) {:?}, discarded {}",
        w.frames,
        where_line(w, walk.as_deref()),
        log.rejected.len(),
        log.dropped,
        log.discarded.len(),
    );
}

/// The local player's place in the model for the progress log: its model
/// sub-tile, the predicted one, its room, the client's active rooms by
/// level and the model's units by type (0 player, 1 monster, 2 object…).
fn where_line(w: &crate::bridge::world::ClientWorld, walk: Option<&PreviewWalk>) -> String {
    let mut levels = std::collections::BTreeMap::<u16, usize>::new();
    for r in w.active_rooms.as_deref().unwrap_or(&[]) {
        *levels.entry(r.level).or_default() += 1;
    }
    let mut types = std::collections::BTreeMap::<u8, usize>::new();
    for k in w.units.keys() {
        *types.entry(k.unit_type).or_default() += 1;
    }
    format!(
        "local cell {:?}, predicted {:?}, local room {:?}, active rooms by level {:?}, units by type {:?}",
        w.local().map(|u| u.cell()),
        walk.and_then(|p| p.predict.cell()),
        w.local_room().map(|r| (r.level, r.x0, r.y0, r.w, r.h)),
        levels,
        types,
    )
}

/// What `d2-client play` runs.
pub struct PlayConfig {
    pub data: GameData,
    pub seed: u32,
    /// The character the join loads (`--save`).
    pub character: single_player::Character,
    /// Close after this many frames (smoke test).
    pub exit_after: Option<u32>,
    /// Where the character is saved on exit (`app::save::save_path`);
    /// `None`: not saved.
    pub save_path: Option<std::path::PathBuf>,
    /// A hardcore character (`play --new --hardcore`); a loaded save's own
    /// status bit makes it hardcore too ([`super::hardcore`]).
    pub hardcore: bool,
    /// The 0x67 flags the front end chose (`front_start`); `None`: the
    /// character's own.
    pub start_flags: Option<u32>,
}

#[derive(Resource)]
struct ExitAfter(u32);

/// Leaves the game through the server after the app stopped
/// (`flows/save-exit.md` §1 r2 – §4 r1): a client still in game sends
/// C→S 0x69 ([`Bridge::save_and_exit`]) and runs bridge frames until the
/// server's 0x05 takes it out of the game (at most 100 frames: the drain
/// of the next server frame answers it). Returns whether the client is
/// out of the game. PROVISIONAL (REC-291): the window close of 1.14d
/// runs the same exit path (`ui/frontend-options.md` §O3, `WM_CLOSE`);
/// its chain is `flows/save-exit.md` OQ1. d2rs-own, unverified.
pub fn leave_game<L: crate::bridge::link::ServerLink>(
    bridge: &mut Bridge<L>,
) -> Result<bool, BridgeError> {
    if bridge.world().in_game && !bridge.world().exit_requested {
        bridge.save_and_exit()?;
    }
    for _ in 0..100 {
        if !bridge.world().in_game {
            return Ok(true);
        }
        bridge.frame()?;
    }
    Ok(!bridge.world().in_game)
}

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
    let item_lookup = match &config.data {
        GameData::Live(d) => Some(d.tables.item_tables().map_err(|e| e.to_string())),
        GameData::Synthetic => None,
    };
    let drlg_source = single_player::client_drlg_source(&config.data);
    let level_rows = single_player::client_level_rows(&config.data);
    let waypoint_map = single_player::client_waypoint_map(&config.data);
    let object_rows = single_player::client_object_rows(&config.data);
    let object_names = single_player::client_object_names(&config.data);
    let request = config.character.clone();
    let loading_files = match &config.data {
        GameData::Live(d) => Some(d.archives.source()),
        GameData::Synthetic => None,
    };
    // d2rs-own, unverified: the map files sit next to the character save.
    let automap_files = config.save_path.as_deref().and_then(|p| {
        Some(super::automap::SaveFiles {
            dir: p.parent()?.to_path_buf(),
            sub: None,
            name: p.file_stem()?.to_string_lossy().into_owned(),
        })
    });
    let automap_source = match &config.data {
        GameData::Live(d) => {
            Some(super::automap::live_source(&d.tables).map_err(anyhow::Error::msg)?)
        }
        GameData::Synthetic => None,
    };
    let hire_rows = match &config.data {
        GameData::Live(d) => d.tables.hire_rows().map_err(anyhow::Error::msg)?,
        GameData::Synthetic => Vec::new(),
    };
    let speeds = single_player::walk_speeds(&config.data, &config.character)?;
    let mut save_base = save::base_save(&config.character);
    let hardcore =
        config.hardcore || save_base.header.status & d2_formats::d2s::status::HARDCORE != 0;
    if hardcore {
        save_base.header.status |= d2_formats::d2s::status::HARDCORE;
    }
    let save_tables: Option<std::sync::Arc<dyn d2_formats::d2s::SaveTables + Send + Sync>> =
        match &config.data {
            GameData::Live(d) => Some(std::sync::Arc::new(d.save.clone())),
            GameData::Synthetic => None,
        };
    let (mut link, started) = single_player::start_with(
        config.data,
        config.seed,
        config.character,
        SystemClock::default(),
    )?;
    if hardcore {
        link.with(|l| l.host_mut().game.events.action.hooks().x.hardcore = true)?;
    }
    // Before the app exists, so not through Bevy's log.
    println!(
        "single player: seed {}, waypoint unit {:?} (GUID {})",
        config.seed, started.waypoint, started.waypoint_guid
    );
    let mut app = App::new();
    // d2rs-own, unverified: the config folder is next to the saves; a bad
    // settings.toml or controls.toml stops here (no silent default).
    let cfg_dir = super::config::config_dir(
        &config
            .save_path
            .as_deref()
            .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
            .unwrap_or_else(super::save::default_save_dir),
    );
    let settings = super::config::load_settings(&cfg_dir)?;
    let bindings = super::config::load_controls(&cfg_dir)?;
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(super::config::window_for(&settings)),
        ..default()
    }));
    app.insert_resource(super::config::ConfigRes {
        dir: cfg_dir,
        settings,
        bindings,
    })
    .add_systems(Update, super::config::apply_settings);
    let (link, saver): (DynLink, Option<save::SaveHandle>) = match (config.save_path, save_tables) {
        (Some(path), Some(tables)) => {
            let (link, handle) = save::share(link, save_base, tables, path)?;
            (Box::new(link), Some(handle))
        }
        (path, _) => {
            if path.is_some() {
                println!("play: no save tables (synthetic data): the character is not saved");
            }
            (Box::new(link), None)
        }
    };
    let (link, tap) = predict_link(link);
    if let Some(h) = &saver {
        app.insert_resource(h.clone());
    }
    add_game(&mut app, link, true)?;
    send_create_game_flags(&mut app, &request, config.start_flags)?;
    add_client_data(&mut app, drlg_source, level_rows.clone());
    app.world_mut()
        .resource_mut::<BridgeResource>()
        .0
        .set_object_rows(object_rows);
    if let Some(mut state) = app.world_mut().get_resource_mut::<WorldViewState>() {
        state.object_labels = ObjectLabels::new(object_names);
    }
    if let Some(archives) = archives {
        let skills = single_player::client_skill_rows(archives.as_ref())?;
        let skill_tables = single_player::client_skill_tables(archives.as_ref())?;
        let class_skills = single_player::client_class_skills(archives.as_ref())?;
        {
            let mut bridge = app.world_mut().resource_mut::<BridgeResource>();
            bridge.0.set_skill_rows(skills);
            bridge.0.set_class_skills(class_skills);
            bridge.0.set_skill_tables(std::sync::Arc::new(skill_tables));
        }
        let units = single_player::client_unit_rows(archives.as_ref())?;
        app.world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .set_unit_rows(units);
        app.world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .set_wall_seconds(wall_seconds);
        let palettes = ActPalettes::live(archives.as_ref()).map_err(anyhow::Error::msg)?;
        let tiles = TileAssets::new(Some(archives.source()), Some(palettes.pl2.clone()));
        let lights = crate::world_view::light_sources::load(archives.as_ref())
            .map_err(|e| warn!("light rows (d2rs-own, unverified): {e}; player light only"))
            .ok();
        let tints = super::missile_art::state_tints(archives.as_ref())
            .map_err(|e| warn!("state tints (d2rs-own, unverified): {e}; no unit tinted"))
            .ok();
        add_preview_tinted(&mut app, level_rows, tiles, lights, tints);
        match crate::world_view::unit_facts::load(archives.as_ref()) {
            Ok(t) => {
                if let Some(mut state) = app.world_mut().get_resource_mut::<WorldViewState>() {
                    state.feed.set_unit_fact_tables(t);
                }
            }
            Err(e) => warn!("unit facts tables: {e}; draw order uses the preview fills only"),
        }
        let mut item_parts =
            super::items::item_parts(archives.as_ref()).map_err(anyhow::Error::msg)?;
        if let Some(lookup) = item_lookup {
            match lookup.and_then(|t| super::items::item_tips(archives.as_ref(), t)) {
                Ok(t) => {
                    app.world_mut()
                        .resource_mut::<BridgeResource>()
                        .0
                        .set_item_tables(std::sync::Arc::new(super::items::TableDecoder(
                            t.tables(),
                        )));
                    item_parts.tips = Some(t);
                }
                Err(e) => warn!("item tips (d2rs-own, unverified): {e}; no tool tips"),
            }
        }
        super::items::add_items(&mut app, archives.source(), item_parts);
        let effects =
            super::missile_art::effect_rows(archives.as_ref()).map_err(anyhow::Error::msg)?;
        super::missile_art::add_missiles(&mut app, archives.source(), effects);
        palette::add_act_palettes(&mut app, palettes);
        if let Some(source) = automap_source {
            super::automap::add_automap(&mut app, source, automap_files, archives.source(), true);
        }
        let parts = ui::UiParts::live(archives.clone()).map_err(anyhow::Error::msg)?;
        ui::add_original_ui(&mut app, parts)?;
        let strings = super::strings::TableStrings::load(archives.as_ref(), super::strings::LANG)
            .map_err(anyhow::Error::msg)?;
        super::strings::install_strings(&mut app, strings);
        super::hud::install_hud_tables(&mut app, archives.as_ref()).map_err(anyhow::Error::msg)?;
        super::hud::install_char_tables(&mut app, archives.as_ref()).map_err(anyhow::Error::msg)?;
        super::hud::install_skill_tree_tables(&mut app, archives.as_ref())
            .map_err(anyhow::Error::msg)?;
        ui::set_waypoint_map(&mut app, waypoint_map);
        ui::set_shop_prices(&mut app, started.prices.clone());
        super::hire_stats::install_hire_stats(&mut app, hire_rows, true);
        let table = sound::sound_table_live(archives.as_ref()).map_err(anyhow::Error::msg)?;
        app.insert_resource(GameAudio::new(AudioParts::original(
            archives.source(),
            table,
        )));
    } else {
        super::synthetic_client::install(&mut app);
        add_preview(&mut app, level_rows, TileAssets::default());
    }
    add_walk(&mut app, tap, speeds);
    super::visibility::add_visibility(&mut app);
    super::loading_overlay::add_loading(&mut app, loading_files);
    super::death::add_death(&mut app);
    super::hardcore::add_hardcore(&mut app, hardcore);
    sound::add_output(&mut app);
    if let Some(frames) = config.exit_after {
        app.insert_resource(ExitAfter(frames))
            .add_systems(Update, exit_after);
    }
    let exit = app.run();
    if let Some(a) = app
        .world_mut()
        .resource_mut::<WorldViewState>()
        .automap
        .as_mut()
    {
        if let Err(e) = a.teardown() {
            eprintln!("play: the automap was NOT saved: {e}");
        }
    }
    // Every way out leaves through the server (`flows/save-exit.md` §2):
    // a window closed in game sends the Save and Exit's 0x69 now, so the
    // server's leave writes the character before its 0x05.
    let left = leave_game(&mut app.world_mut().resource_mut::<BridgeResource>().0);
    match (&saver, left) {
        (Some(h), Ok(true)) => println!(
            "play: left the game; the server's leave wrote {}",
            h.path().display()
        ),
        (None, Ok(_)) => {}
        (_, Ok(false)) => eprintln!("play: the server never answered the leave"),
        (_, Err(e)) => eprintln!("play: the leave failed: {e}"),
    }
    Ok(exit)
}
