// Spec: specs/missiles/missiles.md §R2, §R4, §R5, §R6, §R9.5, §R9.6; specs/sim/pathing.md §13.3 (line_hits); specs/monsters/init.md §22 (seams `MissileUnits`, `MissilePath`, `MissileRooms`, `MissileCombat`, `MissileHooks`, `MissileBodies`)
//! Missiles ↔ units, DRLG and combat: [`View`] implements
//! [`crate::missiles::MissileWorld`]. Real providers: unit allocation and
//! removal (`units.md` §3), seeds, stats, states and state lists
//! (`stat-lists.md`), unit flags, the hireling test, the `justhit` list
//! and its type-12 timer (`tick.md` §5.2); the room search, town test and
//! collision grids of the DRLG (`rooms.md` §10); the to-hit test
//! (`hit.md` §3) and the damage application (`damage.md` §5.2). Path,
//! hostility, skill setup and server-damage functions go to [`Pending`].

use crate::combat::{self, result, DamageRecord};
use crate::game::Game;
use crate::missiles::{
    result_flag, stat, Damage, MissileCombat, MissileData, MissileHooks, MissilePath, MissileRooms,
    MissileUnits,
};
use crate::rng::Seed;
use crate::tick::events::event;
use crate::units::hooks::Sim;
use crate::units::lifecycle::AllocRequest;
use crate::units::{RoomId, UnitId, UnitType};

use super::combat::CombatView;
use super::monsters::umod_mode;
use super::units::STATE_JUSTHIT;
use super::{Pending, View, WiringError};

/// Pierce percent stat of the damage record (`missiles.md` §R6.1:
/// "pierce percent = stat 327").
const PIERCE_PERCENT_STAT: u16 = stat::DAMAGE_FRAMERATE;
/// Armor class (`damage.md` §5.2: stat 31 via `0x00627260`).
const ARMORCLASS: u16 = 31;

impl<X: Pending> View<'_, X> {
    /// A [`CombatView`] on `game` over this view's parts.
    pub fn combat<'b>(&'b mut self, game: &'b mut Game) -> CombatView<'b, X> {
        CombatView {
            game,
            v: View {
                units: &mut *self.units,
                stats: &mut *self.stats,
                data: self.data,
                h: &mut *self.h,
            },
        }
    }

    fn unit_room(game: &Game, u: UnitId) -> Option<RoomId> {
        game.lists.unit(u).and_then(|e| e.room())
    }
}

impl<X: Pending> MissileUnits for View<'_, X> {
    fn alloc_missile(
        &mut self,
        game: &mut Game,
        class: u16,
        x: i32,
        y: i32,
        room: RoomId,
        mode: u8,
    ) -> Option<UnitId> {
        // TODO(unit-order.md §5.2): whether a missile counts toward the
        // room's allied count is not stated; it does not.
        let req = AllocRequest {
            ty: UnitType::Missile,
            class: u32::from(class),
            room: Some(room),
            add: true,
            fixed_guid: None,
            mode: u32::from(mode),
            allied: false,
        };
        self.allocate(game, &req, x, y)
    }
    fn remove_unit(&mut self, game: &mut Game, unit: UnitId) {
        self.remove(game, unit);
    }
    fn seed(&mut self, unit: UnitId) -> &mut Seed {
        View::seed(self, unit)
    }
    fn position(&self, unit: UnitId) -> (i32, i32) {
        self.h.path_position(unit)
    }
    fn size(&self, unit: UnitId) -> i32 {
        self.path_size(unit)
    }
    fn stat(&self, unit: UnitId, s: u16) -> i32 {
        View::stat(self, unit, s)
    }
    fn base_stat(&self, unit: UnitId, s: u16) -> i32 {
        self.stats.unit_base(unit, s, 0)
    }
    fn set_stat(&mut self, unit: UnitId, s: u16, value: i32) {
        self.set_base(unit, s, value);
    }
    fn has_state(&self, unit: UnitId, s: u16) -> bool {
        self.stats.has_state(unit, u32::from(s))
    }
    fn state_stat(&self, unit: UnitId, s: u16, st: u16) -> Option<i32> {
        View::state_stat(self, unit, s, st)
    }
    /// `0x00626D40` for the missile: unit allocation already made it
    /// (`stat-lists.md` §4 rule 3); nothing more.
    fn alloc_stat_list(&mut self, _: UnitId) {}
    fn unit_flag(&self, unit: UnitId, bit: u32) -> bool {
        self.units.get(unit).is_some_and(|r| r.flags & bit != 0)
    }
    fn set_unit_flag(&mut self, unit: UnitId, bit: u32, on: bool) {
        if let Some(r) = self.units.get_mut(unit) {
            if on {
                r.flags |= bit;
            } else {
                r.flags &= !bit;
            }
        }
    }
    fn may_attack(&self, attacker: UnitId, defender: UnitId) -> bool {
        self.h.x.may_attack(attacker, defender)
    }
    fn alignment(&self, unit: UnitId) -> u8 {
        self.h.x.alignment(unit)
    }
    fn is_hireling(&self, unit: UnitId) -> bool {
        self.units.get(unit).is_some_and(|r| {
            r.ty == UnitType::Monster && super::combat::HIRELING_CLASSES.contains(&r.class)
        })
    }
    /// `0x005ADB00`: a stat list with state 86 expiring at `expire`, state
    /// 86 on, a type-12 timer at `expire` (`missiles.md` §R5 step 6.1).
    fn apply_justhit(&mut self, game: &mut Game, unit: UnitId, expire: i32) {
        // TODO(missiles.md §R5 step 6.1): the list's owner is not stated;
        // the hit unit's own type and GUID are used.
        let list = self.create_state_list(unit, STATE_JUSTHIT, None, expire);
        // The list's remove callback `0x005ADAF0` (`push 0; call
        // 0x00639DB0(unit, 86, 0)`): state off and the update queue, so
        // the next missile can hit the unit once the delay has passed.
        if let Some(l) = list {
            self.stats.set_remove_callback(
                l,
                Some(crate::stats::lists::RemoveCallback(
                    crate::skills::use_::bodies::callback::JUSTHIT,
                )),
            );
        }
        self.set_state(unit, STATE_JUSTHIT, true);
        if let Err(e) =
            game.schedule_event(unit, u32::from(event::REMOVE_STATE), expire, None, 0, 0)
        {
            self.unit_error(e.into());
        }
    }
}

