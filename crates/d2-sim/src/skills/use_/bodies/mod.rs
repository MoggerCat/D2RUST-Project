// Spec: specs/skills/bodies.md, specs/skills/functions.tsv (status spec'd-here), specs/skills/use.md §8
//! The per-skill start / do bodies (`skills/bodies.md`). Two kinds:
//!
//! - [`start`]: bodies that read and change nothing (srvst 18 Attract,
//!   `functions.tsv` notes). The start core (`use.md` §5.3 step 6.6) runs
//!   them directly.
//! - [`run_start`] / [`run_do`]: the bodies of the `functions.tsv` rows
//!   with status `spec'd-here` ([`START_BODIES`], [`DO_BODIES`]), over
//!   the [`BodyWorld`] seam. They run behind
//!   [`super::SkillFunctions`]: a provider answers `srvst` / `srvdo`
//!   from here when the slot has a body and from its own seam otherwise
//!   (every other filled slot: status `mapped`, `use.md` Open question
//!   10). The wired provider is `crate::wiring::interaction::UseView`.
//!
//! Helpers (§2) are in [`helpers`], starts (§3) in [`starts`], do
//! functions (§4) in [`dos`]; the `srvmissile` path (§5) is the do core's
//! (`use_::do_core`) with the missile record of [`skill_missile`].
//!
//! Status: implemented, unverified (`bodies.md` is a draft; no recording
//! covers these bodies, its Open questions 1–3).

pub mod dos;
pub mod helpers;
pub mod starts;

#[cfg(test)]
mod tests;

pub use helpers::*;

use super::UseWorld;
use crate::combat::{CombatTables, CombatWorld};
use crate::skills::{SkillEntry, SkillTables};

/// `srvst` slots whose body reads and changes nothing ([`start`]).
pub const PURE_START: &[u16] = &[18];
/// `srvst` slots with a body over [`BodyWorld`] (`functions.tsv` status
/// `spec'd-here`).
pub const START_BODIES: &[u16] = &[1, 2, 3, 4, 5, 15, 29, 32, 33, 46, 65];
/// `srvdo` slots with a body over [`BodyWorld`] (status `spec'd-here`).
pub const DO_BODIES: &[u16] = &[1, 2, 18, 30, 65];

/// `srvst[index](game, unit, skill, level)` when the slot's body reads
/// and changes nothing; `None` otherwise.
pub fn start(index: u16) -> Option<i32> {
    match index {
        // SrvSt18 Attract `0x005C3260`: `mov eax, 1; ret 8`, arguments
        // unread, nothing changed.
        18 => Some(1),
        _ => None,
    }
}

/// The remove callbacks of §2.8 (list +0x38 ids; the 1.14d addresses).
pub mod callback {
    /// Default `0x0056E900`.
    pub const DEFAULT: u32 = 0x0056_E900;
    /// Self aura `0x005CEC50`.
    pub const SELF_AURA: u32 = 0x005C_EC50;
    /// Buff `0x005C9420`.
    pub const BUFF: u32 = 0x005C_9420;
    /// AI curse `0x005C3370` (Dim Vision, Terror).
    pub const AI_CURSE: u32 = 0x005C_3370;
    /// Every callback id §2.8 specifies.
    pub const ALL: [u32; 4] = [DEFAULT, SELF_AURA, BUFF, AI_CURSE];
}

/// The itemstatcost fields the bodies read (`direct` +0x04 bit 4,
/// `updateanimrate` +0x04 bit 9, `maxstat` +0x32).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BodyStat {
    pub direct: bool,
    pub updateanimrate: bool,
    /// −1 when none.
    pub maxstat: i32,
}

/// The table data the bodies read beyond [`SkillTables`] and
/// [`CombatTables`]: itemstatcost flags, states `group` and `aura`, the
/// overlay count. A provider without them answers "no record".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BodyTables {
    /// By itemstatcost row.
    pub stats: Vec<BodyStat>,
    /// States `group` (+0x1E) by state.
    pub state_group: Vec<i32>,
    /// States `aura` (+0x10 bit 1) by state.
    pub state_aura: Vec<bool>,
    /// `overlay.txt` record count.
    pub overlay_count: i32,
}

