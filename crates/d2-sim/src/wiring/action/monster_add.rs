// Spec: specs/sim/intents-events.md §7.1 rule 2.1, §7.2; specs/monsters/init.md §24
//! The monster add messages of `0x00571F90` (`intents-events.md` §7.2):
//! part A (0xAC `0x0053E2E0` in the full server layout of
//! `monsters/init.md` §24, stat 328, class 528's 0x98, the skill
//! messages 0x21, the unit states 0xAA; d2rs keeps no pending event
//! records, §7.9 rule 2) and part B (the mode message §7.4, then the
//! inventory messages `0x00534F80` for state 93 and classes 291, 417,
//! 418). Sent by the room switch and by the per-unit walk for a monster
//! not yet announced (§7.1 rule 2.1, [`View::monster_update`]).

use crate::monsters::init::{assign_mode, component_bits, type_flag, BitWriter, MonsterData};
use crate::stats::states::group;
use crate::units::messages;
use crate::units::{UnitId, UnitType};

use super::{Pending, View};

/// Stat 328, the position stat (§7.2: := (x + y) & 0xFFFF after 0xAC).
const STAT_POSITION: u16 = 328;
/// Class 528 sends 0x98 after 0xAC (§7.2).
const CLASS_98: u32 = 528;
/// State 93 and the classes whose part B sends the inventory (§7.2).
const STATE_INVENTORY: u16 = 93;
const CLASSES_INVENTORY: [u32; 3] = [291, 417, 418];
/// Flag-ex (+0xC8) bit 0x400: the source link of §24 rule 5.
const FLAG_EX_SOURCE: u32 = 0x400;
/// Unit flag (+0xC4) 0x200: sends the type block without umods (§24 rule 4).
const FLAG_TYPE_BLOCK: u32 = 0x200;
/// The 0xAC stream buffer (§24 rule 1).
const STREAM_MAX: usize = 0xF4;

/// The inputs of 0xAC (`monsters/init.md` §24, builder `0x0053E2E0`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AssignMonster {
    pub guid: u32,
    pub class: u16,
    pub x: u16,
    pub y: u16,
    /// `0x005A5650`.
    pub life: u8,
    /// The unit's mode (rule 2 picks what is sent).
    pub mode: u32,
    /// Monster data +0x04 and the class's choice counts.
    pub components: [u8; 16],
    pub counts: [u8; 16],
    /// Rule 4's block, when sent.
    pub type_block: Option<TypeBlock>,
    /// Rule 5: +0x98 when flag-ex 0x400 and owner type +0x94 = 0.
    pub source: Option<u32>,
    /// Rule 6: `None` when L is none; else its first 16 base entries
    /// (stat, param, value) with their itemstatcost send columns
    /// (`None`: the stat is out of range).
    pub stats: Option<Vec<StatEntry>>,
}

/// §24 rule 4's block.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TypeBlock {
    /// Type flags 4, 8, 2, 0x10, 0x40.
    pub flags: u16,
    pub hc_idx: u16,
    /// The umod list (bytes ≠ 0 are sent, at most 9).
    pub umods: Vec<u8>,
    pub name_seed: u16,
    /// The hireling owner's GUID (owner a player, pet type 7).
    pub hireling_owner: Option<u32>,
}

/// One base entry of rule 6.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatEntry {
    pub stat: u16,
    pub param: u16,
    pub value: i32,
    /// (`send other`, `send bits`, `send param bits`); `None` when the
    /// stat has no itemstatcost row.
    pub send: Option<(bool, u8, u8)>,
}

