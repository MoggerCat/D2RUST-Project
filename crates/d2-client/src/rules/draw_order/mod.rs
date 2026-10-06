// Spec: specs/render/draw-order.md
//! The draw order of one frame: the draw-cell grid (§2), filling it from
//! the near rooms (§3, §4), which units draw (§5), the world passes (§6,
//! §7), the wall fade targets (§8) and the `DrawKey` fields of each item
//! (§10). Plain Rust, integer math; positions are `camera.md`'s, blend,
//! shading and lighting are hooks of their owner specs ([`source`]).
//!
//! The inputs are the §9 map-tile feed ([`NearRooms`]: the local player's
//! active room and its near-room array, each room's tile origin, record
//! arrays and unit list), the frame's camera and open mode, the units'
//! client positions and the fade clock. [`order_frame`] writes back what
//! the original writes during a frame (record flags 0x400 / 0x8 / 0x20000
//! and fade bytes, unit flags 0x10000000 and flag-ex 0x80) and returns the
//! items in draw order, each with its pass, major and minor.

pub mod background;
pub mod edges;
pub mod sight;
pub mod source;
pub mod weather;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use crate::bridge::world::UnitKey;
use crate::scene::order::pass;

use super::camera::{tile_entry, Camera, ClientPos, OpenMode};
use super::view::BlockRect;

const SPEC: &str = "render/draw-order.md";

/// Entry pool of a frame (§2, view +0x3C).
pub const POOL: usize = 3_000;
/// Grid margins (§2).
pub const MARGIN_A: i32 = 11;
pub const MARGIN_N: i32 = 11;
pub const MARGIN_X: i32 = 3;
/// Room slack of the room test (§3 r1).
pub const ROOM_SLACK_BOTTOM: i32 = 400;
pub const ROOM_SLACK_LEFT: i32 = 200;

/// Tile record flags (§3, §7, §8; `drlg/rooms.md` §9.5.1).
pub const REC_NO_FADE: u32 = 0x4;
pub const REC_HIDDEN: u32 = 0x8;
pub const REC_FADED_OUT: u32 = 0x400;
pub const REC_DRAWN: u32 = 0x2_0000;
pub const REC_LAYER_BITS: u32 = 0x1_C000;
const REC_SKIP: u32 = REC_HIDDEN | REC_FADED_OUT;

/// Unit flags (§3 r4, §5).
pub const UNIT_FLAT: u32 = 0x10_0000;
pub const UNIT_MISSILE_FLAT: u32 = 0x1_0000;
pub const UNIT_DRAWN: u32 = 0x1000_0000;
pub const UNIT_EX_VISIBLE: u32 = 0x80;
pub const UNIT_EX_SKIP: u32 = 1 << 18;

/// Unit types (`unit-composite.md`).
pub const PLAYER: u8 = 0;
pub const MONSTER: u8 = 1;
pub const OBJECT: u8 = 2;
pub const MISSILE: u8 = 3;
pub const ITEM: u8 = 4;

/// Errors of the draw order: the original's fatal checks and the open
/// questions of the spec a frame runs into (never skipped or defaulted).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OrderError {
    /// `0x00619C40` fatal: room left > right or top > bottom (§3 r1).
    #[error("room {room}: rectangle ({left}, {top})–({right}, {bottom}) is inverted (§3 r1)")]
    RoomRect {
        room: usize,
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    },
    /// Fatal 0xF9: a type-15 record in a wall list (§6 r4).
    #[error("room {room} wall record {record}: type 15 in a wall list (fatal 0xF9, §6 r4)")]
    RoofInWallList { room: usize, record: usize },
    /// A room unit without a client position.
    #[error("unit ({}, {}) of room {room} has no client position", .key.unit_type, .key.guid)]
    NoPosition { room: usize, key: UnitKey },
    /// An open question of the spec decides this frame.
    #[error("TODO(spec: {SPEC} open question {question}): {message}")]
    Open { question: u8, message: String },
}

fn open(question: u8, message: impl Into<String>) -> OrderError {
    OrderError::Open {
        question,
        message: message.into(),
    }
}

// ---------------------------------------------------------------- §2 grid

/// `q(v)` of §2: `v / 160`, minus one for negative `v`.
pub fn q(v: i32) -> i32 {
    if v >= 0 {
        v / 160
    } else {
        v / 160 - 1
    }
}

/// `T(x, y)` (`0x00643340`): the tile of a client pixel (§2).
pub fn tile_of(x: i32, y: i32) -> (i32, i32) {
    (q(x + 2 * y), q(2 * y - x))
}

/// A rectangle in client pixels, edges inclusive as the tests compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PixelRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// The draw-cell grid of a frame (§2): side `n`, tile origin `(x0, y0)`,
/// and the view rectangle in client pixels the fill tests use (§3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DrawGrid {
    pub a: i32,
    pub side: i32,
    pub origin: (i32, i32),
    pub view: PixelRect,
}

