// Spec: specs/skills/use.md §1–§7; specs/skills/bodies.md (BodyWorld); specs/missiles/missiles.md §R2; specs/sim/units.md §4.1, §4.2; specs/sim/tick.md §5.2–§5.4; specs/sim/stat-lists.md §4, §8.1, §9.2
//! Skill use → missiles, combat and the timers: the seams of
//! [`crate::skills::use_`] on the action wiring's providers.
//!
//! [`UseView`] wraps the action wiring's
//! [`crate::wiring::action::combat::CombatView`] (game, unit records,
//! stat lists, shared action state), so [`SkillUnits`] is combat's.
//! Real here: the frame, GUID lookups, the act test, unit modes (the
//! plain mode set `0x00553570`), unit flags, the alive test, the ENDANIM
//! expire, hostility and melee range (the action wiring's seams for the
//! same addresses), the room kind, timer scheduling and deletion, the
//! skill-delay and aura state lists, stat writes, and missile creation
//! through [`crate::missiles::create_missile`] on the real missile store.
//! The skill bodies of `functions.tsv` status `spec'd-here` run on this
//! view ([`BodyWorld`], below): real stat lists, states, timers, rooms,
//! the combat code and the handler lists of [`ActionHooks`]; the parts
//! of other unwritten systems through [`Pending`]'s skill-body seams.
//! The skill list, player data, paths, items and the other per-skill
//! bodies have no provider: [`UseRest`], implemented by the same
//! [`Pending`] value as the action wiring's other open seams (one owner
//! for the skill list, which combat reads through
//! [`Pending::skill_list`]).

use crate::combat::events::{self, EventTables, EventWorld, ItemCastMsg, RaiseStep};
use crate::combat::{CombatWorld, RoomKind};
use crate::game::Game;
use crate::missiles::{self, MissileParams};
use crate::rng::Seed;
use crate::skills::use_::bodies::{self, BodyWorld};
use crate::skills::use_::{
    MissileAim, ModeTarget, ServerMsg, SkillFunctions, UseMissiles, UseState, UseWorld,
};
use crate::skills::{KickItems, ManaUnits, SkillEntry, SkillUnits};
use crate::stats::lists::{ListId, RemoveCallback};
use crate::tick::events::event;
use crate::units::anim::{self, Form};
use crate::units::{RoomId, UnitId, UnitType};
use crate::wiring::action::combat::CombatView;
use crate::wiring::action::{ActionSim, Pending, View, WiringError};
use crate::wiring::interaction::body_path;

/// `skilldelay` (state 121, `use.md` §6).
const STATE_SKILL_DELAY: u16 = 121;
/// List flags of the delay list (`use.md` §6: "flags 2").
const DELAY_LIST_FLAGS: u32 = 2;
/// The delay list's remove callback `0x0056E900` (an opaque id; the
/// stat host gives it its meaning, `stat-lists.md` §4).
pub const DELAY_REMOVE_CALLBACK: RemoveCallback = RemoveCallback(0x0056_E900);

/// The skill use calls no written spec provides yet. Implemented by the
/// action wiring's [`Pending`] provider (the skill list's one owner).
pub trait UseRest {
    // ---- messages (d2-server)
    fn send(&mut self, u: UnitId, msg: ServerMsg);
    // ---- player data (player spec)
    fn has_player_data(&self, u: UnitId) -> bool;
    fn last_point_frame(&self, u: UnitId) -> i32;
    fn set_last_point_frame(&mut self, u: UnitId, frame: i32);
    fn cursor_item(&self, u: UnitId) -> bool;
    // ---- relations, reach (not specified)
    fn in_own_inventory(&self, u: UnitId, item: UnitId) -> bool;
    fn within_reach(&self, a: UnitId, b: UnitId) -> bool;
    fn owner(&self, u: UnitId) -> Option<UnitId>;
    fn is_pet(&self, a: UnitId, b: UnitId) -> bool;
    fn is_ally(&self, a: UnitId, b: UnitId) -> bool;
    // ---- the unit's skill list (units / skills; `use.md` §2)
    fn left_skill(&self, u: UnitId) -> Option<SkillEntry>;
    fn right_skill(&self, u: UnitId) -> Option<SkillEntry>;
    fn set_left_skill(&mut self, u: UnitId, e: SkillEntry);
    fn set_right_skill(&mut self, u: UnitId, e: SkillEntry);
    fn find_entry(&self, u: UnitId, skill: i32) -> Option<SkillEntry>;
    fn find_entry_owned(&self, u: UnitId, skill: i32, owner: i32) -> Option<SkillEntry>;
    fn owns_skill(&self, u: UnitId, skill: i32) -> bool;
    /// The entry [`Pending::used_skill`] then returns.
    fn set_used_skill(&mut self, u: UnitId, e: Option<SkillEntry>);
    fn used_skill_flags(&self, u: UnitId) -> u32;
    fn set_used_skill_flags(&mut self, u: UnitId, f: u32);
    fn entry_mode(&self, u: UnitId, e: &SkillEntry) -> u32;
    fn attack_param4(&self, u: UnitId) -> i32;
    fn set_attack_param4(&mut self, u: UnitId, v: i32);
    /// `0x00647960` (`use.md` §2: parts listed, order not).
    fn use_state(&mut self, u: UnitId, e: &SkillEntry) -> UseState;
    // ---- mana (`skills/levels.md` §4)
    fn shapeshifted(&self, u: UnitId) -> bool;
    fn consume_charges(&mut self, u: UnitId, e: &SkillEntry) -> bool;
    fn pay_life(&mut self, u: UnitId, cost: i32) -> bool;
    // ---- equipment (items, inventory)
    fn can_dual_wield(&self, u: UnitId) -> bool;
    fn equippable(&self, item: UnitId) -> bool;
    fn bow_equipped(&self, u: UnitId) -> bool;
    fn state_mask(&self, u: UnitId, mask: u32) -> bool;
    /// The weapon in use and the item on a body location as the skill
    /// bodies see them (ammo check, bow missile, `bodies.md` §2.3–§2.5),
    /// ahead of combat's [`Pending::current_weapon`] / [`Pending::item_at`]:
    /// a host whose equipped items do not yet feed the stat lists lets the
    /// skills see its weapon without changing the melee damage rules.
    /// Default: not answered (combat's).
    fn skill_weapon(&self, _u: UnitId) -> Option<UnitId> {
        None
    }
    fn skill_item_at(&self, _u: UnitId, _loc: u8) -> Option<UnitId> {
        None
    }
    // ---- modes and paths (units.md player modes, path spec)
    fn start_mode(&mut self, game: &mut Game, u: UnitId, mode: u32, target: ModeTarget<UnitId>);
    fn run_to(&mut self, u: UnitId, target: UnitId, e: SkillEntry);
    fn target(&self, u: UnitId) -> Option<UnitId>;
    fn clear_target(&mut self, u: UnitId);
    /// The target of a mode start: the point or unit the skill's missile
    /// and checks read back through [`UseRest::target`] and
    /// [`UseRest::target_position`] (`use.md` §4: where `0x0057FE90` /
    /// `0x0057FEF0` store it is not stated). Default: not kept.
    fn keep_target(&mut self, _u: UnitId, _target: ModeTarget<UnitId>) {}
    fn event_arg(&self, u: UnitId) -> i32;
    fn set_event_arg(&mut self, u: UnitId, a: i32);
    fn step_path(&mut self, u: UnitId) -> i32;
    fn target_position(&self, u: UnitId) -> Option<(i32, i32)>;
    fn line_clear(&self, u: UnitId, to: (i32, i32), mask: u32) -> bool;
    // ---- aura state (`use.md` §7: list contents not written)
    fn set_aura_state(&mut self, u: UnitId, state: u16, skill: i32, lvl: i32);
    // ---- skill code (the slots `bodies` does not specify)
    fn srvst(&mut self, index: u16, u: UnitId, skill: i32, lvl: i32) -> i32;
    #[allow(clippy::too_many_arguments)]
    fn srvdo(
        &mut self,
        index: u16,
        u: UnitId,
        skill: i32,
        lvl: i32,
        charge: bool,
        item: bool,
        aim: bool,
    ) -> i32;
}

/// The skill use pipeline's world: combat's view plus the skill use
/// seams.
pub struct UseView<'a, X> {
    pub cv: CombatView<'a, X>,
}

