// Spec: specs/render/draw-order.md (§9, §10)
//! The draw order wired into the world view: [`ordered_source`] reads the
//! §9 map-tile feed of a [`ViewFeed`], orders the frame, turns each tile
//! item into a [`MapTile`] (its art through [`ViewFeed::tile_art`]) and
//! wraps the feed in an [`OrderedSource`], the [`ViewSource`] whose
//! `map_tiles` are the ordered tiles and whose `unit_slot` gives each
//! unit's draw key (or not drawn).

use std::collections::BTreeMap;

use crate::bridge::world::{ClientWorld, UnitKey};
use crate::bridge::ClientUnit;
use crate::composite::ComponentFrame;
use crate::scene::{BlendOp, DrawKey, ShadeChain};
use crate::world_view::{UnitPose, ViewAssets, ViewError, ViewFeed};

use super::super::camera::{Camera, ClientPos, OpenMode, TileList, UnitPosition};
use super::super::view::{BlockRect, MapTile, ViewSource};
use super::{order_frame, FrameOrder, Ordered, OrderedTile, TileKind, UnitSlot, SPEC};

/// The art of one ordered tile, answered by its owner specs: the DT1
/// frame (open question 12: the room's tile library maps the record to a
/// file and index), its blocks, shading / lighting and blend (alpha byte
/// and draw mode 4 of shadow tiles: `render/blend-modes.md`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileArt {
    pub frame: ComponentFrame,
    pub blocks: Vec<BlockRect>,
    pub shade: ShadeChain,
    pub blend: BlendOp,
}

/// The camera list a tile kind is placed with (§10: lower walls and
/// shadow tiles are placed as walls; camera §6).
pub fn placement_list(tile: &OrderedTile) -> TileList {
    match tile.kind {
        TileKind::Floor { .. } => TileList::Floor,
        TileKind::Roof { .. } => TileList::Roof {
            roof_height: tile.dt1.roof_height,
        },
        TileKind::LowerWall | TileKind::ShadowTile | TileKind::Wall => TileList::Wall,
    }
}

fn order_error(e: super::OrderError) -> ViewError {
    ViewError::Unresolved {
        what: "draw order",
        spec: SPEC,
        message: e.to_string(),
    }
}

fn open(what: &'static str, message: String) -> ViewError {
    ViewError::Unresolved {
        what,
        spec: SPEC,
        message,
    }
}

/// A [`ViewSource`] over a feed with the frame's draw order.
#[derive(Debug, Clone)]
pub struct OrderedSource<'a, S: ?Sized> {
    pub source: &'a S,
    pub tiles: Vec<MapTile>,
    pub units: BTreeMap<UnitKey, UnitSlot>,
}

impl<S: ViewSource + ?Sized> ViewSource for OrderedSource<'_, S> {
    fn unit_position(&self, unit: &ClientUnit) -> Result<UnitPosition, String> {
        self.source.unit_position(unit)
    }

    fn unit_offset(&self, unit: &ClientUnit, pose: &UnitPose) -> Result<(i32, i32), String> {
        self.source.unit_offset(unit, pose)
    }

    /// The ordered tiles (§3, §6 tests kept), keys filled (§10).
    fn map_tiles(&self, _: &ClientWorld, _: &ViewAssets) -> Result<Vec<MapTile>, ViewError> {
        Ok(self.tiles.clone())
    }

    fn tile_blocks(&self, tile: &MapTile) -> Result<Vec<super::super::view::BlockShade>, ViewError> {
        self.source.tile_blocks(tile)
    }

    /// Drawn units with their key; every other unit is not drawn.
    fn unit_slot(&self, unit: &ClientUnit) -> UnitSlot {
        self.units
            .get(&unit.key)
            .copied()
            .unwrap_or(UnitSlot::NotDrawn)
    }
}

/// Orders the frame from the feed's near rooms (§9) and wraps the feed;
/// `None` when the feed states no map (the feed answers alone).
pub fn ordered_source<'a, F: ViewFeed + ?Sized>(
    world: &ClientWorld,
    camera: &Camera,
    mode: OpenMode,
    feed: &'a mut F,
    assets: &ViewAssets,
) -> Result<Option<OrderedSource<'a, F>>, ViewError> {
    let keys: Vec<(usize, UnitKey)> = match feed.near_rooms(world)? {
        None => return Ok(None),
        Some(near) => near
            .rooms
            .iter()
            .enumerate()
            .flat_map(|(ri, r)| r.units.iter().map(move |u| (ri, u.key)))
            .collect(),
    };
    let mut positions: BTreeMap<UnitKey, ClientPos> = BTreeMap::new();
    for (room, key) in keys {
        let unit = world.units.get(&key).ok_or_else(|| {
            open(
                "draw order",
                format!(
                    "room {room} lists unit ({}, {}), which the client world does not hold",
                    key.unit_type, key.guid
                ),
            )
        })?;
        let at = feed
            .unit_position(unit)
            .map_err(|m| ViewError::Unresolved {
                what: "unit position",
                spec: "render/camera.md",
                message: m,
            })?;
        positions.insert(key, at.client());
    }
    let clock = feed.fade_clock(world)?;
    let order = {
        let near = feed
            .near_rooms(world)?
            .ok_or_else(|| open("draw order", "the near rooms vanished mid-frame".into()))?;
        order_frame(camera, mode, near, &positions, clock).map_err(order_error)?
    };
    let feed: &'a F = feed;
    let (tiles, units) = resolve(camera, &order, |t| feed.tile_art(t, assets))?;
    Ok(Some(OrderedSource {
        source: feed,
        tiles,
        units,
    }))
}

/// The map tiles and unit slots of an order. Fails on the items whose
/// draw no spec states yet: unit shadows (open question 3) and drawn
/// water floors (open question 11: effects and an RNG draw follow).
pub fn resolve(
    camera: &Camera,
    order: &FrameOrder,
    art: impl Fn(&OrderedTile) -> Result<TileArt, ViewError>,
) -> Result<(Vec<MapTile>, BTreeMap<UnitKey, UnitSlot>), ViewError> {
    let mut tiles = Vec::new();
    let mut units = BTreeMap::new();
    for item in &order.items {
        match item {
            Ordered::Tile(t) => {
                let list = placement_list(t);
                if let TileKind::Floor { .. } = t.kind {
                    let handed = camera.tile_handed(list, t.cell.0, t.cell.1);
                    if t.dt1.material & 0x2 != 0 && camera.floor_roof_visible(handed) {
                        return Err(open(
                            "water floor",
                            format!(
                                "open question 11: floor {:?} has material 0x2 (water effects)",
                                t.cell
                            ),
                        ));
                    }
                }
                let a = art(t)?;
                tiles.push(MapTile {
                    cell: t.cell,
                    list,
                    frame: a.frame,
                    blocks: a.blocks,
                    shade: a.shade,
                    blend: a.blend,
                    key: DrawKey::new(t.key.pass, t.key.major, t.key.minor, 0)?,
                });
            }
            Ordered::Unit { key, at } => {
                units.insert(*key, UnitSlot::Drawn(*at));
            }
            Ordered::UnitShadow { key, .. } => {
                return Err(open(
                    "unit shadow",
                    format!(
                        "open question 3: unit ({}, {}) has a shadow entry (0x00471620)",
                        key.unit_type, key.guid
                    ),
                ));
            }
        }
    }
    Ok((tiles, units))
}
