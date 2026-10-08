// Spec: specs/client/render-pipeline.md (A1 stages 1–4, A7), specs/client/bridge.md (§5, §7 rule 4), specs/client/ui.md (A2), specs/render/composition.md (§3, §4)
//! World view: the client's model ([`ClientWorld`]) turned into the scene
//! draw list each frame (render-pipeline §A1 stage 1), then ordered
//! (stage 2) and composed (stage 4a CPU, 4b GPU). Plain Rust, no Bevy
//! types outside [`present`].
//!
//! A presented frame is one frame of the 1.14d frame cycle
//! (`composition.md` §3): a [`FrameCycle`] keeps the index framebuffer
//! between frames, each frame's clears are `cycle.plan(blank_screen)`
//! (BlankScreen of the player's level, [`ViewFeed::blank_screen`]), and
//! the composed indices become the next frame's base
//! ([`compose_cycle_cpu`], [`GpuAtlas::compose_cycle`]).
//!
//! The mechanism is ours: units go through the C7 COF composite
//! ([`composite::build_with`]) in unit-key order, frames come from the
//! frame store of resident C3 frame sets ([`ViewAssets::frames`],
//! [`FrameStore`]: `(FrameSetKey, index)` → scene [`FrameId`], §A7 step
//! 3), UI requests of the C8 core become
//! items in emission order ([`ui_bind`]), and the list is sorted by the C4
//! key ([`scene::order`], stable). Every rule of the original (which COF
//! and frame a unit shows, where a sprite goes, draw keys, shading, blend,
//! map tiles, UI art and text layout) is a method of [`ViewRules`] or
//! [`ui_bind::UiRules`], each naming its owner spec. [`Unspecified`]
//! answers them with the narrowest neutral behavior: nothing the model
//! does not state is drawn, and anything that would need a rule to draw
//! is an error, never a default.
//!
//! The client decides no outcome here (CLAUDE.md rule 7): this module only
//! reads the model.

pub mod automap_view;
pub mod corpse_click;
pub mod disguise;
pub mod feed;
pub mod ground_items;
pub mod interact;
pub mod light_sources;
pub mod missiles;
pub mod model_feed;
pub mod monster_walk;
pub mod near_rooms;
pub mod node;
pub mod object_click;
pub mod object_label;
pub mod overlay;
pub mod panel_art;
pub mod present;
pub mod preview;
pub mod preview_blocks;
pub mod preview_light;
pub mod skill_motion;
pub mod state_tint;
pub mod swap_key;
pub mod tile_assets;
pub mod ui_bind;
pub mod unit_assets;
pub mod unit_facts;
pub mod unit_rules;
pub mod unit_shadow;
pub mod visibility;
pub mod walk;
pub mod walk_room;
pub mod weather_view;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use d2_formats::cof::Cof;
use d2_formats::font::FontTable;
use d2_formats::palette::Palette;

use crate::assets::path::CanonicalPath;
use crate::bridge::world::ClientWorld;
use crate::bridge::ClientUnit;
use crate::composite::{self, ComponentFrame, ComponentRequest, CompositeError, UnitParams};
use crate::frames::{
    Atlas, AtlasError, AtlasSlot, FrameSetKey, FrameStore, IndexFrame, StoreError,
};
use crate::gpu_compositor::{self, Gpu, GpuError};
use crate::scene::{
    self, BlendOp, DrawItem, DrawKey, FrameCycle, FrameId, ItemTag, MapTable, Rect, SceneError,
    ShadeChain,
};

pub use feed::{
    blank_screen, build_frame, build_frame_placed, camera_and_mode, camera_at, frame_anchor,
    frame_camera, FeedLight, NoCamera, NoFeed, RunningShake, ViewFeed,
};
pub use model_feed::ModelFeed;
pub use present::{UiSounds, WorldViewGpu, WorldViewPlugin, WorldViewState, WorldViewUi};
pub use ui_bind::{
    original_text_font, run_ui, run_ui_with, text_sprites, OriginalTextHooks, TextColors, TextFont,
    TextHooks, UiQueue, UiRules, UiRunError, UiSprite,
};

/// The region composed each frame: the full 800×600 frame. Which part of
/// the world it shows is the camera's (render-pipeline §B7), decided by
/// the [`ViewRules`] placement hooks, not by the view rectangle.
pub const VIEW: Rect = Rect::FRAME;

