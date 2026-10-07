// Spec: specs/drlg/levels.md
//! The act DRLG (`levels.md` §2–§3), the level list and level seeds
//! (§4–§6), vis/warp records (§7), coordinate lookups (§8), the level
//! lifecycle (§9) and the spawn-room choice (§10).

use std::collections::BTreeMap;

use crate::rng::Seed;
use crate::units::RoomId;

use super::data::DrlgData;
use super::room::DrlgRoom;
use super::seams::{LevelTypes, Services};
use super::tiles::Dt1File;
use super::{is_town, room_flags, DrlgError, DrlgRoomId, LevelIdx, TileRect, SUBTILES};

/// Level flag 0x10: automap reveal (client copy, §4.3).
pub const LEVEL_FLAG_CLIENT: u32 = 0x10;

/// Inactive-frame reset of a level (§9: 10 passes of tick step 10).
pub const LEVEL_INACTIVITY_RESET: i32 = 10;

/// Tomb base level of Act II (§3.4): levels 66..72.
pub const TOMB_BASE: u32 = 66;

/// Spawn-tile class table `0x006EED88` (§10.2): (a, b) for t = 0..13.
pub const SPAWN_TILE_CLASSES: [(u32, u32); 14] = [
    (1, 0),
    (0, 0),
    (0, 0),
    (0, 0),
    (0, 0),
    (1, 1),
    (0, 1),
    (0, 1),
    (0, 1),
    (0, 1),
    (0, 2),
    (0, 3),
    (0, 4),
    (0, 5),
];

/// Spawn-tile index of the population's warp-distance location (§11.5
/// item 4).
pub const KIND11_TILE: u32 = 11;

/// Spawn-tile index that means "the waypoint room" (§10.2).
pub const SPAWN_WAYPOINT: u32 = 13;

/// One level (`levels.md` §1, 0x230 bytes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Level {
    /// Level id (+0x1D0).
    pub id: u32,
    /// DRLG type (+0x00): 1 maze, 2 preset, 3 outdoor.
    pub drlg_type: u32,
    /// Level type (+0x1C0): `lvltypes` row.
    pub level_type: u32,
    /// Flags (+0x04; 0x10 automap reveal).
    pub flags: u32,
    /// Position (+0x1C, +0x20) and size (+0x24, +0x28) in tiles.
    pub rect: TileRect,
    /// Level seed (+0x1C4).
    pub seed: Seed,
    /// Activity count (+0x0C).
    pub activity: i32,
    /// Inactive frames (+0x1D4).
    pub inactive_frames: i32,
    /// Spawn-tile records (+0x2C, count +0x1D8), set by the type specs.
    pub spawn_tiles: Vec<SpawnTile>,
    /// Warp-room centres in sub-tiles (+0x1E0 / +0x204, count +0x228).
    pub warp_centres: Vec<(i32, i32)>,
    /// Populated-room memory (+0x22C), one entry per room in list order.
    pub populated_memory: Option<Vec<bool>>,
    /// Coordinate-list counter (+0x1DC, §11.1): zero at allocation, not
    /// reset by §9.4.
    pub coord_counter: u32,
    pub(super) first_room: Option<DrlgRoomId>,
    pub(super) room_count: u32,
    pub(super) next: Option<LevelIdx>,
}

/// A spawn-tile record (stride 12: x, y, tile index), tile coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpawnTile {
    pub x: i32,
    pub y: i32,
    pub index: u32,
}

/// A DRLG's editable vis/warp copy of one level (§7.2, 0x48 bytes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WarpRecord {
    pub level_id: u32,
    pub vis: [u32; 8],
    pub warp: [i32; 8],
}

/// The waypoint room and the position of its waypoint object, if any.
type WaypointRoom = (DrlgRoomId, Option<(i32, i32)>);

/// Where a unit spawns in a level (§10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpawnPoint {
    pub room: DrlgRoomId,
    /// Tile position.
    pub x: i32,
    pub y: i32,
    /// The room's active room after streaming, if it has one.
    pub active: Option<RoomId>,
}

