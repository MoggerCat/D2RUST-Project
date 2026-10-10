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
//! The facts come from the game's tables where it has them (leveldefs
//! `SaveMonsters` and the `objects` rows of the object state, monstats2
//! `restore` of the action tables, the lent monster world's monster
//! data), else from the [`Pending`] seams. The restore re-creates the
//! units itself (allocation `0x00555230`, so a monster gets its type init
//! from the lent world and an object its init from the object state).
//!
//! The node key is the DRLG room's sub-tile origin (the rules order
//! nodes by the room origin's x, `0x00619730`; sub-tiles are tiles × 5,
//! so the order is the same).

use crate::game::Game;
use crate::units::inactive::{
    self as ia, Compress, CompressFacts, MonsterFacts, MonsterRecord, MonsterRestore, OtherRecord,
    SpawnKind,
};
use crate::units::lifecycle::AllocRequest;
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
/// Object event 5 (shrine regrow, §3.4 rule 3) and 2 (well refill,
/// `objects.md` §11).
const OBJECT_EVENT_5: u8 = 5;
const OBJECT_EVENT_2: u32 = 2;
/// Object classes whose GUID the record keeps (§3.4 rule 3).
const GUID_CLASSES: [u32; 3] = [59, 60, 100];
/// Unit flags `0x005557D0` sets on every unit it creates
/// (`population.md` §11.1).
const PRESET_UNIT_FLAGS: u32 = 0x300_0000;
/// Unit flag 0x10 a kept unit gets back (§3.4 rule 4.3).
const UNIT_FLAG_10: u32 = 0x10;
/// The mode a kept player body must be in (§3.4 rule 4.3; fatal 0x156).
const PLAYER_BODY_MODE: u32 = 0x11;
/// `objects` `SubClass` bits: shrine (`0x00621B00`), well (§3.4 rule 3).
const SUBCLASS_SHRINE: u8 = 0x1;
const SUBCLASS_WELL: u8 = 0x20;

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

    /// `S`'s level part: the room's level has `SaveMonsters` ≠ 0
    /// (leveldefs +0x94, `0x00642820`), from the object state's leveldefs;
    /// without them the host's answer.
    fn save_monsters(&self, game: &Game, room: RoomId) -> bool {
        let row = self.h.objects.as_ref().and_then(|o| {
            let level = self.h.drlg.level_id(game, room)?;
            o.tables.leveldefs.get(usize::try_from(level).ok()?)
        });
        match row {
            Some(d) => d.savemonsters != 0,
            None => self.h.x.save_monsters(game, room),
        }
    }

    /// monstats2 `restore` (+0x130) of the monster's class through monstats
    /// `MonStatsEx`; `None`: no row (§3.3 rule 8). Without monstats2 rows
    /// at all the host answers.
    fn monster_restore(&self, u: UnitId) -> Option<u8> {
        let t = &self.h.tables.combat;
        if t.monstats2.is_empty() {
            return self.h.x.monster_restore(u);
        }
        let class = self.units.get(u)?.class;
        let ex = t.monstats.get(usize::try_from(class).ok()?)?.monstatsex;
        t.monstats2.get(usize::from(ex)).map(|r| r.restore)
    }

    /// The objects row of an object unit (object state tables).
    fn object_row(&self, u: UnitId) -> Option<&d2_data::tables::Objects> {
        let class = u16::try_from(self.units.get(u)?.class).ok()?;
        self.h.objects.as_ref()?.tables.object(class).ok()
    }

    /// Objects: (`Restore` (+0x173) = 0, unit byte +0x78 has 0x2
    /// (`0x005540D0`), `RestoreVirgins` (+0x174) ≠ 0), from the object
    /// state; without it the host's answer.
    fn object_restore_facts(&self, u: UnitId) -> (bool, bool, bool) {
        let Some(row) = self.object_row(u) else {
            return self.h.x.object_restore_facts(u);
        };
        let spark = self
            .h
            .objects
            .as_ref()
            .and_then(|o| o.control.data.get(&u))
            .map_or(0, |d| d.spark);
        (row.restore == 0, spark & 0x2 != 0, row.restorevirgins != 0)
    }

    /// Monster type flags (+0x16) `& mask` (`0x005A0180`).
    fn type_flag(&self, u: UnitId, mask: u16) -> bool {
        self.h.monster_flag(u, u32::from(mask))
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
        use crate::monsters::init::type_flag as tf;
        let mut rec = MonsterRecord {
            x,
            y,
            class: r.class,
            guid: r.guid,
            unit_flags: r.flags,
            flags_ex: r.flags2,
            bits: ia::monster_bits(
                self.type_flag(u, tf::BOSS),
                self.type_flag(u, tf::CHAMPION),
                r.mode,
                0,
                self.type_flag(u, tf::MINION),
                false,
                i32::from(self.h.x.alignment(u)),
                r.node_index,
                self.type_flag(u, tf::SUPERUNIQUE),
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
        // The fields of the monster data (+0x2C level, +0x30 name seed,
        // +0x32 umods, +0x3C superunique index) when the world is lent.
        if let Some(m) = self.h.monster_data(u) {
            rec.level = u32::try_from(m.level_id).unwrap_or(0);
            rec.name_seed = m.name_seed;
            rec.umods = m.umods;
            rec.superunique = m.boss_hc_idx;
        }
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
        let save = ia::save_flag(self.save_monsters(game, room), flags);
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
                    .filter(|&&m| self.h.monster_flag(u, m))
                    .fold(0, |a, m| a | m),
                unit_flags: flags,
                player_pet: self.h.x.player_pet(u),
                restore: self.monster_restore(u),
            };
            ia::compress_monster(&f, || self.h.x.room_seed_step(game, room))
        } else {
            let (nr, b78, virgins) = if ty == UnitType::Object {
                self.object_restore_facts(u)
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
        let mut stored = false;
        if c.store {
            if let Some((act, origin)) = key {
                stored = self.store_record(game, u, ty, act, origin);
            }
        }
        if ty == UnitType::Item {
            // §3.4 rule 2: the store frees the item. An item without a
            // record (no item writer in this host) stays as §3.3 leaves it
            // ("not freed here"; the freed room unlinks it, `rooms.md` §8
            // rule 4). PROVISIONAL (REC-287).
            if stored {
                self.remove(game, u);
            }
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

    /// Pushes the unit's record (§3.4 rules 1–3); `false`: no record.
    fn store_record(
        &mut self,
        game: &mut Game,
        u: UnitId,
        ty: UnitType,
        act: u8,
        at: (i32, i32),
    ) -> bool {
        match ty {
            UnitType::Monster => {
                game.timers.cancel_unit_events(u, EVENT_TYPE_2, None);
                let Some(rec) = self.monster_record(game, u) else {
                    return false;
                };
                let Some(s) = self.h.inactive.as_mut() else {
                    return false;
                };
                s.push_monster(act, at, rec);
                true
            }
            UnitType::Item => {
                let Some(rec) = self.h.x.item_record(u) else {
                    return false;
                };
                let Some(s) = self.h.inactive.as_mut() else {
                    return false;
                };
                s.push_item(act, at, rec);
                true
            }
            _ => {
                let Some(r) = self.units.get(u) else {
                    return false;
                };
                let guid = r.guid;
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
                    v20: if ty == UnitType::Player { guid } else { 0 },
                    ..OtherRecord::default()
                };
                if ty == UnitType::Object
                    && !self.object_record(game, u, &mut rec)
                    && ia::PORTAL_CLASSES.contains(&rec.class)
                {
                    rec.v20 = guid;
                }
                self.h.x.other_record_extra(u, &mut rec);
                let Some(s) = self.h.inactive.as_mut() else {
                    return false;
                };
                s.push_other(act, at, rec);
                true
            }
        }
    }

    /// The object fields of an "other" record (§3.4 rule 3) from the
    /// object state; `false`: no object state or no data for `u`.
    fn object_record(&mut self, game: &mut Game, u: UnitId, rec: &mut OtherRecord) -> bool {
        let Some(row) = self.object_row(u).cloned() else {
            return false;
        };
        let Some(data) = self
            .h
            .objects
            .as_ref()
            .and_then(|o| o.control.data.get(&u))
            .copied()
        else {
            return false;
        };
        // A mode-1 object whose mode 1 does not cycle (`CycleAnim1`,
        // +0x109) and that has a mode 2 (`Mode2`, +0x141) is set to mode 2.
        if rec.mode == 1 && row.cycleanim1 == 0 && row.mode2 != 0 {
            if !self.object_set_mode(game, u, 2) {
                if let Some(r) = self.units.get_mut(u) {
                    r.mode = 2;
                }
            }
            rec.mode = 2;
        }
        if row.subclass & SUBCLASS_SHRINE != 0 {
            // The pending event 5 time (`0x005415A0`): high 16 bits in
            // +0x20, low 16 in +0x24.
            let t = &game.timers;
            let at = t
                .unit_timers(u)
                .into_iter()
                .filter(|&id| t.event(id).is_some_and(|e| e.0 == OBJECT_EVENT_5))
                .filter_map(|id| t.expire(id))
                .filter(|&e| e > 0)
                .min()
                .unwrap_or(0) as u32;
            rec.v20 = at >> 16;
            rec.v24 = at & 0xFFFF;
        } else if GUID_CLASSES.contains(&rec.class) {
            rec.v20 = data.guid;
        } else {
            rec.v24 = u32::from(data.spark);
        }
        rec.byte4 = data.interact;
        rec.b8 = data.drop_code;
        true
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
            self.restore_monster(game, room, rec, how);
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

    /// `0x00554A30`: the existing unit of `ty` and `guid` (a kept unit,
    /// detached by its compress) placed again in `room` at (x, y).
    fn replace_kept(
        &mut self,
        game: &mut Game,
        room: RoomId,
        ty: UnitType,
        guid: u32,
        x: i32,
        y: i32,
    ) -> Option<UnitId> {
        let u = game.lists.find_unit(ty, guid)?;
        if let Err(e) = game.lists.room_insert(u, room) {
            self.unit_error(crate::game::GameError::from(e).into());
            return None;
        }
        self.path_place(game, u, x, y);
        Some(u)
    }

    /// §3.4 rule 4.1 for one monster record.
    fn restore_monster(
        &mut self,
        game: &mut Game,
        room: RoomId,
        rec: &MonsterRecord,
        how: MonsterRestore,
    ) {
        use crate::monsters::init::type_flag as tf;
        use ia::mrec;
        let (kind, mode) = match how {
            MonsterRestore::Skip => return,
            MonsterRestore::Replace => {
                // PROVISIONAL (REC-287): the owner re-link `0x0058F350`
                // has no written body; the pet keeps its owner's lists.
                self.replace_kept(game, room, UnitType::Monster, rec.guid, rec.x, rec.y);
                return;
            }
            MonsterRestore::Spawn { kind, mode } => (kind, mode),
        };
        // Spawned again with the stored GUID; the allocation runs the
        // monster type init on the lent world (`init.md` §21: created
        // anew). PROVISIONAL (REC-287): the record's alignment (bit 0x100,
        // good) is the allocation's allied flag.
        let req = AllocRequest {
            ty: UnitType::Monster,
            class: rec.class,
            room: Some(room),
            add: true,
            fixed_guid: Some(rec.guid),
            mode,
            allied: rec.bits & mrec::ALIGN_2 != 0,
        };
        let Some(u) = self.allocate(game, &req, rec.x, rec.y) else {
            return;
        };
        // PROVISIONAL (REC-1560): the quest links of the boss mods
        // (`0x005B1CF0` `Chain` steps, `init.md` §14.3: the barbarian
        // cages' prison door 434 -> chain 32) come back with the unit; the
        // anew-created restore otherwise leaves a restored door unlinked
        // and its death never reaches the rescue quest. d2rs-own,
        // unverified: the original's restore path was not read for them.
        self.relink_boss_chains(u);
        if kind == SpawnKind::Plain {
            return;
        }
        // `init.md` §21: the saved umods copied in, the boss's champion
        // flag, superunique index and name seed, the minion's type flag
        // 0x10. PROVISIONAL (REC-287): the umod run (`0x005A2120`, umods
        // 1–4 and the list) is not repeated.
        let Some(m) = self.h.monster_world.as_mut().and_then(|w| w.monster_mut(u)) else {
            return;
        };
        m.umods = rec.umods;
        match kind {
            SpawnKind::Unique => {
                m.type_flags |= tf::BOSS;
                if rec.bits & mrec::CHAMPION != 0 {
                    m.type_flags |= tf::CHAMPION;
                }
                if rec.bits & mrec::SUPERUNIQUE != 0 {
                    m.type_flags |= tf::SUPERUNIQUE;
                    m.boss_hc_idx = rec.superunique;
                }
                m.name_seed = rec.name_seed;
            }
            SpawnKind::Minion => m.type_flags |= tf::MINION,
            SpawnKind::Plain => {}
        }
    }

    /// The `Chain` steps of the unit's boss mods (REC-1560).
    fn relink_boss_chains(&mut self, u: UnitId) {
        use crate::monsters::init::{boss_mods_for, BossStep};
        let Some(class) = self.units.get(u).map(|r| r.class) else {
            return;
        };
        let base = self
            .h
            .tables
            .combat
            .monstats
            .get(class as usize)
            .map_or(class as i32, |m| i32::from(m.baseid));
        for &step in boss_mods_for(base, class) {
            if let BossStep::Chain(n) = step {
                self.h.x.monster_quest_chain(u, n);
            }
        }
    }

    /// §3.4 rule 4.3 for one "other" record.
    fn restore_other(&mut self, game: &mut Game, room: RoomId, rec: &OtherRecord) {
        let Some(&ty) = UnitType::ALL.get(usize::from(rec.ty)) else {
            return;
        };
        if rec.flags_ex & ia::FLAGS2_KEPT != 0 {
            let Some(u) = self.replace_kept(game, room, ty, rec.v20, rec.x, rec.y) else {
                return;
            };
            if let Some(r) = self.units.get_mut(u) {
                r.flags |= UNIT_FLAG_10;
                if ty == UnitType::Player {
                    // Fatal 0x156 outside mode 0x11 in the original.
                    r.mode = PLAYER_BODY_MODE;
                }
            }
            if ty == UnitType::Object {
                if let Some(d) = self.object_data_mut(u) {
                    d.interact = rec.byte4;
                }
            }
            return;
        }
        // `0x005557D0(game, room, type, class, x, y, mode, flags)`: a new
        // unit (new GUID), unit flags 0x3000000. A tile keeps its stored
        // flags under them (`rooms.md` §8 rule 6, [`View::create_tile`]).
        if ty == UnitType::Tile {
            self.create_tile(
                game,
                room,
                rec.class,
                rec.x,
                rec.y,
                rec.mode,
                rec.unit_flags,
            );
            return;
        }
        let u = if ty == UnitType::Object {
            let mode = u8::try_from(rec.mode).unwrap_or(u8::MAX);
            match self.create_object(game, room, rec.class, rec.x, rec.y, mode) {
                Some(u) => Some(u),
                None if self.h.objects.is_none() => self.allocate_other(game, room, ty, rec),
                None => None,
            }
        } else {
            self.allocate_other(game, room, ty, rec)
        };
        let Some(u) = u else {
            return;
        };
        if let Some(r) = self.units.get_mut(u) {
            r.flags |= PRESET_UNIT_FLAGS;
        }
        if ty == UnitType::Object {
            self.restore_object(game, u, rec);
        }
    }

    fn allocate_other(
        &mut self,
        game: &mut Game,
        room: RoomId,
        ty: UnitType,
        rec: &OtherRecord,
    ) -> Option<UnitId> {
        let req = AllocRequest {
            ty,
            class: rec.class,
            room: Some(room),
            add: true,
            fixed_guid: None,
            mode: rec.mode,
            allied: false,
        };
        self.allocate(game, &req, rec.x, rec.y)
    }

    /// The object part of §3.4 rule 4.3 for a new object `u`.
    fn restore_object(&mut self, game: &mut Game, u: UnitId, rec: &OtherRecord) {
        let Some(row) = self.object_row(u).cloned() else {
            return;
        };
        let at = (rec.v20 << 16 | rec.v24 & 0xFFFF) as i32;
        if row.subclass & SUBCLASS_SHRINE != 0 && at > 0 {
            // Event 5 at max(t, frame + 1) (`0x005417D0`), the byte and its
            // shrine record (`0x00621BB0(unit, 0x006414B0(byte))`).
            let when = at.max(game.frame.wrapping_add(1));
            if let Err(e) = game.schedule_event(u, u32::from(OBJECT_EVENT_5), when, None, 0, 0) {
                self.unit_error(e.into());
            }
            let shrines = self
                .h
                .objects
                .as_ref()
                .map_or(0, |o| o.tables.shrines.len());
            if let Some(d) = self.object_data_mut(u) {
                d.interact = rec.byte4;
                d.shrine = (usize::from(rec.byte4) < shrines).then_some(u16::from(rec.byte4));
            }
        } else if row.subclass & SUBCLASS_WELL != 0 {
            // The byte regrows one per `Parm0` (+0x178) frames since the
            // store, capped at M = 2 · `Parm2` (`0x00552AA0`).
            // PROVISIONAL (REC-287): the spec names the event-2
            // re-schedules without their time: one refill event at frame +
            // `Parm0` + 1 (the operate's cadence, `objects.md` §11) while
            // below the cap; the mode stays the stored one.
            let period = i64::from(row.parm0);
            let cap = 2 * i64::from(row.parm2);
            let elapsed = i64::from(game.frame.wrapping_sub(rec.frame)).max(0);
            let grown = if period > 0 { elapsed / period } else { 0 };
            let byte = (i64::from(rec.byte4) + grown).min(cap.max(i64::from(rec.byte4)));
            if let Some(d) = self.object_data_mut(u) {
                d.interact = u8::try_from(byte).unwrap_or(u8::MAX);
            }
            if byte < cap && period > 0 {
                let when = game.frame.wrapping_add(row.parm0 as i32).wrapping_add(1);
                if let Err(e) = game.schedule_event(u, OBJECT_EVENT_2, when, None, 0, 0) {
                    self.unit_error(e.into());
                }
            }
        } else if let Some(d) = self.object_data_mut(u) {
            d.interact = rec.byte4;
            d.spark = rec.v24 as u8;
            if rec.b8 != 0 {
                d.drop_code = rec.b8;
            }
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