/// Errors of a world-view frame. Strict (METHODS M07): a frame the rules
/// cannot fully answer fails as a whole; nothing is skipped or defaulted.
#[derive(Debug, thiserror::Error)]
pub enum ViewError {
    /// A rule hook could not answer; `what` names the hook and `spec` the
    /// owner spec that will define it.
    #[error("{what} (TODO(spec: {spec})): {message}")]
    Unresolved {
        what: &'static str,
        spec: &'static str,
        message: String,
    },
    #[error("COF {0:?} is not loaded")]
    CofMissing(CanonicalPath),
    #[error("font table {0:?} is not loaded")]
    FontMissing(CanonicalPath),
    #[error(transparent)]
    Text(#[from] crate::ui::TextError),
    /// A frame the frame store does not hold (set not resident, index
    /// past the set's end).
    #[error(transparent)]
    Frame(#[from] StoreError),
    /// The GPU atlas was built for another store (C2 eviction rebuilds
    /// both; not wired).
    #[error("GPU atlas holds {atlas} frames, the frame store {store}: rebuild the atlas")]
    AtlasAhead { atlas: usize, store: usize },
    #[error("unit ({unit_type}, {guid}): {error}")]
    Unit {
        unit_type: u8,
        guid: u32,
        error: CompositeError,
    },
    #[error("map tile {index}: {error}")]
    Tile { index: usize, error: Box<ViewError> },
    #[error("UI request {index}: {error}")]
    Ui { index: usize, error: Box<ViewError> },
    #[error(transparent)]
    Scene(#[from] SceneError),
    #[error(transparent)]
    Atlas(#[from] AtlasError),
    #[error(transparent)]
    Gpu(#[from] GpuError),
}

impl ViewError {
    /// The error of a hook that has no owner spec yet.
    pub fn unresolved(what: &'static str, spec: &'static str) -> Self {
        ViewError::Unresolved {
            what,
            spec,
            message: "the neutral rules draw no unit component".into(),
        }
    }
}

/// Everything a frame reads besides the model: parsed COFs, the frame
/// store of resident frame sets (residency is `client/assets.md` §A4: a
/// set missing here is an error, never a skipped draw), the map table
/// (PL2 rows, blend tables) and the frame palette.
#[derive(Debug, Clone)]
pub struct ViewAssets {
    pub cofs: BTreeMap<CanonicalPath, Cof>,
    /// Parsed font tables (`formats/font-tbl.md`) for UI text.
    pub fonts: BTreeMap<CanonicalPath, FontTable>,
    /// Resident frame sets and their scene ids (verify-map's store: the
    /// same numbering for the CPU reference and the GPU atlas).
    pub frames: FrameStore,
    pub maps: MapTable,
    /// The presented palette: one per frame, no per-region palettes
    /// (`composition.md` §4); the act's `pal.pl2` through
    /// [`scene::present_palette`] ([`ViewAssets::from_pl2`]).
    pub palette: Palette,
    /// The shade tables of the act's PL2 (pushed into `maps`), once a
    /// feed has them: unit shadows (`blend-modes.md` §5) read the zero and
    /// alpha maps. `None`: no shadow is drawn.
    pub shades: Option<crate::rules::shading::ShadeTables>,
}

impl ViewAssets {
    /// No COFs or frames, an empty map table.
    pub fn new(palette: Palette) -> Self {
        ViewAssets {
            cofs: BTreeMap::new(),
            fonts: BTreeMap::new(),
            frames: FrameStore::new(),
            maps: MapTable::new(),
            palette,
            shades: None,
        }
    }

    /// [`ViewAssets::new`] with the presented palette of an act's
    /// `pal.pl2` (`composition.md` §4: its first 1,024 bytes).
    pub fn from_pl2(pl2: &[u8]) -> Result<Self, ViewError> {
        Ok(Self::new(scene::present_palette(pl2)?))
    }

    /// The scene id of frame `index` of the resident set `key`.
    pub fn id(&self, key: &FrameSetKey, index: usize) -> Result<FrameId, ViewError> {
        Ok(self.frames.id(key, index)?)
    }

    /// Frame `index` of the resident set `key`.
    pub fn frame(&self, key: &FrameSetKey, index: usize) -> Result<&IndexFrame, ViewError> {
        let id = self.id(key, index)?;
        Ok(self
            .frames
            .frame(id)
            .expect("the store gave the id, so it holds the frame"))
    }
}

/// Which animation a unit shows this frame: a COF and the COF direction
/// and frame (render-pipeline §A7; the sim owns animation state, the
/// client never advances it on its own).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitPose {
    pub cof: CanonicalPath,
    pub dir: usize,
    pub frame: usize,
}

/// One map tile draw, fully answered by [`ViewRules::tiles`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileDraw {
    /// The DT1 tile frame set and frame.
    pub frame: ComponentFrame,
    /// Screen top-left (§B1, §B7).
    pub x: i32,
    pub y: i32,
    pub clip: Rect,
    pub shade: ShadeChain,
    pub blend: BlendOp,
    pub key: DrawKey,
    /// Debug label (`ItemTag::Tile`).
    pub cell: (i32, i32),
}

/// The original-behavior questions of the world view, one hook per
/// question; each method names the owner spec it follows.
pub trait ViewRules {
    /// The map tiles to draw and how: which records (`render/draw-order.md`
    /// §9, §10; the near rooms and tile records of the client DRLG,
    /// `drlg/rooms.md` §9), their screen position and culling
    /// (`render/camera.md` §6, §7). `rules::OriginalView` answers it from
    /// its `ViewSource`.
    fn tiles(&self, world: &ClientWorld, assets: &ViewAssets) -> Result<Vec<TileDraw>, ViewError>;

    /// `render/unit-composite.md` §2, §3, §4, §10 (with the unit's mode,
    /// direction and frame from the S→C message specs): which COF, COF
    /// direction and frame the unit shows. `None` = the unit is not drawn.
    fn unit_pose(
        &self,
        world: &ClientWorld,
        unit: &ClientUnit,
    ) -> Result<Option<UnitPose>, ViewError>;

    /// `render/draw-order.md` §10 (pass/major/minor) and
    /// `render/camera.md` §10 (clip: the frame) shared by the unit's
    /// components.
    fn unit_params(
        &self,
        world: &ClientWorld,
        unit: &ClientUnit,
        pose: &UnitPose,
    ) -> Result<UnitParams, ViewError>;

    /// `render/unit-composite.md` §5.1, §6, §10: the component's frame
    /// set (file path from token, variant, mode, weapon class; file
    /// direction) and frame.
    fn component_frame(
        &self,
        unit: &ClientUnit,
        pose: &UnitPose,
        req: &ComponentRequest<'_>,
    ) -> Result<ComponentFrame, CompositeError>;

    /// The unit's shadow draws (`render/blend-modes.md` §5 r1–r3), keyed
    /// at the shadow pass slot `at` (`draw-order.md` §6 r3; `None` until
    /// `OriginalView` fills it from the frame's draw order), from the
    /// component draws `draws` the unit just built. The default draws
    /// none.
    fn unit_shadows(
        &self,
        _world: &ClientWorld,
        _unit: &ClientUnit,
        _pose: &UnitPose,
        _at: Option<crate::rules::draw_order::OrderKey>,
        _draws: &[composite::ComponentDraw],
        _assets: &ViewAssets,
    ) -> Result<Vec<DrawItem>, ViewError> {
        Ok(Vec::new())
    }

    /// The component's frame, or `None` when the slot draws nothing
    /// (`render/unit-composite.md` §5 r2, §6 r4: failed component request,
    /// missing file). The default draws every slot with
    /// [`ViewRules::component_frame`].
    fn component_slot_frame(
        &self,
        unit: &ClientUnit,
        pose: &UnitPose,
        req: &ComponentRequest<'_>,
    ) -> Result<Option<ComponentFrame>, CompositeError> {
        self.component_frame(unit, pose, req).map(Some)
    }

    /// `render/camera.md` §4, §10, then `render/sprite-placement.md` §2,
    /// §8: screen top-left of the component image from the unit's
    /// position and the frame's own offsets (`image.x_off/y_off`).
    fn place(
        &self,
        unit: &ClientUnit,
        pose: &UnitPose,
        req: &ComponentRequest<'_>,
        image: &IndexFrame,
    ) -> Result<(i32, i32), CompositeError>;

    /// `render/shading.md` §6, §10, `render/unit-composite.md` §7,
    /// `render/lighting.md` §11 r1, §13: the component's remap `P` and
    /// light map `L` (`rules::lighting::view::LitRules`).
    fn shade(
        &self,
        unit: &ClientUnit,
        req: &ComponentRequest<'_>,
    ) -> Result<ShadeChain, CompositeError>;

    /// `render/blend-modes.md` §3, §7: the component's draw mode (with the
    /// COF layer override) as a blend op.
    fn blend(
        &self,
        unit: &ClientUnit,
        req: &ComponentRequest<'_>,
    ) -> Result<BlendOp, CompositeError>;
}

/// The neutral rules while the model lacks the inputs the owner specs
/// read: no map tiles, no unit drawn (the model states nothing a unit
/// looks like: equipped items, component choices), and every hook that
/// would be needed to draw something is an error. A frame built with these
/// is the empty list (plus nothing from the UI unless a panel draws, which
/// is then an error).
#[derive(Debug, Clone, Copy, Default)]
pub struct Unspecified;

fn component_unresolved(req: &ComponentRequest<'_>, what: &'static str) -> CompositeError {
    CompositeError::Unresolved {
        slot: req.slot.slot,
        component: req.slot.component,
        what,
        message: "the neutral rules draw no unit component".into(),
    }
}

impl ViewRules for Unspecified {
    fn tiles(&self, _: &ClientWorld, _: &ViewAssets) -> Result<Vec<TileDraw>, ViewError> {
        Ok(Vec::new())
    }

    fn unit_pose(&self, _: &ClientWorld, _: &ClientUnit) -> Result<Option<UnitPose>, ViewError> {
        Ok(None)
    }

    fn unit_params(
        &self,
        _: &ClientWorld,
        _: &ClientUnit,
        _: &UnitPose,
    ) -> Result<UnitParams, ViewError> {
        Err(ViewError::unresolved(
            "unit draw key",
            "render/draw-order.md",
        ))
    }

    fn component_frame(
        &self,
        _: &ClientUnit,
        _: &UnitPose,
        req: &ComponentRequest<'_>,
    ) -> Result<ComponentFrame, CompositeError> {
        Err(component_unresolved(req, "component frame"))
    }

    fn place(
        &self,
        _: &ClientUnit,
        _: &UnitPose,
        req: &ComponentRequest<'_>,
        _: &IndexFrame,
    ) -> Result<(i32, i32), CompositeError> {
        Err(component_unresolved(req, "placement"))
    }

    fn shade(
        &self,
        _: &ClientUnit,
        req: &ComponentRequest<'_>,
    ) -> Result<ShadeChain, CompositeError> {
        Err(component_unresolved(req, "shade"))
    }

    fn blend(&self, _: &ClientUnit, req: &ComponentRequest<'_>) -> Result<BlendOp, CompositeError> {
        Err(component_unresolved(req, "blend"))
    }
}

/// One built frame: the ordered draw list. Its frame ids are
/// [`ViewAssets::frames`] ids.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorldFrame {
    /// Sorted by [`DrawKey`] (stable: equal keys keep build order).
    pub items: Vec<DrawItem>,
    /// Units drawn, and units the rules left undrawn (`unit_pose` = None).
    pub units_drawn: usize,
    pub units_hidden: usize,
    /// The camera the frame was placed with (`render/camera.md` §3);
    /// `None` without one. Read back by [`visibility`] (the origin
    /// getters of `client/model.md` §13 r1 return the last drawn frame's).
    pub camera: Option<crate::rules::camera::Camera>,
}

/// The C7 resolver of one unit: the hooks. Frame ids are not a hook: they
/// come from the frame store ([`composite::build_with`]).
struct UnitResolver<'a, R: ?Sized> {
    rules: &'a R,
    unit: &'a ClientUnit,
    pose: &'a UnitPose,
    assets: &'a ViewAssets,
}

fn not_resident(req: &ComponentRequest<'_>, what: &'static str, e: ViewError) -> CompositeError {
    CompositeError::Unresolved {
        slot: req.slot.slot,
        component: req.slot.component,
        what,
        message: e.to_string(),
    }
}

impl<R: ViewRules + ?Sized> composite::ComponentResolver for UnitResolver<'_, R> {
    fn frame(&self, req: &ComponentRequest<'_>) -> Result<ComponentFrame, CompositeError> {
        self.rules.component_frame(self.unit, self.pose, req)
    }

