// Spec: specs/sim/stat-lists.md; specs/sim/stats.md §4, §6
//! Stat lists (D2MOO `D2StatListStrc` / `D2StatListExStrc`): records
//! (§1), flags (§2), sorted arrays (§3), allocation (§4), base writes
//! (§5), full values and their recompute (§6), the value-change
//! notification (§7), chain operations (§8), list queries (§9.3), expiry
//! (§10.4) and the mod array (§11). Readers and evaluation are
//! `stats.md` §4 and §6.
//!
//! Lists live in an arena ([`StatLists`]) addressed by generational
//! [`ListId`]s; a unit's extended list is found by its [`UnitId`].
//! Systems other specs own (items, skills, states, the environment) are
//! reached through [`StatHost`]; every host method receives the lists
//! so it can write stats in place, as the original callbacks do.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::ops::{op_row, Contribution, Guard, Operand, Prev, RecomputeBlock};
use super::{by_time, key, key_stat, muldiv, stat, x87_rescale, StatData, StatInfo, NO_STAT};
use crate::units::{UnitId, UnitType};

/// List flags (+0x10, §2; D2MOO names).
pub mod flag {
    pub const BASIC: u32 = 0x0000_0001;
    pub const NEWLENGTH: u32 = 0x0000_0002;
    pub const TEMPONLY: u32 = 0x0000_0004;
    pub const OVERLAY: u32 = 0x0000_0080;
    /// On a unit's list: remove the overlay list's stats (§8.8).
    pub const REMOVE_OVERLAY: u32 = 0x0000_0100;
    /// Parked (SET): lives in the parked chain, counts nowhere.
    pub const SET: u32 = 0x0000_2000;
    pub const PERMANENT: u32 = 0x2000_0000;
    pub const DYNAMIC: u32 = 0x4000_0000;
    pub const EXTENDED: u32 = 0x8000_0000;
}

/// Unit type numbers as stored in a list's owner-type field (+0x08).
pub mod owner {
    pub const PLAYER: u32 = 0;
    pub const MONSTER: u32 = 1;
    pub const ITEM: u32 = 4;
}

/// Stats never put in the mod array (§11.1).
const MOD_EXCLUDED: [u16; 5] = [6, 8, 10, 13, 14];

/// A list, by arena slot and generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ListId {
    index: u32,
    generation: u32,
}

/// An extended list's value-change callback (+0x5C, §7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueCallback {
    /// The server callback `0x0055B800` (players and monsters, §7.2).
    Server,
}

/// A list's remove callback (+0x38), opaque: the [`StatHost`] gives ids
/// their meaning (state code, skills spec).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RemoveCallback(pub u32);

/// One value-change notification (§7.1), as handed to
/// [`StatHost::on_callback`] before the callback body runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CallbackEvent {
    pub list: ListId,
    /// The list's owner unit (+0x44).
    pub owner: UnitId,
    /// The propagation's unit argument.
    pub unit: Option<UnitId>,
    pub key: i32,
    pub old: i32,
    pub new: i32,
}

/// What the stat code needs from systems other specs own. Each default
/// is the narrowest reading: nothing happens.
#[allow(unused_variables)]
pub trait StatHost {
    /// `stats.md` §8: the base time t of the unit's act, or `None` when
    /// the unit has no act record (guard `owner_act` fails). Provider:
    /// the environment spec (world group; `stats.md` open question 3).
    fn act_time(&self, unit: UnitId) -> Option<i32> {
        None
    }

    /// Called for every value-change callback before its body runs
    /// (trace comparison, `stat-lists.md` Test vectors 2).
    fn on_callback(&mut self, lists: &StatLists, ev: &CallbackEvent) {}

    /// §7.2 rule 1: item event registration (`0x005C0BE0`, `0x0056E740`
    /// when `new ≠ 0`, `0x005C0B50` when 0). Provider: items.
    fn item_event(&mut self, lists: &mut StatLists, owner: UnitId, stat: u16, new: i32) {}

    /// §7.2 rule 2, stats 83, 97, 98, 107, 126, 127, 151, 188, 204:
    /// skill and state handlers. Provider: skills.
    fn skill_stat_changed(
        &mut self,
        lists: &mut StatLists,
        owner: UnitId,
        key: i32,
        old: i32,
        new: i32,
    ) {
    }

    /// §8.2 rule 6: a detached list's remove callback (ECX unit, EDX
    /// state, list). Provider: state code (skills).
    fn list_removed(
        &mut self,
        lists: &mut StatLists,
        unit: UnitId,
        state: u32,
        list: ListId,
        callback: RemoveCallback,
    ) {
    }

    /// §8.8 rule 1, `0x0063A4A0`: the state stays on the unit at death.
    /// Provider: states (skills spec). TODO(stat-lists.md §8.8): the rule
    /// is not written; false until it is.
    fn stays_on_death(&self, lists: &StatLists, unit: UnitId, state: u32) -> bool {
        false
    }
}

/// A host with every default: no act time, no item or skill handlers.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoHost;

impl StatHost for NoHost {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry {
    key: i32,
    value: i32,
}

#[derive(Clone, Debug)]
struct Extended {
    /// +0x3C, +0x40: heads of the active and parked chains.
    active: Option<ListId>,
    parked: Option<ListId>,
    /// +0x44.
    owner: UnitId,
    /// The owner's class (charstats row for players, monstats row for
    /// monsters).
    class: u32,
    /// +0x48.
    full: Vec<Entry>,
    /// +0x50, sorted unique keys.
    mods: Vec<i32>,
    /// +0x58: W state words, then W "changed" words.
    states: Vec<u32>,
    /// +0x5C.
    callback: Option<ValueCallback>,
}

/// A list record (§1).
#[derive(Clone, Debug)]
struct List {
    /// +0x04.
    unit: Option<UnitId>,
    /// +0x08, +0x0C.
    owner_type: u32,
    owner_guid: u32,
    /// +0x10.
    flags: u32,
    /// +0x14.
    state: u32,
    /// +0x18.
    expire: i32,
    /// +0x1C, +0x20.
    skill: u32,
    skill_level: u32,
    /// +0x24.
    base: Vec<Entry>,
    /// +0x2C, +0x30, +0x34.
    prev: Option<ListId>,
    next: Option<ListId>,
    parent: Option<ListId>,
    /// +0x38.
    remove_callback: Option<RemoveCallback>,
    ext: Option<Extended>,
}

/// The stat lists of one game.
#[derive(Clone, Debug)]
pub struct StatLists {
    data: Arc<StatData>,
    slots: Vec<(u32, Option<List>)>,
    free: Vec<u32>,
    /// Unit +0x5C.
    units: BTreeMap<UnitId, ListId>,
}

fn find(a: &[Entry], k: i32) -> Result<usize, usize> {
    a.binary_search_by(|e| e.key.cmp(&k))
}

fn get(a: &[Entry], k: i32) -> Option<i32> {
    find(a, k).ok().map(|i| a[i].value)
}

impl StatLists {
    pub fn new(data: Arc<StatData>) -> Self {
        Self {
            data,
            slots: Vec::new(),
            free: Vec::new(),
            units: BTreeMap::new(),
        }
    }

