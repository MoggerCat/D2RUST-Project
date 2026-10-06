// Spec: specs/client/render-pipeline.md (A1 stages 1–4, A7), specs/client/bridge.md (§5, §7 rule 4), specs/client/ui.md (A2)
//! World view: the client's model ([`ClientWorld`]) turned into the scene
//! draw list each frame (render-pipeline §A1 stage 1), then ordered
//! (stage 2) and composed (stage 4a CPU, 4b GPU). Plain Rust, no Bevy
//! types outside [`present`].
//!
//! The mechanism is ours: units go through the C7 COF composite
//! ([`composite::build`]) in unit-key order, frames come from the resident
//! C3 frame sets ([`ViewAssets::sets`]) and get scene [`FrameId`]s in
//! first-use order ([`FrameTable`]), UI requests of the C8 core become
//! items in emission order ([`ui_bind`]), and the list is sorted by the C4
//! key ([`scene::order`], stable). Every rule of the original (which COF
//! and frame a unit shows, where a sprite goes, draw keys, shading, blend,
//! map tiles, UI art and text layout) is a method of [`ViewRules`] or
//! [`ui_bind::UiRules`], each a `TODO(spec: …)` hook. [`Unspecified`]
//! answers them with the narrowest neutral behavior: nothing the model
//! does not state is drawn, and anything that would need a rule to draw
//! is an error, never a default.
//!
//! The client decides no outcome here (CLAUDE.md rule 7): this module only
//! reads the model.

pub mod present;
pub mod ui_bind;

#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::collections::BTreeMap;

use d2_formats::cof::Cof;
use d2_formats::palette::Palette;

use crate::assets::path::CanonicalPath;
use crate::bridge::world::ClientWorld;
use crate::bridge::ClientUnit;
use crate::composite::{self, ComponentFrame, ComponentRequest, CompositeError, UnitParams};
use crate::frames::{Atlas, AtlasError, AtlasSlot, FrameSet, FrameSetKey, IndexFrame};
use crate::gpu_compositor::{self, Gpu, GpuError, SlotSource};
use crate::scene::{
    self, BlendOp, DrawItem, DrawKey, FrameId, FrameSource, FrameView, ItemTag, MapTable, Rect,
    SceneError, ShadeChain,
};

pub use present::{WorldViewGpu, WorldViewPlugin, WorldViewState, WorldViewUi};
pub use ui_bind::{UiQueue, UiRules, UiSprite};

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
    #[error("frame set {0:?} is not resident")]
    SetMissing(FrameSetKey),
    #[error("frame {index} of {key:?} does not exist ({count} frames)")]
    FrameIndex {
        key: FrameSetKey,
        index: usize,
        count: usize,
    },
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
            message: "no rule until the owner spec exists".into(),
        }
    }
}

/// Everything a frame reads besides the model: parsed COFs, resident frame
/// sets (residency is `client/assets.md` §A4: a set missing here is an
/// error, never a skipped draw), the map table (PL2 rows, blend tables)
/// and the frame palette.
#[derive(Debug, Clone)]
pub struct ViewAssets {
    pub cofs: BTreeMap<CanonicalPath, Cof>,
    pub sets: BTreeMap<FrameSetKey, FrameSet>,
    pub maps: MapTable,
    /// TODO(spec: render/shading.md) (§B3): one palette per frame until
    /// palettes per screen region are specified.
    pub palette: Palette,
}

impl ViewAssets {
    /// No COFs or frames, an empty map table.
    pub fn new(palette: Palette) -> Self {
        ViewAssets {
            cofs: BTreeMap::new(),
            sets: BTreeMap::new(),
            maps: MapTable::new(),
            palette,
        }
    }

