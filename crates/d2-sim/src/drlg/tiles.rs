// Spec: specs/drlg/rooms.md §9
//! Room tile grids (`rooms.md` §9): the per-room tile library (§9.3),
//! the rarity-weighted tile choice (§9.4), grid fill and cell rules
//! (§9.5), tiles shared between rooms (§9.6), animated tiles (§9.7) and
//! freeing (§9.2).

use std::collections::BTreeMap;

use super::data::DrlgData;
use super::level::Drlg;
use super::room::RoomKind;
use super::seams::{Services, TileInfo, TileSource};
use super::{room_flags, DrlgError, DrlgRoomId, TileRect};

/// The three fixed library files loaded after the mask's files (§9.3).
pub const FIXED_LIBRARY: [&[u8]; 3] = [
    b"DATA\\GLOBAL\\Tiles\\Act1\\Outdoors\\Blank.dt1",
    b"DATA\\GLOBAL\\Tiles\\Act1\\Barracks\\InvisWal.dt1",
    b"DATA\\GLOBAL\\Tiles\\Act1\\Barracks\\Warp.dt1",
];

/// Library slots per room (+0x68).
pub const LIBRARY_SLOTS: usize = 32;

/// Lookup cap (`0x00604AE0`): 40 entries.
pub const LOOKUP_CAP: usize = 40;

/// Default animation speed when lvlprest AnimSpeed is 0 (§9.7).
pub const DEFAULT_ANIM_SPEED: u32 = 80;

/// Packed cell bits (§9.4 table).
pub mod cell {
    pub const WALL: u32 = 0x1;
    pub const FLOOR: u32 = 0x2;
    pub const LINKED: u32 = 0x4;
    pub const ENCLOSED: u32 = 0x8;
    pub const LAYER_ABOVE: u32 = 0x80;
    pub const FILL_LOS: u32 = 1 << 16;
    pub const UNWALKABLE: u32 = 1 << 17;
    pub const REVEAL_HIDDEN: u32 = 1 << 26;
    pub const SHADOW: u32 = 1 << 27;
    pub const LINKAGE: u32 = 1 << 28;
    pub const OBJECT_WALL: u32 = 1 << 29;
    pub const HIDDEN: u32 = 1 << 31;

    /// Main index (bits 20–25).
    pub const fn main(v: u32) -> u32 {
        (v >> 20) & 0x3F
    }

    /// Sub index (bits 8–15).
    pub const fn sub(v: u32) -> u32 {
        (v >> 8) & 0xFF
    }

    /// Layer index (bits 18–19).
    pub const fn layer(v: u32) -> u32 {
        (v >> 18) & 3
    }
}

/// Tile record flags (§9.5.1 table).
pub mod rec_flags {
    pub const LAYER_ABOVE: u32 = 0x1;
    pub const DOOR_OR_EXIT: u32 = 0x2;
    pub const HIDDEN: u32 = 0x8;
    /// Door unit already placed (skip, §9.5.1 door records).
    pub const DOOR_UNIT: u32 = 0x20;
    pub const UNWALKABLE: u32 = 0x40;
    pub const FILL_LOS: u32 = 0x80;
    /// Layer + 1 in bits 14–15.
    pub const LAYER_SHIFT: u32 = 14;
}

/// A cached DT1 file and its key index (§9.3 "Library index").
#[derive(Clone, Debug)]
pub(super) struct Dt1File {
    pub(super) tiles: Vec<TileInfo>,
    /// Key → tile indexes in reverse file order (head insertion).
    keys: BTreeMap<(u32, u32, u32), Vec<u32>>,
}

impl Dt1File {
    fn new(tiles: &[TileInfo]) -> Self {
        let mut keys: BTreeMap<(u32, u32, u32), Vec<u32>> = BTreeMap::new();
        for (i, t) in tiles.iter().enumerate() {
            keys.entry((t.orientation, t.main, t.sub))
                .or_default()
                .insert(0, i as u32);
        }
        Self {
            tiles: tiles.to_vec(),
            keys,
        }
    }
}

/// A library entry: DT1 file (DRLG cache id) and tile index in file order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileRef {
    pub file: u32,
    pub index: u32,
}