impl DrawGrid {
    /// §2 for view size `Wv × Hv` and tile origin `(cx_t, cy_t)`.
    pub fn new(wv: i32, hv: i32, tile_origin: ClientPos) -> Self {
        let a = wv / 160 + MARGIN_A;
        let side = a + hv / 80 + MARGIN_N;
        let t = tile_of(tile_origin.x, tile_origin.y);
        DrawGrid {
            a,
            side,
            origin: (t.0 - MARGIN_X, t.1 - a),
            view: PixelRect {
                left: tile_origin.x,
                top: tile_origin.y,
                right: tile_origin.x + wv,
                bottom: tile_origin.y + hv,
            },
        }
    }

    /// The grid of a frame's camera: `Wv`, `Hv` from the view rectangle
    /// (camera §1), origin the tile origin (camera §3).
    pub fn of_camera(camera: &Camera) -> Self {
        let v = &camera.view;
        DrawGrid::new(v.right - v.left, v.bottom - v.top, camera.tile)
    }

    /// Number of cells `n²`.
    pub fn cells(&self) -> usize {
        (self.side * self.side) as usize
    }

    /// Cell index `r × n + c` of tile `(tx, ty)`, `None` outside the grid.
    pub fn cell(&self, tile: (i32, i32)) -> Option<usize> {
        let c = tile.0 - self.origin.0;
        let r = tile.1 - self.origin.1;
        let range = 0..self.side;
        (range.contains(&c) && range.contains(&r)).then(|| (r * self.side + c) as usize)
    }
}

// ---------------------------------------------------------------- §9 inputs

/// The DT1 tile fields the order reads (§9: record +0x18).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Dt1Facts {
    pub orientation: u32,
    pub main: u32,
    pub sub: u32,
    /// +0x04.
    pub roof_height: i32,
    /// +0x08.
    pub height: i32,
    /// Material flags (§6 r2: bit 0x2 starts water effects).
    pub material: u16,
}

/// A record's fade bytes (§8, §9: +0x24, +0x28, +0x29, +0x2A, +0x2C).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fade {
    pub state: u8,
    pub alpha: u8,
    pub from: u8,
    pub to: u8,
    pub end: u32,
}

impl Fade {
    /// No fade running, alpha 0xFF.
    pub const OPAQUE: Fade = Fade {
        state: 0,
        alpha: 0xFF,
        from: 0,
        to: 0,
        end: 0,
    };
}

/// The coordinate record a wall record points at (+0x10; `drlg/levels.md`
/// §11.1): its box corner (+0x00 x0, +0x04 y0, level tiles) and index
/// (+0x28). Set only by the grid build of a preset room with lvlprest
/// `Logicals` ≠ 0 (§8, `0x0066C9C0`); every other record has none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Logical {
    pub x0: i32,
    pub y0: i32,
    pub index: i32,
}

/// One 0x30-byte tile record (§9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileRecord {
    /// Tile position in the room (+0x08, +0x0C).
    pub tile: (i32, i32),
    /// +0x14.
    pub flags: u32,
    /// +0x1C.
    pub ty: u32,
    pub dt1: Dt1Facts,
    pub fade: Fade,
    /// +0x10: the coordinate record (§8 group mode), `None` for 0.
    pub logical: Option<Logical>,
}

impl TileRecord {
    /// `ℓ`: record flag bits 14–16 (layer + 1, §4).
    pub fn layer(&self) -> u32 {
        (self.flags & REC_LAYER_BITS) >> 14
    }
}

/// What the order reads of a unit besides its position (§3 r4, §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct UnitFacts {
    pub unit_type: u8,
    pub mode: u32,
    pub flags: u32,
    pub flag_ex: u32,
    /// The local player.
    pub local: bool,
    /// monstats2 `unflatDead` (bit 20).
    pub unflat_dead: bool,
    /// objects `DrawUnder`.
    pub draw_under: u8,
    /// States 7 `playerbody`, 143 `attached`, 146 `invis`.
    pub playerbody: bool,
    pub attached: bool,
    pub invis: bool,
    /// The answer of the sight test (`draw-order-2.md` §15:
    /// [`sight::sight_hidden`] over the client DRLG's collision rooms),
    /// `true` = hidden. The feed fills it; `None` (the feed has no
    /// collision rooms yet) is an error for a unit the test applies to.
    pub sight_hidden: Option<bool>,
}

/// A unit of a room's unit list (room +0x74, list order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoomUnit {
    pub key: UnitKey,
    pub facts: UnitFacts,
}

/// A room's tile rectangle (+0x5C x, +0x60 y, +0x64 w, +0x68 h).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TileRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// One room of the near-room array (§9).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Room {
    /// Level id of the room (§6 r7: the reveal walks the player level's
    /// rooms only).
    pub level: u32,
    pub tiles: TileRect,
    /// Subtile origin (+0x4C, +0x50), for the fade test (§8).
    pub subtile_origin: (i32, i32),
    pub walls: Vec<TileRecord>,
    pub floors: Vec<TileRecord>,
    pub shadows: Vec<TileRecord>,
    pub units: Vec<RoomUnit>,
}

