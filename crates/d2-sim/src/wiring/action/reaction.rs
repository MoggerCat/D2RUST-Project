// Spec: specs/combat/damage.md §7.1, §7.2; specs/combat/vitals.md §4.2–§4.5; specs/sim/units.md §4.6
//! The reaction `0x0057CEE0` after a hit and the kill `0x0057CCB0`
//! (`damage.md` §7), as far as the spec states them: an uninterruptible
//! defender (state 54) only gets `death_delay` (92) when the hit will
//! kill it; a monster defender the hit will kill (result 2) is killed:
//! the kill's guards, its death mode change toward the attacker
//! (`units.md` §4.6, the death start `0x005A6FF0` through
//! [`Pending::monster_death_start`]) and the experience distribution
//! (`vitals.md` §4.4, [`distribute`]).
//!
//! Everything §7 checks at call level only stays a seam:
//! [`Pending::reaction`] (town rule, hit class store, the player
//! branch, the monster knockback / block / get-hit / soft hit mode
//! changes) and [`Pending::kill_step`] (pet credit, attacker
//! bookkeeping, facing, quest kill, barricade doors).

use crate::combat::result;
use crate::combat::vitals::experience::{distribute, ExpShare};
use crate::combat::vitals::VitalsUnits;
use crate::combat::DamageRecord;
use crate::stats::states::state;
use crate::units::modes::{monster_mode, player_mode};
use crate::units::{UnitId, UnitType};

use super::combat::CombatView;
use super::units::STATE_DEATH_DELAY;
use super::{KillStep, Pending, View};

/// Unit event 12 `levelup` (`vitals.md` §4.5).
pub const EV_LEVELUP: u8 = 12;

/// Result flag 2, "will die" (`damage.md` §1).
pub const RESULT_WILL_DIE: u32 = 2;

/// The reaction `0x0057CEE0` (`damage.md` §7.1) of `d` to `a`'s hit.
///
/// TODO(damage.md §7.1, OQ3): §7.1 is checked at call level only. The
/// seam [`Pending::reaction`] runs first (the town rule, the hit class
/// store and the defender branches' mode changes); in 1.14d the
/// state-54 test comes after the hit class and before those branches,
/// and the monster kill comes after the knockback → get-hit adjustment
/// and before the knockback / block / get-hit / soft-hit changes. A
/// host must leave the branches out for a state-54 defender.
pub fn reaction<X: Pending>(
    cv: &mut CombatView<'_, X>,
    a: UnitId,
    d: UnitId,
    rec: &mut DamageRecord,
) {
    cv.v.h.x.reaction(a, d, rec);
    let will_die = u32::from(rec.result) & RESULT_WILL_DIE != 0;
    if cv.v.stats.has_state(d, state::UNINTERRUPTABLE) {
        if will_die {
            cv.v.set_state(d, STATE_DEATH_DELAY, true);
        }
        return;
    }
    let monster = cv
        .game
        .lists
        .unit(d)
        .is_some_and(|e| e.ty == UnitType::Monster);
    if monster && will_die {
        kill(cv, d, a);
        return;
    }
    if monster {
        monster_hit(cv, a, d, rec);
    } else if cv
        .game
        .lists
        .unit(d)
        .is_some_and(|e| e.ty == UnitType::Player)
    {
        player_hit(cv, d, rec);
    }
}

/// Unit flag 0x8000 (+0xC4), the "soft" hit: the client is told with
/// S→C 0x0C (`units.md` §7.3 rule 2 step 7).
pub const UNIT_FLAG_SOFT_HIT: u32 = 0x8000;

/// "Soft" (`damage.md` §7.1): queue the unit for update and set unit
/// flag 0x8000.
fn soft<X: Pending>(cv: &mut CombatView<'_, X>, d: UnitId) {
    if cv.game.lists.queue_update(d).is_err() {
        return;
    }
    if let Some(r) = cv.v.units.get_mut(d) {
        r.flags |= UNIT_FLAG_SOFT_HIT;
    }
}

