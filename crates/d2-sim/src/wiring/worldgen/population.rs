// Spec: specs/monsters/population.md §3, §8, §9, §11 (the PopWorld seam); specs/drlg/rooms.md §5, §9.3, §10; specs/drlg/preset.md §1, §9
//! Population → DRLG rooms, collision and preset units; units:
//! [`PopWorld`] on [`WorldHost`]. Seeds are the real ones (game seed of
//! the action state, the active-room seed +0x6C of the DRLG's active
//! room, the unit record's seed); rooms, boxes and collision are the act
//! DRLG's; preset units are the preset room's list. Queries the DRLG
//! specs do not describe go to [`WorldPending`].

use crate::drlg::DrlgRoomId;
use crate::monsters::population::{CoordRect, PopWorld, PresetUnit, RoomBox, TileRec};
use crate::rng::Seed;
use crate::units::{RoomId, UnitId};

use super::{WorldHost, WorldPending, WorldgenError};

impl<X: WorldPending> WorldHost<'_, X> {
    /// The act and DRLG room of an active room.
    fn drlg_room(&self, room: RoomId) -> Option<(u8, DrlgRoomId)> {
        let act = self.game.lists.room(room)?.act;
        let d = self.v.h.drlg.dungeon.acts.get(usize::from(act))?.as_ref()?;
        Some((act, d.drlg_room_of(room)?))
    }

    /// A scratch seed for a room without an active DRLG room (an error is
    /// logged with it).
    fn orphan_seed(&mut self, room: RoomId) -> &mut Seed {
        self.w.errors.push(WorldgenError::NoActiveRoom(room));
        self.v.h.orphan_seed = Seed::init();
        &mut self.v.h.orphan_seed
    }
}