/// Which record array a record is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordKind {
    Floor,
    Wall,
    /// Shadows and roofs share this array.
    Shadow,
}

/// A tile record (0x30 bytes; render-only fields left out).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileRecord {
    /// Position relative to the room, in tiles.
    pub x: i32,
    pub y: i32,
    /// Tile type (orientation).
    pub kind: u32,
    /// Record flags ([`rec_flags`]).
    pub flags: u32,
    /// The chosen DT1 tile.
    pub tile: TileRef,
    /// The packed cell the record came from.
    pub cell: u32,
    /// The chained type-4 half of a type-3 corner (index in the same array).
    pub half: Option<usize>,
}

/// One animation entry (§9.7).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimEntry {
    pub kind: RecordKind,
    /// Record indexes of frames 0..n−1.
    pub frames: Vec<usize>,
    pub pos: u32,
    pub speed: u32,
}

/// A room's tile grid (room +0x54).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RoomTiles {
    pub floors: Vec<TileRecord>,
    pub walls: Vec<TileRecord>,
    pub shadows: Vec<TileRecord>,
    /// Linked floor records, in list order (§9.6).
    pub floor_links: Vec<usize>,
    /// Linked wall and shadow records, in list order (§9.6).
    pub other_links: Vec<(RecordKind, usize)>,
    /// Animation entries, head first.
    pub anims: Vec<AnimEntry>,
}

impl RoomTiles {
    pub fn records(&self, kind: RecordKind) -> &[TileRecord] {
        match kind {
            RecordKind::Floor => &self.floors,
            RecordKind::Wall => &self.walls,
            RecordKind::Shadow => &self.shadows,
        }
    }

    fn records_mut(&mut self, kind: RecordKind) -> &mut Vec<TileRecord> {
        match kind {
            RecordKind::Floor => &mut self.floors,
            RecordKind::Wall => &mut self.walls,
            RecordKind::Shadow => &mut self.shadows,
        }
    }
}

/// A grid of packed cells, row-major, cell (x, y) relative to the room.
/// Reads outside the grid give 0.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CellGrid {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<u32>,
}

impl CellGrid {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![0; width * height],
        }
    }

    pub fn get(&self, x: i32, y: i32) -> u32 {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return 0;
        }
        self.cells[y as usize * self.width + x as usize]
    }

    pub fn set(&mut self, x: usize, y: usize, v: u32) {
        self.cells[y * self.width + x] = v;
    }
}

/// One `0x0066EC10` call: a cell grid, its orientation grid (wall
/// layers), and FillBlanks (§9.5 table).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GridPass {
    pub cells: CellGrid,
    pub orientation: Option<CellGrid>,
    pub fill_blanks: bool,
}

/// A room's source grids from the level type spec (§9.2 step 3c, §9.5):
/// passes in fill order, edge kills and animation settings (lvlprest).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RoomGrids {
    pub passes: Vec<GridPass>,
    pub kill_edge_x: bool,
    pub kill_edge_y: bool,
    pub animate: bool,
    /// lvlprest AnimSpeed (0 = 80).
    pub anim_speed: u32,
}

/// Index table `0x006EF620` (§9.6) by new tile type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Remap {
    Row(usize),
    Keep,
    Stop,
}

fn remap_index(t: u32) -> Remap {
    match t {
        1 => Remap::Row(0),
        2 => Remap::Row(1),
        3 => Remap::Row(2),
        5 => Remap::Row(3),
        6 => Remap::Row(4),
        7 => Remap::Row(5),
        8 | 9 | 13 => Remap::Keep,
        _ => Remap::Stop,
    }
}

fn is_door(t: u32) -> bool {
    t == 8 || t == 9
}

fn is_exit(t: u32) -> bool {
    t == 10 || t == 11
}

