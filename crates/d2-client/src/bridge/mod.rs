// Spec: specs/client/bridge.md
//! The only link between the Bevy app and the game. Outbound: requests
//! become 1.14d C→S bytes built with `d2-proto` ([`intent`]). Inbound: the
//! S→C bytes the server delivers are split with `d2-proto` and dispatched
//! by id to handlers that update the [`world::ClientWorld`] ([`receive`],
//! [`dispatch`]); ids no spec owns yet are recorded, never interpreted.
//! Bevy entities only mirror the world model ([`mirror`]). Contains no
//! game rules: the server decides every outcome.
//!
//! Everything except [`mirror`] is plain Rust without Bevy types.

pub mod dispatch;
pub mod intent;
pub mod link;
pub mod local;
pub mod mirror;
pub mod receive;
pub mod world;

#[cfg(test)]
mod gaps_numbered_tests;
#[cfg(test)]
mod local_tests;
#[cfg(test)]
mod tests;

use d2_proto::transport::SplitError;
use d2_proto::{FixedMessage, PROTOCOL_VERSION};

use dispatch::{Dispatch, TableError};
use intent::IntentError;
use link::{LinkError, Sent, ServerLink};
use receive::{receive_chunk, ReceiveLog};
use world::ClientWorld;

pub use link::{Pumped, SendQueue, LOCAL_CLIENT};
pub use local::{LocalLink, SinglePlayer};
pub use mirror::{BridgePlugin, BridgeResource, UnitView};
pub use world::{ClientUnit, UnitKey};

/// A bridge failure. All of them stop the frame (spec §8 rule 4).
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("server protocol version {server}, client {client}")]
    Version { client: u32, server: u32 },
    #[error(transparent)]
    Table(#[from] TableError),
    #[error(transparent)]
    Intent(#[from] IntentError),
    #[error(transparent)]
    Link(#[from] LinkError),
    /// A chunk 1.14d asserts on (spec §2 rule 4).
    #[error("S→C chunk refused: {0}")]
    Split(#[from] SplitError),
}

/// One bridge frame (spec §8 rule 1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameReport {
    pub ticked: bool,
    pub chunks: usize,
    pub messages: usize,
    pub handled: usize,
    pub unowned: usize,
    pub rejected: usize,
    pub discarded_bytes: usize,
}

/// The bridge: a server link, the client world model and the dispatch
/// table.
pub struct Bridge<L> {
    link: L,
    dispatch: Dispatch,
    world: ClientWorld,
    log: ReceiveLog,
}

impl<L: ServerLink> Bridge<L> {
    /// A bridge over `link` with the spec's dispatch table.
    pub fn new(link: L) -> Result<Self, BridgeError> {
        Self::with_dispatch(link, Dispatch::from_spec()?)
    }

    /// A bridge with a given dispatch table (tests and tools). Refuses a
    /// link of another protocol version (spec §9 rule 1).
    pub fn with_dispatch(link: L, dispatch: Dispatch) -> Result<Self, BridgeError> {
        let server = link.protocol_version();
        if server != PROTOCOL_VERSION {
            return Err(BridgeError::Version {
                client: PROTOCOL_VERSION,
                server,
            });
        }
        Ok(Self {
            link,
            dispatch,
            world: ClientWorld::default(),
            log: ReceiveLog::default(),
        })
    }

    /// Sends a typed C→S message (spec §4 rule 1).
    pub fn send<M: FixedMessage>(&mut self, msg: &M) -> Result<Sent, BridgeError> {
        self.send_bytes(&intent::encode(msg))
    }

    /// Sends C→S bytes after the classifier check (spec §4 rules 2–3).
    pub fn send_bytes(&mut self, msg: &[u8]) -> Result<Sent, BridgeError> {
        let queue = intent::route(msg)?;
        Ok(self.link.send(queue, msg)?)
    }

    /// One bridge frame: pump the server, then receive and dispatch every
    /// delivered chunk (spec §8 rule 1). A refused chunk ends the frame
    /// with an error; chunks after it in the same receive are not
    /// processed (a fatal assert in 1.14d).
    pub fn frame(&mut self) -> Result<FrameReport, BridgeError> {
        let pumped = self.link.pump()?;
        let mut report = FrameReport {
            ticked: pumped.ticked,
            ..FrameReport::default()
        };
        self.world.frames += 1;
        if pumped.ticked {
            self.world.server_ticks += 1;
        }
        for chunk in self.link.receive() {
            let c = receive_chunk(&mut self.world, &self.dispatch, &mut self.log, &chunk)?;
            report.chunks += 1;
            report.messages += c.messages;
            report.handled += c.handled;
            report.unowned += c.unowned;
            report.rejected += c.rejected;
            report.discarded_bytes += c.discarded_bytes;
        }
        Ok(report)
    }

    /// Applies one S→C chunk without pumping (spec §2).
    pub fn receive_chunk(&mut self, chunk: &[u8]) -> Result<receive::ChunkReport, BridgeError> {
        Ok(receive_chunk(
            &mut self.world,
            &self.dispatch,
            &mut self.log,
            chunk,
        )?)
    }

    pub fn world(&self) -> &ClientWorld {
        &self.world
    }

    pub fn log(&self) -> &ReceiveLog {
        &self.log
    }

    pub fn link(&self) -> &L {
        &self.link
    }

    pub fn link_mut(&mut self) -> &mut L {
        &mut self.link
    }
}

impl<L: ServerLink + ?Sized> ServerLink for Box<L> {
    fn protocol_version(&self) -> u32 {
        (**self).protocol_version()
    }

    fn send(&mut self, queue: SendQueue, msg: &[u8]) -> Result<Sent, LinkError> {
        (**self).send(queue, msg)
    }

    fn pump(&mut self) -> Result<Pumped, LinkError> {
        (**self).pump()
    }

    fn receive(&mut self) -> Vec<Vec<u8>> {
        (**self).receive()
    }
}
