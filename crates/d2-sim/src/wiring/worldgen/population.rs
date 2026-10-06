// Spec: specs/monsters/population.md §3, §8, §9, §11 (the PopWorld seam); specs/drlg/levels.md §11.4–§11.6; specs/drlg/rooms.md §5, §9.3, §10; specs/drlg/preset.md §1, §9
//! Population → DRLG rooms, collision and preset units; units:
//! [`PopWorld`] on [`WorldHost`]. Seeds are the real ones (game seed of
//! the action state, the active-room seed +0x6C of the DRLG's active
//! room, the unit record's seed); rooms, boxes and collision are the act
//! DRLG's; preset units are the preset room's list. Queries the DRLG
//! specs do not describe go to [`WorldPending`]. The DRLG data population
//! reads (coordinate lists, populated level and room count, warp points,
//! the kind-11 location) are the act DRLG's (`drlg/levels.md` §11.6).

use crate::drlg::DrlgRoomId;
use crate::monsters::population::{CoordRect, PopWorld, PresetUnit, RoomBox, TileRec};
use crate::rng::Seed;
use crate::units::{RoomId, UnitId};

use super::{WorldHost, WorldPending, WorldgenError};
use crate::drlg::CoordRec;
use crate::wiring::action::WiringError;

/// A DRLG coordinate record as population reads it: the clipped box
/// (+0x10), the node flag and the index (`levels.md` §11.1).
fn coord_rect(c: &CoordRec) -> CoordRect {
    CoordRect {
        rect: c.clipped,
        node_flag: i32::from(c.node),
        index: c.index as i32,
    }
}

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

    /// `0x0061A1F0` → `0x0066BB20` (`levels.md` §11.5 item 1): 0 for a
    /// null room or a flag-0x800000 room, else the level id.
    fn populated_level(&self, room: RoomId) -> i32 {
        self.v
            .h
            .drlg
            .drlg_room(self.game, room)
            .map_or(0, |(d, r)| d.populated_level(r) as i32)
    }

    /// `0x0061ABF0` → `0x00642BE0` (`levels.md` §11.5 item 2) on the act's
    /// DRLG: allocates the level when absent. An act without a DRLG, or a
    /// DRLG error (logged), counts 0.
    fn populated_room_count(&mut self, act: u8, level: i32) -> i32 {
        let r = self.v.h.drlg.with_act(act, &mut self.game.lists, |d, svc| {
            d.populated_room_count(svc.data, svc.types, level as u32)
        });
        match r {
            Some(Ok(n)) => n as i32,
            Some(Err(e)) => {
                self.w
                    .errors
                    .push(WorldgenError::Wiring(WiringError::Drlg(e)));
                0
            }
            None => 0,
        }
    }

    /// `0x0061AD50` → `0x0066CF30` (`levels.md` §11.4): the room's
    /// coordinate records in `next` order; empty without info (fatal
    /// 0x2CD in the original, unreached in 1.14d, §11.2 step 3).
    fn coord_list(&self, room: RoomId) -> Vec<CoordRect> {
        self.v
            .h
            .drlg
            .drlg_room(self.game, room)
            .and_then(|(d, r)| d.coord_first(r).map(|l| l.iter().map(coord_rect).collect()))
            .unwrap_or_default()
    }

    /// `0x0061AD30` → `0x0066CEB0` (`levels.md` §11.4).
    fn coord_at(&self, room: RoomId, x: i32, y: i32) -> Option<CoordRect> {
        let (d, r) = self.v.h.drlg.drlg_room(self.game, room)?;
        d.coord_at(r, x, y).as_ref().map(coord_rect)
    }

    /// `0x0061B130` → `0x0066CE30` (`levels.md` §11.4): the room holding
    /// the point among the room and its adjacency array; none → 0; that
    /// room's record at the point → its index; null record → −1.
    // TODO(levels.md §11.4): a point outside the room's (W+1) × (H+1)
    // cells reads outside the grid in the original (no bound check);
    // here it reads as a null record (−1).
    fn coord_index_at(&self, room: RoomId, x: i32, y: i32) -> i32 {
        let Some(at) = self.v.h.drlg.find_room(self.game, room, x, y) else {
            return 0;
        };
        self.coord_at(at, x, y).map_or(-1, |c| c.index)
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

    /// `0x0061AC10` → `0x00642380` (`levels.md` §11.5 item 3): the
    /// warp-room centres of the room's level (sub-tiles); none for a room
    /// without a DRLG room (fatal 0x5B9 in the original).
    fn warp_points(&self, room: RoomId) -> Vec<(i32, i32)> {
        self.v
            .h
            .drlg
            .drlg_room(self.game, room)
            .map_or(Vec::new(), |(d, r)| d.warp_points(r).to_vec())
    }

    /// `0x00619E50(act of the level, level id, kind)` → `0x0066B2B0`
    /// (`levels.md` §11.5 item 4, §10): the spawn-room choice for the
    /// room's level with all its effects; tiles, (−1, −1) when no room
    /// was found. A DRLG error is logged and reads as none.
    fn spawn_location(&mut self, room: RoomId, kind: u8) -> Option<(i32, i32)> {
        let level = self.room_level_id(room) as u32;
        let act = crate::drlg::act_of_level(level);
        let r = self
            .v
            .h
            .drlg
            .with_act(act, &mut self.game.lists, |d, svc| {
                if u32::from(kind) == crate::drlg::level::KIND11_TILE {
                    d.kind11_location(svc, level)
                } else {
                    d.spawn_room(svc, level, u32::from(kind))
                        .map(|p| (p.x, p.y))
                }
            })?;
        match r {
            Ok(p) => Some(p),
            Err(e) => {
                self.w
                    .errors
                    .push(WorldgenError::Wiring(WiringError::Drlg(e)));
                None
            }
        }
    }

    /// `0x00463740`: the room holding the point among `room` and its
    /// adjacency array (`rooms.md` §6).
    fn room_at(&self, room: RoomId, x: i32, y: i32) -> Option<RoomId> {
        self.v.h.drlg.find_room(self.game, room, x, y)
    }

    /// `0x0064D9B0` on the act's collision grids (`rooms.md` §10;
    /// `path-placement.md` §4 rules 1–5 with the path provider, as the
    /// missile adapter: size 0, 1 point, 2 plus, 3 box, other 0xFFFF; a
    /// cell without a room reads 0x27).
    // TODO(wire-action W5): without the path provider the sub-tile at
    // (x, y) is read for every size.
    fn collides(&self, room: RoomId, x: i32, y: i32, size: i32, mask: u16) -> bool {
        if self.v.h.paths.is_some() {
            return crate::path::collision::size_value(
                &self.v.h.drlg,
                Some(room),
                x,
                y,
                size,
                mask,
            ) != 0;
        }
        self.v.h.drlg.collision(self.game, room, x, y).unwrap_or(0) & mask != 0
    }

    /// `0x0064CB30` (`path-placement.md` §4 rules 1, 2 with the path
    /// provider).
    fn mask_at(&self, room: RoomId, x: i32, y: i32, mask: u16) -> bool {
        if self.v.h.paths.is_some() {
            return crate::path::collision::point_value(&self.v.h.drlg, Some(room), x, y, mask)
                != 0;
        }
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
