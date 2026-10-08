// Spec: specs/render/draw-order.md (§3, §8, §9), specs/drlg/rooms.md (§6, §9.3), specs/drlg/levels.md (§11.3 step 9, §11.4), specs/sim/unit-order.md (§5 rules 6–8)
//! The §9 map-tile feed of `draw-order.md` from the client DRLG: the
//! near-room array of the local player's active room (its adjacency array,
//! `rooms.md` §6), per room the tile rectangle, sub-tile origin, the three
//! record arrays of its tile data with each record's DT1 header fields
//! (`rooms.md` §9.3 "Entry identity": roof height, height) and coordinate
//! record (`levels.md` §11.3 step 9), and the room's unit list in the
//! client's list order (`unit-order.md` §5 rule 6).
//!
//! The draw writes record flags (0x20000, the fade bits) and fade bytes
//! back into the records, where they persist between frames: [`MapState`]
//! keeps them per (room, array, record) while the room stays active. The
//! fill's Y sort of each room's unit list (`unit-order.md` §5 rule 7)
//! persists in the client's list: [`MapState::take_unit_orders`] hands
//! the sorted orders to the bridge.

use std::collections::BTreeMap;

use crate::bridge::drlg::DrlgRoomId;
use crate::bridge::world::{ClientWorld, LevelRow, UnitKey};
use crate::bridge::ClientUnit;
use crate::rules::draw_order::{
    Dt1Facts, Fade, LevelFacts, Logical, NearRooms, Room, RoomUnit, TileArray, TileRecord,
    TileRect, UnitFacts,
};

use super::ViewError;

const SPEC: &str = "render/draw-order.md";

/// A record's DT1 entry: the file's path and the tile index in file order
/// (`rooms.md` §9.3 "Entry identity"), the key of its block data.
pub type Dt1Entry = (Vec<u8>, u32);

/// The draw state of one record that persists between frames: flags
/// (+0x14) and fade bytes (+0x24…+0x2C).
type DrawState = (u32, Fade);

/// The map part of the model feed: the frame's near rooms and the draw
/// state the records keep between frames.
#[derive(Debug, Clone, Default)]
pub struct MapState {
    /// The bridge frame the near rooms were built for.
    stamp: Option<u64>,
    near: Option<NearRooms>,
    /// The DRLG room of each near room, array order.
    rooms: Vec<DrlgRoomId>,
    /// Per near room, per array (wall, floor, shadow), per record: its
    /// DT1 entry.
    entries: Vec<[Vec<Dt1Entry>; 3]>,
    draw: BTreeMap<(DrlgRoomId, u8, usize), DrawState>,
    /// Play preview only (decision D2): the local player's predicted
    /// 16.16 position. `None` (the strict path): the model's.
    pub local_at: Option<(UnitKey, (u32, u32))>,
}

fn array_slot(a: TileArray) -> u8 {
    match a {
        TileArray::Wall => 0,
        TileArray::Floor => 1,
        TileArray::Shadow => 2,
    }
}

fn unresolved(what: &'static str, message: String) -> ViewError {
    ViewError::Unresolved {
        what,
        spec: SPEC,
        message,
    }
}

impl MapState {
    /// The near rooms of the frame `world.frames`, built once per bridge
    /// frame (the order mutates them during the frame). `None`: no client
    /// DRLG or the local player has no room (no map).
    pub fn near_rooms(
        &mut self,
        world: &ClientWorld,
        levels: Option<&[LevelRow]>,
        facts: impl Fn(&ClientUnit) -> Result<UnitFacts, ViewError>,
    ) -> Result<Option<&mut NearRooms>, ViewError> {
        if self.stamp != Some(world.frames) {
            self.save();
            self.rebuild(world, levels, facts)?;
            self.stamp = Some(world.frames);
        }
        Ok(self.near.as_mut())
    }