/// The client build cursor C (drlg +0x460, `rooms.md` §4.6 rules 6–9).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BuildCursor {
    /// Zero at allocation: rule 6 moves it to the first status-2 room.
    #[default]
    None,
    /// The status-2 list's head node (drlg +0x278), which reads status 2
    /// (rule 9) and so is kept by rule 6.
    Head,
    /// A DRLG room.
    Room(DrlgRoomId),
}

/// One act's DRLG (`levels.md` §1, 0x48C bytes) and everything it owns.
#[derive(Clone, Debug)]
pub struct Drlg {
    /// Act number, 0-based (+0x480).
    pub act: u8,
    /// DRLG seed (+0x00).
    pub seed: Seed,
    /// `dwStartSeed` (+0x470).
    pub start_seed: u32,
    /// Init seed copy (+0x458).
    pub init_seed: u32,
    /// Difficulty (+0x450).
    pub difficulty: u8,
    /// Flags bit 0: client copy (+0x8C).
    pub on_client: bool,
    /// Act II staff tomb level (+0x94), 0 elsewhere.
    pub staff_tomb: u32,
    /// Act II boss tomb level (+0x484), 0 elsewhere.
    pub boss_tomb: u32,
    /// Act III jungle-link bit (+0x474).
    pub jungle_link: bool,
    /// Rooms built (stat, +0x08).
    pub rooms_built: u32,
    /// Builds since the last client update (+0x98, u8).
    pub builds_since_update: u8,
    /// Client build timer T (+0x45C, u8; `rooms.md` §4.6 rule 2).
    pub build_timer: u8,
    /// Client build cursor C (+0x460; `rooms.md` §4.6 rule 2): none at
    /// allocation, a DRLG room, or the status-2 list's head node.
    pub build_cursor: BuildCursor,
    /// Freed-room counter (+0x468).
    pub freed_rooms: u32,
    pub(super) levels: Vec<Level>,
    pub(super) level_head: Option<LevelIdx>,
    /// Vis/warp records, head first (+0x90).
    pub(super) warp_records: Vec<WarpRecord>,
    pub(super) rooms: Vec<Option<DrlgRoom>>,
    /// Room status lists 0..3 (+0xA0), head first.
    pub(super) status_lists: [Vec<DrlgRoomId>; 4],
    /// Active room → DRLG room (lookup only).
    pub(super) active_index: BTreeMap<RoomId, DrlgRoomId>,
    /// DT1 files loaded by any room, by path (process-wide cache).
    pub(super) dt1_files: Vec<Dt1File>,
    pub(super) dt1_by_path: BTreeMap<Vec<u8>, u32>,
}

impl Drlg {
    /// An empty DRLG with its seeds set (§3 steps 1–2); no draw.
    fn alloc(act: u8, init_seed: u32, difficulty: u8, on_client: bool) -> Self {
        Self {
            act,
            seed: Seed::init_low(init_seed),
            start_seed: 0,
            init_seed,
            difficulty,
            on_client,
            staff_tomb: 0,
            boss_tomb: 0,
            jungle_link: false,
            rooms_built: 0,
            builds_since_update: 0,
            build_timer: 0,
            build_cursor: BuildCursor::None,
            freed_rooms: 0,
            levels: Vec::new(),
            level_head: None,
            warp_records: Vec::new(),
            rooms: Vec::new(),
            status_lists: Default::default(),
            active_index: BTreeMap::new(),
            dt1_files: Vec::new(),
            dt1_by_path: BTreeMap::new(),
        }
    }

    /// DRLG creation `0x00642DA0` (§3). `town_level` is the server's town
    /// level id (0 for a client copy, §2.3); `on_client` the client flag.
    /// Step 5 (tile library cache) draws nothing and is the tile
    /// source's job.
    pub fn create(
        act: u8,
        init_seed: u32,
        difficulty: u8,
        town_level: u32,
        on_client: bool,
        data: &DrlgData,
        types: &mut dyn LevelTypes,
    ) -> Result<Self, DrlgError> {
        let mut d = Self::alloc(act, init_seed, difficulty, on_client);
        // Draw 1: dwStartSeed.
        d.start_seed = d.seed.step();
        // Act-specific draws (step 4).
        match act {
            1 => loop {
                let a = d.seed.step() % 7;
                let b = d.seed.step() % 7;
                if a != b {
                    d.staff_tomb = TOMB_BASE + a;
                    d.boss_tomb = TOMB_BASE + b;
                    break;
                }
            },
            2 => d.jungle_link = d.seed.step() & 1 != 0,
            _ => {}
        }
        // Step 6: status lists are empty Vecs. Step 7: the act placer.
        types.create_act_levels(&mut d, data)?;
        // Step 8: the server's town.
        if town_level != 0 {
            let l = d.get_or_alloc_level(data, types, town_level)?;
            d.generate_level(data, types, l)?;
        }
        Ok(d)
    }