    pub fn data(&self) -> &StatData {
        &self.data
    }

    fn info(&self, s: u16) -> Option<&StatInfo> {
        self.data.stats.get(s)
    }

    fn l(&self, id: ListId) -> &List {
        self.try_l(id).expect("live stat list")
    }

    fn try_l(&self, id: ListId) -> Option<&List> {
        match self.slots.get(id.index as usize) {
            Some((g, Some(l))) if *g == id.generation => Some(l),
            _ => None,
        }
    }

    fn lm(&mut self, id: ListId) -> &mut List {
        match self.slots.get_mut(id.index as usize) {
            Some((g, Some(l))) if *g == id.generation => l,
            _ => panic!("live stat list {id:?}"),
        }
    }

    fn ext(&self, id: ListId) -> Option<&Extended> {
        self.try_l(id)?.ext.as_ref()
    }

    fn ext_mut(&mut self, id: ListId) -> Option<&mut Extended> {
        self.lm(id).ext.as_mut()
    }

    /// [`Self::ext_mut`] that answers `None` for a freed list.
    fn try_ext_mut(&mut self, id: ListId) -> Option<&mut Extended> {
        match self.slots.get_mut(id.index as usize) {
            Some((g, Some(l))) if *g == id.generation => l.ext.as_mut(),
            _ => None,
        }
    }

    fn insert(&mut self, list: List) -> ListId {
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            slot.1 = Some(list);
            ListId {
                index,
                generation: slot.0,
            }
        } else {
            self.slots.push((0, Some(list)));
            ListId {
                index: self.slots.len() as u32 - 1,
                generation: 0,
            }
        }
    }

    // ---- §4 allocation -------------------------------------------------

    /// `0x006251F0` (§4.1): a plain list. `expire` is stored without
    /// setting NEWLENGTH.
    pub fn alloc(&mut self, flags: u32, expire: i32, owner_type: u32, owner_guid: u32) -> ListId {
        self.insert(List {
            unit: None,
            owner_type,
            owner_guid,
            flags,
            state: 0,
            expire,
            skill: 0,
            skill_level: 0,
            base: Vec::new(),
            prev: None,
            next: None,
            parent: None,
            remove_callback: None,
            ext: None,
        })
    }

    /// `0x00626D40` (§4.2): the unit's extended list. Frees the unit's
    /// current list first; flags := (flags & 1) | EXTENDED.
    #[allow(clippy::too_many_arguments)]
    pub fn alloc_extended(
        &mut self,
        host: &mut dyn StatHost,
        unit: UnitId,
        unit_type: UnitType,
        guid: u32,
        class: u32,
        flags: u32,
        callback: Option<ValueCallback>,
    ) -> ListId {
        self.free_unit_list(host, unit);
        let words = self.data.states.words();
        let id = self.insert(List {
            unit: None,
            owner_type: unit_type.index() as u32,
            owner_guid: guid,
            flags: (flags & flag::BASIC) | flag::EXTENDED,
            state: 0,
            expire: 0,
            skill: 0,
            skill_level: 0,
            base: Vec::new(),
            prev: None,
            next: None,
            parent: None,
            remove_callback: None,
            ext: Some(Extended {
                active: None,
                parked: None,
                owner: unit,
                class,
                full: Vec::new(),
                mods: Vec::new(),
                states: vec![0; 2 * words],
                callback,
            }),
        });
        self.units.insert(unit, id);
        id
    }

    /// `0x00626D10`: frees the unit's list (if extended).
    pub fn free_unit_list(&mut self, host: &mut dyn StatHost, unit: UnitId) {
        if let Some(&l) = self.units.get(&unit) {
            if self.ext(l).is_some() {
                self.free(host, l);
            }
        }
    }

    /// The unit's list (unit +0x5C).
    pub fn unit_list(&self, unit: UnitId) -> Option<ListId> {
        self.units.get(&unit).copied()
    }

    /// Whether `id` names a live list.
    pub fn is_live(&self, id: ListId) -> bool {
        self.try_l(id).is_some()
    }

    // ---- record fields -------------------------------------------------

    pub fn flags(&self, l: ListId) -> u32 {
        self.l(l).flags
    }

    /// Sets or clears flag bits the callers own (§2, open question 3).
    pub fn set_flags(&mut self, l: ListId, bits: u32, on: bool) {
        let f = &mut self.lm(l).flags;
        if on {
            *f |= bits;
        } else {
            *f &= !bits;
        }
    }

    pub fn is_extended(&self, l: ListId) -> bool {
        self.l(l).ext.is_some()
    }

    pub fn owner_type(&self, l: ListId) -> u32 {
        self.l(l).owner_type
    }

    pub fn owner_guid(&self, l: ListId) -> u32 {
        self.l(l).owner_guid
    }

    /// Extended list owner unit (+0x44).
    pub fn owner(&self, l: ListId) -> Option<UnitId> {
        self.ext(l).map(|e| e.owner)
    }

    /// Unit the list is attached to (+0x04).
    pub fn attached_unit(&self, l: ListId) -> Option<UnitId> {
        self.l(l).unit
    }

    pub fn parent(&self, l: ListId) -> Option<ListId> {
        self.l(l).parent
    }

    pub fn prev(&self, l: ListId) -> Option<ListId> {
        self.l(l).prev
    }

    pub fn next(&self, l: ListId) -> Option<ListId> {
        self.l(l).next
    }

    /// Heads of the active and parked chains of an extended list.
    pub fn heads(&self, l: ListId) -> (Option<ListId>, Option<ListId>) {
        self.ext(l).map_or((None, None), |e| (e.active, e.parked))
    }

    /// `0x006252F0`.
    pub fn state(&self, l: ListId) -> u32 {
        self.l(l).state
    }

    /// `0x006252D0`.
    pub fn set_state(&mut self, l: ListId, state: u32) {
        self.lm(l).state = state;
    }

