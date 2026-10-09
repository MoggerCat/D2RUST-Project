// Spec: specs/drlg/rooms.md
//! DRLG rooms (`rooms.md` §2), rooms-near arrays (§3), statuses and
//! activation (§4): status lists, set/unset handlers, propagation,
//! streaming and the build sequence (§4.4, §9.2).

use crate::rng::Seed;

use super::active::ActiveRoom;
use super::data::DrlgData;
use super::level::{BuildCursor, Drlg};
use super::logic::LogicInfo;
use super::seams::{LevelTypes, Services};
use super::tiles::RoomTiles;
use super::{is_town, room_flags, DrlgError, DrlgRoomId, LevelIdx, TileRect};
use crate::units::RoomId;

/// Room type (+0x48).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomKind {
    /// 1: outdoor-grid room (outdoor and maze-style code).
    Outdoor,
    /// 2: preset (DS1) room.
    Preset,
    /// Any other value (no type data).
    Other(u32),
}

/// A warp link (+0x4C, prepended): the linked room in the target level,
/// enabled flag, lvlwarp row of the slot (direction `'b'`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WarpLink {
    pub target: DrlgRoomId,
    pub enabled: bool,
    pub lvlwarp_row: usize,
}

/// Where [`Drlg::link_room`] adds a room to its level's list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkAt {
    Head,
    Tail,
}

/// Status "none" (`rooms.md` §4).
pub const STATUS_NONE: u8 = 4;

/// Near-gap limit (§3.1): both gaps < 6.
pub const NEAR_GAP: i32 = 6;

/// The level whose rooms never get warp links (§3.3).
pub const NO_WARP_LINK_LEVEL: u32 = 133;

/// A DRLG room (RoomEx, `rooms.md` §1, 0xEC bytes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DrlgRoom {
    /// Level (+0x58).
    pub level: LevelIdx,
    /// Room type (+0x48).
    pub kind: RoomKind,
    /// Tile rect (+0x34..+0x40).
    pub rect: TileRect,
    /// Room seed (+0x14).
    pub seed: Seed,
    /// `dwInitSeed` (+0x04).
    pub init_seed: u32,
    /// Flags (+0x28, [`room_flags`]).
    pub flags: u32,
    /// Other flags (+0x60; bit 0 was populated).
    pub other_flags: u32,
    /// DT1 mask (+0x50), set by the level type spec.
    pub dt1_mask: u32,
    /// Status (+0x44): 0..3, 4 none.
    pub status: u8,
    /// Status reference counts (+0x0C).
    pub counts: [u16; 4],
    /// Warp links, head first (+0x4C).
    pub warp_links: Vec<WarpLink>,
    pub(super) near: Option<Vec<DrlgRoomId>>,
    pub(super) next: Option<DrlgRoomId>,
    /// Tile library slots (+0x68): DT1 file ids of the DRLG cache.
    pub(super) library: Vec<u32>,
    pub(super) tiles: Option<RoomTiles>,
    pub(super) active: Option<ActiveRoom>,
    /// Logical-room info (+0x64, `levels.md` §11.1), built with the tiles.
    pub(super) logic: Option<LogicInfo>,
}

impl DrlgRoom {
    /// The rooms-near array, if built.
    pub fn near(&self) -> Option<&[DrlgRoomId]> {
        self.near.as_deref()
    }

    /// The tile grid, if built.
    pub fn tiles(&self) -> Option<&RoomTiles> {
        self.tiles.as_ref()
    }

    /// The active room, if any.
    pub fn active(&self) -> Option<&ActiveRoom> {
        self.active.as_ref()
    }

    /// The logical-room info (coordinate lists), if built.
    pub fn logic(&self) -> Option<&LogicInfo> {
        self.logic.as_ref()
    }
}

/// Gaps of `r` relative to `a` (§3.1); negative or zero when they overlap
/// or touch.
pub fn near_gaps(a: &TileRect, r: &TileRect) -> (i32, i32) {
    let gx = if a.x < r.x {
        r.x - a.w - a.x
    } else {
        a.x - r.w - r.x
    };
    let gy = if a.y < r.y {
        r.y - a.h - a.y
    } else {
        a.y - r.h - r.y
    };
    (gx, gy)
}

/// Whether `r` is near `a` (§3.1).
pub fn is_near(a: &TileRect, r: &TileRect) -> bool {
    let (gx, gy) = near_gaps(a, r);
    gx < NEAR_GAP && gy < NEAR_GAP
}

