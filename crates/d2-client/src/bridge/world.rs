// Spec: specs/client/bridge.md
//! Client world model (§5): what the S→C messages have told the client.
//! Plain Rust, no Bevy. Not game state: `d2-sim` on the server is.

use std::collections::BTreeMap;

use d2_proto::transport::server_message;

/// Unit type of monsters (`sim/unit-order.md` §1 rule 1).
pub const MONSTER: u8 = 1;

/// A unit's identity: (unit type, GUID) (`sim/unit-order.md` §1 rule 1).
/// Ordered by type, then GUID.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnitKey {
    pub unit_type: u8,
    pub guid: u32,
}

/// A unit as the client knows it. Fields are added by the owner specs of
/// the messages that state them (spec §5 rule 2, open question 2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientUnit {
    pub key: UnitKey,
}

/// The client world model.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClientWorld {
    /// Bridge frames run (§5 rule 3).
    pub frames: u64,
    /// Bridge frames whose pump ran a server tick (§5 rule 3).
    pub server_ticks: u64,
    /// Units in key order (§5 rule 2).
    pub units: BTreeMap<UnitKey, ClientUnit>,
}

/// The unit an S→C message addresses through the receive table's unit
/// handler (`intents-events.md` §3.4 rule 3; spec §5 rule 4). `None` for
/// ids without a unit handler and for messages too short for the lookup
/// (open question 3).
pub fn addressed_unit(msg: &[u8]) -> Option<UnitKey> {
    let &id = msg.first()?;
    server_message(id)?.client_unit_handler?;
    let u32_at = |off: usize| -> Option<u32> {
        let b = msg.get(off..off + 4)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    if (0x67..=0x6D).contains(&id) {
        Some(UnitKey {
            unit_type: MONSTER,
            guid: u32_at(1)?,
        })
    } else {
        Some(UnitKey {
            unit_type: *msg.get(1)?,
            guid: u32_at(2)?,
        })
    }
}
