// Spec: specs/items/inventory.md §4.7 (two-handed, ammo type), §4.8 (level requirement); specs/items/inventory-moves.md §7.19 step 3, §7.20 (spell)
// Spec: specs/items/properties.md §9 (socket fillers), §10 (runewords); specs/items/generation.md §9 step 6 (replenish timers)
//! The item queries and socket effects that the inventory seams answer
//! from d2-sim's own state: the item tables, the item store, the stat
//! lists, the unit records and the inventory state. The [`InvRest`]
//! methods of the same names are no longer asked by [`InvDesk`].
//! Nothing here decides a rule: the rules are `items::inventory`'s and
//! `items::props`'s; this file gathers their inputs.

use super::{InvDesk, InvError, InvRest};
use crate::items::affixes::affix;
use crate::items::inventory::{level_requirement, AffixReq, LevelReqItem, LevelReqUnit};
use crate::items::{props, replenish_timer, stat};
use crate::stats::{key_layer, key_stat};
use crate::units::lifecycle::LifecycleHooks;
use crate::units::record::flags2;
use crate::units::{UnitId, UnitType};

/// Stat 92 `item_levelreq` (§4.8).
pub const STAT_LEVELREQ: u16 = 92;
/// Stat 97 `item_nonclassskill` (§4.8).
pub const STAT_NONCLASSSKILL: u16 = 97;
/// Stat 107 `item_singleskill` (§4.8).
pub const STAT_SINGLESKILL: u16 = 107;
/// Entries `0x006261D0` copies per stat (§4.8: at most 64).
const MAX_ENTRIES: usize = 64;
/// Item timer event 3: replenish (`units.md` §6, `generation.md` §9 step 6).
const EVENT_REPLENISH: u32 = 3;

impl<H: LifecycleHooks, R: InvRest + ?Sized> InvDesk<'_, '_, H, R> {
    /// The items class of an item unit (its item data's record).
    fn class_of(&self, u: UnitId) -> Option<usize> {
        self.econ.items.get(u).map(|i| i.record)
    }

    /// Two-handed `0x006289C0`: items `2handed` ≠ 0 (§4.7 profile "2h").
    pub fn is_two_handed(&self, u: UnitId) -> bool {
        self.class_of(u)
            .and_then(|r| self.tables.item(r))
            .is_some_and(|r| r.twohanded != 0)
    }

    /// Ammo type `0x0062E6F0`: the primary type's itemtypes `shoots`
    /// (§4.7 rule 2), as an itemtypes row (−1: the link's miss value, which
    /// no equivalence test matches). `None`: no item record or type row.
    pub fn ammo_of(&self, u: UnitId) -> Option<i16> {
        let r = self.class_of(u)?;
        self.econ.tables.itype_of(r).map(|t| t.shoots as i16)
    }

    /// Book / scroll spell `0x00627F80(I, 0)`: item data +0x3E, suffix slot
    /// 0 (`skills/bodies-3.md` "+0x3E spell index"; `inventory-moves.md`
    /// §7.20). 0 without item data.
    pub fn spell_of(&self, u: UnitId) -> i32 {
        self.econ.items.get(u).map_or(0, |i| i32::from(i.suffix[0]))
    }

    /// The unit §4.8 computes for: class (unit +4), a player
    /// (`0x0044BE50` = 0), unit flag 0x2000000 (`0x00463720`).
    pub fn level_req_unit(&self, unit: UnitId) -> Option<LevelReqUnit> {
        let r = self.econ.units.get(unit)?;
        Some(LevelReqUnit {
            class: r.class,
            player: r.ty == UnitType::Player,
            expansion: r.flags2 & flags2::EXPANSION != 0,
        })
    }

    /// The entries of `stat` in the item's extended stat list
    /// (`0x006261D0`, at most 64, list order) as (layer, value); a plain
    /// list or none gives none (§4.8).
    fn extended_entries(&self, u: UnitId, s: u16) -> Vec<(u16, i32)> {
        let stats = &*self.econ.stats;
        let Some(l) = stats.unit_list(u).filter(|&l| stats.is_extended(l)) else {
            return Vec::new();
        };
        stats
            .full_entries(l)
            .into_iter()
            .filter(|&(k, _)| key_stat(k) == s)
            .map(|(k, v)| (key_layer(k), v))
            .take(MAX_ENTRIES)
            .collect()
    }