/// The bubble sort `0x0066BBC0` (§3.2): n − 1 passes over i = 0..n−2,
/// swapping when `a[i].x ≥ a[i+1].x + a[i+1].w` or
/// `a[i].y ≥ a[i+1].y + a[i+1].h`. Not a total order: the loop is the
/// rule.
pub fn sort_near<T: Copy>(items: &mut [T], rect: impl Fn(T) -> TileRect) {
    let n = items.len();
    for _ in 1..n {
        for i in 0..n - 1 {
            let (a, b) = (rect(items[i]), rect(items[i + 1]));
            if a.x >= b.x + b.w || a.y >= b.y + b.h {
                items.swap(i, i + 1);
            }
        }
    }
}

impl Drlg {
    // ---- room access ----------------------------------------------------

    pub fn room(&self, id: DrlgRoomId) -> &DrlgRoom {
        self.rooms[id.0 as usize].as_ref().expect("live DRLG room")
    }

    pub fn room_mut(&mut self, id: DrlgRoomId) -> &mut DrlgRoom {
        self.rooms[id.0 as usize].as_mut().expect("live DRLG room")
    }

    /// The room if it is live (not freed).
    pub fn try_room(&self, id: DrlgRoomId) -> Option<&DrlgRoom> {
        self.rooms.get(id.0 as usize)?.as_ref()
    }

    /// The status list `s` (0..3), head first.
    pub fn status_list(&self, s: u8) -> &[DrlgRoomId] {
        &self.status_lists[s as usize]
    }

    // ---- creation (§2) --------------------------------------------------

    /// `0x0066B3E0` (§2): a new room of `level` with status 4. Draws one
    /// level-seed step (room seed := `init_low(lo')`) and one room-seed
    /// step (`dwInitSeed` := `lo'`). The caller (level type spec) does the
    /// type data init (§2.5), sets flags and DT1 mask, and links the room
    /// ([`Drlg::link_room`]).
    pub fn alloc_room(&mut self, level: LevelIdx, kind: RoomKind, rect: TileRect) -> DrlgRoomId {
        let seed = self.level_mut(level).seed.derive();
        let mut room = DrlgRoom {
            level,
            kind,
            rect,
            seed,
            init_seed: 0,
            flags: 0,
            other_flags: 0,
            dt1_mask: 0,
            status: STATUS_NONE,
            counts: [0; 4],
            warp_links: Vec::new(),
            near: None,
            next: None,
            library: Vec::new(),
            tiles: None,
            active: None,
            logic: None,
        };
        room.init_seed = room.seed.step();
        if self.level(level).flags & super::level::LEVEL_FLAG_CLIENT != 0 {
            room.flags |= room_flags::AUTOMAP_REVEAL;
        }
        let id = DrlgRoomId(self.rooms.len() as u32);
        self.rooms.push(Some(room));
        id
    }

    /// Adds a room to its level's list (+0x10 / +0x24) and counts it.
    pub fn link_room(&mut self, id: DrlgRoomId, at: LinkAt) {
        let l = self.room(id).level;
        match at {
            LinkAt::Head => {
                let head = self.level(l).first_room;
                self.room_mut(id).next = head;
                self.level_mut(l).first_room = Some(id);
            }
            LinkAt::Tail => match self.level_rooms(l).last() {
                Some(&last) => self.room_mut(last).next = Some(id),
                None => self.level_mut(l).first_room = Some(id),
            },
        }
        self.level_mut(l).room_count += 1;
    }

    /// Room free `0x0066C100` (from `levels.md` §9.4): the room leaves its
    /// status list and its slot is released.
    pub(super) fn free_room(&mut self, id: DrlgRoomId) {
        let s = self.room(id).status;
        if s < STATUS_NONE {
            self.status_lists[s as usize].retain(|&r| r != id);
        }
        self.rooms[id.0 as usize] = None;
    }

    // ---- rooms-near arrays (§3) ------------------------------------------

