// Spec: specs/sim/intents-events.md
//! The S→C messages whose full layout a system spec gives. Names are the
//! `server-messages.tsv` labels; fields without a stated meaning are
//! named by offset (`f6`), as the generator does.

use super::s2c_message;

/// Size of a quest record (`world/quests.md` §1.5).
pub const QUEST_RECORD: usize = 96;
/// Size of a waypoint record (`world/waypoints.md` §2, §5.3).
pub const WAYPOINT_RECORD: usize = 16;
/// Entries of the quest log list (`world/quests.md` §6.2 step 4).
pub const QUEST_LOG_ENTRIES: usize = 41;
/// NPC slots of 0x91 (`world/quests.md` §6.7).
pub const GOSSIP_SLOTS: usize = 12;

s2c_message! {
    /// 0x0B GameHandshake (6 bytes), sender `0x0053B3D0`: the unit's type
    /// and GUID (`client/model.md` §3 rule 1, the receive layout;
    /// `items/inventory-moves.md` §9.1, the builder). At a single-player join it
    /// names the joining player (`client/model.md` §11 rule 3).
    0x0B GameHandshake 6 {
        /// `u8` at 1.
        unit_type: u8 = 1,
        /// `u32` at 2.
        unit_guid: u32 = 2,
    }
    consts []
    unwritten []
}

s2c_message! {
    /// 0x0D PlayerStop (13 bytes), sender `0x0053B4B0`. Values from
    /// `world/waypoints.md` §7 rule 7 (unit type, GUID, 1, x, y, 0, 0);
    /// widths from its recorded bytes `0d 00 01000000 01 2013 8413 00 00`.
    0x0D PlayerStop 13 {
        /// `u8` at 1.
        unit_type: u8 = 1,
        /// `u32` at 2.
        unit_guid: u32 = 2,
        /// `u8` at 6 (1 on waypoint arrival).
        f6: u8 = 6,
        /// `u16` at 7.
        x: u16 = 7,
        /// `u16` at 9.
        y: u16 = 9,
        /// `u8` at 11 (0 on waypoint arrival).
        f11: u8 = 11,
        /// `u8` at 12 (0 on waypoint arrival).
        f12: u8 = 12,
    }
    consts []
    unwritten []
}

s2c_message! {
    /// 0x28 QuestInfo (103 bytes), sender `0x0053D670`
    /// (`world/quests.md` §1.5): unit, a 0 byte, the player's quest record
    /// for the current difficulty.
    0x28 QuestInfo 103 {
        /// `u8` at 1: 1 (the NPC talked to) or 6 (quest code).
        unit_type: u8 = 1,
        /// `u32` at 2: the NPC's GUID, or 0 with type 6.
        unit_guid: u32 = 2,
        /// 96 bytes at 7.
        record: [u8; QUEST_RECORD] = 7,
    }
    consts [6 => 0]
    unwritten []
}

s2c_message! {
    /// 0x29 GameQuestInfo (97 bytes), sender `0x0053D700`
    /// (`world/quests.md` §1.5): the game quest record.
    0x29 GameQuestInfo 97 {
        /// 96 bytes at 1.
        record: [u8; QUEST_RECORD] = 1,
    }
    consts []
    unwritten []
}

s2c_message! {
    /// 0x2A NpcTransaction (15 bytes), builder `0x0053D740`
    /// (`world/npc.md` §9). Bytes 3–6 are not written by the original.
    0x2A NpcTransaction 15 {
        /// `u8` at 1.
        kind: u8 = 1,
        /// `u8` at 2: result code (`world/npc.md` §9 table).
        code: u8 = 2,
        /// `u32` at 7: item or mercenary GUID, or −1.
        guid: u32 = 7,
        /// `u32` at 11: player gold (stat 14) after the transaction.
        gold: u32 = 11,
    }
    consts []
    unwritten [3, 4, 5, 6]
}

s2c_message! {
    /// 0x4E MercForHire (7 bytes; `world/npc.md` §7.2): one hire-list
    /// offer.
    0x4E MercForHire 7 {
        /// `u16` at 1: name id.
        name: u16 = 1,
        /// `u32` at 3: slot seed.
        seed: u32 = 3,
    }
    consts []
    unwritten []
}

s2c_message! {
    /// 0x50 QuestSpecial (15 bytes), the quest form only (u16 1 at 1;
    /// `world/quests.md` §6.2 step 3). The mercenary form (u16 2 at 1,
    /// `world/npc.md` §7.5) has no stated bytes 5–14 and is not built.
    0x50 QuestSpecial 15 {
        /// `u16` at 3: Den of Evil monsters left.
        den_left: u16 = 3,
        /// `i16` at 5: true tomb level id − 66.
        staff_tomb: i16 = 5,
        /// `u16` at 7: barbarians left.
        barbarians_left: u16 = 7,
    }
    consts [1 => 1, 2 => 0, 9 => 0, 10 => 0, 11 => 0, 12 => 0, 13 => 0, 14 => 0]
    unwritten []
}

s2c_message! {
    /// 0x52 QuestLogInfo (42 bytes; `world/quests.md` §6.2 step 4):
    /// list[0..40].
    0x52 QuestLogInfo 42 {
        /// 41 bytes at 1.
        list: [u8; QUEST_LOG_ENTRIES] = 1,
    }
    consts []
    unwritten []
}