impl<X: WorldPending> PopWorld for WorldHost<'_, X> {
    fn game_seed(&mut self) -> &mut Seed {
        &mut self.v.h.game_seed
    }

    /// The active-room seed (+0x6C, `rooms.md` §5).
    fn room_seed(&mut self, room: RoomId) -> &mut Seed {
        let active = self.drlg_room(room).filter(|&(act, r)| {
            self.v.h.drlg.dungeon.acts[usize::from(act)]
                .as_ref()
                .is_some_and(|d| d.active_room(r).is_some())
        });
        let Some((act, r)) = active else {
            return self.orphan_seed(room);
        };
        self.v.h.drlg.dungeon.acts[usize::from(act)]
            .as_mut()
            .and_then(|d| d.active_room_seed_mut(r))
            .expect("checked above")
    }

    fn unit_seed(&mut self, unit: UnitId) -> &mut Seed {
        self.v.seed(unit)
    }

    /// `0x0061A1B0`: the level id of the room's DRLG level.
    fn room_level(&self, room: RoomId) -> i32 {
        self.room_level_id(room)
    }

    fn populated_level(&self, room: RoomId) -> i32 {
        self.v.h.x.populated_level(room, self.room_level_id(room))
    }

    fn populated_room_count(&self, act: u8, level: i32) -> i32 {
        self.v.h.x.populated_room_count(act, level)
    }

    fn coord_list(&self, room: RoomId) -> Vec<CoordRect> {
        self.v.h.x.coord_list(room)
    }

    fn coord_at(&self, room: RoomId, x: i32, y: i32) -> Option<CoordRect> {
        self.v.h.x.coord_at(room, x, y)
    }

    fn coord_index_at(&self, room: RoomId, x: i32, y: i32) -> i32 {
        self.v.h.x.coord_index_at(room, x, y)
    }

    /// `0x00619730`: the active room's sub-tile rectangle (+0x4C).
    fn room_box(&self, room: RoomId) -> RoomBox {
        self.v
            .h
            .drlg
            .subtiles(self.game, room)
            .map_or(RoomBox::default(), |r| RoomBox {
                x: r.x,
                y: r.y,
                width: r.w,
                height: r.h,
            })
    }

    fn warp_points(&self, room: RoomId) -> Vec<(i32, i32)> {
        self.v.h.x.warp_points(room)
    }

    fn spawn_location(&self, room: RoomId, kind: u8) -> Option<(i32, i32)> {
        self.v.h.x.spawn_location(room, kind)
    }

    /// `0x00463740`: the room holding the point among `room` and its
    /// adjacency array (`rooms.md` §6).
    fn room_at(&self, room: RoomId, x: i32, y: i32) -> Option<RoomId> {
        self.v.h.drlg.find_room(self.game, room, x, y)
    }

    /// `0x0064D9B0` on the act's collision grids (`rooms.md` §10).
    // TODO(rooms.md §10, wire-action W5): the footprint a size covers is
    // not specified; the sub-tile at (x, y) is read for every size.
    fn collides(&self, room: RoomId, x: i32, y: i32, _size: i32, mask: u16) -> bool {
        self.v.h.drlg.collision(self.game, room, x, y).unwrap_or(0) & mask != 0
    }

    /// `0x0064CB30`.
    fn mask_at(&self, room: RoomId, x: i32, y: i32, mask: u16) -> bool {
        self.v.h.drlg.collision(self.game, room, x, y).unwrap_or(0) & mask != 0
    }

    /// `0x00619660`: the room's floor records (`rooms.md` §10.2 lists it
    /// as the floor list), positions in level tiles.
    // TODO(population.md §9.2, rooms.md §9.3): which DT1 header field the
    // accessor `0x00604BC0` reads is not stated; no record counts as
    // water (frog demons find no water point).
    fn tile_records(&self, room: RoomId) -> Vec<TileRec> {
        let Some((act, r)) = self.drlg_room(room) else {
            return Vec::new();
        };
        let Some(d) = self.v.h.drlg.dungeon.acts[usize::from(act)].as_ref() else {
            return Vec::new();
        };
        let dr = d.room(r);
        dr.tiles().map_or(Vec::new(), |t| {
            t.floors
                .iter()
                .map(|rec| TileRec {
                    water: false,
                    x: rec.x + dr.rect.x,
                    y: rec.y + dr.rect.y,
                })
                .collect()
        })
    }

    /// `0x00619FD0`: the preset room's unit list (room-relative sub-tiles,
    /// head first; `preset.md` §1 layout: mode +0x00, class +0x04, path
    /// +0x10, flags +0x1C).
    fn preset_units(&self, room: RoomId) -> Vec<PresetUnit> {
        let Some((act, r)) = self.drlg_room(room) else {
            return Vec::new();
        };
        let types = self.w.types.borrow();
        let Some(p) = types.act_presets(act) else {
            return Vec::new();
        };
        p.room_units(r)
            .iter()
            .map(|u| PresetUnit {
                unit_type: u.unit_type as i32,
                mode: u.mode as u8,
                class: u.class,
                x: u.x,
                y: u.y,
                has_data: u.path.is_some(),
                done: u.flags & 1 != 0,
            })
            .collect()
    }

    /// Client count (room +0x78): the active room's client array.
    fn client_count(&self, room: RoomId) -> u32 {
        let Some((act, r)) = self.drlg_room(room) else {
            return 0;
        };
        self.v.h.drlg.dungeon.acts[usize::from(act)]
            .as_ref()
            .and_then(|d| d.active_room(r))
            .map_or(0, |a| a.clients.len() as u32)
    }

    /// `0x00620BB0`.
    fn unit_room(&self, unit: UnitId) -> Option<RoomId> {
        self.room_of(unit)
    }

    fn unit_position(&self, unit: UnitId) -> (i32, i32) {
        self.v.h.path_position(unit)
    }

    fn quest_flag(&self, flag: u8) -> bool {
        self.v.h.x.quest_flag(flag)
    }

    fn chaos_blocks_population(&self) -> bool {
        self.v.h.x.chaos_blocks_population()
    }

    /// `0x0064E840` (`path-placement.md` §8) with the path provider:
    /// the coarse free-box search with the arguments of its population
    /// caller (`population.md` §6.3 rule 4: mask 0x3C01, size 1);
    /// without it [`WorldPending::nearest_free_point`].
    fn nearest_free_point(&self, room: RoomId, x: i32, y: i32) -> Option<(RoomId, i32, i32)> {
        if self.v.h.paths.is_none() {
            return self.v.h.x.nearest_free_point(room, x, y);
        }
        let mut p = crate::path::coords::Point::new(x, y);
        let r = crate::wiring::path::place::coarse_free_box(
            &self.v.h.drlg,
            room,
            &mut p,
            1,
            u32::from(crate::path::collision::masks::MONSTER_MOVE),
        )?;
        Some((r, p.x, p.y))
    }
}