    // ---- levels (§4) ----------------------------------------------------

    pub fn level(&self, idx: LevelIdx) -> &Level {
        &self.levels[idx.0 as usize]
    }

    pub fn level_mut(&mut self, idx: LevelIdx) -> &mut Level {
        &mut self.levels[idx.0 as usize]
    }

    /// The level with id `id` if allocated (list walk, no allocation).
    pub fn find_level(&self, id: u32) -> Option<LevelIdx> {
        let mut cur = self.level_head;
        while let Some(l) = cur {
            if self.level(l).id == id {
                return Some(l);
            }
            cur = self.level(l).next;
        }
        None
    }

    /// The level list, head first (newest allocation first).
    pub fn level_list(&self) -> Vec<LevelIdx> {
        let mut out = Vec::new();
        let mut cur = self.level_head;
        while let Some(l) = cur {
            out.push(l);
            cur = self.level(l).next;
        }
        out
    }

    /// Get-or-allocate `0x00642BB0` (§4.2). Never generates rooms.
    pub fn get_or_alloc_level(
        &mut self,
        data: &DrlgData,
        types: &mut dyn LevelTypes,
        id: u32,
    ) -> Result<LevelIdx, DrlgError> {
        match self.find_level(id) {
            Some(l) => Ok(l),
            None => self.alloc_level(data, types, id),
        }
    }

    /// Allocate `0x00642AE0` (§4.3): seed `init_low(dwStartSeed + id)`,
    /// type init, then prepend.
    fn alloc_level(
        &mut self,
        data: &DrlgData,
        types: &mut dyn LevelTypes,
        id: u32,
    ) -> Result<LevelIdx, DrlgError> {
        let def = data.level(id)?;
        let idx = LevelIdx(self.levels.len() as u32);
        self.levels.push(Level {
            id,
            drlg_type: def.drlg_type,
            level_type: def.level_type,
            flags: if self.on_client { LEVEL_FLAG_CLIENT } else { 0 },
            rect: TileRect::default(),
            seed: Seed::init_low(self.start_seed.wrapping_add(id)),
            activity: 0,
            inactive_frames: 0,
            spawn_tiles: Vec::new(),
            warp_centres: Vec::new(),
            populated_memory: None,
            coord_counter: 0,
            first_room: None,
            room_count: 0,
            next: None,
        });
        // §4.4: unknown DRLG types get no init (1.14d silently).
        if matches!(def.drlg_type, 1..=3) {
            types.init_level(self, data, idx)?;
            if matches!(def.drlg_type, 1 | 2) {
                self.set_level_position_and_size(data, types, idx)?;
            }
        }
        let head = self.level_head.replace(idx);
        self.level_mut(idx).next = head;
        Ok(idx)
    }

    /// `0x00642D10` (§6.1): size from `SizeX/Y[difficulty]`, position =
    /// depend's position (get-or-allocate) + offset.
    pub fn set_level_position_and_size(
        &mut self,
        data: &DrlgData,
        types: &mut dyn LevelTypes,
        idx: LevelIdx,
    ) -> Result<(), DrlgError> {
        let def = data.level(self.level(idx).id)?.clone();
        let (w, h) = def.size[(self.difficulty as usize).min(2)];
        let (mut x, mut y) = (0, 0);
        if def.depend != 0 {
            let d = self.get_or_alloc_level(data, types, def.depend)?;
            x = self.level(d).rect.x;
            y = self.level(d).rect.y;
        }
        self.level_mut(idx).rect = TileRect::new(
            x.wrapping_add(def.offset.0),
            y.wrapping_add(def.offset.1),
            w,
            h,
        );
        Ok(())
    }