impl BodyTables {
    /// From a loaded `.bin` set.
    pub fn from_bin(set: &d2_data::bin::BinSet) -> Result<Self, crate::skills::TablesError> {
        use crate::skills::TablesError;
        let table = |n: &'static str| set.table(n).ok_or(TablesError::Missing(n));
        Self::from_tables(table("itemstatcost")?, table("states")?, table("overlay")?)
    }

    /// From the itemstatcost, states and overlay tables.
    pub fn from_tables(
        itemstatcost: &d2_data::bin::BinTable,
        states: &d2_data::bin::BinTable,
        overlay: &d2_data::bin::BinTable,
    ) -> Result<Self, crate::skills::TablesError> {
        use d2_data::tables::{decode_all, Itemstatcost, States};
        let isc: Vec<Itemstatcost> = decode_all(itemstatcost)?;
        let states: Vec<States> = decode_all(states)?;
        Ok(Self {
            stats: isc
                .iter()
                .map(|r| BodyStat {
                    direct: r.direct,
                    updateanimrate: r.updateanimrate,
                    maxstat: i32::from(r.maxstat as i16),
                })
                .collect(),
            state_group: states.iter().map(|r| i32::from(r.group as i16)).collect(),
            state_aura: states.iter().map(|r| r.aura).collect(),
            overlay_count: i32::try_from(overlay.count).unwrap_or(i32::MAX),
        })
    }

    /// The itemstatcost fields of stat `s`, `None` past the table.
    pub fn stat(&self, s: i32) -> Option<BodyStat> {
        usize::try_from(s)
            .ok()
            .and_then(|i| self.stats.get(i))
            .copied()
    }
}

/// A unit event handler record (§2.13, 0x20 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handler {
    /// +0x00 event (`events.txt` index).
    pub event: u8,
    /// +0x04 key type (1 = a state).
    pub key_type: i32,
    /// +0x08 key.
    pub key: i32,
    /// +0x0C skill.
    pub skill: i32,
    /// +0x10 level.
    pub level: i32,
    /// +0x14 function (event table `0x007325B0` index).
    pub func: i32,
}

/// One room of a scan (§2.12): the town test and the room's unit list in
/// list order (head +0x74, next +0xE8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanRoom<U> {
    pub town: bool,
    pub units: Vec<U>,
}

/// The skill missile parameter record of §2.4 (`missiles.md` §R2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissileRequest<U> {
    pub flags: u32,
    pub owner: U,
    pub origin: Option<U>,
    pub class: i32,
    pub x: i32,
    pub y: i32,
    pub target_x: i32,
    pub target_y: i32,
    pub skill: i32,
    pub level: i32,
    pub attack_bonus: i32,
}

/// Message 0xA3 queued by the progressive finisher (§2.14 step 5,
/// `0x00571AA0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgressiveMsg<U> {
    pub charges: u8,
    pub skill: u16,
    pub level: u16,
    pub unit: U,
    pub target: U,
    pub roll: u32,
}

/// What the bodies need beyond the skill use pipeline's [`UseWorld`] (a
/// seam). Combat runs through [`BodyWorld::combat`] on the same units.
/// Providers by spec: stat lists and states (`sim/stat-lists.md`), rooms
/// (`drlg/rooms.md` §6, `sim/unit-order.md` §5), items (quantity,
/// durability, composits), monsters (AI install `monsters/ai.md` §3.3),
/// player messages. Each method names its 1.14d address.
pub trait BodyWorld: UseWorld {
    /// A stat list handle.
    type List: Copy + Eq + std::fmt::Debug;
    /// Combat's view of the same units.
    type Combat: CombatWorld<Unit = Self::Unit, Item = Self::Item>;
    fn combat(&mut self) -> &mut Self::Combat;