/// The 0xAC message (§24 rules 1–7).
pub fn assign_monster(a: &AssignMonster) -> Vec<u8> {
    let mut w = BitWriter::new();
    // Rule 2.
    w.write(u32::from(assign_mode(a.mode)), 4);
    // Rule 3.
    let any = a.components.iter().any(|&c| c != 0);
    w.write(u32::from(any), 1);
    if any {
        for (&c, &n) in a.components.iter().zip(&a.counts) {
            w.write(u32::from(c), component_bits(n));
        }
    }
    // Rule 4.
    w.write(u32::from(a.type_block.is_some()), 1);
    if let Some(t) = &a.type_block {
        for f in [
            type_flag::CHAMPION,
            type_flag::UNIQUE,
            type_flag::SUPERUNIQUE,
            type_flag::MINION,
            type_flag::GHOSTLY,
        ] {
            w.write(u32::from(t.flags & f != 0), 1);
        }
        if t.flags & type_flag::SUPERUNIQUE != 0 {
            w.write(u32::from(t.hc_idx), 16);
        }
        for &u in t.umods.iter().filter(|&&u| u != 0).take(9) {
            w.write(u32::from(u), 8);
        }
        w.write(0, 8);
        w.write(u32::from(t.name_seed), 16);
        match t.hireling_owner {
            Some(g) => {
                w.write(1, 1);
                w.write(g, 32);
            }
            None => w.write(0, 1),
        }
    }
    // Rule 5.
    match a.source {
        Some(v) => {
            w.write(1, 1);
            w.write(v & 0x8FFF_FFFF, 31);
        }
        None => w.write(0, 1),
    }
    // Rule 6.
    match &a.stats {
        None => w.write(0, 1),
        Some(entries) => {
            let mut sent = false;
            for e in entries.iter().take(16) {
                let Some((other, bits, param_bits)) = e.send else {
                    continue;
                };
                if !other || bits == 0 {
                    continue;
                }
                if !sent {
                    w.write(1, 1);
                    sent = true;
                }
                w.write(u32::from(e.stat), 9);
                if param_bits > 0 {
                    w.write(u32::from(e.param), u32::from(param_bits));
                }
                w.write(e.value as u32, u32::from(bits.min(32)));
            }
            if sent {
                w.write(0x1FF, 9);
            } else {
                w.write(0, 2);
            }
        }
    }
    // Rule 7: the writer keeps at most 0xF4 bytes.
    let mut stream = w.bytes;
    stream.truncate(STREAM_MAX);
    let mut m = Vec::with_capacity(13 + stream.len());
    m.push(0xAC);
    m.extend_from_slice(&a.guid.to_le_bytes());
    m.extend_from_slice(&a.class.to_le_bytes());
    m.extend_from_slice(&a.x.to_le_bytes());
    m.extend_from_slice(&a.y.to_le_bytes());
    m.push(a.life);
    m.push((13 + stream.len()) as u8);
    m.extend_from_slice(&stream);
    m
}

/// §24 rule 4: the block is sent when the umod list is non-empty or unit
/// flag 0x200 is set.
fn type_block(m: &MonsterData, unit_flags: u32, hireling_owner: Option<u32>) -> Option<TypeBlock> {
    if m.umod_count() == 0 && unit_flags & FLAG_TYPE_BLOCK == 0 {
        return None;
    }
    Some(TypeBlock {
        flags: m.type_flags,
        hc_idx: m.boss_hc_idx,
        umods: m.umod_list().to_vec(),
        name_seed: m.name_seed,
        hireling_owner,
    })
}