impl<X: Pending + UseRest> UseView<'_, X> {
    /// The item type test `0x00643F80` of `use_state` test 5 (`use.md`
    /// §2 "Item type test"): the skill's sets a (`itypea1..3`,
    /// `etypea1..2`) and b (`itypeb1..3`, `etypeb1..2`) against the items
    /// at body locations 4 (A) and 5 (B).
    // PROVISIONAL (q-fix-real-item-type-test): "no inventory → fail"
    // (rule 2) and the matched item's rules (item flags 0x4000 / 0x100,
    // the `shoots` ammo test) are not applied: the host's item flags and
    // `shoots` type are not wired here (`item_shoots` answers false).
    fn weapon_type_ok(&self, u: UnitId, skill: i32) -> bool {
        let Some(r) = self.cv.v.h.tables.skills.skill(skill) else {
            return false;
        };
        // i16 columns, ≤ 0 = none.
        let v = |x: u16| i32::from(x as i16);
        let a = HandSet {
            itypes: [v(r.itypea1), v(r.itypea2), v(r.itypea3)],
            etypes: [v(r.etypea1), v(r.etypea2)],
        };
        let b = HandSet {
            itypes: [v(r.itypeb1), v(r.itypeb2), v(r.itypeb3)],
            etypes: [v(r.etypeb1), v(r.etypeb2)],
        };
        // Rule 1.
        if a.etypes[0] <= 0 && a.itypes[0] <= 0 {
            return true;
        }
        let (mut ha, mut hb) = (self.item_at(u, 4), self.item_at(u, 5));
        // Rule 3: Left Hand Throw / Swing leave the weapon in use out.
        if skill == 4 || skill == 5 {
            let w = self.current_weapon(u);
            if ha.is_some() && ha == w {
                ha = None;
            } else if hb.is_some() && hb == w {
                hb = None;
            }
        }
        // `hand(s, X, Y)` `0x00643D90`.
        let hand = |s: &HandSet, x: Option<UnitId>, y: Option<UnitId>| match x {
            None => {
                if s.etypes[0] <= 0 && s.itypes[0] <= 0 {
                    return true;
                }
                [45, 46, 67].contains(&a.itypes[0])
                    && b.itypes[0] <= 0
                    && !y.is_some_and(|y| self.item_is(y, 45))
            }
            Some(x) => {
                let listed =
                    |t: &[i32]| t.iter().copied().take_while(|&t| t > 0).collect::<Vec<_>>();
                if listed(&s.etypes).into_iter().any(|t| self.item_is(x, t)) {
                    return false;
                }
                let want = listed(&s.itypes);
                want.is_empty() || want.into_iter().any(|t| self.item_is(x, t))
            }
        };
        // Rule 4.
        if hand(&a, ha, hb) {
            hand(&b, hb, ha)
        } else {
            hand(&b, ha, hb) && hand(&a, hb, ha)
        }
    }
}

/// One item-type set of a skill row (`use.md` §2).
struct HandSet {
    itypes: [i32; 3],
    etypes: [i32; 2],
}

impl<X: Pending + UseRest> ActionSim<X> {
    /// Runs `f` with the skill use pipeline's view (message handlers,
    /// the do / periodic event handlers, tests).
    pub fn skill_use<R>(&mut self, game: &mut Game, f: impl FnOnce(&mut UseView<'_, X>) -> R) -> R {
        let s = &mut self.sys;
        let mut w = UseView {
            cv: CombatView {
                game,
                v: View::of(&mut s.units, &mut s.stats, &s.data, &mut s.hooks),
            },
        };
        f(&mut w)
    }
}

impl<X: Pending + UseRest> UseView<'_, X> {
    /// The unit's list in `ActionHooks::skill_lists` (`client/msg-skills.md`
    /// §1), when it has one: the list calls answer from it, the seam
    /// otherwise.
    fn list(&self, u: UnitId) -> Option<&crate::skills::list::SkillList> {
        self.cv.v.h.skill_lists.get(&u)
    }
    fn dynamic_path(&self, u: UnitId) -> Option<&crate::path::DynamicPath> {
        self.cv.v.h.paths.as_ref()?.dynamic(u)
    }
    fn list_entry(&self, u: UnitId, e: &SkillEntry) -> Option<&crate::skills::list::ListEntry> {
        let l = self.list(u)?;
        l.entries.get(l.find(e.skill, e.owner_guid)?)
    }
    fn list_entry_mut(
        &mut self,
        u: UnitId,
        e: &SkillEntry,
    ) -> Option<&mut crate::skills::list::ListEntry> {
        let l = self.list_mut(u)?;
        let i = l.find(e.skill, e.owner_guid)?;
        l.entries.get_mut(i)
    }
    fn list_mut(&mut self, u: UnitId) -> Option<&mut crate::skills::list::SkillList> {
        self.cv.v.h.skill_lists.get_mut(&u)
    }
    pub(super) fn x(&self) -> &X {
        &self.cv.v.h.x
    }
    pub(super) fn xm(&mut self) -> &mut X {
        &mut self.cv.v.h.x
    }
    fn error(&mut self, e: WiringError) {
        self.cv.v.h.errors.push(e);
    }
    /// A §4.2 variant (`sim/units.md`) on the unit's own animation
    /// fields: cancel its type-0 / type-1 events, reschedule, set +0x44.
    fn anim_variant(&mut self, u: UnitId, form: Form) {
        let Some(rec) = self.cv.v.units.get_mut(u) else {
            return;
        };
        if let Err(e) = anim::run(self.cv.game, u, &mut rec.anim, form) {
            self.error(WiringError::Unit(e.into()));
        }
    }
}

// ---- SkillUnits: combat's ------------------------------------------------

impl<X: Pending + UseRest> SkillUnits for UseView<'_, X> {
    type Unit = UnitId;
    type Item = UnitId;

    fn unit_type(&self, u: UnitId) -> UnitType {
        self.cv.unit_type(u)
    }
    fn class_id(&self, u: UnitId) -> i32 {
        self.cv.class_id(u)
    }
    fn stat(&self, u: UnitId, stat: u16, layer: u16) -> i32 {
        SkillUnits::stat(&self.cv, u, stat, layer)
    }
    fn item_stat(&self, u: UnitId, stat: u16, layer: u16) -> i32 {
        SkillUnits::item_stat(&self.cv, u, stat, layer)
    }
    fn base_stat(&self, u: UnitId, stat: u16, layer: u16) -> i32 {
        SkillUnits::base_stat(&self.cv, u, stat, layer)
    }
    fn formula_stat(&self, u: UnitId, stat: u16, mode: i32) -> i32 {
        self.cv.formula_stat(u, stat, mode)
    }
    fn stat_entries(&self, u: UnitId, stat: u16, max: usize) -> Vec<(u16, i32)> {
        self.cv.stat_entries(u, stat, max)
    }
    fn has_state(&self, u: UnitId, state: u16) -> bool {
        SkillUnits::has_state(&self.cv, u, state)
    }
    fn state_stat(&self, u: UnitId, state: u16, stat: u16) -> Option<i32> {
        SkillUnits::state_stat(&self.cv, u, state, stat)
    }
    fn seed(&mut self, u: UnitId) -> &mut Seed {
        SkillUnits::seed(&mut self.cv, u)
    }
    fn skill_list(&self, u: UnitId) -> Vec<SkillEntry> {
        self.cv.skill_list(u)
    }
    fn used_skill(&self, u: UnitId) -> Option<SkillEntry> {
        self.cv.used_skill(u)
    }
    fn current_weapon(&self, u: UnitId) -> Option<UnitId> {
        self.x()
            .skill_weapon(u)
            .or_else(|| self.cv.current_weapon(u))
    }
    fn weapon(&self, u: UnitId) -> Option<UnitId> {
        self.x().skill_weapon(u).or_else(|| self.cv.weapon(u))
    }
    fn item_at(&self, u: UnitId, loc: u8) -> Option<UnitId> {
        self.x()
            .skill_item_at(u, loc)
            .or_else(|| self.cv.item_at(u, loc))
    }
    fn item_is(&self, item: UnitId, itype: i32) -> bool {
        self.cv.item_is(item, itype)
    }
    fn itype_is(&self, itype: i32, parent: i32) -> bool {
        self.cv.itype_is(itype, parent)
    }
    fn wield_type(&self, item: UnitId) -> i32 {
        self.cv.wield_type(item)
    }
    fn item_damage(&self, item: UnitId, max: bool) -> i32 {
        self.cv.item_damage(item, max)
    }
    fn str_dex_bonus(&self, item: UnitId) -> (i32, i32) {
        self.cv.str_dex_bonus(item)
    }
    fn item_flag_throw(&self, item: UnitId) -> bool {
        self.cv.item_flag_throw(item)
    }
    fn missile_level(&self, u: UnitId) -> i32 {
        self.cv.missile_level(u)
    }
}

impl<X: Pending + UseRest> ManaUnits for UseView<'_, X> {
    fn shapeshifted(&self, u: UnitId) -> bool {
        self.x().shapeshifted(u)
    }
    fn consume_charges(&mut self, u: UnitId, entry: &SkillEntry) -> bool {
        self.xm().consume_charges(u, entry)
    }
    fn pay_life(&mut self, u: UnitId, cost: i32) -> bool {
        self.xm().pay_life(u, cost)
    }
    /// Unit set `0x00627260(unit, stat, value, 0)`.
    fn set_stat(&mut self, u: UnitId, stat: u16, value: i32) {
        self.cv.v.set_base(u, stat, value);
    }
}

/// The bodies of `functions.tsv` status `spec'd-here` run here
/// ([`bodies::run_start`] / [`bodies::run_do`] on this view); every other
/// slot goes to [`UseRest::srvst`] / [`UseRest::srvdo`].
impl<X: Pending + UseRest> SkillFunctions for UseView<'_, X> {
    fn srvst(&mut self, index: u16, u: UnitId, skill: i32, lvl: i32) -> i32 {
        let t = self.cv.v.h.tables.clone();
        match bodies::run_start(self, &t.skills, &t.combat, index, u, skill, lvl) {
            Some(v) => v,
            None => self.xm().srvst(index, u, skill, lvl),
        }
    }
    fn srvdo(
        &mut self,
        index: u16,
        u: UnitId,
        skill: i32,
        lvl: i32,
        charge: bool,
        item: bool,
        aim: bool,
    ) -> i32 {
        let t = self.cv.v.h.tables.clone();
        match bodies::run_do(self, &t.skills, &t.combat, index, u, skill, lvl) {
            Some(v) => v,
            None => self.xm().srvdo(index, u, skill, lvl, charge, item, aim),
        }
    }
}

