// Spec: specs/combat/vitals.md §2–§5; specs/skills/levels.md §6.4; specs/sim/stat-lists.md §11
//! Experience and level-up in the play preview (`docs/handoff/q-levelup.md`):
//! the server's slot for C→S 0x3A (stat point) and 0x3B (skill point), and
//! the skill book that slot reads and writes.
//!
//! 0x3A runs the vitals handler on the action wiring's stat lists. 0x3B runs
//! the wired skill-point handler (`handlers::skills::wired`) over a
//! [`SkillBook`], the preview's player skill list (the seam `Pending::
//! skill_list` / `UseRest` / `LearnRest` answer). Every other skill id stays
//! a stub here (the casting session owns them).
//!
//! The book starts empty: a point spent on a class skill adds the entry at
//! level 1 (the saved skill levels reach the book once the join applies
//! them). // d2rs-own, unverified; PROVISIONAL (skills/levels.md §6.4), settled
//! by REC-94.

use std::collections::BTreeMap;

use d2_server::adapters::handlers::skills::wired::WiredSkills;
use d2_server::adapters::handlers::skills::{Call, Handled, LearnRest, SkillHost, SkillRest};
use d2_server::adapters::handlers::world::ActionEvents;
use d2_sim::game::Game;
use d2_sim::skills::use_::{ModeTarget, ServerMsg, UseState};
use d2_sim::skills::SkillEntry;
use d2_sim::units::UnitId;
use d2_sim::wiring::interaction::UseRest;

use super::single_player::LocalSeams;

/// The server's player skill list (unit +0xA8) and the facts the skill-point
/// handler reads of the player (class, GUID).
#[derive(Debug, Default)]
pub struct SkillBook {
    /// Per player: the entries in list order.
    pub lists: BTreeMap<UnitId, Vec<SkillEntry>>,
    /// Per player: (class, GUID), set by [`LevelUpSkills`] before a call.
    pub who: BTreeMap<UnitId, (i32, u32)>,
    /// `skills.txt` `charclass` by skill id (−1: none), set before a call.
    pub charclass: Vec<i32>,
    /// Left / right selected entries and the used entry.
    pub right: BTreeMap<UnitId, SkillEntry>,
    pub left: BTreeMap<UnitId, SkillEntry>,
    pub used: BTreeMap<UnitId, SkillEntry>,
}

impl SkillBook {
    fn find(&self, u: UnitId, skill: i32) -> Option<SkillEntry> {
        self.lists
            .get(&u)?
            .iter()
            .copied()
            .find(|e| e.skill == skill)
    }
}

/// S→C 0x21 UpdateItemOSkill (`server-messages.tsv`): type 0 (player),
/// remove 0, GUID, skill, base level, bonus 0, one pad byte.
fn update_skill(guid: u32, skill: i32, level: i32) -> Vec<u8> {
    let mut m = vec![0x21, 0, 0];
    m.extend_from_slice(&guid.to_le_bytes());
    m.extend_from_slice(&(skill as u16).to_le_bytes());
    m.push(level as u8);
    // Bonus level 0; the message is 12 bytes (`server-messages.tsv`).
    m.extend_from_slice(&[0, 0]);
    m
}

impl LearnRest for LocalSeams {
    /// `0x0056C700`: the skill's `charclass` is the player's class.
    fn is_class_skill(&self, u: UnitId, skill: i32) -> bool {
        let b = &self.book;
        let class = b.who.get(&u).map(|w| w.0);
        let sc = usize::try_from(skill)
            .ok()
            .and_then(|i| b.charclass.get(i).copied());
        class.is_some() && class == sc
    }
    /// `0x00570080` after the cost check: the point cost is spent by the
    /// world (stat 5), the entry gains a level and the client is told.
    fn add_skill_level(&mut self, u: UnitId, skill: i32, _cost: i32) {
        let b = &mut self.book;
        let list = b.lists.entry(u).or_default();
        let level = match list.iter_mut().find(|e| e.skill == skill) {
            Some(e) => {
                e.base += 1;
                e.base
            }
            None => {
                list.push(SkillEntry {
                    skill,
                    base: 1,
                    owner_guid: -1,
                    ..SkillEntry::default()
                });
                1
            }
        };
        let guid = b.who.get(&u).map_or(0, |w| w.1);
        let msg = update_skill(guid, skill, level);
        self.sent.push((u, msg));
    }
    fn after_skill_point(&mut self, _: UnitId) {}
}