    fn slot_frame(
        &self,
        req: &ComponentRequest<'_>,
    ) -> Result<Option<ComponentFrame>, CompositeError> {
        self.rules.component_slot_frame(self.unit, self.pose, req)
    }

    fn place(
        &self,
        req: &ComponentRequest<'_>,
        frame: &ComponentFrame,
    ) -> Result<(i32, i32), CompositeError> {
        let image = self
            .assets
            .frame(&frame.set, frame.index)
            .map_err(|e| not_resident(req, "place", e))?;
        self.rules.place(self.unit, self.pose, req, image)
    }

    fn shade(&self, req: &ComponentRequest<'_>) -> Result<ShadeChain, CompositeError> {
        self.rules.shade(self.unit, req)
    }

    fn blend(&self, req: &ComponentRequest<'_>) -> Result<BlendOp, CompositeError> {
        self.rules.blend(self.unit, req)
    }
}

/// Builds and orders the frame's draw list (§A1 stages 1–2): map tiles,
/// then units in key order (C7 composite each), then the UI requests in
/// emission order; then the stable sort by key. Frame ids are the frame
/// store's. Any error fails the frame.
pub fn build<R: ViewRules + UiRules + ?Sized>(
    world: &ClientWorld,
    ui: &[crate::ui::UiDraw],
    rules: &R,
    assets: &ViewAssets,
) -> Result<WorldFrame, ViewError> {
    let mut items = Vec::new();

    for (index, t) in rules.tiles(world, assets)?.into_iter().enumerate() {
        let at = |error| ViewError::Tile {
            index,
            error: Box::new(error),
        };
        let id = assets.id(&t.frame.set, t.frame.index).map_err(at)?;
        let mut item = DrawItem::new(id, t.x, t.y);
        item.clip = t.clip;
        item.shade = t.shade;
        item.blend = t.blend;
        item.key = t.key;
        item.tag = ItemTag::Tile {
            x: t.cell.0,
            y: t.cell.1,
        };
        items.push(item);
    }

    let (mut units_drawn, mut units_hidden) = (0, 0);
    for unit in world.units.values() {
        let Some(pose) = rules.unit_pose(world, unit)? else {
            units_hidden += 1;
            continue;
        };
        let cof = assets
            .cofs
            .get(&pose.cof)
            .ok_or_else(|| ViewError::CofMissing(pose.cof.clone()))?;
        let params = rules.unit_params(world, unit, &pose)?;
        let resolver = UnitResolver {
            rules,
            unit,
            pose: &pose,
            assets,
        };
        let draws = composite::build_with(
            cof,
            pose.dir,
            pose.frame,
            &params,
            &resolver,
            &assets.frames,
        )
        .map_err(|error| ViewError::Unit {
            unit_type: unit.key.unit_type,
            guid: unit.key.guid,
            error,
        })?;
        let shadows = rules.unit_shadows(world, unit, &pose, None, &draws, assets)?;
        items.extend(draws.into_iter().map(|d| d.item));
        items.extend(shadows);
        units_drawn += 1;
    }

    ui_bind::ui_items(ui, rules, assets, &mut items)?;

    scene::order(&mut items);
    Ok(WorldFrame {
        items,
        units_drawn,
        units_hidden,
        camera: None,
    })
}

