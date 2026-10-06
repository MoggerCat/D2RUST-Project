// Spec: specs/render/capture.md (§3, §4, §6), specs/render/camera.md (§1–§4, §8, §9), specs/client/render-pipeline.md (A10 link 1)
//! The scene source of the `scene` compare case: a recorded state
//! (capture.md §3) through the world view and the original's view rules
//! to compositor inputs.
//!
//! Per frame, [`WorldScene`]:
//!
//! 1. builds the frame's camera from the recorded state (camera §3): the
//!    player's 16.16 path position (§2), the open mode (§1), the frame
//!    size, and the recorded shake offsets `(dx, dy)` as input (camera §9,
//!    capture.md §4: the original's envelope runs on wall-clock time);
//! 2. checks the recorded camera against [`Camera::new`]: view rectangle
//!    and `shiftX` (§1), tile origin and unit origin (§3). A difference
//!    fails the frame (the rule disagrees with the original), before any
//!    pixel is composed;
//! 3. asks a [`CaptureWorld`] for what the recording must state beyond
//!    the camera: the client world (units), their positions and offsets
//!    and the map tiles ([`ViewSource`]), the other owners' hooks
//!    ([`WorldRules`]), the UI requests and the assets (frames, maps,
//!    palette). A frame needs its draw log and its initial framebuffer
//!    (capture.md §6); d2rs does not yet read the draw log into these
//!    inputs: [`NotRecorded`] stops there and names the first missing
//!    one ([`NO_DRAW_LOG`], [`NO_INITIAL_FRAME`], [`RECORDER_GAP`]);
//! 4. builds the frame with [`world_view::build`] through
//!    [`OriginalView`] and hands the draw list, frames, maps and palette
//!    to the comparison (`capture_case::compare_one`).

use super::{SceneJob, SceneOutcome, SceneSource};
use crate::bridge::world::ClientWorld;
use crate::rules::{Camera, FrameSize, OpenMode, OriginalView, UnitPosition, ViewSource};
use crate::scene::FrameImage;
use crate::ui::UiDraw;
use crate::verify::capture::State;
use crate::verify::Built;
use crate::world_view::present::WorldRules;
use crate::world_view::{self, ViewAssets, VIEW};

/// What a recorded frame draws beyond the camera, as the world view needs
/// it.
pub struct RecordedScene<'a> {
    pub world: &'a ClientWorld,
    pub ui: &'a [UiDraw],
    pub rules: &'a dyn WorldRules,
    pub source: &'a dyn ViewSource,
    pub assets: &'a ViewAssets,
}

/// What a [`CaptureWorld`] answered for a frame.
pub enum WorldAnswer<'a> {
    Scene(RecordedScene<'a>),
    /// The recording does not hold what the world view needs: names the
    /// missing fields. Never a pass (`SCENE NOT WIRED`).
    Seam(String),
    /// The recording holds the fields, but they do not make a scene.
    Error(String),
}

