// Spec: specs/client/msg-skills.md §1, §2 (rules 1–3, 8); specs/skills/use.md §2; specs/skills/levels.md §6.4; specs/formats/d2s-load.md §1 r1; specs/formats/d2s.md §7.2 r2
//! The skill-pipeline seams no written spec provides yet (`UseRest`,
//! `LearnRest`), answered for the play preview on
//! [`super::single_player::LocalSeams`], the action wiring's `Pending`
//! value.
//!
//! The skill lists themselves are d2-sim's (`ActionHooks::skill_lists`,
//! `client/msg-skills.md` §1; the join's native skills §2 rule 8 run in
//! `d2_server::adapters::character`): the skill-use wiring answers every
//! list call from them when the unit has one, so the list answers here
//! are reached only for a unit without a list (the narrowest: none).
//! [`SkillStore`] keeps the per-unit fields of the use pipeline that are
//! not list fields (`use.md` §2).
//!
//! Everything else here is a preview fill (decision D1): the narrowest
//! answer unless noted, each marked `// d2rs-own, unverified`.

use std::collections::BTreeMap;

use d2_data::tables::{Charstats, Skills};
use d2_server::adapters::handlers::skills::LearnRest;
use d2_sim::game::Game;
use d2_sim::skills::use_::{ModeTarget, ServerMsg, UseState};
use d2_sim::skills::SkillEntry;
use d2_sim::units::{UnitId, UnitType};
use d2_sim::wiring::interaction::UseRest;

use super::single_player::LocalSeams;
use crate::bridge::world::SkillRow;

/// The per-unit fields the skill use pipeline reads and writes besides
/// the list (`use.md` §2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnitSkills {
    /// Used-skill flags (bit 0 moving, bit 1 arrived).
    pub used_flags: u32,
    /// Param4 of the Attack entry (`use.md` §2 step 2).
    pub attack_param4: i32,
    /// Unit +0x38 bits 8+.
    pub event_arg: i32,
    /// Player data +0x168 (the handlers use the staged value; kept for
    /// the timer path).
    pub last_point_frame: i32,
    /// The target of the last mode start (`UseRest::keep_target`).
    pub target: Option<ModeTarget<UnitId>>,
}

/// The use pipeline's per-unit fields of the game (module docs).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkillStore {
    pub units: BTreeMap<UnitId, UnitSkills>,
    /// The seams' unanswered calls, in call order.
    pub log: Vec<String>,
    /// The players in a shapeshift form (werewolf / werebear), refreshed
    /// by [`sync_shapes`] (q-druid).
    pub shifted: std::collections::BTreeSet<UnitId>,
}

/// The players whose state is the `aurastate` of a `srvdofunc` 116 skill
/// (Werewolf, Werebear: `bodies.md` §8.15), for [`SkillStore::shifted`]:
/// `UseRest::shapeshifted` has no game to read. d2rs-own, unverified.
pub fn sync_shapes(game: &Game, sim: &mut d2_sim::wiring::worldgen::WorldSim<LocalSeams>) {
    let hooks = sim.action.sys.hooks.tables.clone();
    let forms: Vec<u32> = hooks
        .skills
        .skills
        .iter()
        .filter(|r| r.srvdofunc == 116)
        .map(|r| u32::from(r.aurastate))
        .collect();
    let shifted = game
        .lists
        .units_of_type(UnitType::Player)
        .into_iter()
        .filter(|&u| forms.iter().any(|&s| sim.action.sys.stats.has_state(u, s)))
        .collect();
    sim.action.sys.hooks.x.skills.shifted = shifted;
}

/// The `skills` fields of the list operations (`client/msg-skills.md`
/// Inputs) of one record.
pub fn skill_row(s: &Skills) -> SkillRow {
    SkillRow {
        anim: s.anim,
        monanim: s.monanim,
        passivestate: s.passivestate,
        maxlvl: s.maxlvl,
        charclass: s.charclass as i8,
        srvdofunc: s.srvdofunc as i16,
        enhanceable: s.enhanceable,
        skilldesc: s.skilldesc,
        etype: s.etype,
        range: s.range,
        flags: crate::bridge::combat::skill_flags(s),
    }
}

/// A `charstats` row's `Skill 1`…`Skill 10`.
pub fn class_skills(c: &Charstats) -> [u16; 10] {
    [
        c.skill_1, c.skill_2, c.skill_3, c.skill_4, c.skill_5, c.skill_6, c.skill_7, c.skill_8,
        c.skill_9, c.skill_10,
    ]
}

impl SkillStore {
    fn unit(&self, u: UnitId) -> Option<&UnitSkills> {
        self.units.get(&u)
    }

