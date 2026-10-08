// Spec: specs/sim/units.md §3.3, §3.4 (wiring of the inactive store); specs/drlg/rooms.md §8 r6 (warp tiles)
//! The compress of tick step 9 (`0x005433F0`) and the restore of a
//! room's first population (`0x00542B40`) on the action wiring: the
//! rules of [`crate::units::inactive`] with the unit records, the stat
//! lists, the timer queue, the room lists and the DRLG; the facts and
//! re-creations without a d2-sim provider go to [`Pending`]'s
//! inactive-store seams.
//!
//! The store is [`ActionHooks::inactive`]: `None` (the default) keeps the
//! old behaviour (no compress, no restore) for every unit but warp tiles,
//! which go to [`ActionHooks::fallback_tiles`] (PROVISIONAL, REC-230);
//! [`ActionHooks::enable_inactive_store`] turns it on.
//!
//! The node key is the DRLG room's sub-tile origin (the rules order
//! nodes by the room origin's x, `0x00619730`; sub-tiles are tiles × 5,
//! so the order is the same).

use crate::game::Game;
use crate::units::inactive::{
    self as ia, Compress, CompressFacts, MonsterFacts, MonsterRecord, OtherRecord,
};
use crate::units::{RoomId, UnitId, UnitType};

use super::{ActionHooks, ActionSim, Pending, View};

/// State 7 (`playerbody`, §3.3).
const STATE_PLAYERBODY: u16 = 7;
/// Event types cancelled by the store (§3.3 table, §3.4 rule 1).
const EVENT_TYPE_1: u8 = 1;
const EVENT_TYPE_2: u8 = 2;
/// Stats of the monster record (§3.4 rule 1).
const STAT_13: u16 = 13;
const STAT_MAXLIFE: u16 = 7;
const STAT_LIFE: u16 = 6;

impl<X> ActionHooks<X> {
    /// Turns the inactive store on (empty).
    pub fn enable_inactive_store(&mut self) {
        self.inactive = Some(ia::InactiveStore::default());
    }
}