/// The get-hit test `0x0057CB00` (§6.2) is false: the unit enters get-hit.
fn enters_get_hit<X: Pending>(cv: &mut CombatView<'_, X>, d: UnitId, rec: &DamageRecord) -> bool {
    let t = cv.v.h.tables.clone();
    !crate::combat::damage::no_get_hit(cv, &t.combat.hitclass, d, rec, rec.hit_class)
}

/// `damage.md` §7.1 step 4 for a monster the hit does not kill: the
/// get-hit mode (4.6) and the soft hits (4.7, 4.8's flag part).
///
/// d2rs-own, unverified (PROVISIONAL, REC in `docs/HANDOFF.md` §7): the
/// mode request `0x005A7E60` / `0x005A7C20` is the direct mode change
/// toward the attacker; the knockback / block steps (4.1, 4.2, 4.4,
/// 4.5), the umod mode 4 call and the life-percent soft test (4.8) are
/// not done.
fn monster_hit<X: Pending>(cv: &mut CombatView<'_, X>, a: UnitId, d: UnitId, rec: &DamageRecord) {
    let Some(mode) = cv.v.units.get(d).map(|r| r.mode) else {
        return;
    };
    if mode == monster_mode::DT || mode == monster_mode::DD {
        return;
    }
    let f = rec.result;
    if f & result::GET_HIT != 0 {
        if cv.v.stats.has_state(d, STATE_STUNNED) || enters_get_hit(cv, d, rec) {
            let game = &mut *cv.game;
            cv.v.h.mode_target = Some(a);
            cv.v.monster_set_mode(game, d, monster_mode::GH);
            cv.v.h.mode_target = None;
        } else {
            soft(cv, d);
        }
    } else if f & result::SOFT_HIT != 0 {
        soft(cv, d);
    }
}

/// `damage.md` §7.1 step 5 for a player: the soft hits (5.6 test true,
/// 5.7). d2rs-own, unverified: the player's get-hit / block / death mode
/// requests (`pathing.md` §1.2) are not made here.
fn player_hit<X: Pending>(cv: &mut CombatView<'_, X>, d: UnitId, rec: &DamageRecord) {
    let f = rec.result;
    if f & (result::DODGE | result::AVOID | result::EVADE | result::BLOCK | result::WEAPON_BLOCK)
        != 0
        || f & result::WILL_DIE != 0
    {
        return;
    }
    if f & result::GET_HIT != 0 {
        if !cv.v.stats.has_state(d, STATE_STUNNED) && !enters_get_hit(cv, d, rec) {
            soft(cv, d);
        }
    } else if f & result::SOFT_HIT != 0 {
        soft(cv, d);
    }
}

/// State 21 `stunned`.
const STATE_STUNNED: u32 = 21;

/// Unit flag 0x04000000: no experience for this victim (`damage.md`
/// §7.2 step 2, `0x005A4EF0`).
pub const UNIT_FLAG_NO_EXPERIENCE: u32 = 0x0400_0000;

