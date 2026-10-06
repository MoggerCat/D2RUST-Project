// Spec: specs/sim/intents-events.md
//! Typed S→C builders and the client-side parser (§3, §4 rule 2).
//!
//! One type per S→C message whose whole layout a spec gives (every byte
//! after the id is a field, a constant or a byte the 1.14d builder does
//! not write). Layout sources, per type: `world/npc.md` §7, §8.1, §9;
//! `world/quests.md` §1.5, §6.2–§6.7; `world/waypoints.md` §5.3, §7 rule
//! 7; `world/cube.md` §1; `items/inventory.md` §11 and the
//! `server-messages.tsv` `layout` column (those rows are typed in
//! [`crate::generated::server`] and re-exported here).
//!
//! - [`ServerMsg`]: the hand-written messages ([`messages`]).
//! - [`parse`]: one delivered message → [`Message`]; ids without a full
//!   layout are [`ParseError::Unbuilt`] with their [`audit`] status.
//! - [`audit`]: every id 0x00–0xB4 → built / generated / partial /
//!   unspecified / never (mirrored in `docs/handoff/s2c-builders.md`).
//!
//! Bytes the original leaves unwritten (stack contents) are written as 0
//! (as `d2-sim`'s builders do) and listed in [`ServerMsg::UNWRITTEN`] so a
//! comparison can mask them; the parser ignores them.

pub mod audit;
mod field;
pub mod messages;
mod parse;

pub use audit::{audit, Audit, Status, AUDIT};
pub use messages::*;
pub use parse::{parse, Message, ParseError};

// Messages typed by the generator from the TSV `layout` column.
pub use crate::generated::server::{
    AddExpByte, AddExpDword, AddExpWord, ClearCursor, ConnectionTerminated, GameExit, GameFlags,
    GameLoading, LoadAct, LoadComplete, LoadSuccessful, MapReveal, MonsterHit, PortalFlags,
    Relator1, Relator2, SetItemState, SetStatByte, SetStatDword, SetStatWord, SmallGoldPickup,
    StartMercList, Unknown6E, Unknown6F, Unknown70, Unknown71, Unknown72, UnloadComplete,
    UseStackableItem, WeaponSwitch,
};

/// A fixed-size S→C message with a full layout from a spec.
pub trait ServerMsg: Sized {
    const ID: u8;
    const SIZE: usize;
    /// Fields: name, offset, byte length.
    const FIELDS: &'static [(&'static str, usize, usize)];
    /// Bytes the sender always writes with this value (offset, value).
    const CONSTS: &'static [(usize, u8)];
    /// Bytes the 1.14d builder does not write (written 0 here).
    const UNWRITTEN: &'static [usize];
    /// Decodes exactly `SIZE` bytes starting with `ID`; checks
    /// [`Self::CONSTS`], ignores [`Self::UNWRITTEN`].
    fn decode(b: &[u8]) -> Result<Self, ParseError>;
    /// Writes the message into `out` (`SIZE` bytes). Panics on another
    /// length.
    fn write(&self, out: &mut [u8]);
}

/// Shared checks of [`ServerMsg::decode`].
fn check<M: ServerMsg>(b: &[u8]) -> Result<(), ParseError> {
    match b.first() {
        None => return Err(ParseError::Empty),
        Some(&f) if f != M::ID => {
            return Err(ParseError::WrongId {
                expected: M::ID,
                found: f,
            })
        }
        _ => {}
    }
    if b.len() != M::SIZE {
        return Err(ParseError::WrongSize {
            id: M::ID,
            expected: M::SIZE,
            found: b.len(),
        });
    }
    for &(offset, expected) in M::CONSTS {
        if b[offset] != expected {
            return Err(ParseError::Const {
                id: M::ID,
                offset,
                expected,
                found: b[offset],
            });
        }
    }
    Ok(())
}

/// Zeroes `out`, writes the id and the constants.
fn start<M: ServerMsg>(out: &mut [u8]) {
    assert_eq!(out.len(), M::SIZE, "output buffer size");
    out.fill(0);
    out[0] = M::ID;
    for &(offset, v) in M::CONSTS {
        out[offset] = v;
    }
}

/// Defines a [`ServerMsg`] struct from its layout.
macro_rules! s2c_message {
    (
        $(#[$doc:meta])*
        $id:literal $name:ident $size:literal {
            $( $(#[$fdoc:meta])* $field:ident : $ty:ty = $off:literal ),* $(,)?
        }
        consts [ $( $coff:literal => $cval:literal ),* $(,)? ]
        unwritten [ $( $uoff:literal ),* $(,)? ]
    ) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct $name {
            $( $(#[$fdoc])* pub $field: $ty, )*
        }

        impl $crate::s2c::ServerMsg for $name {
            const ID: u8 = $id;
            const SIZE: usize = $size;
            const FIELDS: &'static [(&'static str, usize, usize)] = &[
                $( (stringify!($field), $off, <$ty as $crate::s2c::field::WireField>::LEN), )*
            ];
            const CONSTS: &'static [(usize, u8)] = &[ $( ($coff, $cval), )* ];
            const UNWRITTEN: &'static [usize] = &[ $( $uoff, )* ];
            #[allow(unused_variables)]
            fn decode(b: &[u8]) -> Result<Self, $crate::s2c::ParseError> {
                $crate::s2c::check::<Self>(b)?;
                Ok(Self {
                    $( $field: <$ty as $crate::s2c::field::WireField>::read(b, $off), )*
                })
            }
            #[allow(unused_variables)]
            fn write(&self, out: &mut [u8]) {
                $crate::s2c::start::<Self>(out);
                $( $crate::s2c::field::WireField::write(&self.$field, out, $off); )*
            }
        }

        impl $name {
            /// The message bytes (unwritten bytes 0).
            pub fn encode(&self) -> [u8; $size] {
                let mut b = [0; $size];
                $crate::s2c::ServerMsg::write(self, &mut b);
                b
            }
        }
    };
}
pub(crate) use s2c_message;

#[cfg(test)]
mod tests;