/// The level fields the passes read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LevelFacts {
    /// Level id of the local player's room (§1 pass 1).
    pub id: u32,
    /// levels `DrawEdges` (+9) (§6 r2).
    pub draw_edges: bool,
    /// `[0x0072A968]` = 0: the geometric fade branch of §8. 1.14d has 1
    /// in `.data` and no writer, so `false` (the group mode) is the only
    /// live value; the geometric branch is kept for its spec rule only.
    pub fade_geometric: bool,
}

/// The §9 map-tile feed of a frame: the near-room array of the local
/// player's active room, in array order, plus the player's tile (§8).
/// Owned by the client DRLG copy (`drlg/rooms.md` §3, §6, §9); the order
/// writes the frame's flag and fade changes back into it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NearRooms {
    pub rooms: Vec<Room>,
    /// The player's tile `(px, py)` (`[0x007C8A08]`, `[0x007C8A10]`: path
    /// subtile / 5).
    pub player_tile: (i32, i32),
    /// The player's coordinate-record index `[0x007C8A0C]`
    /// (`0x0061B130(player room, path sub-tile x, y)`, set by `0x004DDB70`
    /// each frame): 0 when the point is in no room, −1 for a null record.
    pub player_logical: i32,
    pub level: LevelFacts,
}

/// The fade clock of a frame (§8 clock arithmetic): `now` is the
/// `GetTickCount()` millisecond count (`u32`, wrapping); `instant` is
/// render kind `0x00477730` ≤ 3, the display type (`composition.md` §1:
/// 1 GDI, 3 DirectDraw), so on the reference every ramp completes at its
/// first walk and `now` only matters for the unreachable bit-2 branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct FadeClock {
    pub now: u32,
    pub instant: bool,
}

// ---------------------------------------------------------------- §3 fill

/// Which record array of a room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TileArray {
    Wall,
    Floor,
    Shadow,
}

/// A grid entry (§2): kind 0 unit, 1 tile, 2 unit shadow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Entry {
    Unit {
        room: usize,
        unit: usize,
    },
    Tile {
        room: usize,
        array: TileArray,
        record: usize,
    },
    UnitShadow {
        room: usize,
        unit: usize,
    },
}

/// A wall-array record in a layer-sorted list, with its `ℓ`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Layered {
    pub room: usize,
    pub record: usize,
    pub layer: u32,
}

/// One draw cell (§2): flag word and five lists.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Cell {
    pub flags: u32,
    pub shadow: Vec<Entry>,
    pub wall: Vec<Layered>,
    pub unit: Vec<Entry>,
    pub roof: Vec<Layered>,
    pub lower: Vec<Layered>,
}

/// The grid flags at view +0x38 (§1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct GridFlags {
    pub lower_walls: bool,
    pub roofs: bool,
    pub shadows: bool,
}

/// The filled grid of a frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lists {
    pub grid: DrawGrid,
    pub cells: Vec<Cell>,
    /// Entries asked of the pool (+0xEA9C), dropped ones included.
    pub count: usize,
    pub flags: GridFlags,
}

/// Layer-sorted insertion (§4): the first element with a successor whose
/// `ℓ` exceeds the new one gets it in front; else append. The last
/// element is never compared.
pub fn insert_layered<T>(list: &mut Vec<T>, item: T, layer: impl Fn(&T) -> u32) {
    let l = layer(&item);
    let at = (0..list.len().saturating_sub(1)).find(|&i| l < layer(&list[i]));
    match at {
        Some(i) => list.insert(i, item),
        None => list.push(item),
    }
}

/// `0x00464860` or unit flag 0x100000: a flat unit (§3 r4).
pub fn is_flat(u: &UnitFacts) -> bool {
    u.flags & UNIT_FLAT != 0
        || match u.unit_type {
            PLAYER => u.mode == 17,
            MONSTER => u.mode == 12 && !u.unflat_dead,
            OBJECT => u.draw_under & 2 != 0 || (u.draw_under & 1 != 0 && u.mode == 2),
            MISSILE => u.flags & UNIT_MISSILE_FLAT != 0,
            ITEM => u.mode == 3,
            _ => false,
        }
}

/// Room corner `E(x, y)` (`0x006433C0`, §3 r1).
pub fn room_corner(x: i32, y: i32) -> (i32, i32) {
    ((x - y) * 80 - 80, (x + y) * 40 + 80)
}

/// The room's rectangle in client pixels (`0x00619C40`, §3 r1).
pub fn room_rect(index: usize, t: &TileRect) -> Result<PixelRect, OrderError> {
    let r = PixelRect {
        left: room_corner(t.x, t.y + t.h).0,
        top: room_corner(t.x, t.y).1,
        right: room_corner(t.x + t.w, t.y).0,
        bottom: room_corner(t.x + t.w, t.y + t.h).1,
    };
    if r.left > r.right || r.top > r.bottom {
        return Err(OrderError::RoomRect {
            room: index,
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        });
    }
    Ok(r)
}

