// Spec: specs/render/draw-order.md (§3, §5, §8, §9), specs/render/draw-order-2.md (§14), specs/drlg/rooms.md (§6, §9.3), specs/drlg/levels.md (§11.3 step 9, §11.4), specs/sim/unit-order.md (§5 rules 6–8)
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
//! keeps them per (room, array, record) while the room stays active, and
//! the units' flag 0x10000000 and flag-ex 0x80 per unit while it lives
//! (`draw-order.md` §5 r3: the sight test of frame N gates the shadow
//! entry of frame N + 1). The
//! fill's Y sort of each room's unit list (`unit-order.md` §5 rule 7)
//! persists in the client's list: [`MapState::take_unit_orders`] hands
//! the sorted orders to the bridge.

use std::collections::BTreeMap;

use crate::bridge::drlg::DrlgRoomId;
use crate::bridge::world::{ClientWorld, LevelRow, UnitKey};
use crate::bridge::ClientUnit;
use crate::rules::draw_order::{
    Dt1Facts, Fade, LevelFacts, Logical, NearRooms, Room, RoomUnit, TileArray, TileRecord,
    TileRect, UnitFacts, UNIT_DRAWN, UNIT_EX_VISIBLE,
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
    /// The unit bits the order writes and the next frame reads
    /// (`draw-order.md` §5 r3, r4): flag 0x10000000 and flag-ex 0x80 of
    /// each unit, as the last frame left them.
    unit_draw: BTreeMap<UnitKey, (u32, u32)>,
    /// The DT1 entry of the act's edge floor record (`draw-order-2.md`
    /// §14), `None` in acts IV and V.
    edge: Option<Dt1Entry>,
}

fn array_slot(a: TileArray) -> u8 {
    match a {
        TileArray::Wall => 0,
        TileArray::Floor => 1,
        TileArray::Shadow => 2,
        TileArray::Edge => 3,
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
    /// DRLG or the local player has no room (no map). `player` is the
    /// frame's one local-player sub-tile (`seams/world-screen.md` §2.4:
    /// the position the camera is built from); `None`: the model cell.
    pub fn near_rooms(
        &mut self,
        world: &ClientWorld,
        levels: Option<&[LevelRow]>,
        player: Option<(i32, i32)>,
        facts: impl Fn(&ClientUnit) -> Result<UnitFacts, ViewError>,
    ) -> Result<Option<&mut NearRooms>, ViewError> {
        if self.stamp != Some(world.frames) {
            self.save();
            self.rebuild(world, levels, player, facts)?;
            self.stamp = Some(world.frames);
        }
        Ok(self.near.as_mut())
    }

    /// The DT1 entry of a record of near room `room`.
    pub fn entry(&self, room: usize, array: TileArray, record: usize) -> Option<&Dt1Entry> {
        if array == TileArray::Edge {
            return self.edge.as_ref();
        }
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

    /// Keeps the last frame's record flags and fades, and the units' draw
    /// bits.
    fn save(&mut self) {
        let Some(near) = &self.near else {
            return;
        };
        for (ri, room) in near.rooms.iter().enumerate() {
            for u in &room.units {
                self.unit_draw.insert(
                    u.key,
                    (
                        u.facts.flags & UNIT_DRAWN,
                        u.facts.flag_ex & UNIT_EX_VISIBLE,
                    ),
                );
            }
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
        player: Option<(i32, i32)>,
        facts: impl Fn(&ClientUnit) -> Result<UnitFacts, ViewError>,
    ) -> Result<(), ViewError> {
        self.near = None;
        self.rooms.clear();
        self.entries.clear();
        self.edge = None;
        let active = world.active_rooms.as_deref().unwrap_or(&[]);
        // A room no longer active has freed its records.
        self.draw
            .retain(|(id, _, _), _| active.iter().any(|a| a.room == *id));
        // A freed unit's bits go with it.
        self.unit_draw.retain(|k, _| world.units.contains_key(k));
        let (Some(cd), Some(own)) = (world.drlg.as_ref(), world.local_room().copied()) else {
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
                        TileArray::Shadow | TileArray::Edge => room.shadows = out,
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
                let mut f = facts(unit)?;
                // `draw-order.md` §5 r3, r4: the bits of the last frame's
                // draw (the sight test's 0x80 gates this frame's shadow).
                if let Some(&(flags, ex)) = self.unit_draw.get(&key) {
                    f.flags = f.flags & !UNIT_DRAWN | flags;
                    f.flag_ex = f.flag_ex & !UNIT_EX_VISIBLE | ex;
                }
                room.units.push(RoomUnit { key, facts: f });
            }
            rooms.push(room);
            self.rooms.push(id);
            self.entries.push(entries);
        }
        // `[0x007C8A08]` / `[0x007C8A10]`: the player's path sub-tile / 5;
        // `[0x007C8A0C]`: `0x0061B130` (`levels.md` §11.4) from the
        // player's room: none → 0, else the record index at the point.
        // The path sub-tile is the frame's one player position (the drawn
        // player's, `seams/world-screen.md` §2.4), not the model cell.
        let (sx, sy) = player.unwrap_or_else(|| {
            let (x, y) = world.local().map_or((0, 0), ClientUnit::cell);
            (i32::from(x), i32::from(y))
        });
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
        // The act's edge floor record (`draw-order-2.md` §14, open question
        // 2): its lookup failing matters only to a level that draws edges.
        let edge = match cd.edge_tile() {
            Ok(t) => t,
            Err(m) if row.draw_edges => return Err(unresolved("act edge record", m)),
            Err(_) => None,
        };
        let edge = edge.map(|t| {
            self.edge = Some((t.path, t.index));
            // Filled at position 0 from a zeroed record (`0x0066DDE0`).
            TileRecord {
                tile: (0, 0),
                flags: 0,
                ty: 0,
                dt1: Dt1Facts {
                    orientation: t.info.orientation,
                    main: t.info.main,
                    sub: t.info.sub,
                    roof_height: i32::from(t.info.roof_height),
                    height: t.info.height,
                    material: t.info.material,
                },
                fade: Fade::OPAQUE,
                logical: None,
            }
        });
        self.near = Some(NearRooms {
            rooms,
            player_tile: (sx / 5, sy / 5),
            player_subtile: (sx, sy),
            edge,
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