/// Record flag rules `0x0066DB20` (shadows `0x0066DF40`: no layer and
/// type rules). Flags are OR'd onto `existing`; only 0x8 can be cleared.
pub fn record_flags(existing: u32, t: u32, v: u32, material: u16, shadow: bool) -> u32 {
    let mut f = existing;
    if !shadow {
        if t != 13 {
            f |= (cell::layer(v) + 1) << rec_flags::LAYER_SHIFT;
        }
        if t == 14 {
            f |= 0x4;
        }
        if (8..=11).contains(&t) {
            f |= rec_flags::DOOR_OR_EXIT;
        }
    }
    if v & cell::LAYER_ABOVE != 0 {
        f |= rec_flags::LAYER_ABOVE;
    }
    if v & cell::LINKAGE != 0 {
        f |= 0x102;
    }
    if v & cell::UNWALKABLE != 0 {
        f |= rec_flags::UNWALKABLE;
    }
    if v & cell::FILL_LOS != 0 {
        f |= rec_flags::FILL_LOS;
    }
    if v & cell::ENCLOSED != 0 {
        f |= 0x4;
    }
    if v & cell::HIDDEN != 0 {
        f |= rec_flags::HIDDEN;
    } else {
        f &= !rec_flags::HIDDEN;
    }
    if v & cell::REVEAL_HIDDEN != 0 {
        f |= 0x20C;
    }
    if v & cell::OBJECT_WALL != 0 {
        f |= 0x800;
    }
    if v & cell::LINKED != 0 {
        f |= 0x2000;
    }
    if material & 0x1 != 0 {
        f |= 0x4;
    }
    if material & 0x4 != 0 {
        f |= 0x800;
    }
    f
}

/// The weighted walk of §9.4 step 4 over the entries' rarities. A total
/// ≤ 0 takes the first entry (§9.4 "Consequences": all rarities 0 → the
/// first entry, no draw; the literal walk would run past the array).
pub fn rarity_walk(rarities: &[i32], r: u32) -> usize {
    if rarities.iter().fold(0i32, |a, &b| a.wrapping_add(b)) <= 0 {
        return 0;
    }
    let mut n = (r as i32).wrapping_add(1);
    let mut i = 0;
    while rarities.len() > 1 && n > 0 && i < rarities.len() {
        n = n.wrapping_sub(rarities[i]);
        i += 1;
    }
    i.saturating_sub(1)
}

impl Drlg {
    /// The DT1 info of a tile reference.
    pub fn tile_info(&self, t: TileRef) -> &TileInfo {
        &self.dt1_files[t.file as usize].tiles[t.index as usize]
    }

    fn dt1_id(&mut self, tiles: &dyn TileSource, path: &[u8]) -> Result<u32, DrlgError> {
        if let Some(&id) = self.dt1_by_path.get(path) {
            return Ok(id);
        }
        let t = tiles
            .dt1(path)
            .ok_or_else(|| DrlgError::MissingDt1(String::from_utf8_lossy(path).into_owned()))?;
        let id = self.dt1_files.len() as u32;
        self.dt1_files.push(Dt1File::new(t));
        self.dt1_by_path.insert(path.to_vec(), id);
        Ok(id)
    }

    fn load_slot(
        &mut self,
        tiles: &dyn TileSource,
        id: DrlgRoomId,
        path: &[u8],
    ) -> Result<(), DrlgError> {
        if self.room(id).library.len() >= LIBRARY_SLOTS {
            return Err(DrlgError::LibraryFull);
        }
        let f = self.dt1_id(tiles, path)?;
        self.room_mut(id).library.push(f);
        Ok(())
    }

    /// `0x0066F240` (§9.3): load the room's DT1 files.
    pub(super) fn load_library(
        &mut self,
        svc: &mut Services<'_>,
        id: DrlgRoomId,
    ) -> Result<(), DrlgError> {
        self.load_library_with(svc.data, svc.tiles, id)
    }

    fn load_library_with(
        &mut self,
        data: &DrlgData,
        tiles: &dyn TileSource,
        id: DrlgRoomId,
    ) -> Result<(), DrlgError> {
        let mask = self.room(id).dt1_mask;
        let lt = self.level(self.room(id).level).level_type;
        for i in 0..32 {
            if mask >> i == 0 {
                break;
            }
            if mask & (1 << i) != 0 {
                // TODO(rooms.md §9.3): an empty `File` name is not covered
                // by the spec; it is skipped.
                if let Some(path) = data.lvltype_file(lt, i) {
                    let path = path.to_vec();
                    self.load_slot(tiles, id, &path)?;
                }
            }
        }
        for path in FIXED_LIBRARY {
            self.load_slot(tiles, id, path)?;
        }
        self.room_mut(id).flags |= room_flags::TILE_LIB_LOADED;
        Ok(())
    }