    /// The DT1 entry of a record of near room `room`.
    pub fn entry(&self, room: usize, array: TileArray, record: usize) -> Option<&Dt1Entry> {
        self.entries
            .get(room)?
            .get(usize::from(array_slot(array)))?
            .get(record)
    }

    /// The DRLG room of near room `room`.
    pub fn room(&self, room: usize) -> Option<DrlgRoomId> {
        self.rooms.get(room).copied()
    }

    /// The unit lists the frame's fill sorted (`unit-order.md` §5 rule 7),
    /// for the client's lists; each is handed over once.
    pub fn take_unit_orders(&mut self) -> Vec<(DrlgRoomId, Vec<UnitKey>)> {
        let Some(near) = self.near.as_mut() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for (ri, room) in near.rooms.iter_mut().enumerate() {
            if room.units_sorted {
                room.units_sorted = false;
                out.push((self.rooms[ri], room.units.iter().map(|u| u.key).collect()));
            }
        }
        out
    }

    /// Keeps the last frame's record flags and fades.
    fn save(&mut self) {
        let Some(near) = &self.near else {
            return;
        };
        for (ri, room) in near.rooms.iter().enumerate() {
            let id = self.rooms[ri];
            for (array, recs) in [
                (TileArray::Wall, &room.walls),
                (TileArray::Floor, &room.floors),
                (TileArray::Shadow, &room.shadows),
            ] {
                for (i, r) in recs.iter().enumerate() {
                    self.draw
                        .insert((id, array_slot(array), i), (r.flags, r.fade));
                }
            }
        }
    }