s2c_message! {
    /// 0x58 OpenUi (7 bytes), builder `0x0053D8D0` (`world/npc.md` §8.1):
    /// NPC service or object-insert result. Byte 6 (`effect`) is written
    /// only on the result-5 path (`0x005852E0`); for every other code the
    /// original leaves it unwritten, d2rs writes 0 and the comparison
    /// masks it by a keyed row (`sim/intents-events.md` §6 rules 3, 6).
    0x58 OpenUi 7 {
        /// `u32` at 1.
        npc_guid: u32 = 1,
        /// `u8` at 5: 6 done, 7 refused; 0, 1, 4, 5 object inserts.
        result: u8 = 5,
        /// `u8` at 6: "accepted with effect" (result 5 only).
        effect: u8 = 6,
    }
    consts []
    unwritten []
}

s2c_message! {
    /// 0x59 AssignPlayer (26 bytes), sender `0x0053E8F0`: part A of the
    /// player's add messages (`intents-events.md` §7.2: GUID, class u8
    /// (unit +0x04), name (player data), x, y (the unit's position)) at the
    /// offsets of the receive handler (`client/msg-units.md` §1.1 rule 1).
    0x59 AssignPlayer 26 {
        /// `u32` at 1.
        guid: u32 = 1,
        /// `u8` at 5.
        class: u8 = 5,
        /// 16 bytes at 6 (zero-padded).
        name: [u8; 16] = 6,
        /// `u16` at 0x16.
        x: u16 = 0x16,
        /// `u16` at 0x18.
        y: u16 = 0x18,
    }
    consts []
    unwritten []
}

s2c_message! {
    /// 0x5D QuestItemState (6 bytes), builder `0x0053D710`
    /// (`world/quests.md` §6.3): one quest's status.
    0x5D QuestItemState 6 {
        /// `u8` at 1.
        chain: u8 = 1,
        /// `u8` at 2: record flags (+0x14).
        flags: u8 = 2,
        /// `u8` at 3.
        status: u8 = 3,
        /// `u16` at 4: monsters / barbarians left, else 0.
        extra: u16 = 4,
    }
    consts []
    unwritten []
}

s2c_message! {
    /// 0x63 WaypointMenu (21 bytes), sender `0x0053D960`
    /// (`world/waypoints.md` §5.3).
    0x63 WaypointMenu 21 {
        /// `u32` at 1: waypoint object GUID.
        object_guid: u32 = 1,
        /// 16 bytes at 5: the player's record for the game difficulty.
        record: [u8; WAYPOINT_RECORD] = 5,
    }
    consts []
    unwritten []
}

s2c_message! {
    /// 0x77 TradeAction (2 bytes; `world/cube.md` §1: 0x0C, 0x11, 0x15).
    0x77 TradeAction 2 {
        /// `u8` at 1.
        action: u8 = 1,
    }
    consts []
    unwritten []
}

s2c_message! {
    /// 0x89 UniqueEvent (2 bytes), builder `0x005456F0`
    /// (`world/quests.md` §6.5).
    0x89 UniqueEvent 2 {
        /// `u8` at 1.
        event: u8 = 1,
    }
    consts []
    unwritten []
}

s2c_message! {
    /// 0x8A NpcWantsInteract (6 bytes; `world/quests.md` §6.4):
    /// `8A 01 <npc GUID>`.
    0x8A NpcWantsInteract 6 {
        /// `u32` at 2.
        npc_guid: u32 = 2,
    }
    consts [1 => 1]
    unwritten []
}

s2c_message! {
    /// 0x91 NpcGossipAct (26 bytes), builder `0x0053E060`
    /// (`world/quests.md` §6.7): introduced NPC class ids packed at the
    /// front, the rest 0xFFFF.
    0x91 NpcGossipAct 26 {
        /// `u8` at 1 (D2MOO: act).
        act: u8 = 1,
        /// 12 × `u16` at 2.
        slots: [u16; GOSSIP_SLOTS] = 2,
    }
    consts []
    unwritten []
}

s2c_message! {
    /// 0x9B Unknown9B (7 bytes), sender `0x0053E0E0` (`world/npc.md` §7.3
    /// step 4: u16 0xFFFF at 1, u32 0 at 3 after a resurrection).
    0x9B Unknown9B 7 {
        /// `u16` at 1.
        f1: u16 = 1,
        /// `u32` at 3.
        f3: u32 = 3,
    }
    consts []
    unwritten []
}

/// Largest 0xAE payload (`intents-events.md` §3.1 rule 1: a length over
/// 0x1FD gives size 0).
pub const WARDEN_MAX: usize = 0x1FD;

/// 0xAE WardenRequest (variable: `len` u16 at 1, `len` bytes at 3;
/// `server-messages.tsv`), sender `0x0053E840`. Transport row (§4 rule 4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WardenRequest {
    pub data: Vec<u8>,
}

impl WardenRequest {
    pub const ID: u8 = 0xAE;

    /// The message bytes. Panics if `data` is longer than [`WARDEN_MAX`].
    pub fn encode(&self) -> Vec<u8> {
        assert!(self.data.len() <= WARDEN_MAX, "0xAE payload too long");
        let mut b = Vec::with_capacity(3 + self.data.len());
        b.push(Self::ID);
        b.extend_from_slice(&(self.data.len() as u16).to_le_bytes());
        b.extend_from_slice(&self.data);
        b
    }

    /// Decodes one whole message (size from the §3.1 rule).
    pub fn decode(b: &[u8]) -> Result<Self, super::ParseError> {
        let expected = super::parse::expected_size(b)?;
        if b[0] != Self::ID {
            return Err(super::ParseError::WrongId {
                expected: Self::ID,
                found: b[0],
            });
        }
        Ok(Self {
            data: b[3..expected].to_vec(),
        })
    }
}
