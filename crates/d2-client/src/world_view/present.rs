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
use bevy::ecs::message::{MessageCursor, Messages};
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
use crate::bridge::world::UnitKey;
use crate::bridge::{Bridge, BridgeError, BridgeResource, FrameOutputs};
use crate::controls::click::ClickOut;
use crate::controls::Bindings;
use crate::frames::atlas::AtlasPage;
use crate::ui::original::OriginalUi;
use crate::ui::{edge, FramePos, PointerButton, StringLookup, UiEvent, UiRoot};

use super::feed::{build_frame_placed, ViewFeed};
use super::node::{add_node, ComposeJob, NodeIndices};
use super::panel_art::PanelArtLoader;
use super::ui_bind::{run_ui_with, world_clicks, TextAssetLoader, UiQueue, UiRules};
use super::walk::PreviewWalk;
use super::{compose_cycle_cpu, play_view, GpuAtlas, ViewAssets, ViewRules};
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
    /// The persistent index framebuffer (`composition.md` §3), [`play_view`]
    /// sized: the last presented frame once committed.
    pub cycle: FrameCycle,
    /// Counts of the last frame, for logs and tests.
    pub last: Option<FrameStats>,
    /// The world-click globals (`ui/controls.md` §6).
    pub click: crate::controls::click::ClickState,
    /// The game's automap (`ui/automap.md`), when the app supplied its
    /// tables; `None`: no automap.
    pub automap: Option<crate::ui::automap::session::AutomapSession>,
    /// The automap's draw sink (`super::automap_view`); `None`: not drawn.
    pub automap_view: Option<super::automap_view::AutomapView>,
    /// The level backgrounds of pass 1 (`super::background_view`,
    /// `draw-order-2.md` §12); `None`: not drawn.
    pub background_view: Option<super::background_view::BackgroundView>,
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
    /// The hover target the previous pass left (`ClickView::prev_hover`).
    pub prev_hover: Option<crate::bridge::world::UnitKey>,
    /// The preview's pending interaction (`super::interact`).
    pub interact: super::interact::PreviewInteract,
    /// Ground items (`super::ground_items`): the app hands in the item art
    /// rows and the archives; the default draws nothing.
    pub ground_items: super::ground_items::GroundItems,
    /// The click on a corpse (`super::corpse_click`).
    pub corpse_clicks: super::corpse_click::CorpseClicks,
    /// Client missiles and cast / state overlays (`super::missiles`).
    pub missiles: super::missiles::Missiles,
    /// The object mouse-over label (`super::object_label`).
    pub object_labels: super::object_label::ObjectLabels,
    /// The last drawn frame's camera (`super::visibility`).
    pub camera: super::visibility::SharedCamera,
    /// The last drawn tick's local-player position and shake
    /// (`seams/world-screen.md` §2.2, §2.4): decided once per drawn tick,
    /// read by the UI, the pick and the labels on every loop pass until
    /// the next tick is drawn.
    pub anchor: Option<crate::rules::camera::FrameAnchor>,
    /// The model's act loads already handed to the cycle
    /// ([`note_act_loads`]).
    act_loads: u64,
}

/// `composition.md` §3 step 4: each S→C 0x03 the model handled since the
/// last call sets the post-draw clear counter to 1 (`0x0044E100`), so the
/// next presented frame is all index 0.
pub fn note_act_loads(cycle: &mut FrameCycle, seen: &mut u64, loads: u64) {
    if loads != *seen {
        *seen = loads;
        cycle.set_post_clear(1);
    }
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
            cycle: FrameCycle::new(play_view().width, play_view().height)
                .expect("the play frame is taller than the uncleared band"),
            last: None,
            click: Default::default(),
            automap: None,
            automap_view: None,
            background_view: None,
            preview: false,
            preview_error: None,
            last_tags: Vec::new(),
            last_ui: Vec::new(),
            prev_hover: None,
            interact: Default::default(),
            ground_items: Default::default(),
            corpse_clicks: Default::default(),
            missiles: Default::default(),
            object_labels: Default::default(),
            camera: Default::default(),
            anchor: None,
            act_loads: 0,
        }
    }
}

/// A check run's recorded frame schedule (`play --frame-schedule`,
/// `specs/tools/scenario-diff.md` §3 r7.5): the client updates on which
/// 1.14d drew a frame, each with the host clock (`GetTickCount()`) of the
/// frame's cursor step, and the cursor timers at the first frame. With a
/// schedule the world view draws exactly those ticks (one frame each) and
/// nothing between them; the host clock and the frame schedule are
/// recorded inputs, not game behaviour.
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct FrameSchedule {
    /// Drawn tick → the clock of its cursor step (`None`: not recorded,
    /// allowed for the last frame only: its step follows the dump).
    pub frames: std::collections::BTreeMap<u64, Option<u32>>,
    /// Drawn tick → the recorded light quality `[0x007B567C]` (a host
    /// input: it follows the measured draw rate, `render/lighting.md` §5),
    /// where the schedule has the column.
    pub quality: std::collections::BTreeMap<u64, u8>,
    /// Drawn tick → the pointer the frame started with (frame pixels): where
    /// the OS put it after a panel's cursor jump (`ui/panels-3.md` §4.3).
    pub pointer: std::collections::BTreeMap<u64, (i32, i32)>,
    /// `last_step` and `idle_since` of the cursor at the first frame.
    pub cursor_last: u32,
    pub cursor_idle: u32,
    /// The schedule was applied (cursor timers, weather replay off).
    pub started: bool,
}

