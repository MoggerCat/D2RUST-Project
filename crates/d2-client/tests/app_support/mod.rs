// Spec: specs/client/bridge.md (§3), specs/sim/intents-events.md (§8)
//! Shared set-up of the app tests over the app's own single-player game:
//! a link the app owns while the test keeps a handle to the server thread
//! (the player exists only after the join, `intents-events.md` §8.2, so
//! the test looks it up after the app's frames ran the session sequence).
#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use d2_client::app::server_thread::ThreadLink;
use d2_client::app::single_player::{self, Link};
use d2_client::bridge::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};
use d2_server::seams::Clock;
use d2_sim::units::UnitId;

/// A link shared by the app (the bridge's link) and the test.
pub struct SharedLink<T>(pub Arc<Mutex<T>>);

impl<T: ServerLink> ServerLink for SharedLink<T> {
    fn protocol_version(&self) -> u32 {
        self.0.lock().unwrap().protocol_version()
    }
    fn send(&mut self, queue: SendQueue, msg: &[u8]) -> Result<Sent, LinkError> {
        self.0.lock().unwrap().send(queue, msg)
    }
    fn pump(&mut self) -> Result<Pumped, LinkError> {
        self.0.lock().unwrap().pump()
    }
    fn receive(&mut self) -> Vec<Vec<u8>> {
        self.0.lock().unwrap().receive()
    }
}

/// The app game's server thread, shared.
pub type Server<C> = Arc<Mutex<ThreadLink<Link<C>>>>;

/// The local player and its GUID, once the join ran.
pub fn local_player<C: Clock + Send + 'static>(server: &Server<C>) -> Option<(UnitId, u32)> {
    server
        .lock()
        .unwrap()
        .with(|l| single_player::local_player(&l.host().game))
        .unwrap()
}

/// Runs `f` on the server's link between frames.
pub fn with<C, R, F>(server: &Server<C>, f: F) -> R
where
    C: Clock + Send + 'static,
    R: Send + 'static,
    F: FnOnce(&mut Link<C>) -> R + Send + 'static,
{
    server.lock().unwrap().with(f).unwrap()
}
