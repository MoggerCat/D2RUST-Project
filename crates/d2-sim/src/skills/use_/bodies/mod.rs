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

pub mod b3_lvl01;
pub mod b3_lvl06;
pub mod b3_lvl12;
pub mod b3_lvl18;
pub mod b3_lvl24;
pub mod b3_lvl30;
pub mod dos;
pub mod dos2;
pub mod effects;
#[cfg(test)]
pub(crate) mod fake;
pub mod helpers;
pub mod helpers2;
pub mod helpers3;
pub mod starts;
pub mod starts2;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests2;
#[cfg(test)]
mod tests3;

pub use effects::{BodyEffect, PathOp};
pub use helpers::*;
pub use helpers2::*;
pub use helpers3::*;

use super::UseWorld;
use crate::combat::{CombatTables, CombatWorld};
use crate::skills::{KickItems, SkillEntry, SkillTables};

/// `srvst` slots whose body reads and changes nothing ([`start`]).
pub const PURE_START: &[u16] = &[18];
/// `srvst` slots with a body over [`BodyWorld`] (`functions.tsv` status
/// `spec'd-here`: `bodies.md` §3, §7, `bodies-2.md`, `bodies-2b.md`).
pub const START_BODIES: &[u16] = &[
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
    27, 28, 29, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 46, 56, 57, 58, 65,
];
/// `srvdo` slots with a body over [`BodyWorld`] (status `spec'd-here`:
/// `bodies.md` §4, §8, `bodies-2.md`, `bodies-2b.md`).
pub const DO_BODIES: &[u16] = &[
    1, 2, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28,
    29, 30, 31, 32, 33, 34, 35, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 54, 55, 56, 57, 58, 59,
    60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82,
    114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 144, 150,
];

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
    /// Charge counter `0x005D3310` (`bodies.md` §6.10): state off.
    pub const CHARGE: u32 = 0x005D_3310;
    /// Inferno `0x005C8BF0` (`bodies.md` §6.16): state off, flags |= 0x40.
    pub const INFERNO: u32 = 0x005C_8BF0;
    /// Blade Fury `0x005D69B0` (`bodies-2b.md` §6.16): as Inferno's.
    pub const BLADE_FURY: u32 = 0x005D_69B0;
    /// Werewolf / Werebear `0x005C6C50` (`bodies.md` §8.15).
    pub const SHAPE: u32 = 0x005C_6C50;
    /// Whirlwind `0x005D8EF0` (`bodies-2b.md` §8.10).
    pub const WHIRLWIND: u32 = 0x005D_8EF0;
    /// Holy Freeze self list `0x005D0770` (`bodies-2b.md` §6.10).
    pub const HOLY_FREEZE: u32 = 0x005D_0770;
    /// Confuse `0x005C3DB0` (`bodies-2b.md` §6.6).
    pub const CONFUSE: u32 = 0x005C_3DB0;
    /// Attract `0x005C3B00` (`bodies-2b.md` §7.8).
    pub const ATTRACT: u32 = 0x005C_3B00;
    /// Conversion `0x005D01A0` (`bodies-2.md` §2.23).
    pub const CONVERSION: u32 = 0x005D_01A0;
    /// Mind Blast `0x005D7310` (`bodies-2.md` §2.23).
    pub const MIND_BLAST: u32 = 0x005D_7310;
    /// Every callback id the bodies specify.
    pub const ALL: [u32; 14] = [
        DEFAULT,
        SELF_AURA,
        BUFF,
        AI_CURSE,
        CHARGE,
        INFERNO,
        BLADE_FURY,
        SHAPE,
        WHIRLWIND,
        HOLY_FREEZE,
        CONFUSE,
        ATTRACT,
        CONVERSION,
        MIND_BLAST,
    ];
}

/// The itemstatcost fields the bodies read (`direct` +0x04 bit 4,
/// `updateanimrate` +0x04 bit 9, `maxstat` +0x32, `itemevent1–2` +0x48 /
/// +0x4A, `itemeventfunc1–2` +0x4C / +0x4E).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BodyStat {
    pub direct: bool,
    pub updateanimrate: bool,
    /// −1 when none.
    pub maxstat: i32,
    /// `itemevent1`, `itemevent2` (i16; ≤ 0 none).
    pub itemevent: [i32; 2],
    /// `itemeventfunc1`, `itemeventfunc2` (i16).
    pub itemeventfunc: [i32; 2],
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
    /// `monlvl.txt` rows (`monsters/init.md` §8.1).
    pub monlvl: Vec<d2_data::tables::Monlvl>,
    /// `pettype.txt` record count.
    pub pettype_count: i32,
}

