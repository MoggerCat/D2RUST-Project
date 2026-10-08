// Spec: specs/client/render-pipeline.md (A1 stages 4–5, A9 presentation), specs/client/ui.md (A4), specs/render/camera.md (§3, §9), specs/render/composition.md (§3), specs/client/bridge.md (§10 rules 4–5), specs/client/msg-ui.md (§16 r4.3, open question 10)
//! Bevy edge of the world view: after the bridge frame (`PreUpdate`,
//! `bridge.md` §8), one `Update` system runs UI → [`super::build_frame`]
//! (the frame's camera from the [`super::ViewFeed`], then the original's
//! view rules, `rules::OriginalView`) → compose → the 800×600 image shown
//! by a sprite, scaled by the integer presentation factor (§A9, outside
//! the verify boundary). Time base (camera §9): one frame per presented
//! server tick, from the model as it stands after that tick; a Bevy frame
//! whose bridge frame ran no tick draws nothing and keeps the presented
//! image (no interpolation). Compose is the
//! GPU compute compositor when [`WorldViewGpu`] exists (the frame is
//! packed here and composed by the render-graph node, [`super::node`],
//! straight into the image's texture), else the CPU reference written
//! into the image.
//!
//! Each composed frame is one frame of the 1.14d frame cycle
//! (`composition.md` §3): [`WorldViewState::cycle`] holds the index
//! framebuffer between frames and the clears come from
//! `cycle.plan(blank_screen)` with the feed's BlankScreen. CPU:
//! `cycle.compose`. GPU: the frame is packed onto `cycle.pixels()`, the
//! node composes it and reads the indices back ([`NodeIndices`]), and the
//! next frame is built only after they are committed to the cycle (a Bevy
//! frame that would build before that waits, like one whose bridge frame
//! ran no tick).
//!
//! The bridge frame's UI and sound outputs (`bridge.md` §10) are applied
//! by [`deliver_outputs`] right after the bridge frame, before the input
//! and UI systems: UI outputs by the original UI, sound outputs (and the
//! sounds the UI makes for them) appended to [`UiSounds`] in list order.
//!
//! Inert until the app inserts [`crate::bridge::BridgeResource`] and
//! [`WorldViewState`]: nothing here builds a server or chooses rules.
//! Errors go to Bevy's error handler; there is no fallback from a failed
//! GPU frame to the CPU one (METHODS M07).

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::render::view::Msaa;
use bevy::window::PrimaryWindow;
use std::sync::Arc;

use crate::audio::driver::SoundRequest;
use crate::bridge::link::ServerLink;
use crate::bridge::mirror::{bridge_frame, mirror_units};
use crate::bridge::output::{dispatch, Output};
use crate::bridge::{Bridge, BridgeError, BridgeResource, FrameOutputs};
use crate::controls::Bindings;
use crate::frames::atlas::AtlasPage;
use crate::ui::original::OriginalUi;
use crate::ui::{edge, FramePos, PointerButton, StringLookup, UiEvent, UiRoot};

use super::feed::{build_frame, ViewFeed};
use super::node::{add_node, ComposeJob, NodeIndices};
use super::panel_art::PanelArtLoader;
use super::ui_bind::{run_ui_with, TextAssetLoader, UiQueue, UiRules};
use super::walk::PreviewWalk;
use super::{compose_cycle_cpu, GpuAtlas, ViewAssets, ViewRules, VIEW};
use crate::scene::{FrameCycle, FramePlan};

/// Render layer of the presented frame and its camera, so the world view
/// never mixes with other sprites of the app.
pub const PRESENT_LAYER: usize = 31;

/// Atlas pages the in-app GPU path may use (64 × 4 MiB).
pub const GPU_ATLAS_PAGES: u32 = 64;

/// Rules of both halves, as one object.
pub trait WorldRules: ViewRules + UiRules {}
impl<T: ViewRules + UiRules + ?Sized> WorldRules for T {}

