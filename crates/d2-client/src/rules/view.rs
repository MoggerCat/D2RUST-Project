// Spec: specs/render/camera.md (§4–§7, §10), specs/render/sprite-placement.md (§5, §7, §8), specs/render/draw-order.md (§5, §10)
//! [`OriginalView`]: the world view's placement hooks answered by the
//! original's rules. `tiles` places map tiles (camera §6, culled by §7,
//! DT1 images by placement §7/§8), `unit_params` sets the frame clip
//! (camera §10) and `place` puts each component cel (camera §4, placement
//! §8). Every other question (which tiles and units, poses, component
//! frames, draw keys, shading, blend, UI) belongs to other owner specs and
//! goes to the wrapped rules `R` or the [`ViewSource`].

use d2_formats::dt1::Dt1Tile;

use crate::bridge::world::ClientWorld;
use crate::bridge::ClientUnit;
use crate::composite::{ComponentFrame, ComponentRequest, CompositeError, UnitParams};
use crate::frames::{FrameAnchor, IndexFrame};
use crate::scene::{BlendOp, DrawKey, Rect, ShadeChain};
use crate::ui::{ImageRequest, TextRequest};
use crate::world_view::{TileDraw, UiRules, UiSprite, UnitPose, ViewAssets, ViewError, ViewRules};

use super::camera::{Camera, TileList, UnitPosition};
use super::draw_order::UnitSlot;
use super::placement;

/// One DT1 block's rectangle in tile coordinates (`b.x`, `b.y`, size),
/// for the per-block wall culling of camera §7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl BlockRect {
    /// Every block of a DT1 tile, in file order.
    pub fn of_tile(tile: &Dt1Tile) -> Vec<BlockRect> {
        tile.blocks
            .iter()
            .map(|b| {
                let (w, h) = b.size();
                BlockRect {
                    x: i32::from(b.x),
                    y: i32::from(b.y),
                    width: w as u32,
                    height: h as u32,
                }
            })
            .collect()
    }
}

/// One map tile to draw, as the map and draw-order owners state it: the
/// cell, its list (camera §6), its assembled DT1 image, its blocks, and
/// the draw answers owned elsewhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapTile {
    /// Absolute tile cell `(tx, ty)`.
    pub cell: (i32, i32),
    pub list: TileList,
    /// The DT1 tile frame set (`FramePart::Tile`) and frame 0.
    pub frame: ComponentFrame,
    /// The tile's blocks (`BlockRect::of_tile`), for wall culling.
    pub blocks: Vec<BlockRect>,
    /// TODO(spec: render/shading.md, render/lighting.md).
    pub shade: ShadeChain,
    /// TODO(spec: render/blend-modes.md) (roof fade, translucent walls).
    pub blend: BlendOp,
    /// `draw-order.md` §10 (`rules::draw_order::source`).
    pub key: DrawKey,
}

/// The inputs placement needs that the client world model does not hold
/// yet. Each method is answered by its owner spec once written; until then
/// an implementation returns an error, never a default.
pub trait ViewSource {
    /// TODO(spec: the S→C owner specs of unit positions; units path spec):
    /// the unit's position as the client keeps it (camera §2).
    fn unit_position(&self, unit: &ClientUnit) -> Result<UnitPosition, String>;

    /// TODO(spec: render/unit-composite.md) (camera §4, open question 3):
    /// the per-unit extra offsets `(ox, oy)`.
    fn unit_offset(&self, unit: &ClientUnit, pose: &UnitPose) -> Result<(i32, i32), String>;

    /// The tiles of the frame, each with its list and draw key. A feed
    /// with the §9 map-tile feed of `draw-order.md` is wrapped in
    /// `draw_order::source::OrderedSource`, which answers this; a source
    /// without one states its tiles directly.
    fn map_tiles(
        &self,
        world: &ClientWorld,
        assets: &ViewAssets,
    ) -> Result<Vec<MapTile>, ViewError>;

    /// The unit's slot in the frame's draw order (`draw-order.md` §3 r4,
    /// §5, §10). [`UnitSlot::Unordered`] (the default) leaves the unit and
    /// its keys to the wrapped rules.
    fn unit_slot(&self, _unit: &ClientUnit) -> UnitSlot {
        UnitSlot::Unordered
    }
}