    fn unit_mut(&mut self, u: UnitId) -> &mut UnitSkills {
        self.units.entry(u).or_default()
    }
}

impl LocalSeams {
    fn note(&mut self, s: String) {
        self.skills.log.push(s);
    }
}

/// The skill use pipeline's rest on the app's seams. The skill list and
/// the per-unit skill fields are [`SkillStore`]'s; the message handlers'
/// player data, positions, messages and mode start are the server's own
/// (`d2_server::adapters::handlers::skills::world::World`), so the
/// answers below for those are reached only on the timer path.
impl UseRest for LocalSeams {
    // d2rs-own, unverified: the messages of `use.md` OQ9 have no layout;
    // logged.
    fn send(&mut self, u: UnitId, msg: ServerMsg) {
        self.note(format!("skill send {} {msg:?}", u.0));
    }
    // d2rs-own, unverified: players have player data (the loader sets
    // it), no other unit does.
    fn has_player_data(&self, u: UnitId) -> bool {
        self.sides
            .get(&u)
            .is_some_and(|&(ty, _, _)| ty == UnitType::Player)
    }
    fn last_point_frame(&self, u: UnitId) -> i32 {
        self.skills.unit(u).map_or(0, |s| s.last_point_frame)
    }
    fn set_last_point_frame(&mut self, u: UnitId, frame: i32) {
        self.skills.unit_mut(u).last_point_frame = frame;
    }
    // d2rs-own, unverified: no cursor item (the inventory model is the
    // wired host's, not reachable here).
    fn cursor_item(&self, _: UnitId) -> bool {
        false
    }
    // d2rs-own, unverified (relations and reach not specified): the
    // narrowest answers.
    fn in_own_inventory(&self, _: UnitId, _: UnitId) -> bool {
        false
    }
    fn within_reach(&self, _: UnitId, _: UnitId) -> bool {
        false
    }
    fn owner(&self, _: UnitId) -> Option<UnitId> {
        None
    }
    fn is_pet(&self, _: UnitId, _: UnitId) -> bool {
        false
    }
    fn is_ally(&self, _: UnitId, _: UnitId) -> bool {
        false
    }
    // The list calls: reached only for a unit without a d2-sim list
    // (module docs); d2rs-own, unverified: none.
    fn left_skill(&self, _: UnitId) -> Option<SkillEntry> {
        None
    }
    fn right_skill(&self, _: UnitId) -> Option<SkillEntry> {
        None
    }
    fn set_left_skill(&mut self, _: UnitId, _: SkillEntry) {}
    fn set_right_skill(&mut self, _: UnitId, _: SkillEntry) {}
    fn find_entry(&self, _: UnitId, _: i32) -> Option<SkillEntry> {
        None
    }
    fn find_entry_owned(&self, _: UnitId, _: i32, _: i32) -> Option<SkillEntry> {
        None
    }
    fn owns_skill(&self, _: UnitId, _: i32) -> bool {
        false
    }
    fn set_used_skill(&mut self, _: UnitId, _: Option<SkillEntry>) {}
    fn used_skill_flags(&self, u: UnitId) -> u32 {
        self.skills.unit(u).map_or(0, |s| s.used_flags)
    }
    fn set_used_skill_flags(&mut self, u: UnitId, f: u32) {
        self.skills.unit_mut(u).used_flags = f;
    }
    fn entry_mode(&self, _: UnitId, _: &SkillEntry) -> u32 {
        0
    }
    fn attack_param4(&self, u: UnitId) -> i32 {
        self.skills.unit(u).map_or(0, |s| s.attack_param4)
    }
    fn set_attack_param4(&mut self, u: UnitId, v: i32) {
        self.skills.unit_mut(u).attack_param4 = v;
    }
    // PROVISIONAL (skills/use.md §2: `0x00647960`'s parts are listed, not
    // their order): an entry with a level is usable, one without is
    // `NoLevel`; mana, quantity, shape, cooldown are not checked here.
    // d2rs-own, unverified.
    fn use_state(&mut self, _: UnitId, e: &SkillEntry) -> UseState {
        if e.base + e.level_bonus > 0 {
            UseState::Usable
        } else {
            UseState::NoLevel
        }
    }
    // d2rs-own, unverified: the player has the state of a Werewolf /
    // Werebear row ([`sync_shapes`], refreshed before each intent and tick).
    fn shapeshifted(&self, u: UnitId) -> bool {
        self.skills.shifted.contains(&u)
    }
    // d2rs-own, unverified: a native entry has no charges to spend; an
    // item entry's charges are refused (no item provider).
    fn consume_charges(&mut self, _: UnitId, e: &SkillEntry) -> bool {
        e.is_native() && !e.has_charges
    }
    // d2rs-own, unverified: a life cost is refused (the stat write is not
    // reachable from here).
    fn pay_life(&mut self, u: UnitId, cost: i32) -> bool {
        if cost <= 0 {
            return true;
        }
        self.note(format!("pay life {} {cost}: refused", u.0));
        false
    }
    // d2rs-own, unverified (items not reachable): no equipment.
    fn can_dual_wield(&self, _: UnitId) -> bool {
        false
    }
    fn equippable(&self, _: UnitId) -> bool {
        false
    }
    fn bow_equipped(&self, _: UnitId) -> bool {
        false
    }
    fn state_mask(&self, _: UnitId, _: u32) -> bool {
        false
    }
    // The message handlers run the start on the unit records (`World`);
    // on the timer path it is logged. d2rs-own, unverified.
    fn start_mode(&mut self, _: &mut Game, u: UnitId, mode: u32, _: ModeTarget<UnitId>) {
        self.note(format!("start mode {} {mode}", u.0));
    }
    // d2rs-own, unverified (path spec not written): logged.
    fn run_to(&mut self, u: UnitId, target: UnitId, e: SkillEntry) {
        self.note(format!("run to {} {} skill {}", u.0, target.0, e.skill));
    }
    fn target(&self, u: UnitId) -> Option<UnitId> {
        // The monster side: the AI's request target, kept past the skill
        // start's clear (q-monster-ai; the next request replaces it).
        let kept = self.skills.unit(u).and_then(|s| match s.target? {
            ModeTarget::Unit(t) => Some(t),
            ModeTarget::Point(..) => None,
        });
        kept.or_else(|| self.monsters.target(u))
    }
    fn clear_target(&mut self, u: UnitId) {
        self.skills.unit_mut(u).target = None;
    }
    // d2rs-own, unverified (use.md §4): the mode start's target kept per
    // unit.
    fn keep_target(&mut self, u: UnitId, target: ModeTarget<UnitId>) {
        self.skills.unit_mut(u).target = Some(target);
    }
    fn event_arg(&self, u: UnitId) -> i32 {
        self.skills.unit(u).map_or(0, |s| s.event_arg)
    }
    fn set_event_arg(&mut self, u: UnitId, a: i32) {
        self.skills.unit_mut(u).event_arg = a;
    }
    // d2rs-own, unverified: no path step here (0: not finished).
    fn step_path(&mut self, _: UnitId) -> i32 {
        0
    }
    // d2rs-own, unverified: the kept point, or the kept unit's position.
    fn target_position(&self, u: UnitId) -> Option<(i32, i32)> {
        match self.skills.unit(u)?.target? {
            ModeTarget::Point(x, y) => Some((x, y)),
            ModeTarget::Unit(t) => self
                .sides
                .get(&t)
                .map(|s| s.2)
                .or_else(|| self.pos.get(&t).copied()),
        }
    }
    // d2rs-own, unverified (collision not reachable): blocked.
    fn line_clear(&self, _: UnitId, _: (i32, i32), _: u32) -> bool {
        false
    }
    fn set_aura_state(&mut self, u: UnitId, state: u16, skill: i32, lvl: i32) {
        self.note(format!("aura state {} {state} {skill} {lvl}", u.0));
    }
    // PROVISIONAL (skills/use.md OQ10): the per-skill bodies are
    // catalogued only; logged, result 0. d2rs-own, unverified.
    fn srvst(&mut self, index: u16, u: UnitId, skill: i32, lvl: i32) -> i32 {
        self.note(format!("srvst {index} {} {skill} {lvl}", u.0));
        0
    }
    fn srvdo(&mut self, i: u16, u: UnitId, s: i32, l: i32, c: bool, it: bool, a: bool) -> i32 {
        self.note(format!("srvdo {i} {} {s} {l} {c} {it} {a}", u.0));
        0
    }
}

/// The skill-point calls (`levels.md` §6.4).
// PROVISIONAL (skills/levels.md §6.4): `0x0056C700` and the spend
// `0x00570080` need the player's stat list (stat 5), which this seam
// cannot reach: no skill is a class skill, so 0x3B is refused with code 3
// before any spend. d2rs-own, unverified.
impl LearnRest for LocalSeams {
    fn is_class_skill(&self, _: UnitId, _: i32) -> bool {
        false
    }
    fn add_skill_level(&mut self, u: UnitId, skill: i32, cost: i32) {
        self.note(format!("add skill level {} {skill} {cost}", u.0));
    }
    fn after_skill_point(&mut self, u: UnitId) {
        self.note(format!("after skill point {}", u.0));
    }
}