/// CPU reference image of a built frame (§A8): RGBA8, alpha 255,
/// `VIEW.width × VIEW.height`.
pub fn compose_cpu(frame: &WorldFrame, assets: &ViewAssets) -> Result<Vec<u8>, ViewError> {
    Ok(scene::compose_rgba(
        &frame.items,
        &assets.frames,
        &assets.maps,
        &assets.palette,
        VIEW,
    )?)
}

/// One frame of the frame cycle on the CPU reference (`composition.md`
/// §3): `cycle.compose` with the plan of `blank_screen`, the draws onto
/// the persistent framebuffer; returns the presented RGBA8 image through
/// the frame palette (§4). The cycle must be `VIEW` sized. On error the
/// cycle is unchanged.
pub fn compose_cycle_cpu(
    cycle: &mut FrameCycle,
    blank_screen: bool,
    frame: &WorldFrame,
    assets: &ViewAssets,
) -> Result<Vec<u8>, ViewError> {
    check_cycle(cycle)?;
    let indices = cycle.compose(blank_screen, &frame.items, &assets.frames, &assets.maps)?;
    Ok(scene::to_rgba(indices, &assets.palette))
}

/// The world view composes `VIEW`; a cycle of another size has no frame
/// mapping.
fn check_cycle(cycle: &FrameCycle) -> Result<(), ViewError> {
    if cycle.view() != VIEW {
        return Err(SceneError::BaseSize {
            len: cycle.pixels().len(),
            pixels: u64::from(VIEW.width) * u64::from(VIEW.height),
        }
        .into());
    }
    Ok(())
}

