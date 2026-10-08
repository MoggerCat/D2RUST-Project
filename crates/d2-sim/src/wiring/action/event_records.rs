// Spec: specs/sim/intents-events.md §7.9 rule 2 (pending event records), §7.3 rule 2 step 3, §7.5 step 2
//! The pending event records of a unit (unit +0xEC, `0x00571CD0`): the
//! writers append a record and queue the unit for update
//! (`0x0064C040`); the per-client update sends each record to the
//! client's player in list order (§7.3 rule 2 step 3, objects and
//! players as well); the room clean-up frees them (§7.5 step 2), so every
//! client in game gets the tick's records once.
//!
//! Writers wired here: 0xA3 (`0x00571AA0`, the progressive-charge
//! bodies' `queue_progressive`) and 0xA4 (`0x00571C00`, the AI's class
//! preload). The 0x9E hireling records are sent at once by
//! `world::hirelings::level` (§7.9 rule 2 is the same bytes); 0xA5, 0xAB,
//! 0x99, 0x9A and 0x23 have no d2rs writer yet.

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
}

impl EventRecord {
    /// The S→C message of the record (`0x00571CD0`'s sender per id).
    pub fn message(&self) -> Vec<u8> {
        match *self {
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

/// The records of every unit, by unit, in append order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EventRecords(std::collections::BTreeMap<UnitId, Vec<EventRecord>>);

impl EventRecords {
    /// Appends `r` to `unit`'s list (the writers' tail append).
    pub fn push(&mut self, unit: UnitId, r: EventRecord) {
        self.0.entry(unit).or_default().push(r);
    }
    /// The messages of `unit`'s records, in list order (`0x00571CD0`).
    pub fn messages(&self, unit: UnitId) -> Vec<Vec<u8>> {
        self.0
            .get(&unit)
            .map_or_else(Vec::new, |l| l.iter().map(EventRecord::message).collect())
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
    pub fn send_event_records(&mut self, receiver: UnitId, unit: UnitId) {
        for m in self.h.event_records.messages(unit) {
            self.h.x.send(receiver, &m);
        }
    }
}
