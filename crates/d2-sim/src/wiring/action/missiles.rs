// Spec: specs/missiles/missiles.md §R2, §R4, §R5, §R6 (seams `MissileUnits`, `MissilePath`, `MissileRooms`, `MissileCombat`, `MissileHooks`)
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
    result_flag, stat, Damage, MissileCombat, MissileHooks, MissilePath, MissileRooms, MissileUnits,
};
use crate::rng::Seed;
use crate::tick::events::event;
use crate::units::lifecycle::AllocRequest;
use crate::units::{RoomId, UnitId, UnitType};

use super::combat::CombatView;
use super::units::STATE_JUSTHIT;
use super::{Pending, View};

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
        self.h.x.position(unit)
    }
    fn size(&self, unit: UnitId) -> i32 {
        self.h.x.size(unit)
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
        self.create_state_list(unit, STATE_JUSTHIT, None, expire);
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
        self.h.x.has_path(unit)
    }
    fn set_velocity(&mut self, unit: UnitId, v: i32) {
        self.h.x.set_velocity(unit, v);
    }
    fn velocity(&self, unit: UnitId) -> i32 {
        self.h.x.velocity(unit)
    }
    fn set_target_unit(&mut self, unit: UnitId, target: UnitId) {
        self.h.x.set_target_unit(unit, target);
    }
    fn set_target_point(&mut self, unit: UnitId, x: i32, y: i32) {
        self.h.x.set_target_point(unit, x, y);
    }
    fn set_footprint_mask(&mut self, unit: UnitId, mask: u16) {
        self.h.x.set_footprint_mask(unit, mask);
    }
    fn set_move_mask(&mut self, unit: UnitId, mask: u16) {
        self.h.x.set_move_mask(unit, mask);
    }
    fn build(&mut self, game: &mut Game, unit: UnitId) {
        self.h.x.build_path(game, unit);
    }
    fn set_acceleration(&mut self, unit: UnitId, accel: i32, max_velocity: i32) {
        self.h.x.set_acceleration(unit, accel, max_velocity);
    }
    fn target_distance(&self, unit: UnitId) -> i32 {
        self.h.x.target_distance(unit)
    }
    fn step(&mut self, game: &mut Game, unit: UnitId) -> bool {
        self.h.x.step(game, unit)
    }
    /// `0x00648EB0` (`missiles.md` §R4 step 6): recomputed at the current
    /// position when the path velocity is 0 (the room's grid), otherwise
    /// the word the last step cached in the path.
    fn collision_word(&self, game: &Game, unit: UnitId) -> u16 {
        let cached = self.h.x.cached_collision_word(unit);
        match cached {
            Some(w) if self.h.x.velocity(unit) != 0 => w,
            // TODO(units.md path): without a path provider caching the
            // word, it is read from the grid at the current position.
            _ => {
                let (x, y) = self.h.x.position(unit);
                Self::unit_room(game, unit)
                    .and_then(|r| self.h.drlg.collision(game, r, x, y))
                    .unwrap_or(0)
            }
        }
    }
    fn crossed_subtiles(&self, unit: UnitId) -> Vec<(i32, i32)> {
        self.h.x.crossed_subtiles(unit)
    }
}