    pub fn expire(&self, l: ListId) -> i32 {
        self.l(l).expire
    }

    /// `0x00625310` / `0x006260B0`: sets the expire frame and, when > 0,
    /// NEWLENGTH.
    pub fn set_expire(&mut self, l: ListId, frame: i32) {
        let list = self.lm(l);
        list.expire = frame;
        if frame > 0 {
            list.flags |= flag::NEWLENGTH;
        }
    }

    /// Skill id and level (+0x1C, +0x20; callers' bookkeeping).
    pub fn skill(&self, l: ListId) -> (u32, u32) {
        let list = self.l(l);
        (list.skill, list.skill_level)
    }

    pub fn set_skill(&mut self, l: ListId, skill: u32, level: u32) {
        let list = self.lm(l);
        list.skill = skill;
        list.skill_level = level;
    }

    pub fn set_remove_callback(&mut self, l: ListId, cb: Option<RemoveCallback>) {
        self.lm(l).remove_callback = cb;
    }

    /// The base array as sorted (key, value) pairs.
    pub fn base_entries(&self, l: ListId) -> Vec<(i32, i32)> {
        self.l(l).base.iter().map(|e| (e.key, e.value)).collect()
    }

    /// The full array as sorted (key, value) pairs (empty for a plain
    /// list).
    pub fn full_entries(&self, l: ListId) -> Vec<(i32, i32)> {
        self.ext(l)
            .map(|e| e.full.iter().map(|e| (e.key, e.value)).collect())
            .unwrap_or_default()
    }

    /// The mod array keys (§11).
    pub fn mods(&self, l: ListId) -> Vec<i32> {
        self.ext(l).map(|e| e.mods.clone()).unwrap_or_default()
    }

    /// The active chain from the head through prev.
    pub fn active_chain(&self, l: ListId) -> Vec<ListId> {
        self.chain(self.heads(l).0)
    }

    /// The parked chain from the head through prev.
    pub fn parked_chain(&self, l: ListId) -> Vec<ListId> {
        self.chain(self.heads(l).1)
    }

    fn chain(&self, mut cur: Option<ListId>) -> Vec<ListId> {
        let mut v = Vec::new();
        while let Some(c) = cur {
            v.push(c);
            cur = self.l(c).prev;
        }
        v
    }

    pub(super) fn state_words(&self, l: ListId) -> Option<&[u32]> {
        self.ext(l).map(|e| e.states.as_slice())
    }

    pub(super) fn state_words_mut(&mut self, l: ListId) -> Option<&mut Vec<u32>> {
        self.ext_mut(l).map(|e| &mut e.states)
    }

    /// The unit type of a unit with an extended list (its owner type).
    pub fn unit_type_raw(&self, unit: UnitId) -> Option<u32> {
        self.unit_list(unit).map(|l| self.l(l).owner_type)
    }

    // ---- stats.md §4 readers ------------------------------------------

    /// Minimum rule (`stats.md` §4.3) on a present value.
    fn min_rule(&self, l: ListId, s: u16, v: i32) -> i32 {
        let Some(info) = self.info(s) else {
            return v;
        };
        let list = self.l(l);
        if info.fmin
            && list.ext.is_some()
            && matches!(list.owner_type, owner::PLAYER | owner::MONSTER)
            && v < info.minaccr
        {
            info.minaccr << info.valshift
        } else {
            v
        }
    }

    /// Base value of a list (`0x00625350`): base array, minimum rule.
    pub fn base(&self, l: ListId, s: u16, layer: u16) -> i32 {
        if self.info(s).is_none() {
            return 0;
        }
        match get(&self.l(l).base, key(s, layer)) {
            Some(v) => self.min_rule(l, s, v),
            None => 0,
        }
    }

    /// Total value of a list (`0x00625420`): full array if extended,
    /// else base array; minimum rule.
    pub fn total(&self, l: ListId, s: u16, layer: u16) -> i32 {
        if self.info(s).is_none() {
            return 0;
        }
        match self.raw_total(l, key(s, layer)) {
            Some(v) => self.min_rule(l, s, v),
            None => 0,
        }
    }

    fn raw_total(&self, l: ListId, k: i32) -> Option<i32> {
        let list = self.l(l);
        match &list.ext {
            Some(e) => get(&e.full, k),
            None => get(&list.base, k),
        }
    }

    fn raw_base(&self, l: ListId, k: i32) -> Option<i32> {
        get(&self.l(l).base, k)
    }

    /// Unit total (`0x00625480`, `0x00625500`); a unit without a list
    /// reads 0.
    pub fn unit_total(&self, unit: UnitId, s: u16, layer: u16) -> i32 {
        self.unit_list(unit).map_or(0, |l| self.total(l, s, layer))
    }

    /// Unit base (`0x006253B0`).
    pub fn unit_base(&self, unit: UnitId, s: u16, layer: u16) -> i32 {
        self.unit_list(unit).map_or(0, |l| self.base(l, s, layer))
    }

    /// Unit bonus (`0x00625560`): total − base.
    pub fn unit_bonus(&self, unit: UnitId, s: u16, layer: u16) -> i32 {
        self.unit_total(unit, s, layer)
            .wrapping_sub(self.unit_base(unit, s, layer))
    }

    /// Max life (`0x00625D10`).
    pub fn max_life(&self, unit: UnitId) -> i32 {
        self.unit_total(unit, stat::MAXHP, 0)
    }

    /// Max mana (`0x00625D60`).
    pub fn max_mana(&self, unit: UnitId) -> i32 {
        self.unit_total(unit, stat::MAXMANA, 0)
    }

    /// Max stamina (`0x00625DB0`).
    pub fn max_stamina(&self, unit: UnitId) -> i32 {
        self.unit_total(unit, stat::MAXSTAMINA, 0)
    }

    /// Percent-adjusted value (`0x006255A0`).
    pub fn percent_adjusted(&self, l: ListId, s: u16, pct: u16, flag: bool) -> i32 {
        let b = self.base(l, s, 0);
        let mut v = b.wrapping_add(muldiv(b, self.total(l, pct, 0), 100));
        if flag {
            v = v.wrapping_add(self.total(l, s, 0).wrapping_sub(b));
        }
        v
    }

    // ---- stats.md §6 evaluation ---------------------------------------