/// The world view's inputs besides the bridge: the assets, the hooks of
/// other owner specs (`rules`: pose, component frames, draw keys,
/// shading, blend, UI) and the camera feed (`feed`: player, open mode,
/// shake, unit positions, map tiles). Placement is the original's
/// (`rules::OriginalView`), not a hook.
#[derive(Resource)]
pub struct WorldViewState {
    pub assets: ViewAssets,
    pub rules: Box<dyn WorldRules + Send + Sync>,
    pub feed: Box<dyn ViewFeed + Send + Sync>,
    /// The persistent index framebuffer (`composition.md` §3), `VIEW`
    /// sized: the last presented frame once committed.
    pub cycle: FrameCycle,
    /// Counts of the last frame, for logs and tests.
    pub last: Option<FrameStats>,
    /// The world-click globals (`ui/controls.md` §6).
    pub click: crate::controls::click::ClickState,
    /// The game's automap (`ui/automap.md`), when the app supplied its
    /// tables; `None`: no automap.
    pub automap: Option<crate::ui::automap::session::AutomapSession>,
    /// The play preview (decision D1, [`super::preview`]): a frame whose
    /// build fails is logged (each message once) and not presented,
    /// instead of failing the app. `false`: strict (M07).
    pub preview: bool,
    /// The last logged preview frame error.
    preview_error: Option<String>,
    /// The last drawn frame's item tags (tiles, units, UI) and UI draws,
    /// in draw order: what the frame shows, for logs and tests.
    pub last_tags: Vec<crate::scene::ItemTag>,
    pub last_ui: Vec<crate::ui::UiDraw>,
    /// The preview's pending interaction (`super::interact`).
    pub interact: super::interact::PreviewInteract,
    /// Ground items (`super::ground_items`): the app hands in the item art
    /// rows and the archives; the default draws nothing.
    pub ground_items: super::ground_items::GroundItems,
}

impl WorldViewState {
    pub fn new(
        assets: ViewAssets,
        rules: Box<dyn WorldRules + Send + Sync>,
        feed: Box<dyn ViewFeed + Send + Sync>,
    ) -> Self {
        WorldViewState {
            assets,
            rules,
            feed,
            cycle: FrameCycle::new(VIEW.width, VIEW.height)
                .expect("VIEW is taller than the uncleared band"),
            last: None,
            click: Default::default(),
            automap: None,
            preview: false,
            preview_error: None,
            last_tags: Vec::new(),
            last_ui: Vec::new(),
            interact: Default::default(),
            ground_items: Default::default(),
        }
    }
}

/// What the last presented frame held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameStats {
    pub bridge_frame: u64,
    /// The server tick the frame shows (`ClientWorld::server_ticks`).
    pub server_tick: u64,
    pub items: usize,
    pub units_drawn: usize,
    pub units_hidden: usize,
    pub ui_sent: usize,
    pub ui_unhandled: usize,
    pub gpu: bool,
}

/// The UI core (non-send: panels are plain trait objects). Optional: with
/// none inserted the frame has no UI items.
pub struct WorldViewUi {
    pub root: UiRoot,
    pub strings: Box<dyn StringLookup>,
    pub queue: UiQueue,
    /// The original UI (`ui/panels.md`) whose panels are in `root`: it
    /// applies their outputs, and its open mode is the feed's
    /// ([`ViewFeed::set_ui_open_mode`]).
    pub original: Option<OriginalUi>,
    /// Key bindings: pressed keys become [`UiEvent::Action`]s (§A4, §A6).
    pub bindings: Option<Bindings>,
    /// Makes the panel DC6 files of the frame's UI draws resident.
    pub art: Option<PanelArtLoader>,
    /// Makes the UI text fonts (`.tbl` and glyph DC6) of the frame's text
    /// draws resident (`ui_bind::TextAssetLoader`).
    pub text: Option<TextAssetLoader>,
    /// Last cursor position sent, so moves are reported once.
    cursor: Option<FramePos>,
}

impl WorldViewUi {
    pub fn new(root: UiRoot, strings: Box<dyn StringLookup>) -> Self {
        WorldViewUi {
            root,
            strings,
            queue: UiQueue::default(),
            original: None,
            bindings: None,
            art: None,
            text: None,
            cursor: None,
        }
    }
}

/// Sound requests for the audio frame, in order: the bridge outputs'
/// (S→C 0x2C server sounds, the sounds of the UI dispatch of 0x5D /
/// 0x77; `client/bridge.md` §10) and the UI's own (`audio/triggers.md`
/// §11). The output dispatcher and the world view append; the audio side
/// drains.
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct UiSounds(pub Vec<SoundRequest>);

