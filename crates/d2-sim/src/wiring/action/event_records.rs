// Spec: specs/sim/intents-events.md §7.9 rule 2 (pending event records), §7.3 rule 2 step 3, §7.5 step 2
//! The pending event records of a unit (unit +0xEC, `0x00571CD0`): the
//! writers append a record and queue the unit for update
//! (`0x0064C040`); the per-client update sends each record to the
//! client's player in list order (§7.3 rule 2 step 3, objects and
//! players as well); the room clean-up frees them (§7.5 step 2), so every
//! client in game gets the tick's records once.
//!
//! Writers wired here: 0xA3 (`0x00571AA0`, the progressive-charge
//! bodies' `queue_progressive`), 0xA4 (`0x00571C00`, the AI's class
//! preload), 0xA5 (`0x00571B70`, the landing message of
//! `skills/bodies-2.md` §2.13), 0xAB (`0x00571A10`, the life-fraction
//! update of `stat-lists.md` §10.1) and 0x99 / 0x9A (`0x005717C0` /
//! `0x00571840`, the item cast of `combat/events.md` §3). The 0x9E
//! hireling records are sent at once by `world::hirelings::level` (§7.9
//! rule 2 is the same bytes); 0x23 has no d2rs writer yet.

use super::reaction::UNIT_FLAG_SOFT_HIT;
use crate::game::Game;
use crate::units::UnitId;

/// One pending record, its fields resolved when written (the record
/// bytes the senders pass through).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventRecord {
    /// 0xA3 (`0x00571AA0`, `0x0053C0E0`, 24 bytes): the record
    /// {n, k, lvl, unit, T, r, 0} of `skills/bodies.md` §2.14 step 5
    /// (`bodies-2.md` §2.21: x = r, y = 0).
    Progressive {
        charges: u8,
        skill: u16,
        level: u16,
        unit: (u8, u32),
        target: (u8, u32),
        x: u32,
        y: u32,
    },
    /// 0xA4 (`0x00571C00`, `0x0053E1A0`, 3 bytes): the class u16@1.
    Preload { class: u16 },
    /// 0xA5 (`0x00571B70`, `0x0053C190`, 8 bytes): the unit's type and
    /// GUID (`skills/bodies-2.md` §2.13: "unit type, GUID, skill"), the
    /// skill u16@6.
    // PROVISIONAL (REC-411): the unit of the message is the unit the record
    // sits on (the writer's unit is the landing caster); settled by a 1.14d
    // recording of a Leap landing (the 0xA5 bytes).
    Landing { skill: u16 },
    /// 0xAB (`0x00571A10`, `0x0053C150`, 7 bytes): the life fraction f
    /// (of 128, `stats.md` §9.3) of the unit the record sits on.
    NpcHeal { life: u8 },
    /// 0x99 / 0x9A (`0x005717C0`, `0x0053D530` with flag 1,
    /// `sim/intents-events.md` §3.5 rule 5): an item cast at a unit. The
    /// bytes depend on the receiver's rooms (16-byte 0x99, or the
    /// 17-byte 0x9A at the target's path target), see
    /// [`View::send_event_records`].
    CastOnUnit {
        skill: u16,
        level: u8,
        target: (u8, u32),
        w: u16,
    },
    /// 0x9A (`0x00571840`, `0x0053D4D0`, 17 bytes): an item cast at a
    /// point.
    CastOnPoint {
        skill: u32,
        level: u8,
        x: u16,
        y: u16,
        w: u16,
    },
}

impl EventRecord {
    /// The S→C message of the record (`0x00571CD0`'s sender per id) of
    /// the unit `(unit type, GUID)` the record sits on. A
    /// [`EventRecord::CastOnUnit`] gives its 16-byte form here (the
    /// receiver-dependent choice is [`View::send_event_records`]'s).
    pub fn message(&self, unit: (u8, u32)) -> Vec<u8> {
        match *self {
            EventRecord::Landing { skill } => landing(unit.0, unit.1, skill).to_vec(),
            EventRecord::NpcHeal { life } => npc_heal(unit.0, unit.1, life).to_vec(),
            EventRecord::CastOnUnit {
                skill,
                level,
                target,
                w,
            } => skill_event_on_unit(unit.0, unit.1, skill, level, target.0, target.1, w).to_vec(),
            EventRecord::CastOnPoint {
                skill,
                level,
                x,
                y,
                w,
            } => skill_event_on_point(unit.0, unit.1, skill, level, x, y, w).to_vec(),
            EventRecord::Progressive {
                charges,
                skill,
                level,
                unit,
                target,
                x,
                y,
            } => progressive(charges, skill, level, unit, target, x, y).to_vec(),
            EventRecord::Preload { class } => preload(class).to_vec(),
        }
    }
}