/// §3 r1: whether the room is filed.
pub fn room_visible(room: &PixelRect, view: &PixelRect) -> bool {
    room.left <= view.right
        && room.top <= view.bottom + ROOM_SLACK_BOTTOM
        && room.right >= view.left - ROOM_SLACK_LEFT
        && room.bottom >= view.top
}

/// §3 r2 / r3: whether a record at entry `(e0, e1)` with DT1 height `h`
/// and roof height `rh` (0 for the shadow array) is kept.
pub fn record_visible(e: (i32, i32), rh: i32, h: i32, view: &PixelRect) -> bool {
    e.0 <= view.right
        && e.0 + 160 >= view.left
        && e.1 - rh + h <= view.bottom
        && e.1 - rh - h >= view.top
}

struct Filler {
    cells: Vec<Cell>,
    count: usize,
    flags: GridFlags,
}

impl Filler {
    /// Takes a pool entry: `false` past the 3,000th (dropped, §2).
    fn take(&mut self) -> bool {
        self.count += 1;
        self.count <= POOL
    }
}

fn absolute(room: &Room, rec: &TileRecord) -> (i32, i32) {
    (room.tiles.x + rec.tile.0, room.tiles.y + rec.tile.1)
}

/// The cell of a tile record (§3 r2): `T(e0, e1) − (0, 1)`.
fn record_cell(grid: &DrawGrid, e: (i32, i32)) -> Option<usize> {
    let t = tile_of(e.0, e.1);
    grid.cell((t.0, t.1 - 1))
}

/// Fills the grid (§3) from the near rooms in array order. `positions`
/// holds each room unit's client position (camera §2). Wall records
/// filed into a wall list get their §8 fade target update.
pub fn fill(
    grid: &DrawGrid,
    near: &mut NearRooms,
    positions: &BTreeMap<UnitKey, ClientPos>,
    clock: FadeClock,
    skip_units: bool,
) -> Result<Lists, OrderError> {
    let mut f = Filler {
        cells: vec![Cell::default(); grid.cells()],
        count: 0,
        flags: GridFlags::default(),
    };
    let view = grid.view;
    let player = near.player_tile;
    let player_logical = near.player_logical;
    let geometric = near.level.fade_geometric;
    for (ri, room) in near.rooms.iter_mut().enumerate() {
        let rect = room_rect(ri, &room.tiles)?;
        if !room_visible(&rect, &view) {
            continue;
        }
        // r2: the wall array.
        for i in 0..room.walls.len() {
            let abs = absolute(room, &room.walls[i]);
            let rec = &mut room.walls[i];
            let e = tile_entry(abs.0, abs.1);
            if !record_visible(e, rec.dt1.roof_height, rec.dt1.height, &view) {
                continue;
            }
            let Some(ci) = record_cell(grid, e) else {
                continue;
            };
            f.cells[ci].flags = 0;
            let layered = Layered {
                room: ri,
                record: i,
                layer: rec.layer(),
            };
            let flag4 = !matches!(rec.ty, 0 | 13) && rec.flags & REC_NO_FADE == 0;
            if rec.flags & REC_LAYER_BITS == 0 {
                if f.take() {
                    f.flags.shadows = true;
                    f.cells[ci].shadow.push(Entry::Tile {
                        room: ri,
                        array: TileArray::Wall,
                        record: i,
                    });
                }
            } else if rec.ty == 15 {
                if f.take() {
                    f.flags.roofs = true;
                    insert_layered(&mut f.cells[ci].roof, layered, |l| l.layer);
                }
            } else if (16..=19).contains(&rec.ty) {
                // Cell flag 4 comes before the pool test (§2 overflow).
                if flag4 {
                    f.cells[ci].flags |= 4;
                }
                if f.take() {
                    f.flags.lower_walls = true;
                    insert_layered(&mut f.cells[ci].lower, layered, |l| l.layer);
                }
            } else {
                if flag4 {
                    f.cells[ci].flags |= 4;
                }
                // A dropped wall gets no fade target (§2 overflow:
                // `0x004DD180` returns before reading the clock).
                if !f.take() {
                    continue;
                }
                if rec.flags & REC_NO_FADE == 0 {
                    let near = if geometric {
                        let sub = (
                            room.subtile_origin.0 + 5 * rec.tile.0,
                            room.subtile_origin.1 + 5 * rec.tile.1,
                        );
                        fade_near(sub, player, rec.ty)
                    } else {
                        fade_near_group(rec.logical, player_logical, player)
                    };
                    retarget(&mut rec.fade, near, clock.now);
                }
                insert_layered(&mut f.cells[ci].wall, layered, |l| l.layer);
            }
        }
        // r3: the shadow array.
        for (i, rec) in room.shadows.iter().enumerate() {
            let abs = (room.tiles.x + rec.tile.0, room.tiles.y + rec.tile.1);
            let e = tile_entry(abs.0, abs.1);
            if !record_visible(e, 0, rec.dt1.height, &view) {
                continue;
            }
            let Some(ci) = record_cell(grid, e) else {
                continue;
            };
            if f.take() {
                f.flags.shadows = true;
                f.cells[ci].shadow.push(Entry::Tile {
                    room: ri,
                    array: TileArray::Shadow,
                    record: i,
                });
            }
        }
        // r4: the units.
        if skip_units {
            continue;
        }
        for (ui, unit) in room.units.iter().enumerate() {
            let at = positions.get(&unit.key).ok_or(OrderError::NoPosition {
                room: ri,
                key: unit.key,
            })?;
            let Some(ci) = grid.cell(tile_of(at.x, at.y)) else {
                continue;
            };
            if is_flat(&unit.facts) {
                if f.take() {
                    f.flags.shadows = true;
                    f.cells[ci].shadow.push(Entry::Unit { room: ri, unit: ui });
                }
                continue;
            }
            if f.take() {
                f.cells[ci].unit.push(Entry::Unit { room: ri, unit: ui });
            }
            if unit.facts.flag_ex & UNIT_EX_VISIBLE != 0 && f.take() {
                f.flags.shadows = true;
                f.cells[ci]
                    .shadow
                    .push(Entry::UnitShadow { room: ri, unit: ui });
            }
        }
    }
    Ok(Lists {
        grid: *grid,
        cells: f.cells,
        count: f.count,
        flags: f.flags,
    })
}