    /// Frame `index` of the resident set `key`.
    pub fn frame(&self, key: &FrameSetKey, index: usize) -> Result<&IndexFrame, ViewError> {
        let set = self
            .sets
            .get(key)
            .ok_or_else(|| ViewError::SetMissing(key.clone()))?;
        set.frames.get(index).ok_or_else(|| ViewError::FrameIndex {
            key: key.clone(),
            index,
            count: set.frames.len(),
        })
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
/// question. Each method is a `TODO(spec: …)` hook named on it.
pub trait ViewRules {
    /// TODO(spec: render/draw-order.md, render/camera.md, DRLG) (§B6, §B7,
    /// §B10): the map tiles to draw and how. The client world model holds
    /// no map yet (`bridge.md` §5: no S→C message has an owner spec), so
    /// every answer comes from the rule.
    fn tiles(&self, world: &ClientWorld, assets: &ViewAssets) -> Result<Vec<TileDraw>, ViewError>;

    /// TODO(spec: render/unit-composite.md) (§B4) and the owner specs of
    /// the S→C messages that state a unit's mode, direction and frame:
    /// which COF, COF direction and frame the unit shows. `None` = the
    /// unit is not drawn.
    fn unit_pose(
        &self,
        world: &ClientWorld,
        unit: &ClientUnit,
    ) -> Result<Option<UnitPose>, ViewError>;

    /// TODO(spec: render/draw-order.md, render/camera.md) (§B6, §B7):
    /// pass/major/minor and clip shared by the unit's components.
    fn unit_params(
        &self,
        world: &ClientWorld,
        unit: &ClientUnit,
        pose: &UnitPose,
    ) -> Result<UnitParams, ViewError>;

    /// TODO(spec: render/unit-composite.md) (§B4): the component's frame
    /// set (file path from token, variant, mode, weapon class; file
    /// direction) and frame.
    fn component_frame(
        &self,
        unit: &ClientUnit,
        pose: &UnitPose,
        req: &ComponentRequest<'_>,
    ) -> Result<ComponentFrame, CompositeError>;

    /// TODO(spec: render/sprite-placement.md, render/camera.md) (§B1,
    /// §B7): screen top-left of the component image from the unit's
    /// position and the frame's own offsets (`image.x_off/y_off`).
    fn place(
        &self,
        unit: &ClientUnit,
        pose: &UnitPose,
        req: &ComponentRequest<'_>,
        image: &IndexFrame,
    ) -> Result<(i32, i32), CompositeError>;

    /// TODO(spec: render/shading.md, render/unit-composite.md,
    /// render/lighting.md) (§B3, §B4, §B8).
    fn shade(
        &self,
        unit: &ClientUnit,
        req: &ComponentRequest<'_>,
    ) -> Result<ShadeChain, CompositeError>;

    /// TODO(spec: render/blend-modes.md) (§B5).
    fn blend(
        &self,
        unit: &ClientUnit,
        req: &ComponentRequest<'_>,
    ) -> Result<BlendOp, CompositeError>;
}

/// The neutral rules until the owner specs exist: no map tiles, no unit
/// drawn (the model states nothing a unit looks like), and every hook that
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
        message: "no rule until the owner spec exists".into(),
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

/// Scene frame ids of one draw list: `FrameId(n)` is the n-th distinct
/// (frame set, frame) the build asked for, in build order, so numbering is
/// deterministic and independent of what else is resident.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FrameTable {
    refs: Vec<(FrameSetKey, usize)>,
    ids: BTreeMap<(FrameSetKey, usize), FrameId>,
}

impl FrameTable {
    /// The id of `(key, index)`, assigning the next one on first use.
    pub fn id(&mut self, key: &FrameSetKey, index: usize) -> FrameId {
        if let Some(&id) = self.ids.get(&(key.clone(), index)) {
            return id;
        }
        let id = FrameId(self.refs.len() as u32);
        self.refs.push((key.clone(), index));
        self.ids.insert((key.clone(), index), id);
        id
    }

    /// What `id` stands for.
    pub fn get(&self, id: FrameId) -> Option<(&FrameSetKey, usize)> {
        self.refs.get(id.0 as usize).map(|(k, i)| (k, *i))
    }

    /// Every referenced (set, frame), in id order.
    pub fn refs(&self) -> &[(FrameSetKey, usize)] {
        &self.refs
    }

    pub fn len(&self) -> usize {
        self.refs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.refs.is_empty()
    }

    /// The table as a scene frame source over `assets`' resident sets.
    pub fn bind<'a>(&'a self, assets: &'a ViewAssets) -> BoundFrames<'a> {
        BoundFrames {
            table: self,
            assets,
        }
    }
}

/// A [`FrameTable`] reading pixels from resident frame sets: the frame
/// source of the CPU compositor (§A8).
#[derive(Debug, Clone, Copy)]
pub struct BoundFrames<'a> {
    table: &'a FrameTable,
    assets: &'a ViewAssets,
}

impl FrameSource for BoundFrames<'_> {
    fn frame(&self, id: FrameId) -> Result<FrameView<'_>, SceneError> {
        let (key, index) = self.table.get(id).ok_or(SceneError::FrameMissing(id))?;
        let f = self
            .assets
            .frame(key, index)
            .map_err(|_| SceneError::FrameMissing(id))?;
        FrameView::new(f.width, f.height, &f.pixels)
    }
}

/// One built frame: the ordered draw list and the frames it references.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorldFrame {
    /// Sorted by [`DrawKey`] (stable: equal keys keep build order).
    pub items: Vec<DrawItem>,
    pub frames: FrameTable,
    /// Units drawn, and units the rules left undrawn (`unit_pose` = None).
    pub units_drawn: usize,
    pub units_hidden: usize,
}

