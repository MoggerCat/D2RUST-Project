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

/// The synthetic client's `skills` table: one row (skill 0). The
/// synthetic game has no `skills` rows, but the join of a new character
/// sends two 0x23 with skill 0 (`intents-events.md` §8.2 rules 3.7, 7),
/// and the client asserts a selected skill is inside its table
/// (`client/msg-skills.md` §2 rule 3); every install has row 0.
pub fn synthetic_skill_rows(app: &mut bevy::prelude::App) {
    app.world_mut()
        .resource_mut::<d2_client::bridge::BridgeResource>()
        .0
        .set_skill_rows(vec![d2_client::bridge::world::SkillRow::default()]);
}

/// The centre of the NPC menu box's selectable row `i` (the spec box sits
/// above the NPC, `ui/menus.md` §2.4–§2.6; `OriginalUi::npc_menu_row_point`).
pub fn npc_menu_row(app: &bevy::prelude::App, i: usize) -> d2_client::ui::Point {
    app.world()
        .non_send::<d2_client::world_view::WorldViewUi>()
        .original
        .as_ref()
        .expect("the original UI")
        .npc_menu_row_point(i)
        .unwrap_or_else(|| panic!("the NPC menu box has a row {i}"))
}

/// The NPC menu's selectable row count, 0 when no box is up.
pub fn npc_menu_len(app: &bevy::prelude::App) -> usize {
    app.world()
        .non_send::<d2_client::world_view::WorldViewUi>()
        .original
        .as_ref()
        .expect("the original UI")
        .npc_menu()
        .filter(|m| !m.talking)
        .map_or(0, |m| m.rows.len())
}