    /// Level generation `0x006424A0` (§5).
    pub fn generate_level(
        &mut self,
        data: &DrlgData,
        types: &mut dyn LevelTypes,
        idx: LevelIdx,
    ) -> Result<(), DrlgError> {
        let id = self.level(idx).id;
        self.level_mut(idx).seed = Seed::init_low(self.start_seed.wrapping_add(id));
        if matches!(self.level(idx).drlg_type, 1..=3) {
            types.generate(self, data, idx)?;
        }
        // §5.3: populated-room memory.
        if let Some(mem) = self.level(idx).populated_memory.clone() {
            for (i, r) in self.level_rooms(idx).into_iter().enumerate() {
                // TODO(levels.md §5.3): more rooms than memory entries
                // would read past the array in the original; unreachable
                // while a regenerated level repeats its room count.
                if mem.get(i).copied().unwrap_or(false) {
                    self.room_mut(r).other_flags |= 1;
                }
            }
        }
        // §5.4: warp-room centres.
        let warp = self.warp_array(data, id)?;
        for r in self.level_rooms(idx) {
            let room = self.room(r);
            let qualifies = room.flags & room_flags::ANY_WAYPOINT != 0
                || (0..8).any(|i| room.flags & (room_flags::WARP_0 << i) != 0 && warp[i] != -1);
            if qualifies {
                let c = (
                    (room.rect.x + room.rect.w / 2) * SUBTILES,
                    (room.rect.y + room.rect.h / 2) * SUBTILES,
                );
                // TODO(levels.md edge case 3): the original has 9 slots
                // and no bound check; a 10th centre overwrites +0x204.
                self.level_mut(idx).warp_centres.push(c);
            }
        }
        Ok(())
    }

    /// Rooms of a level in list order.
    pub fn level_rooms(&self, idx: LevelIdx) -> Vec<DrlgRoomId> {
        let mut out = Vec::new();
        let mut cur = self.level(idx).first_room;
        while let Some(r) = cur {
            out.push(r);
            cur = self.room(r).next;
        }
        out
    }

    /// Room count (+0x08).
    pub fn room_count(&self, idx: LevelIdx) -> u32 {
        self.level(idx).room_count
    }

    // ---- vis and warp records (§7) --------------------------------------

    fn record_pos(&self, level_id: u32) -> Option<usize> {
        self.warp_records
            .iter()
            .position(|r| r.level_id == level_id)
    }

    /// `0x00642860` (§7.2): the record of a level, created from leveldefs
    /// (prepended) if absent.
    pub fn warp_record_mut(
        &mut self,
        data: &DrlgData,
        level_id: u32,
    ) -> Result<&mut WarpRecord, DrlgError> {
        let pos = match self.record_pos(level_id) {
            Some(p) => p,
            None => {
                let def = data.level(level_id)?;
                self.warp_records.insert(
                    0,
                    WarpRecord {
                        level_id,
                        vis: def.vis,
                        warp: def.warp,
                    },
                );
                0
            }
        };
        Ok(&mut self.warp_records[pos])
    }

    /// The records, head first.
    pub fn warp_records(&self) -> &[WarpRecord] {
        &self.warp_records
    }

    /// Vis array reader `0x0066C040` (§7.2).
    pub fn vis_array(&self, data: &DrlgData, level_id: u32) -> Result<[u32; 8], DrlgError> {
        match self.record_pos(level_id) {
            Some(_) if level_id == 0 => Err(DrlgError::WarpRecordLevelZero),
            Some(p) => Ok(self.warp_records[p].vis),
            None => Ok(data.level(level_id)?.vis),
        }
    }

    /// Warp array reader `0x0066AEC0` (§7.2).
    pub fn warp_array(&self, data: &DrlgData, level_id: u32) -> Result<[i32; 8], DrlgError> {
        match self.record_pos(level_id) {
            Some(_) if level_id == 0 => Err(DrlgError::WarpRecordLevelZero),
            Some(p) => Ok(self.warp_records[p].warp),
            None => Ok(data.level(level_id)?.warp),
        }
    }

