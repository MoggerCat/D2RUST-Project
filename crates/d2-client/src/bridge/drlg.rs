// Spec: specs/client/model.md (§5 rule 6.1–6.2, §7 rule 4, §9 rules 1–2, §12 rules 1, 2, 5), specs/drlg/levels.md (§2 rule 3, §9 rule 2), specs/drlg/rooms.md (§4.2, §4.6, §5 rules 5, 9), specs/render/draw-order-2.md (§14, open question 2)
//! The client DRLG copy (`model.md` §12 rule 1): the `d2-sim` DRLG act
//! built from S→C 0x03's fields with the client flag, owned by the bridge
//! and never shared with the server's. 0x07 / 0x08 set and unset its
//! rooms in sight by coordinates (§9 rules 1–2, `rooms.md` §4.2); a room
//! in sight is built and becomes an active room, prepended to the act's
//! room list (`rooms.md` §5 rule 5). [`ClientDrlg::active_rooms`] is that
//! list in list order, the input of the room of a point (§12 rule 2).
//! Each new active room is recorded for the act room callback
//! (`rooms.md` §5 rule 9: the light cache, [`ClientDrlg::take_created`]),
//! and the client update runs the build timer and the level free
//! ([`ClientDrlg::client_update`], `rooms.md` §4.6).
//!
//! The DRLG's table view, tile headers and level types are inputs
//! ([`DrlgSource`]), as the server's are; each act build gets its own
//! level-type state from the source's factory. Plain Rust, no Bevy.

use std::sync::Arc;

use d2_sim::drlg::tiles::{first_entry, ACT_EDGE_TILE};
use d2_sim::drlg::{
    ActRooms, Drlg, DrlgData, DrlgError, LevelTypes, Services, TileInfo, TileSource,
};
use d2_sim::rng::Seed;
use d2_sim::units::RoomId;

use super::dispatch::HandlerError;
use super::world::ActiveRoom;
use crate::rules::lighting::records::LightError;

/// A DRLG room of the client act, by slot (`d2_sim::drlg::DrlgRoomId`).
pub use d2_sim::drlg::DrlgRoomId;

/// The level types of one client act (`drlg/levels.md` §3 step 7,
/// `rooms.md` §9.2): its own state, never the server's.
pub type ClientTypes = Box<dyn LevelTypes + Send + Sync>;

/// Builds the level-type state of a new client act.
pub type TypesFactory = Arc<dyn Fn() -> ClientTypes + Send + Sync>;

/// What the client DRLG reads that is not model state: the DRLG table
/// view, the DT1 tile headers and a factory of level-type state (the
/// same data the server's DRLG reads, `model.md` §12 rule 1).
#[derive(Clone)]
pub struct DrlgSource {
    pub data: Arc<DrlgData>,
    pub tiles: Arc<dyn TileSource + Send + Sync>,
    pub types: TypesFactory,
}

impl std::fmt::Debug for DrlgSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DrlgSource")
            .field("levels", &self.data.levels.len())
            .finish_non_exhaustive()
    }
}

/// The client act's room list (act +0x10, next active room +0x7C): new
/// active rooms are prepended, removed ones unlinked in place
/// (`rooms.md` §5 rule 5, §8; `sim/unit-order.md` §4). Record ids are
/// never reused.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ActList {
    /// Head first.
    pub rooms: Vec<RoomId>,
    next: u32,
    /// Records created since the last [`ClientDrlg::take_created`], in
    /// creation order (the act callback's calls, `rooms.md` §5 rule 9).
    created: Vec<RoomId>,
    /// The records whose flags (+0x34) have bit 0 set: populated by the
    /// client room pass (`model.md` §5 r6.1) or created with it.
    pub populated: std::collections::BTreeSet<RoomId>,
}

impl ActRooms for ActList {
    fn create_active_room(&mut self, _act: u8, flags: u32) -> RoomId {
        let id = RoomId(self.next);
        self.next += 1;
        if flags & 1 != 0 {
            self.populated.insert(id);
        }
        self.rooms.insert(0, id);
        self.created.push(id);
        id
    }

    /// The adjacency array lives in the DRLG's active room
    /// (`rooms.md` §6); the list keeps no copy.
    fn set_adjacent(&mut self, _room: RoomId, _adjacent: &[RoomId]) {}

    fn remove_active_room(&mut self, room: RoomId) -> u32 {
        self.rooms.retain(|&r| r != room);
        u32::from(self.populated.remove(&room))
    }
}

/// A client DRLG operation failed.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ClientDrlgError {
    /// A fatal error of the original's DRLG code, or a level-type error.
    #[error(transparent)]
    Drlg(#[from] DrlgError),
    /// A cloned [`ClientDrlg`] holds no level-type state and cannot
    /// generate (see [`ClientDrlg`]'s `Clone`).
    #[error("client DRLG snapshot: no level-type state")]
    Snapshot,
}

