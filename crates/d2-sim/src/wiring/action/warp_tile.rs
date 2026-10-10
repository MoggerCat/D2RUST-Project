// Spec: specs/sim/path-placement.md §12.1, §12.2 (warp tiles); specs/drlg/rooms.md §8 r6 (warp tiles across deactivation); specs/monsters/population.md §11.1 (first preset walk); specs/sim/units.md §3.4 r4 (restore of other records); specs/drlg/levels.md §10.4 (preset units)
//! Warp tile units and the C→S 0x13 tile case.
//!
//! A level warp is a tile unit (unit type 5, class = the lvlwarp `Id`):
//! the DRLG's warp tile preset (`0x0066E1C0`, §12.1) adds it to the
//! room's preset units; the population preset pass `0x005559A0` hands the
//! list out once per DRLG room ([`View::take_presets`], flag 0x4000000,
//! `rooms.md` §8 rule 6) and its first walk creates the tiles before any
//! preset monster ([`View::spawn_preset_units`]); a deactivated room's
//! tiles are stored and freed, and its restore re-creates them as new
//! units ([`View::create_tile`]); [`View::warp_tile_message`] runs the
//! walk into the warp (`0x005550B0`, §12.2) for a C→S 0x13 whose unit
//! type is 5.
//!
//! The first walk creates the tiles with the room's other non-monster
//! presets, in list order ([`View::spawn_preset_units`], `drlg/rooms.md`
//! §6 "First spawn").
//! PROVISIONAL (REC-99): no spec says what the 0x13 handler checks before
//! `0x005550B0` (`path-placement.md` §12.2 names only the caller
//! `0x00548C32`). d2rs runs the warp when the tile exists; the result is
//! 0 for a warp run, else 1.
// d2rs-own, unverified (the 0x13 case only)

use crate::drlg::room_flags;
use crate::game::Game;
use crate::units::lifecycle::AllocRequest;
use crate::units::{RoomId, UnitId, UnitType};
use crate::wiring::path::{place, PathCtx};

use super::{Pending, View};

/// The unit type of a host-placed monster preset: a level types provider's
/// own preset-list entry (class = monstats class) that population does not
/// read (it takes type 1). PROVISIONAL (REC-124); d2rs-own, unverified.
pub const HOST_MONSTER_PRESET: u32 = 6;
/// Preset unit type for an object the host's level types provider lists
/// (q-a2-duriel, REC-167; d2rs-own, unverified): created through the
/// object state's `create_object` (allocation and the §3 init).
pub const HOST_OBJECT_PRESET: u32 = 7;

/// The unit type of an object preset (DS1 object entries).
pub const OBJECT_PRESET: u32 = 2;

/// The object classes whose preset path the quests keep as a map AI
/// (`quests-act5.md` §5.8: 459 Anya's dummy, 461 Nihlathak's, 543
/// Larzuk's).
const MAP_AI_STORE_CLASSES: [u32; 3] = [459, 461, 543];

/// The unit type of a tile preset (`levels.md` §10.4).
const TILE_PRESET: u32 = crate::path::warp::TILE_UNIT_TYPE as u32;

/// Unit flags `0x005557D0` sets on every unit it creates
/// (`population.md` §11.1; `rooms.md` §8 rule 6 for tiles).
pub const CREATED_UNIT_FLAGS: u32 = 0x300_0000;