/// The GPU path's main-world half: the atlas of the frame store, and the
/// pages last handed to the node (replaced only when frames were added). The compute compositor itself runs in the render
/// world ([`super::node`]).
#[derive(Resource)]
pub struct WorldViewGpu {
    pub atlas: GpuAtlas,
    pages: Arc<Vec<AtlasPage>>,
    /// Frames in `pages` (frames are only added: the pages' version).
    held: usize,
    /// Jobs handed to the node.
    seq: u64,
    /// The job whose indices are not committed to the cycle yet, and its
    /// plan.
    pending: Option<(u64, FramePlan)>,
}

impl WorldViewGpu {
    fn new() -> std::result::Result<Self, super::ViewError> {
        Ok(WorldViewGpu {
            atlas: GpuAtlas::new(GPU_ATLAS_PAGES)?,
            pages: Arc::new(Vec::new()),
            held: 0,
            seq: 0,
            pending: None,
        })
    }
}

/// The presented image and its sprite.
#[derive(Resource)]
pub struct WorldViewTarget {
    pub image: Handle<Image>,
    pub sprite: Entity,
}

#[derive(Component)]
struct WorldViewSprite;

/// Adds the world view systems. `gpu`: add the render-graph node and
/// create [`WorldViewGpu`] once a [`WorldViewState`] exists (no render
/// world, no GPU path: the CPU reference is presented). Add it after
/// Bevy's render plugin.
pub struct WorldViewPlugin {
    pub gpu: bool,
}

impl Default for WorldViewPlugin {
    fn default() -> Self {
        WorldViewPlugin { gpu: true }
    }
}

#[derive(Resource)]
struct GpuWanted(bool);

impl Plugin for WorldViewPlugin {
    fn build(&self, app: &mut App) {
        let node = self.gpu && add_node(app);
        app.insert_resource(GpuWanted(node))
            .init_resource::<UiSounds>()
            .add_systems(
                PreUpdate,
                deliver_outputs
                    .after(bridge_frame)
                    .before(mirror_units)
                    .run_if(resource_exists::<BridgeResource>),
            )
            .add_systems(
                Update,
                (init_gpu, ui_input, world_view_frame, present_scale)
                    .chain()
                    .run_if(resource_exists::<BridgeResource>)
                    .run_if(resource_exists::<WorldViewState>),
            );
    }
}