    /// Warp id of slot `i` (`0x0066AF30`, §7.4).
    pub fn warp_id(&self, data: &DrlgData, level_id: u32, slot: usize) -> Result<i32, DrlgError> {
        Ok(self.warp_array(data, level_id)?[slot])
    }

    /// `0x00642920` (§7.3, D2MOO `DRLG_SetWarpId`). `slot` −1 = any free
    /// slot. Edge case 2 (no match, no free slot) is reproduced: vis goes
    /// to the record's level-id field and warp to vis[7].
    pub fn set_warp(
        &mut self,
        data: &DrlgData,
        level_id: u32,
        vis: u32,
        warp: i32,
        slot: i32,
    ) -> Result<(), DrlgError> {
        let rec = self.warp_record_mut(data, level_id)?;
        if let Some(i) = rec.vis.iter().position(|&v| v == vis) {
            rec.warp[i] = warp;
            return Ok(());
        }
        let mut slot = slot;
        if slot == -1 {
            slot = (0..8)
                .find(|&i| rec.vis[i] == 0 && rec.warp[i] == -1)
                .map_or(-1, |i| i as i32);
        }
        if slot == -1 {
            // Index −1 addresses the field before vis[0] (the level id) and
            // warp[−1] = vis[7].
            rec.level_id = vis;
            rec.vis[7] = warp as u32;
        } else {
            rec.vis[slot as usize] = vis;
            rec.warp[slot as usize] = warp;
        }
        Ok(())
    }

    /// The lvlwarp row of slot `slot` of a level (`0x0066AF50`, §7.4).
    pub fn lvlwarp_row(
        &self,
        data: &DrlgData,
        level_id: u32,
        slot: usize,
        dir: u8,
    ) -> Result<usize, DrlgError> {
        data.lvlwarp_row(self.warp_id(data, level_id, slot)?, dir)
    }

    // ---- coordinates (§8) ----------------------------------------------

    /// First level of the list whose rect contains the point
    /// (`0x006427A0`).
    pub fn level_at(&self, x: i32, y: i32) -> Option<LevelIdx> {
        self.level_list()
            .into_iter()
            .find(|&l| self.level(l).rect.contains(x, y))
    }

    /// `0x00642C30` (§8.1): room at tile (x, y) with an optional hint
    /// room and level. May allocate level 0 and generate a level.
    pub fn room_at(
        &mut self,
        data: &DrlgData,
        types: &mut dyn LevelTypes,
        x: i32,
        y: i32,
        hint: Option<DrlgRoomId>,
        level: Option<LevelIdx>,
    ) -> Result<Option<DrlgRoomId>, DrlgError> {
        if let Some(h) = hint {
            if self.room(h).rect.contains(x, y) {
                return Ok(Some(h));
            }
            if let Some(near) = self.room(h).near.clone() {
                if let Some(&r) = near
                    .iter()
                    .find(|&&r| r != h && self.room(r).rect.contains(x, y))
                {
                    return Ok(Some(r));
                }
            }
        }
        let l = match level.or_else(|| self.level_at(x, y)) {
            Some(l) => l,
            None => self.get_or_alloc_level(data, types, 0)?,
        };
        if self.level(l).first_room.is_none() {
            self.generate_level(data, types, l)?;
        }
        Ok(self
            .level_rooms(l)
            .into_iter()
            .find(|&r| self.room(r).rect.contains(x, y)))
    }

    // ---- level lifecycle (§9) ------------------------------------------

    /// Activity counts `0x00643020` (§9.1) for a client moving from
    /// `old` to `new` (either may be none).
    pub fn update_level_activity(
        &mut self,
        data: &DrlgData,
        types: &mut dyn LevelTypes,
        old: Option<DrlgRoomId>,
        new: Option<DrlgRoomId>,
    ) -> Result<(), DrlgError> {
        let ol = old.map(|r| self.room(r).level);
        let nl = new.map(|r| self.room(r).level);
        if ol == nl {
            return Ok(());
        }
        for (lvl, delta) in [(ol, -1), (nl, 1)] {
            let Some(l) = lvl else { continue };
            self.touch_level(l, delta);
            let id = self.level(l).id;
            for v in self.vis_array(data, id)? {
                if v != 0 {
                    let vl = self.get_or_alloc_level(data, types, v)?;
                    self.touch_level(vl, delta);
                }
            }
        }
        Ok(())
    }

