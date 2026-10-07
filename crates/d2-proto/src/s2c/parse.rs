// Spec: specs/sim/intents-events.md
//! Client-side parse of one delivered S→C message (§3.3, §3.4 rule 3):
//! size by the §3.1 rule, then the typed decode of its id. Pure.

use super::messages::*;
use super::{audit, ServerMsg, Status};
use crate::generated::server as gen;
use crate::schema::Size;
use crate::transport::server_size;
use crate::wire::{DecodeError, FixedMessage};

/// Why a message did not parse.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("message is empty")]
    Empty,
    #[error("id 0x{id:02X}: not a valid S→C message (size rule gives 0)")]
    Invalid { id: u8 },
    #[error("id 0x{id:02X}: too few bytes for its size rule")]
    Incomplete { id: u8 },
    #[error("id 0x{id:02X}: {found} bytes, size rule gives {expected}")]
    WrongSize {
        id: u8,
        expected: usize,
        found: usize,
    },
    #[error("id 0x{found:02X}, expected 0x{expected:02X}")]
    WrongId { expected: u8, found: u8 },
    #[error("id 0x{id:02X}: byte {offset} is 0x{found:02X}, the layout fixes 0x{expected:02X}")]
    Const {
        id: u8,
        offset: usize,
        expected: u8,
        found: u8,
    },
    /// A valid message whose layout the specs do not fully give.
    #[error("id 0x{id:02X}: no full layout in the specs ({status:?})")]
    Unbuilt { id: u8, status: Status },
}

/// The size the §3.1 rule gives for `b`, which must be exactly that long.
pub(crate) fn expected_size(b: &[u8]) -> Result<usize, ParseError> {
    let id = *b.first().ok_or(ParseError::Empty)?;
    match server_size(b) {
        Size::Bytes(n) if n == b.len() => Ok(n),
        Size::Bytes(n) => Err(ParseError::WrongSize {
            id,
            expected: n,
            found: b.len(),
        }),
        Size::Incomplete => Err(ParseError::Incomplete { id }),
        Size::Invalid | Size::Negative(_) => Err(ParseError::Invalid { id }),
    }
}

fn generated<M: FixedMessage>(b: &[u8]) -> Result<M, ParseError> {
    M::decode(b).map_err(|e| match e {
        DecodeError::Empty => ParseError::Empty,
        DecodeError::WrongId { expected, found } => ParseError::WrongId { expected, found },
        DecodeError::WrongSize { expected, found } => ParseError::WrongSize {
            id: M::ID,
            expected,
            found,
        },
    })
}

macro_rules! messages {
    (
        generated: [ $( $gid:literal $g:ident ),* $(,)? ]
        built: [ $( $bid:literal $b:ident ),* $(,)? ]
    ) => {
        /// One parsed S→C message.
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub enum Message {
            $( $g(gen::$g), )*
            $( $b($b), )*
            WardenRequest(WardenRequest),
        }

        /// Parses one whole S→C message (as `split_server_buffer` yields
        /// it). Ids whose layout the specs do not fully give return
        /// [`ParseError::Unbuilt`].
        pub fn parse(b: &[u8]) -> Result<Message, ParseError> {
            expected_size(b)?;
            let id = b[0];
            // The mercenary form of 0x50 (u16 2 at 1) is not built.
            if id == QuestSpecial::ID && b[1..3] != [1, 0] {
                return Err(ParseError::Unbuilt { id, status: Status::Partial });
            }
            Ok(match id {
                $( $gid => Message::$g(generated::<gen::$g>(b)?), )*
                $( $bid => Message::$b(<$b as ServerMsg>::decode(b)?), )*
                0xAE => Message::WardenRequest(WardenRequest::decode(b)?),
                _ => return Err(ParseError::Unbuilt { id, status: audit(id).status }),
            })
        }

        /// Ids [`parse`] decodes.
        #[cfg(test)]
        pub(crate) const PARSED_IDS: &[u8] = &[ $( $gid, )* $( $bid, )* 0xAE ];
    };
}

messages! {
    generated: [
        0x00 GameLoading, 0x01 GameFlags, 0x02 LoadSuccessful, 0x03 LoadAct,
        0x04 LoadComplete, 0x05 UnloadComplete, 0x06 GameExit, 0x07 MapReveal,
        0x0C MonsterHit, 0x19 SmallGoldPickup, 0x1A AddExpByte, 0x1B AddExpWord,
        0x1C AddExpDword, 0x1D SetStatByte, 0x1E SetStatWord, 0x1F SetStatDword,
        0x3F UseStackableItem, 0x42 ClearCursor, 0x47 Relator1, 0x48 Relator2,
        0x4F StartMercList, 0x5F PortalFlags, 0x6E Unknown6E, 0x6F Unknown6F,
        0x70 Unknown70, 0x71 Unknown71, 0x72 Unknown72, 0x7D SetItemState,
        0x97 WeaponSwitch, 0xB0 ConnectionTerminated,
    ]
    built: [
        0x0B GameHandshake, 0x0D PlayerStop, 0x28 QuestInfo, 0x29 GameQuestInfo,
        0x2A NpcTransaction, 0x4E MercForHire, 0x50 QuestSpecial, 0x52 QuestLogInfo,
        0x58 OpenUi, 0x59 AssignPlayer, 0x5D QuestItemState, 0x63 WaypointMenu, 0x77 TradeAction, 0x89 UniqueEvent,
        0x8A NpcWantsInteract, 0x91 NpcGossipAct, 0x9B Unknown9B,
    ]
}