    /// eval(L, k) (`0x00626200`, `stats.md` §6): no side effects. `L`
    /// must be extended.
    pub fn eval(&self, host: &dyn StatHost, l: ListId, k: i32) -> i32 {
        let s = key_stat(k);
        let list = self.l(l);
        let damagerelated = self.info(s).is_some_and(|i| i.damagerelated);
        // §6.1 sum (0x00624FE0).
        let mut v = get(&list.base, k).unwrap_or(0);
        let mut cur = list.ext.as_ref().and_then(|e| e.active);
        while let Some(c) = cur {
            let child = self.l(c);
            if !(damagerelated && child.flags & flag::DYNAMIC != 0) {
                let cv = match &child.ext {
                    Some(e) => get(&e.full, k),
                    None => get(&child.base, k),
                };
                v = v.wrapping_add(cv.unwrap_or(0));
            }
            cur = child.prev;
        }
        let Some(info) = self.info(s) else {
            return v;
        };
        if !info.a52 {
            return v;
        }
        // §6.2 op loop.
        let ext = list.ext.as_ref();
        let owner_unit = ext.map(|e| e.owner);
        let owner_type = list.owner_type;
        let mut acc = v;
        let mut prev = v;
        for e in info.entries.iter().take(16) {
            let Some(row) = op_row(e.op) else {
                continue;
            };
            let pass = match row.guard {
                Guard::Owner => owner_unit.is_some(),
                Guard::OwnerItem => owner_type == owner::ITEM,
                Guard::OwnerPlayer => owner_type == owner::PLAYER && self.class_stats(l).is_some(),
                Guard::OwnerPmPrev => {
                    prev != 0 && matches!(owner_type, owner::PLAYER | owner::MONSTER)
                }
                Guard::OwnerAct => owner_unit.is_some_and(|u| host.act_time(u).is_some()),
                Guard::ListtypePm => matches!(owner_type, owner::PLAYER | owner::MONSTER),
                Guard::UnitPm => list.unit.is_some_and(|u| {
                    matches!(self.unit_type_raw(u), Some(owner::PLAYER | owner::MONSTER))
                }),
                Guard::Never => false,
            };
            if !pass {
                continue;
            }
            if row.prev == Prev::OwnerItemBase && owner_type == owner::ITEM {
                if let Some(o) = owner_unit {
                    prev = self.unit_base(o, s, 0);
                }
            }
            let x = match row.operand {
                Operand::None => 0,
                Operand::OpbaseListTotal | Operand::OpbaseUnitTotal => {
                    let Some(binfo) = (e.base != NO_STAT).then(|| self.info(e.base)).flatten()
                    else {
                        continue;
                    };
                    let x = if row.operand == Operand::OpbaseListTotal {
                        self.raw_total(l, key(e.base, 0)).unwrap_or(0)
                    } else {
                        list.unit.map_or(0, |u| self.unit_total(u, e.base, 0))
                    };
                    let x = x >> binfo.valshift;
                    if x <= 0 {
                        continue;
                    }
                    x
                }
            };
            if row.contribution == Contribution::None {
                continue;
            }
            let r = self.eval(host, l, key(e.source, 0));
            if r == 0 {
                continue;
            }
            let shifted = || r.wrapping_mul(x) >> e.param;
            let t = || owner_unit.and_then(|u| host.act_time(u)).unwrap_or(0);
            let add = match row.contribution {
                Contribution::None => continue,
                Contribution::MuldivPrevR => {
                    if prev == 0 {
                        continue;
                    }
                    muldiv(prev, r, 100)
                }
                Contribution::ShiftRX => shifted(),
                Contribution::MuldivPrevShiftRX => muldiv(prev, shifted(), 100),
                Contribution::BytimeR => by_time(r, t()),
                Contribution::MuldivAccBytimeR => muldiv(acc, by_time(r, t()), 100),
                Contribution::CharstatManaBonus | Contribution::CharstatVitBonus => {
                    let d = r.wrapping_sub(get(&list.base, key(e.source, 0)).unwrap_or(0));
                    if d == 0 {
                        continue;
                    }
                    let c = self.class_stats(l).unwrap_or_default();
                    let per = if row.contribution == Contribution::CharstatManaBonus {
                        c.mana_per_magic
                    } else if s == stat::MAXSTAMINA {
                        c.stamina_per_vitality
                    } else {
                        c.life_per_vitality
                    };
                    i32::from(per).wrapping_mul(d) << 6
                }
            };
            acc = acc.wrapping_add(add);
        }
        acc
    }

    /// Charstats record of a player list's owner class.
    fn class_stats(&self, l: ListId) -> Option<super::ClassStats> {
        let e = self.ext(l)?;
        self.data.classes.get(e.class as usize).copied()
    }

    // ---- §6.3 full entries ---------------------------------------------

    /// Set-full `0x00625150`.
    fn set_full(
        &mut self,
        host: &mut dyn StatHost,
        l: ListId,
        k: i32,
        v: i32,
        unit: Option<UnitId>,
    ) {
        let s = key_stat(k);
        let (keepzero, a53) = self.info(s).map_or((false, false), |i| (i.keepzero, i.a53));
        let list = self.lm(l);
        let Some(e) = list.ext.as_mut() else {
            return;
        };
        let i = match find(&e.full, k) {
            Ok(i) => i,
            Err(_) if v == 0 => return,
            Err(i) => {
                e.full.insert(i, Entry { key: k, value: 0 });
                i
            }
        };
        let old = e.full[i].value;
        if v == 0 && !keepzero {
            e.full.remove(i);
        } else {
            e.full[i].value = v;
            if a53 {
                list.flags |= flag::PERMANENT;
            }
        }
        if old != v {
            self.notify(host, l, k, old, v, unit);
        }
    }

    /// Add-full `0x006250B0`.
    fn add_full(
        &mut self,
        host: &mut dyn StatHost,
        l: ListId,
        k: i32,
        d: i32,
        unit: Option<UnitId>,
    ) {
        let s = key_stat(k);
        let (keepzero, a53) = self.info(s).map_or((false, false), |i| (i.keepzero, i.a53));
        let list = self.lm(l);
        let Some(e) = list.ext.as_mut() else {
            return;
        };
        let i = find(&e.full, k).unwrap_or_else(|i| {
            e.full.insert(i, Entry { key: k, value: 0 });
            i
        });
        let old = e.full[i].value;
        let new = old.wrapping_add(d);
        e.full[i].value = new;
        if new > 0 && a53 {
            list.flags |= flag::PERMANENT;
        }
        if new == 0 && !keepzero {
            e.full.remove(i);
        }
        self.notify(host, l, k, old, new, unit);
    }

    // ---- §6.1 propagate, §6.4 recompute --------------------------------