impl<X: Pending + UseRest> UseMissiles for UseView<'_, X> {
    /// The `srvmissile` path (`bodies.md` §5): `skill_missile`
    /// (`0x0056EE90` for `lob`, else `0x0056ECB0`, §2.4) with `quant` = 0,
    /// on the real missile store ([`BodyWorld::spawn_missile`]).
    fn create_skill_missile(
        &mut self,
        u: UnitId,
        skill: i32,
        lvl: i32,
        missile: u16,
        lob: bool,
        aim: MissileAim,
    ) {
        let (d, at) = match aim {
            MissileAim::None => ((0, 0), (0, 0)),
            MissileAim::At { offset, aim } => (offset, aim),
        };
        bodies::skill_missile(self, i32::from(missile), u, skill, lvl, d, at, false, lob);
    }
}

impl<X: Pending + UseRest> UseWorld for UseView<'_, X> {
    fn frame(&self) -> i32 {
        self.cv.game.frame
    }
    fn send(&mut self, u: UnitId, msg: ServerMsg) {
        UseRest::send(self.xm(), u, msg);
    }
    fn has_player_data(&self, u: UnitId) -> bool {
        self.x().has_player_data(u)
    }
    fn last_point_frame(&self, u: UnitId) -> i32 {
        self.x().last_point_frame(u)
    }
    fn set_last_point_frame(&mut self, u: UnitId, frame: i32) {
        self.xm().set_last_point_frame(u, frame);
    }
    /// The path position (`ActionHooks::path_position`: the path
    /// provider's, else [`Pending::position`]).
    fn position(&self, u: UnitId) -> (i32, i32) {
        self.cv.v.h.path_position(u)
    }
    /// `0x00552F60` on the type's hash (`unit-order.md` §2.3).
    fn find_unit(&self, ty: u32, guid: u32) -> Option<UnitId> {
        let ty = *UnitType::ALL.get(usize::try_from(ty).ok()?)?;
        self.cv.game.lists.find_unit(ty, guid)
    }
    fn in_own_inventory(&self, u: UnitId, item: UnitId) -> bool {
        self.x().in_own_inventory(u, item)
    }
    /// The act fields (unit +0x18) are equal.
    fn same_act(&self, a: UnitId, b: UnitId) -> bool {
        let act = |u| self.cv.v.units.get(u).map(|r| r.act);
        act(a).is_some() && act(a) == act(b)
    }
    fn within_reach(&self, a: UnitId, b: UnitId) -> bool {
        self.x().within_reach(a, b)
    }
    fn owner(&self, u: UnitId) -> Option<UnitId> {
        self.x().owner(u)
    }

    fn left_skill(&self, u: UnitId) -> Option<SkillEntry> {
        match self.list(u) {
            Some(l) => l.left.and_then(|i| l.view().get(i).copied()),
            None => self.x().left_skill(u),
        }
    }
    fn right_skill(&self, u: UnitId) -> Option<SkillEntry> {
        match self.list(u) {
            Some(l) => l.right.and_then(|i| l.view().get(i).copied()),
            None => self.x().right_skill(u),
        }
    }
    fn set_left_skill(&mut self, u: UnitId, e: SkillEntry) {
        match self.list_mut(u) {
            Some(l) => l.left = l.find(e.skill, e.owner_guid),
            None => self.xm().set_left_skill(u, e),
        }
    }
    fn set_right_skill(&mut self, u: UnitId, e: SkillEntry) {
        match self.list_mut(u) {
            Some(l) => l.right = l.find(e.skill, e.owner_guid),
            None => self.xm().set_right_skill(u, e),
        }
    }
    fn find_entry(&self, u: UnitId, skill: i32) -> Option<SkillEntry> {
        match self.list(u) {
            Some(l) => l.view().into_iter().find(|e| e.skill == skill),
            None => self.x().find_entry(u, skill),
        }
    }
    fn find_entry_owned(&self, u: UnitId, skill: i32, owner: i32) -> Option<SkillEntry> {
        match self.list(u) {
            Some(l) => l.find(skill, owner).and_then(|i| l.view().get(i).copied()),
            None => self.x().find_entry_owned(u, skill, owner),
        }
    }
    fn owns_skill(&self, u: UnitId, skill: i32) -> bool {
        match self.list(u) {
            Some(l) => l.has(skill),
            None => self.x().owns_skill(u, skill),
        }
    }
    fn set_used_skill(&mut self, u: UnitId, e: Option<SkillEntry>) {
        match self.list_mut(u) {
            Some(l) => l.current = e.and_then(|e| l.find(e.skill, e.owner_guid)),
            None => self.xm().set_used_skill(u, e),
        }
    }
    /// The used entry's flags word (+0x0C; `use.md` §5.2 step 2): the
    /// skill list's entry when the unit has one, else the seam.
    fn used_skill_flags(&self, u: UnitId) -> u32 {
        match self.list(u).and_then(|l| l.entries.get(l.current?)) {
            Some(e) => e.flags,
            None => self.x().used_skill_flags(u),
        }
    }
    fn set_used_skill_flags(&mut self, u: UnitId, f: u32) {
        match self.list_mut(u).and_then(|l| l.entries.get_mut(l.current?)) {
            Some(e) => e.flags = f,
            None => self.xm().set_used_skill_flags(u, f),
        }
    }
    fn entry_mode(&self, u: UnitId, e: &SkillEntry) -> u32 {
        match self.list(u) {
            Some(l) => l
                .find(e.skill, e.owner_guid)
                .map_or(0, |i| l.entries[i].mode),
            None => self.x().entry_mode(u, e),
        }
    }
    fn attack_param4(&self, u: UnitId) -> i32 {
        self.x().attack_param4(u)
    }
    fn set_attack_param4(&mut self, u: UnitId, v: i32) {
        self.xm().set_attack_param4(u, v);
    }
    fn use_state(&mut self, u: UnitId, e: &SkillEntry) -> UseState {
        let st = self.xm().use_state(u, e);
        if st == UseState::Usable && !self.weapon_type_ok(u, e.skill) {
            return UseState::NoQuantity;
        }
        st
    }
    /// `0x0056C3F0` (`bodies.md` §2.5).
    fn dec_quantity(&mut self, u: UnitId, _skill: i32) {
        bodies::dec_quantity(self, u);
    }

    fn can_dual_wield(&self, u: UnitId) -> bool {
        self.x().can_dual_wield(u)
    }
    fn equippable(&self, item: UnitId) -> bool {
        self.x().equippable(item)
    }
    fn bow_equipped(&self, u: UnitId) -> bool {
        self.x().bow_equipped(u)
    }
    fn state_mask(&self, u: UnitId, mask: u32) -> bool {
        self.x().state_mask(u, mask)
    }
    /// `0x00622C40(a, d, 0x00622870(a))` (the action wiring's seams).
    fn in_melee_range(&self, u: UnitId, target: UnitId) -> bool {
        let x = self.x();
        x.in_melee_range(u, target, x.melee_range(u))
    }

    /// Unit +0x10.
    fn mode(&self, u: UnitId) -> u32 {
        self.cv.v.units.get(u).map_or(0, |r| r.mode)
    }
    fn cursor_item(&self, u: UnitId) -> bool {
        self.x().cursor_item(u)
    }
    /// `0x005415A0`: the smallest positive expire frame of the unit's
    /// type-1 timers (`tick.md` §5), 0 if none.
    fn endanim_expire(&self, u: UnitId) -> i32 {
        let t = &self.cv.game.timers;
        t.unit_timers(u)
            .into_iter()
            .filter(|&id| t.event(id).is_some_and(|e| e.0 == event::END_ANIM))
            .filter_map(|id| t.expire(id))
            .filter(|&e| e > 0)
            .min()
            .unwrap_or(0)
    }
    /// The plain mode set `0x00553570` (`units.md` §4.1).
    fn set_mode(&mut self, u: UnitId, mode: u32) {
        let r = {
            let v = &mut self.cv.v;
            let mut sim = crate::units::hooks::Sim {
                game: self.cv.game,
                units: v.units,
                stats: v.stats,
                data: v.data,
            };
            crate::units::modes::set_mode(&mut sim, &mut *v.h, u, mode)
        };
        if let Err(e) = r {
            self.error(WiringError::Unit(e));
        }
    }
    fn start_mode(&mut self, u: UnitId, mode: u32, target: ModeTarget<UnitId>) {
        let game = &mut *self.cv.game;
        self.cv.v.h.x.start_mode(game, u, mode, target);
        self.cv.v.h.x.keep_target(u, target);
    }
    /// The unit-form run request on the path provider; without one the
    /// host's seam.
    fn run_to(&mut self, u: UnitId, target: UnitId, e: SkillEntry) {
        if self.cv.v.h.paths.is_some() {
            let mut p = crate::wiring::path::walk::PathCtx::of(&mut self.cv.v, &mut *self.cv.game);
            p.run_to_unit(u, target, e.skill as u16);
        } else {
            self.xm().run_to(u, target, e);
        }
    }
    fn target(&self, u: UnitId) -> Option<UnitId> {
        UseRest::target(self.x(), u)
    }
    fn clear_target(&mut self, u: UnitId) {
        self.xm().clear_target(u);
    }
    /// Unit +0xC4.
    fn unit_flags(&self, u: UnitId) -> u32 {
        self.cv.v.units.get(u).map_or(0, |r| r.flags)
    }
    fn set_unit_flags(&mut self, u: UnitId, f: u32) {
        if let Some(r) = self.cv.v.units.get_mut(u) {
            r.flags = f;
        }
    }
    fn event_arg(&self, u: UnitId) -> i32 {
        self.x().event_arg(u)
    }
    fn set_event_arg(&mut self, u: UnitId, a: i32) {
        self.xm().set_event_arg(u, a);
    }
    /// `0x00553490` / `0x00554CA0`: the path provider's step (2 when the
    /// path is finished); without one the host's seam.
    fn step_path(&mut self, u: UnitId) -> i32 {
        if self.cv.v.h.paths.is_none() {
            return self.xm().step_path(u);
        }
        let mut p = crate::wiring::path::walk::PathCtx::of(&mut self.cv.v, &mut *self.cv.game);
        let st = p.step(u);
        match st {
            Some(crate::path::walk::Step::Stopped) => 2,
            _ => 0,
        }
    }
    /// `0x005541B0` on the unit record.
    fn is_alive(&self, u: UnitId) -> bool {
        !self.cv.v.units.is_dead(u)
    }

    /// `0x00554200` (the action wiring's [`Pending::may_attack`]).
    fn is_hostile(&self, a: UnitId, b: UnitId) -> bool {
        self.x().may_attack(a, b)
    }
    fn is_pet(&self, a: UnitId, b: UnitId) -> bool {
        self.x().is_pet(a, b)
    }
    fn is_ally(&self, a: UnitId, b: UnitId) -> bool {
        self.x().is_ally(a, b)
    }
    /// Combat's room kind (`0x00620BB0`, `0x0061AB00`).
    fn room(&self, u: UnitId) -> RoomKind {
        CombatWorld::room(&self.cv, u)
    }
    /// The host's answer, else (a monster's skill, which has no player
    /// skill target) the path's target: the target unit's position, or
    /// the path's target point (`0x0056D2C0`, `pathing.md` §13.2 r2–3,
    /// read-only here: no stale-target clearing). Both coordinates must
    /// be non-zero.
    fn target_position(&self, u: UnitId) -> Option<(i32, i32)> {
        if let Some(p) = self.x().target_position(u) {
            return Some(p);
        }
        let d = self.cv.v.h.paths.as_ref()?.dynamic(u)?;
        let (x, y) = match d.target_unit {
            Some(t)
                if t.unit != u && self.cv.game.lists.find_unit(t.ty, t.guid) == Some(t.unit) =>
            {
                self.cv.v.h.path_position(t.unit)
            }
            _ => (i32::from(d.target_x), i32::from(d.target_y)),
        };
        (x != 0 && y != 0).then_some((x, y))
    }
    fn line_clear(&self, u: UnitId, to: (i32, i32), mask: u32) -> bool {
        self.rooms_line_clear(u, to, mask)
            .unwrap_or_else(|| self.x().line_clear(u, to, mask))
    }

    /// `0x005416B0` (`tick.md` §5.2).
    fn schedule(&mut self, u: UnitId, kind: u8, frame: i32, arg1: i32, arg2: i32) {
        let r =
            self.cv
                .game
                .schedule_event(u, u32::from(kind), frame, None, arg1 as u32, arg2 as u32);
        if let Err(e) = r {
            self.error(WiringError::Unit(e.into()));
        }
    }
    /// `tick.md` §5.4: the unit's timers of `kind` with that arg1.
    fn delete_timers(&mut self, u: UnitId, kind: u8, arg1: i32) {
        self.cv
            .game
            .timers
            .cancel_unit_events(u, kind, Some(arg1 as u32));
    }

    fn has_state_list(&self, u: UnitId, state: u16) -> bool {
        self.cv.v.state_list(u, state).is_some()
    }
    /// `use.md` §6: a list with flags 2, expire `e`, the unit as owner,
    /// state 121, remove callback `0x0056E900`; attached; state 121 on.
    ///
    /// TODO(use.md §6, stat-lists.md §8.1): the attach `reset` argument is
    /// not stated; reset = 1 as the action wiring's state lists.
    fn create_delay_list(&mut self, u: UnitId, expire: i32) {
        let Some((ty, guid)) = self.cv.v.units.get(u).map(|r| (r.ty, r.guid)) else {
            return;
        };
        let v = &mut self.cv.v;
        let l = v
            .stats
            .alloc(DELAY_LIST_FLAGS, expire, ty.index() as u32, guid);
        v.stats.set_expire(l, expire);
        v.stats.set_state(l, u32::from(STATE_SKILL_DELAY));
        v.stats.set_remove_callback(l, Some(DELAY_REMOVE_CALLBACK));
        v.stats.attach(&mut *v.h, u, l, true);
        v.set_state(u, STATE_SKILL_DELAY, true);
    }
    fn set_state_list_expiry(&mut self, u: UnitId, state: u16, expire: i32) {
        if let Some(l) = self.cv.v.state_list(u, state) {
            self.cv.v.stats.set_expire(l, expire);
        }
    }
    /// The aura state's list freed (`stat-lists.md` §8) and the state off
    /// (§9.2).
    fn free_aura_state(&mut self, u: UnitId, state: u16) {
        let v = &mut self.cv.v;
        v.stats.free_state_list(&mut *v.h, u, u32::from(state));
        v.set_state(u, state, false);
    }
    fn set_aura_state(&mut self, u: UnitId, state: u16, skill: i32, lvl: i32) {
        self.xm().set_aura_state(u, state, skill, lvl);
    }
}