    /// Lookup `0x00604AE0` (§9.3): slot order, then reverse file order,
    /// at most 40 entries.
    pub fn lookup_tiles(&self, id: DrlgRoomId, t: u32, main: u32, sub: u32) -> Vec<TileRef> {
        let mut out = Vec::new();
        for &f in &self.room(id).library {
            if let Some(list) = self.dt1_files[f as usize].keys.get(&(t, main, sub)) {
                for &index in list {
                    if out.len() == LOOKUP_CAP {
                        return out;
                    }
                    out.push(TileRef { file: f, index });
                }
            }
        }
        out
    }

    /// Tile choice `0x0066D820` (§9.4) with room `id`'s library and seed.
    pub fn choose_tile(
        &mut self,
        id: DrlgRoomId,
        t: u32,
        main: u32,
        sub: u32,
    ) -> Result<TileRef, DrlgError> {
        let mut e = self.lookup_tiles(id, t, main, sub);
        if e.is_empty() {
            e = self.lookup_tiles(id, 10, 0, 0);
            if e.is_empty() {
                return Err(DrlgError::NoTile);
            }
        }
        let rar: Vec<i32> = e.iter().map(|&x| self.tile_info(x).rarity as i32).collect();
        let total = rar.iter().fold(0i32, |a, &b| a.wrapping_add(b));
        let r = if total > 0 {
            self.room_mut(id).seed.roll(total)
        } else {
            0
        };
        Ok(e[rarity_walk(&rar, r)])
    }

    fn tiles_mut(&mut self, id: DrlgRoomId) -> &mut RoomTiles {
        self.room_mut(id)
            .tiles
            .get_or_insert_with(RoomTiles::default)
    }

    fn material(&self, t: TileRef) -> u16 {
        self.tile_info(t).material
    }

    /// Appends a floor record.
    fn add_floor(&mut self, id: DrlgRoomId, x: i32, y: i32, v: u32, tile: TileRef) -> usize {
        let flags = record_flags(0, 0, v, self.material(tile), false);
        let t = self.tiles_mut(id);
        t.floors.push(TileRecord {
            x,
            y,
            kind: 0,
            flags,
            tile,
            cell: v,
            half: None,
        });
        t.floors.len() - 1
    }

    /// Appends a shadow record (`0x0066DF40`).
    fn add_shadow(&mut self, id: DrlgRoomId, x: i32, y: i32, v: u32, tile: TileRef) -> usize {
        let flags = record_flags(0, 13, v, self.material(tile), true);
        let t = self.tiles_mut(id);
        t.shadows.push(TileRecord {
            x,
            y,
            kind: 13,
            flags,
            tile,
            cell: v,
            half: None,
        });
        t.shadows.len() - 1
    }

    /// Wall records `0x0066DC50`: the record, its door unit, and for type
    /// 3 a chosen type-4 half.
    #[allow(clippy::too_many_arguments)]
    fn add_wall(
        &mut self,
        svc: &mut Services<'_>,
        id: DrlgRoomId,
        x: i32,
        y: i32,
        v: u32,
        t: u32,
        tile: TileRef,
    ) -> Result<usize, DrlgError> {
        let rect = self.room(id).rect;
        if is_door(t) {
            // New record: flag 0x20 is clear.
            svc.types
                .door_unit(self, svc.data, id, rect.x + x, rect.y + y, v);
        }
        let flags = record_flags(0, t, v, self.material(tile), false);
        let tl = self.tiles_mut(id);
        tl.walls.push(TileRecord {
            x,
            y,
            kind: t,
            flags,
            tile,
            cell: v,
            half: None,
        });
        let idx = tl.walls.len() - 1;
        if t == 3 {
            let t4 = self.choose_tile(id, 4, cell::main(v), cell::sub(v))?;
            let flags = record_flags(0, 4, v, self.material(t4), false);
            let tl = self.tiles_mut(id);
            tl.walls.push(TileRecord {
                x,
                y,
                kind: 4,
                flags,
                tile: t4,
                cell: v,
                half: None,
            });
            let h = tl.walls.len() - 1;
            tl.walls[idx].half = Some(h);
        }
        Ok(idx)
    }

