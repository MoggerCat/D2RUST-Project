// Spec: specs/combat/damage.md §7.1, §7.2; specs/combat/vitals.md §4.2, §4.3; specs/sim/units.md §4.6
//! The reaction `0x0057CEE0` after a hit and the kill `0x0057CCB0`
//! (`damage.md` §7), as far as the spec states them: an uninterruptible
//! defender (state 54) only gets `death_delay` (92) when the hit will
//! kill it; a monster defender the hit will kill (result 2) is killed:
//! the kill's guards, its death mode change toward the attacker
//! (`units.md` §4.6, the death start `0x005A6FF0` through
//! [`Pending::monster_death_start`]) and the attacker's experience
//! (`vitals.md` §4.2–§4.3 through
//! [`crate::wiring::interaction::vitals::kill_experience`]).
//!
//! Everything §7 checks at call level only stays a seam:
//! [`Pending::reaction`] (town rule, hit class store, the player
//! branch, the monster knockback / block / get-hit / soft hit mode
//! changes) and [`Pending::kill_step`] (pet credit, attacker
//! bookkeeping, facing, quest kill, barricade doors).

use crate::combat::vitals::VitalsUnits;
use crate::combat::DamageRecord;
use crate::stats::states::state;
use crate::units::modes::monster_mode;
use crate::units::{UnitId, UnitType};
use crate::wiring::interaction::vitals::kill_experience;

use super::combat::CombatView;
use super::units::STATE_DEATH_DELAY;
use super::{KillStep, Pending, View};

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
    }
}

/// The kill `0x0057CCB0`(game, defender, attacker, 1) of a monster
/// (`damage.md` §7.2): nothing for a monster already dying or dead
/// (DT, DD) or not `killable` (nor for other unit types: the player
/// kill belongs to the reaction's player branch); else the steps in the spec's order, the
/// death mode change (`0x005A7C20` with mode DT and the attacker as
/// target) among them.
///
/// TODO(damage.md OQ7, vitals.md OQ2): where the experience is given in
/// the kill is not stated (D2MOO structure); it is given last, to the
/// attacker as [`kill_experience`] computes it (players only, no pet
/// credit, party share or `ExpRatio`). No draw is involved, so only the
/// order of its stat writes against the other steps can differ.
pub fn kill<X: Pending>(cv: &mut CombatView<'_, X>, d: UnitId, a: UnitId) {
    let Some(r) = cv.v.units.get(d) else {
        return;
    };
    let (ty, class, mode) = (r.ty, r.class, r.mode);
    match ty {
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
        }
        // The player kill (§7.1 "will die → death mode", §7.2's player
        // guard) is the reaction's player branch: [`Pending::reaction`].
        _ => return,
    }
    let game = &mut *cv.game;
    cv.v.h.x.kill_step(game, KillStep::PetCredit, d, a);
    cv.v.h
        .x
        .kill_step(game, KillStep::AttackerBookkeeping, d, a);
    cv.v.h.x.kill_step(game, KillStep::FaceAttacker, d, a);
    cv.v.h.mode_target = Some(a);
    cv.v.monster_set_mode(game, d, monster_mode::DT);
    cv.v.h.mode_target = None;
    if !cv.v.h.x.is_revived(d) {
        cv.v.h.x.kill_step(game, KillStep::QuestKill, d, a);
    }
    cv.v.h.x.kill_step(game, KillStep::BarricadeDoors, d, a);
    if let Some(t) = cv.v.h.vitals.clone() {
        kill_experience(&mut cv.v, &t, a, d);
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
    fn level_up_event(&mut self, u: UnitId) {
        self.h.x.level_up_event(u);
    }
}