/// The kill `0x0057CCB0(game, D, A, 1)` (`damage.md` §7.2):
///
/// 1. Guards: a player victim in mode 0 or 17, a monster in mode 0 or 12
///    or not `killable`, any other type → stop. A monster victim then
///    gets the pet death bookkeeping `0x005751A0` (flag 1).
/// 2. The experience distribution `0x0057E990` (`vitals.md` §4.4) unless
///    D has unit flag 0x04000000; the arena kill event `0x0053F720`. The
///    call `0x0066A220(A, D's class)` is an empty stub (`ret 8`) and is
///    omitted.
/// 3. A monster victim: the death mode request toward A, the quest kill
///    parse unless D has unit flag 0x80000000, the barricade doors.
pub fn kill<X: Pending>(cv: &mut CombatView<'_, X>, d: UnitId, a: UnitId) {
    let Some(r) = cv.v.units.get(d) else {
        return;
    };
    let (ty, class, mode, flags) = (r.ty, r.class, r.mode, r.flags);
    // Step 1.
    match ty {
        UnitType::Player => {
            if mode == player_mode::DT || mode == player_mode::DD {
                return;
            }
        }
        UnitType::Monster => {
            if mode == monster_mode::DT || mode == monster_mode::DD {
                return;
            }
            let t = cv.v.h.tables.clone();
            if !t
                .combat
                .monstats
                .get(class as usize)
                .is_some_and(|m| m.killable)
            {
                return;
            }
            // `hirelings.md` §8 rule 1: flag 1 here, so a hireling with a
            // player owner gets `0x005751A0` (on the host that holds the
            // hireling lists, `ActionHooks::pet_deaths`); the other pet
            // types stay on the seam.
            if let Some(q) = cv.v.h.pet_deaths.as_mut() {
                q.push(d);
            }
            let game = &mut *cv.game;
            cv.v.h.x.kill_step(game, KillStep::PetCredit, d, a);
        }
        _ => return,
    }
    // Step 2: A is present at every caller here.
    if flags & UNIT_FLAG_NO_EXPERIENCE == 0 {
        if let Some(t) = cv.v.h.vitals.clone() {
            distribute(cv, &t, a, d);
        }
    }
    let game = &mut *cv.game;
    cv.v.h
        .x
        .kill_step(game, KillStep::AttackerBookkeeping, d, a);
    if ty != UnitType::Monster {
        return;
    }
    // Step 3.
    cv.v.h.x.kill_step(game, KillStep::FaceAttacker, d, a);
    cv.v.h.mode_target = Some(a);
    cv.v.monster_set_mode(game, d, monster_mode::DT);
    cv.v.h.mode_target = None;
    if !cv.v.h.x.is_revived(d) {
        cv.v.h.x.kill_step(game, KillStep::QuestKill, d, a);
        if flags & super::quest_events::UNIT_FLAG_NO_QUEST_KILL == 0 {
            cv.v.h.queue_quest_kill(d, a);
        }
    }
    cv.v.h.x.kill_step(game, KillStep::BarricadeDoors, d, a);
}

/// The distribution's world calls (`vitals.md` §4.4) on the action
/// wiring: the credited player, the hireling share and the party stay
/// [`Pending`] seams (single player: no party, so the solo path).
impl<X: Pending> ExpShare for View<'_, X> {
    fn credited_player(&self, attacker: UnitId, defender: UnitId) -> Option<UnitId> {
        self.h.x.kill_credited_player(attacker, defender)
    }
    fn hireling_share(&mut self, p: UnitId, attacker: UnitId, defender: UnitId, e: i32) {
        self.h.x.kill_hireling_share(p, attacker, defender, e);
    }
    fn in_party(&self, p: UnitId) -> bool {
        self.h.x.kill_in_party(p)
    }
    fn party_members(&self, p: UnitId, defender: UnitId) -> Vec<UnitId> {
        self.h.x.kill_party_members(p, defender)
    }
}

/// The distribution on combat's view: [`View`]'s calls, with the game
/// for unit event 12.
impl<X: Pending> ExpShare for CombatView<'_, X> {
    fn credited_player(&self, attacker: UnitId, defender: UnitId) -> Option<UnitId> {
        self.v.credited_player(attacker, defender)
    }
    fn hireling_share(&mut self, p: UnitId, attacker: UnitId, defender: UnitId, e: i32) {
        self.v.hireling_share(p, attacker, defender, e);
    }
    fn in_party(&self, p: UnitId) -> bool {
        self.v.in_party(p)
    }
    fn party_members(&self, p: UnitId, defender: UnitId) -> Vec<UnitId> {
        self.v.party_members(p, defender)
    }
}