    /// `0x00642F70`: count += delta clamped to ≥ 0, frames := 10.
    fn touch_level(&mut self, l: LevelIdx, delta: i32) {
        let lv = self.level_mut(l);
        lv.activity = (lv.activity + delta).max(0);
        lv.inactive_frames = LEVEL_INACTIVITY_RESET;
    }

    /// Tick step 10 `0x00643200` (§9.2): free the rooms of inactive
    /// levels.
    pub fn free_inactive_levels(
        &mut self,
        data: &DrlgData,
        types: &mut dyn LevelTypes,
    ) -> Result<(), DrlgError> {
        let mut cur = self.level_head;
        while let Some(l) = cur {
            if self.level(l).activity == 0 && self.level(l).first_room.is_some() {
                if self.level(l).inactive_frames > 0 {
                    self.level_mut(l).inactive_frames -= 1;
                } else if self.level_free_test(data, types, l)? {
                    self.free_level_rooms(types, l);
                } else {
                    self.level_mut(l).inactive_frames = LEVEL_INACTIVITY_RESET;
                }
            }
            cur = self.level(l).next;
        }
        Ok(())
    }

    fn room_in_use(&self, r: DrlgRoomId) -> bool {
        let room = self.room(r);
        room.status < 4 || room.flags & room_flags::HAS_ROOM != 0
    }

    /// Free test `0x00643060` (§9.3). Frees warp-linked rooms' links and
    /// near arrays on the way.
    pub fn level_free_test(
        &mut self,
        data: &DrlgData,
        types: &mut dyn LevelTypes,
        l: LevelIdx,
    ) -> Result<bool, DrlgError> {
        if self.level_rooms(l).into_iter().any(|r| self.room_in_use(r)) {
            return Ok(false);
        }
        let id = self.level(l).id;
        for v in self.vis_array(data, id)? {
            if v == 0 {
                continue;
            }
            let vl = self.get_or_alloc_level(data, types, v)?;
            if self.level(vl).first_room.is_none() {
                continue;
            }
            let mut mask = 0;
            for (j, &x) in self.vis_array(data, v)?.iter().enumerate() {
                if x == id {
                    mask |= room_flags::WARP_0 << j;
                }
            }
            let linked: Vec<_> = self
                .level_rooms(vl)
                .into_iter()
                .filter(|&r| self.room(r).flags & mask != 0)
                .collect();
            if linked.iter().any(|&r| self.room_in_use(r)) {
                return Ok(false);
            }
            for r in linked {
                let room = self.room_mut(r);
                room.warp_links.clear();
                room.near = None;
            }
        }
        Ok(true)
    }

    /// `0x00642010` with keep = 1 (§9.4): remember populated bits, free
    /// every room, reset type data; the level stays in the list.
    pub fn free_level_rooms(&mut self, types: &mut dyn LevelTypes, l: LevelIdx) {
        let rooms = self.level_rooms(l);
        let bits: Vec<bool> = rooms
            .iter()
            .map(|&r| self.room(r).other_flags & 1 != 0)
            .collect();
        let mem = self
            .level_mut(l)
            .populated_memory
            .get_or_insert_with(|| vec![false; bits.len()]);
        // TODO(levels.md §9.4): "allocate once": a later free with more
        // rooms than the first would overrun; unreachable while the room
        // count repeats.
        for (m, b) in mem.iter_mut().zip(bits) {
            *m = b;
        }
        for r in rooms {
            self.free_room(r);
        }
        types.reset_level(self, l);
        let lv = self.level_mut(l);
        lv.first_room = None;
        lv.room_count = 0;
        lv.spawn_tiles.clear();
        lv.warp_centres.clear();
    }

    // ---- population queries (§11.5) ---------------------------------------

    /// `0x0066BB20` (§11.5 item 1): the level id of a room, 0 for a room
    /// with flag 0x800000 (no population).
    pub fn populated_level(&self, id: DrlgRoomId) -> u32 {
        let r = self.room(id);
        if r.flags & room_flags::NO_POPULATION != 0 {
            return 0;
        }
        self.level(r.level).id
    }