// ---- BodyWorld: the skill bodies on the wired units ----------------------

impl<X: Pending + UseRest> UseView<'_, X> {
    fn body_tables(&self) -> Option<&bodies::BodyTables> {
        self.cv.v.h.bodies.as_deref()
    }
}

impl<X: Pending + UseRest> KickItems for UseView<'_, X> {
    fn toggle_weapon_lists(&mut self, u: UnitId, on: bool) {
        Pending::toggle_weapon_lists(self.xm(), u, on);
    }
    fn boots_damage(&self, item: UnitId) -> (i32, i32) {
        Pending::boots_damage(self.x(), item)
    }
}

impl<'a, X: Pending + UseRest> BodyWorld for UseView<'a, X> {
    type List = ListId;
    type Room = RoomId;
    type Combat = CombatView<'a, X>;

    fn combat(&mut self) -> &mut CombatView<'a, X> {
        &mut self.cv
    }

    fn stat_info(&self, s: i32) -> Option<bodies::BodyStat> {
        self.body_tables()?.stat(s)
    }
    fn state_count(&self) -> i32 {
        i32::try_from(self.cv.v.stats.data().states.count()).unwrap_or(i32::MAX)
    }
    fn state_flag(&self, s: i32, g: usize) -> bool {
        u32::try_from(s).is_ok_and(|s| self.cv.v.stats.data().states.has_flag(s, g))
    }
    fn state_group(&self, s: i32) -> i32 {
        let i = usize::try_from(s).ok();
        self.body_tables()
            .and_then(|b| b.state_group.get(i?).copied())
            .unwrap_or(0)
    }
    fn state_is_aura(&self, s: i32) -> bool {
        let i = usize::try_from(s).ok();
        self.body_tables()
            .and_then(|b| b.state_aura.get(i?).copied())
            .unwrap_or(false)
    }
    fn overlay_count(&self) -> i32 {
        self.body_tables().map_or(0, |b| b.overlay_count)
    }

    fn has_group(&self, u: UnitId, g: usize) -> bool {
        self.cv.v.stats.has_group(u, g)
    }
    /// `0x00639DB0`: the toggle (`stat-lists.md` §9.2) and the unit
    /// queued for update.
    fn state_on(&mut self, u: UnitId, s: i32, on: bool) {
        let Ok(s) = u16::try_from(s) else { return };
        self.cv.v.set_state(u, s, on);
        BodyWorld::queue_update(self, u);
    }
    fn mark_state_changed(&mut self, u: UnitId, s: i32) {
        if let Ok(s) = u32::try_from(s) {
            self.cv.v.stats.set_state_changed(u, s, true);
        }
    }
    fn clear_group_states(&mut self, u: UnitId, g: usize) {
        for s in 0..self.state_count() {
            let s32 = s as u32;
            if self.cv.v.stats.data().states.has_flag(s32, g) && self.cv.v.stats.has_state(u, s32) {
                self.cv.v.stats.set_state_changed(u, s32, true);
                self.cv.v.set_state(u, s as u16, false);
            }
        }
        BodyWorld::queue_update(self, u);
    }
    /// `0x0064C040` (`unit-order.md` §6.2).
    fn queue_update(&mut self, u: UnitId) {
        if self.cv.game.lists.queue_update(u).is_err() {
            self.error(WiringError::Unit(
                crate::units::modes::UnitError::UnknownUnit(u),
            ));
        }
    }
    /// The stat host's `0x0063A4A0` (`stat-lists.md` §8.8).
    fn stays_on_death(&self, u: UnitId, s: i32) -> bool {
        use crate::stats::lists::StatHost;
        let v = &self.cv.v;
        v.h.stays_on_death(v.stats, u, s as u32)
    }

    fn state_list(&self, u: UnitId, s: i32) -> Option<ListId> {
        self.cv.v.state_list(u, u16::try_from(s).ok()?)
    }
    /// The unit's list by flags (`0x006256E0` on the unit's list).
    fn first_list_with_flags(&self, u: UnitId, flags: u32) -> Option<ListId> {
        let st = &self.cv.v.stats;
        st.list_by_flags(st.unit_list(u)?, flags)
    }
    fn alloc_list(&mut self, flags: u32, expire: i32, owner: Option<UnitId>) -> Option<ListId> {
        let (ty, guid) = match owner {
            Some(o) => {
                let r = self.cv.v.units.get(o)?;
                (r.ty.index() as u32, r.guid)
            }
            None => (6, u32::MAX),
        };
        Some(self.cv.v.stats.alloc(flags, expire, ty, guid))
    }
    fn list_state(&self, l: ListId) -> i32 {
        self.cv.v.stats.state(l) as i32
    }
    fn set_list_state(&mut self, l: ListId, s: i32) {
        self.cv.v.stats.set_state(l, s as u32);
    }
    fn list_skill(&self, l: ListId) -> (i32, i32) {
        let (s, v) = self.cv.v.stats.skill(l);
        (s as i32, v as i32)
    }
    fn set_list_skill(&mut self, l: ListId, skill: i32, lvl: i32) {
        self.cv.v.stats.set_skill(l, skill as u32, lvl as u32);
    }
    fn list_expire(&self, l: ListId) -> i32 {
        self.cv.v.stats.expire(l)
    }
    fn set_list_expire(&mut self, l: ListId, e: i32) {
        self.cv.v.stats.set_expire(l, e);
    }
    fn list_get(&self, l: ListId, s: i32) -> i32 {
        u16::try_from(s).map_or(0, |s| self.cv.v.stats.base(l, s, 0))
    }
    fn list_set(&mut self, l: ListId, s: i32, v: i32) {
        if let Ok(s) = u16::try_from(s) {
            self.cv.v.set_list_stat(l, s, v);
        }
    }
    fn attach(&mut self, u: UnitId, l: ListId) {
        let v = &mut self.cv.v;
        v.stats.attach(&mut *v.h, u, l, true);
    }
    fn set_remove_callback(&mut self, l: ListId, cb: u32) {
        self.cv
            .v
            .stats
            .set_remove_callback(l, Some(RemoveCallback(cb)));
    }
    /// Detach (`stat-lists.md` §8.2), the list's remove callback as the
    /// bodies give it ([`bodies::remove_callback`]; the stat host runs
    /// none), then free (§8.3).
    fn detach_free(&mut self, u: UnitId, l: ListId) {
        let st = self.cv.v.stats.state(l) as i32;
        let cb = self.cv.v.stats.remove_callback(l);
        {
            let v = &mut self.cv.v;
            v.stats.detach(&mut *v.h, l);
        }
        if let Some(cb) = cb {
            let t = self.cv.v.h.tables.clone();
            bodies::remove_callback(self, &t.skills, &t.combat, u, st, cb.0, l);
        }
        let v = &mut self.cv.v;
        v.stats.free_plain(&mut *v.h, l);
    }

    fn add_handler(&mut self, u: UnitId, h: bodies::Handler) {
        self.cv.v.h.handlers.entry(u).or_default().insert(0, h);
    }
    /// TODO(bodies.md OQ6): the deferred free of a running record (flags
    /// bit 0 → flags |= 2) needs the handler iteration, which has no
    /// provider; records are unlinked at once.
    fn remove_handlers(&mut self, u: UnitId, key_type: i32, key: i32) {
        if let Some(v) = self.cv.v.h.handlers.get_mut(&u) {
            v.retain(|h| !(h.key_type == key_type && h.key == key));
        }
    }

    /// The adjacency array (`drlg/rooms.md` §6) of the source's room, or
    /// of the room containing `at` (`0x00463740`); each room's town test
    /// and unit list.
    fn scan_rooms(
        &self,
        source: UnitId,
        at: Option<(i32, i32)>,
    ) -> Option<Vec<bodies::ScanRoom<UnitId>>> {
        let game = &*self.cv.game;
        let drlg = &self.cv.v.h.drlg;
        let mut room = game.lists.unit(source)?.room()?;
        if let Some((x, y)) = at {
            room = drlg.find_room(game, room, x, y)?;
        }
        let adjacent = game.lists.room(room)?.adjacent.clone();
        Some(
            adjacent
                .into_iter()
                .map(|r| bodies::ScanRoom {
                    town: drlg.in_town(game, r),
                    units: game.lists.room_units(r),
                })
                .collect(),
        )
    }
    fn allied(&self, a: UnitId, b: UnitId) -> bool {
        self.x().allied(a, b)
    }
    /// The missile store's owner (`0x00552FD0`).
    fn missile_owner(&self, u: UnitId) -> Option<UnitId> {
        let o = self.cv.v.h.missiles.as_ref()?.get(u)?.owner?;
        self.cv.game.lists.find_unit(o.ty, o.guid)
    }
    fn minion_owner(&self, u: UnitId) -> Option<UnitId> {
        self.x().minion_owner(u)
    }
    fn pet_unsummonable(&self, u: UnitId, pet: UnitId) -> bool {
        self.x().pet_unsummonable(u, pet)
    }

    fn frame_bonus(&self, u: UnitId) -> i32 {
        self.x().frame_bonus(u)
    }
    fn set_anim_frame(&mut self, u: UnitId, v: i32) {
        if let Some(r) = self.cv.v.units.get_mut(u) {
            r.anim.frame = v;
        }
    }
    fn set_entry_param(&mut self, u: UnitId, i: u8, v: i32) {
        self.xm().set_entry_param(u, i, v);
    }
    fn stat_max(&self, u: UnitId, s: u16) -> i32 {
        let st = &self.cv.v.stats;
        match s {
            6 => st.max_life(u),
            8 => st.max_mana(u),
            _ => st.max_stamina(u),
        }
    }

    fn composit_weapon_class(&self, u: UnitId) -> i32 {
        self.x().composit_weapon_class(u)
    }
    fn hand_class(&self, u: UnitId) -> i32 {
        self.x().hand_class(u)
    }
    fn item_shoots(&self, item: UnitId) -> bool {
        self.x().item_shoots(item)
    }
    fn item_stackable(&self, item: UnitId) -> bool {
        self.x().item_stackable(item)
    }
    fn item_stat_of(&self, item: UnitId, s: u16) -> i32 {
        self.cv.v.stats.unit_total(item, s, 0)
    }
    fn set_item_stat(&mut self, item: UnitId, s: u16, v: i32) {
        self.cv.v.set_base(item, s, v);
    }
    fn item_max_stack(&self, item: UnitId) -> i32 {
        self.x().item_max_stack(item)
    }
    /// [`Pending::item_max_durability`]; unknown → the current
    /// durability (nothing is changed).
    fn item_max_durability(&self, item: UnitId) -> i32 {
        self.x()
            .item_max_durability(item)
            .unwrap_or_else(|| self.cv.v.stats.unit_total(item, 72, 0))
    }
    fn quantity_timer(&mut self, item: UnitId) {
        let game = &mut *self.cv.game;
        self.cv.v.h.x.quantity_timer(game, item);
    }
    /// S→C 0x3E (`0x0053D130(client, item, 1, s, v, 0)`) to the unit's
    /// client through the transport seam ([`Pending::send`]).
    /// Field widths settled by `client/msg-stats-items.md`
    /// §5 r1.3, see `units::messages::update_item_stat`.
    fn send_item_stat(&mut self, u: UnitId, item: UnitId, s: u16, v: i32) {
        let guid = self.cv.v.units.get(item).map_or(0, |r| r.guid);
        let msg = crate::units::messages::update_item_stat(guid, s, v, 0);
        Pending::send(self.xm(), u, &msg);
    }
    fn attack_cleanup(&mut self, u: UnitId) {
        self.xm().attack_cleanup(u);
    }
    fn weapon_cleanup(&mut self, u: UnitId) {
        self.xm().weapon_cleanup(u);
    }

    /// `0x0059FA30` (`missiles.md` §R2.3) on the real missile store.
    fn spawn_missile(&mut self, req: bodies::MissileRequest<UnitId>) -> Option<UnitId> {
        let p = MissileParams {
            flags: req.flags,
            owner: Some(req.owner),
            origin: req.origin,
            target: req.target,
            class: req.class,
            x: req.x,
            y: req.y,
            target_x: req.target_x,
            target_y: req.target_y,
            velocity: req.velocity,
            skill: req.skill,
            level: req.level,
            loops: req.loops,
            activate: req.activate,
            attack_bonus: req.attack_bonus,
            range: req.range,
            init: req.init,
            ..MissileParams::default()
        };
        let Some(mut store) = self.cv.v.h.missiles.take() else {
            self.error(WiringError::Reentrant("missiles"));
            return None;
        };
        let t = self.cv.v.h.tables.clone();
        let made = {
            let mut cx = missiles::Ctx {
                tables: &t.missiles,
                store: &mut store,
                world: &mut self.cv.v,
            };
            missiles::create_missile(self.cv.game, &mut cx, &p)
        };
        self.cv.v.h.missiles = Some(store);
        made
    }
    /// `0x00646F20`: every passive skill of the unit whose state is on
    /// ([`bodies::passive`]), then the host's own hook.
    fn passive_refresh(&mut self, u: UnitId) {
        let t = self.cv.v.h.tables.clone();
        for e in self.skill_list(u) {
            let p = t
                .skills
                .skill(e.skill)
                .map_or(-1, |r| i32::from(r.passivestate as i16));
            if p > 0 && SkillUnits::has_state(self, u, p as u16) {
                bodies::passive::refresh(self, &t.skills, u, e.skill);
            }
        }
        self.xm().passive_refresh(u);
    }
    fn buff_refresh(&mut self, u: UnitId) {
        self.xm().buff_refresh(u);
    }
    fn skill_resync(&mut self, u: UnitId) {
        self.resync_pet_maxima(u);
        self.xm().skill_resync(u);
    }
    /// `0x00646D60`: the passive state's stat list ([`bodies::passive`]),
    /// then the host's own hook.
    fn passive_state_apply(&mut self, u: UnitId, e: &SkillEntry) {
        let t = self.cv.v.h.tables.clone();
        bodies::passive::refresh(self, &t.skills, u, e.skill);
        self.xm().passive_state_apply(u, e);
    }
    fn set_ai_state(&mut self, u: UnitId, k: i32) {
        self.xm().set_ai_state(u, k);
    }
    fn blood_mana(&mut self, u: UnitId, cost: i32) {
        self.xm().blood_mana(u, cost);
    }
    /// `0x00571AA0`: an 0xA3 record {n, k, lvl, unit, T, r, 0} on `u`
    /// (`bodies.md` §2.14 step 5; x = the roll, y = 0), the unit queued for
    /// update (`intents-events.md` §7.9 rule 2).
    fn queue_progressive(&mut self, u: UnitId, msg: bodies::ProgressiveMsg<UnitId>) {
        use crate::wiring::action::event_records::EventRecord;
        let units = &self.cv.v.units;
        let of = |id: UnitId| {
            units
                .get(id)
                .map_or((0, u32::MAX), |r| (r.ty.index() as u8, r.guid))
        };
        let r = EventRecord::Progressive {
            charges: msg.charges,
            skill: msg.skill,
            level: msg.level,
            unit: of(msg.unit),
            target: of(msg.target),
            x: msg.roll,
            y: 0,
        };
        self.cv.v.h.event_records.push(u, r);
        let _ = self.cv.game.lists.queue_update(u);
    }
    // ---- batch 2 and 3

    /// [`Pending::body_effect`]; the d2rs fault
    /// [`bodies::BodyEffect::EndlessProgressive`] is a
    /// [`WiringError::EndlessProgressive`] instead.
    fn effect(&mut self, e: bodies::BodyEffect<UnitId, UnitId, RoomId>) {
        if let bodies::BodyEffect::EndlessProgressive { unit, skill, step } = e {
            self.error(WiringError::EndlessProgressive { unit, skill, step });
            return;
        }
        // Find Item `0x005A8000` (`treasure.md` §3.6) on the game's drop
        // state ([`super::super::action::ActionHooks::object_drops`]); a
        // game without it drops nothing. The quality value is ignored.
        if let bodies::BodyEffect::TreasureDrop { corpse, killer, .. } = e {
            let cv = &mut self.cv;
            if let Some(mut d) = cv.v.h.object_drops.take() {
                let mut sim = crate::units::hooks::Sim {
                    game: &mut *cv.game,
                    units: &mut *cv.v.units,
                    stats: &mut *cv.v.stats,
                    data: cv.v.data,
                };
                crate::wiring::economy::find_item_drop(
                    &mut *cv.v.h,
                    &mut sim,
                    &mut d,
                    &mut crate::wiring::economy::StartSpot,
                    corpse,
                    killer,
                );
                cv.v.h.object_drops = Some(d);
            }
            return;
        }
        // Summon equipment `0x005D6B60` (`bodies.md` §6.5 step 9) on the
        // game's drop state (the item tables); a game without it makes
        // no equipment.
        if let bodies::BodyEffect::Equipment {
            owner,
            m,
            lvl,
            ilvl,
            ..
        } = e
        {
            let cv = &mut self.cv;
            if let Some(mut d) = cv.v.h.object_drops.take() {
                let mut sim = crate::units::hooks::Sim {
                    game: &mut *cv.game,
                    units: &mut *cv.v.units,
                    stats: &mut *cv.v.stats,
                    data: cv.v.data,
                };
                crate::wiring::economy::summon_equipment(
                    &mut *cv.v.h,
                    &mut sim,
                    &mut d,
                    owner,
                    m,
                    lvl,
                    ilvl,
                );
                cv.v.h.object_drops = Some(d);
            }
            return;
        }
        // `0x00571B70` (`bodies-2.md` §2.13): the 0xA5 record on the unit,
        // the unit queued for update (`intents-events.md` §7.9 rule 2).
        if let bodies::BodyEffect::MsgA5 { u, skill } = e {
            use crate::wiring::action::event_records::EventRecord;
            let r = EventRecord::Landing {
                skill: skill as u16,
            };
            self.cv.v.h.event_records.push(u, r);
            let _ = self.cv.game.lists.queue_update(u);
        }
        if let Some(e) = self.pet_effect(e) {
            self.xm().body_effect(e);
        }
    }
    fn path_op(&mut self, u: UnitId, op: bodies::PathOp<UnitId>) -> i32 {
        let cv = &mut self.cv;
        match body_path::path_op(&mut cv.v, &mut *cv.game, u, op) {
            Some(r) => {
                // The host also hears of a retarget, so the unit's kept
                // target follows its path target (Double Swing's second
                // swing, q-barb).
                if matches!(op, bodies::PathOp::TargetUnit(_)) {
                    self.xm().body_path_op(u, op);
                }
                r
            }
            None => self.xm().body_path_op(u, op),
        }
    }
    fn monlvl(&self) -> &[d2_data::tables::Monlvl] {
        self.body_tables().map_or(&[], |b| &b.monlvl)
    }
    fn pettype_count(&self) -> i32 {
        self.body_tables().map_or(0, |b| b.pettype_count)
    }
    fn l_flag(&self) -> bool {
        self.x().l_flag()
    }
    /// Unit +0xC8.
    fn unit_c8(&self, u: UnitId) -> u32 {
        self.cv.v.units.get(u).map_or(0, |r| r.flags2)
    }
    fn set_unit_c8(&mut self, u: UnitId, v: u32) {
        if let Some(r) = self.cv.v.units.get_mut(u) {
            r.flags2 = v;
        }
    }
    /// Unit +0x44.
    fn anim_frame(&self, u: UnitId) -> i32 {
        self.cv.v.units.get(u).map_or(0, |r| r.anim.frame)
    }
    fn frame_event_index(&self, u: UnitId) -> i32 {
        self.x().frame_event_index(u)
    }
    fn set_frame_event_index(&mut self, u: UnitId, i: i32) {
        self.xm().set_frame_event_index(u, i);
    }
    /// Unit +0x48.
    fn frame_count(&self, u: UnitId) -> i32 {
        self.cv.v.units.get(u).map_or(0, |r| r.anim.frame_count)
    }
    fn set_frame_count(&mut self, u: UnitId, v: i32) {
        if let Some(r) = self.cv.v.units.get_mut(u) {
            r.anim.frame_count = v;
        }
    }
    /// Unit +0xD0.
    fn node_slot(&self, u: UnitId) -> i32 {
        self.cv
            .v
            .units
            .get(u)
            .map_or(crate::units::record::INITIAL_NODE_INDEX as i32, |r| {
                r.node_index as i32
            })
    }
    /// Unit +0x5C.
    fn has_stat_holder(&self, u: UnitId) -> bool {
        self.cv.v.units.get(u).is_some_and(|r| r.stats.is_some())
    }
    /// [`Pending::size`] (`0x00620510`).
    fn unit_size(&self, u: UnitId) -> i32 {
        Pending::size(self.x(), u)
    }
    fn minion_spawn_class(&self, u: UnitId) -> Option<i32> {
        self.x().minion_spawn_class(u)
    }
    fn linked_unit(&self, u: UnitId) -> Option<UnitId> {
        self.x().linked_unit(u)
    }
    fn killer_of(&self, u: UnitId) -> Option<UnitId> {
        self.x().killer_of(u)
    }
    fn minion_owner_ident(&self, u: UnitId) -> Option<(u32, u32)> {
        self.x().minion_owner_ident(u)
    }
    /// `0x006272B0`: the base value plus v.
    fn add_stat(&mut self, u: UnitId, s: u16, v: i32) {
        let b = SkillUnits::base_stat(&self.cv, u, s, 0);
        self.cv.v.set_base(u, s, b.wrapping_add(v));
    }
    fn list_add(&mut self, l: ListId, s: i32, v: i32) {
        if let Ok(s) = u16::try_from(s) {
            let v2 = &mut self.cv.v;
            v2.stats.add(&mut *v2.h, l, s, v, 0);
        }
    }
    fn list_clear(&mut self, l: ListId) {
        let v = &mut self.cv.v;
        v.stats.remove_all(&mut *v.h, l);
    }
    fn has_handler(&self, u: UnitId, key_type: i32, key: i32, skill: i32) -> bool {
        self.cv.v.h.handlers.get(&u).is_some_and(|v| {
            v.iter()
                .any(|h| h.key_type == key_type && h.key == key && h.skill == skill)
        })
    }
    fn entry_param(&self, u: UnitId, e: &SkillEntry, i: u8) -> i32 {
        match self.list_entry(u, e).zip(i.checked_sub(1)) {
            Some((l, k)) => l.params.get(usize::from(k)).copied().unwrap_or(0),
            None => Pending::entry_param(self.x(), u, e, i),
        }
    }
    fn set_entry_param_of(&mut self, u: UnitId, e: &SkillEntry, i: u8, v: i32) {
        match self.list_entry_mut(u, e) {
            Some(l) => {
                if let Some(p) = i
                    .checked_sub(1)
                    .and_then(|k| l.params.get_mut(usize::from(k)))
                {
                    *p = v;
                }
            }
            None => self.xm().set_entry_param_of(u, e, i, v),
        }
    }
    fn entry_flags(&self, u: UnitId, e: &SkillEntry) -> u32 {
        match self.list_entry(u, e) {
            Some(l) => l.flags,
            None => self.x().entry_flags(u, e),
        }
    }
    fn set_entry_flags(&mut self, u: UnitId, e: &SkillEntry, f: u32) {
        match self.list_entry_mut(u, e) {
            Some(l) => l.flags = f,
            None => self.xm().set_entry_flags(u, e, f),
        }
    }
    fn set_entry_mode(&mut self, u: UnitId, e: &SkillEntry, m: u32) {
        self.xm().set_entry_mode(u, e, m);
    }
    fn disguise_mode(&self, u: UnitId, m: u32) -> u32 {
        self.x().disguise_mode(u, m)
    }
    fn skill_sequence(&self, u: UnitId) -> Option<Vec<[u8; 6]>> {
        self.x().skill_sequence(u)
    }
    /// `0x0056E210` → `0x00553B10` (`sim/units.md` §4.2 variants).
    fn anim_rewind(&mut self, u: UnitId, p: i32) {
        self.anim_variant(u, Form::Percent(p));
    }
    /// `0x00553C70` (`sim/units.md` §4.2 variants).
    fn anim_restart(&mut self, u: UnitId, v: i32) {
        self.anim_variant(u, Form::Frames(v));
    }
    /// `0x00553DC0` (`sim/units.md` §4.2 variants): Leap's and Leap
    /// Attack's frame-10 rewind, Whirlwind's frame 3.
    fn anim_from(&mut self, u: UnitId, f: i32) {
        self.anim_variant(u, Form::StartFrame(f));
    }
    /// `0x00620BB0`.
    fn unit_room(&self, u: UnitId) -> Option<RoomId> {
        self.cv.game.lists.unit(u)?.room()
    }
    /// `0x00463740`.
    fn room_at(&self, from: RoomId, x: i32, y: i32) -> Option<RoomId> {
        self.cv.v.h.drlg.find_room(&*self.cv.game, from, x, y)
    }
    /// `0x0061AB00`.
    fn room_in_town(&self, r: RoomId) -> bool {
        self.cv.v.h.drlg.in_town(&*self.cv.game, r)
    }
    fn room_act(&self, r: RoomId) -> i32 {
        self.x().room_act(r)
    }
    fn room_teleport(&self, r: RoomId) -> Option<i32> {
        self.level_teleport(r).or_else(|| self.x().room_teleport(r))
    }
    fn free_point(
        &mut self,
        r: RoomId,
        at: (i32, i32),
        size: i32,
        mask: u32,
        fallback: bool,
    ) -> Option<(RoomId, (i32, i32))> {
        match self.rooms_free_point(r, at, size, mask, fallback) {
            Some(found) => found,
            None => self.xm().free_point(r, at, size, mask, fallback),
        }
    }
    fn pattern_collides(&self, r: RoomId, at: (i32, i32), u: UnitId, mask: u32) -> bool {
        self.rooms_pattern_collides(r, at, u, mask)
            .unwrap_or_else(|| self.x().pattern_collides(r, at, u, mask))
    }
    fn box_collides(&self, r: RoomId, at: (i32, i32), size: i32, mask: u32) -> bool {
        self.rooms_box_collides(r, at, size, mask)
            .unwrap_or_else(|| self.x().box_collides(r, at, size, mask))
    }
    /// `0x0064E260` (`pathing.md` §13.3, [`crate::path::line::line_test`])
    /// on the DRLG rooms with the path provider; else [`Pending`].
    fn line_blocked(&self, r: RoomId, from: (i32, i32), to: (i32, i32), mask: u32) -> bool {
        if self.cv.v.h.paths.is_some() {
            use crate::path::Point;
            return crate::path::line::line_test(
                &self.cv.v.h.drlg,
                Some(r),
                Point::new(from.0, from.1),
                Point::new(to.0, to.1),
                mask as u16,
            )
            .blocked();
        }
        self.x().body_line_blocked(r, from, to, mask)
    }
    fn place_unit(&mut self, u: UnitId, r: Option<RoomId>, at: (i32, i32)) -> bool {
        match self.rooms_place_unit(u, r, at) {
            Some(placed) => placed,
            None => self.xm().place_unit(u, r, at),
        }
    }
    fn has_path(&self, u: UnitId) -> bool {
        self.cv.v.h.path_has(u)
    }
    /// `0x006487D0`: the path record's point count, on the path provider.
    fn path_point_count(&self, u: UnitId) -> i32 {
        match self.dynamic_path(u) {
            Some(d) => d.point_count as i32,
            None => self.x().path_point_count(u),
        }
    }
    fn path_last_point(&self, u: UnitId) -> (i32, i32) {
        match self.dynamic_path(u) {
            Some(d) => d.live_points().last().map_or((0, 0), |p| (p.x, p.y)),
            None => self.x().path_last_point(u),
        }
    }
    fn path_target_point(&self, u: UnitId) -> (i32, i32) {
        self.cast_target_point(u)
    }
    fn create_monster(
        &mut self,
        r: RoomId,
        at: (i32, i32),
        class: i32,
        mode: i32,
        spread: i32,
    ) -> Option<UnitId> {
        match self.xm().create_monster(r, at, class, mode, spread) {
            Some(m) => Some(m),
            None => self.alloc_monster(r, at, class, mode),
        }
    }
    fn mode_request(&mut self, m: UnitId, mode: i32, target: Option<UnitId>) -> i32 {
        let r = Pending::mode_request(self.xm(), m, mode, target);
        // d2rs-own, unverified (q-skill-gaps, REC-176): without a host
        // answer the request is the monster mode set `0x005A7E60` +
        // `0x005A7C20` (`units.md` §4.6) itself, so a revived corpse
        // stands up.
        let monster = self
            .cv
            .v
            .units
            .get(m)
            .is_some_and(|r| r.ty == UnitType::Monster);
        match u32::try_from(mode) {
            Ok(mode) if r == 0 && monster => {
                if let Some(t) = target {
                    let t = crate::monsters::ai::ModeTarget::Unit(t);
                    self.cv.v.h.x.set_mode_target(m, t);
                }
                i32::from(self.cv.v.monster_set_mode(&mut *self.cv.game, m, mode))
            }
            _ => r,
        }
    }
    /// An item unit is its own item handle here.
    fn as_item(&self, u: UnitId) -> Option<UnitId> {
        (self.cv.v.units.get(u)?.ty == UnitType::Item).then_some(u)
    }
    fn inventory_busy(&self, u: UnitId) -> bool {
        self.x().inventory_busy(u)
    }
    fn has_inventory(&self, u: UnitId) -> bool {
        self.x().has_inventory(u)
    }
    fn weapon_in_use(&self, u: UnitId) -> Option<UnitId> {
        self.x().weapon_in_use(u)
    }
    fn body_loc(&self, i: UnitId) -> i32 {
        self.x().body_loc(i)
    }
    fn item_usable(&self, i: UnitId) -> bool {
        self.x().item_usable(i)
    }
    fn item_active(&self, i: UnitId) -> bool {
        self.x().item_active(i)
    }
    fn item_breakable(&self, i: UnitId) -> bool {
        self.x().item_breakable(i)
    }
    fn shield(&self, u: UnitId) -> Option<UnitId> {
        Pending::shield(self.x(), u)
    }
    fn shield_damage(&self, i: UnitId) -> Option<(i32, i32)> {
        self.x().shield_damage(i)
    }
    fn item_missile_type(&self, i: UnitId) -> i32 {
        self.x().item_missile_type(i)
    }
    fn golem_item(&self, t: UnitId) -> bool {
        self.x().golem_item(t)
    }
    fn item_first_loc(&self, t: UnitId) -> i32 {
        self.x().item_first_loc(t)
    }
    fn two_melee_weapons(&self, u: UnitId) -> bool {
        self.x().two_melee_weapons(u)
    }
    fn attack_frames(&self, u: UnitId, w: UnitId) -> Option<i32> {
        self.x().attack_frames(u, w)
    }
    // ---- batch 4

    fn dir64(&self, u: UnitId, at: (i32, i32)) -> i32 {
        self.x().body_dir64(u, at)
    }
    /// Unit +0x4E.
    fn action_frame(&self, u: UnitId) -> i32 {
        self.cv
            .v
            .units
            .get(u)
            .map_or(0, |r| i32::from(r.anim.action_frame))
    }
    fn set_action_frame(&mut self, u: UnitId, v: i32) {
        if let Some(r) = self.cv.v.units.get_mut(u) {
            r.anim.action_frame = v as u8;
        }
    }
    /// Unit +0x3C (the sequence's speed; no sequence: nothing kept).
    fn set_seq_speed(&mut self, u: UnitId, v: i32) {
        if let Some(q) = self
            .cv
            .v
            .units
            .get_mut(u)
            .and_then(|r| r.anim.sequence.as_mut())
        {
            q.speed = v;
        }
    }
    /// Unit +0x4C (i16).
    fn anim_speed(&self, u: UnitId) -> i32 {
        self.cv
            .v
            .units
            .get(u)
            .map_or(0, |r| i32::from(r.anim.speed))
    }
    fn set_anim_speed(&mut self, u: UnitId, v: i32) {
        if let Some(r) = self.cv.v.units.get_mut(u) {
            r.anim.speed = v as i16;
        }
    }
    fn missile_frames(&self, m: UnitId) -> i32 {
        self.x().body_missile_frames(m)
    }
    fn set_missile_frames(&mut self, m: UnitId, total: i32, left: i32) {
        self.xm().body_set_missile_frames(m, total, left);
    }
    /// Unit +0x30 → +0x34.
    fn sequence_frames(&self, u: UnitId) -> Option<i32> {
        let r = self.cv.v.units.get(u)?;
        r.anim.sequence.as_ref().map(|q| q.frame_count)
    }
    /// The sequence's event byte of frame `f >> 8`.
    fn sequence_event(&self, u: UnitId, f: i32) -> i32 {
        let Some(q) = self
            .cv
            .v
            .units
            .get(u)
            .and_then(|r| r.anim.sequence.as_ref())
        else {
            return 0;
        };
        usize::try_from(f >> 8)
            .ok()
            .and_then(|i| q.events.get(i))
            .map_or(0, |&e| i32::from(e))
    }
    /// Unit +0x50: the AnimData record.
    fn anim_data(&self, u: UnitId) -> Option<(u32, Vec<u8>)> {
        let a = self.cv.v.units.get(u)?.anim.record.as_ref()?;
        Some((a.frames, a.events.to_vec()))
    }
    fn action_event_between(&self, u: UnitId, a: i32, b: i32) -> bool {
        self.x().body_action_event_between(u, a, b)
    }
    /// [`Pending::ai_chain_index`] (`0x006510C0`).
    fn chain_position(&self, class: i32) -> i32 {
        self.x().ai_chain_index(class)
    }
    /// [`Pending::ai_class_for_level`] (`0x0063EC70`).
    fn class_for_level(&self, room: Option<RoomId>, class: i32) -> i32 {
        self.x().ai_class_for_level(self.cv.game, room, class)
    }
    fn book_skills(&self, i: UnitId) -> Option<(i32, i32)> {
        self.x().body_book_skills(i)
    }
    fn inventory_nodes(&self, u: UnitId) -> Vec<(UnitId, i32)> {
        self.x().body_inventory_nodes(u)
    }
    fn unit_find(&self, room: RoomId, at: (i32, i32), r: i32, f: u32) -> Vec<UnitId> {
        self.x().body_unit_find(room, at, r, f)
    }
    fn point_collides(&self, room: RoomId, at: (i32, i32), mask: u32) -> bool {
        self.rooms_point_collides(room, at, mask)
            .unwrap_or_else(|| self.x().body_point_collides(room, at, mask))
    }
    fn spawn_monster(&mut self, q: bodies::MonsterSpawn<UnitId, RoomId>) -> Option<UnitId> {
        self.xm().body_spawn_monster(q)
    }
    /// a = 0: [`BodyWorld::place_unit`]'s provider.
    fn place_unit_flag(&mut self, u: UnitId, r: Option<RoomId>, at: (i32, i32), a: i32) -> bool {
        if a == 0 {
            BodyWorld::place_unit(self, u, r, at)
        } else {
            self.xm().body_place_unit_flag(u, r, at, a)
        }
    }
    /// [`Pending::ai_component`].
    fn component(&self, u: UnitId, k: usize) -> i32 {
        i32::from(self.x().ai_component(u, k))
    }
}

