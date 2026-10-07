// Spec: specs/client/msg-skills.md §1, §2 (rules 1–3, 8); specs/skills/use.md §2; specs/skills/levels.md §6.4; specs/formats/d2s-load.md §1 r1; specs/formats/d2s.md §7.2 r2
//! The server-side skill lists of the app's game and the skill-pipeline
//! seams no written spec provides yet (`UseRest`, `LearnRest`), answered
//! for the play preview on [`super::single_player::LocalSeams`], the
//! action wiring's `Pending` value and so the skill list's one owner
//! (`d2_sim::wiring::interaction::skill_use` module docs).
//!
//! [`SkillStore`] holds each unit's list (unit +0xA8, `msg-skills.md`
//! §1) and runs the shared list operations of §2 on it, the same code as
//! the client's (`crate::bridge::skills`: "the same code runs on the
//! server", §2 heading). Its rows are the action wiring's `skills` and
//! `charstats` tables ([`SkillStore::from_tables`]). The server player
//! init `0x005348C0` runs the native skills `0x00647EE0` right after the
//! list is created (§2 rule 8): [`SkillStore::init_player`], called by
//! the app's character loader right after the player is allocated, so the
//! load's `StartSkill` test (`0x006439F0`) and the save's skill levels
//! (`d2s.md` §7.2) see the list.
//!
//! Everything else here is a preview fill (decision D1): the narrowest
//! answer unless noted, each marked `// d2rs-own, unverified`.

use std::collections::BTreeMap;

use d2_data::tables::{Charstats, Skills};
use d2_server::adapters::handlers::skills::LearnRest;
use d2_sim::game::Game;
use d2_sim::skills::use_::{ModeTarget, ServerMsg, UseState};
use d2_sim::skills::SkillEntry;
use d2_sim::units::UnitId;
use d2_sim::wiring::action::ActionTables;
use d2_sim::wiring::interaction::UseRest;

use super::single_player::LocalSeams;
use crate::bridge::skills::{self as list_ops, Owner, SkillError, SkillList, NATIVE};
use crate::bridge::world::{SkillRow, PLAYER};

/// One unit's skill state on the server: the list (entries, left, right
/// and current entry, §1 rule 2) and the per-unit fields the skill use
/// pipeline reads and writes (`use.md` §2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnitSkills {
    /// Unit type and class (the add rule's mode choice, §2 rule 1).
    pub owner: Option<Owner>,
    pub list: SkillList,
    /// Used-skill flags (bit 0 moving, bit 1 arrived).
    pub used_flags: u32,
    /// Param4 of the Attack entry (`use.md` §2 step 2).
    pub attack_param4: i32,
    /// Unit +0x38 bits 8+.
    pub event_arg: i32,
    /// Player data +0x168 (the handlers use the staged value; kept for
    /// the timer path).
    pub last_point_frame: i32,
}

/// The server-side skill lists of the game (module docs).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkillStore {
    /// The `skills` fields the list operations read, one per skill id
    /// (the skill count is their number).
    pub rows: Vec<SkillRow>,
    /// Each `charstats` row's `Skill 1`…`Skill 10` (§2 rule 8).
    pub class_skills: Vec<[u16; 10]>,
    pub units: BTreeMap<UnitId, UnitSkills>,
    /// What the list operations owed the unit that has no provider here
    /// (passive states, refreshes) and the seams' unanswered calls, in
    /// call order.
    pub log: Vec<String>,
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

/// A list entry as the sim's skill seams see it.
fn sim_entry(e: &list_ops::SkillEntry) -> SkillEntry {
    SkillEntry {
        skill: i32::from(e.skill),
        base: e.base,
        level_bonus: e.level_bonus,
        owner_guid: e.owner as i32,
        charges: e.charges,
        has_charges: e.has_charges,
    }
}

impl SkillStore {
    /// The rows of the action wiring's tables (`skills`, `charstats`).
    pub fn from_tables(t: &ActionTables) -> Self {
        Self {
            rows: t.skills.skills.iter().map(skill_row).collect(),
            class_skills: t.combat.charstats.iter().map(class_skills).collect(),
            ..Self::default()
        }
    }