    // ---- data
    /// The itemstatcost record of `s` (`None`: no record).
    fn stat_info(&self, s: i32) -> Option<BodyStat>;
    /// States count (`[0x744304]+0xC4`).
    fn state_count(&self) -> i32;
    /// State `s` has flag group `g` (`runtime-maps.md` §4 bitsets).
    fn state_flag(&self, s: i32, g: usize) -> bool;
    /// States `group` (+0x1E) of `s`.
    fn state_group(&self, s: i32) -> i32;
    /// States `aura` (+0x10 bit 1).
    fn state_is_aura(&self, s: i32) -> bool;
    /// Overlay count (bound of `srvoverlay` / `tgtoverlay`).
    fn overlay_count(&self) -> i32;

    // ---- states on a unit (`stat-lists.md` §9)
    /// `0x0063A7B0(unit, g)`: any state with flag bit g.
    fn has_group(&self, u: Self::Unit, g: usize) -> bool;
    /// `0x00639DB0`: state on / off (toggle with update-queue insert).
    fn state_on(&mut self, u: Self::Unit, s: i32, on: bool);
    /// `0x00639E30(unit, s, 1)`: mark changed.
    fn mark_state_changed(&mut self, u: Self::Unit, s: i32);
    /// `0x0063A2D0` → `0x0063A180`: every state of group `g` cleared at
    /// once (changed bits set, bits cleared, unit queued).
    fn clear_group_states(&mut self, u: Self::Unit, g: usize);
    /// `0x0064C040`: queue the unit for update.
    fn queue_update(&mut self, u: Self::Unit);
    /// `0x0063A4A0`: the state stays on the unit at death.
    fn stays_on_death(&self, u: Self::Unit, s: i32) -> bool;

    // ---- stat lists (`stat-lists.md`)
    /// `0x006256B0(unit, s)`: the unit's list of state `s`.
    fn state_list(&self, u: Self::Unit, s: i32) -> Option<Self::List>;
    /// `0x00625760`: the unit's first list with all of `flags`.
    fn first_list_with_flags(&self, u: Self::Unit, flags: u32) -> Option<Self::List>;
    /// `0x006251F0(pool, flags, expire, owner type, owner GUID)`; no
    /// owner → type 6, GUID −1.
    fn alloc_list(
        &mut self,
        flags: u32,
        expire: i32,
        owner: Option<Self::Unit>,
    ) -> Option<Self::List>;
    fn list_state(&self, l: Self::List) -> i32;
    /// `0x006252D0`.
    fn set_list_state(&mut self, l: Self::List, s: i32);
    /// List +0x1C skill, +0x20 level.
    fn list_skill(&self, l: Self::List) -> (i32, i32);
    /// `0x006260D0`, `0x006260F0`.
    fn set_list_skill(&mut self, l: Self::List, skill: i32, lvl: i32);
    fn list_expire(&self, l: Self::List) -> i32;
    /// `0x00625310`.
    fn set_list_expire(&mut self, l: Self::List, e: i32);
    /// `0x00625D00(list, s, 0)`.
    fn list_get(&self, l: Self::List, s: i32) -> i32;
    /// List set `0x00627150(list, s, v, 0)` (`0x006270B0` in the fills).
    fn list_set(&mut self, l: Self::List, s: i32, v: i32);
    /// `0x00626E10(unit, list, 1)`.
    fn attach(&mut self, u: Self::Unit, l: Self::List);
    /// `0x00625CE0(list, f)`.
    fn set_remove_callback(&mut self, l: Self::List, cb: u32);
    /// `0x006277E0(unit, list)` then `0x00626CD0(list)`: detach (which
    /// runs the list's remove callback, [`remove_callback`]) and free.
    fn detach_free(&mut self, u: Self::Unit, l: Self::List);

    // ---- unit event handlers (§2.13)
    /// `0x005C0AD0`: allocate the record and prepend it.
    fn add_handler(&mut self, u: Self::Unit, h: Handler);
    /// `0x005C0B50(game, unit, type, key)`.
    fn remove_handlers(&mut self, u: Self::Unit, key_type: i32, key: i32);