/// S→C 0xA3 (`0x0053C0E0`, 24 bytes): v u8@1, skill u16@2, level u16@4,
/// type u8@6, GUID u32@7, target type u8@0xB, target u32@0xC, x u32@0x10,
/// y u32@0x14.
pub fn progressive(
    charges: u8,
    skill: u16,
    level: u16,
    unit: (u8, u32),
    target: (u8, u32),
    x: u32,
    y: u32,
) -> [u8; 24] {
    let mut m = [0u8; 24];
    m[0] = 0xA3;
    m[1] = charges;
    m[2..4].copy_from_slice(&skill.to_le_bytes());
    m[4..6].copy_from_slice(&level.to_le_bytes());
    m[6] = unit.0;
    m[7..11].copy_from_slice(&unit.1.to_le_bytes());
    m[11] = target.0;
    m[12..16].copy_from_slice(&target.1.to_le_bytes());
    m[16..20].copy_from_slice(&x.to_le_bytes());
    m[20..24].copy_from_slice(&y.to_le_bytes());
    m
}

/// S→C 0xA4 (`0x0053E1A0`, 3 bytes): class u16@1.
pub fn preload(class: u16) -> [u8; 3] {
    let c = class.to_le_bytes();
    [0xA4, c[0], c[1]]
}

/// S→C 0xA5 (`0x0053C190`, 8 bytes): unit type u8@1, GUID u32@2, skill
/// u16@6.
pub fn landing(ty: u8, guid: u32, skill: u16) -> [u8; 8] {
    let mut m = [0u8; 8];
    m[0] = 0xA5;
    m[1] = ty;
    m[2..6].copy_from_slice(&guid.to_le_bytes());
    m[6..8].copy_from_slice(&skill.to_le_bytes());
    m
}

/// S→C 0xAB (`0x0053C150`, 7 bytes): unit type u8@1, GUID u32@2, life
/// u8@6 (fraction of 128).
pub fn npc_heal(ty: u8, guid: u32, life: u8) -> [u8; 7] {
    let mut m = [0u8; 7];
    m[0] = 0xAB;
    m[1] = ty;
    m[2..6].copy_from_slice(&guid.to_le_bytes());
    m[6] = life;
    m
}

/// S→C 0x99, the 16-byte form of `0x0053D530` with flag 1 (§3.5 rule 5:
/// id 0x4C + 0x4D): the layout of 0x4C.
pub fn skill_event_on_unit(
    ty: u8,
    guid: u32,
    skill: u16,
    level: u8,
    target_ty: u8,
    target: u32,
    w: u16,
) -> [u8; 16] {
    let mut m = super::unit_update::skill_message::skill_on_unit(
        ty, guid, skill, level, target_ty, target, w,
    );
    m[0] = 0x99;
    m
}

/// S→C 0x9A, the 17-byte form (`0x0053D4D0`, or `0x0053D530` out of the
/// receiver's rooms, flag 1: id 0x4D + 0x4D): the layout of 0x4D.
pub fn skill_event_on_point(
    ty: u8,
    guid: u32,
    skill: u32,
    level: u8,
    x: u16,
    y: u16,
    w: u16,
) -> [u8; 17] {
    let mut m = super::unit_update::skill_message::skill_on_point(ty, guid, skill, level, x, y, w);
    m[0] = 0x9A;
    m
}

/// The records of every unit, by unit, in append order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EventRecords(std::collections::BTreeMap<UnitId, Vec<EventRecord>>);

impl EventRecords {
    /// Appends `r` to `unit`'s list (the writers' tail append).
    pub fn push(&mut self, unit: UnitId, r: EventRecord) {
        self.0.entry(unit).or_default().push(r);
    }
    /// Frees `unit`'s records (the room clean-up, §7.5 step 2).
    pub fn clear(&mut self, unit: UnitId) {
        self.0.remove(&unit);
    }
    /// `unit`'s records.
    pub fn of(&self, unit: UnitId) -> &[EventRecord] {
        self.0.get(&unit).map_or(&[], Vec::as_slice)
    }
}

impl<X: super::Pending> super::View<'_, X> {
    /// `0x00571CD0(unit, client)`: each pending record of `unit`, in list
    /// order, to `receiver` (the client's player).
    pub fn send_event_records(&mut self, game: &Game, receiver: UnitId, unit: UnitId) {
        let Some(r) = self.units.get(unit) else {
            return;
        };
        let (ty, guid, soft) = (r.ty as u8, r.guid, r.flags & UNIT_FLAG_SOFT_HIT != 0);
        for rec in self.h.event_records.of(unit).to_vec() {
            let m = match rec {
                // `0x00571CD0` (`0x00571DF6`-`0x00571E38`, §7.9 r2 (b)): 0xAB
                // only when the unit lacks flag 0x8000 and the receiver may
                // attack it (hostility, `combat/hit.md` §7.1).
                EventRecord::NpcHeal { .. } if soft || !self.h.x.may_attack(receiver, unit) => {
                    continue
                }
                // `0x0053D530` with flag 1: the receiver-dependent form.
                EventRecord::CastOnUnit {
                    skill,
                    level,
                    target,
                    w,
                } => self.skill_on_unit_message(
                    game,
                    receiver,
                    (ty, guid),
                    skill.into(),
                    level,
                    target,
                    w,
                    true,
                ),
                _ => rec.message((ty, guid)),
            };
            self.h.x.send(receiver, &m);
        }
    }
}
