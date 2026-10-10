// Spec: specs/client/bridge.md
//! The in-process server link (§3; `docs/ARCHITECTURE.md`: single player
//! runs as a local server): [`ServerLink`] on `d2_server::host::Host`,
//! with the method mapping of §3 rule 1. The host owns the clock (§3
//! rule 3), the duplicate filter (§4 rule 5), the queues, the tick
//! driver of `tick.md` §1 and the flush; this adapter only forwards and
//! never interprets a message.

use d2_proto::PROTOCOL_VERSION;
use d2_server::adapters::ProtoSizes;
use d2_server::host::{FrameReport as HostFrame, Host, HostError, SystemClock};
use d2_server::seams::{Clock, Intents, MessageSink, MessageSizes, SessionHandler, Tick};
use d2_server::transport::{Classified, SendError};

use super::link::{LinkError, Pumped, SendQueue, Sent, ServerLink, LOCAL_CLIENT};

/// A failure of the local host, as the link reports it.
#[derive(Debug, thiserror::Error)]
pub enum LocalError {
    /// A send 1.14d's transport asserts on.
    #[error(transparent)]
    Send(#[from] SendError),
    /// A host frame 1.14d ends with a fatal assert.
    #[error(transparent)]
    Host(#[from] HostError),
    /// The server classifier did not queue a message the bridge's
    /// classifier passed (both read `d2-proto`'s size rules, so this is
    /// a table disagreement, never a client decision).
    #[error("server classifier dropped message 0x{id:02X}: {class:?}")]
    Dropped { id: u8, class: Classified },
}

impl From<LocalError> for LinkError {
    fn from(e: LocalError) -> Self {
        LinkError::Server(Box::new(e))
    }
}

/// System messages 0x67..=0x70 until the session code exists
/// (`d2_server::seams::SessionHandler`, Phase 5 session spec): records
/// each drained message and answers nothing. It decides no outcome.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PendingSession {
    /// (client, drained bytes, full size), in drain order.
    pub received: Vec<(u32, Vec<u8>, usize)>,
}

/// S→C 0x8F (`0x0053E020`): the id and 32 zero bytes (0x21 bytes;
/// `sim/intents-events.md` §2.5 row 0x6D).
const PONG: [u8; 33] = {
    let mut b = [0; 33];
    b[0] = 0x8F;
    b
};

impl SessionHandler for PendingSession {
    fn system_message(&mut self, client: u32, msg: &[u8], size: usize, out: &mut dyn MessageSink) {
        self.received.push((client, msg.to_vec(), size));
        // C→S 0x6D (ping, `0x0052C400` → `0x005389A0`): the server logs the
        // round trip and answers the pong. The bytes of the ping are data
        // (the client's clock); the answer does not depend on them.
        if msg.first() == Some(&0x6D) {
            let _ = out.queue(client, &PONG);
        }
    }
}

/// The in-process `d2-server` host behind the bridge, for the local
/// player ([`LOCAL_CLIENT`], §3 rule 2).
pub struct LocalLink<G, S = ProtoSizes, H = PendingSession, C = SystemClock> {
    host: Host<G, S, H, C>,
    /// The server part of the last pump (diagnostics and tests).
    last: HostFrame,
    /// Run in each pump after the tick, before its flush
    /// ([`Host::frame_with`]; `state-dump` pokes, `tools/poke.md` §5 r4).
    tick_end: Option<TickEndHook<G, S, H, C>>,
}

/// A [`LocalLink`] tick-end hook.
pub type TickEndHook<G, S, H, C> = Box<dyn FnMut(&mut Host<G, S, H, C>) + Send>;

/// The single-player link of the app: `d2-proto` sizes, the real clock.
pub type SinglePlayer<G> = LocalLink<G, ProtoSizes, PendingSession, SystemClock>;

impl<G> SinglePlayer<G>
where
    G: Intents + Tick,
{
    /// A local server running `game` on the system clock.
    pub fn single_player(game: G) -> Self {
        Self::new(Host::new(
            game,
            ProtoSizes,
            PendingSession::default(),
            SystemClock::default(),
        ))
    }
}

impl<G, S, H, C> LocalLink<G, S, H, C>
where
    G: Intents + Tick,
    S: MessageSizes,
    H: SessionHandler,
    C: Clock,
{
    /// Wraps `host` and connects the local client to it. The game must
    /// already list the client (`SimGame::join`) for its messages to pass
    /// the gate and for the flush to reach it.
    pub fn new(mut host: Host<G, S, H, C>) -> Self {
        host.connect(LOCAL_CLIENT);
        Self {
            host,
            last: HostFrame::default(),
            tick_end: None,
        }
    }

    /// Installs `f` (replacing an earlier one), run in every pump that
    /// ticks, after the tick and before its flush.
    pub fn set_tick_end(&mut self, f: TickEndHook<G, S, H, C>) {
        self.tick_end = Some(f);
    }

    pub fn host(&self) -> &Host<G, S, H, C> {
        &self.host
    }

    pub fn host_mut(&mut self) -> &mut Host<G, S, H, C> {
        &mut self.host
    }

    /// The server part of the last pump: drained messages and their
    /// fate, tick, flush.
    pub fn last_frame(&self) -> &HostFrame {
        &self.last
    }

    fn queued(msg: &[u8], class: Classified) -> Result<Sent, LinkError> {
        match class {
            Classified::Queued(_) => Ok(Sent::Queued),
            class => Err(LocalError::Dropped { id: msg[0], class }.into()),
        }
    }
}

impl<G, S, H, C> ServerLink for LocalLink<G, S, H, C>
where
    G: Intents + Tick,
    S: MessageSizes,
    H: SessionHandler,
    C: Clock,
{
    fn protocol_version(&self) -> u32 {
        PROTOCOL_VERSION
    }

    /// `Host::send_game` (with the client's duplicate filter) or
    /// `Host::send_system`.
    fn send(&mut self, queue: SendQueue, msg: &[u8]) -> Result<Sent, LinkError> {
        match queue {
            SendQueue::Game => match self
                .host
                .send_game(LOCAL_CLIENT, msg)
                .map_err(LocalError::from)?
            {
                None => Ok(Sent::Filtered),
                Some(class) => Self::queued(msg, class),
            },
            SendQueue::System => {
                let class = self
                    .host
                    .send_system(LOCAL_CLIENT, msg)
                    .map_err(LocalError::from)?;
                Self::queued(msg, class)
            }
        }
    }

    /// `Host::frame`: drain → tick driver → flush if a tick ran.
    fn pump(&mut self) -> Result<Pumped, LinkError> {
        let hook = &mut self.tick_end;
        self.last = self
            .host
            .frame_with(|h| {
                if let Some(f) = hook.as_mut() {
                    f(h)
                }
            })
            .map_err(LocalError::from)?;
        Ok(Pumped {
            ticked: self.last.ticked,
        })
    }

    /// `Host::receive(LOCAL_CLIENT)`: system list first, one message per
    /// chunk as local delivery returns them.
    fn receive(&mut self) -> Vec<Vec<u8>> {
        self.host.receive(LOCAL_CLIENT)
    }
}
