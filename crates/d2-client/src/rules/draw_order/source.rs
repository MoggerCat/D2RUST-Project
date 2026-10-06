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
use d2_sim::rng::Seed;

use super::weather::{FloorContext, Weather};

use super::super::camera::{Camera, ClientPos, OpenMode, TileList, UnitPosition};
use super::super::view::{BlockRect, MapTile, ViewSource};
use super::{
    mark_drawn, order_frame, sets_drawn_flag, FrameOrder, Ordered, OrderedTile, TileArray,
    TileKind, UnitSlot, SPEC,
};

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

/// The weather state the floor pass writes (`draw-order-2.md` §11.5),
/// lent by the feed for one frame: the weather pools, the floor pass's
/// persistent context, the local player's seed (`unit +0x20`), the client
/// update count and the level's `Mud`.
#[derive(Debug)]
pub struct WeatherFrame<'a> {
    pub weather: &'a mut Weather,
    pub floors: &'a mut FloorContext,
    pub seed: &'a mut Seed,
    pub update_count: u32,
    pub mud: bool,
}

impl WeatherFrame<'_> {
    /// The floor pass's water effects (§11.5): the frame's context, then
    /// per drawn water floor in draw order, at its handed (X, Y)
    /// (`camera.md` §6 floors), one seed draw and the gated spawns.
    pub fn floor_pass(&mut self, water: &[(i32, i32)]) {
        self.floors
            .begin_frame(self.update_count, self.weather, self.mud);
        for &(x, y) in water {
            self.weather.water_floor(self.floors, x, y, self.seed);
        }
    }
}

impl WeatherFrame<'_> {
    /// Passes 4 and 9 (`draw-order-2.md` §11.6, §11.7) have no art path in
    /// the world view yet (overlay cels, lines, the flash rectangle): a
    /// frame with live pools or a running lightning fails rather than
    /// dropping them.
    pub fn unwired_passes(&self) -> Result<(), ViewError> {
        let w = &*self.weather;
        let live = [
            ("splash", w.splashes().live()),
            ("bubble", w.bubbles().live()),
            ("particle", w.particles().live()),
        ];
        if let Some((what, n)) = live.iter().find(|(_, n)| *n != 0) {
            return Err(open(
                "environment pools",
                format!("{n} live {what} record(s): passes 4 / 9 are not wired to the view"),
            ));
        }
        if w.lightning_on {
            return Err(open(
                "lightning",
                "lightning is on: pass 9 is not wired to the view".into(),
            ));
        }
        Ok(())
    }
}

/// What [`resolve_drawn`] gives besides the tiles and unit slots: the
/// records whose draw sets flag 0x20000 (§6 r6) and the handed (X, Y) of
/// every drawn water floor, in draw order (`draw-order-2.md` §11.5).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DrawEffects {
    pub drawn: Vec<(usize, TileArray, usize)>,
    pub water: Vec<(i32, i32)>,
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
    let (tiles, units, effects) = {
        let feed: &F = feed;
        resolve_drawn(camera, &order, |t| feed.tile_art(t, assets))?
    };
    // `draw-order-2.md` §11.5: the floor pass's water effects.
    match feed.weather_frame(world)? {
        Some(mut w) => {
            w.floor_pass(&effects.water);
            w.unwired_passes()?;
        }
        None if effects.water.is_empty() => {}
        None => {
            return Err(open(
                "water floor",
                format!(
                    "floor at {:?} has material 0x2 and the feed lends no weather state \
                     (draw-order-2.md §11.5: one player-seed draw per drawn water floor)",
                    effects.water[0]
                ),
            ))
        }
    }
    // §6 r6: flag 0x20000 after the draws.
    let near = feed
        .near_rooms(world)?
        .ok_or_else(|| open("draw order", "the near rooms vanished mid-frame".into()))?;
    mark_drawn(near, &effects.drawn);
    let feed: &'a F = feed;
    Ok(Some(OrderedSource {
        source: feed,
        tiles,
        units,
    }))
}

/// The map tiles and unit slots of an order. Fails on the items whose
/// draw is not wired yet: unit shadows (`render/blend-modes.md` §5).
pub fn resolve(
    camera: &Camera,
    order: &FrameOrder,
    art: impl Fn(&OrderedTile) -> Result<TileArt, ViewError>,
) -> Result<(Vec<MapTile>, BTreeMap<UnitKey, UnitSlot>), ViewError> {
    resolve_drawn(camera, order, art).map(|(t, u, _)| (t, u))
}

/// [`resolve`], plus the frame's [`DrawEffects`].
pub fn resolve_drawn(
    camera: &Camera,
    order: &FrameOrder,
    art: impl Fn(&OrderedTile) -> Result<TileArt, ViewError>,
) -> Result<(Vec<MapTile>, BTreeMap<UnitKey, UnitSlot>, DrawEffects), ViewError> {
    let mut tiles = Vec::new();
    let mut units = BTreeMap::new();
    let mut fx = DrawEffects::default();
    for item in &order.items {
        match item {
            Ordered::Tile(t) => {
                let list = placement_list(t);
                let a = art(t)?;
                if sets_drawn_flag(camera, t, &a.blocks) {
                    fx.drawn.push((t.room, t.array, t.record));
                    // A drawn floor (whole-tile test passed) with water.
                    if let TileKind::Floor { .. } = t.kind {
                        if t.dt1.material & 0x2 != 0 {
                            fx.water.push(camera.tile_handed(list, t.cell.0, t.cell.1));
                        }
                    }
                }
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
    Ok((tiles, units, fx))
}