/// The atlas the GPU compositor reads: every frame of the frame store in
/// id order, so `slots[n]` is the slot of `FrameId(n)` (the
/// [`FrameStore::atlas`] numbering). The store is append-only, so frames
/// added since the last call are packed on top (C3 packer); packed frames
/// never move. Design note (C2, no original behavior): page eviction on
/// `AtlasError::Full` belongs to the residency cache (`assets.md` §A5);
/// this atlas never evicts, so a full atlas is an error.
#[derive(Debug, Clone)]
pub struct GpuAtlas {
    atlas: Atlas,
    slots: Vec<AtlasSlot>,
}

impl GpuAtlas {
    pub fn new(max_pages: u32) -> Result<Self, ViewError> {
        Ok(GpuAtlas {
            atlas: Atlas::new(max_pages)?,
            slots: Vec::new(),
        })
    }

    /// Packs the frames `store` gained since the last call, in id order.
    pub fn ensure(&mut self, store: &FrameStore) -> Result<(), ViewError> {
        let held = self.slots.len();
        if held > store.len() {
            return Err(ViewError::AtlasAhead {
                atlas: held,
                store: store.len(),
            });
        }
        if held < store.len() {
            let slots = self.atlas.insert_set(&store.frames()[held..])?;
            self.slots.extend(slots);
        }
        Ok(())
    }

