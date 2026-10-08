// Spec: specs/render/camera.md (§3, §8, §9, §10), specs/render/composition.md (§3 steps 1–3), specs/client/render-pipeline.md (A1 stage 1), specs/render/draw-order.md (§9)
//! The camera of a drawn frame, fed from the client world, and the frame
//! built through the original's view rules ([`rules::OriginalView`]).
//!
//! A [`ViewFeed`] answers what the client world model does not hold yet:
//! the local player's position (camera §3), the screen open mode (§1),
//! the running screen shake and the player seed it draws from (§8), the
//! BlankScreen flag of the player's level (`composition.md` §3 step 2),
//! and (as a [`ViewSource`]) unit positions, unit offsets and the map tiles.
//! Each hook names its owner spec; [`NoFeed`] is the placeholder: no local
//! player, no map, no shake, open mode 0 (no original UI), and an error
//! for anything that needs a model input it lacks. A feed that states the near rooms
//! (`draw-order.md` §9) has its frame ordered by `rules::draw_order`:
//! map tiles and unit draw keys then come from the order.
//!
//! [`frame_camera`] computes the camera once per drawn frame (§3) with
//! the d2rs time base of §9: the shake envelope runs on `t = 40 × (server
//! ticks since the shake started)`, and the frame shows the model as it
//! stands after the presented tick (no interpolation; the caller draws
//! once per tick). [`build_frame`] builds through `OriginalView`; with no
//! local player there is no camera, and the frame is built through
//! [`NoCamera`], which refuses every tile and unit (nothing can be placed
//! without the §3 origins) and passes the UI through.

use d2_data::tables::Levels;
use d2_sim::rng::Seed;

use crate::bridge::drlg::DrlgRoomId;
use crate::bridge::world::{ClientWorld, UnitKey};
use crate::bridge::ClientUnit;
use crate::composite::{ComponentFrame, ComponentRequest, CompositeError, UnitParams};
use crate::frames::IndexFrame;
use crate::rules::camera::shake_offsets;
use crate::rules::draw_order::sky::SkyPasses;
use crate::rules::draw_order::source::{ordered_source, TileArt, WeatherFrame};
use crate::rules::draw_order::{FadeClock, NearRooms, OrderedTile, UnitFacts};
use crate::rules::lighting::view::{FrameLight, LitRules, LookFeed};
use crate::rules::{
    Camera, FrameSize, MapTile, OpenMode, OriginalView, Shake, UnitPosition, ViewSource,
};
use crate::scene::{BlendOp, DrawItem, ShadeChain};
use crate::ui::{ImageRequest, TextRequest, UiDraw};

use super::WorldFrame;
use super::{build, TileDraw, UiRules, UiSprite, UnitPose, ViewAssets, ViewError, ViewRules};

const CAMERA: &str = "render/camera.md";

/// A screen shake started on server tick `start_tick` (camera §8, §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunningShake {
    pub shake: Shake,
    pub start_tick: u64,
}

/// The camera inputs the client world model does not hold yet, plus the
/// [`ViewSource`] answers. One per app (it keeps the client's copy of
/// the player seed between frames).
pub trait ViewFeed: ViewSource {
    /// The local player's position as the client keeps it (camera §2, §3,
    /// `[0x007A6A70]`; `client/model.md` §3 r3 places it); `None` = the
    /// model states no local player.
    fn player(&self, world: &ClientWorld) -> Result<Option<UnitPosition>, ViewError>;

    /// The screen open mode (camera §1). It is UI state only
    /// (`ui/panels-2.md` §22 r5, `ui/panels.md` §4.2): no message or model
    /// field carries it. A feed without the original UI answers 0 (no
    /// panel open); with it, what the UI set ([`Self::set_ui_open_mode`]).
    fn open_mode(&self, world: &ClientWorld) -> Result<OpenMode, ViewError>;

    /// The screen open mode the UI set for this frame (`ui/panels.md`
    /// §4.2, [`crate::ui::original::OriginalUi::open_mode`]), handed over
    /// by the world view before each build when the original UI runs. The
    /// default ignores it: the feed answers [`Self::open_mode`] itself.
    fn set_ui_open_mode(&mut self, _mode: OpenMode) {}