/// [`View`]'s vitals calls; unit event 12 (`vitals.md` §4.5,
/// `0x005C0C30(game, 12, U, 0, 0)`) through the event registry
/// ([`CombatView::fire_unit_event`]).
impl<X: Pending> VitalsUnits for CombatView<'_, X> {
    type Unit = UnitId;

    fn unit_type(&self, u: UnitId) -> UnitType {
        VitalsUnits::unit_type(&self.v, u)
    }
    fn class_id(&self, u: UnitId) -> i32 {
        VitalsUnits::class_id(&self.v, u)
    }
    fn base_stat(&self, u: UnitId, s: u16) -> i32 {
        VitalsUnits::base_stat(&self.v, u, s)
    }
    fn stat(&self, u: UnitId, s: u16) -> i32 {
        VitalsUnits::stat(&self.v, u, s)
    }
    fn set_base_stat(&mut self, u: UnitId, s: u16, v: i32) {
        self.v.set_base_stat(u, s, v);
    }
    fn add_base_stat(&mut self, u: UnitId, s: u16, v: i32) {
        self.v.add_base_stat(u, s, v);
    }
    fn max_life(&self, u: UnitId) -> i32 {
        VitalsUnits::max_life(&self.v, u)
    }
    fn max_mana(&self, u: UnitId) -> i32 {
        VitalsUnits::max_mana(&self.v, u)
    }
    fn max_stamina(&self, u: UnitId) -> i32 {
        VitalsUnits::max_stamina(&self.v, u)
    }
    fn refresh(&mut self, u: UnitId) {
        VitalsUnits::refresh(&mut self.v, u);
    }
    fn level_up_notify(&mut self, u: UnitId) {
        self.v.level_up_notify(u);
    }
    /// Without the registry: [`Pending::level_up_event`].
    fn level_up_event(&mut self, u: UnitId) {
        if self.v.h.unit_events.is_some() {
            self.fire_unit_event(EV_LEVELUP, Some(u), None, None);
        } else {
            self.v.h.x.level_up_event(u);
        }
    }
}

/// The vitals' view on the action wiring: unit records, stat lists
/// (with the action hooks as stat host) and [`Pending`]'s vitals seams.
impl<X: Pending> VitalsUnits for View<'_, X> {
    type Unit = UnitId;

    fn unit_type(&self, u: UnitId) -> UnitType {
        self.units.get(u).map_or(UnitType::Tile, |r| r.ty)
    }
    fn class_id(&self, u: UnitId) -> i32 {
        self.units.get(u).map_or(0, |r| r.class as i32)
    }
    /// `0x006253B0(unit, stat, 0)`.
    fn base_stat(&self, u: UnitId, s: u16) -> i32 {
        self.stats.unit_base(u, s, 0)
    }
    /// `0x00625480(unit, stat, 0)`.
    fn stat(&self, u: UnitId, s: u16) -> i32 {
        self.stats.unit_total(u, s, 0)
    }
    /// `0x00627260(unit, stat, v, 0)`.
    fn set_base_stat(&mut self, u: UnitId, s: u16, v: i32) {
        self.stats.unit_set(&mut *self.h, u, s, v, 0);
    }
    /// `0x006272B0(unit, stat, v, 0)`.
    fn add_base_stat(&mut self, u: UnitId, s: u16, v: i32) {
        self.stats.unit_add(&mut *self.h, u, s, v, 0);
    }
    fn max_life(&self, u: UnitId) -> i32 {
        self.stats.max_life(u)
    }
    fn max_mana(&self, u: UnitId) -> i32 {
        self.stats.max_mana(u)
    }
    fn max_stamina(&self, u: UnitId) -> i32 {
        self.stats.max_stamina(u)
    }
    fn refresh(&mut self, u: UnitId) {
        self.h.x.stats_refresh(u);
    }
    fn level_up_notify(&mut self, u: UnitId) {
        self.h.x.level_up_notify(u);
    }
    /// Without a game: [`Pending::level_up_event`] (the kill's
    /// distribution runs on [`CombatView`], which fires the event).
    fn level_up_event(&mut self, u: UnitId) {
        self.h.x.level_up_event(u);
    }
}