    /// `0x00642BE0` (§11.5 item 2): the level of `level_id`, allocated if
    /// absent (§4.3; no rooms are generated), and the number of its rooms
    /// without flag 0x800000.
    pub fn populated_room_count(
        &mut self,
        data: &DrlgData,
        types: &mut dyn LevelTypes,
        level_id: u32,
    ) -> Result<u32, DrlgError> {
        let l = self.get_or_alloc_level(data, types, level_id)?;
        Ok(self
            .level_rooms(l)
            .into_iter()
            .filter(|&r| self.room(r).flags & room_flags::NO_POPULATION == 0)
            .count() as u32)
    }

    /// `0x00642380` (§11.5 item 3): the warp-room centres (§5.4) of the
    /// room's own level, sub-tiles.
    pub fn warp_points(&self, id: DrlgRoomId) -> &[(i32, i32)] {
        &self.level(self.room(id).level).warp_centres
    }

    /// `0x00619E50(act, level, 11)` → `0x0066B2B0` (§11.5 item 4): the
    /// §10 spawn-room choice with tile index 11 and all its effects
    /// (generation, the level-seed draws, the streamed room); the tile
    /// position, (−1, −1) when no room was found.
    pub fn kind11_location(
        &mut self,
        svc: &mut Services<'_>,
        level_id: u32,
    ) -> Result<(i32, i32), DrlgError> {
        match self.spawn_room(svc, level_id, KIND11_TILE) {
            Ok(p) => Ok((p.x, p.y)),
            Err(DrlgError::NoSpawnRoom) => Ok((-1, -1)),
            Err(e) => Err(e),
        }
    }

    // ---- spawn room (§10) ------------------------------------------------

    /// `0x0066B2B0` (§10): the spawn room and position of a level for
    /// spawn-tile index `tile`; the chosen room is streamed (made active).
    pub fn spawn_room(
        &mut self,
        svc: &mut Services<'_>,
        level_id: u32,
        tile: u32,
    ) -> Result<SpawnPoint, DrlgError> {
        let l = self.get_or_alloc_level(svc.data, svc.types, level_id)?;
        if self.level(l).first_room.is_none() {
            self.generate_level(svc.data, svc.types, l)?;
        }
        let mut pos: Option<(i32, i32)> = None;
        let room;
        // (x, y) start at (−1, −1) (§10 rule 6); rule 5's centre default
        // runs only on the `Position` = 0 path.
        let mut default_centre = true;
        if svc.data.level(level_id)?.position != 0 {
            default_centre = false;
            if tile == SPAWN_WAYPOINT {
                let (r, p) = self
                    .waypoint_room(svc, l)?
                    .ok_or(DrlgError::NoWaypointRoom)?;
                room = r;
                pos = p;
            } else {
                let rec = self.pick_spawn_tile(l, tile);
                pos = Some((rec.x, rec.y));
                room = self
                    .room_at(svc.data, svc.types, rec.x, rec.y, None, None)?
                    .ok_or(DrlgError::NoSpawnRoom)?;
            }
        } else if let Some((r, p)) = self.waypoint_room(svc, l)? {
            room = r;
            pos = p;
        } else if let Some(r) = self.first_warp_room(svc.data, l)? {
            room = r;
        } else {
            let rect = self.level(l).rect;
            let (cx, cy) = (rect.x + rect.w / 2 - 2, rect.y + rect.h / 2 - 2);
            match self.room_at(svc.data, svc.types, cx, cy, None, Some(l))? {
                Some(r) => room = r,
                None => {
                    // `0x0066AE70`: roll(room count) on the level seed.
                    let n = self.room_count(l) as i32;
                    let k = self.level_mut(l).seed.roll(n) as usize;
                    room = *self.level_rooms(l).get(k).ok_or(DrlgError::NoSpawnRoom)?;
                }
            }
        }
        let (x, y) = pos.unwrap_or_else(|| {
            let r = self.room(room).rect;
            if default_centre {
                (r.x + r.w / 2, r.y + r.h / 2)
            } else {
                (-1, -1)
            }
        });
        let active = self.stream_room(svc, room)?;
        Ok(SpawnPoint { room, x, y, active })
    }