/// The act room callback's fatal cases (`render/lighting.md` §6.4) as
/// handler errors (`client/bridge.md` §2.4).
impl From<LightError> for HandlerError {
    fn from(e: LightError) -> Self {
        match e {
            LightError::Fatal0x591 => HandlerError::Fatal(0x591),
            LightError::OwnerWithoutRoom => HandlerError::Unspecified(
                "render/lighting.md §6.4: a cached light record's owner has no room",
            ),
            _ => HandlerError::Invalid("light record error outside the new-room rule"),
        }
    }
}

/// The act's edge floor record (act +0x18, `levels.md` §2 rule 3,
/// `0x00642A30`; `render/draw-order-2.md` §14, open question 2): the
/// first entry of its fixed key in the act's base tile library.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EdgeTile {
    /// The base library's DT1 path and the tile's index in file order.
    pub path: Vec<u8>,
    pub index: u32,
    pub info: TileInfo,
}

/// The client DRLG act of `[0x007A0634]` (`model.md` §1, §12 rule 1).
///
/// `Clone` makes a snapshot: the DRLG and the room list, without the
/// level-type state (which is not cloneable); a snapshot answers every
/// query, and an operation that needs the level types fails with
/// [`ClientDrlgError::Snapshot`]. Equality is that of the DRLG's whole
/// state (its `Debug` form) and the room list.
pub struct ClientDrlg {
    pub drlg: Drlg,
    pub list: ActList,
    types: Option<ClientTypes>,
    data: Arc<DrlgData>,
    tiles: Arc<dyn TileSource + Send + Sync>,
}

impl Clone for ClientDrlg {
    fn clone(&self) -> Self {
        Self {
            drlg: self.drlg.clone(),
            list: self.list.clone(),
            types: None,
            data: self.data.clone(),
            tiles: self.tiles.clone(),
        }
    }
}

impl PartialEq for ClientDrlg {
    fn eq(&self, other: &Self) -> bool {
        self.list == other.list && format!("{:?}", self.drlg) == format!("{:?}", other.drlg)
    }
}

impl Eq for ClientDrlg {}

impl std::fmt::Debug for ClientDrlg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientDrlg")
            .field("act", &self.drlg.act)
            .field("init_seed", &self.drlg.init_seed)
            .field("list", &self.list)
            .finish_non_exhaustive()
    }
}

impl ClientDrlg {
    /// `0x006194A0` with the client flag (`model.md` §7 rule 4,
    /// `levels.md` §2 rule 3): the DRLG of `act` from `init_seed`
    /// (0x03 u32@2) and the game's difficulty, town id 0 and DRLG flags 1
    /// (client copy), so no town is generated (`levels.md` §3 step 8).
    pub fn build(
        src: &DrlgSource,
        act: u8,
        init_seed: u32,
        difficulty: u8,
    ) -> Result<Self, DrlgError> {
        let mut types = (src.types)();
        let drlg = Drlg::create(
            act,
            init_seed,
            difficulty,
            0,
            true,
            &src.data,
            types.as_mut(),
        )?;
        Ok(Self {
            drlg,
            list: ActList::default(),
            types: Some(types),
            data: src.data.clone(),
            tiles: src.tiles.clone(),
        })
    }

    fn split(&mut self) -> Result<(&mut Drlg, Services<'_>), ClientDrlgError> {
        let types = self.types.as_mut().ok_or(ClientDrlgError::Snapshot)?;
        Ok((
            &mut self.drlg,
            Services {
                data: &self.data,
                tiles: &*self.tiles,
                types: types.as_mut(),
                rooms: &mut self.list,
            },
        ))
    }

    /// 0x07 (`model.md` §9 rule 1, `0x0061B640`): the room at tile (x, y)
    /// of level `level` set in sight with propagation when its status-1
    /// count is 0; `hint` is the local player's DRLG room. `None`: the
    /// level has no room at the point.
    pub fn set_in_sight(
        &mut self,
        level: u8,
        x: u16,
        y: u16,
        hint: Option<DrlgRoomId>,
    ) -> Result<Option<DrlgRoomId>, ClientDrlgError> {
        let (d, mut svc) = self.split()?;
        Ok(d.set_in_sight_at(&mut svc, level.into(), x.into(), y.into(), hint)?)
    }

    /// 0x08 (`model.md` §9 rule 2, `0x0061B690`).
    pub fn unset_in_sight(
        &mut self,
        level: u8,
        x: u16,
        y: u16,
        hint: Option<DrlgRoomId>,
    ) -> Result<Option<DrlgRoomId>, ClientDrlgError> {
        let (d, mut svc) = self.split()?;
        Ok(d.unset_in_sight_at(&mut svc, level.into(), x.into(), y.into(), hint)?)
    }