impl FrameSchedule {
    /// Parses the `frame-schedule 1` text: `# frame-schedule 1`, `#
    /// cursor_last N`, `# cursor_idle N`, a `tick\tnow` header, then one
    /// row per drawn frame (`now` `-` when not recorded).
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut s = FrameSchedule::default();
        let (mut version, mut last, mut idle) = (false, None, None);
        for (n, line) in text.lines().enumerate() {
            let at = |m: &str| format!("frame schedule line {}: {m}", n + 1);
            if let Some(h) = line.strip_prefix('#') {
                let mut w = h.split_whitespace();
                match (w.next(), w.next()) {
                    (Some("frame-schedule"), Some("1")) => version = true,
                    (Some("cursor_last"), Some(v)) => {
                        last = Some(v.parse().map_err(|_| at("cursor_last"))?)
                    }
                    (Some("cursor_idle"), Some(v)) => {
                        idle = Some(v.parse().map_err(|_| at("cursor_idle"))?)
                    }
                    _ => {}
                }
                continue;
            }
            if line.trim().is_empty() || line.starts_with("tick") {
                continue;
            }
            let mut c = line.split('\t');
            let tick: u64 = c
                .next()
                .and_then(|v| v.parse().ok())
                .ok_or_else(|| at("tick"))?;
            let now = match c.next() {
                Some("-") => None,
                Some(v) => Some(v.parse().map_err(|_| at("now"))?),
                None => return Err(at("no now column")),
            };
            s.frames.insert(tick, now);
            if let Some(q) = c.next().filter(|q| *q != "-") {
                s.quality
                    .insert(tick, q.parse().map_err(|_| at("quality"))?);
            }
            if let (Some(x), Some(y)) = (c.next(), c.next()) {
                if x != "-" && y != "-" {
                    let x = x.parse().map_err(|_| at("cursor_x"))?;
                    let y = y.parse().map_err(|_| at("cursor_y"))?;
                    s.pointer.insert(tick, (x, y));
                }
            }
        }
        if !version {
            return Err("frame schedule: no `# frame-schedule 1` line".into());
        }
        s.cursor_last = last.ok_or("frame schedule: no cursor_last")?;
        s.cursor_idle = idle.ok_or("frame schedule: no cursor_idle")?;
        let last_tick = s.frames.keys().next_back().copied();
        if let Some(t) = s
            .frames
            .iter()
            .find(|(t, now)| now.is_none() && Some(**t) != last_tick)
            .map(|(t, _)| *t)
        {
            return Err(format!(
                "frame schedule: drawn tick {t} has no recorded host clock (only the last frame may lack it)"
            ));
        }
        if s.frames.is_empty() {
            return Err("frame schedule: no drawn frame".into());
        }
        Ok(s)
    }

    /// Whether 1.14d drew a frame on `tick`.
    pub fn drawn(&self, tick: u64) -> bool {
        self.frames.contains_key(&tick)
    }

    /// The host clock of `tick`'s cursor step.
    pub fn now(&self, tick: u64) -> Option<u32> {
        self.frames.get(&tick).copied().flatten()
    }
}

/// `play --dump-draws` (`specs/tools/facts-render.md` §5): the facts of
/// the first drawn frame at or after the requested server tick, then the
/// app exits. Reads the built frame only.
#[derive(Resource)]
pub struct DrawDump {
    pub request: crate::facts::export::DumpRequest,
    /// Frames drawn so far (the dump's `seq`).
    seen: u64,
    /// The next dump of `request.at_ticks` (§5 r19); all done at its length.
    next: usize,
}

impl DrawDump {
    pub fn new(request: crate::facts::export::DumpRequest) -> Self {
        DrawDump {
            request,
            seen: 0,
            next: 0,
        }
    }