    pub fn atlas(&self) -> &Atlas {
        &self.atlas
    }

    /// Frames packed so far (frames are only ever added: the pages'
    /// version).
    pub fn frames(&self) -> usize {
        self.slots.len()
    }

    /// The slot of each [`FrameId`] (`slots[n]` for `FrameId(n)`).
    pub fn slots(&self) -> &[AtlasSlot] {
        &self.slots
    }

    /// Packs `frame` for the compute compositor (§A9): bins, items, slots,
    /// map rows. Call [`GpuAtlas::ensure`] first.
    pub fn pack(
        &self,
        frame: &WorldFrame,
        assets: &ViewAssets,
    ) -> Result<gpu_compositor::Packed, ViewError> {
        let bins = scene::bin(&frame.items, &assets.frames, &assets.maps, VIEW)?;
        Ok(gpu_compositor::pack(
            &frame.items,
            &bins,
            &assets.frames,
            self.slots(),
            &assets.maps,
            self.atlas.pages().len() as u32,
        )?)
    }

    /// GPU image of a built frame (§A9): equal to [`compose_cpu`] by
    /// design (`d2-client verify` proves it on the synthetic cases).
    pub fn compose(
        &mut self,
        gpu: &Gpu,
        frame: &WorldFrame,
        assets: &ViewAssets,
    ) -> Result<Vec<u8>, ViewError> {
        self.ensure(&assets.frames)?;
        let packed = self.pack(frame, assets)?;
        Ok(gpu
            .compose_rgba(&packed, self.atlas.pages(), &assets.palette)?
            .1)
    }

    /// [`GpuAtlas::pack`] as one frame of `cycle` (`composition.md` §3):
    /// the packed frame starts from `cycle.pixels()` with the clears of
    /// `cycle.plan(blank_screen)`, returned beside the packed buffers.
    /// Hand the composed indices to `cycle.commit(plan, indices)`.
    pub fn pack_cycle(
        &self,
        cycle: &FrameCycle,
        blank_screen: bool,
        frame: &WorldFrame,
        assets: &ViewAssets,
    ) -> Result<(gpu_compositor::Packed, scene::FramePlan), ViewError> {
        check_cycle(cycle)?;
        let plan = cycle.plan(blank_screen);
        let packed = self.pack(frame, assets)?.with_frame(cycle.pixels(), plan)?;
        Ok((packed, plan))
    }

    /// GPU image of one frame of `cycle`: [`GpuAtlas::pack_cycle`],
    /// composed, the indices read back and committed to `cycle`. Equal to
    /// [`compose_cycle_cpu`] by design. On error the cycle is unchanged.
    pub fn compose_cycle(
        &mut self,
        gpu: &Gpu,
        cycle: &mut FrameCycle,
        blank_screen: bool,
        frame: &WorldFrame,
        assets: &ViewAssets,
    ) -> Result<Vec<u8>, ViewError> {
        self.ensure(&assets.frames)?;
        let (packed, plan) = self.pack_cycle(cycle, blank_screen, frame, assets)?;
        let (indices, rgba) = gpu.compose_rgba(&packed, self.atlas.pages(), &assets.palette)?;
        cycle.commit(plan, indices)?;
        Ok(rgba)
    }
}