// ---- unit events (`combat/events.md`) ------------------------------------

/// The event functions' calls beyond [`BodyWorld`]: the handler lists of
/// [`crate::wiring::action::ActionHooks::handlers`], GUIDs and list owners
/// are real; the rest are [`Pending`]'s event seams.
impl<X: Pending + UseRest> EventWorld for UseView<'_, X> {
    /// `data/runtime-maps.md` §3 from the body tables' itemstatcost.
    fn layer_split(&self) -> (u32, u32) {
        self.cv
            .v
            .h
            .bodies
            .as_ref()
            .and_then(|b| b.layer_split)
            .unwrap_or_else(|| self.x().event_layer_split())
    }
    /// The list's owner type / GUID (+0x08 / +0x0C) resolved by
    /// `0x00552F60` (the game's unit hash).
    fn list_owner(&self, l: ListId) -> Option<UnitId> {
        let stats = &self.cv.v.stats;
        let ty = *UnitType::ALL.get(stats.owner_type(l) as usize)?;
        self.cv.game.lists.find_unit(ty, stats.owner_guid(l))
    }
    fn guid(&self, u: UnitId) -> u32 {
        self.cv.v.units.get(u).map_or(0, |r| r.guid)
    }
    fn terror(&mut self, source: UnitId, unit: UnitId, skill: i32, a: i32, b: i32) {
        let game = &mut *self.cv.game;
        self.cv.v.h.x.event_terror(game, source, unit, skill, a, b);
    }
    fn point_free(&self, u: UnitId, at: (i32, i32)) -> bool {
        self.x().event_point_free(u, at)
    }
    fn corpse_near(&mut self, t0: UnitId) -> Option<UnitId> {
        let game = &mut *self.cv.game;
        self.cv.v.h.x.event_corpse_near(game, t0)
    }
    fn queue_item_cast(&mut self, u: UnitId, msg: ItemCastMsg) {
        // `0x005717C0` / `0x00571840`: the 0x99 / 0x9A record on the unit,
        // the unit queued for update (`intents-events.md` §7.9 rule 2).
        // PROVISIONAL (REC-413): the wire level byte is the cast level
        // clamped to a byte and w (u16) is the `aim` flag; the record
        // bytes are not spelled out in the specs; settled by a 1.14d
        // recording of an item-cast skill (`events.txt` item cast).
        use crate::wiring::action::event_records::EventRecord;
        let r = match msg {
            ItemCastMsg::Unit {
                skill,
                level,
                target,
                aim,
            } => EventRecord::CastOnUnit {
                skill: skill as u16,
                level: level.clamp(0, 255) as u8,
                target: (target.0 as u8, target.1),
                w: u16::from(aim),
            },
            ItemCastMsg::Point {
                skill,
                level,
                at,
                aim,
            } => EventRecord::CastOnPoint {
                skill: skill as u32,
                level: level.clamp(0, 255) as u8,
                x: at.0 as u16,
                y: at.1 as u16,
                w: u16::from(aim),
            },
        };
        self.cv.v.h.event_records.push(u, r);
        let _ = self.cv.game.lists.queue_update(u);
        self.xm().queue_item_cast(u, msg);
    }
    fn raise_test(&self, v: UnitId) -> bool {
        self.x().raise_test(v)
    }
    fn clear_pattern(&mut self, v: UnitId) {
        self.xm().clear_pattern(v);
    }
    fn raise_step(&mut self, n: UnitId, step: RaiseStep<UnitId>) {
        let game = &mut *self.cv.game;
        self.cv.v.h.x.raise_step(game, n, step);
    }
    fn handlers_of(&self, u: UnitId) -> Vec<bodies::Handler> {
        self.cv.v.h.handlers.get(&u).cloned().unwrap_or_default()
    }
    fn remove_handler(&mut self, u: UnitId, h: &bodies::Handler) {
        if let Some(v) = self.cv.v.h.handlers.get_mut(&u) {
            if let Some(i) = v.iter().position(|x| x == h) {
                v.remove(i);
            }
        }
    }
}