/// The world of a recorded frame (units, map, UI, assets).
pub trait CaptureWorld {
    fn scene(&mut self, job: &SceneJob<'_>, camera: &Camera) -> WorldAnswer<'_>;
}

/// What a recorded frame lacks before d2rs can draw it (capture.md §6):
/// a frame without a draw log cannot be composed.
pub const NO_DRAW_LOG: &str = "the frame has no draw log (capture.md §3.5, §6: \
     a frame without one cannot be composed; record with --draws-every 1, \
     or select frames the draw log covers)";

/// The initial framebuffer of capture.md §6 is missing.
pub const NO_INITIAL_FRAME: &str = "no initial framebuffer (capture.md §6: \
     the previous captured frame seq − 1 of a recording made with --every 1; \
     the bottom 47 rows and unwritten pixels keep older content, composition.md §3)";

/// What stands between a `frames-raw-2` frame with a draw log and the
/// d2rs scene. The camera, level, act and draw calls are recorded; these
/// are not, or not yet read.
pub const RECORDER_GAP: &str = "the draw log is recorded (capture.md §3.5) but not yet \
     turned into world-view inputs: \
     (1) the DT1 file of each drawn tile header (capture.md open question 3); \
     (2) the unit component cel files, loaded outside 0x004788B0 (capture.md open question 4); \
     (3) the units' positions as camera.md §2 inputs and the extra offsets \
     (camera.md open question 3), the COF pose and component tokens (render/unit-composite.md); \
     (4) the UI draws named through the celfile records (§3.6). \
     The world-view hooks of draw order, shading and blend stay TODO(spec) after that";

/// The [`CaptureWorld`] of today's recordings: stops at the seam, naming
/// the first missing input (§6 draw log, §6 initial framebuffer, then
/// [`RECORDER_GAP`]).
pub struct NotRecorded;

impl CaptureWorld for NotRecorded {
    fn scene(&mut self, job: &SceneJob<'_>, _: &Camera) -> WorldAnswer<'_> {
        let gap = if job.captured.draws.is_none() {
            NO_DRAW_LOG
        } else if job.previous.is_none() {
            NO_INITIAL_FRAME
        } else {
            RECORDER_GAP
        };
        WorldAnswer::Seam(gap.into())
    }
}

/// The camera of a recorded frame (camera §1–§3), from the player's 16.16
/// path position, the open mode, the frame size and the recorded shake
/// offsets.
pub fn recorded_camera(w: i32, h: i32, state: &State) -> Result<Camera, String> {
    let size = FrameSize {
        width: w,
        height: h,
    };
    if size != FrameSize::D2RS {
        return Err(format!(
            "frame {w}x{h}: d2rs draws 800x600 only (camera.md §1)"
        ));
    }
    let player = state
        .player
        .as_ref()
        .ok_or("no player record: an in-game frame needs the local player (camera.md §3)")?;
    let [x16, y16] = player
        .fixed
        .ok_or("the player record has no path position (capture.md §3 `fixed`)")?;
    let mode = u8::try_from(state.open_mode)
        .ok()
        .and_then(OpenMode::new)
        .ok_or_else(|| format!("open mode {} is not 0–3 (camera.md §1)", state.open_mode))?;
    let [_, dx, dy] = state.shake;
    let offset = |v: i64| i32::try_from(v).map_err(|_| format!("shake offset {v} exceeds 32 bits"));
    let shake = (offset(dx)?, offset(dy)?);
    let at = UnitPosition::Moving { x16, y16 }.client();
    Ok(Camera::new(size, mode, at, shake))
}

/// The recorded camera against the rule: one line per difference (empty
/// when every recorded value equals [`Camera::new`]'s).
pub fn camera_differences(camera: &Camera, state: &State) -> Vec<String> {
    let mut out = Vec::new();
    let v = &camera.view;
    let want_view = [v.left, v.top, v.right, v.bottom];
    match state.view_rect {
        Some(r) if r == want_view => {}
        Some(r) => out.push(format!(
            "view rect (camera.md §1): recorded {r:?}, rule {want_view:?}"
        )),
        None => out.push("view rect (camera.md §1): not recorded (view pointer null)".into()),
    }
    if state.shift_x != v.shift_x {
        out.push(format!(
            "shiftX (camera.md §1): recorded {}, rule {}",
            state.shift_x, v.shift_x
        ));
    }
    let want_tile = [camera.tile.x, camera.tile.y];
    match state.tile_origin {
        Some(t) if t == want_tile => {}
        Some(t) => out.push(format!(
            "tile origin (camera.md §3): recorded {t:?}, rule {want_tile:?}"
        )),
        None => out.push("tile origin (camera.md §3): not recorded (view pointer null)".into()),
    }
    let want_unit = [camera.unit.x, camera.unit.y];
    if state.unit_origin != want_unit {
        out.push(format!(
            "unit origin (camera.md §3): recorded {:?}, rule {want_unit:?}",
            state.unit_origin
        ));
    }
    out
}

/// The scene source of the world view: recorded camera, checked, then the
/// world of a [`CaptureWorld`] through [`OriginalView`].
pub struct WorldScene<W> {
    pub world: W,
    notes: Vec<String>,
}

impl<W: CaptureWorld> WorldScene<W> {
    pub fn new(world: W) -> Self {
        WorldScene {
            world,
            notes: Vec::new(),
        }
    }
}

/// The compositor inputs of a frame built by the world view.
pub fn built(frame: world_view::WorldFrame, assets: &ViewAssets) -> Built {
    Built {
        items: frame.items,
        frames: assets
            .frames
            .frames()
            .iter()
            .map(|f| FrameImage {
                width: f.width,
                height: f.height,
                pixels: f.pixels.clone(),
            })
            .collect(),
        maps: assets.maps.clone(),
        palette: assets.palette.clone(),
        view: VIEW,
    }
}

impl<W: CaptureWorld> SceneSource for WorldScene<W> {
    fn scene(&mut self, job: &SceneJob<'_>) -> SceneOutcome {
        let state = &job.captured.state;
        let camera = match recorded_camera(job.frame.w, job.frame.h, state) {
            Ok(c) => c,
            Err(e) => return SceneOutcome::Error(format!("camera: {e}")),
        };
        let differ = camera_differences(&camera, state);
        if !differ.is_empty() {
            return SceneOutcome::Differs(differ);
        }
        self.notes.push(format!(
            "camera: recorded view rect, shiftX, tile origin {:?} and unit origin {:?} equal camera.md §1, §3",
            [camera.tile.x, camera.tile.y],
            [camera.unit.x, camera.unit.y]
        ));
        match self.world.scene(job, &camera) {
            WorldAnswer::Seam(s) => SceneOutcome::Seam(s),
            WorldAnswer::Error(e) => SceneOutcome::Error(format!("world: {e}")),
            WorldAnswer::Scene(s) => {
                let view = OriginalView::new(camera, s.rules, s.source);
                match world_view::build(s.world, s.ui, &view, s.assets) {
                    Ok(frame) => SceneOutcome::Built(Box::new(built(frame, s.assets))),
                    Err(e) => SceneOutcome::Error(format!("world view: {e}")),
                }
            }
        }
    }

    fn take_notes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notes)
    }
}