    fn rebuild(
        &mut self,
        world: &ClientWorld,
        levels: Option<&[LevelRow]>,
        facts: impl Fn(&ClientUnit) -> Result<UnitFacts, ViewError>,
    ) -> Result<(), ViewError> {
        self.near = None;
        self.rooms.clear();
        self.entries.clear();
        let active = world.active_rooms.as_deref().unwrap_or(&[]);
        // A room no longer active has freed its records.
        self.draw
            .retain(|(id, _, _), _| active.iter().any(|a| a.room == *id));
        // The local player's cell: the model's, or in the preview its
        // predicted one.
        let predicted = self
            .local_at
            .filter(|(k, _)| world.local_player == Some(*k))
            .map(|(_, (x16, y16))| ((x16 >> 16) as u16, (y16 >> 16) as u16));
        // PROVISIONAL (client/model.md OQ2; REC-51): in the preview the
        // near rooms are those of the room holding the predicted position
        // (the cell lookup from the model's room, then the act lookup,
        // `model.md` §12 r2), as the original's own-path walk moves its
        // player's room; the model's room only when no active room holds
        // that point. The server sends the walking player nothing
        // (`pathing.md` §10 r2), so the model's room stays where the walk
        // started (and is freed by the room switch's 0x08 once the server
        // player leaves it): without this the map outside it is black.
        let own = match predicted {
            Some((x, y)) => world
                .room_from(world.local_room(), x, y)
                .or_else(|| world.local_room().copied()),
            None => world.local_room().copied(),
        };
        let (Some(cd), Some(own)) = (world.drlg.as_ref(), own) else {
            return Ok(());
        };
        let d = &cd.drlg;
        let mut rooms = Vec::new();
        for id in cd.adjacency(own.room) {
            let a = d
                .active_room(id)
                .ok_or_else(|| unresolved("near rooms", format!("room {id:?} is not active")))?;
            let r = d.room(id);
            let level = d.level(r.level).id;
            let mut room = Room {
                level,
                tiles: TileRect {
                    x: r.rect.x,
                    y: r.rect.y,
                    w: r.rect.w,
                    h: r.rect.h,
                },
                subtile_origin: (a.subtiles.x, a.subtiles.y),
                ..Room::default()
            };
            let mut entries: [Vec<Dt1Entry>; 3] = Default::default();
            if let Some(t) = r.tiles() {
                for (array, recs) in [
                    (TileArray::Wall, &t.walls),
                    (TileArray::Floor, &t.floors),
                    (TileArray::Shadow, &t.shadows),
                ] {
                    let slot = array_slot(array);
                    let mut out = Vec::with_capacity(recs.len());
                    for (i, rec) in recs.iter().enumerate() {
                        let info = d.tile_info(rec.tile);
                        let (flags, fade) = self
                            .draw
                            .get(&(id, slot, i))
                            .copied()
                            .unwrap_or((rec.flags, Fade::OPAQUE));
                        // Record +0x10: walls only (`levels.md` §11.3 step 9).
                        let logical = (array == TileArray::Wall)
                            .then(|| d.wall_coord(id, rec.x, rec.y))
                            .flatten()
                            .map(|c| Logical {
                                x0: c.boxr[0],
                                y0: c.boxr[1],
                                index: c.index as i32,
                            });
                        out.push(TileRecord {
                            tile: (rec.x, rec.y),
                            flags,
                            ty: rec.kind,
                            dt1: Dt1Facts {
                                orientation: info.orientation,
                                main: info.main,
                                sub: info.sub,
                                roof_height: i32::from(info.roof_height),
                                height: info.height,
                                material: info.material,
                            },
                            fade,
                            logical,
                        });
                        entries[usize::from(slot)]
                            .push((d.dt1_path(rec.tile).to_vec(), rec.tile.index));
                    }
                    match array {
                        TileArray::Wall => room.walls = out,
                        TileArray::Floor => room.floors = out,
                        TileArray::Shadow => room.shadows = out,
                    }
                }
            }
            for &key in world.room_units.list(id) {
                let unit = world.units.get(&key).ok_or_else(|| {
                    unresolved(
                        "room unit list",
                        format!("unit {key:?} listed but not in the model"),
                    )
                })?;
                room.units.push(RoomUnit {
                    key,
                    facts: facts(unit)?,
                });
            }
            rooms.push(room);
            self.rooms.push(id);
            self.entries.push(entries);
        }
        // PROVISIONAL (client/model.md OQ2; REC-51): in the preview the
        // local player is filed in the room of its predicted position, not
        // in its model room's list (left behind, or freed by 0x08), so the
        // draw order (§5) files it where it is drawn.
        if let (Some(_), Some(key), Some(unit)) = (predicted, world.local_player, world.local()) {
            let at = self.rooms.iter().position(|&id| id == own.room);
            let filed = at.is_some_and(|i| rooms[i].units.iter().any(|u| u.key == key));
            if let (false, Some(at)) = (filed, at) {
                for r in &mut rooms {
                    r.units.retain(|u| u.key != key);
                }
                rooms[at].units.push(RoomUnit {
                    key,
                    facts: facts(unit)?,
                });
            }
        }
        // `[0x007C8A08]` / `[0x007C8A10]`: the player's path sub-tile / 5;
        // `[0x007C8A0C]`: `0x0061B130` (`levels.md` §11.4) from the
        // player's room: none → 0, else the record index at the point.
        let (sx, sy) = predicted.unwrap_or_else(|| world.local().map_or((0, 0), ClientUnit::cell));
        let (sx, sy) = (i32::from(sx), i32::from(sy));
        let player_logical = match world.cell_lookup(&own, sx, sy) {
            None => 0,
            Some(r) => d.coord_at(r.room, sx, sy).map_or(-1, |c| c.index as i32),
        };
        let row = levels
            .ok_or_else(|| unresolved("level facts", "no Levels rows (DrawEdges)".into()))?
            .get(usize::from(own.level))
            .ok_or_else(|| {
                unresolved(
                    "level facts",
                    format!("level {} past the Levels rows", own.level),
                )
            })?;
        self.near = Some(NearRooms {
            rooms,
            player_tile: (sx / 5, sy / 5),
            player_logical,
            level: LevelFacts {
                id: u32::from(own.level),
                draw_edges: row.draw_edges,
                fade_geometric: false,
            },
        });
        Ok(())
    }
}