/// What the output dispatcher could not apply.
#[derive(Debug, thiserror::Error)]
pub enum DeliverError {
    #[error(transparent)]
    Ui(#[from] crate::ui::original::OriginalUiError),
    /// The bridge could not send the answer of 0x28's dialog branch.
    #[error(transparent)]
    Bridge(#[from] BridgeError),
}

/// The output dispatcher system (`client/bridge.md` §10 rules 4–5): the
/// last bridge frame's outputs through [`deliver`]; the sound requests
/// are appended to [`UiSounds`]. Without the original UI the UI outputs
/// have no consumer and are dropped with a log line.
pub fn deliver_outputs(
    mut bridge: ResMut<BridgeResource>,
    outputs: Option<ResMut<FrameOutputs>>,
    ui: Option<NonSendMut<WorldViewUi>>,
    sounds: Option<ResMut<UiSounds>>,
) -> Result {
    let Some(mut outputs) = outputs else {
        return Ok(());
    };
    let list = std::mem::take(&mut outputs.0);
    if list.is_empty() {
        return Ok(());
    }
    let original = ui.map(|u| u.into_inner()).and_then(|u| u.original.as_mut());
    let requests = deliver(&mut bridge.0, &list, original)?;
    if let Some(mut s) = sounds {
        s.0.extend(requests);
    }
    Ok(())
}

/// Applies one bridge frame's outputs in list order (`client/bridge.md`
/// §10 rules 4–5): UI outputs to the original UI (its sounds appended at
/// once, so the request order is the call order), sound outputs as
/// requests, effect outputs to the (not yet written) effect layer.
/// Returns the sound requests in order.
///
/// When the UI chose the case of 0x28's dialog branch
/// ([`OriginalUi::take_dialog_answer`]), the bridge applies it at once,
/// before the next output (`client/msg-ui.md` §16 r4.3, open question 10
/// decided as A): its model writes, then C→S 0x31 in the slot after
/// 0x28's 0x2F and the sends that waited behind it, in the same frame as
/// the 0x28 (an unanswered slot is dropped at the next bridge frame).
pub fn deliver<L: ServerLink>(
    bridge: &mut Bridge<L>,
    list: &[Output],
    mut original: Option<&mut OriginalUi>,
) -> Result<Vec<SoundRequest>, DeliverError> {
    let requests = std::cell::RefCell::new(Vec::new());
    let bridge = std::cell::RefCell::new(bridge);
    dispatch::<DeliverError>(
        list,
        &mut |o| {
            match original.as_deref_mut() {
                Some(ui) => {
                    ui.apply_output(o, bridge.borrow().world())?;
                    requests.borrow_mut().extend(ui.take_sounds());
                    for s in ui.take_skipped() {
                        debug!("ui output {o:?}: skipped {s}");
                    }
                    if let Some((d, case)) = ui.take_dialog_answer() {
                        bridge.borrow_mut().npc_dialog_branch(&d, case)?;
                    }
                }
                None => debug!("ui output {o:?}: no original UI"),
            }
            Ok(())
        },
        &mut |o| {
            match audio_request(o) {
                Some(r) => requests.borrow_mut().push(r),
                None => debug!("audio output {o:?}: no consumer yet"),
            }
            Ok(())
        },
        &mut |o| {
            // The client effect layer is Phase 6 (`client/bridge.md` §10
            // rule 5): no consumer yet.
            debug!("effects output {o:?}: no effect layer");
            Ok(())
        },
    )?;
    Ok(requests.into_inner())
}

/// The automap's frame facts (`ui/automap.md` §9): the d2rs frame, the
/// open mode, the unit origin of the local player's camera.
fn automap_facts(
    world: &crate::bridge::world::ClientWorld,
    open_mode: u8,
) -> crate::ui::automap::FrameFacts {
    use crate::rules::camera::{moving_to_client, Camera, FrameSize, OpenMode};
    let at = world.local().map_or(Default::default(), |p| {
        let (x, y) = p.cell();
        moving_to_client((u32::from(x) << 16) | 0x8000, (u32::from(y) << 16) | 0x8000)
    });
    let mode = OpenMode::new(open_mode).unwrap_or(OpenMode::NONE);
    let cam = Camera::new(FrameSize::D2RS, mode, at, (0, 0));
    crate::ui::automap::FrameFacts {
        width: FrameSize::D2RS.width,
        height: FrameSize::D2RS.height,
        open_mode,
        mini_down: false,
        unit_origin: cam.unit,
    }
}

/// The sound request of an audio output (`client/bridge.md` §10 rule 5),
/// in list order: S→C 0x2C events, the client object code's calls
/// (`world/objects-client.md` §28 r3: mode sounds, requests, player
/// event sounds) and the 0x4D shrine sound (`client/model.md` §15 rule
/// 4 step 4: request(id, P)). `None`: no sound consumer for it.
pub fn audio_request(o: &Output) -> Option<SoundRequest> {
    use crate::bridge::objects::ObjSound;
    Some(match o {
        &Output::ServerSound { unit, class, event } => SoundRequest::Server { unit, class, event },
        Output::ObjectSound(ObjSound::Mode {
            unit,
            class,
            mode,
            local_dist,
        }) => SoundRequest::ObjectMode {
            unit: unit.key,
            client_only: unit.client_only,
            class: *class,
            mode: *mode,
            local_dist: *local_dist,
        },
        Output::ObjectSound(ObjSound::Request { id, unit }) => SoundRequest::UnitRequest {
            id: *id,
            unit: unit.key,
        },
        &Output::ObjectSound(ObjSound::PlayerEvent { player, event }) => {
            SoundRequest::PlayerEvent {
                unit: player,
                event: u16::from(event),
            }
        }
        &Output::ShrineSound { sound, player } => SoundRequest::UnitRequest {
            id: sound as i32,
            unit: player,
        },
        _ => return None,
    })
}

/// Creates the GPU path once, when the node exists.
fn init_gpu(mut commands: Commands, wanted: Res<GpuWanted>, mut tried: Local<bool>) -> Result {
    if *tried || !wanted.0 {
        return Ok(());
    }
    *tried = true;
    commands.insert_resource(WorldViewGpu::new()?);
    Ok(())
}

/// Window input → UI events (§A4): cursor in frame coordinates, pointer
/// buttons routed as-is (their meanings: `ui/controls.md` §7 r5,
/// [`crate::ui::PointerButton`]).
/// Keyboard actions come from the controls layer (C9), not wired here.
fn ui_input(
    ui: Option<NonSendMut<WorldViewUi>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    walk: Option<ResMut<PreviewWalk>>,
) -> Result {
    let (Some(mut ui), Ok(window)) = (ui, windows.single()) else {
        return Ok(());
    };
    // d2rs-own, unverified (D2): Stand Still (command 36) is held while
    // a key bound to it is down (`ui/controls.md` §4.3 r1).
    if let (Some(mut walk), Some(bindings), Some(keys)) = (walk, &ui.bindings, keys.as_deref()) {
        let held = bindings
            .inputs(crate::controls::Action::StandStill)
            .iter()
            .any(|k| {
                edge::KEY_CODES
                    .iter()
                    .any(|&(c, key)| key == *k && keys.pressed(c))
            });
        if walk.run.stand_still != held {
            walk.run.stand_still = held;
        }
    }
    // Keys in `KEY_CODES` order, so one frame's actions are ordered the
    // same on every run.
    if let (Some(bindings), Some(keys)) = (&ui.bindings, keys.as_deref()) {
        let pressed: Vec<KeyCode> = edge::KEY_CODES
            .iter()
            .map(|&(c, _)| c)
            .filter(|&c| keys.just_pressed(c))
            .collect();
        let actions = edge::key_actions(bindings, &pressed);
        ui.queue.0.extend(actions);
    }
    // A window below 800×600 has no frame mapping (`ui.md` open question
    // 1 of the C8 notes): pointer input is dropped as outside the frame.
    let pos = edge::cursor_frame_pos(window).unwrap_or(FramePos::Outside);
    if ui.cursor != Some(pos) {
        let e = match pos {
            FramePos::Inside(p) => UiEvent::CursorMoved(p),
            FramePos::Outside => UiEvent::CursorLeft,
        };
        ui.queue.0.push(e);
        ui.cursor = Some(pos);
    }
    let FramePos::Inside(at) = pos else {
        return Ok(());
    };
    for (bevy, button) in [
        (MouseButton::Left, PointerButton::Left),
        (MouseButton::Right, PointerButton::Right),
        (MouseButton::Middle, PointerButton::Middle),
    ] {
        if buttons.just_pressed(bevy) {
            ui.queue.0.push(UiEvent::Press { button, at });
        }
        if buttons.just_released(bevy) {
            ui.queue.0.push(UiEvent::Release { button, at });
        }
    }
    Ok(())
}

fn rgba_image(rgba: Vec<u8>) -> Image {
    let mut image = Image::new(
        Extent3d {
            width: VIEW.width,
            height: VIEW.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    image
}

/// UI frame, draw list, composition, image update: once per presented
/// server tick (camera §9). Before the first tick, and on Bevy frames
/// whose bridge frame ran no tick, nothing is drawn and pending UI input
/// waits for the next drawn frame.
#[allow(clippy::too_many_arguments)]
fn world_view_frame(
    mut commands: Commands,
    mut bridge: ResMut<BridgeResource>,
    mut state: ResMut<WorldViewState>,
    ui: Option<NonSendMut<WorldViewUi>>,
    mut gpu: Option<ResMut<WorldViewGpu>>,
    indices: Option<Res<NodeIndices>>,
    target: Option<Res<WorldViewTarget>>,
    mut images: ResMut<Assets<Image>>,
    mut sounds: Option<ResMut<UiSounds>>,
    mut walk: Option<ResMut<PreviewWalk>>,
) -> Result {
    let tick = bridge.0.world().server_ticks;
    if tick == 0 || state.last.is_some_and(|l| l.server_tick == tick) {
        return Ok(());
    }
    // The previous GPU frame is the base of this one: commit its indices
    // first, or wait for them.
    if let Some(g) = gpu.as_deref_mut() {
        if let Some((seq, plan)) = g.pending {
            let Some(back) = indices.as_ref().and_then(|i| i.take(seq)) else {
                return Ok(());
            };
            state.cycle.commit(plan, back)?;
            g.pending = None;
        }
    }
    let state = &mut *state;
    let ui_frame = match ui {
        Some(mut ui) => {
            let ui = &mut *ui;
            let frame = run_ui_with(
                &mut ui.root,
                &mut ui.queue,
                &mut bridge.0,
                ui.strings.as_ref(),
                ui.original.as_mut(),
            )?;
            if let Some(original) = ui.original.as_mut() {
                let outcome = original.take_outcome();
                for e in &outcome.effects {
                    debug!("ui: {e:?}");
                }
                if let Some(s) = sounds.as_deref_mut() {
                    s.0.extend(outcome.sounds);
                }
                state.feed.set_ui_open_mode(original.open_mode());
            }
            if let Some(art) = &ui.art {
                art.ensure(&frame.draws, &mut state.assets)?;
            }
            if let Some(text) = &ui.text {
                text.ensure(&frame.draws, &mut state.assets)?;
            }
            let mouse = match ui.cursor {
                Some(FramePos::Inside(p)) => (p.x, p.y),
                _ => (0, 0),
            };
            let view = crate::bridge::click::ClickView {
                size: crate::rules::camera::FrameSize::D2RS,
                open_mode: ui.original.as_ref().map_or(0, |o| o.open_mode().get()),
                // `[0x007A521C]` = H − 40 (`ui/automap.md` §9).
                right_panel_bottom: crate::rules::camera::FrameSize::D2RS.play_height(),
                // PROVISIONAL (ui/controls.md §6 r7; controls-0001):
                // `0x00454970()` is not specified: the play area H − 40.
                skill_y_limit: crate::rules::camera::FrameSize::D2RS.play_height(),
                mouse,
                game_menu_open: false,
                // d2rs-own, unverified (D1): the preview's hover pick.
                pick: state.preview,
            };
            // d2rs-own, unverified (D2): the run lock (command 35) is the
            // toggle action no panel took; the click reads the predicted
            // position and the modifier word.
            let (mods, local_at) = match walk.as_deref_mut() {
                Some(w) => {
                    let toggle = crate::controls::Action::ToggleRun.index() as u16;
                    for e in &frame.unhandled {
                        if *e == UiEvent::Action(crate::ui::ActionId(toggle)) {
                            w.run.toggle_run();
                        }
                    }
                    // The run button (control panel §10 r2) toggles it too.
                    if let Some(o) = ui.original.as_mut() {
                        for _ in 0..o.sync_run(w.run.run_lock) {
                            w.run.toggle_run();
                        }
                        o.sync_run(w.run.run_lock);
                    }
                    (w.run.word(), w.predict.position())
                }
                None => (0, None),
            };
            let unhandled = state
                .ground_items
                .take_clicks(&mut bridge.0, &frame.unhandled)?;
            crate::bridge::belt::send_keys(&mut bridge.0, &frame.unhandled)?;
            let outs = world_clicks(
                &mut bridge.0,
                &mut state.click,
                view,
                &unhandled,
                mods,
                local_at,
            )?;
            for o in &outs {
                debug!("world click: {o:?}");
            }
            if let Some(w) = walk.as_deref() {
                // d2rs-own, unverified (D1, D2): the pending interaction.
                let pressed = frame
                    .unhandled
                    .iter()
                    .any(|e| matches!(e, UiEvent::Press { .. }));
                state.interact.note(&outs, pressed);
                let walking = w.predict.walking().is_some();
                for o in state.interact.frame(&mut bridge.0, walking)? {
                    debug!("interact: {o:?}");
                }
            }
            // `ui/automap.md` §8 r2: the toggle command no panel took.
            let toggle = crate::controls::Action::ToggleAutomap.index() as u16;
            if let Some(a) = state.automap.as_mut() {
                for e in &frame.unhandled {
                    if *e == UiEvent::Action(crate::ui::ActionId(toggle)) {
                        a.toggle(&automap_facts(bridge.0.world(), view.open_mode));
                    }
                }
            }
            Some(frame)
        }
        None => None,
    };
    let draws = ui_frame.as_ref().map_or(&[][..], |f| &f.draws[..]);
    state.feed.prepare(bridge.0.world(), &mut state.assets)?;
    let built = build_frame(
        bridge.0.world(),
        draws,
        state.rules.as_ref(),
        state.feed.as_mut(),
        &state.assets,
    );
    let mut frame = match built {
        Ok(f) => f,
        // d2rs-own, unverified (D1): the preview keeps running; the
        // frame is not presented.
        Err(e) if state.preview => {
            let message = e.to_string();
            if state.preview_error.as_deref() != Some(message.as_str()) {
                warn!("preview (d2rs-own, unverified): frame not drawn: {message}");
                state.preview_error = Some(message);
            }
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    };
    for m in state.ground_items.add_to_frame(
        bridge.0.world(),
        state.feed.as_ref(),
        &mut state.assets,
        &mut frame,
    ) {
        warn!("preview (d2rs-own, unverified): {m}");
    }
    // `ui/automap.md` §5 r1: the reveal of this frame, after the draw
    // marked its records (from the frame `0x0044C7EB`).
    if let Some(a) = state.automap.as_mut() {
        let world = bridge.0.world();
        let near = state.feed.near_rooms(world)?;
        a.frame(world, near)?;
    }
    // `sim/unit-order.md` §5 rule 7: the fill's Y sort persists in the
    // client's room lists.
    for (room, order) in state.feed.take_unit_orders() {
        bridge.0.set_room_order(room, &order);
    }
    let blank_screen = state.feed.blank_screen(bridge.0.world())?;
    let use_gpu = gpu.is_some();
    let bridge_frame = bridge.0.world().frames;
    state.last_tags = frame.items.iter().map(|i| i.tag).collect();
    state.last_ui = draws.to_vec();
    state.last = Some(FrameStats {
        bridge_frame,
        server_tick: tick,
        items: frame.items.len(),
        units_drawn: frame.units_drawn,
        units_hidden: frame.units_hidden,
        ui_sent: ui_frame.as_ref().map_or(0, |f| f.sent),
        ui_unhandled: ui_frame.as_ref().map_or(0, |f| f.unhandled.len()),
        gpu: use_gpu,
    });
    let image = match &target {
        Some(t) => t.image.clone(),
        None => {
            // The GPU path writes the texture itself: a target texture
            // (zeros) that the main world never rewrites.
            let mut image = if use_gpu {
                {
                    let mut image = Image::new_target_texture(
                        VIEW.width,
                        VIEW.height,
                        TextureFormat::Rgba8UnormSrgb,
                        None,
                    );
                    // Copy source too: tests read the presented frame back.
                    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
                    image
                }
            } else {
                rgba_image(vec![0; (VIEW.width * VIEW.height * 4) as usize])
            };
            image.sampler = ImageSampler::nearest();
            let image = images.add(image);
            let layer = RenderLayers::layer(PRESENT_LAYER);
            commands.spawn((
                Camera2d,
                Camera {
                    order: 1,
                    clear_color: ClearColorConfig::Custom(Color::srgb_u8(0, 0, 0)),
                    ..default()
                },
                Msaa::Off,
                Tonemapping::None,
                layer.clone(),
            ));
            let sprite = commands
                .spawn((Sprite::from_image(image.clone()), WorldViewSprite, layer))
                .id();
            commands.insert_resource(WorldViewTarget {
                image: image.clone(),
                sprite,
            });
            image
        }
    };
    match gpu {
        Some(mut g) => {
            let g = &mut *g;
            g.atlas.ensure(&state.assets.frames)?;
            if g.atlas.frames() != g.held {
                g.held = g.atlas.frames();
                g.pages = Arc::new(g.atlas.atlas().pages().to_vec());
            }
            let (packed, plan) =
                g.atlas
                    .pack_cycle(&state.cycle, blank_screen, &frame, &state.assets)?;
            g.seq += 1;
            g.pending = Some((g.seq, plan));
            commands.insert_resource(ComposeJob {
                seq: g.seq,
                frame: bridge_frame,
                packed: Arc::new(packed),
                pages: g.pages.clone(),
                pages_version: g.held as u64,
                palette: Arc::new(state.assets.palette.clone()),
                target: image,
            });
        }
        None => {
            let rgba = compose_cycle_cpu(&mut state.cycle, blank_screen, &frame, &state.assets)?;
            let mut image = images
                .get_mut(&image)
                .ok_or("world view image asset is gone")?;
            image.data = Some(rgba);
        }
    }
    Ok(())
}

/// Integer presentation scale (§A9; same factor as `ui::Presentation`, so
/// the cursor mapping matches), converted to logical units for Bevy.
fn present_scale(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut sprites: Query<&mut Transform, With<WorldViewSprite>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok(p) = edge::presentation(window) else {
        return;
    };
    let s = p.scale as f32 / window.scale_factor();
    for mut t in &mut sprites {
        t.scale = Vec3::new(s, s, 1.0);
    }
}