impl<X: Pending> MissilePath for View<'_, X> {
    fn has_path(&self, unit: UnitId) -> bool {
        self.h.path_has(unit)
    }
    fn set_velocity(&mut self, unit: UnitId, v: i32) {
        self.h.path_set_velocity(unit, v);
    }
    fn velocity(&self, unit: UnitId) -> i32 {
        self.h.path_velocity(unit)
    }
    fn set_target_unit(&mut self, unit: UnitId, target: UnitId) {
        self.path_set_target_unit(unit, target);
    }
    fn set_target_point(&mut self, unit: UnitId, x: i32, y: i32) {
        self.h.path_set_target_point(unit, x, y);
    }
    fn set_footprint_mask(&mut self, unit: UnitId, mask: u16) {
        self.path_set_foot_mask(unit, mask);
    }
    fn set_move_mask(&mut self, unit: UnitId, mask: u16) {
        self.h.path_set_move_mask(unit, mask);
    }
    /// `0x00649970` (`pathing.md` §3).
    fn build(&mut self, game: &mut Game, unit: UnitId) {
        if self.h.paths.is_some() {
            crate::wiring::path::walk::build_path(self, game, unit);
        } else {
            self.h.x.build_path(game, unit);
        }
    }
    fn set_acceleration(&mut self, unit: UnitId, accel: i32, max_velocity: i32) {
        self.h.path_set_acceleration(unit, accel, max_velocity);
    }
    /// `0x006417F0(missile, target point)` (`missiles.md` §R2.3 step 19,
    /// `skills/bodies.md` §4): max(|dx|, |dy|) + ⌊min(|dx|, |dy|) / 2⌋
    /// from the path position to the path target point, with the path
    /// provider; [`Pending`] without it.
    fn target_distance(&self, unit: UnitId) -> i32 {
        match self.path_target_xy(unit) {
            Some((tx, ty)) => {
                let (x, y) = self.h.path_position(unit);
                crate::path::walk::resync::resync_distance(
                    crate::path::Point::new(x, y),
                    crate::path::Point::new(tx, ty),
                )
            }
            None => self.h.x.target_distance(unit),
        }
    }
    /// Unit step `0x00554CA0` (`pathing.md` §9.3): false when it returns 2.
    fn step(&mut self, game: &mut Game, unit: UnitId) -> bool {
        if self.h.paths.is_some() {
            crate::wiring::path::walk::unit_step(self, game, unit)
        } else {
            self.h.x.step(game, unit)
        }
    }
    /// `0x00648EB0` (`missiles.md` §R4 step 6): recomputed at the current
    /// position when the path velocity is 0 (the room's collision mask
    /// with the missile's size, all bits), otherwise the word the last
    /// step cached in the path (+0x54).
    fn collision_word(&self, game: &Game, unit: UnitId) -> u16 {
        let cached = self.h.path_cached_word(unit);
        let (x, y) = self.h.path_position(unit);
        match cached {
            Some(w) if self.h.path_velocity(unit) != 0 => w,
            _ if self.h.paths.is_some() => crate::path::collision::size_value(
                &self.h.drlg,
                Self::unit_room(game, unit),
                x,
                y,
                self.path_size(unit),
                0xFFFF,
            ),
            // TODO(units.md path): without a path provider caching the
            // word, it is read from the grid at the current position.
            _ => Self::unit_room(game, unit)
                .and_then(|r| self.h.drlg.collision(game, r, x, y))
                .unwrap_or(0),
        }
    }
    fn crossed_subtiles(&self, unit: UnitId) -> Vec<(i32, i32)> {
        self.h.path_crossed(unit)
    }
}