    /// Wall warp tiles `0x0066E260` (§9.5).
    fn wall_warp_tiles(
        &mut self,
        svc: &mut Services<'_>,
        id: DrlgRoomId,
        wx: i32,
        wy: i32,
        v: u32,
    ) {
        let sub = cell::sub(v);
        if sub == 0 || sub == 4 {
            svc.types.warp_unit(self, id, wx, wy, v);
        }
        // TODO(rooms.md §9.5 "Warp tiles"): chaining to the room's warp
        // entry and the LitVersion extra record need the cell → warp entry
        // lookup, which the spec does not give; not modelled (no draw).
    }

    /// Floor warp tiles `0x0066E360` (§9.5).
    fn floor_warp_tiles(&mut self, id: DrlgRoomId) {
        self.room_mut(id).flags |= room_flags::NO_POPULATION;
        // TODO(rooms.md §9.5 "Warp tiles"): the warp entry of the cell's
        // destination is not specified; the LitVersion records (4 choices)
        // and chaining are not modelled.
    }

    /// Tile fill `0x0066EE70` (§9.5): every pass, then animation.
    pub(super) fn fill_room_tiles(
        &mut self,
        svc: &mut Services<'_>,
        id: DrlgRoomId,
        grids: &RoomGrids,
    ) -> Result<(), DrlgError> {
        if matches!(self.room(id).kind, RoomKind::Other(_)) {
            return Ok(());
        }
        self.room_mut(id).tiles = Some(RoomTiles::default());
        let rect = self.room(id).rect;
        let cols = rect.w + 1 - i32::from(grids.kill_edge_x);
        let rows = rect.h + 1 - i32::from(grids.kill_edge_y);
        for pass in &grids.passes {
            for y in 0..rows {
                for x in 0..cols {
                    let v = pass.cells.get(x, y);
                    let t = pass.orientation.as_ref().map_or(0, |o| o.get(x, y));
                    self.tile_cell(svc, id, x, y, v, t, pass.fill_blanks)?;
                }
            }
        }
        if grids.animate {
            self.setup_animation(id, grids.anim_speed)?;
        }
        Ok(())
    }

    /// One cell `0x0066E9B0` (§9.5.1).
    #[allow(clippy::too_many_arguments)]
    fn tile_cell(
        &mut self,
        svc: &mut Services<'_>,
        id: DrlgRoomId,
        x: i32,
        y: i32,
        v: u32,
        t: u32,
        fill_blanks: bool,
    ) -> Result<(), DrlgError> {
        let rect = self.room(id).rect;
        let level_id = self.level(self.room(id).level).id;
        let (wx, wy) = (rect.x + x, rect.y + y);
        let inside = rect.contains(wx, wy);
        let (main, sub) = (cell::main(v), cell::sub(v));
        let blank_key = main == 30 && (sub == 0 || sub == 1);
        // 1.
        if is_exit(t) && main >= 8 {
            return Ok(());
        }
        // 2.
        let mut v = v;
        if t == 0 && blank_key {
            v |= cell::HIDDEN;
        }
        // 3.
        if v & cell::HIDDEN != 0 {
            if is_door(t) && !matches!(level_id, 111 | 112 | 117) {
                svc.types.door_unit(self, svc.data, id, wx, wy, v);
                return Ok(());
            }
            if is_exit(t) {
                svc.types.warp_unit(self, id, wx, wy, v);
                self.floor_warp_tiles(id);
                return Ok(());
            }
        }
        // 4.
        if v & cell::LINKED != 0 {
            if v & cell::FLOOR != 0 {
                if blank_key {
                    v &= !cell::LAYER_ABOVE;
                }
                return self.linked_cell(svc, id, x, y, v, 0);
            } else if v & cell::WALL != 0 {
                return self.linked_cell(svc, id, x, y, v, t);
            } else if v & cell::SHADOW != 0 && v & cell::HIDDEN == 0 {
                return self.linked_cell(svc, id, x, y, v, 13);
            }
        }
        // 5.
        if v & cell::FLOOR != 0 {
            let tile = self.choose_tile(id, 0, main, sub)?;
            self.add_floor(id, x, y, v, tile);
        } else if fill_blanks && inside {
            let bsub = u32::from(level_id == 74);
            let tile = self.choose_tile(id, 0, 30, bsub)?;
            self.add_floor(id, x, y, (v & !cell::LAYER_ABOVE) | cell::HIDDEN, tile);
        }
        // 6.
        if v & cell::WALL != 0 {
            let tile = self.choose_tile(id, t, main, sub)?;
            self.add_wall(svc, id, x, y, v, t, tile)?;
            if is_exit(t) && level_id != 133 {
                self.wall_warp_tiles(svc, id, wx, wy, v);
            }
        }
        // 7.
        if v & cell::SHADOW != 0 {
            let tile = self.choose_tile(id, 13, main, sub)?;
            self.add_shadow(id, x, y, v, tile);
        }
        Ok(())
    }

