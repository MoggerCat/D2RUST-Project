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

use crate::combat::damage::element_hit_class;
use crate::combat::CombatWorld;
use crate::monsters::ai::{AiModes, ModeTarget};
use crate::path::coords::Point;
use crate::path::walk::request::WalkTarget;
use crate::units::hooks::Sim;

use super::arena::ArenaRow;
use super::combat::CombatView;
use super::monsters::umod_mode;
use super::units::STATE_DEATH_DELAY;
use super::{KillStep, Pending, View};

/// Unit event 12 `levelup` (`vitals.md` §4.5).
pub const EV_LEVELUP: u8 = 12;

/// Result flag 2, "will die" (`damage.md` §1).
pub const RESULT_WILL_DIE: u32 = 2;

/// The reaction `0x0057CEE0` (`damage.md` §7.1) of `d` to `a`'s hit:
/// [`Pending::reaction`] (step 1: the town rule), then step 2 (the hit
/// class store), step 3 (state 54), the monster branch (step 4) and the
/// player branch (step 5).
///
/// TODO(damage.md §7.1 step 1): the town rule's early return is the
/// host's ([`Pending::reaction`]); steps 3–5 run after it regardless.
pub fn reaction<X: Pending>(
    cv: &mut CombatView<'_, X>,
    a: UnitId,
    d: UnitId,
    rec: &mut DamageRecord,
) {
    cv.v.h.x.reaction(a, d, rec);
    // Step 2: an unfixed class without an element nibble takes the
    // element hit class (§6.1, steps the process-wide counter); then
    // D +0xB0 := R +0x60.
    if rec.result & result::HIT != 0 {
        if rec.hit_class_fixed == 0 && rec.hit_class & 0xF0 == 0 {
            let base = rec.hit_class;
            rec.hit_class = element_hit_class(cv.hit_class_counter(), rec, base);
        }
        if let Some(r) = cv.v.units.get_mut(d) {
            r.hit_class = rec.hit_class;
        }
    }
    let will_die = u32::from(rec.result) & RESULT_WILL_DIE != 0;
    // Step 3.
    if cv.v.stats.has_state(d, state::UNINTERRUPTABLE) {
        if will_die {
            cv.v.set_state(d, STATE_DEATH_DELAY, true);
        }
        return;
    }
    match cv.game.lists.unit(d).map(|e| e.ty) {
        Some(UnitType::Monster) => monster_hit(cv, a, d, rec),
        Some(UnitType::Player) => player_hit(cv, a, d, rec),
        _ => {}
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

/// Monster classes that never play a block (`damage.md` §7.1 step 4.5):
/// 243 `diablo`, 333 `diabloclone`, 705 `uberdiablo`.
const NO_BLOCK_CLASSES: [u32; 3] = [243, 333, 705];
/// monstats `BaseId` of `sandleaper1` (step 4.2).
const SANDLEAPER_BASE: u16 = 78;
/// AI state of a reaction-less hit (`monsters/ai.md` §3 "AI state").
const AI_STATE_HIT: u32 = 19;
/// Stat 352 `last_sent_hp_pct`.
const LAST_SENT_HP_PCT: u16 = 352;
/// State 1 `freeze`.
const STATE_FREEZE: u32 = 1;
/// Monster mode 6 `BL`, 13 `KB`.
const MONSTER_BL: u8 = 6;
const MONSTER_KB: u8 = 13;

/// Monster mode request m (`0x005A7E60(D, m, &req)` then
/// `0x005A7C20(game, &req, 1)`, `monsters/ai.md` §7.1): the builder
/// zeroes the record, so without a target the request carries the
/// point (0, 0).
fn monster_request<X: Pending>(
    cv: &mut CombatView<'_, X>,
    d: UnitId,
    mode: u8,
    target: Option<UnitId>,
) {
    let t = target.map_or(ModeTarget::Point(0, 0), ModeTarget::Unit);
    let game = &mut *cv.game;
    AiModes::change_mode(&mut cv.v, game, d, mode, t);
}

/// `0x005A43A0(game, D)`: the umod dispatcher in mode 4 on the lent
/// monster world (`umod-callbacks.md` §2 rule 5).
fn umod_get_hit<X: Pending>(cv: &mut CombatView<'_, X>, d: UnitId) {
    let mut sim = Sim {
        game: &mut *cv.game,
        units: &mut *cv.v.units,
        stats: &mut *cv.v.stats,
        data: cv.v.data,
    };
    cv.v.h.run_umods(&mut sim, d, None, umod_mode::GET_HIT);
}

/// Step 4.7's path: soft, AI state 19, umod mode 4.
fn soft_reaction<X: Pending>(cv: &mut CombatView<'_, X>, d: UnitId) {
    soft(cv, d);
    cv.v.h.set_monster_ai_state(d, AI_STATE_HIT);
    umod_get_hit(cv, d);
}

/// `damage.md` §7.1 step 4: a monster defender.
fn monster_hit<X: Pending>(
    cv: &mut CombatView<'_, X>,
    a: UnitId,
    d: UnitId,
    rec: &mut DamageRecord,
) {
    let Some((mode, class)) = cv.v.units.get(d).map(|r| (r.mode, r.class)) else {
        return;
    };
    if mode == monster_mode::DT || mode == monster_mode::DD {
        return;
    }
    let has_mode = |cv: &CombatView<'_, X>, m: u8| cv.v.h.x.class_has_mode(class as i32, m);
    // Step 4.1.
    if rec.result & result::KNOCKBACK != 0 && !has_mode(cv, MONSTER_KB) {
        rec.result = (rec.result & !result::KNOCKBACK) | result::GET_HIT;
    }
    // Step 4.2 (`0x005A54F0`).
    if rec.result & result::GET_HIT != 0 {
        let t = cv.v.h.tables.clone();
        let leaper = t
            .combat
            .monstats
            .get(class as usize)
            .is_some_and(|m| m.baseid == SANDLEAPER_BASE);
        if leaper && !cv.v.stats.has_state(d, STATE_FREEZE) {
            rec.result |= result::KNOCKBACK;
        }
    }
    let f = rec.result;
    // Step 4.3.
    if f & result::WILL_DIE != 0 {
        kill(cv, d, a);
        return;
    }
    // Step 4.4.
    if f & result::KNOCKBACK != 0 {
        monster_request(cv, d, MONSTER_KB, Some(a));
        return;
    }
    // Step 4.5.
    if f & result::BLOCK != 0 {
        if f & result::SOFT_HIT != 0
            || NO_BLOCK_CLASSES.contains(&class)
            || !has_mode(cv, MONSTER_BL)
        {
            cv.v.h.set_monster_ai_state(d, AI_STATE_HIT);
        } else {
            monster_request(cv, d, MONSTER_BL, None);
        }
        return;
    }
    // Step 4.6.
    if f & result::GET_HIT != 0 {
        if cv.v.state_list(d, STATE_STUNNED as u16).is_some() || enters_get_hit(cv, d, rec) {
            monster_request(cv, d, monster_mode::GH as u8, None);
            umod_get_hit(cv, d);
        } else {
            soft_reaction(cv, d);
        }
        return;
    }
    // Step 4.7.
    if f & result::SOFT_HIT != 0 {
        soft_reaction(cv, d);
        return;
    }
    // Step 4.8.
    if f & result::HIT != 0 && rec.total > 0 {
        let b = cv.v.stats.unit_total(d, LAST_SENT_HP_PCT, 0) & 0xFF;
        let hp = cv.v.stats.unit_total(d, crate::stats::stat::HITPOINTS, 0);
        let c = crate::stats::life_fraction(hp, cv.v.stats.max_life(d)) & 0xFF;
        if b.wrapping_sub(c).wrapping_abs() > 4 {
            soft(cv, d);
        }
    }
}

/// Player modes of step 5 (`sim/units.md` §4.1).
const PLAYER_GH: u32 = 4;
const PLAYER_BL: u32 = 9;
const PLAYER_SEQUENCE: u32 = 13;
const PLAYER_KB: u32 = 19;
/// States 65 `dodge`, 66 `avoid`, 68 `evade`.
const STATE_DODGE: u16 = 65;
const STATE_AVOID: u16 = 66;
const STATE_EVADE: u16 = 68;
/// Stat 350 (the state's skill), 95 `lastblockframe`, 102
/// `item_fasterblockrate`.
const STAT_STATE_SKILL: u16 = 350;
const LAST_BLOCK_FRAME: u16 = 95;
const FASTER_BLOCK_RATE: u16 = 102;
/// Skill entry flag 4 (`0x006446A0` / `0x00644660`).
const ENTRY_FLAG_REACTION: u32 = 4;
/// Sound event 12 (`audio/triggers.md`).
const SOUND_EVADE: u16 = 12;

/// A player mode request (`0x005809D0` point form / `0x00580A70` unit
/// form, re-entry 0): the walk code when the path provider is on, else
/// [`Pending::player_mode_request`].
fn player_request<X: Pending>(
    cv: &mut CombatView<'_, X>,
    d: UnitId,
    skill: Option<u16>,
    mode: u32,
    target: WalkTarget,
) {
    let game = &mut *cv.game;
    if cv.v.h.paths.is_some() {
        crate::wiring::path::walk::player_request(&mut cv.v, game, d, skill, mode, target);
    } else {
        cv.v.h.x.player_mode_request(game, d, skill, mode, target);
    }
}

/// The unit form's target (tA, gA).
fn attacker_target<X: Pending>(cv: &CombatView<'_, X>, a: UnitId) -> WalkTarget {
    let (ty, guid) = CombatWorld::ident(cv, a);
    WalkTarget::Unit { ty, guid }
}

/// Steps 5.1 / 5.2: s := stat 350 of the state's list, E := the
/// highest entry of skill s (`0x00625D00`, `highest_entry`).
fn state_skill<X: Pending>(
    cv: &CombatView<'_, X>,
    d: UnitId,
    st: u16,
) -> Option<crate::skills::SkillEntry> {
    let l = cv.v.state_list(d, st)?;
    let s = cv.v.stats.base(l, STAT_STATE_SKILL, 0);
    crate::skills::highest_entry(&cv.v.h.skill_list_of(d), s)
}

/// The skill form of steps 5.1 / 5.2: E flags |= 4, then the unit form
/// request (E, mode 13, tA, gA, 0).
fn skill_form<X: Pending>(
    cv: &mut CombatView<'_, X>,
    a: UnitId,
    d: UnitId,
    e: crate::skills::SkillEntry,
) {
    let f = cv.v.h.x.entry_flags(d, &e);
    cv.v.h.x.set_entry_flags(d, &e, f | ENTRY_FLAG_REACTION);
    let t = attacker_target(cv, a);
    player_request(cv, d, Some(e.skill as u16), PLAYER_SEQUENCE, t);
}

/// `damage.md` §7.1 step 5: a player defender (first matching flag
/// wins).
fn player_hit<X: Pending>(cv: &mut CombatView<'_, X>, a: UnitId, d: UnitId, rec: &DamageRecord) {
    let f = rec.result;
    // Step 5.1.
    if f & (result::DODGE | result::AVOID) != 0 {
        let st = if f & result::DODGE != 0 {
            STATE_DODGE
        } else {
            STATE_AVOID
        };
        if let Some(e) = state_skill(cv, d, st) {
            skill_form(cv, a, d, e);
        }
        return;
    }
    // Step 5.2.
    if f & result::EVADE != 0 {
        if let Some(e) = state_skill(cv, d, STATE_EVADE) {
            skill_form(cv, a, d, e);
            let t = cv.v.h.tables.clone();
            if t.skills
                .skill(e.skill)
                .is_some_and(|r| r.stsound as i16 > 0)
            {
                if let Err(err) = crate::units::sound::queue_sound(cv.game, d, SOUND_EVADE, Some(d))
                {
                    cv.v.unit_error(crate::game::GameError::from(err).into());
                }
            }
        }
        return;
    }
    // Step 5.3.
    if f & (result::BLOCK | result::WEAPON_BLOCK) != 0 {
        if f & result::SOFT_HIT != 0 {
            return;
        }
        let frame = cv.game.frame;
        let last = cv.v.stats.unit_total(d, LAST_BLOCK_FRAME, 0);
        let rate = cv.v.stats.unit_total(d, FASTER_BLOCK_RATE, 0);
        if frame.wrapping_sub(last) > rate / 8 + 15 {
            player_request(cv, d, None, PLAYER_BL, WalkTarget::Point(Point::new(0, 0)));
            cv.v.set_base(d, LAST_BLOCK_FRAME, frame);
        }
        return;
    }
    // Step 5.4.
    if f & result::WILL_DIE != 0 {
        let mode = cv.v.units.get(d).map_or(0, |r| r.mode);
        if mode == player_mode::DT || mode == player_mode::DD {
            return;
        }
        // The unit form (no skill, mode 0, tA, gA, 0) resolves A and,
        // past the mode and interrupt checks (`pathing.md` §1.3, §1.4:
        // allowed for mode 0), runs the DT start with K = A.
        let game = &mut *cv.game;
        cv.v.start_player_death(game, d, Some(a));
        return;
    }
    // Step 5.5.
    if f & result::KNOCKBACK != 0 {
        let t = attacker_target(cv, a);
        player_request(cv, d, None, PLAYER_KB, t);
        return;
    }
    // Step 5.6.
    if f & result::GET_HIT != 0 {
        if cv.v.state_list(d, STATE_STUNNED as u16).is_none() && !enters_get_hit(cv, d, rec) {
            soft(cv, d);
        } else {
            let p = WalkTarget::Point(Point::new(rec.total, 0));
            player_request(cv, d, None, PLAYER_GH, p);
        }
        return;
    }
    // Step 5.7.
    if f & result::SOFT_HIT != 0 {
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
    kill_by(cv, d, Some(a));
}

/// [`kill`] with the killer A optional (`damage.md` §7.2: "A present"
/// gates step 2; step 3 faces D's current direction without A). The
/// monster death by regeneration (`stat-lists.md` §10.1 step 6) passes
/// the poison / open-wounds owner, which may be gone.
///
/// TODO(damage.md §7.2 steps 1, 3): without A, the [`KillStep`] seams
/// (pet credit, quest kill parse, barricade doors) take a unit and are
/// not called.
pub fn kill_by<X: Pending>(cv: &mut CombatView<'_, X>, d: UnitId, a: Option<UnitId>) {
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
            if let Some(a) = a {
                cv.v.h.x.kill_step(game, KillStep::PetCredit, d, a);
            }
        }
        _ => return,
    }
    // Step 2.
    if let Some(a) = a {
        if flags & UNIT_FLAG_NO_EXPERIENCE == 0 {
            if let Some(t) = cv.v.h.vitals.clone() {
                distribute(cv, &t, a, d);
            }
        }
        // The arena kill event `0x0053F720`.
        if let (Some(row), Some(ka), Some(kd)) = (
            cv.v.h.tables.arena.first().map(ArenaRow::from),
            cv.v.units.get(a).map(|r| r.ty),
            cv.v.units.get(d).map(|r| r.ty),
        ) {
            if let Some(st) = cv.v.h.arena.as_mut() {
                st.kill_event(&row, &mut *cv.game, (a, ka), (d, kd));
            }
        }
        let game = &mut *cv.game;
        cv.v.h
            .x
            .kill_step(game, KillStep::AttackerBookkeeping, d, a);
    }
    if ty != UnitType::Monster {
        return;
    }
    // Step 3.
    let game = &mut *cv.game;
    if let Some(a) = a {
        // The death request's direction toward A (`0x00621DC0(D, A x, A
        // y)`), applied by the mode set's snap `0x006488A0`
        // (`sim/pathing.md` §8.5).
        let at = cv.v.h.path_position(a);
        if let Some(dir) = cv.v.path_dir64(d, at) {
            cv.v.path_snap_direction(d, dir);
        }
        cv.v.h.x.kill_step(game, KillStep::FaceAttacker, d, a);
    }
    cv.v.h.mode_target = a;
    cv.v.monster_set_mode(game, d, monster_mode::DT);
    cv.v.h.mode_target = None;
    if let Some(a) = a {
        if !cv.v.h.x.is_revived(d) {
            cv.v.h.x.kill_step(game, KillStep::QuestKill, d, a);
        }
        cv.v.h.x.kill_step(game, KillStep::BarricadeDoors, d, a);
    }
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
    /// `0x0064C040`: the unit joins its room's update queue, so its
    /// player update sends the changed stats (`stat-lists.md` §11 rule 4).
    fn refresh(&mut self, u: UnitId) {
        VitalsUnits::refresh(&mut self.v, u);
        let _ = self.game.lists.queue_update(u);
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