    /// The play preview's predicted position of the local player
    /// (`bridge::predict`, decision D2; 16.16 sub-tiles), handed over
    /// before each build. The default ignores it (strict path: the model's
    /// cell).
    fn set_local_prediction(&mut self, _at: Option<(UnitKey, (u32, u32))>) {}

    /// The play preview's skill-move draw offsets `(dx, dy)` per unit
    /// (`world_view::skill_motion`, Leap's arc; d2rs-own, unverified),
    /// handed over before each build. The default ignores them.
    fn set_motion_offsets(&mut self, _offsets: std::collections::BTreeMap<UnitKey, (i32, i32)>) {}

    /// The unit under the cursor, handed over before each build by the
    /// play preview (`bridge::hover::pick`; d2rs-own, unverified): drawn
    /// highlighted (`blend-modes.md` §3 `h`). The default ignores it.
    fn set_hover(&mut self, _unit: Option<UnitKey>) {}

    /// The table columns of the unit facts (`world_view::unit_facts`). The
    /// default ignores them.
    fn set_unit_fact_tables(&mut self, _tables: super::unit_facts::UnitFactTables) {}

    /// Whether the view places a cel cut by the frame edge and leaves it
    /// to the frame clip (`OriginalView::with_edge_clip`, decision D1). The
    /// default (strict) is `false`.
    fn edge_clip(&self) -> bool {
        false
    }

    /// The shake running at this frame, if any (camera §8, started by
    /// `0x00476A80`). The starts d2rs knows are [`event_shake`]'s.
    fn shake(&self, world: &ClientWorld) -> Result<Option<RunningShake>, ViewError>;

    /// The client's copy of the local player unit's seed (`unit +0x20`,
    /// `ClientUnit::seed`), advanced by the two draws of each shaking
    /// frame (camera §8). Its initial value is `sim/rng.md` §5.3 and
    /// `client/model.md` Randomness r2 ({0x6AC6935F, 0} at a single-player
    /// join; camera open question 6 answered); the other draws on the same
    /// seed (cursor, weather) are `client/model.md` open question 6.
    fn player_seed(&mut self, world: &ClientWorld) -> Result<&mut Seed, ViewError>;

    /// The near-room array of the local player's active room with its tile
    /// records and unit lists (`draw-order.md` §9; built from the client
    /// DRLG, `drlg/rooms.md` §9.3, §9.6, `client/model.md` §12); the draw order
    /// writes the frame's flag and fade changes back. `None` (the default)
    /// = the model states no map, and `map_tiles` answers alone.
    fn near_rooms(&mut self, _world: &ClientWorld) -> Result<Option<&mut NearRooms>, ViewError> {
        Ok(None)
    }

    /// Before the frame's build (`present.rs`): makes the assets the
    /// feed's answers name resident (the map's DT1 tiles, the act's shade
    /// tables; `client/assets.md` §A4). The default needs none.
    fn prepare(&mut self, _world: &ClientWorld, _assets: &mut ViewAssets) -> Result<(), ViewError> {
        Ok(())
    }

    /// The facts of a room unit the draw order reads (`draw-order.md` §3
    /// r4, §5) that the client model does not hold: unit flags (+0xC4),
    /// flag-ex (+0xC8), monstats2 `unflatDead`, objects `DrawUnder`, states
    /// 7, 143, 146 and the sight test (`draw-order-2.md` §15). A feed that
    /// builds near rooms from the model asks this for every listed unit;
    /// the default refuses (no spec puts them in the model yet).
    fn unit_facts(&self, _world: &ClientWorld, unit: &ClientUnit) -> Result<UnitFacts, ViewError> {
        Err(ViewError::Unresolved {
            what: "room unit facts",
            spec: "render/draw-order.md",
            message: format!(
                "unit ({}, {}): unit flags, flag-ex, states and the sight test are not in the \
                 client model",
                unit.key.unit_type, unit.key.guid
            ),
        })
    }