    /// Propagate `0x00626920`(L, k, d, unit) (§6.1).
    pub fn propagate(
        &mut self,
        host: &mut dyn StatHost,
        l: ListId,
        k: i32,
        d: i32,
        unit: Option<UnitId>,
    ) {
        let s = key_stat(k);
        let Some(info) = self.info(s) else {
            return;
        };
        let (simple, damagerelated) = (!info.a51 && !info.a52, info.damagerelated);
        if d == 0 || self.l(l).flags & flag::SET != 0 {
            return;
        }
        let mut cur = if self.is_extended(l) {
            Some(l)
        } else {
            self.l(l).parent
        };
        while let Some(c) = cur {
            if simple {
                self.add_full(host, c, k, d, unit);
            } else {
                self.recompute(host, c, k, unit);
            }
            let list = self.l(c);
            if list.flags & flag::SET != 0 || (damagerelated && list.flags & flag::DYNAMIC != 0) {
                break;
            }
            cur = list.parent;
        }
    }

    /// Recompute `0x006266C0` (§6.4).
    pub fn recompute(
        &mut self,
        host: &mut dyn StatHost,
        l: ListId,
        k: i32,
        unit: Option<UnitId>,
    ) -> i32 {
        let v = self.eval(host, l, k);
        let s = key_stat(k);
        let data = Arc::clone(&self.data);
        let Some(info) = data.stats.get(s) else {
            return v;
        };
        if !info.a51 {
            self.set_full(host, l, k, v, unit);
            return v;
        }
        if v == 0 {
            self.set_full(host, l, k, 0, unit);
        }
        let mut update = true;
        let block = op_row(info.op).map_or(RecomputeBlock::None, |r| r.recompute_block);
        for (j, &os) in info.op_stats.iter().enumerate() {
            // TODO(stat-lists.md §6.4): an op stat ≥ n other than 0xFFFF
            // is not described; the load fix-up stops at it, so does this.
            if os == NO_STAT || self.info(os).is_none() {
                break;
            }
            let ks = key(os, 0);
            let vs = self.recompute(host, l, ks, unit);
            self.set_full(host, l, ks, vs, unit);
            if vs != 0 && self.blocks(l, block, info.entry(j).base) {
                update = false;
            }
        }
        if update && v != 0 {
            self.set_full(host, l, k, v, unit);
            for &d in info.deps.iter().take(64) {
                let kd = key(d, 0);
                let vd = self.recompute(host, l, kd, unit);
                self.set_full(host, l, kd, vd, unit);
            }
        }
        v
    }

    /// Whether a recompute block clears update (§6.4 rule 4).
    fn blocks(&self, l: ListId, block: RecomputeBlock, base: u16) -> bool {
        let list = self.l(l);
        let pm = |t: u32| matches!(t, owner::PLAYER | owner::MONSTER);
        match block {
            RecomputeBlock::None => false,
            RecomputeBlock::Always => true,
            RecomputeBlock::ListtypePm => pm(list.owner_type),
            RecomputeBlock::ListtypeItem => list.owner_type == owner::ITEM,
            RecomputeBlock::ListtypePmAndEntrybaseListTotalPos => {
                pm(list.owner_type)
                    && base != NO_STAT
                    && self.raw_total(l, key(base, 0)).unwrap_or(0) > 0
            }
            RecomputeBlock::UnitPmAndEntrybaseUnitTotalPos => list.unit.is_some_and(|u| {
                self.unit_type_raw(u).is_some_and(pm)
                    && base != NO_STAT
                    && self.unit_total(u, base, 0) > 0
            }),
        }
    }

    // ---- §7 notification -----------------------------------------------

    fn notify(
        &mut self,
        host: &mut dyn StatHost,
        l: ListId,
        k: i32,
        old: i32,
        new: i32,
        unit: Option<UnitId>,
    ) {
        let Some(e) = self.ext(l) else {
            return;
        };
        let (Some(cb), owner) = (e.callback, e.owner) else {
            return;
        };
        if !self.info(key_stat(k)).is_some_and(|i| i.fcallback) {
            return;
        }
        let ev = CallbackEvent {
            list: l,
            owner,
            unit,
            key: k,
            old,
            new,
        };
        host.on_callback(self, &ev);
        match cb {
            ValueCallback::Server => self.server_callback(host, l, &ev),
        }
    }

    /// The server callback `0x0055B800` (§7.2).
    fn server_callback(&mut self, host: &mut dyn StatHost, l: ListId, ev: &CallbackEvent) {
        let s = key_stat(ev.key);
        let Some(info) = self.info(s) else {
            return;
        };
        let owner = ev.owner;
        if info.itemevent1 > 0 {
            host.item_event(self, owner, s, ev.new);
        }
        match s {
            stat::MAXHP | stat::MAXMANA | stat::MAXSTAMINA => {
                let current = s - 1;
                let (new, old) = (ev.new, ev.old);
                if new != old && old > 0 {
                    let c = self.unit_total(owner, current, 0);
                    if c > 0 {
                        let o = if old <= 256 { 256 } else { old };
                        let q = x87_rescale(new, o, c, self.data.rescale_precision);
                        let m = q.max(1);
                        let v = if m >= new { new } else { m };
                        self.unit_set(host, owner, current, v, 0);
                    }
                }
                // TODO(stat-lists.md §7.2): read as a step of its own after
                // the rescale, whatever its condition; confirm with a
                // monster max-life recording.
                if s == stat::MAXHP && self.l(l).owner_type == owner::MONSTER {
                    let class = self.ext(l).map_or(0, |e| e.class);
                    let dr = self
                        .data
                        .damage_regen
                        .get(class as usize)
                        .copied()
                        .unwrap_or(0);
                    if dr != 0 {
                        let v = (ev.new >> 8).wrapping_mul(dr as i32) >> 4;
                        self.unit_set(host, owner, stat::HPREGEN, v, 0);
                    }
                }
            }
            83 | 97 | 98 | 107 | 126 | 127 | 151 | 188 | 204 => {
                host.skill_stat_changed(self, owner, ev.key, ev.old, ev.new);
            }
            _ => {}
        }
    }

    // ---- §5 base writes --------------------------------------------------

    /// Mod insert `0x00624DD0` (§11.1).
    fn mod_insert(&mut self, l: ListId, k: i32) {
        let s = key_stat(k);
        if MOD_EXCLUDED.contains(&s) || !self.info(s).is_some_and(|i| i.saved) {
            return;
        }
        if let Some(e) = self.ext_mut(l) {
            if let Err(i) = e.mods.binary_search(&k) {
                e.mods.insert(i, k);
            }
        }
    }