/// The original's camera and placement over wrapped rules `R` (pose,
/// component frames, draw keys, shading, blend, UI) and a [`ViewSource`].
/// Build one per drawn frame: `camera` is that frame's (camera §3, once
/// per frame).
#[derive(Debug, Clone, Copy)]
pub struct OriginalView<'a, R: ?Sized, S: ?Sized> {
    pub camera: Camera,
    pub rules: &'a R,
    pub source: &'a S,
}

const CAMERA: &str = "render/camera.md";
const PLACEMENT: &str = "render/sprite-placement.md";

fn unresolved(what: &'static str, spec: &'static str, message: impl Into<String>) -> ViewError {
    ViewError::Unresolved {
        what,
        spec,
        message: message.into(),
    }
}

fn component_error(
    req: &ComponentRequest<'_>,
    what: &'static str,
    message: String,
) -> CompositeError {
    CompositeError::Unresolved {
        slot: req.slot.slot,
        component: req.slot.component,
        what,
        message,
    }
}

impl<'a, R: ?Sized, S: ?Sized> OriginalView<'a, R, S> {
    pub fn new(camera: Camera, rules: &'a R, source: &'a S) -> Self {
        OriginalView {
            camera,
            rules,
            source,
        }
    }

    /// The draw of one map tile, or `None` when culled (camera §6, §7).
    pub fn tile(&self, tile: &MapTile, image: &IndexFrame) -> Result<Option<TileDraw>, ViewError> {
        if image.anchor != FrameAnchor::Top {
            return Err(unresolved(
                "tile placement",
                PLACEMENT,
                "a DT1 tile image must be top-anchored (§8)",
            ));
        }
        let cam = &self.camera;
        let frame = cam.size.rect();
        let handed = cam.tile_handed(tile.list, tile.cell.0, tile.cell.1);
        let origin = cam.block_origin(tile.list, handed);
        let clip = match tile.list {
            TileList::Floor | TileList::Roof { .. } => {
                if !cam.floor_roof_visible(handed) {
                    return Ok(None);
                }
                frame
            }
            TileList::Wall => match self.wall_clip(tile, origin, frame)? {
                Some(clip) => clip,
                None => return Ok(None),
            },
        };
        let placed = placement::place(image, origin.0, origin.1, clip);
        Ok(placed.clip.map(|clip| TileDraw {
            frame: tile.frame.clone(),
            x: placed.x,
            y: placed.y,
            clip,
            shade: tile.shade,
            blend: tile.blend,
            key: tile.key,
            cell: tile.cell,
        }))
    }

    /// Camera §7 wall block culling as a clip of the assembled image: the
    /// frame when every block is kept, `None` when none is, else the
    /// bounding box of the kept blocks, which must not touch a culled
    /// block (otherwise one clip cannot express the cull: an error).
    fn wall_clip(
        &self,
        tile: &MapTile,
        origin: (i32, i32),
        frame: Rect,
    ) -> Result<Option<Rect>, ViewError> {
        let screen = |b: &BlockRect| {
            let (x, y) = placement::block_pixel(origin, (b.x, b.y), (0, 0));
            Rect::new(x, y, b.width, b.height)
        };
        let (kept, culled): (Vec<Rect>, Vec<Rect>) = {
            let mut k = Vec::new();
            let mut c = Vec::new();
            for b in &tile.blocks {
                let r = screen(b);
                if self.camera.wall_block_visible(r.x, r.y) {
                    k.push(r);
                } else {
                    c.push(r);
                }
            }
            (k, c)
        };
        if culled.is_empty() {
            return Ok(Some(frame));
        }
        let Some(first) = kept.first() else {
            return Ok(None);
        };
        let (mut x0, mut y0) = (i64::from(first.x), i64::from(first.y));
        let (mut x1, mut y1) = (x0, y0);
        for r in &kept {
            x0 = x0.min(i64::from(r.x));
            y0 = y0.min(i64::from(r.y));
            x1 = x1.max(i64::from(r.x) + i64::from(r.width));
            y1 = y1.max(i64::from(r.y) + i64::from(r.height));
        }
        let bounds = Rect::new(x0 as i32, y0 as i32, (x1 - x0) as u32, (y1 - y0) as u32);
        if let Some(c) = culled.iter().find(|c| c.intersect(&bounds).is_some()) {
            return Err(unresolved(
                "wall block culling",
                CAMERA,
                format!(
                    "tile {:?}: culled block at ({}, {}) overlaps the kept blocks",
                    tile.cell, c.x, c.y
                ),
            ));
        }
        Ok(bounds.intersect(&frame))
    }
}