/// The C7 resolver of one unit: the hooks plus frame id assignment.
struct UnitResolver<'a, R: ?Sized> {
    rules: &'a R,
    unit: &'a ClientUnit,
    pose: &'a UnitPose,
    assets: &'a ViewAssets,
    table: &'a RefCell<FrameTable>,
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

    fn frame_id(
        &self,
        req: &ComponentRequest<'_>,
        frame: &ComponentFrame,
    ) -> Result<FrameId, CompositeError> {
        self.assets
            .frame(&frame.set, frame.index)
            .map_err(|e| not_resident(req, "frame_id", e))?;
        Ok(self.table.borrow_mut().id(&frame.set, frame.index))
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
/// emission order; then the stable sort by key. Frame ids are assigned in
/// that build order. Any error fails the frame.
pub fn build<R: ViewRules + UiRules + ?Sized>(
    world: &ClientWorld,
    ui: &[crate::ui::UiDraw],
    rules: &R,
    assets: &ViewAssets,
) -> Result<WorldFrame, ViewError> {
    let table = RefCell::new(FrameTable::default());
    let mut items = Vec::new();

    for (index, t) in rules.tiles(world, assets)?.into_iter().enumerate() {
        let at = |error| ViewError::Tile {
            index,
            error: Box::new(error),
        };
        assets.frame(&t.frame.set, t.frame.index).map_err(at)?;
        let id = table.borrow_mut().id(&t.frame.set, t.frame.index);
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
            table: &table,
        };
        let draws =
            composite::build(cof, pose.dir, pose.frame, &params, &resolver).map_err(|error| {
                ViewError::Unit {
                    unit_type: unit.key.unit_type,
                    guid: unit.key.guid,
                    error,
                }
            })?;
        items.extend(draws.into_iter().map(|d| d.item));
        units_drawn += 1;
    }

    ui_bind::ui_items(ui, rules, assets, &mut table.borrow_mut(), &mut items)?;

    scene::order(&mut items);
    Ok(WorldFrame {
        items,
        frames: table.into_inner(),
        units_drawn,
        units_hidden,
    })
}

/// CPU reference image of a built frame (§A8): RGBA8, alpha 255,
/// `VIEW.width × VIEW.height`.
pub fn compose_cpu(frame: &WorldFrame, assets: &ViewAssets) -> Result<Vec<u8>, ViewError> {
    Ok(scene::compose_rgba(
        &frame.items,
        &frame.frames.bind(assets),
        &assets.maps,
        &assets.palette,
        VIEW,
    )?)
}

/// The atlas the GPU compositor reads, filled with whole frame sets on
/// first use (C3 packer). TODO(spec: none, design C2): page eviction on
/// `AtlasError::Full` is the residency cache's (`assets.md` §A5); until it
/// is wired, a full atlas is an error.
#[derive(Debug, Clone)]
pub struct GpuAtlas {
    atlas: Atlas,
    slots: BTreeMap<FrameSetKey, Vec<AtlasSlot>>,
}

impl GpuAtlas {
    pub fn new(max_pages: u32) -> Result<Self, ViewError> {
        Ok(GpuAtlas {
            atlas: Atlas::new(max_pages)?,
            slots: BTreeMap::new(),
        })
    }

    /// Inserts every set `frame` references that is not in the atlas yet,
    /// in frame-id order (deterministic packing).
    pub fn ensure(&mut self, frame: &WorldFrame, assets: &ViewAssets) -> Result<(), ViewError> {
        for (key, _) in frame.frames.refs() {
            if self.slots.contains_key(key) {
                continue;
            }
            let set = assets
                .sets
                .get(key)
                .ok_or_else(|| ViewError::SetMissing(key.clone()))?;
            let slots = self.atlas.insert_set(&set.frames)?;
            self.slots.insert(key.clone(), slots);
        }
        Ok(())
    }

    pub fn atlas(&self) -> &Atlas {
        &self.atlas
    }

    /// The slot source of `frame`'s ids.
    pub fn slots<'a>(&'a self, frame: &'a WorldFrame) -> FrameSlots<'a> {
        FrameSlots { atlas: self, frame }
    }

    /// Packs `frame` for the compute compositor (§A9): bins, items, slots,
    /// map rows. Call [`GpuAtlas::ensure`] first.
    pub fn pack(
        &self,
        frame: &WorldFrame,
        assets: &ViewAssets,
    ) -> Result<gpu_compositor::Packed, ViewError> {
        let frames = frame.frames.bind(assets);
        let bins = scene::bin(&frame.items, &frames, &assets.maps, VIEW)?;
        Ok(gpu_compositor::pack(
            &frame.items,
            &bins,
            &frames,
            &self.slots(frame),
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
        self.ensure(frame, assets)?;
        let packed = self.pack(frame, assets)?;
        Ok(gpu
            .compose_rgba(&packed, self.atlas.pages(), &assets.palette)?
            .1)
    }
}

/// Atlas slot of each [`FrameId`] of one frame.
#[derive(Debug, Clone, Copy)]
pub struct FrameSlots<'a> {
    atlas: &'a GpuAtlas,
    frame: &'a WorldFrame,
}

impl SlotSource for FrameSlots<'_> {
    fn slot(&self, id: FrameId) -> Option<AtlasSlot> {
        let (key, index) = self.frame.frames.get(id)?;
        self.atlas.slots.get(key)?.get(index).copied()
    }
}