    // ---- linked cells (§9.6) --------------------------------------------

    /// Find `0x0066E580`: the first neighbour record for a linked cell.
    fn find_linked(
        &self,
        id: DrlgRoomId,
        t: u32,
        v: u32,
        wx: i32,
        wy: i32,
    ) -> Option<(DrlgRoomId, RecordKind, usize)> {
        let near = self.room(id).near.as_ref()?;
        for &n in near {
            if n == id {
                continue;
            }
            let nr = self.room(n);
            let Some(tiles) = nr.tiles.as_ref() else {
                continue;
            };
            if !nr.rect.contains_closed(wx, wy) {
                continue;
            }
            let list: Vec<(RecordKind, usize)> = if t == 0 {
                tiles
                    .floor_links
                    .iter()
                    .map(|&i| (RecordKind::Floor, i))
                    .collect()
            } else {
                tiles.other_links.clone()
            };
            for (k, i) in list {
                let r = &tiles.records(k)[i];
                let layer = (r.flags >> rec_flags::LAYER_SHIFT) & 3;
                if nr.rect.x + r.x == wx
                    && nr.rect.y + r.y == wy
                    && r.kind != 4
                    && (k == RecordKind::Shadow || v & cell::SHADOW == 0)
                    && (layer == 0 || layer - 1 == cell::layer(v))
                {
                    return Some((n, k, i));
                }
            }
        }
        None
    }

    /// `0x0066E940`: find or add a linked cell's record.
    #[allow(clippy::too_many_arguments)]
    fn linked_cell(
        &mut self,
        svc: &mut Services<'_>,
        id: DrlgRoomId,
        x: i32,
        y: i32,
        v: u32,
        t: u32,
    ) -> Result<(), DrlgError> {
        let rect = self.room(id).rect;
        let (wx, wy) = (rect.x + x, rect.y + y);
        match self.find_linked(id, t, v, wx, wy) {
            Some((n, k, i)) => self.linked_found(svc, id, x, y, v, t, n, k, i),
            None => {
                // `0x0066E620`.
                if is_exit(t) && !rect.contains(wx, wy) {
                    return Ok(());
                }
                let tile = self.choose_tile(id, t, cell::main(v), cell::sub(v))?;
                match t {
                    0 => {
                        let i = self.add_floor(id, x, y, v, tile);
                        self.tiles_mut(id).floor_links.push(i);
                    }
                    13 => {
                        let i = self.add_shadow(id, x, y, v, tile);
                        self.tiles_mut(id).other_links.push((RecordKind::Shadow, i));
                    }
                    _ => {
                        let i = self.add_wall(svc, id, x, y, v, t, tile)?;
                        self.tiles_mut(id).other_links.push((RecordKind::Wall, i));
                        // TODO(rooms.md §9.6): the level-133 exclusion is
                        // stated for §9.5.1 step 6 only; applied here too.
                        let level_id = self.level(self.room(id).level).id;
                        if is_exit(t) && level_id != 133 {
                            self.wall_warp_tiles(svc, id, wx, wy, v);
                        }
                    }
                }
                Ok(())
            }
        }
    }