impl BodyTables {
    /// From a loaded `.bin` set.
    pub fn from_bin(set: &d2_data::bin::BinSet) -> Result<Self, crate::skills::TablesError> {
        use crate::skills::TablesError;
        let table = |n: &'static str| set.table(n).ok_or(TablesError::Missing(n));
        let mut b = Self::from_tables(table("itemstatcost")?, table("states")?, table("overlay")?)?;
        b.monlvl = d2_data::tables::decode_all(table("monlvl")?)?;
        b.pettype_count = i32::try_from(table("pettype")?.count).unwrap_or(i32::MAX);
        Ok(b)
    }

    /// From the itemstatcost, states and overlay tables (no monlvl rows,
    /// no pet types).
    pub fn from_tables(
        itemstatcost: &d2_data::bin::BinTable,
        states: &d2_data::bin::BinTable,
        overlay: &d2_data::bin::BinTable,
    ) -> Result<Self, crate::skills::TablesError> {
        use d2_data::tables::{decode_all, Itemstatcost, States};
        let isc: Vec<Itemstatcost> = decode_all(itemstatcost)?;
        let states: Vec<States> = decode_all(states)?;
        let i16v = |v: u16| i32::from(v as i16);
        Ok(Self {
            stats: isc
                .iter()
                .map(|r| BodyStat {
                    direct: r.direct,
                    updateanimrate: r.updateanimrate,
                    maxstat: i16v(r.maxstat),
                    itemevent: [i16v(r.itemevent1), i16v(r.itemevent2)],
                    itemeventfunc: [i16v(r.itemeventfunc1), i16v(r.itemeventfunc2)],
                })
                .collect(),
            state_group: states.iter().map(|r| i32::from(r.group as i16)).collect(),
            state_aura: states.iter().map(|r| r.aura).collect(),
            overlay_count: i32::try_from(overlay.count).unwrap_or(i32::MAX),
            monlvl: Vec::new(),
            pettype_count: 0,
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

/// The missile parameter record the bodies fill (`missiles.md` §R2.1,
/// 0x5C bytes, zeroed first).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissileRequest<U> {
    /// +0x00.
    pub flags: u32,
    /// +0x04.
    pub owner: U,
    /// +0x08.
    pub origin: Option<U>,
    /// +0x0C target unit.
    pub target: Option<U>,
    /// +0x10.
    pub class: i32,
    /// +0x14, +0x18.
    pub x: i32,
    pub y: i32,
    /// +0x1C, +0x20.
    pub target_x: i32,
    pub target_y: i32,
    /// +0x28.
    pub velocity: i32,
    /// +0x2C, +0x30.
    pub skill: i32,
    pub level: i32,
    /// +0x34.
    pub loops: i32,
    /// +0x44 activate frames.
    pub activate: i32,
    /// +0x48.
    pub attack_bonus: i32,
    /// +0x4C.
    pub range: i32,
    /// +0x54 init callback (its 1.14d address) and +0x58 its argument.
    pub init: Option<(u32, u32)>,
}

impl<U> MissileRequest<U> {
    /// A zeroed record with the owner and the class.
    pub fn new(owner: U, class: i32) -> Self {
        Self {
            flags: 0,
            owner,
            origin: None,
            target: None,
            class,
            x: 0,
            y: 0,
            target_x: 0,
            target_y: 0,
            velocity: 0,
            skill: 0,
            level: 0,
            loops: 0,
            activate: 0,
            attack_bonus: 0,
            range: 0,
            init: None,
        }
    }
}

/// Missile init callbacks the bodies pass (+0x54).
pub mod init_cb {
    /// Jitter `0x005C9290` (`bodies-2.md` §2.3).
    pub const JITTER: u32 = 0x005C_9290;
    /// `damagepercent(25)` += argument `0x005DB6A0` (`bodies.md` §8.19).
    pub const DAMAGE_PERCENT: u32 = 0x005D_B6A0;
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
pub trait BodyWorld: UseWorld + KickItems {
    /// A stat list handle.
    type List: Copy + Eq + std::fmt::Debug;
    /// A room handle (`drlg/rooms.md`).
    type Room: Copy + Eq + std::fmt::Debug;
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
    /// `0x0059FA30(game, record)`: creates the missile.
    fn spawn_missile(&mut self, req: MissileRequest<Self::Unit>) -> Option<Self::Unit>;
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

    // ---- batch 2 and 3 (`bodies.md` §6–§8, `bodies-2.md`, `bodies-2b.md`)
    /// A call into another system whose result the bodies do not read.
    fn effect(&mut self, e: BodyEffect<Self::Unit, Self::Item, Self::Room>);
    /// A path operation on the unit's path; returns the compute result
    /// for [`PathOp::Compute`], else 0.
    fn path_op(&mut self, u: Self::Unit, op: PathOp<Self::Unit>) -> i32;

    // ---- data
    /// monlvl rows (`monsters/init.md` §8.1).
    fn monlvl(&self) -> &[d2_data::tables::Monlvl];
    /// `pettype.txt` record count.
    fn pettype_count(&self) -> i32;
    /// The L-flag: game +0x6A ≠ 0 or game +0x74 ≠ 0 (`monsters/init.md`
    /// §8.1).
    fn l_flag(&self) -> bool;

    // ---- units
    /// Unit +0xC8.
    fn unit_c8(&self, u: Self::Unit) -> u32;
    fn set_unit_c8(&mut self, u: Self::Unit, v: u32);
    /// Unit +0x44 (animation frame, 8.8).
    fn anim_frame(&self, u: Self::Unit) -> i32;
    /// Frame event index (unit +0x38 bits 8+, `0x006212C0` set).
    fn frame_event_index(&self, u: Self::Unit) -> i32;
    fn set_frame_event_index(&mut self, u: Self::Unit, i: i32);
    /// Unit +0x48 (frame count, 8.8).
    fn frame_count(&self, u: Self::Unit) -> i32;
    fn set_frame_count(&mut self, u: Self::Unit, v: i32);
    /// Unit +0xD0 (target-list slot; 11 = none).
    fn node_slot(&self, u: Self::Unit) -> i32;
    /// Unit +0x5C: the unit has a stat holder.
    fn has_stat_holder(&self, u: Self::Unit) -> bool;
    /// `0x00620510`: the unit size.
    fn unit_size(&self, u: Self::Unit) -> i32;
    /// The minion spawn class of a monster's AI control (+0x3C,
    /// `0x0058F710`); `None` without monster data or control.
    fn minion_spawn_class(&self, u: Self::Unit) -> Option<i32>;
    /// The owner's linked unit (GUID `0x00554070`, looked up as a monster
    /// `0x00552F60`).
    fn linked_unit(&self, u: Self::Unit) -> Option<Self::Unit>;
    /// `0x00553010(game, unit)`: the killer the death delay names.
    fn killer_of(&self, u: Self::Unit) -> Option<Self::Unit>;
    /// `0x0058F090`: a monster's minion owner record (AI control +0x2C
    /// GUID, +0x30 type); `None` for GUID −1.
    fn minion_owner_ident(&self, u: Self::Unit) -> Option<(u32, u32)>;
    /// `0x006272B0`: unit stat += v.
    fn add_stat(&mut self, u: Self::Unit, s: u16, v: i32);
    /// `0x00627030`: list stat += v.
    fn list_add(&mut self, l: Self::List, s: i32, v: i32);
    /// `0x00627340`: every stat of the list removed.
    fn list_clear(&mut self, l: Self::List);
    /// `0x005C0BE0`: the unit has a handler with this key and skill field.
    fn has_handler(&self, u: Self::Unit, key_type: i32, key: i32, skill: i32) -> bool;

    // ---- skill entries (the skill list's owner; `use.md` §5.2)
    /// Param `i` (1…4: +0x18, +0x1C, +0x20, +0x24) of the unit's entry
    /// `e`.
    fn entry_param(&self, u: Self::Unit, e: &SkillEntry, i: u8) -> i32;
    fn set_entry_param_of(&mut self, u: Self::Unit, e: &SkillEntry, i: u8, v: i32);
    /// Entry flags +0x0C (`0x006446A0` / `0x00644660`).
    fn entry_flags(&self, u: Self::Unit, e: &SkillEntry) -> u32;
    fn set_entry_flags(&mut self, u: Self::Unit, e: &SkillEntry, f: u32);
    /// Entry mode +0x08 := m (`0x00644340`).
    fn set_entry_mode(&mut self, u: Self::Unit, e: &SkillEntry, m: u32);
    /// `0x00645270`: a mode through the disguise remap.
    fn disguise_mode(&self, u: Self::Unit, m: u32) -> u32;
    /// The used skill's sequence frame records (6 bytes each,
    /// `0x006633B0` / `0x006633F0`); `None` without one.
    fn skill_sequence(&self, u: Self::Unit) -> Option<Vec<[u8; 6]>>;

    // ---- animation (`sim/units.md` §4.2 variants)
    /// `0x0056E210(unit, p)` → `0x00553B10(game, unit, p)` (rewind(p)).
    fn anim_rewind(&mut self, u: Self::Unit, p: i32);
    /// `0x00553C70(game, unit, v)`.
    fn anim_restart(&mut self, u: Self::Unit, v: i32);
    /// `0x00553DC0(game, unit, f)`: the animation from frame f.
    fn anim_from(&mut self, u: Self::Unit, f: i32);

    // ---- rooms (`drlg/rooms.md`, `sim/path-placement.md`)
    /// `0x00620BB0`: the unit's room.
    fn unit_room(&self, u: Self::Unit) -> Option<Self::Room>;
    /// `0x00463740`: the room containing (x, y), searched from `from`.
    fn room_at(&self, from: Self::Room, x: i32, y: i32) -> Option<Self::Room>;
    /// `0x0061AB00`.
    fn room_in_town(&self, r: Self::Room) -> bool;
    /// The act of the room's level (`0x0061A1B0`, `0x006427F0`).
    fn room_act(&self, r: Self::Room) -> i32;
    /// The room level's `Teleport` (levels +0x04); `None` without a
    /// level record.
    fn room_teleport(&self, r: Self::Room) -> Option<i32>;
    /// `0x0064E7B0(room, &(x, y), size, mask, fallback)`: the free point
    /// and its room.
    fn free_point(
        &mut self,
        r: Self::Room,
        at: (i32, i32),
        size: i32,
        mask: u32,
        fallback: bool,
    ) -> Option<(Self::Room, (i32, i32))>;
    /// `0x0064D910(room, x, y, u's path pattern, mask)` ≠ 0.
    fn pattern_collides(&self, r: Self::Room, at: (i32, i32), u: Self::Unit, mask: u32) -> bool;
    /// `0x0064D800(room, x, y, size, size, mask)` ≠ 0.
    fn box_collides(&self, r: Self::Room, at: (i32, i32), size: i32, mask: u32) -> bool;
    /// `0x0064E260(room, from, to, mask)` ≠ 0 (`0x00645910` / `0x006229F0`
    /// for a unit's line).
    fn line_blocked(&self, r: Self::Room, from: (i32, i32), to: (i32, i32), mask: u32) -> bool;
    /// `0x00554EA0(game, unit, room, x, y, 0, 0)` (`sim/path-placement.md`
    /// §10): true when placed.
    fn place_unit(&mut self, u: Self::Unit, r: Option<Self::Room>, at: (i32, i32)) -> bool;

    // ---- paths (`sim/pathing.md`)
    /// The unit has a path (+0x2C).
    fn has_path(&self, u: Self::Unit) -> bool;
    /// `0x006487D0`: the path's point count.
    fn path_point_count(&self, u: Self::Unit) -> i32;
    /// The path's last point.
    fn path_last_point(&self, u: Self::Unit) -> (i32, i32);
    /// The path's target point (`0x00648A00`, `0x00648A10`).
    fn path_target_point(&self, u: Self::Unit) -> (i32, i32);

    // ---- monsters
    /// `0x005B2F20(game, room, x, y, class, mode, spread, 0x42)`
    /// (`monsters/init.md` §1).
    fn create_monster(
        &mut self,
        r: Self::Room,
        at: (i32, i32),
        class: i32,
        mode: i32,
        spread: i32,
    ) -> Option<Self::Unit>;

    /// Monster mode request `0x005A7E60(m, mode, &req)` then
    /// `0x005A7C20(game, &req, 1)`, at `target` when given; the second
    /// call's result.
    fn mode_request(&mut self, m: Self::Unit, mode: i32, target: Option<Self::Unit>) -> i32;

    // ---- items
    /// The item handle of an item unit (type 4); `None` for other units.
    fn as_item(&self, u: Self::Unit) -> Option<Self::Item>;
    /// `0x00535060`: the player is busy (`items/inventory.md` §5.2).
    fn inventory_busy(&self, u: Self::Unit) -> bool;
    /// The unit has an inventory (+0x60).
    fn has_inventory(&self, u: Self::Unit) -> bool;
    /// `0x0063BEF0`: the weapon in use.
    fn weapon_in_use(&self, u: Self::Unit) -> Option<Self::Item>;
    /// `0x00627D40`: the item's body location.
    fn body_loc(&self, i: Self::Item) -> i32;
    /// `0x0062A4E0`: usable (not broken, item flag 0x4000 clear).
    fn item_usable(&self, i: Self::Item) -> bool;
    /// `0x00625820(item, 0)`: active on its owner.
    fn item_active(&self, i: Self::Item) -> bool;
    /// `0x00629930`: the item has durability (breakable).
    fn item_breakable(&self, i: Self::Item) -> bool;
    /// `0x0063C8F0(inventory, &S)`: the equipped shield.
    fn shield(&self, u: Self::Unit) -> Option<Self::Item>;
    /// armor `mindam` / `maxdam` (+0xFE, +0xFF) of the item's class;
    /// `None` without an item record.
    fn shield_damage(&self, i: Self::Item) -> Option<(i32, i32)>;
    /// weapons `missiletype` (+0xFA) of the item's class.
    fn item_missile_type(&self, i: Self::Item) -> i32;
    /// Iron Golem start (`bodies-2b.md` §7.11): an item unit in mode 3
    /// whose class has `bitfield1` bit 1, with item flag 0x10, not active
    /// on a unit (Open question 9).
    fn golem_item(&self, t: Self::Unit) -> bool;
    /// `0x0062EA80`: an item unit's first allowed body location.
    fn item_first_loc(&self, t: Self::Unit) -> i32;
    /// `0x0063C050(inventory, 6)` and `(inventory, 5)` both of item type
    /// 46 (`bodies-2.md` Open question 10).
    fn two_melee_weapons(&self, u: Self::Unit) -> bool;
    /// `0x0062A710(unit, W)`: attack frames (AnimData); `None` = fatal.
    fn attack_frames(&self, u: Self::Unit, w: Self::Item) -> Option<i32>;
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
    use b3_lvl01 as l1;
    use b3_lvl06 as l6;
    use b3_lvl12 as l12;
    use b3_lvl18 as l18;
    use b3_lvl24 as l24;
    use b3_lvl30 as l30;
    use starts::*;
    use starts2 as s2;
    Some(match index {
        1 => attack(w, t, ct, u, lvl),
        2 => kick(w, t, ct, u, skill),
        3 => unsummon(w, u),
        4 | 65 => ammo(w, u),
        5 => jab(w, t, u, skill),
        6 => s2::power_strike(w, t, ct, u, skill, lvl),
        7 => l12::impale(w, t, ct, u, skill, lvl),
        8 => l24::strafe_start(w, t, ct, u, skill, lvl),
        9 => l24::fend_start(w, t, ct, u, skill, lvl),
        10 => l30::lightning_strike_start(w, t, ct, u, skill, lvl),
        11 => s2::inferno(w, t, ct, u, skill, lvl),
        12 => s2::telekinesis(w, t, u, skill, lvl),
        13 => l24::thunder_storm_start(w, t, u, skill),
        14 => l30::hydra_start(w, u),
        15 => raise(w, ct, u),
        16 => l6::poison_dagger_start(w, t, ct, u, skill, lvl),
        17 => s2::corpse_explosion(w, ct, u),
        // `bodies-2b.md` §7.7: the pure body of [`start`].
        18 => 1,
        19 => l24::bone_prison_start(w, u),
        20 => l24::iron_golem_start(w, u),
        21 => l30::revive_start(w, ct, u),
        22 => l1::psychic_hammer_start(w, u),
        23 => s2::charge_strike(w, u),
        24 => l1::dragon_talon_start(w, t, ct, u, skill, lvl),
        25 => l6::dragon_claw_start(w, u),
        26 => l18::blade_fury_start(w, t, ct, u, skill, lvl),
        27 => l18::dragon_tail_start(w, t, ct, u, skill, lvl),
        28 => l30::blade_shield_start(w, t, ct, u, skill, lvl),
        29 => sacrifice(w, t, ct, u, skill, lvl),
        31 => l12::charge_start(w, t, ct, u, skill),
        32 => bash(w, t, ct, u, skill, lvl),
        33 => find_potion(w, ct, u),
        34 => l12::find_item_start(w, ct, u),
        35 => l18::vengeance(w, t, ct, u, skill, lvl),
        36 => l24::holy_shield_start(w, u),
        37 => s2::zeal(w, t, ct, u, skill, lvl),
        38 => l30::whirlwind_start(w, t, ct, u, skill, lvl),
        39 => l30::berserk(w, t, ct, u, skill, lvl),
        40 => l6::leap_start(w, t, ct, u, skill, lvl),
        41 => l18::leap_attack_start(w, u, skill),
        46 => andrial_spray(w, u),
        56 => s2::feral_rage(w, t, ct, u, skill, lvl),
        57 => l18::rabies_start(w, t, ct, u, skill, lvl),
        58 => l18::fire_claws(w, t, ct, u, skill, lvl),
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
    use b3_lvl01 as l1;
    use b3_lvl06 as l6;
    use b3_lvl12 as l12;
    use b3_lvl18 as l18;
    use b3_lvl24 as l24;
    use b3_lvl30 as l30;
    use dos::*;
    use dos2 as d2;
    Some(match index {
        1 => attack(w, t, ct, u, skill, lvl),
        2 => melee_state(w, t, ct, u, skill, lvl),
        6 => d2::inner_sight(w, t, ct, u, skill, lvl),
        7 => l1::jab(w, t, ct, u, skill, lvl),
        8 => d2::multiple_shot(w, t, u, skill, lvl),
        9 => l24::frenzy(w, t, ct, u, skill, lvl),
        10 => d2::guided_arrow(w, t, u, skill, lvl),
        11 => l18::charged_strike(w, t, ct, u, skill, lvl),
        12 => l24::strafe(w, t, ct, u, skill, lvl),
        13 => d2::fend(w, t, ct, u, skill, lvl),
        14 => l30::lightning_strike(w, t, ct, u, skill, lvl),
        15 => l24::dopplezon(w, t, ct, u, skill, lvl),
        16 => l30::valkyrie(w, t, ct, u, skill, lvl),
        17 => l1::charged_bolt(w, t, u, skill, lvl),
        18 => buff(w, t, ct, u, skill, lvl),
        19 => d2::inferno_cast(w, t, ct, u, skill, lvl),
        20 => l6::static_field(w, t, ct, u, skill, lvl),
        21 => l6::telekinesis(w, t, ct, u, skill, lvl),
        22 => d2::nova(w, t, u, skill, lvl),
        23 => d2::blaze(w, t, ct, u, skill, lvl),
        24 => l18::fire_wall(w, t, u, skill, lvl),
        25 => l18::enchant(w, t, ct, u, skill, lvl),
        26 => l18::chain_lightning(w, t, u, skill, lvl),
        27 => l18::teleport(w, u),
        28 => d2::meteor(w, t, u, skill, lvl),
        29 => l24::thunder_storm(w, t, ct, u, skill, lvl),
        30 => curse(w, t, ct, u, skill, lvl),
        31 => d2::raise_skeleton(w, t, ct, u, skill, lvl),
        32 => l6::poison_dagger(w, ct, u),
        33 => l1::psychic_hammer(w, t, ct, u, skill, lvl),
        34 => d2::charge_hit(w, t, ct, u, skill, lvl),
        35 => d2::claws(w, t, ct, u, skill, lvl),
        42 => l1::dragon_talon(w, t, ct, u, skill, lvl),
        43 => l6::shock_field(w, t, u, skill, lvl),
        44 => l6::blade_sentinel(w, t, ct, u, skill, lvl),
        45 => d2::sentry_do(w, t, ct, u, skill, lvl),
        46 => l6::dragon_claw(w, t, ct, u, skill, lvl),
        47 => l12::cloak(w, t, ct, u, skill, lvl),
        48 => l18::blade_fury(w, t, ct, u, skill, lvl),
        49 => d2::shadow(w, t, ct, u, skill, lvl),
        50 => l18::dragon_tail(w, t, ct, u, skill, lvl),
        51 => l24::mind_blast(w, t, ct, u, skill, lvl),
        52 => l24::dragon_flight(w, t, ct, u, skill, lvl),
        54 => l30::blade_shield(w, t, ct, u, skill, lvl),
        55 => l6::corpse_explosion(w, t, ct, u, skill, lvl),
        56 => d2::golem(w, t, ct, u, skill, lvl),
        57 => l24::iron_golem(w, t, ct, u, skill, lvl),
        58 => l30::revive(w, t, ct, u, skill, lvl),
        59 => l24::attract(w, t, ct, u, skill, lvl),
        60 => l12::bone_wall(w, t, ct, u, skill, lvl),
        61 => l18::confuse(w, t, ct, u, skill, lvl),
        62 => l24::bone_prison(w, t, ct, u, skill, lvl),
        63 => l18::poison_explosion(w, t, ct, u, skill, lvl),
        64 => l1::sacrifice(w, t, ct, u, skill, lvl),
        65 => aura(w, t, ct, u, skill, lvl),
        66 => d2::damage_aura(w, t, ct, u, skill, lvl, false),
        67 => l12::charge(w, t, ct, u, skill, lvl),
        68 => d2::shout(w, t, u, skill, lvl),
        69 => l1::find_potion(w, t, ct, u, skill, lvl),
        70 => l6::double_swing(w, t, ct, u, skill, lvl),
        71 => l6::taunt(w, t, ct, u, skill, lvl),
        72 => l12::find_item(w, t, ct, u, skill, lvl),
        73 => l18::blessed_hammer(w, t, u, skill, lvl),
        74 => l12::double_throw(w, t, u, skill, lvl),
        75 => l24::grim_ward(w, t, ct, u, skill, lvl),
        76 => l30::whirlwind(w, t, ct, u, skill, lvl),
        77 => l6::leap(w, t, ct, u, skill, lvl),
        78 => l18::leap_attack(w, t, ct, u, skill, lvl),
        79 => l24::conversion_do(w, t, ct, u, skill, lvl),
        80 => l30::fist_of_heavens(w, t, u, skill, lvl),
        81 => d2::damage_aura(w, t, ct, u, skill, lvl, true),
        82 => l30::redemption(w, t, ct, u, skill, lvl),
        114 => l1::raven(w, t, ct, u, skill, lvl),
        115 => d2::vines(w, t, ct, u, skill, lvl),
        116 => d2::shape_shift(w, t, u, skill, lvl),
        117 => l1::firestorm(w, t, u, skill, lvl),
        118 => d2::twister(w, t, u, skill, lvl),
        119 => d2::druid_summon(w, t, ct, u, skill, lvl),
        120 => d2::feral_rage(w, t, ct, u, skill, lvl),
        121 => l18::rabies(w, t, ct, u, skill, lvl),
        122 => l24::hunger(w, t, ct, u, skill, lvl),
        123 => l24::volcano(w, t, u, skill, lvl),
        124 => d2::armageddon(w, t, u, skill, lvl),
        144 => l30::hydra(w, t, ct, u, skill, lvl),
        150 => l1::smite(w, t, ct, u, skill, lvl),
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

/// The remove callback `cb` of a detached list `l` (ECX unit, EDX state,
/// stack list; `bodies.md` §2.8 and the batch 2 / 3 callbacks). Returns
/// false for an id the bodies do not specify (nothing runs).
pub fn remove_callback<W: BodyWorld>(
    w: &mut W,
    t: &SkillTables,
    u: W::Unit,
    state: i32,
    cb: u32,
    l: W::List,
) -> bool {
    match cb {
        callback::DEFAULT => remove_default(w, u, state),
        callback::SELF_AURA => remove_self_aura(w, t, u, state),
        callback::BUFF => remove_buff(w, u, state),
        callback::AI_CURSE => remove_ai_curse(w, u, state),
        callback::CHARGE => w.state_on(u, state, false),
        callback::INFERNO | callback::BLADE_FURY => {
            w.state_on(u, state, false);
            flags_or(w, u, FLAG_40);
        }
        callback::SHAPE => helpers3::remove_shape(w, u, state, l),
        callback::WHIRLWIND => remove_buff(w, u, state),
        callback::HOLY_FREEZE => helpers3::remove_holy_freeze(w, u, state),
        callback::CONFUSE | callback::ATTRACT => helpers3::remove_alignment(w, u, state),
        callback::CONVERSION => helpers3::remove_conversion(w, u, state, false),
        callback::MIND_BLAST => helpers3::remove_conversion(w, u, state, true),
        _ => return false,
    }
    true
}