impl<X: Pending> MissileRooms for View<'_, X> {
    fn find_room(&self, game: &Game, near: RoomId, x: i32, y: i32) -> Option<RoomId> {
        self.h.drlg.find_room(game, near, x, y)
    }
    fn in_town(&self, game: &Game, room: RoomId) -> bool {
        self.h.drlg.in_town(game, room)
    }
    /// `0x0064D9B0` (`path-placement.md` §4 rules 3–5 with the path
    /// provider).
    ///
    /// TODO(missiles.md §R4 step 9, rooms.md §10): without the path
    /// provider the sub-tile at (x, y) is read for every size.
    fn collision_mask(
        &self,
        game: &Game,
        room: RoomId,
        x: i32,
        y: i32,
        size: i32,
        mask: u16,
    ) -> u16 {
        if self.h.paths.is_some() {
            return crate::path::collision::size_value(&self.h.drlg, Some(room), x, y, size, mask);
        }
        self.h.drlg.collision(game, room, x, y).unwrap_or(0) & mask
    }
    /// `0x0064CB30` (`path-placement.md` §4 rule 2 with the path
    /// provider).
    fn collision_at(&self, game: &Game, room: RoomId, x: i32, y: i32, mask: u16) -> u16 {
        if self.h.paths.is_some() {
            return crate::path::collision::point_value(&self.h.drlg, Some(room), x, y, mask);
        }
        self.h.drlg.collision(game, room, x, y).unwrap_or(0) & mask
    }
    /// `0x0064EBA0`: clear bit 0x40 under the unit (with the path
    /// provider: the size clear of `path-placement.md` §5.1 at the path
    /// position, the unit's size).
    ///
    /// TODO(rooms.md §10): without the path provider the unit's sub-tile
    /// only.
    fn clear_footprint(&mut self, game: &mut Game, unit: UnitId) {
        let (x, y) = self.h.path_position(unit);
        if self.h.paths.is_some() {
            let size = self.path_size(unit);
            let room = Self::unit_room(game, unit);
            crate::path::footprint::clear_size(
                &mut self.h.drlg,
                room,
                x,
                y,
                size,
                crate::drlg::collision::bits::MISSILE,
            );
            return;
        }
        if let Some(r) = Self::unit_room(game, unit) {
            if let Some(m) = self.h.drlg.collision_mut(game, r, x, y) {
                *m &= !crate::drlg::collision::bits::MISSILE;
            }
        }
    }
    /// `0x00641CB0(room, x, y, accept, arg, r)` without the accept call
    /// (`sim/path-placement.md` §4 rule 6): `r` outside 1..3 finds none;
    /// the room and its adjacency array (the room first), units in
    /// room-list order; players in mode 0 / 17 and monsters in mode 0 /
    /// 12 are skipped, objects, items and tiles never found; a unit of
    /// size `s` (> 3 counts 3, ≤ 0 skipped) is found when the query
    /// shape of size `r` overlaps its shape (rule 5's table).
    fn units_at(&self, game: &Game, room: RoomId, x: i32, y: i32, r: i32) -> Vec<UnitId> {
        if !(1..=3).contains(&r) {
            return Vec::new();
        }
        let adjacent = game
            .lists
            .room(room)
            .map(|r| r.adjacent.clone())
            .unwrap_or_default();
        std::iter::once(room)
            .chain(adjacent.into_iter().filter(|&r| r != room))
            .flat_map(|r| game.lists.room_units(r))
            .filter(|&u| {
                let Some(rec) = self.units.get(u) else {
                    return false;
                };
                match rec.ty {
                    UnitType::Player if matches!(rec.mode, 0 | 17) => return false,
                    UnitType::Monster if matches!(rec.mode, 0 | 12) => return false,
                    UnitType::Player | UnitType::Monster | UnitType::Missile => {}
                    _ => return false,
                }
                let s = self.path_size(u).min(3);
                if s <= 0 {
                    return false;
                }
                let (ux, uy) = self.h.path_position(u);
                let (dx, dy) = ((x - ux).abs(), (y - uy).abs());
                match (r, s) {
                    (1, 1) => dx == 0 && dy == 0,
                    (1, 2) | (2, 1) => dx + dy <= 1,
                    (1, 3) | (3, 1) => dx <= 1 && dy <= 1,
                    (2, 2) => dx + dy <= 2,
                    (2, 3) | (3, 2) => (dx <= 2 && dy <= 1) || (dx <= 1 && dy <= 2),
                    _ => dx <= 2 && dy <= 2,
                }
            })
            .collect()
    }
}

/// The missile's result flags (d2rs-local bits of
/// [`crate::missiles::result_flag`]) as `damage.md` §1 result bits:
/// hit 1, get-hit 4, knockback 8, soft hit 0x4000.
pub fn result_bits(missile: u32) -> u16 {
    let mut r = 0;
    for (m, c) in [
        (result_flag::HIT, result::HIT),
        (result_flag::GETHIT, result::GET_HIT),
        (result_flag::SOFTHIT, result::SOFT_HIT),
        (result_flag::KNOCKBACK, result::KNOCKBACK),
    ] {
        if missile & m != 0 {
            r |= c;
        }
    }
    r
}