    /// `0x0066C370` (§3): build the room's rooms-near array, its warp
    /// links (may generate linked levels) and the town-border flag.
    pub fn build_near(
        &mut self,
        data: &DrlgData,
        types: &mut dyn LevelTypes,
        id: DrlgRoomId,
    ) -> Result<(), DrlgError> {
        let a = self.room(id).rect;
        let level = self.room(id).level;
        // TODO(rooms.md OQ 5): the original collects into 30 slots without
        // a bound check; more candidates are kept here.
        let mut near: Vec<DrlgRoomId> = self
            .level_rooms(level)
            .into_iter()
            .filter(|&r| is_near(&a, &self.room(r).rect))
            .collect();
        self.sort_rooms(&mut near);
        self.room_mut(id).near = Some(near);

        let level_id = self.level(level).id;
        if self.room(id).flags & room_flags::WARP_MASK != 0 && level_id != NO_WARP_LINK_LEVEL {
            self.build_warp_links(data, types, id, level_id)?;
        }

        // §3.4 town border.
        if !is_town(level_id) {
            let near = self.room(id).near.clone().unwrap_or_default();
            if near
                .iter()
                .any(|&n| is_town(self.level(self.room(n).level).id))
            {
                self.room_mut(id).flags |= room_flags::NO_POPULATION;
            }
        }
        Ok(())
    }

    fn sort_rooms(&self, list: &mut [DrlgRoomId]) {
        sort_near(list, |r| self.room(r).rect);
    }

    fn append_near(&mut self, id: DrlgRoomId, t: DrlgRoomId) {
        let mut near = self.room_mut(id).near.take().unwrap_or_default();
        near.push(t);
        self.sort_rooms(&mut near);
        self.room_mut(id).near = Some(near);
    }

