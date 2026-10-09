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

/// The reason every test on the user's files carries: the real-data
/// gate (`tools/realdata-gate.sh`, `--run-ignored only`) runs it; CI has
/// no game files and skips it.
pub const REAL_DATA: &str = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)";

/// The user's 1.14d install (`$D2_GAME_DIR`), loaded as `d2-client play`
/// loads it (`GameData::select`), once per test binary. No directory is a
/// failure, never a fallback (M23): the tests that call this are
/// `#[ignore = REAL_DATA]`.
pub fn game_data() -> single_player::GameData {
    static DATA: std::sync::OnceLock<single_player::GameData> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        let dir = std::env::var_os("D2_GAME_DIR")
            .map(std::path::PathBuf::from)
            .expect("D2_GAME_DIR: this test runs on the user's 1.14d install");
        single_player::GameData::select(Some(&dir)).expect("the install loads")
    })
    .clone()
}

/// The user's live data (`GameData::Live`).
pub fn live() -> Arc<single_player::LiveData> {
    let single_player::GameData::Live(d) = game_data();
    d
}

/// The client tables `play::add_live_client` gives the bridge on the
/// user's files: skills, class skills, skill tables, unit rows, item
/// tables, object rows (for tests that wire the app by hand).
pub fn live_tables(app: &mut bevy::prelude::App) {
    let d = live();
    let data = single_player::GameData::Live(d.clone());
    let a = d.archives.as_ref();
    let mut b = app
        .world_mut()
        .resource_mut::<d2_client::bridge::BridgeResource>();
    b.0.set_skill_rows(single_player::client_skill_rows(a).unwrap());
    b.0.set_class_skills(single_player::client_class_skills(a).unwrap());
    b.0.set_skill_tables(Arc::new(single_player::client_skill_tables(a).unwrap()));
    b.0.set_unit_rows(single_player::client_unit_rows(a).unwrap());
    b.0.set_item_tables(Arc::new(d2_client::app::items::TableDecoder(Arc::new(
        d.tables.item_tables().unwrap(),
    ))));
    b.0.set_object_rows(single_player::client_object_rows(&data));
}

/// The `lvlwarp` id (the warp tile unit's class, `levels.md` §10.4) of
/// the warp from level `from` to level `to`: the install's `levels` row
/// of `from`, the `Warp` of the `Vis` slot that names `to`.
pub fn warp_id(from: u32, to: u32) -> u32 {
    let d = live();
    let l = &d.levels.drlg.levels[from as usize];
    let slot = l
        .vis
        .iter()
        .position(|&v| v == to)
        .unwrap_or_else(|| panic!("level {from} has no way to level {to}"));
    u32::try_from(l.warp[slot]).expect("a warp id")
}