    /// The room unit lists the frame's fill sorted by y (`sim/unit-order.md`
    /// §5 rule 7), each handed over once after the frame so the client's
    /// lists keep the order ([`crate::bridge::Bridge::set_room_order`]).
    fn take_unit_orders(&mut self) -> Vec<(DrlgRoomId, Vec<UnitKey>)> {
        Vec::new()
    }

    /// The weather state of the frame (`draw-order-2.md` §11; pools,
    /// floor context, the local player's seed, update count, `Mud`).
    /// `None` (the default): no weather state; a frame that draws a water
    /// floor then fails (§11.5 draws the player's seed per such floor).
    fn weather_frame(
        &mut self,
        _world: &ClientWorld,
    ) -> Result<Option<WeatherFrame<'_>>, ViewError> {
        Ok(None)
    }

    /// The draw items of the frame's passes 4 and 9 (`draw-order-2.md`
    /// §11.6, §11.7), keyed at their passes. The default draws none and
    /// refuses a frame that has draws (M07: nothing is dropped).
    fn sky_items(&self, sky: &SkyPasses, _assets: &ViewAssets) -> Result<Vec<DrawItem>, ViewError> {
        if sky.is_empty() {
            return Ok(Vec::new());
        }
        Err(ViewError::Unresolved {
            what: "weather draws",
            spec: "render/draw-order-2.md",
            message: format!(
                "{} pool cel(s) and {} sky draw(s) and the feed has no art for them",
                sky.pools.len(),
                sky.sky.len()
            ),
        })
    }

    /// The fade clock of the frame (`draw-order.md` §8 clock arithmetic):
    /// `now` is a host `GetTickCount`-style millisecond count read once per
    /// frame ([`host_tick_count`]), and `instant` is set: d2rs draws as the
    /// GDI reference (render kind 1 ≤ 3, `composition.md` §1), where every
    /// ramp completes at its first walk, so `now` only reaches the
    /// unreachable bit-2 branch (`draw-order.md` open question 16).
    fn fade_clock(&self, _world: &ClientWorld) -> Result<FadeClock, ViewError> {
        Ok(FadeClock {
            now: host_tick_count(),
            instant: true,
        })
    }

    /// The DT1 frame, blocks, shading and blend of an ordered tile: the
    /// record's DT1 entry (`drlg/rooms.md` §9.3 Entry identity, answering
    /// `draw-order.md` open question 12), its block light and shade
    /// (`render/lighting.md` §11 r2–r4, `render/shading.md` §4) and blend
    /// (`render/blend-modes.md` §6). The default refuses: the art needs the
    /// frame's light, which a feed must state ([`Self::light`]).
    fn tile_art(&self, _tile: &OrderedTile, _assets: &ViewAssets) -> Result<TileArt, ViewError> {
        Err(ViewError::unresolved(
            "tile art",
            "render/draw-order.md open question 12",
        ))
    }

    /// BlankScreen of the player's current level (`composition.md` §3 step
    /// 2; the level of the local player's room, `client/model.md` §11), i.e.
    /// [`blank_screen`] of its `Levels.txt` row; it decides
    /// the frame's start-of-frame clear ([`crate::scene::FrameCycle::plan`]).
    fn blank_screen(&self, world: &ClientWorld) -> Result<bool, ViewError>;

    /// The frame's light (`render/lighting.md` §1 r3: the light map rebuilt
    /// per drawn frame from the light records of §6, §8 and the
    /// `client/model.md` record list; the act's shade tables) and the
    /// per-unit look inputs.
    /// `Some` makes [`build_frame`] answer unit `shade` / `blend` through
    /// [`LitRules`]; `None` (the default) leaves them to the rules.
    fn light(&self, _world: &ClientWorld) -> Result<Option<FeedLight<'_>>, ViewError> {
        Ok(None)
    }
}

/// What [`ViewFeed::light`] hands the frame build.
#[derive(Clone, Copy)]
pub struct FeedLight<'a> {
    pub light: &'a FrameLight,
    pub look: &'a dyn LookFeed,
}