impl<X: Pending> View<'_, X> {
    /// `0x00619FD0` → `0x0066BFA0` (`rooms.md` §8 rule 6): whether the
    /// active `room`'s DRLG room hands its preset list out. A client copy
    /// always does; else the first call sets flag 0x4000000 and does,
    /// every later call finds the flag and gets none (no code clears it).
    /// A room without a DRLG room has no list; `true` then (nothing to
    /// hand out either way).
    pub fn take_presets(&mut self, game: &mut Game, room: RoomId) -> bool {
        let Some(act) = game.lists.room(room).map(|r| r.act) else {
            return true;
        };
        self.h
            .drlg
            .with_act(act, &mut game.lists, |d, _| {
                let Some(r) = d.drlg_room_of(room) else {
                    return true;
                };
                if d.on_client {
                    return true;
                }
                let flags = &mut d.room_mut(r).flags;
                if *flags & room_flags::PRESETS_HANDED_OUT != 0 {
                    return false;
                }
                *flags |= room_flags::PRESETS_HANDED_OUT;
                true
            })
            .unwrap_or(true)
    }

    /// The type-5 presets of the active `room`'s DRLG room (list order,
    /// head first): (class, absolute sub-tile x, y).
    fn tile_presets(&mut self, game: &mut Game, room: RoomId) -> Vec<(u32, i32, i32)> {
        let Some(act) = game.lists.room(room).map(|r| r.act) else {
            return Vec::new();
        };
        let found = self.h.drlg.with_act(act, &mut game.lists, |d, svc| {
            let r = d.drlg_room_of(room)?;
            let origin = d.active_room(r)?.subtiles;
            Some((svc.types.preset_units(d, r), origin))
        });
        let Some(Some((units, origin))) = found else {
            return Vec::new();
        };
        units
            .iter()
            .filter(|p| p.unit_type == TILE_PRESET)
            .map(|p| (p.class, origin.x + p.x, origin.y + p.y))
            .collect()
    }

    /// One type-5 preset: a tile unit at (x, y) ([`Self::create_tile`],
    /// mode 0, flags 0: §12.1 adds no done bit) unless the room has one of
    /// that class there. True when allocated.
    fn spawn_tile_preset(
        &mut self,
        game: &mut Game,
        room: RoomId,
        (x, y): (i32, i32),
        class: u32,
    ) -> bool {
        if self.room_tile_at(game, room, class, (x, y)).is_some() {
            return false;
        }
        self.create_tile(game, room, class, x, y, 0, 0).is_some()
    }

    /// `0x005557D0` sets unit flags 0x3000000 on every unit it creates
    /// (`monsters/population.md` §11.1, `drlg/rooms.md` §8 rule 6): flag
    /// 0x2000000 makes the unit `S` of the compress (`sim/units.md` §3.3),
    /// so a preset object of a level without `SaveMonsters` (the towns)
    /// is stored when its room is freed and restored by the room's next
    /// population. True when `made`.
    fn mark_preset_unit(&mut self, made: Option<UnitId>) -> bool {
        let Some(u) = made else {
            return false;
        };
        if let Some(r) = self.units.get_mut(u) {
            r.flags |= CREATED_UNIT_FLAGS;
        }
        true
    }

    /// `0x00545C90` (`quests-act5.md` §5.8): the quest map-AI store of a
    /// preset object with a path, on the lent quest host.
    fn map_ai_store(&mut self, game: &mut Game, class: u16, path: Vec<(u32, i32, i32)>) {
        let Some(mut host) = self.h.quest_host.take() else {
            return;
        };
        host.map_ai_store(game, self, class, path);
        self.h.quest_host = Some(host);
    }

    /// One type-2 preset: an object at (x, y) through the object state's
    /// `create_object`, unless the room has one of that class there. True
    /// when created.
    fn spawn_object_preset(
        &mut self,
        game: &mut Game,
        room: RoomId,
        (x, y): (i32, i32),
        class: u32,
    ) -> bool {
        let exists = game.lists.room_units(room).into_iter().any(|u| {
            game.lists.unit(u).is_some_and(|e| e.ty == UnitType::Object)
                && self.units.get(u).is_some_and(|r| r.class == class)
                && self.h.path_position(u) == (x, y)
        });
        if exists {
            return false;
        }
        let made = self.create_object(game, room, class, x, y, 0);
        self.mark_preset_unit(made)
    }

    /// The first walk of `0x005559A0` (`drlg/rooms.md` §6 "First spawn",
    /// `rooms.md` §8 rule 6, `population.md` §11.1; settles REC-99's
    /// "where"): every non-monster preset of the active `room`'s DRLG room
    /// in list order (head first; warp tiles are prepended,
    /// `path-placement.md` §12.1 rule 3, so the last-added warp comes
    /// first): type 2 an object ([`Self::spawn_object_preset`]), type 5 a
    /// tile ([`Self::spawn_tile_preset`]). Before the monster walk; the
    /// caller has taken the list ([`Self::take_presets`]). Returns the
    /// number created.
    ///
    /// PROVISIONAL (REC-99): a preset whose unit (same class and
    /// position) is already in the room is skipped; for a tile only
    /// [`Self::spawn_missing_tile`] (the warp arrival into a room not yet
    /// populated) makes one. d2rs-own, unverified.
    pub fn spawn_preset_units(&mut self, game: &mut Game, room: RoomId) -> usize {
        let Some(act) = game.lists.room(room).map(|r| r.act) else {
            return 0;
        };
        let found = self.h.drlg.with_act(act, &mut game.lists, |d, svc| {
            let r = d.drlg_room_of(room)?;
            let origin = d.active_room(r)?.subtiles;
            let init_seed = d.init_seed;
            let units = svc.types.preset_units(d, r);
            let paths: Vec<_> = units
                .iter()
                .enumerate()
                .map(|(i, u)| {
                    if u.unit_type == OBJECT_PRESET && MAP_AI_STORE_CLASSES.contains(&u.class) {
                        svc.types.unit_path(d, r, i)
                    } else {
                        None
                    }
                })
                .collect();
            Some((units, paths, origin, init_seed))
        });
        let Some(Some((units, paths, origin, init_seed))) = found else {
            return 0;
        };
        let level = self.h.drlg.level_id(game, room).unwrap_or(0);
        let mut n = 0;
        for (p, path) in units.iter().zip(paths) {
            if p.unit_type == OBJECT_PRESET
                && preset_object_skipped(game, level, p.class, init_seed)
            {
                continue;
            }
            let at = (origin.x + p.x, origin.y + p.y);
            let made = match p.unit_type {
                OBJECT_PRESET => {
                    let made = self.spawn_object_preset(game, room, at, p.class);
                    if let (true, Some(path)) = (made, path) {
                        self.map_ai_store(game, p.class as u16, path);
                    }
                    made
                }
                TILE_PRESET => self.spawn_tile_preset(game, room, at, p.class),
                _ => false,
            };
            n += usize::from(made);
        }
        n
    }

    /// `0x005557D0(game, room, 5, class, x, y, mode, flags)` → `0x00555230`
    /// (`rooms.md` §8 rule 6): a new tile unit (new GUID), then unit flags
    /// |= 0x3000000 over `flags`.
    #[allow(clippy::too_many_arguments)]
    pub fn create_tile(
        &mut self,
        game: &mut Game,
        room: RoomId,
        class: u32,
        x: i32,
        y: i32,
        mode: u32,
        flags: u32,
    ) -> Option<UnitId> {
        let req = AllocRequest {
            ty: UnitType::Tile,
            class,
            room: Some(room),
            add: true,
            fixed_guid: None,
            mode,
            allied: false,
        };
        let u = self.allocate(game, &req, x, y)?;
        if let Some(r) = self.units.get_mut(u) {
            r.flags |= flags | CREATED_UNIT_FLAGS;
        }
        Some(u)
    }

    /// The warp arrival's tile in a destination room that is active but
    /// not yet populated (`path-placement.md` §12.2 rule 1.3 finds none
    /// there): the first type-5 preset of `class` of the room's list,
    /// created now. PROVISIONAL (REC-99): 1.14d returns no tile and the
    /// warp does nothing; d2rs runs the 0x13 warp from any distance
    /// (see the module note), so the far room can be unpopulated.
    /// d2rs-own, unverified.
    pub fn spawn_missing_tile(
        &mut self,
        game: &mut Game,
        room: RoomId,
        class: u32,
    ) -> Option<UnitId> {
        let (_, x, y) = self
            .tile_presets(game, room)
            .into_iter()
            .find(|&(c, _, _)| c == class)?;
        self.create_tile(game, room, class, x, y, 0, 0)
    }

    /// A tile unit of `class` at `at` in the active `room`'s unit list.
    fn room_tile_at(
        &self,
        game: &Game,
        room: RoomId,
        class: u32,
        at: (i32, i32),
    ) -> Option<UnitId> {
        game.lists.room_units(room).into_iter().find(|&u| {
            game.lists.unit(u).is_some_and(|e| e.ty == UnitType::Tile)
                && self.units.get(u).is_some_and(|r| r.class == class)
                && self.h.path_position(u) == at
        })
    }

    /// Allocates a monster for each [`HOST_MONSTER_PRESET`] entry of the
    /// active `room`'s DRLG room that has none yet (same class), as the
    /// tiles are. Returns the created (unit, class) pairs.
    pub fn spawn_host_monsters(&mut self, game: &mut Game, room: RoomId) -> Vec<(UnitId, u32)> {
        let Some(act) = game.lists.room(room).map(|r| r.act) else {
            return Vec::new();
        };
        let found = self.h.drlg.with_act(act, &mut game.lists, |d, svc| {
            let r = d.drlg_room_of(room)?;
            let origin = d.active_room(r)?.subtiles;
            Some((svc.types.preset_units(d, r), origin))
        });
        let Some(Some((units, origin))) = found else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for p in units.iter().filter(|p| p.unit_type == HOST_MONSTER_PRESET) {
            let exists = game.lists.room_units(room).into_iter().any(|u| {
                game.lists
                    .unit(u)
                    .is_some_and(|e| e.ty == UnitType::Monster)
                    && self.units.get(u).is_some_and(|r| r.class == p.class)
            });
            if exists {
                continue;
            }
            let req = AllocRequest {
                ty: UnitType::Monster,
                class: p.class,
                room: Some(room),
                add: true,
                fixed_guid: None,
                mode: 1,
                allied: false,
            };
            if let Some(u) = self.allocate(game, &req, origin.x + p.x, origin.y + p.y) {
                if let Some(r) = self.units.get_mut(u) {
                    r.flags |= crate::missiles::unit_flag::IS_VALID_TARGET
                        | crate::missiles::unit_flag::CAN_BE_ATTACKED;
                }
                out.push((u, p.class));
            }
        }
        out
    }

    /// Creates an object for each [`HOST_OBJECT_PRESET`] entry of the
    /// active `room`'s DRLG room that has none yet (same class).
    pub fn spawn_host_objects(&mut self, game: &mut Game, room: RoomId) {
        let Some(act) = game.lists.room(room).map(|r| r.act) else {
            return;
        };
        let found = self.h.drlg.with_act(act, &mut game.lists, |d, svc| {
            let r = d.drlg_room_of(room)?;
            let origin = d.active_room(r)?.subtiles;
            Some((svc.types.preset_units(d, r), origin))
        });
        let Some(Some((units, origin))) = found else {
            return;
        };
        for p in units.iter().filter(|p| p.unit_type == HOST_OBJECT_PRESET) {
            let exists = game.lists.room_units(room).into_iter().any(|u| {
                game.lists.unit(u).is_some_and(|e| e.ty == UnitType::Object)
                    && self.units.get(u).is_some_and(|r| r.class == p.class)
            });
            if !exists {
                self.create_object(game, room, p.class, origin.x + p.x, origin.y + p.y, 0);
            }
        }
    }

    /// The first tile unit of `class` in the active `room`'s unit list.
    pub fn room_tile(&self, game: &Game, room: RoomId, class: u32) -> Option<UnitId> {
        game.lists.room_units(room).into_iter().find(|&u| {
            game.lists.unit(u).is_some_and(|e| e.ty == UnitType::Tile)
                && self.units.get(u).is_some_and(|r| r.class == class)
        })
    }

    /// The C→S 0x13 tile case: the tile with `guid` and the walk into its
    /// warp for `player`. `None`: no path provider. 0: the warp ran
    /// ([`WarpOutcome::Arrived`]); 1: nothing happened.
    ///
    /// [`WarpOutcome::Arrived`]: crate::path::warp::WarpOutcome::Arrived
    pub fn warp_tile_message(&mut self, game: &mut Game, player: UnitId, guid: u32) -> Option<u32> {
        self.h.paths.as_ref()?;
        let Some(tile) = game.lists.find_unit(UnitType::Tile, guid) else {
            return Some(1);
        };
        let room = game.lists.unit(tile).and_then(|e| e.room());
        let class = self.units.get(tile).map(|r| r.class);
        let (Some(room), Some(class)) = (room, class) else {
            return Some(1);
        };
        let c = PathCtx::of(self, game);
        let r = place::warp_player(c, player, room, class);
        Some(u32::from(
            r != Some(crate::path::warp::WarpOutcome::Arrived),
        ))
    }
}