    /// §3.3 (`0x0066C220`, `0x0066BE80`).
    fn build_warp_links(
        &mut self,
        data: &DrlgData,
        types: &mut dyn LevelTypes,
        id: DrlgRoomId,
        level_id: u32,
    ) -> Result<(), DrlgError> {
        let vis = self.vis_array(data, level_id)?;
        for i in 0..8 {
            if self.room(id).flags & (room_flags::WARP_0 << i) == 0 {
                continue;
            }
            let v = vis[i];
            let l = self.get_or_alloc_level(data, types, v)?;
            let w = self.warp_id(data, level_id, i)?;
            if self.level(l).first_room.is_none() {
                self.generate_level(data, types, l)?;
            }
            let c = (0..i).filter(|&j| vis[j] == v).count();
            let lvis = self.vis_array(data, v)?;
            let slots: Vec<usize> = (0..8).filter(|&m| lvis[m] == level_id).collect();
            let mut linked = false;
            if w != -1 {
                if let Some(&m) = slots.get(c) {
                    linked = self.try_warp_link(data, id, l, m, i, w, level_id)?;
                }
            }
            if !linked {
                for &m in &slots {
                    if self.try_warp_link(data, id, l, m, i, w, level_id)? {
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    /// One link attempt for slot `m` of level `l` (§3.3).
    #[allow(clippy::too_many_arguments)]
    fn try_warp_link(
        &mut self,
        data: &DrlgData,
        id: DrlgRoomId,
        l: LevelIdx,
        m: usize,
        i: usize,
        w: i32,
        level_id: u32,
    ) -> Result<bool, DrlgError> {
        let flag = room_flags::WARP_0 << m;
        let targets: Vec<DrlgRoomId> = self
            .level_rooms(l)
            .into_iter()
            .filter(|&t| self.room(t).flags & flag != 0)
            .collect();
        if w != -1 {
            let Some(&t) = targets.first() else {
                return Ok(false);
            };
            self.append_near(id, t);
            let row = self.lvlwarp_row(data, level_id, i, b'b')?;
            self.room_mut(id).warp_links.insert(
                0,
                WarpLink {
                    target: t,
                    enabled: true,
                    lvlwarp_row: row,
                },
            );
            return Ok(true);
        }
        let a = self.room(id).rect;
        let mut linked = false;
        for t in targets {
            if is_near(&a, &self.room(t).rect) {
                self.append_near(id, t);
                linked = true;
            }
        }
        Ok(linked)
    }

    // ---- statuses (§4) --------------------------------------------------

    /// Force status `0x0061B210`: unlink, relink at the tail of list `s`
    /// (s < 4), status := s.
    pub(super) fn force_status(&mut self, id: DrlgRoomId, s: u8) {
        let old = self.room(id).status;
        if old == s {
            return;
        }
        if old < STATUS_NONE {
            self.status_lists[old as usize].retain(|&r| r != id);
        }
        if s < STATUS_NONE {
            self.status_lists[s as usize].push(id);
        }
        self.room_mut(id).status = s;
    }

    /// Set handler `s` (table `0x00744384`).
    fn set_handler(
        &mut self,
        svc: &mut Services<'_>,
        id: DrlgRoomId,
        s: u8,
    ) -> Result<(), DrlgError> {
        match s {
            0 => {
                // TODO(rooms.md OQ 1): D2MOO behaviour, not confirmed on 1.14d.
                if self.room(id).status > 0 {
                    self.force_status(id, 0);
                }
            }
            1 => {
                let r = self.room(id);
                if r.active.is_none() && r.flags & room_flags::HAS_ROOM == 0 {
                    self.bring_up(svc, id)?;
                    // §4.6 rule 2 (`0x0061B2F8`): T := R after the build.
                    self.build_timer = self.build_timer_reset();
                }
                if self.room(id).status > 1 {
                    self.force_status(id, 1);
                }
            }
            2 => {
                // TODO(rooms.md OQ 1): D2MOO behaviour, not confirmed on 1.14d.
                let r = self.room(id);
                let ready = r.flags & room_flags::TILE_LIB_LOADED != 0
                    && (r.kind != RoomKind::Preset
                        || r.flags & room_flags::PRESET_UNITS_ADDED != 0);
                if ready && r.status > 2 {
                    self.force_status(id, 2);
                    if self.room(id).counts[1] != 0 {
                        self.set_handler(svc, id, 1)?;
                    }
                }
            }
            _ => {
                self.load_room_data(svc, id)?;
                if self.room(id).status > 3 {
                    self.force_status(id, 3);
                }
            }
        }
        Ok(())
    }

    /// Recompute `0x0061B4F0`.
    fn recompute_status(&mut self, id: DrlgRoomId) {
        let r = self.room(id);
        if r.status >= STATUS_NONE || r.counts[r.status as usize] == 0 {
            let s = (0..4u8)
                .find(|&s| r.counts[s as usize] != 0)
                .unwrap_or(STATUS_NONE);
            self.force_status(id, s);
        }
    }

    /// Unset handler `s` (table `0x00744394`).
    fn unset_handler(&mut self, svc: &mut Services<'_>, id: DrlgRoomId, s: u8) {
        if s < 3 {
            // TODO(rooms.md OQ 1): handler 2 per D2MOO, not confirmed.
            self.recompute_status(id);
        } else if self.room(id).status != STATUS_NONE {
            self.recompute_status(id);
            if self.room(id).status == STATUS_NONE && self.on_client {
                self.free_room_tiles(svc, id);
            }
        }
    }

    /// Propagate `0x0061B390`.
    fn propagate(
        &mut self,
        svc: &mut Services<'_>,
        id: DrlgRoomId,
        s: u8,
    ) -> Result<(), DrlgError> {
        if self.room(id).near.is_none() {
            self.build_near(svc.data, svc.types, id)?;
        }
        let mut i = 0;
        // Count and array re-read every iteration.
        while let Some(&n) = self.room(id).near.as_ref().and_then(|a| a.get(i)) {
            if s < 3 {
                self.propagate(svc, n, s + 1)?;
            }
            self.set_step(svc, n, s)?;
            i += 1;
        }
        Ok(())
    }

    /// Steps 2–3 of the propagate loop for one room.
    fn set_step(&mut self, svc: &mut Services<'_>, n: DrlgRoomId, s: u8) -> Result<(), DrlgError> {
        let r = self.room(n);
        if r.status >= s && r.counts[..=s as usize].iter().all(|&c| c == 0) {
            self.set_handler(svc, n, s)?;
        }
        let c = &mut self.room_mut(n).counts[s as usize];
        *c = c.wrapping_add(1);
        Ok(())
    }

    /// Set-and-propagate `0x0061B490`.
    pub fn set_and_propagate(
        &mut self,
        svc: &mut Services<'_>,
        id: DrlgRoomId,
        s: u8,
    ) -> Result<(), DrlgError> {
        self.propagate(svc, id, s + 1)?;
        self.set_step(svc, id, s)
    }

    /// Unpropagate `0x0061B5B0`.
    pub fn unpropagate(&mut self, svc: &mut Services<'_>, id: DrlgRoomId, s: u8) {
        let mut i = 0;
        while let Some(&n) = self.room(id).near.as_ref().and_then(|a| a.get(i)) {
            let c = &mut self.room_mut(n).counts[s as usize];
            *c = c.wrapping_sub(1);
            self.unset_handler(svc, n, s);
            if s < 3 {
                self.unpropagate(svc, n, s + 1);
            }
            i += 1;
        }
    }

    /// `0x0061B6F0` (§4.1): DRLG status update for a client moving from
    /// `old` to `new`. See [`Drlg::client_changes_room`] for the whole
    /// server room change.
    pub fn change_status_room(
        &mut self,
        svc: &mut Services<'_>,
        old: Option<DrlgRoomId>,
        new: Option<DrlgRoomId>,
    ) -> Result<(), DrlgError> {
        if old == new {
            return Ok(());
        }
        if let Some(n) = new {
            self.set_and_propagate(svc, n, 0)?;
        }
        if let Some(o) = old {
            if self.room(o).counts[0] != 0 {
                self.room_mut(o).counts[0] -= 1;
                self.unset_handler(svc, o, 0);
                self.unpropagate(svc, o, 1);
            }
        }
        Ok(())
    }

    // ---- client in sight by coordinates (§4.2) ----------------------------

    /// The room lookup of `0x0061B640` / `0x0061B690` (§4.2,
    /// `client/model.md` §9 r1): get-or-allocate the level `level_id`
    /// (`0x00642BB0`), then the room at tile (x, y) in it (`0x00642C30`)
    /// with `hint` as the hint room only when it is in that level.
    fn sight_room(
        &mut self,
        svc: &mut Services<'_>,
        level_id: u32,
        x: i32,
        y: i32,
        hint: Option<DrlgRoomId>,
    ) -> Result<Option<DrlgRoomId>, DrlgError> {
        let l = self.get_or_alloc_level(svc.data, svc.types, level_id)?;
        let hint = hint.filter(|&h| self.try_room(h).is_some_and(|r| r.level == l));
        // `0x00642C30` generates a roomless level (`levels.md` §8.1); with
        // the services at hand, with `preset.md` §3.2 step 4.
        if self.level(l).first_room.is_none() {
            self.generate_level_svc(svc, l)?;
        }
        self.room_at(svc.data, svc.types, x, y, hint, Some(l))
    }

    /// Set client in sight by coordinates `0x0061B640` (§4.2,
    /// `client/model.md` §9 r1): if the room's status-1 count (+0x0E) is
    /// 0, set-and-propagate it with status 1. Returns the room, `None`
    /// when the level has no room at the point.
    pub fn set_in_sight_at(
        &mut self,
        svc: &mut Services<'_>,
        level_id: u32,
        x: i32,
        y: i32,
        hint: Option<DrlgRoomId>,
    ) -> Result<Option<DrlgRoomId>, DrlgError> {
        let Some(id) = self.sight_room(svc, level_id, x, y, hint)? else {
            return Ok(None);
        };
        if self.room(id).counts[1] == 0 {
            self.set_and_propagate(svc, id, 1)?;
        }
        Ok(Some(id))
    }

    /// Unset client in sight by coordinates `0x0061B690` (§4.2,
    /// `client/model.md` §9 r2): the same lookup; a room whose status-1
    /// count is not 0 gets count − 1, unset handler 1 and
    /// unpropagate(room, 2). Returns the room, `None` as for
    /// [`Drlg::set_in_sight_at`].
    pub fn unset_in_sight_at(
        &mut self,
        svc: &mut Services<'_>,
        level_id: u32,
        x: i32,
        y: i32,
        hint: Option<DrlgRoomId>,
    ) -> Result<Option<DrlgRoomId>, DrlgError> {
        let Some(id) = self.sight_room(svc, level_id, x, y, hint)? else {
            return Ok(None);
        };
        if self.room(id).counts[1] != 0 {
            self.room_mut(id).counts[1] -= 1;
            self.unset_handler(svc, id, 1);
            self.unpropagate(svc, id, 2);
        }
        Ok(Some(id))
    }

    // ---- streaming and build (§4.3, §4.4, §9.2) ---------------------------

    /// DT1 load and preset units (§9.2 steps 1–2; handler 3).
    fn load_room_data(&mut self, svc: &mut Services<'_>, id: DrlgRoomId) -> Result<(), DrlgError> {
        if self.room(id).flags & room_flags::TILE_LIB_LOADED == 0 {
            self.load_library(svc, id)?;
        }
        let r = self.room(id);
        if r.kind == RoomKind::Preset && r.flags & room_flags::PRESET_UNITS_ADDED == 0 {
            svc.types.add_preset_units(self, id)?;
            self.room_mut(id).flags |= room_flags::PRESET_UNITS_ADDED;
        }
        Ok(())
    }

    /// Stream a room `0x0061B730` (§4.3): load data, build if needed;
    /// no status change. Returns the active room.
    pub fn stream_room(
        &mut self,
        svc: &mut Services<'_>,
        id: DrlgRoomId,
    ) -> Result<Option<RoomId>, DrlgError> {
        self.bring_up(svc, id)?;
        Ok(self.room(id).active.as_ref().map(|a| a.id))
    }

    /// `0x0061B730` / `0x0061B190` (§9.2).
    fn bring_up(&mut self, svc: &mut Services<'_>, id: DrlgRoomId) -> Result<(), DrlgError> {
        self.load_room_data(svc, id)?;
        if self.room(id).flags & room_flags::HAS_ROOM != 0 {
            return Ok(());
        }
        // §4.4 step 1.
        if self.room(id).near.is_none() {
            self.build_near(svc.data, svc.types, id)?;
        }
        // Step 2: room seed reset, grid init.
        let r = self.room_mut(id);
        r.seed = Seed::init_low(r.init_seed);
        let grids = svc.types.room_grids(self, svc.data, id)?;
        // Step 3: tiles.
        self.fill_room_tiles(svc, id, &grids)?;
        self.room_mut(id).flags |= room_flags::HAS_ROOM;
        // Step 4: active room.
        self.create_active_room(svc, id)?;
        // Step 5.
        self.builds_since_update = self.builds_since_update.wrapping_add(1);
        self.rooms_built = self.rooms_built.wrapping_add(1);
        Ok(())
    }

    // ---- client build timer (§4.6) ----------------------------------------

    /// Reset value R of the build timer (`0x00642A00`, §4.6 rule 2): 5 for
    /// a client copy (DRLG flags bit 0), else 7.
    pub fn build_timer_reset(&self) -> u8 {
        if self.on_client {
            5
        } else {
            7
        }
    }

    /// The client build timer `0x0061B920` (§4.6 rules 4–8), once per
    /// client update. The statistics copy of rule 3 changes no outcome
    /// and is not modelled. Returns the rooms it built, in order (at most
    /// one per call).
    pub fn client_build_timer(
        &mut self,
        svc: &mut Services<'_>,
    ) -> Result<Vec<DrlgRoomId>, DrlgError> {
        // Rule 4.
        if self.builds_since_update > 1 {
            self.builds_since_update = 0;
            return Ok(Vec::new());
        }
        // Rule 5: B is kept when the timer has not run out.
        self.build_timer = self.build_timer.wrapping_sub(1);
        if self.build_timer != 0 {
            return Ok(Vec::new());
        }
        // Rule 6: C none or on a room whose status is not 2 → the first
        // status-2 room (the head node when the list is empty). The head
        // node reads status 2 (rule 9, `0x0061B7E0`), so it is kept.
        self.build_timer = self.build_timer_reset();
        let list = self.status_lists[2].clone();
        let n = list.len();
        let start = match self.build_cursor {
            BuildCursor::Head => n,
            BuildCursor::Room(c) if self.try_room(c).is_some_and(|r| r.status == 2) => {
                list.iter().position(|&r| r == c).unwrap_or(0)
            }
            _ => 0,
        };
        // Rule 7: the circular walk from S = C; index n (`None`) is the
        // head node, never built.
        let walk = (0..=n).map(|k| {
            let i = (start + k) % (n + 1);
            list.get(i).copied()
        });
        let mut built = Vec::new();
        let mut next = BuildCursor::Head;
        for (k, node) in walk.enumerate() {
            if let Some(id) = node {
                let r = self.room(id);
                if r.active.is_none() && r.flags & room_flags::HAS_ROOM == 0 {
                    self.bring_up(svc, id)?;
                    built.push(id);
                }
            }
            // Rule 8: C := the room after the last one examined.
            let after = (start + k + 1) % (n + 1);
            next = list
                .get(after)
                .map_or(BuildCursor::Head, |&r| BuildCursor::Room(r));
            if k + 1 == n + 1 || self.builds_since_update >= 1 {
                break;
            }
        }
        self.build_cursor = next;
        self.builds_since_update = 0;
        Ok(built)
    }
}