impl<X: Pending> MissileRooms for View<'_, X> {
    fn find_room(&self, game: &Game, near: RoomId, x: i32, y: i32) -> Option<RoomId> {
        self.h.drlg.find_room(game, near, x, y)
    }
    fn in_town(&self, game: &Game, room: RoomId) -> bool {
        self.h.drlg.in_town(game, room)
    }
    /// `0x0064D9B0`.
    ///
    /// TODO(missiles.md §R4 step 9, rooms.md §10): the footprint a size
    /// covers is not specified; the sub-tile at (x, y) is read for every
    /// size.
    fn collision_mask(
        &self,
        game: &Game,
        room: RoomId,
        x: i32,
        y: i32,
        _size: i32,
        mask: u16,
    ) -> u16 {
        self.h.drlg.collision(game, room, x, y).unwrap_or(0) & mask
    }
    /// `0x0064CB30`.
    fn collision_at(&self, game: &Game, room: RoomId, x: i32, y: i32, mask: u16) -> u16 {
        self.h.drlg.collision(game, room, x, y).unwrap_or(0) & mask
    }
    /// `0x0064EBA0`: clear bit 0x40 under the unit.
    ///
    /// TODO(rooms.md §10): the size footprint is not specified; the unit's
    /// sub-tile only.
    fn clear_footprint(&mut self, game: &mut Game, unit: UnitId) {
        let (x, y) = self.h.x.position(unit);
        if let Some(r) = Self::unit_room(game, unit) {
            if let Some(m) = self.h.drlg.collision_mut(game, r, x, y) {
                *m &= !crate::drlg::collision::bits::MISSILE;
            }
        }
    }
    /// The units on (x, y), searched in `room` and its adjacency array.
    ///
    /// TODO(missiles.md §R4 step 9): the search order of `0x00641CB0` is
    /// not specified; rooms in adjacency order (the room first), units in
    /// room-list order.
    fn units_at(&self, game: &Game, room: RoomId, x: i32, y: i32) -> Vec<UnitId> {
        let adjacent = game
            .lists
            .room(room)
            .map(|r| r.adjacent.clone())
            .unwrap_or_default();
        std::iter::once(room)
            .chain(adjacent.into_iter().filter(|&r| r != room))
            .flat_map(|r| game.lists.room_units(r))
            .filter(|&u| self.h.x.position(u) == (x, y))
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
/// record (`damage.md` §1).
///
/// TODO(missiles.md §R6.2): where the 103/104/106 bypass flags go in the
/// record is not stated (the 0x70-byte layout has no field for them);
/// they are not carried.
pub fn damage_record(d: &Damage) -> DamageRecord {
    let mut result = result_bits(d.result);
    if d.crit {
        result |= result::CRITICAL;
    }
    DamageRecord {
        result,
        physical: d.phys,
        fire: d.fire,
        burn: d.burn,
        burn_len: d.burn_length,
        lightning: d.light,
        magic: d.magic,
        cold: d.cold,
        poison: d.poison,
        poison_len: d.poison_length,
        cold_len: d.cold_length,
        life_leech: d.life_drain,
        mana_leech: d.mana_drain,
        stamina_leech: d.stamina_drain,
        stun_len: d.stun_length,
        ..DamageRecord::default()
    }
}

impl<X: Pending> MissileCombat for View<'_, X> {
    fn damage_setup(
        &mut self,
        game: &mut Game,
        owner: UnitId,
        origin: Option<UnitId>,
        missile: UnitId,
        level: i32,
    ) {
        self.h
            .x
            .missile_damage_setup(game, owner, origin, missile, level);
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
    fn srv_dmg(
        &mut self,
        game: &mut Game,
        index: i16,
        missile: UnitId,
        unit: UnitId,
        damage: &mut Damage,
    ) {
        self.h.x.srv_dmg(game, index, missile, unit, damage);
    }
    /// The rest of `0x005ADCD0` (`missiles.md` §R6.1): the record with the
    /// missile's hit class (`HitClass`) and pierce percent (stat 327),
    /// then `apply(game, owner, unit, missile = 1, record)` (`damage.md`
    /// §5.2) and the reaction (§7.1). No owner: nothing (`0x005AD730`).
    ///
    /// TODO(missiles.md §R6.1): the `avoid` / `block` arguments of the
    /// block/dodge call `0x0057DFB0` and the hit flags made from missile
    /// data flags 1 and 2 are not stated; neither is applied here.
    fn apply_damage(
        &mut self,
        game: &mut Game,
        owner: Option<UnitId>,
        missile: UnitId,
        unit: UnitId,
        damage: &mut Damage,
    ) {
        let Some(owner) = owner else {
            return;
        };
        let mut rec = damage_record(damage);
        let class = self
            .h
            .missiles
            .as_ref()
            .and_then(|s| s.get(missile))
            .map(|d| d.class);
        let t = self.h.tables.clone();
        if let Some(row) = class.and_then(|c| t.missiles.get(usize::from(c))) {
            rec.hit_class = u32::from(row.hitclass);
        }
        rec.pierce_pct = View::stat(self, missile, PIERCE_PERCENT_STAT);
        let mut w = self.combat(game);
        combat::apply(&mut w, &t.combat, owner, unit, true, &mut rec);
        crate::combat::CombatWorld::reaction(&mut w, owner, unit, &mut rec);
        damage.result = rec.result.into();
    }
    /// Unit event 0 (`0x005C0C30`), also with no unit.
    fn hit_by_missile_event(&mut self, _: &mut Game, missile: UnitId, unit: Option<UnitId>) {
        self.h.x.unit_event(0, unit, Some(missile), None);
    }
    /// `missiles.md` §R6.2: stat 121 for demons, 122 for undead, 180 by
    /// monster type (entries whose layer matches the unit's montype), on
    /// the missile.
    fn target_damage_bonus(&self, missile: UnitId, unit: UnitId) -> i32 {
        let mut p = 0i32;
        if self.h.x.is_demon(unit) {
            p = p.wrapping_add(View::stat(self, missile, 121));
        }
        if self.h.x.is_undead(unit) {
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

impl<X: Pending> MissileHooks for View<'_, X> {
    fn init_callback(&mut self, game: &mut Game, missile: UnitId, callback: u32, arg: u32) {
        self.h.x.missile_init_callback(game, missile, callback, arg);
    }
    fn unique_mod_missile(&mut self, game: &mut Game, owner: UnitId, missile: UnitId) {
        self.h.x.unique_mod_missile(game, owner, missile);
    }
}