    /// The server player init's native skills `0x00647EE0` (§2 rule 8) on
    /// a new list of player `u` of `class`: skill 0 and the class's
    /// `charstats` `Skill 1`…`Skill 10` (base 1, owner −1), Attack in
    /// both hands; a class outside the rows gets an empty list.
    pub fn init_player(&mut self, u: UnitId, class: u32) {
        let owner = Owner {
            unit_type: PLAYER,
            class,
        };
        let mut s = UnitSkills {
            owner: Some(owner),
            ..UnitSkills::default()
        };
        let skills = usize::try_from(class)
            .ok()
            .and_then(|c| self.class_skills.get(c));
        let r = list_ops::init_player(&mut s.list, &self.rows, owner, skills);
        self.note(u, &mut s.list, r);
        self.units.insert(u, s);
    }

    /// The unit's list in list order, as the sim sees it.
    pub fn list(&self, u: UnitId) -> Vec<SkillEntry> {
        self.units
            .get(&u)
            .map(|s| s.list.entries.iter().map(sim_entry).collect())
            .unwrap_or_default()
    }

    fn entry(&self, u: UnitId, i: Option<usize>) -> Option<SkillEntry> {
        let s = self.units.get(&u)?;
        s.list.entries.get(i?).map(sim_entry)
    }

    /// The entry of (skill, owner), §1 rule 4.
    fn index(&self, u: UnitId, e: &SkillEntry) -> Option<usize> {
        let skill = u16::try_from(e.skill).ok()?;
        self.units.get(&u)?.list.find(skill, e.owner_guid as u32)
    }

    /// Select (§2 rule 3) of `hand` (left `true`) to the entry (skill,
    /// owner); `false`: no list, or a skill outside the table (fatal
    /// 0x668 in 1.14d, logged).
    pub fn select(&mut self, u: UnitId, left: bool, skill: i32, owner: u32) -> bool {
        let Ok(skill) = u16::try_from(skill) else {
            return false;
        };
        let Some(s) = self.units.get_mut(&u) else {
            return false;
        };
        match list_ops::select(&mut s.list, &self.rows, left, skill, owner) {
            Ok(()) => true,
            Err(e) => {
                self.log.push(format!("select {} {skill}: {e}", u.0));
                false
            }
        }
    }

    /// Assign `0x00647280(unit, skill, level, remove)` (§2 rule 2);
    /// `false`: the unit has no list.
    pub fn assign(&mut self, u: UnitId, skill: i32, level: i32, remove: bool) -> bool {
        let Some(mut s) = self.units.remove(&u) else {
            return false;
        };
        // A skill id outside u16 is outside the table: add finds no row.
        let r = match (u16::try_from(skill), s.owner) {
            (Ok(k), Some(o)) => list_ops::assign(&mut s.list, &self.rows, o, k, level, remove),
            _ => Ok(()),
        };
        self.note(u, &mut s.list, r);
        self.units.insert(u, s);
        true
    }

    /// The operations' owed effects (§2 rules 1, 2.2, 4: passive states
    /// and refreshes on the unit's stat lists) and errors, logged.
    // PROVISIONAL (client/msg-skills.md §2 r4): the server's passive-state
    // stat lists are not wired to this list; the effects are logged, not
    // applied. d2rs-own, unverified.
    fn note(&mut self, u: UnitId, list: &mut SkillList, r: Result<(), SkillError>) {
        for fx in list.fx.drain(..) {
            self.log.push(format!("skill fx {} {fx:?}", u.0));
        }
        if let Err(e) = r {
            self.log.push(format!("skill list {}: {e}", u.0));
        }
    }

    fn unit(&self, u: UnitId) -> Option<&UnitSkills> {
        self.units.get(&u)
    }