/// A missile damage record (`missiles.md` §R6.2) as the combat damage
/// record (`damage.md` §1): [`Damage::to_record`] (deadly strike, bypass
/// hit flags, the §R6.3 fields) with the missile's result flags.
/// The hit-class merge of `0x005AD730` (`missiles.md` §R6.1,
/// `0x005AD863`–`0x005AD87A`): R +0x60 := `HitClass` | (R +0x60 & 0xF0);
/// R byte +0x64 := 1 when either has a bit in 0xF0 (else kept). So the
/// 0x60 of server-damage functions 7 and 9 (§R6.3) survives as
/// `HitClass` | 0x60.
pub fn merge_hit_class(rec: &mut DamageRecord, hit_class: u32) {
    let element = rec.hit_class & 0xF0;
    if element != 0 || hit_class & 0xF0 != 0 {
        rec.hit_class_fixed = 1;
    }
    rec.hit_class = hit_class | element;
}

pub fn damage_record(d: &Damage) -> DamageRecord {
    let mut rec = d.to_record();
    rec.result |= result_bits(d.result);
    rec
}

impl<X: Pending> MissileCombat for View<'_, X> {
    /// `missiles/damage.md` §4 on combat's view.
    fn damage_setup(
        &mut self,
        game: &mut Game,
        owner: UnitId,
        origin: Option<UnitId>,
        missile: UnitId,
        skill: i32,
        level: i32,
    ) -> u32 {
        let t = self.h.tables.clone();
        let mut w = self.combat(game);
        crate::missiles::damage::setup(&mut w, &t.skills, owner, origin, missile, skill, level)
    }
    /// `0x0057D9B0(owner, defender, tohit, missile = 1)` (`hit.md` §3).
    fn hit_test(&mut self, game: &mut Game, owner: UnitId, defender: UnitId, tohit: i32) -> bool {
        let t = self.h.tables.clone();
        let mut w = self.combat(game);
        combat::hit_test(
            &mut w,
            &t.skills,
            &t.combat,
            Some(owner),
            Some(defender),
            tohit,
            true,
        )
    }
    /// The rest of `0x005ADCD0` (`missiles.md` §R6.1): the record with the
    /// missile's hit class (`HitClass`) and pierce percent (stat 327),
    /// then `apply(game, owner, unit, missile = 1, record)` (`damage.md`
    /// §5.2) and the reaction (§7.1). No owner: nothing (`0x005AD730`).
    fn apply_damage(
        &mut self,
        game: &mut Game,
        owner: Option<UnitId>,
        missile: UnitId,
        unit: UnitId,
        damage: &mut Damage,
        data: Option<&MissileData>,
    ) {
        let Some(owner) = owner else {
            return;
        };
        let mut rec = damage_record(damage);
        self.apply_missile_record(game, owner, missile, unit, &mut rec, data);
        damage.result = rec.result.into();
    }
    /// Unit event 0 (`0x005C0C30`), also with no unit.
    fn hit_by_missile_event(&mut self, game: &mut Game, missile: UnitId, unit: Option<UnitId>) {
        self.combat(game)
            .fire_unit_event(0, unit, Some(missile), None);
    }
    /// `missiles.md` §R6.2: stat 121 for demons, 122 for undead, 180 by
    /// monster type (entries whose layer matches the unit's montype), on
    /// the missile.
    fn target_damage_bonus(&self, missile: UnitId, unit: UnitId) -> i32 {
        let mut p = 0i32;
        if self.h.is_demon(unit) {
            p = p.wrapping_add(View::stat(self, missile, 121));
        }
        if self.h.is_undead(unit) {
            p = p.wrapping_add(View::stat(self, missile, 122));
        }
        let class = self.units.get(unit).map_or(0, |r| r.class as i32);
        if let Some(l) = self.stats.unit_list(missile) {
            for (k, v) in self.stats.full_entries(l) {
                if crate::stats::key_stat(k) == 180
                    && self.h.x.montype_matches(crate::stats::key_layer(k), class)
                {
                    p = p.wrapping_add(v);
                }
            }
        }
        p
    }
    /// §R6.1 step 3: armor += delta, clamped at ≥ 0 (missiles OQ5: sign
    /// as the stat gives it).
    fn add_target_ac(&mut self, _: &mut Game, unit: UnitId, delta: i32) {
        let ac = self
            .stats
            .unit_base(unit, ARMORCLASS, 0)
            .wrapping_add(delta)
            .max(0);
        self.set_base(unit, ARMORCLASS, ac);
    }
}