    /// `0x0066AC40` (§10.2): the record for spawn-tile index `t`. A
    /// record matches when its index equals `t`, or when `t`'s class pair
    /// has `a ≠ 0` and the record's `b` equals `t`'s. With `n` > 0
    /// matches, `roll(n)` on the level seed picks one; with none, record
    /// 0 (zeroed if absent: the level struct is zero-initialized).
    fn pick_spawn_tile(&mut self, l: LevelIdx, t: u32) -> SpawnTile {
        let class = |i: u32| SPAWN_TILE_CLASSES.get(i as usize).copied();
        let matches = |rec: &SpawnTile| {
            rec.index == t
                || match (class(t), class(rec.index)) {
                    (Some((a, b)), Some((_, rb))) => a != 0 && b == rb,
                    _ => false,
                }
        };
        let found: Vec<SpawnTile> = self
            .level(l)
            .spawn_tiles
            .iter()
            .copied()
            .filter(matches)
            .collect();
        if found.is_empty() {
            return self
                .level(l)
                .spawn_tiles
                .first()
                .copied()
                .unwrap_or_default();
        }
        let r = self.level_mut(l).seed.roll(found.len() as i32) as usize;
        found[r]
    }

    /// `0x0066AD80` (§10.4): the first room with a waypoint flag, streamed;
    /// position from its first waypoint preset object, if any.
    fn waypoint_room(
        &mut self,
        svc: &mut Services<'_>,
        l: LevelIdx,
    ) -> Result<Option<WaypointRoom>, DrlgError> {
        let Some(r) = self
            .level_rooms(l)
            .into_iter()
            .find(|&r| self.room(r).flags & room_flags::ANY_WAYPOINT != 0)
        else {
            return Ok(None);
        };
        self.stream_room(svc, r)?;
        let rect = self.room(r).rect;
        let pos = svc
            .types
            .preset_units(self, r)
            .into_iter()
            .find(|u| u.unit_type == 2 && u.class < 573 && svc.data.is_waypoint_object(u.class))
            .map(|u| (rect.x + u.x / SUBTILES, rect.y + u.y / SUBTILES));
        Ok(Some((r, pos)))
    }

    /// `0x0066B1F0`: the first room with a warp flag whose warp id ≠ −1.
    fn first_warp_room(
        &self,
        data: &DrlgData,
        l: LevelIdx,
    ) -> Result<Option<DrlgRoomId>, DrlgError> {
        let warp = self.warp_array(data, self.level(l).id)?;
        Ok(self.level_rooms(l).into_iter().find(|&r| {
            let f = self.room(r).flags;
            (0..8).any(|i| f & (room_flags::WARP_0 << i) != 0 && warp[i] != -1)
        }))
    }

    /// Whether a level id is a town or level 120 (§8.1 of `rooms.md`).
    pub(super) fn no_removal_level(id: u32) -> bool {
        is_town(id) || id == 120
    }
}

/// The acts of a game (`levels.md` §2): game +0xBC + 4·act, created on
/// first need.
#[derive(Clone, Debug, Default)]
pub struct Dungeon {
    pub acts: [Option<Drlg>; 5],
}

impl Dungeon {
    /// The act's DRLG, created by `0x0053AC70` (§2) on first need with the
    /// game's init seed (game +0x7C) and difficulty (game +0x6D). The
    /// server's town is the act's town level, or `arena_level` in an
    /// arena game.
    pub fn get_or_create(
        &mut self,
        act: u8,
        init_seed: u32,
        difficulty: u8,
        arena_level: Option<u32>,
        data: &DrlgData,
        types: &mut dyn LevelTypes,
    ) -> Result<&mut Drlg, DrlgError> {
        let slot = &mut self.acts[act as usize];
        if slot.is_none() {
            let town = arena_level.unwrap_or(super::TOWN_LEVELS[act as usize]);
            *slot = Some(Drlg::create(
                act, init_seed, difficulty, town, false, data, types,
            )?);
        }
        Ok(slot.as_mut().expect("created above"))
    }
}