    /// Merged type of a linked cell over an existing record (§9.6 step 3).
    /// `None` = stop.
    ///
    /// TODO(rooms.md §9.6): the spec states the index table, "keep",
    /// "stop with R a normal wall" and the door edge cases without their
    /// full case analysis. Reading used here: a new door on this room's
    /// top/left edge keeps the new type; a non-door over a door on N's
    /// top/left edge stops; "keep" keeps R's type; a row with R's type in
    /// 1..7 takes the table value, else R's type; "stop" stops for R a
    /// normal wall (types 1..7), else keeps R's type.
    #[allow(clippy::too_many_arguments)]
    fn merged_type(
        &self,
        data: &DrlgData,
        id: DrlgRoomId,
        n: DrlgRoomId,
        r_kind: u32,
        wx: i32,
        wy: i32,
        t: u32,
    ) -> Result<Option<u32>, DrlgError> {
        let on_top_left = |r: TileRect| wx == r.x || wy == r.y;
        if is_door(t) && on_top_left(self.room(id).rect) {
            return Ok(Some(t));
        }
        if !is_door(t) && is_door(r_kind) && on_top_left(self.room(n).rect) {
            return Ok(None);
        }
        let normal_wall = (1..=7).contains(&r_kind);
        Ok(match remap_index(t) {
            Remap::Keep => Some(r_kind),
            Remap::Stop => (!normal_wall).then_some(r_kind),
            Remap::Row(i) if normal_wall => {
                let table = data
                    .wall_remap
                    .as_ref()
                    .ok_or(DrlgError::MissingWallRemap)?;
                Some(table.table[i][r_kind as usize - 1])
            }
            Remap::Row(_) => Some(r_kind),
        })
    }

    /// Found `0x0066E740` (§9.6 step 3).
    #[allow(clippy::too_many_arguments)]
    fn linked_found(
        &mut self,
        svc: &mut Services<'_>,
        id: DrlgRoomId,
        x: i32,
        y: i32,
        v: u32,
        t: u32,
        n: DrlgRoomId,
        k: RecordKind,
        i: usize,
    ) -> Result<(), DrlgError> {
        let r = self
            .room(n)
            .tiles
            .as_ref()
            .expect("found in tiles")
            .records(k)[i];
        let shadow = k == RecordKind::Shadow;
        if r.flags & rec_flags::LAYER_ABOVE != 0 {
            if is_door(r.kind) {
                let f = record_flags(r.flags, r.kind, v, self.material(r.tile), shadow);
                self.tiles_mut(n).records_mut(k)[i].flags = f;
            }
            return Ok(());
        }
        let rect = self.room(id).rect;
        let (wx, wy) = (rect.x + x, rect.y + y);
        let mut merged = r.kind;
        if v & cell::LAYER_ABOVE == 0 {
            match self.merged_type(svc.data, id, n, r.kind, wx, wy, t)? {
                Some(m) => merged = m,
                None => return Ok(()),
            }
        }
        // Corner handling.
        if r.kind == 3 && merged != 3 {
            if let Some(h) = r.half {
                let old = {
                    let rec = &mut self.tiles_mut(n).walls[h];
                    rec.flags |= rec_flags::HIDDEN;
                    rec.tile
                };
                self.collision_update(n, RecordKind::Wall, h, Some(old), None);
            }
        } else if r.kind != 3 && merged == 3 {
            self.tiles_mut(n).records_mut(k)[i].flags |= 0xC008;
            self.collision_update(n, k, i, Some(r.tile), None);
            let tile = self.choose_tile(id, 3, cell::main(v), cell::sub(v))?;
            let ni = self.add_wall(svc, id, x, y, v, 3, tile)?;
            self.tiles_mut(id).other_links.push((RecordKind::Wall, ni));
        }
        let tile_key = {
            let ti = self.tile_info(r.tile);
            (ti.main, ti.sub)
        };
        if merged != r.kind || (k == RecordKind::Floor && tile_key == (30, 0)) {
            let tile = self.choose_tile(n, merged, cell::main(v), cell::sub(v))?;
            let rec = &mut self.tiles_mut(n).records_mut(k)[i];
            let old = rec.tile;
            rec.kind = merged;
            rec.tile = tile;
            if tile != old {
                self.collision_update(n, k, i, Some(old), Some(tile));
            }
        }
        let rec = self.room(n).tiles.as_ref().expect("tiles").records(k)[i];
        let f = record_flags(rec.flags, rec.kind, v, self.material(rec.tile), shadow);
        self.tiles_mut(n).records_mut(k)[i].flags = f;
        Ok(())
    }