impl<X: Pending> View<'_, X> {
    /// The damage part of `0x005ADCD0` (`missiles.md` §R6.1) on a
    /// record: the missile's hit class (`HitClass`), the hit flags from
    /// its data flags 1, 2 and pierce percent (stat 327), then `apply(game, owner, unit, missile = 1, record)`
    /// (`damage.md` §5.2) and the reaction (§7.1).
    fn apply_missile_record(
        &mut self,
        game: &mut Game,
        owner: UnitId,
        missile: UnitId,
        unit: UnitId,
        rec: &mut DamageRecord,
        data: Option<&MissileData>,
    ) {
        let (class, data_flags) = data.map_or((None, 0), |d| (Some(d.class), d.flags));
        let t = self.h.tables.clone();
        if let Some(row) = class.and_then(|c| t.missiles.get(usize::from(c))) {
            merge_hit_class(rec, u32::from(row.hitclass));
        }
        rec.pierce_pct = View::stat(self, missile, PIERCE_PERCENT_STAT);
        let mut w = self.combat(game);
        // `0x005AD730` order (`missiles.md` §R6.1): block/dodge on the
        // unit's seed (avoid 1, block = physical != 0), the hit-class
        // merge above, the monster crit on the owner's seed, then apply
        // only while the hit bit stands, and the reaction always.
        let b = combat::block_or_dodge(&mut w, &t.combat, owner, unit, true, rec.physical != 0);
        rec.result |= match b {
            combat::BlockResult::Avoid => result::AVOID,
            combat::BlockResult::Dodge => result::DODGE,
            combat::BlockResult::WeaponBlock => result::WEAPON_BLOCK,
            combat::BlockResult::Block => result::BLOCK,
            combat::BlockResult::Evade | combat::BlockResult::None => 0,
        };
        if b != combat::BlockResult::None {
            rec.result &= !result::HIT;
        }
        combat::monster_crit(&mut w, &t.combat, owner, unit, rec);
        // Step 5: missile data flags (+0x14) 1 → hit flag 0x20 ("rolled",
        // so `domissiledamage` fires), 2 → 0x80 (no `domissiledamage`).
        if data_flags & 1 != 0 {
            rec.hit_flags |= combat::hitflag::ROLLED;
        }
        if data_flags & 2 != 0 {
            rec.hit_flags |= combat::hitflag::NO_MISSILE_EVENT;
        }
        if rec.result & result::HIT != 0 {
            combat::apply(&mut w, &t.combat, owner, unit, true, rec);
        }
        crate::combat::CombatWorld::reaction(&mut w, owner, unit, rec);
    }

    /// `0x005AD730(game, missile, unit, rec)` with a caller's record
    /// (`umod-callbacks.md` §3.2; `missiles.md` §R6.1): the missile's
    /// result flags ([`crate::missiles::result_flags`]: hit, get-hit /
    /// soft-hit, the knockback roll on the missile's seed) or-ed into a
    /// copy of the record, then the damage part of `0x005ADCD0`. The
    /// caller has checked the owner.
    pub fn missile_record_hit(
        &mut self,
        game: &mut Game,
        owner: UnitId,
        missile: UnitId,
        unit: UnitId,
        mut rec: DamageRecord,
    ) {
        let Some(mut store) = self.h.missiles.take() else {
            self.h.errors.push(WiringError::Reentrant("missiles"));
            return;
        };
        let t = self.h.tables.clone();
        let flags = {
            let mut cx = crate::missiles::Ctx {
                tables: &t.missiles,
                store: &mut store,
                world: &mut *self,
            };
            crate::missiles::result_flags(game, &mut cx, missile, unit)
        };
        let data = store.get(missile).cloned();
        self.h.missiles = Some(store);
        rec.result |= result_bits(flags);
        self.apply_missile_record(game, owner, missile, unit, &mut rec, data.as_ref());
    }
}

impl<X: Pending> MissileHooks for View<'_, X> {
    fn init_callback(&mut self, game: &mut Game, missile: UnitId, callback: u32, arg: u32) {
        self.h.x.missile_init_callback(game, missile, callback, arg);
    }
    /// `0x005A43B0(game, owner, missile)` (`missiles.md` rule 28): the
    /// umod dispatcher in mode 5 (`init.md` §22, the callbacks get the
    /// missile) on the lent monster world; without one,
    /// [`Pending::unique_mod_missile`].
    fn unique_mod_missile(&mut self, game: &mut Game, owner: UnitId, missile: UnitId) {
        let mut sim = Sim {
            game: &mut *game,
            units: &mut *self.units,
            stats: &mut *self.stats,
            data: self.data,
        };
        if !self
            .h
            .run_umods(&mut sim, owner, Some(missile), umod_mode::MISSILE)
        {
            self.h.x.unique_mod_missile(game, owner, missile);
        }
    }
}