    /// The client update's DRLG part (`0x0044C790`, `rooms.md` §4.6 rule
    /// 1): the build timer `0x0061B920` (rules 4–8), then, when
    /// `free_levels` (every 13th call, §4.6 last paragraph), the level
    /// free of `levels.md` §9 rule 2 (`0x0061AA20`). Returns the rooms the
    /// timer built.
    pub fn client_update(&mut self, free_levels: bool) -> Result<Vec<DrlgRoomId>, ClientDrlgError> {
        let (d, mut svc) = self.split()?;
        let built = d.client_build_timer(&mut svc)?;
        if free_levels {
            d.free_inactive_levels(svc.data, svc.types)?;
        }
        Ok(built)
    }

    /// The DRLG rooms whose active rooms were created since the last call,
    /// in creation order (the act room callback `0x00475930` runs once
    /// for each, `rooms.md` §5 rule 9). A record already removed again is
    /// left out.
    pub fn take_created(&mut self) -> Vec<DrlgRoomId> {
        std::mem::take(&mut self.list.created)
            .into_iter()
            .filter_map(|id| self.drlg.drlg_room_of(id))
            .collect()
    }

    /// The act's active rooms in list order (act +0x10, newest first):
    /// each one's sub-tile rectangle (+0x4C…+0x58), its level id and its
    /// DRLG room (`model.md` §12 rule 2).
    pub fn active_rooms(&self) -> Vec<ActiveRoom> {
        self.list
            .rooms
            .iter()
            .map(|&id| {
                let room = self
                    .drlg
                    .drlg_room_of(id)
                    .expect("every listed record has its DRLG room");
                let a = self
                    .drlg
                    .active_room(room)
                    .expect("listed rooms are active");
                let level = self.drlg.level(self.drlg.room(room).level).id;
                ActiveRoom {
                    x0: a.subtiles.x,
                    y0: a.subtiles.y,
                    w: a.subtiles.w,
                    h: a.subtiles.h,
                    level: level as u16,
                    room,
                }
            })
            .collect()
    }

    /// The act's edge floor record ([`EdgeTile`]): `Ok(None)` for acts IV
    /// and V (no lookup, the record stays zero). A base library without
    /// the key is fatal 0x44C; a base library the tile source lacks is an
    /// input error.
    pub fn edge_tile(&self) -> Result<Option<EdgeTile>, String> {
        let Some(&Some((path, key))) = ACT_EDGE_TILE.get(usize::from(self.drlg.act)) else {
            return Ok(None);
        };
        let name = || String::from_utf8_lossy(path).into_owned();
        let tiles = self
            .tiles
            .dt1(path)
            .ok_or_else(|| format!("act base library {} not loaded", name()))?;
        let index = first_entry(tiles, key)
            .ok_or_else(|| format!("fatal 0x44C: no tile {key:?} in {}", name()))?;
        Ok(Some(EdgeTile {
            path: path.to_vec(),
            index,
            info: tiles[index as usize].clone(),
        }))
    }

    /// The adjacency array of an active DRLG room (`rooms.md` §6 order;
    /// it holds the room itself), as DRLG rooms. Empty for a room
    /// without an active room.
    pub fn adjacency(&self, room: DrlgRoomId) -> Vec<DrlgRoomId> {
        self.drlg
            .active_room(room)
            .map_or_else(Vec::new, |a| a.adjacency.clone())
    }

    /// The client presets of an active DRLG room (`model.md` §5 r6.2:
    /// its DS1 preset units with flag bit 0, in list order,
    /// room-relative). None without the level-type state (a snapshot).
    pub fn client_presets(&self, room: DrlgRoomId) -> Vec<d2_sim::drlg::ClientPreset> {
        self.types
            .as_ref()
            .map_or_else(Vec::new, |t| t.client_presets(&self.drlg, room))
    }

    /// The active room `room` populated by the client room pass
    /// (active-room flags +0x34 bit 0, `model.md` §5 r6.1).
    pub fn populated(&self, room: DrlgRoomId) -> bool {
        self.drlg
            .active_room(room)
            .is_some_and(|a| self.list.populated.contains(&a.id))
    }

    /// Sets bit 0 of the active room's flags (`model.md` §5 r6.1).
    pub fn set_populated(&mut self, room: DrlgRoomId) {
        if let Some(a) = self.drlg.active_room(room) {
            self.list.populated.insert(a.id);
        }
    }

    /// The active room's seed (+0x6C), the critter pass's R
    /// (`monsters/population.md` §11.7 r2).
    pub fn room_seed(&mut self, room: DrlgRoomId) -> Option<&mut Seed> {
        self.drlg.active_room_seed_mut(room)
    }

    /// The unit seed of a unit created in `room` (`model.md` §2 rule 6,
    /// §12 rule 5, Randomness rule 1): the active room's seed (+0x6C) is
    /// stepped once and the unit seed is `init_low(lo')`. `None` for a
    /// room without an active room.
    pub fn unit_seed(&mut self, room: DrlgRoomId) -> Option<Seed> {
        self.drlg.active_room_seed_mut(room).map(Seed::derive)
    }
}