/// BlankScreen of a `Levels.txt` row (record `+0x218`, `composition.md`
/// §3 step 2): the `bClear` argument of `StartDraw`, which clears when
/// non-zero.
pub fn blank_screen(level: &Levels) -> bool {
    level.blankscreen != 0
}

/// The S→C 0x5A event code that starts a screen shake (`client/msg-ui.md`
/// §19 r3).
pub const SHAKE_EVENT_CODE: u8 = 0x12;

/// The screen shake an S→C 0x5A event of `code` starts at server tick
/// `start_tick` (camera §8 first row, §9): code 0x12 calls
/// `0x00476A80(6, 4000, 10000, 4000)` (`client/msg-ui.md` §19 r3); every
/// other code starts none.
pub fn event_shake(code: u8, start_tick: u64) -> Option<RunningShake> {
    if code != SHAKE_EVENT_CODE {
        return None;
    }
    Shake::start(6, 4000, 10000, 4000).map(|shake| RunningShake { shake, start_tick })
}

/// Client missile 372 `diablo appears` (function 37, `0x004D6540`) at
/// frames left 150 calls `0x00476A80(25, 0, 4000, 0)` (camera §8 fifth
/// row).
// PROVISIONAL (render/camera.md §8): of the ten call sites only this one
// and `event_shake` start a shake; the other rows start none; settled by
// REC-62 (HIGH-PRIORITY CAPTURE: each shaking frame draws twice from the
// client player seed). The client missile layer that reaches it is
// Phase 6 effects.
pub fn diablo_appears_shake(frames_left: u32, start_tick: u64) -> Option<RunningShake> {
    if frames_left != 150 {
        return None;
    }
    Shake::start(25, 0, 4000, 0).map(|shake| RunningShake { shake, start_tick })
}

/// A `GetTickCount`-style host clock (`draw-order.md` §8): milliseconds
/// since the first call, as a wrapping `u32`. Client-only host input
/// (wall clock), never read by game logic.
pub fn host_tick_count() -> u32 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static START: OnceLock<Instant> = OnceLock::new();
    let ms = START.get_or_init(Instant::now).elapsed().as_millis();
    // Wraps mod 2^32 like `GetTickCount`.
    (ms & u128::from(u32::MAX)) as u32
}

/// The placeholder feed: the client world states no local player, no map
/// and no shake (the model holds none of them: `bridge.md` §5), and every
/// question that would need a rule is an error.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoFeed;

impl ViewSource for NoFeed {
    /// The placeholder states no positions (`ModelFeed` answers them from
    /// the model, camera §2, `client/model.md` §3).
    fn unit_position(&self, _: &ClientUnit) -> Result<UnitPosition, String> {
        Err("the placeholder feed states no unit positions (render/camera.md §2)".into())
    }

    /// The extra offsets (`render/unit-composite.md` §8) need the unit's
    /// client motion record and table offsets, which the placeholder lacks.
    fn unit_offset(&self, _: &ClientUnit, _: &UnitPose) -> Result<(i32, i32), String> {
        Err(
            "the placeholder feed states no motion records for the extra offsets \
             (render/unit-composite.md §8)"
                .into(),
        )
    }

    fn map_tiles(&self, _: &ClientWorld, _: &ViewAssets) -> Result<Vec<MapTile>, ViewError> {
        Ok(Vec::new())
    }
}

impl ViewFeed for NoFeed {
    fn player(&self, _: &ClientWorld) -> Result<Option<UnitPosition>, ViewError> {
        Ok(None)
    }

    /// No original UI: open mode 0 (`ui/panels-2.md` §22 r5).
    fn open_mode(&self, _: &ClientWorld) -> Result<OpenMode, ViewError> {
        Ok(OpenMode::NONE)
    }

    fn shake(&self, _: &ClientWorld) -> Result<Option<RunningShake>, ViewError> {
        Ok(None)
    }

    fn player_seed(&mut self, _: &ClientWorld) -> Result<&mut Seed, ViewError> {
        Err(ViewError::unresolved("local player seed", CAMERA))
    }