/// The unit event iteration `0x005C0C30` (`skills/bodies.md` §2.18) on
/// the action wiring: [`events::run`] over [`UseView`] (a
/// [`crate::wiring::action::combat::UnitEventFn`]).
pub fn run_unit_event<X: Pending + UseRest>(
    cv: &mut CombatView<'_, X>,
    event: u8,
    unit: Option<UnitId>,
    other: Option<UnitId>,
    record: Option<&mut crate::combat::DamageRecord>,
) -> i32 {
    let t = cv.v.h.tables.clone();
    let mut w = UseView {
        cv: CombatView {
            game: &mut *cv.game,
            v: View::of(&mut *cv.v.units, &mut *cv.v.stats, cv.v.data, &mut *cv.v.h),
        },
    };
    let tb = EventTables {
        skills: &t.skills,
        combat: &t.combat,
    };
    events::run(&mut w, tb, event, unit, other, record)
}

impl<X: Pending + UseRest> crate::wiring::action::ActionHooks<X> {
    /// Turns the unit event registry on: from now on `0x005C0C30` runs
    /// [`events::run`] on [`Self::handlers`] ([`run_unit_event`]) instead
    /// of [`Pending::unit_event`] / [`Pending::level_up_event`].
    pub fn enable_unit_events(&mut self) {
        self.unit_events = Some(run_unit_event::<X>);
    }
}