    // ---- rooms and relations (§2.11, §2.12)
    /// The rooms a scan visits: the adjacency array (`0x00619790`,
    /// itself included) of the source's room (`at` = `None`) or of the
    /// room containing `at`, searched from the source's room
    /// (`0x00463740`). `None`: no such room.
    fn scan_rooms(
        &self,
        source: Self::Unit,
        at: Option<(i32, i32)>,
    ) -> Option<Vec<ScanRoom<Self::Unit>>>;
    /// `0x00554DE0`: allies (same unit after the monster owner
    /// resolution, or two players in one party).
    fn allied(&self, a: Self::Unit, b: Self::Unit) -> bool;
    /// `0x00552FD0`: a missile's owner.
    fn missile_owner(&self, u: Self::Unit) -> Option<Self::Unit>;
    /// `0x0058F0D0`: a monster's minion owner.
    fn minion_owner(&self, u: Self::Unit) -> Option<Self::Unit>;
    /// `0x00574A20(unit, pet GUID)` and the pettype `unsummon` bit (+0x04
    /// bit 3) of the pet's type.
    fn pet_unsummonable(&self, u: Self::Unit, pet: Self::Unit) -> bool;

    // ---- unit fields
    /// `0x00623B10`: the frame bonus.
    fn frame_bonus(&self, u: Self::Unit) -> i32;
    /// Unit +0x44 (animation frame, 8.8).
    fn set_anim_frame(&mut self, u: Self::Unit, v: i32);
    /// Used skill entry param `i` (1: +0x18 `0x00644560`, 2:
    /// `0x006445A0`).
    fn set_entry_param(&mut self, u: Self::Unit, i: u8, v: i32);
    /// `0x00625D10` / `0x00625D60` / `0x00625DB0`: maximum life, mana,
    /// stamina (stat 6, 8, 10).
    fn stat_max(&self, u: Self::Unit, s: u16) -> i32;

    // ---- items (§2.3, §2.5)
    /// `0x0064F060` (through `0x00645270` when unit +0xC8 bit 3 is set):
    /// the composit weapon class.
    fn composit_weapon_class(&self, u: Self::Unit) -> i32;
    /// `0x00623C60`: a player's hand class.
    fn hand_class(&self, u: Self::Unit) -> i32;
    /// `0x0062E6F0`: the item's type has a `shoots` value (itemtypes
    /// +0x0C).
    fn item_shoots(&self, item: Self::Item) -> bool;
    /// `0x006289F0`: items `stackable` (+0x132).
    fn item_stackable(&self, item: Self::Item) -> bool;
    /// Unit getter on an item (stats 70, 72, 125, 157).
    fn item_stat_of(&self, item: Self::Item, s: u16) -> i32;
    /// Unit set `0x00627260` on an item.
    fn set_item_stat(&mut self, item: Self::Item, s: u16, v: i32);
    /// `0x006295B0`: the maximum stack.
    fn item_max_stack(&self, item: Self::Item) -> i32;
    /// `0x00625E00`: the maximum durability.
    fn item_max_durability(&self, item: Self::Item) -> i32;
    /// `0x00558580(game, item)`: the quantity-replenish timer.
    fn quantity_timer(&mut self, item: Self::Item);
    /// Message 0x3E to the player's client (`0x005531C0`,
    /// `0x0053D130(client, item, 1, s, v, 0)`).
    fn send_item_stat(&mut self, u: Self::Unit, item: Self::Item, s: u16, v: i32);
    /// `0x00580310(game, unit)`: attack-mode cleanup (Open question 4).
    fn attack_cleanup(&mut self, u: Self::Unit);
    /// `0x00580380(game, unit)` (Open question 4).
    fn weapon_cleanup(&mut self, u: Self::Unit);