    fn mod_insert_player(&mut self, l: ListId, k: i32) {
        let list = self.l(l);
        if list.ext.is_some() && list.owner_type == owner::PLAYER {
            self.mod_insert(l, k);
        }
    }

    /// Set (`0x006270B0`; with a callback unit `0x00627170`) (§5.1).
    /// Returns whether the value changed.
    pub fn set(
        &mut self,
        host: &mut dyn StatHost,
        l: ListId,
        s: u16,
        value: i32,
        layer: u16,
        unit: Option<UnitId>,
    ) -> bool {
        let k = key(s, layer);
        let list = self.lm(l);
        let i = match find(&list.base, k) {
            Ok(i) => i,
            Err(_) if value == 0 => return false,
            Err(i) => {
                list.base.insert(i, Entry { key: k, value: 0 });
                i
            }
        };
        let old = list.base[i].value;
        let d = value.wrapping_sub(old);
        if d == 0 {
            return false;
        }
        if value == 0 {
            list.base.remove(i);
        } else {
            list.base[i].value = value;
        }
        self.propagate(host, l, k, d, unit);
        self.mod_insert_player(l, k);
        true
    }

    /// Unit set `0x00627260` (§5.2): set on the unit's list, then the
    /// (redundant) mod insert for a player.
    pub fn unit_set(
        &mut self,
        host: &mut dyn StatHost,
        unit: UnitId,
        s: u16,
        value: i32,
        layer: u16,
    ) {
        if let Some(l) = self.unit_list(unit) {
            self.set(host, l, s, value, layer, None);
            self.mod_insert_player(l, key(s, layer));
        }
    }

    /// Add (`0x00627030`) (§5.3).
    pub fn add(&mut self, host: &mut dyn StatHost, l: ListId, s: u16, d: i32, layer: u16) {
        if d == 0 {
            return;
        }
        let k = key(s, layer);
        let list = self.lm(l);
        let i = find(&list.base, k).unwrap_or_else(|i| {
            list.base.insert(i, Entry { key: k, value: 0 });
            i
        });
        let v = list.base[i].value.wrapping_add(d);
        if v == 0 {
            list.base.remove(i);
        } else {
            list.base[i].value = v;
        }
        self.propagate(host, l, k, d, None);
        self.mod_insert_player(l, k);
    }

    /// Unit add `0x006272B0`.
    pub fn unit_add(&mut self, host: &mut dyn StatHost, unit: UnitId, s: u16, d: i32, layer: u16) {
        if let Some(l) = self.unit_list(unit) {
            self.add(host, l, s, d, layer);
        }
    }

    /// Remove all `0x00627340` (§5.4).
    pub fn remove_all(&mut self, host: &mut dyn StatHost, l: ListId) {
        while let Some(&e) = self.l(l).base.first() {
            self.lm(l).base.remove(0);
            self.propagate(host, l, e.key, e.value.wrapping_neg(), None);
            self.mod_insert_player(l, e.key);
        }
    }

    /// Merge `0x006274F0` (§5.5): add each source base entry, in order.
    pub fn merge(&mut self, host: &mut dyn StatHost, target: ListId, source: ListId) {
        for e in self.l(source).base.clone() {
            self.add(
                host,
                target,
                key_stat(e.key),
                e.value,
                super::key_layer(e.key),
            );
        }
    }

    // ---- §8 chain operations ---------------------------------------------