// ---------------------------------------------------------------- §5 units

/// §5 r1: units the entry skips.
pub fn unit_skipped(u: &UnitFacts) -> bool {
    u.flag_ex & UNIT_EX_SKIP != 0
        || (u.unit_type == PLAYER && u.mode == 17 && !u.playerbody)
        || (matches!(u.unit_type, MONSTER | OBJECT) && (u.attached || u.invis))
}

/// §5 r3: units the sight test applies to.
pub fn sight_tested(u: &UnitFacts) -> bool {
    match u.unit_type {
        PLAYER => !u.local && !matches!(u.mode, 0 | 17),
        MONSTER => !matches!(u.mode, 0 | 12),
        MISSILE | ITEM => true,
        _ => false,
    }
}

/// The unit entry `0x004DC7B0` (§5): whether the unit draws, with the
/// flag writes of r3 and r4.
pub fn unit_draws(u: &mut UnitFacts) -> Result<bool, OrderError> {
    if unit_skipped(u) {
        return Ok(false);
    }
    let hidden = if sight_tested(u) {
        u.sight_hidden.ok_or_else(|| {
            open(
                9,
                "no sight answer (draw-order-2.md §15) for a sight-tested unit",
            )
        })?
    } else {
        false
    };
    if hidden {
        u.flag_ex &= !UNIT_EX_VISIBLE;
        return Ok(false);
    }
    u.flag_ex |= UNIT_EX_VISIBLE;
    u.flags |= UNIT_DRAWN;
    Ok(true)
}

// ---------------------------------------------------------------- §8 fade

/// §8 "near", unused geometric branch (`[0x0072A968]` = 0, never in
/// 1.14d): the record's absolute subtile `sub` against the player's tile.
pub fn fade_near(sub: (i32, i32), player: (i32, i32), ty: u32) -> bool {
    let (lx, ly) = sub;
    let (px, py) = player;
    (5 * px < lx && lx < 5 * px + 20 && matches!(ty, 1 | 4 | 5 | 7 | 8 | 10 | 12))
        || (5 * py < ly && ly < 5 * py + 20 && matches!(ty, 2 | 3 | 6 | 7 | 9 | 11 | 12))
}

/// §8 "near" in the group mode (`0x004DD060`, the live mode in 1.14d):
/// the record's coordinate record `G` is set, its index differs from the
/// player's, and the player's tile lies before `G`'s corner on x or y —
/// the wall belongs to another logical room in front of the player.
pub fn fade_near_group(g: Option<Logical>, player_index: i32, player: (i32, i32)) -> bool {
    g.is_some_and(|g| g.index != player_index && (player.0 < g.x0 || player.1 < g.y0))
}

/// §8 r2: the fade target update at `now` (`t = now + 500`).
pub fn retarget(fade: &mut Fade, near: bool, now: u32) {
    let t = now.wrapping_add(500);
    let alpha = i32::from(fade.alpha);
    if near && fade.state & 1 == 0 {
        fade.from = 0xFF;
        fade.to = 0x80;
        fade.end = t.wrapping_add_signed(500 * (alpha - 0xFF) / 127);
        fade.state |= 3;
    } else if !near && fade.state & 1 != 0 {
        fade.from = 0x80;
        fade.to = 0xFF;
        fade.end = t.wrapping_add_signed(500 * (0x80 - alpha) / 127);
        fade.state = (fade.state & !1) | 2;
    }
}