impl<X: Pending> View<'_, X> {
    /// The node key of a room: its act and sub-tile origin.
    fn node_key(&self, game: &Game, room: RoomId) -> Option<(u8, (i32, i32))> {
        use crate::path::CollisionRooms;
        let act = game.lists.room(room)?.act;
        let r = self.h.drlg.subtile_rect(room)?;
        Some((act, (r.x, r.y)))
    }

    fn monster_record(&self, game: &Game, u: UnitId) -> Option<MonsterRecord> {
        let r = self.units.get(u)?;
        let (x, y) = self.h.path_position(u);
        let level = game
            .lists
            .unit(u)
            .and_then(|e| e.room())
            .and_then(|rm| self.h.drlg.level_id(game, rm))
            .unwrap_or(0);
        let mut rec = MonsterRecord {
            x,
            y,
            class: r.class,
            guid: r.guid,
            unit_flags: r.flags,
            flags_ex: r.flags2,
            bits: ia::monster_bits(
                false,
                self.h.x.monster_flag(u, 4),
                r.mode,
                0,
                false,
                false,
                i32::from(self.h.x.alignment(u)),
                r.node_index,
                self.h.x.monster_flag(u, 2),
            ),
            owner_guid: -1,
            owner_value: 0,
            ai_state: 0,
            level,
            name_seed: 0x1506,
            umods: [0; 9],
            superunique: 0,
            stat13: self.stats.unit_total(u, STAT_13, 0),
            max_life: self.stats.unit_total(u, STAT_MAXLIFE, 0),
            life: self.stats.unit_base(u, STAT_LIFE, 0),
            frame: game.frame,
        };
        self.h.x.monster_record_extra(u, &mut rec);
        Some(rec)
    }

    /// `0x005433F0` for one unit of `room` (§3.3, §3.4 rules 1–3).
    pub fn compress_unit(&mut self, game: &mut Game, room: RoomId, u: UnitId) {
        let Some(r) = self.units.get(u) else {
            return;
        };
        let (ty, class, mode, flags, node) = (r.ty, r.class, r.mode, r.flags, r.node_index);
        let dead = r.is_dead();
        let save = ia::save_flag(self.h.x.save_monsters(game, room), flags);
        let c: Compress = if ty == UnitType::Monster {
            let f = MonsterFacts {
                save,
                mode,
                dead,
                udead_state: self.h.x.udead_state(u),
                alignment: i32::from(self.h.x.alignment(u)),
                node_index: node,
                class,
                type_flags: [8u32, 0x10]
                    .iter()
                    .filter(|&&m| self.h.x.monster_flag(u, m))
                    .fold(0, |a, m| a | m),
                unit_flags: flags,
                player_pet: self.h.x.player_pet(u),
                restore: self.h.x.monster_restore(u),
            };
            ia::compress_monster(&f, || self.h.x.room_seed_step(game, room))
        } else {
            let (nr, b78, virgins) = if ty == UnitType::Object {
                self.h.x.object_restore_facts(u)
            } else {
                (false, false, false)
            };
            ia::compress_other(&CompressFacts {
                ty,
                class,
                mode,
                save,
                player_body: ty == UnitType::Player && self.h.x.unit_has_state(u, STATE_PLAYERBODY),
                object_no_restore: nr,
                object_byte78_2: b78,
                object_restore_virgins: virgins,
            })
        };
        let key = self.node_key(game, room);
        if c.cancel_events {
            game.timers.cancel_unit_events(u, EVENT_TYPE_1, None);
        }
        if c.neutral_mode {
            if let Some(r) = self.units.get_mut(u) {
                r.mode = ia::MODE_NEUTRAL;
            }
        }
        if c.mark_kept {
            if let Some(r) = self.units.get_mut(u) {
                r.flags2 |= ia::FLAGS2_KEPT;
            }
        }
        if c.store {
            if let Some((act, origin)) = key {
                self.store_record(game, u, ty, act, origin);
            }
        }
        if ty == UnitType::Item && c.store {
            // §3.4 rule 2: the store frees the item.
            self.remove(game, u);
            return;
        }
        if c.detach {
            self.h.x.detach_path(u, room);
            let r = game.lists.room_remove(u);
            if let Err(e) = r {
                self.unit_error(crate::game::GameError::from(e).into());
            }
        } else if c.free {
            self.remove(game, u);
        }
    }

    fn store_record(&mut self, game: &mut Game, u: UnitId, ty: UnitType, act: u8, at: (i32, i32)) {
        match ty {
            UnitType::Monster => {
                game.timers.cancel_unit_events(u, EVENT_TYPE_2, None);
                if let Some(rec) = self.monster_record(game, u) {
                    if let Some(s) = self.h.inactive.as_mut() {
                        s.push_monster(act, at, rec);
                    }
                }
            }
            UnitType::Item => {
                if let Some(rec) = self.h.x.item_record(u) {
                    if let Some(s) = self.h.inactive.as_mut() {
                        s.push_item(act, at, rec);
                    }
                }
            }
            _ => {
                let Some(r) = self.units.get(u) else {
                    return;
                };
                let (x, y) = self.h.path_position(u);
                let mut rec = OtherRecord {
                    x,
                    y,
                    ty: ty as u8,
                    class: r.class,
                    mode: r.mode,
                    frame: game.frame,
                    unit_flags: r.flags,
                    flags_ex: r.flags2,
                    v20: if ty == UnitType::Player { r.guid } else { 0 },
                    ..OtherRecord::default()
                };
                if ty == UnitType::Object && ia::PORTAL_CLASSES.contains(&r.class) {
                    rec.v20 = r.guid;
                }
                self.h.x.other_record_extra(u, &mut rec);
                if let Some(s) = self.h.inactive.as_mut() {
                    s.push_other(act, at, rec);
                }
            }
        }
    }

    /// `0x00542B40(game, room)` (§3.4 rule 4).
    pub fn restore_inactive(&mut self, game: &mut Game, room: RoomId) {
        let Some((act, (x, y))) = self.node_key(game, room) else {
            return;
        };
        let Some(node) = self.h.inactive.as_mut().and_then(|s| s.take(act, x, y)) else {
            return;
        };
        let order = ia::restore_order(node);
        let gate = self.h.x.sanctuary_gate(game, room);
        for rec in &order.monsters {
            let how = ia::restore_monster(rec, gate);
            if how != ia::MonsterRestore::Skip {
                self.h.x.restore_monster(game, room, rec, how);
            }
        }
        for rec in &order.items {
            if let Some(expiry) = ia::restore_item_expiry(rec.expiry, game.frame) {
                self.h.x.restore_item(game, room, rec, expiry);
            }
        }
        for rec in &order.others {
            self.restore_other(game, room, rec);
        }
    }

    /// One other record of the restore (§3.4 rule 4.3): a tile is
    /// re-created here as a new unit (`rooms.md` §8 rule 6:
    /// `0x005557D0(5, class, x, y, mode, stored flags)`, flags |=
    /// 0x3000000); the other types go to [`Pending::restore_other`].
    fn restore_other(&mut self, game: &mut Game, room: RoomId, rec: &OtherRecord) {
        if rec.ty == UnitType::Tile as u8 {
            self.create_tile(
                game,
                room,
                rec.class,
                rec.x,
                rec.y,
                rec.mode,
                rec.unit_flags,
            );
        } else {
            self.h.x.restore_other(game, room, rec);
        }
    }

    /// Tick step 9 for a tile while the inactive store is off
    /// ([`ActionHooks::fallback_tiles`]): the tile's record (§3.4 rule 3)
    /// stored and the tile freed, as the store does (`rooms.md` §8 rule
    /// 6). PROVISIONAL (REC-230); d2rs-own, unverified.
    fn compress_fallback_tile(&mut self, game: &mut Game, room: RoomId, u: UnitId) {
        let Some((act, at)) = self.node_key(game, room) else {
            return;
        };
        let Some(r) = self.units.get(u) else {
            return;
        };
        let (x, y) = self.h.path_position(u);
        let rec = OtherRecord {
            x,
            y,
            ty: UnitType::Tile as u8,
            class: r.class,
            mode: r.mode,
            frame: game.frame,
            unit_flags: r.flags,
            flags_ex: r.flags2,
            ..OtherRecord::default()
        };
        self.h.fallback_tiles.push_other(act, at, rec);
        self.remove(game, u);
    }

    /// The restore of [`ActionHooks::fallback_tiles`] for `room`: its
    /// tiles re-created from the list head (reverse store order), new
    /// GUIDs, as the other records of §3.4 rule 4.3. The caller runs it
    /// after the host's restore (monsters and items first).
    /// PROVISIONAL (REC-230); d2rs-own, unverified.
    pub fn restore_fallback_tiles(&mut self, game: &mut Game, room: RoomId) {
        let Some((act, (x, y))) = self.node_key(game, room) else {
            return;
        };
        let Some(node) = self.h.fallback_tiles.take(act, x, y) else {
            return;
        };
        for rec in &ia::restore_order(node).others {
            self.restore_other(game, room, rec);
        }
    }
}

impl<X: Pending> ActionSim<X> {
    /// Tick step 9's compress (`0x005433F0`) when the store is on.
    /// Off: only tiles, into [`ActionHooks::fallback_tiles`].
    pub fn compress(&mut self, game: &mut Game, unit: UnitId) {
        let Some(room) = game.lists.unit(unit).and_then(|e| e.room()) else {
            return;
        };
        if self.sys.hooks.inactive.is_none() {
            if game
                .lists
                .unit(unit)
                .is_some_and(|e| e.ty == UnitType::Tile)
            {
                self.with(game, |g, v| v.compress_fallback_tile(g, room, unit));
            }
            return;
        }
        self.with(game, |g, v| v.compress_unit(g, room, unit));
    }

    /// The restore `0x00542B40` when the store is on; `false`: off.
    pub fn restore(&mut self, game: &mut Game, room: RoomId) -> bool {
        if self.sys.hooks.inactive.is_none() {
            return false;
        }
        self.with(game, |g, v| v.restore_inactive(g, room));
        true
    }
}