/// The server-do / server-hit bodies' seams (`missiles.md` §R9.5,
/// §R9.6) on the wired units: formulas on the skill tables, the path
/// provider's new-step flag and target, collision writes on the DRLG
/// grids, unit records, and the area hit on the combat view. The area
/// scan runs on the skill pipeline ([`Pending::missile_area_units`]).
impl<X: Pending> crate::missiles::MissileBodies for View<'_, X> {
    /// Max life `0x00625D10` (the heal of server-hit 31 reads it).
    fn max_life(&self, unit: UnitId) -> i32 {
        self.stats.max_life(unit)
    }
    /// Max mana `0x00625D60`.
    fn max_mana(&self, unit: UnitId) -> i32 {
        self.stats.max_mana(unit)
    }
    /// The `pettype.txt` row count (data tables +0xBF0).
    fn pet_type_count(&self) -> i32 {
        self.h.bodies.as_ref().map_or(0, |b| b.pettype_count)
    }
    /// `0x0056E620` on the skill pipeline ([`Pending::missile_summon_class`]).
    fn summon_class(
        &mut self,
        game: &mut Game,
        owner: UnitId,
        skill: i32,
        level: i32,
    ) -> crate::missiles::SummonClass {
        let (class, mode) = with_sim(self, game, |h, sim| {
            X::missile_summon_class(h, sim, owner, skill, level)
        });
        crate::missiles::SummonClass { class, mode }
    }
    /// `0x0056D940` flags 0xD on the skill pipeline
    /// ([`Pending::missile_summon_spawn`]).
    fn summon_spawn(
        &mut self,
        game: &mut Game,
        owner: UnitId,
        class: i32,
        mode: i32,
        at: (i32, i32),
        pet_type: i32,
    ) -> Option<UnitId> {
        with_sim(self, game, |h, sim| {
            X::missile_summon_spawn(h, sim, owner, class, mode, at, pet_type)
        })
    }
    /// `0x00554DE0` ([`Pending::missile_ally_test`]).
    fn ally_test(&mut self, game: &mut Game, a: UnitId, b: UnitId) -> bool {
        with_sim(self, game, |h, sim| X::missile_ally_test(h, sim, a, b))
    }
    /// `shout_state` (`0x005D8290`) on the skill use view
    /// ([`Pending::missile_shout_state`]).
    fn shout_state(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        owner: UnitId,
        skill: i32,
        level: i32,
    ) {
        with_sim(self, game, |h, sim| {
            X::missile_shout_state(h, sim, unit, owner, skill, level)
        });
    }
    /// `missiles/bodies-2.md` §33 step 8 ([`Pending::missile_bone_wall_piece`]).
    fn bind_bone_wall_piece(
        &mut self,
        game: &mut Game,
        owner: UnitId,
        anchor: UnitId,
        piece: UnitId,
        skill: i32,
        level: i32,
    ) {
        with_sim(self, game, |h, sim| {
            X::missile_bone_wall_piece(h, sim, owner, anchor, piece, skill, level)
        });
    }
    /// `0x005444B0(game, id)`: the quest control's published answer
    /// ([`Pending::quest_not_intro`]).
    fn quest_test(&self, _game: &Game, id: i32) -> bool {
        u8::try_from(id).is_ok_and(|c| self.h.x.quest_not_intro(c))
    }
    /// `0x0058E920`: queued for the quest control with the missile's
    /// position ([`Pending::missile_spawn_tyrael`]).
    fn spawn_tyrael(&mut self, _game: &mut Game, room: Option<RoomId>, missile: UnitId) {
        let (x, y) = self.h.path_position(missile);
        self.h.x.missile_spawn_tyrael(room, missile, x, y);
    }
    /// `0x0064B7C0` (`skills/levels.md` `eval_missile`).
    fn missile_calc(
        &mut self,
        game: &mut Game,
        missile: UnitId,
        owner: Option<UnitId>,
        field: u32,
        class: i32,
        level: i32,
    ) -> i32 {
        let t = self.h.tables.clone();
        let mut cv = self.combat(game);
        crate::skills::eval_missile(
            &mut cv,
            &t.skills,
            Some(missile),
            owner,
            field,
            class,
            level,
        )
    }
    fn skill_exists(&self, skill: i32) -> bool {
        self.h.tables.skills.skill(skill).is_some()
    }
    /// The raw skills columns (`missiles/bodies.md`, `bodies-2.md`).
    fn skill_field(&self, skill: i32, field: crate::missiles::SkillField) -> i32 {
        use crate::missiles::SkillField as F;
        let Some(r) = self.h.tables.skills.skill(skill) else {
            return 0;
        };
        match field {
            F::Param(n) => [
                r.param1, r.param2, r.param3, r.param4, r.param5, r.param6, r.param7, r.param8,
            ]
            .get(usize::from(n).wrapping_sub(1))
            .map_or(0, |&v| v as i32),
            F::AuraFilter => r.aurafilter as i32,
            F::AuraTargetState => i32::from(r.auratargetstate as i16),
            F::PetType => i32::from(r.pettype),
        }
    }
    fn skill_calc(
        &mut self,
        game: &mut Game,
        owner: Option<UnitId>,
        skill: i32,
        calc: crate::missiles::SkillCalc,
        level: i32,
    ) -> i32 {
        use crate::missiles::SkillCalc as C;
        let t = self.h.tables.clone();
        let Some(r) = t.skills.skill(skill) else {
            return 0;
        };
        let field = match calc {
            C::Calc1 => r.calc1,
            C::Calc2 => r.calc2,
            C::AuraRange => r.aurarangecalc,
            C::AuraLen => r.auralencalc,
            C::Calc4 => r.calc4,
        };
        let mut cv = self.combat(game);
        crate::skills::eval_skill(&mut cv, &t.skills, owner, field, skill, level)
    }
    /// Path +0x34 bit 3 (`path::record::flags::MOVED`) with the path
    /// provider; false without it.
    fn path_new_step(&self, unit: UnitId) -> bool {
        self.h
            .paths
            .as_ref()
            .and_then(|p| p.dynamic(unit))
            .is_some_and(|d| d.flags & crate::path::record::flags::MOVED != 0)
    }
    /// The dynamic path's target unit when it still resolves to the same
    /// unit (`skills/bodies.md` §2.1; a stale target reads as none, the
    /// path field is left to the path code).
    fn path_target(&mut self, game: &Game, unit: UnitId) -> Option<UnitId> {
        let t = self.h.paths.as_ref()?.dynamic(unit)?.target_unit?;
        (game.lists.find_unit(t.ty, t.guid) == Some(t.unit)).then_some(t.unit)
    }
    /// `0x0064CB90`: the room containing (x, y) from `room`, one cell.
    fn or_collision(&mut self, game: &mut Game, room: RoomId, x: i32, y: i32, bits: u16) {
        let Some(r) = self.h.drlg.find_room(game, room, x, y) else {
            return;
        };
        if let Some(c) = self.h.drlg.collision_mut(game, r, x, y) {
            *c |= bits;
        }
    }
    /// `0x0064EA00` (`path-placement.md` §5.1) at the path position with
    /// the unit's size.
    fn stamp_collision(&mut self, game: &mut Game, unit: UnitId, bits: u16) {
        let (x, y) = self.h.path_position(unit);
        let size = self.path_size(unit);
        let room = Self::unit_room(game, unit);
        crate::path::footprint::stamp_size(&mut self.h.drlg, room, x, y, size, bits);
    }
    fn anim_frame(&self, unit: UnitId) -> i32 {
        self.units.get(unit).map_or(0, |r| r.anim.frame)
    }
    fn set_anim_frame(&mut self, unit: UnitId, v: i32) {
        if let Some(r) = self.units.get_mut(unit) {
            r.anim.frame = v;
        }
    }
    fn is_dead(&self, unit: UnitId) -> bool {
        self.units.is_dead(unit)
    }
    fn is_demon(&self, unit: UnitId) -> bool {
        self.h.is_demon(unit)
    }
    fn is_undead(&self, unit: UnitId) -> bool {
        self.h.is_undead(unit)
    }
    fn area_units(
        &mut self,
        game: &mut Game,
        owner: UnitId,
        at: (i32, i32),
        r: i32,
        f: u32,
    ) -> Vec<UnitId> {
        with_sim(self, game, |h, sim| {
            X::missile_area_units(h, sim, owner, at, r, f)
        })
    }
    /// `0x0056B9C0` on the combat view.
    fn area_hit(&mut self, game: &mut Game, owner: UnitId, unit: UnitId, record: &DamageRecord) {
        let t = self.h.tables.clone();
        let mut cv = self.combat(game);
        crate::missiles::bodies::area_hit(&mut cv, &t.combat, owner, unit, record);
    }
    /// The active room's seed (+0x6C) on the DRLG (`bodies-2.md` §47
    /// step 2).
    fn room_seed(&mut self, _: &mut Game, room: RoomId) -> Option<&mut crate::rng::Seed> {
        self.h.drlg.active_seed_mut(room)
    }
    /// `0x0064E260(room, from, to, mask)` (`pathing.md` §13.3) on the
    /// DRLG's collision grids: result 1 (blocked).
    fn line_hits(
        &self,
        _: &Game,
        room: RoomId,
        from: (i32, i32),
        to: (i32, i32),
        mask: u16,
    ) -> bool {
        use crate::path::Point;
        crate::path::line::line_test(
            &self.h.drlg,
            Some(room),
            Point::new(from.0, from.1),
            Point::new(to.0, to.1),
            mask,
        )
        .blocked()
    }
    /// Path target point (`0x00648A00` / `0x00648A10`) with the path
    /// provider ([`crate::wiring::path::missiles`]); (0, 0) without it.
    fn path_target_point(&self, unit: UnitId) -> (i32, i32) {
        self.path_target_xy(unit).unwrap_or((0, 0))
    }
    /// `0x0056D2C0` with the path provider; none without it.
    fn target_position(&mut self, game: &Game, unit: UnitId) -> Option<(i32, i32)> {
        self.path_target_position(game, unit).flatten()
    }
    /// Set type `0x00648CF0` with the path provider.
    fn set_path_type(&mut self, unit: UnitId, ty: i32) {
        let _ = View::path_set_type(self, unit, ty);
    }
    /// Step counts `0x00648E70` with the path provider.
    fn set_path_distance(&mut self, unit: UnitId, d: i32) {
        let _ = self.path_set_step_counts(unit, d);
    }
    /// `0x00621DC0` with the path provider; 0 without it.
    fn dir64(&self, unit: UnitId, at: (i32, i32)) -> i32 {
        self.path_dir64(unit, at).unwrap_or(0)
    }
    /// Path point i with the path provider.
    fn set_path_point(&mut self, unit: UnitId, i: i32, at: (u16, u16)) {
        let _ = self.path_set_point(unit, i, at);
    }
    /// `0x00648790` with the path provider.
    fn set_path_point_count(&mut self, unit: UnitId, n: i32) {
        let _ = self.path_set_point_count(unit, n);
    }
    /// Teleport `0x00650BE0` with the path provider.
    fn path_teleport(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        room: Option<RoomId>,
        x: i32,
        y: i32,
    ) {
        let _ = self.path_teleport_to(game, unit, room, x, y);
    }
    /// The active room's sub-tile rectangle on the DRLG (`bodies-2.md`
    /// §44 unit find step 2).
    fn room_subtiles(&self, _: &Game, room: RoomId) -> Option<(i32, i32, i32, i32)> {
        use crate::path::CollisionRooms;
        self.h.drlg.subtile_rect(room).map(|t| (t.x, t.y, t.w, t.h))
    }
    /// The states count (data tables +0xC4).
    fn states_count(&self) -> i32 {
        i32::try_from(self.stats.data().states.count()).unwrap_or(i32::MAX)
    }
    /// The expiry of the unit's list of `s` (`0x006256B0`, list +0x18).
    fn state_list_expiry(&self, unit: UnitId, s: i32) -> Option<i32> {
        let l = self.state_list(unit, u16::try_from(s).ok()?)?;
        Some(self.stats.expire(l))
    }
    /// `missiles/bodies-2.md` §12 step 6 / §30 (`0x005AB1DD`..`0x005AB234`):
    /// a list with flags 2, expiry `expire` and `owner`'s type and GUID
    /// (`0x006251F0`), state `s` (`0x006252D0`), remove callback
    /// `0x0056E900` (`0x00625CE0`), attached with reset 1 (`0x00626E10`),
    /// then state `s` on and the unit queued for update (`0x00639DB0`).
    fn new_state_list(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        s: i32,
        expire: i32,
        owner: UnitId,
    ) -> bool {
        let Ok(s) = u16::try_from(s) else {
            return false;
        };
        let Some((ty, guid)) = game.lists.unit(owner).map(|e| (e.ty, e.guid)) else {
            return false;
        };
        let l = self.stats.alloc(2, expire, ty.index() as u32, guid);
        self.stats.set_state(l, u32::from(s));
        self.stats.set_remove_callback(
            l,
            Some(crate::wiring::interaction::skill_use::DELAY_REMOVE_CALLBACK),
        );
        self.stats.attach(&mut *self.h, unit, l, true);
        self.set_state(unit, s, true);
        if let Err(e) = game.lists.queue_update(unit) {
            self.unit_error(crate::game::GameError::from(e).into());
        }
        true
    }
    /// `0x005C6CC0(unit, list of s, R, skill, L)` (`skills/bodies.md`
    /// §2.6): `aurastat1–6` evaluated on `unit` with `aurastatcalc1–6`,
    /// each non-zero value set in the list (attack rate also as the other
    /// anim rate), then the anim rate refreshed. Nothing without a list.
    fn aura_fill(&mut self, game: &mut Game, unit: UnitId, s: i32, skill: i32, level: i32) {
        use crate::combat::CombatWorld;
        let Some(l) = u16::try_from(s).ok().and_then(|s| self.state_list(unit, s)) else {
            return;
        };
        let t = self.h.tables.clone();
        let Some(r) = t.skills.skill(skill) else {
            return;
        };
        let pairs = [
            (r.aurastat1, r.aurastatcalc1),
            (r.aurastat2, r.aurastatcalc2),
            (r.aurastat3, r.aurastatcalc3),
            (r.aurastat4, r.aurastatcalc4),
            (r.aurastat5, r.aurastatcalc5),
            (r.aurastat6, r.aurastatcalc6),
        ];
        let mut cv = self.combat(game);
        for (st, calc) in pairs {
            let st = i32::from(st as i16);
            if !(0..t.skills.stat_count).contains(&st) {
                continue;
            }
            let v = crate::skills::eval_skill(&mut cv, &t.skills, Some(unit), calc, skill, level);
            if v != 0 {
                cv.v.set_list_stat(l, st as u16, v);
                // Stats 68 (attackrate) and 69 (other_animrate).
                if st == 68 {
                    cv.v.set_list_stat(l, 69, v);
                }
            }
        }
        cv.refresh_anim_rate(unit);
    }
    /// `0x00639E30(unit, s, 1)`: the state-changed bit.
    fn mark_state_changed(&mut self, unit: UnitId, s: i32) {
        if let Ok(s) = u32::try_from(s) {
            self.stats.set_state_changed(unit, s, true);
        }
    }
    /// `0x006260B0`: the expiry of the unit's list of `s`.
    fn set_state_list_expiry(&mut self, unit: UnitId, s: i32, expire: i32) {
        if let Some(l) = u16::try_from(s).ok().and_then(|s| self.state_list(unit, s)) {
            self.stats.set_expire(l, expire);
        }
    }
}

/// Runs `f` on the hooks and a [`crate::units::hooks::Sim`] over the
/// view's parts and `game`.
fn with_sim<X: Pending, R>(
    v: &mut View<'_, X>,
    game: &mut Game,
    f: impl FnOnce(&mut super::ActionHooks<X>, &mut crate::units::hooks::Sim<'_>) -> R,
) -> R {
    let mut sim = crate::units::hooks::Sim {
        game,
        units: &mut *v.units,
        stats: &mut *v.stats,
        data: v.data,
    };
    f(&mut *v.h, &mut sim)
}