impl<R: ViewRules + ?Sized, S: ViewSource + ?Sized> ViewRules for OriginalView<'_, R, S> {
    fn tiles(&self, world: &ClientWorld, assets: &ViewAssets) -> Result<Vec<TileDraw>, ViewError> {
        let mut out = Vec::new();
        for (index, tile) in self.source.map_tiles(world, assets)?.iter().enumerate() {
            let image = assets
                .frame(&tile.frame.set, tile.frame.index)
                .map_err(|e| ViewError::Tile {
                    index,
                    error: Box::new(e),
                })?;
            if let Some(draw) = self.tile(tile, image).map_err(|e| ViewError::Tile {
                index,
                error: Box::new(e),
            })? {
                out.push(draw);
            }
        }
        Ok(out)
    }

    fn unit_pose(
        &self,
        world: &ClientWorld,
        unit: &ClientUnit,
    ) -> Result<Option<UnitPose>, ViewError> {
        if self.source.unit_slot(unit) == UnitSlot::NotDrawn {
            return Ok(None);
        }
        self.rules.unit_pose(world, unit)
    }

    /// Draw keys from the draw order (`draw-order.md` §10) when the
    /// source has one, else from the wrapped rules; the clip is the frame
    /// (camera §10: the play area is not a clip).
    fn unit_params(
        &self,
        world: &ClientWorld,
        unit: &ClientUnit,
        pose: &UnitPose,
    ) -> Result<UnitParams, ViewError> {
        let mut params = self.rules.unit_params(world, unit, pose)?;
        if let UnitSlot::Drawn(at) = self.source.unit_slot(unit) {
            params.pass = at.pass;
            params.major = at.major;
            params.minor = at.minor;
        }
        params.clip = self.camera.size.rect();
        Ok(params)
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

    /// Camera §4 then placement §8. A cel whose rasterizer rows differ from
    /// the unit's frame clip (a top-down DC6 cel crossing an edge, §4) is
    /// an error: the unit's components share one clip.
    fn place(
        &self,
        unit: &ClientUnit,
        pose: &UnitPose,
        req: &ComponentRequest<'_>,
        image: &IndexFrame,
    ) -> Result<(i32, i32), CompositeError> {
        let at = self
            .source
            .unit_position(unit)
            .map_err(|m| component_error(req, "unit position", m))?
            .client();
        let extra = self
            .source
            .unit_offset(unit, pose)
            .map_err(|m| component_error(req, "unit offset", m))?;
        let (x, y) = self.camera.unit_draw(at, extra);
        let frame = self.camera.size.rect();
        let placed = placement::place(image, x, y, frame);
        if !placed.same_as_frame(image.width, image.height, frame) {
            return Err(component_error(
                req,
                "placement",
                format!(
                    "top-down cel at ({x}, {y}) is cut by the rasterizer (TODO(spec: {PLACEMENT}) §4): \
                     a per-component clip is needed"
                ),
            ));
        }
        Ok((placed.x, placed.y))
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

impl<R: UiRules + ?Sized, S: ?Sized> UiRules for OriginalView<'_, R, S> {
    fn ui_image(&self, req: &ImageRequest, assets: &ViewAssets) -> Result<UiSprite, ViewError> {
        self.rules.ui_image(req, assets)
    }

    fn ui_text(&self, req: &TextRequest, assets: &ViewAssets) -> Result<Vec<UiSprite>, ViewError> {
        self.rules.ui_text(req, assets)
    }

    /// Pass 11: everything after the world draw (`draw-order.md` §10).
    fn ui_pass(&self) -> Result<u32, ViewError> {
        Ok(crate::scene::order::pass::UI)
    }
}