    // ---- animation (§9.7) -------------------------------------------------

    /// `0x0066D700` → `0x0066D440`: walls, floors, shadows.
    fn setup_animation(&mut self, id: DrlgRoomId, speed: u32) -> Result<(), DrlgError> {
        let speed = if speed == 0 {
            DEFAULT_ANIM_SPEED
        } else {
            speed
        };
        for kind in [RecordKind::Wall, RecordKind::Floor, RecordKind::Shadow] {
            let n0 = self
                .room(id)
                .tiles
                .as_ref()
                .map_or(0, |t| t.records(kind).len());
            for i in 0..n0 {
                let rec = self.room(id).tiles.as_ref().expect("tiles").records(kind)[i];
                if self.material(rec.tile) & 0x100 == 0 {
                    continue;
                }
                let f = self.lookup_tiles(id, rec.kind, cell::main(rec.cell), cell::sub(rec.cell));
                let frame = |d: &Self, k: u32| {
                    f.iter()
                        .copied()
                        .find(|&e| d.tile_info(e).rarity == k)
                        .ok_or(DrlgError::MissingFrame(k))
                };
                let f0 = frame(self, 0)?;
                self.tiles_mut(id).records_mut(kind)[i].tile = f0;
                let mut frames = vec![i];
                for k in 1..f.len() as u32 {
                    let fk = frame(self, k)?;
                    // TODO(rooms.md §9.7): frame records take the base
                    // record's flags plus 0x8 (the spec names only 0x8).
                    let recs = self.tiles_mut(id).records_mut(kind);
                    recs.push(TileRecord {
                        tile: fk,
                        flags: rec.flags | rec_flags::HIDDEN,
                        half: None,
                        ..rec
                    });
                    frames.push(recs.len() - 1);
                }
                self.tiles_mut(id).anims.insert(
                    0,
                    AnimEntry {
                        kind,
                        frames,
                        pos: 0,
                        speed,
                    },
                );
                self.room_mut(id).flags |= room_flags::ANIMATED;
            }
        }
        Ok(())
    }

    /// Animation tick `0x0066D410` over the room's near rooms
    /// (`0x0066D3B0` per room with flag 0x8000000 and tiles). No RNG.
    pub fn animate_tiles(&mut self, id: DrlgRoomId) {
        let near = self.room(id).near.clone().unwrap_or_default();
        for n in near {
            if self.room(n).flags & room_flags::ANIMATED == 0 {
                continue;
            }
            let Some(tiles) = self.room_mut(n).tiles.as_mut() else {
                continue;
            };
            let RoomTiles {
                floors,
                walls,
                shadows,
                anims,
                ..
            } = tiles;
            for a in anims.iter_mut() {
                let recs = match a.kind {
                    RecordKind::Floor => &mut *floors,
                    RecordKind::Wall => &mut *walls,
                    RecordKind::Shadow => &mut *shadows,
                };
                let len = a.frames.len() as u32;
                recs[a.frames[(a.pos >> 8) as usize]].flags |= rec_flags::HIDDEN;
                a.pos = (a.pos + a.speed) % (len * 256);
                recs[a.frames[(a.pos >> 8) as usize]].flags &= !rec_flags::HIDDEN;
            }
        }
    }

    // ---- freeing (§9.2) ----------------------------------------------------

    /// `0x0066F1A0` with keep = 0: free a room's tiles. The DT1 library
    /// stays loaded.
    ///
    /// TODO(rooms.md §9.2): the keep argument `0x0066B4C0` passes is not
    /// stated; keep = 0 is used (flag 0x200000 is never set).
    pub(super) fn free_room_tiles(&mut self, svc: &mut Services<'_>, id: DrlgRoomId) {
        if let Some(a) = self.room(id).active.as_ref().map(|a| a.id) {
            // Client copy path (unset handler 3): remove the active room.
            let _ = self.remove_active_room(svc, a);
            return;
        }
        if self.room(id).flags & room_flags::HAS_ROOM != 0 {
            let r = self.room_mut(id);
            r.flags &= !room_flags::HAS_ROOM;
            r.tiles = None;
            self.freed_rooms = self.freed_rooms.wrapping_add(1);
            svc.types.free_room_tiles(self, id);
        }
    }
}