/// §8 r3: advances a running fade (state bit 1) at the clock and writes
/// flags 0x400 / 0x8. Clock arithmetic (§8): "end reached" is the
/// unsigned `end ≤ now`; the ramp `(to − from) × ((now − end) + 500)` is a
/// wrapping 32-bit signed product, C-divided by 500, its low byte added
/// to `from` mod 256 (so a ramp across the `GetTickCount` wrap
/// overshoots, as in the original).
pub fn advance(rec: &mut TileRecord, clock: FadeClock) {
    let f = &mut rec.fade;
    if f.state & 2 == 0 {
        return;
    }
    let reached = f.end <= clock.now;
    if clock.instant || reached {
        f.alpha = f.to;
        f.state &= !2;
    } else {
        let span = (clock.now.wrapping_sub(f.end) as i32).wrapping_add(500);
        let diff = i32::from(f.to) - i32::from(f.from);
        let step = diff.wrapping_mul(span) / 500;
        f.alpha = f.from.wrapping_add(step as u8);
    }
    if f.alpha == 0 {
        rec.flags |= REC_FADED_OUT;
    } else {
        rec.flags &= !REC_FADED_OUT;
    }
    if f.state & 4 != 0 && reached {
        rec.flags |= REC_HIDDEN;
    }
}

// ---------------------------------------------------------------- §6 passes

/// §6 r6: whether the draw of an ordered tile sets record flag 0x20000,
/// decided after the drawer's culling (`camera.md` §7). Walls and lower
/// walls: the wall drawer returned non-zero, i.e. at least one block
/// (screen position = handed (X, Y) + the block's (x, y)) passed the
/// block test. Floors and roofs: the whole-tile test passed. Shadow tiles
/// never set it (units are not tiles).
pub fn sets_drawn_flag(camera: &Camera, tile: &OrderedTile, blocks: &[BlockRect]) -> bool {
    let list = source::placement_list(tile);
    let handed = camera.tile_handed(list, tile.cell.0, tile.cell.1);
    match tile.kind {
        TileKind::ShadowTile => false,
        TileKind::Floor { .. } | TileKind::Roof { .. } => camera.floor_roof_visible(handed),
        TileKind::Wall | TileKind::LowerWall => blocks
            .iter()
            .any(|b| camera.wall_block_visible(handed.0 + b.x, handed.1 + b.y)),
    }
}

/// §6 r6: writes flag 0x20000 into the records whose draw set it.
pub fn mark_drawn(near: &mut NearRooms, drawn: &[(usize, TileArray, usize)]) {
    for &(room, array, record) in drawn {
        let r = &mut near.rooms[room];
        let rec = match array {
            TileArray::Wall => &mut r.walls[record],
            TileArray::Floor => &mut r.floors[record],
            TileArray::Shadow => &mut r.shadows[record],
        };
        rec.flags |= REC_DRAWN;
    }
}

/// The `DrawKey` fields of an item (§10); `sub` is the unit composite's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OrderKey {
    pub pass: u32,
    pub major: u32,
    pub minor: u32,
}

/// How an ordered tile is drawn (§6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TileKind {
    /// §6 r1: wall drawer at the wall position.
    LowerWall,
    /// §6 r2: floor drawer; `layer` is `ℓ` (1 or 2).
    Floor { layer: u32 },
    /// §6 r3: `0x004F6980` at the wall position, draw mode 4.
    ShadowTile,
    /// §6 r4: wall drawer at the wall position.
    Wall,
    /// §6 r5: floor drawer at the roof position, roof pass `L` (1–4).
    Roof { pass: u32 },
}

/// A tile item in draw order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OrderedTile {
    pub room: usize,
    pub array: TileArray,
    pub record: usize,
    pub kind: TileKind,
    /// Absolute tile `(tx, ty)`.
    pub cell: (i32, i32),
    pub dt1: Dt1Facts,
    /// The record's alpha byte at its draw (0xFF opaque).
    pub alpha: u8,
    pub key: OrderKey,
}

/// One item of the frame in draw order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ordered {
    Tile(OrderedTile),
    /// A unit drawn through the unit composite (§5 r4).
    Unit {
        key: UnitKey,
        at: OrderKey,
    },
    /// A unit's shadow `0x00471620` (§6 r3, open question 3).
    UnitShadow {
        key: UnitKey,
        at: OrderKey,
    },
}

/// A unit's slot in the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnitSlot {
    /// No draw order computed this frame (no map feed).
    Unordered,
    /// Not filed, skipped or hidden (§3 r4, §5).
    NotDrawn,
    Drawn(OrderKey),
}

/// The frame's draw order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FrameOrder {
    pub items: Vec<Ordered>,
    /// Pool entries asked this frame (§2).
    pub count: usize,
}