    /// Keys of L's full array whose stat has A53 (first 16, §8.1.7).
    fn a53_keys(&self, l: ListId) -> Vec<i32> {
        self.ext(l)
            .map(|e| {
                e.full
                    .iter()
                    .filter(|e| self.info(key_stat(e.key)).is_some_and(|i| i.a53))
                    .take(16)
                    .map(|e| e.key)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Values A of §8.1.8 and the owner.
    fn values(&self, l: ListId) -> (Vec<Entry>, Option<UnitId>) {
        let list = self.l(l);
        match &list.ext {
            Some(e) => (e.full.clone(), Some(e.owner)),
            None => (list.base.clone(), None),
        }
    }

    fn is_damagerelated(&self, k: i32) -> bool {
        self.info(key_stat(k)).is_some_and(|i| i.damagerelated)
    }

    /// Attach `0x00626E10`(U, L, reset) (§8.1).
    pub fn attach(&mut self, host: &mut dyn StatHost, unit: UnitId, l: ListId, reset: bool) {
        let Some(r) = self.unit_list(unit).filter(|&r| self.is_extended(r)) else {
            return;
        };
        self.detach(host, l);
        let mut a = Some(r);
        while let Some(x) = a {
            if x == l {
                return;
            }
            // A freed parent (edge case 6) ends the ancestor walk.
            a = self.try_l(x).and_then(|x| x.parent);
        }
        if self.l(l).flags & flag::TEMPONLY != 0 {
            self.lm(r).flags |= flag::NEWLENGTH;
        }
        let parked = self.l(l).flags & flag::SET != 0;
        let head = {
            let e = self.ext(r).expect("extended");
            if parked {
                e.parked
            } else {
                e.active
            }
        };
        {
            let list = self.lm(l);
            list.prev = head;
            list.next = None;
            list.parent = Some(r);
            list.unit = Some(unit);
        }
        if let Some(h) = head {
            self.lm(h).next = Some(l);
        }
        let e = self.ext_mut(r).expect("extended");
        if parked {
            e.parked = Some(l);
            return;
        }
        e.active = Some(l);
        if self.is_extended(l) && self.l(l).flags & flag::PERMANENT != 0 {
            for k in self.a53_keys(l) {
                self.recompute(host, l, k, None);
            }
        }
        let (a, owner) = self.values(l);
        if reset {
            self.lm(l).flags &= !flag::DYNAMIC;
        } else {
            self.lm(l).flags |= flag::DYNAMIC;
        }
        for e in a {
            if !reset && self.is_damagerelated(e.key) {
                continue;
            }
            self.propagate(host, r, e.key, e.value, owner);
        }
    }

    /// Detach `0x006269F0`(L) (§8.2). A parked child of a freed parent
    /// (edge case 6) has no live parent whose heads could name it: only
    /// its own links are cleared.
    pub fn detach(&mut self, host: &mut dyn StatHost, l: ListId) {
        let p = self.l(l).parent;
        if let Some(p) = p {
            let prev = self.l(l).prev;
            if let Some(e) = self.try_ext_mut(p) {
                if e.active == Some(l) {
                    e.active = prev;
                }
                if e.parked == Some(l) {
                    e.parked = prev;
                }
            }
            self.lm(l).parent = None;
        }
        let (prev, next) = (self.l(l).prev, self.l(l).next);
        if let Some(n) = next {
            self.lm(n).prev = prev;
        }
        if let Some(pv) = prev {
            self.lm(pv).next = next;
        }
        let list = self.lm(l);
        list.prev = None;
        list.next = None;
        let old_unit = list.unit.take();
        if list.flags & flag::SET != 0 {
            return;
        }
        if self.is_extended(l) && self.l(l).flags & flag::PERMANENT != 0 {
            for k in self.a53_keys(l) {
                self.recompute(host, l, k, None);
            }
        }
        if let Some(p) = p {
            let dynamic = self.l(l).flags & flag::DYNAMIC != 0;
            let (a, owner) = self.values(l);
            for e in a {
                if dynamic && self.is_damagerelated(e.key) {
                    continue;
                }
                self.propagate(host, p, e.key, e.value.wrapping_neg(), owner);
            }
        }
        let (state, cb) = (self.l(l).state, self.l(l).remove_callback);
        if let (Some(u), Some(cb)) = (old_unit, cb) {
            host.list_removed(self, u, state, l, cb);
        }
    }

    /// Detach `0x006277E0`(unit, L): as [`Self::detach`].
    pub fn unit_detach(&mut self, host: &mut dyn StatHost, l: ListId) {
        self.detach(host, l);
    }

    /// Free `0x00626C00`(L) (§8.3). Parked children keep pointing at the
    /// freed parent (edge case 6): their parent id no longer resolves.
    pub fn free(&mut self, host: &mut dyn StatHost, l: ListId) {
        self.detach(host, l);
        if self.is_extended(l) {
            let mut cur = self.heads(l).0;
            while let Some(c) = cur {
                {
                    let child = self.lm(c);
                    child.parent = None;
                    child.unit = None;
                }
                if self.is_extended(c) {
                    cur = self.l(c).prev;
                    continue;
                }
                if self.heads(l).0 == Some(c) {
                    let prev = self.l(c).prev;
                    self.ext_mut(l).expect("extended").active = prev;
                }
                self.free(host, c);
                cur = self.heads(l).0;
            }
            let owner = self.ext(l).map(|e| e.owner);
            if let Some(o) = owner {
                if self.units.get(&o) == Some(&l) {
                    self.units.remove(&o);
                }
            }
        }
        let slot = &mut self.slots[l.index as usize];
        slot.1 = None;
        slot.0 = slot.0.wrapping_add(1);
        self.free.push(l.index);
    }

    /// `0x00626CD0`: frees `l` only when it is a plain list.
    pub fn free_plain(&mut self, host: &mut dyn StatHost, l: ListId) {
        if !self.is_extended(l) {
            self.free(host, l);
        }
    }

    /// Equip and swap `0x00627910`(U, I, reset) (§8.4). `item_list` is
    /// I's list, `swap_location` whether I is in body location 11 or 12.
    pub fn equip(
        &mut self,
        host: &mut dyn StatHost,
        unit: UnitId,
        item_list: Option<ListId>,
        swap_location: bool,
        reset: bool,
    ) {
        let Some(il) = item_list else {
            return;
        };
        if swap_location {
            self.detach(host, il);
        } else if self.l(il).unit == Some(unit) {
            let dynamic = self.l(il).flags & flag::DYNAMIC != 0;
            if reset && dynamic {
                self.make_static(host, unit, il, swap_location);
            } else if !reset && !dynamic {
                self.make_dynamic(host, unit, il, swap_location);
            }
        } else {
            self.attach(host, unit, il, reset);
            self.lm(il).unit = Some(unit);
        }
    }

    /// `0x00627860` (§8.6): make I's list static on U.
    pub fn make_static(&mut self, host: &mut dyn StatHost, unit: UnitId, il: ListId, swap: bool) {
        self.toggle_dynamic(host, unit, il, swap, false);
    }

    /// `0x00627A40` (§8.6): make I's list dynamic on U.
    pub fn make_dynamic(&mut self, host: &mut dyn StatHost, unit: UnitId, il: ListId, swap: bool) {
        self.toggle_dynamic(host, unit, il, swap, true);
    }

    fn toggle_dynamic(
        &mut self,
        host: &mut dyn StatHost,
        unit: UnitId,
        il: ListId,
        swap: bool,
        dynamic: bool,
    ) {
        if self.l(il).unit != Some(unit) {
            self.equip(host, unit, Some(il), swap, !dynamic);
            return;
        }
        let Some(r) = self.unit_list(unit).filter(|&r| self.is_extended(r)) else {
            return;
        };
        let is_dynamic = self.l(il).flags & flag::DYNAMIC != 0;
        if is_dynamic == dynamic {
            return;
        }
        let item = self.owner(il);
        if dynamic {
            self.lm(il).flags |= flag::DYNAMIC;
        } else {
            self.lm(il).flags &= !flag::DYNAMIC;
        }
        let (a, _) = self.values(il);
        for e in a {
            if self.is_damagerelated(e.key) {
                let d = if dynamic {
                    e.value.wrapping_neg()
                } else {
                    e.value
                };
                self.propagate(host, r, e.key, d, item);
            }
        }
    }

    /// Park and unpark `0x006279A0`(unit, state, park) (§8.5): true when
    /// the list changed.
    pub fn park(&mut self, host: &mut dyn StatHost, unit: UnitId, state: u32, park: bool) -> bool {
        let Some(r) = self.unit_list(unit) else {
            return false;
        };
        let Some(l) = self.list_of_state(r, state) else {
            return false;
        };
        let parked = self.l(l).flags & flag::SET != 0;
        if parked == park {
            return false;
        }
        self.detach(host, l);
        if park {
            self.lm(l).flags |= flag::SET;
        } else {
            self.lm(l).flags &= !flag::SET;
        }
        self.attach(host, unit, l, true);
        true
    }

    /// By-time refresh `0x006276C0`(U, I) (§8.7).
    pub fn by_time_refresh(&mut self, host: &mut dyn StatHost, unit: UnitId, item_list: ListId) {
        let Some(r) = self.unit_list(unit).filter(|&r| self.is_extended(r)) else {
            return;
        };
        if !matches!(self.l(r).owner_type, owner::PLAYER | owner::MONSTER) {
            return;
        }
        let entries: Vec<Entry> = self
            .ext(item_list)
            .map(|e| e.full.clone())
            .unwrap_or_default();
        for e in entries {
            let Some(info) = self.info(key_stat(e.key)) else {
                continue;
            };
            if !matches!(info.op, 6 | 7) {
                continue;
            }
            for os in info.op_stats {
                if os == NO_STAT {
                    break;
                }
                let k = key(os, 0);
                let v = self.eval(host, r, k);
                self.set_full(host, r, k, v, Some(unit));
            }
        }
    }

    /// Death `0x00627540`(unit) (§8.8.1).
    pub fn death(&mut self, host: &mut dyn StatHost, unit: UnitId) {
        let Some(r) = self.unit_list(unit).filter(|&r| self.is_extended(r)) else {
            return;
        };
        let mut cur = self.heads(r).0;
        while let Some(c) = cur {
            let list = self.l(c);
            let next = list.prev;
            let drop = list.owner_type != owner::ITEM
                && list.flags & 0x181 == 0
                && !host.stays_on_death(self, unit, list.state);
            if drop && list.ext.is_none() {
                self.free(host, c);
                cur = self.heads(r).0;
            } else {
                cur = next;
            }
        }
    }

    /// Overlay removal `0x00627410`(unit) (§8.8.2).
    pub fn remove_overlay(&mut self, host: &mut dyn StatHost, unit: UnitId) {
        let Some(r) = self.unit_list(unit) else {
            return;
        };
        self.lm(r).flags &= !flag::REMOVE_OVERLAY;
        if let Some(o) = self.list_by_flags(r, flag::OVERLAY) {
            self.remove_all(host, o);
        }
    }

    // ---- §9.3 queries ------------------------------------------------------

    /// List of a state `0x00625650`(unit list, s): first in the active
    /// chain with that state, else first in the parked chain.
    pub fn list_of_state(&self, r: ListId, state: u32) -> Option<ListId> {
        let (active, parked) = self.heads(r);
        [active, parked].into_iter().find_map(|head| {
            self.chain(head)
                .into_iter()
                .find(|&c| self.l(c).state == state)
        })
    }

    /// By flags `0x006256E0`: the parked chain when SET is asked, else
    /// the active chain; first list with any asked flag.
    pub fn list_by_flags(&self, r: ListId, flags: u32) -> Option<ListId> {
        let (active, parked) = self.heads(r);
        let head = if flags & flag::SET != 0 {
            parked
        } else {
            active
        };
        self.chain(head)
            .into_iter()
            .find(|&c| self.l(c).flags & flags != 0)
    }

    /// The owner (type, GUID) of the unit's list of `state`, if any.
    pub fn state_list_owner(&self, unit: UnitId, state: u32) -> Option<(u32, u32)> {
        let r = self.unit_list(unit)?;
        let l = self.list_of_state(r, state)?;
        let list = self.l(l);
        Some((list.owner_type, list.owner_guid))
    }

    /// Detaches and frees (when plain) the unit's list of `state`
    /// (regeneration, §10.1).
    pub fn free_state_list(&mut self, host: &mut dyn StatHost, unit: UnitId, state: u32) {
        let Some(r) = self.unit_list(unit) else {
            return;
        };
        if let Some(l) = self.list_of_state(r, state) {
            self.detach(host, l);
            self.free_plain(host, l);
        }
    }

    // ---- §10.4 expiry ------------------------------------------------------

    /// Expiry `0x00627460`(unit, frame) (§10.4). Frame 0 is the client
    /// form (expire −= 1 first, expired at ≤ 0).
    ///
    /// An expired extended list loops forever in 1.14d (edge case 4, open
    /// question 5); d2rs panics there instead of hanging.
    pub fn expire_lists(&mut self, host: &mut dyn StatHost, unit: UnitId, frame: i32) {
        let Some(r) = self.unit_list(unit).filter(|&r| self.is_extended(r)) else {
            return;
        };
        if frame == 0 {
            for c in self.active_chain(r) {
                let list = self.lm(c);
                if list.flags & flag::NEWLENGTH != 0 {
                    list.expire = list.expire.wrapping_sub(1);
                }
            }
        }
        let mut cur = self.heads(r).0;
        while let Some(c) = cur {
            let list = self.l(c);
            let next = list.prev;
            if list.flags & flag::NEWLENGTH != 0 && list.expire <= frame {
                assert!(
                    list.ext.is_none(),
                    "expired extended stat list: endless loop in 1.14d (stat-lists.md §10.4)"
                );
                self.free(host, c);
                cur = self.heads(r).0;
            } else {
                cur = next;
            }
        }
    }

    // ---- §11 mod array -------------------------------------------------------

    /// Flush values (§11.2): for each mod key in order, the unit's base
    /// value of that key (0 when absent). The caller sends them
    /// (`0x006258D0` with sender `0x00548520`).
    pub fn mod_values(&self, unit: UnitId) -> Vec<(i32, i32)> {
        let Some(r) = self.unit_list(unit) else {
            return Vec::new();
        };
        self.mods(r)
            .into_iter()
            .map(|k| (k, self.raw_base(r, k).unwrap_or(0)))
            .collect()
    }

    /// Clear `0x00625960` (§11.3).
    pub fn clear_mods(&mut self, unit: UnitId) {
        if let Some(e) = self.unit_list(unit).and_then(|r| self.ext_mut(r)) {
            e.mods.clear();
        }
    }

    /// Single stat `0x00625870` (§11.4): the base value (layer 0) when the
    /// key is not in the mod array and is present in the base array.
    pub fn single_stat(&self, unit: UnitId, s: u16) -> Option<i32> {
        let r = self.unit_list(unit)?;
        let k = key(s, 0);
        if self.ext(r)?.mods.binary_search(&k).is_ok() {
            return None;
        }
        self.raw_base(r, k)
    }

    // ---- stats.md §9.2 ---------------------------------------------------------

    /// Clamp current to max `0x006275B0` (`stats.md` §9.2).
    pub fn clamp_to_max(&mut self, host: &mut dyn StatHost, unit: UnitId) {
        for (cur, max) in [
            (stat::STAMINA, stat::MAXSTAMINA),
            (stat::MANA, stat::MAXMANA),
            (stat::HITPOINTS, stat::MAXHP),
        ] {
            let c = self.unit_total(unit, cur, 0);
            let m = self.unit_total(unit, max, 0);
            if c > m {
                self.unit_add(host, unit, cur, m.wrapping_sub(c), 0);
            }
        }
    }
}
