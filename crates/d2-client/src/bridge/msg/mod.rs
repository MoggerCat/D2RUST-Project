// Spec: specs/client/bridge-dispatch.tsv (owner rows), specs/client/model.md, specs/client/msg-units.md, specs/client/msg-stats-items.md
//! The S→C handlers of the client world model, one per owned id of
//! `bridge-dispatch.tsv`, registered in [`HANDLERS`]:
//!
//! - [`session`]: `client/model.md` (0x00–0x08, 0x0B);
//! - [`pets`]: `client/model.md` §14 (0x7A, 0x81);
//! - [`units`]: `client/msg-units.md` (unit add, remove, place, queued
//!   movement and action messages, the local player's vitals);
//! - [`stats_items`]: `client/msg-stats-items.md` (stats, items).

pub mod pets;
pub mod session;
pub mod stats_items;
pub mod units;

#[cfg(test)]
mod tests_model;
#[cfg(test)]
mod tests_stats_items;
#[cfg(test)]
mod tests_units;

use super::dispatch::{Handle, Handler, HandlerError, HandlerFn, UnitHandlerFn};

pub const MODEL: &str = "specs/client/model.md";
pub const UNITS: &str = "specs/client/msg-units.md";
pub const STATS_ITEMS: &str = "specs/client/msg-stats-items.md";

const fn general(id: u8, owner: &'static str, f: HandlerFn) -> Handler {
    Handler {
        id,
        owner,
        handle: Handle::General(f),
    }
}

const fn unit(id: u8, f: UnitHandlerFn) -> Handler {
    Handler {
        id,
        owner: UNITS,
        handle: Handle::Unit(f),
    }
}

/// Every owned id's handler, in id order.
pub const HANDLERS: &[Handler] = &[
    general(0x00, MODEL, session::game_loading),
    general(0x01, MODEL, session::game_flags),
    general(0x02, MODEL, session::load_successful),
    general(0x03, MODEL, session::load_act),
    general(0x04, MODEL, session::load_complete),
    general(0x05, MODEL, session::unload_complete),
    general(0x06, MODEL, session::game_exit),
    general(0x07, MODEL, session::map_reveal),
    general(0x08, MODEL, session::map_hide),
    general(0x0A, UNITS, units::remove_unit),
    general(0x0B, MODEL, session::game_handshake),
    unit(0x0C, units::queued),
    unit(0x0D, units::queued),
    unit(0x0E, units::queued),
    unit(0x0F, units::queued),
    unit(0x10, units::queued),
    general(0x15, UNITS, units::reassign_player),
    general(0x18, UNITS, units::vitals),
    general(0x19, STATS_ITEMS, stats_items::local_stat),
    general(0x1A, STATS_ITEMS, stats_items::local_stat),
    general(0x1B, STATS_ITEMS, stats_items::local_stat),
    general(0x1C, STATS_ITEMS, stats_items::local_stat),
    general(0x1D, STATS_ITEMS, stats_items::local_stat),
    general(0x1E, STATS_ITEMS, stats_items::local_stat),
    general(0x1F, STATS_ITEMS, stats_items::local_stat),
    general(0x20, STATS_ITEMS, stats_items::stat_update),
    general(0x3F, STATS_ITEMS, stats_items::use_stackable_item),
    general(0x42, STATS_ITEMS, stats_items::clear_cursor),
    general(0x47, STATS_ITEMS, stats_items::relator),
    general(0x48, STATS_ITEMS, stats_items::relator),
    unit(0x4C, units::queued),
    unit(0x4D, units::queued),
    general(0x51, UNITS, units::assign_object),
    general(0x59, UNITS, units::assign_player),
    unit(0x67, units::queued),
    unit(0x68, units::queued),
    unit(0x69, units::queued),
    unit(0x6A, units::queued),
    unit(0x6B, units::queued),
    unit(0x6C, units::queued),
    unit(0x6D, units::queued),
    unit(0x6E, units::no_effect),
    unit(0x6F, units::no_effect),
    unit(0x70, units::no_effect),
    unit(0x71, units::no_effect),
    unit(0x72, units::no_effect),
    general(0x7A, MODEL, pets::pet_action),
    general(0x81, MODEL, pets::assign_merc),
    general(0x95, UNITS, units::vitals),
    general(0x96, UNITS, units::vitals),
    general(0x9C, STATS_ITEMS, stats_items::item_action),
    general(0x9D, STATS_ITEMS, stats_items::item_action),
    general(0xAC, UNITS, units::assign_monster),
];

/// Little-endian field reads with a handler error for a short message.
pub(crate) struct Bytes<'a>(pub &'a [u8]);

impl Bytes<'_> {
    fn get(&self, off: usize, n: usize) -> Result<&[u8], HandlerError> {
        self.0
            .get(off..off + n)
            .ok_or(HandlerError::Invalid("message too short for its layout"))
    }

    pub fn u8(&self, off: usize) -> Result<u8, HandlerError> {
        Ok(self.get(off, 1)?[0])
    }

    pub fn u16(&self, off: usize) -> Result<u16, HandlerError> {
        let b = self.get(off, 2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn u32(&self, off: usize) -> Result<u32, HandlerError> {
        let b = self.get(off, 4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn slice(&self, off: usize, n: usize) -> Result<&[u8], HandlerError> {
        self.get(off, n)
    }
}

/// Test support: the spec's dispatch over a bare model.
#[cfg(test)]
pub(crate) mod support {
    use super::super::dispatch::Dispatch;
    use super::super::receive::{receive_chunk, ReceiveLog};
    use super::super::update::update_pass;
    use super::super::world::{ClientUnit, ClientWorld, ModelInputs, UnitKey};

    /// A model, its inputs and the receive log, driven by the spec's
    /// dispatch table.
    #[derive(Default)]
    pub struct Model {
        pub w: ClientWorld,
        pub inputs: ModelInputs,
        pub log: ReceiveLog,
    }

    /// Bytes from a hex string ("6d 06 00 …").
    pub fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace()
            .map(|b| u8::from_str_radix(b, 16).unwrap())
            .collect()
    }

    impl Model {
        /// Receives `bytes` as one chunk; panics if the chunk is refused.
        pub fn recv(&mut self, bytes: &[u8]) -> &mut Self {
            let d = Dispatch::from_spec().unwrap();
            receive_chunk(&mut self.w, &self.inputs, &d, &mut self.log, bytes).unwrap();
            self
        }

        /// Receives the hex message.
        pub fn hex(&mut self, s: &str) -> &mut Self {
            self.recv(&hex(s))
        }

        /// Runs the update pass; returns the messages applied.
        pub fn drain(&mut self) -> usize {
            let d = Dispatch::from_spec().unwrap();
            update_pass(&mut self.w, &self.inputs, &d, &mut self.log)
        }

        /// Inserts a bare unit (fixture state).
        pub fn put(&mut self, key: UnitKey) -> &mut ClientUnit {
            self.w.units.insert(key, ClientUnit::new(key));
            self.w.units.get_mut(&key).unwrap()
        }

        pub fn unit(&self, key: UnitKey) -> &ClientUnit {
            &self.w.units[&key]
        }

        /// The ids the handlers rejected, in order.
        pub fn rejected(&self) -> Vec<(u8, String)> {
            self.log
                .rejected
                .iter()
                .map(|r| (r.id, r.error.to_string()))
                .collect()
        }
    }
}