    fn unit_mut(&mut self, u: UnitId) -> Option<&mut UnitSkills> {
        self.units.get_mut(&u)
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
        self.skills
            .unit(u)
            .is_some_and(|s| s.owner.is_some_and(|o| o.unit_type == PLAYER))
    }
    fn last_point_frame(&self, u: UnitId) -> i32 {
        self.skills.unit(u).map_or(0, |s| s.last_point_frame)
    }
    fn set_last_point_frame(&mut self, u: UnitId, frame: i32) {
        if let Some(s) = self.skills.unit_mut(u) {
            s.last_point_frame = frame;
        }
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
    fn left_skill(&self, u: UnitId) -> Option<SkillEntry> {
        let i = self.skills.unit(u)?.list.left;
        self.skills.entry(u, i)
    }
    fn right_skill(&self, u: UnitId) -> Option<SkillEntry> {
        let i = self.skills.unit(u)?.list.right;
        self.skills.entry(u, i)
    }
    fn set_left_skill(&mut self, u: UnitId, e: SkillEntry) {
        self.skills.select(u, true, e.skill, e.owner_guid as u32);
    }
    fn set_right_skill(&mut self, u: UnitId, e: SkillEntry) {
        self.skills.select(u, false, e.skill, e.owner_guid as u32);
    }
    // PROVISIONAL (skills/use.md §2 step 3): the lookup is not named; the
    // first entry of the skill in list order, any owner (as `0x006439F0`
    // tests). d2rs-own, unverified.
    fn find_entry(&self, u: UnitId, skill: i32) -> Option<SkillEntry> {
        self.skills.list(u).into_iter().find(|e| e.skill == skill)
    }
    fn find_entry_owned(&self, u: UnitId, skill: i32, owner: i32) -> Option<SkillEntry> {
        self.skills
            .list(u)
            .into_iter()
            .find(|e| e.skill == skill && e.owner_guid == owner)
    }
    /// `0x006439F0`: an entry of the skill, any owner.
    fn owns_skill(&self, u: UnitId, skill: i32) -> bool {
        self.skills.list(u).iter().any(|e| e.skill == skill)
    }
    /// The current entry (§1 rule 2, +0x10; `0x00620210`).
    fn set_used_skill(&mut self, u: UnitId, e: Option<SkillEntry>) {
        let i = e.and_then(|e| self.skills.index(u, &e));
        if let Some(s) = self.skills.unit_mut(u) {
            s.list.current = i;
        }
    }
    fn used_skill_flags(&self, u: UnitId) -> u32 {
        self.skills.unit(u).map_or(0, |s| s.used_flags)
    }
    fn set_used_skill_flags(&mut self, u: UnitId, f: u32) {
        if let Some(s) = self.skills.unit_mut(u) {
            s.used_flags = f;
        }
    }
    /// Entry +0x08, fixed by add (§2 rule 1).
    fn entry_mode(&self, u: UnitId, e: &SkillEntry) -> u32 {
        let i = self.skills.index(u, e);
        self.skills
            .unit(u)
            .and_then(|s| s.list.entries.get(i?))
            .map_or(0, |e| e.mode)
    }
    fn attack_param4(&self, u: UnitId) -> i32 {
        self.skills.unit(u).map_or(0, |s| s.attack_param4)
    }
    fn set_attack_param4(&mut self, u: UnitId, v: i32) {
        if let Some(s) = self.skills.unit_mut(u) {
            s.attack_param4 = v;
        }
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
    // d2rs-own, unverified: no shapeshift states in the preview.
    fn shapeshifted(&self, _: UnitId) -> bool {
        false
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
    fn target(&self, _: UnitId) -> Option<UnitId> {
        None
    }
    fn clear_target(&mut self, _: UnitId) {}
    fn event_arg(&self, u: UnitId) -> i32 {
        self.skills.unit(u).map_or(0, |s| s.event_arg)
    }
    fn set_event_arg(&mut self, u: UnitId, a: i32) {
        if let Some(s) = self.skills.unit_mut(u) {
            s.event_arg = a;
        }
    }
    // d2rs-own, unverified: no path step here (0: not finished).
    fn step_path(&mut self, _: UnitId) -> i32 {
        0
    }
    fn target_position(&self, _: UnitId) -> Option<(i32, i32)> {
        None
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

/// The `Pending` skill-list calls of [`LocalSeams`] (its `impl Pending`
/// delegates here).
impl LocalSeams {
    pub(super) fn skill_list_of(&self, u: UnitId) -> Vec<SkillEntry> {
        self.skills.list(u)
    }
    pub(super) fn used_skill_of(&self, u: UnitId) -> Option<SkillEntry> {
        let i = self.skills.unit(u)?.list.current;
        self.skills.entry(u, i)
    }
    /// `0x005701B0`'s list part: the hand := the entry (skill, −1).
    pub(super) fn select_hand(&mut self, u: UnitId, left: bool, skill: i32) -> bool {
        if !self.skills.units.contains_key(&u) {
            return false;
        }
        self.skills.select(u, left, skill, NATIVE);
        true
    }
    /// `0x0056DEB0(unit, skill, level, 1)` → assign (§2 rule 2).
    pub(super) fn assign_level(&mut self, u: UnitId, skill: i32, level: i32) -> bool {
        self.skills.assign(u, skill, level, true)
    }
}