    // ---- other systems
    /// `0x0059FA30(game, record)`: creates the missile; false = none.
    fn spawn_missile(&mut self, req: MissileRequest<Self::Unit>) -> bool;
    /// `0x00646F20(unit)`: passive refresh.
    fn passive_refresh(&mut self, u: Self::Unit);
    /// `0x0056DE40(unit)`: the buff callback's passive refresh.
    fn buff_refresh(&mut self, u: Self::Unit);
    /// `0x00575900(game, unit)`: a player's skill resync (Open question 5).
    fn skill_resync(&mut self, u: Self::Unit);
    /// `0x00646D60(unit, entry)` after a passive state is switched on.
    fn passive_state_apply(&mut self, u: Self::Unit, e: &SkillEntry);
    /// `0x005B0E00(game, unit, AI control or none, k)` (`monsters/ai.md`
    /// §3.3).
    fn set_ai_state(&mut self, u: Self::Unit, k: i32);
    /// `0x005D2B60`: mana paid with the blood mana state (114)
    /// (`levels.md` Open question 8).
    fn blood_mana(&mut self, u: Self::Unit, cost: i32);
    /// `0x00571AA0`: queue message 0xA3 on the unit.
    fn queue_progressive(&mut self, u: Self::Unit, msg: ProgressiveMsg<Self::Unit>);
}

/// `srvst[index](game, unit, skill, level)` for a slot of
/// [`START_BODIES`]; `None` for any other slot.
pub fn run_start<W: BodyWorld>(
    w: &mut W,
    t: &SkillTables,
    ct: &CombatTables,
    index: u16,
    u: W::Unit,
    skill: i32,
    lvl: i32,
) -> Option<i32> {
    use starts::*;
    Some(match index {
        1 => attack(w, t, ct, u, lvl),
        2 => kick(w, t, ct, u, skill),
        3 => unsummon(w, u),
        4 | 65 => ammo(w, u),
        5 => jab(w, t, u, skill),
        15 => raise(w, ct, u),
        29 => sacrifice(w, t, ct, u, skill, lvl),
        32 => bash(w, t, ct, u, skill, lvl),
        33 => find_potion(w, ct, u),
        46 => andrial_spray(w, u),
        _ => return None,
    })
}

/// `srvdo[index](game, unit, skill, level)` for a slot of
/// [`DO_BODIES`]; `None` for any other slot. The do core's `charge`,
/// `item` and `aim` are not arguments of the bodies (§1).
pub fn run_do<W: BodyWorld>(
    w: &mut W,
    t: &SkillTables,
    ct: &CombatTables,
    index: u16,
    u: W::Unit,
    skill: i32,
    lvl: i32,
) -> Option<i32> {
    use dos::*;
    Some(match index {
        1 => attack(w, t, ct, u, skill, lvl),
        2 => melee_state(w, t, ct, u, skill, lvl),
        18 => buff(w, t, ct, u, skill, lvl),
        30 => curse(w, t, ct, u, skill, lvl),
        65 => aura(w, t, ct, u, skill, lvl),
        _ => return None,
    })
}

/// A do slot called from a body (the progressive finisher, §2.14 step
/// 4): a body here, else the provider's seam (with the core's
/// arguments off).
pub(crate) fn do_slot<W: BodyWorld>(
    w: &mut W,
    t: &SkillTables,
    ct: &CombatTables,
    index: u16,
    u: W::Unit,
    skill: i32,
    lvl: i32,
) -> i32 {
    match run_do(w, t, ct, index, u, skill, lvl) {
        Some(v) => v,
        None => w.srvdo(index, u, skill, lvl, false, false, false),
    }
}

/// The remove callback `cb` of a detached list (§2.8; ECX unit, EDX
/// state). Returns false for an id §2.8 does not specify (nothing
/// runs).
pub fn remove_callback<W: BodyWorld>(
    w: &mut W,
    t: &SkillTables,
    u: W::Unit,
    state: i32,
    cb: u32,
) -> bool {
    match cb {
        callback::DEFAULT => remove_default(w, u, state),
        callback::SELF_AURA => remove_self_aura(w, t, u, state),
        callback::BUFF => remove_buff(w, u, state),
        callback::AI_CURSE => remove_ai_curse(w, u, state),
        _ => return false,
    }
    true
}