    /// The model states no level for the player. All 137 rows of the live
    /// `levels.txt` have BlankScreen = 1 (`composition.md` §3 step 2), so
    /// every level of the original data clears; the placeholder answers
    /// that until the level is in the model.
    fn blank_screen(&self, _: &ClientWorld) -> Result<bool, ViewError> {
        Ok(true)
    }
}

fn camera_error(what: &'static str, message: String) -> ViewError {
    ViewError::Unresolved {
        what,
        spec: CAMERA,
        message,
    }
}

/// The frame's shake offsets `(dx, dy)` (camera §8) on the d2rs time base
/// (§9): `t = 40 × (server ticks − start tick)`.
pub fn frame_shake<F: ViewFeed + ?Sized>(
    world: &ClientWorld,
    feed: &mut F,
) -> Result<(i32, i32), ViewError> {
    let Some(running) = feed.shake(world)? else {
        return Ok((0, 0));
    };
    let ticks = world
        .server_ticks
        .checked_sub(running.start_tick)
        .ok_or_else(|| {
            camera_error(
                "screen shake",
                format!(
                    "shake starts at tick {}, after the frame's tick {}",
                    running.start_tick, world.server_ticks
                ),
            )
        })?;
    let ticks = u32::try_from(ticks)
        .map_err(|_| camera_error("screen shake", format!("{ticks} ticks exceed 32 bits")))?;
    match running.shake.amplitude(Shake::time_of(ticks)) {
        None | Some(0) => Ok((0, 0)),
        Some(a) => Ok(shake_offsets(a, feed.player_seed(world)?)),
    }
}

/// The camera of the frame (camera §3), or `None` without a local player.
pub fn frame_camera<F: ViewFeed + ?Sized>(
    world: &ClientWorld,
    feed: &mut F,
) -> Result<Option<Camera>, ViewError> {
    Ok(camera_and_mode(world, feed)?.map(|(camera, _)| camera))
}

/// [`frame_camera`] and the open mode it was computed with.
fn camera_and_mode<F: ViewFeed + ?Sized>(
    world: &ClientWorld,
    feed: &mut F,
) -> Result<Option<(Camera, OpenMode)>, ViewError> {
    let Some(player) = feed.player(world)? else {
        return Ok(None);
    };
    let mode = feed.open_mode(world)?;
    let shake = frame_shake(world, feed)?;
    Ok(Some((
        Camera::new(FrameSize::play(), mode, player.client(), shake),
        mode,
    )))
}

/// The open mode whose frames draw no world (`render/composition.md` §3
/// step 3: `0x00476BC0` is skipped in screen open mode 3).
const NO_WORLD_MODE: u8 = 3;

/// Builds the frame through the original's view rules: the camera once
/// (§3), then [`OriginalView`] over `rules` and `feed` (ordered by
/// `draw-order.md` when the feed states the near rooms); without a local
/// player, through [`NoCamera`]. In screen open mode 3 the camera (and its
/// shake draws) is still computed, but the world is skipped and only the
/// UI is built ([`NoWorld`], `render/composition.md` §3 steps 1 and 3).
pub fn build_frame<R, F>(
    world: &ClientWorld,
    ui: &[UiDraw],
    rules: &R,
    feed: &mut F,
    assets: &ViewAssets,
) -> Result<WorldFrame, ViewError>
where
    R: ViewRules + UiRules + ?Sized,
    F: ViewFeed + ?Sized,
{
    let placed = camera_and_mode(world, feed)?;
    let mut frame = build_placed(world, ui, rules, feed, assets, placed)?;
    frame.camera = placed.map(|(camera, _)| camera);
    Ok(frame)
}