/// The first walk's per-level object skips of `0x005559A0`
/// (`monsters/population.md` §11.1): levels 133 and 135 never place
/// object 397, level 134 never places the waypoint 402 and drops the
/// object preset that brings the game's count ([`Game::sands_preset_objects`])
/// to `init seed % 3 + 3`, level 136 never places objects 26, 268 and 269.
fn preset_object_skipped(game: &mut Game, level: u32, class: u32, init_seed: u32) -> bool {
    match level {
        133 | 135 => class == 397,
        134 => {
            if class == 402 {
                return true;
            }
            game.sands_preset_objects = game.sands_preset_objects.wrapping_add(1);
            game.sands_preset_objects == init_seed % 3 + 3
        }
        136 => matches!(class, 26 | 268 | 269),
        _ => false,
    }
}

#[cfg(test)]
mod preset_skip_tests {
    use super::*;

    #[test]
    fn levels_133_135_136_skip_their_objects() {
        let mut g = Game::default();
        assert!(preset_object_skipped(&mut g, 133, 397, 0));
        assert!(preset_object_skipped(&mut g, 135, 397, 0));
        assert!(!preset_object_skipped(&mut g, 133, 398, 0));
        for c in [26, 268, 269] {
            assert!(preset_object_skipped(&mut g, 136, c, 0));
        }
        assert!(!preset_object_skipped(&mut g, 136, 397, 0));
        assert!(!preset_object_skipped(&mut g, 37, 397, 0));
        assert_eq!(g.sands_preset_objects, 0);
    }

    #[test]
    fn level_134_skips_the_waypoint_and_the_counted_object() {
        // init seed 7: 7 % 3 + 3 = 4, so the fourth counted object goes.
        let mut g = Game::default();
        assert!(preset_object_skipped(&mut g, 134, 402, 7));
        let skipped: Vec<bool> = (0..6)
            .map(|i| preset_object_skipped(&mut g, 134, 100 + i, 7))
            .collect();
        assert_eq!(skipped, [false, false, false, true, false, false]);
        assert_eq!(g.sands_preset_objects, 6);
    }
}