/// The skill use pipeline's rest, narrowest: the skill list is the book's,
/// everything else answers "none / no" (casting is not wired here).
impl UseRest for LocalSeams {
    fn send(&mut self, u: UnitId, msg: ServerMsg) {
        self.log.push(format!("skill send {} {msg:?}", u.0));
    }
    fn has_player_data(&self, _: UnitId) -> bool {
        true
    }
    fn last_point_frame(&self, _: UnitId) -> i32 {
        0
    }
    fn set_last_point_frame(&mut self, _: UnitId, _: i32) {}
    fn cursor_item(&self, _: UnitId) -> bool {
        false
    }
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
        self.book.left.get(&u).copied()
    }
    fn right_skill(&self, u: UnitId) -> Option<SkillEntry> {
        self.book.right.get(&u).copied()
    }
    fn set_left_skill(&mut self, u: UnitId, e: SkillEntry) {
        self.book.left.insert(u, e);
    }
    fn set_right_skill(&mut self, u: UnitId, e: SkillEntry) {
        self.book.right.insert(u, e);
    }
    fn find_entry(&self, u: UnitId, skill: i32) -> Option<SkillEntry> {
        self.book.find(u, skill)
    }
    fn find_entry_owned(&self, _: UnitId, _: i32, _: i32) -> Option<SkillEntry> {
        None
    }
    fn owns_skill(&self, u: UnitId, skill: i32) -> bool {
        self.book.find(u, skill).is_some()
    }
    fn set_used_skill(&mut self, u: UnitId, e: Option<SkillEntry>) {
        match e {
            Some(e) => self.book.used.insert(u, e),
            None => self.book.used.remove(&u),
        };
    }
    fn used_skill_flags(&self, _: UnitId) -> u32 {
        0
    }
    fn set_used_skill_flags(&mut self, _: UnitId, _: u32) {}
    fn entry_mode(&self, _: UnitId, _: &SkillEntry) -> u32 {
        0
    }
    fn attack_param4(&self, _: UnitId) -> i32 {
        0
    }
    fn set_attack_param4(&mut self, _: UnitId, _: i32) {}
    fn use_state(&mut self, _: UnitId, _: &SkillEntry) -> UseState {
        UseState::Usable
    }
    fn shapeshifted(&self, _: UnitId) -> bool {
        false
    }
    fn consume_charges(&mut self, _: UnitId, _: &SkillEntry) -> bool {
        true
    }
    fn pay_life(&mut self, _: UnitId, _: i32) -> bool {
        true
    }
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
    fn start_mode(&mut self, _: &mut Game, _: UnitId, _: u32, _: ModeTarget<UnitId>) {}
    fn run_to(&mut self, _: UnitId, _: UnitId, _: SkillEntry) {}
    fn target(&self, _: UnitId) -> Option<UnitId> {
        None
    }
    fn clear_target(&mut self, _: UnitId) {}
    fn event_arg(&self, _: UnitId) -> i32 {
        0
    }
    fn set_event_arg(&mut self, _: UnitId, _: i32) {}
    fn step_path(&mut self, _: UnitId) -> i32 {
        0
    }
    fn target_position(&self, _: UnitId) -> Option<(i32, i32)> {
        None
    }
    fn line_clear(&self, _: UnitId, _: (i32, i32), _: u32) -> bool {
        false
    }
    fn set_aura_state(&mut self, _: UnitId, _: u16, _: i32, _: i32) {}
    fn srvst(&mut self, _: u16, _: UnitId, _: i32, _: i32) -> i32 {
        0
    }
    fn srvdo(&mut self, _: u16, _: UnitId, _: i32, _: i32, _: bool, _: bool, _: bool) -> i32 {
        0
    }
}

/// The access to the preview's [`SkillBook`] through the sim's `Pending`
/// value.
pub trait HasBook {
    fn book_mut(&mut self) -> &mut SkillBook;
}

impl HasBook for LocalSeams {
    fn book_mut(&mut self) -> &mut SkillBook {
        &mut self.book
    }
}

/// The skill slot of the play world: 0x3A and 0x3B only.
#[derive(Debug, Default)]
pub struct LevelUpSkills(pub WiredSkills);

impl<D: ActionEvents> SkillHost<D> for LevelUpSkills
where
    D::X: SkillRest + HasBook,
{
    fn handle(&mut self, call: Call<'_, D>) -> Option<Handled> {
        if !matches!(call.msg.first(), Some(0x3A | 0x3B)) {
            return None;
        }
        let player = call.staged.player;
        let action = call.events.action();
        let class_guid = action
            .sys
            .units
            .get(player)
            .map(|r| (r.class as i32, r.guid));
        let charclass: Vec<i32> = action
            .hooks()
            .tables
            .skills
            .skills
            .iter()
            .map(|r| i32::from(r.charclass as i8))
            .collect();
        let book = action.hooks().x.book_mut();
        if let Some(w) = class_guid {
            book.who.insert(player, w);
        }
        book.charclass = charclass;
        self.0.handle(call)
    }
}