impl<X: Pending> View<'_, X> {
    /// Part A and part B of a monster's add messages to `receiver`'s
    /// client (§7.2).
    pub fn monster_add(&mut self, game: &crate::game::Game, receiver: UnitId, unit: UnitId) {
        let Some(r) = self.units.get(unit) else {
            return;
        };
        let (class, mode, guid, unit_flags, flags_ex) =
            (r.class, r.mode, r.guid, r.flags, r.flags2);
        let dead = r.is_dead();
        let (x, y) = self.h.path_position(unit);
        // Part A: 0xAC, skipped for a dead unit in a `hide` state.
        if !(dead && self.stats.has_group(unit, group::HIDE)) {
            let m =
                self.assign_monster_input(unit, class, mode, guid, unit_flags, flags_ex, (x, y));
            self.h.x.send(receiver, &assign_monster(&m));
        }
        let pos = (x.wrapping_add(y)) & 0xFFFF;
        self.stats
            .unit_set(&mut *self.h, unit, STAT_POSITION, pos, 0);
        if class == CLASS_98 {
            // PROVISIONAL (intents-events.md §7.2): u16@5 of 0x98
            // (`0x0053E0A0`) is 0; settled by a capture of a class-528
            // monster's add messages.
            self.h.x.send(receiver, &messages::unknown98(guid, 0));
        }
        for (skill, base, bonus) in self.h.x.monster_add_skills(unit) {
            let m =
                messages::update_oskill(UnitType::Monster as u8, false, guid, skill, base, bonus);
            self.h.x.send(receiver, &m);
        }
        let states = self.unit_states_message(UnitType::Monster as u8, guid, unit);
        self.h.x.send(receiver, &states);
        // Part B. Without the path provider no monster has a path record
        // (rule 4's fatal would be logged for every add), so the mode
        // message needs the provider, as [`View::monster_update`] does.
        let client = game
            .lists
            .clients()
            .into_iter()
            .find(|&c| game.lists.client(c).and_then(|e| e.player) == Some(receiver));
        if let Some(client) = client.filter(|_| self.h.paths.is_some()) {
            self.mode_message(game, client, receiver, unit);
        }
        if self.stats.has_state(unit, u32::from(STATE_INVENTORY))
            || CLASSES_INVENTORY.contains(&class)
        {
            self.h.x.inventory_messages(receiver, unit);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn assign_monster_input(
        &mut self,
        unit: UnitId,
        class: u32,
        mode: u32,
        guid: u32,
        unit_flags: u32,
        flags_ex: u32,
        (x, y): (i32, i32),
    ) -> AssignMonster {
        let life = crate::stats::life_fraction(
            self.stats
                .unit_total(unit, crate::stats::stat::HITPOINTS, 0),
            crate::combat::vitals::VitalsUnits::max_life(self, unit),
        ) as u8;
        let hireling_owner = self.h.x.hireling_owner_guid(unit);
        let data = self.h.monster_data(unit);
        let components = data.map_or([0; 16], |d| d.components);
        let type_block = data.and_then(|d| type_block(d, unit_flags, hireling_owner));
        let counts = self
            .h
            .monster_world
            .as_ref()
            .and_then(|w| w.component_counts(class))
            .unwrap_or([0; 16]);
        let source = (flags_ex & FLAG_EX_SOURCE != 0)
            .then(|| self.h.x.unit_owner(unit))
            .flatten()
            .and_then(|(ty, g)| (ty == 0).then_some(g));
        let bodies = self.h.bodies.as_deref();
        let stats = self.stats.list_by_state_flags(unit, 0, 0x40).map(|l| {
            self.stats
                .base_entries(l)
                .into_iter()
                .take(16)
                .map(|(k, v)| {
                    let stat = crate::stats::key_stat(k);
                    StatEntry {
                        stat,
                        param: k as u32 as u16,
                        value: v,
                        send: bodies
                            .and_then(|b| b.stat(i32::from(stat)))
                            .map(|r| (r.send_other, r.send_bits, r.send_param_bits)),
                    }
                })
                .collect()
        });
        AssignMonster {
            guid,
            class: class as u16,
            x: x as u16,
            y: y as u16,
            life,
            mode,
            components,
            counts,
            type_block,
            source,
            stats,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: specs/monsters/init.md §24 r1, §24 r2, §24 r3, §24 r4, §24 r5, §24 r6
    #[test]
    fn assign_monster_minimal_stream() {
        let a = AssignMonster {
            guid: 0x1B,
            class: 5,
            x: 0x1295,
            y: 0x1555,
            life: 128,
            mode: 2,
            ..AssignMonster::default()
        };
        // Mode 1 (walk is not sent), then five 0 bits: one byte 0x01.
        assert_eq!(
            assign_monster(&a),
            [0xAC, 0x1B, 0, 0, 0, 5, 0, 0x95, 0x12, 0x55, 0x15, 128, 14, 0x01]
        );
    }

    // Covers: specs/monsters/init.md §24 r3, §24 r4, §24 r6
    #[test]
    fn assign_monster_components_type_block_and_stats() {
        let mut components = [0u8; 16];
        components[0] = 1;
        let mut counts = [0u8; 16];
        counts[0] = 2;
        counts[1] = 5;
        let a = AssignMonster {
            mode: 12,
            components,
            counts,
            type_block: Some(TypeBlock {
                flags: type_flag::UNIQUE,
                hc_idx: 0,
                umods: vec![5, 0],
                name_seed: 0x1234,
                hireling_owner: None,
            }),
            stats: Some(vec![StatEntry {
                stat: 1,
                param: 0,
                value: 3,
                send: Some((false, 8, 0)),
            }]),
            ..AssignMonster::default()
        };
        let mut w = BitWriter::new();
        w.write(12, 4);
        w.write(1, 1);
        w.write(1, 1); // component 0, 1 bit
        w.write(0, 3); // component 1, count 5 → 3 bits
        for _ in 2..16 {
            w.write(0, 1);
        }
        w.write(1, 1);
        for bit in [0, 1, 0, 0, 0] {
            w.write(bit, 1);
        }
        w.write(5, 8);
        w.write(0, 8);
        w.write(0x1234, 16);
        w.write(0, 1); // no hireling owner
        w.write(0, 1); // no source link
        w.write(0, 2); // stats: none sent (no `send other`)
        let m = assign_monster(&a);
        assert_eq!(&m[13..], &w.bytes[..]);
        assert_eq!(m[12] as usize, 13 + w.bytes.len());
    }

    // Covers: specs/sim/intents-events.md §7.9 r3, §6 r6
    #[test]
    fn overhead_and_portal_builders() {
        assert_eq!(
            messages::overhead_chat(7, 2, 0x10, b"hi"),
            [0x26, 5, 7, 2, 0x10, 0, 0, 0, 0, 0, 0, b'h', b'i', 0]
        );
        let m = messages::portal_ownership(1, b"ab", 2, 0xFFFF_FFFF);
        assert_eq!(&m[..8], &[0x82, 1, 0, 0, 0, b'a', b'b', 0]);
        assert_eq!(&m[21..], &[2, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF]);
    }
}