    /// Everything §4.8 reads of `u`: affix rows of item data +0x36,
    /// +0x38 + 2i, +0x3E + 2i (id 0 or unknown: none, `0x00633EE0`), the
    /// set / unique `lvl req` of the file index, the version, items
    /// `levelreq`, the items of the item's own inventory (recursive), the
    /// skills of stats 107 and 97 (skill ids ≥ the skills count skipped)
    /// and stat 92. No item data → quality 2 and nothing else.
    pub fn level_req_item(&self, u: UnitId) -> LevelReqItem {
        let Some(it) = self.econ.items.get(u) else {
            return LevelReqItem {
                quality: 2,
                ..LevelReqItem::default()
            };
        };
        let t = self.econ.tables;
        let aff = |id: u16| {
            affix(t, id).map(|a| AffixReq {
                levelreq: a.levelreq,
                class: a.class,
                classlevelreq: a.classlevelreq,
            })
        };
        let row = usize::try_from(it.file_index).ok();
        let skill = |layer: u16| t.skills.get(usize::from(layer));
        LevelReqItem {
            quality: it.quality,
            automagic: aff(it.auto_affix),
            prefixes: it.prefix.map(aff),
            suffixes: it.suffix.map(aff),
            set_lvlreq: row.and_then(|i| t.setitems.get(i)).map_or(0, |s| s.lvl_req),
            unique_lvlreq: row.and_then(|i| t.uniques.get(i)).map(|r| r.lvl_req),
            version: it.format,
            class_levelreq: self
                .tables
                .item(it.record)
                .map_or(0, |r| i32::from(r.levelreq)),
            fillers: self
                .state
                .inventories
                .get(&u)
                .map(|inv| {
                    inv.items()
                        .iter()
                        .map(|&f| self.level_req_item(f))
                        .collect()
                })
                .unwrap_or_default(),
            single_skills: self
                .extended_entries(u, STAT_SINGLESKILL)
                .into_iter()
                .filter_map(|(l, _)| skill(l).map(|s| s.reqlevel))
                .collect(),
            nonclass_skills: self
                .extended_entries(u, STAT_NONCLASSSKILL)
                .into_iter()
                .filter_map(|(l, _)| skill(l).map(|s| (s.reqlevel, i32::from(s.charclass))))
                .collect(),
            stat_levelreq: self.econ.stats.unit_total(u, STAT_LEVELREQ, 0),
        }
    }

    /// `0x0062B5B0` (§4.8) of `item` for `unit` (none: no unit given, or
    /// no unit record).
    pub fn item_level_requirement(&self, item: UnitId, unit: Option<UnitId>) -> i32 {
        let who = unit.and_then(|u| self.level_req_unit(u));
        level_requirement(&self.level_req_item(item), who.as_ref())
    }

    /// `properties.md` §9 on a filler just linked into `target`: the
    /// filler's gems row and the target's apply type (`0x00629A40`: items
    /// `gemapplytype`); the properties land in the filler's own list.
    pub fn apply_filler_properties(&mut self, filler: UnitId, target: UnitId) {
        let apply = self
            .class_of(target)
            .and_then(|r| self.econ.tables.item(r))
            .map_or(0, |r| r.gemapplytype);
        if let Err(e) = self.econ.with_item(filler, |s| {
            props::apply_socket_filler(s.tables, s.item, apply)
        }) {
            self.state.errors.push(InvError::Economy(e));
        }
        self.sync_in();
    }

    /// The runeword step of `inventory-moves.md` §7.19 step 3 on `target`:
    /// the record of `properties.md` §10.1 on the target's inventory, then
    /// §10.2 (`0x006600A0`: a `server` row needs game +0x74; a target that
    /// already has a runeword list is left as is), then the replenish
    /// timers (`generation.md` §9 step 6: event 3 when not scheduled).
    /// True when the runeword properties ran.
    // TODO(spec: inventory-moves.md §7.19 step 3 vs properties.md §10.2):
    // the first gates a `server` row on "an expansion game", the second on
    // game +0x74 (ladder); §10.2 (the owner) is followed. The recharge
    // `0x0055FE80` that §7.19 lists after the timers has no written body.
    pub fn activate_runeword_on(&mut self, target: UnitId) -> bool {
        let Some(it) = self.econ.items.get(target) else {
            return false;
        };
        let fillers: Vec<usize> = self
            .state
            .inventories
            .get(&target)
            .map(|inv| {
                inv.items()
                    .iter()
                    .filter_map(|&f| self.class_of(f))
                    .collect()
            })
            .unwrap_or_default();
        let sockets = self.econ.stats.unit_total(target, stat::NUMSOCKETS, 0) as u8;
        let Some(row) =
            props::runeword_row(self.econ.tables, it.record, it.quality, sockets, &fillers)
        else {
            return false;
        };
        let ladder = self.econ.fields.ladder;
        let scheduled = self
            .econ
            .game
            .timers
            .unit_timers(target)
            .into_iter()
            .any(|id| {
                self.econ
                    .game
                    .timers
                    .event(id)
                    .is_some_and(|(e, _, _)| u32::from(e) == EVENT_REPLENISH)
            });
        let frame = self.econ.game.frame as u32;
        let ran = self.econ.with_item(target, |s| {
            let ran = props::activate_runeword(s.tables, s.item, row, ladder);
            (
                ran,
                ran.then(|| replenish_timer(&s.item.stats, scheduled, frame))
                    .flatten(),
            )
        });
        self.sync_in();
        match ran {
            Ok((ran, at)) => {
                if let Some(f) = at {
                    // TODO(units.md §6 row 3): the event's arguments are
                    // not written; scheduled with 0, 0 (as at creation).
                    if let Err(e) =
                        self.econ
                            .game
                            .schedule_event(target, EVENT_REPLENISH, f as i32, None, 0, 0)
                    {
                        self.state.errors.push(InvError::Economy(e.into()));
                    }
                }
                ran
            }
            Err(e) => {
                self.state.errors.push(InvError::Economy(e));
                false
            }
        }
    }
}