impl FrameOrder {
    /// The slot of a unit: drawn with its key, else not drawn.
    pub fn unit_slot(&self, key: UnitKey) -> UnitSlot {
        self.items
            .iter()
            .find_map(|o| match o {
                Ordered::Unit { key: k, at } if *k == key => Some(UnitSlot::Drawn(*at)),
                _ => None,
            })
            .unwrap_or(UnitSlot::NotDrawn)
    }
}

fn key(pass: u32, major: usize, minor: usize) -> OrderKey {
    OrderKey {
        pass,
        major: major as u32,
        minor: minor as u32,
    }
}

struct Passes<'a> {
    near: &'a mut NearRooms,
    clock: FadeClock,
    out: Vec<Ordered>,
}

impl Passes<'_> {
    fn tile(&mut self, room: usize, array: TileArray, record: usize, kind: TileKind, at: OrderKey) {
        let r = &self.near.rooms[room];
        let rec = match array {
            TileArray::Wall => &r.walls[record],
            TileArray::Floor => &r.floors[record],
            TileArray::Shadow => &r.shadows[record],
        };
        self.out.push(Ordered::Tile(OrderedTile {
            room,
            array,
            record,
            kind,
            cell: absolute(r, rec),
            dt1: rec.dt1,
            alpha: rec.fade.alpha,
            key: at,
        }));
    }

    fn wall_record(&mut self, l: &Layered) -> &mut TileRecord {
        &mut self.near.rooms[l.room].walls[l.record]
    }

    /// §6 r1 / r4: advance the fade, then whether it draws.
    fn walk_wall(&mut self, l: &Layered) -> bool {
        let clock = self.clock;
        let rec = self.wall_record(l);
        advance(rec, clock);
        rec.flags & REC_LAYER_BITS != 0 && rec.flags & REC_SKIP == 0
    }

    fn unit(&mut self, room: usize, unit: usize, at: OrderKey) -> Result<(), OrderError> {
        let u = &mut self.near.rooms[room].units[unit];
        if unit_draws(&mut u.facts)? {
            self.out.push(Ordered::Unit { key: u.key, at });
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- §6 r7

/// The automap reveal state (`0x00459020`): countdown `[0x007A51A4]` and
/// the position of the last reveal (`[0x007A51FC]`, `[0x007A51F4]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct AutomapReveal {
    pub countdown: u32,
    pub last: (i32, i32),
}

/// A record the reveal adds to the automap (`0x00457CF0`, owner: the
/// automap spec).
pub type RevealedRecord = (usize, TileArray, usize);

/// `0x00458F40(room, all, ·)`: the room's floor array then its wall
/// array, in order; a record is added when it lacks flag 0x8 and has flag
/// 0x20000, or always when `all` (second argument 1, or `[0x007A51A0]`
/// ≠ 0, which has no writer). The room objects (`0x00458DC0`) follow;
/// they belong to the automap spec.
pub fn reveal_room(ri: usize, room: &Room, all: bool, out: &mut Vec<RevealedRecord>) {
    let keep = |r: &TileRecord| all || (r.flags & REC_HIDDEN == 0 && r.flags & REC_DRAWN != 0);
    for (i, r) in room.floors.iter().enumerate() {
        if keep(r) {
            out.push((ri, TileArray::Floor, i));
        }
    }
    for (i, r) in room.walls.iter().enumerate() {
        if keep(r) {
            out.push((ri, TileArray::Wall, i));
        }
    }
}

impl AutomapReveal {
    /// One frame of `0x00459020` (called by the frame `0x0044C7EB`): a
    /// non-zero countdown is decremented instead; otherwise, when the
    /// player moved by `d = (2·max + min) / 2 ≥ 0x50` since the last
    /// reveal, stores the position and reveals every near room of the
    /// player's level (`level`).
    pub fn frame(
        &mut self,
        player: (i32, i32),
        level: u32,
        near: &NearRooms,
    ) -> Vec<RevealedRecord> {
        let mut out = Vec::new();
        if self.countdown != 0 {
            self.countdown -= 1;
            return out;
        }
        let dx = (player.0 - self.last.0).abs();
        let dy = (player.1 - self.last.1).abs();
        let d = (2 * dx.max(dy) + dx.min(dy)) / 2;
        if d < 0x50 {
            return out;
        }
        self.last = player;
        for (ri, room) in near.rooms.iter().enumerate() {
            if room.level == level {
                reveal_room(ri, room, false, &mut out);
            }
        }
        out
    }
}

/// The draw order of a frame (§1, §3–§8, §10) through the grid of
/// `camera`. Open mode 3 draws no world (§1).
pub fn order_frame(
    camera: &Camera,
    mode: OpenMode,
    near: &mut NearRooms,
    positions: &BTreeMap<UnitKey, ClientPos>,
    clock: FadeClock,
) -> Result<FrameOrder, OrderError> {
    if mode.get() == 3 {
        return Ok(FrameOrder::default());
    }
    order_grid(&DrawGrid::of_camera(camera), mode, near, positions, clock)
}

/// [`order_frame`] for a given grid.
pub fn order_grid(
    grid: &DrawGrid,
    mode: OpenMode,
    near: &mut NearRooms,
    positions: &BTreeMap<UnitKey, ClientPos>,
    clock: FadeClock,
) -> Result<FrameOrder, OrderError> {
    // Pass 1: level backgrounds.
    if matches!(near.level.id, 74 | 120) {
        return Err(open(
            1,
            format!("level {} draws a background", near.level.id),
        ));
    }
    let lists = fill(grid, near, positions, clock, false)?;
    let mut p = Passes {
        near,
        clock,
        out: Vec::new(),
    };

    // Pass 2: lower walls (r1).
    if lists.flags.lower_walls {
        for (ci, cell) in lists.cells.iter().enumerate() {
            for (pos, l) in cell.lower.iter().enumerate() {
                if p.walk_wall(l) {
                    let k = key(pass::LOWER_WALLS, ci, pos);
                    p.tile(l.room, TileArray::Wall, l.record, TileKind::LowerWall, k);
                }
            }
        }
    }

    // Pass 3: floors (r2), straight from the rooms.
    if p.near.level.draw_edges && mode.get() == 0 {
        return Err(open(10, "the level draws edge floors (DrawEdges)"));
    }
    for ri in 0..p.near.rooms.len() {
        for layer in 1..=2 {
            for i in 0..p.near.rooms[ri].floors.len() {
                let rec = &p.near.rooms[ri].floors[i];
                if rec.layer() == layer && rec.flags & REC_SKIP == 0 && rec.dt1.orientation == 0 {
                    let k = key(pass::FLOORS, 2 * ri + (layer as usize - 1), i);
                    p.tile(ri, TileArray::Floor, i, TileKind::Floor { layer }, k);
                }
            }
        }
    }

    // Pass 4: TODO(spec: draw-order open question 2) `0x00473C00`.

    // Pass 5: the shadow pass (r3).
    if lists.flags.shadows {
        for (ci, cell) in lists.cells.iter().enumerate() {
            for (pos, e) in cell.shadow.iter().enumerate() {
                let k = key(pass::SHADOWS, ci, pos);
                match *e {
                    Entry::Unit { room, unit } => p.unit(room, unit, k)?,
                    Entry::Tile {
                        room,
                        array,
                        record,
                    } => {
                        let r = &p.near.rooms[room];
                        let rec = match array {
                            TileArray::Shadow => &r.shadows[record],
                            _ => &r.walls[record],
                        };
                        if rec.flags & REC_SKIP == 0 {
                            p.tile(room, array, record, TileKind::ShadowTile, k);
                        }
                    }
                    Entry::UnitShadow { room, unit } => {
                        let key = p.near.rooms[room].units[unit].key;
                        p.out.push(Ordered::UnitShadow { key, at: k });
                    }
                }
            }
            // The wall list's records without layer bits (none, §3 r2):
            // the walk still advances the fades.
            for l in &cell.wall {
                let clock = p.clock;
                advance(p.wall_record(l), clock);
            }
        }
    }

    // Pass 6: walls and units (r4).
    for (ci, cell) in lists.cells.iter().enumerate() {
        for (pos, l) in cell.wall.iter().enumerate() {
            if p.near.rooms[l.room].walls[l.record].ty == 15 {
                return Err(OrderError::RoofInWallList {
                    room: l.room,
                    record: l.record,
                });
            }
            if p.walk_wall(l) {
                let k = key(pass::WALLS_UNITS, ci, pos);
                p.tile(l.room, TileArray::Wall, l.record, TileKind::Wall, k);
            }
        }
        let walls = cell.wall.len();
        for (pos, e) in cell.unit.iter().enumerate() {
            if let Entry::Unit { room, unit } = *e {
                p.unit(room, unit, key(pass::WALLS_UNITS, ci, walls + pos))?;
            }
        }
    }

    // Pass 7: roofs (r5), four times by layer mask.
    if lists.flags.roofs {
        let n2 = lists.grid.cells();
        for roof_pass in 1..=4u32 {
            let mask = roof_pass << 14;
            for (ci, cell) in lists.cells.iter().enumerate() {
                for (pos, l) in cell.roof.iter().enumerate() {
                    let clock = p.clock;
                    let rec = p.wall_record(l);
                    advance(rec, clock);
                    if rec.flags & mask == mask && rec.flags & REC_SKIP == 0 {
                        let major = (roof_pass as usize - 1) * n2 + ci;
                        let k = key(pass::ROOFS, major, pos);
                        let kind = TileKind::Roof { pass: roof_pass };
                        p.tile(l.room, TileArray::Wall, l.record, kind, k);
                    }
                }
            }
        }
    }

    // Passes 8–10: TODO(spec: draw-order open question 2) `0x00475B20`,
    // `0x00473910`; the screen fade `0x004DC000` has no d2rs input yet.

    Ok(FrameOrder {
        items: p.out,
        count: lists.count,
    })
}