/// [`build_frame`] with the frame's camera and open mode.
fn build_placed<R, F>(
    world: &ClientWorld,
    ui: &[UiDraw],
    rules: &R,
    feed: &mut F,
    assets: &ViewAssets,
    placed: Option<(Camera, OpenMode)>,
) -> Result<WorldFrame, ViewError>
where
    R: ViewRules + UiRules + ?Sized,
    F: ViewFeed + ?Sized,
{
    match placed {
        Some((camera, mode)) if mode.get() == NO_WORLD_MODE => build(
            world,
            ui,
            &NoWorld {
                view: &OriginalView::new(camera, rules, &*feed).with_edge_clip(feed.edge_clip()),
            },
            assets,
        ),
        Some((camera, mode)) => match ordered_source(world, &camera, mode, feed, assets)? {
            Some(source) => {
                let mut frame =
                    build_lit(world, ui, rules, camera, &source, source.source, assets)?;
                frame.slots = Some(source.units.clone());
                // Passes 4 and 9 (`draw-order-2.md` §11.6, §11.7) join the
                // sorted list by their keys.
                let sky = source.source.sky_items(&source.sky, assets)?;
                frame.sky = source.sky.sky.clone();
                if !sky.is_empty() {
                    frame.items.extend(sky);
                    crate::scene::order(&mut frame.items);
                }
                Ok(frame)
            }
            None => build_lit(world, ui, rules, camera, &*feed, &*feed, assets),
        },
        None => build(
            world,
            ui,
            &NoCamera {
                rules,
                source: &*feed,
            },
            assets,
        ),
    }
}

/// [`build`] through [`OriginalView`], with unit `shade` / `blend` from
/// the feed's light ([`LitRules`]) when it states one.
fn build_lit<R, S, F>(
    world: &ClientWorld,
    ui: &[UiDraw],
    rules: &R,
    camera: Camera,
    source: &S,
    feed: &F,
    assets: &ViewAssets,
) -> Result<WorldFrame, ViewError>
where
    R: ViewRules + UiRules + ?Sized,
    S: ViewSource + ?Sized,
    F: ViewFeed + ?Sized,
{
    let clip = feed.edge_clip();
    match feed.light(world)? {
        Some(l) => {
            let lit = LitRules {
                rules,
                feed: l.look,
                light: l.light,
            };
            let view = OriginalView::new(camera, &lit, source).with_edge_clip(clip);
            build(world, ui, &view, assets)
        }
        None => {
            let view = OriginalView::new(camera, rules, source).with_edge_clip(clip);
            build(world, ui, &view, assets)
        }
    }
}

/// The view of a frame without a camera (no local player): map tiles and
/// units cannot be placed (camera §3 needs the player), so any tile the
/// source lists and any unit the rules draw is an error; the UI, which
/// does not use the camera, goes to the wrapped rules.
#[derive(Debug, Clone, Copy)]
pub struct NoCamera<'a, R: ?Sized, S: ?Sized> {
    pub rules: &'a R,
    pub source: &'a S,
}

const NO_PLAYER: &str = "no local player position: the camera origins (§3) are undefined";

impl<R: ViewRules + ?Sized, S: ViewSource + ?Sized> ViewRules for NoCamera<'_, R, S> {
    fn tiles(&self, world: &ClientWorld, assets: &ViewAssets) -> Result<Vec<TileDraw>, ViewError> {
        let tiles = self.source.map_tiles(world, assets)?;
        if tiles.is_empty() {
            return Ok(Vec::new());
        }
        Err(camera_error(
            "tile placement",
            format!("{} map tiles; {NO_PLAYER}", tiles.len()),
        ))
    }

    fn unit_pose(
        &self,
        world: &ClientWorld,
        unit: &ClientUnit,
    ) -> Result<Option<UnitPose>, ViewError> {
        self.rules.unit_pose(world, unit)
    }

    fn unit_params(
        &self,
        _: &ClientWorld,
        unit: &ClientUnit,
        _: &UnitPose,
    ) -> Result<UnitParams, ViewError> {
        Err(camera_error(
            "unit placement",
            format!(
                "unit ({}, {}) is drawn; {NO_PLAYER}",
                unit.key.unit_type, unit.key.guid
            ),
        ))
    }

    fn component_frame(
        &self,
        unit: &ClientUnit,
        pose: &UnitPose,
        req: &ComponentRequest<'_>,
    ) -> Result<ComponentFrame, CompositeError> {
        self.rules.component_frame(unit, pose, req)
    }

    fn component_slot_frame(
        &self,
        unit: &ClientUnit,
        pose: &UnitPose,
        req: &ComponentRequest<'_>,
    ) -> Result<Option<ComponentFrame>, CompositeError> {
        self.rules.component_slot_frame(unit, pose, req)
    }

    fn place(
        &self,
        _: &ClientUnit,
        _: &UnitPose,
        req: &ComponentRequest<'_>,
        _: &IndexFrame,
    ) -> Result<(i32, i32), CompositeError> {
        Err(CompositeError::Unresolved {
            slot: req.slot.slot,
            component: req.slot.component,
            what: "placement",
            message: NO_PLAYER.into(),
        })
    }

    fn shade(
        &self,
        unit: &ClientUnit,
        req: &ComponentRequest<'_>,
    ) -> Result<ShadeChain, CompositeError> {
        self.rules.shade(unit, req)
    }

    fn blend(
        &self,
        unit: &ClientUnit,
        req: &ComponentRequest<'_>,
    ) -> Result<BlendOp, CompositeError> {
        self.rules.blend(unit, req)
    }
}