    fn done(&self) -> bool {
        self.next >= self.request.at_ticks.len()
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
    /// The last cursor position inside the frame: a button released
    /// outside the frame is released there (`ui/controls.md` §6 r1).
    last_at: crate::ui::Point,
    /// The window lost the focus since the last pass (`ui/controls.md`
    /// §4.3 r3): the held world buttons are released.
    focus_lost: bool,
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
            last_at: crate::ui::Point::new(0, 0),
            focus_lost: false,
        }
    }

    /// The cursor position last reported to the UI (the click view's
    /// mouse).
    pub fn cursor(&self) -> Option<FramePos> {
        self.cursor
    }

    /// Sets the cursor position a headless driver reported with its own
    /// `CursorMoved` (`app::autoplay_host`, the window's `ui_input` path).
    pub fn set_cursor(&mut self, at: FramePos) {
        self.cursor = Some(at);
        if let FramePos::Inside(p) = at {
            self.last_at = p;
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

/// The order of the `PreUpdate` systems that take the bridge after
/// [`bridge_frame`] and before [`mirror_units`] (`flows/client-frame.md`
/// §3 r1; q-tick-flow C4): the frame's outputs first (their C→S answers,
/// 0x31, leave in the frame that made them), then the local player's
/// walk prediction, then the monster tracks (players before monsters, the
/// client update pass's order, `client/model.md` §5 r3). Pinned so the
/// order of sends within a frame is not left to Bevy's executor.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PreviewOrder {
    Outputs,
    PlayerWalk,
    MonsterWalk,
}

/// Configures [`PreviewOrder`] in `PreUpdate` (idempotent).
pub fn configure_preview_order(app: &mut App) {
    app.configure_sets(
        PreUpdate,
        (
            PreviewOrder::Outputs,
            PreviewOrder::PlayerWalk,
            PreviewOrder::MonsterWalk,
        )
            .chain()
            .after(bridge_frame)
            .before(mirror_units),
    );
}

/// The paused pass's input (`flows/client-frame.md` §1 r2, `client/bridge.md`
/// §8 r5): the bridge is paused while the original UI has state 9 (the
/// Esc menu) or 11 open.
pub fn pause_frame(mut bridge: ResMut<BridgeResource>, ui: Option<NonSend<WorldViewUi>>) {
    let paused = ui
        .as_ref()
        .and_then(|u| u.original.as_ref())
        .is_some_and(|o| o.is_open(9) || o.is_open(11));
    bridge.0.set_paused(paused);
}

impl Plugin for WorldViewPlugin {
    fn build(&self, app: &mut App) {
        let node = self.gpu && add_node(app);
        configure_preview_order(app);
        app.insert_resource(GpuWanted(node))
            .init_resource::<UiSounds>()
            .add_systems(
                PreUpdate,
                (
                    pause_frame
                        .before(bridge_frame)
                        .run_if(resource_exists::<BridgeResource>),
                    deliver_outputs
                        .in_set(PreviewOrder::Outputs)
                        .run_if(resource_exists::<BridgeResource>),
                ),
            )
            .add_systems(
                Update,
                (
                    init_gpu,
                    ui_input,
                    script_input,
                    world_view_frame,
                    present_scale,
                )
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
    state: Option<ResMut<WorldViewState>>,
) -> Result {
    let Some(mut outputs) = outputs else {
        return Ok(());
    };
    let list = std::mem::take(&mut outputs.0);
    if list.is_empty() {
        return Ok(());
    }
    let mut original = ui.map(|u| u.into_inner()).and_then(|u| u.original.as_mut());
    let preview = state.as_ref().is_some_and(|s| s.preview);
    let requests = deliver_with(&mut bridge.0, &list, original.as_deref_mut(), preview)?;
    // The UI's overlay calls (`client/msg-ui.md` §9 r3, §16 r4.1) go to
    // the effect layer, which runs them before the next update advance.
    if let (Some(ui), Some(mut s)) = (original, state) {
        let calls = ui.take_overlay_calls();
        if !calls.is_empty() {
            let tick = bridge.0.world().server_ticks;
            s.missiles.overlay_calls(tick, calls);
        }
    }
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
    original: Option<&mut OriginalUi>,
) -> Result<Vec<SoundRequest>, DeliverError> {
    deliver_with(bridge, list, original, false)
}

/// [`deliver`], with the play preview's chat close after the dialog
/// branch when `preview_chat_end` (`bridge::chat_end`; d2rs-own,
/// unverified).
pub fn deliver_with<L: ServerLink>(
    bridge: &mut Bridge<L>,
    list: &[Output],
    mut original: Option<&mut OriginalUi>,
    preview_chat_end: bool,
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
                    // S→C 0x5A: the codes whose line goes to the screen
                    // message list `0x0049E3A0`, which requests UI sound 6
                    // (`ui/messages.md` §2 r3) even for the empty line of
                    // the local player's own join (code 2, `client/msg-ui.md`
                    // §19 r3). The original UI does not build these lines
                    // yet, so the sound is requested here. PROVISIONAL
                    // (REC-1681): codes other than 0–5 and 0xD are taken
                    // as lines too.
                    if let Output::EventText { bytes, .. } = o {
                        if event_text_adds_line(bytes[1]) {
                            requests.borrow_mut().push(SoundRequest::Ui(6));
                        }
                    }
                    for s in ui.take_skipped() {
                        debug!("ui output {o:?}: skipped {s}");
                    }
                    if let Some((d, case)) = ui.take_dialog_answer() {
                        bridge.borrow_mut().npc_dialog_branch(&d, case)?;
                        // Preview: no speech or menu, so the chat closes
                        // at once (`bridge::chat_end`; d2rs-own).
                        if preview_chat_end && !ui.npc_menu_up() {
                            bridge.borrow_mut().preview_chat_end(d.guid)?;
                        }
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
/// open mode, the unit origin of the frame's one camera
/// (`seams/world-screen.md` §2.2); no camera: origin (0, 0).
/// UI state 0x0A: the automap shown (`ui/automap.md` §8 r2).
const AUTOMAP_STATE: u8 = 0x0A;

fn automap_facts(
    camera: Option<&crate::rules::camera::Camera>,
    open_mode: u8,
) -> crate::ui::automap::FrameFacts {
    use crate::rules::camera::FrameSize;
    crate::ui::automap::FrameFacts {
        width: FrameSize::play().width,
        height: FrameSize::play().height,
        open_mode,
        mini_down: false,
        unit_origin: camera.map_or(Default::default(), |c| c.unit),
    }
}

/// The sound request of an audio output (`client/bridge.md` §10 rule 5),
/// in list order: S→C 0x2C events, the client object code's calls
/// (`world/objects-client.md` §28 r3: mode sounds, requests, player
/// event sounds), the 0x4D shrine sound (`client/model.md` §15 rule
/// 4 step 4: request(id, P)) and the unit frees (`client/bridge.md` §10
/// r3.1). `None`: no sound consumer for it.
pub fn audio_request(o: &Output) -> Option<SoundRequest> {
    use crate::bridge::objects::ObjSound;
    Some(match o {
        &Output::ServerSound {
            unit,
            class,
            at,
            event,
        } => SoundRequest::Server {
            unit,
            class,
            at,
            event,
        },
        &Output::UnitFreed { unit, client_only } => SoundRequest::UnitFreed { unit, client_only },
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
        &Output::MissileSound(m) => {
            use crate::bridge::client_missiles::MissileSound as M;
            match m {
                M::Request { id, missile } => SoundRequest::UnitRequest { id, unit: missile },
                M::StopOwnerGroup { owner, id } => SoundRequest::GroupStop { unit: owner, id },
                M::DetachTravel { missile, id } => SoundRequest::GroupDetach { unit: missile, id },
            }
        }
        _ => return None,
    })
}

/// Whether a 0x5A code ends in a screen message (`client/msg-ui.md` §19
/// r3: 0–5 and 0xD build a line, 6, 8–11, 0xF–0x11 other forms; 7, 0xC,
/// 0xE and above 0x12 add none).
pub(crate) fn event_text_adds_line(code: u8) -> bool {
    matches!(code, 0..=6 | 8..=11 | 0xD | 0xF..=0x11)
}

/// The player event sounds of a world click's results (`ClickOut::Sound`,
/// `0x004CB9C0(P, event)`) as audio requests, in click order.
pub fn click_sounds(player: Option<UnitKey>, outs: &[ClickOut]) -> Vec<SoundRequest> {
    let Some(unit) = player else {
        return Vec::new();
    };
    outs.iter()
        .filter_map(|o| match *o {
            ClickOut::Sound(event) => Some(SoundRequest::PlayerEvent { unit, event }),
            _ => None,
        })
        .collect()
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
/// A button released outside the frame is still released (at the last
/// frame position), and a lost window focus is noted for the world
/// release of `ui/controls.md` §4.3 r3.
#[allow(clippy::too_many_arguments)]
fn ui_input(
    ui: Option<NonSendMut<WorldViewUi>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    walk: Option<ResMut<PreviewWalk>>,
    time: Option<Res<Time>>,
    script: Option<Res<super::input_script::InputScript>>,
    focus: Option<Res<Messages<bevy::window::WindowFocused>>>,
    mut focus_cursor: Local<MessageCursor<bevy::window::WindowFocused>>,
    wheel: Option<Res<Messages<bevy::input::mouse::MouseWheel>>>,
    mut wheel_cursor: Local<MessageCursor<bevy::input::mouse::MouseWheel>>,
    mut wheel_acc: Local<crate::controls::original::WheelAccumulator>,
) -> Result {
    let (Some(mut ui), Ok(window)) = (ui, windows.single()) else {
        return Ok(());
    };
    if let Some(m) = focus.as_deref() {
        if focus_cursor.read(m).any(|e| !e.focused) {
            ui.focus_lost = true;
        }
    }
    use crate::controls::{Action, Key};
    // An input is down: a key, or the middle / X buttons.
    let down = |k: Key| {
        let key = keys.as_deref().is_some_and(|ks| {
            edge::KEY_CODES
                .iter()
                .any(|&(c, key)| key == k && ks.pressed(c))
        });
        key || match k {
            Key::MouseMiddle => buttons.pressed(MouseButton::Middle),
            Key::Mouse4 => buttons.pressed(MouseButton::Back),
            Key::Mouse5 => buttons.pressed(MouseButton::Forward),
            _ => false,
        }
    };
    // Stand Still (command 36) and Run (command 34) are held while an
    // input bound to them is down (`ui/controls.md` §3, §4.3 r1–r2: the
    // down handler sets, the up handler clears; VK 0x10–0x12 both sides).
    // The Run down handler's walk → run switch of a walking player (mode
    // 3, C→S 0x53 / 0x54) is not modelled: the flag reaches the next
    // world click (`RunMods::word`).
    if let (Some(mut walk), Some(bindings)) = (walk, &ui.bindings) {
        let still = edge::action_held(bindings, Action::StandStill, &down);
        if walk.run.stand_still != still {
            walk.run.stand_still = still;
        }
        let run = edge::action_held(bindings, Action::Run, &down);
        if walk.run.run_held != run {
            walk.run.run_held = run;
        }
    }
    // Show Items (command 37): ui 0x0D on while held, off on the release.
    let show = ui
        .bindings
        .as_ref()
        .is_some_and(|b| edge::action_held(b, Action::ShowItems, &down));
    if let Some(o) = ui.original.as_mut() {
        o.set_show_items(show)?;
    }
    // The middle and X buttons and the wheel call their bound command
    // (`ui/controls.md` §4.2 r2–r3): no panel takes them.
    if let Some(bindings) = ui.bindings.clone() {
        let mut pointer = Vec::new();
        for (b, k) in [
            (MouseButton::Middle, Key::MouseMiddle),
            (MouseButton::Back, Key::Mouse4),
            (MouseButton::Forward, Key::Mouse5),
        ] {
            if buttons.just_pressed(b) {
                pointer.push(k);
            }
        }
        if let Some(m) = wheel.as_deref() {
            for e in wheel_cursor.read(m) {
                // Windows units: 120 per notch.
                let delta = match e.unit {
                    bevy::input::mouse::MouseScrollUnit::Line => (e.y * 120.0) as i32,
                    bevy::input::mouse::MouseScrollUnit::Pixel => e.y as i32,
                };
                pointer.extend(edge::wheel_key(&mut wheel_acc, delta));
            }
        }
        let actions = edge::input_actions(&bindings, &pointer);
        ui.queue.0.extend(actions);
    }
    // d2rs-own, unverified: Shift held, for the shift-click to the belt.
    if let (Some(o), Some(keys)) = (ui.original.as_mut(), keys.as_deref()) {
        o.set_shift(keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight));
        o.set_ctrl(keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight));
    }
    // Keys in `KEY_CODES` order, so one frame's actions are ordered the
    // same on every run.
    // The Controls screen takes the raw keys while it is open, and the
    // game's key bindings none (`ui::controls_host`).
    let mut controls_open = false;
    if let (Some(o), Some(keys)) = (ui.original.as_mut(), keys.as_deref()) {
        if o.controls_open() {
            controls_open = true;
            let now = time.as_ref().map_or(0, |t| t.elapsed().as_millis() as u64);
            for &(c, _) in edge::KEY_CODES {
                if !keys.just_pressed(c) {
                    continue;
                }
                let vk = match c {
                    KeyCode::Escape => Some(27),
                    _ => edge::key_of(c).and_then(crate::controls::keymap::key_to_vk),
                };
                if let Some(vk) = vk {
                    o.controls_key(vk, now);
                }
            }
        }
    }
    if let (Some(bindings), Some(keys), false) = (&ui.bindings, keys.as_deref(), controls_open) {
        let pressed: Vec<KeyCode> = edge::KEY_CODES
            .iter()
            .map(|&(c, _)| c)
            .filter(|&c| keys.just_pressed(c))
            .collect();
        let mode = ui.original.as_ref().map_or(1, |o| o.key_mode());
        let actions = edge::key_actions_in_mode(bindings, &pressed, mode);
        ui.queue.0.extend(actions);
        ui.queue.0.extend(edge::key_chars(&pressed));
    }
    // `play --input`: the script owns the pointer (`script_input`).
    if script.is_some() {
        return Ok(());
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
    // A press needs the cursor in the frame; a release outside it (a
    // black bar, outside the window) still ends the button, at the last
    // frame position (left up reads the current mouse anyway, §6 r1).
    let inside = match pos {
        FramePos::Inside(p) => {
            ui.last_at = p;
            true
        }
        FramePos::Outside => false,
    };
    let at = ui.last_at;
    for (bevy, button) in [
        (MouseButton::Left, PointerButton::Left),
        (MouseButton::Right, PointerButton::Right),
    ] {
        if inside && buttons.just_pressed(bevy) {
            ui.queue.0.push(UiEvent::Press { button, at });
        }
        if buttons.just_released(bevy) {
            ui.queue.0.push(UiEvent::Release { button, at });
        }
    }
    Ok(())
}

/// `play --input` (`facts-render.md` §5 r11): the script's events, once
/// per new server tick, into the UI queue in place of the window pointer.
fn script_input(
    ui: Option<NonSendMut<WorldViewUi>>,
    bridge: Res<BridgeResource>,
    script: Option<ResMut<super::input_script::InputScript>>,
    mut last: Local<u64>,
) {
    let (Some(mut ui), Some(mut script)) = (ui, script) else {
        return;
    };
    let tick = bridge.0.world().server_ticks;
    if tick == 0 || tick == *last {
        return;
    }
    *last = tick;
    // `clickunit` steps: the unit's screen point from the model camera
    // (open mode 0, no shake; d2rs-own, unverified: the drawn frame's
    // camera may follow the walk prediction).
    let world = bridge.0.world();
    let cam = super::input_script::script_camera(world, None);
    let events = script.events_with(tick, &mut |sel| match &cam {
        Some(c) => super::input_script::unit_point(world, c, sel),
        None => Err("no local player".into()),
    });
    for n in script.take_notes() {
        warn!("play --input: {n}");
    }
    if !events.is_empty() {
        ui.queue.0.extend(events);
        ui.cursor = script.cursor();
    }
    // `key` steps: the window's key press path (`ui_input`), bound
    // action first, then the typed character.
    let codes: Vec<KeyCode> = script
        .take_keys()
        .into_iter()
        .filter_map(|k| {
            edge::KEY_CODES
                .iter()
                .find(|(_, e)| *e == k)
                .map(|(c, _)| *c)
        })
        .collect();
    if codes.is_empty() {
        return;
    }
    if let Some(bindings) = &ui.bindings {
        let mode = ui.original.as_ref().map_or(1, |o| o.key_mode());
        let actions = edge::key_actions_in_mode(bindings, &codes, mode);
        ui.queue.0.extend(actions);
    }
    ui.queue.0.extend(edge::key_chars(&codes));
}

/// The world releases of a lost focus (`ui/controls.md` §4.3 r3): left
/// up (kind 2) while left is held, right up (kind 5) while right is held,
/// each unless `events` already releases that button.
pub fn focus_releases(
    st: &crate::controls::click::ClickState,
    events: &[UiEvent],
    at: crate::ui::Point,
) -> Vec<UiEvent> {
    let released = |b: PointerButton| {
        events
            .iter()
            .any(|e| matches!(e, UiEvent::Release { button, .. } if *button == b))
    };
    [
        (st.left_held, PointerButton::Left),
        (st.right_held, PointerButton::Right),
    ]
    .into_iter()
    .filter(|&(held, b)| held && !released(b))
    .map(|(_, button)| UiEvent::Release { button, at })
    .collect()
}

fn rgba_image(rgba: Vec<u8>) -> Image {
    let mut image = Image::new(
        Extent3d {
            width: play_view().width,
            height: play_view().height,
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

/// UI input and world clicks every client loop pass (`ui/controls.md` §6
/// r2, r6: the held repeat and the per-pass latch run per pass, not per
/// server tick); draw list, composition and image update once per
/// presented server tick (camera §9). Before the first tick nothing runs.
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
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut dump: Option<ResMut<DrawDump>>,
    mut drawn: Option<ResMut<crate::bridge::mirror::DrawnTick>>,
    mut exit: MessageWriter<AppExit>,
    mut schedule: Option<ResMut<FrameSchedule>>,
) -> Result {
    let tick = bridge.0.world().server_ticks;
    if tick == 0 {
        return Ok(());
    }
    // A check run's recorded frame schedule (`tools/scenario-diff.md` §3
    // r7.5): a tick 1.14d drew no frame on is a client update without a
    // frame here too: no draw, no weather update, no cursor step.
    let mut ui = ui;
    if let Some(s) = schedule.as_deref_mut() {
        if !s.started {
            s.started = true;
            state.feed.follow_frame_schedule();
            if let Some(o) = ui.as_mut().and_then(|u| u.original.as_mut()) {
                o.set_cursor_timers(s.cursor_last, s.cursor_idle);
            }
        }
        if let Some(o) = ui.as_mut().and_then(|u| u.original.as_mut()) {
            o.set_host_now(s.now(tick));
        }
        if !s.drawn(tick) {
            if let Some(d) = drawn.as_deref_mut() {
                d.0 = tick;
            }
            return Ok(());
        }
    }
    // A new server tick is drawn; the previous GPU frame is the base of
    // this one: commit its indices first, or draw on a later pass.
    let mut draw = !state.last.is_some_and(|l| l.server_tick == tick);
    if let (true, Some(g)) = (draw, gpu.as_deref_mut()) {
        if let Some((seq, plan)) = g.pending {
            match indices.as_ref().and_then(|i| i.take(seq)) {
                Some(back) => {
                    state.cycle.commit(plan, back)?;
                    g.pending = None;
                }
                None => draw = false,
            }
        }
    }
    let state = &mut *state;
    // `seams/world-screen.md` §2.2, §2.4: the local player's position and
    // the shake are decided once per drawn tick (like the draw: the shake
    // draws the player seed); the UI (overhead text), the pick, the labels,
    // the automap and the world draw all read this one, and the passes
    // between ticks keep the drawn tick's. An error here is the frame
    // build's own, reported there.
    if draw {
        state.anchor =
            super::feed::frame_anchor(bridge.0.world(), state.feed.as_mut()).unwrap_or_default();
    }
    let anchor = state.anchor;
    // The frame's one camera, once the UI has set this frame's open mode.
    let mut placed = None;
    let mut hover_mouse: Option<(i32, i32)> = None;
    let ui_frame = match ui.as_mut() {
        Some(ui) => {
            let ui = &mut **ui;
            if let Some(o) = ui.original.as_mut() {
                o.set_frame_anchor(anchor);
                o.set_palette(&state.assets.palette);
            }
            let mut frame = run_ui_with(
                &mut ui.root,
                &mut ui.queue,
                &mut bridge.0,
                ui.strings.as_ref(),
                ui.original.as_mut(),
            )?;
            if let Some(original) = ui.original.as_mut() {
                // The cursor jump of §4.3: warp the window cursor.
                let warp = original.take_cursor_warp();
                if let (Some(at), Ok(mut window)) = (warp, windows.single_mut()) {
                    if let Ok(pos) = edge::frame_to_window(&window, at) {
                        window.set_physical_cursor_position(Some(pos.as_dvec2()));
                    }
                }
                // A check run replays the pointer the OS reported after the
                // jump (the recorded host input) when d2rs's own jump agrees
                // on x; a different x is left to show in the cursor row.
                if let (Some(at), Some(s)) = (warp, schedule.as_deref()) {
                    if let Some((_, &(x, y))) = s.pointer.range(tick + 1..).next() {
                        if x == at.x {
                            let p = crate::ui::Point::new(x, y);
                            ui.queue.0.push(UiEvent::CursorMoved(p));
                            ui.cursor = Some(FramePos::Inside(p));
                        } else {
                            warn!("cursor jump: d2rs x {} vs recorded {x}", at.x);
                        }
                    }
                }
                let outcome = original.take_outcome();
                for e in &outcome.effects {
                    debug!("ui: {e:?}");
                }
                if let Some(s) = sounds.as_deref_mut() {
                    s.0.extend(outcome.sounds);
                }
                state.feed.set_ui_open_mode(original.open_mode());
                // The Esc menu's "Save and Exit Game" (`flows/save-exit.md`
                // §1 r2): C→S 0x69; the app ends on the server's answer.
                if original.take_exit_request() {
                    if let Err(e) = crate::app::save::request_save_and_exit(&mut bridge.0) {
                        warn!("save and exit: {e}");
                    }
                }
                // Configure Controls over the game (`ui::controls_host`).
                let expansion = original.expansion_installed();
                original.service_controls(
                    expansion,
                    crate::ui::front_end::screens::controls::config_path(),
                );
                if let Some(b) = original.take_accepted_bindings() {
                    ui.bindings = Some(b);
                }
                if let Some(b) = &ui.bindings {
                    original.set_belt_keys(b);
                }
            }
            if let (true, Some(art)) = (draw, &ui.art) {
                art.ensure(&frame.draws, &mut state.assets)?;
            }
            if let (true, Some(text)) = (draw, &ui.text) {
                text.ensure(&frame.draws, &mut state.assets)?;
            }
            let mouse = match ui.cursor {
                Some(FramePos::Inside(p)) => (p.x, p.y),
                _ => (0, 0),
            };
            let view = crate::bridge::click::ClickView {
                size: crate::rules::camera::FrameSize::play(),
                open_mode: ui.original.as_ref().map_or(0, |o| o.open_mode().get()),
                // `[0x007A521C]` = H − 40 (`ui/automap.md` §9).
                right_panel_bottom: crate::rules::camera::FrameSize::play().play_height(),
                // PROVISIONAL (ui/controls.md §6 r7; controls-0001):
                // `0x00454970()` is not specified: the play area H − 40.
                skill_y_limit: crate::rules::camera::FrameSize::play().play_height(),
                mouse,
                game_menu_open: ui.original.as_ref().is_some_and(|o| o.is_open(9)),
                // d2rs-own, unverified (D1): the preview's hover pick.
                pick: state.preview,
                // `seams/world-screen.md` §2.6: the pick inverts the
                // frame's shaken camera (the anchor decided on a drawn
                // tick).
                shake: anchor.map_or((0, 0), |a| a.shake),
                // 1.14d's press sees the hover the previous pass drew.
                prev_hover: state.preview.then_some(state.prev_hover),
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
            placed = super::feed::camera_at(bridge.0.world(), state.feed.as_ref(), anchor)?;
            let cam = placed.map(|(c, _)| c);
            // d2rs-own, unverified (D1): the hover target (the preview's pick
            // under the cursor) is drawn highlighted (`blend-modes.md` §3 `h`).
            let over = matches!(ui.cursor, Some(FramePos::Inside(_)));
            let hover = cam
                .as_ref()
                .filter(|_| over && state.preview)
                .and_then(|c| crate::bridge::hover::pick(bridge.0.world(), c, mouse));
            state.feed.set_hover(hover);
            state.prev_hover = hover;
            hover_mouse = over.then_some(mouse);
            // d2rs-own, unverified (REC-239): the hovered object's name.
            if let Some(label) = cam.as_ref().and_then(|c| {
                state
                    .object_labels
                    .draw(bridge.0.world(), c, hover, ui.strings.as_ref())
            }) {
                if let (true, Some(text)) = (draw, &ui.text) {
                    text.ensure(std::slice::from_ref(&label), &mut state.assets)?;
                }
                frame.draws.push(label);
            }
            let unhandled =
                state
                    .corpse_clicks
                    .take_clicks(&mut bridge.0, cam.as_ref(), &frame.unhandled)?;
            let unhandled = state.ground_items.take_clicks(&mut bridge.0, &unhandled)?;
            // `ui/controls.md` §7 r2–r3: Shift (the hireling feed) and ui 9.
            let belt_facts = ui
                .original
                .as_ref()
                .map(|o| crate::bridge::belt::KeyFacts {
                    shift: o.shift_held(),
                    ui9_open: o.is_open(9),
                })
                .unwrap_or_default();
            crate::bridge::belt::send_keys(&mut bridge.0, &frame.unhandled, belt_facts)?;
            // The input reset `0x0044DA40` (`client/msg-ui.md` §2 r2.2):
            // held := 0 before this pass's clicks.
            if ui.original.as_mut().is_some_and(|o| o.take_input_reset()) {
                state.click.input_reset();
            }
            // Focus lost (`ui/controls.md` §4.3 r3): the left / right
            // release of a held button, unless this pass releases it.
            let mut unhandled = unhandled;
            if std::mem::take(&mut ui.focus_lost) {
                let at = crate::ui::Point::new(mouse.0, mouse.1);
                unhandled.extend(focus_releases(&state.click, &unhandled, at));
            }
            let (outs, click_outputs) = world_clicks(
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
            // `seams/bridge-app.md` §2.4: the click's sounds (refusals,
            // 0x13 "can't use that in town") and the interact's sound
            // outputs reach the audio layer in click order; the effect
            // outputs have no consumer yet.
            if let Some(s) = sounds.as_deref_mut() {
                s.0.extend(click_sounds(bridge.0.world().local_player, &outs));
                s.0.extend(click_outputs.iter().filter_map(audio_request));
            }
            let pressed = frame
                .unhandled
                .iter()
                .any(|e| matches!(e, UiEvent::Press { .. }));
            if walk.is_some() {
                state.interact.note(&outs, pressed);
            }
            if let (true, Some(w)) = (draw, walk.as_deref()) {
                // d2rs-own, unverified (D1, D2): the pending interaction
                // (its frame counts drawn ticks; the note runs below on
                // every pass).
                let walking = w.predict.walking().is_some();
                for o in state.interact.frame(&mut bridge.0, walking)? {
                    debug!("interact: {o:?}");
                }
                state.ground_items.frame(&mut bridge.0, walking)?;
                state.corpse_clicks.frame(&mut bridge.0, walking)?;
            }
            // `ui/automap.md` §8 r2: the toggle command no panel took.
            let toggle = crate::controls::Action::ToggleAutomap.index() as u16;
            if let Some(a) = state.automap.as_mut() {
                let tabs = frame
                    .unhandled
                    .iter()
                    .filter(|e| **e == UiEvent::Action(crate::ui::ActionId(toggle)))
                    .count();
                match ui.original.as_mut() {
                    // The command toggles UI state 0x0A, like the
                    // mini-panel Automap button (`control-panel.md` §9 r5).
                    Some(o) => {
                        for _ in 0..tabs {
                            o.set_ui(u32::from(AUTOMAP_STATE), 2, false)?;
                        }
                    }
                    None => {
                        for _ in 0..tabs {
                            a.toggle(&automap_facts(cam.as_ref(), view.open_mode));
                        }
                    }
                }
                // `ui/controls.md` §3 cmds 8–11, 45 (`ui/automap.md` §8
                // r2): F9 re-centre, F10 fade, F11 party, F12 names, V the
                // minimap side.
                let f = automap_facts(cam.as_ref(), view.open_mode);
                let mut spare = crate::ui::automap::options::MemoryStore::default();
                let store: &mut dyn crate::ui::automap::OptionStore =
                    match state.automap_view.as_mut() {
                        Some(v) => v.store_mut(),
                        None => &mut spare,
                    };
                for e in &frame.unhandled {
                    use crate::controls::Action as A;
                    let UiEvent::Action(id) = *e else {
                        continue;
                    };
                    match A::ALL.get(usize::from(id.0)) {
                        Some(A::CenterAutomap) => a.map.centre(&f),
                        Some(A::ToggleAutomapFade) => a.map.options.cycle_fade(store),
                        Some(A::ToggleAutomapParty) => a.map.options.toggle_party(store),
                        Some(A::ToggleAutomapNames) => a.map.options.toggle_party_names(store),
                        Some(A::ToggleMinimap) => a.map.options.toggle_left(store),
                        _ => {}
                    }
                }
                // Space with nothing to close (`panels.md` §2 r9):
                // `0x00457640(0)`, then the close-all with the automap.
                if let Some(o) = ui.original.as_mut() {
                    if o.take_clear_automap() {
                        a.map.cleared(&f);
                        o.set_ui(u32::from(AUTOMAP_STATE), 1, false)?;
                    }
                    // The shown automap follows UI state 0x0A, whoever set
                    // it (§8 r2: closing re-centres with force 0).
                    if o.is_open(AUTOMAP_STATE) != a.open {
                        a.toggle(&f);
                    }
                }
            }
            // `ui/controls.md` §3 cmd 44: no swap while ui 0x0C, 0x17 or
            // 0x19 is open.
            let swap_ok = ui
                .original
                .as_ref()
                .is_none_or(|o| super::swap_key::swap_allowed(&|s| o.is_open(s)));
            if swap_ok {
                super::swap_key::send_swaps(&frame.unhandled, &mut bridge.0)?;
            }
            // `ui/controls.md` §3 cmds 27–33, 55: C→S 0x3F with 0x19 + k
            // (0x20 for Say 7X).
            super::swap_key::send_says(&frame.unhandled, &mut bridge.0)?;
            Some(frame)
        }
        None => None,
    };
    if !draw {
        // A new tick whose draw waits for the GPU still is a client
        // update: the weather steps (REC-1900).
        if !state.last.is_some_and(|l| l.server_tick == tick) {
            state
                .feed
                .weather_update(bridge.0.world(), &mut state.assets);
        }
        return Ok(());
    }
    if let Some(d) = drawn.as_deref_mut() {
        d.0 = tick;
    }
    let draws = ui_frame.as_ref().map_or(&[][..], |f| &f.draws[..]);
    state.feed.prepare(bridge.0.world(), &mut state.assets)?;
    // `client/model.md` Randomness r4: the frame's cursor step draws on
    // the client seed after its weather update (`ui/panels-3.md` §23 r8).
    // With a schedule, a drawn frame without a recorded clock is the last
    // one only (`FrameSchedule::parse`): its step follows the dump.
    let step_now = schedule.as_ref().map(|s| s.now(tick));
    if !matches!(step_now, Some(None)) {
        if let Some(o) = ui.as_mut().and_then(|u| u.original.as_mut()) {
            o.cursor_step(bridge.0.world())
                .map_err(|e| super::ViewError::Unresolved {
                    what: "cursor step",
                    spec: "ui/panels-3.md",
                    message: e.to_string(),
                })?;
        }
    }
    // `render/lighting.md` §6.4: the drawn frame's pass over the client's
    // kept light list (§6.3), held by the model between frames.
    // A check run lights the frame with 1.14d's recorded light quality
    // (`render/lighting.md` §5: it follows the host's wall clock and draw
    // rate; the schedule's `quality` column, `tools/scenario-diff.md` §3
    // r7 step 5).
    let feed = &mut state.feed;
    feed.set_light_quality(
        schedule
            .as_ref()
            .and_then(|s| s.quality.get(&tick).copied()),
    );
    bridge
        .0
        .light_frame(|w, lights| feed.light_frame(w, lights));
    let placed = match ui_frame {
        Some(_) => placed,
        None => super::feed::camera_at(bridge.0.world(), state.feed.as_ref(), anchor)?,
    };
    // `missiles/client.md` §C13 function 2 reads the drawn frame's unit
    // origin (`render/camera.md` §4) at the next client update.
    if let (true, Some((cam, _))) = (draw, placed.as_ref()) {
        bridge.0.set_unit_origin(cam);
    }
    let built = build_frame_placed(
        bridge.0.world(),
        draws,
        state.rules.as_ref(),
        state.feed.as_mut(),
        &state.assets,
        placed,
    );
    // `sim/unit-order.md` §5 rule 7: the fill's Y sort persists in the
    // client's room lists, on every frame the fill ran, the frames whose
    // image is not built included (`seams/bridge-app.md` §2.8).
    for (room, order) in state.feed.take_unit_orders() {
        bridge.0.set_room_order(room, &order);
    }
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
    *state.camera.write().unwrap_or_else(|e| e.into_inner()) = frame.camera;
    for m in state.ground_items.add_to_frame(
        bridge.0.world(),
        state.feed.as_ref(),
        &mut state.assets,
        &mut frame,
    ) {
        warn!("preview (d2rs-own, unverified): {m}");
    }
    for m in state.missiles.add_to_frame(
        bridge.0.world(),
        state.feed.as_ref(),
        &mut state.assets,
        &mut frame,
    ) {
        warn!("preview (d2rs-own, unverified): {m}");
    }
    // `draw-order.md` §1 row 1: the level background of pass 1.
    if let (Some(v), Some((_, mode)), Some(at)) = (state.background_view.as_mut(), placed, anchor) {
        for m in v.add_to_frame(
            bridge.0.world(),
            mode.get(),
            at.player.client().x,
            &mut state.assets,
            &mut frame,
        ) {
            warn!("preview (d2rs-own, unverified): {m}");
        }
    }
    // `ui/automap.md` §10: the open automap's draw pass.
    if let (Some(a), Some(v)) = (state.automap.as_mut(), state.automap_view.as_mut()) {
        let world = bridge.0.world();
        if let (Some((_, mode)), Some(at)) = (placed, anchor) {
            for m in v.add_to_frame(
                a,
                world,
                mode,
                at.player.client(),
                &mut state.assets,
                &mut frame,
            ) {
                warn!("preview (d2rs-own, unverified): {m}");
            }
        }
    }
    // `ui/automap.md` §5 r1: the reveal of this frame, after the draw
    // marked its records (from the frame `0x0044C7EB`).
    if let Some(a) = state.automap.as_mut() {
        let world = bridge.0.world();
        let near = state.feed.near_rooms(world)?;
        a.frame(world, near)?;
    }
    // `specs/tools/scenario-diff.md` §3 r8.2: the hover the next press sees
    // is the unit whose drawn frame rectangle (+-16 px) holds the cursor in
    // this pass's frame (`0x00467AC0`); the click block above reads it.
    if state.preview {
        let mouse = hover_mouse;
        let empty = std::collections::BTreeMap::new();
        let rects = crate::bridge::hover::unit_rects(
            &frame.items,
            &state.assets.frames,
            frame.slots.as_ref().unwrap_or(&empty),
        );
        state.prev_hover =
            mouse.and_then(|m| crate::bridge::hover::pick_rects(bridge.0.world(), &rects, m));
    }
    let blank_screen = state.feed.blank_screen(bridge.0.world())?;
    let loads = bridge.0.world().act_loads;
    note_act_loads(&mut state.cycle, &mut state.act_loads, loads);
    if let Some(d) = dump.as_deref_mut().filter(|d| !d.done()) {
        d.seen += 1;
        let world = bridge.0.world();
        let open_mode = state.feed.open_mode(world).ok().map(|m| m.get());
        // §5 r19: every requested tick this frame reaches is dumped from it.
        while !d.done() && tick >= d.request.at_ticks[d.next] {
            let dir = d.request.dir_for(d.next);
            d.next += 1;
            let frame_in = crate::facts::export::DumpFrame {
                world,
                frame: &frame,
                assets: &state.assets,
                cycle: &state.cycle,
                blank_screen,
                open_mode,
                seq: d.seen,
                inputs: crate::facts::export::FrameInputs {
                    player: anchor.map(|a| {
                        let c = a.player.client();
                        (c.x, c.y)
                    }),
                    light_quality: schedule
                        .as_ref()
                        .and_then(|s| s.quality.get(&tick).copied()),
                    weather: state.feed.level_weather(),
                },
            };
            match crate::facts::export::dump(&d.request, &dir, &frame_in) {
                Ok(()) => {
                    println!(
                        "play: rendering facts of tick {tick} ({} items) written to {}",
                        frame.items.len(),
                        dir.display()
                    );
                    if d.done() {
                        exit.write(AppExit::Success);
                    }
                }
                Err(e) => {
                    eprintln!("play: --dump-draws failed: {e}");
                    d.next = d.request.at_ticks.len();
                    exit.write(AppExit::error());
                }
            }
        }
    }
    let use_gpu = gpu.is_some();
    let bridge_frame = bridge.0.world().frames;
    state.last_tags.clear();
    state.last_tags.extend(frame.items.iter().map(|i| i.tag));
    state.last_ui.clear();
    state.last_ui.extend_from_slice(draws);
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
                        play_view().width,
                        play_view().height,
                        TextureFormat::Rgba8UnormSrgb,
                        None,
                    );
                    // Copy source too: tests read the presented frame back.
                    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
                    image
                }
            } else {
                rgba_image(vec![
                    0;
                    (play_view().width * play_view().height * 4) as usize
                ])
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
/// the cursor mapping matches), converted to logical units for Bevy; the
/// image's top-left sits at the presentation's (left, top)
/// (`Presentation::centre_offset`, `seams/bridge-app.md` §2.7).
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
    let (dx, dy) = p.centre_offset();
    for mut t in &mut sprites {
        t.scale = Vec3::new(s, s, 1.0);
        t.translation.x = dx / window.scale_factor();
        t.translation.y = dy / window.scale_factor();
    }
}

#[cfg(test)]
mod act_load_tests {
    use super::*;
    use crate::scene::{FrameImage, MapTable};

    // Covers: specs/render/composition.md §3
    #[test]
    fn the_frame_after_an_act_load_presents_all_index_0() {
        // The spec's vector: framebuffer all 5, BlankScreen 1, nothing
        // drawn, 800 × 600, counter 1 → all 0, counter back to 0.
        let mut c = FrameCycle::with_pixels(800, 600, vec![5; 800 * 600]).unwrap();
        let mut seen = 0;
        note_act_loads(&mut c, &mut seen, 0);
        assert_eq!(c.post_clear(), 0, "no 0x03 yet");
        note_act_loads(&mut c, &mut seen, 1);
        assert_eq!(c.post_clear(), 1);
        let none: Vec<FrameImage> = Vec::new();
        let out = c.compose(true, &[], &none, &MapTable::default()).unwrap();
        assert!(out.iter().all(|&p| p == 0));
        assert_eq!(c.post_clear(), 0);
        // The same count again: no clear; the next frame keeps rows
        // 553–599 (BlankScreen clears rows 0–552 only).
        note_act_loads(&mut c, &mut seen, 1);
        assert_eq!(c.post_clear(), 0);
        // Two loads before one frame: one cleared frame.
        note_act_loads(&mut c, &mut seen, 3);
        assert_eq!(c.post_clear(), 1);
    }
}

#[cfg(test)]
mod order_tests {
    use super::*;

    #[derive(Resource, Default)]
    struct Order(Vec<&'static str>);

    fn outputs(mut o: ResMut<Order>) {
        o.0.push("outputs");
    }
    fn player_walk(mut o: ResMut<Order>) {
        o.0.push("player walk");
    }
    fn monster_walk(mut o: ResMut<Order>) {
        o.0.push("monster walk");
    }

    /// q-tick-flow C4 (`flows/client-frame.md` §3 r1): the three
    /// `PreUpdate` systems that take the bridge after the bridge frame run
    /// in a pinned order, whatever their registration order: the frame's
    /// outputs, the player walk, the monster tracks.
    // Covers: specs/flows/client-frame.md §3 r1; specs/client/model.md §5 r3
    #[test]
    fn the_bridge_systems_after_the_frame_run_in_a_pinned_order() {
        for _ in 0..8 {
            let mut app = App::new();
            app.init_resource::<Order>();
            app.add_systems(PreUpdate, monster_walk.in_set(PreviewOrder::MonsterWalk));
            app.add_systems(PreUpdate, player_walk.in_set(PreviewOrder::PlayerWalk));
            app.add_systems(PreUpdate, outputs.in_set(PreviewOrder::Outputs));
            configure_preview_order(&mut app);
            app.update();
            assert_eq!(
                app.world().resource::<Order>().0,
                ["outputs", "player walk", "monster walk"]
            );
        }
    }
}

#[cfg(test)]
mod click_sound_tests {
    use super::*;

    // Covers: specs/seams/bridge-app.md §2.4
    #[test]
    fn a_world_clicks_sounds_become_player_event_requests_in_order() {
        let p = UnitKey::new(0, 1);
        let outs = [
            ClickOut::Sound(0x13),
            ClickOut::Retarget,
            ClickOut::Sound(0x1F),
        ];
        assert_eq!(
            click_sounds(Some(p), &outs),
            vec![
                SoundRequest::PlayerEvent {
                    unit: p,
                    event: 0x13
                },
                SoundRequest::PlayerEvent {
                    unit: p,
                    event: 0x1F
                },
            ]
        );
        assert!(click_sounds(None, &outs).is_empty());
    }
}

#[cfg(test)]
mod schedule_tests {
    use super::FrameSchedule;

    // Covers: specs/tools/scenario-diff.md §3 r7
    #[test]
    fn a_recorded_schedule_parses_and_a_missing_clock_fails_loudly() {
        let text = "# frame-schedule 1\n# cursor_last 0\n# cursor_idle 6402500\n\
                    tick\tnow\n3\t6403156\n5\t6403281\n6\t-\n";
        let s = FrameSchedule::parse(text).unwrap();
        assert_eq!((s.cursor_last, s.cursor_idle), (0, 6_402_500));
        assert!(s.drawn(3) && !s.drawn(4) && s.drawn(5) && s.drawn(6));
        assert_eq!(
            (s.now(3), s.now(5), s.now(6)),
            (Some(6_403_156), Some(6_403_281), None)
        );
        // Only the last frame may lack its clock; no version, no timers:
        // refused, never a fallback clock.
        let gap = text.replace("3\t6403156", "3\t-");
        assert!(FrameSchedule::parse(&gap).is_err());
        // The optional light-quality column (a recorded host input).
        let q = FrameSchedule::parse(
            &text
                .replace("tick\tnow\n", "tick\tnow\tquality\n")
                .replace("3\t6403156", "3\t6403156\t2")
                .replace("5\t6403281", "5\t6403281\t0"),
        )
        .unwrap();
        assert_eq!(
            (q.quality.get(&3), q.quality.get(&5), q.quality.get(&6)),
            (Some(&2), Some(&0), None)
        );
        assert!(FrameSchedule::parse(&text.replace("# frame-schedule 1\n", "")).is_err());
        assert!(FrameSchedule::parse(&text.replace("# cursor_idle 6402500\n", "")).is_err());
    }
}