impl<R: UiRules + ?Sized, S: ?Sized> UiRules for NoCamera<'_, R, S> {
    fn ui_image(&self, req: &ImageRequest, assets: &ViewAssets) -> Result<UiSprite, ViewError> {
        self.rules.ui_image(req, assets)
    }

    fn ui_text(&self, req: &TextRequest, assets: &ViewAssets) -> Result<Vec<UiSprite>, ViewError> {
        self.rules.ui_text(req, assets)
    }

    fn ui_pass(&self) -> Result<u32, ViewError> {
        self.rules.ui_pass()
    }
}

/// A frame without its world (`render/composition.md` §3 step 3, screen
/// open mode 3): no map tile and no unit is drawn (every unit counts as
/// hidden); the UI goes to the wrapped view.
#[derive(Debug, Clone, Copy)]
pub struct NoWorld<'a, V: ?Sized> {
    pub view: &'a V,
}

impl<V: ViewRules + ?Sized> ViewRules for NoWorld<'_, V> {
    fn tiles(&self, _: &ClientWorld, _: &ViewAssets) -> Result<Vec<TileDraw>, ViewError> {
        Ok(Vec::new())
    }

    fn unit_pose(&self, _: &ClientWorld, _: &ClientUnit) -> Result<Option<UnitPose>, ViewError> {
        Ok(None)
    }

    fn unit_params(
        &self,
        world: &ClientWorld,
        unit: &ClientUnit,
        pose: &UnitPose,
    ) -> Result<UnitParams, ViewError> {
        self.view.unit_params(world, unit, pose)
    }

    fn component_frame(
        &self,
        unit: &ClientUnit,
        pose: &UnitPose,
        req: &ComponentRequest<'_>,
    ) -> Result<ComponentFrame, CompositeError> {
        self.view.component_frame(unit, pose, req)
    }

    fn place(
        &self,
        unit: &ClientUnit,
        pose: &UnitPose,
        req: &ComponentRequest<'_>,
        image: &IndexFrame,
    ) -> Result<(i32, i32), CompositeError> {
        self.view.place(unit, pose, req, image)
    }

    fn shade(
        &self,
        unit: &ClientUnit,
        req: &ComponentRequest<'_>,
    ) -> Result<ShadeChain, CompositeError> {
        self.view.shade(unit, req)
    }

    fn blend(
        &self,
        unit: &ClientUnit,
        req: &ComponentRequest<'_>,
    ) -> Result<BlendOp, CompositeError> {
        self.view.blend(unit, req)
    }
}

impl<V: UiRules + ?Sized> UiRules for NoWorld<'_, V> {
    fn ui_image(&self, req: &ImageRequest, assets: &ViewAssets) -> Result<UiSprite, ViewError> {
        self.view.ui_image(req, assets)
    }

    fn ui_text(&self, req: &TextRequest, assets: &ViewAssets) -> Result<Vec<UiSprite>, ViewError> {
        self.view.ui_text(req, assets)
    }

    fn ui_pass(&self) -> Result<u32, ViewError> {
        self.view.ui_pass()
    }
}

#[cfg(test)]
mod tests;
